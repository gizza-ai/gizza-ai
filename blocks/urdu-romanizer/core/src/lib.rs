//! urdu-romanizer core — transliterate Urdu (Arabic-script) text into Roman
//! (Latin) Urdu. Pure compute, no wafer/wasm-bindgen deps; shared by the chat
//! skill block and the web page.
//!
//! The transliteration is **deterministic and letter-level**. Urdu script does
//! not write short vowels, so a purely mechanical converter cannot recover them:
//! this module handles that in three complementary ways, in priority order:
//!
//!   1. **Common-word list** — the highest-frequency Urdu words are mapped to
//!      their conventional Roman spellings (`ہے` → `hai`, `پاکستان` →
//!      `pakistan`). Informal scheme only, toggled by [`Options::common_words`].
//!   2. **Diacritics in the input** — zabar/zer/pesh (َ ِ ُ), tanwin, shadda,
//!      sukun and the dagger alef are honoured exactly when present.
//!   3. **Short-vowel policy** — [`ShortVowels`] decides what happens between two
//!      consonants that carry no vowel mark: insert a default `a`, leave the
//!      cluster bare, or drop vowel marks entirely.
//!
//! Three output schemes are supported: plain-ASCII informal Roman Urdu (the
//! default, what people actually type online), ALA-LC, and ISO 15919.

/// Zero-width non-joiner — used inside Urdu words, carries no sound.
const ZWNJ: char = '\u{200C}';
/// Zero-width joiner — likewise soundless.
const ZWJ: char = '\u{200D}';
/// Kashida / tatweel — decorative letter stretching, no sound.
const TATWEEL: char = '\u{0640}';

const FATHA: char = '\u{064E}'; // zabar   → a
const KASRA: char = '\u{0650}'; // zer     → i
const DAMMA: char = '\u{064F}'; // pesh    → u
const FATHATAN: char = '\u{064B}'; // tanwin fath → an
const KASRATAN: char = '\u{064D}'; // tanwin kasr → in
const DAMMATAN: char = '\u{064C}'; // tanwin damm → un
const SHADDA: char = '\u{0651}'; // tashdid → double the previous consonant
const SUKUN: char = '\u{0652}'; // jazm    → explicitly no vowel
const DAGGER_ALEF: char = '\u{0670}'; // khari zabar → long a

/// Output romanization scheme.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scheme {
    /// Plain-ASCII Roman Urdu as typed online: 26 letters, no diacritics, lossy.
    Informal,
    /// ALA-LC style, keeping the Arabic-letter distinctions with diacritics.
    AlaLc,
    /// ISO 15919 style, keeping the Arabic-letter distinctions with diacritics.
    Iso15919,
}

impl Scheme {
    /// Parse a scheme name (case-insensitive; blank → `Informal`).
    pub fn parse(s: &str) -> Result<Scheme, String> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "" | "informal" | "ascii" | "plain" => Ok(Scheme::Informal),
            "ala-lc" | "alalc" | "ala" | "loc" => Ok(Scheme::AlaLc),
            "iso15919" | "iso-15919" | "iso" => Ok(Scheme::Iso15919),
            other => Err(format!(
                "invalid scheme {other:?}: expected one of informal, ala-lc, iso15919"
            )),
        }
    }

    fn index(self) -> usize {
        match self {
            Scheme::Informal => 0,
            Scheme::AlaLc => 1,
            Scheme::Iso15919 => 2,
        }
    }
}

/// What to do about the short vowels Urdu script does not write.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShortVowels {
    /// Honour any diacritics present, and insert a default `a` between two
    /// consecutive unvowelled consonants so the output stays pronounceable.
    InsertA,
    /// Honour diacritics only; consonant clusters are left bare (`کتاب` → `ktab`).
    MarksOnly,
    /// Ignore diacritics entirely and never insert a vowel.
    Omit,
}

