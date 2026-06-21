//! gizza-ai/data-uri-decode core — parse a `data:` URI (RFC 2397) into its MIME
//! type, parameters (e.g. charset), encoding, and decoded payload. Pure-Rust
//! (`base64`, `hex`, magic-byte sniffer). Shared by the chat block and the page.
//!
//! A data URI is: `data:[<mediatype>][;base64],<data>` where `<mediatype>` is
//! `type/subtype` plus zero or more `;param=value` pairs. When the media type is
//! omitted the default is `text/plain;charset=US-ASCII`. The payload is either
//! Base64 (when the `;base64` token is present) or percent-encoded text.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;

/// How many decoded bytes to show in the hex preview for binary payloads.
const HEX_PREVIEW: usize = 64;
/// Cap the decoded text preview so a huge payload doesn't blow up the response.
const TEXT_PREVIEW: usize = 100_000;

/// The structured result of decoding a `data:` URI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DataUri {
    /// The full media type, e.g. "image/png" or "text/plain". Defaults to
    /// "text/plain" when the URI omits the media type.
    pub mime: String,
    /// Parameters parsed from the media type, e.g. `[("charset", "utf-8")]`.
    pub params: Vec<(String, String)>,
    /// Charset parameter value if present (convenience copy of params["charset"]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charset: Option<String>,
    /// Payload encoding in the URI: "base64" or "url" (percent-encoded).
    pub encoding: String,
    /// Number of decoded payload bytes.
    pub bytes: usize,
    /// "text" when the payload decodes to printable UTF-8, else "binary".
    pub kind: String,
    /// Decoded UTF-8 text (present when `kind == "text"`, truncated at 100k chars).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Detected file type from magic bytes, e.g. "image/png" (binary payloads).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_type: Option<String>,
    /// Hex of the first bytes (present when `kind == "binary"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hex_preview: Option<String>,
    /// True when the decoded payload was truncated in `text`/`hex_preview`.
    pub truncated: bool,
}

/// Ratio of "printable" characters in valid UTF-8 text: any non-control
/// character (including non-ASCII like CJK / emoji) counts, plus tab/newline/CR.
/// Used to decide text-vs-binary; multibyte chars are counted once, not per byte.
fn printable_ratio(s: &str) -> f64 {
    let mut total = 0usize;
    let mut ok = 0usize;
    for c in s.chars() {
        total += 1;
        if c == '\t' || c == '\n' || c == '\r' || !c.is_control() {
            ok += 1;
        }
    }
    if total == 0 {
        return 1.0;
    }
    ok as f64 / total as f64
}

/// Percent-decode the non-base64 payload of a data URI. `+` is left as-is
/// (data URIs are not form-encoded; only `%XX` sequences are escapes).
fn percent_decode(s: &str) -> Result<Vec<u8>, String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                if i + 2 >= bytes.len() {
                    return Err("truncated percent-escape in data".into());
                }
                let hi = (bytes[i + 1] as char)
                    .to_digit(16)
                    .ok_or("invalid percent-escape in data")?;
                let lo = (bytes[i + 2] as char)
                    .to_digit(16)
                    .ok_or("invalid percent-escape in data")?;
                out.push((hi * 16 + lo) as u8);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    Ok(out)
}

