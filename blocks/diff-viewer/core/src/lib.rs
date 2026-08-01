//! diff-viewer core — parse a pasted **unified diff** and render it cleanly.
//!
//! Input is an already-computed unified diff (the output of `git diff`,
//! `diff -u`, or a `.patch` file) — NOT two texts to compare (that's the
//! `text-diff` tool). This block parses the diff into files → hunks → lines,
//! computes add/remove stats, and renders one of four views: a clean `inline`
//! diff, a text `side-by-side` (old | new) layout, a `git --stat`-style
//! `stats` summary, or a structured `json` report for programmatic use. An
//! optional `ignore_whitespace` flag hides changes whose only difference is
//! whitespace.
//!
//! Pure-Rust, no I/O — shared by the chat skill block, the CLI, and the web
//! page (whose colored renderer consumes the `json` payload from
//! [`render_payload`]).

use serde::Serialize;

/// The kind of a single line inside a hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LineKind {
    /// Unchanged context line (leading space).
    Context,
    /// Added line (leading `+`).
    Add,
    /// Removed line (leading `-`).
    Del,
}

/// One line within a hunk, with its 1-based numbers on each side.
#[derive(Debug, Clone, Serialize)]
pub struct HunkLine {
    pub kind: LineKind,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_number: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_number: Option<usize>,
}

/// A single `@@ ... @@` hunk.
#[derive(Debug, Clone, Serialize)]
pub struct Hunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    /// The trailing section heading after the second `@@`, if any (e.g. the
    /// enclosing function name git prints).
    #[serde(skip_serializing_if = "str::is_empty")]
    pub section: String,
    pub lines: Vec<HunkLine>,
}

/// High-level change type for a file section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    Added,
    Deleted,
    Modified,
    Renamed,
    Binary,
}

/// One file section of the diff.
#[derive(Debug, Clone, Serialize)]
pub struct FileDiff {
    /// Path on the old side (`a/…`, or `/dev/null` for a new file).
    pub old_path: String,
    /// Path on the new side (`b/…`, or `/dev/null` for a deletion).
    pub new_path: String,
    /// The best single path to display (new path, or old path for a deletion).
    pub path: String,
    pub change_type: ChangeType,
    pub additions: usize,
    pub deletions: usize,
    pub hunks: Vec<Hunk>,
}

/// The whole parsed diff.
#[derive(Debug, Clone, Serialize)]
pub struct ParsedDiff {
    pub files_changed: usize,
    pub additions: usize,
    pub deletions: usize,
    pub files: Vec<FileDiff>,
}

/// Strip a leading `a/` or `b/` (git prefix) from a path for display. Leaves
/// `/dev/null` and prefix-less paths untouched.
fn strip_ab(p: &str) -> &str {
    if p == "/dev/null" {
        return p;
    }
    p.strip_prefix("a/")
        .or_else(|| p.strip_prefix("b/"))
        .unwrap_or(p)
}

/// Take the path token from a `--- ` / `+++ ` header, dropping a trailing tab
/// and everything after it (git appends a timestamp there for `diff -u`).
fn header_path(rest: &str) -> String {
    let cut = rest.split('\t').next().unwrap_or(rest);
    cut.trim_end().to_string()
}

/// Parse the `@@ -old[,cnt] +new[,cnt] @@ section` header. Returns the four
/// numbers plus the section heading, or `None` if malformed.
fn parse_hunk_header(line: &str) -> Option<(usize, usize, usize, usize, String)> {
    let rest = line.strip_prefix("@@ ")?;
    let end = rest.find(" @@")?;
    let ranges = &rest[..end];
    let section = rest[end + 3..].trim().to_string();
    let mut parts = ranges.split(' ');
    let old = parts.next()?.strip_prefix('-')?;
    let new = parts.next()?.strip_prefix('+')?;
    let (os, oc) = parse_range(old)?;
    let (ns, nc) = parse_range(new)?;
    Some((os, oc, ns, nc, section))
}

