//! prose-linter core — flag clichés, weasel words, passive voice, redundancies
//! and other style problems in English prose. Pure Rust, no I/O, no model:
//! every finding comes from an embedded rule set of phrase lists plus a few
//! small heuristics (passive voice, repeated words, sentence length).
//!
//! Shared by the chat skill block, the CLI and the web page.
//!
//! The linter is deliberately *naive* in the same sense as the classic prose
//! linters: it matches phrases and simple patterns, not grammar. It is fast,
//! deterministic and offline, and it will occasionally flag something that
//! reads fine — every finding is a suggestion, not a verdict.

use serde::Serialize;

/// Hard cap on input size (bytes).
pub const MAX_BYTES: usize = 1_000_000;

/// Rules enabled by `checks = "all"`, in canonical (alphabetical) order.
pub const DEFAULT_RULES: &[&str] = &[
    "adverb",
    "archaism",
    "cliche",
    "hedge",
    "illusion",
    "jargon",
    "long-sentence",
    "passive",
    "redundancy",
    "so-start",
    "there-is",
    "uncomparable",
    "weasel",
    "wordy",
];

/// Rules that exist but are NOT part of `all` — they must be asked for by name.
pub const OPT_IN_RULES: &[&str] = &["eprime"];

// ---------------------------------------------------------------------------
// Rule data
// ---------------------------------------------------------------------------

/// Tired stock phrases. Kept free of apostrophes so curly quotes can't break a
/// match (see `matching limits` on the page).
const CLICHES: &[&str] = &[
    "a perfect storm",
    "at the end of the day",
    "avoid it like the plague",
    "back to the drawing board",
    "ball is in your court",
    "beat around the bush",
    "best of both worlds",
    "bite the bullet",
    "blessing in disguise",
    "boots on the ground",
    "burning the midnight oil",
    "by and large",
    "calm before the storm",
    "dead as a doornail",
    "diamond in the rough",
    "drop in the ocean",
    "easier said than done",
    "every cloud has a silver lining",
    "few and far between",
    "fit as a fiddle",
    "food for thought",
    "get the ball rolling",
    "go the extra mile",
    "hit the ground running",
    "in the nick of time",
    "it goes without saying",
    "it is what it is",
    "last but not least",
    "leave no stone unturned",
    "level playing field",
    "light at the end of the tunnel",
    "low-hanging fruit",
    "make a long story short",
    "moving the goalposts",
    "needle in a haystack",
    "no brainer",
    "on the same page",
    "only time will tell",
    "par for the course",
    "paradigm shift",
    "piece of cake",
    "push the envelope",
    "raise the bar",
    "reinvent the wheel",
    "run it up the flagpole",
    "second to none",
    "silver bullet",
    "take it to the next level",
    "the bottom line is",
    "think outside of the box",
    "think outside the box",
    "tip of the iceberg",
    "touch base",
    "under the weather",
    "when push comes to shove",
    "win-win",
];

/// Vague quantity / intensity words that make a claim unfalsifiable.
const WEASELS: &[&str] = &[
    "a good deal of",
    "a great deal of",
    "a lot of",
    "a majority of",
    "a number of",
    "a variety of",
    "are a number",
    "countless",
    "excellent",
    "few",
    "huge",
    "is a number",
    "lots of",
    "many",
    "most of",
    "myriad",
    "numerous",
    "quite",
    "several",
    "tiny",
    "vast",
    "various",
    "very",
];

/// Filler adverbs that weaken the verb they modify.
const ADVERBS: &[&str] = &[
    "absolutely",
    "actually",
    "amazingly",
    "basically",
    "certainly",
    "clearly",
    "completely",
    "definitely",
    "deeply",
    "especially",
    "essentially",
    "exceedingly",
    "extremely",
    "fairly",
    "generally",
    "greatly",
    "highly",
    "hugely",
    "incredibly",
    "interestingly",
    "largely",
    "literally",
    "mostly",
    "notably",
    "obviously",
    "particularly",
    "really",
    "relatively",
    "remarkably",
    "seriously",
    "significantly",
    "simply",
    "slightly",
    "substantially",
    "surprisingly",
    "terribly",
    "totally",
    "truly",
    "utterly",
    "virtually",
];

/// Phrases that back away from the claim being made.
const HEDGES: &[&str] = &[
    "a bit",
    "appears to be",
    "arguably",
    "could be argued",
    "i believe",
    "i feel",
    "i think",
    "in a sense",
    "in my opinion",
    "in my view",
    "it is possible that",
    "it seems",
    "kind of",
    "may or may not",
    "maybe",
    "might be",
    "more or less",
    "perhaps",
    "possibly",
    "seems to",
    "somewhat",
    "sort of",
    "to some extent",
    "we believe",
];

/// Corporate / marketing jargon → the plain word. An empty replacement means
/// "just cut it".
const JARGON: &[(&str, &str)] = &[
    ("actionable", "usable"),
    ("bandwidth", "time"),
    ("best-in-class", ""),
    ("boil the ocean", "do everything at once"),
    ("circle back", "follow up"),
    ("core competency", "strength"),
    ("cutting-edge", ""),
    ("deep dive", "close look"),
    ("drill down", "look closer"),
    ("frictionless", "easy"),
    ("going forward", "from now on"),
    ("holistic", "whole"),
    ("ideate", "think"),
    ("incentivize", "encourage"),
    ("industry-leading", ""),
    ("learnings", "lessons"),
    ("mission-critical", "essential"),
    ("move the needle", "make a difference"),
    ("next-generation", ""),
    ("operationalize", "put into practice"),
    ("robust", "reliable"),
    ("seamless", "smooth"),
    ("state-of-the-art", ""),
    ("synergies", "overlaps"),
    ("synergy", "overlap"),
    ("thought leader", "expert"),
    ("turnkey", "ready to use"),
    ("utilise", "use"),
    ("utilize", "use"),
    ("value add", "benefit"),
    ("world-class", ""),
];

