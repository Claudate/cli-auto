//! LocalMemory: `MemoryPort` adapter over the local [`MemoryStore`].
//!
//! [INPUT]: MemoryPort calls (search/store/delete/health)
//! [OUTPUT]: local SQLite + tantivy backed results
//! [POS]: state/memory_store 子模块；scheduler 等异步调用方经 ports 使用
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/state/CLAUDE.md
//!
//! Design: opens the store per call (short-lived tantivy writer lock) so a
//! long-running scheduler does not starve the planner-side best-effort writes.
//! `relate` is a no-op in P3 (no local knowledge graph yet).

use async_trait::async_trait;

use crate::ports::memory::{MemoryHit, MemoryPort, Metadata};

use super::{MemoryConfig, MemoryStore};

/// Local memory backend handle (config only; store opened per operation).
pub struct LocalMemory {
    cfg: MemoryConfig,
}

impl LocalMemory {
    pub fn new(cfg: MemoryConfig) -> Self {
        Self { cfg }
    }

    fn open(&self) -> anyhow::Result<MemoryStore> {
        MemoryStore::new(self.cfg.clone())
    }
}

#[async_trait]
impl MemoryPort for LocalMemory {
    async fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<MemoryHit>> {
        self.open()?.search(query, limit)
    }

    async fn store(&self, key: &str, content: &str, metadata: Metadata) -> anyhow::Result<()> {
        self.open()?.store(key, content, metadata)?;
        Ok(())
    }

    async fn relate(&self, _from: &str, _to: &str, _relation: &str) -> anyhow::Result<()> {
        // P3: no local knowledge graph; accepted silently for forward compat.
        Ok(())
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.open()?.delete(key)
    }

    async fn health(&self) -> anyhow::Result<()> {
        self.open().map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn local_memory_roundtrip_via_port() {
        let dir = tempdir().unwrap();
        let mem = LocalMemory::new(MemoryConfig {
            storage_root: dir.path().to_path_buf(),
            ..Default::default()
        });

        mem.health().await.unwrap();

        let meta = Metadata {
            provider: Some("claude".into()),
            task_role: Some("implement".into()),
            outcome: Some("timeout".into()),
            ..Default::default()
        };
        mem.store("o1", "outcome claude implement timeout", meta)
            .await
            .unwrap();

        let hits = mem.search("outcome claude implement", 5).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].key, "o1");
        assert_eq!(hits[0].metadata.outcome.as_deref(), Some("timeout"));

        mem.delete("o1").await.unwrap();
    }
}
