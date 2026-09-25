//! confusion-matrix core — build a confusion matrix and the full classification
//! report (per-class precision / recall / F-score / support, macro, weighted and
//! micro averages, accuracy, balanced accuracy, Cohen's kappa, Matthews
//! correlation) from pasted actual vs predicted labels.
//!
//! Pure compute, no I/O: shared verbatim by the chat skill block, the CLI and
//! the browser page, so every surface returns byte-identical output.

/// Hard cap on the number of (actual, predicted) observations, counting the
/// weight column when an aggregated table is pasted.
const MAX_PAIRS: u64 = 500_000;
/// Hard cap on distinct class labels — the matrix is rendered in full, so a
/// runaway label column (e.g. free text mistaken for labels) is rejected early.
const MAX_LABELS: usize = 200;
/// z for a two-sided 95% interval; the Wilson score interval uses it directly.
const Z95: f64 = 1.959_963_984_540_054;

/// One observation: an actual label, a predicted label, and how many times the
/// pair occurs (1 unless an aggregated `actual,predicted,count` table is used).
struct Pair {
    actual: String,
    predicted: String,
    weight: u64,
}

/// One-vs-rest statistics for a single class.
struct ClassStats {
    label: String,
    tp: u64,
    fp: u64,
    fn_: u64,
    tn: u64,
    /// Rows in `actual` carrying this label.
    support: u64,
    /// Rows in `predicted` carrying this label.
    predicted: u64,
    precision: Option<f64>,
    recall: Option<f64>,
    specificity: Option<f64>,
    fscore: Option<f64>,
}

/// A macro / weighted / micro average row of the classification report.
struct Avg {
    name: &'static str,
    precision: Option<f64>,
    recall: Option<f64>,
    fscore: Option<f64>,
}

/// Divide, returning `None` (rendered `n/a`, JSON `null`) when the denominator
/// is zero — an undefined metric is reported as undefined, never as a silent 0.
fn div(num: f64, den: f64) -> Option<f64> {
    if den > 0.0 {
        Some(num / den)
    } else {
        None
    }
}

/// Undefined metrics count as 0 when averaging, matching scikit-learn's
/// `zero_division=0` so macro/weighted numbers line up with `classification_report`.
fn z(v: Option<f64>) -> f64 {
    v.unwrap_or(0.0)
}

/// Wilson score interval for a binomial proportion `x / n` at 95%.
fn wilson(x: u64, n: u64) -> Option<(f64, f64)> {
    if n == 0 {
        return None;
    }
    let (x, n) = (x as f64, n as f64);
    let z2 = Z95 * Z95;
    let center = (x + z2 / 2.0) / (n + z2);
    let half = (Z95 / (n + z2)) * (x * (n - x) / n + z2 / 4.0).sqrt();
    Some(((center - half).max(0.0), (center + half).min(1.0)))
}

fn normalized_head(s: &str) -> String {
    s.trim().to_ascii_lowercase().replace([' ', '_', '-'], "")
}

fn is_actual_header(s: &str) -> bool {
    matches!(
        normalized_head(s).as_str(),
        "actual"
            | "true"
            | "ytrue"
            | "expected"
            | "gold"
            | "reference"
            | "observed"
            | "truth"
            | "target"
            | "actuallabel"
            | "truelabel"
            | "goldlabel"
    )
}

fn is_predicted_header(s: &str) -> bool {
    matches!(
        normalized_head(s).as_str(),
        "predicted"
            | "pred"
            | "ypred"
            | "prediction"
            | "output"
            | "hypothesis"
            | "model"
            | "predictedlabel"
            | "guess"
    )
}

/// Split pasted text into labels. `auto` treats every newline, comma, tab,
/// semicolon and pipe that actually occurs as a separator, and falls back to
/// runs of whitespace when none of them do — so a one-per-line column, a
/// comma-separated row and a space-separated row all work unchanged.
fn split_labels(text: &str, separator: &str) -> Vec<String> {
    let cleaned: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let seps: Vec<char> = match separator {
        "newline" => vec!['\n'],
        "comma" => vec!['\n', ','],
        "tab" => vec!['\n', '\t'],
        "semicolon" => vec!['\n', ';'],
        "pipe" => vec!['\n', '|'],
        "space" => vec![],
        _ => ['\n', ',', '\t', ';', '|']
            .into_iter()
            .filter(|c| cleaned.contains(*c))
            .collect(),
    };
    let parts: Vec<String> = if seps.is_empty() {
        cleaned.split_whitespace().map(|s| s.to_string()).collect()
    } else {
        cleaned
            .split(|c| seps.contains(&c))
            .map(|s| s.trim().to_string())
            .collect()
    };
    parts.into_iter().filter(|s| !s.is_empty()).collect()
}

/// Pick the column separator of a pasted `actual,predicted[,count]` table.
fn table_delimiter(text: &str, separator: &str) -> Option<char> {
    match separator {
        "comma" => return Some(','),
        "tab" => return Some('\t'),
        "semicolon" => return Some(';'),
        "pipe" => return Some('|'),
        "space" | "newline" => return None,
        _ => {}
    }
    for c in ['\t', ';', '|', ','] {
        if text.contains(c) {
            return Some(c);
        }
    }
    None
}

fn split_row(line: &str, delim: Option<char>) -> Vec<String> {
    match delim {
        Some(c) => line.split(c).map(|s| s.trim().to_string()).collect(),
        None => line.split_whitespace().map(|s| s.to_string()).collect(),
    }
}

/// Parse the two-column (optionally three-column, weighted) paired table that is
/// pasted into `actual` when `predicted` is left empty.
fn parse_table(text: &str, separator: &str, header: &str) -> Result<Vec<Pair>, String> {
    let delim = table_delimiter(text, separator);
    let rows: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if rows.is_empty() {
        return Err("no data rows found — paste one `actual,predicted` pair per line".into());
    }
    let first = split_row(rows[0], delim);
    let looks_like_header =
        first.len() >= 2 && (is_actual_header(&first[0]) || is_predicted_header(&first[1]));
    let skip = match header {
        "yes" => 1,
        "no" => 0,
        _ if looks_like_header => 1,
        _ => 0,
    };
    let mut pairs = Vec::new();
    for (i, row) in rows.iter().enumerate().skip(skip) {
        let cells = split_row(row, delim);
        if cells.len() < 2 || cells[0].is_empty() || cells[1].is_empty() {
            return Err(format!(
                "row {} (`{}`): expected an `actual,predicted` pair (optionally with a count in a third column), got {} value(s)",
                i + 1,
                row,
                cells.iter().filter(|c| !c.is_empty()).count()
            ));
        }
        let weight = if cells.len() >= 3 && !cells[2].is_empty() {
            cells[2].parse::<u64>().ok().filter(|w| *w > 0).ok_or_else(|| {
                format!(
                    "row {} (`{}`): the third column is a repeat count and must be a whole number of 1 or more, got `{}`",
                    i + 1,
                    row,
                    cells[2]
                )
            })?
        } else {
            1
        };
        pairs.push(Pair {
            actual: cells[0].clone(),
            predicted: cells[1].clone(),
            weight,
        });
    }
    if pairs.is_empty() {
        return Err(
            "no data rows found after the header row — paste one `actual,predicted` pair per line"
                .into(),
        );
    }
    Ok(pairs)
}