/// Parse a `start[,count]` range; count defaults to 1 when omitted.
fn parse_range(s: &str) -> Option<(usize, usize)> {
    let mut it = s.split(',');
    let start = it.next()?.parse::<usize>().ok()?;
    let count = match it.next() {
        Some(c) => c.parse::<usize>().ok()?,
        None => 1,
    };
    Some((start, count))
}

/// Parse a unified diff into structured files/hunks/lines. Lenient about
/// leading `diff --git`/`index`/mode metadata and about plain `diff -u` output
/// with no `diff --git` line. Returns an error if no file/hunk can be found.
pub fn parse(diff: &str) -> Result<ParsedDiff, String> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut cur: Option<FileDiff> = None;
    let mut hunk: Option<Hunk> = None;
    let mut old_n = 0usize;
    let mut new_n = 0usize;

    // Finalize the in-progress hunk into the current file.
    fn flush_hunk(cur: &mut Option<FileDiff>, hunk: &mut Option<Hunk>) {
        if let (Some(f), Some(h)) = (cur.as_mut(), hunk.take()) {
            f.hunks.push(h);
        }
    }
    // Finalize the current file into the list.
    fn flush_file(files: &mut Vec<FileDiff>, cur: &mut Option<FileDiff>) {
        if let Some(f) = cur.take() {
            files.push(f);
        }
    }

    for raw in diff.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);

        if let Some(rest) = line.strip_prefix("diff --git ") {
            flush_hunk(&mut cur, &mut hunk);
            flush_file(&mut files, &mut cur);
            // `a/path b/path`; paths may contain spaces, but the common case
            // splits cleanly at " b/". Fall back to a midpoint split.
            let (a, b) = split_git_paths(rest);
            cur = Some(new_file(a, b));
            continue;
        }
        if let Some(rest) = line.strip_prefix("--- ") {
            let p = header_path(rest);
            match cur.as_mut() {
                Some(f) => {
                    f.old_path = p;
                }
                None => {
                    let mut f = new_file(String::new(), String::new());
                    f.old_path = p;
                    cur = Some(f);
                }
            }
            recompute_paths(cur.as_mut().unwrap());
            continue;
        }
        if let Some(rest) = line.strip_prefix("+++ ") {
            let p = header_path(rest);
            if let Some(f) = cur.as_mut() {
                f.new_path = p;
                recompute_paths(f);
            }
            continue;
        }
        if line.starts_with("@@ ") {
            if let Some((os, oc, ns, nc, section)) = parse_hunk_header(line) {
                flush_hunk(&mut cur, &mut hunk);
                if cur.is_none() {
                    cur = Some(new_file(String::new(), String::new()));
                }
                old_n = os;
                new_n = ns;
                hunk = Some(Hunk {
                    old_start: os,
                    old_count: oc,
                    new_start: ns,
                    new_count: nc,
                    section,
                    lines: Vec::new(),
                });
                continue;
            }
        }
        if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
            if let Some(f) = cur.as_mut() {
                if f.change_type == ChangeType::Modified {
                    f.change_type = ChangeType::Binary;
                }
            }
            continue;
        }
        // File metadata between the git header and the hunks.
        if line.starts_with("new file mode ") {
            if let Some(f) = cur.as_mut() {
                f.change_type = ChangeType::Added;
            }
            continue;
        }
        if line.starts_with("deleted file mode ") {
            if let Some(f) = cur.as_mut() {
                f.change_type = ChangeType::Deleted;
            }
            continue;
        }
        if line.starts_with("rename from ") || line.starts_with("rename to ") {
            if let Some(f) = cur.as_mut() {
                f.change_type = ChangeType::Renamed;
            }
            continue;
        }

        // Body lines only count inside a hunk.
        if let Some(h) = hunk.as_mut() {
            let (kind, content) = if let Some(c) = line.strip_prefix('+') {
                (LineKind::Add, c)
            } else if let Some(c) = line.strip_prefix('-') {
                (LineKind::Del, c)
            } else if let Some(c) = line.strip_prefix(' ') {
                (LineKind::Context, c)
            } else if line.starts_with('\\') {
                // "\ No newline at end of file" — a marker, not content.
                continue;
            } else {
                // Anything else (stray prose, `index …`, etc.) ends the hunk.
                flush_hunk(&mut cur, &mut hunk);
                continue;
            };
            let f = cur.as_mut().unwrap();
            let (o, n) = match kind {
                LineKind::Context => {
                    let o = old_n;
                    let nn = new_n;
                    old_n += 1;
                    new_n += 1;
                    (Some(o), Some(nn))
                }
                LineKind::Add => {
                    f.additions += 1;
                    let nn = new_n;
                    new_n += 1;
                    (None, Some(nn))
                }
                LineKind::Del => {
                    f.deletions += 1;
                    let o = old_n;
                    old_n += 1;
                    (Some(o), None)
                }
            };
            h.lines.push(HunkLine {
                kind,
                content: content.to_string(),
                old_number: o,
                new_number: n,
            });
        }
        // Non-hunk lines outside a file header (`index …`, blank separators)
        // are ignored.
    }
    flush_hunk(&mut cur, &mut hunk);
    flush_file(&mut files, &mut cur);

    let has_content = files
        .iter()
        .any(|f| !f.hunks.is_empty() || f.change_type != ChangeType::Modified);
    if files.is_empty() || !has_content {
        return Err(
            "no unified diff found — paste the output of `git diff`, `diff -u`, or a .patch file"
                .into(),
        );
    }

    let additions = files.iter().map(|f| f.additions).sum();
    let deletions = files.iter().map(|f| f.deletions).sum();
    Ok(ParsedDiff {
        files_changed: files.len(),
        additions,
        deletions,
        files,
    })
}

