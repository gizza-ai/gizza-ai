//! context-trimmer core — trim text to fit an approximate LLM token budget,
//! keeping the head, tail, middle, or both ends of the text.
//!
//! Pure-Rust, dependency-free. No real tokenizer runs in the browser, so the
//! token count is an APPROXIMATION: `tokens ≈ characters ÷ chars_per_token`
//! (default 4.0, OpenAI's English rule of thumb). Set `chars_per_token` lower
//! for code / non-English text if you want a more conservative fit.
//!
//! Strategy (`Keep`):
//! - `Head`   — keep the beginning, drop the end (append the marker).
//! - `Tail`   — keep the end, drop the beginning (prepend the marker).
//! - `Middle` — keep the centre, drop both ends (marker on each side).
//! - `HeadTail` — keep the beginning AND the end, drop the middle (marker
//!   between). `head_ratio` splits the budget between the two ends.
//!
//! When `break_words` is false (default) each cut is backed up / forwarded to a
//! whitespace boundary so a word is never split. If the text already fits the
//! budget it is returned unchanged, with NO marker inserted.

/// Minimum accepted token budget.
pub const MIN_TOKENS: u32 = 1;
/// Maximum accepted token budget (guards against absurd inputs).
pub const MAX_TOKENS: u32 = 1_000_000;
/// Minimum accepted `chars_per_token` ratio.
pub const MIN_CPT: f64 = 1.0;
/// Maximum accepted `chars_per_token` ratio.
pub const MAX_CPT: f64 = 20.0;

/// Which part of the text to keep.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Keep {
    Head,
    Tail,
    Middle,
    HeadTail,
}

