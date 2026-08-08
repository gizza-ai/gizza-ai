//! diff-hunk-selector core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps, no I/O, no clock: the same input always
//! renders the same bytes.
//!
//! Takes a pasted unified / `git diff` patch, numbers every hunk globally (1-based,
//! `lsdiff`-style), and either lists them, filters them down to a smaller *valid*
//! patch, splits them into one standalone patch per hunk, or reports the inventory
//! as JSON.
//!
//! ```text
//! 1 file · 2 hunks · +3 −1
//!
//! src/main.rs · 2 hunks · +3 −1
//!   [1] @@ -1,3 +1,4 @@ fn main()  +1 −0
//!   [2] @@ -10,3 +11,3 @@          +2 −1
//! ```
//!
//! Selection uses the established CLI span grammar — `all`, `1,3-5`, open-ended
//! `4-` and `-2` — and can be inverted, narrowed to files by glob, or narrowed to
//! hunks touching a span of original-file line numbers. When hunks are dropped,
//! the new-side start of each kept hunk is shifted by the net line delta of the
//! dropped hunks before it in the same file, so the emitted patch still applies.

use serde_json::json;

/// Hard cap on the pasted patch — 1 MB. Bigger patches are a repo dump, not a
/// review unit, and the page keeps the whole thing in wasm linear memory.
pub const MAX_INPUT_BYTES: usize = 1_000_000;

/// Path shown for hunks pasted without any `diff --git` / `---` / `+++` header.
const NO_FILE: &str = "(no file header)";

// ------------------------------------------------------------------- model

#[derive(Debug, Clone)]
struct Hunk {
    /// The `@@ … @@ …` line exactly as pasted — re-emitted verbatim when the
    /// new-side start doesn't move, so a no-op run is byte-identical.
    raw_header: String,
    /// Everything after the closing `@@`, leading space included.
    heading: String,
    old_start: u64,
    old_count: u64,
    new_start: u64,
    new_count: u64,
    /// Whether the pasted header spelled the `,count` out (git omits `,1`).
    old_explicit: bool,
    new_explicit: bool,
    body: Vec<String>,
    added: u64,
    removed: u64,
    /// 1-based index across the whole patch.
    index: usize,
}

#[derive(Debug, Clone)]
struct FileDiff {
    path: String,
    /// `diff --git` / `index` / `---` / `+++` / mode / binary lines — everything
    /// from the file header up to its first hunk.
    preamble: Vec<String>,
    hunks: Vec<Hunk>,
}

