//! gizza-ai/extract-numbers-from-text core — pull every numeric value out of
//! arbitrary prose or logs into a clean list. Recognises integers, decimals,
//! scientific notation, signed numbers, and thousands-separator-formatted
//! numbers (e.g. `1,000,000`). Optional de-duplication, sorting, choice of
//! output delimiter, and summary statistics. Pure-Rust (`regex`).

use regex::Regex;
use serde::Serialize;

/// Which numbers to keep, by textual form.
///
/// A token counts as a *decimal* if it contains a `.`; otherwise it is an
/// *integer* (this includes scientific notation without a dot, e.g. `1e3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Keep every number found.
    All,
    /// Keep only tokens with no decimal point.
    Integers,
    /// Keep only tokens containing a decimal point.
    Decimals,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "all" => Ok(Mode::All),
            "integers" | "integer" | "int" => Ok(Mode::Integers),
            "decimals" | "decimal" | "float" | "floats" => Ok(Mode::Decimals),
            other => Err(format!(
                "unknown mode {other:?}; expected one of: all, integers, decimals"
            )),
        }
    }
}

/// Output sort order for the extracted numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    /// Keep first-seen order from the source text.
    Original,
    /// Ascending by numeric value.
    Ascending,
    /// Descending by numeric value.
    Descending,
}

impl Sort {
    pub fn parse(s: &str) -> Result<Sort, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "original" | "none" => Ok(Sort::Original),
            "ascending" | "asc" => Ok(Sort::Ascending),
            "descending" | "desc" => Ok(Sort::Descending),
            other => Err(format!(
                "unknown sort {other:?}; expected one of: original, ascending, descending"
            )),
        }
    }
}

/// How to join the extracted numbers in the rendered output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    Newline,
    Comma,
    Space,
    Tab,
    Semicolon,
}

