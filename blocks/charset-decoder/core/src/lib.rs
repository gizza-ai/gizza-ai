//! charset-decoder core — take a byte dump pasted as hex or base64, interpret
//! those bytes under a chosen character set, and hand back readable text. Pure
//! compute, shared by the chat skill block and the web page; no wafer/
//! wasm-bindgen deps.
//!
//! The job: bytes arrive as *transport text* (a hex dump from a debugger, a
//! base64 blob from a log/API/JWT payload) and the reader has no idea which
//! charset produced them. `charset=auto` sniffs a BOM, then pure-ASCII, then
//! valid-UTF-8, then falls back to chardetng (Firefox's statistical detector);
//! an explicit charset overrides all of that, which is the whole point when the
//! bytes are Windows-1251 or Shift_JIS and nothing but the human knows it.
//!
//! Deliberately distinct from its neighbours:
//!   * `hex-codec` decodes hex, but only ever as UTF-8 — no charset choice.
//!   * `charset-transcode` repairs mojibake in already-decoded UTF-8 *text*; it
//!     never sees raw bytes.
//!   * `text-encoding-converter` converts whole *files* (bytes in, bytes out)
//!     and has no page — this one is the paste-a-dump surface.
//!
//! encoding_rs is driven through the WITHOUT-replacement APIs so `replace` mode
//! can COUNT substitutions and `strict` mode can report the exact byte offset
//! (the convenience API only yields a had-errors bool).

use base64::engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD};
use base64::Engine as _;
use chardetng::EncodingDetector;
use encoding_rs::{DecoderResult, Encoding, UTF_16BE, UTF_16LE, UTF_8};
use serde::Serialize;

/// Maximum length of the pasted input, in bytes of the input string itself.
/// 1 MiB of hex is ~512 KiB of decoded bytes — far past any realistic paste,
/// and it keeps the whole pipeline (input + bytes + decoded String + rendered
/// output) comfortably inside the wasm sandbox.
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;

/// `hexdump` and `compare` render at most this many leading bytes; the rest is
/// summarised in a trailing note. `text`/`escaped` are never truncated.
pub const PREVIEW_BYTES: usize = 4096;

/// Characters of decoded text shown per row in `compare` mode.
pub const COMPARE_CHARS: usize = 80;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// How the pasted input encodes the bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputFormat {
    /// Detect: pure hex digits (after separators are stripped) with an even
    /// count wins; anything else is treated as base64.
    Auto,
    Hex,
    Base64,
}

impl InputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(InputFormat::Auto),
            "hex" | "base16" | "hexadecimal" => Ok(InputFormat::Hex),
            "base64" | "b64" | "base64url" => Ok(InputFormat::Base64),
            other => Err(format!(
                "invalid input_format {other:?}: expected \"auto\", \"hex\" or \"base64\""
            )),
        }
    }
}

/// What to render.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    /// The decoded text, exactly as decoded.
    Text,
    /// The decoded text with control and invisible characters shown as escapes.
    Escaped,
    /// `offset  hex bytes  |ascii|` rows over the raw bytes.
    Hexdump,
    /// The same bytes decoded under every common charset, side by side.
    Compare,
    /// A key/value diagnostic block (format, byte count, charset, BOM, …).
    Report,
}

impl Output {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" => Ok(Output::Text),
            "escaped" => Ok(Output::Escaped),
            "hexdump" => Ok(Output::Hexdump),
            "compare" => Ok(Output::Compare),
            "report" => Ok(Output::Report),
            other => Err(format!(
                "invalid output {other:?}: expected \"text\", \"escaped\", \"hexdump\", \"compare\" or \"report\""
            )),
        }
    }
}

/// What to do with byte sequences that aren't valid in the chosen charset.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Errors {
    /// Substitute U+FFFD and keep going, reporting how many times.
    Replace,
    /// Fail, naming the byte offset.
    Strict,
}

impl Errors {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "replace" => Ok(Errors::Replace),
            "strict" => Ok(Errors::Strict),
            other => Err(format!(
                "invalid errors {other:?}: expected \"replace\" or \"strict\""
            )),
        }
    }
}

/// Everything the caller can tune. Built from strings by every surface, so the
/// same validation errors reach chat, the CLI and the page.
#[derive(Clone, Debug)]
pub struct Options {
    pub input_format: InputFormat,
    /// Charset label, or `auto`. Any WHATWG label is accepted (plus
    /// `utf-32le`/`utf-32be`, which the WHATWG set omits).
    pub charset: String,
    pub output: Output,
    pub errors: Errors,
    /// Drop a leading byte-order mark when it belongs to the decode charset.
    pub strip_bom: bool,
    /// Decode each non-empty line of the input independently.
    pub per_line: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input_format: InputFormat::Auto,
            charset: "auto".to_string(),
            output: Output::Text,
            errors: Errors::Replace,
            strip_bom: true,
            per_line: false,
        }
    }
}

