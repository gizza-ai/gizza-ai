//! natural-language-task core — pure compute, shared by the chat skill block and
//! the web page. No deps. Turns a plain-English task sentence into a single
//! `todo.txt` line: a leading `(A)`–`(D)` priority, an optional creation date,
//! the description with any inline `+project`/`@context` tags preserved, and a
//! `due:YYYY-MM-DD` key/value parsed from a natural-language date phrase
//! ("tomorrow", "next Friday", "in 3 days", ISO dates, "March 5", 3/5 …).
//!
//! Deterministic and table-driven — no LLM, no clock inside core. Each surface
//! passes an explicit `reference_date` so relative phrases resolve reproducibly.
//! Nothing is invented: a line only gains a `due:` date when it contains a
//! recognised date phrase, and a priority only when a priority cue is present.

// ----------------------------------------------------------------------------
// Calendar math (Howard Hinnant's civil <-> days algorithms)
// ----------------------------------------------------------------------------

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// A Unix-epoch day count → `(year, month, day)`.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
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

fn fmt_date(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
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
            let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
            let after = &t[digits.len()..];
            if !digits.is_empty() && (after.starts_with(". ") || after.starts_with(") ")) {
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

type DateHit = (usize, usize, i64);

/// Find the first date phrase in `lower`. Returns `(start, end, epoch_days)`.
fn find_date(lower: &str, reference: i64) -> Option<DateHit> {
    let mut hits: Vec<DateHit> = Vec::new();

    // Fixed relative phrases (longest first so "day after tomorrow" wins).
    let rel: &[(&str, i64)] = &[
        ("day after tomorrow", 2),
        ("day-after-tomorrow", 2),
        ("tomorrow", 1),
        ("tonight", 0),
        ("today", 0),
        ("yesterday", -1),
    ];
    for (p, off) in rel {
        if let Some((s, e)) = wb(lower, p) {
            hits.push((s, e, reference + off));
        }
    }
    if let Some((s, e)) = wb(lower, "next week") {
        hits.push((s, e, reference + 7));
    }
    if let Some((s, e)) = wb(lower, "next month") {
        hits.push((s, e, add_months(reference, 1)));
    }
    if let Some((s, e)) = wb(lower, "weekend") {
        hits.push((s, e, next_weekday(reference, 6)));
    }

    // Weekdays (with optional leading "next"/"this").
    for (name, wd, abbrevs) in WEEKDAYS {
        let mut found: Option<(usize, usize)> = None;
        for prefix in ["next ", "this "] {
            let form = format!("{prefix}{name}");
            if let Some((s, e)) = wb(lower, &form) {
                found = Some((s, e));
                break;
            }
        }
        if found.is_none() {
            if let Some((s, e)) = wb(lower, name) {
                found = Some((s, e));
            } else {
                for ab in *abbrevs {
                    if let Some((s, e)) = wb(lower, ab) {
                        found = Some((s, e));
                        break;
                    }
                }
            }
        }
        if let Some((s, e)) = found {
            hits.push((s, e, next_weekday(reference, *wd)));
        }
    }

    // "in N days" / "in N weeks" / "in N months" (also "in a day/week").
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
                let resolved = if unit.starts_with("day") {
                    Some(reference + n)
                } else if unit.starts_with("week") {
                    Some(reference + n * 7)
                } else if unit.starts_with("month") {
                    Some(add_months(reference, n))
                } else {
                    None
                };
                if let Some(r) = resolved {
                    hits.push((toks[w].0, toks[w + 2].1, r));
                }
            }
        }
    }

    // Month-name forms: "March 5[, 2027]" and "5 Mar[ 2027]".
    for mi in 0..MONTHS.len() {
        let (full, abbrs) = MONTHS[mi];
        let mut midx = None;
        for (ti, t) in toks.iter().enumerate() {
            if t.2 == full || abbrs.contains(&t.2) {
                midx = Some(ti);
                break;
            }
        }
        let Some(ti) = midx else { continue };
        let month = (mi + 1) as i64;
        let (day, after) = {
            let following = if ti + 1 < toks.len() {
                parse_day_token(toks[ti + 1].2)
            } else {
                None
            };
            match following {
                Some(d) => (Some(d), true),
                None => {
                    if ti > 0 {
                        (parse_day_token(toks[ti - 1].2), false)
                    } else {
                        (None, false)
                    }
                }
            }
        };
        let Some(day) = day else { continue };
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
            let s = toks[ti].0;
            let e = if year.is_some() {
                toks[year_tok_idx].1
            } else {
                toks[ti + 1].1
            };
            (s, e)
        } else {
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
            hits.push((start, end, days));
        }
    }

    if let Some(h) = find_iso(lower) {
        hits.push(h);
    }
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
                return Some((i, i + 10, days_from_civil(y, m, d)));
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
                            return Some((start, end, days));
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
// Priority parsing
// ----------------------------------------------------------------------------

