//! diff-extract-versions core — pure compute, shared by the chat skill block, the
//! CLI, and the web page. No wafer/wasm-bindgen deps, no I/O, no clock: the same
//! input always produces the same bytes.
//!
//! Takes ONE pasted unified diff (`git diff`, `git show`, `git format-patch`,
//! `diff -u`, `svn diff`) and reconstructs the two file versions it describes —
//! the before-text (context + `-` lines) and the after-text (context + `+` lines).
//! It is the tool for the case where the patch is all you were sent: no original
//! file is required. Applying a diff to a file you already have is `apply-patch`;
//! computing a diff from two texts is `text-diff` / `diff-code`; rendering one is
//! `diff-viewer`.
//!
//! What a diff can and cannot tell you is taken seriously here:
//!
//! * lines between two hunks are only recoverable when the diff carries them as
//!   context — with the usual 3 lines of context they are simply absent, so the
//!   gap is reported (counted marker, spliced away, or a hard error) rather than
//!   quietly closed up;
//! * nothing after the last hunk is knowable at all — a diff never records the
//!   file's length — so a reconstruction ends where the diff ends;
//! * `@@` counts are advisory: the hunk body is authoritative, a wrong count is
//!   recounted (`recountdiff` behaviour) and flagged in the JSON output;
//! * `\ No newline at end of file` is honoured per side, and CRLF survives
//!   because everything after the marker character is copied byte for byte.

use serde_json::{json, Value};

/// Hard cap on the pasted diff — 1 MB. Anything larger is a repository dump
/// rather than a patch, and the page keeps the whole thing in wasm memory.
pub const MAX_INPUT_BYTES: usize = 1_000_000;

/// Path shown for hunks pasted without any `diff --git` / `---` / `+++` header.
const NO_FILE: &str = "(no file header)";

/// The unified-diff spelling of "this side has no file".
const DEV_NULL: &str = "/dev/null";

/// Longest quoted line fragment shown inside an error message.
const SNIPPET: usize = 60;

// ------------------------------------------------------------------- model

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Context,
    Removed,
    Added,
}

#[derive(Debug)]
struct BodyLine {
    kind: Kind,
    /// Line content with the leading `+`/`-`/space stripped, byte-verbatim
    /// otherwise (a trailing `\r` from a CRLF file is kept).
    text: String,
}

#[derive(Debug)]
struct Hunk {
    old_start: u64,
    old_count: u64,
    new_start: u64,
    new_count: u64,
    body: Vec<BodyLine>,
    /// The old side of this hunk ends without a trailing newline.
    old_no_nl: bool,
    /// The new side of this hunk ends without a trailing newline.
    new_no_nl: bool,
    /// The `@@` header's counts disagreed with the body that followed.
    miscounted: bool,
}

impl Hunk {
    fn count(&self, before: bool) -> u64 {
        self.body
            .iter()
            .filter(|l| l.kind == Kind::Context || l.kind == side_only(before))
            .count() as u64
    }
}

/// The one-sided line kind that belongs to the before (`-`) or after (`+`) text.
fn side_only(before: bool) -> Kind {
    if before {
        Kind::Removed
    } else {
        Kind::Added
    }
}

#[derive(Debug)]
struct FileEntry {
    before_path: String,
    after_path: String,
    binary: bool,
    renamed: bool,
    /// A `--- ` header has already been consumed for this entry.
    saw_old_header: bool,
    hunks: Vec<Hunk>,
}

impl FileEntry {
    fn new() -> Self {
        FileEntry {
            before_path: String::new(),
            after_path: String::new(),
            binary: false,
            renamed: false,
            saw_old_header: false,
            hunks: Vec::new(),
        }
    }

    fn status(&self) -> &'static str {
        if self.binary {
            "binary"
        } else if self.before_path == DEV_NULL {
            "added"
        } else if self.after_path == DEV_NULL {
            "deleted"
        } else if self.renamed {
            "renamed"
        } else {
            "modified"
        }
    }

    /// Best single name for this entry — the surviving path, else the old one.
    fn label(&self) -> String {
        for p in [&self.after_path, &self.before_path] {
            if !p.is_empty() && p != DEV_NULL {
                return p.clone();
            }
        }
        NO_FILE.to_string()
    }

    /// Banner path for one side, annotated when that side does not exist.
    fn side_label(&self, before: bool) -> String {
        let (own, other) = if before {
            (&self.before_path, &self.after_path)
        } else {
            (&self.after_path, &self.before_path)
        };
        if own == DEV_NULL {
            let name = if other.is_empty() || other == DEV_NULL {
                NO_FILE.to_string()
            } else {
                other.clone()
            };
            return if before {
                format!("{name} (new file — did not exist)")
            } else {
                format!("{name} (deleted)")
            };
        }
        if own.is_empty() {
            return self.label();
        }
        own.clone()
    }
}

/// A run of lines the diff does not carry, in one reconstructed version.
#[derive(Debug, Clone, Copy)]
struct Gap {
    start: u64,
    end: u64,
}

impl Gap {
    fn lines(&self) -> u64 {
        self.end - self.start + 1
    }

