//! gizza-ai/markdown-table-extractor core — find every GitHub-flavored Markdown
//! table in a document and export the selected ones as CSV, JSON, JSON Lines, or
//! an inventory listing.
//!
//! Detection is GFM-strict: a table is a pipe-bearing header line followed by a
//! delimiter row (`---`, `:--`, `--:`, `:-:`) with the SAME cell count, then body
//! rows until a blank line or a line without a pipe. Pipe lines inside fenced code
//! blocks (``` / ~~~) are ignored, so prose documents full of snippets parse
//! correctly. Each table remembers the nearest preceding ATX heading and its
//! source line, which makes a multi-table export self-describing.
//!
//! Pure-Rust (`csv` for RFC-4180 quoting, `serde_json` with `preserve_order` so
//! JSON keys keep column order). No wafer/wasm-bindgen deps.

use serde_json::{json, Map, Value};

/// Largest accepted document, in bytes. Documented on the page.
pub const MAX_INPUT_BYTES: usize = 1_000_000;

/// Output format for the selected tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Delimited text (RFC-4180 quoting), one block per table.
    Csv,
    /// A JSON array of rows (one table) or of table envelopes (several).
    Json,
    /// JSON Lines — one JSON value per data row.
    Jsonl,
    /// An inventory of the tables found: index, heading, line, columns, alignments, row count.
    List,
}

/// Parse the `format` argument. Empty means the default, `csv`.
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "csv" => Ok(Format::Csv),
        "json" => Ok(Format::Json),
        "jsonl" | "jsonlines" | "ndjson" => Ok(Format::Jsonl),
        "list" => Ok(Format::List),
        other => Err(format!(
            "unknown format '{other}' (expected csv, json, jsonl or list)"
        )),
    }
}

/// CSV quoting policy for the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quote {
    /// Quote a field only when it contains the delimiter, a quote, or a newline.
    Minimal,
    /// Wrap every field in double quotes.
    All,
}

impl Quote {
    pub fn parse(s: &str) -> Result<Quote, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "minimal" | "necessary" | "auto" => Ok(Quote::Minimal),
            "all" | "always" => Ok(Quote::All),
            other => Err(format!(
                "unknown quote style '{other}' (expected minimal or all)"
            )),
        }
    }
    fn style(self) -> csv::QuoteStyle {
        match self {
            Quote::Minimal => csv::QuoteStyle::Necessary,
            Quote::All => csv::QuoteStyle::Always,
        }
    }
}

/// Column alignment declared by the delimiter row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Default,
    Left,
    Center,
    Right,
}

impl Align {
    fn name(self) -> &'static str {
        match self {
            Align::Default => "default",
            Align::Left => "left",
            Align::Center => "center",
            Align::Right => "right",
        }
    }
}

/// One table found in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// 0-based position of this table in the document.
    pub index: usize,
    /// Nearest preceding ATX heading (`# …`), if any.
    pub heading: Option<String>,
    /// 1-based source line of the header row.
    pub line: usize,
    /// Header cells, in order.
    pub columns: Vec<String>,
    /// Per-column alignment from the delimiter row.
    pub aligns: Vec<Align>,
    /// Body rows, reconciled to `columns.len()` cells each.
    pub rows: Vec<Vec<String>>,
}

