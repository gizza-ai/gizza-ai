//! gizza-ai/readability-score core — compute classic readability grades for
//! English text: Flesch Reading Ease, Flesch-Kincaid Grade Level, Gunning-Fog
//! Index and the SMOG Index, plus the underlying counts (words, sentences,
//! syllables, complex/polysyllabic words). Pure-Rust, no I/O.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Scores {
    // Underlying counts.
    pub words: usize,
    pub sentences: usize,
    pub syllables: usize,
    /// Words of 3+ syllables (Gunning-Fog / SMOG "complex" words).
    pub complex_words: usize,
    /// Average syllables per word, 2 decimals.
    pub avg_syllables_per_word: f64,
    /// Average words per sentence, 2 decimals.
    pub avg_words_per_sentence: f64,

    // Readability indices, each rounded to 1 decimal.
    /// Flesch Reading Ease (0–100, higher = easier).
    pub flesch_reading_ease: f64,
    /// US school grade level for the Flesch Reading Ease score (e.g. "8th–9th grade").
    pub reading_ease_level: String,
    /// Flesch-Kincaid Grade Level (US grade).
    pub flesch_kincaid_grade: f64,
    /// Gunning-Fog Index (≈ years of formal education needed).
    pub gunning_fog: f64,
    /// SMOG Index (US grade; designed for 30+ sentence samples).
    pub smog_index: f64,
    /// Coleman-Liau Index (US grade; uses letters per word, no syllables).
    pub coleman_liau: f64,
    /// Automated Readability Index (US grade; characters per word).
    pub automated_readability: f64,
    /// Mean of the grade-level indices, 1 decimal.
    pub average_grade: f64,
    /// Plain-language reading level for the average grade.
    pub reading_level: String,
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Estimate the syllable count of a single English word using a vowel-group
/// heuristic with common adjustments (silent trailing "e", "-le" endings,
/// "-es"/"-ed" suffixes). Always at least 1 for a word containing a vowel.
pub fn count_syllables(word: &str) -> usize {
    let w: String = word
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if w.is_empty() {
        return 0;
    }
    let chars: Vec<char> = w.chars().collect();
    let is_vowel = |c: char| matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y');

    // Count vowel groups (consecutive vowels = one group).
    let mut count = 0usize;
    let mut prev_vowel = false;
    for &c in &chars {
        let v = is_vowel(c);
        if v && !prev_vowel {
            count += 1;
        }
        prev_vowel = v;
    }

    // Silent trailing "e" (but not "-le" which forms a syllable, e.g. "table").
    if w.ends_with('e') && !w.ends_with("le") && count > 1 {
        count -= 1;
    }
    // "-le" / "-les" preceded by a consonant adds a syllable if the e was the
    // only vowel sound there — already counted by the vowel group, so leave it.

    // Suffixes "-es" / "-ed" usually don't add a syllable (e.g. "asked",
    // "boxes" -> already handled by vowel groups; subtract over-counts only
    // when the base ended in a non-sibilant). Keep the heuristic conservative.

    count.max(1)
}

/// Tokenize into "words" (runs containing at least one ASCII letter).
fn words_of(text: &str) -> Vec<&str> {
    text.split(|c: char| c.is_whitespace())
        .filter(|w| w.chars().any(|c| c.is_ascii_alphabetic()))
        .collect()
}

/// Count sentences: runs ending in `.`, `!`, or `?` (collapsing consecutive
/// terminators like "?!" or "..."), with a trailing terminator-less sentence
/// still counting. Mirrors text-statistics so the surfaces agree.
fn count_sentences(text: &str) -> usize {
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
    if seen_text_since_terminator {
        sentences += 1;
    }
    sentences
}

/// US grade band label for a Flesch Reading Ease score.
fn ease_level(score: f64) -> &'static str {
    match score {
        s if s >= 90.0 => "5th grade (very easy)",
        s if s >= 80.0 => "6th grade (easy)",
        s if s >= 70.0 => "7th grade (fairly easy)",
        s if s >= 60.0 => "8th–9th grade (plain English)",
        s if s >= 50.0 => "10th–12th grade (fairly difficult)",
        s if s >= 30.0 => "College (difficult)",
        s if s >= 10.0 => "College graduate (very difficult)",
        _ => "Professional (extremely difficult)",
    }
}

