//! meeting-time-finder core — find the meeting slots on a given date that fall
//! inside every participant's working hours, ranked fairness-first.
//!
//! Pure Rust (`chrono` + `chrono-tz`), no clock and no I/O: the caller always
//! supplies the date, so the same inputs always produce the same answer on
//! every surface (chat block, CLI, browser page).
//!
//! Model: each participant is an IANA timezone plus a local working window
//! (09:00–17:00 unless overridden). Candidate meeting starts are enumerated
//! across the 24 hours of `date` in the display zone, stepping by
//! `granularity_minutes`. Every candidate is scored per participant from how
//! much of the meeting lands inside their window (`fit`) and how close the
//! meeting's midpoint sits to the middle of that window (`comfort`), then
//! ranked so the WORST-off participant is as well off as possible.

use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDate, Offset, TimeZone, Timelike};
use chrono_tz::Tz;
use serde_json::json;

/// Hard cap on participants — well above every comparable planner (5–6) while
/// keeping the rendered tables readable.
pub const MAX_PARTICIPANTS: usize = 12;
/// Shortest meeting accepted, in minutes.
pub const MIN_DURATION_MINUTES: i64 = 5;
/// Longest meeting accepted, in minutes.
pub const MAX_DURATION_MINUTES: i64 = 480;
/// Most ranked slots that can be returned.
pub const MAX_RESULTS_CAP: i64 = 24;

const STATUS_IN: &str = "in hours";
const STATUS_PARTIAL: &str = "partial";
const STATUS_OUTSIDE: &str = "outside";
const STATUS_WEEKEND: &str = "weekend";

/// One participant: a zone, a display name and a local working window.
#[derive(Debug, Clone)]
struct Participant {
    name: String,
    zone: Tz,
    /// Working-day start, minutes from local midnight (0..1440).
    work_start: i64,
    /// Working-day end, minutes from local midnight (1..=1440). When it is
    /// less than or equal to `work_start` the window wraps past midnight
    /// (night shift).
    work_end: i64,
}

impl Participant {
    /// Window length in minutes (always 1..=1440).
    fn window_len(&self) -> i64 {
        if self.work_end > self.work_start {
            self.work_end - self.work_start
        } else {
            self.work_end + 1440 - self.work_start
        }
    }
}

/// How one participant fares for one candidate slot.
#[derive(Debug, Clone)]
struct SlotParticipant {
    name: String,
    zone: String,
    local_start: String,
    local_end: String,
    /// `Thu` or `Thu→Fri` when the meeting crosses local midnight.
    day: String,
    weekday: String,
    local_date: String,
    status: &'static str,
    score: i64,
}

/// One ranked candidate slot.
#[derive(Debug, Clone)]
struct Slot {
    start_display: String,
    end_display: String,
    display_day: String,
    start_utc: String,
    end_utc: String,
    all_available: bool,
    worst: i64,
    average: i64,
    participants: Vec<SlotParticipant>,
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Parse a local time-of-day into minutes from midnight.
///
/// Accepts `9`, `9:30`, `09:00`, `9.30`, `9am`, `9 AM`, `5pm`, `17:00` and
/// `24:00` (end of day).
fn parse_time_of_day(raw: &str, what: &str) -> Result<i64, String> {
    let t = raw.trim().to_ascii_lowercase();
    if t.is_empty() {
        return Err(format!("{what} is empty; use a local time like '09:00' or '9am'"));
    }
    let (body, pm) = if let Some(b) = t.strip_suffix("am") {
        (b.trim().to_string(), Some(false))
    } else if let Some(b) = t.strip_suffix("pm") {
        (b.trim().to_string(), Some(true))
    } else {
        (t.clone(), None)
    };
    let body = body.replace('.', ":");
    let (h_str, m_str) = match body.split_once(':') {
        Some((h, m)) => (h.trim(), m.trim()),
        None => (body.trim(), "0"),
    };
    let mut hour: i64 = h_str
        .parse()
        .map_err(|_| format!("{what} {raw:?} is not a time; use '09:00', '9', '9am' or '17:30'"))?;
    let minute: i64 = m_str
        .parse()
        .map_err(|_| format!("{what} {raw:?} has an unreadable minute part; use '09:30' or '9:30pm'"))?;
    if !(0..60).contains(&minute) {
        return Err(format!("{what} {raw:?} has minute {minute}; minutes must be 0-59"));
    }
    match pm {
        Some(is_pm) => {
            if !(1..=12).contains(&hour) {
                return Err(format!(
                    "{what} {raw:?} uses AM/PM, so the hour must be 1-12 (got {hour})"
                ));
            }
            if is_pm && hour < 12 {
                hour += 12;
            }
            if !is_pm && hour == 12 {
                hour = 0;
            }
        }
        None => {
            if !(0..=24).contains(&hour) {
                return Err(format!("{what} {raw:?} has hour {hour}; hours must be 0-24"));
            }
        }
    }
    let total = hour * 60 + minute;
    if total > 1440 {
        return Err(format!("{what} {raw:?} is past 24:00"));
    }
    Ok(total)
}

/// Resolve an IANA timezone name (`Europe/London`, `UTC`, …).
fn parse_zone(name: &str, what: &str) -> Result<Tz, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err(format!("{what} is empty; use an IANA name like 'Europe/London'"));
    }
    n.parse::<Tz>().map_err(|_| {
        format!(
            "unknown {what} {n:?}; use an IANA timezone name like 'America/New_York', \
             'Europe/London', 'Asia/Tokyo', 'Australia/Sydney' or 'UTC'"
        )
    })
}

