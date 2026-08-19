//! spreadsheet-formula-audit core — scan a workbook (`.xlsx`/`.xlsm`/`.xls`/
//! `.ods`) for broken formulas and report the offending cells.
//!
//! No wafer/wasm-bindgen deps; pure logic shared by the chat skill block (and
//! host-testable). Reads via `calamine` (pure Rust, no C deps → compiles to
//! wasm32-unknown-unknown), using the same reader pattern as the sibling
//! `blocks/xlsx-to-csv` and `blocks/spreadsheet-to-sql` blocks — plus
//! `worksheet_formula`, which hands us the stored formula TEXT of every cell
//! (the same call `blocks/xlsx-sheet-diff` uses to diff formulas).
//!
//! What it detects, all from the stored file (no recalculation engine):
//!
//! * `ref_error` — a formula whose text contains `#REF!`, i.e. it points at a
//!   row/column/sheet that was deleted out from under it.
//! * `error_value` — a cell whose cached value is an Excel error
//!   (`#DIV/0!`, `#VALUE!`, `#NAME?`, `#N/A`, `#NUM!`, `#NULL!`, `#REF!`).
//! * `circular_reference` — a cycle in the formula dependency graph, built by
//!   parsing A1-style references out of every formula and linking each formula
//!   cell to the formula cells it reads. Reported as the full chain.
//! * `broken_reference` — a formula that names a worksheet the workbook does
//!   not contain (a rename/delete left the reference dangling).
//! * `external_link` — a formula referencing another workbook (`[Book1.xlsx]`),
//!   whose value cannot be verified from this file alone.
//!
//! Limits (stated on purpose — the report repeats them):
//! * Dependency edges come from literal A1 references. `INDIRECT`/`OFFSET`
//!   build references at calculation time, so cycles that only exist through
//!   them are not visible here.
//! * Structured table references (`Table1[Col]`) and defined names are not
//!   resolved to cells, so they contribute no edges (defined names ARE still
//!   scanned for `#REF!` text).
//! * Cached values are whatever the writing application last stored; a file
//!   saved without cached results has no `error_value` findings.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Cursor;

use calamine::{open_workbook_auto_from_rs, Data, Reader, SheetType, SheetVisible};

/// Highest 0-based row index Excel addresses (1_048_576 rows).
const MAX_ROW: u32 = 1_048_575;
/// Highest 0-based column index Excel addresses (16_384 columns, XFD).
const MAX_COL: u32 = 16_383;

/// Stop collecting formula cells past this many; the report says it truncated.
/// Keeps the dependency graph (and its DFS) bounded on a pathological workbook.
const MAX_FORMULA_CELLS: usize = 50_000;

/// Stop after this many distinct dependency cycles.
const MAX_CYCLES: usize = 100;

/// Every Excel error literal we recognize, longest first so `#N/A` cannot
/// shadow a longer literal that starts the same way.
const ERROR_LITERALS: &[&str] = &[
    "#GETTING_DATA",
    "#CONNECT!",
    "#BLOCKED!",
    "#UNKNOWN!",
    "#SPILL!",
    "#VALUE!",
    "#FIELD!",
    "#DIV/0!",
    "#CALC!",
    "#BUSY!",
    "#NAME?",
    "#NULL!",
    "#NUM!",
    "#REF!",
    "#N/A",
];

/// What kind of problem a finding describes. Ordering is REPORT ordering:
/// structural breakage first, informational last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// A dependency cycle between formula cells.
    Circular,
    /// A formula whose text contains `#REF!`.
    RefError,
    /// A formula naming a worksheet this workbook does not have.
    BrokenReference,
    /// A cell whose cached value is an Excel error.
    ErrorValue,
    /// A formula pointing at another workbook.
    ExternalLink,
}

impl Kind {
    /// Stable machine-readable label (used by the json/csv formats too).
    pub fn label(self) -> &'static str {
        match self {
            Kind::Circular => "circular_reference",
            Kind::RefError => "ref_error",
            Kind::BrokenReference => "broken_reference",
            Kind::ErrorValue => "error_value",
            Kind::ExternalLink => "external_link",
        }
    }
}

/// One reported problem, anchored at the cell that carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub kind: Kind,
    /// Worksheet name, or `(workbook)` for defined-name findings.
    pub sheet: String,
    /// A1 address, or the defined name for `(workbook)` findings.
    pub cell: String,
    /// The stored formula, when the finding comes from one.
    pub formula: Option<String>,
    /// Human-readable explanation of what is wrong.
    pub detail: String,
}

impl Finding {
    /// `Sheet1!B2` — how the finding is addressed in every output format.
    pub fn address(&self) -> String {
        format!("{}!{}", self.sheet, self.cell)
    }
}

/// Audit knobs. `Default` is the shipped default for every field.
#[derive(Debug, Clone)]
pub struct Options {
    /// Restrict REPORTED findings to one sheet (name, or 0-based index as a
    /// string). `None` → every worksheet. Cross-sheet cycles are still found:
    /// the dependency graph always spans the whole workbook.
    pub sheet: Option<String>,
    /// Build the dependency graph and report cycles.
    pub check_cycles: bool,
    /// Report cells whose cached value is an Excel error.
    pub check_error_values: bool,
    /// Also scan hidden / very-hidden worksheets.
    pub include_hidden: bool,
    /// Cap on reported findings (the report says when it clipped).
    pub max_findings: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            sheet: None,
            check_cycles: true,
            check_error_values: true,
            include_hidden: true,
            max_findings: 200,
        }
    }
}

/// Everything the audit learned, before rendering.
#[derive(Debug, Clone)]
pub struct Report {
    /// Worksheets whose findings are reported, in workbook order.
    pub sheets_scanned: Vec<String>,
    /// Formula cells read across the WHOLE workbook (the graph's node count).
    pub formula_cells: usize,
    /// Findings, already sorted and clipped to `max_findings`.
    pub findings: Vec<Finding>,
    /// True when `MAX_FORMULA_CELLS` clipped the scan.
    pub scan_truncated: bool,
    /// Findings dropped by `max_findings`.
    pub findings_omitted: usize,
}

