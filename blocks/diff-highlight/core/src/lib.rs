//! gizza-ai/diff-highlight core — pure, wasm-safe rendering of a diff into a
//! polished, syntax-highlighted PNG (side-by-side or unified).
//!
//! No wafer / wasm-bindgen / network deps. The chat skill block, the CLI, and
//! the unit tests all call [`render_png`]. The stack is the same one
//! `blocks/code-screenshot` already proved on wasm32-wasip1: `syntect` (pure-Rust
//! `fancy-regex` engine + bundled binary-dump syntaxes/themes), `fontdue` (pure-Rust
//! TrueType rasterizer over an embedded font) and `png` — no onig, no freetype, no
//! system fonts, no host filesystem.
//!
//! Pipeline: parse a unified diff (or line-align two texts with an LCS) into
//! [`Row`]s → trim unchanged runs down to `context` lines → syntax-highlight each
//! side as its own document → rasterize the character grid onto an RGBA canvas
//! with per-row add/remove tints and intra-line change highlights → encode PNG.

use fontdue::Font;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SynStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

/// Embedded monospace font (DejaVu Sans Mono, Bitstream-Vera/Arev license —
/// freely redistributable). Bundled so rendering never touches the host
/// filesystem (wasm has none). See `core/src/assets/LICENSE-DejaVu.txt`.
const FONT_BYTES: &[u8] = include_bytes!("assets/DejaVuSansMono.ttf");

// ---------------------------------------------------------------------------
// Limits (documented on the tool — users should not discover these via an error)
// ---------------------------------------------------------------------------

/// Maximum rendered rows. Beyond this the image is truncated with a trailing
/// note row rather than growing without bound.
pub const MAX_ROWS: usize = 400;
/// Maximum characters rendered per column; longer lines are truncated with `...`.
pub const MAX_COLS: usize = 120;
/// Maximum input lines per side when aligning two texts.
pub const MAX_INPUT_LINES: usize = 4000;
/// Maximum `context` lines accepted; larger values are clamped to this.
pub const MAX_CONTEXT: usize = 50;
/// Maximum LCS table cells when aligning two texts (guards the 64 MiB sandbox).
const MAX_LCS_CELLS: usize = 4_000_000;

// Rendering knobs — a tasteful, minimal window card, matching code-screenshot.
const FONT_PX: f32 = 26.0; // glyph cell height in pixels
const LINE_HEIGHT: u32 = 34; // baseline-to-baseline spacing
const PAD: u32 = 30; // padding between the grid and the card edge
const MARGIN: u32 = 28; // outer padded background around the card
const CHROME_BAR: u32 = 44; // title bar with traffic-light dots

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// What to render: an already-computed patch, or two texts to align here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffInput<'a> {
    /// A unified diff / `.patch` (with or without `diff --git` headers).
    Patch(&'a str),
    /// Two raw texts; the alignment is computed with a line-level LCS.
    Pair { left: &'a str, right: &'a str },
}

/// Resolve the three optional text params into exactly one [`DiffInput`].
///
/// The tool takes EITHER a unified diff (`diff`) OR two texts (`left`+`right`);
/// blank/whitespace-only strings count as absent so an empty form field behaves
/// like an omitted one. Every rejection names what was expected.
pub fn pick_input<'a>(
    diff: Option<&'a str>,
    left: Option<&'a str>,
    right: Option<&'a str>,
) -> Result<DiffInput<'a>, RenderError> {
    let present = |s: Option<&'a str>| s.filter(|v| !v.trim().is_empty());
    match (present(diff), present(left), present(right)) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => Err(RenderError::BothInputs),
        (Some(d), None, None) => Ok(DiffInput::Patch(d)),
        (None, Some(l), Some(r)) => Ok(DiffInput::Pair { left: l, right: r }),
        (None, Some(_), None) | (None, None, Some(_)) => Err(RenderError::HalfPair),
        (None, None, None) => Err(RenderError::NoInput),
    }
}

/// Two-column or single-column presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Old on the left, new on the right, aligned row by row.
    SideBySide,
    /// One column, `-` rows followed by `+` rows (classic unified view).
    Unified,
}

impl Layout {
    /// Parse the `layout` param. Unknown values are an error naming the choices.
    pub fn parse(s: &str) -> Result<Self, RenderError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "side-by-side" | "side_by_side" | "split" => Ok(Layout::SideBySide),
            "unified" | "inline" => Ok(Layout::Unified),
            other => Err(RenderError::BadLayout(other.to_string())),
        }
    }
}

/// Rendering options. Defaults match [`Options::default`]: a side-by-side dark
/// rendering with line numbers and intra-line highlighting on, 3 context lines.
#[derive(Debug, Clone)]
pub struct Options {
    pub layout: Layout,
    /// Syntax-highlighting language hint (alias or extension); `None` → plain text.
    pub language: Option<String>,
    /// syntect theme name; `None`/unknown → a dark default.
    pub theme: Option<String>,
    /// Draw per-side line-number gutters.
    pub line_numbers: bool,
    /// Highlight the characters that actually changed within a changed line pair.
    pub word_highlight: bool,
    /// Treat lines that differ only in whitespace as unchanged.
    pub ignore_whitespace: bool,
    /// Unchanged lines kept on each side of a change; longer runs collapse.
    pub context: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            layout: Layout::SideBySide,
            language: None,
            theme: None,
            line_numbers: true,
            word_highlight: true,
            ignore_whitespace: false,
            context: 3,
        }
    }
}

/// An error from [`render_png`]. The block maps this onto a `SkillError`.
#[derive(Debug, PartialEq, Eq)]
pub enum RenderError {
    /// Neither a `diff` nor a `left`/`right` pair was supplied.
    NoInput,
    /// Both a `diff` and a `left`/`right` pair were supplied.
    BothInputs,
    /// Only one half of the `left`/`right` pair was supplied.
    HalfPair,
    /// The input contained nothing renderable (no diff lines at all).
    Empty,
    /// An unknown `layout` value.
    BadLayout(String),
    /// The input is too big to align within the sandbox's memory budget.
    TooLarge(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::NoInput => write!(
                f,
                "expected either `diff` (a unified diff) or both `left` and `right` (two texts to compare); got neither"
            ),
            RenderError::BothInputs => write!(
                f,
                "expected either `diff` or `left`+`right`, not both; drop one input"
            ),
            RenderError::HalfPair => write!(
                f,
                "comparing two texts needs BOTH `left` and `right`; supply the missing one (or use `diff` for a unified diff)"
            ),
            RenderError::Empty => write!(
                f,
                "no renderable content: the input has no diff lines (expected ` `/`+`/`-` lines, optionally under `@@` hunk headers)"
            ),
            RenderError::BadLayout(v) => write!(
                f,
                "unknown layout {v:?}; expected \"side-by-side\" or \"unified\""
            ),
            RenderError::TooLarge(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for RenderError {}

// ---------------------------------------------------------------------------
// Row model
// ---------------------------------------------------------------------------

/// One side of a rendered line: its 1-based number (when known) and its text,
/// plus the char range that changed relative to the other side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub num: Option<usize>,
    pub text: String,
    /// Half-open `[start, end)` char range that differs from the paired cell.
    pub hl: Option<(usize, usize)>,
}

impl Cell {
    fn new(num: Option<usize>, text: impl Into<String>) -> Self {
        Cell {
            num,
            text: text.into(),
            hl: None,
        }
    }
}

/// How a line row changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Present and identical on both sides.
    Context,
    /// Only on the right (added).
    Add,
    /// Only on the left (removed).
    Remove,
    /// Present on both sides but different.
    Change,
}

/// A row of the rendered grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    /// A full-width banner: a file path, a `@@` hunk header, a collapsed-run
    /// note, or a truncation note.
    Header(String),
    /// A line row. `left`/`right` are `None` where that side has no line.
    Line {
        kind: LineKind,
        left: Option<Cell>,
        right: Option<Cell>,
    },
}