impl ShortVowels {
    /// Parse a short-vowel policy (case-insensitive; blank → `InsertA`).
    pub fn parse(s: &str) -> Result<ShortVowels, String> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "" | "insert-a" | "inserta" | "insert" => Ok(ShortVowels::InsertA),
            "marks-only" | "marks" => Ok(ShortVowels::MarksOnly),
            "omit" | "none" | "off" => Ok(ShortVowels::Omit),
            other => Err(format!(
                "invalid short_vowels {other:?}: expected one of insert-a, marks-only, omit"
            )),
        }
    }
}

/// Digit handling.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Digits {
    /// Convert Urdu (۰-۹) and Arabic-Indic (٠-٩) digits to ASCII 0-9.
    Latin,
    /// Leave digit characters exactly as they are.
    Keep,
}

impl Digits {
    /// Parse a digit mode (case-insensitive; blank → `Latin`).
    pub fn parse(s: &str) -> Result<Digits, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "latin" | "ascii" | "english" => Ok(Digits::Latin),
            "keep" | "urdu" | "none" => Ok(Digits::Keep),
            other => Err(format!(
                "invalid digits {other:?}: expected one of latin, keep"
            )),
        }
    }
}

/// Punctuation handling.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Punctuation {
    /// Convert Urdu punctuation to its Latin equivalent (۔ → ., ؟ → ?, …).
    Latin,
    /// Leave punctuation characters exactly as they are.
    Keep,
}

impl Punctuation {
    /// Parse a punctuation mode (case-insensitive; blank → `Latin`).
    pub fn parse(s: &str) -> Result<Punctuation, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "latin" | "ascii" | "english" => Ok(Punctuation::Latin),
            "keep" | "urdu" | "none" => Ok(Punctuation::Keep),
            other => Err(format!(
                "invalid punctuation {other:?}: expected one of latin, keep"
            )),
        }
    }
}

/// How the Roman output is capitalized.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Capitalization {
    /// Leave everything lower-case.
    None,
    /// Capitalize the first letter of each sentence.
    Sentence,
    /// Capitalize the first letter of every word.
    Title,
}

impl Capitalization {
    /// Parse a capitalization mode (case-insensitive; blank → `Sentence`).
    pub fn parse(s: &str) -> Result<Capitalization, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "sentence" => Ok(Capitalization::Sentence),
            "none" | "lower" | "lowercase" => Ok(Capitalization::None),
            "title" | "word" => Ok(Capitalization::Title),
            other => Err(format!(
                "invalid capitalization {other:?}: expected one of none, sentence, title"
            )),
        }
    }
}

/// Every romanization option. Build with [`Options::default`] then override.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub scheme: Scheme,
    pub short_vowels: ShortVowels,
    pub common_words: bool,
    pub digits: Digits,
    pub punctuation: Punctuation,
    pub capitalization: Capitalization,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            scheme: Scheme::Informal,
            short_vowels: ShortVowels::InsertA,
            common_words: true,
            digits: Digits::Latin,
            punctuation: Punctuation::Latin,
            capitalization: Capitalization::Sentence,
        }
    }
}

