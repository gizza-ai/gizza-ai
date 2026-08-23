//! human-readable-units core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Turns a raw magnitude into the string a person actually reads: a byte count
//! into `1.5 GB`, a duration into `2m 3s`, a plain count into `1.2M`. The three
//! families share one pipeline — parse the value, convert it to a base unit
//! (bytes / seconds / plain), scale it, render it in the requested style — so
//! `precision`, `style` and `trim_zeros` behave the same everywhere.

/// Largest magnitude (in base units: bytes, seconds, or plain count) the
/// formatter accepts. Beyond this an `f64` no longer resolves whole units and
/// the "human readable" answer would be a lie.
pub const MAX_MAGNITUDE: f64 = 1e21;
/// Maximum decimal places for the scaled value.
pub const MAX_PRECISION: u32 = 6;
/// Maximum number of segments in a compound duration (`1d 2h 3m 4s …`).
pub const MAX_UNITS: u32 = 7;

/// What the raw value counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A data size, rendered as B / kB / MB / GB …
    Bytes,
    /// A length of time, rendered as a compound `1h 2m 3s`.
    Duration,
    /// A plain count, rendered with a K/M/B/T suffix.
    Number,
}

/// The unit the raw value is expressed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputUnit {
    Auto,
    Byte,
    Bit,
    Kilobyte,
    Megabyte,
    Gigabyte,
    Nanosecond,
    Microsecond,
    Millisecond,
    Second,
    Minute,
    Hour,
    Day,
}

/// Which power and which suffix family the byte scale uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base {
    /// 1000-based with SI suffixes: kB, MB, GB — what drive vendors and
    /// macOS report.
    Decimal,
    /// 1024-based with IEC suffixes: KiB, MiB, GiB — unambiguous.
    BinaryIec,
    /// 1024-based but labelled kB/MB/GB — what Windows and `ls -h` show.
    BinaryJedec,
}

/// How much of the unit name to spell out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Symbol with the conventional spacing: `1.5 GB`, `2m 3s`, `1.2M`.
    Short,
    /// Spelled out: `1.5 gigabytes`, `2 minutes 3 seconds`, `1.2 million`.
    Long,
    /// No spaces at all: `1.5GB`, `2m3s`, `1.2M`.
    Narrow,
}

/// One value, or every equivalent representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detail {
    /// Just the formatted string.
    Value,
    /// The formatted string plus the exact amount and the alternate formats.
    Breakdown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub kind: Kind,
    pub input_unit: InputUnit,
    pub base: Base,
    pub style: Style,
    pub precision: u32,
    pub max_units: u32,
    pub trim_zeros: bool,
    pub detail: Detail,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            kind: Kind::Bytes,
            input_unit: InputUnit::Auto,
            base: Base::Decimal,
            style: Style::Short,
            precision: 1,
            max_units: 2,
            trim_zeros: true,
            detail: Detail::Value,
        }
    }
}

pub fn parse_kind(v: &str) -> Result<Kind, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" | "bytes" | "byte" | "size" | "data" => Ok(Kind::Bytes),
        "duration" | "time" => Ok(Kind::Duration),
        "number" | "count" => Ok(Kind::Number),
        other => Err(format!(
            "invalid kind '{other}': expected one of bytes, duration, number"
        )),
    }
}

pub fn parse_base(v: &str) -> Result<Base, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" | "decimal" => Ok(Base::Decimal),
        "binary-iec" => Ok(Base::BinaryIec),
        "binary-jedec" => Ok(Base::BinaryJedec),
        other => Err(format!(
            "invalid base '{other}': expected one of decimal, binary-iec, binary-jedec"
        )),
    }
}

pub fn parse_style(v: &str) -> Result<Style, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" | "short" => Ok(Style::Short),
        "long" => Ok(Style::Long),
        "narrow" => Ok(Style::Narrow),
        other => Err(format!(
            "invalid style '{other}': expected one of short, long, narrow"
        )),
    }
}

pub fn parse_detail(v: &str) -> Result<Detail, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" | "value" => Ok(Detail::Value),
        "breakdown" => Ok(Detail::Breakdown),
        other => Err(format!(
            "invalid detail '{other}': expected one of value, breakdown"
        )),
    }
}

