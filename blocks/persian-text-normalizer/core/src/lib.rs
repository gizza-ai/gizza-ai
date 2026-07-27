//! persian-text-normalizer core — normalize Persian/Farsi text in one pass.
//! Pure compute, no wafer/wasm-bindgen deps; shared by the chat skill block and
//! the web page.
//!
//! The pipeline runs in this fixed order (each step is independently toggleable):
//!   1. **characters** — fold Arabic letters to their Persian equivalents
//!      (ك→ک, ي→ی, ى→ی, ة→ه).
//!   2. **remove_diacritics** — strip Arabic harakat (tashkeel) and the kashida
//!      (tatweel ـ).
//!   3. **digits** — convert digits to Persian (۰۱۲۳), English (0123), or keep.
//!   4. **persian_punctuation** — convert Latin punctuation to Persian
//!      (`,`→`،`, `;`→`؛`, `?`→`؟`), skipping digit-grouping commas.
//!   5. **half_space** — fix half-spaces (ZWNJ / نیم‌فاصله): tidy spacing around
//!      existing ZWNJ and attach common affixes (می/نمی, ها/تر/ترین …) with a ZWNJ.
//!   6. **punctuation_spacing** — no space before `. ، ؛ ؟ ! :`, one space after
//!      (decimal/time/thousands separators are left intact).
//!   7. **whitespace** — collapse repeated spaces/tabs, normalize line endings, trim.

/// Zero-width non-joiner — the Persian "half-space" (نیم‌فاصله).
const ZWNJ: char = '\u{200C}';
/// Kashida / tatweel — a decorative letter-stretching character with no meaning.
const TATWEEL: char = '\u{0640}';

/// Verb prefixes that attach to the following word with a half-space.
const PREFIXES: &[&str] = &["می", "نمی"];
/// Plural / comparative suffixes that attach to the preceding word with a half-space.
const SUFFIXES: &[&str] = &["ها", "های", "هایی", "تر", "تری", "ترین"];

/// What to do with digit characters.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Digits {
    /// Convert ASCII (0-9) and Arabic-Indic (٠-٩) digits to Persian (۰-۹).
    Persian,
    /// Convert Persian (۰-۹) and Arabic-Indic (٠-٩) digits to ASCII (0-9).
    English,
    /// Leave digit characters untouched.
    Keep,
}

impl Digits {
    /// Parse a digit mode (case-insensitive; blank → `Persian`). Unknown → `Err`.
    pub fn parse(s: &str) -> Result<Digits, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "persian" | "farsi" | "fa" => Ok(Digits::Persian),
            "english" | "latin" | "ascii" | "en" => Ok(Digits::English),
            "keep" | "none" | "off" => Ok(Digits::Keep),
            other => Err(format!(
                "invalid digits {other:?}: expected one of persian, english, keep"
            )),
        }
    }
}

/// All normalization options. Build with [`Options::default`] then override.
#[derive(Clone, Copy)]
pub struct Options {
    pub characters: bool,
    pub digits: Digits,
    pub half_space: bool,
    pub punctuation_spacing: bool,
    pub persian_punctuation: bool,
    pub remove_diacritics: bool,
    pub whitespace: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            characters: true,
            digits: Digits::Persian,
            half_space: true,
            punctuation_spacing: true,
            persian_punctuation: false,
            remove_diacritics: false,
            whitespace: true,
        }
    }
}

/// Normalize `text` per `opts`. Never fails (option parsing is done upstream).
pub fn normalize(text: &str, opts: Options) -> String {
    let mut s = text.to_string();

    if opts.characters {
        s = fold_characters(&s);
    }
    if opts.remove_diacritics {
        s = remove_diacritics(&s);
    }
    s = map_digits(&s, opts.digits);
    if opts.persian_punctuation {
        s = to_persian_punctuation(&s);
    }
    if opts.half_space {
        s = fix_half_spaces(&s);
    }
    if opts.punctuation_spacing {
        s = fix_punctuation_spacing(&s);
    }
    if opts.whitespace {
        s = clean_whitespace(&s);
    }
    s
}

/// Fold the Arabic forms of shared letters to their Persian equivalents.
fn fold_characters(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\u{0643}' => '\u{06A9}', // ك Arabic kaf   → ک Persian keheh
            '\u{064A}' => '\u{06CC}', // ي Arabic yeh   → ی Persian yeh
            '\u{0649}' => '\u{06CC}', // ى alef maksura → ی Persian yeh
            '\u{0629}' => '\u{0647}', // ة teh marbuta  → ه heh
            other => other,
        })
        .collect()
}

/// Remove Arabic diacritics (harakat / tashkeel, U+064B–U+0652 and the
/// superscript alef U+0670) and the kashida (tatweel).
fn remove_diacritics(text: &str) -> String {
    text.chars()
        .filter(|&c| !matches!(c, '\u{064B}'..='\u{0652}' | '\u{0670}') && c != TATWEEL)
        .collect()
}

/// True for any digit character (ASCII, Persian, or Arabic-Indic).
fn is_digit_char(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '\u{06F0}'..='\u{06F9}' | '\u{0660}'..='\u{0669}')
}

