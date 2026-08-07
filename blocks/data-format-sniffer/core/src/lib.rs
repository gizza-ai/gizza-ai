//! data-format-sniffer core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Given a chunk of pasted data (as text, or as base64/hex bytes) the sniffer
//! answers: what format is this, how sure are we, how is it encoded, and — for
//! delimited data — which delimiter, quote character, header row, column count
//! and per-column types it uses.
//!
//! Pipeline: decode `input_form` → byte-magic check (Parquet/Avro/…) → encoding
//! detection (BOM sniff, then chardetng for real bytes) → decode to text →
//! line-ending detection → structure detection (JSON → JSONL → Markdown table →
//! HTML → XML → YAML marker → delimited → fixed-width → plain text) → report.

use serde_json::{json, Value};

/// Hard cap on the decoded input. The wasm sandbox is 64 MiB and every stage
/// here keeps the whole input in memory, so the cap is deliberately modest.
pub const MAX_BYTES: usize = 1024 * 1024;
/// Upper bound for the `sample_lines` option.
pub const MAX_SAMPLE_LINES: usize = 10_000;
/// Upper bound for the `preview_rows` option.
pub const MAX_PREVIEW_ROWS: usize = 50;
/// Delimiters tried before any user-supplied extras, in priority order.
pub const DEFAULT_DELIMITERS: [char; 7] = [',', '\t', ';', '|', ':', '~', ' '];

#[derive(Debug, Clone)]
pub struct Options {
    /// How to interpret `data`: `text`, `base64`, or `hex`.
    pub input_form: String,
    /// Number of leading lines used for structure analysis.
    pub sample_lines: usize,
    /// Extra delimiter characters to try, e.g. `^#`.
    pub extra_delimiters: String,
    /// When non-empty, lines starting with this prefix are ignored.
    pub comment_prefix: String,
    /// Infer per-column types (and use them for header detection).
    pub detect_types: bool,
    /// Rows to include in the preview table.
    pub preview_rows: usize,
    /// `report` (human-readable) or `json`.
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input_form: "text".into(),
            sample_lines: 100,
            extra_delimiters: String::new(),
            comment_prefix: String::new(),
            detect_types: true,
            preview_rows: 5,
            output: "report".into(),
        }
    }
}

/// Analyse `data` and return a report (`output=report`) or JSON (`output=json`).
pub fn sniff(data: &str, o: &Options) -> Result<String, String> {
    let form = norm_opt(&o.input_form, "text");
    if !matches!(form.as_str(), "text" | "base64" | "hex") {
        return Err(format!(
            "input_form must be text, base64, or hex — got \"{}\"",
            o.input_form.trim()
        ));
    }
    let output = norm_opt(&o.output, "report");
    if !matches!(output.as_str(), "report" | "json") {
        return Err(format!(
            "output must be report or json — got \"{}\"",
            o.output.trim()
        ));
    }
    if o.sample_lines == 0 || o.sample_lines > MAX_SAMPLE_LINES {
        return Err(format!(
            "sample_lines must be between 1 and {MAX_SAMPLE_LINES} — got {}",
            o.sample_lines
        ));
    }
    if o.preview_rows > MAX_PREVIEW_ROWS {
        return Err(format!(
            "preview_rows must be between 0 and {MAX_PREVIEW_ROWS} — got {}",
            o.preview_rows
        ));
    }

    let bytes = decode_input(data, &form)?;
    if bytes.len() > MAX_BYTES {
        return Err(format!(
            "input too large: {} bytes (max {MAX_BYTES} bytes = 1 MiB) — analyse a smaller sample",
            bytes.len()
        ));
    }
    if bytes.is_empty() {
        return Err("no data to analyse: the input is empty".into());
    }

    let mut notes: Vec<String> = Vec::new();
    let mut report = Report::new(bytes.len());

    // 1. Binary container formats, straight off the magic bytes.
    if let Some(m) = binary_magic(&bytes) {
        report.format = m.slug.into();
        report.label = m.label.into();
        report.confidence = m.confidence;
        report.encoding = Some(EncodingInfo {
            label: "binary".into(),
            bom: false,
            source: "magic bytes".into(),
        });
        notes.push(m.note.to_string());
        notes.push(
            "Binary containers are identified by their signature only; field-level structure is \
             not parsed here."
                .into(),
        );
        report.notes = notes;
        return Ok(render(&report, &output));
    }

    // 2. Encoding.
    let enc = detect_encoding(&bytes, &form);
    let (text, replacements) = decode_text(&bytes, &enc.label)?;
    report.encoding = Some(enc.clone());
    if form == "text" {
        notes.push(
            "Encoding is UTF-8 by construction because the data arrived as text. To detect the \
             original encoding of a file, paste its bytes with input_form=base64 or hex."
                .into(),
        );
    }
    if replacements > 0 {
        notes.push(format!(
            "{replacements} byte sequence(s) could not be decoded as {} and were replaced with \
             U+FFFD; the detected encoding may be wrong.",
            enc.label
        ));
    }
    if text.trim().is_empty() {
        return Err("no data to analyse: the input is only whitespace".into());
    }

    // 3. Line endings, then normalise so every later stage sees "\n".
    let le = detect_line_endings(&text);
    report.line_ending = Some(le.kind.to_string());
    if le.kind == "mixed" {
        notes.push(format!(
            "Mixed line endings: {} CRLF, {} LF, {} CR.",
            le.crlf, le.lf, le.cr
        ));
    }
    let norm = text.replace("\r\n", "\n").replace('\r', "\n");

    let all_lines: Vec<&str> = norm.split('\n').collect();
    let total_lines = if norm.ends_with('\n') {
        all_lines.len() - 1
    } else {
        all_lines.len()
    };
    report.lines = total_lines;

    // Sample: leading lines, minus comment lines when a prefix was supplied.
    let prefix = o.comment_prefix.trim().to_string();
    let mut sample: Vec<&str> = Vec::new();
    let mut comments = 0usize;
    let mut scanned = 0usize;
    for l in all_lines.iter() {
        if sample.len() >= o.sample_lines {
            break;
        }
        scanned += 1;
        if !prefix.is_empty() && l.trim_start().starts_with(&prefix) {
            comments += 1;
            continue;
        }
        sample.push(l);
    }
    while sample.last().map(|l| l.trim().is_empty()) == Some(true) {
        sample.pop();
    }
    report.sampled_lines = sample.len();
    if comments > 0 {
        notes.push(format!(
            "{comments} comment line(s) starting with \"{prefix}\" were ignored."
        ));
    }
    if scanned < total_lines {
        notes.push(format!(
            "Only the first {} of {total_lines} lines were analysed (sample_lines={}).",
            scanned, o.sample_lines
        ));
    }
    let sample_text = sample.join("\n");

    // 4. Structure. The whole-document JSON check uses the FULL text; every
    //    other check works on the sample.
    if let Some(v) = whole_json(&norm) {
        fill_json(&mut report, &v);
    } else if let Some(n) = jsonl_records(&sample) {
        report.format = "jsonl".into();
        report.label = "JSON Lines / NDJSON".into();
        report.confidence = 95;
        report.extra.push(("json_records".into(), json!(n)));
        notes.push("Every sampled non-empty line parses as a standalone JSON value.".into());
    } else if let Some(cols) = markdown_table(&sample) {
        report.format = "markdown-table".into();
        report.label = "Markdown table".into();
        report.confidence = 95;
        report.columns = Some(cols);
    } else if is_html(&sample_text) {
        report.format = "html".into();
        report.label = "HTML document".into();
        report.confidence = 90;
    } else if let Some((root, elems)) = xml_info(&norm) {
        report.format = "xml".into();
        report.label = "XML document".into();
        report.confidence = 98;
        report.extra.push(("xml_root".into(), json!(root)));
        report.extra.push(("xml_elements".into(), json!(elems)));
    } else if yaml_marker(&sample) {
        report.format = "yaml".into();
        report.label = "YAML document".into();
        report.confidence = 85;
        notes.push(
            "YAML is only reported when a document marker (--- or %YAML) is present; marker-less \
             YAML is reported as plain text or as colon-delimited data."
                .into(),
        );
    } else if let Some(d) = best_delimited(&sample_text, &o.extra_delimiters)? {
        fill_delimited(&mut report, &d, o, &mut notes);
    } else if let Some(fw) = fixed_width(&sample) {
        report.format = "fixed-width".into();
        report.label = "Fixed-width text".into();
        report.confidence = 70;
        report.columns = Some(fw.len());
        report.extra.push((
            "field_ranges".into(),
            Value::Array(
                fw.iter()
                    .map(|(a, b)| json!(format!("{}-{}", a + 1, b)))
                    .collect(),
            ),
        ));
        notes.push(
            "No delimiter was consistent, but the columns line up at fixed character positions."
                .into(),
        );
    } else {
        report.format = "text".into();
        report.label = "Plain text (no tabular or structured layout detected)".into();
        report.confidence = 60;
        notes.push(
            "No delimiter produced a consistent column count. Try extra_delimiters if the file \
             uses an unusual separator."
                .into(),
        );
    }

    if report.preview.is_empty() && o.preview_rows > 0 {
        report.preview = sample
            .iter()
            .take(o.preview_rows)
            .map(|l| vec![l.to_string()])
            .collect();
    }

    report.notes = notes;
    Ok(render(&report, &output))
}