const ALL_INPUT_UNITS: &str = "auto, byte, bit, kilobyte, megabyte, gigabyte, \
nanosecond, microsecond, millisecond, second, minute, hour, day";

pub fn parse_input_unit(v: &str) -> Result<InputUnit, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(InputUnit::Auto),
        "byte" | "bytes" | "b" => Ok(InputUnit::Byte),
        "bit" | "bits" => Ok(InputUnit::Bit),
        "kilobyte" | "kilobytes" | "kb" => Ok(InputUnit::Kilobyte),
        "megabyte" | "megabytes" | "mb" => Ok(InputUnit::Megabyte),
        "gigabyte" | "gigabytes" | "gb" => Ok(InputUnit::Gigabyte),
        "nanosecond" | "nanoseconds" | "ns" => Ok(InputUnit::Nanosecond),
        "microsecond" | "microseconds" | "us" => Ok(InputUnit::Microsecond),
        "millisecond" | "milliseconds" | "ms" => Ok(InputUnit::Millisecond),
        "second" | "seconds" | "s" => Ok(InputUnit::Second),
        "minute" | "minutes" | "min" => Ok(InputUnit::Minute),
        "hour" | "hours" | "h" => Ok(InputUnit::Hour),
        "day" | "days" | "d" => Ok(InputUnit::Day),
        other => Err(format!(
            "invalid input_unit '{other}': expected one of {ALL_INPUT_UNITS}"
        )),
    }
}

// ---------------------------------------------------------------- value parse

/// Parse the raw magnitude. Digit-group separators (`_`, `,`, spaces) are
/// stripped so a value pasted out of a log or a spreadsheet works as-is.
pub fn parse_value(raw: &str) -> Result<f64, String> {
    let cleaned: String = raw
        .chars()
        .filter(|c| !matches!(c, '_' | ',' | ' ' | '\t' | '\n' | '\r'))
        .collect();
    if cleaned.is_empty() {
        return Err("value is empty: enter a number, for example 1500000000".into());
    }
    if !cleaned
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | 'e' | 'E'))
    {
        return Err(format!(
            "value must be a number, got '{}': accepted forms are plain digits (1500000000), \
grouped digits (1,500,000,000 or 1_500_000_000) and scientific notation (1.5e9)",
            raw.trim()
        ));
    }
    let v: f64 = cleaned.parse().map_err(|_| {
        format!(
            "value must be a number, got '{}': accepted forms are plain digits (1500000000), \
grouped digits (1,500,000,000 or 1_500_000_000) and scientific notation (1.5e9)",
            raw.trim()
        )
    })?;
    if !v.is_finite() {
        return Err(format!(
            "value must be a finite number, got '{}'",
            raw.trim()
        ));
    }
    Ok(v)
}

/// Convert the raw value from `input_unit` into the family's base unit
/// (bytes, seconds, or a plain count).
fn to_base_amount(value: f64, kind: Kind, unit: InputUnit) -> Result<f64, String> {
    use InputUnit::*;
    let amount = match kind {
        Kind::Bytes => match unit {
            Auto | Byte => value,
            Bit => value / 8.0,
            Kilobyte => value * 1e3,
            Megabyte => value * 1e6,
            Gigabyte => value * 1e9,
            _ => {
                return Err(format!(
                    "input_unit '{}' is a time unit but kind is 'bytes': expected one of \
auto, byte, bit, kilobyte, megabyte, gigabyte",
                    unit_name(unit)
                ))
            }
        },
        Kind::Duration => match unit {
            Auto | Second => value,
            Nanosecond => value * 1e-9,
            Microsecond => value * 1e-6,
            Millisecond => value * 1e-3,
            Minute => value * 60.0,
            Hour => value * 3600.0,
            Day => value * 86_400.0,
            _ => {
                return Err(format!(
                    "input_unit '{}' is a data unit but kind is 'duration': expected one of \
auto, nanosecond, microsecond, millisecond, second, minute, hour, day",
                    unit_name(unit)
                ))
            }
        },
        Kind::Number => match unit {
            Auto => value,
            _ => {
                return Err(format!(
                    "input_unit '{}' does not apply to kind 'number': expected auto",
                    unit_name(unit)
                ))
            }
        },
    };
    if !amount.is_finite() || amount.abs() > MAX_MAGNITUDE {
        return Err(format!(
            "value out of range: {} {} exceeds the maximum magnitude of 1e21 {}",
            format_number(amount, 0, true),
            base_unit_name(kind),
            base_unit_name(kind)
        ));
    }
    Ok(amount)
}

