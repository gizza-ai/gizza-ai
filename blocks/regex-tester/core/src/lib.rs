//! gizza-ai/regex-tester core — test a regular expression against input text and
//! report, for every match, its character span plus the value and span of every
//! numbered and named capture group (regex101-style). Pure-Rust (`regex`),
//! shared by the chat skill block, the CLI, and the web page.
//!
//! This is distinct from `regex-extract` (which returns a flat list of one chosen
//! group's matches): the tester surfaces the full structured match breakdown —
//! positions and ALL capture groups, named and numbered — for debugging a pattern.

use regex::RegexBuilder;
use serde::Serialize;

/// One capture group within a single match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Group {
    /// 1-based group number (group 0, the whole match, is reported on `Match`).
    pub number: usize,
    /// The group's name, if it was written as `(?<name>…)`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Whether this (possibly optional) group participated in the match.
    pub matched: bool,
    /// The captured text (absent when the group did not participate).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Character offset (0-based) of the group's start, if it matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<usize>,
    /// Character offset of the group's end (exclusive), if it matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<usize>,
}

/// A single whole-pattern match and its capture groups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Match {
    /// 1-based match number in document order.
    pub index: usize,
    /// The whole matched text (capture group 0).
    pub value: String,
    /// Character offset (0-based) of the match start.
    pub start: usize,
    /// Character offset of the match end (exclusive).
    pub end: usize,
    /// Capture groups 1..N (numbered and named).
    pub groups: Vec<Group>,
}

/// The structured result returned to the chat/LLM and CLI surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// Number of matches found.
    pub match_count: usize,
    /// Number of capture groups declared in the pattern.
    pub group_count: usize,
    /// Names of any named capture groups, in declaration order.
    pub group_names: Vec<String>,
    /// Every match, in document order.
    pub matches: Vec<Match>,
}

/// Convert a byte offset (always a char boundary for regex spans) to a 0-based
/// character offset, so positions line up with how a human reads the string
/// rather than its UTF-8 byte layout.
fn char_pos(text: &str, byte: usize) -> usize {
    text[..byte].chars().count()
}

/// Test `pattern` against `text`, returning a structured breakdown of every match.
///
/// - `ignore_case`: case-insensitive matching (the `i` flag).
/// - `multiline`: `^` and `$` match at line boundaries, not just string ends (`m`).
/// - `dotall`: `.` also matches newline characters (`s`).
///
/// Returns an error for an empty or syntactically invalid pattern.
pub fn test(
    text: &str,
    pattern: &str,
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
) -> Result<Report, String> {
    if pattern.is_empty() {
        return Err("pattern must not be empty".to_string());
    }
    let re = RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .multi_line(multiline)
        .dot_matches_new_line(dotall)
        .build()
        .map_err(|e| format!("invalid regular expression: {e}"))?;

    let names: Vec<Option<String>> = re
        .capture_names()
        .map(|n| n.map(|s| s.to_string()))
        .collect();
    let group_count = re.captures_len().saturating_sub(1);
    let group_names: Vec<String> = names.iter().skip(1).flatten().cloned().collect();

    let mut matches = Vec::new();
    for (i, caps) in re.captures_iter(text).enumerate() {
        let whole = caps.get(0).expect("group 0 always present");
        let mut groups = Vec::with_capacity(group_count);
        for g in 1..caps.len() {
            let name = names.get(g).cloned().flatten();
            match caps.get(g) {
                Some(m) => groups.push(Group {
                    number: g,
                    name,
                    matched: true,
                    value: Some(m.as_str().to_string()),
                    start: Some(char_pos(text, m.start())),
                    end: Some(char_pos(text, m.end())),
                }),
                None => groups.push(Group {
                    number: g,
                    name,
                    matched: false,
                    value: None,
                    start: None,
                    end: None,
                }),
            }
        }
        matches.push(Match {
            index: i + 1,
            value: whole.as_str().to_string(),
            start: char_pos(text, whole.start()),
            end: char_pos(text, whole.end()),
            groups,
        });
    }

    Ok(Report {
        match_count: matches.len(),
        group_count,
        group_names,
        matches,
    })
}

