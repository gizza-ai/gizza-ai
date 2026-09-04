//! nps-csat-calculator core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Turns one column of survey ratings into a customer-experience metric:
//!   * `nps`  — Net Promoter Score on the fixed 0-10 scale: promoters (9-10) minus
//!              detractors (0-6) as a share of all responses, reported in points.
//!   * `csat` — Customer Satisfaction Score: the share of responses at or above the
//!              satisfied cut-off (top-2 box by default), reported as a percentage.
//!   * `ces`  — Customer Effort Score: the mean rating, plus the share at or above
//!              the "easy" cut-off.
//!
//! Every metric also reports the three-band breakdown (promoters / passives /
//! detractors, or satisfied / neutral / dissatisfied), the full rating
//! distribution, and a normal-approximation confidence band around the score.
//!
//! Ratings arrive either as individual values (`input = "values"`) or as
//! `rating,count` tallies (`input = "counts"`).

/// Hard cap on how many responses a single run will summarize.
pub const MAX_RESPONSES: usize = 100_000;

/// Width, in characters, of a 100% distribution bar.
const BAR_WIDTH: f64 = 40.0;

/// Cells equal to one of these (case-insensitively), or empty, count as missing.
const MISSING_MARKERS: [&str; 8] = ["na", "n/a", "-", ".", "none", "null", "missing", "?"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Metric {
    Nps,
    Csat,
    Ces,
}

impl Metric {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "nps" => Ok(Metric::Nps),
            "csat" => Ok(Metric::Csat),
            "ces" => Ok(Metric::Ces),
            other => Err(format!(
                "metric must be one of nps, csat, ces (got '{other}')"
            )),
        }
    }

    fn key(self) -> &'static str {
        match self {
            Metric::Nps => "nps",
            Metric::Csat => "csat",
            Metric::Ces => "ces",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Metric::Nps => "Net Promoter Score (NPS)",
            Metric::Csat => "Customer Satisfaction Score (CSAT)",
            Metric::Ces => "Customer Effort Score (CES)",
        }
    }

    /// Labels for the top / middle / bottom bands, in that order.
    fn band_labels(self) -> [&'static str; 3] {
        match self {
            Metric::Nps => ["Promoters", "Passives", "Detractors"],
            Metric::Csat => ["Satisfied", "Neutral", "Dissatisfied"],
            Metric::Ces => ["Easy", "Neutral", "Difficult"],
        }
    }
}

fn z_for(confidence: &str) -> Result<Option<(f64, &'static str)>, String> {
    match confidence.trim().to_ascii_lowercase().as_str() {
        "" | "95" | "95%" | "0.95" => Ok(Some((1.96, "95"))),
        "90" | "90%" | "0.9" | "0.90" => Ok(Some((1.645, "90"))),
        "99" | "99%" | "0.99" => Ok(Some((2.576, "99"))),
        "none" | "off" | "no" => Ok(None),
        other => Err(format!(
            "confidence must be one of 90, 95, 99, none (got '{other}')"
        )),
    }
}

/// Resolve the rating scale for a metric. `auto` picks the conventional scale.
fn resolve_scale(metric: Metric, scale: &str) -> Result<(i64, i64), String> {
    let s = scale.trim().to_ascii_lowercase();
    let explicit = match s.as_str() {
        "" | "auto" => None,
        "0-10" | "0..10" => Some((0, 10)),
        "1-5" | "1..5" => Some((1, 5)),
        "1-7" | "1..7" => Some((1, 7)),
        "1-10" | "1..10" => Some((1, 10)),
        other => {
            return Err(format!(
                "scale must be one of auto, 0-10, 1-5, 1-7, 1-10 (got '{other}')"
            ))
        }
    };
    match metric {
        Metric::Nps => match explicit {
            None | Some((0, 10)) => Ok((0, 10)),
            Some(_) => Err(
                "NPS is defined on the 0-10 scale only — use scale=auto or scale=0-10, or switch \
                 metric to csat/ces for other scales"
                    .to_string(),
            ),
        },
        Metric::Csat => Ok(explicit.unwrap_or((1, 5))),
        Metric::Ces => Ok(explicit.unwrap_or((1, 7))),
    }
}

/// The default "satisfied"/"easy" cut-off: the top-2 box of the scale, except on a
/// 1-7 scale where the usual CES/CSAT cut-off is the top-3 box (5 and up).
fn auto_threshold(min: i64, max: i64) -> i64 {
    let t = if max - min == 6 { max - 2 } else { max - 1 };
    t.max(min + 1)
}

