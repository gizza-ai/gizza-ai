//! hashtag-extractor core — turn prose into ranked, ready-to-paste hashtags.
//!
//! Two jobs in one pass:
//!   1. **Generate** — score the text's content words (and optional multi-word
//!      keyphrases) and format the best ones as hashtags.
//!   2. **Keep** — collect the `#tags` the author already wrote in the text and
//!      list them first, verbatim.
//!
//! Scoring is deliberately simple and explainable so the page can document it:
//!
//! ```text
//! score = occurrences × (1 + 0.5 × earliness) × words_in_phrase
//! earliness = 1 at the very start of the text, 0 at the very end
//! ```
//!
//! A longer phrase that fully contains a shorter candidate with the SAME
//! occurrence count suppresses it, so "content marketing" wins over a redundant
//! "content" + "marketing" pair.
//!
//! Pure Rust, no I/O, no external NLP deps — the English stop-word list is
//! embedded.

use serde::Serialize;
use std::collections::HashMap;

/// One emitted hashtag.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hashtag {
    /// The hashtag including its leading `#`.
    pub tag: String,
    /// Relevance score (see the module docs). 0 for an authored tag whose word
    /// never survives keyword filtering (too short, numeric, or a stop word).
    pub score: f64,
    /// `"text"` for a hashtag already written in the input, `"keywords"` for one
    /// generated from the text's keywords.
    pub source: &'static str,
    /// How many times the underlying word/phrase occurs in the text.
    pub occurrences: usize,
}

/// The full result of one extraction.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HashtagResult {
    /// The hashtags actually returned, in output order.
    pub hashtags: Vec<Hashtag>,
    /// `hashtags.len()`.
    pub count: usize,
    /// Distinct hashtags found before the tag limit was applied.
    pub candidates: usize,
    /// The effective tag limit that was applied, if any.
    pub limit: Option<usize>,
    /// The hashtags joined with the chosen separator — paste this into a post.
    pub formatted: String,
    /// Character length of `formatted` (platforms count characters, not tags).
    pub characters: usize,
}

/// Every knob the tool exposes. Enum-ish fields stay `String` so all three
/// surfaces (chat, CLI, page) funnel through the same validation.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// Maximum hashtags to return; 0 means "no limit of my own".
    pub max_tags: usize,
    /// `none` | `instagram` | `tiktok` | `x` | `linkedin` | `facebook`.
    pub platform: String,
    /// `lowercase` | `camel` | `pascal` | `preserve`.
    pub style: String,
    /// Maximum words joined into one hashtag (1–4).
    pub phrase_words: usize,
    /// Minimum length of a word before it can become (part of) a hashtag (1–20).
    pub min_word_length: usize,
    /// Keep hashtags that are already written in the text.
    pub include_existing: bool,
    /// `space` | `comma` | `newline`.
    pub separator: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            max_tags: 10,
            platform: "none".into(),
            style: "lowercase".into(),
            phrase_words: 1,
            min_word_length: 3,
            include_existing: true,
            separator: "space".into(),
        }
    }
}

/// Hard ceiling on `max_tags` — beyond this the output stops being a caption.
pub const MAX_TAGS_CEILING: usize = 100;
/// Hard ceiling on `phrase_words`.
pub const MAX_PHRASE_WORDS: usize = 4;
/// Hard ceiling on `min_word_length`.
pub const MAX_MIN_WORD_LENGTH: usize = 20;

/// Recommended hashtag counts per platform (2026 guidance, not hard platform
/// maxima — most platforms technically allow far more than is useful).
const PLATFORM_LIMITS: &[(&str, usize)] = &[
    ("facebook", 3),
    ("instagram", 5),
    ("linkedin", 5),
    ("tiktok", 5),
    ("x", 2),
];

/// Built-in English stop-word list (NLTK-style). Kept sorted for `binary_search`.
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

/// `true` if a token can seed a hashtag: long enough, not a stop word, not
/// all-numeric (platforms treat a digits-only tag as a plain number).
fn is_content_word(lower: &str, min_word_length: usize) -> bool {
    lower.chars().count() >= min_word_length
        && !is_stopword(lower)
        && !lower.chars().all(|c| c.is_numeric())
}