/// Long-winded phrases → the short form. Empty replacement means "cut it".
const WORDY: &[(&str, &str)] = &[
    ("a large number of", "many"),
    ("a small number of", "a few"),
    ("a sufficient number of", "enough"),
    ("are able to", "can"),
    ("as a matter of fact", "in fact"),
    ("at all times", "always"),
    ("at the present time", "now"),
    ("at this point in time", "now"),
    ("conduct an investigation", "investigate"),
    ("despite the fact that", "although"),
    ("due to the fact that", "because"),
    ("for the purpose of", "for"),
    ("give consideration to", "consider"),
    ("has the ability to", "can"),
    ("in a timely manner", "promptly"),
    ("in close proximity to", "near"),
    ("in order to", "to"),
    ("in regard to", "about"),
    ("in relation to", "about"),
    ("in spite of the fact that", "although"),
    ("in the absence of", "without"),
    ("in the event that", "if"),
    ("in the near future", "soon"),
    ("in the process of", ""),
    ("is able to", "can"),
    ("it is important to note that", ""),
    ("it should be noted that", ""),
    ("make a decision", "decide"),
    ("needless to say", ""),
    ("on a daily basis", "daily"),
    ("on a regular basis", "regularly"),
    ("on account of the fact that", "because"),
    ("prior to", "before"),
    ("provide assistance to", "help"),
    ("subsequent to", "after"),
    ("take into consideration", "consider"),
    ("the majority of", "most"),
    ("with regard to", "about"),
    ("with respect to", "about"),
];

/// Phrases that say the same thing twice → the shorter form.
const REDUNDANCIES: &[(&str, &str)] = &[
    ("12 midnight", "midnight"),
    ("12 noon", "noon"),
    ("absolutely essential", "essential"),
    ("added bonus", "bonus"),
    ("advance planning", "planning"),
    ("advance warning", "warning"),
    ("annual anniversary", "anniversary"),
    ("atm machine", "ATM"),
    ("basic fundamentals", "fundamentals"),
    ("brief summary", "summary"),
    ("close proximity", "proximity"),
    ("collaborate together", "collaborate"),
    ("combine together", "combine"),
    ("completely destroyed", "destroyed"),
    ("cooperate together", "cooperate"),
    ("current status", "status"),
    ("each and every", "each"),
    ("end result", "result"),
    ("exact same", "same"),
    ("final outcome", "outcome"),
    ("first and foremost", "first"),
    ("free gift", "gift"),
    ("future plans", "plans"),
    ("general consensus", "consensus"),
    ("join together", "join"),
    ("merge together", "merge"),
    ("new innovation", "innovation"),
    ("over exaggerate", "exaggerate"),
    ("past experience", "experience"),
    ("past history", "history"),
    ("personal opinion", "opinion"),
    ("pin number", "PIN"),
    ("plan ahead", "plan"),
    ("postpone until later", "postpone"),
    ("repeat again", "repeat"),
    ("revert back", "revert"),
    ("safe haven", "haven"),
    ("still remains", "remains"),
    ("sum total", "total"),
    ("true fact", "fact"),
    ("unexpected surprise", "surprise"),
    ("usual custom", "custom"),
];

/// Legal-ese and antique wording → the modern word. Empty means "just cut it".
const ARCHAISMS: &[(&str, &str)] = &[
    ("aforementioned", "this"),
    ("aforesaid", "this"),
    ("albeit", "although"),
    ("amidst", "amid"),
    ("amongst", "among"),
    ("betwixt", "between"),
    ("ergo", "so"),
    ("henceforth", "from now on"),
    ("hereby", ""),
    ("herein", "here"),
    ("hereinafter", "later"),
    ("heretofore", "until now"),
    ("herewith", "with this"),
    ("inasmuch as", "since"),
    ("insofar as", "as far as"),
    ("notwithstanding", "despite"),
    ("oftentimes", "often"),
    ("pursuant to", "under"),
    ("thence", "from there"),
    ("thereafter", "after that"),
    ("thereby", "so"),
    ("therein", "in it"),
    ("thereof", "of it"),
    ("thusly", "thus"),
    ("whence", "from where"),
    ("whereby", "by which"),
    ("wherein", "in which"),
    ("wherewithal", "means"),
    ("whilst", "while"),
];

/// Words that make a comparison of degree, e.g. "very" in "very unique".
const INTENSIFIERS: &[&str] = &[
    "absolutely",
    "completely",
    "entirely",
    "extremely",
    "fairly",
    "fully",
    "highly",
    "hugely",
    "incredibly",
    "largely",
    "more",
    "most",
    "mostly",
    "partially",
    "quite",
    "rather",
    "really",
    "slightly",
    "somewhat",
    "totally",
    "utterly",
    "very",
];

/// Adjectives that describe an absolute state, so they take no degree —
/// something either is unique or it is not.
const UNCOMPARABLES: &[&str] = &[
    "absolute",
    "adequate",
    "chief",
    "complete",
    "correct",
    "entire",
    "eternal",
    "fatal",
    "final",
    "ideal",
    "impossible",
    "inevitable",
    "infinite",
    "irrevocable",
    "main",
    "paramount",
    "perfect",
    "perpetual",
    "preferable",
    "principal",
    "singular",
    "sufficient",
    "unanimous",
    "unavoidable",
    "unbroken",
    "unique",
    "universal",
];