// ------------------------------------------------------------------ parsing

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
    let plus = preamble
        .iter()
        .find_map(|l| l.strip_prefix("+++ "))
        .map(clean_path);
    if let Some(p) = plus {
        if p != "/dev/null" && !p.is_empty() {
            return p;
        }
    }
    let minus = preamble
        .iter()
        .find_map(|l| l.strip_prefix("--- "))
        .map(clean_path);
    if let Some(p) = minus {
        if p != "/dev/null" && !p.is_empty() {
            return p;
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

fn parse_hunk_header(header: &str) -> Result<(u64, u64, bool, u64, u64, bool, String), String> {
    if header.starts_with("@@@") {
        return Err("combined (merge) diffs with @@@ headers are not supported — \
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
fn parse_hunk(lines: &[&str], i: usize, index: usize) -> Result<(Hunk, usize), String> {
    let raw_header = lines[i].to_string();
    let (old_start, old_count, old_explicit, new_start, new_count, new_explicit, heading) =
        parse_hunk_header(&raw_header)?;

    let mut body = Vec::new();
    let (mut seen_old, mut seen_new, mut added, mut removed) = (0u64, 0u64, 0u64, 0u64);
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
            Some('+') => {
                seen_new += 1;
                added += 1;
            }
            Some('-') => {
                seen_old += 1;
                removed += 1;
            }
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
            raw_header,
            heading,
            old_start,
            old_count,
            new_start,
            new_count,
            old_explicit,
            new_explicit,
            body,
            added,
            removed,
            index,
        },
        j,
    ))
}

fn parse_diff(text: &str) -> Result<Vec<FileDiff>, String> {
    let normalized = text.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let len = lines.len();

    let mut files: Vec<FileDiff> = Vec::new();
    let mut next_index = 1usize;
    let mut i = 0usize;

    while i < len {
        if is_file_start(&lines, i) {
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

            let mut file = FileDiff {
                path: path_from_preamble(&preamble),
                preamble,
                hunks: Vec::new(),
            };
            while i < len && lines[i].starts_with("@@") {
                let (h, j) = parse_hunk(&lines, i, next_index)?;
                next_index += 1;
                file.hunks.push(h);
                i = j;
            }
            files.push(file);
        } else if lines[i].starts_with("@@") {
            // A bare hunk pasted without any file header — keep it usable.
            let (h, j) = parse_hunk(&lines, i, next_index)?;
            next_index += 1;
            i = j;
            match files.last_mut() {
                Some(f) if f.path == NO_FILE => f.hunks.push(h),
                _ => files.push(FileDiff {
                    path: NO_FILE.to_string(),
                    preamble: Vec::new(),
                    hunks: vec![h],
                }),
            }
        } else {
            i += 1; // commit message, mail headers, `-- ` signature, prose noise
        }
    }

    Ok(files)
}

// ----------------------------------------------------------------- selection

/// `None` means "no constraint" (`all`, `*`, or blank).
type Spans = Option<Vec<(u64, u64)>>;

fn parse_spans(spec: &str, what: &str) -> Result<Spans, String> {
    let spec = spec.trim();
    if spec.is_empty() || spec.eq_ignore_ascii_case("all") || spec == "*" {
        return Ok(None);
    }
    let mut spans = Vec::new();
    for raw in spec.split(',') {
        let item = raw.trim();
        if item.is_empty() {
            continue;
        }
        let num = |s: &str| -> Result<u64, String> {
            s.trim()
                .parse::<u64>()
                .map_err(|_| invalid_item(what, item))
                .and_then(|n| {
                    if n == 0 {
                        Err(format!("invalid {what} selection '{item}' — numbering starts at 1"))
                    } else {
                        Ok(n)
                    }
                })
        };
        // A leading `-` is an open low end (`-3` = 1 through 3), not a negative.
        let span = if let Some(hi) = item.strip_prefix('-') {
            (1, num(hi)?)
        } else if let Some(lo) = item.strip_suffix('-') {
            (num(lo)?, u64::MAX)
        } else if let Some((lo, hi)) = item.split_once('-') {
            let (lo, hi) = (num(lo)?, num(hi)?);
            if lo > hi {
                return Err(format!(
                    "invalid {what} range '{item}' — the start is greater than the end"
                ));
            }
            (lo, hi)
        } else {
            let n = num(item)?;
            (n, n)
        };
        spans.push(span);
    }
    if spans.is_empty() {
        return Err(format!(
            "invalid {what} selection '{spec}' — it selects nothing; use 'all', '2', '1,3-5', '4-' or '-2'"
        ));
    }
    Ok(Some(spans))
}

fn invalid_item(what: &str, item: &str) -> String {
    format!(
        "invalid {what} selection '{item}' — use numbers (2), ranges (3-5), open ranges (4- or -2), \
         a comma list (1,3-5), or 'all'"
    )
}

fn in_spans(spans: &Spans, n: u64) -> bool {
    match spans {
        None => true,
        Some(list) => list.iter().any(|(lo, hi)| n >= *lo && n <= *hi),
    }
}

/// `*` matches any run of characters (path separators included), `?` any single
/// character. Iterative backtracking — no regex dependency.
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
            pi += 1;
            mark = ti;
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

/// Full-path match, plus a basename match for slash-free patterns so `main.rs`
/// finds `src/bin/main.rs` the way people expect.
fn path_matches(pat: &str, path: &str) -> bool {
    if glob_match(pat, path) {
        return true;
    }
    !pat.contains('/') && glob_match(pat, path.rsplit('/').next().unwrap_or(path))
}

fn file_selected(spec: &str, path: &str) -> bool {
    let items: Vec<&str> = spec
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if items.is_empty() {
        return true;
    }
    let mut has_include = false;
    let mut included = false;
    for item in items {
        match item.strip_prefix('!') {
            // An exclude wins wherever it appears in the list.
            Some(ex) => {
                if path_matches(ex.trim(), path) {
                    return false;
                }
            }
            None => {
                has_include = true;
                if path_matches(item, path) {
                    included = true;
                }
            }
        }
    }
    !has_include || included
}

/// Original-file span a hunk touches. A pure-addition hunk has `old_count == 0`;
/// it still sits at one position, so treat it as a single line.
fn old_span(h: &Hunk) -> (u64, u64) {
    let start = h.old_start.max(1);
    (start, start + h.old_count.max(1) - 1)
}

// ----------------------------------------------------------------- rendering

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("{n} {word}")
    } else {
        format!("{n} {word}s")
    }
}

