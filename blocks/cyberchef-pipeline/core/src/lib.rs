//! cyberchef-pipeline core — chain byte-level decode/transform steps into a
//! single client-side recipe, applied top to bottom over a byte buffer. Pure
//! Rust (`base64`, `flate2`, `percent-encoding`), wasm-safe. Shared by the chat
//! skill block and the web page.
//!
//! The recipe is ONE operation per line, applied in order. Blank lines and lines
//! beginning with `#` are ignored. Supported operations:
//!
//! ```text
//! from-base64        decode Base64 (whitespace + URL-safe + slack padding tolerated)
//! to-base64          encode Base64 (standard alphabet, padded)
//! from-hex           decode hex (ignores whitespace, ':', ',', and '0x')
//! to-hex             encode lowercase hex (no separator)
//! url-decode         percent-decode (%XX)
//! url-encode         percent-encode non-alphanumeric bytes
//! rot13              ROT13 on ASCII letters
//! gunzip / gzip      gzip decompress / compress
//! zlib-inflate       zlib (RFC 1950) decompress
//! zlib-deflate       zlib (RFC 1950) compress
//! raw-inflate        raw DEFLATE (RFC 1951) decompress
//! raw-deflate        raw DEFLATE (RFC 1951) compress
//! xor KEY [FMT]      XOR every byte with a repeating key (FMT: hex|utf8|base64|decimal, default hex)
//! add N              add N to every byte, mod 256 (N decimal or 0x..)
//! sub N              subtract N from every byte, mod 256
//! not                bitwise NOT every byte
//! reverse            reverse the byte order
//! upper / lower      ASCII upper / lower case
//! ```

use base64::alphabet;
use base64::engine::general_purpose::STANDARD;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use base64::Engine as _;
use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use flate2::Compression;
use percent_encoding::{percent_decode, utf8_percent_encode, NON_ALPHANUMERIC};
use std::io::{Read, Write};

/// Hard cap on the working buffer at any point (decompression-bomb defense).
pub const MAX_BYTES: usize = 16 * 1024 * 1024;

/// How to render the final byte buffer as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// UTF-8 if the whole buffer is valid UTF-8, otherwise lowercase hex.
    Auto,
    /// Lossy UTF-8 (invalid bytes become the replacement character).
    Utf8,
    /// Lowercase hex, no separator.
    Hex,
    /// Standard Base64, padded.
    Base64,
}

impl OutputFormat {
    /// Parse an output format (case-insensitive; blank → `Auto`). Unknown → `Err`.
    pub fn parse(s: &str) -> Result<OutputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(OutputFormat::Auto),
            "utf8" | "utf-8" | "text" => Ok(OutputFormat::Utf8),
            "hex" => Ok(OutputFormat::Hex),
            "base64" | "b64" => Ok(OutputFormat::Base64),
            other => Err(format!(
                "invalid output_format {other:?}: expected auto, utf8, hex, or base64"
            )),
        }
    }

    fn render(self, buf: &[u8]) -> String {
        match self {
            OutputFormat::Auto => match std::str::from_utf8(buf) {
                Ok(s) => s.to_string(),
                Err(_) => to_hex(buf),
            },
            OutputFormat::Utf8 => String::from_utf8_lossy(buf).into_owned(),
            OutputFormat::Hex => to_hex(buf),
            OutputFormat::Base64 => STANDARD.encode(buf),
        }
    }
}

/// Recipe options.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// How the final byte buffer is rendered as text.
    pub output_format: OutputFormat,
}

impl Default for Options {
    fn default() -> Self {
        Options { output_format: OutputFormat::Auto }
    }
}

fn to_hex(buf: &[u8]) -> String {
    let mut s = String::with_capacity(buf.len() * 2);
    for b in buf {
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((b & 0x0f) as u32, 16).unwrap());
    }
    s
}