    fn marker(&self) -> String {
        if self.lines() == 1 {
            format!("[... 1 line not in the diff (line {}) ...]", self.start)
        } else {
            format!(
                "[... {} lines not in the diff (lines {}-{}) ...]",
                self.lines(),
                self.start,
                self.end
            )
        }
    }
}

#[derive(Debug)]
enum Row {
    Line(u64, String),
    Marker(String),
}

#[derive(Debug)]
struct Side {
    rows: Vec<Row>,
    gaps: Vec<Gap>,
    no_final_nl: bool,
}

// ------------------------------------------------------------------- entry point

/// Reconstruct the before-text and/or after-text described by a unified diff.
///
/// * `diff` — the patch. Required, 1 MB max.
/// * `output` — `both` (default, blank) | `before` | `after` | `json`.
/// * `file` — which path to reconstruct in a multi-file diff (exact path, bare
///   filename, substring, or a `*`/`?` glob). Blank keeps every file.
/// * `gaps` — `marker` (default, blank) | `omit` | `error`: what to do about
///   line ranges the diff does not carry.
/// * `line_numbers` — prefix every emitted line with its number in that version.
pub fn extract_versions(
    diff: &str,
    output: &str,
    file: &str,
    gaps: &str,
    line_numbers: bool,
) -> Result<String, String> {
    if diff.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "diff is {} bytes; the limit is {} bytes (1 MB)",
            diff.len(),
            MAX_INPUT_BYTES
        ));
    }
    let output = match output.trim() {
        "" => "both",
        o => o,
    };
    if !matches!(output, "both" | "before" | "after" | "json") {
        return Err(format!(
            "output must be one of both, before, after, json; got '{output}'"
        ));
    }
    let gaps_mode = match gaps.trim() {
        "" => "marker",
        g => g,
    };
    if !matches!(gaps_mode, "marker" | "omit" | "error") {
        return Err(format!(
            "gaps must be one of marker, omit, error; got '{gaps_mode}'"
        ));
    }
    if diff.trim().is_empty() {
        return Err(
            "diff is empty — paste a unified diff (the output of `git diff`, `git show`, \
             `diff -u`, or a .patch file)"
                .into(),
        );
    }

    let all = parse_diff(diff)?;
    if all.is_empty() {
        return Err(
            "no `@@` hunk headers found — this does not look like a unified diff. Context \
             format (`diff -c`) and normal `diff` output cannot be reconstructed; re-run with \
             `diff -u`, `git diff`, or `git format-patch`"
                .into(),
        );
    }

    let filter = file.trim();
    let files: Vec<&FileEntry> = if filter.is_empty() {
        all.iter().collect()
    } else {
        let picked: Vec<&FileEntry> = all.iter().filter(|f| matches_filter(f, filter)).collect();
        if picked.is_empty() {
            return Err(format!(
                "no file in the diff matches file='{}'; the diff touches: {}",
                filter,
                path_list(&all)
            ));
        }
        picked
    };

    if output == "json" {
        return render_json(&files, gaps_mode, line_numbers);
    }
    render_text(&files, output, gaps_mode, line_numbers)
}

// ------------------------------------------------------------------- parsing

fn parse_diff(diff: &str) -> Result<Vec<FileEntry>, String> {
    let mut raw: Vec<&str> = diff.split('\n').collect();
    if raw.last() == Some(&"") {
        raw.pop();
    }

    let mut files: Vec<FileEntry> = Vec::new();
    let mut i = 0usize;
    while i < raw.len() {
        let ctl = ctl(raw[i]);

        if ctl.starts_with("@@@") {
            return Err(format!(
                "line {}: combined diff headers (`@@@`) from a merge commit carry three columns \
                 and no single before-text — re-run the diff against one parent \
                 (`git diff <parent> <commit>`) and paste that",
                i + 1
            ));
        }

        if ctl.starts_with("@@") {
            let (old_start, old_count, new_start, new_count) =
                parse_hunk_header(ctl).ok_or_else(|| {
                    format!(
                        "line {}: could not parse the hunk header `{}` — expected \
                         `@@ -oldStart,oldCount +newStart,newCount @@`",
                        i + 1,
                        snip(ctl)
                    )
                })?;
            if files.is_empty() {
                files.push(FileEntry::new());
            }
            let body = consume_body(&raw, i + 1, old_count, new_count, i + 1)?;
            let miscounted = body.old_seen != old_count || body.new_seen != new_count;
            let entry = files.last_mut().expect("pushed above");
            entry.hunks.push(Hunk {
                old_start,
                old_count,
                new_start,
                new_count,
                body: body.lines,
                old_no_nl: body.old_no_nl,
                new_no_nl: body.new_no_nl,
                miscounted,
            });
            i = body.next;
            continue;
        }

        if let Some(rest) = ctl.strip_prefix("diff --git ") {
            let mut e = FileEntry::new();
            if let Some((a, b)) = split_git_paths(rest) {
                e.before_path = clean_path(a);
                e.after_path = clean_path(b);
            }
            files.push(e);
        } else if let Some(rest) = ctl.strip_prefix("--- ") {
            let fresh = match files.last() {
                None => true,
                Some(e) => e.saw_old_header || !e.hunks.is_empty(),
            };
            if fresh {
                files.push(FileEntry::new());
            }
            let e = files.last_mut().expect("pushed above");
            e.before_path = clean_path(strip_timestamp(rest));
            e.saw_old_header = true;
        } else if let Some(rest) = ctl.strip_prefix("+++ ") {
            if files.is_empty() {
                files.push(FileEntry::new());
            }
            let e = files.last_mut().expect("non-empty");
            e.after_path = clean_path(strip_timestamp(rest));
        } else if let Some(rest) = ctl.strip_prefix("Index: ") {
            // Subversion-style header: starts a new entry.
            let mut e = FileEntry::new();
            e.before_path = clean_path(rest);
            e.after_path = e.before_path.clone();
            files.push(e);
        } else if let Some(rest) = ctl.strip_prefix("rename from ") {
            let e = ensure_entry(&mut files);
            e.before_path = clean_path(rest);
            e.renamed = true;
        } else if let Some(rest) = ctl.strip_prefix("rename to ") {
            let e = ensure_entry(&mut files);
            e.after_path = clean_path(rest);
            e.renamed = true;
        } else if ctl.starts_with("Binary files ")
            || ctl.starts_with("Binary file ")
            || ctl.starts_with("GIT binary patch")
        {
            let e = ensure_entry(&mut files);
            e.binary = true;
        }
        // Everything else (index …, mode lines, similarity index, mail preamble,
        // commit message text) carries nothing to reconstruct.
        i += 1;
    }

    Ok(files)
}

