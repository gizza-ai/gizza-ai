//! gizza-ai/csv-to-table core — convert CSV into a Markdown or HTML table.
//! Pure-Rust (`csv`). No wafer/wasm-bindgen deps.

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Output table format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Html,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" | "" => Ok(Format::Markdown),
            "html" => Ok(Format::Html),
            other => Err(format!("unknown format '{other}' (use markdown or html)")),
        }
    }
}

fn md_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('|', "\\|").replace('\n', " ")
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Convert `data` (CSV) into a Markdown or HTML table.
pub fn to_table(data: &str, format: Format, has_header: bool, delimiter: &str) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> =
        rdr.records().collect::<Result<_, _>>().map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Err("no rows found".into());
    }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);
    let cell = |rec: &csv::StringRecord, i: usize| rec.get(i).unwrap_or("").to_string();

    let (header, body): (Vec<String>, &[csv::StringRecord]) = if has_header {
        ((0..width).map(|i| cell(&records[0], i)).collect(), &records[1..])
    } else {
        ((1..=width).map(|i| format!("Column {i}")).collect(), &records[..])
    };

    match format {
        Format::Markdown => {
            let mut out = String::new();
            out.push_str(&format!("| {} |\n", header.iter().map(|h| md_escape(h)).collect::<Vec<_>>().join(" | ")));
            out.push_str(&format!("| {} |\n", vec!["---"; width].join(" | ")));
            for rec in body {
                let cells: Vec<String> = (0..width).map(|i| md_escape(&cell(rec, i))).collect();
                out.push_str(&format!("| {} |\n", cells.join(" | ")));
            }
            Ok(out.trim_end().to_string())
        }
        Format::Html => {
            let mut out = String::from("<table>\n");
            out.push_str("  <thead>\n    <tr>");
            for h in &header {
                out.push_str(&format!("<th>{}</th>", html_escape(h)));
            }
            out.push_str("</tr>\n  </thead>\n  <tbody>\n");
            for rec in body {
                out.push_str("    <tr>");
                for i in 0..width {
                    out.push_str(&format!("<td>{}</td>", html_escape(&cell(rec, i))));
                }
                out.push_str("</tr>\n");
            }
            out.push_str("  </tbody>\n</table>");
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_table() {
        let out = to_table("a,b\n1,2\n3,4", Format::Markdown, true, ",").unwrap();
        assert_eq!(out, "| a | b |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |");
    }

    #[test]
    fn markdown_escapes_pipe() {
        let out = to_table("a\nx|y", Format::Markdown, true, ",").unwrap();
        assert!(out.contains("x\\|y"));
    }

    #[test]
    fn html_table() {
        let out = to_table("a,b\n1,2", Format::Html, true, ",").unwrap();
        assert!(out.starts_with("<table>"));
        assert!(out.contains("<th>a</th><th>b</th>"));
        assert!(out.contains("<td>1</td><td>2</td>"));
        assert!(out.ends_with("</table>"));
    }

    #[test]
    fn html_escapes() {
        let out = to_table("x\n<b>&", Format::Html, true, ",").unwrap();
        assert!(out.contains("<td>&lt;b&gt;&amp;</td>"));
    }

    #[test]
    fn no_header_synthesizes_columns() {
        let out = to_table("1,2\n3,4", Format::Markdown, false, ",").unwrap();
        assert!(out.starts_with("| Column 1 | Column 2 |"));
        assert!(out.contains("| 1 | 2 |"));
    }

    #[test]
    fn tab_delimiter() {
        let out = to_table("a\tb\n1\t2", Format::Markdown, true, "tab").unwrap();
        assert!(out.contains("| a | b |"));
    }

    #[test]
    fn errors() {
        assert!(to_table("", Format::Markdown, true, ",").is_err());
        assert!(Format::parse("latex").is_err());
    }
}
