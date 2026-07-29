//! pairwise-test-generator core — pure compute, shared by the chat skill block and the web page.
//!
//! Parses a plain-text parameter model (one parameter per line, `Name: v1, v2, …`) and emits a
//! minimal **all-pairs (pairwise)** test set: a set of cases in which every value of every
//! parameter is paired at least once with every value of every other parameter. Uses a
//! deterministic greedy algorithm — seed each new case from the first still-uncovered pair, then
//! fill the remaining parameters with the value that covers the most still-missing pairs
//! (ties broken by lowest index). Identical input always yields identical output.

use std::collections::HashSet;

/// Upper bounds keep the greedy search finite and the page responsive.
pub const MAX_PARAMS: usize = 20;
pub const MAX_VALUES: usize = 30;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Markdown,
    Csv,
    Json,
    Ascii,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" => Ok(OutputFormat::Markdown),
            "csv" => Ok(OutputFormat::Csv),
            "json" => Ok(OutputFormat::Json),
            "ascii" | "table" | "text" => Ok(OutputFormat::Ascii),
            other => Err(format!(
                "unknown output_format '{other}': use markdown, csv, json, or ascii"
            )),
        }
    }
}

struct Param {
    name: String,
    values: Vec<String>,
}

/// Parse the model text into parameters, validating as we go.
fn parse_params(input: &str) -> Result<Vec<Param>, String> {
    let mut params: Vec<Param> = Vec::new();
    for (idx, raw) in input.lines().enumerate() {
        let line = raw.trim();
        // Skip blank lines and `#` comments.
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let lineno = idx + 1;
        let colon = line.find(':').ok_or_else(|| {
            format!("line {lineno}: expected 'Name: value1, value2, …' but found no ':' in \"{line}\"")
        })?;
        let name = line[..colon].trim().to_string();
        if name.is_empty() {
            return Err(format!("line {lineno}: missing parameter name before ':'"));
        }
        let values: Vec<String> = line[colon + 1..]
            .split(',')
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect();
        if values.is_empty() {
            return Err(format!(
                "line {lineno}: parameter '{name}' has no values — list at least one, e.g. '{name}: a, b'"
            ));
        }
        if values.len() > MAX_VALUES {
            return Err(format!(
                "parameter '{name}' has {} values (max {MAX_VALUES})",
                values.len()
            ));
        }
        // Reject duplicate values within a parameter (they only add redundant pairs).
        for i in 0..values.len() {
            if values[..i].contains(&values[i]) {
                return Err(format!(
                    "parameter '{name}' has duplicate value '{}'",
                    values[i]
                ));
            }
        }
        if params.iter().any(|p| p.name == name) {
            return Err(format!("duplicate parameter name '{name}'"));
        }
        params.push(Param { name, values });
    }

    if params.len() < 2 {
        return Err(
            "need at least 2 parameters to form pairs — add one parameter per line like 'Browser: Chrome, Firefox'"
                .to_string(),
        );
    }
    if params.len() > MAX_PARAMS {
        return Err(format!("{} parameters given (max {MAX_PARAMS})", params.len()));
    }
    Ok(params)
}

/// Canonical key for the pair between parameter `p` (value index `pv`) and parameter `q`
/// (value index `qv`), with the lower parameter index first so each pair has one key.
fn pair_key(p: usize, pv: usize, q: usize, qv: usize) -> (usize, usize, usize, usize) {
    if p < q {
        (p, q, pv, qv)
    } else {
        (q, p, qv, pv)
    }
}