// ---------------------------------------------------------------- report ---

#[derive(Debug, Clone)]
pub struct EncodingInfo {
    pub label: String,
    pub bom: bool,
    pub source: String,
}

#[derive(Debug, Default)]
struct Report {
    format: String,
    label: String,
    confidence: u32,
    bytes: usize,
    lines: usize,
    sampled_lines: usize,
    encoding: Option<EncodingInfo>,
    line_ending: Option<String>,
    delimiter: Option<char>,
    quote_char: Option<char>,
    header: Option<String>,
    columns: Option<usize>,
    data_rows: Option<usize>,
    column_names: Vec<String>,
    column_types: Vec<String>,
    delimiter_scores: Vec<(char, usize, u32)>,
    preview: Vec<Vec<String>>,
    extra: Vec<(String, Value)>,
    notes: Vec<String>,
}

impl Report {
    fn new(bytes: usize) -> Self {
        Report {
            format: "unknown".into(),
            label: "Unknown".into(),
            bytes,
            ..Default::default()
        }
    }
}

fn render(r: &Report, output: &str) -> String {
    if output == "json" {
        render_json(r)
    } else {
        render_report(r)
    }
}

fn render_json(r: &Report) -> String {
    let mut m = serde_json::Map::new();
    m.insert("format".into(), json!(r.format));
    m.insert("format_label".into(), json!(r.label));
    m.insert("confidence".into(), json!(r.confidence));
    m.insert("bytes".into(), json!(r.bytes));
    if let Some(e) = &r.encoding {
        m.insert(
            "encoding".into(),
            json!({ "label": e.label, "bom": e.bom, "source": e.source }),
        );
    }
    if let Some(le) = &r.line_ending {
        m.insert("line_ending".into(), json!(le));
    }
    if r.lines > 0 {
        m.insert("lines".into(), json!(r.lines));
        m.insert("sampled_lines".into(), json!(r.sampled_lines));
    }
    if let Some(d) = r.delimiter {
        m.insert("delimiter".into(), json!(d.to_string()));
        m.insert("delimiter_name".into(), json!(delim_name(d)));
    }
    if let Some(q) = r.quote_char {
        m.insert("quote_char".into(), json!(q.to_string()));
    } else if r.delimiter.is_some() {
        m.insert("quote_char".into(), Value::Null);
    }
    if let Some(h) = &r.header {
        m.insert("header".into(), json!(h));
    }
    if let Some(c) = r.columns {
        m.insert("columns".into(), json!(c));
    }
    if let Some(n) = r.data_rows {
        m.insert("data_rows".into(), json!(n));
    }
    if !r.column_names.is_empty() {
        m.insert("column_names".into(), json!(r.column_names));
    }
    if !r.column_types.is_empty() {
        m.insert("column_types".into(), json!(r.column_types));
    }
    if !r.delimiter_scores.is_empty() {
        m.insert(
            "delimiter_scores".into(),
            Value::Array(
                r.delimiter_scores
                    .iter()
                    .map(|(d, cols, cons)| {
                        json!({
                            "delimiter": d.to_string(),
                            "name": delim_name(*d),
                            "columns": cols,
                            "consistency": cons,
                        })
                    })
                    .collect(),
            ),
        );
    }
    for (k, v) in &r.extra {
        m.insert(k.clone(), v.clone());
    }
    if !r.preview.is_empty() {
        m.insert("preview".into(), json!(r.preview));
    }
    if !r.notes.is_empty() {
        m.insert("notes".into(), json!(r.notes));
    }
    serde_json::to_string_pretty(&Value::Object(m)).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn row(out: &mut String, k: &str, v: String) {
    out.push_str(&format!("{:<14}{}\n", format!("{k}:"), v));
}

fn render_report(r: &Report) -> String {
    let mut out = String::new();
    row(&mut out, "Format", format!("{} ({})", r.label, r.format));
    row(&mut out, "Confidence", format!("{}%", r.confidence));
    if let Some(e) = &r.encoding {
        let bom = if e.bom { ", BOM present" } else { "" };
        row(
            &mut out,
            "Encoding",
            format!("{} ({}{})", e.label, e.source, bom),
        );
    }
    if let Some(le) = &r.line_ending {
        row(&mut out, "Line endings", line_ending_label(le).into());
    }
    if r.lines > 0 {
        row(
            &mut out,
            "Size",
            format!(
                "{} bytes, {} lines ({} sampled)",
                r.bytes, r.lines, r.sampled_lines
            ),
        );
    } else {
        row(&mut out, "Size", format!("{} bytes", r.bytes));
    }

    if let Some(d) = r.delimiter {
        out.push('\n');
        row(
            &mut out,
            "Delimiter",
            format!("{} ({})", delim_display(d), delim_name(d)),
        );
        row(
            &mut out,
            "Quote char",
            match r.quote_char {
                Some(q) => format!("{q}"),
                None => "none detected".into(),
            },
        );
    }
    if let Some(h) = &r.header {
        row(&mut out, "Header row", h.clone());
    }
    if let Some(c) = r.columns {
        row(&mut out, "Columns", c.to_string());
    }
    if let Some(n) = r.data_rows {
        row(&mut out, "Data rows", n.to_string());
    }
    for (k, v) in &r.extra {
        let pretty = k.replace('_', " ");
        let mut label = pretty.clone();
        if let Some(c) = label.get_mut(0..1) {
            c.make_ascii_uppercase();
        }
        let shown = match v {
            Value::String(s) => s.clone(),
            Value::Array(a) => a
                .iter()
                .map(|x| match x {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect::<Vec<_>>()
                .join(", "),
            other => other.to_string(),
        };
        row(&mut out, &label, shown);
    }

    if !r.delimiter_scores.is_empty() {
        out.push_str("\nDelimiter scores:\n");
        for (d, cols, cons) in &r.delimiter_scores {
            out.push_str(&format!(
                "  {:<8}{:<12}{:>3} cols  {:>3}% consistent\n",
                delim_display(*d),
                delim_name(*d),
                cols,
                cons
            ));
        }
    }

    if !r.column_names.is_empty() {
        out.push_str("\nColumns:\n");
        let w = r.column_names.iter().map(|s| s.len()).max().unwrap_or(0);
        for (i, name) in r.column_names.iter().enumerate() {
            let ty = r.column_types.get(i).cloned().unwrap_or_default();
            if ty.is_empty() {
                out.push_str(&format!("  {:>2}  {}\n", i + 1, name));
            } else {
                out.push_str(&format!("  {:>2}  {:<w$}  {}\n", i + 1, name, ty, w = w));
            }
        }
    }

    if !r.preview.is_empty() {
        out.push_str("\nPreview:\n");
        for line in table_rows(&r.preview) {
            out.push_str(&format!("  {line}\n"));
        }
    }

    if !r.notes.is_empty() {
        out.push_str("\nNotes:\n");
        for n in &r.notes {
            out.push_str(&format!("  - {n}\n"));
        }
    }
    out.trim_end().to_string()
}

fn table_rows(rows: &[Vec<String>]) -> Vec<String> {
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut widths = vec![0usize; cols];
    for r in rows {
        for (i, c) in r.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count().min(40));
        }
    }
    rows.iter()
        .map(|r| {
            r.iter()
                .enumerate()
                .map(|(i, c)| {
                    let mut s: String = c.chars().take(40).collect();
                    let pad = widths[i].saturating_sub(s.chars().count());
                    s.push_str(&" ".repeat(pad));
                    s
                })
                .collect::<Vec<_>>()
                .join(" | ")
                .trim_end()
                .to_string()
        })
        .collect()
}

// ---------------------------------------------------------------- inputs ---

fn norm_opt(v: &str, default: &str) -> String {
    let t = v.trim().to_ascii_lowercase();
    if t.is_empty() {
        default.to_string()
    } else {
        t
    }
}

fn decode_input(data: &str, form: &str) -> Result<Vec<u8>, String> {
    match form {
        "text" => Ok(data.as_bytes().to_vec()),
        "base64" => {
            let cleaned: String = data
                .chars()
                .filter(|c| !c.is_whitespace())
                .map(|c| match c {
                    '-' => '+',
                    '_' => '/',
                    other => other,
                })
                .filter(|c| *c != '=')
                .collect();
            if cleaned.is_empty() {
                return Err("no data to analyse: the input is empty".into());
            }
            use base64::Engine;
            base64::engine::general_purpose::STANDARD_NO_PAD
                .decode(cleaned.as_bytes())
                .map_err(|e| {
                    format!("input_form=base64 but the data is not valid base64: {e}")
                })
        }
        _ => {
            let cleaned: String = data
                .chars()
                .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-' && *c != ',')
                .collect();
            let cleaned = cleaned
                .strip_prefix("0x")
                .or_else(|| cleaned.strip_prefix("0X"))
                .unwrap_or(&cleaned)
                .to_string();
            if cleaned.is_empty() {
                return Err("no data to analyse: the input is empty".into());
            }
            if cleaned.len() % 2 != 0 {
                return Err(format!(
                    "input_form=hex but the data has an odd number of hex digits ({}) — expected \
                     two digits per byte",
                    cleaned.len()
                ));
            }
            let b = cleaned.as_bytes();
            let mut out = Vec::with_capacity(b.len() / 2);
            for i in (0..b.len()).step_by(2) {
                let hi = hex_val(b[i])?;
                let lo = hex_val(b[i + 1])?;
                out.push(hi << 4 | lo);
            }
            Ok(out)
        }
    }
}

fn hex_val(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!(
            "input_form=hex but the data contains a non-hex character '{}' — expected 0-9 a-f",
            c as char
        )),
    }
}

