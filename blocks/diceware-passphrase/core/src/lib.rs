//! diceware-passphrase core — generate memorable diceware passphrases from the
//! EFF wordlists, shared by the chat skill block, the CLI, and the web page.
//!
//! Wordlists embedded verbatim from the Electronic Frontier Foundation:
//! - EFF long list  (`eff_large_wordlist.txt`, 7,776 words, 5 dice per word)
//! - EFF short list (`eff_short_wordlist_1.txt`, 1,296 words, 4 dice per word)
//! (c) Electronic Frontier Foundation, CC-BY 3.0 — https://www.eff.org/dice
//!
//! Randomness via `getrandom` (WASI `random_get` under wafer; the page's
//! wasm32-unknown-unknown build uses getrandom's `js` backend). Uniform word
//! indices via rejection sampling (no modulo bias). Passing your own physical
//! dice rolls (`rolls`) makes the lookup fully deterministic.

use std::sync::OnceLock;

const EFF_LARGE_RAW: &str = include_str!("../data/eff_large_wordlist.txt");
const EFF_SHORT_RAW: &str = include_str!("../data/eff_short_wordlist_1.txt");

/// Symbols used for `separator = "random-symbol"` and `add_symbol` (12 → ~3.6 bits each).
const SYMBOLS: &[u8] = b"!@#$%^&*-+=?";

const MAX_WORDS: usize = 20;
const MIN_WORDS: usize = 2;
const MAX_COUNT: usize = 20;
/// Offline attack rate used for the crack-time estimate (fast GPU rig).
const OFFLINE_GUESSES_PER_SEC: f64 = 1e10;

fn parse_list(raw: &'static str, expected: usize) -> Vec<&'static str> {
    let words: Vec<&'static str> = raw
        .lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .collect();
    assert_eq!(words.len(), expected, "embedded EFF wordlist is corrupt");
    words
}

fn large_words() -> &'static [&'static str] {
    static L: OnceLock<Vec<&'static str>> = OnceLock::new();
    L.get_or_init(|| parse_list(EFF_LARGE_RAW, 7776))
}

fn short_words() -> &'static [&'static str] {
    static S: OnceLock<Vec<&'static str>> = OnceLock::new();
    S.get_or_init(|| parse_list(EFF_SHORT_RAW, 1296))
}

/// Uniform random index in `0..n` via rejection sampling (no modulo bias).
fn rand_index(n: usize) -> Result<usize, String> {
    if n == 0 {
        return Err("empty selection set".into());
    }
    let nn = n as u64;
    let zone = (u64::from(u32::MAX) + 1) / nn * nn; // largest multiple of n <= 2^32
    loop {
        let mut b = [0u8; 4];
        getrandom::getrandom(&mut b).map_err(|e| format!("RNG error: {e}"))?;
        let v = u64::from(u32::from_le_bytes(b));
        if v < zone {
            return Ok((v % nn) as usize);
        }
    }
}

/// A wordlist choice with its dice geometry.
struct List {
    words: &'static [&'static str],
    dice: usize,
    display: &'static str,
    size_display: &'static str,
}

fn list_for(wordlist: &str) -> Result<List, String> {
    match wordlist {
        "" | "eff-long" => Ok(List {
            words: large_words(),
            dice: 5,
            display: "EFF long list",
            size_display: "7,776",
        }),
        "eff-short" => Ok(List {
            words: short_words(),
            dice: 4,
            display: "EFF short list",
            size_display: "1,296",
        }),
        other => Err(format!("wordlist {other:?} not supported (eff-long|eff-short)")),
    }
}

fn separator_for(separator: &str) -> Result<Option<&'static str>, String> {
    match separator {
        "" | "hyphen" => Ok(Some("-")),
        "space" => Ok(Some(" ")),
        "underscore" => Ok(Some("_")),
        "dot" => Ok(Some(".")),
        "none" => Ok(Some("")),
        "random-symbol" => Ok(None), // one random symbol per gap
        other => Err(format!(
            "separator {other:?} not supported (hyphen|space|underscore|dot|none|random-symbol)"
        )),
    }
}

