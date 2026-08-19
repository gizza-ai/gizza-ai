//! gizza-ai/csv-coalesce-columns core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps. Builds ONE column from the
//! first non-empty value across a list of source columns read in priority order
//! (SQL `COALESCE` over columns), with an optional fallback when every source is
//! empty, an optional drop of the source columns, and a choice of where the new
//! column lands.

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

/// Where the coalesced column is written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Position {
    /// Appended after the last column.
    End,
    /// Prepended before the first column.
    Start,
    /// At the spot the first listed source column occupies.
    FirstSource,
}

fn parse_position(position: &str) -> Result<Position, String> {
    Ok(match position.trim() {
        "" | "end" => Position::End,
        "start" => Position::Start,
        "first-source" => Position::FirstSource,
        other => {
            return Err(format!(
                "position must be 'end', 'start' or 'first-source', got '{other}'"
            ))
        }
    })
}

/// A cell counts as empty when it is `""`, or — with `blank_is_empty` — when it
/// holds nothing but whitespace, or when it matches one of `null_tokens`
/// (compared case-insensitively against the trimmed cell).
fn is_empty_cell(cell: &str, blank_is_empty: bool, null_tokens: &[String]) -> bool {
    let blank = if blank_is_empty {
        cell.trim().is_empty()
    } else {
        cell.is_empty()
    };
    if blank {
        return true;
    }
    let t = cell.trim().to_lowercase();
    null_tokens.iter().any(|n| *n == t)
}

