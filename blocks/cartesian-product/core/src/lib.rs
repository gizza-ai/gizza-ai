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
    /// Tab ALSO splits on line breaks: tab-separated data is row-based
    /// (spreadsheet cells), so a trailing/embedded newline must never stay
    /// inside an item — and a 2-D spreadsheet paste (tabs + newlines) yields
    /// every cell.
    Tab,
    /// Never auto-detected: slashes are too common inside legitimate items
    /// (URLs, dates, paths) to guess from content.
    Slash,
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
    /// In the enum (unlike "+" etc., which `Custom` covers) because a newline
    /// cannot be typed into the single-line custom-join field.
    Newline,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutFormat {
    Lines,
    Csv,
    Json,
    /// Count only ("2 x 3 = 6") — never generates, so it is exempt from
    /// `max_combinations` (checking whether a product is too big is the point).
    Count,
}

pub fn parse_item_separator(s: &str) -> Result<ItemSep, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(ItemSep::Auto),
        "comma" => Ok(ItemSep::Comma),
        "newline" | "lines" | "line" => Ok(ItemSep::Newline),
        "semicolon" => Ok(ItemSep::Semicolon),
        "pipe" => Ok(ItemSep::Pipe),
        "tab" | "tabs" | "tsv" => Ok(ItemSep::Tab),
        "slash" | "/" => Ok(ItemSep::Slash),
        other => Err(format!(
            "item_separator {other:?} not supported (auto|comma|newline|semicolon|pipe|tab|slash)"
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
        "newline" | "line" => Ok(JoinSep::Newline),
        "custom" => Ok(JoinSep::Custom),
        other => Err(format!(
            "join_separator {other:?} not supported (space|none|comma|dash|underscore|pipe|tab|newline|custom)"
        )),
    }
}

pub fn parse_output_format(s: &str) -> Result<OutFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "lines" | "text" => Ok(OutFormat::Lines),
        "csv" => Ok(OutFormat::Csv),
        "json" => Ok(OutFormat::Json),
        "count" => Ok(OutFormat::Count),
        other => Err(format!(
            "output_format {other:?} not supported (lines|csv|json|count)"
        )),
    }
}

