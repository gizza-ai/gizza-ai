//! apply-patch core — pure compute, shared by the chat skill block, the CLI, and
//! the web page. No wafer/wasm-bindgen deps, no I/O, no clock: the same input
//! always produces the same bytes.
//!
//! Takes one pasted source file plus a unified diff (`git diff`, `git show`,
//! `git format-patch`, `diff -u`, `svn diff`) and applies the patch to the text,
//! returning the patched result — the inverse of the `text-diff` tool and the
//! missing half of `diff-viewer` / `diff-hunk-selector`.
//!
//! Matching follows the reference `patch(1)` behaviour:
//!
//! * a hunk may land away from the line its `@@` header claims (offset search,
//!   nearest position wins);
//! * a `fuzz` factor of 1–3 drops that many leading/trailing CONTEXT lines when
//!   an exact match fails (never a `+`/`-` line);
//! * `ignore_whitespace` compares context and deleted lines with whitespace runs
//!   collapsed, while the emitted text stays byte-verbatim;
//! * hunks apply strictly in order and never overlap.
//!
//! Conflicts are first-class: `on_conflict = "fail"` refuses the whole patch and
//! names the hunk, the expected line and the line actually found; `"skip"` applies
//! what matches and sets the rest aside, and `output = "rejects"` re-emits those
//! failed hunks as a standalone patch. `output = "report"` and `"json"` are
//! diagnostic (a dry run, like `git apply --check`): they never fail on a
//! conflict, they describe it.
//!
//! The source's line ending (LF or CRLF) and its final-newline state are
//! preserved, and `\ No newline at end of file` markers are honoured.

use serde_json::{json, Value};

/// Hard cap on each pasted input — 1 MB. Anything bigger is a repository dump
/// rather than one file, and the page keeps the whole thing in wasm memory.
pub const MAX_INPUT_BYTES: usize = 1_000_000;

/// Highest accepted `fuzz` factor. `patch(1)` caps its own default at 2; 3 is
/// the most context a hunk can lose before a "match" stops meaning anything.
pub const MAX_FUZZ: u32 = 3;

/// Path shown for hunks pasted without any `diff --git` / `---` / `+++` header.
const NO_FILE: &str = "(no file header)";

/// Longest quoted line fragment shown inside an error or report message.
const SNIPPET: usize = 60;

// ------------------------------------------------------------------- model

#[derive(Debug, Clone)]
struct Hunk {
    /// 1-based index among the selected file's hunks.
    index: usize,
    /// The `@@ … @@ …` line exactly as pasted — shown in reports.
    raw_header: String,
    /// Everything after the closing `@@` of the header.
    heading: String,
    old_start: u64,
    new_start: u64,
    /// Body lines with their `+`/`-`/` ` marker, `\ No newline` lines removed.
    body: Vec<String>,
    /// True when the old side of this hunk ends without a trailing newline.
    old_no_nl: bool,
    /// True when the new side of this hunk ends without a trailing newline.
    new_no_nl: bool,
}

impl Hunk {
    /// Context + deleted lines: what the source must contain for this hunk.
    fn old_lines(&self) -> Vec<String> {
        self.body
            .iter()
            .filter(|l| !l.starts_with('+'))
            .map(|l| l[1.min(l.len())..].to_string())
            .collect()
    }

    /// Leading run of context lines — the most `fuzz` may drop from the front.
    fn leading_context(&self) -> usize {
        self.body
            .iter()
            .take_while(|l| is_context(l))
            .filter(|l| is_context(l))
            .count()
    }

    /// Trailing run of context lines — the most `fuzz` may drop from the back.
    fn trailing_context(&self) -> usize {
        self.body.iter().rev().take_while(|l| is_context(l)).count()
    }

    /// Reverse this hunk: `+` and `-` swap sides, as do the two ranges.
    fn reversed(&self) -> Hunk {
        let body = self
            .body
            .iter()
            .map(|l| match l.chars().next() {
                Some('+') => format!("-{}", &l[1..]),
                Some('-') => format!("+{}", &l[1..]),
                _ => l.clone(),
            })
            .collect();
        Hunk {
            index: self.index,
            raw_header: self.raw_header.clone(),
            heading: self.heading.clone(),
            old_start: self.new_start,
            new_start: self.old_start,
            body,
            old_no_nl: self.new_no_nl,
            new_no_nl: self.old_no_nl,
        }
    }

    /// Re-emit the hunk as a standalone `@@` block (used by `output=rejects`),
    /// with counts recomputed from the body so a mis-counted header is fixed.
    fn to_patch_text(&self) -> String {
        let old_count = self.body.iter().filter(|l| !l.starts_with('+')).count();
        let new_count = self.body.iter().filter(|l| !l.starts_with('-')).count();
        let mut out = format!(
            "@@ -{},{} +{},{} @@{}\n",
            self.old_start, old_count, self.new_start, new_count, self.heading
        );
        let last_old = self.body.iter().rposition(|l| !l.starts_with('+'));
        let last_new = self.body.iter().rposition(|l| !l.starts_with('-'));
        for (i, l) in self.body.iter().enumerate() {
            out.push_str(l);
            out.push('\n');
            let marks_old = self.old_no_nl && Some(i) == last_old;
            let marks_new = self.new_no_nl && Some(i) == last_new;
            if marks_old || marks_new {
                out.push_str("\\ No newline at end of file\n");
            }
        }
        out
    }
}

