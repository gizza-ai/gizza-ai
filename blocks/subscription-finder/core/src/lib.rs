//! subscription-finder core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps, no I/O. Given a pasted list of
//! `date, description, amount` transaction lines (a bank/card statement export),
//! it groups repeat charges from the same merchant, detects each one's cadence,
//! and projects the recurring monthly + annual spend — a privacy-first
//! "find my subscriptions" report that runs entirely offline.

/// Lowest occurrence count that can flag a charge as recurring, and the ceiling
/// the descriptor exposes. Blank/0/1 from a caller floors to `MIN_OCCURRENCES`.
pub const MIN_OCCURRENCES: u32 = 2;
pub const MAX_OCCURRENCES: u32 = 24;

/// A single parsed statement row.
struct Txn {
    ord: i64,      // days since 1970-01-01 (proleptic Gregorian)
    desc_raw: String,
    desc_norm: String,
    amount: f64,   // absolute value (a charge magnitude)
    idx: usize,    // input order, for a stable representative name
}

/// One detected recurring charge, ready to render.
struct Charge {
    name: String,
    amount: f64,
    count: usize,
    cadence: String,
    periods_per_year: f64,
    next_date: Option<i64>,
    annual: f64,
}

/// Build the recurring-charge report.
///
/// - `transactions`: one `date, description, amount` row per line (commas inside a
///   description are fine — only the first field is the date and the last the
///   amount). Blank and unparseable lines (e.g. a header row) are skipped.
/// - `min_occurrences`: how many times a merchant+amount must repeat to count as
///   recurring. Clamped to `MIN_OCCURRENCES..=MAX_OCCURRENCES` (blank/0/1 → 2).
/// - `currency`: symbol to prefix amounts with (blank → `$`).
/// - `date_format`: `"auto"` (blank → auto), `"iso"` (YYYY-MM-DD), `"us"`
///   (MM/DD/YYYY) or `"eu"` (DD/MM/YYYY). `auto` sniffs ISO by the `-` separator
///   and disambiguates `/` dates by any day-of-month > 12.
///
/// Returns `Err` on an invalid `date_format`, or when no row parses into a
/// (date, description, amount) triple.
pub fn find(
    transactions: &str,
    min_occurrences: u32,
    currency: &str,
    date_format: &str,
) -> Result<String, String> {
    let fmt = DateFmt::parse(date_format)?;
    let cur = if currency.trim().is_empty() { "$" } else { currency.trim() };
    let min_occ = min_occurrences.clamp(MIN_OCCURRENCES, MAX_OCCURRENCES);

    // For `auto` with slash dates, first scan the whole input to decide US vs EU:
    // any first component > 12 forces EU (DD/MM), else default US (MM/DD).
    let slash_layout = if matches!(fmt, DateFmt::Auto) {
        detect_slash_layout(transactions)
    } else {
        SlashLayout::Us
    };

    let mut txns: Vec<Txn> = Vec::new();
    for (idx, raw) in transactions.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 3 {
            continue; // not a "date, description, amount" row
        }
        let date_s = parts[0].trim();
        let amount_s = parts[parts.len() - 1].trim();
        let desc = parts[1..parts.len() - 1].join(",");
        let desc = desc.trim();

        let ord = match parse_date(date_s, fmt, slash_layout) {
            Some(o) => o,
            None => continue,
        };
        let amount = match parse_amount(amount_s) {
            Some(a) => a,
            None => continue,
        };
        if desc.is_empty() {
            continue;
        }
        txns.push(Txn {
            ord,
            desc_raw: desc.to_string(),
            desc_norm: normalize(desc),
            amount,
            idx,
        });
    }

    if txns.is_empty() {
        return Err("No transactions found. Paste one row per line as \"date, description, amount\" (e.g. 2026-01-15, Netflix, 15.99).".into());
    }

    // Group by normalized merchant (preserving first-seen order), then cluster by
    // amount within tolerance so a $15.99 and a $16.10 charge from the same
    // merchant count as one recurring charge.
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (i, t) in txns.iter().enumerate() {
        match groups.iter_mut().find(|(k, _)| *k == t.desc_norm) {
            Some((_, v)) => v.push(i),
            None => groups.push((t.desc_norm.clone(), vec![i])),
        }
    }

    let mut charges: Vec<Charge> = Vec::new();
    for (_, members) in &groups {
        for cluster in cluster_by_amount(members, &txns) {
            if cluster.len() < min_occ as usize {
                continue;
            }
            // Occurrence dates, sorted, for cadence + next-charge estimate.
            let mut ords: Vec<i64> = cluster.iter().map(|&i| txns[i].ord).collect();
            ords.sort_unstable();
            let intervals: Vec<i64> = ords.windows(2).map(|w| w[1] - w[0]).collect();
            let med = median(&intervals);
            if med <= 0.0 {
                continue; // same-day duplicates, not a recurring cadence
            }
            let (cadence, periods_per_year) = classify_cadence(med);

            let amounts: Vec<f64> = cluster.iter().map(|&i| txns[i].amount).collect();
            let amount = median_f64(&amounts);
            let annual = amount * periods_per_year;

            // Representative display name = the earliest-seen original spelling.
            let name = cluster
                .iter()
                .min_by_key(|&&i| txns[i].idx)
                .map(|&i| txns[i].desc_raw.clone())
                .unwrap_or_default();

            let next_date = ords.last().map(|&last| last + med.round() as i64);

            charges.push(Charge {
                name,
                amount,
                count: cluster.len(),
                cadence,
                periods_per_year,
                next_date,
                annual,
            });
        }
    }

    if charges.is_empty() {
        return Err(format!(
            "No recurring charges found (nothing repeated at least {min_occ} times). Add more history, or lower the minimum occurrences."
        ));
    }

    // Rank by projected annual cost (desc), tie-break by name for determinism.
    charges.sort_by(|a, b| {
        b.annual
            .partial_cmp(&a.annual)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });

    let total_annual: f64 = charges.iter().map(|c| c.annual).sum();
    let total_monthly = total_annual / 12.0;

    let mut out = String::new();
    let noun = if charges.len() == 1 { "recurring charge" } else { "recurring charges" };
    out.push_str(&format!(
        "Found {} {} · {}/mo · {}/yr projected\n\n",
        charges.len(),
        noun,
        fmt_money(total_monthly, cur),
        fmt_money(total_annual, cur),
    ));
    for (n, c) in charges.iter().enumerate() {
        let next = match c.next_date {
            Some(o) => format!(" · next ~{}", fmt_date(o)),
            None => String::new(),
        };
        let _ = c.periods_per_year;
        out.push_str(&format!(
            "{}. {} — {} {} ×{}{} · {}/yr\n",
            n + 1,
            c.name,
            fmt_money(c.amount, cur),
            c.cadence,
            c.count,
            next,
            fmt_money(c.annual, cur),
        ));
    }
    out.push_str(&format!(
        "\nTotal: {}/mo · {}/yr\n",
        fmt_money(total_monthly, cur),
        fmt_money(total_annual, cur),
    ));
    Ok(out)
}

