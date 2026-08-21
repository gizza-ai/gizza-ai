//! gizza-ai/csv-column-number-formatter core — pure compute, shared by the chat
//! skill block and the web page. No wafer/wasm-bindgen deps, no locale tables.
//!
//! Applies ONE uniform numeric format — fixed decimal places, a rounding mode,
//! digit grouping, separators, a sign style and an optional prefix/suffix — to
//! the cells of the CSV columns you pick, and writes the table back out.
//!
//! Correctness note: rounding runs on the DIGIT STRING that was parsed out of
//! the cell, not on a binary `f64`. `1.005` at two places is therefore `1.01`,
//! where the naive `x * 100.0` round returns `1.00` (`1.005` has no exact binary
//! representation — the classic money bug). Nothing is ever converted to `f64`.
//!
//! Distinct from `number-to-currency-formatter` (ONE value, currency symbols and
//! ISO codes, no table), `numeric-string-sanitizer` (the opposite direction —
//! formatted cells back to plain machine floats) and `csv-regex-replace` (a
//! textual find-and-replace with no numeric semantics).

use csv::{QuoteStyle, ReaderBuilder, StringRecord, WriterBuilder};

/// Hard cap on the input table, mirroring the sibling CSV blocks.
pub const MAX_INPUT_BYTES: usize = 5_000_000;

/// Smallest accepted `decimals` value (round to billions).
pub const MIN_DECIMALS: i32 = -9;
/// Largest accepted `decimals` value.
pub const MAX_DECIMALS: i32 = 15;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// How a digit that has to be dropped is resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rounding {
    /// Round half away from zero (the spreadsheet ROUND). 2.5→3, -2.5→-3.
    HalfUp,
    /// Round half toward zero. 2.5→2, -2.5→-2.
    HalfDown,
    /// Round half to even (banker's rounding). 0.5→0, 1.5→2, 2.5→2.
    HalfEven,
    /// Always toward +∞. 2.1→3, -2.9→-2.
    Ceil,
    /// Always toward -∞. 2.9→2, -2.1→-3.
    Floor,
    /// Drop the extra digits (toward zero). 2.9→2, -2.9→-2.
    Truncate,
}

impl Rounding {
    pub fn parse(s: &str) -> Result<Rounding, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "half_up" | "half-up" | "halfup" => Ok(Rounding::HalfUp),
            "half_down" | "half-down" => Ok(Rounding::HalfDown),
            "half_even" | "half-even" | "bankers" => Ok(Rounding::HalfEven),
            "ceil" | "ceiling" | "up" => Ok(Rounding::Ceil),
            "floor" | "down" => Ok(Rounding::Floor),
            "truncate" | "trunc" => Ok(Rounding::Truncate),
            other => Err(format!(
                "unknown rounding '{other}' (use 'half_up', 'half_down', 'half_even', 'ceil', 'floor', or 'truncate')"
            )),
        }
    }
}

/// The overall shape of the rendered number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notation {
    /// Plain positional digits.
    Standard,
    /// Scaled to K / M / B / T with the unit letter appended.
    Compact,
    /// One digit before the decimal mark plus an `e±X` exponent.
    Scientific,
    /// Multiplied by 100 with a `%` appended.
    Percent,
}

impl Notation {
    pub fn parse(s: &str) -> Result<Notation, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "standard" | "plain" => Ok(Notation::Standard),
            "compact" | "abbreviated" => Ok(Notation::Compact),
            "scientific" | "sci" => Ok(Notation::Scientific),
            "percent" | "percentage" => Ok(Notation::Percent),
            other => Err(format!(
                "unknown notation '{other}' (use 'standard', 'compact', 'scientific', or 'percent')"
            )),
        }
    }
}

/// How the integer digits are grouped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grouping {
    /// No separators at all — the value still parses as a number downstream.
    None,
    /// Western: groups of three from the right (`1,234,567`).
    Thousands,
    /// South Asian: three, then twos (`12,34,567`).
    Indian,
}

impl Grouping {
    pub fn parse(s: &str) -> Result<Grouping, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" | "off" => Ok(Grouping::None),
            "thousands" | "western" | "three" => Ok(Grouping::Thousands),
            "indian" | "lakh" | "south_asian" => Ok(Grouping::Indian),
            other => Err(format!(
                "unknown grouping '{other}' (use 'none', 'thousands', or 'indian')"
            )),
        }
    }
}

/// Which character separates digit groups.
fn group_separator(s: &str) -> Result<&'static str, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "comma" => Ok(","),
        "period" | "dot" | "point" => Ok("."),
        "space" => Ok(" "),
        "thin_space" | "thin-space" | "narrow" => Ok("\u{202f}"),
        "apostrophe" | "quote" => Ok("'"),
        "underscore" => Ok("_"),
        other => Err(format!(
            "unknown group_separator '{other}' (use 'comma', 'period', 'space', 'thin_space', 'apostrophe', or 'underscore')"
        )),
    }
}

/// Which character separates the fractional part on OUTPUT.
fn decimal_separator(s: &str) -> Result<&'static str, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "period" | "dot" | "point" => Ok("."),
        "comma" => Ok(","),
        other => Err(format!(
            "unknown decimal_separator '{other}' (use 'period' or 'comma')"
        )),
    }
}

/// How the sign is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sign {
    /// `-` on negatives only.
    Auto,
    /// `+` on zero and positives, `-` on negatives.
    Always,
    /// `+` on positives, nothing on zero, `-` on negatives.
    ExceptZero,
    /// No sign at all — the magnitude.
    Never,
    /// A space where the `+` would go, so a column lines up.
    Space,
    /// Accounting parentheses around negatives: `(1234.00)`.
    Parens,
}

impl Sign {
    pub fn parse(s: &str) -> Result<Sign, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Sign::Auto),
            "always" => Ok(Sign::Always),
            "except_zero" | "except-zero" => Ok(Sign::ExceptZero),
            "never" | "none" => Ok(Sign::Never),
            "space" => Ok(Sign::Space),
            "parens" | "parentheses" | "accounting" => Ok(Sign::Parens),
            other => Err(format!(
                "unknown sign '{other}' (use 'auto', 'always', 'except_zero', 'never', 'space', or 'parens')"
            )),
        }
    }
}

/// Which character the INPUT cells use as their decimal mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputDecimal {
    /// Decide per cell from the punctuation actually present.
    Auto,
    /// `1,234.56` — dot is the decimal mark, comma groups.
    Dot,
    /// `1.234,56` — comma is the decimal mark, dot groups.
    Comma,
}

