//! period-predictor core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Given the first day of the most recent period, an average cycle length, a
//! bleeding (period) duration, a luteal-phase length and a number of cycles to
//! project, predicts the upcoming period start dates and, for each cycle, the
//! bleeding-end date, the estimated ovulation day (period start − luteal phase)
//! and the 6-day fertile window (five days before ovulation through ovulation
//! day). All math is in naive civil dates — no time zones. Predictions are
//! estimates only, not a contraceptive method or medical advice.

use chrono::{NaiveDate, NaiveDateTime};
use serde::Serialize;

/// One predicted menstrual cycle.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Cycle {
    /// 1-based cycle index, counting forward from the last recorded period.
    pub cycle: i64,
    /// Predicted first day of this period (`YYYY-MM-DD`).
    pub period_start: String,
    /// Weekday name of `period_start` (e.g. "Wednesday").
    pub period_start_weekday: String,
    /// Predicted last bleeding day of this period (`YYYY-MM-DD`),
    /// = `period_start` + `period_length` − 1.
    pub period_end: String,
    /// Estimated ovulation day (`YYYY-MM-DD`), = `period_start` − `luteal_phase`.
    pub ovulation_date: String,
    /// First day of the fertile window (`YYYY-MM-DD`), = ovulation − 5 days.
    pub fertile_window_start: String,
    /// Last day of the fertile window (`YYYY-MM-DD`), = ovulation day.
    pub fertile_window_end: String,
}

/// Structured prediction result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Prediction {
    /// The last recorded period start, normalized to `YYYY-MM-DD`.
    pub last_period: String,
    /// Average cycle length used, in days.
    pub cycle_length: i64,
    /// Period (bleeding) duration used, in days.
    pub period_length: i64,
    /// Luteal-phase length used for the ovulation estimate, in days.
    pub luteal_phase: i64,
    /// The first predicted period start (`YYYY-MM-DD`) — same as `cycles[0]`.
    pub next_period_start: String,
    /// Predicted upcoming cycles, earliest first.
    pub cycles: Vec<Cycle>,
    /// Human-readable one-line summary.
    pub summary: String,
}

/// Parse a flexible date or datetime string into a naive date (time is dropped).
///
/// Accepts RFC-3339 (with `Z`/offset), `YYYY-MM-DDTHH:MM[:SS]` /
/// `YYYY-MM-DD HH:MM[:SS]`, and the date-only forms `YYYY-MM-DD`, `YYYY/MM/DD`,
/// `MM/DD/YYYY`, `DD.MM.YYYY`.
fn parse_date(s: &str) -> Result<NaiveDate, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err("empty date".into());
    }

    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(t) {
        return Ok(dt.naive_local().date());
    }
    for fmt in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(t, fmt) {
            return Ok(dt.date());
        }
    }
    for fmt in ["%Y-%m-%d", "%Y/%m/%d", "%m/%d/%Y", "%d.%m.%Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(t, fmt) {
            return Ok(d);
        }
    }
    Err(format!(
        "could not parse date `{t}` — try YYYY-MM-DD (or RFC-3339, YYYY/MM/DD, MM/DD/YYYY, DD.MM.YYYY)"
    ))
}

/// Accepted bounds for the numeric inputs (kept in sync with the descriptor).
const CYCLE_MIN: i64 = 20;
const CYCLE_MAX: i64 = 45;
const PERIOD_MIN: i64 = 1;
const PERIOD_MAX: i64 = 14;
const LUTEAL_MIN: i64 = 9;
const LUTEAL_MAX: i64 = 17;
const CYCLES_MIN: i64 = 1;
const CYCLES_MAX: i64 = 24;

/// Predict the upcoming cycles from a parsed last-period date.
pub fn predict(
    last: NaiveDate,
    cycle_length: i64,
    period_length: i64,
    luteal_phase: i64,
    cycles: i64,
) -> Result<Prediction, String> {
    if !(CYCLE_MIN..=CYCLE_MAX).contains(&cycle_length) {
        return Err(format!(
            "cycle_length must be between {CYCLE_MIN} and {CYCLE_MAX} days (got {cycle_length})"
        ));
    }
    if !(PERIOD_MIN..=PERIOD_MAX).contains(&period_length) {
        return Err(format!(
            "period_length must be between {PERIOD_MIN} and {PERIOD_MAX} days (got {period_length})"
        ));
    }
    if !(LUTEAL_MIN..=LUTEAL_MAX).contains(&luteal_phase) {
        return Err(format!(
            "luteal_phase must be between {LUTEAL_MIN} and {LUTEAL_MAX} days (got {luteal_phase})"
        ));
    }
    if !(CYCLES_MIN..=CYCLES_MAX).contains(&cycles) {
        return Err(format!(
            "cycles must be between {CYCLES_MIN} and {CYCLES_MAX} (got {cycles})"
        ));
    }
    if period_length >= cycle_length {
        return Err(format!(
            "period_length ({period_length}) must be shorter than cycle_length ({cycle_length})"
        ));
    }
    if luteal_phase >= cycle_length {
        return Err(format!(
            "luteal_phase ({luteal_phase}) must be shorter than cycle_length ({cycle_length})"
        ));
    }

    let mut out = Vec::with_capacity(cycles as usize);
    for k in 1..=cycles {
        let start = last + chrono::Duration::days(cycle_length * k);
        let end = start + chrono::Duration::days(period_length - 1);
        let ovulation = start - chrono::Duration::days(luteal_phase);
        let fertile_start = ovulation - chrono::Duration::days(5);
        out.push(Cycle {
            cycle: k,
            period_start: start.to_string(),
            period_start_weekday: start.format("%A").to_string(),
            period_end: end.to_string(),
            ovulation_date: ovulation.to_string(),
            fertile_window_start: fertile_start.to_string(),
            fertile_window_end: ovulation.to_string(),
        });
    }

    let next = out[0].period_start.clone();
    let summary = format!(
        "Next period expected {next} ({}). {cycles} cycle{} predicted on a {cycle_length}-day cycle.",
        out[0].period_start_weekday,
        if cycles == 1 { "" } else { "s" }
    );

    Ok(Prediction {
        last_period: last.to_string(),
        cycle_length,
        period_length,
        luteal_phase,
        next_period_start: next,
        cycles: out,
        summary,
    })
}