/// Default display name for a zone: `America/New_York` → `New York`.
fn zone_label(zone: &Tz) -> String {
    zone.name()
        .rsplit('/')
        .next()
        .unwrap_or(zone.name())
        .replace('_', " ")
}

/// Parse the `participants` list: `Name@Zone:start-end` entries, comma separated.
/// Only the zone is mandatory.
fn parse_participants(
    raw: &str,
    default_start: i64,
    default_end: i64,
) -> Result<Vec<Participant>, String> {
    let entries: Vec<&str> = raw.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    if entries.is_empty() {
        return Err(
            "participants is empty; give a comma-separated list like \
             'Alice@Europe/London, Bob@America/New_York, Asia/Tokyo'"
                .to_string(),
        );
    }
    if entries.len() > MAX_PARTICIPANTS {
        return Err(format!(
            "too many participants: {} (max {MAX_PARTICIPANTS})",
            entries.len()
        ));
    }
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let (name_part, rest) = match entry.split_once('@') {
            Some((n, r)) => (Some(n.trim().to_string()), r.trim()),
            None => (None, entry),
        };
        let (zone_part, hours_part) = match rest.split_once(':') {
            Some((z, h)) => (z.trim(), Some(h.trim())),
            None => (rest, None),
        };
        let zone = parse_zone(zone_part, &format!("timezone in participant {entry:?}"))?;
        let (work_start, work_end) = match hours_part {
            None => (default_start, default_end),
            Some(h) => {
                let (s, e) = h.split_once('-').ok_or_else(|| {
                    format!(
                        "working hours {h:?} for {entry:?} need a start and an end, \
                         e.g. '{}:9-17' or '{}:08:30-16:30'",
                        zone.name(),
                        zone.name()
                    )
                })?;
                (
                    parse_time_of_day(s, &format!("working-hours start for {entry:?}"))?,
                    parse_time_of_day(e, &format!("working-hours end for {entry:?}"))?,
                )
            }
        };
        if work_start == work_end {
            return Err(format!(
                "working hours for {entry:?} start and end at the same time ({}), \
                 so the working day is empty",
                fmt_minutes(work_start, false)
            ));
        }
        let name = match name_part {
            Some(n) if !n.is_empty() => n,
            _ => zone_label(&zone),
        };
        out.push(Participant {
            name,
            zone,
            work_start: work_start % 1440,
            work_end: if work_end == 0 { 1440 } else { work_end },
        });
    }
    Ok(out)
}

/// Parse `YYYY-MM-DD` (also accepts `YYYY/MM/DD`).
fn parse_date(raw: &str) -> Result<NaiveDate, String> {
    let t = raw.trim().replace('/', "-");
    if t.is_empty() {
        return Err("date is empty; use an ISO date like '2026-10-01'".to_string());
    }
    NaiveDate::parse_from_str(&t, "%Y-%m-%d")
        .map_err(|_| format!("could not parse date {raw:?}; use an ISO date like '2026-10-01'"))
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// Render minutes-from-midnight as `15:00` (24-hour) or `3:00 PM` (12-hour).
fn fmt_minutes(minutes: i64, twelve_hour: bool) -> String {
    let m = ((minutes % 1440) + 1440) % 1440;
    let (h, min) = (m / 60, m % 60);
    if twelve_hour {
        let suffix = if h < 12 { "AM" } else { "PM" };
        let h12 = match h % 12 {
            0 => 12,
            other => other,
        };
        format!("{h12}:{min:02} {suffix}")
    } else {
        format!("{h:02}:{min:02}")
    }
}

fn fmt_offset(seconds: i32) -> String {
    let sign = if seconds < 0 { '-' } else { '+' };
    let abs = seconds.abs();
    format!("{sign}{:02}:{:02}", abs / 3600, (abs % 3600) / 60)
}

fn pad(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - len))
    }
}

fn pad_left(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        s.to_string()
    } else {
        format!("{}{s}", " ".repeat(width - len))
    }
}

/// Overlap in minutes between `[a0, a1)` and `[b0, b1)`.
fn overlap(a0: i64, a1: i64, b0: i64, b1: i64) -> i64 {
    (a1.min(b1) - a0.max(b0)).max(0)
}

// ---------------------------------------------------------------------------
// The finder
// ---------------------------------------------------------------------------

