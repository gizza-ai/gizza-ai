//! text-file-inspector core — read a text file's RAW BYTES and report what an
//! editor's status bar would show plus what a linter would complain about:
//! detected encoding, BOM, line-ending style (LF / CRLF / CR / mixed), line
//! count, final-newline state, the longest lines, blank/trailing-whitespace
//! lines, indentation style, and character-class counts.
//!
//! Pure compute, dependency-light, shared by the chat skill block, the CLI and
//! the browser page. No wafer / wasm-bindgen deps here.
//!
//! Scope note: this block DIAGNOSES; it never rewrites. Converting between
//! line-ending styles or charsets is `blocks/text-encoding-converter`'s job,
//! and `blocks/data-format-sniffer` answers "is this CSV/JSON and with which
//! delimiter". The overlap here is deliberate and one-directional: encoding is
//! reported because a mojibake file and a CRLF file present the same symptom
//! ("works on my machine") and you want both answers from one look.
//!
//! Everything is measured on the DECODED text (so a UTF-16 file reports real
//! character counts), while sizes and the BOM are measured on the raw bytes.

use std::fmt::Write as _;

use base64::Engine as _;
use chardetng::EncodingDetector;
use encoding_rs::{Encoding, UTF_16BE, UTF_16LE, UTF_8};

/// Largest input accepted, in bytes. Well under the 64 MiB wasmi sandbox even
/// after base64 transport (4/3 expansion) and decoding to UTF-16 in memory.
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
/// Upper bound for the `preview_lines` option.
pub const MAX_PREVIEW_LINES: usize = 200;
/// Upper bound for the `longest_lines` option.
pub const MAX_LONGEST_LINES: usize = 50;
/// Line snippets (preview + longest-line list) are truncated to this many
/// characters so one 100k-char minified line can't flood the report.
pub const SNIPPET_CHARS: usize = 120;
/// Statistical detection samples at most this many leading bytes.
const DETECT_SAMPLE_BYTES: usize = 1024 * 1024;
/// UTF-16-without-BOM heuristic sample size.
const UTF16_SNIFF_BYTES: usize = 4096;

/// How the caller expressed the file's bytes in the `input` string.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputFormat {
    /// `input` is literal text; its UTF-8 bytes are inspected.
    Text,
    /// `input` is base64 (standard or URL-safe, whitespace tolerated).
    Base64,
    /// `input` is hex (whitespace, `:`, `-`, `_`, `,` and `0x` tolerated).
    Hex,
}

impl InputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "text" => Ok(InputFormat::Text),
            "base64" => Ok(InputFormat::Base64),
            "hex" => Ok(InputFormat::Hex),
            other => Err(format!(
                "invalid input_format {other:?}: expected \"text\", \"base64\" or \"hex\""
            )),
        }
    }
}

/// Report rendering style.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    /// Human-readable aligned report.
    Report,
    /// Machine-readable JSON with the same facts.
    Json,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "report" => Ok(OutputFormat::Report),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!(
                "invalid output {other:?}: expected \"report\" or \"json\""
            )),
        }
    }
}

/// All tunable inputs besides the bytes themselves.
#[derive(Clone, Debug)]
pub struct Options {
    pub input_format: InputFormat,
    pub output: OutputFormat,
    /// List the N longest lines (0 = only the single longest is summarised).
    pub longest_lines: usize,
    /// Flag lines longer than this many characters (0 = off).
    pub max_line_length: usize,
    /// Show the first N lines with visible line-ending markers (0 = off).
    pub preview_lines: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input_format: InputFormat::Text,
            output: OutputFormat::Report,
            longest_lines: 3,
            max_line_length: 0,
            preview_lines: 0,
        }
    }
}

/// The line terminator that ended a line (`None` = unterminated last line).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Terminator {
    Crlf,
    Lf,
    Cr,
    None,
}

impl Terminator {
    fn label(self) -> &'static str {
        match self {
            Terminator::Crlf => "CRLF",
            Terminator::Lf => "LF",
            Terminator::Cr => "CR",
            Terminator::None => "none",
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
    /// Canonical encoding label the BOM implies.
    pub fn encoding(self) -> &'static str {
        match self {
            Bom::Utf8 => "UTF-8",
            Bom::Utf16Le => "UTF-16LE",
            Bom::Utf16Be => "UTF-16BE",
            Bom::Utf32Le => "UTF-32LE",
            Bom::Utf32Be => "UTF-32BE",
        }
    }
    /// The BOM's own bytes, spaced hex.
    pub fn hex(self) -> &'static str {
        match self {
            Bom::Utf8 => "EF BB BF",
            Bom::Utf16Le => "FF FE",
            Bom::Utf16Be => "FE FF",
            Bom::Utf32Le => "FF FE 00 00",
            Bom::Utf32Be => "00 00 FE FF",
        }
    }
    pub fn len(self) -> usize {
        match self {
            Bom::Utf8 => 3,
            Bom::Utf16Le | Bom::Utf16Be => 2,
            Bom::Utf32Le | Bom::Utf32Be => 4,
        }
    }
}

