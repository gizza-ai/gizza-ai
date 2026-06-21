//! gizza-ai/word-frequency core — count how often each word occurs in text and
//! rank them most→least frequent. Pure-Rust, dependency-free (besides serde).
//!
//! Words are runs of letters/digits (Unicode-aware) plus interior apostrophes
//! ("don't" stays one word); all other characters are separators. Optionally
//! lowercase for grouping, drop words shorter than `min_length`, remove common
//! English stop words, and keep only the top `top` results.

use std::collections::HashMap;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub word: String,
    pub count: usize,
    /// Share of all counted words, as a percentage rounded to 2 decimals.
    pub percent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Counts {
    pub entries: Vec<Entry>,
    /// Number of distinct words after filtering.
    pub distinct: usize,
    /// Total words counted (sum of all counts).
    pub total: usize,
}

/// Common English stop words (lowercase, kept sorted for binary search).
/// Filtered out when `ignore_stopwords` is on.
pub const STOP_WORDS: &[&str] = &[
    "a", "about", "above", "after", "again", "against", "all", "am", "an", "and", "any", "are",
    "aren't", "as", "at", "be", "because", "been", "before", "being", "below", "between", "both",
    "but", "by", "can", "can't", "cannot", "could", "couldn't", "did", "didn't", "do", "does",
    "doesn't", "doing", "don't", "down", "during", "each", "few", "for", "from", "further", "had",
    "hadn't", "has", "hasn't", "have", "haven't", "having", "he", "he'd", "he'll", "he's", "her",
    "here", "here's", "hers", "herself", "him", "himself", "his", "how", "how's", "i", "i'd",
    "i'll", "i'm", "i've", "if", "in", "into", "is", "isn't", "it", "it's", "its", "itself",
    "let's", "me", "more", "most", "mustn't", "my", "myself", "no", "nor", "not", "of", "off",
    "on", "once", "only", "or", "other", "ought", "our", "ours", "ourselves", "out", "over", "own",
    "same", "shan't", "she", "she'd", "she'll", "she's", "should", "shouldn't", "so", "some",
    "such", "than", "that", "that's", "the", "their", "theirs", "them", "themselves", "then",
    "there", "there's", "these", "they", "they'd", "they'll", "they're", "they've", "this", "those",
    "through", "to", "too", "under", "until", "up", "very", "was", "wasn't", "we", "we'd", "we'll",
    "we're", "we've", "were", "weren't", "what", "what's", "when", "when's", "where", "where's",
    "which", "while", "who", "who's", "whom", "why", "why's", "with", "won't", "would", "wouldn't",
    "you", "you'd", "you'll", "you're", "you've", "your", "yours", "yourself", "yourselves",
];

fn is_stop_word(lower: &str) -> bool {
    STOP_WORDS.binary_search(&lower).is_ok()
}

/// Split `text` into words. A word is a run of alphanumerics with optional
/// interior apostrophes (straight `'` or curly `’` between two word chars).
/// Everything else separates.
fn tokenize(text: &str) -> Vec<&str> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut end = 0usize;
    for (idx, (i, c)) in chars.iter().enumerate() {
        let is_apos = *c == '\'' || *c == '\u{2019}';
        let next_is_word = chars
            .get(idx + 1)
            .map(|(_, nc)| nc.is_alphanumeric())
            .unwrap_or(false);
        let in_word = c.is_alphanumeric() || (is_apos && start.is_some() && next_is_word);
        if in_word {
            if start.is_none() {
                start = Some(*i);
            }
            end = i + c.len_utf8();
        } else if let Some(s) = start.take() {
            out.push(&text[s..end]);
        }
    }
    if let Some(s) = start {
        out.push(&text[s..end]);
    }
    out
}

/// Count word frequencies and rank most→least frequent.
///
/// - `case_sensitive=false` (typical) lowercases for grouping; the first-seen
///   original casing is kept for display.
/// - `min_length` drops words shorter than N characters (1 = keep all).
/// - `ignore_stopwords` removes common English stop words (the, and, of, …).
/// - `top` keeps only the N most frequent (0 = all).
///
/// Ties keep first-seen order, so the output is fully deterministic.
pub fn count(
    text: &str,
    case_sensitive: bool,
    min_length: usize,
    ignore_stopwords: bool,
    top: usize,
) -> Counts {
    let mut map: HashMap<String, usize> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut display: HashMap<String, String> = HashMap::new();
    let mut total = 0usize;
    let min_length = min_length.max(1);

    for word in tokenize(text) {
        if word.chars().count() < min_length {
            continue;
        }
        let lower = word.to_lowercase();
        if ignore_stopwords && is_stop_word(&lower) {
            continue;
        }
        let key = if case_sensitive { word.to_string() } else { lower };
        total += 1;
        let e = map.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            display.insert(key.clone(), word.to_string());
            0
        });
        *e += 1;
    }

    let mut idx: Vec<(usize, &String)> = order.iter().enumerate().collect();
    idx.sort_by(|a, b| map[b.1].cmp(&map[a.1]).then(a.0.cmp(&b.0)));

    let mut entries: Vec<Entry> = idx
        .into_iter()
        .map(|(_, k)| {
            let count = map[k];
            let percent = if total == 0 {
                0.0
            } else {
                ((count as f64 / total as f64) * 10_000.0).round() / 100.0
            };
            Entry { word: display[k].clone(), count, percent }
        })
        .collect();

    let distinct = entries.len();
    if top > 0 && entries.len() > top {
        entries.truncate(top);
    }

    Counts { entries, distinct, total }
}

