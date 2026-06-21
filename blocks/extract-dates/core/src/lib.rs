//! gizza-ai/extract-dates core — scan text for date and time mentions and
//! normalize each to ISO 8601. Pure-Rust (`regex` + `chrono`).
//!
//! Recognizes:
//!   * ISO dates/datetimes: 2024-01-05, 2024-01-05T14:30[:00]
//!   * numeric dates: 01/05/2024 (month-first by default), 2024/01/05
//!   * month-name dates: "January 5, 2024", "5 Jan 2024", "Jan. 5 2024"
//!   * clock times: 14:30, 2:30:15, 3pm, 9:05 AM
//!
//! Ambiguous numeric dates (e.g. 01/05/2024) are read month-first (US style)
//! unless the first field is > 12, in which case it's day-first.

use chrono::NaiveDate;
use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Found {
    /// The exact text that matched.
    pub text: String,
    /// Normalized ISO-8601 value.
    pub iso: String,
    /// "date", "datetime", or "time".
    pub kind: String,
}

struct Cand {
    start: usize,
    end: usize,
    iso: String,
    kind: &'static str,
}

fn month_num(name: &str) -> Option<u32> {
    let n = name.trim_end_matches('.').to_ascii_lowercase();
    let m = match n.as_str() {
        "jan" | "january" => 1,
        "feb" | "february" => 2,
        "mar" | "march" => 3,
        "apr" | "april" => 4,
        "may" => 5,
        "jun" | "june" => 6,
        "jul" | "july" => 7,
        "aug" | "august" => 8,
        "sep" | "sept" | "september" => 9,
        "oct" | "october" => 10,
        "nov" | "november" => 11,
        "dec" | "december" => 12,
        _ => return None,
    };
    Some(m)
}

fn full_year(y: i32) -> i32 {
    if y >= 100 {
        y
    } else if y < 70 {
        2000 + y
    } else {
        1900 + y
    }
}

fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// Normalize a 12/24h clock time to `HH:MM:SS`. Returns None if invalid.
fn norm_time(h: u32, m: u32, s: u32, ampm: Option<&str>) -> Option<String> {
    if m > 59 || s > 59 {
        return None;
    }
    let h24 = match ampm.map(|a| a.to_ascii_lowercase().replace('.', "")) {
        Some(a) if a.starts_with('p') => {
            if h == 0 || h > 12 {
                return None;
            }
            if h == 12 {
                12
            } else {
                h + 12
            }
        }
        Some(a) if a.starts_with('a') => {
            if h == 0 || h > 12 {
                return None;
            }
            if h == 12 {
                0
            } else {
                h
            }
        }
        _ => {
            if h > 23 {
                return None;
            }
            h
        }
    };
    Some(format!("{h24:02}:{m:02}:{s:02}"))
}

thread_local! {
    static RES: Regexes = Regexes::new();
}

struct Regexes {
    iso: Regex,
    ymd_slash: Regex,
    numeric: Regex,
    month_first: Regex,
    day_first: Regex,
    time: Regex,
    time_ampm: Regex,
}

impl Regexes {
    fn new() -> Self {
        let months = "Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:t)?(?:ember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?";
        Regexes {
            iso: Regex::new(r"(?i)\b(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}):(\d{2})(?::(\d{2}))?)?\b").unwrap(),
            ymd_slash: Regex::new(r"\b(\d{4})/(\d{1,2})/(\d{1,2})\b").unwrap(),
            numeric: Regex::new(r"\b(\d{1,2})[/-](\d{1,2})[/-](\d{2,4})\b").unwrap(),
            month_first: Regex::new(&format!(
                r"(?i)\b({months})\.?\s+(\d{{1,2}})(?:st|nd|rd|th)?,?\s+(\d{{4}})\b"
            ))
            .unwrap(),
            day_first: Regex::new(&format!(
                r"(?i)\b(\d{{1,2}})(?:st|nd|rd|th)?\s+({months})\.?,?\s+(\d{{4}})\b"
            ))
            .unwrap(),
            time: Regex::new(r"(?i)\b(\d{1,2}):(\d{2})(?::(\d{2}))?\s*([ap]\.?m\.?)?").unwrap(),
            // bare hour + required am/pm (no colon), e.g. "3pm", "9 AM".
            time_ampm: Regex::new(r"(?i)\b(\d{1,2})\s*([ap]\.?m\.?)\b").unwrap(),
        }
    }
}

