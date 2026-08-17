//! sorted-order-checker core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Paste a list of numbers and this verifies whether it is in order —
//! ascending, descending, or whichever direction the values themselves imply —
//! and, when it is not, pinpoints the FIRST out-of-order element (position,
//! value, the neighbour it violates, and why). Ties are allowed by default
//! (`1 2 2 5` is sorted ascending); `strict` requires a change at every step.
//! Report-only: the list is never re-ordered, nothing is fetched or persisted.

use serde::Serialize;

/// Hard cap on how many values one run will check.
pub const MAX_VALUES: usize = 20_000;

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// One neighbour comparison that violates the checked order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Break {
    /// 1-based position of the offending value.
    pub position: usize,
    /// The offending value.
    pub value: f64,
    /// 1-based position of the neighbour it was compared against.
    pub prev_position: usize,
    /// The neighbour's value.
    pub prev_value: f64,
    /// Machine kind: "less_than", "greater_than", or "equal".
    pub kind: String,
    /// Human-readable reason.
    pub reason: String,
}

/// The longest stretch of values that IS in the checked order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Run {
    /// 1-based first position of the run.
    pub start: usize,
    /// 1-based last position of the run.
    pub end: usize,
    /// Number of values in the run.
    pub length: usize,
}

/// The full ordering report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// True iff every neighbour pair satisfies the checked order.
    pub sorted: bool,
    /// Direction actually checked: "ascending", "descending", or "constant"
    /// (every value equal — only possible when order=auto).
    pub direction: String,
    /// What the caller asked for: "auto", "ascending", or "descending".
    pub requested_order: String,
    /// True when equal neighbours count as out of order.
    pub strict: bool,
    /// How many numeric values were checked.
    pub count: usize,
    /// Neighbour comparisons performed (`count - 1`, floored at 0).
    pub comparisons: usize,
    /// The first violating comparison, if any.
    pub first_break: Option<Break>,
    /// Violations, truncated to `max_issues`.
    pub breaks: Vec<Break>,
    /// Total violations found (may exceed `breaks.len()`).
    pub breaks_total: usize,
    /// Longest in-order stretch.
    pub longest_run: Run,
    /// Neighbour pairs with identical values.
    pub equal_adjacent: usize,
    /// First value in the list.
    pub first: f64,
    /// Last value in the list.
    pub last: f64,
    /// Smallest value seen.
    pub min: f64,
    /// Largest value seen.
    pub max: f64,
    /// Non-numeric tokens skipped (only possible with non_numeric=ignore).
    pub ignored_tokens: usize,
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Parsed, validated options for one run.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// "auto" | "ascending" | "descending".
    pub order: String,
    /// Equal neighbours count as out of order.
    pub strict: bool,
    /// "auto" | "comma" | "newline" | "space" | "semicolon" | "tab" | "pipe".
    pub separator: String,
    /// Treat `,` and `_` inside a token as digit-group separators.
    pub strip_thousands: bool,
    /// "error" | "ignore".
    pub non_numeric: String,
    /// How many breaks to list (totals always count all of them).
    pub max_issues: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            order: "auto".into(),
            strict: false,
            separator: "auto".into(),
            strip_thousands: false,
            non_numeric: "error".into(),
            max_issues: 20,
        }
    }
}

fn pick(value: &str, allowed: &[&str], field: &str) -> Result<String, String> {
    let v = value.trim().to_ascii_lowercase();
    if v.is_empty() {
        return Ok(allowed[0].to_string());
    }
    if allowed.contains(&v.as_str()) {
        Ok(v)
    } else {
        Err(format!(
            "unknown {field} '{}' — expected one of: {}",
            value.trim(),
            allowed.join(", ")
        ))
    }
}

