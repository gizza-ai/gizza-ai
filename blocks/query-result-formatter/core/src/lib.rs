//! query-result-formatter core — render raw query result rows (JSON array,
//! CSV, or TSV) into a clean, aligned Markdown or ASCII table.
//! Pure-Rust (`csv`, `serde_json`). No wafer/wasm-bindgen deps.

use serde_json::Value;

/// Which shape the raw `data` is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    /// Sniff: leading `[`/`{` → JSON; a `+---+`/`---+---` rule → SQL CLI table;
    /// else a tab in the first line → TSV; else CSV.
    Auto,
    /// A JSON array of row objects/arrays (or a single row object).
    Json,
    /// Comma-separated rows.
    Csv,
    /// Tab-separated rows (the default `psql`/BigQuery dump).
    Tsv,
    /// A `psql`/MySQL/SQLite CLI result table (pipe columns, `+---+`/`---+---`
    /// rules, `(N rows)` footer) — the raw paste from a database shell.
    Sql,
}

impl InputFormat {
    pub fn parse(s: &str) -> Result<InputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "" => Ok(InputFormat::Auto),
            "json" => Ok(InputFormat::Json),
            "csv" => Ok(InputFormat::Csv),
            "tsv" | "tab" => Ok(InputFormat::Tsv),
            "sql" | "psql" | "mysql" => Ok(InputFormat::Sql),
            other => Err(format!("unknown input_format '{other}' (use auto, json, csv, tsv or sql)")),
        }
    }

    /// Resolve `Auto` against the raw text.
    fn resolve(self, data: &str) -> InputFormat {
        match self {
            InputFormat::Auto => {
                let t = data.trim_start();
                if t.starts_with('[') || t.starts_with('{') {
                    InputFormat::Json
                } else if data.lines().any(is_rule_line) {
                    // A `+---+` (MySQL/SQLite) or `---+---` (psql) rule row is the
                    // tell-tale of a CLI result table.
                    InputFormat::Sql
                } else if data.lines().next().is_some_and(|l| l.contains('\t')) {
                    InputFormat::Tsv
                } else {
                    InputFormat::Csv
                }
            }
            other => other,
        }
    }
}

/// Is `line` a horizontal rule from a SQL CLI table — non-empty, containing a
/// `-`, and made up of only `-`, `+`, `|`, `=` and whitespace? (`=` covers
/// SQLite's `.mode box`/`column` double rules.)
fn is_rule_line(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty()
        && t.contains('-')
        && t.chars().all(|c| matches!(c, '-' | '+' | '|' | '=' | ' '))
}

/// Output table format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// GitHub-style Markdown pipe table, padded so columns line up.
    Markdown,
    /// Box-drawing aligned ASCII table (`+---+` borders, padded cells).
    Ascii,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" | "" => Ok(Format::Markdown),
            "ascii" | "text" => Ok(Format::Ascii),
            other => Err(format!("unknown format '{other}' (use markdown or ascii)")),
        }
    }
}

/// Per-column text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
    Center,
}

impl Align {
    pub fn parse(s: &str) -> Result<Align, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "left" | "l" | "" => Ok(Align::Left),
            "right" | "r" => Ok(Align::Right),
            "center" | "centre" | "c" => Ok(Align::Center),
            other => Err(format!("unknown align '{other}' (use left, right or center)")),
        }
    }
}

/// Display width of a cell, in characters (so multi-byte text still aligns).
fn width(s: &str) -> usize {
    s.chars().count()
}

/// Pad `s` to `w` chars according to `align`.
fn pad(s: &str, w: usize, align: Align) -> String {
    let len = width(s);
    if len >= w {
        return s.to_string();
    }
    let total = w - len;
    match align {
        Align::Left => format!("{s}{}", " ".repeat(total)),
        Align::Right => format!("{}{s}", " ".repeat(total)),
        Align::Center => {
            let left = total / 2;
            let right = total - left;
            format!("{}{s}{}", " ".repeat(left), " ".repeat(right))
        }
    }
}

fn md_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('|', "\\|").replace('\n', " ")
}

