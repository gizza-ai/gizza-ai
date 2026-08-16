//! gizza-ai/base64-diff core — decode two Base64 / Base64url blobs and diff the
//! decoded BYTES, not the encoded text.
//!
//! Two Base64 strings can look completely different and still carry the same
//! payload (padding, line wrapping, `+/` vs `-_`), and two nearly identical
//! strings can hide a one-byte payload change. This core decodes both sides and
//! answers the byte-level question: are the payloads equal, and if not, at which
//! offsets do they differ?
//!
//! Pure compute, no I/O. Options:
//! * `alphabet`: `auto` (default, detect per side) | `standard` (`+/`) | `url` (`-_`).
//! * `strict`: reject whitespace / non-canonical padding instead of repairing (default false).
//! * `align`: `offset` (default, byte i vs byte i) | `shift` (trim the common prefix and
//!   suffix, so one insertion is reported as an insertion instead of "everything after").
//! * `output`: `report` JSON (default) | `summary` | `hexdump` | `text`.
//! * `bytes_per_row`: hexdump width, 4-32 (default 8).
//! * `context_rows`: identical hexdump rows / text-diff lines kept around a change (default 2).

use base64::{
    alphabet,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
    DecodeError, Engine,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Longest accepted Base64 input per side, in bytes (~3 MiB decoded).
pub const MAX_INPUT_CHARS: usize = 4 * 1024 * 1024;
/// Most difference ranges listed before the rest are summarised as a count.
const MAX_RANGES: usize = 200;
/// Longest byte run shown inline for a single range.
const MAX_RANGE_BYTES_SHOWN: usize = 32;
/// Most hexdump rows rendered before the dump is truncated.
const MAX_DUMP_ROWS: usize = 4096;
/// Longest decoded-text preview in the JSON report, in characters.
const TEXT_PREVIEW_CHARS: usize = 60;
/// Largest line-diff table computed exactly before falling back to a positional diff.
const LCS_CELL_CAP: usize = 1_000_000;

/// Which Base64 alphabet to decode with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Alphabet {
    Auto,
    Standard,
    UrlSafe,
}

/// How the two byte strings are lined up before comparing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Offset,
    Shift,
}

/// Shape of the returned result.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    Report,
    Summary,
    Hexdump,
    Text,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub alphabet: Alphabet,
    pub strict: bool,
    pub align: Align,
    pub output: Output,
    pub bytes_per_row: usize,
    pub context_rows: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            alphabet: Alphabet::Auto,
            strict: false,
            align: Align::Offset,
            output: Output::Report,
            bytes_per_row: 8,
            context_rows: 2,
        }
    }
}

pub fn parse_alphabet(s: &str) -> Result<Alphabet, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(Alphabet::Auto),
        "standard" | "std" => Ok(Alphabet::Standard),
        "url" | "url-safe" | "urlsafe" | "base64url" => Ok(Alphabet::UrlSafe),
        other => Err(format!(
            "unknown alphabet \"{other}\" — expected one of: auto, standard, url"
        )),
    }
}

pub fn parse_align(s: &str) -> Result<Align, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "offset" => Ok(Align::Offset),
        "shift" => Ok(Align::Shift),
        other => Err(format!(
            "unknown align \"{other}\" — expected one of: offset, shift"
        )),
    }
}

pub fn parse_output(s: &str) -> Result<Output, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "report" => Ok(Output::Report),
        "summary" => Ok(Output::Summary),
        "hexdump" | "hex" => Ok(Output::Hexdump),
        "text" => Ok(Output::Text),
        other => Err(format!(
            "unknown output \"{other}\" — expected one of: report, summary, hexdump, text"
        )),
    }
}

fn parse_bool(s: &str, field: &str) -> Result<bool, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "false" | "0" | "no" | "off" => Ok(false),
        "true" | "1" | "yes" | "on" => Ok(true),
        other => Err(format!(
            "{field} must be true or false, got \"{other}\""
        )),
    }
}

fn parse_usize(s: &str, field: &str, min: usize, max: usize, default: usize) -> Result<usize, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    let n: f64 = t
        .parse()
        .map_err(|_| format!("{field} must be a whole number between {min} and {max}, got \"{t}\""))?;
    if !n.is_finite() || n.fract() != 0.0 {
        return Err(format!(
            "{field} must be a whole number between {min} and {max}, got \"{t}\""
        ));
    }
    let n = n as i64;
    if n < min as i64 || n > max as i64 {
        return Err(format!("{field} must be between {min} and {max}, got {n}"));
    }
    Ok(n as usize)
}

/// Build `Options` from the page's string-valued fields (every page field arrives as text).
#[allow(clippy::too_many_arguments)]
pub fn options_from_strings(
    alphabet: &str,
    strict: &str,
    align: &str,
    output: &str,
    bytes_per_row: &str,
    context_rows: &str,
) -> Result<Options, String> {
    Ok(Options {
        alphabet: parse_alphabet(alphabet)?,
        strict: parse_bool(strict, "strict")?,
        align: parse_align(align)?,
        output: parse_output(output)?,
        bytes_per_row: parse_usize(bytes_per_row, "bytes_per_row", 4, 32, 8)?,
        context_rows: parse_usize(context_rows, "context_rows", 0, 64, 2)?,
    })
}

