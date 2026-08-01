//! gizza-ai/funnel-conversion-analyzer core — step-by-step conversion and
//! drop-off rates from an event CSV. For an ordered list of funnel steps it
//! counts the unique users reaching each step, the conversion from the top and
//! from the previous step, and the users lost (drop-off) at each step.
//! Pure-Rust (`csv`, `serde_json`). No wafer/wasm-bindgen deps.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use serde::Serialize;

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepStats {
    pub step: String,
    /// Unique users who reached this step.
    pub users: usize,
    /// Users at this step ÷ users at the first (top) step, percent.
    pub conversion_from_top: f64,
    /// Users at this step ÷ users at the previous step, percent (100 for the first step).
    pub conversion_from_prev: f64,
    /// Users lost since the previous step (0 for the first step).
    pub dropoff: usize,
    /// Drop-off since the previous step ÷ users at the previous step, percent (0 for the first step).
    pub dropoff_rate: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Funnel {
    pub steps: Vec<StepStats>,
    /// Distinct users that had at least one event in the data.
    pub total_users: usize,
    /// Users at the last step ÷ users at the first step, percent.
    pub overall_conversion: f64,
}

/// Round a 0..1 ratio to a percentage with 2 decimals.
fn pct(ratio: f64) -> f64 {
    (ratio * 10000.0).round() / 100.0
}

/// A sortable timestamp key: numeric epochs sort by value, everything else
/// (ISO-8601 strings included, which sort lexicographically) sorts as text.
#[derive(Clone)]
enum TimeKey {
    Num(f64),
    Txt(String),
}

fn time_key(s: &str) -> TimeKey {
    match s.parse::<f64>() {
        Ok(n) if n.is_finite() => TimeKey::Num(n),
        _ => TimeKey::Txt(s.to_string()),
    }
}

fn cmp_key(a: &TimeKey, b: &TimeKey) -> Ordering {
    match (a, b) {
        (TimeKey::Num(x), TimeKey::Num(y)) => x.total_cmp(y),
        (TimeKey::Txt(x), TimeKey::Txt(y)) => x.cmp(y),
        (TimeKey::Num(_), TimeKey::Txt(_)) => Ordering::Less,
        (TimeKey::Txt(_), TimeKey::Num(_)) => Ordering::Greater,
    }
}

#[derive(Default)]
struct UserAgg {
    /// All event names this user performed.
    set: HashSet<String>,
    /// (time, original row index, event) — only funnel-step events, only when a time column is set.
    seq: Vec<(TimeKey, usize, String)>,
}

/// Resolve a column reference (a header name, or a 1-based index) to a 0-based
/// column index. An empty `spec` falls back to `default_idx`.
fn resolve_col(spec: &str, names: &[String], default_idx: usize, label: &str) -> Result<usize, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        if default_idx < names.len() {
            return Ok(default_idx);
        }
        return Err(format!(
            "{label} column not given and the data has only {} column(s)",
            names.len()
        ));
    }
    if let Ok(n) = spec.parse::<usize>() {
        if (1..=names.len()).contains(&n) {
            return Ok(n - 1);
        }
        return Err(format!(
            "{label} column index {n} is out of range (1..={})",
            names.len()
        ));
    }
    names
        .iter()
        .position(|c| c == spec)
        .ok_or_else(|| format!("{label} column '{spec}' not found in the header row"))
}