/// Consonant table: `(letter, [informal, ala-lc, iso15919])`.
///
/// `ھ` (do-chashmi he) is the aspiration marker and simply appends `h`, which is
/// why `چ` + `ھ` yields `chh` with no special case.
const CONSONANTS: &[(char, [&str; 3])] = &[
    ('\u{0628}', ["b", "b", "b"]),    // ب
    ('\u{067E}', ["p", "p", "p"]),    // پ
    ('\u{062A}', ["t", "t", "t"]),    // ت
    ('\u{0679}', ["t", "ṭ", "ṭ"]),    // ٹ
    ('\u{062B}', ["s", "s̄", "s̱"]),    // ث
    ('\u{062C}', ["j", "j", "j"]),    // ج
    ('\u{0686}', ["ch", "ch", "c"]),  // چ
    ('\u{062D}', ["h", "ḥ", "ḥ"]),    // ح
    ('\u{062E}', ["kh", "k͟h", "ḵẖ"]), // خ
    ('\u{062F}', ["d", "d", "d"]),    // د
    ('\u{0688}', ["d", "ḍ", "ḍ"]),    // ڈ
    ('\u{0630}', ["z", "ẕ", "ẕ"]),    // ذ
    ('\u{0631}', ["r", "r", "r"]),    // ر
    ('\u{0691}', ["r", "ṛ", "ṛ"]),    // ڑ
    ('\u{0632}', ["z", "z", "z"]),    // ز
    ('\u{0698}', ["zh", "zh", "ž"]),  // ژ
    ('\u{0633}', ["s", "s", "s"]),    // س
    ('\u{0634}', ["sh", "sh", "ś"]),  // ش
    ('\u{0635}', ["s", "ṣ", "ṣ"]),    // ص
    ('\u{0636}', ["z", "ẓ", "ż"]),    // ض
    ('\u{0637}', ["t", "t̤", "ṭ"]),    // ط
    ('\u{0638}', ["z", "ẓ̤", "ẓ"]),    // ظ
    ('\u{063A}', ["gh", "g͟h", "ġ"]),  // غ
    ('\u{0641}', ["f", "f", "f"]),    // ف
    ('\u{0642}', ["q", "q", "q"]),    // ق
    ('\u{06A9}', ["k", "k", "k"]),    // ک
    ('\u{0643}', ["k", "k", "k"]),    // ك (Arabic kaf)
    ('\u{06AF}', ["g", "g", "g"]),    // گ
    ('\u{0644}', ["l", "l", "l"]),    // ل
    ('\u{0645}', ["m", "m", "m"]),    // م
    ('\u{0646}', ["n", "n", "n"]),    // ن
    ('\u{06BA}', ["n", "ṉ", "ṁ"]),    // ں (noon ghunna)
    ('\u{06C1}', ["h", "h", "h"]),    // ہ (gol he)
    ('\u{0647}', ["h", "h", "h"]),    // ه (Arabic he)
    ('\u{0629}', ["h", "h", "h"]),    // ة (teh marbuta)
    ('\u{06BE}', ["h", "h", "h"]),    // ھ (do-chashmi he — aspiration)
];

/// Common Urdu words → their conventional informal Roman spelling. Applied only
/// in [`Scheme::Informal`] when [`Options::common_words`] is on; a whole-word
/// match wins over the letter-by-letter path because these are exactly the words
/// whose unwritten short vowels a mechanical converter gets wrong.
const COMMON_WORDS: &[(&str, &str)] = &[
    ("ہے", "hai"),
    ("ہیں", "hain"),
    ("ہو", "ho"),
    ("ہوں", "hoon"),
    ("تھا", "tha"),
    ("تھی", "thi"),
    ("تھے", "the"),
    ("کا", "ka"),
    ("کے", "ke"),
    ("کی", "ki"),
    ("کو", "ko"),
    ("سے", "se"),
    ("پر", "par"),
    ("اور", "aur"),
    ("نہیں", "nahin"),
    ("کیا", "kya"),
    ("یہ", "yeh"),
    ("وہ", "woh"),
    ("اس", "is"),
    ("ان", "in"),
    ("ہم", "hum"),
    ("تم", "tum"),
    ("آپ", "aap"),
    ("کیوں", "kyun"),
    ("کہاں", "kahan"),
    ("کب", "kab"),
    ("کون", "kaun"),
    ("اچھا", "acha"),
    ("شکریہ", "shukriya"),
    ("پاکستان", "pakistan"),
    ("اردو", "urdu"),
    ("سلام", "salaam"),
    ("السلام", "assalam"),
    ("علیکم", "alaikum"),
    ("خدا", "khuda"),
    ("حافظ", "hafiz"),
    ("دن", "din"),
    ("رات", "raat"),
    ("پانی", "pani"),
    ("کتاب", "kitab"),
    ("بہت", "bohat"),
    ("گیا", "gaya"),
    ("کر", "kar"),
    ("کرنا", "karna"),
    ("کرتا", "karta"),
    ("کرتے", "karte"),
    ("کرتی", "karti"),
    ("ایک", "aik"),
    ("دو", "do"),
    ("تین", "teen"),
    ("نام", "naam"),
    ("لیے", "liye"),
    ("ساتھ", "sath"),
    ("بھی", "bhi"),
    ("کہ", "keh"),
    ("جو", "jo"),
    ("میرا", "mera"),
    ("تیرا", "tera"),
    ("ہمارا", "hamara"),
    ("آج", "aaj"),
    ("کل", "kal"),
    ("دنیا", "dunya"),
    ("زندگی", "zindagi"),
    ("محبت", "mohabbat"),
    ("دوست", "dost"),
    ("گھر", "ghar"),
    ("بات", "baat"),
    ("لوگ", "log"),
    ("وقت", "waqt"),
    ("سب", "sab"),
    ("کچھ", "kuch"),
    ("ٹھیک", "theek"),
    ("ہاں", "haan"),
    ("جی", "ji"),
];

