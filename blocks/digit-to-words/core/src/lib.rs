//! digit-to-words core — spell numbers out as English words.
//!
//! Pure compute, shared by the chat skill block, the CLI and the web page.
//! Everything works on decimal DIGIT STRINGS (never `f64`), so a 60-digit
//! integer spells exactly and a money amount never picks up binary-float noise.

const MAX_INPUT_BYTES: usize = 100_000;
const MAX_LINES: usize = 1_000;
/// Short/long scale: 22 groups of three digits (units .. vigintillion).
const MAX_GROUPS: usize = 22;
const MAX_WESTERN_DIGITS: usize = MAX_GROUPS * 3;
/// Indian scale: 3 digits + 9 two-digit groups (units .. mahashankh).
const MAX_INDIAN_DIGITS: usize = 21;
const MAX_FRACTION_DIGITS: usize = 30;
const MAX_EXPONENT: i64 = 400;

const ONES: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];

const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

/// Short-scale group names, index = group number (1 = thousand).
const SHORT_SCALE: [&str; 22] = [
    "",
    "thousand",
    "million",
    "billion",
    "trillion",
    "quadrillion",
    "quintillion",
    "sextillion",
    "septillion",
    "octillion",
    "nonillion",
    "decillion",
    "undecillion",
    "duodecillion",
    "tredecillion",
    "quattuordecillion",
    "quindecillion",
    "sexdecillion",
    "septendecillion",
    "octodecillion",
    "novemdecillion",
    "vigintillion",
];

/// Long-scale -illion series (10^6, 10^12, 10^18, ...).
const LONG_ILLION: [&str; 11] = [
    "million",
    "billion",
    "trillion",
    "quadrillion",
    "quintillion",
    "sextillion",
    "septillion",
    "octillion",
    "nonillion",
    "decillion",
    "undecillion",
];

/// Long-scale -illiard series (10^9, 10^15, 10^21, ...).
const LONG_ILLIARD: [&str; 11] = [
    "milliard",
    "billiard",
    "trilliard",
    "quadrilliard",
    "quintilliard",
    "sextilliard",
    "septilliard",
    "octilliard",
    "nonilliard",
    "decilliard",
    "undecilliard",
];

/// Indian group names for the two-digit groups above the final three digits.
const INDIAN_SCALE: [&str; 9] = [
    "thousand",
    "lakh",
    "crore",
    "arab",
    "kharab",
    "neel",
    "padma",
    "shankh",
    "mahashankh",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style {
    Cardinal,
    Ordinal,
    OrdinalNum,
    Year,
    Currency,
    Check,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scale {
    Short,
    Long,
    Indian,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    Lower,
    Upper,
    Title,
    Sentence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decimals {
    Point,
    Ignore,
    Round,
}

/// Currency vocabulary: singular/plural main unit, singular/plural sub-unit and
/// how many fraction digits the sub-unit has (0 = the currency has none).
struct Currency {
    unit: &'static str,
    units: &'static str,
    sub: &'static str,
    subs: &'static str,
    sub_digits: usize,
}

const CURRENCIES: [(&str, Currency); 25] = [
    (
        "USD",
        Currency { unit: "dollar", units: "dollars", sub: "cent", subs: "cents", sub_digits: 2 },
    ),
    (
        "EUR",
        Currency { unit: "euro", units: "euros", sub: "cent", subs: "cents", sub_digits: 2 },
    ),
    (
        "GBP",
        Currency { unit: "pound", units: "pounds", sub: "penny", subs: "pence", sub_digits: 2 },
    ),
    (
        "JPY",
        Currency { unit: "yen", units: "yen", sub: "sen", subs: "sen", sub_digits: 0 },
    ),
    (
        "INR",
        Currency { unit: "rupee", units: "rupees", sub: "paisa", subs: "paise", sub_digits: 2 },
    ),
    (
        "CAD",
        Currency {
            unit: "Canadian dollar",
            units: "Canadian dollars",
            sub: "cent",
            subs: "cents",
            sub_digits: 2,
        },
    ),
    (
        "AUD",
        Currency {
            unit: "Australian dollar",
            units: "Australian dollars",
            sub: "cent",
            subs: "cents",
            sub_digits: 2,
        },
    ),
    (
        "CHF",
        Currency {
            unit: "Swiss franc",
            units: "Swiss francs",
            sub: "centime",
            subs: "centimes",
            sub_digits: 2,
        },
    ),
    (
        "CNY",
        Currency { unit: "yuan", units: "yuan", sub: "fen", subs: "fen", sub_digits: 2 },
    ),
    (
        "RUB",
        Currency { unit: "ruble", units: "rubles", sub: "kopeck", subs: "kopecks", sub_digits: 2 },
    ),
    (
        "BRL",
        Currency { unit: "real", units: "reais", sub: "centavo", subs: "centavos", sub_digits: 2 },
    ),
    (
        "MXN",
        Currency { unit: "peso", units: "pesos", sub: "centavo", subs: "centavos", sub_digits: 2 },
    ),
    (
        "ZAR",
        Currency { unit: "rand", units: "rand", sub: "cent", subs: "cents", sub_digits: 2 },
    ),
    (
        "NGN",
        Currency { unit: "naira", units: "naira", sub: "kobo", subs: "kobo", sub_digits: 2 },
    ),
    (
        "KRW",
        Currency { unit: "won", units: "won", sub: "jeon", subs: "jeon", sub_digits: 0 },
    ),
    (
        "NZD",
        Currency {
            unit: "New Zealand dollar",
            units: "New Zealand dollars",
            sub: "cent",
            subs: "cents",
            sub_digits: 2,
        },
    ),
    (
        "SGD",
        Currency {
            unit: "Singapore dollar",
            units: "Singapore dollars",
            sub: "cent",
            subs: "cents",
            sub_digits: 2,
        },
    ),
    (
        "HKD",
        Currency {
            unit: "Hong Kong dollar",
            units: "Hong Kong dollars",
            sub: "cent",
            subs: "cents",
            sub_digits: 2,
        },
    ),
    (
        "SEK",
        Currency { unit: "krona", units: "kronor", sub: "öre", subs: "öre", sub_digits: 2 },
    ),
    (
        "NOK",
        Currency { unit: "krone", units: "kroner", sub: "øre", subs: "øre", sub_digits: 2 },
    ),
    (
        "DKK",
        Currency { unit: "krone", units: "kroner", sub: "øre", subs: "øre", sub_digits: 2 },
    ),
    (
        "PLN",
        Currency { unit: "zloty", units: "zlotys", sub: "grosz", subs: "groszy", sub_digits: 2 },
    ),
    (
        "TRY",
        Currency { unit: "lira", units: "lira", sub: "kurus", subs: "kurus", sub_digits: 2 },
    ),
    (
        "PHP",
        Currency {
            unit: "Philippine peso",
            units: "Philippine pesos",
            sub: "centavo",
            subs: "centavos",
            sub_digits: 2,
        },
    ),
    (
        "THB",
        Currency { unit: "baht", units: "baht", sub: "satang", subs: "satang", sub_digits: 2 },
    ),
];

/// Every currency code the tool accepts, in descriptor/manifest order.
pub const CURRENCY_CODES: [&str; 25] = [
    "USD", "EUR", "GBP", "JPY", "INR", "CAD", "AUD", "CHF", "CNY", "RUB", "BRL", "MXN", "ZAR",
    "NGN", "KRW", "NZD", "SGD", "HKD", "SEK", "NOK", "DKK", "PLN", "TRY", "PHP", "THB",
];

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub style: Style,
    pub scale: Scale,
    pub case: Case,
    pub currency: &'static str,
    pub use_and: bool,
    pub hyphenate: bool,
    pub decimals: Decimals,
    pub only_suffix: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            style: Style::Cardinal,
            scale: Scale::Short,
            case: Case::Lower,
            currency: "USD",
            use_and: false,
            hyphenate: true,
            decimals: Decimals::Point,
            only_suffix: false,
        }
    }
}

