//! text-to-reminders core — pure compute, shared by the chat skill block and the
//! web page. No deps. Turns a free-form brain-dump (one task per line) into an
//! iCalendar (RFC 5545) `.ics` document: one reminder/task component per line,
//! with a due date/time parsed deterministically from natural-language phrases
//! (today, tomorrow, "next Monday", "in 3 days", "March 5", 3/5, at 5pm, noon …),
//! anchored on an explicit `reference_date` so every surface stays deterministic.
//!
//! Nothing is invented: a line only gains a due date when it contains a
//! recognised date/time phrase, and a priority only when it contains a priority
//! keyword. All parsing is table-driven and reproducible — no LLM, no clock
//! inside core (each surface passes its own `reference_date`).

/// Days-since-Unix-epoch for a civil (proleptic Gregorian) date.
/// Howard Hinnant's `days_from_civil`.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe - 719468
}

/// Inverse of [`days_from_civil`]: a Unix-epoch day count → `(year, month, day)`.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
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

/// Day of week for a Unix-epoch day count. 0 = Sunday … 6 = Saturday.
fn weekday(z: i64) -> i64 {
    ((z % 7) + 11) % 7 // 1970-01-01 (z=0) is Thursday = 4
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Parse a `YYYY-MM-DD` (leading 10 chars) into an epoch day count.
fn parse_ref_date(s: &str) -> Result<i64, String> {
    let s = s.trim();
    let head: String = s.chars().take(10).collect();
    let parts: Vec<&str> = head.split('-').collect();
    if parts.len() != 3 {
        return Err(format!(
            "reference_date must be an ISO date like 2026-03-02 (got {:?})",
            s
        ));
    }
    let y: i64 = parts[0]
        .parse()
        .map_err(|_| "reference_date year is not a number".to_string())?;
    let m: i64 = parts[1]
        .parse()
        .map_err(|_| "reference_date month is not a number".to_string())?;
    let d: i64 = parts[2]
        .parse()
        .map_err(|_| "reference_date day is not a number".to_string())?;
    if !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return Err(format!("reference_date {:?} is not a valid calendar date", s));
    }
    Ok(days_from_civil(y, m, d))
}

// ----------------------------------------------------------------------------
// Task model
// ----------------------------------------------------------------------------

/// A resolved due moment for one task.
#[derive(Debug, Clone, PartialEq)]
enum Due {
    /// All-day: iCalendar `VALUE=DATE`.
    Date(i64),
    /// Floating local date-time: `YYYYMMDDTHHMMSS` (no TZID — see FAQ).
    DateTime(i64, u32, u32),
}

/// One parsed line → one iCalendar reminder/task component.
#[derive(Debug, Clone, PartialEq)]
struct Task {
    summary: String,
    due: Option<Due>,
    /// iCalendar `PRIORITY` (1 = high … 9 = low); `None` = unset.
    priority: Option<u8>,
}

// ----------------------------------------------------------------------------
// Small string helpers
// ----------------------------------------------------------------------------

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Word-bounded, case-insensitive find of `needle` in the already-lowercased
/// `hay`. Returns the byte range of the match.
fn wb(hay: &str, needle: &str) -> Option<(usize, usize)> {
    if needle.is_empty() {
        return None;
    }
    let hb = hay.as_bytes();
    let mut i = 0;
    while i + needle.len() <= hb.len() {
        if &hay[i..i + needle.len()] == needle {
            let before_ok = i == 0 || !is_word_byte(hb[i - 1]);
            let after = i + needle.len();
            let after_ok = after == hb.len() || !is_word_byte(hb[after]);
            // If the needle itself starts/ends with a word byte, require a
            // boundary; punctuation-containing needles are matched literally.
            if (!is_word_byte(needle.as_bytes()[0]) || before_ok)
                && (!is_word_byte(*needle.as_bytes().last().unwrap()) || after_ok)
            {
                return Some((i, after));
            }
        }
        i += 1;
    }
    None
}

/// Alnum tokens (lowercased runs of `[a-z0-9]`) with their byte offsets.
fn tokens(lower: &str) -> Vec<(usize, usize, &str)> {
    let b = lower.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_alphanumeric() {
            let start = i;
            while i < b.len() && b[i].is_ascii_alphanumeric() {
                i += 1;
            }
            out.push((start, i, &lower[start..i]));
        } else {
            i += 1;
        }
    }
    out
}

