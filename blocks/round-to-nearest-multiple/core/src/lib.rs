//! round-to-nearest-multiple core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps. Rounds each numeric cell of a CSV to the nearest
//! multiple of a chosen step (e.g. 0.25, 5, 1000) using a selectable rounding mode.
//!
//! Correctness note: the quotient `value / step` is computed with EXACT integer arithmetic on
//! the decimal digits you typed (both value and step are scaled to a common power of ten), so a
//! value that lands exactly halfway between two multiples is broken by the chosen tie rule
//! deterministically — e.g. `0.125` to the nearest `0.05` with half_up gives `0.15`, not a
//! binary-float artifact. Only inputs that overflow 128-bit integers or aren't plain decimals
//! (scientific notation like `1.5e3`) fall back to `f64` rounding; non-numeric cells (text,
//! currency symbols, blanks) are left unchanged.

use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoundMode {
    /// Nearest multiple; a halfway value rounds away from zero (classical / MROUND). 2.5→3, -2.5→-3.
    HalfUp,
    /// Nearest multiple; a halfway value rounds toward zero. 2.5→2, -2.5→-2.
    HalfDown,
    /// Nearest multiple; a halfway value rounds to the even multiple (banker's). 0.5→0, 1.5→2, 2.5→2.
    HalfEven,
    /// Always round UP to the next multiple (toward +∞). 2.1→3, -2.9→-2.
    Ceil,
    /// Always round DOWN to the previous multiple (toward -∞). 2.9→2, -2.1→-3.
    Floor,
    /// Always round toward zero (drop toward the lower-magnitude multiple). 2.9→2, -2.9→-2.
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

/// A non-negative decimal magnitude with a sign: value = (neg ? -1 : 1) * scaled / 10^decimals.
#[derive(Clone, Copy, Debug)]
struct Decimal {
    neg: bool,
    scaled: i128,
    decimals: u32,
}

/// Parse a trimmed string as a plain decimal (optional sign, digits, optional single `.`, digits)
/// into an exact `Decimal`. Returns `None` for anything else (scientific notation, currency, text,
/// blanks) or on 128-bit overflow (too many digits) — the caller then falls back to `f64`.
fn parse_decimal(s: &str) -> Option<Decimal> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut neg = false;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        neg = bytes[i] == b'-';
        i += 1;
    }
    let start_digits = i;
    let mut digits = String::new();
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        digits.push(bytes[i] as char);
        i += 1;
    }
    let mut decimals: u32 = 0;
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            digits.push(bytes[i] as char);
            decimals += 1;
            i += 1;
        }
    }
    // Must consume the whole string and have at least one digit somewhere.
    if i != bytes.len() || i == start_digits {
        return None;
    }
    let scaled: i128 = digits.parse().ok()?; // magnitude; None on overflow
    Some(Decimal {
        neg: neg && scaled != 0,
        scaled,
        decimals,
    })
}

/// The validated step to round to: its exact decimal form (when representable) plus its f64 value
/// and decimal-place count (always known), used for output precision and the f64 fallback.
struct StepSpec {
    scaled: Option<i128>, // magnitude / 10^decimals; None if it overflowed i128
    decimals: u32,
    value: f64,
}

fn step_spec(step: f64) -> Result<StepSpec, String> {
    if !step.is_finite() {
        return Err("step must be a finite number".into());
    }
    if step <= 0.0 {
        return Err(format!("step must be greater than 0, got {step}"));
    }
    // Rust's shortest Display never uses exponent notation, so this round-trips small/large
    // steps like 0.05 or 1000 exactly for parse_decimal.
    let s = format!("{step}");
    match parse_decimal(&s) {
        Some(d) => Ok(StepSpec {
            scaled: Some(d.scaled),
            decimals: d.decimals,
            value: step,
        }),
        // Unreachable for a finite positive f64, but stay safe: fall back to f64 with 0 decimals.
        None => Ok(StepSpec {
            scaled: None,
            decimals: 0,
            value: step,
        }),
    }
}

fn pow10(n: u32) -> Option<i128> {
    10i128.checked_pow(n)
}