/// Dice-roll digits (most-significant first) → 0-based wordlist index.
fn roll_to_index(digits: &[u8]) -> usize {
    digits.iter().fold(0usize, |acc, d| acc * 6 + (*d - b'1') as usize)
}

/// 0-based wordlist index → dice-roll digit string (e.g. 0 → "11111").
fn index_to_roll(mut idx: usize, dice: usize) -> String {
    let mut out = vec![b'1'; dice];
    for slot in out.iter_mut().rev() {
        *slot = b'1' + (idx % 6) as u8;
        idx /= 6;
    }
    String::from_utf8(out).expect("roll digits are ASCII")
}

/// Parse user-supplied physical dice rolls into word indices.
fn parse_rolls(rolls: &str, list: &List) -> Result<Vec<usize>, String> {
    let mut digits: Vec<u8> = Vec::new();
    for c in rolls.chars() {
        if c.is_whitespace() || c == ',' || c == '-' {
            continue;
        }
        match c {
            '1'..='6' => digits.push(c as u8),
            '0' | '7' | '8' | '9' => {
                return Err("dice rolls use digits 1-6 only (a six-sided die has no 0, 7, 8 or 9)".into())
            }
            other => return Err(format!("invalid character {other:?} in rolls (expected digits 1-6)")),
        }
    }
    if digits.is_empty() {
        return Err("rolls is empty — enter digits 1-6, e.g. '62315 14534'".into());
    }
    if digits.len() % list.dice != 0 {
        return Err(format!(
            "the {} needs {} dice per word — got {} digits, which is not a multiple of {}",
            list.display,
            list.dice,
            digits.len(),
            list.dice
        ));
    }
    let n = digits.len() / list.dice;
    if n > MAX_WORDS {
        return Err(format!("rolls encode {n} words — the maximum is {MAX_WORDS}"));
    }
    Ok(digits.chunks(list.dice).map(roll_to_index).collect())
}

fn capitalize_word(w: &str) -> String {
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn fmt_qty(v: f64) -> String {
    if v < 10.0 {
        let s = format!("{v:.1}");
        s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
    } else {
        format!("{v:.0}")
    }
}

/// Humanize a duration in seconds ("about 350 thousand years", "under a second").
fn humanize_secs(s: f64) -> String {
    if s < 1.0 {
        return "under a second".into();
    }
    const YEAR: f64 = 31_557_600.0; // 365.25 days
    let (v, unit) = if s < 60.0 {
        (s, "second")
    } else if s < 3600.0 {
        (s / 60.0, "minute")
    } else if s < 86_400.0 {
        (s / 3600.0, "hour")
    } else if s < YEAR {
        (s / 86_400.0, "day")
    } else {
        let y = s / YEAR;
        if y < 1e3 {
            (y, "year")
        } else if y < 1e6 {
            (y / 1e3, "thousand years")
        } else if y < 1e9 {
            (y / 1e6, "million years")
        } else if y < 1e12 {
            (y / 1e9, "billion years")
        } else if y < 1e15 {
            (y / 1e12, "trillion years")
        } else {
            return "over a quadrillion years".into();
        }
    };
    let q = fmt_qty(v);
    if unit.ends_with('s') || q == "1" {
        format!("about {q} {unit}")
    } else {
        format!("about {q} {unit}s")
    }
}

fn strength_label(bits: f64) -> &'static str {
    if bits < 45.0 {
        "weak"
    } else if bits < 70.0 {
        "fair"
    } else if bits < 100.0 {
        "strong"
    } else {
        "very strong"
    }
}

/// Generation options. `Default` matches the descriptor defaults.
pub struct Options {
    pub words: usize,
    pub wordlist: String,
    pub separator: String,
    pub capitalize: bool,
    pub add_number: bool,
    pub add_symbol: bool,
    pub count: usize,
    pub show_rolls: bool,
    pub rolls: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            words: 6,
            wordlist: "eff-long".into(),
            separator: "hyphen".into(),
            capitalize: false,
            add_number: false,
            add_symbol: false,
            count: 1,
            show_rolls: false,
            rolls: String::new(),
        }
    }
}