fn ensure_entry(files: &mut Vec<FileEntry>) -> &mut FileEntry {
    if files.is_empty() {
        files.push(FileEntry::new());
    }
    files.last_mut().expect("non-empty")
}

struct Body {
    lines: Vec<BodyLine>,
    old_seen: u64,
    new_seen: u64,
    old_no_nl: bool,
    new_no_nl: bool,
    /// Index of the first line after this hunk.
    next: usize,
}

/// Consume a hunk body, starting at `start`, honouring the header's counts but
/// tolerating a wrong count in either direction (the body is authoritative).
fn consume_body(
    raw: &[&str],
    start: usize,
    old_count: u64,
    new_count: u64,
    header_line: usize,
) -> Result<Body, String> {
    let mut lines: Vec<BodyLine> = Vec::new();
    let (mut old_seen, mut new_seen) = (0u64, 0u64);
    let (mut old_no_nl, mut new_no_nl) = (false, false);
    let mut last: Option<Kind> = None;
    let mut j = start;

    while j < raw.len() {
        if is_structural(raw, j) {
            break;
        }
        let line = raw[j];
        let c = ctl(line);
        if c.starts_with('\\') {
            match last {
                Some(Kind::Context) => {
                    old_no_nl = true;
                    new_no_nl = true;
                }
                Some(Kind::Removed) => old_no_nl = true,
                Some(Kind::Added) => new_no_nl = true,
                None => {}
            }
            j += 1;
            continue;
        }

        let complete = old_seen >= old_count && new_seen >= new_count;
        let classified = match line.as_bytes().first() {
            Some(b' ') => Some((Kind::Context, line[1..].to_string())),
            Some(b'-') => Some((Kind::Removed, line[1..].to_string())),
            Some(b'+') => Some((Kind::Added, line[1..].to_string())),
            // A context line whose single trailing space was stripped in transit
            // (mail clients and copy/paste do this routinely). Only trusted while
            // the header still expects more lines, so trailing blank lines after
            // a finished hunk are not swallowed as content.
            None if !complete => Some((Kind::Context, String::new())),
            _ => None,
        };
        let Some((kind, text)) = classified else {
            if complete {
                break;
            }
            return Err(format!(
                "line {}: unexpected line `{}` inside the hunk that starts on line {} — every \
                 hunk line must start with ' ' (context), '-' (removed), '+' (added) or '\\'. \
                 If an email client word-wrapped the diff, unwrap it before pasting",
                j + 1,
                snip(c),
                header_line
            ));
        };

        match kind {
            Kind::Context => {
                old_seen += 1;
                new_seen += 1;
            }
            Kind::Removed => old_seen += 1,
            Kind::Added => new_seen += 1,
        }
        last = Some(kind);
        lines.push(BodyLine { kind, text });
        j += 1;
    }

    // A `\ No newline at end of file` that lands exactly on the hunk boundary.
    while j < raw.len() && ctl(raw[j]).starts_with('\\') {
        match last {
            Some(Kind::Context) => {
                old_no_nl = true;
                new_no_nl = true;
            }
            Some(Kind::Removed) => old_no_nl = true,
            Some(Kind::Added) => new_no_nl = true,
            None => {}
        }
        j += 1;
    }

    Ok(Body {
        lines,
        old_seen,
        new_seen,
        old_no_nl,
        new_no_nl,
        next: j,
    })
}