fn push_nonoverlap(cands: &mut Vec<Cand>, c: Cand) {
    // Drop if it overlaps an already-accepted candidate.
    if cands.iter().any(|e| c.start < e.end && e.start < c.end) {
        return;
    }
    cands.push(c);
}

/// Scan `text` and return every date/time found, normalized to ISO 8601.
pub fn extract(text: &str) -> Vec<Found> {
    RES.with(|r| {
        let mut cands: Vec<Cand> = Vec::new();

        // 1. ISO (highest priority — handles date + optional time).
        for c in r.iso.captures_iter(text) {
            let m = c.get(0).unwrap();
            let y: i32 = c[1].parse().unwrap();
            let mo: u32 = c[2].parse().unwrap();
            let d: u32 = c[3].parse().unwrap();
            let Some(date) = NaiveDate::from_ymd_opt(y, mo, d) else { continue };
            if let Some(h) = c.get(4) {
                let hh: u32 = h.as_str().parse().unwrap();
                let mm: u32 = c[5].parse().unwrap();
                let ss: u32 = c.get(6).map(|x| x.as_str().parse().unwrap()).unwrap_or(0);
                let Some(t) = norm_time(hh, mm, ss, None) else { continue };
                push_nonoverlap(&mut cands, Cand { start: m.start(), end: m.end(), iso: format!("{}T{}", fmt_date(date), t), kind: "datetime" });
            } else {
                push_nonoverlap(&mut cands, Cand { start: m.start(), end: m.end(), iso: fmt_date(date), kind: "date" });
            }
        }

        // 2. Y/M/D slash.
        for c in r.ymd_slash.captures_iter(text) {
            let m = c.get(0).unwrap();
            let (y, mo, d): (i32, u32, u32) = (c[1].parse().unwrap(), c[2].parse().unwrap(), c[3].parse().unwrap());
            if let Some(date) = NaiveDate::from_ymd_opt(y, mo, d) {
                push_nonoverlap(&mut cands, Cand { start: m.start(), end: m.end(), iso: fmt_date(date), kind: "date" });
            }
        }

        // 3. Month-name first / day first.
        for c in r.month_first.captures_iter(text) {
            let m = c.get(0).unwrap();
            let Some(mo) = month_num(&c[1]) else { continue };
            let d: u32 = c[2].parse().unwrap();
            let y: i32 = c[3].parse().unwrap();
            if let Some(date) = NaiveDate::from_ymd_opt(y, mo, d) {
                push_nonoverlap(&mut cands, Cand { start: m.start(), end: m.end(), iso: fmt_date(date), kind: "date" });
            }
        }
        for c in r.day_first.captures_iter(text) {
            let m = c.get(0).unwrap();
            let d: u32 = c[1].parse().unwrap();
            let Some(mo) = month_num(&c[2]) else { continue };
            let y: i32 = c[3].parse().unwrap();
            if let Some(date) = NaiveDate::from_ymd_opt(y, mo, d) {
                push_nonoverlap(&mut cands, Cand { start: m.start(), end: m.end(), iso: fmt_date(date), kind: "date" });
            }
        }

        // 4. Numeric slash/dash dates (ambiguous → month-first unless first > 12).
        for c in r.numeric.captures_iter(text) {
            let m = c.get(0).unwrap();
            let a: u32 = c[1].parse().unwrap();
            let b: u32 = c[2].parse().unwrap();
            let y: i32 = full_year(c[3].parse().unwrap());
            let (mo, d) = if a > 12 && b <= 12 { (b, a) } else { (a, b) };
            if let Some(date) = NaiveDate::from_ymd_opt(y, mo, d) {
                push_nonoverlap(&mut cands, Cand { start: m.start(), end: m.end(), iso: fmt_date(date), kind: "date" });
            }
        }

        // 5. Standalone clock times (lowest priority; skips times inside ISO datetimes).
        for c in r.time.captures_iter(text) {
            let m = c.get(0).unwrap();
            let h: u32 = c[1].parse().unwrap();
            let mm: u32 = c[2].parse().unwrap();
            let ss: u32 = c.get(3).map(|x| x.as_str().parse().unwrap()).unwrap_or(0);
            let ampm = c.get(4).map(|x| x.as_str());
            if let Some(t) = norm_time(h, mm, ss, ampm) {
                push_nonoverlap(&mut cands, Cand { start: m.start(), end: m.end(), iso: t, kind: "time" });
            }
        }

        // 6. Bare hour + am/pm (e.g. "3pm").
        for c in r.time_ampm.captures_iter(text) {
            let m = c.get(0).unwrap();
            let h: u32 = c[1].parse().unwrap();
            if let Some(t) = norm_time(h, 0, 0, Some(&c[2])) {
                push_nonoverlap(&mut cands, Cand { start: m.start(), end: m.end(), iso: t, kind: "time" });
            }
        }

        cands.sort_by_key(|c| c.start);
        cands
            .into_iter()
            .map(|c| Found { text: text[c.start..c.end].trim().to_string(), iso: c.iso, kind: c.kind.to_string() })
            .collect()
    })
}