impl Options {
    /// Build from the raw strings every surface hands us (blank → default).
    pub fn from_strs(
        input_format: &str,
        charset: &str,
        output: &str,
        errors: &str,
        strip_bom: bool,
        per_line: bool,
    ) -> Result<Self, String> {
        let charset = charset.trim();
        Ok(Options {
            input_format: InputFormat::parse(input_format)?,
            charset: if charset.is_empty() {
                "auto".to_string()
            } else {
                charset.to_string()
            },
            output: Output::parse(output)?,
            errors: Errors::parse(errors)?,
            strip_bom,
            per_line,
        })
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// The decode result. `text` is already rendered per `Options::output`; the
/// remaining fields are the diagnostics chat and the CLI report alongside it.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Decoded {
    /// Rendered output (decoded text, escaped text, hexdump, comparison table
    /// or diagnostic report, depending on `output`).
    pub text: String,
    /// Canonical name of the charset the bytes were decoded with
    /// (`"mixed"` when `per_line` resolved different charsets per line).
    pub charset: String,
    /// How that charset was chosen: `specified`, `bom`, `ascii`,
    /// `valid-utf-8` or `detector`.
    pub charset_source: String,
    /// How the input was read: `hex` or `base64`.
    pub input_format: String,
    /// Number of bytes the input decoded to.
    pub bytes: usize,
    /// Number of characters those bytes decoded to.
    pub chars: usize,
    /// Byte sequences replaced with U+FFFD (`errors=replace` only).
    pub replaced: usize,
    /// Byte-order mark found at the start of the bytes, if any.
    pub bom: Option<String>,
}

// ---------------------------------------------------------------------------
// Byte-order marks
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Bom {
    Utf8,
    Utf16Le,
    Utf16Be,
    Utf32Le,
    Utf32Be,
}

impl Bom {
    fn label(self) -> &'static str {
        match self {
            Bom::Utf8 => "UTF-8",
            Bom::Utf16Le => "UTF-16LE",
            Bom::Utf16Be => "UTF-16BE",
            Bom::Utf32Le => "UTF-32LE",
            Bom::Utf32Be => "UTF-32BE",
        }
    }

    fn len(self) -> usize {
        match self {
            Bom::Utf8 => 3,
            Bom::Utf16Le | Bom::Utf16Be => 2,
            Bom::Utf32Le | Bom::Utf32Be => 4,
        }
    }
}

/// Sniff a leading BOM. UTF-32LE (`FF FE 00 00`) MUST be tested before UTF-16LE
/// (`FF FE`), which is a prefix of it.
fn sniff_bom(bytes: &[u8]) -> Option<Bom> {
    if bytes.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) {
        Some(Bom::Utf32Le)
    } else if bytes.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
        Some(Bom::Utf32Be)
    } else if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        Some(Bom::Utf8)
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        Some(Bom::Utf16Le)
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        Some(Bom::Utf16Be)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Charsets
// ---------------------------------------------------------------------------

/// A decodable source charset: everything `encoding_rs` handles, plus
/// hand-rolled UTF-32 (not part of the WHATWG Encoding Standard, so
/// `encoding_rs` has no decoder for it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Charset {
    Rs(&'static Encoding),
    Utf32Le,
    Utf32Be,
}

impl Charset {
    fn name(self) -> String {
        match self {
            Charset::Rs(e) => e.name().to_string(),
            Charset::Utf32Le => "UTF-32LE".to_string(),
            Charset::Utf32Be => "UTF-32BE".to_string(),
        }
    }

    /// Whether a sniffed BOM belongs to this charset's family (only then is it
    /// a BOM rather than ordinary data, and only then may it be stripped).
    fn owns_bom(self, bom: Bom) -> bool {
        match (self, bom) {
            (Charset::Rs(e), Bom::Utf8) => e == UTF_8,
            (Charset::Rs(e), Bom::Utf16Le) => e == UTF_16LE,
            (Charset::Rs(e), Bom::Utf16Be) => e == UTF_16BE,
            (Charset::Utf32Le, Bom::Utf32Le) => true,
            (Charset::Utf32Be, Bom::Utf32Be) => true,
            _ => false,
        }
    }
}

/// Lowercase and drop `-`/`_`/spaces so `"UTF-32 LE"`, `"utf_32le"` and
/// `"utf32le"` all compare equal (only needed for the UTF-32 special cases —
/// every other label goes to the WHATWG resolver, which knows its own aliases).
fn squash(label: &str) -> String {
    label
        .chars()
        .filter(|c| !matches!(c, '-' | '_' | ' '))
        .collect::<String>()
        .to_ascii_lowercase()
}

const CHARSET_EXAMPLES: &str = "\"utf-8\", \"utf-16le\", \"windows-1252\", \"iso-8859-1\" (alias \"latin1\"), \"windows-1251\", \"koi8-r\", \"shift_jis\" (alias \"sjis\"), \"euc-jp\", \"gbk\", \"big5\", \"euc-kr\"";