/// Forms of "to be" — the passive-voice trigger and the E-Prime rule.
const BE_VERBS: &[&str] = &["am", "is", "are", "was", "were", "be", "been", "being"];

/// Extra "to be" spellings E-Prime rejects (contractions).
const BE_CONTRACTIONS: &[&str] = &[
    "isn't", "aren't", "wasn't", "weren't", "i'm", "it's", "that's", "there's", "he's", "she's",
    "we're", "they're", "you're", "here's", "what's", "who's",
];

/// Words allowed to sit between a "to be" verb and its participle.
const PASSIVE_SKIPS: &[&str] = &["not", "also", "already", "still", "just", "being", "then", "now"];

/// Irregular past participles (the `-ed` test misses these).
const IRREGULAR_PARTICIPLES: &[&str] = &[
    "beaten", "become", "begun", "bent", "bitten", "blown", "born", "borne", "bought", "broken",
    "brought", "built", "burnt", "caught", "chosen", "come", "cost", "cut", "dealt", "done",
    "drawn", "driven", "drunk", "eaten", "fallen", "fed", "felt", "fought", "found", "forgotten",
    "frozen", "given", "gone", "gotten", "grown", "held", "hidden", "hit", "hurt", "kept", "known",
    "laid", "led", "left", "lent", "let", "lost", "made", "meant", "met", "paid", "put", "read",
    "ridden", "risen", "run", "said", "seen", "sent", "set", "sewn", "shaken", "shot", "shown",
    "shut", "slept", "sold", "sought", "sown", "spent", "spoken", "spread", "stolen", "struck",
    "sung", "sunk", "swum", "taken", "taught", "thought", "thrown", "told", "torn", "understood",
    "withdrawn", "won", "worn", "woven", "written",
];

/// Abbreviations whose trailing period does not end a sentence.
const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "vs", "etc", "e.g", "i.e", "fig", "no",
    "inc", "ltd", "co", "approx", "al",
];

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// One style finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Issue {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column, counted in characters.
    pub col: usize,
    /// Rule name, e.g. `cliche` or `passive`.
    pub rule: &'static str,
    /// What to do about it, in plain English.
    pub message: String,
    /// The exact text that triggered the rule, as written.
    #[serde(rename = "match")]
    pub matched: String,
    /// A shorter replacement, when the rule has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// Byte offset of `matched` within the input.
    #[serde(skip)]
    pub start: usize,
}

/// Report shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Aligned `line:col  rule  issue` table plus per-rule counts.
    Report,
    /// Each offending line reprinted with a caret under the match.
    Annotated,
    /// Machine-readable object.
    Json,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "report" => Ok(OutputFormat::Report),
            "annotated" => Ok(OutputFormat::Annotated),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!(
                "unknown output '{other}' — use report, annotated or json"
            )),
        }
    }
}

#[derive(Serialize)]
struct JsonReport<'a> {
    issues: &'a [Issue],
    total: usize,
    shown: usize,
    truncated: bool,
    counts: Vec<RuleCount>,
    words: usize,
    sentences: usize,
    checks: &'a [&'static str],
}

#[derive(Serialize)]
struct RuleCount {
    rule: &'static str,
    count: usize,
}

// ---------------------------------------------------------------------------
// Text helpers
// ---------------------------------------------------------------------------

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'\'' || b == b'-'
}

/// Byte offsets where each line starts.
fn line_starts(text: &str) -> Vec<usize> {
    let mut v = vec![0usize];
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            v.push(i + 1);
        }
    }
    v
}

/// Map a byte offset to a 1-based (line, column-in-characters).
fn position(text: &str, starts: &[usize], offset: usize) -> (usize, usize) {
    let idx = match starts.binary_search(&offset) {
        Ok(i) => i,
        Err(i) => i - 1,
    };
    let col = text[starts[idx]..offset].chars().count() + 1;
    (idx + 1, col)
}

/// Every occurrence of `needle` in `hay` (already ASCII-lowercased) that sits on
/// word boundaries. Returns `(start, len)` byte spans.
fn find_phrase(hay: &str, needle: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if needle.is_empty() {
        return out;
    }
    let nb = needle.as_bytes();
    let hb = hay.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = hay[from..].find(needle) {
        let start = from + rel;
        let end = start + nb.len();
        let left_ok = start == 0 || !is_word_byte(hb[start - 1]);
        let right_ok = end == hb.len() || !is_word_byte(hb[end]);
        // A phrase that itself starts/ends with a non-word byte doesn't need
        // that side checked (none currently do, but keep it honest).
        let left_ok = left_ok || !is_word_byte(nb[0]);
        let right_ok = right_ok || !is_word_byte(nb[nb.len() - 1]);
        if left_ok && right_ok {
            out.push((start, nb.len()));
        }
        from = start + 1;
    }
    out
}

/// Word tokens as `(start, end)` byte spans over the input.
fn words(text: &str) -> Vec<(usize, usize)> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        if b[i].is_ascii_alphanumeric() {
            let start = i;
            while i < b.len() && is_word_byte(b[i]) {
                i += 1;
            }
            // Don't let a trailing hyphen/apostrophe hang off the token.
            let mut end = i;
            while end > start && !b[end - 1].is_ascii_alphanumeric() {
                end -= 1;
            }
            out.push((start, end));
        } else if b[i] >= 0x80 {
            // Non-ASCII letters (accented words) still count as one word.
            let start = i;
            while i < b.len() && (b[i] >= 0x80 || b[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            out.push((start, i));
        } else {
            i += 1;
        }
    }
    out
}

