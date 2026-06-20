//! gizza-ai/file-to-data-uri core — build a base64 `data:` URI from file bytes +
//! a MIME type. No wafer/wasm-bindgen deps. Pure assembly + base64.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

/// Fallback MIME when none is known.
pub const DEFAULT_MIME: &str = "application/octet-stream";

/// Normalize a MIME string: trim, and fall back to octet-stream when empty.
pub fn normalize_mime(mime: &str) -> &str {
    let m = mime.trim();
    if m.is_empty() {
        DEFAULT_MIME
    } else {
        m
    }
}

/// Build a `data:<mime>;base64,<payload>` URI from raw `bytes`.
pub fn to_data_uri(bytes: &[u8], mime: &str) -> String {
    format!("data:{};base64,{}", normalize_mime(mime), B64.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vector_text() {
        // base64("hi") = aGk=
        assert_eq!(to_data_uri(b"hi", "text/plain"), "data:text/plain;base64,aGk=");
    }

    #[test]
    fn empty_mime_defaults_to_octet_stream() {
        assert_eq!(to_data_uri(b"x", ""), "data:application/octet-stream;base64,eA==");
        assert_eq!(to_data_uri(b"x", "   "), "data:application/octet-stream;base64,eA==");
    }

    #[test]
    fn mime_is_trimmed() {
        assert_eq!(normalize_mime("  image/png  "), "image/png");
        assert_eq!(normalize_mime(""), DEFAULT_MIME);
    }

    #[test]
    fn empty_bytes_yields_empty_payload() {
        assert_eq!(to_data_uri(b"", "text/plain"), "data:text/plain;base64,");
    }
}
