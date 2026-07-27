//! gizza-ai/csv-column-split core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Two modes:
//! - `split`: take one source column and split each cell on a literal `separator`
//!   into several new columns (Excel "Text to Columns"). `max_columns` caps the
//!   number of output columns (0 = split on every occurrence; 2 = split on the
//!   first occurrence only). Output rows are padded to a rectangular width.
//! - `concat`: join several source columns, in the order listed, with `separator`
//!   into a single new column (the inverse — Excel CONCAT / `&`).
//!
//! In both modes `keep_source=false` (default) replaces the source column(s) in
//! place; `keep_source=true` keeps the originals and adds the new column(s) after
//! the source (split) or at the end (concat).

use std::collections::HashSet;

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

/// Parse the comma-separated `columns` list into trimmed, non-empty tokens.
fn column_tokens(columns: &str) -> Vec<String> {
    columns
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Resolve one target token to a 0-based column index: by header name when
/// `has_header`, otherwise (or as a fallback) by 1-based index.
fn resolve_index(
    token: &str,
    header: Option<&csv::StringRecord>,
    has_header: bool,
    width: usize,
) -> Result<usize, String> {
    if let Some(h) = header {
        if let Some(i) = h.iter().position(|c| c == token) {
            return Ok(i);
        }
    }
    let n: usize = token.parse().map_err(|_| {
        if has_header {
            format!("column '{token}' not found in the header (and is not a number)")
        } else {
            format!("column index '{token}' must be a number (no header)")
        }
    })?;
    if n == 0 || n > width {
        return Err(format!("column index {n} is out of range (1..={width})"));
    }
    Ok(n - 1)
}

/// Split one column into several, or concatenate several columns into one.
///
/// - `mode`: "split" or "concat".
/// - `columns`: for split, the source column (name or 1-based index — the first
///   token is used); for concat, a comma-separated list of columns in output order.
/// - `separator`: the literal delimiter to split each cell on (split) or to join
///   the source cells with (concat). May be any string (empty is allowed for concat).
/// - `names`: for split, comma-separated names for the new columns (missing ones are
///   auto-named `<source>_1`, `<source>_2`…); for concat, the name of the combined
///   column (defaults to the source names joined with `_`, or `combined`).
/// - `max_columns`: split only — cap on the number of output columns (0 = unlimited;
///   2 = split on the first occurrence only). Ignored for concat.
/// - `keep_source`: keep the original column(s) alongside the result.
/// - `trim`: split only — trim surrounding whitespace from each split value.
/// - `has_header`: treat the first row as a header.
/// - `delimiter`: the CSV field separator for both input and output.
#[allow(clippy::too_many_arguments)]
pub fn column_split(
    data: &str,
    mode: &str,
    columns: &str,
    separator: &str,
    names: &str,
    max_columns: usize,
    keep_source: bool,
    trim: bool,
    has_header: bool,
    delimiter: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
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
    let header: Option<&csv::StringRecord> = if has_header { records.first() } else { None };
    let names_list = column_tokens(names);

    let out_bytes = match mode {
        "split" => split_mode(
            &records,
            columns,
            separator,
            &names_list,
            max_columns,
            keep_source,
            trim,
            has_header,
            header,
            width,
            delim,
        )?,
        "concat" => concat_mode(
            &records,
            columns,
            separator,
            &names_list,
            keep_source,
            has_header,
            header,
            width,
            delim,
        )?,
        other => {
            return Err(format!("mode must be 'split' or 'concat', got '{other}'"));
        }
    };
    String::from_utf8(out_bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[allow(clippy::too_many_arguments)]
fn split_mode(
    records: &[csv::StringRecord],
    columns: &str,
    separator: &str,
    names_list: &[String],
    max_columns: usize,
    keep_source: bool,
    trim: bool,
    has_header: bool,
    header: Option<&csv::StringRecord>,
    width: usize,
    delim: u8,
) -> Result<Vec<u8>, String> {
    if separator.is_empty() {
        return Err("separator is required for split mode".into());
    }
    let tokens = column_tokens(columns);
    let token = tokens
        .first()
        .ok_or("provide the column to split (a name or 1-based index)")?;
    let src = resolve_index(token, header, has_header, width)?;

    // Split one cell into at most `max_columns` parts (0 = unlimited), trimming if asked.
    let split_cell = |cell: &str| -> Vec<String> {
        let raw: Vec<&str> = if max_columns > 0 {
            cell.splitn(max_columns, separator).collect()
        } else {
            cell.split(separator).collect()
        };
        raw.into_iter()
            .map(|p| {
                if trim {
                    p.trim().to_string()
                } else {
                    p.to_string()
                }
            })
            .collect()
    };

    // Rectangular width: the max parts across all data rows (and at least enough
    // columns to place every provided name; at least 1).
    let data_start = if has_header { 1 } else { 0 };
    let mut n_out = names_list.len();
    for rec in &records[data_start..] {
        n_out = n_out.max(split_cell(rec.get(src).unwrap_or("")).len());
    }
    n_out = n_out.max(1);

    let src_name = if has_header {
        header.and_then(|h| h.get(src)).unwrap_or("").to_string()
    } else {
        format!("col{}", src + 1)
    };
    let out_names: Vec<String> = (0..n_out)
        .map(|i| {
            names_list
                .get(i)
                .filter(|s| !s.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("{src_name}_{}", i + 1))
        })
        .collect();

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);
    for (i, rec) in records.iter().enumerate() {
        let fields: Vec<String> = rec.iter().map(|s| s.to_string()).collect();
        let new_cells: Vec<String> = if has_header && i == 0 {
            out_names.clone()
        } else {
            let mut parts = split_cell(rec.get(src).unwrap_or(""));
            while parts.len() < n_out {
                parts.push(String::new());
            }
            parts
        };
        let mut out: Vec<String> = Vec::with_capacity(fields.len() + n_out);
        let mut placed = false;
        for (ci, f) in fields.iter().enumerate() {
            if ci == src {
                if keep_source {
                    out.push(f.clone());
                }
                out.extend(new_cells.iter().cloned());
                placed = true;
            } else {
                out.push(f.clone());
            }
        }
        if !placed {
            // Ragged row shorter than `src`: append the new columns at the end.
            out.extend(new_cells.iter().cloned());
        }
        wtr.write_record(&out)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    wtr.into_inner()
        .map_err(|e| format!("CSV write error: {e}"))
}

#[allow(clippy::too_many_arguments)]
fn concat_mode(
    records: &[csv::StringRecord],
    columns: &str,
    separator: &str,
    names_list: &[String],
    keep_source: bool,
    has_header: bool,
    header: Option<&csv::StringRecord>,
    width: usize,
    delim: u8,
) -> Result<Vec<u8>, String> {
    let tokens = column_tokens(columns);
    if tokens.is_empty() {
        return Err("provide two or more columns to concatenate (names or 1-based indices)".into());
    }
    let idxs: Vec<usize> = tokens
        .iter()
        .map(|t| resolve_index(t, header, has_header, width))
        .collect::<Result<_, _>>()?;

    let out_name = names_list
        .first()
        .filter(|s| !s.is_empty())
        .cloned()
        .unwrap_or_else(|| {
            if has_header {
                idxs.iter()
                    .map(|&i| header.and_then(|h| h.get(i)).unwrap_or(""))
                    .collect::<Vec<_>>()
                    .join("_")
            } else {
                "combined".to_string()
            }
        });

    let drop: HashSet<usize> = idxs.iter().copied().collect();
    let first_src = *idxs.iter().min().unwrap();

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);
    for (i, rec) in records.iter().enumerate() {
        let fields: Vec<String> = rec.iter().map(|s| s.to_string()).collect();
        let new_cell = if has_header && i == 0 {
            out_name.clone()
        } else {
            idxs.iter()
                .map(|&i| rec.get(i).unwrap_or(""))
                .collect::<Vec<_>>()
                .join(separator)
        };
        let out: Vec<String> = if keep_source {
            let mut o = fields.clone();
            o.push(new_cell);
            o
        } else {
            let mut o = Vec::with_capacity(fields.len());
            let mut placed = false;
            for (ci, f) in fields.iter().enumerate() {
                if ci == first_src {
                    o.push(new_cell.clone());
                    placed = true;
                } else if drop.contains(&ci) {
                    continue;
                } else {
                    o.push(f.clone());
                }
            }
            if !placed {
                o.push(new_cell.clone());
            }
            o
        };
        wtr.write_record(&out)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    wtr.into_inner()
        .map_err(|e| format!("CSV write error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(
        data: &str,
        cols: &str,
        sep: &str,
        names: &str,
        max: usize,
        keep: bool,
        trim: bool,
        hdr: bool,
    ) -> String {
        column_split(data, "split", cols, sep, names, max, keep, trim, hdr, ",").unwrap()
    }
    fn concat(data: &str, cols: &str, sep: &str, names: &str, keep: bool, hdr: bool) -> String {
        column_split(data, "concat", cols, sep, names, 0, keep, true, hdr, ",").unwrap()
    }

    #[test]
    fn split_into_two() {
        let d = "id,name\n1,John Doe\n2,Ada Lovelace";
        assert_eq!(
            split(d, "name", " ", "first,last", 0, false, true, true),
            "id,first,last\n1,John,Doe\n2,Ada,Lovelace\n"
        );
    }

    #[test]
    fn split_first_occurrence_only() {
        // max_columns=2 → split on the first separator only; the rest stays in col 2.
        // Cells containing the field delimiter are quoted so they stay one field.
        let d = "loc\n\"Portland, OR\"\n\"San Jose, CA, USA\"";
        assert_eq!(
            split(d, "loc", ",", "city,rest", 2, false, true, true),
            "city,rest\nPortland,OR\nSan Jose,\"CA, USA\"\n"
        );
    }

    #[test]
    fn split_auto_names_and_padding() {
        // Ragged parts: row 2 has 3 tags, row 3 has 1 → padded to 3 cols, auto-named.
        let d = "tags\na;b;c\nx";
        assert_eq!(
            split(d, "tags", ";", "", 0, false, true, true),
            "tags_1,tags_2,tags_3\na,b,c\nx,,\n"
        );
    }

    #[test]
    fn split_keep_source_by_index_no_header() {
        // No header, index 1, keep the original column in place.
        let d = "a b,x\nc d,y";
        assert_eq!(
            split(d, "1", " ", "", 0, true, true, false),
            "a b,a,b,x\nc d,c,d,y\n"
        );
    }

    #[test]
    fn split_no_trim_keeps_spaces() {
        let d = "v\n\"Portland, OR\"";
        assert_eq!(
            split(d, "v", ",", "a,b", 0, false, false, true),
            "a,b\nPortland, OR\n"
        );
    }

    #[test]
    fn concat_two_columns() {
        let d = "first,last,age\nAda,Lovelace,36";
        assert_eq!(
            concat(d, "first,last", " ", "name", false, true),
            "name,age\nAda Lovelace,36\n"
        );
    }

    #[test]
    fn concat_default_name_and_order() {
        // No name given → source header names joined with '_'; order follows `columns`.
        let d = "a,b\n1,2\n3,4";
        assert_eq!(concat(d, "b,a", "-", "", false, true), "b_a\n2-1\n4-3\n");
    }

    #[test]
    fn concat_keep_source_appends() {
        let d = "first,last\nAda,Lovelace";
        assert_eq!(
            concat(d, "first,last", " ", "full", true, true),
            "first,last,full\nAda,Lovelace,Ada Lovelace\n"
        );
    }

    #[test]
    fn concat_empty_separator() {
        let d = "area,num\n212,5551234";
        assert_eq!(
            concat(d, "area,num", "", "phone", false, true),
            "phone\n2125551234\n"
        );
    }

    #[test]
    fn errors() {
        // empty input
        assert!(column_split("", "split", "a", ",", "", 0, false, true, true, ",").is_err());
        // unknown mode
        assert!(column_split("a\n1", "flip", "a", ",", "", 0, false, true, true, ",").is_err());
        // split: empty separator
        assert!(column_split("a\n1", "split", "a", "", "", 0, false, true, true, ",").is_err());
        // split: missing column
        assert!(
            column_split("a,b\n1,2", "split", "zzz", ",", "", 0, false, true, true, ",").is_err()
        );
        // concat: index out of range, no header
        assert!(
            column_split("1,2\n3,4", "concat", "9,1", ",", "", 0, false, true, false, ",").is_err()
        );
        // bad delimiter
        assert!(column_split("a\n1", "split", "a", ",", "", 0, false, true, true, "xx").is_err());
    }
}
