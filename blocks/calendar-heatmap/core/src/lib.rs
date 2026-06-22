//! gizza-ai/calendar-heatmap core — render date→value pairs as a GitHub-style
//! year contribution calendar (an SVG). Weeks are columns, the 7 weekdays are
//! rows; each day-cell is colored by its value bucketed into a 0..=4 intensity
//! scale (like a contribution graph). Pure-Rust, no deps — Gregorian date math
//! is done by hand so it instantiates everywhere (chat SW + page + CLI).

/// A parsed calendar day: ISO date components.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Day {
    y: i32,
    m: u32,
    d: u32,
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i32, m: u32) -> u32 {
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
        _ => 0,
    }
}

/// Days since proleptic-Gregorian year 0 (Jan 1 of year 0 = ordinal 0). Only used
/// as a monotonic ordinal for ordering/diffing dates and computing weekday — the
/// absolute value doesn't matter, only differences and `% 7`.
fn ordinal(d: Day) -> i64 {
    let y = d.y as i64;
    let leaps = {
        let ym1 = y - 1;
        ym1.div_euclid(4) - ym1.div_euclid(100) + ym1.div_euclid(400) + 1
    };
    let mut days = y * 365 + leaps;
    for mm in 1..d.m {
        days += days_in_month(d.y, mm) as i64;
    }
    days + (d.d as i64 - 1)
}

/// Weekday for a date: 0 = Sunday … 6 = Saturday. (1970-01-01 was a Thursday.)
fn weekday(d: Day) -> u32 {
    let epoch = ordinal(Day { y: 1970, m: 1, d: 1 });
    let diff = ordinal(d) - epoch;
    (((diff % 7 + 4) % 7 + 7) % 7) as u32
}

fn parse_date(tok: &str) -> Result<Day, String> {
    let t = tok.trim();
    let parts: Vec<&str> = t.split(['-', '/']).collect();
    if parts.len() != 3 {
        return Err(format!("'{t}' is not a YYYY-MM-DD date"));
    }
    let y: i32 = parts[0].parse().map_err(|_| format!("'{t}': bad year"))?;
    let m: u32 = parts[1].parse().map_err(|_| format!("'{t}': bad month"))?;
    let d: u32 = parts[2].parse().map_err(|_| format!("'{t}': bad day"))?;
    if !(1..=12).contains(&m) {
        return Err(format!("'{t}': month out of range"));
    }
    if d < 1 || d > days_in_month(y, m) {
        return Err(format!("'{t}': day out of range for that month"));
    }
    Ok(Day { y, m, d })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Five-step intensity color schemes (level 0 = empty/no data, 1..=4 ascending).
fn palette(scheme: &str) -> [&'static str; 5] {
    match scheme {
        "blue" => ["#ebedf0", "#c6dbef", "#6baed6", "#3182bd", "#08519c"],
        "purple" => ["#ebedf0", "#dadaeb", "#9e9ac8", "#756bb1", "#54278f"],
        "orange" => ["#ebedf0", "#fdd0a2", "#fdae6b", "#e6550d", "#a63603"],
        // default GitHub green
        _ => ["#ebedf0", "#9be9a8", "#40c463", "#30a14e", "#216e39"],
    }
}

struct Parsed {
    days: Vec<(Day, f64)>,
    min_ord: i64,
    max_ord: i64,
    max_val: f64,
}

/// Parse the date→value lines, summing duplicate dates. Each non-empty line is
/// `DATE` (value defaults to 1) or `DATE,VALUE` / `DATE VALUE` / `DATE\tVALUE`.
fn parse_data(data: &str) -> Result<Parsed, String> {
    use std::collections::BTreeMap;
    let mut map: BTreeMap<i64, (Day, f64)> = BTreeMap::new();
    let mut max_val = f64::NEG_INFINITY;
    let mut any = false;
    for (li, line) in data.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut it = line.splitn(2, [',', ' ', '\t']);
        let date_tok = it.next().unwrap_or("");
        let day = parse_date(date_tok).map_err(|e| format!("line {}: {e}", li + 1))?;
        let val: f64 = match it.next() {
            None => 1.0,
            Some(rest) => {
                let r = rest.trim();
                if r.is_empty() {
                    1.0
                } else {
                    r.parse()
                        .map_err(|_| format!("line {}: '{r}' is not a number", li + 1))?
                }
            }
        };
        if !val.is_finite() {
            return Err(format!("line {}: non-finite value", li + 1));
        }
        any = true;
        let ord = ordinal(day);
        let e = map.entry(ord).or_insert((day, 0.0));
        e.1 += val;
    }
    if !any {
        return Err("no date,value rows found".into());
    }
    for (_, v) in map.values() {
        max_val = max_val.max(*v);
    }
    let min_ord = *map.keys().next().unwrap();
    let max_ord = *map.keys().next_back().unwrap();
    let days: Vec<(Day, f64)> = map.into_values().collect();
    Ok(Parsed {
        days,
        min_ord,
        max_ord,
        max_val,
    })
}