/// Urdu punctuation → Latin equivalent.
const PUNCTUATION: &[(char, &str)] = &[
    ('\u{06D4}', "."),  // ۔ full stop
    ('\u{060C}', ","),  // ، comma
    ('\u{061B}', ";"),  // ؛ semicolon
    ('\u{061F}', "?"),  // ؟ question mark
    ('\u{066A}', "%"),  // ٪ percent
    ('\u{066B}', "."),  // ٫ decimal separator
    ('\u{066C}', ","),  // ٬ thousands separator
    ('\u{066D}', "*"),  // ٭ five-pointed star
    ('\u{00AB}', "\""), // « quote
    ('\u{00BB}', "\""), // » quote
];

/// True for the letters, marks and joiners that make up an Urdu *word*.
fn is_urdu_word_char(c: char) -> bool {
    matches!(c,
        '\u{0620}'..='\u{063F}'
        | '\u{0641}'..='\u{064A}'
        | '\u{064B}'..='\u{0652}'
        | '\u{0670}'
        | '\u{0671}'..='\u{06BF}'
        | '\u{06C0}'..='\u{06D3}'
        | '\u{06D5}'..='\u{06ED}'
        | '\u{06FA}'..='\u{06FF}'
        | ZWNJ
        | ZWJ
    ) && !is_urdu_digit(c)
}

fn is_urdu_digit(c: char) -> bool {
    ('\u{0660}'..='\u{0669}').contains(&c) || ('\u{06F0}'..='\u{06F9}').contains(&c)
}

fn latin_digit(c: char) -> Option<char> {
    if ('\u{0660}'..='\u{0669}').contains(&c) {
        char::from_u32('0' as u32 + (c as u32 - 0x0660))
    } else if ('\u{06F0}'..='\u{06F9}').contains(&c) {
        char::from_u32('0' as u32 + (c as u32 - 0x06F0))
    } else {
        None
    }
}

fn consonant(c: char, scheme: Scheme) -> Option<&'static str> {
    CONSONANTS
        .iter()
        .find(|(letter, _)| *letter == c)
        .map(|(_, forms)| forms[scheme.index()])
}

fn is_harakat(c: char) -> bool {
    matches!(
        c,
        FATHA | KASRA | DAMMA | FATHATAN | KASRATAN | DAMMATAN | SHADDA | SUKUN | DAGGER_ALEF
    )
}

/// Romanize `text`. Returns `Err` when `text` is empty or whitespace-only.
pub fn romanize(text: &str, opts: Options) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("expected Urdu text, got an empty string".to_string());
    }

    let mut out = String::new();
    let mut word = String::new();
    for c in text.chars() {
        if is_urdu_word_char(c) {
            word.push(c);
            continue;
        }
        if !word.is_empty() {
            out.push_str(&romanize_word(&word, opts));
            word.clear();
        }
        out.push_str(&passthrough(c, opts));
    }
    if !word.is_empty() {
        out.push_str(&romanize_word(&word, opts));
    }

    Ok(capitalize(&out, opts.capitalization))
}

