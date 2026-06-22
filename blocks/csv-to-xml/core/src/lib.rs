//! gizza-ai/csv-to-xml core — convert a CSV table into XML records, using the
//! header names as element tags. Pure-Rust (`csv`). No wafer/wasm-bindgen deps.

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

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Turn an arbitrary header into a valid XML element name: keep letters, digits,
/// '_', '-', '.'; replace the rest with '_'; ensure it starts with a letter or
/// '_'. Empty → "col".
fn xml_name(raw: &str) -> String {
    let mut out = String::new();
    for c in raw.trim().chars() {
        if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        return "col".to_string();
    }
    let first = out.chars().next().unwrap();
    if !(first.is_alphabetic() || first == '_') {
        out.insert(0, '_');
    }
    out
}

/// Convert `data` (CSV) into XML. `root` wraps everything, each record is a `row`
/// element, and each field is `<tag>value</tag>` with the tag from the header
/// (or `col1`… when `has_header` is false).
pub fn to_xml(data: &str, root: &str, row: &str, has_header: bool, delimiter: &str) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let root = xml_name(if root.trim().is_empty() { "rows" } else { root });
    let row = xml_name(if row.trim().is_empty() { "row" } else { row });
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> =
        rdr.records().collect::<Result<_, _>>().map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Err("no rows found".into());
    }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);

    let (tags, body): (Vec<String>, &[csv::StringRecord]) = if has_header {
        let h = &records[0];
        ((0..width).map(|i| xml_name(h.get(i).unwrap_or(""))).collect(), &records[1..])
    } else {
        ((1..=width).map(|i| format!("col{i}")).collect(), &records[..])
    };

    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!("<{root}>\n"));
    for rec in body {
        out.push_str(&format!("  <{row}>\n"));
        for (i, tag) in tags.iter().enumerate() {
            let v = rec.get(i).unwrap_or("");
            out.push_str(&format!("    <{tag}>{}</{tag}>\n", xml_escape(v)));
        }
        out.push_str(&format!("  </{row}>\n"));
    }
    out.push_str(&format!("</{root}>"));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_records() {
        let out = to_xml("name,age\nAda,36", "people", "person", true, ",").unwrap();
        assert!(out.contains("<people>"));
        assert!(out.contains("<person>"));
        assert!(out.contains("<name>Ada</name>"));
        assert!(out.contains("<age>36</age>"));
        assert!(out.trim_end().ends_with("</people>"));
    }

    #[test]
    fn sanitizes_header_to_xml_name() {
        let out = to_xml("first name,1st\nAda,x", "rows", "row", true, ",").unwrap();
        assert!(out.contains("<first_name>Ada</first_name>"));
        assert!(out.contains("<_1st>x</_1st>")); // leading digit gets '_'
    }

    #[test]
    fn escapes_values() {
        let out = to_xml("v\n<a&b>", "rows", "row", true, ",").unwrap();
        assert!(out.contains("<v>&lt;a&amp;b&gt;</v>"));
    }

    #[test]
    fn no_header_uses_col_tags() {
        let out = to_xml("1,2\n3,4", "rows", "row", false, ",").unwrap();
        assert!(out.contains("<col1>1</col1>"));
        assert!(out.contains("<col2>4</col2>"));
    }

    #[test]
    fn tab_delimiter_and_default_names() {
        let out = to_xml("a\tb\n1\t2", "", "", true, "tab").unwrap();
        assert!(out.contains("<rows>"));
        assert!(out.contains("<row>"));
        assert!(out.contains("<a>1</a>"));
    }

    #[test]
    fn errors() {
        assert!(to_xml("", "rows", "row", true, ",").is_err());
        assert!(to_xml("a,b\n1,2", "rows", "row", true, "xx").is_err());
    }
}
