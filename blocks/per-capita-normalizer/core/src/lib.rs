//! per-capita-normalizer core — turn raw counts into population-normalized rates.
//!
//! Pure compute shared by the chat skill block and the web page: parse pasted
//! `label, count, population` rows, divide each count by its population, scale to
//! a reporting base (per person, per 1,000 … per 1,000,000 or a custom base), and
//! emit a ranked table with an index against the overall rate plus a small-count
//! reliability flag. No wafer/wasm-bindgen deps.

/// Maximum number of data rows accepted in one run.
pub const MAX_ROWS: usize = 10_000;

#[derive(Debug, Clone, PartialEq)]
struct Row {
    label: String,
    count: f64,
    population: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct OutRow {
    rank: usize,
    label: String,
    count: f64,
    population: f64,
    rate: f64,
    index: f64,
    flag: &'static str,
}

fn split_line<'a>(line: &'a str, delimiter: &'a str) -> Vec<&'a str> {
    match delimiter {
        "comma" => line.split(',').collect(),
        "tab" => line.split('\t').collect(),
        "semicolon" => line.split(';').collect(),
        "pipe" => line.split('|').collect(),
        "" | "auto" => {
            let counts = [
                ("tab", line.matches('\t').count()),
                ("comma", line.matches(',').count()),
                ("semicolon", line.matches(';').count()),
                ("pipe", line.matches('|').count()),
            ];
            if counts.iter().all(|(_, n)| *n == 0) {
                line.split_whitespace().collect()
            } else {
                let best = counts
                    .iter()
                    .max_by_key(|(_, n)| *n)
                    .map(|(d, _)| *d)
                    .unwrap_or("comma");
                split_line(line, best)
            }
        }
        other => vec![other], // unreachable: validated in parse_rows
    }
}

fn valid_delimiter(delimiter: &str) -> Result<(), String> {
    match delimiter {
        "" | "auto" | "comma" | "tab" | "semicolon" | "pipe" => Ok(()),
        other => Err(format!(
            "delimiter must be auto, comma, tab, semicolon or pipe — got '{other}'"
        )),
    }
}

fn delimiter_join(delimiter: &str) -> &'static str {
    match delimiter {
        "tab" => "\t",
        "semicolon" => ";",
        "pipe" => "|",
        _ => ",",
    }
}

/// Parse one numeric cell, tolerating currency symbols, thousands separators
/// (`,` `_` and thin spaces) and a trailing percent sign.
fn parse_number(s: &str, row: usize, what: &str) -> Result<f64, String> {
    let t = s.trim().trim_start_matches(['$', '£', '€']).trim();
    let cleaned: String = t
        .chars()
        .filter(|c| *c != ',' && *c != '_' && *c != ' ' && *c != '\u{202f}' && *c != '\u{a0}')
        .collect();
    let v: f64 = cleaned
        .parse()
        .map_err(|_| format!("row {row}: {what} '{}' is not a number", s.trim()))?;
    if !v.is_finite() {
        return Err(format!("row {row}: {what} must be a finite number"));
    }
    Ok(v)
}

fn population_multiplier(unit: &str) -> Result<f64, String> {
    Ok(match unit {
        "" | "ones" => 1.0,
        "thousands" => 1_000.0,
        "millions" => 1_000_000.0,
        other => {
            return Err(format!(
                "population_unit must be ones, thousands or millions — got '{other}'"
            ))
        }
    })
}

/// Resolve the reporting base: one of the presets, or `custom_per` when
/// `per = "custom"`.
pub fn resolve_basis(per: &str, custom_per: f64) -> Result<f64, String> {
    let basis = match per {
        "" | "100000" => 100_000.0,
        "1" => 1.0,
        "1000" => 1_000.0,
        "10000" => 10_000.0,
        "1000000" => 1_000_000.0,
        "custom" => {
            if !custom_per.is_finite() || custom_per <= 0.0 {
                return Err(format!(
                    "custom_per must be greater than 0 when per='custom', got {}",
                    fmt_plain(custom_per)
                ));
            }
            if custom_per > 1e12 {
                return Err("custom_per must be 1000000000000 or less".into());
            }
            custom_per
        }
        other => {
            return Err(format!(
                "per must be 1, 1000, 10000, 100000, 1000000 or custom — got '{other}'"
            ))
        }
    };
    Ok(basis)
}

