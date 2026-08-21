//! gizza-ai/regex-codemod core — one regex (or literal) find-and-replace applied
//! across a pasted MULTI-FILE bundle, previewed as a unified diff.
//!
//! Input is text where several files are concatenated behind header markers
//! (`=== path ===`, `--- path ---`, `==> path <==`, `# file: path`, or a custom
//! marker regex). Each file is split out, the replacement is applied to that
//! file's body independently, and the result is rendered as a `git apply`-able
//! unified diff, as the fully rewritten bundle, or as a structured JSON report.
//!
//! Pure Rust, no I/O — shared by the chat skill block, the CLI and the web page.
//! Matching uses the `regex` crate, which has no backtracking, so every pattern
//! runs in time linear in the input (no catastrophic-blowup inputs).

use regex::{NoExpand, Regex, RegexBuilder};
use serde_json::json;

/// Maximum size of the pasted bundle, in characters.
pub const MAX_INPUT_CHARS: usize = 1_000_000;
/// Maximum number of files detected in one bundle.
pub const MAX_FILES: usize = 2_000;
/// Path used for content that appears before any file marker (and for the
/// whole input when `file_marker = "none"`).
pub const UNNAMED_PATH: &str = "input";
/// Above this many LCS cells the diff falls back to one coarse replace hunk.
const LCS_CELL_LIMIT: usize = 2_000_000;

/// Matching / rendering options.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Treat `pattern` as literal text and insert `replacement` verbatim.
    pub literal: bool,
    /// Case-insensitive matching (the `i` flag).
    pub ignore_case: bool,
    /// `^` and `$` match at line boundaries (the `m` flag).
    pub multiline: bool,
    /// `.` also matches a newline (the `s` flag).
    pub dot_matches_newline: bool,
    /// Require a word boundary on both sides of the match.
    pub whole_word: bool,
    /// Replace every match; when false only the first match in each file.
    pub replace_all: bool,
    /// Unchanged context lines around each hunk in the unified diff.
    pub context: usize,
    /// Also report files the pattern did not change.
    pub include_unchanged: bool,
}

/// One line of a file body: the text without its terminator, plus whether a
/// terminating newline was present (only the final line can lack one).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Line {
    text: String,
    nl: bool,
}

/// One file carved out of the pasted bundle.
#[derive(Debug, Clone)]
struct FileChunk {
    path: String,
    /// The raw marker line (with its newline), empty for implicit files.
    marker: String,
    body: String,
}

/// Per-file outcome, exposed through the JSON report.
#[derive(Debug, Clone)]
struct FileResult {
    path: String,
    replacements: usize,
    new_body: String,
    diff: String,
}

/// Build the list of marker regexes for a named marker style.
fn marker_patterns(file_marker: &str, marker_regex: &str) -> Result<Vec<Regex>, String> {
    // `==>` must be tried before `===`: the equals marker would otherwise match
    // `==> path <==` and capture `> path <` as the path.
    const ARROW: &str = r"^==>\s*(\S.*?)\s*<==$";
    const EQUALS: &str = r"^={2,}\s*([^=\s].*?)\s*={2,}$";
    const DASHES: &str = r"^-{3,}\s*([^-\s].*?)\s*-{3,}$";
    const COMMENT: &str = r"^\s*(?://|#)\s*[Ff]ile:\s*(\S.*?)\s*$";

    if file_marker != "custom" && !marker_regex.trim().is_empty() {
        return Err(
            "`marker_regex` is only used when `file_marker` is \"custom\" — set file_marker=custom"
                .into(),
        );
    }

    let sources: Vec<&str> = match file_marker {
        "auto" => vec![ARROW, EQUALS, DASHES, COMMENT],
        "equals" => vec![EQUALS],
        "dashes" => vec![DASHES],
        "arrow" => vec![ARROW],
        "comment" => vec![COMMENT],
        "none" => vec![],
        "custom" => {
            if marker_regex.trim().is_empty() {
                return Err("`file_marker` is \"custom\" but `marker_regex` is empty".into());
            }
            vec![marker_regex]
        }
        other => {
            return Err(format!(
                "unknown file_marker {other:?} — expected auto, equals, dashes, arrow, comment, custom or none"
            ))
        }
    };

    let mut out = Vec::with_capacity(sources.len());
    for src in sources {
        let re = Regex::new(src).map_err(|e| format!("invalid marker_regex: {e}"))?;
        if re.captures_len() < 2 {
            return Err(
                "`marker_regex` must contain one capture group for the file path, e.g. ^// (\\S+)$"
                    .into(),
            );
        }
        out.push(re);
    }
    Ok(out)
}

