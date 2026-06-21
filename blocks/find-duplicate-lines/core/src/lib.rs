//! gizza-ai/find-duplicate-lines core — list the lines that appear more than
//! once in text, with their counts. Pure-Rust, dependency-free (besides serde).

use std::collections::HashMap;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Duplicate {
    /// The line text (first-seen form for display).
    pub line: String,
    /// How many times it appears.
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    pub total_lines: usize,
    pub unique_lines: usize,
    /// Number of distinct lines that appear more than once.
    pub duplicate_count: usize,
    /// The duplicated lines, most frequent first (ties broken by first appearance).
    pub duplicates: Vec<Duplicate>,
}

fn normalize(line: &str, ignore_case: bool, trim: bool) -> String {
    let s = if trim { line.trim() } else { line };
    if ignore_case {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

/// Find duplicated lines in `text`.
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

    let unique_lines = map.len();
    let mut dups: Vec<(String, usize, usize)> =
        map.into_values().filter(|(_, c, _)| *c > 1).collect();
    // most frequent first, ties by first appearance
    dups.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));

    let duplicates: Vec<Duplicate> =
        dups.into_iter().map(|(line, count, _)| Duplicate { line, count }).collect();

    Output {
        total_lines: total,
        unique_lines,
        duplicate_count: duplicates.len(),
        duplicates,
    }
}

/// Human-readable rendering (used by the page).
pub fn render(text: &str, ignore_case: bool, trim: bool) -> Result<String, String> {
    let o = find(text, ignore_case, trim);
    if o.duplicate_count == 0 {
        return Ok("No duplicate lines found.".to_string());
    }
    let mut out = format!("{} duplicated line(s):\n", o.duplicate_count);
    for d in &o.duplicates {
        out.push_str(&format!("{:>5} × {}\n", d.count, d.line));
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_counts_and_order() {
        let t = "a\nb\na\nc\nb\na";
        let o = find(t, false, false);
        assert_eq!(o.total_lines, 6);
        assert_eq!(o.unique_lines, 3);
        assert_eq!(o.duplicate_count, 2);
        // a appears 3×, b 2× → a first
        assert_eq!(o.duplicates[0], Duplicate { line: "a".into(), count: 3 });
        assert_eq!(o.duplicates[1], Duplicate { line: "b".into(), count: 2 });
    }

    #[test]
    fn no_duplicates() {
        let o = find("x\ny\nz", false, false);
        assert_eq!(o.duplicate_count, 0);
        assert!(o.duplicates.is_empty());
    }

    #[test]
    fn ignore_case() {
        let o = find("Foo\nfoo\nFOO", true, false);
        assert_eq!(o.duplicates, vec![Duplicate { line: "Foo".into(), count: 3 }]);
    }

    #[test]
    fn trim_whitespace() {
        let o = find("ab\n  ab  \nab", false, true);
        assert_eq!(o.duplicates, vec![Duplicate { line: "ab".into(), count: 3 }]);
    }

    #[test]
    fn case_sensitive_by_default() {
        let o = find("Foo\nfoo", false, false);
        assert_eq!(o.duplicate_count, 0);
    }

    #[test]
    fn tie_break_by_first_appearance() {
        let o = find("z\nz\na\na", false, false);
        // both 2× → z first (appeared earlier)
        assert_eq!(o.duplicates[0].line, "z");
        assert_eq!(o.duplicates[1].line, "a");
    }

    #[test]
    fn render_works() {
        assert!(render("a\na", false, false).unwrap().contains("2 × a"));
        assert!(render("a\nb", false, false).unwrap().contains("No duplicate"));
    }
}