/// A day number `1..=31` from a token like `5`, `05`, `5th`, `21st`.
fn parse_day_token(tok: &str) -> Option<u32> {
    let digits: String = tok.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.len() > 2 {
        return None;
    }
    let rest = &tok[digits.len()..];
    if !rest.is_empty() && !matches!(rest, "st" | "nd" | "rd" | "th") {
        return None;
    }
    let d: u32 = digits.parse().ok()?;
    if (1..=31).contains(&d) {
        Some(d)
    } else {
        None
    }
}

/// Strip leading list markers (bullets, checkboxes, numbering) from a line.
fn strip_bullet(s: &str) -> String {
    let mut s = s.trim().to_string();
    loop {
        let before = s.clone();
        let t = s.trim_start();
        // Checkbox: [ ] [x] [] at the very start.
        if let Some(rest) = t
            .strip_prefix("[ ]")
            .or_else(|| t.strip_prefix("[x]"))
            .or_else(|| t.strip_prefix("[X]"))
            .or_else(|| t.strip_prefix("[]"))
        {
            s = rest.trim_start().to_string();
        } else if let Some(rest) = t
            .strip_prefix("- ")
            .or_else(|| t.strip_prefix("* "))
            .or_else(|| t.strip_prefix("• "))
            .or_else(|| t.strip_prefix("· "))
            .or_else(|| t.strip_prefix("– "))
            .or_else(|| t.strip_prefix("‣ "))
        {
            s = rest.trim_start().to_string();
        } else {
            // Numbered: leading digits then '.' or ')' then a space.
            let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
            let after = &t[digits.len()..];
            if !digits.is_empty()
                && (after.starts_with(". ") || after.starts_with(") "))
            {
                s = after[2..].trim_start().to_string();
            } else {
                s = t.to_string();
            }
        }
        if s == before {
            break;
        }
    }
    s
}

// ----------------------------------------------------------------------------
// Date parsing
// ----------------------------------------------------------------------------

const WEEKDAYS: &[(&str, i64, &[&str])] = &[
    ("sunday", 0, &["sun"]),
    ("monday", 1, &["mon"]),
    ("tuesday", 2, &["tue", "tues"]),
    ("wednesday", 3, &["wed"]),
    ("thursday", 4, &["thu", "thur", "thurs"]),
    ("friday", 5, &["fri"]),
    ("saturday", 6, &["sat"]),
];

const MONTHS: &[(&str, &[&str])] = &[
    ("january", &["jan"]),
    ("february", &["feb"]),
    ("march", &["mar"]),
    ("april", &["apr"]),
    ("may", &[]),
    ("june", &["jun"]),
    ("july", &["jul"]),
    ("august", &["aug"]),
    ("september", &["sep", "sept"]),
    ("october", &["oct"]),
    ("november", &["nov"]),
    ("december", &["dec"]),
];

/// The soonest date strictly after `reference` that falls on weekday `target`.
fn next_weekday(reference: i64, target: i64) -> i64 {
    let cur = weekday(reference);
    let mut delta = (target - cur + 7) % 7;
    if delta == 0 {
        delta = 7; // a bare weekday means the coming one, never today
    }
    reference + delta
}

fn add_months(reference: i64, months: i64) -> i64 {
    let (y, m, d) = civil_from_days(reference);
    let total = (m - 1) + months;
    let ny = y + total.div_euclid(12);
    let nm = total.rem_euclid(12) + 1;
    let nd = d.min(days_in_month(ny, nm));
    days_from_civil(ny, nm, nd)
}

type DateHit = (usize, usize, i64, Option<(u32, u32)>);

