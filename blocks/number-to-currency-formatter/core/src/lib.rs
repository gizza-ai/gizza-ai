//! number-to-currency-formatter core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps, no network, no exchange rates: this
//! is presentation-only formatting of a number you already have.
//!
//! Correctness note: rounding happens on the DIGIT STRING you typed, not on the
//! binary `f64`, so `1.005` at 2 places is `1.01` (a naive `x*100.0` round returns
//! `1.00` because `1.005` isn't representable in binary — the classic money bug).
//! Only scientific-notation input (`1.5e3`) falls back to `f64` expansion.

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// How a trailing digit that has to be dropped is resolved.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoundMode {
    /// Round half away from zero (classical / spreadsheet ROUND). 2.5→3, -2.5→-3.
    HalfUp,
    /// Round half toward zero. 2.5→2, -2.5→-2.
    HalfDown,
    /// Round half to even (banker's rounding). 0.5→0, 1.5→2, 2.5→2.
    HalfEven,
    /// Always round toward +∞. 2.1→3, -2.9→-2.
    Ceil,
    /// Always round toward -∞. 2.9→2, -2.1→-3.
    Floor,
    /// Drop the extra digits (round toward zero). 2.9→2, -2.9→-2.
    Truncate,
}

/// Where the currency symbol/code sits relative to the digits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Position {
    Before,
    After,
}

/// What the currency is rendered as.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SymbolStyle {
    /// The symbol for a known ISO code (`USD` → `$`), else the text as typed.
    Symbol,
    /// The ISO code itself (`USD`), else the text as typed.
    Code,
    /// Digits only — no symbol, no code.
    None,
}

/// How the sign is shown for positive / zero / negative values.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SignStyle {
    /// `-` on negatives only.
    Auto,
    /// `+` on zero and positives, `-` on negatives.
    Always,
    /// `+` on positives, nothing on zero, `-` on negatives.
    ExceptZero,
    /// No sign at all — the absolute value.
    Never,
    /// A space where the `+` would go, so a column of values lines up.
    Space,
}

/// How the integer digits are chunked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DigitGrouping {
    /// Uniform 3-digit groups: `1,234,567`.
    Western,
    /// South-Asian lakh/crore: 3 digits, then 2s — `12,34,567`.
    Indian,
}

// ---------------------------------------------------------------------------
// Currency table — presentation only, no rates. Unknown codes fall through as
// literal text so a user can type `Kč`, `zł`, or anything else and get it back.
// ---------------------------------------------------------------------------

const CURRENCIES: &[(&str, &str)] = &[
    ("USD", "$"),
    ("EUR", "€"),
    ("GBP", "£"),
    ("JPY", "¥"),
    ("CNY", "¥"),
    ("INR", "₹"),
    ("KRW", "₩"),
    ("RUB", "₽"),
    ("BRL", "R$"),
    ("CAD", "CA$"),
    ("AUD", "A$"),
    ("NZD", "NZ$"),
    ("CHF", "CHF"),
    ("SEK", "kr"),
    ("NOK", "kr"),
    ("DKK", "kr"),
    ("ISK", "kr"),
    ("PLN", "zł"),
    ("CZK", "Kč"),
    ("HUF", "Ft"),
    ("RON", "lei"),
    ("BGN", "лв"),
    ("UAH", "₴"),
    ("TRY", "₺"),
    ("ILS", "₪"),
    ("AED", "د.إ"),
    ("SAR", "﷼"),
    ("QAR", "﷼"),
    ("EGP", "E£"),
    ("ZAR", "R"),
    ("NGN", "₦"),
    ("KES", "KSh"),
    ("GHS", "₵"),
    ("MAD", "DH"),
    ("MXN", "MX$"),
    ("ARS", "AR$"),
    ("CLP", "CL$"),
    ("COP", "CO$"),
    ("PEN", "S/"),
    ("UYU", "$U"),
    ("SGD", "S$"),
    ("HKD", "HK$"),
    ("TWD", "NT$"),
    ("THB", "฿"),
    ("VND", "₫"),
    ("IDR", "Rp"),
    ("MYR", "RM"),
    ("PHP", "₱"),
    ("PKR", "₨"),
    ("BDT", "৳"),
    ("LKR", "Rs"),
    ("NPR", "Rs"),
    ("BTC", "₿"),
    ("ETH", "Ξ"),
];