/// Requested date interpretation.
#[derive(Clone, Copy)]
enum DateFmt {
    Auto,
    Iso,
    Us,
    Eu,
}

impl DateFmt {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(DateFmt::Auto),
            "iso" => Ok(DateFmt::Iso),
            "us" => Ok(DateFmt::Us),
            "eu" => Ok(DateFmt::Eu),
            other => Err(format!(
                "invalid date_format {other:?}: expected \"auto\", \"iso\", \"us\", or \"eu\""
            )),
        }
    }
}

#[derive(Clone, Copy)]
enum SlashLayout {
    Us, // MM/DD/YYYY
    Eu, // DD/MM/YYYY
}

/// Scan every `/`-separated date and pick EU iff some first component exceeds 12
/// (so it can only be a day). Otherwise assume US.
fn detect_slash_layout(transactions: &str) -> SlashLayout {
    for raw in transactions.lines() {
        let line = raw.trim();
        if let Some(first) = line.split(',').next() {
            let f = first.trim();
            if f.contains('/') {
                let comps: Vec<&str> = f.split('/').collect();
                if comps.len() == 3 {
                    if let Ok(a) = comps[0].trim().parse::<i64>() {
                        if a > 12 {
                            return SlashLayout::Eu;
                        }
                    }
                }
            }
        }
    }
    SlashLayout::Us
}

