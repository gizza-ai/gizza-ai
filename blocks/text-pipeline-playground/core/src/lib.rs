//! text-pipeline-playground core — run pasted text through a chain of text
//! transforms defined by a tiny, safe DSL. No wafer/wasm-bindgen deps; shared by
//! the chat skill block and the web page.
//!
//! This is a *safe, declarative* take on tools like Ultimate Plumber: instead of
//! executing arbitrary Python/shell, the pipeline is a fixed list of pure text
//! operations, one per line. Supported operations:
//!
//! ```text
//! grep PATTERN       keep lines matching PATTERN
//! reject PATTERN     drop lines matching PATTERN (grep -v)
//! replace /old/new/  regex-replace on every line ($1 backrefs allowed)
//! prefix TEXT        prepend TEXT to every line
//! suffix TEXT        append TEXT to every line
//! lower              lowercase every line
//! upper              UPPERCASE every line
//! trim               trim leading/trailing whitespace on every line
//! sort               sort lines ascending (sort -r for descending)
//! unique             drop duplicate lines, keeping first occurrence
//! head N             keep the first N lines
//! tail N             keep the last N lines
//! reverse            reverse the line order
//! split SEP          split every line on SEP into more lines (no SEP: whitespace)
//! join SEP           join all lines into one line separated by SEP
//! ```
//!
//! Blank lines and lines beginning with `#` in the pipeline are ignored.

use std::collections::HashSet;

use regex::{Regex, RegexBuilder};

/// What to do when a pipeline line fails to parse.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OnError {
    /// Abort and return the error (default).
    Stop,
    /// Skip the offending pipeline line and keep going.
    Skip,
}

impl OnError {
    /// Parse an on-error mode (case-insensitive; blank → `Stop`). Unknown → `Err`.
    pub fn parse(s: &str) -> Result<OnError, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "stop" | "abort" | "fail" => Ok(OnError::Stop),
            "skip" | "continue" | "ignore" => Ok(OnError::Skip),
            other => Err(format!(
                "invalid on_error {other:?}: expected 'stop' or 'skip'"
            )),
        }
    }
}

/// Pipeline options that apply across every operation.
#[derive(Clone)]
pub struct Options {
    /// Treat `grep`/`reject` patterns as regular expressions (default: literal substring).
    pub regex_mode: bool,
    /// Match/replace case-insensitively.
    pub case_insensitive: bool,
    /// Hard cap on the number of output lines (safety valve; must be ≥ 1).
    pub limit: usize,
    /// What to do when a pipeline line cannot be parsed.
    pub on_error: OnError,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            regex_mode: false,
            case_insensitive: false,
            limit: 10_000,
            on_error: OnError::Stop,
        }
    }
}

/// How to test a line for grep/reject.
enum LineMatch {
    /// Literal substring test. `needle` is pre-lowercased when `ci` is set.
    Lit { needle: String, ci: bool },
    /// Regular-expression test.
    Re(Regex),
}

impl LineMatch {
    fn is_match(&self, line: &str) -> bool {
        match self {
            LineMatch::Lit { needle, ci } => {
                if *ci {
                    line.to_lowercase().contains(needle)
                } else {
                    line.contains(needle)
                }
            }
            LineMatch::Re(re) => re.is_match(line),
        }
    }
}

/// A single parsed pipeline operation.
enum Op {
    /// Keep (invert=false) or drop (invert=true) lines that match.
    Grep { m: LineMatch, invert: bool },
    /// Regex replace on each line.
    Replace { re: Regex, repl: String },
    Prefix(String),
    Suffix(String),
    Lower,
    Upper,
    Trim,
    Sort { reverse: bool },
    Unique,
    Head(usize),
    Tail(usize),
    Reverse,
    /// `None` → split on any whitespace; `Some(sep)` → split on the literal `sep`.
    Split(Option<String>),
    Join(String),
}

const VALID_OPS: &str =
    "grep, reject, replace, prefix, suffix, lower, upper, trim, sort, unique, head, tail, reverse, split, join";

