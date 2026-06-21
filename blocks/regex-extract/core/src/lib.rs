//! gizza-ai/regex-extract core — return every match of a regular expression
//! found in the input text. Pure-Rust (`regex`). Supports case-insensitive,
//! multiline (`^`/`$` per line), and dot-matches-newline modes, extracting
//! either the whole match or a chosen capture group, with optional dedup.

use regex::RegexBuilder;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Found {
    /// Number of matches returned (after optional dedup).
    pub count: usize,
    /// The matched strings (the whole match, or the chosen capture group).
    pub matches: Vec<String>,
}

/// Extract every match of `pattern` in `text`.
///
/// - `ignore_case`: case-insensitive matching.
/// - `multiline`: `^` and `$` match at line boundaries, not just string ends.
/// - `dotall`: `.` also matches newline characters.
/// - `capture_group`: which capture group to return per match (0 = the whole
///   match). A match where the requested group did not participate is skipped.
/// - `unique`: deduplicate the returned matches, keeping first-seen order.
#[allow(clippy::too_many_arguments)]
pub fn extract(
    text: &str,
    pattern: &str,
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
    capture_group: usize,
    unique: bool,
) -> Result<Found, String> {
    if pattern.is_empty() {
        return Err("pattern must not be empty".to_string());
    }
    let re = RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .multi_line(multiline)
        .dot_matches_new_line(dotall)
        .build()
        .map_err(|e| format!("invalid regular expression: {e}"))?;

    if capture_group != 0 && capture_group > re.captures_len().saturating_sub(1) {
        return Err(format!(
            "capture_group {capture_group} is out of range; the pattern has {} capture group(s)",
            re.captures_len().saturating_sub(1)
        ));
    }

    let mut matches: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for caps in re.captures_iter(text) {
        let m = match caps.get(capture_group) {
            Some(m) => m.as_str().to_string(),
            None => continue, // optional group that did not participate
        };
        if unique && !seen.insert(m.clone()) {
            continue;
        }
        matches.push(m);
    }

    Ok(Found {
        count: matches.len(),
        matches,
    })
}

/// Human-readable rendering for the page + CLI text surface.
#[allow(clippy::too_many_arguments)]
pub fn render(
    text: &str,
    pattern: &str,
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
    capture_group: usize,
    unique: bool,
) -> Result<String, String> {
    let r = extract(
        text,
        pattern,
        ignore_case,
        multiline,
        dotall,
        capture_group,
        unique,
    )?;
    if r.count == 0 {
        return Ok("No matches found.".to_string());
    }
    let label = if unique { "unique match(es)" } else { "match(es)" };
    let mut out = format!("{} {label}:\n", r.count);
    for m in &r.matches {
        out.push_str(m);
        out.push('\n');
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_all_matches() {
        let r = extract("a1 b2 c3", r"\d", false, false, false, 0, false).unwrap();
        assert_eq!(r.count, 3);
        assert_eq!(r.matches, vec!["1", "2", "3"]);
    }

    #[test]
    fn case_insensitive() {
        let r = extract("Cat cat CAT", r"cat", true, false, false, 0, false).unwrap();
        assert_eq!(r.count, 3);
    }

    #[test]
    fn capture_group_extracts_subgroup() {
        let r = extract(
            "key=val foo=bar",
            r"(\w+)=(\w+)",
            false,
            false,
            false,
            2,
            false,
        )
        .unwrap();
        assert_eq!(r.matches, vec!["val", "bar"]);
    }

    #[test]
    fn unique_dedupes_first_seen() {
        let r = extract("a a b a c", r"\w", false, false, false, 0, true).unwrap();
        assert_eq!(r.matches, vec!["a", "b", "c"]);
    }

    #[test]
    fn multiline_anchors() {
        let r = extract("foo\nbar\nbaz", r"^b\w+", false, true, false, 0, false).unwrap();
        assert_eq!(r.matches, vec!["bar", "baz"]);
    }

    #[test]
    fn dotall_spans_newlines() {
        let r = extract("a\nb", r"a.b", false, false, true, 0, false).unwrap();
        assert_eq!(r.count, 1);
    }

    #[test]
    fn empty_pattern_errors() {
        assert!(extract("x", "", false, false, false, 0, false).is_err());
    }

    #[test]
    fn invalid_pattern_errors() {
        let e = extract("x", r"(", false, false, false, 0, false).unwrap_err();
        assert!(e.contains("invalid regular expression"));
    }

    #[test]
    fn out_of_range_group_errors() {
        let e = extract("x", r"(x)", false, false, false, 5, false).unwrap_err();
        assert!(e.contains("out of range"));
    }

    #[test]
    fn no_match_count_zero() {
        let r = extract("abc", r"\d", false, false, false, 0, false).unwrap();
        assert_eq!(r.count, 0);
        assert!(r.matches.is_empty());
    }

    #[test]
    fn render_modes() {
        assert!(render("a1 b2", r"\d", false, false, false, 0, false)
            .unwrap()
            .contains("2 match(es)"));
        assert!(render("a a b", r"\w", false, false, false, 0, true)
            .unwrap()
            .contains("unique match(es)"));
        assert!(render("abc", r"\d", false, false, false, 0, false)
            .unwrap()
            .contains("No matches"));
    }
}
