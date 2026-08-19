//! gizza-ai/column-aligner core — align whitespace- or delimiter-separated
//! columns into a neat fixed-width layout, the way `column -t` does.
//!
//! The output stays PLAIN TEXT: every column is padded to the display width of
//! its widest cell, columns are joined by a fixed gap (optionally with a
//! separator string), and no borders, rules or header row are added. Padding is
//! computed with Unicode display width so CJK and emoji cells line up in a
//! monospace font.
//!
//! Pure-Rust (`unicode-width`). No wafer/wasm-bindgen deps.

use unicode_width::UnicodeWidthStr;

/// Maximum number of input lines accepted in one run.
pub const MAX_LINES: usize = 20_000;
/// Maximum number of columns any row may produce.
pub const MAX_COLUMNS: usize = 512;
/// Smallest allowed `gap` (columns may touch when a separator is used).
pub const MIN_GAP: u32 = 0;
/// Largest allowed `gap`.
pub const MAX_GAP: u32 = 16;
/// Default gap, matching `column -t`.
pub const DEFAULT_GAP: u32 = 2;

/// How a column's cells are padded within their column width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// Pad on the right (text hugs the left edge).
    Left,
    /// Pad on the left (text hugs the right edge).
    Right,
    /// Split the padding, extra space on the right.
    Center,
    /// Right for columns whose non-empty cells are all numeric, left otherwise.
    Auto,
}

impl Align {
    /// Parse the whole-document alignment name.
    pub fn parse(s: &str) -> Result<Align, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "left" | "l" => Ok(Align::Left),
            "right" | "r" => Ok(Align::Right),
            "center" | "centre" | "c" => Ok(Align::Center),
            "auto" | "a" => Ok(Align::Auto),
            other => Err(format!(
                "unknown align '{other}': expected left, right, center or auto"
            )),
        }
    }
}

/// How input lines are split into fields.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Delim {
    /// Split on runs of any whitespace (the `column -t` behaviour).
    Whitespace,
    /// Split on a literal string.
    Literal(String),
}

/// Resolve the `delimiter` argument: a friendly name, or any literal string.
fn parse_delimiter(d: &str) -> Delim {
    // Names are matched case-insensitively; anything else is taken literally, so
    // a delimiter of " " (one space) still means "one space", not "whitespace".
    match d.to_ascii_lowercase().as_str() {
        "" | "whitespace" | "ws" | "auto" | "spaces" => Delim::Whitespace,
        "tab" | "\\t" => Delim::Literal("\t".into()),
        "comma" => Delim::Literal(",".into()),
        "semicolon" => Delim::Literal(";".into()),
        "colon" => Delim::Literal(":".into()),
        "pipe" => Delim::Literal("|".into()),
        "space" => Delim::Literal(" ".into()),
        _ => Delim::Literal(d.to_string()),
    }
}

/// Parse the per-column override spec, e.g. `lrr`, `l,r,c` or `l r - r`.
/// `-` inherits the whole-document alignment. An empty spec means "no override".
fn parse_column_align(spec: &str, fallback: Align) -> Result<Vec<Align>, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Ok(Vec::new());
    }
    // Accept both compact ("lrr") and separated ("l,r,r" / "left right") forms.
    let tokens: Vec<String> = if spec.contains([',', ' ', '|', ';', '\t']) {
        spec.split([',', ' ', '|', ';', '\t'])
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string())
            .collect()
    } else {
        spec.chars().map(|c| c.to_string()).collect()
    };
    tokens
        .iter()
        .map(|t| match t.to_ascii_lowercase().as_str() {
            "-" | "." | "d" | "default" => Ok(fallback),
            "l" | "left" => Ok(Align::Left),
            "r" | "right" => Ok(Align::Right),
            "c" | "center" | "centre" => Ok(Align::Center),
            "a" | "auto" => Ok(Align::Auto),
            other => Err(format!(
                "unknown column alignment '{other}' in column_align: use l/left, r/right, \
                 c/center, a/auto, or - to inherit the align setting"
            )),
        })
        .collect()
}