fn split_items(input: &str, sep: ItemSep) -> Vec<String> {
    let effective = match sep {
        ItemSep::Auto => {
            // Tab before newline so a 2-D spreadsheet paste (tabs AND
            // newlines) yields every cell — Tab mode splits on both. Slash is
            // never auto-detected (URLs/dates/paths contain slashes).
            if input.contains('\t') {
                ItemSep::Tab
            } else if input.contains('\n') {
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
        ItemSep::Tab => input.split(['\t', '\n']).collect(),
        ItemSep::Slash => input.split('/').collect(),
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
                        ItemSep::Auto => "tab/newline/comma/semicolon/pipe (auto-detected)",
                        ItemSep::Comma => "commas",
                        ItemSep::Newline => "newlines",
                        ItemSep::Semicolon => "semicolons",
                        ItemSep::Pipe => "pipes",
                        ItemSep::Tab => "tabs or newlines",
                        ItemSep::Slash => "slashes",
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
    // Count-only output: report "sizes = total" without generating, so it is
    // exempt from max_combinations (checking whether a product would be too
    // big is exactly what the count is for).
    if out_format == OutFormat::Count {
        let sizes = lists
            .iter()
            .map(|l| l.len().to_string())
            .collect::<Vec<_>>()
            .join(" x ");
        return Ok(format!("{sizes} = {total}"));
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
        JoinSep::Newline => "\n",
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
            OutFormat::Count => unreachable!("count returns before generation"),
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
        OutFormat::Count => unreachable!("count returns before generation"),
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
    fn tab_separator_splits_tabs_and_newlines() {
        // Explicit tab mode: a pasted spreadsheet ROW ("red\tblue\n") and a
        // 2-D region (tabs + newlines) both yield every cell.
        let out = generate(
            &["red\tblue\ngreen\tteal", "S\tM"],
            ItemSep::Tab,
            false,
            OutFormat::Lines,
            JoinSep::Space,
            "",
            "",
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(
            out,
            "red S\nred M\nblue S\nblue M\ngreen S\ngreen M\nteal S\nteal M"
        );
    }

    #[test]
    fn auto_detects_tab_before_newline() {
        // A 2-D spreadsheet paste contains tabs AND newlines — auto must pick
        // tab (which also splits newlines), not leave tabs inside items.
        let out = simple(&["red\tblue\ngreen\tteal", "S"]).unwrap();
        assert_eq!(out, "red S\nblue S\ngreen S\nteal S");
    }

    #[test]
    fn slash_separator_is_explicit_only() {
        let out = generate(
            &["a/b", "1/2"],
            ItemSep::Slash,
            false,
            OutFormat::Lines,
            JoinSep::Space,
            "",
            "",
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(out, "a 1\na 2\nb 1\nb 2");
        // Auto never picks slash: "a/b" stays ONE item (URLs/dates/paths).
        let out = simple(&["a/b", "1"]).unwrap();
        assert_eq!(out, "a/b 1");
    }

    #[test]
    fn newline_join_separator() {
        let out = generate(
            &["a,b", "1"],
            ItemSep::Comma,
            false,
            OutFormat::Lines,
            JoinSep::Newline,
            "",
            "",
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(out, "a\n1\nb\n1");
    }

    #[test]
    fn count_format_reports_sizes_and_total_without_generating() {
        let out = generate(
            &["red, blue", "S, M, L", "cotton, linen"],
            ItemSep::Auto,
            false,
            OutFormat::Count,
            JoinSep::Space,
            "",
            "",
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(out, "2 x 3 x 2 = 12");
    }

    #[test]
    fn count_format_is_exempt_from_the_cap() {
        // 101 x 101 = 10201 > cap 10000 — generation errors, counting doesn't.
        let list = (0..101).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let out = generate(
            &[&list, &list],
            ItemSep::Comma,
            false,
            OutFormat::Count,
            JoinSep::Space,
            "",
            "",
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(out, "101 x 101 = 10201");
    }

    #[test]
    fn count_format_respects_dedupe_and_required_lists() {
        let out = generate(
            &["red, red, blue", "S"],
            ItemSep::Comma,
            true,
            OutFormat::Count,
            JoinSep::Space,
            "",
            "",
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(out, "2 x 1 = 2");
        let err = generate(
            &["", "S"],
            ItemSep::Auto,
            false,
            OutFormat::Count,
            JoinSep::Space,
            "",
            "",
            "",
            10_000,
        )
        .unwrap_err();
        assert!(err.contains("list1 has no items"), "got: {err}");
    }

    #[test]
    fn digits_only_items_stay_verbatim_strings() {
        // "007" and "1.50" must never be numerically mangled (007 → 7).
        let out = generate(
            &["007, 08", "1.50, 2.0"],
            ItemSep::Comma,
            false,
            OutFormat::Lines,
            JoinSep::Dash,
            "",
            "",
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(out, "007-1.50\n007-2.0\n08-1.50\n08-2.0");
    }

    #[test]
    fn cap_boundary_exactly_at_passes_one_over_errors() {
        // 100 x 100 = 10000 == cap → generates; 10001 (via a 10001-cap check
        // inverse: 100 x 100 with cap 9999) → errors. Exact boundary, both sides.
        let l100 = (0..100).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let out = generate(
            &[&l100, &l100],
            ItemSep::Comma,
            false,
            OutFormat::Lines,
            JoinSep::Space,
            "",
            "",
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(out.lines().count(), 10_000);
        let err = generate(
            &[&l100, &l100],
            ItemSep::Comma,
            false,
            OutFormat::Lines,
            JoinSep::Space,
            "",
            "",
            "",
            9_999,
        )
        .unwrap_err();
        assert!(err.contains("10000 combinations"), "got: {err}");
        assert!(err.contains("max_combinations=9999"), "got: {err}");
    }

    #[test]
    fn whitespace_only_items_are_dropped() {
        let out = simple(&["red, ,  , blue", "S"]).unwrap();
        assert_eq!(out, "red S\nblue S");
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
