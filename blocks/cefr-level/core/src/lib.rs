//! cefr-level core — deterministic English CEFR difficulty heuristics.
//! Pure Rust, no model and no licensed CEFR wordlist.

const LEVELS: [&str; 6] = ["A1", "A2", "B1", "B2", "C1", "C2"];

#[derive(Clone, Debug)]
pub struct Options {
    pub output: String,
    pub target: String,
    pub coverage: u32,
    pub unknown: String,
    pub proper_nouns: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            output: "summary".to_string(),
            target: "B1".to_string(),
            coverage: 90,
            unknown: "estimate".to_string(),
            proper_nouns: false,
        }
    }
}

#[derive(Clone, Debug)]
struct WordInfo {
    token: String,
    lemma: String,
    level: Option<usize>,
    count: usize,
}

#[derive(Clone, Debug)]
struct Analysis {
    word_count: usize,
    sentence_count: usize,
    unique_count: usize,
    unknown_count: usize,
    avg_sentence_len: f64,
    avg_word_len: f64,
    vocab_level: usize,
    grammar_level: usize,
    overall_level: usize,
    sublevel: f64,
    target: usize,
    coverage: u32,
    over_target: usize,
    band_counts: [usize; 6],
    words: Vec<WordInfo>,
}

/// Backward-compatible default summary used by the scaffold and simple callers.
pub fn run(input: &str) -> Result<String, String> {
    run_with_options(input, &Options::default())
}

pub fn run_with_options(input: &str, options: &Options) -> Result<String, String> {
    let analysis = analyze(input, options)?;
    match options.output.trim().to_ascii_lowercase().as_str() {
        "summary" | "" => Ok(format_summary(&analysis)),
        "annotated" => Ok(format_annotated(&analysis)),
        "table" => Ok(format_table(&analysis)),
        "json" => Ok(format_json(&analysis)),
        other => Err(format!(
            "unknown output format '{other}' (use summary, annotated, table, or json)"
        )),
    }
}

fn analyze(input: &str, options: &Options) -> Result<Analysis, String> {
    let text = input.trim();
    if text.is_empty() {
        return Err("input text is required".to_string());
    }
    if text.chars().count() > 200_000 {
        return Err("input is too long (max 200,000 characters)".to_string());
    }
    let coverage = options.coverage.clamp(50, 100);
    let target = parse_level(&options.target).ok_or_else(|| {
        format!(
            "unknown target level '{}' (use A1, A2, B1, B2, C1, or C2)",
            options.target
        )
    })?;
    let unknown_mode = options.unknown.trim().to_ascii_lowercase();
    if !matches!(unknown_mode.as_str(), "estimate" | "c1" | "c2" | "exclude") {
        return Err(format!(
            "unknown unknown-word mode '{}' (use estimate, c1, c2, or exclude)",
            options.unknown
        ));
    }

    let raw_tokens = tokenize(text);
    if raw_tokens.is_empty() {
        return Err("input must contain at least one word".to_string());
    }

    let mut words: Vec<WordInfo> = Vec::new();
    let mut band_counts = [0usize; 6];
    let mut unknown_count = 0usize;
    let mut total_len = 0usize;
    let mut considered = 0usize;

    for raw in raw_tokens {
        let proper = is_probable_proper_noun(&raw);
        let lemma = normalize(&raw);
        if lemma.is_empty() {
            continue;
        }
        total_len += lemma.chars().count();
        let mut level = lookup_level(&lemma);
        let known = level.is_some();
        if level.is_none() && !(proper && !options.proper_nouns) {
            unknown_count += 1;
            level = match unknown_mode.as_str() {
                "estimate" => Some(estimate_level(&lemma)),
                "c1" => Some(4),
                "c2" => Some(5),
                "exclude" => None,
                _ => unreachable!(),
            };
        }
        if proper && !options.proper_nouns && !known {
            level = None;
        }
        if let Some(idx) = level {
            band_counts[idx] += 1;
            considered += 1;
        }
        if let Some(existing) = words
            .iter_mut()
            .find(|w| w.lemma == lemma && w.level == level)
        {
            existing.count += 1;
        } else {
            words.push(WordInfo {
                token: raw,
                lemma,
                level,
                count: 1,
            });
        }
    }

    if considered == 0 {
        return Err(
            "no words remained after exclusions; enable proper_nouns or change unknown handling"
                .to_string(),
        );
    }

    words.sort_by(|a, b| {
        b.level
            .unwrap_or(0)
            .cmp(&a.level.unwrap_or(0))
            .then_with(|| b.count.cmp(&a.count))
            .then_with(|| a.lemma.cmp(&b.lemma))
    });

    let cutoff = (considered as f64 * coverage as f64 / 100.0).ceil() as usize;
    let mut cumulative = 0usize;
    let mut vocab_level = 0usize;
    for (idx, count) in band_counts.iter().enumerate() {
        cumulative += count;
        if cumulative >= cutoff.max(1) {
            vocab_level = idx;
            break;
        }
    }

    let sentence_count = count_sentences(text).max(1);
    let word_count = total_count(&band_counts)
        + words
            .iter()
            .filter(|w| w.level.is_none())
            .map(|w| w.count)
            .sum::<usize>();
    let avg_sentence_len = word_count as f64 / sentence_count as f64;
    let avg_word_len = if word_count == 0 {
        0.0
    } else {
        total_len as f64 / word_count as f64
    };
    let grammar_level = grammar_level(text, avg_sentence_len, avg_word_len);
    let overall_raw = vocab_level as f64 * 0.65 + grammar_level as f64 * 0.35;
    let overall_level = overall_raw.round().clamp(0.0, 5.0) as usize;
    let sublevel = (overall_raw + 1.0).clamp(1.0, 6.0);
    let over_target = words
        .iter()
        .filter(|w| w.level.map(|l| l > target).unwrap_or(false))
        .map(|w| w.count)
        .sum();

    Ok(Analysis {
        word_count,
        sentence_count,
        unique_count: words.len(),
        unknown_count,
        avg_sentence_len,
        avg_word_len,
        vocab_level,
        grammar_level,
        overall_level,
        sublevel,
        target,
        coverage,
        over_target,
        band_counts,
        words,
    })
}