/// A parsed decimal: sign plus two digit strings. `int` never has leading zeros
/// (except the single "0"), `frac` has no meaning-free trailing trimming — the
/// digits the user typed are kept so "1.50" can read "one point five zero".
#[derive(Clone, Debug, PartialEq, Eq)]
struct Decimal {
    negative: bool,
    int: String,
    frac: String,
}

impl Decimal {
    fn is_zero(&self) -> bool {
        self.int.bytes().all(|b| b == b'0') && self.frac.bytes().all(|b| b == b'0')
    }
}

// ---------------------------------------------------------------------------
// option parsing
// ---------------------------------------------------------------------------

fn parse_style(s: &str) -> Result<Style, String> {
    match s.trim() {
        "" | "cardinal" => Ok(Style::Cardinal),
        "ordinal" => Ok(Style::Ordinal),
        "ordinal_num" => Ok(Style::OrdinalNum),
        "year" => Ok(Style::Year),
        "currency" => Ok(Style::Currency),
        "check" => Ok(Style::Check),
        other => Err(format!(
            "unknown style '{other}': expected cardinal, ordinal, ordinal_num, year, currency or check"
        )),
    }
}

fn parse_scale(s: &str) -> Result<Scale, String> {
    match s.trim() {
        "" | "short" => Ok(Scale::Short),
        "long" => Ok(Scale::Long),
        "indian" => Ok(Scale::Indian),
        other => Err(format!(
            "unknown scale '{other}': expected short, long or indian"
        )),
    }
}

fn parse_case(s: &str) -> Result<Case, String> {
    match s.trim() {
        "" | "lower" => Ok(Case::Lower),
        "upper" => Ok(Case::Upper),
        "title" => Ok(Case::Title),
        "sentence" => Ok(Case::Sentence),
        other => Err(format!(
            "unknown letter_case '{other}': expected lower, upper, title or sentence"
        )),
    }
}

fn parse_decimals(s: &str) -> Result<Decimals, String> {
    match s.trim() {
        "" | "point" => Ok(Decimals::Point),
        "ignore" => Ok(Decimals::Ignore),
        "round" => Ok(Decimals::Round),
        other => Err(format!(
            "unknown decimals '{other}': expected point, ignore or round"
        )),
    }
}

