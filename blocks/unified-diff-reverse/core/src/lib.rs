//! unified-diff-reverse core — pure compute, shared by the chat skill block, the
//! CLI, and the web page. No wafer/wasm-bindgen deps, no I/O, no clock: the same
//! input always produces the same bytes.
//!
//! Takes one pasted unified diff (`git diff`, `git show`, `git format-patch`,
//! `diff -u`, `svn diff`) and emits the **inverted** patch — the revert of that
//! change, matching `git diff -R` line for line (the one cosmetic difference:
//! `git diff -R` leaves each path's `a/`/`b/` letter glued to the path, so it
//! writes `--- b/f`; this writes the canonical `--- a/f`, which `git apply`
//! treats identically under `-p1`). Nothing is
//! applied here: the output is a patch, so it is the missing counterpart of
//! `apply-patch` (which applies a patch to a pasted file, and whose `reverse`
//! flag does the *apply* side of this idea) and of `diff-hunk-selector` (which
//! picks hunks out of a patch).
//!
//! Inversion is mechanical and exact:
//!
//! * every `+` body line becomes `-` and every `-` becomes `+`; context lines and
//!   the `\ No newline at end of file` markers stay, the markers moving to the
//!   side they now belong to;
//! * each `@@ -a,b +c,d @@` header becomes `@@ -c,d +a,b @@`, keeping git's
//!   implicit `,1` shorthand and the trailing section heading;
//! * git's extended headers are inverted the way `git diff -R` inverts them —
//!   `new file mode` ↔ `deleted file mode`, `old mode`/`new mode` swap,
//!   `rename`/`copy from`/`to` swap, the two `index` blob hashes swap, and the
//!   `---`/`+++` path lines (and the two sides of `diff --git`) swap.
//!
//! A `GIT binary patch` cannot be inverted from a forward-only delta, so it is
//! handled explicitly (`on_binary`) rather than silently mangled. The input's
//! line ending (LF or CRLF) and final-newline state are preserved.

use serde_json::json;

/// Hard cap on the pasted patch — 1 MB. Anything bigger is a repository dump
/// rather than a review unit, and the page keeps the whole thing in wasm memory.
pub const MAX_INPUT_BYTES: usize = 1_000_000;

/// Path shown for hunks pasted without any `diff --git` / `---` / `+++` header.
const NO_FILE: &str = "(no file header)";

// ------------------------------------------------------------------- model

#[derive(Debug, Clone)]
struct Hunk {
    /// Everything after the closing `@@` of the header, leading space included.
    heading: String,
    old_start: u64,
    old_count: u64,
    new_start: u64,
    new_count: u64,
    /// Whether the pasted header spelled the `,count` out (git omits `,1`).
    old_explicit: bool,
    new_explicit: bool,
    /// Body lines with their `+`/`-`/` ` marker, `\ No newline` lines included.
    body: Vec<String>,
}

impl Hunk {
    /// `@@ -a,b +c,d @@ heading` with the two ranges swapped.
    fn reversed_header(&self) -> String {
        let old = render_range('-', self.new_start, self.new_count, self.new_explicit);
        let new = render_range('+', self.old_start, self.old_count, self.old_explicit);
        format!("@@ {old} {new} @@{}", self.heading)
    }

    /// Body lines with `+` and `-` swapped, re-ordered the way `git diff -R`
    /// orders them: inside each run of changed lines the deletions come first,
    /// then the additions. A `\ No newline at end of file` marker belongs to the
    /// line before it, so it travels with that line to its new position.
    fn reversed_body(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::with_capacity(self.body.len());
        // The current run of changed lines, already sign-flipped and bucketed by
        // its NEW sign. `last` remembers which bucket the previous line went to
        // so a `\` marker follows the line it was attached to.
        let (mut minus, mut plus): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
        let mut last: Option<char> = None;
        for l in &self.body {
            match l.chars().next() {
                // Originally an addition — a deletion once reversed.
                Some('+') => {
                    minus.push(format!("-{}", &l[1..]));
                    last = Some('-');
                }
                // Originally a deletion — an addition once reversed.
                Some('-') => {
                    plus.push(format!("+{}", &l[1..]));
                    last = Some('+');
                }
                Some('\\') => match last {
                    Some('-') => minus.push(l.clone()),
                    Some('+') => plus.push(l.clone()),
                    _ => out.push(l.clone()),
                },
                // Context (or a context line whose trailing space was stripped)
                // ends the run.
                _ => {
                    out.append(&mut minus);
                    out.append(&mut plus);
                    out.push(l.clone());
                    last = None;
                }
            }
        }
        out.append(&mut minus);
        out.append(&mut plus);
        out
    }

