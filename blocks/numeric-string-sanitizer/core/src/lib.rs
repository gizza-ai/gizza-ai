//! numeric-string-sanitizer core — pure compute, shared by the chat skill block and the web page.
//! Turns messy formatted number cells ("$1,234.50 USD", "(250,00) €", "1.2K", "45.2%", "1 234,56")
//! into plain machine-readable floats, one output row per input row.
//! No wafer/wasm-bindgen deps.

/// Hard cap on how many values one run will sanitize.
pub const MAX_VALUES: usize = 20_000;

/// Which character separates the fractional part.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DecimalSep {
    /// Infer one convention for the whole column from the values that are unambiguous.
    Auto,
    /// `1,234.56` — dot is the decimal mark, comma groups thousands.
    Dot,
    /// `1.234,56` — comma is the decimal mark, dot groups thousands.
    Comma,
}

impl DecimalSep {
    pub fn parse(s: &str) -> Result<DecimalSep, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(DecimalSep::Auto),
            "dot" | "." | "period" => Ok(DecimalSep::Dot),
            "comma" | "," => Ok(DecimalSep::Comma),
            other => Err(format!(
                "unknown decimal_separator `{other}` — expected auto, dot, or comma"
            )),
        }
    }
}

/// What a trailing `%` should do to the value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PercentMode {
    /// `45.2%` → `45.2` (the sign is only formatting).
    Strip,
    /// `45.2%` → `0.452` (the sign means "per hundred").
    Divide,
}

impl PercentMode {
    pub fn parse(s: &str) -> Result<PercentMode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "strip" | "keep" => Ok(PercentMode::Strip),
            "divide" | "fraction" => Ok(PercentMode::Divide),
            other => Err(format!(
                "unknown percent `{other}` — expected strip or divide"
            )),
        }
    }
}

/// What to emit for a row that cannot be parsed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OnError {
    /// Emit an empty line (keeps the column aligned with the source).
    Blank,
    /// Emit the original text unchanged.
    Keep,
    /// Emit the `#ERROR` marker.
    Marker,
    /// Abort the whole run with the first row's reason.
    Fail,
}

impl OnError {
    pub fn parse(s: &str) -> Result<OnError, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "blank" => Ok(OnError::Blank),
            "keep" | "original" => Ok(OnError::Keep),
            "marker" => Ok(OnError::Marker),
            "fail" | "error" => Ok(OnError::Fail),
            other => Err(format!(
                "unknown on_error `{other}` — expected blank, keep, marker, or fail"
            )),
        }
    }
}

/// How the result is rendered.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    /// One cleaned value per line — paste straight back into a spreadsheet column.
    Values,
    /// Tab-separated `original`, `value`, `status` audit table with a header row.
    Table,
    /// Structured JSON with per-row status and totals.
    Json,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<OutputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "values" | "text" => Ok(OutputFormat::Values),
            "table" | "tsv" => Ok(OutputFormat::Table),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!(
                "unknown output `{other}` — expected values, table, or json"
            )),
        }
    }
}

/// Options for one sanitize run.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub decimal_separator: DecimalSep,
    pub percent: PercentMode,
    /// Expand magnitude suffixes: `1.2K` → 1200, `3M` → 3000000, `2B`/`2bn`, `1T`.
    pub magnitude_suffixes: bool,
    /// Read accounting parentheses as a negative sign: `(250.00)` → -250.
    pub parentheses_negative: bool,
    /// Round every value to this many decimals; `None` keeps full precision.
    pub decimals: Option<u32>,
    pub on_error: OnError,
    pub output: OutputFormat,
    /// Append a count/sum/min/max/average summary.
    pub stats: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            decimal_separator: DecimalSep::Auto,
            percent: PercentMode::Strip,
            magnitude_suffixes: true,
            parentheses_negative: true,
            decimals: None,
            on_error: OnError::Blank,
            output: OutputFormat::Values,
            stats: false,
        }
    }
}

/// Outcome for a single input row.
#[derive(Clone, Debug, PartialEq)]
pub enum RowStatus {
    Ok(f64),
    Empty,
    Error(String),
}

