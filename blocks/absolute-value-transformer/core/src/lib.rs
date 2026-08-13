//! absolute-value-transformer core — apply a unary sign transform (absolute
//! value, signum, negation, force-negative) to a pasted column of numbers.
//! No wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.

/// Maximum number of tokens accepted in one run (valid or not). Keeps the
/// output bounded for the chat/page surfaces — a spreadsheet column of 20,000
/// values is well past the paste-a-column use case.
pub const MAX_VALUES: usize = 20_000;

/// The unary transforms this tool applies to every value in the column.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Operation {
    /// `|x|` — drop the sign.
    Abs,
    /// `sign(x)` — -1 for negative, 0 for zero, 1 for positive.
    Sign,
    /// `-x` — flip the sign of every value.
    Negate,
    /// `-|x|` — force every value negative.
    ForceNegative,
}

impl Operation {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "abs" | "absolute" => Ok(Operation::Abs),
            "sign" | "signum" => Ok(Operation::Sign),
            "negate" | "flip" => Ok(Operation::Negate),
            "force-negative" | "force_negative" => Ok(Operation::ForceNegative),
            other => Err(format!(
                "unknown operation {other:?} — expected abs, sign, negate, or force-negative"
            )),
        }
    }

    fn apply(self, x: f64) -> f64 {
        match self {
            Operation::Abs => x.abs(),
            Operation::Sign => {
                if x > 0.0 {
                    1.0
                } else if x < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            }
            Operation::Negate => -x,
            Operation::ForceNegative => -x.abs(),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Operation::Abs => "abs",
            Operation::Sign => "sign",
            Operation::Negate => "negate",
            Operation::ForceNegative => "force-negative",
        }
    }
}

/// How values are separated on the way in and on the way out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sep {
    /// Input only: split on any of newline / comma / semicolon / pipe / whitespace.
    Auto,
    Newline,
    Comma,
    Space,
    Semicolon,
    Tab,
    Pipe,
}

impl Sep {
    fn parse(s: &str, field: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "auto" => Ok(Sep::Auto),
            "newline" | "line" => Ok(Sep::Newline),
            "comma" => Ok(Sep::Comma),
            "space" => Ok(Sep::Space),
            "semicolon" => Ok(Sep::Semicolon),
            "tab" => Ok(Sep::Tab),
            "pipe" => Ok(Sep::Pipe),
            other => Err(format!(
                "unknown {field} {other:?} — expected newline, comma, space, semicolon, tab, pipe, or auto/same"
            )),
        }
    }

    /// The literal string used to join output values.
    fn joiner(self) -> &'static str {
        match self {
            // `Auto` never reaches the output side: it is resolved to the
            // detected input separator first.
            Sep::Auto | Sep::Newline => "\n",
            Sep::Comma => ",",
            Sep::Space => " ",
            Sep::Semicolon => ";",
            Sep::Tab => "\t",
            Sep::Pipe => "|",
        }
    }
}

/// What to emit for a token that is not a finite number.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OnError {
    /// Stop the whole run with an error naming the offending token.
    Fail,
    /// Drop the value entirely.
    Skip,
    /// Echo the original text back so the column still lines up.
    Keep,
    /// Emit an empty value so the column still lines up.
    Blank,
}

impl OnError {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "fail" => Ok(OnError::Fail),
            "skip" => Ok(OnError::Skip),
            "keep" => Ok(OnError::Keep),
            "blank" => Ok(OnError::Blank),
            other => Err(format!(
                "unknown on_error {other:?} — expected fail, skip, keep, or blank"
            )),
        }
    }
}

/// Output shape.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OutputFormat {
    /// Transformed values only, joined by the output separator.
    Values,
    /// `original<TAB>result` audit table with a header row.
    Table,
    /// Structured JSON.
    Json,
}

impl OutputFormat {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "values" => Ok(OutputFormat::Values),
            "table" => Ok(OutputFormat::Table),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!(
                "unknown output {other:?} — expected values, table, or json"
            )),
        }
    }
}

/// One input token and what became of it.
struct Row {
    original: String,
    /// `Ok(transformed)` or `Err(reason)`.
    value: Result<f64, String>,
}

