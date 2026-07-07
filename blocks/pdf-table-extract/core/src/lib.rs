//! gizza-ai/pdf-table-extract core — detect tabular data in a text-based PDF and
//! reconstruct it as a grid of cells, exportable to CSV/TSV or JSON.
//!
//! No wafer/wasm-bindgen deps, so it compiles natively for unit tests and to
//! `wasm32-wasip1` (the `wafer build` target) for the chat block.
//!
//! ## How it works (auto-detection from text coordinates)
//!
//! `lopdf`'s convenience `extract_text` gives the decoded text but throws away
//! *where* each run sits on the page — which is exactly what table
//! reconstruction needs. So instead we decode each page's content stream
//! ourselves and run the PDF text-positioning state machine
//! (`BT`/`Tf`/`Td`/`TD`/`Tm`/`T*`/`TL` + the `Tj`/`TJ`/`'`/`"` show operators),
//! recording every text run's baseline `(x, y)` in text space. Runs are then
//! clustered into rows (by `y`) and columns (by the left-edge `x`) to rebuild a
//! grid.
//!
//! ## Scope / limitations (honest)
//!
//! - **Text-based PDFs only.** It reads the embedded selectable-text layer; it
//!   does NOT OCR scanned/image-only PDFs (those have no text runs → no table).
//! - **Auto-detection, not manual selection.** It treats each page's positioned
//!   text as one grid. It works best on pages that ARE a table with clearly
//!   separated, left-aligned columns. Prose mixed with a table, right-aligned
//!   numeric columns, and merged/spanning cells can misalign.
//! - Text runs whose font encoding can't be decoded (e.g. an unparseable
//!   `ToUnicode` CMap) are skipped, and a page whose content stream can't be
//!   parsed is skipped, rather than failing the whole document.

use std::collections::BTreeMap;

use lopdf::content::Content;
use lopdf::{Document, Encoding, Object};

/// Guard against a pathological PDF claiming an absurd page count.
const MAX_PAGES: usize = 10_000;

/// A reconstructed table: a row-major grid of string cells plus the 1-based
/// source page it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// 1-based page number this table was detected on.
    pub page: usize,
    /// Rows of cells, top-to-bottom, each left-to-right. Every row is padded to
    /// the table's column count (empty string = no text in that cell).
    pub rows: Vec<Vec<String>>,
}

impl Table {
    /// Number of columns (0 for an empty table).
    pub fn cols(&self) -> usize {
        self.rows.first().map(|r| r.len()).unwrap_or(0)
    }
}

/// A single positioned text run recovered from a content stream.
struct Run {
    /// x of the run's start (text-space, left edge).
    x: f64,
    /// y of the run's baseline (text-space; larger = higher on the page).
    y: f64,
    /// Effective font height, used to size the row/column clustering tolerance.
    size: f64,
    /// Decoded text.
    text: String,
}

/// A 2-D affine transform stored as PDF's `[a b c d e f]` (last column
/// `[0 0 1]`).
type Mat = [f64; 6];
const IDENT: Mat = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `m × n` for two PDF matrices.
fn mul(m: Mat, n: Mat) -> Mat {
    [
        m[0] * n[0] + m[1] * n[2],
        m[0] * n[1] + m[1] * n[3],
        m[2] * n[0] + m[3] * n[2],
        m[2] * n[1] + m[3] * n[3],
        m[4] * n[0] + m[5] * n[2] + n[4],
        m[4] * n[1] + m[5] * n[3] + n[5],
    ]
}

fn translate(tx: f64, ty: f64) -> Mat {
    [1.0, 0.0, 0.0, 1.0, tx, ty]
}

