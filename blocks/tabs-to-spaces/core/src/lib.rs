//! tabs-to-spaces core — convert tabs to spaces (`expand`) or spaces back to
//! tabs (`unexpand`), respecting tab stops. Pure compute, no wafer/wasm-bindgen
//! deps; shared by the chat skill block and the web page.
//!
//! Unlike a naive find-and-replace, a tab does NOT become a fixed run of spaces:
//! it advances to the next **tab stop**, i.e. the next column that is a multiple
//! of `tab_width`. So with `tab_width = 4`, a tab at column 0 becomes 4 spaces,
//! but a tab at column 2 (after `"ab"`) becomes only 2 spaces — both land on
//! column 4. The reverse (`unexpand`) turns runs of spaces back into as few tabs
//! as possible, only using a tab when it replaces two or more columns at a stop
//! (so a single space is never turned into a tab).
//!
//! Every character counts as one column except a tab. Each line is processed
//! independently; existing line breaks are preserved exactly.

/// Smallest accepted tab width (columns per tab stop).
pub const MIN_TAB_WIDTH: u32 = 1;
/// Largest accepted tab width. 16 covers every common editor setting (2, 4, 8)
/// with headroom; the value is clamped into `MIN_TAB_WIDTH..=MAX_TAB_WIDTH`.
pub const MAX_TAB_WIDTH: u32 = 16;

/// Convert `text` between tabs and spaces, respecting tab stops.
///
/// - `direction` (`"expand"` | `"unexpand"`, blank → `"expand"`):
///     - `expand`: replace tabs with spaces so each tab reaches the next tab stop.
///     - `unexpand`: replace runs of spaces with tabs where a tab lands exactly on
///       a stop and shortens the run (2+ columns); leftover spaces stay spaces.
/// - `tab_width`: columns per tab stop, clamped to `1..=16` (0 → 1).
/// - `scope` (`"all"` | `"leading"`, blank → `"all"`):
///     - `all`: convert leading indentation AND inline whitespace.
///     - `leading`: convert only the whitespace before the first non-blank
///       character on each line; the rest of the line is copied verbatim.
///
/// Returns `Err` on an invalid `direction` or `scope`.
pub fn convert(text: &str, direction: &str, tab_width: u32, scope: &str) -> Result<String, String> {
    let dir = Direction::parse(direction)?;
    let leading_only = parse_scope(scope)?;
    let tw = tab_width.clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH) as usize;

    let out = text
        .split('\n')
        .map(|line| match dir {
            Direction::Expand => expand_line(line, tw, leading_only),
            Direction::Unexpand => unexpand_line(line, tw, leading_only),
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(out)
}

#[derive(Clone, Copy)]
enum Direction {
    Expand,
    Unexpand,
}

impl Direction {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "expand" | "tabs-to-spaces" => Ok(Direction::Expand),
            "unexpand" | "spaces-to-tabs" => Ok(Direction::Unexpand),
            other => Err(format!(
                "invalid direction {other:?}: expected \"expand\" (tabs to spaces) or \"unexpand\" (spaces to tabs)"
            )),
        }
    }
}

fn parse_scope(s: &str) -> Result<bool, String> {
    match s {
        "" | "all" => Ok(false),
        "leading" => Ok(true),
        other => Err(format!(
            "invalid scope {other:?}: expected \"all\" (leading + inline) or \"leading\" (indentation only)"
        )),
    }
}

/// The next tab-stop column strictly after `col` (the next multiple of `tw`).
fn next_tab_stop(col: usize, tw: usize) -> usize {
    col + (tw - col % tw)
}

/// Expand tabs in one line to spaces, advancing each tab to the next tab stop.
/// When `leading_only`, tabs after the first non-blank character are left as-is
/// (only the indentation is expanded).
fn expand_line(line: &str, tw: usize, leading_only: bool) -> String {
    let mut out = String::with_capacity(line.len());
    let mut col = 0usize;
    let mut seen_nonblank = false;
    for ch in line.chars() {
        if ch == '\t' {
            let stop = next_tab_stop(col, tw);
            if leading_only && seen_nonblank {
                out.push('\t');
            } else {
                for _ in col..stop {
                    out.push(' ');
                }
            }
            col = stop;
        } else {
            out.push(ch);
            col += 1;
            if ch != ' ' {
                seen_nonblank = true;
            }
        }
    }
    out
}