/// Plain-text "count<TAB>word" lines, most frequent first (used by the page).
pub fn format_table(
    text: &str,
    case_sensitive: bool,
    min_length: usize,
    ignore_stopwords: bool,
    top: usize,
) -> String {
    let c = count(text, case_sensitive, min_length, ignore_stopwords, top);
    c.entries
        .iter()
        .map(|e| format!("{}\t{:.2}%\t{}", e.count, e.percent, e.word))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_by_frequency() {
        let c = count("red blue red green red blue", false, 1, false, 0);
        assert_eq!(c.distinct, 3);
        assert_eq!(c.total, 6);
        assert_eq!((c.entries[0].word.as_str(), c.entries[0].count), ("red", 3));
        assert_eq!(c.entries[0].percent, 50.0);
        assert_eq!((c.entries[1].word.as_str(), c.entries[1].count), ("blue", 2));
        assert_eq!((c.entries[2].word.as_str(), c.entries[2].count), ("green", 1));
    }

    #[test]
    fn case_insensitive_groups_keep_first_casing() {
        let c = count("The the THE", false, 1, false, 0);
        assert_eq!(c.distinct, 1);
        assert_eq!((c.entries[0].word.as_str(), c.entries[0].count), ("The", 3));
        assert_eq!(c.entries[0].percent, 100.0);
    }

    #[test]
    fn case_sensitive_separates() {
        let c = count("The the", true, 1, false, 0);
        assert_eq!(c.distinct, 2);
    }

    #[test]
    fn ties_keep_first_seen_order() {
        let c = count("b a b a", true, 1, false, 0);
        assert_eq!(c.entries[0].word, "b");
        assert_eq!(c.entries[1].word, "a");
    }

    #[test]
    fn punctuation_is_a_separator() {
        let c = count("hello, world! hello.", false, 1, false, 0);
        assert_eq!((c.entries[0].word.as_str(), c.entries[0].count), ("hello", 2));
        assert_eq!((c.entries[1].word.as_str(), c.entries[1].count), ("world", 1));
    }

    #[test]
    fn keeps_interior_apostrophe() {
        let c = count("don't don't can't", false, 1, false, 0);
        assert_eq!(c.distinct, 2);
        assert_eq!((c.entries[0].word.as_str(), c.entries[0].count), ("don't", 2));
    }

    #[test]
    fn curly_apostrophe_kept() {
        let c = count("don\u{2019}t don\u{2019}t", false, 1, false, 0);
        assert_eq!(c.entries[0].count, 2);
        assert_eq!(c.entries[0].word, "don\u{2019}t");
    }

    #[test]
    fn trailing_apostrophe_not_part_of_word() {
        // possessive plural "dogs'" -> the trailing apostrophe is a separator.
        let c = count("dogs'", false, 1, false, 0);
        assert_eq!(c.entries[0].word, "dogs");
    }

    #[test]
    fn min_length_filters_short_words() {
        let c = count("a an the apple", false, 3, false, 0);
        let words: Vec<&str> = c.entries.iter().map(|e| e.word.as_str()).collect();
        assert!(words.contains(&"the"));
        assert!(words.contains(&"apple"));
        assert!(!words.contains(&"a"));
        assert!(!words.contains(&"an"));
    }

    #[test]
    fn ignore_stopwords_drops_them() {
        let c = count("the cat sat on the mat", false, 1, true, 0);
        let words: Vec<&str> = c.entries.iter().map(|e| e.word.as_str()).collect();
        assert!(!words.contains(&"the"));
        assert!(!words.contains(&"on"));
        assert!(words.contains(&"cat"));
        assert!(words.contains(&"mat"));
    }

    #[test]
    fn top_truncates_but_distinct_is_full() {
        let c = count("a a b b c", false, 1, false, 2);
        assert_eq!(c.distinct, 3);
        assert_eq!(c.entries.len(), 2);
        assert_eq!(c.entries[0].count, 2);
    }

    #[test]
    fn digits_and_unicode_count_as_words() {
        let c = count("café café 2024 2024 2024", false, 1, false, 0);
        assert_eq!((c.entries[0].word.as_str(), c.entries[0].count), ("2024", 3));
        assert_eq!((c.entries[1].word.as_str(), c.entries[1].count), ("café", 2));
    }

    #[test]
    fn empty_input() {
        let c = count("", false, 1, false, 0);
        assert_eq!(c.distinct, 0);
        assert_eq!(c.total, 0);
        assert!(c.entries.is_empty());
    }

    #[test]
    fn table_format() {
        let t = format_table("a a b", false, 1, false, 0);
        assert_eq!(t, "2\t66.67%\ta\n1\t33.33%\tb");
    }

    #[test]
    fn stop_words_are_sorted_for_binary_search() {
        let mut sorted = STOP_WORDS.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted.as_slice(), STOP_WORDS, "STOP_WORDS must stay sorted");
    }
}
