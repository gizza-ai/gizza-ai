//! gizza-ai/csv-fill-down core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps. Forward-fills (spreadsheet "fill
//! down") empty cells in chosen columns with the last non-empty value above them;
//! `direction = "up"` back-fills from the next non-empty value below instead. A
//! cell is "empty" when it is blank or contains only whitespace. Empties with no
//! source value to carry (leading cells on fill-down, trailing on fill-up) stay
//! empty.

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

/// Resolve `columns_csv` (1-based indices and/or header names) to 0-based column
/// indices. Empty → None (fill every column).
fn resolve_columns(
    columns_csv: &str,
    header: Option<&csv::StringRecord>,
) -> Result<Option<Vec<usize>>, String> {
    let toks: Vec<&str> = columns_csv
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if toks.is_empty() {
        return Ok(None);
    }
    let mut idxs = Vec::new();
    for t in toks {
        if let Ok(n) = t.parse::<usize>() {
            if n == 0 {
                return Err("column indices are 1-based (>= 1)".into());
            }
            idxs.push(n - 1);
        } else if let Some(h) = header {
            match h.iter().position(|c| c == t) {
                Some(p) => idxs.push(p),
                None => return Err(format!("column '{t}' not found in the header")),
            }
        } else {
            return Err(format!(
                "column '{t}' is not a number and there is no header to match names"
            ));
        }
    }
    Ok(Some(idxs))
}

fn is_empty_cell(s: &str) -> bool {
    s.trim().is_empty()
}

/// Forward-fill (or back-fill when `direction = "up"`) empty cells in the chosen
/// columns. `has_header` keeps the first row verbatim and lets `columns` name
/// columns. `columns` empty = fill every column. `delimiter` is a char or
/// comma/tab/semicolon/pipe.
pub fn fill_down(
    data: &str,
    columns: &str,
    direction: &str,
    has_header: bool,
    delimiter: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let up = match direction {
        "" | "down" => false,
        "up" => true,
        other => return Err(format!("direction must be 'down' or 'up', got '{other}'")),
    };
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
        return Ok(String::new());
    }

    // Header row (row 0) is kept verbatim and excluded from filling.
    let body_start = usize::from(has_header);
    let header = if has_header { records.first() } else { None };
    let target_cols = resolve_columns(columns, header)?;

    // Which columns to fill: the resolved subset, or all columns (widest row)
    // when none given.
    let cols: Vec<usize> = match target_cols {
        Some(c) => c,
        None => {
            let width = records.iter().map(|r| r.len()).max().unwrap_or(0);
            (0..width).collect()
        }
    };

    // Rebuild each body record as an owned Vec<String> so we can mutate cells.
    let mut body: Vec<Vec<String>> = records[body_start..]
        .iter()
        .map(|r| r.iter().map(|c| c.to_string()).collect())
        .collect();

    for &c in &cols {
        let mut carry: Option<String> = None;
        if up {
            for row in body.iter_mut().rev() {
                fill_one(row, c, &mut carry);
            }
        } else {
            for row in body.iter_mut() {
                fill_one(row, c, &mut carry);
            }
        }
    }

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);
    if let Some(h) = header {
        wtr.write_record(h)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    for row in &body {
        wtr.write_record(row)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

/// Fill cell `c` of `row` from `carry`, or update `carry` from a non-empty cell.
/// A row shorter than `c` has no cell there — leave `carry` untouched (a missing
/// trailing field can't be written without reshaping the row).
fn fill_one(row: &mut [String], c: usize, carry: &mut Option<String>) {
    let Some(cell) = row.get_mut(c) else {
        return;
    };
    if is_empty_cell(cell) {
        if let Some(v) = carry.as_ref() {
            *cell = v.clone();
        }
    } else {
        *carry = Some(cell.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_down_all_columns_with_header() {
        let d = "region,rep\nWest,Ann\n,\n,Bob\nEast,";
        assert_eq!(
            fill_down(d, "", "down", true, ",").unwrap(),
            "region,rep\nWest,Ann\nWest,Ann\nWest,Bob\nEast,Bob\n"
        );
    }

    #[test]
    fn fills_only_chosen_column_by_name() {
        // Only 'region' is filled; 'rep' keeps its blanks.
        let d = "region,rep\nWest,Ann\n,\nEast,Bob";
        assert_eq!(
            fill_down(d, "region", "down", true, ",").unwrap(),
            "region,rep\nWest,Ann\nWest,\nEast,Bob\n"
        );
    }

    #[test]
    fn fills_by_index_no_header() {
        // 1-based index 1, no header → fill first column down.
        let d = "West,Ann\n,Bob\n,Carol";
        assert_eq!(
            fill_down(d, "1", "down", false, ",").unwrap(),
            "West,Ann\nWest,Bob\nWest,Carol\n"
        );
    }

    #[test]
    fn leading_empties_stay_empty_on_fill_down() {
        // No value above the first two 'id' cells → they remain blank. (Two
        // columns so the empty cells carry a delimiter and aren't blank lines,
        // which the CSV reader skips.)
        let d = "row,id\np,\nq,\nr,V";
        assert_eq!(
            fill_down(d, "id", "down", true, ",").unwrap(),
            "row,id\np,\nq,\nr,V\n"
        );
    }

    #[test]
    fn fills_up_back_fills_from_below() {
        let d = "region,x\nWest,1\n,2\nEast,3";
        assert_eq!(
            fill_down(d, "region", "up", true, ",").unwrap(),
            "region,x\nWest,1\nEast,2\nEast,3\n"
        );
    }

    #[test]
    fn whitespace_only_cell_counts_as_empty() {
        let d = "region\nWest\n   \nEast";
        assert_eq!(
            fill_down(d, "", "down", true, ",").unwrap(),
            "region\nWest\nWest\nEast\n"
        );
    }

    #[test]
    fn tab_delimiter() {
        let d = "a\tb\nX\t1\n\t2";
        assert_eq!(
            fill_down(d, "", "down", true, "tab").unwrap(),
            "a\tb\nX\t1\nX\t2\n"
        );
    }

    #[test]
    fn errors() {
        assert!(fill_down("  ", "", "down", true, ",").is_err()); // empty input
        assert!(fill_down("a,b\n1,2", "nope", "down", true, ",").is_err()); // unknown name
        assert!(fill_down("a,b\n1,2", "0", "down", true, ",").is_err()); // 0 index
        assert!(fill_down("a,b\n1,2", "name", "down", false, ",").is_err()); // name w/o header
        assert!(fill_down("a,b\n1,2", "", "sideways", true, ",").is_err()); // bad direction
        assert!(fill_down("a,b\n1,2", "", "down", true, "bad").is_err()); // bad delimiter
    }
}
