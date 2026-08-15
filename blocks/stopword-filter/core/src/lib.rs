//! gizza-ai/stopword-filter core — remove stop words from text using built-in
//! multilingual stop lists, a custom list, or both. Pure Rust, dependency-free
//! besides serde.
//!
//! Matching is whole-word and tokenizer-based (never substring), so `the` never
//! touches the `the` inside `theatre`, and contractions such as `don't` stay a
//! single token. Punctuation and line breaks are preserved by default; the
//! whitespace a removed word leaves behind is repaired so the result reads like
//! prose rather than `cat , sat .`.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

/// Maximum accepted input length, in characters.
pub const MAX_CHARS: usize = 200_000;

/// Language codes with a built-in stop list, plus `none`.
pub const LANGUAGES: &[&str] = &[
    "english",
    "spanish",
    "french",
    "german",
    "italian",
    "portuguese",
    "dutch",
    "russian",
    "none",
];

/// Output views a text surface (page / CLI) can ask for.
pub const OUTPUTS: &[&str] = &["text", "removed", "stats"];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// The rendered view selected by `output`.
    pub result: String,
    /// Word tokens found in the input.
    pub total_words: usize,
    /// Word tokens removed as stop words.
    pub removed_words: usize,
    /// Word tokens kept.
    pub kept_words: usize,
    /// Removed share of all words, percent rounded to 2 decimals.
    pub removed_percent: f64,
    /// Distinct stop words that were removed at least once.
    pub distinct_removed: usize,
}

const ENGLISH: &[&str] = &[
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
    "there", "there's", "these", "they", "they'd", "they'll", "they're", "they've", "this",
    "those", "through", "to", "too", "under", "until", "up", "very", "was", "wasn't", "we", "we'd",
    "we'll", "we're", "we've", "were", "weren't", "what", "what's", "when", "when's", "where",
    "where's", "which", "while", "who", "who's", "whom", "why", "why's", "will", "with", "won't",
    "would", "wouldn't", "you", "you'd", "you'll", "you're", "you've", "your", "yours", "yourself",
    "yourselves",
];

const SPANISH: &[&str] = &[
    "a", "al", "algo", "algunas", "algunos", "ante", "antes", "aunque", "como", "con", "contra",
    "cual", "cuando", "de", "del", "desde", "donde", "dos", "e", "el", "ella", "ellas", "ellos",
    "en", "entre", "era", "eran", "eres", "es", "esa", "esas", "ese", "eso", "esos", "esta",
    "estaba", "estar", "este", "esto", "estos", "fue", "fueron", "ha", "habia", "han", "hasta",
    "hay", "la", "las", "le", "les", "lo", "los", "mas", "más", "me", "mi", "mis", "mucho", "muy",
    "nada", "ni", "no", "nos", "nosotros", "nuestra", "nuestro", "o", "os", "otra", "otro", "para",
    "pero", "poco", "por", "porque", "que", "qué", "quien", "se", "sea", "ser", "si", "sí", "sin",
    "sobre", "son", "su", "sus", "también", "tanto", "te", "tener", "tiene", "tienen", "todo",
    "todos", "tu", "tus", "un", "una", "uno", "unos", "y", "ya", "yo", "él",
];

const FRENCH: &[&str] = &[
    "a", "afin", "ainsi", "alors", "au", "aussi", "autre", "aux", "avec", "avoir", "car", "ce",
    "cela", "ces", "cet", "cette", "chaque", "comme", "dans", "de", "des", "donc", "dont", "du",
    "elle", "elles", "en", "encore", "est", "et", "eux", "être", "été", "fait", "il", "ils", "je",
    "la", "le", "les", "leur", "leurs", "lui", "ma", "mais", "me", "mes", "moi", "mon", "même",
    "ne", "ni", "nos", "notre", "nous", "on", "ou", "où", "par", "pas", "peu", "plus", "pour",
    "près", "qu", "quand", "que", "qui", "quoi", "sa", "sans", "se", "ses", "si", "son", "sont",
    "sous", "sur", "ta", "te", "tes", "toi", "ton", "tous", "tout", "très", "tu", "un", "une",
    "vos", "votre", "vous", "y", "à",
];

