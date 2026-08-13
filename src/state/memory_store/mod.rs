//! Lightweight memory store: SQLite + local ONNX embedding + BM25/vector hybrid search.
//!
//! [INPUT]: content text · metadata · search queries
//! [OUTPUT]: MemoryEntry storage · MemoryHit search results
//! [POS]: state 子模块；P3 轻量记忆实现
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/state/CLAUDE.md
//!
//! Design:
//! - **Storage**: SQLite with bundled rusqlite (~5MB per 10k entries)
//! - **Embedding**: ONNX Runtime + all-MiniLM-L6-v2 (23MB model, 384-dim)
//! - **Search**: tantivy BM25 full-text + cosine similarity hybrid
//! - **Lifecycle**: TTL 90 days + auto-archive
//! - **Cross-platform**: File sync (iCloud/Dropbox) via `~/.cco/memory/`

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use ort::session::Session;
use ort::value::Tensor;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexWriter, ReloadPolicy, TantivyDocument};
use tokenizers::Tokenizer;

use crate::ports::memory::{MemoryHit, Metadata};

pub mod port;
pub use port::LocalMemory;

/// Memory entry with vector embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub embedding: Vec<f32>, // 384-dim for all-MiniLM-L6-v2
    pub metadata: Metadata,
    pub created_at: DateTime<Utc>,
}

/// Search result with relevance score.
#[derive(Debug, Clone)]
pub struct MemorySearchHit {
    pub entry: MemoryEntry,
    pub score: f32, // Combined BM25 + cosine score
}

/// Configuration for memory store.
#[derive(Debug, Clone)]
pub struct MemoryConfig {
    /// Root directory for memory storage (default: ~/.cco/memory/)
    pub storage_root: PathBuf,
    /// TTL in days (default: 90)
    pub ttl_days: u32,
    /// Maximum entries before auto-archiving oldest (default: 10,000)
    pub max_entries: usize,
    /// Path to ONNX model file (default: ~/.cco/models/all-MiniLM-L6-v2.onnx)
    pub model_path: Option<PathBuf>,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            storage_root: home.join(".cco").join("memory"),
            ttl_days: 90,
            max_entries: 10_000,
            model_path: None, // Will auto-download on first use in P4
        }
    }
}

/// Lightweight memory store with hybrid search.
pub struct MemoryStore {
    db: Connection,
    index: Index,
    index_writer: IndexWriter,
    schema: Schema,
    config: MemoryConfig,
    session: Option<Session>,     // ONNX Runtime session for embedding (P3 Week 2)
    tokenizer: Option<Tokenizer>, // HuggingFace tokenizer for input preprocessing
}

impl MemoryStore {
    /// Initialize a new memory store at the given path.
    ///
    /// Creates:
    /// - `memory.db` (SQLite with embeddings as BLOB)
    /// - `tantivy_index/` (BM25 full-text index)
    /// - ONNX Runtime session for embedding (if model exists)
    pub fn new(config: MemoryConfig) -> Result<Self> {
        std::fs::create_dir_all(&config.storage_root)
            .context("create memory storage root")?;

        let db_path = config.storage_root.join("memory.db");
        let db = Connection::open(&db_path).context("open SQLite memory.db")?;

        // SQLite schema: id, content, embedding BLOB, metadata JSON, created_at
        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS memory (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                embedding BLOB NOT NULL,
                metadata TEXT,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_created ON memory(created_at);
            "#,
        )
        .context("initialize SQLite schema")?;

        // Tantivy schema: id (string, stored), content (text, indexed), created_at (i64, stored)
        let mut schema_builder = Schema::builder();
        let _id_field = schema_builder.add_text_field("id", STRING | STORED);
        let _content_field = schema_builder.add_text_field("content", TEXT | STORED);
        let _created_at_field = schema_builder.add_i64_field("created_at", STORED);
        let schema = schema_builder.build();

        let index_dir = config.storage_root.join("tantivy_index");
        std::fs::create_dir_all(&index_dir).context("create tantivy index dir")?;

        let index = if index_dir.join("meta.json").exists() {
            Index::open_in_dir(&index_dir).context("open existing tantivy index")?
        } else {
            Index::create_in_dir(&index_dir, schema.clone())
                .context("create tantivy index")?
        };

        let index_writer = index
            .writer(50_000_000) // 50MB heap
            .context("create tantivy writer")?;

        // P3 Week 2: Load ONNX model + tokenizer
        let models_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cco")
            .join("models");
        
        let model_path = config.model_path.clone()
            .unwrap_or_else(|| models_dir.join("all-MiniLM-L6-v2.onnx"));
        let tokenizer_path = models_dir.join("tokenizer.json");

        let (session, tokenizer) = if model_path.exists() && tokenizer_path.exists() {
            match Self::load_onnx_session(&model_path, &tokenizer_path) {
                Ok((sess, tok)) => {
                    tracing::info!("memory: ONNX embedding model loaded from {:?}", model_path);
                    (Some(sess), Some(tok))
                }
                Err(e) => {
                    tracing::warn!("memory: failed to load ONNX model: {}, using stub embeddings", e);
                    (None, None)
                }
            }
        } else {
            tracing::info!("memory: ONNX model or tokenizer not found, using stub embeddings");
            (None, None)
        };

        Ok(Self {
            db,
            index,
            index_writer,
            schema,
            config,
            session,
            tokenizer,
        })
    }