fn is_missing(tok: &str) -> bool {
    let t = tok.trim();
    t.is_empty() || MISSING_MARKERS.contains(&t.to_ascii_lowercase().as_str())
}

fn split_tokens(line: &str) -> Vec<&str> {
    line.split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '|' || c == ':')
        .filter(|t| !t.is_empty())
        .collect()
}

/// A line is a header if it holds at least one real token and none of them parse
/// as a number — e.g. a pasted CSV's `score` / `How likely are you...` row.
fn looks_like_header(line: &str) -> bool {
    let toks = split_tokens(line);
    let real: Vec<&&str> = toks.iter().filter(|t| !is_missing(t)).collect();
    !real.is_empty() && real.iter().all(|t| t.parse::<f64>().is_err())
}

fn parse_rating(tok: &str, min: i64, max: i64) -> Result<i64, String> {
    let t = tok.trim();
    let v: f64 = t
        .parse()
        .map_err(|_| format!("'{t}' is not a number — ratings must be whole numbers"))?;
    if !v.is_finite() || v.fract() != 0.0 {
        return Err(format!("rating '{t}' must be a whole number"));
    }
    let v = v as i64;
    if v < min || v > max {
        return Err(format!(
            "rating {v} is outside the {min}-{max} scale — change the scale or fix the data"
        ));
    }
    Ok(v)
}

/// Parsed responses: a tally per scale point plus the count of skipped cells.
struct Parsed {
    counts: Vec<usize>,
    missing: usize,
}

fn parse_values(data: &str, min: i64, max: i64) -> Result<Parsed, String> {
    let mut counts = vec![0usize; (max - min + 1) as usize];
    let mut missing = 0usize;
    let mut total = 0usize;
    let mut seen_first_real_line = false;

    for line in data.lines() {
        if split_tokens(line).is_empty() {
            continue;
        }
        if !seen_first_real_line {
            seen_first_real_line = true;
            if looks_like_header(line) {
                continue;
            }
        }
        for tok in split_tokens(line) {
            if is_missing(tok) {
                missing += 1;
                continue;
            }
            let v = parse_rating(tok, min, max)?;
            total += 1;
            if total > MAX_RESPONSES {
                return Err(format!(
                    "too many responses — this tool handles up to {MAX_RESPONSES} ratings per run; \
                     use input=counts for larger samples"
                ));
            }
            counts[(v - min) as usize] += 1;
        }
    }
    Ok(Parsed { counts, missing })
}

fn parse_counts(data: &str, min: i64, max: i64) -> Result<Parsed, String> {
    let mut counts = vec![0usize; (max - min + 1) as usize];
    let mut total = 0usize;
    let mut seen_first_real_line = false;

    for line in data.lines() {
        let toks = split_tokens(line);
        if toks.is_empty() {
            continue;
        }
        if !seen_first_real_line {
            seen_first_real_line = true;
            if looks_like_header(line) {
                continue;
            }
        }
        if toks.len() != 2 {
            return Err(format!(
                "counts rows need exactly two values, 'rating,count' (got '{}')",
                line.trim()
            ));
        }
        let v = parse_rating(toks[0], min, max)?;
        let c: f64 = toks[1]
            .parse()
            .map_err(|_| format!("count '{}' is not a number", toks[1]))?;
        if !c.is_finite() || c.fract() != 0.0 || c < 0.0 {
            return Err(format!(
                "count '{}' must be a whole number of 0 or more",
                toks[1]
            ));
        }
        let c = c as usize;
        total += c;
        if total > MAX_RESPONSES {
            return Err(format!(
                "too many responses — this tool handles up to {MAX_RESPONSES} ratings per run"
            ));
        }
        counts[(v - min) as usize] += c;
    }
    Ok(Parsed { counts, missing: 0 })
}