/// Sentence spans as `(start, end)` byte offsets. Splits on `.`/`!`/`?`
/// followed by whitespace, and on blank lines, with a small abbreviation guard.
fn sentences(text: &str) -> Vec<(usize, usize)> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        if c == b'.' || c == b'!' || c == b'?' {
            // Consume any run of terminators plus trailing quotes/brackets.
            let mut j = i + 1;
            while j < b.len() && matches!(b[j], b'.' | b'!' | b'?' | b'"' | b'\'' | b')' | b']') {
                j += 1;
            }
            let ends_here = j >= b.len() || b[j].is_ascii_whitespace();
            if ends_here && !(c == b'.' && ends_with_abbreviation(text, i)) {
                push_sentence(text, &mut out, start, j);
                start = j;
                i = j;
                continue;
            }
        } else if c == b'\n' {
            // A blank line always ends a sentence.
            let mut j = i + 1;
            while j < b.len() && (b[j] == b' ' || b[j] == b'\t' || b[j] == b'\r') {
                j += 1;
            }
            if j >= b.len() || b[j] == b'\n' {
                push_sentence(text, &mut out, start, i);
                start = j;
                i = j;
                continue;
            }
        }
        i += 1;
    }
    push_sentence(text, &mut out, start, b.len());
    out
}

fn push_sentence(text: &str, out: &mut Vec<(usize, usize)>, start: usize, end: usize) {
    if end <= start {
        return;
    }
    let slice = &text[start..end];
    let lead = slice.len() - slice.trim_start().len();
    let trail = slice.len() - slice.trim_end().len();
    let (s, e) = (start + lead, end - trail);
    if e > s {
        out.push((s, e));
    }
}

/// True when the word immediately before byte `dot` is a known abbreviation.
fn ends_with_abbreviation(text: &str, dot: usize) -> bool {
    let b = text.as_bytes();
    let mut s = dot;
    while s > 0 && (is_word_byte(b[s - 1]) || b[s - 1] == b'.') {
        s -= 1;
    }
    if s >= dot {
        return false;
    }
    let w = text[s..dot].trim_end_matches('.').to_ascii_lowercase();
    ABBREVIATIONS.contains(&w.as_str())
}

// ---------------------------------------------------------------------------
// Check selection
// ---------------------------------------------------------------------------

fn known_rule(name: &str) -> bool {
    DEFAULT_RULES.contains(&name) || OPT_IN_RULES.contains(&name)
}

/// Turn a `checks` spec into the canonical, de-duplicated rule list.
///
/// Accepts `all`, `none`, individual rule names, and `-name` to remove one —
/// e.g. `all,-passive` or `cliche,weasel`.
pub fn resolve_checks(spec: &str) -> Result<Vec<&'static str>, String> {
    let spec = spec.trim();
    let mut on: Vec<&'static str> = Vec::new();
    let tokens: Vec<&str> = if spec.is_empty() {
        vec!["all"]
    } else {
        spec.split(',').map(|t| t.trim()).collect()
    };
    for tok in tokens {
        if tok.is_empty() {
            continue;
        }
        let lower = tok.to_ascii_lowercase();
        let (remove, name) = match lower.strip_prefix('-') {
            Some(rest) => (true, rest.to_string()),
            None => (false, lower),
        };
        if name == "all" {
            if remove {
                on.clear();
            } else {
                for r in DEFAULT_RULES {
                    if !on.contains(r) {
                        on.push(r);
                    }
                }
            }
            continue;
        }
        if name == "none" {
            on.clear();
            continue;
        }
        if !known_rule(&name) {
            let mut valid: Vec<&str> = DEFAULT_RULES.to_vec();
            valid.extend_from_slice(OPT_IN_RULES);
            valid.sort_unstable();
            return Err(format!(
                "unknown check '{name}' — valid checks are all, none, {} (prefix a name with '-' to switch it off)",
                valid.join(", ")
            ));
        }
        let canonical = *DEFAULT_RULES
            .iter()
            .chain(OPT_IN_RULES.iter())
            .find(|r| **r == name)
            .unwrap();
        if remove {
            on.retain(|r| *r != canonical);
        } else if !on.contains(&canonical) {
            on.push(canonical);
        }
    }
    if on.is_empty() {
        return Err("no checks selected — pass checks=all or a comma-separated list".into());
    }
    let mut all: Vec<&'static str> = DEFAULT_RULES.to_vec();
    all.extend_from_slice(OPT_IN_RULES);
    all.sort_unstable();
    Ok(all.into_iter().filter(|r| on.contains(r)).collect())
}

// ---------------------------------------------------------------------------
// Linting
// ---------------------------------------------------------------------------

struct Raw {
    start: usize,
    len: usize,
    rule: &'static str,
    message: String,
    suggestion: Option<String>,
}

fn phrase_rule(low: &str, rule: &'static str, list: &[&str], out: &mut Vec<Raw>, msg: fn(&str) -> String) {
    for p in list {
        for (start, len) in find_phrase(low, p) {
            out.push(Raw {
                start,
                len,
                rule,
                message: msg(&low[start..start + len]),
                suggestion: None,
            });
        }
    }
}

