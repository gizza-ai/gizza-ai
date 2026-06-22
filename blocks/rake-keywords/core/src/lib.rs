//! rake-keywords core — Rapid Automatic Keyword Extraction (RAKE).
//!
//! RAKE (Rose, Engel, Cramer & Cowley, 2010) extracts keywords/keyphrases from a
//! single document without training data:
//!   1. Split the text into candidate phrases at stopwords and punctuation.
//!   2. For every content word build a co-occurrence graph over the phrases and
//!      score each word = degree(word) / freq(word).
//!   3. A phrase's score is the sum of its member word scores.
//!   4. Rank phrases by score (desc), break ties by first appearance.
//!
//! Pure-Rust, no external deps — a built-in English stopword list is embedded.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Keyword {
    /// The keyphrase (lower-cased, whitespace-normalised).
    pub phrase: String,
    /// RAKE score (sum of member word degree/frequency scores).
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RakeResult {
    pub count: usize,
    pub keywords: Vec<Keyword>,
}

/// Built-in English stopword list (NLTK-style, the set RAKE was published with).
const STOPWORDS: &[&str] = &[
    "a", "about", "above", "after", "again", "against", "all", "also", "am", "an", "and", "any",
    "are", "aren't", "as", "at", "be", "because", "been", "before", "being", "below", "between",
    "both", "but", "by", "can", "can't", "cannot", "could", "couldn't", "did", "didn't", "do",
    "does", "doesn't", "doing", "don't", "down", "during", "each", "etc", "few", "for", "from",
    "further", "had", "hadn't", "has", "hasn't", "have", "haven't", "having", "he", "he'd",
    "he'll", "he's", "her", "here", "here's", "hers", "herself", "him", "himself", "his", "how",
    "how's", "however", "i", "i'd", "i'll", "i'm", "i've", "if", "in", "into", "is", "isn't",
    "it", "it's", "its", "itself", "let's", "may", "me", "might", "more", "most", "must",
    "mustn't", "my", "myself", "no", "nor", "not", "of", "off", "on", "once", "only", "or",
    "other", "ought", "our", "ours", "ourselves", "out", "over", "own", "same", "shall",
    "shan't", "she", "she'd", "she'll", "she's", "should", "shouldn't", "so", "some", "such",
    "than", "that", "that's", "the", "their", "theirs", "them", "themselves", "then", "there",
    "there's", "these", "they", "they'd", "they'll", "they're", "they've", "this", "those",
    "through", "to", "too", "under", "until", "up", "upon", "very", "was", "wasn't", "we",
    "we'd", "we'll", "we're", "we've", "were", "weren't", "what", "what's", "when", "when's",
    "where", "where's", "which", "while", "who", "who's", "whom", "why", "why's", "will",
    "with", "won't", "would", "wouldn't", "you", "you'd", "you'll", "you're", "you've", "your",
    "yours", "yourself", "yourselves",
];

fn is_stopword(w: &str) -> bool {
    STOPWORDS.binary_search(&w).is_ok()
}

/// Split text into candidate phrases: runs of content words delimited by
/// stopwords and any non-(alphanumeric|apostrophe|hyphen) character.
fn candidate_phrases(text: &str) -> Vec<Vec<String>> {
    let lower = text.to_lowercase();
    let mut phrases: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();

    // Tokenise on sentence/word boundaries; treat punctuation as a hard break.
    for raw_tok in lower.split(|c: char| {
        !(c.is_alphanumeric() || c == '\'' || c == '-')
    }) {
        let tok = raw_tok.trim_matches(|c: char| c == '\'' || c == '-');
        if tok.is_empty() {
            // a punctuation boundary ends the current phrase
            if !current.is_empty() {
                phrases.push(std::mem::take(&mut current));
            }
            continue;
        }
        if is_stopword(tok) {
            if !current.is_empty() {
                phrases.push(std::mem::take(&mut current));
            }
        } else {
            current.push(tok.to_string());
        }
    }
    if !current.is_empty() {
        phrases.push(current);
    }
    phrases
}

