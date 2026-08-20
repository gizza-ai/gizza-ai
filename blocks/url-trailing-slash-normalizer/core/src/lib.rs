//! url-trailing-slash-normalizer core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Takes a batch of URLs (one per line) and makes their trailing slashes
//! consistent: either every directory-style URL ends in `/` (`add`) or none of
//! them do (`remove`). Only the PATH is rewritten — the scheme, host, port,
//! query string and fragment are copied through byte-for-byte, and nothing is
//! re-encoded. File-like paths (a last segment with a real extension, e.g.
//! `/sitemap.xml`) are left alone by default, because `/sitemap.xml/` is a
//! different resource on every server. The root path stays `/`.

/// Hard cap on the input size, so a paste can't wedge the browser tab.
pub const MAX_BYTES: usize = 1_000_000;
/// Hard cap on how many URLs one run normalizes.
pub const MAX_URLS: usize = 20_000;
/// Longest run of characters after the final `.` still treated as an extension.
const MAX_EXT_LEN: usize = 10;

/// What happened to one input line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// A trailing slash was added (or a run of them collapsed to one).
    Added,
    /// The trailing slash(es) were removed.
    Removed,
    /// Already in the requested style — returned verbatim.
    Unchanged,
    /// A bare or repeated root (`https://example.com`, `…com//`) became `…com/`.
    Root,
    /// Left alone because the last path segment looks like a file.
    SkippedFile,
    /// Not recognizable as a URL or path.
    Invalid,
    /// Normalized to a URL an earlier line already produced.
    Duplicate,
}

impl Action {
    /// Stable machine-readable label used in the CSV report.
    pub fn label(self) -> &'static str {
        match self {
            Action::Added => "added",
            Action::Removed => "removed",
            Action::Unchanged => "unchanged",
            Action::Root => "root",
            Action::SkippedFile => "skipped-file",
            Action::Invalid => "invalid",
            Action::Duplicate => "duplicate",
        }
    }
}

/// One normalized line.
#[derive(Clone, Debug)]
pub struct Row {
    /// 1-based line number in the input.
    pub line: usize,
    /// The line as given (trimmed of surrounding whitespace).
    pub original: String,
    /// The rewritten URL (equal to `original` when nothing changed).
    pub normalized: String,
    /// What the normalizer did.
    pub action: Action,
}

/// Add a trailing slash to directory-style URLs, or remove it everywhere.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Add,
    Remove,
}

/// What to do with a line that isn't a URL.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OnInvalid {
    /// Pass it through untouched (default) — safe for annotated lists.
    Keep,
    /// Leave it out of the result.
    Drop,
    /// Fail the whole run, naming the line.
    Error,
}

/// Which report to return.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    /// Every URL, normalized, one per line.
    Urls,
    /// Only the URLs whose trailing slash actually changed.
    Changed,
    /// `line,original,normalized,action` CSV for every input line.
    Report,
    /// `metric,value` CSV of the run totals.
    Summary,
}

fn parse_mode(v: &str) -> Result<Mode, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" | "add" => Ok(Mode::Add),
        "remove" | "strip" => Ok(Mode::Remove),
        other => Err(format!(
            "mode must be \"add\" or \"remove\", got \"{other}\""
        )),
    }
}

fn parse_on_invalid(v: &str) -> Result<OnInvalid, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" | "keep" => Ok(OnInvalid::Keep),
        "drop" => Ok(OnInvalid::Drop),
        "error" => Ok(OnInvalid::Error),
        other => Err(format!(
            "on_invalid must be \"keep\", \"drop\" or \"error\", got \"{other}\""
        )),
    }
}

fn parse_output(v: &str) -> Result<Output, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" | "urls" => Ok(Output::Urls),
        "changed" => Ok(Output::Changed),
        "report" => Ok(Output::Report),
        "summary" => Ok(Output::Summary),
        other => Err(format!(
            "output must be \"urls\", \"changed\", \"report\" or \"summary\", got \"{other}\""
        )),
    }
}

/// The three pieces of a URL: everything before the path, the path itself, and
/// the `?query#fragment` tail. Only the middle piece is ever rewritten.
struct Parts<'a> {
    prefix: &'a str,
    path: &'a str,
    suffix: &'a str,
}

