//! gizza-ai/markdown-table-to-csv core — parse a Markdown (or pipe-delimited)
//! table back into clean CSV. It strips the alignment/separator row and cell
//! padding, honors optional outer pipes, and unescapes `\|` / `\\` inside cells.
//! Pure-Rust (`csv` for RFC-4180 quoting). No wafer/wasm-bindgen deps.

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        " " | "space" => b' ',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be a single char or tab/comma/semicolon/pipe/space, got '{other}'"
                ));
            }
        }
    })
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
            other => Err(format!("unknown quote style '{other}' (use minimal or all)")),
        }
    }
    fn style(self) -> csv::QuoteStyle {
        match self {
            Quote::Minimal => csv::QuoteStyle::Necessary,
            Quote::All => csv::QuoteStyle::Always,
        }
    }
}

/// True if a parsed row is a Markdown alignment/separator row — every cell is
/// made only of `-`, `:` and spaces and carries at least one `-`
/// (e.g. `---`, `:--`, `--:`, `:-:`), so it holds no data.
fn is_separator_row(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells.iter().all(|c| {
            let t = c.trim();
            !t.is_empty() && t.contains('-') && t.chars().all(|ch| ch == '-' || ch == ':')
        })
}

/// Split one table line into trimmed, unescaped cells, honoring optional outer
/// pipes and `\|` / `\\` escapes.
fn split_row(line: &str) -> Vec<String> {
    let mut s = line.trim();
    // Drop a single optional leading outer pipe.
    if let Some(rest) = s.strip_prefix('|') {
        s = rest;
    }
    // Drop a single optional trailing outer pipe (but not an escaped `\|`).
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
                cells.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    cells.push(cur.trim().to_string());
    cells
}

/// Parse a Markdown / pipe-delimited table into CSV (LF endings, no BOM).
///
/// - `delimiter`: output field separator (single char or comma/tab/semicolon/pipe/space).
/// - `has_header`: when false, the first (header) row is dropped and only data rows are emitted.
/// - `quote`: CSV quoting policy for the output.
pub fn to_csv(
    input: &str,
    delimiter: &str,
    has_header: bool,
    quote: Quote,
) -> Result<String, String> {
    to_csv_ext(input, delimiter, has_header, quote, false, false)
}