/// Lines that end a hunk body no matter what the `@@` counts claim.
fn is_structural(raw: &[&str], j: usize) -> bool {
    let c = ctl(raw[j]);
    if c.starts_with("@@") {
        return true;
    }
    // A `---`/`+++` PAIR is the next file's header; a lone `---` is a removed
    // line that happens to read `-- …`.
    if c.starts_with("--- ") && raw.get(j + 1).is_some_and(|n| ctl(n).starts_with("+++ ")) {
        return true;
    }
    // `git format-patch` signature block.
    if c == "-- " || c == "--" {
        return true;
    }
    const HEADS: [&str; 11] = [
        "diff --git ",
        "Index: ",
        "index ",
        "Binary files ",
        "Binary file ",
        "GIT binary patch",
        "similarity index ",
        "rename from ",
        "rename to ",
        "new file mode ",
        "deleted file mode ",
    ];
    HEADS.iter().any(|h| c.starts_with(h))
}

/// `@@ -l[,s] +l[,s] @@[ heading]` → the four numbers, with the `,1` shorthand.
fn parse_hunk_header(line: &str) -> Option<(u64, u64, u64, u64)> {
    let rest = line.strip_prefix("@@")?;
    let end = rest.find("@@")?;
    let mut parts = rest[..end].split_whitespace();
    let old = parts.next()?.strip_prefix('-')?;
    let new = parts.next()?.strip_prefix('+')?;
    if parts.next().is_some() {
        return None;
    }
    let (old_start, old_count) = parse_range(old)?;
    let (new_start, new_count) = parse_range(new)?;
    Some((old_start, old_count, new_start, new_count))
}

fn parse_range(s: &str) -> Option<(u64, u64)> {
    match s.split_once(',') {
        Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
        None => Some((s.parse().ok()?, 1)),
    }
}

/// `a/src/main.rs b/src/main.rs` → the two halves.
fn split_git_paths(rest: &str) -> Option<(&str, &str)> {
    if let Some(pos) = rest.find(" b/") {
        return Some((&rest[..pos], &rest[pos + 1..]));
    }
    let mut it = rest.split(' ');
    let a = it.next()?;
    let b = it.next()?;
    Some((a, b))
}

/// Drop the `a/`/`b/` prefix, unwrap git's quoting, keep `/dev/null` verbatim.
fn clean_path(p: &str) -> String {
    let p = p.trim_end_matches('\r').trim();
    let p = if p.len() >= 2 && p.starts_with('"') && p.ends_with('"') {
        &p[1..p.len() - 1]
    } else {
        p
    };
    if p == DEV_NULL {
        return p.to_string();
    }
    for pre in ["a/", "b/"] {
        if let Some(stripped) = p.strip_prefix(pre) {
            return stripped.to_string();
        }
    }
    p.to_string()
}

/// `--- file\t2026-08-17 …` / `--- file  2026-08-17 …` → `file`.
fn strip_timestamp(rest: &str) -> &str {
    if let Some(t) = rest.find('\t') {
        return &rest[..t];
    }
    let b = rest.as_bytes();
    let mut i = 0;
    while i + 11 <= b.len() {
        if b[i] == b' '
            && b[i + 1..i + 5].iter().all(|c| c.is_ascii_digit())
            && b[i + 5] == b'-'
            && b[i + 6..i + 8].iter().all(|c| c.is_ascii_digit())
            && b[i + 8] == b'-'
            && b[i + 9..i + 11].iter().all(|c| c.is_ascii_digit())
        {
            return &rest[..i];
        }
        i += 1;
    }
    rest
}

/// Strip a trailing CR so control lines parse the same in CRLF diffs.
fn ctl(line: &str) -> &str {
    line.strip_suffix('\r').unwrap_or(line)
}

fn snip(s: &str) -> String {
    if s.chars().count() <= SNIPPET {
        return s.to_string();
    }
    let cut: String = s.chars().take(SNIPPET).collect();
    format!("{cut}…")
}

// ------------------------------------------------------------------- selection

fn matches_filter(entry: &FileEntry, pat: &str) -> bool {
    let globby = pat.contains('*') || pat.contains('?');
    for path in [&entry.before_path, &entry.after_path] {
        if path.is_empty() || path == DEV_NULL {
            continue;
        }
        let base = path.rsplit('/').next().unwrap_or(path);
        let hit = if globby {
            glob_match(pat, path) || glob_match(pat, base)
        } else {
            path == pat || base == pat || path.contains(pat)
        };
        if hit {
            return true;
        }
    }
    false
}