/// Stringly entry point used by the shared CLI/chat/page wrappers.
pub fn run(
    input: &str,
    scheme: &str,
    short_vowels: &str,
    common_words: bool,
    digits: &str,
    punctuation: &str,
    capitalization: &str,
) -> Result<String, String> {
    romanize(
        input,
        Options {
            scheme: Scheme::parse(scheme)?,
            short_vowels: ShortVowels::parse(short_vowels)?,
            common_words,
            digits: Digits::parse(digits)?,
            punctuation: Punctuation::parse(punctuation)?,
            capitalization: Capitalization::parse(capitalization)?,
        },
    )
}

/// Non-Urdu-letter characters: digits and punctuation are mapped per the
/// options, everything else (Latin text, spaces, newlines, emoji) is preserved.
fn passthrough(c: char, opts: Options) -> String {
    if opts.digits == Digits::Latin {
        if let Some(d) = latin_digit(c) {
            return d.to_string();
        }
    }
    if opts.punctuation == Punctuation::Latin {
        if let Some((_, latin)) = PUNCTUATION.iter().find(|(u, _)| *u == c) {
            return (*latin).to_string();
        }
    }
    c.to_string()
}

/// Strip the soundless joiners/kashida so a word can be looked up verbatim.
fn strip_joiners(word: &str) -> String {
    word.chars()
        .filter(|c| *c != ZWNJ && *c != ZWJ && *c != TATWEEL)
        .collect()
}