/// Structured generation result (chat block serializes it; page/CLI use `format_text`).
#[derive(Debug)]
pub struct Output {
    pub passphrases: Vec<String>,
    /// Per-passphrase raw dictionary words (uncapitalized, no extras).
    pub words: Vec<Vec<String>>,
    /// Per-passphrase dice rolls, one roll string per word (always populated).
    pub rolls: Vec<Vec<String>>,
    pub bits: f64,
    pub strength: &'static str,
    pub crack_time: String,
    pub detail: String,
}

pub fn generate(o: &Options) -> Result<Output, String> {
    let list = list_for(&o.wordlist)?;
    let sep = separator_for(&o.separator)?;
    if o.count < 1 || o.count > MAX_COUNT {
        return Err(format!("count must be between 1 and {MAX_COUNT}"));
    }
    let given_rolls = !o.rolls.trim().is_empty();
    if given_rolls && o.count != 1 {
        return Err("count must be 1 when rolls are provided (one set of rolls = one passphrase)".into());
    }
    let fixed_indices = if given_rolls {
        Some(parse_rolls(&o.rolls, &list)?)
    } else {
        if o.words < MIN_WORDS || o.words > MAX_WORDS {
            return Err(format!("words must be between {MIN_WORDS} and {MAX_WORDS}"));
        }
        None
    };
    let n_words = fixed_indices.as_ref().map_or(o.words, Vec::len);

    let mut passphrases = Vec::with_capacity(o.count);
    let mut all_words = Vec::with_capacity(o.count);
    let mut all_rolls = Vec::with_capacity(o.count);
    for _ in 0..o.count {
        let indices: Vec<usize> = match &fixed_indices {
            Some(v) => v.clone(),
            None => (0..n_words)
                .map(|_| rand_index(list.words.len()))
                .collect::<Result<_, _>>()?,
        };
        let mut phrase = String::new();
        for (i, &idx) in indices.iter().enumerate() {
            if i > 0 {
                match sep {
                    Some(s) => phrase.push_str(s),
                    None => phrase.push(SYMBOLS[rand_index(SYMBOLS.len())?] as char),
                }
            }
            let w = list.words[idx];
            if o.capitalize {
                phrase.push_str(&capitalize_word(w));
            } else {
                phrase.push_str(w);
            }
        }
        if o.add_number {
            phrase.push(char::from_digit(rand_index(10)? as u32, 10).expect("digit"));
        }
        if o.add_symbol {
            phrase.push(SYMBOLS[rand_index(SYMBOLS.len())?] as char);
        }
        all_words.push(indices.iter().map(|&i| list.words[i].to_string()).collect::<Vec<_>>());
        all_rolls.push(indices.iter().map(|&i| index_to_roll(i, list.dice)).collect());
        passphrases.push(phrase);
    }

    let bits_per_word = (list.words.len() as f64).log2();
    let mut bits = n_words as f64 * bits_per_word;
    let mut extras = String::new();
    if sep.is_none() && n_words > 1 {
        bits += (n_words - 1) as f64 * (SYMBOLS.len() as f64).log2();
        extras.push_str(" + random-symbol separators (+3.6 bits each)");
    }
    if o.add_number {
        bits += 10f64.log2();
        extras.push_str(" + trailing digit (+3.3 bits)");
    }
    if o.add_symbol {
        bits += (SYMBOLS.len() as f64).log2();
        extras.push_str(" + trailing symbol (+3.6 bits)");
    }
    let bits = (bits * 10.0).round() / 10.0;
    let avg_secs = (bits - 1.0).exp2() / OFFLINE_GUESSES_PER_SEC;
    let detail = format!(
        "{n_words} words from the {} ({} words, {bits_per_word:.1} bits/word){extras}",
        list.display, list.size_display
    );

    Ok(Output {
        passphrases,
        words: all_words,
        rolls: all_rolls,
        bits,
        strength: strength_label(bits),
        crack_time: humanize_secs(avg_secs),
        detail,
    })
}