impl Delimiter {
    pub fn parse(s: &str) -> Result<Delimiter, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "newline" | "line" | "\n" => Ok(Delimiter::Newline),
            "comma" => Ok(Delimiter::Comma),
            "space" => Ok(Delimiter::Space),
            "tab" => Ok(Delimiter::Tab),
            "semicolon" => Ok(Delimiter::Semicolon),
            other => Err(format!(
                "unknown delimiter {other:?}; expected one of: newline, comma, space, tab, semicolon"
            )),
        }
    }

    fn joiner(self) -> &'static str {
        match self {
            Delimiter::Newline => "\n",
            Delimiter::Comma => ", ",
            Delimiter::Space => " ",
            Delimiter::Tab => "\t",
            Delimiter::Semicolon => "; ",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Extracted {
    /// Number of values returned (after filtering + optional de-duplication).
    pub count: usize,
    /// The number tokens, exactly as they appeared in the text (any applied
    /// unary sign is preserved), post-filter/dedupe/sort.
    pub numbers: Vec<String>,
    /// Sum of the returned values (`None` when the list is empty).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sum: Option<f64>,
    /// Smallest value (`None` when the list is empty).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Largest value (`None` when the list is empty).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// Arithmetic mean of the values (`None` when the list is empty).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average: Option<f64>,
}

thread_local! {
    // Matches an UNSIGNED number core: a thousands-grouped integer or a plain
    // digit run, an optional fractional part (or a bare `.5`), and an optional
    // exponent. The leading sign is handled in code (`regex` has no lookbehind)
    // so a hyphen glued to a preceding digit — e.g. in `2024-01-15` — is treated
    // as a separator, not a negative sign.
    static NUM: Regex = Regex::new(
        r"(?:\d{1,3}(?:,\d{3})+|\d+)(?:\.\d+)?(?:[eE][+-]?\d+)?|\.\d+(?:[eE][+-]?\d+)?",
    )
    .unwrap();
}

/// Parse a matched token (commas stripped) into an `f64`.
fn value_of(token: &str) -> Option<f64> {
    token.replace(',', "").parse::<f64>().ok()
}

/// Canonical de-duplication key: commas removed, a leading `+` dropped, and
/// lowercased, so `1,000` == `1000` and `+5` == `5` and `1E3` == `1e3`. Keys
/// are textual, so `5` and `5e0` (which look different) are NOT merged.
fn dedupe_key(token: &str) -> String {
    let mut k = token.replace(',', "").to_ascii_lowercase();
    if let Some(rest) = k.strip_prefix('+') {
        k = rest.to_string();
    }
    k
}

/// Extract numbers from `text`.
///
/// - `mode`: keep all numbers, only integers, or only decimals.
/// - `unique`: drop duplicate values (first-seen wins).
/// - `sort`: output ordering.
pub fn extract(text: &str, mode: Mode, unique: bool, sort: Sort) -> Extracted {
    let mut tokens: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    NUM.with(|re| {
        for m in re.find_iter(text) {
            let start = m.start();
            let core = m.as_str();
            let has_dot = core.contains('.');

            // Decide whether a unary sign applies: the char immediately before
            // the match must be `-`/`+`, and the char before THAT must not be a
            // letter or digit (so `a-5`, `5-3`, `2024-01` don't become negatives).
            let before = &text[..start];
            let token = {
                let mut chars = before.chars().rev();
                match chars.next() {
                    Some(s @ ('-' | '+')) => {
                        let prev = chars.next();
                        let is_unary = !matches!(prev, Some(c) if c.is_ascii_alphanumeric());
                        if is_unary {
                            format!("{s}{core}")
                        } else {
                            core.to_string()
                        }
                    }
                    _ => core.to_string(),
                }
            };

            match mode {
                Mode::All => {}
                Mode::Integers if has_dot => continue,
                Mode::Decimals if !has_dot => continue,
                _ => {}
            }

            if unique && !seen.insert(dedupe_key(&token)) {
                continue;
            }
            tokens.push(token);
        }
    });

    // Sort by numeric value if requested (stable, so equal values keep order).
    match sort {
        Sort::Original => {}
        Sort::Ascending => tokens.sort_by(|a, b| {
            value_of(a)
                .partial_cmp(&value_of(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        Sort::Descending => tokens.sort_by(|a, b| {
            value_of(b)
                .partial_cmp(&value_of(a))
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
    }

    let values: Vec<f64> = tokens.iter().filter_map(|t| value_of(t)).collect();
    let (sum, min, max, average) = if values.is_empty() {
        (None, None, None, None)
    } else {
        let sum: f64 = values.iter().sum();
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        (Some(sum), Some(min), Some(max), Some(sum / values.len() as f64))
    };

    Extracted {
        count: tokens.len(),
        numbers: tokens,
        sum,
        min,
        max,
        average,
    }
}

/// Format an `f64` for the stats block without a trailing `.0` on whole numbers.
fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.6}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    }
}

/// Human-readable rendering (used by the page).
pub fn render(
    text: &str,
    mode: Mode,
    unique: bool,
    sort: Sort,
    delimiter: Delimiter,
    stats: bool,
) -> String {
    let r = extract(text, mode, unique, sort);
    if r.count == 0 {
        return "No numbers found.".to_string();
    }
    let mut out = r.numbers.join(delimiter.joiner());
    if stats {
        out.push_str("\n\n");
        out.push_str(&format!("Count: {}\n", r.count));
        if let (Some(sum), Some(min), Some(max), Some(avg)) = (r.sum, r.min, r.max, r.average) {
            out.push_str(&format!("Sum: {}\n", fmt_num(sum)));
            out.push_str(&format!("Min: {}\n", fmt_num(min)));
            out.push_str(&format!("Max: {}\n", fmt_num(max)));
            out.push_str(&format!("Average: {}", fmt_num(avg)));
        }
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_mixed_numbers() {
        let t = "Order 42 shipped for $1,299.99 on 2024. Weight 3.5kg, ref -7.";
        let r = extract(t, Mode::All, false, Sort::Original);
        assert_eq!(r.numbers, vec!["42", "1,299.99", "2024", "3.5", "-7"]);
        assert_eq!(r.count, 5);
        assert_eq!(r.min, Some(-7.0));
        assert_eq!(r.max, Some(2024.0));
    }

    #[test]
    fn scientific_signed_and_separators() {
        let r = extract(
            "Values: 6.022e23, 1.6E-19, -2e-3, +1,000.",
            Mode::All,
            false,
            Sort::Original,
        );
        assert_eq!(r.numbers, vec!["6.022e23", "1.6E-19", "-2e-3", "+1,000"]);
    }

    #[test]
    fn dates_do_not_become_negatives() {
        // The hyphens in a date must not be read as unary minus signs.
        let r = extract("meeting 2024-01-15 at 9", Mode::All, false, Sort::Original);
        assert_eq!(r.numbers, vec!["2024", "01", "15", "9"]);
    }

    #[test]
    fn integers_only_and_decimals_only() {
        let ints = extract("1 2.5 3 4.0 5e2", Mode::Integers, false, Sort::Original);
        assert_eq!(ints.numbers, vec!["1", "3", "5e2"]);
        let decs = extract("1 2.5 3 4.0 5e2", Mode::Decimals, false, Sort::Original);
        assert_eq!(decs.numbers, vec!["2.5", "4.0"]);
    }

    #[test]
    fn dedupe_by_value_ignoring_grouping_and_sign() {
        let r = extract("1000 1,000 +5 5 5", Mode::All, true, Sort::Original);
        assert_eq!(r.numbers, vec!["1000", "+5"]);
    }

    #[test]
    fn sort_ascending_and_descending() {
        let asc = extract("10 -3 2.5 100", Mode::All, false, Sort::Ascending);
        assert_eq!(asc.numbers, vec!["-3", "2.5", "10", "100"]);
        let desc = extract("10 -3 2.5 100", Mode::All, false, Sort::Descending);
        assert_eq!(desc.numbers, vec!["100", "10", "2.5", "-3"]);
    }

    #[test]
    fn no_numbers_found() {
        let r = extract("just some words, no digits here", Mode::All, false, Sort::Original);
        assert_eq!(r.count, 0);
        assert!(r.numbers.is_empty());
        assert_eq!(r.sum, None);
        assert_eq!(
            render("no digits", Mode::All, false, Sort::Original, Delimiter::Newline, true),
            "No numbers found."
        );
    }

    #[test]
    fn parse_rejects_bad_enums() {
        assert!(Mode::parse("nope").is_err());
        assert!(Sort::parse("sideways").is_err());
        assert!(Delimiter::parse("pipe").is_err());
    }

    #[test]
    fn render_with_stats_and_delimiter() {
        let out = render("a 1 b 2 c 3", Mode::All, false, Sort::Original, Delimiter::Comma, true);
        assert!(out.starts_with("1, 2, 3"));
        assert!(out.contains("Count: 3"));
        assert!(out.contains("Sum: 6"));
        assert!(out.contains("Average: 2"));
    }

    #[test]
    fn leading_decimal_and_percent_context() {
        let r = extract("grew by .5 and 25% then", Mode::All, false, Sort::Original);
        assert_eq!(r.numbers, vec![".5", "25"]);
    }
}