fn romanize_word(word: &str, opts: Options) -> String {
    let bare = strip_joiners(word);

    if opts.scheme == Scheme::Informal && opts.common_words {
        if let Some((_, roman)) = COMMON_WORDS.iter().find(|(urdu, _)| *urdu == bare) {
            return (*roman).to_string();
        }
    }

    let chars: Vec<char> = bare.chars().collect();
    let s = opts.scheme;
    let i_scheme = s.index();
    let mut out = String::new();
    // The consonant just emitted with no vowel after it yet — the trigger for
    // short-vowel insertion.
    let mut pending: Option<&'static str> = None;
    // Where the most recent consonant form starts in `out`, so a shadda can
    // duplicate it in place. Unicode canonical order puts a vowel mark BEFORE
    // the shadda (ccc 30 < 33), so "double whatever was last emitted" would
    // double the vowel instead — insert at the recorded offset instead.
    let mut last_consonant: Option<(usize, String)> = None;
    // The most recent emitted unit, used to decide whether و/ی act as consonants.
    let mut last_was_vowel = false;

    for (i, &c) in chars.iter().enumerate() {
        let at_start = out.is_empty();
        let at_end = i + 1 == chars.len();
        let next_is_harakat = chars.get(i + 1).is_some_and(|n| is_harakat(*n));

        // --- diacritics -------------------------------------------------
        if is_harakat(c) {
            if opts.short_vowels == ShortVowels::Omit {
                if c == SHADDA {
                    if let Some((pos, form)) = last_consonant.take() {
                        out.insert_str(pos, &form);
                    }
                }
                continue;
            }
            match c {
                SHADDA => {
                    if let Some((pos, form)) = last_consonant.take() {
                        out.insert_str(pos, &form);
                    }
                }
                SUKUN => {
                    pending = None;
                }
                FATHA => {
                    out.push('a');
                    pending = None;
                    last_was_vowel = true;
                }
                KASRA => {
                    // A word-final kasra is the izafat linker (صدرِ مملکت).
                    if at_end {
                        out.push_str(if s == Scheme::Informal { "-e" } else { "-i" });
                    } else {
                        out.push('i');
                    }
                    pending = None;
                    last_was_vowel = true;
                }
                DAMMA => {
                    out.push('u');
                    pending = None;
                    last_was_vowel = true;
                }
                FATHATAN => {
                    out.push_str("an");
                    pending = None;
                    last_was_vowel = true;
                }
                KASRATAN => {
                    out.push_str("in");
                    pending = None;
                    last_was_vowel = true;
                }
                DAMMATAN => {
                    out.push_str("un");
                    pending = None;
                    last_was_vowel = true;
                }
                DAGGER_ALEF => {
                    out.push_str(if s == Scheme::Informal { "a" } else { "ā" });
                    pending = None;
                    last_was_vowel = true;
                }
                _ => {}
            }
            continue;
        }

        // --- vowel carriers and semi-vowels -----------------------------
        match c {
            // آ / أ / إ / ٱ — alef with madda or hamza
            '\u{0622}' => {
                out.push_str(if s == Scheme::Informal { "aa" } else { "ā" });
                pending = None;
                last_was_vowel = true;
                continue;
            }
            '\u{0627}' | '\u{0623}' | '\u{0625}' | '\u{0671}' => {
                // Word-initial alef is only a carrier: if the vowel is spelled
                // out by a following harakat or long vowel, emit nothing.
                if at_start {
                    let next_is_long = matches!(
                        chars.get(i + 1),
                        Some('\u{0648}') | Some('\u{06CC}') | Some('\u{064A}') | Some('\u{06D2}')
                    );
                    if !next_is_harakat && !next_is_long {
                        out.push('a');
                        last_was_vowel = true;
                    }
                } else {
                    out.push_str(if s == Scheme::Informal { "a" } else { "ā" });
                    last_was_vowel = true;
                }
                pending = None;
                continue;
            }
            // و — consonant at word start or after a vowel, else a long vowel.
            '\u{0648}' | '\u{0624}' => {
                let as_consonant = at_start || last_was_vowel;
                if as_consonant && !at_end {
                    let v = if s == Scheme::Informal { "w" } else { "v" };
                    if opts.short_vowels == ShortVowels::InsertA && pending.is_some() {
                        out.push('a');
                    }
                    last_consonant = Some((out.len(), v.to_string()));
                    out.push_str(v);
                    pending = Some(v);
                    last_was_vowel = false;
                } else {
                    out.push_str(if s == Scheme::Informal { "o" } else { "ū" });
                    pending = None;
                    last_was_vowel = true;
                }
                continue;
            }
            // ی / ي / ئ — consonant at word start or after a vowel, else long i.
            '\u{06CC}' | '\u{064A}' | '\u{0626}' => {
                let as_consonant = at_start || last_was_vowel;
                if as_consonant && !at_end {
                    if opts.short_vowels == ShortVowels::InsertA && pending.is_some() {
                        out.push('a');
                    }
                    last_consonant = Some((out.len(), "y".to_string()));
                    out.push('y');
                    pending = Some("y");
                    last_was_vowel = false;
                } else {
                    out.push_str(if s == Scheme::Informal { "i" } else { "ī" });
                    pending = None;
                    last_was_vowel = true;
                }
                continue;
            }
            // ے / ۓ — bari ye, always the vowel e.
            '\u{06D2}' | '\u{06D3}' => {
                out.push_str(if s == Scheme::Iso15919 { "ē" } else { "e" });
                pending = None;
                last_was_vowel = true;
                continue;
            }
            // ۂ / ۀ — he with hamza: he plus the izafat linker.
            '\u{06C2}' | '\u{06C0}' => {
                if opts.short_vowels == ShortVowels::InsertA && pending.is_some() {
                    out.push('a');
                }
                out.push('h');
                out.push_str(if s == Scheme::Informal { "-e" } else { "-i" });
                pending = None;
                last_was_vowel = true;
                continue;
            }
            // ع — ain. Informal folds it into a vowel; the scholarly schemes
            // keep it as its own sign.
            '\u{0639}' => {
                out.push_str(match s {
                    Scheme::Informal => "a",
                    Scheme::AlaLc => "ʻ",
                    Scheme::Iso15919 => "ʿ",
                });
                pending = None;
                last_was_vowel = s == Scheme::Informal;
                continue;
            }
            // ء — standalone hamza. Dropped in ASCII output.
            '\u{0621}' => {
                if s != Scheme::Informal {
                    out.push('ʼ');
                }
                continue;
            }
            _ => {}
        }

        // --- consonants -------------------------------------------------
        if let Some(form) = consonant(c, s) {
            // Aspiration (ھ) attaches to the preceding consonant, so it must not
            // trigger a short-vowel insertion between the two.
            let is_aspiration = c == '\u{06BE}';
            if opts.short_vowels == ShortVowels::InsertA && pending.is_some() && !is_aspiration {
                out.push('a');
            }
            last_consonant = Some((out.len(), form.to_string()));
            out.push_str(form);
            pending = if is_aspiration { pending } else { Some(form) };
            last_was_vowel = false;
            continue;
        }

        // Anything else inside the word run (rare marks): keep it verbatim.
        let _ = i_scheme;
        out.push(c);
    }

    out
}

