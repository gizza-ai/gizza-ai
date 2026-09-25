//! decimal-to-fraction core — turn a decimal into an exact or simplest-approximate fraction.
//!
//! The conversion path is exact integer/rational arithmetic (`i128`), never floating point:
//! the typed decimal is parsed into an exact rational, then either kept exact, snapped to a
//! fixed denominator (nearest 1/16, 1/100, …), or approximated with continued-fraction
//! convergents (denominator cap) / a Stern-Brocot simplest-in-interval search (tolerance).
//! `f64` appears only in the reported decimal/error fields.

use serde::Serialize;
use std::cmp::Ordering;

/// Fractional digits accepted (non-repeating + repeating). 10^18 still fits i64 for output.
const MAX_FRACTIONAL_DIGITS: usize = 18;
/// Digits accepted before the decimal point.
const MAX_INT_DIGITS: usize = 18;
/// Length of a repeating block.
const MAX_REPEATING_DIGITS: usize = 9;
/// Largest `e±N` exponent accepted.
const MAX_EXPONENT: i32 = 18;
/// Cap for both `max_denominator` and the fixed `denominator` snap.
const MAX_DENOMINATOR_CAP: i128 = 1_000_000_000_000;
/// Smallest non-zero tolerance accepted (kept in step with `TOLERANCE_DIGITS`).
const MIN_TOLERANCE: f64 = 1e-12;
/// Decimal places used to turn the `tolerance` f64 into an exact rational.
const TOLERANCE_DIGITS: usize = 12;
/// How many convergents are reported.
const MAX_CONVERGENTS: usize = 20;
/// Continued-fraction terms computed before giving up.
const MAX_CF_TERMS: usize = 48;

const OVERFLOW: &str =
    "this conversion needs numbers too large to compute exactly — use fewer digits, or a smaller denominator limit";

/// Raw tool inputs. Numeric fields are `Option<f64>` because every surface (chat JSON, CLI
/// key=value, page form field) may omit them; `None` means "not supplied".
#[derive(Debug, Default, Clone)]
pub struct Inputs {
    pub decimal: String,
    pub repeating_digits: Option<f64>,
    pub tolerance: Option<f64>,
    pub max_denominator: Option<f64>,
    pub denominator: Option<f64>,
    pub rounding: String,
    pub reduce: Option<bool>,
}

/// The full conversion result (serialized as the tool's JSON output).
#[derive(Debug, Clone, Serialize)]
pub struct Conversion {
    /// The decimal as typed, after whitespace/separator cleanup.
    pub input: String,
    /// The parsed decimal value.
    pub decimal: f64,
    /// Result fraction, e.g. `5/8`, `-5/8`, or `3` when it is a whole number.
    pub fraction: String,
    pub numerator: i64,
    pub denominator: i64,
    /// Mixed-number form, e.g. `2 1/2` (same as `fraction` when already proper).
    pub mixed_number: String,
    pub whole_part: i64,
    pub proper_numerator: i64,
    pub proper_denominator: i64,
    /// True when the fraction equals the decimal exactly.
    pub is_exact: bool,
    /// True when |numerator| < denominator.
    pub is_proper: bool,
    /// True when the result was reduced to lowest terms.
    pub reduced: bool,
    /// Decimal value of the result fraction.
    pub fraction_value: f64,
    /// `fraction_value - decimal` (0 when exact).
    pub error: f64,
    /// Error as a percentage of the decimal (0 when exact or the decimal is 0).
    pub error_percent: f64,
    /// Exact fraction for the digits as typed, in lowest terms.
    pub exact_fraction: String,
    /// True when the input was treated as a repeating decimal.
    pub repeating: bool,
    /// Length of the repeating block (0 when not repeating).
    pub repeating_digits: u32,
    /// `exact`, `fixed-denominator`, `tolerance` or `max-denominator`.
    pub method: String,
    /// Continued-fraction convergents of the exact value, simplest first.
    pub convergents: Vec<String>,
    /// Worked steps, in order.
    pub steps: Vec<String>,
    /// One-line plain-English result.
    pub summary: String,
}

/// Convert `inputs` and return the pretty-printed JSON result.
pub fn convert_json(inputs: &Inputs) -> Result<String, String> {
    let c = convert(inputs)?;
    serde_json::to_string_pretty(&c).map_err(|e| format!("could not serialize the result: {e}"))
}

/// Which way an approximation may move off the true value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Nearest,
    Up,
    Down,
}

fn parse_side(raw: &str) -> Result<Side, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "nearest" => Ok(Side::Nearest),
        "up" => Ok(Side::Up),
        "down" => Ok(Side::Down),
        other => Err(format!(
            "rounding must be nearest, up or down, got '{other}'"
        )),
    }
}

