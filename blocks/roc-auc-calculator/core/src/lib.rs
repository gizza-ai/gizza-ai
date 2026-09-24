//! roc-auc-calculator core — pure compute, shared by the chat skill block and the web page.
//!
//! Binary ROC / AUC analysis from continuous scores plus class labels:
//! Mann-Whitney AUC with tie midranks, the fast-DeLong standard error, a confidence
//! interval and a z-test against AUC = 0.5, a full threshold sweep with the usual
//! operating-point metrics, an optimal cutoff under several criteria, an optional
//! ASCII ROC plot, and markdown / text / CSV / JSON output.

use std::fmt::Write as _;

/// Hard cap on parsed observations (stated on the page).
pub const MAX_OBSERVATIONS: usize = 20_000;

/// Label tokens treated as the positive class when no `positive_label` is given.
const POSITIVE_TOKENS: &[&str] = &[
    "1",
    "true",
    "t",
    "yes",
    "y",
    "pos",
    "positive",
    "case",
    "cases",
    "event",
    "disease",
    "diseased",
    "sick",
    "abnormal",
    "malignant",
    "fraud",
    "fraudulent",
    "default",
    "churn",
    "churned",
    "spam",
    "click",
    "clicked",
    "failure",
    "failed",
    "anomaly",
    "attack",
];

/// Label tokens treated as the negative class when no `positive_label` is given.
const NEGATIVE_TOKENS: &[&str] = &[
    "0",
    "false",
    "f",
    "no",
    "n",
    "neg",
    "negative",
    "control",
    "controls",
    "non-event",
    "nonevent",
    "healthy",
    "well",
    "normal",
    "benign",
    "legit",
    "legitimate",
    "ham",
    "stay",
    "retained",
    "success",
    "ok",
    "clean",
];

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the analysis. Every argument maps 1:1 to a descriptor param.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    labels: &str,
    input_format: &str,
    column_order: &str,
    separator: &str,
    header: &str,
    positive_label: &str,
    optimize: &str,
    cost_ratio: f64,
    threshold: &str,
    confidence_level: &str,
    table_rows: f64,
    plot: bool,
    decimals: f64,
    percent: bool,
    format: &str,
) -> Result<String, String> {
    let input_format = pick(
        input_format,
        "auto",
        &["auto", "pairs", "columns"],
        "input_format",
    )?;
    let column_order = pick(
        column_order,
        "auto",
        &["auto", "score_label", "label_score"],
        "column_order",
    )?;
    let separator = pick(
        separator,
        "auto",
        &[
            "auto",
            "comma",
            "tab",
            "semicolon",
            "pipe",
            "space",
            "newline",
        ],
        "separator",
    )?;
    let header = pick(header, "auto", &["auto", "yes", "no"], "header")?;
    let optimize = pick(
        optimize,
        "youden",
        &["youden", "f1", "closest", "accuracy", "cost"],
        "optimize",
    )?;
    let confidence_level = pick(
        confidence_level,
        "95",
        &["90", "95", "99"],
        "confidence_level",
    )?;
    let format = pick(
        format,
        "markdown",
        &["markdown", "text", "csv", "json"],
        "format",
    )?;

    if !cost_ratio.is_finite() || cost_ratio <= 0.0 {
        return Err(format!(
            "expected cost_ratio to be a positive number (cost of a false negative relative to a false positive), got {cost_ratio}"
        ));
    }
    if !decimals.is_finite() || !(0.0..=10.0).contains(&decimals) {
        return Err(format!(
            "expected decimals between 0 and 10, got {decimals}"
        ));
    }
    if !table_rows.is_finite() || !(0.0..=200.0).contains(&table_rows) {
        return Err(format!(
            "expected table_rows between 0 and 200, got {table_rows}"
        ));
    }
    let cfg = Cfg {
        decimals: decimals as usize,
        percent,
    };

    let parsed = parse_input(
        data,
        labels,
        input_format,
        column_order,
        separator,
        header,
        positive_label,
    )?;

    let user_threshold = if threshold.trim().is_empty() {
        None
    } else {
        Some(threshold.trim().parse::<f64>().map_err(|_| {
            format!(
                "expected threshold to be a number (a cutoff on the same scale as the scores), got {:?}",
                threshold.trim()
            )
        })?)
    };
    if let Some(t) = user_threshold {
        if !t.is_finite() {
            return Err("expected threshold to be a finite number".to_string());
        }
    }

    let report = analyze(
        parsed,
        optimize,
        cost_ratio,
        user_threshold,
        confidence_level.parse::<u32>().unwrap_or(95),
        table_rows as usize,
        plot,
    )?;

    Ok(match format {
        "text" => render_text(&report, &cfg),
        "csv" => render_csv(&report, &cfg),
        "json" => render_json(&report, &cfg),
        _ => render_markdown(&report, &cfg),
    })
}

fn pick<'a>(
    value: &'a str,
    default: &'a str,
    allowed: &[&'a str],
    name: &str,
) -> Result<&'a str, String> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(default);
    }
    let lower = v.to_ascii_lowercase();
    allowed
        .iter()
        .find(|a| **a == lower)
        .copied()
        .ok_or_else(|| {
            format!(
                "expected {name} to be one of {}, got {:?}",
                allowed.join(", "),
                v
            )
        })
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
struct Obs {
    score: f64,
    positive: bool,
}

struct Parsed {
    obs: Vec<Obs>,
    pos_label: String,
    neg_label: String,
    label_source: &'static str,
}

#[derive(Clone, Copy, PartialEq)]
enum Sep {
    Char(char),
    Whitespace,
    Newline,
}

fn sep_from_name(name: &str) -> Option<Sep> {
    Some(match name {
        "comma" => Sep::Char(','),
        "tab" => Sep::Char('\t'),
        "semicolon" => Sep::Char(';'),
        "pipe" => Sep::Char('|'),
        "space" => Sep::Whitespace,
        "newline" => Sep::Newline,
        _ => return None,
    })
}

