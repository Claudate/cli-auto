//! Local project preview (dev server) — detached from chat Claude process.
//!
//! [INPUT]: project root · optional script name
//! [OUTPUT]: PreviewStatus (running · url · pid · error)
//! [POS]: services IO · app/chat 薄委托；**不**经 Mode B confirm
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/services/CLAUDE.md
//! note: 进程 setsid/新进程组，chat 结束不带走；先探测端口再报「已启动」
//! note: 启动探测顺序 npm scripts → 静态 index.html（python -m http.server）

mod detect;

use std::fs::{self, File};
use std::io::Read;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use detect::detect_preview_cmd;

const STATE_NAME: &str = "state.json";
const LOG_NAME: &str = "dev.log";
const PID_NAME: &str = "dev.pid";

/// Default ports to probe when log has no URL yet (Astro / Vite / Next / static).
const DEFAULT_PORTS: &[u16] = &[4321, 5173, 3000, 4173, 8080, 8000, 5000, 4322];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewStatus {
    pub running: bool,
    pub url: Option<String>,
    pub pid: Option<u32>,
    pub command: Option<String>,
    pub log_path: Option<String>,
    pub error: Option<String>,
    /// Human one-liner for chat bubble.
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PreviewState {
    pid: u32,
    command: String,
    url: Option<String>,
    log_rel: String,
    started_at: String,
}

fn preview_dir(project: &Path) -> PathBuf {
    project.join(".cco").join("preview")
}

fn state_path(project: &Path) -> PathBuf {
    preview_dir(project).join(STATE_NAME)
}

fn log_path(project: &Path) -> PathBuf {
    preview_dir(project).join(LOG_NAME)
}

fn pid_path(project: &Path) -> PathBuf {
    preview_dir(project).join(PID_NAME)
}

fn load_state(project: &Path) -> Option<PreviewState> {
    let p = state_path(project);
    let raw = fs::read_to_string(p).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_state(project: &Path, st: &PreviewState) -> Result<()> {
    let dir = preview_dir(project);
    fs::create_dir_all(&dir)?;
    let p = state_path(project);
    fs::write(p, serde_json::to_string_pretty(st)?)?;
    fs::write(pid_path(project), format!("{}\n", st.pid))?;
    Ok(())
}

fn clear_state(project: &Path) {
    let _ = fs::remove_file(state_path(project));
    let _ = fs::remove_file(pid_path(project));
}

/// Existence check without libc crate (`kill -0`).
fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn kill_pid_tree(pid: u32) {
    // Prefer process-group signal (leader from process_group(0)).
    let _ = Command::new("kill")
        .args(["-TERM", &format!("-{pid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    thread::sleep(Duration::from_millis(400));
    if pid_alive(pid) {
        let _ = Command::new("kill")
            .args(["-KILL", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

pub(crate) fn port_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(200),
    )
    .is_ok()
}

fn parse_url_from_log(log: &Path) -> Option<String> {
    let mut f = File::open(log).ok()?;
    let mut buf = String::new();
    let _ = f.read_to_string(&mut buf);
    // Astro: Local    http://localhost:4321/
    // Vite:  Local:   http://localhost:5173/
    let re = regex::Regex::new(r"https?://(?:localhost|127\.0\.0\.1):\d+").ok()?;
    let m = re.find(&buf)?;
    let mut u = m.as_str().to_string();
    if !u.ends_with('/') {
        u.push('/');
    }
    Some(u)
}

fn probe_url(log: &Path) -> Option<String> {
    if let Some(u) = parse_url_from_log(log) {
        if let Some(port) = url_port(&u) {
            if port_open(port) {
                return Some(u);
            }
        } else {
            return Some(u);
        }
    }
    for &p in DEFAULT_PORTS {
        if port_open(p) {
            return Some(format!("http://localhost:{p}/"));
        }
    }
    None
}

fn url_port(url: &str) -> Option<u16> {
    let after = url.split("://").nth(1)?;
    let hostport = after.split('/').next()?;
    let port = hostport.split(':').nth(1)?;
    port.parse().ok()
}

/// Resolve absolute path to `npm` / `node` under GUI-safe PATH.
pub(crate) fn resolve_bin(name: &str) -> Result<PathBuf> {
    let path = crate::runtime::provider::worker_path_env();
    for dir in path.split(':') {
        if dir.is_empty() {
            continue;
        }
        let p = Path::new(dir).join(name);
        if p.is_file() {
            return Ok(p);
        }
    }
    if let Ok(out) = Command::new("/usr/bin/which")
        .arg(name)
        .env("PATH", &path)
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                let p = PathBuf::from(&s);
                if p.is_file() {
                    return Ok(p);
                }
            }
        }
    }
    bail!("找不到可执行文件 `{name}`（PATH 已含 Homebrew）。请确认已安装 Node/npm。");
}

pub(crate) fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// True only when TCP accepts (avoids false “已启动”).
fn url_listening(url: &str) -> bool {
    url_port(url).map(port_open).unwrap_or(false)
}

/// Prefer known hint (static server) if that port is up; else log / port scan.
fn resolve_ready_url(log: &Path, hint: &Option<String>) -> Option<String> {
    if let Some(u) = hint {
        if url_listening(u) {
            return Some(u.clone());
        }
    }
    probe_url(log).filter(|u| url_listening(u))
}

/// Start (or reuse) **daemonized** preview via `nohup`; only report after port listens.
pub fn preview_start(project: &Path) -> Result<PreviewStatus> {
    if !project.is_dir() {
        bail!("项目路径不存在: {}", project.display());
    }

    // Reuse only if pid alive AND port still listens.
    if let Some(st) = load_state(project) {
        let log = project.join(&st.log_rel);
        let url = st
            .url
            .clone()
            .or_else(|| probe_url(&log))
            .filter(|u| url_listening(u));
        if pid_alive(st.pid) {
            if let Some(url) = url {
                let mut st2 = st.clone();
                st2.url = Some(url.clone());
                let _ = save_state(project, &st2);
                return Ok(PreviewStatus {
                    running: true,
                    url: Some(url.clone()),
                    pid: Some(st.pid),
                    command: Some(st.command),
                    log_path: Some(log.display().to_string()),
                    error: None,
                    message: format_running_msg(&url, true),
                });
            }
            // Dead listener — stop only our recorded pid tree, then restart.
            kill_pid_tree(st.pid);
            clear_state(project);
        } else {
            clear_state(project);
        }
    }

    let dir = preview_dir(project);
    fs::create_dir_all(&dir)?;
    let log = log_path(project);
    let _ = fs::write(&log, "");

    let cmd = detect_preview_cmd(project)?;
    let label = cmd.label.clone();
    let hint_url = cmd.hint_url.clone();
    let path_env = crate::runtime::provider::worker_path_env();

    // Launcher script: survives parent exit (chat / Tauri / cargo test).
    let launcher = dir.join("run.sh");
    let body = format!(
        "#!/bin/sh\nexport PATH={path}\ncd {cwd} || exit 1\n{exec}",
        path = shell_single_quote(&path_env),
        cwd = shell_single_quote(&project.display().to_string()),
        exec = cmd.exec_body,
    );
    fs::write(&launcher, body).context("write preview run.sh")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&launcher)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&launcher, perms)?;
    }

    // nohup + background; capture $! as daemon pid (not the shell that exits).
    let spawn_line = format!(
        "nohup {launcher} >>{log} 2>&1 & echo $!",
        launcher = shell_single_quote(&launcher.display().to_string()),
        log = shell_single_quote(&log.display().to_string()),
    );
    let out = Command::new("/bin/sh")
        .arg("-c")
        .arg(&spawn_line)
        .env("PATH", &path_env)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("启动失败：{label}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("启动失败：{label}\n{err}");
    }
    let pid_str = String::from_utf8_lossy(&out.stdout);
    let pid: u32 = pid_str
        .trim()
        .lines()
        .rev()
        .find_map(|l| l.trim().parse().ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "启动失败：未能取得后台 pid（stdout={}）",
                pid_str.trim()
            )
        })?;

    let st = PreviewState {
        pid,
        command: label.clone(),
        url: None,
        log_rel: format!(".cco/preview/{LOG_NAME}"),
        started_at: chrono::Utc::now().to_rfc3339(),
    };
    save_state(project, &st)?;

    // Wait until a local port accepts connections (not just log line).
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if let Some(u) = resolve_ready_url(&log, &hint_url) {
            let mut st2 = st.clone();
            st2.url = Some(u.clone());
            let _ = save_state(project, &st2);
            return Ok(PreviewStatus {
                running: true,
                url: Some(u.clone()),
                pid: Some(pid),
                command: Some(label),
                log_path: Some(log.display().to_string()),
                error: None,
                message: format_running_msg(&u, false),
            });
        }
        // Process vanished with no listening port
        if !pid_alive(pid) {
            // Child may have re-parented; still accept if port is up
            if let Some(u) = resolve_ready_url(&log, &hint_url) {
                let mut st2 = st.clone();
                st2.url = Some(u.clone());
                let _ = save_state(project, &st2);
                return Ok(PreviewStatus {
                    running: true,
                    url: Some(u.clone()),
                    pid: Some(pid),
                    command: Some(label),
                    log_path: Some(log.display().to_string()),
                    error: None,
                    message: format_running_msg(&u, false),
                });
            }
            let tail = fs::read_to_string(&log).unwrap_or_default();
            let snip: String = tail
                .chars()
                .rev()
                .take(800)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            clear_state(project);
            return Ok(PreviewStatus {
                running: false,
                url: None,
                pid: None,
                command: Some(label),
                log_path: Some(log.display().to_string()),
                error: Some("进程已退出".into()),
                message: format!(
                    "没能打开网站（程序很快退出了），请先别点旧链接。\n\
可再发「启动本地预览」试一次。\n\
详情：{}",
                    snip.trim()
                ),
            });
        }
        thread::sleep(Duration::from_millis(400));
    }

    Ok(PreviewStatus {
        running: pid_alive(pid),
        url: resolve_ready_url(&log, &hint_url),
        pid: Some(pid),
        command: Some(label),
        log_path: Some(log.display().to_string()),
        error: Some("超时未检测到端口".into()),
        message: format!(
            "还在准备，网页暂时打不开，请先别点链接。\n\
稍等再发一次「启动本地预览」。\n\
（日志：{}）",
            log.display()
        ),
    })
}

