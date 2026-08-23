//! percent-decimal-converter core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps. Converts the cells of a CSV column
//! between percentage strings (`12.5%`) and decimal fractions (`0.125`), in either
//! direction, optionally auto-detecting the direction per cell.
//!
//! Correctness note: the conversion SHIFTS THE DECIMAL POINT on the cell's digit
//! string; it never multiplies an `f64`. `0.1 * 100.0` is `10.000000000000002` in
//! binary floating point, and `12.345 / 100.0` is `0.12344999999999999` — both would
//! leak into the output. Shifting digits is exact for every input the parser accepts.

use std::collections::HashSet;

/// Largest input accepted, in bytes. Keeps a runaway paste from wedging the
/// browser tab the page runs in.
pub const MAX_INPUT_BYTES: usize = 5_000_000;

/// Which side of the conversion a cell is going to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    /// Decide per cell: a `%`/`‰`/`bp(s)` suffix means "convert down to a decimal
    /// fraction"; a bare number means "convert up to the percent-side unit".
    Auto,
    /// Always divide by the unit scale: `12.5%` → `0.125`.
    PercentToDecimal,
    /// Always multiply by the unit scale: `0.125` → `12.5%`.
    DecimalToPercent,
}

fn parse_direction(s: &str) -> Result<Direction, String> {
    Ok(match s {
        "" | "auto" => Direction::Auto,
        "percent_to_decimal" => Direction::PercentToDecimal,
        "decimal_to_percent" => Direction::DecimalToPercent,
        other => {
            return Err(format!(
                "direction must be one of auto/percent_to_decimal/decimal_to_percent, got '{other}'"
            ))
        }
    })
}

/// The percent-side unit: how many decimal places separate it from a raw fraction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unit {
    /// 1 = 100% — two places.
    Percent,
    /// 1 = 1000‰ — three places.
    PerMille,
    /// 1 = 10000 bps — four places.
    BasisPoints,
}

impl Unit {
    /// Decimal places between the fraction and this unit.
    fn places(self) -> usize {
        match self {
            Unit::Percent => 2,
            Unit::PerMille => 3,
            Unit::BasisPoints => 4,
        }
    }

    /// The symbol appended when `suffix` is on.
    fn symbol(self) -> &'static str {
        match self {
            Unit::Percent => "%",
            Unit::PerMille => "‰",
            Unit::BasisPoints => " bps",
        }
    }
}

fn parse_unit(s: &str) -> Result<Unit, String> {
    Ok(match s {
        "" | "percent" => Unit::Percent,
        "permille" => Unit::PerMille,
        "basis_points" => Unit::BasisPoints,
        other => {
            return Err(format!(
                "unit must be one of percent/permille/basis_points, got '{other}'"
            ))
        }
    })
}

/// Parse a delimiter spec: a single char, or a friendly name.
fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
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
                    "delimiter must be a single char or comma/tab/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// A cell parsed into an exact sign + digit string, plus whether it carried a
/// percent-side unit marker.
struct Parsed {
    neg: bool,
    int_digits: String,
    frac_digits: String,
    /// True when the source cell ended in `%`, `‰`, `bp` or `bps`.
    marked: bool,
}

/// Strip a trailing percent-side marker, returning `(body, was_marked)`.
fn split_marker(s: &str) -> (&str, bool) {
    for m in ["%", "‰"] {
        if let Some(rest) = s.strip_suffix(m) {
            return (rest.trim_end(), true);
        }
    }
    // `bps` before `bp` so the longer match wins.
    let lower = s.to_ascii_lowercase();
    for m in ["bps", "bp"] {
        if lower.ends_with(m) {
            let rest = &s[..s.len() - m.len()];
            // Only a unit marker when a separator or digit precedes it, never a
            // word that merely ends in "bp" (there are none in practice, but a
            // bare "bp" cell has no number and is rejected by the digit check).
            return (rest.trim_end(), true);
        }
    }
    (s, false)
}