/// A context line: a leading space, or an empty line whose trailing space was
/// stripped in transit (mail clients and copy/paste do this routinely).
fn is_context(line: &str) -> bool {
    line.is_empty() || line.starts_with(' ')
}

#[derive(Debug, Clone)]
struct FileDiff {
    path: String,
    hunks: Vec<Hunk>,
    /// A `Binary files … differ` / `GIT binary patch` entry — nothing to apply.
    binary: bool,
}

/// What happened to one hunk.
#[derive(Debug, Clone)]
struct Outcome {
    index: usize,
    raw_header: String,
    /// 1-based source line the hunk landed on, when it applied.
    at: Option<usize>,
    /// `at` minus the line the header claimed.
    offset: i64,
    /// Context lines dropped from each end to make it match.
    fuzz_used: u32,
    /// Why it did not apply.
    reason: Option<String>,
}

// ----------------------------------------------------------------- parsing

/// Split text into lines without terminators, dropping the empty piece a final
/// newline produces. `strip_cr` removes the `\r` of a CRLF pair.
fn split_lines(text: &str, strip_cr: bool) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut v: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
    if v.last().map(|s| s.is_empty()).unwrap_or(false) {
        v.pop();
    }
    if strip_cr {
        for l in v.iter_mut() {
            if l.ends_with('\r') {
                l.pop();
            }
        }
    }
    v
}

fn is_file_start(lines: &[String], i: usize) -> bool {
    let l = &lines[i];
    l.starts_with("diff --git ")
        || (l.starts_with("--- ") && i + 1 < lines.len() && lines[i + 1].starts_with("+++ "))
}