impl InputDecimal {
    pub fn parse(s: &str) -> Result<InputDecimal, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(InputDecimal::Auto),
            "dot" | "period" | "point" => Ok(InputDecimal::Dot),
            "comma" => Ok(InputDecimal::Comma),
            other => Err(format!(
                "unknown input_decimal '{other}' (use 'auto', 'dot', or 'comma')"
            )),
        }
    }
}

/// What to do with a selected cell that is not a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonNumeric {
    /// Copy the cell through untouched (default).
    Keep,
    /// Replace it with an empty cell.
    Blank,
    /// Stop and report where it was.
    Error,
}

impl NonNumeric {
    pub fn parse(s: &str) -> Result<NonNumeric, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "keep" => Ok(NonNumeric::Keep),
            "blank" | "empty" => Ok(NonNumeric::Blank),
            "error" | "fail" => Ok(NonNumeric::Error),
            other => Err(format!(
                "unknown non_numeric '{other}' (use 'keep', 'blank', or 'error')"
            )),
        }
    }
}

/// What the tool returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// The whole table with the format applied.
    Csv,
    /// Only the rows whose selected cells actually changed, plus the header.
    Changed,
    /// A per-column `column,cells_formatted,cells_unchanged,non_numeric` audit.
    Report,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "csv" => Ok(Output::Csv),
            "changed" => Ok(Output::Changed),
            "report" => Ok(Output::Report),
            other => Err(format!(
                "unknown output '{other}' (use 'csv', 'changed', or 'report')"
            )),
        }
    }
}

fn quote_style(s: &str) -> Result<QuoteStyle, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "minimal" => Ok(QuoteStyle::Necessary),
        "always" => Ok(QuoteStyle::Always),
        "non_numeric" | "non-numeric" => Ok(QuoteStyle::NonNumeric),
        other => Err(format!(
            "unknown quote_style '{other}' (use 'minimal', 'always', or 'non_numeric')"
        )),
    }
}

/// Resolve a delimiter spec (`auto` is handled by the caller) to a single byte.
fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d.trim() {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be 'auto', a single character, or comma/tab/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Count a candidate separator on the first line, ignoring anything inside quotes.
fn count_outside_quotes(line: &str, sep: char) -> usize {
    let mut in_quotes = false;
    let mut n = 0;
    for c in line.chars() {
        if c == '"' {
            in_quotes = !in_quotes;
        } else if c == sep && !in_quotes {
            n += 1;
        }
    }
    n
}

/// Sniff the separator from the first line: most frequent candidate outside
/// quotes, comma winning any tie.
fn sniff_delimiter(data: &str) -> u8 {
    let first = data.lines().next().unwrap_or("");
    let mut best = b',';
    let mut best_n = 0;
    for (c, b) in [(',', b','), ('\t', b'\t'), (';', b';'), ('|', b'|')] {
        let n = count_outside_quotes(first, c);
        if n > best_n {
            best_n = n;
            best = b;
        }
    }
    best
}

// ---------------------------------------------------------------------------
// Column selection (names / 1-based indices / ranges)
// ---------------------------------------------------------------------------

/// Resolve the `columns` spec into a per-column selected flag. Blank or `*`
/// selects every column.
fn resolve_columns(
    spec: &str,
    header: Option<&StringRecord>,
    width: usize,
) -> Result<Vec<bool>, String> {
    let trimmed = spec.trim();
    if trimmed.is_empty() || trimmed == "*" {
        return Ok(vec![true; width]);
    }
    let mut selected = vec![false; width];
    for raw in trimmed.split(',') {
        let key = raw.trim();
        if key.is_empty() {
            continue;
        }
        if let Some(idx) = match_header(key, header) {
            selected[idx] = true;
            continue;
        }
        // A `2-4` range, but only when both sides are plain numbers — a header
        // named "unit-price" already matched above.
        if let Some((lo, hi)) = key.split_once('-') {
            let (lo, hi) = (lo.trim(), hi.trim());
            if !lo.is_empty()
                && !hi.is_empty()
                && lo.chars().all(|c| c.is_ascii_digit())
                && hi.chars().all(|c| c.is_ascii_digit())
            {
                let lo = parse_index(lo, width)?;
                let hi = parse_index(hi, width)?;
                if lo > hi {
                    return Err(format!(
                        "column range '{key}' runs backwards (write it low-to-high)"
                    ));
                }
                for item in selected.iter_mut().take(hi + 1).skip(lo) {
                    *item = true;
                }
                continue;
            }
        }
        let idx = match parse_index(key, width) {
            Ok(i) => i,
            Err(e) if key.chars().all(|c| c.is_ascii_digit()) => return Err(e),
            Err(e) => {
                return Err(match header {
                    Some(h) => {
                        let names: Vec<&str> = h.iter().collect();
                        format!(
                            "no column named '{key}' and it is not a valid index — available: {}",
                            names.join(", ")
                        )
                    }
                    None => format!(
                        "{e} (there is no header row, so columns must be 1-based indices or ranges)"
                    ),
                })
            }
        };
        selected[idx] = true;
    }
    if !selected.iter().any(|s| *s) {
        return Err("no columns selected — leave 'columns' blank to format every column".into());
    }
    Ok(selected)
}

fn match_header(key: &str, header: Option<&StringRecord>) -> Option<usize> {
    let hdr = header?;
    for (i, name) in hdr.iter().enumerate() {
        if name.trim() == key {
            return Some(i);
        }
    }
    for (i, name) in hdr.iter().enumerate() {
        if name.trim().eq_ignore_ascii_case(key) {
            return Some(i);
        }
    }
    None
}

/// Parse a 1-based column index into a 0-based offset, bounds-checked.
fn parse_index(key: &str, width: usize) -> Result<usize, String> {
    let n: usize = key.parse().map_err(|_| {
        format!("column must be a name, a 1-based index, or a range like 2-4, got '{key}'")
    })?;
    if n == 0 || n > width {
        return Err(format!(
            "column index {n} out of range (the table has {width} column(s))"
        ));
    }
    Ok(n - 1)
}

// ---------------------------------------------------------------------------
// Number parsing — digit strings only, never f64
// ---------------------------------------------------------------------------

/// A parsed cell value: sign plus an exact decimal digit string.
///
/// The value is `(-1)^neg * digits * 10^(-scale)`, where `digits` holds only
/// ASCII digits and `scale` is how many of them are fractional.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Decimal {
    neg: bool,
    digits: String,
    scale: usize,
}