/// Format a signed `scaled` magnitude at `decimals` places into a plain decimal string,
/// trimming trailing zeros unless `trailing_zeros`. `-0` never appears.
fn format_scaled(rs: i128, decimals: u32, trailing_zeros: bool) -> String {
    let neg = rs < 0;
    let mag = rs.unsigned_abs();
    let mut int_part;
    let mut frac_part = String::new();
    if decimals == 0 {
        int_part = mag.to_string();
    } else {
        let divisor = 10u128.pow(decimals);
        int_part = (mag / divisor).to_string();
        frac_part = format!("{:0width$}", mag % divisor, width = decimals as usize);
    }
    if !trailing_zeros {
        while frac_part.ends_with('0') {
            frac_part.pop();
        }
    }
    if int_part.is_empty() {
        int_part.push('0');
    }
    let is_zero = mag == 0;
    let mut out = String::new();
    if neg && !is_zero {
        out.push('-');
    }
    out.push_str(&int_part);
    if !frac_part.is_empty() {
        out.push('.');
        out.push_str(&frac_part);
    }
    out
}

/// Exact path: round `v` to the nearest multiple of `step` using 128-bit integers.
/// Returns `None` on any overflow so the caller can fall back to `f64`.
fn round_exact(
    v: Decimal,
    step_scaled: i128,
    step_decimals: u32,
    mode: RoundMode,
    trailing_zeros: bool,
) -> Option<String> {
    let dv = v.decimals;
    let ds = step_decimals;
    let c = dv.max(ds);
    // Scale value and step to the common decimal count `c`.
    let value_mag = v.scaled.checked_mul(pow10(c - dv)?)?;
    let a: i128 = if v.neg { value_mag.checked_neg()? } else { value_mag };
    let b: i128 = step_scaled.checked_mul(pow10(c - ds)?)?; // > 0

    // Floor division: fl = floor(a / b), rem in [0, b).
    let fl = a.div_euclid(b);
    let rem = a - fl.checked_mul(b)?;

    let n: i128 = match mode {
        RoundMode::Floor => fl,
        RoundMode::Ceil => {
            if rem == 0 {
                fl
            } else {
                fl.checked_add(1)?
            }
        }
        RoundMode::Truncate => a / b, // truncation toward zero
        RoundMode::HalfUp | RoundMode::HalfDown | RoundMode::HalfEven => {
            let twice = rem.checked_mul(2)?;
            if twice < b {
                fl
            } else if twice > b {
                fl.checked_add(1)?
            } else {
                // Exact halfway between fl and fl+1.
                match mode {
                    RoundMode::HalfUp => {
                        if a > 0 {
                            fl.checked_add(1)?
                        } else {
                            fl
                        }
                    }
                    RoundMode::HalfDown => {
                        if a > 0 {
                            fl
                        } else {
                            fl.checked_add(1)?
                        }
                    }
                    // banker's: pick the even multiple count.
                    _ => {
                        if fl.rem_euclid(2) == 0 {
                            fl
                        } else {
                            fl.checked_add(1)?
                        }
                    }
                }
            }
        }
    };

    // result = n * step = (n * step_scaled) / 10^step_decimals.
    let rs = n.checked_mul(step_scaled)?;
    Some(format_scaled(rs, ds, trailing_zeros))
}