/// Stringify one JSON scalar/compound value for a table cell.
fn cell_str(v: &Value, null_text: &str) -> String {
    match v {
        Value::Null => null_text.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        // Nested arrays/objects are kept as compact JSON so a cell never breaks.
        other => other.to_string(),
    }
}

/// Parse JSON `data` into (header, rows).
fn parse_json(data: &str, has_header: bool, null_text: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let value: Value = serde_json::from_str(data).map_err(|e| format!("invalid JSON: {e}"))?;
    let elems: Vec<Value> = match value {
        // A single row object → a one-row table (keys as header).
        Value::Object(_) => vec![value],
        Value::Array(a) => a,
        _ => return Err("JSON must be an array of rows or a single row object".into()),
    };
    if elems.is_empty() {
        return Err("no rows found".into());
    }

    // Array of objects: header = union of keys in first-seen order.
    if elems.iter().any(|e| e.is_object()) {
        let mut header: Vec<String> = Vec::new();
        for e in &elems {
            if let Value::Object(map) = e {
                for k in map.keys() {
                    if !header.iter().any(|h| h == k) {
                        header.push(k.clone());
                    }
                }
            }
        }
        if header.is_empty() {
            return Err("no columns found in JSON objects".into());
        }
        let rows: Vec<Vec<String>> = elems
            .iter()
            .map(|e| {
                header
                    .iter()
                    .map(|k| match e {
                        Value::Object(map) => map.get(k).map(|v| cell_str(v, null_text)).unwrap_or_else(|| null_text.to_string()),
                        // A non-object element alongside objects: put it in the first column.
                        _ if header.first() == Some(k) => cell_str(e, null_text),
                        _ => null_text.to_string(),
                    })
                    .collect()
            })
            .collect();
        return Ok((header, rows));
    }

    // Array of arrays (row vectors) — optional header row, like CSV.
    if elems.iter().all(|e| e.is_array()) {
        let mut rows: Vec<Vec<String>> = elems
            .iter()
            .map(|e| match e {
                Value::Array(a) => a.iter().map(|v| cell_str(v, null_text)).collect(),
                _ => unreachable!(),
            })
            .collect();
        let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        if has_header && !rows.is_empty() {
            let mut header = rows.remove(0);
            while header.len() < ncols {
                header.push(format!("Column {}", header.len() + 1));
            }
            return Ok((header, rows));
        }
        let header = (1..=ncols).map(|i| format!("Column {i}")).collect();
        return Ok((header, rows));
    }

    // Array of scalars → a single-column table.
    let rows: Vec<Vec<String>> = elems.iter().map(|e| vec![cell_str(e, null_text)]).collect();
    Ok((vec!["value".to_string()], rows))
}

/// Parse delimited (CSV/TSV) `data` into (header, rows).
fn parse_delimited(data: &str, delim: u8, has_header: bool) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> =
        rdr.records().collect::<Result<_, _>>().map_err(|e| format!("parse error: {e}"))?;
    if records.is_empty() {
        return Err("no rows found".into());
    }
    let ncols = records.iter().map(|r| r.len()).max().unwrap_or(0);
    let to_row = |rec: &csv::StringRecord| -> Vec<String> {
        (0..ncols).map(|i| rec.get(i).unwrap_or("").to_string()).collect()
    };
    if has_header {
        let header = to_row(&records[0]);
        let rows = records[1..].iter().map(to_row).collect();
        Ok((header, rows))
    } else {
        let header = (1..=ncols).map(|i| format!("Column {i}")).collect();
        let rows = records.iter().map(to_row).collect();
        Ok((header, rows))
    }
}

