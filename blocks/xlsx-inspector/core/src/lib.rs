//! xlsx-inspector core — report the STRUCTURE of a spreadsheet workbook.
//!
//! Opens an `.xlsx`/`.xlsm`/`.xlsb`/`.xls`/`.ods` workbook with `calamine` (pure
//! Rust, no C deps → compiles to `wasm32-wasip1` and `wasm32-unknown-unknown`)
//! and answers the questions you ask BEFORE you start converting or diffing a
//! workbook:
//!
//! * what worksheets exist, in what order, of what type (worksheet / chart /
//!   macro / dialog sheet) and with what visibility (visible / hidden /
//!   very hidden);
//! * how big each sheet actually is — the used range in A1 notation, the last
//!   cell, and the row × column extent;
//! * how many cells hold data, and the **formula vs value** split (a cell counts
//!   as a formula cell when the workbook stores formula text for it, regardless
//!   of whether a cached result was saved);
//! * what those cells hold — text, number, date/time, boolean, error — with a
//!   per-error-literal breakdown (`#REF!`, `#DIV/0!`, `#N/A`, …);
//! * the defined names (named ranges), the reference each points at, the sheet
//!   that reference names, and whether it is broken (`#REF!`);
//! * an object inventory read from the OPC package part list — tables, pivot
//!   tables, charts, images, comments, drawings, external links, data
//!   connections and a VBA macro project.
//!
//! Sibling blocks cover the neighbouring jobs: `xlsx-to-csv` extracts one sheet,
//! `xlsx-sheet-diff` compares two, and `spreadsheet-formula-audit` hunts for
//! broken formulas. This one answers "what is in this file?".
//!
//! Stated limits (the report repeats them, so a reader never has to guess):
//!
//! * Values are whatever the writing application last cached — nothing is
//!   recalculated here.
//! * `calamine` exposes defined names as a flat `(name, reference)` list, so a
//!   name's SCOPE (workbook-level vs worksheet-level), its comment, and whether
//!   the name itself is hidden are not available. The sheet shown per name is
//!   derived from the reference text.
//! * Object counts come from the ZIP package part list, so they are
//!   workbook-level, not per-sheet, and they are unavailable for the non-ZIP
//!   `.xls` (CFB) and `.ods` (ODF) containers — the report says so instead of
//!   printing zeros.
//! * Merged regions and array-formula flags are not surfaced by `calamine`'s
//!   format-agnostic reader, so they are not reported.

use std::collections::{BTreeMap, HashSet};
use std::io::Cursor;

use calamine::{open_workbook_auto_from_rs, Data, Reader, SheetType, SheetVisible, Sheets};
use serde_json::json;

/// Highest 0-based row/column Excel addresses, used to keep A1 rendering sane.
const MAX_COL: u32 = 16_383;

/// Do not walk a used range bigger than this many cells. A sheet whose extent is
/// pathologically sparse (one cell in A1, one in XFD1048576) would otherwise cost
/// an unbounded scan; its dimensions are still reported, the counts are not.
const MAX_SCAN_CELLS: usize = 8_000_000;

/// Hard ceiling on `max_named_ranges` so a hostile workbook cannot make the
/// report unbounded.
pub const NAMED_RANGE_CEILING: usize = 10_000;

/// What to include in the report.
#[derive(Debug, Clone)]
pub struct Options {
    /// Restrict the sheet table to one worksheet: a sheet name, or a 0-based
    /// index as a string. `None` reports every sheet.
    pub sheet: Option<String>,
    /// Include hidden and very-hidden sheets in the sheet table.
    pub include_hidden: bool,
    /// Include the defined-names (named ranges) section.
    pub include_named_ranges: bool,
    /// Include the workbook object inventory (tables, charts, images, …).
    pub include_object_counts: bool,
    /// Maximum defined names listed before the section is clipped with a note.
    pub max_named_ranges: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            sheet: None,
            include_hidden: true,
            include_named_ranges: true,
            include_object_counts: true,
            max_named_ranges: 200,
        }
    }
}

/// Per-sheet structural facts.
#[derive(Debug, Clone)]
struct SheetInfo {
    index: usize,
    name: String,
    kind: &'static str,
    visibility: &'static str,
    /// `A1:D40`, or `None` for a sheet with no used cells.
    used_range: Option<String>,
    rows: usize,
    columns: usize,
    non_empty_cells: usize,
    formula_cells: usize,
    value_cells: usize,
    text_cells: usize,
    number_cells: usize,
    date_cells: usize,
    bool_cells: usize,
    error_cells: usize,
    error_kinds: BTreeMap<String, usize>,
    /// Set when the sheet could not be read (chart/macro sheets have no cell
    /// grid) or when its extent exceeded [`MAX_SCAN_CELLS`].
    note: Option<String>,
}