/// Does this cell read as a number? Used by `align = auto`.
///
/// Accepts an optional sign, a leading currency symbol, `,`/`_` group
/// separators and a trailing `%` — the shapes that show up in pasted reports.
fn is_numeric(cell: &str) -> bool {
    let t = cell.trim();
    if t.is_empty() {
        return false;
    }
    let t = t.strip_suffix('%').unwrap_or(t);
    let rest = t.strip_prefix(['+', '-']).unwrap_or(t);
    let rest = rest.trim_start_matches(['$', '€', '£', '¥']);
    let cleaned: String = rest.chars().filter(|c| *c != ',' && *c != '_').collect();
    if cleaned.is_empty() {
        return false;
    }
    cleaned.parse::<f64>().is_ok()
}

/// Display width of a cell in terminal columns (CJK counts as 2).
fn width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Pad `s` to `w` display columns according to `align`.
fn pad(s: &str, w: usize, align: Align) -> String {
    let len = width(s);
    if len >= w {
        return s.to_string();
    }
    let total = w - len;
    match align {
        Align::Left | Align::Auto => format!("{s}{}", " ".repeat(total)),
        Align::Right => format!("{}{s}", " ".repeat(total)),
        Align::Center => {
            let left = total / 2;
            format!("{}{s}{}", " ".repeat(left), " ".repeat(total - left))
        }
    }
}

/// Split one line into fields.
fn split_line(line: &str, delim: &Delim, trim: bool) -> Vec<String> {
    match delim {
        Delim::Whitespace => line.split_whitespace().map(|f| f.to_string()).collect(),
        Delim::Literal(d) => {
            if line.is_empty() {
                return Vec::new();
            }
            line.split(d.as_str())
                .map(|f| if trim { f.trim().to_string() } else { f.to_string() })
                .collect()
        }
    }
}

/// Align `text` into fixed-width columns.
///
/// * `delimiter` — `whitespace` (default), a name (`tab`/`comma`/`semicolon`/
///   `colon`/`pipe`/`space`) or any literal string.
/// * `align` — `left` (default), `right`, `center` or `auto`.
/// * `column_align` — optional per-column override such as `lrr`; empty = none.
/// * `gap` — spaces between columns (0–16, default 2); with a non-empty
///   `separator` the gap is applied on BOTH sides of it.
/// * `separator` — optional string drawn between columns (e.g. `|`).
/// * `trim` — strip whitespace around each field before aligning.
///
/// Trailing spaces are never emitted, blank lines are preserved, and the result
/// has no trailing newline.
pub fn align_columns(
    text: &str,
    delimiter: &str,
    align: &str,
    column_align: &str,
    gap: u32,
    separator: &str,
    trim: bool,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("input is empty: paste at least one line of text to align".into());
    }
    if gap > MAX_GAP {
        return Err(format!(
            "gap must be between {MIN_GAP} and {MAX_GAP} spaces, got {gap}"
        ));
    }
    let default_align = Align::parse(align)?;
    let overrides = parse_column_align(column_align, default_align)?;
    let delim = parse_delimiter(delimiter);

    // Normalise line endings; a single trailing newline is not reproduced.
    let normalised = text.replace("\r\n", "\n");
    let body = normalised.strip_suffix('\n').unwrap_or(&normalised);
    let lines: Vec<&str> = body.split('\n').collect();
    if lines.len() > MAX_LINES {
        return Err(format!(
            "too many lines: {} (limit {MAX_LINES}) — split the input and align it in parts",
            lines.len()
        ));
    }

    let rows: Vec<Vec<String>> = lines
        .iter()
        .map(|l| split_line(l, &delim, trim))
        .collect();
    let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if ncols > MAX_COLUMNS {
        return Err(format!(
            "too many columns: {ncols} (limit {MAX_COLUMNS}) — check the delimiter matches the data"
        ));
    }
    if ncols == 0 {
        return Err("no fields found: every line was blank after splitting".into());
    }

    // Resolve each column's alignment once: override → default → numeric probe.
    let col_aligns: Vec<Align> = (0..ncols)
        .map(|i| {
            let a = overrides.get(i).copied().unwrap_or(default_align);
            if a != Align::Auto {
                return a;
            }
            let cells: Vec<&str> = rows
                .iter()
                .filter_map(|r| r.get(i))
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if !cells.is_empty() && cells.iter().all(|c| is_numeric(c)) {
                Align::Right
            } else {
                Align::Left
            }
        })
        .collect();

    let mut widths = vec![0usize; ncols];
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(width(cell));
        }
    }

    let spaces = " ".repeat(gap as usize);
    let joint = if separator.is_empty() {
        spaces.clone()
    } else {
        format!("{spaces}{separator}{spaces}")
    };

    let mut out = String::new();
    for (n, row) in rows.iter().enumerate() {
        if n > 0 {
            out.push('\n');
        }
        if row.is_empty() {
            // Blank input line stays a blank output line.
            continue;
        }
        // Only pad up to the last populated cell of this row — a ragged row must
        // not grow trailing filler for columns it never had.
        let last = row.len() - 1;
        let mut line = String::new();
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                line.push_str(&joint);
            }
            if i == last {
                line.push_str(cell);
            } else {
                line.push_str(&pad(cell, widths[i], col_aligns[i]));
            }
        }
        // A right/centre-aligned final cell still needs its leading pad.
        if last > 0 || col_aligns[last] != Align::Left {
            line = {
                let mut rebuilt = String::new();
                for (i, cell) in row.iter().enumerate() {
                    if i > 0 {
                        rebuilt.push_str(&joint);
                    }
                    rebuilt.push_str(&pad(cell, widths[i], col_aligns[i]));
                }
                rebuilt
            };
        }
        while line.ends_with(' ') {
            line.pop();
        }
        out.push_str(&line);
    }
    Ok(out)
}

