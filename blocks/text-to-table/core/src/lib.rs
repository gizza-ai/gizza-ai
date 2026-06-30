//! gizza-ai/text-to-table core — render delimited text (CSV/TSV/custom
//! delimiter) as an aligned ASCII or Markdown table.
//! Pure-Rust (`csv`). No wafer/wasm-bindgen deps.

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

/// Output table format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Box-drawing aligned ASCII table (`+---+---+` borders, padded cells).
    Ascii,
    /// GitHub-style Markdown pipe table, padded so columns line up.
    Markdown,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "ascii" | "text" | "" => Ok(Format::Ascii),
            "markdown" | "md" => Ok(Format::Markdown),
            other => Err(format!("unknown format '{other}' (use ascii or markdown)")),
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

/// Render `data` (delimited text) as an aligned ASCII or Markdown table.
pub fn to_table(
    data: &str,
    format: Format,
    has_header: bool,
    delimiter: &str,
    align: Align,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr
        .records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("parse error: {e}"))?;
    if records.is_empty() {
        return Err("no rows found".into());
    }
    let ncols = records.iter().map(|r| r.len()).max().unwrap_or(0);
    let cell = |rec: &csv::StringRecord, i: usize| rec.get(i).unwrap_or("").to_string();

    let (header, body): (Vec<String>, &[csv::StringRecord]) = if has_header {
        ((0..ncols).map(|i| cell(&records[0], i)).collect(), &records[1..])
    } else {
        ((1..=ncols).map(|i| format!("Column {i}")).collect(), &records[..])
    };

    match format {
        Format::Markdown => render_markdown(&header, body, ncols, &cell, align),
        Format::Ascii => render_ascii(&header, body, ncols, &cell, align),
    }
}

fn col_widths<F>(header: &[String], body: &[csv::StringRecord], ncols: usize, cell: &F, min: usize) -> Vec<usize>
where
    F: Fn(&csv::StringRecord, usize) -> String,
{
    (0..ncols)
        .map(|i| {
            let mut w = width(&header[i]).max(min);
            for rec in body {
                w = w.max(width(&cell(rec, i)));
            }
            w
        })
        .collect()
}

fn render_markdown<F>(
    header: &[String],
    body: &[csv::StringRecord],
    ncols: usize,
    cell: &F,
    align: Align,
) -> Result<String, String>
where
    F: Fn(&csv::StringRecord, usize) -> String,
{
    let esc_header: Vec<String> = header.iter().map(|h| md_escape(h)).collect();
    let esc_body: Vec<Vec<String>> = body
        .iter()
        .map(|rec| (0..ncols).map(|i| md_escape(&cell(rec, i))).collect())
        .collect();
    // Markdown needs at least 3 dashes for the separator row to render.
    let widths: Vec<usize> = (0..ncols)
        .map(|i| {
            let mut w = width(&esc_header[i]).max(3);
            for row in &esc_body {
                w = w.max(width(&row[i]));
            }
            w
        })
        .collect();

    let row_line = |cells: &[String]| {
        let padded: Vec<String> = cells
            .iter()
            .enumerate()
            .map(|(i, c)| pad(c, widths[i], align))
            .collect();
        format!("| {} |\n", padded.join(" | "))
    };

    let mut out = String::new();
    out.push_str(&row_line(&esc_header));
    // separator row carries the alignment markers
    let sep: Vec<String> = widths
        .iter()
        .map(|&w| match align {
            Align::Left => "-".repeat(w),
            Align::Right => format!("{}:", "-".repeat(w - 1)),
            Align::Center => format!(":{}:", "-".repeat(w - 2)),
        })
        .collect();
    out.push_str(&format!("| {} |\n", sep.join(" | ")));
    for row in &esc_body {
        out.push_str(&row_line(row));
    }
    Ok(out.trim_end().to_string())
}

