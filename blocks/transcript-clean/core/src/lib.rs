//! transcript-clean core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps. Turns a raw speech-to-text / captions
//! transcript into clean prose: drops timestamps and caption scaffolding, strips
//! non-verbal cue markers, removes filler words and stutters, merges consecutive
//! same-speaker turns, and normalizes punctuation + capitalization.
//!
//! Everything is DETERMINISTIC — fixed word lists and rules, no LLM, no network.
//! That means the aggressive filler level and the discourse-marker lists can
//! over-strip genuine words ("like", "right"); the page states the trade-off.

use regex::Regex;
use std::sync::LazyLock;

// ---- Filler word lists ------------------------------------------------------

/// Unambiguous non-word interjections. Removed at the `standard` level. These are
/// vocalized pauses that are never meaningful content, so removing them is safe.
const STANDARD_FILLERS: &[&str] = &[
    "um", "umm", "ummm", "uh", "uhh", "uhhh", "uhm", "erm", "er", "hmm", "hmmm",
    "mmm", "mhm", "mm-hmm", "uh-huh",
];

/// Discourse markers added at the `aggressive` level (on top of `standard`).
/// These CAN be meaningful words, so removing them is opt-in and may over-strip.
const AGGRESSIVE_FILLERS: &[&str] = &[
    "you know", "i mean", "kind of", "sort of", "like", "basically", "actually",
    "literally", "seriously", "honestly", "right", "i guess", "you see",
];

// ---- Static regexes ---------------------------------------------------------

