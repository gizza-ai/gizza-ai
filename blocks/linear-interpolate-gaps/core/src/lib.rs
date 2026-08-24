//! linear-interpolate-gaps core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Fills missing values in an ordered numeric series by drawing a straight line between the two
//! nearest known neighbours. Optional controls mirror the reference implementations users already
//! know: a max-gap limit (how many consecutive blanks may be filled), a fill direction, and an
//! explicit policy for gaps that sit outside the known range (leading/trailing runs).

/// Hard cap on how many values a single run may contain.
pub const MAX_POINTS: usize = 100_000;

/// Tokens treated as "missing" without any extra configuration (case-insensitive, after trimming).
pub const DEFAULT_NA_TOKENS: [&str; 10] = ["", "na", "n/a", "nan", "null", "none", "nil", "-", "--", "?"];

#[derive(Debug, Clone)]
pub struct Options {
    /// "auto" | "values" | "xy"
    pub layout: String,
    /// Longest run of consecutive missing values that may be filled. 0 = no limit.
    pub max_gap: usize,
    /// "both" | "forward" | "backward"
    pub direction: String,
    /// "leave" | "hold" | "extrapolate"
    pub edges: String,
    /// Extra comma-separated tokens that also count as missing.
    pub na_tokens: String,
    /// Decimal places for interpolated values (0-12).
    pub decimals: usize,
    /// "values" | "csv" | "json"
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            layout: "auto".into(),
            max_gap: 0,
            direction: "both".into(),
            edges: "leave".into(),
            na_tokens: String::new(),
            decimals: 6,
            output: "values".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Status {
    Known,
    Filled,
    Missing,
}

struct Gap {
    start: usize, // 1-based, inclusive
    end: usize,   // 1-based, inclusive
    kind: &'static str,
    filled: usize,
    reason: Option<String>,
}

/// Fill the gaps in `input` and render the requested output format.
pub fn interpolate_gaps(input: &str, opts: &Options) -> Result<String, String> {
    let layout = pick(&opts.layout, "layout", &["auto", "values", "xy"])?;
    let direction = pick(&opts.direction, "direction", &["both", "forward", "backward"])?;
    let edges = pick(&opts.edges, "edges", &["leave", "hold", "extrapolate"])?;
    let output = pick(&opts.output, "output", &["values", "csv", "json"])?;
    if opts.decimals > 12 {
        return Err(format!(
            "decimals must be between 0 and 12, got {}",
            opts.decimals
        ));
    }

    let text = input.trim();
    if text.is_empty() {
        return Err("input is empty: paste at least one value (blanks or NA markers mark the gaps)".into());
    }

    let na = na_set(&opts.na_tokens);
    let rows: Vec<&str> = text.split('\n').map(|r| r.trim_end_matches('\r')).collect();
    let fields: Vec<Vec<String>> = rows.iter().map(|r| split_fields(r)).collect();

    let xy = match layout {
        "xy" => true,
        "values" => false,
        // Auto: two or more rows that ALL carry exactly two fields read as x,y pairs.
        _ => fields.len() >= 2 && fields.iter().all(|f| f.len() == 2),
    };

    let (raw_x, raw_y, xs, ys) = if xy {
        parse_xy(&fields, &na)?
    } else {
        parse_values(&fields, &na)?
    };

    if ys.len() > MAX_POINTS {
        return Err(format!(
            "too many values: {} (max {}). Split the series and run it in parts.",
            ys.len(),
            MAX_POINTS
        ));
    }

    let known: Vec<usize> = ys
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_some())
        .map(|(i, _)| i)
        .collect();
    if known.is_empty() {
        return Err(
            "every value is missing: linear interpolation needs at least one known value to anchor on"
                .into(),
        );
    }

    let n = ys.len();
    let mut out: Vec<Option<f64>> = ys.clone();
    let mut status: Vec<Status> = ys
        .iter()
        .map(|v| if v.is_some() { Status::Known } else { Status::Missing })
        .collect();
    let mut gaps: Vec<Gap> = Vec::new();

    let fwd = direction == "both" || direction == "forward";
    let bwd = direction == "both" || direction == "backward";
    let cap = |len: usize| if opts.max_gap == 0 { len } else { opts.max_gap.min(len) };

