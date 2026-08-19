//! likert-summary core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Summarizes Likert-scale survey answers. Two input shapes:
//!   * `responses` — a CSV whose first row is the item (question) headers and each
//!                   later row is one respondent's answers, given either as scale
//!                   codes (1..points) or as the scale labels themselves.
//!   * `counts`    — one row per item: the item name, then a tally of how many
//!                   respondents chose each of the `points` scale categories.
//!
//! Output is a monospaced report: a per-item table (n, missing, mean, SD, median,
//! mode, bottom-box / neutral / top-box percentages), the full response
//! distribution, optional stacked bars (plain or neutral-centred/diverging),
//! floor/ceiling flags, the overall mean of item means, and optional Cronbach's
//! alpha.

/// Cells equal to one of these (case-insensitively), or empty, count as missing.
const MISSING_MARKERS: [&str; 8] = ["na", "n/a", "-", ".", "none", "null", "missing", "?"];

/// Width, in characters, of a full-scale (100%) stacked bar.
const BAR_WIDTH: usize = 40;

/// A category share at or above this fraction of an item's valid responses in the
/// lowest/highest category is reported as a floor/ceiling effect (a common rule of
/// thumb in scale-validation work).
const FLOOR_CEILING_PCT: f64 = 15.0;

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Built-in label sets for the named scale families, for 2..=7 points.
fn preset_labels(scale: &str, points: usize) -> Result<Vec<String>, String> {
    let set: &[&str] = match (scale, points) {
        ("agreement", 2) => &["Disagree", "Agree"],
        ("agreement", 3) => &["Disagree", "Neutral", "Agree"],
        ("agreement", 4) => &["Strongly disagree", "Disagree", "Agree", "Strongly agree"],
        ("agreement", 5) => &[
            "Strongly disagree",
            "Disagree",
            "Neutral",
            "Agree",
            "Strongly agree",
        ],
        ("agreement", 6) => &[
            "Strongly disagree",
            "Disagree",
            "Somewhat disagree",
            "Somewhat agree",
            "Agree",
            "Strongly agree",
        ],
        ("agreement", 7) => &[
            "Strongly disagree",
            "Disagree",
            "Somewhat disagree",
            "Neutral",
            "Somewhat agree",
            "Agree",
            "Strongly agree",
        ],
        ("satisfaction", 2) => &["Dissatisfied", "Satisfied"],
        ("satisfaction", 3) => &["Dissatisfied", "Neutral", "Satisfied"],
        ("satisfaction", 4) => &[
            "Very dissatisfied",
            "Dissatisfied",
            "Satisfied",
            "Very satisfied",
        ],
        ("satisfaction", 5) => &[
            "Very dissatisfied",
            "Dissatisfied",
            "Neutral",
            "Satisfied",
            "Very satisfied",
        ],
        ("satisfaction", 6) => &[
            "Very dissatisfied",
            "Dissatisfied",
            "Somewhat dissatisfied",
            "Somewhat satisfied",
            "Satisfied",
            "Very satisfied",
        ],
        ("satisfaction", 7) => &[
            "Very dissatisfied",
            "Dissatisfied",
            "Somewhat dissatisfied",
            "Neutral",
            "Somewhat satisfied",
            "Satisfied",
            "Very satisfied",
        ],
        ("frequency", 2) => &["Never", "Always"],
        ("frequency", 3) => &["Never", "Sometimes", "Always"],
        ("frequency", 4) => &["Never", "Rarely", "Often", "Always"],
        ("frequency", 5) => &["Never", "Rarely", "Sometimes", "Often", "Always"],
        ("frequency", 6) => &[
            "Never",
            "Very rarely",
            "Rarely",
            "Sometimes",
            "Often",
            "Always",
        ],
        ("frequency", 7) => &[
            "Never",
            "Very rarely",
            "Rarely",
            "Sometimes",
            "Often",
            "Very often",
            "Always",
        ],
        ("quality", 2) => &["Poor", "Good"],
        ("quality", 3) => &["Poor", "Fair", "Good"],
        ("quality", 4) => &["Poor", "Fair", "Good", "Excellent"],
        ("quality", 5) => &["Very poor", "Poor", "Fair", "Good", "Excellent"],
        ("quality", 6) => &[
            "Very poor",
            "Poor",
            "Fair",
            "Good",
            "Very good",
            "Excellent",
        ],
        ("quality", 7) => &[
            "Very poor",
            "Poor",
            "Below average",
            "Average",
            "Above average",
            "Good",
            "Excellent",
        ],
        (s, p) => {
            return Err(format!(
                "the '{s}' scale has built-in labels for 2-7 points only (asked for {p}); use \
                 scale=numeric or scale=custom with labels=..."
            ))
        }
    };
    Ok(set.iter().map(|s| s.to_string()).collect())
}

