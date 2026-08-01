//! gizza-ai/html-diff core — diff the *visible text* of two HTML snippets while
//! ignoring tag and attribute noise. Pure-Rust, no I/O — shared by the chat
//! skill block and the web page.
//!
//! Each side is first reduced to plain, visible text with `nanohtml2text` (every
//! tag and attribute is dropped; paragraph and list structure becomes newlines),
//! then a longest-common-subsequence (LCS) diff is computed at either `line` or
//! `word` granularity. Two renderers are provided: a classic `unified` diff for
//! lines / a `wdiff`-style inline diff for words, and a structured `json` report.

use serde::Serialize;
use serde_json::json;

/// Diff token granularity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granularity {
    /// One token per line of visible text (classic patch-style diff).
    Line,
    /// One token per whitespace-separated word (prose-style inline diff).
    Word,
}

impl Granularity {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "line" | "lines" => Ok(Granularity::Line),
            "word" | "words" => Ok(Granularity::Word),
            other => Err(format!(
                "unknown granularity '{other}' — expected 'line' or 'word'"
            )),
        }
    }
}

/// Comparison options that affect token MATCHING only — the rendered/echoed text
/// is always the original, unmodified token.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub ignore_case: bool,
    pub ignore_whitespace: bool,
    /// Lines of unchanged context to keep around each hunk (line granularity's
    /// unified renderer only).
    pub context: usize,
}

/// Per-token edit operation in the raw (un-paired) diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tag {
    Equal,
    Delete,
    Insert,
}

/// One token of the raw diff with its 1-based numbers on each side.
#[derive(Debug, Clone)]
struct DiffTok {
    tag: Tag,
    left_no: Option<usize>,
    right_no: Option<usize>,
    text: String,
}

/// Strip HTML to visible text: `nanohtml2text` drops every tag + attribute,
/// then CRLF is normalized to LF and runs of 3+ blank lines collapse to one.
/// Mirrors the html-to-text block so the two tools agree on "visible text".
pub fn visible_text(html: &str) -> String {
    let raw = nanohtml2text::html2text(html);
    let lf = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::with_capacity(lf.len());
    let mut newline_run = 0usize;
    for ch in lf.chars() {
        if ch == '\n' {
            newline_run += 1;
            if newline_run <= 2 {
                out.push(ch);
            }
        } else {
            newline_run = 0;
            out.push(ch);
        }
    }
    out.trim().to_string()
}

/// Split visible text into diff tokens for the chosen granularity. An empty
/// string yields no tokens at either granularity, so an empty side against a
/// non-empty one renders as pure additions/removals rather than a spurious
/// "changed" pairing of one empty token.
fn tokenize(text: &str, gran: Granularity) -> Vec<String> {
    match gran {
        Granularity::Line => {
            if text.is_empty() {
                return Vec::new();
            }
            let mut out: Vec<String> = text.split('\n').map(|l| l.to_string()).collect();
            // A trailing '\n' yields a spurious empty final element; drop it.
            if out.len() > 1 && out.last().map(|s| s.is_empty()).unwrap_or(false) {
                out.pop();
            }
            out
        }
        Granularity::Word => text.split_whitespace().map(|w| w.to_string()).collect(),
    }
}

/// Build the comparison key for a token under the active options.
fn key(tok: &str, opt: &Options) -> String {
    let mut s = if opt.ignore_whitespace {
        tok.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        tok.to_string()
    };
    if opt.ignore_case {
        s = s.to_lowercase();
    }
    s
}

