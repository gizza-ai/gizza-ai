//! spaced-repetition-scheduler core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Replays a batch review log (one row per card review) and reports, for every card, the
//! scheduler state it ends up in and the date it is next due. Two engines are available:
//!
//!   * `sm2` — the classic SuperMemo-2 state machine (repetition count, ease factor,
//!     interval), extended with the tuning knobs modern SM-2 decks expose: a configurable
//!     starting ease and ease floor, fixed first/second intervals, an easy bonus, a hard
//!     multiplier, a global interval modifier, a lapse multiplier and an interval cap.
//!
//!   * `fsrs` — an FSRS-style three-component memory model: difficulty (1–10), stability
//!     (days) and retrievability (0–1). This is an independent implementation of the
//!     published FSRS-6 formula shape with a built-in 21-number default weight vector; it
//!     is NOT a certified port of any app's build, so numbers can differ in the last digits
//!     from a specific release. Paste your own optimised vector into `fsrs_weights` to line
//!     it up with your own collection.
//!
//! Everything is deterministic: the same log and the same parameters always produce the same
//! schedule. There is deliberately no interval fuzz and no clock access — `today` defaults to
//! the latest review date found in the log.

use serde_json::{json, Map, Value};

/// Hard cap on parsed rows. Keeps a pasted export bounded.
pub const MAX_ROWS: usize = 5000;
/// Hard cap on distinct cards in one run.
pub const MAX_CARDS: usize = 2000;
/// Hard cap on projected reviews per card in `forecast` output.
pub const MAX_FORECAST: i64 = 50;

/// The built-in FSRS weight vector used when `fsrs_weights` is blank. Twenty-one numbers,
/// in the FSRS-6 order: four initial stabilities, two initial-difficulty terms, difficulty
/// change + mean reversion, the recall-stability terms, the forget-stability terms, the
/// hard/easy stability modifiers, the short-term terms, and the forgetting-curve decay.
pub const FSRS_DEFAULT_WEIGHTS: [f64; 21] = [
    0.2172, 1.1771, 3.2602, 16.1507, 7.0114, 0.57, 2.0966, 0.0069, 1.5261, 0.112, 1.0178, 1.849,
    0.1133, 0.3127, 2.2934, 0.2191, 3.0004, 0.7536, 0.3332, 0.1437, 0.2,
];

// ---------------------------------------------------------------------------
// calendar helpers (no clock, no chrono — days since 1970-01-01)
// ---------------------------------------------------------------------------

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
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

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// Parse `YYYY-MM-DD` (also accepted with `/` or `.` separators) into a day number.
fn parse_date(s: &str) -> Result<i64, String> {
    let t = s.trim();
    let norm: String = t
        .chars()
        .map(|c| if c == '/' || c == '.' { '-' } else { c })
        .collect();
    let parts: Vec<&str> = norm.split('-').collect();
    if parts.len() != 3 {
        return Err(format!("'{t}' is not a date — use YYYY-MM-DD"));
    }
    let nums: Result<Vec<i64>, _> = parts.iter().map(|p| p.trim().parse::<i64>()).collect();
    let nums = nums.map_err(|_| format!("'{t}' is not a date — use YYYY-MM-DD"))?;
    let (y, m, d) = (nums[0], nums[1], nums[2]);
    if parts[0].trim().len() != 4 || !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return Err(format!("'{t}' is not a valid calendar date — use YYYY-MM-DD"));
    }
    Ok(days_from_civil(y, m, d))
}

fn fmt_date(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{y:04}-{m:02}-{d:02}")
}

// ---------------------------------------------------------------------------
// options
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Algo {
    Sm2,
    Fsrs,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Scale {
    Auto,
    Sm2,
    Anki,
}

struct Config {
    algo: Algo,
    ease_start: f64,
    min_ease: f64,
    first_interval: f64,
    second_interval: f64,
    easy_bonus: f64,
    hard_multiplier: f64,
    interval_modifier: f64,
    lapse_multiplier: f64,
    max_interval: f64,
    desired_retention: f64,
    leech_threshold: i64,
    w: [f64; 21],
}

fn check_range(name: &str, v: f64, lo: f64, hi: f64) -> Result<f64, String> {
    if !v.is_finite() || v < lo || v > hi {
        return Err(format!("{name} must be between {lo} and {hi}"));
    }
    Ok(v)
}

fn parse_weights(s: &str) -> Result<[f64; 21], String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(FSRS_DEFAULT_WEIGHTS);
    }
    let cleaned: String = t
        .chars()
        .map(|c| if c == '[' || c == ']' { ' ' } else { c })
        .collect();
    let toks: Vec<&str> = cleaned
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    if toks.len() != 21 {
        return Err(format!(
            "fsrs_weights needs exactly 21 numbers, got {} — leave it blank to use the built-in vector",
            toks.len()
        ));
    }
    let mut w = [0.0f64; 21];
    for (i, tok) in toks.iter().enumerate() {
        w[i] = tok
            .parse::<f64>()
            .map_err(|_| format!("fsrs_weights entry {} ('{tok}') is not a number", i + 1))?;
        if !w[i].is_finite() {
            return Err(format!("fsrs_weights entry {} is not finite", i + 1));
        }
    }
    if w[20].abs() < 1e-6 {
        return Err("fsrs_weights entry 21 (decay) must not be zero".into());
    }
    Ok(w)
}

// ---------------------------------------------------------------------------
// grades
// ---------------------------------------------------------------------------

/// Word/letter aliases accepted in any grade scale. `q` is the SuperMemo 0–5 quality,
/// `rating` the four-button 1–4 rating.
fn word_grade(tok: &str) -> Option<(u8, u8)> {
    match tok {
        "again" | "a" | "fail" | "failed" | "wrong" | "forgot" | "no" | "lapse" => Some((0, 1)),
        "hard" | "h" | "difficult" => Some((3, 2)),
        "good" | "g" | "ok" | "okay" | "pass" | "yes" | "correct" => Some((4, 3)),
        "easy" | "e" | "perfect" => Some((5, 4)),
        _ => None,
    }
}

fn rating_name(rating: u8) -> &'static str {
    match rating {
        1 => "again",
        2 => "hard",
        3 => "good",
        _ => "easy",
    }
}

/// SuperMemo quality 0–5 → four-button rating 1–4.
fn q_to_rating(q: u8) -> u8 {
    match q {
        0 | 1 | 2 => 1,
        3 => 2,
        4 => 3,
        _ => 4,
    }
}

/// Anki-style rating 1–4 → SuperMemo quality. `again` is a lapse, `hard` still passes.
fn rating_to_q(rating: u8) -> u8 {
    match rating {
        1 => 0,
        2 => 3,
        3 => 4,
        _ => 5,
    }
}

// ---------------------------------------------------------------------------
// parsing the review log
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
struct Over {
    reps: Option<f64>,
    ease: Option<f64>,
    interval: Option<f64>,
    lapses: Option<f64>,
    difficulty: Option<f64>,
    stability: Option<f64>,
    last: Option<i64>,
}

impl Over {
    fn is_empty(&self) -> bool {
        self.reps.is_none()
            && self.ease.is_none()
            && self.interval.is_none()
            && self.lapses.is_none()
            && self.difficulty.is_none()
            && self.stability.is_none()
            && self.last.is_none()
    }
}

