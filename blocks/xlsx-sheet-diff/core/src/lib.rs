//! xlsx-sheet-diff core — cell-by-cell diff of two worksheets in one workbook.
//!
//! Opens a spreadsheet (`.xlsx`/`.xlsm`/`.xls`/`.ods`) with calamine, picks two
//! sheets (by name or 0-based index), and compares them **cell-by-cell at the
//! same address** (A1 in sheet 1 vs A1 in sheet 2), reporting:
//!   - **value changes** — a cell whose displayed value differs (changed / added
//!     in sheet 2 / removed from sheet 1),
//!   - **formula changes** — a cell whose stored formula string differs (a
//!     rewritten `=SUM(A1:A2)` → `=SUM(A1:A3)` is caught even when the cached
//!     result is unchanged — many "value-only" diff tools miss this),
//!   - **structural changes** — rows or columns that exist in only one sheet
//!     (different used extents).
//!
//! Three renderers: a readable `table` report, a structured `json` report, and a
//! flat `csv` change-log (one row per changed cell). Pure logic, no I/O — shared
//! by the chat block and the CLI.

use std::collections::BTreeMap;
use std::io::Cursor;

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use serde_json::json;

/// Comparison options that affect cell MATCHING only. Rendered output always
/// shows the original, unmodified cell text.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// Compare cell/formula text case-insensitively.
    pub ignore_case: bool,
    /// Collapse runs of whitespace (and trim) before comparing.
    pub ignore_whitespace: bool,
    /// Also compare the stored formula strings, not just displayed values.
    pub compare_formulas: bool,
}

/// One cell-level value difference between the two sheets.
struct ValueChange {
    row: u32, // 0-based
    col: u32, // 0-based
    kind: ChangeKind,
    old: String,
    new: String,
}

/// One cell-level formula difference between the two sheets.
struct FormulaChange {
    row: u32,
    col: u32,
    old: String,
    new: String,
}

#[derive(Clone, Copy, PartialEq)]
enum ChangeKind {
    Changed,
    Added,   // present only in sheet 2
    Removed, // present only in sheet 1
}

impl ChangeKind {
    fn label(self) -> &'static str {
        match self {
            ChangeKind::Changed => "changed",
            ChangeKind::Added => "added",
            ChangeKind::Removed => "removed",
        }
    }
}

/// Compare two worksheets of the spreadsheet `bytes`.
///
/// `sheet1` / `sheet2` each select a worksheet: a sheet **name**, or a 0-based
/// **index** as a string (e.g. `"0"`). `None`/empty defaults to index 0 for
/// `sheet1` and index 1 for `sheet2` (the first two sheets).
///
/// `format` is `"table"`, `"json"`, or `"csv"`. Returns `Err` on unreadable
/// bytes, an unknown sheet, a workbook with fewer than two sheets when relying
/// on defaults, or an unknown format.
pub fn diff(
    bytes: &[u8],
    sheet1: Option<&str>,
    sheet2: Option<&str>,
    opts: Options,
    format: &str,
) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("empty spreadsheet bytes".to_string());
    }
    let fmt = match format.trim() {
        "" => "table",
        f @ ("table" | "json" | "csv") => f,
        other => return Err(format!("unknown format {other:?} (use table, json, or csv)")),
    };

    let mut wb = open_workbook_auto_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|e| format!("not a readable spreadsheet: {e}"))?;

    let names = wb.sheet_names().to_vec();
    if names.is_empty() {
        return Err("spreadsheet has no worksheets".to_string());
    }

    let name1 = resolve_sheet_name(&names, sheet1, 0)?;
    let name2 = resolve_sheet_name(&names, sheet2, 1)?;

    // Read displayed values for both sheets.
    let range1 = wb
        .worksheet_range(&name1)
        .map_err(|e| format!("failed to read sheet {name1:?}: {e}"))?;
    let range2 = wb
        .worksheet_range(&name2)
        .map_err(|e| format!("failed to read sheet {name2:?}: {e}"))?;

    let vals1 = value_map(&range1);
    let vals2 = value_map(&range2);
    let extent1 = extent(&range1);
    let extent2 = extent(&range2);

    // Value changes over the union of populated addresses.
    let mut value_changes: Vec<ValueChange> = Vec::new();
    let mut addrs: Vec<(u32, u32)> = vals1.keys().chain(vals2.keys()).copied().collect();
    addrs.sort_unstable();
    addrs.dedup();
    for (r, c) in addrs {
        let old = vals1.get(&(r, c)).cloned().unwrap_or_default();
        let new = vals2.get(&(r, c)).cloned().unwrap_or_default();
        if fold(&old, opts) == fold(&new, opts) {
            continue;
        }
        let kind = if old.is_empty() {
            ChangeKind::Added
        } else if new.is_empty() {
            ChangeKind::Removed
        } else {
            ChangeKind::Changed
        };
        value_changes.push(ValueChange { row: r, col: c, kind, old, new });
    }

    // Formula changes (optional).
    let mut formula_changes: Vec<FormulaChange> = Vec::new();
    if opts.compare_formulas {
        let f1 = formula_map(&mut wb, &name1)?;
        let f2 = formula_map(&mut wb, &name2)?;
        let mut faddrs: Vec<(u32, u32)> = f1.keys().chain(f2.keys()).copied().collect();
        faddrs.sort_unstable();
        faddrs.dedup();
        for (r, c) in faddrs {
            let old = f1.get(&(r, c)).cloned().unwrap_or_default();
            let new = f2.get(&(r, c)).cloned().unwrap_or_default();
            if fold(&old, opts) == fold(&new, opts) {
                continue;
            }
            formula_changes.push(FormulaChange { row: r, col: c, old, new });
        }
    }

    // Structural differences (rows/columns present in only one sheet).
    let structural = structural_notes(extent1, extent2, &name1, &name2);

    Ok(match fmt {
        "json" => render_json(
            &name1, &name2, extent1, extent2, &value_changes, &formula_changes, &structural,
            opts.compare_formulas,
        ),
        "csv" => render_csv(&value_changes, &formula_changes),
        _ => render_table(
            &name1, &name2, extent1, extent2, &value_changes, &formula_changes, &structural,
            opts.compare_formulas,
        ),
    })
}