/// Audit `bytes` and render the report in `format` (`table`, `json`, `csv`).
///
/// Returns `Err` on unreadable/empty bytes, an unknown sheet selector, or an
/// unknown format.
pub fn audit(bytes: &[u8], opts: &Options, format: &str) -> Result<String, String> {
    let report = analyze(bytes, opts)?;
    match format.trim().to_ascii_lowercase().as_str() {
        "table" => Ok(render_table(&report)),
        "json" => Ok(render_json(&report)),
        "csv" => Ok(render_csv(&report)),
        other => Err(format!(
            "unknown format {other:?} (expected \"table\", \"json\", or \"csv\")"
        )),
    }
}

/// Read the workbook and collect every finding. Split out of [`audit`] so the
/// unit tests can assert on structure rather than on rendered text.
pub fn analyze(bytes: &[u8], opts: &Options) -> Result<Report, String> {
    if bytes.is_empty() {
        return Err("empty spreadsheet bytes".to_string());
    }

    let mut wb = open_workbook_auto_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|e| format!("not a readable spreadsheet: {e}"))?;

    // Worksheets only: a chart/macro sheet has no cell grid to read.
    let sheets: Vec<(String, bool)> = wb
        .sheets_metadata()
        .iter()
        .filter(|s| s.typ == SheetType::WorkSheet)
        .map(|s| (s.name.clone(), s.visible == SheetVisible::Visible))
        .collect();
    if sheets.is_empty() {
        return Err("spreadsheet has no worksheets".to_string());
    }
    let names: Vec<String> = sheets.iter().map(|(n, _)| n.clone()).collect();

    // Which sheets' findings get reported (the graph still spans all of them).
    let reported: Vec<usize> = match opts
        .sheet
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(sel) => vec![resolve_sheet_index(&names, sel)?],
        None => (0..names.len())
            .filter(|&i| opts.include_hidden || sheets[i].1)
            .collect(),
    };
    let reported_set: HashSet<usize> = reported.iter().copied().collect();

    // ---- collect formulas across the whole workbook -----------------------
    let mut cells: Vec<FormulaCell> = Vec::new();
    let mut scan_truncated = false;
    for (idx, name) in names.iter().enumerate() {
        if cells.len() >= MAX_FORMULA_CELLS {
            scan_truncated = true;
            break;
        }
        // A sheet we cannot read formulas for (an odd/partial file) is skipped
        // rather than failing the whole audit.
        let Ok(range) = wb.worksheet_formula(name) else {
            continue;
        };
        let (r0, c0) = range.start().unwrap_or((0, 0));
        let (h, w) = range.get_size();
        for r in 0..h {
            for c in 0..w {
                let Some(text) = range.get((r, c)) else {
                    continue;
                };
                if text.is_empty() {
                    continue;
                }
                if cells.len() >= MAX_FORMULA_CELLS {
                    scan_truncated = true;
                    break;
                }
                cells.push(FormulaCell {
                    sheet: idx,
                    row: r0 + r as u32,
                    col: c0 + c as u32,
                    // calamine stores the formula without the leading `=`.
                    text: format!("={}", text.strip_prefix('=').unwrap_or(text)),
                });
            }
            if scan_truncated {
                break;
            }
        }
    }

    let mut findings: Vec<Finding> = Vec::new();
    let mut parsed: Vec<Parsed> = Vec::with_capacity(cells.len());

    for cell in &cells {
        let p = parse_formula(&cell.text);
        if reported_set.contains(&cell.sheet) {
            if p.has_ref_error {
                findings.push(Finding {
                    kind: Kind::RefError,
                    sheet: names[cell.sheet].clone(),
                    cell: a1(cell.row, cell.col),
                    formula: Some(cell.text.clone()),
                    detail: "formula contains #REF! — it points at a deleted cell, row, column, or sheet".to_string(),
                });
            }
            for book in &p.external_books {
                findings.push(Finding {
                    kind: Kind::ExternalLink,
                    sheet: names[cell.sheet].clone(),
                    cell: a1(cell.row, cell.col),
                    formula: Some(cell.text.clone()),
                    detail: format!(
                        "links to another workbook ({book}); its value cannot be checked from this file"
                    ),
                });
            }
            // Sheet names a formula mentions that this workbook does not have.
            let mut reported_names: HashSet<String> = HashSet::new();
            for r in &p.refs {
                let Some(sheet_name) = r.sheet.as_deref() else {
                    continue;
                };
                if find_sheet(&names, sheet_name).is_none()
                    && reported_names.insert(sheet_name.to_ascii_lowercase())
                {
                    findings.push(Finding {
                        kind: Kind::BrokenReference,
                        sheet: names[cell.sheet].clone(),
                        cell: a1(cell.row, cell.col),
                        formula: Some(cell.text.clone()),
                        detail: format!(
                            "references sheet {sheet_name:?}, which this workbook does not contain (renamed or deleted)"
                        ),
                    });
                }
            }
        }
        parsed.push(p);
    }

    // ---- defined names carrying #REF! -------------------------------------
    // Only when the whole workbook is in scope: a defined name is not owned by
    // one sheet, so a single-sheet audit would report it out of context.
    if opts.sheet.is_none() {
        for (name, formula) in wb.defined_names() {
            if parse_formula(formula).has_ref_error {
                findings.push(Finding {
                    kind: Kind::RefError,
                    sheet: "(workbook)".to_string(),
                    cell: name.clone(),
                    formula: Some(formula.clone()),
                    detail: "defined name contains #REF! — its target was deleted".to_string(),
                });
            }
        }
    }

    // ---- cached error values ----------------------------------------------
    if opts.check_error_values {
        // A cell already reported as a #REF! formula would otherwise be
        // reported twice (the formula AND its cached #REF! result).
        let ref_error_cells: HashSet<(usize, u32, u32)> = cells
            .iter()
            .zip(&parsed)
            .filter(|(_, p)| p.has_ref_error)
            .map(|(c, _)| (c.sheet, c.row, c.col))
            .collect();
        let formula_at: HashMap<(usize, u32, u32), &str> = cells
            .iter()
            .map(|c| ((c.sheet, c.row, c.col), c.text.as_str()))
            .collect();

        for &idx in &reported {
            let Ok(range) = wb.worksheet_range(&names[idx]) else {
                continue;
            };
            let (r0, c0) = range.start().unwrap_or((0, 0));
            let (h, w) = range.get_size();
            for r in 0..h {
                for c in 0..w {
                    let Some(value) = range.get((r, c)) else {
                        continue;
                    };
                    let Some(err) = error_text(value) else {
                        continue;
                    };
                    let (row, col) = (r0 + r as u32, c0 + c as u32);
                    if ref_error_cells.contains(&(idx, row, col)) {
                        continue;
                    }
                    findings.push(Finding {
                        kind: Kind::ErrorValue,
                        sheet: names[idx].clone(),
                        cell: a1(row, col),
                        formula: formula_at.get(&(idx, row, col)).map(|f| f.to_string()),
                        detail: format!("cell value is {err}"),
                    });
                }
            }
        }
    }

    // ---- dependency cycles -------------------------------------------------
    if opts.check_cycles {
        let graph = build_graph(&cells, &parsed, &names);
        for cycle in find_cycles(&graph) {
            // Report a cycle if ANY of its cells is in the reported scope, so a
            // single-sheet audit still surfaces cycles that leave the sheet.
            if !cycle
                .iter()
                .any(|&n| reported_set.contains(&cells[n].sheet))
            {
                continue;
            }
            let chain: Vec<String> = cycle
                .iter()
                .chain(cycle.first())
                .map(|&n| {
                    format!(
                        "{}!{}",
                        names[cells[n].sheet],
                        a1(cells[n].row, cells[n].col)
                    )
                })
                .collect();
            let head = &cells[cycle[0]];
            findings.push(Finding {
                kind: Kind::Circular,
                sheet: names[head.sheet].clone(),
                cell: a1(head.row, head.col),
                formula: Some(head.text.clone()),
                detail: if cycle.len() == 1 {
                    format!("formula refers to its own cell ({})", chain[0])
                } else {
                    format!("circular chain: {}", chain.join(" → "))
                },
            });
        }
    }

    // Stable ordering: kind severity, then sheet, then cell address.
    findings.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.sheet.cmp(&b.sheet))
            .then_with(|| cell_sort_key(&a.cell).cmp(&cell_sort_key(&b.cell)))
            .then_with(|| a.detail.cmp(&b.detail))
    });

    let findings_omitted = findings.len().saturating_sub(opts.max_findings);
    findings.truncate(opts.max_findings);

    Ok(Report {
        sheets_scanned: reported.iter().map(|&i| names[i].clone()).collect(),
        formula_cells: cells.len(),
        findings,
        scan_truncated,
        findings_omitted,
    })
}