/// Find and render the best meeting slots.
///
/// * `participants` — comma-separated `Name@Zone:start-end` entries (zone only is fine).
/// * `date` — the calendar date to search, in the display zone.
/// * `work_start` / `work_end` — default working window for participants without an override.
/// * `duration_minutes` — meeting length (5–480).
/// * `granularity_minutes` — candidate step: `15`, `30` or `60`.
/// * `max_results` — how many ranked slots to return (1–24).
/// * `clock` — `24h` or `12h` rendering.
/// * `display_zone` — zone for the ranked column; empty = the first participant's zone.
/// * `skip_weekends` — treat a participant's local Saturday/Sunday as non-working.
/// * `allow_partial` — when no slot suits everyone, still rank the closest compromises.
/// * `output_format` — `summary`, `timeline`, `table`, `json`, `csv` or `ics`.
#[allow(clippy::too_many_arguments)]
pub fn find(
    participants: &str,
    date: &str,
    work_start: &str,
    work_end: &str,
    duration_minutes: f64,
    granularity_minutes: &str,
    max_results: f64,
    clock: &str,
    display_zone: &str,
    skip_weekends: bool,
    allow_partial: bool,
    output_format: &str,
) -> Result<String, String> {
    // --- validate scalars -------------------------------------------------
    if !duration_minutes.is_finite() {
        return Err("duration_minutes must be a number".to_string());
    }
    let duration = duration_minutes.round() as i64;
    if !(MIN_DURATION_MINUTES..=MAX_DURATION_MINUTES).contains(&duration) {
        return Err(format!(
            "duration_minutes must be between {MIN_DURATION_MINUTES} and {MAX_DURATION_MINUTES} (got {duration})"
        ));
    }
    let step = match granularity_minutes.trim() {
        "15" => 15,
        "30" => 30,
        "60" | "" => 60,
        other => {
            return Err(format!(
                "granularity_minutes must be 15, 30 or 60 (got {other:?})"
            ))
        }
    };
    if !max_results.is_finite() {
        return Err("max_results must be a number".to_string());
    }
    let wanted = max_results.round() as i64;
    if !(1..=MAX_RESULTS_CAP).contains(&wanted) {
        return Err(format!(
            "max_results must be between 1 and {MAX_RESULTS_CAP} (got {wanted})"
        ));
    }
    let twelve_hour = match clock.trim().to_ascii_lowercase().as_str() {
        "24h" | "24" | "" => false,
        "12h" | "12" => true,
        other => return Err(format!("clock must be '24h' or '12h' (got {other:?})")),
    };
    let format = output_format.trim().to_ascii_lowercase();
    let format = if format.is_empty() { "summary".to_string() } else { format };
    if !matches!(
        format.as_str(),
        "summary" | "timeline" | "table" | "json" | "csv" | "ics"
    ) {
        return Err(format!(
            "output_format must be summary, timeline, table, json, csv or ics (got {output_format:?})"
        ));
    }

    let default_start = parse_time_of_day(work_start, "work_start")?;
    let default_end = parse_time_of_day(work_end, "work_end")?;
    if default_start == default_end {
        return Err(format!(
            "work_start and work_end are both {}, so the default working day is empty",
            fmt_minutes(default_start, twelve_hour)
        ));
    }
    let people = parse_participants(participants, default_start, default_end)?;
    let day = parse_date(date)?;
    let display_tz = if display_zone.trim().is_empty() {
        people[0].zone
    } else {
        parse_zone(display_zone, "display_zone")?
    };

    // --- anchor: local midnight of `date` in the display zone -------------
    let mut anchor: Option<DateTime<Tz>> = None;
    for extra_hour in 0..4 {
        let naive = day
            .and_hms_opt(extra_hour, 0, 0)
            .ok_or_else(|| "internal: invalid hour".to_string())?;
        anchor = match display_tz.from_local_datetime(&naive) {
            LocalResult::Single(dt) => Some(dt),
            LocalResult::Ambiguous(earlier, _later) => Some(earlier),
            LocalResult::None => None,
        };
        if anchor.is_some() {
            break;
        }
    }
    let anchor = anchor.ok_or_else(|| {
        format!("{date} has no valid start-of-day in {} (daylight-saving gap)", display_tz.name())
    })?;

    // --- DST notes --------------------------------------------------------
    let mut dst_notes: Vec<String> = Vec::new();
    let mut seen_zones: Vec<String> = Vec::new();
    for p in &people {
        if seen_zones.iter().any(|z| z == p.zone.name()) {
            continue;
        }
        seen_zones.push(p.zone.name().to_string());
        let start_off = anchor.with_timezone(&p.zone).offset().fix().local_minus_utc();
        let end_off = (anchor + Duration::minutes(1439))
            .with_timezone(&p.zone)
            .offset()
            .fix()
            .local_minus_utc();
        if start_off != end_off {
            dst_notes.push(format!(
                "{} changes its UTC offset on this date ({} -> {}) — a daylight-saving transition",
                p.zone.name(),
                fmt_offset(start_off),
                fmt_offset(end_off)
            ));
        }
    }

    // --- score every candidate -------------------------------------------
    let slot_count = 1440 / step;
    let mut slots: Vec<Slot> = Vec::with_capacity(slot_count as usize);
    for k in 0..slot_count {
        let start = anchor + Duration::minutes(k * step);
        slots.push(build_slot(start, duration, &people, skip_weekends, twelve_hour, display_tz));
    }

    let any_all_available = slots.iter().any(|s| s.all_available);
    let mut ranked: Vec<Slot> = slots.clone();
    ranked.sort_by(|a, b| {
        b.all_available
            .cmp(&a.all_available)
            .then(b.worst.cmp(&a.worst))
            .then(b.average.cmp(&a.average))
            .then(a.start_utc.cmp(&b.start_utc))
    });
    if !allow_partial {
        ranked.retain(|s| s.all_available);
    }
    ranked.truncate(wanted as usize);

    let ctx = Report {
        date: day,
        display_zone: display_tz.name().to_string(),
        duration,
        step,
        default_start,
        default_end,
        twelve_hour,
        skip_weekends,
        allow_partial,
        any_all_available,
        people: people.clone(),
        dst_notes,
        ranked,
        hourly: slots,
    };

    Ok(match format.as_str() {
        "summary" => render_summary(&ctx),
        "timeline" => render_timeline(&ctx),
        "table" => render_table(&ctx),
        "json" => render_json(&ctx),
        "csv" => render_csv(&ctx),
        _ => render_ics(&ctx),
    })
}