/// One defined name (named range).
#[derive(Debug, Clone)]
struct NamedRange {
    name: String,
    refers_to: String,
    sheet: Option<String>,
    broken: bool,
}

/// Counts read from the OPC package part list. `None` for non-ZIP containers.
#[derive(Debug, Clone, Default)]
struct ObjectCounts {
    tables: usize,
    pivot_tables: usize,
    charts: usize,
    images: usize,
    comments: usize,
    drawings: usize,
    external_links: usize,
    has_data_connections: bool,
    has_macros: bool,
}

/// The full report, before rendering.
#[derive(Debug, Clone)]
struct Report {
    format: &'static str,
    container: &'static str,
    file_bytes: usize,
    sheets_total: usize,
    visible_sheets: usize,
    hidden_sheets: usize,
    very_hidden_sheets: usize,
    sheets: Vec<SheetInfo>,
    named_ranges: Vec<NamedRange>,
    named_ranges_total: usize,
    named_ranges_broken: usize,
    objects: Option<ObjectCounts>,
    notes: Vec<String>,
}

impl Report {
    fn total(&self, f: impl Fn(&SheetInfo) -> usize) -> usize {
        self.sheets.iter().map(f).sum()
    }
}

/// Inspect `bytes` and render the report in `format` (`table`, `json` or `csv`).
///
/// Returns `Err` with an actionable message on empty/corrupt bytes, an unknown
/// sheet selector, or an unknown output format.
pub fn inspect(bytes: &[u8], opts: &Options, format: &str) -> Result<String, String> {
    let report = build_report(bytes, opts)?;
    match format {
        "table" => Ok(render_table(&report)),
        "json" => Ok(render_json(&report)),
        "csv" => Ok(render_csv(&report)),
        other => Err(format!(
            "unknown format {other:?}: expected \"table\", \"json\", or \"csv\""
        )),
    }
}

fn build_report(bytes: &[u8], opts: &Options) -> Result<Report, String> {
    if bytes.is_empty() {
        return Err("empty workbook bytes".to_string());
    }

    let mut workbook = open_workbook_auto_from_rs(Cursor::new(bytes.to_vec())).map_err(|e| {
        format!("not a readable workbook (.xlsx/.xlsm/.xlsb/.xls/.ods expected): {e}")
    })?;

    let (format, container) = match &workbook {
        Sheets::Xlsx(_) => ("xlsx", "opc"),
        Sheets::Xlsb(_) => ("xlsb", "opc"),
        Sheets::Xls(_) => ("xls", "cfb"),
        Sheets::Ods(_) => ("ods", "odf"),
    };

    let metadata_sheets = workbook.sheets_metadata().to_vec();
    if metadata_sheets.is_empty() {
        return Err("workbook has no sheets".to_string());
    }

    let mut visible_sheets = 0usize;
    let mut hidden_sheets = 0usize;
    let mut very_hidden_sheets = 0usize;
    for s in &metadata_sheets {
        match s.visible {
            SheetVisible::Visible => visible_sheets += 1,
            SheetVisible::Hidden => hidden_sheets += 1,
            SheetVisible::VeryHidden => very_hidden_sheets += 1,
        }
    }

    // Which sheets go in the table: one selected sheet, or every sheet subject
    // to the hidden filter.
    let all_names: Vec<String> = metadata_sheets.iter().map(|s| s.name.clone()).collect();
    let selected: Vec<usize> = match opts
        .sheet
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(sel) => vec![resolve_sheet_index(&all_names, sel)?],
        None => (0..metadata_sheets.len())
            .filter(|&i| opts.include_hidden || metadata_sheets[i].visible == SheetVisible::Visible)
            .collect(),
    };

    let mut sheets = Vec::with_capacity(selected.len());
    for i in selected {
        sheets.push(scan_sheet(&mut workbook, i, &metadata_sheets[i]));
    }

    // Defined names. calamine returns them as a flat (name, reference) list.
    let mut named_ranges = Vec::new();
    let mut named_ranges_total = 0usize;
    let mut named_ranges_broken = 0usize;
    if opts.include_named_ranges {
        let names = workbook.defined_names().to_vec();
        named_ranges_total = names.len();
        let cap = opts.max_named_ranges.clamp(1, NAMED_RANGE_CEILING);
        for (name, refers_to) in names {
            let broken = refers_to.contains("#REF!");
            if broken {
                named_ranges_broken += 1;
            }
            if named_ranges.len() < cap {
                let sheet = sheet_of_reference(&refers_to);
                named_ranges.push(NamedRange {
                    name,
                    refers_to,
                    sheet,
                    broken,
                });
            }
        }
    }

    let objects = if opts.include_object_counts && container == "opc" {
        scan_package_parts(bytes)
    } else {
        None
    };

    let mut notes = vec![
        "Cell values are whatever the writing application last cached; nothing is recalculated."
            .to_string(),
        "A cell counts as a formula cell when the file stores formula text for it, even if no cached result was saved."
            .to_string(),
    ];
    if opts.include_named_ranges {
        notes.push(
            "Defined-name scope (workbook vs worksheet), name comments, and hidden names are not stored in a form this reader exposes; the sheet shown is derived from the reference text."
                .to_string(),
        );
    }
    if opts.include_object_counts {
        match container {
            "opc" if objects.is_some() => notes.push(
                "Object counts come from the package part list, so they cover the whole workbook rather than a single sheet."
                    .to_string(),
            ),
            "opc" => notes.push(
                "Object counts are unavailable: the package directory could not be read."
                    .to_string(),
            ),
            _ => notes.push(format!(
                "Object counts are unavailable for {format} files, which are not ZIP packages."
            )),
        }
    }
    if !opts.include_hidden && (hidden_sheets > 0 || very_hidden_sheets > 0) {
        notes.push(format!(
            "{} hidden and {very_hidden_sheets} very-hidden sheet(s) were excluded by include_hidden=false.",
            hidden_sheets
        ));
    }
    if named_ranges_total > named_ranges.len() && opts.include_named_ranges {
        notes.push(format!(
            "Named-range list clipped at {} of {named_ranges_total} entries; raise max_named_ranges to see the rest.",
            named_ranges.len()
        ));
    }

    Ok(Report {
        format,
        container,
        file_bytes: bytes.len(),
        sheets_total: metadata_sheets.len(),
        visible_sheets,
        hidden_sheets,
        very_hidden_sheets,
        sheets,
        named_ranges,
        named_ranges_total,
        named_ranges_broken,
        objects,
        notes,
    })
}