/// Convert digits to the requested script.
fn map_digits(text: &str, target: Digits) -> String {
    if target == Digits::Keep {
        return text.to_string();
    }
    text.chars()
        .map(|c| {
            // Normalize any digit to its 0-9 value first.
            let value = match c {
                '0'..='9' => Some(c as u32 - '0' as u32),
                '\u{06F0}'..='\u{06F9}' => Some(c as u32 - 0x06F0),
                '\u{0660}'..='\u{0669}' => Some(c as u32 - 0x0660),
                _ => None,
            };
            match (value, target) {
                (Some(v), Digits::Persian) => char::from_u32(0x06F0 + v).unwrap_or(c),
                (Some(v), Digits::English) => char::from_u32('0' as u32 + v).unwrap_or(c),
                _ => c,
            }
        })
        .collect()
}

/// Convert Latin punctuation to its Persian equivalent. A comma between two
/// digits (thousands separator) is left as an ASCII comma.
fn to_persian_punctuation(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        let mapped = match c {
            ',' => {
                let between_digits = i > 0
                    && chars[i - 1].is_ascii_digit()
                    && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit());
                if between_digits {
                    ','
                } else {
                    '،'
                }
            }
            ';' => '؛',
            '?' => '؟',
            other => other,
        };
        out.push(mapped);
    }
    out
}

/// Tidy spacing around existing ZWNJ: drop spaces adjacent to a ZWNJ, collapse
/// consecutive ZWNJ, and drop a ZWNJ that would sit at a line boundary.
fn clean_zwnj(text: &str) -> String {
    let mut out: Vec<char> = Vec::with_capacity(text.len());
    for c in text.chars() {
        if c == ZWNJ {
            while matches!(out.last(), Some(' ') | Some('\t')) {
                out.pop();
            }
            // Skip a duplicate ZWNJ or one that would open a line.
            if out.last() == Some(&ZWNJ) || out.is_empty() || out.last() == Some(&'\n') {
                continue;
            }
            out.push(ZWNJ);
        } else if (c == ' ' || c == '\t') && out.last() == Some(&ZWNJ) {
            // Drop a space directly after a ZWNJ.
            continue;
        } else {
            out.push(c);
        }
    }
    // Drop a trailing ZWNJ (end of string or before a newline).
    let mut result: Vec<char> = Vec::with_capacity(out.len());
    for (i, &c) in out.iter().enumerate() {
        if c == ZWNJ && matches!(out.get(i + 1), None | Some('\n')) {
            continue;
        }
        result.push(c);
    }
    result.into_iter().collect()
}

/// A Persian/Arabic letter (used to gate affix attachment).
fn is_letter(c: char) -> bool {
    c.is_alphabetic() && !c.is_ascii()
}