/// The display symbol for an ISO 4217 code, if we know it.
pub fn symbol_for_code(code: &str) -> Option<&'static str> {
    let up = code.trim().to_ascii_uppercase();
    CURRENCIES.iter().find(|(c, _)| *c == up).map(|(_, s)| *s)
}

// ---------------------------------------------------------------------------
// Option parsing — every fixed-choice option is validated, never silently
// coerced, so a typo in chat/CLI comes back as an actionable message.
// ---------------------------------------------------------------------------

fn parse_mode(s: &str) -> Result<RoundMode, String> {
    Ok(match s.trim() {
        "" | "half_up" => RoundMode::HalfUp,
        "half_down" => RoundMode::HalfDown,
        "half_even" => RoundMode::HalfEven,
        "ceil" => RoundMode::Ceil,
        "floor" => RoundMode::Floor,
        "truncate" => RoundMode::Truncate,
        other => {
            return Err(format!(
                "rounding must be one of half_up/half_down/half_even/ceil/floor/truncate, got '{other}'"
            ))
        }
    })
}

fn parse_position(s: &str) -> Result<Position, String> {
    Ok(match s.trim() {
        "" | "before" => Position::Before,
        "after" => Position::After,
        other => {
            return Err(format!(
                "position must be 'before' or 'after', got '{other}'"
            ))
        }
    })
}

fn parse_symbol_style(s: &str) -> Result<SymbolStyle, String> {
    Ok(match s.trim() {
        "" | "symbol" => SymbolStyle::Symbol,
        "code" => SymbolStyle::Code,
        "none" => SymbolStyle::None,
        other => {
            return Err(format!(
                "symbol_style must be one of symbol/code/none, got '{other}'"
            ))
        }
    })
}

fn parse_sign_style(s: &str) -> Result<SignStyle, String> {
    Ok(match s.trim() {
        "" | "auto" => SignStyle::Auto,
        "always" => SignStyle::Always,
        "except_zero" => SignStyle::ExceptZero,
        "never" => SignStyle::Never,
        "space" => SignStyle::Space,
        other => {
            return Err(format!(
                "sign_style must be one of auto/always/except_zero/never/space, got '{other}'"
            ))
        }
    })
}

fn parse_digit_grouping(s: &str) -> Result<DigitGrouping, String> {
    Ok(match s.trim() {
        "" | "western" => DigitGrouping::Western,
        "indian" => DigitGrouping::Indian,
        other => {
            return Err(format!(
                "digit_grouping must be 'western' or 'indian', got '{other}'"
            ))
        }
    })
}

fn group_sep_char(s: &str) -> Result<&'static str, String> {
    Ok(match s.trim() {
        "" | "comma" => ",",
        "period" => ".",
        "space" => " ",
        // U+202F narrow no-break space — the separator French/Nordic typography uses.
        "narrow_space" => "\u{202f}",
        "apostrophe" => "'",
        "underscore" => "_",
        "none" => "",
        other => {
            return Err(format!(
                "group_separator must be one of comma/period/space/narrow_space/apostrophe/underscore/none, got '{other}'"
            ))
        }
    })
}

fn decimal_sep_char(s: &str) -> Result<&'static str, String> {
    Ok(match s.trim() {
        "" | "period" => ".",
        "comma" => ",",
        other => {
            return Err(format!(
                "decimal_separator must be 'period' or 'comma', got '{other}'"
            ))
        }
    })
}

// ---------------------------------------------------------------------------
// Input parsing — deliberately forgiving about how the number was written
// (pasted from a spreadsheet, a European locale, or an accounting report) and
// strict about anything that isn't a number.
// ---------------------------------------------------------------------------

