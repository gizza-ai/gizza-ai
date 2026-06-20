//! gizza-ai/csv-insert-column core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps. Inserts a new column at a chosen
//! 1-based position (or appends), filling every data row with a constant value;
//! the header row (if any) gets the new column name.

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => { let b = other.as_bytes(); if b.len()==1 { b[0] } else { return Err(format!("delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'")); } }
    })
}

/// Insert a column named `name` (header) filled with `value` (data rows) at the
/// 1-based `position` (clamped to [1, width+1]; 0 or empty → append at the end).
pub fn insert_column(data: &str, name: &str, value: &str, position: &str, has_header: bool, delimiter: &str) -> Result<String, String> {
    if data.trim().is_empty() { return Err("input is empty".into()); }
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new().delimiter(delim).has_headers(false).flexible(true).from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr.records().collect::<Result<_,_>>().map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() { return Err("no rows found".into()); }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);

    // Resolve position: 1-based; "" / "end" / 0 / > width+1 → append.
    let pos = {
        let p = position.trim();
        if p.is_empty() || p.eq_ignore_ascii_case("end") {
            width // 0-based insert index = end
        } else {
            let n: usize = p.parse().map_err(|_| format!("position must be a number or 'end', got '{p}'"))?;
            if n == 0 { width } else { (n - 1).min(width) }
        }
    };

    let mut wtr = csv::WriterBuilder::new().delimiter(delim).flexible(true).from_writer(vec![]);
    for (i, rec) in records.iter().enumerate() {
        let cell = if has_header && i == 0 { name } else { value };
        let mut fields: Vec<String> = rec.iter().map(|s| s.to_string()).collect();
        let at = pos.min(fields.len());
        fields.insert(at, cell.to_string());
        wtr.write_record(&fields).map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_by_default() {
        let d = "a,b\n1,2\n3,4";
        assert_eq!(insert_column(d, "c", "x", "end", true, ",").unwrap(), "a,b,c\n1,2,x\n3,4,x\n");
    }

    #[test]
    fn insert_at_front() {
        let d = "a,b\n1,2";
        assert_eq!(insert_column(d, "id", "0", "1", true, ",").unwrap(), "id,a,b\n0,1,2\n");
    }

    #[test]
    fn insert_in_middle() {
        let d = "a,c\n1,3";
        assert_eq!(insert_column(d, "b", "2", "2", true, ",").unwrap(), "a,b,c\n1,2,3\n");
    }

    #[test]
    fn no_header_fills_every_row() {
        let d = "1\n2";
        assert_eq!(insert_column(d, "ignored", "Z", "end", false, ",").unwrap(), "1,Z\n2,Z\n");
    }

    #[test]
    fn errors() {
        assert!(insert_column("  ", "c", "x", "end", true, ",").is_err());
        assert!(insert_column("a,b\n1,2", "c", "x", "abc", true, ",").is_err());
    }
}