/// A word of the input: its lowercase key plus the spelling it first appeared with.
struct Tok {
    lower: String,
    surface: String,
}

/// Split `text` into tokens plus the runs of consecutive content words
/// (broken at stop words and at any non-whitespace punctuation).
// The final `flush!()` writes `pending_break` one last time and never reads it.
#[allow(unused_assignments)]
fn tokenize(text: &str, min_word_length: usize) -> (Vec<Tok>, Vec<Vec<usize>>) {
    let mut toks: Vec<Tok> = Vec::new();
    let mut runs: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut buf = String::new();
    let mut pending_break = false;

    // A closure would need two mutable borrows, so this stays inline.
    macro_rules! flush {
        () => {
            if !buf.is_empty() {
                let surface = buf.trim_matches('\'').to_string();
                buf.clear();
                if !surface.is_empty() {
                    if pending_break && !current.is_empty() {
                        runs.push(std::mem::take(&mut current));
                    }
                    pending_break = false;
                    let lower = surface.to_lowercase();
                    let content = is_content_word(&lower, min_word_length);
                    toks.push(Tok { lower, surface });
                    if content {
                        current.push(toks.len() - 1);
                    } else if !current.is_empty() {
                        runs.push(std::mem::take(&mut current));
                    }
                }
            }
        };
    }

    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '\'' {
            buf.push(ch);
        } else {
            flush!();
            if !ch.is_whitespace() {
                pending_break = true;
            }
        }
    }
    flush!();
    if !current.is_empty() {
        runs.push(current);
    }
    (toks, runs)
}

/// A candidate keyphrase (1..=`phrase_words` consecutive content words).
struct Cand {
    words: Vec<String>,
    surfaces: Vec<String>,
    freq: usize,
    first: usize,
}

/// Strip everything a hashtag can't carry (apostrophes, stray marks) from a word.
fn clean(word: &str) -> String {
    word.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect()
}

fn capitalize(word: &str) -> String {
    let mut cs = word.chars();
    match cs.next() {
        Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
        None => String::new(),
    }
}

/// Render one candidate's words as a hashtag body in the requested style.
fn style_tag(words: &[String], surfaces: &[String], style: &str) -> String {
    match style {
        "preserve" => surfaces.iter().map(|w| clean(w)).collect::<Vec<_>>().concat(),
        "pascal" => words
            .iter()
            .map(|w| capitalize(&clean(w)))
            .collect::<Vec<_>>()
            .concat(),
        "camel" => words
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let c = clean(w);
                if i == 0 {
                    c
                } else {
                    capitalize(&c)
                }
            })
            .collect::<Vec<_>>()
            .concat(),
        // "lowercase"
        _ => words.iter().map(|w| clean(w)).collect::<Vec<_>>().concat(),
    }
}

/// Collect the hashtags already written in `text`, in order of appearance.
/// A tag body is the run of letters/digits/underscores after a `#`.
fn authored_hashtags(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '#' {
            continue;
        }
        let mut body = String::new();
        while let Some(&next) = chars.peek() {
            if next.is_alphanumeric() || next == '_' {
                body.push(next);
                chars.next();
            } else {
                break;
            }
        }
        if !body.is_empty() && !body.chars().all(|c| c.is_numeric()) {
            out.push(body);
        }
    }
    out
}

fn separator_str(sep: &str) -> &'static str {
    match sep {
        "comma" => ", ",
        "newline" => "\n",
        _ => " ",
    }
}