/// Resolve an explicit charset label (never `"auto"`).
fn resolve_charset(label: &str) -> Result<Charset, String> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return Err("a charset is required; use \"auto\" to detect one".to_string());
    }
    match squash(trimmed).as_str() {
        "utf32le" => return Ok(Charset::Utf32Le),
        "utf32be" => return Ok(Charset::Utf32Be),
        "utf32" => {
            return Err(
                "utf-32 needs an endianness: use charset=utf-32le or charset=utf-32be".to_string(),
            )
        }
        _ => {}
    }
    Encoding::for_label(trimmed.as_bytes())
        .map(Charset::Rs)
        .ok_or_else(|| {
            format!(
                "unknown charset {trimmed:?}: use \"auto\" or a charset label such as {CHARSET_EXAMPLES}"
            )
        })
}

/// The charsets `compare` decodes side by side — the ones legacy byte dumps
/// actually turn out to be, one per script family.
const COMPARE_CANDIDATES: &[&str] = &[
    "utf-8",
    "utf-16le",
    "windows-1252",
    "iso-8859-1",
    "iso-8859-15",
    "windows-1250",
    "windows-1251",
    "koi8-r",
    "iso-8859-7",
    "windows-1256",
    "shift_jis",
    "euc-jp",
    "gbk",
    "big5",
    "euc-kr",
];

/// Detect the charset of `bytes`: BOM, then pure ASCII, then valid UTF-8, then
/// the chardetng statistical detector. Returns the charset and how it was found.
fn detect_charset(bytes: &[u8]) -> (Charset, &'static str) {
    if let Some(bom) = sniff_bom(bytes) {
        let cs = match bom {
            Bom::Utf8 => Charset::Rs(UTF_8),
            Bom::Utf16Le => Charset::Rs(UTF_16LE),
            Bom::Utf16Be => Charset::Rs(UTF_16BE),
            Bom::Utf32Le => Charset::Utf32Le,
            Bom::Utf32Be => Charset::Utf32Be,
        };
        return (cs, "bom");
    }
    if bytes.iter().all(|b| *b < 0x80) {
        return (Charset::Rs(UTF_8), "ascii");
    }
    if std::str::from_utf8(bytes).is_ok() {
        return (Charset::Rs(UTF_8), "valid-utf-8");
    }
    let mut det = EncodingDetector::new();
    det.feed(bytes, true);
    (Charset::Rs(det.guess(None, true)), "detector")
}

// ---------------------------------------------------------------------------
// Input parsing (hex / base64 → bytes)
// ---------------------------------------------------------------------------

/// Characters ignored between hex digits, so a dump copied with spaces, colons,
/// dashes, commas or line breaks pastes in as-is.
fn is_hex_separator(c: char) -> bool {
    c.is_ascii_whitespace() || matches!(c, ':' | '-' | ',' | '_' | '.' | '|')
}

/// Strip the `data:<mime>;base64,` wrapper a data URI carries, if present.
fn strip_data_uri(s: &str) -> &str {
    let t = s.trim_start();
    if t.len() >= 5 && t[..5].eq_ignore_ascii_case("data:") {
        if let Some(i) = t.find(";base64,") {
            return &t[i + ";base64,".len()..];
        }
        if let Some(i) = t.find(',') {
            return &t[i + 1..];
        }
    }
    s
}

/// Decode a hex dump, tolerating whitespace, `:`/`-`/`,`/`_`/`.`/`|` separators
/// and per-byte `0x` / `\x` prefixes.
fn decode_hex(input: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(input.len() / 2);
    let mut hi: Option<u8> = None;
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if is_hex_separator(c) {
            i += 1;
            continue;
        }
        // Per-byte prefixes: `0x`/`0X` (only when a hex digit follows, so the
        // `0` of "0f 0x" isn't eaten) and `\x`.
        if (c == '0' || c == '\\')
            && i + 2 < chars.len()
            && (chars[i + 1] == 'x' || chars[i + 1] == 'X')
            && chars[i + 2].is_ascii_hexdigit()
        {
            i += 2;
            continue;
        }
        match c.to_digit(16) {
            Some(v) => {
                hi = match hi {
                    None => Some(v as u8),
                    Some(h) => {
                        out.push((h << 4) | v as u8);
                        None
                    }
                };
            }
            None => {
                return Err(format!(
                    "invalid hex digit {c:?} at position {i} — hex input takes 0-9 a-f, optionally separated by spaces, colons or dashes; set input_format=base64 if this is base64"
                ))
            }
        }
        i += 1;
    }
    if hi.is_some() {
        return Err(format!(
            "hex input has an odd number of digits ({}) — every byte needs exactly 2",
            out.len() * 2 + 1
        ));
    }
    Ok(out)
}

