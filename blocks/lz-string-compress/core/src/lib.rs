//! lz-string-compress core — compress/decompress text with the LZ-String
//! algorithm (pieroxy's lz-string), in the three transport-safe encodings.
//! No wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.
//!
//! LZ-String is an LZW variant tuned for the browser: it packs the dictionary
//! stream into characters so the result survives in places where raw bytes
//! can't — `localStorage`/`sessionStorage` (UTF-16 strings), URLs (a
//! `?param=` value) and Base64 text. The three output `format`s mirror the JS
//! library's transport-safe encoders (the raw `compress()` is intentionally
//! omitted: it emits unbalanced UTF-16 code units that don't round-trip
//! through a URL or storage, which defeats the point of this tool).

/// Output / input encoding for the compressed payload.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    /// `compressToBase64` — A–Z a–z 0–9 `+ /` with `=` padding. The most
    /// portable; safe in JSON, headers, and anywhere ASCII text is expected.
    Base64,
    /// `compressToEncodedURIComponent` — URL-query-safe alphabet (`+ /`
    /// replaced by `- _`, no `=` padding), so it drops straight into a
    /// `?param=` value without further `encodeURIComponent`.
    Uri,
    /// `compressToUTF16` — packs 15 bits per UTF-16 char. The most compact for
    /// `localStorage`, which stores UTF-16 and so wastes space on Base64 text.
    Utf16,
}

impl Format {
    /// Parse the `format` argument; blank → `base64` (the most portable default).
    fn parse(format: &str) -> Result<Self, String> {
        match format {
            "" | "base64" => Ok(Format::Base64),
            "uri" => Ok(Format::Uri),
            "utf16" => Ok(Format::Utf16),
            other => Err(format!(
                "invalid format {other:?}: expected \"base64\", \"uri\", or \"utf16\""
            )),
        }
    }
}

/// Compress or decompress `text` with LZ-String.
///
/// - `mode` (`"compress"` | `"decompress"`, blank → `"compress"`): direction.
/// - `format` (`"base64"` | `"uri"` | `"utf16"`, blank → `"base64"`): the
///   encoding of the compressed payload — the output of `compress`, and the
///   expected input of `decompress`. Must match the encoding the payload was
///   produced with.
///
/// The Base64 output is normalised to be byte-identical to pieroxy's JS
/// `LZString.compressToBase64` (padded to a multiple of 4 `=`), so payloads
/// interoperate with browser apps using the original library.
///
/// Returns `Err` on an invalid `mode`/`format`, or when a decompress is given
/// input that isn't a valid LZ-String payload for the chosen `format`.
pub fn convert(text: &str, mode: &str, format: &str) -> Result<String, String> {
    let fmt = Format::parse(format)?;
    match mode {
        "" | "compress" => Ok(compress(text, fmt)),
        "decompress" => decompress(text, fmt),
        other => Err(format!(
            "invalid mode {other:?}: expected \"compress\" or \"decompress\""
        )),
    }
}

fn compress(text: &str, fmt: Format) -> String {
    match fmt {
        // The lz-str crate over-pads Base64 by one `=` vs the JS library;
        // re-pad to a multiple of 4 so the output matches `compressToBase64`.
        Format::Base64 => normalize_base64_padding(lz_str::compress_to_base64(text)),
        Format::Uri => lz_str::compress_to_encoded_uri_component(text),
        Format::Utf16 => lz_str::compress_to_utf16(text),
    }
}

/// Strip any `=` padding and re-pad to the next multiple of 4 — standard Base64
/// padding, matching `LZString.compressToBase64`. (Decompression ignores
/// padding, so this only affects the textual form, not round-tripping.)
fn normalize_base64_padding(s: String) -> String {
    let core_len = s.trim_end_matches('=').len();
    let mut out = s[..core_len].to_string();
    let pad = (4 - core_len % 4) % 4;
    out.extend(std::iter::repeat('=').take(pad));
    out
}