// ------------------------------------------------------------ magic bytes ---

struct Magic {
    slug: &'static str,
    label: &'static str,
    note: &'static str,
    confidence: u32,
}

fn binary_magic(b: &[u8]) -> Option<Magic> {
    if b.starts_with(b"PAR1") {
        let both = b.len() >= 8 && b.ends_with(b"PAR1");
        return Some(Magic {
            slug: "parquet",
            label: "Apache Parquet".into(),
            note: if both {
                "The PAR1 marker is present at both the start and the end of the input."
            } else {
                "The file starts with the PAR1 marker; the closing marker was not in this sample."
            },
            confidence: if both { 100 } else { 90 },
        });
    }
    if b.starts_with(b"Obj\x01") {
        return Some(Magic {
            slug: "avro",
            label: "Apache Avro object container file",
            note: "The input starts with the Avro object-container signature Obj\\x01.",
            confidence: 100,
        });
    }
    if b.starts_with(b"SQLite format 3\0") {
        return Some(Magic {
            slug: "sqlite",
            label: "SQLite database",
            note: "The input starts with the SQLite 3 header string.",
            confidence: 100,
        });
    }
    if b.starts_with(b"PK\x03\x04") || b.starts_with(b"PK\x05\x06") {
        return Some(Magic {
            slug: "zip",
            label: "ZIP container (also used by XLSX, ODS, DOCX)",
            note: "The input starts with a ZIP local-file signature.",
            confidence: 95,
        });
    }
    if b.starts_with(&[0x1f, 0x8b]) {
        return Some(Magic {
            slug: "gzip",
            label: "gzip-compressed data",
            note: "The input starts with the gzip signature; decompress it before sniffing.",
            confidence: 95,
        });
    }
    if b.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        return Some(Magic {
            slug: "zstd",
            label: "Zstandard-compressed data",
            note: "The input starts with the zstd frame magic; decompress it before sniffing.",
            confidence: 95,
        });
    }
    if b.starts_with(b"ORC") {
        return Some(Magic {
            slug: "orc",
            label: "Apache ORC",
            note: "The input starts with the ORC signature.",
            confidence: 90,
        });
    }
    None
}