/// Decode base64, accepting the standard (`+/`) and URL-safe (`-_`) alphabets,
/// optional padding, embedded whitespace and a `data:` URI wrapper.
fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    let body = strip_data_uri(input);
    let cleaned: String = body.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let cleaned = cleaned.trim_end_matches('=');
    if cleaned.is_empty() {
        return Ok(Vec::new());
    }
    let std_alpha = cleaned.contains('+') || cleaned.contains('/');
    let url_alpha = cleaned.contains('-') || cleaned.contains('_');
    if std_alpha && url_alpha {
        return Err(
            "base64 input mixes the standard alphabet (+/) with the URL-safe one (-_); pick one"
                .to_string(),
        );
    }
    let engine = if url_alpha {
        &URL_SAFE_NO_PAD
    } else {
        &STANDARD_NO_PAD
    };
    engine.decode(cleaned).map_err(|e| {
        format!(
            "invalid base64 input: {e}. Accepted: standard (+/) or URL-safe (-_) alphabets, with \
             or without = padding; set input_format=hex if this is a hex dump"
        )
    })
}

/// Does this look like a hex dump? Every non-separator character must be a hex
/// digit (allowing per-byte 0x/\x prefixes) and at least one digit is present.
/// Odd digit counts still return true so auto mode can surface the better
/// "odd number of digits" hex error instead of a misleading base64 error.
fn looks_like_hex(input: &str) -> bool {
    let mut digits = 0usize;
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if is_hex_separator(c) {
            i += 1;
            continue;
        }
        if (c == '0' || c == '\\')
            && i + 2 < chars.len()
            && (chars[i + 1] == 'x' || chars[i + 1] == 'X')
            && chars[i + 2].is_ascii_hexdigit()
        {
            i += 2;
            continue;
        }
        if c.is_ascii_hexdigit() {
            digits += 1;
        } else {
            return false;
        }
        i += 1;
    }
    digits > 0
}