fn parse_rows(
    data: &str,
    delimiter: &str,
    header: &str,
    pop_mult: f64,
) -> Result<Vec<Row>, String> {
    valid_delimiter(delimiter)?;
    if !matches!(header, "" | "auto" | "yes" | "no") {
        return Err(format!("header must be auto, yes or no — got '{header}'"));
    }
    let mut rows: Vec<Row> = Vec::new();
    let mut saw_first = false;
    for line in data.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let parts = split_line(line, delimiter);
        let n = parts.len();
        if n < 2 {
            return Err(format!(
                "row {}: expected at least count and population, got '{line}'",
                rows.len() + 1
            ));
        }
        let count_part = parts[n - 2];
        let pop_part = parts[n - 1];
        let label = if n == 2 {
            format!("row {}", rows.len() + 1)
        } else {
            parts[..n - 2]
                .join(delimiter_join(delimiter))
                .trim()
                .trim_matches('"')
                .to_string()
        };
        if !saw_first {
            saw_first = true;
            let first_is_header = match header {
                "yes" => true,
                "no" => false,
                _ => {
                    parse_number(count_part, 1, "count").is_err()
                        || parse_number(pop_part, 1, "population").is_err()
                }
            };
            if first_is_header {
                continue;
            }
        }
        let row_no = rows.len() + 1;
        if label.is_empty() {
            return Err(format!("row {row_no}: label is empty"));
        }
        let count = parse_number(count_part, row_no, "count")?;
        let population = parse_number(pop_part, row_no, "population")? * pop_mult;
        if count < 0.0 {
            return Err(format!(
                "row {row_no} ({label}): count must be zero or greater, got {}",
                fmt_plain(count)
            ));
        }
        if population <= 0.0 {
            return Err(format!(
                "row {row_no} ({label}): population must be greater than zero, got {}",
                fmt_plain(population)
            ));
        }
        rows.push(Row {
            label,
            count,
            population,
        });
    }
    if rows.is_empty() {
        return Err("no data rows found: paste at least one 'label, count, population' row".into());
    }
    if rows.len() > MAX_ROWS {
        return Err(format!(
            "at most {MAX_ROWS} rows are supported, got {}",
            rows.len()
        ));
    }
    Ok(rows)
}

/// Format a number without scientific notation, dropping a trailing `.0`.
fn fmt_plain(v: f64) -> String {
    if !v.is_finite() {
        return "not a number".into();
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn round(v: f64, d: usize) -> String {
    format!("{v:.d$}")
}

fn basis_suffix(basis: f64) -> String {
    if basis == 1.0 {
        "person".into()
    } else {
        fmt_plain(basis)
    }
}

fn rate_column(basis: f64) -> String {
    format!("rate_per_{}", basis_suffix(basis))
}

fn basis_phrase(basis: f64) -> String {
    if basis == 1.0 {
        "per person (per capita)".into()
    } else {
        format!("per {}", fmt_plain(basis))
    }
}

fn analyze(
    data: &str,
    delimiter: &str,
    header: &str,
    basis: f64,
    pop_mult: f64,
    sort: &str,
    unstable_below: f64,
) -> Result<(Vec<OutRow>, f64, f64, f64), String> {
    if !unstable_below.is_finite() || unstable_below < 0.0 || unstable_below.fract() != 0.0 {
        return Err(format!(
            "unstable_below must be a whole number of 0 or more, got {}",
            fmt_plain(unstable_below)
        ));
    }
    let rows = parse_rows(data, delimiter, header, pop_mult)?;
    let total_count: f64 = rows.iter().map(|r| r.count).sum();
    let total_population: f64 = rows.iter().map(|r| r.population).sum();
    let overall_rate = total_count / total_population * basis;

    let mut scored: Vec<(Row, f64)> = rows
        .into_iter()
        .map(|r| {
            let rate = r.count / r.population * basis;
            (r, rate)
        })
        .collect();
    match sort {
        "" | "rate_desc" => scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0.label.cmp(&b.0.label))
        }),
        "rate_asc" => scored.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0.label.cmp(&b.0.label))
        }),
        "input" => {}
        other => {
            return Err(format!(
                "sort must be rate_desc, rate_asc or input — got '{other}'"
            ))
        }
    }

    let out = scored
        .into_iter()
        .enumerate()
        .map(|(i, (r, rate))| OutRow {
            rank: i + 1,
            label: r.label,
            count: r.count,
            population: r.population,
            rate,
            index: if overall_rate > 0.0 {
                rate / overall_rate
            } else {
                0.0
            },
            flag: if unstable_below > 0.0 && r.count < unstable_below {
                "unstable"
            } else {
                "ok"
            },
        })
        .collect::<Vec<_>>();
    Ok((out, total_count, total_population, overall_rate))
}

