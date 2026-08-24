//! sentence-length-stats core — measure how long the sentences in a block of
//! text are: count, average, median, shortest, longest, spread, and the
//! distribution across five length bands. Pure compute, shared by the chat
//! skill block, the CLI and the web page.
//!
//! Sentence boundaries come from the `sentence-split` block's rule-based
//! detector, so abbreviations (`Dr.`, `e.g.`), initials (`J. R. R.`), decimals
//! (`3.14`), reference prefixes (`No. 5`) and list enumerators (`1. Buy milk`)
//! do not create phantom sentences. No model, no training data — the same input
//! always yields the same numbers.

use gizza_ai_sentence_split_core::{split_text, MAX_CHARS};
use serde::Serialize;

/// Largest accepted `long_threshold` (words).
pub const MAX_LONG_THRESHOLD: usize = 500;
/// Largest number of longest sentences that can be listed.
pub const MAX_LIST_LONGEST: usize = 50;
/// Word-count gap at or below which two adjacent sentences count as "similar".
pub const SIMILAR_GAP: usize = 2;
/// Longest snippet shown for a listed sentence, in characters.
const SNIPPET_CHARS: usize = 80;

/// The five fixed length bands, as `(label, inclusive_low, inclusive_high)`.
/// `usize::MAX` is the open top end. Bands match the convention used across
/// writing tools: very short under 10, then 10–14, 15–24, 25–34, 35 and up.
const BANDS: [(&str, usize, usize); 5] = [
    ("Very short", 1, 9),
    ("Short", 10, 14),
    ("Medium", 15, 24),
    ("Long", 25, 34),
    ("Very long", 35, usize::MAX),
];

/// One row of the length distribution.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Band {
    /// Human label, e.g. `"Medium"`.
    pub label: String,
    /// Word range as displayed, e.g. `"15-24"` or `"35+"`.
    pub range: String,
    /// Sentences that fall in this band.
    pub count: usize,
    /// Share of all sentences, in percent, 1 decimal.
    pub percent: f64,
}

/// One entry of the "longest sentences" list.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LongSentence {
    /// 1-based position of the sentence in the input.
    pub index: usize,
    pub words: usize,
    pub characters: usize,
    /// The sentence, truncated to a readable snippet.
    pub text: String,
}

/// Everything the tool reports.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stats {
    pub sentences: usize,
    pub words: usize,
    pub characters: usize,
    /// Mean words per sentence, 1 decimal.
    pub average_words: f64,
    /// Mean characters per sentence, 1 decimal.
    pub average_characters: f64,
    /// Middle value of the sorted word counts, 1 decimal.
    pub median_words: f64,
    pub shortest_words: usize,
    /// 1-based index of the shortest sentence (first one, on a tie).
    pub shortest_index: usize,
    pub longest_words: usize,
    /// 1-based index of the longest sentence (first one, on a tie).
    pub longest_index: usize,
    /// Population standard deviation of the word counts, 1 decimal.
    pub std_deviation: f64,
    /// 0–100 spread score, or `None` with fewer than two sentences.
    pub variety_score: Option<u32>,
    /// `"varied"` / `"moderate"` / `"monotonous"`, or `None` when unscored.
    pub variety_label: Option<String>,
    /// Word count at or above which a sentence counts as long.
    pub long_threshold: usize,
    pub long_sentences: usize,
    /// Share of sentences that are long, in percent, 1 decimal.
    pub long_percent: f64,
    /// Adjacent sentence pairs whose word counts differ by <= [`SIMILAR_GAP`].
    pub similar_pairs: usize,
    /// Total adjacent pairs (`sentences - 1`).
    pub total_pairs: usize,
    /// Share of adjacent pairs that are similar, in percent, 1 decimal.
    pub similar_percent: f64,
    pub distribution: Vec<Band>,
    pub longest_sentences: Vec<LongSentence>,
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn percent(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        round1(part as f64 * 100.0 / whole as f64)
    }
}

fn band_range(low: usize, high: usize) -> String {
    if high == usize::MAX {
        format!("{low}+")
    } else {
        format!("{low}-{high}")
    }
}

fn snippet(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= SNIPPET_CHARS {
        return text.to_string();
    }
    let head: String = chars[..SNIPPET_CHARS].iter().collect();
    format!("{}…", head.trim_end())
}

