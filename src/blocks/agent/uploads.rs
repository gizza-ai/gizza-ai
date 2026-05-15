//! Decode and stage user-uploaded attachments + the synthetic history prefix
//! that lets a plain-chat turn see the uploaded refs in context.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::default_filename_for_mime;
use wafer_block::Attachment;

use super::{AgentError, UploadEntry};

/// Decode an [`UploadEntry`] vec into staged-upload tuples
/// `(id, Attachment, display_name)`. v1 caps each upload at 10 MiB; only
/// `image/*` and `video/*` MIMEs are accepted; ids must start with
/// `"upload_"`.
pub(super) fn decode_uploads(
    entries: &[UploadEntry],
) -> Result<Vec<(String, Attachment, String)>, AgentError> {
    const MAX_UPLOAD_BYTES: usize = 10 * 1024 * 1024;
    let mut staged = Vec::with_capacity(entries.len());
    for u in entries {
        if !u.id.starts_with("upload_") {
            return Err(AgentError::UploadIdInvalid(u.id.clone()));
        }
        if !(u.mime.starts_with("image/") || u.mime.starts_with("video/")) {
            return Err(AgentError::UploadUnsupportedMime {
                id: u.id.clone(),
                mime: u.mime.clone(),
            });
        }
        let bytes = B64
            .decode(&u.bytes_base64)
            .map_err(|source| AgentError::UploadBase64 {
                id: u.id.clone(),
                source,
            })?;
        if bytes.len() > MAX_UPLOAD_BYTES {
            return Err(AgentError::UploadTooLarge {
                id: u.id.clone(),
                bytes: bytes.len(),
            });
        }
        let display_name = u
            .filename
            .clone()
            .unwrap_or_else(|| default_filename_for_mime(&u.mime).to_string());
        let att = Attachment {
            mime: u.mime.clone(),
            bytes,
            filename: u.filename.clone(),
        };
        staged.push((u.id.clone(), att, display_name));
    }
    Ok(staged)
}

/// Build the synthetic assistant + tool history-prefix entries the agent
/// injects ahead of the user message when the request carries uploads.
/// Lets the LLM see the upload refs in context for plain-chat turns.
pub(super) fn build_upload_history_prefix(
    staged: &[(String, Attachment, String)],
) -> Vec<serde_json::Value> {
    let mut out = Vec::with_capacity(staged.len() * 2);
    for (id, att, display) in staged {
        out.push(serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "id": id,
                "type": "function",
                "function": { "name": "user_upload", "arguments": "{}" }
            }]
        }));
        let content = format!(
            "ref {id} saved ({mime}, {size} bytes, {display:?}). \
             Pass {{\"ref\": \"{id}\"}} to a slash command.",
            id = id,
            mime = att.mime,
            size = att.bytes.len(),
            display = display,
        );
        out.push(serde_json::json!({
            "role": "tool",
            "tool_call_id": id,
            "content": content,
        }));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_uploads_seeds_attachments_with_upload_ids() {
        let raw = b"\x89PNG\r\n\x1a\n";
        let b64 = B64.encode(raw);
        let entries = vec![UploadEntry {
            id: "upload_1".into(),
            mime: "image/png".into(),
            filename: Some("cat.png".into()),
            bytes_base64: b64,
        }];
        let staged = decode_uploads(&entries).expect("decoded");
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].0, "upload_1");
        assert_eq!(staged[0].1.mime, "image/png");
        assert_eq!(staged[0].1.bytes, raw);
        assert_eq!(staged[0].1.filename.as_deref(), Some("cat.png"));
        assert_eq!(staged[0].2, "cat.png");
    }

    #[test]
    fn decode_uploads_rejects_non_upload_id() {
        let entries = vec![UploadEntry {
            id: "call_1".into(),
            mime: "image/png".into(),
            filename: None,
            bytes_base64: B64.encode(b"x"),
        }];
        let r = decode_uploads(&entries);
        assert!(r.is_err());
    }

    #[test]
    fn decode_uploads_rejects_unsupported_mime() {
        let entries = vec![UploadEntry {
            id: "upload_1".into(),
            mime: "application/pdf".into(),
            filename: None,
            bytes_base64: B64.encode(b"x"),
        }];
        let r = decode_uploads(&entries);
        assert!(r.is_err());
    }

    #[test]
    fn decode_uploads_rejects_oversize_decoded_bytes() {
        let big = vec![0u8; 10 * 1024 * 1024 + 1];
        let entries = vec![UploadEntry {
            id: "upload_1".into(),
            mime: "image/png".into(),
            filename: None,
            bytes_base64: B64.encode(&big),
        }];
        let r = decode_uploads(&entries);
        assert!(r.is_err());
    }

    #[test]
    fn build_upload_history_prefix_emits_assistant_and_tool_pair_per_upload() {
        let staged = vec![(
            "upload_1".to_string(),
            Attachment {
                mime: "image/png".into(),
                bytes: vec![0u8; 1228800],
                filename: Some("cat.png".into()),
            },
            "cat.png".to_string(),
        )];
        let prefix = build_upload_history_prefix(&staged);
        assert_eq!(prefix.len(), 2);
        assert_eq!(prefix[0]["role"], "assistant");
        assert_eq!(prefix[0]["tool_calls"][0]["id"], "upload_1");
        assert_eq!(prefix[1]["role"], "tool");
        assert_eq!(prefix[1]["tool_call_id"], "upload_1");
        let content = prefix[1]["content"].as_str().unwrap();
        assert!(content.contains("upload_1"));
        assert!(content.contains("image/png"));
        assert!(content.contains("slash command"));
    }
}
