//! gizza-ai/text-diff core — line-level diff of two blocks of text.
//!
//! Computes a longest-common-subsequence (LCS) line diff between a `left` (old)
//! and `right` (new) text and reports which lines were added, removed, changed
//! (a removed line replaced by an added one) or left unchanged. Two output
//! renderers are provided: a classic `unified` diff (`@@` hunks with `+`/`-`/` `
//! prefixes, ideal for highlighting) and a structured `json` report. Pure-Rust,
//! no I/O — shared by the chat skill block and the web page.

use serde::Serialize;
use serde_json::json;

/// Per-line edit operation in the raw (un-paired) diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tag {
    Equal,
    Delete,
    Insert,
}

/// One line of the raw diff with its 1-based numbers in each side.
#[derive(Debug, Clone)]
struct DiffLine {
    tag: Tag,
    left_no: Option<usize>,
    right_no: Option<usize>,
    text: String,
}

/// Comparison options that affect line MATCHING only — the rendered/echoed text
/// is always the original, unmodified line.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub ignore_case: bool,
    pub ignore_whitespace: bool,
    /// Lines of unchanged context to keep around each hunk in the unified diff.
    pub context: usize,
}

/// Split text into lines, preserving a final line even without a trailing
/// newline. An empty string is treated as a single empty line so that adding or
/// removing content against "" diffs sensibly. A trailing `\n` does not produce
/// a spurious empty final line; `\r` is stripped at render time via `trim_cr`.
fn split_lines(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return vec![""];
    }
    let mut out: Vec<&str> = s.split('\n').collect();
    // A trailing '\n' yields a spurious empty final element ("a\n" -> ["a", ""]).
    // Drop it so "a\n" is a single line "a".
    if out.len() > 1 && out.last() == Some(&"") {
        out.pop();
    }
    out
}

fn trim_cr(s: &str) -> &str {
    s.strip_suffix('\r').unwrap_or(s)
}

/// Build the comparison key for a line under the active options.
fn key(line: &str, opt: &Options) -> String {
    let line = trim_cr(line);
    let mut s = if opt.ignore_whitespace {
        line.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        line.to_string()
    };
    if opt.ignore_case {
        s = s.to_lowercase();
    }
    s
}

/// Classic LCS-length DP over the two key vectors, returning the raw diff lines
/// in original order.
fn diff_lines(left: &str, right: &str, opt: &Options) -> Vec<DiffLine> {
    let a: Vec<&str> = split_lines(left);
    let b: Vec<&str> = split_lines(right);
    let ak: Vec<String> = a.iter().map(|l| key(l, opt)).collect();
    let bk: Vec<String> = b.iter().map(|l| key(l, opt)).collect();
    let (n, m) = (a.len(), b.len());

    // dp[i][j] = LCS length of a[i..] and b[j..].
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if ak[i] == bk[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if ak[i] == bk[j] {
            out.push(DiffLine {
                tag: Tag::Equal,
                left_no: Some(i + 1),
                right_no: Some(j + 1),
                text: trim_cr(a[i]).to_string(),
            });
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            out.push(DiffLine {
                tag: Tag::Delete,
                left_no: Some(i + 1),
                right_no: None,
                text: trim_cr(a[i]).to_string(),
            });
            i += 1;
        } else {
            out.push(DiffLine {
                tag: Tag::Insert,
                left_no: None,
                right_no: Some(j + 1),
                text: trim_cr(b[j]).to_string(),
            });
            j += 1;
        }
    }
    while i < n {
        out.push(DiffLine {
            tag: Tag::Delete,
            left_no: Some(i + 1),
            right_no: None,
            text: trim_cr(a[i]).to_string(),
        });
        i += 1;
    }
    while j < m {
        out.push(DiffLine {
            tag: Tag::Insert,
            left_no: None,
            right_no: Some(j + 1),
            text: trim_cr(b[j]).to_string(),
        });
        j += 1;
    }
    out
}

