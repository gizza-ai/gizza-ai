//! gap-finder core — audit a pasted sequence that should run consecutively
//! (invoice numbers, order IDs, cheque numbers, serials) and report what is
//! missing: individual missing values, compressed gap ranges, duplicates,
//! off-step entries, values outside the expected range, and entries pasted out
//! of order. No wafer/wasm-bindgen deps — shared by the chat skill block and
//! the web page.

use std::collections::BTreeMap;

/// Maximum number of entries accepted in one run.
pub const MAX_VALUES: usize = 20_000;
/// Maximum number of expected slots between the start and the end of the
/// sequence. Guards against an expected range like 1 to 1,000,000,000.
pub const MAX_SPAN: i64 = 5_000_000;
/// Maximum accepted value for the `limit` listing cap.
pub const MAX_LIMIT: usize = 20_000;

/// How strictly an entry is parsed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum IdFormat {
    /// Accept plain numbers or IDs with a shared prefix/suffix (`INV-1004`).
    Auto,
    /// Accept plain integers only.
    Number,
}

impl IdFormat {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "auto" => Ok(IdFormat::Auto),
            "number" | "numbers" => Ok(IdFormat::Number),
            other => Err(format!(
                "unknown id_format {other:?} — expected auto or number"
            )),
        }
    }
}

/// How entries are separated in the pasted input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sep {
    /// Split on any of newline / comma / semicolon / pipe / whitespace.
    Auto,
    Newline,
    Comma,
    Space,
    Semicolon,
    Tab,
    Pipe,
}

impl Sep {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "auto" => Ok(Sep::Auto),
            "newline" | "line" => Ok(Sep::Newline),
            "comma" => Ok(Sep::Comma),
            "space" => Ok(Sep::Space),
            "semicolon" => Ok(Sep::Semicolon),
            "tab" => Ok(Sep::Tab),
            "pipe" => Ok(Sep::Pipe),
            other => Err(format!(
                "unknown separator {other:?} — expected auto, newline, comma, space, semicolon, tab, or pipe"
            )),
        }
    }
}

/// Whether the pasted order carries meaning.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Order {
    /// Treat the input as an unordered set (default).
    Sorted,
    /// Also flag entries that appear after a larger entry.
    Input,
}

impl Order {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "sorted" | "sort" => Ok(Order::Sorted),
            "input" | "preserve" => Ok(Order::Input),
            other => Err(format!(
                "unknown order {other:?} — expected sorted or input"
            )),
        }
    }
}

/// Output shape.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OutputFormat {
    /// Human-readable audit summary.
    Report,
    /// Every missing value, one per line.
    Missing,
    /// `gap_start<TAB>gap_end<TAB>count` rows.
    Table,
    /// Structured JSON.
    Json,
}

impl OutputFormat {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "report" => Ok(OutputFormat::Report),
            "missing" => Ok(OutputFormat::Missing),
            "table" => Ok(OutputFormat::Table),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!(
                "unknown output {other:?} — expected report, missing, table, or json"
            )),
        }
    }
}

/// Split the input into trimmed, non-empty entries.
fn split_tokens(data: &str, sep: Sep) -> Vec<&str> {
    let raw: Vec<&str> = match sep {
        Sep::Auto => data
            .split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '|')
            .collect(),
        Sep::Space => data.split_whitespace().collect(),
        Sep::Newline => data.split('\n').collect(),
        Sep::Comma => data.split(',').collect(),
        Sep::Semicolon => data.split(';').collect(),
        Sep::Tab => data.split('\t').collect(),
        Sep::Pipe => data.split('|').collect(),
    };
    raw.into_iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect()
}

/// One parsed entry: the text around its single digit run, the digit run
/// itself (so zero padding can be reproduced), and its numeric value.
struct Parsed {
    prefix: String,
    digits: String,
    suffix: String,
    value: i64,
}