/// Convert `inputs` into a [`Conversion`].
pub fn convert(inputs: &Inputs) -> Result<Conversion, String> {
    let side = parse_side(&inputs.rounding)?;
    let repeating_digits = whole_arg(
        inputs.repeating_digits,
        "repeating_digits",
        0,
        MAX_REPEATING_DIGITS as i128,
    )?;
    let max_denominator = whole_arg(
        inputs.max_denominator,
        "max_denominator",
        0,
        MAX_DENOMINATOR_CAP,
    )?;
    let fixed_denominator = whole_arg(inputs.denominator, "denominator", 0, MAX_DENOMINATOR_CAP)?;
    let reduce = inputs.reduce.unwrap_or(true);
    let tolerance = match inputs.tolerance {
        None => 0.0,
        Some(t) if !t.is_finite() => return Err("tolerance must be a finite number".into()),
        Some(t) if t < 0.0 => return Err("tolerance cannot be negative".into()),
        Some(t) if t >= 1.0 => {
            return Err("tolerance must be smaller than 1 — try 0.001 or 0.000001".into())
        }
        Some(t) if t > 0.0 && t < MIN_TOLERANCE => {
            return Err(format!(
                "tolerance must be 0 (exact) or at least {MIN_TOLERANCE:e}"
            ))
        }
        Some(t) => t,
    };

    let parsed = parse_decimal(&inputs.decimal, repeating_digits as usize)?;
    let exact = parsed.value;
    let mut steps = parsed.steps.clone();

    let terms = cf_terms(abs_rat(exact))?;
    let convergent_pairs = convergents_from(&terms)?;
    let convergents = convergent_pairs
        .iter()
        .take(MAX_CONVERGENTS)
        .map(|&(p, q)| {
            let signed = if exact.n < 0 { -p } else { p };
            format!("{signed}/{q}")
        })
        .collect::<Vec<_>>();

    // Pick the result: fixed denominator > tolerance > denominator cap > exact.
    let (mut res_n, mut res_d, method) = if fixed_denominator > 0 {
        let snapped = snap_to_denominator(exact, fixed_denominator, side)?;
        steps.push(format!(
            "Snap to the nearest 1/{fixed_denominator}: {} × {fixed_denominator} = {}, rounded {} to {} → {}/{fixed_denominator}.",
            trim_float(exact.to_f64()),
            trim_float(exact.to_f64() * fixed_denominator as f64),
            match side {
                Side::Nearest => "to the nearest whole number",
                Side::Up => "up",
                Side::Down => "down",
            },
            snapped,
            snapped
        ));
        (snapped, fixed_denominator, "fixed-denominator".to_string())
    } else if tolerance > 0.0 {
        let tol = f64_to_rat(tolerance)?;
        let simplest = simplest_within(exact, tol, side, 0)?;
        if max_denominator > 0 && simplest.d > max_denominator {
            let capped = best_within_denominator(exact, max_denominator, side)?;
            steps.push(format!(
                "No fraction within ±{} has a denominator of {max_denominator} or less (the simplest one is {}), so the closest fraction under the cap is used instead.",
                trim_float(tolerance),
                fmt_rat(simplest)
            ));
            steps.push(format!(
                "Closest fraction with a denominator of {max_denominator} or less: {}.",
                fmt_rat(capped)
            ));
            (capped.n, capped.d, "max-denominator".to_string())
        } else {
            steps.push(format!(
                "Simplest fraction within ±{} of {}: {}.",
                trim_float(tolerance),
                trim_float(exact.to_f64()),
                fmt_rat(simplest)
            ));
            (simplest.n, simplest.d, "tolerance".to_string())
        }
    } else if max_denominator > 0 {
        let capped = best_within_denominator(exact, max_denominator, side)?;
        if !terms.is_empty() {
            steps.push(format!(
                "Continued-fraction terms of {}: [{}].",
                trim_float(exact.to_f64()),
                cf_display(&terms)
            ));
        }
        if !convergents.is_empty() {
            steps.push(format!("Convergents: {}.", convergents.join(", ")));
        }
        steps.push(format!(
            "Simplest convergent with a denominator of {max_denominator} or less: {}.",
            fmt_rat(capped)
        ));
        (capped.n, capped.d, "max-denominator".to_string())
    } else {
        steps.push(format!(
            "No approximation requested, so the exact value {} is the answer.",
            fmt_rat(exact)
        ));
        (parsed.raw_n, parsed.raw_d, "exact".to_string())
    };

    // Reduce unless the caller explicitly asked to keep the denominator as-is.
    let unreduced = (res_n, res_d);
    if reduce {
        let g = gcd(res_n, res_d);
        if g > 1 {
            steps.push(format!(
                "Divide both parts by their greatest common divisor {g}: {}/{} → {}/{}.",
                unreduced.0,
                unreduced.1,
                res_n / g,
                res_d / g
            ));
            res_n /= g;
            res_d /= g;
        }
    }
    let result = Rat::new(res_n, res_d)?;
    let reduced_flag = gcd(res_n, res_d) == 1;

    let numerator = to_i64(res_n, "numerator")?;
    let denominator = to_i64(res_d, "denominator")?;
    let fraction = fmt_pair(res_n, res_d);
    let whole = res_n / res_d;
    let proper_num = (res_n % res_d).abs();
    let mixed_number = if proper_num == 0 {
        format!("{whole}")
    } else if whole == 0 {
        fmt_pair(res_n, res_d)
    } else {
        format!("{whole} {proper_num}/{res_d}")
    };
    if whole != 0 && proper_num != 0 {
        steps.push(format!(
            "As a mixed number: {}/{} = {}.",
            res_n, res_d, mixed_number
        ));
    }

    let diff = result.sub(exact)?;
    let is_exact = diff.n == 0;
    let value_f = exact.to_f64();
    let result_f = result.to_f64();
    let error = diff.to_f64();
    let error_percent = if value_f == 0.0 {
        0.0
    } else {
        error / value_f * 100.0
    };
    let summary = if is_exact {
        format!("{} = {} exactly.", parsed.display, fraction)
    } else {
        format!(
            "{} ≈ {} ({} = {}, off by {}).",
            parsed.display,
            fraction,
            fraction,
            trim_float(result_f),
            trim_float(error.abs())
        )
    };

    Ok(Conversion {
        input: parsed.display,
        decimal: value_f,
        fraction,
        numerator,
        denominator,
        mixed_number,
        whole_part: to_i64(whole, "whole part")?,
        proper_numerator: to_i64(proper_num, "numerator")?,
        proper_denominator: denominator,
        is_exact,
        is_proper: res_n.abs() < res_d,
        reduced: reduced_flag,
        fraction_value: result_f,
        error,
        error_percent,
        exact_fraction: fmt_rat(exact),
        repeating: parsed.repeating_len > 0,
        repeating_digits: parsed.repeating_len as u32,
        method,
        convergents,
        steps,
        summary,
    })
}