/// Read one sheet's grid and classify every used cell.
fn scan_sheet(
    workbook: &mut Sheets<Cursor<Vec<u8>>>,
    index: usize,
    meta: &calamine::Sheet,
) -> SheetInfo {
    let mut info = SheetInfo {
        index,
        name: meta.name.clone(),
        kind: sheet_kind(meta.typ),
        visibility: sheet_visibility(meta.visible),
        used_range: None,
        rows: 0,
        columns: 0,
        non_empty_cells: 0,
        formula_cells: 0,
        value_cells: 0,
        text_cells: 0,
        number_cells: 0,
        date_cells: 0,
        bool_cells: 0,
        error_cells: 0,
        error_kinds: BTreeMap::new(),
        note: None,
    };

    let range = match workbook.worksheet_range(&meta.name) {
        Ok(r) => r,
        Err(e) => {
            info.note = Some(format!("no cell grid could be read: {e}"));
            return info;
        }
    };

    if range.is_empty() {
        info.note = Some("sheet contains no used cells".to_string());
        return info;
    }

    let (start_row, start_col) = range.start().unwrap_or((0, 0));
    let (end_row, end_col) = range.end().unwrap_or((0, 0));
    info.used_range = Some(format!(
        "{}:{}",
        a1(start_row, start_col),
        a1(end_row, end_col)
    ));
    info.rows = range.height();
    info.columns = range.width();

    if info.rows.saturating_mul(info.columns) > MAX_SCAN_CELLS {
        info.note = Some(format!(
            "used range spans {} cells, above the {MAX_SCAN_CELLS} scan cap — dimensions reported, cell counts skipped",
            info.rows.saturating_mul(info.columns)
        ));
        return info;
    }

    // Absolute positions of every cell the file stores formula text for. The
    // formula range can start at a different cell than the data range, so both
    // are converted to absolute coordinates before comparing.
    let mut formula_at: HashSet<(u32, u32)> = HashSet::new();
    if let Ok(formulas) = workbook.worksheet_formula(&meta.name) {
        if let Some((frow, fcol)) = formulas.start() {
            for (r, c, f) in formulas.used_cells() {
                if !f.trim().is_empty() {
                    formula_at.insert((frow + r as u32, fcol + c as u32));
                }
            }
        }
    }
    info.formula_cells = formula_at.len();

    for (r, c, cell) in range.used_cells() {
        let abs = (start_row + r as u32, start_col + c as u32);
        info.non_empty_cells += 1;
        if !formula_at.contains(&abs) {
            info.value_cells += 1;
        }
        match cell {
            Data::String(_) => info.text_cells += 1,
            Data::Int(_) | Data::Float(_) => info.number_cells += 1,
            Data::DateTime(_) | Data::DateTimeIso(_) | Data::DurationIso(_) => info.date_cells += 1,
            Data::Bool(_) => info.bool_cells += 1,
            Data::Error(e) => {
                info.error_cells += 1;
                *info.error_kinds.entry(e.to_string()).or_insert(0) += 1;
            }
            Data::Empty => {}
        }
    }

    // A formula whose cached result was never saved is empty in the data range,
    // so it would otherwise vanish from "cells with data".
    let formula_without_value = info
        .formula_cells
        .saturating_sub(info.non_empty_cells.saturating_sub(info.value_cells));
    if formula_without_value > 0 {
        info.non_empty_cells += formula_without_value;
    }

    info
}

