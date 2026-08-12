//! Memory port: semantic search and knowledge graph for agent memory.
//!
//! [INPUT]: query string · content · metadata · relation triples
//! [OUTPUT]: MemoryHit (scored results) · store confirmation
//!
//! Design: Port trait for pluggable memory backends (agentmemory, local, noop).
//! Supports semantic search, knowledge graph relations, and lifecycle management.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Memory search result with relevance score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryHit {
    /// Unique key for this memory entry.
    pub key: String,
    
    /// Full content text.
    pub content: String,
    
    /// Relevance score (0.0 - 1.0, higher = more relevant).
    pub score: f32,
    
    /// Structured metadata for filtering and context.
    pub metadata: Metadata,
    
    /// When this memory was created.
    pub created_at: DateTime<Utc>,
}

/// Structured metadata for memory entries.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metadata {
    /// Project identifier (file path).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    
    /// Task role (browser, backend, frontend, etc).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_role: Option<String>,
    
    /// Provider name (claude, codex, fake).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    
    /// Outcome classification (success, timeout, rejected, error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    
    /// Free-form tags for categorization.
    #[serde(default)]
    pub tags: Vec<String>,
    
    /// Additional key-value pairs.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Relation between two memory entries (for knowledge graphs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    /// Source memory key.
    pub from: String,
    
    /// Target memory key.
    pub to: String,
    
    /// Relation type (solved_by, caused_by, relates_to, etc).
    pub relation: String,
}

/// Memory port trait for semantic search and knowledge graphs.
#[async_trait]
pub trait MemoryPort: Send + Sync {
    /// Semantic search for relevant memories.
    ///
    /// # Arguments
    /// - `query`: Natural language query string
    /// - `limit`: Maximum number of results to return
    ///
    /// # Returns
    /// Vector of `MemoryHit` sorted by relevance score (descending).
    async fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<MemoryHit>>;
    
    /// Store a new memory entry.
    ///
    /// # Arguments
    /// - `key`: Unique identifier (e.g., "task-browser-ui-verify-2026-08-12")
    /// - `content`: Full text content to store and index
    /// - `metadata`: Structured metadata for filtering
    async fn store(
        &self,
        key: &str,
        content: &str,
        metadata: Metadata,
    ) -> anyhow::Result<()>;
    
    /// Create a relation between two memory entries (knowledge graph).
    ///
    /// # Arguments
    /// - `from`: Source memory key
    /// - `to`: Target memory key
    /// - `relation`: Relation type (e.g., "solved_by", "caused_by")
    async fn relate(&self, from: &str, to: &str, relation: &str) -> anyhow::Result<()>;
    
    /// Delete a memory entry by key.
    async fn delete(&self, key: &str) -> anyhow::Result<()>;
    
    /// Check if memory backend is available and healthy.
    async fn health(&self) -> anyhow::Result<()>;
}

/// No-op memory implementation (backward compatible fallback).
///
/// Returns empty results for all operations, allowing the system to run
/// without memory support. Used when memory feature is disabled.
pub struct NoOpMemory;

#[async_trait]
impl MemoryPort for NoOpMemory {
    async fn search(&self, _query: &str, _limit: usize) -> anyhow::Result<Vec<MemoryHit>> {
        Ok(vec![])
    }
    
    async fn store(
        &self,
        _key: &str,
        _content: &str,
        _metadata: Metadata,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    
    async fn relate(&self, _from: &str, _to: &str, _relation: &str) -> anyhow::Result<()> {
        Ok(())
    }
    
    async fn delete(&self, _key: &str) -> anyhow::Result<()> {
        Ok(())
    }
    
    async fn health(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn noop_memory_returns_empty() {
        let mem = NoOpMemory;
        let hits = mem.search("test query", 10).await.unwrap();
        assert!(hits.is_empty(), "NoOpMemory should return empty results");
    }
    
    #[tokio::test]
    async fn noop_memory_store_succeeds() {
        let mem = NoOpMemory;
        let metadata = Metadata {
            project_id: Some("test-project".to_string()),
            task_role: Some("backend".to_string()),
            ..Default::default()
        };
        
        mem.store("test-key", "test content", metadata)
            .await
            .unwrap();
    }
    
    #[tokio::test]
    async fn noop_memory_relate_succeeds() {
        let mem = NoOpMemory;
        mem.relate("key1", "key2", "solved_by").await.unwrap();
    }
    
    #[tokio::test]
    async fn noop_memory_delete_succeeds() {
        let mem = NoOpMemory;
        mem.delete("test-key").await.unwrap();
    }
    
    #[tokio::test]
    async fn noop_memory_health_succeeds() {
        let mem = NoOpMemory;
        mem.health().await.unwrap();
    }
    
    #[test]
    fn metadata_serialization() {
        let metadata = Metadata {
            project_id: Some("/path/to/project".to_string()),
            task_role: Some("browser".to_string()),
            provider: Some("claude".to_string()),
            outcome: Some("timeout".to_string()),
            tags: vec!["ui".to_string(), "flaky".to_string()],
            extra: {
                let mut map = HashMap::new();
                map.insert("retry_count".to_string(), serde_json::json!(3));
                map
            },
        };
        
        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: Metadata = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.project_id, metadata.project_id);
        assert_eq!(deserialized.task_role, metadata.task_role);
        assert_eq!(deserialized.provider, metadata.provider);
        assert_eq!(deserialized.outcome, metadata.outcome);
        assert_eq!(deserialized.tags, metadata.tags);
        assert_eq!(
            deserialized.extra.get("retry_count"),
            metadata.extra.get("retry_count")
        );
    }
}
