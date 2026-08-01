//! subscription-tracker core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps, no I/O. Given a pasted list of
//! recurring subscriptions (`Name: amount [cycle]`, one per line), it normalizes
//! every plan's cost to a common monthly and annual figure, sums the totals
//! (monthly / annual / 5-year / per-day), shows each plan's share of the total,
//! and flags the single biggest annual spend as a cancel candidate — a
//! "what am I really paying for subscriptions" report that runs entirely offline.
//!
//! All money is handled in integer cents. Annualization uses fixed periods-per-year
//! multipliers (weekly ×52, biweekly ×26, monthly ×12, quarterly ×4, semiannual ×2,
//! yearly ×1, daily ×365); the monthly equivalent is annual ÷ 12.

/// Most subscription lines accepted in one call (a paste guard).
pub const MAX_LINES: usize = 200;
/// Largest per-line amount, in cents ($1,000,000,000.00).
pub const MAX_AMOUNT_CENTS: i64 = 100_000_000_000;

/// A billing cadence: how often the charge recurs.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Cycle {
    /// Canonical lowercase name (e.g. "monthly").
    name: &'static str,
    /// Short label used in the table (e.g. "/mo").
    short: &'static str,
    /// Charges per year (exact integer, so annual cents stay exact).
    per_year: i64,
}

const CYCLES: &[Cycle] = &[
    Cycle { name: "daily", short: "/day", per_year: 365 },
    Cycle { name: "weekly", short: "/wk", per_year: 52 },
    Cycle { name: "biweekly", short: "/2wk", per_year: 26 },
    Cycle { name: "monthly", short: "/mo", per_year: 12 },
    Cycle { name: "quarterly", short: "/qtr", per_year: 4 },
    Cycle { name: "semiannual", short: "/6mo", per_year: 2 },
    Cycle { name: "yearly", short: "/yr", per_year: 1 },
];

/// Map a cycle token (with synonyms) to a `Cycle`. Case-insensitive; a leading
/// `/` or `per ` and a trailing `ly` are tolerated. Returns `None` if the token
/// is not a recognized cadence (so line parsing can treat it as part of a name).
fn parse_cycle(tok: &str) -> Option<Cycle> {
    let t = tok
        .trim()
        .trim_matches(|c: char| matches!(c, '(' | ')' | '[' | ']'))
        .to_ascii_lowercase();
    let t = t.trim_start_matches('/').trim_start_matches("per ").trim();
    let canon = match t {
        "daily" | "day" | "d" | "perday" => "daily",
        "weekly" | "week" | "wk" | "w" | "perweek" => "weekly",
        "biweekly" | "fortnightly" | "fortnight" | "2wk" | "2week" | "everytwoweeks" => "biweekly",
        "monthly" | "month" | "mo" | "m" | "mth" | "pm" | "permonth" => "monthly",
        "quarterly" | "quarter" | "qtr" | "q" | "3mo" | "perquarter" => "quarterly",
        "semiannual" | "semiannually" | "biannual" | "biannually" | "halfyearly"
        | "half-yearly" | "6mo" | "6month" | "everysixmonths" => "semiannual",
        "yearly" | "annual" | "annually" | "year" | "yr" | "y" | "pa" | "peryear" | "perannum"
        | "annum" => "yearly",
        _ => return None,
    };
    CYCLES.iter().copied().find(|c| c.name == canon)
}

/// Resolve the `default_cycle` param (blank → monthly). Errors on an unknown value.
fn parse_default_cycle(s: &str) -> Result<Cycle, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(*CYCLES.iter().find(|c| c.name == "monthly").unwrap());
    }
    parse_cycle(t).ok_or_else(|| {
        format!(
            "unknown default_cycle `{}` — use weekly, biweekly, monthly, quarterly, semiannual or yearly",
            t
        )
    })
}

/// How to order the rows in the report.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    Cost,
    Name,
    Input,
}

fn parse_sort(s: &str) -> Result<Sort, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "cost" => Ok(Sort::Cost),
        "name" => Ok(Sort::Name),
        "input" => Ok(Sort::Input),
        other => Err(format!("unknown sort `{}` — use cost, name or input", other)),
    }
}

