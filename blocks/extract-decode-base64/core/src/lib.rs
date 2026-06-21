//! gizza-ai/extract-decode-base64 core — scan text for embedded Base64 blobs,
//! decode them, and classify each as decoded text or a binary preview
//! (file-type + hex). Pure-Rust (`base64`, `regex`, magic-byte sniffer).

use base64::engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD};
use base64::Engine;
use regex::Regex;
use serde::Serialize;

/// Shortest run we treat as a candidate (≈12 decoded bytes).
const MIN_LEN: usize = 16;
/// How many decoded bytes to show in the hex preview for binary blobs.
const HEX_PREVIEW: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Decoded {
    /// The Base64 substring that was found (trimmed of padding for matching).
    pub source: String,
    /// "text" or "binary".
    pub kind: String,
    /// Number of decoded bytes.
    pub bytes: usize,
    /// Decoded UTF-8 text (present when `kind == "text"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Detected file type, e.g. "image/png" (present for recognized binaries).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_type: Option<String>,
    /// Hex of the first bytes (present when `kind == "binary"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hex_preview: Option<String>,
}

fn printable_ratio(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    let ok = bytes
        .iter()
        .filter(|&&b| b == b'\t' || b == b'\n' || b == b'\r' || (0x20..=0x7e).contains(&b))
        .count();
    ok as f64 / bytes.len() as f64
}

thread_local! {
    // One run over the union of both alphabets; the alphabet is decided per token
    // by which marker chars it contains (avoids fragmenting url-safe tokens).
    static TOKEN: Regex = Regex::new(r"[A-Za-z0-9+/_-]{16,}={0,2}").unwrap();
}

fn try_decode(token: &str) -> Option<Vec<u8>> {
    let trimmed = token.trim_end_matches('=');
    if trimmed.len() < MIN_LEN || trimmed.len() % 4 == 1 {
        return None;
    }
    // A real Base64 token won't mix the two alphabets.
    let has_std = trimmed.contains(['+', '/']);
    let has_url = trimmed.contains(['-', '_']);
    if has_std && has_url {
        return None;
    }
    let eng = if has_url { URL_SAFE_NO_PAD } else { STANDARD_NO_PAD };
    eng.decode(trimmed.as_bytes()).ok()
}

fn classify(source: String, raw: Vec<u8>) -> Option<Decoded> {
    // Accept only blobs that look like real content: printable text, or a
    // recognized file type. This filters out random alphanumeric noise.
    let bytes = raw.len();
    if let Ok(s) = std::str::from_utf8(&raw) {
        if printable_ratio(raw.as_slice()) >= 0.85 {
            return Some(Decoded {
                source,
                kind: "text".into(),
                bytes,
                text: Some(s.to_string()),
                file_type: None,
                hex_preview: None,
            });
        }
    }
    if let Some(det) = gizza_ai_detect_file_type_core::detect(&raw) {
        let hex_preview = hex::encode(&raw[..raw.len().min(HEX_PREVIEW)]);
        return Some(Decoded {
            source,
            kind: "binary".into(),
            bytes,
            text: None,
            file_type: Some(det.mime.to_string()),
            hex_preview: Some(hex_preview),
        });
    }
    None
}

/// Scan `text` for embedded Base64 blobs and decode them.
pub fn extract(text: &str) -> Vec<Decoded> {
    let mut out: Vec<Decoded> = Vec::new();
    TOKEN.with(|re| {
        for m in re.find_iter(text) {
            if let Some(raw) = try_decode(m.as_str()) {
                if let Some(d) = classify(m.as_str().trim_end_matches('=').to_string(), raw) {
                    out.push(d);
                }
            }
        }
    });
    out
}

/// Human-readable rendering (used by the page).
pub fn render(text: &str) -> Result<String, String> {
    let found = extract(text);
    if found.is_empty() {
        return Ok("No decodable Base64 blobs found.".to_string());
    }
    let mut out = format!("{} Base64 blob(s) found:\n", found.len());
    for (i, d) in found.iter().enumerate() {
        out.push_str(&format!("\n[{}] {} bytes — {}\n", i + 1, d.bytes, d.kind));
        match d.kind.as_str() {
            "text" => {
                out.push_str("decoded text:\n");
                out.push_str(d.text.as_deref().unwrap_or(""));
                out.push('\n');
            }
            _ => {
                if let Some(ft) = &d.file_type {
                    out.push_str(&format!("file type: {ft}\n"));
                }
                if let Some(h) = &d.hex_preview {
                    out.push_str(&format!("hex: {h}\n"));
                }
            }
        }
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_text_blob() {
        // "Hello, world! This is base64." encoded.
        let b64 = base64::engine::general_purpose::STANDARD.encode("Hello, world! This is base64.");
        let text = format!("here is a token {b64} in some prose");
        let f = extract(&text);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, "text");
        assert_eq!(f[0].text.as_deref(), Some("Hello, world! This is base64."));
    }

    #[test]
    fn finds_binary_blob_with_filetype() {
        // PNG signature + IHDR-ish bytes.
        let png: &[u8] = &[
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
            0x52, 0x00, 0x00, 0x00, 0x01,
        ];
        let b64 = base64::engine::general_purpose::STANDARD.encode(png);
        let f = extract(&format!("img: {b64}"));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, "binary");
        assert_eq!(f[0].file_type.as_deref(), Some("image/png"));
        assert!(f[0].hex_preview.as_deref().unwrap().starts_with("89504e47"));
    }

    #[test]
    fn empty_when_none() {
        assert!(extract("short abc def").is_empty());
        assert!(render("nothing").unwrap().contains("No decodable"));
    }

    #[test]
    fn url_safe_variant() {
        // Bytes chosen so standard base64 yields '+' and '/', forcing the
        // url-safe alphabet to differ (produces '-'/'_').
        let raw: &[u8] = b"\xfb\xff\xbf this is plenty long to be a token";
        let b64 = URL_SAFE_NO_PAD.encode(raw);
        assert!(b64.contains(['-', '_']));
        let f = extract(&format!("t {b64} x"));
        assert_eq!(f.len(), 1);
        // decodes back to the same bytes (text mode since mostly printable? leading
        // bytes are non-printable → binary). Just assert byte count round-trips.
        assert_eq!(f[0].bytes, raw.len());
    }
}