/// Parse one entry into prefix + digit run + suffix. The LAST run of digits is
/// the counter, so `INV-1004`, `2026-0007` and `1004 (paid)` all work; a
/// leading `-`/`+` on an otherwise bare number is treated as a sign.
fn parse_entry(token: &str) -> Result<Parsed, String> {
    let t = token.trim();
    let chars: Vec<char> = t.chars().collect();
    let Some(last) = chars.iter().rposition(|c| c.is_ascii_digit()) else {
        return Err(format!(
            "{t:?} has no number in it — every entry needs a numeric part, e.g. 1004 or INV-1004"
        ));
    };
    let mut first = last;
    while first > 0 && chars[first - 1].is_ascii_digit() {
        first -= 1;
    }
    let digits: String = chars[first..=last].iter().collect();
    let mut prefix: String = chars[..first].iter().collect();
    let suffix: String = chars[last + 1..].iter().collect();
    let mut negative = false;
    if prefix == "-" {
        negative = true;
        prefix.clear();
    } else if prefix == "+" {
        prefix.clear();
    }
    let magnitude: i64 = digits.parse().map_err(|_| {
        format!("{t:?} is too large — the numeric part must fit in 18 digits")
    })?;
    Ok(Parsed {
        prefix,
        digits,
        suffix,
        value: if negative { -magnitude } else { magnitude },
    })
}

/// Render a value back in the input's own shape (prefix, zero padding, suffix).
fn fmt_value(v: i64, prefix: &str, suffix: &str, pad: usize) -> String {
    let mag = v.unsigned_abs().to_string();
    let padded = if mag.len() < pad {
        format!("{}{}", "0".repeat(pad - mag.len()), mag)
    } else {
        mag
    };
    format!(
        "{}{}{}{}",
        if v < 0 { "-" } else { "" },
        prefix,
        padded,
        suffix
    )
}

/// Render a gap as a single value or a compressed range. `A-B` is only used for
/// plain non-negative numbers, where it cannot be misread.
fn fmt_range(a: i64, b: i64, prefix: &str, suffix: &str, pad: usize) -> String {
    let fa = fmt_value(a, prefix, suffix, pad);
    if a == b {
        return fa;
    }
    let fb = fmt_value(b, prefix, suffix, pad);
    if prefix.is_empty() && suffix.is_empty() && a >= 0 && b >= 0 {
        format!("{fa}-{fb}")
    } else {
        format!("{fa} to {fb}")
    }
}

