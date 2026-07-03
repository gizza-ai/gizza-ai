//! gizza-ai/cartesian-product core — generate every combination (tuple) across
//! two or more input lists (sizes × colors × materials). Pure string
//! processing, no deps. Items are trimmed and empties dropped; the rightmost
//! list varies fastest (odometer order, like itertools.product). A
//! `max_combinations` cap guards against exploding output.

/// Hard ceiling for `max_combinations` (the descriptor also enforces it).
pub const MAX_COMBINATIONS_CAP: u64 = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemSep {
    Auto,
    Comma,
    Newline,
    Semicolon,
    Pipe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinSep {
    Space,
    None,
    Comma,
    Dash,
    Underscore,
    Pipe,
    Tab,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutFormat {
    Lines,
    Csv,
    Json,
}

pub fn parse_item_separator(s: &str) -> Result<ItemSep, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(ItemSep::Auto),
        "comma" => Ok(ItemSep::Comma),
        "newline" | "lines" | "line" => Ok(ItemSep::Newline),
        "semicolon" => Ok(ItemSep::Semicolon),
        "pipe" => Ok(ItemSep::Pipe),
        other => Err(format!(
            "item_separator {other:?} not supported (auto|comma|newline|semicolon|pipe)"
        )),
    }
}

pub fn parse_join_separator(s: &str) -> Result<JoinSep, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "space" => Ok(JoinSep::Space),
        "none" => Ok(JoinSep::None),
        "comma" => Ok(JoinSep::Comma),
        "dash" | "hyphen" => Ok(JoinSep::Dash),
        "underscore" => Ok(JoinSep::Underscore),
        "pipe" => Ok(JoinSep::Pipe),
        "tab" => Ok(JoinSep::Tab),
        "custom" => Ok(JoinSep::Custom),
        other => Err(format!(
            "join_separator {other:?} not supported (space|none|comma|dash|underscore|pipe|tab|custom)"
        )),
    }
}

pub fn parse_output_format(s: &str) -> Result<OutFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "lines" | "text" => Ok(OutFormat::Lines),
        "csv" => Ok(OutFormat::Csv),
        "json" => Ok(OutFormat::Json),
        other => Err(format!(
            "output_format {other:?} not supported (lines|csv|json)"
        )),
    }
}

fn split_items(input: &str, sep: ItemSep) -> Vec<String> {
    let effective = match sep {
        ItemSep::Auto => {
            if input.contains('\n') {
                ItemSep::Newline
            } else if input.contains(',') {
                ItemSep::Comma
            } else if input.contains(';') {
                ItemSep::Semicolon
            } else if input.contains('|') {
                ItemSep::Pipe
            } else {
                ItemSep::Newline
            }
        }
        other => other,
    };
    let raw: Vec<&str> = match effective {
        ItemSep::Comma => input.split(',').collect(),
        ItemSep::Newline => input.split('\n').collect(),
        ItemSep::Semicolon => input.split(';').collect(),
        ItemSep::Pipe => input.split('|').collect(),
        ItemSep::Auto => unreachable!(),
    };
    raw.into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
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
    out.push('"');
    out
}