/// Plain-text rendering shared by the web page and the CLI-facing docs.
pub fn format_text(out: &Output, show_rolls: bool) -> String {
    let mut s = out.passphrases.join("\n");
    s.push_str(&format!(
        "\n\nEntropy: {:.1} bits — strength: {}\nRecipe: {}\nCrack time: {} (offline, 10 billion guesses/sec)",
        out.bits, out.strength, out.detail, out.crack_time
    ));
    if show_rolls {
        s.push_str("\n\nDice rolls:");
        for (pi, rolls) in out.rolls.iter().enumerate() {
            if pi > 0 {
                s.push('\n');
            }
            for (roll, word) in rolls.iter().zip(&out.words[pi]) {
                s.push_str(&format!("\n{roll}  {word}"));
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_rolls_map_to_eff_long_words() {
        let o = Options {
            rolls: "11111 11112 11113 11114 11115 11116".into(),
            ..Options::default()
        };
        let out = generate(&o).unwrap();
        assert_eq!(
            out.passphrases,
            vec!["abacus-abdomen-abdominal-abide-abiding-ability".to_string()]
        );
        assert_eq!(out.bits, 77.5);
        assert_eq!(out.strength, "strong");
        assert_eq!(out.rolls[0][0], "11111");
        assert_eq!(out.rolls[0][5], "11116");
    }

    #[test]
    fn deterministic_rolls_eff_short() {
        let o = Options {
            wordlist: "eff-short".into(),
            rolls: "1111 6666".into(),
            ..Options::default()
        };
        let out = generate(&o).unwrap();
        assert_eq!(out.passphrases, vec!["acid-zoom".to_string()]);
        assert_eq!(out.bits, 20.7);
        assert_eq!(out.strength, "weak");
    }

    #[test]
    fn last_roll_is_zoom_on_both_lists() {
        let long = generate(&Options { rolls: "66666".repeat(2), ..Options::default() }).unwrap();
        assert_eq!(long.passphrases, vec!["zoom-zoom".to_string()]);
        let short = generate(&Options {
            wordlist: "eff-short".into(),
            rolls: "66666666".into(),
            ..Options::default()
        })
        .unwrap();
        assert_eq!(short.passphrases, vec!["zoom-zoom".to_string()]);
    }

    #[test]
    fn random_generation_uses_list_words_and_separator() {
        let o = Options { words: 5, separator: "dot".into(), ..Options::default() };
        let out = generate(&o).unwrap();
        let p = &out.passphrases[0];
        let parts: Vec<&str> = p.split('.').collect();
        assert_eq!(parts.len(), 5);
        for w in &parts {
            assert!(large_words().contains(w), "{w} not in EFF long list");
        }
        // two runs differ (5 words = 64.6 bits; collision chance negligible)
        let out2 = generate(&o).unwrap();
        assert_ne!(out.passphrases, out2.passphrases);
    }

    #[test]
    fn capitalize_and_none_separator() {
        let o = Options {
            rolls: "1111 6666".into(),
            wordlist: "eff-short".into(),
            separator: "none".into(),
            capitalize: true,
            ..Options::default()
        };
        let out = generate(&o).unwrap();
        assert_eq!(out.passphrases, vec!["AcidZoom".to_string()]);
    }

    #[test]
    fn extras_add_entropy_and_characters() {
        let o = Options {
            rolls: "11111 11112".into(),
            add_number: true,
            add_symbol: true,
            ..Options::default()
        };
        let out = generate(&o).unwrap();
        let p = &out.passphrases[0];
        let chars: Vec<char> = p.chars().collect();
        let sym = chars[chars.len() - 1];
        let digit = chars[chars.len() - 2];
        assert!(SYMBOLS.contains(&(sym as u8)), "expected trailing symbol, got {sym:?}");
        assert!(digit.is_ascii_digit(), "expected trailing digit, got {digit:?}");
        // 2×12.92 + 3.32 + 3.58 = 32.8
        assert_eq!(out.bits, 32.8);
        assert!(out.detail.contains("trailing digit"), "{}", out.detail);
        assert!(out.detail.contains("trailing symbol"), "{}", out.detail);
    }

    #[test]
    fn random_symbol_separator_counts_gaps() {
        let o = Options {
            rolls: "11111 11112 11113".into(),
            separator: "random-symbol".into(),
            ..Options::default()
        };
        let out = generate(&o).unwrap();
        // 3×12.92 + 2×3.58 = 45.9
        assert_eq!(out.bits, 45.9);
        let p = &out.passphrases[0];
        assert!(p.starts_with("abacus") && p.ends_with("abdominal"), "{p}");
        assert_eq!(
            p.matches(|c: char| c.is_ascii() && SYMBOLS.contains(&(c as u8))).count(),
            2,
            "{p}"
        );
    }

    #[test]
    fn count_produces_distinct_lines() {
        let o = Options { count: 3, ..Options::default() };
        let out = generate(&o).unwrap();
        assert_eq!(out.passphrases.len(), 3);
        assert_ne!(out.passphrases[0], out.passphrases[1]);
        let text = format_text(&out, false);
        assert_eq!(text.lines().take_while(|l| !l.is_empty()).count(), 3);
    }

    #[test]
    fn format_text_exact_with_rolls() {
        let o = Options { rolls: "11111 11112".into(), ..Options::default() };
        let out = generate(&o).unwrap();
        let text = format_text(&out, true);
        assert_eq!(
            text,
            "abacus-abdomen\n\nEntropy: 25.8 bits — strength: weak\nRecipe: 2 words from the EFF long list (7,776 words, 12.9 bits/word)\nCrack time: under a second (offline, 10 billion guesses/sec)\n\nDice rolls:\n11111  abacus\n11112  abdomen"
        );
    }

    #[test]
    fn error_cases() {
        // invalid dice digit
        assert!(generate(&Options { rolls: "11117".into(), ..Options::default() })
            .unwrap_err()
            .contains("digits 1-6"));
        // wrong group size for the list
        assert!(generate(&Options { rolls: "111".into(), ..Options::default() })
            .unwrap_err()
            .contains("multiple of 5"));
        // words out of range
        assert!(generate(&Options { words: 1, ..Options::default() })
            .unwrap_err()
            .contains("between 2 and 20"));
        assert!(generate(&Options { words: 21, ..Options::default() })
            .unwrap_err()
            .contains("between 2 and 20"));
        // count out of range / count with rolls
        assert!(generate(&Options { count: 0, ..Options::default() }).unwrap_err().contains("count"));
        assert!(generate(&Options { count: 21, ..Options::default() }).unwrap_err().contains("count"));
        assert!(generate(&Options { count: 2, rolls: "11111".into(), ..Options::default() })
            .unwrap_err()
            .contains("count must be 1"));
        // unknown enum values
        assert!(generate(&Options { wordlist: "bip39".into(), ..Options::default() }).is_err());
        assert!(generate(&Options { separator: "comma".into(), ..Options::default() }).is_err());
    }

    #[test]
    fn humanize_buckets() {
        assert_eq!(humanize_secs(0.5), "under a second");
        assert_eq!(humanize_secs(90.0), "about 1.5 minutes");
        assert_eq!(humanize_secs(3.2e13), "about 1 million years");
        assert_eq!(humanize_secs(1e40), "over a quadrillion years");
    }

    #[test]
    fn six_words_crack_time_is_thousands_of_years() {
        let out = generate(&Options::default()).unwrap();
        assert_eq!(out.bits, 77.5);
        assert!(out.crack_time.contains("thousand years"), "{}", out.crack_time);
    }
}