fn parse_currency(s: &str) -> Result<&'static str, String> {
    let want = s.trim();
    let want = if want.is_empty() { "USD" } else { want };
    let upper = want.to_ascii_uppercase();
    CURRENCY_CODES
        .iter()
        .copied()
        .find(|c| *c == upper)
        .ok_or_else(|| {
            format!(
                "unknown currency '{want}': expected one of {}",
                CURRENCY_CODES.join(", ")
            )
        })
}

fn currency_data(code: &str) -> &'static Currency {
    &CURRENCIES
        .iter()
        .find(|(c, _)| *c == code)
        .expect("currency code validated by parse_currency")
        .1
}

// ---------------------------------------------------------------------------
// number parsing
// ---------------------------------------------------------------------------

const CURRENCY_SYMBOLS: [char; 12] =
    ['$', '€', '£', '¥', '₹', '₽', '₩', '¢', '₦', '₺', '₱', '฿'];

/// Parse one user-written number: separators, currency symbols, accounting
/// parentheses, a leading "minus", and scientific notation are all accepted.
fn parse_decimal(raw: &str) -> Result<Decimal, String> {
    let original = raw.trim();
    if original.is_empty() {
        return Err("empty number".to_string());
    }

    let mut s: String = original.to_string();

    // "minus 12" / "negative 12"
    let mut negative = false;
    let lowered = s.to_ascii_lowercase();
    for word in ["minus ", "negative "] {
        if let Some(rest) = lowered.strip_prefix(word) {
            negative = true;
            s = rest.to_string();
            break;
        }
    }

    // Drop currency symbols and any ASCII letters that belong to a code suffix
    // is NOT done here — letters other than a trailing exponent are an error.
    s = s
        .chars()
        .filter(|c| !CURRENCY_SYMBOLS.contains(c) && !c.is_whitespace())
        .collect();

    // Accounting negatives: (1234.50)
    if s.starts_with('(') && s.ends_with(')') && s.len() >= 3 {
        negative = !negative;
        s = s[1..s.len() - 1].to_string();
    }

    if let Some(rest) = s.strip_prefix('-') {
        negative = !negative;
        s = rest.to_string();
    } else if let Some(rest) = s.strip_prefix('+') {
        s = rest.to_string();
    }

    if s.is_empty() {
        return Err(format!("'{original}' has no digits"));
    }

    // Scientific notation — only when it really looks like one, so a word such
    // as "twelve" still reports the offending letter instead of a bad exponent.
    let mut exponent: i64 = 0;
    if let Some(pos) = s.find(['e', 'E']) {
        let (mantissa, exp) = s.split_at(pos);
        let exp = &exp[1..];
        let exp_looks_numeric = !exp.is_empty()
            && exp
                .char_indices()
                .all(|(i, c)| c.is_ascii_digit() || (i == 0 && (c == '-' || c == '+')));
        let mantissa_has_digit = mantissa.bytes().any(|b| b.is_ascii_digit());
        if exp_looks_numeric && mantissa_has_digit {
            exponent = exp.parse::<i64>().map_err(|_| {
                format!("'{original}' has an unreadable exponent '{exp}': expected digits like e6 or e-3")
            })?;
            if exponent.abs() > MAX_EXPONENT {
                return Err(format!(
                    "'{original}' exponent {exponent} is out of range: keep it within ±{MAX_EXPONENT}"
                ));
            }
            s = mantissa.to_string();
        }
    }

    // Decide which separator is the decimal point.
    let dots = s.matches('.').count();
    let commas = s.matches(',').count();
    let decimal_is_comma = dots == 0
        && commas == 1
        && s.split(',').nth(1).map(|t| t.len() != 3).unwrap_or(false);

    let mut int = String::new();
    let mut frac = String::new();
    let mut seen_point = false;
    for c in s.chars() {
        match c {
            '0'..='9' => {
                if seen_point {
                    frac.push(c)
                } else {
                    int.push(c)
                }
            }
            '.' if !decimal_is_comma => {
                if seen_point {
                    return Err(format!("'{original}' has more than one decimal point"));
                }
                seen_point = true;
            }
            ',' if decimal_is_comma => {
                seen_point = true;
            }
            ',' | '_' | '\'' | '.' | ' ' => {} // grouping separators
            other => {
                return Err(format!(
                    "'{original}' contains '{other}', which is not part of a number"
                ))
            }
        }
    }

    if int.is_empty() && frac.is_empty() {
        return Err(format!("'{original}' has no digits"));
    }

    let mut value = Decimal { negative, int, frac };
    if exponent != 0 {
        value = shift_point(value, exponent, original)?;
    }
    normalize(&mut value);

    if value.frac.len() > MAX_FRACTION_DIGITS {
        return Err(format!(
            "'{original}' has {} fraction digits: at most {MAX_FRACTION_DIGITS} are supported",
            value.frac.len()
        ));
    }
    Ok(value)
}