/// Generate the cartesian product of up to four lists.
///
/// `raw_lists` holds the raw field texts in order (list1..list4). The first
/// two are required — an empty one is an error; later lists that are blank
/// (or parse to zero items) are ignored. The rightmost non-empty list varies
/// fastest. Errors (never truncates) when the total combination count exceeds
/// `max_combinations`.
#[allow(clippy::too_many_arguments)]
pub fn generate(
    raw_lists: &[&str],
    item_sep: ItemSep,
    dedupe: bool,
    out_format: OutFormat,
    join_sep: JoinSep,
    custom_join: &str,
    prefix: &str,
    suffix: &str,
    max_combinations: u64,
) -> Result<String, String> {
    if max_combinations < 1 || max_combinations > MAX_COMBINATIONS_CAP {
        return Err(format!(
            "max_combinations must be between 1 and {MAX_COMBINATIONS_CAP}, got {max_combinations}"
        ));
    }

    let mut lists: Vec<Vec<String>> = Vec::new();
    for (i, raw) in raw_lists.iter().enumerate() {
        let mut items = split_items(raw, item_sep);
        if dedupe {
            let mut seen = std::collections::HashSet::new();
            items.retain(|it| seen.insert(it.clone()));
        }
        if items.is_empty() {
            if i < 2 {
                return Err(format!(
                    "list{} has no items — enter at least one item per required list (items are split by {} and blanks are dropped)",
                    i + 1,
                    match item_sep {
                        ItemSep::Auto => "newline/comma/semicolon/pipe (auto-detected)",
                        ItemSep::Comma => "commas",
                        ItemSep::Newline => "newlines",
                        ItemSep::Semicolon => "semicolons",
                        ItemSep::Pipe => "pipes",
                    }
                ));
            }
            continue; // optional list left blank — ignore it
        }
        lists.push(items);
    }

    let mut total: u64 = 1;
    for l in &lists {
        total = total.checked_mul(l.len() as u64).ok_or_else(|| {
            "combination count overflows a 64-bit counter — shrink the lists".to_string()
        })?;
    }
    if total > max_combinations {
        let sizes = lists
            .iter()
            .map(|l| l.len().to_string())
            .collect::<Vec<_>>()
            .join(" x ");
        return Err(format!(
            "cartesian product of lists sized {sizes} would produce {total} combinations, more than max_combinations={max_combinations} (hard cap {MAX_COMBINATIONS_CAP}) — shrink the lists or raise max_combinations"
        ));
    }

    let join = match join_sep {
        JoinSep::Space => " ",
        JoinSep::None => "",
        JoinSep::Comma => ", ",
        JoinSep::Dash => "-",
        JoinSep::Underscore => "_",
        JoinSep::Pipe => "|",
        JoinSep::Tab => "\t",
        JoinSep::Custom => custom_join,
    };

    let n = lists.len();
    let total = total as usize;
    let mut idx = vec![0usize; n];
    let mut rows: Vec<String> = Vec::with_capacity(total);
    for _ in 0..total {
        let tuple: Vec<&str> = (0..n).map(|i| lists[i][idx[i]].as_str()).collect();
        rows.push(match out_format {
            OutFormat::Lines => format!("{prefix}{}{suffix}", tuple.join(join)),
            OutFormat::Csv => tuple
                .iter()
                .map(|c| csv_cell(c))
                .collect::<Vec<_>>()
                .join(","),
            OutFormat::Json => format!(
                "  [{}]",
                tuple.iter().map(|c| json_string(c)).collect::<Vec<_>>().join(", ")
            ),
        });
        // odometer increment: rightmost list varies fastest
        for i in (0..n).rev() {
            idx[i] += 1;
            if idx[i] < lists[i].len() {
                break;
            }
            idx[i] = 0;
        }
    }

    Ok(match out_format {
        OutFormat::Lines | OutFormat::Csv => rows.join("\n"),
        OutFormat::Json => format!("[\n{}\n]", rows.join(",\n")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple(raw: &[&str]) -> Result<String, String> {
        generate(
            raw,
            ItemSep::Auto,
            false,
            OutFormat::Lines,
            JoinSep::Space,
            "",
            "",
            "",
            10_000,
        )
    }

    #[test]
    fn two_lists_odometer_order() {
        let out = simple(&["red, blue", "S, M, L"]).unwrap();
        assert_eq!(out, "red S\nred M\nred L\nblue S\nblue M\nblue L");
    }

    #[test]
    fn three_lists_with_blank_optional() {
        let out = simple(&["red", "S, M", "cotton, linen", ""]).unwrap();
        assert_eq!(
            out,
            "red S cotton\nred S linen\nred M cotton\nred M linen"
        );
    }

    #[test]
    fn join_separators_and_prefix_suffix() {
        let out = generate(
            &["red,blue", "S,M"],
            ItemSep::Comma,
            false,
            OutFormat::Lines,
            JoinSep::Dash,
            "",
            "sku-",
            "-2026",
            10_000,
        )
        .unwrap();
        assert_eq!(
            out,
            "sku-red-S-2026\nsku-red-M-2026\nsku-blue-S-2026\nsku-blue-M-2026"
        );

        let out = generate(
            &["a,b", "1,2"],
            ItemSep::Comma,
            false,
            OutFormat::Lines,
            JoinSep::Custom,
            " :: ",
            "",
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(out, "a :: 1\na :: 2\nb :: 1\nb :: 2");
    }

    #[test]
    fn csv_output_quotes_when_needed() {
        let out = generate(
            &["navy blue, ruby\"red", "S"],
            ItemSep::Comma,
            false,
            OutFormat::Csv,
            JoinSep::Space,
            "",
            "",
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(out, "navy blue,S\n\"ruby\"\"red\",S");
    }

    #[test]
    fn json_output_escapes() {
        let out = generate(
            &["a\"b", "1"],
            ItemSep::Newline,
            false,
            OutFormat::Json,
            JoinSep::Space,
            "",
            "",
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(out, "[\n  [\"a\\\"b\", \"1\"]\n]");
    }

    #[test]
    fn dedupe_within_each_list() {
        let out = generate(
            &["red, red, blue", "S, S"],
            ItemSep::Comma,
            true,
            OutFormat::Lines,
            JoinSep::Space,
            "",
            "",
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(out, "red S\nblue S");
    }

    #[test]
    fn empty_required_list_errors() {
        let err = simple(&["", "S, M"]).unwrap_err();
        assert!(err.contains("list1 has no items"), "got: {err}");
        let err = simple(&["red", "  ,, "]).unwrap_err();
        assert!(err.contains("list2 has no items"), "got: {err}");
    }

    #[test]
    fn cap_exceeded_errors_with_count() {
        let list = (0..101).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let err = generate(
            &[&list, &list],
            ItemSep::Comma,
            false,
            OutFormat::Lines,
            JoinSep::Space,
            "",
            "",
            "",
            10_000,
        )
        .unwrap_err();
        assert!(err.contains("10201 combinations"), "got: {err}");
        assert!(err.contains("max_combinations=10000"), "got: {err}");
    }

    #[test]
    fn bad_cap_and_bad_enum_values_error() {
        let err = generate(
            &["a", "b"],
            ItemSep::Auto,
            false,
            OutFormat::Lines,
            JoinSep::Space,
            "",
            "",
            "",
            0,
        )
        .unwrap_err();
        assert!(
            err.contains("max_combinations must be between 1 and 100000"),
            "got: {err}"
        );
        assert!(parse_item_separator("nope").is_err());
        assert!(parse_join_separator("nope").is_err());
        assert!(parse_output_format("nope").is_err());
    }
}
