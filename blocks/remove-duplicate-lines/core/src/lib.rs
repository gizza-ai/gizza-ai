//! gizza-ai/remove-duplicate-lines core — remove duplicate lines from text,
//! returning the deduplicated text plus a small summary. Pure-Rust,
//! dependency-free (besides serde). Shared by the chat skill block and the page.

use std::collections::HashSet;

use serde::Serialize;

/// Which occurrence of a duplicated line to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    /// Keep the first occurrence, drop later ones (default).
    First,
    /// Keep the last occurrence, drop earlier ones.
    Last,
}

impl Keep {
    pub fn parse(s: &str) -> Result<Keep, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "first" | "" => Ok(Keep::First),
            "last" => Ok(Keep::Last),
            other => Err(format!("keep must be \"first\" or \"last\", got \"{other}\"")),
        }
    }
}

/// Options controlling how duplicates are detected and removed.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Compare lines case-insensitively.
    pub ignore_case: bool,
    /// Trim leading/trailing whitespace before comparing (and from the kept output).
    pub trim: bool,
    /// Only collapse runs of *consecutive* identical lines (like `uniq`); a line
    /// that reappears later, separated by other lines, is kept.
    pub adjacent_only: bool,
    /// Which occurrence of a duplicated line to keep.
    pub keep: Keep,
    /// Drop empty (or whitespace-only, when `trim`) lines entirely.
    pub remove_empty: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            ignore_case: false,
            trim: false,
            adjacent_only: false,
            keep: Keep::First,
            remove_empty: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    /// The deduplicated text.
    pub text: String,
    /// Number of input lines.
    pub total_lines: usize,
    /// Number of lines kept in the output.
    pub kept_lines: usize,
    /// Number of lines removed (duplicates + any dropped empties).
    pub removed_lines: usize,
}