fn validate(opts: &Options) -> Result<(), String> {
    if !matches!(
        opts.style.as_str(),
        "lowercase" | "camel" | "pascal" | "preserve"
    ) {
        return Err(format!(
            "style must be one of lowercase, camel, pascal, preserve — got \"{}\"",
            opts.style
        ));
    }
    if !matches!(opts.separator.as_str(), "space" | "comma" | "newline") {
        return Err(format!(
            "separator must be one of space, comma, newline — got \"{}\"",
            opts.separator
        ));
    }
    if opts.platform != "none" && !PLATFORM_LIMITS.iter().any(|(p, _)| *p == opts.platform) {
        return Err(format!(
            "platform must be one of none, facebook, instagram, linkedin, tiktok, x — got \"{}\"",
            opts.platform
        ));
    }
    if opts.max_tags > MAX_TAGS_CEILING {
        return Err(format!(
            "max_tags must be between 0 and {MAX_TAGS_CEILING} (0 = no limit) — got {}",
            opts.max_tags
        ));
    }
    if opts.phrase_words < 1 || opts.phrase_words > MAX_PHRASE_WORDS {
        return Err(format!(
            "phrase_words must be between 1 and {MAX_PHRASE_WORDS} — got {}",
            opts.phrase_words
        ));
    }
    if opts.min_word_length < 1 || opts.min_word_length > MAX_MIN_WORD_LENGTH {
        return Err(format!(
            "min_word_length must be between 1 and {MAX_MIN_WORD_LENGTH} — got {}",
            opts.min_word_length
        ));
    }
    Ok(())
}

/// Effective tag cap: the tighter of `max_tags` and the platform's recommended
/// count. `None` = return everything.
fn effective_limit(opts: &Options) -> Option<usize> {
    let platform = PLATFORM_LIMITS
        .iter()
        .find(|(p, _)| *p == opts.platform)
        .map(|(_, n)| *n);
    match (opts.max_tags, platform) {
        (0, p) => p,
        (m, None) => Some(m),
        (m, Some(p)) => Some(m.min(p)),
    }
}

/// Turn `text` into ranked hashtags.
pub fn extract(text: &str, opts: &Options) -> Result<HashtagResult, String> {
    validate(opts)?;
    if text.trim().is_empty() {
        return Err("no text provided — paste the caption, article, or post you want hashtags for"
            .to_string());
    }

    let (toks, runs) = tokenize(text, opts.min_word_length);

    // Build 1..=phrase_words n-grams out of each content-word run.
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut cands: Vec<Cand> = Vec::new();
    for run in &runs {
        for start in 0..run.len() {
            for n in 1..=opts.phrase_words {
                if start + n > run.len() {
                    break;
                }
                let idxs = &run[start..start + n];
                let words: Vec<String> = idxs.iter().map(|&i| toks[i].lower.clone()).collect();
                let key = words.join(" ");
                match index.get(&key) {
                    Some(&at) => cands[at].freq += 1,
                    None => {
                        index.insert(key, cands.len());
                        cands.push(Cand {
                            surfaces: idxs.iter().map(|&i| toks[i].surface.clone()).collect(),
                            words,
                            freq: 1,
                            first: idxs[0],
                        });
                    }
                }
            }
        }
    }

    // score = occurrences × earliness bonus × phrase length.
    let span = toks.len().saturating_sub(1).max(1) as f64;
    let scored = |c: &Cand| -> f64 {
        let earliness = 1.0 - (c.first as f64 / span);
        let raw = c.freq as f64 * (1.0 + 0.5 * earliness) * c.words.len() as f64;
        (raw * 1000.0).round() / 1000.0
    };

    let mut order: Vec<usize> = (0..cands.len()).collect();
    order.sort_by(|&a, &b| {
        scored(&cands[b])
            .partial_cmp(&scored(&cands[a]))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(cands[a].first.cmp(&cands[b].first))
            .then(cands[a].words.cmp(&cands[b].words))
    });

    // Suppress a candidate fully contained in an already-accepted longer phrase
    // that occurs exactly as often — it adds no information.
    let mut accepted: Vec<usize> = Vec::new();
    for &i in &order {
        let subsumed = accepted.iter().any(|&j| {
            cands[j].words.len() > cands[i].words.len()
                && cands[j].freq == cands[i].freq
                && cands[j]
                    .words
                    .windows(cands[i].words.len())
                    .any(|w| w == cands[i].words.as_slice())
        });
        if !subsumed {
            accepted.push(i);
        }
    }

    // Authored hashtags first (verbatim), then the generated ones. Dedupe is
    // case-insensitive across both groups.
    let mut seen: HashMap<String, ()> = HashMap::new();
    let mut hashtags: Vec<Hashtag> = Vec::new();
    if opts.include_existing {
        for body in authored_hashtags(text) {
            let key = body.to_lowercase();
            if seen.insert(key.clone(), ()).is_some() {
                continue;
            }
            let (score, occurrences) = match index.get(&key) {
                Some(&at) => (scored(&cands[at]), cands[at].freq),
                None => (0.0, text.matches(&format!("#{body}")).count()),
            };
            hashtags.push(Hashtag {
                tag: format!("#{body}"),
                score,
                source: "text",
                occurrences,
            });
        }
    }
    for &i in &accepted {
        let body = style_tag(&cands[i].words, &cands[i].surfaces, &opts.style);
        if body.is_empty() {
            continue;
        }
        if seen.insert(body.to_lowercase(), ()).is_some() {
            continue;
        }
        hashtags.push(Hashtag {
            tag: format!("#{body}"),
            score: scored(&cands[i]),
            source: "keywords",
            occurrences: cands[i].freq,
        });
    }

    if hashtags.is_empty() {
        return Err(format!(
            "no hashtag candidates found — every word was a stop word, shorter than \
             min_word_length ({}), or numeric. Try lowering min_word_length or pasting more text.",
            opts.min_word_length
        ));
    }

    let candidates = hashtags.len();
    let limit = effective_limit(opts);
    if let Some(n) = limit {
        hashtags.truncate(n);
    }

    let formatted = hashtags
        .iter()
        .map(|h| h.tag.as_str())
        .collect::<Vec<_>>()
        .join(separator_str(&opts.separator));

    Ok(HashtagResult {
        count: hashtags.len(),
        characters: formatted.chars().count(),
        candidates,
        limit,
        formatted,
        hashtags,
    })
}

