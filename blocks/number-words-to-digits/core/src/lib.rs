//! number-words-to-digits core — pure compute, shared by the chat skill block and the web page.
//! Parses spelled-out English numbers ("two hundred forty-three", "one point five million",
//! "minus one and a half", "three lakh") into numeric digits, either in place inside prose or as
//! standalone values. No wafer/wasm-bindgen deps, no network, no floating point: values are held
//! as exact 128-bit scaled decimals.

/// Hard cap on the input length, in characters.
pub const MAX_INPUT_CHARS: usize = 200_000;

/// Maximum number of digits a `digit_sequences` run may produce.
const MAX_SEQUENCE_DIGITS: usize = 38;

fn too_large() -> String {
    "number is too large to represent exactly (the limit is about 1.7e38)".to_string()
}

/// What the tool does with the numbers it finds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Rewrite the input text, replacing every spelled-out number with digits.
    Replace,
    /// Treat each non-empty line as one number phrase and return only the values.
    Value,
    /// Return every number found, one per line, dropping the surrounding text.
    Extract,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "replace" | "text" | "in-text" => Ok(Mode::Replace),
            "value" | "values" | "strict" => Ok(Mode::Value),
            "extract" | "list" => Ok(Mode::Extract),
            other => Err(format!(
                "unknown mode `{other}` — expected replace, value, or extract"
            )),
        }
    }
}

/// Which reading of `billion`, `trillion`, … applies.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScaleSystem {
    /// Short scale: billion = 10^9, trillion = 10^12 (US/modern UK).
    Short,
    /// Long scale: billion = 10^12, trillion = 10^18 (continental European).
    Long,
}

impl ScaleSystem {
    pub fn parse(s: &str) -> Result<ScaleSystem, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "short" | "us" => Ok(ScaleSystem::Short),
            "long" | "european" => Ok(ScaleSystem::Long),
            other => Err(format!(
                "unknown scale `{other}` — expected short or long"
            )),
        }
    }
}

/// How ordinal words (`first`, `twenty-first`, `hundredth`) are handled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OrdinalMode {
    /// `twenty-first` becomes `21`.
    Cardinal,
    /// `twenty-first` becomes `21st`.
    Suffix,
    /// Ordinal words are left alone and never end a number phrase.
    Ignore,
}

impl OrdinalMode {
    pub fn parse(s: &str) -> Result<OrdinalMode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "cardinal" | "number" => Ok(OrdinalMode::Cardinal),
            "suffix" | "ordinal" => Ok(OrdinalMode::Suffix),
            "ignore" | "skip" | "keep" => Ok(OrdinalMode::Ignore),
            other => Err(format!(
                "unknown ordinals `{other}` — expected cardinal, suffix, or ignore"
            )),
        }
    }
}

/// Thousands separator used when rendering the digits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Separator {
    None,
    Comma,
    Space,
    Underscore,
}

impl Separator {
    pub fn parse(s: &str) -> Result<Separator, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" | "plain" => Ok(Separator::None),
            "comma" | "," => Ok(Separator::Comma),
            "space" => Ok(Separator::Space),
            "underscore" | "_" => Ok(Separator::Underscore),
            other => Err(format!(
                "unknown separator `{other}` — expected none, comma, space, or underscore"
            )),
        }
    }

    fn ch(self) -> Option<char> {
        match self {
            Separator::None => None,
            Separator::Comma => Some(','),
            Separator::Space => Some(' '),
            Separator::Underscore => Some('_'),
        }
    }
}

/// Parsed run options.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub mode: Mode,
    pub separator: Separator,
    pub scale: ScaleSystem,
    pub ordinals: OrdinalMode,
    pub fractions: bool,
    pub digit_sequences: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: Mode::Replace,
            separator: Separator::None,
            scale: ScaleSystem::Short,
            ordinals: OrdinalMode::Cardinal,
            fractions: true,
            digit_sequences: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Exact scaled-decimal arithmetic (value = d / 10^e, d >= 0)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Dec {
    d: i128,
    e: u32,
}