/// Split a line into prefix/path/suffix, or `None` when it isn't a URL or path.
///
/// Recognized forms: `https://host/path`, any `scheme://host/path`,
/// scheme-relative `//host/path`, bare `host/path` (and `host:8080/path`), and
/// path-only `/path`. Scheme forms without an authority (`mailto:`, `tel:`,
/// `data:`, `javascript:`) are rejected — they have no path to normalize.
fn split_url(line: &str) -> Option<Parts<'_>> {
    if line.is_empty() || line.chars().any(|c| c.is_whitespace()) {
        return None;
    }
    let cut = line
        .find(['?', '#'])
        .unwrap_or(line.len());
    let (before, suffix) = line.split_at(cut);
    if before.is_empty() {
        // A bare "?q=1" / "#top" has no path.
        return None;
    }

    let auth_start = if before.starts_with("//") {
        Some(2)
    } else if let Some(i) = before.find("://") {
        let scheme = &before[..i];
        let valid = !scheme.is_empty()
            && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
        if !valid {
            return None;
        }
        Some(i + 3)
    } else if before.starts_with('/') {
        None
    } else if let Some(i) = before.find(':') {
        // "host:8080/path" is an authority with a port; "mailto:a@b" is not.
        let after = &before[i + 1..];
        let port: &str = after.split('/').next().unwrap_or(after);
        if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        Some(0)
    } else {
        Some(0)
    };

    match auth_start {
        None => Some(Parts {
            prefix: "",
            path: before,
            suffix,
        }),
        Some(start) => {
            let rel = &before[start..];
            let path_start = rel.find('/').map(|i| start + i).unwrap_or(before.len());
            let authority = &before[start..path_start];
            if authority.is_empty() {
                return None;
            }
            // A scheme-less bare authority must at least look like a host, so a
            // stray word ("todo") isn't silently turned into "todo/".
            if start == 0 && !authority.contains('.') && !authority.starts_with("localhost") {
                return None;
            }
            Some(Parts {
                prefix: &before[..path_start],
                path: &before[path_start..],
                suffix,
            })
        }
    }
}

/// The last non-empty path segment (`/a/b/` → `b`, `/a/b.html` → `b.html`).
fn last_segment(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(i) => &trimmed[i + 1..],
        None => trimmed,
    }
}

/// Does the last path segment look like a file rather than a directory?
///
/// True for `report.pdf`, `index.html`, `logo.svg`; false for `v1.2` (no letter
/// in the extension), `.well-known` (leading dot) and `about` (no dot at all).
pub fn is_file_like(path: &str) -> bool {
    let seg = last_segment(path);
    let Some(dot) = seg.rfind('.') else {
        return false;
    };
    if dot == 0 || dot + 1 >= seg.len() {
        return false;
    }
    let ext = &seg[dot + 1..];
    ext.len() <= MAX_EXT_LEN
        && ext.chars().all(|c| c.is_ascii_alphanumeric())
        && ext.chars().any(|c| c.is_ascii_alphabetic())
}

