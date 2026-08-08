//! Pure cumulative percentage / Pareto-table builder.

#[derive(Debug, Clone, PartialEq)]
struct Row {
    label: String,
    value: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct OutRow {
    rank: usize,
    label: String,
    value: f64,
    percent: f64,
    cumulative_count: usize,
    cumulative_sum: f64,
    cumulative_percent: f64,
    zone: &'static str,
}

const MAX_ROWS: usize = 10_000;

fn split_line<'a>(line: &'a str, delimiter: &str) -> Vec<&'a str> {
    match delimiter {
        "comma" => line.split(',').collect(),
        "tab" => line.split('\t').collect(),
        "semicolon" => line.split(';').collect(),
        "pipe" => line.split('|').collect(),
        _ => {
            let counts = [
                ("tab", line.matches('\t').count()),
                ("comma", line.matches(',').count()),
                ("semicolon", line.matches(';').count()),
                ("pipe", line.matches('|').count()),
            ];
            let best = counts.iter().max_by_key(|(_, n)| *n).map(|(d, _)| *d).unwrap_or("comma");
            if counts.iter().all(|(_, n)| *n == 0) {
                line.split_whitespace().collect()
            } else {
                split_line(line, best)
            }
        }
    }
}

fn parse_number(s: &str, row: usize) -> Result<f64, String> {
    let t = s.trim().trim_start_matches(['$', '£', '€']).trim_end_matches('%').trim();
    let cleaned: String = t.chars().filter(|c| *c != ',' && *c != '_').collect();
    let v: f64 = cleaned.parse().map_err(|_| format!("row {row}: '{s}' is not a number"))?;
    if !v.is_finite() || v < 0.0 {
        return Err(format!("row {row}: value must be a finite non-negative number"));
    }
    Ok(v)
}

fn parse_rows(data: &str, delimiter: &str, header: &str) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    let mut saw_first = false;
    for line in data.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let parts = split_line(line, delimiter);
        if parts.len() < 2 {
            return Err(format!("row {}: expected label and value", rows.len() + 1));
        }
        let label = parts[..parts.len() - 1].join(" ").trim().trim_matches('"').to_string();
        let value_part = parts[parts.len() - 1];
        if label.is_empty() {
            return Err(format!("row {}: label is empty", rows.len() + 1));
        }
        if !saw_first {
            saw_first = true;
            let first_is_header = match header {
                "yes" => true,
                "no" => false,
                "auto" => parse_number(value_part, 1).is_err(),
                other => return Err(format!("header must be auto, yes or no — got '{other}'")),
            };
            if first_is_header {
                continue;
            }
        }
        let value = parse_number(value_part, rows.len() + 1)?;
        rows.push(Row { label, value });
    }
    if rows.len() < 2 {
        return Err(format!("need at least 2 data rows after the header, got {}", rows.len()));
    }
    if rows.len() > MAX_ROWS {
        return Err(format!("at most {MAX_ROWS} rows are supported, got {}", rows.len()));
    }
    if rows.iter().all(|r| r.value == 0.0) {
        return Err("total must be greater than zero; every value was zero".into());
    }
    Ok(rows)
}

fn round(v: f64, d: usize) -> String {
    format!("{v:.d$}")
}

fn analyze(data: &str, delimiter: &str, header: &str, sort: &str, threshold: f64, top_n: usize) -> Result<(Vec<OutRow>, f64, usize, f64), String> {
    if !(0.0..=100.0).contains(&threshold) || !threshold.is_finite() {
        return Err(format!("threshold must be between 0 and 100, got {threshold}"));
    }
    let mut rows = parse_rows(data, delimiter, header)?;
    match sort {
        "desc" => rows.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal).then(a.label.cmp(&b.label))),
        "input" => {}
        other => return Err(format!("sort must be desc or input, got '{other}'")),
    }
    if top_n > 0 && rows.len() > top_n {
        let tail: f64 = rows[top_n..].iter().map(|r| r.value).sum();
        rows.truncate(top_n);
        rows.push(Row { label: "Other".into(), value: tail });
    }
    let total: f64 = rows.iter().map(|r| r.value).sum();
    let mut cumulative = 0.0;
    let mut vital_count = 0usize;
    let mut crossed = false;
    let out = rows.into_iter().enumerate().map(|(i, r)| {
        cumulative += r.value;
        let cp = cumulative / total * 100.0;
        let zone = if !crossed { "vital" } else { "trivial" };
        if !crossed {
            vital_count += 1;
            if cp >= threshold { crossed = true; }
        }
        OutRow { rank: i + 1, label: r.label, value: r.value, percent: r.value / total * 100.0, cumulative_count: i + 1, cumulative_sum: cumulative, cumulative_percent: cp, zone }
    }).collect::<Vec<_>>();
    let top_share = out.first().map(|r| r.percent).unwrap_or(0.0);
    Ok((out, total, vital_count, top_share))
}