/// Everything the report/JSON/CSV writers need.
struct Outcome {
    metric: Metric,
    min: i64,
    max: i64,
    threshold: i64,
    counts: Vec<usize>,
    n: usize,
    missing: usize,
    mean: f64,
    sd: f64,
    /// (label, inclusive rating range, count) for the top / middle / bottom bands.
    bands: [(&'static str, (i64, i64), usize); 3],
    score: f64,
    /// Share (%) of responses in the top band — the "easy"/"satisfied" share.
    top_pct: f64,
    /// (level, margin, low, high) of the confidence band, when computable.
    ci: Option<(&'static str, f64, f64, f64)>,
    ci_note: Option<&'static str>,
    rating: &'static str,
}

fn band_range_label(lo: i64, hi: i64) -> String {
    if lo > hi {
        "(none)".to_string()
    } else if lo == hi {
        format!("({lo})")
    } else {
        format!("({lo}-{hi})")
    }
}

fn nps_tier(score: f64) -> &'static str {
    if score >= 70.0 {
        "World class"
    } else if score >= 50.0 {
        "Excellent"
    } else if score >= 30.0 {
        "Great"
    } else if score >= 0.0 {
        "Good"
    } else {
        "Needs improvement"
    }
}

fn csat_tier(pct: f64) -> &'static str {
    if pct >= 90.0 {
        "Excellent"
    } else if pct >= 75.0 {
        "Good"
    } else if pct >= 60.0 {
        "Fair"
    } else {
        "Needs improvement"
    }
}

/// CES is rated on where the mean sits within the scale, so 1-5 and 1-7 compare.
fn ces_tier(mean: f64, min: i64, max: i64) -> &'static str {
    let span = (max - min) as f64;
    let frac = if span <= 0.0 {
        0.0
    } else {
        (mean - min as f64) / span
    };
    if frac >= 0.75 {
        "Low effort"
    } else if frac >= 0.6 {
        "Moderate effort"
    } else {
        "High effort"
    }
}

fn fmt_num(v: f64, decimals: i64) -> String {
    let d = decimals.clamp(0, 6) as usize;
    let s = format!("{v:.d$}");
    // Avoid a "-0.0" that only exists because of rounding.
    if s.trim_start_matches('-').chars().all(|c| c == '0' || c == '.') && s.starts_with('-') {
        s[1..].to_string()
    } else {
        s
    }
}

fn pct(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64 * 100.0
    }
}

#[allow(clippy::too_many_arguments)]
fn analyse(
    metric: Metric,
    min: i64,
    max: i64,
    threshold: i64,
    parsed: Parsed,
    confidence: &str,
) -> Result<Outcome, String> {
    let counts = parsed.counts;
    let n: usize = counts.iter().sum();
    if n == 0 {
        return Err(
            "no ratings found — paste a column of numeric ratings (blank cells and NA markers are \
             skipped)"
                .to_string(),
        );
    }

    let mut sum = 0.0f64;
    for (i, c) in counts.iter().enumerate() {
        sum += (min + i as i64) as f64 * *c as f64;
    }
    let mean = sum / n as f64;
    let sd = if n < 2 {
        0.0
    } else {
        let mut ss = 0.0f64;
        for (i, c) in counts.iter().enumerate() {
            let d = (min + i as i64) as f64 - mean;
            ss += d * d * *c as f64;
        }
        (ss / (n - 1) as f64).sqrt()
    };

    let tally = |lo: i64, hi: i64| -> usize {
        if lo > hi {
            return 0;
        }
        (lo..=hi)
            .filter(|v| *v >= min && *v <= max)
            .map(|v| counts[(v - min) as usize])
            .sum()
    };

    let (top_r, mid_r, bot_r) = match metric {
        Metric::Nps => ((9, 10), (7, 8), (0, 6)),
        _ => ((threshold, max), (threshold - 1, threshold - 1), (min, threshold - 2)),
    };
    let labels = metric.band_labels();
    let bands = [
        (labels[0], top_r, tally(top_r.0, top_r.1)),
        (labels[1], mid_r, tally(mid_r.0, mid_r.1)),
        (labels[2], bot_r, tally(bot_r.0, bot_r.1)),
    ];

    let top_pct = pct(bands[0].2, n);
    let p = bands[0].2 as f64 / n as f64;
    let d = bands[2].2 as f64 / n as f64;

    let score = match metric {
        Metric::Nps => (p - d) * 100.0,
        Metric::Csat => top_pct,
        Metric::Ces => mean,
    };

    let (ci, ci_note) = match z_for(confidence)? {
        None => (None, None),
        Some((z, level)) if n >= 2 => {
            let (margin, lo_clamp, hi_clamp) = match metric {
                Metric::Nps => {
                    let var = (p + d - (p - d) * (p - d)).max(0.0) / n as f64;
                    (z * var.sqrt() * 100.0, -100.0, 100.0)
                }
                Metric::Csat => (z * (p * (1.0 - p) / n as f64).sqrt() * 100.0, 0.0, 100.0),
                Metric::Ces => (z * sd / (n as f64).sqrt(), min as f64, max as f64),
            };
            (
                Some((
                    level,
                    margin,
                    (score - margin).max(lo_clamp),
                    (score + margin).min(hi_clamp),
                )),
                None,
            )
        }
        Some(_) => (None, Some("needs at least 2 responses")),
    };

    let rating = match metric {
        Metric::Nps => nps_tier(score),
        Metric::Csat => csat_tier(score),
        Metric::Ces => ces_tier(mean, min, max),
    };

    Ok(Outcome {
        metric,
        min,
        max,
        threshold,
        counts,
        n,
        missing: parsed.missing,
        mean,
        sd,
        bands,
        score,
        top_pct,
        ci,
        ci_note,
        rating,
    })
}

