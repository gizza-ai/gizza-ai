//! ast-diff core — a structural (AST-level) diff for Rust source.
//!
//! Both inputs are parsed into a `syn` abstract syntax tree and re-emitted in a
//! single canonical form (`prettyplease`). Because the AST carries no formatting,
//! whitespace, indentation, or line-comment information, two files that differ
//! only in how they are laid out canonicalize to the *same* text — so those
//! changes never appear in the diff. Only differences the compiler would see
//! (renamed items, changed expressions, added/removed code) survive.
//!
//! Pure compute, shared by the chat skill block and the web page. No wafer /
//! wasm-bindgen deps.

/// Which output the caller wants back.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// A git-style unified diff of the two canonical forms (the default).
    Unified,
    /// A one-line-per-fact summary: identical / formatting-only / N added, M removed.
    Summary,
    /// The canonical (re-formatted) form of source A — no diff.
    CanonicalA,
    /// The canonical (re-formatted) form of source B — no diff.
    CanonicalB,
}

impl Mode {
    fn parse(s: &str) -> Result<Mode, String> {
        match s.trim() {
            "" | "unified" => Ok(Mode::Unified),
            "summary" => Ok(Mode::Summary),
            "canonical-a" => Ok(Mode::CanonicalA),
            "canonical-b" => Ok(Mode::CanonicalB),
            other => Err(format!(
                "unknown mode '{other}'; expected one of: unified, summary, canonical-a, canonical-b"
            )),
        }
    }
}

/// Guard against the O(n·m) diff table blowing the 64 MiB sandbox: a snippet
/// diff never needs millions of line-pairs, and a pathological paste shouldn't
/// OOM-trap. ~4M cells ≈ 32 MiB of `u32`.
const MAX_DIFF_CELLS: usize = 4_000_000;

/// Diff two Rust sources structurally.
///
/// `a`/`b` are Rust source text (a whole file or a paste-able snippet). `mode`
/// selects the output (see [`Mode`]). `context` is the number of unchanged
/// context lines kept around each change in unified mode (clamped to 0..=100).
pub fn diff(a: &str, b: &str, mode: &str, context: usize) -> Result<String, String> {
    let mode = Mode::parse(mode)?;
    let canon_a = canonicalize(a).map_err(|e| format!("source A: {e}"))?;

    if mode == Mode::CanonicalA {
        return Ok(canon_a);
    }
    let canon_b = canonicalize(b).map_err(|e| format!("source B: {e}"))?;
    if mode == Mode::CanonicalB {
        return Ok(canon_b);
    }

    let a_lines: Vec<&str> = canon_a.lines().collect();
    let b_lines: Vec<&str> = canon_b.lines().collect();
    if a_lines.len().saturating_mul(b_lines.len()) > MAX_DIFF_CELLS {
        return Err(format!(
            "sources are too large to diff ({} × {} canonical lines exceeds the {}-cell limit); \
             diff smaller files or fewer items",
            a_lines.len(),
            b_lines.len(),
            MAX_DIFF_CELLS
        ));
    }

    let ops = diff_ops(&a_lines, &b_lines);
    let (added, removed) = ops.iter().fold((0usize, 0usize), |(add, del), op| match op {
        Op::Ins(_) => (add + 1, del),
        Op::Del(_) => (add, del + 1),
        Op::Eq(..) => (add, del),
    });

    let structurally_identical = added == 0 && removed == 0;

    match mode {
        Mode::Summary => Ok(summary(a, b, structurally_identical, added, removed)),
        Mode::Unified => {
            if structurally_identical {
                Ok(no_diff_message(a, b))
            } else {
                Ok(render_unified(&a_lines, &b_lines, &ops, context.min(100)))
            }
        }
        Mode::CanonicalA | Mode::CanonicalB => unreachable!(),
    }
}

/// Parse Rust source and re-emit it in canonical form. Returns a human-readable
/// parse error (with line/column) when the input isn't valid Rust.
fn canonicalize(src: &str) -> Result<String, String> {
    match syn::parse_file(src) {
        Ok(file) => Ok(prettyplease::unparse(&file)),
        Err(e) => {
            let start = e.span().start();
            // proc-macro2's span-locations feature makes start.line 1-based; it
            // is 0 only when the lexer couldn't attribute a position.
            if start.line > 0 {
                Err(format!(
                    "not valid Rust: {e} (at line {}, column {})",
                    start.line,
                    start.column + 1
                ))
            } else {
                Err(format!("not valid Rust: {e}"))
            }
        }
    }
}