/// Everything the renderers need.
struct Report {
    date: NaiveDate,
    display_zone: String,
    duration: i64,
    step: i64,
    default_start: i64,
    default_end: i64,
    twelve_hour: bool,
    skip_weekends: bool,
    allow_partial: bool,
    any_all_available: bool,
    people: Vec<Participant>,
    dst_notes: Vec<String>,
    ranked: Vec<Slot>,
    /// Every candidate in chronological order (used by the timeline view).
    hourly: Vec<Slot>,
}

fn build_slot(
    start: DateTime<Tz>,
    duration: i64,
    people: &[Participant],
    skip_weekends: bool,
    twelve_hour: bool,
    display_tz: Tz,
) -> Slot {
    let end = start + Duration::minutes(duration);
    let disp_start = start.with_timezone(&display_tz);
    let disp_end = end.with_timezone(&display_tz);
    let mut parts = Vec::with_capacity(people.len());
    let mut worst = 100i64;
    let mut total = 0i64;
    let mut all_available = true;
    for p in people {
        let local_start = start.with_timezone(&p.zone);
        let local_end = end.with_timezone(&p.zone);
        let m = local_start.hour() as i64 * 60 + local_start.minute() as i64;
        let weekend = matches!(
            local_start.weekday(),
            chrono::Weekday::Sat | chrono::Weekday::Sun
        );
        let win_len = p.window_len();
        let (status, score) = if skip_weekends && weekend {
            (STATUS_WEEKEND, 0)
        } else {
            // Circular overlap: shift the meeting by ±1 day so a window or a
            // meeting that wraps past midnight still measures correctly.
            let mut covered = 0;
            for shift in [-1440, 0, 1440] {
                covered += overlap(
                    m + shift,
                    m + shift + duration,
                    p.work_start,
                    p.work_start + win_len,
                );
            }
            let fit = covered as f64 / duration as f64;
            let centre = p.work_start + win_len / 2;
            let midpoint = m + duration / 2;
            let raw = (((midpoint - centre) % 1440) + 1440) % 1440;
            let dist = raw.min(1440 - raw) as f64;
            let comfort = 1.0 - (dist / (win_len as f64 / 2.0 + 180.0)).min(1.0);
            let score = (100.0 * (0.7 * fit + 0.3 * comfort)).round() as i64;
            let status = if fit >= 1.0 {
                STATUS_IN
            } else if fit > 0.0 {
                STATUS_PARTIAL
            } else {
                STATUS_OUTSIDE
            };
            (status, score)
        };
        if status != STATUS_IN {
            all_available = false;
        }
        worst = worst.min(score);
        total += score;
        let day = if local_end.date_naive() != local_start.date_naive() {
            format!(
                "{}→{}",
                local_start.format("%a"),
                local_end.format("%a")
            )
        } else {
            local_start.format("%a").to_string()
        };
        parts.push(SlotParticipant {
            name: p.name.clone(),
            zone: p.zone.name().to_string(),
            local_start: fmt_minutes(m, twelve_hour),
            local_end: fmt_minutes(
                local_end.hour() as i64 * 60 + local_end.minute() as i64,
                twelve_hour,
            ),
            day,
            weekday: local_start.format("%A").to_string(),
            local_date: local_start.format("%Y-%m-%d").to_string(),
            status,
            score,
        });
    }
    let average = if people.is_empty() {
        0
    } else {
        (total as f64 / people.len() as f64).round() as i64
    };
    Slot {
        start_display: fmt_minutes(
            disp_start.hour() as i64 * 60 + disp_start.minute() as i64,
            twelve_hour,
        ),
        end_display: fmt_minutes(
            disp_end.hour() as i64 * 60 + disp_end.minute() as i64,
            twelve_hour,
        ),
        display_day: disp_start.format("%a %d %b").to_string(),
        start_utc: start
            .with_timezone(&chrono::Utc)
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string(),
        end_utc: end
            .with_timezone(&chrono::Utc)
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string(),
        all_available,
        worst,
        average,
        participants: parts,
    }
}