/// Parse a date into days-since-1970 under the chosen format. Returns `None` for
/// anything that isn't a valid `Y-M-D` under the format.
fn parse_date(s: &str, fmt: DateFmt, slash: SlashLayout) -> Option<i64> {
    let s = s.trim();
    let (y, m, d) = if s.contains('-') {
        // ISO-style YYYY-MM-DD (only accepted for Auto/Iso).
        if !matches!(fmt, DateFmt::Auto | DateFmt::Iso) {
            return None;
        }
        let c: Vec<&str> = s.split('-').collect();
        if c.len() != 3 {
            return None;
        }
        (num(c[0])?, num(c[1])?, num(c[2])?)
    } else if s.contains('/') {
        let c: Vec<&str> = s.split('/').collect();
        if c.len() != 3 {
            return None;
        }
        let (a, b, y) = (num(c[0])?, num(c[1])?, num(c[2])?);
        let layout = match fmt {
            DateFmt::Us => SlashLayout::Us,
            DateFmt::Eu => SlashLayout::Eu,
            _ => slash, // Auto (or Iso, which shouldn't reach a slash date)
        };
        match layout {
            SlashLayout::Us => (y, a, b), // MM/DD/YYYY
            SlashLayout::Eu => (y, b, a), // DD/MM/YYYY
        }
    } else {
        return None;
    };
    valid_ymd(y, m, d).then(|| days_from_civil(y, m, d))
}

fn num(s: &str) -> Option<i64> {
    s.trim().parse::<i64>().ok()
}

fn valid_ymd(y: i64, m: i64, d: i64) -> bool {
    if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || d < 1 {
        return false;
    }
    d <= days_in_month(y, m)
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Strip currency symbols/grouping and parse the magnitude. `(9.99)` and `-9.99`
/// are both read as a 9.99 charge. Returns `None` if no number is present.
fn parse_amount(s: &str) -> Option<f64> {
    let mut cleaned = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            cleaned.push(ch);
        }
    }
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok().map(f64::abs)
}

/// Normalize a merchant description for grouping: lowercase, drop the noisy
/// digit/punctuation runs statements append (store #, ref codes, dates), and
/// collapse whitespace to a single space.
fn normalize(desc: &str) -> String {
    let mut out = String::new();
    let mut prev_space = false;
    for ch in desc.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphabetic() {
            out.push(c);
            prev_space = false;
        } else {
            // Any non-letter (digit, symbol, space) becomes a single separator.
            if !prev_space && !out.is_empty() {
                out.push(' ');
            }
            prev_space = true;
        }
    }
    out.trim().to_string()
}

/// Cluster a merchant's rows by amount so minor variations still group. Sorts by
/// amount and starts a new cluster whenever the gap to the previous amount
/// exceeds max(5% of it, $0.50).
fn cluster_by_amount(members: &[usize], txns: &[Txn]) -> Vec<Vec<usize>> {
    let mut sorted: Vec<usize> = members.to_vec();
    sorted.sort_by(|&a, &b| {
        txns[a]
            .amount
            .partial_cmp(&txns[b].amount)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    let mut prev: Option<f64> = None;
    for &i in &sorted {
        let amt = txns[i].amount;
        match prev {
            Some(p) if amt - p > (p * 0.05).max(0.50) => {
                clusters.push(std::mem::take(&mut cur));
                cur.push(i);
            }
            _ => cur.push(i),
        }
        prev = Some(amt);
    }
    if !cur.is_empty() {
        clusters.push(cur);
    }
    clusters
}

/// Median of integer intervals as f64 (average of the two middles when even).
fn median(v: &[i64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_unstable();
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2] as f64
    } else {
        (s[n / 2 - 1] + s[n / 2]) as f64 / 2.0
    }
}

fn median_f64(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        (s[n / 2 - 1] + s[n / 2]) / 2.0
    }
}

/// Map a median inter-charge interval (days) to a cadence label + charges/year.
fn classify_cadence(days: f64) -> (String, f64) {
    if (5.0..=9.5).contains(&days) {
        ("weekly".to_string(), 52.0)
    } else if (9.5..=18.0).contains(&days) {
        ("biweekly".to_string(), 26.0)
    } else if (18.0..=45.0).contains(&days) {
        ("monthly".to_string(), 12.0)
    } else if (45.0..=135.0).contains(&days) {
        ("quarterly".to_string(), 4.0)
    } else if (135.0..=250.0).contains(&days) {
        ("semiannual".to_string(), 2.0)
    } else if (250.0..=430.0).contains(&days) {
        ("annual".to_string(), 1.0)
    } else {
        let per_year = 365.25 / days;
        (format!("every ~{} days", days.round() as i64), per_year)
    }
}

/// Format a money amount as `<symbol>1,234.56`.
fn fmt_money(amount: f64, currency: &str) -> String {
    let cents = (amount * 100.0).round() as i64;
    let whole = cents / 100;
    let frac = (cents % 100).abs();
    let whole_str = group_thousands(whole);
    format!("{currency}{whole_str}.{frac:02}")
}