/// Human-readable rendering for the page surface: the paste-ready hashtag line,
/// then a one-line summary.
pub fn render(text: &str, opts: &Options) -> Result<String, String> {
    let r = extract(text, opts)?;
    let tag_word = if r.count == 1 { "hashtag" } else { "hashtags" };
    let mut summary = format!(
        "{} {} · {} characters",
        r.count, tag_word, r.characters
    );
    if r.candidates > r.count {
        summary.push_str(&format!(" · {} candidates found", r.candidates));
    }
    Ok(format!("{}\n\n{}", r.formatted, summary))
}

/// Blank means "use the documented default" — the page sends an empty string
/// for a field the user cleared, and serde sends one for an omitted chat arg.
fn or_default<'a>(value: &'a str, fallback: &'a str) -> String {
    let t = value.trim();
    if t.is_empty() {
        fallback.to_string()
    } else {
        t.to_lowercase()
    }
}

/// A count arrives as a signed integer on every surface; `usize` conversion is
/// the only check that has to happen before [`validate`] can report the range.
fn non_negative(value: i64, field: &str) -> Result<usize, String> {
    usize::try_from(value).map_err(|_| format!("{field} must not be negative — got {value}"))
}

/// The flat, stringly-typed entry point every surface funnels through — the chat
/// block, the CLI and the browser page all call this with the same argument
/// order, so the three surfaces cannot drift. Returns the paste-ready hashtag
/// line plus a one-line summary.
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    max_tags: i64,
    platform: &str,
    style: &str,
    phrase_words: i64,
    min_word_length: i64,
    include_existing: bool,
    separator: &str,
) -> Result<String, String> {
    let opts = Options {
        max_tags: non_negative(max_tags, "max_tags")?,
        platform: or_default(platform, "none"),
        style: or_default(style, "lowercase"),
        phrase_words: non_negative(phrase_words, "phrase_words")?,
        min_word_length: non_negative(min_word_length, "min_word_length")?,
        include_existing,
        separator: or_default(separator, "space"),
    };
    render(text, &opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defaults `run` documents must reproduce `Options::default()`.
    #[test]
    fn run_with_blank_options_uses_the_documented_defaults() {
        let out = run(
            "Content marketing builds brand trust. Great content marketing wins.",
            10,
            "",
            "",
            1,
            3,
            true,
            "",
        )
        .unwrap();
        assert_eq!(
            out,
            render(
                "Content marketing builds brand trust. Great content marketing wins.",
                &Options::default()
            )
            .unwrap()
        );
    }

    #[test]
    fn run_rejects_negative_counts() {
        let err = run("hello world", -1, "none", "lowercase", 1, 3, true, "space").unwrap_err();
        assert!(err.contains("max_tags must not be negative"), "got: {err}");
        let err = run("hello world", 10, "none", "lowercase", 1, -3, true, "space").unwrap_err();
        assert!(
            err.contains("min_word_length must not be negative"),
            "got: {err}"
        );
    }

    #[test]
    fn stopwords_sorted_for_binary_search() {
        let mut s = STOPWORDS.to_vec();
        s.sort();
        assert_eq!(s, STOPWORDS, "STOPWORDS must stay sorted for binary_search");
    }

    #[test]
    fn ranks_single_word_hashtags_by_relevance() {
        let r = extract(
            "Content marketing builds brand trust. Great content marketing wins.",
            &Options::default(),
        )
        .unwrap();
        assert_eq!(
            r.formatted,
            "#content #marketing #builds #brand #trust #great #wins"
        );
        assert_eq!(r.count, 7);
        assert_eq!(r.characters, 54);
        assert_eq!(r.hashtags[0].occurrences, 2);
        assert_eq!(r.hashtags[0].source, "keywords");
    }

    #[test]
    fn page_render_shows_summary_line() {
        let out = render(
            "Content marketing builds brand trust. Great content marketing wins.",
            &Options::default(),
        )
        .unwrap();
        assert_eq!(
            out,
            "#content #marketing #builds #brand #trust #great #wins\n\n7 hashtags · 54 characters"
        );
    }

    #[test]
    fn max_tags_truncates_and_reports_candidates() {
        let opts = Options { max_tags: 3, ..Options::default() };
        let out = render("Content marketing builds brand trust.", &opts).unwrap();
        assert_eq!(
            out,
            "#content #marketing #builds\n\n3 hashtags · 27 characters · 5 candidates found"
        );
    }

    #[test]
    fn multiword_phrases_subsume_their_parts() {
        let opts = Options { phrase_words: 2, ..Options::default() };
        let r = extract(
            "Content marketing builds brand trust. Great content marketing wins.",
            &opts,
        )
        .unwrap();
        assert_eq!(r.hashtags[0].tag, "#contentmarketing");
        assert_eq!(r.hashtags[0].occurrences, 2);
        // The redundant single-word parts are gone.
        assert!(!r.hashtags.iter().any(|h| h.tag == "#content"));
        assert!(!r.hashtags.iter().any(|h| h.tag == "#marketing"));
    }

    #[test]
    fn casing_styles_apply_to_generated_tags() {
        let text = "Remote Work culture. Remote Work wins.";
        let pascal = extract(
            text,
            &Options { phrase_words: 2, style: "pascal".into(), ..Options::default() },
        )
        .unwrap();
        assert_eq!(pascal.hashtags[0].tag, "#RemoteWork");
        let camel = extract(
            text,
            &Options { phrase_words: 2, style: "camel".into(), ..Options::default() },
        )
        .unwrap();
        assert_eq!(camel.hashtags[0].tag, "#remoteWork");
        let preserve = extract(
            text,
            &Options { phrase_words: 2, style: "preserve".into(), ..Options::default() },
        )
        .unwrap();
        assert_eq!(preserve.hashtags[0].tag, "#RemoteWork");
    }

    #[test]
    fn keeps_authored_hashtags_first_and_verbatim() {
        let r = extract(
            "Shipping the new release today. #DevLog #BuildInPublic",
            &Options::default(),
        )
        .unwrap();
        assert_eq!(r.hashtags[0].tag, "#DevLog");
        assert_eq!(r.hashtags[0].source, "text");
        assert_eq!(r.hashtags[1].tag, "#BuildInPublic");
        // ...and the generated ones follow.
        assert!(r.hashtags[2..].iter().all(|h| h.source == "keywords"));
    }

    #[test]
    fn authored_hashtags_can_be_dropped_and_are_deduped() {
        let text = "Remote work is great. #remote #Remote";
        let without = extract(
            text,
            &Options { include_existing: false, ..Options::default() },
        )
        .unwrap();
        assert!(without.hashtags.iter().all(|h| h.source == "keywords"));

        let with = extract(text, &Options::default()).unwrap();
        assert_eq!(with.hashtags[0].tag, "#remote");
        // Case-insensitive dedupe: "#Remote" and the generated "#remote" collapse.
        assert_eq!(with.hashtags.iter().filter(|h| h.tag.to_lowercase() == "#remote").count(), 1);
    }

    #[test]
    fn platform_caps_the_output() {
        let text = "Content marketing builds brand trust and great content marketing wins today.";
        let x = extract(
            text,
            &Options { platform: "x".into(), max_tags: 0, ..Options::default() },
        )
        .unwrap();
        assert_eq!(x.count, 2);
        assert_eq!(x.limit, Some(2));
        // The tighter of max_tags and the platform count wins.
        let tight = extract(
            text,
            &Options { platform: "instagram".into(), max_tags: 3, ..Options::default() },
        )
        .unwrap();
        assert_eq!(tight.count, 3);
    }

    #[test]
    fn separators_and_no_limit() {
        let opts = Options {
            max_tags: 0,
            separator: "comma".into(),
            ..Options::default()
        };
        let r = extract("Alpha beta gamma.", &opts).unwrap();
        assert_eq!(r.formatted, "#alpha, #beta, #gamma");
        assert_eq!(r.limit, None);

        let nl = Options { separator: "newline".into(), ..opts };
        let r = extract("Alpha beta gamma.", &nl).unwrap();
        assert_eq!(r.formatted, "#alpha\n#beta\n#gamma");
    }

    #[test]
    fn min_word_length_filters_short_words() {
        let long = Options { min_word_length: 6, ..Options::default() };
        let r = extract("Big data drives modern marketing.", &long).unwrap();
        assert_eq!(r.formatted, "#drives #modern #marketing");
    }

    #[test]
    fn tokenizes_non_latin_scripts() {
        let r = extract("Καλημέρα κόσμε καλημέρα.", &Options::default()).unwrap();
        assert_eq!(r.hashtags[0].tag, "#καλημέρα");
        assert_eq!(r.hashtags[0].occurrences, 2);
    }

    #[test]
    fn error_on_empty_text() {
        let err = extract("   \n ", &Options::default()).unwrap_err();
        assert!(err.contains("no text provided"), "got: {err}");
    }

    #[test]
    fn error_when_nothing_survives_filtering() {
        let err = extract("It is the and of a 42.", &Options::default()).unwrap_err();
        assert!(err.contains("no hashtag candidates found"), "got: {err}");
        assert!(err.contains("min_word_length (3)"), "got: {err}");
    }

    #[test]
    fn error_on_bad_enum_values() {
        let err = extract(
            "hello world",
            &Options { style: "shouty".into(), ..Options::default() },
        )
        .unwrap_err();
        assert!(err.contains("style must be one of"), "got: {err}");

        let err = extract(
            "hello world",
            &Options { platform: "myspace".into(), ..Options::default() },
        )
        .unwrap_err();
        assert!(err.contains("platform must be one of"), "got: {err}");

        let err = extract(
            "hello world",
            &Options { separator: "tab".into(), ..Options::default() },
        )
        .unwrap_err();
        assert!(err.contains("separator must be one of"), "got: {err}");
    }

    #[test]
    fn error_on_out_of_range_numbers() {
        let err = extract(
            "hello world",
            &Options { phrase_words: 9, ..Options::default() },
        )
        .unwrap_err();
        assert!(err.contains("phrase_words must be between 1 and 4"), "got: {err}");

        let err = extract(
            "hello world",
            &Options { min_word_length: 0, ..Options::default() },
        )
        .unwrap_err();
        assert!(err.contains("min_word_length must be between 1 and 20"), "got: {err}");

        let err = extract(
            "hello world",
            &Options { max_tags: 101, ..Options::default() },
        )
        .unwrap_err();
        assert!(err.contains("max_tags must be between 0 and 100"), "got: {err}");
    }
}