fn build_match(pat: &str, opts: &Options) -> Result<LineMatch, String> {
    if opts.regex_mode {
        let re = RegexBuilder::new(pat)
            .case_insensitive(opts.case_insensitive)
            .build()
            .map_err(|e| format!("invalid regex {pat:?}: {e}"))?;
        Ok(LineMatch::Re(re))
    } else {
        let needle = if opts.case_insensitive {
            pat.to_lowercase()
        } else {
            pat.to_string()
        };
        Ok(LineMatch::Lit {
            needle,
            ci: opts.case_insensitive,
        })
    }
}

fn parse_replace(rest: &str, opts: &Options) -> Result<Op, String> {
    let r = rest.trim_start();
    let delim = r
        .chars()
        .next()
        .ok_or_else(|| "replace needs a pattern, e.g. replace /old/new/".to_string())?;
    if delim.is_whitespace() {
        return Err(
            "replace needs a delimiter right after the command, e.g. replace /old/new/".to_string(),
        );
    }
    let body = &r[delim.len_utf8()..];
    let mut parts = body.splitn(3, delim);
    let find = parts.next().unwrap_or("");
    let repl = match parts.next() {
        Some(x) => x,
        None => {
            return Err(format!(
                "replace needs the form replace {delim}find{delim}new{delim} (missing the closing {delim:?})"
            ))
        }
    };
    if find.is_empty() {
        return Err("replace: the find pattern is empty".to_string());
    }
    let re = RegexBuilder::new(find)
        .case_insensitive(opts.case_insensitive)
        .build()
        .map_err(|e| format!("replace: invalid regex {find:?}: {e}"))?;
    Ok(Op::Replace {
        re,
        repl: repl.to_string(),
    })
}

/// Parse one pipeline line into an [`Op`]. Returns `Ok(None)` for a comment or
/// blank line, `Err(msg)` for an unrecognized/malformed operation.
fn parse_op(line: &str, opts: &Options) -> Result<Option<Op>, String> {
    let content = line.trim_start_matches([' ', '\t']);
    if content.is_empty() || content.starts_with('#') {
        return Ok(None);
    }
    let (cmd, rest) = match content.split_once(char::is_whitespace) {
        Some((c, r)) => (c, r),
        None => (content, ""),
    };

    let op = match cmd {
        "grep" | "keep" => {
            let pat = rest.trim();
            if pat.is_empty() {
                return Err("grep needs a pattern, e.g. grep ERROR".to_string());
            }
            Op::Grep {
                m: build_match(pat, opts)?,
                invert: false,
            }
        }
        "reject" | "drop" => {
            let pat = rest.trim();
            if pat.is_empty() {
                return Err("reject needs a pattern, e.g. reject DEBUG".to_string());
            }
            Op::Grep {
                m: build_match(pat, opts)?,
                invert: true,
            }
        }
        "replace" | "sub" => parse_replace(rest, opts)?,
        "prefix" => Op::Prefix(rest.to_string()),
        "suffix" => Op::Suffix(rest.to_string()),
        "lower" | "lowercase" => Op::Lower,
        "upper" | "uppercase" => Op::Upper,
        "trim" | "strip" => Op::Trim,
        "sort" => {
            let a = rest.trim();
            let reverse = match a {
                "" | "asc" | "-asc" => false,
                "-r" | "r" | "reverse" | "desc" | "-desc" => true,
                other => {
                    return Err(format!(
                        "sort: unknown option {other:?} (use 'sort' or 'sort -r')"
                    ))
                }
            };
            Op::Sort { reverse }
        }
        "unique" | "uniq" | "dedupe" => Op::Unique,
        "head" | "take" => Op::Head(parse_count(cmd, rest)?),
        "tail" => Op::Tail(parse_count(cmd, rest)?),
        "reverse" | "rev" => Op::Reverse,
        "split" => {
            let sep = rest.trim();
            if sep.is_empty() {
                Op::Split(None)
            } else {
                Op::Split(Some(sep.to_string()))
            }
        }
        "join" => Op::Join(rest.to_string()),
        other => {
            return Err(format!("unknown operation {other:?} (valid: {VALID_OPS})"))
        }
    };
    Ok(Some(op))
}

fn parse_count(cmd: &str, rest: &str) -> Result<usize, String> {
    let n = rest.trim();
    n.parse::<usize>()
        .map_err(|_| format!("{cmd} expected a line count, e.g. {cmd} 10 (got {n:?})"))
}