// ---------------------------------------------------------------------------
// input parsing
// ---------------------------------------------------------------------------

struct Parsed {
    /// Exact rational value of the typed digits, in lowest terms.
    value: Rat,
    /// The same value before reduction (`625/1000` for `0.625`) — used by `reduce = false`.
    raw_n: i128,
    raw_d: i128,
    /// Cleaned input for display/summary.
    display: String,
    repeating_len: usize,
    steps: Vec<String>,
}

/// Parse a decimal string into an exact rational.
///
/// Accepts an optional sign (`-`, `+`, unicode minus), digit-group separators (`,`, `_`,
/// spaces), a trailing `%`, scientific notation (`1.25e-3`), and repeating-digit notation
/// (`0.1(6)`, `0.1[6]`, `0.16...`). `repeating_digits` marks that many trailing fractional
/// digits as repeating when the input carries no notation of its own.
fn parse_decimal(raw: &str, repeating_digits: usize) -> Result<Parsed, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("enter a decimal number to convert, for example 0.625".into());
    }
    let mut s: String = trimmed
        .chars()
        .filter(|c| !matches!(c, ',' | '_' | ' ' | '\u{00a0}' | '\u{2009}' | '\''))
        .map(|c| match c {
            '\u{2212}' | '\u{2013}' | '\u{2014}' => '-',
            '\u{ff0e}' => '.',
            '\u{2026}' => '~', // ellipsis → repeat marker, normalized below
            other => other,
        })
        .collect();

    let mut percent = false;
    if let Some(rest) = s.strip_suffix('%') {
        percent = true;
        s = rest.to_string();
    }

    let mut negative = false;
    if let Some(rest) = s.strip_prefix('-') {
        negative = true;
        s = rest.to_string();
    } else if let Some(rest) = s.strip_prefix('+') {
        s = rest.to_string();
    }

    // Repeating-digit notation: 0.1(6), 0.1[6], 0.16... / 0.16~
    let mut notation_repeat: Option<String> = None;
    let mut ellipsis = false;
    if let Some(open) = s.find(['(', '[']) {
        let close = if s.as_bytes()[open] == b'(' { ')' } else { ']' };
        let end = s
            .find(close)
            .ok_or_else(|| format!("unclosed repeating-digit group in '{trimmed}'"))?;
        if end < open {
            return Err(format!("unclosed repeating-digit group in '{trimmed}'"));
        }
        let inner = s[open + 1..end].to_string();
        let tail = s[end + 1..].trim_end_matches(['.', '~']).to_string();
        if !tail.is_empty() {
            return Err(format!(
                "the repeating digits must come last, as in 0.1(6) — got '{trimmed}'"
            ));
        }
        if inner.is_empty() || !inner.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!(
                "the repeating group must contain only digits, as in 0.1(6) — got '{trimmed}'"
            ));
        }
        notation_repeat = Some(inner);
        s = s[..open].to_string();
    } else {
        let before = s.len();
        s = s.trim_end_matches('~').to_string();
        if s.len() != before {
            ellipsis = true;
        }
        let before = s.len();
        s = s.trim_end_matches("...").to_string();
        if s.len() != before {
            ellipsis = true;
        }
    }

    // Scientific notation.
    let mut exponent: i32 = 0;
    if let Some(pos) = s.find(['e', 'E']) {
        if notation_repeat.is_some() || ellipsis {
            return Err(
                "repeating digits and scientific notation cannot be combined — write the digits out instead"
                    .into(),
            );
        }
        let exp_str = &s[pos + 1..];
        exponent = exp_str.parse::<i32>().map_err(|_| {
            format!("'{exp_str}' is not a valid exponent — try a form like 1.25e-3")
        })?;
        if exponent.abs() > MAX_EXPONENT {
            return Err(format!(
                "the exponent must be between -{MAX_EXPONENT} and {MAX_EXPONENT}"
            ));
        }
        s = s[..pos].to_string();
    }

    // Split integer / fractional digits.
    let (int_str, frac_str) = match s.split_once('.') {
        Some((i, f)) => (i.to_string(), f.to_string()),
        None => (s.clone(), String::new()),
    };
    if s.contains('.') && s.matches('.').count() > 1 {
        return Err(format!("'{trimmed}' has more than one decimal point"));
    }
    let int_digits = if int_str.is_empty() {
        "0".to_string()
    } else {
        int_str
    };
    for (label, digits) in [("digits", &int_digits), ("decimal places", &frac_str)] {
        if !digits.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!(
                "'{trimmed}' is not a decimal number — the {label} must be 0-9 (a leading sign, %, e-notation and 0.1(6) repeat notation are fine)"
            ));
        }
    }
    if int_digits.is_empty() && frac_str.is_empty() {
        return Err(format!("'{trimmed}' has no digits to convert"));
    }
    if int_digits.len() > MAX_INT_DIGITS {
        return Err(format!(
            "at most {MAX_INT_DIGITS} digits before the decimal point are supported"
        ));
    }

    // Decide the repeating block.
    let (non_repeat, repeat) = if let Some(block) = notation_repeat {
        if block.len() > MAX_REPEATING_DIGITS {
            return Err(format!(
                "the repeating block can be at most {MAX_REPEATING_DIGITS} digits long"
            ));
        }
        (frac_str.clone(), block)
    } else if repeating_digits > 0 {
        if repeating_digits > frac_str.len() {
            return Err(format!(
                "repeating_digits is {repeating_digits} but '{trimmed}' has only {} decimal place(s)",
                frac_str.len()
            ));
        }
        let split = frac_str.len() - repeating_digits;
        (frac_str[..split].to_string(), frac_str[split..].to_string())
    } else if ellipsis && !frac_str.is_empty() {
        let split = frac_str.len() - 1;
        (frac_str[..split].to_string(), frac_str[split..].to_string())
    } else {
        (frac_str.clone(), String::new())
    };
    if non_repeat.len() + repeat.len() > MAX_FRACTIONAL_DIGITS {
        return Err(format!(
            "at most {MAX_FRACTIONAL_DIGITS} decimal places are supported"
        ));
    }

    // Exact rational for the digits.
    let prefix: i128 = parse_digits(&format!("{int_digits}{non_repeat}"))?;
    let mut steps: Vec<String> = Vec::new();
    let (mut num, mut den) = if repeat.is_empty() {
        let den = pow10(non_repeat.len())?;
        if non_repeat.is_empty() {
            steps.push(format!(
                "{int_digits} is a whole number, so it is {int_digits}/1."
            ));
        } else {
            steps.push(format!(
                "{}.{} has {} decimal place(s), so write it over 10^{} = {}: {}/{}.",
                int_digits,
                non_repeat,
                non_repeat.len(),
                non_repeat.len(),
                den,
                prefix,
                den
            ));
        }
        (prefix, den)
    } else {
        let full = parse_digits(&format!("{int_digits}{non_repeat}{repeat}"))?;
        let big = pow10(non_repeat.len() + repeat.len())?;
        let small = pow10(non_repeat.len())?;
        let num = ck(full.checked_sub(prefix))?;
        let den = ck(big.checked_sub(small))?;
        steps.push(format!(
            "{}.{}({}) repeats {} digit(s) after {} fixed decimal place(s).",
            int_digits,
            non_repeat,
            repeat,
            repeat.len(),
            non_repeat.len()
        ));
        steps.push(format!(
            "Multiply by 10^{} and 10^{} and subtract to cancel the repeat: ({} - {}) / ({} - {}) = {}/{}.",
            non_repeat.len() + repeat.len(),
            non_repeat.len(),
            full,
            prefix,
            big,
            small,
            num,
            den
        ));
        (num, den)
    };

    if exponent > 0 {
        num = ck(num.checked_mul(pow10(exponent as usize)?))?;
        steps.push(format!(
            "The exponent e{exponent} shifts the decimal point {exponent} place(s) right: {num}/{den}."
        ));
    } else if exponent < 0 {
        den = ck(den.checked_mul(pow10((-exponent) as usize)?))?;
        steps.push(format!(
            "The exponent e{exponent} shifts the decimal point {} place(s) left: {num}/{den}.",
            -exponent
        ));
    }
    if percent {
        den = ck(den.checked_mul(100))?;
        steps.push(format!("A percentage is a value out of 100: {num}/{den}."));
    }
    if negative {
        num = ck(num.checked_neg())?;
    }

    let value = Rat::new(num, den)?;
    to_i64(num, "numerator")?;
    to_i64(den, "denominator")?;

    let mut display = String::new();
    if negative {
        display.push('-');
    }
    display.push_str(&int_digits);
    if !non_repeat.is_empty() || !repeat.is_empty() {
        display.push('.');
        display.push_str(&non_repeat);
        if !repeat.is_empty() {
            display.push('(');
            display.push_str(&repeat);
            display.push(')');
        }
    }
    if exponent != 0 {
        display.push_str(&format!("e{exponent}"));
    }
    if percent {
        display.push('%');
    }

    Ok(Parsed {
        value,
        raw_n: num,
        raw_d: den,
        display,
        repeating_len: repeat.len(),
        steps,
    })
}