fn summary(
    rows: &[OutRow],
    total_count: f64,
    total_population: f64,
    overall_rate: f64,
    basis: f64,
    decimals: usize,
    unstable_below: f64,
) -> String {
    let unstable = rows.iter().filter(|r| r.flag == "unstable").count();
    let mut s = format!(
        "{} · rows: {} · total count: {} · total population: {} · overall rate: {}",
        basis_phrase(basis),
        rows.len(),
        fmt_plain(total_count),
        fmt_plain(total_population),
        round(overall_rate, decimals)
    );
    if unstable_below > 0.0 {
        s.push_str(&format!(
            " · flagged unstable (count < {}): {}",
            fmt_plain(unstable_below),
            unstable
        ));
    }
    s
}

fn text_table(rows: &[OutRow], head: &str, basis: f64, decimals: usize, markdown: bool) -> String {
    let col = rate_column(basis);
    let mut s = format!("{head}\n\n");
    if markdown {
        s.push_str(&format!(
            "| rank | label | count | population | {col} | index | flag |\n|---:|---|---:|---:|---:|---:|---|\n"
        ));
        for r in rows {
            s.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                r.rank,
                r.label,
                fmt_plain(r.count),
                fmt_plain(r.population),
                round(r.rate, decimals),
                round(r.index, 2),
                r.flag
            ));
        }
        return s;
    }
    s.push_str(&format!(
        "rank\tlabel\tcount\tpopulation\t{col}\tindex\tflag\n"
    ));
    for r in rows {
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            r.rank,
            r.label,
            fmt_plain(r.count),
            fmt_plain(r.population),
            round(r.rate, decimals),
            round(r.index, 2),
            r.flag
        ));
    }
    let max_rate = rows.iter().map(|r| r.rate).fold(0.0_f64, f64::max);
    if max_rate > 0.0 {
        s.push_str("\nrate chart (# = rate relative to the highest row)\n");
        for r in rows {
            let bar = "#".repeat(((r.rate / max_rate * 40.0).round() as usize).clamp(1, 40));
            s.push_str(&format!(
                "{:>2}. {:<24} {:<40} {}\n",
                r.rank,
                r.label.chars().take(24).collect::<String>(),
                bar,
                round(r.rate, decimals)
            ));
        }
    }
    s
}

