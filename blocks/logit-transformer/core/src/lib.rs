//! logit-transformer core — map a column of probabilities to log-odds
//! (`logit(p) = log(p / (1 - p))`) and map log-odds back to probabilities with
//! the inverse logit (sigmoid / expit).
//! No wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.

/// Maximum number of tokens accepted in one run (valid or not). Keeps the
/// output bounded for the chat/page surfaces — a spreadsheet column of 20,000
/// probabilities is well past the paste-a-column use case.
pub const MAX_VALUES: usize = 20_000;

/// Which way the transform runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Direction {
    /// Probability → log-odds: `log(p / (1 - p))`.
    Logit,
    /// Log-odds → probability: `1 / (1 + base^-x)`.
    Inverse,
}

impl Direction {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "logit" | "forward" | "to-logit" => Ok(Direction::Logit),
            "inverse" | "sigmoid" | "expit" | "logistic" | "to-probability" => {
                Ok(Direction::Inverse)
            }
            other => Err(format!(
                "unknown direction {other:?} — expected logit or inverse"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Direction::Logit => "logit",
            Direction::Inverse => "inverse",
        }
    }
}

/// Logarithm base shared by the forward and inverse transforms, so a
/// round-trip through both is exact for any supported base.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Base {
    /// Natural log — the statistical default.
    E,
    Two,
    Ten,
}

impl Base {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "e" | "E" | "natural" | "ln" => Ok(Base::E),
            "2" | "binary" => Ok(Base::Two),
            "10" | "common" => Ok(Base::Ten),
            other => Err(format!("unknown base {other:?} — expected e, 2, or 10")),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Base::E => "e",
            Base::Two => "2",
            Base::Ten => "10",
        }
    }

    fn log(self, x: f64) -> f64 {
        match self {
            Base::E => x.ln(),
            Base::Two => x.log2(),
            Base::Ten => x.log10(),
        }
    }

    fn exp(self, x: f64) -> f64 {
        match self {
            Base::E => x.exp(),
            Base::Two => x.exp2(),
            Base::Ten => 10f64.powf(x),
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

/// What to do with a probability that sits on (or outside) the open interval
/// the logit is defined on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OnBoundary {
    /// Stop the whole run with an error naming the offending value.
    Fail,
    /// Nudge `p` into `[epsilon, 1 - epsilon]` before transforming.
    Clamp,
    /// Drop the value entirely.
    Skip,
    /// Emit an empty value so the column still lines up.
    Blank,
    /// Emit `-Infinity` / `Infinity`, the mathematical limit.
    Infinity,
}

impl OnBoundary {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "fail" => Ok(OnBoundary::Fail),
            "clamp" => Ok(OnBoundary::Clamp),
            "skip" => Ok(OnBoundary::Skip),
            "blank" => Ok(OnBoundary::Blank),
            "infinity" | "inf" => Ok(OnBoundary::Infinity),
            other => Err(format!(
                "unknown on_boundary {other:?} — expected fail, clamp, skip, blank, or infinity"
            )),
        }
    }
}

/// Output shape.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OutputFormat {
    /// Transformed values only, joined by the output separator.
    Values,
    /// `input<TAB>odds<TAB>result` audit table with a header row.
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

/// One transformed row: the odds implied by the input and the result value.
struct Ok3 {
    odds: f64,
    value: f64,
}

