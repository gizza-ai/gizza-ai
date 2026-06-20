//! gizza-ai/csv-merge core — pure compute, shared by the chat skill block. No
//! wafer/wasm-bindgen deps. Concatenates (stacks) multiple CSVs into one. With a
//! header, the output uses the UNION of all input headers (first-seen order) and
//! maps each file's rows into it by column name (missing columns are blank).
//! Without a header, rows are stacked positionally (padded to the widest row).

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => { let b = other.as_bytes(); if b.len()==1 { b[0] } else { return Err(format!("delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'")); } }
    })
}

fn parse(data: &str, delim: u8) -> Result<Vec<csv::StringRecord>, String> {
    let mut rdr = csv::ReaderBuilder::new().delimiter(delim).has_headers(false).flexible(true).from_reader(data.as_bytes());
    rdr.records().collect::<Result<_,_>>().map_err(|e| format!("CSV parse error: {e}"))
}

/// Concatenate `files` (each a CSV text). See module docs for header-union vs
/// positional stacking.
pub fn merge(files: &[String], has_header: bool, delimiter: &str) -> Result<String, String> {
    let files: Vec<&String> = files.iter().filter(|f| !f.trim().is_empty()).collect();
    if files.len() < 2 {
        return Err("need at least 2 non-empty CSV inputs to merge".into());
    }
    let delim = delim_byte(delimiter)?;
    let mut wtr = csv::WriterBuilder::new().delimiter(delim).flexible(true).from_writer(vec![]);

    if has_header {
        // Union of headers in first-seen order.
        let mut union: Vec<String> = Vec::new();
        let parsed: Vec<Vec<csv::StringRecord>> = files.iter().map(|f| parse(f, delim)).collect::<Result<_,_>>()?;
        for recs in &parsed {
            if let Some(h) = recs.first() {
                for name in h.iter() {
                    if !union.iter().any(|u| u == name) { union.push(name.to_string()); }
                }
            }
        }
        if union.is_empty() { return Err("no header columns found".into()); }
        wtr.write_record(&union).map_err(|e| format!("CSV write error: {e}"))?;
        for recs in &parsed {
            let header = match recs.first() { Some(h) => h, None => continue };
            // map each input column index -> union index
            let idx: Vec<usize> = header.iter().map(|name| union.iter().position(|u| u == name).unwrap()).collect();
            for rec in recs.iter().skip(1) {
                let mut out = vec![String::new(); union.len()];
                for (ci, cell) in rec.iter().enumerate() {
                    if let Some(&ui) = idx.get(ci) { out[ui] = cell.to_string(); }
                }
                wtr.write_record(&out).map_err(|e| format!("CSV write error: {e}"))?;
            }
        }
    } else {
        for f in &files {
            for rec in parse(f, delim)? {
                wtr.write_record(&rec).map_err(|e| format!("CSV write error: {e}"))?;
            }
        }
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concat_same_header() {
        let a = "name,age\nAlice,30".to_string();
        let b = "name,age\nBob,25".to_string();
        assert_eq!(merge(&[a, b], true, ",").unwrap(), "name,age\nAlice,30\nBob,25\n");
    }

    #[test]
    fn header_union_aligns_columns() {
        let a = "name,age\nAlice,30".to_string();
        let b = "name,city\nBob,LA".to_string();
        // union: name, age, city
        assert_eq!(merge(&[a, b], true, ",").unwrap(), "name,age,city\nAlice,30,\nBob,,LA\n");
    }

    #[test]
    fn no_header_stacks_rows() {
        let a = "1,2\n3,4".to_string();
        let b = "5,6".to_string();
        assert_eq!(merge(&[a, b], false, ",").unwrap(), "1,2\n3,4\n5,6\n");
    }

    #[test]
    fn errors() {
        assert!(merge(&["a,b\n1,2".to_string()], true, ",").is_err()); // <2 inputs
        assert!(merge(&["a,b\n1,2".to_string(), "  ".to_string()], true, ",").is_err()); // 2nd empty -> <2 non-empty
    }
}