/// Split a trimmed cell into an exact sign/digits form if it is a plain decimal
/// number, with optional grouping commas and an optional unit marker. Returns
/// `None` for anything else (scientific notation, currency, text, blanks) so the
/// caller can pass the cell through unchanged.
fn parse_cell(cell: &str) -> Option<Parsed> {
    // Normalise the whitespace competitors' exports leak in: NBSP and narrow NBSP
    // are common in spreadsheet copy-paste ("12,5 %").
    let cleaned: String = cell
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{a0}' && *c != '\u{202f}')
        .collect();
    let (body, marked) = split_marker(&cleaned);
    let body = body.replace(',', ""); // grouping separators: 1,234.5

    let bytes = body.as_bytes();
    let mut i = 0;
    let mut neg = false;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        neg = bytes[i] == b'-';
        i += 1;
    }
    let mut int_digits = String::new();
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        int_digits.push(bytes[i] as char);
        i += 1;
    }
    let mut frac_digits = String::new();
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            frac_digits.push(bytes[i] as char);
            i += 1;
        }
    }
    // Must consume the whole body and have at least one digit somewhere.
    if i != bytes.len() || (int_digits.is_empty() && frac_digits.is_empty()) {
        return None;
    }
    Some(Parsed {
        neg,
        int_digits,
        frac_digits,
        marked,
    })
}

/// Shift the decimal point of `int.frac` by `places` (positive = right/×10ⁿ,
/// negative = left/÷10ⁿ), exactly. Returns `(int_digits, frac_digits)` — both
/// unnormalised digit strings.
fn shift(int_digits: &str, frac_digits: &str, places: i32) -> (String, String) {
    let mut digits = String::with_capacity(int_digits.len() + frac_digits.len());
    digits.push_str(int_digits);
    digits.push_str(frac_digits);
    // Position of the point within `digits`, counted from the left.
    let point = int_digits.len() as i32 + places;

    if point <= 0 {
        // Pad zeros on the left: 0.0…0<digits>
        let pad = "0".repeat((-point) as usize);
        (String::from("0"), format!("{pad}{digits}"))
    } else if point as usize >= digits.len() {
        // Pad zeros on the right: <digits>0…0
        let pad = "0".repeat(point as usize - digits.len());
        (format!("{digits}{pad}"), String::new())
    } else {
        let p = point as usize;
        (digits[..p].to_string(), digits[p..].to_string())
    }
}

/// Increment a magnitude digit string by one ("99" → "100").
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

/// Round `int.frac` half-away-from-zero to `d` places and pad to exactly `d`.
fn round_fixed(int_digits: &str, frac_digits: &str, d: usize) -> (String, String) {
    if frac_digits.len() <= d {
        let mut frac = frac_digits.to_string();
        while frac.len() < d {
            frac.push('0');
        }
        return (strip_leading_zeros(int_digits), frac);
    }
    let mut keep = String::with_capacity(int_digits.len() + d);
    keep.push_str(int_digits);
    keep.push_str(&frac_digits[..d]);
    if frac_digits.as_bytes()[d] >= b'5' {
        keep = inc_digits(&keep);
    }
    if d == 0 {
        (strip_leading_zeros(&keep), String::new())
    } else if keep.len() <= d {
        let padded = "0".repeat(d + 1 - keep.len()) + &keep;
        let split = padded.len() - d;
        (padded[..split].to_string(), padded[split..].to_string())
    } else {
        let split = keep.len() - d;
        (
            strip_leading_zeros(&keep[..split]),
            keep[split..].to_string(),
        )
    }
}

/// Assemble the final numeric text, dropping a `-` that would render `-0`.
fn assemble(neg: bool, int_digits: &str, frac_digits: &str) -> String {
    let int_part = strip_leading_zeros(int_digits);
    let is_zero = int_part.bytes().all(|b| b == b'0') && frac_digits.bytes().all(|b| b == b'0');
    let mut out = String::new();
    if neg && !is_zero {
        out.push('-');
    }
    out.push_str(&int_part);
    if !frac_digits.is_empty() {
        out.push('.');
        out.push_str(frac_digits);
    }
    out
}