/// Summary statistics for the diff.
#[derive(Debug, Clone, Serialize, PartialEq)]
struct Stats {
    equal: bool,
    added: usize,
    removed: usize,
    changed: usize,
    unchanged: usize,
}

/// Count added/removed/changed lines. A "changed" pair is one removed line
/// directly replaced by one added line within the same contiguous edit block;
/// `min(deletes, inserts)` of each block are reported as changes and the
/// remainder as pure additions/removals.
fn stats(lines: &[DiffLine]) -> Stats {
    let (mut added, mut removed, mut changed, mut unchanged) = (0, 0, 0, 0);
    let mut idx = 0;
    while idx < lines.len() {
        match lines[idx].tag {
            Tag::Equal => {
                unchanged += 1;
                idx += 1;
            }
            _ => {
                // Span the contiguous non-Equal block.
                let start = idx;
                while idx < lines.len() && lines[idx].tag != Tag::Equal {
                    idx += 1;
                }
                let block = &lines[start..idx];
                let dels = block.iter().filter(|l| l.tag == Tag::Delete).count();
                let ins = block.iter().filter(|l| l.tag == Tag::Insert).count();
                let pairs = dels.min(ins);
                changed += pairs;
                removed += dels - pairs;
                added += ins - pairs;
            }
        }
    }
    Stats {
        equal: added == 0 && removed == 0 && changed == 0,
        added,
        removed,
        changed,
        unchanged,
    }
}

/// Render a classic unified diff (`@@ -ls,ln +rs,rn @@`, with ` `/`-`/`+` line
/// prefixes). Returns `No differences.` when the inputs match.
fn render_unified(lines: &[DiffLine], opt: &Options) -> String {
    let st = stats(lines);
    if st.equal {
        return "No differences.".to_string();
    }
    let ctx = opt.context;
    let n = lines.len();

    // Mark which lines are changes; expand by `ctx` and merge into hunk ranges.
    let mut keep = vec![false; n];
    for (k, l) in lines.iter().enumerate() {
        if l.tag != Tag::Equal {
            let lo = k.saturating_sub(ctx);
            let hi = (k + ctx + 1).min(n);
            for slot in keep.iter_mut().take(hi).skip(lo) {
                *slot = true;
            }
        }
    }

    let mut out = String::new();
    out.push_str("--- left\n+++ right\n");
    let mut k = 0;
    while k < n {
        if !keep[k] {
            k += 1;
            continue;
        }
        let start = k;
        while k < n && keep[k] {
            k += 1;
        }
        let hunk = &lines[start..k];

        // Hunk header line ranges (1-based start, count).
        let l_start = hunk
            .iter()
            .find_map(|x| x.left_no)
            .or_else(|| {
                // A pure-insert hunk: anchor on the preceding left line number.
                lines[..start]
                    .iter()
                    .rev()
                    .find_map(|x| x.left_no)
                    .map(|v| v + 1)
            })
            .unwrap_or(0);
        let r_start = hunk
            .iter()
            .find_map(|x| x.right_no)
            .or_else(|| {
                lines[..start]
                    .iter()
                    .rev()
                    .find_map(|x| x.right_no)
                    .map(|v| v + 1)
            })
            .unwrap_or(0);
        let l_count = hunk.iter().filter(|x| x.left_no.is_some()).count();
        let r_count = hunk.iter().filter(|x| x.right_no.is_some()).count();

        out.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            if l_count == 0 {
                l_start.saturating_sub(1)
            } else {
                l_start
            },
            l_count,
            if r_count == 0 {
                r_start.saturating_sub(1)
            } else {
                r_start
            },
            r_count
        ));
        for l in hunk {
            let prefix = match l.tag {
                Tag::Equal => ' ',
                Tag::Delete => '-',
                Tag::Insert => '+',
            };
            out.push(prefix);
            out.push_str(&l.text);
            out.push('\n');
        }
    }
    // Drop the trailing newline for a clean single string.
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Render the structured JSON report. Contiguous delete+insert blocks are paired
/// into `changed` rows so each modified line shows its old and new text.
fn render_json(lines: &[DiffLine], indent: usize) -> String {
    let st = stats(lines);
    let mut rows: Vec<serde_json::Value> = Vec::new();
    let mut idx = 0;
    while idx < lines.len() {
        match lines[idx].tag {
            Tag::Equal => {
                let l = &lines[idx];
                rows.push(json!({
                    "op": "equal",
                    "left_line": l.left_no,
                    "right_line": l.right_no,
                    "text": l.text,
                }));
                idx += 1;
            }
            _ => {
                let start = idx;
                while idx < lines.len() && lines[idx].tag != Tag::Equal {
                    idx += 1;
                }
                let block = &lines[start..idx];
                let dels: Vec<&DiffLine> = block.iter().filter(|l| l.tag == Tag::Delete).collect();
                let ins: Vec<&DiffLine> = block.iter().filter(|l| l.tag == Tag::Insert).collect();
                let pairs = dels.len().min(ins.len());
                for p in 0..pairs {
                    rows.push(json!({
                        "op": "changed",
                        "left_line": dels[p].left_no,
                        "right_line": ins[p].right_no,
                        "old": dels[p].text,
                        "new": ins[p].text,
                    }));
                }
                for d in dels.iter().skip(pairs) {
                    rows.push(json!({
                        "op": "removed",
                        "left_line": d.left_no,
                        "text": d.text,
                    }));
                }
                for a in ins.iter().skip(pairs) {
                    rows.push(json!({
                        "op": "added",
                        "right_line": a.right_no,
                        "text": a.text,
                    }));
                }
            }
        }
    }
    let report = json!({
        "equal": st.equal,
        "added": st.added,
        "removed": st.removed,
        "changed": st.changed,
        "unchanged": st.unchanged,
        "lines": rows,
    });
    if indent == 0 {
        serde_json::to_string(&report).unwrap()
    } else {
        let mut buf = Vec::new();
        let pad = " ".repeat(indent).into_bytes();
        let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
        serde::Serialize::serialize(&report, &mut ser).unwrap();
        String::from_utf8(buf).unwrap()
    }
}