fn pow10(k: u32) -> Result<i128, String> {
    let mut acc: i128 = 1;
    for _ in 0..k {
        acc = acc.checked_mul(10).ok_or_else(too_large)?;
    }
    Ok(acc)
}

impl Dec {
    fn zero() -> Dec {
        Dec { d: 0, e: 0 }
    }

    fn int(v: i128) -> Dec {
        Dec { d: v, e: 0 }
    }

    fn is_zero(self) -> bool {
        self.d == 0
    }

    /// Multiply by 10^k.
    fn scale_up(self, k: u32) -> Result<Dec, String> {
        if self.e >= k {
            Ok(Dec {
                d: self.d,
                e: self.e - k,
            })
        } else {
            Ok(Dec {
                d: self
                    .d
                    .checked_mul(pow10(k - self.e)?)
                    .ok_or_else(too_large)?,
                e: 0,
            })
        }
    }

    fn align(self, o: Dec) -> Result<(i128, i128, u32), String> {
        let e = self.e.max(o.e);
        let a = self
            .d
            .checked_mul(pow10(e - self.e)?)
            .ok_or_else(too_large)?;
        let b = o.d.checked_mul(pow10(e - o.e)?).ok_or_else(too_large)?;
        Ok((a, b, e))
    }

    fn add(self, o: Dec) -> Result<Dec, String> {
        let (a, b, e) = self.align(o)?;
        Ok(Dec {
            d: a.checked_add(b).ok_or_else(too_large)?,
            e,
        })
    }

    fn sub(self, o: Dec) -> Result<Dec, String> {
        let (a, b, e) = self.align(o)?;
        Ok(Dec {
            d: a.checked_sub(b).ok_or_else(too_large)?,
            e,
        })
    }

    fn mul_int(self, m: i128) -> Result<Dec, String> {
        Ok(Dec {
            d: self.d.checked_mul(m).ok_or_else(too_large)?,
            e: self.e,
        })
    }

    fn normalized(self) -> Dec {
        let mut d = self.d;
        let mut e = self.e;
        while e > 0 && d % 10 == 0 {
            d /= 10;
            e -= 1;
        }
        Dec { d, e }
    }

    fn as_int(self) -> Option<i128> {
        let n = self.normalized();
        if n.e == 0 {
            Some(n.d)
        } else {
            None
        }
    }

    fn render(self, neg: bool, sep: Separator) -> Result<String, String> {
        let n = self.normalized();
        let p = pow10(n.e)?;
        let int_part = n.d / p;
        let frac_part = n.d % p;
        let mut out = group(&int_part.to_string(), sep);
        if n.e > 0 {
            out.push('.');
            out.push_str(&format!("{:0width$}", frac_part, width = n.e as usize));
        }
        if neg && n.d != 0 {
            out.insert(0, '-');
        }
        Ok(out)
    }
}

fn group(digits: &str, sep: Separator) -> String {
    let ch = match sep.ch() {
        Some(c) => c,
        None => return digits.to_string(),
    };
    let bytes: Vec<char> = digits.chars().collect();
    let mut out = String::with_capacity(bytes.len() + bytes.len() / 3);
    for (idx, c) in bytes.iter().enumerate() {
        if idx > 0 && (bytes.len() - idx) % 3 == 0 {
            out.push(ch);
        }
        out.push(*c);
    }
    out
}

fn ordinal_suffix(n: i128) -> &'static str {
    let n = n.abs();
    match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    }
}

// ---------------------------------------------------------------------------
// Lexicon
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum W {
    /// A single word worth 0-99 (`seven`, `nineteen`, `forty`).
    Num(u8),
    /// `hundred` — multiplies the group by 100.
    Hundred,
    /// A scale word, as a power of ten (`thousand` = 3, `million` = 6, `crore` = 7).
    Scale(u32),
    /// `point` / `decimal` / `dot`.
    Point,
    /// An exact fraction: (digits, exponent), so `half` is (5, 1).
    Frac(i128, u32),
    /// `and`, used as a connector inside a number phrase.
    And,
    /// `a` / `an`, as in "a hundred" or "and a half".
    Article,
    /// `minus` / `negative`.
    Minus,
}