/// Pick a worksheet name honoring the selector: exact name match first (so a
/// sheet literally named `"0"` stays reachable), then a 0-based integer index.
/// `None`/empty falls back to `default_idx`.
fn resolve_sheet_name(
    names: &[String],
    sel: Option<&str>,
    default_idx: usize,
) -> Result<String, String> {
    let sel = sel.map(str::trim).filter(|s| !s.is_empty());
    let Some(sel) = sel else {
        return names.get(default_idx).cloned().ok_or_else(|| {
            format!(
                "workbook has only {} sheet(s); need at least {} to compare two sheets \
                 (name the sheets explicitly with sheet1/sheet2)",
                names.len(),
                default_idx + 1
            )
        });
    };
    if let Some(found) = names.iter().find(|n| n.as_str() == sel) {
        return Ok(found.clone());
    }
    if let Ok(idx) = sel.parse::<usize>() {
        return names
            .get(idx)
            .cloned()
            .ok_or_else(|| format!("sheet index {idx} out of range (sheets: {names:?})"));
    }
    Err(format!("no sheet named {sel:?} (available: {names:?})"))
}

/// Absolute-address → displayed-text map for the non-empty cells of a value
/// range. Addresses are absolute 0-based worksheet coordinates (so two ranges
/// with different start offsets still align at A1/B2/…).
fn value_map(range: &calamine::Range<Data>) -> BTreeMap<(u32, u32), String> {
    let (r0, c0) = range.start().unwrap_or((0, 0));
    let (h, w) = range.get_size();
    let mut m = BTreeMap::new();
    for r in 0..h {
        for c in 0..w {
            if let Some(cell) = range.get((r, c)) {
                if matches!(cell, Data::Empty) {
                    continue;
                }
                let s = cell_to_string(cell);
                if !s.is_empty() {
                    m.insert((r0 + r as u32, c0 + c as u32), s);
                }
            }
        }
    }
    m
}

/// Absolute-address → formula-text map for cells carrying a formula. calamine's
/// formula range stores an empty string where there is no formula.
fn formula_map(
    wb: &mut calamine::Sheets<Cursor<Vec<u8>>>,
    name: &str,
) -> Result<BTreeMap<(u32, u32), String>, String> {
    let range = wb
        .worksheet_formula(name)
        .map_err(|e| format!("failed to read formulas of sheet {name:?}: {e}"))?;
    let (r0, c0) = range.start().unwrap_or((0, 0));
    let (h, w) = range.get_size();
    let mut m = BTreeMap::new();
    for r in 0..h {
        for c in 0..w {
            if let Some(f) = range.get((r, c)) {
                if f.is_empty() {
                    continue;
                }
                // Normalize the leading `=` so the display is consistent.
                let f = f.strip_prefix('=').unwrap_or(f);
                m.insert((r0 + r as u32, c0 + c as u32), format!("={f}"));
            }
        }
    }
    Ok(m)
}