/// `[1, 2, 3, 7]` → `1-3, 7`.
fn format_ranges(nums: &[usize]) -> String {
    if nums.is_empty() {
        return "none".into();
    }
    let mut out: Vec<String> = Vec::new();
    let mut lo = nums[0];
    let mut hi = nums[0];
    for &n in &nums[1..] {
        if n == hi + 1 {
            hi = n;
        } else {
            out.push(if lo == hi {
                lo.to_string()
            } else {
                format!("{lo}-{hi}")
            });
            lo = n;
            hi = n;
        }
    }
    out.push(if lo == hi {
        lo.to_string()
    } else {
        format!("{lo}-{hi}")
    });
    out.join(", ")
}

/// The `@@ … @@ …` line for a kept hunk, rebuilt only when the new-side start
/// actually moved (so an unfiltered re-emit is byte-identical to the input).
fn header_line(h: &Hunk, new_start: u64) -> String {
    if new_start == h.new_start {
        return h.raw_header.clone();
    }
    let old = if h.old_explicit {
        format!("{},{}", h.old_start, h.old_count)
    } else {
        h.old_start.to_string()
    };
    let new = if h.new_explicit {
        format!("{},{}", new_start, h.new_count)
    } else {
        new_start.to_string()
    };
    format!("@@ -{} +{} @@{}", old, new, h.heading)
}

/// Emits a complete patch containing exactly the hunks flagged in `keep`
/// (indexed by `hunk.index - 1`), with each kept file's preamble.
fn render_patch(files: &[FileDiff], keep: &[bool], renumber: bool) -> String {
    let mut out = String::new();
    for f in files {
        if !f.hunks.iter().any(|h| keep[h.index - 1]) {
            continue;
        }
        for l in &f.preamble {
            out.push_str(l);
            out.push('\n');
        }
        // Net new-side lines contributed by the dropped hunks before this point.
        let mut delta: i64 = 0;
        for h in &f.hunks {
            if !keep[h.index - 1] {
                delta += h.new_count as i64 - h.old_count as i64;
                continue;
            }
            let new_start = if renumber {
                let floor = if h.new_count == 0 { 0 } else { 1 };
                (h.new_start as i64 - delta).max(floor) as u64
            } else {
                h.new_start
            };
            out.push_str(&header_line(h, new_start));
            out.push('\n');
            for l in &h.body {
                out.push_str(l);
                out.push('\n');
            }
        }
    }
    out
}

fn render_list(files: &[FileDiff], keep: &[bool], filtered: bool) -> String {
    let all: Vec<&Hunk> = files.iter().flat_map(|f| f.hunks.iter()).collect();
    let total = all.len();
    let added: u64 = all.iter().map(|h| h.added).sum();
    let removed: u64 = all.iter().map(|h| h.removed).sum();
    let idx_width = total.to_string().len();
    let header_width = all.iter().map(|h| h.raw_header.len()).max().unwrap_or(0);

    let mut out = format!(
        "{} · {} · +{} −{}\n",
        plural(files.len(), "file"),
        plural(total, "hunk"),
        added,
        removed
    );

    for f in files {
        out.push('\n');
        if f.hunks.is_empty() {
            out.push_str(&format!(
                "{} · no textual hunks (binary, rename, or mode change only)\n",
                f.path
            ));
            continue;
        }
        let fa: u64 = f.hunks.iter().map(|h| h.added).sum();
        let fr: u64 = f.hunks.iter().map(|h| h.removed).sum();
        out.push_str(&format!(
            "{} · {} · +{} −{}\n",
            f.path,
            plural(f.hunks.len(), "hunk"),
            fa,
            fr
        ));
        for h in &f.hunks {
            let mark = if filtered && keep[h.index - 1] { "*" } else { " " };
            out.push_str(&format!(
                "{}[{:>width$}] {:<hw$}  +{} −{}\n",
                mark,
                h.index,
                h.raw_header,
                h.added,
                h.removed,
                width = idx_width,
                hw = header_width
            ));
        }
    }

    if filtered {
        let picked: Vec<usize> = all
            .iter()
            .filter(|h| keep[h.index - 1])
            .map(|h| h.index)
            .collect();
        out.push_str(&format!(
            "\nSelected (*): {} — {} of {}\n",
            format_ranges(&picked),
            picked.len(),
            total
        ));
    }

    out.trim_end().to_string()
}