/// A forgiving Base64 decoder: standard OR URL-safe alphabet, padding optional.
fn base64_decode(buf: &[u8]) -> Result<Vec<u8>, String> {
    // Strip ASCII whitespace so wrapped/pretty-printed Base64 still decodes.
    let cleaned: Vec<u8> = buf
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let url_safe = cleaned.iter().any(|&b| b == b'-' || b == b'_');
    let cfg = GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent);
    let alpha = if url_safe { &alphabet::URL_SAFE } else { &alphabet::STANDARD };
    let engine = GeneralPurpose::new(alpha, cfg);
    engine
        .decode(&cleaned)
        .map_err(|e| format!("from-base64: not valid Base64 ({e})"))
}

fn hex_decode(buf: &[u8]) -> Result<Vec<u8>, String> {
    // Ignore common separators and a leading/embedded "0x".
    let mut digits: Vec<u8> = Vec::with_capacity(buf.len());
    let mut i = 0;
    while i < buf.len() {
        let b = buf[i];
        if b == b'0' && i + 1 < buf.len() && (buf[i + 1] | 0x20) == b'x' {
            i += 2; // skip a 0x / 0X prefix
            continue;
        }
        if b.is_ascii_whitespace() || b == b':' || b == b',' || b == b'-' {
            i += 1;
            continue;
        }
        if !b.is_ascii_hexdigit() {
            return Err(format!(
                "from-hex: unexpected character {:?} (expected hex digits and separators)",
                b as char
            ));
        }
        digits.push(b);
        i += 1;
    }
    if digits.len() % 2 != 0 {
        return Err("from-hex: odd number of hex digits".into());
    }
    let mut out = Vec::with_capacity(digits.len() / 2);
    for pair in digits.chunks_exact(2) {
        let hi = (pair[0] as char).to_digit(16).unwrap() as u8;
        let lo = (pair[1] as char).to_digit(16).unwrap() as u8;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

/// Parse a repeating XOR key given as `KEY [hex|utf8|base64|decimal]`.
fn parse_xor_key(args: &str) -> Result<Vec<u8>, String> {
    let args = args.trim();
    if args.is_empty() {
        return Err("xor: missing key (e.g. 'xor 2a' or 'xor secret utf8')".into());
    }
    // Split off an optional trailing format token.
    let (key_part, fmt) = match args.rsplit_once(char::is_whitespace) {
        Some((left, right))
            if matches!(
                right.to_ascii_lowercase().as_str(),
                "hex" | "utf8" | "utf-8" | "base64" | "decimal"
            ) =>
        {
            (left.trim(), right.to_ascii_lowercase())
        }
        _ => (args, "hex".to_string()),
    };
    let key = match fmt.as_str() {
        "utf8" | "utf-8" => key_part.as_bytes().to_vec(),
        "base64" => STANDARD
            .decode(key_part.trim())
            .map_err(|e| format!("xor: key is not valid Base64 ({e})"))?,
        "decimal" => {
            let mut v = Vec::new();
            for tok in key_part.split(|c: char| c == ',' || c.is_whitespace()) {
                if tok.is_empty() {
                    continue;
                }
                let n: u16 = tok
                    .parse()
                    .map_err(|_| format!("xor: {tok:?} is not a decimal byte (0-255)"))?;
                if n > 255 {
                    return Err(format!("xor: decimal byte {n} out of range (0-255)"));
                }
                v.push(n as u8);
            }
            v
        }
        // hex
        _ => hex_decode(key_part.as_bytes()).map_err(|e| e.replace("from-hex", "xor: key"))?,
    };
    if key.is_empty() {
        return Err("xor: key decoded to zero bytes".into());
    }
    Ok(key)
}

/// Parse a single byte operand for `add`/`sub` (decimal or `0x..`).
fn parse_byte(arg: &str, op: &str) -> Result<u8, String> {
    let arg = arg.trim();
    if arg.is_empty() {
        return Err(format!("{op}: missing operand (e.g. '{op} 5' or '{op} 0x0a')"));
    }
    let n = if let Some(hex) = arg.strip_prefix("0x").or_else(|| arg.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).map_err(|_| format!("{op}: {arg:?} is not a valid number"))?
    } else {
        arg.parse::<u16>().map_err(|_| format!("{op}: {arg:?} is not a valid number"))?
    };
    if n > 255 {
        return Err(format!("{op}: operand {n} out of range (0-255)"));
    }
    Ok(n as u8)
}

fn inflate<R: Read>(dec: R, what: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    // Cap the decompressed size to defend against decompression bombs.
    let mut limited = dec.take((MAX_BYTES + 1) as u64);
    limited
        .read_to_end(&mut out)
        .map_err(|e| format!("{what}: not valid {what} data ({e})"))?;
    if out.len() > MAX_BYTES {
        return Err(format!("{what}: decompressed output exceeds {MAX_BYTES} bytes"));
    }
    Ok(out)
}

fn apply_op(line_no: usize, buf: Vec<u8>, name: &str, args: &str) -> Result<Vec<u8>, String> {
    let ctx = |e: String| format!("recipe line {line_no}: {e}");
    let out = match name {
        "from-base64" => base64_decode(&buf).map_err(ctx)?,
        "to-base64" => STANDARD.encode(&buf).into_bytes(),
        "from-hex" => hex_decode(&buf).map_err(ctx)?,
        "to-hex" => to_hex(&buf).into_bytes(),
        "url-decode" => percent_decode(&buf).collect(),
        "url-encode" => {
            let s = String::from_utf8_lossy(&buf);
            utf8_percent_encode(&s, NON_ALPHANUMERIC).to_string().into_bytes()
        }
        "rot13" => buf
            .iter()
            .map(|&b| match b {
                b'a'..=b'z' => (b - b'a' + 13) % 26 + b'a',
                b'A'..=b'Z' => (b - b'A' + 13) % 26 + b'A',
                other => other,
            })
            .collect(),
        "gunzip" => inflate(GzDecoder::new(&buf[..]), "gunzip").map_err(ctx)?,
        "zlib-inflate" => inflate(ZlibDecoder::new(&buf[..]), "zlib-inflate").map_err(ctx)?,
        "raw-inflate" => inflate(DeflateDecoder::new(&buf[..]), "raw-inflate").map_err(ctx)?,
        "gzip" => {
            let mut e = GzEncoder::new(Vec::new(), Compression::default());
            e.write_all(&buf)
                .and_then(|_| e.finish())
                .map_err(|e| ctx(format!("gzip: {e}")))?
        }
        "zlib-deflate" => {
            let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
            e.write_all(&buf)
                .and_then(|_| e.finish())
                .map_err(|e| ctx(format!("zlib-deflate: {e}")))?
        }
        "raw-deflate" => {
            let mut e = DeflateEncoder::new(Vec::new(), Compression::default());
            e.write_all(&buf)
                .and_then(|_| e.finish())
                .map_err(|e| ctx(format!("raw-deflate: {e}")))?
        }
        "xor" => {
            let key = parse_xor_key(args).map_err(ctx)?;
            buf.iter()
                .enumerate()
                .map(|(i, &b)| b ^ key[i % key.len()])
                .collect()
        }
        "add" => {
            let n = parse_byte(args, "add").map_err(ctx)?;
            buf.iter().map(|&b| b.wrapping_add(n)).collect()
        }
        "sub" => {
            let n = parse_byte(args, "sub").map_err(ctx)?;
            buf.iter().map(|&b| b.wrapping_sub(n)).collect()
        }
        "not" => buf.iter().map(|&b| !b).collect(),
        "reverse" => buf.iter().rev().copied().collect(),
        "upper" => buf.iter().map(|b| b.to_ascii_uppercase()).collect(),
        "lower" => buf.iter().map(|b| b.to_ascii_lowercase()).collect(),
        other => {
            return Err(ctx(format!(
                "unknown operation {other:?} (see the operation list)"
            )))
        }
    };
    if out.len() > MAX_BYTES {
        return Err(ctx(format!("output exceeds {MAX_BYTES} bytes")));
    }
    Ok(out)
}

/// Run `input` through `recipe` and render the result per `opts.output_format`.
pub fn run(input: &str, recipe: &str, opts: &Options) -> Result<String, String> {
    let mut buf = input.as_bytes().to_vec();
    if buf.len() > MAX_BYTES {
        return Err(format!("input exceeds {MAX_BYTES} bytes"));
    }
    let mut ran_any = false;
    for (idx, raw) in recipe.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name, args) = match line.split_once(char::is_whitespace) {
            Some((n, a)) => (n, a.trim()),
            None => (line, ""),
        };
        buf = apply_op(idx + 1, buf, &name.to_ascii_lowercase(), args)?;
        ran_any = true;
    }
    if !ran_any {
        return Err("recipe is empty: add at least one operation, one per line".into());
    }
    Ok(opts.output_format.render(&buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_ok(input: &str, recipe: &str) -> String {
        run(input, recipe, &Options::default()).unwrap()
    }

    #[test]
    fn from_base64_single_step() {
        assert_eq!(run_ok("SGVsbG8sIHdvcmxkIQ==", "from-base64"), "Hello, world!");
    }

    #[test]
    fn classic_decode_chain_base64_gunzip_xor() {
        // Build the canonical obfuscated payload: text -> xor 0x2a -> gzip -> base64.
        let plain = b"the quick brown fox";
        let xored: Vec<u8> = plain.iter().map(|b| b ^ 0x2a).collect();
        let mut gz = GzEncoder::new(Vec::new(), Compression::default());
        gz.write_all(&xored).unwrap();
        let gzipped = gz.finish().unwrap();
        let b64 = STANDARD.encode(&gzipped);
        // Decode chain reverses it.
        let out = run_ok(&b64, "from-base64\ngunzip\nxor 2a");
        assert_eq!(out, "the quick brown fox");
    }

    #[test]
    fn hex_roundtrip_and_url_and_rot13() {
        assert_eq!(run_ok("48656c6c6f", "from-hex"), "Hello");
        assert_eq!(run_ok("a b", "to-hex"), "612062");
        assert_eq!(run_ok("a%20b%2Fc", "url-decode"), "a b/c");
        assert_eq!(run_ok("Uryyb", "rot13"), "Hello");
    }

    #[test]
    fn xor_key_formats_and_arithmetic() {
        // utf8 key round-trips (xor is symmetric)
        let once = run_ok("secret", "xor pw utf8\nto-hex");
        assert_eq!(
            run(&once, "from-hex\nxor pw utf8", &Options::default()).unwrap(),
            "secret"
        );
        // add then sub cancels
        assert_eq!(run_ok("ABC", "add 5\nsub 5"), "ABC");
        // decimal key form
        assert_eq!(run_ok("AAA", "xor 65 decimal"), "\0\0\0");
    }

    #[test]
    fn output_format_selects_rendering() {
        // 0xff is not valid UTF-8, so hex format renders it plainly.
        let opts_hex = Options { output_format: OutputFormat::Hex };
        assert_eq!(run("ff", "from-hex", &opts_hex).unwrap(), "ff");
        let opts_b64 = Options { output_format: OutputFormat::Base64 };
        assert_eq!(run("Man", "to-base64\nfrom-base64", &opts_b64).unwrap(), "TWFu");
        // auto: valid utf8 stays text
        assert_eq!(run_ok("Man", "reverse"), "naM");
    }

    #[test]
    fn comments_and_blank_lines_ignored() {
        assert_eq!(run_ok("hello", "# just uppercase\n\nupper\n"), "HELLO");
    }

    #[test]
    fn unknown_operation_errors_with_line() {
        let err = run("x", "upper\nbogus-op foo", &Options::default()).unwrap_err();
        assert!(err.contains("recipe line 2"), "got: {err}");
        assert!(err.contains("bogus-op"), "got: {err}");
    }

    #[test]
    fn bad_hex_and_empty_recipe_error() {
        let err = run("abc", "from-hex", &Options::default()).unwrap_err();
        assert!(err.contains("odd number of hex digits"), "got: {err}");
        let err2 = run("abc", "# only a comment\n", &Options::default()).unwrap_err();
        assert!(err2.contains("recipe is empty"), "got: {err2}");
    }
}