/// Fix half-spaces: normalize existing ZWNJ spacing, then attach common verb
/// prefixes and plural/comparative suffixes to their host word with a ZWNJ.
/// Works line by line so newlines are preserved.
fn fix_half_spaces(text: &str) -> String {
    let cleaned = clean_zwnj(text);
    cleaned
        .split('\n')
        .map(join_affixes)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Join affixes within a single line (no newlines).
fn join_affixes(line: &str) -> String {
    let tokens: Vec<&str> = line.split(' ').filter(|t| !t.is_empty()).collect();
    if tokens.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(line.len());
    out.push_str(tokens[0]);
    for i in 1..tokens.len() {
        let prev = tokens[i - 1];
        let cur = tokens[i];
        let attach_prefix = PREFIXES.contains(&prev) && cur.starts_with(is_letter);
        let attach_suffix = SUFFIXES.contains(&cur) && prev.ends_with(is_letter);
        if attach_prefix || attach_suffix {
            out.push(ZWNJ);
        } else {
            out.push(' ');
        }
        out.push_str(cur);
    }
    out
}

/// Sentence punctuation that takes no space before and one space after.
const SPACING_PUNCT: &[char] = &['.', '،', '؛', '؟', '!', ':', ',', '?', ';'];

/// Enforce Persian punctuation spacing: strip spaces before the punctuation and
/// leave exactly one space after it. Digit separators (3.14, 10:30, 1,000) are
/// left untouched.
fn fix_punctuation_spacing(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if SPACING_PUNCT.contains(&c) {
            // Leave digit-grouping / decimal / time separators intact.
            let prev = out.last().copied();
            let next = chars.get(i + 1).copied();
            if matches!(c, '.' | ':' | ',')
                && prev.is_some_and(is_digit_char)
                && next.is_some_and(is_digit_char)
            {
                out.push(c);
                i += 1;
                continue;
            }
            while matches!(out.last(), Some(' ') | Some('\t')) {
                out.pop();
            }
            out.push(c);
            // Skip any run of spaces/tabs already following the punctuation.
            let mut j = i + 1;
            while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
                j += 1;
            }
            // Add a single space unless we hit end/newline, another punctuation,
            // a closing bracket, or a ZWNJ.
            if let Some(&nc) = chars.get(j) {
                let no_space = nc == '\n'
                    || nc == '\r'
                    || nc == ZWNJ
                    || SPACING_PUNCT.contains(&nc)
                    || matches!(nc, ')' | ']' | '}' | '»' | '،');
                if !no_space {
                    out.push(' ');
                }
            }
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out.into_iter().collect()
}

/// Collapse runs of spaces/tabs to one space, normalize line endings to `\n`,
/// trim each line, and trim the whole string. ZWNJ and newlines are preserved.
fn clean_whitespace(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(|line| {
            line.split(|c| c == ' ' || c == '\t')
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A neutral base: every pass off, digits kept — tests toggle one piece.
    fn opts() -> Options {
        Options {
            characters: false,
            digits: Digits::Keep,
            half_space: false,
            punctuation_spacing: false,
            persian_punctuation: false,
            remove_diacritics: false,
            whitespace: false,
        }
    }

    #[test]
    fn folds_arabic_characters_to_persian() {
        let o = Options {
            characters: true,
            ..opts()
        };
        // Arabic kaf + yeh → Persian keheh + yeh.
        assert_eq!(normalize("كتاب زيبا", o), "کتاب زیبا");
        // alef maksura and teh marbuta.
        assert_eq!(normalize("مصطفى مدينة", o), "مصطفی مدینه");
    }

    #[test]
    fn removes_diacritics_and_kashida() {
        let o = Options {
            remove_diacritics: true,
            ..opts()
        };
        // fatha + shadda stripped.
        assert_eq!(normalize("مُحَمَّد", o), "محمد");
        // kashida (tatweel) removed.
        assert_eq!(normalize("سلامـــت", o), "سلامت");
    }

    #[test]
    fn digit_conversion_all_directions() {
        let persian = Options {
            digits: Digits::Persian,
            ..opts()
        };
        // ASCII + Arabic-Indic → Persian.
        assert_eq!(normalize("123 ٤٥٦", persian), "۱۲۳ ۴۵۶");
        let english = Options {
            digits: Digits::English,
            ..opts()
        };
        assert_eq!(normalize("۱۲۳ ٤٥٦", english), "123 456");
        let keep = Options {
            digits: Digits::Keep,
            ..opts()
        };
        assert_eq!(normalize("۱۲۳ 456", keep), "۱۲۳ 456");
    }

    #[test]
    fn persian_punctuation_conversion() {
        let o = Options {
            persian_punctuation: true,
            ..opts()
        };
        assert_eq!(normalize("سلام, حالت خوبه?", o), "سلام، حالت خوبه؟");
        // Thousands separator stays ASCII.
        assert_eq!(normalize("1,000", o), "1,000");
    }

    #[test]
    fn half_space_attaches_prefix_and_suffix() {
        let o = Options {
            half_space: true,
            ..opts()
        };
        // Verb prefix می, plural suffix ها, comparative ترین.
        assert_eq!(normalize("می روم", o), "می\u{200C}روم");
        assert_eq!(normalize("کتاب ها", o), "کتاب\u{200C}ها");
        assert_eq!(normalize("بزرگ ترین", o), "بزرگ\u{200C}ترین");
    }

    #[test]
    fn half_space_cleans_existing_zwnj_spacing() {
        let o = Options {
            half_space: true,
            ..opts()
        };
        // A stray space around an existing ZWNJ is removed.
        assert_eq!(normalize("خانه \u{200C} ها", o), "خانه\u{200C}ها");
    }

    #[test]
    fn punctuation_spacing_fix() {
        let o = Options {
            punctuation_spacing: true,
            ..opts()
        };
        assert_eq!(normalize("سلام ،حالت خوبه ؟", o), "سلام، حالت خوبه؟");
        // Decimal and time separators are preserved.
        assert_eq!(normalize("عدد 3.14 در 10:30", o), "عدد 3.14 در 10:30");
    }

    #[test]
    fn whitespace_cleanup() {
        let o = Options {
            whitespace: true,
            ..opts()
        };
        assert_eq!(normalize("  سلام   دنیا  ", o), "سلام دنیا");
        // \r\n line endings normalize to \n; blank lines and per-line spacing tidy up.
        assert_eq!(normalize("خط اول\r\n\r\nخط  دوم", o), "خط اول\n\nخط دوم");
    }

    #[test]
    fn default_pipeline_end_to_end() {
        // Defaults: characters + persian digits + half-space + punct spacing + whitespace.
        let got = normalize("كتاب123 می خواهم ,خوب", Options::default());
        assert_eq!(got, "کتاب۱۲۳ می\u{200C}خواهم, خوب");
    }

    #[test]
    fn empty_input_is_ok() {
        assert_eq!(normalize("", Options::default()), "");
    }

    #[test]
    fn parsers_accept_aliases_and_reject_unknown() {
        assert!(Digits::parse("Persian").is_ok());
        assert!(Digits::parse("").is_ok());
        assert!(Digits::parse("english").is_ok());
        assert!(Digits::parse("keep").is_ok());
        assert!(Digits::parse("roman").is_err());
    }
}
