//! Preview launch detection: npm scripts, then static `index.html` + python http.server.
//!
//! [INPUT]: project root
//! [OUTPUT]: PreviewCmd (shell exec body · label · optional hint URL)
//! [POS]: services/preview · no Mode B
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/services/CLAUDE.md

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Result};

use super::{port_open, resolve_bin, shell_single_quote};

/// Prefer free ports used by static servers / common local tools.
const STATIC_PORTS: &[u16] = &[8080, 8000, 5500, 5173, 4173, 5000, 4321, 3000];

/// Resolved command to write into `.cco/preview/run.sh`.
#[derive(Debug, Clone)]
pub struct PreviewCmd {
    /// Lines after `cd $project` (may include `export` + `exec`).
    pub exec_body: String,
    pub label: String,
    /// When the process does not print a Local URL (python -m http.server).
    pub hint_url: Option<String>,
}

/// npm `dev|start|preview` if present; else static `index.html` via python http.server.
pub fn detect_preview_cmd(project: &Path) -> Result<PreviewCmd> {
    if let Some(cmd) = try_npm_cmd(project)? {
        return Ok(cmd);
    }
    if let Some(cmd) = try_static_cmd(project)? {
        return Ok(cmd);
    }
    bail!(
        "无法自动启动预览。\n\
· Node 项目：根目录需要 package.json，且 scripts 含 dev / start / preview 之一\n\
· 静态站：根目录需要 index.html（将用 python3 -m http.server）\n\
当前两者都不满足，请补入口文件后再发「启动本地预览」。"
    );
}

fn try_npm_cmd(project: &Path) -> Result<Option<PreviewCmd>> {
    let pkg = project.join("package.json");
    if !pkg.is_file() {
        return Ok(None);
    }
    let raw = match fs::read_to_string(&pkg) {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };
    let v: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let Some(scripts) = v.get("scripts").and_then(|s| s.as_object()) else {
        return Ok(None);
    };
    let order = ["dev", "start", "preview"];
    let Some(name) = order
        .iter()
        .find(|k| scripts.contains_key(**k))
        .map(|s| (*s).to_string())
    else {
        return Ok(None);
    };
    // npm missing → fall through to static (if any), not a hard error.
    let Ok(npm) = resolve_bin("npm") else {
        return Ok(None);
    };
    let npm_q = shell_single_quote(&npm.display().to_string());
    let script_q = shell_single_quote(&name);
    Ok(Some(PreviewCmd {
        exec_body: format!(
            "export BROWSER=none\nexport CI=true\nexec {npm_q} run {script_q}\n"
        ),
        label: format!("{} run {name}", npm.display()),
        hint_url: None,
    }))
}

fn try_static_cmd(project: &Path) -> Result<Option<PreviewCmd>> {
    if !project.join("index.html").is_file() {
        return Ok(None);
    }
    let port = match pick_free_port() {
        Some(p) => p,
        None => bail!("常用预览端口都被占用，请先关掉其它本地服务后再试"),
    };
    let python = resolve_python()?;
    let py_q = shell_single_quote(&python.display().to_string());
    Ok(Some(PreviewCmd {
        exec_body: format!("exec {py_q} -m http.server {port}\n"),
        label: format!("{} -m http.server {port}", python.display()),
        hint_url: Some(format!("http://localhost:{port}/")),
    }))
}

fn pick_free_port() -> Option<u16> {
    STATIC_PORTS.iter().copied().find(|&p| !port_open(p))
}

fn resolve_python() -> Result<PathBuf> {
    if let Ok(p) = resolve_bin("python3") {
        return Ok(p);
    }
    if let Ok(p) = resolve_bin("python") {
        return Ok(p);
    }
    // Last resort: which without our PATH helper already failed in resolve_bin.
    for name in ["python3", "python"] {
        if let Ok(out) = Command::new("/usr/bin/which")
            .arg(name)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let p = PathBuf::from(&s);
                if p.is_file() {
                    return Ok(p);
                }
            }
        }
    }
    bail!(
        "静态站预览需要 python3（`python3 -m http.server`），但系统找不到。\n\
请安装 Python 3，或为项目添加 package.json 的 scripts.dev。"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn npm_dev_preferred() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"dev":"astro dev","build":"astro build"}}"#,
        )
        .unwrap();
        let cmd = detect_preview_cmd(dir.path()).unwrap();
        assert!(cmd.label.contains("dev"), "{}", cmd.label);
        assert!(cmd.hint_url.is_none());
        assert!(cmd.exec_body.contains("run"));
    }

    #[test]
    fn static_index_without_package_json() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("index.html"), "<!doctype html><title>t</title>").unwrap();
        let cmd = detect_preview_cmd(dir.path()).unwrap();
        assert!(
            cmd.label.contains("http.server"),
            "label={}",
            cmd.label
        );
        let url = cmd.hint_url.expect("hint_url");
        assert!(url.starts_with("http://localhost:"), "{url}");
        assert!(cmd.exec_body.contains("http.server"));
    }

    #[test]
    fn package_json_without_scripts_falls_back_to_static() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("package.json"), r#"{"name":"x"}"#).unwrap();
        fs::write(dir.path().join("index.html"), "<!doctype html>").unwrap();
        let cmd = detect_preview_cmd(dir.path()).unwrap();
        assert!(cmd.label.contains("http.server"), "{}", cmd.label);
    }

    #[test]
    fn neither_npm_nor_static_errors() {
        let dir = tempfile::tempdir().unwrap();
        let err = detect_preview_cmd(dir.path()).unwrap_err().to_string();
        assert!(err.contains("无法自动启动预览"), "{err}");
        assert!(err.contains("index.html"), "{err}");
    }
}