/// Split the `a/path b/path` tail of a `diff --git` line into the two paths.
fn split_git_paths(rest: &str) -> (String, String) {
    if let Some(idx) = rest.find(" b/") {
        let a = rest[..idx].to_string();
        let b = rest[idx + 1..].to_string();
        return (a, b);
    }
    // Fallback: split on the middle space.
    match rest.rsplit_once(' ') {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => (rest.to_string(), rest.to_string()),
    }
}

fn new_file(old_path: String, new_path: String) -> FileDiff {
    let mut f = FileDiff {
        old_path,
        new_path,
        path: String::new(),
        change_type: ChangeType::Modified,
        additions: 0,
        deletions: 0,
        hunks: Vec::new(),
    };
    recompute_paths(&mut f);
    f
}

/// Recompute the display `path` and infer add/delete from `/dev/null` sides.
fn recompute_paths(f: &mut FileDiff) {
    let old_dev = f.old_path == "/dev/null";
    let new_dev = f.new_path == "/dev/null";
    if old_dev && !new_dev && f.change_type == ChangeType::Modified {
        f.change_type = ChangeType::Added;
    }
    if new_dev && !old_dev && f.change_type == ChangeType::Modified {
        f.change_type = ChangeType::Deleted;
    }
    let display = if new_dev {
        strip_ab(&f.old_path)
    } else if old_dev {
        strip_ab(&f.new_path)
    } else if !f.new_path.is_empty() {
        strip_ab(&f.new_path)
    } else {
        strip_ab(&f.old_path)
    };
    f.path = display.to_string();
}