/// Split the input into trimmed, non-empty tokens using `sep`.
fn split_tokens(data: &str, sep: Sep) -> Vec<&str> {
    let raw: Vec<&str> = match sep {
        Sep::Auto => data
            .split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '|')
            .collect(),
        Sep::Space => data.split_whitespace().collect(),
        Sep::Newline => data.split('\n').collect(),
        Sep::Comma => data.split(',').collect(),
        Sep::Semicolon => data.split(';').collect(),
        Sep::Tab => data.split('\t').collect(),
        Sep::Pipe => data.split('|').collect(),
    };
    raw.into_iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect()
}

/// Guess which separator the pasted column uses, so `output_separator = same`
/// can mirror it. Counts the structural separators and picks the most common;
/// falls back to spaces, then to newlines.
fn detect_sep(data: &str) -> Sep {
    let candidates = [
        (Sep::Newline, data.matches('\n').count()),
        (Sep::Comma, data.matches(',').count()),
        (Sep::Semicolon, data.matches(';').count()),
        (Sep::Tab, data.matches('\t').count()),
        (Sep::Pipe, data.matches('|').count()),
    ];
    let best = candidates.iter().max_by_key(|(_, n)| *n).copied().unwrap();
    if best.1 > 0 {
        // Ties resolve to the earliest candidate, which `max_by_key` already
        // does NOT guarantee — re-scan in declaration order for determinism.
        let top = best.1;
        for (sep, n) in candidates {
            if n == top {
                return sep;
            }
        }
    }
    if data.split_whitespace().count() > 1 {
        return Sep::Space;
    }
    Sep::Newline
}

/// Parse one token as a finite decimal number.
fn parse_number(token: &str) -> Result<f64, String> {
    let t = token.trim();
    if t.contains('/') {
        return Err(format!(
            "{t:?} is not a plain number — fractions are not supported, use a decimal like 0.25"
        ));
    }
    match t.parse::<f64>() {
        Ok(n) if n.is_finite() => Ok(n),
        Ok(_) => Err(format!(
            "{t:?} is not a finite number — infinity and NaN cannot be transformed"
        )),
        Err(_) => Err(format!(
            "{t:?} is not a number — strip currency symbols, thousands separators, and units first"
        )),
    }
}