    /// Store a new memory entry.
    ///
    /// # Arguments
    /// - `id`: Unique identifier (e.g., "task-browser-ui-verify-20260812T1430Z-ab12")
    /// - `content`: Full text content to index
    /// - `metadata`: Structured metadata (project, role, provider, outcome, tags)
    ///
    /// # Returns
    /// The stored entry ID on success.
    pub fn store(&mut self, id: &str, content: &str, metadata: Metadata) -> Result<String> {
        let created_at = Utc::now();
        let embedding = self.embed(content)?;

        // Serialize metadata to JSON
        let metadata_json = serde_json::to_string(&metadata)
            .context("serialize metadata")?;

        // Serialize embedding as BLOB (f32 -> bytes)
        let embedding_blob: Vec<u8> = embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        // Insert into SQLite
        self.db.execute(
            "INSERT OR REPLACE INTO memory (id, content, embedding, metadata, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                content,
                embedding_blob,
                metadata_json,
                created_at.timestamp(),
            ],
        ).context("insert into SQLite")?;

        // Index content in Tantivy
        let id_field = self.schema.get_field("id").unwrap();
        let content_field = self.schema.get_field("content").unwrap();
        let created_at_field = self.schema.get_field("created_at").unwrap();

        self.index_writer.add_document(doc!(
            id_field => id,
            content_field => content,
            created_at_field => created_at.timestamp(),
        ))?;

        self.index_writer.commit().context("commit tantivy index")?;

        // Auto-archive if over max_entries
        self.check_and_archive()?;