/// Collapse the inner whitespace of a line for whitespace-insensitive
/// comparison (leading/trailing/repeated spaces all fold together).
fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Apply `ignore_whitespace`: within each hunk, a deleted line directly paired
/// with an added line that is identical after whitespace normalization becomes
/// a single unchanged (context) line, and stops counting toward +/-. Re-counts
/// each file's additions/deletions afterward.
fn apply_ignore_whitespace(d: &mut ParsedDiff) {
    for f in &mut d.files {
        for h in &mut f.hunks {
            let mut rebuilt: Vec<HunkLine> = Vec::with_capacity(h.lines.len());
            let mut i = 0;
            while i < h.lines.len() {
                if h.lines[i].kind == LineKind::Context {
                    rebuilt.push(h.lines[i].clone());
                    i += 1;
                    continue;
                }
                // Span a contiguous change block.
                let start = i;
                while i < h.lines.len() && h.lines[i].kind != LineKind::Context {
                    i += 1;
                }
                let block = &h.lines[start..i];
                let dels: Vec<&HunkLine> =
                    block.iter().filter(|l| l.kind == LineKind::Del).collect();
                let adds: Vec<&HunkLine> =
                    block.iter().filter(|l| l.kind == LineKind::Add).collect();
                let pairs = dels.len().min(adds.len());
                for p in 0..pairs {
                    if normalize_ws(&dels[p].content) == normalize_ws(&adds[p].content) {
                        // Whitespace-only change → keep as a single context line
                        // showing the new content but carrying both numbers.
                        rebuilt.push(HunkLine {
                            kind: LineKind::Context,
                            content: adds[p].content.clone(),
                            old_number: dels[p].old_number,
                            new_number: adds[p].new_number,
                        });
                    } else {
                        rebuilt.push((*dels[p]).clone());
                        rebuilt.push((*adds[p]).clone());
                    }
                }
                for d in dels.iter().skip(pairs) {
                    rebuilt.push((**d).clone());
                }
                for a in adds.iter().skip(pairs) {
                    rebuilt.push((**a).clone());
                }
            }
            h.lines = rebuilt;
        }
        f.additions = f
            .hunks
            .iter()
            .flat_map(|h| h.lines.iter())
            .filter(|l| l.kind == LineKind::Add)
            .count();
        f.deletions = f
            .hunks
            .iter()
            .flat_map(|h| h.lines.iter())
            .filter(|l| l.kind == LineKind::Del)
            .count();
    }
    d.additions = d.files.iter().map(|f| f.additions).sum();
    d.deletions = d.files.iter().map(|f| f.deletions).sum();
}

/// Human-readable `"N files changed, A insertions(+), D deletions(-)"` summary.
fn summary_line(d: &ParsedDiff) -> String {
    let files = plural(d.files_changed, "file", "files");
    let mut parts = vec![format!("{} {} changed", d.files_changed, files)];
    if d.additions > 0 {
        parts.push(format!(
            "{} {}(+)",
            d.additions,
            plural(d.additions, "insertion", "insertions")
        ));
    }
    if d.deletions > 0 {
        parts.push(format!(
            "{} {}(-)",
            d.deletions,
            plural(d.deletions, "deletion", "deletions")
        ));
    }
    parts.join(", ")
}

fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 {
        one
    } else {
        many
    }
}

/// Clean inline view: a summary line, then each file's hunks with `@@` headers
/// and ` `/`+`/`-` prefixes.
fn render_inline(d: &ParsedDiff) -> String {
    let mut out = String::new();
    out.push_str(&summary_line(d));
    out.push('\n');
    for f in &d.files {
        out.push('\n');
        out.push_str(&file_banner(f));
        out.push('\n');
        for h in &f.hunks {
            out.push_str(&format!(
                "@@ -{},{} +{},{} @@",
                h.old_start, h.old_count, h.new_start, h.new_count
            ));
            if !h.section.is_empty() {
                out.push(' ');
                out.push_str(&h.section);
            }
            out.push('\n');
            for l in &h.lines {
                let p = match l.kind {
                    LineKind::Context => ' ',
                    LineKind::Add => '+',
                    LineKind::Del => '-',
                };
                out.push(p);
                out.push_str(&l.content);
                out.push('\n');
            }
        }
    }
    out.trim_end_matches('\n').to_string()
}

/// A one-line banner describing a file section (path + change type).
fn file_banner(f: &FileDiff) -> String {
    match f.change_type {
        ChangeType::Added => format!("{} (new file)", f.path),
        ChangeType::Deleted => format!("{} (deleted)", f.path),
        ChangeType::Renamed => format!(
            "{} → {} (renamed)",
            strip_ab(&f.old_path),
            strip_ab(&f.new_path)
        ),
        ChangeType::Binary => format!("{} (binary)", f.path),
        ChangeType::Modified => f.path.clone(),
    }
}