fn parse_digits(s: &str) -> Result<i128, String> {
    if s.is_empty() {
        return Ok(0);
    }
    s.parse::<i128>().map_err(|_| OVERFLOW.to_string())
}

fn whole_arg(v: Option<f64>, name: &str, min: i128, max: i128) -> Result<i128, String> {
    let Some(v) = v else { return Ok(min.max(0)) };
    if !v.is_finite() {
        return Err(format!("{name} must be a finite number"));
    }
    if v.fract() != 0.0 {
        return Err(format!("{name} must be a whole number, got {v}"));
    }
    let v = v as i128;
    if v < min || v > max {
        return Err(format!("{name} must be between {min} and {max}, got {v}"));
    }
    Ok(v)
}

// ---------------------------------------------------------------------------
// rational arithmetic
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rat {
    n: i128,
    d: i128,
}

impl Rat {
    fn new(n: i128, d: i128) -> Result<Self, String> {
        if d == 0 {
            return Err("a fraction cannot have a denominator of 0".into());
        }
        let (mut n, mut d) = if d < 0 {
            (ck(n.checked_neg())?, ck(d.checked_neg())?)
        } else {
            (n, d)
        };
        let g = gcd(n, d);
        if g > 1 {
            n /= g;
            d /= g;
        }
        Ok(Rat { n, d })
    }