/// Characters that only ever separate groups of digits, never carry meaning.
fn is_group_noise(c: char) -> bool {
    matches!(
        c,
        ' ' | '\t' | '_' | '\'' | '\u{a0}' | '\u{202f}' | '\u{2009}' | '\u{2007}' | '\u{2019}'
    )
}

/// True for a character that can only be part of a currency affix (`$`, `€`,
/// `US`, `kr`) — i.e. not a digit and not a decimal/group mark.
fn is_affix_char(c: char) -> bool {
    !c.is_ascii_digit() && c != '.' && c != ',' && !is_group_noise(c)
}

/// Trim a leading/trailing currency affix of at most `MAX_AFFIX` characters so
/// `$1,234.50`, `1 234,50 €` and `EUR 12` all parse. Longer runs of junk are
/// left in place and rejected by the digit check below.
fn strip_affixes(s: &str) -> &str {
    const MAX_AFFIX: usize = 5;
    let mut out = s.trim();
    let lead = out.chars().take_while(|c| is_affix_char(*c)).count();
    if lead > 0 && lead <= MAX_AFFIX {
        out = out[out
            .char_indices()
            .nth(lead)
            .map(|(i, _)| i)
            .unwrap_or(out.len())..]
            .trim();
    }
    let trail = out.chars().rev().take_while(|c| is_affix_char(*c)).count();
    if trail > 0 && trail <= MAX_AFFIX {
        let keep = out.chars().count() - trail;
        out = out[..out
            .char_indices()
            .nth(keep)
            .map(|(i, _)| i)
            .unwrap_or(out.len())]
            .trim();
    }
    out
}

/// Normalize whichever of `.` / `,` acted as the decimal mark to a single `.`
/// and drop the group marks.
///
/// Rules (documented on the page, because they resolve real ambiguity):
/// * both present → the LAST one is the decimal mark, the other groups digits;
/// * several commas and no dot → all commas group digits (`1,234,567`);
/// * one comma and no dot → it groups digits only when exactly three digits
///   follow it (`1,234`), otherwise it is a decimal comma (`0,5` → `0.5`);
/// * several dots and no comma → all dots group digits (`1.234.567`);
/// * one dot → always a decimal point.
fn normalize_separators(s: &str) -> Result<String, String> {
    let dots = s.matches('.').count();
    let commas = s.matches(',').count();

    let decimal_is_comma = if dots > 0 && commas > 0 {
        s.rfind(',') > s.rfind('.')
    } else if commas == 1 && dots == 0 {
        let after = s.split(',').nth(1).unwrap_or("");
        let before = s.split(',').next().unwrap_or("");
        !(after.chars().count() == 3 && before.chars().any(|c| c.is_ascii_digit()))
    } else {
        false
    };

    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            ',' if decimal_is_comma => out.push('.'),
            '.' if !decimal_is_comma && dots > 0 && commas == 0 && dots > 1 => {}
            '.' if decimal_is_comma => {}
            ',' => {}
            other => out.push(other),
        }
    }
    if out.matches('.').count() > 1 {
        return Err(format!("'{s}' has more than one decimal point"));
    }
    Ok(out)
}

/// Expand a float into a plain decimal digit string (used only for
/// scientific-notation input, which has no exact digit string to round).
fn expand_float(v: f64) -> Result<(String, String), String> {
    if !v.is_finite() {
        return Err("the value must be a finite number".into());
    }
    let s = format!("{:.*}", 12, v.abs());
    let (i, f) = s.split_once('.').unwrap_or((s.as_str(), ""));
    Ok((i.to_string(), f.to_string()))
}