/// Parse a money amount ("15.99", "$1,299", "1_299.00") to whole cents.
fn parse_amount_cents(raw: &str) -> Result<i64, String> {
    let s = raw.trim();
    let cleaned: String = s.chars().filter(|c| !matches!(c, ',' | '_')).collect();
    let cleaned = cleaned
        .trim()
        .trim_start_matches(['$', '€', '£', '¥', '₹'])
        .trim_start_matches('+')
        .trim()
        .to_string();
    if cleaned.starts_with('-') {
        return Err(format!("`{}` is negative — amounts must be 0 or more", s));
    }
    let (int_part, frac_part) = match cleaned.split_once('.') {
        Some((a, b)) => (a, b),
        None => (cleaned.as_str(), ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return Err(format!("`{}` is not an amount", s));
    }
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
    {
        return Err(format!(
            "`{}` is not an amount — use digits like 15.99 or 1,299",
            s
        ));
    }
    if frac_part.len() > 2 {
        return Err(format!(
            "`{}` has more than 2 decimal places — amounts are whole cents",
            s
        ));
    }
    if int_part.len() > 10 {
        return Err(format!("`{}` is too large (max 1,000,000,000)", s));
    }
    let dollars: i64 = if int_part.is_empty() {
        0
    } else {
        int_part
            .parse()
            .map_err(|_| format!("`{}` is not an amount", s))?
    };
    let cents_frac: i64 = match frac_part.len() {
        0 => 0,
        1 => frac_part.parse::<i64>().unwrap_or(0) * 10,
        _ => frac_part.parse::<i64>().unwrap_or(0),
    };
    let cents = dollars * 100 + cents_frac;
    if cents > MAX_AMOUNT_CENTS {
        return Err(format!("`{}` is too large (max 1,000,000,000)", s));
    }
    Ok(cents)
}

/// One parsed subscription, normalized to annual/monthly cents.
struct Sub {
    name: String,
    amount_cents: i64, // charge per billing cycle
    cycle: Cycle,
    annual_cents: i64,  // amount_cents * cycle.per_year
    monthly_cents: i64, // round(annual_cents / 12)
    idx: usize,         // input order, for stable ties
}

/// Round a÷b for non-negative integers.
fn div_round(a: i64, b: i64) -> i64 {
    (a + b / 2) / b
}

/// Parse one subscription line: `Name: amount [cycle]`, `Name = amount [cycle]`
/// or `Name amount [cycle]`. `default` supplies the cycle when the line omits one.
fn parse_line(line: &str, default: Cycle, idx: usize) -> Result<Sub, String> {
    let n = idx + 1;
    let ctx = |msg: &str| format!("subscription line {} (`{}`): {}", n, line.trim(), msg);

    let (name, amount_tok, cycle) = if let Some((left, right)) = line.split_once([':', '=']) {
        let name = left.trim().to_string();
        let mut toks: Vec<&str> = right.split_whitespace().collect();
        let mut cycle = default;
        if let Some(last) = toks.last() {
            if let Some(c) = parse_cycle(last) {
                cycle = c;
                toks.pop();
            }
        }
        if toks.len() != 1 {
            return Err(ctx(
                "expected `Name: amount` with an optional cycle (e.g. `Netflix: 15.99 monthly`)",
            ));
        }
        (name, toks[0].to_string(), cycle)
    } else {
        let mut toks: Vec<&str> = line.split_whitespace().collect();
        let mut cycle = default;
        if toks.len() >= 3 {
            if let Some(c) = parse_cycle(toks.last().unwrap()) {
                cycle = c;
                toks.pop();
            }
        }
        if toks.len() < 2 {
            return Err(ctx(
                "expected `Name amount [cycle]` (e.g. `Netflix 15.99 monthly`)",
            ));
        }
        let amount_tok = toks.pop().unwrap().to_string();
        let name = toks.join(" ");
        (name, amount_tok, cycle)
    };

    if name.is_empty() {
        return Err(ctx("the subscription name is empty"));
    }
    let amount_cents = parse_amount_cents(&amount_tok).map_err(|e| ctx(&e))?;
    if amount_cents == 0 {
        return Err(ctx("amount is 0 — enter what the subscription costs"));
    }
    let annual_cents = amount_cents
        .checked_mul(cycle.per_year)
        .filter(|c| *c <= MAX_AMOUNT_CENTS * 365)
        .ok_or_else(|| ctx("annualized amount is too large"))?;
    Ok(Sub {
        name,
        amount_cents,
        cycle,
        annual_cents,
        monthly_cents: div_round(annual_cents, 12),
        idx,
    })
}

/// Format cents as `$1,234.56` with grouping and the given currency symbol.
fn fmt_money(cents: i64, cur: &str) -> String {
    let dollars = cents / 100;
    let frac = cents % 100;
    let mut grouped = String::new();
    let ds = dollars.to_string();
    let bytes = ds.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(*b as char);
    }
    format!("{}{}.{:02}", cur, grouped, frac)
}