    fn int(v: i128) -> Self {
        Rat { n: v, d: 1 }
    }

    fn to_f64(self) -> f64 {
        self.n as f64 / self.d as f64
    }

    fn sub(self, o: Rat) -> Result<Rat, String> {
        let a = ck(self.n.checked_mul(o.d))?;
        let b = ck(o.n.checked_mul(self.d))?;
        Rat::new(ck(a.checked_sub(b))?, ck(self.d.checked_mul(o.d))?)
    }

    fn add(self, o: Rat) -> Result<Rat, String> {
        let a = ck(self.n.checked_mul(o.d))?;
        let b = ck(o.n.checked_mul(self.d))?;
        Rat::new(ck(a.checked_add(b))?, ck(self.d.checked_mul(o.d))?)
    }

    fn inv(self) -> Result<Rat, String> {
        if self.n == 0 {
            return Err("cannot invert zero while searching for a fraction".into());
        }
        Rat::new(self.d, self.n)
    }

    fn floor(self) -> i128 {
        div_floor(self.n, self.d)
    }

    fn ceil(self) -> i128 {
        div_ceil(self.n, self.d)
    }

    fn cmp_rat(self, o: Rat) -> Result<Ordering, String> {
        let a = ck(self.n.checked_mul(o.d))?;
        let b = ck(o.n.checked_mul(self.d))?;
        Ok(a.cmp(&b))
    }
}

fn neg_rat(r: Rat) -> Rat {
    Rat { n: -r.n, d: r.d }
}

fn abs_rat(r: Rat) -> Rat {
    Rat {
        n: r.n.abs(),
        d: r.d,
    }
}

fn ck(v: Option<i128>) -> Result<i128, String> {
    v.ok_or_else(|| OVERFLOW.to_string())
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    if a == 0 {
        1
    } else {
        a
    }
}

fn pow10(n: usize) -> Result<i128, String> {
    let mut v: i128 = 1;
    for _ in 0..n {
        v = ck(v.checked_mul(10))?;
    }
    Ok(v)
}

fn div_floor(n: i128, d: i128) -> i128 {
    let q = n / d;
    if n % d != 0 && ((n < 0) != (d < 0)) {
        q - 1
    } else {
        q
    }
}

fn div_ceil(n: i128, d: i128) -> i128 {
    let q = n / d;
    if n % d != 0 && ((n < 0) == (d < 0)) {
        q + 1
    } else {
        q
    }
}

/// Round `n/d` (with `d > 0`) to the nearest integer, ties away from zero.
fn div_round(n: i128, d: i128) -> Result<i128, String> {
    let twice = ck(n.abs().checked_mul(2))?;
    let q = ck(ck(twice.checked_add(d))?.checked_div(ck(d.checked_mul(2))?))?;
    Ok(if n < 0 { -q } else { q })
}

fn to_i64(v: i128, what: &str) -> Result<i64, String> {
    i64::try_from(v).map_err(|_| format!("the {what} is too large to report — use fewer digits"))
}

fn fmt_pair(n: i128, d: i128) -> String {
    if d == 1 {
        format!("{n}")
    } else {
        format!("{n}/{d}")
    }
}

fn fmt_rat(r: Rat) -> String {
    fmt_pair(r.n, r.d)
}

/// Format an f64 without a trailing `.0` or exponent noise, for step/summary text.
fn trim_float(v: f64) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    if v.abs() >= 1e-6 && v.abs() < 1e15 {
        let s = format!("{v:.12}");
        let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        if s.is_empty() || s == "-" {
            "0".to_string()
        } else {
            s
        }
    } else {
        format!("{v:e}")
    }
}

fn cf_display(terms: &[i128]) -> String {
    let head = terms[0].to_string();
    if terms.len() == 1 {
        return head;
    }
    let tail: Vec<String> = terms[1..].iter().map(|t| t.to_string()).collect();
    format!("{head}; {}", tail.join(", "))
}

// ---------------------------------------------------------------------------
// approximation
// ---------------------------------------------------------------------------

/// Continued-fraction terms of a non-negative rational.
fn cf_terms(v: Rat) -> Result<Vec<i128>, String> {
    let mut terms = Vec::new();
    let (mut n, mut d) = (v.n, v.d);
    while d != 0 && terms.len() < MAX_CF_TERMS {
        let a = div_floor(n, d);
        terms.push(a);
        let r = ck(n.checked_sub(ck(a.checked_mul(d))?))?;
        n = d;
        d = r;
    }
    Ok(terms)
}

