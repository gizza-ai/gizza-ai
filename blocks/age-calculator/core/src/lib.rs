//! age-calculator core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Given a birthdate and a target ("as of") date, computes the exact age as a
//! calendar breakdown (years, months, days) that respects variable month lengths
//! and leap years, plus flat totals (total months / weeks / days / hours), the
//! weekday the person was born on, the countdown to their next birthday, and the
//! Western (tropical) zodiac sign. All math is in naive civil dates — the inputs
//! are interpreted as-is, no time zones.

use chrono::{Datelike, NaiveDate, NaiveDateTime};
use serde::Serialize;

/// Structured age result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Age {
    /// The birthdate, normalized to `YYYY-MM-DD`.
    pub birthdate: String,
    /// The target / "as of" date, normalized to `YYYY-MM-DD`.
    pub as_of: String,
    /// Calendar breakdown — completed years.
    pub years: i64,
    /// Calendar breakdown — completed months past the last whole year.
    pub months: i64,
    /// Calendar breakdown — completed days past the last whole month.
    pub days: i64,
    /// Flat total — whole months lived.
    pub total_months: i64,
    /// Flat total — whole weeks lived.
    pub total_weeks: i64,
    /// Flat total — whole days lived.
    pub total_days: i64,
    /// Flat total — whole hours lived (days × 24).
    pub total_hours: i64,
    /// Flat total — whole minutes lived (days × 1440).
    pub total_minutes: i64,
    /// Flat total — whole seconds lived (days × 86400).
    pub total_seconds: i64,
    /// Weekday the person was born on (e.g. "Tuesday").
    pub day_of_week_born: String,
    /// The date of the next birthday relative to `as_of` (`YYYY-MM-DD`).
    pub next_birthday: String,
    /// Whole days from `as_of` until the next birthday. 0 means the birthday is
    /// today.
    pub days_until_next_birthday: i64,
    /// Western (tropical) zodiac sign for the birthdate.
    pub zodiac_sign: String,
    /// Chinese zodiac animal for the birth year.
    pub chinese_zodiac: String,
    /// Cultural generation label for the birth year (e.g. "Millennial").
    pub generation: String,
    /// Human-readable one-line summary of the calendar breakdown.
    pub summary: String,
}

/// Parse a flexible date or datetime string into a naive date (time is dropped).
///
/// Accepts (in order):
/// - RFC-3339 with offset/Z (`2024-01-02T03:04:05Z`, `...+02:00`)
/// - `YYYY-MM-DDTHH:MM:SS` / `YYYY-MM-DD HH:MM:SS`
/// - `YYYY-MM-DDTHH:MM` / `YYYY-MM-DD HH:MM`
/// - `YYYY-MM-DD`, `YYYY/MM/DD`, `MM/DD/YYYY`, `DD.MM.YYYY`
fn parse_date(s: &str) -> Result<NaiveDate, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err("empty date".into());
    }

    // RFC-3339 / ISO-8601 with timezone: keep the wall-clock date, drop offset.
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(t) {
        return Ok(dt.naive_local().date());
    }

    // Datetime forms — keep the date part.
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

    // Date-only forms.
    for fmt in ["%Y-%m-%d", "%Y/%m/%d", "%m/%d/%Y", "%d.%m.%Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(t, fmt) {
            return Ok(d);
        }
    }

    Err(format!(
        "could not parse date `{t}` — try YYYY-MM-DD (or RFC-3339, YYYY/MM/DD, MM/DD/YYYY, DD.MM.YYYY)"
    ))
}

/// Western (tropical) zodiac sign for a given month/day.
fn zodiac(month: u32, day: u32) -> &'static str {
    match (month, day) {
        (3, 21..=31) | (4, 1..=19) => "Aries",
        (4, 20..=30) | (5, 1..=20) => "Taurus",
        (5, 21..=31) | (6, 1..=20) => "Gemini",
        (6, 21..=30) | (7, 1..=22) => "Cancer",
        (7, 23..=31) | (8, 1..=22) => "Leo",
        (8, 23..=31) | (9, 1..=22) => "Virgo",
        (9, 23..=30) | (10, 1..=22) => "Libra",
        (10, 23..=31) | (11, 1..=21) => "Scorpio",
        (11, 22..=30) | (12, 1..=21) => "Sagittarius",
        (12, 22..=31) | (1, 1..=19) => "Capricorn",
        (1, 20..=31) | (2, 1..=18) => "Aquarius",
        _ => "Pisces", // Feb 19 – Mar 20
    }
}

/// Chinese zodiac animal for a year. The 12-animal cycle is anchored so that
/// 2020 = Rat (the start of the cycle); uses Gregorian-year approximation (the
/// animal actually changes at Chinese New Year in late Jan/Feb, but the
/// year-based mapping is the convention used by mainstream age calculators).
fn chinese_zodiac(year: i32) -> &'static str {
    const ANIMALS: [&str; 12] = [
        "Rat", "Ox", "Tiger", "Rabbit", "Dragon", "Snake", "Horse", "Goat", "Monkey", "Rooster",
        "Dog", "Pig",
    ];
    let idx = (year - 2020).rem_euclid(12) as usize;
    ANIMALS[idx]
}