/// Public entry: diff `left` vs `right` and render as `format` ("unified" or
/// "json"). `indent` applies only to the json renderer (0 minifies). Errors on
/// an unknown format.
pub fn diff_text(
    left: &str,
    right: &str,
    format: &str,
    opt: &Options,
    indent: usize,
) -> Result<String, String> {
    let lines = diff_lines(left, right, opt);
    match format {
        "unified" => Ok(render_unified(&lines, opt)),
        "json" => Ok(render_json(&lines, indent)),
        other => Err(format!(
            "unknown format '{other}' — expected 'unified' or 'json'"
        )),
    }
}

/// Convenience wrapper used by the chat block, the CLI, and the web page: build
/// [`Options`] from the individual flags and render `format` ("unified" or
/// "json"). The JSON renderer is pretty-printed with a 2-space indent; the
/// unified renderer ignores `indent`. Errors on an unknown format (forwarded
/// from [`diff_text`]).
pub fn run(
    left: &str,
    right: &str,
    format: &str,
    ignore_case: bool,
    ignore_whitespace: bool,
    context: usize,
) -> Result<String, String> {
    let opt = Options {
        ignore_case,
        ignore_whitespace,
        context,
    };
    let indent = if format == "json" { 2 } else { 0 };
    diff_text(left, right, format, &opt, indent)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opt(ctx: usize) -> Options {
        Options {
            context: ctx,
            ..Default::default()
        }
    }

    #[test]
    fn unified_change_add_remove() {
        let left = "alpha\nbeta\ngamma\n";
        let right = "alpha\nBETA\ngamma\ndelta\n";
        let out = diff_text(left, right, "unified", &opt(1), 0).unwrap();
        assert!(out.contains("-beta"), "{out}");
        assert!(out.contains("+BETA"), "{out}");
        assert!(out.contains("+delta"), "{out}");
        assert!(out.starts_with("--- left\n+++ right\n@@"), "{out}");
    }

    #[test]
    fn equal_inputs_report_no_diff() {
        let out = diff_text("a\nb\n", "a\nb\n", "unified", &opt(3), 0).unwrap();
        assert_eq!(out, "No differences.");
    }

    #[test]
    fn json_pairs_changed_lines() {
        let left = "one\ntwo\nthree";
        let right = "one\nTWO\nthree";
        let out = diff_text(left, right, "json", &opt(3), 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["changed"], 1);
        assert_eq!(v["added"], 0);
        assert_eq!(v["removed"], 0);
        assert_eq!(v["unchanged"], 2);
        assert_eq!(v["equal"], false);
        // The middle row is a changed pair carrying old + new.
        assert_eq!(v["lines"][1]["op"], "changed");
        assert_eq!(v["lines"][1]["old"], "two");
        assert_eq!(v["lines"][1]["new"], "TWO");
    }

    #[test]
    fn json_counts_pure_add_and_remove() {
        let left = "a\nb\nc";
        let right = "a\nc\nd";
        // b removed; d added; a,c unchanged.
        let out = diff_text(left, right, "json", &opt(3), 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["removed"], 1);
        assert_eq!(v["added"], 1);
        assert_eq!(v["changed"], 0);
        assert_eq!(v["unchanged"], 2);
    }

    #[test]
    fn ignore_case_and_whitespace() {
        let left = "Hello   World";
        let right = "hello world";
        let mut o = Options {
            context: 3,
            ..Default::default()
        };
        // Without the flags they differ.
        let plain = diff_text(left, right, "json", &o, 0).unwrap();
        let pv: serde_json::Value = serde_json::from_str(&plain).unwrap();
        assert_eq!(pv["equal"], false);
        // With both flags they match.
        o.ignore_case = true;
        o.ignore_whitespace = true;
        let folded = diff_text(left, right, "json", &o, 0).unwrap();
        let fv: serde_json::Value = serde_json::from_str(&folded).unwrap();
        assert_eq!(fv["equal"], true);
    }

    #[test]
    fn crlf_lf_not_a_difference() {
        let out = diff_text("a\r\nb\r\n", "a\nb\n", "unified", &opt(3), 0).unwrap();
        assert_eq!(out, "No differences.");
    }

    #[test]
    fn unknown_format_errors() {
        let e = diff_text("a", "b", "side-by-side", &opt(3), 0).unwrap_err();
        assert!(e.contains("unknown format"), "{e}");
    }

    #[test]
    fn pretty_json_indents() {
        let out = diff_text("a", "b", "json", &opt(3), 2).unwrap();
        assert!(out.contains("\n  \"equal\""), "{out}");
    }

    #[test]
    fn run_wrapper_builds_options_and_pretty_json() {
        // Default flags: case- and whitespace-sensitive, 3 lines of context.
        let unified = run("alpha\nbeta", "alpha\nBETA", "unified", false, false, 3).unwrap();
        assert!(unified.contains("-beta") && unified.contains("+BETA"), "{unified}");
        // json is pretty-printed by the wrapper (2-space indent).
        let pretty = run("a", "b", "json", false, false, 3).unwrap();
        assert!(pretty.contains("\n  \"equal\""), "{pretty}");
        // Folding flags collapse a case/space-only change to "no differences".
        let folded = run("Hello   World", "hello world", "unified", true, true, 3).unwrap();
        assert_eq!(folded, "No differences.");
        // Unknown format still errors through the wrapper.
        assert!(run("a", "b", "xml", false, false, 3).is_err());
    }
}