/// Generate the pairwise case set as row-major value indices (`cases[c][param]`).
fn generate_cases(params: &[Param]) -> Vec<Vec<usize>> {
    let n = params.len();
    let total_pairs: usize = (0..n)
        .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
        .map(|(i, j)| params[i].values.len() * params[j].values.len())
        .sum();

    let mut covered: HashSet<(usize, usize, usize, usize)> = HashSet::with_capacity(total_pairs);
    let mut cases: Vec<Vec<usize>> = Vec::new();
    const UNSET: usize = usize::MAX;

    while covered.len() < total_pairs {
        let mut case = vec![UNSET; n];

        // Seed from the first still-uncovered pair (deterministic scan order).
        'seed: for i in 0..n {
            for j in i + 1..n {
                for vi in 0..params[i].values.len() {
                    for vj in 0..params[j].values.len() {
                        if !covered.contains(&(i, j, vi, vj)) {
                            case[i] = vi;
                            case[j] = vj;
                            break 'seed;
                        }
                    }
                }
            }
        }

        // Fill the remaining parameters greedily.
        for p in 0..n {
            if case[p] != UNSET {
                continue;
            }
            let mut best_v = 0usize;
            let mut best_score: i64 = -1;
            for v in 0..params[p].values.len() {
                let mut score: i64 = 0;
                for (q, &qv) in case.iter().enumerate() {
                    if q == p || qv == UNSET {
                        continue;
                    }
                    if !covered.contains(&pair_key(p, v, q, qv)) {
                        score += 1;
                    }
                }
                if score > best_score {
                    best_score = score;
                    best_v = v;
                }
            }
            case[p] = best_v;
        }

        // Record every pair now covered by this case.
        for i in 0..n {
            for j in i + 1..n {
                covered.insert((i, j, case[i], case[j]));
            }
        }
        cases.push(case);
    }

    cases
}

