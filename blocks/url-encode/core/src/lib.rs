//! url-encode core — percent-encode / percent-decode text and URLs. No
//! wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};

/// Component encoding set: percent-encode everything that is not an RFC 3986
/// "unreserved" character (`A-Z a-z 0-9 - _ . ~`). Spaces, `&`, `=`, `/`, etc.
/// are all encoded — this is the set you want for a single query-string value or
/// path segment (e.g. `São Paulo` → `S%C3%A3o%20Paulo`, `a&b` → `a%26b`).
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// Whole-URL encoding set: like `COMPONENT`, but additionally preserves the RFC
/// 3986 "reserved" characters that carry structural meaning in a URL
/// (`: / ? # [ ] @ ! $ & ' ( ) * + , ; =`). Use this to clean up an entire URL
/// without breaking its delimiters — only genuinely-unsafe bytes (spaces,
/// non-ASCII, control chars) get encoded.
const URI: &AsciiSet = &COMPONENT
    .remove(b':')
    .remove(b'/')
    .remove(b'?')
    .remove(b'#')
    .remove(b'[')
    .remove(b']')
    .remove(b'@')
    .remove(b'!')
    .remove(b'$')
    .remove(b'&')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')')
    .remove(b'*')
    .remove(b'+')
    .remove(b',')
    .remove(b';')
    .remove(b'=');

/// Percent-encode or percent-decode `text`.
///
/// - `mode` (`"encode"` | `"decode"`, default `"encode"` when empty): direction.
/// - `target` (`"component"` | `"uri"`, default `"component"` when empty): which
///   character set to preserve when encoding. Ignored for decode.
///
/// Returns `Err` on an explicitly-bad `mode`/`target` value, or when decoding
/// yields invalid UTF-8.
pub fn convert(text: &str, mode: &str, target: &str) -> Result<String, String> {
    match mode {
        "" | "encode" => encode(text, target),
        "decode" => decode(text),
        other => Err(format!(
            "invalid mode {other:?}: expected \"encode\" or \"decode\""
        )),
    }
}

fn encode(text: &str, target: &str) -> Result<String, String> {
    let set: &AsciiSet = match target {
        "" | "component" => COMPONENT,
        "uri" => URI,
        other => {
            return Err(format!(
                "invalid target {other:?}: expected \"component\" or \"uri\""
            ))
        }
    };
    Ok(utf8_percent_encode(text, set).to_string())
}

fn decode(text: &str) -> Result<String, String> {
    percent_decode_str(text)
        .decode_utf8()
        .map(|cow| cow.into_owned())
        .map_err(|e| format!("decode failed: input is not valid UTF-8 ({e})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_component_escapes_space_and_unicode() {
        assert_eq!(
            convert("São Paulo", "encode", "component").unwrap(),
            "S%C3%A3o%20Paulo"
        );
        // Reserved delimiters are escaped in component mode.
        assert_eq!(
            convert("name=John Doe&city=x", "encode", "component").unwrap(),
            "name%3DJohn%20Doe%26city%3Dx"
        );
    }

    #[test]
    fn encode_component_preserves_unreserved() {
        assert_eq!(
            convert("a-_.~Z9", "encode", "component").unwrap(),
            "a-_.~Z9"
        );
    }

    #[test]
    fn encode_uri_preserves_structure_escapes_space() {
        // Whole-URL: delimiters survive, only the space gets encoded.
        assert_eq!(
            convert("https://ex.com/a b?x=1&y=2#frag", "encode", "uri").unwrap(),
            "https://ex.com/a%20b?x=1&y=2#frag"
        );
    }

    #[test]
    fn defaults_to_encode_component_when_blank() {
        assert_eq!(convert("a b", "", "").unwrap(), "a%20b");
    }

    #[test]
    fn decode_round_trips_unicode() {
        assert_eq!(
            convert("S%C3%A3o%20Paulo", "decode", "").unwrap(),
            "São Paulo"
        );
        // target is ignored on decode.
        assert_eq!(convert("a%26b", "decode", "uri").unwrap(), "a&b");
    }

    #[test]
    fn decode_rejects_invalid_utf8() {
        // %FF is a lone 0xFF byte — never valid UTF-8.
        let err = convert("%FF", "decode", "").unwrap_err();
        assert!(err.contains("UTF-8"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_mode() {
        let err = convert("x", "uppercase", "").unwrap_err();
        assert!(err.contains("invalid mode"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_target() {
        let err = convert("x", "encode", "fullurl").unwrap_err();
        assert!(err.contains("invalid target"), "got: {err}");
    }
}