const GERMAN: &[&str] = &[
    "aber", "alle", "als", "also", "am", "an", "auch", "auf", "aus", "bei", "bin", "bis", "bist",
    "da", "damit", "dann", "das", "dass", "dein", "deine", "dem", "den", "denn", "der", "des",
    "dessen", "die", "dies", "diese", "diesem", "diesen", "dieser", "dieses", "doch", "dort", "du",
    "durch", "ein", "eine", "einem", "einen", "einer", "eines", "er", "es", "euer", "eure", "für",
    "hat", "hatte", "hatten", "hier", "ich", "ihr", "ihre", "im", "in", "ist", "ja", "jede",
    "jeder", "kann", "kein", "keine", "mein", "meine", "mit", "muss", "nach", "nicht", "noch",
    "nun", "nur", "ob", "oder", "ohne", "sehr", "sein", "seine", "sich", "sie", "sind", "so",
    "soll", "über", "um", "und", "uns", "unser", "unter", "vom", "von", "vor", "war", "waren",
    "was", "weil", "wenn", "wer", "werden", "wie", "wieder", "wir", "wird", "wo", "zu", "zum",
    "zur", "zwar", "zwischen",
];

const ITALIAN: &[&str] = &[
    "a", "ad", "agli", "ai", "al", "alla", "alle", "allo", "anche", "che", "chi", "ci", "come",
    "con", "cui", "da", "dal", "dalla", "degli", "dei", "del", "della", "delle", "dello", "di",
    "dove", "e", "ed", "gli", "ha", "hanno", "ho", "i", "il", "in", "io", "la", "le", "lei", "li",
    "lo", "loro", "ma", "me", "mi", "ne", "negli", "nei", "nel", "nella", "nelle", "no", "noi",
    "non", "o", "per", "perché", "però", "più", "quale", "quando", "quello", "questa", "queste",
    "questi", "questo", "se", "sei", "si", "sia", "siamo", "sono", "sta", "su", "sui", "sul",
    "sulla", "te", "ti", "tra", "tu", "tuo", "tutti", "tutto", "un", "una", "uno", "voi", "vostro",
    "è",
];

const PORTUGUESE: &[&str] = &[
    "a", "ao", "aos", "as", "com", "como", "da", "das", "de", "dela", "dele", "deles", "do", "dos",
    "e", "ela", "elas", "ele", "eles", "em", "entre", "era", "eram", "essa", "esse", "esta",
    "este", "eu", "foi", "foram", "há", "isso", "isto", "já", "lhe", "lhes", "mais", "mas", "me",
    "mesmo", "meu", "minha", "muito", "na", "nas", "nem", "no", "nos", "nós", "num", "numa", "não",
    "o", "os", "ou", "para", "pela", "pelo", "pelos", "por", "qual", "quando", "que", "quem", "se",
    "sem", "ser", "seu", "seus", "sua", "suas", "só", "também", "te", "tem", "ter", "teu", "tu",
    "um", "uma", "você", "vocês", "à", "às",
];

const DUTCH: &[&str] = &[
    "aan", "al", "alles", "als", "altijd", "andere", "ben", "bij", "daar", "dan", "dat", "de",
    "der", "deze", "die", "dit", "doch", "doen", "door", "dus", "een", "eens", "en", "er", "geen",
    "geweest", "haar", "had", "heb", "hebben", "heeft", "hem", "het", "hier", "hij", "hoe", "hun",
    "iemand", "iets", "ik", "in", "is", "ja", "je", "kan", "kon", "kunnen", "maar", "me", "meer",
    "men", "met", "mij", "mijn", "moet", "na", "naar", "niet", "niets", "nog", "nu", "of", "om",
    "omdat", "ons", "ook", "op", "over", "reeds", "te", "tegen", "toch", "toen", "tot", "u", "uit",
    "uw", "van", "veel", "voor", "want", "waren", "was", "wat", "we", "werd", "wezen", "wie",
    "wij", "wil", "worden", "wordt", "zal", "ze", "zei", "zelf", "zich", "zij", "zijn", "zo",
    "zonder", "zou",
];

const RUSSIAN: &[&str] = &[
    "а", "без", "более", "бы", "был", "была", "были", "было", "быть", "в", "вам", "вас", "весь",
    "во", "вот", "все", "всего", "всех", "вы", "где", "да", "даже", "для", "до", "его", "ее",
    "если", "есть", "еще", "же", "за", "здесь", "и", "из", "или", "им", "их", "к", "как", "ко",
    "когда", "кто", "ли", "либо", "мне", "может", "мы", "на", "надо", "наш", "не", "него", "нее",
    "нет", "ни", "них", "но", "ну", "о", "об", "однако", "он", "она", "они", "оно", "от", "очень",
    "по", "под", "при", "с", "со", "так", "также", "такой", "там", "те", "тем", "то", "того",
    "тоже", "той", "только", "том", "ты", "у", "уже", "хотя", "чего", "чей", "чем", "что", "чтобы",
    "эта", "эти", "это", "я",
];

