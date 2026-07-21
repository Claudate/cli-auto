//! Chat image attachments under `.cco/chat/attachments/<session>/` (G4).

use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::Utc;

use crate::domain::chat::DEFAULT_SESSION;

use super::types::ChatAttachment;

/// G4: max attachment bytes (5 MiB).
const MAX_ATTACHMENT_BYTES: usize = 5 * 1024 * 1024;
/// G4: max images per message.
pub(crate) const MAX_ATTACHMENTS_PER_MSG: usize = 4;

pub(crate) fn allowed_image_mime(mime: &str) -> bool {
    matches!(
        mime.trim().to_ascii_lowercase().as_str(),
        "image/png" | "image/jpeg" | "image/jpg" | "image/webp" | "image/gif"
    )
}

fn ext_for_mime(mime: &str) -> &'static str {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "bin",
    }
}

/// G4: write one image under `.cco/chat/attachments/<session>/`.
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
    if !allowed_image_mime(mime) {
        bail!("unsupported image type: {mime} (use png/jpeg/webp/gif)");
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
    let ext = ext_for_mime(mime);
    let stamp = Utc::now().format("%Y%m%d%H%M%S%3f").to_string();
    let base = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
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
        "image".into()
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
    Ok(ChatAttachment {
        path: rel,
        mime: mime.trim().to_ascii_lowercase(),
        name: display_name,
    })
}

pub(crate) fn format_attachments_block(atts: &[ChatAttachment]) -> String {
    if atts.is_empty() {
        return String::new();
    }
    let mut lines = vec!["\n\n--- 附图（项目相对路径，请结合图片理解需求）---".to_string()];
    for (i, a) in atts.iter().enumerate() {
        lines.push(format!("{}. {} ({}) → {}", i + 1, a.name, a.mime, a.path));
    }
    lines.join("\n")
}