impl Decimal {
    /// Multiply by `10^n` without touching the digits.
    fn shift_left(&mut self, n: usize) {
        for _ in 0..n {
            if self.scale > 0 {
                self.scale -= 1;
            } else {
                self.digits.push('0');
            }
        }
    }

    /// Divide by `10^n` without touching the digits.
    fn shift_right(&mut self, n: usize) {
        self.scale += n;
    }

    /// Digits before the decimal mark, with no leading zeros (at least "0").
    fn int_len(&self) -> usize {
        self.digits.len().saturating_sub(self.scale)
    }

    fn is_zero(&self) -> bool {
        self.digits.bytes().all(|b| b == b'0')
    }
}

/// Characters accepted as a digit-group mark in the input.
const INPUT_GROUP_MARKS: [char; 5] = [' ', '\u{202f}', '\u{a0}', '_', '\''];

/// Parse one cell into an exact [`Decimal`].
///
/// Tolerates leading/trailing whitespace, a leading or trailing currency-ish
/// symbol, group marks (`,` `.` space `_` `'`), a leading `+`/`-`/`−`, a
/// trailing `-` (ledger form), accounting parentheses, and scientific notation.
/// `input_decimal` fixes which of `.` and `,` is the decimal mark; `Auto`
/// decides per cell.
fn parse_decimal(raw: &str, input_decimal: InputDecimal) -> Result<Decimal, String> {
    let mut s = raw.trim().to_string();
    if s.is_empty() {
        return Err("empty".into());
    }

    let mut neg = false;
    // Accounting parentheses.
    if s.starts_with('(') && s.ends_with(')') && s.len() >= 3 {
        neg = true;
        s = s[1..s.len() - 1].trim().to_string();
    }
    // Strip a currency-ish symbol on either end: anything that is not a digit,
    // a sign, a decimal/group mark, an exponent letter or a percent sign.
    let is_symbol = |c: char| {
        !(c.is_ascii_digit()
            || c == '+'
            || c == '-'
            || c == '\u{2212}'
            || c == '.'
            || c == ','
            || c == 'e'
            || c == 'E'
            || c == '%'
            || INPUT_GROUP_MARKS.contains(&c))
    };
    while s.chars().next().is_some_and(is_symbol) {
        let mut it = s.chars();
        it.next();
        s = it.as_str().trim_start().to_string();
    }
    while s.chars().next_back().is_some_and(is_symbol) {
        let mut it = s.chars();
        it.next_back();
        s = it.as_str().trim_end().to_string();
    }
    // A trailing percent sign is only presentation on input — the value stays
    // as written (45.2% parses as 45.2). Notation::Percent is what scales.
    if let Some(rest) = s.strip_suffix('%') {
        s = rest.trim_end().to_string();
    }
    // Leading sign, then the ledger trailing minus.
    if let Some(rest) = s.strip_prefix(['-', '\u{2212}']) {
        neg = !neg;
        s = rest.trim_start().to_string();
    } else if let Some(rest) = s.strip_prefix('+') {
        s = rest.trim_start().to_string();
    }
    if let Some(rest) = s.strip_suffix(['-', '\u{2212}']) {
        neg = !neg;
        s = rest.trim_end().to_string();
    }
    if s.is_empty() {
        return Err("no digits".into());
    }

    // Split off a scientific exponent.
    let mut exponent: i32 = 0;
    if let Some(pos) = s.find(['e', 'E']) {
        let (mantissa, exp) = s.split_at(pos);
        let exp = &exp[1..];
        if mantissa.is_empty() || exp.is_empty() {
            return Err("malformed exponent".into());
        }
        exponent = exp
            .replace('\u{2212}', "-")
            .parse::<i32>()
            .map_err(|_| "malformed exponent".to_string())?;
        if !(-1_000..=1_000).contains(&exponent) {
            return Err("exponent out of range".into());
        }
        s = mantissa.to_string();
    }

    // Decide which mark is the decimal point.
    let dot_count = s.matches('.').count();
    let comma_count = s.matches(',').count();
    let decimal_char: Option<char> = match input_decimal {
        InputDecimal::Dot => Some('.'),
        InputDecimal::Comma => Some(','),
        InputDecimal::Auto => {
            if dot_count > 0 && comma_count > 0 {
                // Whichever comes last is the decimal mark.
                if s.rfind('.') > s.rfind(',') {
                    Some('.')
                } else {
                    Some(',')
                }
            } else if dot_count == 1 && comma_count == 0 {
                Some('.')
            } else if comma_count == 1 && dot_count == 0 {
                // A lone comma followed by exactly three digits is grouping
                // ("1,234"); anything else is a decimal comma ("0,5").
                let tail = &s[s.find(',').unwrap() + ','.len_utf8()..];
                if tail.len() == 3 && tail.bytes().all(|b| b.is_ascii_digit()) {
                    None
                } else {
                    Some(',')
                }
            } else {
                // Zero or several of one mark → all grouping.
                None
            }
        }
    };

    let mut digits = String::new();
    let mut scale = 0usize;
    let mut seen_decimal = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            if seen_decimal {
                scale += 1;
            }
        } else if Some(c) == decimal_char {
            if seen_decimal {
                return Err("more than one decimal mark".into());
            }
            seen_decimal = true;
        } else if c == '.' || c == ',' || INPUT_GROUP_MARKS.contains(&c) {
            // A group mark. After the decimal mark it is meaningless punctuation.
            if seen_decimal {
                return Err("group mark after the decimal mark".into());
            }
        } else {
            return Err(format!("unexpected character '{c}'"));
        }
    }
    if digits.is_empty() {
        return Err("no digits".into());
    }
    if digits.len() > 4096 {
        return Err("too many digits".into());
    }

    let mut d = Decimal { neg, digits, scale };
    match exponent.cmp(&0) {
        std::cmp::Ordering::Greater => d.shift_left(exponent as usize),
        std::cmp::Ordering::Less => d.shift_right(exponent.unsigned_abs() as usize),
        std::cmp::Ordering::Equal => {}
    }
    Ok(d)
}

// ---------------------------------------------------------------------------
// Rounding + rendering
// ---------------------------------------------------------------------------