/// The built-in stop list for `language`, or `Err` for an unknown code.
pub fn stop_list(language: &str) -> Result<&'static [&'static str], String> {
    match language.trim().to_lowercase().as_str() {
        "none" | "" => Ok(&[]),
        "english" | "en" => Ok(ENGLISH),
        "spanish" | "es" => Ok(SPANISH),
        "french" | "fr" => Ok(FRENCH),
        "german" | "de" => Ok(GERMAN),
        "italian" | "it" => Ok(ITALIAN),
        "portuguese" | "pt" => Ok(PORTUGUESE),
        "dutch" | "nl" => Ok(DUTCH),
        "russian" | "ru" => Ok(RUSSIAN),
        other => Err(format!(
            "unknown language '{other}' — use one of: {}",
            LANGUAGES.join(", ")
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seg<'a> {
    Word(&'a str),
    Sep(&'a str),
}

/// Split text into alternating word / separator segments. A word is a run of
/// alphanumerics with interior apostrophes (straight `'` or curly `’`) kept, so
/// `don't` is one token; everything else is a separator.
fn segment(text: &str) -> Vec<Seg<'_>> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut word_start: Option<usize> = None;
    let mut word_end = 0usize;
    let mut sep_start: Option<usize> = None;

    for (idx, (i, c)) in chars.iter().enumerate() {
        let is_apos = *c == '\'' || *c == '\u{2019}';
        let next_is_word = chars
            .get(idx + 1)
            .map(|(_, nc)| nc.is_alphanumeric())
            .unwrap_or(false);
        let in_word = c.is_alphanumeric() || (is_apos && word_start.is_some() && next_is_word);
        if in_word {
            if let Some(s) = sep_start.take() {
                out.push(Seg::Sep(&text[s..*i]));
            }
            if word_start.is_none() {
                word_start = Some(*i);
            }
            word_end = i + c.len_utf8();
        } else {
            if let Some(s) = word_start.take() {
                out.push(Seg::Word(&text[s..word_end]));
            }
            if sep_start.is_none() {
                sep_start = Some(*i);
            }
        }
    }
    if let Some(s) = word_start {
        out.push(Seg::Word(&text[s..word_end]));
    }
    if let Some(s) = sep_start {
        out.push(Seg::Sep(&text[s..]));
    }
    out
}

/// Split a user-supplied word list on commas, semicolons, and whitespace.
fn split_list(list: &str) -> Vec<&str> {
    list.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .map(|w| w.trim())
        .filter(|w| !w.is_empty())
        .collect()
}

fn key_of(word: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        word.to_string()
    } else {
        word.to_lowercase()
    }
}

/// Tidy the spacing left behind after words were pulled out: collapse runs of
/// spaces/tabs, drop a space stranded before closing punctuation, and trim
/// trailing spaces at the end of every line.
fn tidy(s: &str) -> String {
    let mut collapsed = String::with_capacity(s.len());
    let mut last_space = false;
    for c in s.chars() {
        if c == ' ' || c == '\t' {
            if !last_space {
                collapsed.push(' ');
            }
            last_space = true;
        } else {
            collapsed.push(c);
            last_space = false;
        }
    }

    // Remove a space directly before closing punctuation ("cat ." -> "cat.").
    let mut out = String::with_capacity(collapsed.len());
    let chars: Vec<char> = collapsed.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if *c == ' ' {
            if let Some(next) = chars.get(i + 1) {
                if matches!(
                    next,
                    ',' | '.' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '%' | '…' | '»' | '”'
                ) {
                    continue;
                }
            }
        }
        out.push(*c);
    }

    out.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Remove stop words from `text`.
///
/// - `language` — built-in list to use (`none` = custom list only).
/// - `custom_words` — extra words to remove (comma/semicolon/whitespace separated).
/// - `keep_words` — words that are never removed, even if a list contains them.
/// - `case_sensitive` — when true, only exact-case matches are removed.
/// - `remove_punctuation` — when true, punctuation is dropped as well.
/// - `output` — `text` (cleaned text), `removed` (the removed words + counts),
///   or `stats` (a count summary).
pub fn filter(
    text: &str,
    language: &str,
    custom_words: &str,
    keep_words: &str,
    case_sensitive: bool,
    remove_punctuation: bool,
    output: &str,
) -> Result<Report, String> {
    if text.chars().count() > MAX_CHARS {
        return Err(format!(
            "text is too long ({} characters) — the limit is {MAX_CHARS} characters",
            text.chars().count()
        ));
    }
    let mode = output.trim().to_lowercase();
    let mode = if mode.is_empty() { "text".to_string() } else { mode };
    if !OUTPUTS.contains(&mode.as_str()) {
        return Err(format!(
            "unknown output '{mode}' — use one of: {}",
            OUTPUTS.join(", ")
        ));
    }

    let builtin = stop_list(language)?;
    let mut stop: HashSet<String> = builtin.iter().map(|w| key_of(w, case_sensitive)).collect();
    for w in split_list(custom_words) {
        stop.insert(key_of(w, case_sensitive));
    }
    let keep: HashSet<String> = split_list(keep_words)
        .into_iter()
        .map(|w| key_of(w, case_sensitive))
        .collect();

    let mut total = 0usize;
    let mut removed = 0usize;
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut display: HashMap<String, String> = HashMap::new();

    let mut cleaned = String::with_capacity(text.len());
    let mut pending_gap = false;

    for seg in segment(text) {
        match seg {
            Seg::Word(w) => {
                total += 1;
                let key = key_of(w, case_sensitive);
                if !keep.contains(&key) && stop.contains(&key) {
                    removed += 1;
                    let entry = counts.entry(key.clone()).or_insert_with(|| {
                        order.push(key.clone());
                        display.insert(key.clone(), w.to_string());
                        0
                    });
                    *entry += 1;
                    pending_gap = true;
                } else {
                    cleaned.push_str(w);
                    pending_gap = false;
                }
            }
            Seg::Sep(s) => {
                let piece: String = if remove_punctuation {
                    s.chars().filter(|c| c.is_whitespace()).collect()
                } else {
                    s.to_string()
                };
                // Swallow the single space a removed word left behind, but never
                // a line break.
                let swallow = pending_gap
                    && !piece.is_empty()
                    && piece.chars().all(|c| c == ' ' || c == '\t');
                if !swallow {
                    cleaned.push_str(&piece);
                }
                pending_gap = false;
            }
        }
    }

    let kept = total - removed;
    let removed_percent = if total == 0 {
        0.0
    } else {
        ((removed as f64 / total as f64) * 10_000.0).round() / 100.0
    };

    let result = match mode.as_str() {
        "text" => tidy(&cleaned),
        "removed" => {
            if order.is_empty() {
                "(no stop words removed)".to_string()
            } else {
                let mut idx: Vec<(usize, &String)> = order.iter().enumerate().collect();
                idx.sort_by(|a, b| counts[b.1].cmp(&counts[a.1]).then(a.0.cmp(&b.0)));
                idx.into_iter()
                    .map(|(_, k)| format!("{}\t{}", counts[k], display[k]))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        _ => format!(
            "Total words: {total}\nRemoved: {removed} ({removed_percent:.2}%)\nKept: {kept}\nDistinct stop words removed: {}",
            order.len()
        ),
    };

    Ok(Report {
        result,
        total_words: total,
        removed_words: removed,
        kept_words: kept,
        removed_percent,
        distinct_removed: order.len(),
    })
}

/// Convenience wrapper for the page: returns just the rendered view.
#[allow(clippy::too_many_arguments)]
pub fn filter_text(
    text: &str,
    language: &str,
    custom_words: &str,
    keep_words: &str,
    case_sensitive: bool,
    remove_punctuation: bool,
    output: &str,
) -> Result<String, String> {
    filter(
        text,
        language,
        custom_words,
        keep_words,
        case_sensitive,
        remove_punctuation,
        output,
    )
    .map(|r| r.result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_english_stop_words_and_repairs_spacing() {
        let r = filter(
            "This is a test of the emergency broadcast system.",
            "english",
            "",
            "",
            false,
            false,
            "text",
        )
        .unwrap();
        assert_eq!(r.result, "test emergency broadcast system.");
        assert_eq!(r.total_words, 9);
        assert_eq!(r.removed_words, 5);
        assert_eq!(r.kept_words, 4);
        assert_eq!(r.removed_percent, 55.56);
        assert_eq!(r.distinct_removed, 5);
    }

    #[test]
    fn unknown_language_is_an_error() {
        let e = filter("hello", "klingon", "", "", false, false, "text").unwrap_err();
        assert!(e.contains("unknown language 'klingon'"), "{e}");
    }

    #[test]
    fn unknown_output_is_an_error() {
        let e = filter("hello", "english", "", "", false, false, "csv").unwrap_err();
        assert!(e.contains("unknown output 'csv'"), "{e}");
    }

    #[test]
    fn over_the_cap_is_an_error() {
        let big = "a ".repeat(MAX_CHARS); // 2 * MAX_CHARS chars
        let e = filter(&big, "english", "", "", false, false, "text").unwrap_err();
        assert!(e.contains("text is too long"), "{e}");
    }

    #[test]
    fn cap_boundary_is_accepted() {
        let exact: String = "a".repeat(MAX_CHARS);
        let r = filter(&exact, "none", "", "", false, false, "stats").unwrap();
        assert_eq!(r.total_words, 1);
    }

    #[test]
    fn whole_word_matching_only() {
        let r = filter("the theatre in Athens", "english", "", "", false, false, "text").unwrap();
        assert_eq!(r.result, "theatre Athens");
    }

    #[test]
    fn contractions_stay_one_token() {
        let r = filter("I don't think so", "english", "", "", false, false, "text").unwrap();
        assert_eq!(r.result, "think");
        assert_eq!(r.total_words, 4);
    }

    #[test]
    fn custom_words_extend_the_list() {
        let r = filter(
            "buy cheap widgets now",
            "none",
            "cheap, now",
            "",
            false,
            false,
            "text",
        )
        .unwrap();
        assert_eq!(r.result, "buy widgets");
    }

    #[test]
    fn keep_words_override_the_stop_list() {
        let r = filter("to be or not to be", "english", "", "not", false, false, "text").unwrap();
        assert_eq!(r.result, "not");
    }

    #[test]
    fn case_sensitive_matches_exact_casing_only() {
        let insensitive =
            filter("The Cat and THE dog", "english", "", "", false, false, "text").unwrap();
        assert_eq!(insensitive.result, "Cat dog");
        let sensitive =
            filter("The Cat and THE dog", "english", "", "", true, false, "text").unwrap();
        assert_eq!(sensitive.result, "The Cat THE dog");
    }

    #[test]
    fn remove_punctuation_drops_symbols_but_keeps_lines() {
        let r = filter(
            "Hello, world!\nThe cat, it sat.",
            "english",
            "",
            "",
            false,
            true,
            "text",
        )
        .unwrap();
        // Line break preserved, punctuation gone.
        assert_eq!(r.result, "Hello world\ncat sat");
    }

    #[test]
    fn spanish_list_works() {
        let r = filter(
            "el gato y el perro de la casa",
            "spanish",
            "",
            "",
            false,
            false,
            "text",
        )
        .unwrap();
        assert_eq!(r.result, "gato perro casa");
    }

    #[test]
    fn every_advertised_language_loads() {
        for lang in LANGUAGES {
            let r = filter("hello world", lang, "", "", false, false, "text").unwrap();
            assert_eq!(r.result, "hello world", "{lang}");
        }
    }

    #[test]
    fn removed_view_ranks_by_count() {
        let r = filter(
            "the cat and the dog and the bird",
            "english",
            "",
            "",
            false,
            false,
            "removed",
        )
        .unwrap();
        assert_eq!(r.result, "3\tthe\n2\tand");
    }

    #[test]
    fn removed_view_when_nothing_matched() {
        let r = filter("cat dog", "english", "", "", false, false, "removed").unwrap();
        assert_eq!(r.result, "(no stop words removed)");
    }

    #[test]
    fn stats_view_summarises() {
        let r = filter("the cat sat on the mat", "english", "", "", false, false, "stats").unwrap();
        assert_eq!(
            r.result,
            "Total words: 6\nRemoved: 3 (50.00%)\nKept: 3\nDistinct stop words removed: 2"
        );
    }

    #[test]
    fn empty_text_is_not_an_error() {
        let r = filter("", "english", "", "", false, false, "text").unwrap();
        assert_eq!(r.result, "");
        assert_eq!(r.total_words, 0);
        assert_eq!(r.removed_percent, 0.0);
    }

    #[test]
    fn line_breaks_and_paragraphs_survive() {
        let r = filter(
            "The first line.\nThe second line.",
            "english",
            "",
            "",
            false,
            false,
            "text",
        )
        .unwrap();
        assert_eq!(r.result, "first line.\nsecond line.");
    }

    #[test]
    fn filter_text_returns_the_view_only() {
        let s = filter_text("the cat", "english", "", "", false, false, "text").unwrap();
        assert_eq!(s, "cat");
    }
}