fn format_summary(a: &Analysis) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "CEFR estimate: {} ({:.1})\nVocabulary: {} at {}% coverage\nGrammar/sentence: {}\nWords: {} total, {} unique, {} sentence(s)\nAverage sentence length: {:.1} words; average word length: {:.1} letters\nTarget {}: {} word(s) above target\nUnknown words estimated: {}\n\nBand profile:\n",
        LEVELS[a.overall_level],
        a.sublevel,
        LEVELS[a.vocab_level],
        a.coverage,
        LEVELS[a.grammar_level],
        a.word_count,
        a.unique_count,
        a.sentence_count,
        a.avg_sentence_len,
        a.avg_word_len,
        LEVELS[a.target],
        a.over_target,
        a.unknown_count
    ));
    for (idx, count) in a.band_counts.iter().enumerate() {
        let pct = if a.word_count == 0 {
            0.0
        } else {
            *count as f64 * 100.0 / a.word_count as f64
        };
        out.push_str(&format!("{}: {} ({:.1}%)\n", LEVELS[idx], count, pct));
    }
    out
}

fn format_table(a: &Analysis) -> String {
    let mut out = String::from("count\tlevel\tword\tabove_target\n");
    for w in &a.words {
        let level = w.level.map(|i| LEVELS[i]).unwrap_or("excluded");
        let above = w.level.map(|i| i > a.target).unwrap_or(false);
        out.push_str(&format!("{}\t{}\t{}\t{}\n", w.count, level, w.lemma, above));
    }
    out
}

fn format_annotated(a: &Analysis) -> String {
    let mut out = format_summary(a);
    out.push_str("\nWord breakdown (hardest first):\n");
    for w in &a.words {
        let level = w.level.map(|i| LEVELS[i]).unwrap_or("excluded");
        let marker = if w.level.map(|i| i > a.target).unwrap_or(false) {
            " over target"
        } else {
            ""
        };
        out.push_str(&format!(
            "- {} ×{} — {}{}\n",
            w.lemma, w.count, level, marker
        ));
    }
    out
}