fn unit_name(u: InputUnit) -> &'static str {
    use InputUnit::*;
    match u {
        Auto => "auto",
        Byte => "byte",
        Bit => "bit",
        Kilobyte => "kilobyte",
        Megabyte => "megabyte",
        Gigabyte => "gigabyte",
        Nanosecond => "nanosecond",
        Microsecond => "microsecond",
        Millisecond => "millisecond",
        Second => "second",
        Minute => "minute",
        Hour => "hour",
        Day => "day",
    }
}

fn base_unit_name(k: Kind) -> &'static str {
    match k {
        Kind::Bytes => "bytes",
        Kind::Duration => "seconds",
        Kind::Number => "units",
    }
}

// ------------------------------------------------------------- number helpers

/// Format `v` with `precision` decimals, optionally dropping trailing zeros.
pub fn format_number(v: f64, precision: u32, trim_zeros: bool) -> String {
    let mut s = format!("{:.*}", precision as usize, v);
    if trim_zeros && s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

/// Insert thousands separators into an already-formatted decimal string.
fn group_digits(s: &str) -> String {
    let (sign, rest) = match s.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", s),
    };
    let (int_part, frac_part) = match rest.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (rest, None),
    };
    let bytes = int_part.as_bytes();
    let mut out = String::with_capacity(int_part.len() + int_part.len() / 3 + 2);
    out.push_str(sign);
    for (i, c) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*c as char);
    }
    if let Some(f) = frac_part {
        out.push('.');
        out.push_str(f);
    }
    out
}

/// Scale `a` (already non-negative) down by `div` until it fits the largest
/// suffix, returning the rendered mantissa and the suffix index. Re-scales once
/// more when rounding pushes the mantissa back up to a full `div` (999_999_999
/// bytes at 1 decimal is `1 GB`, never `1000.0 MB`).
fn scale(a: f64, div: f64, max_idx: usize, precision: u32, trim_zeros: bool) -> (String, usize) {
    let mut a = a;
    let mut idx = 0usize;
    while a >= div && idx < max_idx {
        a /= div;
        idx += 1;
    }
    let mut s = format_number(a, precision, trim_zeros);
    if idx < max_idx {
        if let Ok(rendered) = s.replace(',', "").parse::<f64>() {
            if rendered >= div {
                a /= div;
                idx += 1;
                s = format_number(a, precision, trim_zeros);
            }
        }
    }
    (s, idx)
}

fn plural<'a>(mantissa: &str, singular: &'a str, plural_form: &'a str) -> &'a str {
    if mantissa == "1" || mantissa == "-1" {
        singular
    } else {
        plural_form
    }
}

// ---------------------------------------------------------------------- bytes

const DECIMAL_SYMS: [&str; 8] = ["B", "kB", "MB", "GB", "TB", "PB", "EB", "ZB"];
const IEC_SYMS: [&str; 8] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB", "ZiB"];
const JEDEC_SYMS: [&str; 8] = ["B", "KB", "MB", "GB", "TB", "PB", "EB", "ZB"];
const DECIMAL_WORDS: [(&str, &str); 8] = [
    ("byte", "bytes"),
    ("kilobyte", "kilobytes"),
    ("megabyte", "megabytes"),
    ("gigabyte", "gigabytes"),
    ("terabyte", "terabytes"),
    ("petabyte", "petabytes"),
    ("exabyte", "exabytes"),
    ("zettabyte", "zettabytes"),
];
const IEC_WORDS: [(&str, &str); 8] = [
    ("byte", "bytes"),
    ("kibibyte", "kibibytes"),
    ("mebibyte", "mebibytes"),
    ("gibibyte", "gibibytes"),
    ("tebibyte", "tebibytes"),
    ("pebibyte", "pebibytes"),
    ("exbibyte", "exbibytes"),
    ("zebibyte", "zebibytes"),
];
const BIT_SYMS: [&str; 8] = [
    "bit", "kbit", "Mbit", "Gbit", "Tbit", "Pbit", "Ebit", "Zbit",
];