/// Sniff a byte-order mark. UTF-32LE is checked before UTF-16LE: `FF FE 00 00`
/// starts with the UTF-16LE BOM, so the order matters.
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

/// One of the longest lines, or one line of the preview.
#[derive(Clone, Debug)]
pub struct LineSample {
    /// 1-based line number.
    pub number: usize,
    /// Characters in the line, excluding its terminator.
    pub chars: usize,
    /// The line's text, truncated to [`SNIPPET_CHARS`] (an ellipsis is added).
    pub snippet: String,
    /// The terminator that ended this line.
    pub terminator: Terminator,
}

/// Everything the inspector measured.
#[derive(Clone, Debug)]
pub struct Report {
    // --- bytes / encoding ---
    pub bytes: usize,
    pub encoding: String,
    /// How the encoding was decided: `bom` | `ascii` | `valid-utf-8` |
    /// `utf-16-heuristic` | `detector` | `empty`.
    pub detection: &'static str,
    pub bom: Option<Bom>,
    /// U+FFFD substitutions made while decoding — bytes that are NOT valid in
    /// the detected encoding.
    pub decode_errors: usize,

    // --- line endings ---
    pub crlf: usize,
    pub lf: usize,
    pub cr: usize,
    /// `CRLF` | `LF` | `CR` | `none` — the most common terminator.
    pub dominant: &'static str,
    pub mixed: bool,

    // --- line metrics ---
    pub lines: usize,
    pub terminated_lines: usize,
    pub final_newline: bool,
    pub longest: Vec<LineSample>,
    pub longest_chars: usize,
    pub average_chars: f64,
    pub empty_lines: usize,
    pub whitespace_only_lines: usize,
    pub trailing_whitespace_lines: Vec<usize>,

    // --- indentation ---
    pub tab_indented: usize,
    pub space_indented: usize,
    pub mixed_indent_lines: Vec<usize>,

    // --- characters ---
    pub chars: usize,
    pub non_ascii: usize,
    pub control_chars: usize,
    pub nul_bytes: usize,
    /// U+2028 / U+2029, which most editors show as line breaks but no line
    /// counter treats as one.
    pub unicode_separators: usize,

    // --- option-driven extras ---
    pub over_limit: Vec<usize>,
    pub limit: usize,
    pub preview: Vec<LineSample>,
    /// Human-readable warnings (binary bytes, mixed endings, missing final
    /// newline, decode errors, …), in report order.
    pub notes: Vec<String>,
}

/// Turn the `input` string into the file's raw bytes.
pub fn decode_input(input: &str, format: InputFormat) -> Result<Vec<u8>, String> {
    let bytes = match format {
        InputFormat::Text => input.as_bytes().to_vec(),
        InputFormat::Base64 => {
            let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
            let engine = if cleaned.contains('-') || cleaned.contains('_') {
                &base64::engine::general_purpose::URL_SAFE_NO_PAD
            } else {
                &base64::engine::general_purpose::STANDARD_NO_PAD
            };
            let trimmed = cleaned.trim_end_matches('=');
            engine
                .decode(trimmed)
                .map_err(|e| format!("input is not valid base64: {e}"))?
        }
        InputFormat::Hex => {
            let cleaned: String = input
                .replace("0x", "")
                .replace("0X", "")
                .replace("\\x", "")
                .chars()
                .filter(|c| !c.is_whitespace() && !matches!(c, ':' | '-' | '_' | ','))
                .collect();
            if cleaned.len() % 2 != 0 {
                return Err(format!(
                    "input is not valid hex: {} hex digits is an odd count",
                    cleaned.len()
                ));
            }
            let mut out = Vec::with_capacity(cleaned.len() / 2);
            let digits: Vec<char> = cleaned.chars().collect();
            for pair in digits.chunks(2) {
                let hi = pair[0].to_digit(16).ok_or_else(|| {
                    format!("input is not valid hex: {:?} is not a hex digit", pair[0])
                })?;
                let lo = pair[1].to_digit(16).ok_or_else(|| {
                    format!("input is not valid hex: {:?} is not a hex digit", pair[1])
                })?;
                out.push((hi * 16 + lo) as u8);
            }
            out
        }
    };
    if bytes.len() > MAX_BYTES {
        return Err(format!(
            "input is {} bytes; this tool inspects up to {} bytes ({} MiB)",
            bytes.len(),
            MAX_BYTES,
            MAX_BYTES / (1024 * 1024)
        ));
    }
    Ok(bytes)
}