fn small_word(w: &str) -> Option<u8> {
    Some(match w {
        "zero" => 0,
        "one" => 1,
        "two" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        "ten" => 10,
        "eleven" => 11,
        "twelve" => 12,
        "thirteen" => 13,
        "fourteen" => 14,
        "fifteen" => 15,
        "sixteen" => 16,
        "seventeen" => 17,
        "eighteen" => 18,
        "nineteen" => 19,
        "twenty" => 20,
        "thirty" => 30,
        "forty" | "fourty" => 40,
        "fifty" => 50,
        "sixty" => 60,
        "seventy" => 70,
        "eighty" => 80,
        "ninety" => 90,
        _ => return None,
    })
}

fn scale_word(w: &str, scale: ScaleSystem) -> Option<u32> {
    // Words whose exponent never depends on the short/long convention.
    let fixed = match w {
        "thousand" => Some(3),
        "lakh" | "lakhs" | "lac" => Some(5),
        "million" => Some(6),
        "crore" | "crores" => Some(7),
        "milliard" => Some(9),
        "billiard" => Some(15),
        "trilliard" => Some(21),
        "quadrilliard" => Some(27),
        _ => None,
    };
    if fixed.is_some() {
        return fixed;
    }
    Some(match (w, scale) {
        ("billion", ScaleSystem::Short) => 9,
        ("billion", ScaleSystem::Long) => 12,
        ("trillion", ScaleSystem::Short) => 12,
        ("trillion", ScaleSystem::Long) => 18,
        ("quadrillion", ScaleSystem::Short) => 15,
        ("quadrillion", ScaleSystem::Long) => 24,
        ("quintillion", ScaleSystem::Short) => 18,
        ("quintillion", ScaleSystem::Long) => 30,
        ("sextillion", ScaleSystem::Short) => 21,
        ("sextillion", ScaleSystem::Long) => 36,
        ("septillion", ScaleSystem::Short) => 24,
        ("septillion", ScaleSystem::Long) => 42,
        ("octillion", ScaleSystem::Short) => 27,
        ("nonillion", ScaleSystem::Short) => 30,
        ("decillion", ScaleSystem::Short) => 33,
        _ => return None,
    })
}

fn classify(w: &str, o: &Options) -> Option<W> {
    if let Some(v) = small_word(w) {
        return Some(W::Num(v));
    }
    if w == "hundred" {
        return Some(W::Hundred);
    }
    if let Some(k) = scale_word(w, o.scale) {
        return Some(W::Scale(k));
    }
    if o.fractions {
        match w {
            "half" | "halves" => return Some(W::Frac(5, 1)),
            "quarter" | "quarters" => return Some(W::Frac(25, 2)),
            _ => {}
        }
    }
    match w {
        "point" | "decimal" | "dot" => Some(W::Point),
        "and" => Some(W::And),
        "a" | "an" => Some(W::Article),
        "minus" | "negative" => Some(W::Minus),
        _ => None,
    }
}

/// Ordinal words, mapped onto the cardinal token they stand for.
fn classify_ordinal(w: &str, o: &Options) -> Option<W> {
    let small = match w {
        "first" => 1,
        "second" => 2,
        "third" => 3,
        "fourth" => 4,
        "fifth" => 5,
        "sixth" => 6,
        "seventh" => 7,
        "eighth" => 8,
        "ninth" => 9,
        "tenth" => 10,
        "eleventh" => 11,
        "twelfth" => 12,
        "thirteenth" => 13,
        "fourteenth" => 14,
        "fifteenth" => 15,
        "sixteenth" => 16,
        "seventeenth" => 17,
        "eighteenth" => 18,
        "nineteenth" => 19,
        "twentieth" => 20,
        "thirtieth" => 30,
        "fortieth" => 40,
        "fiftieth" => 50,
        "sixtieth" => 60,
        "seventieth" => 70,
        "eightieth" => 80,
        "ninetieth" => 90,
        "hundredth" => return Some(W::Hundred),
        "thousandth" => return Some(W::Scale(3)),
        "millionth" => return Some(W::Scale(6)),
        "billionth" => {
            return Some(W::Scale(if o.scale == ScaleSystem::Short {
                9
            } else {
                12
            }))
        }
        "trillionth" => {
            return Some(W::Scale(if o.scale == ScaleSystem::Short {
                12
            } else {
                18
            }))
        }
        _ => return None,
    };
    Some(W::Num(small))
}

