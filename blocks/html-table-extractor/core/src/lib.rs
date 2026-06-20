//! gizza-ai/html-table-extractor core — extract `<table>` data from HTML and
//! emit it as CSV or JSON. No wafer/wasm-bindgen deps. Uses `scraper`
//! (html5ever-based, wasm32-safe) to parse; `csv` for safe CSV escaping;
//! `serde_json` for JSON.

use scraper::{Html, Selector};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Csv,
    Json,
}

pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "csv" => Ok(Format::Csv),
        "json" => Ok(Format::Json),
        other => Err(format!("format {other:?} not supported (csv|json)")),
    }
}

/// Collapse runs of whitespace (incl. newlines) in a cell to single spaces.
fn clean_cell(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract the rows (each a vector of cell strings) of the `index`-th table.
pub fn extract_rows(html: &str, index: usize) -> Result<Vec<Vec<String>>, String> {
    if html.trim().is_empty() {
        return Err("input HTML is empty".into());
    }
    let doc = Html::parse_fragment(html);
    let table_sel = Selector::parse("table").map_err(|e| format!("selector error: {e}"))?;
    let row_sel = Selector::parse("tr").map_err(|e| format!("selector error: {e}"))?;
    let cell_sel = Selector::parse("th,td").map_err(|e| format!("selector error: {e}"))?;

    let tables: Vec<_> = doc.select(&table_sel).collect();
    if tables.is_empty() {
        return Err("no <table> elements found in the HTML".into());
    }
    let table = tables
        .get(index)
        .ok_or_else(|| format!("table index {index} out of range (found {} table(s))", tables.len()))?;

    let mut rows: Vec<Vec<String>> = Vec::new();
    for tr in table.select(&row_sel) {
        let cells: Vec<String> = tr
            .select(&cell_sel)
            .map(|c| clean_cell(&c.text().collect::<String>()))
            .collect();
        if !cells.is_empty() {
            rows.push(cells);
        }
    }
    if rows.is_empty() {
        return Err("the selected table has no rows".into());
    }
    Ok(rows)
}

/// Serialize rows to CSV (RFC-4180 escaping via the `csv` crate).
fn rows_to_csv(rows: &[Vec<String>]) -> Result<String, String> {
    let mut wtr = csv::WriterBuilder::new().flexible(true).from_writer(vec![]);
    for row in rows {
        wtr.write_record(row).map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

/// Serialize rows to JSON. With `header`, the first row is keys and each later
/// row becomes an object (missing cells omitted, extra cells keyed by index);
/// otherwise an array of arrays.
fn rows_to_json(rows: &[Vec<String>], header: bool) -> Result<String, String> {
    use serde_json::{Map, Value};
    if header && rows.len() >= 1 {
        let keys = &rows[0];
        let objects: Vec<Value> = rows[1..]
            .iter()
            .map(|row| {
                let mut obj = Map::new();
                for (i, cell) in row.iter().enumerate() {
                    let key = keys.get(i).cloned().unwrap_or_else(|| format!("column{}", i + 1));
                    obj.insert(key, Value::String(cell.clone()));
                }
                Value::Object(obj)
            })
            .collect();
        serde_json::to_string_pretty(&objects).map_err(|e| format!("JSON error: {e}"))
    } else {
        serde_json::to_string_pretty(rows).map_err(|e| format!("JSON error: {e}"))
    }
}

/// Extract the `index`-th table and render it as `format`. `header` only affects
/// JSON shape (array-of-objects vs array-of-arrays).
pub fn extract(html: &str, index: usize, format: Format, header: bool) -> Result<String, String> {
    let rows = extract_rows(html, index)?;
    match format {
        Format::Csv => rows_to_csv(&rows),
        Format::Json => rows_to_json(&rows, header),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML: &str = r#"<div>
        <table>
            <tr><th>Name</th><th>Age</th></tr>
            <tr><td>Alice</td><td>30</td></tr>
            <tr><td>Bob</td><td>25</td></tr>
        </table>
        <table><tr><td>second</td><td>table</td></tr></table>
    </div>"#;

    #[test]
    fn extracts_first_table_to_csv() {
        let csv = extract(HTML, 0, Format::Csv, true).unwrap();
        assert_eq!(csv, "Name,Age\nAlice,30\nBob,25\n");
    }

    #[test]
    fn extracts_to_json_objects_with_header() {
        let json = extract(HTML, 0, Format::Json, true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v[0]["Name"], "Alice");
        assert_eq!(v[0]["Age"], "30");
        assert_eq!(v[1]["Name"], "Bob");
        assert_eq!(v.as_array().unwrap().len(), 2);
    }

    #[test]
    fn json_array_of_arrays_without_header() {
        let json = extract(HTML, 0, Format::Json, false).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v[0][0], "Name");
        assert_eq!(v[1][0], "Alice");
        assert_eq!(v.as_array().unwrap().len(), 3); // header row included
    }

    #[test]
    fn selects_second_table_by_index() {
        let csv = extract(HTML, 1, Format::Csv, true).unwrap();
        assert_eq!(csv, "second,table\n");
    }

    #[test]
    fn collapses_whitespace_in_cells() {
        let h = "<table><tr><td>  hello \n  world </td></tr></table>";
        let csv = extract(h, 0, Format::Csv, false).unwrap();
        assert_eq!(csv, "hello world\n");
    }

    #[test]
    fn csv_escapes_commas_and_quotes() {
        let h = r#"<table><tr><td>a,b</td><td>he said "hi"</td></tr></table>"#;
        let csv = extract(h, 0, Format::Csv, false).unwrap();
        assert_eq!(csv, "\"a,b\",\"he said \"\"hi\"\"\"\n");
    }

    #[test]
    fn errors() {
        assert!(extract("", 0, Format::Csv, true).is_err());
        assert!(extract("<p>no tables here</p>", 0, Format::Csv, true).is_err());
        assert!(extract(HTML, 9, Format::Csv, true).is_err()); // index out of range
    }

    #[test]
    fn parse_format_values() {
        assert_eq!(parse_format("csv").unwrap(), Format::Csv);
        assert_eq!(parse_format("").unwrap(), Format::Csv);
        assert_eq!(parse_format("JSON").unwrap(), Format::Json);
        assert!(parse_format("xml").is_err());
    }
}