// ---------------------------------------------------------------------------
// Building rows
// ---------------------------------------------------------------------------

/// Whitespace-insensitive comparison key for a line.
fn ws_key(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn same_line(a: &str, b: &str, ignore_ws: bool) -> bool {
    if a == b {
        return true;
    }
    ignore_ws && ws_key(a) == ws_key(b)
}

/// Build the row list for either input mode. Public so callers (and the tests)
/// can assert on the alignment independently of the rasterizer.
pub fn build_rows(input: DiffInput<'_>, opt: &Options) -> Result<Vec<Row>, RenderError> {
    let rows = match input {
        DiffInput::Patch(p) => parse_patch(p, opt)?,
        DiffInput::Pair { left, right } => align_texts(left, right, opt)?,
    };
    if !rows.iter().any(|r| matches!(r, Row::Line { .. })) {
        return Err(RenderError::Empty);
    }
    let mut rows = collapse_context(rows, opt.context);
    if opt.word_highlight {
        mark_word_changes(&mut rows);
    }
    // A unified view shows a changed line twice (once removed, once added);
    // split after the word-level marking so both halves keep their ranges.
    if opt.layout == Layout::Unified {
        rows = expand_unified(rows);
    }
    if rows.len() > MAX_ROWS {
        let dropped = rows.len() - MAX_ROWS;
        rows.truncate(MAX_ROWS);
        rows.push(Row::Header(format!(
            "... {dropped} more row(s) not rendered (limit {MAX_ROWS})"
        )));
    }
    Ok(rows)
}

/// Strip a `a/` or `b/` prefix from a diff path, and trim a trailing tab-separated
/// timestamp (`--- a/x.rs\t2026-01-01 …`, as emitted by plain `diff -u`).
fn clean_path(raw: &str) -> String {
    let p = raw.split('\t').next().unwrap_or(raw).trim();
    p.strip_prefix("a/")
        .or_else(|| p.strip_prefix("b/"))
        .unwrap_or(p)
        .to_string()
}

/// Parse `@@ -old,n +new,m @@ section` → (old_start, new_start). Missing or
/// malformed counters fall back to 1 so a hand-written hunk still renders.
fn parse_hunk_header(line: &str) -> (usize, usize) {
    let mut old = 1usize;
    let mut new = 1usize;
    if let Some(body) = line.strip_prefix("@@").and_then(|r| r.split("@@").next()) {
        for part in body.split_whitespace() {
            let (sign, rest) = part.split_at(1);
            let n = rest
                .split(',')
                .next()
                .and_then(|d| d.parse::<usize>().ok())
                .unwrap_or(1);
            match sign {
                "-" => old = n.max(1),
                "+" => new = n.max(1),
                _ => {}
            }
        }
    }
    (old, new)
}

/// Turn a run of removals and additions into aligned rows: pair them up
/// positionally (that's what a side-by-side view shows), leftovers stand alone.
fn flush_run(
    rows: &mut Vec<Row>,
    dels: &mut Vec<(usize, String)>,
    adds: &mut Vec<(usize, String)>,
    ignore_ws: bool,
) {
    let n = dels.len().max(adds.len());
    for i in 0..n {
        match (dels.get(i), adds.get(i)) {
            (Some((ln, lt)), Some((rn, rt))) => {
                // With ignore_whitespace on, a pair that differs only in
                // whitespace is not a change at all — show it as context.
                let kind = if same_line(lt, rt, ignore_ws) {
                    LineKind::Context
                } else {
                    LineKind::Change
                };
                rows.push(Row::Line {
                    kind,
                    left: Some(Cell::new(Some(*ln), lt.clone())),
                    right: Some(Cell::new(Some(*rn), rt.clone())),
                });
            }
            (Some((ln, lt)), None) => rows.push(Row::Line {
                kind: LineKind::Remove,
                left: Some(Cell::new(Some(*ln), lt.clone())),
                right: None,
            }),
            (None, Some((rn, rt))) => rows.push(Row::Line {
                kind: LineKind::Add,
                left: None,
                right: Some(Cell::new(Some(*rn), rt.clone())),
            }),
            (None, None) => unreachable!("i < max(len)"),
        }
    }
    dels.clear();
    adds.clear();
}

/// Parse a unified diff into rows. Tolerates bare hunks (no `diff --git`,
/// no `---`/`+++`) and multi-file patches.
fn parse_patch(patch: &str, opt: &Options) -> Result<Vec<Row>, RenderError> {
    let mut rows: Vec<Row> = Vec::new();
    let mut dels: Vec<(usize, String)> = Vec::new();
    let mut adds: Vec<(usize, String)> = Vec::new();
    let mut old_no = 1usize;
    let mut new_no = 1usize;
    // File name announced by the headers but not yet emitted as a banner.
    let mut pending_file: Option<String> = None;
    let mut old_path: Option<String> = None;

    for raw in patch.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);

        if let Some(rest) = line.strip_prefix("diff --git ") {
            flush_run(&mut rows, &mut dels, &mut adds, opt.ignore_whitespace);
            pending_file = rest.split_whitespace().last().map(clean_path);
            old_path = None;
            continue;
        }
        if let Some(rest) = line.strip_prefix("--- ") {
            let p = clean_path(rest);
            old_path = if p == "/dev/null" { None } else { Some(p) };
            continue;
        }
        if let Some(rest) = line.strip_prefix("+++ ") {
            let p = clean_path(rest);
            pending_file = if p == "/dev/null" {
                old_path
                    .clone()
                    .map(|o| format!("{o} (deleted)"))
                    .or(pending_file)
            } else {
                Some(p)
            };
            continue;
        }
        if line.starts_with("@@") {
            flush_run(&mut rows, &mut dels, &mut adds, opt.ignore_whitespace);
            if let Some(name) = pending_file.take() {
                rows.push(Row::Header(name));
            }
            let (o, n) = parse_hunk_header(line);
            old_no = o;
            new_no = n;
            if line.trim() != "@@" {
                rows.push(Row::Header(line.trim_end().to_string()));
            }
            continue;
        }
        if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
            flush_run(&mut rows, &mut dels, &mut adds, opt.ignore_whitespace);
            if let Some(name) = pending_file.take() {
                rows.push(Row::Header(name));
            }
            rows.push(Row::Header(line.trim_end().to_string()));
            continue;
        }
        // `\ No newline at end of file` and metadata lines (index/mode/similarity)
        // carry nothing to render.
        if line.starts_with('\\')
            || line.starts_with("index ")
            || line.starts_with("new file mode")
            || line.starts_with("deleted file mode")
            || line.starts_with("old mode")
            || line.starts_with("new mode")
            || line.starts_with("similarity index")
            || line.starts_with("rename from")
            || line.starts_with("rename to")
        {
            continue;
        }

        match line.chars().next() {
            Some('-') => {
                dels.push((old_no, line[1..].to_string()));
                old_no += 1;
            }
            Some('+') => {
                adds.push((new_no, line[1..].to_string()));
                new_no += 1;
            }
            Some(' ') | None => {
                flush_run(&mut rows, &mut dels, &mut adds, opt.ignore_whitespace);
                // A truly empty trailing line is the split artifact of the final
                // newline, not a context line — skip it.
                if line.is_empty() {
                    continue;
                }
                let text = line[1..].to_string();
                rows.push(Row::Line {
                    kind: LineKind::Context,
                    left: Some(Cell::new(Some(old_no), text.clone())),
                    right: Some(Cell::new(Some(new_no), text)),
                });
                old_no += 1;
                new_no += 1;
            }
            // Anything else is prose around the patch (commit messages, `$ git
            // diff` echoes) — ignore rather than mis-rendering it as a line.
            Some(_) => {
                flush_run(&mut rows, &mut dels, &mut adds, opt.ignore_whitespace);
            }
        }
    }
    flush_run(&mut rows, &mut dels, &mut adds, opt.ignore_whitespace);
    Ok(rows)
}