// ---------------------------------------------------------------------------
// Renderers
// ---------------------------------------------------------------------------

fn header_lines(r: &Report) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Meeting time finder — {} · slots shown in {}",
            r.date.format("%a %d %b %Y"),
            r.display_zone
        ),
        format!(
            "{} participant{} · {}-minute meeting · {}-minute steps · default hours {}-{} local · weekends {}",
            r.people.len(),
            if r.people.len() == 1 { "" } else { "s" },
            r.duration,
            r.step,
            fmt_minutes(r.default_start, r.twelve_hour),
            fmt_minutes(r.default_end, r.twelve_hour),
            if r.skip_weekends { "skipped" } else { "allowed" }
        ),
    ];
    for note in &r.dst_notes {
        lines.push(format!("DST: {note}"));
    }
    lines
}

fn render_summary(r: &Report) -> String {
    let mut out = header_lines(r);
    if r.ranked.is_empty() {
        out.push(String::new());
        out.push(format!(
            "No slot on {} works for everyone inside their working hours.",
            r.date.format("%a %d %b %Y")
        ));
        out.push(
            "Try widening work_start/work_end, another date, or set allow_partial to true to see the closest compromises."
                .to_string(),
        );
        return out.join("\n");
    }
    out.push(String::new());
    if r.any_all_available {
        let best = &r.ranked[0];
        out.push(format!(
            "Best: {}-{} {} ({}) — works for all {} · fairness {}/100 · average {}/100",
            best.start_display,
            best.end_display,
            r.display_zone,
            best.display_day,
            r.people.len(),
            best.worst,
            best.average
        ));
    } else {
        out.push(
            "No slot suits everyone inside their working hours — the closest compromises are ranked below."
                .to_string(),
        );
    }
    out.push(String::new());

    let name_w = r.people.iter().map(|p| p.name.chars().count()).max().unwrap_or(4);
    let zone_w = r
        .people
        .iter()
        .map(|p| p.zone.name().chars().count())
        .max()
        .unwrap_or(3);
    for (i, slot) in r.ranked.iter().enumerate() {
        out.push(format!(
            "{}. {}-{} {} ({}) — {} · fairness {} · average {}",
            i + 1,
            slot.start_display,
            slot.end_display,
            r.display_zone,
            slot.display_day,
            if slot.all_available {
                format!("all {} available", r.people.len())
            } else {
                let out_count = slot
                    .participants
                    .iter()
                    .filter(|p| p.status != STATUS_IN)
                    .count();
                format!(
                    "{} of {} available",
                    r.people.len() - out_count,
                    r.people.len()
                )
            },
            slot.worst,
            slot.average
        ));
        for p in &slot.participants {
            out.push(format!(
                "   {} {} {}-{} {} {} {}",
                pad(&p.name, name_w),
                pad(&p.zone, zone_w),
                p.local_start,
                p.local_end,
                pad(&p.day, 7),
                pad(p.status, 8),
                pad_left(&p.score.to_string(), 3)
            ));
        }
    }
    out.push(String::new());
    out.push(
        "Score 100 = the meeting sits centred inside that person's working day; 0 = fully outside it."
            .to_string(),
    );
    out.join("\n")
}

fn render_timeline(r: &Report) -> String {
    let mut out = header_lines(r);
    out.push(String::new());
    out.push(format!(
        "Hour-by-hour overlap (columns are {} hours, each cell is a {}-minute meeting starting then)",
        r.display_zone, r.duration
    ));
    let label_w = r
        .people
        .iter()
        .map(|p| p.name.chars().count() + p.zone.name().chars().count() + 2)
        .max()
        .unwrap_or(8)
        .max(8);
    // Hourly columns regardless of the candidate step, so the grid stays 24 wide.
    let per_hour: Vec<&Slot> = (0..24)
        .filter_map(|h| r.hourly.iter().find(|s| s.start_utc == hourly_key(r, h)))
        .collect();
    let mut head = pad("", label_w);
    for h in 0..24 {
        head.push_str(&format!(" {h:02}"));
    }
    out.push(head);
    for (idx, p) in r.people.iter().enumerate() {
        let mut row = pad(&format!("{}  {}", p.name, p.zone.name()), label_w);
        for slot in &per_hour {
            let sp = &slot.participants[idx];
            row.push_str(&format!("  {}", status_char(sp.status)));
        }
        out.push(row);
    }
    let mut everyone = pad("EVERYONE", label_w);
    for slot in &per_hour {
        let ch = if slot.all_available {
            '#'
        } else if slot
            .participants
            .iter()
            .all(|p| p.status == STATUS_IN || p.status == STATUS_PARTIAL)
        {
            '+'
        } else {
            '.'
        };
        everyone.push_str(&format!("  {ch}"));
    }
    out.push(everyone);
    out.push(String::new());
    out.push("Legend: # fully inside working hours · + partly inside · . outside · W weekend".to_string());
    if let Some(best) = r.ranked.first() {
        out.push(format!(
            "Best slot: {}-{} {} ({}) · fairness {}/100",
            best.start_display, best.end_display, r.display_zone, best.display_day, best.worst
        ));
    } else {
        out.push(format!(
            "No slot on {} works for everyone inside their working hours.",
            r.date.format("%a %d %b %Y")
        ));
    }
    out.join("\n")
}

