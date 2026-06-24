//! column-math core — element-wise arithmetic between two equal-length numeric
//! columns. No wafer/wasm-bindgen deps. Shared by the chat skill block and the
//! web page.

/// Maximum number of values allowed in either column. Keeps the output bounded
/// for the chat/page surfaces (a column of a few thousand numbers is plenty for
/// the spreadsheet-style use case).
pub const MAX_LEN: usize = 100_000;

/// The four element-wise operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Op {
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl Op {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "add" => Ok(Op::Add),
            "subtract" => Ok(Op::Subtract),
            "multiply" => Ok(Op::Multiply),
            "divide" => Ok(Op::Divide),
            other => Err(format!(
                "invalid operation {other:?}: expected \"add\", \"subtract\", \"multiply\", or \"divide\""
            )),
        }
    }

    fn apply(self, a: f64, b: f64) -> Result<f64, String> {
        Ok(match self {
            Op::Add => a + b,
            Op::Subtract => a - b,
            Op::Multiply => a * b,
            Op::Divide => {
                if b == 0.0 {
                    return Err("division by zero".into());
                }
                a / b
            }
        })
    }
}

/// Parse a column of numbers. Values may be separated by commas, whitespace, or
/// newlines (any mix). Blank tokens are ignored, so trailing separators / blank
/// lines are fine. Returns an error naming the first token that isn't a number.
fn parse_column(text: &str, which: &str) -> Result<Vec<f64>, String> {
    let mut out = Vec::new();
    for tok in text.split(|c: char| c == ',' || c.is_whitespace()) {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let n: f64 = tok
            .parse()
            .map_err(|_| format!("column {which}: {tok:?} is not a valid number"))?;
        out.push(n);
        if out.len() > MAX_LEN {
            return Err(format!("column {which} exceeds the maximum of {MAX_LEN} values"));
        }
    }
    Ok(out)
}

/// Format an `f64` result without a trailing `.0` for whole numbers, so `4.0`
/// prints as `4` but `4.5` stays `4.5`.
fn fmt_num(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Perform `operation` element-wise between columns `a` and `b`.
///
/// - `a`, `b`: numeric columns (comma / whitespace / newline separated).
/// - `operation`: `"add"` (default) | `"subtract"` | `"multiply"` | `"divide"`.
///
/// The two columns must have the same number of values. On `divide`, a zero
/// divisor is an error. Returns the results as a newline-separated list.
pub fn compute(a: &str, b: &str, operation: &str) -> Result<String, String> {
    let op = Op::parse(operation)?;
    let col_a = parse_column(a, "A")?;
    let col_b = parse_column(b, "B")?;

    if col_a.is_empty() || col_b.is_empty() {
        return Err("both columns must contain at least one number".into());
    }
    if col_a.len() != col_b.len() {
        return Err(format!(
            "columns must have the same length: A has {} value(s), B has {} value(s)",
            col_a.len(),
            col_b.len()
        ));
    }

    let mut out = Vec::with_capacity(col_a.len());
    for (i, (&x, &y)) in col_a.iter().zip(col_b.iter()).enumerate() {
        let r = op
            .apply(x, y)
            .map_err(|e| format!("row {}: {e}", i + 1))?;
        out.push(fmt_num(r));
    }
    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_defaults_when_blank_operation() {
        assert_eq!(compute("1,2,3", "4,5,6", "").unwrap(), "5\n7\n9");
        assert_eq!(compute("1,2,3", "4,5,6", "add").unwrap(), "5\n7\n9");
    }

    #[test]
    fn subtract_multiply_divide() {
        assert_eq!(compute("10 20", "3 5", "subtract").unwrap(), "7\n15");
        assert_eq!(compute("2,3", "4,5", "multiply").unwrap(), "8\n15");
        assert_eq!(compute("10,9", "2,3", "divide").unwrap(), "5\n3");
    }

    #[test]
    fn mixed_separators_and_blank_lines() {
        assert_eq!(compute("1, 2\n3", "4 5\n6\n", "add").unwrap(), "5\n7\n9");
    }

    #[test]
    fn decimals_preserved() {
        assert_eq!(compute("1.5,2.25", "0.5,0.25", "add").unwrap(), "2\n2.5");
    }

    #[test]
    fn negative_numbers() {
        assert_eq!(compute("-1,-2", "3,4", "add").unwrap(), "2\n2");
    }

    #[test]
    fn length_mismatch_errors() {
        let err = compute("1,2,3", "4,5", "add").unwrap_err();
        assert!(err.contains("same length"), "got: {err}");
        assert!(err.contains("3 value"), "got: {err}");
        assert!(err.contains("2 value"), "got: {err}");
    }

    #[test]
    fn divide_by_zero_errors() {
        let err = compute("1,2", "1,0", "divide").unwrap_err();
        assert!(err.contains("division by zero"), "got: {err}");
        assert!(err.contains("row 2"), "got: {err}");
    }

    #[test]
    fn non_numeric_token_errors() {
        let err = compute("1,abc", "4,5", "add").unwrap_err();
        assert!(err.contains("column A"), "got: {err}");
        assert!(err.contains("abc"), "got: {err}");
    }

    #[test]
    fn invalid_operation_errors() {
        let err = compute("1", "2", "power").unwrap_err();
        assert!(err.contains("invalid operation"), "got: {err}");
    }

    #[test]
    fn empty_columns_error() {
        let err = compute("", "", "add").unwrap_err();
        assert!(err.contains("at least one"), "got: {err}");
    }
}
