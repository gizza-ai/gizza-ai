//! csv-anti-join core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Returns the rows of CSV A whose KEY has no match in CSV B — the SQL
//! `LEFT ANTI JOIN` / `NOT IN` primitive — and optionally the reverse. The first
//! row of each input is the header. The key is one or more columns, referenced by
//! header NAME or 1-based index (comma-separated for a composite key) and matched
//! on VALUES (the key columns may be named differently in A and B). Unlike a join,
//! B is treated as a key SET: duplicate keys in B do NOT multiply output rows, and
//! every A row whose key is absent from B is kept (duplicates included).
//!
//! `direction`:
//!   - `a-only` — rows in A not in B (output = A's header + those rows).
//!   - `b-only` — rows in B not in A (output = B's header + those rows).
//!   - `both`   — symmetric difference: a leading `_source` column (`A`/`B`) then
//!                A's header; A-only rows first, then B-only rows mapped onto A's
//!                columns by position (assumes A and B share the same layout).

/// Which side(s) of the anti-join to emit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    AOnly,
    BOnly,
    Both,
}

impl Direction {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "a-only" | "a_only" | "a" | "left" => Ok(Direction::AOnly),
            "b-only" | "b_only" | "b" | "right" | "reverse" => Ok(Direction::BOnly),
            "both" | "symmetric" | "symmetric-difference" => Ok(Direction::Both),
            other => Err(format!(
                "direction must be a-only/b-only/both, got '{other}'"
            )),
        }
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

/// Resolve ONE column reference (header name, else 1-based index) to a 0-based index.
fn resolve_one(header: &csv::StringRecord, key: &str, side: &str) -> Result<usize, String> {
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

/// Resolve a comma-separated composite key reference to 0-based column indices.
fn resolve_key(header: &csv::StringRecord, key: &str, side: &str) -> Result<Vec<usize>, String> {
    let cols: Vec<usize> = key
        .split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| resolve_one(header, p, side))
        .collect::<Result<_, _>>()?;
    if cols.is_empty() {
        return Err(format!("{side} key column is required"));
    }
    Ok(cols)
}

/// Cell value at `idx`, or "" if the row is shorter than the header (ragged rows).
fn cell(rec: &csv::StringRecord, idx: usize) -> &str {
    rec.get(idx).unwrap_or("")
}

/// Build the normalized composite key string for a row (unit-separator joined).
fn row_key(rec: &csv::StringRecord, cols: &[usize], case_sensitive: bool, trim_keys: bool) -> String {
    let mut out = String::new();
    for (i, &c) in cols.iter().enumerate() {
        if i > 0 {
            out.push('\u{1f}'); // ASCII unit separator — cannot appear in a CSV cell value.
        }
        let mut v = cell(rec, c);
        if trim_keys {
            v = v.trim();
        }
        if case_sensitive {
            out.push_str(v);
        } else {
            out.push_str(&v.to_lowercase());
        }
    }
    out
}