fn group_thousands(n: i64) -> String {
    let digits = n.abs().to_string();
    let mut out = String::new();
    let len = digits.len();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

/// Format days-since-1970 back to `YYYY-MM-DD`.
fn fmt_date(ord: i64) -> String {
    let (y, m, d) = civil_from_days(ord);
    format!("{y:04}-{m:02}-{d:02}")
}

// --- Proleptic Gregorian day-count (Howard Hinnant's algorithms) ---

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_ranks_monthly_charges() {
        let input = "\
2026-01-01, Netflix, $15.99
2026-02-01, Netflix, $15.99
2026-03-01, Netflix, $15.99
2026-01-05, Spotify, 9.99
2026-02-05, Spotify, 9.99
2026-01-10, Corner Cafe, 4.50";
        let out = find(input, 2, "$", "auto").unwrap();
        let expected = "\
Found 2 recurring charges · $25.98/mo · $311.76/yr projected

1. Netflix — $15.99 monthly ×3 · next ~2026-03-31 · $191.88/yr
2. Spotify — $9.99 monthly ×2 · next ~2026-03-08 · $119.88/yr

Total: $25.98/mo · $311.76/yr
";
        assert_eq!(out, expected);
    }

    #[test]
    fn amount_tolerance_groups_near_prices() {
        // $15.99 and $16.20 from the same merchant are one recurring charge.
        let input = "\
2026-01-01, Acme Cloud, 15.99
2026-02-01, Acme Cloud, 16.20
2026-03-01, Acme Cloud, 15.99";
        let out = find(input, 2, "$", "iso").unwrap();
        assert!(out.contains("×3"), "should merge near-equal amounts: {out}");
        assert!(out.contains("monthly"), "got: {out}");
    }

    #[test]
    fn detects_weekly_cadence() {
        let input = "\
2026-01-01, Daily News, 2.00
2026-01-08, Daily News, 2.00
2026-01-15, Daily News, 2.00
2026-01-22, Daily News, 2.00";
        let out = find(input, 2, "$", "iso").unwrap();
        assert!(out.contains("weekly"), "got: {out}");
        // 4 × $2.00 weekly → $104.00/yr.
        assert!(out.contains("$104.00/yr"), "got: {out}");
    }

    #[test]
    fn eu_date_format_disambiguates() {
        // 13/01 can only be DD/MM, so EU parsing must be picked.
        let input = "\
13/01/2026, Gym, 30.00
13/02/2026, Gym, 30.00";
        let out = find(input, 2, "£", "eu").unwrap();
        assert!(out.contains("£30.00"), "got: {out}");
        assert!(out.contains("monthly"), "got: {out}");
    }

    #[test]
    fn min_occurrences_threshold_drops_singletons() {
        let input = "\
2026-01-01, OneOff Store, 50.00
2026-01-02, Repeaty, 5.00
2026-02-02, Repeaty, 5.00
2026-03-02, Repeaty, 5.00";
        let out = find(input, 3, "$", "iso").unwrap();
        assert!(out.contains("Repeaty"), "got: {out}");
        assert!(!out.contains("OneOff"), "singleton must be dropped: {out}");
        assert!(out.contains("Found 1 recurring charge "), "got: {out}");
    }

    #[test]
    fn thousands_separator_in_totals() {
        // A pricey annual plan surfaces grouped thousands.
        let input = "\
2026-01-01, Enterprise SaaS, 1299.00
2026-01-08, Enterprise SaaS, 1299.00
2026-01-15, Enterprise SaaS, 1299.00";
        let out = find(input, 2, "$", "iso").unwrap();
        assert!(out.contains("$1,299.00"), "got: {out}");
    }

    #[test]
    fn error_on_no_transactions() {
        let err = find("just some prose\nno csv here", 2, "$", "auto").unwrap_err();
        assert!(err.contains("No transactions"), "got: {err}");
    }

    #[test]
    fn error_on_invalid_date_format() {
        let err = find("2026-01-01, X, 1.00", 2, "$", "julian").unwrap_err();
        assert!(err.contains("invalid date_format"), "got: {err}");
    }

    #[test]
    fn error_when_nothing_recurs() {
        let input = "\
2026-01-01, A Store, 5.00
2026-01-02, B Store, 6.00";
        let err = find(input, 2, "$", "iso").unwrap_err();
        assert!(err.contains("No recurring charges"), "got: {err}");
    }

    #[test]
    fn roundtrip_civil_days() {
        for &(y, m, d) in &[(1970, 1, 1), (2000, 2, 29), (2026, 3, 31), (1999, 12, 31)] {
            let ord = days_from_civil(y, m, d);
            assert_eq!(civil_from_days(ord), (y, m, d));
        }
    }
}