fn is_hunk_start(line: &str) -> bool {
    line.starts_with("@@") && line[2..].contains("@@")
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
            .map(clean_path)
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

/// `-12,7` → (12, 7). A missing `,count` means 1 (git's shorthand).
fn parse_range(tok: &str) -> Option<(u64, u64)> {
    let body = tok.strip_prefix('-').or_else(|| tok.strip_prefix('+'))?;
    match body.split_once(',') {
        Some((s, c)) => Some((s.parse().ok()?, c.parse().ok()?)),
        None => Some((body.parse().ok()?, 1)),
    }
}

fn parse_hunk_header(header: &str) -> Result<(u64, u64, u64, u64, String), String> {
    if header.starts_with("@@@") {
        return Err("combined (merge) diffs with @@@ headers cannot be applied to a single \
                    file — re-run the diff against one parent"
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
    Ok((old.0, old.1, new.0, new.1, heading))
}

/// Consume one hunk starting at `lines[i]` (its `@@` header). The declared counts
/// bound the body so a trailing mail signature or the next file header is not
/// swallowed, but a body that runs on past a wrong count is still accepted
/// (`git apply --recount` behaviour) — a new `@@`/`diff --git` line ends it.
fn parse_hunk(lines: &[String], i: usize, index: usize) -> Result<(Hunk, usize), String> {
    let raw_header = lines[i].clone();
    let (old_start, old_count, new_start, new_count, heading) = parse_hunk_header(&raw_header)?;

    let mut body: Vec<String> = Vec::new();
    let (mut seen_old, mut seen_new) = (0u64, 0u64);
    let (mut old_no_nl, mut new_no_nl) = (false, false);
    let mut j = i + 1;
    while j < lines.len() {
        let l = &lines[j];
        // `\ No newline at end of file` belongs to the previous body line and
        // counts for neither side.
        if l.starts_with('\\') {
            match body.last().and_then(|p| p.chars().next()) {
                Some('-') => old_no_nl = true,
                Some('+') => new_no_nl = true,
                _ => {
                    old_no_nl = true;
                    new_no_nl = true;
                }
            }
            j += 1;
            continue;
        }
        let counted = seen_old >= old_count && seen_new >= new_count;
        // Past the declared counts, only keep going while the line still looks
        // like body content — never across a new hunk or file header.
        if counted && (is_hunk_start(l) || is_file_start(lines, j)) {
            break;
        }
        match l.chars().next() {
            Some(' ') | None => {
                seen_old += 1;
                seen_new += 1;
            }
            Some('+') => seen_new += 1,
            Some('-') => seen_old += 1,
            // Malformed body — stop rather than eat the next file's header.
            _ => break,
        }
        body.push(l.clone());
        j += 1;
        if seen_old >= old_count && seen_new >= new_count {
            // Peek: only continue past the counts if the next line is clearly
            // more body (a mis-counted header), never a new hunk/file.
            if j < lines.len() && (is_hunk_start(&lines[j]) || is_file_start(lines, j)) {
                break;
            }
            // A `\ No newline at end of file` marker still belongs to this hunk.
            if j >= lines.len()
                || !matches!(lines[j].chars().next(), Some(' ' | '+' | '-' | '\\') | None)
            {
                break;
            }
        }
    }
    if body.is_empty() {
        return Err(format!("hunk has no body lines: {raw_header}"));
    }
    Ok((
        Hunk {
            index,
            raw_header,
            heading,
            old_start,
            new_start,
            body,
            old_no_nl,
            new_no_nl,
        },
        j,
    ))
}

fn parse_patch(patch: &str) -> Result<Vec<FileDiff>, String> {
    let strip_cr = patch.contains("\r\n");
    let lines = split_lines(patch, strip_cr);
    let mut files: Vec<FileDiff> = Vec::new();
    let mut hunk_no = 0usize;
    let mut i = 0usize;
    while i < lines.len() {
        if is_file_start(&lines, i) {
            let mut preamble = vec![lines[i].clone()];
            let mut j = i + 1;
            let mut binary = false;
            while j < lines.len() && !is_hunk_start(&lines[j]) && !is_file_start(&lines, j) {
                if lines[j].starts_with("Binary files ") || lines[j].starts_with("GIT binary patch")
                {
                    binary = true;
                }
                preamble.push(lines[j].clone());
                j += 1;
            }
            files.push(FileDiff {
                path: path_from_preamble(&preamble),
                hunks: Vec::new(),
                binary,
            });
            hunk_no = 0;
            i = j;
            continue;
        }
        if is_hunk_start(&lines[i]) {
            if files.is_empty() {
                files.push(FileDiff {
                    path: NO_FILE.to_string(),
                    hunks: Vec::new(),
                    binary: false,
                });
                hunk_no = 0;
            }
            hunk_no += 1;
            let (hunk, next) = parse_hunk(&lines, i, hunk_no)?;
            files.last_mut().unwrap().hunks.push(hunk);
            i = next;
            continue;
        }
        // Prose, mail headers, `index …` lines outside a file block — skip.
        i += 1;
    }
    Ok(files)
}

// ---------------------------------------------------------------- matching

/// Whitespace-insensitive comparison key: runs of whitespace collapse to one
/// space and the ends are trimmed. Only used for MATCHING — emitted text is
/// always the verbatim original.
fn norm_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn lines_match(a: &str, b: &str, ignore_ws: bool) -> bool {
    a == b || (ignore_ws && norm_ws(a) == norm_ws(b))
}

fn window_matches(src: &[String], pos: usize, pattern: &[String], ignore_ws: bool) -> bool {
    pattern
        .iter()
        .enumerate()
        .all(|(k, p)| lines_match(&src[pos + k], p, ignore_ws))
}

/// Find where `pattern` sits in `src`, searching outward from `guess` and never
/// before `min_pos`. Ties prefer the earlier position, matching `patch(1)`.
fn find_match(
    src: &[String],
    pattern: &[String],
    guess: usize,
    min_pos: usize,
    ignore_ws: bool,
) -> Option<usize> {
    if pattern.is_empty() {
        return Some(guess.clamp(min_pos, src.len()));
    }
    if pattern.len() > src.len() {
        return None;
    }
    let last = src.len() - pattern.len();
    let guess = guess.min(last);
    for d in 0..=src.len() {
        if let Some(back) = guess.checked_sub(d) {
            if back >= min_pos && window_matches(src, back, pattern, ignore_ws) {
                return Some(back);
            }
        }
        if d > 0 {
            let fwd = guess + d;
            if fwd <= last && fwd >= min_pos && window_matches(src, fwd, pattern, ignore_ws) {
                return Some(fwd);
            }
        }
    }
    None
}

fn snippet(s: &str) -> String {
    let t: String = s.chars().take(SNIPPET).collect();
    if t.chars().count() < s.chars().count() {
        format!("{t}…")
    } else {
        t
    }
}

/// Explain why a hunk did not match, in terms of the line it expected.
fn failure_reason(src: &[String], pattern: &[String], guess: usize, ignore_ws: bool) -> String {
    if pattern.is_empty() {
        return "no position left in the source text for this hunk".to_string();
    }
    let at = guess.min(src.len());
    for (k, p) in pattern.iter().enumerate() {
        let idx = at + k;
        if idx >= src.len() {
            return format!(
                "the hunk needs {} more line(s) from line {}, but the source text ends at line {}",
                pattern.len() - k,
                at + 1,
                src.len()
            );
        }
        if !lines_match(&src[idx], p, ignore_ws) {
            return format!(
                "expected `{}` at line {}, found `{}`",
                snippet(p),
                idx + 1,
                snippet(&src[idx])
            );
        }
    }
    "the matching region was already consumed by an earlier hunk".to_string()
}

// ------------------------------------------------------------ file picking

/// Choose which file's hunks apply to the pasted source.
fn pick_file<'a>(files: &'a [FileDiff], want: &str) -> Result<&'a FileDiff, String> {
    let want = want.trim();
    let paths = |f: &[&FileDiff]| {
        f.iter()
            .map(|d| d.path.clone())
            .collect::<Vec<_>>()
            .join(", ")
    };
    if !want.is_empty() {
        let exact: Vec<&FileDiff> = files.iter().filter(|f| f.path == want).collect();
        let matches: Vec<&FileDiff> = if !exact.is_empty() {
            exact
        } else {
            files
                .iter()
                .filter(|f| {
                    f.path.ends_with(&format!("/{want}"))
                        || f.path.rsplit('/').next() == Some(want)
                        || f.path.contains(want)
                })
                .collect()
        };
        return match matches.len() {
            0 => Err(format!(
                "no file in the patch matches file={want:?} — the patch touches: {}",
                paths(&files.iter().collect::<Vec<_>>())
            )),
            1 => Ok(matches[0]),
            _ => Err(format!(
                "file={want:?} matches more than one path in the patch ({}) — use the full path",
                paths(&matches)
            )),
        };
    }
    let with_hunks: Vec<&FileDiff> = files.iter().filter(|f| !f.hunks.is_empty()).collect();
    match with_hunks.len() {
        0 => {
            if files.iter().any(|f| f.binary) {
                Err("the patch only contains binary file entries, which have no text hunks to \
                     apply"
                    .to_string())
            } else {
                Err("no @@ hunks found in the patch — paste a unified diff (`git diff`, \
                     `git show`, or `diff -u` output)"
                    .to_string())
            }
        }
        1 => Ok(with_hunks[0]),
        _ => Err(format!(
            "the patch touches {} files ({}) — set 'file' to the one your source text is",
            with_hunks.len(),
            paths(&with_hunks)
        )),
    }
}

// ------------------------------------------------------------------- apply

struct Applied {
    lines: Vec<String>,
    final_newline: bool,
    outcomes: Vec<Outcome>,
    rejected: Vec<Hunk>,
}

fn apply_hunks(
    src: &[String],
    src_final_nl: bool,
    hunks: &[Hunk],
    fuzz: u32,
    ignore_ws: bool,
) -> Applied {
    let mut out: Vec<String> = Vec::new();
    let mut cursor = 0usize;
    let mut final_nl = src_final_nl;
    let mut outcomes = Vec::new();
    let mut rejected = Vec::new();

    for hunk in hunks {
        let old_lines = hunk.old_lines();
        let lead = hunk.leading_context();
        let trail = hunk.trailing_context();
        let base_guess = hunk.old_start.max(1) as usize - 1;

        let mut landed = None;
        for fz in 0..=fuzz {
            let front = (fz as usize).min(lead);
            let back = (fz as usize).min(trail);
            if front + back >= hunk.body.len() {
                continue;
            }
            // Trimming the BODY (not just the old side) keeps the `+` lines and
            // the dropped context in lockstep.
            let eff = &hunk.body[front..hunk.body.len() - back];
            let pattern: Vec<String> = eff
                .iter()
                .filter(|l| !l.starts_with('+'))
                .map(|l| l[1.min(l.len())..].to_string())
                .collect();
            // Fuzz must never strip a hunk down to nothing to match against —
            // that would turn a real conflict into a blind insertion.
            if pattern.is_empty() && fz > 0 {
                continue;
            }
            let guess = base_guess + front;
            if let Some(pos) = find_match(src, &pattern, guess, cursor, ignore_ws) {
                landed = Some((pos, front, back, fz));
                break;
            }
        }

        match landed {
            Some((pos, front, back, fz)) => {
                let eff = &hunk.body[front..hunk.body.len() - back];
                out.extend_from_slice(&src[cursor..pos]);
                // Context lines are re-emitted from the SOURCE, so an
                // ignore_whitespace match keeps the file's own indentation.
                let mut sp = pos;
                for l in eff {
                    match l.chars().next() {
                        Some('+') => out.push(l[1..].to_string()),
                        Some('-') => sp += 1,
                        _ => {
                            out.push(src[sp].clone());
                            sp += 1;
                        }
                    }
                }
                cursor = sp;
                let at = pos + 1 - front.min(pos);
                if cursor == src.len() && back == 0 {
                    final_nl = !hunk.new_no_nl;
                }
                outcomes.push(Outcome {
                    index: hunk.index,
                    raw_header: hunk.raw_header.clone(),
                    at: Some(at),
                    offset: at as i64 - hunk.old_start.max(1) as i64,
                    fuzz_used: fz,
                    reason: None,
                });
            }
            None => {
                let reason = failure_reason(src, &old_lines, base_guess.max(cursor), ignore_ws);
                outcomes.push(Outcome {
                    index: hunk.index,
                    raw_header: hunk.raw_header.clone(),
                    at: None,
                    offset: 0,
                    fuzz_used: 0,
                    reason: Some(reason),
                });
                rejected.push(hunk.clone());
            }
        }
    }
    out.extend_from_slice(&src[cursor..]);
    Applied {
        lines: out,
        final_newline: final_nl,
        outcomes,
        rejected,
    }
}

// ---------------------------------------------------------------- rendering

fn join(lines: &[String], eol: &str, final_newline: bool) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let mut s = lines.join(eol);
    if final_newline {
        s.push_str(eol);
    }
    s
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

fn render_report(path: &str, reverse: bool, applied: &Applied) -> String {
    let total = applied.outcomes.len();
    let ok = applied.outcomes.iter().filter(|o| o.at.is_some()).count();
    let failed = total - ok;
    let width = applied
        .outcomes
        .iter()
        .map(|o| o.raw_header.chars().count())
        .max()
        .unwrap_or(0);

    let mut s = format!(
        "{ok} of {} applied · {path}{}\n\n",
        plural(total, "hunk", "hunks"),
        if reverse { " · reversed" } else { "" }
    );
    for o in &applied.outcomes {
        let pad = width - o.raw_header.chars().count();
        match (&o.reason, o.at) {
            (None, Some(at)) => {
                let mut notes = Vec::new();
                if o.offset != 0 {
                    notes.push(format!("offset {:+}", o.offset));
                }
                if o.fuzz_used > 0 {
                    notes.push(format!("fuzz {}", o.fuzz_used));
                }
                let note = if notes.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", notes.join(", "))
                };
                s.push_str(&format!(
                    " [{}] {}{}  applied at line {at}{note}\n",
                    o.index,
                    o.raw_header,
                    " ".repeat(pad)
                ));
            }
            _ => s.push_str(&format!(
                " [{}] {}{}  FAILED — {}\n",
                o.index,
                o.raw_header,
                " ".repeat(pad),
                o.reason.clone().unwrap_or_default()
            )),
        }
    }
    s.push('\n');
    if failed == 0 {
        s.push_str("The patch applies cleanly. Set output=patched for the patched text.");
    } else {
        s.push_str(&format!(
            "{} failed. Set output=rejects for those hunks as a patch, or on_conflict=skip to \
             apply the rest.",
            plural(failed, "hunk", "hunks")
        ));
    }
    s
}

