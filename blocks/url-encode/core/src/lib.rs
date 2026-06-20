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

/// Maximum number of encode/decode rounds `repeat` may request. Mirrors the
/// common "decode recursively up to 16 times" cap for un-nesting input that was
/// percent-encoded several times over.
pub const MAX_REPEAT: u32 = 16;

/// Percent-encode or percent-decode `text`.
///
/// - `mode` (`"encode"` | `"decode"`, blank → `"encode"`): direction.
/// - `target` (`"component"` | `"uri"` | `"form"`, blank → `"component"`):
///     - `component`: escape everything outside the RFC 3986 unreserved set —
///       for a single query value or path segment.
///     - `uri`: keep the URL delimiters, escape only unsafe bytes — for a whole URL.
///     - `form`: `application/x-www-form-urlencoded` — like `component`, but a
///       space becomes `+` (and when decoding, `+` becomes a space).
/// - `per_line`: when `true`, split on `'\n'` and convert each line
///   independently, rejoining with `'\n'` (a batch list of values/URLs).
/// - `repeat`: apply the operation this many times, clamped to `1..=MAX_REPEAT`.
///   `repeat > 1` un-nests multiply-encoded input when decoding (or double-encodes).
///
/// Returns `Err` on an invalid `mode`/`target`, or when a decode round yields
/// invalid UTF-8.
pub fn convert(
    text: &str,
    mode: &str,
    target: &str,
    per_line: bool,
    repeat: u32,
) -> Result<String, String> {
    let op = Op::parse(mode)?;
    validate_target(target)?;
    let rounds = repeat.clamp(1, MAX_REPEAT);

    let run = |line: &str| -> Result<String, String> {
        let mut cur = line.to_string();
        for _ in 0..rounds {
            cur = op.apply(&cur, target)?;
        }
        Ok(cur)
    };

    if per_line {
        text.split('\n')
            .map(run)
            .collect::<Result<Vec<_>, _>>()
            .map(|lines| lines.join("\n"))
    } else {
        run(text)
    }
}

#[derive(Clone, Copy)]
enum Op {
    Encode,
    Decode,
}

impl Op {
    fn parse(mode: &str) -> Result<Self, String> {
        match mode {
            "" | "encode" => Ok(Op::Encode),
            "decode" => Ok(Op::Decode),
            other => Err(format!(
                "invalid mode {other:?}: expected \"encode\" or \"decode\""
            )),
        }
    }

    fn apply(self, text: &str, target: &str) -> Result<String, String> {
        match self {
            // `target` is pre-validated, so `encode` is infallible.
            Op::Encode => Ok(encode(text, target)),
            Op::Decode => decode(text, target),
        }
    }
}

fn validate_target(target: &str) -> Result<(), String> {
    match target {
        "" | "component" | "uri" | "form" => Ok(()),
        other => Err(format!(
            "invalid target {other:?}: expected \"component\", \"uri\", or \"form\""
        )),
    }
}

fn encode(text: &str, target: &str) -> String {
    match target {
        "uri" => utf8_percent_encode(text, URI).to_string(),
        // form: COMPONENT already encodes a literal `+` as `%2B`, so swapping
        // `%20` → `+` afterwards only ever turns real spaces into `+`.
        "form" => utf8_percent_encode(text, COMPONENT)
            .to_string()
            .replace("%20", "+"),
        // "" | "component" (target was validated upstream).
        _ => utf8_percent_encode(text, COMPONENT).to_string(),
    }
}