fn quote_csv(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn escape_md(field: &str) -> String {
    field.replace('\\', "\\\\").replace('|', "\\|").replace('\n', " ")
}

fn escape_json(field: &str) -> String {
    let mut out = String::with_capacity(field.len() + 2);
    for c in field.chars() {
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

fn render(params: &[Param], cases: &[Vec<usize>], fmt: OutputFormat, include_index: bool) -> String {
    let cell = |c: usize, p: usize| params[p].values[cases[c][p]].as_str();
    let n = params.len();

    match fmt {
        OutputFormat::Json => {
            let mut out = String::from("[\n");
            for (c, _) in cases.iter().enumerate() {
                out.push_str("  {");
                let mut first = true;
                if include_index {
                    out.push_str(&format!("\"#\": {}", c + 1));
                    first = false;
                }
                for p in 0..n {
                    if !first {
                        out.push_str(", ");
                    }
                    first = false;
                    out.push_str(&format!(
                        "\"{}\": \"{}\"",
                        escape_json(&params[p].name),
                        escape_json(cell(c, p))
                    ));
                }
                out.push('}');
                if c + 1 < cases.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push(']');
            out
        }
        OutputFormat::Csv => {
            let mut header: Vec<String> = Vec::new();
            if include_index {
                header.push("#".to_string());
            }
            header.extend(params.iter().map(|p| quote_csv(&p.name)));
            let mut lines = vec![header.join(",")];
            for (c, _) in cases.iter().enumerate() {
                let mut row: Vec<String> = Vec::new();
                if include_index {
                    row.push((c + 1).to_string());
                }
                row.extend((0..n).map(|p| quote_csv(cell(c, p))));
                lines.push(row.join(","));
            }
            lines.join("\n")
        }
        OutputFormat::Markdown => {
            let mut headers: Vec<String> = Vec::new();
            if include_index {
                headers.push("#".to_string());
            }
            headers.extend(params.iter().map(|p| escape_md(&p.name)));

            let mut rows: Vec<Vec<String>> = Vec::new();
            for (c, _) in cases.iter().enumerate() {
                let mut row: Vec<String> = Vec::new();
                if include_index {
                    row.push((c + 1).to_string());
                }
                row.extend((0..n).map(|p| escape_md(cell(c, p))));
                rows.push(row);
            }

            let cols = headers.len();
            let mut width = vec![0usize; cols];
            for (i, h) in headers.iter().enumerate() {
                width[i] = h.chars().count();
            }
            for row in &rows {
                for (i, v) in row.iter().enumerate() {
                    width[i] = width[i].max(v.chars().count());
                }
            }
            let pad = |s: &str, w: usize| {
                let len = s.chars().count();
                format!("{}{}", s, " ".repeat(w.saturating_sub(len)))
            };
            let fmt_row = |cells: &[String]| -> String {
                let inner: Vec<String> =
                    cells.iter().enumerate().map(|(i, c)| pad(c, width[i])).collect();
                format!("| {} |", inner.join(" | "))
            };
            let mut out = vec![fmt_row(&headers)];
            let sep: Vec<String> = width.iter().map(|w| "-".repeat((*w).max(3))).collect();
            out.push(format!("| {} |", sep.join(" | ")));
            for row in &rows {
                out.push(fmt_row(row));
            }
            out.join("\n")
        }
        OutputFormat::Ascii => {
            let mut headers: Vec<String> = Vec::new();
            if include_index {
                headers.push("#".to_string());
            }
            headers.extend(params.iter().map(|p| p.name.replace('\n', " ")));

            let mut rows: Vec<Vec<String>> = Vec::new();
            for (c, _) in cases.iter().enumerate() {
                let mut row: Vec<String> = Vec::new();
                if include_index {
                    row.push((c + 1).to_string());
                }
                row.extend((0..n).map(|p| cell(c, p).replace('\n', " ")));
                rows.push(row);
            }

            let cols = headers.len();
            let mut width = vec![0usize; cols];
            for (i, h) in headers.iter().enumerate() {
                width[i] = h.chars().count();
            }
            for row in &rows {
                for (i, v) in row.iter().enumerate() {
                    width[i] = width[i].max(v.chars().count());
                }
            }
            let border: String = {
                let segs: Vec<String> = width.iter().map(|w| "-".repeat(w + 2)).collect();
                format!("+{}+", segs.join("+"))
            };
            let pad = |s: &str, w: usize| {
                let len = s.chars().count();
                format!(" {}{} ", s, " ".repeat(w.saturating_sub(len)))
            };
            let fmt_row = |cells: &[String]| -> String {
                let inner: Vec<String> =
                    cells.iter().enumerate().map(|(i, c)| pad(c, width[i])).collect();
                format!("|{}|", inner.join("|"))
            };
            let mut out = vec![border.clone(), fmt_row(&headers), border.clone()];
            for row in &rows {
                out.push(fmt_row(row));
            }
            out.push(border);
            out.join("\n")
        }
    }
}

/// Public entry point: parse `input`, generate the pairwise set, render as `format`.
pub fn generate(input: &str, format: OutputFormat, include_index: bool) -> Result<String, String> {
    let params = parse_params(input)?;
    let cases = generate_cases(&params);
    Ok(render(&params, &cases, format, include_index))
}

/// Number of exhaustive combinations (product of value counts) vs. the generated case count.
pub fn stats(input: &str) -> Result<(usize, usize), String> {
    let params = parse_params(input)?;
    let exhaustive = params.iter().map(|p| p.values.len()).product();
    let cases = generate_cases(&params);
    Ok((exhaustive, cases.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify EVERY value pair across every parameter combination is covered by the case set.
    fn assert_all_pairs_covered(input: &str) {
        let params = parse_params(input).unwrap();
        let cases = generate_cases(&params);
        let n = params.len();
        for i in 0..n {
            for j in i + 1..n {
                for vi in 0..params[i].values.len() {
                    for vj in 0..params[j].values.len() {
                        let hit = cases.iter().any(|c| c[i] == vi && c[j] == vj);
                        assert!(
                            hit,
                            "pair ({}={}, {}={}) never covered",
                            params[i].name, params[i].values[vi], params[j].name, params[j].values[vj]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn covers_all_pairs_three_params() {
        assert_all_pairs_covered(
            "Browser: Chrome, Firefox, Safari\nOS: Windows, macOS, Linux\nColor: Light, Dark",
        );
    }

    #[test]
    fn covers_all_pairs_uneven() {
        assert_all_pairs_covered("A: 1, 2, 3, 4\nB: x, y\nC: p, q, r\nD: t, f");
    }

    #[test]
    fn deterministic() {
        let input = "A: 1, 2, 3\nB: x, y\nC: p, q, r";
        let a = generate(input, OutputFormat::Csv, false).unwrap();
        let b = generate(input, OutputFormat::Csv, false).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn reduces_below_exhaustive() {
        // 3x3x2 = 18 exhaustive; pairwise should need far fewer (≈9).
        let (exhaustive, cases) =
            stats("Browser: Chrome, Firefox, Safari\nOS: Windows, macOS, Linux\nColor: Light, Dark")
                .unwrap();
        assert_eq!(exhaustive, 18);
        assert!(cases < exhaustive, "expected reduction, got {cases} of {exhaustive}");
        assert!(cases >= 9, "pairwise of two 3-value params needs at least 9 cases");
    }

    #[test]
    fn markdown_shape() {
        let out = generate("A: 1, 2\nB: x, y", OutputFormat::Markdown, true).unwrap();
        let first = out.lines().next().unwrap();
        assert_eq!(first, "| # | A | B |");
        assert!(out.lines().nth(1).unwrap().starts_with("| ---"));
    }

    #[test]
    fn csv_quotes_comma_in_name() {
        // A parameter name is everything before the first ':' and may contain a comma.
        let out = generate("A, size: 1, 2\nB: x, y", OutputFormat::Csv, false).unwrap();
        assert_eq!(out.lines().next().unwrap(), "\"A, size\",B");
    }

    #[test]
    fn json_is_array_of_objects() {
        let out = generate("A: 1, 2\nB: x, y", OutputFormat::Json, true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.is_array());
        let first = &v[0];
        assert_eq!(first["#"], 1);
        assert!(first["A"].is_string());
    }

    #[test]
    fn ascii_has_borders() {
        let out = generate("A: 1, 2\nB: x, y", OutputFormat::Ascii, false).unwrap();
        assert!(out.starts_with("+--"));
        assert!(out.contains("| A "));
    }

    #[test]
    fn skips_blank_and_comment_lines() {
        let out = generate("# a model\nA: 1, 2\n\nB: x, y\n", OutputFormat::Csv, false).unwrap();
        assert_eq!(out.lines().next().unwrap(), "A,B");
    }

    // ---- validation errors ----

    #[test]
    fn err_no_colon() {
        let e = generate("A 1 2\nB: x", OutputFormat::Csv, false).unwrap_err();
        assert!(e.contains("no ':'"), "{e}");
    }

    #[test]
    fn err_empty_values() {
        let e = generate("A:\nB: x, y", OutputFormat::Csv, false).unwrap_err();
        assert!(e.contains("no values"), "{e}");
    }

    #[test]
    fn err_missing_name() {
        let e = generate(": 1, 2\nB: x", OutputFormat::Csv, false).unwrap_err();
        assert!(e.contains("missing parameter name"), "{e}");
    }

    #[test]
    fn err_duplicate_param() {
        let e = generate("A: 1, 2\nA: 3, 4", OutputFormat::Csv, false).unwrap_err();
        assert!(e.contains("duplicate parameter name"), "{e}");
    }

    #[test]
    fn err_duplicate_value() {
        let e = generate("A: 1, 1\nB: x", OutputFormat::Csv, false).unwrap_err();
        assert!(e.contains("duplicate value"), "{e}");
    }

    #[test]
    fn err_too_few_params() {
        let e = generate("A: 1, 2, 3", OutputFormat::Csv, false).unwrap_err();
        assert!(e.contains("at least 2 parameters"), "{e}");
    }

    #[test]
    fn err_too_many_values() {
        let vals: Vec<String> = (0..MAX_VALUES + 1).map(|i| i.to_string()).collect();
        let input = format!("A: {}\nB: x, y", vals.join(", "));
        let e = generate(&input, OutputFormat::Csv, false).unwrap_err();
        assert!(e.contains("max"), "{e}");
    }

    #[test]
    fn err_bad_format() {
        assert!(OutputFormat::parse("xml").is_err());
    }
}