/// Split the comma-separated `null_tokens` list into lowercase, trimmed tokens.
fn parse_null_tokens(null_tokens: &str) -> Vec<String> {
    null_tokens
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Resolve `columns_csv` to 0-based column indices, PRESERVING the listed order —
/// that order is the lookup priority. Tokens are 1-based indices, or header names
/// when a header row is present (a purely numeric token is always read as an
/// index, so a header literally named `2` must be addressed by its position).
fn resolve_sources(
    columns_csv: &str,
    header: Option<&csv::StringRecord>,
    width: usize,
) -> Result<Vec<usize>, String> {
    let toks: Vec<&str> = columns_csv
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if toks.is_empty() {
        return Err("list at least one source column in `columns` (header names, or 1-based indices)".into());
    }
    let mut idxs: Vec<usize> = Vec::new();
    for t in toks {
        let idx = if let Ok(n) = t.parse::<usize>() {
            if n == 0 {
                return Err("column indices are 1-based (>= 1)".into());
            }
            n - 1
        } else if let Some(h) = header {
            match h.iter().position(|c| c == t) {
                Some(p) => p,
                None => return Err(format!("column '{t}' not found in the header")),
            }
        } else {
            return Err(format!(
                "column '{t}' is not a number and header=false, so there is no header to match names against"
            ));
        };
        if idx >= width {
            return Err(format!(
                "column {} is out of range — the CSV has {width} column(s)",
                idx + 1
            ));
        }
        if idxs.contains(&idx) {
            return Err(format!("column '{t}' is listed twice in `columns`"));
        }
        idxs.push(idx);
    }
    Ok(idxs)
}

/// Coalesce `columns` (a comma-separated priority list of header names or 1-based
/// indices) into one column named `output` ("" → `coalesced`).
///
/// Each row takes the first source cell that isn't empty; when every source is
/// empty the row gets `fallback` (default `""`). `position` places the new column
/// (`end`/`start`/`first-source`), `drop_sources` removes the sources afterwards,
/// `blank_is_empty` decides whether whitespace-only cells count as empty,
/// `null_tokens` lists extra placeholder values (`NA`, `NULL`, `-`) that also
/// count as empty, `has_header` keeps + rewrites the first row, and `delimiter`
/// is a single char or comma/tab/semicolon/pipe.
#[allow(clippy::too_many_arguments)]
pub fn coalesce_columns(
    data: &str,
    columns: &str,
    output: &str,
    position: &str,
    fallback: &str,
    drop_sources: bool,
    blank_is_empty: bool,
    null_tokens: &str,
    has_header: bool,
    delimiter: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let pos = parse_position(position)?;
    let delim = delim_byte(delimiter)?;
    let nulls = parse_null_tokens(null_tokens);
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
    let sources = resolve_sources(columns, header, width)?;

    let name = if output.trim().is_empty() {
        "coalesced"
    } else {
        output.trim()
    };

    // Columns that survive into the output, in original order.
    let kept: Vec<usize> = (0..width)
        .filter(|i| !(drop_sources && sources.contains(i)))
        .collect();
    // Insertion point expressed against the SURVIVING columns.
    let insert_at = match pos {
        Position::End => kept.len(),
        Position::Start => 0,
        Position::FirstSource => kept
            .iter()
            .position(|&i| i >= sources[0])
            .unwrap_or(kept.len()),
    };

    if let Some(h) = header {
        if kept.iter().any(|&i| h.get(i) == Some(name)) {
            return Err(format!(
                "a column named '{name}' already exists — give the new column another name in `output`, or turn on drop_sources"
            ));
        }
    }

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);

    if let Some(h) = header {
        let mut row: Vec<String> = kept
            .iter()
            .map(|&i| h.get(i).unwrap_or("").to_string())
            .collect();
        row.insert(insert_at, name.to_string());
        wtr.write_record(&row)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }

    for rec in &records[usize::from(has_header)..] {
        let value = sources
            .iter()
            .find_map(|&i| {
                let cell = rec.get(i).unwrap_or("");
                (!is_empty_cell(cell, blank_is_empty, &nulls)).then(|| cell.to_string())
            })
            .unwrap_or_else(|| fallback.to_string());
        let mut row: Vec<String> = kept
            .iter()
            .map(|&i| rec.get(i).unwrap_or("").to_string())
            .collect();
        row.insert(insert_at, value);
        wtr.write_record(&row)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }

    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesces_named_columns_in_priority_order() {
        let d = "name,mobile,office,home\nAnn,555-1,555-2,555-3\nBob,,555-4,555-5\nCleo,,,555-6\n";
        assert_eq!(
            coalesce_columns(d, "mobile,office,home", "phone", "end", "", false, true, "", true, ",")
                .unwrap(),
            "name,mobile,office,home,phone\n\
             Ann,555-1,555-2,555-3,555-1\n\
             Bob,,555-4,555-5,555-4\n\
             Cleo,,,555-6,555-6\n"
        );
    }

    #[test]
    fn drops_sources_and_lands_at_the_first_source_slot() {
        let d = "name,mobile,office,city\nAnn,555-1,555-2,Rome\nBob,,555-4,Oslo\n";
        assert_eq!(
            coalesce_columns(
                d,
                "mobile,office",
                "phone",
                "first-source",
                "",
                true,
                true, "",
                true,
                ","
            )
            .unwrap(),
            "name,phone,city\nAnn,555-1,Rome\nBob,555-4,Oslo\n"
        );
    }

    #[test]
    fn falls_back_when_every_source_is_empty() {
        let d = "a,b\n,\nx,\n";
        assert_eq!(
            coalesce_columns(d, "a,b", "v", "end", "N/A", false, true, "", true, ",").unwrap(),
            "a,b,v\n,,N/A\nx,,x\n"
        );
    }

    #[test]
    fn whitespace_only_cell_wins_when_blank_is_empty_is_off() {
        // With blank_is_empty=false a cell holding a single space is a real value.
        let d = "a,b\n \x2Cfallbackvalue\n";
        assert_eq!(
            coalesce_columns(d, "a,b", "v", "end", "", false, false, "", true, ",").unwrap(),
            "a,b,v\n ,fallbackvalue, \n"
        );
        // With the default (true) the same cell is skipped.
        assert_eq!(
            coalesce_columns(d, "a,b", "v", "end", "", false, true, "", true, ",").unwrap(),
            "a,b,v\n ,fallbackvalue,fallbackvalue\n"
        );
    }

    #[test]
    fn resolves_1_based_indices_without_a_header() {
        let d = ",b1\na2,\n";
        assert_eq!(
            coalesce_columns(d, "1,2", "", "start", "-", false, true, "", false, ",").unwrap(),
            "b1,,b1\na2,a2,\n"
        );
    }

    #[test]
    fn handles_tab_delimited_input() {
        let d = "a\tb\n\tz\n";
        assert_eq!(
            coalesce_columns(d, "a,b", "v", "end", "", false, true, "", true, "tab").unwrap(),
            "a\tb\tv\n\tz\tz\n"
        );
    }

    #[test]
    fn short_rows_are_padded_not_dropped() {
        let d = "a,b,c\nx\n,,z\n";
        assert_eq!(
            coalesce_columns(d, "b,c", "v", "end", "-", false, true, "", true, ",").unwrap(),
            "a,b,c,v\nx,,,-\n,,z,z\n"
        );
    }

    #[test]
    fn errors_on_unknown_column_name() {
        let d = "a,b\n1,2\n";
        let err = coalesce_columns(d, "a,nope", "v", "end", "", false, true, "", true, ",").unwrap_err();
        assert_eq!(err, "column 'nope' not found in the header");
    }

    #[test]
    fn errors_when_the_output_name_collides_with_a_kept_column() {
        let d = "a,b\n1,2\n";
        let err = coalesce_columns(d, "a,b", "a", "end", "", false, true, "", true, ",").unwrap_err();
        assert!(err.starts_with("a column named 'a' already exists"), "{err}");
    }

    #[test]
    fn errors_on_empty_input_and_empty_column_list() {
        assert_eq!(
            coalesce_columns("   ", "a", "v", "end", "", false, true, "", true, ",").unwrap_err(),
            "input is empty"
        );
        assert!(coalesce_columns("a,b\n1,2\n", "  ", "v", "end", "", false, true, "", true, ",")
            .unwrap_err()
            .starts_with("list at least one source column"));
    }

    #[test]
    fn errors_on_a_bad_position_and_an_out_of_range_index() {
        assert_eq!(
            coalesce_columns("a,b\n1,2\n", "a", "v", "middle", "", false, true, "", true, ",")
                .unwrap_err(),
            "position must be 'end', 'start' or 'first-source', got 'middle'"
        );
        assert_eq!(
            coalesce_columns("a,b\n1,2\n", "1,9", "v", "end", "", false, true, "", true, ",")
                .unwrap_err(),
            "column 9 is out of range — the CSV has 2 column(s)"
        );
    }

    #[test]
    fn null_tokens_are_skipped_case_insensitively() {
        let d = "id,a,b\n1,NULL,x\n2,n/a,N/A\n3,-,y\n";
        // Without null_tokens the literal placeholders are real values.
        assert_eq!(
            coalesce_columns(d, "a,b", "v", "end", "?", false, true, "", true, ",").unwrap(),
            "id,a,b,v\n1,NULL,x,NULL\n2,n/a,N/A,n/a\n3,-,y,-\n"
        );
        // With them listed they behave like empties, so row 2 hits the fallback.
        assert_eq!(
            coalesce_columns(d, "a,b", "v", "end", "?", false, true, "NULL, N/A ,-", true, ",")
                .unwrap(),
            "id,a,b,v\n1,NULL,x,x\n2,n/a,N/A,?\n3,-,y,y\n"
        );
    }

    #[test]
    fn errors_on_a_duplicate_source_column() {
        let err =
            coalesce_columns("a,b\n1,2\n", "a,a", "v", "end", "", false, true, "", true, ",").unwrap_err();
        assert_eq!(err, "column 'a' is listed twice in `columns`");
    }
}