/// Parse a `psql`/MySQL/SQLite CLI result table into (header, rows). The first
/// content (non-rule) line is always the header; `+---+`/`---+---` rule rows and
/// a trailing `(N rows)` / `N rows in set` footer are dropped. Pipe columns are
/// split on `|`, with the outer border pipes (MySQL `| … |`) stripped.
fn parse_sql_table(data: &str, null_text: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let split_cells = |line: &str| -> Vec<String> {
        let t = line.trim();
        // MySQL/SQLite wrap rows in border pipes (`| a | b |`); psql has none
        // (` a | b `). Only strip the outer pair when the row STARTS with `|`, so
        // a psql trailing empty (null) column isn't mistaken for a border pipe.
        let core = match t.strip_prefix('|') {
            Some(rest) => rest.strip_suffix('|').unwrap_or(rest),
            None => t,
        };
        core.split('|').map(|c| c.trim().to_string()).collect()
    };
    // psql prints NULLs as an empty cell already; map an empty cell to null_text.
    let apply_null = |cells: Vec<String>| -> Vec<String> {
        cells
            .into_iter()
            .map(|c| if c.is_empty() { null_text.to_string() } else { c })
            .collect()
    };
    let is_footer = |t: &str| -> bool {
        // psql `(2 rows)` / MySQL `2 rows in set (0.00 sec)` / `Empty set`.
        let low = t.to_ascii_lowercase();
        (t.starts_with('(') && low.contains("row"))
            || low.contains("rows in set")
            || low.contains("row in set")
            || low.starts_with("empty set")
    };

    let mut content = data
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !is_rule_line(l) && l.contains('|') && !is_footer(l));

    let header = match content.next() {
        Some(h) => apply_null(split_cells(h)),
        None => return Err("no SQL table rows found (expected a psql/MySQL-style `| col | … |` table)".into()),
    };
    let rows: Vec<Vec<String>> = content.map(|l| apply_null(split_cells(l))).collect();
    Ok((header, rows))
}

/// Normalize a header + rows to a rectangular grid (pad short rows, extend header).
fn normalize(mut header: Vec<String>, mut rows: Vec<Vec<String>>) -> (Vec<String>, Vec<Vec<String>>) {
    let ncols = header.len().max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
    while header.len() < ncols {
        header.push(format!("Column {}", header.len() + 1));
    }
    for r in &mut rows {
        while r.len() < ncols {
            r.push(String::new());
        }
    }
    (header, rows)
}

/// Render raw query-result `data` (JSON/CSV/TSV) as a Markdown or ASCII table.
pub fn format_result(
    data: &str,
    input_format: InputFormat,
    output: Format,
    has_header: bool,
    align: Align,
    null_text: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let (header, rows) = match input_format.resolve(data) {
        InputFormat::Json => parse_json(data, has_header, null_text)?,
        InputFormat::Csv => parse_delimited(data, b',', has_header)?,
        InputFormat::Tsv => parse_delimited(data, b'\t', has_header)?,
        InputFormat::Sql => parse_sql_table(data, null_text)?,
        InputFormat::Auto => unreachable!("resolve() never returns Auto"),
    };
    let (header, rows) = normalize(header, rows);
    if header.is_empty() {
        return Err("no columns found".into());
    }
    let ncols = header.len();
    match output {
        Format::Markdown => Ok(render_markdown(&header, &rows, ncols, align)),
        Format::Ascii => Ok(render_ascii(&header, &rows, ncols, align)),
    }
}

fn render_markdown(header: &[String], rows: &[Vec<String>], ncols: usize, align: Align) -> String {
    let esc_header: Vec<String> = header.iter().map(|h| md_escape(h)).collect();
    let esc_rows: Vec<Vec<String>> =
        rows.iter().map(|r| r.iter().map(|c| md_escape(c)).collect()).collect();
    // Markdown needs at least 3 dashes for the separator row to render.
    let widths: Vec<usize> = (0..ncols)
        .map(|i| {
            let mut w = width(&esc_header[i]).max(3);
            for row in &esc_rows {
                w = w.max(width(&row[i]));
            }
            w
        })
        .collect();

    let row_line = |cells: &[String]| {
        let padded: Vec<String> =
            cells.iter().enumerate().map(|(i, c)| pad(c, widths[i], align)).collect();
        format!("| {} |\n", padded.join(" | "))
    };

    let mut out = String::new();
    out.push_str(&row_line(&esc_header));
    let sep: Vec<String> = widths
        .iter()
        .map(|&w| match align {
            Align::Left => "-".repeat(w),
            Align::Right => format!("{}:", "-".repeat(w - 1)),
            Align::Center => format!(":{}:", "-".repeat(w - 2)),
        })
        .collect();
    out.push_str(&format!("| {} |\n", sep.join(" | ")));
    for row in &esc_rows {
        out.push_str(&row_line(row));
    }
    out.trim_end().to_string()
}