/// Parse an already-tallied K×K matrix: rows are the actual class, columns the
/// predicted class, cells whole counts. The grid may carry its own class names
/// as a header row and/or a first label column (a corner cell is allowed); when
/// it carries none, `fallback` is used, then `positive`/`negative` for a 2×2
/// and `class 1`…`class K` beyond that. Returns the pairs, the class order the
/// grid implies, and whether the grid named its own classes.
fn parse_matrix(
    text: &str,
    separator: &str,
    header: &str,
    fallback: &[String],
) -> Result<(Vec<Pair>, Vec<String>, bool), String> {
    let delim = table_delimiter(text, separator);
    let mut rows: Vec<Vec<String>> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| split_row(l, delim))
        .collect();
    let is_num = |s: &str| s.parse::<f64>().is_ok();
    if rows.is_empty() {
        return Err("no matrix rows found — paste one row of counts per actual class".into());
    }
    let first_all_num = rows[0].iter().filter(|c| !c.is_empty()).all(|c| is_num(c));
    let has_header = match header {
        "yes" => true,
        "no" => false,
        _ => !first_all_num,
    };
    let col_names = if has_header {
        Some(rows.remove(0))
    } else {
        None
    };
    if rows.is_empty() {
        return Err("no matrix rows found after the header row".into());
    }
    let row_labeled = rows.iter().all(|r| {
        r.first()
            .map(|c| !c.is_empty() && !is_num(c))
            .unwrap_or(false)
    });
    let mut row_names: Vec<String> = Vec::new();
    let mut grid: Vec<Vec<u64>> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let mut cells: Vec<String> = r.clone();
        if row_labeled {
            row_names.push(cells.remove(0));
        }
        let mut counts = Vec::new();
        for c in cells.iter().filter(|c| !c.is_empty()) {
            counts.push(c.parse::<u64>().map_err(|_| {
                format!(
                    "matrix row {} (`{}`): every cell must be a whole count of 0 or more, got `{}`",
                    i + 1,
                    r.join(" "),
                    c
                )
            })?);
        }
        grid.push(counts);
    }
    let k = grid.len();
    if k < 2 {
        return Err(format!(
            "a confusion matrix needs at least 2 classes — got {k} row(s)"
        ));
    }
    if k > MAX_LABELS {
        return Err(format!("too many classes: {k} (maximum {MAX_LABELS})"));
    }
    for (i, row) in grid.iter().enumerate() {
        if row.len() != k {
            return Err(format!(
                "the matrix must be square — there are {k} rows but row {} has {} count(s)",
                i + 1,
                row.len()
            ));
        }
    }
    let mut names: Vec<String> = Vec::new();
    if row_labeled {
        names = row_names;
    } else if let Some(cn) = col_names {
        let mut cn: Vec<String> = cn.into_iter().filter(|c| !c.is_empty()).collect();
        if cn.len() == k + 1 {
            cn.remove(0);
        }
        if cn.len() == k {
            names = cn;
        }
    }
    let named = !names.is_empty();
    if names.is_empty() {
        names = if fallback.len() == k {
            fallback.to_vec()
        } else if k == 2 {
            vec!["positive".to_string(), "negative".to_string()]
        } else {
            (1..=k).map(|i| format!("class {i}")).collect()
        };
    }
    for (i, n) in names.iter().enumerate() {
        if names[..i].contains(n) {
            return Err(format!("class `{n}` appears twice in the matrix labels"));
        }
    }
    let total: u64 = grid.iter().flatten().sum();
    if total == 0 {
        return Err("the matrix is all zeros — there is nothing to score".into());
    }
    let mut pairs = Vec::with_capacity(k * k);
    for (i, row) in grid.iter().enumerate() {
        for (j, count) in row.iter().enumerate() {
            pairs.push(Pair {
                actual: names[i].clone(),
                predicted: names[j].clone(),
                weight: *count,
            });
        }
    }
    Ok((pairs, names, named))
}

/// Does a paste look like an already-tallied K×K grid of counts rather than a
/// list of `actual,predicted` pairs? Only consulted when `input` is left on
/// `auto`: a grid qualifies when, after dropping an optional header row and an
/// optional leading label column, every remaining cell is a whole number and
/// the grid is square. Numeric class labels are the ambiguous case (`0,1` rows
/// are both a 2×2 grid and a pair list) — that is what `input` is for.
fn looks_like_matrix(text: &str, separator: &str, header: &str) -> bool {
    let delim = table_delimiter(text, separator);
    let mut rows: Vec<Vec<String>> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| split_row(l, delim))
        .collect();
    let is_num = |s: &str| s.parse::<u64>().is_ok();
    if rows.is_empty() {
        return false;
    }
    let first_all_num = rows[0].iter().filter(|c| !c.is_empty()).all(|c| is_num(c));
    let drop_header = match header {
        "yes" => true,
        "no" => false,
        _ => !first_all_num,
    };
    if drop_header {
        rows.remove(0);
    }
    if rows.len() < 2 {
        return false;
    }
    let row_labeled = rows.iter().all(|r| {
        r.first()
            .map(|c| !c.is_empty() && !is_num(c))
            .unwrap_or(false)
    });
    let k = rows.len();
    rows.iter().all(|r| {
        let cells = if row_labeled { &r[1..] } else { &r[..] };
        let counts: Vec<&String> = cells.iter().filter(|c| !c.is_empty()).collect();
        counts.len() == k && counts.iter().all(|c| is_num(c))
    })
}

/// Parse the two separate label lists.
fn parse_lists(
    actual: &str,
    predicted: &str,
    separator: &str,
    header: &str,
) -> Result<Vec<Pair>, String> {
    let mut a = split_labels(actual, separator);
    let mut p = split_labels(predicted, separator);
    if a.is_empty() {
        return Err(
            "actual labels are empty — paste one label per line (or one comma-separated row)"
                .into(),
        );
    }
    if p.is_empty() {
        return Err(
            "predicted labels are empty — paste one label per line (or one comma-separated row)"
                .into(),
        );
    }
    let drop = match header {
        "yes" => true,
        "no" => false,
        _ => is_actual_header(&a[0]) || is_predicted_header(&p[0]),
    };
    if drop {
        a.remove(0);
        p.remove(0);
    }
    if a.len() != p.len() {
        return Err(format!(
            "actual and predicted must have the same number of labels — got {} actual and {} predicted",
            a.len(),
            p.len()
        ));
    }
    if a.is_empty() {
        return Err("no labels left after dropping the header row".into());
    }
    Ok(a.into_iter()
        .zip(p)
        .map(|(actual, predicted)| Pair {
            actual,
            predicted,
            weight: 1,
        })
        .collect())
}

/// Order the class labels: any label named in `wanted` first, in the order
/// given, then every remaining observed label (numerically when all labels are
/// numbers, otherwise alphabetically). Requested labels that never occur are
/// kept with zero support; observed labels are never dropped. `wanted` is the
/// user's `labels` list, or — when that is empty — the order a pasted matrix
/// implied.
fn order_labels(pairs: &[Pair], wanted: &[String]) -> Result<Vec<String>, String> {
    let mut seen: Vec<String> = Vec::new();
    for l in wanted {
        if seen.contains(l) {
            return Err(format!("label `{l}` is listed twice in the label order"));
        }
        seen.push(l.clone());
    }
    let mut wanted = seen;
    let mut observed: Vec<String> = Vec::new();
    for p in pairs {
        for l in [&p.actual, &p.predicted] {
            if !observed.contains(l) {
                observed.push(l.clone());
            }
        }
    }
    let numeric = observed.iter().all(|l| l.parse::<f64>().is_ok());
    let mut rest: Vec<String> = observed
        .into_iter()
        .filter(|l| !wanted.contains(l))
        .collect();
    if numeric {
        rest.sort_by(|a, b| {
            let (x, y) = (
                a.parse::<f64>().unwrap_or(0.0),
                b.parse::<f64>().unwrap_or(0.0),
            );
            x.partial_cmp(&y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.cmp(b))
        });
    } else {
        rest.sort();
    }
    wanted.extend(rest);
    if wanted.len() < 2 {
        return Err(format!(
            "a confusion matrix needs at least 2 classes — only `{}` was found; add the other class to the label order if it never occurs",
            wanted.first().map(|s| s.as_str()).unwrap_or("")
        ));
    }
    if wanted.len() > MAX_LABELS {
        return Err(format!(
            "too many distinct labels: {} (maximum {MAX_LABELS}) — check that the pasted columns are class labels and not free text",
            wanted.len()
        ));
    }
    Ok(wanted)
}

fn fmt_prop(v: Option<f64>, d: usize, percent: bool) -> String {
    match v {
        None => "n/a".to_string(),
        Some(x) if percent => format!("{:.*}%", d, x * 100.0),
        Some(x) => format!("{:.*}", d, x),
    }
}

/// Values that are not proportions (kappa, MCC, likelihood ratios) never take
/// the percent formatting — only the 0..1 rates do.
fn fmt_num(v: Option<f64>, d: usize) -> String {
    match v {
        None => "n/a".to_string(),
        Some(x) if x.is_infinite() => "inf".to_string(),
        Some(x) => format!("{:.*}", d, x),
    }
}

fn fmt_ci(ci: Option<(f64, f64)>, d: usize, percent: bool) -> String {
    match ci {
        None => "n/a".to_string(),
        Some((lo, hi)) => format!(
            "{} – {}",
            fmt_prop(Some(lo), d, percent),
            fmt_prop(Some(hi), d, percent)
        ),
    }
}

fn pad_left(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{}{}", " ".repeat(w - n), s)
    }
}

fn pad_right(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(w - n))
    }
}