/// Read a numeric operand (Integer or Real) as f64.
fn num(o: &Object) -> Option<f64> {
    match o {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

fn nth_num(operands: &[Object], i: usize) -> Option<f64> {
    operands.get(i).and_then(num)
}

/// Decode the text carried by a show operator's operands into `out`, replicating
/// lopdf's own `collect_text`: `TJ` arrays interleave strings with numeric
/// spacing adjustments, and a large negative adjustment (`< -100`) marks a word
/// gap → a space.
fn collect(out: &mut String, enc: &Encoding, operands: &[Object]) {
    for o in operands {
        match o {
            Object::String(bytes, _) => {
                if let Ok(t) = Document::decode_text(enc, bytes) {
                    out.push_str(&t);
                }
            }
            Object::Array(arr) => collect(out, enc, arr),
            Object::Integer(i) => {
                if *i < -100 {
                    out.push(' ');
                }
            }
            Object::Real(r) => {
                if *r < -100.0 {
                    out.push(' ');
                }
            }
            _ => {}
        }
    }
}

/// Record one run at the current text position if it decodes to non-blank text.
fn push_run(runs: &mut Vec<Run>, tm: &Mat, size: f64, enc: Option<&Encoding>, operands: &[Object]) {
    let enc = match enc {
        Some(e) => e,
        None => return,
    };
    let mut text = String::new();
    collect(&mut text, enc, operands);
    if text.trim().is_empty() {
        return;
    }
    // Effective height = the font size scaled by the text matrix's vertical
    // component; fall back to the raw size for rotated/degenerate matrices.
    let eff = (size * tm[3]).abs();
    let eff = if eff > 0.0 { eff } else { size.abs().max(1.0) };
    runs.push(Run {
        x: tm[4],
        y: tm[5],
        size: eff,
        text: text.trim().to_string(),
    });
}

/// Recover every positioned text run from one page's content stream.
fn page_runs(doc: &Document, page_id: (u32, u16)) -> Result<Vec<Run>, String> {
    let fonts = doc
        .get_page_fonts(page_id)
        .map_err(|e| format!("read page fonts: {e}"))?;
    // font-resource-name → decoding, skipping fonts whose encoding won't parse.
    let encodings: BTreeMap<Vec<u8>, Encoding> = fonts
        .into_iter()
        .filter_map(|(name, font)| font.get_font_encoding(doc).ok().map(|enc| (name, enc)))
        .collect();

    let data = doc
        .get_page_content(page_id)
        .map_err(|e| format!("read page content: {e}"))?;
    let content = Content::decode(&data).map_err(|e| format!("decode content stream: {e}"))?;

    let mut runs = Vec::new();
    let mut enc: Option<&Encoding> = None;
    let mut size = 0.0f64;
    let mut leading = 0.0f64;
    let mut tm = IDENT; // text matrix
    let mut tlm = IDENT; // text line matrix

    for op in &content.operations {
        match op.operator.as_str() {
            "BT" => {
                tm = IDENT;
                tlm = IDENT;
            }
            "Tf" => {
                if let Some(name) = op.operands.first().and_then(|o| o.as_name().ok()) {
                    enc = encodings.get(name);
                }
                size = nth_num(&op.operands, 1).unwrap_or(size);
            }
            "Td" => {
                let tx = nth_num(&op.operands, 0).unwrap_or(0.0);
                let ty = nth_num(&op.operands, 1).unwrap_or(0.0);
                tlm = mul(translate(tx, ty), tlm);
                tm = tlm;
            }
            "TD" => {
                let tx = nth_num(&op.operands, 0).unwrap_or(0.0);
                let ty = nth_num(&op.operands, 1).unwrap_or(0.0);
                leading = -ty;
                tlm = mul(translate(tx, ty), tlm);
                tm = tlm;
            }
            "Tm" => {
                if op.operands.len() >= 6 {
                    let mut m = IDENT;
                    for (i, slot) in m.iter_mut().enumerate() {
                        *slot = nth_num(&op.operands, i).unwrap_or(IDENT[i]);
                    }
                    tlm = m;
                    tm = m;
                }
            }
            "T*" => {
                tlm = mul(translate(0.0, -leading), tlm);
                tm = tlm;
            }
            "TL" => {
                leading = nth_num(&op.operands, 0).unwrap_or(leading);
            }
            "Tj" | "TJ" => {
                push_run(&mut runs, &tm, size, enc, &op.operands);
            }
            // `'` = next line then show.
            "'" => {
                tlm = mul(translate(0.0, -leading), tlm);
                tm = tlm;
                push_run(&mut runs, &tm, size, enc, &op.operands);
            }
            // `"` = set word/char spacing (operands 0,1; ignored), next line,
            // then show operand 2.
            "\"" => {
                tlm = mul(translate(0.0, -leading), tlm);
                tm = tlm;
                if op.operands.len() >= 3 {
                    push_run(&mut runs, &tm, size, enc, &op.operands[2..]);
                }
            }
            _ => {}
        }
    }
    Ok(runs)
}

/// Cluster sorted values with single-linkage: start a new cluster whenever the
/// gap to the previous value exceeds `tol`. Returns each cluster's midpoint.
fn cluster_centers(mut values: Vec<f64>, tol: f64) -> Vec<f64> {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut ranges: Vec<(f64, f64)> = Vec::new();
    for v in values {
        match ranges.last_mut() {
            Some(last) if v - last.1 <= tol => last.1 = v,
            _ => ranges.push((v, v)),
        }
    }
    ranges.iter().map(|(lo, hi)| (lo + hi) / 2.0).collect()
}

/// Index of the center nearest to `x`.
fn nearest(centers: &[f64], x: f64) -> usize {
    let mut best = 0;
    let mut best_d = f64::MAX;
    for (i, &c) in centers.iter().enumerate() {
        let d = (c - x).abs();
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best
}

/// Cluster a page's positioned runs into a row/column grid.
fn build_table(mut runs: Vec<Run>, page: usize) -> Option<Table> {
    runs.retain(|r| !r.text.is_empty());
    if runs.is_empty() {
        return None;
    }

    // Median run height → clustering tolerances.
    let mut sizes: Vec<f64> = runs.iter().map(|r| r.size.abs().max(1.0)).collect();
    sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let med = sizes[sizes.len() / 2];
    let row_tol = (med * 0.5).max(2.0);
    let col_tol = (med * 0.5).max(3.0);

    // Rows: sort by y descending (top of page first), then break into rows when
    // y drops by more than the tolerance from the current row's anchor.
    runs.sort_by(|a, b| b.y.partial_cmp(&a.y).unwrap_or(std::cmp::Ordering::Equal));
    let mut rows: Vec<Vec<Run>> = Vec::new();
    for run in runs {
        match rows.last_mut() {
            Some(row) if (row[0].y - run.y).abs() <= row_tol => row.push(run),
            _ => rows.push(vec![run]),
        }
    }
    for row in &mut rows {
        row.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
    }

    // Columns: cluster every run's left-edge x across all rows.
    let xs: Vec<f64> = rows.iter().flatten().map(|r| r.x).collect();
    let centers = cluster_centers(xs, col_tol);
    let ncols = centers.len().max(1);

    let grid: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            let mut cells = vec![String::new(); ncols];
            for run in row {
                let col = nearest(&centers, run.x);
                if !cells[col].is_empty() {
                    cells[col].push(' ');
                }
                cells[col].push_str(&run.text);
            }
            cells
        })
        .collect();

    Some(Table { page, rows: grid })
}

/// Detect tables in a PDF.
///
/// - `bytes` — the raw PDF file.
/// - `page` — optional 1-based page number. `None` scans every page and returns
///   one table per page that has any text; `Some(n)` scans only page `n`.
///
/// Returns one [`Table`] per page on which any positioned text was found (a page
/// with no selectable text yields no table). Returns `Err` when the bytes don't
/// parse as a PDF or `page` is out of range.
pub fn extract_tables(bytes: &[u8], page: Option<usize>) -> Result<Vec<Table>, String> {
    let doc = Document::load_mem(bytes).map_err(|e| format!("failed to parse PDF: {e}"))?;

    let pages_map = doc.get_pages();
    let mut page_numbers: Vec<u32> = pages_map.keys().copied().collect();
    page_numbers.sort_unstable();

    if page_numbers.is_empty() {
        return Err("PDF has no pages".to_string());
    }
    if page_numbers.len() > MAX_PAGES {
        return Err(format!(
            "PDF has too many pages: {} (cap {MAX_PAGES})",
            page_numbers.len()
        ));
    }

    let selected: Vec<u32> = match page {
        Some(p) => {
            let p32 = u32::try_from(p)
                .map_err(|_| format!("page {p} out of range (1..={})", page_numbers.len()))?;
            if !page_numbers.contains(&p32) {
                return Err(format!(
                    "page {p} out of range (1..={})",
                    page_numbers.len()
                ));
            }
            vec![p32]
        }
        None => page_numbers,
    };

    let mut tables = Vec::new();
    for pn in selected {
        let page_id = match pages_map.get(&pn) {
            Some(id) => *id,
            None => continue,
        };
        // Skip a page whose fonts/content can't be parsed rather than failing
        // the whole document.
        let runs = match page_runs(&doc, page_id) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if let Some(t) = build_table(runs, pn as usize) {
            tables.push(t);
        }
    }
    Ok(tables)
}

/// Escape one CSV field per RFC 4180: wrap in double quotes (doubling any inner
/// quote) when it contains the delimiter, a quote, or a line break.
fn csv_field(s: &str, delim: char) -> String {
    if s.contains(delim) || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Serialize tables to delimited text (CSV/TSV). Uses CRLF line endings (RFC
/// 4180). Multiple page-tables are separated by one blank line.
pub fn to_csv(tables: &[Table], delim: char) -> String {
    let d = delim.to_string();
    let mut out = String::new();
    for (i, t) in tables.iter().enumerate() {
        if i > 0 {
            out.push_str("\r\n");
        }
        for row in &t.rows {
            let line: Vec<String> = row.iter().map(|c| csv_field(c, delim)).collect();
            out.push_str(&line.join(&d));
            out.push_str("\r\n");
        }
    }
    out
}

/// Serialize tables to a JSON array of per-page table objects:
/// `[{ "page": 1, "rows": [...] }, ...]`.
///
/// When `header` is false each table's `rows` is an array-of-arrays of cell
/// strings. When `header` is true the first row is used as column keys and the
/// remaining rows become objects `{ key: value }` (a table with only a header
/// row yields an empty `rows`).
pub fn to_json(tables: &[Table], header: bool) -> String {
    use serde_json::{json, Map, Value};

    let arr: Vec<Value> = tables
        .iter()
        .map(|t| {
            let rows: Value = if header {
                let keys: Vec<String> = t.rows.first().cloned().unwrap_or_default();
                let objs: Vec<Value> = t
                    .rows
                    .iter()
                    .skip(1)
                    .map(|row| {
                        let mut m = Map::new();
                        for (i, key) in keys.iter().enumerate() {
                            let val = row.get(i).cloned().unwrap_or_default();
                            m.insert(key.clone(), Value::String(val));
                        }
                        Value::Object(m)
                    })
                    .collect();
                Value::Array(objs)
            } else {
                json!(t.rows)
            };
            json!({ "page": t.page, "rows": rows })
        })
        .collect();

    // Pretty-print so a human reading the download/response can scan it.
    serde_json::to_string_pretty(&Value::Array(arr)).unwrap_or_else(|_| "[]".to_string())
}

/// Total number of rows across all tables (for a summary line).
pub fn total_rows(tables: &[Table]) -> usize {
    tables.iter().map(|t| t.rows.len()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Object, Stream};

    /// Build a minimal PDF whose pages each draw a grid of cells. `pages` is a
    /// list of tables; each table is a list of rows; each row is a list of
    /// `(x, y, text)` placements. Every cell is emitted as its own `Tm`+`Tj` so
    /// the extractor sees a positioned run per cell.
    fn build_grid_pdf(pages: &[&[(f64, f64, &str)]]) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
            "Encoding" => "WinAnsiEncoding",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });

        let mut kids: Vec<Object> = Vec::new();
        for cells in pages {
            let mut ops = vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
            ];
            for (x, y, text) in cells.iter() {
                ops.push(Operation::new(
                    "Tm",
                    vec![
                        1.into(),
                        0.into(),
                        0.into(),
                        1.into(),
                        Object::Real(*x as f32),
                        Object::Real(*y as f32),
                    ],
                ));
                ops.push(Operation::new("Tj", vec![Object::string_literal(*text)]));
            }
            ops.push(Operation::new("ET", vec![]));
            let content = Content { operations: ops };
            let content_id =
                doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                "Resources" => resources_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            });
            kids.push(page_id.into());
        }

        let count = kids.len() as i64;
        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => count,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages_dict));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    /// A 2-column × 3-row table: header row + two data rows.
    fn sample_table() -> Vec<u8> {
        build_grid_pdf(&[&[
            (100.0, 700.0, "Name"),
            (300.0, 700.0, "Age"),
            (100.0, 680.0, "Alice"),
            (300.0, 680.0, "30"),
            (100.0, 660.0, "Bob"),
            (300.0, 660.0, "25"),
        ]])
    }

    #[test]
    fn detects_rows_and_columns() {
        let pdf = sample_table();
        let tables = extract_tables(&pdf, None).unwrap();
        assert_eq!(tables.len(), 1, "one page → one table");
        let t = &tables[0];
        assert_eq!(t.rows.len(), 3, "3 rows");
        assert_eq!(t.cols(), 2, "2 columns");
        assert_eq!(t.rows[0], vec!["Name", "Age"]);
        assert_eq!(t.rows[1], vec!["Alice", "30"]);
        assert_eq!(t.rows[2], vec!["Bob", "25"]);
    }

    #[test]
    fn csv_output_is_rfc4180() {
        let pdf = sample_table();
        let tables = extract_tables(&pdf, None).unwrap();
        let csv = to_csv(&tables, ',');
        assert_eq!(csv, "Name,Age\r\nAlice,30\r\nBob,25\r\n");
    }

    #[test]
    fn tsv_uses_tab_delimiter() {
        let pdf = sample_table();
        let tables = extract_tables(&pdf, None).unwrap();
        let tsv = to_csv(&tables, '\t');
        assert_eq!(tsv, "Name\tAge\r\nAlice\t30\r\nBob\t25\r\n");
    }

    #[test]
    fn csv_field_quotes_delimiter_and_quotes() {
        assert_eq!(csv_field("plain", ','), "plain");
        assert_eq!(csv_field("a,b", ','), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\"", ','), "\"say \"\"hi\"\"\"");
        // Semicolon delimiter: a comma no longer needs quoting.
        assert_eq!(csv_field("a,b", ';'), "a,b");
    }

    #[test]
    fn json_array_of_arrays_without_header() {
        let pdf = sample_table();
        let tables = extract_tables(&pdf, None).unwrap();
        let json = to_json(&tables, false);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v[0]["page"], 1);
        assert_eq!(v[0]["rows"][0][0], "Name");
        assert_eq!(v[0]["rows"][1][1], "30");
    }

    #[test]
    fn json_objects_with_header() {
        let pdf = sample_table();
        let tables = extract_tables(&pdf, None).unwrap();
        let json = to_json(&tables, true);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        // Header row consumed; two data objects keyed by Name/Age.
        assert_eq!(v[0]["rows"].as_array().unwrap().len(), 2);
        assert_eq!(v[0]["rows"][0]["Name"], "Alice");
        assert_eq!(v[0]["rows"][0]["Age"], "30");
        assert_eq!(v[0]["rows"][1]["Name"], "Bob");
    }

    #[test]
    fn selects_a_single_page() {
        let pdf = build_grid_pdf(&[
            &[(100.0, 700.0, "PageOneCell")],
            &[(100.0, 700.0, "PageTwoCell")],
        ]);
        let only2 = extract_tables(&pdf, Some(2)).unwrap();
        assert_eq!(only2.len(), 1);
        assert_eq!(only2[0].page, 2);
        assert_eq!(only2[0].rows[0][0], "PageTwoCell");
    }

    #[test]
    fn all_pages_each_yield_a_table() {
        let pdf = build_grid_pdf(&[
            &[(100.0, 700.0, "One")],
            &[(100.0, 700.0, "Two")],
        ]);
        let tables = extract_tables(&pdf, None).unwrap();
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0].page, 1);
        assert_eq!(tables[1].page, 2);
    }

    #[test]
    fn page_out_of_range_errors() {
        let pdf = sample_table();
        let err = extract_tables(&pdf, Some(9)).unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
    }

    #[test]
    fn rejects_non_pdf_bytes() {
        let err = extract_tables(b"this is plainly not a pdf", None).unwrap_err();
        assert!(err.contains("failed to parse PDF"), "got: {err}");
    }

    #[test]
    fn multi_word_cell_from_single_run_is_one_cell() {
        // A multi-word cell drawn as ONE show op (the common case for
        // data-export PDFs) stays a single cell — its inner space is preserved
        // and it does not spawn extra columns.
        let pdf = build_grid_pdf(&[&[
            (100.0, 700.0, "City"),
            (300.0, 700.0, "Role"),
            (100.0, 680.0, "New York"),
            (300.0, 680.0, "Engineer"),
        ]]);
        let tables = extract_tables(&pdf, None).unwrap();
        let t = &tables[0];
        assert_eq!(t.cols(), 2, "still two columns");
        assert_eq!(t.rows[0], vec!["City", "Role"]);
        assert_eq!(t.rows[1], vec!["New York", "Engineer"]);
    }
}
