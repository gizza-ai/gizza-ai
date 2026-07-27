//! currency-converter core — convert an amount between currencies using a rate
//! table you supply, following multi-leg paths when there is no direct rate.
//! No wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.
//!
//! Rate table: one exchange rate per line, `FROM TO RATE`, meaning
//! "1 FROM = RATE TO". The two currency codes and the number may be separated by
//! whitespace, `/`, `:`, `=`, or `,` — so `USD EUR 0.92`, `USD/EUR 0.92`, and
//! `USD/EUR = 0.92` all parse identically. Blank lines and `#` comments are
//! ignored. When `bidirectional` is on (the default) each rate is also usable in
//! reverse (`1/RATE`), so a single `USD EUR 0.92` line converts both directions.
//! Conversions take the path with the fewest legs (`USD → EUR → JPY`).

use std::collections::{HashMap, VecDeque};

/// Largest number of decimal places `precision` may request when rounding the
/// converted amount. 10 is well past what any currency (or the f64 mantissa)
/// meaningfully carries.
pub const MAX_PRECISION: u32 = 10;

/// The result of a currency conversion.
#[derive(Debug, Clone, PartialEq)]
pub struct Conversion {
    /// The input amount, in `from`.
    pub amount: f64,
    /// Source currency code (upper-cased).
    pub from: String,
    /// Target currency code (upper-cased).
    pub to: String,
    /// The converted amount, in `to` (rounded to `precision` when rendered).
    pub converted: f64,
    /// Effective rate: 1 `from` = `rate` `to`.
    pub rate: f64,
    /// The currency codes visited, `from` first and `to` last. Length 1 when
    /// `from == to`; length 2 for a direct rate; longer for a multi-leg path.
    pub path: Vec<String>,
    /// Decimal places the converted amount is rounded to when rendered.
    pub precision: u32,
}

impl Conversion {
    /// Number of exchange legs the path used (0 when converting a currency to
    /// itself, 1 for a direct rate).
    pub fn legs(&self) -> usize {
        self.path.len().saturating_sub(1)
    }

    /// Human-readable multi-line result, e.g.
    /// `100 USD = 14,400.00 JPY` / `Rate: 1 USD = 144 JPY` /
    /// `Path: USD → EUR → JPY (2 legs)`.
    pub fn render(&self) -> String {
        let mut out = format!(
            "{} {} = {} {}",
            fmt_amount(self.amount),
            self.from,
            fmt_fixed(self.converted, self.precision),
            self.to,
        );
        out.push_str(&format!(
            "\nRate: 1 {} = {} {}",
            self.from,
            fmt_rate(self.rate),
            self.to,
        ));
        if self.legs() > 1 {
            out.push_str(&format!(
                "\nPath: {} ({} legs)",
                self.path.join(" → "),
                self.legs(),
            ));
        }
        out
    }
}

/// Convert `amount` from currency `from` to currency `to` using the `rates`
/// table, then render the result. See [`convert`] for the structured form.
pub fn run(
    amount: f64,
    from: &str,
    to: &str,
    rates: &str,
    bidirectional: bool,
    precision: u32,
) -> Result<String, String> {
    Ok(convert(amount, from, to, rates, bidirectional, precision)?.render())
}

/// Convert `amount` from `from` to `to` through the `rates` table, returning the
/// structured [`Conversion`] (amount, effective rate, and the path taken).
///
/// - `amount` must be finite.
/// - `from` / `to` are currency codes, matched case-insensitively.
/// - `rates`: one `FROM TO RATE` line per rate (see the module docs); each RATE
///   must be finite and `> 0`.
/// - `bidirectional`: also allow every rate in reverse (`1/RATE`).
/// - `precision`: decimal places for the rendered amount, clamped to
///   `0..=MAX_PRECISION`.
///
/// Errors on a non-finite amount, an unparseable / non-positive rate line, or
/// when no path of rates connects `from` to `to`.
pub fn convert(
    amount: f64,
    from: &str,
    to: &str,
    rates: &str,
    bidirectional: bool,
    precision: u32,
) -> Result<Conversion, String> {
    if !amount.is_finite() {
        return Err(format!("invalid amount {amount}: must be a finite number"));
    }
    let precision = precision.min(MAX_PRECISION);
    let from = normalize_code(from)?;
    let to = normalize_code(to)?;

    // Same currency: identity, no rate table needed (checked before parsing so
    // an empty/absent table still converts a currency to itself).
    if from == to {
        return Ok(Conversion {
            amount,
            converted: amount,
            rate: 1.0,
            path: vec![from.clone()],
            from,
            to,
            precision,
        });
    }

    let graph = parse_rates(rates, bidirectional)?;

    let (rate, path) = shortest_path(&graph, &from, &to).ok_or_else(|| {
        format!(
            "no conversion path from {from} to {to}: add a rate line connecting them \
             (e.g. \"{from} {to} <rate>\"){}",
            if bidirectional {
                ""
            } else {
                ", or enable reverse rates"
            }
        )
    })?;

    Ok(Conversion {
        amount,
        converted: amount * rate,
        rate,
        path,
        from,
        to,
        precision,
    })
}