/// Analyze `text` and return its sentence-length statistics.
///
/// - `newlines`: `"paragraph"` (default — only a blank line ends a sentence),
///   `"never"` (line breaks are plain whitespace) or `"always"` (every line
///   break ends a sentence; use for subtitles and bullet lists).
/// - `long_threshold`: word count at or above which a sentence is counted as
///   long (1..=[`MAX_LONG_THRESHOLD`]).
/// - `list_longest`: how many of the longest sentences to return
///   (0..=[`MAX_LIST_LONGEST`]; 0 lists none).
/// - `extra_abbreviations`: comma-separated extra abbreviations that must never
///   end a sentence, e.g. `"Ing., approx."`.
///
/// Returns `Err` with a message naming what was expected when the text is
/// empty or over [`MAX_CHARS`], when an option is out of range, or when
/// `newlines` is not one of the three accepted values.
pub fn analyze(
    text: &str,
    newlines: &str,
    long_threshold: usize,
    list_longest: usize,
    extra_abbreviations: &str,
) -> Result<Stats, String> {
    if text.trim().is_empty() {
        return Err(
            "text is empty: paste the text whose sentence lengths you want to measure".to_string(),
        );
    }
    let char_count = text.chars().count();
    if char_count > MAX_CHARS {
        return Err(format!(
            "text is {char_count} characters: the maximum is {MAX_CHARS}"
        ));
    }
    if long_threshold < 1 || long_threshold > MAX_LONG_THRESHOLD {
        return Err(format!(
            "long_threshold is {long_threshold}: expected 1 to {MAX_LONG_THRESHOLD} words"
        ));
    }
    if list_longest > MAX_LIST_LONGEST {
        return Err(format!(
            "list_longest is {list_longest}: expected 0 to {MAX_LIST_LONGEST}"
        ));
    }

    let sentences = split_text(text, newlines, true, 0, extra_abbreviations)?;
    let n = sentences.len();

    let words: usize = sentences.iter().map(|s| s.words).sum();
    let characters: usize = sentences.iter().map(|s| s.characters).sum();
    let average_words = round1(words as f64 / n as f64);
    let average_characters = round1(characters as f64 / n as f64);

    let mut sorted: Vec<usize> = sentences.iter().map(|s| s.words).collect();
    sorted.sort_unstable();
    let median_words = if n % 2 == 1 {
        sorted[n / 2] as f64
    } else {
        round1((sorted[n / 2 - 1] + sorted[n / 2]) as f64 / 2.0)
    };

    // First index wins a tie, so the report is stable.
    let (shortest_index, shortest_words) = sentences
        .iter()
        .map(|s| (s.index, s.words))
        .min_by_key(|(i, w)| (*w, *i))
        .expect("at least one sentence");
    let (longest_index, longest_words) = sentences
        .iter()
        .map(|s| (s.index, s.words))
        .max_by_key(|(i, w)| (*w, std::cmp::Reverse(*i)))
        .expect("at least one sentence");

    let mean = words as f64 / n as f64;
    let variance = sentences
        .iter()
        .map(|s| {
            let d = s.words as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / n as f64;
    let std_deviation = variance.sqrt();

    // Variety = coefficient of variation, mapped onto 0-100. A CV of 0.8 (a
    // strong mix of clipped and flowing sentences) and above scores 100.
    let (variety_score, variety_label) = if n < 2 || mean == 0.0 {
        (None, None)
    } else {
        let cv = std_deviation / mean;
        let score = ((cv / 0.8).min(1.0) * 100.0).round() as u32;
        let label = if score >= 70 {
            "varied"
        } else if score >= 40 {
            "moderate"
        } else {
            "monotonous"
        };
        (Some(score), Some(label.to_string()))
    };

    let long_sentences = sentences
        .iter()
        .filter(|s| s.words >= long_threshold)
        .count();
    let total_pairs = n.saturating_sub(1);
    let similar_pairs = sentences
        .windows(2)
        .filter(|w| w[0].words.abs_diff(w[1].words) <= SIMILAR_GAP)
        .count();

    let distribution = BANDS
        .iter()
        .map(|(label, low, high)| {
            let count = sentences
                .iter()
                .filter(|s| s.words >= *low && s.words <= *high)
                .count();
            Band {
                label: (*label).to_string(),
                range: band_range(*low, *high),
                count,
                percent: percent(count, n),
            }
        })
        .collect();

    let mut ranked: Vec<_> = sentences.iter().collect();
    // Longest first; ties keep document order.
    ranked.sort_by_key(|s| (std::cmp::Reverse(s.words), s.index));
    let longest_sentences = ranked
        .into_iter()
        .take(list_longest)
        .map(|s| LongSentence {
            index: s.index,
            words: s.words,
            characters: s.characters,
            text: snippet(&s.text),
        })
        .collect();

    Ok(Stats {
        sentences: n,
        words,
        characters,
        average_words,
        average_characters,
        median_words,
        shortest_words,
        shortest_index,
        longest_words,
        longest_index,
        std_deviation: round1(std_deviation),
        variety_score,
        variety_label,
        long_threshold,
        long_sentences,
        long_percent: percent(long_sentences, n),
        similar_pairs,
        total_pairs,
        similar_percent: percent(similar_pairs, total_pairs),
        distribution,
        longest_sentences,
    })
}

/// Render [`Stats`] as the plain-text report the page and CLI show.
pub fn render(s: &Stats) -> String {
    let mut out = String::new();
    out.push_str(&format!("Sentences: {}\n", s.sentences));
    out.push_str(&format!("Words: {}\n", s.words));
    out.push_str(&format!(
        "Average length: {:.1} words ({:.1} characters)\n",
        s.average_words, s.average_characters
    ));
    out.push_str(&format!("Median length: {:.1} words\n", s.median_words));
    out.push_str(&format!(
        "Shortest: {} words (sentence {})\n",
        s.shortest_words, s.shortest_index
    ));
    out.push_str(&format!(
        "Longest: {} words (sentence {})\n",
        s.longest_words, s.longest_index
    ));
    out.push_str(&format!(
        "Standard deviation: {:.1} words\n",
        s.std_deviation
    ));
    match (s.variety_score, &s.variety_label) {
        (Some(score), Some(label)) => {
            out.push_str(&format!("Variety score: {score}/100 ({label})\n"))
        }
        _ => out.push_str("Variety score: n/a (needs at least 2 sentences)\n"),
    }
    out.push_str(&format!(
        "Long sentences ({}+ words): {} of {} ({:.1}%)\n",
        s.long_threshold, s.long_sentences, s.sentences, s.long_percent
    ));
    if s.total_pairs > 0 {
        out.push_str(&format!(
            "Adjacent pairs within {} words: {} of {} ({:.1}%)\n",
            SIMILAR_GAP, s.similar_pairs, s.total_pairs, s.similar_percent
        ));
    }

    out.push_str("\nDistribution (words per sentence)\n");
    let peak = s.distribution.iter().map(|b| b.count).max().unwrap_or(0);
    for b in &s.distribution {
        let bar_len = if b.count == 0 || peak == 0 {
            0
        } else {
            ((b.count as f64 / peak as f64) * 20.0).round().max(1.0) as usize
        };
        out.push_str(&format!(
            "  {:<11}{:<7}{:>3}  {:>5.1}%  {}\n",
            b.label,
            b.range,
            b.count,
            b.percent,
            "#".repeat(bar_len)
        ));
    }

    if !s.longest_sentences.is_empty() {
        out.push_str("\nLongest sentences\n");
        for (rank, ls) in s.longest_sentences.iter().enumerate() {
            out.push_str(&format!(
                "  {}. {} words (sentence {}): {}\n",
                rank + 1,
                ls.words,
                ls.index,
                ls.text
            ));
        }
    }

    out.trim_end().to_string()
}

/// Convenience wrapper: analyze then render. Used by the web page and the CLI
/// text surface.
pub fn run(
    text: &str,
    newlines: &str,
    long_threshold: usize,
    list_longest: usize,
    extra_abbreviations: &str,
) -> Result<String, String> {
    analyze(
        text,
        newlines,
        long_threshold,
        list_longest,
        extra_abbreviations,
    )
    .map(|s| render(&s))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "The cat sat. It was warm and the sun came through the window in a long \
        bright stripe that fell across the floor. Rain later.";

    #[test]
    fn happy_path_counts_and_averages() {
        let s = analyze(SAMPLE, "paragraph", 25, 3, "").unwrap();
        assert_eq!(s.sentences, 3);
        assert_eq!(s.shortest_words, 2);
        assert_eq!(s.shortest_index, 3);
        assert_eq!(s.longest_words, 20);
        assert_eq!(s.longest_index, 2);
        assert_eq!(s.words, 3 + 20 + 2);
        assert_eq!(s.average_words, 8.3);
        assert_eq!(s.median_words, 3.0);
        assert_eq!(s.distribution.len(), 5);
        // 2 and 3 words -> very short; 20 words -> medium.
        assert_eq!(s.distribution[0].count, 2);
        assert_eq!(s.distribution[2].count, 1);
        assert_eq!(s.longest_sentences[0].index, 2);
        assert_eq!(s.longest_sentences.len(), 3);
    }

    #[test]
    fn abbreviations_do_not_split_sentences() {
        let s = analyze(
            "Dr. Ada met Mr. Poe at 3.14 p.m. They talked.",
            "paragraph",
            25,
            0,
            "",
        )
        .unwrap();
        assert_eq!(s.sentences, 2);
    }

    #[test]
    fn extra_abbreviations_are_honoured() {
        let plain = analyze(
            "Acme. ten people came. Then it rained.",
            "paragraph",
            25,
            0,
            "",
        )
        .unwrap();
        assert_eq!(plain.sentences, 2);
        let tuned = analyze(
            "Acme. ten people came. Then it rained.",
            "paragraph",
            25,
            0,
            "acme",
        )
        .unwrap();
        assert_eq!(tuned.sentences, 2);
    }

    #[test]
    fn newlines_always_splits_line_oriented_text() {
        let text = "First line\nSecond line\nThird line";
        assert_eq!(analyze(text, "never", 25, 0, "").unwrap().sentences, 1);
        assert_eq!(analyze(text, "always", 25, 0, "").unwrap().sentences, 3);
    }

    #[test]
    fn long_threshold_is_configurable() {
        let s = analyze(SAMPLE, "paragraph", 20, 0, "").unwrap();
        assert_eq!(s.long_sentences, 1);
        assert_eq!(s.long_threshold, 20);
        let s = analyze(SAMPLE, "paragraph", 30, 0, "").unwrap();
        assert_eq!(s.long_sentences, 0);
    }

    #[test]
    fn median_averages_the_middle_pair_on_even_counts() {
        let s = analyze("One two. One two three four.", "paragraph", 25, 0, "").unwrap();
        assert_eq!(s.sentences, 2);
        assert_eq!(s.median_words, 3.0);
    }

    #[test]
    fn variety_is_unscored_for_a_single_sentence() {
        let s = analyze("Just the one sentence here.", "paragraph", 25, 0, "").unwrap();
        assert_eq!(s.variety_score, None);
        assert_eq!(s.variety_label, None);
        assert_eq!(s.total_pairs, 0);
        assert!(render(&s).contains("Variety score: n/a"));
    }

    #[test]
    fn identical_lengths_score_monotonous_and_fully_similar() {
        let s = analyze(
            "One two three. Four five six. Seven eight nine.",
            "paragraph",
            25,
            0,
            "",
        )
        .unwrap();
        assert_eq!(s.std_deviation, 0.0);
        assert_eq!(s.variety_score, Some(0));
        assert_eq!(s.variety_label.as_deref(), Some("monotonous"));
        assert_eq!(s.similar_pairs, 2);
        assert_eq!(s.total_pairs, 2);
        assert_eq!(s.similar_percent, 100.0);
    }

    #[test]
    fn list_longest_zero_omits_the_section() {
        let s = analyze(SAMPLE, "paragraph", 25, 0, "").unwrap();
        assert!(s.longest_sentences.is_empty());
        assert!(!render(&s).contains("Longest sentences"));
    }

    #[test]
    fn snippets_are_truncated() {
        let long = format!("{} end.", "word ".repeat(40));
        let s = analyze(&long, "paragraph", 25, 1, "").unwrap();
        let shown = &s.longest_sentences[0].text;
        assert!(shown.ends_with('…'));
        assert!(shown.chars().count() <= SNIPPET_CHARS + 1);
    }

    #[test]
    fn render_has_every_section() {
        let out = run(SAMPLE, "paragraph", 25, 2, "").unwrap();
        assert!(out.starts_with("Sentences: 3\n"));
        assert!(out.contains("Distribution (words per sentence)"));
        assert!(out.contains("Very long  35+"));
        assert!(out.contains("Longest sentences"));
    }

    #[test]
    fn empty_text_is_an_error() {
        assert_eq!(
            analyze("   ", "paragraph", 25, 3, "").unwrap_err(),
            "text is empty: paste the text whose sentence lengths you want to measure"
        );
    }

    #[test]
    fn out_of_range_options_are_errors() {
        assert_eq!(
            analyze("Hi there.", "paragraph", 0, 3, "").unwrap_err(),
            "long_threshold is 0: expected 1 to 500 words"
        );
        assert_eq!(
            analyze("Hi there.", "paragraph", 25, 51, "").unwrap_err(),
            "list_longest is 51: expected 0 to 50"
        );
    }

    #[test]
    fn invalid_newlines_is_an_error() {
        let err = analyze("Hi there.", "sometimes", 25, 3, "").unwrap_err();
        assert!(err.contains("invalid newlines"), "{err}");
    }

    #[test]
    fn oversized_text_is_an_error() {
        let big = "a. ".repeat(MAX_CHARS);
        let err = analyze(&big, "paragraph", 25, 3, "").unwrap_err();
        assert!(err.contains("the maximum is"), "{err}");
    }
}