/// One input row plus its outcome.
#[derive(Clone, Debug)]
pub struct Row {
    pub original: String,
    pub status: RowStatus,
}

/// Full result of a sanitize run.
#[derive(Clone, Debug)]
pub struct Sanitized {
    pub rows: Vec<Row>,
    /// The decimal mark actually used (after auto-inference).
    pub decimal_used: char,
    pub parsed: usize,
    pub empty: usize,
    pub failed: usize,
}

/// Space-like characters that show up between a currency symbol and its digits, or as
/// thousands separators in fr/sv/fi exports (NBSP, narrow NBSP, figure space, thin space)
/// plus zero-width junk pasted out of PDFs and web tables.
fn is_space_like(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '\u{00a0}'
                | '\u{2007}'
                | '\u{2008}'
                | '\u{2009}'
                | '\u{202f}'
                | '\u{200b}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{feff}'
        )
}

fn is_minus(c: char) -> bool {
    matches!(
        c,
        '-' | '\u{2212}' | '\u{2013}' | '\u{2014}' | '\u{fe63}' | '\u{ff0d}'
    )
}

/// Grouping characters that are never a decimal mark: Swiss apostrophes, programmer
/// underscores, and every flavour of space.
fn is_group_only(c: char) -> bool {
    matches!(c, '\'' | '\u{2019}' | '_') || is_space_like(c)
}

fn is_percent(c: char) -> bool {
    matches!(c, '%' | '\u{066a}' | '\u{fe6a}' | '\u{ff05}')
}

/// Trim leading/trailing whitespace including the unicode spaces Excel's TRIM misses.
fn trim_spaces(s: &str) -> &str {
    s.trim_matches(is_space_like)
}

/// Digits + separators that may appear inside the numeric body of a value.
fn is_core_char(c: char) -> bool {
    c.is_ascii_digit() || c == '.' || c == ',' || is_group_only(c)
}

/// Split a trimmed value into (prefix, core, suffix) around its first and last digit.
fn split_core(s: &str) -> Option<(&str, &str, &str)> {
    let first = s.char_indices().find(|(_, c)| c.is_ascii_digit())?.0;
    let last = s
        .char_indices()
        .filter(|(_, c)| c.is_ascii_digit())
        .next_back()?;
    let end = last.0 + last.1.len_utf8();
    Some((&s[..first], &s[first..end], &s[end..]))
}

/// Strip one layer of accounting parentheses, reporting whether it found them.
fn strip_parens(s: &str) -> (&str, bool) {
    let t = trim_spaces(s);
    if t.len() >= 2 && t.starts_with('(') && t.ends_with(')') {
        (trim_spaces(&t[1..t.len() - 1]), true)
    } else if t.len() >= 2 && t.starts_with('[') && t.ends_with(']') {
        (trim_spaces(&t[1..t.len() - 1]), true)
    } else {
        (t, false)
    }
}

/// Column-level inference: let the values that are unambiguous decide the convention for
/// the ambiguous ones, so a single `1.234` in a European column is not read as 1.234.
fn infer_decimal(values: &[&str]) -> char {
    let (mut dot_votes, mut comma_votes) = (0usize, 0usize);
    for raw in values {
        let (body, _) = strip_parens(trim_spaces(raw));
        let core = match split_core(body) {
            Some((_, core, _)) => core,
            None => continue,
        };
        let cleaned: String = core.chars().filter(|c| !is_group_only(*c)).collect();
        let dots = cleaned.matches('.').count();
        let commas = cleaned.matches(',').count();
        match (dots, commas) {
            (0, 0) => {}
            (_, 0) => {
                if dots > 1 {
                    comma_votes += 1;
                } else if tail_len(&cleaned, '.') != 3 {
                    dot_votes += 1;
                }
            }
            (0, _) => {
                if commas > 1 {
                    dot_votes += 1;
                } else if tail_len(&cleaned, ',') != 3 {
                    comma_votes += 1;
                }
            }
            _ => {
                // Both present: whichever comes last is the decimal mark.
                let last_dot = cleaned.rfind('.').unwrap();
                let last_comma = cleaned.rfind(',').unwrap();
                if last_dot > last_comma {
                    dot_votes += 1;
                } else {
                    comma_votes += 1;
                }
            }
        }
    }
    if comma_votes > dot_votes {
        ','
    } else {
        '.'
    }
}