/// UTC key of the candidate that starts `h` whole hours after the anchor.
fn hourly_key(r: &Report, h: i64) -> String {
    let per_hour = 60 / r.step;
    let idx = (h * per_hour) as usize;
    r.hourly
        .get(idx)
        .map(|s| s.start_utc.clone())
        .unwrap_or_default()
}

fn status_char(status: &str) -> char {
    match status {
        STATUS_IN => '#',
        STATUS_PARTIAL => '+',
        STATUS_WEEKEND => 'W',
        _ => '.',
    }
}

fn render_table(r: &Report) -> String {
    let mut out = header_lines(r);
    out.push(String::new());
    if r.ranked.is_empty() {
        out.push(format!(
            "No slot on {} works for everyone inside their working hours.",
            r.date.format("%a %d %b %Y")
        ));
        return out.join("\n");
    }
    let mut headers: Vec<String> = vec![
        "#".to_string(),
        format!("Start ({})", r.display_zone),
        "End".to_string(),
        "All".to_string(),
        "Fair".to_string(),
        "Avg".to_string(),
    ];
    for p in &r.people {
        headers.push(p.name.clone());
    }
    let mut rows: Vec<Vec<String>> = Vec::new();
    for (i, slot) in r.ranked.iter().enumerate() {
        let mut row = vec![
            (i + 1).to_string(),
            slot.start_display.clone(),
            slot.end_display.clone(),
            if slot.all_available { "yes" } else { "no" }.to_string(),
            slot.worst.to_string(),
            slot.average.to_string(),
        ];
        for p in &slot.participants {
            row.push(format!("{} {} ({})", p.local_start, p.status, p.score));
        }
        rows.push(row);
    }
    let widths: Vec<usize> = (0..headers.len())
        .map(|c| {
            rows.iter()
                .map(|r| r[c].chars().count())
                .chain(std::iter::once(headers[c].chars().count()))
                .max()
                .unwrap_or(1)
        })
        .collect();
    let line = |cells: &[String]| -> String {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| pad(c, widths[i]))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    out.push(line(&headers));
    for row in &rows {
        out.push(line(row));
    }
    out.join("\n")
}

