//! HTTP readiness probe for local preview (TCP alone is not enough).
//!
//! [INPUT]: localhost URL (http://127.0.0.1:PORT/…)
//! [OUTPUT]: true only when GET returns 2xx/3xx within timeout
//! [POS]: services/preview · sync · no Mode B
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/services/CLAUDE.md

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// True when a localhost URL accepts HTTP and returns 2xx or 3xx.
pub fn http_ready(url: &str) -> bool {
    let Some(port) = url_port(url) else {
        return false;
    };
    let path = url_path(url);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(350)) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(600)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(400)));
    let req = format!("GET {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut buf = [0u8; 96];
    let n = match stream.read(&mut buf) {
        Ok(n) if n >= 12 => n,
        _ => return false,
    };
    status_ok(&buf[..n])
}

/// TCP accept only (faster first filter). Prefer [`http_ready`] before "已启动".
pub fn port_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(200),
    )
    .is_ok()
}

pub fn url_port(url: &str) -> Option<u16> {
    let after = url.split("://").nth(1)?;
    let hostport = after.split('/').next()?;
    let port = hostport.split(':').nth(1)?;
    port.parse().ok()
}

fn url_path(url: &str) -> String {
    let after = match url.split("://").nth(1) {
        Some(a) => a,
        None => return "/".into(),
    };
    let path = after.find('/').map(|i| &after[i..]).unwrap_or("/");
    if path.is_empty() {
        "/".into()
    } else {
        // Drop query/fragment for simple GET.
        path.split('?').next().unwrap_or("/").to_string()
    }
}

fn status_ok(head: &[u8]) -> bool {
    // "HTTP/1.x Nxx"
    if head.len() < 12 || !head.starts_with(b"HTTP/1.") {
        return false;
    }
    // Status code starts at index 9 for "HTTP/1.0 " / "HTTP/1.1 "
    let code = head.get(9).copied().unwrap_or(0);
    code == b'2' || code == b'3'
}

/// Scan assistant prose for localhost URLs that are **not** serving; append host truth.
/// Prevents the repeated "AI says 200, browser CONNECTION_REFUSED" lie.
pub fn annotate_false_preview_claims(text: &str) -> String {
    let re = match regex::Regex::new(r"https?://(?:localhost|127\.0\.0\.1):(\d+)") {
        Ok(r) => r,
        Err(_) => return text.to_string(),
    };
    let mut dead: Vec<u16> = Vec::new();
    for cap in re.captures_iter(text) {
        let Ok(port) = cap[1].parse::<u16>() else {
            continue;
        };
        if port == 0 {
            continue;
        }
        let url = format!("http://127.0.0.1:{port}/");
        if !http_ready(&url) {
            dead.push(port);
        }
    }
    dead.sort_unstable();
    dead.dedup();
    if dead.is_empty() {
        return text.to_string();
    }
    let ports = dead
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join("、");
    format!(
        "{text}\n\n——\n\
**系统核验**：上面写的本地地址目前**打不开**（端口 {ports} 没有在响应网页）。\n\
聊天会话里起的服务常会随本轮结束而消失，所以浏览器会报「无法访问 / CONNECTION_REFUSED」。\n\
请直接发 **「启动本地预览」**（或点同义短句）——由 cco **独立进程**托管，关聊天也不会带走。"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn url_port_parses() {
        assert_eq!(url_port("http://127.0.0.1:5173/"), Some(5173));
        assert_eq!(url_port("http://localhost:4321"), Some(4321));
    }

    #[test]
    fn status_ok_2xx_3xx() {
        assert!(status_ok(b"HTTP/1.0 200 OK\r\n"));
        assert!(status_ok(b"HTTP/1.1 301 Moved\r\n"));
        assert!(!status_ok(b"HTTP/1.1 404 Not Found\r\n"));
        assert!(!status_ok(b"nope"));
    }

    #[test]
    fn http_ready_against_tiny_server() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let mut buf = [0u8; 256];
                let _ = s.read(&mut buf);
                let _ = s.write_all(b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\nok");
            }
        });
        thread::sleep(Duration::from_millis(50));
        let url = format!("http://127.0.0.1:{port}/");
        assert!(http_ready(&url), "expected ready on {url}");
    }

    #[test]
    fn annotate_appends_when_dead() {
        let text = "已修好，请打开 http://127.0.0.1:59999/";
        let out = annotate_false_preview_claims(text);
        assert!(out.contains("系统核验"), "{out}");
        assert!(out.contains("启动本地预览"), "{out}");
    }

    #[test]
    fn annotate_silent_when_no_url() {
        let text = "计划已写好，请保存。";
        assert_eq!(annotate_false_preview_claims(text), text);
    }
}