    /// `+` lines in the ORIGINAL patch (they become deletions once inverted).
    fn added(&self) -> u64 {
        self.body.iter().filter(|l| l.starts_with('+')).count() as u64
    }

    fn removed(&self) -> u64 {
        self.body.iter().filter(|l| l.starts_with('-')).count() as u64
    }
}

#[derive(Debug, Clone)]
struct FileDiff {
    /// Path shown in reports — the new-side path of the original patch.
    path: String,
    /// `diff --git` / `index` / `---` / `+++` / mode / binary lines — everything
    /// from the file header up to this file's first hunk.
    preamble: Vec<String>,
    hunks: Vec<Hunk>,
    /// True when the preamble carries a `GIT binary patch` / `Binary files …`
    /// marker, which a forward-only patch cannot invert.
    binary: bool,
}

// ------------------------------------------------------------------ parsing

fn render_range(sign: char, start: u64, count: u64, explicit: bool) -> String {
    if explicit || count != 1 {
        format!("{sign}{start},{count}")
    } else {
        format!("{sign}{start}")
    }
}

fn is_file_start(lines: &[&str], i: usize) -> bool {
    let l = lines[i];
    l.starts_with("diff --git ")
        || (l.starts_with("--- ") && i + 1 < lines.len() && lines[i + 1].starts_with("+++ "))
}

/// `--- a/src/main.rs\t2024-01-01` → `src/main.rs`. Strips the timestamp column,
/// the `a/`/`b/` prefix, and surrounding quotes.
fn clean_path(raw: &str) -> String {
    let mut p = raw.split('\t').next().unwrap_or("").trim().to_string();
    if p.len() >= 2 && p.starts_with('"') && p.ends_with('"') {
        p = p[1..p.len() - 1].to_string();
    }
    for pre in ["a/", "b/", "i/", "w/", "c/", "o/"] {
        if let Some(rest) = p.strip_prefix(pre) {
            return rest.to_string();
        }
    }
    p
}

/// Prefer the new-side path (`+++`), fall back to the old side for deletions,
/// then to the `diff --git` line.
fn path_from_preamble(preamble: &[String]) -> String {
    for prefix in ["+++ ", "--- "] {
        if let Some(p) = preamble
            .iter()
            .find_map(|l| l.strip_prefix(prefix))
            .map(|s| clean_path(s))
        {
            if p != "/dev/null" && !p.is_empty() {
                return p;
            }
        }
    }
    if let Some(rest) = preamble.iter().find_map(|l| l.strip_prefix("diff --git ")) {
        // `a/x b/x` — the b-side is the last whitespace-separated token.
        if let Some(last) = rest.split_whitespace().last() {
            let p = clean_path(last);
            if !p.is_empty() {
                return p;
            }
        }
    }
    NO_FILE.to_string()
}

/// `-12,7` → (12, 7, true). A missing `,count` means 1 (git's shorthand).
fn parse_range(tok: &str) -> Option<(u64, u64, bool)> {
    let body = tok.strip_prefix('-').or_else(|| tok.strip_prefix('+'))?;
    match body.split_once(',') {
        Some((s, c)) => Some((s.parse().ok()?, c.parse().ok()?, true)),
        None => Some((body.parse().ok()?, 1, false)),
    }
}

type HunkHeader = (u64, u64, bool, u64, u64, bool, String);

fn parse_hunk_header(header: &str) -> Result<HunkHeader, String> {
    if header.starts_with("@@@") {
        return Err("combined (merge) diffs with @@@ headers cannot be reversed — \
                    re-run the diff against a single parent"
            .into());
    }
    let rest = header
        .strip_prefix("@@ ")
        .ok_or_else(|| format!("malformed hunk header: {header}"))?;
    let end = rest
        .find(" @@")
        .ok_or_else(|| format!("malformed hunk header (no closing @@): {header}"))?;
    let heading = rest[end + 3..].to_string();
    let mut toks = rest[..end].split_whitespace();
    let old = toks
        .next()
        .and_then(parse_range)
        .ok_or_else(|| format!("malformed hunk header (bad old range): {header}"))?;
    let new = toks
        .next()
        .and_then(parse_range)
        .ok_or_else(|| format!("malformed hunk header (bad new range): {header}"))?;
    Ok((old.0, old.1, old.2, new.0, new.1, new.2, heading))
}