fn render_json(r: &Report) -> String {
    let people: Vec<serde_json::Value> = r
        .people
        .iter()
        .map(|p| {
            json!({
                "name": p.name,
                "zone": p.zone.name(),
                "work_start": fmt_minutes(p.work_start, false),
                "work_end": fmt_minutes(p.work_end % 1440, false),
            })
        })
        .collect();
    let slots: Vec<serde_json::Value> = r
        .ranked
        .iter()
        .enumerate()
        .map(|(i, s)| {
            json!({
                "rank": i + 1,
                "start_utc": s.start_utc,
                "end_utc": s.end_utc,
                "start_display": s.start_display,
                "end_display": s.end_display,
                "display_day": s.display_day,
                "all_available": s.all_available,
                "fairness_score": s.worst,
                "average_score": s.average,
                "participants": s.participants.iter().map(|p| json!({
                    "name": p.name,
                    "zone": p.zone,
                    "local_start": p.local_start,
                    "local_end": p.local_end,
                    "local_date": p.local_date,
                    "weekday": p.weekday,
                    "status": p.status,
                    "score": p.score,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    serde_json::to_string_pretty(&json!({
        "date": r.date.format("%Y-%m-%d").to_string(),
        "display_zone": r.display_zone,
        "duration_minutes": r.duration,
        "granularity_minutes": r.step,
        "default_work_start": fmt_minutes(r.default_start, false),
        "default_work_end": fmt_minutes(r.default_end % 1440, false),
        "skip_weekends": r.skip_weekends,
        "any_slot_suits_everyone": r.any_all_available,
        "dst_notes": r.dst_notes,
        "participants": people,
        "slots": slots,
    }))
    .unwrap_or_else(|e| format!("could not serialize result: {e}"))
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(r: &Report) -> String {
    let mut out = vec![
        "rank,start_utc,end_utc,start_display,end_display,display_zone,all_available,fairness_score,average_score,participant,zone,local_start,local_end,local_date,status,score"
            .to_string(),
    ];
    for (i, s) in r.ranked.iter().enumerate() {
        for p in &s.participants {
            out.push(
                [
                    (i + 1).to_string(),
                    s.start_utc.clone(),
                    s.end_utc.clone(),
                    s.start_display.clone(),
                    s.end_display.clone(),
                    r.display_zone.clone(),
                    s.all_available.to_string(),
                    s.worst.to_string(),
                    s.average.to_string(),
                    csv_escape(&p.name),
                    p.zone.clone(),
                    p.local_start.clone(),
                    p.local_end.clone(),
                    p.local_date.clone(),
                    p.status.to_string(),
                    p.score.to_string(),
                ]
                .join(","),
            );
        }
    }
    out.join("\n")
}

fn ics_compact(iso_utc: &str) -> String {
    iso_utc.replace(['-', ':'], "")
}

fn render_ics(r: &Report) -> String {
    let Some(best) = r.ranked.first() else {
        return format!(
            "No slot on {} works for everyone inside their working hours, so there is nothing to export.",
            r.date.format("%a %d %b %Y")
        );
    };
    let start = ics_compact(&best.start_utc);
    let end = ics_compact(&best.end_utc);
    let stamp = format!("{}T000000Z", r.date.format("%Y%m%d"));
    let description = best
        .participants
        .iter()
        .map(|p| format!("{} ({}) {}-{} {}", p.name, p.zone, p.local_start, p.local_end, p.status))
        .collect::<Vec<_>>()
        .join("\\n");
    [
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//gizza-ai//meeting-time-finder//EN".to_string(),
        "CALSCALE:GREGORIAN".to_string(),
        "METHOD:PUBLISH".to_string(),
        "BEGIN:VEVENT".to_string(),
        format!("UID:{start}-meeting-time-finder@gizza-ai"),
        format!("DTSTAMP:{stamp}"),
        format!("DTSTART:{start}"),
        format!("DTEND:{end}"),
        format!(
            "SUMMARY:Meeting ({} participant{})",
            r.people.len(),
            if r.people.len() == 1 { "" } else { "s" }
        ),
        format!("DESCRIPTION:{description}"),
        "END:VEVENT".to_string(),
        "END:VCALENDAR".to_string(),
    ]
    .join("\r\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_find(participants: &str, format: &str) -> Result<String, String> {
        find(
            participants,
            "2026-10-01",
            "09:00",
            "17:00",
            60.0,
            "30",
            3.0,
            "24h",
            "",
            true,
            true,
            format,
        )
    }

    #[test]
    fn finds_the_classic_london_new_york_overlap() {
        let out = default_find("Alice@Europe/London, Bob@America/New_York", "summary").unwrap();
        // 14:00 London = 09:00 New York, the first hour both are working.
        assert!(out.contains("works for all 2"), "{out}");
        assert!(out.contains("slots shown in Europe/London"), "{out}");
        let best_line = out
            .lines()
            .find(|l| l.starts_with("Best: "))
            .expect("a best line");
        assert!(
            best_line.contains("15:00-16:00") || best_line.contains("14:00-15:00"),
            "{best_line}"
        );
    }

    #[test]
    fn scores_the_midday_slot_highest_for_a_single_participant() {
        let out = default_find("Europe/London", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["slots"][0]["start_display"], "12:30");
        assert_eq!(v["slots"][0]["fairness_score"], 100);
        assert_eq!(v["participants"][0]["name"], "London");
    }

    #[test]
    fn per_participant_working_hours_override_the_default() {
        let out = find(
            "Early@Europe/London:06:00-12:00",
            "2026-10-01",
            "09:00",
            "17:00",
            60.0,
            "60",
            1.0,
            "24h",
            "",
            true,
            true,
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["participants"][0]["work_start"], "06:00");
        assert_eq!(v["slots"][0]["start_display"], "08:00");
    }

    #[test]
    fn impossible_overlap_is_reported_when_partials_are_disallowed() {
        let out = find(
            "Asia/Tokyo, America/Los_Angeles",
            "2026-10-01",
            "09:00",
            "17:00",
            60.0,
            "60",
            5.0,
            "24h",
            "UTC",
            true,
            false,
            "summary",
        )
        .unwrap();
        assert!(out.contains("No slot on Thu 01 Oct 2026 works for everyone"), "{out}");
    }

    #[test]
    fn weekend_participants_are_marked_and_skipped() {
        // 2026-10-03 is a Saturday.
        let out = find(
            "Europe/London",
            "2026-10-03",
            "09:00",
            "17:00",
            60.0,
            "60",
            1.0,
            "24h",
            "",
            true,
            true,
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["slots"][0]["participants"][0]["status"], "weekend");
        assert_eq!(v["any_slot_suits_everyone"], false);
    }

    #[test]
    fn weekends_can_be_allowed() {
        let out = find(
            "Europe/London",
            "2026-10-03",
            "09:00",
            "17:00",
            60.0,
            "60",
            1.0,
            "24h",
            "",
            false,
            true,
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["slots"][0]["participants"][0]["status"], "in hours");
        assert_eq!(v["slots"][0]["participants"][0]["weekday"], "Saturday");
    }

    #[test]
    fn dst_transition_on_the_date_is_flagged() {
        // 2026-10-25: Europe/London falls back to GMT.
        let out = find(
            "Europe/London",
            "2026-10-25",
            "09:00",
            "17:00",
            60.0,
            "60",
            1.0,
            "24h",
            "",
            false,
            true,
            "summary",
        )
        .unwrap();
        assert!(out.contains("DST: Europe/London changes its UTC offset"), "{out}");
        assert!(out.contains("+01:00 -> +00:00"), "{out}");
    }

    #[test]
    fn twelve_hour_clock_renders_am_pm() {
        let out = find(
            "Europe/London",
            "2026-10-01",
            "9am",
            "5pm",
            60.0,
            "60",
            1.0,
            "12h",
            "",
            true,
            true,
            "summary",
        )
        .unwrap();
        assert!(out.contains("12:00 PM-1:00 PM"), "{out}");
    }

    #[test]
    fn night_shift_window_wraps_past_midnight() {
        let out = find(
            "Night@UTC:22:00-06:00",
            "2026-10-01",
            "09:00",
            "17:00",
            60.0,
            "60",
            2.0,
            "24h",
            "UTC",
            false,
            true,
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["slots"][0]["start_display"], "01:00");
        assert_eq!(v["slots"][0]["participants"][0]["status"], "in hours");
    }

    #[test]
    fn timeline_renders_24_hour_columns() {
        let out = default_find("Europe/London, America/New_York", "timeline").unwrap();
        assert!(out.contains("EVERYONE"), "{out}");
        assert!(out.contains(" 00 01 02"), "{out}");
        assert!(out.contains("Legend: # fully inside working hours"), "{out}");
    }

    #[test]
    fn csv_has_one_row_per_participant_per_slot() {
        let out = default_find("Europe/London, America/New_York", "csv").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].starts_with("rank,start_utc,end_utc"));
        assert_eq!(lines.len(), 1 + 3 * 2);
    }

    #[test]
    fn ics_export_wraps_the_best_slot() {
        let out = default_find("Europe/London, America/New_York", "ics").unwrap();
        assert!(out.starts_with("BEGIN:VCALENDAR\r\n"), "{out}");
        assert!(out.contains("DTSTART:20261001T"), "{out}");
        assert!(out.ends_with("END:VCALENDAR"), "{out}");
    }

    #[test]
    fn table_lists_one_row_per_ranked_slot() {
        let out = default_find("Europe/London, America/New_York", "table").unwrap();
        let header = out.lines().find(|l| l.starts_with("#  ")).expect("header row");
        assert!(header.contains("Start (Europe/London)"), "{header}");
        assert!(header.contains("Fair"), "{header}");
    }

    #[test]
    fn unknown_timezone_is_rejected_with_guidance() {
        let err = default_find("Alice@Mars/Olympus", "summary").unwrap_err();
        assert!(err.contains("unknown timezone"), "{err}");
        assert!(err.contains("Europe/London"), "{err}");
    }

    #[test]
    fn empty_participants_is_rejected() {
        let err = default_find("  ", "summary").unwrap_err();
        assert!(err.contains("participants is empty"), "{err}");
    }

    #[test]
    fn too_many_participants_is_rejected() {
        let list = vec!["UTC"; MAX_PARTICIPANTS + 1].join(",");
        let err = default_find(&list, "summary").unwrap_err();
        assert!(err.contains("too many participants: 13 (max 12)"), "{err}");
    }

    #[test]
    fn bad_duration_and_granularity_are_rejected() {
        let err = find(
            "UTC", "2026-10-01", "09:00", "17:00", 1.0, "30", 3.0, "24h", "", true, true, "summary",
        )
        .unwrap_err();
        assert!(err.contains("duration_minutes must be between 5 and 480"), "{err}");
        let err = find(
            "UTC", "2026-10-01", "09:00", "17:00", 60.0, "45", 3.0, "24h", "", true, true, "summary",
        )
        .unwrap_err();
        assert!(err.contains("granularity_minutes must be 15, 30 or 60"), "{err}");
    }

    #[test]
    fn bad_date_and_format_are_rejected() {
        let err = find(
            "UTC", "01/10/2026", "09:00", "17:00", 60.0, "30", 3.0, "24h", "", true, true, "summary",
        )
        .unwrap_err();
        assert!(err.contains("could not parse date"), "{err}");
        let err = default_find("UTC", "pdf").unwrap_err();
        assert!(err.contains("output_format must be summary"), "{err}");
    }

    #[test]
    fn empty_working_window_is_rejected() {
        let err = find(
            "UTC", "2026-10-01", "09:00", "09:00", 60.0, "30", 3.0, "24h", "", true, true, "summary",
        )
        .unwrap_err();
        assert!(err.contains("default working day is empty"), "{err}");
    }

    #[test]
    fn granularity_15_gives_quarter_hour_starts() {
        let out = find(
            "Europe/London",
            "2026-10-01",
            "09:00",
            "17:00",
            30.0,
            "15",
            4.0,
            "24h",
            "",
            true,
            true,
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let starts: Vec<String> = v["slots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["start_display"].as_str().unwrap().to_string())
            .collect();
        assert!(starts.contains(&"12:45".to_string()), "{starts:?}");
    }

    #[test]
    fn display_zone_switches_the_ranked_column() {
        let out = find(
            "Europe/London, America/New_York",
            "2026-10-01",
            "09:00",
            "17:00",
            60.0,
            "60",
            1.0,
            "24h",
            "Asia/Tokyo",
            true,
            true,
            "summary",
        )
        .unwrap();
        assert!(out.contains("slots shown in Asia/Tokyo"), "{out}");
    }
}