/// Convergents `h_k/k_k` for the given continued-fraction terms.
fn convergents_from(terms: &[i128]) -> Result<Vec<(i128, i128)>, String> {
    let mut out = Vec::new();
    let (mut pm1, mut qm1) = (0i128, 1i128);
    let (mut p, mut q) = (1i128, 0i128);
    for &a in terms {
        let np = ck(ck(a.checked_mul(p))?.checked_add(pm1))?;
        let nq = ck(ck(a.checked_mul(q))?.checked_add(qm1))?;
        pm1 = p;
        qm1 = q;
        p = np;
        q = nq;
        out.push((p, q));
    }
    Ok(out)
}

/// Round `value` onto the `1/den` grid.
fn snap_to_denominator(value: Rat, den: i128, side: Side) -> Result<i128, String> {
    let scaled_n = ck(value.n.checked_mul(den))?;
    match side {
        Side::Nearest => div_round(scaled_n, value.d),
        Side::Up => Ok(div_ceil(scaled_n, value.d)),
        Side::Down => Ok(div_floor(scaled_n, value.d)),
    }
}

/// Best approximation to `value` with a denominator of at most `cap`, honouring `side`.
///
/// Candidates are the continued-fraction convergents plus the extreme semiconvergent at the
/// point the cap is reached — the set that provably contains the best such approximation.
fn best_within_denominator(value: Rat, cap: i128, side: Side) -> Result<Rat, String> {
    let sign_negative = value.n < 0;
    let v = abs_rat(value);
    let inner_side = match (side, sign_negative) {
        (Side::Nearest, _) => Side::Nearest,
        (Side::Up, false) | (Side::Down, true) => Side::Up,
        (Side::Down, false) | (Side::Up, true) => Side::Down,
    };
    let terms = cf_terms(v)?;

    let mut candidates: Vec<Rat> = Vec::new();
    candidates.push(Rat::int(v.floor()));
    candidates.push(Rat::int(ck(v.floor().checked_add(1))?));

    let (mut pm1, mut qm1) = (0i128, 1i128);
    let (mut p, mut q) = (1i128, 0i128);
    for &a in &terms {
        if q > 0 && cap > qm1 {
            let mut t = (cap - qm1) / q;
            if t > a {
                t = a;
            }
            if t >= 1 {
                let sp = ck(ck(t.checked_mul(p))?.checked_add(pm1))?;
                let sq = ck(ck(t.checked_mul(q))?.checked_add(qm1))?;
                if sq <= cap {
                    candidates.push(Rat::new(sp, sq)?);
                }
            }
        }
        let np = ck(ck(a.checked_mul(p))?.checked_add(pm1))?;
        let nq = ck(ck(a.checked_mul(q))?.checked_add(qm1))?;
        pm1 = p;
        qm1 = q;
        p = np;
        q = nq;
        if q <= cap {
            candidates.push(Rat::new(p, q)?);
        } else {
            break;
        }
    }

    let best = pick_candidate(&candidates, v, inner_side)?;
    Ok(if sign_negative { neg_rat(best) } else { best })
}

/// Choose the candidate closest to `v` on the requested side; ties go to the simpler fraction.
fn pick_candidate(candidates: &[Rat], v: Rat, side: Side) -> Result<Rat, String> {
    let mut best: Option<(Rat, Rat)> = None; // (candidate, |candidate - v|)
    for &c in candidates {
        if c.d > 0 {
            let ord = c.cmp_rat(v)?;
            let allowed = match side {
                Side::Nearest => true,
                Side::Up => ord != Ordering::Less,
                Side::Down => ord != Ordering::Greater,
            };
            if !allowed {
                continue;
            }
            let diff = abs_rat(c.sub(v)?);
            let better = match &best {
                None => true,
                Some((bc, bd)) => match cmp_abs(diff, *bd) {
                    Ordering::Less => true,
                    Ordering::Equal => c.d < bc.d,
                    Ordering::Greater => false,
                },
            };
            if better {
                best = Some((c, diff));
            }
        }
    }
    best.map(|(c, _)| c).ok_or_else(|| {
        "no fraction satisfies that combination of rounding direction and denominator limit"
            .to_string()
    })
}

/// Compare two non-negative rationals, falling back to f64 if the exact compare overflows.
fn cmp_abs(a: Rat, b: Rat) -> Ordering {
    match a.cmp_rat(b) {
        Ok(o) => o,
        Err(_) => a
            .to_f64()
            .partial_cmp(&b.to_f64())
            .unwrap_or(Ordering::Equal),
    }
}

/// The simplest fraction within `tol` of `value`, on the requested side.
fn simplest_within(value: Rat, tol: Rat, side: Side, _depth: usize) -> Result<Rat, String> {
    let (lo, hi) = match side {
        Side::Nearest => (value.sub(tol)?, value.add(tol)?),
        Side::Up => (value, value.add(tol)?),
        Side::Down => (value.sub(tol)?, value),
    };
    simplest_between(lo, hi, 0)
}

