//! Terminal session registry (persisted under run dir).

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::external::{self, detect_launcher, ExternalLauncher};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Embedded,
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSession {
    pub id: String,
    pub task_id: String,
    pub kind: SessionKind,
    pub launcher: Option<String>,
    pub cwd: PathBuf,
    pub command: String,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default)]
    pub closed: bool,
    /// For embedded/log panes: path being tailed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Store {
    sessions: Vec<TerminalSession>,
}

pub struct TerminalManager {
    store_path: PathBuf,
    prefer_launcher: String,
    custom_command: Option<String>,
    max_external: usize,
    max_embedded: usize,
}

impl TerminalManager {
    pub fn for_run(run_dir: &Path, prefer_launcher: &str, custom_command: Option<String>) -> Self {
        Self {
            store_path: run_dir.join("terminals.json"),
            prefer_launcher: prefer_launcher.to_string(),
            custom_command,
            max_external: 8,
            max_embedded: 6,
        }
    }

    pub fn with_limits(mut self, max_embedded: usize, max_external: usize) -> Self {
        self.max_embedded = max_embedded;
        self.max_external = max_external;
        self
    }

    fn load(&self) -> Result<Store> {
        if !self.store_path.exists() {
            return Ok(Store::default());
        }
        let text = std::fs::read_to_string(&self.store_path)
            .with_context(|| format!("read {}", self.store_path.display()))?;
        Ok(serde_json::from_str(&text).unwrap_or_default())
    }

    fn save(&self, store: &Store) -> Result<()> {
        if let Some(parent) = self.store_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            &self.store_path,
            serde_json::to_string_pretty(store)?,
        )?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<TerminalSession>> {
        Ok(self.load()?.sessions)
    }

    pub fn list_for_task(&self, task_id: &str) -> Result<Vec<TerminalSession>> {
        Ok(self
            .load()?
            .sessions
            .into_iter()
            .filter(|s| s.task_id == task_id && !s.closed)
            .collect())
    }

    /// Register an embedded (log-follow) session without spawning a PTY (TUI later).
    pub fn open_embedded(
        &self,
        task_id: &str,
        cwd: &Path,
        log_path: &Path,
    ) -> Result<TerminalSession> {
        let mut store = self.load()?;
        let open_embedded = store
            .sessions
            .iter()
            .filter(|s| matches!(s.kind, SessionKind::Embedded) && !s.closed)
            .count();
        if open_embedded >= self.max_embedded {
            bail!(
                "max embedded terminals reached ({})",
                self.max_embedded
            );
        }

        let session = TerminalSession {
            id: short_id(),
            task_id: task_id.into(),
            kind: SessionKind::Embedded,
            launcher: None,
            cwd: cwd.to_path_buf(),
            command: format!("tail -f {}", log_path.display()),
            created_at: Utc::now(),
            pid: None,
            closed: false,
            log_path: Some(log_path.to_path_buf()),
        };
        store.sessions.push(session.clone());
        self.save(&store)?;
        Ok(session)
    }

    /// Open external terminal window following task logs (or custom command).
    pub fn open_external(
        &self,
        task_id: &str,
        cwd: &Path,
        command: &str,
    ) -> Result<TerminalSession> {
        let mut store = self.load()?;
        let open_ext = store
            .sessions
            .iter()
            .filter(|s| matches!(s.kind, SessionKind::External) && !s.closed)
            .count();
        if open_ext >= self.max_external {
            bail!("max external terminals reached ({})", self.max_external);
        }

        let launcher = detect_launcher(&self.prefer_launcher);
        let pid = external::open_window(
            launcher,
            cwd,
            command,
            self.custom_command.as_deref(),
            task_id,
        )?;

        let session = TerminalSession {
            id: short_id(),
            task_id: task_id.into(),
            kind: SessionKind::External,
            launcher: Some(launcher.as_str().into()),
            cwd: cwd.to_path_buf(),
            command: command.into(),
            created_at: Utc::now(),
            pid,
            closed: false,
            log_path: None,
        };
        store.sessions.push(session.clone());
        self.save(&store)?;
        Ok(session)
    }

    pub fn open_follow_logs(
        &self,
        task_id: &str,
        cwd: &Path,
        stdout_path: &Path,
        stderr_path: &Path,
        kind: SessionKind,
    ) -> Result<TerminalSession> {
        match kind {
            SessionKind::External => {
                let cmd = external::follow_logs_command(stdout_path, stderr_path);
                self.open_external(task_id, cwd, &cmd)
            }
            SessionKind::Embedded => self.open_embedded(task_id, cwd, stdout_path),
        }
    }

    pub fn open_shell(&self, task_id: &str, cwd: &Path) -> Result<TerminalSession> {
        let cmd = external::shell_in_dir_command();
        self.open_external(task_id, cwd, &cmd)
    }

    pub fn close(&self, session_id: &str) -> Result<TerminalSession> {
        let mut store = self.load()?;
        let Some(session) = store.sessions.iter_mut().find(|s| s.id == session_id) else {
            bail!("session not found: {session_id}");
        };
        if session.closed {
            return Ok(session.clone());
        }
        if let Some(pid) = session.pid {
            kill_pid(pid);
        }
        session.closed = true;
        let out = session.clone();
        self.save(&store)?;
        Ok(out)
    }

    pub fn close_task(&self, task_id: &str) -> Result<usize> {
        let mut store = self.load()?;
        let mut n = 0;
        for s in store.sessions.iter_mut() {
            if s.task_id == task_id && !s.closed {
                if let Some(pid) = s.pid {
                    kill_pid(pid);
                }
                s.closed = true;
                n += 1;
            }
        }
        self.save(&store)?;
        Ok(n)
    }

    pub fn detected_launcher(&self) -> ExternalLauncher {
        detect_launcher(&self.prefer_launcher)
    }
}

fn short_id() -> String {
    Uuid::new_v4().to_string()[..8].to_string()
}

fn kill_pid(pid: u32) {
    #[cfg(unix)]
    {
        unsafe {
            extern "C" {
                fn kill(pid: i32, sig: i32) -> i32;
            }
            let _ = kill(pid as i32, 15);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status();
    }
}
