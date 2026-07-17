//! gizza-ai/regex-search core — a grep-style line search over pasted text.
//!
//! Returns the whole **lines** that match a regular expression (or a literal
//! fixed string), grep-style, with 1-based line numbers, optional surrounding
//! context lines, invert/whole-word/ignore-case modifiers, and optional match
//! highlighting. Pure-Rust (`regex`); shared by the chat skill block, the CLI,
//! and the web page.
//!
//! This is distinct from `regex-extract` (which returns the matched *substrings*
//! or a chosen capture group) and `regex-tester` (which returns each match's
//! character span and every capture group for debugging a pattern). `regex-search`
//! is line-oriented: it is the browser equivalent of `grep -n -C`.

use regex::RegexBuilder;
use serde::Serialize;

/// The markers wrapped around each matched substring when `highlight` is on.
/// Chosen to be visually distinct and rare in real text, and text-safe (the page
/// renders plain text, so HTML colour spans are not an option here).
const HL_OPEN: char = '«';
const HL_CLOSE: char = '»';

/// One output line: either a matching ("hit") line or a context line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Line {
    /// 1-based line number in the original text.
    pub line_number: usize,
    /// The line's text (with `«…»` markers around matches when highlighting is on;
    /// context lines are never highlighted).
    pub text: String,
    /// `true` for a selected/hit line (grep's `:` prefix), `false` for a context
    /// line pulled in by the `context` window (grep's `-` prefix).
    pub is_match: bool,
}

/// The structured result returned to the chat/LLM and CLI surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchResult {
    /// Number of selected/hit lines (grep's `-c` count).
    pub match_count: usize,
    /// The output lines — hits plus any context — in original order.
    pub lines: Vec<Line>,
}

/// Build the effective regex from the raw pattern + modifier flags.
fn build_regex(
    pattern: &str,
    literal: bool,
    ignore_case: bool,
    whole_word: bool,
) -> Result<regex::Regex, String> {
    if pattern.is_empty() {
        return Err("pattern must not be empty".to_string());
    }
    // Literal/fixed-string mode: escape every regex metacharacter so the pattern
    // matches verbatim (grep -F).
    let mut effective = if literal {
        regex::escape(pattern)
    } else {
        pattern.to_string()
    };
    // Whole-word mode: wrap in word boundaries (grep -w). The inner pattern is
    // grouped with a non-capturing group so alternations stay intact.
    if whole_word {
        effective = format!(r"\b(?:{effective})\b");
    }
    RegexBuilder::new(&effective)
        .case_insensitive(ignore_case)
        .build()
        .map_err(|e| format!("invalid regular expression: {e}"))
}

/// Wrap every non-overlapping match of `re` in `line` with the highlight markers.
fn highlight_line(re: &regex::Regex, line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut last = 0;
    for m in re.find_iter(line) {
        // Skip zero-width matches so we don't emit empty `«»` pairs everywhere.
        if m.start() == m.end() {
            continue;
        }
        out.push_str(&line[last..m.start()]);
        out.push(HL_OPEN);
        out.push_str(&line[m.start()..m.end()]);
        out.push(HL_CLOSE);
        last = m.end();
    }
    out.push_str(&line[last..]);
    out
}

/// Search `text` line by line and return the selected lines (plus context).
///
/// - `literal`: treat `pattern` as a fixed string, escaping regex metacharacters.
/// - `ignore_case`: case-insensitive matching (grep -i).
/// - `whole_word`: only match when the pattern is a whole word (grep -w).
/// - `invert`: select the lines that do NOT match instead (grep -v).
/// - `context`: also include this many lines before AND after each hit (grep -C).
/// - `highlight`: wrap matched substrings on hit lines in `«…»` markers.
#[allow(clippy::too_many_arguments)]
pub fn search(
    text: &str,
    pattern: &str,
    literal: bool,
    ignore_case: bool,
    whole_word: bool,
    invert: bool,
    context: usize,
    highlight: bool,
) -> Result<SearchResult, String> {
    let re = build_regex(pattern, literal, ignore_case, whole_word)?;

    // `str::lines()` splits on \n and \r\n and drops the trailing terminator, so a
    // final newline does not produce a spurious empty line (grep behaves the same).
    let lines: Vec<&str> = text.lines().collect();

    // Which line indices are "selected" (hits). invert flips the selection.
    let selected: Vec<bool> = lines.iter().map(|l| re.is_match(l) != invert).collect();
    let match_count = selected.iter().filter(|&&s| s).count();

    // Expand each hit into a [i-context, i+context] window, then union them so
    // overlapping/adjacent windows share their context lines (like grep).
    let mut include = vec![false; lines.len()];
    for (i, &hit) in selected.iter().enumerate() {
        if !hit {
            continue;
        }
        let start = i.saturating_sub(context);
        let end = (i + context).min(lines.len().saturating_sub(1));
        for slot in include.iter_mut().take(end + 1).skip(start) {
            *slot = true;
        }
    }

    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if !include[i] {
            continue;
        }
        let is_match = selected[i];
        // Highlight only genuine matching lines (not inverted, not context).
        let text = if highlight && is_match && !invert {
            highlight_line(&re, line)
        } else {
            (*line).to_string()
        };
        out.push(Line {
            line_number: i + 1,
            text,
            is_match,
        });
    }

    Ok(SearchResult {
        match_count,
        lines: out,
    })
}