/// Anti-join `a` against `b` on their key columns. See the module docs.
///
/// `key` is a comma-separated list of header names or 1-based indices in A;
/// `key_b` is the same for B (blank reuses `key`).
pub fn anti_join(
    a: &str,
    b: &str,
    key: &str,
    key_b: &str,
    direction: &str,
    delimiter: &str,
    case_sensitive: bool,
    trim_keys: bool,
) -> Result<String, String> {
    if a.trim().is_empty() {
        return Err("CSV A is empty".into());
    }
    if b.trim().is_empty() {
        return Err("CSV B is empty".into());
    }
    let dir = Direction::parse(direction)?;
    let delim = delim_byte(delimiter)?;

    let (ah, arows) = parse(a, delim, "A")?;
    let (bh, brows) = parse(b, delim, "B")?;

    let ak = resolve_key(&ah, key, "A")?;
    let bk_ref = if key_b.trim().is_empty() { key } else { key_b };
    let bk = resolve_key(&bh, bk_ref, "B")?;
    if ak.len() != bk.len() {
        return Err(format!(
            "key column count mismatch: A key has {} column(s) but B key has {}",
            ak.len(),
            bk.len()
        ));
    }

    // Key SETS (not multimaps) — B duplicates never multiply output rows.
    let a_keys: std::collections::HashSet<String> = arows
        .iter()
        .map(|r| row_key(r, &ak, case_sensitive, trim_keys))
        .collect();
    let b_keys: std::collections::HashSet<String> = brows
        .iter()
        .map(|r| row_key(r, &bk, case_sensitive, trim_keys))
        .collect();

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);

    match dir {
        Direction::AOnly => {
            wtr.write_record(&ah)
                .map_err(|e| format!("CSV write error: {e}"))?;
            for r in &arows {
                if !b_keys.contains(&row_key(r, &ak, case_sensitive, trim_keys)) {
                    wtr.write_record(r)
                        .map_err(|e| format!("CSV write error: {e}"))?;
                }
            }
        }
        Direction::BOnly => {
            wtr.write_record(&bh)
                .map_err(|e| format!("CSV write error: {e}"))?;
            for r in &brows {
                if !a_keys.contains(&row_key(r, &bk, case_sensitive, trim_keys)) {
                    wtr.write_record(r)
                        .map_err(|e| format!("CSV write error: {e}"))?;
                }
            }
        }
        Direction::Both => {
            // Header: `_source` + A's header. A-only rows tagged A, then B-only rows
            // tagged B and mapped onto A's columns by position.
            let width = ah.len();
            let mut header: Vec<String> = Vec::with_capacity(width + 1);
            header.push("_source".to_string());
            header.extend(ah.iter().map(|s| s.to_string()));
            wtr.write_record(&header)
                .map_err(|e| format!("CSV write error: {e}"))?;
            let mut emit = |src: &str, r: &csv::StringRecord| -> Result<(), String> {
                let mut row: Vec<String> = Vec::with_capacity(width + 1);
                row.push(src.to_string());
                for i in 0..width {
                    row.push(cell(r, i).to_string());
                }
                wtr.write_record(&row)
                    .map_err(|e| format!("CSV write error: {e}"))
            };
            for r in &arows {
                if !b_keys.contains(&row_key(r, &ak, case_sensitive, trim_keys)) {
                    emit("A", r)?;
                }
            }
            for r in &brows {
                if !a_keys.contains(&row_key(r, &bk, case_sensitive, trim_keys)) {
                    emit("B", r)?;
                }
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

    const A: &str = "id,name\n1,Alice\n2,Bob\n3,Carol";
    const B: &str = "id,city\n2,Berlin\n3,Cairo\n4,Delhi";

    #[test]
    fn a_only_keeps_rows_of_a_absent_from_b() {
        let out = anti_join(A, B, "id", "", "a-only", ",", true, false).unwrap();
        assert_eq!(out, "id,name\n1,Alice\n");
    }

    #[test]
    fn b_only_is_the_reverse() {
        let out = anti_join(A, B, "id", "", "b-only", ",", true, false).unwrap();
        assert_eq!(out, "id,city\n4,Delhi\n");
    }

    #[test]
    fn both_is_symmetric_difference_with_source_column() {
        let out = anti_join(A, B, "id", "", "both", ",", true, false).unwrap();
        assert_eq!(out, "_source,id,name\nA,1,Alice\nB,4,Delhi\n");
    }

    #[test]
    fn different_key_names_match_on_values() {
        let a = "user_id,name\n1,Alice\n2,Bob";
        let b = "uid,plan\n2,Pro";
        let out = anti_join(a, b, "user_id", "uid", "a-only", ",", true, false).unwrap();
        assert_eq!(out, "user_id,name\n1,Alice\n");
    }

    #[test]
    fn composite_key_uses_all_named_columns() {
        let a = "first,last,age\nAda,Lovelace,36\nAlan,Turing,41";
        let b = "first,last\nAda,Lovelace";
        let out = anti_join(a, b, "first,last", "", "a-only", ",", true, false).unwrap();
        assert_eq!(out, "first,last,age\nAlan,Turing,41\n");
    }

    #[test]
    fn duplicate_keys_in_a_are_all_kept_when_unmatched() {
        // Two rows share the unmatched key x → both kept; y matches B → both dropped.
        let a = "k,v\nx,1\nx,2\ny,3\ny,4";
        let b = "k\ny";
        let out = anti_join(a, b, "k", "", "a-only", ",", true, false).unwrap();
        assert_eq!(out, "k,v\nx,1\nx,2\n");
    }

    #[test]
    fn duplicate_keys_in_b_do_not_multiply_output() {
        let a = "k,v\nx,1";
        let b = "k\nx\nx\nx";
        let out = anti_join(a, b, "k", "", "a-only", ",", true, false).unwrap();
        assert_eq!(out, "k,v\n"); // header only — x is matched (once), no Cartesian blow-up
    }

    #[test]
    fn key_by_index_and_case_insensitive() {
        let a = "id,name\nA1,Alice\nB2,Bob";
        let b = "id,city\na1,Berlin";
        // 1-based index for the key, case-insensitive so A1 == a1.
        let out = anti_join(a, b, "1", "1", "a-only", ",", false, false).unwrap();
        assert_eq!(out, "id,name\nB2,Bob\n");
    }

    #[test]
    fn trim_keys_ignores_surrounding_whitespace() {
        let a = "id,name\n 1 ,Alice\n2,Bob";
        let b = "id\n1";
        let out = anti_join(a, b, "id", "", "a-only", ",", true, true).unwrap();
        assert_eq!(out, "id,name\n2,Bob\n"); // " 1 " trims to "1" and matches B → dropped, "2" kept
    }

    #[test]
    fn semicolon_delimiter() {
        let a = "id;name\n1;Alice\n2;Bob";
        let b = "id;city\n1;Berlin";
        let out = anti_join(a, b, "id", "", "a-only", "semicolon", true, false).unwrap();
        assert_eq!(out, "id;name\n2;Bob\n");
    }

    // --- error paths ---

    #[test]
    fn unknown_key_column_errors() {
        let err = anti_join(A, B, "nope", "", "a-only", ",", true, false).unwrap_err();
        assert!(err.contains("A key column 'nope' not found"), "{err}");
    }

    #[test]
    fn bad_direction_errors() {
        let err = anti_join(A, B, "id", "", "sideways", ",", true, false).unwrap_err();
        assert!(err.contains("direction must be"), "{err}");
    }

    #[test]
    fn empty_input_errors() {
        let err = anti_join("", B, "id", "", "a-only", ",", true, false).unwrap_err();
        assert!(err.contains("CSV A is empty"), "{err}");
    }

    #[test]
    fn key_count_mismatch_errors() {
        let err = anti_join(A, B, "id", "id,city", "a-only", ",", true, false).unwrap_err();
        assert!(err.contains("key column count mismatch"), "{err}");
    }
}
