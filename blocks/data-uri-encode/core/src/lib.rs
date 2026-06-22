//! gizza-ai/data-uri-encode core — build a `data:` URI (RFC 2397) from text,
//! with a chosen MIME type and either Base64 or percent (URL) encoding. Pure
//! Rust (`base64`), shared by the chat block and the standalone page.
//!
//! A data URI is: `data:[<mediatype>][;base64],<data>`. For the percent form
//! the payload is percent-encoded text; for the base64 form it is the standard
//! Base64 of the UTF-8 bytes, tagged with the `;base64` token.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;

/// Default media type when the caller leaves `mime` blank (RFC 2397 default).
pub const DEFAULT_MIME: &str = "text/plain";

/// The structured result of building a `data:` URI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DataUri {
    /// The full `data:` URI string.
    pub data_uri: String,
    /// The media type embedded in the URI.
    pub mime: String,
    /// Encoding used: `"base64"` or `"url"` (percent-encoded).
    pub encoding: String,
    /// Number of UTF-8 bytes of the input payload.
    pub bytes: usize,
    /// Total length of the produced `data:` URI in characters.
    pub uri_length: usize,
}

/// Percent-encode `bytes` per RFC 3986 for use as a data-URI payload: keep the
/// unreserved set `A-Z a-z 0-9 - _ . ~`, percent-escape everything else. This is
/// the conservative form that round-trips through `data-uri-decode`.
fn percent_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        let unreserved = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~');
        if unreserved {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0x0f) as usize] as char);
        }
    }
    out
}

/// Validate that `mime` looks like a `type/subtype[;param=value...]` media type.
/// Empty (after trimming) is allowed and means "use the default". We do not try
/// to validate every RFC token, just guard against obviously broken input (a
/// leading `data:`, embedded commas/whitespace) that would corrupt the URI.
fn normalize_mime(mime: &str) -> Result<String, String> {
    let m = mime.trim();
    if m.is_empty() {
        return Ok(DEFAULT_MIME.to_string());
    }
    if m.starts_with("data:") {
        return Err("mime must be a media type like 'image/svg+xml', not a full data: URI".into());
    }
    // The base (type/subtype) part is everything before the first ';'.
    let base = m.split(';').next().unwrap_or("").trim();
    if base.contains(char::is_whitespace) || m.contains(',') {
        return Err("mime must not contain whitespace or commas".into());
    }
    // A media type must have a single '/' separating non-empty type and subtype.
    let mut parts = base.splitn(2, '/');
    let ty = parts.next().unwrap_or("");
    let sub = parts.next().unwrap_or("");
    if ty.is_empty() || sub.is_empty() || sub.contains('/') {
        return Err(format!(
            "mime '{m}' is not a valid 'type/subtype' media type (e.g. 'text/plain' or 'image/png')"
        ));
    }
    Ok(m.to_string())
}

/// Build a `data:` URI from `text` with the given `mime` and `encoding`.
///
/// - `mime`: media type to embed; blank → `text/plain`.
/// - `encoding`: `"base64"` (default) or `"url"` (percent-encoded).
pub fn encode(text: &str, mime: &str, encoding: &str) -> Result<DataUri, String> {
    let mime = normalize_mime(mime)?;
    let enc = encoding.trim().to_ascii_lowercase();
    let enc = if enc.is_empty() { "base64" } else { enc.as_str() };
    let bytes = text.as_bytes();

    let (encoding, data_uri) = match enc {
        "base64" => {
            let b64 = STANDARD.encode(bytes);
            ("base64".to_string(), format!("data:{mime};base64,{b64}"))
        }
        "url" | "percent" | "uri" => {
            let pct = percent_encode(bytes);
            ("url".to_string(), format!("data:{mime},{pct}"))
        }
        other => {
            return Err(format!(
                "unknown encoding '{other}': use 'base64' or 'url'"
            ))
        }
    };

    Ok(DataUri {
        uri_length: data_uri.chars().count(),
        bytes: bytes.len(),
        mime,
        encoding,
        data_uri,
    })
}

/// Convenience wrapper returning the structured result as a JSON string.
pub fn run(text: &str, mime: &str, encoding: &str) -> Result<String, String> {
    let r = encode(text, mime, encoding)?;
    serde_json::to_string(&r).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_default_mime() {
        let r = encode("Hello, World!", "", "base64").unwrap();
        assert_eq!(r.data_uri, "data:text/plain;base64,SGVsbG8sIFdvcmxkIQ==");
        assert_eq!(r.mime, "text/plain");
        assert_eq!(r.encoding, "base64");
        assert_eq!(r.bytes, 13);
        assert_eq!(r.uri_length, r.data_uri.chars().count());
    }

    #[test]
    fn base64_with_mime() {
        let svg = "<svg/>";
        let r = encode(svg, "image/svg+xml", "base64").unwrap();
        assert_eq!(r.data_uri, "data:image/svg+xml;base64,PHN2Zy8+");
        assert_eq!(r.mime, "image/svg+xml");
    }

    #[test]
    fn url_encoding_escapes_reserved() {
        let r = encode("a b/c?", "text/plain", "url").unwrap();
        assert_eq!(r.data_uri, "data:text/plain,a%20b%2Fc%3F");
        assert_eq!(r.encoding, "url");
    }

    #[test]
    fn url_encoding_keeps_unreserved() {
        let r = encode("aA0-_.~", "text/plain", "url").unwrap();
        assert_eq!(r.data_uri, "data:text/plain,aA0-_.~");
    }

    #[test]
    fn encoding_blank_defaults_base64() {
        let r = encode("hi", "", "").unwrap();
        assert_eq!(r.encoding, "base64");
        assert_eq!(r.data_uri, "data:text/plain;base64,aGk=");
    }

    #[test]
    fn mime_with_charset_param_preserved() {
        let r = encode("x", "text/html;charset=utf-8", "base64").unwrap();
        assert_eq!(r.mime, "text/html;charset=utf-8");
        assert!(r.data_uri.starts_with("data:text/html;charset=utf-8;base64,"));
    }

    #[test]
    fn rejects_bad_mime() {
        assert!(encode("x", "notamime", "base64").is_err());
        assert!(encode("x", "image/", "base64").is_err());
        assert!(encode("x", "data:text/plain", "base64").is_err());
        assert!(encode("x", "image png", "base64").is_err());
    }

    #[test]
    fn rejects_unknown_encoding() {
        assert!(encode("x", "text/plain", "rot13").is_err());
    }

    #[test]
    fn run_returns_json() {
        let j = run("hi", "text/plain", "base64").unwrap();
        assert!(j.contains("\"data_uri\":\"data:text/plain;base64,aGk=\""));
        assert!(j.contains("\"encoding\":\"base64\""));
    }
}