/// Human-readable grep-style rendering for the page + CLI text surface.
///
/// `line_numbers` prefixes each line with its number (`n:` for hits, `n-` for
/// context). When `context > 0`, non-adjacent groups are separated by a `--`
/// line, exactly as grep prints them.
#[allow(clippy::too_many_arguments)]
pub fn render(
    text: &str,
    pattern: &str,
    literal: bool,
    ignore_case: bool,
    whole_word: bool,
    invert: bool,
    context: usize,
    line_numbers: bool,
    highlight: bool,
) -> Result<String, String> {
    let r = search(
        text,
        pattern,
        literal,
        ignore_case,
        whole_word,
        invert,
        context,
        highlight,
    )?;

    if r.match_count == 0 {
        return Ok("No matching lines found.".to_string());
    }

    let header = format!(
        "{} matching line{}:",
        r.match_count,
        if r.match_count == 1 { "" } else { "s" }
    );
    let mut out = String::from(&header);
    out.push('\n');

    let mut prev_num: Option<usize> = None;
    for line in &r.lines {
        // Emit a `--` group separator between non-adjacent context groups.
        if context > 0 {
            if let Some(pn) = prev_num {
                if line.line_number > pn + 1 {
                    out.push_str("--\n");
                }
            }
        }
        if line_numbers {
            let sep = if line.is_match { ':' } else { '-' };
            out.push_str(&format!("{}{}{}\n", line.line_number, sep, line.text));
        } else {
            out.push_str(&line.text);
            out.push('\n');
        }
        prev_num = Some(line.line_number);
    }

    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_matching_lines_with_numbers() {
        let r =
            search("apple\nbanana\navocado", "^a", false, false, false, false, 0, false).unwrap();
        assert_eq!(r.match_count, 2);
        assert_eq!(r.lines.len(), 2);
        assert_eq!(r.lines[0].line_number, 1);
        assert_eq!(r.lines[0].text, "apple");
        assert!(r.lines[0].is_match);
        assert_eq!(r.lines[1].line_number, 3);
        assert_eq!(r.lines[1].text, "avocado");
    }

    #[test]
    fn ignore_case_matches_any_case() {
        let r = search("Cat\ncatalog\nDOG", "cat", false, true, false, false, 0, false).unwrap();
        assert_eq!(r.match_count, 2);
    }

    #[test]
    fn literal_mode_escapes_metacharacters() {
        // `a.c` as a literal only matches the exact "a.c", not "abc".
        let r = search("a.c\nabc\naXc", "a.c", true, false, false, false, 0, false).unwrap();
        assert_eq!(r.match_count, 1);
        assert_eq!(r.lines[0].text, "a.c");
    }

    #[test]
    fn whole_word_requires_boundaries() {
        let r = search("cat\ncategory\nthe cat sat", "cat", false, false, true, false, 0, false)
            .unwrap();
        // "category" is excluded; "cat" and "the cat sat" match.
        assert_eq!(r.match_count, 2);
        assert_eq!(r.lines[0].text, "cat");
        assert_eq!(r.lines[1].text, "the cat sat");
    }

    #[test]
    fn invert_selects_non_matching_lines() {
        let r =
            search("apple\nbanana\navocado", "^a", false, false, false, true, 0, false).unwrap();
        assert_eq!(r.match_count, 1);
        assert_eq!(r.lines[0].text, "banana");
        assert!(r.lines[0].is_match); // "selected" in the grep sense
    }

    #[test]
    fn context_includes_surrounding_lines_and_merges() {
        let r = search(
            "one\ntwo\nthree\nfour\nfive",
            "three",
            false,
            false,
            false,
            false,
            1,
            false,
        )
        .unwrap();
        assert_eq!(r.match_count, 1);
        // three ± 1 context line = two, three, four
        let nums: Vec<usize> = r.lines.iter().map(|l| l.line_number).collect();
        assert_eq!(nums, vec![2, 3, 4]);
        assert!(!r.lines[0].is_match); // context
        assert!(r.lines[1].is_match); // hit
    }

    #[test]
    fn highlight_wraps_matches() {
        let r = search("the cat and the cat", "cat", false, false, false, false, 0, true).unwrap();
        assert_eq!(r.lines[0].text, "the «cat» and the «cat»");
    }

    #[test]
    fn no_match_is_empty() {
        let r = search("abc\ndef", r"\d", false, false, false, false, 0, false).unwrap();
        assert_eq!(r.match_count, 0);
        assert!(r.lines.is_empty());
    }

    #[test]
    fn empty_pattern_errors() {
        assert!(search("x", "", false, false, false, false, 0, false).is_err());
    }

    #[test]
    fn invalid_pattern_errors() {
        let e = search("x", "(", false, false, false, false, 0, false).unwrap_err();
        assert!(e.contains("invalid regular expression"));
    }

    #[test]
    fn render_grep_style_with_line_numbers() {
        let out = render(
            "one\ntwo\nthree\nfour\nfive",
            "three",
            false,
            false,
            false,
            false,
            1,
            true,
            false,
        )
        .unwrap();
        assert_eq!(out, "1 matching line:\n2-two\n3:three\n4-four");
    }

    #[test]
    fn render_group_separators() {
        let out = render(
            "a\nX\nb\nc\nd\nX\ne",
            "X",
            false,
            false,
            false,
            false,
            1,
            true,
            false,
        )
        .unwrap();
        // Two hits (lines 2 and 6), non-adjacent → a `--` between the groups.
        assert_eq!(out, "2 matching lines:\n1-a\n2:X\n3-b\n--\n5-d\n6:X\n7-e");
    }

    #[test]
    fn render_no_line_numbers() {
        let out = render(
            "apple\nbanana\navocado",
            "^a",
            false,
            false,
            false,
            false,
            0,
            false,
            false,
        )
        .unwrap();
        assert_eq!(out, "2 matching lines:\napple\navocado");
    }

    #[test]
    fn render_no_match_message() {
        let out = render("abc", r"\d", false, false, false, false, 0, true, false).unwrap();
        assert_eq!(out, "No matching lines found.");
    }
}