/// Count the OPC package parts that correspond to workbook objects. Only the ZIP
/// central directory is read — no entry is decompressed.
fn scan_package_parts(bytes: &[u8]) -> Option<ObjectCounts> {
    let archive = zip::ZipArchive::new(Cursor::new(bytes.to_vec())).ok()?;
    let mut counts = ObjectCounts::default();
    for raw in archive.file_names() {
        let name = raw.trim_start_matches('/').to_ascii_lowercase();
        let (dir, file) = match name.rsplit_once('/') {
            Some((d, f)) => (d, f),
            None => ("", name.as_str()),
        };
        match dir {
            "xl/tables" if file.starts_with("table") && file.ends_with(".xml") => {
                counts.tables += 1
            }
            "xl/pivottables" if file.starts_with("pivottable") && file.ends_with(".xml") => {
                counts.pivot_tables += 1
            }
            "xl/charts" if file.starts_with("chart") && file.ends_with(".xml") => {
                // chartN.xml and chartExN.xml are charts; colorsN.xml and
                // styleN.xml are their sidecars and must not be counted.
                counts.charts += 1
            }
            "xl/media" => counts.images += 1,
            "xl/drawings" if file.starts_with("drawing") && file.ends_with(".xml") => {
                counts.drawings += 1
            }
            "xl/threadedcomments" if file.ends_with(".xml") => counts.comments += 1,
            "xl/externallinks" if file.starts_with("externallink") && file.ends_with(".xml") => {
                counts.external_links += 1
            }
            "xl" => {
                if file.starts_with("comments") && file.ends_with(".xml") {
                    counts.comments += 1;
                } else if file == "connections.xml" {
                    counts.has_data_connections = true;
                } else if file == "vbaproject.bin" {
                    counts.has_macros = true;
                }
            }
            _ => {}
        }
    }
    Some(counts)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sheet_kind(t: SheetType) -> &'static str {
    match t {
        SheetType::WorkSheet => "worksheet",
        SheetType::DialogSheet => "dialog sheet",
        SheetType::MacroSheet => "macro sheet",
        SheetType::ChartSheet => "chart sheet",
        SheetType::Vba => "vba module",
    }
}

fn sheet_visibility(v: SheetVisible) -> &'static str {
    match v {
        SheetVisible::Visible => "visible",
        SheetVisible::Hidden => "hidden",
        SheetVisible::VeryHidden => "very hidden",
    }
}

/// 0-based (row, col) → A1 notation (`(0, 0)` → `A1`).
fn a1(row: u32, col: u32) -> String {
    let mut name = String::new();
    let mut c = col.min(MAX_COL) as i64;
    loop {
        name.insert(0, (b'A' + (c % 26) as u8) as char);
        c = c / 26 - 1;
        if c < 0 {
            break;
        }
    }
    format!("{name}{}", row as u64 + 1)
}

/// Pull the sheet name out of a defined-name reference such as
/// `Sheet1!$A$1:$D$40` or `='My Sheet'!$A$1`.
fn sheet_of_reference(refers_to: &str) -> Option<String> {
    let body = refers_to.trim().trim_start_matches('=').trim();

    // The sheet name ends at the first `!` that is NOT inside a quoted name —
    // `'My Sheet'!$A$1` must not be cut at the space, and `#REF!#REF!` must be
    // cut at the first bang so the dangling name is recognised.
    let mut in_quotes = false;
    let mut bang = None;
    for (i, ch) in body.char_indices() {
        match ch {
            '\'' => in_quotes = !in_quotes,
            '!' if !in_quotes => {
                bang = Some(i);
                break;
            }
            _ => {}
        }
    }
    let mut sheet = body[..bang?].trim().to_string();

    if sheet.starts_with('\'') && sheet.ends_with('\'') && sheet.len() >= 2 {
        sheet = sheet[1..sheet.len() - 1].replace("''", "'");
    }
    // Cross-workbook references carry a [Book.xlsx] prefix; keep just the sheet.
    if let Some(close) = sheet.rfind(']') {
        sheet = sheet[close + 1..].to_string();
    }
    if sheet.is_empty() || sheet.contains("#REF") {
        None
    } else {
        Some(sheet)
    }
}