/// Adjacency list: currency → list of (neighbour, rate) where 1 unit of the key
/// currency equals `rate` units of the neighbour.
type Graph = HashMap<String, Vec<(String, f64)>>;

fn parse_rates(rates: &str, bidirectional: bool) -> Result<Graph, String> {
    let mut graph: Graph = HashMap::new();
    let mut any = false;

    for (i, raw) in rates.lines().enumerate() {
        // Strip inline `#` comments and surrounding whitespace; skip blanks.
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let ln = i + 1;

        // Normalise the pair/number separators to spaces, then tokenise.
        let normalized: String = line
            .chars()
            .map(|c| if matches!(c, '/' | ':' | '=' | ',') { ' ' } else { c })
            .collect();
        let tokens: Vec<&str> = normalized.split_whitespace().collect();
        if tokens.len() != 3 {
            return Err(format!(
                "invalid rate on line {ln}: expected \"FROM TO RATE\" (e.g. \"USD EUR 0.92\"), \
                 got {raw:?}"
            ));
        }

        // Exactly one token is the numeric rate; the other two are the codes,
        // in FROM→TO order.
        let numeric: Vec<usize> = (0..3).filter(|&j| parse_num(tokens[j]).is_some()).collect();
        if numeric.len() != 1 {
            return Err(format!(
                "invalid rate on line {ln}: expected two currency codes and one number, got {raw:?}"
            ));
        }
        let num_idx = numeric[0];
        let rate = parse_num(tokens[num_idx]).unwrap();
        if !(rate.is_finite() && rate > 0.0) {
            return Err(format!(
                "invalid rate on line {ln}: rate must be a positive number, got {:?}",
                tokens[num_idx]
            ));
        }
        let codes: Vec<usize> = (0..3).filter(|&j| j != num_idx).collect();
        let a = normalize_code(tokens[codes[0]])
            .map_err(|e| format!("invalid rate on line {ln}: {e}"))?;
        let b = normalize_code(tokens[codes[1]])
            .map_err(|e| format!("invalid rate on line {ln}: {e}"))?;
        if a == b {
            return Err(format!(
                "invalid rate on line {ln}: {a} and {b} are the same currency"
            ));
        }

        graph.entry(a.clone()).or_default().push((b.clone(), rate));
        if bidirectional {
            graph.entry(b).or_default().push((a, 1.0 / rate));
        }
        any = true;
    }

    if !any {
        return Err("no exchange rates provided: paste a rate table, one \"FROM TO RATE\" \
                    line per rate (e.g. \"USD EUR 0.92\")"
            .into());
    }
    Ok(graph)
}

/// Breadth-first search for the fewest-legs path from `from` to `to`, returning
/// the cumulative rate and the code path. BFS guarantees the first path found
/// has the minimum number of legs.
fn shortest_path(graph: &Graph, from: &str, to: &str) -> Option<(f64, Vec<String>)> {
    let mut queue: VecDeque<(String, f64, Vec<String>)> = VecDeque::new();
    let mut seen: HashMap<String, ()> = HashMap::new();
    queue.push_back((from.to_string(), 1.0, vec![from.to_string()]));
    seen.insert(from.to_string(), ());

    while let Some((cur, rate, path)) = queue.pop_front() {
        if cur == to {
            return Some((rate, path));
        }
        if let Some(edges) = graph.get(&cur) {
            for (next, edge_rate) in edges {
                if seen.contains_key(next) {
                    continue;
                }
                seen.insert(next.clone(), ());
                let mut np = path.clone();
                np.push(next.clone());
                queue.push_back((next.clone(), rate * edge_rate, np));
            }
        }
    }
    None
}