/// Number of characters after the last occurrence of `sep`.
fn tail_len(s: &str, sep: char) -> usize {
    match s.rfind(sep) {
        Some(i) => s[i + sep.len_utf8()..].chars().count(),
        None => usize::MAX,
    }
}

/// Turn the numeric body into a plain `123.45` string using `decimal` as the decimal mark.
fn normalize_body(core: &str, decimal: char) -> Result<String, String> {
    let group = if decimal == '.' { ',' } else { '.' };
    let mut out = String::with_capacity(core.len());
    let mut seen_decimal = false;
    for c in core.chars() {
        if c.is_ascii_digit() {
            out.push(c);
        } else if is_group_only(c) || c == group {
            continue;
        } else if c == decimal {
            if seen_decimal {
                return Err(format!(
                    "more than one `{decimal}` decimal separator — set the decimal separator explicitly"
                ));
            }
            seen_decimal = true;
            out.push('.');
        } else {
            return Err(format!("unexpected character `{c}` inside the number"));
        }
    }
    if !out.chars().any(|c| c.is_ascii_digit()) {
        return Err("no digits found".into());
    }
    if out.starts_with('.') {
        out.insert(0, '0');
    }
    if out.ends_with('.') {
        out.push('0');
    }
    Ok(out)
}

/// Magnitude suffix → multiplier. Only the standard finance abbreviations, so real units
/// like `kg`, `km`, or `ms` are stripped rather than silently multiplied.
fn magnitude(unit: &str) -> Option<f64> {
    match unit.to_ascii_lowercase().as_str() {
        "k" => Some(1e3),
        "m" => Some(1e6),
        "b" | "bn" => Some(1e9),
        "t" | "tn" => Some(1e12),
        _ => None,
    }
}

/// Kill floating-point noise from division/multiplication by rounding to 12 significant
/// digits, so `45.2%` ÷ 100 prints `0.452` and not `0.45200000000000007`.
fn clean(v: f64) -> f64 {
    if !v.is_finite() || v == 0.0 {
        return v;
    }
    let mag = v.abs().log10().floor();
    let factor = 10f64.powf(11.0 - mag);
    let scaled = v * factor;
    if factor.is_finite() && factor != 0.0 && scaled.is_finite() && scaled.abs() < 9e15 {
        scaled.round() / factor
    } else {
        v
    }
}

/// Parse a fraction body such as `3/4` or `1 1/2` (whole + numerator/denominator).
fn parse_fraction(core: &str, decimal: char) -> Result<f64, String> {
    let parts: Vec<&str> = core.split('/').collect();
    if parts.len() != 2 {
        return Err("expected one `/` in a fraction such as 3/4".into());
    }
    let denom_s = normalize_body(trim_spaces(parts[1]), decimal)?;
    let denom: f64 = denom_s
        .parse()
        .map_err(|_| format!("could not read `{denom_s}` as a number"))?;
    if denom == 0.0 {
        return Err("fraction denominator is zero".into());
    }
    let left = trim_spaces(parts[0]);
    let tokens: Vec<&str> = left
        .split(is_space_like)
        .filter(|t| !t.is_empty())
        .collect();
    let (whole, numer_s) = match tokens.len() {
        1 => (0.0, tokens[0]),
        2 => {
            let w = normalize_body(tokens[0], decimal)?;
            (
                w.parse::<f64>()
                    .map_err(|_| format!("could not read `{w}` as a number"))?,
                tokens[1],
            )
        }
        _ => return Err("expected a fraction such as 3/4 or 1 1/2".into()),
    };
    let numer_s = normalize_body(numer_s, decimal)?;
    let numer: f64 = numer_s
        .parse()
        .map_err(|_| format!("could not read `{numer_s}` as a number"))?;
    Ok(whole + numer / denom)
}