/// Descriptor/web wrapper with the same argument order exposed by the CLI and page.
pub fn run(
    input: &str,
    delimiter: &str,
    align: &str,
    column_align: &str,
    gap: u32,
    separator: &str,
    trim: bool,
) -> Result<String, String> {
    align_columns(input, delimiter, align, column_align, gap, separator, trim)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_whitespace_columns_like_column_t() {
        let got = align_columns(
            "name age city\nalice 30 Berlin\nbo 7 SF",
            "whitespace",
            "left",
            "",
            2,
            "",
            true,
        )
        .unwrap();
        assert_eq!(got, "name   age  city\nalice  30   Berlin\nbo     7    SF");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = align_columns("   \n  ", "whitespace", "left", "", 2, "", true).unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn unknown_align_names_the_valid_values() {
        let err = align_columns("a b", "whitespace", "sideways", "", 2, "", true).unwrap_err();
        assert!(err.contains("left, right, center or auto"), "{err}");
    }

    #[test]
    fn gap_above_the_cap_is_rejected_with_the_limit() {
        let err = align_columns("a b", "whitespace", "left", "", 17, "", true).unwrap_err();
        assert!(err.contains("0 and 16"), "{err}");
    }

    #[test]
    fn bad_column_align_token_is_rejected() {
        let err = align_columns("a b", "whitespace", "left", "lx", 2, "", true).unwrap_err();
        assert!(err.contains("unknown column alignment 'x'"), "{err}");
    }

    #[test]
    fn right_align_pads_on_the_left() {
        let got = align_columns("a 1\nbbb 22", "whitespace", "right", "", 1, "", true).unwrap();
        assert_eq!(got, "  a  1\nbbb 22");
    }

    #[test]
    fn center_align_puts_extra_space_on_the_right() {
        let got = align_columns("a\nbbbb", "whitespace", "center", "", 2, "", true).unwrap();
        assert_eq!(got, " a\nbbbb");
    }

    #[test]
    fn auto_right_aligns_numeric_columns_only() {
        let got = align_columns(
            "item qty\nwidget 5\nbolt 1200",
            "whitespace",
            "auto",
            "",
            2,
            "",
            true,
        )
        .unwrap();
        // "qty" is a header, so the column is not all-numeric → stays left.
        assert_eq!(got, "item    qty\nwidget  5\nbolt    1200");
        let got = align_columns("widget 5\nbolt 1200", "whitespace", "auto", "", 2, "", true)
            .unwrap();
        assert_eq!(got, "widget  \u{20}  5\nbolt    1200");
    }

    #[test]
    fn per_column_align_overrides_the_default() {
        let got = align_columns("a 1\nbbb 2222", "whitespace", "left", "lr", 2, "", true).unwrap();
        assert_eq!(got, "a       1\nbbb  2222");
    }

    #[test]
    fn per_column_align_accepts_separated_tokens_and_inherits_the_rest() {
        let got = align_columns(
            "a 1 x\nbbb 2222 yy",
            "whitespace",
            "left",
            "-,right",
            1,
            "",
            true,
        )
        .unwrap();
        assert_eq!(got, "a      1 x\nbbb 2222 yy");
    }

    #[test]
    fn comma_delimiter_trims_fields_by_default() {
        let got =
            align_columns("a,  bb\nccc,d", "comma", "left", "", 2, "", true).unwrap();
        assert_eq!(got, "a    bb\nccc  d");
    }

    #[test]
    fn trim_off_keeps_the_original_field_padding() {
        let got = align_columns("a, bb\nccc,d", "comma", "left", "", 1, "", false).unwrap();
        assert_eq!(got, "a    bb\nccc d");
    }

    #[test]
    fn separator_is_padded_on_both_sides_by_gap() {
        let got = align_columns("a 1\nbbb 22", "whitespace", "left", "", 1, "|", true).unwrap();
        assert_eq!(got, "a   | 1\nbbb | 22");
    }

    #[test]
    fn literal_multi_char_delimiter_is_taken_verbatim() {
        let got = align_columns("a::bb\nccc::d", "::", "left", "", 2, "", true).unwrap();
        assert_eq!(got, "a    bb\nccc  d");
    }

    #[test]
    fn ragged_rows_do_not_grow_trailing_whitespace() {
        let got = align_columns("a bb ccc\nx\ny zz", "whitespace", "left", "", 2, "", true).unwrap();
        assert_eq!(got, "a  bb  ccc\nx\ny  zz");
        for line in got.lines() {
            assert!(!line.ends_with(' '), "trailing space in {line:?}");
        }
    }

    #[test]
    fn blank_lines_are_preserved() {
        let got = align_columns("a b\n\nccc d", "whitespace", "left", "", 2, "", true).unwrap();
        assert_eq!(got, "a    b\n\nccc  d");
    }

    #[test]
    fn crlf_input_is_normalised_and_no_trailing_newline_is_emitted() {
        let got = align_columns("a b\r\nccc d\r\n", "whitespace", "left", "", 2, "", true).unwrap();
        assert_eq!(got, "a    b\nccc  d");
    }

    #[test]
    fn cjk_cells_align_by_display_width() {
        let got = align_columns("東京 1\nab 2", "whitespace", "left", "", 2, "", true).unwrap();
        // "東京" is 4 display columns; "ab" is 2 and gets 2 extra spaces.
        assert_eq!(got, "東京  1\nab    2");
    }

    #[test]
    fn too_many_lines_reports_the_limit() {
        let big = "a b\n".repeat(MAX_LINES + 1);
        let err = align_columns(&big, "whitespace", "left", "", 2, "", true).unwrap_err();
        assert!(err.contains("too many lines"), "{err}");
    }

    #[test]
    fn numeric_detection_covers_report_shapes() {
        assert!(is_numeric("42"));
        assert!(is_numeric("-3.5"));
        assert!(is_numeric("1,200"));
        assert!(is_numeric("$19.99"));
        assert!(is_numeric("12%"));
        assert!(!is_numeric("qty"));
        assert!(!is_numeric(""));
        assert!(!is_numeric("12abc"));
    }
}