/// Upper-case + validate a currency code: 2–10 ASCII letters/digits, e.g.
/// `usd` → `USD`, `btc` → `BTC`.
fn normalize_code(code: &str) -> Result<String, String> {
    let c = code.trim().to_ascii_uppercase();
    if c.is_empty() {
        return Err("missing currency code".into());
    }
    let ok = (2..=10).contains(&c.chars().count())
        && c.chars().all(|ch| ch.is_ascii_alphanumeric());
    if !ok {
        return Err(format!(
            "invalid currency code {code:?}: use a 2–10 character code like USD, EUR, or BTC"
        ));
    }
    Ok(c)
}

/// Parse a rate token as f64. Grouping commas were already normalised to spaces
/// by the caller, so a bare number remains — reject anything else (so codes like
/// `EUR` are never mistaken for a number).
fn parse_num(tok: &str) -> Option<f64> {
    tok.parse::<f64>().ok()
}

/// Format the input amount with a thousands separator and its natural precision
/// (trailing zeros trimmed): `100.0` → `100`, `1234.5` → `1,234.5`.
fn fmt_amount(x: f64) -> String {
    group_thousands(&trim_float(x))
}

/// Format a value to exactly `precision` decimals with a thousands separator:
/// `(10856.0, 2)` → `10,856.00`.
fn fmt_fixed(x: f64, precision: u32) -> String {
    group_thousands(&format!("{:.*}", precision as usize, x))
}

/// Format an effective rate: up to 6 decimals, trailing zeros trimmed. Not
/// grouped (rates read better ungrouped): `1.087000` → `1.087`.
fn fmt_rate(x: f64) -> String {
    trim_float((x * 1e6).round() / 1e6)
}