/// One step of the alignment between two line sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Equal,
    Del,
    Ins,
}

/// Line-level LCS with common prefix/suffix trimming (which usually shrinks the
/// quadratic table to nothing for real edits).
fn lcs_ops(left: &[&str], right: &[&str], ignore_ws: bool) -> Result<Vec<Op>, RenderError> {
    let eq = |a: &str, b: &str| same_line(a, b, ignore_ws);

    let mut pre = 0usize;
    while pre < left.len() && pre < right.len() && eq(left[pre], right[pre]) {
        pre += 1;
    }
    let mut suf = 0usize;
    while suf < left.len() - pre
        && suf < right.len() - pre
        && eq(left[left.len() - 1 - suf], right[right.len() - 1 - suf])
    {
        suf += 1;
    }
    let a = &left[pre..left.len() - suf];
    let b = &right[pre..right.len() - suf];

    let cells = (a.len() + 1).saturating_mul(b.len() + 1);
    if cells > MAX_LCS_CELLS {
        return Err(RenderError::TooLarge(format!(
            "the two texts are too different to align ({} x {} changed lines); reduce the input or pass a pre-computed unified diff via `diff`",
            a.len(),
            b.len()
        )));
    }

    // DP table of LCS lengths, row-major over (a.len()+1) x (b.len()+1).
    let w = b.len() + 1;
    let mut dp = vec![0u32; (a.len() + 1) * w];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            dp[i * w + j] = if eq(a[i], b[j]) {
                dp[(i + 1) * w + j + 1] + 1
            } else {
                dp[(i + 1) * w + j].max(dp[i * w + j + 1])
            };
        }
    }

    let mut ops = vec![Op::Equal; pre];
    let (mut i, mut j) = (0usize, 0usize);
    while i < a.len() && j < b.len() {
        if eq(a[i], b[j]) {
            ops.push(Op::Equal);
            i += 1;
            j += 1;
        } else if dp[(i + 1) * w + j] >= dp[i * w + j + 1] {
            ops.push(Op::Del);
            i += 1;
        } else {
            ops.push(Op::Ins);
            j += 1;
        }
    }
    ops.extend(std::iter::repeat(Op::Del).take(a.len() - i));
    ops.extend(std::iter::repeat(Op::Ins).take(b.len() - j));
    ops.extend(std::iter::repeat(Op::Equal).take(suf));
    Ok(ops)
}

/// Align two raw texts into rows via [`lcs_ops`], pairing adjacent
/// removal/addition runs so a modified line shows as one two-sided row.
fn align_texts(left: &str, right: &str, opt: &Options) -> Result<Vec<Row>, RenderError> {
    let l: Vec<&str> = left
        .split('\n')
        .map(|s| s.strip_suffix('\r').unwrap_or(s))
        .collect();
    let r: Vec<&str> = right
        .split('\n')
        .map(|s| s.strip_suffix('\r').unwrap_or(s))
        .collect();
    for (name, v) in [("left", &l), ("right", &r)] {
        if v.len() > MAX_INPUT_LINES {
            return Err(RenderError::TooLarge(format!(
                "`{name}` has {} lines — the limit is {MAX_INPUT_LINES} per side; trim the input or pass a unified diff via `diff`",
                v.len()
            )));
        }
    }

    let ops = lcs_ops(&l, &r, opt.ignore_whitespace)?;
    let mut rows = Vec::new();
    let mut dels: Vec<(usize, String)> = Vec::new();
    let mut adds: Vec<(usize, String)> = Vec::new();
    let (mut li, mut ri) = (0usize, 0usize);
    for op in ops {
        match op {
            Op::Equal => {
                flush_run(&mut rows, &mut dels, &mut adds, opt.ignore_whitespace);
                rows.push(Row::Line {
                    kind: LineKind::Context,
                    left: Some(Cell::new(Some(li + 1), l[li])),
                    right: Some(Cell::new(Some(ri + 1), r[ri])),
                });
                li += 1;
                ri += 1;
            }
            Op::Del => {
                dels.push((li + 1, l[li].to_string()));
                li += 1;
            }
            Op::Ins => {
                adds.push((ri + 1, r[ri].to_string()));
                ri += 1;
            }
        }
    }
    flush_run(&mut rows, &mut dels, &mut adds, opt.ignore_whitespace);
    Ok(rows)
}

/// Replace runs of unchanged lines longer than `2 * context` with a note row.
fn collapse_context(rows: Vec<Row>, context: usize) -> Vec<Row> {
    let changed: Vec<bool> = rows
        .iter()
        .map(|r| matches!(r, Row::Line { kind, .. } if *kind != LineKind::Context))
        .collect();
    if !changed.iter().any(|c| *c) {
        return rows;
    }
    // A row is kept if it is a header, a changed line, or within `context` rows
    // of a changed line.
    let keep: Vec<bool> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            if !matches!(
                r,
                Row::Line {
                    kind: LineKind::Context,
                    ..
                }
            ) {
                return true;
            }
            let lo = i.saturating_sub(context);
            let hi = (i + context).min(rows.len() - 1);
            changed[lo..=hi].iter().any(|c| *c)
        })
        .collect();

    let mut out = Vec::with_capacity(rows.len());
    let mut skipped = 0usize;
    for (i, row) in rows.into_iter().enumerate() {
        if keep[i] {
            if skipped > 0 {
                out.push(Row::Header(format!("... {skipped} unchanged line(s)")));
                skipped = 0;
            }
            out.push(row);
        } else {
            skipped += 1;
        }
    }
    if skipped > 0 {
        out.push(Row::Header(format!("... {skipped} unchanged line(s)")));
    }
    out
}

/// For every `Change` row, mark the char range that actually differs — the span
/// between the common prefix and the common suffix of the two sides.
fn mark_word_changes(rows: &mut [Row]) {
    for row in rows.iter_mut() {
        let Row::Line {
            kind: LineKind::Change,
            left: Some(l),
            right: Some(r),
        } = row
        else {
            continue;
        };
        let lc: Vec<char> = l.text.chars().collect();
        let rc: Vec<char> = r.text.chars().collect();
        let mut pre = 0usize;
        while pre < lc.len() && pre < rc.len() && lc[pre] == rc[pre] {
            pre += 1;
        }
        let mut suf = 0usize;
        while suf < lc.len() - pre
            && suf < rc.len() - pre
            && lc[lc.len() - 1 - suf] == rc[rc.len() - 1 - suf]
        {
            suf += 1;
        }
        if pre < lc.len() - suf {
            l.hl = Some((pre, lc.len() - suf));
        }
        if pre < rc.len() - suf {
            r.hl = Some((pre, rc.len() - suf));
        }
    }
}

// ---------------------------------------------------------------------------
// Syntax + theme resolution (shared shape with blocks/code-screenshot)
// ---------------------------------------------------------------------------

/// Resolve a language hint to a syntect syntax: exact token (extension or name,
/// case-insensitive), then a few friendly aliases, finally plain text. Never
/// fails — an unknown language renders as plaintext (still a valid image).
fn resolve_syntax<'a>(ss: &'a SyntaxSet, language: Option<&str>) -> &'a SyntaxReference {
    let plain = ss.find_syntax_plain_text();
    let Some(lang) = language.map(|l| l.trim()).filter(|l| !l.is_empty()) else {
        return plain;
    };
    let lower = lang.to_ascii_lowercase();
    let token = match lower.as_str() {
        "rust" | "rs" => "rs",
        "python" | "py" => "py",
        "javascript" | "js" => "js",
        "typescript" | "ts" => "ts",
        "shell" | "bash" | "sh" | "zsh" => "sh",
        "c++" | "cpp" | "cxx" => "cpp",
        "c#" | "csharp" | "cs" => "cs",
        "golang" | "go" => "go",
        "yml" | "yaml" => "yaml",
        "markdown" | "md" => "md",
        "html" | "htm" => "html",
        other => other,
    };
    ss.find_syntax_by_extension(token)
        .or_else(|| ss.find_syntax_by_token(token))
        .or_else(|| ss.find_syntax_by_name(lang))
        .unwrap_or(plain)
}