/// Decode raw bytes to text, picking the encoding the way an editor would.
/// Returns `(text, encoding label, detection method, decode-error count)`.
fn decode_text(bytes: &[u8], bom: Option<Bom>) -> (String, String, &'static str) {
    let body = &bytes[bom.map_or(0, Bom::len)..];
    if let Some(b) = bom {
        let label = b.encoding().to_string();
        let text = match b {
            Bom::Utf8 => decode_with(UTF_8, body),
            Bom::Utf16Le => decode_with(UTF_16LE, body),
            Bom::Utf16Be => decode_with(UTF_16BE, body),
            Bom::Utf32Le => decode_utf32(body, true),
            Bom::Utf32Be => decode_utf32(body, false),
        };
        return (text, label, "bom");
    }
    if body.is_empty() {
        return (String::new(), "empty".to_string(), "empty");
    }
    if let Some(le) = sniff_utf16(body) {
        let enc = if le { UTF_16LE } else { UTF_16BE };
        let label = if le { "UTF-16LE" } else { "UTF-16BE" };
        return (
            decode_with(enc, body),
            format!("{label} (no BOM)"),
            "utf-16-heuristic",
        );
    }
    if let Ok(s) = core::str::from_utf8(body) {
        if body.iter().all(|&b| b < 0x80) && !body.contains(&0) {
            return (decode_with(UTF_8, body), "ASCII".to_string(), "ascii");
        }
        return (s.to_string(), "UTF-8".to_string(), "valid-utf-8");
    }
    let sample = &body[..body.len().min(DETECT_SAMPLE_BYTES)];
    let mut det = EncodingDetector::new();
    det.feed(sample, sample.len() == body.len());
    let enc = det.guess(None, true);
    (decode_with(enc, body), enc.name().to_string(), "detector")
}

fn decode_with(enc: &'static Encoding, body: &[u8]) -> String {
    let (text, _had_errors) = enc.decode_without_bom_handling(body);
    text.into_owned()
}

/// UTF-32 has no WHATWG encoder/decoder (and therefore none in `encoding_rs`),
/// so decode it by hand: 4-byte code units, invalid ones become U+FFFD.
fn decode_utf32(body: &[u8], little_endian: bool) -> String {
    let mut out = String::with_capacity(body.len() / 4);
    for chunk in body.chunks(4) {
        if chunk.len() < 4 {
            out.push('\u{FFFD}');
            break;
        }
        let v = if little_endian {
            u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
        } else {
            u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
        };
        out.push(char::from_u32(v).unwrap_or('\u{FFFD}'));
    }
    out
}

/// BOM-less UTF-16 heuristic: ASCII-range text in UTF-16 is half NUL bytes,
/// all on the same parity. Returns `Some(true)` for LE, `Some(false)` for BE.
/// Without this, a BOM-less UTF-16 export reads as "binary" — the single most
/// confusing thing an editor can tell you about a plainly readable file.
fn sniff_utf16(body: &[u8]) -> Option<bool> {
    let sample = &body[..body.len().min(UTF16_SNIFF_BYTES)];
    if sample.len() < 4 {
        return None;
    }
    let even_nul = sample.iter().step_by(2).filter(|&&b| b == 0).count();
    let odd_nul = sample
        .iter()
        .skip(1)
        .step_by(2)
        .filter(|&&b| b == 0)
        .count();
    let pairs = sample.len() / 2;
    if odd_nul * 10 >= pairs * 8 && even_nul * 10 <= pairs {
        Some(true) // NULs in the high byte of each little-endian unit
    } else if even_nul * 10 >= pairs * 8 && odd_nul * 10 <= pairs {
        Some(false)
    } else {
        None
    }
}

fn truncate_snippet(s: &str) -> String {
    let mut out: String = s.chars().take(SNIPPET_CHARS).collect();
    if s.chars().count() > SNIPPET_CHARS {
        out.push('…');
    }
    out
}

