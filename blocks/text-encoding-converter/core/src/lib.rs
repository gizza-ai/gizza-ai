//! text-encoding-converter core — byte-level charset detection + conversion
//! (the iconv/chardet job). Pure compute, shared by the chat skill block and
//! the CLI; no wafer/wasm-bindgen deps.
//!
//! Input is RAW BYTES in an unknown or stated encoding (an uploaded/fetched
//! text file). We sniff a BOM first (UTF-8 / UTF-16LE/BE / UTF-32LE/BE), then
//! fall back to `chardetng` (Firefox's statistical detector) for BOM-less
//! legacy input, decode to Unicode, and re-encode to any WHATWG target charset
//! plus hand-rolled UTF-16LE/BE writers (the WHATWG spec — and therefore
//! `encoding_rs` — has no UTF-16 *encoder*).
//!
//! This is deliberately DIFFERENT from `blocks/charset-transcode`, which
//! repairs mojibake in already-decoded UTF-8 *text* (it never sees raw legacy
//! bytes and can only output UTF-8 text). Here bytes go in and bytes come out.
//!
//! Honesty note: `chardetng` returns a single best guess and NO numeric
//! confidence — we surface the guess plus a `candidates` list (multi-byte
//! charsets under which every byte sequence in the sample is valid) instead of
//! inventing a score.

use chardetng::EncodingDetector;
use encoding_rs::{DecoderResult, EncoderResult, Encoding, REPLACEMENT, UTF_16BE, UTF_16LE, UTF_8};

/// Max characters kept in text previews (detect report + convert summary).
pub const MAX_PREVIEW_CHARS: usize = 160;
/// Statistical detection samples at most this many leading bytes (keeps the
/// wasmi-interpreted detector fast on multi-MiB files).
pub const DETECT_SAMPLE_BYTES: usize = 1024 * 1024;
/// Candidate validity checks sample at most this many leading bytes.
pub const CANDIDATE_SAMPLE_BYTES: usize = 256 * 1024;

/// Policy for undecodable byte sequences / unencodable characters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Errors {
    /// Substitute U+FFFD on decode, `?` on encode, and keep going.
    Replace,
    /// Fail with a positioned error instead.
    Strict,
}

impl Errors {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "replace" => Ok(Errors::Replace),
            "strict" => Ok(Errors::Strict),
            other => Err(format!(
                "invalid errors {other:?}: expected \"replace\" or \"strict\""
            )),
        }
    }
}

/// A byte-order mark found at the start of the input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bom {
    Utf8,
    Utf16Le,
    Utf16Be,
    Utf32Le,
    Utf32Be,
}

impl Bom {
    pub fn label(self) -> &'static str {
        match self {
            Bom::Utf8 => "UTF-8",
            Bom::Utf16Le => "UTF-16LE",
            Bom::Utf16Be => "UTF-16BE",
            Bom::Utf32Le => "UTF-32LE",
            Bom::Utf32Be => "UTF-32BE",
        }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(self) -> usize {
        match self {
            Bom::Utf8 => 3,
            Bom::Utf16Le | Bom::Utf16Be => 2,
            Bom::Utf32Le | Bom::Utf32Be => 4,
        }
    }
}

/// Sniff a leading BOM. UTF-32 patterns are checked BEFORE UTF-16 because
/// `FF FE 00 00` (UTF-32LE) starts with `FF FE` (UTF-16LE) — standard sniffers
/// resolve that prefix collision in favor of UTF-32.
pub fn sniff_bom(bytes: &[u8]) -> Option<Bom> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        Some(Bom::Utf8)
    } else if bytes.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) {
        Some(Bom::Utf32Le)
    } else if bytes.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
        Some(Bom::Utf32Be)
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        Some(Bom::Utf16Le)
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        Some(Bom::Utf16Be)
    } else {
        None
    }
}

/// A resolved source encoding: everything `encoding_rs` decodes, plus
/// hand-rolled UTF-32 (not part of the WHATWG set).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Src {
    Rs(&'static Encoding),
    Utf32Le,
    Utf32Be,
}

impl Src {
    fn name(self) -> String {
        match self {
            Src::Rs(e) => e.name().to_string(),
            Src::Utf32Le => "UTF-32LE".to_string(),
            Src::Utf32Be => "UTF-32BE".to_string(),
        }
    }

