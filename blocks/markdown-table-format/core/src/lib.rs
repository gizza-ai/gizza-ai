//! markdown-table-format core — align and pretty-print GitHub-flavored Markdown
//! pipe tables. Dependency-free, shared by the chat skill block and the web page.
//!
//! It scans the input for blocks of consecutive lines that look like a GFM table
//! (a header row, a delimiter row of `---`/`:--`/`--:`/`:-:` cells, then zero or
//! more body rows), recomputes each column's width from the widest cell, and
//! re-renders every row with aligned `|` separators and a normalized delimiter
//! row that carries the column alignment. Text outside tables is passed through
//! unchanged, so a whole document can be reformatted in one pass.
//!
//! Cell width is measured in Unicode scalar values widened for East-Asian
//! wide/fullwidth characters (CJK, etc.) so columns line up in a monospace font.
//! Inline `\|` escapes inside a cell are preserved (they don't split the cell).

/// Per-column alignment, derived from the delimiter row (`:--` left, `--:` right,
/// `:-:` center, `---` none).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

/// How to choose each column's alignment for the rendered output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignMode {
    /// Keep whatever the source delimiter row specified.
    Keep,
    Left,
    Center,
    Right,
}

/// Output layout: `Pretty` pads every column to its widest cell so the grid lines
/// up in a monospace font; `Compact` uses a single space of padding (no width
/// alignment) for the smallest, diff-friendly output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Pretty,
    Compact,
}

impl Style {
    fn parse(s: &str) -> Result<Style, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "pretty" | "padded" | "align" | "aligned" => Ok(Style::Pretty),
            "compact" | "minify" | "minified" | "narrow" => Ok(Style::Compact),
            other => Err(format!("unknown style '{other}' (use pretty | compact)")),
        }
    }
}

impl AlignMode {
    fn parse(s: &str) -> Result<AlignMode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "keep" | "preserve" => Ok(AlignMode::Keep),
            "left" => Ok(AlignMode::Left),
            "center" | "centre" => Ok(AlignMode::Center),
            "right" => Ok(AlignMode::Right),
            other => Err(format!(
                "unknown align '{other}' (use keep | left | center | right)"
            )),
        }
    }
}

/// Monospace display width: most scalars count as 1, East-Asian wide/fullwidth
/// ranges count as 2, zero-width / combining marks count as 0.
fn ch_width(c: char) -> usize {
    let u = c as u32;
    // Zero-width: combining marks, ZWSP/ZWNJ/ZWJ, variation selectors, BOM.
    if (0x0300..=0x036F).contains(&u)
        || (0x200B..=0x200F).contains(&u)
        || (0xFE00..=0xFE0F).contains(&u)
        || u == 0xFEFF
    {
        return 0;
    }
    // East-Asian Wide & Fullwidth (a practical subset covering CJK + emoji).
    let wide = (0x1100..=0x115F).contains(&u)        // Hangul Jamo
        || (0x2E80..=0x303E).contains(&u)            // CJK radicals, Kangxi, punctuation
        || (0x3041..=0x33FF).contains(&u)            // Hiragana..CJK compat
        || (0x3400..=0x4DBF).contains(&u)            // CJK Ext A
        || (0x4E00..=0x9FFF).contains(&u)            // CJK Unified
        || (0xA000..=0xA4CF).contains(&u)            // Yi
        || (0xAC00..=0xD7A3).contains(&u)            // Hangul syllables
        || (0xF900..=0xFAFF).contains(&u)            // CJK compat ideographs
        || (0xFE30..=0xFE4F).contains(&u)            // CJK compat forms
        || (0xFF00..=0xFF60).contains(&u)            // Fullwidth forms
        || (0xFFE0..=0xFFE6).contains(&u)            // Fullwidth signs
        || (0x1F300..=0x1FAFF).contains(&u)          // emoji / symbols
        || (0x20000..=0x3FFFD).contains(&u); // CJK Ext B+
    if wide {
        2
    } else {
        1
    }
}

fn str_width(s: &str) -> usize {
    s.chars().map(ch_width).sum()
}

/// True if a line, trimmed, looks like the body of a pipe-table row (contains an
/// unescaped `|`).
fn looks_like_row(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    t.contains('|')
}