/// Render a byte count as a scaled, suffixed string.
pub fn format_bytes(bytes: f64, base: Base, style: Style, precision: u32, trim_zeros: bool) -> String {
    let sign = if bytes < 0.0 { "-" } else { "" };
    let div = match base {
        Base::Decimal => 1000.0,
        Base::BinaryIec | Base::BinaryJedec => 1024.0,
    };
    let syms = match base {
        Base::Decimal => &DECIMAL_SYMS,
        Base::BinaryIec => &IEC_SYMS,
        Base::BinaryJedec => &JEDEC_SYMS,
    };
    let words = match base {
        Base::BinaryIec => &IEC_WORDS,
        _ => &DECIMAL_WORDS,
    };
    let (mantissa, idx) = scale(bytes.abs(), div, syms.len() - 1, precision, trim_zeros);
    match style {
        Style::Short => format!("{sign}{mantissa} {}", syms[idx]),
        Style::Narrow => format!("{sign}{mantissa}{}", syms[idx]),
        Style::Long => {
            let (s1, s2) = words[idx];
            format!("{sign}{mantissa} {}", plural(&mantissa, s1, s2))
        }
    }
}

/// Render a byte count as bits (always decimal-scaled — network rates are SI).
fn format_bits(bytes: f64, precision: u32, trim_zeros: bool) -> String {
    let bits = bytes * 8.0;
    let sign = if bits < 0.0 { "-" } else { "" };
    let (mantissa, idx) = scale(bits.abs(), 1000.0, BIT_SYMS.len() - 1, precision, trim_zeros);
    format!("{sign}{mantissa} {}", BIT_SYMS[idx])
}

// ------------------------------------------------------------------- duration

const NS_PER_DAY: i128 = 86_400_000_000_000;
const NS_PER_HOUR: i128 = 3_600_000_000_000;
const NS_PER_MINUTE: i128 = 60_000_000_000;
const NS_PER_SECOND: i128 = 1_000_000_000;

/// Duration segments, largest first: (nanoseconds, symbol, singular, plural).
const DURATION_UNITS: [(i128, &str, &str, &str); 7] = [
    (NS_PER_DAY, "d", "day", "days"),
    (NS_PER_HOUR, "h", "hour", "hours"),
    (NS_PER_MINUTE, "m", "minute", "minutes"),
    (NS_PER_SECOND, "s", "second", "seconds"),
    (1_000_000, "ms", "millisecond", "milliseconds"),
    (1_000, "\u{b5}s", "microsecond", "microseconds"),
    (1, "ns", "nanosecond", "nanoseconds"),
];

fn seconds_to_ns(secs: f64) -> i128 {
    (secs.abs() * 1e9).round() as i128
}

/// Render a duration as a compound `1h 2m 3s`, capped at `max_units` segments.
/// Segments are truncated, never rounded up, and zero segments are skipped.
pub fn format_duration(secs: f64, style: Style, max_units: u32) -> String {
    let sign = if secs < 0.0 { "-" } else { "" };
    let total = seconds_to_ns(secs);
    if total == 0 {
        return match style {
            Style::Long => "0 seconds".to_string(),
            _ => "0s".to_string(),
        };
    }
    let mut rem = total;
    let mut segments: Vec<String> = Vec::new();
    for (ns, sym, s1, s2) in DURATION_UNITS.iter() {
        if segments.len() as u32 >= max_units || rem == 0 {
            break;
        }
        let amount = rem / ns;
        if amount == 0 {
            continue;
        }
        rem -= amount * ns;
        segments.push(match style {
            Style::Long => format!("{amount} {}", if amount == 1 { s1 } else { s2 }),
            _ => format!("{amount}{sym}"),
        });
    }
    let joined = match style {
        Style::Narrow => segments.join(""),
        _ => segments.join(" "),
    };
    format!("{sign}{joined}")
}

