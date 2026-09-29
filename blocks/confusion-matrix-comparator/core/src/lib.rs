//! confusion-matrix-comparator core — diff two confusion matrices entrywise and
//! report per-class deltas in precision, recall and F-score.
//!
//! Takes a baseline matrix (A) and a candidate matrix (B) — each as a K×K grid of
//! counts, as `actual,predicted` label pairs, or as `actual,predicted,count`
//! triples — reconciles their class order, and reports every headline metric as a
//! triple (A, B, Δ): accuracy, balanced accuracy, macro/weighted/micro
//! precision/recall/F-score, Cohen's kappa and the multiclass Matthews
//! correlation. Adds a per-class delta table, the entrywise `B − A` grid, the
//! biggest movers, a binary block when there are exactly two classes, and an
//! unpaired two-proportion test on the accuracy difference.
//!
//! Pure compute, no I/O: shared verbatim by the chat skill block, the CLI and the
//! browser page, so every surface returns byte-identical output.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Most classes we will diff. Beyond this the per-class table stops being
/// readable and the entrywise grid stops fitting on a page.
const MAX_CLASSES: usize = 50;
/// Most rows we will read out of one input field.
const MAX_ROWS: usize = 50_000;
/// Largest total count we will accept in one matrix.
const MAX_TOTAL: f64 = 1e12;

/// ln Γ(1/2) = ln √π — the only gamma value the normal CDF needs.
const LN_GAMMA_HALF: f64 = 0.572_364_942_924_700_1;

// ---------------------------------------------------------------------------
// numeric helpers
// ---------------------------------------------------------------------------

/// P(a, x) by its series expansion, for x below a + 1.
fn gamma_p_series(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut ap = a;
    let mut sum = 1.0 / a;
    let mut del = sum;
    for _ in 0..500 {
        ap += 1.0;
        del *= x / ap;
        sum += del;
        if del.abs() < sum.abs() * 1e-17 {
            break;
        }
    }
    sum * (-x + a * x.ln() - LN_GAMMA_HALF).exp()
}

