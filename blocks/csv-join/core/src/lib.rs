//! csv-join core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Joins TWO CSV inputs on a key column, SQL-style: inner / left / right / full
//! outer. The first row of each input is the header. The key is referenced by
//! header NAME or 1-based column index and matched on VALUES (the two key columns
//! may have different names). Output columns = the left header, then every
//! non-key right column; a right column whose name collides with an existing
//! output column is suffixed with `_right`. Unmatched rows are padded with blank
//! cells (the key cell is filled from whichever side is present). Duplicate keys
//! produce a Cartesian product (one output row per left×right match), matching SQL
//! join semantics.

/// SQL-style join variants.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Outer,
}

impl JoinType {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "inner" => Ok(JoinType::Inner),
            "left" => Ok(JoinType::Left),
            "right" => Ok(JoinType::Right),
            "outer" | "full" | "full outer" | "full-outer" => Ok(JoinType::Outer),
            other => Err(format!(
                "join_type must be inner/left/right/outer, got '{other}'"
            )),
        }
    }
    fn keep_left_unmatched(self) -> bool {
        matches!(self, JoinType::Left | JoinType::Outer)
    }
    fn keep_right_unmatched(self) -> bool {
        matches!(self, JoinType::Right | JoinType::Outer)
    }
}

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

/// Parse CSV text into (header, rows). Errors if there is no header row.
fn parse(
    data: &str,
    delim: u8,
    side: &str,
) -> Result<(csv::StringRecord, Vec<csv::StringRecord>), String> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let mut recs = rdr
        .records()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{side} CSV parse error: {e}"))?;
    if recs.is_empty() {
        return Err(format!("{side} CSV is empty — a header row is required"));
    }
    let header = recs.remove(0);
    Ok((header, recs))
}

/// Resolve a column reference (header name, else 1-based index) to a 0-based index.
fn resolve_col(header: &csv::StringRecord, key: &str, side: &str) -> Result<usize, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err(format!("{side} key column is required"));
    }
    if let Some(pos) = header.iter().position(|h| h == key) {
        return Ok(pos);
    }
    if let Ok(n) = key.parse::<usize>() {
        if n >= 1 && n <= header.len() {
            return Ok(n - 1);
        }
        return Err(format!(
            "{side} key index {n} out of range 1..={} (header has {} columns)",
            header.len(),
            header.len()
        ));
    }
    Err(format!(
        "{side} key column '{key}' not found in header [{}]",
        header.iter().collect::<Vec<_>>().join(", ")
    ))
}

/// Cell value at `idx`, or "" if the row is shorter than the header (ragged rows).
fn cell(rec: &csv::StringRecord, idx: usize) -> &str {
    rec.get(idx).unwrap_or("")
}

fn norm_key(v: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        v.to_string()
    } else {
        v.to_lowercase()
    }
}

