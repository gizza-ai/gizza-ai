//! round-decimals core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps. Rounds the numeric cells of a CSV to a chosen
//! number of decimal places using a selectable rounding mode.
//!
//! Correctness note: plain decimal cells are rounded on their DIGIT STRING, not on
//! the binary `f64`, so `1.005` at 2 places is `1.01` (a naive `x*100` round returns
//! `1.00` because `1.005` isn't representable in binary). Only scientific-notation
//! cells (`1e5`) fall back to `f64` rounding; genuinely non-numeric cells (currency
//! symbols, text) are left unchanged.

use std::collections::HashSet;

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

fn parse_mode(s: &str) -> Result<RoundMode, String> {
    Ok(match s {
        "" | "half_up" => RoundMode::HalfUp,
        "half_down" => RoundMode::HalfDown,
        "half_even" => RoundMode::HalfEven,
        "ceil" => RoundMode::Ceil,
        "floor" => RoundMode::Floor,
        "truncate" => RoundMode::Truncate,
        other => {
            return Err(format!(
                "mode must be one of half_up/half_down/half_even/ceil/floor/truncate, got '{other}'"
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

/// Split a trimmed cell into `(negative, integer_digits, fraction_digits)` if it is
/// a plain decimal number (optional sign, digits, optional single `.`, digits).
/// Returns `None` for anything else (scientific notation, currency, text, blanks).
fn parse_decimal(s: &str) -> Option<(bool, String, String)> {
    let bytes = s.as_bytes();
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
    // Must consume the whole string and have at least one digit somewhere.
    if i != bytes.len() || (int_digits.is_empty() && frac_digits.is_empty()) {
        return None;
    }
    Some((neg, int_digits, frac_digits))
}

/// Increment a magnitude digit string by one (may grow by a digit: "99" → "100").
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

/// Round a single numeric cell. `d` = decimal places (already validated 0..=10).
/// Returns `None` when the cell is not a number (left unchanged by the caller).
fn round_cell(cell: &str, d: usize, mode: RoundMode, trailing_zeros: bool) -> Option<String> {
    let trimmed = cell.trim();
    if let Some((neg, int_digits, frac_digits)) = parse_decimal(trimmed) {
        Some(round_decimal(neg, &int_digits, &frac_digits, d, mode, trailing_zeros))
    } else {
        // Fallback: scientific notation and other f64-parseable forms.
        let v: f64 = trimmed.parse().ok()?;
        if !v.is_finite() {
            return None;
        }
        Some(round_via_f64(v, d, mode, trailing_zeros))
    }
}

fn round_decimal(
    neg: bool,
    int_digits: &str,
    frac_digits: &str,
    d: usize,
    mode: RoundMode,
    trailing_zeros: bool,
) -> String {
    let (out_int, mut out_frac) = if frac_digits.len() <= d {
        // Nothing to round off — keep the value, pad/trim happens below.
        (strip_leading_zeros(int_digits), frac_digits.to_string())
    } else {
        let kept_frac = &frac_digits[..d];
        let dropped = &frac_digits[d..];
        let first = dropped.as_bytes()[0] - b'0';
        let rest_nonzero = dropped.as_bytes()[1..].iter().any(|&b| b != b'0');
        let any_dropped = first != 0 || rest_nonzero;

        let mut keep_digits = String::with_capacity(int_digits.len() + d);
        keep_digits.push_str(int_digits);
        keep_digits.push_str(kept_frac);
        let last_kept_odd = keep_digits
            .bytes()
            .last()
            .map(|b| (b - b'0') % 2 == 1)
            .unwrap_or(false);

        let away = match mode {
            RoundMode::HalfUp => first >= 5,
            RoundMode::HalfDown => first > 5 || (first == 5 && rest_nonzero),
            RoundMode::HalfEven => {
                if first > 5 {
                    true
                } else if first < 5 {
                    false
                } else {
                    rest_nonzero || last_kept_odd
                }
            }
            RoundMode::Ceil => !neg && any_dropped,
            RoundMode::Floor => neg && any_dropped,
            RoundMode::Truncate => false,
        };
        if away {
            keep_digits = inc_digits(&keep_digits);
        }

        if d == 0 {
            (strip_leading_zeros(&keep_digits), String::new())
        } else if keep_digits.len() <= d {
            let padded = "0".repeat(d + 1 - keep_digits.len()) + &keep_digits;
            let split = padded.len() - d;
            (padded[..split].to_string(), padded[split..].to_string())
        } else {
            let split = keep_digits.len() - d;
            (
                strip_leading_zeros(&keep_digits[..split]),
                keep_digits[split..].to_string(),
            )
        }
    };

    if trailing_zeros {
        while out_frac.len() < d {
            out_frac.push('0');
        }
    } else {
        while out_frac.ends_with('0') {
            out_frac.pop();
        }
    }

    let is_zero =
        out_int.bytes().all(|b| b == b'0') && out_frac.bytes().all(|b| b == b'0');
    let mut result = String::new();
    if neg && !is_zero {
        result.push('-');
    }
    result.push_str(&out_int);
    if !out_frac.is_empty() {
        result.push('.');
        result.push_str(&out_frac);
    }
    result
}

fn round_via_f64(v: f64, d: usize, mode: RoundMode, trailing_zeros: bool) -> String {
    let scale = 10f64.powi(d as i32);
    let y = v * scale;
    let r = match mode {
        RoundMode::HalfUp => y.round(), // Rust's round() is half-away-from-zero
        RoundMode::HalfDown => {
            let t = y.trunc();
            if (y - t).abs() > 0.5 {
                y.round()
            } else {
                t
            }
        }
        RoundMode::HalfEven => y.round_ties_even(),
        RoundMode::Ceil => y.ceil(),
        RoundMode::Floor => y.floor(),
        RoundMode::Truncate => y.trunc(),
    };
    let mut out = r / scale;
    if out == 0.0 {
        out = 0.0; // normalize -0.0 → 0.0
    }
    let mut s = format!("{:.*}", d, out);
    if !trailing_zeros && s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

/// Round the selected numeric columns of a CSV.
///
/// * `decimals` — decimal places to keep (0..=10).
/// * `mode` — `half_up`/`half_down`/`half_even`/`ceil`/`floor`/`truncate`.
/// * `columns` — comma-separated 1-based indices and/or header names to round;
///   empty = every numeric cell in every column.
/// * `header` — treat the first row as a header (never rounded; enables name refs).
/// * `delimiter` — field separator (char or comma/tab/semicolon/pipe).
/// * `trailing_zeros` — pad every rounded cell to exactly `decimals` places (3 → 3.00).
#[allow(clippy::too_many_arguments)]
pub fn round_csv(
    data: &str,
    decimals: i64,
    mode: &str,
    columns: &str,
    header: bool,
    delimiter: &str,
    trailing_zeros: bool,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    if !(0..=10).contains(&decimals) {
        return Err(format!("decimals must be between 0 and 10, got {decimals}"));
    }
    let d = decimals as usize;
    let mode = parse_mode(mode)?;
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

    // Resolve which column indices to round from the header (or first row).
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
                let round_this = match &selected {
                    Some(set) => set.contains(&col),
                    None => true,
                };
                if round_this {
                    match round_cell(cell, d, mode, trailing_zeros) {
                        Some(r) => r,
                        None => cell.to_string(),
                    }
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

    fn rc(cell: &str, d: usize, mode: RoundMode) -> String {
        round_cell(cell, d, mode, false).unwrap()
    }

    #[test]
    fn float_representation_is_correct() {
        // The classic trap: 1.005 * 100 == 100.4999… so a naive round gives 1.00.
        assert_eq!(rc("1.005", 2, RoundMode::HalfUp), "1.01");
        assert_eq!(rc("2.675", 2, RoundMode::HalfUp), "2.68");
        assert_eq!(rc("0.1", 1, RoundMode::HalfUp), "0.1");
    }

    #[test]
    fn rounding_modes() {
        assert_eq!(rc("2.5", 0, RoundMode::HalfUp), "3");
        assert_eq!(rc("-2.5", 0, RoundMode::HalfUp), "-3");
        assert_eq!(rc("2.5", 0, RoundMode::HalfDown), "2");
        assert_eq!(rc("2.51", 0, RoundMode::HalfDown), "3");
        assert_eq!(rc("2.5", 0, RoundMode::HalfEven), "2");
        assert_eq!(rc("3.5", 0, RoundMode::HalfEven), "4");
        assert_eq!(rc("2.1", 0, RoundMode::Ceil), "3");
        assert_eq!(rc("-2.9", 0, RoundMode::Ceil), "-2");
        assert_eq!(rc("2.9", 0, RoundMode::Floor), "2");
        assert_eq!(rc("-2.1", 0, RoundMode::Floor), "-3");
        assert_eq!(rc("2.99", 0, RoundMode::Truncate), "2");
        assert_eq!(rc("-2.99", 0, RoundMode::Truncate), "-2");
    }

    #[test]
    fn carry_and_negative_zero() {
        assert_eq!(rc("9.99", 1, RoundMode::HalfUp), "10");
        assert_eq!(rc("99.95", 1, RoundMode::HalfUp), "100");
        // -0.001 rounds to a bare 0, never "-0".
        assert_eq!(rc("-0.001", 2, RoundMode::HalfUp), "0");
        assert_eq!(rc(".5", 0, RoundMode::HalfUp), "1");
    }

    #[test]
    fn trailing_zeros_padding() {
        assert_eq!(round_cell("3", 2, RoundMode::HalfUp, true).unwrap(), "3.00");
        assert_eq!(round_cell("3.5", 2, RoundMode::HalfUp, true).unwrap(), "3.50");
        assert_eq!(
            round_cell("2.675", 2, RoundMode::HalfUp, true).unwrap(),
            "2.68"
        );
        assert_eq!(round_cell("3", 0, RoundMode::HalfUp, true).unwrap(), "3");
    }

    #[test]
    fn non_numeric_cells_pass_through() {
        assert_eq!(round_cell("$12.99", 1, RoundMode::HalfUp, false), None);
        assert_eq!(round_cell("hello", 1, RoundMode::HalfUp, false), None);
        assert_eq!(round_cell("", 1, RoundMode::HalfUp, false), None);
    }

    #[test]
    fn scientific_fallback() {
        // 1.5e1 = 15.0 → f64 path.
        assert_eq!(rc("1.5e1", 0, RoundMode::HalfUp), "15");
    }

    #[test]
    fn csv_all_columns_default() {
        let d = "name,price,qty\nApple,1.005,3.14159\nPear,2.5,10";
        let got = round_csv(d, 2, "half_up", "", true, ",", false).unwrap();
        assert_eq!(got, "name,price,qty\nApple,1.01,3.14\nPear,2.5,10\n");
    }

    #[test]
    fn csv_selected_columns_by_name_and_index() {
        let d = "name,price,qty\nApple,1.005,3.14159\nPear,2.555,10";
        // Round only "price" (col 2) → qty stays untouched.
        let got = round_csv(d, 2, "half_up", "price", true, ",", true).unwrap();
        assert_eq!(got, "name,price,qty\nApple,1.01,3.14159\nPear,2.56,10\n");
        // Same via 1-based index.
        let got2 = round_csv(d, 2, "half_up", "2", true, ",", true).unwrap();
        assert_eq!(got2, got);
    }

    #[test]
    fn csv_no_header_and_delimiter() {
        let d = "1.005;2.5\n9.99;0.1";
        // 1.005→1.0 shown as "1" (trailing zero trimmed); 9.99→10.0 shown as "10".
        let got = round_csv(d, 1, "half_up", "", false, "semicolon", false).unwrap();
        assert_eq!(got, "1;2.5\n10;0.1\n");
        // With trailing_zeros, the same input keeps the fixed width.
        let padded = round_csv(d, 1, "half_up", "", false, "semicolon", true).unwrap();
        assert_eq!(padded, "1.0;2.5\n10.0;0.1\n");
    }

    #[test]
    fn errors() {
        assert!(round_csv("  ", 2, "half_up", "", true, ",", false).is_err()); // empty
        assert!(round_csv("a,b\n1,2", 99, "half_up", "", true, ",", false).is_err()); // decimals range
        assert!(round_csv("a,b\n1,2", 2, "bogus", "", true, ",", false).is_err()); // bad mode
        assert!(round_csv("a,b\n1,2", 2, "half_up", "nope", true, ",", false).is_err()); // bad name
        assert!(round_csv("a,b\n1,2", 2, "half_up", "5", true, ",", false).is_err()); // index oob
        assert!(round_csv("a,b\n1,2", 2, "half_up", "name", false, ",", false).is_err()); // name w/o header
        assert!(round_csv("a,b\n1,2", 2, "half_up", "", true, "nope", false).is_err()); // bad delim
    }
}
