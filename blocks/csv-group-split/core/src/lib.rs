//! gizza-ai/csv-group-split core — pure compute, shared by the chat skill block.
//! No wafer/wasm-bindgen deps. Splits one CSV into per-group CSV strings, one per
//! distinct value in a key column (each carries the header). The block zips them.

use std::collections::HashMap;

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => { let b = other.as_bytes(); if b.len()==1 { b[0] } else { return Err(format!("delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'")); } }
    })
}

fn resolve(name: &str, header: Option<&csv::StringRecord>) -> Result<usize, String> {
    if let Ok(n) = name.parse::<usize>() {
        if n == 0 { return Err("column index is 1-based (>= 1)".into()); }
        return Ok(n - 1);
    }
    match header {
        Some(h) => h.iter().position(|c| c == name).ok_or_else(|| format!("column '{name}' not found in the header")),
        None => Err(format!("column '{name}' is not a number and there is no header to match names")),
    }
}

/// Make a key value safe as a file stem; blank → "_empty".
fn safe_stem(v: &str) -> String {
    let s: String = v.chars().map(|c| if c.is_alphanumeric() || c=='-' || c=='_' || c=='.' || c==' ' { c } else { '_' }).collect();
    let s = s.trim().to_string();
    if s.is_empty() { "_empty".to_string() } else { s }
}

/// Split `data` on the `key` column. Returns `(filename, csv_text)` per distinct
/// value, in first-seen order. Each output CSV includes the header (if any).
pub fn split(data: &str, key: &str, has_header: bool, delimiter: &str) -> Result<Vec<(String, String)>, String> {
    if data.trim().is_empty() { return Err("input is empty".into()); }
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new().delimiter(delim).has_headers(false).flexible(true).from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr.records().collect::<Result<_,_>>().map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() { return Err("no rows found".into()); }
    let header = if has_header { records.first().cloned() } else { None };
    let kcol = resolve(key, header.as_ref())?;

    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<csv::StringRecord>> = HashMap::new();
    for rec in records.iter().skip(if has_header { 1 } else { 0 }) {
        let val = rec.get(kcol).unwrap_or("").to_string();
        groups.entry(val.clone()).or_insert_with(|| { order.push(val.clone()); Vec::new() }).push(rec.clone());
    }
    if order.is_empty() { return Err("no data rows to split".into()); }

    // Build per-group CSV text, with collision-safe filenames.
    let mut used: HashMap<String, u32> = HashMap::new();
    let mut out = Vec::new();
    for val in &order {
        let mut stem = safe_stem(val);
        let n = used.entry(stem.clone()).or_insert(0);
        *n += 1;
        if *n > 1 { stem = format!("{stem} ({})", *n); }
        let mut wtr = csv::WriterBuilder::new().delimiter(delim).flexible(true).from_writer(vec![]);
        if let Some(h) = &header { wtr.write_record(h).map_err(|e| format!("CSV write error: {e}"))?; }
        for rec in &groups[val] { wtr.write_record(rec).map_err(|e| format!("CSV write error: {e}"))?; }
        let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
        out.push((format!("{stem}.csv"), String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))?));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_by_key_with_header() {
        let d = "dept,name\nA,Alice\nB,Bob\nA,Carol";
        let parts = split(d, "dept", true, ",").unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].0, "A.csv");
        assert_eq!(parts[0].1, "dept,name\nA,Alice\nA,Carol\n");
        assert_eq!(parts[1].0, "B.csv");
        assert_eq!(parts[1].1, "dept,name\nB,Bob\n");
    }

    #[test]
    fn no_header_by_index() {
        let d = "x,1\ny,2\nx,3";
        let parts = split(d, "1", false, ",").unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], ("x.csv".to_string(), "x,1\nx,3\n".to_string()));
    }

    #[test]
    fn unsafe_key_value_sanitized() {
        let d = "k,v\na/b,1";
        let parts = split(d, "k", true, ",").unwrap();
        assert_eq!(parts[0].0, "a_b.csv");
    }

    #[test]
    fn errors() {
        assert!(split("  ", "k", true, ",").is_err());
        assert!(split("a,b\n1,2", "nope", true, ",").is_err());
        assert!(split("a,b\n1,2", "k", false, ",").is_err());
    }
}