/// Shortest decimal string for `x` with trailing zeros/dot removed.
fn trim_float(x: f64) -> String {
    let s = format!("{:.10}", x);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Insert `,` thousands separators into the integer part of a decimal string,
/// preserving any sign and fractional part.
fn group_thousands(num: &str) -> String {
    let (sign, rest) = num
        .strip_prefix('-')
        .map(|r| ("-", r))
        .unwrap_or(("", num));
    let (int_part, frac) = match rest.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (rest, None),
    };
    let mut grouped = String::new();
    let digits: Vec<char> = int_part.chars().collect();
    for (idx, ch) in digits.iter().enumerate() {
        if idx > 0 && (digits.len() - idx) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(*ch);
    }
    let mut out = format!("{sign}{grouped}");
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_conversion() {
        let c = convert(100.0, "USD", "EUR", "USD EUR 0.92", true, 2).unwrap();
        assert_eq!(c.converted, 92.0);
        assert_eq!(c.rate, 0.92);
        assert_eq!(c.path, vec!["USD", "EUR"]);
        assert_eq!(c.legs(), 1);
        assert_eq!(c.render(), "100 USD = 92.00 EUR\nRate: 1 USD = 0.92 EUR");
    }

    #[test]
    fn reverse_rate_used_when_bidirectional() {
        // Only USD→EUR is given; EUR→USD must come from the inverse.
        let c = convert(92.0, "EUR", "USD", "USD EUR 0.92", true, 2).unwrap();
        assert!((c.converted - 100.0).abs() < 1e-9);
        assert!((c.rate - (1.0 / 0.92)).abs() < 1e-12);
    }

    #[test]
    fn reverse_rate_rejected_when_unidirectional() {
        let err = convert(92.0, "EUR", "USD", "USD EUR 0.92", false, 2).unwrap_err();
        assert!(err.contains("no conversion path"), "got: {err}");
        assert!(err.contains("enable reverse rates"), "got: {err}");
    }

    #[test]
    fn multi_leg_path_takes_fewest_legs() {
        // USD→EUR→JPY, no direct USD→JPY line.
        let rates = "USD EUR 0.9\nEUR JPY 160\nGBP JPY 190";
        let c = convert(100.0, "USD", "JPY", rates, true, 2).unwrap();
        assert!((c.rate - (0.9 * 160.0)).abs() < 1e-9);
        assert_eq!(c.path, vec!["USD", "EUR", "JPY"]);
        assert_eq!(c.legs(), 2);
        assert_eq!(
            c.render(),
            "100 USD = 14,400.00 JPY\nRate: 1 USD = 144 JPY\nPath: USD → EUR → JPY (2 legs)"
        );
    }

    #[test]
    fn prefers_direct_over_multi_leg() {
        // A direct USD→JPY exists alongside the two-leg route; BFS takes it.
        let rates = "USD EUR 0.9\nEUR JPY 160\nUSD JPY 150";
        let c = convert(2.0, "USD", "JPY", rates, true, 2).unwrap();
        assert_eq!(c.path, vec!["USD", "JPY"]);
        assert_eq!(c.converted, 300.0);
    }

    #[test]
    fn same_currency_is_identity() {
        let c = convert(50.0, "usd", "USD", "", true, 2).unwrap();
        assert_eq!(c.converted, 50.0);
        assert_eq!(c.rate, 1.0);
        assert_eq!(c.path, vec!["USD"]);
        assert_eq!(c.render(), "50 USD = 50.00 USD\nRate: 1 USD = 1 USD");
    }

    #[test]
    fn flexible_separators_parse() {
        // slash + equals, and a comment + blank line.
        let rates = "USD/EUR = 0.92\n# a comment\n\nEUR:GBP 0.85";
        let c = convert(1.0, "USD", "GBP", rates, true, 6).unwrap();
        assert!((c.rate - (0.92 * 0.85)).abs() < 1e-12);
    }

    #[test]
    fn code_and_number_order_agnostic() {
        // "USD 0.92 EUR" — the number can appear anywhere; codes stay FROM→TO.
        let c = convert(10.0, "USD", "EUR", "USD 0.92 EUR", true, 2).unwrap();
        assert!((c.converted - 9.2).abs() < 1e-9);
    }

    #[test]
    fn precision_rounds_and_groups() {
        let c = convert(1234.5, "USD", "JPY", "USD JPY 150.25", true, 0).unwrap();
        // 1234.5 * 150.25 = 185,483.625 → 0 dp → 185,484
        assert_eq!(c.render().lines().next().unwrap(), "1,234.5 USD = 185,484 JPY");
    }

    #[test]
    fn precision_clamped_to_max() {
        let c = convert(1.0, "USD", "EUR", "USD EUR 0.5", true, 999).unwrap();
        assert_eq!(c.precision, MAX_PRECISION);
    }

    #[test]
    fn negative_amount_allowed() {
        let c = convert(-50.0, "USD", "EUR", "USD EUR 0.9", true, 2).unwrap();
        assert_eq!(c.converted, -45.0);
        assert_eq!(c.render().lines().next().unwrap(), "-50 USD = -45.00 EUR");
    }

    #[test]
    fn rejects_empty_rate_table() {
        let err = convert(1.0, "USD", "EUR", "   \n # only a comment", true, 2).unwrap_err();
        assert!(err.contains("no exchange rates"), "got: {err}");
    }

    #[test]
    fn rejects_no_path() {
        let err = convert(1.0, "USD", "JPY", "USD EUR 0.9", true, 2).unwrap_err();
        assert!(err.contains("no conversion path from USD to JPY"), "got: {err}");
    }

    #[test]
    fn rejects_bad_rate_line() {
        let err = convert(1.0, "USD", "EUR", "USD EUR", true, 2).unwrap_err();
        assert!(err.contains("line 1"), "got: {err}");
        assert!(err.contains("FROM TO RATE"), "got: {err}");
    }

    #[test]
    fn rejects_non_positive_rate() {
        let err = convert(1.0, "USD", "EUR", "USD EUR -0.5", true, 2).unwrap_err();
        assert!(err.contains("positive number"), "got: {err}");
        let err0 = convert(1.0, "USD", "EUR", "USD EUR 0", true, 2).unwrap_err();
        assert!(err0.contains("positive number"), "got: {err0}");
    }

    #[test]
    fn rejects_self_pair_rate() {
        let err = convert(1.0, "USD", "EUR", "USD USD 1.5", true, 2).unwrap_err();
        assert!(err.contains("same currency"), "got: {err}");
    }

    #[test]
    fn rejects_bad_currency_code() {
        let err = convert(1.0, "U", "EUR", "USD EUR 0.9", true, 2).unwrap_err();
        assert!(err.contains("invalid currency code"), "got: {err}");
    }

    #[test]
    fn rejects_non_finite_amount() {
        let err = convert(f64::NAN, "USD", "EUR", "USD EUR 0.9", true, 2).unwrap_err();
        assert!(err.contains("finite"), "got: {err}");
    }
}
