//! path-extractor core — pull every file path out of arbitrary text (build logs,
//! stack traces, `git status` output, chat messages) and return them deduplicated.
//!
//! Pure Rust, no I/O: nothing is ever stat'ed on disk, so a path is recognised by
//! its SHAPE only. The scanner is deliberately high-precision — it prefers missing
//! an ambiguous token over flooding the result with prose words.

use std::collections::HashMap;

use regex::Regex;
use serde::Serialize;

/// Largest input accepted, in bytes (~1 MB).
pub const MAX_INPUT_BYTES: usize = 1_000_000;
/// Largest number of path occurrences accepted before erroring out.
pub const MAX_MATCHES: usize = 20_000;
/// Longest single path accepted (POSIX `PATH_MAX` is 4096).
pub const MAX_PATH_LEN: usize = 4096;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Which flavour of path to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathStyle {
    /// Keep both POSIX and Windows paths.
    Any,
    /// Drop Windows-style paths (drive letters, UNC, backslash separators).
    Posix,
    /// Drop POSIX-style (forward-slash) paths.
    Windows,
}

/// Which part of each path to return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKind {
    /// The whole path.
    Path,
    /// Only the last segment (`src/main.rs` → `main.rs`).
    Filename,
    /// Only the containing directory (`src/main.rs` → `src`).
    Directory,
}

/// How the `extensions` list is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionMode {
    /// Keep only paths whose extension is listed.
    Include,
    /// Drop paths whose extension is listed.
    Exclude,
}

/// Result ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Order of first appearance in the text.
    FirstSeen,
    /// A→Z, case-insensitive.
    Asc,
    /// Z→A, case-insensitive.
    Desc,
}

/// Rendering of the `formatted` output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// One value per line.
    List,
    /// `value,occurrences` with a header row.
    Csv,
    /// A JSON object with a `count` and a `matches` array.
    Json,
}

/// Everything that changes what `extract` returns.
#[derive(Debug, Clone)]
pub struct Options {
    pub path_style: PathStyle,
    pub require_separator: bool,
    pub keep_line_numbers: bool,
    pub output: OutputKind,
    pub extensions: Vec<String>,
    pub extension_mode: ExtensionMode,
    pub dedupe: bool,
    pub sort: SortOrder,
    pub format: Format,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            path_style: PathStyle::Any,
            require_separator: true,
            keep_line_numbers: false,
            output: OutputKind::Path,
            extensions: Vec::new(),
            extension_mode: ExtensionMode::Include,
            dedupe: true,
            sort: SortOrder::FirstSeen,
            format: Format::List,
        }
    }
}

fn bad_choice(param: &str, got: &str, allowed: &[&str]) -> String {
    format!(
        "{param} must be one of {} — got \"{got}\"",
        allowed.join(", ")
    )
}

impl Options {
    /// Build options from the raw string/bool values every surface supplies.
    #[allow(clippy::too_many_arguments)]
    pub fn parse(
        path_style: &str,
        require_separator: bool,
        keep_line_numbers: bool,
        output: &str,
        extensions: &str,
        extension_mode: &str,
        dedupe: bool,
        sort: &str,
        format: &str,
    ) -> Result<Self, String> {
        Ok(Self {
            path_style: match path_style.trim() {
                "" | "any" => PathStyle::Any,
                "posix" => PathStyle::Posix,
                "windows" => PathStyle::Windows,
                other => {
                    return Err(bad_choice(
                        "path_style",
                        other,
                        &["any", "posix", "windows"],
                    ))
                }
            },
            require_separator,
            keep_line_numbers,
            output: match output.trim() {
                "" | "path" => OutputKind::Path,
                "filename" => OutputKind::Filename,
                "directory" => OutputKind::Directory,
                other => {
                    return Err(bad_choice(
                        "output",
                        other,
                        &["path", "filename", "directory"],
                    ))
                }
            },
            extensions: parse_extension_list(extensions),
            extension_mode: match extension_mode.trim() {
                "" | "include" => ExtensionMode::Include,
                "exclude" => ExtensionMode::Exclude,
                other => return Err(bad_choice("extension_mode", other, &["include", "exclude"])),
            },
            dedupe,
            sort: match sort.trim() {
                "" | "first-seen" => SortOrder::FirstSeen,
                "asc" => SortOrder::Asc,
                "desc" => SortOrder::Desc,
                other => return Err(bad_choice("sort", other, &["first-seen", "asc", "desc"])),
            },
            format: match format.trim() {
                "" | "list" => Format::List,
                "csv" => Format::Csv,
                "json" => Format::Json,
                other => return Err(bad_choice("format", other, &["list", "csv", "json"])),
            },
        })
    }
}