/// Human-readable rendering (used by the page).
pub fn render(text: &str) -> Result<String, String> {
    let found = extract(text);
    if found.is_empty() {
        return Ok("No dates or times found.".to_string());
    }
    let mut out = format!("{} found:\n", found.len());
    for f in &found {
        out.push_str(&format!("  {} → {} [{}]\n", f.text, f.iso, f.kind));
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn isos(t: &str) -> Vec<String> {
        extract(t).into_iter().map(|f| f.iso).collect()
    }

    #[test]
    fn iso_date_and_datetime() {
        assert_eq!(isos("on 2024-01-05 we met"), vec!["2024-01-05"]);
        assert_eq!(isos("at 2024-01-05T14:30:00 sharp"), vec!["2024-01-05T14:30:00"]);
        assert_eq!(isos("2024-01-05 14:30"), vec!["2024-01-05T14:30:00"]);
    }

    #[test]
    fn month_names() {
        assert_eq!(isos("January 5, 2024"), vec!["2024-01-05"]);
        assert_eq!(isos("5 Jan 2024"), vec!["2024-01-05"]);
        assert_eq!(isos("Dec. 25 1999"), vec!["1999-12-25"]);
    }

    #[test]
    fn numeric_ambiguous() {
        assert_eq!(isos("01/05/2024"), vec!["2024-01-05"]); // month-first
        assert_eq!(isos("25/12/2024"), vec!["2024-12-25"]); // first>12 → day-first
        assert_eq!(isos("2024/01/05"), vec!["2024-01-05"]); // y/m/d
        assert_eq!(isos("3/4/99"), vec!["1999-03-04"]); // 2-digit year
    }

    #[test]
    fn times() {
        assert_eq!(isos("meet at 3pm"), vec!["15:00:00"]);
        assert_eq!(isos("9:05 AM"), vec!["09:05:00"]);
        assert_eq!(isos("at 14:30:15"), vec!["14:30:15"]);
        assert_eq!(isos("12am midnight"), vec!["00:00:00"]); // 12am → midnight
    }

    #[test]
    fn invalid_dropped() {
        assert!(isos("2024-13-40").is_empty()); // bad month/day
        assert!(isos("25:99").is_empty()); // bad time
        assert_eq!(isos("Feb 30, 2024"), Vec::<String>::new()); // no Feb 30
    }

    #[test]
    fn multiple_in_order() {
        let r = isos("From 2024-01-01 to March 3, 2024 at 5pm.");
        assert_eq!(r, vec!["2024-01-01", "2024-03-03", "17:00:00"]);
    }

    #[test]
    fn render_works() {
        let r = render("Due 2024-01-05.").unwrap();
        assert!(r.contains("2024-01-05"));
        assert!(render("nothing here").unwrap().contains("No dates"));
    }
}