fn format_running_msg(url: &str, reused: bool) -> String {
    let head = if reused {
        "网站已经在跑，不用重新开。"
    } else {
        "网站已打开，可以预览了。"
    };
    format!(
        "{head}\n\
\n\
点这里看效果：{url}\n\
\n\
· 只给**当前这个项目**用，不影响电脑上别的网站。\n\
· 继续聊天也没事，网页不会自己关掉。\n\
· 不想看了，发一句「关闭服务」即可。"
    )
}

/// Stop preview process started by [`preview_start`].
pub fn preview_stop(project: &Path) -> Result<PreviewStatus> {
    let Some(st) = load_state(project) else {
        return Ok(PreviewStatus {
            running: false,
            url: None,
            pid: None,
            command: None,
            log_path: Some(log_path(project).display().to_string()),
            error: None,
            message: "现在没有在跑的预览。要看网站的话，发「启动本地预览」。".into(),
        });
    };

    // Only signal the recorded process group / pid — never sweep arbitrary ports.
    kill_pid_tree(st.pid);
    clear_state(project);
    let still = pid_alive(st.pid);
    Ok(PreviewStatus {
        running: still,
        url: None,
        pid: if still { Some(st.pid) } else { None },
        command: Some(st.command),
        log_path: Some(log_path(project).display().to_string()),
        error: if still {
            Some("进程仍在".into())
        } else {
            None
        },
        message: if still {
            "正在关闭，可能还要几秒。再发一次「关闭服务」即可。".into()
        } else {
            "已关掉这个项目的预览。浏览器里再刷新会打不开，是正常的。\n\
（只关当前项目，不影响电脑上别的网站。）"
                .into()
        },
    })
}