/// `HH:MM:SS`, prefixed with `D:` once the duration passes a day.
fn format_clock(secs: f64) -> String {
    let sign = if secs < 0.0 { "-" } else { "" };
    let total = seconds_to_ns(secs);
    let days = total / NS_PER_DAY;
    let h = (total % NS_PER_DAY) / NS_PER_HOUR;
    let m = (total % NS_PER_HOUR) / NS_PER_MINUTE;
    let s = (total % NS_PER_MINUTE) / NS_PER_SECOND;
    if days > 0 {
        format!("{sign}{days}:{h:02}:{m:02}:{s:02}")
    } else {
        format!("{sign}{h:02}:{m:02}:{s:02}")
    }
}

/// ISO 8601 duration, e.g. `PT2M3S` or `P1DT2H30M`.
fn format_iso8601(secs: f64) -> String {
    let sign = if secs < 0.0 { "-" } else { "" };
    let total = seconds_to_ns(secs);
    if total == 0 {
        return "PT0S".to_string();
    }
    let days = total / NS_PER_DAY;
    let h = (total % NS_PER_DAY) / NS_PER_HOUR;
    let m = (total % NS_PER_HOUR) / NS_PER_MINUTE;
    let s = (total % NS_PER_MINUTE) / NS_PER_SECOND;
    let frac = total % NS_PER_SECOND;
    let mut out = String::from("P");
    if days > 0 {
        out.push_str(&format!("{days}D"));
    }
    let mut t = String::new();
    if h > 0 {
        t.push_str(&format!("{h}H"));
    }
    if m > 0 {
        t.push_str(&format!("{m}M"));
    }
    if s > 0 || frac > 0 {
        if frac > 0 {
            let mut f = format!("{frac:09}");
            while f.ends_with('0') {
                f.pop();
            }
            t.push_str(&format!("{s}.{f}S"));
        } else {
            t.push_str(&format!("{s}S"));
        }
    }
    if !t.is_empty() {
        out.push('T');
        out.push_str(&t);
    }
    format!("{sign}{out}")
}

// --------------------------------------------------------------- large number

const NUMBER_SYMS: [&str; 5] = ["", "K", "M", "B", "T"];
const NUMBER_WORDS: [&str; 5] = ["", "thousand", "million", "billion", "trillion"];

/// Render a plain count with a K/M/B/T suffix (always 1000-based).
pub fn format_count(value: f64, style: Style, precision: u32, trim_zeros: bool) -> String {
    let sign = if value < 0.0 { "-" } else { "" };
    let (mantissa, idx) = scale(
        value.abs(),
        1000.0,
        NUMBER_SYMS.len() - 1,
        precision,
        trim_zeros,
    );
    if idx == 0 {
        return format!("{sign}{mantissa}");
    }
    match style {
        Style::Long => format!("{sign}{mantissa} {}", NUMBER_WORDS[idx]),
        _ => format!("{sign}{mantissa}{}", NUMBER_SYMS[idx]),
    }
}

// ------------------------------------------------------------------ top level