/// Shell-style `*` (any run, including `/`) and `?` (one char) matching.
fn glob_match(pat: &str, text: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (usize::MAX, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = pi;
            mark = ti;
            pi += 1;
        } else if star != usize::MAX {
            pi = star + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

fn path_list(files: &[FileEntry]) -> String {
    files
        .iter()
        .map(|f| f.label())
        .collect::<Vec<_>>()
        .join(", ")
}

// ------------------------------------------------------------------- reconstruction

/// Walk one file entry's hunks and rebuild one side of it.
fn build_side(entry: &FileEntry, before: bool) -> Side {
    let mut rows: Vec<Row> = Vec::new();
    let mut gaps: Vec<Gap> = Vec::new();
    let mut cursor = 1u64;
    let mut no_final_nl = false;

    for hunk in &entry.hunks {
        let (start, header_count) = if before {
            (hunk.old_start, hunk.old_count)
        } else {
            (hunk.new_start, hunk.new_count)
        };
        // A zero-count side names the line BEFORE the insertion point.
        let first = if header_count == 0 {
            start + 1
        } else {
            start.max(1)
        };
        if first > cursor {
            let gap = Gap {
                start: cursor,
                end: first - 1,
            };
            rows.push(Row::Marker(gap.marker()));
            gaps.push(gap);
        }
        let mut n = first.max(cursor);
        let mut emitted = 0u64;
        for line in &hunk.body {
            if line.kind == Kind::Context || line.kind == side_only(before) {
                rows.push(Row::Line(n, line.text.clone()));
                n += 1;
                emitted += 1;
            }
        }
        if emitted > 0 {
            no_final_nl = if before { hunk.old_no_nl } else { hunk.new_no_nl };
        }
        cursor = cursor.max(n);
    }

    Side {
        rows,
        gaps,
        no_final_nl,
    }
}

fn format_side(side: &Side, gaps_mode: &str, line_numbers: bool) -> String {
    let rows: Vec<&Row> = side
        .rows
        .iter()
        .filter(|r| !(gaps_mode == "omit" && matches!(r, Row::Marker(_))))
        .collect();
    if rows.is_empty() {
        return String::new();
    }
    let width = side
        .rows
        .iter()
        .filter_map(|r| match r {
            Row::Line(n, _) => Some(*n),
            Row::Marker(_) => None,
        })
        .max()
        .map(|m| m.to_string().len())
        .unwrap_or(1);

    let mut out = String::new();
    for row in rows {
        match row {
            Row::Line(n, text) => {
                if line_numbers {
                    out.push_str(&format!("{n:>width$} | {text}"));
                } else {
                    out.push_str(text);
                }
            }
            Row::Marker(m) => {
                if line_numbers {
                    out.push_str(&format!("{:>width$} | {m}", ""));
                } else {
                    out.push_str(m);
                }
            }
        }
        out.push('\n');
    }
    if side.no_final_nl {
        out.pop();
    }
    out
}

fn gap_error(entry: &FileEntry, before: bool, gap: &Gap) -> String {
    format!(
        "the {} reconstruction of `{}` is incomplete: {} not carried by the diff, starting at \
         line {} (a diff only keeps the context lines around each hunk). Re-run the diff with \
         full context (`git diff -U100000`), or set gaps=marker to mark the missing region",
        if before { "before" } else { "after" },
        entry.label(),
        if gap.lines() == 1 {
            "1 line is".to_string()
        } else {
            format!("{} lines are", gap.lines())
        },
        gap.start
    )
}

// ------------------------------------------------------------------- rendering

fn render_text(
    files: &[&FileEntry],
    output: &str,
    gaps_mode: &str,
    line_numbers: bool,
) -> Result<String, String> {
    let with_hunks: Vec<&&FileEntry> = files.iter().filter(|f| !f.hunks.is_empty()).collect();
    if with_hunks.is_empty() {
        let only = files[0];
        if only.binary {
            return Err(format!(
                "`{}` is a binary diff — a binary patch carries no text to reconstruct",
                only.label()
            ));
        }
        return Err(format!(
            "the diff has no `@@` hunks for `{}` — a rename-only or mode-only entry records no \
             text, so there is nothing to reconstruct",
            only.label()
        ));
    }
    let multi = files.len() > 1;

    let mut out = String::new();
    for entry in files {
        if entry.binary {
            if multi {
                push_banner(
                    &mut out,
                    &format!("===== {}: binary diff — no text to reconstruct =====", entry.label()),
                );
            } else {
                return Err(format!(
                    "`{}` is a binary diff — a binary patch carries no text to reconstruct",
                    entry.label()
                ));
            }
            continue;
        }
        if entry.hunks.is_empty() {
            if multi {
                push_banner(
                    &mut out,
                    &format!("===== {}: {} — no hunks =====", entry.label(), entry.status()),
                );
            }
            continue;
        }

        let sides: Vec<bool> = match output {
            "before" => vec![true],
            "after" => vec![false],
            _ => vec![true, false],
        };
        for before in sides {
            let side = build_side(entry, before);
            if gaps_mode == "error" {
                if let Some(gap) = side.gaps.first() {
                    return Err(gap_error(entry, before, gap));
                }
            }
            let text = format_side(&side, gaps_mode, line_numbers);
            let banner = match output {
                "both" => Some(format!(
                    "===== {}: {} =====",
                    if before { "BEFORE" } else { "AFTER" },
                    entry.side_label(before)
                )),
                _ if multi => Some(format!("===== {} =====", entry.side_label(before))),
                _ => None,
            };
            if let Some(b) = banner {
                push_banner(&mut out, &b);
            }
            out.push_str(&text);
            if !text.is_empty() && !text.ends_with('\n') && (multi || output == "both") {
                out.push('\n');
            }
        }
    }
    Ok(out)
}

fn push_banner(out: &mut String, banner: &str) {
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(banner);
    out.push('\n');
}

fn render_json(
    files: &[&FileEntry],
    gaps_mode: &str,
    line_numbers: bool,
) -> Result<String, String> {
    let mut arr: Vec<Value> = Vec::new();
    for entry in files {
        let before = build_side(entry, true);
        let after = build_side(entry, false);
        if gaps_mode == "error" {
            if let Some(gap) = before.gaps.first() {
                return Err(gap_error(entry, true, gap));
            }
            if let Some(gap) = after.gaps.first() {
                return Err(gap_error(entry, false, gap));
            }
        }
        let added = entry
            .hunks
            .iter()
            .flat_map(|h| h.body.iter())
            .filter(|l| l.kind == Kind::Added)
            .count();
        let removed = entry
            .hunks
            .iter()
            .flat_map(|h| h.body.iter())
            .filter(|l| l.kind == Kind::Removed)
            .count();
        let last_before = entry
            .hunks
            .last()
            .map(|h| h.old_start.max(1) + h.count(true))
            .map(|e| e.saturating_sub(1))
            .unwrap_or(0);
        let last_after = entry
            .hunks
            .last()
            .map(|h| h.new_start.max(1) + h.count(false))
            .map(|e| e.saturating_sub(1))
            .unwrap_or(0);

        arr.push(json!({
            "before_path": null_if_dev_null(&entry.before_path),
            "after_path": null_if_dev_null(&entry.after_path),
            "status": entry.status(),
            "hunks": entry.hunks.len(),
            "added": added,
            "removed": removed,
            "complete": before.gaps.is_empty() && after.gaps.is_empty(),
            "reconstructed_through_before_line": last_before,
            "reconstructed_through_after_line": last_after,
            "gaps": {
                "before": gap_values(&before.gaps),
                "after": gap_values(&after.gaps),
            },
            "header_counts_corrected": entry.hunks.iter().any(|h| h.miscounted),
            "before_no_final_newline": before.no_final_nl,
            "after_no_final_newline": after.no_final_nl,
            "before": format_side(&before, gaps_mode, line_numbers),
            "after": format_side(&after, gaps_mode, line_numbers),
        }));
    }
    let doc = json!({ "files": arr });
    serde_json::to_string_pretty(&doc).map_err(|e| format!("could not serialize JSON: {e}"))
}

fn null_if_dev_null(p: &str) -> Value {
    if p.is_empty() || p == DEV_NULL {
        Value::Null
    } else {
        Value::String(p.to_string())
    }
}

fn gap_values(gaps: &[Gap]) -> Vec<Value> {
    gaps.iter()
        .map(|g| json!({ "start": g.start, "end": g.end, "lines": g.lines() }))
        .collect()
}

// ------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE: &str = "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,4 +1,5 @@\n fn main() {\n     let x = 1;\n+    let y = 2;\n     println!(\"{x}\");\n }\n";

    #[test]
    fn reconstructs_both_versions() {
        let out = extract_versions(SIMPLE, "both", "", "marker", false).unwrap();
        assert_eq!(
            out,
            "===== BEFORE: src/main.rs =====\nfn main() {\n    let x = 1;\n    println!(\"{x}\");\n}\n\
             ===== AFTER: src/main.rs =====\nfn main() {\n    let x = 1;\n    let y = 2;\n    println!(\"{x}\");\n}\n"
        );
    }

    #[test]
    fn before_only_is_byte_exact() {
        let out = extract_versions(SIMPLE, "before", "", "marker", false).unwrap();
        assert_eq!(out, "fn main() {\n    let x = 1;\n    println!(\"{x}\");\n}\n");
    }

    #[test]
    fn after_only_is_byte_exact() {
        let out = extract_versions(SIMPLE, "after", "", "marker", false).unwrap();
        assert_eq!(
            out,
            "fn main() {\n    let x = 1;\n    let y = 2;\n    println!(\"{x}\");\n}\n"
        );
    }

    #[test]
    fn empty_diff_is_an_error() {
        let err = extract_versions("   \n", "both", "", "marker", false).unwrap_err();
        assert!(err.contains("diff is empty"), "{err}");
    }

    #[test]
    fn non_unified_input_is_rejected() {
        let err = extract_versions("2c2\n< old\n---\n> new\n", "both", "", "marker", false)
            .unwrap_err();
        assert!(err.contains("no `@@` hunk headers found"), "{err}");
    }

    #[test]
    fn over_the_cap_is_rejected() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        let err = extract_versions(&big, "both", "", "marker", false).unwrap_err();
        assert!(err.contains("1000000 bytes"), "{err}");
    }

    #[test]
    fn bad_enum_values_are_named() {
        let err = extract_versions(SIMPLE, "sideways", "", "marker", false).unwrap_err();
        assert!(err.contains("output must be one of"), "{err}");
        let err = extract_versions(SIMPLE, "both", "", "fill", false).unwrap_err();
        assert!(err.contains("gaps must be one of"), "{err}");
    }

    #[test]
    fn leading_and_interior_gaps_are_marked() {
        let diff = "--- a/list.txt\n+++ b/list.txt\n@@ -3,2 +3,2 @@\n-charlie\n+CHARLIE\n delta\n@@ -8,2 +8,2 @@\n-hotel\n+HOTEL\n india\n";
        let out = extract_versions(diff, "before", "", "marker", false).unwrap();
        assert_eq!(
            out,
            "[... 2 lines not in the diff (lines 1-2) ...]\ncharlie\ndelta\n\
             [... 3 lines not in the diff (lines 5-7) ...]\nhotel\nindia\n"
        );
    }

    #[test]
    fn gaps_omit_splices_and_gaps_error_refuses() {
        let diff = "--- a/list.txt\n+++ b/list.txt\n@@ -3,2 +3,2 @@\n-charlie\n+CHARLIE\n delta\n";
        let omitted = extract_versions(diff, "before", "", "omit", false).unwrap();
        assert_eq!(omitted, "charlie\ndelta\n");
        let err = extract_versions(diff, "before", "", "error", false).unwrap_err();
        assert!(err.contains("2 lines are not carried"), "{err}");
        assert!(err.contains("-U100000"), "{err}");
    }

    #[test]
    fn line_numbers_align_with_the_gap_markers() {
        let diff = "--- a/list.txt\n+++ b/list.txt\n@@ -9,2 +9,2 @@\n-nine\n+NINE\n ten\n";
        let out = extract_versions(diff, "before", "", "marker", true).unwrap();
        assert_eq!(
            out,
            "   | [... 8 lines not in the diff (lines 1-8) ...]\n 9 | nine\n10 | ten\n"
        );
    }

    #[test]
    fn new_and_deleted_files_reconstruct_as_empty() {
        let created = "--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1,2 @@\n+alpha\n+bravo\n";
        assert_eq!(
            extract_versions(created, "before", "", "marker", false).unwrap(),
            ""
        );
        assert_eq!(
            extract_versions(created, "after", "", "marker", false).unwrap(),
            "alpha\nbravo\n"
        );
        let deleted = "--- a/old.txt\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-alpha\n-bravo\n";
        assert_eq!(
            extract_versions(deleted, "after", "", "marker", false).unwrap(),
            ""
        );
        assert_eq!(
            extract_versions(deleted, "before", "", "marker", false).unwrap(),
            "alpha\nbravo\n"
        );
    }

    #[test]
    fn no_newline_marker_is_honoured_per_side() {
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -1 +1 @@\n-one\n\\ No newline at end of file\n+one!\n";
        assert_eq!(
            extract_versions(diff, "before", "", "marker", false).unwrap(),
            "one"
        );
        assert_eq!(
            extract_versions(diff, "after", "", "marker", false).unwrap(),
            "one!\n"
        );
    }

    #[test]
    fn crlf_content_survives() {
        let diff = "--- a/f.txt\r\n+++ b/f.txt\r\n@@ -1,2 +1,2 @@\r\n one\r\n-two\r\n+TWO\r\n";
        assert_eq!(
            extract_versions(diff, "after", "", "marker", false).unwrap(),
            "one\r\nTWO\r\n"
        );
    }

    #[test]
    fn multi_file_diff_sections_every_file_and_the_filter_picks_one() {
        let diff = "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,1 +1,1 @@\n-a\n+A\ndiff --git a/src/b.rs b/src/b.rs\n--- a/src/b.rs\n+++ b/src/b.rs\n@@ -1,1 +1,1 @@\n-b\n+B\n";
        let all = extract_versions(diff, "after", "", "marker", false).unwrap();
        assert_eq!(
            all,
            "===== src/a.rs =====\nA\n===== src/b.rs =====\nB\n"
        );
        assert_eq!(
            extract_versions(diff, "after", "b.rs", "marker", false).unwrap(),
            "B\n"
        );
        assert_eq!(
            extract_versions(diff, "after", "src/*.rs", "marker", false)
                .unwrap()
                .lines()
                .count(),
            4
        );
        let err = extract_versions(diff, "after", "nope.rs", "marker", false).unwrap_err();
        assert!(err.contains("src/a.rs, src/b.rs"), "{err}");
    }

    #[test]
    fn wrong_hunk_counts_are_recounted_and_flagged() {
        // Header claims 2/2 but the body carries 3/3.
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n one\n-two\n+TWO\n three\n";
        assert_eq!(
            extract_versions(diff, "after", "", "marker", false).unwrap(),
            "one\nTWO\nthree\n"
        );
        let doc: Value =
            serde_json::from_str(&extract_versions(diff, "json", "", "marker", false).unwrap())
                .unwrap();
        assert_eq!(doc["files"][0]["header_counts_corrected"], json!(true));
    }

    #[test]
    fn json_reports_paths_counts_and_gaps() {
        let doc: Value =
            serde_json::from_str(&extract_versions(SIMPLE, "json", "", "marker", false).unwrap())
                .unwrap();
        let f = &doc["files"][0];
        assert_eq!(f["before_path"], json!("src/main.rs"));
        assert_eq!(f["status"], json!("modified"));
        assert_eq!(f["hunks"], json!(1));
        assert_eq!(f["added"], json!(1));
        assert_eq!(f["removed"], json!(0));
        assert_eq!(f["complete"], json!(true));
        assert_eq!(f["reconstructed_through_after_line"], json!(5));
        assert_eq!(f["gaps"]["before"], json!([]));
        assert!(f["before"].as_str().unwrap().starts_with("fn main() {"));
    }

    #[test]
    fn json_marks_a_created_file_and_its_gaps() {
        let diff = "diff --git a/n.txt b/n.txt\nnew file mode 100644\n--- /dev/null\n+++ b/n.txt\n@@ -0,0 +1,2 @@\n+x\n+y\n";
        let doc: Value =
            serde_json::from_str(&extract_versions(diff, "json", "", "marker", false).unwrap())
                .unwrap();
        let f = &doc["files"][0];
        assert_eq!(f["status"], json!("added"));
        assert_eq!(f["before_path"], Value::Null);
        assert_eq!(f["after_path"], json!("n.txt"));
        assert_eq!(f["before"], json!(""));
    }

    #[test]
    fn combined_and_binary_diffs_are_named_not_dropped() {
        let combined = "--- a/f.txt\n+++ b/f.txt\n@@@ -1,1 -1,1 +1,1 @@@\n- a\n +b\n";
        let err = extract_versions(combined, "both", "", "marker", false).unwrap_err();
        assert!(err.contains("combined diff"), "{err}");

        let binary = "diff --git a/logo.png b/logo.png\nindex 1234567..89abcde 100644\nBinary files a/logo.png and b/logo.png differ\n";
        let err = extract_versions(binary, "both", "", "marker", false).unwrap_err();
        assert!(err.contains("binary diff"), "{err}");
    }

    #[test]
    fn a_junk_line_inside_a_hunk_names_the_line() {
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -1,3 +1,3 @@\n one\nJUNK\n three\n";
        let err = extract_versions(diff, "both", "", "marker", false).unwrap_err();
        assert!(err.contains("line 5"), "{err}");
        assert!(err.contains("word-wrapped"), "{err}");
    }

    #[test]
    fn a_malformed_hunk_header_is_named() {
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -x,3 +1,3 @@\n one\n";
        let err = extract_versions(diff, "both", "", "marker", false).unwrap_err();
        assert!(err.contains("line 3"), "{err}");
        assert!(err.contains("could not parse the hunk header"), "{err}");
    }

    #[test]
    fn format_patch_preamble_and_signature_are_ignored() {
        let diff = "From 0123456789abcdef Mon Sep 17 00:00:00 2001\nFrom: Someone <s@example.com>\nSubject: [PATCH] tweak\n\n---\n f.txt | 2 +-\n 1 file changed\n\ndiff --git a/f.txt b/f.txt\nindex 111..222 100644\n--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n keep\n-old\n+new\n-- \n2.39.0\n\n";
        assert_eq!(
            extract_versions(diff, "after", "", "marker", false).unwrap(),
            "keep\nnew\n"
        );
    }

    #[test]
    fn rename_only_entries_report_instead_of_reconstructing() {
        let diff = "diff --git a/old.txt b/new.txt\nsimilarity index 100%\nrename from old.txt\nrename to new.txt\n";
        let err = extract_versions(diff, "both", "", "marker", false).unwrap_err();
        assert!(err.contains("no `@@` hunks"), "{err}");
        let doc: Value =
            serde_json::from_str(&extract_versions(diff, "json", "", "marker", false).unwrap())
                .unwrap();
        assert_eq!(doc["files"][0]["status"], json!("renamed"));
        assert_eq!(doc["files"][0]["hunks"], json!(0));
    }

    #[test]
    fn context_lines_stripped_of_their_trailing_space_still_parse() {
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -1,3 +1,3 @@\n one\n\n-three\n+THREE\n";
        assert_eq!(
            extract_versions(diff, "before", "", "marker", false).unwrap(),
            "one\n\nthree\n"
        );
    }

    #[test]
    fn plain_diff_u_headers_with_timestamps_parse() {
        let diff = "--- f.txt\t2026-08-17 10:00:00.000000000 +0000\n+++ f.txt\t2026-08-17 10:01:00.000000000 +0000\n@@ -1 +1 @@\n-old\n+new\n";
        let doc: Value =
            serde_json::from_str(&extract_versions(diff, "json", "", "marker", false).unwrap())
                .unwrap();
        assert_eq!(doc["files"][0]["before_path"], json!("f.txt"));
        assert_eq!(
            extract_versions(diff, "after", "", "marker", false).unwrap(),
            "new\n"
        );
    }

    #[test]
    fn globs_and_substrings_both_select() {
        assert!(glob_match("src/*.rs", "src/main.rs"));
        assert!(glob_match("*.rs", "src/main.rs"));
        assert!(!glob_match("*.toml", "src/main.rs"));
        assert!(glob_match("m?in.rs", "main.rs"));
    }
}