/// Priority cue → `(letter_index 0..=3, byte_range)`. 0 = A (highest) … 3 = D.
fn find_priority(lower: &str) -> Option<(usize, (usize, usize))> {
    // 1. Explicit `(A)`..`(Z)` anywhere — first wins, clamped to A..=D.
    let b = lower.as_bytes();
    let mut i = 0;
    while i + 2 < b.len() {
        if b[i] == b'(' && b[i + 1].is_ascii_alphabetic() && b[i + 2] == b')' {
            let letter = b[i + 1].to_ascii_uppercase();
            let idx = ((letter - b'A') as usize).min(3);
            return Some((idx, (i, i + 3)));
        }
        i += 1;
    }

    // 2. Todoist-style p1..p4.
    for (n, idx) in [("p1", 0usize), ("p2", 1), ("p3", 2), ("p4", 3)] {
        if let Some(r) = wb(lower, n) {
            return Some((idx, r));
        }
    }

    // 3. Keyword tiers (longest phrases first so they win the range).
    let high: &[&str] = &[
        "highest priority",
        "high priority",
        "top priority",
        "urgent",
        "asap",
        "critical",
        "important",
        "emergency",
    ];
    let low: &[&str] = &[
        "low priority",
        "low-priority",
        "someday",
        "whenever",
        "eventually",
        "minor",
    ];
    let mut best: Option<(usize, (usize, usize))> = None;
    for kw in high {
        if let Some(r) = wb(lower, kw) {
            match best {
                Some((_, (bs, _))) if bs <= r.0 => {}
                _ => best = Some((0, r)),
            }
        }
    }
    for kw in low {
        if let Some(r) = wb(lower, kw) {
            match best {
                Some((_, (bs, _))) if bs <= r.0 => {}
                _ => best = Some((2, r)),
            }
        }
    }
    best
}

// ----------------------------------------------------------------------------
// Assembly
// ----------------------------------------------------------------------------

/// Remove several byte ranges from `s`, leaving a separator so words don't fuse.
fn remove_ranges(s: &str, mut ranges: Vec<(usize, usize)>) -> String {
    ranges.sort_by_key(|r| r.0);
    let mut out = String::with_capacity(s.len());
    let mut cur = 0;
    for (a, b) in ranges {
        if a >= cur && a <= s.len() && b <= s.len() {
            out.push_str(&s[cur..a]);
            out.push(' ');
            cur = b;
        }
    }
    out.push_str(&s[cur.min(s.len())..]);
    out
}

/// Collapse whitespace and strip dangling connective words / punctuation left
/// after a date phrase was removed ("call bob by" → "call bob").
fn tidy(s: &str) -> String {
    let mut words: Vec<&str> = s.split_whitespace().collect();
    let is_connective = |w: &str| {
        let w = w
            .trim_matches(|c: char| !c.is_ascii_alphanumeric())
            .to_ascii_lowercase();
        matches!(
            w.as_str(),
            "by" | "on" | "due" | "before" | "at" | "this" | "next" | "the" | "of" | "end"
        )
    };
    while words.last().map(|w| is_connective(w)).unwrap_or(false) {
        words.pop();
    }
    words
        .join(" ")
        .trim_matches(|c: char| c == ',' || c == '-' || c == ':' || c == ';' || c.is_whitespace())
        .to_string()
}

/// Normalise a user-supplied project/context token: strip a leading sigil,
/// squeeze internal whitespace to `-`, keep it a single todo.txt word.
fn normalise_tag(raw: &str, sigil: char) -> Option<String> {
    let t = raw.trim().trim_start_matches(sigil).trim();
    if t.is_empty() {
        return None;
    }
    let joined = t.split_whitespace().collect::<Vec<_>>().join("-");
    Some(format!("{sigil}{joined}"))
}