/// Status of cco-managed preview (and opportunistic port probe).
pub fn preview_status(project: &Path) -> Result<PreviewStatus> {
    if let Some(st) = load_state(project) {
        let alive = pid_alive(st.pid);
        let log = project.join(&st.log_rel);
        if alive {
            let url = probe_url(&log).or(st.url.clone());
            return Ok(PreviewStatus {
                running: true,
                url: url.clone(),
                pid: Some(st.pid),
                command: Some(st.command),
                log_path: Some(log.display().to_string()),
                error: None,
                message: match url {
                    Some(u) => format_running_msg(&u, true),
                    None => "网站程序在跑，但地址还没准备好。稍后再发「启动本地预览」，或发「关闭服务」停掉。"
                        .into(),
                },
            });
        }
        clear_state(project);
    }

    // Opportunistic: something listening on default ports (not cco-managed)
    for &p in DEFAULT_PORTS {
        if port_open(p) {
            let u = format!("http://localhost:{p}/");
            return Ok(PreviewStatus {
                running: true,
                url: Some(u.clone()),
                pid: None,
                command: None,
                log_path: None,
                error: None,
                message: format!(
                    "电脑上已有网页在跑（不是这次点按钮开的）。\n点这里看看：{u}\n\
若关不掉，可能要你在终端里自己停；发「关闭服务」不一定管得到它。"
                ),
            });
        }
    }

    Ok(PreviewStatus {
        running: false,
        url: None,
        pid: None,
        command: None,
        log_path: Some(log_path(project).display().to_string()),
        error: None,
        message: "现在没有预览。发「启动本地预览」或「你来跑」就能打开网站。".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_astro_local_line() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("dev.log");
        fs::write(
            &log,
            " astro  v5 ready\n┃ Local    http://localhost:4321/\n┃ Network  use --host\n",
        )
        .unwrap();
        let u = parse_url_from_log(&log).expect("url");
        assert!(u.contains("4321"), "{u}");
    }

    /// Live smoke: `CCO_PREVIEW_SMOKE=/path/to/project cargo test -p cco preview_live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn preview_live_start_stop() {
        let root =
            std::env::var("CCO_PREVIEW_SMOKE").expect("set CCO_PREVIEW_SMOKE to project path");
        let project = PathBuf::from(root);
        let st = preview_start(&project).expect("start");
        assert!(
            st.running && st.url.is_some() && st.error.is_none(),
            "start: {st:?}"
        );
        let url = st.url.unwrap();
        let port = url_port(&url).expect("port");
        assert!(port_open(port), "port {port} closed after claim ready");
        let stop = preview_stop(&project).expect("stop");
        assert!(!stop.running, "stop: {stop:?}");
        thread::sleep(Duration::from_millis(500));
        assert!(!port_open(port), "port {port} still open after stop");
    }
}