/// Add one to a digit string, returning the (possibly longer) result.
fn increment(digits: &str) -> String {
    let mut bytes: Vec<u8> = digits.as_bytes().to_vec();
    let mut i = bytes.len();
    loop {
        if i == 0 {
            bytes.insert(0, b'1');
            break;
        }
        i -= 1;
        if bytes[i] == b'9' {
            bytes[i] = b'0';
        } else {
            bytes[i] += 1;
            break;
        }
    }
    String::from_utf8(bytes).expect("ascii digits stay ascii")
}

/// Round `d` to a multiple of `10^(-decimals)` and return the integer digit
/// string of the SCALED value (i.e. the value times `10^decimals`), plus the
/// sign after any `-0` collapse.
fn round_scaled(d: &Decimal, decimals: i32, mode: Rounding) -> (bool, String) {
    let shift = d.scale as i64 - decimals as i64;
    let mut kept = if shift <= 0 {
        let mut s = d.digits.clone();
        // Guard against an absurd zero-fill from a huge negative shift.
        let pad = (-shift).min(4096) as usize;
        s.push_str(&"0".repeat(pad));
        s
    } else {
        let shift = shift as usize;
        if shift >= d.digits.len() {
            // Everything is dropped: the kept part is a bare zero, and the
            // dropped digits are the whole number padded on the left.
            let mut dropped = "0".repeat(shift - d.digits.len());
            dropped.push_str(&d.digits);
            let up = round_up(&dropped, "0", d.neg, mode);
            if up {
                "1".to_string()
            } else {
                "0".to_string()
            }
        } else {
            let (head, tail) = d.digits.split_at(d.digits.len() - shift);
            if round_up(tail, head, d.neg, mode) {
                increment(head)
            } else {
                head.to_string()
            }
        }
    };
    if kept.is_empty() {
        kept.push('0');
    }
    let all_zero = kept.bytes().all(|b| b == b'0');
    (d.neg && !all_zero, kept)
}

/// Decide whether the kept digits must be incremented, given the digits being
/// dropped, the digits being kept, the sign, and the mode.
fn round_up(dropped: &str, kept: &str, neg: bool, mode: Rounding) -> bool {
    if dropped.bytes().all(|b| b == b'0') {
        return false;
    }
    let first = dropped.as_bytes()[0];
    let rest_nonzero = dropped.as_bytes()[1..].iter().any(|b| *b != b'0');
    match mode {
        Rounding::Truncate => false,
        Rounding::Ceil => !neg,
        Rounding::Floor => neg,
        Rounding::HalfUp => first >= b'5',
        Rounding::HalfDown => first > b'5' || (first == b'5' && rest_nonzero),
        Rounding::HalfEven => {
            if first > b'5' || (first == b'5' && rest_nonzero) {
                true
            } else if first == b'5' {
                let last = kept.bytes().next_back().unwrap_or(b'0');
                (last - b'0') % 2 == 1
            } else {
                false
            }
        }
    }
}

/// Split the scaled integer digits back into an integer part and a fraction of
/// exactly `decimals` digits (`decimals < 0` zero-fills the integer part).
fn split_parts(scaled: &str, decimals: i32) -> (String, String) {
    if decimals >= 0 {
        let d = decimals as usize;
        let padded = if scaled.len() <= d {
            format!("{}{}", "0".repeat(d + 1 - scaled.len()), scaled)
        } else {
            scaled.to_string()
        };
        let split = padded.len() - d;
        let (int, frac) = padded.split_at(split);
        (strip_leading_zeros(int), frac.to_string())
    } else {
        let mut int = strip_leading_zeros(scaled);
        // A value that rounded away to nothing is a plain "0", not "0000".
        if int != "0" {
            int.push_str(&"0".repeat(decimals.unsigned_abs() as usize));
        }
        (int, String::new())
    }
}

fn strip_leading_zeros(s: &str) -> String {
    let t = s.trim_start_matches('0');
    if t.is_empty() {
        "0".to_string()
    } else {
        t.to_string()
    }
}

/// Insert the group separator into an integer digit string.
fn group_digits(int: &str, grouping: Grouping, sep: &str) -> String {
    match grouping {
        Grouping::None => int.to_string(),
        Grouping::Thousands => {
            let mut out = String::new();
            let n = int.len();
            for (i, c) in int.chars().enumerate() {
                if i > 0 && (n - i) % 3 == 0 {
                    out.push_str(sep);
                }
                out.push(c);
            }
            out
        }
        Grouping::Indian => {
            // Last three digits, then groups of two.
            if int.len() <= 3 {
                return int.to_string();
            }
            let (head, tail) = int.split_at(int.len() - 3);
            let mut groups: Vec<String> = Vec::new();
            let bytes = head.as_bytes();
            let mut end = bytes.len();
            while end > 0 {
                let start = end.saturating_sub(2);
                groups.push(String::from_utf8(bytes[start..end].to_vec()).unwrap());
                end = start;
            }
            groups.reverse();
            groups.push(tail.to_string());
            groups.join(sep)
        }
    }
}

/// The compact-notation unit letters, one per power of 1000.
const COMPACT_UNITS: [&str; 5] = ["", "K", "M", "B", "T"];