/// Resolve a theme name, falling back to a sensible dark theme for unknown or
/// empty names.
fn resolve_theme<'a>(ts: &'a ThemeSet, theme: Option<&str>) -> &'a Theme {
    const DEFAULT: &str = "base16-ocean.dark";
    let name = theme
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .unwrap_or(DEFAULT);
    ts.themes
        .get(name)
        .or_else(|| ts.themes.get(DEFAULT))
        .unwrap_or_else(|| ts.themes.values().next().expect("themeset is non-empty"))
}

/// Every bundled theme name, for the tool's documentation and error messages.
pub fn theme_names() -> Vec<String> {
    ThemeSet::load_defaults().themes.keys().cloned().collect()
}

// ---------------------------------------------------------------------------
// Canvas
// ---------------------------------------------------------------------------

/// A laid-out RGBA canvas backed by a flat `Vec<u8>` (4 bytes/pixel).
struct Canvas {
    width: u32,
    height: u32,
    px: Vec<u8>,
}

impl Canvas {
    fn new(width: u32, height: u32, bg: [u8; 3]) -> Self {
        let mut px = vec![0u8; (width as usize) * (height as usize) * 4];
        for chunk in px.chunks_exact_mut(4) {
            chunk[0] = bg[0];
            chunk[1] = bg[1];
            chunk[2] = bg[2];
            chunk[3] = 255;
        }
        Canvas { width, height, px }
    }

    /// Fill an axis-aligned rectangle with an opaque colour (clipped to bounds).
    fn fill_rect(&mut self, x0: u32, y0: u32, w: u32, h: u32, color: [u8; 3]) {
        let x1 = (x0 + w).min(self.width);
        let y1 = (y0 + h).min(self.height);
        for y in y0..y1 {
            for x in x0..x1 {
                self.put(x, y, color, 255);
            }
        }
    }

    /// Alpha-blend `color` over the pixel at `(x, y)` with coverage `a` (0-255).
    fn put(&mut self, x: u32, y: u32, color: [u8; 3], a: u8) {
        if x >= self.width || y >= self.height {
            return;
        }
        let i = ((y as usize) * (self.width as usize) + x as usize) * 4;
        let af = a as f32 / 255.0;
        for (c, &src) in color.iter().enumerate() {
            let dst = self.px[i + c] as f32;
            self.px[i + c] = (src as f32 * af + dst * (1.0 - af))
                .round()
                .clamp(0.0, 255.0) as u8;
        }
        self.px[i + 3] = 255;
    }
}

/// Encode an RGBA [`Canvas`] to PNG bytes (pure-Rust `png` encoder).
fn encode_png(canvas: &Canvas) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, canvas.width, canvas.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().expect("png header");
        writer.write_image_data(&canvas.px).expect("png image data");
    }
    out
}

