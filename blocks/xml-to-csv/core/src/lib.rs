//! gizza-ai/xml-to-csv core — flatten repeated XML elements into a CSV table.
//!
//! The "record" element is repeated (e.g. `<book>` inside `<catalog>`); each
//! record becomes one CSV row. Columns are inferred, in first-seen order, from
//! the record's child element tags and (optionally) its attributes. Pure-Rust
//! (`quick-xml` + `csv`); no wafer/wasm-bindgen deps.

use std::collections::BTreeMap;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

/// Strip an `ns:local` prefix, returning the local name.
fn local_name(qname: &[u8]) -> String {
    let s = String::from_utf8_lossy(qname);
    match s.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => s.to_string(),
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

/// Decode XML entities in attribute / text content (e.g. `&amp;` → `&`).
fn unescape(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    // quick-xml's unescape works on &str; fall back to the raw text if it fails.
    quick_xml::escape::unescape(&s)
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| s.into_owned())
}

/// One parsed record: ordered list of (column, value) pairs.
type Record = Vec<(String, String)>;

/// Detect the repeated record tag: the local name that occurs most often as a
/// *direct child of the root* element. Ties break to the first-seen tag.
fn detect_record(xml: &str) -> Result<Option<String>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut depth: i32 = 0;
    // Count direct children of the root (depth becomes 1 after we enter root).
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut buf_depth_start: Option<i32> = None; // root depth marker

    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                depth += 1;
                if buf_depth_start.is_none() {
                    buf_depth_start = Some(depth); // this is the root level
                } else if depth == buf_depth_start.unwrap() + 1 {
                    let name = local_name(e.name().as_ref());
                    if !counts.contains_key(&name) {
                        order.push(name.clone());
                    }
                    *counts.entry(name).or_insert(0) += 1;
                }
            }
            Ok(Event::Empty(e)) => {
                if let Some(root_d) = buf_depth_start {
                    if depth == root_d {
                        let name = local_name(e.name().as_ref());
                        if !counts.contains_key(&name) {
                            order.push(name.clone());
                        }
                        *counts.entry(name).or_insert(0) += 1;
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
            }
            Ok(_) => {}
            Err(e) => {
                return Err(format!(
                    "XML is not well-formed at byte {}: {e}",
                    reader.error_position()
                ));
            }
        }
    }

    // Pick the most frequent direct child (first-seen on ties).
    let best = order
        .into_iter()
        .max_by_key(|name| counts.get(name).copied().unwrap_or(0));
    Ok(best)
}

/// Parse every `record` element into a flat Record. `include_attrs` adds each
/// attribute as a column named `@attr` (on the record element) or `child@attr`
/// (on a child element). Repeated child tags get `tag`, `tag.2`, `tag.3`… .
fn parse_records(
    xml: &str,
    record: &str,
    include_attrs: bool,
) -> Result<Vec<Record>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut out: Vec<Record> = Vec::new();
    let mut depth: i32 = 0;

    // State while inside a record element.
    let mut in_record_at: Option<i32> = None; // depth at which the record opened
    let mut cur: Record = Vec::new();
    let mut seen_counts: BTreeMap<String, usize> = BTreeMap::new();
    // Element-tag path BELOW the record (dot-notation column prefix), one entry
    // per open element. e.g. inside <record><address><city> → ["address","city"].
    let mut path: Vec<String> = Vec::new();
    // For the currently-open innermost element: collected text + whether it has
    // had an element child (a non-leaf element does not emit its own column).
    let mut text = String::new();
    let mut has_child_elem: Vec<bool> = Vec::new();

    let flush =
        |col: &str, val: &str, cur: &mut Record, seen: &mut BTreeMap<String, usize>| {
            let n = seen.entry(col.to_string()).or_insert(0);
            *n += 1;
            let name = if *n == 1 {
                col.to_string()
            } else {
                format!("{col}.{n}")
            };
            cur.push((name, val.to_string()));
        };

    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                depth += 1;
                let name = local_name(e.name().as_ref());
                if in_record_at.is_none() && name == record {
                    // Open a fresh record.
                    in_record_at = Some(depth);
                    cur = Vec::new();
                    seen_counts = BTreeMap::new();
                    path.clear();
                    has_child_elem.clear();
                    text.clear();
                    if include_attrs {
                        for a in e.attributes().flatten() {
                            let k = format!("@{}", local_name(a.key.as_ref()));
                            let v = unescape(&a.value);
                            flush(&k, &v, &mut cur, &mut seen_counts);
                        }
                    }
                } else if in_record_at.is_some() {
                    // A nested element inside the record.
                    if let Some(last) = has_child_elem.last_mut() {
                        *last = true; // parent now has an element child → not a leaf
                    }
                    path.push(name.clone());
                    has_child_elem.push(false);
                    text.clear();
                    if include_attrs {
                        let prefix = path.join(".");
                        for a in e.attributes().flatten() {
                            let k = format!("{prefix}@{}", local_name(a.key.as_ref()));
                            let v = unescape(&a.value);
                            flush(&k, &v, &mut cur, &mut seen_counts);
                        }
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref());
                if in_record_at.is_none() && name == record {
                    // self-closing record: attributes only.
                    cur = Vec::new();
                    seen_counts = BTreeMap::new();
                    if include_attrs {
                        for a in e.attributes().flatten() {
                            let k = format!("@{}", local_name(a.key.as_ref()));
                            let v = unescape(&a.value);
                            flush(&k, &v, &mut cur, &mut seen_counts);
                        }
                    }
                    out.push(std::mem::take(&mut cur));
                } else if in_record_at.is_some() {
                    // A self-closing leaf element inside the record → empty value.
                    if let Some(last) = has_child_elem.last_mut() {
                        *last = true;
                    }
                    path.push(name.clone());
                    let col = path.join(".");
                    if include_attrs {
                        for a in e.attributes().flatten() {
                            let k = format!("{col}@{}", local_name(a.key.as_ref()));
                            let v = unescape(&a.value);
                            flush(&k, &v, &mut cur, &mut seen_counts);
                        }
                    }
                    flush(&col, "", &mut cur, &mut seen_counts);
                    path.pop();
                }
            }
            Ok(Event::Text(t)) => {
                if in_record_at.is_some() && !path.is_empty() {
                    text.push_str(&unescape(t.as_ref()));
                }
            }
            Ok(Event::CData(t)) => {
                if in_record_at.is_some() && !path.is_empty() {
                    text.push_str(&String::from_utf8_lossy(t.as_ref()));
                }
            }
            Ok(Event::End(_)) => {
                if let Some(rd) = in_record_at {
                    if depth == rd {
                        // closing the record itself.
                        out.push(std::mem::take(&mut cur));
                        in_record_at = None;
                    } else if !path.is_empty() {
                        // closing a nested element: emit a column only if it's a
                        // leaf (no element children) — otherwise its children
                        // already produced dot-notation columns.
                        let is_leaf = !has_child_elem.pop().unwrap_or(false);
                        let col = path.join(".");
                        if is_leaf {
                            flush(&col, text.trim(), &mut cur, &mut seen_counts);
                        }
                        path.pop();
                        text.clear();
                    }
                }
                depth -= 1;
            }
            Ok(_) => {}
            Err(e) => {
                return Err(format!(
                    "XML is not well-formed at byte {}: {e}",
                    reader.error_position()
                ));
            }
        }
    }

    Ok(out)
}