fn decode(text: &str, target: &str) -> Result<String, String> {
    // form decoding is the inverse of form encoding: a `+` is an encoded space.
    let swapped;
    let input = if target == "form" {
        swapped = text.replace('+', " ");
        swapped.as_str()
    } else {
        text
    };
    percent_decode_str(input)
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
            convert("São Paulo", "encode", "component", false, 1).unwrap(),
            "S%C3%A3o%20Paulo"
        );
        // Reserved delimiters are escaped in component mode.
        assert_eq!(
            convert("name=John Doe&city=x", "encode", "component", false, 1).unwrap(),
            "name%3DJohn%20Doe%26city%3Dx"
        );
    }

    #[test]
    fn encode_component_preserves_unreserved() {
        assert_eq!(
            convert("a-_.~Z9", "encode", "component", false, 1).unwrap(),
            "a-_.~Z9"
        );
    }

    #[test]
    fn encode_uri_preserves_structure_escapes_space() {
        // Whole-URL: delimiters survive, only the space gets encoded.
        assert_eq!(
            convert("https://ex.com/a b?x=1&y=2#frag", "encode", "uri", false, 1).unwrap(),
            "https://ex.com/a%20b?x=1&y=2#frag"
        );
    }

    #[test]
    fn defaults_to_encode_component_when_blank() {
        assert_eq!(convert("a b", "", "", false, 1).unwrap(), "a%20b");
    }

    #[test]
    fn decode_round_trips_unicode() {
        assert_eq!(
            convert("S%C3%A3o%20Paulo", "decode", "", false, 1).unwrap(),
            "São Paulo"
        );
        // component/uri decode leaves a literal `+` untouched.
        assert_eq!(
            convert("a%26b", "decode", "uri", false, 1).unwrap(),
            "a&b"
        );
    }

    #[test]
    fn decode_rejects_invalid_utf8() {
        // %FF is a lone 0xFF byte — never valid UTF-8.
        let err = convert("%FF", "decode", "", false, 1).unwrap_err();
        assert!(err.contains("UTF-8"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_mode() {
        let err = convert("x", "uppercase", "", false, 1).unwrap_err();
        assert!(err.contains("invalid mode"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_target() {
        let err = convert("x", "encode", "fullurl", false, 1).unwrap_err();
        assert!(err.contains("invalid target"), "got: {err}");
        // An invalid target is rejected on decode too, not silently ignored.
        let err = convert("x", "decode", "fullurl", false, 1).unwrap_err();
        assert!(err.contains("invalid target"), "got: {err}");
    }

    #[test]
    fn form_encodes_space_as_plus_and_escapes_literal_plus() {
        // space → '+', literal '+' → '%2B', '&'/'=' still escaped like component.
        assert_eq!(
            convert("a b+c&d=e", "encode", "form", false, 1).unwrap(),
            "a+b%2Bc%26d%3De"
        );
    }

    #[test]
    fn form_decode_round_trips_plus_as_space() {
        // The inverse of form encode: '+' decodes back to a space.
        assert_eq!(
            convert("a+b%2Bc", "decode", "form", false, 1).unwrap(),
            "a b+c"
        );
    }

    #[test]
    fn per_line_converts_each_line_independently() {
        assert_eq!(
            convert("a b\nc&d", "encode", "component", true, 1).unwrap(),
            "a%20b\nc%26d"
        );
        // Without per_line the newline itself is encoded (%0A).
        assert_eq!(
            convert("a b\nc&d", "encode", "component", false, 1).unwrap(),
            "a%20b%0Ac%26d"
        );
    }

    #[test]
    fn repeat_unnests_double_encoding() {
        // Encode "a b" twice, then decode twice to recover it.
        let once = convert("a b", "encode", "component", false, 1).unwrap();
        let twice = convert(&once, "encode", "component", false, 1).unwrap();
        assert_eq!(twice, "a%2520b");
        assert_eq!(
            convert(&twice, "decode", "component", false, 2).unwrap(),
            "a b"
        );
    }

    #[test]
    fn repeat_is_clamped_to_valid_range() {
        // repeat 0 behaves as 1 (a single round); huge values cap at MAX_REPEAT.
        assert_eq!(convert("a b", "encode", "component", false, 0).unwrap(), "a%20b");
        // Decoding a once-encoded string is idempotent past round 1, so a large
        // (clamped) repeat still recovers the original.
        assert_eq!(
            convert("a%20b", "decode", "component", false, 1_000).unwrap(),
            "a b"
        );
    }
}