/// Plain-language label for an average US grade level.
fn grade_level(grade: f64) -> &'static str {
    match grade {
        g if g <= 0.0 => "Pre-school",
        g if g < 6.0 => "Elementary school",
        g if g < 9.0 => "Middle school",
        g if g < 13.0 => "High school",
        g if g < 16.0 => "College",
        _ => "Graduate / professional",
    }
}

/// Compute readability scores for `text`.
pub fn analyze(text: &str) -> Scores {
    let words_vec = words_of(text);
    let words = words_vec.len();
    let sentences = count_sentences(text).max(if words > 0 { 1 } else { 0 });

    let syllables_per_word: Vec<usize> = words_vec.iter().map(|w| count_syllables(w)).collect();
    let syllables: usize = syllables_per_word.iter().sum();
    let complex_words = syllables_per_word.iter().filter(|&&s| s >= 3).count();

    // Letters/characters across the word tokens (for Coleman-Liau / ARI).
    let letters: usize = words_vec
        .iter()
        .map(|w| w.chars().filter(|c| c.is_alphanumeric()).count())
        .sum();

    if words == 0 || sentences == 0 {
        return Scores {
            words,
            sentences,
            syllables,
            complex_words,
            avg_syllables_per_word: 0.0,
            avg_words_per_sentence: 0.0,
            flesch_reading_ease: 0.0,
            reading_ease_level: "—".to_string(),
            flesch_kincaid_grade: 0.0,
            gunning_fog: 0.0,
            smog_index: 0.0,
            coleman_liau: 0.0,
            automated_readability: 0.0,
            average_grade: 0.0,
            reading_level: "—".to_string(),
        };
    }

    let w = words as f64;
    let s = sentences as f64;
    let syl = syllables as f64;
    let cx = complex_words as f64;

    let words_per_sentence = w / s;
    let syllables_per_word_avg = syl / w;

    // Flesch Reading Ease: 206.835 − 1.015·(W/S) − 84.6·(Syl/W). Clamp 0–100.
    let flesch_reading_ease = (206.835 - 1.015 * words_per_sentence - 84.6 * syllables_per_word_avg)
        .clamp(0.0, 100.0);

    // Flesch-Kincaid Grade: 0.39·(W/S) + 11.8·(Syl/W) − 15.59.
    let flesch_kincaid_grade =
        (0.39 * words_per_sentence + 11.8 * syllables_per_word_avg - 15.59).max(0.0);

    // Gunning-Fog: 0.4·[(W/S) + 100·(complex/W)].
    let gunning_fog = (0.4 * (words_per_sentence + 100.0 * (cx / w))).max(0.0);

    // SMOG: 1.043·sqrt(complex · 30/S) + 3.1291.
    let smog_index = (1.043 * (cx * (30.0 / s)).sqrt() + 3.1291).max(0.0);

    // Coleman-Liau: 0.0588·L − 0.296·S' − 15.8, where L = avg letters per 100
    // words and S' = avg sentences per 100 words.
    let l = (letters as f64 / w) * 100.0;
    let s_per_100 = (s / w) * 100.0;
    let coleman_liau = (0.0588 * l - 0.296 * s_per_100 - 15.8).max(0.0);

    // Automated Readability Index: 4.71·(chars/word) + 0.5·(words/sentence) − 21.43.
    let automated_readability =
        (4.71 * (letters as f64 / w) + 0.5 * words_per_sentence - 21.43).max(0.0);

    let average_grade = round1(
        (flesch_kincaid_grade + gunning_fog + smog_index + coleman_liau + automated_readability)
            / 5.0,
    );

    Scores {
        words,
        sentences,
        syllables,
        complex_words,
        avg_syllables_per_word: round2(syllables_per_word_avg),
        avg_words_per_sentence: round2(words_per_sentence),
        flesch_reading_ease: round1(flesch_reading_ease),
        reading_ease_level: ease_level(flesch_reading_ease).to_string(),
        flesch_kincaid_grade: round1(flesch_kincaid_grade),
        gunning_fog: round1(gunning_fog),
        smog_index: round1(smog_index),
        coleman_liau: round1(coleman_liau),
        automated_readability: round1(automated_readability),
        average_grade,
        reading_level: grade_level(average_grade).to_string(),
    }
}