impl Keep {
    /// Parse the strategy string (case-insensitive, a few friendly aliases).
    pub fn parse(s: &str) -> Result<Keep, String> {
        match s.trim().to_ascii_lowercase().replace(['-', ' '], "_").as_str() {
            "head" | "start" | "beginning" | "first" => Ok(Keep::Head),
            "tail" | "end" | "last" => Ok(Keep::Tail),
            "middle" | "center" | "centre" => Ok(Keep::Middle),
            "head_tail" | "headtail" | "both" | "ends" | "both_ends" => Ok(Keep::HeadTail),
            other => Err(format!(
                "keep must be one of \"head\", \"tail\", \"middle\", \"head_tail\" (got {other:?})"
            )),
        }
    }
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// Approximate token count for `text` at the given ratio: `ceil(chars / cpt)`.
pub fn estimate_tokens(text: &str, chars_per_token: f64) -> u64 {
    let cpt = chars_per_token.clamp(MIN_CPT, MAX_CPT);
    (char_len(text) as f64 / cpt).ceil() as u64
}

/// First `budget` chars; when `break_words` is false back up to the last
/// whitespace so a word is not split (unless the cut already lands on a word
/// boundary). Trailing whitespace is trimmed.
fn take_head(chars: &[char], budget: usize, break_words: bool) -> String {
    if budget >= chars.len() {
        return chars.iter().collect();
    }
    let mut kept: String = chars[..budget].iter().collect();
    // Only back up if the cut splits a word (the next char is not whitespace).
    if !break_words && !chars[budget].is_whitespace() {
        if let Some(pos) = kept.rfind(char::is_whitespace) {
            if !kept[..pos].trim().is_empty() {
                kept.truncate(pos);
            }
        }
    }
    kept.trim_end().to_string()
}

/// Last `budget` chars; when `break_words` is false forward past a partial
/// leading word so a word is not split (unless the cut already lands on a word
/// boundary). Leading whitespace is trimmed.
fn take_tail(chars: &[char], budget: usize, break_words: bool) -> String {
    if budget >= chars.len() {
        return chars.iter().collect();
    }
    let start = chars.len() - budget;
    let mut kept: String = chars[start..].iter().collect();
    // Only forward if the cut splits a word (the preceding char isn't whitespace).
    if !break_words && !chars[start - 1].is_whitespace() {
        if let Some(pos) = kept.find(char::is_whitespace) {
            // Keep everything AFTER the first whitespace (drops the partial word).
            let after = pos + kept[pos..].chars().next().map(char::len_utf8).unwrap_or(1);
            if !kept[after..].trim().is_empty() {
                kept = kept[after..].to_string();
            }
        }
    }
    kept.trim_start().to_string()
}

/// Central `budget` chars, dropping (len-budget) chars split across both ends.
fn take_middle(chars: &[char], budget: usize, break_words: bool) -> String {
    if budget >= chars.len() {
        return chars.iter().collect();
    }
    let drop = chars.len() - budget;
    let left = drop / 2;
    let slice = &chars[left..left + budget];
    let mut kept: String = slice.iter().collect();
    if !break_words {
        // Trim a partial leading word and a partial trailing word.
        if let Some(pos) = kept.find(char::is_whitespace) {
            let after = pos + kept[pos..].chars().next().map(char::len_utf8).unwrap_or(1);
            if !kept[after..].trim().is_empty() {
                kept = kept[after..].to_string();
            }
        }
        if let Some(pos) = kept.rfind(char::is_whitespace) {
            if !kept[..pos].trim().is_empty() {
                kept.truncate(pos);
            }
        }
    }
    kept.trim().to_string()
}

/// Trim `text` to fit approximately `max_tokens` tokens, keeping the part named
/// by `keep`. `marker` is inserted where text is removed and its length counts
/// toward the budget so the result stays within `max_tokens`. `head_ratio`
/// (0.0–1.0) splits the budget between the two ends when `keep` is `HeadTail`.
///
/// Errors if `max_tokens` is outside `MIN_TOKENS..=MAX_TOKENS` or `head_ratio`
/// is outside `0.0..=1.0`.
pub fn trim(
    text: &str,
    max_tokens: u32,
    chars_per_token: f64,
    keep: Keep,
    marker: &str,
    head_ratio: f64,
    break_words: bool,
) -> Result<String, String> {
    if !(MIN_TOKENS..=MAX_TOKENS).contains(&max_tokens) {
        return Err(format!(
            "max_tokens must be between {MIN_TOKENS} and {MAX_TOKENS} (got {max_tokens})"
        ));
    }
    if !(0.0..=1.0).contains(&head_ratio) {
        return Err(format!(
            "head_ratio must be between 0.0 and 1.0 (got {head_ratio})"
        ));
    }
    let cpt = chars_per_token.clamp(MIN_CPT, MAX_CPT);

    let chars: Vec<char> = text.chars().collect();
    // Total character budget the token budget maps to.
    let target_chars = (max_tokens as f64 * cpt).floor() as usize;

    // Already fits → unchanged, no marker.
    if chars.len() <= target_chars {
        return Ok(text.to_string());
    }

    let marker_chars = char_len(marker);
    let markers = match keep {
        Keep::Middle => 2,
        _ => 1,
    };
    // Reserve room for the marker(s) so the final result stays within budget.
    let content_budget = target_chars.saturating_sub(marker_chars * markers);

    let out = match keep {
        Keep::Head => {
            let head = take_head(&chars, content_budget, break_words);
            format!("{head}{marker}")
        }
        Keep::Tail => {
            let tail = take_tail(&chars, content_budget, break_words);
            format!("{marker}{tail}")
        }
        Keep::Middle => {
            let mid = take_middle(&chars, content_budget, break_words);
            format!("{marker}{mid}{marker}")
        }
        Keep::HeadTail => {
            let head_budget = (content_budget as f64 * head_ratio).round() as usize;
            let tail_budget = content_budget.saturating_sub(head_budget);
            let head = take_head(&chars, head_budget, break_words);
            let tail = take_tail(&chars, tail_budget, break_words);
            format!("{head}{marker}{tail}")
        }
    };
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // "the quick brown fox jumps over the lazy dog" is 43 chars.
    const SENT: &str = "the quick brown fox jumps over the lazy dog";

    #[test]
    fn already_fits_is_unchanged() {
        // 43 chars, budget 20 tokens * 4 = 80 chars → fits, unchanged, no marker.
        let got = trim(SENT, 20, 4.0, Keep::Head, "…", 0.5, false).unwrap();
        assert_eq!(got, SENT);
    }

    #[test]
    fn keep_head_backs_up_to_word() {
        // budget 3 tokens * 4 = 12 chars; marker "…" (1) reserved → 11 content.
        // "the quick b" backed up to "the quick" + "…".
        let got = trim(SENT, 3, 4.0, Keep::Head, "…", 0.5, false).unwrap();
        assert_eq!(got, "the quick…");
    }

    #[test]
    fn keep_head_break_words() {
        let got = trim(SENT, 3, 4.0, Keep::Head, "…", 0.5, true).unwrap();
        assert_eq!(got, "the quick b…");
    }

    #[test]
    fn keep_tail_backs_up_to_word() {
        // 11 content chars from the end = "he lazy dog"; drop the partial leading
        // word → "lazy dog", prefixed with the marker.
        let got = trim(SENT, 3, 4.0, Keep::Tail, "…", 0.5, false).unwrap();
        assert_eq!(got, "…lazy dog");
    }

    #[test]
    fn keep_middle_wraps_with_markers() {
        let got = trim(SENT, 3, 4.0, Keep::Middle, "…", 0.5, false).unwrap();
        assert!(got.starts_with('…') && got.ends_with('…'), "got {got:?}");
        // The centre slice comes from the middle of the text.
        assert!(got.contains("fox") || got.contains("jumps"), "got {got:?}");
    }

    #[test]
    fn keep_head_tail_keeps_both_ends() {
        // budget 5 tokens * 4 = 20 chars; marker 1 → 19 content; head_ratio 0.5
        // → head 10 (round), tail 9. Head "the quick"; tail "lazy dog".
        let got = trim(SENT, 5, 4.0, Keep::HeadTail, "…", 0.5, false).unwrap();
        assert_eq!(got, "the quick…lazy dog");
        // Result stays within the token budget.
        assert!(estimate_tokens(&got, 4.0) <= 5);
    }

    #[test]
    fn head_ratio_shifts_the_split() {
        // All budget to the head → tail empty. Budget 19 lands exactly after
        // "fox" (a clean word boundary), so the whole word is kept.
        let got = trim(SENT, 5, 4.0, Keep::HeadTail, "…", 1.0, false).unwrap();
        assert_eq!(got, "the quick brown fox…");
    }

    #[test]
    fn empty_marker_is_hard_cut() {
        let got = trim(SENT, 3, 4.0, Keep::Head, "", 0.5, true).unwrap();
        assert_eq!(got, "the quick br"); // 3*4 = 12 chars, no marker reserved
    }

    #[test]
    fn unicode_counted_by_chars() {
        let t = "ééééé ààààà ììììì";
        // 17 chars; budget 1 token * 4 = 4 chars; marker "…" (1) → 3 content,
        // "ééé" (break_words) + "…".
        let got = trim(t, 1, 4.0, Keep::Head, "…", 0.5, true).unwrap();
        assert_eq!(got, "ééé…");
    }

    #[test]
    fn estimate_tokens_ceils() {
        assert_eq!(estimate_tokens("", 4.0), 0);
        assert_eq!(estimate_tokens("abcd", 4.0), 1);
        assert_eq!(estimate_tokens("abcde", 4.0), 2); // 5/4 = 1.25 → 2
    }

    #[test]
    fn rejects_bad_budget() {
        assert!(trim("x", 0, 4.0, Keep::Head, "…", 0.5, false).is_err());
        assert!(trim("x", MAX_TOKENS + 1, 4.0, Keep::Head, "…", 0.5, false).is_err());
    }

    #[test]
    fn rejects_bad_head_ratio() {
        assert!(trim("x", 5, 4.0, Keep::HeadTail, "…", 1.5, false).is_err());
        assert!(trim("x", 5, 4.0, Keep::HeadTail, "…", -0.1, false).is_err());
    }

    #[test]
    fn keep_parse_aliases() {
        assert_eq!(Keep::parse("Head").unwrap(), Keep::Head);
        assert_eq!(Keep::parse("both ends").unwrap(), Keep::HeadTail);
        assert_eq!(Keep::parse("head-tail").unwrap(), Keep::HeadTail);
        assert_eq!(Keep::parse("center").unwrap(), Keep::Middle);
        assert!(Keep::parse("sideways").is_err());
    }
}