/// `"rs, .toml  py"` → `["rs", "toml", "py"]` (lowercased, dots and blanks dropped).
fn parse_extension_list(raw: &str) -> Vec<String> {
    raw.split([',', ';', ' ', '\t', '\n'])
        .map(|s| s.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// One returned value plus what was learned about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Match {
    /// The value, already projected per `output` (path, filename or directory).
    pub path: String,
    /// Line number parsed off a `path:LINE` / `path(LINE,COL)` suffix, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Column number parsed off a `path:LINE:COL` / `path(LINE,COL)` suffix, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    /// How many times this value occurred in the text.
    pub occurrences: usize,
}

/// What every surface gets back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Extracted {
    /// Number of values returned (after filtering, and after de-duplication when on).
    pub count: usize,
    pub matches: Vec<Match>,
    /// The `matches` rendered per the `format` option — what the page displays.
    pub formatted: String,
}

// ---------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Posix,
    Windows,
    /// A bare filename — belongs to neither flavour, so it survives both filters.
    Neutral,
}

#[derive(Debug, Clone)]
struct Hit {
    path: String,
    line: Option<u32>,
    column: Option<u32>,
    style: Style,
}

thread_local! {
    static DQUOTED: Regex = Regex::new("\"([^\"\\n]{1,4096})\"").unwrap();
    static SQUOTED: Regex = Regex::new("'([^'\\n]{1,4096})'").unwrap();
    static BQUOTED: Regex = Regex::new("`([^`\\n]{1,4096})`").unwrap();
}

/// Wrapper punctuation that never belongs to a path when it leads the token.
const LEAD_TRIM: &[char] = &['(', '[', '{', '<', '"', '\'', '`', '*', '=', ','];
/// Trailing prose punctuation. Closing brackets are handled separately (balanced).
const TRAIL_TRIM: &[char] = &['"', '\'', '`', ',', ';', ':', '!', '?', '.', '*', '='];