fn rows(text: &str) -> Vec<&str> {
    text.lines()
        .map(|l| l.trim_matches('\r').trim())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Pick the delimiter that splits every row into the same number of >1 fields.
fn detect_separator(lines: &[&str]) -> Sep {
    for c in [',', '\t', ';', '|'] {
        let counts: Vec<usize> = lines.iter().map(|l| l.matches(c).count()).collect();
        if counts.iter().all(|n| *n >= 1) && counts.windows(2).all(|w| w[0] == w[1]) {
            return Sep::Char(c);
        }
    }
    Sep::Whitespace
}

fn split_row(row: &str, sep: Sep) -> Vec<String> {
    match sep {
        Sep::Char(c) => row.split(c).map(|s| s.trim().to_string()).collect(),
        Sep::Whitespace | Sep::Newline => {
            row.split_whitespace().map(|s| s.to_string()).collect()
        }
    }
}

/// Tokenize a whole column field: newlines always split, plus the chosen separator.
fn tokenize_column(text: &str, sep: Sep) -> Vec<String> {
    let mut out = Vec::new();
    for line in rows(text) {
        match sep {
            Sep::Newline => out.push(line.to_string()),
            _ => out.extend(split_row(line, sep).into_iter().filter(|t| !t.is_empty())),
        }
    }
    out.retain(|t| !t.is_empty());
    out
}

/// Does this token read as a score? Accepts a trailing `%` (normalized later).
fn is_number(token: &str) -> bool {
    token
        .trim()
        .trim_end_matches('%')
        .parse::<f64>()
        .map(|v| v.is_finite())
        .unwrap_or(false)
}

fn parse_input(
    data: &str,
    labels: &str,
    input_format: &str,
    column_order: &str,
    separator: &str,
    header: &str,
    positive_label: &str,
) -> Result<Parsed, String> {
    if data.trim().is_empty() {
        return Err("expected scored observations, got an empty data field. Paste one `score,label` pair per line, for example `0.91,1`.".to_string());
    }
    let shape = match input_format {
        "pairs" => "pairs",
        "columns" => "columns",
        _ => {
            if labels.trim().is_empty() {
                "pairs"
            } else {
                "columns"
            }
        }
    };
    if shape == "columns" && labels.trim().is_empty() {
        return Err("input_format = columns needs the labels field filled with one class label per score. Leave it empty to paste `score,label` pairs instead.".to_string());
    }

    let (raw_scores, raw_labels) = if shape == "columns" {
        parse_columns(data, labels, separator, header)?
    } else {
        parse_pairs(data, separator, header, column_order)?
    };

    if raw_scores.len() < 2 {
        return Err(format!(
            "expected at least 2 scored observations, got {}",
            raw_scores.len()
        ));
    }
    if raw_scores.len() > MAX_OBSERVATIONS {
        return Err(format!(
            "expected at most {MAX_OBSERVATIONS} observations, got {}",
            raw_scores.len()
        ));
    }

    let (pos_label, neg_label, label_source) = resolve_classes(&raw_labels, positive_label)?;
    let pos_key = pos_label.to_ascii_lowercase();

    let mut obs = Vec::with_capacity(raw_scores.len());
    for (score, label) in raw_scores.iter().zip(raw_labels.iter()) {
        obs.push(Obs {
            score: *score,
            positive: label.to_ascii_lowercase() == pos_key,
        });
    }

    let n_pos = obs.iter().filter(|o| o.positive).count();
    let n_neg = obs.len() - n_pos;
    if n_pos == 0 {
        return Err(format!(
            "no observation carries the positive class {pos_label:?}; AUC needs both classes present"
        ));
    }
    if n_neg == 0 {
        return Err(format!(
            "every observation is in the positive class {pos_label:?}; AUC needs at least one negative observation too"
        ));
    }

    Ok(Parsed {
        obs,
        pos_label,
        neg_label,
        label_source,
    })
}

fn parse_columns(
    data: &str,
    labels: &str,
    separator: &str,
    header: &str,
) -> Result<(Vec<f64>, Vec<String>), String> {
    let data_lines = rows(data);
    let sep = match sep_from_name(separator) {
        Some(s) => s,
        None => {
            let d = detect_separator(&data_lines);
            if data_lines.len() > 1 && matches!(d, Sep::Whitespace) {
                Sep::Newline
            } else {
                d
            }
        }
    };
    let mut scores = tokenize_column(data, sep);
    let mut labs = tokenize_column(labels, sep);

    let drop_header = match header {
        "yes" => true,
        "no" => false,
        _ => !scores.is_empty() && !is_number(&scores[0]),
    };
    if drop_header {
        if scores.is_empty() || labs.is_empty() {
            return Err("expected a header row followed by data, got an empty column".to_string());
        }
        scores.remove(0);
        labs.remove(0);
    }

    if scores.len() != labs.len() {
        return Err(format!(
            "expected the scores and labels columns to have the same length, got {} scores and {} labels",
            scores.len(),
            labs.len()
        ));
    }

    let mut out = Vec::with_capacity(scores.len());
    for (i, token) in scores.iter().enumerate() {
        out.push(parse_score(token, i + 1 + usize::from(drop_header))?);
    }
    for (i, l) in labs.iter().enumerate() {
        if l.is_empty() {
            return Err(format!(
                "expected a class label on row {}, got an empty value",
                i + 1 + usize::from(drop_header)
            ));
        }
    }
    Ok((out, labs))
}

fn parse_pairs(
    data: &str,
    separator: &str,
    header: &str,
    column_order: &str,
) -> Result<(Vec<f64>, Vec<String>), String> {
    let lines = rows(data);
    if lines.is_empty() {
        return Err("expected scored observations, got an empty data field".to_string());
    }
    let sep = match sep_from_name(separator) {
        Some(Sep::Newline) => {
            return Err("separator = newline cannot split the score from the label on the same row. Choose comma, tab, semicolon, pipe, spaces, or auto — or paste the labels in the separate labels field.".to_string());
        }
        Some(s) => s,
        None => detect_separator(&lines),
    };

    let mut cells: Vec<Vec<String>> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let parts: Vec<String> = split_row(line, sep)
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() < 2 {
            return Err(format!(
                "expected 2 columns (a score and a class label) on row {}, got {} in {:?}. Check the separator setting.",
                i + 1,
                parts.len(),
                line
            ));
        }
        cells.push(parts);
    }

    let drop_header = match header {
        "yes" => true,
        "no" => false,
        _ => !cells[0].iter().any(|c| is_number(c)),
    };
    if drop_header {
        cells.remove(0);
        if cells.is_empty() {
            return Err("expected data rows after the header row, got none".to_string());
        }
    }
    let row_offset = 1 + usize::from(drop_header);

    let numeric_first = cells.iter().all(|r| is_number(&r[0]));
    let numeric_second = cells.iter().all(|r| is_number(&r[1]));
    let order = match column_order {
        "score_label" => "score_label",
        "label_score" => "label_score",
        _ => {
            // A 0/1 column is the class column; the other one carries the scores.
            // This is checked first so a single unparseable score still reports
            // its own row rather than flipping the whole table around.
            let binary_first = looks_binary(cells.iter().map(|r| r[0].as_str()));
            let binary_second = looks_binary(cells.iter().map(|r| r[1].as_str()));
            if binary_second && !binary_first {
                "score_label"
            } else if binary_first && !binary_second {
                "label_score"
            } else if numeric_first && !numeric_second {
                "score_label"
            } else if numeric_second && !numeric_first {
                "label_score"
            } else if numeric_first || numeric_second {
                "score_label"
            } else {
                return Err(format!(
                    "expected a numeric score column, got non-numeric values in both columns (first row: {:?}). Set column_order, or check the separator and header settings.",
                    cells[0]
                ));
            }
        }
    };

    let (si, li) = if order == "score_label" { (0, 1) } else { (1, 0) };
    let mut scores = Vec::with_capacity(cells.len());
    let mut labs = Vec::with_capacity(cells.len());
    for (i, row) in cells.iter().enumerate() {
        scores.push(parse_score(&row[si], i + row_offset)?);
        if row[li].is_empty() {
            return Err(format!(
                "expected a class label on row {}, got an empty value",
                i + row_offset
            ));
        }
        labs.push(row[li].clone());
    }
    Ok((scores, labs))
}

fn looks_binary<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let mut seen: Vec<String> = Vec::new();
    for v in values {
        let key = v.trim().to_ascii_lowercase();
        if !matches!(key.as_str(), "0" | "1" | "0.0" | "1.0") {
            return false;
        }
        if !seen.contains(&key) {
            seen.push(key);
        }
    }
    !seen.is_empty()
}

fn parse_score(token: &str, row: usize) -> Result<f64, String> {
    let cleaned = token.trim().trim_end_matches('%');
    let v: f64 = cleaned.parse().map_err(|_| {
        format!("expected a numeric score on row {row}, got {token:?}")
    })?;
    if !v.is_finite() {
        return Err(format!(
            "expected a finite numeric score on row {row}, got {token:?}"
        ));
    }
    Ok(if token.trim().ends_with('%') {
        v / 100.0
    } else {
        v
    })
}

