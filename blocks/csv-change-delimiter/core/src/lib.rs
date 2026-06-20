//! gizza-ai/csv-change-delimiter core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps. Re-serializes CSV/DSV data
//! from one field separator to another via the `csv` crate, so quoting is fixed
//! up correctly for the new delimiter (fields containing it get quoted, fields
//! that no longer need quoting are unquoted).

/// Resolve a delimiter string to one byte: a literal char or the words
/// tab/comma/semicolon/pipe.
fn delim_byte(d: &str, which: &str) -> Result<u8, String> {
    Ok(match d {
        "" => return Err(format!("{which} delimiter is required")),
        "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 { b[0] } else {
                return Err(format!("{which} delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'"));
            }
        }
    })
}

/// Re-save `data` parsed with `from` delimiter using the `to` delimiter.
pub fn change_delimiter(data: &str, from: &str, to: &str) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let from_b = delim_byte(from, "input")?;
    let to_b = delim_byte(to, "output")?;
    let mut rdr = csv::ReaderBuilder::new().delimiter(from_b).has_headers(false).flexible(true).from_reader(data.as_bytes());
    let mut wtr = csv::WriterBuilder::new().delimiter(to_b).flexible(true).from_writer(vec![]);
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("CSV parse error: {e}"))?;
        wtr.write_record(&rec).map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comma_to_tab() {
        assert_eq!(change_delimiter("a,b,c\n1,2,3", ",", "tab").unwrap(), "a\tb\tc\n1\t2\t3\n");
    }

    #[test]
    fn tab_to_semicolon() {
        assert_eq!(change_delimiter("a\tb\n1\t2", "tab", ";").unwrap(), "a;b\n1;2\n");
    }

    #[test]
    fn requotes_fields_containing_new_delimiter() {
        // value "x;y" must get quoted when output delim is ';'
        let out = change_delimiter("a,b\n\"x;y\",2", ",", ";").unwrap();
        assert_eq!(out, "a;b\n\"x;y\";2\n");
    }

    #[test]
    fn unquotes_when_no_longer_needed() {
        // "a,b" quoted under comma; switching to pipe removes the now-unneeded quotes
        let out = change_delimiter("\"a,b\",c", ",", "|").unwrap();
        assert_eq!(out, "a,b|c\n");
    }

    #[test]
    fn errors() {
        assert!(change_delimiter("   ", ",", ";").is_err());
        assert!(change_delimiter("a,b", "", ";").is_err());
        assert!(change_delimiter("a,b", ",", "xx").is_err());
    }
}