fn render_ascii<F>(
    header: &[String],
    body: &[csv::StringRecord],
    ncols: usize,
    cell: &F,
    align: Align,
) -> Result<String, String>
where
    F: Fn(&csv::StringRecord, usize) -> String,
{
    // ASCII cells: collapse embedded newlines so the grid stays rectangular.
    let clean = |s: &str| s.replace('\n', " ").replace('\r', "");
    let esc_header: Vec<String> = header.iter().map(|h| clean(h)).collect();
    let widths = col_widths(&esc_header, body, ncols, &|rec, i| clean(&cell(rec, i)), 1);

    let border = || {
        let segs: Vec<String> = widths.iter().map(|&w| "-".repeat(w + 2)).collect();
        format!("+{}+", segs.join("+"))
    };
    let row_line = |cells: &[String]| {
        let padded: Vec<String> = cells
            .iter()
            .enumerate()
            .map(|(i, c)| format!(" {} ", pad(c, widths[i], align)))
            .collect();
        format!("|{}|", padded.join("|"))
    };

    let mut out = String::new();
    out.push_str(&border());
    out.push('\n');
    out.push_str(&row_line(&esc_header));
    out.push('\n');
    out.push_str(&border());
    out.push('\n');
    for rec in body {
        let cells: Vec<String> = (0..ncols).map(|i| clean(&cell(rec, i))).collect();
        out.push_str(&row_line(&cells));
        out.push('\n');
    }
    out.push_str(&border());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_table_aligned() {
        let out = to_table("name,role\nAda,Engineer\nBo,Designer", Format::Ascii, true, ",", Align::Left)
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
    fn ascii_right_align() {
        let out = to_table("a,bb\n1,2", Format::Ascii, true, ",", Align::Right).unwrap();
        assert!(out.contains("| a | bb |"));
        assert!(out.contains("| 1 |  2 |"));
    }

    #[test]
    fn ascii_center_align() {
        let out = to_table("xxxx\nab", Format::Ascii, true, ",", Align::Center).unwrap();
        assert!(out.contains("|  ab  |"), "{out}");
    }

    #[test]
    fn markdown_table_aligned() {
        let out = to_table("name,role\nAda,Engineer", Format::Markdown, true, ",", Align::Left).unwrap();
        let expected = "\
| name | role     |
| ---- | -------- |
| Ada  | Engineer |";
        assert_eq!(out, expected);
    }

    #[test]
    fn markdown_align_markers() {
        let r = to_table("a,b\n1,2", Format::Markdown, true, ",", Align::Right).unwrap();
        assert!(r.contains("--:"), "right alignment marker: {r}");
        let c = to_table("a,b\n1,2", Format::Markdown, true, ",", Align::Center).unwrap();
        assert!(c.contains(":-"), "center alignment marker: {c}");
    }

    #[test]
    fn markdown_escapes_pipe() {
        let out = to_table("a\nx|y", Format::Markdown, true, ",", Align::Left).unwrap();
        assert!(out.contains("x\\|y"));
    }

    #[test]
    fn no_header_synthesizes_columns() {
        let out = to_table("1,2\n3,4", Format::Ascii, false, ",", Align::Left).unwrap();
        assert!(out.contains("| Column 1 | Column 2 |"));
        assert!(out.contains("| 1        | 2        |"));
    }

    #[test]
    fn tab_delimiter() {
        let out = to_table("a\tbb\n1\t2", Format::Ascii, true, "tab", Align::Left).unwrap();
        assert!(out.contains("| a | bb |"));
    }

    #[test]
    fn ragged_rows_padded() {
        let out = to_table("a,b,c\n1,2", Format::Ascii, true, ",", Align::Left).unwrap();
        assert!(out.contains("| 1 | 2 |   |"));
    }

    #[test]
    fn unicode_width_chars() {
        // 'é' is two bytes but one char — alignment must use char width.
        let out = to_table("x\né", Format::Ascii, true, ",", Align::Left).unwrap();
        assert!(out.contains("| x |"));
        assert!(out.contains("| é |"));
    }

    #[test]
    fn errors() {
        assert!(to_table("", Format::Ascii, true, ",", Align::Left).is_err());
        assert!(Format::parse("latex").is_err());
        assert!(Align::parse("middle").is_err());
        assert!(to_table("a,b", Format::Ascii, true, "xyz", Align::Left).is_err());
    }
}