fn render_split(files: &[FileDiff], keep: &[bool], renumber: bool) -> String {
    let picked: Vec<(&str, &Hunk)> = files
        .iter()
        .flat_map(|f| f.hunks.iter().map(move |h| (f.path.as_str(), h)))
        .filter(|(_, h)| keep[h.index - 1])
        .collect();
    let n = picked.len();
    let mut out = String::new();
    for (i, (path, h)) in picked.iter().enumerate() {
        let mut solo = vec![false; keep.len()];
        solo[h.index - 1] = true;
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!(
            "==== patch {} of {} · hunk [{}] · {} ====\n",
            i + 1,
            n,
            h.index,
            path
        ));
        out.push_str(&render_patch(files, &solo, renumber));
    }
    out
}

fn render_json(
    files: &[FileDiff],
    keep: &[bool],
    hunks: &str,
    invert: bool,
    files_spec: &str,
    lines: &str,
    renumber: bool,
) -> Result<String, String> {
    let all: Vec<&Hunk> = files.iter().flat_map(|f| f.hunks.iter()).collect();
    let file_json: Vec<_> = files
        .iter()
        .map(|f| {
            json!({
                "path": f.path,
                "hunks": f.hunks.iter().map(|h| h.index).collect::<Vec<_>>(),
                "selected": f.hunks.iter().filter(|h| keep[h.index - 1]).map(|h| h.index).collect::<Vec<_>>(),
                "added": f.hunks.iter().map(|h| h.added).sum::<u64>(),
                "removed": f.hunks.iter().map(|h| h.removed).sum::<u64>(),
            })
        })
        .collect();
    let hunk_json: Vec<_> = files
        .iter()
        .flat_map(|f| {
            f.hunks.iter().map(move |h| {
                json!({
                    "n": h.index,
                    "file": f.path,
                    "header": h.raw_header,
                    "heading": h.heading.trim(),
                    "old_start": h.old_start,
                    "old_count": h.old_count,
                    "new_start": h.new_start,
                    "new_count": h.new_count,
                    "added": h.added,
                    "removed": h.removed,
                    "selected": keep[h.index - 1],
                })
            })
        })
        .collect();
    let picked: Vec<usize> = all
        .iter()
        .filter(|h| keep[h.index - 1])
        .map(|h| h.index)
        .collect();

    let doc = json!({
        "totals": {
            "files": files.len(),
            "hunks": all.len(),
            "added": all.iter().map(|h| h.added).sum::<u64>(),
            "removed": all.iter().map(|h| h.removed).sum::<u64>(),
            "selected": picked.len(),
        },
        "selection": {
            "hunks": hunks,
            "invert": invert,
            "files": files_spec,
            "lines": lines,
            "renumber": renumber,
            "selected": picked,
        },
        "files": file_json,
        "hunks": hunk_json,
    });
    serde_json::to_string_pretty(&doc).map_err(|e| format!("could not serialize JSON: {e}"))
}

// --------------------------------------------------------------------- entry