/// Format one parsed value into its final display string.
#[allow(clippy::too_many_arguments)]
fn render(
    value: &Decimal,
    decimals: i32,
    rounding: Rounding,
    notation: Notation,
    grouping: Grouping,
    gsep: &str,
    dsep: &str,
    sign: Sign,
    prefix: &str,
    suffix: &str,
) -> String {
    let mut v = value.clone();
    let mut unit = "";
    let mut exp_text = String::new();

    match notation {
        Notation::Standard => {}
        Notation::Percent => v.shift_left(2),
        Notation::Compact => {
            // The largest unit whose scaled value still has an integer part:
            // a number with N integer digits belongs to unit floor((N-1)/3).
            let significant = v.digits.trim_start_matches('0');
            let mut idx = if significant.is_empty() {
                0
            } else {
                let leading = v.digits.len() - significant.len();
                let int_len = (v.digits.len() - leading) as i64 - v.scale as i64;
                if int_len <= 1 {
                    0
                } else {
                    ((int_len - 1) / 3).min(COMPACT_UNITS.len() as i64 - 1) as usize
                }
            };
            // Rounding can carry 999.999K up to 1000K — promote a unit so that
            // 999,999 at 0 decimals reads 1M rather than 1000K.
            if idx + 1 < COMPACT_UNITS.len() {
                let mut probe = v.clone();
                probe.shift_right(3 * idx);
                let (_, scaled) = round_scaled(&probe, decimals, rounding);
                let (int, _) = split_parts(&scaled, decimals);
                if int.len() > 3 {
                    idx += 1;
                }
            }
            if idx > 0 {
                v.shift_right(3 * idx);
            }
            unit = COMPACT_UNITS[idx];
        }
        Notation::Scientific => {
            // Normalize to one non-zero digit before the decimal mark.
            let significant = v.digits.trim_start_matches('0');
            if significant.is_empty() {
                v = Decimal {
                    neg: false,
                    digits: "0".to_string(),
                    scale: 0,
                };
                exp_text = "e+0".to_string();
            } else {
                let leading_zeros = v.digits.len() - significant.len();
                // Exponent of the first significant digit.
                let e = (v.digits.len() as i64 - leading_zeros as i64 - 1) - v.scale as i64;
                v = Decimal {
                    neg: v.neg,
                    digits: significant.to_string(),
                    scale: significant.len() - 1,
                };
                exp_text = format!("e{}{}", if e < 0 { '-' } else { '+' }, e.abs());
            }
        }
    }

    let (neg, scaled) = round_scaled(&v, decimals, rounding);
    let (int, frac) = split_parts(&scaled, decimals);
    // Scientific rounding can carry 9.99 → 10.0; that is left as-is (still a
    // correct rendering of the value) rather than re-normalizing the exponent.
    let grouped = if notation == Notation::Scientific {
        int
    } else {
        group_digits(&int, grouping, gsep)
    };

    let mut digits = grouped;
    if !frac.is_empty() {
        digits.push_str(dsep);
        digits.push_str(&frac);
    }
    digits.push_str(&exp_text);
    digits.push_str(unit);
    if notation == Notation::Percent {
        digits.push('%');
    }

    let is_zero = scaled.bytes().all(|b| b == b'0');
    let body = format!("{prefix}{digits}{suffix}");
    if sign == Sign::Parens {
        return if neg {
            format!("({body})")
        } else {
            body
        };
    }
    let sign_text = if sign == Sign::Never {
        ""
    } else if neg {
        "-"
    } else {
        match sign {
            Sign::Always => "+",
            Sign::ExceptZero if !is_zero => "+",
            Sign::Space => " ",
            _ => "",
        }
    };
    format!("{sign_text}{body}")
}

/// Format a single value with the same rules the table path uses. Exposed so
/// callers (and tests) can format one number without building a CSV.
#[allow(clippy::too_many_arguments)]
pub fn format_value(
    raw: &str,
    decimals: i32,
    rounding: &str,
    notation: &str,
    grouping: &str,
    group_sep: &str,
    decimal_sep: &str,
    sign: &str,
    prefix: &str,
    suffix: &str,
    input_decimal: &str,
) -> Result<String, String> {
    check_decimals(decimals)?;
    let value = parse_decimal(raw, InputDecimal::parse(input_decimal)?)
        .map_err(|e| format!("'{raw}' is not a number ({e})"))?;
    Ok(render(
        &value,
        decimals,
        Rounding::parse(rounding)?,
        Notation::parse(notation)?,
        Grouping::parse(grouping)?,
        group_separator(group_sep)?,
        decimal_separator(decimal_sep)?,
        Sign::parse(sign)?,
        prefix,
        suffix,
    ))
}