/// Smallest-denominator fraction in `[lo, hi]` (Stern-Brocot descent via continued fractions).
fn simplest_between(lo: Rat, hi: Rat, depth: usize) -> Result<Rat, String> {
    if depth > 64 {
        return Err(OVERFLOW.to_string());
    }
    if lo.cmp_rat(hi)? == Ordering::Greater {
        return simplest_between(hi, lo, depth + 1);
    }
    if lo.n <= 0 && hi.n >= 0 {
        return Ok(Rat::int(0));
    }
    if hi.n < 0 {
        return Ok(neg_rat(simplest_between(
            neg_rat(hi),
            neg_rat(lo),
            depth + 1,
        )?));
    }
    // 0 < lo <= hi
    let candidate = Rat::int(lo.ceil());
    if candidate.cmp_rat(hi)? != Ordering::Greater {
        return Ok(candidate);
    }
    let floor = lo.floor();
    let base = Rat::int(floor);
    let lo2 = hi.sub(base)?.inv()?;
    let hi2 = lo.sub(base)?.inv()?;
    let inner = simplest_between(lo2, hi2, depth + 1)?;
    Rat::new(
        ck(ck(floor.checked_mul(inner.n))?.checked_add(inner.d))?,
        inner.n,
    )
}

// ---------------------------------------------------------------------------
// tolerance helper
// ---------------------------------------------------------------------------