/// Parse a human-written number into `(negative, integer_digits, fraction_digits)`.
pub fn parse_number(raw: &str) -> Result<(bool, String, String), String> {
    let mut s = raw.trim();
    if s.is_empty() {
        return Err("enter a number to format".into());
    }
    let mut neg = false;

    // Accounting parentheses: (1,234.50) is -1234.50.
    if s.starts_with('(') && s.ends_with(')') && s.len() >= 2 {
        neg = true;
        s = s[1..s.len() - 1].trim();
    }
    // Leading and trailing signs (a trailing '-' is how some ledgers write it).
    loop {
        let next = s
            .strip_prefix('-')
            .or_else(|| s.strip_prefix('\u{2212}'))
            .map(|r| {
                neg = !neg;
                r
            })
            .or_else(|| s.strip_prefix('+'));
        match next {
            Some(r) => s = r.trim_start(),
            None => break,
        }
    }
    if let Some(r) = s.strip_suffix('-') {
        neg = !neg;
        s = r.trim_end();
    }

    let s = strip_affixes(s);
    if s.is_empty() {
        return Err(format!("'{}' is not a number", raw.trim()));
    }
    let cleaned: String = s.chars().filter(|c| !is_group_noise(*c)).collect();

    // Scientific notation goes through f64 — there is no exact digit string.
    if cleaned.contains('e') || cleaned.contains('E') {
        let v: f64 = cleaned
            .replace(',', ".")
            .parse()
            .map_err(|_| format!("'{}' is not a number", raw.trim()))?;
        let (i, f) = expand_float(v)?;
        return Ok((neg, i, f));
    }

    let normalized = normalize_separators(&cleaned)?;
    let (int_part, frac_part) = match normalized.split_once('.') {
        Some((i, f)) => (i, f),
        None => (normalized.as_str(), ""),
    };
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
        || (int_part.is_empty() && frac_part.is_empty())
    {
        return Err(format!("'{}' is not a number", raw.trim()));
    }
    Ok((neg, int_part.to_string(), frac_part.to_string()))
}

// ---------------------------------------------------------------------------
// Digit-string rounding
// ---------------------------------------------------------------------------

fn inc_digits(s: &str) -> String {
    let mut digits: Vec<u8> = s.bytes().collect();
    if digits.is_empty() {
        return "1".to_string();
    }
    let mut i = digits.len();
    loop {
        if i == 0 {
            digits.insert(0, b'1');
            break;
        }
        i -= 1;
        if digits[i] == b'9' {
            digits[i] = b'0';
        } else {
            digits[i] += 1;
            break;
        }
    }
    String::from_utf8(digits).unwrap()
}

fn strip_leading_zeros(s: &str) -> String {
    let t = s.trim_start_matches('0');
    if t.is_empty() {
        "0".to_string()
    } else {
        t.to_string()
    }
}

/// Round `int_digits.frac_digits` to exactly `d` fraction digits, returning the
/// zero-padded `(integer, fraction)` pair. `neg` only matters for ceil/floor.
fn round_digits(
    neg: bool,
    int_digits: &str,
    frac_digits: &str,
    d: usize,
    mode: RoundMode,
) -> (String, String) {
    if frac_digits.len() <= d {
        let mut frac = frac_digits.to_string();
        while frac.len() < d {
            frac.push('0');
        }
        return (strip_leading_zeros(int_digits), frac);
    }

    let kept = &frac_digits[..d];
    let dropped = &frac_digits[d..];
    let first = dropped.as_bytes()[0] - b'0';
    let rest_nonzero = dropped.as_bytes()[1..].iter().any(|&b| b != b'0');
    let any_dropped = first != 0 || rest_nonzero;

    let mut keep_digits = String::with_capacity(int_digits.len() + d);
    keep_digits.push_str(int_digits);
    keep_digits.push_str(kept);
    let last_kept_odd = keep_digits
        .bytes()
        .last()
        .map(|b| (b - b'0') % 2 == 1)
        .unwrap_or(false);

    let away = match mode {
        RoundMode::HalfUp => first >= 5,
        RoundMode::HalfDown => first > 5 || (first == 5 && rest_nonzero),
        RoundMode::HalfEven => match first.cmp(&5) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => rest_nonzero || last_kept_odd,
        },
        RoundMode::Ceil => !neg && any_dropped,
        RoundMode::Floor => neg && any_dropped,
        RoundMode::Truncate => false,
    };
    if away {
        keep_digits = inc_digits(&keep_digits);
    }

    if d == 0 {
        return (strip_leading_zeros(&keep_digits), String::new());
    }
    if keep_digits.len() <= d {
        let padded = "0".repeat(d + 1 - keep_digits.len()) + &keep_digits;
        let split = padded.len() - d;
        return (padded[..split].to_string(), padded[split..].to_string());
    }
    let split = keep_digits.len() - d;
    (
        strip_leading_zeros(&keep_digits[..split]),
        keep_digits[split..].to_string(),
    )
}