// --------------------------------------------------------------- encoding ---

fn detect_encoding(bytes: &[u8], form: &str) -> EncodingInfo {
    // UTF-32LE must be checked before UTF-16LE — they share the FF FE prefix.
    let boms: [(&[u8], &str); 5] = [
        (&[0xFF, 0xFE, 0x00, 0x00], "utf-32le"),
        (&[0x00, 0x00, 0xFE, 0xFF], "utf-32be"),
        (&[0xEF, 0xBB, 0xBF], "utf-8"),
        (&[0xFF, 0xFE], "utf-16le"),
        (&[0xFE, 0xFF], "utf-16be"),
    ];
    for (sig, label) in boms {
        if bytes.starts_with(sig) {
            return EncodingInfo {
                label: label.into(),
                bom: true,
                source: "byte-order mark".into(),
            };
        }
    }
    if form == "text" {
        return EncodingInfo {
            label: "utf-8".into(),
            bom: false,
            source: "declared: input pasted as text".into(),
        };
    }
    let mut det = chardetng::EncodingDetector::new();
    det.feed(bytes, true);
    let enc = det.guess(None, true);
    EncodingInfo {
        label: enc.name().to_ascii_lowercase(),
        bom: false,
        source: "statistical detection".into(),
    }
}

fn decode_text(bytes: &[u8], label: &str) -> Result<(String, usize), String> {
    if label.starts_with("utf-32") {
        let le = label.ends_with("le");
        let body = if bytes.len() >= 4 { &bytes[4..] } else { &bytes[..] };
        if body.len() % 4 != 0 {
            return Err(format!(
                "input is {label} but its length is not a multiple of 4 bytes"
            ));
        }
        let mut s = String::new();
        let mut bad = 0usize;
        for c in body.chunks_exact(4) {
            let v = if le {
                u32::from_le_bytes([c[0], c[1], c[2], c[3]])
            } else {
                u32::from_be_bytes([c[0], c[1], c[2], c[3]])
            };
            match char::from_u32(v) {
                Some(ch) => s.push(ch),
                None => {
                    bad += 1;
                    s.push('\u{FFFD}');
                }
            }
        }
        return Ok((s, bad));
    }
    let enc = encoding_rs::Encoding::for_label(label.as_bytes()).ok_or_else(|| {
        format!("detected encoding \"{label}\" is not a known character encoding")
    })?;
    let (cow, _, _) = enc.decode(bytes);
    let replacements = cow.chars().filter(|c| *c == '\u{FFFD}').count();
    Ok((cow.into_owned(), replacements))
}

struct LineEndings {
    kind: &'static str,
    crlf: usize,
    lf: usize,
    cr: usize,
}

fn detect_line_endings(t: &str) -> LineEndings {
    let crlf = t.matches("\r\n").count();
    let lf = t.matches('\n').count() - crlf;
    let cr = t.matches('\r').count() - crlf;
    let kinds = [(crlf, "crlf"), (lf, "lf"), (cr, "cr")];
    let present: Vec<&str> = kinds.iter().filter(|(n, _)| *n > 0).map(|(_, k)| *k).collect();
    let kind = match present.len() {
        0 => "none",
        1 => present[0],
        _ => "mixed",
    };
    LineEndings { kind, crlf, lf, cr }
}

fn line_ending_label(k: &str) -> &'static str {
    match k {
        "lf" => "LF (\\n, Unix)",
        "crlf" => "CRLF (\\r\\n, Windows)",
        "cr" => "CR (\\r, classic Mac)",
        "mixed" => "mixed",
        _ => "none (single line)",
    }
}

// -------------------------------------------------------------- structure ---

fn whole_json(t: &str) -> Option<Value> {
    let s = t.trim();
    if !(s.starts_with('{') || s.starts_with('[')) {
        return None;
    }
    serde_json::from_str::<Value>(s).ok()
}

fn fill_json(r: &mut Report, v: &Value) {
    r.format = "json".into();
    r.confidence = 100;
    match v {
        Value::Array(a) => {
            r.label = "JSON document (top-level array)".into();
            r.extra.push(("json_root".into(), json!("array")));
            r.extra.push(("json_items".into(), json!(a.len())));
            if let Some(Value::Object(o)) = a.first() {
                let keys: Vec<String> = o.keys().take(20).cloned().collect();
                r.extra.push(("json_keys".into(), json!(keys)));
                r.columns = Some(o.len());
            }
        }
        Value::Object(o) => {
            r.label = "JSON document (top-level object)".into();
            r.extra.push(("json_root".into(), json!("object")));
            let keys: Vec<String> = o.keys().take(20).cloned().collect();
            r.extra.push(("json_keys".into(), json!(keys)));
        }
        _ => {
            r.label = "JSON document".into();
        }
    }
}

fn jsonl_records(sample: &[&str]) -> Option<usize> {
    let lines: Vec<&&str> = sample.iter().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() < 2 {
        return None;
    }
    let mut structured = 0usize;
    for l in &lines {
        let t = l.trim();
        let v: Value = serde_json::from_str(t).ok()?;
        if matches!(v, Value::Object(_) | Value::Array(_)) {
            structured += 1;
        }
    }
    if structured == 0 {
        return None;
    }
    Some(lines.len())
}

fn markdown_table(sample: &[&str]) -> Option<usize> {
    let rows: Vec<&&str> = sample.iter().filter(|l| !l.trim().is_empty()).collect();
    if rows.len() < 2 {
        return None;
    }
    let head = rows[0].trim();
    let sep = rows[1].trim();
    if !head.contains('|') {
        return None;
    }
    if sep.len() < 3
        || !sep.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
        || sep.matches('-').count() < 3
    {
        return None;
    }
    let cols = head
        .trim_matches('|')
        .split('|')
        .filter(|s| !s.trim().is_empty() || true)
        .count();
    Some(cols)
}

fn is_html(t: &str) -> bool {
    let head: String = t.trim_start().chars().take(400).collect::<String>().to_ascii_lowercase();
    head.starts_with("<!doctype html")
        || head.starts_with("<html")
        || (head.starts_with('<') && (head.contains("<body") || head.contains("<div") || head.contains("<head")))
}