/// Used extent of a range as `(rows, cols)` — the 1-based count from A1 to the
/// bottom-right populated cell (so a sheet using B2:C3 reports 3×3, matching how
/// the addresses read). Empty sheets report `(0, 0)`.
fn extent(range: &calamine::Range<Data>) -> (u32, u32) {
    let mut max_r = 0u32;
    let mut max_c = 0u32;
    let mut any = false;
    let (r0, c0) = range.start().unwrap_or((0, 0));
    let (h, w) = range.get_size();
    for r in 0..h {
        for c in 0..w {
            if let Some(cell) = range.get((r, c)) {
                if matches!(cell, Data::Empty) || cell_to_string(cell).is_empty() {
                    continue;
                }
                any = true;
                max_r = max_r.max(r0 + r as u32);
                max_c = max_c.max(c0 + c as u32);
            }
        }
    }
    if any {
        (max_r + 1, max_c + 1)
    } else {
        (0, 0)
    }
}

/// Human-readable notes for rows/columns present in only one sheet.
fn structural_notes(e1: (u32, u32), e2: (u32, u32), name1: &str, name2: &str) -> Vec<String> {
    let mut out = Vec::new();
    // Rows (1-based row numbers).
    if e2.0 > e1.0 {
        out.push(row_note(e1.0 + 1, e2.0, name2));
    } else if e1.0 > e2.0 {
        out.push(row_note(e2.0 + 1, e1.0, name1));
    }
    // Columns (0-based indices → A1 letters).
    if e2.1 > e1.1 {
        out.push(col_note(e1.1, e2.1 - 1, name2));
    } else if e1.1 > e2.1 {
        out.push(col_note(e2.1, e1.1 - 1, name1));
    }
    out
}

fn row_note(from: u32, to: u32, sheet: &str) -> String {
    if from == to {
        format!("Row {from} exists only in {sheet:?}")
    } else {
        format!("Rows {from}–{to} exist only in {sheet:?}")
    }
}

fn col_note(from_idx: u32, to_idx: u32, sheet: &str) -> String {
    let from = col_letters(from_idx);
    let to = col_letters(to_idx);
    if from == to {
        format!("Column {from} exists only in {sheet:?}")
    } else {
        format!("Columns {from}–{to} exist only in {sheet:?}")
    }
}

/// Fold a value for comparison per the options (whitespace then case).
fn fold(s: &str, opts: Options) -> String {
    let mut out = s.to_string();
    if opts.ignore_whitespace {
        out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    if opts.ignore_case {
        out = out.to_lowercase();
    }
    out
}

/// A1-style address for a 0-based (row, col).
fn a1(row: u32, col: u32) -> String {
    format!("{}{}", col_letters(col), row + 1)
}

/// Excel bijective base-26 column letters for a 0-based column index.
fn col_letters(col: u32) -> String {
    let mut n = col as u64 + 1;
    let mut buf = Vec::new();
    while n > 0 {
        let rem = ((n - 1) % 26) as u8;
        buf.push(b'A' + rem);
        n = (n - 1) / 26;
    }
    buf.reverse();
    String::from_utf8(buf).unwrap()
}

/// Render one calamine cell as plain text (Excel-style: whole floats drop `.0`).
fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => format_float(*f),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        Data::DateTime(dt) => format_float(dt.as_f64()),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("{e:?}"),
    }
}

fn format_float(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

fn render_table(
    name1: &str,
    name2: &str,
    e1: (u32, u32),
    e2: (u32, u32),
    values: &[ValueChange],
    formulas: &[FormulaChange],
    structural: &[String],
    compare_formulas: bool,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("Sheet diff: {name1:?} vs {name2:?}\n"));
    out.push_str(&format!(
        "Dimensions: {name1:?} = {}×{}, {name2:?} = {}×{} (rows×cols)\n",
        e1.0, e1.1, e2.0, e2.1
    ));

    if values.is_empty() && formulas.is_empty() && structural.is_empty() {
        out.push_str("\nNo differences found — the two sheets are identical.\n");
        return out;
    }

    out.push_str(&format!("\nValue changes ({}):\n", values.len()));
    if values.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for v in values {
            let addr = a1(v.row, v.col);
            match v.kind {
                ChangeKind::Changed => {
                    out.push_str(&format!("  {addr}: {:?} -> {:?}\n", v.old, v.new));
                }
                ChangeKind::Added => {
                    out.push_str(&format!("  {addr}: (empty) -> {:?}  [added in {name2:?}]\n", v.new));
                }
                ChangeKind::Removed => {
                    out.push_str(&format!("  {addr}: {:?} -> (empty)  [removed from {name1:?}]\n", v.old));
                }
            }
        }
    }

    if compare_formulas {
        out.push_str(&format!("\nFormula changes ({}):\n", formulas.len()));
        if formulas.is_empty() {
            out.push_str("  (none)\n");
        } else {
            for f in formulas {
                let addr = a1(f.row, f.col);
                let old = if f.old.is_empty() { "(none)".to_string() } else { f.old.clone() };
                let new = if f.new.is_empty() { "(none)".to_string() } else { f.new.clone() };
                out.push_str(&format!("  {addr}: {old} -> {new}\n"));
            }
        }
    }

    out.push_str("\nStructural changes:\n");
    if structural.is_empty() {
        out.push_str("  (none — same used rows and columns)\n");
    } else {
        for s in structural {
            out.push_str(&format!("  {s}\n"));
        }
    }

    out
}