/// Move the decimal point by `exponent` places (scientific notation).
fn shift_point(value: Decimal, exponent: i64, original: &str) -> Result<Decimal, String> {
    let digits: String = format!("{}{}", value.int, value.frac);
    let point = value.int.len() as i64 + exponent;
    let total = digits.len() as i64 + exponent.abs();
    if total > 1_200 {
        return Err(format!("'{original}' expands to too many digits to spell"));
    }
    let (int, frac) = if point <= 0 {
        (
            "0".to_string(),
            format!("{}{}", "0".repeat((-point) as usize), digits),
        )
    } else if point as usize >= digits.len() {
        (
            format!("{}{}", digits, "0".repeat(point as usize - digits.len())),
            String::new(),
        )
    } else {
        let p = point as usize;
        (digits[..p].to_string(), digits[p..].to_string())
    };
    Ok(Decimal { negative: value.negative, int, frac })
}

fn normalize(value: &mut Decimal) {
    while value.int.len() > 1 && value.int.starts_with('0') {
        value.int.remove(0);
    }
    if value.int.is_empty() {
        value.int.push('0');
    }
    if value.is_zero() {
        value.negative = false;
    }
}

/// Round a decimal to `places` fraction digits, half away from zero.
fn round_to(value: &Decimal, places: usize) -> Decimal {
    if value.frac.len() <= places {
        let mut frac = value.frac.clone();
        while frac.len() < places {
            frac.push('0');
        }
        let mut out = Decimal { negative: value.negative, int: value.int.clone(), frac };
        normalize(&mut out);
        return out;
    }
    let keep = &value.frac[..places];
    let next = value.frac.as_bytes()[places] - b'0';
    let mut digits: Vec<u8> = value
        .int
        .bytes()
        .chain(keep.bytes())
        .map(|b| b - b'0')
        .collect();
    if next >= 5 {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, 1);
                break;
            }
            i -= 1;
            if digits[i] == 9 {
                digits[i] = 0;
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    let s: String = digits.iter().map(|d| (d + b'0') as char).collect();
    let cut = s.len() - places;
    let mut out = Decimal {
        negative: value.negative,
        int: s[..cut].to_string(),
        frac: s[cut..].to_string(),
    };
    normalize(&mut out);
    out
}

// ---------------------------------------------------------------------------
// integer -> words
// ---------------------------------------------------------------------------

fn two_digit_words(n: u16, hyphenate: bool) -> String {
    if n < 20 {
        return ONES[n as usize].to_string();
    }
    let tens = TENS[(n / 10) as usize];
    let unit = n % 10;
    if unit == 0 {
        tens.to_string()
    } else {
        format!("{tens}{}{}", if hyphenate { "-" } else { " " }, ONES[unit as usize])
    }
}

fn three_digit_words(n: u16, use_and: bool, hyphenate: bool) -> String {
    let hundreds = n / 100;
    let rest = n % 100;
    if hundreds == 0 {
        return two_digit_words(rest, hyphenate);
    }
    let head = format!("{} hundred", ONES[hundreds as usize]);
    if rest == 0 {
        head
    } else if use_and {
        format!("{head} and {}", two_digit_words(rest, hyphenate))
    } else {
        format!("{head} {}", two_digit_words(rest, hyphenate))
    }
}

fn western_group_name(scale: Scale, group: usize) -> Result<&'static str, String> {
    match scale {
        Scale::Short => Ok(SHORT_SCALE[group]),
        Scale::Long => {
            if group == 0 {
                Ok("")
            } else if group == 1 {
                Ok("thousand")
            } else if group % 2 == 0 {
                Ok(LONG_ILLION[group / 2 - 1])
            } else {
                Ok(LONG_ILLIARD[(group - 1) / 2 - 1])
            }
        }
        Scale::Indian => Err("indian scale does not use three-digit groups".to_string()),
    }
}

/// Spell a non-negative integer given as a digit string.
fn int_to_words(digits: &str, o: &Options) -> Result<String, String> {
    let trimmed = {
        let t = digits.trim_start_matches('0');
        if t.is_empty() {
            "0"
        } else {
            t
        }
    };
    if trimmed == "0" {
        return Ok("zero".to_string());
    }
    match o.scale {
        Scale::Indian => indian_to_words(trimmed, o),
        _ => western_to_words(trimmed, o),
    }
}

fn western_to_words(digits: &str, o: &Options) -> Result<String, String> {
    if digits.len() > MAX_WESTERN_DIGITS {
        return Err(format!(
            "{} digits is beyond the {} scale (maximum {MAX_WESTERN_DIGITS} digits, up to {})",
            digits.len(),
            if o.scale == Scale::Long { "long" } else { "short" },
            if o.scale == Scale::Long { "undecilliard" } else { "vigintillion" }
        ));
    }
    let bytes = digits.as_bytes();
    let group_count = bytes.len().div_ceil(3);
    if group_count > MAX_GROUPS {
        return Err(format!("{} digits is too many to spell", digits.len()));
    }
    let mut parts: Vec<String> = Vec::new();
    for gi in 0..group_count {
        let group = group_count - 1 - gi; // 0 = units
        let end = bytes.len() - group * 3;
        let start = end.saturating_sub(3);
        let value: u16 = digits[start..end].parse().unwrap_or(0);
        if value == 0 {
            continue;
        }
        let mut part = three_digit_words(value, o.use_and, o.hyphenate);
        if o.use_and && group == 0 && value < 100 && !parts.is_empty() {
            part = format!("and {part}");
        }
        let name = western_group_name(o.scale, group)?;
        if !name.is_empty() {
            part.push(' ');
            part.push_str(name);
        }
        parts.push(part);
    }
    Ok(parts.join(" "))
}