// A bracketed clock timestamp: [00:01:23], (1:02:03.456), (00:00:04,000).
static TS_BRACKET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\[(]\s*\d{1,2}:\d{2}(?::\d{2})?(?:[.,]\d{1,3})?\s*[\])]").unwrap()
});
// A leading bare timestamp (with an optional trailing dash/arrow) at line start.
static TS_LEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*\d{1,2}:\d{2}(?::\d{2})?(?:[.,]\d{1,3})?\s*[-\u{2013}\u{2014}>]*\s*").unwrap()
});
// Bracketed / parenthesized annotation spans, e.g. [laughter], (applause).
static SQUARE_SPAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[[^\]]*\]").unwrap());
static PAREN_SPAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\([^)]*\)").unwrap());
// Speaker label forms.
static SPK_ARROWS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*>+\s*").unwrap());
static SPK_BRACKET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\[([^\]]{1,40})\]\s*:\s*(.*)$").unwrap());
static SPK_PLAIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([A-Za-z][A-Za-z0-9 _'\-]{0,38}?)\s*:\s+(.*)$").unwrap());
// Hyphenated stutter candidate: word(-word)+ , e.g. I-I-I, w-w-what, self-service.
static STUTTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\w+(?:-\w+)+\b").unwrap());
// Standalone "i" and its contractions, for capitalization.
static LONE_I: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bi\b").unwrap());
static I_CONTRACTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bi(['\u{2019}])(m|ll|ve|d|re)\b").unwrap());
// Punctuation-spacing helpers.
static SPACE_BEFORE_PUNCT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+([,.!?;:])").unwrap());
static COMMA_NO_SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([,;])(\w)").unwrap());
static SENT_NO_SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([.!?])([A-Za-z])").unwrap());
static MULTISPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]{2,}").unwrap());
static REPEAT_PUNCT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([,;])[,;]+").unwrap());

/// One parsed transcript turn: an optional speaker plus its cleaned text.
struct Turn {
    speaker: Option<String>,
    text: String,
}

/// Clean a raw transcript. `filler_level` is one of `off` | `standard` |
/// `aggressive`; `extra_fillers` is a comma-separated list of extra words/phrases
/// removed at every level. Returns the cleaned transcript.
#[allow(clippy::too_many_arguments)]
pub fn clean(
    input: &str,
    filler_level: &str,
    extra_fillers: &str,
    remove_timestamps: bool,
    remove_brackets: bool,
    merge_speakers: bool,
    fix_capitalization: bool,
    fix_punctuation: bool,
) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty — paste a transcript to clean".into());
    }
    let level = filler_level.trim().to_ascii_lowercase();
    let level = if level.is_empty() { "standard" } else { level.as_str() };
    if !matches!(level, "off" | "standard" | "aggressive") {
        return Err(format!(
            "filler_level must be one of off, standard, aggressive (got '{filler_level}')"
        ));
    }

    let filler_re = build_filler_regex(level, extra_fillers);

    let mut turns: Vec<Turn> = Vec::new();
    for raw_line in input.lines() {
        // 1. Timestamp / caption-scaffolding line handling.
        if remove_timestamps && is_timestamp_line(raw_line) {
            continue;
        }
        let mut line = raw_line.to_string();
        if remove_timestamps {
            line = TS_BRACKET.replace_all(&line, " ").into_owned();
            line = TS_LEADING.replace(&line, "").into_owned();
        }

        // 2. Speaker label extraction (before bracket stripping, so a `[Name]:`
        //    label survives even when bracket removal is on).
        let (speaker, mut text) = parse_speaker(&line);

        // 3. Bracketed / parenthesized non-verbal cue markers.
        if remove_brackets {
            text = SQUARE_SPAN.replace_all(&text, " ").into_owned();
            text = PAREN_SPAN.replace_all(&text, " ").into_owned();
        }

        // 4. Stutters + filler words.
        text = collapse_stutters(&text);
        if let Some(re) = &filler_re {
            text = re.replace_all(&text, " ").into_owned();
        }

        // 5. Punctuation + spacing.
        if fix_punctuation {
            text = fix_punctuation_spacing(&text);
        } else {
            text = MULTISPACE.replace_all(text.trim(), " ").into_owned();
        }

        // 6. Capitalization.
        if fix_capitalization {
            text = fix_caps(&text);
        }

        let text = text.trim().to_string();
        if text.is_empty() {
            continue;
        }
        turns.push(Turn { speaker, text });
    }

    // 7. Merge consecutive same-speaker turns.
    if merge_speakers {
        turns = merge_same_speaker(turns);
    }

    // 8. Render + drop consecutive duplicate lines.
    let mut out: Vec<String> = Vec::new();
    for t in &turns {
        let rendered = match &t.speaker {
            Some(s) => format!("{s}: {}", t.text),
            None => t.text.clone(),
        };
        if out.last().map(|l| l == &rendered).unwrap_or(false) {
            continue;
        }
        out.push(rendered);
    }

    Ok(out.join("\n"))
}

/// Whether a whole line is caption scaffolding to drop: an SRT sequence index,
/// the `WEBVTT` header, or an SRT/VTT `-->` cue-timing line.
fn is_timestamp_line(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    if t.chars().all(|c| c.is_ascii_digit()) {
        return true; // SRT sequence number
    }
    if t == "WEBVTT" || t.starts_with("WEBVTT") {
        return true;
    }
    if t.contains("-->") {
        return true; // cue timing line
    }
    false
}

/// Extract a leading speaker label. Returns (speaker, remaining text). Handles
/// `>> Name:`, `[Name]:`, and `Name:` (name ≤ 4 tokens) forms.
fn parse_speaker(line: &str) -> (Option<String>, String) {
    let line = SPK_ARROWS.replace(line, "").into_owned();
    if let Some(c) = SPK_BRACKET.captures(&line) {
        let name = c.get(1).unwrap().as_str().trim().to_string();
        let text = c.get(2).unwrap().as_str().to_string();
        if !name.is_empty() {
            return (Some(name), text);
        }
    }
    if let Some(c) = SPK_PLAIN.captures(&line) {
        let name = c.get(1).unwrap().as_str().trim().to_string();
        if name.split_whitespace().count() <= 4 {
            let text = c.get(2).unwrap().as_str().to_string();
            return (Some(name), text);
        }
    }
    (None, line)
}

/// Build the filler-removal regex for a level + custom list, or None if nothing
/// would be removed. Alternatives are sorted longest-first so multi-word phrases
/// match before any prefix.
fn build_filler_regex(level: &str, extra: &str) -> Option<Regex> {
    let mut words: Vec<String> = Vec::new();
    if level == "standard" || level == "aggressive" {
        words.extend(STANDARD_FILLERS.iter().map(|s| s.to_string()));
    }
    if level == "aggressive" {
        words.extend(AGGRESSIVE_FILLERS.iter().map(|s| s.to_string()));
    }
    for w in extra.split(',') {
        let w = w.trim().to_ascii_lowercase();
        if !w.is_empty() {
            words.push(w);
        }
    }
    if words.is_empty() {
        return None;
    }
    words.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
    words.dedup();
    let alt = words.iter().map(|w| regex::escape(w)).collect::<Vec<_>>().join("|");
    // Whole-word, case-insensitive; also eat one trailing comma so "um, so" tidies.
    Regex::new(&format!(r"(?i)\b(?:{alt})\b,?")).ok()
}

/// Collapse hyphenated stutters (I-I-I → I, w-w-what → what) while leaving real
/// hyphenated words (self-service, twenty-one) untouched: a run is a stutter only
/// when every fragment but the last is a case-insensitive prefix of the last.
fn collapse_stutters(text: &str) -> String {
    STUTTER
        .replace_all(text, |caps: &regex::Captures| {
            let tok = caps.get(0).unwrap().as_str();
            let frags: Vec<&str> = tok.split('-').collect();
            let last = *frags.last().unwrap();
            let last_lc = last.to_ascii_lowercase();
            let is_stutter = frags.len() >= 2
                && frags[..frags.len() - 1]
                    .iter()
                    .all(|f| !f.is_empty() && last_lc.starts_with(&f.to_ascii_lowercase()));
            if is_stutter {
                last.to_string()
            } else {
                tok.to_string()
            }
        })
        .into_owned()
}

/// Normalize spacing and terminal punctuation.
fn fix_punctuation_spacing(text: &str) -> String {
    let mut s = MULTISPACE.replace_all(text, " ").into_owned();
    s = REPEAT_PUNCT.replace_all(&s, "$1").into_owned();
    s = SPACE_BEFORE_PUNCT.replace_all(&s, "$1").into_owned();
    s = COMMA_NO_SPACE.replace_all(&s, "$1 $2").into_owned();
    s = SENT_NO_SPACE.replace_all(&s, "$1 $2").into_owned();
    s = MULTISPACE.replace_all(&s, " ").into_owned();
    let mut s = s
        .trim()
        .trim_start_matches(|c| c == ',' || c == ';' || c == ':')
        .trim()
        .to_string();
    // Ensure the line ends with a terminal mark when it ends on a word char.
    if s.chars().next_back().map(|c| c.is_alphanumeric()).unwrap_or(false) {
        s.push('.');
    }
    s
}

/// Capitalize sentence starts and fix the pronoun "I".
fn fix_caps(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cap_next = true;
    for ch in text.chars() {
        if cap_next && ch.is_alphabetic() {
            out.extend(ch.to_uppercase());
            cap_next = false;
        } else {
            out.push(ch);
            if ch.is_alphanumeric() {
                cap_next = false;
            } else if ch == '.' || ch == '!' || ch == '?' {
                cap_next = true;
            }
        }
    }
    let out = I_CONTRACTION.replace_all(&out, "I$1$2").into_owned();
    LONE_I.replace_all(&out, "I").into_owned()
}

/// Merge consecutive turns that share the same speaker (case-insensitive).
fn merge_same_speaker(turns: Vec<Turn>) -> Vec<Turn> {
    let mut out: Vec<Turn> = Vec::new();
    for t in turns {
        if let (Some(prev), Some(cur)) = (out.last(), &t.speaker) {
            if let Some(ps) = &prev.speaker {
                if ps.eq_ignore_ascii_case(cur) {
                    let last = out.last_mut().unwrap();
                    last.text.push(' ');
                    last.text.push_str(&t.text);
                    continue;
                }
            }
        }
        out.push(t);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(input: &str) -> String {
        clean(input, "standard", "", true, true, true, true, true).unwrap()
    }

    #[test]
    fn removes_standard_fillers_and_fixes_caps() {
        let got = run("um so i think uh we should ship");
        assert_eq!(got, "So I think we should ship.");
    }

    #[test]
    fn strips_timestamps_and_cue_markers() {
        let input = "[00:01:23] John: um hello [laughter]\n[00:01:25] John: how are you";
        let got = run(input);
        assert_eq!(got, "John: Hello. How are you.");
    }

    #[test]
    fn merges_consecutive_same_speaker() {
        let input = "Alice: first point\nAlice: second point\nBob: a reply";
        let got = run(input);
        assert_eq!(got, "Alice: First point. Second point.\nBob: A reply.");
    }

    #[test]
    fn drops_srt_scaffolding() {
        let input = "1\n00:00:01,000 --> 00:00:04,000\nWEBVTT\nHello there";
        let got = run(input);
        assert_eq!(got, "Hello there.");
    }

    #[test]
    fn collapses_stutters_but_keeps_real_hyphenates() {
        let got = run("i-i-i want a self-service option");
        assert_eq!(got, "I want a self-service option.");
    }

    #[test]
    fn aggressive_removes_discourse_markers() {
        let got = clean(
            "like, basically we you know need to actually decide",
            "aggressive",
            "",
            true,
            true,
            true,
            true,
            true,
        )
        .unwrap();
        assert_eq!(got, "We need to decide.");
    }

    #[test]
    fn off_level_keeps_fillers() {
        let got = clean("um hello", "off", "", true, true, true, true, true).unwrap();
        assert_eq!(got, "Um hello.");
    }

    #[test]
    fn extra_fillers_apply_at_off_level() {
        let got = clean("wowza hello there", "off", "wowza", true, true, true, true, true).unwrap();
        assert_eq!(got, "Hello there.");
    }

    #[test]
    fn dedups_consecutive_duplicate_lines() {
        let got = run("Same line here\nSame line here\nDifferent");
        assert_eq!(got, "Same line here.\nDifferent.");
    }

    #[test]
    fn empty_input_errors() {
        assert!(clean("   ", "standard", "", true, true, true, true, true).is_err());
    }

    #[test]
    fn invalid_level_errors() {
        assert!(clean("hi", "loud", "", true, true, true, true, true).is_err());
    }

    #[test]
    fn keeps_timestamps_when_disabled() {
        let got = clean("[00:01] hello", "standard", "", false, false, true, true, true).unwrap();
        assert_eq!(got, "[00:01] hello.");
    }
}