fn xml_info(t: &str) -> Option<(String, usize)> {
    let s = t.trim();
    if !s.starts_with('<') {
        return None;
    }
    let mut reader = quick_xml::Reader::from_str(s);
    reader.config_mut().check_end_names = true;
    let mut buf_root: Option<String> = None;
    let mut elems = 0usize;
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(e)) | Ok(quick_xml::events::Event::Empty(e)) => {
                elems += 1;
                if buf_root.is_none() {
                    buf_root = Some(String::from_utf8_lossy(e.name().as_ref()).into_owned());
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    buf_root.map(|r| (r, elems))
}

fn yaml_marker(sample: &[&str]) -> bool {
    for l in sample {
        let t = l.trim();
        if t.is_empty() {
            continue;
        }
        return t == "---" || t.starts_with("%YAML");
    }
    false
}

// -------------------------------------------------------------- delimited ---

#[derive(Debug)]
struct Parsed {
    records: Vec<Vec<String>>,
    start_lines: Vec<usize>,
    modal: usize,
    consistency: f64,
    quoted_fields: usize,
}

fn parse_records(text: &str, delim: char, quote: Option<char>) -> Parsed {
    let mut records: Vec<Vec<String>> = Vec::new();
    let mut start_lines: Vec<usize> = Vec::new();
    let mut rec: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut field_quoted = false;
    let mut quoted_fields = 0usize;
    let mut line = 1usize;
    let mut rec_line = 1usize;
    let mut started = false;
    let mut it = text.chars().peekable();

    while let Some(c) = it.next() {
        if in_quotes {
            if Some(c) == quote {
                if it.peek().copied() == quote {
                    it.next();
                    field.push(c);
                } else {
                    in_quotes = false;
                }
            } else {
                if c == '\n' {
                    line += 1;
                }
                field.push(c);
            }
            continue;
        }
        if Some(c) == quote && field.trim().is_empty() {
            in_quotes = true;
            field_quoted = true;
            field.clear();
            if !started {
                started = true;
                rec_line = line;
            }
            continue;
        }
        if c == delim {
            rec.push(std::mem::take(&mut field));
            if field_quoted {
                quoted_fields += 1;
                field_quoted = false;
            }
            if !started {
                started = true;
                rec_line = line;
            }
            continue;
        }
        if c == '\n' {
            if started || !field.trim().is_empty() || !rec.is_empty() {
                rec.push(std::mem::take(&mut field));
                if field_quoted {
                    quoted_fields += 1;
                    field_quoted = false;
                }
                if !(rec.len() == 1 && rec[0].trim().is_empty()) {
                    records.push(std::mem::take(&mut rec));
                    start_lines.push(if started { rec_line } else { line });
                } else {
                    rec.clear();
                }
            }
            started = false;
            line += 1;
            continue;
        }
        if !started && !c.is_whitespace() {
            started = true;
            rec_line = line;
        }
        field.push(c);
    }
    if started || !field.is_empty() || !rec.is_empty() {
        rec.push(field);
        if field_quoted {
            quoted_fields += 1;
        }
        if !(rec.len() == 1 && rec[0].trim().is_empty()) {
            records.push(rec);
            start_lines.push(rec_line);
        }
    }

    let mut counts: Vec<usize> = records.iter().map(|r| r.len()).collect();
    counts.sort_unstable();
    let modal = modal_of(&counts);
    let matching = records.iter().filter(|r| r.len() == modal).count();
    let consistency = if records.is_empty() {
        0.0
    } else {
        matching as f64 / records.len() as f64
    };
    Parsed {
        records,
        start_lines,
        modal,
        consistency,
        quoted_fields,
    }
}

fn modal_of(sorted: &[usize]) -> usize {
    let mut best = 0usize;
    let mut best_n = 0usize;
    let mut i = 0usize;
    while i < sorted.len() {
        let v = sorted[i];
        let mut j = i;
        while j < sorted.len() && sorted[j] == v {
            j += 1;
        }
        let n = j - i;
        // Ties go to the LARGER column count (more columns = a better dialect,
        // the same tie-break the reference sniffers use).
        if n >= best_n {
            best_n = n;
            best = v;
        }
        i = j;
    }
    best
}

struct DelimResult {
    delim: char,
    quote: Option<char>,
    parsed: Parsed,
    scores: Vec<(char, usize, u32)>,
}

fn candidates(extra: &str) -> Result<Vec<char>, String> {
    let mut v: Vec<char> = DEFAULT_DELIMITERS.to_vec();
    for c in extra.chars() {
        if c == '\n' || c == '\r' {
            return Err("extra_delimiters cannot contain a newline".into());
        }
        if c == '"' || c == '\'' {
            return Err(format!(
                "extra_delimiters cannot contain the quote character {c} — quotes are detected \
                 separately"
            ));
        }
        if !v.contains(&c) {
            v.push(c);
        }
    }
    Ok(v)
}

fn best_delimited(sample_text: &str, extra: &str) -> Result<Option<DelimResult>, String> {
    let cands = candidates(extra)?;
    let mut scores: Vec<(char, usize, u32)> = Vec::new();
    let mut best: Option<(f64, DelimResult)> = None;

    for d in cands {
        let dq = parse_records(sample_text, d, Some('"'));
        let sq = parse_records(sample_text, d, Some('\''));
        let (parsed, quote) = if sq.quoted_fields > dq.quoted_fields && sq.consistency >= dq.consistency
        {
            (sq, Some('\''))
        } else {
            let q = if dq.quoted_fields > 0 { Some('"') } else { None };
            (dq, q)
        };
        scores.push((
            d,
            parsed.modal,
            (parsed.consistency * 100.0).round() as u32,
        ));
        if parsed.modal < 2 {
            continue;
        }
        // Space is the noisiest candidate (prose splits cleanly on it), so it
        // needs near-perfect consistency and more than one record.
        let floor = if d == ' ' { 0.9 } else { 0.6 };
        if parsed.consistency < floor || (d == ' ' && parsed.records.len() < 2) {
            continue;
        }
        let score = parsed.consistency * 100.0 + (parsed.modal.min(30) as f64) * 0.1;
        let better = match &best {
            None => true,
            Some((s, _)) => score > *s + 1e-9,
        };
        if better {
            best = Some((
                score,
                DelimResult {
                    delim: d,
                    quote,
                    parsed,
                    scores: Vec::new(),
                },
            ));
        }
    }
    Ok(best.map(|(_, mut r)| {
        r.scores = scores;
        r
    }))
}

fn fill_delimited(r: &mut Report, d: &DelimResult, o: &Options, notes: &mut Vec<String>) {
    let (slug, label) = match d.delim {
        ',' => ("csv", "CSV (comma-separated values)"),
        '\t' => ("tsv", "TSV (tab-separated values)"),
        ';' => ("ssv", "Semicolon-separated values"),
        '|' => ("psv", "Pipe-separated values"),
        _ => ("delimited", "Delimited text"),
    };
    r.format = slug.into();
    r.label = label.into();
    r.delimiter = Some(d.delim);
    r.quote_char = d.quote;
    r.columns = Some(d.parsed.modal);
    r.delimiter_scores = d.scores.clone();

    let mut confidence = (d.parsed.consistency * 100.0).round() as u32;
    if d.parsed.records.len() < 2 {
        confidence = confidence.min(70);
        notes.push(
            "Only one record was found; the delimiter guess comes from a single line.".into(),
        );
    }
    r.confidence = confidence;

    let ragged: Vec<usize> = d
        .parsed
        .records
        .iter()
        .enumerate()
        .filter(|(_, rec)| rec.len() != d.parsed.modal)
        .map(|(i, _)| d.parsed.start_lines.get(i).copied().unwrap_or(i + 1))
        .collect();
    if !ragged.is_empty() {
        let shown: Vec<String> = ragged.iter().take(5).map(|l| l.to_string()).collect();
        notes.push(format!(
            "{} row(s) do not have {} columns (first at line {}{}).",
            ragged.len(),
            d.parsed.modal,
            shown.join(", line "),
            if ragged.len() > 5 { ", …" } else { "" }
        ));
    }
    if d.parsed.records.iter().any(|rec| rec.iter().any(|f| f.contains('\n'))) {
        notes.push("At least one quoted field spans multiple lines.".into());
    }

    let cols = d.parsed.modal;
    let rows: Vec<&Vec<String>> = d.parsed.records.iter().filter(|r| r.len() == cols).collect();

    let mut header = "unknown".to_string();
    let mut types: Vec<String> = Vec::new();
    if o.detect_types && rows.len() >= 2 {
        let data = &rows[1..];
        let mut mismatch = false;
        let mut typed = false;
        for j in 0..cols {
            let vals: Vec<&str> = data.iter().map(|r| r[j].trim()).collect();
            let ty = column_type(&vals);
            types.push(ty.name().into());
            if ty != ColType::Text && ty != ColType::Null {
                typed = true;
                let first = rows[0][j].trim();
                if !first.is_empty() && type_mask(first) & ty.bit() == 0 {
                    mismatch = true;
                }
            }
        }
        header = if mismatch {
            "likely".into()
        } else if typed {
            "unlikely".into()
        } else {
            notes.push(
                "Every column is text, so the header row could not be confirmed by type mismatch."
                    .into(),
            );
            "unknown".into()
        };
        if header != "likely" {
            // Types were inferred from rows 2.. — recompute over every row so
            // the reported types describe the data as a whole.
            types.clear();
            for j in 0..cols {
                let vals: Vec<&str> = rows.iter().map(|r| r[j].trim()).collect();
                types.push(column_type(&vals).name().into());
            }
        }
    } else if !o.detect_types {
        notes.push(
            "Column types and header detection are off (detect_types=false).".into(),
        );
    } else {
        notes.push("Too few rows to detect a header row.".into());
    }

    let has_header = header == "likely";
    r.header = Some(header);
    r.data_rows = Some(if has_header {
        rows.len().saturating_sub(1)
    } else {
        rows.len()
    });
    r.column_names = if has_header {
        rows[0]
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let t = c.trim();
                if t.is_empty() {
                    format!("column_{}", i + 1)
                } else {
                    t.to_string()
                }
            })
            .collect()
    } else {
        (1..=cols).map(|i| format!("column_{i}")).collect()
    };
    r.column_types = types;

    if o.preview_rows > 0 {
        r.preview = d
            .parsed
            .records
            .iter()
            .take(o.preview_rows)
            .map(|rec| rec.iter().map(|f| f.replace('\n', "\\n")).collect())
            .collect();
    }
}