/// `git diff --stat`-style view: per-file bar graph + totals.
fn render_stats(d: &ParsedDiff) -> String {
    const GRAPH_WIDTH: usize = 40;
    let name_w = d
        .files
        .iter()
        .map(|f| stat_name(f).chars().count())
        .max()
        .unwrap_or(0);
    let max_changes = d
        .files
        .iter()
        .map(|f| f.additions + f.deletions)
        .max()
        .unwrap_or(0);
    let scale = if max_changes > GRAPH_WIDTH {
        GRAPH_WIDTH as f64 / max_changes as f64
    } else {
        1.0
    };

    let mut out = String::new();
    for f in &d.files {
        let name = stat_name(f);
        let changes = f.additions + f.deletions;
        let bar = if f.change_type == ChangeType::Binary {
            "Bin".to_string()
        } else {
            let plus = scaled(f.additions, scale);
            let minus = scaled(f.deletions, scale);
            format!("{}{}", "+".repeat(plus), "-".repeat(minus))
        };
        out.push_str(&format!(
            " {:<width$} | {:>4} {}\n",
            name,
            changes,
            bar,
            width = name_w
        ));
    }
    out.push_str(&format!(" {}", summary_line(d)));
    out
}

/// Scale a count to the graph, rounding up so any nonzero count shows ≥1 mark.
fn scaled(n: usize, scale: f64) -> usize {
    if n == 0 {
        return 0;
    }
    ((n as f64) * scale).ceil().max(1.0) as usize
}

/// The name shown in the stat table (`old → new` for renames).
fn stat_name(f: &FileDiff) -> String {
    if f.change_type == ChangeType::Renamed {
        format!("{} → {}", strip_ab(&f.old_path), strip_ab(&f.new_path))
    } else {
        f.path.clone()
    }
}

/// Text side-by-side view: `old# old | new# new`, changed lines paired.
fn render_side_by_side(d: &ParsedDiff) -> String {
    // Column width for the old side's content, capped so very long lines don't
    // blow out the layout.
    const MAX_COL: usize = 80;
    let content_w = d
        .files
        .iter()
        .flat_map(|f| f.hunks.iter())
        .flat_map(|h| h.lines.iter())
        .filter(|l| l.kind != LineKind::Add)
        .map(|l| l.content.chars().count())
        .max()
        .unwrap_or(0)
        .min(MAX_COL)
        .max(4);

    let mut out = String::new();
    out.push_str(&summary_line(d));
    out.push('\n');
    for f in &d.files {
        out.push('\n');
        out.push_str(&file_banner(f));
        out.push('\n');
        for h in &f.hunks {
            let rows = pair_rows(&h.lines);
            for (left, right) in rows {
                out.push_str(&fmt_sbs_row(left, right, content_w));
                out.push('\n');
            }
        }
    }
    out.trim_end_matches('\n').to_string()
}

/// A resolved side-by-side cell: line number + content + a change marker.
struct Cell<'a> {
    number: Option<usize>,
    content: &'a str,
    marker: char,
}

/// Pair hunk lines into (old, new) rows: context aligns on both sides; a run of
/// deletions/additions is zipped so changed lines sit across from each other.
fn pair_rows(lines: &[HunkLine]) -> Vec<(Option<Cell<'_>>, Option<Cell<'_>>)> {
    let mut rows = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        match lines[i].kind {
            LineKind::Context => {
                let l = &lines[i];
                rows.push((
                    Some(Cell {
                        number: l.old_number,
                        content: &l.content,
                        marker: ' ',
                    }),
                    Some(Cell {
                        number: l.new_number,
                        content: &l.content,
                        marker: ' ',
                    }),
                ));
                i += 1;
            }
            _ => {
                let start = i;
                while i < lines.len() && lines[i].kind != LineKind::Context {
                    i += 1;
                }
                let block = &lines[start..i];
                let dels: Vec<&HunkLine> =
                    block.iter().filter(|l| l.kind == LineKind::Del).collect();
                let adds: Vec<&HunkLine> =
                    block.iter().filter(|l| l.kind == LineKind::Add).collect();
                let n = dels.len().max(adds.len());
                for k in 0..n {
                    let left = dels.get(k).map(|l| Cell {
                        number: l.old_number,
                        content: &l.content,
                        marker: if adds.get(k).is_some() { '~' } else { '<' },
                    });
                    let right = adds.get(k).map(|l| Cell {
                        number: l.new_number,
                        content: &l.content,
                        marker: if dels.get(k).is_some() { '~' } else { '>' },
                    });
                    rows.push((left, right));
                }
            }
        }
    }
    rows
}