/// Bucket a value into 0..=4. 0 = no contribution; positive values map to 1..=4
/// by quartile of the max.
fn level(v: f64, max: f64) -> usize {
    if v <= 0.0 {
        return 0;
    }
    if max <= 0.0 {
        return 1;
    }
    let frac = v / max;
    if frac <= 0.25 {
        1
    } else if frac <= 0.5 {
        2
    } else if frac <= 0.75 {
        3
    } else {
        4
    }
}

/// Walk `offset` days (forward, offset >= 0) from `start` using the calendar.
fn day_from_ord(start: Day, offset: i64) -> Day {
    let mut y = start.y;
    let mut m = start.m;
    let mut d = start.d as i64 + offset;
    loop {
        let dim = days_in_month(y, m) as i64;
        if d <= dim {
            break;
        }
        d -= dim;
        m += 1;
        if m > 12 {
            m = 1;
            y += 1;
        }
    }
    Day { y, m, d: d as u32 }
}

/// Walk `offset` days (may be negative) from `start` using the calendar.
fn day_from_ord_signed(start: Day, offset: i64) -> Day {
    if offset >= 0 {
        return day_from_ord(start, offset);
    }
    let mut y = start.y;
    let mut m = start.m;
    let mut d = start.d as i64 + offset; // negative
    loop {
        if d >= 1 {
            break;
        }
        m -= 1;
        if m < 1 {
            m = 12;
            y -= 1;
        }
        d += days_in_month(y, m) as i64;
    }
    Day { y, m, d: d as u32 }
}