/// Decide which label text is the positive class.
fn resolve_classes(
    labels: &[String],
    positive_label: &str,
) -> Result<(String, String, &'static str), String> {
    let mut distinct: Vec<String> = Vec::new();
    for l in labels {
        let key = l.to_ascii_lowercase();
        if !distinct.iter().any(|d| d.to_ascii_lowercase() == key) {
            distinct.push(l.clone());
        }
        if distinct.len() > 64 {
            break;
        }
    }

    let requested = positive_label.trim();
    if !requested.is_empty() {
        let key = requested.to_ascii_lowercase();
        let matched = distinct
            .iter()
            .find(|d| d.to_ascii_lowercase() == key)
            .cloned();
        let pos = match matched {
            Some(p) => p,
            None => {
                return Err(format!(
                    "positive_label {requested:?} does not appear in the labels; found {}",
                    preview(&distinct)
                ))
            }
        };
        let neg = if distinct.len() == 2 {
            distinct
                .iter()
                .find(|d| d.to_ascii_lowercase() != key)
                .cloned()
                .unwrap_or_else(|| "other".to_string())
        } else {
            format!("not {pos}")
        };
        return Ok((pos, neg, "set by positive_label"));
    }

    if distinct.len() == 1 {
        return Err(format!(
            "only one class is present ({:?}); AUC needs both a positive and a negative class",
            distinct[0]
        ));
    }
    if distinct.len() > 2 {
        return Err(format!(
            "expected 2 classes, found {} ({}). Set positive_label to score one class against the rest.",
            distinct.len(),
            preview(&distinct)
        ));
    }

    let a = distinct[0].clone();
    let b = distinct[1].clone();
    let ka = a.to_ascii_lowercase();
    let kb = b.to_ascii_lowercase();
    let a_pos = POSITIVE_TOKENS.contains(&ka.as_str());
    let b_pos = POSITIVE_TOKENS.contains(&kb.as_str());
    let a_neg = NEGATIVE_TOKENS.contains(&ka.as_str());
    let b_neg = NEGATIVE_TOKENS.contains(&kb.as_str());

    if a_pos && !b_pos {
        return Ok((a, b, "auto-detected from a common positive label"));
    }
    if b_pos && !a_pos {
        return Ok((b, a, "auto-detected from a common positive label"));
    }
    if a_neg && !b_neg {
        return Ok((b, a, "auto-detected from a common negative label"));
    }
    if b_neg && !a_neg {
        return Ok((a, b, "auto-detected from a common negative label"));
    }
    if let (Ok(na), Ok(nb)) = (ka.parse::<f64>(), kb.parse::<f64>()) {
        return Ok(if na > nb {
            (a, b, "auto-detected: the larger numeric label is positive")
        } else {
            (b, a, "auto-detected: the larger numeric label is positive")
        });
    }
    Ok(if ka > kb {
        (a, b, "auto-detected: the later label alphabetically is positive")
    } else {
        (b, a, "auto-detected: the later label alphabetically is positive")
    })
}

fn preview(distinct: &[String]) -> String {
    let shown: Vec<String> = distinct.iter().take(6).map(|d| format!("{d:?}")).collect();
    if distinct.len() > 6 {
        format!("{}, …", shown.join(", "))
    } else {
        shown.join(", ")
    }
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Op {
    threshold: f64,
    tp: usize,
    fp: usize,
    fn_: usize,
    tn: usize,
    sens: f64,
    spec: f64,
    fpr: f64,
    ppv: Option<f64>,
    npv: Option<f64>,
    acc: f64,
    f1: Option<f64>,
    youden: f64,
    dist: f64,
    cost: f64,
    mcc: Option<f64>,
}

fn operating_point(threshold: f64, tp: usize, fp: usize, n_pos: usize, n_neg: usize, cost_ratio: f64) -> Op {
    let fn_ = n_pos - tp;
    let tn = n_neg - fp;
    let sens = tp as f64 / n_pos as f64;
    let spec = tn as f64 / n_neg as f64;
    let fpr = 1.0 - spec;
    let ppv = if tp + fp > 0 {
        Some(tp as f64 / (tp + fp) as f64)
    } else {
        None
    };
    let npv = if tn + fn_ > 0 {
        Some(tn as f64 / (tn + fn_) as f64)
    } else {
        None
    };
    let acc = (tp + tn) as f64 / (n_pos + n_neg) as f64;
    let f1_den = (2 * tp + fp + fn_) as f64;
    let f1 = if f1_den > 0.0 {
        Some(2.0 * tp as f64 / f1_den)
    } else {
        None
    };
    let mcc_den = ((tp + fp) as f64) * ((tp + fn_) as f64) * ((tn + fp) as f64) * ((tn + fn_) as f64);
    let mcc = if mcc_den > 0.0 {
        Some(((tp as f64) * (tn as f64) - (fp as f64) * (fn_ as f64)) / mcc_den.sqrt())
    } else {
        None
    };
    Op {
        threshold,
        tp,
        fp,
        fn_,
        tn,
        sens,
        spec,
        fpr,
        ppv,
        npv,
        acc,
        f1,
        youden: sens + spec - 1.0,
        dist: (fpr * fpr + (1.0 - sens) * (1.0 - sens)).sqrt(),
        cost: cost_ratio * fn_ as f64 + fp as f64,
        mcc,
    }
}

struct Report {
    n: usize,
    n_pos: usize,
    n_neg: usize,
    pos_label: String,
    neg_label: String,
    label_source: &'static str,
    auc: f64,
    auc_trapezoid: f64,
    gini: f64,
    se: Option<f64>,
    ci: Option<(f64, f64)>,
    z: Option<f64>,
    p: Option<f64>,
    conf: u32,
    brier: Option<f64>,
    band: &'static str,
    criterion: &'static str,
    cost_ratio: f64,
    optimal: Op,
    user: Option<Op>,
    table: Vec<(Op, bool)>,
    plot: Option<String>,
    curve_points: usize,
    distinct_scores: usize,
}

fn analyze(
    parsed: Parsed,
    optimize: &str,
    cost_ratio: f64,
    user_threshold: Option<f64>,
    conf: u32,
    table_rows: usize,
    want_plot: bool,
) -> Result<Report, String> {
    let Parsed {
        mut obs,
        pos_label,
        neg_label,
        label_source,
    } = parsed;
    let n = obs.len();
    let n_pos = obs.iter().filter(|o| o.positive).count();
    let n_neg = n - n_pos;

    // Descending by score; positives first inside a tie group only affects
    // nothing (tie groups are consumed whole).
    obs.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // Curve points: (threshold = +inf, 0, 0) then one point per distinct score.
    let mut cum: Vec<(f64, usize, usize)> = vec![(f64::INFINITY, 0, 0)];
    let mut tp = 0usize;
    let mut fp = 0usize;
    let mut i = 0usize;
    while i < n {
        let s = obs[i].score;
        while i < n && obs[i].score == s {
            if obs[i].positive {
                tp += 1;
            } else {
                fp += 1;
            }
            i += 1;
        }
        cum.push((s, tp, fp));
    }
    let distinct_scores = cum.len() - 1;

    // Trapezoidal area over the ROC polyline (cross-check on the rank AUC).
    let mut auc_trapezoid = 0.0;
    for w in cum.windows(2) {
        let (_, tp0, fp0) = w[0];
        let (_, tp1, fp1) = w[1];
        let x0 = fp0 as f64 / n_neg as f64;
        let x1 = fp1 as f64 / n_neg as f64;
        let y0 = tp0 as f64 / n_pos as f64;
        let y1 = tp1 as f64 / n_pos as f64;
        auc_trapezoid += (x1 - x0) * (y0 + y1) / 2.0;
    }

    let (auc, se) = delong(&obs, n_pos, n_neg);
    let z_crit = match conf {
        90 => 1.644_853_626_951_472_2,
        99 => 2.575_829_303_548_900_4,
        _ => 1.959_963_984_540_054,
    };
    let ci = se.map(|s| {
        (
            (auc - z_crit * s).clamp(0.0, 1.0),
            (auc + z_crit * s).clamp(0.0, 1.0),
        )
    });
    let z = se.and_then(|s| if s > 0.0 { Some((auc - 0.5) / s) } else { None });
    let p = z.map(|zz| (2.0 * norm_sf(zz.abs())).clamp(0.0, 1.0));

    let brier = if obs.iter().all(|o| (0.0..=1.0).contains(&o.score)) {
        Some(
            obs.iter()
                .map(|o| {
                    let y = if o.positive { 1.0 } else { 0.0 };
                    (o.score - y) * (o.score - y)
                })
                .sum::<f64>()
                / n as f64,
        )
    } else {
        None
    };

    // Candidate cutoffs = the observed scores (skip the degenerate +inf point).
    let candidates: Vec<Op> = cum[1..]
        .iter()
        .map(|(t, tp, fp)| operating_point(*t, *tp, *fp, n_pos, n_neg, cost_ratio))
        .collect();

    let criterion: &'static str = match optimize {
        "f1" => "f1",
        "closest" => "closest",
        "accuracy" => "accuracy",
        "cost" => "cost",
        _ => "youden",
    };
    let mut best_idx = 0usize;
    for (idx, op) in candidates.iter().enumerate() {
        if idx == 0 {
            continue;
        }
        if better(op, &candidates[best_idx], criterion) {
            best_idx = idx;
        }
    }
    let optimal = candidates[best_idx];

    let user = user_threshold.map(|t| {
        let mut tp = 0usize;
        let mut fp = 0usize;
        for o in &obs {
            if o.score >= t {
                if o.positive {
                    tp += 1;
                } else {
                    fp += 1;
                }
            }
        }
        operating_point(t, tp, fp, n_pos, n_neg, cost_ratio)
    });

    let table = sample_table(&candidates, best_idx, table_rows);

    let plot = if want_plot {
        let pts: Vec<(f64, f64)> = cum
            .iter()
            .map(|(_, tp, fp)| (*fp as f64 / n_neg as f64, *tp as f64 / n_pos as f64))
            .collect();
        Some(ascii_plot(&pts, (optimal.fpr, optimal.sens)))
    } else {
        None
    };

    Ok(Report {
        n,
        n_pos,
        n_neg,
        pos_label,
        neg_label,
        label_source,
        auc,
        auc_trapezoid,
        gini: 2.0 * auc - 1.0,
        se,
        ci,
        z,
        p,
        conf,
        brier,
        band: band(auc),
        criterion,
        cost_ratio,
        optimal,
        user,
        table,
        plot,
        curve_points: cum.len(),
        distinct_scores,
    })
}

