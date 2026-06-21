//! gizza-ai/csv-transpose core — transpose a CSV (swap rows and columns), so the
//! first column becomes the header row and vice versa. Pure-Rust (`csv`). No
//! wafer/wasm-bindgen deps. Ragged rows are padded with empty cells.

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

/// Transpose `data` (CSV). The output has one row per input column and one column
/// per input row.
pub fn transpose(data: &str, delimiter: &str) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let rows: Vec<csv::StringRecord> =
        rdr.records().collect::<Result<_, _>>().map_err(|e| format!("CSV parse error: {e}"))?;
    if rows.is_empty() {
        return Err("no rows found".into());
    }
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);

    let mut wtr = csv::WriterBuilder::new().delimiter(delim).flexible(true).from_writer(vec![]);
    for c in 0..width {
        let out: Vec<&str> = rows.iter().map(|r| r.get(c).unwrap_or("")).collect();
        wtr.write_record(&out).map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square() {
        let out = transpose("a,b\n1,2", ",").unwrap();
        assert_eq!(out, "a,1\nb,2\n");
    }

    #[test]
    fn rectangular() {
        // 3 rows x 2 cols -> 2 rows x 3 cols
        let out = transpose("a,b\n1,2\n3,4", ",").unwrap();
        assert_eq!(out, "a,1,3\nb,2,4\n");
    }

    #[test]
    fn header_becomes_first_column() {
        let out = transpose("name,age\nAda,36\nBo,40", ",").unwrap();
        assert_eq!(out, "name,Ada,Bo\nage,36,40\n");
    }

    #[test]
    fn ragged_padded() {
        // second row is short; missing cell becomes empty.
        let out = transpose("a,b,c\n1,2", ",").unwrap();
        assert_eq!(out, "a,1\nb,2\nc,\n");
    }

    #[test]
    fn double_transpose_roundtrips() {
        let d = "a,b\n1,2\n3,4";
        let once = transpose(d, ",").unwrap();
        let twice = transpose(once.trim_end(), ",").unwrap();
        assert_eq!(twice.trim_end(), d);
    }

    #[test]
    fn tab_delimiter() {
        let out = transpose("a\tb\n1\t2", "tab").unwrap();
        assert_eq!(out, "a\t1\nb\t2\n");
    }

    #[test]
    fn errors() {
        assert!(transpose("", ",").is_err());
        assert!(transpose("a,b", "xx").is_err());
    }
}