/// For NPS: the cheapest set of upgrades that would reach the next rating tier.
fn next_tier_line(o: &Outcome) -> Option<String> {
    if o.metric != Metric::Nps {
        return None;
    }
    let tiers: [(f64, &str); 4] = [
        (0.0, "Good"),
        (30.0, "Great"),
        (50.0, "Excellent"),
        (70.0, "World class"),
    ];
    let next = tiers.iter().find(|(floor, _)| o.score < *floor - 1e-9);
    let Some((floor, name)) = next else {
        return Some("Next tier: already in the top band.".to_string());
    };
    // One detractor promoted moves the score by 200/n; one passive by 100/n.
    let steps_needed = (floor - o.score) * o.n as f64 / 100.0;
    let detractors = o.bands[2].2 as f64;
    let passives = o.bands[1].2 as f64;
    let use_d = (steps_needed / 2.0).ceil().min(detractors).max(0.0);
    let left = steps_needed - use_d * 2.0;
    let use_p = if left > 1e-9 {
        left.ceil().min(passives)
    } else {
        0.0
    };
    let mut parts = Vec::new();
    if use_d > 0.0 {
        parts.push(format!(
            "{} detractor{}",
            use_d as usize,
            if use_d == 1.0 { "" } else { "s" }
        ));
    }
    if use_p > 0.0 {
        parts.push(format!(
            "{} passive{}",
            use_p as usize,
            if use_p == 1.0 { "" } else { "s" }
        ));
    }
    if parts.is_empty() {
        return Some(format!(
            "Next tier: {name} ({}+) needs more promoters than this sample can supply.",
            fmt_num(*floor, 0)
        ));
    }
    Some(format!(
        "Next tier: {name} ({}+) — move {} into the promoter band (at n = {}).",
        fmt_num(*floor, 0),
        parts.join(" and "),
        o.n
    ))
}