/// One input token and what became of it.
struct Row {
    original: String,
    /// `Ok(row)` or `Err(reason)` for a token that could not be transformed.
    value: Result<Ok3, String>,
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
    let top = candidates.iter().map(|(_, n)| *n).max().unwrap_or(0);
    if top > 0 {
        // Ties resolve to the earliest candidate in declaration order.
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

/// True for a token like `3/1` or `1 / 4` — both sides are numbers, so the
/// user almost certainly pasted odds or a fraction rather than a typo.
fn looks_like_ratio(t: &str) -> bool {
    match t.split_once('/') {
        Some((a, b)) => a.trim().parse::<f64>().is_ok() && b.trim().parse::<f64>().is_ok(),
        None => false,
    }
}

/// Parse one token as a finite decimal number. A trailing `%` is accepted and
/// divided by 100, so `90%` and `0.9` mean the same probability.
fn parse_number(token: &str) -> Result<f64, String> {
    let t = token.trim();
    let (body, percent) = match t.strip_suffix('%') {
        Some(rest) => (rest.trim(), true),
        None => (t, false),
    };
    match body.parse::<f64>() {
        Ok(n) if n.is_finite() => Ok(if percent { n / 100.0 } else { n }),
        Ok(_) => Err(format!(
            "{t:?} is not a finite number — infinity and NaN cannot be transformed"
        )),
        Err(_) if looks_like_ratio(t) => Err(format!(
            "{t:?} looks like an odds ratio — convert it to a probability first, so 3/1 becomes 0.75"
        )),
        Err(_) => Err(format!(
            "{t:?} is not a number — strip currency symbols, thousands separators, and units first"
        )),
    }
}

/// Format a value. With `decimals = None` whole numbers print without a
/// trailing `.0`; with `Some(n)` every value gets exactly `n` places.
/// Infinities always print as `Infinity` / `-Infinity`.
fn format_value(v: f64, decimals: Option<u32>) -> String {
    if v.is_infinite() {
        return if v > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
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

/// JSON numbers cannot be infinite — emit those as quoted strings instead.
fn json_number(v: f64, decimals: Option<u32>) -> String {
    if v.is_infinite() {
        format!("\"{}\"", format_value(v, decimals))
    } else {
        format_value(v, decimals)
    }
}

/// Transform a single probability into log-odds, honouring the boundary policy.
fn forward(p: f64, base: Base, policy: OnBoundary, epsilon: f64) -> Result<Ok3, String> {
    // Range-check BEFORE clamping, so `clamp` only rescues the two undefined
    // endpoints and never silently turns a bad value like 90 into 1 - epsilon.
    if p < 0.0 || p > 1.0 {
        return Err(format!(
            "probability {} is outside 0-1 — percentages need a % suffix (for example 90%), and odds must be converted to a probability first",
            format_value(p, None)
        ));
    }
    let p = if policy == OnBoundary::Clamp {
        p.clamp(epsilon, 1.0 - epsilon)
    } else {
        p
    };
    if p == 0.0 || p == 1.0 {
        let signed = if p == 0.0 {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
        return match policy {
            OnBoundary::Infinity => Ok(Ok3 {
                odds: if p == 0.0 { 0.0 } else { f64::INFINITY },
                value: signed,
            }),
            _ => Err(format!(
                "logit is undefined at p = {} — the log-odds are {}; use the boundary option to clamp, skip, blank, or emit infinity",
                format_value(p, None),
                if p == 0.0 { "-Infinity" } else { "Infinity" }
            )),
        };
    }
    let odds = p / (1.0 - p);
    Ok(Ok3 {
        odds,
        value: base.log(odds),
    })
}

/// Transform a single log-odds value back into a probability. The inverse is
/// defined on the whole real line, so it never hits the boundary policy.
fn inverse(x: f64, base: Base) -> Ok3 {
    let odds = base.exp(x);
    // `odds / (1 + odds)` overflows to NaN once `odds` is infinite; the limit
    // is 1, so special-case it.
    let p = if odds.is_infinite() {
        1.0
    } else {
        odds / (1.0 + odds)
    };
    Ok3 { odds, value: p }
}

/// Apply the logit transform (or its inverse) to every value in `data`.
///
/// - `data`: the column of values (separated per `separator`). In the `logit`
///   direction these are probabilities in 0-1 (a `%` suffix is accepted); in
///   the `inverse` direction they are log-odds on the whole real line.
/// - `direction`: `logit` (default) | `inverse`.
/// - `base`: `e` (default) | `2` | `10` — used by both directions, so a
///   round-trip is exact for any base.
/// - `separator`: `auto` (default) | `newline` | `comma` | `space` |
///   `semicolon` | `tab` | `pipe` — how the input is split.
/// - `output_separator`: `same` (default, mirrors the input) or one of the
///   separator names above.
/// - `on_boundary`: `fail` (default) | `clamp` | `skip` | `blank` | `infinity`
///   for probabilities of exactly 0 or 1, where the logit is undefined.
/// - `epsilon`: the clamp distance used by `on_boundary = clamp`.
/// - `decimals`: `None` keeps full precision, `Some(n)` rounds to `n` places.
/// - `output`: `values` (default) | `table` | `json`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    direction: &str,
    base: &str,
    separator: &str,
    output_separator: &str,
    on_boundary: &str,
    epsilon: f64,
    decimals: Option<u32>,
    output: &str,
) -> Result<String, String> {
    let dir = Direction::parse(direction)?;
    let base = Base::parse(base)?;
    let in_sep = Sep::parse(separator, "separator")?;
    let out_sep_opt = match output_separator.trim() {
        "" | "same" | "auto" => None,
        other => Some(Sep::parse(other, "output_separator")?),
    };
    let policy = OnBoundary::parse(on_boundary)?;
    let fmt = OutputFormat::parse(output)?;
    if let Some(n) = decimals {
        if n > 8 {
            return Err(format!("decimals must be auto or 0-8 (got {n})"));
        }
    }
    if !epsilon.is_finite() || epsilon <= 0.0 || epsilon >= 0.5 {
        return Err(format!(
            "epsilon must be greater than 0 and less than 0.5 (got {})",
            format_value(epsilon, None)
        ));
    }

    let tokens = split_tokens(data, in_sep);
    if tokens.is_empty() {
        return Err(match dir {
            Direction::Logit => {
                "no values found — paste a column of probabilities between 0 and 1, one per line"
            }
            Direction::Inverse => {
                "no values found — paste a column of log-odds values, one per line"
            }
        }
        .into());
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
            Ok(n) => match dir {
                Direction::Logit => forward(n, base, policy, epsilon),
                Direction::Inverse => Ok(inverse(n, base)),
            },
            Err(e) => Err(e),
        };
        // `skip`/`blank` absorb anything untransformable; every other policy
        // still fails loudly on a bad token (`clamp`/`infinity` only rescue the
        // two undefined endpoints, which `forward` has already handled).
        if let Err(e) = &value {
            if !matches!(policy, OnBoundary::Skip | OnBoundary::Blank) {
                return Err(format!("value {}: {e}", i + 1));
            }
        }
        rows.push(Row {
            original: (*token).to_string(),
            value,
        });
    }

    let out_sep = out_sep_opt.unwrap_or(match in_sep {
        Sep::Auto => detect_sep(data),
        other => other,
    });

    let (in_header, out_header) = match dir {
        Direction::Logit => ("probability", "logit"),
        Direction::Inverse => ("logit", "probability"),
    };

    Ok(match fmt {
        OutputFormat::Values => {
            let mut cells: Vec<String> = Vec::with_capacity(rows.len());
            for row in &rows {
                match (&row.value, policy) {
                    (Ok(v), _) => cells.push(format_value(v.value, decimals)),
                    (Err(_), OnBoundary::Skip) => {}
                    // `Fail` already returned above; the rest keep alignment.
                    (Err(_), _) => cells.push(String::new()),
                }
            }
            cells.join(out_sep.joiner())
        }
        OutputFormat::Table => {
            let mut out = format!("{in_header}\todds\t{out_header}");
            for row in &rows {
                match (&row.value, policy) {
                    (Ok(v), _) => out.push_str(&format!(
                        "\n{}\t{}\t{}",
                        row.original,
                        format_value(v.odds, decimals),
                        format_value(v.value, decimals)
                    )),
                    (Err(_), OnBoundary::Skip) => {}
                    (Err(_), _) => out.push_str(&format!("\n{}\t\t", row.original)),
                }
            }
            out
        }
        OutputFormat::Json => {
            let transformed = rows.iter().filter(|r| r.value.is_ok()).count();
            let invalid = rows.len() - transformed;
            let mut out = String::from("{\n");
            out.push_str(&format!("  \"direction\": \"{}\",\n", dir.label()));
            out.push_str(&format!("  \"base\": \"{}\",\n", base.label()));
            out.push_str(&format!("  \"count\": {},\n", rows.len()));
            out.push_str(&format!("  \"transformed\": {transformed},\n"));
            out.push_str(&format!("  \"invalid\": {invalid},\n"));
            out.push_str("  \"values\": [");
            let mut first = true;
            for row in &rows {
                if row.value.is_err() && policy == OnBoundary::Skip {
                    continue;
                }
                if !first {
                    out.push(',');
                }
                first = false;
                out.push_str("\n    {");
                out.push_str(&format!(
                    "\"{in_header}\": \"{}\", ",
                    json_escape(&row.original)
                ));
                match &row.value {
                    Ok(v) => out.push_str(&format!(
                        "\"odds\": {}, \"{out_header}\": {}",
                        json_number(v.odds, decimals),
                        json_number(v.value, decimals)
                    )),
                    Err(e) => out.push_str(&format!(
                        "\"odds\": null, \"{out_header}\": null, \"error\": \"{}\"",
                        json_escape(e)
                    )),
                }
                out.push('}');
            }
            out.push_str("\n  ]\n}");
            out
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-6;

    fn logit(data: &str) -> String {
        run(data, "logit", "e", "auto", "same", "fail", EPS, None, "values").unwrap()
    }

    fn logit_at(data: &str, decimals: u32) -> String {
        run(
            data,
            "logit",
            "e",
            "auto",
            "same",
            "fail",
            EPS,
            Some(decimals),
            "values",
        )
        .unwrap()
    }

    #[test]
    fn logit_of_a_newline_column() {
        assert_eq!(logit_at("0.5\n0.75\n0.25", 4), "0.0000\n1.0986\n-1.0986");
    }

    #[test]
    fn logit_of_one_half_is_exactly_zero() {
        assert_eq!(logit("0.5"), "0");
    }

    #[test]
    fn inverse_maps_log_odds_back_to_probabilities() {
        let out = run(
            "0\n1\n-1",
            "inverse",
            "e",
            "newline",
            "same",
            "fail",
            EPS,
            Some(4),
            "values",
        )
        .unwrap();
        assert_eq!(out, "0.5000\n0.7311\n0.2689");
    }

    #[test]
    fn round_trip_is_exact_for_every_base() {
        for base in ["e", "2", "10"] {
            let fwd = run(
                "0.1\n0.42\n0.9", "logit", base, "newline", "same", "fail", EPS, None, "values",
            )
            .unwrap();
            let back = run(
                &fwd, "inverse", base, "newline", "same", "fail", EPS, Some(6), "values",
            )
            .unwrap();
            assert_eq!(back, "0.100000\n0.420000\n0.900000", "base {base}");
        }
    }

    #[test]
    fn base_ten_logit_is_log10_of_the_odds() {
        let out = run(
            "0.9", "logit", "10", "newline", "same", "fail", EPS, Some(6), "values",
        )
        .unwrap();
        // log10(0.9 / 0.1) = log10(9) = 0.954243
        assert_eq!(out, "0.954243");
    }

    #[test]
    fn percent_suffix_is_accepted() {
        assert_eq!(logit_at("90%\n0.9", 6), "2.197225\n2.197225");
    }

    #[test]
    fn boundary_fails_by_default() {
        let err = run(
            "0.5\n1", "logit", "e", "newline", "same", "fail", EPS, None, "values",
        )
        .unwrap_err();
        assert!(err.contains("value 2"), "{err}");
        assert!(err.contains("undefined at p = 1"), "{err}");
    }

    #[test]
    fn boundary_clamp_uses_epsilon() {
        let out = run(
            "0\n1", "logit", "e", "newline", "same", "clamp", 1e-6, Some(4), "values",
        )
        .unwrap();
        assert_eq!(out, "-13.8155\n13.8155");
    }

    #[test]
    fn boundary_skip_blank_and_infinity() {
        let out = |policy| {
            run(
                "0\n0.5\n1", "logit", "e", "newline", "newline", policy, EPS, Some(2), "values",
            )
            .unwrap()
        };
        // `skip` drops both undefined endpoints; `blank` keeps their rows so the
        // column still lines up with the input.
        assert_eq!(out("skip"), "0.00");
        assert_eq!(out("blank"), "\n0.00\n");
        assert_eq!(out("infinity"), "-Infinity\n0.00\nInfinity");
    }

    #[test]
    fn out_of_range_probability_is_rejected_with_a_hint() {
        let err = logit_err("90");
        assert!(err.contains("outside 0-1"), "{err}");
        assert!(err.contains('%'), "{err}");
    }

    #[test]
    fn non_numeric_token_is_rejected() {
        let err = run(
            "0.5\nn/a", "logit", "e", "newline", "same", "fail", EPS, None, "values",
        )
        .unwrap_err();
        assert!(err.contains("is not a number"), "{err}");
    }

    #[test]
    fn odds_ratio_input_is_rejected_clearly() {
        let err = run(
            "3/1", "logit", "e", "newline", "same", "fail", EPS, None, "values",
        )
        .unwrap_err();
        assert!(err.contains("odds ratio"), "{err}");
    }

    #[test]
    fn epsilon_must_be_inside_the_unit_interval() {
        let err = run(
            "0.5", "logit", "e", "newline", "same", "clamp", 0.0, None, "values",
        )
        .unwrap_err();
        assert!(err.contains("epsilon must be"), "{err}");
        let err = run(
            "0.5", "logit", "e", "newline", "same", "clamp", 0.9, None, "values",
        )
        .unwrap_err();
        assert!(err.contains("epsilon must be"), "{err}");
    }

    #[test]
    fn unknown_options_are_named() {
        assert!(run("0.5", "sideways", "e", "auto", "same", "fail", EPS, None, "values")
            .unwrap_err()
            .contains("unknown direction"));
        assert!(run("0.5", "logit", "7", "auto", "same", "fail", EPS, None, "values")
            .unwrap_err()
            .contains("unknown base"));
        assert!(
            run("0.5", "logit", "e", "auto", "same", "sometimes", EPS, None, "values")
                .unwrap_err()
                .contains("unknown on_boundary")
        );
        assert!(run("0.5", "logit", "e", "auto", "same", "fail", EPS, None, "csv")
            .unwrap_err()
            .contains("unknown output"));
    }

    #[test]
    fn empty_input_is_rejected() {
        assert!(logit_err("   ").contains("no values found"));
    }

    #[test]
    fn table_output_carries_the_odds_column() {
        let out = run(
            "0.75", "logit", "e", "newline", "same", "fail", EPS, Some(4), "table",
        )
        .unwrap();
        assert_eq!(out, "probability\todds\tlogit\n0.75\t3.0000\t1.0986");
    }

    #[test]
    fn json_output_reports_counts_and_flips_headers_on_inverse() {
        let out = run(
            "1", "inverse", "e", "newline", "same", "fail", EPS, Some(4), "json",
        )
        .unwrap();
        assert!(out.contains("\"direction\": \"inverse\""), "{out}");
        assert!(out.contains("\"transformed\": 1"), "{out}");
        assert!(out.contains("\"probability\": 0.7311"), "{out}");
    }

    #[test]
    fn separators_are_detected_and_mirrored() {
        // Explicit in, explicit out.
        let out = run(
            "0.5;0.5", "logit", "e", "semicolon", "pipe", "fail", EPS, Some(1), "values",
        )
        .unwrap();
        assert_eq!(out, "0.0|0.0");
        // `auto` in + `same` out mirrors whichever separator the paste used.
        assert_eq!(logit_at("0.5,0.75", 4), "0.0000,1.0986");
        assert_eq!(logit_at("0.5\t0.75", 4), "0.0000\t1.0986");
        assert_eq!(logit_at("0.5 0.75", 4), "0.0000 1.0986");
    }

    #[test]
    fn cap_is_enforced() {
        let over = vec!["0.5"; MAX_VALUES + 1].join("\n");
        assert!(logit_err(&over).contains("exceeds the maximum"));
        let at = vec!["0.5"; MAX_VALUES].join("\n");
        assert_eq!(logit_at(&at, 0).split('\n').count(), MAX_VALUES);
    }

    fn logit_err(data: &str) -> String {
        run(data, "logit", "e", "auto", "same", "fail", EPS, None, "values").unwrap_err()
    }
}