fn indian_to_words(digits: &str, o: &Options) -> Result<String, String> {
    if digits.len() > MAX_INDIAN_DIGITS {
        return Err(format!(
            "{} digits is beyond the indian scale (maximum {MAX_INDIAN_DIGITS} digits, up to mahashankh)",
            digits.len()
        ));
    }
    let (head, tail) = if digits.len() > 3 {
        digits.split_at(digits.len() - 3)
    } else {
        ("", digits)
    };
    let tail_value: u16 = tail.parse().unwrap_or(0);

    // Split `head` into two-digit groups, least significant first.
    let mut pairs: Vec<u16> = Vec::new();
    let mut rest = head;
    while !rest.is_empty() {
        let start = rest.len().saturating_sub(2);
        pairs.push(rest[start..].parse().unwrap_or(0));
        rest = &rest[..start];
    }
    if pairs.len() > INDIAN_SCALE.len() {
        return Err(format!("{} digits is too many to spell", digits.len()));
    }

    let mut parts: Vec<String> = Vec::new();
    for idx in (0..pairs.len()).rev() {
        let value = pairs[idx];
        if value == 0 {
            continue;
        }
        parts.push(format!(
            "{} {}",
            two_digit_words(value, o.hyphenate),
            INDIAN_SCALE[idx]
        ));
    }
    if tail_value != 0 {
        let mut part = three_digit_words(tail_value, o.use_and, o.hyphenate);
        if o.use_and && tail_value < 100 && !parts.is_empty() {
            part = format!("and {part}");
        }
        parts.push(part);
    }
    Ok(parts.join(" "))
}

// ---------------------------------------------------------------------------
// ordinals
// ---------------------------------------------------------------------------

fn ordinal_word(word: &str) -> String {
    match word {
        "zero" => return "zeroth".to_string(),
        "one" => return "first".to_string(),
        "two" => return "second".to_string(),
        "three" => return "third".to_string(),
        "four" => return "fourth".to_string(),
        "five" => return "fifth".to_string(),
        "six" => return "sixth".to_string(),
        "seven" => return "seventh".to_string(),
        "eight" => return "eighth".to_string(),
        "nine" => return "ninth".to_string(),
        "twelve" => return "twelfth".to_string(),
        _ => {}
    }
    if let Some(stem) = word.strip_suffix('y') {
        return format!("{stem}ieth");
    }
    format!("{word}th")
}

/// Turn a cardinal phrase into its ordinal form by rewriting the final word.
fn ordinalize(words: &str) -> String {
    let cut = words
        .rfind(|c: char| c == ' ' || c == '-')
        .map(|i| i + 1)
        .unwrap_or(0);
    format!("{}{}", &words[..cut], ordinal_word(&words[cut..]))
}

/// The `st`/`nd`/`rd`/`th` suffix for a digit string (11-13 are always `th`).
fn ordinal_suffix(digits: &str) -> &'static str {
    let start = digits.len().saturating_sub(2);
    let last_two: u32 = digits[start..].parse().unwrap_or(0);
    if (11..=13).contains(&(last_two % 100)) {
        return "th";
    }
    match last_two % 10 {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    }
}

// ---------------------------------------------------------------------------
// years
// ---------------------------------------------------------------------------

/// Read a 4-digit year the way English speakers say it: "nineteen eighty-four",
/// "nineteen oh five", "nineteen hundred", but "two thousand five" for the
/// x000-x009 block. Anything that isn't four digits falls back to the cardinal.
fn year_words(digits: &str, o: &Options) -> Result<String, String> {
    if digits.len() == 4 {
        let high: u16 = digits[..2].parse().unwrap_or(0);
        let low: u16 = digits[2..].parse().unwrap_or(0);
        let reads_as_thousand = low < 10 && high % 10 == 0;
        if high != 0 && !reads_as_thousand {
            return Ok(if low == 0 {
                format!("{} hundred", two_digit_words(high, o.hyphenate))
            } else if low < 10 {
                format!(
                    "{} oh {}",
                    two_digit_words(high, o.hyphenate),
                    ONES[low as usize]
                )
            } else {
                format!(
                    "{} {}",
                    two_digit_words(high, o.hyphenate),
                    two_digit_words(low, o.hyphenate)
                )
            });
        }
    }
    int_to_words(digits, o)
}

// ---------------------------------------------------------------------------
// letter case
// ---------------------------------------------------------------------------