fn render_report(o: &Outcome, decimals: i64, distribution: bool) -> String {
    let mut out = String::new();
    let title = o.metric.title();
    out.push_str(title);
    out.push('\n');
    out.push_str(&"=".repeat(title.chars().count()));
    out.push_str("\n\n");

    let lbl = |s: &str| format!("{s:<26}");
    match o.metric {
        Metric::Nps => out.push_str(&format!(
            "{}{}   (-100 to +100)\n",
            lbl("NPS"),
            fmt_num(o.score, decimals)
        )),
        Metric::Csat => out.push_str(&format!(
            "{}{}%\n",
            lbl("CSAT"),
            fmt_num(o.score, decimals)
        )),
        Metric::Ces => out.push_str(&format!(
            "{}{}   (scale {}-{})\n",
            lbl("CES"),
            fmt_num(o.score, decimals),
            o.min,
            o.max
        )),
    }
    out.push_str(&format!("{}{}\n", lbl("Rating"), o.rating));
    out.push_str(&format!("{}{}\n", lbl("Responses"), o.n));
    if o.missing > 0 {
        out.push_str(&format!("{}{}\n", lbl("Skipped (blank/NA)"), o.missing));
    }
    out.push_str(&format!(
        "{}{}\n",
        lbl("Mean rating"),
        fmt_num(o.mean, decimals)
    ));
    out.push_str(&format!(
        "{}{}\n",
        lbl("Standard deviation"),
        fmt_num(o.sd, decimals)
    ));
    if o.metric != Metric::Nps {
        out.push_str(&format!(
            "{}{}+ of {}\n",
            lbl(match o.metric {
                Metric::Csat => "Satisfied cut-off",
                _ => "Easy cut-off",
            }),
            o.threshold,
            o.max
        ));
        out.push_str(&format!(
            "{}{}%\n",
            lbl(match o.metric {
                Metric::Csat => "Satisfied share",
                _ => "Easy share",
            }),
            fmt_num(o.top_pct, decimals)
        ));
    }
    match (&o.ci, o.ci_note) {
        (Some((level, margin, lo, hi)), _) => out.push_str(&format!(
            "{}±{}   ({} to {})\n",
            lbl(&format!("{level}% confidence")),
            fmt_num(*margin, decimals),
            fmt_num(*lo, decimals),
            fmt_num(*hi, decimals)
        )),
        (None, Some(note)) => {
            out.push_str(&format!("{}{}\n", lbl("Confidence band"), note));
        }
        _ => {}
    }

    out.push_str("\nBreakdown\n");
    for (label, range, count) in &o.bands {
        out.push_str(&format!(
            "  {:<13}{:<9}{:>7}   {}%\n",
            label,
            band_range_label(range.0, range.1),
            count,
            fmt_num(pct(*count, o.n), decimals)
        ));
    }

    if distribution {
        out.push_str("\nDistribution\n");
        for v in (o.min..=o.max).rev() {
            let c = o.counts[(v - o.min) as usize];
            let share = c as f64 / o.n as f64;
            let bar = "#".repeat((share * BAR_WIDTH).round() as usize);
            out.push_str(&format!(
                "  {:>3}{:>7}   {:>6}%  {}\n",
                v,
                c,
                fmt_num(share * 100.0, decimals),
                bar
            ));
        }
    }

    if let Some(line) = next_tier_line(o) {
        out.push('\n');
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn render_json(o: &Outcome, decimals: i64, distribution: bool) -> String {
    let num = |v: f64| fmt_num(v, decimals);
    let mut s = String::from("{\n");
    s.push_str(&format!("  \"metric\": \"{}\",\n", o.metric.key()));
    s.push_str(&format!("  \"scale\": \"{}-{}\",\n", o.min, o.max));
    s.push_str(&format!("  \"score\": {},\n", num(o.score)));
    s.push_str(&format!(
        "  \"unit\": \"{}\",\n",
        match o.metric {
            Metric::Nps => "points",
            Metric::Csat => "percent",
            Metric::Ces => "mean rating",
        }
    ));
    s.push_str(&format!("  \"rating\": \"{}\",\n", json_escape(o.rating)));
    s.push_str(&format!("  \"responses\": {},\n", o.n));
    s.push_str(&format!("  \"skipped\": {},\n", o.missing));
    s.push_str(&format!("  \"mean\": {},\n", num(o.mean)));
    s.push_str(&format!("  \"std_dev\": {},\n", num(o.sd)));
    if o.metric != Metric::Nps {
        s.push_str(&format!("  \"threshold\": {},\n", o.threshold));
        s.push_str(&format!("  \"top_box_percent\": {},\n", num(o.top_pct)));
    }
    s.push_str("  \"bands\": [\n");
    let rows: Vec<String> = o
        .bands
        .iter()
        .map(|(label, range, count)| {
            format!(
                "    {{ \"label\": \"{}\", \"range\": \"{}\", \"count\": {}, \"percent\": {} }}",
                json_escape(label),
                band_range_label(range.0, range.1).trim_matches(|c| c == '(' || c == ')'),
                count,
                num(pct(*count, o.n))
            )
        })
        .collect();
    s.push_str(&rows.join(",\n"));
    s.push_str("\n  ],\n");
    match o.ci {
        Some((level, margin, lo, hi)) => s.push_str(&format!(
            "  \"confidence\": {{ \"level\": {}, \"margin\": {}, \"low\": {}, \"high\": {} }}",
            level,
            num(margin),
            num(lo),
            num(hi)
        )),
        None => s.push_str("  \"confidence\": null"),
    }
    if distribution {
        s.push_str(",\n  \"distribution\": [\n");
        let drows: Vec<String> = (o.min..=o.max)
            .rev()
            .map(|v| {
                let c = o.counts[(v - o.min) as usize];
                format!(
                    "    {{ \"rating\": {}, \"count\": {}, \"percent\": {} }}",
                    v,
                    c,
                    num(pct(c, o.n))
                )
            })
            .collect();
        s.push_str(&drows.join(",\n"));
        s.push_str("\n  ]");
    }
    s.push_str("\n}\n");
    s
}

fn render_csv(o: &Outcome, decimals: i64, distribution: bool) -> String {
    let num = |v: f64| fmt_num(v, decimals);
    let mut s = String::from("section,label,value,percent\n");
    s.push_str(&format!("score,{},{},\n", o.metric.key(), num(o.score)));
    s.push_str(&format!("score,rating,{},\n", o.rating));
    s.push_str(&format!("sample,responses,{},\n", o.n));
    s.push_str(&format!("sample,skipped,{},\n", o.missing));
    s.push_str(&format!("sample,mean,{},\n", num(o.mean)));
    s.push_str(&format!("sample,std_dev,{},\n", num(o.sd)));
    if o.metric != Metric::Nps {
        s.push_str(&format!("sample,threshold,{},\n", o.threshold));
    }
    for (label, range, count) in &o.bands {
        s.push_str(&format!(
            "band,{} {},{},{}\n",
            label.to_ascii_lowercase(),
            band_range_label(range.0, range.1).trim_matches(|c| c == '(' || c == ')'),
            count,
            num(pct(*count, o.n))
        ));
    }
    if let Some((level, margin, lo, hi)) = o.ci {
        s.push_str(&format!("confidence,level,{level},\n"));
        s.push_str(&format!("confidence,margin,{},\n", num(margin)));
        s.push_str(&format!("confidence,low,{},\n", num(lo)));
        s.push_str(&format!("confidence,high,{},\n", num(hi)));
    }
    if distribution {
        for v in (o.min..=o.max).rev() {
            let c = o.counts[(v - o.min) as usize];
            s.push_str(&format!("rating,{},{},{}\n", v, c, num(pct(c, o.n))));
        }
    }
    s
}

/// Compute NPS / CSAT / CES from a column of survey ratings.
///
/// * `ratings` — the ratings, one per cell; newline/comma/semicolon/tab/space
///   separated. A leading header line with no numbers in it is skipped.
/// * `metric` — `nps` | `csat` | `ces`.
/// * `input` — `values` (individual ratings) | `counts` (`rating,count` rows).
/// * `scale` — `auto` | `0-10` | `1-5` | `1-7` | `1-10`.
/// * `threshold` — satisfied/easy cut-off; `-1` = auto (top-2 box). Ignored for NPS.
/// * `confidence` — `90` | `95` | `99` | `none`.
/// * `decimals` — 0-6 decimal places for scores, means and percentages.
/// * `distribution` — include the per-rating distribution.
/// * `format` — `report` | `json` | `csv`.
#[allow(clippy::too_many_arguments)]
pub fn calculate(
    ratings: &str,
    metric: &str,
    input: &str,
    scale: &str,
    threshold: i64,
    confidence: &str,
    decimals: i64,
    distribution: bool,
    format: &str,
) -> Result<String, String> {
    let metric = Metric::parse(metric)?;
    let (min, max) = resolve_scale(metric, scale)?;

    let threshold = if metric == Metric::Nps {
        9
    } else if threshold < 0 {
        auto_threshold(min, max)
    } else {
        if threshold <= min || threshold > max {
            return Err(format!(
                "threshold must be between {} and {} (or -1 for the automatic top-2 box cut-off)",
                min + 1,
                max
            ));
        }
        threshold
    };

    if !(0..=6).contains(&decimals) {
        return Err(format!(
            "decimals must be between 0 and 6 (got {decimals})"
        ));
    }

    let parsed = match input.trim().to_ascii_lowercase().as_str() {
        "" | "values" | "raw" => parse_values(ratings, min, max)?,
        "counts" | "tally" => parse_counts(ratings, min, max)?,
        other => {
            return Err(format!(
                "input must be one of values, counts (got '{other}')"
            ))
        }
    };

    let outcome = analyse(metric, min, max, threshold, parsed, confidence)?;

    match format.trim().to_ascii_lowercase().as_str() {
        "" | "report" | "text" => Ok(render_report(&outcome, decimals, distribution)),
        "json" => Ok(render_json(&outcome, decimals, distribution)),
        "csv" => Ok(render_csv(&outcome, decimals, distribution)),
        other => Err(format!(
            "format must be one of report, json, csv (got '{other}')"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nps(data: &str) -> String {
        calculate(data, "nps", "values", "auto", -1, "95", 1, true, "report").unwrap()
    }

    #[test]
    fn nps_happy_path() {
        let out = nps("10,9,9,8,7,6,5,10,9,3");
        assert!(out.starts_with("Net Promoter Score (NPS)\n"));
        // promoters 5, passives 2, detractors 3 -> (0.5 - 0.3) * 100 = 20.0
        assert!(out.contains("NPS                       20.0   (-100 to +100)"));
        assert!(out.contains("Rating                    Good"));
        assert!(out.contains("Responses                 10"));
        assert!(out.contains("Promoters    (9-10)         5   50.0%"));
        assert!(out.contains("Passives     (7-8)          2   20.0%"));
        assert!(out.contains("Detractors   (0-6)          3   30.0%"));
        assert!(out.contains("Next tier: Great (30+)"));
    }

    #[test]
    fn nps_confidence_band_uses_the_difference_standard_error() {
        // p = .5, d = .3, n = 10: 1.96 * sqrt((0.8 - 0.04)/10) * 100 = 54.03
        let out = nps("10,9,9,8,7,6,5,10,9,3");
        assert!(out.contains("95% confidence            ±54.0   (-34.0 to 74.0)"), "{out}");
        let ninety =
            calculate("10,9,9,8,7,6,5,10,9,3", "nps", "values", "auto", -1, "90", 1, false, "report")
                .unwrap();
        assert!(ninety.contains("90% confidence            ±45.3   (-25.3 to 65.3)"), "{ninety}");
        let off =
            calculate("10,9,9,8,7,6,5,10,9,3", "nps", "values", "auto", -1, "none", 1, false, "report")
                .unwrap();
        assert!(!off.contains("confidence"));
    }

    #[test]
    fn header_row_blanks_and_na_markers_are_skipped() {
        let out = nps("score\n10\n9\n\nNA\n8\n-\n0\n");
        assert!(out.contains("Responses                 4"), "{out}");
        assert!(out.contains("Skipped (blank/NA)        2"), "{out}");
    }

    #[test]
    fn counts_mode_matches_the_same_values() {
        let a = nps("10,10,9,8,8,0");
        let b = calculate(
            "rating,count\n10,2\n9,1\n8,2\n0,1\n",
            "nps",
            "counts",
            "auto",
            -1,
            "95",
            1,
            true,
            "report",
        )
        .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn csat_auto_threshold_is_top_two_box_on_a_1_5_scale() {
        let out =
            calculate("5,5,4,4,3,2,1,5", "csat", "values", "auto", -1, "95", 1, false, "report")
                .unwrap();
        // satisfied = 4+ -> 5 of 8 = 62.5%
        assert!(out.contains("CSAT                      62.5%"), "{out}");
        assert!(out.contains("Satisfied cut-off         4+ of 5"), "{out}");
        assert!(out.contains("Satisfied    (4-5)          5   62.5%"), "{out}");
        assert!(out.contains("Neutral      (3)            1   12.5%"), "{out}");
        assert!(out.contains("Dissatisfied (1-2)          2   25.0%"), "{out}");
        assert!(out.contains("Rating                    Fair"), "{out}");
    }

    #[test]
    fn csat_threshold_override_moves_the_cut_off() {
        let out =
            calculate("5,5,4,4,3,2,1,5", "csat", "values", "1-5", 5, "none", 0, false, "report")
                .unwrap();
        // satisfied = 5 only -> 3 of 8 = 37.5% -> 38 at 0 decimals
        assert!(out.contains("CSAT                      38%"), "{out}");
        assert!(out.contains("Satisfied    (5)            3   38%"), "{out}");
    }

    #[test]
    fn ces_reports_the_mean_and_easy_share_on_a_1_7_scale() {
        let out =
            calculate("7,6,5,5,4,3,2,7", "ces", "values", "auto", -1, "95", 2, false, "report")
                .unwrap();
        // mean = 39/8 = 4.875, easy (5+) = 5 of 8 = 62.5%
        assert!(out.contains("CES                       4.88   (scale 1-7)"), "{out}");
        assert!(out.contains("Easy cut-off              5+ of 7"), "{out}");
        assert!(out.contains("Easy share                62.50%"), "{out}");
        assert!(out.contains("Easy         (5-7)          5   62.50%"), "{out}");
        assert!(out.contains("Neutral      (4)            1   12.50%"), "{out}");
        assert!(out.contains("Difficult    (1-3)          2   25.00%"), "{out}");
        assert!(out.contains("Rating                    Moderate effort"), "{out}");
    }

    #[test]
    fn boundary_ratings_land_in_the_advertised_bands() {
        // Exactly one response at every NPS band boundary.
        let out = nps("0,6,7,8,9,10");
        assert!(out.contains("Promoters    (9-10)         2   33.3%"), "{out}");
        assert!(out.contains("Passives     (7-8)          2   33.3%"), "{out}");
        assert!(out.contains("Detractors   (0-6)          2   33.3%"), "{out}");
        assert!(out.contains("NPS                       0.0"), "{out}");
    }

    #[test]
    fn every_scale_and_format_runs() {
        for scale in ["auto", "0-10", "1-5", "1-7", "1-10"] {
            let out =
                calculate("5,3,1", "csat", "values", scale, -1, "95", 1, true, "report").unwrap();
            assert!(out.contains("CSAT"), "{scale}: {out}");
        }
        for fmt in ["report", "json", "csv"] {
            let out = calculate("10,9,0", "nps", "values", "auto", -1, "95", 1, true, fmt).unwrap();
            assert!(!out.is_empty(), "{fmt}");
        }
    }

    #[test]
    fn json_and_csv_carry_the_same_numbers() {
        let j = calculate("10,9,9,8,7,6,5,10,9,3", "nps", "values", "auto", -1, "95", 1, false, "json")
            .unwrap();
        assert!(j.contains("\"metric\": \"nps\""), "{j}");
        assert!(j.contains("\"score\": 20.0"), "{j}");
        assert!(j.contains("\"label\": \"Promoters\", \"range\": \"9-10\", \"count\": 5"), "{j}");
        assert!(j.contains("\"confidence\": { \"level\": 95, \"margin\": 54.0"), "{j}");
        let c = calculate("10,9,9,8,7,6,5,10,9,3", "nps", "values", "auto", -1, "95", 1, false, "csv")
            .unwrap();
        assert!(c.starts_with("section,label,value,percent\nscore,nps,20.0,\n"), "{c}");
        assert!(c.contains("band,promoters 9-10,5,50.0\n"), "{c}");
    }

    #[test]
    fn distribution_can_be_turned_off() {
        assert!(nps("10,9,0").contains("Distribution"));
        let out =
            calculate("10,9,0", "nps", "values", "auto", -1, "95", 1, false, "report").unwrap();
        assert!(!out.contains("Distribution"), "{out}");
    }

    #[test]
    fn error_on_empty_input() {
        let e = calculate("", "nps", "values", "auto", -1, "95", 1, true, "report").unwrap_err();
        assert!(e.contains("no ratings found"), "{e}");
    }

    #[test]
    fn error_on_out_of_range_rating() {
        let e = calculate("10,11", "nps", "values", "auto", -1, "95", 1, true, "report").unwrap_err();
        assert!(e.contains("rating 11 is outside the 0-10 scale"), "{e}");
    }

    #[test]
    fn error_on_non_numeric_rating() {
        let e =
            calculate("10\n9\nnope\n", "nps", "values", "auto", -1, "95", 1, true, "report")
                .unwrap_err();
        assert!(e.contains("'nope' is not a number"), "{e}");
    }

    #[test]
    fn error_when_nps_is_asked_for_on_another_scale() {
        let e = calculate("5,4", "nps", "values", "1-5", -1, "95", 1, true, "report").unwrap_err();
        assert!(e.contains("NPS is defined on the 0-10 scale only"), "{e}");
    }

    #[test]
    fn error_on_bad_threshold_and_bad_enums() {
        let e = calculate("5,4", "csat", "values", "1-5", 9, "95", 1, true, "report").unwrap_err();
        assert!(e.contains("threshold must be between 2 and 5"), "{e}");
        let e = calculate("5,4", "wow", "values", "auto", -1, "95", 1, true, "report").unwrap_err();
        assert!(e.contains("metric must be one of"), "{e}");
        let e = calculate("5,4", "csat", "values", "1-5", -1, "80", 1, true, "report").unwrap_err();
        assert!(e.contains("confidence must be one of"), "{e}");
        let e = calculate("5,4", "csat", "values", "1-5", -1, "95", 1, true, "pdf").unwrap_err();
        assert!(e.contains("format must be one of"), "{e}");
        let e = calculate("5,4", "csat", "nope", "1-5", -1, "95", 1, true, "report").unwrap_err();
        assert!(e.contains("input must be one of"), "{e}");
    }

    #[test]
    fn error_on_malformed_counts_row() {
        let e = calculate("10,2,3\n", "nps", "counts", "auto", -1, "95", 1, true, "report")
            .unwrap_err();
        assert!(e.contains("counts rows need exactly two values"), "{e}");
    }

    #[test]
    fn response_cap_is_the_advertised_boundary() {
        let at_cap = "9\n".repeat(MAX_RESPONSES);
        let out =
            calculate(&at_cap, "nps", "values", "auto", -1, "none", 0, false, "report").unwrap();
        assert!(out.contains(&format!("Responses                 {MAX_RESPONSES}")), "{out}");
        let over = format!("{at_cap}9\n");
        let e =
            calculate(&over, "nps", "values", "auto", -1, "none", 0, false, "report").unwrap_err();
        assert!(e.contains("too many responses"), "{e}");
    }

    #[test]
    fn single_response_reports_no_confidence_band() {
        let out = calculate("10", "nps", "values", "auto", -1, "95", 1, false, "report").unwrap();
        assert!(out.contains("Confidence band           needs at least 2 responses"), "{out}");
    }
}