/// Single-digit words usable inside a spoken digit string ("nine one one").
fn digit_word(w: &str) -> Option<char> {
    Some(match w {
        "zero" | "oh" | "o" | "nought" | "naught" => '0',
        "one" => '1',
        "two" => '2',
        "three" => '3',
        "four" => '4',
        "five" => '5',
        "six" => '6',
        "seven" => '7',
        "eight" => '8',
        "nine" => '9',
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Tokenizing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Tok {
    lower: String,
    start: usize,
    end: usize,
}

fn tokenize(text: &str) -> Vec<Tok> {
    let mut toks = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in text.char_indices() {
        if c.is_alphabetic() {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            toks.push(Tok {
                lower: text[s..i].to_lowercase(),
                start: s,
                end: i,
            });
        }
    }
    if let Some(s) = start {
        toks.push(Tok {
            lower: text[s..].to_lowercase(),
            start: s,
            end: text.len(),
        });
    }
    toks
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Gap {
    /// Only spaces and hyphens — words are part of the same phrase.
    Plain,
    /// Spaces plus a comma — joins only after a scale word ("one million, two hundred").
    Comma,
    /// Anything else (newline, sentence punctuation, digits) — phrase ends here.
    Break,
}

fn gap_kind(text: &str, from: usize, to: usize) -> Gap {
    let mut saw_comma = false;
    for c in text[from..to].chars() {
        match c {
            ' ' | '\t' | '\u{00a0}' => {}
            '-' | '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' => {}
            ',' => saw_comma = true,
            _ => return Gap::Break,
        }
    }
    if saw_comma {
        Gap::Comma
    } else {
        Gap::Plain
    }
}

fn joined_plain(text: &str, toks: &[Tok], idx: usize) -> bool {
    idx > 0 && gap_kind(text, toks[idx - 1].end, toks[idx].start) == Gap::Plain
}

// ---------------------------------------------------------------------------
// Phrase parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Parsed {
    value: Dec,
    neg: bool,
    ordinal: bool,
    /// Token index just past the last token belonging to the phrase.
    end: usize,
}

impl Parsed {
    fn render(&self, o: &Options) -> Result<String, String> {
        let mut s = self.value.render(self.neg, o.separator)?;
        if self.ordinal && o.ordinals == OrdinalMode::Suffix {
            if let Some(n) = self.value.as_int() {
                s.push_str(ordinal_suffix(n));
            }
        }
        Ok(s)
    }
}

/// Is `and` here a number connector rather than an ordinary conjunction?
/// True after a hundred/scale word, or before a fraction tail ("and a half",
/// "and three quarters").
fn and_joins_fraction(text: &str, toks: &[Tok], mut k: usize, o: &Options) -> bool {
    if !o.fractions {
        return false;
    }
    if k >= toks.len() || !joined_plain(text, toks, k) {
        return false;
    }
    match classify(&toks[k].lower, o) {
        Some(W::Article) | Some(W::Num(_)) => k += 1,
        _ => return false,
    }
    if k >= toks.len() || !joined_plain(text, toks, k) {
        return false;
    }
    matches!(classify(&toks[k].lower, o), Some(W::Frac(_, _)))
}

fn parse_phrase(
    text: &str,
    toks: &[Tok],
    start: usize,
    allow_sign: bool,
    o: &Options,
) -> Result<Option<Parsed>, String> {
    if start >= toks.len() {
        return Ok(None);
    }
    let mut i = start;
    let mut neg = false;
    if allow_sign && matches!(classify(&toks[i].lower, o), Some(W::Minus)) {
        i += 1;
        if i >= toks.len() || !joined_plain(text, toks, i) {
            return Ok(None);
        }
        neg = true;
    }

    // Spoken digit strings ("nine one one" -> 911) short-circuit the grammar.
    if o.digit_sequences {
        let mut j = i;
        let mut digits = String::new();
        while j < toks.len() && (j == i || joined_plain(text, toks, j)) {
            match digit_word(&toks[j].lower) {
                Some(c) => {
                    digits.push(c);
                    j += 1;
                }
                None => break,
            }
        }
        if digits.len() >= 2 {
            if digits.len() > MAX_SEQUENCE_DIGITS {
                return Err(too_large());
            }
            let v: i128 = digits.parse().map_err(|_| too_large())?;
            return Ok(Some(Parsed {
                value: Dec::int(v),
                neg,
                ordinal: false,
                end: j,
            }));
        }
    }

    let mut total = Dec::zero();
    let mut cur = Dec::zero();
    let mut any = false;
    let mut commit_end = 0usize;
    let mut in_frac = false;
    let mut frac_pos: u32 = 0;
    let mut last_small: Option<i128> = None;
    // Sub-100 value already contributed to the current group, so that only the
    // combinations English allows ("twenty five") continue a phrase — "one two
    // three" must stay three separate numbers, not 6.
    let mut group_small: Option<u8> = None;
    let mut prev_article = false;
    let mut last_was_scale = false;
    let mut is_ordinal = false;

    let mut j = i;
    while j < toks.len() {
        if j > i {
            match gap_kind(text, toks[j - 1].end, toks[j].start) {
                Gap::Plain => {}
                Gap::Comma if last_was_scale => {}
                _ => break,
            }
        }
        let (w, from_ordinal) = match classify(&toks[j].lower, o) {
            Some(w) => (w, false),
            None if o.ordinals != OrdinalMode::Ignore => {
                match classify_ordinal(&toks[j].lower, o) {
                    Some(w) => (w, true),
                    None => break,
                }
            }
            None => break,
        };

        match w {
            W::Minus => break,
            W::Article => {
                if j + 1 >= toks.len() || !joined_plain(text, toks, j + 1) {
                    break;
                }
                match classify(&toks[j + 1].lower, o) {
                    Some(W::Hundred) | Some(W::Scale(_)) | Some(W::Frac(_, _)) => {}
                    _ => break,
                }
                prev_article = true;
                last_was_scale = false;
                j += 1;
                continue;
            }
            W::And => {
                if !(last_was_scale || and_joins_fraction(text, toks, j + 1, o)) {
                    break;
                }
                // "one and three quarters" — the numerator after `and` starts a
                // fresh group, so it must not be rejected as a second small word.
                group_small = None;
                j += 1;
                continue;
            }
            W::Point => {
                if in_frac || j + 1 >= toks.len() || !joined_plain(text, toks, j + 1) {
                    break;
                }
                if !matches!(classify(&toks[j + 1].lower, o), Some(W::Num(_))) {
                    break;
                }
                in_frac = true;
                frac_pos = 0;
                group_small = None;
                prev_article = false;
                last_was_scale = false;
                j += 1;
                continue;
            }
            W::Num(v) => {
                if in_frac {
                    let (digits, width, consumed) = if v >= 20 && v % 10 == 0 {
                        // "forty seven" after the point reads as .47, "forty" alone as .40
                        let unit = if j + 1 < toks.len() && joined_plain(text, toks, j + 1) {
                            match classify(&toks[j + 1].lower, o) {
                                Some(W::Num(u)) if (1..=9).contains(&u) => Some(u),
                                _ => None,
                            }
                        } else {
                            None
                        };
                        match unit {
                            Some(u) => ((v + u) as i128, 2u32, 2usize),
                            None => (v as i128, 2, 1),
                        }
                    } else if v >= 10 {
                        (v as i128, 2, 1)
                    } else {
                        (v as i128, 1, 1)
                    };
                    frac_pos += width;
                    cur = cur.add(Dec {
                        d: digits,
                        e: frac_pos,
                    })?;
                    any = true;
                    j += consumed;
                    commit_end = j;
                    last_small = None;
                    prev_article = false;
                    last_was_scale = false;
                    continue;
                }
                if let Some(prev) = group_small {
                    // Only "tens + unit" (forty-seven) continues the group.
                    if !(prev >= 20 && prev % 10 == 0 && (1..=9).contains(&v)) {
                        break;
                    }
                }
                cur = cur.add(Dec::int(v as i128))?;
                any = true;
                last_small = Some(v as i128);
                group_small = Some(group_small.unwrap_or(0) + v);
                prev_article = false;
                last_was_scale = false;
                j += 1;
                commit_end = j;
                if from_ordinal {
                    is_ordinal = true;
                    break;
                }
            }
            W::Hundred => {
                cur = if cur.is_zero() {
                    Dec::int(100)
                } else {
                    cur.scale_up(2)?
                };
                any = true;
                last_small = None;
                group_small = None;
                prev_article = false;
                last_was_scale = true;
                in_frac = false;
                j += 1;
                commit_end = j;
                if from_ordinal {
                    is_ordinal = true;
                    break;
                }
            }
            W::Scale(k) => {
                let base = if cur.is_zero() { Dec::int(1) } else { cur };
                total = total.add(base.scale_up(k)?)?;
                cur = Dec::zero();
                any = true;
                last_small = None;
                group_small = None;
                prev_article = false;
                last_was_scale = true;
                in_frac = false;
                j += 1;
                commit_end = j;
                if from_ordinal {
                    is_ordinal = true;
                    break;
                }
            }
            W::Frac(d, e) => {
                let f = Dec { d, e };
                cur = match last_small {
                    Some(c) if !prev_article => cur.sub(Dec::int(c))?.add(f.mul_int(c)?)?,
                    _ => cur.add(f)?,
                };
                any = true;
                last_small = None;
                group_small = None;
                prev_article = false;
                last_was_scale = false;
                in_frac = false;
                j += 1;
                commit_end = j;
            }
        }
    }

    if !any || commit_end == 0 {
        return Ok(None);
    }
    Ok(Some(Parsed {
        value: total.add(cur)?,
        neg,
        ordinal: is_ordinal,
        end: commit_end,
    }))
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Convert spelled-out numbers in `text` into digits.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    text: &str,
    mode: &str,
    separator: &str,
    scale: &str,
    ordinals: &str,
    fractions: bool,
    digit_sequences: bool,
) -> Result<String, String> {
    let o = Options {
        mode: Mode::parse(mode)?,
        separator: Separator::parse(separator)?,
        scale: ScaleSystem::parse(scale)?,
        ordinals: OrdinalMode::parse(ordinals)?,
        fractions,
        digit_sequences,
    };
    convert_with(text, &o)
}

/// Convert with already-parsed options.
pub fn convert_with(text: &str, o: &Options) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("input text is empty — paste a number written in words".to_string());
    }
    if text.chars().count() > MAX_INPUT_CHARS {
        return Err(format!(
            "input is too long ({} characters); the limit is {MAX_INPUT_CHARS}",
            text.chars().count()
        ));
    }
    match o.mode {
        Mode::Replace => replace_in_text(text, o),
        Mode::Extract => {
            let found = collect(text, o)?;
            if found.is_empty() {
                return Err(
                    "no spelled-out numbers found — check the words or try a different mode"
                        .to_string(),
                );
            }
            Ok(found.join("\n"))
        }
        Mode::Value => values_per_line(text, o),
    }
}

fn replace_in_text(text: &str, o: &Options) -> Result<String, String> {
    let toks = tokenize(text);
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    let mut i = 0usize;
    let mut prev_end = usize::MAX;
    while i < toks.len() {
        match parse_phrase(text, &toks, i, i != prev_end, o)? {
            Some(p) => {
                out.push_str(&text[cursor..toks[i].start]);
                out.push_str(&p.render(o)?);
                cursor = toks[p.end - 1].end;
                i = p.end;
                prev_end = p.end;
            }
            None => i += 1,
        }
    }
    out.push_str(&text[cursor..]);
    Ok(out)
}

fn collect(text: &str, o: &Options) -> Result<Vec<String>, String> {
    let toks = tokenize(text);
    let mut found = Vec::new();
    let mut i = 0usize;
    let mut prev_end = usize::MAX;
    while i < toks.len() {
        match parse_phrase(text, &toks, i, i != prev_end, o)? {
            Some(p) => {
                found.push(p.render(o)?);
                i = p.end;
                prev_end = p.end;
            }
            None => i += 1,
        }
    }
    Ok(found)
}

fn values_per_line(text: &str, o: &Options) -> Result<String, String> {
    let mut out: Vec<String> = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let n = idx + 1;
        if line.trim().is_empty() {
            continue;
        }
        let toks = tokenize(line);
        if toks.is_empty() {
            return Err(format!("line {n}: no number words found in {line:?}"));
        }
        let p = parse_phrase(line, &toks, 0, true, o)?.ok_or_else(|| {
            format!(
                "line {n}: {:?} does not start a number written in words",
                toks[0].lower
            )
        })?;
        if p.end < toks.len() {
            return Err(format!(
                "line {n}: {:?} is not part of a number phrase",
                toks[p.end].lower
            ));
        }
        out.push(p.render(o)?);
    }
    if out.is_empty() {
        return Err("no number words found in the input".to_string());
    }
    Ok(out.join("\n"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn val(text: &str) -> String {
        let o = Options {
            mode: Mode::Value,
            ..Options::default()
        };
        convert_with(text, &o).unwrap()
    }

    fn rep(text: &str) -> String {
        convert_with(text, &Options::default()).unwrap()
    }

    #[test]
    fn parses_a_plain_compound_number() {
        assert_eq!(val("two hundred forty-three"), "243");
        assert_eq!(val("twenty five"), "25");
        assert_eq!(val("nineteen"), "19");
        assert_eq!(val("zero"), "0");
    }

    #[test]
    fn rejects_words_that_are_not_numbers() {
        let o = Options {
            mode: Mode::Value,
            ..Options::default()
        };
        let err = convert_with("two hundred apples", &o).unwrap_err();
        assert!(err.contains("apples"), "unexpected error: {err}");
        assert!(convert_with("   ", &o).unwrap_err().contains("empty"));
    }

    #[test]
    fn handles_and_only_where_english_uses_it() {
        assert_eq!(val("one hundred and forty-two"), "142");
        assert_eq!(val("one thousand and one"), "1001");
        // Ordinary prose conjunctions must not be swallowed.
        assert_eq!(rep("one and two"), "1 and 2");
    }

    #[test]
    fn handles_scales_including_indian_units() {
        assert_eq!(val("one million two hundred fifty thousand"), "1250000");
        assert_eq!(val("two million three hundred thousand"), "2300000");
        assert_eq!(val("one million, two hundred and fifty thousand"), "1250000");
        assert_eq!(val("three lakh"), "300000");
        assert_eq!(val("two crore fifty lakh"), "25000000");
        assert_eq!(val("a thousand"), "1000");
        assert_eq!(val("a hundred"), "100");
    }

    #[test]
    fn handles_decimal_words() {
        assert_eq!(val("five point forty-seven"), "5.47");
        assert_eq!(val("five point four seven"), "5.47");
        assert_eq!(val("one point five million"), "1500000");
        assert_eq!(val("zero point zero five"), "0.05");
    }

    #[test]
    fn handles_fractions_and_negatives() {
        assert_eq!(val("one and a half"), "1.5");
        assert_eq!(val("six and a quarter"), "6.25");
        assert_eq!(val("three quarters"), "0.75");
        assert_eq!(val("one and three quarters"), "1.75");
        assert_eq!(val("half a million"), "500000");
        assert_eq!(val("minus one hundred"), "-100");
        assert_eq!(val("negative twenty one"), "-21");
    }

    #[test]
    fn fractions_can_be_switched_off() {
        let o = Options {
            mode: Mode::Replace,
            fractions: false,
            ..Options::default()
        };
        assert_eq!(convert_with("one and a half", &o).unwrap(), "1 and a half");
    }

    #[test]
    fn ordinal_modes() {
        assert_eq!(val("twenty-first"), "21");
        let suffix = Options {
            mode: Mode::Replace,
            ordinals: OrdinalMode::Suffix,
            ..Options::default()
        };
        assert_eq!(
            convert_with("the twenty-first of June", &suffix).unwrap(),
            "the 21st of June"
        );
        let ignore = Options {
            mode: Mode::Replace,
            ordinals: OrdinalMode::Ignore,
            ..Options::default()
        };
        assert_eq!(
            convert_with("the twenty-first of June", &ignore).unwrap(),
            "the 20-first of June"
        );
    }

    #[test]
    fn replaces_numbers_inside_prose_and_keeps_the_rest() {
        assert_eq!(
            rep("We shipped twenty-five units and one hundred and two spares."),
            "We shipped 25 units and 102 spares."
        );
        assert_eq!(rep("no numbers here"), "no numbers here");
    }

    #[test]
    fn adjacent_small_words_do_not_silently_sum() {
        // Only "tens + unit" continues a group; everything else is a new number.
        assert_eq!(rep("one two three"), "1 2 3");
        assert_eq!(rep("seventeen five"), "17 5");
        assert_eq!(rep("nineteen eighty four"), "19 84");
        assert_eq!(val("forty seven"), "47");
        assert_eq!(val("two hundred forty seven"), "247");
    }

    #[test]
    fn digit_sequences_are_opt_in() {
        assert_eq!(rep("one two three"), "1 2 3");
        let o = Options {
            mode: Mode::Replace,
            digit_sequences: true,
            ..Options::default()
        };
        assert_eq!(convert_with("one two three", &o).unwrap(), "123");
        assert_eq!(convert_with("nine one one", &o).unwrap(), "911");
        // Grammar still wins when a scale word is present.
        assert_eq!(convert_with("one hundred", &o).unwrap(), "100");
    }

    #[test]
    fn long_scale_changes_billion() {
        let o = Options {
            mode: Mode::Value,
            scale: ScaleSystem::Long,
            ..Options::default()
        };
        assert_eq!(convert_with("one billion", &o).unwrap(), "1000000000000");
        assert_eq!(convert_with("one milliard", &o).unwrap(), "1000000000");
        assert_eq!(val("one billion"), "1000000000");
    }

    #[test]
    fn separators_group_the_output() {
        for (sep, want) in [
            (Separator::None, "1250000"),
            (Separator::Comma, "1,250,000"),
            (Separator::Space, "1 250 000"),
            (Separator::Underscore, "1_250_000"),
        ] {
            let o = Options {
                mode: Mode::Value,
                separator: sep,
                ..Options::default()
            };
            assert_eq!(
                convert_with("one million two hundred fifty thousand", &o).unwrap(),
                want
            );
        }
    }

    #[test]
    fn extract_mode_lists_every_number() {
        let o = Options {
            mode: Mode::Extract,
            ..Options::default()
        };
        assert_eq!(
            convert_with("order twelve widgets and thirty-four bolts", &o).unwrap(),
            "12\n34"
        );
        assert!(convert_with("nothing numeric", &o)
            .unwrap_err()
            .contains("no spelled-out numbers"));
    }

    #[test]
    fn value_mode_handles_multiple_lines() {
        assert_eq!(val("twelve\nthree hundred\n\nminus four"), "12\n300\n-4");
    }

    #[test]
    fn very_large_scales_error_instead_of_losing_precision() {
        let o = Options {
            mode: Mode::Value,
            scale: ScaleSystem::Long,
            ..Options::default()
        };
        let err = convert_with("one septillion", &o).unwrap_err();
        assert!(err.contains("too large"), "unexpected error: {err}");
        assert_eq!(
            val("one decillion"),
            "1000000000000000000000000000000000"
        );
    }

    #[test]
    fn input_length_is_capped() {
        let big = "one ".repeat(MAX_INPUT_CHARS);
        let err = convert_with(&big, &Options::default()).unwrap_err();
        assert!(err.contains("too long"), "unexpected error: {err}");
    }
}