/// A formula cell: which sheet it lives on, where, and its text (with `=`).
#[derive(Debug, Clone)]
struct FormulaCell {
    sheet: usize,
    row: u32,
    col: u32,
    text: String,
}

/// A rectangular reference a formula makes. `sheet` is the written sheet name
/// (`None` = the formula's own sheet); bounds are inclusive 0-based.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RefRect {
    sheet: Option<String>,
    r1: u32,
    c1: u32,
    r2: u32,
    c2: u32,
}

/// Everything one formula's text tells us.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Parsed {
    refs: Vec<RefRect>,
    /// The text contains a literal `#REF!` outside a string.
    has_ref_error: bool,
    /// Workbook names referenced via `[Book.xlsx]` / `[1]`.
    external_books: Vec<String>,
}

/// One token of a reference: a full cell, a bare column, or a bare row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tok {
    Cell(u32, u32),
    Col(u32),
    Row(u32),
}

/// Pull every A1-style reference, `#REF!` marker, and external-workbook name
/// out of a formula's text.
///
/// Deliberately conservative: string literals are skipped, a token immediately
/// followed by `(` is a function call (so `LOG10(` is not cell `LOG10`), a
/// token must be a syntactically valid Excel address (≤3 letters within column
/// XFD, row 1..=1_048_576), and a bare column/row only counts inside a range
/// (`A:A`, `2:5`).
fn parse_formula(formula: &str) -> Parsed {
    let ch: Vec<char> = formula.chars().collect();
    let n = ch.len();
    let mut out = Parsed::default();
    let mut i = 0usize;
    // Index just past the last `]` — a run starting exactly there is the sheet
    // part of an external reference (`[1]Sheet1!A1`), not a local sheet.
    let mut after_bracket = usize::MAX;
    // Previous char, to avoid starting a token mid-identifier (`1E5`).
    let mut prev: Option<char> = None;

    while i < n {
        let c = ch[i];

        // "quoted string" — never contains references.
        if c == '"' {
            i += 1;
            while i < n {
                if ch[i] == '"' {
                    // `""` is an escaped quote inside the string.
                    if i + 1 < n && ch[i + 1] == '"' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            prev = Some('"');
            continue;
        }

        // #REF! and friends.
        if c == '#' {
            if let Some(lit) = match_error_literal(&ch[i..]) {
                if lit == "#REF!" {
                    out.has_ref_error = true;
                }
                i += lit.chars().count();
                prev = Some('!');
                continue;
            }
            i += 1;
            prev = Some(c);
            continue;
        }

        // [Book1.xlsx] / [1] — an external workbook link.
        if c == '[' {
            if let Some(close) = (i + 1..n).find(|&j| ch[j] == ']') {
                let book: String = ch[i + 1..close].iter().collect();
                if !out.external_books.iter().any(|b| b == &book) {
                    out.external_books.push(book);
                }
                i = close + 1;
                after_bracket = i;
                prev = Some(']');
                continue;
            }
            i += 1;
            prev = Some(c);
            continue;
        }

        // 'Sheet with spaces'!A1 — or '[Book.xlsx]Sheet1'!A1.
        if c == '\'' {
            let mut j = i + 1;
            let mut name = String::new();
            while j < n {
                if ch[j] == '\'' {
                    if j + 1 < n && ch[j + 1] == '\'' {
                        name.push('\'');
                        j += 2;
                        continue;
                    }
                    j += 1;
                    break;
                }
                name.push(ch[j]);
                j += 1;
            }
            if j < n && ch[j] == '!' {
                let external = name.contains('[');
                if external {
                    if let (Some(a), Some(b)) = (name.find('['), name.find(']')) {
                        if a < b {
                            let book = name[a + 1..b].to_string();
                            if !out.external_books.iter().any(|x| x == &book) {
                                out.external_books.push(book);
                            }
                        }
                    }
                }
                if let Some((mut rect, next)) = parse_ref_at(&ch, j + 1) {
                    // An external reference names a sheet in ANOTHER workbook;
                    // do not check it against this workbook's sheet list.
                    rect.sheet = if external { None } else { Some(name.clone()) };
                    if !external {
                        out.refs.push(rect);
                    }
                    i = next;
                    prev = Some(']');
                    continue;
                }
            }
            i = j.max(i + 1);
            prev = Some('\'');
            continue;
        }

        // An unquoted token: a sheet prefix, a function name, or a reference.
        let startable = c.is_ascii_alphanumeric() || c == '$' || c == '_';
        let blocked = matches!(prev, Some(p) if p.is_ascii_alphanumeric() || p == '_' || p == '.' || p == '$');
        if startable && !blocked {
            let (tok, j) = read_token(&ch, i);
            if j < n && ch[j] == '!' {
                // Sheet1!… — the token names a sheet.
                let external = i == after_bracket;
                if let Some((mut rect, next)) = parse_ref_at(&ch, j + 1) {
                    rect.sheet = if external { None } else { Some(tok) };
                    if !external {
                        out.refs.push(rect);
                    }
                    i = next;
                    prev = Some(']');
                    continue;
                }
                i = j + 1;
                prev = Some('!');
                continue;
            }
            if let Some((rect, next)) = parse_ref_at(&ch, i) {
                out.refs.push(rect);
                i = next;
                prev = Some(']');
                continue;
            }
            i = j.max(i + 1);
            prev = ch.get(i - 1).copied();
            continue;
        }

        i += 1;
        prev = Some(c);
    }

    out
}

/// Longest error literal starting at `ch[0]`, if any.
fn match_error_literal(ch: &[char]) -> Option<&'static str> {
    let head: String = ch.iter().take(14).collect::<String>().to_ascii_uppercase();
    ERROR_LITERALS.iter().copied().find(|l| head.starts_with(l))
}

/// Maximal run of `[A-Za-z0-9_.$]` starting at `i`, plus the index after it.
fn read_token(ch: &[char], i: usize) -> (String, usize) {
    let mut j = i;
    while j < ch.len() {
        let c = ch[j];
        if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '$' {
            j += 1;
        } else {
            break;
        }
    }
    (ch[i..j].iter().collect(), j)
}

/// Parse a cell reference or range at `i` (sheet prefix already consumed).
/// Returns the rectangle and the index just past it.
fn parse_ref_at(ch: &[char], i: usize) -> Option<(RefRect, usize)> {
    let n = ch.len();
    let (tok1, j) = read_token(ch, i);
    if tok1.is_empty() || (j < n && ch[j] == '(') {
        return None; // a function call, not a reference
    }
    let t1 = classify_token(&tok1);

    if j < n && ch[j] == ':' {
        let (tok2, k) = read_token(ch, j + 1);
        if !tok2.is_empty() && !(k < n && ch[k] == '(') {
            if let (Some(a), Some(b)) = (t1, classify_token(&tok2)) {
                if let Some(rect) = combine(a, b) {
                    return Some((rect, k));
                }
            }
        }
    }

    match t1 {
        Some(Tok::Cell(r, c)) => Some((
            RefRect {
                sheet: None,
                r1: r,
                c1: c,
                r2: r,
                c2: c,
            },
            j,
        )),
        // A bare column/row on its own is not a reference.
        _ => None,
    }
}

/// Classify `$A$1` / `A` / `12` into a reference token, or `None` when the text
/// is not a syntactically valid Excel address part.
fn classify_token(tok: &str) -> Option<Tok> {
    let t: String = tok.chars().filter(|&c| c != '$').collect();
    if t.is_empty() {
        return None;
    }
    let letters: String = t.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let digits = &t[letters.len()..];
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let col = if letters.is_empty() {
        None
    } else {
        col_index(&letters)?.into()
    };
    let row = if digits.is_empty() {
        None
    } else {
        let r: u32 = digits.parse().ok()?;
        if r == 0 || r > MAX_ROW + 1 {
            return None;
        }
        Some(r - 1)
    };
    match (col, row) {
        (Some(c), Some(r)) => Some(Tok::Cell(r, c)),
        (Some(c), None) => Some(Tok::Col(c)),
        (None, Some(r)) => Some(Tok::Row(r)),
        (None, None) => None,
    }
}

/// 0-based column index for bijective base-26 letters, or `None` past XFD.
fn col_index(letters: &str) -> Option<u32> {
    if letters.len() > 3 {
        return None;
    }
    let mut n: u32 = 0;
    for c in letters.chars() {
        n = n
            .checked_mul(26)?
            .checked_add(c.to_ascii_uppercase() as u32 - 'A' as u32 + 1)?;
    }
    let idx = n.checked_sub(1)?;
    (idx <= MAX_COL).then_some(idx)
}

/// Build the rectangle two matching range endpoints describe.
fn combine(a: Tok, b: Tok) -> Option<RefRect> {
    let (r1, c1, r2, c2) = match (a, b) {
        (Tok::Cell(ra, ca), Tok::Cell(rb, cb)) => (ra.min(rb), ca.min(cb), ra.max(rb), ca.max(cb)),
        // A:C — whole columns.
        (Tok::Col(ca), Tok::Col(cb)) => (0, ca.min(cb), MAX_ROW, ca.max(cb)),
        // 2:5 — whole rows.
        (Tok::Row(ra), Tok::Row(rb)) => (ra.min(rb), 0, ra.max(rb), MAX_COL),
        _ => return None,
    };
    Some(RefRect {
        sheet: None,
        r1,
        c1,
        r2,
        c2,
    })
}

/// Case-insensitive sheet lookup (Excel sheet names are case-insensitive).
fn find_sheet(names: &[String], want: &str) -> Option<usize> {
    let want = want.trim();
    names
        .iter()
        .position(|n| n.eq_ignore_ascii_case(want))
        .or_else(|| names.iter().position(|n| n == want))
}

/// Pick the worksheet to report on: exact name first (so a sheet literally
/// named `0` stays reachable), else a 0-based index.
fn resolve_sheet_index(names: &[String], sel: &str) -> Result<usize, String> {
    if let Some(i) = names.iter().position(|n| n.as_str() == sel) {
        return Ok(i);
    }
    if let Ok(idx) = sel.parse::<usize>() {
        return if idx < names.len() {
            Ok(idx)
        } else {
            Err(format!(
                "sheet index {idx} out of range (workbook has {} sheet(s): {names:?})",
                names.len()
            ))
        };
    }
    if let Some(i) = find_sheet(names, sel) {
        return Ok(i);
    }
    Err(format!("no sheet named {sel:?} (available: {names:?})"))
}

/// Adjacency list over formula cells: `graph[u]` are the formula cells `u`
/// reads.
fn build_graph(cells: &[FormulaCell], parsed: &[Parsed], names: &[String]) -> Vec<Vec<usize>> {
    // Per sheet, formula cells sorted by (row, col) so a rectangle lookup can
    // binary-search the row band instead of expanding the rectangle (a whole
    // column reference covers a million cells).
    let mut by_sheet: BTreeMap<usize, Vec<(u32, u32, usize)>> = BTreeMap::new();
    for (id, c) in cells.iter().enumerate() {
        by_sheet
            .entry(c.sheet)
            .or_default()
            .push((c.row, c.col, id));
    }
    for v in by_sheet.values_mut() {
        v.sort_unstable();
    }

    let mut graph = vec![Vec::new(); cells.len()];
    for (id, (cell, p)) in cells.iter().zip(parsed).enumerate() {
        let mut targets: Vec<usize> = Vec::new();
        for r in &p.refs {
            let sheet = match r.sheet.as_deref() {
                Some(name) => match find_sheet(names, name) {
                    Some(i) => i,
                    None => continue, // dangling sheet — reported separately
                },
                None => cell.sheet,
            };
            let Some(list) = by_sheet.get(&sheet) else {
                continue;
            };
            // First entry with row >= r1.
            let start = list.partition_point(|&(row, _, _)| row < r.r1);
            for &(row, col, other) in &list[start..] {
                if row > r.r2 {
                    break;
                }
                if col >= r.c1 && col <= r.c2 {
                    targets.push(other);
                }
            }
        }
        targets.sort_unstable();
        targets.dedup();
        graph[id] = targets;
    }
    graph
}

/// Every distinct dependency cycle reachable by DFS, each as the chain of node
/// ids from the cycle's entry point back to itself (the repeat is implied).
///
/// Iterative (an explicit stack) so a deep dependency chain cannot blow the
/// wasm stack. Cycles are deduped by their canonical rotation, so the same loop
/// found from two entry points is reported once.
fn find_cycles(graph: &[Vec<usize>]) -> Vec<Vec<usize>> {
    const WHITE: u8 = 0;
    const GRAY: u8 = 1;
    const BLACK: u8 = 2;

    let n = graph.len();
    let mut color = vec![WHITE; n];
    let mut cycles: Vec<Vec<usize>> = Vec::new();
    let mut seen: HashSet<Vec<usize>> = HashSet::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut path: Vec<usize> = Vec::new();

    for start in 0..n {
        if color[start] != WHITE {
            continue;
        }
        color[start] = GRAY;
        path.push(start);
        stack.push((start, 0));

        while let Some(&(u, idx)) = stack.last() {
            if idx < graph[u].len() {
                stack.last_mut().unwrap().1 += 1;
                let v = graph[u][idx];
                match color[v] {
                    WHITE => {
                        color[v] = GRAY;
                        path.push(v);
                        stack.push((v, 0));
                    }
                    GRAY => {
                        // Back edge → the path from v to u is a cycle.
                        if cycles.len() < MAX_CYCLES {
                            let pos = path.iter().position(|&x| x == v).unwrap_or(0);
                            let cycle = path[pos..].to_vec();
                            if seen.insert(canonical(&cycle)) {
                                cycles.push(cycle);
                            }
                        }
                    }
                    _ => {}
                }
            } else {
                color[u] = BLACK;
                path.pop();
                stack.pop();
            }
        }
    }
    cycles
}

/// Rotate a cycle so its smallest node id leads — two rotations of the same
/// loop then compare equal.
fn canonical(cycle: &[usize]) -> Vec<usize> {
    let Some(min_at) = (0..cycle.len()).min_by_key(|&i| cycle[i]) else {
        return Vec::new();
    };
    cycle[min_at..]
        .iter()
        .chain(&cycle[..min_at])
        .copied()
        .collect()
}

/// Excel error text for a cell value, if it is an error.
///
/// Covers both shapes a workbook can store: a typed error cell (what Excel
/// writes) and a cached string that is exactly an error literal (what some
/// writers emit for a formula's stored result).
fn error_text(value: &Data) -> Option<String> {
    match value {
        Data::Error(e) => Some(e.to_string()),
        Data::String(s) => {
            let t = s.trim().to_ascii_uppercase();
            ERROR_LITERALS.contains(&t.as_str()).then_some(t)
        }
        _ => None,
    }
}

/// A1-style address for a 0-based `(row, col)`.
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

/// Sort key that orders `A2` before `A10` (and puts defined names last).
fn cell_sort_key(cell: &str) -> (u32, u32, String) {
    match classify_token(cell) {
        Some(Tok::Cell(r, c)) => (r, c, String::new()),
        _ => (u32::MAX, u32::MAX, cell.to_string()),
    }
}

/// One-line count summary, e.g. `1 circular reference, 2 error values`.
fn counts_line(findings: &[Finding]) -> String {
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for f in findings {
        *counts.entry(f.kind.label()).or_default() += 1;
    }
    counts
        .iter()
        .map(|(k, v)| format!("{v} {k}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Readable fixed-width report.
fn render_table(rep: &Report) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Formula audit — {} sheet(s) scanned ({}), {} formula cell(s) read.\n",
        rep.sheets_scanned.len(),
        if rep.sheets_scanned.is_empty() {
            "none".to_string()
        } else {
            rep.sheets_scanned.join(", ")
        },
        rep.formula_cells
    ));

    if rep.findings.is_empty() {
        out.push_str(
            "\nNo problems found: no circular references, no #REF! formulas, no error values.\n",
        );
    } else {
        out.push_str(&format!(
            "\n{} problem(s) found — {}.\n\n",
            rep.findings.len(),
            counts_line(&rep.findings)
        ));

        let rows: Vec<[String; 3]> = rep
            .findings
            .iter()
            .map(|f| {
                [
                    f.kind.label().to_string(),
                    f.address(),
                    match &f.formula {
                        Some(fx) => format!("{} — {fx}", f.detail),
                        None => f.detail.clone(),
                    },
                ]
            })
            .collect();
        let headers = ["TYPE", "CELL", "DETAIL"];
        let w0 = rows
            .iter()
            .map(|r| r[0].chars().count())
            .chain([headers[0].len()])
            .max()
            .unwrap_or(4);
        let w1 = rows
            .iter()
            .map(|r| r[1].chars().count())
            .chain([headers[1].len()])
            .max()
            .unwrap_or(4);
        out.push_str(&format!(
            "{:<w0$}  {:<w1$}  {}\n",
            headers[0],
            headers[1],
            headers[2],
            w0 = w0,
            w1 = w1
        ));
        for r in &rows {
            out.push_str(&format!(
                "{:<w0$}  {:<w1$}  {}\n",
                r[0],
                r[1],
                r[2],
                w0 = w0,
                w1 = w1
            ));
        }
    }

    if rep.findings_omitted > 0 {
        out.push_str(&format!(
            "\n{} more finding(s) not shown (raise max_findings).\n",
            rep.findings_omitted
        ));
    }
    if rep.scan_truncated {
        out.push_str(&format!(
            "\nScan stopped at {MAX_FORMULA_CELLS} formula cells; later cells were not checked.\n"
        ));
    }
    out.push_str(
        "\nChecked from the stored file, without recalculating: references built at calculation \
         time (INDIRECT, OFFSET) and structured table references contribute no dependency edges, \
         and error values are the results the writing application last cached.\n",
    );
    out
}

/// Structured report — same content, machine-readable.
fn render_json(rep: &Report) -> String {
    let findings: Vec<serde_json::Value> = rep
        .findings
        .iter()
        .map(|f| {
            serde_json::json!({
                "type": f.kind.label(),
                "sheet": f.sheet,
                "cell": f.cell,
                "address": f.address(),
                "formula": f.formula,
                "detail": f.detail,
            })
        })
        .collect();
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for f in &rep.findings {
        *counts.entry(f.kind.label()).or_default() += 1;
    }
    let value = serde_json::json!({
        "sheets_scanned": rep.sheets_scanned,
        "formula_cells": rep.formula_cells,
        "problem_count": rep.findings.len(),
        "counts": counts,
        "findings_omitted": rep.findings_omitted,
        "scan_truncated": rep.scan_truncated,
        "findings": findings,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

/// Flat one-row-per-finding CSV for triage in a spreadsheet.
fn render_csv(rep: &Report) -> String {
    let mut out = String::from("type,sheet,cell,formula,detail\r\n");
    for f in &rep.findings {
        out.push_str(&format!(
            "{},{},{},{},{}\r\n",
            csv_field(f.kind.label()),
            csv_field(&f.sheet),
            csv_field(&f.cell),
            csv_field(f.formula.as_deref().unwrap_or("")),
            csv_field(&f.detail),
        ));
    }
    out
}

/// RFC-4180-quote a CSV field iff it needs it.
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

    /// A workbook exercising every finding kind:
    ///
    /// * `Data`   — A1=10, B1 `=A1*2`, C1 `=1/0` cached `#DIV/0!`,
    ///              D1 `=SUM(#REF!)`, E1 `=Missing!A1`, F1 `='[Book2.xlsx]S'!A1`
    /// * `Calc`   — A1 `=Calc!B1+1`, B1 `=A2`, A2 `=A1` (a 3-cell cycle),
    ///              D1 `=D1+1` (self reference), E1 `=SUM(Data!A1:B1)` (clean)
    fn sample() -> Vec<u8> {
        let mut wb = Workbook::new();

        let data = wb.add_worksheet().set_name("Data").unwrap();
        data.write_number(0, 0, 10.0).unwrap();
        data.write_formula(0, 1, Formula::new("=A1*2")).unwrap();
        data.write_formula_with_format(
            0,
            2,
            Formula::new("=1/0").set_result("#DIV/0!"),
            &rust_xlsxwriter::Format::new(),
        )
        .unwrap();
        data.write_formula(0, 3, Formula::new("=SUM(#REF!)"))
            .unwrap();
        data.write_formula(0, 4, Formula::new("=Missing!A1"))
            .unwrap();
        data.write_formula(0, 5, Formula::new("='[Book2.xlsx]S'!A1"))
            .unwrap();

        let calc = wb.add_worksheet().set_name("Calc").unwrap();
        calc.write_formula(0, 0, Formula::new("=Calc!B1+1"))
            .unwrap();
        calc.write_formula(0, 1, Formula::new("=A2")).unwrap();
        calc.write_formula(1, 0, Formula::new("=A1")).unwrap();
        calc.write_formula(0, 3, Formula::new("=D1+1")).unwrap();
        calc.write_formula(0, 4, Formula::new("=SUM(Data!A1:B1)"))
            .unwrap();

        wb.save_to_buffer().unwrap()
    }

    /// A workbook with no problems at all.
    fn clean() -> Vec<u8> {
        let mut wb = Workbook::new();
        let s = wb.add_worksheet().set_name("Sheet1").unwrap();
        s.write_number(0, 0, 2.0).unwrap();
        s.write_number(1, 0, 3.0).unwrap();
        s.write_formula(2, 0, Formula::new("=SUM(A1:A2)")).unwrap();
        wb.save_to_buffer().unwrap()
    }

    fn findings_of(bytes: &[u8], opts: &Options) -> Vec<Finding> {
        analyze(bytes, opts).unwrap().findings
    }

    #[test]
    fn detects_ref_error_formula() {
        let f = findings_of(&sample(), &Options::default());
        let hit = f
            .iter()
            .find(|x| x.kind == Kind::RefError && x.address() == "Data!D1")
            .unwrap_or_else(|| panic!("no #REF! finding for Data!D1 in {f:#?}"));
        assert_eq!(hit.formula.as_deref(), Some("=SUM(#REF!)"));
        assert!(hit.detail.contains("#REF!"), "got: {}", hit.detail);
    }

    #[test]
    fn detects_circular_chain_and_self_reference() {
        let f = findings_of(&sample(), &Options::default());
        let cycles: Vec<&Finding> = f.iter().filter(|x| x.kind == Kind::Circular).collect();
        assert_eq!(cycles.len(), 2, "expected 2 cycles, got {cycles:#?}");

        let selfref = cycles
            .iter()
            .find(|c| c.address() == "Calc!D1")
            .expect("self-reference on Calc!D1");
        assert!(
            selfref.detail.contains("its own cell"),
            "got: {}",
            selfref.detail
        );

        let chain = cycles
            .iter()
            .find(|c| c.detail.contains("circular chain"))
            .expect("multi-cell chain");
        // A1 → B1 → A2 → A1, reported from whichever cell DFS entered on.
        for cell in ["Calc!A1", "Calc!B1", "Calc!A2"] {
            assert!(
                chain.detail.contains(cell),
                "missing {cell}: {}",
                chain.detail
            );
        }
    }

    #[test]
    fn detects_broken_sheet_reference() {
        let f = findings_of(&sample(), &Options::default());
        let hit = f
            .iter()
            .find(|x| x.kind == Kind::BrokenReference)
            .unwrap_or_else(|| panic!("no broken-reference finding in {f:#?}"));
        assert_eq!(hit.address(), "Data!E1");
        assert!(hit.detail.contains("Missing"), "got: {}", hit.detail);
    }

    #[test]
    fn detects_external_workbook_link() {
        let f = findings_of(&sample(), &Options::default());
        let hit = f
            .iter()
            .find(|x| x.kind == Kind::ExternalLink)
            .unwrap_or_else(|| panic!("no external-link finding in {f:#?}"));
        assert_eq!(hit.address(), "Data!F1");
        assert!(hit.detail.contains("Book2.xlsx"), "got: {}", hit.detail);
    }

    #[test]
    fn detects_cached_error_value() {
        let f = findings_of(&sample(), &Options::default());
        let hit = f
            .iter()
            .find(|x| x.kind == Kind::ErrorValue)
            .unwrap_or_else(|| panic!("no error-value finding in {f:#?}"));
        assert_eq!(hit.address(), "Data!C1");
        assert!(hit.detail.contains("#DIV/0!"), "got: {}", hit.detail);
    }

    #[test]
    fn error_values_can_be_switched_off() {
        let opts = Options {
            check_error_values: false,
            ..Default::default()
        };
        let f = findings_of(&sample(), &opts);
        assert!(f.iter().all(|x| x.kind != Kind::ErrorValue), "got {f:#?}");
        // The other checks still run.
        assert!(f.iter().any(|x| x.kind == Kind::RefError));
    }

    #[test]
    fn cycles_can_be_switched_off() {
        let opts = Options {
            check_cycles: false,
            ..Default::default()
        };
        let f = findings_of(&sample(), &opts);
        assert!(f.iter().all(|x| x.kind != Kind::Circular), "got {f:#?}");
    }

    #[test]
    fn sheet_selector_scopes_the_report() {
        let opts = Options {
            sheet: Some("Data".to_string()),
            ..Default::default()
        };
        let rep = analyze(&sample(), &opts).unwrap();
        assert_eq!(rep.sheets_scanned, vec!["Data".to_string()]);
        assert!(
            rep.findings.iter().all(|f| f.sheet == "Data"),
            "got {:#?}",
            rep.findings
        );
        // Selecting by 0-based index picks the same sheet.
        let by_index = analyze(
            &sample(),
            &Options {
                sheet: Some("0".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_index.sheets_scanned, vec!["Data".to_string()]);
    }

    #[test]
    fn clean_workbook_reports_nothing() {
        let rep = analyze(&clean(), &Options::default()).unwrap();
        assert!(rep.findings.is_empty(), "got {:#?}", rep.findings);
        assert_eq!(rep.formula_cells, 1);
        let table = audit(&clean(), &Options::default(), "table").unwrap();
        assert!(table.contains("No problems found"), "got: {table}");
    }

    #[test]
    fn max_findings_clips_and_counts_the_remainder() {
        let opts = Options {
            max_findings: 2,
            ..Default::default()
        };
        let rep = analyze(&sample(), &opts).unwrap();
        assert_eq!(rep.findings.len(), 2);
        assert!(rep.findings_omitted >= 1);
        let table = audit(&sample(), &opts, "table").unwrap();
        assert!(table.contains("more finding(s) not shown"), "got: {table}");
    }

    #[test]
    fn table_format_lists_every_finding() {
        let table = audit(&sample(), &Options::default(), "table").unwrap();
        assert!(table.contains("problem(s) found"), "got: {table}");
        assert!(table.contains("circular_reference"), "got: {table}");
        assert!(table.contains("Data!D1"), "got: {table}");
    }

    #[test]
    fn json_format_is_structured() {
        let out = audit(&sample(), &Options::default(), "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["sheets_scanned"], serde_json::json!(["Data", "Calc"]));
        assert!(v["problem_count"].as_u64().unwrap() >= 5);
        let kinds: Vec<String> = v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["type"].as_str().unwrap().to_string())
            .collect();
        assert!(kinds.contains(&"ref_error".to_string()), "got {kinds:?}");
        assert!(
            kinds.contains(&"circular_reference".to_string()),
            "got {kinds:?}"
        );
    }

    #[test]
    fn csv_format_is_flat() {
        let out = audit(&sample(), &Options::default(), "csv").unwrap();
        let mut lines = out.lines();
        assert_eq!(lines.next().unwrap(), "type,sheet,cell,formula,detail");
        assert!(out.contains("ref_error,Data,D1,"), "got: {out}");
    }

    #[test]
    fn empty_bytes_error() {
        let err = analyze(&[], &Options::default()).unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn garbage_bytes_error() {
        let err = analyze(b"not a spreadsheet at all", &Options::default()).unwrap_err();
        assert!(err.contains("not a readable spreadsheet"), "got: {err}");
    }

    #[test]
    fn unknown_sheet_error() {
        let opts = Options {
            sheet: Some("Nope".to_string()),
            ..Default::default()
        };
        let err = analyze(&sample(), &opts).unwrap_err();
        assert!(err.contains("no sheet named"), "got: {err}");
    }

    #[test]
    fn sheet_index_out_of_range_error() {
        let opts = Options {
            sheet: Some("9".to_string()),
            ..Default::default()
        };
        let err = analyze(&sample(), &opts).unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
    }

    #[test]
    fn unknown_format_error() {
        let err = audit(&clean(), &Options::default(), "yaml").unwrap_err();
        assert!(err.contains("unknown format"), "got: {err}");
    }

    // ---- reference parser ------------------------------------------------

    fn refs(formula: &str) -> Vec<RefRect> {
        parse_formula(formula).refs
    }

    fn cell(sheet: Option<&str>, r: u32, c: u32) -> RefRect {
        RefRect {
            sheet: sheet.map(str::to_string),
            r1: r,
            c1: c,
            r2: r,
            c2: c,
        }
    }

    #[test]
    fn parses_plain_cells_and_ranges() {
        assert_eq!(refs("=A1+B2"), vec![cell(None, 0, 0), cell(None, 1, 1)]);
        assert_eq!(
            refs("=SUM(A1:B3)"),
            vec![RefRect {
                sheet: None,
                r1: 0,
                c1: 0,
                r2: 2,
                c2: 1
            }]
        );
        // Absolute markers are just anchors, not part of the address.
        assert_eq!(refs("=$A$1"), vec![cell(None, 0, 0)]);
    }

    #[test]
    fn parses_sheet_qualified_references() {
        assert_eq!(refs("=Sheet2!C4"), vec![cell(Some("Sheet2"), 3, 2)]);
        assert_eq!(refs("='My Sheet'!C4"), vec![cell(Some("My Sheet"), 3, 2)]);
    }

    #[test]
    fn parses_whole_column_and_row_ranges() {
        assert_eq!(
            refs("=SUM(A:B)"),
            vec![RefRect {
                sheet: None,
                r1: 0,
                c1: 0,
                r2: MAX_ROW,
                c2: 1
            }]
        );
        assert_eq!(
            refs("=SUM(2:3)"),
            vec![RefRect {
                sheet: None,
                r1: 1,
                c1: 0,
                r2: 2,
                c2: MAX_COL
            }]
        );
    }

    #[test]
    fn does_not_mistake_functions_names_or_numbers_for_cells() {
        // A function name that reads like a cell address.
        assert!(refs("=LOG10(A1)").contains(&cell(None, 0, 0)));
        assert_eq!(refs("=LOG10(A1)").len(), 1);
        // Scientific notation is a number, not cell E5.
        assert_eq!(refs("=A1*1E5"), vec![cell(None, 0, 0)]);
        // Text inside a string literal is never a reference.
        assert_eq!(refs("=\"see A1\"&B2"), vec![cell(None, 1, 1)]);
        // Past the last column/row there is no such cell.
        assert!(refs("=ZZZZ1").is_empty());
        assert!(refs("=A1048577").is_empty());
        // A bare column on its own is not a reference.
        assert!(refs("=TRUE").is_empty());
    }

    #[test]
    fn flags_ref_error_outside_strings_only() {
        assert!(parse_formula("=SUM(#REF!)").has_ref_error);
        assert!(parse_formula("=#REF!Sheet1!A1").has_ref_error);
        assert!(!parse_formula("=\"#REF! is bad\"").has_ref_error);
        assert!(!parse_formula("=A1").has_ref_error);
    }

    #[test]
    fn external_links_do_not_become_broken_sheet_references() {
        let p = parse_formula("=[1]Sheet9!A1+'[Book2.xlsx]Data'!B2");
        assert_eq!(
            p.external_books,
            vec!["1".to_string(), "Book2.xlsx".to_string()]
        );
        // Neither reference is checked against THIS workbook's sheets.
        assert!(p.refs.is_empty(), "got {:#?}", p.refs);
    }

    #[test]
    fn column_letters_and_a1_round_trip() {
        assert_eq!(a1(0, 0), "A1");
        assert_eq!(a1(9, 26), "AA10");
        assert_eq!(col_letters(MAX_COL), "XFD");
        assert_eq!(col_index("XFD"), Some(MAX_COL));
        assert_eq!(col_index("XFE"), None);
    }

    #[test]
    fn cycle_detection_is_bounded_on_a_long_chain() {
        // A1 → A2 → … → A2000 → A1: one cycle, no stack overflow.
        let n = 2000u32;
        let cells: Vec<FormulaCell> = (0..n)
            .map(|r| FormulaCell {
                sheet: 0,
                row: r,
                col: 0,
                text: format!("=A{}", if r + 1 == n { 1 } else { r + 2 }),
            })
            .collect();
        let parsed: Vec<Parsed> = cells.iter().map(|c| parse_formula(&c.text)).collect();
        let graph = build_graph(&cells, &parsed, &["S".to_string()]);
        let cycles = find_cycles(&graph);
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].len(), n as usize);
    }

    #[test]
    fn error_text_reads_both_typed_and_cached_string_errors() {
        assert_eq!(
            error_text(&Data::Error(calamine::CellErrorType::Ref)).as_deref(),
            Some("#REF!")
        );
        assert_eq!(
            error_text(&Data::String("#div/0!".to_string())).as_deref(),
            Some("#DIV/0!")
        );
        assert_eq!(error_text(&Data::String("ok".to_string())), None);
        assert_eq!(error_text(&Data::Float(1.0)), None);
    }
}