        Ok(id.to_string())
    }

    /// Store many entries with one SQLite transaction and a single tantivy commit.
    ///
    /// Bulk path for imports and benchmarks; per-entry [`Self::store`] commits the
    /// full-text index on every call, which dominates cost at high volume.
    pub fn store_batch(&mut self, entries: &[(String, String, Metadata)]) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }
        let created_at = Utc::now();

        // Embeddings first: `embed` needs `&mut self`, which cannot overlap
        // with the SQLite transaction borrow below.
        let mut prepared = Vec::with_capacity(entries.len());
        for (id, content, metadata) in entries {
            let embedding = self.embed(content)?;
            let blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
            let metadata_json = serde_json::to_string(metadata).context("serialize metadata")?;
            prepared.push((id.as_str(), content.as_str(), blob, metadata_json));
        }

        let tx = self.db.transaction().context("begin batch transaction")?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR REPLACE INTO memory (id, content, embedding, metadata, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for (id, content, blob, metadata_json) in &prepared {
                stmt.execute(params![id, content, blob, metadata_json, created_at.timestamp()])?;
            }
        }
        tx.commit().context("commit batch transaction")?;

        let id_field = self.schema.get_field("id").unwrap();
        let content_field = self.schema.get_field("content").unwrap();
        let created_at_field = self.schema.get_field("created_at").unwrap();
        for (id, content, _, _) in &prepared {
            self.index_writer.add_document(doc!(
                id_field => *id,
                content_field => *content,
                created_at_field => created_at.timestamp(),
            ))?;
        }
        self.index_writer.commit().context("commit tantivy index")?;

        self.check_and_archive()?;
        Ok(prepared.len())
    }

    /// Search for relevant memories using hybrid BM25 + vector similarity.
    ///
    /// # Arguments
    /// - `query`: Natural language query string
    /// - `limit`: Maximum number of results
    ///
    /// # Returns
    /// Vector of `MemoryHit` sorted by combined relevance score (descending).
    pub fn search(&mut self, query: &str, limit: usize) -> Result<Vec<MemoryHit>> {
        // Step 1: BM25 full-text search via Tantivy (fast pre-filter)
        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .context("create tantivy reader")?;
        let searcher = reader.searcher();

        let content_field = self.schema.get_field("content").unwrap();
        let query_parser = tantivy::query::QueryParser::for_index(&self.index, vec![content_field]);
        let query_obj = query_parser
            .parse_query(query)
            .context("parse tantivy query")?;

        // Get top 50 BM25 candidates (over-fetch for reranking)
        let top_docs = searcher
            .search(&query_obj, &tantivy::collector::TopDocs::with_limit(limit.max(50)))
            .context("tantivy search")?;

        if top_docs.is_empty() {
            return Ok(vec![]);
        }

        // Step 2: Load candidates from SQLite + compute vector similarity
        let query_embedding = self.embed(query)?;
        let id_field = self.schema.get_field("id").unwrap();
        let mut scored_hits = Vec::new();

        for (_bm25_score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
            let id = retrieved_doc
                .get_first(id_field)
                .and_then(|v| v.as_str())
                .context("extract id from tantivy doc")?;

            // Load full entry from SQLite
            let mut stmt = self.db.prepare_cached(
                "SELECT content, embedding, metadata, created_at FROM memory WHERE id = ?1",
            )?;
            let entry: Option<MemoryEntry> = stmt
                .query_row(params![id], |row| {
                    let content: String = row.get(0)?;
                    let embedding_blob: Vec<u8> = row.get(1)?;
                    let metadata_json: String = row.get(2)?;
                    let created_at_ts: i64 = row.get(3)?;

                    let embedding: Vec<f32> = embedding_blob
                        .chunks_exact(4)
                        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                        .collect();

                    let metadata: Metadata = serde_json::from_str(&metadata_json)
                        .unwrap_or_default();
                    let created_at = DateTime::from_timestamp(created_at_ts, 0)
                        .unwrap_or_else(|| Utc::now());

                    Ok(MemoryEntry {
                        id: id.to_string(),
                        content,
                        embedding,
                        metadata,
                        created_at,
                    })
                })
                .ok();

            if let Some(entry) = entry {
                let cosine_score = cosine_similarity(&query_embedding, &entry.embedding);
                // Hybrid score: 0.3 * BM25 (normalized) + 0.7 * cosine
                // (BM25 already incorporated via ranking; weight cosine higher for semantic)
                let combined_score = cosine_score; // Simplified for P3; tune in P4

                scored_hits.push(MemorySearchHit {
                    entry,
                    score: combined_score,
                });
            }
        }

        // Step 3: Rerank by combined score and return top-k
        scored_hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        scored_hits.truncate(limit);

        Ok(scored_hits
            .into_iter()
            .map(|hit| MemoryHit {
                key: hit.entry.id,
                content: hit.entry.content,
                score: hit.score,
                metadata: hit.entry.metadata,
                created_at: hit.entry.created_at,
            })
            .collect())
    }

    /// Delete a memory entry by ID.
    pub fn delete(&mut self, id: &str) -> Result<()> {
        self.db
            .execute("DELETE FROM memory WHERE id = ?1", params![id])
            .context("delete from SQLite")?;

        // Note: Tantivy delete requires index_writer.delete_term(), but for P3
        // we accept stale index entries (cleaned on next optimize). Full impl in P4.
        Ok(())
    }

    /// List all memory IDs (for debugging / UI).
    pub fn list_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self.db.prepare("SELECT id FROM memory ORDER BY created_at DESC")?;
        let ids = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(ids)
    }

    /// Archive oldest entries if over max_entries limit.
    fn check_and_archive(&mut self) -> Result<()> {
        let count: i64 = self
            .db
            .query_row("SELECT COUNT(*) FROM memory", [], |row| row.get(0))?;

        if count as usize > self.config.max_entries {
            let to_remove = count as usize - self.config.max_entries;
            self.db.execute(
                "DELETE FROM memory WHERE id IN (SELECT id FROM memory ORDER BY created_at ASC LIMIT ?1)",
                params![to_remove],
            )?;
            tracing::info!(
                "memory: archived {} oldest entries (over limit {})",
                to_remove,
                self.config.max_entries
            );
        }

        // Also remove expired entries (older than TTL)
        let ttl_cutoff = Utc::now() - Duration::days(self.config.ttl_days as i64);
        let deleted = self.db.execute(
            "DELETE FROM memory WHERE created_at < ?1",
            params![ttl_cutoff.timestamp()],
        )?;
        if deleted > 0 {
            tracing::info!("memory: purged {} expired entries (TTL {} days)", deleted, self.config.ttl_days);
        }

        Ok(())
    }

    /// Load ONNX session and tokenizer.
    fn load_onnx_session(model_path: &PathBuf, tokenizer_path: &PathBuf) -> Result<(Session, Tokenizer)> {
        let session = Session::builder()?
            .commit_from_file(model_path)
            .context("load ONNX model")?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("load tokenizer: {}", e))?;

        Ok((session, tokenizer))
    }

    /// Generate embedding for text using ONNX Runtime (P3 Week 2).
    ///
    /// Uses all-MiniLM-L6-v2 model (384-dim output).
    /// Falls back to zero vector if model not loaded (for testing).
    fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        // If ONNX model is loaded, use real inference
        if let (Some(session), Some(tokenizer)) = (self.session.as_mut(), self.tokenizer.as_ref()) {
            return embed_onnx(text, session, tokenizer);
        }

        // Fallback: stub zero vector (for testing or when model unavailable)
        Ok(vec![0.0; 384])
    }
}