/// Strip prose wrappers/punctuation from around a raw token.
fn strip_wrappers(mut s: &str) -> &str {
    loop {
        let before = s;
        s = s.trim_start_matches(LEAD_TRIM).trim_end_matches(TRAIL_TRIM);
        // Only drop a closing bracket when it is unbalanced — `main.c(12,4)` keeps its pair.
        for (open, close) in [('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')] {
            while s.ends_with(close) && s.matches(open).count() < s.matches(close).count() {
                s = &s[..s.len() - close.len_utf8()];
            }
        }
        if s == before {
            return s;
        }
    }
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// Split a trailing `:LINE`, `:LINE:COL` or `(LINE,COL)` locator off a candidate.
fn split_locator(s: &str) -> (&str, Option<u32>, Option<u32>) {
    // MSVC / Node style: `src/main.c(12,4)` or `src/main.c(12)`.
    if let Some(open) = s.rfind('(') {
        if s.ends_with(')') && open > 0 {
            let inner = &s[open + 1..s.len() - 1];
            let (l, c) = match inner.split_once(',') {
                Some((a, b)) => (a.trim(), Some(b.trim())),
                None => (inner, None),
            };
            let col_ok = c.map(all_digits).unwrap_or(true);
            if all_digits(l) && col_ok {
                return (
                    &s[..open],
                    l.parse().ok(),
                    c.and_then(|b| b.parse::<u32>().ok()),
                );
            }
        }
    }
    // grep / rustc / Python style: `src/main.rs:42` or `src/main.rs:42:9`.
    // A bare drive letter (`C`) is never a path, so `C:42` is left alone.
    let is_droppable_base = |b: &str| b.len() > 1 || !b.chars().all(|c| c.is_ascii_alphabetic());
    if let Some((head, last)) = s.rsplit_once(':') {
        if all_digits(last) && is_droppable_base(head) {
            if let Some((head2, mid)) = head.rsplit_once(':') {
                if all_digits(mid) && is_droppable_base(head2) && !head2.is_empty() {
                    return (head2, mid.parse().ok(), last.parse().ok());
                }
            }
            if !head.is_empty() {
                return (head, last.parse().ok(), None);
            }
        }
    }
    (s, None, None)
}

/// Decide whether a cleaned-up candidate really is a file path.
fn classify(raw: &str, require_separator: bool) -> Option<Hit> {
    let cand = strip_wrappers(raw.trim());
    if cand.is_empty() || cand.len() > MAX_PATH_LEN {
        return None;
    }
    // URLs are a different tool's job — `https://example.com/a/b` is not a file path.
    if cand.contains("://") {
        return None;
    }
    let (base, line, column) = split_locator(cand);
    let base = strip_wrappers(base);
    if base.is_empty() || base.len() > MAX_PATH_LEN {
        return None;
    }
    // Characters that are illegal in a path on every platform we claim to support.
    if base.contains(['<', '>', '|', '"', '\n', '\t']) {
        return None;
    }
    // Something nameable has to be in there — `/`, `..`, `///` are not paths.
    if !base.chars().any(|c| c.is_alphanumeric()) {
        return None;
    }

    let has_fwd = base.contains('/');
    let has_back = base.contains('\\');
    let drive = {
        let b = base.as_bytes();
        b.len() >= 3
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && (b[2] == b'\\' || b[2] == b'/')
    };
    let unc = base.starts_with("\\\\");

    let style = if drive || unc || (has_back && !has_fwd) {
        Style::Windows
    } else if has_fwd {
        Style::Posix
    } else if has_back {
        Style::Windows
    } else {
        Style::Neutral
    };

    if style == Style::Neutral {
        if require_separator {
            return None;
        }
        // A bare filename only counts with a word-like extension, so that prose,
        // version numbers (`1.2.3`) and decimals (`3.14`) stay out.
        let (stem, ext) = base.rsplit_once('.')?;
        if stem.is_empty()
            || ext.is_empty()
            || ext.len() > 8
            || !ext.chars().all(|c| c.is_ascii_alphanumeric())
            || !ext.chars().any(|c| c.is_ascii_alphabetic())
        {
            return None;
        }
    } else {
        // A path made only of numeric segments is a date or a ratio (`2026/08/17`).
        let numeric_only = base
            .split(['/', '\\'])
            .filter(|s| !s.is_empty())
            .all(all_digits);
        if numeric_only {
            return None;
        }
    }

    Some(Hit {
        path: base.to_string(),
        line,
        column,
        style,
    })
}

/// Does a quoted span with spaces in it look like one anchored path?
fn anchored(s: &str) -> bool {
    let b = s.as_bytes();
    s.starts_with('/')
        || s.starts_with("./")
        || s.starts_with("../")
        || s.starts_with("~/")
        || s.starts_with("\\\\")
        || (b.len() >= 3
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && (b[2] == b'\\' || b[2] == b'/'))
}

/// Byte offset + text of every whitespace-separated token.
fn tokens(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, ch) in text.char_indices() {
        if ch.is_whitespace() {
            if let Some(s) = start.take() {
                out.push((s, &text[s..i]));
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        out.push((s, &text[s..]));
    }
    out
}

/// Find every path in `text`, in order of first appearance.
fn scan(text: &str, require_separator: bool) -> Vec<Hit> {
    let mut hits: Vec<(usize, Hit)> = Vec::new();
    // Bytes already claimed by a quoted path, so the token pass doesn't re-split them.
    let mut claimed = vec![false; text.len()];

    for re in [&DQUOTED, &SQUOTED, &BQUOTED] {
        re.with(|re| {
            for caps in re.captures_iter(text) {
                let full = caps.get(0).unwrap();
                let m = caps.get(1).unwrap();
                let inner = m.as_str();
                if claimed[full.start()] || claimed[m.start()] {
                    continue;
                }
                // Spaces are only trusted inside an anchored path; otherwise a stray
                // apostrophe in prose would swallow the real path next to it.
                if inner.chars().any(char::is_whitespace) && !anchored(inner.trim()) {
                    continue;
                }
                if let Some(hit) = classify(inner, require_separator) {
                    hits.push((m.start(), hit));
                    claimed[full.start()..full.end()].fill(true);
                }
            }
        });
    }

    for (start, tok) in tokens(text) {
        if claimed[start] {
            continue;
        }
        if let Some(hit) = classify(tok, require_separator) {
            hits.push((start, hit));
        }
    }

    hits.sort_by_key(|(s, _)| *s);
    hits.into_iter().map(|(_, h)| h).collect()
}

// ---------------------------------------------------------------------------
// Projection + rendering
// ---------------------------------------------------------------------------

fn last_separator(path: &str) -> Option<usize> {
    path.rfind(['/', '\\'])
}

fn filename_of(path: &str) -> &str {
    // A trailing separator means the path already names a directory.
    let trimmed = path.trim_end_matches(['/', '\\']);
    match last_separator(trimmed) {
        Some(i) => &trimmed[i + 1..],
        None => trimmed,
    }
}

fn directory_of(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    match last_separator(trimmed) {
        // `/etc/hosts` → `/`, `C:\a` → `C:\`
        Some(0) => trimmed[..1].to_string(),
        Some(i) => trimmed[..i].to_string(),
        None => ".".to_string(),
    }
}

/// Lowercase extension of a path's filename, without the dot.
fn extension_of(path: &str) -> String {
    let name = filename_of(path);
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => ext.to_ascii_lowercase(),
        _ => String::new(),
    }
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) || s.trim() != s {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render(matches: &[Match], o: &Options) -> String {
    match o.format {
        Format::List => {
            if matches.is_empty() {
                "No file paths found.".to_string()
            } else {
                matches
                    .iter()
                    .map(|m| m.path.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        Format::Csv => {
            let header = match o.output {
                OutputKind::Path => "path",
                OutputKind::Filename => "filename",
                OutputKind::Directory => "directory",
            };
            let mut out = format!("{header},occurrences");
            for m in matches {
                out.push('\n');
                out.push_str(&csv_cell(&m.path));
                out.push(',');
                out.push_str(&m.occurrences.to_string());
            }
            out
        }
        Format::Json => serde_json::to_string_pretty(&serde_json::json!({
            "count": matches.len(),
            "matches": matches,
        }))
        .unwrap_or_else(|e| format!("could not serialize result: {e}")),
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Extract every file path in `text` under `o`.
pub fn extract(text: &str, o: &Options) -> Result<Extracted, String> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {MAX_INPUT_BYTES} bytes (about 1 MB) — split the log and run it in parts",
            text.len()
        ));
    }

    let hits = scan(text, o.require_separator);
    if hits.len() > MAX_MATCHES {
        return Err(format!(
            "found {} path occurrences; the limit is {MAX_MATCHES} — narrow the input or filter by extension",
            hits.len()
        ));
    }

    let wanted_style = |s: Style| match o.path_style {
        PathStyle::Any => true,
        // A bare filename belongs to neither flavour, so it survives both filters.
        PathStyle::Posix => s != Style::Windows,
        PathStyle::Windows => s != Style::Posix,
    };

    let mut matches: Vec<Match> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();

    for hit in hits {
        if !wanted_style(hit.style) {
            continue;
        }
        if !o.extensions.is_empty() {
            let ext = extension_of(&hit.path);
            let listed = o.extensions.iter().any(|e| *e == ext);
            let keep = match o.extension_mode {
                ExtensionMode::Include => listed,
                ExtensionMode::Exclude => !listed,
            };
            if !keep {
                continue;
            }
        }

        let value = match o.output {
            OutputKind::Path => {
                let mut v = hit.path.clone();
                if o.keep_line_numbers {
                    if let Some(l) = hit.line {
                        v.push(':');
                        v.push_str(&l.to_string());
                        if let Some(c) = hit.column {
                            v.push(':');
                            v.push_str(&c.to_string());
                        }
                    }
                }
                v
            }
            OutputKind::Filename => filename_of(&hit.path).to_string(),
            OutputKind::Directory => directory_of(&hit.path),
        };

        if o.dedupe {
            if let Some(&i) = index.get(&value) {
                matches[i].occurrences += 1;
                continue;
            }
            index.insert(value.clone(), matches.len());
        }
        matches.push(Match {
            path: value,
            line: hit.line,
            column: hit.column,
            occurrences: 1,
        });
    }

    match o.sort {
        SortOrder::FirstSeen => {}
        SortOrder::Asc => matches.sort_by(|a, b| {
            a.path
                .to_lowercase()
                .cmp(&b.path.to_lowercase())
                .then_with(|| a.path.cmp(&b.path))
        }),
        SortOrder::Desc => matches.sort_by(|a, b| {
            b.path
                .to_lowercase()
                .cmp(&a.path.to_lowercase())
                .then_with(|| b.path.cmp(&a.path))
        }),
    }

    let formatted = render(&matches, o);
    Ok(Extracted {
        count: matches.len(),
        matches,
        formatted,
    })
}

/// String-in/string-out convenience used by the browser wrapper.
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    path_style: &str,
    require_separator: bool,
    keep_line_numbers: bool,
    output: &str,
    extensions: &str,
    extension_mode: &str,
    dedupe: bool,
    sort: &str,
    format: &str,
) -> Result<String, String> {
    let o = Options::parse(
        path_style,
        require_separator,
        keep_line_numbers,
        output,
        extensions,
        extension_mode,
        dedupe,
        sort,
        format,
    )?;
    Ok(extract(text, &o)?.formatted)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(text: &str, o: &Options) -> Vec<String> {
        extract(text, o)
            .unwrap()
            .matches
            .into_iter()
            .map(|m| m.path)
            .collect()
    }

    #[test]
    fn extracts_and_dedupes_from_a_build_log() {
        let log = "\
error[E0308]: mismatched types
  --> src/main.rs:42:9
warning: unused import in src/main.rs
   Compiling foo (/home/dev/projects/foo)";
        let got = paths(log, &Options::default());
        assert_eq!(
            got,
            vec!["src/main.rs", "/home/dev/projects/foo"],
            "duplicate src/main.rs collapses; the locator is stripped"
        );
        let r = extract(log, &Options::default()).unwrap();
        assert_eq!(r.count, 2);
        assert_eq!(r.matches[0].occurrences, 2);
        assert_eq!(r.matches[0].line, Some(42));
        assert_eq!(r.matches[0].column, Some(9));
    }

    #[test]
    fn rejects_bad_enum_value_with_an_actionable_message() {
        let err = Options::parse(
            "macos",
            true,
            false,
            "path",
            "",
            "include",
            true,
            "first-seen",
            "list",
        )
        .unwrap_err();
        assert_eq!(
            err,
            "path_style must be one of any, posix, windows — got \"macos\""
        );
    }

    #[test]
    fn rejects_oversized_input() {
        let big = "a/b.txt ".repeat(MAX_INPUT_BYTES / 8 + 1);
        assert!(big.len() > MAX_INPUT_BYTES);
        let err = extract(&big, &Options::default()).unwrap_err();
        assert!(err.contains("the limit is 1000000 bytes"), "{err}");
    }

    #[test]
    fn accepts_input_exactly_at_the_cap() {
        let mut s = String::from("src/main.rs ");
        s.push_str(&"x".repeat(MAX_INPUT_BYTES - s.len()));
        assert_eq!(s.len(), MAX_INPUT_BYTES);
        assert_eq!(extract(&s, &Options::default()).unwrap().count, 1);
    }

    #[test]
    fn rejects_too_many_matches() {
        let text = "a/b.txt\n".repeat(MAX_MATCHES + 1);
        let err = extract(&text, &Options::default()).unwrap_err();
        assert!(err.contains("the limit is 20000"), "{err}");
    }

    #[test]
    fn windows_drive_unc_and_backslash_paths() {
        let o = Options::default();
        assert_eq!(
            paths(r"open C:\Users\dev\app.log now", &o),
            vec![r"C:\Users\dev\app.log"]
        );
        assert_eq!(
            paths(r"see \\srv\share\report.csv", &o),
            vec![r"\\srv\share\report.csv"]
        );
        assert_eq!(
            paths(r"at src\lib\util.cs line", &o),
            vec![r"src\lib\util.cs"]
        );
    }

    #[test]
    fn quoted_paths_may_contain_spaces_when_anchored() {
        let o = Options::default();
        assert_eq!(
            paths("copy \"C:\\Program Files\\App\\run.exe\" here", &o),
            vec![r"C:\Program Files\App\run.exe"]
        );
        assert_eq!(
            paths("  File \"/srv/my app/main.py\", line 12", &o),
            vec!["/srv/my app/main.py"]
        );
    }

    #[test]
    fn prose_apostrophes_do_not_swallow_a_real_path() {
        // "don't ... don't" forms a single-quoted span around the real path.
        let got = paths("don't touch /etc/hosts, don't", &Options::default());
        assert_eq!(got, vec!["/etc/hosts"]);
    }

    #[test]
    fn strips_wrappers_and_prose_punctuation() {
        let o = Options::default();
        assert_eq!(paths("edit (src/main.rs).", &o), vec!["src/main.rs"]);
        assert_eq!(paths("[docs/readme.md];", &o), vec!["docs/readme.md"]);
        assert_eq!(
            paths("`./scripts/build.sh`", &o),
            vec!["./scripts/build.sh"]
        );
        assert_eq!(
            paths("~/.config/nvim/init.lua!", &o),
            vec!["~/.config/nvim/init.lua"]
        );
    }

    #[test]
    fn msvc_style_locator_is_parsed() {
        let r = extract(r"src\main.c(12,4): error C2065", &Options::default()).unwrap();
        assert_eq!(r.matches[0].path, r"src\main.c");
        assert_eq!(r.matches[0].line, Some(12));
        assert_eq!(r.matches[0].column, Some(4));
    }

    #[test]
    fn urls_dates_and_prose_are_not_paths() {
        let o = Options::default();
        assert!(paths("visit https://example.com/a/b now", &o).is_empty());
        assert!(paths("on 2026/08/17 at 12:34 the ratio was 3/4", &o).is_empty());
        assert!(paths("just some ordinary words here", &o).is_empty());
        assert!(paths("////  ..  ...", &o).is_empty());
    }

    #[test]
    fn bare_filenames_need_the_opt_in() {
        let text = "rebuilt main.rs and Cargo.toml, version 1.2.3, pi is 3.14";
        assert!(paths(text, &Options::default()).is_empty());
        let o = Options {
            require_separator: false,
            ..Options::default()
        };
        assert_eq!(paths(text, &o), vec!["main.rs", "Cargo.toml"]);
    }

    #[test]
    fn path_style_filter() {
        let text = r"src/a.rs and C:\tmp\b.txt and bare.md";
        let mut o = Options {
            require_separator: false,
            path_style: PathStyle::Posix,
            ..Options::default()
        };
        assert_eq!(paths(text, &o), vec!["src/a.rs", "bare.md"]);
        o.path_style = PathStyle::Windows;
        assert_eq!(paths(text, &o), vec![r"C:\tmp\b.txt", "bare.md"]);
    }

    #[test]
    fn extension_include_and_exclude() {
        let text = "src/a.rs src/b.toml src/c.RS docs/d.md";
        let mut o = Options {
            extensions: parse_extension_list(".rs, md"),
            ..Options::default()
        };
        assert_eq!(paths(text, &o), vec!["src/a.rs", "src/c.RS", "docs/d.md"]);
        o.extension_mode = ExtensionMode::Exclude;
        assert_eq!(paths(text, &o), vec!["src/b.toml"]);
    }

    #[test]
    fn output_projections() {
        let text = "src/app/main.rs /etc/hosts";
        let mut o = Options::default();
        o.output = OutputKind::Filename;
        assert_eq!(paths(text, &o), vec!["main.rs", "hosts"]);
        o.output = OutputKind::Directory;
        assert_eq!(paths(text, &o), vec!["src/app", "/etc"]);
        o.output = OutputKind::Directory;
        assert_eq!(paths("bare/x.txt", &o), vec!["bare"]);
    }

    #[test]
    fn keep_line_numbers_round_trip() {
        let text = "src/a.rs:10:2 and src/a.rs:11";
        let o = Options {
            keep_line_numbers: true,
            ..Options::default()
        };
        assert_eq!(paths(text, &o), vec!["src/a.rs:10:2", "src/a.rs:11"]);
        assert_eq!(paths(text, &Options::default()), vec!["src/a.rs"]);
    }

    #[test]
    fn dedupe_off_keeps_every_occurrence() {
        let o = Options {
            dedupe: false,
            ..Options::default()
        };
        assert_eq!(paths("a/x.txt a/x.txt", &o), vec!["a/x.txt", "a/x.txt"]);
    }

    #[test]
    fn sort_orders() {
        let text = "z/b.txt a/A.txt m/c.txt";
        let mut o = Options::default();
        o.sort = SortOrder::Asc;
        assert_eq!(paths(text, &o), vec!["a/A.txt", "m/c.txt", "z/b.txt"]);
        o.sort = SortOrder::Desc;
        assert_eq!(paths(text, &o), vec!["z/b.txt", "m/c.txt", "a/A.txt"]);
    }

    #[test]
    fn formats() {
        let text = "src/a.rs src/a.rs docs/b, md";
        let mut o = Options::default();
        assert_eq!(extract(text, &o).unwrap().formatted, "src/a.rs\ndocs/b");
        o.format = Format::Csv;
        assert_eq!(
            extract(text, &o).unwrap().formatted,
            "path,occurrences\nsrc/a.rs,2\ndocs/b,1"
        );
        o.format = Format::Json;
        let json: serde_json::Value =
            serde_json::from_str(&extract(text, &o).unwrap().formatted).unwrap();
        assert_eq!(json["count"], 2);
        assert_eq!(json["matches"][0]["path"], "src/a.rs");
        assert_eq!(json["matches"][0]["occurrences"], 2);
        assert!(json["matches"][0].get("line").is_none());
    }

    #[test]
    fn empty_input_says_so() {
        assert_eq!(
            extract("", &Options::default()).unwrap().formatted,
            "No file paths found."
        );
    }

    #[test]
    fn csv_quotes_values_containing_commas() {
        let o = Options {
            format: Format::Csv,
            ..Options::default()
        };
        assert_eq!(
            extract("\"/tmp/a,b/c.txt\"", &o).unwrap().formatted,
            "path,occurrences\n\"/tmp/a,b/c.txt\",1"
        );
    }

    #[test]
    fn git_status_output() {
        let text = "\
On branch main
Changes not staged for commit:
\tmodified:   src/lib.rs
\tdeleted:    docs/old.md
Untracked files:
\tnotes/todo.txt";
        assert_eq!(
            paths(text, &Options::default()),
            vec!["src/lib.rs", "docs/old.md", "notes/todo.txt"]
        );
    }
}