/// Classic LCS-length DP over the two key vectors, returning the raw diff tokens
/// in original order.
fn diff_tokens(a: &[String], b: &[String], opt: &Options) -> Vec<DiffTok> {
    let ak: Vec<String> = a.iter().map(|t| key(t, opt)).collect();
    let bk: Vec<String> = b.iter().map(|t| key(t, opt)).collect();
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
            out.push(DiffTok {
                tag: Tag::Equal,
                left_no: Some(i + 1),
                right_no: Some(j + 1),
                text: a[i].clone(),
            });
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            out.push(DiffTok {
                tag: Tag::Delete,
                left_no: Some(i + 1),
                right_no: None,
                text: a[i].clone(),
            });
            i += 1;
        } else {
            out.push(DiffTok {
                tag: Tag::Insert,
                left_no: None,
                right_no: Some(j + 1),
                text: b[j].clone(),
            });
            j += 1;
        }
    }
    while i < n {
        out.push(DiffTok {
            tag: Tag::Delete,
            left_no: Some(i + 1),
            right_no: None,
            text: a[i].clone(),
        });
        i += 1;
    }
    while j < m {
        out.push(DiffTok {
            tag: Tag::Insert,
            left_no: None,
            right_no: Some(j + 1),
            text: b[j].clone(),
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

/// Count added/removed/changed tokens. A "changed" pair is one removed token
/// directly replaced by one added token within the same contiguous edit block;
/// `min(deletes, inserts)` of each block are reported as changes, the remainder
/// as pure additions/removals.
fn stats(toks: &[DiffTok]) -> Stats {
    let (mut added, mut removed, mut changed, mut unchanged) = (0, 0, 0, 0);
    let mut idx = 0;
    while idx < toks.len() {
        match toks[idx].tag {
            Tag::Equal => {
                unchanged += 1;
                idx += 1;
            }
            _ => {
                let start = idx;
                while idx < toks.len() && toks[idx].tag != Tag::Equal {
                    idx += 1;
                }
                let block = &toks[start..idx];
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

/// Render a classic unified diff (`@@ -ls,ln +rs,rn @@`, ` `/`-`/`+` prefixes)
/// for line-granularity tokens. Returns `No differences.` when the sides match.
fn render_unified(toks: &[DiffTok], opt: &Options) -> String {
    let st = stats(toks);
    if st.equal {
        return "No differences.".to_string();
    }
    let ctx = opt.context;
    let n = toks.len();

    let mut keep = vec![false; n];
    for (k, l) in toks.iter().enumerate() {
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
        let hunk = &toks[start..k];

        let l_start = hunk
            .iter()
            .find_map(|x| x.left_no)
            .or_else(|| {
                toks[..start]
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
                toks[..start]
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
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Render a `wdiff`-style inline diff for word-granularity tokens: equal words
/// are plain, removed words wrapped `[-...-]`, added words wrapped `{+...+}`.
/// Consecutive same-op words are grouped inside one marker. Returns
/// `No differences.` when the sides match.
fn render_inline(toks: &[DiffTok]) -> String {
    let st = stats(toks);
    if st.equal {
        return "No differences.".to_string();
    }
    let mut out: Vec<String> = Vec::new();
    let mut idx = 0;
    while idx < toks.len() {
        let tag = toks[idx].tag;
        let start = idx;
        while idx < toks.len() && toks[idx].tag == tag {
            idx += 1;
        }
        let words: Vec<&str> = toks[start..idx].iter().map(|t| t.text.as_str()).collect();
        let joined = words.join(" ");
        match tag {
            Tag::Equal => out.push(joined),
            Tag::Delete => out.push(format!("[-{joined}-]")),
            Tag::Insert => out.push(format!("{{+{joined}+}}")),
        }
    }
    out.join(" ")
}

/// Render the structured JSON report. Contiguous delete+insert blocks are paired
/// into `changed` rows so each modified token shows its old and new text.
fn render_json(toks: &[DiffTok], gran: Granularity, indent: usize) -> String {
    let st = stats(toks);
    let mut rows: Vec<serde_json::Value> = Vec::new();
    let mut idx = 0;
    while idx < toks.len() {
        match toks[idx].tag {
            Tag::Equal => {
                let l = &toks[idx];
                rows.push(json!({
                    "op": "equal",
                    "left": l.left_no,
                    "right": l.right_no,
                    "text": l.text,
                }));
                idx += 1;
            }
            _ => {
                let start = idx;
                while idx < toks.len() && toks[idx].tag != Tag::Equal {
                    idx += 1;
                }
                let block = &toks[start..idx];
                let dels: Vec<&DiffTok> = block.iter().filter(|l| l.tag == Tag::Delete).collect();
                let ins: Vec<&DiffTok> = block.iter().filter(|l| l.tag == Tag::Insert).collect();
                let pairs = dels.len().min(ins.len());
                for p in 0..pairs {
                    rows.push(json!({
                        "op": "changed",
                        "left": dels[p].left_no,
                        "right": ins[p].right_no,
                        "old": dels[p].text,
                        "new": ins[p].text,
                    }));
                }
                for d in dels.iter().skip(pairs) {
                    rows.push(json!({
                        "op": "removed",
                        "left": d.left_no,
                        "text": d.text,
                    }));
                }
                for a in ins.iter().skip(pairs) {
                    rows.push(json!({
                        "op": "added",
                        "right": a.right_no,
                        "text": a.text,
                    }));
                }
            }
        }
    }
    let unit = match gran {
        Granularity::Line => "line",
        Granularity::Word => "word",
    };
    let report = json!({
        "equal": st.equal,
        "granularity": unit,
        "added": st.added,
        "removed": st.removed,
        "changed": st.changed,
        "unchanged": st.unchanged,
        "tokens": rows,
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

/// Public entry: strip `left`/`right` HTML to visible text, then diff at
/// `granularity` ("line" | "word") and render as `format` ("unified" | "json").
/// The `unified` renderer produces a patch-style diff for lines and a
/// `wdiff`-style inline diff for words. `indent` applies only to the json
/// renderer (0 minifies). Errors on an unknown granularity/format, or when both
/// inputs are empty (nothing to compare).
pub fn diff_html(
    left: &str,
    right: &str,
    granularity: &str,
    format: &str,
    opt: &Options,
    indent: usize,
) -> Result<String, String> {
    if left.trim().is_empty() && right.trim().is_empty() {
        return Err("both inputs are empty — provide HTML in at least one side".into());
    }
    let gran = Granularity::parse(granularity)?;
    let lt = visible_text(left);
    let rt = visible_text(right);
    let a = tokenize(&lt, gran);
    let b = tokenize(&rt, gran);
    let toks = diff_tokens(&a, &b, opt);
    match format {
        "unified" => Ok(match gran {
            Granularity::Line => render_unified(&toks, opt),
            Granularity::Word => render_inline(&toks),
        }),
        "json" => Ok(render_json(&toks, gran, indent)),
        other => Err(format!(
            "unknown format '{other}' — expected 'unified' or 'json'"
        )),
    }
}

/// Convenience wrapper used by the chat block, the CLI, and the web page: build
/// [`Options`] from the individual flags and render. The JSON renderer is
/// pretty-printed with a 2-space indent; the unified/inline renderer ignores
/// `indent`. Errors are forwarded from [`diff_html`].
#[allow(clippy::too_many_arguments)]
pub fn run(
    left: &str,
    right: &str,
    granularity: &str,
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
    diff_html(left, right, granularity, format, &opt, indent)
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
    fn tags_and_attrs_are_ignored() {
        // Same visible text, wildly different markup -> no differences.
        let left = r#"<p class="a" id="x">Hello <b>world</b></p>"#;
        let right = r#"<div><span style="color:red">Hello</span> <em>world</em></div>"#;
        let out = diff_html(left, right, "word", "unified", &opt(3), 0).unwrap();
        assert_eq!(out, "No differences.", "got: {out:?}");
    }

    #[test]
    fn line_unified_shows_add_remove_change() {
        let left = "<p>alpha</p><p>beta</p><p>gamma</p>";
        let right = "<p>alpha</p><p>BETA</p><p>gamma</p><p>delta</p>";
        let out = diff_html(left, right, "line", "unified", &opt(1), 0).unwrap();
        assert!(out.contains("-beta"), "{out}");
        assert!(out.contains("+BETA"), "{out}");
        assert!(out.contains("+delta"), "{out}");
        assert!(out.starts_with("--- left\n+++ right\n@@"), "{out}");
    }

    #[test]
    fn word_inline_markers() {
        let left = "<p>the quick brown fox</p>";
        let right = "<p>the slow brown fox</p>";
        let out = diff_html(left, right, "word", "unified", &opt(3), 0).unwrap();
        assert!(out.contains("[-quick-]"), "{out}");
        assert!(out.contains("{+slow+}"), "{out}");
        assert!(out.contains("the"), "{out}");
        assert!(out.contains("brown fox"), "{out}");
    }

    #[test]
    fn word_json_pairs_changed() {
        let left = "<b>one two three</b>";
        let right = "<b>one TWO three</b>";
        let out = diff_html(left, right, "word", "json", &opt(3), 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["granularity"], "word");
        assert_eq!(v["changed"], 1);
        assert_eq!(v["unchanged"], 2);
        assert_eq!(v["equal"], false);
        assert_eq!(v["tokens"][1]["op"], "changed");
        assert_eq!(v["tokens"][1]["old"], "two");
        assert_eq!(v["tokens"][1]["new"], "TWO");
    }

    #[test]
    fn ignore_case_folds_word() {
        let left = "<p>Hello World</p>";
        let right = "<p>hello world</p>";
        let mut o = opt(3);
        let plain = diff_html(left, right, "word", "json", &o, 0).unwrap();
        let pv: serde_json::Value = serde_json::from_str(&plain).unwrap();
        assert_eq!(pv["equal"], false);
        o.ignore_case = true;
        let folded = diff_html(left, right, "word", "json", &o, 0).unwrap();
        let fv: serde_json::Value = serde_json::from_str(&folded).unwrap();
        assert_eq!(fv["equal"], true, "{folded}");
    }

    #[test]
    fn ignore_whitespace_line_fold() {
        // Line granularity, ignore_whitespace collapses intra-line spacing so
        // two lines that differ only by run-length of spaces compare equal.
        let left = "<pre>a    b\nc</pre>";
        let right = "<pre>a b\nc</pre>";
        let mut o = opt(3);
        o.ignore_whitespace = true;
        assert_eq!(
            diff_html(left, right, "line", "unified", &o, 0).unwrap(),
            "No differences.",
            "ignore_whitespace should fold the spacing change"
        );
    }

    #[test]
    fn identical_visible_text_reports_no_diff() {
        let out = diff_html("<h1>Hi</h1>", "<h2>Hi</h2>", "line", "unified", &opt(3), 0).unwrap();
        assert_eq!(out, "No differences.");
    }

    #[test]
    fn both_empty_errors() {
        let e = diff_html("", "   ", "line", "unified", &opt(3), 0).unwrap_err();
        assert!(e.contains("both inputs are empty"), "{e}");
    }

    #[test]
    fn one_side_empty_is_all_added() {
        let out = diff_html("", "<p>new text</p>", "line", "json", &opt(3), 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["added"], 1);
        assert_eq!(v["removed"], 0);
    }

    #[test]
    fn unknown_granularity_errors() {
        let e = diff_html("<p>a</p>", "<p>b</p>", "char", "unified", &opt(3), 0).unwrap_err();
        assert!(e.contains("unknown granularity"), "{e}");
    }

    #[test]
    fn unknown_format_errors() {
        let e = diff_html("<p>a</p>", "<p>b</p>", "line", "side-by-side", &opt(3), 0).unwrap_err();
        assert!(e.contains("unknown format"), "{e}");
    }

    #[test]
    fn pretty_json_indents() {
        let out = diff_html("<p>a</p>", "<p>b</p>", "line", "json", &opt(3), 2).unwrap();
        assert!(out.contains("\n  \"equal\""), "{out}");
    }

    #[test]
    fn run_wrapper_end_to_end() {
        // Word inline via the wrapper.
        let inline = run(
            "<p>keep <b>this</b> word</p>",
            "<p>keep that word</p>",
            "word",
            "unified",
            false,
            false,
            3,
        )
        .unwrap();
        assert!(
            inline.contains("[-this-]") && inline.contains("{+that+}"),
            "{inline}"
        );
        // json pretty via the wrapper.
        let pretty = run("<p>a</p>", "<p>b</p>", "line", "json", false, false, 3).unwrap();
        assert!(pretty.contains("\n  \"equal\""), "{pretty}");
        // Unknown format still errors through the wrapper.
        assert!(run("<p>a</p>", "<p>b</p>", "line", "xml", false, false, 3).is_err());
    }
}