/// One decoded side of the comparison.
#[derive(Clone, Debug)]
pub struct Payload {
    pub bytes: Vec<u8>,
    /// Alphabet actually used: "standard", "url-safe" or "either" (no distinguishing characters).
    pub alphabet: &'static str,
    /// Base64 characters after cleaning (padding included).
    pub chars: usize,
    /// "canonical", "missing", "non-canonical" or "not required".
    pub padding: &'static str,
    pub whitespace_removed: usize,
    pub data_uri: bool,
}

fn engine_for(a: Alphabet, strict: bool) -> GeneralPurpose {
    let cfg = if strict {
        GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::RequireCanonical)
    } else {
        GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::Indifferent)
            .with_decode_allow_trailing_bits(true)
    };
    let alpha = match a {
        Alphabet::UrlSafe => &alphabet::URL_SAFE,
        _ => &alphabet::STANDARD,
    };
    GeneralPurpose::new(alpha, cfg)
}

fn describe_decode_error(side: &str, e: DecodeError) -> String {
    match e {
        DecodeError::InvalidByte(idx, b) => {
            let shown = if b.is_ascii_graphic() {
                format!("'{}'", b as char)
            } else {
                format!("0x{b:02x}")
            };
            format!(
                "{side}: invalid Base64 character {shown} at position {idx} — the input is not valid Base64 (turn off strict mode to ignore whitespace and repair padding, or switch the alphabet)"
            )
        }
        DecodeError::InvalidLength(idx) => format!(
            "{side}: truncated Base64 — the data ends mid-group at position {idx} (a Base64 string needs 4 characters per 3 bytes)"
        ),
        DecodeError::InvalidLastSymbol(idx, b) => format!(
            "{side}: non-canonical final character '{}' at position {idx} — its unused trailing bits are not zero (turn off strict mode to accept it)",
            b as char
        ),
        DecodeError::InvalidPadding => format!(
            "{side}: wrong '=' padding — expected the string length to be a multiple of 4 with 0-2 trailing '=' (turn off strict mode to repair it)"
        ),
    }
}

fn strip_data_uri(s: &str) -> (&str, bool) {
    let t = s.trim();
    if t.len() > 5 && t[..5].eq_ignore_ascii_case("data:") {
        if let Some(pos) = t.to_ascii_lowercase().find(";base64,") {
            return (&t[pos + ";base64,".len()..], true);
        }
    }
    (t, false)
}

/// Decode one side, reporting exactly what was wrong when it fails.
pub fn decode_side(raw: &str, side: &str, opts: &Options) -> Result<Payload, String> {
    if raw.len() > MAX_INPUT_CHARS {
        return Err(format!(
            "{side}: input is {} characters, over the {} character limit (~3 MiB of decoded payload)",
            raw.len(),
            MAX_INPUT_CHARS
        ));
    }
    let (body, data_uri) = if opts.strict {
        (raw.trim(), false)
    } else {
        strip_data_uri(raw)
    };
    if body.trim().is_empty() {
        return Err(format!("{side}: no Base64 data — paste a Base64 or Base64url string"));
    }
    let cleaned: String = if opts.strict {
        body.to_string()
    } else {
        body.chars().filter(|c| !c.is_ascii_whitespace()).collect()
    };
    let whitespace_removed = body.chars().filter(|c| c.is_ascii_whitespace()).count();

    let has_std = cleaned.contains('+') || cleaned.contains('/');
    let has_url = cleaned.contains('-') || cleaned.contains('_');
    let (chosen, label) = match opts.alphabet {
        Alphabet::Standard => (Alphabet::Standard, "standard"),
        Alphabet::UrlSafe => (Alphabet::UrlSafe, "url-safe"),
        Alphabet::Auto => {
            if has_std && has_url {
                return Err(format!(
                    "{side}: mixed Base64 alphabets — the data contains both '+'/'/' (standard) and '-'/'_' (URL-safe); pick one alphabet explicitly"
                ));
            } else if has_url {
                (Alphabet::UrlSafe, "url-safe")
            } else if has_std {
                (Alphabet::Standard, "standard")
            } else {
                (Alphabet::Standard, "either")
            }
        }
    };

    let body_len = cleaned.trim_end_matches('=').len();
    let pads = cleaned.len() - body_len;
    let expected = (4 - body_len % 4) % 4;
    let padding = if expected == 0 && pads == 0 {
        "not required"
    } else if pads == expected {
        "canonical"
    } else if pads == 0 {
        "missing"
    } else {
        "non-canonical"
    };

    let bytes = engine_for(chosen, opts.strict)
        .decode(cleaned.as_bytes())
        .map_err(|e| describe_decode_error(side, e))?;

    Ok(Payload {
        bytes,
        alphabet: label,
        chars: cleaned.chars().count(),
        padding,
        whitespace_removed,
        data_uri,
    })
}