fn render_json(
    path: &str,
    reverse: bool,
    fuzz: u32,
    ignore_ws: bool,
    applied: &Applied,
    patched: &str,
) -> String {
    let total = applied.outcomes.len();
    let ok = applied.outcomes.iter().filter(|o| o.at.is_some()).count();
    let hunks: Vec<Value> = applied
        .outcomes
        .iter()
        .map(|o| match (&o.reason, o.at) {
            (None, Some(at)) => json!({
                "index": o.index,
                "header": o.raw_header,
                "status": "applied",
                "applied_at_line": at,
                "offset": o.offset,
                "fuzz_used": o.fuzz_used,
            }),
            _ => json!({
                "index": o.index,
                "header": o.raw_header,
                "status": "failed",
                "reason": o.reason.clone().unwrap_or_default(),
            }),
        })
        .collect();
    let value = json!({
        "file": path,
        "reverse": reverse,
        "fuzz": fuzz,
        "ignore_whitespace": ignore_ws,
        "hunks_total": total,
        "hunks_applied": ok,
        "hunks_failed": total - ok,
        "clean": total == ok,
        "hunks": hunks,
        "patched": patched,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn render_rejects(path: &str, rejected: &[Hunk]) -> String {
    if rejected.is_empty() {
        return "No rejected hunks — every hunk applied cleanly.".to_string();
    }
    let mut s = format!("--- a/{path}\n+++ b/{path}\n");
    for h in rejected {
        s.push_str(&h.to_patch_text());
    }
    s
}

// -------------------------------------------------------------- public API

/// Apply `patch` to `source` and render the requested `output`.
///
/// * `output` — `patched` (default) | `report` | `json` | `rejects`.
/// * `reverse` — unapply the patch (`+` and `-` swap roles).
/// * `fuzz` — 0–3 context lines that may be dropped from each end of a hunk.
/// * `ignore_whitespace` — match context/deleted lines ignoring whitespace runs.
/// * `on_conflict` — `fail` (default) refuses the patch; `skip` applies the rest.
/// * `file` — which path's hunks to use when the patch touches several files.
///
/// `report` and `json` are dry runs: they describe a conflict instead of failing.
#[allow(clippy::too_many_arguments)]
pub fn apply_patch(
    source: &str,
    patch: &str,
    output: &str,
    reverse: bool,
    fuzz: u32,
    ignore_whitespace: bool,
    on_conflict: &str,
    file: &str,
) -> Result<String, String> {
    if source.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "source text is too large: {} bytes, max {MAX_INPUT_BYTES}",
            source.len()
        ));
    }
    if patch.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "patch is too large: {} bytes, max {MAX_INPUT_BYTES}",
            patch.len()
        ));
    }
    let output = match output.trim() {
        "" => "patched",
        o => o,
    };
    if !matches!(output, "patched" | "report" | "json" | "rejects") {
        return Err(format!(
            "unknown output {output:?} — use patched, report, json, or rejects"
        ));
    }
    let on_conflict = match on_conflict.trim() {
        "" => "fail",
        c => c,
    };
    if !matches!(on_conflict, "fail" | "skip") {
        return Err(format!(
            "unknown on_conflict {on_conflict:?} — use fail or skip"
        ));
    }
    if fuzz > MAX_FUZZ {
        return Err(format!("fuzz must be 0-{MAX_FUZZ}, got {fuzz}"));
    }
    if patch.trim().is_empty() {
        return Err("patch is empty — paste a unified diff (`git diff`, `git show`, or \
                    `diff -u` output)"
            .to_string());
    }

    let files = parse_patch(patch)?;
    if files.is_empty() {
        return Err("no @@ hunks found in the patch — paste a unified diff (`git diff`, \
                    `git show`, or `diff -u` output)"
            .to_string());
    }
    let picked = pick_file(&files, file)?;
    if picked.hunks.is_empty() {
        return Err(format!(
            "the patch entry for {} has no text hunks to apply{}",
            picked.path,
            if picked.binary {
                " (binary file entry)"
            } else {
                " (rename-only or mode-only entry)"
            }
        ));
    }

    let crlf = source.contains("\r\n");
    let eol = if crlf { "\r\n" } else { "\n" };
    let src = split_lines(source, crlf);
    let src_final_nl = source.ends_with('\n');

    let hunks: Vec<Hunk> = if reverse {
        picked.hunks.iter().map(|h| h.reversed()).collect()
    } else {
        picked.hunks.clone()
    };

    let applied = apply_hunks(&src, src_final_nl, &hunks, fuzz, ignore_whitespace);
    let patched = join(&applied.lines, eol, applied.final_newline);
    let failures: Vec<&Outcome> = applied
        .outcomes
        .iter()
        .filter(|o| o.reason.is_some())
        .collect();

    match output {
        "report" => Ok(render_report(&picked.path, reverse, &applied)),
        "json" => Ok(render_json(
            &picked.path,
            reverse,
            fuzz,
            ignore_whitespace,
            &applied,
            &patched,
        )),
        "rejects" => Ok(render_rejects(&picked.path, &applied.rejected)),
        _ => {
            if !failures.is_empty() && on_conflict == "fail" {
                let first = failures[0];
                return Err(format!(
                    "conflict: hunk [{}] {} does not apply — {}. {} of {} hunks failed; set \
                     on_conflict=skip to apply the rest, output=report to see every hunk, or \
                     raise fuzz / enable ignore_whitespace if the source only drifted.",
                    first.index,
                    first.raw_header,
                    first.reason.clone().unwrap_or_default(),
                    failures.len(),
                    applied.outcomes.len()
                ));
            }
            Ok(patched)
        }
    }
}