/// Consumes one hunk starting at `lines[i]` (its `@@` header). The declared
/// counts decide where the body ends, so a following `diff --git` / `-- ` mail
/// signature / prose can't be swallowed as a deletion line.
fn parse_hunk(lines: &[&str], i: usize) -> Result<(Hunk, usize), String> {
    let (old_start, old_count, old_explicit, new_start, new_count, new_explicit, heading) =
        parse_hunk_header(lines[i])?;

    let mut body = Vec::new();
    let (mut seen_old, mut seen_new) = (0u64, 0u64);
    let mut j = i + 1;
    while j < lines.len() && (seen_old < old_count || seen_new < new_count) {
        let l = lines[j];
        // `\ No newline at end of file` belongs to the previous line, counts for
        // neither side, and must survive into the output patch.
        if l.starts_with('\\') {
            body.push(l.to_string());
            j += 1;
            continue;
        }
        match l.chars().next() {
            Some(' ') => {
                seen_old += 1;
                seen_new += 1;
            }
            Some('+') => seen_new += 1,
            Some('-') => seen_old += 1,
            // An empty line is a context line whose trailing space was stripped
            // (mail clients and copy/paste do this routinely).
            None => {
                seen_old += 1;
                seen_new += 1;
            }
            _ => break, // malformed body — stop here rather than eat the next file
        }
        body.push(l.to_string());
        j += 1;
    }
    // A trailing no-newline marker sits after the last counted line.
    while j < lines.len() && lines[j].starts_with('\\') {
        body.push(lines[j].to_string());
        j += 1;
    }

    Ok((
        Hunk {
            heading,
            old_start,
            old_count,
            new_start,
            new_count,
            old_explicit,
            new_explicit,
            body,
        },
        j,
    ))
}

fn is_binary_marker(l: &str) -> bool {
    l.starts_with("GIT binary patch")
        || (l.starts_with("Binary files ") && l.ends_with(" differ"))
        || l.starts_with("Files ") && l.ends_with(" differ")
}

fn parse_diff(lines: &[&str]) -> Result<Vec<FileDiff>, String> {
    let len = lines.len();
    let mut files: Vec<FileDiff> = Vec::new();
    let mut i = 0usize;

    while i < len {
        if is_file_start(lines, i) {
            // Preamble: the file header block up to this file's first hunk.
            let start = i;
            let mut preamble: Vec<String> = Vec::new();
            let mut seen_pair = false;
            while i < len {
                let l = lines[i];
                if l.starts_with("@@") {
                    break;
                }
                if i > start {
                    if l.starts_with("diff --git ") {
                        break;
                    }
                    if seen_pair
                        && l.starts_with("--- ")
                        && i + 1 < len
                        && lines[i + 1].starts_with("+++ ")
                    {
                        break;
                    }
                }
                if l.starts_with("--- ") && i + 1 < len && lines[i + 1].starts_with("+++ ") {
                    seen_pair = true;
                    preamble.push(l.to_string());
                    preamble.push(lines[i + 1].to_string());
                    i += 2;
                    continue;
                }
                preamble.push(l.to_string());
                i += 1;
            }
            // A `GIT binary patch` payload runs to the next file header — keep it
            // verbatim in the preamble so `on_binary = "keep"` round-trips it.
            let binary = preamble.iter().any(|l| is_binary_marker(l));
            if binary {
                while i < len && !is_file_start(lines, i) && !lines[i].starts_with("@@") {
                    preamble.push(lines[i].to_string());
                    i += 1;
                }
            }

            let mut file = FileDiff {
                path: path_from_preamble(&preamble),
                preamble,
                hunks: Vec::new(),
                binary,
            };
            while i < len && lines[i].starts_with("@@") {
                let (h, j) = parse_hunk(lines, i)?;
                file.hunks.push(h);
                i = j;
            }
            files.push(file);
        } else if lines[i].starts_with("@@") {
            // A bare hunk pasted without any file header — keep it usable.
            let (h, j) = parse_hunk(lines, i)?;
            i = j;
            match files.last_mut() {
                Some(f) if f.path == NO_FILE => f.hunks.push(h),
                _ => files.push(FileDiff {
                    path: NO_FILE.to_string(),
                    preamble: Vec::new(),
                    hunks: vec![h],
                    binary: false,
                }),
            }
        } else {
            i += 1; // commit message, mail headers, `-- ` signature, prose noise
        }
    }

    Ok(files)
}

// ---------------------------------------------------------- header inversion

/// `index 1a2b3c..4d5e6f 100644` → `index 4d5e6f..1a2b3c 100644`. The trailing
/// mode (present only when the mode did NOT change) stays put.
fn swap_index_line(l: &str) -> Option<String> {
    let rest = l.strip_prefix("index ")?;
    let (hashes, mode) = match rest.split_once(' ') {
        Some((h, m)) => (h, format!(" {m}")),
        None => (rest, String::new()),
    };
    let (old, new) = hashes.split_once("..")?;
    Some(format!("index {new}..{old}{mode}"))
}