fn format_json(a: &Analysis) -> String {
    let profile = LEVELS
        .iter()
        .enumerate()
        .map(|(i, level)| format!("{{\"level\":\"{}\",\"count\":{}}}", level, a.band_counts[i]))
        .collect::<Vec<_>>()
        .join(",");
    let words = a
        .words
        .iter()
        .map(|w| {
            let level = w.level.map(|i| LEVELS[i]).unwrap_or("excluded");
            format!(
                "{{\"word\":\"{}\",\"example\":\"{}\",\"level\":\"{}\",\"count\":{},\"above_target\":{}}}",
                json_escape(&w.lemma),
                json_escape(&w.token),
                level,
                w.count,
                w.level.map(|i| i > a.target).unwrap_or(false)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"level\":\"{}\",\"sublevel\":{:.2},\"vocabulary_level\":\"{}\",\"grammar_level\":\"{}\",\"word_count\":{},\"sentence_count\":{},\"unique_words\":{},\"target\":\"{}\",\"over_target\":{},\"unknown_words\":{},\"profile\":[{}],\"words\":[{}]}}",
        LEVELS[a.overall_level],
        a.sublevel,
        LEVELS[a.vocab_level],
        LEVELS[a.grammar_level],
        a.word_count,
        a.sentence_count,
        a.unique_count,
        LEVELS[a.target],
        a.over_target,
        a.unknown_count,
        profile,
        words
    )
}

fn tokenize(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_alphabetic() || (ch == '\'' && !cur.is_empty()) || (ch == '-' && !cur.is_empty()) {
            cur.push(ch);
        } else if !cur.is_empty() {
            words.push(cur.trim_matches(['\'', '-']).to_string());
            cur.clear();
        }
    }
    if !cur.is_empty() {
        words.push(cur.trim_matches(['\'', '-']).to_string());
    }
    words.into_iter().filter(|w| !w.is_empty()).collect()
}

fn normalize(word: &str) -> String {
    let mut s = word
        .trim_matches(|c: char| !c.is_alphabetic() && c != '\'' && c != '-')
        .to_ascii_lowercase()
        .replace('’', "'");
    for (from, to) in [
        ("can't", "can"),
        ("won't", "will"),
        ("n't", ""),
        ("'re", ""),
        ("'ve", ""),
        ("'ll", ""),
        ("'d", ""),
        ("'s", ""),
    ] {
        if s.ends_with(from) {
            if from.starts_with('\'') || from == "n't" {
                s.truncate(s.len() - from.len());
            } else {
                s = to.to_string();
            }
        }
    }
    for suffix in ["ingly", "edly", "ing", "ed", "es", "s"] {
        if s.len() > suffix.len() + 3 && s.ends_with(suffix) {
            s.truncate(s.len() - suffix.len());
            break;
        }
    }
    s
}

fn lookup_level(word: &str) -> Option<usize> {
    word_level(word).or_else(|| word.split('-').filter_map(word_level).max())
}

fn word_level(word: &str) -> Option<usize> {
    let level = match word {
        "i" | "you" | "he" | "she" | "it" | "we" | "they" | "me" | "my" | "your" | "the" | "a"
        | "an" | "and" | "or" | "but" | "in" | "on" | "at" | "to" | "for" | "of" | "from"
        | "with" | "is" | "am" | "are" | "be" | "have" | "has" | "do" | "go" | "get" | "make"
        | "like" | "want" | "need" | "can" | "will" | "good" | "bad" | "big" | "small" | "new"
        | "old" | "day" | "time" | "home" | "school" | "work" | "food" | "water" | "book"
        | "family" | "friend" | "city" | "house" | "car" | "dog" | "cat" | "red" | "blue"
        | "green" | "one" | "two" | "three" => 0,
        "because" | "before" | "after" | "during" | "between" | "under" | "over" | "always"
        | "often" | "usually" | "sometimes" | "people" | "place" | "travel" | "holiday"
        | "weather" | "health" | "money" | "shop" | "story" | "music" | "movie" | "email"
        | "phone" | "question" | "answer" | "learn" | "teach" | "change" | "important"
        | "different" | "easy" | "difficult" | "happy" | "early" | "late" | "first" | "last" => 1,
        "although" | "however" | "therefore" | "while" | "instead" | "perhaps" | "probably"
        | "experience" | "education" | "environment" | "community" | "culture" | "technology"
        | "information" | "decision" | "support" | "improve" | "develop" | "explain"
        | "describe" | "compare" | "suggest" | "opinion" | "advantage" | "problem" | "solution"
        | "available" | "necessary" | "possible" | "regular" | "personal" => 2,
        "nevertheless" | "whereas" | "consequently" | "furthermore" | "significant"
        | "appropriate" | "efficient" | "complex" | "specific" | "evidence" | "approach"
        | "analysis" | "concept" | "context" | "factor" | "impact" | "policy" | "process"
        | "research" | "require" | "achieve" | "maintain" | "evaluate" | "indicate"
        | "interpret" | "participate" | "individual" | "professional" => 3,
        "notwithstanding" | "subsequently" | "predominantly" | "methodology" | "hypothesis"
        | "criterion" | "phenomenon" | "framework" | "implication" | "infrastructure"
        | "sustainability" | "comprehensive" | "considerable" | "controversial" | "fundamental"
        | "implement" | "constitute" | "differentiate" | "facilitate" | "substantiate"
        | "ambiguous" | "coherent" | "implicit" => 4,
        "epistemological" | "idiosyncratic" | "incommensurable" | "juxtaposition"
        | "metacognitive" | "paradigmatic" | "quintessential" | "recontextualise"
        | "socioeconomic" | "transcendental" | "ubiquitous" | "anachronistic" | "esoteric"
        | "perfunctory" | "proliferation" | "equivocation" => 5,
        _ => return None,
    };
    Some(level)
}

fn estimate_level(word: &str) -> usize {
    let len = word.chars().count();
    let syllables = estimate_syllables(word);
    let academic = [
        "tion", "sion", "ment", "ance", "ence", "ity", "ism", "ology", "ogical", "ative", "uous",
        "ious",
    ]
    .iter()
    .any(|s| word.ends_with(s));
    match (len, syllables, academic) {
        (0..=4, 0..=1, false) => 0,
        (0..=6, 0..=2, false) => 1,
        (0..=8, 0..=3, false) => 2,
        (0..=10, _, false) => 3,
        (0..=12, _, _) => 4,
        _ => 5,
    }
}

fn estimate_syllables(word: &str) -> usize {
    let mut count = 0usize;
    let mut prev_vowel = false;
    for ch in word.chars() {
        let vowel = matches!(ch, 'a' | 'e' | 'i' | 'o' | 'u' | 'y');
        if vowel && !prev_vowel {
            count += 1;
        }
        prev_vowel = vowel;
    }
    if word.ends_with('e') && count > 1 {
        count -= 1;
    }
    count.max(1)
}

fn is_probable_proper_noun(word: &str) -> bool {
    let mut chars = word.chars();
    matches!(chars.next(), Some(c) if c.is_uppercase())
        && chars.any(|c| c.is_lowercase())
        && !matches!(word, "I")
}

fn count_sentences(text: &str) -> usize {
    let count = text
        .chars()
        .filter(|c| matches!(c, '.' | '!' | '?'))
        .count();
    count.max(1)
}

fn grammar_level(text: &str, avg_sentence_len: f64, avg_word_len: f64) -> usize {
    let lower = text.to_ascii_lowercase();
    let connectors = [
        "although",
        "whereas",
        "therefore",
        "however",
        "nevertheless",
        "consequently",
        "furthermore",
        "despite",
        "unless",
    ]
    .iter()
    .filter(|needle| lower.contains(*needle))
    .count();
    let mut level = match avg_sentence_len {
        x if x <= 8.0 => 0,
        x if x <= 12.0 => 1,
        x if x <= 18.0 => 2,
        x if x <= 24.0 => 3,
        x if x <= 32.0 => 4,
        _ => 5,
    };
    if connectors >= 2 || avg_word_len >= 7.5 {
        level = (level + 1).min(5);
    }
    level
}

fn parse_level(level: &str) -> Option<usize> {
    match level.trim().to_ascii_uppercase().as_str() {
        "A1" => Some(0),
        "A2" => Some(1),
        "B1" => Some(2),
        "B2" => Some(3),
        "C1" => Some(4),
        "C2" => Some(5),
        _ => None,
    }
}

fn total_count(counts: &[usize; 6]) -> usize {
    counts.iter().sum()
}

fn json_escape(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            c => vec![c],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_text_is_a1_a2() {
        let out = run("I like my family. We go to school and read a book.").unwrap();
        assert!(out.contains("CEFR estimate: A1") || out.contains("CEFR estimate: A2"));
        assert!(out.contains("Band profile:"));
    }

    #[test]
    fn academic_text_rates_higher_and_flags_target() {
        let opts = Options {
            output: "table".into(),
            target: "B1".into(),
            coverage: 90,
            unknown: "estimate".into(),
            proper_nouns: true,
        };
        let out = run_with_options(
            "Nevertheless, the epistemological methodology has significant implications for sustainability.",
            &opts,
        )
        .unwrap();
        assert!(out.contains("epistemological"));
        assert!(out.contains("true"));
    }

    #[test]
    fn rejects_empty_input() {
        assert!(run("   ").unwrap_err().contains("required"));
    }

    #[test]
    fn json_output_is_structured() {
        let opts = Options {
            output: "json".into(),
            ..Options::default()
        };
        let out = run_with_options("This is a simple test.", &opts).unwrap();
        assert!(out.starts_with('{'));
        assert!(out.contains("\"level\""));
        assert!(out.contains("\"words\""));
    }
}