/// Flatten repeated XML elements into a CSV table.
///
/// - `record`: the repeated element tag to treat as a row. Empty → auto-detect
///   (the most frequent direct child of the root element).
/// - `include_attrs`: when true, attributes become columns (`@attr` on the
///   record, `child@attr` on a child element).
/// - `delimiter`: a single char or `comma`/`tab`/`semicolon`/`pipe`.
///
/// Nested child elements flatten to dot-notation columns (`address.city`); a
/// non-leaf element does not get its own column. Columns are the union of all
/// records' keys, in first-seen order. A record missing a column gets an empty
/// cell.
pub fn to_csv(
    xml: &str,
    record: &str,
    include_attrs: bool,
    delimiter: &str,
) -> Result<String, String> {
    if xml.trim().is_empty() {
        return Err("input is empty".into());
    }
    let delim = delim_byte(delimiter)?;

    let record_tag = if record.trim().is_empty() {
        detect_record(xml)?.ok_or_else(|| {
            "could not auto-detect a repeated record element; pass the record tag explicitly"
                .to_string()
        })?
    } else {
        record.trim().to_string()
    };

    let records = parse_records(xml, &record_tag, include_attrs)?;
    if records.is_empty() {
        return Err(format!(
            "no <{record_tag}> records found in the XML"
        ));
    }

    // Build the column order: first-seen across all records.
    let mut columns: Vec<String> = Vec::new();
    for rec in &records {
        for (k, _) in rec {
            if !columns.contains(k) {
                columns.push(k.clone());
            }
        }
    }

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .from_writer(Vec::new());
    wtr.write_record(&columns)
        .map_err(|e| format!("CSV write error: {e}"))?;
    for rec in &records {
        let map: BTreeMap<&str, &str> =
            rec.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let row: Vec<&str> = columns.iter().map(|c| *map.get(c.as_str()).unwrap_or(&"")).collect();
        wtr.write_record(&row)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV flush error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("output is not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_autodetect() {
        let xml = r#"<catalog>
            <book><title>Dune</title><author>Herbert</author></book>
            <book><title>Hyperion</title><author>Simmons</author></book>
        </catalog>"#;
        let csv = to_csv(xml, "", false, ",").unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "title,author");
        assert_eq!(lines[1], "Dune,Herbert");
        assert_eq!(lines[2], "Hyperion,Simmons");
    }

    #[test]
    fn explicit_record_tag() {
        let xml = r#"<data><row><a>1</a><b>2</b></row><row><a>3</a><b>4</b></row></data>"#;
        let csv = to_csv(xml, "row", false, ",").unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "a,b");
        assert_eq!(lines[1], "1,2");
        assert_eq!(lines[2], "3,4");
    }

    #[test]
    fn attributes_as_columns() {
        let xml = r#"<users>
            <user id="1"><name>Ada</name></user>
            <user id="2"><name>Bo</name></user>
        </users>"#;
        let csv = to_csv(xml, "user", true, ",").unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "@id,name");
        assert_eq!(lines[1], "1,Ada");
        assert_eq!(lines[2], "2,Bo");
    }

    #[test]
    fn attributes_ignored_when_off() {
        let xml = r#"<users><user id="1"><name>Ada</name></user></users>"#;
        let csv = to_csv(xml, "user", false, ",").unwrap();
        assert_eq!(csv.lines().next().unwrap(), "name");
    }

    #[test]
    fn child_attribute_column() {
        let xml = r#"<rows><row><price currency="USD">9.99</price></row></rows>"#;
        let csv = to_csv(xml, "row", true, ",").unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "price@currency,price");
        assert_eq!(lines[1], "USD,9.99");
    }

    #[test]
    fn union_of_columns_with_missing_cells() {
        let xml = r#"<rows>
            <row><a>1</a><b>2</b></row>
            <row><a>3</a><c>9</c></row>
        </rows>"#;
        let csv = to_csv(xml, "row", false, ",").unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "a,b,c");
        assert_eq!(lines[1], "1,2,");
        assert_eq!(lines[2], "3,,9");
    }

    #[test]
    fn repeated_child_tags_suffixed() {
        let xml = r#"<rows><row><tag>x</tag><tag>y</tag></row></rows>"#;
        let csv = to_csv(xml, "row", false, ",").unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "tag,tag.2");
        assert_eq!(lines[1], "x,y");
    }

    #[test]
    fn entities_are_decoded() {
        let xml = r#"<rows><row><v>a &amp; b &lt;c&gt;</v></row></rows>"#;
        let csv = to_csv(xml, "row", false, ",").unwrap();
        assert_eq!(csv.lines().nth(1).unwrap(), "a & b <c>");
    }

    #[test]
    fn values_with_delimiter_are_quoted() {
        let xml = r#"<rows><row><v>a,b</v></row></rows>"#;
        let csv = to_csv(xml, "row", false, ",").unwrap();
        assert_eq!(csv.lines().nth(1).unwrap(), "\"a,b\"");
    }

    #[test]
    fn cdata_text_kept() {
        let xml = r#"<rows><row><html><![CDATA[<b>hi</b>]]></html></row></rows>"#;
        let csv = to_csv(xml, "row", false, ",").unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "html");
        assert_eq!(lines[1], "<b>hi</b>");
    }

    #[test]
    fn tab_delimiter() {
        let xml = r#"<rows><row><a>1</a><b>2</b></row></rows>"#;
        let csv = to_csv(xml, "row", false, "tab").unwrap();
        assert_eq!(csv.lines().next().unwrap(), "a\tb");
    }

    #[test]
    fn namespaced_tags_use_local_name() {
        let xml = r#"<ns:rows xmlns:ns="http://e"><ns:row><ns:a>1</ns:a></ns:row></ns:rows>"#;
        let csv = to_csv(xml, "row", false, ",").unwrap();
        assert_eq!(csv.lines().next().unwrap(), "a");
    }

    #[test]
    fn self_closing_child_is_empty_cell() {
        let xml = r#"<rows><row><a>1</a><b/></row></rows>"#;
        let csv = to_csv(xml, "row", false, ",").unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "a,b");
        assert_eq!(lines[1], "1,");
    }

    #[test]
    fn nested_elements_use_dot_notation() {
        let xml = r#"<users>
            <user><name>Ada</name><address><city>Paris</city><zip>75001</zip></address></user>
            <user><name>Bo</name><address><city>Rome</city><zip>00100</zip></address></user>
        </users>"#;
        let csv = to_csv(xml, "user", false, ",").unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "name,address.city,address.zip");
        assert_eq!(lines[1], "Ada,Paris,75001");
        assert_eq!(lines[2], "Bo,Rome,00100");
    }

    #[test]
    fn deeply_nested_attribute_path() {
        let xml = r#"<rows><row><a><b unit="kg">5</b></a></row></rows>"#;
        let csv = to_csv(xml, "row", true, ",").unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "a.b@unit,a.b");
        assert_eq!(lines[1], "kg,5");
    }

    #[test]
    fn empty_input_errors() {
        assert!(to_csv("", "", false, ",").is_err());
    }

    #[test]
    fn no_records_errors() {
        let xml = r#"<root><only>1</only></root>"#;
        assert!(to_csv(xml, "missing", false, ",").is_err());
    }

    #[test]
    fn malformed_xml_errors() {
        assert!(to_csv("<rows><row><a>1</a>", "row", false, ",").is_err());
    }

    #[test]
    fn bad_delimiter_errors() {
        let xml = r#"<rows><row><a>1</a></row></rows>"#;
        assert!(to_csv(xml, "row", false, "xx").is_err());
    }
}