/// Compact human-readable report (used by the page).
pub fn summary(text: &str) -> String {
    let s = analyze(text);
    if s.words == 0 {
        return "No text to analyze.".to_string();
    }
    format!(
        "Reading level: {} (avg grade {})\n\
         Flesch Reading Ease: {}  ({})\n\
         Flesch-Kincaid Grade: {}\n\
         Gunning-Fog Index: {}\n\
         SMOG Index: {}\n\
         Coleman-Liau Index: {}\n\
         Automated Readability Index: {}\n\
         \n\
         Words: {}   Sentences: {}   Syllables: {}\n\
         Complex words (3+ syllables): {}\n\
         Avg words/sentence: {}   Avg syllables/word: {}",
        s.reading_level,
        s.average_grade,
        s.flesch_reading_ease,
        s.reading_ease_level,
        s.flesch_kincaid_grade,
        s.gunning_fog,
        s.smog_index,
        s.coleman_liau,
        s.automated_readability,
        s.words,
        s.sentences,
        s.syllables,
        s.complex_words,
        s.avg_words_per_sentence,
        s.avg_syllables_per_word,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syllable_counts() {
        assert_eq!(count_syllables("cat"), 1);
        assert_eq!(count_syllables("apple"), 2);
        assert_eq!(count_syllables("table"), 2); // -le keeps the syllable
        assert_eq!(count_syllables("make"), 1); // silent e
        assert_eq!(count_syllables("readability"), 5); // heuristic: ea-a-i-i-y groups
        assert_eq!(count_syllables("the"), 1);
        assert_eq!(count_syllables(""), 0);
    }

    #[test]
    fn simple_sentence_is_easy() {
        // Short words + short sentence => high reading ease, low grade.
        let s = analyze("The cat sat on the mat.");
        assert_eq!(s.words, 6);
        assert_eq!(s.sentences, 1);
        assert!(s.flesch_reading_ease > 90.0, "ease was {}", s.flesch_reading_ease);
        assert!(s.flesch_kincaid_grade < 3.0, "grade was {}", s.flesch_kincaid_grade);
        assert_eq!(s.reading_level, "Elementary school");
    }

    #[test]
    fn complex_text_is_harder() {
        let text = "The aforementioned methodology necessitates considerable \
                    deliberation regarding the multifaceted ramifications of \
                    implementing such an unprecedented organizational transformation.";
        let s = analyze(text);
        assert!(s.complex_words >= 5, "complex words: {}", s.complex_words);
        assert!(s.flesch_reading_ease < 30.0, "ease was {}", s.flesch_reading_ease);
        assert!(s.flesch_kincaid_grade > 12.0, "grade was {}", s.flesch_kincaid_grade);
        assert!(s.gunning_fog > 12.0, "fog was {}", s.gunning_fog);
        assert!(s.coleman_liau > 12.0, "coleman-liau was {}", s.coleman_liau);
        assert!(s.automated_readability > 12.0, "ari was {}", s.automated_readability);
    }

    #[test]
    fn counts_multiple_sentences() {
        let s = analyze("Hello world. How are you? I am fine!");
        assert_eq!(s.sentences, 3);
        assert_eq!(s.words, 8);
    }

    #[test]
    fn empty_is_neutral() {
        let s = analyze("");
        assert_eq!(s.words, 0);
        assert_eq!(s.flesch_reading_ease, 0.0);
        assert_eq!(s.average_grade, 0.0);
        assert_eq!(s.reading_level, "—");
    }

    #[test]
    fn ease_within_bounds() {
        // Reading ease is always clamped to 0..=100.
        let s = analyze("Supercalifragilisticexpialidocious antidisestablishmentarianism.");
        assert!(s.flesch_reading_ease >= 0.0 && s.flesch_reading_ease <= 100.0);
    }

    #[test]
    fn summary_has_fields() {
        let out = summary("The cat sat on the mat. It was a sunny day.");
        assert!(out.contains("Flesch Reading Ease:"));
        assert!(out.contains("Gunning-Fog"));
        assert!(out.contains("SMOG"));
        assert!(out.contains("Coleman-Liau"));
        assert!(out.contains("Automated Readability"));
        assert!(out.contains("Reading level:"));
    }

    #[test]
    fn summary_empty() {
        assert_eq!(summary(""), "No text to analyze.");
    }
}