fn decompress(text: &str, fmt: Format) -> Result<String, String> {
    // An empty payload round-trips to an empty string (matches the JS library,
    // where decompressing "" yields ""). lz-str returns None for "", so handle
    // it before dispatching.
    if text.is_empty() {
        return Ok(String::new());
    }
    // lz-str's base64/uri decoders are silently lenient: each input char is
    // mapped through `KEY.iter().position(..)` inside a `flat_map`, so a char
    // that isn't in the alphabet contributes *nothing* and is dropped. Pure
    // garbage like "@@@@" therefore collapses to an empty bit-stream and
    // decodes to `Some(vec![])` — after the fact, indistinguishable from a
    // genuine empty payload such as `compressToBase64("")` (which is non-empty
    // text made of real alphabet chars and also decodes to zero code units).
    // The only honest way to tell them apart is to reject input containing
    // characters outside the format's alphabet *before* decoding, so invalid
    // payloads fail loudly instead of masquerading as an empty-string result.
    validate_alphabet(text, fmt)?;
    let units: Option<Vec<u16>> = match fmt {
        Format::Base64 => lz_str::decompress_from_base64(text),
        Format::Uri => lz_str::decompress_from_encoded_uri_component(text),
        Format::Utf16 => lz_str::decompress_from_utf16(text),
    };
    let units = units.ok_or_else(|| {
        "decompress failed: input is not a valid LZ-String payload for this format".to_string()
    })?;
    // The dictionary stream is UTF-16 code units; reassemble to a Rust String.
    String::from_utf16(&units)
        .map_err(|e| format!("decompress failed: result is not valid UTF-16 ({e})"))
}

