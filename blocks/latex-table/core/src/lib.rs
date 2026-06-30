//! gizza-ai/latex-table core — convert CSV or TSV data into a LaTeX `tabular`
//! (optionally wrapped in a `table` float). Pure-Rust (`csv`). No wafer/wasm deps.

/// Table rule style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// `booktabs` rules: \toprule / \midrule / \bottomrule (needs \usepackage{booktabs}).
    Booktabs,
    /// Classic grid: vertical bars + \hline between every row.
    Grid,
    /// No rules at all.
    Plain,
}

impl Style {
    pub fn parse(s: &str) -> Result<Style, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "booktabs" | "" => Ok(Style::Booktabs),
            "grid" | "lines" | "hlines" => Ok(Style::Grid),
            "plain" | "none" => Ok(Style::Plain),
            other => Err(format!("unknown style '{other}' (use booktabs, grid, or plain)")),
        }
    }
}

fn delim_byte(d: &str, data: &str) -> Result<u8, String> {
    Ok(match d.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => {
            // Detect from the first line: a tab beats a comma beats a semicolon.
            let first = data.lines().next().unwrap_or("");
            if first.contains('\t') {
                b'\t'
            } else if first.contains(',') {
                b','
            } else if first.contains(';') {
                b';'
            } else {
                b','
            }
        }
        "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be a single char or auto/tab/comma/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Escape LaTeX special characters in a cell. Backslash is handled first.
fn latex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '&' => out.push_str("\\&"),
            '%' => out.push_str("\\%"),
            '$' => out.push_str("\\$"),
            '#' => out.push_str("\\#"),
            '_' => out.push_str("\\_"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            '\n' => out.push(' '),
            other => out.push(other),
        }
    }
    out
}

/// Build the column specifier (e.g. "lcr" or "|l|c|r|") for `width` columns.
fn column_spec(alignment: &str, width: usize, grid: bool) -> Result<String, String> {
    let a = alignment.trim();
    let a = if a.is_empty() { "l" } else { a };
    let valid = |c: char| matches!(c, 'l' | 'c' | 'r');
    let cols: Vec<char> = if a.chars().count() == 1 {
        let c = a.chars().next().unwrap();
        if !valid(c) {
            return Err(format!("alignment '{a}' must be l, c, r, or a per-column string of them"));
        }
        vec![c; width]
    } else {
        let chars: Vec<char> = a.chars().collect();
        if chars.len() != width {
            return Err(format!(
                "alignment '{a}' has {} columns but the data has {width}",
                chars.len()
            ));
        }
        for c in &chars {
            if !valid(*c) {
                return Err(format!("alignment '{a}' must only contain l, c, or r"));
            }
        }
        chars
    };
    Ok(if grid {
        let mut s = String::from("|");
        for c in cols {
            s.push(c);
            s.push('|');
        }
        s
    } else {
        cols.into_iter().collect()
    })
}

/// Options for rendering a LaTeX table.
pub struct Options<'a> {
    pub style: Style,
    pub has_header: bool,
    pub alignment: &'a str,
    pub escape: bool,
    pub bold_header: bool,
    pub caption: &'a str,
    pub label: &'a str,
}