/// One contiguous difference between the two payloads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Range {
    pub offset: usize,
    pub length: usize,
    /// "changed", "added" (only on the right) or "removed" (only on the left).
    pub kind: &'static str,
    pub left: Vec<u8>,
    pub right: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct ByteDiff {
    pub equal: bool,
    pub ranges: Vec<Range>,
    pub truncated_ranges: bool,
    pub differing_bytes: usize,
    pub first_difference: Option<usize>,
    pub common_prefix: usize,
    pub common_suffix: usize,
}

fn common_prefix_len(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count()
}

/// Compare two byte strings under the chosen alignment.
pub fn diff_bytes(l: &[u8], r: &[u8], align: Align) -> ByteDiff {
    let prefix = common_prefix_len(l, r);
    let suffix_room = l.len().min(r.len()) - prefix;
    let suffix = l
        .iter()
        .rev()
        .zip(r.iter().rev())
        .take(suffix_room)
        .take_while(|(x, y)| x == y)
        .count();

    let mut ranges: Vec<Range> = Vec::new();
    let mut differing_bytes = 0usize;

    match align {
        Align::Offset => {
            let n = l.len().min(r.len());
            let mut i = 0usize;
            while i < n {
                if l[i] != r[i] {
                    let start = i;
                    while i < n && l[i] != r[i] {
                        i += 1;
                    }
                    differing_bytes += i - start;
                    if ranges.len() < MAX_RANGES {
                        ranges.push(Range {
                            offset: start,
                            length: i - start,
                            kind: "changed",
                            left: l[start..i].to_vec(),
                            right: r[start..i].to_vec(),
                        });
                    }
                } else {
                    i += 1;
                }
            }
            if l.len() > n && ranges.len() < MAX_RANGES {
                ranges.push(Range {
                    offset: n,
                    length: l.len() - n,
                    kind: "removed",
                    left: l[n..].to_vec(),
                    right: Vec::new(),
                });
            } else if r.len() > n && ranges.len() < MAX_RANGES {
                ranges.push(Range {
                    offset: n,
                    length: r.len() - n,
                    kind: "added",
                    left: Vec::new(),
                    right: r[n..].to_vec(),
                });
            }
        }
        Align::Shift => {
            let lm = &l[prefix..l.len() - suffix];
            let rm = &r[prefix..r.len() - suffix];
            if !lm.is_empty() || !rm.is_empty() {
                let kind = if lm.is_empty() {
                    "added"
                } else if rm.is_empty() {
                    "removed"
                } else {
                    "changed"
                };
                differing_bytes = lm.len().max(rm.len());
                ranges.push(Range {
                    offset: prefix,
                    length: lm.len().max(rm.len()),
                    kind,
                    left: lm.to_vec(),
                    right: rm.to_vec(),
                });
            }
        }
    }

    ByteDiff {
        equal: l == r,
        first_difference: ranges.first().map(|r| r.offset),
        truncated_ranges: ranges.len() >= MAX_RANGES,
        differing_bytes,
        ranges,
        common_prefix: prefix,
        common_suffix: suffix,
    }
}

fn hex_of(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn ascii_of(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if (0x20..0x7f).contains(&b) {
                b as char
            } else {
                '.'
            }
        })
        .collect()
}

fn shown(bytes: &[u8]) -> (String, String, bool) {
    let cut = bytes.len() > MAX_RANGE_BYTES_SHOWN;
    let head = &bytes[..bytes.len().min(MAX_RANGE_BYTES_SHOWN)];
    (hex_of(head), ascii_of(head), cut)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Best-effort magic-byte sniff, so a payload's kind is obvious without decoding it by hand.
pub fn detect_type(bytes: &[u8]) -> &'static str {
    const MAGIC: &[(&[u8], &str)] = &[
        (b"\x89PNG\r\n\x1a\n", "PNG image"),
        (b"\xff\xd8\xff", "JPEG image"),
        (b"GIF87a", "GIF image"),
        (b"GIF89a", "GIF image"),
        (b"%PDF-", "PDF document"),
        (b"PK\x03\x04", "ZIP archive (also docx/xlsx/jar)"),
        (b"\x1f\x8b", "gzip data"),
        (b"BZh", "bzip2 data"),
        (b"7z\xbc\xaf\x27\x1c", "7-Zip archive"),
        (b"\x7fELF", "ELF binary"),
        (b"\0asm", "WebAssembly module"),
        (b"RIFF", "RIFF container (WAV/AVI/WebP)"),
        (b"OggS", "Ogg container"),
        (b"ID3", "MP3 audio (ID3)"),
        (b"\xca\xfe\xba\xbe", "Java class file"),
        (b"SQLite format 3\0", "SQLite database"),
        (b"-----BEGIN ", "PEM block"),
        (b"\x30\x82", "DER / ASN.1 sequence"),
    ];
    if bytes.is_empty() {
        return "empty";
    }
    for (sig, name) in MAGIC {
        if bytes.starts_with(sig) {
            return name;
        }
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => {
            let t = s.trim_start();
            if t.starts_with('{') || t.starts_with('[') {
                "JSON-like text"
            } else {
                "UTF-8 text"
            }
        }
        Err(_) => "binary data",
    }
}