/// Format `value` according to `opts`. This is the single entry point every
/// surface (chat block, CLI, page) calls.
pub fn format(value: &str, opts: &Options) -> Result<String, String> {
    if opts.precision > MAX_PRECISION {
        return Err(format!(
            "precision must be between 0 and {MAX_PRECISION}, got {}",
            opts.precision
        ));
    }
    if opts.max_units < 1 || opts.max_units > MAX_UNITS {
        return Err(format!(
            "max_units must be between 1 and {MAX_UNITS}, got {}",
            opts.max_units
        ));
    }
    let raw = parse_value(value)?;
    let amount = to_base_amount(raw, opts.kind, opts.input_unit)?;

    let primary = match opts.kind {
        Kind::Bytes => format_bytes(amount, opts.base, opts.style, opts.precision, opts.trim_zeros),
        Kind::Duration => format_duration(amount, opts.style, opts.max_units),
        Kind::Number => format_count(amount, opts.style, opts.precision, opts.trim_zeros),
    };
    if opts.detail == Detail::Value {
        return Ok(primary);
    }

    let mut lines = vec![format!("Formatted: {primary}")];
    match opts.kind {
        Kind::Bytes => {
            let exact = group_digits(&format_number(amount, 6, true));
            let unit = plural(&exact, "byte", "bytes");
            lines.push(format!("Exact: {exact} {unit}"));
            lines.push(format!(
                "Decimal (1000): {}",
                format_bytes(amount, Base::Decimal, Style::Short, opts.precision, opts.trim_zeros)
            ));
            lines.push(format!(
                "Binary (1024): {}",
                format_bytes(
                    amount,
                    Base::BinaryIec,
                    Style::Short,
                    opts.precision,
                    opts.trim_zeros
                )
            ));
            lines.push(format!(
                "Bits: {}",
                format_bits(amount, opts.precision, opts.trim_zeros)
            ));
        }
        Kind::Duration => {
            let exact = group_digits(&format_number(amount, 9, true));
            let unit = plural(&exact, "second", "seconds");
            lines.push(format!("Exact: {exact} {unit}"));
            lines.push(format!(
                "Short: {}",
                format_duration(amount, Style::Short, opts.max_units)
            ));
            lines.push(format!(
                "Long: {}",
                format_duration(amount, Style::Long, opts.max_units)
            ));
            lines.push(format!("Clock: {}", format_clock(amount)));
            lines.push(format!("ISO 8601: {}", format_iso8601(amount)));
        }
        Kind::Number => {
            lines.push(format!(
                "Exact: {}",
                group_digits(&format_number(amount, 6, true))
            ));
            lines.push(format!(
                "Short: {}",
                format_count(amount, Style::Short, opts.precision, opts.trim_zeros)
            ));
            lines.push(format!(
                "Long: {}",
                format_count(amount, Style::Long, opts.precision, opts.trim_zeros)
            ));
            lines.push(format!("Scientific: {:e}", amount));
        }
    }
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(kind: Kind) -> Options {
        Options {
            kind,
            ..Options::default()
        }
    }

    #[test]
    fn the_three_headline_examples_render() {
        assert_eq!(format("1500000000", &opts(Kind::Bytes)).unwrap(), "1.5 GB");
        assert_eq!(format("123", &opts(Kind::Duration)).unwrap(), "2m 3s");
        assert_eq!(format("1200000", &opts(Kind::Number)).unwrap(), "1.2M");
    }

    #[test]
    fn an_empty_value_is_rejected_with_an_example() {
        let e = format("", &opts(Kind::Bytes)).unwrap_err();
        assert!(e.contains("value is empty"), "{e}");
        assert!(e.contains("1500000000"), "{e}");
    }

    #[test]
    fn a_non_numeric_value_names_the_accepted_forms() {
        let e = format("12 GB", &opts(Kind::Bytes)).unwrap_err();
        assert!(e.contains("must be a number"), "{e}");
        assert!(e.contains("scientific notation"), "{e}");
    }

    #[test]
    fn grouped_and_scientific_inputs_parse() {
        assert_eq!(parse_value("1,500,000,000").unwrap(), 1.5e9);
        assert_eq!(parse_value("1_500_000_000").unwrap(), 1.5e9);
        assert_eq!(parse_value("1.5e9").unwrap(), 1.5e9);
        assert_eq!(parse_value(" -42 ").unwrap(), -42.0);
    }

    #[test]
    fn byte_bases_use_their_own_divisor_and_suffixes() {
        let v = "1073741824";
        let mut o = opts(Kind::Bytes);
        assert_eq!(format(v, &o).unwrap(), "1.1 GB");
        o.base = Base::BinaryIec;
        assert_eq!(format(v, &o).unwrap(), "1 GiB");
        o.base = Base::BinaryJedec;
        assert_eq!(format(v, &o).unwrap(), "1 GB");
    }

    #[test]
    fn byte_styles_differ_in_spacing_and_words() {
        let mut o = opts(Kind::Bytes);
        assert_eq!(format("1500000000", &o).unwrap(), "1.5 GB");
        o.style = Style::Narrow;
        assert_eq!(format("1500000000", &o).unwrap(), "1.5GB");
        o.style = Style::Long;
        assert_eq!(format("1500000000", &o).unwrap(), "1.5 gigabytes");
        assert_eq!(format("1000000000", &o).unwrap(), "1 gigabyte");
    }

    #[test]
    fn rounding_at_a_boundary_promotes_to_the_next_unit() {
        // 999,999,999 B is 1000.0 MB at one decimal — it must read as 1 GB.
        assert_eq!(format("999999999", &opts(Kind::Bytes)).unwrap(), "1 GB");
    }

    #[test]
    fn trim_zeros_off_keeps_the_fixed_decimals() {
        let mut o = opts(Kind::Bytes);
        o.trim_zeros = false;
        assert_eq!(format("1000000000", &o).unwrap(), "1.0 GB");
        o.precision = 3;
        assert_eq!(format("1000000000", &o).unwrap(), "1.000 GB");
    }

    #[test]
    fn zero_and_small_byte_counts_stay_in_bytes() {
        assert_eq!(format("0", &opts(Kind::Bytes)).unwrap(), "0 B");
        assert_eq!(format("999", &opts(Kind::Bytes)).unwrap(), "999 B");
    }

    #[test]
    fn negative_values_keep_their_sign() {
        assert_eq!(format("-1500000000", &opts(Kind::Bytes)).unwrap(), "-1.5 GB");
        assert_eq!(format("-123", &opts(Kind::Duration)).unwrap(), "-2m 3s");
        assert_eq!(format("-1200000", &opts(Kind::Number)).unwrap(), "-1.2M");
    }

    #[test]
    fn input_units_convert_before_scaling() {
        let mut o = opts(Kind::Bytes);
        o.input_unit = InputUnit::Megabyte;
        assert_eq!(format("1500", &o).unwrap(), "1.5 GB");
        o.input_unit = InputUnit::Bit;
        assert_eq!(format("8000", &o).unwrap(), "1 kB");

        let mut d = opts(Kind::Duration);
        d.input_unit = InputUnit::Millisecond;
        assert_eq!(format("11722000", &d).unwrap(), "3h 15m");
        d.input_unit = InputUnit::Hour;
        assert_eq!(format("1.5", &d).unwrap(), "1h 30m");
    }

    #[test]
    fn a_mismatched_input_unit_names_the_valid_set() {
        let mut o = opts(Kind::Bytes);
        o.input_unit = InputUnit::Second;
        let e = format("10", &o).unwrap_err();
        assert!(e.contains("is a time unit but kind is 'bytes'"), "{e}");

        let mut d = opts(Kind::Duration);
        d.input_unit = InputUnit::Megabyte;
        let e = format("10", &d).unwrap_err();
        assert!(e.contains("is a data unit but kind is 'duration'"), "{e}");

        let mut n = opts(Kind::Number);
        n.input_unit = InputUnit::Byte;
        let e = format("10", &n).unwrap_err();
        assert!(e.contains("does not apply to kind 'number'"), "{e}");
    }

    #[test]
    fn durations_truncate_and_skip_empty_segments() {
        let mut o = opts(Kind::Duration);
        assert_eq!(format("3661", &o).unwrap(), "1h 1m"); // 1s dropped by max_units
        assert_eq!(format("3601", &o).unwrap(), "1h 1s"); // zero minutes skipped
        o.max_units = 1;
        assert_eq!(format("90", &o).unwrap(), "1m"); // truncated, not rounded to 2m
        o.max_units = 4;
        assert_eq!(format("90061", &o).unwrap(), "1d 1h 1m 1s");
    }

    #[test]
    fn sub_second_durations_descend_to_ms_us_ns() {
        let mut o = opts(Kind::Duration);
        o.input_unit = InputUnit::Millisecond;
        assert_eq!(format("2.5", &o).unwrap(), "2ms 500\u{b5}s");
        o.input_unit = InputUnit::Nanosecond;
        assert_eq!(format("1500", &o).unwrap(), "1\u{b5}s 500ns");
        assert_eq!(format("0", &o).unwrap(), "0s");
    }

    #[test]
    fn duration_styles_differ_in_spacing_and_words() {
        let mut o = opts(Kind::Duration);
        o.style = Style::Narrow;
        assert_eq!(format("123", &o).unwrap(), "2m3s");
        o.style = Style::Long;
        assert_eq!(format("123", &o).unwrap(), "2 minutes 3 seconds");
        assert_eq!(format("61", &o).unwrap(), "1 minute 1 second");
    }

    #[test]
    fn counts_scale_through_k_m_b_t() {
        let o = opts(Kind::Number);
        assert_eq!(format("999", &o).unwrap(), "999");
        assert_eq!(format("1200", &o).unwrap(), "1.2K");
        assert_eq!(format("1200000", &o).unwrap(), "1.2M");
        assert_eq!(format("1200000000", &o).unwrap(), "1.2B");
        assert_eq!(format("1200000000000", &o).unwrap(), "1.2T");
        // Past a trillion the suffix stays T rather than inventing one.
        assert_eq!(format("1.2e15", &o).unwrap(), "1200T");
    }

    #[test]
    fn count_long_style_spells_the_scale_word() {
        let mut o = opts(Kind::Number);
        o.style = Style::Long;
        assert_eq!(format("1200000", &o).unwrap(), "1.2 million");
        assert_eq!(format("999", &o).unwrap(), "999");
    }

    #[test]
    fn breakdown_lists_every_equivalent_form() {
        let mut o = opts(Kind::Bytes);
        o.detail = Detail::Breakdown;
        assert_eq!(
            format("1500000000", &o).unwrap(),
            "Formatted: 1.5 GB\n\
             Exact: 1,500,000,000 bytes\n\
             Decimal (1000): 1.5 GB\n\
             Binary (1024): 1.4 GiB\n\
             Bits: 12 Gbit"
        );

        let mut d = opts(Kind::Duration);
        d.detail = Detail::Breakdown;
        assert_eq!(
            format("11722", &d).unwrap(),
            "Formatted: 3h 15m\n\
             Exact: 11,722 seconds\n\
             Short: 3h 15m\n\
             Long: 3 hours 15 minutes\n\
             Clock: 03:15:22\n\
             ISO 8601: PT3H15M22S"
        );

        let mut n = opts(Kind::Number);
        n.detail = Detail::Breakdown;
        assert_eq!(
            format("1200000", &n).unwrap(),
            "Formatted: 1.2M\n\
             Exact: 1,200,000\n\
             Short: 1.2M\n\
             Long: 1.2 million\n\
             Scientific: 1.2e6"
        );
    }

    #[test]
    fn iso_and_clock_handle_days_zero_and_fractions() {
        assert_eq!(format_iso8601(0.0), "PT0S");
        assert_eq!(format_clock(0.0), "00:00:00");
        assert_eq!(format_iso8601(90_061.0), "P1DT1H1M1S");
        assert_eq!(format_clock(90_061.0), "1:01:01:01");
        assert_eq!(format_iso8601(1.5), "PT1.5S");
    }

    #[test]
    fn the_magnitude_cap_is_enforced_at_the_boundary() {
        let o = opts(Kind::Bytes);
        assert!(format("1e21", &o).is_ok(), "1e21 bytes is the cap and works");
        let e = format("2e21", &o).unwrap_err();
        assert!(e.contains("value out of range"), "{e}");
        assert!(e.contains("1e21"), "{e}");
    }

    #[test]
    fn out_of_range_precision_and_max_units_are_rejected() {
        let mut o = opts(Kind::Bytes);
        o.precision = 7;
        let e = format("1000", &o).unwrap_err();
        assert!(e.contains("precision must be between 0 and 6"), "{e}");

        let mut d = opts(Kind::Duration);
        d.max_units = 0;
        let e = format("100", &d).unwrap_err();
        assert!(e.contains("max_units must be between 1 and 7"), "{e}");
    }

    #[test]
    fn enum_parsers_reject_unknown_values_by_name() {
        assert!(parse_kind("bits").unwrap_err().contains("invalid kind"));
        assert!(parse_base("iec").unwrap_err().contains("invalid base"));
        assert!(parse_style("tiny").unwrap_err().contains("invalid style"));
        assert!(parse_detail("all").unwrap_err().contains("invalid detail"));
        assert!(parse_input_unit("furlong")
            .unwrap_err()
            .contains("invalid input_unit"));
        // Blank means "use the default" on every surface.
        assert_eq!(parse_kind("").unwrap(), Kind::Bytes);
        assert_eq!(parse_base("").unwrap(), Base::Decimal);
        assert_eq!(parse_style("").unwrap(), Style::Short);
        assert_eq!(parse_detail("").unwrap(), Detail::Value);
        assert_eq!(parse_input_unit("").unwrap(), InputUnit::Auto);
    }
}