/// Cultural generation label for a birth year (common Western groupings).
fn generation(year: i32) -> &'static str {
    match year {
        i32::MIN..=1927 => "Greatest Generation",
        1928..=1945 => "Silent Generation",
        1946..=1964 => "Baby Boomer",
        1965..=1980 => "Generation X",
        1981..=1996 => "Millennial",
        1997..=2012 => "Generation Z",
        _ => "Generation Alpha",
    }
}

/// Number of days in a given month.
fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_next = NaiveDate::from_ymd_opt(ny, nm, 1).unwrap();
    let first_this = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    (first_next - first_this).num_days() as u32
}

/// Add `months` calendar months to `d`, clamping the day to the target month's
/// length (e.g. Jan 31 + 1 month = Feb 28/29).
fn add_months(d: NaiveDate, months: i64) -> NaiveDate {
    let total = (d.year() as i64) * 12 + (d.month0() as i64) + months;
    let year = total.div_euclid(12) as i32;
    let month0 = total.rem_euclid(12) as u32;
    let month = month0 + 1;
    let dim = days_in_month(year, month);
    NaiveDate::from_ymd_opt(year, month, d.day().min(dim)).expect("valid clamped date")
}

/// Compute the age of someone born on `birth` as of the date `as_of`.
///
/// `as_of` must not be earlier than `birth`; an earlier `as_of` is an error
/// (you cannot have a negative age).
pub fn age_at(birth: NaiveDate, as_of: NaiveDate) -> Result<Age, String> {
    if as_of < birth {
        return Err(format!(
            "the target date ({as_of}) is before the birthdate ({birth}) — age cannot be negative"
        ));
    }

    // Calendar breakdown: count whole years, then whole months, then days.
    let mut years = (as_of.year() - birth.year()) as i64;
    if (as_of.month(), as_of.day()) < (birth.month(), birth.day()) {
        years -= 1;
    }
    let after_years = add_months(birth, years * 12);

    let mut months = 0i64;
    while add_months(after_years, months + 1) <= as_of {
        months += 1;
    }
    let after_months = add_months(after_years, months);
    let days = (as_of - after_months).num_days();

    // Flat totals.
    let total_days = (as_of - birth).num_days();
    let total_weeks = total_days / 7;
    let total_hours = total_days * 24;
    let total_minutes = total_days * 24 * 60;
    let total_seconds = total_days * 24 * 60 * 60;
    let total_months = years * 12 + months;

    // Next birthday relative to as_of.
    let next_bday = next_birthday_on_or_after(birth, as_of);
    let days_until = (next_bday - as_of).num_days();

    let summary = format!(
        "{years} year{} {months} month{} {days} day{} old",
        plural(years),
        plural(months),
        plural(days)
    );

    Ok(Age {
        birthdate: birth.to_string(),
        as_of: as_of.to_string(),
        years,
        months,
        days,
        total_months,
        total_weeks,
        total_days,
        total_hours,
        total_minutes,
        total_seconds,
        day_of_week_born: birth.format("%A").to_string(),
        next_birthday: next_bday.to_string(),
        days_until_next_birthday: days_until,
        zodiac_sign: zodiac(birth.month(), birth.day()).to_string(),
        chinese_zodiac: chinese_zodiac(birth.year()).to_string(),
        generation: generation(birth.year()).to_string(),
        summary,
    })
}

/// The next occurrence of the birth month/day on or after `as_of`. Handles
/// Feb-29 birthdays by falling back to Feb-28 in non-leap years.
fn next_birthday_on_or_after(birth: NaiveDate, as_of: NaiveDate) -> NaiveDate {
    for year in [as_of.year(), as_of.year() + 1] {
        let dim = days_in_month(year, birth.month());
        let cand = NaiveDate::from_ymd_opt(year, birth.month(), birth.day().min(dim)).unwrap();
        if cand >= as_of {
            return cand;
        }
    }
    add_months(birth, ((as_of.year() - birth.year()) as i64 + 1) * 12)
}

fn plural(n: i64) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// Convenience: parse string inputs and compute the age. `as_of` falls back to
/// `default_today` (the caller's resolved "today") when the string is empty.
pub fn age(birth: &str, as_of: &str, default_today: NaiveDate) -> Result<Age, String> {
    let b = parse_date(birth)?;
    let a = if as_of.trim().is_empty() {
        default_today
    } else {
        parse_date(as_of)?
    };
    age_at(b, a)
}