    // ---- interior gaps: both neighbours are known, so the straight line is defined ----
    for w in known.windows(2) {
        let (a, b) = (w[0], w[1]);
        if b == a + 1 {
            continue;
        }
        let len = b - a - 1;
        let allowed = cap(len);
        let mut take = vec![false; len];
        if fwd {
            for slot in take.iter_mut().take(allowed) {
                *slot = true;
            }
        }
        if bwd {
            for k in 0..allowed {
                take[len - 1 - k] = true;
            }
        }
        let (x0, y0) = (xs[a], ys[a].unwrap());
        let (x1, y1) = (xs[b], ys[b].unwrap());
        let mut filled = 0usize;
        for (k, want) in take.iter().enumerate() {
            if *want {
                let i = a + 1 + k;
                out[i] = Some(lerp(x0, y0, x1, y1, xs[i]));
                status[i] = Status::Filled;
                filled += 1;
            }
        }
        gaps.push(Gap {
            start: a + 2,
            end: b,
            kind: "interior",
            filled,
            reason: if filled < len {
                Some(format!(
                    "run of {len} consecutive gaps exceeds max_gap {}",
                    opts.max_gap
                ))
            } else {
                None
            },
        });
    }

    // ---- leading gap: no earlier anchor, so it depends on `edges` + a backward direction ----
    let first = known[0];
    if first > 0 {
        let len = first;
        let mut filled = 0usize;
        let mut reason = None;
        if edges == "leave" {
            reason = Some("leading gap left as-is (edges=leave)".to_string());
        } else if !bwd {
            reason = Some(format!(
                "leading gap needs a backward fill (direction={direction})"
            ));
        } else {
            let allowed = cap(len);
            for k in 0..allowed {
                let i = first - 1 - k;
                out[i] = Some(edge_value(edges, &known, &xs, &ys, xs[i], true));
                status[i] = Status::Filled;
                filled += 1;
            }
            if filled < len {
                reason = Some(format!(
                    "run of {len} consecutive gaps exceeds max_gap {}",
                    opts.max_gap
                ));
            }
        }
        gaps.insert(
            0,
            Gap {
                start: 1,
                end: len,
                kind: "leading",
                filled,
                reason,
            },
        );
    }

    // ---- trailing gap: no later anchor, so it depends on `edges` + a forward direction ----
    let last = *known.last().unwrap();
    if last + 1 < n {
        let len = n - last - 1;
        let mut filled = 0usize;
        let mut reason = None;
        if edges == "leave" {
            reason = Some("trailing gap left as-is (edges=leave)".to_string());
        } else if !fwd {
            reason = Some(format!(
                "trailing gap needs a forward fill (direction={direction})"
            ));
        } else {
            let allowed = cap(len);
            for k in 0..allowed {
                let i = last + 1 + k;
                out[i] = Some(edge_value(edges, &known, &xs, &ys, xs[i], false));
                status[i] = Status::Filled;
                filled += 1;
            }
            if filled < len {
                reason = Some(format!(
                    "run of {len} consecutive gaps exceeds max_gap {}",
                    opts.max_gap
                ));
            }
        }
        gaps.push(Gap {
            start: last + 2,
            end: n,
            kind: "trailing",
            filled,
            reason,
        });
    }

    let d = opts.decimals;
    let cell = |i: usize| -> String {
        match status[i] {
            Status::Known => raw_y[i].clone(),
            Status::Filled => fmt_num(out[i].unwrap(), d),
            Status::Missing => String::new(),
        }
    };

