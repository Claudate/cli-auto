//! Shared private types and path helpers for the scheduler modules.
//!
//! [INPUT]: run_dir paths
//! [OUTPUT]: ProgressWatch · StallAction · stdout_len · mirror_run
//! [POS]: runtime/scheduler private
//! [PROTOCOL]: 变更时更新 scheduler/mod.rs 头部

use std::path::Path;

use anyhow::{Context, Result};

/// In-memory progress fingerprint for stall patrol (not persisted).
pub(super) struct ProgressWatch {
    pub last_bytes: u64,
    pub last_change: chrono::DateTime<chrono::Utc>,
}

pub(super) struct StallAction {
    pub reason_code: String,
    pub reason: String,
}

pub(super) fn stdout_len(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

pub(super) fn mirror_run(src: &Path, dst_root: &Path) -> Result<()> {
    let name = src.file_name().context("run dir name")?;
    let dst = dst_root.join(name);
    copy_dir_all(src, &dst)?;
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for ent in std::fs::read_dir(src)? {
        let ent = ent?;
        let ty = ent.file_type()?;
        let to = dst.join(ent.file_name());
        if ty.is_dir() {
            copy_dir_all(&ent.path(), &to)?;
        } else {
            std::fs::copy(ent.path(), to)?;
        }
    }
    Ok(())
}