fn check_decimals(decimals: i32) -> Result<(), String> {
    if !(MIN_DECIMALS..=MAX_DECIMALS).contains(&decimals) {
        return Err(format!(
            "decimals must be between {MIN_DECIMALS} and {MAX_DECIMALS}, got {decimals}"
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The table entry point
// ---------------------------------------------------------------------------

/// Apply a uniform numeric format to the selected columns of a CSV table.
///
/// * `data` — the CSV/delimited table text (max [`MAX_INPUT_BYTES`]).
/// * `columns` — blank (or `*`) for every column, else names / 1-based indices / `2-4` ranges.
/// * `decimals` — fractional digits to keep; negative rounds to tens/hundreds/… ([`MIN_DECIMALS`]..=[`MAX_DECIMALS`]).
/// * `rounding` — `half_up` (default), `half_down`, `half_even`, `ceil`, `floor`, `truncate`.
/// * `notation` — `standard` (default), `compact`, `scientific`, `percent`.
/// * `grouping` — `none` (default), `thousands`, `indian`.
/// * `group_sep` — `comma` (default), `period`, `space`, `thin_space`, `apostrophe`, `underscore`.
/// * `decimal_sep` — `period` (default) or `comma`.
/// * `sign` — `auto` (default), `always`, `except_zero`, `never`, `space`, `parens`.
/// * `prefix` / `suffix` — text wrapped around each formatted number.
/// * `input_decimal` — `auto` (default), `dot`, `comma`: how the INPUT cells are read.
/// * `non_numeric` — `keep` (default), `blank`, `error`.
/// * `has_header` — treat row 1 as a header (default true); it is never reformatted.
/// * `delimiter` — `auto`, a single character, or comma/tab/semicolon/pipe.
/// * `quote_style_spec` — `minimal` (default), `always`, `non_numeric`.
/// * `output` — `csv` (default), `changed`, `report`.
#[allow(clippy::too_many_arguments)]
pub fn format_columns(
    data: &str,
    columns: &str,
    decimals: i32,
    rounding: &str,
    notation: &str,
    grouping: &str,
    group_sep: &str,
    decimal_sep: &str,
    sign: &str,
    prefix: &str,
    suffix: &str,
    input_decimal: &str,
    non_numeric: &str,
    has_header: bool,
    delimiter: &str,
    quote_style_spec: &str,
    output: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("no CSV data provided".into());
    }
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, over the {MAX_INPUT_BYTES}-byte limit",
            data.len()
        ));
    }
    check_decimals(decimals)?;

    let rounding = Rounding::parse(rounding)?;
    let notation = Notation::parse(notation)?;
    let grouping = Grouping::parse(grouping)?;
    let gsep = group_separator(group_sep)?;
    let dsep = decimal_separator(decimal_sep)?;
    let sign = Sign::parse(sign)?;
    let input_decimal = InputDecimal::parse(input_decimal)?;
    let on_bad = NonNumeric::parse(non_numeric)?;
    let output = Output::parse(output)?;
    let qstyle = quote_style(quote_style_spec)?;
    let delim = if delimiter.trim().eq_ignore_ascii_case("auto") {
        sniff_delimiter(data)
    } else {
        delim_byte(delimiter)?
    };

    let mut rdr = ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .has_headers(false)
        .from_reader(data.as_bytes());

    let mut records: Vec<StringRecord> = Vec::new();
    for rec in rdr.records() {
        records.push(rec.map_err(|e| format!("could not parse the CSV: {e}"))?);
    }
    if records.is_empty() {
        return Err("no CSV rows found".into());
    }

    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);
    let header = if has_header { Some(&records[0]) } else { None };
    let selected = resolve_columns(columns, header, width)?;

    let first_data_row = usize::from(has_header);
    let mut formatted = vec![0usize; width];
    let mut unchanged = vec![0usize; width];
    let mut skipped = vec![0usize; width];
    let mut changed_rows = vec![false; records.len()];

    let mut out_records: Vec<StringRecord> = Vec::with_capacity(records.len());
    for (row, rec) in records.iter().enumerate() {
        if row < first_data_row {
            out_records.push(rec.clone());
            continue;
        }
        let mut fields: Vec<String> = Vec::with_capacity(rec.len());
        for (col, value) in rec.iter().enumerate() {
            if col >= selected.len() || !selected[col] {
                fields.push(value.to_string());
                continue;
            }
            // An empty cell stays empty in every policy — a missing value is not
            // the same as a zero, and inventing "0.00" would be a data change.
            if value.trim().is_empty() {
                fields.push(value.to_string());
                continue;
            }
            match parse_decimal(value, input_decimal) {
                Ok(parsed) => {
                    let new_value = render(
                        &parsed, decimals, rounding, notation, grouping, gsep, dsep, sign, prefix,
                        suffix,
                    );
                    if new_value != value {
                        formatted[col] += 1;
                        changed_rows[row] = true;
                    } else {
                        unchanged[col] += 1;
                    }
                    fields.push(new_value);
                }
                Err(why) => {
                    skipped[col] += 1;
                    match on_bad {
                        NonNumeric::Keep => fields.push(value.to_string()),
                        NonNumeric::Blank => {
                            if !value.is_empty() {
                                changed_rows[row] = true;
                            }
                            fields.push(String::new());
                        }
                        NonNumeric::Error => {
                            let name = column_name(header, col);
                            return Err(format!(
                                "row {} column '{name}': '{value}' is not a number ({why}) — set non_numeric to 'keep' or 'blank' to allow it",
                                row + 1
                            ));
                        }
                    }
                }
            }
        }
        out_records.push(StringRecord::from(fields));
    }

    match output {
        Output::Csv => write_csv(delim, qstyle, out_records.iter()),
        Output::Changed => {
            let mut keep: Vec<&StringRecord> = Vec::new();
            if has_header {
                keep.push(&out_records[0]);
            }
            for (row, rec) in out_records.iter().enumerate() {
                if row == 0 && has_header {
                    continue;
                }
                if changed_rows[row] {
                    keep.push(rec);
                }
            }
            write_csv(delim, qstyle, keep.into_iter())
        }
        Output::Report => {
            let mut rows: Vec<StringRecord> = vec![StringRecord::from(vec![
                "column",
                "cells_formatted",
                "cells_unchanged",
                "non_numeric",
            ])];
            for (col, sel) in selected.iter().enumerate() {
                if !sel {
                    continue;
                }
                rows.push(StringRecord::from(vec![
                    column_name(header, col),
                    formatted[col].to_string(),
                    unchanged[col].to_string(),
                    skipped[col].to_string(),
                ]));
            }
            rows.push(StringRecord::from(vec![
                "TOTAL".to_string(),
                formatted.iter().sum::<usize>().to_string(),
                unchanged.iter().sum::<usize>().to_string(),
                skipped.iter().sum::<usize>().to_string(),
            ]));
            write_csv(delim, qstyle, rows.iter())
        }
    }
}

/// The header name for a column, falling back to its 1-based index.
fn column_name(header: Option<&StringRecord>, col: usize) -> String {
    match header {
        Some(h) if col < h.len() && !h[col].trim().is_empty() => h[col].to_string(),
        _ => (col + 1).to_string(),
    }
}