fn csv_out(
    rows: &[OutRow],
    total_count: f64,
    total_population: f64,
    overall_rate: f64,
    basis: f64,
    decimals: usize,
    unstable_below: f64,
) -> String {
    let col = rate_column(basis);
    let mut s = format!(
        "metric,value\nbasis,{}\nrows,{}\ntotal_count,{}\ntotal_population,{}\noverall_rate,{}\nunstable_below,{}\n\n",
        fmt_plain(basis),
        rows.len(),
        fmt_plain(total_count),
        fmt_plain(total_population),
        round(overall_rate, decimals),
        fmt_plain(unstable_below)
    );
    s.push_str(&format!("rank,label,count,population,{col},index,flag\n"));
    for r in rows {
        let label = r.label.replace('"', "\"\"");
        s.push_str(&format!(
            "{},\"{}\",{},{},{},{},{}\n",
            r.rank,
            label,
            fmt_plain(r.count),
            fmt_plain(r.population),
            round(r.rate, decimals),
            round(r.index, 2),
            r.flag
        ));
    }
    s
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

fn json_out(
    rows: &[OutRow],
    total_count: f64,
    total_population: f64,
    overall_rate: f64,
    basis: f64,
    decimals: usize,
    unstable_below: f64,
) -> String {
    let col = rate_column(basis);
    let mut s = format!(
        "{{\n  \"basis\": {},\n  \"rate_column\": \"{}\",\n  \"rows\": {},\n  \"total_count\": {},\n  \"total_population\": {},\n  \"overall_rate\": {},\n  \"unstable_below\": {},\n  \"items\": [\n",
        fmt_plain(basis),
        col,
        rows.len(),
        fmt_plain(total_count),
        fmt_plain(total_population),
        round(overall_rate, decimals),
        fmt_plain(unstable_below)
    );
    for (i, r) in rows.iter().enumerate() {
        s.push_str(&format!(
            "    {{\"rank\": {}, \"label\": \"{}\", \"count\": {}, \"population\": {}, \"rate\": {}, \"index\": {}, \"flag\": \"{}\"}}{}\n",
            r.rank,
            json_escape(&r.label),
            fmt_plain(r.count),
            fmt_plain(r.population),
            round(r.rate, decimals),
            round(r.index, 2),
            r.flag,
            if i + 1 == rows.len() { "" } else { "," }
        ));
    }
    s.push_str("  ]\n}\n");
    s
}

/// Normalize counts by population into rates on a reporting base.
///
/// `data` is one `label, count, population` row per line (a 2-field row is read
/// as `count, population`). Returns the rendered report in the requested
/// `output` format.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    delimiter: &str,
    header: &str,
    per: &str,
    custom_per: f64,
    population_unit: &str,
    decimals: usize,
    sort: &str,
    unstable_below: f64,
    output: &str,
) -> Result<String, String> {
    if decimals > 6 {
        return Err(format!("decimals must be between 0 and 6, got {decimals}"));
    }
    let basis = resolve_basis(per, custom_per)?;
    let pop_mult = population_multiplier(population_unit)?;
    let (rows, total_count, total_population, overall_rate) = analyze(
        data,
        delimiter,
        header,
        basis,
        pop_mult,
        sort,
        unstable_below,
    )?;
    let head = summary(
        &rows,
        total_count,
        total_population,
        overall_rate,
        basis,
        decimals,
        unstable_below,
    );
    match output {
        "" | "table" => Ok(text_table(&rows, &head, basis, decimals, false)),
        "markdown" => Ok(text_table(&rows, &head, basis, decimals, true)),
        "csv" => Ok(csv_out(
            &rows,
            total_count,
            total_population,
            overall_rate,
            basis,
            decimals,
            unstable_below,
        )),
        "json" => Ok(json_out(
            &rows,
            total_count,
            total_population,
            overall_rate,
            basis,
            decimals,
            unstable_below,
        )),
        other => Err(format!(
            "output must be table, csv, markdown or json — got '{other}'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str =
        "region,cases,population\nNorthbridge,120,400000\nEastvale,45,150000\nWestport,18,900000";

    #[test]
    fn builds_per_100k_table() {
        let out = run(
            SAMPLE,
            "comma",
            "yes",
            "100000",
            0.0,
            "ones",
            2,
            "rate_desc",
            20.0,
            "table",
        )
        .unwrap();
        assert!(
            out.contains("per 100000 · rows: 3 · total count: 183 · total population: 1450000 · overall rate: 12.62 · flagged unstable (count < 20): 1"),
            "{out}"
        );
        assert!(
            out.contains("1\tEastvale\t45\t150000\t30.00\t2.38\tok"),
            "{out}"
        );
        assert!(
            out.contains("2\tNorthbridge\t120\t400000\t30.00\t2.38\tok"),
            "{out}"
        );
        assert!(
            out.contains("3\tWestport\t18\t900000\t2.00\t0.16\tunstable"),
            "{out}"
        );
        assert!(out.contains("rate chart"), "{out}");
    }

    #[test]
    fn per_capita_and_custom_basis() {
        let out = run(
            "A,50,200", "comma", "no", "1", 0.0, "ones", 3, "input", 0.0, "table",
        )
        .unwrap();
        assert!(out.contains("per person (per capita)"), "{out}");
        assert!(out.contains("rate_per_person"), "{out}");
        assert!(out.contains("1\tA\t50\t200\t0.250\t1.00\tok"), "{out}");

        let out = run(
            "A,50,200", "comma", "no", "custom", 250.0, "ones", 1, "input", 0.0, "csv",
        )
        .unwrap();
        assert!(out.contains("basis,250"), "{out}");
        assert!(
            out.contains("rank,label,count,population,rate_per_250,index,flag"),
            "{out}"
        );
        assert!(out.contains("1,\"A\",50,200,62.5,1.00,ok"), "{out}");
    }

    #[test]
    fn population_unit_scales_and_two_column_rows_work() {
        // Population given in thousands: 8 thousand people, 4 cases → 50 per 100k.
        let out = run(
            "4\t8",
            "tab",
            "no",
            "100000",
            0.0,
            "thousands",
            1,
            "input",
            0.0,
            "markdown",
        )
        .unwrap();
        assert!(
            out.contains("| 1 | row 1 | 4 | 8000 | 50.0 | 1.00 | ok |"),
            "{out}"
        );

        let out = run(
            "2,1", "comma", "no", "1000", 0.0, "millions", 2, "input", 0.0, "json",
        )
        .unwrap();
        assert!(out.contains("\"rate\": 0.00"), "{out}");
        assert!(out.contains("\"population\": 1000000"), "{out}");
    }

    #[test]
    fn auto_delimiter_and_auto_header_and_sorting() {
        let out = run(
            "area\tevents\tpeople\nNorth\t10\t1000\nSouth\t30\t1000",
            "auto",
            "auto",
            "1000",
            0.0,
            "ones",
            0,
            "rate_asc",
            0.0,
            "markdown",
        )
        .unwrap();
        assert!(
            out.contains("| 1 | North | 10 | 1000 | 10 | 0.50 | ok |"),
            "{out}"
        );
        assert!(
            out.contains("| 2 | South | 30 | 1000 | 30 | 1.50 | ok |"),
            "{out}"
        );
    }

    #[test]
    fn labels_may_contain_the_delimiter() {
        let out = run(
            "Springfield, IL,10,1000",
            "comma",
            "no",
            "100000",
            0.0,
            "ones",
            0,
            "input",
            0.0,
            "table",
        )
        .unwrap();
        assert!(
            out.contains("1\tSpringfield, IL\t10\t1000\t1000\t1.00\tok"),
            "{out}"
        );
    }

    #[test]
    fn thousands_separators_are_tolerated() {
        let out = run(
            "Metro\t1_200\t8,175,133",
            "tab",
            "no",
            "100000",
            0.0,
            "ones",
            2,
            "input",
            0.0,
            "table",
        )
        .unwrap();
        assert!(
            out.contains("1\tMetro\t1200\t8175133\t14.68\t1.00\tok"),
            "{out}"
        );
    }

    #[test]
    fn errors_are_actionable() {
        let e = run(
            "A,ten,100",
            "comma",
            "no",
            "100000",
            0.0,
            "ones",
            2,
            "input",
            0.0,
            "table",
        )
        .unwrap_err();
        assert!(e.contains("row 1: count 'ten' is not a number"), "{e}");

        let e = run(
            "A,5,0", "comma", "no", "100000", 0.0, "ones", 2, "input", 0.0, "table",
        )
        .unwrap_err();
        assert!(e.contains("population must be greater than zero"), "{e}");

        let e = run(
            "A,-5,10", "comma", "no", "100000", 0.0, "ones", 2, "input", 0.0, "table",
        )
        .unwrap_err();
        assert!(e.contains("count must be zero or greater"), "{e}");

        let e = run(
            "A,5,10", "comma", "no", "custom", 0.0, "ones", 2, "input", 0.0, "table",
        )
        .unwrap_err();
        assert!(e.contains("custom_per must be greater than 0"), "{e}");

        let e = run(
            "A,5,10", "comma", "no", "100000", 0.0, "ones", 7, "input", 0.0, "table",
        )
        .unwrap_err();
        assert!(e.contains("decimals must be between 0 and 6"), "{e}");

        let e = run(
            "", "comma", "no", "100000", 0.0, "ones", 2, "input", 0.0, "table",
        )
        .unwrap_err();
        assert!(e.contains("no data rows found"), "{e}");

        let e = run(
            "A,5,10", "comma", "no", "100000", 0.0, "ones", 2, "input", 0.0, "yaml",
        )
        .unwrap_err();
        assert!(
            e.contains("output must be table, csv, markdown or json"),
            "{e}"
        );

        let e = run(
            "A,5,10", "comma", "no", "100000", 0.0, "kilos", 2, "input", 0.0, "table",
        )
        .unwrap_err();
        assert!(e.contains("population_unit must be"), "{e}");

        let big = (0..MAX_ROWS + 1)
            .map(|i| format!("r{i},1,10"))
            .collect::<Vec<_>>()
            .join("\n");
        let e = run(
            &big, "comma", "no", "100000", 0.0, "ones", 2, "input", 0.0, "table",
        )
        .unwrap_err();
        assert!(e.contains("at most 10000 rows"), "{e}");
    }

    #[test]
    fn row_cap_boundary_is_accepted() {
        let rows = (0..MAX_ROWS)
            .map(|i| format!("r{i},1,10"))
            .collect::<Vec<_>>()
            .join("\n");
        let out = run(
            &rows, "comma", "no", "100000", 0.0, "ones", 0, "input", 0.0, "table",
        )
        .unwrap();
        assert!(out.contains("rows: 10000"), "{}", &out[..120]);
    }
}