fn apply_case(text: &str, case: Case) -> String {
    match case {
        Case::Lower => text.to_lowercase(),
        Case::Upper => text.to_uppercase(),
        Case::Title => {
            let mut out = String::with_capacity(text.len());
            let mut start_of_word = true;
            for c in text.chars() {
                if c.is_alphabetic() {
                    if start_of_word {
                        out.extend(c.to_uppercase());
                    } else {
                        out.extend(c.to_lowercase());
                    }
                    start_of_word = false;
                } else {
                    out.push(c);
                    // Only whitespace and the compound hyphen start a new word —
                    // digits must not, or "4732nd" would title-case to "4732Nd".
                    start_of_word = c.is_whitespace() || c == '-';
                }
            }
            out
        }
        Case::Sentence => {
            let lower = text.to_lowercase();
            let mut out = String::with_capacity(lower.len());
            let mut done = false;
            for c in lower.chars() {
                if !done && c.is_alphabetic() {
                    out.extend(c.to_uppercase());
                    done = true;
                } else {
                    out.push(c);
                }
            }
            out
        }
    }
}

// ---------------------------------------------------------------------------
// styles
// ---------------------------------------------------------------------------

fn spell_fraction_digits(frac: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for b in frac.bytes() {
        parts.push(ONES[(b - b'0') as usize]);
    }
    parts.join(" ")
}

fn spell_cardinal(value: &Decimal, o: &Options) -> Result<String, String> {
    let value = match o.decimals {
        Decimals::Point => value.clone(),
        Decimals::Ignore => Decimal {
            negative: value.negative,
            int: value.int.clone(),
            frac: String::new(),
        },
        Decimals::Round => round_to(value, 0),
    };
    let mut words = int_to_words(&value.int, o)?;
    if o.decimals == Decimals::Point && !value.frac.is_empty() {
        words.push_str(" point ");
        words.push_str(&spell_fraction_digits(&value.frac));
    }
    if value.negative && !value.is_zero() {
        words = format!("minus {words}");
    }
    Ok(words)
}

/// Drop the fraction for the styles that only make sense on a whole number,
/// honouring the `decimals` setting (and naming the fix when it can't).
fn whole_number(value: &Decimal, o: &Options, style_name: &str) -> Result<Decimal, String> {
    Ok(match o.decimals {
        Decimals::Ignore => Decimal {
            negative: value.negative,
            int: value.int.clone(),
            frac: String::new(),
        },
        Decimals::Round => round_to(value, 0),
        Decimals::Point => {
            if !value.frac.trim_end_matches('0').is_empty() {
                return Err(format!(
                    "{style_name} style needs a whole number, got '{}.{}': set decimals to round or ignore, or drop the fraction",
                    value.int, value.frac
                ));
            }
            Decimal {
                negative: value.negative,
                int: value.int.clone(),
                frac: String::new(),
            }
        }
    })
}

fn spell_ordinal(value: &Decimal, o: &Options) -> Result<String, String> {
    let whole = whole_number(value, o, "ordinal")?;
    let mut words = ordinalize(&int_to_words(&whole.int, o)?);
    if whole.negative && !whole.is_zero() {
        words = format!("minus {words}");
    }
    Ok(words)
}

/// `4732` -> `4732nd` — the digits kept, only the suffix spelled.
fn spell_ordinal_num(value: &Decimal, o: &Options) -> Result<String, String> {
    let whole = whole_number(value, o, "ordinal_num")?;
    if whole.int.len() > MAX_WESTERN_DIGITS {
        return Err(format!(
            "{} digits is too many to number: at most {MAX_WESTERN_DIGITS} are supported",
            whole.int.len()
        ));
    }
    let mut out = format!("{}{}", whole.int, ordinal_suffix(&whole.int));
    if whole.negative && !whole.is_zero() {
        out = format!("minus {out}");
    }
    Ok(out)
}

fn spell_year(value: &Decimal, o: &Options) -> Result<String, String> {
    let whole = whole_number(value, o, "year")?;
    let mut words = year_words(&whole.int, o)?;
    if whole.negative && !whole.is_zero() {
        words = format!("{words} BC");
    }
    Ok(words)
}

fn spell_currency(value: &Decimal, o: &Options) -> Result<String, String> {
    let cur = currency_data(o.currency);
    let rounded = round_to(value, cur.sub_digits);
    let main = int_to_words(&rounded.int, o)?;
    let main_is_one = rounded.int == "1";
    let sub_value: u16 = if rounded.frac.is_empty() {
        0
    } else {
        rounded.frac.parse().unwrap_or(0)
    };

    let mut parts: Vec<String> = Vec::new();
    if rounded.int != "0" || sub_value == 0 {
        parts.push(format!(
            "{main} {}",
            if main_is_one { cur.unit } else { cur.units }
        ));
    }
    if sub_value != 0 {
        parts.push(format!(
            "{} {}",
            three_digit_words(sub_value, o.use_and, o.hyphenate),
            if sub_value == 1 { cur.sub } else { cur.subs }
        ));
    }
    let mut out = parts.join(" and ");
    if rounded.negative && !rounded.is_zero() {
        out = format!("minus {out}");
    }
    Ok(out)
}

fn spell_check(value: &Decimal, o: &Options) -> Result<String, String> {
    let cur = currency_data(o.currency);
    let rounded = round_to(value, cur.sub_digits);
    let main = int_to_words(&rounded.int, o)?;
    let mut out = if cur.sub_digits == 0 {
        format!("{main} {}", cur.units)
    } else {
        format!(
            "{main} and {}/{} {}",
            rounded.frac,
            10usize.pow(cur.sub_digits as u32),
            cur.units
        )
    };
    if rounded.negative && !rounded.is_zero() {
        out = format!("minus {out}");
    }
    Ok(out)
}