// ------------------------------------------------------------ column types ---

const T_BOOL: u8 = 1;
const T_INT: u8 = 2;
const T_FLOAT: u8 = 4;
const T_TIME: u8 = 8;
const T_DATE: u8 = 16;
const T_DATETIME: u8 = 32;
const T_TEXT: u8 = 64;
const T_ALL: u8 = 127;

#[derive(PartialEq, Clone, Copy, Debug)]
enum ColType {
    Null,
    Boolean,
    Integer,
    Float,
    Time,
    Date,
    DateTime,
    Text,
}

impl ColType {
    fn name(self) -> &'static str {
        match self {
            ColType::Null => "null",
            ColType::Boolean => "boolean",
            ColType::Integer => "integer",
            ColType::Float => "float",
            ColType::Time => "time",
            ColType::Date => "date",
            ColType::DateTime => "datetime",
            ColType::Text => "text",
        }
    }
    fn bit(self) -> u8 {
        match self {
            ColType::Null => T_ALL,
            ColType::Boolean => T_BOOL,
            ColType::Integer => T_INT,
            ColType::Float => T_FLOAT,
            ColType::Time => T_TIME,
            ColType::Date => T_DATE,
            ColType::DateTime => T_DATETIME,
            ColType::Text => T_TEXT,
        }
    }
}

fn type_mask(s: &str) -> u8 {
    let t = s.trim();
    if t.is_empty() {
        return T_ALL;
    }
    let mut m = T_TEXT;
    let low = t.to_ascii_lowercase();
    if matches!(low.as_str(), "true" | "false" | "yes" | "no") {
        m |= T_BOOL;
    }
    if is_integer(t) {
        m |= T_INT | T_FLOAT;
    } else if t.parse::<f64>().map(|v| v.is_finite()).unwrap_or(false) {
        m |= T_FLOAT;
    }
    if is_time(t) {
        m |= T_TIME;
    }
    if is_date(t) {
        m |= T_DATE;
    }
    if is_datetime(t) {
        m |= T_DATETIME;
    }
    m
}

fn column_type(values: &[&str]) -> ColType {
    let mut mask = T_ALL;
    let mut any = false;
    for v in values {
        if v.trim().is_empty() {
            continue;
        }
        any = true;
        mask &= type_mask(v);
    }
    if !any {
        return ColType::Null;
    }
    for t in [
        ColType::Boolean,
        ColType::Integer,
        ColType::Float,
        ColType::Time,
        ColType::Date,
        ColType::DateTime,
    ] {
        if mask & t.bit() != 0 {
            return t;
        }
    }
    ColType::Text
}

fn is_integer(t: &str) -> bool {
    let b = t.strip_prefix(['+', '-']).unwrap_or(t);
    !b.is_empty() && b.len() <= 19 && b.bytes().all(|c| c.is_ascii_digit())
}