/// Split the pasted bundle into files at every marker line.
fn split_files(text: &str, markers: &[Regex]) -> Result<Vec<FileChunk>, String> {
    let mut files: Vec<FileChunk> = Vec::new();
    let mut current = FileChunk {
        path: UNNAMED_PATH.to_string(),
        marker: String::new(),
        body: String::new(),
    };
    let mut started = false;

    for chunk in text.split_inclusive('\n') {
        let bare = chunk.strip_suffix('\n').unwrap_or(chunk);
        let bare = bare.strip_suffix('\r').unwrap_or(bare);
        let hit = markers
            .iter()
            .find_map(|re| re.captures(bare))
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
        match hit {
            Some(path) => {
                if started || !current.body.is_empty() {
                    files.push(current);
                }
                started = true;
                current = FileChunk {
                    path,
                    marker: chunk.to_string(),
                    body: String::new(),
                };
                if files.len() >= MAX_FILES {
                    return Err(format!(
                        "bundle contains more than {MAX_FILES} files — split it into smaller batches"
                    ));
                }
            }
            None => current.body.push_str(chunk),
        }
    }
    if started || !current.body.is_empty() {
        files.push(current);
    }
    Ok(files)
}

/// Split a file body into lines, remembering whether the last line ended with a newline.
fn to_lines(body: &str) -> Vec<Line> {
    body.split_inclusive('\n')
        .map(|c| match c.strip_suffix('\n') {
            Some(t) => Line {
                text: t.to_string(),
                nl: true,
            },
            None => Line {
                text: c.to_string(),
                nl: false,
            },
        })
        .collect()
}

/// Edit operation produced by the line diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tag {
    Equal,
    Delete,
    Insert,
}

/// One diff step, carrying the index of the line it came from.
#[derive(Debug, Clone, Copy)]
struct Op {
    tag: Tag,
    old: Option<usize>,
    new: Option<usize>,
}