    /// Whether a sniffed Unicode BOM belongs to this source's family.
    fn matches_bom(self, bom: Bom) -> bool {
        match (self, bom) {
            (Src::Rs(e), Bom::Utf8) => e == UTF_8,
            (Src::Rs(e), Bom::Utf16Le) => e == UTF_16LE,
            (Src::Rs(e), Bom::Utf16Be) => e == UTF_16BE,
            (Src::Utf32Le, Bom::Utf32Le) => true,
            (Src::Utf32Be, Bom::Utf32Be) => true,
            _ => false,
        }
    }
}

/// Lowercase and strip `-`/`_`/space so "UTF-32 LE", "utf_32le" and "utf32le"
/// compare equal (only used for the UTF-32 special cases; everything else goes
/// straight to the WHATWG label resolver, which knows its own aliases).
fn squash(label: &str) -> String {
    label
        .trim()
        .chars()
        .filter(|c| !matches!(c, '-' | '_' | ' '))
        .collect::<String>()
        .to_ascii_lowercase()
}

const CHARSET_EXAMPLES: &str = "\"utf-8\", \"utf-16le\", \"shift_jis\" (alias \"sjis\"), \"euc-jp\", \"iso-2022-jp\", \"gbk\", \"gb18030\", \"big5\", \"euc-kr\", \"windows-1252\", \"iso-8859-1\" (alias \"latin1\"), \"windows-1251\", \"koi8-r\", \"macintosh\"";

/// Resolve an explicit `from` label (NOT "auto") against an optional sniffed
/// BOM. A Unicode BOM that contradicts the stated charset is an error — the
/// BOM is authoritative, and decoding it as legacy data would inject mojibake.
fn resolve_explicit(label: &str, sniffed: Option<Bom>) -> Result<Src, String> {
    let sq = squash(label);
    let src = match sq.as_str() {
        "utf32le" => Src::Utf32Le,
        "utf32be" => Src::Utf32Be,
        "utf32" => match sniffed {
            Some(Bom::Utf32Le) => Src::Utf32Le,
            Some(Bom::Utf32Be) => Src::Utf32Be,
            _ => {
                return Err(
                    "utf-32 needs an endianness when there is no BOM: use from=utf-32le or from=utf-32be".to_string(),
                )
            }
        },
        _ => match Encoding::for_label(label.trim().as_bytes()) {
            Some(e) => Src::Rs(e),
            None => {
                return Err(format!(
                    "unknown source charset {label:?}: use \"auto\" or a WHATWG label such as {CHARSET_EXAMPLES}"
                ))
            }
        },
    };
    if let Some(bom) = sniffed {
        if !src.matches_bom(bom) {
            return Err(format!(
                "the input starts with a {} byte-order mark, which contradicts from={label:?}; pass from=auto (or from={}) instead",
                bom.label(),
                bom.label().to_ascii_lowercase(),
            ));
        }
    }
    Ok(src)
}

/// Decode with an `encoding_rs` decoder, counting (or rejecting) malformed
/// sequences. Driven through the without-replacement API so `Replace` mode can
/// COUNT substitutions (the convenience API only reports a had-errors bool)
/// and `Strict` mode can report a byte offset.
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
                        "byte sequence not valid in {} at byte offset {offset}; use errors=replace to substitute U+FFFD, or a different 'from' charset",
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