struct Row {
    day: i64,
    token: String,
    line: usize,
    over: Over,
}

struct Card {
    name: String,
    order: usize,
    rows: Vec<Row>,
}

const HEADER_WORDS: [&str; 18] = [
    "card",
    "cards",
    "name",
    "id",
    "card_id",
    "date",
    "day",
    "when",
    "reviewed",
    "grade",
    "rating",
    "quality",
    "score",
    "reps",
    "ease",
    "interval",
    "lapses",
    "difficulty",
];

/// Split one row into fields. The delimiter is whichever of tab / comma / semicolon /
/// pipe appears first in the line; a line with none of them splits on whitespace.
fn split_fields(line: &str) -> Vec<String> {
    let delim = if line.contains('\t') {
        Some('\t')
    } else if line.contains(',') {
        Some(',')
    } else if line.contains(';') {
        Some(';')
    } else if line.contains('|') {
        Some('|')
    } else {
        None
    };
    match delim {
        Some(d) => line.split(d).map(|f| f.trim().to_string()).collect(),
        None => line.split_whitespace().map(|f| f.to_string()).collect(),
    }
}

fn parse_num_field(key: &str, val: &str, line: usize) -> Result<f64, String> {
    val.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("line {line}: '{key}' must be a number, got '{val}'"))
}

fn parse_log(text: &str) -> Result<Vec<Card>, String> {
    let mut cards: Vec<Card> = Vec::new();
    let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut rows_seen = 0usize;
    let mut first_data_row = true;

    for (i, raw) in text.lines().enumerate() {
        let line_no = i + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let fields = split_fields(line);
        let mut positional: Vec<String> = Vec::new();
        let mut over = Over::default();
        for f in fields.iter().filter(|f| !f.is_empty()) {
            match f.split_once('=') {
                Some((k, v)) => {
                    let key = k.trim().to_ascii_lowercase();
                    let val = v.trim();
                    match key.as_str() {
                        "reps" | "repetitions" => over.reps = Some(parse_num_field("reps", val, line_no)?),
                        "ease" | "ef" | "ease_factor" => {
                            over.ease = Some(parse_num_field("ease", val, line_no)?)
                        }
                        "interval" | "ivl" => {
                            over.interval = Some(parse_num_field("interval", val, line_no)?)
                        }
                        "lapses" => over.lapses = Some(parse_num_field("lapses", val, line_no)?),
                        "difficulty" | "d" => {
                            over.difficulty = Some(parse_num_field("difficulty", val, line_no)?)
                        }
                        "stability" | "s" => {
                            over.stability = Some(parse_num_field("stability", val, line_no)?)
                        }
                        "last" | "last_review" | "previous" => {
                            over.last = Some(parse_date(val).map_err(|e| format!("line {line_no}: {e}"))?)
                        }
                        _ => {
                            return Err(format!(
                                "line {line_no}: unknown field '{key}' — supported state fields are \
                                 reps, ease, interval, lapses, difficulty, stability, last"
                            ))
                        }
                    }
                }
                None => positional.push(f.clone()),
            }
        }
        if positional.is_empty() {
            return Err(format!("line {line_no}: no card name on this row"));
        }

        // A leading header row (card,date,grade,…) is skipped, once.
        if first_data_row
            && positional.len() >= 2
            && positional
                .iter()
                .all(|p| HEADER_WORDS.contains(&p.to_ascii_lowercase().as_str()))
        {
            first_data_row = false;
            continue;
        }
        first_data_row = false;

        let name = positional[0].clone();
        let entry = match index.get(&name) {
            Some(&pos) => pos,
            None => {
                if cards.len() >= MAX_CARDS {
                    return Err(format!(
                        "more than {MAX_CARDS} distinct cards — split the log and schedule in batches"
                    ));
                }
                let pos = cards.len();
                index.insert(name.clone(), pos);
                cards.push(Card {
                    name,
                    order: pos,
                    rows: Vec::new(),
                });
                pos
            }
        };

        if positional.len() == 1 {
            if !over.is_empty() {
                return Err(format!(
                    "line {line_no}: state fields need a date and a grade on the same row"
                ));
            }
            continue; // a bare card name declares a new, never-reviewed card
        }
        if positional.len() == 2 {
            return Err(format!(
                "line {line_no}: expected card, date and grade (e.g. `{}, 2026-08-01, good`)",
                positional[0]
            ));
        }
        rows_seen += 1;
        if rows_seen > MAX_ROWS {
            return Err(format!(
                "more than {MAX_ROWS} review rows — split the log and schedule in batches"
            ));
        }
        let day = parse_date(&positional[1]).map_err(|e| format!("line {line_no}: {e}"))?;
        cards[entry].rows.push(Row {
            day,
            token: positional[2].to_ascii_lowercase(),
            line: line_no,
            over,
        });
    }

    if cards.is_empty() {
        return Err(
            "no review rows found — paste one row per review, e.g. `capital-of-peru, 2026-08-01, good`"
                .into(),
        );
    }
    for c in &mut cards {
        c.rows.sort_by_key(|r| r.day); // stable: same-day reviews keep log order
    }
    Ok(cards)
}

/// Resolve `auto`: any numeric `0` or `5` in the log means the SuperMemo 0–5 scale,
/// anything else is read as the four-button 1–4 scale.
fn resolve_scale(cards: &[Card], scale: Scale) -> Scale {
    if scale != Scale::Auto {
        return scale;
    }
    for c in cards {
        for r in &c.rows {
            if let Ok(n) = r.token.parse::<i64>() {
                if n == 0 || n == 5 {
                    return Scale::Sm2;
                }
            }
        }
    }
    Scale::Anki
}

/// One token → (SuperMemo quality 0–5, four-button rating 1–4).
fn grade_of(token: &str, scale: Scale, line: usize) -> Result<(u8, u8), String> {
    if let Some(g) = word_grade(token) {
        return Ok(g);
    }
    let n: i64 = token.parse().map_err(|_| {
        format!(
            "line {line}: '{token}' is not a grade — use again/hard/good/easy or a number"
        )
    })?;
    match scale {
        Scale::Sm2 => {
            if !(0..=5).contains(&n) {
                return Err(format!(
                    "line {line}: grade {n} is outside the SuperMemo 0–5 scale"
                ));
            }
            Ok((n as u8, q_to_rating(n as u8)))
        }
        _ => {
            if !(1..=4).contains(&n) {
                return Err(format!(
                    "line {line}: grade {n} is outside the four-button 1–4 scale \
                     (set grade_scale=sm2 for the 0–5 scale)"
                ));
            }
            Ok((rating_to_q(n as u8), n as u8))
        }
    }
}

// ---------------------------------------------------------------------------
// engines
// ---------------------------------------------------------------------------

struct State {
    reps: f64,
    ease: f64,
    interval: f64,
    lapses: f64,
    difficulty: f64,
    stability: f64,
    started: bool,
}

fn round_interval(v: f64, cfg: &Config) -> f64 {
    (v * cfg.interval_modifier).round().clamp(1.0, cfg.max_interval)
}