fn render_ascii(header: &[String], rows: &[Vec<String>], ncols: usize, align: Align) -> String {
    // ASCII cells: collapse embedded newlines so the grid stays rectangular.
    let clean = |s: &str| s.replace('\n', " ").replace('\r', "");
    let esc_header: Vec<String> = header.iter().map(|h| clean(h)).collect();
    let esc_rows: Vec<Vec<String>> =
        rows.iter().map(|r| r.iter().map(|c| clean(c)).collect()).collect();
    let widths: Vec<usize> = (0..ncols)
        .map(|i| {
            let mut w = width(&esc_header[i]).max(1);
            for row in &esc_rows {
                w = w.max(width(&row[i]));
            }
            w
        })
        .collect();

    let border = || {
        let segs: Vec<String> = widths.iter().map(|&w| "-".repeat(w + 2)).collect();
        format!("+{}+", segs.join("+"))
    };
    let row_line = |cells: &[String]| {
        let padded: Vec<String> =
            cells.iter().enumerate().map(|(i, c)| format!(" {} ", pad(c, widths[i], align))).collect();
        format!("|{}|", padded.join("|"))
    };

    let mut out = String::new();
    out.push_str(&border());
    out.push('\n');
    out.push_str(&row_line(&esc_header));
    out.push('\n');
    out.push_str(&border());
    out.push('\n');
    for row in &esc_rows {
        out.push_str(&row_line(row));
        out.push('\n');
    }
    out.push_str(&border());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(data: &str, f: InputFormat) -> String {
        format_result(data, f, Format::Markdown, true, Align::Left, "").unwrap()
    }

    #[test]
    fn json_objects_to_markdown() {
        let out = md(r#"[{"id":1,"name":"Ada"},{"id":2,"name":"Linus"}]"#, InputFormat::Json);
        // id col width = max(len("id"), 3) = 3; name col width = len("Linus") = 5.
        let expected = "\
| id  | name  |
| --- | ----- |
| 1   | Ada   |
| 2   | Linus |";
        assert_eq!(out, expected);
    }

    #[test]
    fn json_objects_union_keys_and_nulls() {
        // second row is missing "name", first is missing "email" → union columns, nulls filled.
        let out = format_result(
            r#"[{"id":1,"name":"Ada"},{"id":2,"email":"l@x.io"}]"#,
            InputFormat::Json,
            Format::Ascii,
            true,
            Align::Left,
            "NULL",
        )
        .unwrap();
        assert!(out.contains("| id | name | email  |"), "{out}");
        assert!(out.contains("| 1  | Ada  | NULL   |"), "{out}");
        assert!(out.contains("| 2  | NULL | l@x.io |"), "{out}");
    }

    #[test]
    fn json_array_of_arrays_with_header() {
        let out = md(r#"[["a","b"],[1,2],[3,4]]"#, InputFormat::Json);
        // Markdown pads header cells to a min width of 3 (for the `---` separator).
        assert!(out.contains("| a   | b   |"), "{out}");
        assert!(out.contains("| 1   | 2   |"), "{out}");
    }

    #[test]
    fn json_single_object_is_one_row() {
        let out = md(r#"{"host":"db1","up":true}"#, InputFormat::Json);
        assert!(out.contains("| host | up   |"), "{out}");
        assert!(out.contains("| db1  | true |"), "{out}");
    }

    #[test]
    fn json_scalars_single_column() {
        let out = md(r#"["us-east","us-west"]"#, InputFormat::Json);
        assert!(out.contains("| value   |"), "{out}");
        assert!(out.contains("| us-east |"), "{out}");
    }

    #[test]
    fn json_nested_value_kept_as_json() {
        let out = md(r#"[{"id":1,"tags":["x","y"]}]"#, InputFormat::Json);
        assert!(out.contains(r#"["x","y"]"#), "{out}");
    }

    #[test]
    fn csv_to_ascii_aligned() {
        let out = format_result(
            "name,role\nAda,Engineer\nBo,Designer",
            InputFormat::Csv,
            Format::Ascii,
            true,
            Align::Left,
            "",
        )
        .unwrap();
        let expected = "\
+------+----------+
| name | role     |
+------+----------+
| Ada  | Engineer |
| Bo   | Designer |
+------+----------+";
        assert_eq!(out, expected);
    }

    #[test]
    fn tsv_auto_detected() {
        let out = format_result("a\tbb\n1\t2", InputFormat::Auto, Format::Ascii, true, Align::Left, "")
            .unwrap();
        assert!(out.contains("| a | bb |"), "{out}");
        assert!(out.contains("| 1 | 2  |"), "{out}");
    }

    #[test]
    fn auto_detects_json() {
        let out = format_result(r#"  [{"x":1}]"#, InputFormat::Auto, Format::Markdown, true, Align::Left, "");
        assert!(out.unwrap().contains("| x "), "auto should sniff leading [ as JSON");
    }

    #[test]
    fn sql_mysql_box_table() {
        // Raw MySQL/SQLite CLI output with +---+ rules and border pipes.
        let data = "\
+----+-------+
| id | name  |
+----+-------+
|  1 | Ada   |
|  2 | Linus |
+----+-------+
2 rows in set (0.00 sec)";
        let out = md(data, InputFormat::Sql);
        let expected = "\
| id  | name  |
| --- | ----- |
| 1   | Ada   |
| 2   | Linus |";
        assert_eq!(out, expected);
    }

    #[test]
    fn sql_psql_table_with_footer_and_null() {
        // Postgres psql output: no border pipes, a ---+--- rule, a (N rows) footer,
        // and an empty NULL cell that null_text should fill.
        let data = "\
 id |  name
----+-------
  1 | Ada
  2 |
(2 rows)";
        let out = format_result(data, InputFormat::Sql, Format::Ascii, true, Align::Left, "NULL").unwrap();
        assert!(out.contains("| id | name |"), "{out}");
        assert!(out.contains("| 1  | Ada  |"), "{out}");
        assert!(out.contains("| 2  | NULL |"), "{out}");
    }

    #[test]
    fn sql_auto_detected() {
        let data = "+---+\n| a |\n+---+\n| 1 |\n+---+";
        let out = format_result(data, InputFormat::Auto, Format::Markdown, true, Align::Left, "").unwrap();
        assert!(out.contains("| a "), "auto should sniff a +---+ rule as SQL: {out}");
        assert!(out.contains("| 1 "), "{out}");
    }

    #[test]
    fn sql_empty_errors() {
        assert!(format_result("no pipes here", InputFormat::Sql, Format::Markdown, true, Align::Left, "").is_err());
    }

    #[test]
    fn markdown_right_align_markers() {
        let out = format_result("a,b\n1,2", InputFormat::Csv, Format::Markdown, true, Align::Right, "").unwrap();
        assert!(out.contains("--:"), "right marker: {out}");
    }

    #[test]
    fn markdown_escapes_pipe() {
        let out = md("a\nx|y", InputFormat::Csv);
        assert!(out.contains("x\\|y"), "{out}");
    }

    #[test]
    fn no_header_synthesizes_columns() {
        let out = format_result("1,2\n3,4", InputFormat::Csv, Format::Ascii, false, Align::Left, "").unwrap();
        assert!(out.contains("| Column 1 | Column 2 |"), "{out}");
    }

    #[test]
    fn errors() {
        assert!(format_result("", InputFormat::Auto, Format::Markdown, true, Align::Left, "").is_err());
        assert!(format_result("[]", InputFormat::Json, Format::Markdown, true, Align::Left, "").is_err());
        assert!(format_result("{not json", InputFormat::Json, Format::Markdown, true, Align::Left, "").is_err());
        assert!(format_result("42", InputFormat::Json, Format::Markdown, true, Align::Left, "").is_err());
        assert!(Format::parse("latex").is_err());
        assert!(Align::parse("middle").is_err());
        assert!(InputFormat::parse("yaml").is_err());
    }
}