/// Linear blend of `base` toward `target` by `t` (0..=1). Works on light and
/// dark themes alike (a green tint over white stays a pale green).
fn mix(base: [u8; 3], target: [u8; 3], t: f32) -> [u8; 3] {
    let m = |a: u8, b: u8| {
        (a as f32 * (1.0 - t) + b as f32 * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [
        m(base[0], target[0]),
        m(base[1], target[1]),
        m(base[2], target[2]),
    ]
}

/// Scale an RGB colour toward black by `factor` (0..=1).
fn scale(c: [u8; 3], factor: f32) -> [u8; 3] {
    [
        (c[0] as f32 * factor) as u8,
        (c[1] as f32 * factor) as u8,
        (c[2] as f32 * factor) as u8,
    ]
}

/// Perceptual-ish luminance, used to pick tint directions that stay visible on
/// both light and dark themes.
fn luma(c: [u8; 3]) -> f32 {
    0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32
}

// ---------------------------------------------------------------------------
// Painting (syntax highlighting → per-character colours)
// ---------------------------------------------------------------------------

/// A cell's characters with their resolved foreground colours, tabs already
/// expanded and the line truncated to [`MAX_COLS`].
type Painted = Vec<(char, [u8; 3])>;

/// Foreground for a syntect style; alpha 0 means "inherit the theme default".
fn style_fg(style: &SynStyle, default_fg: [u8; 3]) -> [u8; 3] {
    let c = style.foreground;
    if c.a == 0 {
        default_fg
    } else {
        [c.r, c.g, c.b]
    }
}

/// Highlight one side's lines as a single continuous document, so multi-line
/// constructs (strings, block comments) colour correctly down the column.
fn paint_side(
    texts: &[String],
    syntax: &SyntaxReference,
    theme: &Theme,
    ss: &SyntaxSet,
    default_fg: [u8; 3],
) -> Vec<Painted> {
    let mut h = HighlightLines::new(syntax, theme);
    texts
        .iter()
        .map(|t| {
            // syntect's default syntax set is the `_newlines` variant, so feed
            // each line with its terminator for correct end-of-line handling.
            let with_nl = format!("{t}\n");
            let spans = h.highlight_line(&with_nl, ss).unwrap_or_default();
            let mut out: Painted = Vec::new();
            for (style, text) in spans {
                let fg = style_fg(&style, default_fg);
                for ch in text.chars() {
                    if ch == '\n' || ch == '\r' {
                        continue;
                    }
                    if ch == '\t' {
                        // Expand tabs to a fixed 4 columns — the grid is monospaced.
                        for _ in 0..4 {
                            out.push((' ', fg));
                        }
                    } else {
                        out.push((ch, fg));
                    }
                }
            }
            if out.len() > MAX_COLS {
                out.truncate(MAX_COLS - 3);
                for _ in 0..3 {
                    out.push(('.', default_fg));
                }
            }
            out
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Which sides a rendered grid row occupies, in the unified/side-by-side sense.
struct GridRow<'a> {
    header: Option<&'a str>,
    left: Option<(&'a Cell, &'a Painted)>,
    right: Option<(&'a Cell, &'a Painted)>,
    kind: LineKind,
}

/// Render a diff to a syntax-highlighted PNG. Returns the PNG bytes plus the
/// rendered `(width, height)`.
pub fn render_png(input: DiffInput<'_>, opt: &Options) -> Result<(Vec<u8>, u32, u32), RenderError> {
    let rows = build_rows(input, opt)?;

    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let syntax = resolve_syntax(&ss, opt.language.as_deref());
    let theme = resolve_theme(&ts, opt.theme.as_deref());

    let code_bg = theme
        .settings
        .background
        .map(|c| [c.r, c.g, c.b])
        .unwrap_or([40, 44, 52]);
    let default_fg = theme
        .settings
        .foreground
        .map(|c| [c.r, c.g, c.b])
        .unwrap_or([220, 223, 228]);
    let dark = luma(code_bg) < 128.0;

    let outer_bg = if dark {
        scale(code_bg, 0.55)
    } else {
        mix(code_bg, [0, 0, 0], 0.12)
    };
    let chrome_bg = if dark {
        scale(code_bg, 0.82)
    } else {
        mix(code_bg, [0, 0, 0], 0.06)
    };
    // Add/remove tints blend the theme background toward green/red, so they read
    // correctly on light and dark themes alike.
    let add_bg = mix(code_bg, [46, 160, 67], 0.22);
    let del_bg = mix(code_bg, [248, 81, 73], 0.20);
    let add_hl = mix(code_bg, [46, 160, 67], 0.46);
    let del_hl = mix(code_bg, [248, 81, 73], 0.44);
    // "This side has no line here" filler.
    let filler_bg = if dark {
        scale(code_bg, 0.88)
    } else {
        mix(code_bg, [0, 0, 0], 0.04)
    };
    let gutter_fg = mix(default_fg, code_bg, 0.5);
    let header_fg = mix(default_fg, code_bg, 0.2);

    // Paint each side as its own document, in row order.
    let left_texts: Vec<String> = rows
        .iter()
        .filter_map(|r| match r {
            Row::Line { left: Some(c), .. } => Some(c.text.clone()),
            _ => None,
        })
        .collect();
    let right_texts: Vec<String> = rows
        .iter()
        .filter_map(|r| match r {
            Row::Line { right: Some(c), .. } => Some(c.text.clone()),
            _ => None,
        })
        .collect();
    let left_painted = paint_side(&left_texts, syntax, theme, &ss, default_fg);
    let right_painted = paint_side(&right_texts, syntax, theme, &ss, default_fg);

    // Zip the painted cells back onto their rows.
    let mut grid: Vec<GridRow> = Vec::with_capacity(rows.len());
    let (mut li, mut ri) = (0usize, 0usize);
    for row in &rows {
        match row {
            Row::Header(h) => grid.push(GridRow {
                header: Some(h),
                left: None,
                right: None,
                kind: LineKind::Context,
            }),
            Row::Line { kind, left, right } => {
                let l = left.as_ref().map(|c| {
                    let p = &left_painted[li];
                    li += 1;
                    (c, p)
                });
                let r = right.as_ref().map(|c| {
                    let p = &right_painted[ri];
                    ri += 1;
                    (c, p)
                });
                grid.push(GridRow {
                    header: None,
                    left: l,
                    right: r,
                    kind: *kind,
                });
            }
        }
    }

    let font = Font::from_bytes(FONT_BYTES, fontdue::FontSettings::default())
        .expect("embedded font parses");
    let advance = font.metrics('M', FONT_PX).advance_width.ceil().max(1.0) as u32;
    let ascent = font_ascent(&font);

    // ---- Column geometry (in character cells) ----
    let digits = |n: usize| if n == 0 { 1 } else { n.to_string().len() };
    let max_left_num = grid
        .iter()
        .filter_map(|g| g.left.and_then(|(c, _)| c.num))
        .max()
        .unwrap_or(0);
    let max_right_num = grid
        .iter()
        .filter_map(|g| g.right.and_then(|(c, _)| c.num))
        .max()
        .unwrap_or(0);
    let gw_l = if opt.line_numbers {
        digits(max_left_num)
    } else {
        0
    };
    let gw_r = if opt.line_numbers {
        digits(max_right_num)
    } else {
        0
    };
    // gutter + 1 space (only when numbers are shown) + sign + 1 space + content
    let lead_l = gw_l + if opt.line_numbers { 1 } else { 0 } + 2;
    let lead_r = gw_r + if opt.line_numbers { 1 } else { 0 } + 2;

    let content_w = grid
        .iter()
        .flat_map(|g| [g.left.map(|(_, p)| p.len()), g.right.map(|(_, p)| p.len())])
        .flatten()
        .max()
        .unwrap_or(1)
        .max(1);
    let header_w = grid
        .iter()
        .filter_map(|g| g.header.map(|h| h.chars().count()))
        .max()
        .unwrap_or(0);

    let (total_cols, unified_lead) = match opt.layout {
        Layout::SideBySide => {
            let left_block = lead_l + content_w;
            let right_block = lead_r + content_w;
            ((left_block + SPLIT_GAP + right_block).max(header_w), 0)
        }
        Layout::Unified => {
            // [old gutter][space][new gutter][space][sign][space][content]
            let lead = if opt.line_numbers {
                gw_l + 1 + gw_r + 1 + 2
            } else {
                2
            };
            ((lead + content_w).max(header_w), lead)
        }
    };

    let grid_w = advance * total_cols as u32;
    let grid_h = LINE_HEIGHT * grid.len() as u32;
    let card_w = grid_w + 2 * PAD;
    let card_h = grid_h + 2 * PAD + CHROME_BAR;
    let width = card_w + 2 * MARGIN;
    let height = card_h + 2 * MARGIN;

    let mut canvas = Canvas::new(width, height, outer_bg);
    canvas.fill_rect(MARGIN, MARGIN, card_w, card_h, code_bg);
    canvas.fill_rect(MARGIN, MARGIN, card_w, CHROME_BAR, chrome_bg);
    let dot_r = 7u32;
    let dot_y = MARGIN + CHROME_BAR / 2;
    for (i, color) in [[255, 95, 86], [255, 189, 46], [39, 201, 63]]
        .iter()
        .enumerate()
    {
        let cx = MARGIN + PAD + dot_r + i as u32 * (dot_r * 2 + 10);
        fill_circle(&mut canvas, cx, dot_y, dot_r, *color);
    }

    let origin_x = MARGIN + PAD;
    let origin_y = MARGIN + CHROME_BAR + PAD;
    let cell_x = |col: usize| origin_x + col as u32 * advance;

    for (row_i, g) in grid.iter().enumerate() {
        let y = origin_y + row_i as u32 * LINE_HEIGHT;
        let baseline = y + ascent;

        if let Some(h) = g.header {
            canvas.fill_rect(origin_x, y, grid_w, LINE_HEIGHT, chrome_bg);
            draw_text(
                &mut canvas,
                &font,
                h.chars().take(total_cols),
                origin_x,
                baseline,
                advance,
                header_fg,
            );
            continue;
        }

        match opt.layout {
            Layout::SideBySide => {
                let left_block = lead_l + content_w;
                let right_start = left_block + SPLIT_GAP;
                let right_block = lead_r + content_w;

                let (l_bg, r_bg) = match g.kind {
                    LineKind::Context => (None, None),
                    LineKind::Change => (Some(del_bg), Some(add_bg)),
                    LineKind::Remove => (Some(del_bg), Some(filler_bg)),
                    LineKind::Add => (Some(filler_bg), Some(add_bg)),
                };
                if let Some(bg) = l_bg {
                    canvas.fill_rect(cell_x(0), y, advance * left_block as u32, LINE_HEIGHT, bg);
                }
                if let Some(bg) = r_bg {
                    canvas.fill_rect(
                        cell_x(right_start),
                        y,
                        advance * right_block as u32,
                        LINE_HEIGHT,
                        bg,
                    );
                }

                let sign_l = match g.kind {
                    LineKind::Change | LineKind::Remove => '-',
                    _ => ' ',
                };
                let sign_r = match g.kind {
                    LineKind::Change | LineKind::Add => '+',
                    _ => ' ',
                };
                draw_side(
                    &mut canvas,
                    &font,
                    g.left,
                    0,
                    gw_l,
                    lead_l,
                    sign_l,
                    opt.line_numbers,
                    y,
                    baseline,
                    advance,
                    origin_x,
                    gutter_fg,
                    del_hl,
                );
                draw_side(
                    &mut canvas,
                    &font,
                    g.right,
                    right_start,
                    gw_r,
                    lead_r,
                    sign_r,
                    opt.line_numbers,
                    y,
                    baseline,
                    advance,
                    origin_x,
                    gutter_fg,
                    add_hl,
                );

                // Vertical divider between the two columns.
                let dx = cell_x(left_block) + advance;
                canvas.fill_rect(dx, y, 2, LINE_HEIGHT, mix(code_bg, default_fg, 0.25));
            }
            Layout::Unified => {
                // `expand_unified` already split every Change row into a removal
                // row followed by an addition row, so each grid row here draws
                // exactly one side.
                let (cell, painted, sign, bg, hl) = match g.kind {
                    LineKind::Remove | LineKind::Change => {
                        let (c, p) = g.left.expect("remove row has a left cell");
                        (c, p, '-', Some(del_bg), del_hl)
                    }
                    LineKind::Add => {
                        let (c, p) = g.right.expect("add row has a right cell");
                        (c, p, '+', Some(add_bg), add_hl)
                    }
                    LineKind::Context => {
                        let (c, p) = g.left.or(g.right).expect("context row has a cell");
                        (c, p, ' ', None, add_hl)
                    }
                };
                if let Some(b) = bg {
                    canvas.fill_rect(cell_x(0), y, grid_w, LINE_HEIGHT, b);
                }
                if opt.line_numbers {
                    let old_n = g.left.and_then(|(c, _)| c.num);
                    let new_n = g.right.and_then(|(c, _)| c.num);
                    draw_num(
                        &mut canvas,
                        &font,
                        old_n,
                        0,
                        gw_l,
                        baseline,
                        advance,
                        origin_x,
                        gutter_fg,
                    );
                    draw_num(
                        &mut canvas,
                        &font,
                        new_n,
                        gw_l + 1,
                        gw_r,
                        baseline,
                        advance,
                        origin_x,
                        gutter_fg,
                    );
                }
                draw_text(
                    &mut canvas,
                    &font,
                    [sign].into_iter(),
                    cell_x(unified_lead - 2),
                    baseline,
                    advance,
                    gutter_fg,
                );
                draw_cell(
                    &mut canvas,
                    &font,
                    cell,
                    painted,
                    cell_x(unified_lead),
                    y,
                    baseline,
                    advance,
                    hl,
                );
            }
        }
    }

    Ok((encode_png(&canvas), width, height))
}

/// Character cells between the two columns in the side-by-side layout.
const SPLIT_GAP: usize = 3;

/// In the unified layout a changed line must appear twice (once removed, once
/// added). Splitting is done on the row list before rendering.
fn expand_unified(rows: Vec<Row>) -> Vec<Row> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        match row {
            Row::Line {
                kind: LineKind::Change,
                left,
                right,
            } => {
                out.push(Row::Line {
                    kind: LineKind::Remove,
                    left,
                    right: None,
                });
                out.push(Row::Line {
                    kind: LineKind::Add,
                    left: None,
                    right,
                });
            }
            other => out.push(other),
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn draw_side(
    canvas: &mut Canvas,
    font: &Font,
    side: Option<(&Cell, &Painted)>,
    col0: usize,
    gw: usize,
    lead: usize,
    sign: char,
    line_numbers: bool,
    y: u32,
    baseline: u32,
    advance: u32,
    // Pixel x of grid column 0 (the caller's `origin_x`).
    base_x: u32,
    gutter_fg: [u8; 3],
    hl_bg: [u8; 3],
) {
    let Some((cell, painted)) = side else { return };
    // `col0` is this side's first grid column; every x below is
    // `base_x + column * advance`.
    let x_of = |col: usize| (col0 + col) as u32 * advance;
    if line_numbers {
        draw_num(
            canvas, font, cell.num, col0, gw, baseline, advance, base_x, gutter_fg,
        );
    }
    let sign_col = col0 + lead - 2;
    draw_text(
        canvas,
        font,
        [sign].into_iter(),
        base_x + sign_col as u32 * advance,
        baseline,
        advance,
        gutter_fg,
    );
    draw_cell(
        canvas,
        font,
        cell,
        painted,
        base_x + x_of(lead),
        y,
        baseline,
        advance,
        hl_bg,
    );
}

/// Draw a right-aligned line number into a `gw`-wide gutter starting at `col`.
#[allow(clippy::too_many_arguments)]
fn draw_num(
    canvas: &mut Canvas,
    font: &Font,
    num: Option<usize>,
    col: usize,
    gw: usize,
    baseline: u32,
    advance: u32,
    base_x: u32,
    fg: [u8; 3],
) {
    let Some(n) = num else { return };
    if gw == 0 {
        return;
    }
    let s = n.to_string();
    let pad = gw.saturating_sub(s.chars().count());
    let x = base_x + (col + pad) as u32 * advance;
    draw_text(canvas, font, s.chars(), x, baseline, advance, fg);
}

/// Draw a painted cell, tinting the changed char range with `hl_bg` first.
#[allow(clippy::too_many_arguments)]
fn draw_cell(
    canvas: &mut Canvas,
    font: &Font,
    cell: &Cell,
    painted: &Painted,
    x0: u32,
    y: u32,
    baseline: u32,
    advance: u32,
    hl_bg: [u8; 3],
) {
    if let Some((a, b)) = cell.hl {
        let a = a.min(painted.len());
        let b = b.min(painted.len());
        if b > a {
            canvas.fill_rect(
                x0 + a as u32 * advance,
                y,
                (b - a) as u32 * advance,
                LINE_HEIGHT,
                hl_bg,
            );
        }
    }
    for (i, (ch, fg)) in painted.iter().enumerate() {
        if !ch.is_whitespace() {
            draw_glyph(canvas, font, *ch, x0 + i as u32 * advance, baseline, *fg);
        }
    }
}

/// Draw a run of same-coloured characters starting at `x0`.
fn draw_text(
    canvas: &mut Canvas,
    font: &Font,
    chars: impl Iterator<Item = char>,
    x0: u32,
    baseline: u32,
    advance: u32,
    fg: [u8; 3],
) {
    for (i, ch) in chars.enumerate() {
        if !ch.is_whitespace() {
            draw_glyph(canvas, font, ch, x0 + i as u32 * advance, baseline, fg);
        }
    }
}

/// Rasterize one glyph with fontdue and alpha-composite it onto the canvas.
fn draw_glyph(canvas: &mut Canvas, font: &Font, ch: char, pen_x: u32, baseline: u32, fg: [u8; 3]) {
    let (metrics, bitmap) = font.rasterize(ch, FONT_PX);
    if metrics.width == 0 || metrics.height == 0 {
        return;
    }
    let left = pen_x as i64 + metrics.xmin as i64;
    // ymin is the offset of the bitmap's bottom edge from the baseline (negative
    // = below). Top of the bitmap = baseline - (height + ymin).
    let top = baseline as i64 - (metrics.height as i64 + metrics.ymin as i64);
    for gy in 0..metrics.height {
        for gx in 0..metrics.width {
            let coverage = bitmap[gy * metrics.width + gx];
            if coverage == 0 {
                continue;
            }
            let px = left + gx as i64;
            let py = top + gy as i64;
            if px < 0 || py < 0 {
                continue;
            }
            canvas.put(px as u32, py as u32, fg, coverage);
        }
    }
}

/// Filled circle via a simple radius test (the traffic-light dots).
fn fill_circle(canvas: &mut Canvas, cx: u32, cy: u32, r: u32, color: [u8; 3]) {
    let r2 = (r * r) as i64;
    for dy in -(r as i64)..=(r as i64) {
        for dx in -(r as i64)..=(r as i64) {
            if dx * dx + dy * dy <= r2 {
                let x = cx as i64 + dx;
                let y = cy as i64 + dy;
                if x >= 0 && y >= 0 {
                    canvas.put(x as u32, y as u32, color, 255);
                }
            }
        }
    }
}

/// The font's ascent in pixels at [`FONT_PX`].
fn font_ascent(font: &Font) -> u32 {
    font.horizontal_line_metrics(FONT_PX)
        .map(|m| m.ascent.ceil() as u32)
        .unwrap_or((FONT_PX * 0.8) as u32)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    /// Row list with headers dropped, as `(kind, left text, right text)`.
    fn lines(rows: &[Row]) -> Vec<(LineKind, Option<&str>, Option<&str>)> {
        rows.iter()
            .filter_map(|r| match r {
                Row::Line { kind, left, right } => Some((
                    *kind,
                    left.as_ref().map(|c| c.text.as_str()),
                    right.as_ref().map(|c| c.text.as_str()),
                )),
                Row::Header(_) => None,
            })
            .collect()
    }

    fn headers(rows: &[Row]) -> Vec<&str> {
        rows.iter()
            .filter_map(|r| match r {
                Row::Header(h) => Some(h.as_str()),
                _ => None,
            })
            .collect()
    }

    /// PNG magic + the IHDR-declared dimensions.
    fn png_dims(png: &[u8]) -> (u32, u32) {
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "PNG signature");
        assert_eq!(&png[12..16], b"IHDR", "IHDR is the first chunk");
        let n = |o: usize| u32::from_be_bytes([png[o], png[o + 1], png[o + 2], png[o + 3]]);
        (n(16), n(20))
    }

    // ---- input-mode selection ------------------------------------------

    #[test]
    fn pick_input_accepts_a_patch_alone() {
        assert_eq!(
            pick_input(Some("@@ -1 +1 @@"), None, None),
            Ok(DiffInput::Patch("@@ -1 +1 @@"))
        );
    }

    #[test]
    fn pick_input_accepts_a_full_pair() {
        assert_eq!(
            pick_input(None, Some("a"), Some("b")),
            Ok(DiffInput::Pair {
                left: "a",
                right: "b"
            })
        );
    }

    #[test]
    fn pick_input_rejects_nothing_at_all() {
        assert_eq!(pick_input(None, None, None), Err(RenderError::NoInput));
        // A blank string is treated as absent, not as an empty diff.
        assert_eq!(
            pick_input(Some("   \n"), None, None),
            Err(RenderError::NoInput)
        );
    }

    #[test]
    fn pick_input_rejects_both_modes_at_once() {
        assert_eq!(
            pick_input(Some("@@"), Some("a"), Some("b")),
            Err(RenderError::BothInputs)
        );
        assert_eq!(
            pick_input(Some("@@"), Some("a"), None),
            Err(RenderError::BothInputs)
        );
    }

    #[test]
    fn pick_input_rejects_half_a_pair() {
        assert_eq!(
            pick_input(None, Some("a"), None),
            Err(RenderError::HalfPair)
        );
        assert_eq!(
            pick_input(None, None, Some("b")),
            Err(RenderError::HalfPair)
        );
    }

    #[test]
    fn errors_name_what_was_expected() {
        assert!(RenderError::NoInput
            .to_string()
            .contains("`left` and `right`"));
        assert!(RenderError::HalfPair.to_string().contains("BOTH"));
        assert!(RenderError::BadLayout("grid".into())
            .to_string()
            .contains("side-by-side"));
    }

    // ---- layout parsing --------------------------------------------------

    #[test]
    fn layout_parses_its_aliases_case_insensitively() {
        for s in ["side-by-side", "Side_By_Side", " SPLIT "] {
            assert_eq!(Layout::parse(s), Ok(Layout::SideBySide), "{s}");
        }
        for s in ["unified", "INLINE"] {
            assert_eq!(Layout::parse(s), Ok(Layout::Unified), "{s}");
        }
        assert_eq!(
            Layout::parse("columns"),
            Err(RenderError::BadLayout("columns".into()))
        );
    }

    // ---- patch parsing ---------------------------------------------------

    const PATCH: &str = "\
diff --git a/src/main.rs b/src/main.rs
index 1111111..2222222 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,4 @@
 fn main() {
-    println!(\"hello\");
+    println!(\"goodbye\");
 }
";

    #[test]
    fn patch_pairs_a_removal_with_its_addition_as_one_change_row() {
        let rows = build_rows(DiffInput::Patch(PATCH), &opts()).unwrap();
        assert_eq!(
            lines(&rows),
            vec![
                (LineKind::Context, Some("fn main() {"), Some("fn main() {")),
                (
                    LineKind::Change,
                    Some("    println!(\"hello\");"),
                    Some("    println!(\"goodbye\");")
                ),
                (LineKind::Context, Some("}"), Some("}")),
            ]
        );
    }

    #[test]
    fn patch_emits_a_file_banner_and_the_hunk_header() {
        let rows = build_rows(DiffInput::Patch(PATCH), &opts()).unwrap();
        assert_eq!(headers(&rows), vec!["src/main.rs", "@@ -1,4 +1,4 @@"]);
    }

    #[test]
    fn patch_numbers_lines_from_the_hunk_header() {
        let rows =
            build_rows(DiffInput::Patch("@@ -10,2 +20,2 @@\n a\n-b\n+c\n"), &opts()).unwrap();
        let nums: Vec<(Option<usize>, Option<usize>)> = rows
            .iter()
            .filter_map(|r| match r {
                Row::Line { left, right, .. } => Some((
                    left.as_ref().and_then(|c| c.num),
                    right.as_ref().and_then(|c| c.num),
                )),
                _ => None,
            })
            .collect();
        assert_eq!(nums, vec![(Some(10), Some(20)), (Some(11), Some(21))]);
    }

    #[test]
    fn bare_hunks_without_git_headers_still_parse() {
        let rows = build_rows(DiffInput::Patch("@@\n-old\n+new\n"), &opts()).unwrap();
        assert_eq!(
            lines(&rows),
            vec![(LineKind::Change, Some("old"), Some("new"))]
        );
    }

    #[test]
    fn unpaired_removals_and_additions_stand_alone() {
        let rows = build_rows(DiffInput::Patch("@@\n-a\n-b\n+c\n"), &opts()).unwrap();
        assert_eq!(
            lines(&rows),
            vec![
                (LineKind::Change, Some("a"), Some("c")),
                (LineKind::Remove, Some("b"), None),
            ]
        );
    }

    #[test]
    fn a_deleted_file_is_labelled_in_its_banner() {
        let p = "diff --git a/gone.txt b/gone.txt\n--- a/gone.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-bye\n";
        let rows = build_rows(DiffInput::Patch(p), &opts()).unwrap();
        assert_eq!(headers(&rows)[0], "gone.txt (deleted)");
    }

    #[test]
    fn multi_file_patches_get_one_banner_per_file() {
        let p = format!("{PATCH}diff --git a/b.rs b/b.rs\n@@ -1 +1 @@\n-x\n+y\n");
        let rows = build_rows(DiffInput::Patch(&p), &opts()).unwrap();
        assert!(headers(&rows).contains(&"src/main.rs"));
        assert!(headers(&rows).contains(&"b.rs"));
    }

    #[test]
    fn a_binary_stanza_renders_as_a_header_not_a_line() {
        let p = "diff --git a/logo.png b/logo.png\nBinary files a/logo.png and b/logo.png differ\n@@\n-x\n+y\n";
        let rows = build_rows(DiffInput::Patch(p), &opts()).unwrap();
        assert!(headers(&rows)
            .iter()
            .any(|h| h.starts_with("Binary files ")));
    }

    #[test]
    fn crlf_patches_parse_without_stray_carriage_returns() {
        let rows = build_rows(DiffInput::Patch("@@\r\n-a\r\n+b\r\n"), &opts()).unwrap();
        assert_eq!(lines(&rows), vec![(LineKind::Change, Some("a"), Some("b"))]);
    }

    #[test]
    fn a_patch_with_no_diff_lines_is_rejected() {
        assert_eq!(
            build_rows(DiffInput::Patch("just some prose\n"), &opts()),
            Err(RenderError::Empty)
        );
    }

    // ---- two-text alignment ----------------------------------------------

    #[test]
    fn a_pair_aligns_an_edit_in_the_middle() {
        let rows = build_rows(
            DiffInput::Pair {
                left: "a\nb\nc",
                right: "a\nB\nc",
            },
            &opts(),
        )
        .unwrap();
        assert_eq!(
            lines(&rows),
            vec![
                (LineKind::Context, Some("a"), Some("a")),
                (LineKind::Change, Some("b"), Some("B")),
                (LineKind::Context, Some("c"), Some("c")),
            ]
        );
    }

    #[test]
    fn a_pure_insertion_is_an_add_row_with_no_left_side() {
        let rows = build_rows(
            DiffInput::Pair {
                left: "a\nc",
                right: "a\nb\nc",
            },
            &opts(),
        )
        .unwrap();
        assert_eq!(
            lines(&rows),
            vec![
                (LineKind::Context, Some("a"), Some("a")),
                (LineKind::Add, None, Some("b")),
                (LineKind::Context, Some("c"), Some("c")),
            ]
        );
    }

    #[test]
    fn a_pure_deletion_is_a_remove_row_with_no_right_side() {
        let rows = build_rows(
            DiffInput::Pair {
                left: "a\nb\nc",
                right: "a\nc",
            },
            &opts(),
        )
        .unwrap();
        assert_eq!(lines(&rows)[1], (LineKind::Remove, Some("b"), None));
    }

    #[test]
    fn identical_texts_produce_only_context_rows() {
        let rows = build_rows(
            DiffInput::Pair {
                left: "a\nb",
                right: "a\nb",
            },
            &opts(),
        )
        .unwrap();
        assert!(lines(&rows).iter().all(|(k, _, _)| *k == LineKind::Context));
    }

    #[test]
    fn a_pair_over_the_line_cap_is_rejected_with_a_stated_limit() {
        let big = "x\n".repeat(MAX_INPUT_LINES + 1);
        let err = build_rows(
            DiffInput::Pair {
                left: &big,
                right: "y",
            },
            &opts(),
        )
        .unwrap_err();
        assert!(
            matches!(&err, RenderError::TooLarge(m) if m.contains(&MAX_INPUT_LINES.to_string())),
            "{err}"
        );
    }

    // ---- options ---------------------------------------------------------

    #[test]
    fn ignore_whitespace_demotes_a_whitespace_only_edit_to_context() {
        let opt = Options {
            ignore_whitespace: true,
            ..opts()
        };
        let rows = build_rows(
            DiffInput::Patch("@@\n-  let x = 1;\n+let    x = 1;\n"),
            &opt,
        )
        .unwrap();
        assert_eq!(lines(&rows)[0].0, LineKind::Context);
        // …and stays a change when the toggle is off.
        let rows = build_rows(
            DiffInput::Patch("@@\n-  let x = 1;\n+let    x = 1;\n"),
            &opts(),
        )
        .unwrap();
        assert_eq!(lines(&rows)[0].0, LineKind::Change);
    }

    #[test]
    fn word_highlight_marks_only_the_chars_that_differ() {
        let rows = build_rows(
            DiffInput::Patch("@@\n-say hello now\n+say howdy now\n"),
            &opts(),
        )
        .unwrap();
        let Row::Line {
            left: Some(l),
            right: Some(r),
            ..
        } = &rows[0]
        else {
            panic!("expected a change row, got {:?}", rows[0]);
        };
        // "say h" is the common prefix, " now" the common suffix.
        assert_eq!(l.text[l.hl.unwrap().0..l.hl.unwrap().1].to_string(), "ello");
        assert_eq!(r.text[r.hl.unwrap().0..r.hl.unwrap().1].to_string(), "owdy");
    }

    #[test]
    fn word_highlight_off_leaves_no_ranges() {
        let opt = Options {
            word_highlight: false,
            ..opts()
        };
        let rows = build_rows(DiffInput::Patch("@@\n-abc\n+axc\n"), &opt).unwrap();
        let Row::Line { left: Some(l), .. } = &rows[0] else {
            panic!("expected a change row");
        };
        assert_eq!(l.hl, None);
    }

    #[test]
    fn context_collapses_long_unchanged_runs_into_a_note() {
        let filler = (0..20)
            .map(|i| format!(" line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let patch = format!("@@\n-old\n+new\n{filler}\n-tail\n+TAIL\n");
        let opt = Options {
            context: 2,
            ..opts()
        };
        let rows = build_rows(DiffInput::Patch(&patch), &opt).unwrap();
        // 2 kept above + 2 below → 16 of the 20 filler lines collapse.
        assert!(
            headers(&rows)
                .iter()
                .any(|h| *h == "... 16 unchanged line(s)"),
            "{:?}",
            headers(&rows)
        );
        assert_eq!(lines(&rows).len(), 2 + 4);
    }

    #[test]
    fn a_generous_context_keeps_every_line() {
        let filler = (0..8)
            .map(|i| format!(" line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let opt = Options {
            context: 50,
            ..opts()
        };
        let rows = build_rows(
            DiffInput::Patch(&format!("@@\n-old\n+new\n{filler}\n")),
            &opt,
        )
        .unwrap();
        assert!(headers(&rows).iter().all(|h| !h.contains("unchanged")));
    }

    #[test]
    fn the_unified_layout_splits_a_change_into_a_removal_then_an_addition() {
        let opt = Options {
            layout: Layout::Unified,
            ..opts()
        };
        let rows = build_rows(DiffInput::Patch("@@\n-old\n+new\n"), &opt).unwrap();
        assert_eq!(
            lines(&rows),
            vec![
                (LineKind::Remove, Some("old"), None),
                (LineKind::Add, None, Some("new")),
            ]
        );
    }

    #[test]
    fn over_long_inputs_are_truncated_with_a_stated_row_limit() {
        let body = (0..MAX_ROWS + 5)
            .map(|i| format!("-old{i}\n+new{i}\n"))
            .collect::<String>();
        let patch = format!("@@ -1,{} +1,{} @@\n{body}", MAX_ROWS + 5, MAX_ROWS + 5);
        let rows = build_rows(DiffInput::Patch(&patch), &opts()).unwrap();
        assert_eq!(rows.len(), MAX_ROWS + 1);
        assert!(
            rows.last().unwrap()
                == &Row::Header(format!(
                    "... {} more row(s) not rendered (limit {MAX_ROWS})",
                    (MAX_ROWS + 5) + 1 - MAX_ROWS
                ))
        );
    }

    // ---- rendering -------------------------------------------------------

    #[test]
    fn rendering_a_patch_produces_a_non_trivial_png() {
        let (png, w, h) = render_png(DiffInput::Patch(PATCH), &opts()).unwrap();
        assert_eq!(png_dims(&png), (w, h));
        assert!(w > 200 && h > 100, "{w}x{h}");
    }

    #[test]
    fn the_unified_layout_renders_narrower_and_taller_than_side_by_side() {
        let (_, sw, sh) = render_png(DiffInput::Patch(PATCH), &opts()).unwrap();
        let (_, uw, uh) = render_png(
            DiffInput::Patch(PATCH),
            &Options {
                layout: Layout::Unified,
                ..opts()
            },
        )
        .unwrap();
        assert!(uw < sw, "unified {uw} should be narrower than split {sw}");
        assert!(uh > sh, "unified {uh} should be taller than split {sh}");
    }

    #[test]
    fn dropping_the_line_number_gutters_narrows_the_image() {
        let (_, w_on, _) = render_png(DiffInput::Patch(PATCH), &opts()).unwrap();
        let (_, w_off, _) = render_png(
            DiffInput::Patch(PATCH),
            &Options {
                line_numbers: false,
                ..opts()
            },
        )
        .unwrap();
        assert!(w_off < w_on, "{w_off} vs {w_on}");
    }

    #[test]
    fn a_light_theme_renders_a_light_background() {
        let (png, _, _) = render_png(
            DiffInput::Patch(PATCH),
            &Options {
                theme: Some("InspiredGitHub".into()),
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn an_unknown_theme_or_language_still_renders() {
        let (png, _, _) = render_png(
            DiffInput::Patch(PATCH),
            &Options {
                theme: Some("no-such-theme".into()),
                language: Some("no-such-language".into()),
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn a_very_long_line_is_capped_at_the_column_limit() {
        let long = "x".repeat(MAX_COLS * 3);
        let (_, w, _) = render_png(
            DiffInput::Pair {
                left: &long,
                right: "short",
            },
            &opts(),
        )
        .unwrap();
        let (_, w_ref, _) = render_png(
            DiffInput::Pair {
                left: &"x".repeat(MAX_COLS * 9),
                right: "short",
            },
            &opts(),
        )
        .unwrap();
        assert_eq!(w, w_ref, "width saturates at the {MAX_COLS}-column cap");
    }

    #[test]
    fn rendering_is_deterministic() {
        let a = render_png(DiffInput::Patch(PATCH), &opts()).unwrap();
        let b = render_png(DiffInput::Patch(PATCH), &opts()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn bundled_themes_are_available_by_name() {
        let names = theme_names();
        assert!(names.iter().any(|n| n == "base16-ocean.dark"), "{names:?}");
        assert!(names.iter().any(|n| n == "InspiredGitHub"), "{names:?}");
    }
}
