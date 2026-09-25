//! Pure Elo rating maths: expected scores, rating deltas and post-match ratings for a
//! two-sided match (or a series of games between the same two opponents).
//!
//! Deterministic, allocation-light, no I/O — the same core backs the chat block, the CLI
//! and the browser page.

/// How the result is rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable report: both players, expectancies, formula substitution, scenarios.
    Summary,
    /// Only the win/draw/loss (or all-wins/all-draws/all-losses) what-if block.
    Scenarios,
    /// Machine-readable object with every intermediate value.
    Json,
    /// One header row plus one row per player.
    Csv,
    /// The bare signed rating change for player A, e.g. `+12`.
    Delta,
}

pub fn parse_output_format(s: &str) -> Result<OutputFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "summary" => Ok(OutputFormat::Summary),
        "scenarios" => Ok(OutputFormat::Scenarios),
        "json" => Ok(OutputFormat::Json),
        "csv" => Ok(OutputFormat::Csv),
        "delta" => Ok(OutputFormat::Delta),
        other => Err(format!(
            "output_format must be one of summary, scenarios, json, csv, delta; got '{other}'"
        )),
    }
}

pub const MIN_RATING: f64 = 0.0;
pub const MAX_RATING: f64 = 5000.0;
pub const MAX_GAMES: u64 = 1000;
pub const MIN_K: f64 = 0.1;
pub const MAX_K: f64 = 200.0;
pub const MAX_DECIMALS: u64 = 6;
pub const MAX_NAME_LEN: usize = 60;

/// Everything computed for one side of the match.
#[derive(Clone, Debug, PartialEq)]
pub struct Side {
    pub name: String,
    pub rating_before: f64,
    pub expected_score: f64,
    pub expected_total: f64,
    pub score: f64,
    pub k_factor: f64,
    pub rating_change_raw: f64,
    pub rating_change: f64,
    pub rating_after: f64,
}

/// The full outcome of one Elo update.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub a: Side,
    pub b: Side,
    pub games: u64,
    pub rating_difference: f64,
    pub effective_difference: f64,
    pub capped: bool,
    pub decimals: u64,
}

/// Classical Elo expectancy for a player rated `rating` against `opponent`.
pub fn expected_score(rating: f64, opponent: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf((opponent - rating) / 400.0))
}

fn round_to(value: f64, decimals: u64) -> f64 {
    let factor = 10f64.powi(decimals as i32);
    (value * factor).round() / factor
}

fn check_rating(label: &str, value: f64) -> Result<(), String> {
    if !value.is_finite() {
        return Err(format!("{label} must be a number, got '{value}'"));
    }
    if value < MIN_RATING || value > MAX_RATING {
        return Err(format!(
            "{label} must be between {MIN_RATING:.0} and {MAX_RATING:.0}, got {}",
            trim_num(value)
        ));
    }
    Ok(())
}

fn check_k(label: &str, value: f64) -> Result<(), String> {
    if !value.is_finite() {
        return Err(format!("{label} must be a number, got '{value}'"));
    }
    if value < MIN_K || value > MAX_K {
        return Err(format!(
            "{label} must be between {MIN_K} and {MAX_K:.0} (FIDE uses 10, 20 or 40; Chess.com uses about 32), got {}",
            trim_num(value)
        ));
    }
    Ok(())
}

fn clean_name(label: &str, raw: &str, fallback: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(fallback.to_string());
    }
    if trimmed.chars().count() > MAX_NAME_LEN {
        return Err(format!(
            "{label} must be {MAX_NAME_LEN} characters or fewer, got {}",
            trimmed.chars().count()
        ));
    }
    Ok(trimmed.replace(['\r', '\n', '\t'], " "))
}