/// `diff --git a/old b/new` → `diff --git a/new b/old`, keeping each side's
/// original prefix letter so the header still reads as a git header.
fn swap_diff_git_line(l: &str) -> Option<String> {
    let rest = l.strip_prefix("diff --git ")?;
    let toks: Vec<&str> = rest.split_whitespace().collect();
    if toks.len() != 2 {
        return None; // paths with spaces — leave the line alone rather than guess
    }
    let (a_pre, a_path) = split_prefix(toks[0]);
    let (b_pre, b_path) = split_prefix(toks[1]);
    Some(format!("diff --git {a_pre}{b_path} {b_pre}{a_path}"))
}

/// `a/src/main.rs` → (`a/`, `src/main.rs`); a path with no known prefix keeps an
/// empty prefix.
fn split_prefix(tok: &str) -> (&str, &str) {
    for pre in ["a/", "b/", "i/", "w/", "c/", "o/"] {
        if let Some(rest) = tok.strip_prefix(pre) {
            return (&tok[..pre.len()], rest);
        }
    }
    ("", tok)
}

/// Re-prefixes a path body that has moved to the other side of the patch: git
/// always writes `a/` on the `---` line and `b/` on the `+++` line, so a swapped
/// path takes its new side's letter. `/dev/null` and prefix-less `diff -u` paths
/// are left exactly as they were, and the timestamp column travels with the path.
fn retarget(body: &str, prefix: &str) -> String {
    let (path, tail) = match body.split_once('\t') {
        Some((p, t)) => (p, format!("\t{t}")),
        None => (body, String::new()),
    };
    if path.trim() == "/dev/null" {
        return body.to_string();
    }
    let (pre, rest) = split_prefix(path);
    if pre.is_empty() {
        body.to_string()
    } else {
        format!("{prefix}{rest}{tail}")
    }
}

/// Swap the two path lines' bodies, keeping each line's own `---`/`+++` marker.
fn swap_path_lines(preamble: &mut [String]) {
    let minus = preamble.iter().position(|l| l.starts_with("--- "));
    let plus = preamble.iter().position(|l| l.starts_with("+++ "));
    if let (Some(m), Some(p)) = (minus, plus) {
        let old_body = preamble[m][4..].to_string();
        let new_body = preamble[p][4..].to_string();
        preamble[m] = format!("--- {}", retarget(&new_body, "a/"));
        preamble[p] = format!("+++ {}", retarget(&old_body, "b/"));
    }
}

/// Invert one file's header block the way `git diff -R` does.
fn reverse_preamble(preamble: &[String], index_lines: IndexMode, swap_paths: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(preamble.len());
    // Paired headers swap their VALUES, not their keywords, so the emitted block
    // keeps git's canonical line order. Collect both sides of each pair first.
    let pair = |key: &str| -> Option<String> {
        preamble
            .iter()
            .find_map(|l| l.strip_prefix(key))
            .map(|s| s.to_string())
    };
    let old_mode = pair("old mode ");
    let new_mode = pair("new mode ");
    let rename_from = pair("rename from ");
    let rename_to = pair("rename to ");
    let copy_from = pair("copy from ");
    let copy_to = pair("copy to ");

    for l in preamble {
        let mapped = if let Some(mode) = l.strip_prefix("new file mode ") {
            Some(format!("deleted file mode {mode}"))
        } else if let Some(mode) = l.strip_prefix("deleted file mode ") {
            Some(format!("new file mode {mode}"))
        } else if l.starts_with("old mode ") {
            new_mode.as_ref().map(|m| format!("old mode {m}"))
        } else if l.starts_with("new mode ") {
            old_mode.as_ref().map(|m| format!("new mode {m}"))
        } else if l.starts_with("rename from ") {
            rename_to.as_ref().map(|p| format!("rename from {p}"))
        } else if l.starts_with("rename to ") {
            rename_from.as_ref().map(|p| format!("rename to {p}"))
        } else if l.starts_with("copy from ") {
            copy_to.as_ref().map(|p| format!("copy from {p}"))
        } else if l.starts_with("copy to ") {
            copy_from.as_ref().map(|p| format!("copy to {p}"))
        } else if l.starts_with("index ") {
            match index_lines {
                IndexMode::Swap => swap_index_line(l),
                IndexMode::Keep => None,
                IndexMode::Drop => continue,
            }
        } else if swap_paths && l.starts_with("diff --git ") {
            swap_diff_git_line(l)
        } else {
            None
        };
        out.push(mapped.unwrap_or_else(|| l.clone()));
    }
    if swap_paths {
        swap_path_lines(&mut out);
    }
    out
}