/// Turn the pasted input into bytes, reporting which format was used.
fn to_bytes(input: &str, format: InputFormat) -> Result<(Vec<u8>, &'static str), String> {
    match format {
        InputFormat::Hex => decode_hex(input).map(|b| (b, "hex")),
        InputFormat::Base64 => decode_base64(input).map(|b| (b, "base64")),
        InputFormat::Auto => {
            if looks_like_hex(input) {
                decode_hex(input).map(|b| (b, "hex"))
            } else {
                decode_base64(input).map(|b| (b, "base64"))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Byte → text decoding
// ---------------------------------------------------------------------------

/// Decode with an `encoding_rs` decoder, counting (`replace`) or rejecting
/// (`strict`) malformed sequences. The without-replacement API is what makes
/// the count and the byte offset available at all.
fn decode_rs(
    enc: &'static Encoding,
    bytes: &[u8],
    errors: Errors,
) -> Result<(String, usize), String> {
    let mut decoder = enc.new_decoder_without_bom_handling();
    let cap = decoder
        .max_utf8_buffer_length_without_replacement(bytes.len())
        .ok_or_else(|| "input too large to decode".to_string())?;
    let mut out = String::with_capacity(cap);
    let mut replaced = 0usize;
    let mut pos = 0usize;
    loop {
        let (res, read) =
            decoder.decode_to_string_without_replacement(&bytes[pos..], &mut out, true);
        pos += read;
        match res {
            DecoderResult::InputEmpty => break,
            DecoderResult::OutputFull => out.reserve(64 * 1024),
            DecoderResult::Malformed(seq_len, extra) => match errors {
                Errors::Strict => {
                    let offset = pos.saturating_sub(seq_len as usize + extra as usize);
                    return Err(format!(
                        "byte sequence at offset {offset} is not valid {}; use errors=replace to substitute U+FFFD, or pick a different charset (output=compare shows the candidates)",
                        enc.name()
                    ));
                }
                Errors::Replace => {
                    out.push('\u{FFFD}');
                    replaced += 1;
                }
            },
        }
    }
    Ok((out, replaced))
}

/// Hand-rolled UTF-32 decode (UTF-32 is not a WHATWG encoding).
fn decode_utf32(bytes: &[u8], le: bool, errors: Errors) -> Result<(String, usize), String> {
    let name = if le { "UTF-32LE" } else { "UTF-32BE" };
    let mut out = String::with_capacity(bytes.len() / 2);
    let mut replaced = 0usize;
    let mut i = 0usize;
    while i + 4 <= bytes.len() {
        let quad = [bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]];
        let v = if le {
            u32::from_le_bytes(quad)
        } else {
            u32::from_be_bytes(quad)
        };
        match char::from_u32(v) {
            Some(c) => out.push(c),
            None => match errors {
                Errors::Strict => {
                    return Err(format!(
                        "0x{v:X} at byte offset {i} is not a {name} code point; use errors=replace to substitute U+FFFD"
                    ))
                }
                Errors::Replace => {
                    out.push('\u{FFFD}');
                    replaced += 1;
                }
            },
        }
        i += 4;
    }
    if i != bytes.len() {
        match errors {
            Errors::Strict => {
                return Err(format!(
                    "truncated {name} input: the {} trailing byte(s) at offset {i} do not form a 4-byte unit",
                    bytes.len() - i
                ))
            }
            Errors::Replace => {
                out.push('\u{FFFD}');
                replaced += 1;
            }
        }
    }
    Ok((out, replaced))
}

fn decode_with(cs: Charset, bytes: &[u8], errors: Errors) -> Result<(String, usize), String> {
    match cs {
        Charset::Rs(e) => decode_rs(e, bytes, errors),
        Charset::Utf32Le => decode_utf32(bytes, true, errors),
        Charset::Utf32Be => decode_utf32(bytes, false, errors),
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Escape control and invisible characters so they can be seen. Printable and
/// non-ASCII graphic characters are left alone — the point is to reveal what a
/// plain text view hides, not to ASCII-fy the result.
pub fn escape_invisible(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // C0/C1 controls, DEL, and the usual invisible format characters.
            c if c.is_control()
                || matches!(
                    c,
                    '\u{00A0}'
                        | '\u{00AD}'
                        | '\u{200B}'..='\u{200F}'
                        | '\u{202A}'..='\u{202E}'
                        | '\u{2060}'
                        | '\u{FEFF}'
                ) =>
            {
                let v = c as u32;
                if v <= 0xFF {
                    out.push_str(&format!("\\x{v:02X}"));
                } else {
                    out.push_str(&format!("\\u{{{v:04X}}}"));
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// One-line preview for `compare` rows: escaped, collapsed to a single line and
/// truncated to `COMPARE_CHARS` characters.
fn compare_preview(s: &str) -> String {
    let escaped = escape_invisible(s);
    let mut out: String = escaped.chars().take(COMPARE_CHARS).collect();
    if escaped.chars().count() > COMPARE_CHARS {
        out.push('…');
    }
    out
}

/// Classic hexdump: `offset  16 hex bytes  |ascii gutter|`. The gutter is
/// byte-wise ASCII (a multi-byte character can't line up with its bytes) — the
/// charset-decoded text is what `output=text` is for.
fn render_hexdump(bytes: &[u8]) -> String {
    let shown = bytes.len().min(PREVIEW_BYTES);
    let mut out = String::with_capacity(shown / 16 * 78 + 64);
    for (row, chunk) in bytes[..shown].chunks(16).enumerate() {
        out.push_str(&format!("{:08x}  ", row * 16));
        for i in 0..16 {
            match chunk.get(i) {
                Some(b) => out.push_str(&format!("{b:02x} ")),
                None => out.push_str("   "),
            }
            if i == 7 {
                out.push(' ');
            }
        }
        out.push('|');
        for b in chunk {
            out.push(if (0x20..0x7f).contains(b) {
                *b as char
            } else {
                '.'
            });
        }
        out.push_str("|\n");
    }
    if bytes.len() > shown {
        out.push_str(&format!(
            "… {} more byte(s) (hexdump shows the first {shown})\n",
            bytes.len() - shown
        ));
    }
    out
}

/// Decode the same bytes under every candidate charset, one row each, with the
/// charset actually in use marked `→`.
fn render_compare(bytes: &[u8], chosen: &str) -> String {
    let sample = &bytes[..bytes.len().min(PREVIEW_BYTES)];
    let mut rows: Vec<(bool, String, String)> = Vec::with_capacity(COMPARE_CANDIDATES.len());
    let mut width = 0usize;
    for label in COMPARE_CANDIDATES {
        let cs = match resolve_charset(label) {
            Ok(cs) => cs,
            Err(_) => continue,
        };
        let name = cs.name();
        let (text, replaced) = match decode_with(cs, sample, Errors::Replace) {
            Ok(v) => v,
            Err(e) => (e, 0),
        };
        let mut cell = compare_preview(&text);
        if replaced > 0 {
            cell.push_str(&format!("   ({replaced} invalid)"));
        }
        width = width.max(name.chars().count());
        rows.push((name == chosen, name, cell));
    }
    let mut out = String::with_capacity(rows.len() * 96 + 128);
    for (is_chosen, name, cell) in rows {
        out.push_str(if is_chosen { "→ " } else { "  " });
        out.push_str(&format!("{name:<width$}  {cell}\n"));
    }
    if bytes.len() > sample.len() {
        out.push_str(&format!(
            "\n(compared over the first {} of {} bytes)\n",
            sample.len(),
            bytes.len()
        ));
    }
    out.push_str("\nRe-run with charset=<label> to decode with one of these.\n");
    out
}

/// Human phrase for how the charset was picked.
fn source_phrase(source: &str) -> &'static str {
    match source {
        "specified" => "as specified",
        "bom" => "auto-detected from the byte-order mark",
        "ascii" => "auto-detected: the bytes are pure 7-bit ASCII",
        "valid-utf-8" => "auto-detected: the bytes are valid UTF-8",
        "detector" => "auto-detected statistically (chardetng)",
        _ => "auto-detected",
    }
}

/// Key/value diagnostic block for `output=report`.
fn render_report(d: &Decoded, text: &str) -> String {
    let preview = compare_preview(text);
    format!(
        "input format   {}\nbytes          {}\ncharset        {} ({})\ncharacters     {}\nreplacements   {}\nbyte-order mark {}\ntext           {}\n",
        d.input_format,
        d.bytes,
        d.charset,
        source_phrase(&d.charset_source),
        d.chars,
        d.replaced,
        d.bom.as_deref().unwrap_or("none"),
        if preview.is_empty() {
            "(empty)".to_string()
        } else {
            preview
        },
    )
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// Decode one chunk of input (no `per_line` handling) into bytes + text +
/// diagnostics. `text_only` returns the raw decoded text alongside the render.
fn decode_chunk(input: &str, opts: &Options) -> Result<(Decoded, String), String> {
    let (bytes, format) = to_bytes(input, opts.input_format)?;
    let bom = sniff_bom(&bytes);
    let (cs, source) = if opts.charset.trim().eq_ignore_ascii_case("auto") {
        detect_charset(&bytes)
    } else {
        (resolve_charset(&opts.charset)?, "specified")
    };
    // A BOM is only a BOM under its own charset family; under any other charset
    // those bytes are ordinary data and must be decoded, not dropped.
    let body = match bom {
        Some(b) if opts.strip_bom && cs.owns_bom(b) => &bytes[b.len()..],
        _ => &bytes[..],
    };
    let (text, replaced) = decode_with(cs, body, opts.errors)?;
    let decoded = Decoded {
        text: String::new(),
        charset: cs.name(),
        charset_source: source.to_string(),
        input_format: format.to_string(),
        bytes: bytes.len(),
        chars: text.chars().count(),
        replaced,
        bom: bom.map(|b| b.label().to_string()),
    };
    let rendered = match opts.output {
        Output::Text => text.clone(),
        Output::Escaped => escape_invisible(&text),
        Output::Hexdump => render_hexdump(&bytes),
        Output::Compare => render_compare(&bytes, &decoded.charset),
        Output::Report => render_report(&decoded, &text),
    };
    Ok((
        Decoded {
            text: rendered,
            ..decoded
        },
        text,
    ))
}

/// Decode a hex or base64 byte dump into text under the chosen charset.
///
/// Errors on: an over-cap input, an unparseable hex/base64 body, an unknown
/// charset, a byte sequence invalid in the charset under `errors=strict`, and
/// `per_line` combined with an output mode that isn't per-line renderable.
pub fn decode(input: &str, opts: &Options) -> Result<Decoded, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, over the {MAX_INPUT_BYTES}-byte limit; decode it in smaller chunks",
            input.len()
        ));
    }
    if input.trim().is_empty() {
        return Err(
            "no input: paste the bytes as hex (e.g. 48 65 6c 6c 6f) or base64 (e.g. SGVsbG8=)"
                .to_string(),
        );
    }
    if !opts.per_line {
        return decode_chunk(input, opts).map(|(d, _)| d);
    }
    if !matches!(opts.output, Output::Text | Output::Escaped) {
        return Err(
            "per_line only applies to output=text or output=escaped; turn it off for hexdump, compare and report"
                .to_string(),
        );
    }

    let mut rendered = String::new();
    let mut charsets: Vec<String> = Vec::new();
    let mut sources: Vec<String> = Vec::new();
    let mut formats: Vec<String> = Vec::new();
    let (mut bytes, mut chars, mut replaced) = (0usize, 0usize, 0usize);
    let mut bom: Option<String> = None;
    let mut n = 0usize;
    for (i, line) in input.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (d, _) = decode_chunk(line, opts).map_err(|e| format!("line {}: {e}", i + 1))?;
        if n > 0 {
            rendered.push('\n');
        }
        rendered.push_str(&d.text);
        bytes += d.bytes;
        chars += d.chars;
        replaced += d.replaced;
        if bom.is_none() {
            bom = d.bom.clone();
        }
        charsets.push(d.charset);
        sources.push(d.charset_source);
        formats.push(d.input_format);
        n += 1;
    }
    if n == 0 {
        return Err("no non-empty lines to decode".to_string());
    }
    Ok(Decoded {
        text: rendered,
        charset: unify(&charsets),
        charset_source: unify(&sources),
        input_format: unify(&formats),
        bytes,
        chars,
        replaced,
        bom,
    })
}

/// Collapse per-line values to one: the common value, or `"mixed"`.
fn unify(values: &[String]) -> String {
    match values.first() {
        None => "mixed".to_string(),
        Some(first) => {
            if values.iter().all(|v| v == first) {
                first.clone()
            } else {
                "mixed".to_string()
            }
        }
    }
}

/// Convenience for the page/web surface: just the rendered text.
pub fn decode_text(input: &str, opts: &Options) -> Result<String, String> {
    decode(input, opts).map(|d| d.text)
}

/// Chat/CLI entry point: parse stringly surface options, decode the pasted
/// bytes, and return the rendered output plus diagnostics.
pub fn run(
    input: &str,
    input_format: &str,
    charset: &str,
    output: &str,
    errors: &str,
    strip_bom: bool,
    per_line: bool,
) -> Result<Decoded, String> {
    let opts = Options::from_strs(input_format, charset, output, errors, strip_bom, per_line)?;
    decode(input, &opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(charset: &str, output: &str) -> Options {
        Options {
            charset: charset.to_string(),
            output: Output::parse(output).unwrap(),
            ..Options::default()
        }
    }

    #[test]
    fn hex_ascii_round_trip() {
        let d = decode("48656c6c6f2c20776f726c6421", &Options::default()).unwrap();
        assert_eq!(d.text, "Hello, world!");
        assert_eq!(d.input_format, "hex");
        assert_eq!(d.charset, "UTF-8");
        assert_eq!(d.charset_source, "ascii");
        assert_eq!(d.bytes, 13);
        assert_eq!(d.chars, 13);
        assert_eq!(d.replaced, 0);
        assert_eq!(d.bom, None);
    }

    #[test]
    fn hex_tolerates_separators_and_prefixes() {
        for form in [
            "48 65 6c 6c 6f",
            "48:65:6C:6C:6F",
            "48-65-6c-6c-6f",
            "0x48 0x65 0x6c 0x6c 0x6f",
            "\\x48\\x65\\x6c\\x6c\\x6f",
            "48\n65\n6c\n6c\n6f",
        ] {
            assert_eq!(
                decode(form, &Options::default()).unwrap().text,
                "Hello",
                "{form}"
            );
        }
    }

    #[test]
    fn base64_standard_urlsafe_unpadded_and_data_uri() {
        // "Hello?~" encodes with both + and / in the standard alphabet.
        for (input, expect) in [
            ("SGVsbG8sIHdvcmxkIQ==", "Hello, world!"),
            ("SGVsbG8sIHdvcmxkIQ", "Hello, world!"),
            (
                "data:text/plain;base64,SGVsbG8sIHdvcmxkIQ==",
                "Hello, world!",
            ),
            ("SGVsbG8s\nIHdvcmxkIQ==", "Hello, world!"),
        ] {
            let d = decode(input, &Options::default()).unwrap();
            assert_eq!(d.text, expect, "{input}");
            assert_eq!(d.input_format, "base64");
        }
        // URL-safe alphabet: bytes FB FF FE decode from "-__-" style input.
        let std = decode("++/+", &opts("auto", "text")).unwrap();
        let url = decode("--_-", &opts("auto", "text")).unwrap();
        assert_eq!(std.bytes, 3);
        assert_eq!(std.text, url.text);
    }

    #[test]
    fn legacy_charsets_need_an_explicit_choice() {
        // 0xC0 0xF0 0xF2 0xE2 0xE5 0xF2 is "Привет"-ish Cyrillic in KOI8-R and
        // windows-1251, and gibberish elsewhere — the whole point of the tool.
        let bytes = "cff0e8e2e5f2";
        assert_eq!(
            decode(bytes, &opts("windows-1251", "text")).unwrap().text,
            "Привет"
        );
        let latin = decode(bytes, &opts("iso-8859-1", "text")).unwrap();
        assert_eq!(latin.text, "ÏðèâåòP".trim_end_matches('P'));
        assert_eq!(latin.charset, "windows-1252"); // WHATWG maps iso-8859-1 here
        let sjis = decode("824f82508251", &opts("shift_jis", "text")).unwrap();
        assert_eq!(sjis.text, "０１２");
        assert_eq!(sjis.charset, "Shift_JIS");
    }

    #[test]
    fn utf16_and_utf32_with_boms() {
        // "Hi" in UTF-16LE with a BOM.
        let d = decode("fffe48006900", &Options::default()).unwrap();
        assert_eq!(d.text, "Hi");
        assert_eq!(d.charset, "UTF-16LE");
        assert_eq!(d.charset_source, "bom");
        assert_eq!(d.bom.as_deref(), Some("UTF-16LE"));
        // Keeping the BOM leaves U+FEFF in the output.
        let kept = decode(
            "fffe48006900",
            &Options {
                strip_bom: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(kept.text, "\u{FEFF}Hi");
        // "Hi" in UTF-32LE with a BOM — sniffed ahead of the UTF-16LE prefix.
        let d32 = decode("fffe000048000000690000 00", &Options::default()).unwrap();
        assert_eq!(d32.text, "Hi");
        assert_eq!(d32.charset, "UTF-32LE");
    }

    #[test]
    fn auto_detects_utf8_over_legacy() {
        // C3 A9 = é in UTF-8; valid UTF-8 wins before the statistical detector.
        let d = decode("48c3a96c6c6f", &Options::default()).unwrap();
        assert_eq!(d.text, "Héllo");
        assert_eq!(d.charset_source, "valid-utf-8");
    }

    #[test]
    fn replace_counts_and_strict_reports_the_offset() {
        // 0xFF is not valid UTF-8 anywhere.
        let d = decode("48ff69", &opts("utf-8", "text")).unwrap();
        assert_eq!(d.text, "H\u{FFFD}i");
        assert_eq!(d.replaced, 1);
        let err = decode(
            "48ff69",
            &Options {
                charset: "utf-8".into(),
                errors: Errors::Strict,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("offset 1"), "{err}");
        assert!(err.contains("UTF-8"), "{err}");
    }

    #[test]
    fn escaped_reveals_invisible_characters() {
        // "A\tB\r\n" plus a non-breaking space and a zero-width space.
        let d = decode("4109420d0ac2a0e2808b", &opts("utf-8", "escaped")).unwrap();
        assert_eq!(d.text, "A\\tB\\r\\n\\xA0\\u{200B}");
    }

    #[test]
    fn hexdump_rows_and_gutter() {
        let d = decode("48656c6c6f", &opts("utf-8", "hexdump")).unwrap();
        assert_eq!(
            d.text,
            "00000000  48 65 6c 6c 6f                                   |Hello|\n"
        );
    }

    #[test]
    fn compare_lists_candidates_and_marks_the_chosen_one() {
        let d = decode("cff0e8e2e5f2", &opts("koi8-r", "compare")).unwrap();
        assert!(d.text.contains("→ KOI8-R"), "{}", d.text);
        assert!(d.text.contains("windows-1251  Привет"), "{}", d.text);
        assert!(d.text.contains("Re-run with charset="));
    }

    #[test]
    fn report_lists_the_diagnostics() {
        let d = decode("48656c6c6f", &opts("auto", "report")).unwrap();
        assert!(d.text.contains("input format   hex"), "{}", d.text);
        assert!(d.text.contains("bytes          5"), "{}", d.text);
        assert!(
            d.text
                .contains("charset        UTF-8 (auto-detected: the bytes are pure 7-bit ASCII)"),
            "{}",
            d.text
        );
    }

    #[test]
    fn per_line_decodes_each_line_independently() {
        let d = decode(
            "48656c6c6f\nSGVsbG8sIHdvcmxkIQ==\n\n576f726c64",
            &Options {
                per_line: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(d.text, "Hello\nHello, world!\nWorld");
        assert_eq!(d.input_format, "mixed");
        assert_eq!(d.bytes, 5 + 13 + 5);
    }

    #[test]
    fn per_line_rejects_non_line_outputs() {
        let err = decode(
            "48656c6c6f",
            &Options {
                per_line: true,
                output: Output::Hexdump,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("per_line only applies"), "{err}");
    }

    #[test]
    fn per_line_errors_name_the_line() {
        let err = decode(
            "48656c6c6f\n!!",
            &Options {
                per_line: true,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.starts_with("line 2:"), "{err}");
    }

    #[test]
    fn bad_input_errors_say_what_was_expected() {
        let odd = decode("48656c6c6", &opts("auto", "text")).unwrap_err();
        assert!(odd.contains("odd number of digits"), "{odd}");
        let bad_hex = decode("48 65 zz", &opts("auto", "text")).unwrap_err();
        assert!(bad_hex.contains("invalid base64 input"), "{bad_hex}");
        let forced_hex = decode(
            "48 65 zz",
            &Options {
                input_format: InputFormat::Hex,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(forced_hex.contains("invalid hex digit 'z'"), "{forced_hex}");
        let mixed = decode(
            "ab-cd_ef+gh/ij",
            &Options {
                input_format: InputFormat::Base64,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(mixed.contains("mixes the standard alphabet"), "{mixed}");
        let empty = decode("   ", &Options::default()).unwrap_err();
        assert!(empty.contains("no input"), "{empty}");
        let unknown = decode("4865", &opts("klingon-1", "text")).unwrap_err();
        assert!(unknown.contains("unknown charset"), "{unknown}");
        let utf32 = decode("4865", &opts("utf-32", "text")).unwrap_err();
        assert!(utf32.contains("needs an endianness"), "{utf32}");
    }

    #[test]
    fn input_cap_is_enforced_at_the_boundary() {
        let at_cap = "4".repeat(MAX_INPUT_BYTES); // even digit count → valid hex
        assert!(decode(&at_cap, &Options::default()).is_ok());
        let over = "4".repeat(MAX_INPUT_BYTES + 2);
        let err = decode(&over, &Options::default()).unwrap_err();
        assert!(err.contains("over the"), "{err}");
    }

    #[test]
    fn option_parsing_rejects_unknown_values() {
        assert!(Options::from_strs("auto", "utf-8", "text", "replace", true, false).is_ok());
        assert!(InputFormat::parse("morse").is_err());
        assert!(Output::parse("yaml").is_err());
        assert!(Errors::parse("ignore").is_err());
    }
}
