//! csv-cleaner core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps. Cleans a CSV in one pass: trim cell
//! whitespace, drop fully-empty rows, fill empty cells, remove duplicate rows
//! (first kept), and normalize the delimiter + line endings. A first-row header
//! is optionally kept and exempted from row-drop / dedup / fill.

use std::collections::HashSet;

/// Parse a delimiter spec: a single char, or a friendly name.
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
                    "delimiter must be a single char or comma/tab/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Resolve the output delimiter: `"same"`/empty keeps the input delimiter,
/// otherwise a single char or a friendly name.
fn out_delim_byte(spec: &str, in_delim: u8) -> Result<u8, String> {
    match spec {
        "" | "same" => Ok(in_delim),
        other => delim_byte(other),
    }
}

/// Clean a CSV.
///
/// * `header` — treat the first row as a header: keep it, and never drop/dedup/fill it.
/// * `delimiter` — the INPUT field separator (char or comma/tab/semicolon/pipe).
/// * `output_delimiter` — target separator (`same`/char/name); `same` keeps the input's.
/// * `trim` — strip leading/trailing whitespace from every cell.
/// * `dedupe` — drop rows equal to an earlier row (first kept).
/// * `drop_empty_rows` — drop rows whose every cell is empty/whitespace.
/// * `empty_cells` — `"keep"` leaves empties as-is; `"fill"` replaces them with `fill_value`.
/// * `fill_value` — the value written into empty cells when `empty_cells == "fill"`.
/// * `line_ending` — `"lf"` (\n) or `"crlf"` (\r\n) for the output.
#[allow(clippy::too_many_arguments)]
pub fn clean(
    data: &str,
    header: bool,
    delimiter: &str,
    output_delimiter: &str,
    trim: bool,
    dedupe: bool,
    drop_empty_rows: bool,
    empty_cells: &str,
    fill_value: &str,
    line_ending: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let in_delim = delim_byte(delimiter)?;
    let out_delim = out_delim_byte(output_delimiter, in_delim)?;

    let fill = match empty_cells {
        "keep" => None,
        "fill" => Some(fill_value.to_string()),
        other => {
            return Err(format!("empty_cells must be 'keep' or 'fill', got '{other}'"));
        }
    };

    let crlf = match line_ending {
        "" | "lf" => false,
        "crlf" => true,
        other => {
            return Err(format!("line_ending must be 'lf' or 'crlf', got '{other}'"));
        }
    };

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(in_delim)
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

    let terminator = if crlf {
        csv::Terminator::CRLF
    } else {
        csv::Terminator::Any(b'\n')
    };
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(out_delim)
        .terminator(terminator)
        .flexible(true)
        .from_writer(vec![]);

    let mut seen: HashSet<String> = HashSet::new();
    for (i, rec) in records.iter().enumerate() {
        let is_header = header && i == 0;

        // Build the cleaned field list for this row.
        let mut fields: Vec<String> = rec
            .iter()
            .map(|f| if trim { f.trim().to_string() } else { f.to_string() })
            .collect();

        if !is_header {
            if drop_empty_rows && fields.iter().all(|f| f.trim().is_empty()) {
                continue;
            }
            if let Some(fv) = &fill {
                for f in fields.iter_mut() {
                    if f.trim().is_empty() {
                        *f = fv.clone();
                    }
                }
            }
            if dedupe {
                let key = fields.join("\u{1}");
                if !seen.insert(key) {
                    continue;
                }
            }
        }

        wtr.write_record(&fields)
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

    // Convenience wrapper with the default preset.
    fn clean_default(data: &str) -> Result<String, String> {
        clean(data, true, ",", "same", true, true, true, "keep", "", "lf")
    }

    #[test]
    fn happy_full_clean() {
        // Trims cells, drops the all-empty row, dedupes Bob, keeps the header.
        let d = "name , age\n Alice ,30\nBob,25\n,,\nBob,25\nCarol, 40 ";
        assert_eq!(
            clean_default(d).unwrap(),
            "name,age\nAlice,30\nBob,25\nCarol,40\n"
        );
    }

    #[test]
    fn fill_empty_cells() {
        let d = "a,b,c\n1,,3\n,5,";
        let got = clean(d, true, ",", "same", true, false, false, "fill", "N/A", "lf").unwrap();
        assert_eq!(got, "a,b,c\n1,N/A,3\nN/A,5,N/A\n");
    }

    #[test]
    fn change_delimiter_and_crlf() {
        let d = "a,b\n1,2";
        let got = clean(d, true, ",", "semicolon", false, false, false, "keep", "", "crlf").unwrap();
        assert_eq!(got, "a;b\r\n1;2\r\n");
    }

    #[test]
    fn tab_input_and_no_header_dedup() {
        // No header → the first row participates in dedup too.
        let d = "x\ty\nx\ty\nz\tw";
        let got = clean(d, false, "tab", "comma", true, true, true, "keep", "", "lf").unwrap();
        assert_eq!(got, "x,y\nz,w\n");
    }

    #[test]
    fn errors() {
        assert!(clean_default("   ").is_err()); // empty input
        assert!(clean("a,b\n1,2", true, "nope", "same", true, true, true, "keep", "", "lf").is_err()); // bad delim
        assert!(clean("a,b\n1,2", true, ",", "same", true, true, true, "bogus", "", "lf").is_err()); // bad empty_cells
        assert!(clean("a,b\n1,2", true, ",", "same", true, true, true, "keep", "", "mac").is_err()); // bad line_ending
    }
}