fn fmt_sbs_row(left: Option<Cell<'_>>, right: Option<Cell<'_>>, content_w: usize) -> String {
    let (lnum, lmark, ltext) = match &left {
        Some(c) => (
            c.number.map(|n| n.to_string()).unwrap_or_default(),
            c.marker,
            c.content,
        ),
        None => (String::new(), ' ', ""),
    };
    let (rnum, rmark, rtext) = match &right {
        Some(c) => (
            c.number.map(|n| n.to_string()).unwrap_or_default(),
            c.marker,
            c.content,
        ),
        None => (String::new(), ' ', ""),
    };
    // Pad the old-side content to the column width (truncation-free: a longer
    // line simply pushes the separator right for that row).
    let lpad = content_w.saturating_sub(ltext.chars().count());
    format!(
        "{:>5} {} {}{} | {:>5} {} {}",
        lnum,
        lmark,
        ltext,
        " ".repeat(lpad),
        rnum,
        rmark,
        rtext
    )
    .trim_end()
    .to_string()
}

/// Render the structured JSON report (pretty-printed with a 2-space indent).
fn render_json(d: &ParsedDiff) -> String {
    serde_json::to_string_pretty(d).unwrap()
}

/// Parse `diff`, optionally fold whitespace-only changes, and validate `view`.
fn prepare(diff: &str, view: &str, ignore_whitespace: bool) -> Result<ParsedDiff, String> {
    if !matches!(view, "inline" | "side-by-side" | "stats" | "json") {
        return Err(format!(
            "unknown view '{view}' — expected 'inline', 'side-by-side', 'stats', or 'json'"
        ));
    }
    let mut parsed = parse(diff)?;
    if ignore_whitespace {
        apply_ignore_whitespace(&mut parsed);
    }
    Ok(parsed)
}

/// Public entry: parse `diff` and render it in `view`
/// (`inline` | `side-by-side` | `stats` | `json`). `ignore_whitespace` folds
/// whitespace-only changes into context. Errors on an unknown view or an input
/// that contains no recognizable diff.
pub fn render(diff: &str, view: &str, ignore_whitespace: bool) -> Result<String, String> {
    let parsed = prepare(diff, view, ignore_whitespace)?;
    Ok(match view {
        "inline" => render_inline(&parsed),
        "side-by-side" => render_side_by_side(&parsed),
        "stats" => render_stats(&parsed),
        "json" => render_json(&parsed),
        _ => unreachable!("view validated in prepare"),
    })
}

/// Convenience alias matching the block/CLI entry naming convention.
pub fn run(diff: &str, view: &str, ignore_whitespace: bool) -> Result<String, String> {
    render(diff, view, ignore_whitespace)
}