/// Join up to `limit` items, appending `, +N more` when truncated.
fn join_capped(items: &[String], limit: usize) -> String {
    if items.len() <= limit {
        return items.join(", ");
    }
    format!(
        "{}, +{} more",
        items[..limit].join(", "),
        items.len() - limit
    )
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

fn json_str_array(items: &[String], indent: &str) -> String {
    if items.is_empty() {
        return "[]".to_string();
    }
    let body: Vec<String> = items
        .iter()
        .map(|s| format!("{indent}  \"{}\"", json_escape(s)))
        .collect();
    format!("[\n{}\n{indent}]", body.join(",\n"))
}

/// Parse an expected-bound override (`start`/`end`). A bare number is accepted
/// even when the data uses an ID prefix; a prefixed bound must match the data.
fn parse_bound(raw: &str, field: &str, prefix: &str, suffix: &str) -> Result<i64, String> {
    let p = parse_entry(raw).map_err(|e| format!("{field}: {e}"))?;
    if !p.prefix.is_empty() && p.prefix != prefix {
        return Err(format!(
            "{field} {raw:?} uses prefix {:?} but the sequence uses {:?}",
            p.prefix, prefix
        ));
    }
    if !p.suffix.is_empty() && p.suffix != suffix {
        return Err(format!(
            "{field} {raw:?} uses suffix {:?} but the sequence uses {:?}",
            p.suffix, suffix
        ));
    }
    Ok(p.value)
}

/// Audit `data` for gaps. See the parameter descriptions in the block
/// descriptor for the accepted values of each option.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    id_format: &str,
    separator: &str,
    step: i64,
    start: &str,
    end: &str,
    order: &str,
    duplicates: bool,
    output: &str,
    limit: usize,
) -> Result<String, String> {
    let id_format = IdFormat::parse(id_format)?;
    let sep = Sep::parse(separator)?;
    let order = Order::parse(order)?;
    let out = OutputFormat::parse(output)?;
    if step < 1 {
        return Err(format!(
            "step must be at least 1 (got {step}) — use 1 for a consecutive sequence, 2 for every other number"
        ));
    }
    if limit == 0 || limit > MAX_LIMIT {
        return Err(format!(
            "limit must be between 1 and {MAX_LIMIT} (got {limit})"
        ));
    }

    let tokens = split_tokens(data, sep);
    if tokens.is_empty() {
        return Err(
            "no values found — paste the sequence to audit, one number or ID per line".to_string(),
        );
    }
    if tokens.len() > MAX_VALUES {
        return Err(format!(
            "{} values exceeds the maximum of {MAX_VALUES} — split the sequence into smaller batches",
            tokens.len()
        ));
    }

    // Parse every entry, holding the first entry's prefix/suffix as the format
    // the whole sequence must share.
    let mut prefix = String::new();
    let mut suffix = String::new();
    let mut pad = 0usize;
    let mut values: Vec<i64> = Vec::with_capacity(tokens.len());
    for (i, tok) in tokens.iter().enumerate() {
        let p = parse_entry(tok)?;
        if id_format == IdFormat::Number && (!p.prefix.is_empty() || !p.suffix.is_empty()) {
            return Err(format!(
                "{tok:?} is not a plain number — switch the ID format to auto to audit prefixed IDs like INV-1004"
            ));
        }
        if i == 0 {
            prefix = p.prefix.clone();
            suffix = p.suffix.clone();
        } else if p.prefix != prefix || p.suffix != suffix {
            return Err(format!(
                "{tok:?} does not match the sequence format {:?} — every entry must share the same prefix and suffix",
                format!("{prefix}#{suffix}")
            ));
        }
        if p.digits.starts_with('0') && p.digits.len() > 1 {
            pad = pad.max(p.digits.len());
        }
        values.push(p.value);
    }

    let mut counts: BTreeMap<i64, usize> = BTreeMap::new();
    for v in &values {
        *counts.entry(*v).or_insert(0) += 1;
    }
    let observed_min = *counts.keys().next().unwrap();
    let observed_max = *counts.keys().next_back().unwrap();

    let start_v = if start.trim().is_empty() {
        observed_min
    } else {
        parse_bound(start.trim(), "start", &prefix, &suffix)?
    };
    let end_v = if end.trim().is_empty() {
        observed_max
    } else {
        parse_bound(end.trim(), "end", &prefix, &suffix)?
    };
    if end_v < start_v {
        return Err(format!(
            "expected end {} is before expected start {} — swap them",
            fmt_value(end_v, &prefix, &suffix, pad),
            fmt_value(start_v, &prefix, &suffix, pad)
        ));
    }
    let span = (end_v as i128 - start_v as i128) / step as i128 + 1;
    if span > MAX_SPAN as i128 {
        return Err(format!(
            "the expected sequence spans {span} values, which exceeds the maximum of {MAX_SPAN} — narrow the expected start/end or raise the step"
        ));
    }
    let expected = span as i64;

    // Classify the unique values.
    let mut present: Vec<i64> = Vec::new();
    let mut off_step: Vec<i64> = Vec::new();
    let mut out_of_range: Vec<i64> = Vec::new();
    for &v in counts.keys() {
        if v < start_v || v > end_v {
            out_of_range.push(v);
        } else if (v - start_v) % step != 0 {
            off_step.push(v);
        } else {
            present.push(v);
        }
    }

    // Walk the present values in order, collecting the gaps between them.
    let mut gaps: Vec<(i64, i64, i64)> = Vec::new();
    let mut cursor = start_v;
    for &p in &present {
        if p > cursor {
            gaps.push((cursor, p - step, (p - cursor) / step));
        }
        cursor = p + step;
    }
    if cursor <= end_v {
        gaps.push((cursor, end_v, (end_v - cursor) / step + 1));
    }
    let missing_count: i64 = gaps.iter().map(|g| g.2).sum();

    let dup_list: Vec<(i64, usize)> = counts
        .iter()
        .filter(|(_, &c)| c > 1)
        .map(|(&v, &c)| (v, c))
        .collect();

    let mut out_of_order: Vec<(i64, i64)> = Vec::new();
    if order == Order::Input {
        let mut prev = i64::MIN;
        for &v in &values {
            if prev != i64::MIN && v < prev {
                out_of_order.push((v, prev));
            }
            prev = prev.max(v);
        }
    }

    let f = |v: i64| fmt_value(v, &prefix, &suffix, pad);
    let gap_items: Vec<String> = gaps
        .iter()
        .map(|&(a, b, _)| fmt_range(a, b, &prefix, &suffix, pad))
        .collect();

    match out {
        OutputFormat::Missing => {
            if missing_count == 0 {
                return Ok("No missing values".to_string());
            }
            let mut lines: Vec<String> = Vec::new();
            let mut truncated = 0i64;
            'outer: for &(a, b, _) in &gaps {
                let mut v = a;
                while v <= b {
                    if lines.len() == limit {
                        truncated = missing_count - lines.len() as i64;
                        break 'outer;
                    }
                    lines.push(f(v));
                    v += step;
                }
            }
            if truncated > 0 {
                lines.push(format!(
                    "# {truncated} more missing values not shown — raise the limit to list them"
                ));
            }
            Ok(lines.join("\n"))
        }
        OutputFormat::Table => {
            let mut lines = vec!["gap_start\tgap_end\tcount".to_string()];
            for &(a, b, n) in gaps.iter().take(limit) {
                lines.push(format!("{}\t{}\t{}", f(a), f(b), n));
            }
            if gaps.len() > limit {
                lines.push(format!(
                    "# {} more gaps not shown — raise the limit to list them",
                    gaps.len() - limit
                ));
            }
            Ok(lines.join("\n"))
        }
        OutputFormat::Json => {
            let mut missing_vals: Vec<String> = Vec::new();
            'outer: for &(a, b, _) in &gaps {
                let mut v = a;
                while v <= b {
                    if missing_vals.len() == limit {
                        break 'outer;
                    }
                    missing_vals.push(f(v));
                    v += step;
                }
            }
            let truncated = missing_count > missing_vals.len() as i64;
            let gap_objs: Vec<String> = gaps
                .iter()
                .map(|&(a, b, n)| {
                    format!(
                        "    {{ \"start\": \"{}\", \"end\": \"{}\", \"count\": {} }}",
                        json_escape(&f(a)),
                        json_escape(&f(b)),
                        n
                    )
                })
                .collect();
            let dup_objs: Vec<String> = dup_list
                .iter()
                .map(|&(v, c)| {
                    format!(
                        "    {{ \"value\": \"{}\", \"count\": {} }}",
                        json_escape(&f(v)),
                        c
                    )
                })
                .collect();
            let ooo: Vec<String> = out_of_order
                .iter()
                .map(|&(v, prev)| format!("{} after {}", f(v), f(prev)))
                .collect();
            let arr = |objs: &[String]| {
                if objs.is_empty() {
                    "[]".to_string()
                } else {
                    format!("[\n{}\n  ]", objs.join(",\n"))
                }
            };
            Ok(format!(
                "{{\n  \"start\": \"{}\",\n  \"end\": \"{}\",\n  \"step\": {},\n  \"expected\": {},\n  \"present\": {},\n  \"missing_count\": {},\n  \"missing\": {},\n  \"gaps\": {},\n  \"duplicates\": {},\n  \"off_step\": {},\n  \"out_of_range\": {},\n  \"out_of_order\": {},\n  \"truncated\": {}\n}}",
                json_escape(&f(start_v)),
                json_escape(&f(end_v)),
                step,
                expected,
                present.len(),
                missing_count,
                json_str_array(&missing_vals, "  "),
                arr(&gap_objs),
                arr(&dup_objs),
                json_str_array(
                    &off_step.iter().map(|&v| f(v)).collect::<Vec<_>>(),
                    "  "
                ),
                json_str_array(
                    &out_of_range.iter().map(|&v| f(v)).collect::<Vec<_>>(),
                    "  "
                ),
                json_str_array(&ooo, "  "),
                truncated
            ))
        }
        OutputFormat::Report => {
            let mut lines = vec![
                format!("Range: {} to {} (step {step})", f(start_v), f(end_v)),
                format!("Present: {} of {expected} expected", present.len()),
                if missing_count == 0 {
                    "Missing: none".to_string()
                } else {
                    format!(
                        "Missing: {missing_count} ({})",
                        join_capped(&gap_items, limit)
                    )
                },
            ];
            if duplicates {
                if dup_list.is_empty() {
                    lines.push("Duplicates: none".to_string());
                } else {
                    let items: Vec<String> = dup_list
                        .iter()
                        .map(|&(v, c)| format!("{} x{c}", f(v)))
                        .collect();
                    lines.push(format!(
                        "Duplicates: {} ({})",
                        dup_list.len(),
                        join_capped(&items, limit)
                    ));
                }
            }
            if !off_step.is_empty() {
                let items: Vec<String> = off_step.iter().map(|&v| f(v)).collect();
                lines.push(format!(
                    "Off-step: {} ({})",
                    off_step.len(),
                    join_capped(&items, limit)
                ));
            }
            if !out_of_range.is_empty() {
                let items: Vec<String> = out_of_range.iter().map(|&v| f(v)).collect();
                lines.push(format!(
                    "Out of range: {} ({})",
                    out_of_range.len(),
                    join_capped(&items, limit)
                ));
            }
            if !out_of_order.is_empty() {
                let items: Vec<String> = out_of_order
                    .iter()
                    .map(|&(v, prev)| format!("{} after {}", f(v), f(prev)))
                    .collect();
                lines.push(format!(
                    "Out of order: {} ({})",
                    out_of_order.len(),
                    join_capped(&items, limit)
                ));
            }
            Ok(lines.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(data: &str) -> String {
        run(data, "auto", "auto", 1, "", "", "sorted", true, "report", 1000).unwrap()
    }

    #[test]
    fn reports_missing_values_and_compressed_ranges() {
        assert_eq!(
            report("1001\n1002\n1003\n1006\n1009"),
            "Range: 1001 to 1009 (step 1)\n\
             Present: 5 of 9 expected\n\
             Missing: 4 (1004-1005, 1007-1008)\n\
             Duplicates: none"
        );
    }

    #[test]
    fn reports_duplicates_alongside_gaps() {
        assert_eq!(
            report("5, 6, 6, 8"),
            "Range: 5 to 8 (step 1)\n\
             Present: 3 of 4 expected\n\
             Missing: 1 (7)\n\
             Duplicates: 1 (6 x2)"
        );
    }

    #[test]
    fn no_gaps_reads_clean() {
        assert_eq!(
            report("1\n2\n3"),
            "Range: 1 to 3 (step 1)\nPresent: 3 of 3 expected\nMissing: none\nDuplicates: none"
        );
    }

    #[test]
    fn prefixed_ids_keep_prefix_and_zero_padding() {
        assert_eq!(
            report("INV-0001\nINV-0002\nINV-0005"),
            "Range: INV-0001 to INV-0005 (step 1)\n\
             Present: 3 of 5 expected\n\
             Missing: 2 (INV-0003 to INV-0004)\n\
             Duplicates: none"
        );
    }

    #[test]
    fn year_prefixed_and_suffixed_ids_are_supported() {
        // The LAST digit run is the counter, so a year prefix stays put.
        let out = report("2026-005\n2026-007");
        assert!(out.starts_with("Range: 2026-005 to 2026-007 (step 1)"), "{out}");
        assert!(out.contains("Missing: 1 (2026-006)"), "{out}");
        let suffixed = run(
            "1001 (paid)\n1003 (paid)",
            "auto",
            "newline",
            1,
            "",
            "",
            "sorted",
            true,
            "report",
            1000,
        )
        .unwrap();
        assert!(suffixed.contains("Missing: 1 (1002 (paid))"), "{suffixed}");
    }

    #[test]
    fn explicit_start_and_end_extend_the_expected_range() {
        let out = run(
            "1002\n1003", "auto", "auto", 1, "1000", "1005", "sorted", true, "report", 1000,
        )
        .unwrap();
        assert_eq!(
            out,
            "Range: 1000 to 1005 (step 1)\n\
             Present: 2 of 6 expected\n\
             Missing: 4 (1000-1001, 1004-1005)\n\
             Duplicates: none"
        );
    }

    #[test]
    fn values_outside_an_explicit_range_are_flagged() {
        let out = run(
            "8\n9\n12", "auto", "auto", 1, "8", "10", "sorted", true, "report", 1000,
        )
        .unwrap();
        assert!(out.contains("Out of range: 1 (12)"), "{out}");
        assert!(out.contains("Missing: 1 (10)"), "{out}");
    }

    #[test]
    fn step_two_finds_gaps_and_off_step_entries() {
        let out = run(
            "2\n4\n5\n10", "auto", "auto", 2, "", "", "sorted", true, "report", 1000,
        )
        .unwrap();
        assert_eq!(
            out,
            "Range: 2 to 10 (step 2)\n\
             Present: 3 of 5 expected\n\
             Missing: 2 (6-8)\n\
             Duplicates: none\n\
             Off-step: 1 (5)"
        );
    }

    #[test]
    fn input_order_mode_flags_entries_that_go_backwards() {
        let out = run(
            "1\n3\n2\n4", "auto", "auto", 1, "", "", "input", true, "report", 1000,
        )
        .unwrap();
        assert!(out.contains("Out of order: 1 (2 after 3)"), "{out}");
        assert!(out.contains("Missing: none"), "{out}");
    }

    #[test]
    fn duplicates_line_can_be_switched_off() {
        let out = run(
            "5\n5\n7", "auto", "auto", 1, "", "", "sorted", false, "report", 1000,
        )
        .unwrap();
        assert!(!out.contains("Duplicates"), "{out}");
    }

    #[test]
    fn missing_mode_expands_every_value() {
        let out = run(
            "10\n14", "auto", "auto", 1, "", "", "sorted", true, "missing", 1000,
        )
        .unwrap();
        assert_eq!(out, "11\n12\n13");
        let clean = run(
            "10\n11", "auto", "auto", 1, "", "", "sorted", true, "missing", 1000,
        )
        .unwrap();
        assert_eq!(clean, "No missing values");
    }

    #[test]
    fn missing_mode_respects_the_limit_and_says_so() {
        let out = run(
            "1\n100", "auto", "auto", 1, "", "", "sorted", true, "missing", 3,
        )
        .unwrap();
        assert_eq!(
            out,
            "2\n3\n4\n# 95 more missing values not shown — raise the limit to list them"
        );
    }

    #[test]
    fn report_mode_caps_the_listed_gaps() {
        let out = run(
            "1\n3\n5\n7\n9", "auto", "auto", 1, "", "", "sorted", true, "report", 2,
        )
        .unwrap();
        assert!(out.contains("Missing: 4 (2, 4, +2 more)"), "{out}");
    }

    #[test]
    fn table_mode_is_tab_separated() {
        let out = run(
            "1001\n1002\n1006", "auto", "auto", 1, "", "", "sorted", true, "table", 1000,
        )
        .unwrap();
        assert_eq!(out, "gap_start\tgap_end\tcount\n1003\t1005\t3");
    }

    #[test]
    fn json_mode_reports_counts_gaps_and_duplicates() {
        let out = run(
            "1001\n1001\n1004", "auto", "auto", 1, "", "", "sorted", true, "json", 1000,
        )
        .unwrap();
        assert!(out.contains("\"missing_count\": 2"), "{out}");
        assert!(out.contains("\"missing\": [\n    \"1002\",\n    \"1003\"\n  ]"), "{out}");
        assert!(
            out.contains("{ \"start\": \"1002\", \"end\": \"1003\", \"count\": 2 }"),
            "{out}"
        );
        assert!(
            out.contains("{ \"value\": \"1001\", \"count\": 2 }"),
            "{out}"
        );
        assert!(out.contains("\"truncated\": false"), "{out}");
    }

    #[test]
    fn negative_numbers_use_a_readable_range_form() {
        let out = report("-5\n-1");
        assert!(out.contains("Missing: 3 (-4 to -2)"), "{out}");
    }

    #[test]
    fn separators_and_id_format_are_honoured() {
        assert!(report("7;8;10").contains("Missing: 1 (9)"));
        assert!(run("3|5", "auto", "pipe", 1, "", "", "sorted", true, "report", 1000)
            .unwrap()
            .contains("Missing: 1 (4)"));
        let err = run(
            "INV-1\nINV-2", "number", "auto", 1, "", "", "sorted", true, "report", 1000,
        )
        .unwrap_err();
        assert!(err.contains("not a plain number"), "{err}");
    }

    #[test]
    fn rejects_entries_that_are_not_numbers() {
        let err = report_err("1001\nn/a\n1003");
        assert!(err.contains("has no number in it"), "{err}");
    }

    #[test]
    fn rejects_mixed_id_formats() {
        let err = report_err("INV-1\nORD-2");
        assert!(err.contains("must share the same prefix and suffix"), "{err}");
    }

    #[test]
    fn rejects_empty_input_and_bad_options() {
        assert!(report_err("   ").contains("no values found"));
        let err = run("1\n2", "auto", "auto", 0, "", "", "sorted", true, "report", 1000).unwrap_err();
        assert!(err.contains("step must be at least 1"), "{err}");
        let err = run("1\n2", "auto", "auto", 1, "9", "3", "sorted", true, "report", 1000)
            .unwrap_err();
        assert!(err.contains("is before expected start"), "{err}");
        let err = run("1\n2", "auto", "auto", 1, "", "", "sorted", true, "chart", 1000)
            .unwrap_err();
        assert!(err.contains("unknown output"), "{err}");
        let err = run("1\n2", "auto", "auto", 1, "", "", "sorted", true, "report", 0).unwrap_err();
        assert!(err.contains("limit must be between"), "{err}");
    }

    #[test]
    fn rejects_an_absurd_expected_span() {
        let err = run(
            "1\n2", "auto", "auto", 1, "1", "9000000", "sorted", true, "report", 1000,
        )
        .unwrap_err();
        assert!(err.contains("exceeds the maximum"), "{err}");
    }

    #[test]
    fn rejects_more_values_than_the_cap() {
        let big: Vec<String> = (1..=MAX_VALUES + 1).map(|n| n.to_string()).collect();
        let err = report_err(&big.join("\n"));
        assert!(err.contains("exceeds the maximum"), "{err}");
        let at_cap: Vec<String> = (1..=MAX_VALUES).map(|n| n.to_string()).collect();
        assert!(report(&at_cap.join("\n")).contains("Missing: none"));
    }

    fn report_err(data: &str) -> String {
        run(data, "auto", "auto", 1, "", "", "sorted", true, "report", 1000).unwrap_err()
    }
}
