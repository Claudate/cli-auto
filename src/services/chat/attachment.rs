//! Chat file attachments under `.cco/chat/attachments/<session>/` (G4 · images + docs).

use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::Utc;

use crate::domain::chat::DEFAULT_SESSION;

use super::types::ChatAttachment;

/// Max attachment bytes (8 MiB) — docs/pdf need more headroom than thumbnails.
const MAX_ATTACHMENT_BYTES: usize = 8 * 1024 * 1024;
/// Max files per message.
pub(crate) const MAX_ATTACHMENTS_PER_MSG: usize = 6;

/// Blocked extensions (executables / scripts) even if mime lies.
fn blocked_ext(ext: &str) -> bool {
    matches!(
        ext.trim().trim_start_matches('.').to_ascii_lowercase().as_str(),
        "exe"
            | "dll"
            | "so"
            | "dylib"
            | "bat"
            | "cmd"
            | "com"
            | "msi"
            | "scr"
            | "ps1"
            | "vbs"
            | "js"
            | "jse"
            | "wsf"
            | "sh"
            | "bash"
            | "zsh"
            | "fish"
            | "app"
            | "dmg"
            | "pkg"
            | "deb"
            | "rpm"
            | "apk"
            | "jar"
            | "class"
            | "wasm"
    )
}

pub(crate) fn allowed_attachment_mime(mime: &str) -> bool {
    let m = mime.trim().to_ascii_lowercase();
    if m.starts_with("image/") {
        return matches!(
            m.as_str(),
            "image/png" | "image/jpeg" | "image/jpg" | "image/webp" | "image/gif" | "image/svg+xml"
        );
    }
    matches!(
        m.as_str(),
        "text/plain"
            | "text/markdown"
            | "text/x-markdown"
            | "text/csv"
            | "text/tab-separated-values"
            | "text/html"
            | "text/css"
            | "text/xml"
            | "text/rtf"
            | "application/json"
            | "application/ld+json"
            | "application/xml"
            | "application/pdf"
            | "application/rtf"
            | "application/msword"
            | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "application/vnd.ms-excel"
            | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            | "application/vnd.ms-powerpoint"
            | "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            | "application/x-yaml"
            | "text/yaml"
            | "text/x-yaml"
            | "application/yaml"
            | "text/javascript"
            | "application/javascript"
            | "application/typescript"
            | "text/x-python"
            | "text/x-rust"
            | "text/x-go"
            | "text/x-java-source"
            | "text/x-c"
            | "text/x-c++"
            | "text/x-csharp"
            | "application/sql"
            | "text/x-sql"
            | "application/x-sh"
            | "text/x-shellscript"
    )
}

// (legacy allowed_image_mime removed — use allowed_attachment_mime)

fn ext_from_name_or_mime(file_name: &str, mime: &str) -> String {
    if let Some(ext) = Path::new(file_name)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
    {
        if !ext.is_empty() && ext.len() <= 12 {
            return ext;
        }
    }
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/png" => "png".into(),
        "image/jpeg" | "image/jpg" => "jpg".into(),
        "image/webp" => "webp".into(),
        "image/gif" => "gif".into(),
        "image/svg+xml" => "svg".into(),
        "application/pdf" => "pdf".into(),
        "application/json" | "application/ld+json" => "json".into(),
        "text/markdown" | "text/x-markdown" => "md".into(),
        "text/csv" => "csv".into(),
        "text/plain" => "txt".into(),
        "text/html" => "html".into(),
        "text/css" => "css".into(),
        "text/xml" | "application/xml" => "xml".into(),
        "application/msword" => "doc".into(),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx".into(),
        "application/vnd.ms-excel" => "xls".into(),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx".into(),
        "application/vnd.ms-powerpoint" => "ppt".into(),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            "pptx".into()
        }
        "text/yaml" | "text/x-yaml" | "application/x-yaml" | "application/yaml" => "yml".into(),
        _ => "bin".into(),
    }
}