/// Resolve a sheet selector (name, else 0-based index) to an index.
fn resolve_sheet_index(names: &[String], sel: &str) -> Result<usize, String> {
    if let Some(i) = names.iter().position(|n| n == sel) {
        return Ok(i);
    }
    if let Ok(i) = sel.parse::<usize>() {
        if i < names.len() {
            return Ok(i);
        }
        return Err(format!(
            "sheet index {i} is out of range: workbook has {} sheet(s) (0-{})",
            names.len(),
            names.len() - 1
        ));
    }
    Err(format!(
        "sheet {sel:?} not found: available sheets are {}",
        names
            .iter()
            .map(|n| format!("{n:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

// ---------------------------------------------------------------------------
// Renderers
// ---------------------------------------------------------------------------

/// Render `rows` (first row = header) as a fixed-width table, indented by two
/// spaces.
fn fixed_width(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0usize; cols];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let mut out = String::new();
    for row in rows {
        let mut line = String::from("  ");
        for (i, cell) in row.iter().enumerate() {
            if i + 1 == row.len() {
                line.push_str(cell);
            } else {
                line.push_str(cell);
                let pad = widths[i].saturating_sub(cell.chars().count()) + 2;
                line.push_str(&" ".repeat(pad));
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn render_table(r: &Report) -> String {
    let mut out = String::new();

    out.push_str("Workbook overview\n");
    let overview = vec![
        vec![
            "Format:".to_string(),
            format!("{} ({})", r.format, container_label(r.container)),
        ],
        vec!["Size:".to_string(), format!("{} bytes", r.file_bytes)],
        vec![
            "Sheets:".to_string(),
            format!(
                "{} ({} visible, {} hidden, {} very hidden)",
                r.sheets_total, r.visible_sheets, r.hidden_sheets, r.very_hidden_sheets
            ),
        ],
        vec![
            "Cells with data:".to_string(),
            r.total(|s| s.non_empty_cells).to_string(),
        ],
        vec![
            "Formula cells:".to_string(),
            r.total(|s| s.formula_cells).to_string(),
        ],
        vec![
            "Value cells:".to_string(),
            r.total(|s| s.value_cells).to_string(),
        ],
        vec![
            "Error cells:".to_string(),
            r.total(|s| s.error_cells).to_string(),
        ],
        vec![
            "Named ranges:".to_string(),
            if r.named_ranges_broken > 0 {
                format!(
                    "{} ({} broken)",
                    r.named_ranges_total, r.named_ranges_broken
                )
            } else {
                r.named_ranges_total.to_string()
            },
        ],
    ];
    out.push_str(&fixed_width(&overview));

    out.push('\n');
    out.push_str(&format!("Sheets reported ({})\n", r.sheets.len()));
    let mut rows = vec![vec![
        "#".to_string(),
        "Name".to_string(),
        "Type".to_string(),
        "Visibility".to_string(),
        "Used range".to_string(),
        "Rows".to_string(),
        "Cols".to_string(),
        "Cells".to_string(),
        "Formulas".to_string(),
        "Values".to_string(),
        "Text".to_string(),
        "Number".to_string(),
        "Date".to_string(),
        "Bool".to_string(),
        "Error".to_string(),
    ]];
    for s in &r.sheets {
        rows.push(vec![
            s.index.to_string(),
            s.name.clone(),
            s.kind.to_string(),
            s.visibility.to_string(),
            s.used_range.clone().unwrap_or_else(|| "-".to_string()),
            s.rows.to_string(),
            s.columns.to_string(),
            s.non_empty_cells.to_string(),
            s.formula_cells.to_string(),
            s.value_cells.to_string(),
            s.text_cells.to_string(),
            s.number_cells.to_string(),
            s.date_cells.to_string(),
            s.bool_cells.to_string(),
            s.error_cells.to_string(),
        ]);
    }
    out.push_str(&fixed_width(&rows));
    for s in &r.sheets {
        if let Some(note) = &s.note {
            out.push_str(&format!("  note: {} — {}\n", s.name, note));
        }
        if !s.error_kinds.is_empty() {
            let kinds = s
                .error_kinds
                .iter()
                .map(|(k, v)| format!("{k} x{v}"))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("  errors: {} — {}\n", s.name, kinds));
        }
    }

    if !r.named_ranges.is_empty() {
        out.push('\n');
        out.push_str(&format!(
            "Named ranges ({} of {})\n",
            r.named_ranges.len(),
            r.named_ranges_total
        ));
        let mut nrows = vec![vec![
            "Name".to_string(),
            "Refers to".to_string(),
            "Sheet".to_string(),
            "Status".to_string(),
        ]];
        for n in &r.named_ranges {
            nrows.push(vec![
                n.name.clone(),
                n.refers_to.clone(),
                n.sheet.clone().unwrap_or_else(|| "-".to_string()),
                if n.broken { "broken (#REF!)" } else { "ok" }.to_string(),
            ]);
        }
        out.push_str(&fixed_width(&nrows));
    } else if r.named_ranges_total == 0 {
        out.push_str("\nNamed ranges (0)\n  none defined\n");
    }

    if let Some(o) = &r.objects {
        out.push('\n');
        out.push_str("Workbook objects\n");
        let orows = vec![
            vec!["Tables:".to_string(), o.tables.to_string()],
            vec!["Pivot tables:".to_string(), o.pivot_tables.to_string()],
            vec!["Charts:".to_string(), o.charts.to_string()],
            vec!["Images:".to_string(), o.images.to_string()],
            vec!["Comments:".to_string(), o.comments.to_string()],
            vec!["Drawings:".to_string(), o.drawings.to_string()],
            vec!["External links:".to_string(), o.external_links.to_string()],
            vec![
                "Data connections:".to_string(),
                yes_no(o.has_data_connections),
            ],
            vec!["Macros (VBA):".to_string(), yes_no(o.has_macros)],
        ];
        out.push_str(&fixed_width(&orows));
    }

    out.push('\n');
    out.push_str("Notes\n");
    for n in &r.notes {
        out.push_str(&format!("  - {n}\n"));
    }
    out
}

fn container_label(container: &str) -> &'static str {
    match container {
        "opc" => "Office Open XML, ZIP package",
        "cfb" => "legacy binary, CFB container",
        _ => "OpenDocument, ZIP package",
    }
}

fn yes_no(b: bool) -> String {
    if b { "yes" } else { "no" }.to_string()
}

fn render_json(r: &Report) -> String {
    let sheets: Vec<_> = r
        .sheets
        .iter()
        .map(|s| {
            json!({
                "index": s.index,
                "name": s.name,
                "type": s.kind,
                "visibility": s.visibility,
                "used_range": s.used_range,
                "rows": s.rows,
                "columns": s.columns,
                "non_empty_cells": s.non_empty_cells,
                "formula_cells": s.formula_cells,
                "value_cells": s.value_cells,
                "cell_types": {
                    "text": s.text_cells,
                    "number": s.number_cells,
                    "date": s.date_cells,
                    "boolean": s.bool_cells,
                    "error": s.error_cells,
                },
                "error_kinds": s.error_kinds,
                "note": s.note,
            })
        })
        .collect();

    let named: Vec<_> = r
        .named_ranges
        .iter()
        .map(|n| {
            json!({
                "name": n.name,
                "refers_to": n.refers_to,
                "sheet": n.sheet,
                "broken": n.broken,
            })
        })
        .collect();

    let objects = r.objects.as_ref().map(|o| {
        json!({
            "tables": o.tables,
            "pivot_tables": o.pivot_tables,
            "charts": o.charts,
            "images": o.images,
            "comments": o.comments,
            "drawings": o.drawings,
            "external_links": o.external_links,
            "has_data_connections": o.has_data_connections,
            "has_macros": o.has_macros,
        })
    });

    let value = json!({
        "workbook": {
            "format": r.format,
            "container": r.container,
            "file_bytes": r.file_bytes,
            "sheets_total": r.sheets_total,
            "visible_sheets": r.visible_sheets,
            "hidden_sheets": r.hidden_sheets,
            "very_hidden_sheets": r.very_hidden_sheets,
            "cells_with_data": r.total(|s| s.non_empty_cells),
            "formula_cells": r.total(|s| s.formula_cells),
            "value_cells": r.total(|s| s.value_cells),
            "error_cells": r.total(|s| s.error_cells),
            "named_ranges_total": r.named_ranges_total,
            "named_ranges_broken": r.named_ranges_broken,
        },
        "sheets": sheets,
        "named_ranges": named,
        "objects": objects,
        "notes": r.notes,
    });
    let mut s = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string());
    s.push('\n');
    s
}

/// One RFC-4180 row per reported sheet. Named ranges and object counts are not
/// tabular per sheet, so they stay in the `table`/`json` renderers.
fn render_csv(r: &Report) -> String {
    let mut out = String::new();
    out.push_str("index,name,type,visibility,used_range,rows,columns,non_empty_cells,formula_cells,value_cells,text_cells,number_cells,date_cells,bool_cells,error_cells\r\n");
    for s in &r.sheets {
        let fields = [
            s.index.to_string(),
            s.name.clone(),
            s.kind.to_string(),
            s.visibility.to_string(),
            s.used_range.clone().unwrap_or_default(),
            s.rows.to_string(),
            s.columns.to_string(),
            s.non_empty_cells.to_string(),
            s.formula_cells.to_string(),
            s.value_cells.to_string(),
            s.text_cells.to_string(),
            s.number_cells.to_string(),
            s.date_cells.to_string(),
            s.bool_cells.to_string(),
            s.error_cells.to_string(),
        ];
        out.push_str(
            &fields
                .iter()
                .map(|f| csv_field(f))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push_str("\r\n");
    }
    out
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_xlsxwriter::{Formula, Workbook};

    /// Three sheets: a data sheet with text/number/bool/formula cells, a hidden
    /// sheet, and an empty sheet. Plus two defined names, one broken.
    fn fixture() -> Vec<u8> {
        let mut wb = Workbook::new();

        let s1 = wb.add_worksheet().set_name("Sales").unwrap();
        s1.write_string(0, 0, "Region").unwrap();
        s1.write_string(0, 1, "Units").unwrap();
        s1.write_string(1, 0, "North").unwrap();
        s1.write_number(1, 1, 10.0).unwrap();
        s1.write_string(2, 0, "South").unwrap();
        s1.write_number(2, 1, 32.0).unwrap();
        s1.write_boolean(3, 0, true).unwrap();
        s1.write_formula(3, 1, Formula::new("=SUM(B2:B3)").set_result("42"))
            .unwrap();

        let s2 = wb.add_worksheet().set_name("Notes").unwrap();
        s2.set_hidden(true);
        s2.write_string(0, 0, "internal").unwrap();

        wb.add_worksheet().set_name("Blank").unwrap();

        wb.define_name("SalesData", "=Sales!$A$1:$B$4").unwrap();
        wb.define_name("Gone", "=#REF!#REF!").unwrap();

        wb.save_to_buffer().unwrap()
    }

    #[test]
    fn table_report_covers_every_sheet_and_the_formula_value_split() {
        let out = inspect(&fixture(), &Options::default(), "table").unwrap();
        assert!(out.contains("Sheets:"), "{out}");
        assert!(out.contains("Sales"), "{out}");
        assert!(out.contains("Notes"), "{out}");
        assert!(out.contains("Blank"), "{out}");
        assert!(out.contains("hidden"), "{out}");
        assert!(out.contains("A1:B4"), "{out}");
        assert!(out.contains("SalesData"), "{out}");
        assert!(out.contains("broken (#REF!)"), "{out}");
    }

    #[test]
    fn json_report_counts_cells_by_kind() {
        let out = inspect(&fixture(), &Options::default(), "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();

        assert_eq!(v["workbook"]["format"], "xlsx");
        assert_eq!(v["workbook"]["sheets_total"], 3);
        assert_eq!(v["workbook"]["visible_sheets"], 2);
        assert_eq!(v["workbook"]["hidden_sheets"], 1);

        let sales = &v["sheets"][0];
        assert_eq!(sales["name"], "Sales");
        assert_eq!(sales["used_range"], "A1:B4");
        assert_eq!(sales["rows"], 4);
        assert_eq!(sales["columns"], 2);
        // 8 cells: 4 text, 2 numbers, 1 boolean, 1 formula (cached 42).
        assert_eq!(sales["non_empty_cells"], 8);
        assert_eq!(sales["formula_cells"], 1);
        assert_eq!(sales["value_cells"], 7);
        assert_eq!(sales["cell_types"]["text"], 4);
        assert_eq!(sales["cell_types"]["boolean"], 1);
        assert_eq!(sales["cell_types"]["error"], 0);

        let blank = &v["sheets"][2];
        assert_eq!(blank["name"], "Blank");
        assert_eq!(blank["non_empty_cells"], 0);
        assert_eq!(blank["used_range"], serde_json::Value::Null);

        assert_eq!(v["workbook"]["named_ranges_total"], 2);
        assert_eq!(v["workbook"]["named_ranges_broken"], 1);
        let names = v["named_ranges"].as_array().unwrap();
        let find = |n: &str| {
            names
                .iter()
                .find(|e| e["name"] == n)
                .unwrap_or_else(|| panic!("named range {n} missing from {out}"))
        };
        assert_eq!(find("SalesData")["sheet"], "Sales");
        assert_eq!(find("SalesData")["broken"], false);
        assert_eq!(find("Gone")["broken"], true);
        assert_eq!(find("Gone")["sheet"], serde_json::Value::Null);
    }

    #[test]
    fn object_counts_read_the_package_parts() {
        let out = inspect(&fixture(), &Options::default(), "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        // A plain generated workbook has no tables/charts/media and no macros —
        // the section must still be PRESENT (not null) for an OPC package.
        assert!(v["objects"].is_object(), "{out}");
        assert_eq!(v["objects"]["charts"], 0);
        assert_eq!(v["objects"]["has_macros"], false);
    }

    #[test]
    fn include_hidden_false_drops_the_hidden_sheet() {
        let opts = Options {
            include_hidden: false,
            ..Options::default()
        };
        let out = inspect(&fixture(), &opts, "csv").unwrap();
        assert!(out.contains("Sales"), "{out}");
        assert!(!out.contains("Notes"), "{out}");
        // Header + Sales + Blank.
        assert_eq!(out.lines().count(), 3, "{out}");
    }

    #[test]
    fn sheet_selector_accepts_name_and_index() {
        let by_name = Options {
            sheet: Some("Notes".to_string()),
            ..Options::default()
        };
        let by_index = Options {
            sheet: Some("1".to_string()),
            ..Options::default()
        };
        let a = inspect(&fixture(), &by_name, "csv").unwrap();
        let b = inspect(&fixture(), &by_index, "csv").unwrap();
        assert_eq!(a, b);
        assert!(a.contains("Notes,worksheet,hidden"), "{a}");
        assert_eq!(a.lines().count(), 2, "{a}");
    }

    #[test]
    fn csv_is_rfc4180_with_a_header_row() {
        let out = inspect(&fixture(), &Options::default(), "csv").unwrap();
        assert!(
            out.starts_with("index,name,type,visibility,used_range,"),
            "{out}"
        );
        assert!(out.ends_with("\r\n"), "{out}");
        assert_eq!(out.lines().count(), 4);
    }

    #[test]
    fn unknown_sheet_names_the_available_sheets() {
        let opts = Options {
            sheet: Some("Nope".to_string()),
            ..Options::default()
        };
        let err = inspect(&fixture(), &opts, "table").unwrap_err();
        assert!(err.contains("not found"), "{err}");
        assert!(err.contains("\"Sales\""), "{err}");
    }

    #[test]
    fn out_of_range_sheet_index_is_rejected() {
        let opts = Options {
            sheet: Some("9".to_string()),
            ..Options::default()
        };
        let err = inspect(&fixture(), &opts, "table").unwrap_err();
        assert!(err.contains("out of range"), "{err}");
        assert!(err.contains("workbook has 3 sheet(s)"), "{err}");
    }

    #[test]
    fn non_workbook_bytes_are_rejected() {
        let err = inspect(
            b"this is not a workbook at all",
            &Options::default(),
            "table",
        )
        .unwrap_err();
        assert!(err.contains("not a readable workbook"), "{err}");
        assert!(inspect(b"", &Options::default(), "table")
            .unwrap_err()
            .contains("empty workbook bytes"));
    }

    #[test]
    fn unknown_format_is_rejected() {
        let err = inspect(&fixture(), &Options::default(), "yaml").unwrap_err();
        assert!(err.contains("unknown format"), "{err}");
    }

    #[test]
    fn max_named_ranges_clips_and_says_so() {
        let opts = Options {
            max_named_ranges: 1,
            ..Options::default()
        };
        let out = inspect(&fixture(), &opts, "table").unwrap();
        assert!(out.contains("Named ranges (1 of 2)"), "{out}");
        assert!(out.contains("clipped at 1 of 2"), "{out}");
    }

    #[test]
    fn include_named_ranges_false_omits_the_section() {
        let opts = Options {
            include_named_ranges: false,
            ..Options::default()
        };
        let out = inspect(&fixture(), &opts, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["workbook"]["named_ranges_total"], 0);
        assert_eq!(v["named_ranges"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn a1_notation_rolls_over_columns() {
        assert_eq!(a1(0, 0), "A1");
        assert_eq!(a1(39, 3), "D40");
        assert_eq!(a1(0, 25), "Z1");
        assert_eq!(a1(0, 26), "AA1");
        assert_eq!(a1(1_048_575, 16_383), "XFD1048576");
    }

    #[test]
    fn reference_sheet_extraction_handles_quotes_and_workbooks() {
        assert_eq!(
            sheet_of_reference("=Sheet1!$A$1:$B$2").as_deref(),
            Some("Sheet1")
        );
        assert_eq!(
            sheet_of_reference("='My Sheet'!$A$1").as_deref(),
            Some("My Sheet")
        );
        assert_eq!(
            sheet_of_reference("=[Book1.xlsx]Data!$A$1").as_deref(),
            Some("Data")
        );
        assert_eq!(sheet_of_reference("=#REF!#REF!"), None);
        assert_eq!(sheet_of_reference("=42"), None);
    }
}
