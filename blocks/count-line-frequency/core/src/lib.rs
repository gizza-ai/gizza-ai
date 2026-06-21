//! gizza-ai/count-line-frequency core — count how often each line/value occurs
//! and rank them most→least frequent (like `sort | uniq -c | sort -rn`).
//! Pure-Rust, dependency-free (besides serde for the output struct).

use std::collections::HashMap;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub value: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub entries: Vec<Entry>,
    /// Number of distinct values.
    pub distinct: usize,
    /// Total non-skipped lines counted.
    pub total: usize,
}

/// Count line frequencies. `case_sensitive=false` lowercases for grouping (the
/// first-seen original casing is kept as the displayed value). `trim` strips
/// surrounding whitespace; blank lines are always skipped.
pub fn count(text: &str, case_sensitive: bool, trim: bool) -> Counts {
    // key -> (display value, count); preserve first-seen order for stable ties.
    let mut map: HashMap<String, usize> = HashMap::new();
    let mut order: Vec<String> = Vec::new(); // keys in first-seen order
    let mut display: HashMap<String, String> = HashMap::new();
    let mut total = 0usize;

    for raw in text.lines() {
        let line = if trim { raw.trim() } else { raw };
        if line.is_empty() {
            continue;
        }
        let key = if case_sensitive { line.to_string() } else { line.to_lowercase() };
        total += 1;
        let e = map.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            display.insert(key.clone(), line.to_string());
            0
        });
        *e += 1;
    }

    // Rank by count desc, then first-seen order for ties (stable).
    let mut idx: Vec<(usize, &String)> = order.iter().enumerate().map(|(i, k)| (i, k)).collect();
    idx.sort_by(|a, b| map[b.1].cmp(&map[a.1]).then(a.0.cmp(&b.0)));

    let entries: Vec<Entry> = idx
        .into_iter()
        .map(|(_, k)| Entry { value: display[k].clone(), count: map[k] })
        .collect();

    let distinct = entries.len();
    Counts { entries, distinct, total }
}

/// Plain-text "count<TAB>value" lines, most frequent first (used by the page).
pub fn format_table(text: &str, case_sensitive: bool, trim: bool) -> String {
    let c = count(text, case_sensitive, trim);
    c.entries
        .iter()
        .map(|e| format!("{}\t{}", e.count, e.value))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_by_frequency() {
        let c = count("apple\nbanana\napple\ncherry\napple\nbanana", true, true);
        assert_eq!(c.distinct, 3);
        assert_eq!(c.total, 6);
        assert_eq!(c.entries[0], Entry { value: "apple".into(), count: 3 });
        assert_eq!(c.entries[1], Entry { value: "banana".into(), count: 2 });
        assert_eq!(c.entries[2], Entry { value: "cherry".into(), count: 1 });
    }

    #[test]
    fn ties_keep_first_seen_order() {
        let c = count("b\na\nb\na", true, true);
        // both count 2; b seen first.
        assert_eq!(c.entries[0].value, "b");
        assert_eq!(c.entries[1].value, "a");
    }

    #[test]
    fn case_insensitive_groups() {
        let c = count("Apple\napple\nAPPLE", false, true);
        assert_eq!(c.distinct, 1);
        assert_eq!(c.entries[0].count, 3);
        assert_eq!(c.entries[0].value, "Apple"); // first-seen casing
    }

    #[test]
    fn trim_and_skip_blanks() {
        let c = count("  x  \nx\n\n   \ny", true, true);
        assert_eq!(c.total, 3);
        assert_eq!(c.entries[0], Entry { value: "x".into(), count: 2 });
    }

    #[test]
    fn no_trim_distinguishes_whitespace() {
        let c = count("x\n x", true, false);
        assert_eq!(c.distinct, 2);
    }

    #[test]
    fn table_format() {
        let t = format_table("a\nb\na", true, true);
        assert_eq!(t, "2\ta\n1\tb");
    }

    #[test]
    fn empty() {
        let c = count("", true, true);
        assert_eq!(c.distinct, 0);
        assert_eq!(c.total, 0);
    }
}