/// One SM-2 transition. `q` is the SuperMemo quality; the interval it leaves behind is a
/// whole number of days, exactly as a deck would store it.
fn step_sm2(st: &mut State, q: u8, cfg: &Config) {
    if q < 3 {
        st.lapses += 1.0;
        st.ease = (st.ease - 0.20).max(cfg.min_ease);
        st.reps = 0.0;
        let base = if cfg.lapse_multiplier > 0.0 {
            (st.interval * cfg.lapse_multiplier).max(1.0)
        } else {
            cfg.first_interval
        };
        st.interval = round_interval(base, cfg);
    } else {
        st.reps += 1.0;
        let qf = q as f64;
        st.ease = (st.ease + 0.1 - (5.0 - qf) * (0.08 + (5.0 - qf) * 0.02)).max(cfg.min_ease);
        let base = if st.reps <= 1.0 {
            cfg.first_interval
        } else if st.reps == 2.0 {
            cfg.second_interval
        } else if q == 3 {
            st.interval * cfg.hard_multiplier
        } else if q == 5 {
            st.interval * st.ease * cfg.easy_bonus
        } else {
            st.interval * st.ease
        };
        st.interval = round_interval(base, cfg);
    }
    st.started = true;
}

/// FSRS forgetting curve: R(t) = (1 + factor·t/S)^decay.
fn retrievability(elapsed: f64, stability: f64, w: &[f64; 21]) -> f64 {
    let decay = -w[20];
    let factor = 0.9f64.powf(1.0 / decay) - 1.0;
    (1.0 + factor * elapsed / stability.max(0.01)).powf(decay)
}

/// Days until the memory decays to `retention`.
fn fsrs_interval(stability: f64, retention: f64, w: &[f64; 21]) -> f64 {
    let decay = -w[20];
    let factor = 0.9f64.powf(1.0 / decay) - 1.0;
    stability / factor * (retention.powf(1.0 / decay) - 1.0)
}

fn fsrs_initial_difficulty(rating: u8, w: &[f64; 21]) -> f64 {
    (w[4] - (w[5] * (rating as f64 - 1.0)).exp() + 1.0).clamp(1.0, 10.0)
}

/// One FSRS transition. `elapsed` is whole days since the previous review of this card.
fn step_fsrs(st: &mut State, rating: u8, elapsed: f64, cfg: &Config) -> f64 {
    let w = &cfg.w;
    let g = rating as f64;
    if !st.started {
        st.stability = w[(rating - 1) as usize].max(0.01);
        st.difficulty = fsrs_initial_difficulty(rating, w);
        st.reps = 1.0;
        if rating == 1 {
            st.lapses += 1.0;
        }
        st.started = true;
        st.interval = round_interval(
            fsrs_interval(st.stability, cfg.desired_retention, w),
            cfg,
        );
        return 1.0;
    }
    let r = retrievability(elapsed.max(0.0), st.stability, w);
    let (d, s) = (st.difficulty, st.stability);
    let new_s = if elapsed <= 0.0 {
        // Same-day (short-term) review.
        s * (w[17] * (g - 3.0 + w[18])).exp() * s.powf(-w[19])
    } else if rating == 1 {
        let fail = w[11] * d.powf(-w[12]) * ((s + 1.0).powf(w[13]) - 1.0) * (w[14] * (1.0 - r)).exp();
        fail.min(s)
    } else {
        let hard = if rating == 2 { w[15] } else { 1.0 };
        let easy = if rating == 4 { w[16] } else { 1.0 };
        s * (1.0
            + w[8].exp() * (11.0 - d) * s.powf(-w[9]) * ((w[10] * (1.0 - r)).exp() - 1.0) * hard * easy)
    };
    // Difficulty: linear-damped change, then mean reversion toward the "easy" default.
    let delta = -w[6] * (g - 3.0);
    let d1 = d + delta * (10.0 - d) / 9.0;
    st.difficulty = (w[7] * fsrs_initial_difficulty(4, w) + (1.0 - w[7]) * d1).clamp(1.0, 10.0);
    st.stability = if new_s.is_finite() {
        new_s.clamp(0.01, 36500.0)
    } else {
        s
    };
    if rating == 1 {
        st.lapses += 1.0;
        st.reps = 0.0;
    } else {
        st.reps += 1.0;
    }
    st.interval = round_interval(
        fsrs_interval(st.stability, cfg.desired_retention, w),
        cfg,
    );
    r
}

// ---------------------------------------------------------------------------
// per-card result
// ---------------------------------------------------------------------------

struct Out {
    name: String,
    order: usize,
    reviews: usize,
    lapses: i64,
    reps: i64,
    last_day: Option<i64>,
    ease: Option<f64>,
    difficulty: Option<f64>,
    stability: Option<f64>,
    retrievability: Option<f64>,
    interval: i64,
    due: i64,
    days_until: i64,
    status: &'static str,
    steps: Vec<String>,
    state: State,
}