/// Write one attachment under `.cco/chat/attachments/<session>/`.
pub fn chat_save_attachment(
    project: &Path,
    session_id: Option<&str>,
    file_name: &str,
    mime: &str,
    data: &[u8],
) -> Result<ChatAttachment> {
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let ext = ext_from_name_or_mime(file_name, mime);
    if blocked_ext(&ext) {
        bail!("blocked file type: .{ext}");
    }
    if !allowed_attachment_mime(mime) {
        // Empty / generic browser mime for .md/.txt etc. — allow known doc/code extensions.
        let ok_ext = matches!(
            ext.as_str(),
            "png"
                | "jpg"
                | "jpeg"
                | "webp"
                | "gif"
                | "svg"
                | "pdf"
                | "md"
                | "markdown"
                | "txt"
                | "csv"
                | "tsv"
                | "json"
                | "yaml"
                | "yml"
                | "toml"
                | "xml"
                | "html"
                | "htm"
                | "css"
                | "rs"
                | "ts"
                | "tsx"
                | "js"
                | "jsx"
                | "py"
                | "go"
                | "java"
                | "c"
                | "h"
                | "cpp"
                | "hpp"
                | "cs"
                | "sql"
                | "doc"
                | "docx"
                | "xls"
                | "xlsx"
                | "ppt"
                | "pptx"
                | "rtf"
                | "log"
        );
        let generic = {
            let m = mime.trim().to_ascii_lowercase();
            m.is_empty() || m == "application/octet-stream"
        };
        if !(generic && ok_ext) {
            bail!("unsupported file type: {mime} (name={file_name})");
        }
    }
    if data.is_empty() {
        bail!("empty attachment");
    }
    if data.len() > MAX_ATTACHMENT_BYTES {
        bail!(
            "attachment too large (max {} MB)",
            MAX_ATTACHMENT_BYTES / (1024 * 1024)
        );
    }
    let sid = session_id.unwrap_or(DEFAULT_SESSION);
    let safe_sid: String = sid
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stamp = Utc::now().format("%Y%m%d%H%M%S%3f").to_string();
    let base = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let safe_base: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    let safe_base = if safe_base.is_empty() {
        "file".into()
    } else {
        safe_base
    };
    let file = format!("{safe_base}-{stamp}.{ext}");
    let rel = format!(".cco/chat/attachments/{safe_sid}/{file}");
    let abs = project.join(&rel);
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create attachment dir {}", parent.display()))?;
    }
    std::fs::write(&abs, data).with_context(|| format!("write attachment {}", abs.display()))?;
    let display_name = {
        let n = file_name.trim();
        if n.is_empty() {
            file.clone()
        } else {
            Path::new(n)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or(file.clone())
        }
    };
    let mime_out = {
        let m = mime.trim().to_ascii_lowercase();
        if m.is_empty() {
            guess_mime_from_ext(&ext)
        } else if m == "image/jpg" {
            "image/jpeg".into()
        } else {
            m
        }
    };
    Ok(ChatAttachment {
        path: rel,
        mime: mime_out,
        name: display_name,
    })
}

fn guess_mime_from_ext(ext: &str) -> String {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "md" | "markdown" => "text/markdown",
        "json" => "application/json",
        "csv" => "text/csv",
        "txt" | "log" => "text/plain",
        "yml" | "yaml" => "text/yaml",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "xml" => "application/xml",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "doc" => "application/msword",
        "xls" => "application/vnd.ms-excel",
        "ppt" => "application/vnd.ms-powerpoint",
        _ => "application/octet-stream",
    }
    .into()
}

pub(crate) fn format_attachments_block(atts: &[ChatAttachment]) -> String {
    if atts.is_empty() {
        return String::new();
    }
    let mut lines =
        vec!["\n\n--- 附件（项目相对路径，请结合文件理解需求）---".to_string()];
    for (i, a) in atts.iter().enumerate() {
        lines.push(format!("{}. {} ({}) → {}", i + 1, a.name, a.mime, a.path));
    }
    lines.join("\n")
}