fn better(a: &Op, b: &Op, criterion: &str) -> bool {
    match criterion {
        "f1" => a.f1.unwrap_or(-1.0) > b.f1.unwrap_or(-1.0),
        "closest" => a.dist < b.dist,
        "accuracy" => a.acc > b.acc,
        "cost" => a.cost < b.cost,
        _ => a.youden > b.youden,
    }
}

fn band(auc: f64) -> &'static str {
    if auc < 0.5 {
        "below chance — the ranking is inverted, so check which class is the positive one"
    } else if auc < 0.6 {
        "close to chance — the scores barely separate the two classes"
    } else if auc < 0.7 {
        "weak separation"
    } else if auc < 0.8 {
        "fair separation"
    } else if auc < 0.9 {
        "good separation"
    } else if auc < 1.0 {
        "strong separation"
    } else {
        "perfect separation — every positive outranks every negative"
    }
}

/// Midranks (1-based, ties averaged) aligned to the input order.
fn midranks(values: &[f64]) -> Vec<f64> {
    let n = values.len();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| {
        values[a]
            .partial_cmp(&values[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut out = vec![0.0; n];
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j + 1 < n && values[idx[j + 1]] == values[idx[i]] {
            j += 1;
        }
        let mid = ((i + 1) + (j + 1)) as f64 / 2.0;
        for k in i..=j {
            out[idx[k]] = mid;
        }
        i = j + 1;
    }
    out
}

/// Fast DeLong: returns (AUC, standard error). SE is None when either class has
/// fewer than 2 observations (the variance components are then undefined).
fn delong(obs: &[Obs], n_pos: usize, n_neg: usize) -> (f64, Option<f64>) {
    let pos: Vec<f64> = obs.iter().filter(|o| o.positive).map(|o| o.score).collect();
    let neg: Vec<f64> = obs.iter().filter(|o| !o.positive).map(|o| o.score).collect();
    let m = pos.len();
    let n = neg.len();
    debug_assert_eq!((m, n), (n_pos, n_neg));

    let tx = midranks(&pos);
    let ty = midranks(&neg);
    let mut all = Vec::with_capacity(m + n);
    all.extend_from_slice(&pos);
    all.extend_from_slice(&neg);
    let tz = midranks(&all);

    let v10: Vec<f64> = (0..m).map(|i| (tz[i] - tx[i]) / n as f64).collect();
    let v01: Vec<f64> = (0..n).map(|j| 1.0 - (tz[m + j] - ty[j]) / m as f64).collect();
    let auc = v10.iter().sum::<f64>() / m as f64;

    if m < 2 || n < 2 {
        return (auc, None);
    }
    let s10 = v10.iter().map(|v| (v - auc) * (v - auc)).sum::<f64>() / (m - 1) as f64;
    let s01 = v01.iter().map(|v| (v - auc) * (v - auc)).sum::<f64>() / (n - 1) as f64;
    let var = s10 / m as f64 + s01 / n as f64;
    (auc, Some(var.max(0.0).sqrt()))
}

/// Complementary error function (Numerical Recipes rational fit, |error| < 1.2e-7).
fn erfc(x: f64) -> f64 {
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let ans = t * (-z * z - 1.265_512_23
        + t * (1.000_023_68
            + t * (0.374_091_96
                + t * (0.096_784_18
                    + t * (-0.186_288_06
                        + t * (0.278_868_07
                            + t * (-1.135_203_98
                                + t * (1.488_515_87 + t * (-0.822_152_23 + t * 0.170_872_77)))))))))
        .exp();
    if x >= 0.0 {
        ans
    } else {
        2.0 - ans
    }
}

/// Upper tail of the standard normal distribution, P(Z > z).
fn norm_sf(z: f64) -> f64 {
    0.5 * erfc(z / std::f64::consts::SQRT_2)
}

fn sample_table(candidates: &[Op], best_idx: usize, rows: usize) -> Vec<(Op, bool)> {
    if rows == 0 || candidates.is_empty() {
        return Vec::new();
    }
    let mut idxs: Vec<usize> = if candidates.len() <= rows {
        (0..candidates.len()).collect()
    } else if rows == 1 {
        vec![best_idx]
    } else {
        let last = candidates.len() - 1;
        let mut v: Vec<usize> = (0..rows)
            .map(|i| i * last / (rows - 1))
            .collect();
        v.dedup();
        if !v.contains(&best_idx) {
            // Replace the sampled row nearest the optimum so the cap is respected.
            let nearest = v
                .iter()
                .enumerate()
                .min_by_key(|(_, &x)| x.abs_diff(best_idx))
                .map(|(p, _)| p)
                .unwrap_or(0);
            v[nearest] = best_idx;
            v.sort_unstable();
            v.dedup();
        }
        v
    };
    idxs.sort_unstable();
    idxs.dedup();
    idxs.into_iter()
        .map(|i| (candidates[i], i == best_idx))
        .collect()
}

const PLOT_W: usize = 41;
const PLOT_H: usize = 21;

fn ascii_plot(points: &[(f64, f64)], optimal: (f64, f64)) -> String {
    let mut grid = vec![vec![b' '; PLOT_W]; PLOT_H];
    let cell = |x: f64, y: f64| -> (usize, usize) {
        let col = (x.clamp(0.0, 1.0) * (PLOT_W - 1) as f64).round() as usize;
        let row = PLOT_H - 1 - (y.clamp(0.0, 1.0) * (PLOT_H - 1) as f64).round() as usize;
        (row, col)
    };
    for c in 0..PLOT_W {
        let x = c as f64 / (PLOT_W - 1) as f64;
        let (r, _) = cell(x, x);
        grid[r][c] = b'.';
    }
    for w in points.windows(2) {
        let (x0, y0) = w[0];
        let (x1, y1) = w[1];
        for s in 0..=64 {
            let t = s as f64 / 64.0;
            let (r, c) = cell(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
            grid[r][c] = b'*';
        }
    }
    let (r, c) = cell(optimal.0, optimal.1);
    grid[r][c] = b'+';

    let mut out = String::new();
    out.push_str("TPR (sensitivity)\n");
    for (r, row) in grid.iter().enumerate() {
        let label = match r {
            0 => "1.0",
            10 => "0.5",
            20 => "0.0",
            _ => "",
        };
        let _ = writeln!(
            out,
            "{label:>4} |{}",
            String::from_utf8_lossy(row).trim_end()
        );
    }
    let _ = writeln!(out, "     +{}", "-".repeat(PLOT_W));
    let _ = writeln!(out, "      0.0{}1.0", " ".repeat(PLOT_W - 6));
    out.push_str("      FPR (1 - specificity)\n");
    out.push_str("      * ROC curve   + chosen cutoff   . chance line");
    out
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

struct Cfg {
    decimals: usize,
    percent: bool,
}

impl Cfg {
    /// A plain number (AUC, z, Brier, Youden's J) — never percent-formatted.
    fn num(&self, v: f64) -> String {
        let v = if v == 0.0 { 0.0 } else { v };
        format!("{:.*}", self.decimals, v)
    }
    /// A rate in 0..1 — percent-formatted when `percent` is on.
    fn rate(&self, v: f64) -> String {
        if self.percent {
            format!("{:.*}%", self.decimals, v * 100.0)
        } else {
            self.num(v)
        }
    }
    fn orate(&self, v: Option<f64>) -> String {
        v.map(|x| self.rate(x)).unwrap_or_else(|| "n/a".to_string())
    }
    fn onum(&self, v: Option<f64>) -> String {
        v.map(|x| self.num(x)).unwrap_or_else(|| "n/a".to_string())
    }
    fn pval(&self, v: Option<f64>) -> String {
        match v {
            None => "n/a".to_string(),
            Some(p) if p < 0.0001 => "< 0.0001".to_string(),
            Some(p) => format!("{:.*}", self.decimals.max(4), p),
        }
    }
}

/// Thresholds are data values, printed at their shortest exact representation.
fn score_str(v: f64) -> String {
    if v.is_infinite() {
        return "+inf".to_string();
    }
    format!("{v}")
}

fn criterion_label(c: &str) -> &'static str {
    match c {
        "f1" => "maximum F1 score",
        "closest" => "closest point to the top-left corner",
        "accuracy" => "maximum accuracy",
        "cost" => "minimum misclassification cost",
        _ => "maximum Youden's J (sensitivity + specificity - 1)",
    }
}

fn op_rows(op: &Op, cfg: &Cfg) -> Vec<(String, String)> {
    vec![
        ("threshold (predict positive when score >= t)".into(), score_str(op.threshold)),
        ("sensitivity (recall, TPR)".into(), cfg.rate(op.sens)),
        ("specificity (TNR)".into(), cfg.rate(op.spec)),
        ("false positive rate (FPR)".into(), cfg.rate(op.fpr)),
        ("precision (PPV)".into(), cfg.orate(op.ppv)),
        ("negative predictive value (NPV)".into(), cfg.orate(op.npv)),
        ("accuracy".into(), cfg.rate(op.acc)),
        ("F1 score".into(), cfg.orate(op.f1)),
        ("Youden's J".into(), cfg.num(op.youden)),
        ("Matthews correlation (MCC)".into(), cfg.onum(op.mcc)),
        ("distance to (0, 1)".into(), cfg.num(op.dist)),
        ("misclassification cost".into(), cfg.num(op.cost)),
        ("true positives (TP)".into(), op.tp.to_string()),
        ("false positives (FP)".into(), op.fp.to_string()),
        ("true negatives (TN)".into(), op.tn.to_string()),
        ("false negatives (FN)".into(), op.fn_.to_string()),
    ]
}

fn summary_rows(r: &Report, cfg: &Cfg) -> Vec<(String, String)> {
    let mut rows = vec![
        ("observations".to_string(), r.n.to_string()),
        (
            format!("positives ({})", r.pos_label),
            r.n_pos.to_string(),
        ),
        (
            format!("negatives ({})", r.neg_label),
            r.n_neg.to_string(),
        ),
        ("distinct score values".to_string(), r.distinct_scores.to_string()),
        ("AUC".to_string(), cfg.num(r.auc)),
        (
            format!("{}% CI (DeLong)", r.conf),
            match r.ci {
                Some((lo, hi)) => format!("{} to {}", cfg.num(lo), cfg.num(hi)),
                None => "n/a".to_string(),
            },
        ),
        ("standard error (DeLong)".to_string(), cfg.onum(r.se)),
        ("z vs AUC = 0.5".to_string(), cfg.onum(r.z)),
        ("p-value (two-sided)".to_string(), cfg.pval(r.p)),
        ("Gini coefficient (2 x AUC - 1)".to_string(), cfg.num(r.gini)),
    ];
    if let Some(b) = r.brier {
        rows.push(("Brier score".to_string(), cfg.num(b)));
    }
    rows.push(("interpretation".to_string(), r.band.to_string()));
    rows.push(("positive class".to_string(), format!("{} ({})", r.pos_label, r.label_source)));
    rows
}

fn table_header() -> [&'static str; 13] {
    [
        "threshold >=",
        "sens",
        "spec",
        "FPR",
        "PPV",
        "NPV",
        "accuracy",
        "F1",
        "Youden J",
        "TP",
        "FP",
        "TN",
        "FN",
    ]
}

fn table_cells(op: &Op, cfg: &Cfg) -> Vec<String> {
    vec![
        score_str(op.threshold),
        cfg.rate(op.sens),
        cfg.rate(op.spec),
        cfg.rate(op.fpr),
        cfg.orate(op.ppv),
        cfg.orate(op.npv),
        cfg.rate(op.acc),
        cfg.orate(op.f1),
        cfg.num(op.youden),
        op.tp.to_string(),
        op.fp.to_string(),
        op.tn.to_string(),
        op.fn_.to_string(),
    ]
}

fn render_markdown(r: &Report, cfg: &Cfg) -> String {
    let mut s = String::new();
    s.push_str("## ROC / AUC summary\n\n| Metric | Value |\n| --- | --- |\n");
    for (k, v) in summary_rows(r, cfg) {
        let _ = writeln!(s, "| {k} | {v} |");
    }
    let _ = write!(
        s,
        "\n## Chosen cutoff — {}\n\n| Metric | Value |\n| --- | --- |\n",
        criterion_label(r.criterion)
    );
    for (k, v) in op_rows(&r.optimal, cfg) {
        let _ = writeln!(s, "| {k} | {v} |");
    }
    if r.criterion == "cost" {
        let _ = writeln!(
            s,
            "\nCost weighting: one false negative counts as {} false positives.",
            cfg.num(r.cost_ratio)
        );
    }
    if let Some(u) = &r.user {
        s.push_str("\n## Your threshold\n\n| Metric | Value |\n| --- | --- |\n");
        for (k, v) in op_rows(u, cfg) {
            let _ = writeln!(s, "| {k} | {v} |");
        }
    }
    if let Some(p) = &r.plot {
        let _ = write!(s, "\n## ROC curve\n\n```text\n{p}\n```\n");
    }
    if !r.table.is_empty() {
        s.push_str("\n## Threshold table\n\n");
        let head = table_header();
        let _ = writeln!(s, "| {} | {} |", "*", head.join(" | "));
        let _ = writeln!(s, "| --- |{}", " --- |".repeat(head.len()));
        for (op, is_best) in &r.table {
            let mark = if *is_best { "*" } else { "" };
            let _ = writeln!(s, "| {} | {} |", mark, table_cells(op, cfg).join(" | "));
        }
        s.push_str("\nRows marked `*` are the chosen cutoff.\n");
    }
    s.trim_end().to_string()
}

fn render_text(r: &Report, cfg: &Cfg) -> String {
    let mut s = String::new();
    s.push_str("ROC / AUC summary\n");
    let rows = summary_rows(r, cfg);
    let width = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(10);
    for (k, v) in &rows {
        let _ = writeln!(s, "  {k:<width$}  {v}");
    }
    let _ = write!(s, "\nChosen cutoff — {}\n", criterion_label(r.criterion));
    let op = op_rows(&r.optimal, cfg);
    let ow = op.iter().map(|(k, _)| k.len()).max().unwrap_or(10);
    for (k, v) in &op {
        let _ = writeln!(s, "  {k:<ow$}  {v}");
    }
    if r.criterion == "cost" {
        let _ = writeln!(
            s,
            "  cost weighting: one false negative counts as {} false positives",
            cfg.num(r.cost_ratio)
        );
    }
    if let Some(u) = &r.user {
        s.push_str("\nYour threshold\n");
        for (k, v) in op_rows(u, cfg) {
            let _ = writeln!(s, "  {k:<ow$}  {v}");
        }
    }
    if let Some(p) = &r.plot {
        let _ = write!(s, "\nROC curve\n{p}\n");
    }
    if !r.table.is_empty() {
        s.push_str("\nThreshold table (* = chosen cutoff)\n");
        let head = table_header();
        let mut grid: Vec<Vec<String>> = vec![{
            let mut h = vec![" ".to_string()];
            h.extend(head.iter().map(|x| x.to_string()));
            h
        }];
        for (op, is_best) in &r.table {
            let mut row = vec![if *is_best { "*".to_string() } else { " ".to_string() }];
            row.extend(table_cells(op, cfg));
            grid.push(row);
        }
        let cols = grid[0].len();
        let widths: Vec<usize> = (0..cols)
            .map(|c| grid.iter().map(|row| row[c].len()).max().unwrap_or(1))
            .collect();
        for row in &grid {
            let line: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(c, cellv)| format!("{cellv:>w$}", w = widths[c]))
                .collect();
            let _ = writeln!(s, "  {}", line.join("  "));
        }
    }
    s.trim_end().to_string()
}

fn render_csv(r: &Report, cfg: &Cfg) -> String {
    let mut s = String::new();
    s.push_str("section,metric,value\n");
    for (k, v) in summary_rows(r, cfg) {
        let _ = writeln!(s, "summary,{},{}", csv_cell(&k), csv_cell(&v));
    }
    for (k, v) in op_rows(&r.optimal, cfg) {
        let _ = writeln!(s, "chosen_cutoff,{},{}", csv_cell(&k), csv_cell(&v));
    }
    if let Some(u) = &r.user {
        for (k, v) in op_rows(u, cfg) {
            let _ = writeln!(s, "your_threshold,{},{}", csv_cell(&k), csv_cell(&v));
        }
    }
    if !r.table.is_empty() {
        s.push('\n');
        let _ = writeln!(s, "chosen,{}", table_header().join(","));
        for (op, is_best) in &r.table {
            let cells: Vec<String> = table_cells(op, cfg).iter().map(|c| csv_cell(c)).collect();
            let _ = writeln!(
                s,
                "{},{}",
                if *is_best { "yes" } else { "no" },
                cells.join(",")
            );
        }
    }
    s.trim_end().to_string()
}

fn csv_cell(v: &str) -> String {
    if v.contains(',') || v.contains('"') || v.contains('\n') {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_string()
    }
}

fn json_escape(v: &str) -> String {
    let mut out = String::with_capacity(v.len() + 2);
    for c in v.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn jnum(v: f64, cfg: &Cfg) -> String {
    format!("{:.*}", cfg.decimals, v)
}

fn jopt(v: Option<f64>, cfg: &Cfg) -> String {
    v.map(|x| jnum(x, cfg)).unwrap_or_else(|| "null".to_string())
}

fn json_op(op: &Op, cfg: &Cfg, indent: &str) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "{indent}{{");
    let _ = writeln!(s, "{indent}  \"threshold\": {},", score_str(op.threshold));
    let _ = writeln!(s, "{indent}  \"sensitivity\": {},", jnum(op.sens, cfg));
    let _ = writeln!(s, "{indent}  \"specificity\": {},", jnum(op.spec, cfg));
    let _ = writeln!(s, "{indent}  \"fpr\": {},", jnum(op.fpr, cfg));
    let _ = writeln!(s, "{indent}  \"ppv\": {},", jopt(op.ppv, cfg));
    let _ = writeln!(s, "{indent}  \"npv\": {},", jopt(op.npv, cfg));
    let _ = writeln!(s, "{indent}  \"accuracy\": {},", jnum(op.acc, cfg));
    let _ = writeln!(s, "{indent}  \"f1\": {},", jopt(op.f1, cfg));
    let _ = writeln!(s, "{indent}  \"youden_j\": {},", jnum(op.youden, cfg));
    let _ = writeln!(s, "{indent}  \"mcc\": {},", jopt(op.mcc, cfg));
    let _ = writeln!(s, "{indent}  \"distance_to_corner\": {},", jnum(op.dist, cfg));
    let _ = writeln!(s, "{indent}  \"cost\": {},", jnum(op.cost, cfg));
    let _ = writeln!(s, "{indent}  \"tp\": {}, \"fp\": {}, \"tn\": {}, \"fn\": {}", op.tp, op.fp, op.tn, op.fn_);
    let _ = write!(s, "{indent}}}");
    s
}

fn render_json(r: &Report, cfg: &Cfg) -> String {
    let mut s = String::new();
    s.push_str("{\n");
    let _ = writeln!(s, "  \"observations\": {},", r.n);
    let _ = writeln!(s, "  \"positives\": {},", r.n_pos);
    let _ = writeln!(s, "  \"negatives\": {},", r.n_neg);
    let _ = writeln!(s, "  \"positive_label\": \"{}\",", json_escape(&r.pos_label));
    let _ = writeln!(s, "  \"negative_label\": \"{}\",", json_escape(&r.neg_label));
    let _ = writeln!(s, "  \"positive_class_source\": \"{}\",", json_escape(r.label_source));
    let _ = writeln!(s, "  \"distinct_scores\": {},", r.distinct_scores);
    let _ = writeln!(s, "  \"curve_points\": {},", r.curve_points);
    let _ = writeln!(s, "  \"auc\": {},", jnum(r.auc, cfg));
    let _ = writeln!(s, "  \"auc_trapezoidal\": {},", jnum(r.auc_trapezoid, cfg));
    let _ = writeln!(s, "  \"gini\": {},", jnum(r.gini, cfg));
    let _ = writeln!(s, "  \"standard_error\": {},", jopt(r.se, cfg));
    let _ = writeln!(s, "  \"confidence_level\": {},", r.conf);
    match r.ci {
        Some((lo, hi)) => {
            let _ = writeln!(s, "  \"ci_lower\": {}, \"ci_upper\": {},", jnum(lo, cfg), jnum(hi, cfg));
        }
        None => {
            s.push_str("  \"ci_lower\": null, \"ci_upper\": null,\n");
        }
    }
    let _ = writeln!(s, "  \"z\": {},", jopt(r.z, cfg));
    let _ = writeln!(
        s,
        "  \"p_value\": {},",
        r.p.map(|p| format!("{:.*}", cfg.decimals.max(6), p))
            .unwrap_or_else(|| "null".to_string())
    );
    let _ = writeln!(s, "  \"brier_score\": {},", jopt(r.brier, cfg));
    let _ = writeln!(s, "  \"interpretation\": \"{}\",", json_escape(r.band));
    let _ = writeln!(s, "  \"criterion\": \"{}\",", json_escape(r.criterion));
    let _ = writeln!(s, "  \"cost_ratio\": {},", jnum(r.cost_ratio, cfg));
    let _ = writeln!(s, "  \"chosen_cutoff\":\n{},", json_op(&r.optimal, cfg, "  "));
    match &r.user {
        Some(u) => {
            let _ = writeln!(s, "  \"your_threshold\":\n{},", json_op(u, cfg, "  "));
        }
        None => s.push_str("  \"your_threshold\": null,\n"),
    }
    s.push_str("  \"threshold_table\": [\n");
    let n = r.table.len();
    for (i, (op, _)) in r.table.iter().enumerate() {
        s.push_str(&json_op(op, cfg, "    "));
        s.push_str(if i + 1 < n { ",\n" } else { "\n" });
    }
    s.push_str("  ]\n}");
    s
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULTS: (&str, f64, &str, &str, f64, bool, f64, bool) =
        ("youden", 1.0, "", "95", 12.0, true, 4.0, false);

    fn go(data: &str, format: &str) -> String {
        run(
            data, "", "auto", "auto", "auto", "auto", "", DEFAULTS.0, DEFAULTS.1, DEFAULTS.2,
            DEFAULTS.3, DEFAULTS.4, DEFAULTS.5, DEFAULTS.6, DEFAULTS.7, format,
        )
        .expect("run should succeed")
    }

    const PERFECT: &str = "0.9,1\n0.8,1\n0.7,1\n0.4,0\n0.3,0\n0.2,0";

    #[test]
    fn perfect_separation_scores_auc_one() {
        let out = go(PERFECT, "text");
        assert!(out.contains("AUC"), "{out}");
        assert!(out.contains("1.0000"), "{out}");
        assert!(out.contains("perfect separation"), "{out}");
    }

    #[test]
    fn ties_count_as_half() {
        // One positive and one negative share the same score: AUC = 0.5.
        let out = run(
            "0.5,1\n0.5,0", "", "auto", "auto", "auto", "auto", "", "youden", 1.0, "", "95", 0.0,
            false, 4.0, false, "json",
        )
        .unwrap();
        assert!(out.contains("\"auc\": 0.5000"), "{out}");
    }

    #[test]
    fn rank_auc_matches_the_trapezoidal_area() {
        let data = "0.91,1\n0.83,0\n0.83,1\n0.77,1\n0.60,0\n0.55,1\n0.41,0\n0.41,0\n0.22,1\n0.10,0";
        let out = go(data, "json");
        let auc = grab(&out, "\"auc\": ");
        let trap = grab(&out, "\"auc_trapezoidal\": ");
        assert_eq!(auc, trap, "{out}");
    }

    #[test]
    fn known_auc_value() {
        // 3 positives (0.9, 0.7, 0.4), 3 negatives (0.8, 0.6, 0.3):
        // pairs won = 2 + 2 + 1 = ... check by hand: 0.9 beats all 3, 0.7 beats
        // 0.6 and 0.3, 0.4 beats 0.3 => 6/9.
        let out = go("0.9,1\n0.8,0\n0.7,1\n0.6,0\n0.4,1\n0.3,0", "json");
        assert!(out.contains("\"auc\": 0.6667"), "{out}");
    }

    #[test]
    fn youden_cutoff_is_reported() {
        let out = go(PERFECT, "json");
        assert!(out.contains("\"criterion\": \"youden\""), "{out}");
        assert!(out.contains("\"threshold\": 0.7"), "{out}");
        assert!(out.contains("\"sensitivity\": 1.0000"), "{out}");
        assert!(out.contains("\"specificity\": 1.0000"), "{out}");
    }

    #[test]
    fn f1_criterion_can_pick_a_different_cutoff() {
        let data = "0.95,1\n0.90,0\n0.85,1\n0.80,1\n0.70,0\n0.60,1\n0.50,0\n0.40,0";
        let youden = run(
            data, "", "auto", "auto", "auto", "auto", "", "youden", 1.0, "", "95", 0.0, false,
            4.0, false, "json",
        )
        .unwrap();
        let f1 = run(
            data, "", "auto", "auto", "auto", "auto", "", "f1", 1.0, "", "95", 0.0, false, 4.0,
            false, "json",
        )
        .unwrap();
        assert!(f1.contains("\"criterion\": \"f1\""), "{f1}");
        assert!(youden.contains("\"criterion\": \"youden\""), "{youden}");
    }

    #[test]
    fn cost_criterion_prefers_catching_positives() {
        let data = "0.95,1\n0.90,0\n0.85,1\n0.80,1\n0.70,0\n0.60,1\n0.50,0\n0.40,0";
        let cheap = run(
            data, "", "auto", "auto", "auto", "auto", "", "cost", 1.0, "", "95", 0.0, false, 4.0,
            false, "json",
        )
        .unwrap();
        let costly = run(
            data, "", "auto", "auto", "auto", "auto", "", "cost", 20.0, "", "95", 0.0, false,
            4.0, false, "json",
        )
        .unwrap();
        let t_cheap: f64 = grab(&cheap, "\"threshold\": ").parse().unwrap();
        let t_costly: f64 = grab(&costly, "\"threshold\": ").parse().unwrap();
        assert!(
            t_costly <= t_cheap,
            "a costly false negative should lower the cutoff: {t_costly} vs {t_cheap}"
        );
    }

    #[test]
    fn delong_interval_brackets_the_auc() {
        // Overlapping classes so the DeLong variance is strictly positive.
        let mut data = String::new();
        for i in 0..40 {
            let _ = writeln!(data, "{},1", 0.2 + i as f64 / 100.0);
        }
        for i in 0..40 {
            let _ = writeln!(data, "{},0", i as f64 / 100.0);
        }
        let out = go(&data, "json");
        let auc: f64 = grab(&out, "\"auc\": ").parse().unwrap();
        let lo: f64 = grab(&out, "\"ci_lower\": ").parse().unwrap();
        let hi: f64 = grab(&out, "\"ci_upper\": ").parse().unwrap();
        let se: f64 = grab(&out, "\"standard_error\": ").parse().unwrap();
        assert!(se > 0.0, "{out}");
        assert!(lo < auc && auc < hi, "{lo} < {auc} < {hi}");
        assert!(out.contains("\"p_value\": 0.000000"), "{out}");
    }

    #[test]
    fn two_columns_input_shape() {
        let out = run(
            "0.9\n0.8\n0.2\n0.1",
            "yes\nyes\nno\nno",
            "columns",
            "auto",
            "newline",
            "no",
            "",
            "youden",
            1.0,
            "",
            "95",
            0.0,
            false,
            4.0,
            false,
            "json",
        )
        .unwrap();
        assert!(out.contains("\"auc\": 1.0000"), "{out}");
        assert!(out.contains("\"positive_label\": \"yes\""), "{out}");
    }

    #[test]
    fn label_first_column_order_is_detected() {
        let out = go("1,0.9\n1,0.8\ncontrol,0.2\ncontrol,0.1", "json");
        assert!(out.contains("\"auc\": 1.0000"), "{out}");
        assert!(out.contains("\"positive_label\": \"1\""), "{out}");
    }

    #[test]
    fn header_row_is_dropped_automatically() {
        let out = go("score,label\n0.9,1\n0.8,1\n0.2,0\n0.1,0", "json");
        assert!(out.contains("\"observations\": 4"), "{out}");
        assert!(out.contains("\"auc\": 1.0000"), "{out}");
    }

    #[test]
    fn tab_and_semicolon_separators_work() {
        let tabbed = go("0.9\t1\n0.8\t1\n0.2\t0\n0.1\t0", "json");
        assert!(tabbed.contains("\"auc\": 1.0000"), "{tabbed}");
        let semi = go("0.9;1\n0.8;1\n0.2;0\n0.1;0", "json");
        assert!(semi.contains("\"auc\": 1.0000"), "{semi}");
        let piped = go("0.9|1\n0.8|1\n0.2|0\n0.1|0", "json");
        assert!(piped.contains("\"auc\": 1.0000"), "{piped}");
        let spaced = go("0.9 1\n0.8 1\n0.2 0\n0.1 0", "json");
        assert!(spaced.contains("\"auc\": 1.0000"), "{spaced}");
    }

    #[test]
    fn positive_label_override_supports_more_than_two_classes() {
        let out = run(
            "0.9,cat\n0.8,cat\n0.3,dog\n0.2,mouse",
            "",
            "auto",
            "auto",
            "auto",
            "auto",
            "cat",
            "youden",
            1.0,
            "",
            "95",
            0.0,
            false,
            4.0,
            false,
            "json",
        )
        .unwrap();
        assert!(out.contains("\"auc\": 1.0000"), "{out}");
        assert!(out.contains("\"positive_label\": \"cat\""), "{out}");
        assert!(out.contains("\"negative_label\": \"not cat\""), "{out}");
    }

    #[test]
    fn brier_score_only_when_scores_are_probabilities() {
        let probs = go("0.9,1\n0.8,1\n0.2,0\n0.1,0", "json");
        assert!(probs.contains("\"brier_score\": 0.0"), "{probs}");
        let raw = go("9,1\n8,1\n2,0\n1,0", "json");
        assert!(raw.contains("\"brier_score\": null"), "{raw}");
    }

    #[test]
    fn user_threshold_section_is_reported() {
        let out = run(
            PERFECT, "", "auto", "auto", "auto", "auto", "", "youden", 1.0, "0.75", "95", 0.0,
            false, 4.0, false, "text",
        )
        .unwrap();
        assert!(out.contains("Your threshold"), "{out}");
        assert!(out.contains("0.6667"), "{out}");
    }

    #[test]
    fn percent_formatting_applies_to_rates_only() {
        let out = run(
            PERFECT, "", "auto", "auto", "auto", "auto", "", "youden", 1.0, "", "95", 0.0, false,
            2.0, true, "text",
        )
        .unwrap();
        assert!(out.contains("100.00%"), "{out}");
        assert!(out.contains("AUC") && out.contains("1.00"), "{out}");
    }

    #[test]
    fn threshold_table_respects_the_row_cap_and_keeps_the_optimum() {
        let mut data = String::new();
        for i in 0..30 {
            let _ = writeln!(data, "{},{}", 1.0 - i as f64 / 100.0, i % 2);
        }
        let out = run(
            &data, "", "auto", "auto", "auto", "auto", "", "youden", 1.0, "", "95", 5.0, false,
            4.0, false, "csv",
        )
        .unwrap();
        let rows = out.lines().filter(|l| l.starts_with("yes,") || l.starts_with("no,")).count();
        assert_eq!(rows, 5, "{out}");
        assert_eq!(out.lines().filter(|l| l.starts_with("yes,")).count(), 1, "{out}");
    }

    #[test]
    fn ascii_plot_is_emitted_by_default() {
        let out = go(PERFECT, "markdown");
        assert!(out.contains("## ROC curve"), "{out}");
        assert!(out.contains("* ROC curve"), "{out}");
        assert!(out.contains("+ chosen cutoff"), "{out}");
    }

    #[test]
    fn single_class_input_is_rejected() {
        let err = run(
            "0.9,1\n0.8,1", "", "auto", "auto", "auto", "auto", "", "youden", 1.0, "", "95",
            12.0, true, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(err.contains("only one class"), "{err}");
    }

    #[test]
    fn non_numeric_score_is_rejected_with_the_row_number() {
        let err = run(
            "0.9,1\nabc,0\n0.1,0", "", "auto", "auto", "auto", "auto", "", "youden", 1.0, "",
            "95", 12.0, true, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(err.contains("row 2"), "{err}");
        assert!(err.contains("numeric score"), "{err}");
    }

    #[test]
    fn unknown_positive_label_is_rejected() {
        let err = run(
            "0.9,yes\n0.1,no", "", "auto", "auto", "auto", "auto", "maybe", "youden", 1.0, "",
            "95", 12.0, true, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(err.contains("does not appear"), "{err}");
    }

    #[test]
    fn bad_enum_value_is_rejected() {
        let err = run(
            PERFECT, "", "auto", "auto", "auto", "auto", "", "youden", 1.0, "", "95", 12.0, true,
            4.0, false, "yaml",
        )
        .unwrap_err();
        assert!(err.contains("expected format to be one of"), "{err}");
    }

    #[test]
    fn observation_cap_is_enforced() {
        let mut data = String::new();
        for i in 0..(MAX_OBSERVATIONS + 1) {
            let _ = writeln!(data, "{},{}", i, i % 2);
        }
        let err = run(
            &data, "", "auto", "auto", "auto", "auto", "", "youden", 1.0, "", "95", 0.0, false,
            4.0, false, "json",
        )
        .unwrap_err();
        assert!(err.contains("at most 20000 observations"), "{err}");
    }

    #[test]
    fn csv_output_has_both_sections() {
        let out = go(PERFECT, "csv");
        assert!(out.starts_with("section,metric,value\n"), "{out}");
        assert!(out.contains("\nchosen,threshold >=,"), "{out}");
    }

    #[test]
    fn percent_scores_are_normalized() {
        let out = go("90%,1\n80%,1\n20%,0\n10%,0", "json");
        assert!(out.contains("\"auc\": 1.0000"), "{out}");
        assert!(out.contains("\"brier_score\": 0.0"), "{out}");
    }

    fn grab(json: &str, key: &str) -> String {
        let start = json.find(key).unwrap_or_else(|| panic!("missing {key} in {json}")) + key.len();
        let rest = &json[start..];
        let end = rest
            .find(|c: char| c == ',' || c == '\n' || c == '}')
            .unwrap_or(rest.len());
        rest[..end].trim().to_string()
    }
}