fn fmt2(v: f64) -> String {
    format!("{v:.2}")
}
fn fmt4(v: f64) -> String {
    format!("{v:.4}")
}
fn opt2(v: Option<f64>) -> String {
    v.map(fmt2).unwrap_or_default()
}
fn opt4(v: Option<f64>) -> String {
    v.map(fmt4).unwrap_or_default()
}
fn opt_date(v: Option<i64>) -> String {
    v.map(fmt_date).unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn simulate(card: &Card, scale: Scale, cfg: &Config, today: i64) -> Result<Out, String> {
    let mut st = State {
        reps: 0.0,
        ease: cfg.ease_start,
        interval: 0.0,
        lapses: 0.0,
        difficulty: 0.0,
        stability: 0.0,
        started: false,
    };
    let mut steps: Vec<String> = Vec::new();
    let mut last_day: Option<i64> = None;

    for row in &card.rows {
        let (q, rating) = grade_of(&row.token, scale, row.line)?;
        // State fields on a row seed the card as of that row, before its grade is applied.
        let o = &row.over;
        if let Some(v) = o.reps {
            st.reps = v.max(0.0);
            st.started = true;
        }
        if let Some(v) = o.ease {
            st.ease = v.max(cfg.min_ease);
        }
        if let Some(v) = o.interval {
            st.interval = v.max(0.0);
            st.started = true;
        }
        if let Some(v) = o.lapses {
            st.lapses = v.max(0.0);
        }
        if let Some(v) = o.difficulty {
            st.difficulty = v.clamp(1.0, 10.0);
            st.started = true;
        }
        if let Some(v) = o.stability {
            st.stability = v.max(0.01);
            st.started = true;
        }
        if let Some(v) = o.last {
            last_day = Some(v);
        }
        let elapsed = last_day.map(|l| (row.day - l) as f64).unwrap_or(0.0);
        let before = st.interval;

        match cfg.algo {
            Algo::Sm2 => {
                if q < 3 && !st.started {
                    // A first-ever "again" still counts as a lapse in the report.
                }
                step_sm2(&mut st, q, cfg);
                steps.push(format!(
                    "  {}  {:<5} q{}  elapsed {:<4} reps {:<3} ease {}  interval {}d -> {}d  due {}",
                    fmt_date(row.day),
                    rating_name(rating),
                    q,
                    if last_day.is_some() {
                        format!("{}d", elapsed as i64)
                    } else {
                        "-".to_string()
                    },
                    st.reps as i64,
                    fmt2(st.ease),
                    before as i64,
                    st.interval as i64,
                    fmt_date(row.day + st.interval as i64)
                ));
            }
            Algo::Fsrs => {
                let r = step_fsrs(&mut st, rating, elapsed, cfg);
                steps.push(format!(
                    "  {}  {:<5} r{}  elapsed {:<4} R {}  D {}  S {}  interval {}d  due {}",
                    fmt_date(row.day),
                    rating_name(rating),
                    rating,
                    if last_day.is_some() {
                        format!("{}d", elapsed as i64)
                    } else {
                        "-".to_string()
                    },
                    fmt4(r),
                    fmt2(st.difficulty),
                    fmt2(st.stability),
                    st.interval as i64,
                    fmt_date(row.day + st.interval as i64)
                ));
            }
        }
        last_day = Some(row.day);
    }

    let is_new = card.rows.is_empty();
    let interval = if is_new { 0 } else { st.interval as i64 };
    let due = match last_day {
        Some(l) => l + interval,
        None => today,
    };
    let days_until = due - today;
    let leech = cfg.leech_threshold > 0 && st.lapses as i64 >= cfg.leech_threshold;
    let status = if leech {
        "leech"
    } else if is_new {
        "new"
    } else if days_until < 0 {
        "overdue"
    } else if days_until == 0 {
        "due"
    } else {
        "scheduled"
    };
    let retr = match (cfg.algo, last_day) {
        (Algo::Fsrs, Some(l)) if st.started => Some(retrievability(
            (today - l).max(0) as f64,
            st.stability,
            &cfg.w,
        )),
        _ => None,
    };

    Ok(Out {
        name: card.name.clone(),
        order: card.order,
        reviews: card.rows.len(),
        lapses: st.lapses as i64,
        reps: st.reps as i64,
        last_day,
        ease: if cfg.algo == Algo::Sm2 {
            Some(st.ease)
        } else {
            None
        },
        difficulty: if cfg.algo == Algo::Fsrs && st.started {
            Some(st.difficulty)
        } else {
            None
        },
        stability: if cfg.algo == Algo::Fsrs && st.started {
            Some(st.stability)
        } else {
            None
        },
        retrievability: retr,
        interval,
        due,
        days_until,
        status,
        steps,
        state: st,
    })
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn headers(algo: Algo) -> Vec<&'static str> {
    match algo {
        Algo::Sm2 => vec![
            "card",
            "reviews",
            "lapses",
            "reps",
            "last_review",
            "ease",
            "interval_days",
            "due",
            "days_until",
            "status",
        ],
        Algo::Fsrs => vec![
            "card",
            "reviews",
            "lapses",
            "last_review",
            "difficulty",
            "stability",
            "retrievability",
            "interval_days",
            "due",
            "days_until",
            "status",
        ],
    }
}

fn row_cells(o: &Out, algo: Algo) -> Vec<String> {
    match algo {
        Algo::Sm2 => vec![
            o.name.clone(),
            o.reviews.to_string(),
            o.lapses.to_string(),
            o.reps.to_string(),
            opt_date(o.last_day),
            opt2(o.ease),
            o.interval.to_string(),
            fmt_date(o.due),
            o.days_until.to_string(),
            o.status.to_string(),
        ],
        Algo::Fsrs => vec![
            o.name.clone(),
            o.reviews.to_string(),
            o.lapses.to_string(),
            opt_date(o.last_day),
            opt2(o.difficulty),
            opt2(o.stability),
            opt4(o.retrievability),
            o.interval.to_string(),
            fmt_date(o.due),
            o.days_until.to_string(),
            o.status.to_string(),
        ],
    }
}

fn render_table(rows: &[Out], algo: Algo) -> String {
    let head = headers(algo);
    let mut grid: Vec<Vec<String>> = vec![head.iter().map(|h| h.to_string()).collect()];
    for o in rows {
        grid.push(row_cells(o, algo));
    }
    let cols = head.len();
    let mut width = vec![0usize; cols];
    for line in &grid {
        for (i, cell) in line.iter().enumerate() {
            width[i] = width[i].max(cell.chars().count());
        }
    }
    // Everything but the card name and the status reads as a number — right-align it.
    let right: Vec<bool> = (0..cols).map(|i| i != 0 && i != cols - 1).collect();
    let mut out = String::new();
    for line in &grid {
        let mut parts: Vec<String> = Vec::with_capacity(cols);
        for (i, cell) in line.iter().enumerate() {
            let pad = width[i] - cell.chars().count();
            parts.push(if right[i] {
                format!("{}{}", " ".repeat(pad), cell)
            } else {
                format!("{}{}", cell, " ".repeat(pad))
            });
        }
        out.push_str(parts.join("  ").trim_end());
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn render_csv(rows: &[Out], algo: Algo) -> String {
    let mut out = headers(algo).join(",");
    for o in rows {
        out.push('\n');
        out.push_str(
            &row_cells(o, algo)
                .iter()
                .map(|c| csv_cell(c))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    out
}

fn render_json(rows: &[Out], algo: Algo, scale: Scale, today: i64, cfg: &Config) -> String {
    let cards: Vec<Value> = rows
        .iter()
        .map(|o| {
            let mut m = Map::new();
            m.insert("card".into(), json!(o.name));
            m.insert("reviews".into(), json!(o.reviews));
            m.insert("lapses".into(), json!(o.lapses));
            m.insert(
                "last_review".into(),
                o.last_day.map(|d| json!(fmt_date(d))).unwrap_or(Value::Null),
            );
            match algo {
                Algo::Sm2 => {
                    m.insert("reps".into(), json!(o.reps));
                    m.insert(
                        "ease".into(),
                        o.ease
                            .map(|v| json!((v * 100.0).round() / 100.0))
                            .unwrap_or(Value::Null),
                    );
                }
                Algo::Fsrs => {
                    m.insert(
                        "difficulty".into(),
                        o.difficulty
                            .map(|v| json!((v * 100.0).round() / 100.0))
                            .unwrap_or(Value::Null),
                    );
                    m.insert(
                        "stability".into(),
                        o.stability
                            .map(|v| json!((v * 100.0).round() / 100.0))
                            .unwrap_or(Value::Null),
                    );
                    m.insert(
                        "retrievability".into(),
                        o.retrievability
                            .map(|v| json!((v * 10000.0).round() / 10000.0))
                            .unwrap_or(Value::Null),
                    );
                }
            }
            m.insert("interval_days".into(), json!(o.interval));
            m.insert("due".into(), json!(fmt_date(o.due)));
            m.insert("days_until".into(), json!(o.days_until));
            m.insert("status".into(), json!(o.status));
            Value::Object(m)
        })
        .collect();
    let doc = json!({
        "algorithm": if algo == Algo::Sm2 { "sm2" } else { "fsrs" },
        "grade_scale": if scale == Scale::Sm2 { "sm2" } else { "anki" },
        "today": fmt_date(today),
        "desired_retention": cfg.desired_retention,
        "count": cards.len(),
        "cards": cards,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".into())
}

fn render_explain(rows: &[Out], algo: Algo, today: i64) -> String {
    let mut out = String::new();
    for (i, o) in rows.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!(
            "{}  ({}, {} review{})\n",
            o.name,
            if algo == Algo::Sm2 { "sm2" } else { "fsrs" },
            o.reviews,
            if o.reviews == 1 { "" } else { "s" }
        ));
        if o.steps.is_empty() {
            out.push_str("  no reviews yet — new card\n");
        }
        for s in &o.steps {
            out.push_str(s);
            out.push('\n');
        }
        out.push_str(&format!(
            "  => due {} ({}), interval {}d, lapses {}, status {}\n",
            fmt_date(o.due),
            match o.days_until {
                0 => "today".to_string(),
                d if d < 0 => format!("{} day(s) overdue", -d),
                d => format!("in {d} day(s)"),
            },
            o.interval,
            o.lapses,
            o.status
        ));
    }
    let _ = today;
    out.trim_end().to_string()
}

fn render_forecast(
    rows: &[Out],
    algo: Algo,
    cfg: &Config,
    today: i64,
    n: i64,
    grade_token: &str,
) -> String {
    let (q, rating) = word_grade(grade_token)
        .unwrap_or((4, 3));
    let mut out = String::new();
    out.push_str(&format!(
        "Projected schedule assuming \"{}\" on every future review.\n",
        rating_name(rating)
    ));
    for o in rows {
        out.push('\n');
        let start = o.last_day.map(|l| l + o.interval).unwrap_or(today);
        out.push_str(&format!("{}  first projected review {}\n", o.name, fmt_date(start)));
        let mut st = State {
            reps: o.state.reps,
            ease: o.state.ease,
            interval: o.state.interval,
            lapses: o.state.lapses,
            difficulty: o.state.difficulty,
            stability: o.state.stability,
            started: o.state.started,
        };
        let mut last = o.last_day;
        let mut when = start;
        for k in 1..=n {
            let elapsed = last.map(|l| (when - l) as f64).unwrap_or(0.0);
            match algo {
                Algo::Sm2 => step_sm2(&mut st, q, cfg),
                Algo::Fsrs => {
                    step_fsrs(&mut st, rating, elapsed, cfg);
                }
            }
            out.push_str(&format!(
                "  {k:>2}  {}  interval {}d  next {}\n",
                fmt_date(when),
                st.interval as i64,
                fmt_date(when + st.interval as i64)
            ));
            last = Some(when);
            when += st.interval as i64;
        }
    }
    out.trim_end().to_string()
}

// ---------------------------------------------------------------------------
// entry point
// ---------------------------------------------------------------------------

/// Replay a review log and report the next review date for every card.
///
/// `reviews` is one row per review — `card, date, grade` — with optional trailing
/// `key=value` state fields. A row that is just a card name declares a new card.
#[allow(clippy::too_many_arguments)]
pub fn schedule(
    reviews: &str,
    algorithm: &str,
    grade_scale: &str,
    today: &str,
    output: &str,
    sort: &str,
    only_due: bool,
    desired_retention: f64,
    ease_start: f64,
    min_ease: f64,
    first_interval: f64,
    second_interval: f64,
    easy_bonus: f64,
    hard_multiplier: f64,
    interval_modifier: f64,
    lapse_multiplier: f64,
    max_interval: f64,
    leech_threshold: i64,
    forecast_reviews: i64,
    forecast_grade: &str,
    fsrs_weights: &str,
) -> Result<String, String> {
    let algo = match algorithm.trim().to_ascii_lowercase().as_str() {
        "" | "sm2" | "sm-2" | "supermemo2" => Algo::Sm2,
        "fsrs" => Algo::Fsrs,
        other => return Err(format!("algorithm must be sm2 or fsrs, got '{other}'")),
    };
    let scale_req = match grade_scale.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Scale::Auto,
        "sm2" => Scale::Sm2,
        "anki" => Scale::Anki,
        other => return Err(format!("grade_scale must be auto, sm2 or anki, got '{other}'")),
    };
    let output = output.trim().to_ascii_lowercase();
    let output = if output.is_empty() { "table".to_string() } else { output };
    if !["table", "csv", "json", "explain", "forecast"].contains(&output.as_str()) {
        return Err(format!(
            "output must be table, csv, json, explain or forecast, got '{output}'"
        ));
    }
    let sort = sort.trim().to_ascii_lowercase();
    let sort = if sort.is_empty() { "due".to_string() } else { sort };
    if !["due", "card", "interval", "lapses", "input"].contains(&sort.as_str()) {
        return Err(format!(
            "sort must be due, card, interval, lapses or input, got '{sort}'"
        ));
    }
    let forecast_grade_norm = forecast_grade.trim().to_ascii_lowercase();
    let forecast_grade_norm = if forecast_grade_norm.is_empty() {
        "good".to_string()
    } else {
        forecast_grade_norm
    };
    if word_grade(&forecast_grade_norm).is_none() {
        return Err(format!(
            "forecast_grade must be again, hard, good or easy, got '{forecast_grade_norm}'"
        ));
    }
    if !(1..=MAX_FORECAST).contains(&forecast_reviews) {
        return Err(format!(
            "forecast_reviews must be between 1 and {MAX_FORECAST}"
        ));
    }
    if !(0..=1000).contains(&leech_threshold) {
        return Err("leech_threshold must be between 0 and 1000".into());
    }

    let cfg = Config {
        algo,
        ease_start: check_range("ease_start", ease_start, 1.0, 10.0)?,
        min_ease: check_range("min_ease", min_ease, 1.0, 10.0)?,
        first_interval: check_range("first_interval", first_interval, 1.0, 365.0)?,
        second_interval: check_range("second_interval", second_interval, 1.0, 3650.0)?,
        easy_bonus: check_range("easy_bonus", easy_bonus, 1.0, 10.0)?,
        hard_multiplier: check_range("hard_multiplier", hard_multiplier, 0.1, 10.0)?,
        interval_modifier: check_range("interval_modifier", interval_modifier, 0.1, 10.0)?,
        lapse_multiplier: check_range("lapse_multiplier", lapse_multiplier, 0.0, 1.0)?,
        max_interval: check_range("max_interval", max_interval, 1.0, 36500.0)?,
        desired_retention: check_range("desired_retention", desired_retention, 0.70, 0.99)?,
        leech_threshold,
        w: parse_weights(fsrs_weights)?,
    };
    if cfg.min_ease > cfg.ease_start {
        return Err("min_ease must not be greater than ease_start".into());
    }

    let cards = parse_log(reviews)?;
    let scale = resolve_scale(&cards, scale_req);

    let today_day = if today.trim().is_empty() {
        cards
            .iter()
            .flat_map(|c| c.rows.iter().map(|r| r.day))
            .max()
            .ok_or("today is required when the log has no dated review rows")?
    } else {
        parse_date(today).map_err(|e| format!("today: {e}"))?
    };

    let mut rows: Vec<Out> = Vec::with_capacity(cards.len());
    for c in &cards {
        rows.push(simulate(c, scale, &cfg, today_day)?);
    }
    if only_due {
        rows.retain(|o| o.days_until <= 0);
    }
    match sort.as_str() {
        "card" => rows.sort_by(|a, b| a.name.cmp(&b.name)),
        "interval" => rows.sort_by(|a, b| a.interval.cmp(&b.interval).then(a.name.cmp(&b.name))),
        "lapses" => rows.sort_by(|a, b| b.lapses.cmp(&a.lapses).then(a.name.cmp(&b.name))),
        "input" => rows.sort_by_key(|a| a.order),
        _ => rows.sort_by(|a, b| a.due.cmp(&b.due).then(a.name.cmp(&b.name))),
    }
    if rows.is_empty() {
        return Ok(format!(
            "No cards are due on or before {}.",
            fmt_date(today_day)
        ));
    }

    Ok(match output.as_str() {
        "csv" => render_csv(&rows, algo),
        "json" => render_json(&rows, algo, scale, today_day, &cfg),
        "explain" => render_explain(&rows, algo, today_day),
        "forecast" => render_forecast(
            &rows,
            algo,
            &cfg,
            today_day,
            forecast_reviews,
            &forecast_grade_norm,
        ),
        _ => render_table(&rows, algo),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Defaults matching the descriptor, so a test only names what it changes.
    #[allow(clippy::too_many_arguments)]
    fn run(reviews: &str, algorithm: &str, output: &str) -> Result<String, String> {
        schedule(
            reviews, algorithm, "auto", "", output, "due", false, 0.9, 2.5, 1.3, 1.0, 6.0, 1.3,
            1.2, 1.0, 0.0, 36500.0, 8, 6, "good", "",
        )
    }

    #[test]
    fn sm2_happy_path_first_two_reviews_use_the_fixed_intervals() {
        // Two "good" reviews: interval 1 then 6, ease unchanged at 2.50.
        let out = run(
            "capital-of-peru, 2026-08-01, good\ncapital-of-peru, 2026-08-02, good",
            "sm2",
            "csv",
        )
        .unwrap();
        assert_eq!(
            out,
            "card,reviews,lapses,reps,last_review,ease,interval_days,due,days_until,status\n\
             capital-of-peru,2,0,2,2026-08-02,2.50,6,2026-08-08,6,scheduled"
        );
    }

    #[test]
    fn sm2_third_good_review_multiplies_by_ease() {
        // 6 days * 2.5 = 15.
        let out = run(
            "c, 2026-08-01, good\nc, 2026-08-02, good\nc, 2026-08-08, good",
            "sm2",
            "csv",
        )
        .unwrap();
        assert!(out.contains(",2.50,15,2026-08-23,"), "{out}");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = run("   \n\n", "sm2", "table").unwrap_err();
        assert!(err.contains("no review rows found"), "{err}");
    }

    #[test]
    fn unparseable_date_names_the_line() {
        let err = run("c, 2026-13-01, good", "sm2", "table").unwrap_err();
        assert!(err.contains("line 1"), "{err}");
        assert!(err.contains("not a valid calendar date"), "{err}");
    }

    #[test]
    fn unparseable_grade_names_the_line() {
        let err = run("c, 2026-08-01, splendid", "sm2", "table").unwrap_err();
        assert!(err.contains("line 1") && err.contains("is not a grade"), "{err}");
    }

    #[test]
    fn anki_words_letters_and_numbers_agree() {
        let words = run("c, 2026-08-01, again\nc, 2026-08-02, good", "sm2", "csv").unwrap();
        let letters = run("c, 2026-08-01, a\nc, 2026-08-02, g", "sm2", "csv").unwrap();
        let numbers = run("c, 2026-08-01, 1\nc, 2026-08-02, 3", "sm2", "csv").unwrap();
        assert_eq!(words, letters);
        assert_eq!(words, numbers);
        assert!(words.contains(",1,"), "one lapse recorded: {words}");
    }

    #[test]
    fn auto_scale_switches_to_supermemo_when_a_zero_or_five_appears() {
        // A 5 forces the 0–5 reading, where 5 = easy.
        let out = run("c, 2026-08-01, 5", "sm2", "csv").unwrap();
        assert!(out.contains(",2.60,1,"), "ease rises to 2.60 on q5: {out}");
        // Without a 0/5 the same log is read as the four-button scale, where 4 = easy.
        let out = run("c, 2026-08-01, 4", "sm2", "csv").unwrap();
        assert!(out.contains(",2.60,1,"), "{out}");
        // Forcing the SuperMemo scale reads 4 as "good" — ease stays put.
        let out = schedule(
            "c, 2026-08-01, 4", "sm2", "sm2", "", "csv", "due", false, 0.9, 2.5, 1.3, 1.0, 6.0,
            1.3, 1.2, 1.0, 0.0, 36500.0, 8, 6, "good", "",
        )
        .unwrap();
        assert!(out.contains(",2.50,1,"), "{out}");
    }

    #[test]
    fn sm2_grade_out_of_range_is_rejected_per_scale() {
        let err = schedule(
            "c, 2026-08-01, 7", "sm2", "sm2", "", "csv", "due", false, 0.9, 2.5, 1.3, 1.0, 6.0,
            1.3, 1.2, 1.0, 0.0, 36500.0, 8, 6, "good", "",
        )
        .unwrap_err();
        assert!(err.contains("SuperMemo 0–5 scale"), "{err}");
        let err = schedule(
            "c, 2026-08-01, 9", "sm2", "anki", "", "csv", "due", false, 0.9, 2.5, 1.3, 1.0, 6.0,
            1.3, 1.2, 1.0, 0.0, 36500.0, 8, 6, "good", "",
        )
        .unwrap_err();
        assert!(err.contains("four-button 1–4 scale"), "{err}");
    }

    #[test]
    fn lapse_restarts_the_interval_and_drops_the_ease() {
        let out = run(
            "c, 2026-08-01, good\nc, 2026-08-02, good\nc, 2026-08-08, again",
            "sm2",
            "csv",
        )
        .unwrap();
        assert!(out.contains(",1,0,2026-08-08,2.30,1,2026-08-09,"), "{out}");
    }

    #[test]
    fn lapse_multiplier_keeps_a_share_of_the_old_interval() {
        let out = schedule(
            "c, 2026-08-01, good\nc, 2026-08-02, good\nc, 2026-08-08, good\nc, 2026-08-23, again",
            "sm2", "auto", "", "csv", "due", false, 0.9, 2.5, 1.3, 1.0, 6.0, 1.3, 1.2, 1.0, 0.5,
            36500.0, 8, 6, "good", "",
        )
        .unwrap();
        // 15-day interval * 0.5 = 8 (rounded), instead of a restart at 1.
        assert!(out.contains(",8,2026-08-31,"), "{out}");
    }

    #[test]
    fn max_interval_caps_a_long_schedule() {
        let log = "c, 2026-08-01, easy\nc, 2026-08-02, easy\nc, 2026-08-08, easy\nc, 2026-09-01, easy";
        let out = schedule(
            log, "sm2", "auto", "", "csv", "due", false, 0.9, 2.5, 1.3, 1.0, 6.0, 1.3, 1.2, 1.0,
            0.0, 21.0, 8, 6, "good", "",
        )
        .unwrap();
        assert!(out.contains(",21,2026-09-22,"), "capped at 21 days: {out}");
    }

    #[test]
    fn leech_threshold_flags_a_repeatedly_failed_card() {
        let log = (1..=4)
            .map(|d| format!("c, 2026-08-0{d}, again"))
            .collect::<Vec<_>>()
            .join("\n");
        let out = schedule(
            &log, "sm2", "auto", "", "csv", "due", false, 0.9, 2.5, 1.3, 1.0, 6.0, 1.3, 1.2, 1.0,
            0.0, 36500.0, 3, 6, "good", "",
        )
        .unwrap();
        assert!(out.trim_end().ends_with(",leech"), "{out}");
        // Threshold 0 turns leech detection off.
        let out = schedule(
            &log, "sm2", "auto", "", "csv", "due", false, 0.9, 2.5, 1.3, 1.0, 6.0, 1.3, 1.2, 1.0,
            0.0, 36500.0, 0, 6, "good", "",
        )
        .unwrap();
        assert!(!out.contains("leech"), "{out}");
    }

    #[test]
    fn a_bare_card_name_declares_a_new_card_due_today() {
        let out = schedule(
            "old, 2026-08-01, good\nbrand-new", "sm2", "auto", "2026-08-05", "csv", "card", false,
            0.9, 2.5, 1.3, 1.0, 6.0, 1.3, 1.2, 1.0, 0.0, 36500.0, 8, 6, "good", "",
        )
        .unwrap();
        assert!(out.contains("brand-new,0,0,0,,2.50,0,2026-08-05,0,new"), "{out}");
    }

    #[test]
    fn only_due_hides_cards_scheduled_into_the_future() {
        // `later` reaches the 6-day second interval; `soon` lapses back to 1 day.
        let log = "soon, 2026-08-01, again\nlater, 2026-08-01, good\nlater, 2026-08-01, good";
        let out = schedule(
            log, "sm2", "auto", "2026-08-02", "csv", "card", true, 0.9, 2.5, 1.3, 1.0, 6.0, 1.3,
            1.2, 1.0, 0.0, 36500.0, 8, 6, "good", "",
        )
        .unwrap();
        assert!(out.contains("soon,"), "{out}");
        assert!(!out.contains("later,"), "{out}");
    }

    #[test]
    fn only_due_with_nothing_due_explains_itself() {
        let out = schedule(
            "c, 2026-08-01, good", "sm2", "auto", "2026-07-01", "csv", "due", true, 0.9, 2.5, 1.3,
            1.0, 6.0, 1.3, 1.2, 1.0, 0.0, 36500.0, 8, 6, "good", "",
        )
        .unwrap();
        assert_eq!(out, "No cards are due on or before 2026-07-01.");
    }

    #[test]
    fn today_defaults_to_the_latest_review_date_in_the_log() {
        let out = run("c, 2026-08-01, good", "sm2", "json").unwrap();
        assert!(out.contains("\"today\": \"2026-08-01\""), "{out}");
    }

    #[test]
    fn state_fields_seed_a_card_without_a_full_history() {
        // Pasted current state: 4 reps, ease 2.3, 21-day interval, reviewed 21 days ago.
        let out = schedule(
            "algebra, 2026-08-22, good, reps=4, ease=2.3, interval=21, last=2026-08-01",
            "sm2", "auto", "", "csv", "due", false, 0.9, 2.5, 1.3, 1.0, 6.0, 1.3, 1.2, 1.0, 0.0,
            36500.0, 8, 6, "good", "",
        )
        .unwrap();
        // 21 * 2.30 = 48.3 -> 48 days.
        assert!(out.contains(",2.30,48,2026-10-09,"), "{out}");
    }

    #[test]
    fn unknown_state_field_is_rejected() {
        let err = run("c, 2026-08-01, good, banana=3", "sm2", "csv").unwrap_err();
        assert!(err.contains("unknown field 'banana'"), "{err}");
    }

    #[test]
    fn header_row_and_comments_are_skipped() {
        let out = run(
            "card,date,grade\n# my deck\nc, 2026-08-01, good",
            "sm2",
            "csv",
        )
        .unwrap();
        assert!(out.contains("\nc,1,"), "{out}");
        assert!(!out.contains("\ncard,1,"), "{out}");
    }

    #[test]
    fn tab_and_semicolon_separated_rows_parse_the_same_as_csv() {
        let csv = run("c, 2026-08-01, good", "sm2", "csv").unwrap();
        assert_eq!(run("c\t2026-08-01\tgood", "sm2", "csv").unwrap(), csv);
        assert_eq!(run("c; 2026-08-01; good", "sm2", "csv").unwrap(), csv);
        assert_eq!(run("c 2026-08-01 good", "sm2", "csv").unwrap(), csv);
    }

    #[test]
    fn fsrs_first_good_review_uses_the_default_initial_stability() {
        let out = run("c, 2026-08-01, good", "fsrs", "csv").unwrap();
        assert_eq!(
            out,
            "card,reviews,lapses,last_review,difficulty,stability,retrievability,interval_days,due,days_until,status\n\
             c,1,0,2026-08-01,4.88,3.26,1.0000,3,2026-08-04,3,scheduled"
        );
    }

    #[test]
    fn fsrs_interval_equals_stability_at_ninety_percent_retention() {
        // The forgetting curve is defined so R = 0.9 exactly one stability out.
        let w = FSRS_DEFAULT_WEIGHTS;
        let ivl = fsrs_interval(10.0, 0.9, &w);
        assert!((ivl - 10.0).abs() < 1e-6, "{ivl}");
        let r = retrievability(10.0, 10.0, &w);
        assert!((r - 0.9).abs() < 1e-6, "{r}");
    }

    #[test]
    fn fsrs_lower_desired_retention_lengthens_the_interval() {
        let tight = schedule(
            "c, 2026-08-01, good", "fsrs", "auto", "", "csv", "due", false, 0.95, 2.5, 1.3, 1.0,
            6.0, 1.3, 1.2, 1.0, 0.0, 36500.0, 8, 6, "good", "",
        )
        .unwrap();
        let loose = schedule(
            "c, 2026-08-01, good", "fsrs", "auto", "", "csv", "due", false, 0.80, 2.5, 1.3, 1.0,
            6.0, 1.3, 1.2, 1.0, 0.0, 36500.0, 8, 6, "good", "",
        )
        .unwrap();
        assert!(tight.contains(",1,2026-08-02,"), "{tight}");
        assert!(loose.contains(",10,2026-08-11,"), "{loose}");
    }

    #[test]
    fn fsrs_again_lowers_stability_and_records_a_lapse() {
        let out = run(
            "c, 2026-08-01, good\nc, 2026-08-04, again",
            "fsrs",
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["cards"][0]["lapses"], 1);
        let s = v["cards"][0]["stability"].as_f64().unwrap();
        assert!(s < 3.26, "stability drops after a lapse: {s}");
    }

    #[test]
    fn fsrs_same_day_second_review_uses_the_short_term_formula() {
        let out = run(
            "c, 2026-08-01, good\nc, 2026-08-01, good",
            "fsrs",
            "csv",
        )
        .unwrap();
        // Two same-day "good" reviews leave stability above a single one.
        assert!(out.contains(",2,0,2026-08-01,"), "{out}");
        let cells: Vec<&str> = out.lines().nth(1).unwrap().split(',').collect();
        assert!(cells[5].parse::<f64>().unwrap() > 3.26, "{out}");
    }

    #[test]
    fn custom_fsrs_weights_are_accepted_and_validated() {
        let mut w = FSRS_DEFAULT_WEIGHTS;
        w[2] = 9.0; // initial stability for "good"
        let vector = w
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let out = schedule(
            "c, 2026-08-01, good", "fsrs", "auto", "", "csv", "due", false, 0.9, 2.5, 1.3, 1.0,
            6.0, 1.3, 1.2, 1.0, 0.0, 36500.0, 8, 6, "good", &vector,
        )
        .unwrap();
        assert!(out.contains(",9.00,1.0000,9,2026-08-10,"), "{out}");

        let err = schedule(
            "c, 2026-08-01, good", "fsrs", "auto", "", "csv", "due", false, 0.9, 2.5, 1.3, 1.0,
            6.0, 1.3, 1.2, 1.0, 0.0, 36500.0, 8, 6, "good", "1, 2, 3",
        )
        .unwrap_err();
        assert!(err.contains("exactly 21 numbers, got 3"), "{err}");
    }

    #[test]
    fn explain_output_shows_every_state_transition() {
        let out = run(
            "c, 2026-08-01, good\nc, 2026-08-02, hard",
            "sm2",
            "explain",
        )
        .unwrap();
        assert!(out.starts_with("c  (sm2, 2 reviews)"), "{out}");
        assert!(out.contains("good  q4"), "{out}");
        assert!(out.contains("hard  q3"), "{out}");
        assert!(out.contains("interval 1d -> 6d"), "{out}");
        assert!(out.contains("=> due"), "{out}");
    }

    #[test]
    fn forecast_projects_the_requested_number_of_reviews() {
        let out = schedule(
            "c, 2026-08-01, good\nc, 2026-08-02, good", "sm2", "auto", "", "forecast", "due",
            false, 0.9, 2.5, 1.3, 1.0, 6.0, 1.3, 1.2, 1.0, 0.0, 36500.0, 8, 3, "good", "",
        )
        .unwrap();
        assert!(out.contains("assuming \"good\""), "{out}");
        assert_eq!(out.lines().filter(|l| l.contains("interval")).count(), 3);
        assert!(out.contains("  1  2026-08-08  interval 15d  next 2026-08-23"), "{out}");
    }

    #[test]
    fn table_output_is_aligned_and_headed() {
        let out = run("alpha, 2026-08-01, good\nbeta, 2026-08-01, easy", "sm2", "table").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].starts_with("card "), "{out}");
        assert!(lines[0].contains("days_until"), "{out}");
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn sort_orders_are_distinct_and_deterministic() {
        let log = "zebra, 2026-08-01, easy\nalpha, 2026-08-01, again";
        let by_card = schedule(
            log, "sm2", "auto", "", "csv", "card", false, 0.9, 2.5, 1.3, 1.0, 6.0, 1.3, 1.2, 1.0,
            0.0, 36500.0, 8, 6, "good", "",
        )
        .unwrap();
        assert!(by_card.lines().nth(1).unwrap().starts_with("alpha"), "{by_card}");
        let by_input = schedule(
            log, "sm2", "auto", "", "csv", "input", false, 0.9, 2.5, 1.3, 1.0, 6.0, 1.3, 1.2, 1.0,
            0.0, 36500.0, 8, 6, "good", "",
        )
        .unwrap();
        assert!(by_input.lines().nth(1).unwrap().starts_with("zebra"), "{by_input}");
        let by_lapses = schedule(
            log, "sm2", "auto", "", "csv", "lapses", false, 0.9, 2.5, 1.3, 1.0, 6.0, 1.3, 1.2,
            1.0, 0.0, 36500.0, 8, 6, "good", "",
        )
        .unwrap();
        assert!(by_lapses.lines().nth(1).unwrap().starts_with("alpha"), "{by_lapses}");
        let by_interval = schedule(
            log, "sm2", "auto", "", "csv", "interval", false, 0.9, 2.5, 1.3, 1.0, 6.0, 1.3, 1.2,
            1.0, 0.0, 36500.0, 8, 6, "good", "",
        )
        .unwrap();
        assert!(by_interval.lines().nth(1).unwrap().starts_with("alpha"), "{by_interval}");
    }

    #[test]
    fn rows_out_of_order_are_replayed_by_date() {
        let forward = run("c, 2026-08-01, good\nc, 2026-08-02, good", "sm2", "csv").unwrap();
        let shuffled = run("c, 2026-08-02, good\nc, 2026-08-01, good", "sm2", "csv").unwrap();
        assert_eq!(forward, shuffled);
    }

    #[test]
    fn numeric_parameters_are_range_checked() {
        let bad = schedule(
            "c, 2026-08-01, good", "sm2", "auto", "", "csv", "due", false, 0.5, 2.5, 1.3, 1.0,
            6.0, 1.3, 1.2, 1.0, 0.0, 36500.0, 8, 6, "good", "",
        )
        .unwrap_err();
        assert!(bad.contains("desired_retention must be between 0.7 and 0.99"), "{bad}");
        let bad = schedule(
            "c, 2026-08-01, good", "sm2", "auto", "", "csv", "due", false, 0.9, 2.5, 2.9, 1.0,
            6.0, 1.3, 1.2, 1.0, 0.0, 36500.0, 8, 6, "good", "",
        )
        .unwrap_err();
        assert!(bad.contains("min_ease must not be greater than ease_start"), "{bad}");
        let bad = schedule(
            "c, 2026-08-01, good", "sm2", "auto", "", "csv", "due", false, 0.9, 2.5, 1.3, 1.0,
            6.0, 1.3, 1.2, 1.0, 0.0, 36500.0, 8, 99, "good", "",
        )
        .unwrap_err();
        assert!(bad.contains("forecast_reviews must be between 1 and 50"), "{bad}");
    }

    #[test]
    fn unknown_enum_values_are_rejected() {
        assert!(run("c, 2026-08-01, good", "sm3", "csv")
            .unwrap_err()
            .contains("algorithm must be sm2 or fsrs"));
        assert!(run("c, 2026-08-01, good", "sm2", "yaml")
            .unwrap_err()
            .contains("output must be table"));
    }

    #[test]
    fn row_limits_are_enforced() {
        let log = (0..=MAX_ROWS)
            .map(|i| format!("c{i}, 2026-08-01, good"))
            .collect::<Vec<_>>()
            .join("\n");
        let err = run(&log, "sm2", "csv").unwrap_err();
        assert!(err.contains("cards") || err.contains("review rows"), "{err}");
    }

    #[test]
    fn card_names_containing_commas_round_trip_through_csv() {
        let out = run("c\t2026-08-01\tgood", "sm2", "csv").unwrap();
        assert!(out.contains("\nc,1,"), "{out}");
        let out = run("a, b\t2026-08-01\tgood", "sm2", "csv").unwrap();
        assert!(out.contains("\"a, b\""), "{out}");
    }
}