/// Sanitize one value. `decimal` is the already-resolved decimal mark.
pub fn sanitize_value(raw: &str, decimal: char, opts: &Options) -> Result<Option<f64>, String> {
    let trimmed = trim_spaces(raw);
    if trimmed.is_empty() {
        return Ok(None);
    }
    let (body, paren) = strip_parens(trimmed);
    if body.is_empty() {
        return Err("no digits found".into());
    }
    let (prefix, core, suffix) = match split_core(body) {
        Some(t) => t,
        None => return Err("no digits found".into()),
    };

    let mut negative = paren && opts.parentheses_negative;
    // A leading minus, or the trailing minus used by SAP/DATEV style exports.
    if prefix.chars().any(is_minus) || suffix.chars().any(is_minus) {
        negative = !negative;
    }
    let percent = prefix.chars().any(is_percent) || suffix.chars().any(is_percent);

    // Whatever letters trail the digits are either a magnitude suffix or a unit/currency
    // code; take the first letter run so `1.5 M USD` still scales while `7 kg` does not.
    let unit: String = suffix
        .chars()
        .skip_while(|c| !c.is_alphabetic())
        .take_while(|c| c.is_alphabetic())
        .collect::<String>();
    let mult = if opts.magnitude_suffixes {
        magnitude(&unit).unwrap_or(1.0)
    } else {
        1.0
    };

    let mut value = if core.contains('/') {
        parse_fraction(core, decimal)?
    } else if let Some(epos) = exponent_split(core) {
        let (mantissa, exp) = core.split_at(epos);
        let mantissa = normalize_body(mantissa, decimal)?;
        let exp_digits = &exp[1..];
        let normalized = format!("{mantissa}e{exp_digits}");
        normalized
            .parse::<f64>()
            .map_err(|_| format!("could not read `{core}` as a number"))?
    } else {
        if !core.chars().all(is_core_char) {
            let bad = core.chars().find(|c| !is_core_char(*c)).unwrap();
            return Err(format!("unexpected character `{bad}` inside the number"));
        }
        let normalized = normalize_body(core, decimal)?;
        normalized
            .parse::<f64>()
            .map_err(|_| format!("could not read `{core}` as a number"))?
    };

    value *= mult;
    if percent && opts.percent == PercentMode::Divide {
        value /= 100.0;
    }
    if negative {
        value = -value;
    }
    if !value.is_finite() {
        return Err("value is out of range for a 64-bit float".into());
    }
    let mut value = clean(value);
    if let Some(d) = opts.decimals {
        let f = 10f64.powi(d as i32);
        let scaled = value * f;
        if scaled.is_finite() && scaled.abs() < 9e15 {
            value = scaled.round() / f;
        }
    }
    Ok(Some(value))
}

/// Position of the `e`/`E` that starts a valid exponent (`1.5e-3`), if any.
fn exponent_split(core: &str) -> Option<usize> {
    let pos = core
        .char_indices()
        .find(|(_, c)| *c == 'e' || *c == 'E')
        .map(|(i, _)| i)?;
    let rest = &core[pos + 1..];
    let digits = rest.strip_prefix(['+', '-']).unwrap_or(rest);
    if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
        Some(pos)
    } else {
        None
    }
}

/// Sanitize a whole column of values (one per line).
pub fn sanitize(values: &str, opts: &Options) -> Result<Sanitized, String> {
    let lines: Vec<&str> = values
        .split('\n')
        .map(|l| l.trim_end_matches('\r'))
        .collect();
    // Trailing blank lines are paste artifacts, not data.
    let mut end = lines.len();
    while end > 0 && trim_spaces(lines[end - 1]).is_empty() {
        end -= 1;
    }
    let lines = &lines[..end];
    if lines.is_empty() {
        return Err("no values given — paste one value per line".into());
    }
    if lines.len() > MAX_VALUES {
        return Err(format!(
            "too many values: {} (max {MAX_VALUES}) — split the column into smaller batches",
            lines.len()
        ));
    }

    let decimal = match opts.decimal_separator {
        DecimalSep::Auto => infer_decimal(lines),
        DecimalSep::Dot => '.',
        DecimalSep::Comma => ',',
    };

    let mut rows = Vec::with_capacity(lines.len());
    let (mut parsed, mut empty, mut failed) = (0usize, 0usize, 0usize);
    for (i, raw) in lines.iter().enumerate() {
        let status = match sanitize_value(raw, decimal, opts) {
            Ok(Some(v)) => {
                parsed += 1;
                RowStatus::Ok(v)
            }
            Ok(None) => {
                empty += 1;
                RowStatus::Empty
            }
            Err(e) => {
                if opts.on_error == OnError::Fail {
                    return Err(format!("line {}: `{}` — {}", i + 1, raw.trim(), e));
                }
                failed += 1;
                RowStatus::Error(e)
            }
        };
        rows.push(Row {
            original: (*raw).to_string(),
            status,
        });
    }

    Ok(Sanitized {
        rows,
        decimal_used: decimal,
        parsed,
        empty,
        failed,
    })
}