/// Convenience: parse the string date and predict. Numeric params are already
/// integers (each surface parses its own strings before calling).
pub fn period_predict(
    last_period: &str,
    cycle_length: i64,
    period_length: i64,
    luteal_phase: i64,
    cycles: i64,
) -> Result<Prediction, String> {
    let last = parse_date(last_period)?;
    predict(last, cycle_length, period_length, luteal_phase, cycles)
}

/// Same as [`period_predict`] but returns a pretty-printed JSON string. Used by
/// the web page.
pub fn period_predict_json(
    last_period: &str,
    cycle_length: i64,
    period_length: i64,
    luteal_phase: i64,
    cycles: i64,
) -> Result<String, String> {
    let r = period_predict(last_period, cycle_length, period_length, luteal_phase, cycles)?;
    serde_json::to_string_pretty(&r).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn standard_28_day_cycle() {
        let r = predict(d(2026, 7, 1), 28, 5, 14, 3).unwrap();
        assert_eq!(r.next_period_start, "2026-07-29");
        assert_eq!(r.cycles.len(), 3);
        assert_eq!(r.cycles[0].period_start, "2026-07-29");
        assert_eq!(r.cycles[0].period_start_weekday, "Wednesday");
        assert_eq!(r.cycles[0].period_end, "2026-08-02"); // 29 + 4
        assert_eq!(r.cycles[0].ovulation_date, "2026-07-15"); // 29 - 14
        assert_eq!(r.cycles[0].fertile_window_start, "2026-07-10");
        assert_eq!(r.cycles[0].fertile_window_end, "2026-07-15");
        assert_eq!(r.cycles[1].period_start, "2026-08-26");
        assert_eq!(r.cycles[2].period_start, "2026-09-23");
    }

    #[test]
    fn crosses_month_and_year_boundaries() {
        let r = predict(d(2026, 12, 20), 30, 6, 14, 2).unwrap();
        assert_eq!(r.cycles[0].period_start, "2027-01-19");
        assert_eq!(r.cycles[0].period_end, "2027-01-24"); // 19 + 5
        assert_eq!(r.cycles[1].period_start, "2027-02-18");
    }

    #[test]
    fn short_cycle_ovulation_tracks_luteal_phase() {
        let r = predict(d(2026, 3, 1), 24, 4, 12, 1).unwrap();
        assert_eq!(r.cycles[0].period_start, "2026-03-25");
        assert_eq!(r.cycles[0].ovulation_date, "2026-03-13"); // 25 - 12
    }

    #[test]
    fn flexible_date_parsing() {
        let a = period_predict("2026-07-01", 28, 5, 14, 1).unwrap();
        let b = period_predict("07/01/2026", 28, 5, 14, 1).unwrap();
        let c = period_predict("2026-07-01T09:15:00Z", 28, 5, 14, 1).unwrap();
        assert_eq!(a.next_period_start, "2026-07-29");
        assert_eq!(b.next_period_start, "2026-07-29");
        assert_eq!(c.next_period_start, "2026-07-29");
    }

    #[test]
    fn bad_date_errors() {
        assert!(period_predict("not-a-date", 28, 5, 14, 3).is_err());
        assert!(period_predict("", 28, 5, 14, 3).is_err());
    }

    #[test]
    fn out_of_range_numeric_errors() {
        assert!(predict(d(2026, 7, 1), 10, 5, 14, 3).is_err()); // cycle too short
        assert!(predict(d(2026, 7, 1), 60, 5, 14, 3).is_err()); // cycle too long
        assert!(predict(d(2026, 7, 1), 28, 0, 14, 3).is_err()); // period too short
        assert!(predict(d(2026, 7, 1), 28, 5, 14, 0).is_err()); // cycles too few
        assert!(predict(d(2026, 7, 1), 28, 5, 14, 25).is_err()); // cycles too many
    }


    #[test]
    fn boundary_values_are_accepted() {
        assert!(predict(d(2026, 7, 1), CYCLE_MIN, PERIOD_MIN, LUTEAL_MIN, CYCLES_MIN).is_ok());
        assert!(predict(d(2026, 7, 1), CYCLE_MAX, PERIOD_MAX, LUTEAL_MAX, CYCLES_MAX).is_ok());
    }

    #[test]
    fn json_is_pretty_and_parses() {
        let s = period_predict_json("2026-07-01", 28, 5, 14, 6).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["next_period_start"], "2026-07-29");
        assert_eq!(v["cycles"].as_array().unwrap().len(), 6);
        assert_eq!(v["cycles"][5]["period_start"], "2026-12-16");
    }
}