fn spell_one(raw: &str, o: &Options) -> Result<String, String> {
    let value = parse_decimal(raw)?;
    let words = match o.style {
        Style::Cardinal => spell_cardinal(&value, o)?,
        Style::Ordinal => spell_ordinal(&value, o)?,
        Style::OrdinalNum => spell_ordinal_num(&value, o)?,
        Style::Year => spell_year(&value, o)?,
        Style::Currency => spell_currency(&value, o)?,
        Style::Check => spell_check(&value, o)?,
    };
    let words = if o.only_suffix {
        format!("{words} only")
    } else {
        words
    };
    Ok(apply_case(&words, o.case))
}

// ---------------------------------------------------------------------------
// public entry points
// ---------------------------------------------------------------------------

/// Spell every non-blank line of `numbers`, one result per line.
pub fn convert_with(numbers: &str, o: &Options) -> Result<String, String> {
    if numbers.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes: the limit is {MAX_INPUT_BYTES} bytes",
            numbers.len()
        ));
    }
    let lines: Vec<&str> = numbers
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return Err("no number to spell: enter a number such as 1234 or 25.40".to_string());
    }
    if lines.len() > MAX_LINES {
        return Err(format!(
            "{} numbers: the limit is {MAX_LINES} per run",
            lines.len()
        ));
    }
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        match spell_one(line, o) {
            Ok(words) => out.push(words),
            Err(e) => {
                return Err(if lines.len() == 1 {
                    e
                } else {
                    format!("line {}: {e}", i + 1)
                })
            }
        }
    }
    Ok(out.join("\n"))
}

/// Default-options convenience wrapper: lower-case short-scale cardinals, one
/// per line. Callers that need a style, scale or currency use [`convert`].
pub fn run(input: &str) -> Result<String, String> {
    convert_with(input, &Options::default())
}

