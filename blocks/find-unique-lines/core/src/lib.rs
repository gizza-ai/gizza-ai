//! gizza-ai/find-unique-lines core — return only the lines that appear exactly
//! once across the input (the `uniq -u` behaviour). Pure-Rust, dependency-free
//! (besides serde).

use std::collections::HashMap;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    /// Total number of input lines scanned.
    pub total_lines: usize,
    /// Number of distinct lines (after normalization).
    pub distinct_lines: usize,
    /// Number of lines that appear exactly once.
    pub unique_count: usize,
    /// The lines that appear exactly once, in first-appearance order.
    pub unique_lines: Vec<String>,
}

fn normalize(line: &str, ignore_case: bool, trim: bool) -> String {
    let s = if trim { line.trim() } else { line };
    if ignore_case {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

/// Find the lines that appear exactly once in `text`.
///
/// `ignore_case` compares case-insensitively; `trim` ignores surrounding
/// whitespace when comparing. Results are returned in the order the unique
/// lines first appeared.
pub fn find(text: &str, ignore_case: bool, trim: bool) -> Output {
    // key -> (display line, count, first index)
    let mut map: HashMap<String, (String, usize, usize)> = HashMap::new();
    let mut total = 0;
    for (i, line) in text.lines().enumerate() {
        total += 1;
        let key = normalize(line, ignore_case, trim);
        let display = if trim { line.trim().to_string() } else { line.to_string() };
        let entry = map.entry(key).or_insert((display, 0, i));
        entry.1 += 1;
    }

    let distinct_lines = map.len();
    let mut uniques: Vec<(String, usize)> = map
        .into_values()
        .filter(|(_, c, _)| *c == 1)
        .map(|(line, _, idx)| (line, idx))
        .collect();
    // preserve first-appearance order
    uniques.sort_by_key(|(_, idx)| *idx);

    let unique_lines: Vec<String> = uniques.into_iter().map(|(line, _)| line).collect();

    Output {
        total_lines: total,
        distinct_lines,
        unique_count: unique_lines.len(),
        unique_lines,
    }
}

/// Human-readable rendering (used by the page): the unique lines, one per line.
pub fn render(text: &str, ignore_case: bool, trim: bool) -> Result<String, String> {
    let o = find(text, ignore_case, trim);
    if o.unique_count == 0 {
        return Ok("No unique lines found — every line is repeated.".to_string());
    }
    Ok(o.unique_lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_unique_in_order() {
        // a×3, b×2, c×1, d×1 → only c and d are unique, in first-seen order
        let t = "a\nb\na\nc\nb\na\nd";
        let o = find(t, false, false);
        assert_eq!(o.total_lines, 7);
        assert_eq!(o.distinct_lines, 4);
        assert_eq!(o.unique_count, 2);
        assert_eq!(o.unique_lines, vec!["c".to_string(), "d".to_string()]);
    }

    #[test]
    fn all_unique() {
        let o = find("x\ny\nz", false, false);
        assert_eq!(o.unique_count, 3);
        assert_eq!(o.unique_lines, vec!["x", "y", "z"]);
    }

    #[test]
    fn none_unique() {
        let o = find("a\na\nb\nb", false, false);
        assert_eq!(o.unique_count, 0);
        assert!(o.unique_lines.is_empty());
    }

    #[test]
    fn ignore_case_groups() {
        // Foo/foo/FOO collapse to one key (count 3) → not unique
        let o = find("Foo\nfoo\nFOO\nbar", true, false);
        assert_eq!(o.unique_lines, vec!["bar".to_string()]);
    }

    #[test]
    fn case_sensitive_by_default() {
        // without ignore_case, Foo and foo are distinct, each appears once → both unique
        let o = find("Foo\nfoo", false, false);
        assert_eq!(o.unique_count, 2);
        assert_eq!(o.unique_lines, vec!["Foo".to_string(), "foo".to_string()]);
    }

    #[test]
    fn trim_whitespace_groups() {
        // "ab" appears 3× after trimming → not unique; "c" is unique
        let o = find("ab\n  ab  \nab\nc", false, true);
        assert_eq!(o.unique_lines, vec!["c".to_string()]);
    }

    #[test]
    fn first_appearance_order_preserved() {
        let o = find("z\na\nz", false, false);
        // z repeats, a is unique
        assert_eq!(o.unique_lines, vec!["a".to_string()]);
        let o2 = find("m\nk\nj", false, false);
        assert_eq!(o2.unique_lines, vec!["m", "k", "j"]);
    }

    #[test]
    fn render_works() {
        assert_eq!(render("a\nb\na", false, false).unwrap(), "b");
        assert!(render("a\na", false, false).unwrap().contains("No unique lines"));
    }
}