/// Turn a small positive f64 tolerance into an exact rational (12 decimal places).
fn f64_to_rat(v: f64) -> Result<Rat, String> {
    let scale = pow10(TOLERANCE_DIGITS)?;
    let scaled = (v * scale as f64).round() as i128;
    if scaled <= 0 {
        return Err(format!(
            "tolerance must be 0 (exact) or at least {MIN_TOLERANCE:e}"
        ));
    }
    Rat::new(scaled, scale)
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(decimal: &str) -> Inputs {
        Inputs {
            decimal: decimal.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn terminating_decimal_converts_exactly() {
        let c = convert(&inputs("0.625")).unwrap();
        assert_eq!(c.fraction, "5/8");
        assert_eq!(c.numerator, 5);
        assert_eq!(c.denominator, 8);
        assert!(c.is_exact);
        assert!(c.is_proper);
        assert_eq!(c.error, 0.0);
        assert_eq!(c.summary, "0.625 = 5/8 exactly.");
        assert!(c.steps.iter().any(|s| s.contains("625/1000")));
    }

    #[test]
    fn improper_value_reports_a_mixed_number() {
        let c = convert(&inputs("1.625")).unwrap();
        assert_eq!(c.fraction, "13/8");
        assert_eq!(c.mixed_number, "1 5/8");
        assert_eq!(c.whole_part, 1);
        assert_eq!(c.proper_numerator, 5);
        assert!(!c.is_proper);
    }

    #[test]
    fn negative_and_whole_values_work() {
        let c = convert(&inputs("-2.5")).unwrap();
        assert_eq!(c.fraction, "-5/2");
        assert_eq!(c.mixed_number, "-2 1/2");
        let whole = convert(&inputs("4")).unwrap();
        assert_eq!(whole.fraction, "4");
        assert_eq!(whole.mixed_number, "4");
        assert_eq!(whole.denominator, 1);
    }

    #[test]
    fn repeating_notation_is_exact() {
        for form in ["0.(3)", "0.[3]", "0.3..."] {
            let c = convert(&inputs(form)).unwrap();
            assert_eq!(c.fraction, "1/3", "form {form}");
            assert!(c.is_exact);
            assert!(c.repeating);
            assert_eq!(c.repeating_digits, 1);
        }
        let c = convert(&inputs("2.6(6)")).unwrap();
        assert_eq!(c.fraction, "8/3");
        assert_eq!(c.mixed_number, "2 2/3");
        let two = convert(&inputs("0.(36)")).unwrap();
        assert_eq!(two.fraction, "4/11");
        let mixed = convert(&inputs("1.8(3)")).unwrap();
        assert_eq!(mixed.fraction, "11/6");
    }

    #[test]
    fn repeating_digits_count_matches_competitor_style_input() {
        let c = convert(&Inputs {
            decimal: "0.625".into(),
            repeating_digits: Some(2.0),
            ..Default::default()
        })
        .unwrap();
        // 0.6(25) = 619/990
        assert_eq!(c.fraction, "619/990");
        assert!(c.is_exact);
    }

    #[test]
    fn percent_scientific_and_separators_parse() {
        assert_eq!(convert(&inputs("12.5%")).unwrap().fraction, "1/8");
        assert_eq!(convert(&inputs("1.25e-3")).unwrap().fraction, "1/800");
        assert_eq!(convert(&inputs("1,234.5")).unwrap().fraction, "2469/2");
        assert_eq!(convert(&inputs("\u{2212}0.75")).unwrap().fraction, "-3/4");
    }

    #[test]
    fn tolerance_finds_the_simplest_fraction() {
        let c = convert(&Inputs {
            decimal: "0.142857".into(),
            tolerance: Some(0.000001),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(c.fraction, "1/7");
        assert_eq!(c.method, "tolerance");
        assert!(!c.is_exact);
        assert!(c.error.abs() < 0.000001);

        let pi = convert(&Inputs {
            decimal: "3.14159265".into(),
            tolerance: Some(0.000001),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(pi.fraction, "355/113");
        assert_eq!(pi.mixed_number, "3 16/113");
    }

    #[test]
    fn max_denominator_caps_the_denominator() {
        let c = convert(&Inputs {
            decimal: "3.14159265".into(),
            max_denominator: Some(100.0),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(c.fraction, "311/99");
        assert_eq!(c.method, "max-denominator");
        assert!(c.denominator <= 100);
        assert!(c.convergents.iter().any(|v| v == "22/7"));
    }

    #[test]
    fn fixed_denominator_snaps_and_respects_rounding() {
        let nearest = convert(&Inputs {
            decimal: "0.31".into(),
            denominator: Some(16.0),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(nearest.fraction, "5/16");
        assert_eq!(nearest.method, "fixed-denominator");

        let down = convert(&Inputs {
            decimal: "0.31".into(),
            denominator: Some(16.0),
            rounding: "down".into(),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(down.fraction, "1/4"); // 4/16 reduced
        assert!(down.fraction_value < 0.31);

        let up = convert(&Inputs {
            decimal: "0.31".into(),
            denominator: Some(16.0),
            rounding: "up".into(),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(up.fraction, "5/16");
    }

    #[test]
    fn reduce_false_keeps_the_denominator() {
        let raw = convert(&Inputs {
            decimal: "0.5".into(),
            denominator: Some(16.0),
            reduce: Some(false),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(raw.fraction, "8/16");
        assert!(!raw.reduced);
        assert!(raw.is_exact);

        let exact_raw = convert(&Inputs {
            decimal: "0.625".into(),
            reduce: Some(false),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(exact_raw.fraction, "625/1000");
    }

    #[test]
    fn tolerance_beyond_the_cap_falls_back_to_the_cap() {
        let c = convert(&Inputs {
            decimal: "0.142857".into(),
            tolerance: Some(0.000000001),
            max_denominator: Some(10.0),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(c.fraction, "1/7");
        assert_eq!(c.method, "max-denominator");
        assert!(c.steps.iter().any(|s| s.contains("No fraction within")));
    }

    #[test]
    fn one_sided_tolerance_never_undershoots() {
        let up = convert(&Inputs {
            decimal: "0.142857".into(),
            tolerance: Some(0.01),
            rounding: "up".into(),
            ..Default::default()
        })
        .unwrap();
        assert!(up.fraction_value >= 0.142857);
        let down = convert(&Inputs {
            decimal: "0.142857".into(),
            tolerance: Some(0.01),
            rounding: "down".into(),
            ..Default::default()
        })
        .unwrap();
        assert!(down.fraction_value <= 0.142857);
    }

    #[test]
    fn zero_converts() {
        let c = convert(&inputs("0.0")).unwrap();
        assert_eq!(c.fraction, "0");
        assert_eq!(c.error_percent, 0.0);
        assert!(c.is_exact);
    }

    #[test]
    fn blank_input_is_an_error() {
        let err = convert(&inputs("   ")).unwrap_err();
        assert!(err.contains("enter a decimal number"), "got {err}");
    }

    #[test]
    fn non_numeric_input_is_an_error() {
        let err = convert(&inputs("abc")).unwrap_err();
        assert!(err.contains("not a decimal number"), "got {err}");
        let err = convert(&inputs("1.2.3")).unwrap_err();
        assert!(
            err.contains("not a decimal number") || err.contains("decimal point"),
            "got {err}"
        );
    }

    #[test]
    fn out_of_range_arguments_are_errors() {
        let err = convert(&Inputs {
            decimal: "0.5".into(),
            denominator: Some(1e15),
            ..Default::default()
        })
        .unwrap_err();
        assert!(err.contains("denominator must be between"), "got {err}");

        let err = convert(&Inputs {
            decimal: "0.5".into(),
            tolerance: Some(2.0),
            ..Default::default()
        })
        .unwrap_err();
        assert!(
            err.contains("tolerance must be smaller than 1"),
            "got {err}"
        );

        let err = convert(&Inputs {
            decimal: "0.5".into(),
            rounding: "sideways".into(),
            ..Default::default()
        })
        .unwrap_err();
        assert!(err.contains("rounding must be"), "got {err}");

        let err = convert(&Inputs {
            decimal: "0.5".into(),
            repeating_digits: Some(2.0),
            ..Default::default()
        })
        .unwrap_err();
        assert!(err.contains("only 1 decimal place"), "got {err}");
    }

    #[test]
    fn too_many_decimal_places_is_an_error() {
        let err = convert(&inputs("0.1234567890123456789")).unwrap_err();
        assert!(err.contains("decimal places are supported"), "got {err}");
    }

    #[test]
    fn json_output_is_pretty_and_complete() {
        let json = convert_json(&inputs("0.625")).unwrap();
        assert!(json.contains("\"fraction\": \"5/8\""));
        assert!(json.contains("\"mixed_number\": \"5/8\""));
        assert!(json.contains("\"is_exact\": true"));
        assert!(json.contains("\"method\": \"exact\""));
        assert!(json.contains("\"convergents\""));
    }

    #[test]
    fn convergent_ladder_is_reported() {
        let c = convert(&inputs("0.142857")).unwrap();
        assert_eq!(c.convergents.first().unwrap(), "0/1");
        assert_eq!(c.convergents.get(1).unwrap(), "1/7");
        assert_eq!(c.exact_fraction, "142857/1000000");
    }
}