fn normalize_key(line: &str, opts: &Options) -> String {
    let s = if opts.trim { line.trim() } else { line };
    if opts.ignore_case {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

/// Render a kept line into its output form (trimmed if requested).
fn render_line(line: &str, opts: &Options) -> String {
    if opts.trim {
        line.trim().to_string()
    } else {
        line.to_string()
    }
}

fn is_empty(line: &str, opts: &Options) -> bool {
    if opts.trim {
        line.trim().is_empty()
    } else {
        line.is_empty()
    }
}

/// Remove duplicate lines from `text` according to `opts`.
///
/// Line endings are normalized to `\n`. A trailing newline in the output is
/// preserved iff the input ended with a newline and at least one line is kept.
pub fn dedupe(text: &str, opts: &Options) -> Output {
    let ends_with_newline = text.ends_with('\n');
    let lines: Vec<&str> = text.lines().collect();
    let total_lines = lines.len();

    // Pre-filter empties (count them as removed) so they never participate in
    // duplicate detection or output.
    let mut work: Vec<&str> = Vec::with_capacity(lines.len());
    let mut removed_empty = 0usize;
    for &l in &lines {
        if opts.remove_empty && is_empty(l, opts) {
            removed_empty += 1;
        } else {
            work.push(l);
        }
    }

    let kept: Vec<&str> = if opts.adjacent_only {
        // Collapse consecutive runs; keep first/last of each run.
        let mut out: Vec<&str> = Vec::new();
        let mut prev_key: Option<String> = None;
        for &l in &work {
            let key = normalize_key(l, opts);
            if prev_key.as_deref() == Some(key.as_str()) {
                // same run as previous
                if opts.keep == Keep::Last {
                    // replace the run's representative with this (later) line
                    *out.last_mut().unwrap() = l;
                }
            } else {
                out.push(l);
                prev_key = Some(key);
            }
        }
        out
    } else if opts.keep == Keep::Last {
        // Keep the last occurrence: walk from the end keeping first-seen keys,
        // then reverse to restore order. Output position is the LAST occurrence.
        let mut seen: HashSet<String> = HashSet::new();
        let mut out: Vec<&str> = Vec::new();
        for &l in work.iter().rev() {
            let key = normalize_key(l, opts);
            if seen.insert(key) {
                out.push(l);
            }
        }
        out.reverse();
        out
    } else {
        // Keep the first occurrence (global, order-preserving).
        let mut seen: HashSet<String> = HashSet::new();
        let mut out: Vec<&str> = Vec::new();
        for &l in &work {
            let key = normalize_key(l, opts);
            if seen.insert(key) {
                out.push(l);
            }
        }
        out
    };

    let kept_lines = kept.len();
    let rendered: Vec<String> = kept.iter().map(|&l| render_line(l, opts)).collect();
    let mut text_out = rendered.join("\n");
    if ends_with_newline && !text_out.is_empty() {
        text_out.push('\n');
    }

    let removed_lines = total_lines.saturating_sub(kept_lines);
    let _ = removed_empty; // accounted for in removed_lines via total - kept

    Output { text: text_out, total_lines, kept_lines, removed_lines }
}

/// Human-readable rendering for the page: the deduplicated text, with a summary
/// comment line is intentionally omitted so the output is paste-ready.
pub fn render(text: &str, opts: &Options) -> Result<String, String> {
    Ok(dedupe(text, opts).text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn basic_keep_first() {
        let o = dedupe("a\nb\na\nc\nb", &opts());
        assert_eq!(o.text, "a\nb\nc");
        assert_eq!(o.total_lines, 5);
        assert_eq!(o.kept_lines, 3);
        assert_eq!(o.removed_lines, 2);
    }

    #[test]
    fn keep_last() {
        let mut o = opts();
        o.keep = Keep::Last;
        // last occurrence of each kept, preserving the order of last occurrences
        let r = dedupe("a\nb\na\nc\nb", &o);
        assert_eq!(r.text, "a\nc\nb");
    }

    #[test]
    fn preserves_trailing_newline() {
        let o = dedupe("a\na\nb\n", &opts());
        assert_eq!(o.text, "a\nb\n");
    }

    #[test]
    fn no_trailing_newline_when_input_had_none() {
        let o = dedupe("a\na\nb", &opts());
        assert_eq!(o.text, "a\nb");
    }

    #[test]
    fn ignore_case() {
        let mut o = opts();
        o.ignore_case = true;
        let r = dedupe("Foo\nfoo\nFOO\nbar", &o);
        // first-seen form ("Foo") kept
        assert_eq!(r.text, "Foo\nbar");
    }

    #[test]
    fn case_sensitive_by_default() {
        let r = dedupe("Foo\nfoo", &opts());
        assert_eq!(r.text, "Foo\nfoo");
    }

    #[test]
    fn trim_whitespace() {
        let mut o = opts();
        o.trim = true;
        let r = dedupe("ab\n  ab  \nab", &o);
        // all collapse; kept form is trimmed
        assert_eq!(r.text, "ab");
        assert_eq!(r.kept_lines, 1);
    }

    #[test]
    fn adjacent_only_keeps_non_consecutive() {
        let mut o = opts();
        o.adjacent_only = true;
        // a a b a -> a b a (the last 'a' is not consecutive with the run)
        let r = dedupe("a\na\nb\na", &o);
        assert_eq!(r.text, "a\nb\na");
    }

    #[test]
    fn adjacent_only_keep_last_of_run() {
        let mut o = opts();
        o.adjacent_only = true;
        o.keep = Keep::Last;
        o.trim = true;
        // run of "x" with different surrounding whitespace; keep last form
        let r = dedupe("x\n x \nx  \ny", &o);
        assert_eq!(r.text, "x\ny");
    }

    #[test]
    fn remove_empty_lines() {
        let mut o = opts();
        o.remove_empty = true;
        let r = dedupe("a\n\nb\n\n\na", &o);
        // empties dropped, then 'a' deduped
        assert_eq!(r.text, "a\nb");
        assert_eq!(r.total_lines, 6);
        assert_eq!(r.kept_lines, 2);
        assert_eq!(r.removed_lines, 4);
    }

    #[test]
    fn empty_input() {
        let r = dedupe("", &opts());
        assert_eq!(r.text, "");
        assert_eq!(r.total_lines, 0);
        assert_eq!(r.kept_lines, 0);
        assert_eq!(r.removed_lines, 0);
    }

    #[test]
    fn all_unique_unchanged() {
        let r = dedupe("x\ny\nz", &opts());
        assert_eq!(r.text, "x\ny\nz");
        assert_eq!(r.removed_lines, 0);
    }

    #[test]
    fn crlf_normalized() {
        // lines() splits on \n and strips a trailing \r, so CRLF input
        // dedupes correctly and emits LF.
        let r = dedupe("a\r\na\r\nb\r\n", &opts());
        assert_eq!(r.text, "a\nb\n");
    }

    #[test]
    fn keep_parse() {
        assert_eq!(Keep::parse("first").unwrap(), Keep::First);
        assert_eq!(Keep::parse("LAST").unwrap(), Keep::Last);
        assert_eq!(Keep::parse("").unwrap(), Keep::First);
        assert!(Keep::parse("middle").is_err());
    }
}