fn swap_rule(
    low: &str,
    rule: &'static str,
    list: &[(&str, &str)],
    out: &mut Vec<Raw>,
    with_swap: fn(&str, &str) -> String,
    cut: fn(&str) -> String,
) {
    for (p, better) in list {
        for (start, len) in find_phrase(low, p) {
            let m = &low[start..start + len];
            out.push(Raw {
                start,
                len,
                rule,
                message: if better.is_empty() {
                    cut(m)
                } else {
                    with_swap(m, better)
                },
                suggestion: if better.is_empty() {
                    None
                } else {
                    Some((*better).to_string())
                },
            });
        }
    }
}

fn is_participle(w: &str) -> bool {
    if IRREGULAR_PARTICIPLES.contains(&w) {
        return true;
    }
    w.len() >= 4 && w.ends_with("ed") && w.bytes().all(|b| b.is_ascii_alphabetic())
}

/// Run the selected checks and return the findings, sorted by position.
pub fn lint(
    text: &str,
    checks: &[&'static str],
    ignore: &str,
    long_sentence_words: usize,
) -> Vec<Issue> {
    let low = text.to_ascii_lowercase();
    let mut raw: Vec<Raw> = Vec::new();
    let on = |r: &str| checks.contains(&r);

    if on("cliche") {
        phrase_rule(&low, "cliche", CLICHES, &mut raw, |m| {
            format!("\"{m}\" is a cliché — say what you actually mean.")
        });
    }
    if on("weasel") {
        phrase_rule(&low, "weasel", WEASELS, &mut raw, |m| {
            format!("\"{m}\" is a weasel word — give a number or cut it.")
        });
    }
    if on("adverb") {
        phrase_rule(&low, "adverb", ADVERBS, &mut raw, |m| {
            format!("\"{m}\" is a filler adverb — cut it or use a stronger verb.")
        });
    }
    if on("hedge") {
        phrase_rule(&low, "hedge", HEDGES, &mut raw, |m| {
            format!("\"{m}\" hedges — commit to the claim or drop it.")
        });
    }
    if on("jargon") {
        swap_rule(
            &low,
            "jargon",
            JARGON,
            &mut raw,
            |m, b| format!("\"{m}\" is jargon — use \"{b}\"."),
            |m| format!("\"{m}\" is jargon — cut it."),
        );
    }
    if on("wordy") {
        swap_rule(
            &low,
            "wordy",
            WORDY,
            &mut raw,
            |m, b| format!("\"{m}\" is wordy — use \"{b}\"."),
            |m| format!("\"{m}\" is filler — cut it."),
        );
    }
    if on("redundancy") {
        swap_rule(
            &low,
            "redundancy",
            REDUNDANCIES,
            &mut raw,
            |m, b| format!("\"{m}\" is redundant — use \"{b}\"."),
            |m| format!("\"{m}\" is redundant — cut it."),
        );
    }
    if on("archaism") {
        swap_rule(
            &low,
            "archaism",
            ARCHAISMS,
            &mut raw,
            |m, b| format!("\"{m}\" is archaic — use \"{b}\"."),
            |m| format!("\"{m}\" is archaic — cut it."),
        );
    }

    let toks = words(text);
    let lower_tok: Vec<String> = toks
        .iter()
        .map(|(s, e)| low[*s..*e].to_string())
        .collect();

    if on("passive") {
        for i in 0..toks.len() {
            if !BE_VERBS.contains(&lower_tok[i].as_str()) {
                continue;
            }
            let mut j = i + 1;
            while j < toks.len()
                && j <= i + 2
                && (PASSIVE_SKIPS.contains(&lower_tok[j].as_str())
                    || (lower_tok[j].ends_with("ly") && lower_tok[j].len() > 3))
            {
                j += 1;
            }
            if j >= toks.len() || j > i + 3 {
                continue;
            }
            if is_participle(&lower_tok[j]) {
                let start = toks[i].0;
                let end = toks[j].1;
                let m = &text[start..end];
                raw.push(Raw {
                    start,
                    len: end - start,
                    rule: "passive",
                    message: format!("\"{m}\" is passive voice — say who does it."),
                    suggestion: None,
                });
            }
        }
    }

    if on("uncomparable") {
        for i in 1..toks.len() {
            if !INTENSIFIERS.contains(&lower_tok[i - 1].as_str())
                || !UNCOMPARABLES.contains(&lower_tok[i].as_str())
            {
                continue;
            }
            // Only when the two words sit side by side, e.g. not "very, unique".
            let between = &text[toks[i - 1].1..toks[i].0];
            if !between.chars().all(char::is_whitespace) {
                continue;
            }
            let start = toks[i - 1].0;
            let end = toks[i].1;
            let adjective = &text[toks[i].0..toks[i].1];
            raw.push(Raw {
                start,
                len: end - start,
                rule: "uncomparable",
                message: format!(
                    "\"{}\" — \"{adjective}\" is absolute and takes no degree; cut \"{}\".",
                    &text[start..end],
                    &text[toks[i - 1].0..toks[i - 1].1]
                ),
                suggestion: Some(adjective.to_string()),
            });
        }
    }

    if on("illusion") {
        for i in 1..toks.len() {
            let (a, b) = (&lower_tok[i - 1], &lower_tok[i]);
            if a.is_empty() || a != b {
                continue;
            }
            if !a.bytes().all(|c| c.is_ascii_alphabetic()) || a == "had" || a == "that" {
                continue;
            }
            // Only a true repeat: nothing but whitespace between the two words.
            let between = &text[toks[i - 1].1..toks[i].0];
            if !between.chars().all(char::is_whitespace) {
                continue;
            }
            let start = toks[i - 1].0;
            let end = toks[i].1;
            raw.push(Raw {
                start,
                len: end - start,
                rule: "illusion",
                message: format!("\"{}\" repeats a word — delete one.", &text[start..end]),
                suggestion: None,
            });
        }
    }

    if on("eprime") {
        for (i, (s, e)) in toks.iter().enumerate() {
            let w = lower_tok[i].as_str();
            if BE_VERBS.contains(&w) || BE_CONTRACTIONS.contains(&w) {
                raw.push(Raw {
                    start: *s,
                    len: e - s,
                    rule: "eprime",
                    message: format!(
                        "\"{}\" is a form of \"to be\" — E-Prime asks for a concrete verb.",
                        &text[*s..*e]
                    ),
                    suggestion: None,
                });
            }
        }
    }

    let sents = sentences(text);
    if on("so-start") || on("there-is") || on("long-sentence") {
        for (ss, se) in &sents {
            let sent_words: Vec<usize> = (0..toks.len())
                .filter(|k| toks[*k].0 >= *ss && toks[*k].1 <= *se)
                .collect();
            if sent_words.is_empty() {
                continue;
            }
            let first = sent_words[0];
            if on("so-start") && lower_tok[first] == "so" {
                let (s, e) = toks[first];
                raw.push(Raw {
                    start: s,
                    len: e - s,
                    rule: "so-start",
                    message: format!(
                        "Sentence starts with \"{}\" — cut it and start with the point.",
                        &text[s..e]
                    ),
                    suggestion: None,
                });
            }
            if on("there-is") && sent_words.len() >= 2 {
                let second = sent_words[1];
                let a = lower_tok[first].as_str();
                let b = lower_tok[second].as_str();
                let hit = matches!(a, "there" | "here")
                    && matches!(b, "is" | "are" | "was" | "were");
                if hit {
                    let s = toks[first].0;
                    let e = toks[second].1;
                    raw.push(Raw {
                        start: s,
                        len: e - s,
                        rule: "there-is",
                        message: format!(
                            "Sentence starts with \"{}\" — name the real subject instead.",
                            &text[s..e]
                        ),
                        suggestion: None,
                    });
                }
            }
            if on("long-sentence")
                && long_sentence_words > 0
                && sent_words.len() > long_sentence_words
            {
                let (s, e) = toks[first];
                raw.push(Raw {
                    start: s,
                    len: e - s,
                    rule: "long-sentence",
                    message: format!(
                        "Sentence runs {} words (limit {long_sentence_words}) — split it.",
                        sent_words.len()
                    ),
                    suggestion: None,
                });
            }
        }
    }

    // Ignore list: drop findings whose matched text is explicitly allowed.
    let allow: Vec<String> = ignore
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let starts = line_starts(text);
    let mut issues: Vec<Issue> = raw
        .into_iter()
        .filter(|r| {
            let m = low[r.start..r.start + r.len].to_string();
            !allow.contains(&m)
        })
        .map(|r| {
            let (line, col) = position(text, &starts, r.start);
            Issue {
                line,
                col,
                rule: r.rule,
                message: r.message,
                matched: text[r.start..r.start + r.len].to_string(),
                suggestion: r.suggestion,
                start: r.start,
            }
        })
        .collect();
    issues.sort_by(|a, b| a.start.cmp(&b.start).then(a.rule.cmp(b.rule)));
    issues.dedup_by(|a, b| a.start == b.start && a.rule == b.rule && a.matched == b.matched);
    issues
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn rule_counts(issues: &[Issue]) -> Vec<RuleCount> {
    let mut all: Vec<&'static str> = DEFAULT_RULES.to_vec();
    all.extend_from_slice(OPT_IN_RULES);
    all.sort_unstable();
    all.into_iter()
        .filter_map(|r| {
            let count = issues.iter().filter(|i| i.rule == r).count();
            (count > 0).then_some(RuleCount { rule: r, count })
        })
        .collect()
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

fn render_report(
    shown: &[Issue],
    total: usize,
    word_count: usize,
    sentence_count: usize,
    checks: &[&'static str],
) -> String {
    let mut out = String::new();
    if total == 0 {
        out.push_str(&format!(
            "No issues found in {} / {}.\n",
            plural(word_count, "word", "words"),
            plural(sentence_count, "sentence", "sentences")
        ));
    } else {
        out.push_str(&format!(
            "{} found in {} / {}.\n",
            plural(total, "issue", "issues"),
            plural(word_count, "word", "words"),
            plural(sentence_count, "sentence", "sentences")
        ));
    }

    if !shown.is_empty() {
        let pos: Vec<String> = shown
            .iter()
            .map(|i| format!("{}:{}", i.line, i.col))
            .collect();
        let wpos = pos.iter().map(|p| p.len()).chain([8]).max().unwrap();
        let wrule = shown
            .iter()
            .map(|i| i.rule.len())
            .chain([4])
            .max()
            .unwrap();
        out.push('\n');
        out.push_str(&format!(
            "{:<wpos$}  {:<wrule$}  ISSUE\n",
            "LINE:COL",
            "RULE",
            wpos = wpos,
            wrule = wrule
        ));
        for (i, issue) in shown.iter().enumerate() {
            out.push_str(&format!(
                "{:<wpos$}  {:<wrule$}  {}\n",
                pos[i],
                issue.rule,
                issue.message,
                wpos = wpos,
                wrule = wrule
            ));
        }
        if shown.len() < total {
            out.push_str(&format!(
                "\nShowing the first {} of {total} issues — raise max_issues to see the rest.\n",
                shown.len()
            ));
        }

        let counts = rule_counts(shown);
        let wr = counts.iter().map(|c| c.rule.len()).max().unwrap_or(4);
        out.push_str("\nBY RULE\n");
        for c in &counts {
            out.push_str(&format!("{:<wr$}  {}\n", c.rule, c.count, wr = wr));
        }
    }

    out.push_str(&format!("\nChecks run: {}\n", checks.join(", ")));
    out
}

fn render_annotated(
    text: &str,
    shown: &[Issue],
    total: usize,
    word_count: usize,
    sentence_count: usize,
    checks: &[&'static str],
) -> String {
    let mut out = String::new();
    if total == 0 {
        out.push_str(&format!(
            "No issues found in {} / {}.\n",
            plural(word_count, "word", "words"),
            plural(sentence_count, "sentence", "sentences")
        ));
        out.push_str(&format!("\nChecks run: {}\n", checks.join(", ")));
        return out;
    }
    out.push_str(&format!(
        "{} found in {} / {}.\n\n",
        plural(total, "issue", "issues"),
        plural(word_count, "word", "words"),
        plural(sentence_count, "sentence", "sentences")
    ));

    let lines: Vec<&str> = text.split('\n').collect();
    let gutter = shown
        .iter()
        .map(|i| i.line.to_string().len())
        .max()
        .unwrap_or(1);

    let mut last_line = 0usize;
    for issue in shown {
        if issue.line != last_line {
            if last_line != 0 {
                out.push('\n');
            }
            let src = lines.get(issue.line - 1).copied().unwrap_or("");
            out.push_str(&format!(
                "{:>gutter$} | {}\n",
                issue.line,
                src.trim_end_matches('\r'),
                gutter = gutter
            ));
            last_line = issue.line;
        }
        let carets = "^".repeat(issue.matched.chars().count().max(1));
        out.push_str(&format!(
            "{:>gutter$} | {}{} {}: {}\n",
            "",
            " ".repeat(issue.col - 1),
            carets,
            issue.rule,
            issue.message,
            gutter = gutter
        ));
    }
    if shown.len() < total {
        out.push_str(&format!(
            "\nShowing the first {} of {total} issues — raise max_issues to see the rest.\n",
            shown.len()
        ));
    }
    out.push_str(&format!("\nChecks run: {}\n", checks.join(", ")));
    out
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Lint `text` and render the result in the requested format.
pub fn analyze(
    text: &str,
    checks_spec: &str,
    output: OutputFormat,
    ignore: &str,
    max_issues: usize,
    long_sentence_words: usize,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("text is empty — paste the writing you want linted".into());
    }
    if text.len() > MAX_BYTES {
        return Err(format!(
            "text is {} bytes; the limit is {MAX_BYTES} bytes",
            text.len()
        ));
    }
    let checks = resolve_checks(checks_spec)?;
    let issues = lint(text, &checks, ignore, long_sentence_words);
    let total = issues.len();
    let shown: &[Issue] = if max_issues == 0 || total <= max_issues {
        &issues
    } else {
        &issues[..max_issues]
    };
    let word_count = words(text).len();
    let sentence_count = sentences(text).len();

    Ok(match output {
        OutputFormat::Report => render_report(shown, total, word_count, sentence_count, &checks),
        OutputFormat::Annotated => {
            render_annotated(text, shown, total, word_count, sentence_count, &checks)
        }
        OutputFormat::Json => {
            let report = JsonReport {
                issues: shown,
                total,
                shown: shown.len(),
                truncated: shown.len() < total,
                counts: rule_counts(shown),
                words: word_count,
                sentences: sentence_count,
                checks: &checks,
            };
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_report_lists_every_rule_hit() {
        let out = analyze(
            "So the cat was stolen at the end of the day.",
            "all",
            OutputFormat::Report,
            "",
            0,
            30,
        )
        .unwrap();
        assert!(out.starts_with("3 issues found in 11 words / 1 sentence.\n"), "{out}");
        assert!(out.contains("1:1       so-start"), "{out}");
        assert!(out.contains("\"was stolen\" is passive voice"), "{out}");
        assert!(out.contains("\"at the end of the day\" is a cliché"), "{out}");
        assert!(out.contains("\nBY RULE\n"), "{out}");
        assert!(out.ends_with("Checks run: adverb, archaism, cliche, hedge, illusion, jargon, long-sentence, passive, redundancy, so-start, there-is, uncomparable, weasel, wordy\n"), "{out}");
    }

    #[test]
    fn error_on_empty_text() {
        let err = analyze("   \n ", "all", OutputFormat::Report, "", 0, 30).unwrap_err();
        assert!(err.contains("text is empty"), "{err}");
    }

    #[test]
    fn error_on_unknown_check() {
        let err = analyze("Hello there.", "spelling", OutputFormat::Report, "", 0, 30).unwrap_err();
        assert!(err.contains("unknown check 'spelling'"), "{err}");
        assert!(err.contains("long-sentence"), "{err}");
    }

    #[test]
    fn error_on_oversize_text() {
        let big = "word ".repeat(MAX_BYTES / 5 + 10);
        let err = analyze(&big, "all", OutputFormat::Report, "", 0, 30).unwrap_err();
        assert!(err.contains("the limit is 1000000 bytes"), "{err}");
    }

    #[test]
    fn error_when_no_checks_selected() {
        let err = analyze("Hello there.", "none", OutputFormat::Report, "", 0, 30).unwrap_err();
        assert!(err.contains("no checks selected"), "{err}");
    }

    #[test]
    fn resolve_checks_supports_all_and_removal() {
        assert_eq!(resolve_checks("all").unwrap(), DEFAULT_RULES.to_vec());
        assert_eq!(
            resolve_checks("cliche, weasel").unwrap(),
            vec!["cliche", "weasel"]
        );
        let no_passive = resolve_checks("all,-passive").unwrap();
        assert!(!no_passive.contains(&"passive"));
        assert_eq!(no_passive.len(), DEFAULT_RULES.len() - 1);
        // eprime is opt-in: never in `all`, available by name.
        assert!(!resolve_checks("all").unwrap().contains(&"eprime"));
        assert_eq!(resolve_checks("eprime").unwrap(), vec!["eprime"]);
        // Canonical order regardless of the order asked for.
        assert_eq!(
            resolve_checks("weasel,cliche,adverb").unwrap(),
            vec!["adverb", "cliche", "weasel"]
        );
    }

    #[test]
    fn passive_detects_irregular_participles_and_skips_adverbs() {
        let hits = lint("The report was written. It is clearly given away.", &["passive"], "", 0);
        let m: Vec<&str> = hits.iter().map(|i| i.matched.as_str()).collect();
        assert_eq!(m, vec!["was written", "is clearly given"]);
    }

    #[test]
    fn passive_ignores_plain_adjectives() {
        let hits = lint("The sky is blue and the box is red.", &["passive"], "", 0);
        assert!(hits.is_empty(), "{hits:?}");
    }

    #[test]
    fn wordy_and_redundancy_carry_suggestions() {
        let hits = lint(
            "In order to plan ahead we need a brief summary.",
            &["redundancy", "wordy"],
            "",
            0,
        );
        let pairs: Vec<(&str, Option<&str>)> = hits
            .iter()
            .map(|i| (i.matched.as_str(), i.suggestion.as_deref()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("In order to", Some("to")),
                ("plan ahead", Some("plan")),
                ("brief summary", Some("summary")),
            ]
        );
    }

    #[test]
    fn ignore_list_suppresses_a_match() {
        let base = lint("This is very good.", &["weasel"], "", 0);
        assert_eq!(base.len(), 1);
        let filtered = lint("This is very good.", &["weasel"], "very", 0);
        assert!(filtered.is_empty(), "{filtered:?}");
    }

    #[test]
    fn word_boundaries_prevent_substring_matches() {
        // "few" must not fire inside "fewer"; "many" not inside "germany".
        let hits = lint("Fewer people left Germany.", &["weasel"], "", 0);
        assert!(hits.is_empty(), "{hits:?}");
    }

    #[test]
    fn illusion_flags_a_real_repeat_only() {
        let hits = lint("We we shipped it. She had had enough.", &["illusion"], "", 0);
        let m: Vec<&str> = hits.iter().map(|i| i.matched.as_str()).collect();
        assert_eq!(m, vec!["We we"]);
    }

    #[test]
    fn there_is_and_long_sentence_use_sentence_starts() {
        let text = "There are many reasons. one two three four five six.";
        let hits = lint(text, &["long-sentence", "there-is"], "", 5);
        let pairs: Vec<(&str, &str)> = hits.iter().map(|i| (i.rule, i.matched.as_str())).collect();
        assert_eq!(pairs, vec![("there-is", "There are"), ("long-sentence", "one")]);
    }

    #[test]
    fn abbreviations_do_not_split_sentences() {
        assert_eq!(sentences("Dr. Wu shipped it. Then we left.").len(), 2);
    }

    #[test]
    fn line_and_column_are_one_based_characters() {
        let hits = lint("ok\n  naïve very good", &["weasel"], "", 0);
        assert_eq!(hits.len(), 1);
        assert_eq!((hits[0].line, hits[0].col), (2, 9));
    }

    #[test]
    fn max_issues_truncates_and_says_so() {
        let text = "It is very very very good.";
        let out = analyze(text, "weasel", OutputFormat::Report, "", 2, 0).unwrap();
        assert!(out.starts_with("3 issues found"), "{out}");
        assert!(out.contains("Showing the first 2 of 3 issues"), "{out}");
    }

    #[test]
    fn annotated_output_places_carets_under_the_match() {
        let out = analyze("So we shipped.", "so-start", OutputFormat::Annotated, "", 0, 0).unwrap();
        assert!(out.contains("1 | So we shipped."), "{out}");
        assert!(out.contains("  | ^^ so-start:"), "{out}");
    }

    #[test]
    fn json_output_is_machine_readable() {
        let out = analyze("This is very good.", "weasel", OutputFormat::Json, "", 0, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["total"], 1);
        assert_eq!(v["truncated"], false);
        assert_eq!(v["issues"][0]["rule"], "weasel");
        assert_eq!(v["issues"][0]["match"], "very");
        assert_eq!(v["issues"][0]["line"], 1);
        assert_eq!(v["issues"][0]["col"], 9);
        assert_eq!(v["counts"][0]["rule"], "weasel");
        assert_eq!(v["checks"][0], "weasel");
    }

    #[test]
    fn clean_text_reports_no_issues() {
        let out = analyze("The team shipped the release on Friday.", "all", OutputFormat::Report, "", 0, 30).unwrap();
        assert!(out.starts_with("No issues found in 7 words / 1 sentence.\n"), "{out}");
    }

    #[test]
    fn output_format_parse_rejects_junk() {
        assert_eq!(OutputFormat::parse("JSON").unwrap(), OutputFormat::Json);
        assert!(OutputFormat::parse("html").unwrap_err().contains("unknown output"));
    }
}