/// Turn runs of blanks in one line back into tabs where a tab lands on a stop.
/// When `leading_only`, only the whitespace before the first non-blank character
/// is re-tabbed; the rest of the line is copied verbatim.
fn unexpand_line(line: &str, tw: usize, leading_only: bool) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut col = 0usize;
    let mut seen_nonblank = false;
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == ' ' || c == '\t' {
            // Measure the maximal blank run in *columns* (tabs count to a stop).
            let start = col;
            let mut end = col;
            let mut j = i;
            while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
                end = if chars[j] == '\t' {
                    next_tab_stop(end, tw)
                } else {
                    end + 1
                };
                j += 1;
            }
            if leading_only && seen_nonblank {
                // Past the indentation in leading-only mode: keep the run verbatim.
                out.extend(&chars[i..j]);
            } else {
                retabify(&mut out, start, end, tw);
            }
            col = end;
            i = j;
        } else {
            out.push(c);
            col += 1;
            seen_nonblank = true;
            i += 1;
        }
    }
    out
}

/// Emit the shortest tab+space sequence spanning columns `start..end`. A tab is
/// only used when it advances to a stop AND replaces at least two columns, so a
/// single space (or a lone space before a stop) is never turned into a tab.
fn retabify(out: &mut String, start: usize, end: usize, tw: usize) {
    let mut c = start;
    loop {
        let stop = next_tab_stop(c, tw);
        if stop <= end && stop - c >= 2 {
            out.push('\t');
            c = stop;
        } else {
            break;
        }
    }
    for _ in c..end {
        out.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_leading_tab_respects_stop() {
        // Tab at column 0, width 4 → 4 spaces.
        assert_eq!(convert("\tcode", "expand", 4, "all").unwrap(), "    code");
        // Tab after "ab" (column 2) advances only to column 4 → 2 spaces.
        assert_eq!(convert("ab\tcd", "expand", 4, "all").unwrap(), "ab  cd");
    }

    #[test]
    fn expand_default_direction_is_expand() {
        assert_eq!(convert("\tx", "", 4, "").unwrap(), "    x");
    }

    #[test]
    fn expand_width_eight() {
        assert_eq!(convert("\tx", "expand", 8, "all").unwrap(), "        x");
    }

    #[test]
    fn expand_leading_only_keeps_inline_tabs() {
        // Leading tab expands; the inline tab after "a" stays a literal tab.
        assert_eq!(
            convert("\ta\tb", "expand", 4, "leading").unwrap(),
            "    a\tb"
        );
    }

    #[test]
    fn expand_processes_each_line() {
        assert_eq!(
            convert("\ta\n\tb", "expand", 2, "all").unwrap(),
            "  a\n  b"
        );
    }

    #[test]
    fn unexpand_leading_spaces_become_tab() {
        assert_eq!(convert("    code", "unexpand", 4, "all").unwrap(), "\tcode");
    }

    #[test]
    fn unexpand_partial_run_keeps_trailing_spaces() {
        // 6 spaces at width 4 → one tab (to col 4) + 2 spaces.
        assert_eq!(
            convert("      code", "unexpand", 4, "all").unwrap(),
            "\t  code"
        );
    }

    #[test]
    fn unexpand_single_space_not_tabbed() {
        // "abc" then a single space to column 4 must NOT become a tab.
        assert_eq!(convert("abc x", "unexpand", 4, "all").unwrap(), "abc x");
    }

    #[test]
    fn unexpand_leading_only_leaves_inline_spaces() {
        // Only the indentation is re-tabbed; the interior double space stays.
        assert_eq!(
            convert("    a  b", "unexpand", 4, "leading").unwrap(),
            "\ta  b"
        );
    }

    #[test]
    fn expand_then_unexpand_round_trips_indentation() {
        let spaced = convert("\t\tdeep", "expand", 4, "all").unwrap();
        assert_eq!(spaced, "        deep");
        assert_eq!(
            convert(&spaced, "unexpand", 4, "leading").unwrap(),
            "\t\tdeep"
        );
    }

    #[test]
    fn tab_width_is_clamped() {
        // 0 clamps up to 1; oversized clamps down to MAX_TAB_WIDTH.
        assert_eq!(convert("\tx", "expand", 0, "all").unwrap(), " x");
        assert_eq!(
            convert("\tx", "expand", 999, "all").unwrap(),
            format!("{}x", " ".repeat(MAX_TAB_WIDTH as usize))
        );
    }

    #[test]
    fn rejects_unknown_direction() {
        let err = convert("x", "sideways", 4, "all").unwrap_err();
        assert!(err.contains("invalid direction"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_scope() {
        let err = convert("x", "expand", 4, "middle").unwrap_err();
        assert!(err.contains("invalid scope"), "got: {err}");
    }
}
