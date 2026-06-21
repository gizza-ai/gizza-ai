//! gizza-ai/remove-whitespace core — clean up whitespace in text. Pure-Rust,
//! dependency-free. Three modes:
//!   * "trim"     — strip leading/trailing whitespace from each line (and drop
//!                  leading/trailing blank lines on the whole text); structure
//!                  and internal spacing preserved.
//!   * "collapse" — trim each line AND collapse runs of inline whitespace to a
//!                  single space; optionally collapse blank-line runs.
//!   * "strip"    — remove ALL whitespace characters, producing one dense string.
//!
//! "Whitespace" follows Rust's `char::is_whitespace` (covers Unicode spaces such
//! as NBSP U+00A0 and the ideographic space U+3000), so it cleans more than ASCII.

/// What to do with whitespace.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Trim each line; keep internal spacing and line structure.
    Trim,
    /// Trim each line and collapse internal whitespace runs to one space.
    Collapse,
    /// Remove every whitespace character.
    Strip,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "trim" => Ok(Mode::Trim),
            "collapse" => Ok(Mode::Collapse),
            "strip" => Ok(Mode::Strip),
            other => Err(format!(
                "unknown mode '{other}' (expected 'trim', 'collapse', or 'strip')"
            )),
        }
    }
}

/// Normalize line endings to `\n` so output is consistent across platforms.
fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Collapse runs of inline whitespace to a single ' '.
fn collapse_inline(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_ws = false;
    for c in line.chars() {
        if c.is_whitespace() {
            in_ws = true;
        } else {
            if in_ws && !out.is_empty() {
                out.push(' ');
            }
            in_ws = false;
            out.push(c);
        }
    }
    out
}

/// Clean whitespace in `text`.
///
/// * `mode` selects the strategy.
/// * `collapse_blank_lines` (Trim/Collapse only): collapse runs of 2+ blank
///   lines down to a single blank line.
pub fn clean(text: &str, mode: Mode, collapse_blank_lines: bool) -> String {
    let norm = normalize_newlines(text);

    if mode == Mode::Strip {
        return norm.chars().filter(|c| !c.is_whitespace()).collect();
    }

    let mut out: Vec<String> = Vec::new();
    let mut blank_run = 0usize;
    for raw in norm.split('\n') {
        let line = match mode {
            Mode::Trim => raw.trim().to_string(),
            Mode::Collapse => collapse_inline(raw.trim()),
            Mode::Strip => unreachable!(),
        };
        if line.is_empty() {
            blank_run += 1;
            if collapse_blank_lines && blank_run > 1 {
                continue;
            }
            out.push(String::new());
        } else {
            blank_run = 0;
            out.push(line);
        }
    }
    // Drop leading and trailing blank lines.
    while out.first().map(|s| s.is_empty()).unwrap_or(false) {
        out.remove(0);
    }
    while out.last().map(|s| s.is_empty()).unwrap_or(false) {
        out.pop();
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_parse_accepts_known_and_rejects_unknown() {
        assert_eq!(Mode::parse("Trim").unwrap(), Mode::Trim);
        assert_eq!(Mode::parse(" collapse ").unwrap(), Mode::Collapse);
        assert_eq!(Mode::parse("STRIP").unwrap(), Mode::Strip);
        assert!(Mode::parse("nope").is_err());
    }

    #[test]
    fn trim_strips_line_edges_keeps_internal() {
        let t = "  hello   world  \n\tindented line\t";
        assert_eq!(clean(t, Mode::Trim, false), "hello   world\nindented line");
    }

    #[test]
    fn collapse_squashes_internal_runs() {
        let t = "  hello   world  \nfoo\t\tbar";
        assert_eq!(clean(t, Mode::Collapse, false), "hello world\nfoo bar");
    }

    #[test]
    fn strip_removes_everything() {
        let t = "a b\tc\nd  e";
        assert_eq!(clean(t, Mode::Strip, false), "abcde");
        // Unicode NBSP (U+00A0) and ideographic space (U+3000) are removed too.
        assert_eq!(clean("x\u{00A0}y\u{3000}z", Mode::Strip, false), "xyz");
    }

    #[test]
    fn collapse_blank_lines_option() {
        let t = "A\n\n\n\nB";
        assert_eq!(clean(t, Mode::Trim, false), "A\n\n\n\nB");
        assert_eq!(clean(t, Mode::Trim, true), "A\n\nB");
    }

    #[test]
    fn crlf_normalized() {
        assert_eq!(clean("a\r\nb\r\n", Mode::Trim, false), "a\nb");
    }

    #[test]
    fn trailing_and_leading_blank_lines_trimmed() {
        assert_eq!(clean("\n\n  hi  \n\n", Mode::Trim, false), "hi");
    }

    #[test]
    fn collapse_handles_unicode_spaces() {
        // NBSP between words collapses to a single ASCII space.
        assert_eq!(clean("a\u{00A0}\u{00A0}b", Mode::Collapse, false), "a b");
    }

    #[test]
    fn empty_input() {
        assert_eq!(clean("", Mode::Trim, false), "");
        assert_eq!(clean("   ", Mode::Strip, false), "");
        assert_eq!(clean("   ", Mode::Collapse, false), "");
    }
}