/// Reject decompress input that contains characters outside the chosen
/// format's transport alphabet — these would be silently dropped by lz-str and
/// turn garbage into a bogus empty/short result. The alphabets mirror lz-str's
/// `BASE64_KEY` / `URI_KEY` exactly. `utf16` has no small alphabet (it packs 15
/// bits across nearly the whole BMP), so we leave that to lz-str's own decode.
fn validate_alphabet(text: &str, fmt: Format) -> Result<(), String> {
    let key: &[u8] = match fmt {
        Format::Base64 => b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=",
        Format::Uri => b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+-$",
        Format::Utf16 => return Ok(()),
    };
    for ch in text.chars() {
        // The uri decoder maps a literal space back to '+' (URLs turn the
        // payload's '+' into a space), so accept space for the uri alphabet.
        let ok = (fmt == Format::Uri && ch == ' ')
            || (ch.is_ascii() && key.contains(&(ch as u8)));
        if !ok {
            return Err(format!(
                "decompress failed: input is not a valid LZ-String payload for this \
                 format (unexpected character {ch:?})"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "The quick brown fox jumps over the lazy dog. 1234567890";

    #[test]
    fn base64_round_trips() {
        let c = convert(SAMPLE, "compress", "base64").unwrap();
        assert!(!c.is_empty());
        assert_eq!(convert(&c, "decompress", "base64").unwrap(), SAMPLE);
    }

    #[test]
    fn uri_round_trips_and_is_url_safe() {
        let c = convert(SAMPLE, "compress", "uri").unwrap();
        // The encoded-URI-component alphabet never contains chars that need
        // further escaping in a query value.
        assert!(!c.contains('+') && !c.contains('/') && !c.contains('='));
        assert_eq!(convert(&c, "decompress", "uri").unwrap(), SAMPLE);
    }

    #[test]
    fn utf16_round_trips() {
        let c = convert(SAMPLE, "compress", "utf16").unwrap();
        assert_eq!(convert(&c, "decompress", "utf16").unwrap(), SAMPLE);
    }

    #[test]
    fn unicode_round_trips() {
        let s = "São Paulo — café ☕ 日本語 😀";
        let c = convert(s, "compress", "base64").unwrap();
        assert_eq!(convert(&c, "decompress", "base64").unwrap(), s);
    }

    #[test]
    fn repetitive_input_actually_shrinks() {
        // LZ compression should beat the raw byte length on highly repetitive
        // text (Base64's ~4/3 expansion is more than offset by the dictionary).
        let s = "ab".repeat(500); // 1000 bytes
        let c = convert(&s, "compress", "base64").unwrap();
        assert!(c.len() < s.len(), "expected shrink, got {} >= {}", c.len(), s.len());
        assert_eq!(convert(&c, "decompress", "base64").unwrap(), s);
    }

    #[test]
    fn empty_input_round_trips() {
        let c = convert("", "compress", "base64").unwrap();
        assert_eq!(convert(&c, "decompress", "base64").unwrap(), "");
        // A literally-empty decompress input is also the empty string.
        assert_eq!(convert("", "decompress", "base64").unwrap(), "");
    }

    #[test]
    fn defaults_to_compress_base64_when_blank() {
        assert_eq!(
            convert(SAMPLE, "", "").unwrap(),
            convert(SAMPLE, "compress", "base64").unwrap()
        );
    }

    #[test]
    fn matches_js_lz_string_base64_vectors() {
        // Reference vectors generated from pieroxy's lz-string JS library
        // (npm `lz-string`), confirming byte-for-byte interop after padding
        // normalisation. Without normalisation the Rust crate emits one extra
        // trailing '='.
        assert_eq!(convert("Hello", "compress", "base64").unwrap(), "BIUwNmD2Q===");
        assert_eq!(
            convert(SAMPLE, "compress", "base64").unwrap(),
            "CoCwpgBAjgrglgYwNYQEYCcD2B3AdhAM0wA8IArGAWwAcBnCTANzHQgBdwIAbAQwC8AnhAAmmAOYA6CAEYATAGYALAFYAbAHYAHAE4ADEA=="
        );
        // The library's own output decompresses back.
        assert_eq!(
            convert("BIUwNmD2Q===", "decompress", "base64").unwrap(),
            "Hello"
        );
    }

    #[test]
    fn matches_js_lz_string_uri_vector() {
        // URI form needs no normalisation — already identical to JS.
        assert_eq!(convert("Hello", "compress", "uri").unwrap(), "BIUwNmD2Q");
    }

    #[test]
    fn base64_output_is_multiple_of_four() {
        for s in ["a", "Hello", SAMPLE, "x".repeat(37).as_str()] {
            let c = convert(s, "compress", "base64").unwrap();
            assert_eq!(c.len() % 4, 0, "{s:?} -> {c:?} not padded to 4");
        }
    }

    #[test]
    fn rejects_unknown_mode() {
        let err = convert("x", "encrypt", "base64").unwrap_err();
        assert!(err.contains("invalid mode"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_format() {
        let err = convert("x", "compress", "hex").unwrap_err();
        assert!(err.contains("invalid format"), "got: {err}");
    }

    #[test]
    fn rejects_garbage_decompress_input() {
        // Text that isn't a valid LZ-String payload should error, not panic or
        // return mojibake. lz-str silently drops out-of-alphabet chars, so
        // "@@@@" would otherwise collapse to a bogus empty result — reject it.
        let err = convert("@@@@", "decompress", "base64").unwrap_err();
        assert!(err.contains("decompress failed"), "got: {err}");
        // Same defence for the url-safe alphabet ('=' and '/' aren't in it).
        let err = convert("====", "decompress", "uri").unwrap_err();
        assert!(err.contains("decompress failed"), "got: {err}");
    }

    #[test]
    fn valid_empty_payload_decompresses_to_empty_not_rejected() {
        // The fix must NOT over-reject: `compress("")` is a real, non-empty
        // payload made of alphabet chars that legitimately decodes to "".
        for fmt in ["base64", "uri", "utf16"] {
            let c = convert("", "compress", fmt).unwrap();
            assert!(!c.is_empty(), "{fmt}: expected a non-empty empty-payload");
            assert_eq!(convert(&c, "decompress", fmt).unwrap(), "", "fmt={fmt}");
        }
    }

    #[test]
    fn cross_format_decode_is_distinct() {
        // The same source compressed to different formats yields different text.
        let b64 = convert(SAMPLE, "compress", "base64").unwrap();
        let uri = convert(SAMPLE, "compress", "uri").unwrap();
        assert_ne!(b64, uri);
    }
}