/// Parse a Markdown / pipe-delimited table into CSV, with output line-ending and
/// BOM control.
///
/// - `crlf`: when true, rows are separated by `\r\n` (Windows/Excel) instead of `\n`.
/// - `bom`: when true, prepend a UTF-8 byte-order mark so Excel opens the CSV as UTF-8.
pub fn to_csv_ext(
    input: &str,
    delimiter: &str,
    has_header: bool,
    quote: Quote,
    crlf: bool,
    bom: bool,
) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty".into());
    }
    let delim = delim_byte(delimiter)?;

    // Keep only pipe-bearing lines: a line with no `|` isn't part of a pipe
    // table, so surrounding prose can be pasted along and is ignored.
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in input.lines() {
        if line.trim().is_empty() || !line.contains('|') {
            continue;
        }
        let cells = split_row(line);
        if is_separator_row(&cells) {
            continue;
        }
        rows.push(cells);
    }
    if rows.is_empty() {
        return Err("no table rows found (expected a pipe-delimited or Markdown table)".into());
    }
    if !has_header {
        rows.remove(0);
        if rows.is_empty() {
            return Err("only a header row was found; nothing left after dropping it".into());
        }
    }

    // Rectangularize: pad short rows to the widest row so the CSV imports cleanly.
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);

    let terminator = if crlf {
        csv::Terminator::CRLF
    } else {
        csv::Terminator::Any(b'\n')
    };
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .quote_style(quote.style())
        .terminator(terminator)
        .flexible(false)
        .from_writer(vec![]);
    for row in &rows {
        let mut rec: Vec<&str> = row.iter().map(String::as_str).collect();
        while rec.len() < width {
            rec.push("");
        }
        wtr.write_record(&rec)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV finalize error: {e}"))?;
    let out = String::from_utf8(bytes).map_err(|e| format!("CSV utf8 error: {e}"))?;
    let out = out.trim_end_matches(['\r', '\n']).to_string();
    Ok(if bom {
        format!("\u{feff}{out}")
    } else {
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_markdown_table() {
        let md = "| name | role |\n| --- | --- |\n| Ada | Engineer |\n| Bo | Designer |";
        let out = to_csv(md, ",", true, Quote::Minimal).unwrap();
        assert_eq!(out, "name,role\nAda,Engineer\nBo,Designer");
    }

    #[test]
    fn strips_alignment_row_with_colons() {
        let md = "| a | b | c |\n|:---|:--:|---:|\n| 1 | 2 | 3 |";
        let out = to_csv(md, ",", true, Quote::Minimal).unwrap();
        assert_eq!(out, "a,b,c\n1,2,3");
    }

    #[test]
    fn no_outer_pipes() {
        let md = "name | role\n--- | ---\nAda | Engineer";
        let out = to_csv(md, ",", true, Quote::Minimal).unwrap();
        assert_eq!(out, "name,role\nAda,Engineer");
    }

    #[test]
    fn trims_padding() {
        let md = "|  a   |    b |\n| --- | --- |\n|   1  |  2   |";
        let out = to_csv(md, ",", true, Quote::Minimal).unwrap();
        assert_eq!(out, "a,b\n1,2");
    }

    #[test]
    fn escaped_pipe_becomes_literal_pipe_and_is_quoted() {
        let md = "| expr | note |\n| --- | --- |\n| a \\| b | ok |";
        // The `\|` is a literal pipe inside the cell, not a column boundary. With a
        // comma delimiter the pipe needs no quoting (RFC 4180 quotes only for the
        // delimiter/quote/newline), so it stays a bare `a | b`.
        let out = to_csv(md, ",", true, Quote::Minimal).unwrap();
        assert_eq!(out, "expr,note\na | b,ok");
        // With a PIPE delimiter the same cell must be quoted so it re-imports as one field.
        let out_pipe = to_csv(md, "pipe", true, Quote::Minimal).unwrap();
        assert_eq!(out_pipe, "expr|note\n\"a | b\"|ok");
    }

    #[test]
    fn escaped_backslash() {
        let md = "| p |\n| --- |\n| C:\\\\x |";
        let out = to_csv(md, ",", true, Quote::Minimal).unwrap();
        assert_eq!(out, "p\nC:\\x");
    }

    #[test]
    fn quotes_cell_with_comma() {
        let md = "| city | note |\n| --- | --- |\n| Paris, FR | capital |";
        let out = to_csv(md, ",", true, Quote::Minimal).unwrap();
        assert_eq!(out, "city,note\n\"Paris, FR\",capital");
    }

    #[test]
    fn tab_delimiter_output() {
        let md = "| a | b |\n| --- | --- |\n| 1 | 2 |";
        let out = to_csv(md, "tab", true, Quote::Minimal).unwrap();
        assert_eq!(out, "a\tb\n1\t2");
    }

    #[test]
    fn quote_all_wraps_every_field() {
        let md = "| a | b |\n| --- | --- |\n| 1 | 2 |";
        let out = to_csv(md, ",", true, Quote::All).unwrap();
        assert_eq!(out, "\"a\",\"b\"\n\"1\",\"2\"");
    }

    #[test]
    fn drop_header_emits_data_only() {
        let md = "| name | role |\n| --- | --- |\n| Ada | Engineer |";
        let out = to_csv(md, ",", false, Quote::Minimal).unwrap();
        assert_eq!(out, "Ada,Engineer");
    }

    #[test]
    fn ragged_rows_are_padded() {
        let md = "| a | b | c |\n| --- | --- | --- |\n| 1 |\n| 4 | 5 | 6 |";
        let out = to_csv(md, ",", true, Quote::Minimal).unwrap();
        assert_eq!(out, "a,b,c\n1,,\n4,5,6");
    }

    #[test]
    fn pipe_delimited_without_separator_row() {
        // No alignment row: every non-blank pipe line is data.
        let md = "1 | 2 | 3\n4 | 5 | 6";
        let out = to_csv(md, ",", true, Quote::Minimal).unwrap();
        assert_eq!(out, "1,2,3\n4,5,6");
    }

    #[test]
    fn ignores_surrounding_prose() {
        let md = "Here is a table:\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n\nThanks!";
        let out = to_csv(md, ",", true, Quote::Minimal).unwrap();
        assert_eq!(out, "a,b\n1,2");
    }

    #[test]
    fn crlf_line_endings() {
        let md = "| a | b |\n| --- | --- |\n| 1 | 2 |";
        let out = to_csv_ext(md, ",", true, Quote::Minimal, true, false).unwrap();
        assert_eq!(out, "a,b\r\n1,2");
    }

    #[test]
    fn bom_is_prepended() {
        let md = "| a | b |\n| --- | --- |\n| 1 | 2 |";
        let out = to_csv_ext(md, ",", true, Quote::Minimal, false, true).unwrap();
        assert_eq!(out, "\u{feff}a,b\n1,2");
        assert!(out.starts_with('\u{feff}'));
    }

    #[test]
    fn errors() {
        assert!(to_csv("", ",", true, Quote::Minimal).is_err());
        assert!(to_csv("just some prose, no pipes", ",", true, Quote::Minimal).is_err());
        assert!(Quote::parse("fancy").is_err());
        assert!(delim_byte("nope").is_err());
    }
}