/// Compute the full funnel from an event CSV.
#[allow(clippy::too_many_arguments)]
pub fn compute(
    data: &str,
    steps_spec: &str,
    user_spec: &str,
    event_spec: &str,
    time_spec: &str,
    ordered: bool,
    has_header: bool,
    delimiter: &str,
) -> Result<Funnel, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> =
        rdr.records().collect::<Result<_, _>>().map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Err("no rows found".into());
    }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);

    let (names, data_rows): (Vec<String>, &[csv::StringRecord]) = if has_header {
        let h = &records[0];
        let mut names: Vec<String> = (0..width).map(|i| h.get(i).unwrap_or("").to_string()).collect();
        for (i, n) in names.iter_mut().enumerate() {
            if n.is_empty() {
                *n = format!("col{}", i + 1);
            }
        }
        (names, &records[1..])
    } else {
        ((0..width).map(|i| format!("col{}", i + 1)).collect(), &records[..])
    };
    if data_rows.is_empty() {
        return Err("no data rows found".into());
    }

    let user_idx = resolve_col(user_spec, &names, 0, "user")?;
    let event_idx = resolve_col(event_spec, &names, 1, "event")?;
    let time_idx = if time_spec.trim().is_empty() {
        None
    } else {
        Some(resolve_col(time_spec, &names, 0, "time")?)
    };

    // Funnel steps: explicit list, or auto-derived distinct events in first-seen order.
    let steps: Vec<String> = if steps_spec.trim().is_empty() {
        let mut seen: HashSet<String> = HashSet::new();
        let mut v = Vec::new();
        for rec in data_rows {
            let e = rec.get(event_idx).unwrap_or("").trim();
            if !e.is_empty() && seen.insert(e.to_string()) {
                v.push(e.to_string());
            }
        }
        v
    } else {
        steps_spec.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    };
    if steps.len() < 2 {
        return Err(
            "a funnel needs at least 2 steps: set `steps` to a comma-separated list \
             (e.g. \"view,signup,purchase\") or supply data with 2 or more distinct events"
                .into(),
        );
    }
    let step_set: HashSet<&str> = steps.iter().map(|s| s.as_str()).collect();

    // Aggregate per user.
    let mut users: HashMap<String, UserAgg> = HashMap::new();
    for (ri, rec) in data_rows.iter().enumerate() {
        let uid = rec.get(user_idx).unwrap_or("").trim();
        if uid.is_empty() {
            continue;
        }
        let ev = rec.get(event_idx).unwrap_or("").trim();
        if ev.is_empty() {
            continue;
        }
        let entry = users.entry(uid.to_string()).or_default();
        entry.set.insert(ev.to_string());
        if let Some(ti) = time_idx {
            if step_set.contains(ev) {
                let t = rec.get(ti).unwrap_or("").trim();
                entry.seq.push((time_key(t), ri, ev.to_string()));
            }
        }
    }
    let total_users = users.len();
    if total_users == 0 {
        return Err("no rows with both a user id and an event value".into());
    }

    // Count users reaching each step.
    let mut reached = vec![0usize; steps.len()];
    for agg in users.values() {
        if ordered {
            let depth = if time_idx.is_some() {
                // Greedy in-order subsequence match over the user's time-sorted events.
                let mut seq = agg.seq.clone();
                seq.sort_by(|a, b| cmp_key(&a.0, &b.0).then(a.1.cmp(&b.1)));
                let mut p = 0usize;
                for (_, _, ev) in &seq {
                    if p < steps.len() && *ev == steps[p] {
                        p += 1;
                    }
                }
                p
            } else {
                // Set membership: reached step i iff every step up to i was performed.
                let mut d = 0usize;
                while d < steps.len() && agg.set.contains(&steps[d]) {
                    d += 1;
                }
                d
            };
            for r in reached.iter_mut().take(depth) {
                *r += 1;
            }
        } else {
            // Independent: reached step i iff the user performed step i at all.
            for (i, s) in steps.iter().enumerate() {
                if agg.set.contains(s) {
                    reached[i] += 1;
                }
            }
        }
    }

    let top = reached[0];
    let mut out_steps = Vec::with_capacity(steps.len());
    for (i, name) in steps.iter().enumerate() {
        let u = reached[i];
        let prev = if i == 0 { u } else { reached[i - 1] };
        let conversion_from_top = if top == 0 { 0.0 } else { pct(u as f64 / top as f64) };
        let conversion_from_prev =
            if i == 0 { 100.0 } else if prev == 0 { 0.0 } else { pct(u as f64 / prev as f64) };
        let dropoff = if i == 0 { 0 } else { prev.saturating_sub(u) };
        let dropoff_rate =
            if i == 0 || prev == 0 { 0.0 } else { pct(dropoff as f64 / prev as f64) };
        out_steps.push(StepStats {
            step: name.clone(),
            users: u,
            conversion_from_top,
            conversion_from_prev,
            dropoff,
            dropoff_rate,
        });
    }
    let overall = if top == 0 { 0.0 } else { pct(reached[steps.len() - 1] as f64 / top as f64) };

    Ok(Funnel { steps: out_steps, total_users, overall_conversion: overall })
}

/// Plain-text funnel table (used by the page and CLI text view).
fn render_table(f: &Funnel) -> String {
    let mut out = format!("Funnel: {} step(s), {} total user(s)\n", f.steps.len(), f.total_users);
    for (i, s) in f.steps.iter().enumerate() {
        if i == 0 {
            out.push_str(&format!(
                "{}. {}: {} users | {}% of top\n",
                i + 1,
                s.step,
                s.users,
                s.conversion_from_top
            ));
        } else {
            out.push_str(&format!(
                "{}. {}: {} users | {}% of top | {}% from prev | drop {} ({}%)\n",
                i + 1,
                s.step,
                s.users,
                s.conversion_from_top,
                s.conversion_from_prev,
                s.dropoff,
                s.dropoff_rate
            ));
        }
    }
    out.push_str(&format!("Overall conversion: {}%", f.overall_conversion));
    out
}