/// f64 fallback for scientific-notation / very-large cells.
fn round_f64(v: f64, step: &StepSpec, mode: RoundMode, trailing_zeros: bool) -> String {
    let q = v / step.value;
    let n = match mode {
        RoundMode::HalfUp => q.round(), // Rust round() is half-away-from-zero
        RoundMode::HalfDown => {
            let t = q.trunc();
            if (q - t).abs() > 0.5 {
                q.round()
            } else {
                t
            }
        }
        RoundMode::HalfEven => q.round_ties_even(),
        RoundMode::Ceil => q.ceil(),
        RoundMode::Floor => q.floor(),
        RoundMode::Truncate => q.trunc(),
    };
    let mut out = n * step.value;
    if out == 0.0 {
        out = 0.0; // normalize -0.0
    }
    let d = step.decimals as usize;
    let mut s = format!("{out:.d$}");
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

/// Round one cell to the nearest multiple of `step`. Returns `None` when the cell isn't a number
/// (the caller leaves it unchanged).
fn round_cell(cell: &str, step: &StepSpec, mode: RoundMode, trailing_zeros: bool) -> Option<String> {
    let trimmed = cell.trim();
    if let (Some(v), Some(step_scaled)) = (parse_decimal(trimmed), step.scaled) {
        if let Some(r) = round_exact(v, step_scaled, step.decimals, mode, trailing_zeros) {
            return Some(r);
        }
    }
    let v: f64 = trimmed.parse().ok()?;
    if !v.is_finite() {
        return None;
    }
    Some(round_f64(v, step, mode, trailing_zeros))
}

/// Round every selected numeric cell of a CSV to the nearest multiple of `step`.
///
/// * `step` — the multiple to round to (must be > 0, e.g. 0.25, 5, 1000).
/// * `mode` — `half_up`/`half_down`/`half_even`/`ceil`/`floor`/`truncate`.
/// * `columns` — comma-separated 1-based indices and/or header names to round; empty = every
///   numeric cell in every column.
/// * `header` — treat the first row as a header (never rounded; enables name refs).
/// * `delimiter` — field separator (char or comma/tab/semicolon/pipe).
/// * `trailing_zeros` — pad every rounded cell to the step's own decimal places (0.25 → 1.00 / 1.25).
#[allow(clippy::too_many_arguments)]
pub fn round_csv(
    data: &str,
    step: f64,
    mode: &str,
    columns: &str,
    header: bool,
    delimiter: &str,
    trailing_zeros: bool,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let step = step_spec(step)?;
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
                    match round_cell(cell, &step, mode, trailing_zeros) {
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

    fn spec(step: f64) -> StepSpec {
        step_spec(step).unwrap()
    }
    fn rc(cell: &str, step: f64, mode: RoundMode) -> String {
        round_cell(cell, &spec(step), mode, false).unwrap()
    }

    #[test]
    fn nearest_basic_multiples() {
        assert_eq!(rc("47", 5.0, RoundMode::HalfUp), "45");
        assert_eq!(rc("82", 10.0, RoundMode::HalfUp), "80");
        assert_eq!(rc("137", 25.0, RoundMode::HalfUp), "125");
        assert_eq!(rc("276", 50.0, RoundMode::HalfUp), "300");
        assert_eq!(rc("1234", 1000.0, RoundMode::HalfUp), "1000");
        assert_eq!(rc("1500", 1000.0, RoundMode::HalfUp), "2000"); // exact tie away from zero
    }

    #[test]
    fn decimal_steps_are_exact() {
        // 0.125 is exactly halfway between 0.10 and 0.15 at step 0.05.
        assert_eq!(rc("0.125", 0.05, RoundMode::HalfUp), "0.15");
        assert_eq!(rc("0.125", 0.05, RoundMode::HalfDown), "0.1");
        assert_eq!(rc("1.23", 0.05, RoundMode::HalfUp), "1.25");
        assert_eq!(rc("1.24", 0.05, RoundMode::HalfUp), "1.25");
        assert_eq!(rc("2.42", 0.05, RoundMode::HalfUp), "2.4");
        // Quarter rounding.
        assert_eq!(rc("1.1", 0.25, RoundMode::HalfUp), "1"); // 1.1/0.25=4.4 → 4 → 1.0
        assert_eq!(rc("1.2", 0.25, RoundMode::HalfUp), "1.25");
        assert_eq!(rc("1.375", 0.25, RoundMode::HalfUp), "1.5"); // exact tie
    }

    #[test]
    fn directions_up_down_truncate() {
        assert_eq!(rc("41", 5.0, RoundMode::Ceil), "45");
        assert_eq!(rc("45", 5.0, RoundMode::Ceil), "45"); // already a multiple
        assert_eq!(rc("49", 5.0, RoundMode::Floor), "45");
        assert_eq!(rc("2.1", 0.25, RoundMode::Ceil), "2.25");
        assert_eq!(rc("2.99", 0.25, RoundMode::Floor), "2.75");
        assert_eq!(rc("-2.1", 0.25, RoundMode::Ceil), "-2");
        assert_eq!(rc("-2.1", 0.25, RoundMode::Floor), "-2.25");
        assert_eq!(rc("-2.9", 5.0, RoundMode::Truncate), "0"); // toward zero
        assert_eq!(rc("7.9", 5.0, RoundMode::Truncate), "5");
    }

    #[test]
    fn negatives_and_ties() {
        assert_eq!(rc("-1500", 1000.0, RoundMode::HalfUp), "-2000"); // away from zero
        assert_eq!(rc("-1500", 1000.0, RoundMode::HalfDown), "-1000"); // toward zero
        assert_eq!(rc("-2.5", 5.0, RoundMode::HalfEven), "0"); // -0.5 quotient → even 0
        assert_eq!(rc("7.5", 5.0, RoundMode::HalfEven), "10"); // 1.5 quotient → even 2
        assert_eq!(rc("2.5", 5.0, RoundMode::HalfEven), "0"); // 0.5 quotient → even 0
        // -0 normalizes to a bare 0.
        assert_eq!(rc("-0.2", 5.0, RoundMode::HalfUp), "0");
    }

    #[test]
    fn trailing_zeros_padding() {
        let s = spec(0.25);
        assert_eq!(round_cell("1", &s, RoundMode::HalfUp, true).unwrap(), "1.00");
        assert_eq!(round_cell("1.2", &s, RoundMode::HalfUp, true).unwrap(), "1.25");
        assert_eq!(round_cell("1.4", &s, RoundMode::HalfUp, true).unwrap(), "1.50");
        // Integer step can't pad.
        let si = spec(5.0);
        assert_eq!(round_cell("6", &si, RoundMode::HalfUp, true).unwrap(), "5");
    }

    #[test]
    fn non_numeric_cells_pass_through() {
        let s = spec(5.0);
        assert_eq!(round_cell("$12.99", &s, RoundMode::HalfUp, false), None);
        assert_eq!(round_cell("hello", &s, RoundMode::HalfUp, false), None);
        assert_eq!(round_cell("", &s, RoundMode::HalfUp, false), None);
        assert_eq!(round_cell("1,234", &s, RoundMode::HalfUp, false), None); // thousands sep
    }

    #[test]
    fn scientific_fallback() {
        // 1.5e3 = 1500 → f64 path → nearest 1000 → 2000.
        assert_eq!(rc("1.5e3", 1000.0, RoundMode::HalfUp), "2000");
    }

    #[test]
    fn step_validation() {
        assert!(step_spec(0.0).is_err());
        assert!(step_spec(-1.0).is_err());
        assert!(step_spec(f64::NAN).is_err());
        assert!(step_spec(f64::INFINITY).is_err());
        assert!(step_spec(0.25).is_ok());
    }

    #[test]
    fn csv_all_columns_default() {
        let d = "name,price,qty\nApple,1.23,7\nPear,2.42,11";
        let got = round_csv(d, 0.05, "half_up", "", true, ",", false).unwrap();
        assert_eq!(got, "name,price,qty\nApple,1.25,7\nPear,2.4,11\n");
    }

    #[test]
    fn csv_selected_column_and_padding() {
        let d = "name,price,qty\nApple,1.2,7\nPear,1.4,11";
        // Round only "price" to nearest 0.25, padded.
        let got = round_csv(d, 0.25, "half_up", "price", true, ",", true).unwrap();
        assert_eq!(got, "name,price,qty\nApple,1.25,7\nPear,1.50,11\n");
        // Same via 1-based index.
        let got2 = round_csv(d, 0.25, "half_up", "2", true, ",", true).unwrap();
        assert_eq!(got2, got);
    }

    #[test]
    fn csv_no_header_and_delimiter() {
        let d = "1234;41\n1500;49";
        let got = round_csv(d, 1000.0, "half_up", "", false, "semicolon", false).unwrap();
        assert_eq!(got, "1000;0\n2000;0\n");
    }

    #[test]
    fn errors() {
        assert!(round_csv("  ", 5.0, "half_up", "", true, ",", false).is_err()); // empty
        assert!(round_csv("a,b\n1,2", 0.0, "half_up", "", true, ",", false).is_err()); // step 0
        assert!(round_csv("a,b\n1,2", -5.0, "half_up", "", true, ",", false).is_err()); // step neg
        assert!(round_csv("a,b\n1,2", 5.0, "bogus", "", true, ",", false).is_err()); // bad mode
        assert!(round_csv("a,b\n1,2", 5.0, "half_up", "nope", true, ",", false).is_err()); // bad name
        assert!(round_csv("a,b\n1,2", 5.0, "half_up", "5", true, ",", false).is_err()); // index oob
        assert!(round_csv("a,b\n1,2", 5.0, "half_up", "name", false, ",", false).is_err()); // name w/o header
        assert!(round_csv("a,b\n1,2", 5.0, "half_up", "", true, "nope", false).is_err()); // bad delim
    }
}