/// Split one table row into its cell strings, honoring `\|` escapes and trimming
/// a single leading/trailing `|`. Returns trimmed cell contents.
fn split_cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let mut cells: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut prev_backslash = false;
    for c in t.chars() {
        if c == '|' && !prev_backslash {
            cells.push(cur.clone());
            cur.clear();
        } else {
            cur.push(c);
        }
        prev_backslash = c == '\\' && !prev_backslash;
    }
    cells.push(cur);
    // A leading `|` makes the first cell empty; a trailing `|` makes the last
    // cell empty. Drop those boundary empties so `| a | b |` == `a | b`.
    if !cells.is_empty() && cells.first().map(|s| s.trim().is_empty()).unwrap_or(false) {
        cells.remove(0);
    }
    if cells.len() > 1 && cells.last().map(|s| s.trim().is_empty()).unwrap_or(false) {
        cells.pop();
    }
    cells.into_iter().map(|c| c.trim().to_string()).collect()
}

/// Parse a delimiter row cell like `:---:` into an Align, or None if it isn't a
/// valid delimiter cell.
fn parse_delim_cell(cell: &str) -> Option<Align> {
    let c = cell.trim();
    if c.is_empty() {
        return None;
    }
    let left = c.starts_with(':');
    let right = c.ends_with(':');
    let core = c.trim_start_matches(':').trim_end_matches(':');
    if core.is_empty() || !core.chars().all(|ch| ch == '-') {
        return None;
    }
    Some(match (left, right) {
        (true, true) => Align::Center,
        (true, false) => Align::Left,
        (false, true) => Align::Right,
        (false, false) => Align::None,
    })
}

/// Is this line a valid GFM delimiter row? Returns the per-column alignments.
fn parse_delim_row(line: &str) -> Option<Vec<Align>> {
    if !line.contains('-') {
        return None;
    }
    let cells = split_cells(line);
    if cells.is_empty() {
        return None;
    }
    let mut aligns = Vec::with_capacity(cells.len());
    for cell in &cells {
        aligns.push(parse_delim_cell(cell)?);
    }
    Some(aligns)
}

fn pad(content: &str, target: usize, align: Align) -> String {
    let w = str_width(content);
    let fill = target.saturating_sub(w);
    match align {
        Align::Right => format!("{}{}", " ".repeat(fill), content),
        Align::Center => {
            let left = fill / 2;
            let right = fill - left;
            format!("{}{}{}", " ".repeat(left), content, " ".repeat(right))
        }
        // Left and None both left-align the visible content.
        _ => format!("{}{}", content, " ".repeat(fill)),
    }
}

/// Render the delimiter cell for a column, fitting it to `width` and carrying the
/// alignment markers.
fn delim_cell(width: usize, align: Align) -> String {
    let (left, right) = match align {
        Align::None => (false, false),
        Align::Left => (true, false),
        Align::Right => (false, true),
        Align::Center => (true, true),
    };
    let markers = left as usize + right as usize;
    let dashes = width.max(3).saturating_sub(markers).max(1);
    let mut s = String::new();
    if left {
        s.push(':');
    }
    s.push_str(&"-".repeat(dashes));
    if right {
        s.push(':');
    }
    s
}

/// Detect a fenced-code-block fence line (``` or ~~~), returning its fence char
/// + length so the matching closing fence (same char, >= length) can be found.
fn fence_marker(line: &str) -> Option<(char, usize)> {
    let t = line.trim_start();
    for ch in ['`', '~'] {
        let n = t.chars().take_while(|&c| c == ch).count();
        if n >= 3 {
            return Some((ch, n));
        }
    }
    None
}

/// Format every Markdown table found in `input`, preserving the source column
/// alignment (`keep`) and the pretty (padded) layout. Convenience wrapper.
pub fn format_tables(input: &str, align_mode: &str) -> Result<String, String> {
    format_tables_styled(input, align_mode, "pretty")
}

/// Format every Markdown table found in `input`. `align_mode` overrides the
/// per-column alignment (`keep` = use the source delimiter row's); `style`
/// chooses the layout (`pretty` = width-aligned grid, `compact` = single-space
/// padding). Tables inside fenced code blocks (``` / ~~~) are left untouched.
pub fn format_tables_styled(
    input: &str,
    align_mode: &str,
    style: &str,
) -> Result<String, String> {
    let mode = AlignMode::parse(align_mode)?;
    let style = Style::parse(style)?;
    let lines: Vec<&str> = input.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    let mut formatted_any = false;
    let mut in_fence: Option<(char, usize)> = None;
    while i < lines.len() {
        // Track fenced code blocks so tables inside them are never reformatted.
        if let Some((fc, fn_)) = fence_marker(lines[i]) {
            match in_fence {
                None => in_fence = Some((fc, fn_)),
                Some((oc, on)) if oc == fc && fn_ >= on => in_fence = None,
                _ => {}
            }
            out.push(lines[i].to_string());
            i += 1;
            continue;
        }
        // A table is a header row immediately followed by a delimiter row.
        if in_fence.is_none() && i + 1 < lines.len() && looks_like_row(lines[i]) {
            if let Some(aligns) = parse_delim_row(lines[i + 1]) {
                let header = split_cells(lines[i]);
                if header.len() == aligns.len() {
                    // Gather body rows.
                    let mut body: Vec<Vec<String>> = Vec::new();
                    let mut j = i + 2;
                    while j < lines.len() && looks_like_row(lines[j]) {
                        body.push(split_cells(lines[j]));
                        j += 1;
                    }
                    let rendered = render_table(&header, &aligns, &body, mode, style);
                    out.extend(rendered);
                    formatted_any = true;
                    i = j;
                    continue;
                }
            }
        }
        out.push(lines[i].to_string());
        i += 1;
    }
    if !formatted_any {
        return Err(
            "no Markdown table found — input needs a header row, a |---|---| delimiter row, then body rows"
                .into(),
        );
    }
    Ok(out.join("\n"))
}