// ------------------------------------------------------------------ options

#[derive(Debug, Clone, Copy, PartialEq)]
enum IndexMode {
    Swap,
    Keep,
    Drop,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BinaryMode {
    Fail,
    Skip,
    Keep,
}

fn parse_index_mode(s: &str) -> Result<IndexMode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "swap" => Ok(IndexMode::Swap),
        "keep" => Ok(IndexMode::Keep),
        "drop" => Ok(IndexMode::Drop),
        other => Err(format!(
            "unknown index_lines '{other}' — use 'swap', 'keep' or 'drop'"
        )),
    }
}

fn parse_binary_mode(s: &str) -> Result<BinaryMode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "fail" => Ok(BinaryMode::Fail),
        "skip" => Ok(BinaryMode::Skip),
        "keep" => Ok(BinaryMode::Keep),
        other => Err(format!(
            "unknown on_binary '{other}' — use 'fail', 'skip' or 'keep'"
        )),
    }
}

/// Matches the `file` selector against a parsed path: exact, suffix-on-a-path-
/// boundary, or basename. Keeps the selector forgiving about `a/`/`b/` prefixes.
fn file_matches(path: &str, want: &str) -> bool {
    let want = clean_path(want);
    if want.is_empty() || path == want {
        return true;
    }
    if let Some(rest) = path.strip_suffix(&want) {
        if rest.is_empty() || rest.ends_with('/') {
            return true;
        }
    }
    path.rsplit('/').next() == Some(want.as_str())
}

// ----------------------------------------------------------------- rendering

struct Stats {
    files: usize,
    hunks: usize,
    /// `+` lines in the ORIGINAL patch — deletions once reversed.
    added: u64,
    removed: u64,
}