// -------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "fn main() {\n    let x = 1;\n    println!(\"{x}\");\n}\n";
    const PATCH: &str = "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,4 +1,5 @@\n fn main() {\n     let x = 1;\n+    let y = 2;\n     println!(\"{x}\");\n }\n";

    fn run(src: &str, patch: &str) -> Result<String, String> {
        apply_patch(src, patch, "patched", false, 2, false, "fail", "")
    }

    #[test]
    fn applies_a_hunk_and_returns_the_patched_text() {
        assert_eq!(
            run(SRC, PATCH).unwrap(),
            "fn main() {\n    let x = 1;\n    let y = 2;\n    println!(\"{x}\");\n}\n"
        );
    }

    #[test]
    fn reverse_unapplies_a_patch() {
        let patched = run(SRC, PATCH).unwrap();
        let back = apply_patch(&patched, PATCH, "patched", true, 2, false, "fail", "").unwrap();
        assert_eq!(back, SRC);
    }

    #[test]
    fn conflicting_hunk_fails_with_the_expected_and_found_lines() {
        let err = run("totally different\ncontents here\n", PATCH).unwrap_err();
        assert!(err.starts_with("conflict: hunk [1] @@ -1,4 +1,5 @@ does not apply"), "{err}");
        assert!(err.contains("expected `fn main() {` at line 1, found `totally different`"), "{err}");
    }

    #[test]
    fn skip_applies_what_matches_and_rejects_the_rest() {
        let src = "alpha\nbravo\ncharlie\ndelta\necho\nfoxtrot\ngolf\nhotel\n";
        let patch = "--- a/l.txt\n+++ b/l.txt\n@@ -1,2 +1,3 @@\n alpha\n+ADDED\n bravo\n@@ -6,2 +7,2 @@\n-NOPE\n+YES\n hotel\n";
        let out = apply_patch(src, patch, "patched", false, 0, false, "skip", "").unwrap();
        assert_eq!(out, "alpha\nADDED\nbravo\ncharlie\ndelta\necho\nfoxtrot\ngolf\nhotel\n");
        let rej = apply_patch(src, patch, "rejects", false, 0, false, "skip", "").unwrap();
        assert_eq!(rej, "--- a/l.txt\n+++ b/l.txt\n@@ -6,2 +7,2 @@\n-NOPE\n+YES\n hotel\n");
    }

    #[test]
    fn report_is_a_dry_run_that_never_fails_on_a_conflict() {
        let out = apply_patch(
            "totally different\ncontents here\n",
            PATCH,
            "report",
            false,
            2,
            false,
            "fail",
            "",
        )
        .unwrap();
        assert!(out.starts_with("0 of 1 hunk applied · src/main.rs\n"), "{out}");
        assert!(out.contains("FAILED — expected `fn main() {` at line 1"), "{out}");
        assert!(out.contains("1 hunk failed."), "{out}");
    }

    #[test]
    fn report_shows_offset_and_fuzz() {
        // Two extra leading lines shift the hunk, and its first and last context
        // lines drifted — so it only lands once one line of context is dropped
        // from each end, at an offset from the line the header claims.
        let src = "// header\n// header2\nfn main() {\n    let x = 1;\n    println!(\"{x}\");\n} // trailing comment\n";
        assert!(apply_patch(src, PATCH, "patched", false, 0, false, "fail", "").is_err());
        let out = apply_patch(src, PATCH, "report", false, 1, false, "fail", "").unwrap();
        assert!(out.contains("applied at line 3 (offset +2, fuzz 1)"), "{out}");
        let patched = apply_patch(src, PATCH, "patched", false, 1, false, "fail", "").unwrap();
        assert_eq!(
            patched,
            "// header\n// header2\nfn main() {\n    let x = 1;\n    let y = 2;\n    println!(\"{x}\");\n} // trailing comment\n"
        );
    }

    #[test]
    fn json_reports_every_hunk_and_the_patched_text() {
        let out = apply_patch(SRC, PATCH, "json", false, 2, false, "fail", "").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["file"], "src/main.rs");
        assert_eq!(v["hunks_applied"], 1);
        assert_eq!(v["clean"], true);
        assert_eq!(v["hunks"][0]["status"], "applied");
        assert_eq!(v["hunks"][0]["applied_at_line"], 1);
        assert!(v["patched"].as_str().unwrap().contains("let y = 2;"));
    }

    #[test]
    fn offset_search_finds_a_hunk_that_moved() {
        let src = "one\ntwo\nthree\nfour\nfive\nsix\n";
        let patch = "--- a/n.txt\n+++ b/n.txt\n@@ -20,2 +20,3 @@\n four\n+FOUR AND A HALF\n five\n";
        assert_eq!(
            apply_patch(src, patch, "patched", false, 0, false, "fail", "").unwrap(),
            "one\ntwo\nthree\nfour\nFOUR AND A HALF\nfive\nsix\n"
        );
    }

    #[test]
    fn fuzz_only_helps_once_it_reaches_the_drifted_context_line() {
        // `bravo` (the SECOND leading context line) drifted, so fuzz 1 — which
        // drops only one line from each end — still cannot match.
        let src = "alpha\nDRIFTED\ntarget\nomega\n";
        let patch = "--- a/f.txt\n+++ b/f.txt\n@@ -1,4 +1,4 @@\n alpha\n bravo\n-target\n+TARGET\n omega\n";
        assert!(apply_patch(src, patch, "patched", false, 0, false, "fail", "").is_err());
        assert!(apply_patch(src, patch, "patched", false, 1, false, "fail", "").is_err());
        assert_eq!(
            apply_patch(src, patch, "patched", false, 2, false, "fail", "").unwrap(),
            "alpha\nDRIFTED\nTARGET\nomega\n"
        );
    }

    #[test]
    fn fuzz_drops_leading_context() {
        let src = "DRIFTED HEADER\nbravo\ntarget\nomega\n";
        let patch = "--- a/f.txt\n+++ b/f.txt\n@@ -1,4 +1,4 @@\n alpha\n bravo\n-target\n+TARGET\n omega\n";
        assert!(apply_patch(src, patch, "patched", false, 0, false, "fail", "").is_err());
        assert_eq!(
            apply_patch(src, patch, "patched", false, 1, false, "fail", "").unwrap(),
            "DRIFTED HEADER\nbravo\nTARGET\nomega\n"
        );
    }

    #[test]
    fn ignore_whitespace_matches_reindented_context() {
        let src = "fn main() {\n\tlet x = 1;\n    println!(\"{x}\");\n}\n";
        assert!(apply_patch(src, PATCH, "patched", false, 0, false, "fail", "").is_err());
        let out = apply_patch(src, PATCH, "patched", false, 0, true, "fail", "").unwrap();
        // The context line keeps the source's own indentation, verbatim.
        assert_eq!(
            out,
            "fn main() {\n\tlet x = 1;\n    let y = 2;\n    println!(\"{x}\");\n}\n"
        );
    }

    #[test]
    fn multi_file_patch_needs_a_file_filter() {
        let patch = "--- a/one.txt\n+++ b/one.txt\n@@ -1 +1 @@\n-a\n+A\n--- a/two.txt\n+++ b/two.txt\n@@ -1 +1 @@\n-b\n+B\n";
        let err = apply_patch("a\n", patch, "patched", false, 0, false, "fail", "").unwrap_err();
        assert!(err.contains("touches 2 files (one.txt, two.txt)"), "{err}");
        assert_eq!(
            apply_patch("b\n", patch, "patched", false, 0, false, "fail", "two.txt").unwrap(),
            "B\n"
        );
        let miss =
            apply_patch("b\n", patch, "patched", false, 0, false, "fail", "nope.txt").unwrap_err();
        assert!(miss.contains("no file in the patch matches"), "{miss}");
    }

    #[test]
    fn crlf_source_keeps_its_line_endings() {
        let src = "fn main() {\r\n    let x = 1;\r\n    println!(\"{x}\");\r\n}\r\n";
        let out = run(src, PATCH).unwrap();
        assert_eq!(
            out,
            "fn main() {\r\n    let x = 1;\r\n    let y = 2;\r\n    println!(\"{x}\");\r\n}\r\n"
        );
    }

    #[test]
    fn no_newline_at_end_of_file_is_honoured() {
        let src = "one\ntwo\n";
        let patch = "--- a/t.txt\n+++ b/t.txt\n@@ -2 +2 @@\n-two\n+TWO\n\\ No newline at end of file\n";
        assert_eq!(
            apply_patch(src, patch, "patched", false, 0, false, "fail", "").unwrap(),
            "one\nTWO"
        );
    }

    #[test]
    fn new_file_hunk_applies_to_empty_source() {
        let patch = "--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1,2 @@\n+hello\n+world\n";
        assert_eq!(
            apply_patch("", patch, "patched", false, 0, false, "fail", "").unwrap(),
            "hello\nworld\n"
        );
    }

    #[test]
    fn a_mis_counted_hunk_header_still_applies() {
        // `@@ -1,2 +1,2 @@` under-counts a 4-line body (git apply --recount case).
        let patch = "--- a/m.txt\n+++ b/m.txt\n@@ -1,2 +1,2 @@\n one\n-two\n+TWO\n three\n";
        assert_eq!(
            apply_patch("one\ntwo\nthree\n", patch, "patched", false, 0, false, "fail", "")
                .unwrap(),
            "one\nTWO\nthree\n"
        );
    }

    #[test]
    fn rejects_output_is_a_valid_patch_and_says_so_when_empty() {
        assert_eq!(
            apply_patch(SRC, PATCH, "rejects", false, 2, false, "fail", "").unwrap(),
            "No rejected hunks — every hunk applied cleanly."
        );
    }

    #[test]
    fn bad_arguments_are_rejected_by_name() {
        assert!(apply_patch(SRC, PATCH, "nope", false, 0, false, "fail", "")
            .unwrap_err()
            .contains("unknown output"));
        assert!(apply_patch(SRC, PATCH, "patched", false, 0, false, "maybe", "")
            .unwrap_err()
            .contains("unknown on_conflict"));
        assert!(apply_patch(SRC, PATCH, "patched", false, 9, false, "fail", "")
            .unwrap_err()
            .contains("fuzz must be 0-3"));
        assert!(apply_patch(SRC, "", "patched", false, 0, false, "fail", "")
            .unwrap_err()
            .contains("patch is empty"));
        assert!(apply_patch(SRC, "just prose, no hunks", "patched", false, 0, false, "fail", "")
            .unwrap_err()
            .contains("no @@ hunks found"));
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        assert!(apply_patch(&big, PATCH, "patched", false, 0, false, "fail", "")
            .unwrap_err()
            .contains("source text is too large"));
        assert!(apply_patch(SRC, &big, "patched", false, 0, false, "fail", "")
            .unwrap_err()
            .contains("patch is too large"));
    }

    #[test]
    fn combined_merge_diffs_are_rejected() {
        let patch = "--- a/c.txt\n+++ b/c.txt\n@@@ -1,2 -1,2 +1,2 @@@\n  a\n++b\n";
        assert!(apply_patch("a\n", patch, "patched", false, 0, false, "fail", "")
            .unwrap_err()
            .contains("combined (merge) diffs"));
    }

    #[test]
    fn binary_only_patch_is_reported_not_silently_dropped() {
        let patch = "diff --git a/logo.png b/logo.png\nindex 1234567..89abcde 100644\nBinary files a/logo.png and b/logo.png differ\n";
        assert!(apply_patch("x\n", patch, "patched", false, 0, false, "fail", "")
            .unwrap_err()
            .contains("binary file entries"));
    }

    #[test]
    fn hunks_without_file_headers_still_apply() {
        let patch = "@@ -1,2 +1,3 @@\n one\n+ONE AND A HALF\n two\n";
        assert_eq!(
            apply_patch("one\ntwo\n", patch, "patched", false, 0, false, "fail", "").unwrap(),
            "one\nONE AND A HALF\ntwo\n"
        );
    }

    #[test]
    fn source_at_the_cap_is_accepted() {
        let filler = "x\n".repeat((MAX_INPUT_BYTES - "one\ntwo\n".len()) / 2);
        let src = format!("one\ntwo\n{filler}");
        assert_eq!(src.len(), MAX_INPUT_BYTES);
        let patch = "@@ -1,2 +1,2 @@\n one\n-two\n+TWO\n";
        let out = apply_patch(&src, patch, "patched", false, 0, false, "fail", "").unwrap();
        assert!(out.starts_with("one\nTWO\n"));
    }
}