/// Everything the exporter needs. Built by the caller from the tool's params.
#[derive(Debug, Clone)]
pub struct Options {
    pub format: Format,
    /// `all` (default), a 0-based index, or a comma list / range such as `0,2-3`.
    pub table: String,
    /// Keep the header row (CSV) / key rows by it (JSON, JSONL).
    pub header: bool,
    /// CSV field separator: a single char or comma/tab/semicolon/pipe/space.
    pub delimiter: String,
    pub quote: Quote,
    /// CRLF row endings instead of LF.
    pub crlf: bool,
    /// Trim whitespace padding inside each cell.
    pub trim: bool,
    /// Render inline Markdown (bold, code, links, `<br>`) as plain text.
    pub strip_formatting: bool,
    /// JSON indent width, 0 = minified. Clamped to 0..=8.
    pub json_indent: usize,
    /// Prefix each CSV block with a `# Table n` comment when several are exported.
    pub labels: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: Format::Csv,
            table: "all".into(),
            header: true,
            delimiter: ",".into(),
            quote: Quote::Minimal,
            crlf: false,
            trim: true,
            strip_formatting: false,
            json_indent: 2,
            labels: true,
        }
    }
}

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        " " | "space" => b' ',
        other => {
            let bytes = other.as_bytes();
            if bytes.len() == 1 {
                bytes[0]
            } else {
                return Err(format!(
                    "delimiter must be a single character or one of tab/comma/semicolon/pipe/space, got '{other}'"
                ));
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Inline Markdown → plain text
// ---------------------------------------------------------------------------

fn is_boundary(c: Option<char>) -> bool {
    c.map_or(true, |x| x.is_whitespace() || x.is_ascii_punctuation())
}

/// Render inline Markdown as plain text: unescape `\x`, unwrap code spans,
/// keep only the link/image text, drop emphasis markers, and turn `<br>` into a
/// space (other HTML tags are dropped). Whitespace is collapsed by the caller.
fn strip_inline(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            '\\' if i + 1 < b.len() && b[i + 1].is_ascii_punctuation() => {
                out.push(b[i + 1]);
                i += 2;
            }
            '`' => {
                let mut n = 0;
                while i + n < b.len() && b[i + n] == '`' {
                    n += 1;
                }
                let start = i + n;
                let mut j = start;
                let mut close = None;
                while j < b.len() {
                    if b[j] == '`' {
                        let mut m = 0;
                        while j + m < b.len() && b[j + m] == '`' {
                            m += 1;
                        }
                        if m == n {
                            close = Some(j);
                            break;
                        }
                        j += m;
                    } else {
                        j += 1;
                    }
                }
                match close {
                    Some(cl) => {
                        out.extend(&b[start..cl]);
                        i = cl + n;
                    }
                    // Unmatched run: drop the backticks, keep the rest.
                    None => i += n,
                }
            }
            '<' => {
                let gt = (i + 1..b.len()).find(|&k| b[k] == '>');
                let mut handled = false;
                if let Some(gt) = gt {
                    let inner: String = b[i + 1..gt].iter().collect();
                    let name = inner.trim_start_matches('/');
                    let first: String = name
                        .chars()
                        .take_while(|ch| ch.is_ascii_alphanumeric())
                        .collect();
                    if !first.is_empty() {
                        if first.eq_ignore_ascii_case("br") {
                            out.push(' ');
                        }
                        i = gt + 1;
                        handled = true;
                    }
                }
                if !handled {
                    out.push('<');
                    i += 1;
                }
            }
            // Image marker: drop the `!` and let the `[` arm keep the alt text.
            '!' if i + 1 < b.len() && b[i + 1] == '[' => i += 1,
            '[' => {
                let mut depth = 1usize;
                let mut j = i + 1;
                while j < b.len() && depth > 0 {
                    match b[j] {
                        '\\' => j += 1,
                        '[' => depth += 1,
                        ']' => depth -= 1,
                        _ => {}
                    }
                    j += 1;
                }
                let mut consumed = false;
                if depth == 0 {
                    let close = j - 1;
                    let text: String = b[i + 1..close].iter().collect();
                    if let Some(&open_c) = b.get(close + 1) {
                        if open_c == '(' || open_c == '[' {
                            let close_c = if open_c == '(' { ')' } else { ']' };
                            let mut d = 1usize;
                            let mut k = close + 2;
                            while k < b.len() && d > 0 {
                                if b[k] == open_c {
                                    d += 1;
                                } else if b[k] == close_c {
                                    d -= 1;
                                }
                                k += 1;
                            }
                            if d == 0 {
                                out.push_str(&strip_inline(&text));
                                i = k;
                                consumed = true;
                            }
                        }
                    }
                }
                if !consumed {
                    out.push('[');
                    i += 1;
                }
            }
            '*' => {
                while i < b.len() && b[i] == '*' {
                    i += 1;
                }
            }
            '~' => {
                let mut n = 0;
                while i + n < b.len() && b[i + n] == '~' {
                    n += 1;
                }
                if n >= 2 {
                    i += n;
                } else {
                    out.push('~');
                    i += 1;
                }
            }
            '_' => {
                let mut n = 0;
                while i + n < b.len() && b[i + n] == '_' {
                    n += 1;
                }
                let prev = if i == 0 { None } else { Some(b[i - 1]) };
                let next = b.get(i + n).copied();
                // Intra-word underscores (snake_case) are data, not emphasis.
                if is_boundary(prev) || is_boundary(next) {
                    i += n;
                } else {
                    for _ in 0..n {
                        out.push('_');
                    }
                    i += n;
                }
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Table detection
// ---------------------------------------------------------------------------

/// Split one table line into cells, honoring optional outer pipes and the `\|`
/// / `\\` escapes. Cells are returned raw (no trimming) — the caller decides.
fn split_row(line: &str) -> Vec<String> {
    let mut s = line.trim();
    if let Some(rest) = s.strip_prefix('|') {
        s = rest;
    }
    if s.ends_with('|') && !s.ends_with("\\|") {
        s = &s[..s.len() - 1];
    }
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.peek() {
                Some('|') => {
                    cur.push('|');
                    chars.next();
                }
                Some('\\') => {
                    cur.push('\\');
                    chars.next();
                }
                _ => cur.push('\\'),
            },
            '|' => {
                cells.push(std::mem::take(&mut cur));
            }
            _ => cur.push(ch),
        }
    }
    cells.push(cur);
    cells
}

/// Parse a delimiter row (`| --- | :--: |`) into per-column alignments, or
/// `None` if the line isn't one.
fn delimiter_aligns(line: &str) -> Option<Vec<Align>> {
    if !line.contains('-') || !line.contains('|') {
        return None;
    }
    let cells = split_row(line);
    if cells.is_empty() {
        return None;
    }
    let mut aligns = Vec::with_capacity(cells.len());
    for cell in &cells {
        let t = cell.trim();
        let body = t.trim_start_matches(':').trim_end_matches(':');
        if body.is_empty() || !body.chars().all(|c| c == '-') {
            return None;
        }
        aligns.push(match (t.starts_with(':'), t.ends_with(':')) {
            (true, true) => Align::Center,
            (true, false) => Align::Left,
            (false, true) => Align::Right,
            (false, false) => Align::Default,
        });
    }
    Some(aligns)
}

/// True for a fence opener/closer (``` or ~~~). Returns the fence char + run length.
fn fence_marker(line: &str) -> Option<(char, usize)> {
    let t = line.trim_start();
    let c = t.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let n = t.chars().take_while(|&x| x == c).count();
    if n >= 3 {
        Some((c, n))
    } else {
        None
    }
}

fn atx_heading(line: &str) -> Option<String> {
    let t = line.trim_start();
    let hashes = t.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &t[hashes..];
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let text = rest.trim().trim_end_matches('#').trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// Reconcile one body row against the header width, the way a Markdown renderer
/// displays it: short rows are padded with empty cells, cells past the header
/// count are dropped.
fn reconcile(mut cells: Vec<String>, width: usize) -> Vec<String> {
    cells.truncate(width);
    while cells.len() < width {
        cells.push(String::new());
    }
    cells
}

/// Find every GFM table in `md`, in document order.
///
/// - `trim`: strip whitespace padding inside each cell.
/// - `strip_formatting`: render inline Markdown as plain text (whitespace collapsed).
pub fn find_tables(md: &str, trim: bool, strip_formatting: bool) -> Vec<Table> {
    let lines: Vec<&str> = md.lines().collect();
    let clean = |cell: &str| -> String {
        let s = if strip_formatting {
            collapse_ws(&strip_inline(cell))
        } else {
            cell.to_string()
        };
        if trim {
            s.trim().to_string()
        } else {
            s
        }
    };

    let mut tables = Vec::new();
    let mut heading: Option<String> = None;
    let mut fence: Option<(char, usize)> = None;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];

        // Fenced code blocks: everything inside is literal, never a table.
        if let Some((c, n)) = fence_marker(line) {
            match fence {
                Some((fc, fn_)) if fc == c && n >= fn_ => fence = None,
                Some(_) => {}
                None => fence = Some((c, n)),
            }
            i += 1;
            continue;
        }
        if fence.is_some() {
            i += 1;
            continue;
        }

        if let Some(h) = atx_heading(line) {
            heading = Some(h);
            i += 1;
            continue;
        }

        if !line.contains('|') || line.trim().is_empty() {
            i += 1;
            continue;
        }

        // A GFM table = header row + a delimiter row with the same cell count.
        let header_cells = split_row(line);
        let aligns = match lines.get(i + 1).and_then(|l| delimiter_aligns(l)) {
            Some(a) if a.len() == header_cells.len() => a,
            _ => {
                i += 1;
                continue;
            }
        };

        let width = header_cells.len();
        let columns: Vec<String> = header_cells.iter().map(|c| clean(c)).collect();
        let mut rows = Vec::new();
        let mut j = i + 2;
        while j < lines.len() {
            let l = lines[j];
            if l.trim().is_empty() || !l.contains('|') || fence_marker(l).is_some() {
                break;
            }
            let cells: Vec<String> = split_row(l).iter().map(|c| clean(c)).collect();
            rows.push(reconcile(cells, width));
            j += 1;
        }

        tables.push(Table {
            index: tables.len(),
            heading: heading.clone(),
            line: i + 1,
            columns,
            aligns,
            rows,
        });
        i = j;
    }
    tables
}

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

/// Resolve the `table` selection spec against `n` found tables.
/// Accepts `all` / empty, a single 0-based index, or a comma list of indices and
/// `a-b` ranges (e.g. `0,2-3`). Duplicates collapse, order is preserved.
pub fn parse_selection(spec: &str, n: usize) -> Result<Vec<usize>, String> {
    let s = spec.trim().to_ascii_lowercase();
    if s.is_empty() || s == "all" || s == "*" {
        return Ok((0..n).collect());
    }
    let oob = |i: usize| -> String {
        format!(
            "table {i} is out of range: {n} table{} found, valid indices are 0..{}",
            if n == 1 { "" } else { "s" },
            n - 1
        )
    };
    let num = |t: &str| -> Result<usize, String> {
        t.trim().parse::<usize>().map_err(|_| {
            format!("invalid table selection '{t}' (expected 'all', an index like 2, or a list/range like 0,2-3)")
        })
    };
    let mut out: Vec<usize> = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (from, to) = match part.split_once('-') {
            Some((a, b)) => (num(a)?, num(b)?),
            None => {
                let v = num(part)?;
                (v, v)
            }
        };
        if from > to {
            return Err(format!("invalid table range '{part}' (start is after end)"));
        }
        for i in from..=to {
            if i >= n {
                return Err(oob(i));
            }
            if !out.contains(&i) {
                out.push(i);
            }
        }
    }
    if out.is_empty() {
        return Err("no tables selected (use 'all', an index like 2, or a list/range like 0,2-3)".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Make JSON object keys unique and non-empty, preserving column order.
fn json_keys(columns: &[String]) -> Vec<String> {
    let mut keys: Vec<String> = Vec::with_capacity(columns.len());
    for (i, c) in columns.iter().enumerate() {
        let base = if c.trim().is_empty() {
            format!("column_{}", i + 1)
        } else {
            c.trim().to_string()
        };
        let mut name = base.clone();
        let mut n = 2;
        while keys.contains(&name) {
            name = format!("{base}_{n}");
            n += 1;
        }
        keys.push(name);
    }
    keys
}

fn row_value(t: &Table, row: &[String], header: bool) -> Value {
    if header {
        let keys = json_keys(&t.columns);
        let mut map = Map::new();
        for (k, v) in keys.iter().zip(row.iter()) {
            map.insert(k.clone(), Value::String(v.clone()));
        }
        Value::Object(map)
    } else {
        Value::Array(row.iter().map(|c| Value::String(c.clone())).collect())
    }
}

fn table_rows(t: &Table, header: bool) -> Value {
    Value::Array(t.rows.iter().map(|r| row_value(t, r, header)).collect())
}

fn to_json_text(v: &Value, indent: usize) -> Result<String, String> {
    if indent == 0 {
        return serde_json::to_string(v).map_err(|e| format!("JSON encode error: {e}"));
    }
    let pad = " ".repeat(indent.min(8));
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(pad.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    serde::Serialize::serialize(v, &mut ser).map_err(|e| format!("JSON encode error: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("JSON utf8 error: {e}"))
}

fn envelope(t: &Table, header: bool) -> Value {
    json!({
        "index": t.index,
        "heading": t.heading.clone().map(Value::String).unwrap_or(Value::Null),
        "line": t.line,
        "columns": t.columns,
        "rows": table_rows(t, header),
    })
}

fn csv_block(t: &Table, o: &Options, delim: u8) -> Result<String, String> {
    let terminator = if o.crlf {
        csv::Terminator::CRLF
    } else {
        csv::Terminator::Any(b'\n')
    };
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .quote_style(o.quote.style())
        .terminator(terminator)
        .flexible(false)
        .from_writer(vec![]);
    if o.header {
        wtr.write_record(&t.columns)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    for row in &t.rows {
        wtr.write_record(row)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV finalize error: {e}"))?;
    let out = String::from_utf8(bytes).map_err(|e| format!("CSV utf8 error: {e}"))?;
    Ok(out.trim_end_matches(['\r', '\n']).to_string())
}

/// Find the tables in `md` and render the selected ones per `o`.
pub fn extract(md: &str, o: &Options) -> Result<String, String> {
    if md.trim().is_empty() {
        return Err("input is empty: paste a Markdown document containing at least one table".into());
    }
    if md.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is too large: {} bytes (max {MAX_INPUT_BYTES})",
            md.len()
        ));
    }
    let tables = find_tables(md, o.trim, o.strip_formatting);
    if tables.is_empty() {
        return Err("no Markdown tables found: a GitHub-flavored table needs a header row and a |---| separator row directly under it (lines inside ``` code fences are ignored)".into());
    }
    let picked = parse_selection(&o.table, tables.len())?;
    let selected: Vec<&Table> = picked.iter().map(|&i| &tables[i]).collect();
    let nl = if o.crlf { "\r\n" } else { "\n" };
    let indent = o.json_indent.min(8);

    match o.format {
        Format::List => {
            let items: Vec<Value> = selected
                .iter()
                .map(|t| {
                    json!({
                        "index": t.index,
                        "heading": t.heading.clone().map(Value::String).unwrap_or(Value::Null),
                        "line": t.line,
                        "columns": t.columns,
                        "align": t.aligns.iter().map(|a| a.name()).collect::<Vec<_>>(),
                        "rows": t.rows.len(),
                    })
                })
                .collect();
            to_json_text(&Value::Array(items), indent)
        }
        Format::Json => {
            let v = if selected.len() == 1 {
                table_rows(selected[0], o.header)
            } else {
                Value::Array(selected.iter().map(|t| envelope(t, o.header)).collect())
            };
            to_json_text(&v, indent)
        }
        Format::Jsonl => {
            let multi = selected.len() > 1;
            let mut lines = Vec::new();
            for t in &selected {
                for row in &t.rows {
                    let v = row_value(t, row, o.header);
                    let v = if multi {
                        json!({ "table": t.index, "row": v })
                    } else {
                        v
                    };
                    lines.push(serde_json::to_string(&v).map_err(|e| format!("JSON encode error: {e}"))?);
                }
            }
            Ok(lines.join(nl))
        }
        Format::Csv => {
            let delim = delim_byte(&o.delimiter)?;
            let multi = selected.len() > 1;
            let mut blocks = Vec::new();
            for t in &selected {
                let body = csv_block(t, o, delim)?;
                if multi && o.labels {
                    let label = match &t.heading {
                        Some(h) => format!("# Table {}: {h}", t.index),
                        None => format!("# Table {}", t.index),
                    };
                    blocks.push(format!("{label}{nl}{body}"));
                } else {
                    blocks.push(body);
                }
            }
            Ok(blocks.join(&format!("{nl}{nl}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "# Team\n\nSome prose.\n\n| name | role |\n| --- | ---: |\n| Ada | Engineer |\n| Bo | Designer |\n\n## Stock\n\n| item | qty |\n|:---|:--:|\n| bolt | 12 |\n";

    fn opts(format: Format) -> Options {
        Options {
            format,
            ..Options::default()
        }
    }

    #[test]
    fn finds_every_table_with_heading_and_line() {
        let t = find_tables(DOC, true, false);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].heading.as_deref(), Some("Team"));
        assert_eq!(t[0].line, 5);
        assert_eq!(t[0].columns, vec!["name", "role"]);
        assert_eq!(t[0].rows.len(), 2);
        assert_eq!(t[0].aligns, vec![Align::Default, Align::Right]);
        assert_eq!(t[1].heading.as_deref(), Some("Stock"));
        assert_eq!(t[1].aligns, vec![Align::Left, Align::Center]);
        assert_eq!(t[1].rows, vec![vec!["bolt".to_string(), "12".to_string()]]);
    }

    #[test]
    fn csv_of_all_tables_is_labelled_and_blank_line_separated() {
        let out = extract(DOC, &opts(Format::Csv)).unwrap();
        assert_eq!(
            out,
            "# Table 0: Team\nname,role\nAda,Engineer\nBo,Designer\n\n# Table 1: Stock\nitem,qty\nbolt,12"
        );
    }

    #[test]
    fn labels_off_gives_plain_csv_blocks() {
        let o = Options {
            labels: false,
            ..opts(Format::Csv)
        };
        let out = extract(DOC, &o).unwrap();
        assert_eq!(out, "name,role\nAda,Engineer\nBo,Designer\n\nitem,qty\nbolt,12");
    }

    #[test]
    fn single_table_selection_has_no_label() {
        let o = Options {
            table: "1".into(),
            ..opts(Format::Csv)
        };
        assert_eq!(extract(DOC, &o).unwrap(), "item,qty\nbolt,12");
    }

    #[test]
    fn header_off_drops_the_header_row() {
        let o = Options {
            table: "0".into(),
            header: false,
            ..opts(Format::Csv)
        };
        assert_eq!(extract(DOC, &o).unwrap(), "Ada,Engineer\nBo,Designer");
    }

    #[test]
    fn json_of_one_table_is_an_array_of_objects() {
        let o = Options {
            table: "1".into(),
            json_indent: 0,
            ..opts(Format::Json)
        };
        assert_eq!(extract(DOC, &o).unwrap(), r#"[{"item":"bolt","qty":"12"}]"#);
    }

    #[test]
    fn json_header_off_is_an_array_of_arrays() {
        let o = Options {
            table: "1".into(),
            header: false,
            json_indent: 0,
            ..opts(Format::Json)
        };
        assert_eq!(extract(DOC, &o).unwrap(), r#"[["bolt","12"]]"#);
    }

    #[test]
    fn json_of_several_tables_wraps_each_in_an_envelope() {
        let o = Options {
            json_indent: 0,
            ..opts(Format::Json)
        };
        let out = extract(DOC, &o).unwrap();
        assert!(out.starts_with(r#"[{"index":0,"heading":"Team","line":5,"columns":["name","role"],"rows":["#));
        assert!(out.contains(r#"{"index":1,"heading":"Stock""#));
    }

    #[test]
    fn jsonl_emits_one_row_per_line_and_tags_multi_table_rows() {
        let one = Options {
            table: "0".into(),
            ..opts(Format::Jsonl)
        };
        assert_eq!(
            extract(DOC, &one).unwrap(),
            "{\"name\":\"Ada\",\"role\":\"Engineer\"}\n{\"name\":\"Bo\",\"role\":\"Designer\"}"
        );
        let all = extract(DOC, &opts(Format::Jsonl)).unwrap();
        assert!(all.starts_with("{\"table\":0,\"row\":{\"name\":\"Ada\""));
        assert!(all.ends_with("{\"table\":1,\"row\":{\"item\":\"bolt\",\"qty\":\"12\"}}"));
    }

    #[test]
    fn list_is_an_inventory_of_every_table() {
        let o = Options {
            json_indent: 0,
            ..opts(Format::List)
        };
        assert_eq!(
            extract(DOC, &o).unwrap(),
            r#"[{"index":0,"heading":"Team","line":5,"columns":["name","role"],"align":["default","right"],"rows":2},{"index":1,"heading":"Stock","line":12,"columns":["item","qty"],"align":["left","center"],"rows":1}]"#
        );
    }

    #[test]
    fn pipe_tables_inside_code_fences_are_ignored() {
        let md = "```\n| fake | table |\n| --- | --- |\n| a | b |\n```\n\n| real | one |\n| --- | --- |\n| x | y |\n";
        let t = find_tables(md, true, false);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].columns, vec!["real", "one"]);
    }

    #[test]
    fn tilde_fences_are_honored_too() {
        let md = "~~~md\n| fake | table |\n| --- | --- |\n~~~\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n";
        assert_eq!(find_tables(md, true, false).len(), 1);
    }

    #[test]
    fn a_pipe_line_without_a_delimiter_row_is_not_a_table() {
        let md = "a | b | c\nnot a table at all\n";
        assert!(find_tables(md, true, false).is_empty());
        assert!(extract(md, &opts(Format::Csv)).is_err());
    }

    #[test]
    fn delimiter_row_cell_count_must_match_the_header() {
        let md = "| a | b | c |\n| --- | --- |\n| 1 | 2 | 3 |\n";
        assert!(find_tables(md, true, false).is_empty());
    }

    #[test]
    fn ragged_rows_follow_gfm_pad_and_truncate() {
        let md = "| a | b |\n| --- | --- |\n| 1 |\n| 1 | 2 | 3 |\n";
        let t = find_tables(md, true, false);
        assert_eq!(t[0].rows[0], vec!["1".to_string(), String::new()]);
        assert_eq!(t[0].rows[1], vec!["1".to_string(), "2".to_string()]);
    }

    #[test]
    fn escaped_pipes_stay_inside_a_cell() {
        let md = "| expr | note |\n| --- | --- |\n| a \\| b | or |\n";
        let t = find_tables(md, true, false);
        assert_eq!(t[0].rows[0][0], "a | b");
    }

    #[test]
    fn outer_pipes_are_optional() {
        let md = "name | role\n--- | ---\nAda | Engineer\n";
        let o = Options {
            ..opts(Format::Csv)
        };
        assert_eq!(extract(md, &o).unwrap(), "name,role\nAda,Engineer");
    }

    #[test]
    fn strip_formatting_renders_cells_as_plain_text() {
        let md = "| pkg | doc |\n| --- | --- |\n| **bold** `code` | [Guide](https://example.com) line<br>two |\n| snake_case | ~~gone~~ *em* \\* |\n";
        let o = Options {
            table: "0".into(),
            strip_formatting: true,
            json_indent: 0,
            ..opts(Format::Json)
        };
        assert_eq!(
            extract(md, &o).unwrap(),
            r#"[{"pkg":"bold code","doc":"Guide line two"},{"pkg":"snake_case","doc":"gone em *"}]"#
        );
    }

    #[test]
    fn formatting_is_kept_verbatim_by_default() {
        let md = "| pkg |\n| --- |\n| **bold** |\n";
        assert_eq!(extract(md, &opts(Format::Csv)).unwrap(), "pkg\n**bold**");
    }

    #[test]
    fn trim_off_keeps_cell_padding() {
        let md = "|  a  |  b  |\n| --- | --- |\n|  1  |  2  |\n";
        let o = Options {
            trim: false,
            quote: Quote::All,
            ..opts(Format::Csv)
        };
        assert_eq!(extract(md, &o).unwrap(), "\"  a  \",\"  b  \"\n\"  1  \",\"  2  \"");
    }

    #[test]
    fn delimiter_and_quote_and_crlf_options_apply() {
        let md = "| a | b |\n| --- | --- |\n| x,1 | y |\n";
        let o = Options {
            delimiter: "tab".into(),
            quote: Quote::All,
            crlf: true,
            ..opts(Format::Csv)
        };
        assert_eq!(extract(md, &o).unwrap(), "\"a\"\t\"b\"\r\n\"x,1\"\t\"y\"");
    }

    #[test]
    fn minimal_quoting_only_quotes_when_needed() {
        let md = "| a | b |\n| --- | --- |\n| x,1 | y |\n";
        assert_eq!(extract(md, &opts(Format::Csv)).unwrap(), "a,b\n\"x,1\",y");
    }

    #[test]
    fn ranges_and_lists_select_tables() {
        assert_eq!(parse_selection("0,2-3", 4).unwrap(), vec![0, 2, 3]);
        assert_eq!(parse_selection("", 2).unwrap(), vec![0, 1]);
        assert_eq!(parse_selection("ALL", 2).unwrap(), vec![0, 1]);
        assert_eq!(parse_selection("1,1", 2).unwrap(), vec![1]);
    }

    #[test]
    fn duplicate_and_empty_columns_get_unique_json_keys() {
        let md = "| a | a |  |\n| --- | --- | --- |\n| 1 | 2 | 3 |\n";
        let o = Options {
            json_indent: 0,
            ..opts(Format::Json)
        };
        assert_eq!(
            extract(md, &o).unwrap(),
            r#"[{"a":"1","a_2":"2","column_3":"3"}]"#
        );
    }

    #[test]
    fn pretty_json_uses_the_requested_indent() {
        let md = "| a |\n| --- |\n| 1 |\n";
        let o = Options {
            json_indent: 4,
            ..opts(Format::Json)
        };
        assert_eq!(extract(md, &o).unwrap(), "[\n    {\n        \"a\": \"1\"\n    }\n]");
    }

    #[test]
    fn a_header_only_table_still_extracts() {
        let md = "| a | b |\n| --- | --- |\n";
        assert_eq!(extract(md, &opts(Format::Csv)).unwrap(), "a,b");
    }

    // --- error paths ---

    #[test]
    fn empty_input_is_an_error() {
        assert!(extract("   \n ", &opts(Format::Csv))
            .unwrap_err()
            .contains("input is empty"));
    }

    #[test]
    fn a_document_without_tables_is_an_error() {
        let err = extract("# Title\n\njust prose\n", &opts(Format::Csv)).unwrap_err();
        assert!(err.contains("no Markdown tables found"));
    }

    #[test]
    fn an_out_of_range_index_names_the_valid_range() {
        let o = Options {
            table: "7".into(),
            ..opts(Format::Csv)
        };
        assert_eq!(
            extract(DOC, &o).unwrap_err(),
            "table 7 is out of range: 2 tables found, valid indices are 0..1"
        );
    }

    #[test]
    fn a_nonsense_selection_explains_the_accepted_forms() {
        let o = Options {
            table: "first".into(),
            ..opts(Format::Csv)
        };
        assert!(extract(DOC, &o)
            .unwrap_err()
            .contains("expected 'all', an index like 2, or a list/range like 0,2-3"));
    }

    #[test]
    fn a_backwards_range_is_an_error() {
        assert!(parse_selection("3-1", 5).unwrap_err().contains("start is after end"));
    }

    #[test]
    fn a_multi_char_delimiter_is_an_error() {
        let o = Options {
            delimiter: "::".into(),
            ..opts(Format::Csv)
        };
        assert!(extract(DOC, &o)
            .unwrap_err()
            .contains("delimiter must be a single character"));
    }

    #[test]
    fn unknown_format_and_quote_are_errors() {
        assert!(parse_format("xml").unwrap_err().contains("expected csv, json, jsonl or list"));
        assert!(Quote::parse("smart").unwrap_err().contains("expected minimal or all"));
    }

    #[test]
    fn oversized_input_is_rejected_at_the_boundary() {
        let head = "| a |\n| --- |\n";
        let filler = "| x |\n".repeat((MAX_INPUT_BYTES - head.len()) / 6);
        let mut doc = format!("{head}{filler}");
        assert!(doc.len() <= MAX_INPUT_BYTES);
        // Exactly at the cap: still accepted.
        doc.push_str(&"z".repeat(MAX_INPUT_BYTES - doc.len()));
        assert_eq!(doc.len(), MAX_INPUT_BYTES);
        assert!(extract(&doc, &opts(Format::List)).is_ok());
        // One byte over: rejected with both numbers.
        doc.push('z');
        let err = extract(&doc, &opts(Format::List)).unwrap_err();
        assert_eq!(err, format!("input is too large: 1000001 bytes (max 1000000)"));
    }
}