/// Find the first date phrase in `lower`. Returns `(start, end, epoch_days,
/// default_time)` where `default_time` is set for phrases that imply a
/// time-of-day (e.g. "tonight").
fn find_date(lower: &str, reference: i64) -> Option<DateHit> {
    let mut hits: Vec<DateHit> = Vec::new();

    // Fixed relative phrases (longest first so "day after tomorrow" wins).
    let rel: &[(&str, i64, Option<(u32, u32)>)] = &[
        ("day after tomorrow", 2, None),
        ("day-after-tomorrow", 2, None),
        ("tomorrow", 1, None),
        ("tonight", 0, Some((19, 0))),
        ("today", 0, None),
        ("yesterday", -1, None),
    ];
    for (p, off, dt) in rel {
        if let Some((s, e)) = wb(lower, p) {
            hits.push((s, e, reference + off, *dt));
        }
    }
    if let Some((s, e)) = wb(lower, "next week") {
        hits.push((s, e, reference + 7, None));
    }
    if let Some((s, e)) = wb(lower, "next month") {
        hits.push((s, e, add_months(reference, 1), None));
    }
    if let Some((s, e)) = wb(lower, "weekend") {
        // "this/next weekend" → the coming Saturday.
        hits.push((s, e, next_weekday(reference, 6), None));
    }

    // Weekdays (with optional leading "next").
    for (name, wd, abbrevs) in WEEKDAYS {
        let mut found: Option<(usize, usize)> = None;
        let next_form = format!("next {name}");
        if let Some((s, e)) = wb(lower, &next_form) {
            found = Some((s, e));
        } else if let Some((s, e)) = wb(lower, name) {
            found = Some((s, e));
        } else {
            for ab in *abbrevs {
                if let Some((s, e)) = wb(lower, ab) {
                    found = Some((s, e));
                    break;
                }
            }
        }
        if let Some((s, e)) = found {
            hits.push((s, e, next_weekday(reference, *wd), None));
        }
    }

    // "in N days" / "in N weeks" (also "in a day/week").
    let toks = tokens(lower);
    for w in 0..toks.len() {
        if toks[w].2 == "in" && w + 2 < toks.len() {
            let n = if toks[w + 1].2 == "a" || toks[w + 1].2 == "an" {
                Some(1i64)
            } else {
                toks[w + 1].2.parse::<i64>().ok()
            };
            if let Some(n) = n {
                let unit = toks[w + 2].2;
                let days = if unit.starts_with("day") {
                    Some(n)
                } else if unit.starts_with("week") {
                    Some(n * 7)
                } else if unit.starts_with("month") {
                    Some(-1) // sentinel handled below
                } else {
                    None
                };
                if let Some(d) = days {
                    let resolved = if unit.starts_with("month") {
                        add_months(reference, n)
                    } else {
                        reference + d
                    };
                    hits.push((toks[w].0, toks[w + 2].1, resolved, None));
                }
            }
        }
    }

    // Month-name forms: "March 5[, 2027]" and "5 Mar[ 2027]".
    for mi in 0..MONTHS.len() {
        let (full, abbrs) = MONTHS[mi];
        // locate the month token index
        let mut midx = None;
        for (ti, t) in toks.iter().enumerate() {
            if t.2 == full || abbrs.contains(&t.2) {
                midx = Some(ti);
                break;
            }
        }
        let Some(ti) = midx else { continue };
        let month = (mi + 1) as i64;
        // Day: prefer the following token, else the preceding token.
        let (day, day_ti_after) = if ti + 1 < toks.len() {
            (parse_day_token(toks[ti + 1].2), true)
        } else {
            (None, true)
        };
        let (day, after) = match day {
            Some(d) => (Some(d), day_ti_after),
            None => {
                if ti > 0 {
                    (parse_day_token(toks[ti - 1].2), false)
                } else {
                    (None, false)
                }
            }
        };
        let Some(day) = day else { continue };
        // Optional explicit year: 4-digit token after the day.
        let mut year: Option<i64> = None;
        let year_tok_idx = if after { ti + 2 } else { ti + 1 };
        if year_tok_idx < toks.len() {
            let yt = toks[year_tok_idx].2;
            if yt.len() == 4 {
                if let Ok(y) = yt.parse::<i64>() {
                    year = Some(y);
                }
            }
        }
        let (start, end) = if after {
            // month .. day (.. year)
            let s = toks[ti].0;
            let e = if year.is_some() {
                toks[year_tok_idx].1
            } else {
                toks[ti + 1].1
            };
            (s, e)
        } else {
            // day .. month (.. year)
            let s = toks[ti - 1].0;
            let e = if year.is_some() {
                toks[year_tok_idx].1
            } else {
                toks[ti].1
            };
            (s, e)
        };
        let base_year = year.unwrap_or_else(|| civil_from_days(reference).0);
        if day as i64 <= days_in_month(base_year, month) {
            let mut days = days_from_civil(base_year, month, day as i64);
            if year.is_none() && days < reference {
                days = days_from_civil(base_year + 1, month, day as i64);
            }
            hits.push((start, end, days, None));
        }
    }

    // ISO YYYY-MM-DD.
    if let Some(h) = find_iso(lower) {
        hits.push(h);
    }
    // M/D[/Y].
    if let Some(h) = find_slash(lower, reference) {
        hits.push(h);
    }

    // Earliest mention wins; ties broken by the longer match.
    hits.into_iter()
        .min_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
}