fn resolve_labels(scale: &str, labels: &str, points: usize) -> Result<Vec<String>, String> {
    let custom: Vec<String> = labels
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if !custom.is_empty() {
        if custom.len() != points {
            return Err(format!(
                "labels lists {} value(s) but the scale has {points} points — give one label per \
                 point, lowest first",
                custom.len()
            ));
        }
        return Ok(custom);
    }
    match scale {
        "custom" => Err(
            "scale=custom needs labels=... (one label per scale point, lowest first)".to_string(),
        ),
        "numeric" => Ok((1..=points).map(|i| i.to_string()).collect()),
        other => preset_labels(other, points),
    }
}

/// Read the input into non-empty rows of trimmed cells.
fn parse_rows(data: &str, delim: u8) -> Result<Vec<Vec<String>>, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .has_headers(false)
        .from_reader(data.as_bytes());
    let mut rows = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("could not parse the input as delimited text: {e}"))?;
        let cells: Vec<String> = rec.iter().map(|c| c.trim().to_string()).collect();
        if cells.iter().all(|c| c.is_empty()) {
            continue;
        }
        rows.push(cells);
    }
    if rows.is_empty() {
        return Err("no data rows found — paste a header row plus at least one data row".into());
    }
    Ok(rows)
}

fn is_missing(cell: &str) -> bool {
    let c = cell.trim();
    c.is_empty() || MISSING_MARKERS.contains(&c.to_ascii_lowercase().as_str())
}

/// Resolve one column reference (1-based index OR header name) against a header row.
fn resolve_col(name: &str, header: &[String]) -> Result<usize, String> {
    let name = name.trim();
    if let Ok(n) = name.parse::<usize>() {
        if n == 0 {
            return Err("column index is 1-based (>= 1)".into());
        }
        if n > header.len() {
            return Err(format!(
                "column index {n} is out of range (the header has {} columns)",
                header.len()
            ));
        }
        return Ok(n - 1);
    }
    header
        .iter()
        .position(|c| c.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("column '{name}' not found in the header row"))
}

fn split_list(s: &str) -> Vec<&str> {
    s.split(',').map(|p| p.trim()).filter(|p| !p.is_empty()).collect()
}

/// Turn one raw answer into a 1-based scale code.
fn parse_code(cell: &str, labels: &[String], points: usize, item: &str) -> Result<usize, String> {
    let c = cell.trim();
    if let Ok(v) = c.parse::<f64>() {
        if v.fract() != 0.0 || v < 1.0 || v > points as f64 {
            return Err(format!(
                "'{c}' in item '{item}' is outside the 1-{points} scale — set points=... if your \
                 scale has a different number of categories"
            ));
        }
        return Ok(v as usize);
    }
    let lower = c.to_ascii_lowercase();
    if let Some(i) = labels.iter().position(|l| l.eq_ignore_ascii_case(c)) {
        return Ok(i + 1);
    }
    let hits: Vec<usize> = labels
        .iter()
        .enumerate()
        .filter(|(_, l)| l.to_ascii_lowercase().starts_with(&lower))
        .map(|(i, _)| i + 1)
        .collect();
    if hits.len() == 1 {
        return Ok(hits[0]);
    }
    Err(format!(
        "could not read '{c}' in item '{item}' as a scale answer — use 1-{points} or one of: {}. \
         If that column is not a Likert item (an ID or a comment), name the real ones with items=...",
        labels.join(", ")
    ))
}

struct Item {
    name: String,
    counts: Vec<u64>,
    missing: u64,
}

impl Item {
    fn n(&self) -> u64 {
        self.counts.iter().sum()
    }
    fn mean(&self) -> f64 {
        let n = self.n();
        if n == 0 {
            return 0.0;
        }
        let sum: f64 = self
            .counts
            .iter()
            .enumerate()
            .map(|(i, c)| (i as f64 + 1.0) * *c as f64)
            .sum();
        sum / n as f64
    }
    fn sd(&self) -> Option<f64> {
        let n = self.n();
        if n < 2 {
            return None;
        }
        let mean = self.mean();
        let ss: f64 = self
            .counts
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let d = (i as f64 + 1.0) - mean;
                d * d * *c as f64
            })
            .sum();
        Some((ss / (n as f64 - 1.0)).sqrt())
    }
    /// Value at the 1-based rank `k` in the sorted response list.
    fn nth(&self, k: u64) -> f64 {
        let mut seen = 0u64;
        for (i, c) in self.counts.iter().enumerate() {
            seen += c;
            if seen >= k {
                return i as f64 + 1.0;
            }
        }
        self.counts.len() as f64
    }
    fn median(&self) -> Option<f64> {
        let n = self.n();
        if n == 0 {
            return None;
        }
        if n % 2 == 1 {
            Some(self.nth(n / 2 + 1))
        } else {
            Some((self.nth(n / 2) + self.nth(n / 2 + 1)) / 2.0)
        }
    }
    fn modes(&self) -> Vec<usize> {
        let max = self.counts.iter().copied().max().unwrap_or(0);
        if max == 0 {
            return Vec::new();
        }
        self.counts
            .iter()
            .enumerate()
            .filter(|(_, c)| **c == max)
            .map(|(i, _)| i + 1)
            .collect()
    }
    fn pct(&self, code: usize) -> f64 {
        let n = self.n();
        if n == 0 {
            0.0
        } else {
            self.counts[code - 1] as f64 / n as f64 * 100.0
        }
    }
    fn box_pct(&self, codes: std::ops::RangeInclusive<usize>) -> f64 {
        codes.map(|c| self.pct(c)).sum()
    }
}