fn table(rows: &[OutRow], total: f64, vital_count: usize, top_share: f64, threshold: f64, decimals: usize, markdown: bool) -> String {
    let mut s = String::new();
    s.push_str(&format!("total: {} · rows: {} · vital few at {:.1}%: {} · top share: {}%\n\n", round(total, decimals), rows.len(), threshold, vital_count, round(top_share, decimals)));
    if markdown {
        s.push_str("| rank | label | value | percent | cumulative count | cumulative sum | cumulative percent | zone |\n|---:|---|---:|---:|---:|---:|---:|---|\n");
        for r in rows { s.push_str(&format!("| {} | {} | {} | {}% | {} | {} | {}% | {} |\n", r.rank, r.label, round(r.value, decimals), round(r.percent, decimals), r.cumulative_count, round(r.cumulative_sum, decimals), round(r.cumulative_percent, decimals), r.zone)); }
    } else {
        s.push_str("rank\tlabel\tvalue\tpercent\tcumulative_count\tcumulative_sum\tcumulative_percent\tzone\n");
        for r in rows { s.push_str(&format!("{}\t{}\t{}\t{}%\t{}\t{}\t{}%\t{}\n", r.rank, r.label, round(r.value, decimals), round(r.percent, decimals), r.cumulative_count, round(r.cumulative_sum, decimals), round(r.cumulative_percent, decimals), r.zone)); }
        s.push_str("\npareto chart (# = share, | marks cumulative threshold crossing)\n");
        for r in rows {
            let bar = "#".repeat(((r.percent / 2.0).round() as usize).max(1).min(50));
            let marker = if r.zone == "vital" { "|" } else { " " };
            s.push_str(&format!("{:>2}. {:<24} {:<50} {} {}% cum\n", r.rank, r.label.chars().take(24).collect::<String>(), bar, marker, round(r.cumulative_percent, decimals)));
        }
    }
    s
}

fn csv(rows: &[OutRow], total: f64, vital_count: usize, top_share: f64, threshold: f64, decimals: usize) -> String {
    let mut s = format!("metric,value\ntotal,{}\nrow_count,{}\nvital_few_threshold,{}\nvital_few_count,{}\ntop_share_percent,{}\n\n", round(total, decimals), rows.len(), round(threshold, decimals), vital_count, round(top_share, decimals));
    s.push_str("rank,label,value,percent,cumulative_count,cumulative_sum,cumulative_percent,zone\n");
    for r in rows {
        let label = r.label.replace('"', "\"\"");
        s.push_str(&format!("{},\"{}\",{},{},{},{},{},{}\n", r.rank, label, round(r.value, decimals), round(r.percent, decimals), r.cumulative_count, round(r.cumulative_sum, decimals), round(r.cumulative_percent, decimals), r.zone));
    }
    s
}

#[allow(clippy::too_many_arguments)]
pub fn run(data: &str, delimiter: &str, header: &str, sort: &str, threshold: f64, top_n: usize, decimals: usize, output: &str) -> Result<String, String> {
    if decimals > 6 { return Err(format!("decimals must be between 0 and 6, got {decimals}")); }
    let (rows, total, vital_count, top_share) = analyze(data, delimiter, header, sort, threshold, top_n)?;
    match output {
        "table" => Ok(table(&rows, total, vital_count, top_share, threshold, decimals, false)),
        "markdown" => Ok(table(&rows, total, vital_count, top_share, threshold, decimals, true)),
        "csv" => Ok(csv(&rows, total, vital_count, top_share, threshold, decimals)),
        other => Err(format!("output must be table, csv or markdown, got '{other}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "issue,count\nScratches,400\nDents,250\nMisalignment,150\nPackaging,120\nOther,80";

    #[test]
    fn builds_descending_pareto_table() {
        let out = run(SAMPLE, "comma", "yes", "desc", 80.0, 0, 1, "table").unwrap();
        assert!(out.contains("total: 1000.0 · rows: 5 · vital few at 80.0%: 3"), "{out}");
        assert!(out.contains("1\tScratches\t400.0\t40.0%\t1\t400.0\t40.0%\tvital"), "{out}");
        assert!(out.contains("3\tMisalignment\t150.0\t15.0%\t3\t800.0\t80.0%\tvital"), "{out}");
        assert!(out.contains("pareto chart"));
    }

    #[test]
    fn supports_tail_bucket_and_csv() {
        let out = run(SAMPLE, "comma", "yes", "desc", 80.0, 3, 0, "csv").unwrap();
        assert!(out.contains("4,\"Other\",200,20,4,1000,100,trivial"), "{out}");
    }

    #[test]
    fn auto_header_and_tab_input_work() {
        let out = run("name\tvalue\nB\t2\nA\t8", "auto", "auto", "desc", 80.0, 0, 0, "markdown").unwrap();
        assert!(out.contains("| 1 | A | 8 | 80% | 1 | 8 | 80% | vital |"), "{out}");
    }

    #[test]
    fn errors_are_actionable() {
        assert!(run("A,ten\nB,2", "comma", "no", "desc", 80.0, 0, 1, "table").unwrap_err().contains("not a number"));
        assert!(run("A,0\nB,0", "comma", "no", "desc", 80.0, 0, 1, "table").unwrap_err().contains("total must be greater"));
        assert!(run("A,1", "comma", "no", "desc", 80.0, 0, 7, "table").unwrap_err().contains("decimals"));
    }
}