/// Run RAKE over `text` and return up to `top_n` keyphrases (0 = all),
/// optionally dropping single-word phrases when `min_word_len`/`phrases_only`
/// constraints apply.
///
/// `max_words` caps the phrase length (0 = no cap); `top_n` limits results
/// (0 = all).
pub fn rake(text: &str, top_n: usize, max_words: usize) -> RakeResult {
    use std::collections::HashMap;

    let mut phrases = candidate_phrases(text);
    if max_words > 0 {
        phrases.retain(|p| p.len() <= max_words);
    }

    // Word frequency and degree (degree = co-occurrence count incl. self).
    let mut freq: HashMap<&str, f64> = HashMap::new();
    let mut degree: HashMap<&str, f64> = HashMap::new();
    for p in &phrases {
        let n = p.len() as f64;
        for w in p {
            *freq.entry(w.as_str()).or_insert(0.0) += 1.0;
            // degree contribution = (phrase length - 1) + freq; classic RAKE
            // adds the phrase length to each member word's degree.
            *degree.entry(w.as_str()).or_insert(0.0) += n;
        }
    }

    // word score = degree / freq
    let mut word_score: HashMap<&str, f64> = HashMap::new();
    for (w, f) in &freq {
        let d = degree.get(w).copied().unwrap_or(0.0);
        word_score.insert(*w, d / f);
    }

    // Phrase score = sum of member word scores. Keep first-seen order for ties
    // and dedupe identical phrases (keep the first occurrence's score).
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut keywords: Vec<Keyword> = Vec::new();
    for p in &phrases {
        let phrase = p.join(" ");
        let score: f64 = p.iter().map(|w| word_score.get(w.as_str()).copied().unwrap_or(0.0)).sum();
        if let Some(&idx) = seen.get(&phrase) {
            // keep max score for the dedup'd phrase (scores are identical anyway)
            if score > keywords[idx].score {
                keywords[idx].score = score;
            }
        } else {
            seen.insert(phrase.clone(), keywords.len());
            keywords.push(Keyword { phrase, score });
        }
    }

    // Sort by score desc; stable sort preserves first-seen order on ties.
    keywords.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // Round scores to 3 decimals for stable presentation.
    for k in &mut keywords {
        k.score = (k.score * 1000.0).round() / 1000.0;
    }

    if top_n > 0 && keywords.len() > top_n {
        keywords.truncate(top_n);
    }

    RakeResult { count: keywords.len(), keywords }
}

/// Human-readable rendering for the page surface.
pub fn render(text: &str, top_n: usize, max_words: usize) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("Please provide some text to analyse.".into());
    }
    let r = rake(text, top_n, max_words);
    if r.count == 0 {
        return Ok("No keywords found.".into());
    }
    let mut out = format!("Top {} keyphrase(s):\n", r.count);
    for (i, k) in r.keywords.iter().enumerate() {
        out.push_str(&format!("{:>2}. {}  ({:.3})\n", i + 1, k.phrase, k.score));
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopwords_sorted_for_binary_search() {
        let mut s = STOPWORDS.to_vec();
        s.sort();
        assert_eq!(s, STOPWORDS, "STOPWORDS must stay sorted for binary_search");
    }

    #[test]
    fn extracts_multiword_phrases() {
        // Classic RAKE example sentence.
        let text = "Compatibility of systems of linear constraints over the set of natural numbers. \
                    Criteria of compatibility of a system of linear Diophantine equations, strict \
                    inequations, and nonstrict inequations are considered.";
        let r = rake(text, 0, 0);
        // The highest-scoring phrases should be the long content phrases.
        let top = &r.keywords[0].phrase;
        assert!(
            top.contains("linear") || top.contains("inequations") || top.contains("numbers"),
            "unexpected top phrase: {top}"
        );
        // multi-word phrase present
        assert!(r.keywords.iter().any(|k| k.phrase.split(' ').count() >= 2));
    }

    #[test]
    fn splits_on_stopwords_and_punctuation() {
        let r = rake("the quick brown fox and the lazy dog", 0, 0);
        let phrases: Vec<&str> = r.keywords.iter().map(|k| k.phrase.as_str()).collect();
        assert!(phrases.contains(&"quick brown fox"));
        assert!(phrases.contains(&"lazy dog"));
    }

    #[test]
    fn top_n_limits_results() {
        let text = "machine learning models. deep neural networks. natural language processing. \
                    computer vision systems. reinforcement learning agents.";
        let r = rake(text, 2, 0);
        assert_eq!(r.count, 2);
        assert_eq!(r.keywords.len(), 2);
    }

    #[test]
    fn max_words_caps_phrase_length() {
        let r = rake("very long candidate keyphrase here", 0, 2);
        // the 4-word phrase is dropped; nothing left -> no keywords
        assert!(r.keywords.iter().all(|k| k.phrase.split(' ').count() <= 2));
    }

    #[test]
    fn dedupes_repeated_phrases() {
        let r = rake("data science. data science. data science.", 0, 0);
        let count = r.keywords.iter().filter(|k| k.phrase == "data science").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn scores_are_descending() {
        let text = "Compatibility of systems of linear constraints over the set of natural numbers.";
        let r = rake(text, 0, 0);
        for w in r.keywords.windows(2) {
            assert!(w[0].score >= w[1].score, "scores not sorted desc");
        }
    }

    #[test]
    fn render_errors_on_empty() {
        assert!(render("   ", 0, 0).is_err());
    }

    #[test]
    fn render_lists_phrases() {
        let out = render("quick brown fox jumps", 0, 0).unwrap();
        assert!(out.contains("quick brown fox jumps"));
        assert!(out.contains("keyphrase"));
    }
}