fn render_table(
    header: &[String],
    src_aligns: &[Align],
    body: &[Vec<String>],
    mode: AlignMode,
    style: Style,
) -> Vec<String> {
    let ncols = header.len();
    // Resolve final alignment per column.
    let aligns: Vec<Align> = (0..ncols)
        .map(|c| match mode {
            AlignMode::Keep => src_aligns[c],
            AlignMode::Left => Align::Left,
            AlignMode::Center => Align::Center,
            AlignMode::Right => Align::Right,
        })
        .collect();

    // Compute each column's content width (max over header + body cells), with a
    // floor of 3 so the delimiter row never collapses below `---`. In compact
    // style we don't pad to the widest cell — each column is just its own content
    // width (still floored at 3 for a valid delimiter), so cells aren't widened.
    let mut widths = vec![0usize; ncols];
    if style == Style::Pretty {
        for (c, h) in header.iter().enumerate() {
            widths[c] = widths[c].max(str_width(h));
        }
        for row in body {
            for (c, w) in widths.iter_mut().enumerate() {
                if let Some(cell) = row.get(c) {
                    *w = (*w).max(str_width(cell));
                }
            }
        }
        for w in widths.iter_mut() {
            *w = (*w).max(3);
        }
    }

    let render_row = |cells: &[String]| -> String {
        let mut parts = Vec::with_capacity(ncols);
        for c in 0..ncols {
            let content = cells.get(c).map(|s| s.as_str()).unwrap_or("");
            // Pretty pads to the column width; compact emits the bare content.
            parts.push(pad(content, widths[c], aligns[c]));
        }
        format!("| {} |", parts.join(" | "))
    };

    let mut lines = Vec::with_capacity(2 + body.len());
    lines.push(render_row(header));
    // Delimiter row. In compact style every column collapses to the minimal
    // `---` (+ alignment colons); in pretty style it fills the column width.
    let delims: Vec<String> = (0..ncols)
        .map(|c| delim_cell(widths[c], aligns[c]))
        .collect();
    lines.push(format!("| {} |", delims.join(" | ")));
    for row in body {
        lines.push(render_row(row));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_ragged_table() {
        let input = "|Name|Role|\n|---|---|\n|Ada|Engineer|\n|Bo|Designer|";
        let got = format_tables(input, "keep").unwrap();
        let expected = "\
| Name | Role     |
| ---- | -------- |
| Ada  | Engineer |
| Bo   | Designer |";
        assert_eq!(got, expected);
    }

    #[test]
    fn preserves_and_renders_alignment() {
        let input = "| a | b | c |\n|:--|:-:|--:|\n| 1 | 2 | 3 |";
        let got = format_tables(input, "keep").unwrap();
        assert!(got.contains(":--"), "left marker kept: {got}");
        assert!(got.contains(":-:"), "center marker kept: {got}");
        assert!(got.contains("--:"), "right marker kept: {got}");
        // Right-aligned column pads on the left.
        assert!(got.lines().any(|l| l.contains("|   c |")), "right pad: {got}");
    }

    #[test]
    fn override_alignment_right() {
        let input = "|a|bb|\n|---|---|\n|1|2|";
        let got = format_tables(input, "right").unwrap();
        // Every delimiter cell should now carry the right marker `--:`.
        let delim = got.lines().nth(1).unwrap();
        assert!(delim.contains("--:"), "delim row: {delim}");
        assert!(!delim.contains(":--"), "no left marker: {delim}");
    }

    #[test]
    fn passes_through_non_table_text() {
        let input = "# Title\n\nSome prose.\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nMore text.";
        let got = format_tables(input, "keep").unwrap();
        assert!(got.starts_with("# Title\n\nSome prose."), "prose kept: {got}");
        assert!(got.ends_with("More text."), "trailing kept: {got}");
        // The table itself is reformatted (cells padded to the 3-wide floor).
        assert!(got.contains("| a   | b   |"), "table reformatted: {got}");
    }

    #[test]
    fn ragged_rows_get_padded() {
        // A short body row (missing a cell) is padded out, not dropped.
        let input = "| a | b | c |\n|---|---|---|\n| 1 | 2 |";
        let got = format_tables(input, "keep").unwrap();
        let last = got.lines().last().unwrap();
        assert!(last.starts_with("| 1   | 2   |"), "padded row: {last}");
        assert_eq!(last.matches('|').count(), 4, "three columns: {last}");
    }

    #[test]
    fn wide_cjk_columns_line_up() {
        let input = "| 名前 | x |\n|---|---|\n| a | y |";
        let got = format_tables(input, "keep").unwrap();
        let header = got.lines().next().unwrap();
        assert!(header.contains("| 名前 |"), "header: {header}");
        // Body 'a' padded to width 4 (名前).
        assert!(got.lines().any(|l| l.starts_with("| a    |")), "pad cjk: {got}");
    }

    #[test]
    fn escaped_pipe_not_split() {
        let input = "| a | b |\n|---|---|\n| x \\| y | z |";
        let got = format_tables(input, "keep").unwrap();
        assert!(got.contains("x \\| y"), "escaped pipe kept in one cell: {got}");
        // The escaped pipe stays inside cell 1, so cell 2 is still `z`.
        let last = got.lines().last().unwrap();
        assert!(last.contains("x \\| y |"), "cell boundary after escaped pipe: {last}");
        assert!(last.trim_end().ends_with("z   |"), "second cell is z: {last}");
        // split_cells round-trips to exactly two cells.
        assert_eq!(split_cells(last).len(), 2, "two columns: {last}");
    }

    #[test]
    fn error_when_no_table() {
        let err = format_tables("just some text\nwith no table", "keep").unwrap_err();
        assert!(err.contains("no Markdown table"), "err: {err}");
    }

    #[test]
    fn error_on_bad_align() {
        let err = format_tables("|a|\n|---|\n|1|", "sideways").unwrap_err();
        assert!(err.contains("unknown align"), "err: {err}");
    }

    #[test]
    fn compact_style_does_not_pad() {
        let input = "|Name|Role|\n|---|---|\n|Ada|Engineer|\n|Bo|Designer|";
        let got = format_tables_styled(input, "keep", "compact").unwrap();
        let expected = "\
| Name | Role |
| --- | --- |
| Ada | Engineer |
| Bo | Designer |";
        assert_eq!(got, expected);
    }

    #[test]
    fn compact_keeps_alignment_markers() {
        let input = "| a | b | c |\n|:--|:-:|--:|\n| 1 | 2 | 3 |";
        let got = format_tables_styled(input, "keep", "compact").unwrap();
        let delim = got.lines().nth(1).unwrap();
        assert_eq!(delim, "| :-- | :-: | --: |", "compact delim: {delim}");
    }

    #[test]
    fn tables_inside_fenced_code_are_untouched() {
        let input = "\
```
|a|b|
|---|---|
|1|2|
```
| x | y |
|---|---|
| 9 | 8 |";
        let got = format_tables_styled(input, "keep", "pretty").unwrap();
        // The fenced table is byte-identical to the input (not reformatted).
        assert!(got.contains("|a|b|\n|---|---|\n|1|2|"), "fenced table kept: {got}");
        // The real table after the fence IS reformatted (cells padded).
        assert!(got.contains("| x   | y   |"), "outside table formatted: {got}");
        assert!(got.contains("| 9   | 8   |"), "outside body formatted: {got}");
    }

    #[test]
    fn tilde_fence_also_respected() {
        let input = "~~~\n|a|b|\n|---|---|\n~~~\n| c | d |\n|---|---|\n| 1 | 2 |";
        let got = format_tables_styled(input, "keep", "pretty").unwrap();
        assert!(got.contains("|a|b|\n|---|---|"), "tilde-fenced kept: {got}");
        assert!(got.contains("| c   | d   |"), "real table formatted: {got}");
    }

    #[test]
    fn error_on_bad_style() {
        let err = format_tables_styled("|a|\n|---|\n|1|", "keep", "fancy").unwrap_err();
        assert!(err.contains("unknown style"), "err: {err}");
    }

    #[test]
    fn fenced_only_table_errors_as_no_table() {
        // A table that lives ONLY inside a code fence isn't a real table.
        let input = "```\n|a|b|\n|---|---|\n|1|2|\n```";
        let err = format_tables_styled(input, "keep", "pretty").unwrap_err();
        assert!(err.contains("no Markdown table"), "err: {err}");
    }
}