fn write_csv<'a, I: Iterator<Item = &'a StringRecord>>(
    delim: u8,
    qstyle: QuoteStyle,
    records: I,
) -> Result<String, String> {
    let mut wtr = WriterBuilder::new()
        .delimiter(delim)
        .quote_style(qstyle)
        .flexible(true)
        .from_writer(vec![]);
    for rec in records {
        wtr.write_record(rec)
            .map_err(|e| format!("could not write the CSV: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("could not write the CSV: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("output was not valid UTF-8: {e}"))
}

/// Default entry point: format every column to 2 decimal places, half-up, no
/// grouping, a decimal point, minus signs, non-numeric cells kept as they are.
pub fn run(data: &str) -> Result<String, String> {
    format_columns(
        data, "", 2, "half_up", "standard", "none", "comma", "period", "auto", "", "", "auto",
        "keep", true, "auto", "minimal", "csv",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(raw: &str, decimals: i32) -> String {
        format_value(
            raw, decimals, "half_up", "standard", "none", "comma", "period", "auto", "", "", "auto",
        )
        .unwrap()
    }

    #[test]
    fn formats_a_column_with_the_defaults() {
        let out = format_columns(
            "sku,price\nA1,1234.5\nB2,7\n",
            "price",
            2,
            "half_up",
            "standard",
            "none",
            "comma",
            "period",
            "auto",
            "",
            "",
            "auto",
            "keep",
            true,
            "auto",
            "minimal",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "sku,price\nA1,1234.50\nB2,7.00\n");
    }

    #[test]
    fn rounds_the_digit_string_not_the_float() {
        // 1.005 is not representable in binary; f64 rounding gives 1.00.
        assert_eq!(fmt("1.005", 2), "1.01");
        assert_eq!(fmt("2.675", 2), "2.68");
        assert_eq!(fmt("8.475", 2), "8.48");
    }

    #[test]
    fn covers_every_rounding_mode() {
        let m = |raw: &str, mode: &str| {
            format_value(
                raw, 0, mode, "standard", "none", "comma", "period", "auto", "", "", "auto",
            )
            .unwrap()
        };
        assert_eq!(m("2.5", "half_up"), "3");
        assert_eq!(m("-2.5", "half_up"), "-3");
        assert_eq!(m("2.5", "half_down"), "2");
        assert_eq!(m("2.51", "half_down"), "3");
        assert_eq!(m("2.5", "half_even"), "2");
        assert_eq!(m("1.5", "half_even"), "2");
        assert_eq!(m("2.1", "ceil"), "3");
        assert_eq!(m("-2.9", "ceil"), "-2");
        assert_eq!(m("2.9", "floor"), "2");
        assert_eq!(m("-2.1", "floor"), "-3");
        assert_eq!(m("2.9", "truncate"), "2");
        assert_eq!(m("-2.9", "truncate"), "-2");
    }

    #[test]
    fn negative_decimals_round_to_tens_and_hundreds() {
        assert_eq!(fmt("12345", -2), "12300");
        assert_eq!(fmt("12355", -1), "12360");
        assert_eq!(fmt("499", -3), "0");
        assert_eq!(fmt("500", -3), "1000");
    }

    #[test]
    fn groups_western_and_indian() {
        let g = |raw: &str, style: &str, sep: &str| {
            format_value(
                raw, 2, "half_up", "standard", style, sep, "period", "auto", "", "", "auto",
            )
            .unwrap()
        };
        assert_eq!(g("1234567.891", "thousands", "comma"), "1,234,567.89");
        assert_eq!(g("1234567.891", "indian", "comma"), "12,34,567.89");
        assert_eq!(g("1234567.891", "thousands", "space"), "1 234 567.89");
        assert_eq!(g("1234567.891", "thousands", "apostrophe"), "1'234'567.89");
        assert_eq!(g("999", "indian", "comma"), "999.00");
    }

    #[test]
    fn european_output_and_input_conventions_round_trip() {
        // Read "1.234,56" as European, write it back the same way.
        let out = format_value(
            "1.234,56",
            2,
            "half_up",
            "standard",
            "thousands",
            "period",
            "comma",
            "auto",
            "",
            "",
            "comma",
        )
        .unwrap();
        assert_eq!(out, "1.234,56");
    }

    #[test]
    fn covers_every_notation() {
        let n = |raw: &str, notation: &str, decimals: i32| {
            format_value(
                raw, decimals, "half_up", notation, "none", "comma", "period", "auto", "", "",
                "auto",
            )
            .unwrap()
        };
        assert_eq!(n("1234567", "compact", 2), "1.23M");
        assert_eq!(n("999", "compact", 2), "999.00");
        assert_eq!(n("1500", "compact", 1), "1.5K");
        assert_eq!(n("2500000000", "compact", 2), "2.50B");
        assert_eq!(n("1234567", "scientific", 2), "1.23e+6");
        assert_eq!(n("0.00042", "scientific", 2), "4.20e-4");
        assert_eq!(n("0", "scientific", 2), "0.00e+0");
        assert_eq!(n("0.452", "percent", 1), "45.2%");
        assert_eq!(n("1", "percent", 0), "100%");
    }

    #[test]
    fn covers_every_sign_style() {
        let s = |raw: &str, style: &str| {
            format_value(
                raw, 2, "half_up", "standard", "none", "comma", "period", style, "", "", "auto",
            )
            .unwrap()
        };
        assert_eq!(s("-5", "auto"), "-5.00");
        assert_eq!(s("5", "auto"), "5.00");
        assert_eq!(s("5", "always"), "+5.00");
        assert_eq!(s("0", "always"), "+0.00");
        assert_eq!(s("0", "except_zero"), "0.00");
        assert_eq!(s("5", "except_zero"), "+5.00");
        assert_eq!(s("-5", "never"), "5.00");
        assert_eq!(s("5", "space"), " 5.00");
        assert_eq!(s("-5", "parens"), "(5.00)");
        assert_eq!(s("5", "parens"), "5.00");
        // -0.001 at 2 places is zero, and a zero never carries a minus sign.
        assert_eq!(s("-0.001", "auto"), "0.00");
    }

    #[test]
    fn prefix_and_suffix_wrap_the_number() {
        let out = format_value(
            "1234.5",
            2,
            "half_up",
            "standard",
            "thousands",
            "comma",
            "period",
            "parens",
            "$",
            "",
            "auto",
        )
        .unwrap();
        assert_eq!(out, "$1,234.50");
        let neg = format_value(
            "-1234.5",
            2,
            "half_up",
            "standard",
            "thousands",
            "comma",
            "period",
            "parens",
            "$",
            "",
            "auto",
        )
        .unwrap();
        assert_eq!(neg, "($1,234.50)");
        let unit = format_value(
            "3.14159", 3, "half_up", "standard", "none", "comma", "period", "auto", "", " kg",
            "auto",
        )
        .unwrap();
        assert_eq!(unit, "3.142 kg");
    }

    #[test]
    fn parses_messy_input_forms() {
        assert_eq!(fmt(" 1,234.5 ", 2), "1234.50");
        assert_eq!(fmt("$1,234.50", 2), "1234.50");
        assert_eq!(fmt("1 234.5", 2), "1234.50");
        assert_eq!(fmt("1_234.5", 2), "1234.50");
        assert_eq!(fmt("1'234.5", 2), "1234.50");
        assert_eq!(fmt("(250)", 2), "-250.00");
        assert_eq!(fmt("250-", 2), "-250.00");
        assert_eq!(fmt("\u{2212}12", 2), "-12.00");
        assert_eq!(fmt("1.5e3", 2), "1500.00");
        assert_eq!(fmt("1.5E-3", 4), "0.0015");
        assert_eq!(fmt(".5", 2), "0.50");
        assert_eq!(fmt("45.2%", 1), "45.2");
        assert_eq!(fmt("1.234.567", 0), "1234567");
        assert_eq!(fmt("0,5", 2), "0.50");
        assert_eq!(fmt("1,234", 2), "1234.00");
        assert_eq!(fmt("12 €", 2), "12.00");
    }

    #[test]
    fn rejects_things_that_are_not_numbers() {
        for bad in ["abc", "12abc34", "1.2,3.4", "--", "", "1e", "$"] {
            assert!(
                format_value(
                    bad, 2, "half_up", "standard", "none", "comma", "period", "auto", "", "",
                    "auto",
                )
                .is_err(),
                "expected '{bad}' to be rejected"
            );
        }
    }

    #[test]
    fn non_numeric_policies_keep_blank_or_error() {
        let call = |policy: &str| {
            format_columns(
                "id,amount\n1,10\n2,n/a\n",
                "amount",
                1,
                "half_up",
                "standard",
                "none",
                "comma",
                "period",
                "auto",
                "",
                "",
                "auto",
                policy,
                true,
                "auto",
                "minimal",
                "csv",
            )
        };
        assert_eq!(call("keep").unwrap(), "id,amount\n1,10.0\n2,n/a\n");
        assert_eq!(call("blank").unwrap(), "id,amount\n1,10.0\n2,\n");
        let err = call("error").unwrap_err();
        assert!(err.contains("row 3"), "{err}");
        assert!(err.contains("'n/a' is not a number"), "{err}");
    }

    #[test]
    fn empty_cells_stay_empty_in_every_policy() {
        for policy in ["keep", "blank", "error"] {
            let out = format_columns(
                "id,amount\n1,\n2,3\n",
                "amount",
                2,
                "half_up",
                "standard",
                "none",
                "comma",
                "period",
                "auto",
                "",
                "",
                "auto",
                policy,
                true,
                "auto",
                "minimal",
                "csv",
            )
            .unwrap();
            assert_eq!(out, "id,amount\n1,\n2,3.00\n", "policy {policy}");
        }
    }

    #[test]
    fn header_is_never_reformatted_and_other_columns_pass_through() {
        let out = format_columns(
            "2024,note\n1.5,keep me\n",
            "1",
            1,
            "half_up",
            "standard",
            "none",
            "comma",
            "period",
            "auto",
            "",
            "",
            "auto",
            "keep",
            true,
            "auto",
            "minimal",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "2024,note\n1.5,keep me\n");
    }

    #[test]
    fn headerless_tables_format_every_row() {
        let out = format_columns(
            "1.5,a\n2.5,b\n",
            "1",
            0,
            "half_even",
            "standard",
            "none",
            "comma",
            "period",
            "auto",
            "",
            "",
            "auto",
            "keep",
            false,
            "auto",
            "minimal",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "2,a\n2,b\n");
    }

    #[test]
    fn column_specs_accept_names_indices_and_ranges() {
        let out = format_columns(
            "a,b,c,d\n1,2,3,4\n",
            "a,3-4",
            1,
            "half_up",
            "standard",
            "none",
            "comma",
            "period",
            "auto",
            "",
            "",
            "auto",
            "keep",
            true,
            "auto",
            "minimal",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a,b,c,d\n1.0,2,3.0,4.0\n");
    }

    #[test]
    fn unknown_column_lists_the_available_names() {
        let err = format_columns(
            "a,b\n1,2\n",
            "nope",
            2,
            "half_up",
            "standard",
            "none",
            "comma",
            "period",
            "auto",
            "",
            "",
            "auto",
            "keep",
            true,
            "auto",
            "minimal",
            "csv",
        )
        .unwrap_err();
        assert!(err.contains("no column named 'nope'"), "{err}");
        assert!(err.contains("available: a, b"), "{err}");
    }

    #[test]
    fn changed_and_report_outputs() {
        let table = "id,amount\n1,1.00\n2,2.5\n3,x\n";
        let changed = format_columns(
            table, "amount", 2, "half_up", "standard", "none", "comma", "period", "auto", "", "",
            "auto", "keep", true, "auto", "minimal", "changed",
        )
        .unwrap();
        assert_eq!(changed, "id,amount\n2,2.50\n");
        let report = format_columns(
            table, "amount", 2, "half_up", "standard", "none", "comma", "period", "auto", "", "",
            "auto", "keep", true, "auto", "minimal", "report",
        )
        .unwrap();
        assert_eq!(
            report,
            "column,cells_formatted,cells_unchanged,non_numeric\namount,1,1,1\nTOTAL,1,1,1\n"
        );
    }

    #[test]
    fn grouped_values_are_requoted_so_the_csv_survives() {
        let out = format_columns(
            "id,amount\n1,1234567\n",
            "amount",
            0,
            "half_up",
            "standard",
            "thousands",
            "comma",
            "period",
            "auto",
            "",
            "",
            "auto",
            "keep",
            true,
            "auto",
            "minimal",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "id,amount\n1,\"1,234,567\"\n");
    }

    #[test]
    fn honours_delimiters_and_quote_styles() {
        let tsv = format_columns(
            "a\tb\n1.5\t2\n",
            "a",
            0,
            "half_up",
            "standard",
            "none",
            "comma",
            "period",
            "auto",
            "",
            "",
            "auto",
            "keep",
            true,
            "auto",
            "minimal",
            "csv",
        )
        .unwrap();
        assert_eq!(tsv, "a\tb\n2\t2\n");
        let always = format_columns(
            "a,b\n1.5,x\n",
            "a",
            0,
            "half_up",
            "standard",
            "none",
            "comma",
            "period",
            "auto",
            "",
            "",
            "auto",
            "keep",
            true,
            ",",
            "always",
            "csv",
        )
        .unwrap();
        assert_eq!(always, "\"a\",\"b\"\n\"2\",\"x\"\n");
    }

    #[test]
    fn rejects_out_of_range_decimals_and_unknown_options() {
        let call = |decimals: i32, rounding: &str| {
            format_columns(
                "a\n1\n", "", decimals, rounding, "standard", "none", "comma", "period", "auto",
                "", "", "auto", "keep", true, "auto", "minimal", "csv",
            )
        };
        assert!(call(16, "half_up").unwrap_err().contains("decimals must be"));
        assert!(call(-10, "half_up").unwrap_err().contains("decimals must be"));
        assert!(call(15, "half_up").is_ok());
        assert!(call(-9, "half_up").is_ok());
        assert!(call(2, "nearest").unwrap_err().contains("unknown rounding"));
    }

    #[test]
    fn enforces_the_input_cap_at_the_boundary() {
        let at_cap = format!("a\n{}", "1\n".repeat((MAX_INPUT_BYTES - 2) / 2));
        assert_eq!(at_cap.len(), MAX_INPUT_BYTES);
        assert!(run(&at_cap).is_ok());
        let over_cap = format!("{at_cap}1");
        assert_eq!(over_cap.len(), MAX_INPUT_BYTES + 1);
        assert!(run(&over_cap)
            .unwrap_err()
            .contains("over the 5000000-byte limit"));
    }

    #[test]
    fn rejects_empty_input() {
        assert!(run("   ").unwrap_err().contains("no CSV data provided"));
    }
}