/// Apply the capitalization policy to already-romanized text.
fn capitalize(text: &str, mode: Capitalization) -> String {
    match mode {
        Capitalization::None => text.to_string(),
        Capitalization::Sentence => {
            let mut out = String::with_capacity(text.len());
            let mut start_of_sentence = true;
            for c in text.chars() {
                if start_of_sentence && c.is_alphabetic() {
                    out.extend(c.to_uppercase());
                    start_of_sentence = false;
                } else {
                    out.push(c);
                    if matches!(c, '.' | '!' | '?' | '\n') {
                        start_of_sentence = true;
                    }
                }
            }
            out
        }
        Capitalization::Title => {
            let mut out = String::with_capacity(text.len());
            let mut start_of_word = true;
            for c in text.chars() {
                if start_of_word && c.is_alphabetic() {
                    out.extend(c.to_uppercase());
                    start_of_word = false;
                } else {
                    out.push(c);
                    if c.is_whitespace() {
                        start_of_word = true;
                    }
                }
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn informal(text: &str) -> String {
        romanize(text, Options::default()).unwrap()
    }

    #[test]
    fn romanizes_a_common_sentence_with_defaults() {
        // Every word here is in the common-word list, so the output is the
        // conventional Roman Urdu spelling, sentence-cased, with ۔ → '.'.
        assert_eq!(informal("یہ کتاب اچھی ہے۔"), "Yeh kitab achhi hai.");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = romanize("   \n ", Options::default()).unwrap_err();
        assert!(err.contains("empty"), "unexpected error: {err}");
    }

    #[test]
    fn invalid_scheme_is_an_error() {
        let err = Scheme::parse("wade-giles").unwrap_err();
        assert!(err.contains("expected one of informal"), "got: {err}");
    }

    #[test]
    fn common_words_can_be_disabled() {
        let opts = Options {
            common_words: false,
            ..Options::default()
        };
        // Without the word list, پاکستان falls back to letter-by-letter with the
        // default `a` inserted between unvowelled consonants.
        assert_eq!(romanize("پاکستان", opts).unwrap(), "Pakasatan");
        assert_eq!(informal("پاکستان"), "Pakistan");
    }

    #[test]
    fn diacritics_are_honoured() {
        let opts = Options {
            common_words: false,
            capitalization: Capitalization::None,
            ..Options::default()
        };
        // مُحَمَّد — pesh, zabar, shadda, zabar.
        assert_eq!(romanize("مُحَمَّد", opts).unwrap(), "muhammad");
    }

    #[test]
    fn short_vowel_policies_differ() {
        let base = Options {
            common_words: false,
            capitalization: Capitalization::None,
            ..Options::default()
        };
        let marks = Options {
            short_vowels: ShortVowels::MarksOnly,
            ..base
        };
        let omit = Options {
            short_vowels: ShortVowels::Omit,
            ..base
        };
        assert_eq!(romanize("کتاب", base).unwrap(), "katab");
        assert_eq!(romanize("کتاب", marks).unwrap(), "ktab");
        assert_eq!(romanize("مُحَمَّد", omit).unwrap(), "mhmmd");
    }

    #[test]
    fn scholarly_schemes_keep_letter_distinctions() {
        let opts = |scheme| Options {
            scheme,
            common_words: false,
            capitalization: Capitalization::None,
            ..Options::default()
        };
        // ط ٹ ت all collapse to "t" informally but stay distinct otherwise.
        assert_eq!(romanize("طٹت", opts(Scheme::Informal)).unwrap(), "tatat");
        assert_eq!(romanize("طٹت", opts(Scheme::AlaLc)).unwrap(), "t̤aṭat");
        assert_eq!(romanize("طٹت", opts(Scheme::Iso15919)).unwrap(), "ṭaṭat");
    }

    #[test]
    fn aspiration_does_not_split_the_consonant() {
        let opts = Options {
            common_words: false,
            capitalization: Capitalization::None,
            ..Options::default()
        };
        // بھ is one aspirated consonant: "bh", never "bah".
        assert_eq!(romanize("بھائی", opts).unwrap(), "bhayi");
    }

    #[test]
    fn digits_and_punctuation_convert_by_default() {
        assert_eq!(informal("۱۲۳ ہے، جی۔"), "123 Hai, ji.");
        let keep = Options {
            digits: Digits::Keep,
            punctuation: Punctuation::Keep,
            ..Options::default()
        };
        assert_eq!(romanize("۱۲۳ ہے", keep).unwrap(), "۱۲۳ Hai");
    }

    #[test]
    fn izafat_renders_as_a_linker() {
        let opts = Options {
            common_words: false,
            capitalization: Capitalization::None,
            ..Options::default()
        };
        // A word-final zer is the izafat construction, not a plain short i.
        assert_eq!(romanize("صدرِ", opts).unwrap(), "sadar-e");
    }

    #[test]
    fn capitalization_modes() {
        let opts = |capitalization| Options {
            capitalization,
            ..Options::default()
        };
        assert_eq!(
            romanize("جی ہاں۔ شکریہ۔", opts(Capitalization::None)).unwrap(),
            "ji haan. shukriya."
        );
        assert_eq!(
            romanize("جی ہاں۔ شکریہ۔", opts(Capitalization::Sentence)).unwrap(),
            "Ji haan. Shukriya."
        );
        assert_eq!(
            romanize("جی ہاں۔ شکریہ۔", opts(Capitalization::Title)).unwrap(),
            "Ji Haan. Shukriya."
        );
    }

    #[test]
    fn latin_text_and_line_breaks_pass_through() {
        let opts = Options {
            capitalization: Capitalization::None,
            ..Options::default()
        };
        assert_eq!(romanize("HTML ہے\nجی", opts).unwrap(), "HTML hai\nji");
    }

    #[test]
    fn parsers_accept_aliases_and_blanks() {
        assert_eq!(Scheme::parse("").unwrap(), Scheme::Informal);
        assert_eq!(Scheme::parse("ALA_LC").unwrap(), Scheme::AlaLc);
        assert_eq!(ShortVowels::parse("marks").unwrap(), ShortVowels::MarksOnly);
        assert_eq!(Digits::parse("keep").unwrap(), Digits::Keep);
        assert_eq!(Punctuation::parse("").unwrap(), Punctuation::Latin);
        assert_eq!(Capitalization::parse("").unwrap(), Capitalization::Sentence);
        assert!(Digits::parse("roman").is_err());
        assert!(Punctuation::parse("fancy").is_err());
        assert!(Capitalization::parse("shout").is_err());
        assert!(ShortVowels::parse("guess").is_err());
    }
}