/// Format a value the way a spreadsheet expects it: plain digits, dot decimal, no grouping.
pub fn format_value(v: f64, decimals: Option<u32>) -> String {
    match decimals {
        Some(d) => format!("{:.*}", d as usize, v),
        // Rust's float Display is plain decimal (never exponent form) and picks the
        // shortest string that round-trips, which is exactly the spreadsheet-ready shape.
        None => format!("{v}"),
    }
}

fn error_cell(row: &Row, on_error: OnError) -> String {
    match on_error {
        OnError::Blank | OnError::Fail => String::new(),
        OnError::Keep => row.original.trim().to_string(),
        OnError::Marker => "#ERROR".to_string(),
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
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

fn stats_of(res: &Sanitized) -> Option<(f64, f64, f64, f64)> {
    let vals: Vec<f64> = res
        .rows
        .iter()
        .filter_map(|r| match r.status {
            RowStatus::Ok(v) => Some(v),
            _ => None,
        })
        .collect();
    if vals.is_empty() {
        return None;
    }
    let sum: f64 = vals.iter().sum();
    let min = vals.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    Some((clean(sum), min, max, clean(sum / vals.len() as f64)))
}

fn render_stats(res: &Sanitized, opts: &Options, out: &mut String) {
    out.push_str("\n\n--- Summary ---\n");
    out.push_str(&format!("Values: {}\n", res.rows.len()));
    out.push_str(&format!("Parsed: {}\n", res.parsed));
    out.push_str(&format!("Failed: {}\n", res.failed));
    out.push_str(&format!("Empty: {}\n", res.empty));
    out.push_str(&format!("Decimal separator: {}\n", res.decimal_used));
    match stats_of(res) {
        Some((sum, min, max, avg)) => {
            out.push_str(&format!("Sum: {}\n", format_value(sum, opts.decimals)));
            out.push_str(&format!("Min: {}\n", format_value(min, opts.decimals)));
            out.push_str(&format!("Max: {}\n", format_value(max, opts.decimals)));
            out.push_str(&format!("Average: {}", format_value(avg, opts.decimals)));
        }
        None => out.push_str("Sum: n/a"),
    }
}

fn render_values(res: &Sanitized, opts: &Options) -> String {
    let mut out = String::new();
    for (i, row) in res.rows.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        match &row.status {
            RowStatus::Ok(v) => out.push_str(&format_value(*v, opts.decimals)),
            RowStatus::Empty => {}
            RowStatus::Error(_) => out.push_str(&error_cell(row, opts.on_error)),
        }
    }
    if opts.stats {
        render_stats(res, opts, &mut out);
    }
    out
}

fn render_table(res: &Sanitized, opts: &Options) -> String {
    let mut out = String::from("original\tvalue\tstatus");
    for row in &res.rows {
        let original = row.original.trim().replace('\t', " ");
        let (value, status) = match &row.status {
            RowStatus::Ok(v) => (format_value(*v, opts.decimals), "ok".to_string()),
            RowStatus::Empty => (String::new(), "empty".to_string()),
            RowStatus::Error(e) => (error_cell(row, opts.on_error), format!("error: {e}")),
        };
        out.push_str(&format!("\n{original}\t{value}\t{status}"));
    }
    if opts.stats {
        render_stats(res, opts, &mut out);
    }
    out
}

fn render_json(res: &Sanitized, opts: &Options) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!(
        "  \"decimal_separator\": \"{}\",\n",
        res.decimal_used
    ));
    out.push_str(&format!("  \"count\": {},\n", res.rows.len()));
    out.push_str(&format!("  \"parsed\": {},\n", res.parsed));
    out.push_str(&format!("  \"failed\": {},\n", res.failed));
    out.push_str(&format!("  \"empty\": {},\n", res.empty));
    out.push_str("  \"rows\": [");
    for (i, row) in res.rows.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("\n    {");
        out.push_str(&format!(
            "\"original\": \"{}\", ",
            json_escape(row.original.trim())
        ));
        match &row.status {
            RowStatus::Ok(v) => out.push_str(&format!(
                "\"value\": {}, \"status\": \"ok\"",
                format_value(*v, opts.decimals)
            )),
            RowStatus::Empty => out.push_str("\"value\": null, \"status\": \"empty\""),
            RowStatus::Error(e) => out.push_str(&format!(
                "\"value\": null, \"status\": \"error\", \"error\": \"{}\"",
                json_escape(e)
            )),
        }
        out.push('}');
    }
    out.push_str("\n  ]");
    if opts.stats {
        if let Some((sum, min, max, avg)) = stats_of(res) {
            out.push_str(",\n  \"stats\": {");
            out.push_str(&format!("\"sum\": {}, ", format_value(sum, opts.decimals)));
            out.push_str(&format!("\"min\": {}, ", format_value(min, opts.decimals)));
            out.push_str(&format!("\"max\": {}, ", format_value(max, opts.decimals)));
            out.push_str(&format!(
                "\"average\": {}",
                format_value(avg, opts.decimals)
            ));
            out.push('}');
        } else {
            out.push_str(",\n  \"stats\": null");
        }
    }
    out.push_str("\n}");
    out
}

