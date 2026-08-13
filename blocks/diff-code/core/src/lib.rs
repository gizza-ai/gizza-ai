//! gizza-ai/diff-code core — compare two code/text snippets and render the
//! difference as side-by-side columns, a unified patch, a word-diff stream, a
//! stat summary, or structured JSON.
//!
//! The line-level engine is a **patience diff**: lines that occur exactly once
//! on both sides anchor the alignment (via the longest increasing subsequence
//! of those anchors), and the gaps between anchors are diffed recursively,
//! falling back to a bounded longest-common-subsequence DP when a gap holds no
//! unique anchor. Patience anchoring is what keeps a moved/duplicated brace or
//! blank line from dragging an otherwise clean code diff out of alignment, and
//! it keeps the cost linear-ish on inputs where a plain O(n·m) DP would not fit
//! in a browser tab.
//!
//! Changed line pairs are then refined **inside the line** at word or character
//! granularity, marked with the `git diff --word-diff` convention —
//! `[-removed-]` and `{+added+}`.
//!
//! Pure-Rust, no I/O — shared by the chat skill block, the CLI, and the page.

use serde_json::{json, Value};
use std::collections::HashMap;

/// Hard input cap per side. Named in the error message so a caller who trips it
/// knows the exact limit.
pub const MAX_INPUT_BYTES: usize = 1_000_000;

/// A changed line pair whose either side tokenises past this many tokens is
/// marked whole-line instead of refined — intra-line refinement is quadratic in
/// the token count, and a minified/one-line-JSON "line" would otherwise stall
/// the page.
pub const MAX_REFINE_TOKENS: usize = 400;

/// Cell budget for the fallback LCS DP (both the line-level gap fallback and
/// the token-level refinement). Past this, the block is emitted as a plain
/// delete-all-then-insert-all replacement rather than allocating a huge table.
const DP_CELL_LIMIT: usize = 1_000_000;

/// Recursion cap for the patience gap recursion. Anchors always shrink the
/// range, so this only guards a pathological input that yields one anchor per
/// level; past it the gap is handed to the bounded DP.
const MAX_RECURSION: usize = 64;

/// Tab stop used when the side-by-side renderer expands tabs for alignment.
const TAB_STOP: usize = 4;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// How a changed line pair is refined inside the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granularity {
    /// Split into word / identifier / punctuation / whitespace tokens.
    Word,
    /// Split into individual characters.
    Char,
    /// No intra-line refinement — a changed line is marked whole.
    None,
}

impl Granularity {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "word" => Ok(Granularity::Word),
            "char" => Ok(Granularity::Char),
            "none" => Ok(Granularity::None),
            other => Err(format!(
                "unknown granularity '{other}' — expected 'word', 'char', or 'none'"
            )),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Granularity::Word => "word",
            Granularity::Char => "char",
            Granularity::None => "none",
        }
    }
}

/// Everything that affects matching and rendering. Matching options never alter
/// the text that is echoed back — the original line is always what you see.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub granularity: Granularity,
    pub ignore_case: bool,
    pub ignore_whitespace: bool,
    /// Unchanged lines kept around each change; further ones collapse.
    pub context: usize,
    /// Show the per-side line numbers in the text views.
    pub line_numbers: bool,
    /// Per-column content width for the side-by-side view.
    pub width: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            granularity: Granularity::Word,
            ignore_case: false,
            ignore_whitespace: false,
            context: 3,
            line_numbers: true,
            width: 60,
        }
    }
}

// ---------------------------------------------------------------------------
// Line splitting + matching keys
// ---------------------------------------------------------------------------

/// Split into lines, keeping a final line that has no trailing newline. An
/// empty input is one empty line so diffing against "" behaves sensibly.
fn split_lines(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return vec![""];
    }
    let mut out: Vec<&str> = s.split('\n').collect();
    if out.len() > 1 && out.last() == Some(&"") {
        out.pop();
    }
    out
}

fn trim_cr(s: &str) -> &str {
    s.strip_suffix('\r').unwrap_or(s)
}

