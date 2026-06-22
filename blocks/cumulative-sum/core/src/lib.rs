//! cumulative-sum core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Produces the running total (prefix sum) of a list of numbers, with optional
//! running average (mean of values seen so far), running minimum, and running
//! maximum. The output is a small table, one row per input value.

/// Maximum number of values accepted. Keeps the output bounded for the chat /
/// page surfaces (a few thousand numbers is plenty for the running-total use
/// case).
pub const MAX_LEN: usize = 100_000;

/// Parse a list of numbers. Values may be separated by commas, whitespace, or
/// newlines (any mix). Blank tokens are ignored, so trailing separators / blank
/// lines are fine. Returns an error naming the first token that isn't a number.
fn parse_numbers(text: &str) -> Result<Vec<f64>, String> {
    let mut out = Vec::new();
    for tok in text.split(|c: char| c == ',' || c.is_whitespace()) {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let n: f64 = tok
            .parse()
            .map_err(|_| format!("{tok:?} is not a valid number"))?;
        if !n.is_finite() {
            return Err(format!("{tok:?} is not a finite number"));
        }
        out.push(n);
        if out.len() > MAX_LEN {
            return Err(format!("input exceeds the maximum of {MAX_LEN} values"));
        }
    }
    Ok(out)
}

/// Format an `f64` without a trailing `.0` for whole numbers, so `4.0` prints as
/// `4` but `4.5` stays `4.5`.
fn fmt_num(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        // Trim noisy float tails (e.g. 0.30000000000000004) to a sane precision.
        let s = format!("{:.10}", n);
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    }
}

/// Compute the running total (and optional running average / min / max) of a
/// list of numbers.
///
/// - `numbers`: the values, separated by commas, spaces, or newlines.
/// - `average`: also show the running average (mean of values seen so far).
/// - `min`: also show the running minimum.
/// - `max`: also show the running maximum.
///
/// Returns a table with a header row and one row per input value. The first
/// column is the value, the second is the cumulative sum, followed by any of the
/// optional columns that were requested. Columns are separated by `" | "`.
pub fn cumulative(
    numbers: &str,
    average: bool,
    min: bool,
    max: bool,
) -> Result<String, String> {
    let values = parse_numbers(numbers)?;
    if values.is_empty() {
        return Err("provide at least one number".into());
    }

    let mut header = vec!["value".to_string(), "cumulative_sum".to_string()];
    if average {
        header.push("running_avg".to_string());
    }
    if min {
        header.push("running_min".to_string());
    }
    if max {
        header.push("running_max".to_string());
    }

    let mut lines = Vec::with_capacity(values.len() + 1);
    lines.push(header.join(" | "));

    let mut sum = 0.0_f64;
    let mut run_min = f64::INFINITY;
    let mut run_max = f64::NEG_INFINITY;
    for (i, &v) in values.iter().enumerate() {
        sum += v;
        if v < run_min {
            run_min = v;
        }
        if v > run_max {
            run_max = v;
        }
        let mut row = vec![fmt_num(v), fmt_num(sum)];
        if average {
            row.push(fmt_num(sum / (i as f64 + 1.0)));
        }
        if min {
            row.push(fmt_num(run_min));
        }
        if max {
            row.push(fmt_num(run_max));
        }
        lines.push(row.join(" | "));
    }

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_running_total() {
        let out = cumulative("1, 2, 3, 4", false, false, false).unwrap();
        assert_eq!(
            out,
            "value | cumulative_sum\n1 | 1\n2 | 3\n3 | 6\n4 | 10"
        );
    }

    #[test]
    fn newline_and_space_separated() {
        let out = cumulative("10\n20 30", false, false, false).unwrap();
        assert_eq!(out, "value | cumulative_sum\n10 | 10\n20 | 30\n30 | 60");
    }

    #[test]
    fn negatives_and_decimals() {
        let out = cumulative("1.5, -0.5, 2", false, false, false).unwrap();
        assert_eq!(out, "value | cumulative_sum\n1.5 | 1.5\n-0.5 | 1\n2 | 3");
    }

    #[test]
    fn with_running_average() {
        let out = cumulative("2, 4, 6", true, false, false).unwrap();
        assert_eq!(
            out,
            "value | cumulative_sum | running_avg\n2 | 2 | 2\n4 | 6 | 3\n6 | 12 | 4"
        );
    }

    #[test]
    fn with_running_min_and_max() {
        let out = cumulative("3, 1, 4, 1, 5", false, true, true).unwrap();
        assert_eq!(
            out,
            "value | cumulative_sum | running_min | running_max\n\
             3 | 3 | 3 | 3\n\
             1 | 4 | 1 | 3\n\
             4 | 8 | 1 | 4\n\
             1 | 9 | 1 | 4\n\
             5 | 14 | 1 | 5"
        );
    }

    #[test]
    fn all_columns() {
        let out = cumulative("4, 2", true, true, true).unwrap();
        assert_eq!(
            out,
            "value | cumulative_sum | running_avg | running_min | running_max\n\
             4 | 4 | 4 | 4 | 4\n\
             2 | 6 | 3 | 2 | 4"
        );
    }

    #[test]
    fn trailing_separators_ignored() {
        let out = cumulative("1, 2,\n", false, false, false).unwrap();
        assert_eq!(out, "value | cumulative_sum\n1 | 1\n2 | 3");
    }

    #[test]
    fn fractional_average() {
        let out = cumulative("1, 2", true, false, false).unwrap();
        assert_eq!(out, "value | cumulative_sum | running_avg\n1 | 1 | 1\n2 | 3 | 1.5");
    }

    #[test]
    fn empty_is_error() {
        assert!(cumulative("", false, false, false).is_err());
        assert!(cumulative("  \n , ", false, false, false).is_err());
    }

    #[test]
    fn non_number_is_error() {
        let err = cumulative("1, two, 3", false, false, false).unwrap_err();
        assert!(err.contains("two"), "error names the bad token: {err}");
    }

    #[test]
    fn single_value() {
        let out = cumulative("42", true, true, true).unwrap();
        assert_eq!(
            out,
            "value | cumulative_sum | running_avg | running_min | running_max\n42 | 42 | 42 | 42 | 42"
        );
    }
}