/// Measure `bytes` as a text file.
pub fn analyze(bytes: &[u8], opts: &Options) -> Report {
    let bom = sniff_bom(bytes);
    let (text, encoding, detection) = decode_text(bytes, bom);
    let decode_errors = text.chars().filter(|&c| c == '\u{FFFD}').count();

    // --- split into (line, terminator) pairs over the decoded text ---
    let mut lines: Vec<(&str, Terminator)> = Vec::new();
    let (mut crlf, mut lf, mut cr) = (0usize, 0usize, 0usize);
    let chars: Vec<char> = text.chars().collect();
    let mut start_byte = 0usize;
    let mut idx_byte = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        let clen = c.len_utf8();
        match c {
            '\r' => {
                let crlf_here = chars.get(i + 1) == Some(&'\n');
                lines.push((
                    &text[start_byte..idx_byte],
                    if crlf_here {
                        Terminator::Crlf
                    } else {
                        Terminator::Cr
                    },
                ));
                if crlf_here {
                    crlf += 1;
                    i += 2;
                    idx_byte += clen + 1;
                } else {
                    cr += 1;
                    i += 1;
                    idx_byte += clen;
                }
                start_byte = idx_byte;
                continue;
            }
            '\n' => {
                lines.push((&text[start_byte..idx_byte], Terminator::Lf));
                lf += 1;
                i += 1;
                idx_byte += clen;
                start_byte = idx_byte;
                continue;
            }
            _ => {}
        }
        i += 1;
        idx_byte += clen;
    }
    let terminated_lines = lines.len();
    let final_newline = terminated_lines > 0 && start_byte == text.len();
    if start_byte < text.len() {
        lines.push((&text[start_byte..], Terminator::None));
    }

    // --- per-line metrics ---
    let mut empty_lines = 0usize;
    let mut whitespace_only_lines = 0usize;
    let mut trailing_whitespace_lines: Vec<usize> = Vec::new();
    let mut tab_indented = 0usize;
    let mut space_indented = 0usize;
    let mut mixed_indent_lines: Vec<usize> = Vec::new();
    let mut over_limit: Vec<usize> = Vec::new();
    let mut total_chars = 0usize;
    let mut ranked: Vec<(usize, usize)> = Vec::with_capacity(lines.len()); // (chars, line no)

    for (n, (line, _term)) in lines.iter().enumerate() {
        let number = n + 1;
        let len = line.chars().count();
        total_chars += len;
        ranked.push((len, number));
        if line.is_empty() {
            empty_lines += 1;
        } else {
            if line.trim().is_empty() {
                whitespace_only_lines += 1;
            } else if line.len() != line.trim_end().len() {
                trailing_whitespace_lines.push(number);
            }
            let indent: String = line
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect();
            if indent.contains('\t') && indent.contains(' ') {
                mixed_indent_lines.push(number);
            }
            match indent.chars().next() {
                Some('\t') => tab_indented += 1,
                Some(' ') => space_indented += 1,
                _ => {}
            }
        }
        if opts.max_line_length > 0 && len > opts.max_line_length {
            over_limit.push(number);
        }
    }

    // Longest lines: longest first, earlier line number breaking ties.
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let want = opts
        .longest_lines
        .clamp(1, MAX_LONGEST_LINES)
        .min(lines.len());
    let longest: Vec<LineSample> = ranked
        .iter()
        .take(want)
        .map(|&(len, number)| LineSample {
            number,
            chars: len,
            snippet: truncate_snippet(lines[number - 1].0),
            terminator: lines[number - 1].1,
        })
        .collect();
    let longest_chars = ranked.first().map_or(0, |&(len, _)| len);
    let average_chars = if lines.is_empty() {
        0.0
    } else {
        total_chars as f64 / lines.len() as f64
    };

    let preview: Vec<LineSample> = lines
        .iter()
        .take(opts.preview_lines.min(MAX_PREVIEW_LINES))
        .enumerate()
        .map(|(n, (line, term))| LineSample {
            number: n + 1,
            chars: line.chars().count(),
            snippet: truncate_snippet(line),
            terminator: *term,
        })
        .collect();

    // --- character classes ---
    let non_ascii = text.chars().filter(|&c| c as u32 > 0x7F).count();
    let control_chars = text
        .chars()
        .filter(|&c| (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') || c as u32 == 0x7F)
        .count();
    let unicode_separators = text
        .chars()
        .filter(|&c| c == '\u{2028}' || c == '\u{2029}')
        .count();
    let nul_bytes = bytes.iter().filter(|&&b| b == 0).count();

    let styles = [(crlf, "CRLF"), (lf, "LF"), (cr, "CR")];
    let mut dominant = "none";
    let mut dominant_count = 0usize;
    for (n, label) in styles.iter().copied() {
        // Strictly greater keeps the first style on ties, so CRLF wins a
        // one-each CRLF/LF/CR mix and the report stays deterministic.
        if n > dominant_count {
            dominant_count = n;
            dominant = label;
        }
    }
    let mixed = styles.iter().filter(|(n, _)| *n > 0).count() > 1;

    let mut report = Report {
        bytes: bytes.len(),
        encoding,
        detection,
        bom,
        decode_errors,
        crlf,
        lf,
        cr,
        dominant,
        mixed,
        lines: lines.len(),
        terminated_lines,
        final_newline,
        longest,
        longest_chars,
        average_chars,
        empty_lines,
        whitespace_only_lines,
        trailing_whitespace_lines,
        tab_indented,
        space_indented,
        mixed_indent_lines,
        chars: chars.len(),
        non_ascii,
        control_chars,
        nul_bytes,
        unicode_separators,
        over_limit,
        limit: opts.max_line_length,
        preview,
        notes: Vec::new(),
    };
    report.notes = build_notes(&report);
    report
}

fn build_notes(r: &Report) -> Vec<String> {
    let mut notes = Vec::new();
    if r.mixed {
        notes.push(
            "Mixed line endings — Git diffs, shell scripts and some parsers break on these. \
             Normalise the file to one style."
                .to_string(),
        );
    }
    if r.dominant == "CR" {
        notes.push(
            "Bare CR line endings are classic Mac OS (pre-2001); most modern tools read the \
             file as a single line."
                .to_string(),
        );
    }
    if r.lines > 0 && !r.final_newline {
        notes.push(
            "No newline at end of file — POSIX tools (wc, diff, cat) and many linters expect \
             a trailing newline."
                .to_string(),
        );
    }
    if r.bom == Some(Bom::Utf8) {
        notes.push(
            "UTF-8 BOM present — harmless in editors, but it breaks shebangs, JSON parsers, \
             CSV headers and PHP/shell output."
                .to_string(),
        );
    }
    if r.decode_errors > 0 {
        notes.push(format!(
            "{} byte sequence(s) could not be decoded as {} and became U+FFFD — the encoding \
             guess may be wrong, or the file is not text.",
            r.decode_errors, r.encoding
        ));
    }
    if r.nul_bytes > 0 && !r.encoding.starts_with("UTF-16") && !r.encoding.starts_with("UTF-32") {
        notes.push(format!(
            "{} NUL byte(s) — this looks like a binary file, not text.",
            r.nul_bytes
        ));
    }
    if r.unicode_separators > 0 {
        notes.push(format!(
            "{} U+2028/U+2029 separator(s) — some editors draw them as line breaks, but no \
             line counter (this one included) treats them as one.",
            r.unicode_separators
        ));
    }
    if !r.mixed_indent_lines.is_empty() {
        notes.push(format!(
            "{} line(s) mix tabs and spaces in their indentation.",
            r.mixed_indent_lines.len()
        ));
    }
    notes
}

/// Thousands-separated integer, e.g. `1,234`.
fn commas(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn human_bytes(n: usize) -> String {
    const KIB: f64 = 1024.0;
    let f = n as f64;
    if f >= KIB * KIB {
        format!("{:.1} MiB", f / (KIB * KIB))
    } else if f >= KIB {
        format!("{:.1} KiB", f / KIB)
    } else {
        format!("{n} B")
    }
}

fn pct(part: usize, whole: usize) -> String {
    if whole == 0 {
        return "0%".to_string();
    }
    format!("{:.0}%", part as f64 * 100.0 / whole as f64)
}

/// Comma-joined line numbers, capped so a whole-file problem stays readable.
fn number_list(nums: &[usize], cap: usize) -> String {
    let shown: Vec<String> = nums.iter().take(cap).map(|n| n.to_string()).collect();
    if nums.len() > cap {
        format!("{}, … (+{} more)", shown.join(", "), nums.len() - cap)
    } else {
        shown.join(", ")
    }
}

/// Render a line's snippet with visible terminator markers.
fn marked(sample: &LineSample) -> String {
    let body = sample.snippet.replace('\t', "→");
    match sample.terminator {
        Terminator::Crlf => format!("{body}␍␊"),
        Terminator::Lf => format!("{body}␊"),
        Terminator::Cr => format!("{body}␍"),
        Terminator::None => format!("{body}⏎̸"),
    }
}

/// The aligned human-readable report.
pub fn render_report(r: &Report) -> String {
    let mut o = String::new();
    let _ = writeln!(
        o,
        "File          {} bytes ({})",
        commas(r.bytes),
        human_bytes(r.bytes)
    );
    let _ = writeln!(o, "Encoding      {} (via {})", r.encoding, r.detection);
    match r.bom {
        Some(b) => {
            let _ = writeln!(
                o,
                "BOM           yes — {} ({}, {} bytes)",
                b.encoding(),
                b.hex(),
                b.len()
            );
        }
        None => {
            let _ = writeln!(o, "BOM           none");
        }
    }

    let total_breaks = r.crlf + r.lf + r.cr;
    let _ = writeln!(o);
    let style = if r.mixed {
        format!("MIXED (mostly {})", r.dominant)
    } else {
        match r.dominant {
            "CRLF" => "CRLF (Windows)".to_string(),
            "LF" => "LF (Unix, macOS, Linux)".to_string(),
            "CR" => "CR (classic Mac OS)".to_string(),
            _ => "none (no line breaks)".to_string(),
        }
    };
    let _ = writeln!(o, "Line endings  {style}");
    let _ = writeln!(
        o,
        "  CRLF        {} ({})",
        commas(r.crlf),
        pct(r.crlf, total_breaks)
    );
    let _ = writeln!(
        o,
        "  LF          {} ({})",
        commas(r.lf),
        pct(r.lf, total_breaks)
    );
    let _ = writeln!(
        o,
        "  CR          {} ({})",
        commas(r.cr),
        pct(r.cr, total_breaks)
    );
    let _ = writeln!(
        o,
        "Final newline {}",
        if r.final_newline { "yes" } else { "no" }
    );

    let _ = writeln!(o);
    let _ = writeln!(
        o,
        "Lines         {} ({} terminated, {} unterminated)",
        commas(r.lines),
        commas(r.terminated_lines),
        r.lines - r.terminated_lines
    );
    let _ = writeln!(
        o,
        "Longest line  {} chars (line {})",
        commas(r.longest_chars),
        r.longest.first().map_or(0, |l| l.number)
    );
    let _ = writeln!(o, "Average line  {:.1} chars", r.average_chars);
    let _ = writeln!(
        o,
        "Blank lines   {} empty, {} whitespace-only",
        commas(r.empty_lines),
        commas(r.whitespace_only_lines)
    );
    let _ = writeln!(
        o,
        "Trailing WS   {} line(s){}",
        commas(r.trailing_whitespace_lines.len()),
        if r.trailing_whitespace_lines.is_empty() {
            String::new()
        } else {
            format!(" — {}", number_list(&r.trailing_whitespace_lines, 10))
        }
    );
    let indent = if r.tab_indented > 0 && r.space_indented > 0 {
        format!(
            "MIXED — {} tab-indented, {} space-indented",
            commas(r.tab_indented),
            commas(r.space_indented)
        )
    } else if r.tab_indented > 0 {
        format!("tabs ({} line(s))", commas(r.tab_indented))
    } else if r.space_indented > 0 {
        format!("spaces ({} line(s))", commas(r.space_indented))
    } else {
        "none".to_string()
    };
    let _ = writeln!(o, "Indentation   {indent}");
    let _ = writeln!(
        o,
        "Characters    {} total, {} non-ASCII, {} control, {} NUL",
        commas(r.chars),
        commas(r.non_ascii),
        commas(r.control_chars),
        commas(r.nul_bytes)
    );

    if r.limit > 0 {
        let _ = writeln!(o);
        let _ = writeln!(
            o,
            "Over {} chars  {} line(s){}",
            r.limit,
            commas(r.over_limit.len()),
            if r.over_limit.is_empty() {
                String::new()
            } else {
                format!(" — {}", number_list(&r.over_limit, 10))
            }
        );
    }

    if r.longest.len() > 1 {
        let _ = writeln!(o);
        let _ = writeln!(o, "Longest {} lines:", r.longest.len());
        for l in &r.longest {
            let _ = writeln!(o, "  {:>6}  {:>6} chars  {}", l.number, l.chars, l.snippet);
        }
    }

    if !r.preview.is_empty() {
        let _ = writeln!(o);
        let _ = writeln!(
            o,
            "Preview (first {} line(s); ␍ = CR, ␊ = LF, → = tab, ⏎̸ = no terminator):",
            r.preview.len()
        );
        for l in &r.preview {
            let _ = writeln!(o, "  {:>6} | {}", l.number, marked(l));
        }
    }

    if !r.notes.is_empty() {
        let _ = writeln!(o);
        let _ = writeln!(o, "Notes:");
        for n in &r.notes {
            let _ = writeln!(o, "  • {n}");
        }
    }
    o.trim_end().to_string()
}

/// The machine-readable report — same facts, stable key names.
pub fn render_json(r: &Report) -> String {
    let samples = |v: &[LineSample]| -> Vec<serde_json::Value> {
        v.iter()
            .map(|l| {
                serde_json::json!({
                    "line": l.number,
                    "chars": l.chars,
                    "terminator": l.terminator.label(),
                    "text": l.snippet,
                })
            })
            .collect()
    };
    let value = serde_json::json!({
        "bytes": r.bytes,
        "encoding": r.encoding,
        "detection": r.detection,
        "bom": r.bom.map(|b| serde_json::json!({
            "encoding": b.encoding(),
            "hex": b.hex(),
            "bytes": b.len(),
        })),
        "decode_errors": r.decode_errors,
        "line_endings": {
            "crlf": r.crlf,
            "lf": r.lf,
            "cr": r.cr,
            "dominant": r.dominant,
            "mixed": r.mixed,
        },
        "lines": {
            "total": r.lines,
            "terminated": r.terminated_lines,
            "unterminated": r.lines - r.terminated_lines,
            "final_newline": r.final_newline,
            "longest_chars": r.longest_chars,
            "average_chars": (r.average_chars * 100.0).round() / 100.0,
            "empty": r.empty_lines,
            "whitespace_only": r.whitespace_only_lines,
            "trailing_whitespace": r.trailing_whitespace_lines,
            "longest": samples(&r.longest),
        },
        "indentation": {
            "tab_indented": r.tab_indented,
            "space_indented": r.space_indented,
            "mixed_lines": r.mixed_indent_lines,
        },
        "characters": {
            "total": r.chars,
            "non_ascii": r.non_ascii,
            "control": r.control_chars,
            "nul_bytes": r.nul_bytes,
            "unicode_separators": r.unicode_separators,
        },
        "max_line_length": {
            "limit": r.limit,
            "over_limit": r.over_limit,
        },
        "preview": samples(&r.preview),
        "notes": r.notes,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

/// End-to-end: decode the input, analyse it, render the requested format.
pub fn inspect(input: &str, opts: &Options) -> Result<String, String> {
    let bytes = decode_input(input, opts.input_format)?;
    if bytes.is_empty() {
        return Err("input is empty — paste text or choose a file to inspect".to_string());
    }
    let report = analyze(&bytes, opts);
    Ok(match opts.output {
        OutputFormat::Report => render_report(&report),
        OutputFormat::Json => render_json(&report),
    })
}

/// String-argument entry point shared by the block, the CLI and the page.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    longest_lines: i64,
    max_line_length: i64,
    preview_lines: i64,
) -> Result<String, String> {
    if longest_lines < 0 || max_line_length < 0 || preview_lines < 0 {
        return Err("longest_lines, max_line_length and preview_lines must be 0 or more".into());
    }
    let opts = Options {
        input_format: InputFormat::parse(input_format)?,
        output: OutputFormat::parse(output)?,
        longest_lines: (longest_lines as usize).min(MAX_LONGEST_LINES),
        max_line_length: max_line_length as usize,
        preview_lines: (preview_lines as usize).min(MAX_PREVIEW_LINES),
    };
    inspect(input, &opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn happy_path_reports_crlf_lines_and_longest_line() {
        let out = run("alpha\r\nbeta beta\r\ngamma\r\n", "text", "report", 1, 0, 0).unwrap();
        assert!(out.contains("Line endings  CRLF (Windows)"), "{out}");
        assert!(out.contains("  CRLF        3 (100%)"), "{out}");
        assert!(
            out.contains("Lines         3 (3 terminated, 0 unterminated)"),
            "{out}"
        );
        assert!(out.contains("Longest line  9 chars (line 2)"), "{out}");
        assert!(out.contains("Final newline yes"), "{out}");
    }

    #[test]
    fn error_on_invalid_base64() {
        let err = run("!!!not base64!!!", "base64", "report", 3, 0, 0).unwrap_err();
        assert!(err.contains("not valid base64"), "{err}");
    }

    #[test]
    fn error_on_unknown_input_format() {
        let err = run("x", "utf9", "report", 3, 0, 0).unwrap_err();
        assert!(err.contains("invalid input_format"), "{err}");
    }

    #[test]
    fn error_on_empty_input() {
        let err = run("", "text", "report", 3, 0, 0).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn mixed_endings_are_flagged_with_a_note() {
        let r = analyze(b"a\r\nb\nc\rd", &opts());
        assert!(r.mixed);
        assert_eq!((r.crlf, r.lf, r.cr), (1, 1, 1));
        assert_eq!(r.dominant, "CRLF"); // tie → first style wins, CRLF listed first
        assert_eq!(r.lines, 4);
        assert_eq!(r.terminated_lines, 3);
        assert!(!r.final_newline);
        assert!(r.notes.iter().any(|n| n.contains("Mixed line endings")));
        assert!(r
            .notes
            .iter()
            .any(|n| n.contains("No newline at end of file")));
    }

    #[test]
    fn utf8_bom_is_detected_and_stripped_from_the_text() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("héllo\n".as_bytes());
        let r = analyze(&bytes, &opts());
        assert_eq!(r.bom, Some(Bom::Utf8));
        assert_eq!(r.encoding, "UTF-8");
        assert_eq!(r.detection, "bom");
        assert_eq!(r.chars, 6); // héllo + \n — the BOM is not counted as text
        assert_eq!(r.non_ascii, 1);
        assert!(r.notes.iter().any(|n| n.contains("UTF-8 BOM present")));
    }

    #[test]
    fn utf16le_with_bom_decodes_to_real_characters() {
        let mut bytes = vec![0xFF, 0xFE];
        for u in "hi\r\n".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        let r = analyze(&bytes, &opts());
        assert_eq!(r.bom, Some(Bom::Utf16Le));
        assert_eq!(r.encoding, "UTF-16LE");
        assert_eq!(r.lines, 1);
        assert_eq!(r.crlf, 1);
        assert_eq!(r.chars, 4);
        // NUL bytes are expected in UTF-16 — no "looks binary" note.
        assert!(!r.notes.iter().any(|n| n.contains("binary")));
    }

    #[test]
    fn bomless_utf16le_is_sniffed_not_called_binary() {
        let mut bytes = Vec::new();
        for u in "hello world\nsecond line\n".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        let r = analyze(&bytes, &opts());
        assert_eq!(r.bom, None);
        assert_eq!(r.encoding, "UTF-16LE (no BOM)");
        assert_eq!(r.detection, "utf-16-heuristic");
        assert_eq!(r.lines, 2);
    }

    #[test]
    fn ascii_only_input_reports_ascii() {
        let r = analyze(b"plain\n", &opts());
        assert_eq!(r.encoding, "ASCII");
        assert_eq!(r.detection, "ascii");
        assert_eq!(r.non_ascii, 0);
    }

    #[test]
    fn legacy_bytes_fall_back_to_the_statistical_detector() {
        // Windows-1252 "café" — 0xE9 is not valid UTF-8.
        let r = analyze(b"caf\xE9 na\xEFve text in a longer line\n", &opts());
        assert_eq!(r.detection, "detector");
        assert!(r.encoding != "UTF-8", "{}", r.encoding);
    }

    #[test]
    fn nul_bytes_flag_a_binary_file() {
        let r = analyze(b"text\x00\x00more text here\n", &opts());
        assert_eq!(r.nul_bytes, 2);
        assert!(r
            .notes
            .iter()
            .any(|n| n.contains("looks like a binary file")));
    }

    #[test]
    fn whitespace_indentation_and_long_lines_are_measured() {
        let src = "\ttabbed\n    spaced\n \t both  \nplain\n";
        let mut o = opts();
        o.max_line_length = 6;
        let r = analyze(src.as_bytes(), &o);
        assert_eq!(r.tab_indented, 1);
        assert_eq!(r.space_indented, 2);
        assert_eq!(r.mixed_indent_lines, vec![3]);
        assert_eq!(r.trailing_whitespace_lines, vec![3]);
        assert_eq!(r.over_limit, vec![1, 2, 3]);
        assert_eq!(r.limit, 6);
    }

    #[test]
    fn json_output_carries_the_same_facts() {
        let out = run("a\nbb\n", "text", "json", 2, 0, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["line_endings"]["lf"], 2);
        assert_eq!(v["line_endings"]["dominant"], "LF");
        assert_eq!(v["lines"]["total"], 2);
        assert_eq!(v["lines"]["longest_chars"], 2);
        assert_eq!(v["lines"]["final_newline"], true);
        assert_eq!(v["bom"], serde_json::Value::Null);
        assert_eq!(v["lines"]["longest"][0]["line"], 2);
    }

    #[test]
    fn base64_and_hex_inputs_agree_with_text() {
        let text = "one\r\ntwo\n";
        let b64 = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
        let hex: String = text.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
        let a = run(text, "text", "json", 1, 0, 0).unwrap();
        let b = run(&b64, "base64", "json", 1, 0, 0).unwrap();
        let c = run(&format!("0x{hex}"), "hex", "json", 1, 0, 0).unwrap();
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn preview_renders_visible_terminators() {
        let out = run("a\r\nb\n\tc", "text", "report", 1, 0, 5).unwrap();
        assert!(out.contains("1 | a␍␊"), "{out}");
        assert!(out.contains("2 | b␊"), "{out}");
        assert!(out.contains("3 | →c⏎̸"), "{out}");
    }

    #[test]
    fn unicode_separators_are_reported_but_not_counted_as_lines() {
        let r = analyze("a\u{2028}b\n".as_bytes(), &opts());
        assert_eq!(r.unicode_separators, 1);
        assert_eq!(r.lines, 1);
        assert!(r.notes.iter().any(|n| n.contains("U+2028")));
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = "x".repeat(MAX_BYTES + 1);
        let err = decode_input(&big, InputFormat::Text).unwrap_err();
        assert!(err.contains("inspects up to"), "{err}");
    }

    #[test]
    fn long_snippets_are_truncated() {
        let line = "y".repeat(SNIPPET_CHARS + 50);
        let r = analyze(line.as_bytes(), &opts());
        assert_eq!(r.longest_chars, SNIPPET_CHARS + 50);
        assert!(r.longest[0].snippet.ends_with('…'));
        assert_eq!(r.longest[0].snippet.chars().count(), SNIPPET_CHARS + 1);
    }
}