/// Hand-rolled UTF-32 decode (UTF-32 is not a WHATWG encoding, so
/// `encoding_rs` has no decoder for it).
fn decode_utf32(
    bytes: &[u8],
    little_endian: bool,
    errors: Errors,
) -> Result<(String, usize), String> {
    let name = if little_endian {
        "UTF-32LE"
    } else {
        "UTF-32BE"
    };
    let mut out = String::with_capacity(bytes.len());
    let mut replaced = 0usize;
    let mut i = 0usize;
    while i + 4 <= bytes.len() {
        let quad = [bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]];
        let v = if little_endian {
            u32::from_le_bytes(quad)
        } else {
            u32::from_be_bytes(quad)
        };
        match char::from_u32(v) {
            Some(c) => out.push(c),
            None => match errors {
                Errors::Strict => {
                    return Err(format!(
                        "invalid {name} code point 0x{v:X} at byte offset {i}; use errors=replace to substitute U+FFFD"
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
                    "truncated {name} input: {} trailing byte(s) at offset {i} do not form a 4-byte unit",
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

fn decode_src(src: Src, bytes: &[u8], errors: Errors) -> Result<(String, usize), String> {
    match src {
        Src::Rs(e) => decode_rs(e, bytes, errors),
        Src::Utf32Le => decode_utf32(bytes, true, errors),
        Src::Utf32Be => decode_utf32(bytes, false, errors),
    }
}

/// The result of encoding text into the target charset.
#[derive(Debug)]
pub struct Encoded {
    pub out: Vec<u8>,
    /// Canonical target name (e.g. "Shift_JIS", "UTF-16LE").
    pub name: String,
    /// Characters replaced with `?` (Replace mode only).
    pub replaced: usize,
    pub bom_written: bool,
}

/// Encode `text` into the target charset named by `label`. `bom` prepends a
/// byte-order mark — meaningful for UTF-8 only (off by default; UTF-16 output
/// ALWAYS gets a BOM, standard practice for UTF-16 files; legacy charsets have
/// no BOM, so `bom=true` there is an error rather than a silent no-op).
pub fn encode_to(text: &str, label: &str, errors: Errors, bom: bool) -> Result<Encoded, String> {
    let sq = squash(label);
    if matches!(sq.as_str(), "utf32" | "utf32le" | "utf32be") {
        return Err(
            "UTF-32 output is not supported: choose utf-8, utf-16le, utf-16be, or a legacy charset such as shift_jis or windows-1252"
                .to_string(),
        );
    }
    let enc = Encoding::for_label(label.trim().as_bytes()).ok_or_else(|| {
        format!("unknown target charset {label:?}: use a WHATWG label such as {CHARSET_EXAMPLES}")
    })?;
    if enc == UTF_8 {
        let mut out = Vec::with_capacity(text.len() + 3);
        if bom {
            out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }
        out.extend_from_slice(text.as_bytes());
        return Ok(Encoded {
            out,
            name: "UTF-8".to_string(),
            replaced: 0,
            bom_written: bom,
        });
    }
    if enc == UTF_16LE || enc == UTF_16BE {
        let le = enc == UTF_16LE;
        let mut out = Vec::with_capacity(2 + 2 * text.len());
        out.extend_from_slice(if le { &[0xFF, 0xFE] } else { &[0xFE, 0xFF] });
        for unit in text.encode_utf16() {
            out.extend_from_slice(&if le {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        return Ok(Encoded {
            out,
            name: if le { "UTF-16LE" } else { "UTF-16BE" }.to_string(),
            replaced: 0,
            bom_written: true,
        });
    }
    if enc == REPLACEMENT {
        return Err(format!(
            "charset {label:?} is decode-only (a WHATWG replacement encoding) and cannot be an output target"
        ));
    }
    if bom {
        return Err(format!(
            "bom=true applies only to Unicode targets (utf-8, utf-16le, utf-16be); {} has no byte-order mark",
            enc.name()
        ));
    }
    let mut encoder = enc.new_encoder();
    let cap = encoder
        .max_buffer_length_from_utf8_without_replacement(text.len())
        .ok_or_else(|| "input too large to encode".to_string())?;
    let mut out = Vec::with_capacity(cap);
    let mut replaced = 0usize;
    let mut pos = 0usize;
    loop {
        let (res, read) =
            encoder.encode_from_utf8_to_vec_without_replacement(&text[pos..], &mut out, true);
        pos += read;
        match res {
            EncoderResult::InputEmpty => break,
            EncoderResult::OutputFull => out.reserve(64 * 1024),
            EncoderResult::Unmappable(c) => match errors {
                Errors::Strict => {
                    let char_index = text[..pos].chars().count().saturating_sub(1);
                    return Err(format!(
                        "character '{c}' (U+{:04X}) at character index {char_index} cannot be encoded in {}; use errors=replace to substitute '?', or a Unicode target like utf-8",
                        c as u32,
                        enc.name()
                    ));
                }
                Errors::Replace => {
                    // Safe even for ISO-2022-JP: the WHATWG encoder reports
                    // unmappables only after transitioning back to ASCII state.
                    out.push(b'?');
                    replaced += 1;
                }
            },
        }
    }
    out.shrink_to_fit();
    Ok(Encoded {
        out,
        name: enc.name().to_string(),
        replaced,
        bom_written: false,
    })
}

/// True if `bytes` decodes under `enc` with zero malformed sequences.
/// `last=false` when `bytes` is a truncated sample (a cut-off trailing
/// sequence must not count as malformed).
fn decodes_cleanly(enc: &'static Encoding, bytes: &[u8], last: bool) -> bool {
    let mut decoder = enc.new_decoder_without_bom_handling();
    let mut scratch = String::with_capacity(64 * 1024);
    let mut pos = 0usize;
    loop {
        let (res, read) =
            decoder.decode_to_string_without_replacement(&bytes[pos..], &mut scratch, last);
        pos += read;
        match res {
            DecoderResult::Malformed(..) => return false,
            DecoderResult::InputEmpty => return true,
            DecoderResult::OutputFull => scratch.clear(),
        }
    }
}

/// Decode up to `limit` leading bytes with replacement (never fails) and trim
/// to a display preview. Non-final samples pass `last=false` so a cut-off
/// trailing sequence at the sample boundary doesn't surface as spurious U+FFFD.
fn preview_of(src: Src, bytes: &[u8], limit: usize) -> String {
    let sample = &bytes[..bytes.len().min(limit)];
    let text = match src {
        Src::Rs(e) => {
            let mut decoder = e.new_decoder_without_bom_handling();
            let cap = decoder
                .max_utf8_buffer_length(sample.len())
                .unwrap_or(sample.len() * 3 + 16);
            let mut out = String::with_capacity(cap);
            let last = sample.len() == bytes.len();
            let _ = decoder.decode_to_string(sample, &mut out, last);
            out
        }
        Src::Utf32Le => decode_utf32(sample, true, Errors::Replace)
            .map(|(t, _)| t)
            .unwrap_or_default(),
        Src::Utf32Be => decode_utf32(sample, false, Errors::Replace)
            .map(|(t, _)| t)
            .unwrap_or_default(),
    };
    preview_str(&text)
}

/// First [`MAX_PREVIEW_CHARS`] chars, control chars flattened to spaces,
/// with a trailing ellipsis when truncated.
pub fn preview_str(text: &str) -> String {
    let mut out = String::new();
    let mut truncated = false;
    for (i, c) in text.chars().enumerate() {
        if i >= MAX_PREVIEW_CHARS {
            truncated = true;
            break;
        }
        out.push(if c.is_control() { ' ' } else { c });
    }
    if truncated {
        out.push('…');
    }
    out
}

/// Multi-byte candidate set for the detect report. Single-byte charsets
/// (windows-1252 & friends) define all 256 byte values, so "decodes cleanly"
/// is vacuous for them — they are deliberately excluded.
const CANDIDATES: &[&Encoding] = &[
    &encoding_rs::UTF_8_INIT,
    &encoding_rs::SHIFT_JIS_INIT,
    &encoding_rs::EUC_JP_INIT,
    &encoding_rs::ISO_2022_JP_INIT,
    &encoding_rs::GBK_INIT,
    &encoding_rs::GB18030_INIT,
    &encoding_rs::BIG5_INIT,
    &encoding_rs::EUC_KR_INIT,
];

/// A detection report over raw bytes.
#[derive(Debug)]
pub struct Detection {
    /// Canonical name of the best guess (e.g. "Shift_JIS", "UTF-8", "ASCII").
    pub encoding: String,
    /// How the guess was made: "bom" | "ascii" | "valid-utf-8" | "detector".
    pub method: &'static str,
    /// BOM found at the start of the input, if any.
    pub bom: Option<&'static str>,
    pub valid_utf8: bool,
    pub ascii_only: bool,
    /// Multi-byte charsets under which the sampled bytes are entirely valid.
    pub candidates: Vec<String>,
    /// Preview of the text decoded under the guess.
    pub preview: String,
}

/// Detect the encoding of `bytes`: BOM first, then pure-ASCII / valid-UTF-8
/// shortcuts, then chardetng over the first [`DETECT_SAMPLE_BYTES`].
pub fn detect(bytes: &[u8]) -> Detection {
    let bom = sniff_bom(bytes);
    let valid_utf8 = core::str::from_utf8(bytes).is_ok();
    let ascii_only = bytes.iter().all(|&b| b < 0x80);
    let (src, method): (Src, &'static str) = match bom {
        Some(Bom::Utf8) => (Src::Rs(UTF_8), "bom"),
        Some(Bom::Utf16Le) => (Src::Rs(UTF_16LE), "bom"),
        Some(Bom::Utf16Be) => (Src::Rs(UTF_16BE), "bom"),
        Some(Bom::Utf32Le) => (Src::Utf32Le, "bom"),
        Some(Bom::Utf32Be) => (Src::Utf32Be, "bom"),
        None if ascii_only => (Src::Rs(UTF_8), "ascii"),
        None if valid_utf8 => (Src::Rs(UTF_8), "valid-utf-8"),
        None => {
            let sample = &bytes[..bytes.len().min(DETECT_SAMPLE_BYTES)];
            let mut det = EncodingDetector::new();
            det.feed(sample, sample.len() == bytes.len());
            (Src::Rs(det.guess(None, true)), "detector")
        }
    };
    let candidates = if bom.is_some() {
        vec![src.name()]
    } else {
        let sample = &bytes[..bytes.len().min(CANDIDATE_SAMPLE_BYTES)];
        let last = sample.len() == bytes.len();
        CANDIDATES
            .iter()
            .filter(|e| decodes_cleanly(e, sample, last))
            .map(|e| e.name().to_string())
            .collect()
    };
    let data = &bytes[bom.map_or(0, Bom::len)..];
    let preview = preview_of(src, data, 16 * 1024);
    Detection {
        encoding: if method == "ascii" {
            "ASCII".to_string()
        } else {
            src.name()
        },
        method,
        bom: bom.map(Bom::label),
        valid_utf8,
        ascii_only,
        candidates,
        preview,
    }
}

/// The result of a full byte-level conversion.
#[derive(Debug)]
pub struct Conversion {
    pub out: Vec<u8>,
    /// Canonical source-charset name actually used.
    pub from_name: String,
    /// "explicit" | "bom" | "ascii" | "valid-utf-8" | "detector".
    pub from_method: &'static str,
    pub to_name: String,
    /// U+FFFD substitutions made while decoding (Replace mode).
    pub replaced_decode: usize,
    /// `?` substitutions made while encoding (Replace mode).
    pub replaced_encode: usize,
    pub bom_written: bool,
    /// Label of an input BOM that was recognized and stripped.
    pub bom_stripped: Option<&'static str>,
    /// Characters in the decoded text.
    pub chars: usize,
    pub preview: String,
}

/// Convert `bytes` from charset `from` ("auto" = BOM sniff → ASCII/UTF-8
/// shortcut → chardetng) to charset `to`. Input BOMs are stripped; the output
/// carries a BOM per [`encode_to`]'s rules.
pub fn convert(
    bytes: &[u8],
    from: &str,
    to: &str,
    errors: Errors,
    bom: bool,
) -> Result<Conversion, String> {
    let sniffed = sniff_bom(bytes);
    let trimmed = from.trim();
    let auto = trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto");
    let (src, from_method): (Src, &'static str) = if auto {
        match sniffed {
            Some(Bom::Utf8) => (Src::Rs(UTF_8), "bom"),
            Some(Bom::Utf16Le) => (Src::Rs(UTF_16LE), "bom"),
            Some(Bom::Utf16Be) => (Src::Rs(UTF_16BE), "bom"),
            Some(Bom::Utf32Le) => (Src::Utf32Le, "bom"),
            Some(Bom::Utf32Be) => (Src::Utf32Be, "bom"),
            None => {
                if bytes.iter().all(|&b| b < 0x80) {
                    (Src::Rs(UTF_8), "ascii")
                } else if core::str::from_utf8(bytes).is_ok() {
                    (Src::Rs(UTF_8), "valid-utf-8")
                } else {
                    let sample = &bytes[..bytes.len().min(DETECT_SAMPLE_BYTES)];
                    let mut det = EncodingDetector::new();
                    det.feed(sample, sample.len() == bytes.len());
                    (Src::Rs(det.guess(None, true)), "detector")
                }
            }
        }
    } else {
        (resolve_explicit(trimmed, sniffed)?, "explicit")
    };
    // Strip a BOM the resolved source recognizes as its own (an explicit
    // `from` that CONTRADICTS a BOM already errored in resolve_explicit).
    let bom_stripped = sniffed.filter(|b| src.matches_bom(*b));
    let data = &bytes[bom_stripped.map_or(0, Bom::len)..];
    let (text, replaced_decode) = decode_src(src, data, errors)?;
    let encoded = encode_to(&text, to, errors, bom)?;
    let chars = text.chars().count();
    let preview = preview_str(&text);
    Ok(Conversion {
        out: encoded.out,
        from_name: src.name(),
        from_method,
        to_name: encoded.name,
        replaced_decode,
        replaced_encode: encoded.replaced,
        bom_written: encoded.bom_written,
        bom_stripped: bom_stripped.map(Bom::label),
        chars,
        preview,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // "こんにちは" in Shift_JIS.
    const SJIS_HELLO: &[u8] = &[0x82, 0xB1, 0x82, 0xF1, 0x82, 0xC9, 0x82, 0xBF, 0x82, 0xCD];
    const UTF8_HELLO: &str = "こんにちは";

    #[test]
    fn sniff_bom_variants_and_utf32_precedence() {
        assert_eq!(sniff_bom(&[0xEF, 0xBB, 0xBF, b'a']), Some(Bom::Utf8));
        assert_eq!(sniff_bom(&[0xFF, 0xFE, b'a', 0x00]), Some(Bom::Utf16Le));
        assert_eq!(sniff_bom(&[0xFE, 0xFF, 0x00, b'a']), Some(Bom::Utf16Be));
        // FF FE 00 00 must win as UTF-32LE, not UTF-16LE.
        assert_eq!(sniff_bom(&[0xFF, 0xFE, 0x00, 0x00]), Some(Bom::Utf32Le));
        assert_eq!(sniff_bom(&[0x00, 0x00, 0xFE, 0xFF]), Some(Bom::Utf32Be));
        assert_eq!(sniff_bom(b"plain"), None);
    }

    #[test]
    fn shift_jis_to_utf8_explicit_alias() {
        // "sjis" is a WHATWG alias for Shift_JIS.
        let c = convert(SJIS_HELLO, "sjis", "utf-8", Errors::Strict, false).unwrap();
        assert_eq!(c.out, UTF8_HELLO.as_bytes());
        assert_eq!(c.from_name, "Shift_JIS");
        assert_eq!(c.from_method, "explicit");
        assert_eq!(c.to_name, "UTF-8");
        assert_eq!(c.chars, 5);
        assert_eq!(c.replaced_decode + c.replaced_encode, 0);
        assert!(!c.bom_written);
    }

    #[test]
    fn utf8_to_shift_jis_roundtrip() {
        let c = convert(
            UTF8_HELLO.as_bytes(),
            "auto",
            "shift_jis",
            Errors::Strict,
            false,
        )
        .unwrap();
        assert_eq!(c.out, SJIS_HELLO);
        assert_eq!(c.from_method, "valid-utf-8");
        assert_eq!(c.to_name, "Shift_JIS");
    }

    #[test]
    fn auto_bom_utf16le_strips_bom() {
        // BOM + "hi" as UTF-16LE.
        let bytes = [0xFF, 0xFE, b'h', 0x00, b'i', 0x00];
        let c = convert(&bytes, "auto", "utf-8", Errors::Strict, false).unwrap();
        assert_eq!(c.out, b"hi");
        assert_eq!(c.from_name, "UTF-16LE");
        assert_eq!(c.from_method, "bom");
        assert_eq!(c.bom_stripped, Some("UTF-16LE"));
    }

    #[test]
    fn auto_detector_finds_shift_jis() {
        // Enough natural Japanese for chardetng to lock on. Built via our own
        // encoder, which is separately vector-tested above.
        let ja = "今日はとても良い天気ですね。明日は雨が降るかもしれません。東京の桜はもう咲きましたか。".repeat(20);
        let sjis = encode_to(&ja, "shift_jis", Errors::Strict, false)
            .unwrap()
            .out;
        let c = convert(&sjis, "auto", "utf-8", Errors::Strict, false).unwrap();
        assert_eq!(c.from_name, "Shift_JIS");
        assert_eq!(c.from_method, "detector");
        assert_eq!(c.out, ja.as_bytes());
    }

    #[test]
    fn encode_utf16le_exact_bytes_with_bom() {
        let e = encode_to("A€", "utf-16le", Errors::Strict, false).unwrap();
        assert_eq!(e.out, [0xFF, 0xFE, 0x41, 0x00, 0xAC, 0x20]);
        assert!(e.bom_written);
        assert_eq!(e.name, "UTF-16LE");
        // Astral plane goes through surrogate pairs: U+1F600 = D83D DE00.
        let e = encode_to("😀", "utf-16be", Errors::Strict, false).unwrap();
        assert_eq!(e.out, [0xFE, 0xFF, 0xD8, 0x3D, 0xDE, 0x00]);
    }

    #[test]
    fn utf8_bom_flag_prepends_bom() {
        let e = encode_to("hi", "utf-8", Errors::Strict, true).unwrap();
        assert_eq!(e.out, [0xEF, 0xBB, 0xBF, b'h', b'i']);
        assert!(e.bom_written);
        let e = encode_to("hi", "utf-8", Errors::Strict, false).unwrap();
        assert_eq!(e.out, b"hi");
    }

    #[test]
    fn unmappable_replace_and_strict() {
        // U+4E2D 中 does not exist in windows-1252.
        let e = encode_to("a中b", "windows-1252", Errors::Replace, false).unwrap();
        assert_eq!(e.out, b"a?b");
        assert_eq!(e.replaced, 1);
        let err = encode_to("a中b", "windows-1252", Errors::Strict, false).unwrap_err();
        assert!(err.contains("U+4E2D"), "{err}");
        assert!(err.contains("character index 1"), "{err}");
    }

    #[test]
    fn strict_decode_reports_offset() {
        // 0x82 is a Shift_JIS lead byte with no trail byte.
        let err =
            decode_rs(encoding_rs::SHIFT_JIS, &[b'o', b'k', 0x82], Errors::Strict).unwrap_err();
        assert!(err.contains("Shift_JIS"), "{err}");
        assert!(err.contains("offset 2"), "{err}");
        // Replace mode substitutes and counts.
        let (text, replaced) =
            decode_rs(encoding_rs::SHIFT_JIS, &[b'o', b'k', 0x82], Errors::Replace).unwrap();
        assert_eq!(text, "ok\u{FFFD}");
        assert_eq!(replaced, 1);
    }

    #[test]
    fn unknown_labels_error() {
        assert!(convert(b"x", "klingon-8", "utf-8", Errors::Replace, false)
            .unwrap_err()
            .contains("unknown source charset"));
        assert!(encode_to("x", "klingon-8", Errors::Replace, false)
            .unwrap_err()
            .contains("unknown target charset"));
    }

    #[test]
    fn bom_flag_rejected_for_legacy_target() {
        let err = encode_to("x", "shift_jis", Errors::Replace, true).unwrap_err();
        assert!(err.contains("Unicode targets"), "{err}");
    }

    #[test]
    fn utf32_target_rejected() {
        let err = encode_to("x", "utf-32le", Errors::Replace, false).unwrap_err();
        assert!(err.contains("not supported"), "{err}");
    }

    #[test]
    fn explicit_from_contradicting_bom_errors() {
        let bytes = [0xFF, 0xFE, b'h', 0x00];
        let err = convert(&bytes, "shift_jis", "utf-8", Errors::Replace, false).unwrap_err();
        assert!(err.contains("UTF-16LE byte-order mark"), "{err}");
    }

    #[test]
    fn explicit_utf8_with_utf8_bom_strips_it() {
        let bytes = [0xEF, 0xBB, 0xBF, b'h', b'i'];
        let c = convert(&bytes, "utf-8", "utf-8", Errors::Strict, false).unwrap();
        assert_eq!(c.out, b"hi");
        assert_eq!(c.bom_stripped, Some("UTF-8"));
        assert_eq!(c.from_method, "explicit");
    }

    #[test]
    fn utf32_decode_explicit_and_bom() {
        // "A" in UTF-32LE, no BOM, explicit from.
        let c = convert(&[0x41, 0, 0, 0], "utf-32le", "utf-8", Errors::Strict, false).unwrap();
        assert_eq!(c.out, b"A");
        assert_eq!(c.from_name, "UTF-32LE");
        // BOM + "A" in UTF-32LE, auto.
        let c = convert(
            &[0xFF, 0xFE, 0, 0, 0x41, 0, 0, 0],
            "auto",
            "utf-8",
            Errors::Strict,
            false,
        )
        .unwrap();
        assert_eq!(c.out, b"A");
        assert_eq!(c.from_name, "UTF-32LE");
        assert_eq!(c.from_method, "bom");
        // Bare "utf-32" without a BOM must demand an endianness.
        let err = convert(&[0x41, 0, 0, 0], "utf-32", "utf-8", Errors::Strict, false).unwrap_err();
        assert!(err.contains("endianness"), "{err}");
        // Truncated UTF-32 unit, strict.
        let err = convert(
            &[0x41, 0, 0, 0, 0x42],
            "utf-32le",
            "utf-8",
            Errors::Strict,
            false,
        )
        .unwrap_err();
        assert!(err.contains("truncated"), "{err}");
    }

    #[test]
    fn iso_2022_jp_roundtrip_exercises_stateful_encoder() {
        let e = encode_to(UTF8_HELLO, "iso-2022-jp", Errors::Strict, false).unwrap();
        // Must contain the JIS escape into two-byte mode and back to ASCII.
        assert!(e.out.starts_with(&[0x1B, 0x24, 0x42]), "{:?}", e.out);
        assert!(e.out.ends_with(&[0x1B, 0x28, 0x42]), "{:?}", e.out);
        let c = convert(&e.out, "iso-2022-jp", "utf-8", Errors::Strict, false).unwrap();
        assert_eq!(c.out, UTF8_HELLO.as_bytes());
    }

    #[test]
    fn gbk_and_big5_vectors() {
        // "中文": GBK D6D0 CEC4, Big5 A4A4 A4E5.
        let c = convert(
            &[0xD6, 0xD0, 0xCE, 0xC4],
            "gbk",
            "utf-8",
            Errors::Strict,
            false,
        )
        .unwrap();
        assert_eq!(c.out, "中文".as_bytes());
        let c = convert("中文".as_bytes(), "utf-8", "big5", Errors::Strict, false).unwrap();
        assert_eq!(c.out, [0xA4, 0xA4, 0xA4, 0xE5]);
    }

    #[test]
    fn detect_ascii_utf8_and_bom() {
        let d = detect(b"plain ascii text");
        assert_eq!(d.encoding, "ASCII");
        assert_eq!(d.method, "ascii");
        assert!(d.ascii_only && d.valid_utf8);
        assert!(d.candidates.iter().any(|c| c == "UTF-8"));

        let d = detect("héllo".as_bytes());
        assert_eq!(d.encoding, "UTF-8");
        assert_eq!(d.method, "valid-utf-8");
        assert!(!d.ascii_only && d.valid_utf8);

        let d = detect(&[0xEF, 0xBB, 0xBF, b'h', b'i']);
        assert_eq!(d.encoding, "UTF-8");
        assert_eq!(d.method, "bom");
        assert_eq!(d.bom, Some("UTF-8"));
        assert_eq!(d.candidates, vec!["UTF-8".to_string()]);
        assert_eq!(d.preview, "hi");
    }

    #[test]
    fn detect_shift_jis_statistically() {
        let ja = "今日はとても良い天気ですね。明日は雨が降るかもしれません。東京の桜はもう咲きましたか。".repeat(20);
        let sjis = encode_to(&ja, "shift_jis", Errors::Strict, false)
            .unwrap()
            .out;
        let d = detect(&sjis);
        assert_eq!(d.encoding, "Shift_JIS");
        assert_eq!(d.method, "detector");
        assert!(!d.valid_utf8);
        assert!(
            d.candidates.iter().any(|c| c == "Shift_JIS"),
            "{:?}",
            d.candidates
        );
        assert!(d.preview.starts_with("今日は"), "{}", d.preview);
    }

    #[test]
    fn detect_gbk_candidates_include_gbk() {
        let zh = "简体中文的字符编码转换测试。".repeat(6);
        let gbk = encode_to(&zh, "gbk", Errors::Strict, false).unwrap().out;
        let d = detect(&gbk);
        assert!(!d.valid_utf8);
        assert!(
            d.candidates.iter().any(|c| c == "GBK"),
            "{:?}",
            d.candidates
        );
    }

    #[test]
    fn preview_truncates_and_flattens_controls() {
        let long = "x\ty\r\n".repeat(100);
        let p = preview_str(&long);
        assert!(p.ends_with('…'));
        assert_eq!(p.chars().count(), MAX_PREVIEW_CHARS + 1);
        assert!(!p.contains('\n') && !p.contains('\t'));
    }

    #[test]
    fn errors_parse() {
        assert_eq!(Errors::parse("").unwrap(), Errors::Replace);
        assert_eq!(Errors::parse("replace").unwrap(), Errors::Replace);
        assert_eq!(Errors::parse("strict").unwrap(), Errors::Strict);
        assert!(Errors::parse("panic").is_err());
    }
}
