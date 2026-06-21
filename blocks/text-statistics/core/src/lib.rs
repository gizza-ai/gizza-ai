//! gizza-ai/text-statistics core — count words, characters, sentences,
//! paragraphs and lines in text, plus reading/speaking time and averages.
//! Pure-Rust, dependency-free (besides serde for the output struct).

use serde::Serialize;

/// Words read per minute (average silent reading).
const READING_WPM: f64 = 200.0;
/// Words spoken per minute (average speaking pace).
const SPEAKING_WPM: f64 = 130.0;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stats {
    pub characters: usize,
    pub characters_no_spaces: usize,
    pub words: usize,
    pub sentences: usize,
    pub paragraphs: usize,
    pub lines: usize,
    /// Estimated silent reading time in minutes (at 200 wpm), 1 decimal.
    pub reading_time_min: f64,
    /// Estimated speaking time in minutes (at 130 wpm), 1 decimal.
    pub speaking_time_min: f64,
    /// Average word length in characters, 1 decimal.
    pub avg_word_length: f64,
    /// Average words per sentence, 1 decimal.
    pub avg_words_per_sentence: f64,
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Analyze `text` and return its statistics.
pub fn analyze(text: &str) -> Stats {
    let characters = text.chars().count();
    let characters_no_spaces = text.chars().filter(|c| !c.is_whitespace()).count();

    let words_vec: Vec<&str> = text.split_whitespace().collect();
    let words = words_vec.len();

    // Sentences: runs ending in . ! ? (collapsing consecutive terminators like
    // "?!" or "..."). A trailing sentence without a terminator still counts.
    let mut sentences = 0usize;
    let mut prev_was_terminator = false;
    let mut seen_text_since_terminator = false;
    for c in text.chars() {
        if matches!(c, '.' | '!' | '?') {
            if !prev_was_terminator && seen_text_since_terminator {
                sentences += 1;
            }
            prev_was_terminator = true;
            seen_text_since_terminator = false;
        } else {
            if !c.is_whitespace() {
                seen_text_since_terminator = true;
            }
            prev_was_terminator = false;
        }
    }
    // Count a final terminator-less sentence (e.g. "hello world").
    if seen_text_since_terminator {
        sentences += 1;
    }

    // Paragraphs: blocks separated by one or more blank lines.
    let paragraphs = text
        .split("\n\n")
        .map(|p| p.replace(['\r', '\n', ' ', '\t'], ""))
        .filter(|p| !p.is_empty())
        .count();

    // Lines: number of newline-separated lines that contain content, but at
    // least 1 if there is any text.
    let lines = if text.is_empty() {
        0
    } else {
        text.lines().count().max(1)
    };

    // Average word length counts letters/digits only (ignores attached
    // punctuation like a trailing period), so it reflects actual word size.
    let total_word_chars: usize = words_vec
        .iter()
        .map(|w| w.chars().filter(|c| c.is_alphanumeric()).count())
        .sum();
    let avg_word_length = if words > 0 {
        round1(total_word_chars as f64 / words as f64)
    } else {
        0.0
    };
    let avg_words_per_sentence = if sentences > 0 {
        round1(words as f64 / sentences as f64)
    } else {
        0.0
    };

    Stats {
        characters,
        characters_no_spaces,
        words,
        sentences,
        paragraphs,
        lines,
        reading_time_min: round1(words as f64 / READING_WPM),
        speaking_time_min: round1(words as f64 / SPEAKING_WPM),
        avg_word_length,
        avg_words_per_sentence,
    }
}

/// A compact human-readable summary (used by the page).
pub fn summary(text: &str) -> String {
    let s = analyze(text);
    format!(
        "Words: {}\nCharacters: {} ({} without spaces)\nSentences: {}\nParagraphs: {}\nLines: {}\nReading time: {} min\nSpeaking time: {} min\nAvg word length: {} chars\nAvg words/sentence: {}",
        s.words,
        s.characters,
        s.characters_no_spaces,
        s.sentences,
        s.paragraphs,
        s.lines,
        s.reading_time_min,
        s.speaking_time_min,
        s.avg_word_length,
        s.avg_words_per_sentence,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_counts() {
        let s = analyze("Hello world. How are you?");
        assert_eq!(s.words, 5);
        assert_eq!(s.sentences, 2);
        assert_eq!(s.characters, 25);
        assert_eq!(s.characters_no_spaces, 21);
    }

    #[test]
    fn paragraphs_and_lines() {
        let text = "First para line one.\nFirst para line two.\n\nSecond para.";
        let s = analyze(text);
        assert_eq!(s.paragraphs, 2);
        assert_eq!(s.lines, 4); // 4 lines incl. the blank separator line
        assert_eq!(s.sentences, 3);
    }

    #[test]
    fn collapses_repeated_terminators() {
        let s = analyze("Really?! Yes... ok");
        // "Really" (?!), "Yes" (...), "ok" (no terminator) = 3 sentences
        assert_eq!(s.sentences, 3);
        assert_eq!(s.words, 3); // "Really?!" "Yes..." "ok"
    }

    #[test]
    fn reading_time() {
        // 400 words -> 2.0 min reading at 200 wpm.
        let text = "word ".repeat(400);
        let s = analyze(&text);
        assert_eq!(s.words, 400);
        assert_eq!(s.reading_time_min, 2.0);
    }

    #[test]
    fn averages() {
        let s = analyze("aa bbbb. cc");
        // words: aa(2) bbbb(4) cc(2) = 8 chars / 3 words = 2.7
        assert_eq!(s.words, 3);
        assert_eq!(s.avg_word_length, 2.7);
        // 2 sentences ("aa bbbb." and "cc") -> 3 words / 2 = 1.5
        assert_eq!(s.sentences, 2);
        assert_eq!(s.avg_words_per_sentence, 1.5);
    }

    #[test]
    fn empty() {
        let s = analyze("");
        assert_eq!(s.words, 0);
        assert_eq!(s.sentences, 0);
        assert_eq!(s.paragraphs, 0);
        assert_eq!(s.lines, 0);
        assert_eq!(s.reading_time_min, 0.0);
    }

    #[test]
    fn summary_has_fields() {
        let out = summary("Hello world.");
        assert!(out.contains("Words: 2"));
        assert!(out.contains("Reading time:"));
    }
}