// ---------------------------------------------------------------------------
// Grouping
// ---------------------------------------------------------------------------

fn group_integer(digits: &str, sep: &str, style: DigitGrouping) -> String {
    if sep.is_empty() || digits.len() <= 3 {
        return digits.to_string();
    }
    let bytes = digits.as_bytes();
    let mut chunks: Vec<&str> = Vec::new();
    let mut end = bytes.len();
    // The rightmost group is always three digits; after that Western keeps
    // taking three, Indian switches to two (lakh / crore).
    let mut size = 3usize;
    while end > 0 {
        let start = end.saturating_sub(size);
        chunks.push(&digits[start..end]);
        end = start;
        if style == DigitGrouping::Indian {
            size = 2;
        }
    }
    chunks.reverse();
    chunks.join(sep)
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Format a raw number as a currency string. Presentation only — no exchange
/// rates, no network.
///
/// * `value` — the number, however it was written (`1234.5`, `1,234.50`,
///   `1 234,50`, `$1,234.50`, `(1234.5)`, `1.5e3`).
/// * `currency` — an ISO 4217 code (`USD`, `EUR`, `JPY`) or any literal
///   symbol/text (`$`, `Kč`, `Bs.`).
/// * `symbol_style` — `symbol` | `code` | `none`.
/// * `position` — `before` | `after` the digits.
/// * `symbol_space` — put a space between the currency and the digits.
/// * `decimals` — fraction digits to keep, 0–8.
/// * `rounding` — `half_up` | `half_down` | `half_even` | `ceil` | `floor` | `truncate`.
/// * `grouping` — group the integer digits at all.
/// * `digit_grouping` — `western` (1,234,567) | `indian` (12,34,567).
/// * `group_separator` — `comma` | `period` | `space` | `narrow_space` | `apostrophe` | `underscore` | `none`.
/// * `decimal_separator` — `period` | `comma`.
/// * `sign_style` — `auto` | `always` | `except_zero` | `never` | `space`.
/// * `accounting` — wrap negatives in parentheses instead of showing a minus.
/// * `trim_zeros` — drop trailing fraction zeros (and a then-empty separator).
#[allow(clippy::too_many_arguments)]
pub fn format_currency(
    value: &str,
    currency: &str,
    symbol_style: &str,
    position: &str,
    symbol_space: bool,
    decimals: i64,
    rounding: &str,
    grouping: bool,
    digit_grouping: &str,
    group_separator: &str,
    decimal_separator: &str,
    sign_style: &str,
    accounting: bool,
    trim_zeros: bool,
) -> Result<String, String> {
    if !(0..=8).contains(&decimals) {
        return Err(format!("decimals must be between 0 and 8, got {decimals}"));
    }
    let d = decimals as usize;
    let style = parse_symbol_style(symbol_style)?;
    let position = parse_position(position)?;
    let mode = parse_mode(rounding)?;
    let grouping_style = parse_digit_grouping(digit_grouping)?;
    // Validate the separator even when grouping is off, so a typo is reported
    // instead of silently ignored.
    let requested_group_sep = group_sep_char(group_separator)?;
    let group_sep = if grouping { requested_group_sep } else { "" };
    let dec_sep = decimal_sep_char(decimal_separator)?;
    let sign_style = parse_sign_style(sign_style)?;

    if !group_sep.is_empty() && group_sep == dec_sep {
        return Err(format!(
            "the group separator and the decimal separator must differ (both are '{dec_sep}')"
        ));
    }

    let currency = currency.trim();
    let money = match style {
        SymbolStyle::None => String::new(),
        SymbolStyle::Symbol => {
            if currency.is_empty() {
                return Err("currency is required unless symbol_style is 'none'".into());
            }
            symbol_for_code(currency)
                .map(|s| s.to_string())
                .unwrap_or_else(|| currency.to_string())
        }
        SymbolStyle::Code => {
            if currency.is_empty() {
                return Err("currency is required unless symbol_style is 'none'".into());
            }
            if symbol_for_code(currency).is_some() {
                currency.to_ascii_uppercase()
            } else {
                currency.to_string()
            }
        }
    };

    let (neg, int_digits, frac_digits) = parse_number(value)?;
    let (int_rounded, mut frac) = round_digits(neg, &int_digits, &frac_digits, d, mode);

    if trim_zeros {
        while frac.ends_with('0') {
            frac.pop();
        }
    }

    let is_zero = int_rounded.bytes().all(|b| b == b'0') && frac.bytes().all(|b| b == b'0');
    // -0 is never a real amount: a value that rounds to zero prints unsigned.
    let neg = neg && !is_zero && sign_style != SignStyle::Never;

    let grouped = group_integer(&int_rounded, group_sep, grouping_style);
    let mut digits = grouped;
    if !frac.is_empty() {
        digits.push_str(dec_sep);
        digits.push_str(&frac);
    }

    let gap = if symbol_space && !money.is_empty() {
        " "
    } else {
        ""
    };
    let body = match position {
        Position::Before => format!("{money}{gap}{digits}"),
        Position::After => format!("{digits}{gap}{money}"),
    };

    if accounting && neg {
        return Ok(format!("({body})"));
    }
    let sign = if neg {
        "-"
    } else {
        match sign_style {
            SignStyle::Always => "+",
            SignStyle::ExceptZero if !is_zero => "+",
            SignStyle::Space => " ",
            _ => "",
        }
    };
    Ok(format!("{sign}{body}"))
}

/// Default entry point: USD, symbol before, 2 decimals, half-up, western comma
/// grouping, decimal point, normal minus signs.
pub fn run(input: &str) -> Result<String, String> {
    format_currency(
        input, "USD", "symbol", "before", false, 2, "half_up", true, "western", "comma", "period",
        "auto", false, false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defaults every surface applies: USD, symbol before, 2 places,
    /// half-up, western grouping with a comma, a decimal point, minus signs.
    fn fmt(value: &str) -> String {
        format_currency(
            value, "USD", "symbol", "before", false, 2, "half_up", true, "western", "comma",
            "period", "auto", false, false,
        )
        .unwrap()
    }

    #[test]
    fn formats_a_plain_number_with_the_defaults() {
        assert_eq!(fmt("1234.5"), "$1,234.50");
        assert_eq!(fmt("0"), "$0.00");
        assert_eq!(fmt("-7"), "-$7.00");
        assert_eq!(fmt("999999.999"), "$1,000,000.00");
    }

    #[test]
    fn parses_messy_input_forms() {
        assert_eq!(fmt(" 1,234.5 "), "$1,234.50");
        assert_eq!(fmt("1 234.5"), "$1,234.50");
        assert_eq!(fmt("1_234.5"), "$1,234.50");
        assert_eq!(fmt("1'234.5"), "$1,234.50");
        assert_eq!(fmt("$1,234.50"), "$1,234.50");
        assert_eq!(fmt("1.234,50"), "$1,234.50"); // European written form
        assert_eq!(fmt("1.234.567"), "$1,234,567.00"); // dots as group marks
        assert_eq!(fmt("0,5"), "$0.50"); // lone comma, 1 digit → decimal comma
        assert_eq!(fmt("1,234"), "$1,234.00"); // lone comma, 3 digits → grouping
        assert_eq!(fmt("(1234.5)"), "-$1,234.50"); // accounting input
        assert_eq!(fmt("1234.5-"), "-$1,234.50"); // trailing-minus ledger form
        assert_eq!(fmt("\u{2212}12"), "-$12.00"); // Unicode minus
        assert_eq!(fmt("1.5e3"), "$1,500.00"); // scientific notation
        assert_eq!(fmt(".5"), "$0.50");
    }

    #[test]
    fn rejects_things_that_are_not_numbers() {
        assert!(format_currency(
            "", "USD", "symbol", "before", false, 2, "half_up", true, "western", "comma", "period",
            "auto", false, false
        )
        .is_err());
        for bad in [
            "abc",
            "12abc34",
            "1.2,3.4",
            "--",
            "1,2,3.4.5",
            "twelve dollars",
        ] {
            assert!(fmt_err(bad).is_err(), "{bad} should be rejected");
        }
    }

    fn fmt_err(value: &str) -> Result<String, String> {
        format_currency(
            value, "USD", "symbol", "before", false, 2, "half_up", true, "western", "comma",
            "period", "auto", false, false,
        )
    }

    #[test]
    fn rejects_invalid_options() {
        let bad_decimals = format_currency(
            "1", "USD", "symbol", "before", false, 9, "half_up", true, "western", "comma",
            "period", "auto", false, false,
        );
        assert!(bad_decimals.unwrap_err().contains("between 0 and 8"));

        let bad_mode = format_currency(
            "1", "USD", "symbol", "before", false, 2, "nearest", true, "western", "comma",
            "period", "auto", false, false,
        );
        assert!(bad_mode.unwrap_err().contains("rounding must be"));

        let clash = format_currency(
            "1", "USD", "symbol", "before", false, 2, "half_up", true, "western", "comma", "comma",
            "auto", false, false,
        );
        assert!(clash.unwrap_err().contains("must differ"));

        let no_currency = format_currency(
            "1", "", "symbol", "before", false, 2, "half_up", true, "western", "comma", "period",
            "auto", false, false,
        );
        assert!(no_currency.unwrap_err().contains("currency is required"));
    }

    #[test]
    fn honours_symbol_style_position_and_spacing() {
        let euro = |style: &str, pos: &str, space: bool| {
            format_currency(
                "1234.5", "EUR", style, pos, space, 2, "half_up", true, "western", "period",
                "comma", "auto", false, false,
            )
            .unwrap()
        };
        assert_eq!(euro("symbol", "after", true), "1.234,50 €");
        assert_eq!(euro("code", "after", true), "1.234,50 EUR");
        assert_eq!(euro("none", "before", true), "1.234,50");
        assert_eq!(euro("symbol", "before", false), "€1.234,50");
        // An unknown currency is passed through verbatim, both styles.
        let literal = format_currency(
            "9.5", "Kč", "code", "after", true, 2, "half_up", true, "western", "space", "comma",
            "auto", false, false,
        )
        .unwrap();
        assert_eq!(literal, "9,50 Kč");
    }

    #[test]
    fn supports_every_separator_and_grouping_choice() {
        let g = |sep: &str, grouping: bool, style: &str| {
            format_currency(
                "12345678.9",
                "USD",
                "none",
                "before",
                false,
                2,
                "half_up",
                grouping,
                style,
                sep,
                "period",
                "auto",
                false,
                false,
            )
            .unwrap()
        };
        assert_eq!(g("comma", true, "western"), "12,345,678.90");
        assert_eq!(g("space", true, "western"), "12 345 678.90");
        assert_eq!(
            g("narrow_space", true, "western"),
            "12\u{202f}345\u{202f}678.90"
        );
        assert_eq!(g("apostrophe", true, "western"), "12'345'678.90");
        assert_eq!(g("underscore", true, "western"), "12_345_678.90");
        assert_eq!(g("none", true, "western"), "12345678.90");
        assert_eq!(g("comma", false, "western"), "12345678.90");
        assert_eq!(g("comma", true, "indian"), "1,23,45,678.90");
    }

    #[test]
    fn indian_grouping_matches_the_lakh_crore_pattern() {
        let ind = |v: &str| {
            format_currency(
                v, "INR", "symbol", "before", false, 0, "half_up", true, "indian", "comma",
                "period", "auto", false, false,
            )
            .unwrap()
        };
        assert_eq!(ind("1234"), "₹1,234");
        assert_eq!(ind("123456"), "₹1,23,456");
        assert_eq!(ind("10000000"), "₹1,00,00,000");
    }

    #[test]
    fn rounds_on_the_digit_string_not_the_float() {
        // 1.005 * 100.0 == 100.49999999999999 — a naive round returns 1.00.
        let r = |v: &str, mode: &str, d: i64| {
            format_currency(
                v, "USD", "none", "before", false, d, mode, false, "western", "comma", "period",
                "auto", false, false,
            )
            .unwrap()
        };
        assert_eq!(r("1.005", "half_up", 2), "1.01");
        assert_eq!(r("2.675", "half_up", 2), "2.68");
        assert_eq!(r("2.5", "half_down", 0), "2");
        assert_eq!(r("2.5", "half_even", 0), "2");
        assert_eq!(r("3.5", "half_even", 0), "4");
        assert_eq!(r("2.001", "ceil", 2), "2.01");
        assert_eq!(r("-2.001", "ceil", 2), "-2.00");
        assert_eq!(r("2.009", "floor", 2), "2.00");
        assert_eq!(r("-2.001", "floor", 2), "-2.01");
        assert_eq!(r("2.999", "truncate", 2), "2.99");
        assert_eq!(r("-2.999", "truncate", 0), "-2");
    }

    #[test]
    fn supports_zero_through_eight_decimals() {
        for d in 0..=8 {
            let out = format_currency(
                "1.123456789",
                "USD",
                "none",
                "before",
                false,
                d,
                "truncate",
                false,
                "western",
                "comma",
                "period",
                "auto",
                false,
                false,
            )
            .unwrap();
            let want_frac = &"123456789"[..d as usize];
            let want = if d == 0 {
                "1".to_string()
            } else {
                format!("1.{want_frac}")
            };
            assert_eq!(out, want, "at {d} decimals");
        }
    }

    #[test]
    fn sign_styles_cover_positive_zero_and_negative() {
        let s = |v: &str, style: &str| {
            format_currency(
                v, "USD", "symbol", "before", false, 2, "half_up", true, "western", "comma",
                "period", style, false, false,
            )
            .unwrap()
        };
        assert_eq!(s("5", "auto"), "$5.00");
        assert_eq!(s("-5", "auto"), "-$5.00");
        assert_eq!(s("5", "always"), "+$5.00");
        assert_eq!(s("0", "always"), "+$0.00");
        assert_eq!(s("5", "except_zero"), "+$5.00");
        assert_eq!(s("0", "except_zero"), "$0.00");
        assert_eq!(s("-5", "never"), "$5.00");
        assert_eq!(s("5", "space"), " $5.00");
        assert_eq!(s("-5", "space"), "-$5.00");
        // A negative that rounds away never prints as "-$0.00".
        assert_eq!(s("-0.001", "auto"), "$0.00");
    }

    #[test]
    fn accounting_parentheses_replace_the_minus_sign() {
        let a = |v: &str, sign: &str| {
            format_currency(
                v, "USD", "symbol", "before", false, 2, "half_up", true, "western", "comma",
                "period", sign, true, false,
            )
            .unwrap()
        };
        assert_eq!(a("-1234.5", "auto"), "($1,234.50)");
        assert_eq!(a("1234.5", "auto"), "$1,234.50");
        assert_eq!(a("0", "auto"), "$0.00");
        // sign_style=never means "absolute value", so there is nothing to bracket.
        assert_eq!(a("-1234.5", "never"), "$1,234.50");
    }

    #[test]
    fn trim_zeros_drops_the_fraction_when_it_is_empty() {
        let t = |v: &str, d: i64| {
            format_currency(
                v, "USD", "symbol", "before", false, d, "half_up", true, "western", "comma",
                "period", "auto", false, true,
            )
            .unwrap()
        };
        assert_eq!(t("1234.5", 2), "$1,234.5");
        assert_eq!(t("1234", 2), "$1,234");
        assert_eq!(t("1234.567", 2), "$1,234.57");
        assert_eq!(t("0", 4), "$0");
    }

    #[test]
    fn known_codes_resolve_to_a_symbol() {
        assert_eq!(symbol_for_code("usd"), Some("$"));
        assert_eq!(symbol_for_code("JPY"), Some("¥"));
        assert_eq!(symbol_for_code("ZZZ"), None);
    }
}