/// Q(a, x) by the modified Lentz continued fraction, for x above a + 1.
fn gamma_q_cf(a: f64, x: f64) -> f64 {
    const FPMIN: f64 = 1e-300;
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / FPMIN;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..500 {
        let i = i as f64;
        let an = -i * (i - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = b + an / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-17 {
            break;
        }
    }
    (-x + a * x.ln() - LN_GAMMA_HALF).exp() * h
}

/// The complementary error function, via the incomplete gamma (≈1e-15 accurate).
fn erfc(x: f64) -> f64 {
    let x2 = x * x;
    let split = x2 < 1.5;
    if x >= 0.0 {
        if split {
            1.0 - gamma_p_series(0.5, x2)
        } else {
            gamma_q_cf(0.5, x2)
        }
    } else if split {
        1.0 + gamma_p_series(0.5, x2)
    } else {
        2.0 - gamma_q_cf(0.5, x2)
    }
}

/// The standard normal survival function — the area to the RIGHT of `z`.
fn norm_sf(z: f64) -> f64 {
    0.5 * erfc(z / std::f64::consts::SQRT_2)
}

/// Round half away from zero at `decimals`, so the printed figure and the JSON
/// number agree.
fn round_to(v: f64, decimals: usize) -> f64 {
    if !v.is_finite() {
        return v;
    }
    let f = 10f64.powi(decimals as i32);
    (v * f).round() / f
}

// ---------------------------------------------------------------------------
// parsing
// ---------------------------------------------------------------------------

/// Column names we recognise so a pasted header row can be dropped even when its
/// cells would otherwise read as class labels.
const HEADER_TOKENS: &[&str] = &[
    "",
    "actual",
    "actuals",
    "predicted",
    "prediction",
    "predictions",
    "pred",
    "true",
    "truth",
    "y_true",
    "y_pred",
    "ytrue",
    "ypred",
    "y",
    "class",
    "classes",
    "label",
    "labels",
    "gold",
    "reference",
    "target",
    "expected",
    "output",
    "count",
    "counts",
    "n",
    "freq",
    "frequency",
    "support",
    "total",
];

/// A number, tolerating a leading `+` and surrounding space.
fn parse_number(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let t = t.strip_prefix('+').unwrap_or(t);
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn is_number(s: &str) -> bool {
    parse_number(s).is_some()
}

/// Order labels numerically when they are numbers, alphabetically otherwise, with
/// numbers first — the order scikit-learn's `classes_` would produce.
fn label_order(a: &str, b: &str) -> Ordering {
    match (parse_number(a), parse_number(b)) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal).then(a.cmp(b)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

/// Split a field into rows, dropping blank lines.
fn rows_of(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect()
}

/// Split one row by the named separator. `space` collapses runs of whitespace.
fn split_row(row: &str, sep: &str) -> Vec<String> {
    let parts: Vec<String> = match sep {
        "comma" => row.split(',').map(|s| s.trim().to_string()).collect(),
        "tab" => row.split('\t').map(|s| s.trim().to_string()).collect(),
        "semicolon" => row.split(';').map(|s| s.trim().to_string()).collect(),
        "pipe" => row.split('|').map(|s| s.trim().to_string()).collect(),
        _ => row.split_whitespace().map(|s| s.to_string()).collect(),
    };
    parts
}

/// Pick the separator that splits every row into the same number of fields, more
/// than one. Tried in the order a paste is most likely to use.
fn detect_separator(rows: &[&str]) -> Option<String> {
    for cand in ["comma", "tab", "semicolon", "pipe", "space"] {
        let mut width = None;
        let mut ok = true;
        for row in rows {
            let n = split_row(row, cand).len();
            if n < 2 {
                ok = false;
                break;
            }
            match width {
                None => width = Some(n),
                Some(w) if w == n => {}
                Some(_) => {
                    ok = false;
                    break;
                }
            }
        }
        if ok && width.is_some() {
            return Some(cand.to_string());
        }
    }
    None
}

/// Split the optional `labels` field. Never splits on spaces, so class names may
/// contain them.
fn split_labels(text: &str) -> Vec<String> {
    text.split(['\n', '\r', ',', '\t', ';', '|'])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// One parsed input, before the two matrices' class orders are reconciled.
enum Raw {
    /// A K×K grid of counts, with class names when the paste carried them.
    Grid {
        labels: Option<Vec<String>>,
        m: Vec<Vec<f64>>,
    },
    /// `(actual, predicted, count)` triples, still to be tallied.
    Triples(Vec<(String, String, f64)>),
}

impl Raw {
    fn labels(&self) -> Option<&Vec<String>> {
        match self {
            Raw::Grid { labels, .. } => labels.as_ref(),
            Raw::Triples(_) => None,
        }
    }
}

/// Validate one count cell.
fn count_of(field: &str, who: &str) -> Result<f64, String> {
    let v = parse_number(field).ok_or_else(|| {
        format!("{who}: expected a count, got `{field}` — confusion-matrix cells must be numbers")
    })?;
    if v < 0.0 {
        return Err(format!(
            "{who}: counts cannot be negative, got `{field}`"
        ));
    }
    if (v - v.round()).abs() > 1e-9 {
        return Err(format!(
            "{who}: counts must be whole numbers, got `{field}` — normalised or averaged matrices cannot be compared by support"
        ));
    }
    Ok(v.round())
}

/// Read one matrix field into a [`Raw`].
fn parse_input(
    text: &str,
    who: &str,
    input_format: &str,
    separator: &str,
    header: &str,
    orientation: &str,
) -> Result<Raw, String> {
    let rows = rows_of(text);
    if rows.is_empty() {
        return Err(format!("{who}: no data — paste a confusion matrix"));
    }
    if rows.len() > MAX_ROWS {
        return Err(format!(
            "{who}: {} rows is more than the {MAX_ROWS} this tool reads",
            rows.len()
        ));
    }

    let sep = if separator == "auto" {
        detect_separator(&rows).ok_or_else(|| {
            format!(
                "{who}: could not find a column separator — every row must split into the same number of fields by comma, tab, semicolon, pipe or spaces (row 1 is `{}`)",
                rows[0]
            )
        })?
    } else {
        separator.to_string()
    };

    let mut grid: Vec<Vec<String>> = rows.iter().map(|r| split_row(r, &sep)).collect();
    let widths: Vec<usize> = grid.iter().map(Vec::len).collect();
    if widths.iter().any(|&w| w < 2) {
        return Err(format!(
            "{who}: row {} splits into fewer than 2 fields with the `{sep}` separator — pick the separator your paste actually uses",
            widths.iter().position(|&w| w < 2).unwrap() + 1
        ));
    }

    // ---- header row ----
    let looks_like_header = {
        let first = &grid[0];
        let no_numbers = !first.iter().any(|f| is_number(f));
        let all_tokens = first
            .iter()
            .all(|f| HEADER_TOKENS.contains(&f.to_ascii_lowercase().trim()));
        let rest_numeric = grid.len() > 1
            && grid[1..]
                .iter()
                .all(|r| r.iter().skip(1).all(|f| is_number(f)));
        all_tokens || (no_numbers && rest_numeric)
    };
    let take_header = match header {
        "yes" => true,
        "no" => false,
        _ => looks_like_header,
    };
    let header_row = if take_header {
        if grid.len() < 2 {
            return Err(format!(
                "{who}: the only row was read as a header — set 'First row is a header' to No, or paste some data"
            ));
        }
        Some(grid.remove(0))
    } else {
        None
    };

    // ---- row-label column ----
    let width = grid[0].len();
    let has_label_col = grid.iter().all(|r| r.len() == width)
        && grid
            .iter()
            .all(|r| !is_number(&r[0]) && r[1..].iter().all(|f| is_number(f)));
    let mut row_labels: Option<Vec<String>> = None;
    if has_label_col {
        row_labels = Some(grid.iter().map(|r| r[0].clone()).collect());
        for r in grid.iter_mut() {
            r.remove(0);
        }
    }

    // Header labels line up with the data columns: drop a corner cell when the
    // header is one field wider than the body.
    let header_labels = header_row.as_ref().map(|h| {
        let body = grid[0].len();
        if h.len() == body + 1 {
            h[1..].to_vec()
        } else {
            h.clone()
        }
    });

    let widths: Vec<usize> = grid.iter().map(Vec::len).collect();
    let uniform = widths.iter().all(|&w| w == widths[0]);
    let all_numeric = grid.iter().all(|r| r.iter().all(|f| is_number(f)));
    let square = uniform && widths[0] == grid.len();

    let chosen = match input_format {
        "matrix" => "matrix",
        "labels" => "labels",
        "table" => "table",
        _ => {
            // An all-numeric paste is read positionally: square is a matrix, two
            // columns are `y_true,y_pred` values, anything else is ambiguous and
            // asks for the shape rather than guessing.
            if uniform && all_numeric {
                if square {
                    "matrix"
                } else if widths[0] == 2 {
                    "labels"
                } else {
                    return Err(format!(
                        "{who}: a {}×{} grid of numbers is not square — a confusion matrix needs one row and one column per class. Set the input shape to `table` if these are actual,predicted,count triples.",
                        grid.len(),
                        widths[0]
                    ));
                }
            } else if uniform && widths[0] == 3 && grid.iter().all(|r| is_number(&r[2])) {
                "table"
            } else if uniform && widths[0] == 2 {
                "labels"
            } else {
                return Err(format!(
                    "{who}: could not tell what this is — paste a square grid of counts, two columns of `actual,predicted` labels, or three columns of `actual,predicted,count`, or set the input shape explicitly"
                ));
            }
        }
    };

    match chosen {
        "matrix" => {
            if !uniform {
                return Err(format!(
                    "{who}: matrix rows have different lengths ({} then {})",
                    widths[0],
                    widths.iter().find(|&&w| w != widths[0]).unwrap()
                ));
            }
            if widths[0] != grid.len() {
                return Err(format!(
                    "{who}: a confusion matrix must be square, got {} rows × {} columns",
                    grid.len(),
                    widths[0]
                ));
            }
            if grid.len() < 2 {
                return Err(format!(
                    "{who}: need at least 2 classes, got a {}×{} matrix",
                    grid.len(),
                    widths[0]
                ));
            }
            if grid.len() > MAX_CLASSES {
                return Err(format!(
                    "{who}: {} classes is more than the {MAX_CLASSES} this tool compares",
                    grid.len()
                ));
            }
            let mut m = Vec::with_capacity(grid.len());
            for (i, r) in grid.iter().enumerate() {
                let mut out = Vec::with_capacity(r.len());
                for (j, f) in r.iter().enumerate() {
                    out.push(count_of(f, &format!("{who} cell (row {}, column {})", i + 1, j + 1))?);
                }
                m.push(out);
            }
            if orientation == "actual_columns" {
                m = transpose(&m);
            }
            // Row labels and header labels must agree when both are present.
            let labels = match (&row_labels, &header_labels) {
                (Some(r), Some(h)) if r.len() == h.len() => {
                    let (mut rs, mut hs) = (r.clone(), h.clone());
                    rs.sort();
                    hs.sort();
                    if rs != hs {
                        return Err(format!(
                            "{who}: the row labels ({}) and the column header ({}) name different classes",
                            r.join(", "),
                            h.join(", ")
                        ));
                    }
                    Some(r.clone())
                }
                (Some(r), _) => Some(r.clone()),
                (_, Some(h)) => Some(h.clone()),
                _ => None,
            };
            if let Some(l) = &labels {
                if l.len() != m.len() {
                    return Err(format!(
                        "{who}: {} class names for a {}×{} matrix",
                        l.len(),
                        m.len(),
                        m.len()
                    ));
                }
                let mut seen = l.clone();
                seen.sort();
                seen.dedup();
                if seen.len() != l.len() {
                    return Err(format!("{who}: duplicate class name in `{}`", l.join(", ")));
                }
            }
            Ok(Raw::Grid { labels, m })
        }
        "table" => {
            if !uniform || widths[0] != 3 {
                return Err(format!(
                    "{who}: an `actual,predicted,count` table needs exactly 3 fields per row, got {}",
                    widths[0]
                ));
            }
            let mut out = Vec::with_capacity(grid.len());
            for (i, r) in grid.iter().enumerate() {
                let n = count_of(&r[2], &format!("{who} row {}", i + 1))?;
                let (a, p) = if orientation == "actual_columns" {
                    (r[1].clone(), r[0].clone())
                } else {
                    (r[0].clone(), r[1].clone())
                };
                if a.is_empty() || p.is_empty() {
                    return Err(format!("{who} row {}: blank class label", i + 1));
                }
                out.push((a, p, n));
            }
            Ok(Raw::Triples(out))
        }
        _ => {
            if !uniform || widths[0] != 2 {
                return Err(format!(
                    "{who}: an `actual,predicted` label list needs exactly 2 fields per row, got {}",
                    widths[0]
                ));
            }
            let mut out = Vec::with_capacity(grid.len());
            for (i, r) in grid.iter().enumerate() {
                let (a, p) = if orientation == "actual_columns" {
                    (r[1].clone(), r[0].clone())
                } else {
                    (r[0].clone(), r[1].clone())
                };
                if a.is_empty() || p.is_empty() {
                    return Err(format!("{who} row {}: blank class label", i + 1));
                }
                out.push((a, p, 1.0));
            }
            Ok(Raw::Triples(out))
        }
    }
}

fn transpose(m: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let k = m.len();
    (0..k).map(|i| (0..k).map(|j| m[j][i]).collect()).collect()
}

/// Reorder a labelled grid onto the canonical class order.
fn reorder(m: &[Vec<f64>], from: &[String], to: &[String], who: &str) -> Result<Vec<Vec<f64>>, String> {
    let idx: BTreeMap<&str, usize> = from.iter().map(|s| s.as_str()).zip(0..).collect();
    let mut pos = Vec::with_capacity(to.len());
    for l in to {
        match idx.get(l.as_str()) {
            Some(&i) => pos.push(i),
            None => {
                return Err(format!(
                    "{who}: no class `{l}` — it has {}, the comparison needs {}",
                    from.join(", "),
                    to.join(", ")
                ))
            }
        }
    }
    Ok(pos
        .iter()
        .map(|&i| pos.iter().map(|&j| m[i][j]).collect())
        .collect())
}

/// Tally triples onto the canonical class order.
fn tally(
    triples: &[(String, String, f64)],
    labels: &[String],
    who: &str,
) -> Result<Vec<Vec<f64>>, String> {
    let idx: BTreeMap<&str, usize> = labels.iter().map(|s| s.as_str()).zip(0..).collect();
    let k = labels.len();
    let mut m = vec![vec![0.0; k]; k];
    for (a, p, n) in triples {
        let i = *idx.get(a.as_str()).ok_or_else(|| {
            format!(
                "{who}: actual label `{a}` is not one of the compared classes ({})",
                labels.join(", ")
            )
        })?;
        let j = *idx.get(p.as_str()).ok_or_else(|| {
            format!(
                "{who}: predicted label `{p}` is not one of the compared classes ({})",
                labels.join(", ")
            )
        })?;
        m[i][j] += n;
    }
    Ok(m)
}

// ---------------------------------------------------------------------------
// metrics
// ---------------------------------------------------------------------------

/// Every figure one confusion matrix yields.
struct Metrics {
    m: Vec<Vec<f64>>,
    total: f64,
    correct: f64,
    accuracy: f64,
    support: Vec<f64>,
    predicted: Vec<f64>,
    tp: Vec<f64>,
    fp: Vec<f64>,
    fneg: Vec<f64>,
    tn: Vec<f64>,
    precision: Vec<f64>,
    recall: Vec<f64>,
    fscore: Vec<f64>,
    specificity: Vec<f64>,
    undefined_precision: Vec<usize>,
    undefined_recall: Vec<usize>,
    macro_p: f64,
    macro_r: f64,
    macro_f: f64,
    weighted_p: f64,
    weighted_r: f64,
    weighted_f: f64,
    balanced_accuracy: f64,
    kappa: f64,
    mcc: f64,
}

/// A rate whose denominator may be zero. scikit-learn's convention: report 0 and
/// remember that it was undefined so the report can say so.
fn ratio(num: f64, den: f64, undefined: &mut bool) -> f64 {
    if den <= 0.0 {
        *undefined = true;
        0.0
    } else {
        num / den
    }
}

fn metrics(m: Vec<Vec<f64>>, beta: f64) -> Metrics {
    let k = m.len();
    let support: Vec<f64> = m.iter().map(|r| r.iter().sum()).collect();
    let predicted: Vec<f64> = (0..k).map(|j| (0..k).map(|i| m[i][j]).sum()).collect();
    let total: f64 = support.iter().sum();
    let correct: f64 = (0..k).map(|i| m[i][i]).sum();

    let tp: Vec<f64> = (0..k).map(|i| m[i][i]).collect();
    let fp: Vec<f64> = (0..k).map(|i| predicted[i] - tp[i]).collect();
    let fneg: Vec<f64> = (0..k).map(|i| support[i] - tp[i]).collect();
    let tn: Vec<f64> = (0..k).map(|i| total - tp[i] - fp[i] - fneg[i]).collect();

    let b2 = beta * beta;
    let mut precision = Vec::with_capacity(k);
    let mut recall = Vec::with_capacity(k);
    let mut fscore = Vec::with_capacity(k);
    let mut specificity = Vec::with_capacity(k);
    let mut undefined_precision = Vec::new();
    let mut undefined_recall = Vec::new();
    for i in 0..k {
        let mut up = false;
        let p = ratio(tp[i], tp[i] + fp[i], &mut up);
        let mut ur = false;
        let r = ratio(tp[i], tp[i] + fneg[i], &mut ur);
        if up {
            undefined_precision.push(i);
        }
        if ur {
            undefined_recall.push(i);
        }
        let mut _u = false;
        let f = if b2 * p + r > 0.0 {
            (1.0 + b2) * p * r / (b2 * p + r)
        } else {
            0.0
        };
        specificity.push(ratio(tn[i], tn[i] + fp[i], &mut _u));
        precision.push(p);
        recall.push(r);
        fscore.push(f);
    }

    let kf = k as f64;
    let mean = |v: &Vec<f64>| v.iter().sum::<f64>() / kf;
    let wmean = |v: &Vec<f64>| {
        if total <= 0.0 {
            0.0
        } else {
            v.iter()
                .zip(&support)
                .map(|(x, s)| x * s)
                .sum::<f64>()
                / total
        }
    };

    let accuracy = if total > 0.0 { correct / total } else { 0.0 };

    // Cohen's kappa: observed agreement against the agreement two independent
    // raters with these marginals would reach by chance.
    let pe: f64 = if total > 0.0 {
        (0..k)
            .map(|i| (support[i] / total) * (predicted[i] / total))
            .sum()
    } else {
        0.0
    };
    let kappa = if (1.0 - pe).abs() < 1e-15 {
        if (accuracy - 1.0).abs() < 1e-15 {
            1.0
        } else {
            0.0
        }
    } else {
        (accuracy - pe) / (1.0 - pe)
    };

    // Multiclass Matthews correlation (Gorodkin's R_K).
    let s = total;
    let c = correct;
    let sum_pt: f64 = (0..k).map(|i| predicted[i] * support[i]).sum();
    let sum_p2: f64 = predicted.iter().map(|v| v * v).sum();
    let sum_t2: f64 = support.iter().map(|v| v * v).sum();
    let den = ((s * s - sum_p2) * (s * s - sum_t2)).sqrt();
    let mcc = if den > 0.0 { (c * s - sum_pt) / den } else { 0.0 };

    Metrics {
        m,
        total,
        correct,
        accuracy,
        macro_p: mean(&precision),
        macro_r: mean(&recall),
        macro_f: mean(&fscore),
        weighted_p: wmean(&precision),
        weighted_r: wmean(&recall),
        weighted_f: wmean(&fscore),
        balanced_accuracy: mean(&recall),
        kappa,
        mcc,
        support,
        predicted,
        tp,
        fp,
        fneg,
        tn,
        precision,
        recall,
        fscore,
        specificity,
        undefined_precision,
        undefined_recall,
    }
}

// ---------------------------------------------------------------------------
// formatting
// ---------------------------------------------------------------------------

/// Decimal places + percent formatting, shared by every output format.
struct Fmt {
    decimals: usize,
    percent: bool,
}

/// Prefix a formatted magnitude with its sign, leaving an exact zero unsigned.
fn sign_wrap(body: String, v: f64) -> String {
    if body.chars().all(|c| matches!(c, '0' | '.' | '%')) {
        body
    } else if v < 0.0 {
        format!("-{body}")
    } else {
        format!("+{body}")
    }
}

impl Fmt {
    /// A rate that lives in 0…1 — shown as a percentage when asked.
    fn rate(&self, v: f64) -> String {
        if self.percent {
            format!("{:.*}%", self.decimals, v * 100.0)
        } else {
            format!("{:.*}", self.decimals, v)
        }
    }
    fn rate_delta(&self, v: f64) -> String {
        sign_wrap(self.rate(v.abs()), v)
    }
    /// A plain number (kappa, MCC, z) — never percent-formatted.
    fn num(&self, v: f64) -> String {
        format!("{:.*}", self.decimals, v)
    }
    fn num_delta(&self, v: f64) -> String {
        sign_wrap(self.num(v.abs()), v)
    }
    fn count(&self, v: f64) -> String {
        format!("{}", v.round() as i64)
    }
    fn count_delta(&self, v: f64) -> String {
        sign_wrap(self.count(v.abs()), v)
    }
    /// A p-value keeps at least 4 decimals so a significant result never prints
    /// as a bare 0.
    fn pval(&self, v: f64) -> String {
        format!("{:.*}", self.decimals.max(4), v)
    }
}

/// `F1` when beta is 1, otherwise `F0.5`, `F2`, …
fn fscore_name(beta: f64) -> String {
    if (beta - 1.0).abs() < 1e-12 {
        "F1".to_string()
    } else {
        let s = format!("{beta}");
        format!("F{s}")
    }
}

fn md_table(headers: &[String], rows: &[Vec<String>], out: &mut String) {
    let _ = writeln!(out, "| {} |", headers.join(" | "));
    let _ = writeln!(
        out,
        "| {} |",
        headers.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
    );
    for r in rows {
        let _ = writeln!(out, "| {} |", r.join(" | "));
    }
}

/// A plain-text table: left-align the first column, right-align the rest.
fn text_table(headers: &[String], rows: &[Vec<String>], out: &mut String) {
    let n = headers.len();
    let mut w: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for r in rows {
        for i in 0..n {
            w[i] = w[i].max(r.get(i).map(|c| c.chars().count()).unwrap_or(0));
        }
    }
    let line = |cells: &[String]| -> String {
        let mut s = String::new();
        for i in 0..n {
            let c = cells.get(i).cloned().unwrap_or_default();
            if i == 0 {
                let _ = write!(s, "{:<width$}", c, width = w[i]);
            } else {
                let _ = write!(s, "  {:>width$}", c, width = w[i]);
            }
        }
        s.trim_end().to_string()
    };
    let _ = writeln!(out, "{}", line(headers));
    let _ = writeln!(
        out,
        "{}",
        line(&w.iter().map(|&x| "-".repeat(x)).collect::<Vec<_>>())
    );
    for r in rows {
        let _ = writeln!(out, "{}", line(r));
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn csv_row(cells: &[String]) -> String {
    cells
        .iter()
        .map(|c| csv_escape(c))
        .collect::<Vec<_>>()
        .join(",")
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
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// A JSON number at the report's precision.
fn jnum(v: f64, decimals: usize) -> String {
    format!("{:.*}", decimals, round_to(v, decimals))
}

// ---------------------------------------------------------------------------
// assembled comparison
// ---------------------------------------------------------------------------

/// One row of the per-class delta table.
struct ClassRow {
    idx: usize,
    label: String,
    sup_a: f64,
    sup_b: f64,
    p_a: f64,
    p_b: f64,
    r_a: f64,
    r_b: f64,
    f_a: f64,
    f_b: f64,
}

impl ClassRow {
    fn dp(&self) -> f64 {
        self.p_b - self.p_a
    }
    fn dr(&self) -> f64 {
        self.r_b - self.r_a
    }
    fn df(&self) -> f64 {
        self.f_b - self.f_a
    }
}

/// The unpaired two-proportion test on the accuracy difference.
struct AccTest {
    diff: f64,
    ci_lower: f64,
    ci_upper: f64,
    z: Option<f64>,
    p: Option<f64>,
    level: u32,
    significant: bool,
}

fn accuracy_test(a: &Metrics, b: &Metrics, level: u32) -> AccTest {
    let (n1, n2) = (a.total, b.total);
    let (p1, p2) = (a.accuracy, b.accuracy);
    let diff = p2 - p1;
    let z_crit = match level {
        90 => 1.644_853_626_951_472_2,
        99 => 2.575_829_303_548_900_4,
        _ => 1.959_963_984_540_054,
    };
    let se_unpooled = (p1 * (1.0 - p1) / n1 + p2 * (1.0 - p2) / n2).sqrt();
    let ci_lower = (diff - z_crit * se_unpooled).max(-1.0);
    let ci_upper = (diff + z_crit * se_unpooled).min(1.0);
    let pooled = (a.correct + b.correct) / (n1 + n2);
    let se_pooled = (pooled * (1.0 - pooled) * (1.0 / n1 + 1.0 / n2)).sqrt();
    let (z, p) = if se_pooled > 0.0 {
        let z = diff / se_pooled;
        (Some(z), Some((2.0 * norm_sf(z.abs())).clamp(0.0, 1.0)))
    } else {
        (None, None)
    };
    let alpha = 1.0 - level as f64 / 100.0;
    AccTest {
        diff,
        ci_lower,
        ci_upper,
        z,
        p,
        level,
        significant: p.map(|p| p < alpha).unwrap_or(false),
    }
}

/// Validate an enum-valued option, treating empty as the default (the first).
fn allowed(name: &str, v: &str, opts: &[&str]) -> Result<String, String> {
    let t = v.trim().to_ascii_lowercase();
    if t.is_empty() {
        return Ok(opts[0].to_string());
    }
    if opts.contains(&t.as_str()) {
        Ok(t)
    } else {
        Err(format!(
            "{name} must be one of {}, got `{v}`",
            opts.join(", ")
        ))
    }
}

/// Diff two confusion matrices and report per-class deltas.
///
/// `matrix_a` is the baseline and `matrix_b` the candidate; each may be a K×K
/// grid of counts, two columns of `actual,predicted` labels, or three columns of
/// `actual,predicted,count`. `labels` fixes the class order. See the block
/// descriptor for the meaning of every option.
#[allow(clippy::too_many_arguments)]
pub fn run(
    matrix_a: &str,
    matrix_b: &str,
    labels: &str,
    name_a: &str,
    name_b: &str,
    input_format: &str,
    orientation: &str,
    separator: &str,
    header: &str,
    beta: f64,
    sort_by: &str,
    significance: bool,
    confidence_level: &str,
    matrix_delta: bool,
    decimals: f64,
    percent: bool,
    format: &str,
) -> Result<String, String> {
    let input_format = allowed("input", input_format, &["auto", "matrix", "labels", "table"])?;
    let orientation = allowed(
        "orientation",
        orientation,
        &["actual_rows", "actual_columns"],
    )?;
    let separator = allowed(
        "separator",
        separator,
        &["auto", "comma", "tab", "semicolon", "pipe", "space"],
    )?;
    let header = allowed("header", header, &["auto", "yes", "no"])?;
    let sort_by = allowed(
        "sort",
        sort_by,
        &[
            "class",
            "f1_delta",
            "regression",
            "precision_delta",
            "recall_delta",
            "support",
        ],
    )?;
    let confidence_level = allowed("confidence level", confidence_level, &["95", "90", "99"])?;
    let format = allowed("format", format, &["markdown", "text", "csv", "json"])?;
    if !beta.is_finite() || beta < 0.1 || beta > 10.0 {
        return Err(format!(
            "beta must be between 0.1 and 10, got `{beta}` — 1 weights precision and recall equally"
        ));
    }
    if !decimals.is_finite() || decimals < 0.0 || decimals > 10.0 {
        return Err(format!("decimals must be a whole number between 0 and 10, got `{decimals}`"));
    }
    let decimals = decimals.round() as usize;
    let level: u32 = confidence_level.parse().unwrap_or(95);

    let name_a = {
        let t = name_a.trim();
        if t.is_empty() {
            "Model A".to_string()
        } else {
            t.to_string()
        }
    };
    let name_b = {
        let t = name_b.trim();
        if t.is_empty() {
            "Model B".to_string()
        } else {
            t.to_string()
        }
    };
    if name_a == name_b {
        return Err(format!(
            "the two models need different names — both are called `{name_a}`"
        ));
    }

    let raw_a = parse_input(
        matrix_a,
        "matrix A",
        &input_format,
        &separator,
        &header,
        &orientation,
    )?;
    let raw_b = parse_input(
        matrix_b,
        "matrix B",
        &input_format,
        &separator,
        &header,
        &orientation,
    )?;

    // ---- reconcile the class order ----
    let explicit = split_labels(labels);
    let order: Vec<String> = if !explicit.is_empty() {
        let mut seen = explicit.clone();
        seen.sort();
        seen.dedup();
        if seen.len() != explicit.len() {
            return Err(format!(
                "the class list repeats a name: `{}`",
                explicit.join(", ")
            ));
        }
        if explicit.len() < 2 {
            return Err("the class list needs at least 2 class names".to_string());
        }
        explicit
    } else if let Some(l) = raw_a.labels() {
        l.clone()
    } else if let Some(l) = raw_b.labels() {
        l.clone()
    } else {
        // No names anywhere: take them from the tallied labels, or fall back to
        // scikit-learn's positional 0…K-1.
        let mut set: Vec<String> = Vec::new();
        for raw in [&raw_a, &raw_b] {
            if let Raw::Triples(t) = raw {
                for (a, p, _) in t {
                    for l in [a, p] {
                        if !set.contains(l) {
                            set.push(l.clone());
                        }
                    }
                }
            }
        }
        if set.is_empty() {
            let k = match &raw_a {
                Raw::Grid { m, .. } => m.len(),
                Raw::Triples(_) => 0,
            };
            (0..k).map(|i| i.to_string()).collect()
        } else {
            set.sort_by(|a, b| label_order(a, b));
            set
        }
    };
    if order.len() < 2 {
        return Err("need at least 2 classes to compare".to_string());
    }
    if order.len() > MAX_CLASSES {
        return Err(format!(
            "{} classes is more than the {MAX_CLASSES} this tool compares",
            order.len()
        ));
    }

    let build = |raw: &Raw, who: &str| -> Result<Vec<Vec<f64>>, String> {
        match raw {
            Raw::Grid { labels: Some(l), m } => reorder(m, l, &order, who),
            Raw::Grid { labels: None, m } => {
                if m.len() != order.len() {
                    return Err(format!(
                        "{who} is {}×{} but the comparison has {} classes ({}) — give it a label row, or fix the class list",
                        m.len(),
                        m.len(),
                        order.len(),
                        order.join(", ")
                    ));
                }
                Ok(m.clone())
            }
            Raw::Triples(t) => tally(t, &order, who),
        }
    };
    let m_a = build(&raw_a, "matrix A")?;
    let m_b = build(&raw_b, "matrix B")?;

    for (m, who) in [(&m_a, "matrix A"), (&m_b, "matrix B")] {
        let total: f64 = m.iter().flatten().sum();
        if total <= 0.0 {
            return Err(format!("{who} is all zeros — there is nothing to score"));
        }
        if total > MAX_TOTAL {
            return Err(format!(
                "{who} totals {total:.0} observations, more than this tool adds up"
            ));
        }
    }

    let a = metrics(m_a, beta);
    let b = metrics(m_b, beta);
    let k = order.len();
    let fname = fscore_name(beta);
    let f = Fmt { decimals, percent };

    let mut rows: Vec<ClassRow> = (0..k)
        .map(|i| ClassRow {
            idx: i,
            label: order[i].clone(),
            sup_a: a.support[i],
            sup_b: b.support[i],
            p_a: a.precision[i],
            p_b: b.precision[i],
            r_a: a.recall[i],
            r_b: b.recall[i],
            f_a: a.fscore[i],
            f_b: b.fscore[i],
        })
        .collect();
    let by = |x: f64, y: f64| y.partial_cmp(&x).unwrap_or(Ordering::Equal);
    match sort_by.as_str() {
        "f1_delta" => rows.sort_by(|x, y| by(x.df(), y.df()).then(x.idx.cmp(&y.idx))),
        "regression" => rows.sort_by(|x, y| {
            x.df()
                .partial_cmp(&y.df())
                .unwrap_or(Ordering::Equal)
                .then(x.idx.cmp(&y.idx))
        }),
        "precision_delta" => rows.sort_by(|x, y| by(x.dp(), y.dp()).then(x.idx.cmp(&y.idx))),
        "recall_delta" => rows.sort_by(|x, y| by(x.dr(), y.dr()).then(x.idx.cmp(&y.idx))),
        "support" => rows.sort_by(|x, y| by(x.sup_a, y.sup_a).then(x.idx.cmp(&y.idx))),
        _ => {}
    }

    let delta: Vec<Vec<f64>> = (0..k)
        .map(|i| (0..k).map(|j| b.m[i][j] - a.m[i][j]).collect())
        .collect();

    // ---- overall metric triples ----
    let overall: Vec<(&str, f64, f64, bool)> = vec![
        ("Accuracy", a.accuracy, b.accuracy, true),
        (
            "Balanced accuracy",
            a.balanced_accuracy,
            b.balanced_accuracy,
            true,
        ),
        ("Macro precision", a.macro_p, b.macro_p, true),
        ("Macro recall", a.macro_r, b.macro_r, true),
        ("Macro", a.macro_f, b.macro_f, true),
        ("Weighted precision", a.weighted_p, b.weighted_p, true),
        ("Weighted recall", a.weighted_r, b.weighted_r, true),
        ("Weighted", a.weighted_f, b.weighted_f, true),
        ("Micro", a.accuracy, b.accuracy, true),
        ("Cohen's kappa", a.kappa, b.kappa, false),
        ("Matthews correlation", a.mcc, b.mcc, false),
    ];
    // "Macro"/"Weighted"/"Micro" get the F-score name appended.
    let label_of = |name: &str| -> String {
        match name {
            "Macro" | "Weighted" | "Micro" => format!("{name} {fname}"),
            other => other.to_string(),
        }
    };

    // ---- biggest movers ----
    let mut improved: Vec<&ClassRow> = rows.iter().filter(|r| r.df() > 1e-12).collect();
    improved.sort_by(|x, y| by(x.df(), y.df()).then(x.idx.cmp(&y.idx)));
    let mut regressed: Vec<&ClassRow> = rows.iter().filter(|r| r.df() < -1e-12).collect();
    regressed.sort_by(|x, y| {
        x.df()
            .partial_cmp(&y.df())
            .unwrap_or(Ordering::Equal)
            .then(x.idx.cmp(&y.idx))
    });
    let mut cells: Vec<(usize, usize, f64)> = (0..k)
        .flat_map(|i| (0..k).map(move |j| (i, j)))
        .map(|(i, j)| (i, j, delta[i][j]))
        .collect();
    let up = cells
        .iter()
        .filter(|(_, _, d)| *d > 0.0)
        .max_by(|x, y| x.2.partial_cmp(&y.2).unwrap_or(Ordering::Equal))
        .copied();
    cells.sort_by(|x, y| x.2.partial_cmp(&y.2).unwrap_or(Ordering::Equal));
    let down = cells.iter().find(|(_, _, d)| *d < 0.0).copied();

    let macro_delta = b.macro_f - a.macro_f;
    let acc_delta = b.accuracy - a.accuracy;
    let verdict = if macro_delta.abs() < 1e-12 && acc_delta.abs() < 1e-12 {
        format!("{name_a} and {name_b} score identically")
    } else if macro_delta > 1e-12 && acc_delta >= -1e-12 {
        format!(
            "{name_b} is ahead — macro {fname} {}, accuracy {}",
            f.rate_delta(macro_delta),
            f.rate_delta(acc_delta)
        )
    } else if macro_delta < -1e-12 && acc_delta <= 1e-12 {
        format!(
            "{name_a} is ahead — macro {fname} {}, accuracy {}",
            f.rate_delta(macro_delta),
            f.rate_delta(acc_delta)
        )
    } else {
        format!(
            "mixed — macro {fname} {}, accuracy {}",
            f.rate_delta(macro_delta),
            f.rate_delta(acc_delta)
        )
    };

    let test = if significance {
        Some(accuracy_test(&a, &b, level))
    } else {
        None
    };

    // Footnotes for zero-denominator rates.
    let mut notes: Vec<String> = Vec::new();
    for (mm, who) in [(&a, &name_a), (&b, &name_b)] {
        if !mm.undefined_precision.is_empty() {
            notes.push(format!(
                "In {who}, precision is undefined for {} (nothing was predicted into {}); reported as 0.",
                mm.undefined_precision
                    .iter()
                    .map(|&i| format!("`{}`", order[i]))
                    .collect::<Vec<_>>()
                    .join(", "),
                if mm.undefined_precision.len() == 1 {
                    "that class"
                } else {
                    "those classes"
                }
            ));
        }
        if !mm.undefined_recall.is_empty() {
            notes.push(format!(
                "In {who}, recall is undefined for {} (no observations of {}); reported as 0.",
                mm.undefined_recall
                    .iter()
                    .map(|&i| format!("`{}`", order[i]))
                    .collect::<Vec<_>>()
                    .join(", "),
                if mm.undefined_recall.len() == 1 {
                    "that class"
                } else {
                    "those classes"
                }
            ));
        }
    }

    match format.as_str() {
        "json" => Ok(render_json(
            &name_a, &name_b, &order, beta, &a, &b, &rows, &delta, matrix_delta, &test, &notes,
            decimals, &verdict,
        )),
        "csv" => Ok(render_csv(
            &name_a,
            &name_b,
            &order,
            &fname,
            &overall,
            label_of,
            &rows,
            &delta,
            matrix_delta,
            &a,
            &b,
            &test,
            &f,
        )),
        "text" => Ok(render_body(
            false, &name_a, &name_b, &order, &fname, &overall, label_of, &rows, &delta,
            matrix_delta, &a, &b, &improved, &regressed, up, down, &test, &notes, &verdict, &f,
        )),
        _ => Ok(render_body(
            true, &name_a, &name_b, &order, &fname, &overall, label_of, &rows, &delta,
            matrix_delta, &a, &b, &improved, &regressed, up, down, &test, &notes, &verdict, &f,
        )),
    }
}

// ---------------------------------------------------------------------------
// renderers
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn render_body(
    md: bool,
    name_a: &str,
    name_b: &str,
    order: &[String],
    fname: &str,
    overall: &[(&str, f64, f64, bool)],
    label_of: impl Fn(&str) -> String,
    rows: &[ClassRow],
    delta: &[Vec<f64>],
    matrix_delta: bool,
    a: &Metrics,
    b: &Metrics,
    improved: &[&ClassRow],
    regressed: &[&ClassRow],
    up: Option<(usize, usize, f64)>,
    down: Option<(usize, usize, f64)>,
    test: &Option<AccTest>,
    notes: &[String],
    verdict: &str,
    f: &Fmt,
) -> String {
    let k = order.len();
    let mut out = String::new();
    let table = |h: &[String], r: &[Vec<String>], out: &mut String| {
        if md {
            md_table(h, r, out);
        } else {
            text_table(h, r, out);
        }
    };
    let heading = |level: u8, text: &str, out: &mut String| {
        if md {
            let _ = writeln!(out, "{} {text}\n", "#".repeat(level as usize));
        } else if level == 1 {
            let _ = writeln!(out, "{text}\n{}\n", "=".repeat(text.chars().count()));
        } else {
            let _ = writeln!(out, "{text}\n{}\n", "-".repeat(text.chars().count()));
        }
    };
    let bullet = |text: &str, out: &mut String| {
        let _ = writeln!(out, "{}{text}", if md { "- " } else { "  " });
    };

    heading(1, "Confusion matrix comparison", &mut out);
    bullet(
        &format!("Models: {name_a} (A) vs {name_b} (B)"),
        &mut out,
    );
    bullet(
        &format!("Classes: {k} — {}", order.join(", ")),
        &mut out,
    );
    bullet(
        &format!(
            "Observations: A = {}, B = {} ({} correct vs {})",
            f.count(a.total),
            f.count(b.total),
            f.count(a.correct),
            f.count(b.correct)
        ),
        &mut out,
    );
    bullet(&format!("Verdict: {verdict}"), &mut out);
    out.push('\n');

    heading(2, "Overall", &mut out);
    let h = vec![
        "Metric".to_string(),
        name_a.to_string(),
        name_b.to_string(),
        "Delta (B - A)".to_string(),
    ];
    let mut r: Vec<Vec<String>> = overall
        .iter()
        .map(|(name, va, vb, is_rate)| {
            let (sa, sb, sd) = if *is_rate {
                (f.rate(*va), f.rate(*vb), f.rate_delta(vb - va))
            } else {
                (f.num(*va), f.num(*vb), f.num_delta(vb - va))
            };
            vec![label_of(name), sa, sb, sd]
        })
        .collect();
    r.push(vec![
        "Observations".to_string(),
        f.count(a.total),
        f.count(b.total),
        f.count_delta(b.total - a.total),
    ]);
    r.push(vec![
        "Correct".to_string(),
        f.count(a.correct),
        f.count(b.correct),
        f.count_delta(b.correct - a.correct),
    ]);
    table(&h, &r, &mut out);
    out.push('\n');

    heading(2, "Per-class deltas", &mut out);
    let h = vec![
        "Class".to_string(),
        "Support A".to_string(),
        "Support B".to_string(),
        "Precision A".to_string(),
        "Precision B".to_string(),
        "Delta P".to_string(),
        "Recall A".to_string(),
        "Recall B".to_string(),
        "Delta R".to_string(),
        format!("{fname} A"),
        format!("{fname} B"),
        format!("Delta {fname}"),
    ];
    let r: Vec<Vec<String>> = rows
        .iter()
        .map(|c| {
            vec![
                c.label.clone(),
                f.count(c.sup_a),
                f.count(c.sup_b),
                f.rate(c.p_a),
                f.rate(c.p_b),
                f.rate_delta(c.dp()),
                f.rate(c.r_a),
                f.rate(c.r_b),
                f.rate_delta(c.dr()),
                f.rate(c.f_a),
                f.rate(c.f_b),
                f.rate_delta(c.df()),
            ]
        })
        .collect();
    table(&h, &r, &mut out);
    out.push('\n');

    if k == 2 {
        let pos = 1usize;
        heading(
            2,
            &format!("Binary summary (positive class: {})", order[pos]),
            &mut out,
        );
        let h = vec![
            "Metric".to_string(),
            name_a.to_string(),
            name_b.to_string(),
            "Delta (B - A)".to_string(),
        ];
        let rate_row = |n: &str, va: f64, vb: f64| {
            vec![
                n.to_string(),
                f.rate(va),
                f.rate(vb),
                f.rate_delta(vb - va),
            ]
        };
        let count_row = |n: &str, va: f64, vb: f64| {
            vec![
                n.to_string(),
                f.count(va),
                f.count(vb),
                f.count_delta(vb - va),
            ]
        };
        let r = vec![
            rate_row("Precision", a.precision[pos], b.precision[pos]),
            rate_row(
                "Recall (sensitivity)",
                a.recall[pos],
                b.recall[pos],
            ),
            rate_row("Specificity", a.specificity[pos], b.specificity[pos]),
            rate_row(fname, a.fscore[pos], b.fscore[pos]),
            count_row("True positives", a.tp[pos], b.tp[pos]),
            count_row("False positives", a.fp[pos], b.fp[pos]),
            count_row("False negatives", a.fneg[pos], b.fneg[pos]),
            count_row("True negatives", a.tn[pos], b.tn[pos]),
        ];
        table(&h, &r, &mut out);
        let _ = writeln!(
            out,
            "\nThe second class is the positive one; reorder the class list to flip it.\n"
        );
    }

    if matrix_delta {
        heading(2, &format!("Entrywise delta ({name_b} - {name_a})"), &mut out);
        let _ = writeln!(
            out,
            "Rows are actual classes, columns are predicted classes. The diagonal is\ncorrect predictions gained or lost.\n"
        );
        let mut h = vec!["Actual \\ Predicted".to_string()];
        h.extend(order.iter().cloned());
        let r: Vec<Vec<String>> = (0..k)
            .map(|i| {
                let mut row = vec![order[i].clone()];
                row.extend((0..k).map(|j| f.count_delta(delta[i][j])));
                row
            })
            .collect();
        table(&h, &r, &mut out);
        out.push('\n');
    }

    heading(2, "Biggest movers", &mut out);
    match improved.first() {
        Some(c) => bullet(
            &format!(
                "Most improved class: {} — {fname} {} (precision {}, recall {})",
                c.label,
                f.rate_delta(c.df()),
                f.rate_delta(c.dp()),
                f.rate_delta(c.dr())
            ),
            &mut out,
        ),
        None => bullet("Most improved class: none — no class gained", &mut out),
    }
    match regressed.first() {
        Some(c) => bullet(
            &format!(
                "Most regressed class: {} — {fname} {} (precision {}, recall {})",
                c.label,
                f.rate_delta(c.df()),
                f.rate_delta(c.dp()),
                f.rate_delta(c.dr())
            ),
            &mut out,
        ),
        None => bullet(
            "Most regressed class: none — every class held or improved",
            &mut out,
        ),
    }
    match up {
        Some((i, j, d)) => bullet(
            &format!(
                "Largest cell increase: actual {} predicted as {} — {}",
                order[i],
                order[j],
                f.count_delta(d)
            ),
            &mut out,
        ),
        None => bullet("Largest cell increase: none — no cell grew", &mut out),
    }
    match down {
        Some((i, j, d)) => bullet(
            &format!(
                "Largest cell decrease: actual {} predicted as {} — {}",
                order[i],
                order[j],
                f.count_delta(d)
            ),
            &mut out,
        ),
        None => bullet("Largest cell decrease: none — no cell shrank", &mut out),
    }
    out.push('\n');

    if let Some(t) = test {
        heading(2, "Accuracy difference", &mut out);
        let h = vec!["Quantity".to_string(), "Value".to_string()];
        let mut r = vec![
            vec![format!("Accuracy {name_a}"), f.rate(a.accuracy)],
            vec![format!("Accuracy {name_b}"), f.rate(b.accuracy)],
            vec!["Difference (B - A)".to_string(), f.rate_delta(t.diff)],
            vec![
                format!("{}% confidence interval", t.level),
                format!("{} to {}", f.rate_delta(t.ci_lower), f.rate_delta(t.ci_upper)),
            ],
        ];
        match (t.z, t.p) {
            (Some(z), Some(p)) => {
                r.push(vec!["z".to_string(), f.num(z)]);
                r.push(vec!["p-value (two-sided)".to_string(), f.pval(p)]);
                r.push(vec![
                    "Verdict".to_string(),
                    if t.significant {
                        format!("significant at the {}% level", 100 - t.level)
                    } else {
                        format!("not significant at the {}% level", 100 - t.level)
                    },
                ]);
            }
            _ => {
                r.push(vec![
                    "z".to_string(),
                    "undefined (both models scored every item the same way)".to_string(),
                ]);
            }
        }
        table(&h, &r, &mut out);
        let _ = writeln!(
            out,
            "\nThis is an unpaired two-proportion test: it assumes the two matrices come from\nINDEPENDENT test sets. If both models were scored on the SAME items it is\nconservative, and the correct test is a paired McNemar test — which cannot be\ncomputed from two confusion matrices, because it needs the per-item agreement\ncounts that a matrix has already summed away.\n"
        );
    }

    if !notes.is_empty() {
        heading(2, "Notes", &mut out);
        for n in notes {
            bullet(n, &mut out);
        }
        out.push('\n');
    }

    out.trim_end().to_string()
}

#[allow(clippy::too_many_arguments)]
fn render_csv(
    name_a: &str,
    name_b: &str,
    order: &[String],
    fname: &str,
    overall: &[(&str, f64, f64, bool)],
    label_of: impl Fn(&str) -> String,
    rows: &[ClassRow],
    delta: &[Vec<f64>],
    matrix_delta: bool,
    a: &Metrics,
    b: &Metrics,
    test: &Option<AccTest>,
    f: &Fmt,
) -> String {
    let k = order.len();
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{}",
        csv_row(&[
            "section".into(),
            "metric".into(),
            name_a.into(),
            name_b.into(),
            "delta".into()
        ])
    );
    for (name, va, vb, is_rate) in overall {
        let (sa, sb, sd) = if *is_rate {
            (f.rate(*va), f.rate(*vb), f.rate_delta(vb - va))
        } else {
            (f.num(*va), f.num(*vb), f.num_delta(vb - va))
        };
        let _ = writeln!(
            out,
            "{}",
            csv_row(&["overall".into(), label_of(name), sa, sb, sd])
        );
    }
    let _ = writeln!(
        out,
        "{}",
        csv_row(&[
            "overall".into(),
            "observations".into(),
            f.count(a.total),
            f.count(b.total),
            f.count_delta(b.total - a.total)
        ])
    );
    let _ = writeln!(
        out,
        "{}",
        csv_row(&[
            "overall".into(),
            "correct".into(),
            f.count(a.correct),
            f.count(b.correct),
            f.count_delta(b.correct - a.correct)
        ])
    );
    if let Some(t) = test {
        let _ = writeln!(
            out,
            "{}",
            csv_row(&[
                "test".into(),
                "accuracy_difference".into(),
                f.rate(a.accuracy),
                f.rate(b.accuracy),
                f.rate_delta(t.diff)
            ])
        );
        let _ = writeln!(
            out,
            "{}",
            csv_row(&[
                "test".into(),
                format!("ci_{}_lower", t.level),
                String::new(),
                String::new(),
                f.rate_delta(t.ci_lower)
            ])
        );
        let _ = writeln!(
            out,
            "{}",
            csv_row(&[
                "test".into(),
                format!("ci_{}_upper", t.level),
                String::new(),
                String::new(),
                f.rate_delta(t.ci_upper)
            ])
        );
        if let (Some(z), Some(p)) = (t.z, t.p) {
            let _ = writeln!(
                out,
                "{}",
                csv_row(&["test".into(), "z".into(), String::new(), String::new(), f.num(z)])
            );
            let _ = writeln!(
                out,
                "{}",
                csv_row(&[
                    "test".into(),
                    "p_value".into(),
                    String::new(),
                    String::new(),
                    f.pval(p)
                ])
            );
        }
    }

    out.push('\n');
    let fl = fname.to_ascii_lowercase();
    let _ = writeln!(
        out,
        "{}",
        csv_row(&[
            "class".into(),
            "support_a".into(),
            "support_b".into(),
            "precision_a".into(),
            "precision_b".into(),
            "precision_delta".into(),
            "recall_a".into(),
            "recall_b".into(),
            "recall_delta".into(),
            format!("{fl}_a"),
            format!("{fl}_b"),
            format!("{fl}_delta"),
        ])
    );
    for c in rows {
        let _ = writeln!(
            out,
            "{}",
            csv_row(&[
                c.label.clone(),
                f.count(c.sup_a),
                f.count(c.sup_b),
                f.rate(c.p_a),
                f.rate(c.p_b),
                f.rate_delta(c.dp()),
                f.rate(c.r_a),
                f.rate(c.r_b),
                f.rate_delta(c.dr()),
                f.rate(c.f_a),
                f.rate(c.f_b),
                f.rate_delta(c.df()),
            ])
        );
    }

    if matrix_delta {
        out.push('\n');
        let _ = writeln!(
            out,
            "{}",
            csv_row(&[
                "actual".into(),
                "predicted".into(),
                "count_a".into(),
                "count_b".into(),
                "delta".into()
            ])
        );
        for i in 0..k {
            for j in 0..k {
                let _ = writeln!(
                    out,
                    "{}",
                    csv_row(&[
                        order[i].clone(),
                        order[j].clone(),
                        f.count(a.m[i][j]),
                        f.count(b.m[i][j]),
                        f.count_delta(delta[i][j]),
                    ])
                );
            }
        }
    }
    out.trim_end().to_string()
}

#[allow(clippy::too_many_arguments)]
fn render_json(
    name_a: &str,
    name_b: &str,
    order: &[String],
    beta: f64,
    a: &Metrics,
    b: &Metrics,
    rows: &[ClassRow],
    delta: &[Vec<f64>],
    matrix_delta: bool,
    test: &Option<AccTest>,
    notes: &[String],
    d: usize,
    verdict: &str,
) -> String {
    let k = order.len();
    let n = |v: f64| jnum(v, d);
    let c = |v: f64| format!("{}", v.round() as i64);
    let arr = |v: &[String]| {
        v.iter()
            .map(|s| format!("\"{}\"", json_escape(s)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let grid = |m: &[Vec<f64>]| {
        m.iter()
            .map(|r| {
                format!(
                    "[{}]",
                    r.iter().map(|&v| c(v)).collect::<Vec<_>>().join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join(",\n    ")
    };
    let triple = |name: &str, va: f64, vb: f64| {
        format!(
            "    \"{name}\": {{ \"a\": {}, \"b\": {}, \"delta\": {} }}",
            n(va),
            n(vb),
            n(vb - va)
        )
    };

    let mut out = String::new();
    let _ = writeln!(out, "{{");
    let _ = writeln!(out, "  \"name_a\": \"{}\",", json_escape(name_a));
    let _ = writeln!(out, "  \"name_b\": \"{}\",", json_escape(name_b));
    let _ = writeln!(out, "  \"classes\": [{}],", arr(order));
    let _ = writeln!(out, "  \"beta\": {},", n(beta));
    let _ = writeln!(out, "  \"observations_a\": {},", c(a.total));
    let _ = writeln!(out, "  \"observations_b\": {},", c(b.total));
    let _ = writeln!(out, "  \"correct_a\": {},", c(a.correct));
    let _ = writeln!(out, "  \"correct_b\": {},", c(b.correct));
    let _ = writeln!(out, "  \"verdict\": \"{}\",", json_escape(verdict));
    let _ = writeln!(out, "  \"overall\":\n  {{");
    let parts = vec![
        triple("accuracy", a.accuracy, b.accuracy),
        triple(
            "balanced_accuracy",
            a.balanced_accuracy,
            b.balanced_accuracy,
        ),
        triple("macro_precision", a.macro_p, b.macro_p),
        triple("macro_recall", a.macro_r, b.macro_r),
        triple("macro_fscore", a.macro_f, b.macro_f),
        triple("weighted_precision", a.weighted_p, b.weighted_p),
        triple("weighted_recall", a.weighted_r, b.weighted_r),
        triple("weighted_fscore", a.weighted_f, b.weighted_f),
        triple("micro_fscore", a.accuracy, b.accuracy),
        triple("cohens_kappa", a.kappa, b.kappa),
        triple("matthews_correlation", a.mcc, b.mcc),
    ];
    let _ = writeln!(out, "{}", parts.join(",\n"));
    let _ = writeln!(out, "  }},");

    let _ = writeln!(out, "  \"per_class\": [");
    let cls: Vec<String> = rows
        .iter()
        .map(|r| {
            format!(
                "    {{ \"class\": \"{}\", \"support_a\": {}, \"support_b\": {}, \"precision_a\": {}, \"precision_b\": {}, \"precision_delta\": {}, \"recall_a\": {}, \"recall_b\": {}, \"recall_delta\": {}, \"fscore_a\": {}, \"fscore_b\": {}, \"fscore_delta\": {} }}",
                json_escape(&r.label),
                c(r.sup_a),
                c(r.sup_b),
                n(r.p_a),
                n(r.p_b),
                n(r.dp()),
                n(r.r_a),
                n(r.r_b),
                n(r.dr()),
                n(r.f_a),
                n(r.f_b),
                n(r.df())
            )
        })
        .collect();
    let _ = writeln!(out, "{}", cls.join(",\n"));
    let _ = writeln!(out, "  ],");

    if matrix_delta {
        let _ = writeln!(out, "  \"matrix_a\": [\n    {}\n  ],", grid(&a.m));
        let _ = writeln!(out, "  \"matrix_b\": [\n    {}\n  ],", grid(&b.m));
        let _ = writeln!(out, "  \"matrix_delta\": [\n    {}\n  ],", grid(delta));
    }

    if k == 2 {
        let p = 1usize;
        let _ = writeln!(out, "  \"binary\":\n  {{");
        let _ = writeln!(
            out,
            "    \"positive_class\": \"{}\",",
            json_escape(&order[p])
        );
        let bp = vec![
            triple("precision", a.precision[p], b.precision[p]),
            triple("recall", a.recall[p], b.recall[p]),
            triple("specificity", a.specificity[p], b.specificity[p]),
            triple("fscore", a.fscore[p], b.fscore[p]),
        ];
        let _ = writeln!(out, "{}", bp.join(",\n"));
        let _ = writeln!(
            out,
            "    ,\"counts\": {{ \"tp_a\": {}, \"tp_b\": {}, \"fp_a\": {}, \"fp_b\": {}, \"fn_a\": {}, \"fn_b\": {}, \"tn_a\": {}, \"tn_b\": {} }}",
            c(a.tp[p]),
            c(b.tp[p]),
            c(a.fp[p]),
            c(b.fp[p]),
            c(a.fneg[p]),
            c(b.fneg[p]),
            c(a.tn[p]),
            c(b.tn[p])
        );
        let _ = writeln!(out, "  }},");
    }

    match test {
        Some(t) => {
            let _ = writeln!(out, "  \"accuracy_test\":\n  {{");
            let _ = writeln!(out, "    \"kind\": \"unpaired two-proportion z-test\",");
            let _ = writeln!(out, "    \"difference\": {},", n(t.diff));
            let _ = writeln!(out, "    \"confidence_level\": {},", t.level);
            let _ = writeln!(out, "    \"ci_lower\": {},", n(t.ci_lower));
            let _ = writeln!(out, "    \"ci_upper\": {},", n(t.ci_upper));
            match (t.z, t.p) {
                (Some(z), Some(p)) => {
                    let _ = writeln!(out, "    \"z\": {},", n(z));
                    let _ = writeln!(out, "    \"p_value\": {},", jnum(p, d.max(4)));
                    let _ = writeln!(out, "    \"significant\": {}", t.significant);
                }
                _ => {
                    let _ = writeln!(out, "    \"z\": null,");
                    let _ = writeln!(out, "    \"p_value\": null,");
                    let _ = writeln!(out, "    \"significant\": false");
                }
            }
            let _ = writeln!(out, "  }},");
        }
        None => {
            let _ = writeln!(out, "  \"accuracy_test\": null,");
        }
    }
    let _ = writeln!(out, "  \"notes\": [{}]", arr(notes));
    let _ = write!(out, "}}");
    out
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// `run` with the descriptor defaults, so a test only states what it changes.
    #[allow(clippy::too_many_arguments)]
    fn go(a: &str, b: &str) -> String {
        run(
            a, b, "", "", "", "auto", "actual_rows", "auto", "auto", 1.0, "class", true, "95",
            true, 4.0, false, "markdown",
        )
        .unwrap()
    }

    fn json(a: &str, b: &str) -> String {
        run(
            a, b, "", "", "", "auto", "actual_rows", "auto", "auto", 1.0, "class", false, "95",
            false, 4.0, false, "json",
        )
        .unwrap()
    }

    const A2: &str = "40,10\n5,45";
    const B2: &str = "45,5\n8,42";

    #[test]
    fn happy_path_reports_overall_and_per_class_deltas() {
        let out = go(A2, B2);
        assert!(out.starts_with("# Confusion matrix comparison"), "{out}");
        // A: 85/100 correct, B: 87/100.
        assert!(out.contains("| Accuracy | 0.8500 | 0.8700 | +0.0200 |"), "{out}");
        assert!(out.contains("Observations: A = 100, B = 100 (85 correct vs 87)"), "{out}");
        // Class 0 recall 40/50 -> 45/50.
        assert!(out.contains("| 0 | 50 | 50 | 0.8889 | 0.8491 | -0.0398 | 0.8000 | 0.9000 | +0.1000 |"), "{out}");
        assert!(out.contains("## Per-class deltas"), "{out}");
        assert!(out.contains("## Entrywise delta (Model B - Model A)"), "{out}");
        assert!(out.contains("| 0 | +5 | -5 |"), "{out}");
        assert!(out.contains("## Biggest movers"), "{out}");
        assert!(out.contains("## Accuracy difference"), "{out}");
    }

    #[test]
    fn identical_matrices_report_no_movement() {
        let out = go(A2, A2);
        assert!(out.contains("Verdict: Model A and Model B score identically"), "{out}");
        assert!(out.contains("Most improved class: none — no class gained"), "{out}");
        assert!(
            out.contains("Most regressed class: none — every class held or improved"),
            "{out}"
        );
        assert!(out.contains("Largest cell increase: none — no cell grew"), "{out}");
        assert!(out.contains("| Accuracy | 0.8500 | 0.8500 | 0.0000 |"), "{out}");
    }

    #[test]
    fn binary_summary_appears_for_two_classes_only() {
        let two = go(A2, B2);
        assert!(two.contains("## Binary summary (positive class: 1)"), "{two}");
        assert!(two.contains("| True positives | 45 | 42 | -3 |"), "{two}");
        let three = go("8,1,1\n1,8,1\n1,1,8", "9,1,0\n1,8,1\n0,2,8");
        assert!(!three.contains("Binary summary"), "{three}");
    }

    #[test]
    fn header_row_and_label_column_are_read_as_class_names() {
        let a = "actual,cat,dog\ncat,8,2\ndog,3,7";
        let b = "actual,cat,dog\ncat,9,1\ndog,2,8";
        let out = go(a, b);
        assert!(out.contains("Classes: 2 — cat, dog"), "{out}");
        assert!(out.contains("| cat | 10 | 10 |"), "{out}");
        assert!(out.contains("positive class: dog"), "{out}");
    }

    #[test]
    fn matrix_b_is_reordered_onto_matrix_a_class_order() {
        // B lists dog first; the counts must follow the label, not the position.
        let a = "cat,8,2\ndog,3,7";
        let b = "dog,8,2\ncat,1,9";
        let out = go(a, b);
        assert!(out.contains("Classes: 2 — cat, dog"), "{out}");
        // cat row in B: cat->cat 9, cat->dog 1.
        assert!(out.contains("| cat | +1 | -1 |"), "{out}");
        assert!(out.contains("| dog | -1 | +1 |"), "{out}");
    }

    #[test]
    fn label_pairs_are_tallied_into_a_matrix() {
        let a = "cat,cat\ncat,dog\ndog,dog\ndog,dog";
        let b = "cat,cat\ncat,cat\ndog,dog\ndog,cat";
        let out = go(a, b);
        assert!(out.contains("Classes: 2 — cat, dog"), "{out}");
        assert!(out.contains("Observations: A = 4, B = 4 (3 correct vs 3)"), "{out}");
        assert!(out.contains("| Accuracy | 0.7500 | 0.7500 | 0.0000 |"), "{out}");
    }

    #[test]
    fn actual_predicted_count_triples_are_tallied() {
        let a = "actual,predicted,count\ncat,cat,8\ncat,dog,2\ndog,cat,3\ndog,dog,7";
        let b = "actual,predicted,count\ncat,cat,9\ncat,dog,1\ndog,cat,2\ndog,dog,8";
        let out = run(
            a, b, "", "", "", "table", "actual_rows", "auto", "auto", 1.0, "class", false, "95",
            true, 4.0, false, "markdown",
        )
        .unwrap();
        assert!(out.contains("Observations: A = 20, B = 20 (15 correct vs 17)"), "{out}");
        assert!(out.contains("| cat | +1 | -1 |"), "{out}");
    }

    #[test]
    fn orientation_transposes_a_predicted_major_matrix() {
        let out = run(
            "40,5\n10,45", "45,8\n5,42", "", "", "", "matrix", "actual_columns", "auto", "no",
            1.0, "class", false, "95", false, 4.0, false, "markdown",
        )
        .unwrap();
        // Transposing gives the same matrices as the actual-rows happy path.
        assert!(out.contains("| Accuracy | 0.8500 | 0.8700 | +0.0200 |"), "{out}");
        assert!(out.contains("| 0 | 50 | 50 | 0.8889 | 0.8491 |"), "{out}");
    }

    #[test]
    fn explicit_labels_set_the_class_order_and_the_positive_class() {
        let out = run(
            A2, B2, "no,yes", "", "", "auto", "actual_rows", "auto", "auto", 1.0, "class", false,
            "95", false, 4.0, false, "markdown",
        )
        .unwrap();
        assert!(out.contains("Classes: 2 — no, yes"), "{out}");
        assert!(out.contains("positive class: yes"), "{out}");
    }

    #[test]
    fn model_names_appear_in_every_heading() {
        let out = run(
            A2, B2, "", "Baseline", "Candidate", "auto", "actual_rows", "auto", "auto", 1.0,
            "class", false, "95", true, 4.0, false, "markdown",
        )
        .unwrap();
        assert!(out.contains("Models: Baseline (A) vs Candidate (B)"), "{out}");
        assert!(out.contains("| Metric | Baseline | Candidate | Delta (B - A) |"), "{out}");
        assert!(out.contains("## Entrywise delta (Candidate - Baseline)"), "{out}");
        assert!(out.contains("Verdict: Candidate is ahead"), "{out}");
    }

    #[test]
    fn sort_by_regression_puts_the_worst_class_first() {
        let a = "8,1,1\n1,8,1\n1,1,8";
        let b = "10,0,0\n1,8,1\n3,3,4";
        let sorted = run(
            a, b, "", "", "", "auto", "actual_rows", "auto", "auto", 1.0, "regression", false,
            "95", false, 4.0, false, "text",
        )
        .unwrap();
        let body = sorted.split("Per-class deltas").nth(1).unwrap();
        let first = body
            .lines()
            .find(|l| l.starts_with('0') || l.starts_with('1') || l.starts_with('2'))
            .unwrap();
        assert!(first.starts_with('2'), "worst class first, got `{first}`");
        // The default order is by class.
        let plain = run(
            a, b, "", "", "", "auto", "actual_rows", "auto", "auto", 1.0, "class", false, "95",
            false, 4.0, false, "text",
        )
        .unwrap();
        let body = plain.split("Per-class deltas").nth(1).unwrap();
        let first = body
            .lines()
            .find(|l| l.starts_with('0') || l.starts_with('1') || l.starts_with('2'))
            .unwrap();
        assert!(first.starts_with('0'), "class order, got `{first}`");
    }

    #[test]
    fn fbeta_renames_the_column_and_reweights() {
        let out = run(
            A2, B2, "", "", "", "auto", "actual_rows", "auto", "auto", 0.5, "class", false, "95",
            false, 4.0, false, "markdown",
        )
        .unwrap();
        assert!(out.contains("| Macro F0.5 |"), "{out}");
        assert!(out.contains("Delta F0.5"), "{out}");
        assert!(!out.contains("Macro F1 "), "{out}");
    }

    #[test]
    fn percent_formats_rates_but_not_kappa() {
        let out = run(
            A2, B2, "", "", "", "auto", "actual_rows", "auto", "auto", 1.0, "class", false, "95",
            false, 2.0, true, "markdown",
        )
        .unwrap();
        assert!(out.contains("| Accuracy | 85.00% | 87.00% | +2.00% |"), "{out}");
        assert!(out.contains("| Cohen's kappa | 0.70 | 0.74 | +0.04 |"), "{out}");
    }

    #[test]
    fn undefined_rates_are_reported_as_zero_with_a_note() {
        // Nothing is ever predicted as class 2 in A.
        let a = "5,5,0\n2,8,0\n1,1,0";
        let b = "5,5,0\n2,8,0\n1,0,1";
        let out = go(a, b);
        assert!(
            out.contains("precision is undefined for `2` (nothing was predicted into that class); reported as 0."),
            "{out}"
        );
        assert!(out.contains("## Notes"), "{out}");
    }

    #[test]
    fn accuracy_test_flags_a_significant_improvement() {
        // 500/1000 vs 700/1000 is unmissable.
        let a = "250,250\n250,250";
        let b = "350,150\n150,350";
        let out = go(a, b);
        assert!(out.contains("| p-value (two-sided) | 0.0000 |"), "{out}");
        assert!(out.contains("| Verdict | significant at the 5% level |"), "{out}");
        assert!(out.contains("paired McNemar test"), "{out}");
    }

    #[test]
    fn accuracy_test_can_be_switched_off() {
        let out = run(
            A2, B2, "", "", "", "auto", "actual_rows", "auto", "auto", 1.0, "class", false, "95",
            false, 4.0, false, "markdown",
        )
        .unwrap();
        assert!(!out.contains("Accuracy difference"), "{out}");
        assert!(!out.contains("Entrywise delta"), "{out}");
    }

    #[test]
    fn confidence_level_widens_the_interval() {
        let ninety = run(
            A2, B2, "", "", "", "auto", "actual_rows", "auto", "auto", 1.0, "class", true, "90",
            false, 4.0, false, "markdown",
        )
        .unwrap();
        let ninetynine = run(
            A2, B2, "", "", "", "auto", "actual_rows", "auto", "auto", 1.0, "class", true, "99",
            false, 4.0, false, "markdown",
        )
        .unwrap();
        assert!(ninety.contains("| 90% confidence interval |"), "{ninety}");
        assert!(ninetynine.contains("| 99% confidence interval |"), "{ninetynine}");
        assert!(ninetynine.contains("not significant at the 1% level"), "{ninetynine}");
    }

    #[test]
    fn text_format_aligns_without_pipes() {
        let out = run(
            A2, B2, "", "", "", "auto", "actual_rows", "auto", "auto", 1.0, "class", false, "95",
            false, 4.0, false, "text",
        )
        .unwrap();
        assert!(out.starts_with("Confusion matrix comparison\n==========================="), "{out}");
        assert!(!out.contains('|'), "{out}");
        assert!(
            out.contains("Accuracy               0.8500   0.8700        +0.0200"),
            "{out}"
        );
    }

    #[test]
    fn csv_format_emits_three_blocks() {
        let out = run(
            A2, B2, "", "", "", "auto", "actual_rows", "auto", "auto", 1.0, "class", true, "95",
            true, 4.0, false, "csv",
        )
        .unwrap();
        assert!(out.starts_with("section,metric,Model A,Model B,delta\n"), "{out}");
        assert!(out.contains("overall,Accuracy,0.8500,0.8700,+0.0200\n"), "{out}");
        assert!(out.contains("\nclass,support_a,support_b,precision_a,"), "{out}");
        assert!(out.contains("\nactual,predicted,count_a,count_b,delta\n"), "{out}");
        assert!(out.contains("\n0,1,10,5,-5\n"), "{out}");
        assert!(out.contains("test,p_value,,,"), "{out}");
    }

    #[test]
    fn json_format_is_parseable_and_carries_every_triple() {
        let out = json(A2, B2);
        assert!(out.starts_with("{\n  \"name_a\": \"Model A\","), "{out}");
        assert!(out.contains("\"accuracy\": { \"a\": 0.8500, \"b\": 0.8700, \"delta\": 0.0200 }"), "{out}");
        assert!(out.contains("\"classes\": [\"0\", \"1\"]"), "{out}");
        assert!(out.contains("\"fscore_delta\""), "{out}");
        assert!(out.contains("\"accuracy_test\": null"), "{out}");
        assert!(out.contains("\"binary\""), "{out}");
        assert!(out.ends_with('}'), "{out}");
        // Balanced braces/brackets is a cheap structural check without serde.
        let opens = out.chars().filter(|&c| c == '{' || c == '[').count();
        let closes = out.chars().filter(|&c| c == '}' || c == ']').count();
        assert_eq!(opens, closes, "{out}");
    }

    #[test]
    fn json_includes_the_grids_when_asked() {
        let out = run(
            A2, B2, "", "", "", "auto", "actual_rows", "auto", "auto", 1.0, "class", true, "95",
            true, 4.0, false, "json",
        )
        .unwrap();
        assert!(out.contains("\"matrix_a\": [\n    [40, 10],\n    [5, 45]\n  ],"), "{out}");
        assert!(out.contains("\"matrix_delta\": [\n    [5, -5],\n    [3, -3]\n  ],"), "{out}");
        assert!(out.contains("\"kind\": \"unpaired two-proportion z-test\""), "{out}");
    }

    #[test]
    fn separator_and_header_options_are_honoured() {
        let a = "40\t10\n5\t45";
        let b = "45|5\n8|42";
        let out = run(
            a, b, "", "", "", "matrix", "actual_rows", "auto", "no", 1.0, "class", false, "95",
            false, 4.0, false, "markdown",
        )
        .unwrap();
        assert!(out.contains("| Accuracy | 0.8500 | 0.8700 | +0.0200 |"), "{out}");
        // Space-separated matrices parse too.
        let out = run(
            "40 10\n5 45", "45 5\n8 42", "", "", "", "matrix", "actual_rows", "space", "no", 1.0,
            "class", false, "95", false, 4.0, false, "markdown",
        )
        .unwrap();
        assert!(out.contains("| Accuracy | 0.8500 | 0.8700 | +0.0200 |"), "{out}");
    }

    #[test]
    fn three_class_macro_and_weighted_averages_are_right() {
        let a = "8,1,1\n1,8,1\n1,1,8";
        let out = go(a, a);
        // Symmetric matrix: every class has precision = recall = 0.8.
        assert!(out.contains("| Macro F1 | 0.8000 | 0.8000 | 0.0000 |"), "{out}");
        assert!(out.contains("| Weighted F1 | 0.8000 | 0.8000 | 0.0000 |"), "{out}");
        assert!(out.contains("| Cohen's kappa | 0.7000 | 0.7000 | 0.0000 |"), "{out}");
        assert!(out.contains("| Matthews correlation | 0.7000 | 0.7000 | 0.0000 |"), "{out}");
    }

    #[test]
    fn normal_survival_function_matches_known_values() {
        assert!((norm_sf(0.0) - 0.5).abs() < 1e-12);
        assert!((norm_sf(1.959_963_984_540_054) - 0.025).abs() < 1e-12);
        assert!((norm_sf(1.0) - 0.158_655_253_931_457_05).abs() < 1e-12);
        assert!((norm_sf(-2.0) - 0.977_249_868_051_820_8).abs() < 1e-12);
    }

    // ---- errors ----

    #[test]
    fn non_square_matrix_is_rejected() {
        let e = run(
            "1,2,3\n4,5,6", A2, "", "", "", "auto", "actual_rows", "auto", "no", 1.0, "class",
            true, "95", true, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(e.contains("not square"), "{e}");
    }

    #[test]
    fn mismatched_class_sets_are_rejected() {
        let e = run(
            "cat,8,2\ndog,3,7",
            "fox,8,2\ndog,3,7",
            "",
            "",
            "",
            "auto",
            "actual_rows",
            "auto",
            "auto",
            1.0,
            "class",
            true,
            "95",
            true,
            4.0,
            false,
            "markdown",
        )
        .unwrap_err();
        assert!(e.contains("no class `cat`"), "{e}");
    }

    #[test]
    fn different_class_counts_are_rejected() {
        let e = run(
            "8,1,1\n1,8,1\n1,1,8", A2, "", "", "", "matrix", "actual_rows", "auto", "no", 1.0,
            "class", true, "95", true, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(e.contains("the comparison has 3 classes"), "{e}");
    }

    #[test]
    fn fractional_counts_are_rejected() {
        let e = run(
            "0.8,0.2\n0.1,0.9", A2, "", "", "", "matrix", "actual_rows", "auto", "no", 1.0,
            "class", true, "95", true, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(e.contains("whole numbers"), "{e}");
        assert!(e.contains("normalised"), "{e}");
    }

    #[test]
    fn negative_counts_are_rejected() {
        let e = run(
            "-1,2\n3,4", A2, "", "", "", "matrix", "actual_rows", "auto", "no", 1.0, "class",
            true, "95", true, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(e.contains("cannot be negative"), "{e}");
    }

    #[test]
    fn all_zero_matrix_is_rejected() {
        let e = run(
            "0,0\n0,0", A2, "", "", "", "matrix", "actual_rows", "auto", "no", 1.0, "class", true,
            "95", true, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(e.contains("all zeros"), "{e}");
    }

    #[test]
    fn empty_input_is_rejected() {
        let e = run(
            "   ", A2, "", "", "", "auto", "actual_rows", "auto", "auto", 1.0, "class", true,
            "95", true, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(e.contains("matrix A: no data"), "{e}");
    }

    #[test]
    fn bad_option_values_are_rejected_by_name() {
        let e = run(
            A2, B2, "", "", "", "auto", "actual_rows", "auto", "auto", 1.0, "class", true, "95",
            true, 4.0, false, "yaml",
        )
        .unwrap_err();
        assert!(e.contains("format must be one of markdown, text, csv, json, got `yaml`"), "{e}");
        let e = run(
            A2, B2, "", "", "", "auto", "actual_rows", "auto", "auto", 20.0, "class", true, "95",
            true, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(e.contains("beta must be between 0.1 and 10"), "{e}");
        let e = run(
            A2, B2, "", "", "", "auto", "actual_rows", "auto", "auto", 1.0, "class", true, "95",
            true, 99.0, false, "markdown",
        )
        .unwrap_err();
        assert!(e.contains("decimals must be a whole number between 0 and 10"), "{e}");
    }

    #[test]
    fn identical_model_names_are_rejected() {
        let e = run(
            A2, B2, "", "v2", "v2", "auto", "actual_rows", "auto", "auto", 1.0, "class", true,
            "95", true, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(e.contains("different names"), "{e}");
    }

    #[test]
    fn single_class_matrix_is_rejected() {
        let e = run(
            "5", "5", "", "", "", "matrix", "actual_rows", "comma", "no", 1.0, "class", true,
            "95", true, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(e.contains("fewer than 2 fields"), "{e}");
        // A one-name class list can never describe a comparison either.
        let e = run(
            A2, B2, "only", "", "", "matrix", "actual_rows", "auto", "no", 1.0, "class", true,
            "95", true, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(e.contains("at least 2 class names"), "{e}");
    }

    #[test]
    fn a_numeric_two_column_paste_is_read_as_label_pairs() {
        // 3 rows × 2 numeric columns can only be y_true,y_pred values.
        let out = go("1,1\n0,0\n0,1", "1,1\n0,0\n0,0");
        assert!(out.contains("Observations: A = 3, B = 3 (2 correct vs 3)"), "{out}");
        assert!(out.contains("| Accuracy | 0.6667 | 1.0000 | +0.3333 |"), "{out}");
    }

    #[test]
    fn unknown_label_in_a_tally_is_rejected() {
        let e = run(
            "cat,cat\ndog,dog",
            "cat,cat\nfox,fox",
            "cat,dog",
            "",
            "",
            "labels",
            "actual_rows",
            "auto",
            "no",
            1.0,
            "class",
            true,
            "95",
            true,
            4.0,
            false,
            "markdown",
        )
        .unwrap_err();
        assert!(e.contains("actual label `fox` is not one of the compared classes"), "{e}");
    }

    #[test]
    fn too_many_classes_is_rejected() {
        let k = MAX_CLASSES + 1;
        let row: String = vec!["1"; k].join(",");
        let m: String = vec![row; k].join("\n");
        let e = run(
            &m, &m, "", "", "", "matrix", "actual_rows", "auto", "no", 1.0, "class", true, "95",
            true, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(e.contains("more than the 50 this tool compares"), "{e}");
    }
}