fn stats(files: &[FileDiff]) -> Stats {
    Stats {
        files: files.len(),
        hunks: files.iter().map(|f| f.hunks.len()).sum(),
        added: files
            .iter()
            .flat_map(|f| f.hunks.iter())
            .map(|h| h.added())
            .sum(),
        removed: files
            .iter()
            .flat_map(|f| f.hunks.iter())
            .map(|h| h.removed())
            .sum(),
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

fn render_patch(files: &[FileDiff], index_lines: IndexMode, swap_paths: bool) -> Vec<String> {
    let mut out = Vec::new();
    for f in files {
        out.extend(reverse_preamble(&f.preamble, index_lines, swap_paths));
        for h in &f.hunks {
            out.push(h.reversed_header());
            out.extend(h.reversed_body());
        }
    }
    out
}

fn render_summary(files: &[FileDiff], warnings: &[String]) -> String {
    let s = stats(files);
    let mut out = String::new();
    out.push_str(&format!(
        "Reverse patch · {} · {} · +{} −{} (original was +{} −{})\n",
        plural(s.files, "file", "files"),
        plural(s.hunks, "hunk", "hunks"),
        s.removed,
        s.added,
        s.added,
        s.removed
    ));
    for f in files {
        let fa: u64 = f.hunks.iter().map(|h| h.added()).sum();
        let fr: u64 = f.hunks.iter().map(|h| h.removed()).sum();
        out.push_str(&format!(
            "\n{} · {}{} · +{fr} −{fa}\n",
            f.path,
            plural(f.hunks.len(), "hunk", "hunks"),
            if f.binary { " · binary" } else { "" }
        ));
        for h in &f.hunks {
            out.push_str(&format!(
                "  {}  (was @@ {} {} @@)\n",
                h.reversed_header().trim_end(),
                render_range('-', h.old_start, h.old_count, h.old_explicit),
                render_range('+', h.new_start, h.new_count, h.new_explicit),
            ));
        }
    }
    for w in warnings {
        out.push_str(&format!("\nwarning: {w}\n"));
    }
    out
}

fn render_json(
    files: &[FileDiff],
    patch: &str,
    warnings: &[String],
    index_lines: IndexMode,
    swap_paths: bool,
) -> String {
    let s = stats(files);
    let per_file: Vec<_> = files
        .iter()
        .map(|f| {
            let fa: u64 = f.hunks.iter().map(|h| h.added()).sum();
            let fr: u64 = f.hunks.iter().map(|h| h.removed()).sum();
            json!({
                "path": f.path,
                "hunks": f.hunks.len(),
                "binary": f.binary,
                // Post-inversion counts: the original's additions are now removals.
                "added": fr,
                "removed": fa,
                "original_added": fa,
                "original_removed": fr,
                "headers": f.hunks.iter().map(|h| h.reversed_header()).collect::<Vec<_>>(),
            })
        })
        .collect();
    let value = json!({
        "files": s.files,
        "hunks": s.hunks,
        "added": s.removed,
        "removed": s.added,
        "original_added": s.added,
        "original_removed": s.removed,
        "index_lines": match index_lines {
            IndexMode::Swap => "swap",
            IndexMode::Keep => "keep",
            IndexMode::Drop => "drop",
        },
        "swap_paths": swap_paths,
        "warnings": warnings,
        "per_file": per_file,
        "patch": patch,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

// -------------------------------------------------------------------- entry

/// Reverse a pasted unified diff.
///
/// * `diff` — the patch text (1 MB cap).
/// * `output` — `patch` (default) | `summary` | `json`.
/// * `file` — restrict a multi-file patch to one file (blank = every file).
/// * `index_lines` — `swap` (default) | `keep` | `drop` for git `index` lines.
/// * `swap_paths` — swap the `---`/`+++` (and `diff --git`) path sides.
/// * `on_binary` — `fail` (default) | `skip` | `keep` for binary file sections.
pub fn reverse_diff(
    diff: &str,
    output: &str,
    file: &str,
    index_lines: &str,
    swap_paths: bool,
    on_binary: &str,
) -> Result<String, String> {
    if diff.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "patch is {} bytes — over the {} byte (1 MB) limit; reverse it one file at a time",
            diff.len(),
            MAX_INPUT_BYTES
        ));
    }
    if diff.trim().is_empty() {
        return Err("no patch supplied — paste a unified diff (the output of `git diff`, \
                    `git show`, `git format-patch` or `diff -u`)"
            .into());
    }
    let index_lines = parse_index_mode(index_lines)?;
    let on_binary = parse_binary_mode(on_binary)?;
    let output = {
        let o = output.trim().to_ascii_lowercase();
        if o.is_empty() {
            "patch".to_string()
        } else {
            o
        }
    };
    if !matches!(output.as_str(), "patch" | "summary" | "json") {
        return Err(format!(
            "unknown output '{output}' — use 'patch', 'summary' or 'json'"
        ));
    }

    // Line endings: a CRLF patch keeps CRLF, and the final-newline state is
    // preserved, so a reversed patch is a drop-in replacement for the original.
    let crlf = diff
        .find('\n')
        .is_some_and(|n| n > 0 && diff.as_bytes()[n - 1] == b'\r');
    let normalized = diff.replace("\r\n", "\n");
    let trailing_newline = normalized.ends_with('\n');
    let body = normalized.strip_suffix('\n').unwrap_or(&normalized);
    let lines: Vec<&str> = body.split('\n').collect();

    let mut files = parse_diff(&lines)?;
    if files.is_empty() {
        return Err("no unified-diff hunks found — a patch needs at least one `@@ -a,b +c,d @@` \
                    header; check that the paste wasn't reflowed or truncated"
            .into());
    }

    if !file.trim().is_empty() {
        let want = file.trim();
        let before = files.len();
        files.retain(|f| file_matches(&f.path, want));
        if files.is_empty() {
            return Err(format!(
                "no file matching '{want}' in this patch ({before} file(s) present) — \
                 use the path as it appears after `+++ b/`"
            ));
        }
    }

    let mut warnings: Vec<String> = Vec::new();
    if files.iter().any(|f| f.binary) {
        match on_binary {
            BinaryMode::Fail => {
                let names: Vec<&str> = files
                    .iter()
                    .filter(|f| f.binary)
                    .map(|f| f.path.as_str())
                    .collect();
                return Err(format!(
                    "binary patch for {} cannot be reversed — a forward-only binary delta \
                     carries no reverse delta. Re-run `git diff -R --binary` on the source \
                     repository, or set on_binary='skip' to drop the binary file(s) or \
                     'keep' to pass them through unchanged",
                    names.join(", ")
                ));
            }
            BinaryMode::Skip => {
                for f in files.iter().filter(|f| f.binary) {
                    warnings.push(format!("skipped binary file section for {}", f.path));
                }
                files.retain(|f| !f.binary);
                if files.is_empty() {
                    return Err(
                        "every file in this patch is binary — nothing left to reverse".into()
                    );
                }
            }
            BinaryMode::Keep => {
                for f in files.iter().filter(|f| f.binary) {
                    warnings.push(format!(
                        "binary file section for {} passed through UNCHANGED — it will not \
                         revert the change",
                        f.path
                    ));
                }
            }
        }
    }

    let out_lines = render_patch(&files, index_lines, swap_paths);
    let mut patch = out_lines.join("\n");
    if trailing_newline {
        patch.push('\n');
    }
    if crlf {
        patch = patch.replace('\n', "\r\n");
    }

    Ok(match output.as_str() {
        "summary" => render_summary(&files, &warnings),
        "json" => render_json(&files, &patch, &warnings, index_lines, swap_paths),
        _ => patch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE: &str = "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,4 +1,5 @@\n fn main() {\n     let x = 1;\n+    let y = 2;\n     println!(\"{x}\");\n }\n";

    #[test]
    fn reverses_a_simple_patch() {
        let out = reverse_diff(SIMPLE, "patch", "", "swap", true, "fail").unwrap();
        assert_eq!(
            out,
            "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,5 +1,4 @@\n fn main() {\n     let x = 1;\n-    let y = 2;\n     println!(\"{x}\");\n }\n"
        );
    }

    #[test]
    fn reversing_twice_is_the_identity() {
        let once = reverse_diff(SIMPLE, "patch", "", "swap", true, "fail").unwrap();
        let twice = reverse_diff(&once, "patch", "", "swap", true, "fail").unwrap();
        assert_eq!(twice, SIMPLE);
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = reverse_diff("   \n", "patch", "", "swap", true, "fail").unwrap_err();
        assert!(err.contains("no patch supplied"), "{err}");
    }

    #[test]
    fn prose_without_hunks_is_an_error() {
        let err = reverse_diff("just some notes\nabout a change\n", "patch", "", "swap", true, "fail")
            .unwrap_err();
        assert!(err.contains("no unified-diff hunks found"), "{err}");
    }

    #[test]
    fn over_the_cap_is_an_error() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        let err = reverse_diff(&big, "patch", "", "swap", true, "fail").unwrap_err();
        assert!(err.contains("over the 1000000 byte"), "{err}");
    }

    #[test]
    fn combined_merge_diffs_are_rejected() {
        let err = reverse_diff(
            "--- a/f\n+++ b/f\n@@@ -1,2 -1,2 +1,2 @@@\n  a\n",
            "patch",
            "",
            "swap",
            true,
            "fail",
        )
        .unwrap_err();
        assert!(err.contains("combined (merge) diffs"), "{err}");
    }

    #[test]
    fn git_headers_invert_like_git_diff_r() {
        let p = "diff --git a/old.txt b/new.txt\nsimilarity index 90%\nrename from old.txt\nrename to new.txt\nindex 1111111..2222222 100644\n--- a/old.txt\n+++ b/new.txt\n@@ -1 +1 @@\n-one\n+two\n";
        let out = reverse_diff(p, "patch", "", "swap", true, "fail").unwrap();
        assert_eq!(
            out,
            "diff --git a/new.txt b/old.txt\nsimilarity index 90%\nrename from new.txt\nrename to old.txt\nindex 2222222..1111111 100644\n--- a/new.txt\n+++ b/old.txt\n@@ -1 +1 @@\n-two\n+one\n"
        );
    }

    #[test]
    fn new_file_becomes_deleted_file() {
        let p = "diff --git a/x.txt b/x.txt\nnew file mode 100644\nindex 0000000..83db48f\n--- /dev/null\n+++ b/x.txt\n@@ -0,0 +1 @@\n+hello\n";
        let out = reverse_diff(p, "patch", "", "swap", true, "fail").unwrap();
        assert!(out.contains("deleted file mode 100644"), "{out}");
        assert!(out.contains("--- a/x.txt\n+++ /dev/null\n"), "{out}");
        assert!(out.contains("@@ -1 +0,0 @@\n-hello\n"), "{out}");
    }

    #[test]
    fn mode_change_swaps_both_values() {
        let p = "diff --git a/s.sh b/s.sh\nold mode 100644\nnew mode 100755\n--- a/s.sh\n+++ b/s.sh\n@@ -1 +1 @@\n-a\n+b\n";
        let out = reverse_diff(p, "patch", "", "swap", true, "fail").unwrap();
        assert!(out.contains("old mode 100755\nnew mode 100644\n"), "{out}");
    }

    #[test]
    fn index_lines_can_be_kept_or_dropped() {
        let p = "diff --git a/f b/f\nindex aaa..bbb 100644\n--- a/f\n+++ b/f\n@@ -1 +1 @@\n-a\n+b\n";
        let kept = reverse_diff(p, "patch", "", "keep", true, "fail").unwrap();
        assert!(kept.contains("index aaa..bbb 100644"), "{kept}");
        let dropped = reverse_diff(p, "patch", "", "drop", true, "fail").unwrap();
        assert!(!dropped.contains("index "), "{dropped}");
    }

    #[test]
    fn swap_paths_false_keeps_the_header_paths() {
        let p = "--- a/old.txt\n+++ b/new.txt\n@@ -1 +1 @@\n-one\n+two\n";
        let out = reverse_diff(p, "patch", "", "swap", false, "fail").unwrap();
        assert!(out.starts_with("--- a/old.txt\n+++ b/new.txt\n"), "{out}");
    }

    #[test]
    fn no_newline_marker_survives() {
        let p = "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-a\n\\ No newline at end of file\n+b\n";
        let out = reverse_diff(p, "patch", "", "swap", true, "fail").unwrap();
        assert_eq!(
            out,
            "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-b\n+a\n\\ No newline at end of file\n"
        );
    }

    #[test]
    fn crlf_input_stays_crlf() {
        let p = "--- a/f\r\n+++ b/f\r\n@@ -1 +1 @@\r\n-a\r\n+b\r\n";
        let out = reverse_diff(p, "patch", "", "swap", true, "fail").unwrap();
        assert_eq!(out, "--- a/f\r\n+++ b/f\r\n@@ -1 +1 @@\r\n-b\r\n+a\r\n");
    }

    #[test]
    fn file_selector_picks_one_file_of_many() {
        let p = "--- a/one.txt\n+++ b/one.txt\n@@ -1 +1 @@\n-a\n+b\n--- a/dir/two.txt\n+++ b/dir/two.txt\n@@ -1 +1 @@\n-c\n+d\n";
        let out = reverse_diff(p, "patch", "two.txt", "swap", true, "fail").unwrap();
        assert!(out.contains("dir/two.txt"), "{out}");
        assert!(!out.contains("one.txt"), "{out}");
        let err = reverse_diff(p, "patch", "nope.txt", "swap", true, "fail").unwrap_err();
        assert!(err.contains("no file matching 'nope.txt'"), "{err}");
    }

    #[test]
    fn binary_sections_fail_skip_or_pass_through() {
        let p = "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-a\n+b\ndiff --git a/img.png b/img.png\nindex 111..222 100644\nGIT binary patch\nliteral 4\nzcmZ\n";
        let err = reverse_diff(p, "patch", "", "swap", true, "fail").unwrap_err();
        assert!(err.contains("binary patch for img.png"), "{err}");

        let skipped = reverse_diff(p, "patch", "", "swap", true, "skip").unwrap();
        assert!(!skipped.contains("GIT binary patch"), "{skipped}");
        assert!(skipped.contains("a.txt"), "{skipped}");

        let kept = reverse_diff(p, "patch", "", "swap", true, "keep").unwrap();
        assert!(kept.contains("GIT binary patch\nliteral 4\nzcmZ"), "{kept}");
    }

    #[test]
    fn summary_reports_swapped_counts() {
        let out = reverse_diff(SIMPLE, "summary", "", "swap", true, "fail").unwrap();
        assert!(
            out.starts_with("Reverse patch · 1 file · 1 hunk · +0 −1 (original was +1 −0)\n"),
            "{out}"
        );
        assert!(out.contains("src/main.rs · 1 hunk · +0 −1"), "{out}");
        assert!(out.contains("@@ -1,5 +1,4 @@  (was @@ -1,4 +1,5 @@)"), "{out}");
    }

    #[test]
    fn json_carries_the_patch_and_swapped_counts() {
        let out = reverse_diff(SIMPLE, "json", "", "swap", true, "fail").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["files"], 1);
        assert_eq!(v["hunks"], 1);
        assert_eq!(v["added"], 0);
        assert_eq!(v["removed"], 1);
        assert_eq!(v["original_added"], 1);
        assert_eq!(v["per_file"][0]["path"], "src/main.rs");
        assert_eq!(v["per_file"][0]["headers"][0], "@@ -1,5 +1,4 @@");
        assert!(v["patch"].as_str().unwrap().contains("-    let y = 2;"));
    }

    #[test]
    fn bare_hunk_without_a_file_header_still_reverses() {
        let out = reverse_diff("@@ -1,2 +1,2 @@\n-a\n+b\n c\n", "patch", "", "swap", true, "fail")
            .unwrap();
        assert_eq!(out, "@@ -1,2 +1,2 @@\n-b\n+a\n c\n");
    }

    #[test]
    fn unknown_option_values_are_errors() {
        assert!(reverse_diff(SIMPLE, "nope", "", "swap", true, "fail")
            .unwrap_err()
            .contains("unknown output"));
        assert!(reverse_diff(SIMPLE, "patch", "", "nope", true, "fail")
            .unwrap_err()
            .contains("unknown index_lines"));
        assert!(reverse_diff(SIMPLE, "patch", "", "swap", true, "nope")
            .unwrap_err()
            .contains("unknown on_binary"));
    }
}