/// Build the JSON payload the web page consumes: the full parsed structure plus
/// the selected `view` so the browser renderer knows which layout to draw.
/// Applies `ignore_whitespace` just like [`render`].
pub fn render_payload(diff: &str, view: &str, ignore_whitespace: bool) -> Result<String, String> {
    let parsed = prepare(diff, view, ignore_whitespace)?;
    #[derive(Serialize)]
    struct Payload<'a> {
        view: &'a str,
        #[serde(flatten)]
        diff: &'a ParsedDiff,
    }
    Ok(serde_json::to_string(&Payload {
        view,
        diff: &parsed,
    })
    .unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "diff --git a/src/main.rs b/src/main.rs\nindex e69de29..4b825dc 100644\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,4 +1,5 @@\n fn main() {\n-    println!(\"hi\");\n+    println!(\"hello\");\n+    println!(\"world\");\n }\n";

    #[test]
    fn parses_stats_and_lines() {
        let d = parse(SAMPLE).unwrap();
        assert_eq!(d.files_changed, 1);
        assert_eq!(d.additions, 2);
        assert_eq!(d.deletions, 1);
        assert_eq!(d.files[0].path, "src/main.rs");
        assert_eq!(d.files[0].change_type, ChangeType::Modified);
        let h = &d.files[0].hunks[0];
        assert_eq!(
            (h.old_start, h.old_count, h.new_start, h.new_count),
            (1, 4, 1, 5)
        );
        // Context line carries both numbers; add carries only new; del only old.
        assert_eq!(h.lines[0].kind, LineKind::Context);
        assert_eq!(h.lines[0].old_number, Some(1));
        assert_eq!(h.lines[0].new_number, Some(1));
        assert_eq!(h.lines[1].kind, LineKind::Del);
        assert_eq!(h.lines[1].old_number, Some(2));
        assert_eq!(h.lines[1].new_number, None);
        assert_eq!(h.lines[2].kind, LineKind::Add);
        assert_eq!(h.lines[2].new_number, Some(2));
    }

    #[test]
    fn inline_view_has_summary_and_prefixes() {
        let out = render(SAMPLE, "inline", false).unwrap();
        assert!(
            out.starts_with("1 file changed, 2 insertions(+), 1 deletion(-)"),
            "{out}"
        );
        assert!(out.contains("src/main.rs\n@@ -1,4 +1,5 @@"), "{out}");
        assert!(out.contains("-    println!(\"hi\");"), "{out}");
        assert!(out.contains("+    println!(\"hello\");"), "{out}");
    }

    #[test]
    fn stats_view_bar_and_totals() {
        let out = render(SAMPLE, "stats", false).unwrap();
        assert!(out.contains("src/main.rs |    3 ++-"), "{out}");
        assert!(
            out.trim_end()
                .ends_with("1 file changed, 2 insertions(+), 1 deletion(-)"),
            "{out}"
        );
    }

    #[test]
    fn side_by_side_pairs_changed_lines() {
        let out = render(SAMPLE, "side-by-side", false).unwrap();
        // The changed line pairs old `hi` opposite new `hello` with `~` markers.
        assert!(out.contains('~'), "{out}");
        assert!(out.contains("println!(\"hi\");"), "{out}");
        assert!(out.contains("println!(\"hello\");"), "{out}");
        // The pure add (`world`) sits on the right with an empty left side.
        assert!(
            out.lines()
                .any(|l| l.contains("println!(\"world\");") && l.trim_start().starts_with('|')),
            "{out}"
        );
    }

    #[test]
    fn json_view_is_structured() {
        let out = render(SAMPLE, "json", false).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["files_changed"], 1);
        assert_eq!(v["additions"], 2);
        assert_eq!(v["deletions"], 1);
        assert_eq!(v["files"][0]["path"], "src/main.rs");
        assert_eq!(v["files"][0]["change_type"], "modified");
        assert_eq!(v["files"][0]["hunks"][0]["lines"][1]["kind"], "del");
        assert_eq!(v["files"][0]["hunks"][0]["lines"][1]["old_number"], 2);
    }

    #[test]
    fn payload_embeds_view() {
        let out = render_payload(SAMPLE, "side-by-side", false).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["view"], "side-by-side");
        assert_eq!(v["files_changed"], 1);
        assert_eq!(v["files"][0]["path"], "src/main.rs");
    }

    #[test]
    fn detects_new_and_deleted_files() {
        let add = "diff --git a/new.txt b/new.txt\nnew file mode 100644\n--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1,2 @@\n+one\n+two\n";
        let d = parse(add).unwrap();
        assert_eq!(d.files[0].change_type, ChangeType::Added);
        assert_eq!(d.files[0].path, "new.txt");
        assert_eq!(d.additions, 2);

        let del = "diff --git a/old.txt b/old.txt\ndeleted file mode 100644\n--- a/old.txt\n+++ /dev/null\n@@ -1,1 +0,0 @@\n-gone\n";
        let d = parse(del).unwrap();
        assert_eq!(d.files[0].change_type, ChangeType::Deleted);
        assert_eq!(d.files[0].path, "old.txt");
        assert_eq!(d.deletions, 1);
    }

    #[test]
    fn detects_rename_and_binary() {
        let ren = "diff --git a/from.txt b/to.txt\nsimilarity index 100%\nrename from from.txt\nrename to to.txt\n";
        let d = parse(ren).unwrap();
        assert_eq!(d.files[0].change_type, ChangeType::Renamed);
        assert!(
            file_banner(&d.files[0]).contains('→'),
            "{}",
            file_banner(&d.files[0])
        );

        let bin = "diff --git a/logo.png b/logo.png\nindex 0000..1111 100644\nBinary files a/logo.png and b/logo.png differ\n";
        let d = parse(bin).unwrap();
        assert_eq!(d.files[0].change_type, ChangeType::Binary);
    }

    #[test]
    fn handles_multiple_files() {
        let multi = format!(
            "{SAMPLE}diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old title\n+new title\n"
        );
        let d = parse(&multi).unwrap();
        assert_eq!(d.files_changed, 2);
        assert_eq!(d.additions, 3);
        assert_eq!(d.deletions, 2);
        assert_eq!(d.files[1].path, "README.md");
    }

    #[test]
    fn plain_diff_u_without_git_header() {
        let plain = "--- a.txt\n+++ b.txt\n@@ -1,2 +1,2 @@\n line one\n-line two\n+line 2\n";
        let d = parse(plain).unwrap();
        assert_eq!(d.files_changed, 1);
        assert_eq!(d.additions, 1);
        assert_eq!(d.deletions, 1);
        assert_eq!(d.files[0].path, "b.txt");
    }

    #[test]
    fn hunk_section_heading_preserved() {
        let s = "--- a/x.rs\n+++ b/x.rs\n@@ -10,3 +10,3 @@ fn compute() {\n-    let a = 1;\n+    let a = 2;\n b\n c\n";
        let d = parse(s).unwrap();
        assert_eq!(d.files[0].hunks[0].section, "fn compute() {");
        let out = render(s, "inline", false).unwrap();
        assert!(out.contains("@@ -10,3 +10,3 @@ fn compute() {"), "{out}");
    }

    #[test]
    fn ignore_whitespace_folds_ws_only_changes() {
        // Only line 2 changes, and only by indentation.
        let s = "--- a/x\n+++ b/x\n@@ -1,3 +1,3 @@\n a\n-  b\n+b\n c\n";
        let plain = parse(s).unwrap();
        assert_eq!(plain.additions, 1);
        assert_eq!(plain.deletions, 1);
        let out = render(s, "json", true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["additions"], 0);
        assert_eq!(v["deletions"], 0);
        // A real (non-whitespace) change is preserved under ignore_whitespace.
        let s2 = "--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n-  b\n+  c\n d\n";
        let out2 = render(s2, "json", true).unwrap();
        let v2: serde_json::Value = serde_json::from_str(&out2).unwrap();
        assert_eq!(v2["additions"], 1);
        assert_eq!(v2["deletions"], 1);
    }

    #[test]
    fn empty_input_errors() {
        let e = render("", "inline", false).unwrap_err();
        assert!(e.contains("no unified diff found"), "{e}");
        let e2 = render("just some prose\nnot a diff", "inline", false).unwrap_err();
        assert!(e2.contains("no unified diff found"), "{e2}");
    }

    #[test]
    fn unknown_view_errors() {
        let e = render(SAMPLE, "sideways", false).unwrap_err();
        assert!(e.contains("unknown view"), "{e}");
    }

    #[test]
    fn no_newline_marker_ignored() {
        let s = "--- a\n+++ b\n@@ -1 +1 @@\n-a\n\\ No newline at end of file\n+b\n\\ No newline at end of file\n";
        let d = parse(s).unwrap();
        assert_eq!(d.additions, 1);
        assert_eq!(d.deletions, 1);
        // The marker lines are not counted as content.
        assert_eq!(d.files[0].hunks[0].lines.len(), 2);
    }
}