    Ok(match output {
        "values" => (0..n)
            .map(|i| {
                let v = if status[i] == Status::Missing {
                    raw_y[i].clone()
                } else {
                    cell(i)
                };
                if xy {
                    format!("{},{}", raw_x[i], v)
                } else {
                    v
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        "csv" => {
            let mut s = String::from(if xy { "x,y,status\n" } else { "index,value,status\n" });
            for i in 0..n {
                let key = if xy {
                    raw_x[i].clone()
                } else {
                    (i + 1).to_string()
                };
                s.push_str(&format!("{key},{},{}\n", cell(i), status_name(status[i])));
            }
            s
        }
        _ => render_json(&status, &out, &xs, &gaps, xy, opts, d),
    })
}

fn status_name(s: Status) -> &'static str {
    match s {
        Status::Known => "known",
        Status::Filled => "filled",
        Status::Missing => "missing",
    }
}

fn render_json(
    status: &[Status],
    out: &[Option<f64>],
    xs: &[f64],
    gaps: &[Gap],
    xy: bool,
    opts: &Options,
    d: usize,
) -> String {
    let n = status.len();
    let known = status.iter().filter(|s| **s == Status::Known).count();
    let filled = status.iter().filter(|s| **s == Status::Filled).count();
    let missing = n - known - filled;
    let mut s = String::from("{\n");
    s.push_str(&format!("  \"count\": {n},\n"));
    s.push_str(&format!("  \"known\": {known},\n"));
    s.push_str(&format!("  \"filled\": {filled},\n"));
    s.push_str(&format!("  \"missing\": {missing},\n"));
    s.push_str(&format!("  \"max_gap\": {},\n", opts.max_gap));
    s.push_str(&format!("  \"direction\": \"{}\",\n", opts.direction.trim()));
    s.push_str(&format!("  \"edges\": \"{}\",\n", opts.edges.trim()));
    if xy {
        s.push_str(&format!(
            "  \"x\": [{}],\n",
            xs.iter()
                .map(|v| fmt_num(*v, d))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    s.push_str(&format!(
        "  \"values\": [{}],\n",
        (0..n)
            .map(|i| match out[i] {
                Some(v) => fmt_num(v, d),
                None => "null".to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    ));
    s.push_str("  \"gaps\": [");
    if gaps.is_empty() {
        s.push_str("]\n}");
        return s;
    }
    s.push('\n');
    let last = gaps.len() - 1;
    for (i, g) in gaps.iter().enumerate() {
        let len = g.end - g.start + 1;
        let st = if g.filled == 0 {
            "skipped"
        } else if g.filled < len {
            "partial"
        } else {
            "filled"
        };
        s.push_str(&format!(
            "    {{ \"start\": {}, \"end\": {}, \"length\": {len}, \"kind\": \"{}\", \"filled\": {}, \"status\": \"{st}\"",
            g.start, g.end, g.kind, g.filled
        ));
        if let Some(r) = &g.reason {
            s.push_str(&format!(", \"reason\": \"{}\"", r));
        }
        s.push_str(" }");
        if i != last {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ]\n}");
    s
}

fn lerp(x0: f64, y0: f64, x1: f64, y1: f64, x: f64) -> f64 {
    y0 + (x - x0) * (y1 - y0) / (x1 - x0)
}

/// Value for a gap that sits outside the known range: hold the nearest known value, or extend the
/// line through the two nearest known points. With a single known point, extrapolate degrades to
/// hold — there is no slope to extend.
fn edge_value(
    edges: &str,
    known: &[usize],
    xs: &[f64],
    ys: &[Option<f64>],
    x: f64,
    leading: bool,
) -> f64 {
    let anchor = if leading { known[0] } else { known[known.len() - 1] };
    if edges == "hold" || known.len() < 2 {
        return ys[anchor].unwrap();
    }
    let other = if leading {
        known[1]
    } else {
        known[known.len() - 2]
    };
    lerp(
        xs[other],
        ys[other].unwrap(),
        xs[anchor],
        ys[anchor].unwrap(),
        x,
    )
}

fn fmt_num(v: f64, decimals: usize) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let s = format!("{:.*}", decimals, v);
    if s.contains('.') {
        let t = s.trim_end_matches('0').trim_end_matches('.');
        if t.is_empty() || t == "-" {
            "0".to_string()
        } else {
            t.to_string()
        }
    } else {
        s
    }
}

fn pick<'a>(value: &'a str, name: &str, allowed: &[&'a str]) -> Result<&'a str, String> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(allowed[0]);
    }
    allowed
        .iter()
        .find(|a| **a == v)
        .copied()
        .ok_or_else(|| format!("{name} must be one of {}, got '{v}'", allowed.join(", ")))
}

fn na_set(extra: &str) -> Vec<String> {
    let mut set: Vec<String> = DEFAULT_NA_TOKENS.iter().map(|s| s.to_string()).collect();
    for t in extra.split(',') {
        let t = t.trim().to_ascii_lowercase();
        if !t.is_empty() && !set.contains(&t) {
            set.push(t);
        }
    }
    set
}

fn is_missing(token: &str, na: &[String]) -> bool {
    na.contains(&token.trim().to_ascii_lowercase())
}

/// Split one row into fields: commas/semicolons/tabs win (so empty fields survive); otherwise a
/// whitespace-separated row is split on runs of whitespace.
fn split_fields(row: &str) -> Vec<String> {
    if row.contains(',') || row.contains(';') || row.contains('\t') {
        row.split([',', ';', '\t'])
            .map(|s| s.trim().to_string())
            .collect()
    } else {
        let t = row.trim();
        if t.is_empty() {
            vec![String::new()]
        } else {
            t.split_whitespace().map(|s| s.to_string()).collect()
        }
    }
}

type Parsed = (Vec<String>, Vec<String>, Vec<f64>, Vec<Option<f64>>);

fn parse_values(fields: &[Vec<String>], na: &[String]) -> Result<Parsed, String> {
    let mut raw: Vec<String> = Vec::new();
    for f in fields {
        for token in f {
            raw.push(token.clone());
        }
    }
    // Optional header: a leading non-numeric, non-NA label followed by real numbers.
    if raw.len() >= 2
        && !is_missing(&raw[0], na)
        && raw[0].parse::<f64>().is_err()
        && raw[1..].iter().any(|t| t.parse::<f64>().is_ok())
    {
        raw.remove(0);
    }
    let mut ys: Vec<Option<f64>> = Vec::with_capacity(raw.len());
    for (i, token) in raw.iter().enumerate() {
        if is_missing(token, na) {
            ys.push(None);
        } else {
            ys.push(Some(token.parse::<f64>().map_err(|_| {
                format!(
                    "value {}: expected a number or a blank/NA marker, got '{token}'",
                    i + 1
                )
            })?));
        }
    }
    let xs: Vec<f64> = (0..ys.len()).map(|i| (i + 1) as f64).collect();
    Ok((Vec::new(), raw, xs, ys))
}

fn parse_xy(fields: &[Vec<String>], na: &[String]) -> Result<Parsed, String> {
    let mut rows: Vec<(usize, &Vec<String>)> = fields.iter().enumerate().collect();
    // Optional header row: both cells are non-numeric labels.
    if let Some((_, f)) = rows.first() {
        if f.len() == 2 && f[0].parse::<f64>().is_err() && f[1].parse::<f64>().is_err() {
            rows.remove(0);
        }
    }
    if rows.is_empty() {
        return Err("no data rows after the header: add at least one x,y row".into());
    }
    let mut raw_x = Vec::with_capacity(rows.len());
    let mut raw_y = Vec::with_capacity(rows.len());
    let mut xs = Vec::with_capacity(rows.len());
    let mut ys = Vec::with_capacity(rows.len());
    for (ri, f) in rows {
        if f.len() != 2 {
            return Err(format!(
                "row {}: expected exactly 2 fields (x,y), got {}",
                ri + 1,
                f.len()
            ));
        }
        let x = f[0].parse::<f64>().map_err(|_| {
            format!(
                "row {}: x must be a number (x values cannot be missing), got '{}'",
                ri + 1,
                f[0]
            )
        })?;
        if !x.is_finite() {
            return Err(format!("row {}: x must be a finite number", ri + 1));
        }
        if let Some(prev) = xs.last() {
            if x <= *prev {
                return Err(format!(
                    "row {}: x values must be strictly increasing, got {x} after {prev}",
                    ri + 1
                ));
            }
        }
        let y = if is_missing(&f[1], na) {
            None
        } else {
            Some(f[1].parse::<f64>().map_err(|_| {
                format!(
                    "row {}: y must be a number or a blank/NA marker, got '{}'",
                    ri + 1,
                    f[1]
                )
            })?)
        };
        raw_x.push(f[0].clone());
        raw_y.push(f[1].clone());
        xs.push(x);
        ys.push(y);
    }
    Ok((raw_x, raw_y, xs, ys))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(pairs: &[(&str, &str)]) -> Options {
        let mut o = Options::default();
        for (k, v) in pairs {
            match *k {
                "layout" => o.layout = v.to_string(),
                "max_gap" => o.max_gap = v.parse().unwrap(),
                "direction" => o.direction = v.to_string(),
                "edges" => o.edges = v.to_string(),
                "na_tokens" => o.na_tokens = v.to_string(),
                "decimals" => o.decimals = v.parse().unwrap(),
                "output" => o.output = v.to_string(),
                other => panic!("unknown option {other}"),
            }
        }
        o
    }

    #[test]
    fn fills_a_single_interior_gap() {
        let got = interpolate_gaps("1,,3", &Options::default()).unwrap();
        assert_eq!(got, "1\n2\n3");
    }

    #[test]
    fn fills_a_multi_step_gap_evenly() {
        let got = interpolate_gaps("10\n\n\n\n20", &Options::default()).unwrap();
        assert_eq!(got, "10\n12.5\n15\n17.5\n20");
    }

    #[test]
    fn keeps_known_values_verbatim() {
        // 1.00 stays as typed; only the interpolated cell is formatted.
        let got = interpolate_gaps("1.00, NA, 2.00", &Options::default()).unwrap();
        assert_eq!(got, "1.00\n1.5\n2.00");
    }

    #[test]
    fn max_gap_leaves_long_runs_alone() {
        // A run of 3 with max_gap=1 and direction=both fills one from each side.
        let got = interpolate_gaps("0,,,,4", &opts(&[("max_gap", "1")])).unwrap();
        assert_eq!(got, "0\n1\n\n3\n4");
    }

    #[test]
    fn max_gap_admits_a_run_at_the_boundary() {
        let got = interpolate_gaps("0,,,,4", &opts(&[("max_gap", "3")])).unwrap();
        assert_eq!(got, "0\n1\n2\n3\n4");
    }

    #[test]
    fn direction_forward_fills_only_from_the_left() {
        let got = interpolate_gaps(
            "0,,,,4",
            &opts(&[("max_gap", "1"), ("direction", "forward")]),
        )
        .unwrap();
        assert_eq!(got, "0\n1\n\n\n4");
    }

    #[test]
    fn direction_backward_fills_only_from_the_right() {
        let got = interpolate_gaps(
            "0,,,,4",
            &opts(&[("max_gap", "1"), ("direction", "backward")]),
        )
        .unwrap();
        assert_eq!(got, "0\n\n\n3\n4");
    }

    #[test]
    fn edges_default_leaves_outside_gaps_untouched() {
        let got = interpolate_gaps(",,2,4,,", &Options::default()).unwrap();
        assert_eq!(got, "\n\n2\n4\n\n");
    }

    #[test]
    fn edges_hold_repeats_the_nearest_known_value() {
        let got = interpolate_gaps(",,2,4,,", &opts(&[("edges", "hold")])).unwrap();
        assert_eq!(got, "2\n2\n2\n4\n4\n4");
    }

    #[test]
    fn edges_extrapolate_extends_the_end_slope() {
        let got = interpolate_gaps(",,2,4,,", &opts(&[("edges", "extrapolate")])).unwrap();
        assert_eq!(got, "-2\n0\n2\n4\n6\n8");
    }

    #[test]
    fn edges_extrapolate_degrades_to_hold_with_one_known_point() {
        let got = interpolate_gaps(",5,", &opts(&[("edges", "extrapolate")])).unwrap();
        assert_eq!(got, "5\n5\n5");
    }

    #[test]
    fn xy_layout_uses_real_x_spacing() {
        // The gap sits at x=4 between (1,10) and (5,50): 10 + 3*(40/4) = 40.
        let got = interpolate_gaps("1,10\n4,\n5,50", &Options::default()).unwrap();
        assert_eq!(got, "1,10\n4,40\n5,50");
    }

    #[test]
    fn xy_layout_skips_a_header_row() {
        let got = interpolate_gaps("day,reading\n1,10\n2,\n3,30", &Options::default()).unwrap();
        assert_eq!(got, "1,10\n2,20\n3,30");
    }

    #[test]
    fn values_layout_can_be_forced_over_the_xy_heuristic() {
        let got = interpolate_gaps("1,3\n5,7", &opts(&[("layout", "values")])).unwrap();
        assert_eq!(got, "1\n3\n5\n7");
    }

    #[test]
    fn custom_na_tokens_are_recognised() {
        let got = interpolate_gaps("1, MISSING, 3", &opts(&[("na_tokens", "missing")])).unwrap();
        assert_eq!(got, "1\n2\n3");
    }

    #[test]
    fn decimals_round_interpolated_values() {
        let got = interpolate_gaps("0,,1", &opts(&[("decimals", "2")])).unwrap();
        assert_eq!(got, "0\n0.5\n1");
        let got = interpolate_gaps("0,,,1", &opts(&[("decimals", "2")])).unwrap();
        assert_eq!(got, "0\n0.33\n0.67\n1");
    }

    #[test]
    fn whitespace_separated_single_line_parses() {
        let got = interpolate_gaps("1 NA 3", &Options::default()).unwrap();
        assert_eq!(got, "1\nNA\n3".replace("NA", "2"));
    }

    #[test]
    fn csv_output_flags_every_row() {
        let got = interpolate_gaps("1,,3", &opts(&[("output", "csv")])).unwrap();
        assert_eq!(
            got,
            "index,value,status\n1,1,known\n2,2,filled\n3,3,known\n"
        );
    }

    #[test]
    fn json_output_reports_gap_runs() {
        let got =
            interpolate_gaps("1,,,,5,,", &opts(&[("output", "json"), ("max_gap", "1")])).unwrap();
        assert!(got.contains("\"count\": 7"), "{got}");
        assert!(got.contains("\"known\": 2,"), "{got}");
        assert!(got.contains("\"missing\": 3,"), "{got}");
        assert!(
            got.contains("\"values\": [1, 2, null, 4, 5, null, null]"),
            "{got}"
        );
        assert!(got.contains("\"kind\": \"interior\""), "{got}");
        assert!(got.contains("\"status\": \"partial\""), "{got}");
        assert!(got.contains("\"kind\": \"trailing\""), "{got}");
        assert!(got.contains("edges=leave"), "{got}");
    }

    #[test]
    fn json_output_includes_x_for_xy_layout() {
        let got = interpolate_gaps("1,10\n4,\n5,50", &opts(&[("output", "json")])).unwrap();
        assert!(got.contains("\"x\": [1, 4, 5]"), "{got}");
        assert!(got.contains("\"values\": [10, 40, 50]"), "{got}");
    }

    #[test]
    fn accepts_the_cap_boundary_and_rejects_one_over() {
        let at_cap: String = (0..MAX_POINTS)
            .map(|i| if i % 2 == 1 { String::new() } else { i.to_string() })
            .collect::<Vec<_>>()
            .join(",");
        assert!(interpolate_gaps(&at_cap, &Options::default()).is_ok());
        let over = format!("{at_cap},1");
        let err = interpolate_gaps(&over, &Options::default()).unwrap_err();
        assert!(err.contains("too many values: 100001"), "{err}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = interpolate_gaps("   \n  ", &Options::default()).unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn rejects_a_non_numeric_value() {
        let err = interpolate_gaps("1, oops, 3", &Options::default()).unwrap_err();
        assert_eq!(
            err,
            "value 2: expected a number or a blank/NA marker, got 'oops'"
        );
    }

    #[test]
    fn rejects_an_all_missing_series() {
        let err = interpolate_gaps(",,,", &Options::default()).unwrap_err();
        assert!(err.contains("every value is missing"), "{err}");
    }

    #[test]
    fn rejects_non_increasing_x() {
        let err = interpolate_gaps("1,10\n5,20\n3,30", &Options::default()).unwrap_err();
        assert!(
            err.contains("x values must be strictly increasing"),
            "{err}"
        );
    }

    #[test]
    fn rejects_a_missing_x() {
        let err = interpolate_gaps("1,10\n,20\n3,30", &opts(&[("layout", "xy")])).unwrap_err();
        assert!(err.contains("x values cannot be missing"), "{err}");
    }

    #[test]
    fn rejects_an_unknown_enum_value() {
        let err = interpolate_gaps("1,,3", &opts(&[("direction", "sideways")])).unwrap_err();
        assert_eq!(
            err,
            "direction must be one of both, forward, backward, got 'sideways'"
        );
    }
}