/// Longest-common-subsequence line diff, with common prefix/suffix trimming and
/// a coarse fallback for pathologically large changed regions.
fn diff_ops(old: &[Line], new: &[Line]) -> Vec<Op> {
    let mut ops = Vec::new();
    let max_pre = old.len().min(new.len());
    let mut pre = 0;
    while pre < max_pre && old[pre] == new[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < max_pre - pre && old[old.len() - 1 - suf] == new[new.len() - 1 - suf] {
        suf += 1;
    }
    for i in 0..pre {
        ops.push(Op {
            tag: Tag::Equal,
            old: Some(i),
            new: Some(i),
        });
    }

    let a = &old[pre..old.len() - suf];
    let b = &new[pre..new.len() - suf];
    if a.len().saturating_mul(b.len()) > LCS_CELL_LIMIT {
        for i in 0..a.len() {
            ops.push(Op {
                tag: Tag::Delete,
                old: Some(pre + i),
                new: None,
            });
        }
        for j in 0..b.len() {
            ops.push(Op {
                tag: Tag::Insert,
                old: None,
                new: Some(pre + j),
            });
        }
    } else {
        // Classic LCS table, then walk it forwards to emit ops in file order.
        let (n, m) = (a.len(), b.len());
        let mut table = vec![0u32; (n + 1) * (m + 1)];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                table[i * (m + 1) + j] = if a[i] == b[j] {
                    table[(i + 1) * (m + 1) + j + 1] + 1
                } else {
                    table[(i + 1) * (m + 1) + j].max(table[i * (m + 1) + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0usize, 0usize);
        while i < n && j < m {
            if a[i] == b[j] {
                ops.push(Op {
                    tag: Tag::Equal,
                    old: Some(pre + i),
                    new: Some(pre + j),
                });
                i += 1;
                j += 1;
            } else if table[(i + 1) * (m + 1) + j] >= table[i * (m + 1) + j + 1] {
                ops.push(Op {
                    tag: Tag::Delete,
                    old: Some(pre + i),
                    new: None,
                });
                i += 1;
            } else {
                ops.push(Op {
                    tag: Tag::Insert,
                    old: None,
                    new: Some(pre + j),
                });
                j += 1;
            }
        }
        while i < n {
            ops.push(Op {
                tag: Tag::Delete,
                old: Some(pre + i),
                new: None,
            });
            i += 1;
        }
        while j < m {
            ops.push(Op {
                tag: Tag::Insert,
                old: None,
                new: Some(pre + j),
            });
            j += 1;
        }
    }

    for k in 0..suf {
        ops.push(Op {
            tag: Tag::Equal,
            old: Some(old.len() - suf + k),
            new: Some(new.len() - suf + k),
        });
    }
    ops
}

/// Render one line of unified-diff body, including the no-trailing-newline marker.
fn push_line(out: &mut String, prefix: char, line: &Line) {
    out.push(prefix);
    out.push_str(&line.text);
    out.push('\n');
    if !line.nl {
        out.push_str("\\ No newline at end of file\n");
    }
}

/// Render a `git apply`-able unified diff for one file. Empty when unchanged.
fn unified_diff(path: &str, old: &[Line], new: &[Line], context: usize) -> String {
    let ops = diff_ops(old, new);
    let changed: Vec<usize> = ops
        .iter()
        .enumerate()
        .filter(|(_, o)| o.tag != Tag::Equal)
        .map(|(i, _)| i)
        .collect();
    if changed.is_empty() {
        return String::new();
    }

    // Expand each change by `context` lines and merge overlapping windows.
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for &c in &changed {
        let start = c.saturating_sub(context);
        let end = (c + context + 1).min(ops.len());
        match ranges.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => ranges.push((start, end)),
        }
    }

    let mut out = format!("--- a/{path}\n+++ b/{path}\n");
    for (start, end) in ranges {
        let before = &ops[..start];
        let old_before = before.iter().filter(|o| o.old.is_some()).count();
        let new_before = before.iter().filter(|o| o.new.is_some()).count();
        let window = &ops[start..end];
        let old_len = window.iter().filter(|o| o.old.is_some()).count();
        let new_len = window.iter().filter(|o| o.new.is_some()).count();
        let old_start = if old_len == 0 { old_before } else { old_before + 1 };
        let new_start = if new_len == 0 { new_before } else { new_before + 1 };
        out.push_str(&format!(
            "@@ -{old_start},{old_len} +{new_start},{new_len} @@\n"
        ));
        for op in window {
            match op.tag {
                Tag::Equal => push_line(&mut out, ' ', &old[op.old.unwrap()]),
                Tag::Delete => push_line(&mut out, '-', &old[op.old.unwrap()]),
                Tag::Insert => push_line(&mut out, '+', &new[op.new.unwrap()]),
            }
        }
    }
    out
}

/// `1 file` / `3 files`.
fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("{n} {word}")
    } else {
        format!("{n} {word}s")
    }
}

