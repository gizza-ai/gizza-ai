//! timesheet-calculator core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Parses a freeform work log of start–stop times tagged by project, then totals
//! hours per project and computes billable amounts. Each entry line looks like:
//!
//! ```text
//! [YYYY-MM-DD]  START-END  PROJECT  [notes...]
//! ```
//!
//! e.g. `9:00-12:30 Acme fixed login bug`, `2024-01-15 13:00-17:15 #Beta`,
//! `10pm-2am OnCall` (overnight — end < start rolls to the next day). Blank lines
//! and lines beginning with `#` or `//` are ignored as comments.
//!
//! Durations can be rounded to a billing increment (6-minute / 0.1h is the legal
//! standard; 15-minute is common in payroll). A global hourly `rate` applies to
//! every entry, with optional per-project overrides via `rates`
//! (`Project=amount`, comma- or newline-separated). All arithmetic is
//! canonicalised through whole minutes so every surface is deterministic.

use serde::Serialize;
use std::collections::BTreeMap;

/// One parsed work-log entry after rounding + billing.
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct Entry {
    /// 1-based source line number.
    pub line: usize,
    /// Optional `YYYY-MM-DD` date prefix, if the line carried one.
    pub date: Option<String>,
    /// Start time, canonicalised to `HH:MM` (24-hour).
    pub start: String,
    /// End time, canonicalised to `HH:MM` (24-hour).
    pub end: String,
    /// Project/tag the entry was booked to.
    pub project: String,
    /// Billed duration in whole minutes (after any rounding).
    pub minutes: i64,
    /// Billed duration in decimal hours, rounded to 4 dp.
    pub hours: f64,
    /// Hourly rate applied to this entry.
    pub rate: f64,
    /// Billable amount for this entry, rounded to 2 dp.
    pub amount: f64,
    /// Free-text notes after the project token (empty if none).
    pub notes: String,
}

/// Per-project rollup.
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct ProjectTotal {
    pub project: String,
    pub minutes: i64,
    pub hours: f64,
    pub rate: f64,
    pub amount: f64,
}

/// The full timesheet report returned from every surface.
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct Report {
    pub entries: Vec<Entry>,
    /// Projects sorted alphabetically by name.
    pub projects: Vec<ProjectTotal>,
    pub total_minutes: i64,
    pub total_hours: f64,
    pub total_amount: f64,
    pub currency: String,
    /// Rounding increment in minutes (0 = no rounding).
    pub round_minutes: i64,
    pub summary: String,
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}
fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

/// Parse one clock time into minutes-since-midnight (0..=1439). Accepts
/// `HH:MM` / `H:MM` 24-hour, an optional `am`/`pm` (or `a`/`p`) suffix, and a
/// bare hour with a meridiem (`9am`, `5pm`). The token must contain no spaces.
fn parse_time(tok: &str) -> Result<i64, String> {
    let t = tok.trim().to_ascii_lowercase();
    if t.is_empty() {
        return Err("empty time".into());
    }
    // Detect an am/pm suffix.
    let (body, mer) = if let Some(b) = t.strip_suffix("am") {
        (b, Some(false))
    } else if let Some(b) = t.strip_suffix("pm") {
        (b, Some(true))
    } else if let Some(b) = t.strip_suffix('a') {
        (b, Some(false))
    } else if let Some(b) = t.strip_suffix('p') {
        (b, Some(true))
    } else {
        (t.as_str(), None)
    };
    let body = body.trim();
    let (h, m) = match body.split_once(':') {
        Some((hs, ms)) => {
            let h: i64 = hs
                .trim()
                .parse()
                .map_err(|_| format!("'{tok}' is not a valid time"))?;
            let m: i64 = ms
                .trim()
                .parse()
                .map_err(|_| format!("'{tok}' is not a valid time"))?;
            (h, m)
        }
        None => {
            let h: i64 = body
                .parse()
                .map_err(|_| format!("'{tok}' is not a valid time"))?;
            (h, 0)
        }
    };
    if !(0..=59).contains(&m) {
        return Err(format!("'{tok}': minutes must be 0-59"));
    }
    let h = match mer {
        // 12-hour clock: 12am → 00:xx, 12pm → 12:xx, otherwise add 12 for pm.
        Some(pm) => {
            if !(1..=12).contains(&h) {
                return Err(format!("'{tok}': 12-hour hour must be 1-12"));
            }
            match (h, pm) {
                (12, false) => 0,
                (12, true) => 12,
                (h, false) => h,
                (h, true) => h + 12,
            }
        }
        None => {
            if !(0..=23).contains(&h) {
                return Err(format!("'{tok}': hour must be 0-23"));
            }
            h
        }
    };
    Ok(h * 60 + m)
}

