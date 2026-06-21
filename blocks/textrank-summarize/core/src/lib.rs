//! gizza-ai/textrank-summarize core — extractive text summary via the TextRank
//! algorithm. Pure-Rust, dependency-free, no ML model.
//!
//! Splits the text into sentences, builds a graph where sentences are connected
//! by word overlap (the classic TextRank similarity: shared words normalised by
//! the log of each sentence's length), runs PageRank, and returns the top-ranked
//! sentences in their original order.

const DAMPING: f64 = 0.85;
const ITERATIONS: usize = 40;

const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "has", "he", "in", "is", "it",
    "its", "of", "on", "that", "the", "to", "was", "were", "will", "with", "this", "but", "they",
    "have", "had", "what", "when", "where", "who", "which", "why", "how", "all", "would", "there",
    "their", "i", "you", "we", "or", "so", "if", "not", "no", "do", "does", "did", "can", "could",
    "should", "than", "then", "them", "these", "those", "been", "being", "her", "his", "she", "him",
];

/// Split text into sentences, returning each sentence's trimmed text.
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        current.push(c);
        if matches!(c, '.' | '!' | '?') {
            // Sentence boundary if followed by whitespace/end (and not a decimal).
            let next_ws = chars.get(i + 1).map(|n| n.is_whitespace()).unwrap_or(true);
            if next_ws {
                let t = current.trim();
                if !t.is_empty() {
                    sentences.push(t.to_string());
                }
                current.clear();
            }
        }
    }
    let t = current.trim();
    if !t.is_empty() {
        sentences.push(t.to_string());
    }
    sentences
}

/// Content words of a sentence: lowercased alphanumeric tokens, minus stopwords.
fn words(sentence: &str) -> Vec<String> {
    sentence
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .filter(|w| !STOPWORDS.contains(&w.as_str()))
        .collect()
}

/// TextRank similarity between two word lists.
fn similarity(a: &[String], b: &[String]) -> f64 {
    if a.len() < 1 || b.len() < 1 {
        return 0.0;
    }
    let mut common = 0usize;
    for w in a {
        if b.contains(w) {
            common += 1;
        }
    }
    if common == 0 {
        return 0.0;
    }
    let denom = (a.len() as f64).ln() + (b.len() as f64).ln();
    if denom <= 0.0 {
        // Very short sentences (1 word each): fall back to raw overlap.
        common as f64
    } else {
        common as f64 / denom
    }
}

/// Produce an extractive summary of `text` with at most `max_sentences`
/// sentences, returned in original document order and joined by spaces.
pub fn summarize(text: &str, max_sentences: usize) -> String {
    let sentences = split_sentences(text);
    let n = sentences.len();
    let max_sentences = max_sentences.max(1);
    if n <= max_sentences {
        return sentences.join(" ");
    }

    let word_sets: Vec<Vec<String>> = sentences.iter().map(|s| words(s)).collect();

    // Weighted adjacency + row sums.
    let mut weight = vec![vec![0.0f64; n]; n];
    let mut out_sum = vec![0.0f64; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let s = similarity(&word_sets[i], &word_sets[j]);
            weight[i][j] = s;
            weight[j][i] = s;
        }
        out_sum[i] = weight[i].iter().sum();
    }

    // Weighted PageRank.
    let mut score = vec![1.0f64 / n as f64; n];
    for _ in 0..ITERATIONS {
        let mut next = vec![(1.0 - DAMPING) / n as f64; n];
        for i in 0..n {
            for j in 0..n {
                if i != j && weight[j][i] > 0.0 && out_sum[j] > 0.0 {
                    next[i] += DAMPING * (weight[j][i] / out_sum[j]) * score[j];
                }
            }
        }
        score = next;
    }

    // Pick the top `max_sentences` by score, then restore original order.
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| score[b].partial_cmp(&score[a]).unwrap_or(std::cmp::Ordering::Equal));
    let mut chosen: Vec<usize> = idx.into_iter().take(max_sentences).collect();
    chosen.sort_unstable();

    chosen
        .iter()
        .map(|&i| sentences[i].as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_sentences() {
        let s = split_sentences("Hello world. How are you? I am fine!");
        assert_eq!(s.len(), 3);
        assert_eq!(s[1], "How are you?");
    }

    #[test]
    fn short_text_returned_whole() {
        let text = "Only one sentence here.";
        assert_eq!(summarize(text, 3), text);
    }

    #[test]
    fn selects_subset_in_order() {
        let text = "Cats are great pets. Cats purr when happy. \
                    The stock market fell sharply today. \
                    Investors worried about the market and stocks. \
                    Cats love to nap in the sun.";
        let out = summarize(text, 2);
        // Returns at most 2 sentences, each from the original text.
        let count = out.matches(|c| matches!(c, '.' | '!' | '?')).count();
        assert!(count <= 2 && count >= 1);
        for piece in out.split_inclusive(['.', '!', '?']) {
            let p = piece.trim();
            if !p.is_empty() {
                assert!(text.contains(p), "summary sentence must come from the source: {p:?}");
            }
        }
    }

    #[test]
    fn preserves_original_order() {
        let text = "Alpha beta gamma delta. Beta gamma delta epsilon. \
                    Gamma delta epsilon zeta. Completely unrelated sentence about weather.";
        let out = summarize(text, 2);
        // The first chosen sentence should appear before the second in the source.
        let parts: Vec<&str> = out.split_inclusive(['.', '!', '?']).map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if parts.len() == 2 {
            let p0 = text.find(parts[0]).unwrap();
            let p1 = text.find(parts[1]).unwrap();
            assert!(p0 < p1, "summary must preserve original order");
        }
    }

    #[test]
    fn empty() {
        assert_eq!(summarize("", 3), "");
    }
}