/// Apply a regex/literal find-and-replace across a pasted multi-file bundle.
///
/// * `text` — the bundle.
/// * `pattern` — the regular expression (or literal text when `opts.literal`).
/// * `replacement` — replacement text; `$1`/`${name}` insert capture groups
///   unless `opts.literal` is set, in which case it is inserted verbatim.
/// * `file_marker` — `auto|equals|dashes|arrow|comment|custom|none`.
/// * `marker_regex` — the custom marker pattern (only with `file_marker=custom`).
/// * `output` — `diff|full|json`.
pub fn run(
    text: &str,
    pattern: &str,
    replacement: &str,
    file_marker: &str,
    marker_regex: &str,
    output: &str,
    opts: &Options,
) -> Result<String, String> {
    if !matches!(output, "diff" | "full" | "json") {
        return Err(format!(
            "unknown output {output:?} — expected diff, full or json"
        ));
    }
    if pattern.is_empty() {
        return Err("`pattern` must not be empty".into());
    }
    let chars = text.chars().count();
    if chars > MAX_INPUT_CHARS {
        return Err(format!(
            "input is {chars} characters; the limit is {MAX_INPUT_CHARS} — split the paste into smaller batches"
        ));
    }
    if opts.context > 100 {
        return Err("`context` must be between 0 and 100".into());
    }

    let mut source = if opts.literal {
        regex::escape(pattern)
    } else {
        pattern.to_string()
    };
    if opts.whole_word {
        source = format!(r"\b(?:{source})\b");
    }
    let re = RegexBuilder::new(&source)
        .case_insensitive(opts.ignore_case)
        .multi_line(opts.multiline)
        .dot_matches_new_line(opts.dot_matches_newline)
        .build()
        .map_err(|e| format!("invalid regular expression: {e}"))?;

    let markers = marker_patterns(file_marker, marker_regex)?;
    let files = split_files(text, &markers)?;

    let mut results: Vec<FileResult> = Vec::with_capacity(files.len());
    for f in &files {
        let count = if opts.replace_all {
            re.find_iter(&f.body).count()
        } else {
            usize::from(re.find(&f.body).is_some())
        };
        let new_body = match (opts.literal, opts.replace_all) {
            (false, true) => re.replace_all(&f.body, replacement).into_owned(),
            (false, false) => re.replace(&f.body, replacement).into_owned(),
            (true, true) => re
                .replace_all(&f.body, NoExpand(replacement))
                .into_owned(),
            (true, false) => re.replace(&f.body, NoExpand(replacement)).into_owned(),
        };
        let diff = if new_body == f.body {
            String::new()
        } else {
            unified_diff(&f.path, &to_lines(&f.body), &to_lines(&new_body), opts.context)
        };
        results.push(FileResult {
            path: f.path.clone(),
            replacements: count,
            new_body,
            diff,
        });
    }

    let total: usize = results.iter().map(|r| r.replacements).sum();
    let changed = results.iter().filter(|r| !r.diff.is_empty()).count();

    match output {
        "full" => {
            let mut out = String::new();
            for (f, r) in files.iter().zip(&results) {
                out.push_str(&f.marker);
                out.push_str(&r.new_body);
            }
            Ok(out)
        }
        "json" => {
            let entries: Vec<_> = results
                .iter()
                .filter(|r| opts.include_unchanged || !r.diff.is_empty())
                .map(|r| {
                    json!({
                        "path": r.path,
                        "replacements": r.replacements,
                        "changed": !r.diff.is_empty(),
                        "diff": r.diff,
                    })
                })
                .collect();
            let report = json!({
                "replacements": total,
                "files_total": results.len(),
                "files_changed": changed,
                "files": entries,
            });
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())
        }
        _ => {
            let mut out = format!(
                "# {} in {} of {}\n",
                plural(total, "replacement"),
                changed,
                plural(results.len(), "file")
            );
            if opts.include_unchanged {
                for r in results.iter().filter(|r| r.diff.is_empty()) {
                    out.push_str(&format!("# unchanged: {}\n", r.path));
                }
            }
            for r in results.iter().filter(|r| !r.diff.is_empty()) {
                out.push_str(&r.diff);
            }
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options {
            replace_all: true,
            context: 3,
            ..Options::default()
        }
    }

    const BUNDLE: &str = "=== src/a.js ===\nconst oldName = 1;\nuse(oldName);\n=== src/b.js ===\nconst other = 2;\n";

    #[test]
    fn diff_across_two_files_renames_only_the_matching_one() {
        let out = run(BUNDLE, r"\boldName\b", "newName", "auto", "", "diff", &opts()).unwrap();
        assert_eq!(
            out,
            "# 2 replacements in 1 of 2 files\n\
             --- a/src/a.js\n\
             +++ b/src/a.js\n\
             @@ -1,2 +1,2 @@\n\
             -const oldName = 1;\n\
             -use(oldName);\n\
             +const newName = 1;\n\
             +use(newName);\n"
        );
    }

    #[test]
    fn capture_groups_reorder_a_date() {
        let out = run(
            "==> dates.txt <==\n12/31/2026\n",
            r"(\d{2})/(\d{2})/(\d{4})",
            "$3-$1-$2",
            "auto",
            "",
            "full",
            &opts(),
        )
        .unwrap();
        assert_eq!(out, "==> dates.txt <==\n2026-12-31\n");
    }

    #[test]
    fn named_capture_groups_expand() {
        let out = run(
            "a.txt\nkey=value\n",
            r"(?P<k>\w+)=(?P<v>\w+)",
            "${v}:${k}",
            "none",
            "",
            "full",
            &opts(),
        )
        .unwrap();
        assert_eq!(out, "a.txt\nvalue:key\n");
    }

    #[test]
    fn literal_mode_escapes_pattern_and_replacement() {
        let out = run(
            "a.b\n",
            ".",
            "$1[]",
            "none",
            "",
            "full",
            &Options {
                literal: true,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, "a$1[]b\n");
    }

    #[test]
    fn first_only_stops_after_one_match_per_file() {
        let out = run(
            BUNDLE,
            "oldName",
            "newName",
            "auto",
            "",
            "full",
            &Options {
                replace_all: false,
                ..opts()
            },
        )
        .unwrap();
        assert!(out.contains("const newName = 1;\nuse(oldName);"));
    }

    #[test]
    fn whole_word_does_not_match_inside_a_longer_identifier() {
        let out = run(
            "a.txt\nid idx\n",
            "id",
            "ID",
            "none",
            "",
            "full",
            &Options {
                whole_word: true,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, "a.txt\nID idx\n");
    }

    #[test]
    fn ignore_case_matches_mixed_case() {
        let out = run(
            "Foo FOO foo\n",
            "foo",
            "bar",
            "none",
            "",
            "full",
            &Options {
                ignore_case: true,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, "bar bar bar\n");
    }

    #[test]
    fn multiline_anchors_match_each_line() {
        let out = run(
            "one\ntwo\n",
            r"^(\w+)$",
            "> $1",
            "none",
            "",
            "full",
            &Options {
                multiline: true,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, "> one\n> two\n");
    }

    #[test]
    fn dot_matches_newline_spans_lines() {
        let out = run(
            "<a>\nx\n</a>\n",
            "<a>.*</a>",
            "gone",
            "none",
            "",
            "full",
            &Options {
                dot_matches_newline: true,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, "gone\n");
    }

    #[test]
    fn json_report_counts_files_and_replacements() {
        let out = run(BUNDLE, "oldName", "newName", "auto", "", "json", &opts()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["replacements"], 2);
        assert_eq!(v["files_total"], 2);
        assert_eq!(v["files_changed"], 1);
        assert_eq!(v["files"].as_array().unwrap().len(), 1);
        assert_eq!(v["files"][0]["path"], "src/a.js");
    }

    #[test]
    fn include_unchanged_lists_untouched_files() {
        let out = run(
            BUNDLE,
            "oldName",
            "newName",
            "auto",
            "",
            "diff",
            &Options {
                include_unchanged: true,
                ..opts()
            },
        )
        .unwrap();
        assert!(out.contains("# unchanged: src/b.js\n"));
    }

    #[test]
    fn dashes_and_comment_markers_are_detected() {
        let out = run(
            "--- one.txt ---\nx\n// file: two.txt\nx\n",
            "x",
            "y",
            "auto",
            "",
            "json",
            &opts(),
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["files_total"], 2);
        assert_eq!(v["files"][0]["path"], "one.txt");
        assert_eq!(v["files"][1]["path"], "two.txt");
    }

    #[test]
    fn custom_marker_regex_splits_on_its_capture_group() {
        let out = run(
            "@@@ a.rs\nx\n@@@ b.rs\nx\n",
            "x",
            "y",
            "custom",
            r"^@@@ (\S+)$",
            "json",
            &opts(),
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["files_total"], 2);
        assert_eq!(v["files"][1]["path"], "b.rs");
    }

    #[test]
    fn content_before_the_first_marker_becomes_the_unnamed_file() {
        let out = run("loose x\n=== a.txt ===\nx\n", "x", "y", "auto", "", "json", &opts()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["files_total"], 2);
        assert_eq!(v["files"][0]["path"], UNNAMED_PATH);
    }

    #[test]
    fn missing_trailing_newline_is_marked_in_the_diff() {
        let out = run("a.txt", "a", "b", "none", "", "diff", &opts()).unwrap();
        assert!(out.contains("\\ No newline at end of file\n"));
    }

    #[test]
    fn context_zero_emits_only_changed_lines() {
        let out = run(
            "keep\nold\nkeep2\n",
            "old",
            "new",
            "none",
            "",
            "diff",
            &Options {
                context: 0,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(
            out,
            "# 1 replacement in 1 of 1 file\n--- a/input\n+++ b/input\n@@ -2,1 +2,1 @@\n-old\n+new\n"
        );
    }

    #[test]
    fn deleting_a_line_shrinks_the_new_side() {
        let out = run("a\ndrop\nb\n", "(?m)^drop\n", "", "none", "", "diff", &opts()).unwrap();
        assert_eq!(
            out,
            "# 1 replacement in 1 of 1 file\n\
             --- a/input\n+++ b/input\n@@ -1,3 +1,2 @@\n a\n-drop\n b\n"
        );
    }

    #[test]
    fn no_match_reports_zero_replacements() {
        let out = run(BUNDLE, "zzz", "y", "auto", "", "diff", &opts()).unwrap();
        assert_eq!(out, "# 0 replacements in 0 of 2 files\n");
    }

    // --- error paths ---

    #[test]
    fn invalid_regex_is_an_error() {
        let err = run("x\n", "(unclosed", "y", "none", "", "diff", &opts()).unwrap_err();
        assert!(err.starts_with("invalid regular expression:"), "{err}");
    }

    #[test]
    fn empty_pattern_is_rejected() {
        let err = run("x\n", "", "y", "none", "", "diff", &opts()).unwrap_err();
        assert_eq!(err, "`pattern` must not be empty");
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = "x".repeat(MAX_INPUT_CHARS + 1);
        let err = run(&big, "x", "y", "none", "", "diff", &opts()).unwrap_err();
        assert!(err.contains("the limit is 1000000"), "{err}");
    }

    #[test]
    fn cap_boundary_is_accepted() {
        let at_cap = "x".repeat(MAX_INPUT_CHARS);
        assert!(run(&at_cap, "zzz", "y", "none", "", "json", &opts()).is_ok());
    }

    #[test]
    fn marker_regex_without_custom_is_rejected() {
        let err = run("x\n", "x", "y", "auto", r"^(\S+)$", "diff", &opts()).unwrap_err();
        assert!(err.contains("only used when"), "{err}");
    }

    #[test]
    fn custom_without_marker_regex_is_rejected() {
        let err = run("x\n", "x", "y", "custom", "", "diff", &opts()).unwrap_err();
        assert!(err.contains("`marker_regex` is empty"), "{err}");
    }

    #[test]
    fn custom_marker_regex_needs_a_capture_group() {
        let err = run("x\n", "x", "y", "custom", "^@@@", "diff", &opts()).unwrap_err();
        assert!(err.contains("one capture group"), "{err}");
    }

    #[test]
    fn unknown_output_is_rejected() {
        let err = run("x\n", "x", "y", "none", "", "html", &opts()).unwrap_err();
        assert!(err.contains("unknown output"), "{err}");
    }

    #[test]
    fn unknown_file_marker_is_rejected() {
        let err = run("x\n", "x", "y", "chunks", "", "diff", &opts()).unwrap_err();
        assert!(err.contains("unknown file_marker"), "{err}");
    }
}