/// Parse + decode a `data:` URI. Returns an error for any string that is not a
/// well-formed data URI or whose payload can't be decoded.
pub fn decode(input: &str) -> Result<DataUri, String> {
    let input = input.trim();
    let rest = input
        .strip_prefix("data:")
        .or_else(|| input.strip_prefix("DATA:"))
        .ok_or("not a data URI: must start with 'data:'")?;

    // Split metadata from the payload at the FIRST comma. Per RFC 2397 the comma
    // separates the header from the data; any later commas are part of the data.
    let (meta, data) = rest
        .split_once(',')
        .ok_or("malformed data URI: missing ',' separating the header from the data")?;

    // The header is a ';'-separated list. The first token (if it contains '/')
    // is the media type; the rest are params, with the optional trailing
    // ';base64' marker.
    let mut tokens: Vec<&str> = meta.split(';').collect();

    let is_base64 = tokens
        .last()
        .map(|t| t.eq_ignore_ascii_case("base64"))
        .unwrap_or(false);
    if is_base64 {
        tokens.pop();
    }

    // Media type: first token if it looks like type/subtype, else default.
    let mut mime = String::from("text/plain");
    let mut default_charset = true;
    if let Some(first) = tokens.first() {
        if first.contains('/') {
            mime = first.trim().to_ascii_lowercase();
            tokens.remove(0);
        }
    }

    let mut params: Vec<(String, String)> = Vec::new();
    let mut charset: Option<String> = None;
    for tok in &tokens {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let (k, v) = tok.split_once('=').unwrap_or((tok, ""));
        let k = k.trim().to_ascii_lowercase();
        let v = v.trim().to_string();
        if k == "charset" {
            charset = Some(v.clone());
            default_charset = false;
        }
        params.push((k, v));
    }
    // RFC 2397: a bare `data:,` (no media type) defaults to text/plain;charset=US-ASCII.
    if default_charset && mime == "text/plain" && meta.split(';').next().map(|t| !t.contains('/')).unwrap_or(true) {
        charset = Some("US-ASCII".to_string());
    }

    let (raw, encoding) = if is_base64 {
        // data URIs may legitimately contain whitespace/newlines in the base64.
        let cleaned: String = data.chars().filter(|c| !c.is_whitespace()).collect();
        let decoded = STANDARD
            .decode(cleaned.as_bytes())
            .map_err(|e| format!("invalid Base64 payload: {e}"))?;
        (decoded, "base64".to_string())
    } else {
        (percent_decode(data)?, "url".to_string())
    };

    let bytes = raw.len();
    let mut truncated = false;

    // Classify: printable UTF-8 → text, else binary with file-type + hex.
    if let Ok(s) = std::str::from_utf8(&raw) {
        if printable_ratio(s) >= 0.85 {
            let text = if s.chars().count() > TEXT_PREVIEW {
                truncated = true;
                s.chars().take(TEXT_PREVIEW).collect()
            } else {
                s.to_string()
            };
            return Ok(DataUri {
                mime,
                params,
                charset,
                encoding,
                bytes,
                kind: "text".into(),
                text: Some(text),
                detected_type: None,
                hex_preview: None,
                truncated,
            });
        }
    }

    let detected_type = gizza_ai_detect_file_type_core::detect(&raw).map(|d| d.mime.to_string());
    if raw.len() > HEX_PREVIEW {
        truncated = true;
    }
    let hex_preview = hex::encode(&raw[..raw.len().min(HEX_PREVIEW)]);
    Ok(DataUri {
        mime,
        params,
        charset,
        encoding,
        bytes,
        kind: "binary".into(),
        text: None,
        detected_type,
        hex_preview: Some(hex_preview),
        truncated,
    })
}