/// Parse `diff`, apply the selection, and render it as `output`.
///
/// * `output` — `list` (blank/default) | `patch` | `split` | `json`
/// * `hunks`  — `all` (blank/default), `2`, `1,3-5`, `4-`, `-2`
/// * `invert` — keep everything the `hunks` spec does *not* name
/// * `files`  — comma-separated globs; a `!` prefix excludes
/// * `lines`  — same span grammar, matched against original-file line numbers
/// * `renumber` — shift kept hunks' new-side starts by the dropped hunks' net delta
pub fn select_hunks(
    diff: &str,
    output: &str,
    hunks: &str,
    invert: bool,
    files_spec: &str,
    lines: &str,
    renumber: bool,
) -> Result<String, String> {
    let output = match output.trim() {
        "" => "list",
        o => o,
    };
    if !matches!(output, "list" | "patch" | "split" | "json") {
        return Err(format!(
            "unknown output '{output}' — use list, patch, split, or json"
        ));
    }
    if diff.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "diff is too large: {} bytes (cap {} bytes / 1 MB) — split the patch or narrow it with `git diff -- <path>`",
            diff.len(),
            MAX_INPUT_BYTES
        ));
    }
    if diff.trim().is_empty() {
        return Err("no diff input — paste a unified diff or the output of `git diff`".into());
    }

    let files = parse_diff(diff)?;
    let total: usize = files.iter().map(|f| f.hunks.len()).sum();
    if total == 0 {
        return Err(if files.is_empty() {
            "no diff found — expected unified diff lines such as `--- a/file`, `+++ b/file` and `@@ -1,4 +1,5 @@`"
                .into()
        } else {
            format!(
                "no diff hunks found — the patch has {} file entr{} but no @@ hunks (binary files, renames, or mode changes only)",
                files.len(),
                if files.len() == 1 { "y" } else { "ies" }
            )
        });
    }

    let hunk_spans = parse_spans(hunks, "hunks")?;
    let line_spans = parse_spans(lines, "lines")?;
    if let Some(spans) = &hunk_spans {
        // A typo'd start (`7` in a 5-hunk patch) is worth an error; an over-long
        // end (`1-100`) is a legitimate "to the end" and just clips.
        for (lo, _) in spans {
            if *lo > total as u64 {
                return Err(format!(
                    "hunk {lo} is out of range — the patch has {}",
                    plural(total, "hunk")
                ));
            }
        }
    }

    let mut keep = vec![false; total];
    for f in &files {
        let file_ok = file_selected(files_spec, &f.path);
        for h in &f.hunks {
            let by_number = in_spans(&hunk_spans, h.index as u64) != invert;
            let (lo, hi) = old_span(h);
            let by_line = match &line_spans {
                None => true,
                Some(spans) => spans.iter().any(|(a, b)| lo <= *b && hi >= *a),
            };
            keep[h.index - 1] = file_ok && by_number && by_line;
        }
    }

    let filtered = hunk_spans.is_some()
        || invert
        || !files_spec.trim().is_empty()
        || line_spans.is_some();
    let selected = keep.iter().filter(|k| **k).count();

    match output {
        "list" => Ok(render_list(&files, &keep, filtered)),
        "json" => render_json(&files, &keep, hunks, invert, files_spec, lines, renumber),
        _ if selected == 0 => Err(format!(
            "no hunks selected — the patch has {}; check hunks/files/lines (run output=list to see the numbered inventory)",
            plural(total, "hunk")
        )),
        "patch" => Ok(render_patch(&files, &keep, renumber)),
        _ => Ok(render_split(&files, &keep, renumber)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A real two-file patch. Written as a plain multi-line literal on purpose:
    // a `\`-continuation would eat each context line's leading space and quietly
    // turn the fixture into a malformed diff.
    const DIFF: &str = r"diff --git a/src/main.rs b/src/main.rs
index 1111111..2222222 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,6 @@ fn main()
 use std::io;
+use std::fmt;
+use std::env;
 use std::path::Path;
 use std::process;
 fn main() {
@@ -20,7 +22,7 @@ fn helper()
     let a = 1;
-    let b = 2;
+    let b = 3;
     let c = 4;
     let d = 5;
     let e = 6;
     let f = 7;
     let g = 8;
diff --git a/README.md b/README.md
--- a/README.md
+++ b/README.md
@@ -1,2 +1,3 @@
 # Title
+A new line.
 Body.
";

    fn list(d: &str) -> String {
        select_hunks(d, "list", "all", false, "", "", true).unwrap()
    }

    #[test]
    fn lists_every_hunk_with_global_numbers_and_counts() {
        let out = list(DIFF);
        assert!(out.starts_with("2 files · 3 hunks · +4 −1\n"), "{out}");
        assert!(out.contains("src/main.rs · 2 hunks · +3 −1"), "{out}");
        assert!(out.contains(" [1] @@ -1,4 +1,6 @@ fn main()"), "{out}");
        assert!(out.contains(" [3] "), "{out}");
        assert!(out.contains("README.md · 1 hunk · +1 −0"), "{out}");
    }

    #[test]
    fn patch_of_one_hunk_keeps_the_file_header() {
        let out = select_hunks(DIFF, "patch", "2", false, "", "", true).unwrap();
        assert!(out.starts_with("diff --git a/src/main.rs b/src/main.rs\n"), "{out}");
        assert!(out.contains("--- a/src/main.rs\n+++ b/src/main.rs\n"), "{out}");
        assert!(!out.contains("README.md"), "{out}");
        assert_eq!(out.lines().filter(|l| l.starts_with("@@")).count(), 1, "{out}");
        // hunk 1 added 2 net lines and is dropped → new side shifts back by 2
        assert!(out.contains("@@ -20,7 +20,7 @@ fn helper()"), "{out}");
        assert!(out.ends_with('\n'), "{out}");
    }

    #[test]
    fn renumber_off_keeps_the_original_header() {
        let out = select_hunks(DIFF, "patch", "2", false, "", "", false).unwrap();
        assert!(out.contains("@@ -20,7 +22,7 @@ fn helper()"), "{out}");
    }

    #[test]
    fn selecting_everything_round_trips_the_hunks_byte_for_byte() {
        let out = select_hunks(DIFF, "patch", "all", false, "", "", true).unwrap();
        assert_eq!(out, DIFF);
    }

    #[test]
    fn ranges_open_ranges_and_invert() {
        let pick = |spec: &str, invert: bool| {
            select_hunks(DIFF, "json", spec, invert, "", "", true).unwrap()
        };
        let sel = |s: &str| {
            let v: serde_json::Value = serde_json::from_str(s).unwrap();
            v["selection"]["selected"].clone()
        };
        assert_eq!(sel(&pick("1,3", false)).to_string(), "[1,3]");
        assert_eq!(sel(&pick("2-3", false)).to_string(), "[2,3]");
        assert_eq!(sel(&pick("-2", false)).to_string(), "[1,2]");
        assert_eq!(sel(&pick("2-", false)).to_string(), "[2,3]");
        assert_eq!(sel(&pick("1-100", false)).to_string(), "[1,2,3]");
        assert_eq!(sel(&pick("2", true)).to_string(), "[1,3]");
    }

    #[test]
    fn file_globs_include_and_exclude() {
        let sel = |spec: &str| {
            let v: serde_json::Value =
                serde_json::from_str(&select_hunks(DIFF, "json", "all", false, spec, "", true).unwrap())
                    .unwrap();
            v["selection"]["selected"].to_string()
        };
        assert_eq!(sel("*.rs"), "[1,2]");
        assert_eq!(sel("!*.md"), "[1,2]");
        assert_eq!(sel("README.md"), "[3]");
        assert_eq!(sel("src/*"), "[1,2]");
        assert_eq!(sel("*, !src/main.rs"), "[3]");
    }

    #[test]
    fn line_spans_match_original_line_numbers() {
        let sel = |spec: &str| {
            let v: serde_json::Value =
                serde_json::from_str(&select_hunks(DIFF, "json", "all", false, "", spec, true).unwrap())
                    .unwrap();
            v["selection"]["selected"].to_string()
        };
        // hunk 1 covers old 1-4, hunk 2 covers old 20-26, hunk 3 covers old 1-2
        assert_eq!(sel("1-4"), "[1,3]");
        assert_eq!(sel("22-23"), "[2]");
        assert_eq!(sel("100-"), "[]");
    }

    #[test]
    fn split_emits_one_standalone_patch_per_hunk() {
        let out = select_hunks(DIFF, "split", "1,2", false, "", "", true).unwrap();
        assert!(out.contains("==== patch 1 of 2 · hunk [1] · src/main.rs ===="), "{out}");
        assert!(out.contains("==== patch 2 of 2 · hunk [2] · src/main.rs ===="), "{out}");
        // each piece carries the file header and exactly one hunk
        assert_eq!(out.matches("--- a/src/main.rs").count(), 2, "{out}");
        assert_eq!(out.lines().filter(|l| l.starts_with("@@")).count(), 2, "{out}");
        // piece 2 stands alone against the original file → renumbered
        assert!(out.contains("@@ -20,7 +20,7 @@ fn helper()"), "{out}");
    }

    #[test]
    fn no_newline_markers_survive() {
        let d = "--- a/f.txt\n+++ b/f.txt\n@@ -1 +1 @@\n-old\n\\ No newline at end of file\n+new\n\\ No newline at end of file\n";
        let out = select_hunks(d, "patch", "1", false, "", "", true).unwrap();
        assert_eq!(out, d);
        assert_eq!(out.matches("\\ No newline at end of file").count(), 2);
    }

    #[test]
    fn trailing_mail_signature_is_not_eaten_as_a_deletion() {
        let d = "--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n ctx\n-old\n+new\n-- \n2.39.0\n";
        let out = select_hunks(d, "patch", "all", false, "", "", true).unwrap();
        assert!(!out.contains("2.39.0"), "{out}");
        assert!(out.ends_with("+new\n"), "{out}");
    }

    #[test]
    fn binary_only_files_are_listed_but_have_no_hunks() {
        let d = "diff --git a/logo.png b/logo.png\nBinary files a/logo.png and b/logo.png differ\ndiff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1 +1 @@\n-a\n+b\n";
        let out = list(d);
        assert!(out.contains("logo.png · no textual hunks"), "{out}");
        let patch = select_hunks(d, "patch", "all", false, "", "", true).unwrap();
        assert!(!patch.contains("logo.png"), "{patch}");
    }

    #[test]
    fn bare_hunk_without_a_file_header_still_parses() {
        let d = "@@ -1,2 +1,3 @@\n a\n+b\n c\n";
        let out = select_hunks(d, "patch", "1", false, "", "", true).unwrap();
        assert_eq!(out, d);
        assert!(list(d).contains("(no file header)"));
    }

    #[test]
    fn list_marks_the_current_selection() {
        let out = select_hunks(DIFF, "list", "1,3", false, "", "", true).unwrap();
        assert!(out.contains("*[1]"), "{out}");
        assert!(out.contains(" [2]"), "{out}");
        assert!(out.ends_with("Selected (*): 1, 3 — 2 of 3"), "{out}");
    }

    #[test]
    fn errors_are_actionable() {
        let e = |r: Result<String, String>| r.unwrap_err();
        assert!(e(select_hunks("  \n ", "list", "all", false, "", "", true)).contains("no diff input"));
        assert!(e(select_hunks("hello\nworld", "list", "all", false, "", "", true)).contains("no diff found"));
        assert!(e(select_hunks(DIFF, "list", "9", false, "", "", true)).contains("out of range"));
        assert!(e(select_hunks(DIFF, "list", "x", false, "", "", true)).contains("invalid hunks selection"));
        assert!(e(select_hunks(DIFF, "list", "5-2", false, "", "", true)).contains("greater than the end"));
        assert!(e(select_hunks(DIFF, "list", "0", false, "", "", true)).contains("starts at 1"));
        assert!(e(select_hunks(DIFF, "patch", "all", true, "", "", true)).contains("no hunks selected"));
        assert!(e(select_hunks(DIFF, "patch", "all", false, "nope.txt", "", true)).contains("no hunks selected"));
        assert!(e(select_hunks(DIFF, "nope", "all", false, "", "", true)).contains("unknown output"));
        let big = "a".repeat(MAX_INPUT_BYTES + 1);
        assert!(e(select_hunks(&big, "list", "all", false, "", "", true)).contains("too large"));
    }

    #[test]
    fn json_reports_the_full_inventory() {
        let v: serde_json::Value =
            serde_json::from_str(&select_hunks(DIFF, "json", "2", false, "", "", true).unwrap())
                .unwrap();
        assert_eq!(v["totals"]["files"], 2);
        assert_eq!(v["totals"]["hunks"], 3);
        assert_eq!(v["totals"]["added"], 4);
        assert_eq!(v["totals"]["removed"], 1);
        assert_eq!(v["totals"]["selected"], 1);
        assert_eq!(v["hunks"][1]["file"], "src/main.rs");
        assert_eq!(v["hunks"][1]["old_start"], 20);
        assert_eq!(v["hunks"][1]["new_start"], 22);
        assert_eq!(v["hunks"][1]["selected"], true);
        assert_eq!(v["hunks"][0]["selected"], false);
        assert_eq!(v["files"][1]["path"], "README.md");
    }

    #[test]
    fn crlf_input_is_normalized() {
        let d = "--- a/f.txt\r\n+++ b/f.txt\r\n@@ -1 +1 @@\r\n-a\r\n+b\r\n";
        let out = select_hunks(d, "patch", "1", false, "", "", true).unwrap();
        assert_eq!(out, "--- a/f.txt\n+++ b/f.txt\n@@ -1 +1 @@\n-a\n+b\n");
    }

    #[test]
    fn headers_without_explicit_counts_keep_their_shape_when_renumbered() {
        let d = "--- a/f.txt\n+++ b/f.txt\n@@ -1,0 +2 @@\n+added\n@@ -5 +7 @@\n-x\n+y\n";
        let out = select_hunks(d, "patch", "2", false, "", "", true).unwrap();
        assert!(out.contains("@@ -5 +6 @@"), "{out}");
    }
}