fn text_preview(bytes: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(bytes).ok()?;
    let mut out: String = s
        .chars()
        .take(TEXT_PREVIEW_CHARS)
        .map(|c| if c.is_control() { '·' } else { c })
        .collect();
    if s.chars().count() > TEXT_PREVIEW_CHARS {
        out.push('…');
    }
    Some(out)
}

fn side_json(p: &Payload) -> Value {
    let mut v = json!({
        "alphabet": p.alphabet,
        "base64_chars": p.chars,
        "padding": p.padding,
        "bytes": p.bytes.len(),
        "sha256": sha256_hex(&p.bytes),
        "detected_type": detect_type(&p.bytes),
        "utf8": std::str::from_utf8(&p.bytes).is_ok(),
    });
    if let Some(t) = text_preview(&p.bytes) {
        v["text_preview"] = Value::String(t);
    }
    if p.whitespace_removed > 0 {
        v["whitespace_removed"] = json!(p.whitespace_removed);
    }
    if p.data_uri {
        v["data_uri_prefix_stripped"] = json!(true);
    }
    v
}

fn encoding_notes(left_raw: &str, right_raw: &str, l: &Payload, r: &Payload, equal: bool) -> Vec<String> {
    let mut notes = Vec::new();
    if equal && left_raw != right_raw {
        let mut why = Vec::new();
        if l.alphabet != r.alphabet {
            why.push("different alphabets");
        }
        if l.padding != r.padding {
            why.push("different padding");
        }
        if l.whitespace_removed != r.whitespace_removed {
            why.push("different line wrapping/whitespace");
        }
        if l.data_uri != r.data_uri {
            why.push("one side is a data: URI");
        }
        let reason = if why.is_empty() {
            "the encoded text differs".to_string()
        } else {
            why.join(", ")
        };
        notes.push(format!(
            "The two Base64 strings are not identical ({reason}), but they decode to the same bytes."
        ));
    }
    if l.alphabet != r.alphabet && l.alphabet != "either" && r.alphabet != "either" {
        notes.push(format!(
            "Left decoded as {} Base64, right as {}.",
            l.alphabet, r.alphabet
        ));
    }
    notes
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

fn headline(l: &Payload, r: &Payload, d: &ByteDiff) -> String {
    if d.equal {
        return format!(
            "Payloads are identical: {} ({}), sha256 {}.",
            plural(l.bytes.len(), "byte", "bytes"),
            detect_type(&l.bytes),
            sha256_hex(&l.bytes)
        );
    }
    let delta = r.bytes.len() as i64 - l.bytes.len() as i64;
    let size = if delta == 0 {
        format!("both {}", plural(l.bytes.len(), "byte", "bytes"))
    } else {
        format!(
            "left {} bytes, right {} bytes ({}{})",
            l.bytes.len(),
            r.bytes.len(),
            if delta > 0 { "+" } else { "" },
            delta
        )
    };
    let first = d
        .first_difference
        .map(|o| format!(" First difference at offset 0x{o:04x} ({o})."))
        .unwrap_or_default();
    format!("Payloads differ: {size}.{first}")
}

fn range_line(rg: &Range) -> String {
    let (hex, txt, cut) = match rg.kind {
        "added" => shown(&rg.right),
        "removed" => shown(&rg.left),
        _ => shown(&rg.left),
    };
    let ell = if cut { " …" } else { "" };
    match rg.kind {
        "changed" => {
            let (rhex, rtxt, rcut) = shown(&rg.right);
            let rell = if rcut { " …" } else { "" };
            format!(
                "@ 0x{:04x} ({}) changed: {}{} |{}| -> {}{} |{}|",
                rg.offset,
                plural(rg.length, "byte", "bytes"),
                hex,
                ell,
                txt,
                rhex,
                rell,
                rtxt
            )
        }
        "added" => format!(
            "@ 0x{:04x} ({}) added on the right: {}{} |{}|",
            rg.offset,
            plural(rg.length, "byte", "bytes"),
            hex,
            ell,
            txt
        ),
        _ => format!(
            "@ 0x{:04x} ({}) removed from the left: {}{} |{}|",
            rg.offset,
            plural(rg.length, "byte", "bytes"),
            hex,
            ell,
            txt
        ),
    }
}

fn render_summary(l: &Payload, r: &Payload, d: &ByteDiff, notes: &[String]) -> String {
    let mut out = vec![headline(l, r, d)];
    if !d.equal {
        out.push(format!(
            "{} {} across {}.",
            plural(d.differing_bytes, "byte", "bytes"),
            if d.differing_bytes == 1 { "differs" } else { "differ" },
            plural(d.ranges.len(), "range", "ranges")
        ));
    }
    for n in notes {
        out.push(format!("Note: {n}"));
    }
    for rg in &d.ranges {
        out.push(range_line(rg));
    }
    if d.truncated_ranges {
        out.push(format!(
            "… range list truncated at {MAX_RANGES} entries."
        ));
    }
    out.join("\n")
}

fn render_hexdump(l: &Payload, r: &Payload, d: &ByteDiff, notes: &[String], opts: &Options) -> String {
    let w = opts.bytes_per_row;
    let lb = &l.bytes;
    let rb = &r.bytes;
    let total = lb.len().max(rb.len());
    let rows = total.div_ceil(w).max(1);

    let row_differs = |i: usize| -> bool {
        let start = i * w;
        (start..(start + w).min(total)).any(|k| lb.get(k) != rb.get(k))
    };
    let differing: Vec<bool> = (0..rows).map(row_differs).collect();
    let keep: Vec<bool> = (0..rows)
        .map(|i| {
            if d.equal {
                true
            } else {
                (i.saturating_sub(opts.context_rows)..=(i + opts.context_rows).min(rows - 1))
                    .any(|j| differing[j])
            }
        })
        .collect();

    let cell = |bytes: &[u8], start: usize| -> (String, String) {
        let mut hex = String::new();
        let mut txt = String::new();
        for k in 0..w {
            if k > 0 {
                hex.push(' ');
            }
            match bytes.get(start + k) {
                Some(&b) => {
                    hex.push_str(&format!("{b:02x}"));
                    txt.push(if (0x20..0x7f).contains(&b) { b as char } else { '.' });
                }
                None => {
                    hex.push_str("  ");
                    txt.push(' ');
                }
            }
        }
        (hex, txt)
    };

    let hex_w = w * 3 - 1;
    let mut out = vec![headline(l, r, d)];
    for n in notes {
        out.push(format!("Note: {n}"));
    }
    out.push(String::new());
    out.push(
        format!(
            "{:<9} {:<hex_w$}  {:<ascii_w$}  {}",
            "offset",
            "left",
            "",
            "right",
            hex_w = hex_w,
            ascii_w = w + 2
        )
        .trim_end()
        .to_string(),
    );

    let mut skipped = 0usize;
    let mut rendered = 0usize;
    for i in 0..rows {
        if !keep[i] {
            skipped += 1;
            continue;
        }
        if skipped > 0 {
            out.push(format!("           … {} identical rows skipped …", skipped));
            skipped = 0;
        }
        if rendered >= MAX_DUMP_ROWS {
            out.push(format!(
                "           … dump truncated at {MAX_DUMP_ROWS} rows …"
            ));
            break;
        }
        let start = i * w;
        let (lh, lt) = cell(lb, start);
        let (rh, rt) = cell(rb, start);
        out.push(format!(
            "{:08x}{} {}  |{}|  {}  |{}|",
            start,
            if differing[i] { "*" } else { " " },
            lh,
            lt,
            rh,
            rt
        ));
        rendered += 1;
    }
    if skipped > 0 {
        out.push(format!("           … {} identical rows skipped …", skipped));
    }
    if !d.equal {
        out.push(String::new());
        out.push("* marks a row containing at least one differing byte.".to_string());
    }
    out.join("\n")
}

/// One rendered line of the decoded-text diff.
struct DLine {
    tag: char,
    text: String,
    a_no: usize,
    b_no: usize,
}

enum Op {
    Eq,
    Del,
    Ins,
}

fn line_ops(a: &[&str], b: &[&str]) -> Vec<Op> {
    if a.len().saturating_mul(b.len()) > LCS_CELL_CAP {
        // Too big for an exact LCS — fall back to a positional comparison.
        let mut ops = Vec::new();
        for i in 0..a.len().max(b.len()) {
            match (a.get(i), b.get(i)) {
                (Some(x), Some(y)) if x == y => ops.push(Op::Eq),
                (Some(_), Some(_)) => {
                    ops.push(Op::Del);
                    ops.push(Op::Ins);
                }
                (Some(_), None) => ops.push(Op::Del),
                (None, Some(_)) => ops.push(Op::Ins),
                (None, None) => {}
            }
        }
        return ops;
    }
    let (n, m) = (a.len(), b.len());
    let mut dp = vec![0u32; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i * (m + 1) + j] = if a[i] == b[j] {
                dp[(i + 1) * (m + 1) + j + 1] + 1
            } else {
                dp[(i + 1) * (m + 1) + j].max(dp[i * (m + 1) + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0usize, 0usize);
    let mut ops = Vec::new();
    while i < n && j < m {
        if a[i] == b[j] {
            ops.push(Op::Eq);
            i += 1;
            j += 1;
        } else if dp[(i + 1) * (m + 1) + j] >= dp[i * (m + 1) + j + 1] {
            ops.push(Op::Del);
            i += 1;
        } else {
            ops.push(Op::Ins);
            j += 1;
        }
    }
    while i < n {
        ops.push(Op::Del);
        i += 1;
    }
    while j < m {
        ops.push(Op::Ins);
        j += 1;
    }
    ops
}

fn render_text(l: &Payload, r: &Payload, notes: &[String], opts: &Options) -> Result<String, String> {
    let lt = std::str::from_utf8(&l.bytes).map_err(|e| {
        format!(
            "output=text needs both payloads to be UTF-8 text, but the left payload is binary (invalid byte at offset {}) — use output=hexdump or output=report instead",
            e.valid_up_to()
        )
    })?;
    let rt = std::str::from_utf8(&r.bytes).map_err(|e| {
        format!(
            "output=text needs both payloads to be UTF-8 text, but the right payload is binary (invalid byte at offset {}) — use output=hexdump or output=report instead",
            e.valid_up_to()
        )
    })?;
    let a: Vec<&str> = lt.lines().collect();
    let b: Vec<&str> = rt.lines().collect();
    if lt == rt {
        return Ok(format!(
            "The decoded payloads are identical: {}, {}.{}",
            plural(a.len(), "line", "lines"),
            plural(l.bytes.len(), "byte", "bytes"),
            if notes.is_empty() {
                String::new()
            } else {
                format!("\nNote: {}", notes.join("\nNote: "))
            }
        ));
    }

    let ops = line_ops(&a, &b);
    let mut lines: Vec<DLine> = Vec::new();
    let (mut ai, mut bi) = (0usize, 0usize);
    for op in &ops {
        match op {
            Op::Eq => {
                lines.push(DLine { tag: ' ', text: a[ai].to_string(), a_no: ai + 1, b_no: bi + 1 });
                ai += 1;
                bi += 1;
            }
            Op::Del => {
                lines.push(DLine { tag: '-', text: a[ai].to_string(), a_no: ai + 1, b_no: bi });
                ai += 1;
            }
            Op::Ins => {
                lines.push(DLine { tag: '+', text: b[bi].to_string(), a_no: ai, b_no: bi + 1 });
                bi += 1;
            }
        }
    }

    let ctx = opts.context_rows;
    let changed: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.tag != ' ')
        .map(|(i, _)| i)
        .collect();

    let mut out = vec![
        format!("--- left  ({})", plural(a.len(), "line", "lines")),
        format!("+++ right ({})", plural(b.len(), "line", "lines")),
    ];
    for n in notes {
        out.push(format!("# note: {n}"));
    }

    let mut idx = 0usize;
    while idx < changed.len() {
        let first = changed[idx];
        let mut last = first;
        while idx + 1 < changed.len() && changed[idx + 1] <= last + 2 * ctx + 1 {
            idx += 1;
            last = changed[idx];
        }
        idx += 1;
        let start = first.saturating_sub(ctx);
        let end = (last + ctx + 1).min(lines.len());
        let slice = &lines[start..end];
        let a_count = slice.iter().filter(|l| l.tag != '+').count();
        let b_count = slice.iter().filter(|l| l.tag != '-').count();
        let a_start = slice
            .iter()
            .find(|l| l.tag != '+')
            .map(|l| l.a_no)
            .unwrap_or_else(|| slice[0].a_no);
        let b_start = slice
            .iter()
            .find(|l| l.tag != '-')
            .map(|l| l.b_no)
            .unwrap_or_else(|| slice[0].b_no);
        out.push(format!(
            "@@ -{},{} +{},{} @@",
            a_start, a_count, b_start, b_count
        ));
        for l in slice {
            out.push(format!("{}{}", l.tag, l.text));
        }
    }
    Ok(out.join("\n"))
}

fn render_report(
    l: &Payload,
    r: &Payload,
    d: &ByteDiff,
    notes: &[String],
    identical_encoding: bool,
    opts: &Options,
) -> String {
    let ranges: Vec<Value> = d
        .ranges
        .iter()
        .map(|rg| {
            let mut v = json!({
                "offset": rg.offset,
                "length": rg.length,
                "kind": rg.kind,
            });
            if !rg.left.is_empty() {
                let (hex, txt, cut) = shown(&rg.left);
                v["left_hex"] = Value::String(hex);
                v["left_text"] = Value::String(txt);
                if cut {
                    v["left_truncated"] = json!(true);
                }
            }
            if !rg.right.is_empty() {
                let (hex, txt, cut) = shown(&rg.right);
                v["right_hex"] = Value::String(hex);
                v["right_text"] = Value::String(txt);
                if cut {
                    v["right_truncated"] = json!(true);
                }
            }
            v
        })
        .collect();

    let report = json!({
        "equal": d.equal,
        "identical_encoding": identical_encoding,
        "notes": notes,
        "left": side_json(l),
        "right": side_json(r),
        "diff": {
            "align": if opts.align == Align::Shift { "shift" } else { "offset" },
            "first_difference_offset": d.first_difference,
            "differing_bytes": d.differing_bytes,
            "size_delta": r.bytes.len() as i64 - l.bytes.len() as i64,
            "common_prefix_bytes": d.common_prefix,
            "common_suffix_bytes": d.common_suffix,
            "range_count": d.ranges.len(),
            "truncated": d.truncated_ranges,
            "ranges": ranges,
        }
    });
    serde_json::to_string_pretty(&report).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

/// Decode both Base64 inputs and render the byte-level diff in the requested shape.
pub fn diff_base64(left: &str, right: &str, opts: &Options) -> Result<String, String> {
    if opts.bytes_per_row < 4 || opts.bytes_per_row > 32 {
        return Err(format!(
            "bytes_per_row must be between 4 and 32, got {}",
            opts.bytes_per_row
        ));
    }
    if opts.context_rows > 64 {
        return Err(format!(
            "context_rows must be between 0 and 64, got {}",
            opts.context_rows
        ));
    }
    let l = decode_side(left, "left", opts)?;
    let r = decode_side(right, "right", opts)?;
    let d = diff_bytes(&l.bytes, &r.bytes, opts.align);
    let notes = encoding_notes(left, right, &l, &r, d.equal);
    Ok(match opts.output {
        Output::Report => render_report(&l, &r, &d, &notes, left == right, opts),
        Output::Summary => render_summary(&l, &r, &d, &notes),
        Output::Hexdump => render_hexdump(&l, &r, &d, &notes, opts),
        Output::Text => render_text(&l, &r, &notes, opts)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(output: Output) -> Options {
        Options { output, ..Default::default() }
    }

    // "Hello world!" vs "Hello World!"
    const HELLO_LOWER: &str = "SGVsbG8gd29ybGQh";
    const HELLO_UPPER: &str = "SGVsbG8gV29ybGQh";

    #[test]
    fn summary_reports_the_changed_byte() {
        let out = diff_base64(HELLO_LOWER, HELLO_UPPER, &opts(Output::Summary)).unwrap();
        assert_eq!(
            out,
            "Payloads differ: both 12 bytes. First difference at offset 0x0006 (6).\n\
             1 byte differs across 1 range.\n\
             @ 0x0006 (1 byte) changed: 77 |w| -> 57 |W|"
        );
    }

    #[test]
    fn different_encodings_of_the_same_bytes_are_equal() {
        // Standard vs URL-safe encoding of the same three bytes (0xfb 0xff 0xbf).
        let out = diff_base64("+/+/", "-_-_", &opts(Output::Summary)).unwrap();
        assert!(out.starts_with("Payloads are identical: 3 bytes"), "got {out}");
        assert!(out.contains("decode to the same bytes"), "got {out}");
    }

    #[test]
    fn whitespace_and_missing_padding_are_repaired_by_default() {
        let wrapped = "SGVsbG8g\nd29ybGQh\n";
        let out = diff_base64(wrapped, HELLO_LOWER, &opts(Output::Summary)).unwrap();
        assert!(out.starts_with("Payloads are identical: 12 bytes"), "got {out}");
        // Unpadded input still decodes when strict is off.
        let out = diff_base64("SGk", "SGk=", &opts(Output::Summary)).unwrap();
        assert!(out.starts_with("Payloads are identical: 2 bytes"), "got {out}");
    }

    #[test]
    fn strict_mode_rejects_missing_padding() {
        let o = Options { strict: true, ..opts(Output::Summary) };
        let err = diff_base64("SGk", "SGk=", &o).unwrap_err();
        assert!(err.starts_with("left: "), "got {err}");
        assert!(err.contains("padding") || err.contains("mid-group"), "got {err}");
    }

    #[test]
    fn invalid_base64_names_the_side_and_position() {
        let err = diff_base64("SGVsbG8#", HELLO_LOWER, &opts(Output::Summary)).unwrap_err();
        assert!(err.contains("left: invalid Base64 character '#' at position 7"), "got {err}");
    }

    #[test]
    fn mixed_alphabets_are_rejected_in_auto_mode() {
        let err = diff_base64("ab+c-d", HELLO_LOWER, &opts(Output::Summary)).unwrap_err();
        assert!(err.contains("mixed Base64 alphabets"), "got {err}");
    }

    #[test]
    fn offset_alignment_cascades_where_shift_reports_one_insertion() {
        // "Hello world!" vs "Hello, world!" — one inserted byte at offset 5.
        let left = HELLO_LOWER;
        let right = "SGVsbG8sIHdvcmxkIQ==";
        let offset = diff_base64(left, right, &opts(Output::Summary)).unwrap();
        assert!(offset.contains("left 12 bytes, right 13 bytes (+1)"), "got {offset}");
        assert!(offset.contains("@ 0x0005 (7 bytes) changed"), "got {offset}");

        let o = Options { align: Align::Shift, ..opts(Output::Summary) };
        let shift = diff_base64(left, right, &o).unwrap();
        assert!(
            shift.contains("@ 0x0005 (1 byte) added on the right: 2c |,|"),
            "got {shift}"
        );
    }

    #[test]
    fn report_carries_sizes_hashes_and_ranges() {
        let out = diff_base64(HELLO_LOWER, HELLO_UPPER, &opts(Output::Report)).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["equal"], json!(false));
        assert_eq!(v["left"]["bytes"], json!(12));
        assert_eq!(v["left"]["detected_type"], json!("UTF-8 text"));
        assert_eq!(v["left"]["alphabet"], json!("either"));
        assert_eq!(v["diff"]["first_difference_offset"], json!(6));
        assert_eq!(v["diff"]["common_prefix_bytes"], json!(6));
        assert_eq!(v["diff"]["ranges"][0]["left_hex"], json!("77"));
        assert_eq!(v["diff"]["ranges"][0]["right_text"], json!("W"));
        assert_eq!(
            v["left"]["sha256"],
            json!("c0535e4be2b79ffd93291305436bf889314e4a3faec05ecffcbb7df31ad9e51a")
        );
    }

    #[test]
    fn hexdump_marks_the_differing_row() {
        let out = diff_base64(HELLO_LOWER, HELLO_UPPER, &opts(Output::Hexdump)).unwrap();
        assert!(out.contains("00000000* 48 65 6c 6c 6f 20 77 6f  |Hello wo|"), "got {out}");
        assert!(out.contains("00000008  72 6c 64 21              |rld!    |"), "got {out}");
        assert!(out.contains("* marks a row"), "got {out}");
        let starred: Vec<&str> = out.lines().filter(|l| l.starts_with("00000000*")).collect();
        assert_eq!(starred.len(), 1, "got {out}");
    }

    #[test]
    fn hexdump_hides_identical_rows_outside_the_context_window() {
        let payload: Vec<u8> = (0..200u32).map(|i| (i % 251) as u8).collect();
        let mut other = payload.clone();
        other[190] ^= 0xff;
        let l = base64::engine::general_purpose::STANDARD.encode(&payload);
        let r = base64::engine::general_purpose::STANDARD.encode(&other);
        let o = Options { context_rows: 1, ..opts(Output::Hexdump) };
        let out = diff_base64(&l, &r, &o).unwrap();
        assert!(out.contains("identical rows skipped"), "got {out}");
        assert!(out.contains("000000b8*"), "got {out}");
        // context_rows = 1 keeps exactly one identical row on each side of the change.
        assert!(out.contains("000000b0  "), "got {out}");
        assert!(!out.contains("000000a8  "), "got {out}");
    }

    #[test]
    fn text_output_produces_a_unified_line_diff() {
        let l = base64::engine::general_purpose::STANDARD.encode("alpha\nbeta\ngamma\n");
        let r = base64::engine::general_purpose::STANDARD.encode("alpha\nBETA\ngamma\n");
        let out = diff_base64(&l, &r, &opts(Output::Text)).unwrap();
        assert_eq!(
            out,
            "--- left  (3 lines)\n+++ right (3 lines)\n@@ -1,3 +1,3 @@\n alpha\n-beta\n+BETA\n gamma"
        );
    }

    #[test]
    fn text_output_refuses_binary_payloads() {
        let l = base64::engine::general_purpose::STANDARD.encode([0xff, 0xfe, 0x00]);
        let err = diff_base64(&l, &l, &opts(Output::Text)).unwrap_err();
        assert!(err.contains("left payload is binary"), "got {err}");
        assert!(err.contains("output=hexdump"), "got {err}");
    }

    #[test]
    fn data_uri_prefix_is_stripped_when_not_strict() {
        let out = diff_base64(
            "data:text/plain;base64,SGVsbG8gd29ybGQh",
            HELLO_LOWER,
            &opts(Output::Report),
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["equal"], json!(true));
        assert_eq!(v["left"]["data_uri_prefix_stripped"], json!(true));
    }

    #[test]
    fn detects_common_payload_types() {
        assert_eq!(detect_type(b"\x89PNG\r\n\x1a\n\x00"), "PNG image");
        assert_eq!(detect_type(b"%PDF-1.7"), "PDF document");
        assert_eq!(detect_type(b"{\"a\":1}"), "JSON-like text");
        assert_eq!(detect_type(&[0xff, 0xfe, 0x01]), "binary data");
        assert_eq!(detect_type(b""), "empty");
    }

    #[test]
    fn empty_input_is_a_clear_error() {
        let err = diff_base64("", HELLO_LOWER, &opts(Output::Summary)).unwrap_err();
        assert!(err.contains("left: no Base64 data"), "got {err}");
    }

    #[test]
    fn option_parsing_rejects_unknown_values() {
        assert!(parse_output("wat").unwrap_err().contains("report, summary, hexdump, text"));
        assert!(parse_align("fuzzy").unwrap_err().contains("offset, shift"));
        assert!(parse_alphabet("rot13").unwrap_err().contains("auto, standard, url"));
        assert!(options_from_strings("auto", "false", "offset", "report", "99", "2")
            .unwrap_err()
            .contains("between 4 and 32"));
        assert!(options_from_strings("auto", "maybe", "offset", "report", "8", "2")
            .unwrap_err()
            .contains("must be true or false"));
    }

    #[test]
    fn string_options_round_trip_from_page_fields() {
        let o = options_from_strings("url", "true", "shift", "hexdump", "16", "0").unwrap();
        assert_eq!(o.alphabet, Alphabet::UrlSafe);
        assert!(o.strict);
        assert_eq!(o.align, Align::Shift);
        assert_eq!(o.output, Output::Hexdump);
        assert_eq!(o.bytes_per_row, 16);
        assert_eq!(o.context_rows, 0);
        // Empty page fields fall back to the descriptor defaults.
        let d = options_from_strings("", "", "", "", "", "").unwrap();
        assert_eq!(d.bytes_per_row, 8);
        assert_eq!(d.context_rows, 2);
        assert_eq!(d.output, Output::Report);
    }

    #[test]
    fn oversize_input_is_rejected() {
        let big = "A".repeat(MAX_INPUT_CHARS + 4);
        let err = diff_base64(&big, HELLO_LOWER, &opts(Output::Summary)).unwrap_err();
        assert!(err.contains("over the"), "got {err}");
    }
}