fn apply(op: &Op, lines: Vec<String>) -> Vec<String> {
    match op {
        Op::Grep { m, invert } => lines
            .into_iter()
            .filter(|l| m.is_match(l) != *invert)
            .collect(),
        Op::Replace { re, repl } => lines
            .into_iter()
            .map(|l| re.replace_all(&l, repl.as_str()).into_owned())
            .collect(),
        Op::Prefix(p) => lines.into_iter().map(|l| format!("{p}{l}")).collect(),
        Op::Suffix(s) => lines.into_iter().map(|l| format!("{l}{s}")).collect(),
        Op::Lower => lines.into_iter().map(|l| l.to_lowercase()).collect(),
        Op::Upper => lines.into_iter().map(|l| l.to_uppercase()).collect(),
        Op::Trim => lines.into_iter().map(|l| l.trim().to_string()).collect(),
        Op::Sort { reverse } => {
            let mut v = lines;
            v.sort();
            if *reverse {
                v.reverse();
            }
            v
        }
        Op::Unique => {
            let mut seen = HashSet::new();
            lines.into_iter().filter(|l| seen.insert(l.clone())).collect()
        }
        Op::Head(n) => lines.into_iter().take(*n).collect(),
        Op::Tail(n) => {
            let skip = lines.len().saturating_sub(*n);
            lines.into_iter().skip(skip).collect()
        }
        Op::Reverse => {
            let mut v = lines;
            v.reverse();
            v
        }
        Op::Split(sep) => {
            let mut out = Vec::new();
            for l in lines {
                match sep {
                    None => out.extend(l.split_whitespace().map(|s| s.to_string())),
                    Some(sep) => out.extend(l.split(sep.as_str()).map(|s| s.to_string())),
                }
            }
            out
        }
        Op::Join(sep) => vec![lines.join(sep)],
    }
}