/// Human-readable rendering (used by the page output).
pub fn render(input: &str) -> Result<String, String> {
    let d = decode(input)?;
    let mut out = String::new();
    out.push_str(&format!("MIME type:   {}\n", d.mime));
    if let Some(cs) = &d.charset {
        out.push_str(&format!("charset:     {cs}\n"));
    }
    for (k, v) in &d.params {
        if k == "charset" {
            continue;
        }
        out.push_str(&format!("param:       {k} = {v}\n"));
    }
    out.push_str(&format!("encoding:    {}\n", d.encoding));
    out.push_str(&format!("size:        {} bytes\n", d.bytes));
    out.push_str(&format!("payload:     {}\n", d.kind));
    if let Some(det) = &d.detected_type {
        out.push_str(&format!("detected:    {det} (from magic bytes)\n"));
    }
    if d.truncated {
        out.push_str("(payload truncated in this preview)\n");
    }
    out.push('\n');
    match d.kind.as_str() {
        "text" => {
            out.push_str("Decoded text:\n");
            out.push_str(d.text.as_deref().unwrap_or(""));
        }
        _ => {
            out.push_str("Hex preview:\n");
            out.push_str(d.hex_preview.as_deref().unwrap_or(""));
        }
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_plain_text_base64() {
        // data:text/plain;base64, of "Hello, World!"
        let b64 = STANDARD.encode("Hello, World!");
        let uri = format!("data:text/plain;base64,{b64}");
        let d = decode(&uri).unwrap();
        assert_eq!(d.mime, "text/plain");
        assert_eq!(d.encoding, "base64");
        assert_eq!(d.kind, "text");
        assert_eq!(d.text.as_deref(), Some("Hello, World!"));
        assert_eq!(d.bytes, 13);
    }

    #[test]
    fn decodes_charset_param() {
        let d = decode("data:text/plain;charset=utf-8,Hello%20%E4%B8%96%E7%95%8C").unwrap();
        assert_eq!(d.mime, "text/plain");
        assert_eq!(d.charset.as_deref(), Some("utf-8"));
        assert_eq!(d.encoding, "url");
        assert_eq!(d.text.as_deref(), Some("Hello 世界"));
    }

    #[test]
    fn default_mediatype_and_charset() {
        // data:,A%20brief%20note → text/plain;charset=US-ASCII default.
        let d = decode("data:,A%20brief%20note").unwrap();
        assert_eq!(d.mime, "text/plain");
        assert_eq!(d.charset.as_deref(), Some("US-ASCII"));
        assert_eq!(d.text.as_deref(), Some("A brief note"));
    }

    #[test]
    fn decodes_binary_png_with_filetype() {
        let png: &[u8] = &[
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01,
        ];
        let uri = format!("data:image/png;base64,{}", STANDARD.encode(png));
        let d = decode(&uri).unwrap();
        assert_eq!(d.mime, "image/png");
        assert_eq!(d.kind, "binary");
        assert_eq!(d.detected_type.as_deref(), Some("image/png"));
        assert!(d.hex_preview.as_deref().unwrap().starts_with("89504e47"));
    }

    #[test]
    fn data_with_commas_preserved() {
        // The first comma splits header/data; later commas are data.
        let d = decode("data:text/csv,a,b,c").unwrap();
        assert_eq!(d.mime, "text/csv");
        assert_eq!(d.text.as_deref(), Some("a,b,c"));
    }

    #[test]
    fn base64_whitespace_tolerated() {
        let b64 = STANDARD.encode("multi\nline");
        // Insert a newline mid-token to mimic wrapped base64.
        let mid = b64.len() / 2;
        let wrapped = format!("{}\n{}", &b64[..mid], &b64[mid..]);
        let d = decode(&format!("data:text/plain;base64,{wrapped}")).unwrap();
        assert_eq!(d.text.as_deref(), Some("multi\nline"));
    }

    #[test]
    fn rejects_non_data_uri() {
        assert!(decode("https://example.com").is_err());
        assert!(decode("hello world").is_err());
    }

    #[test]
    fn rejects_missing_comma() {
        assert!(decode("data:text/plain;base64").is_err());
    }

    #[test]
    fn rejects_bad_base64() {
        assert!(decode("data:application/octet-stream;base64,@@@not-base64@@@").is_err());
    }

    #[test]
    fn rejects_bad_percent_escape() {
        assert!(decode("data:text/plain,bad%ZZescape").is_err());
    }

    #[test]
    fn case_insensitive_prefix_and_base64_token() {
        let b64 = STANDARD.encode("ok");
        let d = decode(&format!("DATA:text/plain;BASE64,{b64}")).unwrap();
        assert_eq!(d.text.as_deref(), Some("ok"));
    }

    #[test]
    fn render_text_output() {
        let r = render("data:text/plain;charset=utf-8,Hi").unwrap();
        assert!(r.contains("MIME type:   text/plain"));
        assert!(r.contains("charset:     utf-8"));
        assert!(r.contains("Decoded text:\nHi"));
    }
}