fn find_iso(lower: &str) -> Option<DateHit> {
    let b = lower.as_bytes();
    let mut i = 0;
    while i + 10 <= b.len() {
        let ok = b[i].is_ascii_digit()
            && b[i + 1].is_ascii_digit()
            && b[i + 2].is_ascii_digit()
            && b[i + 3].is_ascii_digit()
            && b[i + 4] == b'-'
            && b[i + 5].is_ascii_digit()
            && b[i + 6].is_ascii_digit()
            && b[i + 7] == b'-'
            && b[i + 8].is_ascii_digit()
            && b[i + 9].is_ascii_digit();
        let before_ok = i == 0 || !is_word_byte(b[i - 1]);
        let after_ok = i + 10 == b.len() || !is_word_byte(b[i + 10]);
        if ok && before_ok && after_ok {
            let y: i64 = lower[i..i + 4].parse().ok()?;
            let m: i64 = lower[i + 5..i + 7].parse().ok()?;
            let d: i64 = lower[i + 8..i + 10].parse().ok()?;
            if (1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m) {
                return Some((i, i + 10, days_from_civil(y, m, d), None));
            }
        }
        i += 1;
    }
    None
}

fn find_slash(lower: &str, reference: i64) -> Option<DateHit> {
    let b = lower.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !is_word_byte(b[i - 1])) {
            let start = i;
            let mut j = i;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let first = &lower[start..j];
            if first.len() <= 2 && j < b.len() && b[j] == b'/' {
                let mut k = j + 1;
                let ds = k;
                while k < b.len() && b[k].is_ascii_digit() {
                    k += 1;
                }
                let second = &lower[ds..k];
                if !second.is_empty() && second.len() <= 2 {
                    // optional /year
                    let mut year: Option<i64> = None;
                    let mut end = k;
                    if k < b.len() && b[k] == b'/' {
                        let ys = k + 1;
                        let mut y2 = ys;
                        while y2 < b.len() && b[y2].is_ascii_digit() {
                            y2 += 1;
                        }
                        let ytok = &lower[ys..y2];
                        if ytok.len() == 2 {
                            year = Some(2000 + ytok.parse::<i64>().ok()?);
                            end = y2;
                        } else if ytok.len() == 4 {
                            year = Some(ytok.parse::<i64>().ok()?);
                            end = y2;
                        }
                    }
                    let after_ok = end == b.len() || !is_word_byte(b[end]);
                    let m: i64 = first.parse().ok()?;
                    let d: i64 = second.parse().ok()?;
                    if after_ok && (1..=12).contains(&m) && d >= 1 {
                        let by = year.unwrap_or_else(|| civil_from_days(reference).0);
                        if d <= days_in_month(by, m) {
                            let mut days = days_from_civil(by, m, d);
                            if year.is_none() && days < reference {
                                days = days_from_civil(by + 1, m, d);
                            }
                            return Some((start, end, days, None));
                        }
                    }
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    None
}

// ----------------------------------------------------------------------------
// Time parsing
// ----------------------------------------------------------------------------

/// Find the first time-of-day phrase. Returns `(start, end, hour, minute)`.
fn find_time(lower: &str) -> Option<(usize, usize, u32, u32)> {
    let mut hits: Vec<(usize, usize, u32, u32)> = Vec::new();

    let named: &[(&str, u32, u32)] = &[
        ("midnight", 0, 0),
        ("midday", 12, 0),
        ("noon", 12, 0),
        ("morning", 9, 0),
        ("afternoon", 14, 0),
        ("evening", 19, 0),
    ];
    for (n, h, m) in named {
        if let Some((s, e)) = wb(lower, n) {
            hits.push((s, e, *h, *m));
        }
    }

    // Clock: H[:MM][am|pm]; requires a colon or am/pm to qualify.
    let b = lower.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !is_word_byte(b[i - 1])) {
            let start = i;
            let mut j = i;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let hour_str = &lower[start..j];
            if hour_str.len() > 2 {
                i = j;
                continue;
            }
            let mut hour: i64 = hour_str.parse().unwrap_or(-1);
            let mut minute: i64 = 0;
            let mut has_colon = false;
            let mut end = j;
            if j + 2 < b.len() + 1
                && j < b.len()
                && b[j] == b':'
                && j + 2 < b.len() + 1
                && j + 1 < b.len()
                && b[j + 1].is_ascii_digit()
                && j + 2 < b.len()
                && b[j + 2].is_ascii_digit()
            {
                minute = lower[j + 1..j + 3].parse().unwrap_or(0);
                has_colon = true;
                end = j + 3;
            }
            // optional space then am/pm
            let mut k = end;
            if k < b.len() && b[k] == b' ' {
                k += 1;
            }
            let mut has_ampm = false;
            let mut pm = false;
            if k + 2 <= b.len() {
                let suf = &lower[k..k + 2];
                if suf == "am" || suf == "pm" {
                    // must be word-bounded after
                    let after = k + 2;
                    let after_ok = after == b.len() || !is_word_byte(b[after]);
                    if after_ok {
                        has_ampm = true;
                        pm = suf == "pm";
                        end = k + 2;
                    }
                }
            }
            if (has_colon || has_ampm) && (0..=23).contains(&hour) && (0..=59).contains(&minute) {
                if has_ampm {
                    if pm && hour < 12 {
                        hour += 12;
                    }
                    if !pm && hour == 12 {
                        hour = 0;
                    }
                }
                if (0..=23).contains(&hour) {
                    let mut s = start;
                    // absorb a leading "at "
                    if s >= 3 && &lower[s - 3..s] == "at " && (s == 3 || !is_word_byte(b[s - 4])) {
                        s -= 3;
                    }
                    hits.push((s, end, hour as u32, minute as u32));
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }

    hits.into_iter()
        .min_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
}

// ----------------------------------------------------------------------------
// Priority parsing
// ----------------------------------------------------------------------------

/// Detect a priority keyword, returning `(priority, matched_range)`.
fn detect_priority(lower: &str) -> Option<(u8, (usize, usize))> {
    // (phrase, priority). Longest / most specific first.
    let table: &[(&str, u8)] = &[
        ("high priority", 1),
        ("high-priority", 1),
        ("top priority", 1),
        ("low priority", 9),
        ("low-priority", 9),
        ("urgent", 1),
        ("asap", 1),
        ("important", 1),
        ("critical", 1),
        ("someday", 9),
        ("whenever", 9),
        ("eventually", 9),
    ];
    let mut best: Option<(u8, (usize, usize))> = None;
    for (p, prio) in table {
        if let Some((s, e)) = wb(lower, p) {
            match best {
                Some((_, (bs, _))) if bs <= s => {}
                _ => best = Some((*prio, (s, e))),
            }
        }
    }
    best
}

// ----------------------------------------------------------------------------
// Line → Task
// ----------------------------------------------------------------------------

const TRAILING_FILLER: &[&str] = &[
    "at", "on", "by", "due", "in", "this", "next", "from", "for", "the", "of", "before",
];
const LEADING_FILLER: &[&str] = &["at", "on", "by", "due", "-", ":", "for", "to"];

fn clean_summary(s: &str) -> String {
    // collapse whitespace
    let mut out = String::new();
    let mut prev_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space && !out.is_empty() {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    let mut out = out.trim().to_string();

    // strip trailing filler words / dangling punctuation
    loop {
        let before = out.clone();
        out = out
            .trim_end_matches(|c: char| c == '-' || c == ':' || c == ',' || c == ';' || c == '.')
            .trim()
            .to_string();
        let low = out.to_ascii_lowercase();
        for f in TRAILING_FILLER {
            if let Some(stripped) = low.strip_suffix(f) {
                let cut = stripped.len();
                if cut == 0 || out.as_bytes()[cut - 1] == b' ' {
                    out = out[..cut].trim_end().to_string();
                    break;
                }
            }
        }
        if out == before {
            break;
        }
    }
    // strip leading filler
    loop {
        let before = out.clone();
        out = out
            .trim_start_matches(|c: char| c == '-' || c == ':' || c == ',' || c == ';')
            .trim()
            .to_string();
        let low = out.to_ascii_lowercase();
        for f in LEADING_FILLER {
            let pref = format!("{f} ");
            if low.starts_with(&pref) {
                out = out[f.len()..].trim_start().to_string();
                break;
            }
        }
        if out == before {
            break;
        }
    }
    out
}

fn remove_ranges(s: &str, mut ranges: Vec<(usize, usize)>) -> String {
    if ranges.is_empty() {
        return s.to_string();
    }
    ranges.sort();
    let mut out = String::new();
    let mut pos = 0;
    for (start, end) in ranges {
        if start < pos {
            continue; // overlapping / already consumed
        }
        out.push_str(&s[pos..start]);
        out.push(' ');
        pos = end;
    }
    out.push_str(&s[pos..]);
    out
}

fn parse_line(raw: &str, reference: i64, detect_prio: bool) -> Option<Task> {
    let s = strip_bullet(raw);
    if s.trim().is_empty() {
        return None;
    }
    let lower = s.to_ascii_lowercase();
    let mut ranges: Vec<(usize, usize)> = Vec::new();

    let priority = if detect_prio {
        detect_priority(&lower).map(|(p, r)| {
            ranges.push(r);
            p
        })
    } else {
        None
    };

    let date = find_date(&lower, reference);
    let time = find_time(&lower);
    if let Some((s0, e0, ..)) = date {
        ranges.push((s0, e0));
    }
    if let Some((s0, e0, ..)) = time {
        ranges.push((s0, e0));
    }

    let due = match (date, time) {
        (Some((_, _, days, dflt)), Some((_, _, h, m))) => {
            let _ = dflt;
            Some(Due::DateTime(days, h, m))
        }
        (Some((_, _, days, Some((h, m)))), None) => Some(Due::DateTime(days, h, m)),
        (Some((_, _, days, None)), None) => Some(Due::Date(days)),
        (None, Some((_, _, h, m))) => Some(Due::DateTime(reference, h, m)),
        (None, None) => None,
    };

    let summary = clean_summary(&remove_ranges(&s, ranges));
    if summary.is_empty() {
        return None;
    }
    Some(Task {
        summary,
        due,
        priority,
    })
}

// ----------------------------------------------------------------------------
// iCalendar emission
// ----------------------------------------------------------------------------

/// Escape an iCalendar TEXT value (RFC 5545 §3.3.11).
fn escape_text(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            _ => out.push(ch),
        }
    }
    out
}

/// Fold a content line at 75 octets (RFC 5545 §3.1), continuation lines led by a
/// single space.
fn fold(line: &str) -> String {
    if line.len() <= 75 {
        return line.to_string();
    }
    let mut segs: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_bytes = 0usize;
    let mut first = true;
    for ch in line.chars() {
        let cap = if first { 75 } else { 74 };
        if cur_bytes + ch.len_utf8() > cap {
            segs.push(std::mem::take(&mut cur));
            cur_bytes = 0;
            first = false;
        }
        cur.push(ch);
        cur_bytes += ch.len_utf8();
    }
    if !cur.is_empty() {
        segs.push(cur);
    }
    segs.join("\r\n ")
}

fn compact_date(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{:04}{:02}{:02}", y, m, d)
}

/// Build a `.ics` (VCALENDAR) document from free-form text.
///
/// * `text` — one task per non-blank line.
/// * `reference_date` — ISO `YYYY-MM-DD` anchor for relative phrases.
/// * `detect_priority` — map priority keywords → iCalendar PRIORITY.
/// * `include_undated` — keep lines with no date as tasks with no due.
/// * `alarm_minutes` — if > 0, add a display reminder N minutes before each due.
pub fn build_reminders(
    text: &str,
    reference_date: &str,
    detect_priority_flag: bool,
    include_undated: bool,
    alarm_minutes: i64,
) -> Result<String, String> {
    let reference = parse_ref_date(reference_date)?;
    let alarm = alarm_minutes.max(0);

    let mut tasks: Vec<Task> = Vec::new();
    for line in text.lines() {
        if let Some(t) = parse_line(line, reference, detect_priority_flag) {
            if t.due.is_none() && !include_undated {
                continue;
            }
            tasks.push(t);
        }
    }

    if tasks.is_empty() {
        return Err("no reminders found: put one task per line, for example \
             'Call the dentist tomorrow at 3pm' or 'Submit the report by Friday'. \
             Turn on \"keep undated tasks\" to also list lines with no date."
            .to_string());
    }

    let stamp = format!("{}T000000Z", compact_date(reference));
    let ref_compact = compact_date(reference);

    let mut lines: Vec<String> = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//gizza-ai//text-to-reminders//EN".to_string(),
        "CALSCALE:GREGORIAN".to_string(),
    ];

    for (i, t) in tasks.iter().enumerate() {
        lines.push("BEGIN:VTODO".to_string());
        lines.push(fold(&format!("UID:todo-{}-{}@text-to-reminders", i + 1, ref_compact)));
        lines.push(format!("DTSTAMP:{}", stamp));
        lines.push(fold(&format!("SUMMARY:{}", escape_text(&t.summary))));
        match &t.due {
            Some(Due::Date(days)) => {
                lines.push(format!("DUE;VALUE=DATE:{}", compact_date(*days)));
            }
            Some(Due::DateTime(days, h, m)) => {
                lines.push(format!(
                    "DUE:{}T{:02}{:02}00",
                    compact_date(*days),
                    h,
                    m
                ));
            }
            None => {}
        }
        if let Some(p) = t.priority {
            lines.push(format!("PRIORITY:{}", p));
        }
        if alarm > 0 && t.due.is_some() {
            lines.push("BEGIN:VALARM".to_string());
            lines.push("ACTION:DISPLAY".to_string());
            lines.push(fold(&format!("DESCRIPTION:{}", escape_text(&t.summary))));
            lines.push(format!("TRIGGER;RELATED=END:-PT{}M", alarm));
            lines.push("END:VALARM".to_string());
        }
        lines.push("END:VTODO".to_string());
    }

    lines.push("END:VCALENDAR".to_string());
    Ok(lines.join("\r\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-03-02 is a Monday.
    const REF: &str = "2026-03-02";

    fn build(text: &str) -> String {
        build_reminders(text, REF, true, true, 0).unwrap()
    }

    #[test]
    fn ref_date_is_monday() {
        let days = parse_ref_date(REF).unwrap();
        assert_eq!(weekday(days), 1);
        assert_eq!(civil_from_days(days), (2026, 3, 2));
    }

    #[test]
    fn header_and_single_all_day_task() {
        let out = build("Renew passport tomorrow");
        assert!(out.starts_with("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n"));
        assert!(out.contains("PRODID:-//gizza-ai//text-to-reminders//EN"));
        assert!(out.contains("BEGIN:VTODO"));
        assert!(out.contains("SUMMARY:Renew passport"));
        assert!(out.contains("DUE;VALUE=DATE:20260303"));
        assert!(out.ends_with("END:VCALENDAR"));
        assert_eq!(out.matches("BEGIN:VTODO").count(), 1);
    }

    #[test]
    fn timed_task_from_tomorrow_at_time() {
        let out = build("Call the dentist tomorrow at 3pm");
        assert!(out.contains("SUMMARY:Call the dentist"));
        assert!(out.contains("DUE:20260303T150000"));
    }

    #[test]
    fn weekday_and_noon() {
        // next Friday after Mon 2026-03-02 is 2026-03-06.
        let out = build("Team lunch Friday at noon");
        assert!(out.contains("SUMMARY:Team lunch"));
        assert!(out.contains("DUE:20260306T120000"));
    }

    #[test]
    fn next_weekday_rolls_a_week() {
        // next Monday (bare or "next") after Mon 03-02 is 03-09.
        let out = build("Standup next Monday");
        assert!(out.contains("DUE;VALUE=DATE:20260309"));
    }

    #[test]
    fn in_n_days_and_weeks() {
        let out = build("Ship release in 3 days\nRetro in 2 weeks");
        assert!(out.contains("DUE;VALUE=DATE:20260305")); // +3
        assert!(out.contains("DUE;VALUE=DATE:20260316")); // +14
        assert!(out.contains("SUMMARY:Ship release"));
        assert!(out.contains("SUMMARY:Retro"));
    }

    #[test]
    fn absolute_month_name_and_year() {
        let out = build("File taxes April 15 2027");
        assert!(out.contains("SUMMARY:File taxes"));
        assert!(out.contains("DUE;VALUE=DATE:20270415"));
    }

    #[test]
    fn day_before_month_form() {
        let out = build("Dentist 5 Mar");
        // 5 Mar 2026 is after the 2nd, same year.
        assert!(out.contains("SUMMARY:Dentist"));
        assert!(out.contains("DUE;VALUE=DATE:20260305"));
    }

    #[test]
    fn iso_and_slash_dates() {
        let out = build("Renew domain 2026-12-31\nPay rent 4/1");
        assert!(out.contains("DUE;VALUE=DATE:20261231"));
        assert!(out.contains("DUE;VALUE=DATE:20260401"));
    }

    #[test]
    fn slash_date_in_past_rolls_forward() {
        // 1/1 is before the reference 2026-03-02, so it rolls to 2027.
        let out = build("Annual review 1/1");
        assert!(out.contains("DUE;VALUE=DATE:20270101"));
    }

    #[test]
    fn priority_high_detected_and_stripped() {
        let out = build("Urgent: reply to the auditor tomorrow");
        assert!(out.contains("SUMMARY:reply to the auditor"));
        assert!(out.contains("PRIORITY:1"));
    }

    #[test]
    fn priority_can_be_disabled() {
        let out = build_reminders("Urgent thing tomorrow", REF, false, true, 0).unwrap();
        assert!(!out.contains("PRIORITY:"));
        // keyword kept in the summary when detection is off
        assert!(out.contains("SUMMARY:Urgent thing"));
    }

    #[test]
    fn undated_kept_or_dropped() {
        let kept = build("Buy milk");
        assert!(kept.contains("SUMMARY:Buy milk"));
        assert!(!kept.contains("DUE"));

        let dropped = build_reminders("Buy milk\nCall Bob tomorrow", REF, true, false, 0).unwrap();
        assert!(!dropped.contains("SUMMARY:Buy milk"));
        assert!(dropped.contains("SUMMARY:Call Bob"));
    }

    #[test]
    fn alarm_adds_valarm_only_for_dated() {
        let out = build_reminders("Take medicine tonight\nBuy milk", REF, true, true, 30).unwrap();
        assert!(out.contains("BEGIN:VALARM"));
        assert!(out.contains("TRIGGER;RELATED=END:-PT30M"));
        // undated task has no alarm
        assert_eq!(out.matches("BEGIN:VALARM").count(), 1);
    }

    #[test]
    fn tonight_defaults_to_evening() {
        let out = build("Water the plants tonight");
        assert!(out.contains("DUE:20260302T190000"));
        assert!(out.contains("SUMMARY:Water the plants"));
    }

    #[test]
    fn bullets_and_checkboxes_stripped() {
        let out = build("- [ ] 1. Email the client tomorrow");
        assert!(out.contains("SUMMARY:Email the client"));
        assert!(out.contains("DUE;VALUE=DATE:20260303"));
    }

    #[test]
    fn escaping_in_summary() {
        let out = build("Buy eggs, milk; and bread tomorrow");
        assert!(out.contains("SUMMARY:Buy eggs\\, milk\\; and bread"));
    }

    #[test]
    fn twentyfour_hour_time() {
        let out = build("Standup 2026-03-05 at 09:30");
        assert!(out.contains("DUE:20260305T093000"));
    }

    #[test]
    fn empty_input_errors() {
        let err = build_reminders("   \n\n", REF, true, true, 0).unwrap_err();
        assert!(err.contains("no reminders found"));
    }

    #[test]
    fn multiple_tasks_get_unique_uids() {
        let out = build("Call Bob tomorrow\nEmail Sue Friday");
        assert!(out.contains("UID:todo-1-20260302@text-to-reminders"));
        assert!(out.contains("UID:todo-2-20260302@text-to-reminders"));
    }

    #[test]
    fn crlf_line_endings() {
        let out = build("Buy milk tomorrow");
        assert!(out.contains("\r\n"));
        assert!(!out.contains("\n\n"));
    }

    #[test]
    fn folding_long_summary() {
        let long = "Prepare the very detailed quarterly board presentation covering revenue growth and hiring plans tomorrow";
        let out = build(long);
        // a folded continuation line begins with a space
        assert!(out.contains("\r\n "));
    }
}