/// Normalize a single already-trimmed URL. Returns the rewritten URL and what
/// was done to it.
pub fn normalize_one(
    line: &str,
    mode: Mode,
    skip_file_paths: bool,
    normalize_root: bool,
) -> (String, Action) {
    let Some(p) = split_url(line) else {
        return (line.to_string(), Action::Invalid);
    };

    // Root: no path at all, or nothing but slashes.
    if p.path.is_empty() || p.path.chars().all(|c| c == '/') {
        if !normalize_root {
            return (line.to_string(), Action::Unchanged);
        }
        let out = format!("{}/{}", p.prefix, p.suffix);
        let action = if out == line {
            Action::Unchanged
        } else {
            Action::Root
        };
        return (out, action);
    }

    if skip_file_paths && is_file_like(p.path) {
        return (line.to_string(), Action::SkippedFile);
    }

    let stem = p.path.trim_end_matches('/');
    let new_path = match mode {
        Mode::Add => format!("{stem}/"),
        // `stem` can't be empty here — an all-slash path took the root branch.
        Mode::Remove => stem.to_string(),
    };
    let out = format!("{}{}{}", p.prefix, new_path, p.suffix);
    if out == line {
        return (out, Action::Unchanged);
    }
    let action = match mode {
        Mode::Add => Action::Added,
        Mode::Remove => Action::Removed,
    };
    (out, action)
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Normalize a whole batch and render the requested `output`.
///
/// `mode` is `add`|`remove`, `on_invalid` is `keep`|`drop`|`error`, `output` is
/// `urls`|`changed`|`report`|`summary`. Empty strings take the defaults.
#[allow(clippy::too_many_arguments)]
pub fn normalize(
    input: &str,
    mode: &str,
    skip_file_paths: bool,
    normalize_root: bool,
    dedupe: bool,
    on_invalid: &str,
    output: &str,
) -> Result<String, String> {
    let mode = parse_mode(mode)?;
    let on_invalid = parse_on_invalid(on_invalid)?;
    let output = parse_output(output)?;

    if input.len() > MAX_BYTES {
        return Err(format!(
            "input is too large: {} bytes, max {} bytes",
            input.len(),
            MAX_BYTES
        ));
    }
    if input.trim().is_empty() {
        return Err("input is empty — paste one URL per line".into());
    }

    let mut rows: Vec<Row> = Vec::new();
    let mut input_lines = 0usize;
    for (idx, raw) in input.lines().enumerate() {
        input_lines = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if rows.len() == MAX_URLS {
            return Err(format!(
                "too many URLs: more than {MAX_URLS} non-empty lines (line {} and beyond) — split the list and run it in batches",
                idx + 1
            ));
        }
        let (normalized, action) = normalize_one(line, mode, skip_file_paths, normalize_root);
        if action == Action::Invalid && on_invalid == OnInvalid::Error {
            return Err(format!(
                "line {}: \"{}\" is not a URL — expected https://host/path, //host/path, host/path or /path (set on_invalid=keep to pass unrecognized lines through)",
                idx + 1,
                line
            ));
        }
        rows.push(Row {
            line: idx + 1,
            original: line.to_string(),
            normalized,
            action,
        });
    }
    if rows.is_empty() {
        return Err("input is empty — paste one URL per line".into());
    }

    // Emission rules: invalid lines follow `on_invalid`; duplicates are dropped
    // (and relabeled) only when `dedupe` is on.
    let mut seen: Vec<String> = Vec::new();
    let mut emit: Vec<bool> = Vec::with_capacity(rows.len());
    let mut duplicates = 0usize;
    for row in rows.iter_mut() {
        let mut keep = true;
        if row.action == Action::Invalid && on_invalid == OnInvalid::Drop {
            keep = false;
        } else if dedupe {
            if seen.iter().any(|s| s == &row.normalized) {
                row.action = Action::Duplicate;
                duplicates += 1;
                keep = false;
            } else {
                seen.push(row.normalized.clone());
            }
        }
        emit.push(keep);
    }

    let out = match output {
        Output::Urls => rows
            .iter()
            .zip(&emit)
            .filter(|(_, keep)| **keep)
            .map(|(r, _)| r.normalized.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        Output::Changed => {
            let changed: Vec<&str> = rows
                .iter()
                .zip(&emit)
                .filter(|(r, keep)| **keep && r.normalized != r.original)
                .map(|(r, _)| r.normalized.as_str())
                .collect();
            if changed.is_empty() {
                format!(
                    "# no changes — all {} URL(s) already match the requested trailing-slash style",
                    rows.len()
                )
            } else {
                changed.join("\n")
            }
        }
        Output::Report => {
            let mut s = String::from("line,original,normalized,action");
            for row in &rows {
                s.push('\n');
                s.push_str(&format!(
                    "{},{},{},{}",
                    row.line,
                    csv_field(&row.original),
                    csv_field(&row.normalized),
                    row.action.label()
                ));
            }
            s
        }
        Output::Summary => {
            let count = |a: Action| rows.iter().filter(|r| r.action == a).count();
            let returned = emit.iter().filter(|k| **k).count();
            let mut s = String::from("metric,value");
            for (k, v) in [
                ("input_lines", input_lines),
                ("urls_processed", rows.len()),
                ("trailing_slash_added", count(Action::Added)),
                ("trailing_slash_removed", count(Action::Removed)),
                ("root_normalized", count(Action::Root)),
                ("already_correct", count(Action::Unchanged)),
                ("file_paths_left_alone", count(Action::SkippedFile)),
                ("unrecognized_lines", count(Action::Invalid)),
                ("duplicates_removed", duplicates),
                ("urls_returned", returned),
            ] {
                s.push_str(&format!("\n{k},{v}"));
            }
            s
        }
    };
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(input: &str) -> String {
        normalize(input, "add", true, true, false, "keep", "urls").unwrap()
    }

    fn remove(input: &str) -> String {
        normalize(input, "remove", true, true, false, "keep", "urls").unwrap()
    }

    #[test]
    fn adds_trailing_slash_to_directory_paths() {
        assert_eq!(
            add("https://example.com/blog\nhttps://example.com/blog/about"),
            "https://example.com/blog/\nhttps://example.com/blog/about/"
        );
    }

    #[test]
    fn removes_trailing_slash() {
        assert_eq!(
            remove("https://example.com/blog/\nhttps://example.com/blog/about/"),
            "https://example.com/blog\nhttps://example.com/blog/about"
        );
    }

    #[test]
    fn leaves_file_like_paths_alone_in_both_directions() {
        assert_eq!(
            add("https://example.com/sitemap.xml\nhttps://example.com/a/report.pdf"),
            "https://example.com/sitemap.xml\nhttps://example.com/a/report.pdf"
        );
        assert_eq!(
            remove("https://example.com/style.css/"),
            "https://example.com/style.css/"
        );
    }

    #[test]
    fn skip_file_paths_off_forces_the_rewrite() {
        assert_eq!(
            normalize(
                "https://example.com/sitemap.xml",
                "add",
                false,
                true,
                false,
                "keep",
                "urls"
            )
            .unwrap(),
            "https://example.com/sitemap.xml/"
        );
    }

    #[test]
    fn extension_needs_a_letter_so_versions_are_directories() {
        assert_eq!(add("https://example.com/api/v1.2"), "https://example.com/api/v1.2/");
        assert!(!is_file_like("/api/v1.2"));
        assert!(is_file_like("/a/index.html"));
        assert!(!is_file_like("/.well-known"));
    }

    #[test]
    fn root_is_always_a_single_slash() {
        assert_eq!(
            add("https://example.com\nhttps://example.com//"),
            "https://example.com/\nhttps://example.com/"
        );
        // Even in remove mode the root keeps its slash.
        assert_eq!(remove("https://example.com/"), "https://example.com/");
        assert_eq!(remove("https://example.com"), "https://example.com/");
    }

    #[test]
    fn normalize_root_off_leaves_the_bare_host() {
        assert_eq!(
            normalize(
                "https://example.com",
                "add",
                true,
                false,
                false,
                "keep",
                "urls"
            )
            .unwrap(),
            "https://example.com"
        );
    }

    #[test]
    fn query_and_fragment_are_preserved_verbatim() {
        assert_eq!(
            add("https://example.com/blog?page=2&q=a%20b#top"),
            "https://example.com/blog/?page=2&q=a%20b#top"
        );
        assert_eq!(add("https://example.com?a=1"), "https://example.com/?a=1");
        assert_eq!(
            remove("https://example.com/blog/#top"),
            "https://example.com/blog#top"
        );
    }

    #[test]
    fn repeated_trailing_slashes_collapse() {
        assert_eq!(add("https://example.com/blog///"), "https://example.com/blog/");
        assert_eq!(remove("https://example.com/blog///"), "https://example.com/blog");
    }

    #[test]
    fn accepts_scheme_relative_bare_host_port_and_path_only_lines() {
        assert_eq!(add("//cdn.example.com/assets"), "//cdn.example.com/assets/");
        assert_eq!(add("example.com/blog"), "example.com/blog/");
        assert_eq!(add("example.com:8080/blog"), "example.com:8080/blog/");
        assert_eq!(add("/blog/post"), "/blog/post/");
        assert_eq!(add("ftp://files.example.com/pub"), "ftp://files.example.com/pub/");
    }

    #[test]
    fn unrecognized_lines_are_kept_dropped_or_fatal() {
        let input = "https://example.com/blog\nmailto:hi@example.com";
        assert_eq!(
            normalize(input, "add", true, true, false, "keep", "urls").unwrap(),
            "https://example.com/blog/\nmailto:hi@example.com"
        );
        assert_eq!(
            normalize(input, "add", true, true, false, "drop", "urls").unwrap(),
            "https://example.com/blog/"
        );
        let err = normalize(input, "add", true, true, false, "error", "urls").unwrap_err();
        assert!(err.contains("line 2"), "{err}");
        assert!(err.contains("is not a URL"), "{err}");
    }

    #[test]
    fn dedupe_keeps_the_first_occurrence() {
        assert_eq!(
            normalize(
                "https://example.com/blog\nhttps://example.com/blog/\nhttps://example.com/news",
                "add",
                true,
                true,
                true,
                "keep",
                "urls"
            )
            .unwrap(),
            "https://example.com/blog/\nhttps://example.com/news/"
        );
    }

    #[test]
    fn changed_output_lists_only_rewritten_urls() {
        assert_eq!(
            normalize(
                "https://example.com/blog\nhttps://example.com/news/",
                "add",
                true,
                true,
                false,
                "keep",
                "changed"
            )
            .unwrap(),
            "https://example.com/blog/"
        );
        assert!(normalize(
            "https://example.com/news/",
            "add",
            true,
            true,
            false,
            "keep",
            "changed"
        )
        .unwrap()
        .starts_with("# no changes"));
    }

    #[test]
    fn report_is_csv_with_one_row_per_line() {
        let out = normalize(
            "https://example.com/blog\nhttps://example.com/sitemap.xml\nnot a url",
            "add",
            true,
            true,
            false,
            "keep",
            "report",
        )
        .unwrap();
        assert_eq!(
            out,
            "line,original,normalized,action\n\
             1,https://example.com/blog,https://example.com/blog/,added\n\
             2,https://example.com/sitemap.xml,https://example.com/sitemap.xml,skipped-file\n\
             3,not a url,not a url,invalid"
        );
    }

    #[test]
    fn report_quotes_fields_containing_commas() {
        let out = normalize(
            "https://example.com/a,b",
            "add",
            true,
            true,
            false,
            "keep",
            "report",
        )
        .unwrap();
        assert!(out.contains("\"https://example.com/a,b\",\"https://example.com/a,b/\",added"));
    }

    #[test]
    fn summary_counts_every_bucket() {
        let out = normalize(
            "https://example.com/blog\nhttps://example.com/news/\nhttps://example.com/sitemap.xml\nhttps://example.com\nnope\n\nhttps://example.com/blog/",
            "add",
            true,
            true,
            true,
            "keep",
            "summary",
        )
        .unwrap();
        assert_eq!(
            out,
            "metric,value\n\
             input_lines,7\n\
             urls_processed,6\n\
             trailing_slash_added,1\n\
             trailing_slash_removed,0\n\
             root_normalized,1\n\
             already_correct,1\n\
             file_paths_left_alone,1\n\
             unrecognized_lines,1\n\
             duplicates_removed,1\n\
             urls_returned,5"
        );
    }

    #[test]
    fn blank_lines_are_ignored() {
        assert_eq!(
            add("\n  https://example.com/a  \n\n https://example.com/b\n"),
            "https://example.com/a/\nhttps://example.com/b/"
        );
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = normalize("   \n\n", "add", true, true, false, "keep", "urls").unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn bad_enum_values_name_the_allowed_set() {
        let err = normalize("https://a.com/b", "sideways", true, true, false, "keep", "urls")
            .unwrap_err();
        assert_eq!(err, "mode must be \"add\" or \"remove\", got \"sideways\"");
        let err =
            normalize("https://a.com/b", "add", true, true, false, "maybe", "urls").unwrap_err();
        assert!(err.starts_with("on_invalid must be"), "{err}");
        let err = normalize("https://a.com/b", "add", true, true, false, "keep", "csv").unwrap_err();
        assert!(err.starts_with("output must be"), "{err}");
    }

    #[test]
    fn caps_are_enforced_at_the_boundary() {
        let ok = "https://example.com/a\n".repeat(MAX_URLS);
        assert_eq!(
            normalize(&ok, "add", true, true, false, "keep", "urls")
                .unwrap()
                .lines()
                .count(),
            MAX_URLS
        );
        let over = "https://example.com/a\n".repeat(MAX_URLS + 1);
        let err = normalize(&over, "add", true, true, false, "keep", "urls").unwrap_err();
        assert!(err.contains("too many URLs"), "{err}");

        let huge = "x".repeat(MAX_BYTES + 1);
        let err = normalize(&huge, "add", true, true, false, "keep", "urls").unwrap_err();
        assert!(err.contains("input is too large"), "{err}");
    }
}