/// Build one todo.txt line from one raw input line.
fn build_line(
    raw: &str,
    reference: i64,
    add_creation_date: bool,
    detect_priority: bool,
    detect_due: bool,
    default_project: &str,
    default_context: &str,
) -> String {
    let desc0 = strip_bullet(raw);
    let lower = desc0.to_lowercase();

    let mut ranges: Vec<(usize, usize)> = Vec::new();

    let mut due: Option<i64> = None;
    if detect_due {
        if let Some((s, e, days)) = find_date(&lower, reference) {
            due = Some(days);
            ranges.push((s, e));
        }
    }

    let mut prio: Option<usize> = None;
    if detect_priority {
        if let Some((idx, r)) = find_priority(&lower) {
            prio = Some(idx);
            ranges.push(r);
        }
    }

    let stripped = remove_ranges(&desc0, ranges);
    let mut description = tidy(&stripped);
    // If stripping emptied the description, fall back to the bullet-cleaned raw
    // text so we never emit a task with no words.
    if description.is_empty() {
        description = desc0.trim().to_string();
    }

    // Append a default project/context only if the description has none.
    let has_project = description
        .split_whitespace()
        .any(|w| w.starts_with('+') && w.len() > 1);
    let has_context = description
        .split_whitespace()
        .any(|w| w.starts_with('@') && w.len() > 1);
    if !has_project {
        if let Some(p) = normalise_tag(default_project, '+') {
            description.push(' ');
            description.push_str(&p);
        }
    }
    if !has_context {
        if let Some(c) = normalise_tag(default_context, '@') {
            description.push(' ');
            description.push_str(&c);
        }
    }

    let mut line = String::new();
    if let Some(idx) = prio {
        let letter = (b'A' + idx as u8) as char;
        line.push('(');
        line.push(letter);
        line.push_str(") ");
    }
    if add_creation_date {
        line.push_str(&fmt_date(reference));
        line.push(' ');
    }
    line.push_str(description.trim());
    if let Some(days) = due {
        if !line.ends_with(' ') {
            line.push(' ');
        }
        line.push_str("due:");
        line.push_str(&fmt_date(days));
    }
    line.trim().to_string()
}

/// Turn one or more plain-English task sentences (one per line) into todo.txt
/// lines. `reference_date` is an ISO `YYYY-MM-DD` anchor for relative phrases.
#[allow(clippy::too_many_arguments)]
pub fn to_todo_txt(
    text: &str,
    reference_date: &str,
    add_creation_date: bool,
    detect_priority: bool,
    detect_due: bool,
    default_project: &str,
    default_context: &str,
) -> Result<String, String> {
    let reference = parse_ref_date(reference_date)?;
    let lines: Vec<String> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            build_line(
                l,
                reference,
                add_creation_date,
                detect_priority,
                detect_due,
                default_project,
                default_context,
            )
        })
        .collect();
    if lines.is_empty() {
        return Err("no task text provided — enter at least one non-blank line".into());
    }
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const REF: &str = "2026-07-28"; // a Tuesday

    fn one(text: &str) -> String {
        to_todo_txt(text, REF, false, true, true, "", "").unwrap()
    }

    #[test]
    fn happy_priority_project_context_due() {
        // urgent → (A); +project/@context preserved; "tomorrow" → due tomorrow.
        assert_eq!(
            one("urgent call the plumber +house @phone tomorrow"),
            "(A) call the plumber +house @phone due:2026-07-29"
        );
    }

    #[test]
    fn next_friday_resolves_forward() {
        // 2026-07-28 is a Tuesday; the coming Friday is 2026-07-31.
        assert_eq!(one("submit report by next Friday"), "submit report due:2026-07-31");
    }

    #[test]
    fn iso_date_and_in_n_days() {
        assert_eq!(one("pay rent 2026-08-01"), "pay rent due:2026-08-01");
        assert_eq!(one("water plants in 3 days"), "water plants due:2026-07-31");
    }

    #[test]
    fn p_levels_map_to_letters() {
        assert_eq!(one("p1 ship release"), "(A) ship release");
        assert_eq!(one("p3 tidy desk"), "(C) tidy desk");
        assert_eq!(one("low priority read book"), "(C) read book");
    }

    #[test]
    fn creation_date_and_defaults() {
        let got = to_todo_txt("buy milk", REF, true, true, true, "Errands", "shop").unwrap();
        assert_eq!(got, "2026-07-28 buy milk +Errands @shop");
    }

    #[test]
    fn default_tag_skipped_when_present() {
        // Description already has a +project → the default project is not added.
        let got = to_todo_txt("email +Work team", REF, false, true, true, "Home", "").unwrap();
        assert_eq!(got, "email +Work team");
    }

    #[test]
    fn detect_due_off_keeps_date_text() {
        let got = to_todo_txt("call bob tomorrow", REF, false, true, false, "", "").unwrap();
        assert_eq!(got, "call bob tomorrow");
    }

    #[test]
    fn multiline_batch() {
        let got = one("urgent fix bug tomorrow\nbuy milk");
        assert_eq!(got, "(A) fix bug due:2026-07-29\nbuy milk");
    }

    #[test]
    fn multiword_default_tag_is_hyphenated() {
        let got =
            to_todo_txt("plan trip", REF, false, true, true, "+Summer Vacation", "@to do").unwrap();
        assert_eq!(got, "plan trip +Summer-Vacation @to-do");
    }

    #[test]
    fn err_on_blank_text() {
        let e = to_todo_txt("   \n  ", REF, false, true, true, "", "").unwrap_err();
        assert!(e.contains("no task text"), "got: {e}");
    }

    #[test]
    fn err_on_bad_reference_date() {
        let e = to_todo_txt("x", "2026-13-40", false, true, true, "", "").unwrap_err();
        assert!(e.contains("reference_date"), "got: {e}");
    }
}
