//! gizza-ai/censor-text core — pure compute, shared by the chat skill block and
//! the web page. No deps. Redacts a supplied list of words (or a built-in common
//! list) in `text` by masking each match with a mask character. Case-insensitive;
//! whole-word matching by default.

/// A small built-in list used when the caller supplies no words. Kept modest;
/// the real power is the caller-supplied `words` list.
const DEFAULT_LIST: &[&str] = &[
    "damn", "hell", "crap", "shit", "fuck", "bitch", "ass", "bastard", "dick", "piss",
];

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Censor `text`. `words_csv` is a comma-separated list of words to redact (empty
/// → the built-in list). `mask` supplies the masking character (its first char;
/// default `*`). When `whole_word`, only whole-word matches are masked.
pub fn censor(text: &str, words_csv: &str, mask: &str, whole_word: bool) -> Result<String, String> {
    if text.is_empty() {
        return Err("input text is empty".into());
    }
    let mask_char = mask.chars().next().unwrap_or('*');
    let owned: Vec<String> = words_csv
        .split(',')
        .map(|w| w.trim().to_ascii_lowercase())
        .filter(|w| !w.is_empty())
        .collect();
    let targets: Vec<&str> = if owned.is_empty() {
        DEFAULT_LIST.to_vec()
    } else {
        owned.iter().map(|s| s.as_str()).collect()
    };

    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = chars.iter().map(|c| c.to_ascii_lowercase()).collect();
    let mut masked = vec![false; chars.len()];

    for target in &targets {
        let t: Vec<char> = target.chars().collect();
        if t.is_empty() {
            continue;
        }
        let n = t.len();
        if n > chars.len() {
            continue;
        }
        let mut i = 0;
        while i + n <= lower.len() {
            if lower[i..i + n] == t[..] {
                let before_ok = i == 0 || !is_word_char(chars[i - 1]);
                let after_ok = i + n == chars.len() || !is_word_char(chars[i + n]);
                if !whole_word || (before_ok && after_ok) {
                    for j in i..i + n {
                        masked[j] = true;
                    }
                    i += n;
                    continue;
                }
            }
            i += 1;
        }
    }

    let out: String = chars
        .iter()
        .enumerate()
        .map(|(idx, &c)| if masked[idx] { mask_char } else { c })
        .collect();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_supplied_words_case_insensitive() {
        assert_eq!(censor("The Quick brown QUICK fox", "quick", "*", true).unwrap(), "The ***** brown ***** fox");
    }

    #[test]
    fn whole_word_does_not_mask_substrings() {
        // "ass" should not mask inside "class" when whole_word=true
        assert_eq!(censor("a class assignment", "ass", "*", true).unwrap(), "a class assignment");
    }

    #[test]
    fn substring_mode_masks_inside_words() {
        let out = censor("classy", "ass", "*", false).unwrap();
        assert_eq!(out, "cl***y");
    }

    #[test]
    fn default_list_used_when_empty() {
        let out = censor("oh damn that", "", "*", true).unwrap();
        assert_eq!(out, "oh **** that");
    }

    #[test]
    fn custom_mask_char() {
        assert_eq!(censor("bad word", "bad", "#", true).unwrap(), "### word");
    }

    #[test]
    fn multiple_words() {
        assert_eq!(censor("foo and bar", "foo,bar", "*", true).unwrap(), "*** and ***");
    }

    #[test]
    fn empty_text_errors() {
        assert!(censor("", "x", "*", true).is_err());
    }

    #[test]
    fn unicode_preserved_for_unmasked() {
        let out = censor("café damn", "damn", "*", true).unwrap();
        assert_eq!(out, "café ****");
    }
}