/// Render the calendar heatmap SVG. `data` is date→value lines; `scheme` selects
/// the color ramp (green|blue|purple|orange); `start`/`end` (YYYY-MM-DD, optional)
/// override the date window; `title` is an optional heading. The grid spans from
/// the Sunday on/before the window start to the window end (full weeks, GitHub-style).
pub fn render_svg(
    data: &str,
    scheme: &str,
    start: &str,
    end: &str,
    title: &str,
) -> Result<String, String> {
    let parsed = parse_data(data)?;

    // Determine the window.
    let (mut win_start_ord, mut win_end_ord) = (parsed.min_ord, parsed.max_ord);
    if !start.trim().is_empty() {
        win_start_ord = ordinal(parse_date(start)?);
    }
    if !end.trim().is_empty() {
        win_end_ord = ordinal(parse_date(end)?);
    }
    if win_end_ord < win_start_ord {
        return Err("end date is before start date".into());
    }
    if win_end_ord - win_start_ord > 366 * 5 {
        return Err("date window too large (max ~5 years)".into());
    }

    use std::collections::HashMap;
    let by_ord: HashMap<i64, f64> =
        parsed.days.iter().map(|(d, v)| (ordinal(*d), *v)).collect();

    // Reconstruct a Day for win_start_ord by walking from the first data day.
    let base = parsed.days[0].0;
    let base_ord = ordinal(base);
    let start_day = day_from_ord_signed(base, win_start_ord - base_ord);
    let lead = weekday(start_day); // 0=Sun
    let grid_start_ord = win_start_ord - lead as i64;
    let grid_start_day = day_from_ord_signed(start_day, -(lead as i64));
    let total_days = win_end_ord - grid_start_ord + 1;
    let n_weeks = ((total_days + 6) / 7) as usize;

    let pal = palette(scheme);
    let cell = 13.0_f64;
    let gap = 3.0_f64;
    let step = cell + gap;
    let left_pad = 30.0_f64; // weekday labels
    let top_pad = if title.trim().is_empty() { 20.0 } else { 42.0 };
    let grid_x = left_pad;
    let grid_y = top_pad;
    let legend_h = 22.0_f64;
    let w = grid_x + n_weeks as f64 * step + 10.0;
    let h = grid_y + 7.0 * step + legend_h + 6.0;

    let mut svg = String::new();
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0}" height="{h:.0}" viewBox="0 0 {w:.0} {h:.0}" font-family="-apple-system,Segoe UI,Helvetica,Arial,sans-serif"><rect width="{w:.0}" height="{h:.0}" fill="#ffffff"/>"##
    ));

    if !title.trim().is_empty() {
        svg.push_str(&format!(
            r##"<text x="{x:.0}" y="26" text-anchor="middle" font-size="16" font-weight="bold" fill="#111">{t}</text>"##,
            x = w / 2.0,
            t = esc(title.trim())
        ));
    }

    // Weekday labels (Mon, Wed, Fri — rows 1,3,5, like GitHub).
    for (row, lab) in [(1u32, "Mon"), (3, "Wed"), (5, "Fri")] {
        let y = grid_y + row as f64 * step + cell - 2.0;
        svg.push_str(&format!(
            r##"<text x="{x:.0}" y="{y:.0}" text-anchor="end" font-size="9" fill="#767676">{lab}</text>"##,
            x = grid_x - 4.0,
        ));
    }

    let mut last_month: i32 = -1;
    for wk in 0..n_weeks {
        for dow in 0..7i64 {
            let off = wk as i64 * 7 + dow;
            let ord = grid_start_ord + off;
            if ord > win_end_ord {
                continue;
            }
            let day = day_from_ord_signed(grid_start_day, off);
            // Month label at the top when the column's first day enters a new month.
            if dow == 0 {
                let col_first = day_from_ord_signed(grid_start_day, wk as i64 * 7);
                if col_first.m as i32 != last_month {
                    last_month = col_first.m as i32;
                    let mx = grid_x + wk as f64 * step;
                    svg.push_str(&format!(
                        r##"<text x="{mx:.0}" y="{my:.0}" font-size="9" fill="#767676">{m}</text>"##,
                        my = grid_y - 5.0,
                        m = MONTH_NAMES[(col_first.m - 1) as usize]
                    ));
                }
            }
            let in_window = ord >= win_start_ord && ord <= win_end_ord;
            let v = by_ord.get(&ord).copied().unwrap_or(0.0);
            let lv = if in_window { level(v, parsed.max_val) } else { 0 };
            let x = grid_x + wk as f64 * step;
            let y = grid_y + dow as f64 * step;
            let fill = if in_window { pal[lv] } else { "#f6f6f6" };
            let date_str = format!("{:04}-{:02}-{:02}", day.y, day.m, day.d);
            let tip = if in_window {
                format!("{date_str}: {}", fmt_num(v))
            } else {
                date_str
            };
            svg.push_str(&format!(
                r##"<rect x="{x:.0}" y="{y:.0}" width="{cell:.0}" height="{cell:.0}" rx="2" fill="{fill}"><title>{tip}</title></rect>"##,
                tip = esc(&tip)
            ));
        }
    }

    // Legend: "Less [][][][][] More" bottom-right.
    let leg_y = grid_y + 7.0 * step + 4.0;
    let leg_x0 = (w - 10.0 - 5.0 * (cell + 2.0) - 70.0).max(grid_x);
    svg.push_str(&format!(
        r##"<text x="{x:.0}" y="{y:.0}" font-size="9" fill="#767676">Less</text>"##,
        x = leg_x0,
        y = leg_y + cell - 2.0
    ));
    for (i, c) in pal.iter().enumerate() {
        let x = leg_x0 + 26.0 + i as f64 * (cell + 2.0);
        svg.push_str(&format!(
            r##"<rect x="{x:.0}" y="{y:.0}" width="{cell:.0}" height="{cell:.0}" rx="2" fill="{c}"/>"##,
            y = leg_y
        ));
    }
    svg.push_str(&format!(
        r##"<text x="{x:.0}" y="{y:.0}" font-size="9" fill="#767676">More</text>"##,
        x = leg_x0 + 26.0 + 5.0 * (cell + 2.0) + 4.0,
        y = leg_y + cell - 2.0
    ));

    svg.push_str("</svg>");
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekday_known_dates() {
        assert_eq!(weekday(Day { y: 2024, m: 1, d: 1 }), 1); // Monday
        assert_eq!(weekday(Day { y: 2023, m: 1, d: 1 }), 0); // Sunday
        assert_eq!(weekday(Day { y: 1970, m: 1, d: 1 }), 4); // Thursday
    }

    #[test]
    fn leap_year_feb() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2023, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
    }

    #[test]
    fn ordinal_diffs_are_calendar_days() {
        let a = ordinal(Day { y: 2024, m: 1, d: 1 });
        let b = ordinal(Day { y: 2024, m: 12, d: 31 });
        assert_eq!(b - a, 365); // 2024 leap → 366 days, dec-31 is +365
        let c = ordinal(Day { y: 2025, m: 1, d: 1 });
        assert_eq!(c - a, 366);
    }

    #[test]
    fn day_from_ord_roundtrip() {
        let start = Day { y: 2024, m: 1, d: 1 };
        let d = day_from_ord_signed(start, 59); // +59 → 2024-02-29
        assert_eq!(d, Day { y: 2024, m: 2, d: 29 });
        let back = day_from_ord_signed(d, -59);
        assert_eq!(back, start);
        let prev = day_from_ord_signed(start, -1);
        assert_eq!(prev, Day { y: 2023, m: 12, d: 31 });
    }

    #[test]
    fn levels_bucket() {
        assert_eq!(level(0.0, 10.0), 0);
        assert_eq!(level(-5.0, 10.0), 0);
        assert_eq!(level(1.0, 10.0), 1);
        assert_eq!(level(3.0, 10.0), 2);
        assert_eq!(level(6.0, 10.0), 3);
        assert_eq!(level(10.0, 10.0), 4);
    }

    #[test]
    fn renders_basic_calendar() {
        let svg = render_svg(
            "2024-01-01,5\n2024-01-02,2\n2024-06-15,9\n2024-12-31",
            "green",
            "",
            "",
            "My Year",
        )
        .unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("My Year"));
        assert!(svg.contains("#216e39")); // green darkest for max (9)
        assert!(svg.contains(">Less<") && svg.contains(">More<"));
        assert!(svg.contains("2024-06-15: 9"));
    }

    #[test]
    fn duplicate_dates_sum() {
        let svg = render_svg("2024-03-10,2\n2024-03-10,3", "green", "", "", "").unwrap();
        assert!(svg.contains("2024-03-10: 5"));
    }

    #[test]
    fn value_defaults_to_one() {
        let svg = render_svg("2024-05-05", "green", "", "", "").unwrap();
        assert!(svg.contains("2024-05-05: 1"));
    }

    #[test]
    fn scheme_blue_used() {
        let svg = render_svg("2024-01-01,10", "blue", "", "", "").unwrap();
        assert!(svg.contains("#08519c")); // blue darkest
    }

    #[test]
    fn explicit_window_pads_full_weeks() {
        let svg = render_svg("2024-01-15,1", "green", "2024-01-01", "2024-01-31", "").unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">Jan<"));
    }

    #[test]
    fn errors() {
        assert!(render_svg("", "green", "", "", "").is_err());
        assert!(render_svg("not-a-date,1", "green", "", "", "").is_err());
        assert!(render_svg("2024-13-01,1", "green", "", "", "").is_err()); // bad month
        assert!(render_svg("2024-02-30,1", "green", "", "", "").is_err()); // bad day
        assert!(render_svg("2024-01-01,x", "green", "", "", "").is_err()); // bad value
        assert!(render_svg("2024-01-01,1", "green", "2024-06-01", "2024-01-01", "").is_err());
    }
}