/// Matching key for a line under the active options. CRLF and LF never differ.
fn line_key(line: &str, opt: &Options) -> String {
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

/// Intern the keys so the diff engine compares `u32`s instead of strings.
fn intern(keys: &[String], map: &mut HashMap<String, u32>) -> Vec<u32> {
    keys.iter()
        .map(|k| {
            let next = map.len() as u32;
            *map.entry(k.clone()).or_insert(next)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Patience diff engine (generic over interned token ids)
// ---------------------------------------------------------------------------

/// One edit-script step, carrying the index into the side it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Equal(usize, usize),
    Delete(usize),
    Insert(usize),
}

/// Full edit script aligning `a` to `b`.
fn diff_ops(a: &[u32], b: &[u32]) -> Vec<Op> {
    let mut out = Vec::new();
    rec(a, b, 0, a.len(), 0, b.len(), 0, &mut out);
    out
}

#[allow(clippy::too_many_arguments)]
fn rec(
    a: &[u32],
    b: &[u32],
    mut a0: usize,
    mut a1: usize,
    mut b0: usize,
    mut b1: usize,
    depth: usize,
    out: &mut Vec<Op>,
) {
    // Common prefix — emitted straight away.
    while a0 < a1 && b0 < b1 && a[a0] == b[b0] {
        out.push(Op::Equal(a0, b0));
        a0 += 1;
        b0 += 1;
    }
    // Common suffix — held back until the middle has been emitted.
    let mut suffix = Vec::new();
    while a0 < a1 && b0 < b1 && a[a1 - 1] == b[b1 - 1] {
        a1 -= 1;
        b1 -= 1;
        suffix.push(Op::Equal(a1, b1));
    }

    if a0 == a1 {
        out.extend((b0..b1).map(Op::Insert));
    } else if b0 == b1 {
        out.extend((a0..a1).map(Op::Delete));
    } else {
        let anchors = if depth < MAX_RECURSION {
            patience_anchors(a, b, a0, a1, b0, b1)
        } else {
            Vec::new()
        };
        if anchors.is_empty() {
            lcs_block(a, b, a0, a1, b0, b1, out);
        } else {
            let (mut pa, mut pb) = (a0, b0);
            for (ai, bi) in anchors {
                rec(a, b, pa, ai, pb, bi, depth + 1, out);
                out.push(Op::Equal(ai, bi));
                pa = ai + 1;
                pb = bi + 1;
            }
            rec(a, b, pa, a1, pb, b1, depth + 1, out);
        }
    }

    suffix.reverse();
    out.extend(suffix);
}

/// Ids occurring exactly once in `a[a0..a1]` AND exactly once in `b[b0..b1]`,
/// reduced to the longest increasing subsequence so the anchors are monotone in
/// both sides.
fn patience_anchors(
    a: &[u32],
    b: &[u32],
    a0: usize,
    a1: usize,
    b0: usize,
    b1: usize,
) -> Vec<(usize, usize)> {
    let mut ca: HashMap<u32, (u32, usize)> = HashMap::new();
    for (i, id) in a.iter().enumerate().take(a1).skip(a0) {
        let e = ca.entry(*id).or_insert((0, i));
        e.0 += 1;
        e.1 = i;
    }
    let mut cb: HashMap<u32, (u32, usize)> = HashMap::new();
    for (j, id) in b.iter().enumerate().take(b1).skip(b0) {
        let e = cb.entry(*id).or_insert((0, j));
        e.0 += 1;
        e.1 = j;
    }

    // Candidates in increasing `a` order, so only `b` needs to be made monotone.
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for (i, id) in a.iter().enumerate().take(a1).skip(a0) {
        match (ca.get(id), cb.get(id)) {
            (Some(&(1, _)), Some(&(1, bj))) => pairs.push((i, bj)),
            _ => continue,
        }
    }
    longest_increasing(&pairs)
}

/// Patience-sorting LIS over the second component, with back-pointers.
fn longest_increasing(pairs: &[(usize, usize)]) -> Vec<(usize, usize)> {
    if pairs.is_empty() {
        return Vec::new();
    }
    let mut tails: Vec<usize> = Vec::new();
    let mut prev: Vec<usize> = vec![usize::MAX; pairs.len()];
    for (idx, &(_, bj)) in pairs.iter().enumerate() {
        let pos = tails.partition_point(|&t| pairs[t].1 < bj);
        if pos > 0 {
            prev[idx] = tails[pos - 1];
        }
        if pos == tails.len() {
            tails.push(idx);
        } else {
            tails[pos] = idx;
        }
    }
    let mut out = Vec::new();
    let mut cur = *tails.last().unwrap();
    loop {
        out.push(pairs[cur]);
        if prev[cur] == usize::MAX {
            break;
        }
        cur = prev[cur];
    }
    out.reverse();
    out
}

/// Bounded LCS DP for a gap with no patience anchor. Past `DP_CELL_LIMIT` cells
/// the gap is emitted as a straight replacement instead.
#[allow(clippy::too_many_arguments)]
fn lcs_block(
    a: &[u32],
    b: &[u32],
    a0: usize,
    a1: usize,
    b0: usize,
    b1: usize,
    out: &mut Vec<Op>,
) {
    let (n, m) = (a1 - a0, b1 - b0);
    if n.saturating_mul(m) > DP_CELL_LIMIT {
        out.extend((a0..a1).map(Op::Delete));
        out.extend((b0..b1).map(Op::Insert));
        return;
    }
    let stride = m + 1;
    let mut dp = vec![0u32; (n + 1) * stride];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i * stride + j] = if a[a0 + i] == b[b0 + j] {
                dp[(i + 1) * stride + j + 1] + 1
            } else {
                dp[(i + 1) * stride + j].max(dp[i * stride + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if a[a0 + i] == b[b0 + j] {
            out.push(Op::Equal(a0 + i, b0 + j));
            i += 1;
            j += 1;
        } else if dp[(i + 1) * stride + j] >= dp[i * stride + j + 1] {
            out.push(Op::Delete(a0 + i));
            i += 1;
        } else {
            out.push(Op::Insert(b0 + j));
            j += 1;
        }
    }
    out.extend((a0 + i..a1).map(Op::Delete));
    out.extend((b0 + j..b1).map(Op::Insert));
}

// ---------------------------------------------------------------------------
// Intra-line refinement
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpanOp {
    Equal,
    Delete,
    Insert,
}

impl SpanOp {
    fn name(self) -> &'static str {
        match self {
            SpanOp::Equal => "equal",
            SpanOp::Delete => "delete",
            SpanOp::Insert => "insert",
        }
    }
}

#[derive(Debug, Clone)]
struct Span {
    op: SpanOp,
    text: String,
}

/// Split a line into code-aware tokens: runs of whitespace, runs of
/// word characters (alphanumeric or `_`), and single punctuation characters.
/// Concatenating the tokens reproduces the line exactly.
fn word_tokens(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut kind: u8 = 0; // 0 none, 1 whitespace, 2 word
    for ch in s.chars() {
        let k = if ch.is_whitespace() {
            1
        } else if ch.is_alphanumeric() || ch == '_' {
            2
        } else {
            3
        };
        if k == 3 {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            out.push(ch.to_string());
            kind = 0;
            continue;
        }
        if k != kind && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        kind = k;
        cur.push(ch);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn char_tokens(s: &str) -> Vec<String> {
    s.chars().map(|c| c.to_string()).collect()
}

fn token_key(tok: &str, opt: &Options) -> String {
    let mut s = if opt.ignore_whitespace && tok.chars().all(char::is_whitespace) {
        " ".to_string()
    } else {
        tok.to_string()
    };
    if opt.ignore_case {
        s = s.to_lowercase();
    }
    s
}

/// Merged span stream for a changed line pair, or `None` when refinement is
/// skipped (granularity `none`, or either side past `MAX_REFINE_TOKENS`).
fn refine(left: &str, right: &str, opt: &Options) -> Option<Vec<Span>> {
    let (lt, rt) = match opt.granularity {
        Granularity::None => return None,
        Granularity::Word => (word_tokens(left), word_tokens(right)),
        Granularity::Char => (char_tokens(left), char_tokens(right)),
    };
    if lt.len() > MAX_REFINE_TOKENS || rt.len() > MAX_REFINE_TOKENS {
        return None;
    }

    let mut map: HashMap<String, u32> = HashMap::new();
    let lk: Vec<String> = lt.iter().map(|t| token_key(t, opt)).collect();
    let rk: Vec<String> = rt.iter().map(|t| token_key(t, opt)).collect();
    let a = intern(&lk, &mut map);
    let b = intern(&rk, &mut map);

    let mut spans: Vec<Span> = Vec::new();
    for op in diff_ops(&a, &b) {
        let (o, text) = match op {
            Op::Equal(i, _) => (SpanOp::Equal, lt[i].clone()),
            Op::Delete(i) => (SpanOp::Delete, lt[i].clone()),
            Op::Insert(j) => (SpanOp::Insert, rt[j].clone()),
        };
        match spans.last_mut() {
            Some(last) if last.op == o => last.text.push_str(&text),
            _ => spans.push(Span { op: o, text }),
        }
    }
    Some(spans)
}

/// Render one side of a refined pair. `keep` selects which non-equal op belongs
/// to this side; that op's runs are wrapped in `open`/`close`.
fn render_side(spans: &[Span], keep: SpanOp, open: &str, close: &str) -> String {
    let mut out = String::new();
    for s in spans {
        if s.op == SpanOp::Equal {
            out.push_str(&s.text);
        } else if s.op == keep {
            out.push_str(open);
            out.push_str(&s.text);
            out.push_str(close);
        }
    }
    out
}

/// Render the merged `git diff --word-diff` stream for a refined pair.
fn render_merged(spans: &[Span]) -> String {
    let mut out = String::new();
    for s in spans {
        match s.op {
            SpanOp::Equal => out.push_str(&s.text),
            SpanOp::Delete => {
                out.push_str("[-");
                out.push_str(&s.text);
                out.push_str("-]");
            }
            SpanOp::Insert => {
                out.push_str("{+");
                out.push_str(&s.text);
                out.push_str("+}");
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Row model (side-by-side / word-diff / json / stats)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowKind {
    Equal,
    Changed,
    Deleted,
    Inserted,
}

impl RowKind {
    fn name(self) -> &'static str {
        match self {
            RowKind::Equal => "equal",
            RowKind::Changed => "changed",
            RowKind::Deleted => "deleted",
            RowKind::Inserted => "inserted",
        }
    }
}

#[derive(Debug, Clone)]
struct Row {
    kind: RowKind,
    left_no: Option<usize>,
    right_no: Option<usize>,
    left: Option<String>,
    right: Option<String>,
    /// Only set for a refined `Changed` row.
    spans: Option<Vec<Span>>,
}

/// Pair the edit script into display rows: inside each contiguous edit block the
/// k-th deletion pairs with the k-th insertion as a single `Changed` row, and
/// the surplus on either side becomes `Deleted` / `Inserted` rows.
fn build_rows(ops: &[Op], a: &[&str], b: &[&str], opt: &Options) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut idx = 0;
    while idx < ops.len() {
        match ops[idx] {
            Op::Equal(i, j) => {
                rows.push(Row {
                    kind: RowKind::Equal,
                    left_no: Some(i + 1),
                    right_no: Some(j + 1),
                    left: Some(trim_cr(a[i]).to_string()),
                    right: Some(trim_cr(b[j]).to_string()),
                    spans: None,
                });
                idx += 1;
            }
            _ => {
                let start = idx;
                while idx < ops.len() && !matches!(ops[idx], Op::Equal(_, _)) {
                    idx += 1;
                }
                let mut dels: Vec<usize> = Vec::new();
                let mut inss: Vec<usize> = Vec::new();
                for op in &ops[start..idx] {
                    match *op {
                        Op::Delete(i) => dels.push(i),
                        Op::Insert(j) => inss.push(j),
                        Op::Equal(_, _) => unreachable!(),
                    }
                }
                let pairs = dels.len().min(inss.len());
                for k in 0..pairs {
                    let l = trim_cr(a[dels[k]]).to_string();
                    let r = trim_cr(b[inss[k]]).to_string();
                    let spans = refine(&l, &r, opt);
                    rows.push(Row {
                        kind: RowKind::Changed,
                        left_no: Some(dels[k] + 1),
                        right_no: Some(inss[k] + 1),
                        left: Some(l),
                        right: Some(r),
                        spans,
                    });
                }
                for &i in &dels[pairs..] {
                    rows.push(Row {
                        kind: RowKind::Deleted,
                        left_no: Some(i + 1),
                        right_no: None,
                        left: Some(trim_cr(a[i]).to_string()),
                        right: None,
                        spans: None,
                    });
                }
                for &j in &inss[pairs..] {
                    rows.push(Row {
                        kind: RowKind::Inserted,
                        left_no: None,
                        right_no: Some(j + 1),
                        left: None,
                        right: Some(trim_cr(b[j]).to_string()),
                        spans: None,
                    });
                }
            }
        }
    }
    rows
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Stats {
    left_lines: usize,
    right_lines: usize,
    added: usize,
    removed: usize,
    changed: usize,
    unchanged: usize,
    tokens_removed: usize,
    tokens_added: usize,
}

impl Stats {
    fn equal(&self) -> bool {
        self.added == 0 && self.removed == 0 && self.changed == 0
    }

    /// Unchanged lines as a share of the longer side. Reported to one decimal.
    fn similarity(&self) -> f64 {
        let denom = self.left_lines.max(self.right_lines);
        if denom == 0 {
            return 100.0;
        }
        (self.unchanged as f64) * 100.0 / (denom as f64)
    }
}

fn stats_of(rows: &[Row], left_lines: usize, right_lines: usize) -> Stats {
    let mut s = Stats {
        left_lines,
        right_lines,
        added: 0,
        removed: 0,
        changed: 0,
        unchanged: 0,
        tokens_removed: 0,
        tokens_added: 0,
    };
    for r in rows {
        match r.kind {
            RowKind::Equal => s.unchanged += 1,
            RowKind::Changed => s.changed += 1,
            RowKind::Deleted => s.removed += 1,
            RowKind::Inserted => s.added += 1,
        }
        if let Some(spans) = &r.spans {
            for sp in spans {
                match sp.op {
                    SpanOp::Delete => s.tokens_removed += 1,
                    SpanOp::Insert => s.tokens_added += 1,
                    SpanOp::Equal => {}
                }
            }
        }
    }
    s
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn summary_line(s: &Stats) -> String {
    format!(
        "{}, {}, {}, {}",
        plural(s.added, "line added", "lines added"),
        plural(s.removed, "line removed", "lines removed"),
        plural(s.changed, "line changed", "lines changed"),
        plural(s.unchanged, "line unchanged", "lines unchanged"),
    )
}

// ---------------------------------------------------------------------------
// Context windowing
// ---------------------------------------------------------------------------

/// Which rows survive the `context` window, and where the collapsed gaps are.
/// Returns `(keep, gap_before)` — `gap_before[i]` is the number of unchanged
/// rows dropped immediately before kept row `i`.
fn context_window(rows: &[Row], context: usize) -> (Vec<usize>, HashMap<usize, usize>) {
    let mut keep_flag = vec![false; rows.len()];
    for (i, r) in rows.iter().enumerate() {
        if r.kind == RowKind::Equal {
            continue;
        }
        let lo = i.saturating_sub(context);
        let hi = (i + context).min(rows.len().saturating_sub(1));
        for f in keep_flag.iter_mut().take(hi + 1).skip(lo) {
            *f = true;
        }
    }
    let mut keep = Vec::new();
    let mut gaps = HashMap::new();
    let mut dropped = 0usize;
    for (i, f) in keep_flag.iter().enumerate() {
        if *f {
            if dropped > 0 {
                gaps.insert(keep.len(), dropped);
                dropped = 0;
            }
            keep.push(i);
        } else {
            dropped += 1;
        }
    }
    (keep, gaps)
}

fn gap_marker(n: usize) -> String {
    format!("… {} …", plural(n, "unchanged line", "unchanged lines"))
}

// ---------------------------------------------------------------------------
// Renderers
// ---------------------------------------------------------------------------

/// Expand tabs to `TAB_STOP` stops so the two columns stay aligned. Display
/// only, and only in the side-by-side view.
fn expand_tabs(s: &str) -> String {
    let mut out = String::new();
    let mut col = 0usize;
    for ch in s.chars() {
        if ch == '\t' {
            let pad = TAB_STOP - (col % TAB_STOP);
            out.extend(std::iter::repeat_n(' ', pad));
            col += pad;
        } else {
            out.push(ch);
            col += 1;
        }
    }
    out
}

fn pad_to(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - len))
    }
}

fn render_side_by_side(rows: &[Row], stats: &Stats, opt: &Options) -> String {
    let (keep, gaps) = context_window(rows, opt.context);
    // Number column wide enough for the largest line number on either side.
    let no_w = if opt.line_numbers {
        stats.left_lines.max(stats.right_lines).to_string().len().max(2)
    } else {
        0
    };
    let cell = |no: Option<usize>, mark: char, text: Option<&String>, pad: bool| -> String {
        let mut s = String::new();
        if opt.line_numbers {
            match no {
                Some(n) => s.push_str(&format!("{n:>no_w$} ")),
                None => s.push_str(&format!("{} ", " ".repeat(no_w))),
            }
        }
        s.push(mark);
        s.push(' ');
        let body = expand_tabs(text.map(String::as_str).unwrap_or(""));
        s.push_str(&if pad { pad_to(&body, opt.width) } else { body });
        s
    };

    let mut lines: Vec<String> = Vec::new();
    for (pos, &ri) in keep.iter().enumerate() {
        if let Some(&n) = gaps.get(&pos) {
            lines.push(gap_marker(n));
        }
        let r = &rows[ri];
        let (lm, rm) = match r.kind {
            RowKind::Equal => (' ', ' '),
            RowKind::Changed => ('~', '~'),
            RowKind::Deleted => ('-', ' '),
            RowKind::Inserted => (' ', '+'),
        };
        let (lt, rt) = match (&r.spans, r.kind) {
            (Some(spans), RowKind::Changed) => (
                Some(render_side(spans, SpanOp::Delete, "[-", "-]")),
                Some(render_side(spans, SpanOp::Insert, "{+", "+}")),
            ),
            _ => (r.left.clone(), r.right.clone()),
        };
        let left_cell = cell(r.left_no, lm, lt.as_ref(), true);
        let right_cell = cell(r.right_no, rm, rt.as_ref(), false);
        lines.push(format!("{left_cell} | {}", right_cell.trim_end()));
    }
    // Trailing gap (unchanged tail beyond the context window).
    let dropped_tail = rows.len() - keep.last().map(|i| i + 1).unwrap_or(0);
    if dropped_tail > 0 {
        lines.push(gap_marker(dropped_tail));
    }
    lines.push(String::new());
    lines.push(summary_line(stats));
    lines.join("\n")
}

fn render_word_diff(rows: &[Row], stats: &Stats, opt: &Options) -> String {
    let (keep, gaps) = context_window(rows, opt.context);
    let no_w = if opt.line_numbers {
        stats.left_lines.max(stats.right_lines).to_string().len().max(2)
    } else {
        0
    };
    let mut lines: Vec<String> = Vec::new();
    for (pos, &ri) in keep.iter().enumerate() {
        if let Some(&n) = gaps.get(&pos) {
            lines.push(gap_marker(n));
        }
        let r = &rows[ri];
        let body = match r.kind {
            RowKind::Equal => r.left.clone().unwrap_or_default(),
            RowKind::Changed => match &r.spans {
                Some(spans) => render_merged(spans),
                None => format!(
                    "[-{}-]{{+{}+}}",
                    r.left.clone().unwrap_or_default(),
                    r.right.clone().unwrap_or_default()
                ),
            },
            RowKind::Deleted => format!("[-{}-]", r.left.clone().unwrap_or_default()),
            RowKind::Inserted => format!("{{+{}+}}", r.right.clone().unwrap_or_default()),
        };
        if opt.line_numbers {
            let n = r.right_no.or(r.left_no).unwrap_or(0);
            lines.push(format!("{n:>no_w$}  {body}"));
        } else {
            lines.push(body);
        }
    }
    let dropped_tail = rows.len() - keep.last().map(|i| i + 1).unwrap_or(0);
    if dropped_tail > 0 {
        lines.push(gap_marker(dropped_tail));
    }
    lines.push(String::new());
    lines.push(summary_line(stats));
    lines.join("\n")
}

/// Classic `@@` unified diff. Deliberately CLEAN — no intra-line markers and no
/// summary line — so the result stays a valid patch you can pipe into
/// `git apply`, the `apply-patch` tool, or the `diff-viewer` tool.
fn render_unified(ops: &[Op], a: &[&str], b: &[&str], opt: &Options) -> String {
    // Flatten to marker lines, emitting every deletion of an edit block before
    // its insertions (the conventional unified ordering).
    let mut flat: Vec<(char, usize, usize, String)> = Vec::new();
    let mut idx = 0;
    while idx < ops.len() {
        match ops[idx] {
            Op::Equal(i, j) => {
                flat.push((' ', i + 1, j + 1, trim_cr(a[i]).to_string()));
                idx += 1;
            }
            _ => {
                let start = idx;
                while idx < ops.len() && !matches!(ops[idx], Op::Equal(_, _)) {
                    idx += 1;
                }
                for op in &ops[start..idx] {
                    if let Op::Delete(i) = *op {
                        flat.push(('-', i + 1, 0, trim_cr(a[i]).to_string()));
                    }
                }
                for op in &ops[start..idx] {
                    if let Op::Insert(j) = *op {
                        flat.push(('+', 0, j + 1, trim_cr(b[j]).to_string()));
                    }
                }
            }
        }
    }

    // Group changed lines into hunks, padded by `context` unchanged lines.
    let mut keep = vec![false; flat.len()];
    for i in 0..flat.len() {
        if flat[i].0 == ' ' {
            continue;
        }
        let lo = i.saturating_sub(opt.context);
        let hi = (i + opt.context).min(flat.len().saturating_sub(1));
        for f in keep.iter_mut().take(hi + 1).skip(lo) {
            *f = true;
        }
    }

    let mut out = vec!["--- left".to_string(), "+++ right".to_string()];
    let mut i = 0;
    while i < flat.len() {
        if !keep[i] {
            i += 1;
            continue;
        }
        let start = i;
        while i < flat.len() && keep[i] {
            i += 1;
        }
        let hunk = &flat[start..i];
        let old_count = hunk.iter().filter(|l| l.0 != '+').count();
        let new_count = hunk.iter().filter(|l| l.0 != '-').count();
        let old_start = hunk
            .iter()
            .find(|l| l.0 != '+')
            .map(|l| l.1)
            .unwrap_or(0)
            .max(if old_count == 0 { 0 } else { 1 });
        let new_start = hunk
            .iter()
            .find(|l| l.0 != '-')
            .map(|l| l.2)
            .unwrap_or(0)
            .max(if new_count == 0 { 0 } else { 1 });
        out.push(format!(
            "@@ -{old_start},{old_count} +{new_start},{new_count} @@"
        ));
        for (mark, _, _, text) in hunk {
            out.push(format!("{mark}{text}"));
        }
    }
    out.join("\n")
}

fn render_stats(stats: &Stats, opt: &Options) -> String {
    let mut out = vec![
        format!(
            "Lines:      {} left / {} right",
            stats.left_lines, stats.right_lines
        ),
        format!("Added:      {}", stats.added),
        format!("Removed:    {}", stats.removed),
        format!("Changed:    {}", stats.changed),
        format!("Unchanged:  {}", stats.unchanged),
        format!("Similarity: {:.1}%", stats.similarity()),
    ];
    if opt.granularity != Granularity::None && stats.changed > 0 {
        out.push(format!(
            "{}-level:  {} removed / {} added inside changed lines",
            if opt.granularity == Granularity::Word {
                "Word"
            } else {
                "Char"
            },
            stats.tokens_removed,
            stats.tokens_added,
        ));
    }
    out.join("\n")
}

fn render_json(rows: &[Row], stats: &Stats, opt: &Options) -> String {
    let row_values: Vec<Value> = rows
        .iter()
        .map(|r| {
            let mut o = serde_json::Map::new();
            o.insert("kind".into(), json!(r.kind.name()));
            if let Some(n) = r.left_no {
                o.insert("left_line".into(), json!(n));
            }
            if let Some(n) = r.right_no {
                o.insert("right_line".into(), json!(n));
            }
            if let Some(t) = &r.left {
                o.insert("left".into(), json!(t));
            }
            if let Some(t) = &r.right {
                o.insert("right".into(), json!(t));
            }
            if let Some(spans) = &r.spans {
                o.insert(
                    "spans".into(),
                    json!(spans
                        .iter()
                        .map(|s| json!({ "op": s.op.name(), "text": s.text }))
                        .collect::<Vec<_>>()),
                );
            }
            Value::Object(o)
        })
        .collect();

    let report = json!({
        "equal": stats.equal(),
        "granularity": opt.granularity.name(),
        "stats": {
            "left_lines": stats.left_lines,
            "right_lines": stats.right_lines,
            "added": stats.added,
            "removed": stats.removed,
            "changed": stats.changed,
            "unchanged": stats.unchanged,
            "similarity": (stats.similarity() * 10.0).round() / 10.0,
            "tokens_removed": stats.tokens_removed,
            "tokens_added": stats.tokens_added,
        },
        "rows": row_values,
    });
    serde_json::to_string_pretty(&report).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Every accepted `view` value, in descriptor order.
pub const VIEWS: [&str; 5] = ["side-by-side", "unified", "word-diff", "stats", "json"];

/// Diff `left` vs `right` and render as `view`.
///
/// Errors when either side exceeds [`MAX_INPUT_BYTES`], or when `view` /
/// `Options::granularity` is not a recognised value.
pub fn diff_code(left: &str, right: &str, view: &str, opt: &Options) -> Result<String, String> {
    if left.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "left is {} bytes — the limit is {MAX_INPUT_BYTES} bytes (1 MB) per side",
            left.len()
        ));
    }
    if right.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "right is {} bytes — the limit is {MAX_INPUT_BYTES} bytes (1 MB) per side",
            right.len()
        ));
    }
    let view = if view.trim().is_empty() {
        "side-by-side"
    } else {
        view.trim()
    };
    if !VIEWS.contains(&view) {
        return Err(format!(
            "unknown view '{view}' — expected one of {}",
            VIEWS
                .iter()
                .map(|v| format!("'{v}'"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let a = split_lines(left);
    let b = split_lines(right);
    let mut map: HashMap<String, u32> = HashMap::new();
    let ak: Vec<String> = a.iter().map(|l| line_key(l, opt)).collect();
    let bk: Vec<String> = b.iter().map(|l| line_key(l, opt)).collect();
    let ai = intern(&ak, &mut map);
    let bi = intern(&bk, &mut map);

    let ops = diff_ops(&ai, &bi);
    let rows = build_rows(&ops, &a, &b, opt);
    let stats = stats_of(&rows, a.len(), b.len());

    if stats.equal() && matches!(view, "side-by-side" | "unified" | "word-diff") {
        return Ok("No differences.".to_string());
    }

    Ok(match view {
        "side-by-side" => render_side_by_side(&rows, &stats, opt),
        "unified" => render_unified(&ops, &a, &b, opt),
        "word-diff" => render_word_diff(&rows, &stats, opt),
        "stats" => render_stats(&stats, opt),
        "json" => render_json(&rows, &stats, opt),
        _ => unreachable!("view validated above"),
    })
}

/// Flag-taking wrapper shared by the chat block, the CLI, and the web page.
/// `context` is clamped to 0–100 and `width` to 20–200.
#[allow(clippy::too_many_arguments)]
pub fn run(
    left: &str,
    right: &str,
    view: &str,
    granularity: &str,
    ignore_case: bool,
    ignore_whitespace: bool,
    context: usize,
    line_numbers: bool,
    width: usize,
) -> Result<String, String> {
    let opt = Options {
        granularity: Granularity::parse(granularity)?,
        ignore_case,
        ignore_whitespace,
        context: context.min(100),
        line_numbers,
        width: width.clamp(20, 200),
    };
    diff_code(left, right, view, &opt)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "fn main() {\n    println!(\"hi\");\n}";
    const NEW: &str = "fn main() {\n    println!(\"hello\");\n    println!(\"world\");\n}";

    fn opt() -> Options {
        Options::default()
    }

    #[test]
    fn side_by_side_marks_word_level_change() {
        let out = diff_code(OLD, NEW, "side-by-side", &opt()).unwrap();
        // The changed line keeps its unchanged prefix and marks only the token.
        assert!(out.contains("[-hi-]"), "{out}");
        assert!(out.contains("{+hello+}"), "{out}");
        // The pure insertion shows up on the right side only.
        assert!(out.contains("+     println!(\"world\");"), "{out}");
        // Two columns separated by the divider.
        assert!(out.lines().next().unwrap().contains(" | "), "{out}");
        assert!(
            out.ends_with("1 line added, 0 lines removed, 1 line changed, 2 lines unchanged"),
            "{out}"
        );
    }

    #[test]
    fn unknown_view_errors() {
        let e = diff_code(OLD, NEW, "split", &opt()).unwrap_err();
        assert!(e.contains("unknown view 'split'"), "{e}");
        assert!(e.contains("'side-by-side'"), "{e}");
    }

    #[test]
    fn unknown_granularity_errors() {
        let e = run(OLD, NEW, "side-by-side", "token", false, false, 3, true, 60).unwrap_err();
        assert!(e.contains("unknown granularity 'token'"), "{e}");
    }

    #[test]
    fn oversize_input_errors_with_the_limit() {
        let big = "x\n".repeat(MAX_INPUT_BYTES); // 2 MB
        let e = diff_code(&big, "y", "stats", &opt()).unwrap_err();
        assert!(e.contains("the limit is 1000000 bytes (1 MB) per side"), "{e}");
        let e = diff_code("y", &big, "stats", &opt()).unwrap_err();
        assert!(e.starts_with("right is "), "{e}");
    }

    #[test]
    fn unified_is_a_clean_applicable_patch() {
        let out = diff_code(OLD, NEW, "unified", &opt()).unwrap();
        assert_eq!(
            out,
            "--- left\n\
             +++ right\n\
             @@ -1,3 +1,4 @@\n\
             \x20fn main() {\n\
             -    println!(\"hi\");\n\
             +    println!(\"hello\");\n\
             +    println!(\"world\");\n\
             \x20}"
        );
        // No intra-line markers and no summary line leak into the patch.
        assert!(!out.contains("[-"), "{out}");
        assert!(!out.contains("lines unchanged"), "{out}");
    }

    #[test]
    fn word_diff_merges_both_sides_into_one_stream() {
        let out = diff_code(OLD, NEW, "word-diff", &opt()).unwrap();
        assert!(out.contains("[-hi-]{+hello+}"), "{out}");
        assert!(out.contains("{+    println!(\"world\");+}"), "{out}");
    }

    #[test]
    fn char_granularity_is_finer_than_word() {
        let o = Options {
            granularity: Granularity::Char,
            ..Options::default()
        };
        // "color" -> "colour": word granularity replaces the whole identifier,
        // char granularity inserts the single letter.
        let out = diff_code("var color;", "var colour;", "word-diff", &o).unwrap();
        assert!(out.contains("colo{+u+}r"), "{out}");
        let w = diff_code("var color;", "var colour;", "word-diff", &opt()).unwrap();
        assert!(w.contains("[-color-]{+colour+}"), "{w}");
    }

    #[test]
    fn granularity_none_marks_whole_lines() {
        let o = Options {
            granularity: Granularity::None,
            ..Options::default()
        };
        let out = diff_code(OLD, NEW, "word-diff", &o).unwrap();
        assert!(out.contains("[-    println!(\"hi\");-]"), "{out}");
        assert!(!out.contains("[-hi-]"), "{out}");
    }

    #[test]
    fn ignore_case_and_whitespace_match_only() {
        let o = Options {
            ignore_case: true,
            ignore_whitespace: true,
            ..Options::default()
        };
        let out = diff_code("Hello   World", "hello world", "side-by-side", &o).unwrap();
        assert_eq!(out, "No differences.");
        // Without the flags it is a real change, and the original text is echoed.
        let out = diff_code("Hello   World", "hello world", "side-by-side", &opt()).unwrap();
        assert!(out.contains("Hello"), "{out}");
        assert!(out.contains("hello"), "{out}");
    }

    #[test]
    fn crlf_is_not_a_difference() {
        let out = diff_code("a\r\nb\r\n", "a\nb\n", "side-by-side", &opt()).unwrap();
        assert_eq!(out, "No differences.");
    }

    #[test]
    fn context_collapses_distant_unchanged_lines() {
        let left: String = (1..=20).map(|i| format!("line {i}\n")).collect();
        let right = left.replace("line 10", "line TEN");
        let o = Options {
            context: 1,
            ..Options::default()
        };
        let out = diff_code(&left, &right, "side-by-side", &o).unwrap();
        assert!(out.contains("… 8 unchanged lines …"), "{out}");
        assert!(out.contains("… 9 unchanged lines …"), "{out}");
        assert!(!out.contains("line 5"), "{out}");
        // context=0 keeps only the changed row.
        let o0 = Options {
            context: 0,
            ..Options::default()
        };
        let out0 = diff_code(&left, &right, "side-by-side", &o0).unwrap();
        assert!(!out0.contains("line 9"), "{out0}");
    }

    #[test]
    fn stats_view_reports_counts_and_similarity() {
        let out = diff_code(OLD, NEW, "stats", &opt()).unwrap();
        assert_eq!(
            out,
            "Lines:      3 left / 4 right\n\
             Added:      1\n\
             Removed:    0\n\
             Changed:    1\n\
             Unchanged:  2\n\
             Similarity: 50.0%\n\
             Word-level:  1 removed / 1 added inside changed lines"
        );
    }

    #[test]
    fn json_view_reports_rows_and_spans() {
        let out = diff_code(OLD, NEW, "json", &opt()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["equal"], false);
        assert_eq!(v["granularity"], "word");
        assert_eq!(v["stats"]["changed"], 1);
        assert_eq!(v["stats"]["added"], 1);
        assert_eq!(v["stats"]["similarity"], 50.0);
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows[0]["kind"], "equal");
        assert_eq!(rows[1]["kind"], "changed");
        assert_eq!(rows[1]["left_line"], 2);
        let spans = rows[1]["spans"].as_array().unwrap();
        assert!(spans.iter().any(|s| s["op"] == "delete" && s["text"] == "hi"));
        assert!(spans.iter().any(|s| s["op"] == "insert" && s["text"] == "hello"));
        // json still renders when the two sides are identical.
        let same: serde_json::Value =
            serde_json::from_str(&diff_code(OLD, OLD, "json", &opt()).unwrap()).unwrap();
        assert_eq!(same["equal"], true);
        assert_eq!(same["stats"]["similarity"], 100.0);
    }

    #[test]
    fn line_numbers_can_be_turned_off() {
        let out = diff_code(OLD, NEW, "side-by-side", &opt()).unwrap();
        assert!(out.contains(" 1   fn main() {"), "{out}");
        let o = Options {
            line_numbers: false,
            ..Options::default()
        };
        let bare = diff_code(OLD, NEW, "side-by-side", &o).unwrap();
        assert!(bare.starts_with("  fn main() {"), "{bare}");
    }

    #[test]
    fn width_sets_the_left_column_and_tabs_expand() {
        let o = Options {
            width: 24,
            line_numbers: false,
            ..Options::default()
        };
        let out = diff_code("\tab", "\tab", "side-by-side", &o).unwrap();
        assert_eq!(out, "No differences.");
        let out = diff_code("\tab\nx", "\tab\ny", "side-by-side", &o).unwrap();
        let first = out.lines().next().unwrap();
        // Tab expanded to 4 spaces, then padded out to the 24-char column.
        assert!(first.starts_with("      ab"), "{first:?}");
        assert_eq!(first.find(" | "), Some(2 + 24), "{first:?}");
    }

    #[test]
    fn long_lines_skip_intra_line_refinement() {
        let left = format!("{} end", "a ".repeat(MAX_REFINE_TOKENS));
        let right = format!("{} tail", "a ".repeat(MAX_REFINE_TOKENS));
        let out = diff_code(&left, &right, "word-diff", &opt()).unwrap();
        // Refinement is skipped, so the whole line is marked on each side.
        assert!(out.contains("end-]"), "{out}");
        assert!(!out.contains("a-]"), "{out}");
    }

    #[test]
    fn patience_anchoring_keeps_repeated_braces_aligned() {
        // A duplicated closing brace must not drag the diff out of alignment:
        // only the body line changes.
        let left = "fn a() {\n    one();\n}\nfn b() {\n    two();\n}\n";
        let right = "fn a() {\n    one();\n}\nfn b() {\n    TWO();\n}\n";
        let out = diff_code(left, right, "stats", &opt()).unwrap();
        assert!(out.contains("Changed:    1"), "{out}");
        assert!(out.contains("Added:      0"), "{out}");
        assert!(out.contains("Removed:    0"), "{out}");
    }

    #[test]
    fn empty_against_content_is_a_pure_insertion() {
        let out = diff_code("", "new line\n", "stats", &opt()).unwrap();
        assert!(out.contains("Lines:      1 left / 1 right"), "{out}");
        assert!(out.contains("Changed:    1"), "{out}");
    }

    #[test]
    fn run_wrapper_clamps_and_defaults() {
        // Blank view falls back to the side-by-side default.
        let out = run(OLD, NEW, "", "", false, false, 3, true, 60).unwrap();
        assert!(out.contains(" | "), "{out}");
        // Out-of-range context/width are clamped rather than rejected.
        let out = run(OLD, NEW, "side-by-side", "word", false, false, 9999, true, 1).unwrap();
        assert!(out.contains("[-hi-]"), "{out}");
        assert!(run(OLD, NEW, "stats", "none", true, true, 0, false, 500).is_ok());
    }
}