fn no_diff_message(a: &str, b: &str) -> String {
    if a == b {
        "The two sources are identical.".to_string()
    } else {
        "No structural differences.\n\
         The inputs are the same program — they differ only in formatting, whitespace, or comments."
            .to_string()
    }
}

fn summary(a: &str, b: &str, identical: bool, added: usize, removed: usize) -> String {
    if identical {
        if a == b {
            "identical: the two sources are byte-for-byte the same".to_string()
        } else {
            "formatting-only: same program, differs only in formatting/whitespace/comments"
                .to_string()
        }
    } else {
        format!(
            "changed: {added} line{} added, {removed} line{} removed (canonical form)",
            plural(added),
            plural(removed)
        )
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// One step of a line-level edit script.
enum Op {
    /// Line present in both (index in A, index in B).
    Eq(usize, usize),
    /// Line removed from A (index in A).
    Del(usize),
    /// Line added in B (index in B).
    Ins(usize),
}

/// Classic LCS (longest-common-subsequence) line diff. Deterministic and small
/// enough for source snippets; the caller bounds the table size.
fn diff_ops(a: &[&str], b: &[&str]) -> Vec<Op> {
    let n = a.len();
    let m = b.len();
    // dp[i][j] = LCS length of a[i..] and b[j..]. Row-major (n+1)·(m+1).
    let mut dp = vec![0u32; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[idx(i, j)] = if a[i] == b[j] {
                dp[idx(i + 1, j + 1)] + 1
            } else {
                dp[idx(i + 1, j)].max(dp[idx(i, j + 1)])
            };
        }
    }
    let mut ops = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if a[i] == b[j] {
            ops.push(Op::Eq(i, j));
            i += 1;
            j += 1;
        } else if dp[idx(i + 1, j)] >= dp[idx(i, j + 1)] {
            ops.push(Op::Del(i));
            i += 1;
        } else {
            ops.push(Op::Ins(j));
            j += 1;
        }
    }
    while i < n {
        ops.push(Op::Del(i));
        i += 1;
    }
    while j < m {
        ops.push(Op::Ins(j));
        j += 1;
    }
    ops
}

