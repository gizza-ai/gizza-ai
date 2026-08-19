//! one-hot-encoder core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! One-hot (dummy variable) encoding: expand one categorical CSV column into a block of binary
//! indicator columns, one per distinct value, where each row carries a 1 in the column matching
//! its category and a 0 everywhere else. This is the encoding `pandas.get_dummies`,
//! scikit-learn's `OneHotEncoder` and feature-engine's `OneHotEncoder` all produce; the knobs
//! here mirror theirs — a column-name prefix and separator, dropping a reference level to avoid
//! the dummy-variable trap, capping the expansion to the top-N most frequent categories or to
//! values seen at least N times, and choosing what a blank cell means.

use std::collections::HashMap;

/// Hard cap on generated indicator columns. A high-cardinality column (an ID, an email, a
/// free-text note) would otherwise expand into thousands of columns and produce an unusable CSV;
/// `max_categories` / `min_count` are the intended way to stay under it.
pub const MAX_COLUMNS: usize = 512;

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

/// Resolve a column spec (a header name, or a 1-based index) to a 0-based column index.
/// If a header row is present its names win over index parsing (so a numeric column name
/// like "2020" is matched as a name, not treated as position 2020).
fn resolve_col(
    spec: &str,
    header: Option<&csv::StringRecord>,
    width: usize,
) -> Result<usize, String> {
    let s = spec.trim();
    if s.is_empty() {
        return Err("column selector is empty".into());
    }
    if let Some(h) = header {
        if let Some(i) = h.iter().position(|c| c.trim() == s) {
            return Ok(i);
        }
    }
    match s.parse::<usize>() {
        Ok(n) if n >= 1 && n <= width => Ok(n - 1),
        Ok(_) => Err(format!(
            "column '{s}' is out of range (the CSV has {width} column(s))"
        )),
        Err(_) => Err(format!(
            "column '{s}' not found — pass a header name or a 1-based column number"
        )),
    }
}

/// One distinct category found in the source column.
struct Cat {
    /// Grouping key (trimmed, and lower-cased when case-insensitive).
    key: String,
    /// Display text used in the generated column name — the first spelling seen.
    label: String,
    count: usize,
    /// Position of first appearance, used as a stable tie-break when sorting.
    seen: usize,
}