/// Same as [`age`] but returns a pretty-printed JSON string. Used by the web page.
pub fn age_json(birth: &str, as_of: &str, default_today: NaiveDate) -> Result<String, String> {
    let r = age(birth, as_of, default_today)?;
    serde_json::to_string_pretty(&r).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn simple_age_whole_years() {
        let r = age_at(d(2000, 6, 22), d(2026, 6, 22)).unwrap();
        assert_eq!(r.years, 26);
        assert_eq!(r.months, 0);
        assert_eq!(r.days, 0);
        assert_eq!(r.days_until_next_birthday, 0);
        assert_eq!(r.next_birthday, "2026-06-22");
    }

    #[test]
    fn breakdown_years_months_days() {
        let r = age_at(d(1990, 1, 15), d(2026, 6, 22)).unwrap();
        assert_eq!(r.years, 36);
        assert_eq!(r.months, 5);
        assert_eq!(r.days, 7);
        assert_eq!(r.summary, "36 years 5 months 7 days old");
    }

    #[test]
    fn day_before_birthday_is_not_a_full_year() {
        let r = age_at(d(2000, 6, 22), d(2026, 6, 21)).unwrap();
        assert_eq!(r.years, 25);
        assert_eq!(r.months, 11);
        assert_eq!(r.days_until_next_birthday, 1);
        assert_eq!(r.next_birthday, "2026-06-22");
    }

    #[test]
    fn jan31_plus_one_month_clamps_to_feb() {
        let r = age_at(d(2025, 1, 31), d(2025, 2, 28)).unwrap();
        assert_eq!(r.years, 0);
        assert_eq!(r.months, 1);
        assert_eq!(r.days, 0);
    }

    #[test]
    fn leap_day_birthday_in_common_year_uses_feb28() {
        let r = age_at(d(2000, 2, 29), d(2025, 3, 1)).unwrap();
        assert_eq!(r.years, 25);
        assert_eq!(r.next_birthday, "2026-02-28");
    }

    #[test]
    fn totals_match() {
        let r = age_at(d(2026, 6, 1), d(2026, 6, 22)).unwrap();
        assert_eq!(r.total_days, 21);
        assert_eq!(r.total_weeks, 3);
        assert_eq!(r.total_hours, 21 * 24);
        assert_eq!(r.total_minutes, 21 * 24 * 60);
        assert_eq!(r.total_seconds, 21 * 24 * 60 * 60);
        assert_eq!(r.total_months, 0);
    }

    #[test]
    fn day_of_week_and_zodiac() {
        let r = age_at(d(2000, 6, 22), d(2026, 6, 22)).unwrap();
        assert_eq!(r.day_of_week_born, "Thursday");
        assert_eq!(r.zodiac_sign, "Cancer");
        // 2000 = Dragon; born 2000 → Generation Z (1997–2012).
        assert_eq!(r.chinese_zodiac, "Dragon");
        assert_eq!(r.generation, "Generation Z");
    }

    #[test]
    fn chinese_zodiac_and_generation_cycle() {
        assert_eq!(chinese_zodiac(2020), "Rat");
        assert_eq!(chinese_zodiac(2021), "Ox");
        assert_eq!(chinese_zodiac(2008), "Rat");
        assert_eq!(chinese_zodiac(2019), "Pig");
        assert_eq!(generation(1990), "Millennial");
        assert_eq!(generation(2000), "Generation Z");
        assert_eq!(generation(2015), "Generation Alpha");
        assert_eq!(generation(1970), "Generation X");
        assert_eq!(generation(1950), "Baby Boomer");
    }

    #[test]
    fn zodiac_boundaries() {
        assert_eq!(zodiac(3, 21), "Aries");
        assert_eq!(zodiac(3, 20), "Pisces");
        assert_eq!(zodiac(12, 22), "Capricorn");
        assert_eq!(zodiac(1, 19), "Capricorn");
        assert_eq!(zodiac(1, 20), "Aquarius");
    }

    #[test]
    fn future_birthdate_is_error() {
        assert!(age_at(d(2030, 1, 1), d(2026, 6, 22)).is_err());
    }

    #[test]
    fn flexible_parsing_and_default_today() {
        let today = d(2026, 6, 22);
        let r = age("2000-06-22", "", today).unwrap();
        assert_eq!(r.years, 26);
        let r2 = age("06/22/2000", "2026-06-22", today).unwrap();
        assert_eq!(r2.years, 26);
        let r3 = age("2000-06-22T00:00:00Z", "", today).unwrap();
        assert_eq!(r3.years, 26);
    }

    #[test]
    fn bad_input_errors() {
        let today = d(2026, 6, 22);
        assert!(age("not-a-date", "", today).is_err());
        assert!(age("", "", today).is_err());
    }

    #[test]
    fn json_is_pretty_and_parses() {
        let s = age_json("2000-06-22", "2026-06-22", d(2026, 6, 22)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["years"], 26);
        assert_eq!(v["zodiac_sign"], "Cancer");
    }
}