/// Convert `data` (CSV or TSV) into a LaTeX table.
pub fn to_latex(data: &str, delimiter: &str, opts: &Options) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let delim = delim_byte(delimiter, data)?;
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
    if width == 0 {
        return Err("no columns found".into());
    }

    let grid = opts.style == Style::Grid;
    let spec = column_spec(opts.alignment, width, grid)?;

    let render_cell = |raw: &str, bold: bool| -> String {
        let cell = if opts.escape { latex_escape(raw) } else { raw.replace('\n', " ") };
        if bold && opts.bold_header {
            format!("\\textbf{{{cell}}}")
        } else {
            cell
        }
    };
    let render_row = |rec: &csv::StringRecord, bold: bool| -> String {
        let cells: Vec<String> =
            (0..width).map(|i| render_cell(rec.get(i).unwrap_or(""), bold)).collect();
        format!("{} \\\\", cells.join(" & "))
    };

    let (header, body): (Option<&csv::StringRecord>, &[csv::StringRecord]) =
        if opts.has_header { (records.first(), &records[1..]) } else { (None, &records[..]) };

    let indent = if !opts.caption.is_empty() || !opts.label.is_empty() { "  " } else { "" };
    let mut tab = String::new();
    tab.push_str(&format!("{indent}\\begin{{tabular}}{{{spec}}}\n"));

    match opts.style {
        Style::Booktabs => {
            tab.push_str(&format!("{indent}\\toprule\n"));
            if let Some(h) = header {
                tab.push_str(&format!("{indent}{}\n", render_row(h, true)));
                tab.push_str(&format!("{indent}\\midrule\n"));
            }
            for rec in body {
                tab.push_str(&format!("{indent}{}\n", render_row(rec, false)));
            }
            tab.push_str(&format!("{indent}\\bottomrule\n"));
        }
        Style::Grid => {
            tab.push_str(&format!("{indent}\\hline\n"));
            if let Some(h) = header {
                tab.push_str(&format!("{indent}{}\n", render_row(h, true)));
                tab.push_str(&format!("{indent}\\hline\n"));
            }
            for rec in body {
                tab.push_str(&format!("{indent}{}\n", render_row(rec, false)));
                tab.push_str(&format!("{indent}\\hline\n"));
            }
        }
        Style::Plain => {
            if let Some(h) = header {
                tab.push_str(&format!("{indent}{}\n", render_row(h, true)));
            }
            for rec in body {
                tab.push_str(&format!("{indent}{}\n", render_row(rec, false)));
            }
        }
    }
    tab.push_str(&format!("{indent}\\end{{tabular}}"));

    if opts.caption.is_empty() && opts.label.is_empty() {
        return Ok(tab);
    }

    let mut out = String::from("\\begin{table}[ht]\n  \\centering\n");
    out.push_str(&tab);
    out.push('\n');
    if !opts.caption.is_empty() {
        let cap = if opts.escape { latex_escape(opts.caption) } else { opts.caption.to_string() };
        out.push_str(&format!("  \\caption{{{cap}}}\n"));
    }
    if !opts.label.is_empty() {
        out.push_str(&format!("  \\label{{{}}}\n", opts.label.trim()));
    }
    out.push_str("\\end{table}");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options<'static> {
        Options {
            style: Style::Booktabs,
            has_header: true,
            alignment: "l",
            escape: true,
            bold_header: false,
            caption: "",
            label: "",
        }
    }

    #[test]
    fn booktabs_basic() {
        let out = to_latex("a,b\n1,2\n3,4", ",", &opts()).unwrap();
        assert_eq!(
            out,
            "\\begin{tabular}{ll}\n\\toprule\na & b \\\\\n\\midrule\n1 & 2 \\\\\n3 & 4 \\\\\n\\bottomrule\n\\end{tabular}"
        );
    }

    #[test]
    fn grid_style_has_hlines_and_bars() {
        let o = Options { style: Style::Grid, ..opts() };
        let out = to_latex("a,b\n1,2", ",", &o).unwrap();
        assert!(out.contains("\\begin{tabular}{|l|l|}"));
        assert!(out.contains("\\hline"));
    }

    #[test]
    fn plain_style_no_rules() {
        let o = Options { style: Style::Plain, ..opts() };
        let out = to_latex("a,b\n1,2", ",", &o).unwrap();
        assert!(!out.contains("rule"));
        assert!(!out.contains("\\hline"));
        assert!(out.contains("a & b \\\\"));
    }

    #[test]
    fn tab_delimiter_and_auto_detect() {
        let explicit = to_latex("a\tb\n1\t2", "tab", &opts()).unwrap();
        assert!(explicit.contains("a & b \\\\"));
        let auto = to_latex("a\tb\n1\t2", "auto", &opts()).unwrap();
        assert!(auto.contains("a & b \\\\"));
    }

    #[test]
    fn no_header_omits_midrule() {
        let o = Options { has_header: false, ..opts() };
        let out = to_latex("1,2\n3,4", ",", &o).unwrap();
        assert!(!out.contains("\\midrule"));
        assert!(out.contains("\\toprule\n1 & 2 \\\\"));
    }

    #[test]
    fn per_column_alignment() {
        let o = Options { alignment: "lcr", ..opts() };
        let out = to_latex("a,b,c\n1,2,3", ",", &o).unwrap();
        assert!(out.contains("\\begin{tabular}{lcr}"));
    }

    #[test]
    fn escapes_special_chars() {
        let out = to_latex("a\n50% & #1_x", ",", &opts()).unwrap();
        assert!(out.contains("50\\% \\& \\#1\\_x"), "got: {out}");
    }

    #[test]
    fn escape_can_be_disabled() {
        let o = Options { escape: false, ..opts() };
        let out = to_latex("a\n\\alpha", ",", &o).unwrap();
        assert!(out.contains("\\alpha"));
        assert!(!out.contains("textbackslash"));
    }

    #[test]
    fn bold_header_wraps_cells() {
        let o = Options { bold_header: true, ..opts() };
        let out = to_latex("a,b\n1,2", ",", &o).unwrap();
        assert!(out.contains("\\textbf{a} & \\textbf{b} \\\\"));
        assert!(out.contains("1 & 2 \\\\"));
    }

    #[test]
    fn caption_and_label_wrap_in_table_float() {
        let o = Options { caption: "My results", label: "tab:res", ..opts() };
        let out = to_latex("a,b\n1,2", ",", &o).unwrap();
        assert!(out.starts_with("\\begin{table}[ht]\n  \\centering"));
        assert!(out.contains("  \\caption{My results}"));
        assert!(out.contains("  \\label{tab:res}"));
        assert!(out.ends_with("\\end{table}"));
        assert!(out.contains("  \\begin{tabular}"));
    }

    #[test]
    fn ragged_rows_padded() {
        let out = to_latex("a,b,c\n1,2", ",", &opts()).unwrap();
        assert!(out.contains("1 & 2 &  \\\\"));
    }

    #[test]
    fn errors() {
        assert!(to_latex("", ",", &opts()).is_err());
        assert!(Style::parse("fancy").is_err());
        let bad = Options { alignment: "xy", ..opts() };
        assert!(to_latex("a,b\n1,2", ",", &bad).is_err());
        let mismatch = Options { alignment: "lc", ..opts() };
        assert!(to_latex("a,b,c\n1,2,3", ",", &mismatch).is_err());
    }
}