fn is_time(t: &str) -> bool {
    let parts: Vec<&str> = t.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return false;
    }
    let sec = parts.last().unwrap().split('.').next().unwrap_or("");
    let nums = [parts[0], parts[1], sec];
    let bounds = [23u32, 59, 59];
    for (i, p) in nums.iter().enumerate().take(parts.len()) {
        if p.is_empty() || p.len() > 2 || !p.bytes().all(|c| c.is_ascii_digit()) {
            return false;
        }
        if p.parse::<u32>().unwrap_or(99) > bounds[i] {
            return false;
        }
    }
    true
}

fn is_date(t: &str) -> bool {
    let sep = if t.contains('-') {
        '-'
    } else if t.contains('/') {
        '/'
    } else if t.contains('.') {
        '.'
    } else {
        return false;
    };
    let p: Vec<&str> = t.split(sep).collect();
    if p.len() != 3 || p.iter().any(|s| s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit())) {
        return false;
    }
    let n: Vec<u32> = p.iter().filter_map(|s| s.parse().ok()).collect();
    if n.len() != 3 {
        return false;
    }
    if p[0].len() == 4 {
        n[1] >= 1 && n[1] <= 12 && n[2] >= 1 && n[2] <= 31
    } else if p[2].len() == 4 {
        n[0] >= 1 && n[0] <= 31 && n[1] >= 1 && n[1] <= 31 && (n[0] <= 12 || n[1] <= 12)
    } else {
        false
    }
}

fn is_datetime(t: &str) -> bool {
    let body = t.trim_end_matches('Z');
    let (d, rest) = match body.split_once('T').or_else(|| body.split_once(' ')) {
        Some(v) => v,
        None => return false,
    };
    if !is_date(d) {
        return false;
    }
    let time = rest
        .split(['+', 'Z'])
        .next()
        .unwrap_or(rest)
        .rsplit_once('-')
        .map(|(a, _)| a)
        .unwrap_or(rest);
    is_time(time.trim()) || is_time(rest.trim())
}

// ------------------------------------------------------------ fixed width ---

fn fixed_width(sample: &[&str]) -> Option<Vec<(usize, usize)>> {
    let lines: Vec<Vec<char>> = sample
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.chars().collect())
        .collect();
    if lines.len() < 3 {
        return None;
    }
    let width = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    if width < 4 {
        return None;
    }
    let is_space = |p: usize| lines.iter().all(|l| l.get(p).copied().unwrap_or(' ') == ' ');
    let mut fields: Vec<(usize, usize)> = Vec::new();
    let mut start: Option<usize> = None;
    let mut run = 0usize;
    for p in 0..width {
        if is_space(p) {
            run += 1;
            if run >= 2 {
                if let Some(s) = start.take() {
                    fields.push((s, p + 1 - run));
                }
            }
        } else {
            run = 0;
            if start.is_none() {
                start = Some(p);
            }
        }
    }
    if let Some(s) = start.take() {
        fields.push((s, width));
    }
    if fields.len() < 2 {
        return None;
    }
    Some(fields)
}

// ------------------------------------------------------------- delimiters ---

fn delim_name(c: char) -> &'static str {
    match c {
        ',' => "comma",
        '\t' => "tab",
        ';' => "semicolon",
        '|' => "pipe",
        ':' => "colon",
        '~' => "tilde",
        ' ' => "space",
        '^' => "caret",
        '#' => "hash",
        _ => "custom",
    }
}

fn delim_display(c: char) -> String {
    match c {
        '\t' => "\"\\t\"".into(),
        ' ' => "\" \"".into(),
        other => format!("\"{other}\""),
    }
}