fn render_json(
    name1: &str,
    name2: &str,
    e1: (u32, u32),
    e2: (u32, u32),
    values: &[ValueChange],
    formulas: &[FormulaChange],
    structural: &[String],
    compare_formulas: bool,
) -> String {
    let value_json: Vec<_> = values
        .iter()
        .map(|v| {
            json!({
                "cell": a1(v.row, v.col),
                "row": v.row + 1,
                "col": col_letters(v.col),
                "kind": v.kind.label(),
                "old": v.old,
                "new": v.new,
            })
        })
        .collect();
    let formula_json: Vec<_> = formulas
        .iter()
        .map(|f| {
            json!({
                "cell": a1(f.row, f.col),
                "row": f.row + 1,
                "col": col_letters(f.col),
                "old": f.old,
                "new": f.new,
            })
        })
        .collect();

    let identical = values.is_empty() && formulas.is_empty() && structural.is_empty();
    let mut obj = json!({
        "sheet1": name1,
        "sheet2": name2,
        "dimensions": {
            "sheet1": { "rows": e1.0, "cols": e1.1 },
            "sheet2": { "rows": e2.0, "cols": e2.1 },
        },
        "value_changes": value_json,
        "structural_changes": structural,
        "summary": {
            "value_changes": values.len(),
            "structural_changes": structural.len(),
            "identical": identical,
        },
    });
    if compare_formulas {
        obj["formula_changes"] = json!(formula_json);
        obj["summary"]["formula_changes"] = json!(formulas.len());
    }
    serde_json::to_string_pretty(&obj).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn render_csv(values: &[ValueChange], formulas: &[FormulaChange]) -> String {
    let mut out = String::from("cell,type,old,new\r\n");
    for v in values {
        let ty = match v.kind {
            ChangeKind::Changed => "value",
            ChangeKind::Added => "value-added",
            ChangeKind::Removed => "value-removed",
        };
        out.push_str(&format!(
            "{},{},{},{}\r\n",
            a1(v.row, v.col),
            ty,
            csv_field(&v.old),
            csv_field(&v.new)
        ));
    }
    for f in formulas {
        out.push_str(&format!(
            "{},{},{},{}\r\n",
            a1(f.row, f.col),
            "formula",
            csv_field(&f.old),
            csv_field(&f.new)
        ));
    }
    out
}

/// RFC-4180-quote a field iff it contains a comma, quote, CR, or LF.
fn csv_field(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_xlsxwriter::{Formula, Workbook};

    /// Two sheets:
    ///   "Before": A1="name", B1="qty", A2="apple", B2=10, C2 formula =A2&B2
    ///   "After":  A1="name", B1="qty", A2="apple", B2=12 (value change),
    ///             C2 formula =A2 (formula change), D3="extra" (structural col+row)
    fn sample() -> Vec<u8> {
        let mut wb = Workbook::new();
        let s1 = wb.add_worksheet().set_name("Before").unwrap();
        s1.write_string(0, 0, "name").unwrap();
        s1.write_string(0, 1, "qty").unwrap();
        s1.write_string(1, 0, "apple").unwrap();
        s1.write_number(1, 1, 10.0).unwrap();
        s1.write_formula(1, 2, Formula::new("=A2&B2")).unwrap();

        let s2 = wb.add_worksheet().set_name("After").unwrap();
        s2.write_string(0, 0, "name").unwrap();
        s2.write_string(0, 1, "qty").unwrap();
        s2.write_string(1, 0, "apple").unwrap();
        s2.write_number(1, 1, 12.0).unwrap();
        s2.write_formula(1, 2, Formula::new("=A2")).unwrap();
        s2.write_string(2, 3, "extra").unwrap();

        wb.save_to_buffer().unwrap()
    }

    #[test]
    fn table_reports_value_formula_and_structural_changes() {
        let bytes = sample();
        let opts = Options { compare_formulas: true, ..Default::default() };
        let out = diff(&bytes, Some("Before"), Some("After"), opts, "table").unwrap();
        // B2 value changed 10 -> 12.
        assert!(out.contains("B2: \"10\" -> \"12\""), "value change missing:\n{out}");
        // C2 formula changed.
        assert!(out.contains("C2: =A2&B2 -> =A2"), "formula change missing:\n{out}");
        // D3 added a cell only in After.
        assert!(out.contains("D3:") && out.contains("added in \"After\""), "add missing:\n{out}");
        // Structural: extra column D and row 3 only in After.
        assert!(out.contains("only in \"After\""), "structural missing:\n{out}");
    }

    #[test]
    fn identical_sheets_report_no_differences() {
        // Compare a sheet against itself → identical.
        let bytes = sample();
        let out = diff(&bytes, Some("Before"), Some("Before"), Options::default(), "table").unwrap();
        assert!(out.contains("No differences found"), "expected identical:\n{out}");
    }

    #[test]
    fn defaults_pick_first_two_sheets() {
        let bytes = sample();
        let out = diff(&bytes, None, None, Options::default(), "table").unwrap();
        assert!(out.contains("Sheet diff: \"Before\" vs \"After\""), "{out}");
    }

    #[test]
    fn formulas_ignored_when_disabled() {
        let bytes = sample();
        let out = diff(&bytes, Some("Before"), Some("After"), Options::default(), "table").unwrap();
        assert!(!out.contains("Formula changes"), "formulas should be hidden:\n{out}");
    }

    #[test]
    fn json_format_is_structured() {
        let bytes = sample();
        let opts = Options { compare_formulas: true, ..Default::default() };
        let out = diff(&bytes, Some("Before"), Some("After"), opts, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["sheet1"], "Before");
        assert_eq!(v["summary"]["identical"], false);
        assert!(v["value_changes"].as_array().unwrap().iter().any(|c| c["cell"] == "B2"));
        assert!(v["formula_changes"].as_array().unwrap().iter().any(|c| c["cell"] == "C2"));
    }

    #[test]
    fn csv_format_is_flat_changelog() {
        let bytes = sample();
        let opts = Options { compare_formulas: true, ..Default::default() };
        let out = diff(&bytes, Some("Before"), Some("After"), opts, "csv").unwrap();
        assert!(out.starts_with("cell,type,old,new\r\n"), "{out}");
        assert!(out.contains("B2,value,10,12\r\n"), "{out}");
        assert!(out.contains("C2,formula,=A2&B2,=A2\r\n"), "{out}");
    }

    #[test]
    fn ignore_case_folds_text() {
        let mut wb = Workbook::new();
        let s1 = wb.add_worksheet().set_name("a").unwrap();
        s1.write_string(0, 0, "Apple").unwrap();
        let s2 = wb.add_worksheet().set_name("b").unwrap();
        s2.write_string(0, 0, "APPLE").unwrap();
        let bytes = wb.save_to_buffer().unwrap();

        let out_sensitive = diff(&bytes, Some("a"), Some("b"), Options::default(), "table").unwrap();
        assert!(out_sensitive.contains("A1:"), "case-sensitive should flag A1:\n{out_sensitive}");
        let opts = Options { ignore_case: true, ..Default::default() };
        let out_folded = diff(&bytes, Some("a"), Some("b"), opts, "table").unwrap();
        assert!(out_folded.contains("No differences found"), "ignore_case should match:\n{out_folded}");
    }

    #[test]
    fn unknown_sheet_errors() {
        let bytes = sample();
        let err = diff(&bytes, Some("Nope"), Some("After"), Options::default(), "table").unwrap_err();
        assert!(err.contains("no sheet named"), "{err}");
    }

    #[test]
    fn unknown_format_errors() {
        let bytes = sample();
        let err = diff(&bytes, None, None, Options::default(), "html").unwrap_err();
        assert!(err.contains("unknown format"), "{err}");
    }

    #[test]
    fn empty_bytes_error() {
        let err = diff(&[], None, None, Options::default(), "table").unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn a1_addressing() {
        assert_eq!(a1(0, 0), "A1");
        assert_eq!(a1(1, 1), "B2");
        assert_eq!(col_letters(26), "AA");
        assert_eq!(col_letters(27), "AB");
    }
}