impl Options {
    /// Build options from raw string/bool/number fields (CLI, chat, page).
    pub fn from_raw(
        order: &str,
        strict: bool,
        separator: &str,
        strip_thousands: bool,
        non_numeric: &str,
        max_issues: u64,
    ) -> Result<Options, String> {
        let order = pick(order, &["auto", "ascending", "descending"], "order")?;
        let separator = pick(
            separator,
            &[
                "auto",
                "comma",
                "newline",
                "space",
                "semicolon",
                "tab",
                "pipe",
            ],
            "separator",
        )?;
        let non_numeric = pick(non_numeric, &["error", "ignore"], "non_numeric")?;
        if !(1..=1000).contains(&max_issues) {
            return Err(format!(
                "max_issues must be between 1 and 1000 (got {max_issues})"
            ));
        }
        if strip_thousands && separator == "comma" {
            return Err(
                "strip_thousands cannot be used with separator=comma — a comma cannot be both the \
                 delimiter and a digit-group separator. Use separator=space, newline, semicolon, \
                 pipe, tab, or auto."
                    .into(),
            );
        }
        Ok(Options {
            order,
            strict,
            separator,
            strip_thousands,
            non_numeric,
            max_issues: max_issues as usize,
        })
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn is_delimiter(c: char, separator: &str, strip_thousands: bool) -> bool {
    match separator {
        "comma" => c == ',',
        "newline" => c == '\n' || c == '\r',
        "space" => c == ' ',
        "semicolon" => c == ';',
        "tab" => c == '\t',
        "pipe" => c == '|',
        // auto: any whitespace, plus the usual list punctuation. When commas
        // are digit-group separators they stop being delimiters.
        _ => c.is_whitespace() || c == ';' || c == '|' || (c == ',' && !strip_thousands),
    }
}

/// Split raw input into non-empty tokens using the chosen separator.
pub fn tokenize(input: &str, opts: &Options) -> Vec<String> {
    input
        .split(|c: char| is_delimiter(c, &opts.separator, opts.strip_thousands))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

fn parse_number(token: &str, strip_thousands: bool) -> Option<f64> {
    let cleaned: String = if strip_thousands {
        token.chars().filter(|c| *c != ',' && *c != '_').collect()
    } else {
        token.to_string()
    };
    match cleaned.parse::<f64>() {
        // NaN has no order — every comparison against it is false, so a list
        // containing one can never be honestly called sorted.
        Ok(v) if v.is_nan() => None,
        Ok(v) => Some(v),
        Err(_) => None,
    }
}

/// Parse the input into values, honouring the non_numeric policy.
/// Returns the values plus how many tokens were skipped.
pub fn parse_values(input: &str, opts: &Options) -> Result<(Vec<f64>, usize), String> {
    let tokens = tokenize(input, opts);
    let mut values = Vec::with_capacity(tokens.len());
    let mut ignored = 0usize;
    for (i, token) in tokens.iter().enumerate() {
        match parse_number(token, opts.strip_thousands) {
            Some(v) => values.push(v),
            None => {
                if opts.non_numeric == "ignore" {
                    ignored += 1;
                } else {
                    return Err(format!(
                        "token {} is not a number: '{}'. Set non_numeric=ignore to skip \
                         non-numeric entries, or fix the value.",
                        i + 1,
                        token
                    ));
                }
            }
        }
        if values.len() > MAX_VALUES {
            return Err(format!(
                "too many values: this tool checks up to {MAX_VALUES} numbers per run"
            ));
        }
    }
    if values.is_empty() {
        return Err(
            "no numbers found — paste a list of numbers separated by commas, spaces, or newlines"
                .into(),
        );
    }
    Ok((values, ignored))
}

// ---------------------------------------------------------------------------
// Checking
// ---------------------------------------------------------------------------

fn in_order(prev: f64, cur: f64, ascending: bool, strict: bool) -> bool {
    match (ascending, strict) {
        (true, false) => cur >= prev,
        (true, true) => cur > prev,
        (false, false) => cur <= prev,
        (false, true) => cur < prev,
    }
}

/// Direction the values themselves imply: `Some(true)` = ascending,
/// `Some(false)` = descending, `None` = every value is equal.
fn detect_direction(values: &[f64]) -> Option<bool> {
    values
        .windows(2)
        .find(|w| w[0] != w[1])
        .map(|w| w[1] > w[0])
}

/// Run the full check and build the report.
pub fn check(input: &str, opts: &Options) -> Result<Report, String> {
    let (values, ignored_tokens) = parse_values(input, opts)?;

    let (ascending, direction) = match opts.order.as_str() {
        "ascending" => (true, "ascending".to_string()),
        "descending" => (false, "descending".to_string()),
        _ => match detect_direction(&values) {
            Some(true) => (true, "ascending".to_string()),
            Some(false) => (false, "descending".to_string()),
            // Every value equal: constant lists are sorted either way when ties
            // are allowed, and sorted neither way under strict.
            None => (true, "constant".to_string()),
        },
    };

    let mut breaks: Vec<Break> = Vec::new();
    let mut breaks_total = 0usize;
    let mut equal_adjacent = 0usize;
    let mut run_start = 1usize;
    let mut longest = Run {
        start: 1,
        end: 1,
        length: 1,
    };

    for i in 1..values.len() {
        let (prev, cur) = (values[i - 1], values[i]);
        if prev == cur {
            equal_adjacent += 1;
        }
        if in_order(prev, cur, ascending, opts.strict) {
            let length = i + 1 - run_start + 1;
            if length > longest.length {
                longest = Run {
                    start: run_start,
                    end: i + 1,
                    length,
                };
            }
            continue;
        }
        breaks_total += 1;
        run_start = i + 1;
        if breaks.len() < opts.max_issues {
            let (kind, reason) = if cur == prev {
                (
                    "equal",
                    format!(
                        "{} repeats the previous value (strict order requires a change at every step)",
                        fmt_num(cur)
                    ),
                )
            } else if ascending {
                (
                    "less_than",
                    format!(
                        "{} is less than the previous value {}",
                        fmt_num(cur),
                        fmt_num(prev)
                    ),
                )
            } else {
                (
                    "greater_than",
                    format!(
                        "{} is greater than the previous value {}",
                        fmt_num(cur),
                        fmt_num(prev)
                    ),
                )
            };
            breaks.push(Break {
                position: i + 1,
                value: cur,
                prev_position: i,
                prev_value: prev,
                kind: kind.to_string(),
                reason,
            });
        }
    }

    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    Ok(Report {
        sorted: breaks_total == 0,
        direction,
        requested_order: opts.order.clone(),
        strict: opts.strict,
        count: values.len(),
        comparisons: values.len().saturating_sub(1),
        first_break: breaks.first().cloned(),
        breaks,
        breaks_total,
        longest_run: longest,
        equal_adjacent,
        first: values[0],
        last: values[values.len() - 1],
        min,
        max,
        ignored_tokens,
    })
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Format a value the way a person wrote it: whole numbers without a `.0`.
pub fn fmt_num(v: f64) -> String {
    if v.is_infinite() {
        return if v > 0.0 { "infinity".into() } else { "-infinity".into() };
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn direction_phrase(report: &Report) -> String {
    match (report.direction.as_str(), report.strict) {
        ("constant", _) => "constant (every value is equal)".to_string(),
        (dir, true) => format!("strictly {dir}"),
        (dir, false) => format!("{dir} (equal neighbours allowed)"),
    }
}

/// Render the report as the plain-text page/CLI output.
pub fn render_text(report: &Report) -> String {
    let mut out = String::new();

    if report.sorted {
        if report.count == 1 {
            out.push_str("Sorted — a single value is trivially in order.\n");
        } else if report.direction == "constant" {
            out.push_str(&format!(
                "Sorted — all {} values are equal, so the list is in order either way.\n",
                report.count
            ));
        } else {
            out.push_str(&format!(
                "Sorted {} — all {} comparisons are in order.\n",
                direction_phrase(report),
                report.comparisons
            ));
        }
    } else {
        let first = report.first_break.as_ref().expect("unsorted implies a break");
        out.push_str(&format!(
            "Not sorted {} — the order first breaks at position {}.\n",
            direction_phrase(report),
            first.position
        ));
    }

    out.push('\n');
    out.push_str(&format!("Direction checked: {}\n", direction_phrase(report)));
    if report.requested_order == "auto" && report.direction != "constant" {
        out.push_str("Direction source: detected from the first two differing values\n");
    }
    out.push_str(&format!("Values checked: {}\n", report.count));
    out.push_str(&format!("Comparisons: {}\n", report.comparisons));

    if let Some(first) = report.first_break.as_ref() {
        out.push_str(&format!(
            "First out-of-order element: position {}, value {} (previous position {}, value {})\n",
            first.position,
            fmt_num(first.value),
            first.prev_position,
            fmt_num(first.prev_value)
        ));
        out.push_str(&format!("Reason: {}\n", first.reason));
        out.push_str(&format!(
            "Out-of-order elements: {} of {} comparisons\n",
            report.breaks_total, report.comparisons
        ));
        for (i, b) in report.breaks.iter().enumerate() {
            out.push_str(&format!(
                "  {}. position {}: {}\n",
                i + 1,
                b.position,
                b.reason
            ));
        }
        let hidden = report.breaks_total - report.breaks.len();
        if hidden > 0 {
            out.push_str(&format!(
                "  … and {hidden} more (raise max_issues to list them)\n"
            ));
        }
    }

    out.push_str(&format!(
        "Longest sorted run: positions {}-{} ({} values)\n",
        report.longest_run.start, report.longest_run.end, report.longest_run.length
    ));
    out.push_str(&format!("Equal adjacent pairs: {}\n", report.equal_adjacent));
    out.push_str(&format!("First value: {}\n", fmt_num(report.first)));
    out.push_str(&format!("Last value: {}\n", fmt_num(report.last)));
    out.push_str(&format!("Minimum: {}\n", fmt_num(report.min)));
    out.push_str(&format!("Maximum: {}\n", fmt_num(report.max)));
    if report.ignored_tokens > 0 {
        out.push_str(&format!(
            "Ignored non-numeric tokens: {}\n",
            report.ignored_tokens
        ));
    }

    out.trim_end().to_string()
}

/// Render the report as pretty JSON.
pub fn render_json(report: &Report) -> Result<String, String> {
    serde_json::to_string_pretty(report).map_err(|e| format!("could not serialize report: {e}"))
}

/// Full run: parse options, check, render in the requested format.
#[allow(clippy::too_many_arguments)]
pub fn run_with_options(
    numbers: &str,
    order: &str,
    strict: bool,
    separator: &str,
    strip_thousands: bool,
    non_numeric: &str,
    max_issues: u64,
    format: &str,
) -> Result<String, String> {
    let format = pick(format, &["text", "json"], "format")?;
    let opts = Options::from_raw(
        order,
        strict,
        separator,
        strip_thousands,
        non_numeric,
        max_issues,
    )?;
    let report = check(numbers, &opts)?;
    if format == "json" {
        render_json(&report)
    } else {
        Ok(render_text(&report))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn text(numbers: &str) -> String {
        run_with_options(numbers, "auto", false, "auto", false, "error", 20, "text").unwrap()
    }

    #[test]
    fn ascending_list_is_sorted() {
        let out = text("1, 2, 2, 5, 8");
        assert!(
            out.starts_with("Sorted ascending (equal neighbours allowed) — all 4 comparisons are in order."),
            "{out}"
        );
        assert!(out.contains("Equal adjacent pairs: 1"), "{out}");
        assert!(out.contains("Longest sorted run: positions 1-5 (5 values)"), "{out}");
    }

    #[test]
    fn descending_is_detected_by_auto() {
        let out = text("9 4 4 -2");
        assert!(out.starts_with("Sorted descending (equal neighbours allowed)"), "{out}");
        assert!(out.contains("Minimum: -2"), "{out}");
    }

    #[test]
    fn pinpoints_the_first_out_of_order_element() {
        let out = text("1, 3, 2, 7");
        assert!(
            out.starts_with("Not sorted ascending (equal neighbours allowed) — the order first breaks at position 3."),
            "{out}"
        );
        assert!(
            out.contains(
                "First out-of-order element: position 3, value 2 (previous position 2, value 3)"
            ),
            "{out}"
        );
        assert!(out.contains("Reason: 2 is less than the previous value 3"), "{out}");
        assert!(out.contains("Out-of-order elements: 1 of 3 comparisons"), "{out}");
        assert!(out.contains("Longest sorted run: positions 1-2 (2 values)"), "{out}");
    }

    #[test]
    fn strict_flags_repeated_values() {
        let out =
            run_with_options("5, 5, 9", "ascending", true, "auto", false, "error", 20, "text")
                .unwrap();
        assert!(out.starts_with("Not sorted strictly ascending"), "{out}");
        assert!(
            out.contains("Reason: 5 repeats the previous value (strict order requires a change at every step)"),
            "{out}"
        );
    }

    #[test]
    fn non_strict_constant_list_is_sorted() {
        let out = text("7 7 7");
        assert!(out.starts_with("Sorted — all 3 values are equal"), "{out}");
        assert!(out.contains("Direction checked: constant (every value is equal)"), "{out}");
    }

    #[test]
    fn single_value_is_trivially_sorted() {
        let out = text("42");
        assert!(out.starts_with("Sorted — a single value is trivially in order."), "{out}");
        assert!(out.contains("Comparisons: 0"), "{out}");
    }

    #[test]
    fn explicit_order_overrides_detection() {
        let out =
            run_with_options("9 4 1", "ascending", false, "auto", false, "error", 20, "text")
                .unwrap();
        assert!(out.starts_with("Not sorted ascending (equal neighbours allowed) — the order first breaks at position 2."), "{out}");
        assert!(out.contains("Out-of-order elements: 2 of 2 comparisons"), "{out}");
    }

    #[test]
    fn newline_and_decimal_and_scientific_values_parse() {
        let out = run_with_options(
            "-2.5\n.5\n1e3\n+2000",
            "ascending",
            false,
            "newline",
            false,
            "error",
            20,
            "text",
        )
        .unwrap();
        assert!(out.starts_with("Sorted ascending"), "{out}");
        assert!(out.contains("Minimum: -2.5"), "{out}");
        assert!(out.contains("Maximum: 2000"), "{out}");
    }

    #[test]
    fn thousands_separators_are_stripped_when_enabled() {
        let out = run_with_options(
            "1,024 2,048 4,096",
            "ascending",
            false,
            "space",
            true,
            "error",
            20,
            "text",
        )
        .unwrap();
        assert!(out.starts_with("Sorted ascending"), "{out}");
        assert!(out.contains("Values checked: 3"), "{out}");
        assert!(out.contains("Maximum: 4096"), "{out}");
    }

    #[test]
    fn separator_choices_split_as_advertised() {
        for (sep, input) in [
            ("comma", "1,2,3"),
            ("newline", "1\n2\n3"),
            ("space", "1 2 3"),
            ("semicolon", "1;2;3"),
            ("tab", "1\t2\t3"),
            ("pipe", "1|2|3"),
            ("auto", "1, 2 ; 3"),
        ] {
            let out =
                run_with_options(input, "ascending", false, sep, false, "error", 20, "text")
                    .unwrap();
            assert!(out.contains("Values checked: 3"), "{sep}: {out}");
        }
    }

    #[test]
    fn ignore_skips_non_numeric_tokens() {
        let out = run_with_options(
            "1, n/a, 4, 9",
            "ascending",
            false,
            "auto",
            false,
            "ignore",
            20,
            "text",
        )
        .unwrap();
        assert!(out.starts_with("Sorted ascending"), "{out}");
        assert!(out.contains("Ignored non-numeric tokens: 1"), "{out}");
    }

    #[test]
    fn max_issues_truncates_the_listing_but_not_the_total() {
        let out = run_with_options(
            "9, 1, 8, 2, 7, 3",
            "ascending",
            false,
            "auto",
            false,
            "error",
            2,
            "text",
        )
        .unwrap();
        assert!(out.contains("Out-of-order elements: 3 of 5 comparisons"), "{out}");
        assert!(out.contains("… and 1 more (raise max_issues to list them)"), "{out}");
    }

    #[test]
    fn json_format_carries_the_machine_readable_report() {
        let out =
            run_with_options("1, 3, 2", "auto", false, "auto", false, "error", 20, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["sorted"], serde_json::json!(false));
        assert_eq!(v["direction"], serde_json::json!("ascending"));
        assert_eq!(v["first_break"]["position"], serde_json::json!(3));
        assert_eq!(v["first_break"]["kind"], serde_json::json!("less_than"));
        assert_eq!(v["breaks_total"], serde_json::json!(1));
    }

    // ---- error paths ----

    #[test]
    fn non_numeric_token_errors_by_default() {
        let err = run_with_options("1, two, 3", "auto", false, "auto", false, "error", 20, "text")
            .unwrap_err();
        assert!(err.contains("token 2 is not a number: 'two'"), "{err}");
    }

    #[test]
    fn empty_input_errors() {
        let err = text_err("   ");
        assert!(err.contains("no numbers found"), "{err}");
    }

    #[test]
    fn nan_is_rejected_as_unorderable() {
        let err = text_err("1, NaN, 3");
        assert!(err.contains("not a number: 'NaN'"), "{err}");
    }

    #[test]
    fn unknown_enum_value_errors() {
        let err =
            run_with_options("1 2", "sideways", false, "auto", false, "error", 20, "text")
                .unwrap_err();
        assert!(err.contains("unknown order 'sideways'"), "{err}");
    }

    #[test]
    fn comma_separator_with_thousands_stripping_errors() {
        let err =
            run_with_options("1,024", "auto", false, "comma", true, "error", 20, "text")
                .unwrap_err();
        assert!(err.contains("cannot be used with separator=comma"), "{err}");
    }

    #[test]
    fn max_issues_out_of_range_errors() {
        let err =
            run_with_options("1 2", "auto", false, "auto", false, "error", 0, "text").unwrap_err();
        assert!(err.contains("max_issues must be between 1 and 1000"), "{err}");
    }

    #[test]
    fn value_cap_is_enforced() {
        let big = (0..=MAX_VALUES).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let err = text_err(&big);
        assert!(err.contains("checks up to 20000 numbers"), "{err}");

        let at_cap = (0..MAX_VALUES).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let out = text(&at_cap);
        assert!(out.contains("Values checked: 20000"), "{}", &out[..80]);
    }

    fn text_err(numbers: &str) -> String {
        run_with_options(numbers, "auto", false, "auto", false, "error", 20, "text").unwrap_err()
    }
}