/// Convert one cell. Returns `None` when the cell is not a number (the caller
/// leaves it untouched).
fn convert_cell(
    cell: &str,
    direction: Direction,
    unit: Unit,
    decimals: i64,
    trim_zeros: bool,
    suffix: bool,
) -> Option<String> {
    let p = parse_cell(cell)?;
    let to_decimal = match direction {
        Direction::PercentToDecimal => true,
        Direction::DecimalToPercent => false,
        // Auto: a marked cell is already on the percent side, so bring it down.
        Direction::Auto => p.marked,
    };
    let places = unit.places() as i32;
    let (int_digits, mut frac_digits) = shift(
        &p.int_digits,
        &p.frac_digits,
        if to_decimal { -places } else { places },
    );
    let mut int_digits = int_digits;

    if decimals >= 0 {
        let (i, f) = round_fixed(&int_digits, &frac_digits, decimals as usize);
        int_digits = i;
        frac_digits = f;
    }
    // Exact mode always trims (the shift itself introduces the zeros); a fixed
    // width trims only when asked, so `decimals` reads as "at most this many".
    if decimals < 0 || trim_zeros {
        while frac_digits.ends_with('0') {
            frac_digits.pop();
        }
    }

    let mut out = assemble(p.neg, &int_digits, &frac_digits);
    if !to_decimal && suffix {
        out.push_str(unit.symbol());
    }
    Some(out)
}