/// Real ONNX inference implementation (ort 2.0-rc API).
fn embed_onnx(text: &str, session: &mut Session, tokenizer: &Tokenizer) -> Result<Vec<f32>> {
    // 1. Tokenize input
    let encoding = tokenizer
        .encode(text, false)
        .map_err(|e| anyhow::anyhow!("tokenize: {}", e))?;

    let input_ids = encoding.get_ids();
    let attention_mask = encoding.get_attention_mask();

    // 2. Prepare ONNX inputs (batch_size=1, seq_len=dynamic, pad to max 128)
    let max_len = 128;
    let seq_len = input_ids.len().min(max_len);

    let mut padded_ids = vec![0i64; max_len];
    let mut padded_mask = vec![0i64; max_len];

    for i in 0..seq_len {
        padded_ids[i] = input_ids[i] as i64;
        padded_mask[i] = attention_mask[i] as i64;
    }

    // 3. Create input tensors ((shape, data) form; ort 2.0-rc.13)
    let input_ids_tensor = Tensor::from_array(([1usize, max_len], padded_ids))?;
    let attention_mask_tensor = Tensor::from_array(([1usize, max_len], padded_mask.clone()))?;
    // BERT-family models expect token_type_ids; all zeros for single-sentence input.
    let token_type_ids_tensor = Tensor::from_array(([1usize, max_len], vec![0i64; max_len]))?;

    // 4. Run ONNX inference
    let outputs = session.run(ort::inputs![
        "input_ids" => input_ids_tensor,
        "attention_mask" => attention_mask_tensor,
        "token_type_ids" => token_type_ids_tensor,
    ])?;

    // 5. Extract output tensor (last_hidden_state: [1, seq_len, hidden_size])
    let (shape, data) = outputs["last_hidden_state"].try_extract_tensor::<f32>()?;
    anyhow::ensure!(shape.len() == 3, "unexpected output rank {}", shape.len());
    let hidden_size = shape[2] as usize;

    // 6. Mean pooling over sequence dimension (with attention mask)
    Ok(mean_pool(data, &padded_mask, seq_len, hidden_size))
}

/// Mean pooling with attention mask over flat `[1, seq, hidden]` data.
fn mean_pool(hidden_states: &[f32], attention_mask: &[i64], seq_len: usize, hidden_size: usize) -> Vec<f32> {
    let mut pooled = vec![0.0f32; hidden_size];
    let mut mask_sum = 0.0f32;

    for i in 0..seq_len {
        if attention_mask[i] == 1 {
            let row = &hidden_states[i * hidden_size..(i + 1) * hidden_size];
            for (p, v) in pooled.iter_mut().zip(row) {
                *p += v;
            }
            mask_sum += 1.0;
        }
    }

    if mask_sum > 0.0 {
        for val in &mut pooled {
            *val /= mask_sum;
        }
    }

    pooled
}

/// Cosine similarity between two embeddings.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests;