/// Run `text` through the `pipeline` (one operation per line) under `opts`.
/// Returns the transformed text, or a helpful error naming the offending line.
pub fn run(text: &str, pipeline: &str, opts: &Options) -> Result<String, String> {
    // Parse every pipeline line first so errors surface before any work.
    let mut ops: Vec<Op> = Vec::new();
    for (i, raw) in pipeline.lines().enumerate() {
        match parse_op(raw, opts) {
            Ok(Some(op)) => ops.push(op),
            Ok(None) => {}
            Err(msg) => {
                if opts.on_error == OnError::Stop {
                    return Err(format!("pipeline line {}: {msg}", i + 1));
                }
                // Skip: ignore this line and continue.
            }
        }
    }

    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    for op in &ops {
        lines = apply(op, lines);
    }

    if lines.len() > opts.limit {
        lines.truncate(opts.limit);
    }

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    const LOG: &str = "\
2024-01-01 INFO started
2024-01-01 ERROR disk full
2024-01-02 ERROR disk full
2024-01-02 WARN low memory
2024-01-03 ERROR timeout";

    #[test]
    fn worked_example_grep_replace_sort_unique() {
        let pipeline = "grep ERROR\nreplace /^\\S+ ERROR /!! /\nsort\nunique";
        assert_eq!(run(LOG, pipeline, &opts()).unwrap(), "!! disk full\n!! timeout");
    }

    #[test]
    fn grep_and_reject_are_substring_by_default() {
        assert_eq!(
            run(LOG, "grep WARN", &opts()).unwrap(),
            "2024-01-02 WARN low memory"
        );
        assert_eq!(
            run("keep\ndrop\nkeep", "reject drop", &opts()).unwrap(),
            "keep\nkeep"
        );
    }

    #[test]
    fn regex_mode_and_case_insensitive() {
        let o = Options {
            regex_mode: true,
            ..opts()
        };
        assert_eq!(run(LOG, "grep ^2024-01-0[13]", &o).unwrap().lines().count(), 3);

        let ci = Options {
            case_insensitive: true,
            ..opts()
        };
        assert_eq!(
            run("Apple\nBANANA\ncherry", "grep a", &ci).unwrap(),
            "Apple\nBANANA"
        );
    }

    #[test]
    fn replace_supports_backrefs_and_alt_delimiters() {
        assert_eq!(
            run("a=1\nb=2", "replace /(\\w+)=(\\d+)/$2:$1/", &opts()).unwrap(),
            "1:a\n2:b"
        );
        // A non-slash delimiter so the pattern can contain slashes.
        assert_eq!(run("a/b/c", "replace |/|-|", &opts()).unwrap(), "a-b-c");
    }

    #[test]
    fn prefix_suffix_preserve_spacing() {
        assert_eq!(
            run("x\ny", "prefix > \nsuffix  !", &opts()).unwrap(),
            "> x !\n> y !"
        );
    }

    #[test]
    fn case_and_trim() {
        assert_eq!(run("  Hi There  ", "trim\nlower", &opts()).unwrap(), "hi there");
        assert_eq!(run("abc", "upper", &opts()).unwrap(), "ABC");
    }

    #[test]
    fn sort_ascending_and_descending() {
        assert_eq!(run("b\na\nc", "sort", &opts()).unwrap(), "a\nb\nc");
        assert_eq!(run("b\na\nc", "sort -r", &opts()).unwrap(), "c\nb\na");
    }

    #[test]
    fn head_tail_reverse() {
        assert_eq!(run("1\n2\n3\n4", "head 2", &opts()).unwrap(), "1\n2");
        assert_eq!(run("1\n2\n3\n4", "tail 2", &opts()).unwrap(), "3\n4");
        assert_eq!(run("1\n2\n3", "reverse", &opts()).unwrap(), "3\n2\n1");
        // tail asking for more than exists just returns all.
        assert_eq!(run("1\n2", "tail 9", &opts()).unwrap(), "1\n2");
    }

    #[test]
    fn split_and_join() {
        assert_eq!(run("a,b,c", "split ,", &opts()).unwrap(), "a\nb\nc");
        assert_eq!(run("a b\tc", "split", &opts()).unwrap(), "a\nb\nc");
        assert_eq!(run("a\nb\nc", "join , ", &opts()).unwrap(), "a, b, c");
    }

    #[test]
    fn comments_and_blank_lines_ignored() {
        let pipeline = "# just keep errors\n\ngrep ERROR\n  # trailing comment\nhead 1";
        assert_eq!(run(LOG, pipeline, &opts()).unwrap(), "2024-01-01 ERROR disk full");
    }

    #[test]
    fn empty_pipeline_is_identity() {
        assert_eq!(run("a\nb", "", &opts()).unwrap(), "a\nb");
        assert_eq!(run("", "grep x", &opts()).unwrap(), "");
    }

    #[test]
    fn limit_caps_output_lines() {
        let o = Options { limit: 2, ..opts() };
        assert_eq!(run("1\n2\n3\n4", "sort", &o).unwrap(), "1\n2");
    }

    #[test]
    fn error_unknown_op_reports_line_number() {
        let err = run(LOG, "grep ERROR\ngrpe X", &opts()).unwrap_err();
        assert!(err.contains("pipeline line 2"), "{err}");
        assert!(err.contains("unknown operation"), "{err}");
    }

    #[test]
    fn error_bad_head_count() {
        let err = run("a", "head two", &opts()).unwrap_err();
        assert!(err.contains("expected a line count"), "{err}");
    }

    #[test]
    fn error_bad_replace_form_and_regex() {
        let err = run("a", "replace /only-one", &opts()).unwrap_err();
        assert!(err.contains("closing"), "{err}");
        let err2 = run("a", "replace /(/x/", &opts()).unwrap_err();
        assert!(err2.contains("invalid regex"), "{err2}");
    }

    #[test]
    fn on_error_skip_continues_past_bad_line() {
        let o = Options {
            on_error: OnError::Skip,
            ..opts()
        };
        // The bogus middle line is skipped; grep + head still run.
        let pipeline = "grep ERROR\nbogus-op\nhead 1";
        assert_eq!(run(LOG, pipeline, &o).unwrap(), "2024-01-01 ERROR disk full");
    }

    #[test]
    fn on_error_parse_and_aliases() {
        assert!(OnError::parse("SKIP").is_ok());
        assert!(OnError::parse("").is_ok());
        assert!(OnError::parse("nope").is_err());
    }
}