/// Compute the funnel and render it as `format` ("table" or "json").
#[allow(clippy::too_many_arguments)]
pub fn analyze(
    data: &str,
    steps_spec: &str,
    user_spec: &str,
    event_spec: &str,
    time_spec: &str,
    ordered: bool,
    has_header: bool,
    delimiter: &str,
    format: &str,
) -> Result<String, String> {
    let f = compute(data, steps_spec, user_spec, event_spec, time_spec, ordered, has_header, delimiter)?;
    match format.trim() {
        "" | "table" => Ok(render_table(&f)),
        "json" => serde_json::to_string_pretty(&f).map_err(|e| e.to_string()),
        other => Err(format!("format must be 'table' or 'json', got '{other}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "user,event\n\
        u1,view\nu1,signup\nu1,purchase\n\
        u2,view\nu2,signup\n\
        u3,view";

    #[test]
    fn ordered_funnel_conversion_and_dropoff() {
        let f = compute(SAMPLE, "view,signup,purchase", "user", "event", "", true, true, ",").unwrap();
        assert_eq!(f.total_users, 3);
        assert_eq!(f.steps.len(), 3);
        // reached = [3, 2, 1]
        assert_eq!(f.steps[0].users, 3);
        assert_eq!(f.steps[0].conversion_from_top, 100.0);
        assert_eq!(f.steps[1].users, 2);
        assert_eq!(f.steps[1].conversion_from_prev, 66.67);
        assert_eq!(f.steps[1].dropoff, 1);
        assert_eq!(f.steps[1].dropoff_rate, 33.33);
        assert_eq!(f.steps[2].users, 1);
        assert_eq!(f.steps[2].conversion_from_prev, 50.0);
        assert_eq!(f.steps[2].dropoff, 1);
        assert_eq!(f.overall_conversion, 33.33);
    }

    #[test]
    fn ordered_requires_prior_steps_but_independent_does_not() {
        // u4 did purchase without signup (skipped a step).
        let data = format!("{SAMPLE}\nu4,view\nu4,purchase");
        // Ordered: u4 reaches only `view` (missing signup), so purchase stays at 1.
        let ord = compute(&data, "view,signup,purchase", "user", "event", "", true, true, ",").unwrap();
        assert_eq!(ord.steps[0].users, 4);
        assert_eq!(ord.steps[2].users, 1);
        // Independent: purchase counts every user who ever purchased (u1 + u4).
        let ind = compute(&data, "view,signup,purchase", "user", "event", "", false, true, ",").unwrap();
        assert_eq!(ind.steps[2].users, 2);
    }

    #[test]
    fn time_column_enforces_chronological_order() {
        // u9 performed purchase BEFORE signup — with time ordering they only reach `view`.
        let data = "user,event,ts\n\
            u9,view,1\nu9,purchase,2\nu9,signup,3";
        let f = compute(data, "view,signup,purchase", "user", "event", "ts", true, true, ",").unwrap();
        assert_eq!(f.steps[0].users, 1); // view
        assert_eq!(f.steps[1].users, 1); // signup happened (after view) at t=3
        assert_eq!(f.steps[2].users, 0); // purchase was before signup → not counted
    }

    #[test]
    fn auto_derives_steps_and_renders_table() {
        let out = analyze(SAMPLE, "", "user", "event", "", true, true, ",", "table").unwrap();
        assert!(out.contains("Funnel: 3 step(s), 3 total user(s)"));
        assert!(out.contains("Overall conversion: 33.33%"));
    }

    #[test]
    fn json_output_is_structured() {
        let out = analyze(SAMPLE, "view,signup,purchase", "user", "event", "", true, true, ",", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["total_users"], 3);
        assert_eq!(v["steps"][1]["dropoff_rate"], 33.33);
        assert_eq!(v["overall_conversion"], 33.33);
    }

    #[test]
    fn column_by_index_works() {
        let data = "u1,view\nu1,buy\nu2,view";
        let f = compute(data, "view,buy", "1", "2", "", true, false, ",").unwrap();
        assert_eq!(f.total_users, 2);
        assert_eq!(f.steps[1].users, 1);
    }

    #[test]
    fn errors() {
        assert!(analyze("", "", "", "", "", true, true, ",", "table").is_err()); // empty
        assert!(compute("user,event\nu1,view", "view", "user", "event", "", true, true, ",").is_err()); // <2 steps
        assert!(compute(SAMPLE, "view,signup", "user", "event", "", true, true, "xx").is_err()); // bad delimiter
        assert!(compute(SAMPLE, "view,signup", "nope", "event", "", true, true, ",").is_err()); // missing column
        assert!(analyze(SAMPLE, "view,signup", "user", "event", "", true, true, ",", "xml").is_err()); // bad format
    }
}