/// Format a transformed value. With `decimals = None` whole numbers print
/// without a trailing `.0`; with `Some(n)` every value gets exactly `n` places.
fn format_value(v: f64, decimals: Option<u32>) -> String {
    match decimals {
        Some(n) => {
            let factor = 10f64.powi(n as i32);
            let mut r = (v * factor).round() / factor;
            if r == 0.0 {
                // Collapse -0.0 so rounding never prints "-0.00".
                r = 0.0;
            }
            format!("{:.*}", n as usize, r)
        }
        None => {
            if v.fract() == 0.0 && v.abs() < 1e15 {
                format!("{}", v as i64)
            } else {
                format!("{v}")
            }
        }
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// count / sum / min / max / mean over the successfully transformed values.
fn stats_of(rows: &[Row]) -> Option<(usize, f64, f64, f64, f64)> {
    let vals: Vec<f64> = rows.iter().filter_map(|r| r.value.as_ref().ok().copied()).collect();
    if vals.is_empty() {
        return None;
    }
    let sum: f64 = vals.iter().sum();
    let min = vals.iter().copied().fold(f64::INFINITY, f64::min);
    let max = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Some((vals.len(), sum, min, max, sum / vals.len() as f64))
}

fn render_stats(rows: &[Row], decimals: Option<u32>, out: &mut String) {
    out.push_str("\n\n--- Summary ---\n");
    match stats_of(rows) {
        Some((count, sum, min, max, mean)) => {
            out.push_str(&format!("Count: {count}\n"));
            out.push_str(&format!("Sum: {}\n", format_value(sum, decimals)));
            out.push_str(&format!("Min: {}\n", format_value(min, decimals)));
            out.push_str(&format!("Max: {}\n", format_value(max, decimals)));
            out.push_str(&format!("Mean: {}", format_value(mean, decimals)));
        }
        None => out.push_str("Count: 0\nSum: n/a"),
    }
}

/// Apply a sign transform to every number in `data`.
///
/// - `data`: the numeric column (values separated per `separator`).
/// - `operation`: `abs` (default) | `sign` | `negate` | `force-negative`.
/// - `separator`: `auto` (default) | `newline` | `comma` | `space` |
///   `semicolon` | `tab` | `pipe` — how the input is split.
/// - `output_separator`: `same` (default, mirrors the input) or one of the
///   separator names above.
/// - `decimals`: `None` keeps full precision, `Some(n)` rounds to `n` places.
/// - `on_error`: `fail` (default) | `skip` | `keep` | `blank` for tokens that
///   are not numbers.
/// - `output`: `values` (default) | `table` | `json`.
/// - `stats`: append count/sum/min/max/mean over the transformed values.
pub fn run(
    data: &str,
    operation: &str,
    separator: &str,
    output_separator: &str,
    decimals: Option<u32>,
    on_error: &str,
    output: &str,
    stats: bool,
) -> Result<String, String> {
    let op = Operation::parse(operation)?;
    let in_sep = Sep::parse(separator, "separator")?;
    let out_sep_opt = match output_separator.trim() {
        "" | "same" | "auto" => None,
        other => Some(Sep::parse(other, "output_separator")?),
    };
    let on_err = OnError::parse(on_error)?;
    let fmt = OutputFormat::parse(output)?;
    if let Some(n) = decimals {
        if n > 6 {
            return Err(format!("decimals must be auto or 0-6 (got {n})"));
        }
    }

    let tokens = split_tokens(data, in_sep);
    if tokens.is_empty() {
        return Err("no numbers found — paste a column of values, one per line".into());
    }
    if tokens.len() > MAX_VALUES {
        return Err(format!(
            "input has {} values, which exceeds the maximum of {MAX_VALUES}",
            tokens.len()
        ));
    }

    let mut rows: Vec<Row> = Vec::with_capacity(tokens.len());
    for (i, token) in tokens.iter().enumerate() {
        let value = match parse_number(token) {
            Ok(n) => Ok(op.apply(n)),
            Err(e) => {
                if on_err == OnError::Fail {
                    return Err(format!("value {}: {e}", i + 1));
                }
                Err(e)
            }
        };
        rows.push(Row {
            original: (*token).to_string(),
            value,
        });
    }

    let out_sep = out_sep_opt.unwrap_or(match in_sep {
        Sep::Auto => detect_sep(data),
        other => other,
    });

    Ok(match fmt {
        OutputFormat::Values => {
            let mut cells: Vec<String> = Vec::with_capacity(rows.len());
            for row in &rows {
                match (&row.value, on_err) {
                    (Ok(v), _) => cells.push(format_value(*v, decimals)),
                    (Err(_), OnError::Skip) => {}
                    (Err(_), OnError::Keep) => cells.push(row.original.clone()),
                    // `Fail` already returned above; `Blank` keeps alignment.
                    (Err(_), _) => cells.push(String::new()),
                }
            }
            let mut out = cells.join(out_sep.joiner());
            if stats {
                render_stats(&rows, decimals, &mut out);
            }
            out
        }
        OutputFormat::Table => {
            let mut out = String::from("original\tresult");
            for row in &rows {
                match (&row.value, on_err) {
                    (Ok(v), _) => {
                        out.push_str(&format!(
                            "\n{}\t{}",
                            row.original,
                            format_value(*v, decimals)
                        ));
                    }
                    (Err(_), OnError::Skip) => {}
                    (Err(_), OnError::Keep) => {
                        out.push_str(&format!("\n{}\t{}", row.original, row.original));
                    }
                    (Err(_), _) => out.push_str(&format!("\n{}\t", row.original)),
                }
            }
            if stats {
                render_stats(&rows, decimals, &mut out);
            }
            out
        }
        OutputFormat::Json => {
            let transformed = rows.iter().filter(|r| r.value.is_ok()).count();
            let invalid = rows.len() - transformed;
            let mut out = String::from("{\n");
            out.push_str(&format!("  \"operation\": \"{}\",\n", op.label()));
            out.push_str(&format!("  \"count\": {},\n", rows.len()));
            out.push_str(&format!("  \"transformed\": {transformed},\n"));
            out.push_str(&format!("  \"invalid\": {invalid},\n"));
            out.push_str("  \"values\": [");
            let mut first = true;
            for row in &rows {
                if row.value.is_err() && on_err == OnError::Skip {
                    continue;
                }
                if !first {
                    out.push(',');
                }
                first = false;
                out.push_str("\n    {");
                out.push_str(&format!(
                    "\"original\": \"{}\", ",
                    json_escape(&row.original)
                ));
                match &row.value {
                    Ok(v) => out.push_str(&format!("\"value\": {}", format_value(*v, decimals))),
                    Err(e) => out.push_str(&format!(
                        "\"value\": null, \"error\": \"{}\"",
                        json_escape(e)
                    )),
                }
                out.push('}');
            }
            out.push_str("\n  ]");
            if stats {
                if let Some((count, sum, min, max, mean)) = stats_of(&rows) {
                    out.push_str(",\n  \"stats\": {");
                    out.push_str(&format!("\"count\": {count}, "));
                    out.push_str(&format!("\"sum\": {}, ", format_value(sum, decimals)));
                    out.push_str(&format!("\"min\": {}, ", format_value(min, decimals)));
                    out.push_str(&format!("\"max\": {}, ", format_value(max, decimals)));
                    out.push_str(&format!("\"mean\": {}", format_value(mean, decimals)));
                    out.push('}');
                } else {
                    out.push_str(",\n  \"stats\": null");
                }
            }
            out.push_str("\n}");
            out
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abs(data: &str) -> String {
        run(data, "abs", "auto", "same", None, "fail", "values", false).unwrap()
    }

    #[test]
    fn absolute_value_of_a_newline_column() {
        assert_eq!(abs("-3\n4\n-2.5\n0"), "3\n4\n2.5\n0");
    }

    #[test]
    fn sign_extraction_is_minus_one_zero_one() {
        let out = run("-7\n0\n0.001", "sign", "newline", "same", None, "fail", "values", false)
            .unwrap();
        assert_eq!(out, "-1\n0\n1");
    }

    #[test]
    fn negate_flips_every_sign() {
        let out = run("-3, 4, 0", "negate", "auto", "same", None, "fail", "values", false).unwrap();
        // The comma column round-trips as a comma column.
        assert_eq!(out, "3,-4,0");
    }

    #[test]
    fn force_negative_makes_everything_a_debit() {
        let out = run(
            "3\n-4\n0",
            "force-negative",
            "newline",
            "same",
            None,
            "fail",
            "values",
            false,
        )
        .unwrap();
        assert_eq!(out, "-3\n-4\n0");
    }

    #[test]
    fn output_separator_overrides_the_input_shape() {
        let out = run("-1\n-2", "abs", "newline", "comma", None, "fail", "values", false).unwrap();
        assert_eq!(out, "1,2");
    }

    #[test]
    fn scientific_notation_and_plus_signs_parse() {
        assert_eq!(abs("-1.5e3\n+2\n-0.25"), "1500\n2\n0.25");
    }

    #[test]
    fn decimals_round_and_never_print_negative_zero() {
        let out = run(
            "-0.004\n2.345",
            "negate",
            "newline",
            "same",
            Some(2),
            "fail",
            "values",
            false,
        )
        .unwrap();
        assert_eq!(out, "0.00\n-2.35");
    }

    #[test]
    fn table_output_keeps_the_original_column() {
        let out = run("-3\n4", "abs", "newline", "same", None, "fail", "table", false).unwrap();
        assert_eq!(out, "original\tresult\n-3\t3\n4\t4");
    }

    #[test]
    fn json_output_reports_counts_and_values() {
        let out = run("-3\nx", "abs", "newline", "same", None, "blank", "json", false).unwrap();
        assert!(out.contains("\"operation\": \"abs\""));
        assert!(out.contains("\"transformed\": 1"));
        assert!(out.contains("\"invalid\": 1"));
        assert!(out.contains("\"original\": \"-3\", \"value\": 3"));
        assert!(out.contains("\"value\": null, \"error\":"));
    }

    #[test]
    fn stats_summarise_the_transformed_values() {
        let out = run("-3\n4\n-5", "abs", "newline", "same", None, "fail", "values", true).unwrap();
        assert!(out.ends_with("--- Summary ---\nCount: 3\nSum: 12\nMin: 3\nMax: 5\nMean: 4"));
    }

    #[test]
    fn on_error_skip_drops_bad_tokens_and_keep_echoes_them() {
        let skipped =
            run("-3\nn/a\n4", "abs", "newline", "same", None, "skip", "values", false).unwrap();
        assert_eq!(skipped, "3\n4");
        let kept =
            run("-3\nn/a\n4", "abs", "newline", "same", None, "keep", "values", false).unwrap();
        assert_eq!(kept, "3\nn/a\n4");
        let blanked =
            run("-3\nn/a\n4", "abs", "newline", "same", None, "blank", "values", false).unwrap();
        assert_eq!(blanked, "3\n\n4");
    }

    #[test]
    fn non_numeric_token_fails_by_default_with_its_position() {
        let err = run("-3\n1,234\n4", "abs", "newline", "same", None, "fail", "values", false)
            .unwrap_err();
        assert!(err.contains("value 2"), "{err}");
        assert!(err.contains("thousands separators"), "{err}");
    }

    #[test]
    fn fractions_get_a_dedicated_error() {
        let err = run("1/4", "abs", "newline", "same", None, "fail", "values", false).unwrap_err();
        assert!(err.contains("fractions are not supported"), "{err}");
    }

    #[test]
    fn infinity_and_nan_are_rejected() {
        for bad in ["inf", "-infinity", "NaN"] {
            let err =
                run(bad, "abs", "newline", "same", None, "fail", "values", false).unwrap_err();
            assert!(err.contains("finite"), "{bad}: {err}");
        }
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = run("   \n\n", "abs", "auto", "same", None, "fail", "values", false).unwrap_err();
        assert!(err.contains("no numbers found"), "{err}");
    }

    #[test]
    fn unknown_option_values_are_rejected() {
        assert!(run("1", "sqrt", "auto", "same", None, "fail", "values", false)
            .unwrap_err()
            .contains("unknown operation"));
        assert!(run("1", "abs", "colon", "same", None, "fail", "values", false)
            .unwrap_err()
            .contains("unknown separator"));
        assert!(run("1", "abs", "auto", "colon", None, "fail", "values", false)
            .unwrap_err()
            .contains("unknown output_separator"));
        assert!(run("1", "abs", "auto", "same", None, "maybe", "values", false)
            .unwrap_err()
            .contains("unknown on_error"));
        assert!(run("1", "abs", "auto", "same", None, "fail", "csv", false)
            .unwrap_err()
            .contains("unknown output"));
        assert!(run("1", "abs", "auto", "same", Some(7), "fail", "values", false)
            .unwrap_err()
            .contains("0-6"));
    }

    #[test]
    fn value_cap_is_enforced_at_the_boundary() {
        let at_cap = vec!["-1"; MAX_VALUES].join("\n");
        assert!(run(&at_cap, "abs", "newline", "same", None, "fail", "values", false).is_ok());
        let over_cap = vec!["-1"; MAX_VALUES + 1].join("\n");
        let err =
            run(&over_cap, "abs", "newline", "same", None, "fail", "values", false).unwrap_err();
        assert!(err.contains("exceeds the maximum"), "{err}");
    }

    #[test]
    fn every_separator_splits_and_joins() {
        for (name, joined) in [
            ("comma", "-1,2"),
            ("semicolon", "-1;2"),
            ("tab", "-1\t2"),
            ("pipe", "-1|2"),
            ("space", "-1 2"),
        ] {
            let out = run(joined, "abs", name, name, None, "fail", "values", false).unwrap();
            assert_eq!(out, joined.replace("-1", "1"), "separator {name}");
        }
    }
}
