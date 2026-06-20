//! gizza-ai/csv-filter core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps. Keeps CSV rows matching a simple
//! `<column> <op> <value>` condition. Operators: == != < <= > >= contains.
//! Comparison is numeric when both the cell and the value parse as numbers, else
//! string; `contains` is a case-insensitive substring test.

#[derive(Clone, Copy, PartialEq, Debug)]
enum Op { Eq, Ne, Lt, Le, Gt, Ge, Contains }

struct Cond {
    column: String,
    op: Op,
    value: String,
}

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 { b[0] } else { return Err(format!("delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'")); }
        }
    })
}

fn parse_condition(c: &str) -> Result<Cond, String> {
    let c = c.trim();
    // `contains` keyword (whitespace-delimited) first.
    if let Some(i) = c.to_ascii_lowercase().find(" contains ") {
        let column = c[..i].trim().to_string();
        let value = c[i + " contains ".len()..].trim().to_string();
        if column.is_empty() {
            return Err("condition is missing a column before 'contains'".into());
        }
        return Ok(Cond { column, op: Op::Contains, value });
    }
    // Symbolic ops: scan for the first position that starts a 2-char op, else 1-char.
    let bytes = c.as_bytes();
    for i in 0..bytes.len() {
        let two = if i + 2 <= bytes.len() { &c[i..i + 2] } else { "" };
        let op = match two {
            "==" => Some((Op::Eq, 2)),
            "!=" => Some((Op::Ne, 2)),
            "<=" => Some((Op::Le, 2)),
            ">=" => Some((Op::Ge, 2)),
            _ => match &c[i..i + 1] {
                ">" => Some((Op::Gt, 1)),
                "<" => Some((Op::Lt, 1)),
                "=" => Some((Op::Eq, 1)),
                _ => None,
            },
        };
        if let Some((op, len)) = op {
            let column = c[..i].trim().to_string();
            let value = c[i + len..].trim().to_string();
            if column.is_empty() {
                return Err("condition is missing a column".into());
            }
            return Ok(Cond { column, op, value });
        }
    }
    Err("condition must be '<column> <op> <value>' with op one of == != < <= > >= contains".into())
}

fn resolve_column(name: &str, header: Option<&csv::StringRecord>) -> Result<usize, String> {
    if let Ok(n) = name.parse::<usize>() {
        if n == 0 {
            return Err("column index is 1-based (>= 1)".into());
        }
        return Ok(n - 1);
    }
    match header {
        Some(h) => h.iter().position(|c| c == name).ok_or_else(|| format!("column '{name}' not found in the header")),
        None => Err(format!("column '{name}' is not a number and there is no header to match names")),
    }
}

fn matches(cell: &str, op: Op, value: &str) -> bool {
    if op == Op::Contains {
        return cell.to_lowercase().contains(&value.to_lowercase());
    }
    match (cell.trim().parse::<f64>(), value.trim().parse::<f64>()) {
        (Ok(a), Ok(b)) => match op {
            Op::Eq => a == b, Op::Ne => a != b, Op::Lt => a < b,
            Op::Le => a <= b, Op::Gt => a > b, Op::Ge => a >= b, Op::Contains => unreachable!(),
        },
        _ => match op {
            Op::Eq => cell == value, Op::Ne => cell != value, Op::Lt => cell < value,
            Op::Le => cell <= value, Op::Gt => cell > value, Op::Ge => cell >= value, Op::Contains => unreachable!(),
        },
    }
}

/// Keep rows matching `condition`. The header row (when `has_header`) is always
/// kept. Returns the filtered CSV.
pub fn filter(data: &str, condition: &str, has_header: bool, delimiter: &str) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let cond = parse_condition(condition)?;
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new().delimiter(delim).has_headers(false).flexible(true).from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr.records().collect::<Result<_, _>>().map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Ok(String::new());
    }
    let header = if has_header { records.first() } else { None };
    let col = resolve_column(&cond.column, header)?;

    let mut wtr = csv::WriterBuilder::new().delimiter(delim).flexible(true).from_writer(vec![]);
    for (i, rec) in records.iter().enumerate() {
        if has_header && i == 0 {
            wtr.write_record(rec).map_err(|e| format!("CSV write error: {e}"))?;
            continue;
        }
        let cell = rec.get(col).unwrap_or("");
        if matches(cell, cond.op, &cond.value) {
            wtr.write_record(rec).map_err(|e| format!("CSV write error: {e}"))?;
        }
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_greater_than() {
        let d = "name,age\nAlice,30\nBob,25\nCarol,40";
        assert_eq!(filter(d, "age > 28", true, ",").unwrap(), "name,age\nAlice,30\nCarol,40\n");
    }

    #[test]
    fn string_equality() {
        let d = "name,city\nA,NYC\nB,LA\nC,NYC";
        assert_eq!(filter(d, "city == NYC", true, ",").unwrap(), "name,city\nA,NYC\nC,NYC\n");
    }

    #[test]
    fn contains_case_insensitive() {
        let d = "name\nAlice\nbob\nALBERT";
        assert_eq!(filter(d, "name contains al", true, ",").unwrap(), "name\nAlice\nALBERT\n");
    }

    #[test]
    fn no_space_condition_and_index_no_header() {
        let d = "a,1\nb,2\nc,3";
        // column by 1-based index 2, no header, ">=2"
        assert_eq!(filter(d, "2>=2", false, ",").unwrap(), "b,2\nc,3\n");
    }

    #[test]
    fn not_equal() {
        let d = "x\n1\n2\n1";
        assert_eq!(filter(d, "x != 1", true, ",").unwrap(), "x\n2\n");
    }

    #[test]
    fn errors() {
        assert!(filter("  ", "a==1", true, ",").is_err());
        assert!(filter("a,b\n1,2", "nope==1", true, ",").is_err());      // unknown column
        assert!(filter("a,b\n1,2", "a", true, ",").is_err());            // no operator
        assert!(filter("a,b\n1,2", "==1", true, ",").is_err());          // missing column
    }
}