/// Join `left` and `right` CSV text on their key columns.
///
/// `left_key`/`right_key` are header names or 1-based indices; a blank
/// `right_key` reuses `left_key`'s reference. See the module docs for the join
/// semantics.
pub fn join(
    left: &str,
    right: &str,
    left_key: &str,
    right_key: &str,
    join_type: &str,
    delimiter: &str,
    case_sensitive: bool,
) -> Result<String, String> {
    if left.trim().is_empty() {
        return Err("left CSV is empty".into());
    }
    if right.trim().is_empty() {
        return Err("right CSV is empty".into());
    }
    let jt = JoinType::parse(join_type)?;
    let delim = delim_byte(delimiter)?;

    let (lh, lrows) = parse(left, delim, "left")?;
    let (rh, rrows) = parse(right, delim, "right")?;

    let lk = resolve_col(&lh, left_key, "left")?;
    // Blank right key → reuse the left key reference (name or index).
    let rk_ref = if right_key.trim().is_empty() {
        left_key
    } else {
        right_key
    };
    let rk = resolve_col(&rh, rk_ref, "right")?;

    // Output header: full left header, then right non-key columns (collisions suffixed).
    let mut out_header: Vec<String> = lh.iter().map(|s| s.to_string()).collect();
    // Source index in the right record for each appended right column.
    let mut right_src: Vec<usize> = Vec::new();
    for (ci, name) in rh.iter().enumerate() {
        if ci == rk {
            continue;
        }
        let mut out_name = name.to_string();
        if out_header.iter().any(|h| h == &out_name) {
            out_name = format!("{name}_right");
            // Extremely rare second collision: disambiguate with the column index.
            while out_header.iter().any(|h| h == &out_name) {
                out_name = format!("{name}_right{ci}");
            }
        }
        out_header.push(out_name);
        right_src.push(ci);
    }

    let width = out_header.len();
    let l_width = lh.len();

    // Index right rows by (normalized) key value.
    let mut r_index: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (i, r) in rrows.iter().enumerate() {
        let k = norm_key(cell(r, rk), case_sensitive);
        r_index.entry(k).or_default().push(i);
    }

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);
    wtr.write_record(&out_header)
        .map_err(|e| format!("CSV write error: {e}"))?;

    // Build one output row from an optional left row and optional right row.
    let build_row =
        |lrow: Option<&csv::StringRecord>, rrow: Option<&csv::StringRecord>| -> Vec<String> {
            let mut out = vec![String::new(); width];
            if let Some(l) = lrow {
                for (ci, slot) in out.iter_mut().enumerate().take(l_width) {
                    *slot = cell(l, ci).to_string();
                }
            } else if let Some(r) = rrow {
                // Right-only row: populate the (left) key column from the right key value.
                out[lk] = cell(r, rk).to_string();
            }
            if let Some(r) = rrow {
                for (out_off, &src) in right_src.iter().enumerate() {
                    out[l_width + out_off] = cell(r, src).to_string();
                }
            }
            out
        };

    let mut right_matched = vec![false; rrows.len()];

    for l in &lrows {
        let k = norm_key(cell(l, lk), case_sensitive);
        match r_index.get(&k) {
            Some(idxs) if !idxs.is_empty() => {
                for &ri in idxs {
                    right_matched[ri] = true;
                    let row = build_row(Some(l), Some(&rrows[ri]));
                    wtr.write_record(&row)
                        .map_err(|e| format!("CSV write error: {e}"))?;
                }
            }
            _ => {
                if jt.keep_left_unmatched() {
                    let row = build_row(Some(l), None);
                    wtr.write_record(&row)
                        .map_err(|e| format!("CSV write error: {e}"))?;
                }
            }
        }
    }

    if jt.keep_right_unmatched() {
        for (i, r) in rrows.iter().enumerate() {
            if !right_matched[i] {
                let row = build_row(None, Some(r));
                wtr.write_record(&row)
                    .map_err(|e| format!("CSV write error: {e}"))?;
            }
        }
    }

    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("output not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: &str = "id,name\n1,Alice\n2,Bob\n3,Carol";
    const R: &str = "id,city\n2,Berlin\n3,Cairo\n4,Delhi";

    #[test]
    fn inner_join_keeps_only_matches() {
        let out = join(L, R, "id", "", "inner", ",", true).unwrap();
        assert_eq!(out, "id,name,city\n2,Bob,Berlin\n3,Carol,Cairo\n");
    }

    #[test]
    fn left_join_pads_unmatched_left_rows() {
        let out = join(L, R, "id", "id", "left", ",", true).unwrap();
        assert_eq!(out, "id,name,city\n1,Alice,\n2,Bob,Berlin\n3,Carol,Cairo\n");
    }

    #[test]
    fn right_join_pads_unmatched_right_rows_and_fills_key() {
        let out = join(L, R, "id", "id", "right", ",", true).unwrap();
        // matched rows first (left order), then right-only row 4 with blank name.
        assert_eq!(out, "id,name,city\n2,Bob,Berlin\n3,Carol,Cairo\n4,,Delhi\n");
    }

    #[test]
    fn outer_join_keeps_both_sides() {
        let out = join(L, R, "id", "id", "outer", ",", true).unwrap();
        assert_eq!(
            out,
            "id,name,city\n1,Alice,\n2,Bob,Berlin\n3,Carol,Cairo\n4,,Delhi\n"
        );
    }

    #[test]
    fn different_key_names_match_on_values() {
        let l = "user_id,name\n1,Alice\n2,Bob";
        let r = "uid,plan\n2,Pro\n1,Free";
        let out = join(l, r, "user_id", "uid", "inner", ",", true).unwrap();
        assert_eq!(out, "user_id,name,plan\n1,Alice,Free\n2,Bob,Pro\n");
    }

    #[test]
    fn overlapping_column_names_get_right_suffix() {
        let l = "id,name\n1,Alice";
        let r = "id,name\n1,Alicia";
        let out = join(l, r, "id", "id", "inner", ",", true).unwrap();
        assert_eq!(out, "id,name,name_right\n1,Alice,Alicia\n");
    }

    #[test]
    fn duplicate_keys_produce_cartesian_product() {
        let l = "k,a\nx,1\nx,2";
        let r = "k,b\nx,9";
        let out = join(l, r, "k", "k", "inner", ",", true).unwrap();
        assert_eq!(out, "k,a,b\nx,1,9\nx,2,9\n");
    }

    #[test]
    fn key_by_index_and_case_insensitive() {
        let l = "id,name\nA1,Alice";
        let r = "id,city\na1,Berlin";
        // 1-based index for the key, case-insensitive match A1 == a1.
        let out = join(l, r, "1", "1", "inner", ",", false).unwrap();
        assert_eq!(out, "id,name,city\nA1,Alice,Berlin\n");
    }

    #[test]
    fn semicolon_delimiter() {
        let l = "id;name\n1;Alice";
        let r = "id;city\n1;Berlin";
        let out = join(l, r, "id", "id", "inner", "semicolon", true).unwrap();
        assert_eq!(out, "id;name;city\n1;Alice;Berlin\n");
    }

    // --- error paths ---

    #[test]
    fn unknown_key_column_errors() {
        let err = join(L, R, "nope", "", "inner", ",", true).unwrap_err();
        assert!(err.contains("left key column 'nope' not found"), "{err}");
    }

    #[test]
    fn bad_join_type_errors() {
        let err = join(L, R, "id", "", "sideways", ",", true).unwrap_err();
        assert!(err.contains("join_type must be"), "{err}");
    }

    #[test]
    fn empty_input_errors() {
        let err = join("", R, "id", "", "inner", ",", true).unwrap_err();
        assert!(err.contains("left CSV is empty"), "{err}");
    }
}
