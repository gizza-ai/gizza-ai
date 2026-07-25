//! gizza-ai/remove-empty-lines core — delete blank or whitespace-only lines,
//! compacting the text. Pure-Rust, dependency-free. Shared by the chat skill
//! block and the web page.
//!
//! Two modes:
//!   * "remove"   — delete EVERY empty line, leaving no gap between content.
//!   * "collapse" — reduce a run of 2+ consecutive empty lines down to a single
//!                  blank line (keeps paragraph separation without big gaps).
//!
//! `whitespace_only` (default true): treat a line that contains only spaces/tabs
//! (or any Unicode whitespace) as empty too, not just literally-empty lines.
//!
//! `trim_lines` (default false): trim leading and trailing whitespace from each
//! kept line.

use serde::Serialize;

/// What to do with empty lines.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Delete every empty line.
    Remove,
    /// Collapse runs of 2+ empty lines to a single blank line.
    Collapse,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "remove" | "remove-all" | "all" | "" => Ok(Mode::Remove),
            "collapse" => Ok(Mode::Collapse),
            other => Err(format!(
                "unknown mode '{other}' (expected 'remove' or 'collapse')"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    /// Total lines in the input (after newline normalization).
    pub total: usize,
    /// Empty lines that were deleted.
    pub removed: usize,
    /// Lines kept in the result.
    pub kept: usize,
    /// The compacted text.
    pub result: String,
}

/// Is `line` considered empty? Literally-empty always counts; whitespace-only
/// counts too when `whitespace_only` is set.
fn is_empty_line(line: &str, whitespace_only: bool) -> bool {
    if whitespace_only {
        line.trim().is_empty()
    } else {
        line.is_empty()
    }
}

/// Normalize CRLF/CR to LF so output is consistent across platforms.
fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Remove (or collapse) empty lines in `text`.
pub fn process(text: &str, mode: Mode, whitespace_only: bool, trim_lines: bool) -> Output {
    let norm = normalize_newlines(text);
    let mut out: Vec<String> = Vec::new();
    let mut total = 0usize;
    let mut removed = 0usize;
    let mut blank_run = 0usize;

    for raw in norm.split('\n') {
        total += 1;
        if is_empty_line(raw, whitespace_only) {
            match mode {
                Mode::Remove => {
                    removed += 1;
                }
                Mode::Collapse => {
                    blank_run += 1;
                    if blank_run > 1 {
                        removed += 1;
                    } else {
                        out.push(String::new());
                    }
                }
            }
        } else {
            blank_run = 0;
            let line = if trim_lines {
                raw.trim().to_string()
            } else {
                raw.to_string()
            };
            out.push(line);
        }
    }

    Output {
        total,
        removed,
        kept: out.len(),
        result: out.join("\n"),
    }
}

/// Human-readable rendering (used by the page) — just the compacted text.
pub fn render(text: &str, mode: Mode, whitespace_only: bool, trim_lines: bool) -> String {
    process(text, mode, whitespace_only, trim_lines).result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_blank_and_whitespace_only_lines() {
        let t = "alpha\n\nbeta\n   \ngamma";
        let o = process(t, Mode::Remove, true, false);
        assert_eq!(o.result, "alpha\nbeta\ngamma");
        assert_eq!(o.total, 5);
        assert_eq!(o.removed, 2);
        assert_eq!(o.kept, 3);
    }

    #[test]
    fn whitespace_only_off_keeps_space_lines() {
        // With whitespace_only=false only literally-empty lines go; "   " stays.
        let t = "a\n\nb\n   \nc";
        let o = process(t, Mode::Remove, false, false);
        assert_eq!(o.result, "a\nb\n   \nc");
        assert_eq!(o.removed, 1);
    }

    #[test]
    fn collapse_reduces_runs_to_single_blank() {
        let t = "one\n\n\n\ntwo\n\nthree";
        let o = process(t, Mode::Collapse, true, false);
        assert_eq!(o.result, "one\n\ntwo\n\nthree");
        // First run had 3 blanks (2 removed), second run had 1 blank (0 removed).
        assert_eq!(o.removed, 2);
    }

    #[test]
    fn trim_lines_strips_edges_of_kept_lines() {
        let t = "  hello  \n\n\tworld\t";
        let o = process(t, Mode::Remove, true, true);
        assert_eq!(o.result, "hello\nworld");
    }

    #[test]
    fn crlf_normalized_and_trailing_blank_removed() {
        let t = "a\r\n\r\nb\r\n";
        let o = process(t, Mode::Remove, true, false);
        assert_eq!(o.result, "a\nb");
    }

    #[test]
    fn unicode_whitespace_line_is_empty() {
        // NBSP (U+00A0) + ideographic space (U+3000) only line.
        let t = "x\n\u{00A0}\u{3000}\ny";
        let o = process(t, Mode::Remove, true, false);
        assert_eq!(o.result, "x\ny");
    }

    #[test]
    fn empty_input() {
        let o = process("", Mode::Remove, true, false);
        assert_eq!(o.result, "");
        assert_eq!(o.kept, 0);
    }

    #[test]
    fn mode_parse_accepts_known_and_rejects_unknown() {
        assert_eq!(Mode::parse("Remove").unwrap(), Mode::Remove);
        assert_eq!(Mode::parse(" collapse ").unwrap(), Mode::Collapse);
        assert_eq!(Mode::parse("").unwrap(), Mode::Remove);
        assert!(Mode::parse("nope").is_err());
    }
}