// ------------------------------------------------------------------ tests ---

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    fn json_of(data: &str, o: &Options) -> Value {
        let mut o = o.clone();
        o.output = "json".into();
        serde_json::from_str(&sniff(data, &o).unwrap()).unwrap()
    }

    #[test]
    fn detects_csv_with_header_and_types() {
        let v = json_of(
            "name,age,city,joined\nAda,36,London,1815-12-10\nAlan,41,Wilmslow,1912-06-23\nGrace,45,New York,1906-12-09",
            &opts(),
        );
        assert_eq!(v["format"], "csv");
        assert_eq!(v["delimiter"], ",");
        assert_eq!(v["delimiter_name"], "comma");
        assert_eq!(v["confidence"], 100);
        assert_eq!(v["columns"], 4);
        assert_eq!(v["header"], "likely");
        assert_eq!(v["data_rows"], 3);
        assert_eq!(v["column_names"][0], "name");
        assert_eq!(v["column_types"][1], "integer");
        assert_eq!(v["column_types"][3], "date");
        assert_eq!(v["line_ending"], "lf");
        assert_eq!(v["encoding"]["label"], "utf-8");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = sniff("", &opts()).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn rejects_unknown_input_form() {
        let mut o = opts();
        o.input_form = "yaml".into();
        let err = sniff("a,b", &o).unwrap_err();
        assert!(err.contains("input_form must be text, base64, or hex"), "{err}");
    }

    #[test]
    fn rejects_oversized_input() {
        let big = "a".repeat(MAX_BYTES + 1);
        let err = sniff(&big, &opts()).unwrap_err();
        assert!(err.contains("input too large"), "{err}");
        // Exactly at the cap is accepted.
        let at = "a".repeat(MAX_BYTES);
        assert!(sniff(&at, &opts()).is_ok());
    }

    #[test]
    fn detects_tsv_and_crlf() {
        let v = json_of("a\tb\tc\r\n1\t2\t3\r\n4\t5\t6\r\n", &opts());
        assert_eq!(v["format"], "tsv");
        assert_eq!(v["delimiter"], "\t");
        assert_eq!(v["line_ending"], "crlf");
        assert_eq!(v["columns"], 3);
    }

    #[test]
    fn detects_semicolon_and_pipe() {
        assert_eq!(json_of("a;b;c\n1;2;3\n4;5;6", &opts())["format"], "ssv");
        assert_eq!(json_of("a|b|c\n1|2|3\n4|5|6", &opts())["format"], "psv");
    }

    #[test]
    fn detects_json_object_and_array() {
        let v = json_of("{\"a\": 1, \"b\": [2,3]}", &opts());
        assert_eq!(v["format"], "json");
        assert_eq!(v["json_root"], "object");
        let v = json_of("[{\"a\":1},{\"a\":2}]", &opts());
        assert_eq!(v["format"], "json");
        assert_eq!(v["json_root"], "array");
        assert_eq!(v["json_items"], 2);
    }

    #[test]
    fn detects_jsonl() {
        let v = json_of("{\"a\":1}\n{\"a\":2}\n{\"a\":3}", &opts());
        assert_eq!(v["format"], "jsonl");
        assert_eq!(v["json_records"], 3);
    }

    #[test]
    fn detects_xml_and_html_and_markdown() {
        let v = json_of("<?xml version=\"1.0\"?>\n<root><row id=\"1\"/></root>", &opts());
        assert_eq!(v["format"], "xml");
        assert_eq!(v["xml_root"], "root");
        assert_eq!(json_of("<!DOCTYPE html>\n<html><body>hi</body></html>", &opts())["format"], "html");
        let v = json_of("| a | b |\n| --- | --- |\n| 1 | 2 |", &opts());
        assert_eq!(v["format"], "markdown-table");
    }

    #[test]
    fn detects_quoted_fields_and_embedded_newlines() {
        let v = json_of(
            "name,note\n\"Ada\",\"line one\nline two\"\n\"Alan\",\"plain\"",
            &opts(),
        );
        assert_eq!(v["format"], "csv");
        assert_eq!(v["quote_char"], "\"");
        assert_eq!(v["columns"], 2);
        let notes = v["notes"].as_array().unwrap();
        assert!(notes.iter().any(|n| n.as_str().unwrap().contains("multiple lines")));
    }

    #[test]
    fn reports_ragged_rows() {
        let v = json_of("a,b,c\n1,2,3\n4,5\n6,7,8", &opts());
        let notes = v["notes"].as_array().unwrap();
        assert!(
            notes.iter().any(|n| n.as_str().unwrap().contains("do not have 3 columns")),
            "{notes:?}"
        );
    }

    #[test]
    fn extra_delimiter_is_used() {
        let mut o = opts();
        o.extra_delimiters = "^".into();
        let v = json_of("a^b^c\n1^2^3\n4^5^6", &o);
        assert_eq!(v["format"], "delimited");
        assert_eq!(v["delimiter"], "^");
        assert_eq!(v["columns"], 3);
    }

    #[test]
    fn comment_prefix_skips_lines() {
        let mut o = opts();
        o.comment_prefix = "#".into();
        let v = json_of("# generated\n# by hand\na,b\n1,2\n3,4", &o);
        assert_eq!(v["format"], "csv");
        assert_eq!(v["columns"], 2);
        assert!(v["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n.as_str().unwrap().contains("comment line")));
    }

    #[test]
    fn detect_types_off_skips_types_and_header() {
        let mut o = opts();
        o.detect_types = false;
        let v = json_of("name,age\nAda,36\nAlan,41", &o);
        assert_eq!(v["format"], "csv");
        assert_eq!(v["header"], "unknown");
        assert!(v.get("column_types").is_none());
    }

    #[test]
    fn base64_input_detects_parquet() {
        // "PAR1" + filler + "PAR1"
        let mut bytes = b"PAR1".to_vec();
        bytes.extend_from_slice(&[0u8; 16]);
        bytes.extend_from_slice(b"PAR1");
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let mut o = opts();
        o.input_form = "base64".into();
        let v = json_of(&b64, &o);
        assert_eq!(v["format"], "parquet");
        assert_eq!(v["confidence"], 100);
    }

    #[test]
    fn hex_input_detects_avro_and_gzip() {
        let mut o = opts();
        o.input_form = "hex".into();
        assert_eq!(json_of("4f 62 6a 01 00 00", &o)["format"], "avro");
        assert_eq!(json_of("1f8b080000000000", &o)["format"], "gzip");
    }

    #[test]
    fn hex_input_rejects_odd_length() {
        let mut o = opts();
        o.input_form = "hex".into();
        let err = sniff("4f6", &o).unwrap_err();
        assert!(err.contains("odd number of hex digits"), "{err}");
    }

    #[test]
    fn base64_bytes_detect_latin1_encoding() {
        // "naïve;café" in windows-1252 repeated so the detector has signal.
        let mut raw: Vec<u8> = Vec::new();
        for _ in 0..80 {
            raw.extend_from_slice(&[
                b'n', b'a', 0xEF, b'v', b'e', b';', b'c', b'a', b'f', 0xE9, b'\n',
            ]);
        }
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&raw);
        let mut o = opts();
        o.input_form = "base64".into();
        let v = json_of(&b64, &o);
        assert_eq!(v["encoding"]["source"], "statistical detection");
        assert_ne!(v["encoding"]["label"], "utf-8");
        assert_eq!(v["format"], "ssv");
    }

    #[test]
    fn utf16le_bom_is_decoded() {
        let mut raw: Vec<u8> = vec![0xFF, 0xFE];
        for ch in "a,b\n1,2\n3,4".encode_utf16() {
            raw.extend_from_slice(&ch.to_le_bytes());
        }
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&raw);
        let mut o = opts();
        o.input_form = "base64".into();
        let v = json_of(&b64, &o);
        assert_eq!(v["encoding"]["label"], "utf-16le");
        assert_eq!(v["encoding"]["bom"], true);
        assert_eq!(v["format"], "csv");
    }

    #[test]
    fn detects_fixed_width() {
        let v = json_of(
            "Ada     36  London\nAlan    41  Wilmslow\nGrace   45  New York\nEdsger  72  Austin",
            &opts(),
        );
        assert_eq!(v["format"], "fixed-width");
        assert_eq!(v["columns"], 3);
    }

    #[test]
    fn plain_prose_is_text() {
        let v = json_of(
            "The quick brown fox jumps over the lazy dog.\nIt was a bright cold day in April and everyone was busy.\nNothing here is tabular at all, truly.",
            &opts(),
        );
        assert_eq!(v["format"], "text");
    }

    #[test]
    fn sample_lines_limits_the_scan() {
        let mut data = String::from("a,b\n");
        for i in 0..50 {
            data.push_str(&format!("{i},{i}\n"));
        }
        let mut o = opts();
        o.sample_lines = 5;
        let v = json_of(&data, &o);
        assert_eq!(v["sampled_lines"], 5);
        assert_eq!(v["lines"], 51);
        assert!(v["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n.as_str().unwrap().contains("Only the first 5")));
    }

    #[test]
    fn rejects_bad_sample_lines_and_preview_rows() {
        let mut o = opts();
        o.sample_lines = 0;
        assert!(sniff("a,b", &o).unwrap_err().contains("sample_lines must be"));
        let mut o = opts();
        o.preview_rows = MAX_PREVIEW_ROWS + 1;
        assert!(sniff("a,b", &o).unwrap_err().contains("preview_rows must be"));
    }

    #[test]
    fn report_output_is_human_readable() {
        let out = sniff("a,b,c\n1,2,3\n4,5,6", &opts()).unwrap();
        assert!(out.starts_with("Format:"), "{out}");
        assert!(out.contains("Delimiter:"), "{out}");
        assert!(out.contains("Delimiter scores:"), "{out}");
        assert!(out.contains("Preview:"), "{out}");
    }

    #[test]
    fn yaml_marker_detected() {
        let v = json_of("---\nname: Ada\nage: 36\n", &opts());
        assert_eq!(v["format"], "yaml");
    }

    #[test]
    fn extra_delimiters_reject_quotes() {
        let mut o = opts();
        o.extra_delimiters = "\"".into();
        let err = sniff("a,b\n1,2", &o).unwrap_err();
        assert!(err.contains("quote character"), "{err}");
    }
}