/// Parse + normalize the subscription list. Shared by all surfaces.
///
/// - `subscriptions`: one `Name: amount [cycle]` per line. Blank lines and `#`
///   comment lines are skipped. Amounts may include a currency symbol and
///   grouping commas.
/// - `default_cycle`: cadence assumed when a line omits one (blank → monthly).
/// - `currency`: symbol prefixed to amounts (blank → `$`).
/// - `sort`: row order — `cost` (biggest annual first, default), `name`, `input`.
///
/// Returns `Err` on a bad option value or when no line parses.
pub fn track(
    subscriptions: &str,
    default_cycle: &str,
    currency: &str,
    sort: &str,
) -> Result<String, String> {
    let default = parse_default_cycle(default_cycle)?;
    let sort = parse_sort(sort)?;
    let cur = {
        let c = currency.trim();
        if c.is_empty() {
            "$".to_string()
        } else if c.chars().count() > 8 {
            return Err("currency symbol is too long (max 8 characters)".into());
        } else {
            c.to_string()
        }
    };

    let raw_lines: Vec<&str> = subscriptions
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if raw_lines.is_empty() {
        return Err(
            "no subscriptions — add at least one line like `Netflix: 15.99 monthly`".into(),
        );
    }
    if raw_lines.len() > MAX_LINES {
        return Err(format!(
            "too many subscriptions ({}) — the maximum is {}",
            raw_lines.len(),
            MAX_LINES
        ));
    }

    let mut subs: Vec<Sub> = Vec::with_capacity(raw_lines.len());
    for (i, line) in raw_lines.iter().enumerate() {
        subs.push(parse_line(line, default, i)?);
    }

    let total_annual: i64 = subs.iter().map(|s| s.annual_cents).sum();
    let total_monthly = div_round(total_annual, 12);
    let total_daily = div_round(total_annual, 365);
    let total_5yr = total_annual * 5;

    // Cancel candidate = biggest annual spend (first in input order on a tie).
    let biggest = subs
        .iter()
        .max_by(|a, b| a.annual_cents.cmp(&b.annual_cents).then(b.idx.cmp(&a.idx)))
        .map(|s| (s.name.clone(), s.annual_cents, s.monthly_cents));

    // Order rows for display.
    let mut order: Vec<&Sub> = subs.iter().collect();
    match sort {
        Sort::Cost => order.sort_by(|a, b| {
            b.annual_cents.cmp(&a.annual_cents).then(a.idx.cmp(&b.idx))
        }),
        Sort::Name => order.sort_by(|a, b| {
            a.name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase())
                .then(a.idx.cmp(&b.idx))
        }),
        Sort::Input => {}
    }

    // Build the aligned table.
    let name_w = order
        .iter()
        .map(|s| s.name.chars().count())
        .max()
        .unwrap_or(4)
        .max(12);
    let billed: Vec<String> = order
        .iter()
        .map(|s| format!("{}{}", fmt_money(s.amount_cents, &cur), s.cycle.short))
        .collect();
    let billed_w = billed.iter().map(|b| b.chars().count()).max().unwrap_or(6).max(8);
    let monthly: Vec<String> = order.iter().map(|s| fmt_money(s.monthly_cents, &cur)).collect();
    let annual: Vec<String> = order.iter().map(|s| fmt_money(s.annual_cents, &cur)).collect();
    let monthly_w = monthly.iter().map(|m| m.chars().count()).max().unwrap_or(7).max(7);
    let annual_w = annual.iter().map(|a| a.chars().count()).max().unwrap_or(6).max(6);

    let mut out = String::new();
    let plural = if subs.len() == 1 { "" } else { "s" };
    out.push_str(&format!("Subscription spend — {} active plan{}\n\n", subs.len(), plural));
    out.push_str(&format!(
        "{:<nw$}  {:<bw$}  {:>mw$}  {:>aw$}  Share\n",
        "Subscription",
        "Billed",
        "Monthly",
        "Annual",
        nw = name_w,
        bw = billed_w,
        mw = monthly_w,
        aw = annual_w,
    ));
    for (i, s) in order.iter().enumerate() {
        let share = if total_annual > 0 {
            s.annual_cents as f64 / total_annual as f64 * 100.0
        } else {
            0.0
        };
        out.push_str(&format!(
            "{:<nw$}  {:<bw$}  {:>mw$}  {:>aw$}  {:>4.1}%\n",
            s.name,
            billed[i],
            monthly[i],
            annual[i],
            share,
            nw = name_w,
            bw = billed_w,
            mw = monthly_w,
            aw = annual_w,
        ));
    }

    out.push('\n');
    out.push_str(&format!("Monthly total:  {}\n", fmt_money(total_monthly, &cur)));
    out.push_str(&format!("Annual total:   {}\n", fmt_money(total_annual, &cur)));
    out.push_str(&format!("5-year total:   {}\n", fmt_money(total_5yr, &cur)));
    out.push_str(&format!("Per day:        {}\n", fmt_money(total_daily, &cur)));

    if let Some((name, annual_c, monthly_c)) = biggest {
        out.push('\n');
        out.push_str(&format!(
            "Biggest spend: {} at {}/yr ({}/mo). Cancelling it saves {}/year.\n",
            name,
            fmt_money(annual_c, &cur),
            fmt_money(monthly_c, &cur),
            fmt_money(annual_c, &cur),
        ));
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worked_example_totals_and_biggest() {
        let input = "Netflix: 15.99 monthly\nSpotify: 10.99\nAmazon Prime: 139 yearly\nAdobe: 59.99 quarterly";
        let r = track(input, "monthly", "$", "cost").unwrap();
        // Annual: Netflix 191.88, Spotify 131.88, Amazon 139.00, Adobe 239.96.
        // Total annual = 702.72; monthly = 58.56.
        assert!(r.contains("Annual total:   $702.72"), "report:\n{r}");
        assert!(r.contains("Monthly total:  $58.56"), "report:\n{r}");
        assert!(r.contains("5-year total:   $3,513.60"), "report:\n{r}");
        // Adobe is the biggest annual spend and cancel candidate.
        assert!(
            r.contains("Biggest spend: Adobe at $239.96/yr"),
            "report:\n{r}"
        );
        // Default sort = cost: Adobe row comes before Netflix.
        let adobe = r.find("Adobe").unwrap();
        let netflix = r.find("Netflix").unwrap();
        assert!(adobe < netflix, "Adobe should sort above Netflix:\n{r}");
    }

    #[test]
    fn weekly_annualizes_times_52_and_currency_symbol() {
        let r = track("Coffee: 5 weekly", "monthly", "£", "cost").unwrap();
        assert!(r.contains("Annual total:   £260.00"), "report:\n{r}");
        assert!(r.contains("£5.00/wk"), "report:\n{r}");
    }

    #[test]
    fn default_cycle_applies_when_omitted() {
        // No per-line cycle → uses default_cycle=yearly → 100/yr.
        let r = track("Domain: 100", "yearly", "$", "input").unwrap();
        assert!(r.contains("Annual total:   $100.00"), "report:\n{r}");
        assert!(r.contains("$100.00/yr"), "report:\n{r}");
    }

    #[test]
    fn empty_input_errors() {
        let e = track("   \n # a comment\n", "monthly", "$", "cost").unwrap_err();
        assert!(e.contains("no subscriptions"), "got: {e}");
    }

    #[test]
    fn unparseable_line_errors_with_context() {
        let e = track("Netflix: not-a-price monthly", "monthly", "$", "cost").unwrap_err();
        assert!(e.contains("subscription line 1"), "got: {e}");
        assert!(e.contains("not an amount"), "got: {e}");
    }

    #[test]
    fn bad_option_values_error() {
        assert!(track("A: 1", "fortnite", "$", "cost")
            .unwrap_err()
            .contains("unknown default_cycle"));
        assert!(track("A: 1", "monthly", "$", "sideways")
            .unwrap_err()
            .contains("unknown sort"));
    }
}