/// Format a number with trailing zeros trimmed (max 6 decimals) — used for ratings the
/// user typed and for scores, so `1600` stays `1600` and `2.5` stays `2.5`.
pub fn trim_num(value: f64) -> String {
    let mut s = format!("{:.6}", value);
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

fn fmt_fixed(value: f64, decimals: u64) -> String {
    let rounded = round_to(value, decimals);
    let s = format!("{:.*}", decimals as usize, rounded);
    if s.starts_with("-0") && rounded == 0.0 {
        s[1..].to_string()
    } else {
        s
    }
}

fn fmt_signed(value: f64, decimals: u64) -> String {
    let rounded = round_to(value, decimals);
    if rounded > 0.0 {
        format!("+{}", fmt_fixed(rounded, decimals))
    } else {
        fmt_fixed(rounded, decimals)
    }
}

/// Expected score, six decimals — the precision every printed substitution uses.
fn fmt_exp(value: f64) -> String {
    format!("{:.6}", value)
}

fn fmt_pct(value: f64) -> String {
    format!("{:.2}%", value * 100.0)
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Core solver. `score_a` is player A's TOTAL points across `games` games
/// (1 per win, 0.5 per draw); player B's score is implied as `games - score_a`.
#[allow(clippy::too_many_arguments)]
pub fn solve(
    player_a_rating: f64,
    player_b_rating: f64,
    score_a: f64,
    k_factor: f64,
    games: u64,
    k_factor_b: f64,
    max_rating_difference: f64,
    decimals: u64,
    player_a_name: &str,
    player_b_name: &str,
) -> Result<Outcome, String> {
    check_rating("player_a_rating", player_a_rating)?;
    check_rating("player_b_rating", player_b_rating)?;
    check_k("k_factor", k_factor)?;
    let k_b = if k_factor_b == 0.0 {
        k_factor
    } else {
        check_k("k_factor_b", k_factor_b)?;
        k_factor_b
    };
    if games == 0 || games > MAX_GAMES {
        return Err(format!(
            "games must be between 1 and {MAX_GAMES}, got {games}"
        ));
    }
    if !score_a.is_finite() || score_a < 0.0 || score_a > games as f64 {
        return Err(format!(
            "score_a must be between 0 and {games} (the number of games) — 1 per win, 0.5 per draw; got {}",
            trim_num(score_a)
        ));
    }
    if !max_rating_difference.is_finite()
        || max_rating_difference < 0.0
        || max_rating_difference > MAX_RATING
    {
        return Err(format!(
            "max_rating_difference must be between 0 (no cap) and {MAX_RATING:.0}; FIDE uses 400. Got {}",
            trim_num(max_rating_difference)
        ));
    }
    if decimals > MAX_DECIMALS {
        return Err(format!(
            "decimals must be between 0 and {MAX_DECIMALS}, got {decimals}"
        ));
    }
    let name_a = clean_name("player_a_name", player_a_name, "Player A")?;
    let name_b = clean_name("player_b_name", player_b_name, "Player B")?;

    let raw_diff = player_a_rating - player_b_rating;
    let effective_diff = if max_rating_difference > 0.0 {
        raw_diff.clamp(-max_rating_difference, max_rating_difference)
    } else {
        raw_diff
    };
    let capped = effective_diff != raw_diff;

    // Expectancy uses the (optionally capped) difference, not the raw ratings.
    let e_a = expected_score(effective_diff, 0.0);
    let e_b = 1.0 - e_a;
    let score_b = games as f64 - score_a;

    let side = |name: String, rating: f64, expected: f64, score: f64, k: f64| -> Side {
        let expected_total = expected * games as f64;
        let raw = k * (score - expected_total);
        let change = round_to(raw, decimals);
        Side {
            name,
            rating_before: rating,
            expected_score: expected,
            expected_total,
            score,
            k_factor: k,
            rating_change_raw: raw,
            rating_change: change,
            rating_after: round_to(rating + change, decimals),
        }
    };

    Ok(Outcome {
        a: side(name_a, player_a_rating, e_a, score_a, k_factor),
        b: side(name_b, player_b_rating, e_b, score_b, k_b),
        games,
        rating_difference: raw_diff,
        effective_difference: effective_diff,
        capped,
        decimals,
    })
}

fn side_line(s: &Side, decimals: u64) -> String {
    format!(
        "{}: {} -> {} ({})",
        s.name,
        trim_num(s.rating_before),
        fmt_fixed(s.rating_after, decimals),
        fmt_signed(s.rating_change, decimals)
    )
}

fn substitution_line(s: &Side, opponent_rating: f64, outcome: &Outcome) -> String {
    let games = outcome.games;
    let d = outcome.decimals;
    let expectation = if games == 1 {
        fmt_exp(s.expected_score)
    } else {
        format!(
            "{} x {} games = {}",
            fmt_exp(s.expected_score),
            games,
            trim_num(round_to(s.expected_total, 6))
        )
    };
    format!(
        "{}: E = 1 / (1 + 10^(({} - {}) / 400)) = {}; dR = {} x ({} - {}) = {} -> {}",
        s.name,
        trim_num(opponent_rating),
        trim_num(s.rating_before),
        expectation,
        trim_num(s.k_factor),
        trim_num(s.score),
        trim_num(round_to(s.expected_total, 6)),
        fmt_signed(round_to(s.rating_change_raw, 4), 4),
        fmt_signed(s.rating_change, d)
    )
}

/// The win/draw/loss (single game) or all-wins/all-draws/all-losses (series) what-if block.
fn scenarios(outcome: &Outcome, k_factor_b_arg: f64, max_diff: f64) -> Result<String, String> {
    let games = outcome.games;
    let g = games as f64;
    let labels: [(&str, f64); 3] = if games == 1 {
        [("win", 1.0), ("draw", 0.5), ("loss", 0.0)]
    } else {
        [("all wins", g), ("all draws", g / 2.0), ("all losses", 0.0)]
    };
    let mut lines = vec![format!("Scenarios for {}:", outcome.a.name)];
    for (label, score) in labels {
        let alt = solve(
            outcome.a.rating_before,
            outcome.b.rating_before,
            score,
            outcome.a.k_factor,
            games,
            k_factor_b_arg,
            max_diff,
            outcome.decimals,
            &outcome.a.name,
            &outcome.b.name,
        )?;
        lines.push(format!(
            "Score {} ({}): {} -> {} ({}); {} {} -> {} ({})",
            trim_num(score),
            label,
            trim_num(alt.a.rating_before),
            fmt_fixed(alt.a.rating_after, alt.decimals),
            fmt_signed(alt.a.rating_change, alt.decimals),
            alt.b.name,
            trim_num(alt.b.rating_before),
            fmt_fixed(alt.b.rating_after, alt.decimals),
            fmt_signed(alt.b.rating_change, alt.decimals),
        ));
    }
    Ok(lines.join("\n"))
}

fn render_json(outcome: &Outcome) -> String {
    let side = |s: &Side| {
        format!(
            "{{\"name\": \"{}\", \"rating_before\": {}, \"expected_score\": {}, \"expected_percent\": {}, \"expected_total\": {}, \"score\": {}, \"k_factor\": {}, \"rating_change_raw\": {}, \"rating_change\": {}, \"rating_after\": {}}}",
            json_escape(&s.name),
            trim_num(s.rating_before),
            fmt_exp(s.expected_score),
            format!("{:.2}", s.expected_score * 100.0),
            trim_num(round_to(s.expected_total, 6)),
            trim_num(s.score),
            trim_num(s.k_factor),
            trim_num(round_to(s.rating_change_raw, 6)),
            trim_num(s.rating_change),
            trim_num(s.rating_after),
        )
    };
    format!(
        "{{\n  \"player_a\": {},\n  \"player_b\": {},\n  \"games\": {},\n  \"rating_difference\": {},\n  \"effective_rating_difference\": {},\n  \"rating_difference_capped\": {}\n}}",
        side(&outcome.a),
        side(&outcome.b),
        outcome.games,
        trim_num(outcome.rating_difference),
        trim_num(outcome.effective_difference),
        outcome.capped,
    )
}

fn render_csv(outcome: &Outcome) -> String {
    let row = |s: &Side| {
        format!(
            "{},{},{},{},{},{},{}",
            csv_field(&s.name),
            trim_num(s.rating_before),
            fmt_exp(s.expected_score),
            trim_num(s.score),
            trim_num(s.k_factor),
            trim_num(s.rating_change),
            trim_num(s.rating_after),
        )
    };
    format!(
        "player,rating_before,expected_score,score,k_factor,rating_change,rating_after\n{}\n{}",
        row(&outcome.a),
        row(&outcome.b)
    )
}

fn render_summary(outcome: &Outcome, k_factor_b_arg: f64, max_diff: f64) -> Result<String, String> {
    let d = outcome.decimals;
    let mut out = String::new();
    out.push_str(&side_line(&outcome.a, d));
    out.push('\n');
    out.push_str(&side_line(&outcome.b, d));
    out.push_str("\n\n");

    let game_word = if outcome.games == 1 { "game" } else { "games" };
    out.push_str(&format!(
        "Match: {} {}, {} scored {}, {} scored {}\n",
        outcome.games,
        game_word,
        outcome.a.name,
        trim_num(outcome.a.score),
        outcome.b.name,
        trim_num(outcome.b.score)
    ));
    out.push_str(&format!(
        "Expected: {} {} ({}), {} {} ({})\n",
        outcome.a.name,
        fmt_exp(outcome.a.expected_score),
        fmt_pct(outcome.a.expected_score),
        outcome.b.name,
        fmt_exp(outcome.b.expected_score),
        fmt_pct(outcome.b.expected_score)
    ));
    let leader = if outcome.rating_difference > 0.0 {
        format!("in favour of {}", outcome.a.name)
    } else if outcome.rating_difference < 0.0 {
        format!("in favour of {}", outcome.b.name)
    } else {
        "— evenly matched".to_string()
    };
    out.push_str(&format!(
        "Rating difference: {} points {}\n",
        trim_num(outcome.rating_difference.abs()),
        leader
    ));
    if outcome.capped {
        out.push_str(&format!(
            "Capped: the expectancy used a difference of {} points (max_rating_difference = {})\n",
            trim_num(outcome.effective_difference.abs()),
            trim_num(max_diff)
        ));
    }
    out.push_str(&format!(
        "K-factor: {} {}, {} {}\n",
        outcome.a.name,
        trim_num(outcome.a.k_factor),
        outcome.b.name,
        trim_num(outcome.b.k_factor)
    ));

    out.push_str("\nFormula: E = 1 / (1 + 10^((opponent - player) / 400)), dR = K x (S - E)\n");
    let (a_opp, b_opp) = if outcome.capped {
        (
            outcome.a.rating_before - outcome.effective_difference,
            outcome.b.rating_before + outcome.effective_difference,
        )
    } else {
        (outcome.b.rating_before, outcome.a.rating_before)
    };
    out.push_str(&substitution_line(&outcome.a, a_opp, outcome));
    out.push('\n');
    out.push_str(&substitution_line(&outcome.b, b_opp, outcome));

    out.push_str("\n\n");
    out.push_str(&scenarios(outcome, k_factor_b_arg, max_diff)?);
    Ok(out)
}

/// Raw field entry point used by the chat block, CLI and browser page.
#[allow(clippy::too_many_arguments)]
pub fn run(
    player_a_rating: f64,
    player_b_rating: f64,
    score_a: f64,
    k_factor: f64,
    games: u64,
    k_factor_b: f64,
    max_rating_difference: f64,
    decimals: u64,
    output_format: &str,
    player_a_name: &str,
    player_b_name: &str,
) -> Result<String, String> {
    compute(
        player_a_rating,
        player_b_rating,
        score_a,
        k_factor,
        games,
        k_factor_b,
        max_rating_difference,
        decimals,
        parse_output_format(output_format)?,
        player_a_name,
        player_b_name,
    )
}

/// Solve + render in one call — the entry point every surface uses.
#[allow(clippy::too_many_arguments)]
pub fn compute(
    player_a_rating: f64,
    player_b_rating: f64,
    score_a: f64,
    k_factor: f64,
    games: u64,
    k_factor_b: f64,
    max_rating_difference: f64,
    decimals: u64,
    format: OutputFormat,
    player_a_name: &str,
    player_b_name: &str,
) -> Result<String, String> {
    let outcome = solve(
        player_a_rating,
        player_b_rating,
        score_a,
        k_factor,
        games,
        k_factor_b,
        max_rating_difference,
        decimals,
        player_a_name,
        player_b_name,
    )?;
    match format {
        OutputFormat::Summary => render_summary(&outcome, k_factor_b, max_rating_difference),
        OutputFormat::Scenarios => scenarios(&outcome, k_factor_b, max_rating_difference),
        OutputFormat::Json => Ok(render_json(&outcome)),
        OutputFormat::Csv => Ok(render_csv(&outcome)),
        OutputFormat::Delta => Ok(fmt_signed(outcome.a.rating_change, outcome.decimals)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_compute(
        a: f64,
        b: f64,
        score: f64,
        k: f64,
        format: OutputFormat,
    ) -> Result<String, String> {
        compute(a, b, score, k, 1, 0.0, 0.0, 0, format, "", "")
    }

    #[test]
    fn expected_score_is_one_half_for_equal_ratings() {
        assert!((expected_score(1500.0, 1500.0) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn expected_score_matches_the_classic_400_point_table() {
        // A 400-point edge is the textbook 10:1 expectancy.
        assert!((expected_score(1900.0, 1500.0) - 10.0 / 11.0).abs() < 1e-12);
        assert!((expected_score(1500.0, 1900.0) - 1.0 / 11.0).abs() < 1e-12);
    }

    #[test]
    fn delta_for_a_favourite_winning_is_small() {
        let out = default_compute(1600.0, 1500.0, 1.0, 32.0, OutputFormat::Delta).unwrap();
        assert_eq!(out, "+12");
    }

    #[test]
    fn delta_for_an_underdog_winning_is_large() {
        let out = default_compute(1500.0, 1600.0, 1.0, 32.0, OutputFormat::Delta).unwrap();
        assert_eq!(out, "+20");
    }

    #[test]
    fn a_draw_costs_the_favourite_points() {
        let out = default_compute(1600.0, 1500.0, 0.5, 32.0, OutputFormat::Delta).unwrap();
        assert_eq!(out, "-4");
    }

    #[test]
    fn equal_ratings_and_a_draw_move_nothing() {
        let out = default_compute(1500.0, 1500.0, 0.5, 32.0, OutputFormat::Delta).unwrap();
        assert_eq!(out, "0");
    }

    #[test]
    fn summary_reports_both_sides_and_the_substitution() {
        let out = default_compute(1600.0, 1500.0, 1.0, 32.0, OutputFormat::Summary).unwrap();
        assert!(out.starts_with("Player A: 1600 -> 1612 (+12)\nPlayer B: 1500 -> 1488 (-12)\n"));
        assert!(
            out.contains("Expected: Player A 0.640065 (64.01%), Player B 0.359935 (35.99%)"),
            "{out}"
        );
        assert!(
            out.contains(
                "Player A: E = 1 / (1 + 10^((1500 - 1600) / 400)) = 0.640065; dR = 32 x (1 - 0.640065) = +11.5179 -> +12"
            ),
            "{out}"
        );
        assert!(
            out.contains("Rating difference: 100 points in favour of Player A"),
            "{out}"
        );
        assert!(out.contains("Score 0.5 (draw): 1600 -> 1596 (-4)"), "{out}");
    }

    #[test]
    fn scenarios_price_all_three_outcomes() {
        let out = default_compute(1600.0, 1500.0, 1.0, 32.0, OutputFormat::Scenarios).unwrap();
        assert_eq!(
            out,
            "Scenarios for Player A:\n\
             Score 1 (win): 1600 -> 1612 (+12); Player B 1500 -> 1488 (-12)\n\
             Score 0.5 (draw): 1600 -> 1596 (-4); Player B 1500 -> 1504 (+4)\n\
             Score 0 (loss): 1600 -> 1580 (-20); Player B 1500 -> 1520 (+20)"
        );
    }

    #[test]
    fn a_series_scales_the_expected_total() {
        // 5 games, 3.5 points for the 1600 against the 1500: expected total 3.200325.
        let out = compute(
            1600.0,
            1500.0,
            3.5,
            32.0,
            5,
            0.0,
            0.0,
            0,
            OutputFormat::Summary,
            "",
            "",
        )
        .unwrap();
        assert!(out.starts_with("Player A: 1600 -> 1610 (+10)\n"), "{out}");
        assert!(
            out.contains("Match: 5 games, Player A scored 3.5, Player B scored 1.5"),
            "{out}"
        );
        assert!(out.contains("0.640065 x 5 games = 3.200325"), "{out}");
        assert!(out.contains("Score 5 (all wins)"), "{out}");
    }

    #[test]
    fn the_400_point_cap_limits_the_expectancy() {
        // A 1000-point gap capped at 400 behaves exactly like a 400-point gap.
        let capped = compute(
            2400.0,
            1400.0,
            1.0,
            20.0,
            1,
            0.0,
            400.0,
            0,
            OutputFormat::Json,
            "",
            "",
        )
        .unwrap();
        let plain = compute(
            1800.0,
            1400.0,
            1.0,
            20.0,
            1,
            0.0,
            0.0,
            0,
            OutputFormat::Json,
            "",
            "",
        )
        .unwrap();
        assert!(capped.contains("\"expected_score\": 0.909091"), "{capped}");
        assert!(plain.contains("\"expected_score\": 0.909091"), "{plain}");
        assert!(
            capped.contains("\"rating_difference_capped\": true"),
            "{capped}"
        );
        assert!(
            plain.contains("\"rating_difference_capped\": false"),
            "{plain}"
        );
    }

    #[test]
    fn asymmetric_k_factors_break_the_zero_sum() {
        let out = compute(
            1600.0,
            1500.0,
            1.0,
            10.0,
            1,
            40.0,
            0.0,
            0,
            OutputFormat::Csv,
            "",
            "",
        )
        .unwrap();
        assert_eq!(
            out,
            "player,rating_before,expected_score,score,k_factor,rating_change,rating_after\n\
             Player A,1600,0.640065,1,10,4,1604\n\
             Player B,1500,0.359935,0,40,-14,1486"
        );
    }

    #[test]
    fn decimals_expose_the_unrounded_change() {
        let out = compute(
            1600.0,
            1500.0,
            1.0,
            32.0,
            1,
            0.0,
            0.0,
            2,
            OutputFormat::Delta,
            "",
            "",
        )
        .unwrap();
        assert_eq!(out, "+11.52");
    }

    #[test]
    fn rating_after_always_equals_before_plus_reported_change() {
        for (a, b, s) in [
            (1234.0, 1987.0, 0.5),
            (800.0, 810.0, 1.0),
            (2600.0, 2100.0, 0.0),
        ] {
            let o = solve(a, b, s, 24.0, 1, 0.0, 0.0, 0, "", "").unwrap();
            assert_eq!(o.a.rating_after, o.a.rating_before + o.a.rating_change);
            assert_eq!(o.b.rating_after, o.b.rating_before + o.b.rating_change);
        }
    }

    #[test]
    fn json_carries_every_intermediate_value() {
        let out = default_compute(1500.0, 1500.0, 1.0, 32.0, OutputFormat::Json).unwrap();
        assert!(out.contains("\"expected_score\": 0.500000"), "{out}");
        assert!(out.contains("\"expected_percent\": 50.00"), "{out}");
        assert!(out.contains("\"rating_change\": 16"), "{out}");
        assert!(out.contains("\"rating_after\": 1516"), "{out}");
        assert!(out.contains("\"games\": 1"), "{out}");
    }

    #[test]
    fn names_are_used_and_csv_quotes_them() {
        let out = compute(
            1500.0,
            1500.0,
            1.0,
            32.0,
            1,
            0.0,
            0.0,
            0,
            OutputFormat::Csv,
            "Carlsen, M",
            "Nakamura",
        )
        .unwrap();
        assert!(
            out.contains("\"Carlsen, M\",1500,0.500000,1,32,16,1516"),
            "{out}"
        );
        assert!(
            out.contains("Nakamura,1500,0.500000,0,32,-16,1484"),
            "{out}"
        );
    }

    #[test]
    fn score_above_the_game_count_is_rejected_with_the_bound() {
        let err = default_compute(1500.0, 1500.0, 2.0, 32.0, OutputFormat::Summary).unwrap_err();
        assert!(
            err.contains("score_a must be between 0 and 1 (the number of games)"),
            "{err}"
        );
        assert!(err.contains("got 2"), "{err}");
    }

    #[test]
    fn out_of_range_rating_is_rejected() {
        let err = default_compute(9000.0, 1500.0, 1.0, 32.0, OutputFormat::Summary).unwrap_err();
        assert_eq!(err, "player_a_rating must be between 0 and 5000, got 9000");
    }

    #[test]
    fn out_of_range_k_factor_is_rejected() {
        let err = default_compute(1500.0, 1500.0, 1.0, 400.0, OutputFormat::Summary).unwrap_err();
        assert!(
            err.starts_with("k_factor must be between 0.1 and 200"),
            "{err}"
        );
    }

    #[test]
    fn zero_games_is_rejected() {
        let err = compute(
            1500.0,
            1500.0,
            0.0,
            32.0,
            0,
            0.0,
            0.0,
            0,
            OutputFormat::Summary,
            "",
            "",
        )
        .unwrap_err();
        assert_eq!(err, "games must be between 1 and 1000, got 0");
    }

    #[test]
    fn overlong_name_is_rejected() {
        let long = "x".repeat(61);
        let err = compute(
            1500.0,
            1500.0,
            1.0,
            32.0,
            1,
            0.0,
            0.0,
            0,
            OutputFormat::Summary,
            &long,
            "",
        )
        .unwrap_err();
        assert_eq!(err, "player_a_name must be 60 characters or fewer, got 61");
    }

    #[test]
    fn unknown_output_format_is_rejected() {
        let err = parse_output_format("chart").unwrap_err();
        assert!(err.contains("output_format must be one of"), "{err}");
        assert!(err.contains("got 'chart'"), "{err}");
    }

    #[test]
    fn output_format_parsing_is_lenient_about_case_and_blanks() {
        assert_eq!(parse_output_format("").unwrap(), OutputFormat::Summary);
        assert_eq!(parse_output_format(" JSON ").unwrap(), OutputFormat::Json);
        assert_eq!(parse_output_format("Csv").unwrap(), OutputFormat::Csv);
    }
}