/// String-argument entry point shared by the block, the CLI and the web export.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    numbers: &str,
    style: &str,
    scale: &str,
    letter_case: &str,
    currency: &str,
    use_and: bool,
    hyphenate: bool,
    decimals: &str,
    only_suffix: bool,
) -> Result<String, String> {
    let o = Options {
        style: parse_style(style)?,
        scale: parse_scale(scale)?,
        case: parse_case(letter_case)?,
        currency: parse_currency(currency)?,
        use_and,
        hyphenate,
        decimals: parse_decimals(decimals)?,
        only_suffix,
    };
    convert_with(numbers, &o)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cardinal(n: &str) -> String {
        convert_with(n, &Options::default()).unwrap()
    }

    #[test]
    fn run_uses_default_options() {
        assert_eq!(run("342").unwrap(), "three hundred forty-two");
        assert_eq!(run("1\n2").unwrap(), "one\ntwo");
        assert!(run("").is_err());
    }

    #[test]
    fn spells_small_and_compound_numbers() {
        assert_eq!(cardinal("0"), "zero");
        assert_eq!(cardinal("7"), "seven");
        assert_eq!(cardinal("13"), "thirteen");
        assert_eq!(cardinal("21"), "twenty-one");
        assert_eq!(cardinal("100"), "one hundred");
        assert_eq!(cardinal("342"), "three hundred forty-two");
        assert_eq!(cardinal("1000"), "one thousand");
        assert_eq!(
            cardinal("1234567"),
            "one million two hundred thirty-four thousand five hundred sixty-seven"
        );
    }

    #[test]
    fn handles_separators_signs_and_symbols() {
        assert_eq!(cardinal("1,234"), "one thousand two hundred thirty-four");
        assert_eq!(cardinal("$1 234"), "one thousand two hundred thirty-four");
        assert_eq!(cardinal("-12"), "minus twelve");
        assert_eq!(cardinal("(12)"), "minus twelve");
        assert_eq!(cardinal("minus 12"), "minus twelve");
        assert_eq!(cardinal("1.5e3"), "one thousand five hundred");
        assert_eq!(cardinal("1.50"), "one point five zero");
        assert_eq!(cardinal("1234,5"), "one thousand two hundred thirty-four point five");
    }

    #[test]
    fn scales_differ_where_they_should() {
        let mut o = Options::default();
        assert_eq!(convert_with("1000000000", &o).unwrap(), "one billion");
        o.scale = Scale::Long;
        assert_eq!(convert_with("1000000000", &o).unwrap(), "one milliard");
        assert_eq!(convert_with("1000000000000", &o).unwrap(), "one billion");
        o.scale = Scale::Indian;
        assert_eq!(convert_with("100000", &o).unwrap(), "one lakh");
        assert_eq!(
            convert_with("12345678", &o).unwrap(),
            "one crore twenty-three lakh forty-five thousand six hundred seventy-eight"
        );
    }

    #[test]
    fn british_and_hyphen_and_case_options() {
        let mut o = Options::default();
        o.use_and = true;
        assert_eq!(
            convert_with("1023", &o).unwrap(),
            "one thousand and twenty-three"
        );
        assert_eq!(
            convert_with("1234", &o).unwrap(),
            "one thousand two hundred and thirty-four"
        );
        o.use_and = false;
        o.hyphenate = false;
        assert_eq!(convert_with("21", &o).unwrap(), "twenty one");
        o.hyphenate = true;
        o.case = Case::Upper;
        assert_eq!(convert_with("21", &o).unwrap(), "TWENTY-ONE");
        o.case = Case::Title;
        assert_eq!(convert_with("21", &o).unwrap(), "Twenty-One");
        o.case = Case::Sentence;
        assert_eq!(convert_with("21", &o).unwrap(), "Twenty-one");
    }

    #[test]
    fn ordinal_currency_and_check_styles() {
        let mut o = Options::default();
        o.style = Style::Ordinal;
        assert_eq!(convert_with("1", &o).unwrap(), "first");
        assert_eq!(convert_with("21", &o).unwrap(), "twenty-first");
        assert_eq!(convert_with("40", &o).unwrap(), "fortieth");
        assert_eq!(convert_with("1000000", &o).unwrap(), "one millionth");

        o.style = Style::Currency;
        assert_eq!(
            convert_with("25.40", &o).unwrap(),
            "twenty-five dollars and forty cents"
        );
        assert_eq!(convert_with("1.01", &o).unwrap(), "one dollar and one cent");
        assert_eq!(convert_with("25", &o).unwrap(), "twenty-five dollars");
        assert_eq!(convert_with("0.40", &o).unwrap(), "forty cents");

        o.style = Style::Check;
        o.case = Case::Sentence;
        assert_eq!(
            convert_with("14273.38", &o).unwrap(),
            "Fourteen thousand two hundred seventy-three and 38/100 dollars"
        );
        assert_eq!(
            convert_with("500", &o).unwrap(),
            "Five hundred and 00/100 dollars"
        );
    }

    #[test]
    fn currency_vocabularies_and_zero_decimal_currencies() {
        let mut o = Options::default();
        o.style = Style::Currency;
        o.currency = "GBP";
        assert_eq!(convert_with("2.01", &o).unwrap(), "two pounds and one penny");
        assert_eq!(convert_with("2.05", &o).unwrap(), "two pounds and five pence");
        o.currency = "JPY";
        assert_eq!(convert_with("1500.7", &o).unwrap(), "one thousand five hundred one yen");
        o.style = Style::Check;
        assert_eq!(convert_with("1500", &o).unwrap(), "one thousand five hundred yen");
        o.currency = "INR";
        o.scale = Scale::Indian;
        o.only_suffix = true;
        o.case = Case::Title;
        assert_eq!(
            convert_with("125000", &o).unwrap(),
            "One Lakh Twenty-Five Thousand And 00/100 Rupees Only"
        );
    }

    #[test]
    fn rounding_and_decimal_modes() {
        let mut o = Options::default();
        o.decimals = Decimals::Round;
        assert_eq!(convert_with("2.5", &o).unwrap(), "three");
        assert_eq!(convert_with("2.4", &o).unwrap(), "two");
        assert_eq!(convert_with("9.99", &o).unwrap(), "ten");
        o.decimals = Decimals::Ignore;
        assert_eq!(convert_with("2.9", &o).unwrap(), "two");
        o.style = Style::Currency;
        o.decimals = Decimals::Point;
        assert_eq!(
            convert_with("1.005", &o).unwrap(),
            "one dollar and one cent"
        );
    }

    #[test]
    fn very_large_integers_stay_exact() {
        let o = Options::default();
        assert_eq!(
            convert_with("1000000000000000000000000000000000", &o).unwrap(),
            "one decillion"
        );
        let sixty_six = format!("1{}", "0".repeat(65));
        assert_eq!(convert_with(&sixty_six, &o).unwrap(), "one hundred vigintillion");
        let sixty_seven = format!("1{}", "0".repeat(66));
        assert!(convert_with(&sixty_seven, &o)
            .unwrap_err()
            .contains("beyond the short scale"));
    }

    #[test]
    fn batches_one_number_per_line() {
        let o = Options::default();
        assert_eq!(convert_with("1\n2\n\n3", &o).unwrap(), "one\ntwo\nthree");
    }

    #[test]
    fn errors_name_what_went_wrong() {
        let o = Options::default();
        assert!(convert_with("", &o).unwrap_err().contains("no number to spell"));
        assert!(convert_with("twelve", &o)
            .unwrap_err()
            .contains("is not part of a number"));
        assert!(convert_with("1\nabc", &o).unwrap_err().starts_with("line 2:"));
        assert!(convert_with("1.2.3", &o)
            .unwrap_err()
            .contains("more than one decimal point"));

        let mut o = Options::default();
        o.style = Style::Ordinal;
        assert!(convert_with("2.5", &o)
            .unwrap_err()
            .contains("ordinal style needs a whole number"));

        assert!(convert("1", "nope", "short", "lower", "USD", false, true, "point", false)
            .unwrap_err()
            .contains("unknown style"));
        assert!(convert("1", "cardinal", "short", "lower", "XYZ", false, true, "point", false)
            .unwrap_err()
            .contains("unknown currency"));
    }
}