fn round_to(v: f64, decimals: usize) -> String {
    format!("{v:.decimals$}")
}

/// Split `width` characters across `pcts` (which sum to ~100) by largest remainder,
/// so the rendered bar is exactly `width` characters wide.
fn allocate(pcts: &[f64], width: usize) -> Vec<usize> {
    let exact: Vec<f64> = pcts.iter().map(|p| p / 100.0 * width as f64).collect();
    let mut out: Vec<usize> = exact.iter().map(|e| e.floor() as usize).collect();
    let mut left = width.saturating_sub(out.iter().sum::<usize>());
    let mut order: Vec<usize> = (0..pcts.len()).collect();
    order.sort_by(|a, b| {
        let ra = exact[*a] - exact[*a].floor();
        let rb = exact[*b] - exact[*b].floor();
        rb.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(b))
    });
    for i in order {
        if left == 0 {
            break;
        }
        out[i] += 1;
        left -= 1;
    }
    out
}

/// Character drawn for scale code `code` in a stacked bar: 1-9 then a, b.
fn bar_char(code: usize) -> char {
    if code <= 9 {
        (b'0' + code as u8) as char
    } else {
        (b'a' + (code - 10) as u8) as char
    }
}

fn pad(s: &str, w: usize) -> String {
    let len = s.chars().count();
    if len >= w {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(w - len))
    }
}

fn rpad_left(s: &str, w: usize) -> String {
    let len = s.chars().count();
    if len >= w {
        s.to_string()
    } else {
        format!("{}{s}", " ".repeat(w - len))
    }
}

fn truncate(s: &str, w: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= w {
        s.to_string()
    } else {
        chars[..w.saturating_sub(1)].iter().collect::<String>() + "…"
    }
}