/// Sanitize `values` and render the chosen output format.
#[allow(clippy::too_many_arguments)]
pub fn run(
    values: &str,
    decimal_separator: &str,
    percent: &str,
    magnitude_suffixes: bool,
    parentheses_negative: bool,
    decimals: Option<u32>,
    on_error: &str,
    output: &str,
    stats: bool,
) -> Result<String, String> {
    if let Some(d) = decimals {
        if d > 12 {
            return Err(format!("decimals must be 0-12 (got {d})"));
        }
    }
    let opts = Options {
        decimal_separator: DecimalSep::parse(decimal_separator)?,
        percent: PercentMode::parse(percent)?,
        magnitude_suffixes,
        parentheses_negative,
        decimals,
        on_error: OnError::parse(on_error)?,
        output: OutputFormat::parse(output)?,
        stats,
    };
    let res = sanitize(values, &opts)?;
    Ok(match opts.output {
        OutputFormat::Values => render_values(&res, &opts),
        OutputFormat::Table => render_table(&res, &opts),
        OutputFormat::Json => render_json(&res, &opts),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(raw: &str) -> f64 {
        let opts = Options::default();
        let res = sanitize(raw, &opts).unwrap();
        match res.rows[0].status {
            RowStatus::Ok(v) => v,
            ref other => panic!("expected a value for {raw:?}, got {other:?}"),
        }
    }

    #[test]
    fn strips_currency_grouping_and_unit() {
        assert_eq!(one("$1,234.50 USD"), 1234.5);
        assert_eq!(one("  €  1,000  "), 1000.0);
        assert_eq!(one("12.5 kg"), 12.5);
        assert_eq!(one("CHF 1'234'567.89"), 1234567.89);
        assert_eq!(one("£2,500"), 2500.0);
    }

    #[test]
    fn handles_nbsp_and_zero_width_junk() {
        assert_eq!(one("\u{00a0}1\u{202f}234,56\u{200b}"), 1234.56);
        assert_eq!(one("1 234 567"), 1234567.0);
    }

    #[test]
    fn accounting_parentheses_and_trailing_minus() {
        assert_eq!(one("(250.00)"), -250.0);
        assert_eq!(one("($1,234.00)"), -1234.0);
        assert_eq!(one("1234-"), -1234.0);
        assert_eq!(one("\u{2212}42"), -42.0);
        let opts = Options {
            parentheses_negative: false,
            ..Options::default()
        };
        let res = sanitize("(250.00)", &opts).unwrap();
        assert_eq!(res.rows[0].status, RowStatus::Ok(250.0));
    }

    #[test]
    fn european_decimal_is_inferred_per_column() {
        // "1.234" alone is ambiguous, but the column proves comma-decimal.
        let res = sanitize("1.234,56\n1.234\n9,50", &Options::default()).unwrap();
        assert_eq!(res.decimal_used, ',');
        assert_eq!(res.rows[0].status, RowStatus::Ok(1234.56));
        assert_eq!(res.rows[1].status, RowStatus::Ok(1234.0));
        assert_eq!(res.rows[2].status, RowStatus::Ok(9.5));
    }

    #[test]
    fn us_column_stays_dot_decimal() {
        let res = sanitize("1,234.56\n1,234\n0.75", &Options::default()).unwrap();
        assert_eq!(res.decimal_used, '.');
        assert_eq!(res.rows[0].status, RowStatus::Ok(1234.56));
        assert_eq!(res.rows[1].status, RowStatus::Ok(1234.0));
    }

    #[test]
    fn explicit_decimal_separator_overrides_inference() {
        let opts = Options {
            decimal_separator: DecimalSep::Comma,
            ..Options::default()
        };
        let res = sanitize("1.234", &opts).unwrap();
        assert_eq!(res.rows[0].status, RowStatus::Ok(1234.0));
    }

    #[test]
    fn percent_modes() {
        assert_eq!(one("45.2%"), 45.2);
        let opts = Options {
            percent: PercentMode::Divide,
            ..Options::default()
        };
        let res = sanitize("45.2%", &opts).unwrap();
        assert_eq!(res.rows[0].status, RowStatus::Ok(0.452));
    }

    #[test]
    fn magnitude_suffixes_expand_but_units_do_not() {
        assert_eq!(one("1.2K"), 1200.0);
        assert_eq!(one("3M"), 3_000_000.0);
        assert_eq!(one("2bn"), 2e9);
        assert_eq!(one("1.5T"), 1.5e12);
        // `kg` is a unit, not a kilo-multiplier.
        assert_eq!(one("7 kg"), 7.0);
        let opts = Options {
            magnitude_suffixes: false,
            ..Options::default()
        };
        let res = sanitize("1.2K", &opts).unwrap();
        assert_eq!(res.rows[0].status, RowStatus::Ok(1.2));
    }

    #[test]
    fn scientific_notation_and_fractions() {
        assert_eq!(one("3.14e5"), 314000.0);
        assert_eq!(one("1.5E-3"), 0.0015);
        assert_eq!(one("3/4"), 0.75);
        assert_eq!(one("1 1/2"), 1.5);
    }

    #[test]
    fn rounding_and_formatting() {
        assert_eq!(format_value(1234.5, None), "1234.5");
        assert_eq!(format_value(1200.0, None), "1200");
        assert_eq!(format_value(1234.567, Some(2)), "1234.57");
        let opts = Options {
            decimals: Some(2),
            ..Options::default()
        };
        assert_eq!(run_default(&opts, "$1,234.567"), "1234.57");
    }

    fn run_default(opts: &Options, input: &str) -> String {
        let res = sanitize(input, opts).unwrap();
        render_values(&res, opts)
    }

    #[test]
    fn unparseable_rows_follow_the_error_policy() {
        let input = "12\nn/a\n34";
        let blank = run(
            input, "auto", "strip", true, true, None, "blank", "values", false,
        )
        .unwrap();
        assert_eq!(blank, "12\n\n34");
        let keep = run(
            input, "auto", "strip", true, true, None, "keep", "values", false,
        )
        .unwrap();
        assert_eq!(keep, "12\nn/a\n34");
        let marker = run(
            input, "auto", "strip", true, true, None, "marker", "values", false,
        )
        .unwrap();
        assert_eq!(marker, "12\n#ERROR\n34");
        let err = run(
            input, "auto", "strip", true, true, None, "fail", "values", false,
        )
        .unwrap_err();
        assert!(err.contains("line 2"), "{err}");
        assert!(err.contains("no digits found"), "{err}");
    }

    #[test]
    fn empty_rows_stay_aligned_and_are_not_failures() {
        let res = sanitize("1\n\n3", &Options::default()).unwrap();
        assert_eq!(res.parsed, 2);
        assert_eq!(res.empty, 1);
        assert_eq!(res.failed, 0);
        assert_eq!(
            run("1\n\n3", "auto", "strip", true, true, None, "blank", "values", false).unwrap(),
            "1\n\n3"
        );
    }

    #[test]
    fn digits_interrupted_by_letters_are_an_error() {
        let res = sanitize("12ab34", &Options::default()).unwrap();
        match &res.rows[0].status {
            RowStatus::Error(e) => assert!(e.contains("unexpected character"), "{e}"),
            other => panic!("expected an error, got {other:?}"),
        }
    }

    #[test]
    fn two_decimal_marks_are_an_error_when_forced() {
        let opts = Options {
            decimal_separator: DecimalSep::Dot,
            ..Options::default()
        };
        let res = sanitize("1.234.567", &opts).unwrap();
        match &res.rows[0].status {
            RowStatus::Error(e) => assert!(e.contains("more than one"), "{e}"),
            other => panic!("expected an error, got {other:?}"),
        }
    }

    #[test]
    fn table_and_json_outputs() {
        let table = run(
            "$1,200\nn/a",
            "auto",
            "strip",
            true,
            true,
            None,
            "marker",
            "table",
            false,
        )
        .unwrap();
        assert_eq!(
            table,
            "original\tvalue\tstatus\n$1,200\t1200\tok\nn/a\t#ERROR\terror: no digits found"
        );
        let json = run(
            "$1,200\nn/a",
            "auto",
            "strip",
            true,
            true,
            None,
            "blank",
            "json",
            true,
        )
        .unwrap();
        assert!(
            json.contains("\"value\": 1200, \"status\": \"ok\""),
            "{json}"
        );
        assert!(json.contains("\"failed\": 1"), "{json}");
        assert!(json.contains("\"sum\": 1200"), "{json}");
    }

    #[test]
    fn stats_block_reports_totals() {
        let out = run(
            "1,000\n2,000",
            "auto",
            "strip",
            true,
            true,
            None,
            "blank",
            "values",
            true,
        )
        .unwrap();
        assert!(out.contains("Values: 2"), "{out}");
        assert!(out.contains("Sum: 3000"), "{out}");
        assert!(out.contains("Average: 1500"), "{out}");
        assert!(out.contains("Decimal separator: ."), "{out}");
    }

    #[test]
    fn empty_input_is_rejected() {
        let err = run(
            "   ", "auto", "strip", true, true, None, "blank", "values", false,
        )
        .unwrap_err();
        assert!(err.contains("no values given"), "{err}");
    }

    #[test]
    fn bad_option_values_are_rejected() {
        let err = run(
            "1", "swedish", "strip", true, true, None, "blank", "values", false,
        )
        .unwrap_err();
        assert!(err.contains("unknown decimal_separator"), "{err}");
        let err = run(
            "1",
            "auto",
            "strip",
            true,
            true,
            Some(20),
            "blank",
            "values",
            false,
        )
        .unwrap_err();
        assert!(err.contains("decimals must be 0-12"), "{err}");
    }

    #[test]
    fn too_many_values_is_rejected() {
        let big = "1\n".repeat(MAX_VALUES + 1);
        let err = sanitize(&big, &Options::default()).unwrap_err();
        assert!(err.contains("too many values"), "{err}");
    }

    #[test]
    fn cap_boundary_is_accepted() {
        let at_cap = vec!["1"; MAX_VALUES].join("\n");
        let res = sanitize(&at_cap, &Options::default()).unwrap();
        assert_eq!(res.parsed, MAX_VALUES);
    }
}