fn fmt_hm(mins: i64) -> String {
    let m = mins.rem_euclid(1440);
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// Is `tok` a `YYYY-MM-DD` (or `YYYY-M-D`) date token?
fn is_date(tok: &str) -> bool {
    let parts: Vec<&str> = tok.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    if parts[0].len() != 4 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Split a `START-END` range token on the separating dash. Handles ASCII `-`,
/// en dash `–` and em dash `—`.
fn split_range(tok: &str) -> Option<(String, String)> {
    let norm = tok.replace('\u{2013}', "-").replace('\u{2014}', "-");
    let (a, b) = norm.split_once('-')?;
    if a.is_empty() || b.is_empty() {
        return None;
    }
    Some((a.to_string(), b.to_string()))
}

/// Parse the optional `rates` overrides string: `Project=amount` pairs separated
/// by newlines or commas. Later entries win. Blank entries are ignored.
fn parse_rates(s: &str) -> Result<BTreeMap<String, f64>, String> {
    let mut map = BTreeMap::new();
    for raw in s.split(['\n', ',']) {
        let pair = raw.trim();
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair
            .split_once('=')
            .ok_or_else(|| format!("rate override '{pair}' must be 'Project=amount'"))?;
        let key = k.trim();
        let val = v.trim().trim_start_matches(['$', '£', '€']).trim();
        let rate: f64 = val
            .parse()
            .map_err(|_| format!("rate override '{pair}': '{v}' is not a number"))?;
        if key.is_empty() {
            return Err(format!("rate override '{pair}' has an empty project name"));
        }
        map.insert(key.to_string(), rate);
    }
    Ok(map)
}

/// Round `minutes` to the nearest `inc` (ties round up). `inc <= 1` is a no-op.
fn round_minutes(minutes: i64, inc: i64) -> i64 {
    if inc <= 1 {
        return minutes;
    }
    ((minutes as f64) / (inc as f64)).round() as i64 * inc
}

/// Compute the full timesheet report.
///
/// * `log` — the work log, one entry per line.
/// * `rate` — fallback hourly rate for every project (0 = no billing).
/// * `rates` — optional `Project=amount` per-project overrides.
/// * `currency` — currency symbol/prefix for amounts (e.g. `$`).
/// * `round_min` — billing increment in minutes (0 or 1 = exact).
pub fn compute(
    log: &str,
    rate: f64,
    rates: &str,
    currency: &str,
    round_min: i64,
) -> Result<Report, String> {
    if rate < 0.0 {
        return Err("rate must not be negative".into());
    }
    let inc = round_min.max(0);
    let overrides = parse_rates(rates)?;
    let currency = if currency.trim().is_empty() {
        "$"
    } else {
        currency.trim()
    };

    let mut entries: Vec<Entry> = Vec::new();
    for (i, raw) in log.lines().enumerate() {
        let line_no = i + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let mut toks = line.split_whitespace();
        let mut date: Option<String> = None;
        let first = toks
            .next()
            .ok_or_else(|| format!("line {line_no}: empty entry"))?;
        // Optional leading date.
        let range_tok = if is_date(first) {
            date = Some(first.to_string());
            toks.next().ok_or_else(|| {
                format!("line {line_no}: expected a START-END time range after the date")
            })?
        } else {
            first
        };
        let (a, b) = split_range(range_tok).ok_or_else(|| {
            format!("line {line_no}: '{range_tok}' is not a START-END time range")
        })?;
        let start = parse_time(&a).map_err(|e| format!("line {line_no}: {e}"))?;
        let end = parse_time(&b).map_err(|e| format!("line {line_no}: {e}"))?;
        let mut dur = end - start;
        if dur < 0 {
            dur += 1440; // overnight: rolls past midnight
        }
        let dur = round_minutes(dur, inc);

        let project = toks.next().unwrap_or("(no project)");
        let project = project.trim_start_matches('#');
        let project = if project.is_empty() {
            "(no project)"
        } else {
            project
        };
        let notes: Vec<&str> = toks.collect();
        let notes = notes.join(" ");

        let ent_rate = overrides.get(project).copied().unwrap_or(rate);
        let hours = dur as f64 / 60.0;
        let amount = round2(hours * ent_rate);
        entries.push(Entry {
            line: line_no,
            date,
            start: fmt_hm(start),
            end: fmt_hm(end),
            project: project.to_string(),
            minutes: dur,
            hours: round4(hours),
            rate: ent_rate,
            amount,
            notes,
        });
    }

    if entries.is_empty() {
        return Err("no entries found — add at least one 'START-END Project' line".into());
    }

    // Per-project rollup (alphabetical, stable).
    let mut by_project: BTreeMap<String, (i64, f64)> = BTreeMap::new();
    for e in &entries {
        let slot = by_project.entry(e.project.clone()).or_insert((0, e.rate));
        slot.0 += e.minutes;
    }
    let mut projects: Vec<ProjectTotal> = Vec::new();
    for (project, (minutes, rate)) in by_project {
        let hours = minutes as f64 / 60.0;
        projects.push(ProjectTotal {
            project,
            minutes,
            hours: round4(hours),
            rate,
            amount: round2(hours * rate),
        });
    }

    let total_minutes: i64 = entries.iter().map(|e| e.minutes).sum();
    let total_amount: f64 = round2(entries.iter().map(|e| e.amount).sum());
    let total_hours = round4(total_minutes as f64 / 60.0);

    let summary = if total_amount > 0.0 {
        format!(
            "{} entries across {} project(s) · {:.2} h · {}{:.2}",
            entries.len(),
            projects.len(),
            total_hours,
            currency,
            total_amount
        )
    } else {
        format!(
            "{} entries across {} project(s) · {:.2} h",
            entries.len(),
            projects.len(),
            total_hours
        )
    };

    Ok(Report {
        entries,
        projects,
        total_minutes,
        total_hours,
        total_amount,
        currency: currency.to_string(),
        round_minutes: inc,
        summary,
    })
}

/// JSON string form used by the web page + CLI.
pub fn compute_json(
    log: &str,
    rate: f64,
    rates: &str,
    currency: &str,
    round_min: i64,
) -> Result<String, String> {
    let report = compute(log, rate, rates, currency, round_min)?;
    serde_json::to_string_pretty(&report).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_totals_and_billing() {
        let log = "\
9:00-12:30 Acme kickoff call
13:00-17:15 Acme build feature
2024-01-15 10:00-11:00 #Beta review
";
        let r = compute(log, 100.0, "", "$", 0).unwrap();
        assert_eq!(r.entries.len(), 3);
        // Acme: 3h30 + 4h15 = 7h45 = 465 min; Beta: 60 min.
        let acme = r.projects.iter().find(|p| p.project == "Acme").unwrap();
        assert_eq!(acme.minutes, 465);
        assert_eq!(acme.hours, 7.75);
        assert_eq!(acme.amount, 775.0);
        let beta = r.projects.iter().find(|p| p.project == "Beta").unwrap();
        assert_eq!(beta.minutes, 60);
        assert_eq!(beta.amount, 100.0);
        assert_eq!(r.total_minutes, 525);
        assert_eq!(r.total_hours, 8.75);
        assert_eq!(r.total_amount, 875.0);
        assert_eq!(r.entries[2].date.as_deref(), Some("2024-01-15"));
        assert_eq!(r.entries[0].notes, "kickoff call");
    }

    #[test]
    fn overnight_and_meridiem() {
        let r = compute("10pm-2am OnCall", 0.0, "", "$", 0).unwrap();
        assert_eq!(r.entries[0].start, "22:00");
        assert_eq!(r.entries[0].end, "02:00");
        assert_eq!(r.total_minutes, 240);
    }

    #[test]
    fn per_project_rate_override() {
        let log = "9:00-10:00 Acme\n9:00-10:00 Beta";
        let r = compute(log, 50.0, "Acme=200", "$", 0).unwrap();
        let acme = r.projects.iter().find(|p| p.project == "Acme").unwrap();
        let beta = r.projects.iter().find(|p| p.project == "Beta").unwrap();
        assert_eq!(acme.amount, 200.0);
        assert_eq!(beta.amount, 50.0);
        assert_eq!(r.total_amount, 250.0);
    }

    #[test]
    fn rounding_to_six_minutes() {
        // 9:00-9:04 = 4 min → nearest 6-min increment = 6 min = 0.1 h.
        let r = compute("9:00-9:04 Legal", 300.0, "", "$", 6).unwrap();
        assert_eq!(r.entries[0].minutes, 6);
        assert_eq!(r.entries[0].hours, 0.1);
        assert_eq!(r.total_amount, 30.0);
        assert_eq!(r.round_minutes, 6);
    }

    #[test]
    fn error_on_empty_log() {
        assert!(compute("   \n# just a comment", 0.0, "", "$", 0).is_err());
    }

    #[test]
    fn error_on_bad_time() {
        let e = compute("25:00-26:00 Acme", 0.0, "", "$", 0).unwrap_err();
        assert!(e.contains("line 1"), "got: {e}");
    }

    #[test]
    fn error_on_missing_range() {
        let e = compute("Acme did some work", 0.0, "", "$", 0).unwrap_err();
        assert!(e.contains("time range"), "got: {e}");
    }

    #[test]
    fn json_shape() {
        let j = compute_json("9:00-10:00 Acme", 10.0, "", "$", 0).unwrap();
        assert!(j.contains("\"total_hours\": 1.0"));
        assert!(j.contains("\"summary\""));
    }
}