#[allow(clippy::too_many_arguments)]
pub fn summarize(
    data: &str,
    input: &str,
    items: &str,
    points: i64,
    scale: &str,
    labels: &str,
    reverse: &str,
    box_size: i64,
    missing: &str,
    sort: &str,
    decimals: i64,
    chart: bool,
    diverging: bool,
    alpha: bool,
    delimiter: &str,
) -> Result<String, String> {
    // ---- validate options -------------------------------------------------
    let input = if input.trim().is_empty() { "responses" } else { input.trim() };
    if !matches!(input, "responses" | "counts") {
        return Err(format!("input must be 'responses' or 'counts', got '{input}'"));
    }
    if !(2..=11).contains(&points) {
        return Err(format!("points must be between 2 and 11, got {points}"));
    }
    let points = points as usize;
    let scale = if scale.trim().is_empty() { "agreement" } else { scale.trim() };
    if !matches!(
        scale,
        "agreement" | "satisfaction" | "frequency" | "quality" | "numeric" | "custom"
    ) {
        return Err(format!(
            "scale must be agreement, satisfaction, frequency, quality, numeric or custom, got \
             '{scale}'"
        ));
    }
    let max_box = points / 2;
    if box_size < 1 || box_size as usize > max_box {
        return Err(format!(
            "box_size must be between 1 and {max_box} for a {points}-point scale, got {box_size}"
        ));
    }
    let box_size = box_size as usize;
    let missing_mode = if missing.trim().is_empty() { "exclude" } else { missing.trim() };
    if !matches!(missing_mode, "exclude" | "listwise") {
        return Err(format!(
            "missing must be 'exclude' or 'listwise', got '{missing_mode}'"
        ));
    }
    let sort = if sort.trim().is_empty() { "input" } else { sort.trim() };
    if !matches!(sort, "input" | "mean-desc" | "mean-asc" | "top-desc") {
        return Err(format!(
            "sort must be input, mean-desc, mean-asc or top-desc, got '{sort}'"
        ));
    }
    if !(0..=6).contains(&decimals) {
        return Err(format!("decimals must be between 0 and 6, got {decimals}"));
    }
    let decimals = decimals as usize;
    let labels = resolve_labels(scale, labels, points)?;
    let delim = delim_byte(delimiter)?;
    let rows = parse_rows(data, delim)?;

    // ---- build per-item counts -------------------------------------------
    let mut stats: Vec<Item>;
    let mut respondents = 0usize;
    let mut dropped_rows = 0usize;
    // Respondent-level codes per item, kept for Cronbach's alpha (responses mode only).
    let mut matrix: Vec<Vec<Option<usize>>> = Vec::new();

    if input == "responses" {
        let header = &rows[0];
        let picked: Vec<usize> = if split_list(items).is_empty() {
            (0..header.len()).collect()
        } else {
            let mut out = Vec::new();
            for name in split_list(items) {
                let idx = resolve_col(name, header)?;
                if !out.contains(&idx) {
                    out.push(idx);
                }
            }
            out
        };
        if picked.is_empty() {
            return Err("no item columns selected".into());
        }
        let mut reversed = vec![false; picked.len()];
        for name in split_list(reverse) {
            let idx = resolve_col(name, header)?;
            match picked.iter().position(|p| *p == idx) {
                Some(i) => reversed[i] = true,
                None => {
                    return Err(format!(
                        "reverse names column '{name}', which is not one of the selected items"
                    ))
                }
            }
        }
        stats = picked
            .iter()
            .map(|i| Item {
                name: if header[*i].is_empty() {
                    format!("column {}", i + 1)
                } else {
                    header[*i].clone()
                },
                counts: vec![0; points],
                missing: 0,
            })
            .collect();
        if rows.len() < 2 {
            return Err(
                "the input has a header row but no respondent rows — add one row per respondent, \
                 or switch input to 'counts'"
                    .into(),
            );
        }
        for row in &rows[1..] {
            let mut codes: Vec<Option<usize>> = Vec::with_capacity(picked.len());
            for (slot, col) in picked.iter().enumerate() {
                let cell = row.get(*col).map(|s| s.as_str()).unwrap_or("");
                if is_missing(cell) {
                    codes.push(None);
                } else {
                    let mut code = parse_code(cell, &labels, points, &stats[slot].name)?;
                    if reversed[slot] {
                        code = points + 1 - code;
                    }
                    codes.push(Some(code));
                }
            }
            respondents += 1;
            if missing_mode == "listwise" && codes.iter().any(|c| c.is_none()) {
                dropped_rows += 1;
                continue;
            }
            for (slot, code) in codes.iter().enumerate() {
                match code {
                    Some(c) => stats[slot].counts[*c - 1] += 1,
                    None => stats[slot].missing += 1,
                }
            }
            matrix.push(codes);
        }
    } else {
        // counts mode: item name + one count per scale point.
        let start = if rows[0]
            .iter()
            .skip(1)
            .all(|c| c.is_empty() || c.parse::<f64>().is_err())
            && rows.len() > 1
        {
            1 // first row is a header of category labels
        } else {
            0
        };
        if start >= rows.len() {
            return Err("counts input has a header row but no item rows".into());
        }
        stats = Vec::new();
        for row in &rows[start..] {
            let name = row.first().cloned().unwrap_or_default();
            let name = if name.is_empty() {
                format!("item {}", stats.len() + 1)
            } else {
                name
            };
            let cells: Vec<&String> = row.iter().skip(1).collect();
            if cells.len() != points {
                return Err(format!(
                    "item '{name}' has {} count column(s) but the scale has {points} points — give \
                     one count per category after the item name (or set points=...)",
                    cells.len()
                ));
            }
            let mut counts = Vec::with_capacity(points);
            for (i, c) in cells.iter().enumerate() {
                let v = if c.trim().is_empty() {
                    0.0
                } else {
                    c.parse::<f64>().map_err(|_| {
                        format!(
                            "count '{c}' for item '{name}', category {} is not a number",
                            i + 1
                        )
                    })?
                };
                if v < 0.0 || v.fract() != 0.0 {
                    return Err(format!(
                        "count '{c}' for item '{name}', category {} must be a whole number >= 0",
                        i + 1
                    ));
                }
                counts.push(v as u64);
            }
            stats.push(Item { name, counts, missing: 0 });
        }
        if stats.is_empty() {
            return Err("no item rows found in the counts input".into());
        }
        if !split_list(items).is_empty() {
            let names: Vec<String> = stats.iter().map(|s| s.name.clone()).collect();
            let mut keep = Vec::new();
            for name in split_list(items) {
                let idx = resolve_col(name, &names)?;
                if !keep.contains(&idx) {
                    keep.push(idx);
                }
            }
            stats = keep.into_iter().map(|i| Item {
                name: stats[i].name.clone(),
                counts: stats[i].counts.clone(),
                missing: 0,
            })
            .collect();
        }
        let names: Vec<String> = stats.iter().map(|s| s.name.clone()).collect();
        for name in split_list(reverse) {
            let idx = resolve_col(name, &names)?;
            stats[idx].counts.reverse();
        }
        respondents = stats.iter().map(|s| s.n() as usize).max().unwrap_or(0);
    }

    if stats.iter().all(|s| s.n() == 0) {
        return Err("no valid scale answers found — every cell was blank or missing".into());
    }

    // ---- ordering ---------------------------------------------------------
    let top_range = (points - box_size + 1)..=points;
    let mut order: Vec<usize> = (0..stats.len()).collect();
    match sort {
        "mean-desc" => order.sort_by(|a, b| {
            stats[*b].mean().partial_cmp(&stats[*a].mean()).unwrap().then(a.cmp(b))
        }),
        "mean-asc" => order.sort_by(|a, b| {
            stats[*a].mean().partial_cmp(&stats[*b].mean()).unwrap().then(a.cmp(b))
        }),
        "top-desc" => order.sort_by(|a, b| {
            stats[*b]
                .box_pct(top_range.clone())
                .partial_cmp(&stats[*a].box_pct(top_range.clone()))
                .unwrap()
                .then(a.cmp(b))
        }),
        _ => {}
    }

    // ---- header -----------------------------------------------------------
    let mut out = String::new();
    let total_valid: u64 = stats.iter().map(|s| s.n()).sum();
    let total_missing: u64 = stats.iter().map(|s| s.missing).sum();
    out.push_str(&format!(
        "Likert summary — {} item{}, {respondents} respondent{}\n",
        stats.len(),
        if stats.len() == 1 { "" } else { "s" },
        if respondents == 1 { "" } else { "s" }
    ));
    out.push_str(&format!(
        "Scale: {points}-point {scale} (1 = {} … {points} = {})\n",
        labels[0],
        labels[points - 1]
    ));
    out.push_str(&format!(
        "Box size: {box_size} categor{} each end. Missing: {missing_mode}.\n",
        if box_size == 1 { "y" } else { "ies" }
    ));
    if missing_mode == "listwise" && dropped_rows > 0 {
        out.push_str(&format!(
            "Listwise: dropped {dropped_rows} incomplete respondent row(s).\n"
        ));
    }
    out.push('\n');

    // ---- per-item table ---------------------------------------------------
    let name_w = stats
        .iter()
        .map(|s| truncate(&s.name, 30).chars().count())
        .max()
        .unwrap_or(4)
        .max(4);
    let bottom_h = format!("Bottom {box_size}");
    let top_h = format!("Top {box_size}");
    let odd = points % 2 == 1;
    let mut header_line = format!(
        "{}  {}  {}  {}  {}  {}  {}  {}",
        pad("Item", name_w),
        rpad_left("n", 5),
        rpad_left("miss", 5),
        rpad_left("mean", 7),
        rpad_left("sd", 7),
        rpad_left("median", 7),
        rpad_left("mode", 6),
        rpad_left(&bottom_h, 9),
    );
    if odd {
        header_line.push_str(&format!("  {}", rpad_left("Neutral", 9)));
    }
    header_line.push_str(&format!("  {}", rpad_left(&top_h, 9)));
    out.push_str(&header_line);
    out.push('\n');
    out.push_str(&"-".repeat(header_line.chars().count()));
    out.push('\n');

    let neutral_code = (points + 1) / 2;
    for i in &order {
        let it = &stats[*i];
        let modes = it
            .modes()
            .iter()
            .map(|m| m.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let mut line = format!(
            "{}  {}  {}  {}  {}  {}  {}  {}",
            pad(&truncate(&it.name, 30), name_w),
            rpad_left(&it.n().to_string(), 5),
            rpad_left(&it.missing.to_string(), 5),
            rpad_left(&round_to(it.mean(), decimals), 7),
            rpad_left(
                &it.sd().map(|v| round_to(v, decimals)).unwrap_or_else(|| "-".into()),
                7
            ),
            rpad_left(
                &it.median().map(|v| round_to(v, decimals)).unwrap_or_else(|| "-".into()),
                7
            ),
            rpad_left(if modes.is_empty() { "-" } else { &modes }, 6),
            rpad_left(&format!("{:.1}%", it.box_pct(1..=box_size)), 9),
        );
        if odd {
            line.push_str(&format!(
                "  {}",
                rpad_left(&format!("{:.1}%", it.pct(neutral_code)), 9)
            ));
        }
        line.push_str(&format!(
            "  {}",
            rpad_left(&format!("{:.1}%", it.box_pct(top_range.clone())), 9)
        ));
        out.push_str(&line);
        out.push('\n');
    }

    // ---- overall ----------------------------------------------------------
    let scored: Vec<&Item> = stats.iter().filter(|s| s.n() > 0).collect();
    let overall = scored.iter().map(|s| s.mean()).sum::<f64>() / scored.len() as f64;
    out.push('\n');
    out.push_str(&format!(
        "Overall mean of item means: {} ({total_valid} valid answer{}, {total_missing} missing)\n",
        round_to(overall, decimals),
        if total_valid == 1 { "" } else { "s" }
    ));

    // Floor / ceiling flags.
    let mut flags = Vec::new();
    for i in &order {
        let it = &stats[*i];
        if it.n() == 0 {
            continue;
        }
        if it.pct(1) >= FLOOR_CEILING_PCT {
            flags.push(format!(
                "  floor: {} — {:.1}% chose the lowest category ({})",
                it.name,
                it.pct(1),
                labels[0]
            ));
        }
        if it.pct(points) >= FLOOR_CEILING_PCT {
            flags.push(format!(
                "  ceiling: {} — {:.1}% chose the highest category ({})",
                it.name,
                it.pct(points),
                labels[points - 1]
            ));
        }
    }
    if !flags.is_empty() {
        out.push_str(&format!(
            "Floor/ceiling effects (>= {FLOOR_CEILING_PCT:.0}% at an end category):\n"
        ));
        for f in &flags {
            out.push_str(f);
            out.push('\n');
        }
    }

    if alpha {
        out.push_str(&cronbach_line(&matrix, stats.len(), input, decimals));
    }

    // ---- distribution -----------------------------------------------------
    out.push_str("\nDistribution (count and % of that item's valid answers)\n");
    let label_w = labels.iter().map(|l| l.chars().count()).max().unwrap_or(1);
    for i in &order {
        let it = &stats[*i];
        out.push_str(&format!("\n{} (n = {})\n", it.name, it.n()));
        for code in 1..=points {
            out.push_str(&format!(
                "  {code} {}  {}  {}\n",
                pad(&labels[code - 1], label_w),
                rpad_left(&it.counts[code - 1].to_string(), 5),
                rpad_left(&format!("{:.1}%", it.pct(code)), 7),
            ));
        }
    }

    // ---- stacked bars -----------------------------------------------------
    if chart {
        out.push('\n');
        if diverging {
            out.push_str(&format!(
                "Diverging stacked bars (centred between category {} and {}; {BAR_WIDTH} chars = \
                 100%)\n",
                points / 2,
                points / 2 + 1
            ));
        } else {
            out.push_str(&format!(
                "Stacked bars ({BAR_WIDTH} chars = 100% of that item's valid answers)\n"
            ));
        }
        let mut rendered: Vec<(String, String, String)> = Vec::new(); // name, left, right
        for i in &order {
            let it = &stats[*i];
            if it.n() == 0 {
                rendered.push((it.name.clone(), String::new(), String::new()));
                continue;
            }
            let pcts: Vec<f64> = (1..=points).map(|c| it.pct(c)).collect();
            let widths = allocate(&pcts, BAR_WIDTH);
            if diverging {
                let mut left = String::new();
                let mut right = String::new();
                for (idx, w) in widths.iter().enumerate() {
                    let code = idx + 1;
                    let seg: String = std::iter::repeat(bar_char(code)).take(*w).collect();
                    if odd && code == neutral_code {
                        let half = w / 2;
                        left.push_str(&seg[..half]);
                        right.push_str(&seg[half..]);
                    } else if code <= points / 2 {
                        left.push_str(&seg);
                    } else {
                        right.push_str(&seg);
                    }
                }
                rendered.push((it.name.clone(), left, right));
            } else {
                let bar: String = widths
                    .iter()
                    .enumerate()
                    .flat_map(|(idx, w)| std::iter::repeat(bar_char(idx + 1)).take(*w))
                    .collect();
                rendered.push((it.name.clone(), String::new(), bar));
            }
        }
        let left_w = rendered.iter().map(|r| r.1.chars().count()).max().unwrap_or(0);
        for (name, left, right) in &rendered {
            out.push_str(&format!(
                "  {}  {}{}{}\n",
                pad(&truncate(name, 30), name_w),
                rpad_left(left, left_w),
                if diverging { "|" } else { "" },
                right
            ));
        }
        out.push_str("  Key: ");
        out.push_str(
            &(1..=points)
                .map(|c| format!("{}={}", bar_char(c), labels[c - 1]))
                .collect::<Vec<_>>()
                .join("  "),
        );
        out.push('\n');
    }

    Ok(out)
}

/// Cronbach's alpha over the listwise-complete respondent rows.
fn cronbach_line(
    matrix: &[Vec<Option<usize>>],
    k: usize,
    input: &str,
    decimals: usize,
) -> String {
    if input != "responses" {
        return "Cronbach's alpha: not available from counts input (needs respondent-level rows)\n"
            .to_string();
    }
    if k < 2 {
        return "Cronbach's alpha: not computable (needs at least 2 items)\n".to_string();
    }
    let complete: Vec<Vec<f64>> = matrix
        .iter()
        .filter(|r| r.iter().all(|c| c.is_some()))
        .map(|r| r.iter().map(|c| c.unwrap() as f64).collect())
        .collect();
    let n = complete.len();
    if n < 2 {
        return "Cronbach's alpha: not computable (needs at least 2 complete respondent rows)\n"
            .to_string();
    }
    let var = |xs: &[f64]| -> f64 {
        let m = xs.iter().sum::<f64>() / xs.len() as f64;
        xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (xs.len() as f64 - 1.0)
    };
    let item_var_sum: f64 = (0..k)
        .map(|i| var(&complete.iter().map(|r| r[i]).collect::<Vec<_>>()))
        .sum();
    let totals: Vec<f64> = complete.iter().map(|r| r.iter().sum()).collect();
    let total_var = var(&totals);
    if total_var <= 0.0 {
        return "Cronbach's alpha: not computable (total scores have zero variance)\n".to_string();
    }
    let a = k as f64 / (k as f64 - 1.0) * (1.0 - item_var_sum / total_var);
    format!(
        "Cronbach's alpha: {} ({k} items, {n} complete respondent rows)\n",
        round_to(a, decimals)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = "Ease of use,Support,Value\n5,2,4\n4,3,4\n5,1,3\n4,2,5\n3,4,4\n";

    fn run(data: &str) -> String {
        summarize(
            data, "responses", "", 5, "agreement", "", "", 2, "exclude", "input", 2, false,
            false, false, ",",
        )
        .unwrap()
    }

    #[test]
    fn per_item_means_and_boxes() {
        let out = run(CSV);
        // Ease of use: 5,4,5,4,3 → mean 4.20, top-2 (4+5) = 80.0%
        assert!(out.contains("Ease of use"), "{out}");
        assert!(out.contains("4.20"), "{out}");
        assert!(out.contains("80.0%"), "{out}");
        // Support: 2,3,1,2,4 → mean 2.40
        assert!(out.contains("2.40"), "{out}");
        assert!(out.contains("Overall mean of item means: 3.53"), "{out}");
    }

    #[test]
    fn distribution_counts_are_reported() {
        let out = run(CSV);
        assert!(out.contains("Ease of use (n = 5)"), "{out}");
        assert!(out.contains("5 Strongly agree"), "{out}");
    }

    #[test]
    fn labels_are_accepted_as_answers() {
        let out = run("Q1\nStrongly agree\nAgree\nagree\n");
        // 5,4,4 → mean 4.33
        assert!(out.contains("4.33"), "{out}");
    }

    #[test]
    fn missing_values_are_excluded_and_counted() {
        let out = run("Q1,Q2\n5,\n4,NA\n3,2\n");
        assert!(out.contains("Q2"), "{out}");
        // Q2 has 1 valid answer and 2 missing.
        let line = out.lines().find(|l| l.starts_with("Q2")).unwrap();
        assert!(line.contains("    1      2"), "{line}");
    }

    #[test]
    fn listwise_drops_incomplete_rows() {
        let out = summarize(
            "Q1,Q2\n5,4\n4,\n3,2\n", "responses", "", 5, "agreement", "", "", 2, "listwise",
            "input", 2, false, false, false, ",",
        )
        .unwrap();
        assert!(out.contains("dropped 1 incomplete respondent row"), "{out}");
        // Q1 keeps 2 answers (5 and 3) → mean 4.00
        let line = out.lines().find(|l| l.starts_with("Q1")).unwrap();
        assert!(line.contains("4.00"), "{line}");
    }

    #[test]
    fn reverse_scoring_flips_an_item() {
        let out = summarize(
            "Q1\n1\n2\n", "responses", "", 5, "agreement", "", "Q1", 2, "exclude", "input", 2,
            false, false, false, ",",
        )
        .unwrap();
        // 1,2 reversed on a 5-point scale → 5,4 → mean 4.50
        assert!(out.contains("4.50"), "{out}");
    }

    #[test]
    fn counts_input_mode() {
        let out = summarize(
            "Item,SD,D,N,A,SA\nOnboarding,1,1,2,4,2\n", "counts", "", 5, "agreement", "", "", 2,
            "exclude", "input", 2, false, false, false, ",",
        )
        .unwrap();
        // (1*1 + 2*1 + 3*2 + 4*4 + 5*2)/10 = 3.50, top-2 = 60.0%
        assert!(out.contains("3.50"), "{out}");
        assert!(out.contains("60.0%"), "{out}");
    }

    #[test]
    fn sorting_by_mean_reorders_items() {
        let out = summarize(
            CSV, "responses", "", 5, "agreement", "", "", 2, "exclude", "mean-desc", 2, false,
            false, false, ",",
        )
        .unwrap();
        let body: Vec<&str> = out
            .lines()
            .skip_while(|l| !l.starts_with("---"))
            .skip(1)
            .take(3)
            .collect();
        assert!(body[0].starts_with("Ease of use"), "{body:?}");
        assert!(body[2].starts_with("Support"), "{body:?}");
    }

    #[test]
    fn stacked_bar_is_exactly_full_width() {
        let out = summarize(
            "Q1\n5\n5\n5\n5\n", "responses", "", 5, "agreement", "", "", 2, "exclude", "input",
            2, true, false, false, ",",
        )
        .unwrap();
        assert!(out.contains(&"5".repeat(BAR_WIDTH)), "{out}");
        assert!(out.contains("Key: 1=Strongly disagree"), "{out}");
    }

    #[test]
    fn diverging_bar_has_a_centre_marker() {
        let out = summarize(
            CSV, "responses", "", 5, "agreement", "", "", 2, "exclude", "input", 2, true, true,
            false, ",",
        )
        .unwrap();
        assert!(out.contains("Diverging stacked bars"), "{out}");
        assert!(out.lines().any(|l| l.contains('|')), "{out}");
    }

    #[test]
    fn cronbach_alpha_is_reported() {
        let out = summarize(
            CSV, "responses", "", 5, "agreement", "", "", 2, "exclude", "input", 2, false, false,
            true, ",",
        )
        .unwrap();
        assert!(out.contains("Cronbach's alpha:"), "{out}");
        assert!(out.contains("3 items, 5 complete respondent rows"), "{out}");
    }

    #[test]
    fn seven_point_scale_and_custom_labels() {
        let out = summarize(
            "Q1\n7\n6\n", "responses", "", 7, "agreement", "", "", 3, "exclude", "input", 1,
            false, false, false, ",",
        )
        .unwrap();
        assert!(out.contains("7-point agreement"), "{out}");
        let out2 = summarize(
            "Q1\nLow\nHigh\n", "responses", "", 3, "custom", "Low,Mid,High", "", 1, "exclude",
            "input", 2, false, false, false, ",",
        )
        .unwrap();
        assert!(out2.contains("1 = Low … 3 = High"), "{out2}");
    }

    #[test]
    fn items_selects_columns() {
        let out = summarize(
            "id,Q1\nR1,5\nR2,4\n", "responses", "Q1", 5, "agreement", "", "", 2, "exclude",
            "input", 2, false, false, false, ",",
        )
        .unwrap();
        assert!(out.contains("1 item, 2 respondents"), "{out}");
        assert!(out.contains("4.50"), "{out}");
    }

    #[test]
    fn floor_and_ceiling_are_flagged() {
        let out = run("Q1\n1\n1\n5\n5\n");
        assert!(out.contains("floor: Q1"), "{out}");
        assert!(out.contains("ceiling: Q1"), "{out}");
    }

    // ---- error paths ------------------------------------------------------

    #[test]
    fn out_of_range_answer_errors() {
        let err = summarize(
            "Q1\n9\n", "responses", "", 5, "agreement", "", "", 2, "exclude", "input", 2, false,
            false, false, ",",
        )
        .unwrap_err();
        assert!(err.contains("outside the 1-5 scale"), "{err}");
    }

    #[test]
    fn unreadable_answer_mentions_items_option() {
        let err = summarize(
            "id,Q1\nR1,5\n", "responses", "", 5, "agreement", "", "", 2, "exclude", "input", 2,
            false, false, false, ",",
        )
        .unwrap_err();
        assert!(err.contains("items="), "{err}");
    }

    #[test]
    fn bad_box_size_errors() {
        let err = summarize(
            CSV, "responses", "", 5, "agreement", "", "", 3, "exclude", "input", 2, false, false,
            false, ",",
        )
        .unwrap_err();
        assert!(err.contains("box_size must be between 1 and 2"), "{err}");
    }

    #[test]
    fn custom_scale_without_labels_errors() {
        let err = summarize(
            CSV, "responses", "", 5, "custom", "", "", 2, "exclude", "input", 2, false, false,
            false, ",",
        )
        .unwrap_err();
        assert!(err.contains("scale=custom needs labels"), "{err}");
    }

    #[test]
    fn wrong_label_count_errors() {
        let err = summarize(
            CSV, "responses", "", 5, "custom", "Low,High", "", 2, "exclude", "input", 2, false,
            false, false, ",",
        )
        .unwrap_err();
        assert!(err.contains("but the scale has 5 points"), "{err}");
    }

    #[test]
    fn counts_row_width_mismatch_errors() {
        let err = summarize(
            "Item,A,B,C\nOnboarding,1,2,3\n", "counts", "", 5, "agreement", "", "", 2, "exclude",
            "input", 2, false, false, false, ",",
        )
        .unwrap_err();
        assert!(err.contains("count column(s)"), "{err}");
    }

    #[test]
    fn empty_input_errors() {
        let err = summarize(
            "   ", "responses", "", 5, "agreement", "", "", 2, "exclude", "input", 2, false,
            false, false, ",",
        )
        .unwrap_err();
        assert!(err.contains("no data rows"), "{err}");
    }
}