/// Render a git-style unified diff from the edit script, grouping changes into
/// `@@` hunks with up to `context` unchanged lines on each side.
fn render_unified(a: &[&str], b: &[&str], ops: &[Op], context: usize) -> String {
    // Mark which op indices are changes, then walk them into hunks.
    let is_change: Vec<bool> = ops
        .iter()
        .map(|o| !matches!(o, Op::Eq(..)))
        .collect();

    let mut out = String::from("--- a\n+++ b\n");
    let mut k = 0usize;
    while k < ops.len() {
        if !is_change[k] {
            k += 1;
            continue;
        }
        // Extend the hunk: start `context` equal lines before the first change,
        // and keep going while another change is within `context` equal lines.
        let mut start = k;
        let mut back = 0;
        while start > 0 && is_change[start - 1] == false && back < context {
            start -= 1;
            back += 1;
        }
        let mut end = k; // inclusive index of last op in the hunk
        let mut j = k;
        while j < ops.len() {
            if is_change[j] {
                end = j;
                j += 1;
            } else {
                // Look ahead: is there another change within `context` equals?
                let mut look = j;
                let mut gap = 0;
                while look < ops.len() && !is_change[look] && gap < context * 2 + 1 {
                    look += 1;
                    gap += 1;
                }
                if look < ops.len() && is_change[look] && gap <= context {
                    // Absorb the trailing context of this equal run into the hunk.
                    j = look;
                } else {
                    break;
                }
            }
        }
        // Add trailing context after the last change.
        let mut tail = end + 1;
        let mut fwd = 0;
        while tail < ops.len() && !is_change[tail] && fwd < context {
            tail += 1;
            fwd += 1;
        }
        let hunk_end = tail; // exclusive

        emit_hunk(&mut out, a, b, &ops[start..hunk_end]);
        k = hunk_end;
    }
    // Drop the trailing newline for a clean single-value output.
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn emit_hunk(out: &mut String, _a: &[&str], _b: &[&str], hunk: &[Op]) {
    // Compute the 1-based start line + length for each side.
    let mut a_start = None;
    let mut b_start = None;
    let mut a_len = 0usize;
    let mut b_len = 0usize;
    for op in hunk {
        match op {
            Op::Eq(i, j) => {
                a_start.get_or_insert(*i);
                b_start.get_or_insert(*j);
                a_len += 1;
                b_len += 1;
            }
            Op::Del(i) => {
                a_start.get_or_insert(*i);
                a_len += 1;
            }
            Op::Ins(j) => {
                b_start.get_or_insert(*j);
                b_len += 1;
            }
        }
    }
    let a_start = a_start.map(|x| x + 1).unwrap_or(0);
    let b_start = b_start.map(|x| x + 1).unwrap_or(0);
    out.push_str(&format!(
        "@@ -{a_start},{a_len} +{b_start},{b_len} @@\n"
    ));
    for op in hunk {
        match op {
            Op::Eq(i, _) => {
                out.push(' ');
                out.push_str(_a[*i]);
                out.push('\n');
            }
            Op::Del(i) => {
                out.push('-');
                out.push_str(_a[*i]);
                out.push('\n');
            }
            Op::Ins(j) => {
                out.push('+');
                out.push_str(_b[*j]);
                out.push('\n');
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_only_change_shows_no_diff() {
        let a = "fn main(){let x=1;println!(\"{}\",x);}";
        let b = "fn main() {\n    let x = 1;\n    println!(\"{}\", x);\n}\n";
        let out = diff(a, b, "unified", 3).unwrap();
        assert!(out.starts_with("No structural differences."), "got: {out}");
    }

    #[test]
    fn identical_sources_reported() {
        let a = "fn main() {}\n";
        assert_eq!(
            diff(a, a, "unified", 3).unwrap(),
            "The two sources are identical."
        );
    }

    #[test]
    fn real_change_shows_unified_diff() {
        let a = "fn answer() -> i32 { 1 }";
        let b = "fn answer() -> i32 { 2 }";
        let out = diff(a, b, "unified", 3).unwrap();
        assert!(out.contains("--- a\n+++ b\n"), "header missing: {out}");
        assert!(out.contains("-    1"), "removed line missing: {out}");
        assert!(out.contains("+    2"), "added line missing: {out}");
    }

    #[test]
    fn comment_only_change_is_invisible() {
        // Line comments live outside the AST, so adding one is not a difference.
        let a = "fn main() {}";
        let b = "// a leading comment\nfn main() {}";
        let out = diff(a, b, "summary", 3).unwrap();
        assert!(out.starts_with("formatting-only"), "got: {out}");
    }

    #[test]
    fn summary_counts_changes() {
        let a = "fn a() {}\nfn b() {}\n";
        let b = "fn a() {}\nfn c() {}\n";
        let out = diff(a, b, "summary", 3).unwrap();
        assert!(out.starts_with("changed:"), "got: {out}");
        assert!(out.contains("added"), "got: {out}");
        assert!(out.contains("removed"), "got: {out}");
    }

    #[test]
    fn canonical_mode_reformats() {
        let a = "fn main(){let x=1;}";
        let out = diff(a, "", "canonical-a", 3).unwrap();
        assert_eq!(out, "fn main() {\n    let x = 1;\n}\n");
    }

    #[test]
    fn invalid_rust_errors_with_location() {
        let err = diff("fn main( {", "fn main() {}", "unified", 3).unwrap_err();
        assert!(err.starts_with("source A: not valid Rust"), "got: {err}");
    }

    #[test]
    fn unknown_mode_rejected() {
        let err = diff("fn a(){}", "fn a(){}", "bogus", 3).unwrap_err();
        assert!(err.contains("unknown mode"), "got: {err}");
    }

    #[test]
    fn hunk_header_line_numbers() {
        let a = "fn a() {}\nfn b() {}\nfn c() {}\n";
        let b = "fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\n";
        let out = diff(a, b, "unified", 1).unwrap();
        assert!(out.contains("@@ "), "no hunk header: {out}");
        assert!(out.contains("+fn d() {}"), "added item missing: {out}");
    }
}