/// One-hot encode one categorical column of a CSV.
///
/// - `column`: a header name, or a 1-based column index.
/// - `prefix`: column-name prefix for the generated columns; blank uses the source column's name.
/// - `separator`: text between the prefix and the category value (`_` gives `city_Paris`).
/// - `drop`: which reference level to omit — `"none"`, `"first"`, `"last"`, or `"if-binary"`
///   (drop the first level only when the column has exactly two categories).
/// - `drop_original`: remove the source column from the output.
/// - `missing`: what a blank cell means — `"zeros"` (0 in every indicator), `"separate"` (its own
///   indicator column), `"blank"` (empty indicator cells), or `"error"`.
/// - `max_categories`: keep only the N most frequent categories (0 = keep all).
/// - `min_count`: keep only categories occurring at least this many times (0/1 = keep all).
/// - `other_column`: add one combined indicator for the categories excluded by the two limits.
/// - `positive` / `negative`: the text written for a match / non-match (default `1` / `0`).
/// - `case_sensitive`: when false, values differing only in case share one column.
/// - `sort`: column order — `"alphabetical"`, `"frequency"` (most common first), or `"first-seen"`.
#[allow(clippy::too_many_arguments)]
pub fn encode(
    data: &str,
    column: &str,
    prefix: &str,
    separator: &str,
    drop: &str,
    drop_original: bool,
    missing: &str,
    max_categories: usize,
    min_count: usize,
    other_column: bool,
    positive: &str,
    negative: &str,
    case_sensitive: bool,
    sort: &str,
    has_header: bool,
    delimiter: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let drop = if drop.is_empty() { "none" } else { drop };
    match drop {
        "none" | "first" | "last" | "if-binary" => {}
        other => {
            return Err(format!(
                "drop must be 'none', 'first', 'last', or 'if-binary', got '{other}'"
            ))
        }
    }
    let missing = if missing.is_empty() { "zeros" } else { missing };
    match missing {
        "zeros" | "separate" | "blank" | "error" => {}
        other => {
            return Err(format!(
                "missing must be 'zeros', 'separate', 'blank', or 'error', got '{other}'"
            ))
        }
    }
    let sort = if sort.is_empty() { "alphabetical" } else { sort };
    match sort {
        "alphabetical" | "frequency" | "first-seen" => {}
        other => {
            return Err(format!(
                "sort must be 'alphabetical', 'frequency', or 'first-seen', got '{other}'"
            ))
        }
    }

    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr
        .records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Err("no rows found".into());
    }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);

    let header = if has_header { records.first() } else { None };
    let col_idx = resolve_col(column, header, width)?;
    let col_name = match header {
        Some(h) => h.get(col_idx).unwrap_or("").trim().to_string(),
        None => format!("col{}", col_idx + 1),
    };
    let data_start = if has_header { 1 } else { 0 };
    if records.len() <= data_start {
        return Err("no data rows found — the CSV has only a header row".into());
    }

    // Grouping key for one cell: trimmed, and case-folded when case_sensitive is off.
    let key_of = |cell: &str| -> String {
        let t = cell.trim();
        if case_sensitive {
            t.to_string()
        } else {
            t.to_lowercase()
        }
    };

    // Pass 1: collect the distinct categories in first-seen order, plus how many rows are blank.
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut cats: Vec<Cat> = Vec::new();
    let mut blank_rows = 0usize;
    for rec in &records[data_start..] {
        let cell = rec.get(col_idx).unwrap_or("");
        if cell.trim().is_empty() {
            blank_rows += 1;
            continue;
        }
        let key = key_of(cell);
        match index.get(&key) {
            Some(&i) => cats[i].count += 1,
            None => {
                let seen = cats.len();
                index.insert(key.clone(), seen);
                cats.push(Cat {
                    key,
                    label: cell.trim().to_string(),
                    count: 1,
                    seen,
                });
            }
        }
    }
    if missing == "error" && blank_rows > 0 {
        return Err(format!(
            "found {blank_rows} blank cell(s) in column '{col_name}' — set 'missing' to zeros, separate, or blank to allow them"
        ));
    }
    if cats.is_empty() {
        return Err(format!(
            "column '{col_name}' has no non-blank values to encode"
        ));
    }

    // Selection: a category must clear `min_count`, and then survive the top-N cut. The top-N cut
    // is always by frequency (that is what "top categories" means); `sort` only decides the order
    // the surviving columns are written in.
    let floor = min_count.max(1);
    let mut kept: Vec<usize> = (0..cats.len()).filter(|&i| cats[i].count >= floor).collect();
    let mut excluded: Vec<usize> = (0..cats.len()).filter(|&i| cats[i].count < floor).collect();
    if max_categories > 0 && kept.len() > max_categories {
        kept.sort_by(|&a, &b| {
            cats[b]
                .count
                .cmp(&cats[a].count)
                .then(cats[a].seen.cmp(&cats[b].seen))
        });
        excluded.extend(kept.split_off(max_categories));
    }
    if kept.is_empty() {
        return Err(
            "no categories survived the min_count / max_categories limits — lower them to keep at least one column".into(),
        );
    }

    match sort {
        "frequency" => kept.sort_by(|&a, &b| {
            cats[b]
                .count
                .cmp(&cats[a].count)
                .then(cats[a].seen.cmp(&cats[b].seen))
        }),
        "first-seen" => kept.sort_by_key(|&i| cats[i].seen),
        // "alphabetical": by grouping key so case-insensitive runs order predictably.
        _ => kept.sort_by(|&a, &b| cats[a].key.cmp(&cats[b].key).then(cats[a].seen.cmp(&cats[b].seen))),
    }

    // Drop a reference level to avoid the dummy-variable trap. This applies to the category
    // columns only — the "other" and blank buckets are separate and are never the dropped level.
    match drop {
        "first" => {
            kept.remove(0);
        }
        "last" => {
            kept.pop();
        }
        "if-binary" if kept.len() == 2 => {
            kept.remove(0);
        }
        _ => {}
    }

    let excluded_keys: Vec<&str> = excluded.iter().map(|&i| cats[i].key.as_str()).collect();
    let want_other = other_column && !excluded_keys.is_empty();
    let want_missing = missing == "separate" && blank_rows > 0;

    let total_cols = kept.len() + usize::from(want_other) + usize::from(want_missing);
    if total_cols > MAX_COLUMNS {
        return Err(format!(
            "one-hot encoding column '{col_name}' would add {total_cols} columns (limit {MAX_COLUMNS}) — use max_categories to keep only the most frequent values, or min_count to drop rare ones"
        ));
    }

    let base = if prefix.trim().is_empty() {
        col_name.as_str()
    } else {
        prefix.trim()
    };
    let name_of = |label: &str| format!("{base}{separator}{label}");

    // Pass 2: emit rows.
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);
    for (i, rec) in records.iter().enumerate() {
        let mut fields: Vec<String> = rec.iter().map(|s| s.to_string()).collect();
        while fields.len() <= col_idx {
            fields.push(String::new());
        }

        if has_header && i == 0 {
            for &c in &kept {
                fields.push(name_of(&cats[c].label));
            }
            if want_other {
                fields.push(name_of("other"));
            }
            if want_missing {
                fields.push(name_of("NaN"));
            }
        } else {
            let cell = rec.get(col_idx).unwrap_or("");
            if cell.trim().is_empty() {
                // A blank cell matches no category; `missing` decides what the row carries.
                let fill = if missing == "blank" { "" } else { negative };
                for _ in 0..kept.len() + usize::from(want_other) {
                    fields.push(fill.to_string());
                }
                if want_missing {
                    fields.push(positive.to_string());
                }
            } else {
                let key = key_of(cell);
                for &c in &kept {
                    fields.push(if cats[c].key == key { positive } else { negative }.to_string());
                }
                if want_other {
                    fields.push(
                        if excluded_keys.contains(&key.as_str()) {
                            positive
                        } else {
                            negative
                        }
                        .to_string(),
                    );
                }
                if want_missing {
                    fields.push(negative.to_string());
                }
            }
        }

        if drop_original {
            fields.remove(col_idx);
        }
        wtr.write_record(&fields)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // city counts: Paris=2, Rome=1 over 3 data rows.
    const D: &str = "city,n\nParis,1\nRome,2\nParis,3";

    #[allow(clippy::too_many_arguments)]
    fn dflt(data: &str, column: &str) -> Result<String, String> {
        encode(
            data, column, "", "_", "none", true, "zeros", 0, 0, false, "1", "0", true,
            "alphabetical", true, ",",
        )
    }

    #[test]
    fn expands_column_into_one_indicator_per_category() {
        assert_eq!(
            dflt(D, "city").unwrap(),
            "n,city_Paris,city_Rome\n1,1,0\n2,0,1\n3,1,0\n"
        );
    }

    #[test]
    fn keeps_the_source_column_when_asked() {
        let out = encode(
            D, "city", "", "_", "none", false, "zeros", 0, 0, false, "1", "0", true,
            "alphabetical", true, ",",
        )
        .unwrap();
        assert_eq!(
            out,
            "city,n,city_Paris,city_Rome\nParis,1,1,0\nRome,2,0,1\nParis,3,1,0\n"
        );
    }

    #[test]
    fn drop_first_and_last_remove_a_reference_level() {
        let first = encode(
            D, "city", "", "_", "first", true, "zeros", 0, 0, false, "1", "0", true,
            "alphabetical", true, ",",
        )
        .unwrap();
        assert_eq!(first, "n,city_Rome\n1,0\n2,1\n3,0\n");
        let last = encode(
            D, "city", "", "_", "last", true, "zeros", 0, 0, false, "1", "0", true,
            "alphabetical", true, ",",
        )
        .unwrap();
        assert_eq!(last, "n,city_Paris\n1,1\n2,0\n3,1\n");
    }

    #[test]
    fn if_binary_drops_only_when_there_are_exactly_two_categories() {
        // two categories → behaves like drop=first
        let two = encode(
            D, "city", "", "_", "if-binary", true, "zeros", 0, 0, false, "1", "0", true,
            "alphabetical", true, ",",
        )
        .unwrap();
        assert_eq!(two, "n,city_Rome\n1,0\n2,1\n3,0\n");
        // three categories → nothing dropped
        let d3 = "city\nParis\nRome\nOslo";
        let three = encode(
            d3, "city", "", "_", "if-binary", true, "zeros", 0, 0, false, "1", "0", true,
            "alphabetical", true, ",",
        )
        .unwrap();
        assert_eq!(
            three,
            "city_Oslo,city_Paris,city_Rome\n0,1,0\n0,0,1\n1,0,0\n"
        );
    }

    #[test]
    fn prefix_and_separator_rename_the_generated_columns() {
        let out = encode(
            D, "city", "is", "=", "none", true, "zeros", 0, 0, false, "1", "0", true,
            "alphabetical", true, ",",
        )
        .unwrap();
        assert_eq!(out.lines().next().unwrap(), "n,is=Paris,is=Rome");
    }

    #[test]
    fn custom_positive_and_negative_values() {
        let out = encode(
            D, "city", "", "_", "none", true, "zeros", 0, 0, false, "true", "false", true,
            "alphabetical", true, ",",
        )
        .unwrap();
        assert_eq!(out.lines().nth(1).unwrap(), "1,true,false");
    }

    #[test]
    fn missing_modes() {
        let d = "city,n\nParis,1\n,2\nRome,3";
        // zeros: the blank row is 0 everywhere (pandas get_dummies default)
        let zeros = encode(
            d, "city", "", "_", "none", true, "zeros", 0, 0, false, "1", "0", true,
            "alphabetical", true, ",",
        )
        .unwrap();
        assert_eq!(zeros, "n,city_Paris,city_Rome\n1,1,0\n2,0,0\n3,0,1\n");
        // blank: empty indicator cells instead of zeros
        let blank = encode(
            d, "city", "", "_", "none", true, "blank", 0, 0, false, "1", "0", true,
            "alphabetical", true, ",",
        )
        .unwrap();
        assert_eq!(blank, "n,city_Paris,city_Rome\n1,1,0\n2,,\n3,0,1\n");
        // separate: blanks get their own indicator column, written last
        let sep = encode(
            d, "city", "", "_", "none", true, "separate", 0, 0, false, "1", "0", true,
            "alphabetical", true, ",",
        )
        .unwrap();
        assert_eq!(
            sep,
            "n,city_Paris,city_Rome,city_NaN\n1,1,0,0\n2,0,0,1\n3,0,1,0\n"
        );
        // error: refuses to guess
        assert!(encode(
            d, "city", "", "_", "none", true, "error", 0, 0, false, "1", "0", true,
            "alphabetical", true, ","
        )
        .is_err());
    }

    #[test]
    fn max_categories_keeps_only_the_most_frequent() {
        // A=3, B=2, C=1 → top 2 are A and B, still written alphabetically.
        let d = "v\nA\nA\nA\nB\nB\nC";
        let out = encode(
            d, "v", "", "_", "none", true, "zeros", 2, 0, false, "1", "0", true, "alphabetical",
            true, ",",
        )
        .unwrap();
        assert_eq!(out, "v_A,v_B\n1,0\n1,0\n1,0\n0,1\n0,1\n0,0\n");
        // with other_column the dropped C lands in one combined bucket
        let other = encode(
            d, "v", "", "_", "none", true, "zeros", 2, 0, true, "1", "0", true, "alphabetical",
            true, ",",
        )
        .unwrap();
        assert_eq!(
            other,
            "v_A,v_B,v_other\n1,0,0\n1,0,0\n1,0,0\n0,1,0\n0,1,0\n0,0,1\n"
        );
    }

    #[test]
    fn min_count_drops_rare_categories() {
        let d = "v\nA\nA\nB";
        let out = encode(
            d, "v", "", "_", "none", true, "zeros", 0, 2, false, "1", "0", true, "alphabetical",
            true, ",",
        )
        .unwrap();
        assert_eq!(out, "v_A\n1\n1\n0\n");
    }

    #[test]
    fn sort_orders_the_generated_columns() {
        // B=2, A=1 — alphabetical puts A first, frequency puts B first, first-seen keeps B first.
        let d = "v\nB\nA\nB";
        let alpha = encode(
            d, "v", "", "_", "none", true, "zeros", 0, 0, false, "1", "0", true, "alphabetical",
            true, ",",
        )
        .unwrap();
        assert_eq!(alpha.lines().next().unwrap(), "v_A,v_B");
        let freq = encode(
            d, "v", "", "_", "none", true, "zeros", 0, 0, false, "1", "0", true, "frequency",
            true, ",",
        )
        .unwrap();
        assert_eq!(freq.lines().next().unwrap(), "v_B,v_A");
        let seen = encode(
            d, "v", "", "_", "none", true, "zeros", 0, 0, false, "1", "0", true, "first-seen",
            true, ",",
        )
        .unwrap();
        assert_eq!(seen.lines().next().unwrap(), "v_B,v_A");
    }

    #[test]
    fn case_insensitive_groups_spellings_under_the_first_seen_label() {
        let d = "city\nParis\nPARIS\nRome";
        let folded = encode(
            d, "city", "", "_", "none", true, "zeros", 0, 0, false, "1", "0", false,
            "alphabetical", true, ",",
        )
        .unwrap();
        assert_eq!(folded, "city_Paris,city_Rome\n1,0\n1,0\n0,1\n");
        let exact = encode(
            d, "city", "", "_", "none", true, "zeros", 0, 0, false, "1", "0", true,
            "alphabetical", true, ",",
        )
        .unwrap();
        assert_eq!(exact.lines().next().unwrap(), "city_PARIS,city_Paris,city_Rome");
    }

    #[test]
    fn column_by_index_without_header() {
        let d = "Paris,1\nRome,2";
        let out = encode(
            d, "1", "", "_", "none", true, "zeros", 0, 0, false, "1", "0", true, "alphabetical",
            false, ",",
        )
        .unwrap();
        // No header row in, no header row out — just the indicator values.
        assert_eq!(out, "1,1,0\n2,0,1\n");
    }

    #[test]
    fn tab_delimiter_round_trips() {
        let d = "city\tn\nParis\t1\nRome\t2";
        let out = encode(
            d, "city", "", "_", "none", true, "zeros", 0, 0, false, "1", "0", true,
            "alphabetical", true, "tab",
        )
        .unwrap();
        assert_eq!(out, "n\tcity_Paris\tcity_Rome\n1\t1\t0\n2\t0\t1\n");
    }

    #[test]
    fn category_values_with_delimiters_are_quoted() {
        let d = "city,n\n\"Paris, FR\",1\nRome,2";
        let out = encode(
            d, "city", "", "_", "none", true, "zeros", 0, 0, false, "1", "0", true,
            "alphabetical", true, ",",
        )
        .unwrap();
        assert_eq!(out.lines().next().unwrap(), "n,\"city_Paris, FR\",city_Rome");
    }

    #[test]
    fn column_cap_is_enforced() {
        // 513 distinct values → one column over the limit.
        let mut d = String::from("v\n");
        for i in 0..=MAX_COLUMNS {
            d.push_str(&format!("v{i}\n"));
        }
        let err = dflt(&d, "v").unwrap_err();
        assert!(err.contains("limit 512"), "{err}");
        // exactly at the cap is fine
        let mut ok = String::from("v\n");
        for i in 0..MAX_COLUMNS {
            ok.push_str(&format!("v{i}\n"));
        }
        assert!(dflt(&ok, "v").is_ok());
    }

    #[test]
    fn errors() {
        // empty input
        assert!(dflt("   ", "city").is_err());
        // unknown column name
        assert!(dflt(D, "nope").is_err());
        // out-of-range index
        assert!(encode(
            D, "9", "", "_", "none", true, "zeros", 0, 0, false, "1", "0", true, "alphabetical",
            false, ","
        )
        .is_err());
        // bad enum values
        assert!(encode(
            D, "city", "", "_", "sideways", true, "zeros", 0, 0, false, "1", "0", true,
            "alphabetical", true, ","
        )
        .is_err());
        assert!(encode(
            D, "city", "", "_", "none", true, "maybe", 0, 0, false, "1", "0", true,
            "alphabetical", true, ","
        )
        .is_err());
        assert!(encode(
            D, "city", "", "_", "none", true, "zeros", 0, 0, false, "1", "0", true, "random",
            true, ","
        )
        .is_err());
        // bad delimiter
        assert!(encode(
            D, "city", "", "_", "none", true, "zeros", 0, 0, false, "1", "0", true,
            "alphabetical", true, "::"
        )
        .is_err());
        // header-only input has no data rows
        assert!(dflt("city,n", "city").is_err());
        // an all-blank column has nothing to encode
        assert!(dflt("city,n\n,1\n,2", "city").is_err());
        // limits that exclude every category
        assert!(encode(
            D, "city", "", "_", "none", true, "zeros", 0, 99, false, "1", "0", true,
            "alphabetical", true, ","
        )
        .is_err());
    }
}