/// Render a fixed-width table: first column left-aligned, the rest right-aligned.
fn text_table(headers: &[String], rows: &[Vec<String>]) -> String {
    let cols = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for r in rows {
        for (i, c) in r.iter().enumerate().take(cols) {
            widths[i] = widths[i].max(c.chars().count());
        }
    }
    let line = |cells: &[String]| -> String {
        let mut s = String::new();
        for (i, c) in cells.iter().enumerate().take(cols) {
            if i == 0 {
                s.push_str(&pad_right(c, widths[0]));
            } else {
                s.push_str("  ");
                s.push_str(&pad_left(c, widths[i]));
            }
        }
        s.trim_end().to_string()
    };
    let mut out = line(headers);
    out.push('\n');
    for r in rows {
        out.push_str(&line(r));
        out.push('\n');
    }
    out
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|")
}

fn md_table(headers: &[String], rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    out.push_str("| ");
    out.push_str(
        &headers
            .iter()
            .map(|h| md_escape(h))
            .collect::<Vec<_>>()
            .join(" | "),
    );
    out.push_str(" |\n|");
    for i in 0..headers.len() {
        out.push_str(if i == 0 { " --- |" } else { " ---: |" });
    }
    out.push('\n');
    for r in rows {
        out.push_str("| ");
        out.push_str(
            &r.iter()
                .map(|c| md_escape(c))
                .collect::<Vec<_>>()
                .join(" | "),
        );
        out.push_str(" |\n");
    }
    out
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

fn jstr(s: &str) -> String {
    format!("\"{}\"", json_escape(s))
}

fn jnum(v: Option<f64>, d: usize) -> String {
    match v {
        None => "null".to_string(),
        Some(x) if !x.is_finite() => "null".to_string(),
        Some(x) => format!("{:.*}", d, x),
    }
}

/// Everything the renderers need, computed once from the parsed pairs.
struct Report {
    labels: Vec<String>,
    matrix: Vec<Vec<u64>>,
    per_class: Vec<ClassStats>,
    averages: Vec<Avg>,
    total: u64,
    correct: u64,
    accuracy: Option<f64>,
    balanced_accuracy: Option<f64>,
    kappa: Option<f64>,
    mcc: Option<f64>,
    beta: f64,
    binary: Option<Binary>,
}

/// The binary (one positive class vs the rest) summary block.
struct Binary {
    positive: String,
    tp: u64,
    fp: u64,
    fn_: u64,
    tn: u64,
}

fn compute(pairs: &[Pair], labels: Vec<String>, beta: f64, positive: Option<String>) -> Report {
    let n = labels.len();
    let index = |l: &str| labels.iter().position(|x| x == l);
    let mut matrix = vec![vec![0u64; n]; n];
    for p in pairs {
        if let (Some(a), Some(b)) = (index(&p.actual), index(&p.predicted)) {
            matrix[a][b] += p.weight;
        }
    }
    let total: u64 = matrix.iter().flatten().sum();
    let correct: u64 = (0..n).map(|i| matrix[i][i]).sum();
    let row_sum = |i: usize| -> u64 { matrix[i].iter().sum() };
    let col_sum = |j: usize| -> u64 { (0..n).map(|i| matrix[i][j]).sum() };

    let b2 = beta * beta;
    let mut per_class = Vec::with_capacity(n);
    for (i, label) in labels.iter().enumerate() {
        let tp = matrix[i][i];
        let support = row_sum(i);
        let pred = col_sum(i);
        let fp = pred - tp;
        let fn_ = support - tp;
        let tn = total - tp - fp - fn_;
        let precision = div(tp as f64, pred as f64);
        let recall = div(tp as f64, support as f64);
        let specificity = div(tn as f64, (tn + fp) as f64);
        let fscore = match (precision, recall) {
            (Some(p), Some(r)) => div((1.0 + b2) * p * r, b2 * p + r).or(Some(0.0)),
            _ => None,
        };
        per_class.push(ClassStats {
            label: label.clone(),
            tp,
            fp,
            fn_,
            tn,
            support,
            predicted: pred,
            precision,
            recall,
            specificity,
            fscore,
        });
    }

    let k = n as f64;
    let macro_avg = Avg {
        name: "macro avg",
        precision: Some(per_class.iter().map(|c| z(c.precision)).sum::<f64>() / k),
        recall: Some(per_class.iter().map(|c| z(c.recall)).sum::<f64>() / k),
        fscore: Some(per_class.iter().map(|c| z(c.fscore)).sum::<f64>() / k),
    };
    let sup = total as f64;
    let weighted_avg = Avg {
        name: "weighted avg",
        precision: div(
            per_class
                .iter()
                .map(|c| z(c.precision) * c.support as f64)
                .sum::<f64>(),
            sup,
        ),
        recall: div(
            per_class
                .iter()
                .map(|c| z(c.recall) * c.support as f64)
                .sum::<f64>(),
            sup,
        ),
        fscore: div(
            per_class
                .iter()
                .map(|c| z(c.fscore) * c.support as f64)
                .sum::<f64>(),
            sup,
        ),
    };
    let mtp: u64 = per_class.iter().map(|c| c.tp).sum();
    let mfp: u64 = per_class.iter().map(|c| c.fp).sum();
    let mfn: u64 = per_class.iter().map(|c| c.fn_).sum();
    let micro_p = div(mtp as f64, (mtp + mfp) as f64);
    let micro_r = div(mtp as f64, (mtp + mfn) as f64);
    let micro_avg = Avg {
        name: "micro avg",
        precision: micro_p,
        recall: micro_r,
        fscore: match (micro_p, micro_r) {
            (Some(p), Some(r)) => div((1.0 + b2) * p * r, b2 * p + r).or(Some(0.0)),
            _ => None,
        },
    };

    let accuracy = div(correct as f64, total as f64);
    let present: Vec<f64> = per_class
        .iter()
        .filter(|c| c.support > 0)
        .map(|c| z(c.recall))
        .collect();
    let balanced_accuracy = div(present.iter().sum::<f64>(), present.len() as f64);

    // Cohen's kappa and the multiclass Matthews correlation (Gorodkin's R_K).
    let s = total as f64;
    let sum_tp_pp: f64 = (0..n).map(|i| row_sum(i) as f64 * col_sum(i) as f64).sum();
    let sum_pp2: f64 = (0..n).map(|i| (col_sum(i) as f64).powi(2)).sum();
    let sum_tt2: f64 = (0..n).map(|i| (row_sum(i) as f64).powi(2)).sum();
    let kappa = if s > 0.0 {
        let po = correct as f64 / s;
        let pe = sum_tp_pp / (s * s);
        div(po - pe, 1.0 - pe)
    } else {
        None
    };
    let mcc = {
        let denom = ((s * s - sum_pp2) * (s * s - sum_tt2)).sqrt();
        div(correct as f64 * s - sum_tp_pp, denom)
    };

    let binary = positive.and_then(|p| {
        let i = index(&p)?;
        Some(Binary {
            positive: p,
            tp: per_class[i].tp,
            fp: per_class[i].fp,
            fn_: per_class[i].fn_,
            tn: per_class[i].tn,
        })
    });

    Report {
        labels,
        matrix,
        per_class,
        averages: vec![macro_avg, weighted_avg, micro_avg],
        total,
        correct,
        accuracy,
        balanced_accuracy,
        kappa,
        mcc,
        beta,
        binary,
    }
}

/// Matrix cells as displayed: raw counts, or proportions of the row / column /
/// grand total. Totals are the sums of the displayed values.
fn matrix_cells(r: &Report, normalize: &str, d: usize, percent: bool) -> Vec<Vec<String>> {
    let n = r.labels.len();
    let row_sum = |i: usize| -> f64 { r.matrix[i].iter().sum::<u64>() as f64 };
    let col_sum = |j: usize| -> f64 { (0..n).map(|i| r.matrix[i][j]).sum::<u64>() as f64 };
    let total = r.total as f64;
    let mut rows = Vec::with_capacity(n + 1);
    let mut col_totals = vec![0f64; n];
    for i in 0..n {
        let mut row = vec![format!("actual: {}", r.labels[i])];
        let mut row_total = 0f64;
        for j in 0..n {
            let raw = r.matrix[i][j] as f64;
            let cell = match normalize {
                "row" => div(raw, row_sum(i)),
                "column" => div(raw, col_sum(j)),
                "all" => div(raw, total),
                _ => Some(raw),
            };
            row_total += z(cell);
            col_totals[j] += z(cell);
            row.push(match normalize {
                "none" => format!("{}", r.matrix[i][j]),
                _ => fmt_prop(cell, d, percent),
            });
        }
        row.push(match normalize {
            "none" => format!("{}", row_total as u64),
            _ => fmt_prop(Some(row_total), d, percent),
        });
        rows.push(row);
    }
    let mut totals = vec!["total".to_string()];
    for t in &col_totals {
        totals.push(match normalize {
            "none" => format!("{}", *t as u64),
            _ => fmt_prop(Some(*t), d, percent),
        });
    }
    totals.push(match normalize {
        "none" => format!("{}", r.total),
        _ => fmt_prop(Some(col_totals.iter().sum::<f64>()), d, percent),
    });
    rows.push(totals);
    rows
}

fn matrix_headers(r: &Report) -> Vec<String> {
    let mut h = vec!["actual \\ predicted".to_string()];
    h.extend(r.labels.iter().map(|l| format!("pred: {l}")));
    h.push("total".to_string());
    h
}

fn fscore_name(beta: f64) -> String {
    if (beta - 1.0).abs() < 1e-9 {
        return "f1-score".to_string();
    }
    let s = format!("{beta}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    format!("f{s}-score")
}

/// Every scalar of the binary summary, as (metric, value, 95% CI) triples.
fn binary_rows(b: &Binary, beta: f64, d: usize, percent: bool) -> Vec<Vec<String>> {
    let (tp, fp, fn_, tn) = (b.tp as f64, b.fp as f64, b.fn_ as f64, b.tn as f64);
    let n = tp + fp + fn_ + tn;
    let sens = div(tp, tp + fn_);
    let spec = div(tn, tn + fp);
    let prec = div(tp, tp + fp);
    let npv = div(tn, tn + fn_);
    let acc = div(tp + tn, n);
    let b2 = beta * beta;
    let f = match (prec, sens) {
        (Some(p), Some(r)) => div((1.0 + b2) * p * r, b2 * p + r).or(Some(0.0)),
        _ => None,
    };
    let mcc = div(
        tp * tn - fp * fn_,
        ((tp + fp) * (tp + fn_) * (tn + fp) * (tn + fn_)).sqrt(),
    );
    let po = z(acc);
    let pe = ((tp + fn_) * (tp + fp) + (tn + fp) * (tn + fn_)) / (n * n);
    let kappa = if n > 0.0 {
        div(po - pe, 1.0 - pe)
    } else {
        None
    };
    let lr_pos = match (sens, spec) {
        (Some(se), Some(sp)) => div(se, 1.0 - sp),
        _ => None,
    };
    let lr_neg = match (sens, spec) {
        (Some(se), Some(sp)) => div(1.0 - se, sp),
        _ => None,
    };
    let dor = match (lr_pos, lr_neg) {
        (Some(p), Some(neg)) => div(p, neg),
        _ => None,
    };
    let youden = match (sens, spec) {
        (Some(se), Some(sp)) => Some(se + sp - 1.0),
        _ => None,
    };
    let rows = vec![
        vec![
            "true positives (TP)".into(),
            format!("{}", b.tp),
            String::new(),
        ],
        vec![
            "false positives (FP)".into(),
            format!("{}", b.fp),
            String::new(),
        ],
        vec![
            "false negatives (FN)".into(),
            format!("{}", b.fn_),
            String::new(),
        ],
        vec![
            "true negatives (TN)".into(),
            format!("{}", b.tn),
            String::new(),
        ],
        vec![
            "accuracy".into(),
            fmt_prop(acc, d, percent),
            fmt_ci(wilson(b.tp + b.tn, b.tp + b.fp + b.fn_ + b.tn), d, percent),
        ],
        vec![
            "precision (PPV)".into(),
            fmt_prop(prec, d, percent),
            fmt_ci(wilson(b.tp, b.tp + b.fp), d, percent),
        ],
        vec![
            "recall (sensitivity, TPR)".into(),
            fmt_prop(sens, d, percent),
            fmt_ci(wilson(b.tp, b.tp + b.fn_), d, percent),
        ],
        vec![
            "specificity (TNR)".into(),
            fmt_prop(spec, d, percent),
            fmt_ci(wilson(b.tn, b.tn + b.fp), d, percent),
        ],
        vec![
            "negative predictive value (NPV)".into(),
            fmt_prop(npv, d, percent),
            fmt_ci(wilson(b.tn, b.tn + b.fn_), d, percent),
        ],
        vec![fscore_name(beta), fmt_prop(f, d, percent), String::new()],
        vec![
            "balanced accuracy".into(),
            fmt_prop(
                match (sens, spec) {
                    (Some(se), Some(sp)) => Some((se + sp) / 2.0),
                    _ => None,
                },
                d,
                percent,
            ),
            String::new(),
        ],
        vec![
            "false positive rate (FPR)".into(),
            fmt_prop(div(fp, fp + tn), d, percent),
            String::new(),
        ],
        vec![
            "false negative rate (FNR)".into(),
            fmt_prop(div(fn_, fn_ + tp), d, percent),
            String::new(),
        ],
        vec![
            "false discovery rate (FDR)".into(),
            fmt_prop(div(fp, fp + tp), d, percent),
            String::new(),
        ],
        vec![
            "false omission rate (FOR)".into(),
            fmt_prop(div(fn_, fn_ + tn), d, percent),
            String::new(),
        ],
        vec![
            "prevalence".into(),
            fmt_prop(div(tp + fn_, n), d, percent),
            String::new(),
        ],
        vec![
            "threat score (Jaccard)".into(),
            fmt_prop(div(tp, tp + fn_ + fp), d, percent),
            String::new(),
        ],
        vec![
            "Matthews correlation (MCC)".into(),
            fmt_num(mcc, d),
            String::new(),
        ],
        vec!["Cohen's kappa".into(), fmt_num(kappa, d), String::new()],
        vec!["Youden's J".into(), fmt_num(youden, d), String::new()],
        vec![
            "positive likelihood ratio (LR+)".into(),
            fmt_num(lr_pos, d),
            String::new(),
        ],
        vec![
            "negative likelihood ratio (LR-)".into(),
            fmt_num(lr_neg, d),
            String::new(),
        ],
        vec![
            "diagnostic odds ratio (DOR)".into(),
            fmt_num(dor, d),
            String::new(),
        ],
    ];
    rows
}

fn report_rows(r: &Report, d: usize, percent: bool) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for c in &r.per_class {
        rows.push(vec![
            c.label.clone(),
            fmt_prop(c.precision, d, percent),
            fmt_prop(c.recall, d, percent),
            fmt_prop(c.fscore, d, percent),
            format!("{}", c.support),
        ]);
    }
    rows.push(vec![
        "accuracy".into(),
        String::new(),
        String::new(),
        fmt_prop(r.accuracy, d, percent),
        format!("{}", r.total),
    ]);
    for a in &r.averages {
        rows.push(vec![
            a.name.to_string(),
            fmt_prop(a.precision, d, percent),
            fmt_prop(a.recall, d, percent),
            fmt_prop(a.fscore, d, percent),
            format!("{}", r.total),
        ]);
    }
    rows
}

fn overall_rows(r: &Report, d: usize, percent: bool) -> Vec<Vec<String>> {
    vec![
        vec!["observations".into(), format!("{}", r.total)],
        vec!["correct".into(), format!("{}", r.correct)],
        vec!["incorrect".into(), format!("{}", r.total - r.correct)],
        vec!["classes".into(), format!("{}", r.labels.len())],
        vec!["accuracy".into(), fmt_prop(r.accuracy, d, percent)],
        vec![
            "error rate".into(),
            fmt_prop(r.accuracy.map(|a| 1.0 - a), d, percent),
        ],
        vec![
            "balanced accuracy".into(),
            fmt_prop(r.balanced_accuracy, d, percent),
        ],
        vec!["Cohen's kappa".into(), fmt_num(r.kappa, d)],
        vec!["Matthews correlation (MCC)".into(), fmt_num(r.mcc, d)],
    ]
}

fn class_detail_rows(r: &Report) -> Vec<Vec<String>> {
    r.per_class
        .iter()
        .map(|c| {
            vec![
                c.label.clone(),
                format!("{}", c.tp),
                format!("{}", c.fp),
                format!("{}", c.fn_),
                format!("{}", c.tn),
                format!("{}", c.support),
                format!("{}", c.predicted),
            ]
        })
        .collect()
}

fn render_text(r: &Report, normalize: &str, d: usize, percent: bool) -> String {
    let mut o = String::new();
    o.push_str("Confusion matrix (rows = actual, columns = predicted)\n\n");
    o.push_str(&text_table(
        &matrix_headers(r),
        &matrix_cells(r, normalize, d, percent),
    ));
    if normalize != "none" {
        o.push_str(&format!("\nCells are normalized over the {normalize}.\n"));
    }
    o.push_str("\nClassification report\n\n");
    let headers = vec![
        "class".to_string(),
        "precision".into(),
        "recall".into(),
        fscore_name(r.beta),
        "support".into(),
    ];
    o.push_str(&text_table(&headers, &report_rows(r, d, percent)));
    o.push_str("\nPer-class one-vs-rest counts\n\n");
    let detail_headers = vec![
        "class".to_string(),
        "TP".into(),
        "FP".into(),
        "FN".into(),
        "TN".into(),
        "support".into(),
        "predicted".into(),
    ];
    o.push_str(&text_table(&detail_headers, &class_detail_rows(r)));
    o.push_str("\nOverall\n\n");
    o.push_str(&text_table(
        &["metric".to_string(), "value".to_string()],
        &overall_rows(r, d, percent),
    ));
    if let Some(b) = &r.binary {
        o.push_str(&format!(
            "\nBinary summary (positive class: {})\n\n",
            b.positive
        ));
        o.push_str(&text_table(
            &[
                "metric".to_string(),
                "value".to_string(),
                "95% CI".to_string(),
            ],
            &binary_rows(b, r.beta, d, percent),
        ));
    }
    o
}

fn render_markdown(r: &Report, normalize: &str, d: usize, percent: bool) -> String {
    let mut o = String::new();
    o.push_str(
        "## Confusion matrix\n\nRows are the actual class, columns are the predicted class.\n\n",
    );
    o.push_str(&md_table(
        &matrix_headers(r),
        &matrix_cells(r, normalize, d, percent),
    ));
    if normalize != "none" {
        o.push_str(&format!("\nCells are normalized over the {normalize}.\n"));
    }
    o.push_str("\n## Classification report\n\n");
    let headers = vec![
        "class".to_string(),
        "precision".into(),
        "recall".into(),
        fscore_name(r.beta),
        "support".into(),
    ];
    o.push_str(&md_table(&headers, &report_rows(r, d, percent)));
    o.push_str("\n## Per-class one-vs-rest counts\n\n");
    let detail_headers = vec![
        "class".to_string(),
        "TP".into(),
        "FP".into(),
        "FN".into(),
        "TN".into(),
        "support".into(),
        "predicted".into(),
    ];
    o.push_str(&md_table(&detail_headers, &class_detail_rows(r)));
    o.push_str("\n## Overall\n\n");
    o.push_str(&md_table(
        &["metric".to_string(), "value".to_string()],
        &overall_rows(r, d, percent),
    ));
    if let Some(b) = &r.binary {
        o.push_str(&format!(
            "\n## Binary summary (positive class: {})\n\n",
            md_escape(&b.positive)
        ));
        o.push_str(&md_table(
            &[
                "metric".to_string(),
                "value".to_string(),
                "95% CI".to_string(),
            ],
            &binary_rows(b, r.beta, d, percent),
        ));
    }
    o
}

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Tidy one-header CSV: `section,label,metric,value` — every number in the
/// report reachable with a single parse, no multi-header sections.
fn render_csv(r: &Report, normalize: &str, d: usize, percent: bool) -> String {
    let mut o = String::from("section,label,metric,value\n");
    let mut push = |section: &str, label: &str, metric: &str, value: &str| {
        o.push_str(&format!(
            "{},{},{},{}\n",
            csv_cell(section),
            csv_cell(label),
            csv_cell(metric),
            csv_cell(value)
        ));
    };
    let cells = matrix_cells(r, normalize, d, percent);
    for (i, row) in cells.iter().enumerate() {
        let label = if i < r.labels.len() {
            r.labels[i].clone()
        } else {
            "total".to_string()
        };
        for (j, v) in row.iter().enumerate().skip(1) {
            let metric = if j <= r.labels.len() {
                r.labels[j - 1].clone()
            } else {
                "total".to_string()
            };
            push("matrix", &label, &metric, v);
        }
    }
    for c in &r.per_class {
        push(
            "per_class",
            &c.label,
            "precision",
            &fmt_prop(c.precision, d, percent),
        );
        push(
            "per_class",
            &c.label,
            "recall",
            &fmt_prop(c.recall, d, percent),
        );
        push(
            "per_class",
            &c.label,
            &fscore_name(r.beta),
            &fmt_prop(c.fscore, d, percent),
        );
        push(
            "per_class",
            &c.label,
            "specificity",
            &fmt_prop(c.specificity, d, percent),
        );
        push("per_class", &c.label, "support", &format!("{}", c.support));
        push(
            "per_class",
            &c.label,
            "predicted",
            &format!("{}", c.predicted),
        );
        push("per_class", &c.label, "tp", &format!("{}", c.tp));
        push("per_class", &c.label, "fp", &format!("{}", c.fp));
        push("per_class", &c.label, "fn", &format!("{}", c.fn_));
        push("per_class", &c.label, "tn", &format!("{}", c.tn));
    }
    for a in &r.averages {
        push(
            "average",
            a.name,
            "precision",
            &fmt_prop(a.precision, d, percent),
        );
        push("average", a.name, "recall", &fmt_prop(a.recall, d, percent));
        push(
            "average",
            a.name,
            &fscore_name(r.beta),
            &fmt_prop(a.fscore, d, percent),
        );
        push("average", a.name, "support", &format!("{}", r.total));
    }
    for row in overall_rows(r, d, percent) {
        push("overall", "", &row[0], &row[1]);
    }
    if let Some(b) = &r.binary {
        for row in binary_rows(b, r.beta, d, percent) {
            push("binary", &b.positive, &row[0], &row[1]);
            if !row[2].is_empty() && row[2] != "n/a" {
                push(
                    "binary",
                    &b.positive,
                    &format!("{} 95% CI", row[0]),
                    &row[2],
                );
            }
        }
    }
    o
}

fn render_json(r: &Report, normalize: &str, d: usize) -> String {
    let f_key = if (r.beta - 1.0).abs() < 1e-9 {
        "f1"
    } else {
        "f_beta"
    };
    let mut o = String::from("{\n");
    o.push_str(&format!(
        "  \"labels\": [{}],\n",
        r.labels
            .iter()
            .map(|l| jstr(l))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    o.push_str("  \"matrix\": [");
    o.push_str(
        &r.matrix
            .iter()
            .map(|row| {
                format!(
                    "[{}]",
                    row.iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join(", "),
    );
    o.push_str("],\n");
    if normalize != "none" {
        let n = r.labels.len();
        let row_sum = |i: usize| -> f64 { r.matrix[i].iter().sum::<u64>() as f64 };
        let col_sum = |j: usize| -> f64 { (0..n).map(|i| r.matrix[i][j]).sum::<u64>() as f64 };
        let norm: Vec<String> = (0..n)
            .map(|i| {
                let cells: Vec<String> = (0..n)
                    .map(|j| {
                        let raw = r.matrix[i][j] as f64;
                        let v = match normalize {
                            "row" => div(raw, row_sum(i)),
                            "column" => div(raw, col_sum(j)),
                            _ => div(raw, r.total as f64),
                        };
                        jnum(v, d)
                    })
                    .collect();
                format!("[{}]", cells.join(", "))
            })
            .collect();
        o.push_str(&format!(
            "  \"matrix_normalized\": [{}],\n",
            norm.join(", ")
        ));
        o.push_str(&format!("  \"normalize\": {},\n", jstr(normalize)));
    }
    o.push_str(&format!("  \"observations\": {},\n", r.total));
    o.push_str(&format!("  \"correct\": {},\n", r.correct));
    o.push_str(&format!("  \"accuracy\": {},\n", jnum(r.accuracy, d)));
    o.push_str(&format!(
        "  \"error_rate\": {},\n",
        jnum(r.accuracy.map(|a| 1.0 - a), d)
    ));
    o.push_str(&format!(
        "  \"balanced_accuracy\": {},\n",
        jnum(r.balanced_accuracy, d)
    ));
    o.push_str(&format!("  \"cohens_kappa\": {},\n", jnum(r.kappa, d)));
    o.push_str(&format!("  \"mcc\": {},\n", jnum(r.mcc, d)));
    o.push_str(&format!("  \"beta\": {},\n", jnum(Some(r.beta), 4)));
    o.push_str("  \"per_class\": [\n");
    let classes: Vec<String> = r
        .per_class
        .iter()
        .map(|c| {
            format!(
                "    {{\"label\": {}, \"precision\": {}, \"recall\": {}, \"{}\": {}, \"specificity\": {}, \"support\": {}, \"predicted\": {}, \"tp\": {}, \"fp\": {}, \"fn\": {}, \"tn\": {}}}",
                jstr(&c.label),
                jnum(c.precision, d),
                jnum(c.recall, d),
                f_key,
                jnum(c.fscore, d),
                jnum(c.specificity, d),
                c.support,
                c.predicted,
                c.tp,
                c.fp,
                c.fn_,
                c.tn
            )
        })
        .collect();
    o.push_str(&classes.join(",\n"));
    o.push_str("\n  ],\n");
    for a in &r.averages {
        let key = a.name.replace(' ', "_");
        o.push_str(&format!(
            "  \"{}\": {{\"precision\": {}, \"recall\": {}, \"{}\": {}, \"support\": {}}},\n",
            key,
            jnum(a.precision, d),
            jnum(a.recall, d),
            f_key,
            jnum(a.fscore, d),
            r.total
        ));
    }
    match &r.binary {
        Some(b) => {
            let rows = binary_rows(b, r.beta, d, false);
            let entries: Vec<String> = rows
                .iter()
                .map(|row| {
                    let key = row[0]
                        .replace(['(', ')', '\'', '+'], "")
                        .replace(['-', ' ', '%'], "_")
                        .replace("__", "_")
                        .trim_matches('_')
                        .to_ascii_lowercase();
                    let raw = &row[1];
                    let value = if raw == "n/a" || raw == "inf" {
                        "null".to_string()
                    } else {
                        raw.clone()
                    };
                    let ci = if row[2].is_empty() || row[2] == "n/a" {
                        String::new()
                    } else {
                        let parts: Vec<&str> = row[2].split(" – ").collect();
                        format!(
                            ", \"{}_ci\": [{}, {}]",
                            key,
                            parts.first().copied().unwrap_or("null"),
                            parts.get(1).copied().unwrap_or("null")
                        )
                    };
                    format!("\"{key}\": {value}{ci}")
                })
                .collect();
            o.push_str(&format!(
                "  \"binary\": {{\"positive_label\": {}, {}}}\n",
                jstr(&b.positive),
                entries.join(", ")
            ));
        }
        None => o.push_str("  \"binary\": null\n"),
    }
    o.push_str("}\n");
    o
}

/// Build the confusion matrix and classification report.
///
/// `actual` holds the true labels — or, when `predicted` is empty, a whole
/// `actual,predicted[,count]` table or an already-tallied K×K matrix of counts;
/// `input_format` (`auto`, `labels`, `table`, `matrix`) forces the reading when
/// the guess would be wrong. `predicted` holds the model's labels. `labels` is
/// an optional class order. See the block descriptor for the meaning of every
/// option.
#[allow(clippy::too_many_arguments)]
pub fn run(
    actual: &str,
    predicted: &str,
    labels: &str,
    positive_label: &str,
    input_format: &str,
    separator: &str,
    header: &str,
    normalize: &str,
    beta: f64,
    decimals: f64,
    percent: bool,
    format: &str,
) -> Result<String, String> {
    let allowed = |name: &str, v: &str, opts: &[&str]| -> Result<String, String> {
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
    };
    let input_format = allowed(
        "input",
        input_format,
        &["auto", "labels", "table", "matrix"],
    )?;
    let separator = allowed(
        "separator",
        separator,
        &[
            "auto",
            "newline",
            "comma",
            "tab",
            "semicolon",
            "pipe",
            "space",
        ],
    )?;
    let header = allowed("header", header, &["auto", "yes", "no"])?;
    let normalize = allowed("normalize", normalize, &["none", "row", "column", "all"])?;
    let format = allowed("format", format, &["markdown", "text", "csv", "json"])?;
    if !beta.is_finite() || beta < 0.1 || beta > 10.0 {
        return Err(format!(
            "beta must be between 0.1 and 10, got `{beta}` (1 gives the usual F1 score)"
        ));
    }
    if !decimals.is_finite() || !(0.0..=10.0).contains(&decimals) {
        return Err(format!(
            "decimals must be between 0 and 10, got `{decimals}`"
        ));
    }
    let d = decimals.round() as usize;

    if actual.trim().is_empty() {
        return Err(
            "actual labels are required — paste one label per line, or a two-column `actual,predicted` table"
                .into(),
        );
    }
    let wanted: Vec<String> = split_labels(labels, &separator);
    let has_predicted = !predicted.trim().is_empty();
    // The class order a pasted grid implies (its own header/label names, or the
    // fallback names it was given) — used to order the report when the caller
    // did not spell out `labels`.
    let mut grid_order: Vec<String> = Vec::new();
    let pairs = match input_format.as_str() {
        "labels" => {
            if !has_predicted {
                return Err("input is set to labels, so the predicted labels are required — paste the model's labels in `predicted`, or set input to table or matrix".into());
            }
            parse_lists(actual, predicted, &separator, &header)?
        }
        "table" | "matrix" if has_predicted => {
            return Err(format!(
                "input is set to {input_format}, so `predicted` must be empty — paste the whole {} into `actual`",
                if input_format == "matrix" { "grid of counts" } else { "two-column table" }
            ));
        }
        "table" => parse_table(actual, &separator, &header)?,
        "matrix" => {
            let (pairs, names, _) = parse_matrix(actual, &separator, &header, &wanted)?;
            grid_order = names;
            pairs
        }
        // auto: two lists when `predicted` is filled in, otherwise a square grid
        // of whole counts if the paste looks like one, else a paired table.
        _ if has_predicted => parse_lists(actual, predicted, &separator, &header)?,
        _ if looks_like_matrix(actual, &separator, &header) => {
            let (pairs, names, _) = parse_matrix(actual, &separator, &header, &wanted)?;
            grid_order = names;
            pairs
        }
        _ => parse_table(actual, &separator, &header)?,
    };
    let total: u64 = pairs.iter().map(|p| p.weight).sum();
    if total > MAX_PAIRS {
        return Err(format!(
            "too many observations: {total} (maximum {MAX_PAIRS})"
        ));
    }

    let mut order_hint = wanted;
    for name in &grid_order {
        if !order_hint.contains(name) {
            order_hint.push(name.clone());
        }
    }
    let order = order_labels(&pairs, &order_hint)?;
    let positive = {
        let want = positive_label.trim();
        if want.is_empty() {
            if order.len() == 2 {
                const PREFERRED: [&str; 10] = [
                    "1",
                    "true",
                    "yes",
                    "positive",
                    "pos",
                    "y",
                    "t",
                    "spam",
                    "fraud",
                    "malignant",
                ];
                Some(
                    order
                        .iter()
                        .find(|l| PREFERRED.contains(&l.to_ascii_lowercase().as_str()))
                        .cloned()
                        .unwrap_or_else(|| order[1].clone()),
                )
            } else {
                None
            }
        } else if let Some(l) = order.iter().find(|l| l.as_str() == want) {
            Some(l.clone())
        } else {
            return Err(format!(
                "positive_label `{want}` is not one of the classes: {}",
                order.join(", ")
            ));
        }
    };

    let report = compute(&pairs, order, beta, positive);
    Ok(match format.as_str() {
        "text" => render_text(&report, &normalize, d, percent),
        "csv" => render_csv(&report, &normalize, d, percent),
        "json" => render_json(&report, &normalize, d),
        _ => render_markdown(&report, &normalize, d, percent),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACTUAL: &str =
        "cat\ncat\ncat\ncat\ncat\ncat\ndog\ndog\ndog\ndog\ndog\ndog\ndog\ndog\ndog";
    const PREDICTED: &str =
        "cat\ncat\ncat\ncat\ncat\ndog\ncat\ncat\ndog\ndog\ndog\ndog\ndog\ndog\ndog";

    fn md(actual: &str, predicted: &str) -> String {
        run(
            actual, predicted, "", "", "auto", "auto", "auto", "none", 1.0, 4.0, false, "markdown",
        )
        .unwrap()
    }

    #[test]
    fn binary_markdown_report_has_the_expected_metrics() {
        let out = md(ACTUAL, PREDICTED);
        // 5 cat->cat, 1 cat->dog, 2 dog->cat, 7 dog->dog.
        assert!(out.contains("| actual: cat | 5 | 1 | 6 |"), "{out}");
        assert!(out.contains("| actual: dog | 2 | 7 | 9 |"), "{out}");
        assert!(out.contains("| total | 7 | 8 | 15 |"), "{out}");
        // cat: precision 5/7, recall 5/6; dog: precision 7/8, recall 7/9.
        assert!(
            out.contains("| cat | 0.7143 | 0.8333 | 0.7692 | 6 |"),
            "{out}"
        );
        assert!(
            out.contains("| dog | 0.8750 | 0.7778 | 0.8235 | 9 |"),
            "{out}"
        );
        assert!(out.contains("| accuracy |  |  | 0.8000 | 15 |"), "{out}");
        assert!(
            out.contains("| macro avg | 0.7946 | 0.8056 | 0.7964 | 15 |"),
            "{out}"
        );
        assert!(
            out.contains("| weighted avg | 0.8107 | 0.8000 | 0.8018 | 15 |"),
            "{out}"
        );
        assert!(
            out.contains("| micro avg | 0.8000 | 0.8000 | 0.8000 | 15 |"),
            "{out}"
        );
    }

    #[test]
    fn sklearn_classification_report_example_matches() {
        // scikit-learn's documented classification_report example.
        let actual = "0\n1\n2\n2\n2";
        let predicted = "0\n0\n2\n2\n1";
        let out = run(
            actual, predicted, "", "", "auto", "auto", "auto", "none", 1.0, 2.0, false, "text",
        )
        .unwrap();
        assert!(
            out.contains("0                  0.50    1.00      0.67        1"),
            "{out}"
        );
        assert!(
            out.contains("1                  0.00    0.00      0.00        1"),
            "{out}"
        );
        assert!(
            out.contains("2                  1.00    0.67      0.80        3"),
            "{out}"
        );
        assert!(
            out.contains("accuracy                             0.60        5"),
            "{out}"
        );
        assert!(
            out.contains("macro avg          0.50    0.56      0.49        5"),
            "{out}"
        );
        assert!(
            out.contains("weighted avg       0.70    0.60      0.61        5"),
            "{out}"
        );
    }

    #[test]
    fn binary_summary_is_added_for_two_classes() {
        let out = md(ACTUAL, PREDICTED);
        assert!(
            out.contains("Binary summary (positive class: dog)"),
            "{out}"
        );
        assert!(out.contains("| true positives (TP) | 7 |"), "{out}");
        assert!(out.contains("| false positives (FP) | 1 |"), "{out}");
        assert!(out.contains("| false negatives (FN) | 2 |"), "{out}");
        assert!(out.contains("| true negatives (TN) | 5 |"), "{out}");
        assert!(out.contains("| specificity (TNR) | 0.8333 |"), "{out}");
        assert!(out.contains("| Youden's J | 0.6111 |"), "{out}");
    }

    #[test]
    fn positive_label_selects_the_binary_class() {
        let out = run(
            ACTUAL, PREDICTED, "", "cat", "auto", "auto", "auto", "none", 1.0, 4.0, false,
            "markdown",
        )
        .unwrap();
        assert!(
            out.contains("Binary summary (positive class: cat)"),
            "{out}"
        );
        assert!(out.contains("| true positives (TP) | 5 |"), "{out}");
        assert!(out.contains("| false negatives (FN) | 1 |"), "{out}");
    }

    #[test]
    fn spam_example_matches_published_numbers() {
        // 1000 emails: 180 TP, 20 FN, 40 FP, 760 TN — accuracy 94%, recall 90%,
        // specificity 95%, precision ~81.82%, F1 ~85.71%.
        let table = "actual,predicted,count\nspam,spam,180\nspam,ham,20\nham,spam,40\nham,ham,760";
        let out = run(
            table, "", "", "spam", "auto", "auto", "auto", "none", 1.0, 4.0, true, "text",
        )
        .unwrap();
        assert!(
            out.contains("accuracy                         94.0000%  92.3529% – 95.3104%"),
            "{out}"
        );
        assert!(
            out.contains("recall (sensitivity, TPR)        90.0000%  85.0594% – 93.4330%"),
            "{out}"
        );
        assert!(
            out.contains("specificity (TNR)                95.0000%  93.2630% – 96.3069%"),
            "{out}"
        );
        assert!(
            out.contains("precision (PPV)                  81.8182%  76.1900% – 86.3542%"),
            "{out}"
        );
        assert!(
            out.contains("f1-score                         85.7143%"),
            "{out}"
        );
    }

    #[test]
    fn json_output_is_parseable_shaped() {
        let out = run(
            ACTUAL, PREDICTED, "", "dog", "auto", "auto", "auto", "none", 1.0, 4.0, false, "json",
        )
        .unwrap();
        assert!(out.contains("\"labels\": [\"cat\", \"dog\"]"), "{out}");
        assert!(out.contains("\"matrix\": [[5, 1], [2, 7]]"), "{out}");
        assert!(out.contains("\"accuracy\": 0.8000"), "{out}");
        assert!(
            out.contains("\"macro_avg\": {\"precision\": 0.7946"),
            "{out}"
        );
        assert!(out.contains("\"positive_label\": \"dog\""), "{out}");
        assert!(out.contains("\"true_positives_tp\": 7"), "{out}");
        assert!(out.contains("\"accuracy_ci\": ["), "{out}");
    }

    #[test]
    fn csv_output_is_tidy() {
        let out = run(
            ACTUAL, PREDICTED, "", "", "auto", "auto", "auto", "none", 1.0, 4.0, false, "csv",
        )
        .unwrap();
        assert!(out.starts_with("section,label,metric,value\n"), "{out}");
        assert!(out.contains("matrix,cat,cat,5\n"), "{out}");
        assert!(out.contains("matrix,cat,dog,1\n"), "{out}");
        assert!(out.contains("per_class,dog,precision,0.8750\n"), "{out}");
        assert!(out.contains("average,macro avg,recall,0.8056\n"), "{out}");
        assert!(out.contains("overall,,accuracy,0.8000\n"), "{out}");
    }

    #[test]
    fn multiclass_matrix_and_kappa() {
        let actual = "a,a,a,b,b,b,c,c,c";
        let predicted = "a,a,b,b,b,c,c,c,a";
        let out = run(
            actual, predicted, "", "", "auto", "auto", "auto", "none", 1.0, 4.0, false, "text",
        )
        .unwrap();
        assert!(
            out.contains("actual: a                 2        1        0      3"),
            "{out}"
        );
        // 6/9 correct -> accuracy 0.6667, kappa 0.5 with balanced marginals.
        assert!(out.contains("accuracy                    0.6667"), "{out}");
        assert!(out.contains("Cohen's kappa               0.5000"), "{out}");
    }

    #[test]
    fn aggregated_counts_table_is_accepted() {
        let table = "actual,predicted,count\ncat,cat,5\ncat,dog,1\ndog,cat,2\ndog,dog,7";
        let out = run(
            table, "", "", "", "auto", "auto", "auto", "none", 1.0, 4.0, false, "markdown",
        )
        .unwrap();
        assert_eq!(out, md(ACTUAL, PREDICTED));
    }

    #[test]
    fn paired_table_without_counts_is_accepted() {
        let table = "actual,predicted\ncat,cat\ncat,dog\ndog,dog";
        let out = run(
            table, "", "", "", "auto", "auto", "auto", "none", 1.0, 4.0, false, "text",
        )
        .unwrap();
        assert!(
            out.contains("actual: cat                 1          1      2"),
            "{out}"
        );
    }

    #[test]
    fn header_rows_are_detected_in_both_columns() {
        let out = md("y_true\ncat\ndog", "y_pred\ncat\ncat");
        assert!(out.contains("| actual: cat | 1 | 0 | 1 |"), "{out}");
        assert!(out.contains("| actual: dog | 1 | 0 | 1 |"), "{out}");
    }

    #[test]
    fn label_order_puts_requested_classes_first_and_keeps_unseen_ones() {
        let out = run(
            "cat,dog,cat",
            "cat,dog,dog",
            "dog, cat, fox",
            "",
            "auto",
            "auto",
            "auto",
            "none",
            1.0,
            4.0,
            false,
            "markdown",
        )
        .unwrap();
        assert!(
            out.contains("| actual \\ predicted | pred: dog | pred: cat | pred: fox | total |"),
            "{out}"
        );
        assert!(out.contains("| fox | n/a | n/a | n/a | 0 |"), "{out}");
    }

    #[test]
    fn normalize_row_turns_cells_into_recall() {
        let out = run(
            ACTUAL, PREDICTED, "", "", "auto", "auto", "auto", "row", 1.0, 4.0, false, "markdown",
        )
        .unwrap();
        assert!(
            out.contains("| actual: cat | 0.8333 | 0.1667 | 1.0000 |"),
            "{out}"
        );
        assert!(out.contains("normalized over the row"), "{out}");
    }

    #[test]
    fn beta_two_weights_recall_higher() {
        let out = run(
            ACTUAL, PREDICTED, "", "dog", "auto", "auto", "auto", "none", 2.0, 4.0, false,
            "markdown",
        )
        .unwrap();
        assert!(out.contains("f2-score"), "{out}");
        // dog: precision 0.875, recall 0.7778 -> F2 = 5pr/(4p+r) = 0.7955.
        assert!(
            out.contains("| dog | 0.8750 | 0.7778 | 0.7955 | 9 |"),
            "{out}"
        );
    }

    #[test]
    fn space_separated_single_rows_work() {
        let out = md("yes no yes no", "yes yes no no");
        assert!(out.contains("| actual: no | 1 | 1 | 2 |"), "{out}");
    }

    #[test]
    fn mismatched_lengths_are_rejected() {
        let err = run(
            "a\nb\nc", "a\nb", "", "", "auto", "auto", "auto", "none", 1.0, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert_eq!(
            err,
            "actual and predicted must have the same number of labels — got 3 actual and 2 predicted"
        );
    }

    #[test]
    fn empty_input_is_rejected() {
        let err = run(
            "  ", "a\nb", "", "", "auto", "auto", "auto", "none", 1.0, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(err.contains("actual labels are required"), "{err}");
    }

    #[test]
    fn single_class_is_rejected() {
        let err = run(
            "a\na", "a\na", "", "", "auto", "auto", "auto", "none", 1.0, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(err.contains("at least 2 classes"), "{err}");
    }

    #[test]
    fn unknown_positive_label_is_rejected() {
        let err = run(
            ACTUAL, PREDICTED, "", "bird", "auto", "auto", "auto", "none", 1.0, 4.0, false,
            "markdown",
        )
        .unwrap_err();
        assert_eq!(
            err,
            "positive_label `bird` is not one of the classes: cat, dog"
        );
    }

    #[test]
    fn unknown_format_is_rejected() {
        let err = run(
            ACTUAL, PREDICTED, "", "", "auto", "auto", "auto", "none", 1.0, 4.0, false, "yaml",
        )
        .unwrap_err();
        assert_eq!(
            err,
            "format must be one of markdown, text, csv, json, got `yaml`"
        );
    }

    #[test]
    fn out_of_range_beta_is_rejected() {
        let err = run(
            ACTUAL, PREDICTED, "", "", "auto", "auto", "auto", "none", 0.0, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(err.contains("beta must be between 0.1 and 10"), "{err}");
    }

    #[test]
    fn bad_count_column_is_rejected() {
        let err = run(
            "cat,cat,x\ndog,dog,2",
            "",
            "",
            "",
            "auto",
            "auto",
            "auto",
            "none",
            1.0,
            4.0,
            false,
            "markdown",
        )
        .unwrap_err();
        assert!(
            err.contains("must be a whole number of 1 or more, got `x`"),
            "{err}"
        );
    }

    #[test]
    fn short_table_row_is_rejected() {
        let err = run(
            "cat,cat\ndog",
            "",
            "",
            "",
            "auto",
            "auto",
            "auto",
            "none",
            1.0,
            4.0,
            false,
            "markdown",
        )
        .unwrap_err();
        assert!(err.contains("expected an `actual,predicted` pair"), "{err}");
    }

    #[test]
    fn too_many_labels_are_rejected() {
        let actual: Vec<String> = (0..MAX_LABELS + 1).map(|i| format!("c{i}")).collect();
        let joined = actual.join("\n");
        let err = run(
            &joined, &joined, "", "", "auto", "auto", "auto", "none", 1.0, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(err.contains("too many distinct labels: 201"), "{err}");
    }

    #[test]
    fn cap_boundary_is_accepted() {
        let actual: Vec<String> = (0..MAX_LABELS).map(|i| format!("c{i}")).collect();
        let joined = actual.join("\n");
        let out = run(
            &joined, &joined, "", "", "auto", "auto", "auto", "none", 1.0, 4.0, false, "json",
        )
        .unwrap();
        assert!(out.contains("\"accuracy\": 1.0000"), "{out}");
    }

    #[test]
    fn too_many_observations_are_rejected() {
        let table = format!("cat,cat,{}\ndog,dog,1", MAX_PAIRS);
        let err = run(
            &table, "", "", "", "auto", "auto", "auto", "none", 1.0, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(err.contains("too many observations: 500001"), "{err}");
    }

    #[test]
    fn labelled_grid_of_counts_is_detected_and_named() {
        // A pasted sklearn-style grid: corner cell, header names, row names.
        let grid = ",cat,dog\ncat,5,1\ndog,2,7";
        let out = run(
            grid, "", "", "", "auto", "auto", "auto", "none", 1.0, 4.0, false, "markdown",
        )
        .unwrap();
        assert_eq!(out, md(ACTUAL, PREDICTED));
    }

    #[test]
    fn bare_grid_of_counts_uses_positive_negative_for_two_classes() {
        let out = run(
            "5,1\n2,7", "", "", "", "matrix", "auto", "no", "none", 1.0, 4.0, false, "text",
        )
        .unwrap();
        assert!(out.contains("actual: positive"), "{out}");
        assert!(out.contains(" 5               1      6"), "{out}");
        assert!(
            out.contains("Binary summary (positive class: positive)"),
            "{out}"
        );
        assert!(out.contains("accuracy                    0.8000"), "{out}");
    }

    #[test]
    fn grid_labels_come_from_the_labels_option_when_the_grid_has_none() {
        let out = run(
            "5,1\n2,7",
            "",
            "spam, ham",
            "",
            "matrix",
            "auto",
            "no",
            "none",
            1.0,
            4.0,
            false,
            "markdown",
        )
        .unwrap();
        assert!(out.contains("| actual: spam | 5 | 1 | 6 |"), "{out}");
        assert!(out.contains("| actual: ham | 2 | 7 | 9 |"), "{out}");
    }

    #[test]
    fn input_table_reads_numeric_pairs_that_auto_would_read_as_a_grid() {
        // Square, all-numeric and two rows: auto reads this as a 2x2 grid, so
        // `input = table` is how numeric class labels are forced to be pairs.
        let numeric = "0,1\n1,1";
        let as_grid = run(
            numeric, "", "", "", "auto", "auto", "no", "none", 1.0, 4.0, false, "json",
        )
        .unwrap();
        assert!(as_grid.contains("\"observations\": 3"), "{as_grid}");
        let as_pairs = run(
            numeric, "", "", "", "table", "auto", "no", "none", 1.0, 4.0, false, "json",
        )
        .unwrap();
        assert!(as_pairs.contains("\"observations\": 2"), "{as_pairs}");
        assert!(
            as_pairs.contains("\"matrix\": [[0, 1], [0, 1]]"),
            "{as_pairs}"
        );
    }

    #[test]
    fn input_labels_without_predicted_is_rejected() {
        let err = run(
            "cat,dog", "", "", "", "labels", "auto", "auto", "none", 1.0, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert!(err.contains("input is set to labels"), "{err}");
    }

    #[test]
    fn input_matrix_with_a_predicted_column_is_rejected() {
        let err = run(
            "5,1\n2,7", "cat", "", "", "matrix", "auto", "auto", "none", 1.0, 4.0, false,
            "markdown",
        )
        .unwrap_err();
        assert!(err.contains("`predicted` must be empty"), "{err}");
    }

    #[test]
    fn unknown_input_format_is_rejected() {
        let err = run(
            ACTUAL, PREDICTED, "", "", "grid", "auto", "auto", "none", 1.0, 4.0, false, "markdown",
        )
        .unwrap_err();
        assert_eq!(
            err,
            "input must be one of auto, labels, table, matrix, got `grid`"
        );
    }

    #[test]
    fn non_square_grid_is_rejected() {
        let err = run(
            "5,1,0\n2,7,1",
            "",
            "",
            "",
            "matrix",
            "auto",
            "no",
            "none",
            1.0,
            4.0,
            false,
            "markdown",
        )
        .unwrap_err();
        assert!(err.contains("must be square"), "{err}");
    }

    #[test]
    fn comments_are_ignored() {
        let out = md("# labels\ncat\ndog", "cat\ndog");
        assert!(out.contains("| accuracy |  |  | 1.0000 | 2 |"), "{out}");
    }

    #[test]
    fn undefined_precision_reports_na_and_averages_as_zero() {
        // Nothing is ever predicted as `b`, so b's precision is undefined.
        let out = md("a\nb", "a\na");
        assert!(out.contains("| b | n/a | 0.0000 | n/a | 1 |"), "{out}");
        assert!(
            out.contains("| macro avg | 0.2500 | 0.5000 | 0.3333 | 2 |"),
            "{out}"
        );
    }
}