/// Convert the selected columns of a CSV between percentage strings and decimal
/// fractions.
///
/// * `direction` — `auto` (per cell, by the `%`/`‰`/`bps` marker),
///   `percent_to_decimal`, or `decimal_to_percent`.
/// * `unit` — the percent-side unit: `percent` (÷100), `permille` (÷1000) or
///   `basis_points` (÷10000).
/// * `columns` — comma-separated 1-based indices and/or header names to convert;
///   empty = every numeric cell in every column.
/// * `header` — treat the first row as a header (never converted; enables name refs).
/// * `delimiter` — field separator (char or comma/tab/semicolon/pipe).
/// * `decimals` — decimal places to keep, 0–12, padded with zeros; `-1` keeps the
///   exact value with no padding.
/// * `trim_zeros` — drop trailing fractional zeros after rounding, turning
///   `decimals` into an "at most" width. Always on in exact mode.
/// * `suffix` — append the unit symbol when converting to the percent side.
#[allow(clippy::too_many_arguments)]
pub fn convert_csv(
    data: &str,
    direction: &str,
    unit: &str,
    columns: &str,
    header: bool,
    delimiter: &str,
    decimals: i64,
    trim_zeros: bool,
    suffix: bool,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {MAX_INPUT_BYTES} bytes",
            data.len()
        ));
    }
    if !(-1..=12).contains(&decimals) {
        return Err(format!(
            "decimals must be between -1 (exact) and 12, got {decimals}"
        ));
    }
    let direction = parse_direction(direction)?;
    let unit = parse_unit(unit)?;
    let delim = delim_byte(delimiter)?;

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr
        .records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Ok(String::new());
    }

    // Resolve which column indices to convert from the header (or first row).
    let ref_row = &records[0];
    let width = ref_row.len();
    let selected: Option<HashSet<usize>> = if columns.trim().is_empty() {
        None // all columns
    } else {
        let mut set = HashSet::new();
        for tok in columns.split(',') {
            let name = tok.trim();
            if name.is_empty() {
                continue;
            }
            if name.chars().all(|c| c.is_ascii_digit()) {
                let idx: usize = name
                    .parse()
                    .map_err(|_| format!("invalid column index '{name}'"))?;
                if idx == 0 || idx > width {
                    return Err(format!(
                        "column index {idx} is out of range; the file has {width} column(s)"
                    ));
                }
                set.insert(idx - 1);
            } else if header {
                match ref_row.iter().position(|h| h.trim() == name) {
                    Some(i) => {
                        set.insert(i);
                    }
                    None => return Err(format!("column '{name}' not found in the header row")),
                }
            } else {
                return Err(format!(
                    "column '{name}' is a name, but header is off — use 1-based indices instead"
                ));
            }
        }
        if set.is_empty() {
            return Err("no columns selected".into());
        }
        Some(set)
    };

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);

    for (i, rec) in records.iter().enumerate() {
        let is_header = header && i == 0;
        let fields: Vec<String> = rec
            .iter()
            .enumerate()
            .map(|(col, cell)| {
                if is_header {
                    return cell.to_string();
                }
                let convert_this = match &selected {
                    Some(set) => set.contains(&col),
                    None => true,
                };
                if convert_this {
                    convert_cell(cell, direction, unit, decimals, trim_zeros, suffix)
                        .unwrap_or_else(|| cell.to_string())
                } else {
                    cell.to_string()
                }
            })
            .collect();
        wtr.write_record(&fields)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }

    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Exact mode, percent unit, symbol on — the shorthand most cases want.
    fn cc(cell: &str, dir: Direction) -> String {
        convert_cell(cell, dir, Unit::Percent, -1, false, true).unwrap()
    }

    #[test]
    fn percent_to_decimal_is_exact() {
        assert_eq!(cc("12.5%", Direction::PercentToDecimal), "0.125");
        // 12.345 / 100.0 in f64 is 0.12344999999999999 — the digit shift is exact.
        assert_eq!(cc("12.345%", Direction::PercentToDecimal), "0.12345");
        assert_eq!(cc("3.25%", Direction::PercentToDecimal), "0.0325");
        assert_eq!(cc("150%", Direction::PercentToDecimal), "1.5");
        assert_eq!(cc("100", Direction::PercentToDecimal), "1");
        assert_eq!(cc("0.5%", Direction::PercentToDecimal), "0.005");
    }

    #[test]
    fn decimal_to_percent_is_exact() {
        // 0.1 * 100.0 in f64 is 10.000000000000002.
        assert_eq!(cc("0.1", Direction::DecimalToPercent), "10%");
        assert_eq!(cc("0.125", Direction::DecimalToPercent), "12.5%");
        assert_eq!(cc("1", Direction::DecimalToPercent), "100%");
        assert_eq!(cc("0.0325", Direction::DecimalToPercent), "3.25%");
        assert_eq!(cc(".5", Direction::DecimalToPercent), "50%");
    }

    #[test]
    fn auto_direction_uses_the_marker() {
        // Marked → down to a fraction; bare → up to a percentage.
        assert_eq!(cc("12.5%", Direction::Auto), "0.125");
        assert_eq!(cc("0.125", Direction::Auto), "12.5%");
        // A space before the sign, and a NBSP, are both tolerated.
        assert_eq!(cc("12.5 %", Direction::Auto), "0.125");
        assert_eq!(cc("12.5\u{a0}%", Direction::Auto), "0.125");
    }

    #[test]
    fn negatives_and_zero() {
        assert_eq!(cc("-12.5%", Direction::PercentToDecimal), "-0.125");
        assert_eq!(cc("-0.125", Direction::DecimalToPercent), "-12.5%");
        assert_eq!(cc("0%", Direction::PercentToDecimal), "0");
        // A tiny negative rounded away never renders as "-0".
        assert_eq!(
            convert_cell(
                "-0.004%",
                Direction::PercentToDecimal,
                Unit::Percent,
                2,
                false,
                true
            )
            .unwrap(),
            "0.00"
        );
    }

    #[test]
    fn units_permille_and_basis_points() {
        assert_eq!(
            convert_cell(
                "125‰",
                Direction::PercentToDecimal,
                Unit::PerMille,
                -1,
                false,
                true
            )
            .unwrap(),
            "0.125"
        );
        assert_eq!(
            convert_cell(
                "0.125",
                Direction::DecimalToPercent,
                Unit::PerMille,
                -1,
                false,
                true
            )
            .unwrap(),
            "125‰"
        );
        assert_eq!(
            convert_cell(
                "25bps",
                Direction::PercentToDecimal,
                Unit::BasisPoints,
                -1,
                false,
                true
            )
            .unwrap(),
            "0.0025"
        );
        assert_eq!(
            convert_cell(
                "0.0025",
                Direction::DecimalToPercent,
                Unit::BasisPoints,
                -1,
                false,
                true
            )
            .unwrap(),
            "25 bps"
        );
        // `BPS` uppercase is accepted too.
        assert_eq!(
            convert_cell("25 BPS", Direction::Auto, Unit::BasisPoints, -1, false, true).unwrap(),
            "0.0025"
        );
    }

    #[test]
    fn decimals_rounds_and_pads() {
        // 1/3 as a percentage, to 2 places.
        assert_eq!(
            convert_cell(
                "0.333333",
                Direction::DecimalToPercent,
                Unit::Percent,
                2,
                false,
                true
            )
            .unwrap(),
            "33.33%"
        );
        // Half rounds away from zero.
        assert_eq!(
            convert_cell(
                "0.005",
                Direction::DecimalToPercent,
                Unit::Percent,
                0,
                false,
                true
            )
            .unwrap(),
            "1%"
        );
        // Short values are padded to the requested width.
        assert_eq!(
            convert_cell(
                "50%",
                Direction::PercentToDecimal,
                Unit::Percent,
                4,
                false,
                true
            )
            .unwrap(),
            "0.5000"
        );
        // Carry across the point.
        assert_eq!(
            convert_cell(
                "99.96%",
                Direction::PercentToDecimal,
                Unit::Percent,
                3,
                false,
                true
            )
            .unwrap(),
            "1.000"
        );
    }

    #[test]
    fn trim_zeros_turns_decimals_into_an_upper_bound() {
        // Rounded to at most 4 places, then the padding zeros come back off.
        assert_eq!(
            convert_cell(
                "50%",
                Direction::PercentToDecimal,
                Unit::Percent,
                4,
                true,
                true
            )
            .unwrap(),
            "0.5"
        );
        // Rounding still happens first: 33.3333% → 0.3333 → trimmed is unchanged.
        assert_eq!(
            convert_cell(
                "33.3333%",
                Direction::PercentToDecimal,
                Unit::Percent,
                4,
                true,
                true
            )
            .unwrap(),
            "0.3333"
        );
        // A value that rounds to a whole number loses the point entirely.
        assert_eq!(
            convert_cell(
                "0.999996",
                Direction::DecimalToPercent,
                Unit::Percent,
                3,
                true,
                true
            )
            .unwrap(),
            "100%"
        );
    }

    #[test]
    fn suffix_can_be_turned_off() {
        assert_eq!(
            convert_cell(
                "0.125",
                Direction::DecimalToPercent,
                Unit::Percent,
                -1,
                false,
                false
            )
            .unwrap(),
            "12.5"
        );
    }

    #[test]
    fn non_numeric_cells_pass_through() {
        assert!(convert_cell("hello", Direction::Auto, Unit::Percent, -1, false, true).is_none());
        assert!(convert_cell("$12.99", Direction::Auto, Unit::Percent, -1, false, true).is_none());
        assert!(convert_cell("", Direction::Auto, Unit::Percent, -1, false, true).is_none());
        // Scientific notation is not a plain decimal — left alone rather than
        // silently mis-shifted.
        assert!(convert_cell("1e5", Direction::Auto, Unit::Percent, -1, false, true).is_none());
        // A bare unit marker with no number.
        assert!(convert_cell("%", Direction::Auto, Unit::Percent, -1, false, true).is_none());
    }

    #[test]
    fn grouping_separators_survive() {
        // A quoted CSV field can hold "1,234.5%".
        assert_eq!(cc("1,234.5%", Direction::PercentToDecimal), "12.345");
    }

    #[test]
    fn one_value_per_line_list() {
        // The commonest shape: a bare list pasted out of a spreadsheet column.
        let got = convert_csv(
            "12.5%\n7%\n0.5%",
            "percent_to_decimal",
            "percent",
            "",
            false,
            ",",
            -1,
            false,
            true,
        )
        .unwrap();
        assert_eq!(got, "0.125\n0.07\n0.005\n");
    }

    #[test]
    fn crlf_newlines_are_accepted() {
        let got = convert_csv(
            "rate\r\n12.5%\r\n7%\r\n",
            "percent_to_decimal",
            "percent",
            "",
            true,
            ",",
            -1,
            false,
            true,
        )
        .unwrap();
        assert_eq!(got, "rate\n0.125\n0.07\n");
    }

    #[test]
    fn csv_auto_all_columns() {
        let d = "name,rate,share\nApple,12.5%,0.25\nPear,7%,0.5";
        let got = convert_csv(d, "auto", "percent", "", true, ",", -1, false, true).unwrap();
        assert_eq!(got, "name,rate,share\nApple,0.125,25%\nPear,0.07,50%\n");
    }

    #[test]
    fn csv_selected_column_by_name_and_index() {
        let d = "name,rate,qty\nApple,12.5%,3\nPear,7.25%,10";
        let by_name = convert_csv(
            d,
            "percent_to_decimal",
            "percent",
            "rate",
            true,
            ",",
            -1,
            false,
            true,
        )
        .unwrap();
        assert_eq!(by_name, "name,rate,qty\nApple,0.125,3\nPear,0.0725,10\n");
        let by_index = convert_csv(
            d,
            "percent_to_decimal",
            "percent",
            "2",
            true,
            ",",
            -1,
            false,
            true,
        )
        .unwrap();
        assert_eq!(by_index, by_name);
    }

    #[test]
    fn csv_no_header_and_delimiter() {
        let d = "0.125;0.5\n0.075;1";
        let got = convert_csv(
            d,
            "decimal_to_percent",
            "percent",
            "",
            false,
            "semicolon",
            -1,
            false,
            true,
        )
        .unwrap();
        assert_eq!(got, "12.5%;50%\n7.5%;100%\n");
    }

    #[test]
    fn csv_header_row_is_never_converted() {
        // A header cell that looks numeric stays put.
        let d = "2024,2025\n0.1,0.2";
        let got = convert_csv(
            d,
            "decimal_to_percent",
            "percent",
            "",
            true,
            ",",
            -1,
            false,
            true,
        )
        .unwrap();
        assert_eq!(got, "2024,2025\n10%,20%\n");
    }

    #[test]
    fn errors() {
        // Empty input.
        assert!(convert_csv("  ", "auto", "percent", "", true, ",", -1, false, true).is_err());
        // Unknown direction / unit / delimiter.
        assert!(
            convert_csv("a\n1", "sideways", "percent", "", true, ",", -1, false, true).is_err()
        );
        assert!(convert_csv("a\n1", "auto", "furlongs", "", true, ",", -1, false, true).is_err());
        assert!(convert_csv("a\n1", "auto", "percent", "", true, "nope", -1, false, true).is_err());
        // The old value names are gone — the error names the accepted set.
        let e = convert_csv("a\n1", "to_decimal", "percent", "", true, ",", -1, false, true)
            .unwrap_err();
        assert!(e.contains("percent_to_decimal"), "{e}");
        // decimals out of range.
        assert!(convert_csv("a\n1", "auto", "percent", "", true, ",", 99, false, true).is_err());
        assert!(convert_csv("a\n1", "auto", "percent", "", true, ",", -2, false, true).is_err());
        // Column selection problems.
        assert!(
            convert_csv("a,b\n1,2", "auto", "percent", "nope", true, ",", -1, false, true).is_err()
        );
        assert!(
            convert_csv("a,b\n1,2", "auto", "percent", "5", true, ",", -1, false, true).is_err()
        );
        assert!(
            convert_csv("a,b\n1,2", "auto", "percent", "a", false, ",", -1, false, true).is_err()
        );
    }

    #[test]
    fn oversize_input_is_rejected() {
        let big = "1\n".repeat(MAX_INPUT_BYTES / 2 + 1);
        assert!(big.len() > MAX_INPUT_BYTES);
        let e = convert_csv(&big, "auto", "percent", "", false, ",", -1, false, true).unwrap_err();
        assert!(e.contains("the limit is"), "{e}");
    }
}