/// Human-readable rendering for the page + CLI text surface.
pub fn render(
    text: &str,
    pattern: &str,
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
) -> Result<String, String> {
    let r = test(text, pattern, ignore_case, multiline, dotall)?;

    let m_word = if r.match_count == 1 { "match" } else { "matches" };
    let g_word = if r.group_count == 1 { "group" } else { "groups" };
    let mut out = format!(
        "\u{2713} Valid pattern \u{2014} {} {m_word}, {} capture {g_word}",
        r.match_count, r.group_count
    );
    if !r.group_names.is_empty() {
        out.push_str(&format!(" (named: {})", r.group_names.join(", ")));
    }
    out.push('\n');

    if r.match_count == 0 {
        out.push_str("\nNo matches found.");
        return Ok(out.trim_end().to_string());
    }

    for m in &r.matches {
        out.push_str(&format!(
            "\nMatch {} at {}\u{2013}{}: {:?}\n",
            m.index, m.start, m.end, m.value
        ));
        for g in &m.groups {
            let label = match &g.name {
                Some(n) => format!("Group {} ({n})", g.number),
                None => format!("Group {}", g.number),
            };
            match &g.value {
                Some(v) => out.push_str(&format!(
                    "  {label} at {}\u{2013}{}: {:?}\n",
                    g.start.unwrap(),
                    g.end.unwrap(),
                    v
                )),
                None => out.push_str(&format!("  {label}: (no match)\n")),
            }
        }
    }

    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_matches_with_positions() {
        let r = test("a1 b2 c3", r"\d", false, false, false).unwrap();
        assert_eq!(r.match_count, 3);
        assert_eq!(r.group_count, 0);
        assert_eq!(r.matches[0].value, "1");
        assert_eq!(r.matches[0].start, 1);
        assert_eq!(r.matches[0].end, 2);
        assert_eq!(r.matches[2].start, 7);
    }

    #[test]
    fn numbered_and_named_groups() {
        let r = test(
            "2024-01-15",
            r"(?<year>\d{4})-(\d{2})-(?<day>\d{2})",
            false,
            false,
            false,
        )
        .unwrap();
        assert_eq!(r.match_count, 1);
        assert_eq!(r.group_count, 3);
        assert_eq!(r.group_names, vec!["year", "day"]);
        let m = &r.matches[0];
        assert_eq!(m.value, "2024-01-15");
        assert_eq!(m.groups.len(), 3);
        assert_eq!(m.groups[0].number, 1);
        assert_eq!(m.groups[0].name.as_deref(), Some("year"));
        assert_eq!(m.groups[0].value.as_deref(), Some("2024"));
        assert_eq!(m.groups[0].start, Some(0));
        assert_eq!(m.groups[0].end, Some(4));
        assert_eq!(m.groups[1].name, None);
        assert_eq!(m.groups[1].value.as_deref(), Some("01"));
        assert_eq!(m.groups[2].name.as_deref(), Some("day"));
    }

    #[test]
    fn optional_group_that_did_not_participate() {
        let r = test("abc", r"a(x)?(b)", false, false, false).unwrap();
        let m = &r.matches[0];
        assert_eq!(m.groups[0].number, 1);
        assert!(!m.groups[0].matched);
        assert_eq!(m.groups[0].value, None);
        assert!(m.groups[1].matched);
        assert_eq!(m.groups[1].value.as_deref(), Some("b"));
    }

    #[test]
    fn ignore_case_and_multiline_flags() {
        assert_eq!(
            test("Cat cat CAT", "cat", true, false, false)
                .unwrap()
                .match_count,
            3
        );
        // ^ only matches line starts when multiline is on.
        assert_eq!(
            test("a\nb\nc", "^.", false, true, false)
                .unwrap()
                .match_count,
            3
        );
        assert_eq!(
            test("a\nb\nc", "^.", false, false, false)
                .unwrap()
                .match_count,
            1
        );
    }

    #[test]
    fn dotall_lets_dot_span_newlines() {
        assert!(test("a\nb", "a.b", false, false, false)
            .unwrap()
            .matches
            .is_empty());
        assert_eq!(
            test("a\nb", "a.b", false, false, true).unwrap().match_count,
            1
        );
    }

    #[test]
    fn char_positions_are_unicode_aware() {
        // "é" is 2 bytes but 1 char; the digit after "café " is char index 5.
        let r = test("café 7", r"\d", false, false, false).unwrap();
        assert_eq!(r.matches[0].start, 5);
    }

    #[test]
    fn empty_pattern_is_rejected() {
        assert!(test("abc", "", false, false, false).is_err());
    }

    #[test]
    fn invalid_pattern_is_rejected() {
        let e = test("abc", "a(", false, false, false).unwrap_err();
        assert!(e.contains("invalid regular expression"));
    }

    #[test]
    fn render_no_match() {
        let s = render("abc", r"\d", false, false, false).unwrap();
        assert!(s.contains("0 matches"));
        assert!(s.contains("No matches found."));
    }

    #[test]
    fn render_lists_matches_and_groups() {
        let s = render("2024-01", r"(?<year>\d{4})-(\d{2})", false, false, false).unwrap();
        assert!(s.contains("1 match"));
        assert!(s.contains("named: year"));
        assert!(s.contains("Match 1 at 0\u{2013}7"));
        assert!(s.contains("Group 1 (year)"));
        assert!(s.contains("Group 2 at"));
    }
}
