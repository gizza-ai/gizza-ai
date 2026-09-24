//! matrix-text-parser core — normalize a matrix pasted in any common syntax into a
//! rectangular 2D array plus its shape, then re-emit it in the requested format.
//!
//! Accepted input syntaxes:
//!   * delimited rows — one row per line, cells split on space/comma/tab/semicolon/pipe
//!   * Python / NumPy nested lists — `[[1, 2], [3, 4]]`, including `np.array(...)` wrappers
//!   * MATLAB / Octave — `[1 2; 3 4]`, with or without the outer brackets
//!   * LaTeX — `\begin{bmatrix} 1 & 2 \\ 3 & 4 \end{bmatrix}`
//!
//! Pure and deterministic: text in, text out, no I/O and no clock.

use serde_json::{Map, Number, Value};

/// Maximum accepted input size in bytes.
pub const MAX_BYTES: usize = 2_000_000;
/// Maximum accepted number of matrix rows.
pub const MAX_ROWS: usize = 10_000;
/// Maximum accepted number of matrix columns.
pub const MAX_COLUMNS: usize = 2_000;
/// Maximum accepted number of matrix cells (rows x columns).
pub const MAX_CELLS: usize = 1_000_000;

/// Prefixes that introduce a whole-line comment in pasted matrix dumps.
const COMMENT_PREFIXES: [&str; 3] = ["#", "//", "%"];

/// Call wrappers stripped before syntax detection, e.g. `np.array([[1,2]])`.
const CALL_WRAPPERS: [&str; 9] = [
    "np.array",
    "numpy.array",
    "np.matrix",
    "numpy.matrix",
    "jnp.array",
    "torch.tensor",
    "tf.constant",
    "array",
    "matrix",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Syntax {
    Delimited,
    Python,
    Matlab,
    Latex,
}

impl Syntax {
    fn name(self) -> &'static str {
        match self {
            Syntax::Delimited => "delimited",
            Syntax::Python => "python",
            Syntax::Matlab => "matlab",
            Syntax::Latex => "latex",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Delim {
    Auto,
    Comma,
    Space,
    Tab,
    Semicolon,
    Pipe,
}

impl Delim {
    fn name(self) -> &'static str {
        match self {
            Delim::Auto => "auto",
            Delim::Comma => "comma",
            Delim::Space => "space",
            Delim::Tab => "tab",
            Delim::Semicolon => "semicolon",
            Delim::Pipe => "pipe",
        }
    }

    fn char(self) -> Option<char> {
        match self {
            Delim::Comma => Some(','),
            Delim::Tab => Some('\t'),
            Delim::Semicolon => Some(';'),
            Delim::Pipe => Some('|'),
            Delim::Space | Delim::Auto => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cells {
    Auto,
    Number,
    Text,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ragged {
    Error,
    Pad,
    Trim,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Output {
    Json,
    Array,
    Csv,
    Tsv,
    Matlab,
    Numpy,
    Latex,
    Aligned,
}

/// Parse a pasted matrix and render it in the requested output format.
///
/// * `matrix` — the pasted matrix text.
/// * `input_format` — `auto`, `delimited`, `python`, `matlab` or `latex`.
/// * `delimiter` — `auto`, `comma`, `space`, `tab`, `semicolon` or `pipe` (delimited rows only).
/// * `output` — `json`, `array`, `csv`, `tsv`, `matlab`, `numpy`, `latex` or `aligned`.
/// * `cells` — `auto` (numbers where possible, text otherwise), `number` (strict), `text`.
/// * `fractions` — evaluate `3/4`-style cells into decimal numbers.
/// * `header` — treat the first row as column labels rather than data.
/// * `ragged` — `error`, `pad` or `trim` when rows have different lengths.
/// * `fill` — the cell written into short rows when `ragged = pad`.
/// * `indent` — JSON indentation in spaces, 0 for minified.
#[allow(clippy::too_many_arguments)]
pub fn parse_matrix(
    matrix: &str,
    input_format: &str,
    delimiter: &str,
    output: &str,
    cells: &str,
    fractions: bool,
    header: bool,
    ragged: &str,
    fill: &str,
    indent: f64,
) -> Result<String, String> {
    if matrix.len() > MAX_BYTES {
        return Err(format!(
            "matrix is {} bytes, over the {MAX_BYTES}-byte limit",
            matrix.len()
        ));
    }
    if !(0.0..=8.0).contains(&indent) || indent.fract() != 0.0 {
        return Err(format!(
            "indent must be a whole number of spaces between 0 and 8 (got {indent})"
        ));
    }
    let indent = indent as usize;

    let requested = match trimmed_lower(input_format).as_str() {
        "" | "auto" => None,
        "delimited" => Some(Syntax::Delimited),
        "python" | "numpy" | "json" => Some(Syntax::Python),
        "matlab" | "octave" => Some(Syntax::Matlab),
        "latex" | "tex" => Some(Syntax::Latex),
        other => {
            return Err(format!(
                "unknown input_format '{other}'; expected auto, delimited, python, matlab or latex"
            ))
        }
    };
    let delim = match trimmed_lower(delimiter).as_str() {
        "" | "auto" => Delim::Auto,
        "comma" | "," => Delim::Comma,
        "space" | "whitespace" | " " => Delim::Space,
        "tab" | "\t" | "\\t" => Delim::Tab,
        "semicolon" | ";" => Delim::Semicolon,
        "pipe" | "|" => Delim::Pipe,
        other => {
            return Err(format!(
                "unknown delimiter '{other}'; expected auto, comma, space, tab, semicolon or pipe"
            ))
        }
    };
    let output = match trimmed_lower(output).as_str() {
        "" | "json" => Output::Json,
        "array" => Output::Array,
        "csv" => Output::Csv,
        "tsv" => Output::Tsv,
        "matlab" => Output::Matlab,
        "numpy" | "python" => Output::Numpy,
        "latex" => Output::Latex,
        "aligned" => Output::Aligned,
        other => {
            return Err(format!(
                "unknown output '{other}'; expected json, array, csv, tsv, matlab, numpy, latex or aligned"
            ))
        }
    };
    let cells = match trimmed_lower(cells).as_str() {
        "" | "auto" => Cells::Auto,
        "number" | "numeric" => Cells::Number,
        "text" | "string" => Cells::Text,
        other => {
            return Err(format!(
                "unknown cells '{other}'; expected auto, number or text"
            ))
        }
    };
    let ragged = match trimmed_lower(ragged).as_str() {
        "" | "error" => Ragged::Error,
        "pad" => Ragged::Pad,
        "trim" => Ragged::Trim,
        other => {
            return Err(format!(
                "unknown ragged '{other}'; expected error, pad or trim"
            ))
        }
    };

    let cleaned = strip_comments(matrix);
    if cleaned.trim().is_empty() {
        return Err("matrix is empty; paste rows such as '1 2\\n3 4', '[[1,2],[3,4]]' or '[1 2; 3 4]'".into());
    }

    let body = strip_call_wrapper(cleaned.trim());
    let syntax = requested.unwrap_or_else(|| detect_syntax(body));

    let (mut grid, used_delim) = match syntax {
        Syntax::Delimited => parse_delimited(body, delim)?,
        Syntax::Python => (parse_nested(body)?, Delim::Comma),
        Syntax::Matlab => (parse_matlab(body)?, Delim::Space),
        Syntax::Latex => (parse_latex(body)?, Delim::Space),
    };

    if grid.is_empty() {
        return Err("no matrix rows were found in the input".into());
    }
    if grid.len() > MAX_ROWS {
        return Err(format!(
            "matrix has {} rows, over the {MAX_ROWS}-row limit",
            grid.len()
        ));
    }

    // Rectangularize.
    let widths: Vec<usize> = grid.iter().map(|r| r.len()).collect();
    let min_w = *widths.iter().min().unwrap_or(&0);
    let max_w = *widths.iter().max().unwrap_or(&0);
    if max_w == 0 {
        return Err("every row is empty; nothing to parse".into());
    }
    let mut ragged_fixed = 0usize;
    if min_w != max_w {
        match ragged {
            Ragged::Error => {
                let (bad, len) = widths
                    .iter()
                    .enumerate()
                    .find(|(_, w)| **w != widths[0])
                    .map(|(i, w)| (i + 1, *w))
                    .unwrap_or((1, widths[0]));
                return Err(format!(
                    "ragged matrix: row 1 has {} cells but row {bad} has {len}; set ragged to pad or trim to normalize it",
                    widths[0]
                ));
            }
            Ragged::Pad => {
                for row in grid.iter_mut() {
                    while row.len() < max_w {
                        row.push(fill.to_string());
                        ragged_fixed += 1;
                    }
                }
            }
            Ragged::Trim => {
                for row in grid.iter_mut() {
                    ragged_fixed += row.len() - min_w;
                    row.truncate(min_w);
                }
            }
        }
    }
    let width = grid[0].len();
    if width > MAX_COLUMNS {
        return Err(format!(
            "matrix has {width} columns, over the {MAX_COLUMNS}-column limit"
        ));
    }
    if grid.len().saturating_mul(width) > MAX_CELLS {
        return Err(format!(
            "matrix has {} cells, over the {MAX_CELLS}-cell limit",
            grid.len() * width
        ));
    }

    // Split off the header row if asked.
    let header_row: Option<Vec<String>> = if header {
        if grid.len() < 2 {
            return Err(
                "header is on but the matrix has only one row; there would be no data left".into(),
            );
        }
        Some(grid.remove(0))
    } else {
        None
    };

    // Type the cells.
    let mut typed: Vec<Vec<Value>> = Vec::with_capacity(grid.len());
    let mut all_numeric = true;
    for (r, row) in grid.iter().enumerate() {
        let mut out_row = Vec::with_capacity(row.len());
        for (c, raw) in row.iter().enumerate() {
            let value = cell_value(raw, cells, fractions).ok_or_else(|| {
                format!(
                    "cell '{}' at row {}, column {} is not a number; switch cells to auto or text to keep it as written",
                    raw,
                    r + 1 + usize::from(header),
                    c + 1
                )
            })?;
            if !value.is_number() {
                all_numeric = false;
            }
            out_row.push(value);
        }
        typed.push(out_row);
    }

    let rows = typed.len();
    let columns = width;
    let report = Report {
        rows,
        columns,
        square: rows == columns,
        numeric: all_numeric,
        syntax,
        delimiter: used_delim,
        header: header_row,
        ragged_fixed,
        matrix: typed,
    };
    render(&report, output, indent)
}

struct Report {
    rows: usize,
    columns: usize,
    square: bool,
    numeric: bool,
    syntax: Syntax,
    delimiter: Delim,
    header: Option<Vec<String>>,
    ragged_fixed: usize,
    matrix: Vec<Vec<Value>>,
}

fn render(r: &Report, output: Output, indent: usize) -> Result<String, String> {
    let grid: Vec<Value> = r
        .matrix
        .iter()
        .map(|row| Value::Array(row.clone()))
        .collect();
    match output {
        Output::Json => {
            let mut map = Map::new();
            map.insert(
                "shape".into(),
                Value::Array(vec![num(r.rows as f64), num(r.columns as f64)]),
            );
            map.insert("rows".into(), num(r.rows as f64));
            map.insert("columns".into(), num(r.columns as f64));
            map.insert("square".into(), Value::Bool(r.square));
            map.insert("numeric".into(), Value::Bool(r.numeric));
            map.insert("input_format".into(), Value::String(r.syntax.name().into()));
            map.insert(
                "delimiter".into(),
                Value::String(r.delimiter.name().into()),
            );
            if let Some(h) = &r.header {
                map.insert(
                    "header".into(),
                    Value::Array(h.iter().map(|s| Value::String(s.clone())).collect()),
                );
            }
            if r.ragged_fixed > 0 {
                map.insert("ragged_fixed".into(), num(r.ragged_fixed as f64));
            }
            map.insert("matrix".into(), Value::Array(grid));
            to_json(&Value::Object(map), indent)
        }
        Output::Array => to_json(&Value::Array(grid), indent),
        Output::Csv => Ok(delimited_text(r, ',')),
        Output::Tsv => Ok(delimited_text(r, '\t')),
        Output::Matlab => {
            let body = r
                .matrix
                .iter()
                .map(|row| {
                    row.iter()
                        .map(scalar_text)
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect::<Vec<_>>()
                .join("; ");
            Ok(with_header_comment(r, "%", format!("[{body}]")))
        }
        Output::Numpy => {
            let body = r
                .matrix
                .iter()
                .map(|row| {
                    format!(
                        "[{}]",
                        row.iter().map(scalar_text).collect::<Vec<_>>().join(", ")
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            Ok(with_header_comment(
                r,
                "#",
                format!("np.array([{body}])"),
            ))
        }
        Output::Latex => {
            let body = r
                .matrix
                .iter()
                .map(|row| row.iter().map(scalar_text).collect::<Vec<_>>().join(" & "))
                .collect::<Vec<_>>()
                .join(" \\\\\n");
            Ok(with_header_comment(
                r,
                "%",
                format!("\\begin{{bmatrix}}\n{body}\n\\end{{bmatrix}}"),
            ))
        }
        Output::Aligned => {
            let mut lines: Vec<Vec<String>> = Vec::new();
            if let Some(h) = &r.header {
                lines.push(h.clone());
            }
            for row in &r.matrix {
                lines.push(row.iter().map(scalar_text).collect());
            }
            let mut widths = vec![0usize; r.columns];
            for line in &lines {
                for (i, cell) in line.iter().enumerate() {
                    if i < widths.len() {
                        widths[i] = widths[i].max(cell.chars().count());
                    }
                }
            }
            Ok(lines
                .iter()
                .map(|line| {
                    line.iter()
                        .enumerate()
                        .map(|(i, cell)| {
                            let pad = widths.get(i).copied().unwrap_or(0);
                            format!("{cell:>pad$}")
                        })
                        .collect::<Vec<_>>()
                        .join("  ")
                        .trim_end()
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
    }
}

fn with_header_comment(r: &Report, marker: &str, body: String) -> String {
    match &r.header {
        Some(h) => format!("{marker} {}\n{body}", h.join(", ")),
        None => body,
    }
}

fn delimited_text(r: &Report, sep: char) -> String {
    let mut lines: Vec<String> = Vec::with_capacity(r.matrix.len() + 1);
    if let Some(h) = &r.header {
        lines.push(
            h.iter()
                .map(|c| quote_cell(c, sep))
                .collect::<Vec<_>>()
                .join(&sep.to_string()),
        );
    }
    for row in &r.matrix {
        lines.push(
            row.iter()
                .map(|v| quote_cell(&scalar_text(v), sep))
                .collect::<Vec<_>>()
                .join(&sep.to_string()),
        );
    }
    lines.join("\n")
}

fn quote_cell(cell: &str, sep: char) -> String {
    if cell.contains(sep) || cell.contains('"') || cell.contains('\n') {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_string()
    }
}

fn scalar_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn num(v: f64) -> Value {
    if v.fract() == 0.0 && v.abs() < 9.0e15 {
        Value::Number(Number::from(v as i64))
    } else {
        Number::from_f64(v).map(Value::Number).unwrap_or(Value::Null)
    }
}

fn to_json(value: &Value, indent: usize) -> Result<String, String> {
    if indent == 0 {
        return serde_json::to_string(value).map_err(|e| e.to_string());
    }
    let pad = " ".repeat(indent);
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(pad.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    serde::Serialize::serialize(value, &mut ser).map_err(|e| e.to_string())?;
    String::from_utf8(buf).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// input cleaning + syntax detection
// ---------------------------------------------------------------------------

fn trimmed_lower(s: &str) -> String {
    s.trim().to_ascii_lowercase()
}

/// Drop whole-line comments (`#`, `//`, `%`) and keep everything else verbatim.
fn strip_comments(s: &str) -> String {
    s.lines()
        .filter(|line| {
            let t = line.trim_start();
            !COMMENT_PREFIXES.iter().any(|p| t.starts_with(p))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Strip a `np.array( ... )`-style call wrapper down to its bracketed argument.
fn strip_call_wrapper(s: &str) -> &str {
    let lower = s.to_ascii_lowercase();
    for name in CALL_WRAPPERS {
        if let Some(rest) = lower.strip_prefix(name) {
            if rest.trim_start().starts_with('(') {
                let open = s.len() - rest.len() + (rest.len() - rest.trim_start().len());
                if let Some(close) = matching_bracket(s, open) {
                    let inner = &s[open + 1..close];
                    // Peel off trailing keyword args such as `, dtype=float`.
                    if let Some(start) = inner.find(['[', '{']) {
                        if let Some(end) = matching_bracket(inner, start) {
                            return inner[start..=end].trim();
                        }
                    }
                    return inner.trim();
                }
            }
        }
    }
    s
}

/// Index of the bracket matching the opener at `open`, respecting quotes.
fn matching_bracket(s: &str, open: usize) -> Option<usize> {
    let bytes: Vec<(usize, char)> = s.char_indices().collect();
    let start = bytes.iter().position(|(i, _)| *i == open)?;
    let opener = bytes[start].1;
    let closer = match opener {
        '[' => ']',
        '{' => '}',
        '(' => ')',
        _ => return None,
    };
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    for &(i, ch) in &bytes[start..] {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                }
            }
            None => match ch {
                '\'' | '"' => quote = Some(ch),
                c if c == opener => depth += 1,
                c if c == closer => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            },
        }
    }
    None
}

fn detect_syntax(body: &str) -> Syntax {
    let t = body.trim();
    if t.contains("\\begin{") || (t.contains("\\\\") && t.contains('&')) {
        return Syntax::Latex;
    }
    if let Some(open) = t.find(['[', '{']) {
        if let Some(close) = matching_bracket(t, open) {
            let inner = &t[open + 1..close];
            if inner.contains('[') || inner.contains('{') {
                return Syntax::Python;
            }
            if inner.contains(';') {
                return Syntax::Matlab;
            }
            // A single bracketed row: MATLAB's parser handles both `[1 2 3]` and `[1, 2, 3]`.
            if close + 1 >= t.len() {
                return Syntax::Matlab;
            }
        }
    }
    // Bracket-less MATLAB: one line with semicolon row separators.
    if !t.contains('\n') && t.contains(';') {
        return Syntax::Matlab;
    }
    Syntax::Delimited
}

// ---------------------------------------------------------------------------
// per-syntax parsers
// ---------------------------------------------------------------------------

fn parse_delimited(body: &str, delim: Delim) -> Result<(Vec<Vec<String>>, Delim), String> {
    let lines: Vec<&str> = body
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return Err("no non-blank rows were found in the input".into());
    }
    let cleaned: Vec<String> = lines.iter().map(|l| strip_row_decoration(l)).collect();
    let used = if delim == Delim::Auto {
        detect_delimiter(&cleaned)
    } else {
        delim
    };
    let mut grid = Vec::with_capacity(cleaned.len());
    for line in &cleaned {
        let row: Vec<String> = match used.char() {
            Some(c) => line.split(c).map(|c| c.trim().to_string()).collect(),
            None => line.split_whitespace().map(|c| c.to_string()).collect(),
        };
        if row.is_empty() {
            continue;
        }
        grid.push(row.into_iter().map(|c| unquote(&c)).collect());
    }
    Ok((grid, used))
}

/// Remove per-row brackets and trailing separators left by copy-pasted code.
fn strip_row_decoration(line: &str) -> String {
    let mut t = line.trim().to_string();
    while t.ends_with(',') || t.ends_with(';') {
        t.pop();
        t = t.trim_end().to_string();
    }
    let pairs = [('[', ']'), ('(', ')'), ('{', '}')];
    loop {
        let first = t.chars().next();
        let last = t.chars().last();
        match (first, last) {
            (Some(f), Some(l))
                if t.chars().count() >= 2 && pairs.iter().any(|(a, b)| *a == f && *b == l) =>
            {
                t = t[f.len_utf8()..t.len() - l.len_utf8()].trim().to_string();
            }
            _ => break,
        }
    }
    t
}

fn detect_delimiter(lines: &[String]) -> Delim {
    for (candidate, ch) in [
        (Delim::Tab, '\t'),
        (Delim::Comma, ','),
        (Delim::Semicolon, ';'),
        (Delim::Pipe, '|'),
    ] {
        if lines.iter().any(|l| l.contains(ch)) {
            return candidate;
        }
    }
    Delim::Space
}

fn parse_matlab(body: &str) -> Result<Vec<Vec<String>>, String> {
    let t = body.trim();
    let inner = match t.find(['[', '{']) {
        Some(open) => match matching_bracket(t, open) {
            Some(close) => &t[open + 1..close],
            None => {
                return Err(format!(
                    "unbalanced brackets: '{}' is never closed",
                    &t[open..(open + 1)]
                ))
            }
        },
        None => t,
    };
    let mut grid = Vec::new();
    for raw_row in inner.split(|c| c == ';' || c == '\n') {
        let row: Vec<String> = raw_row
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|c| !c.is_empty())
            .map(unquote)
            .collect();
        if !row.is_empty() {
            grid.push(row);
        }
    }
    if grid.is_empty() {
        return Err("no matrix rows were found between the brackets".into());
    }
    Ok(grid)
}

fn parse_nested(body: &str) -> Result<Vec<Vec<String>>, String> {
    let t = body.trim();
    let open = t
        .find(['[', '{'])
        .ok_or_else(|| format!("expected a nested list such as [[1,2],[3,4]], got '{}'", clip(t)))?;
    let close = matching_bracket(t, open)
        .ok_or_else(|| format!("unbalanced brackets: '{}' is never closed", &t[open..open + 1]))?;
    let inner = &t[open + 1..close];
    let segments = split_top_level(inner, &[',']);
    let mut grid = Vec::new();
    let mut flat: Vec<String> = Vec::new();
    for (i, seg) in segments.iter().enumerate() {
        let s = seg.trim();
        if s.is_empty() {
            continue;
        }
        if s.starts_with('[') || s.starts_with('{') || s.starts_with('(') {
            if !flat.is_empty() {
                return Err(format!(
                    "row {} mixes bare values with nested rows; use either [[1,2],[3,4]] or [1,2]",
                    i + 1
                ));
            }
            let rclose = matching_bracket(s, 0).ok_or_else(|| {
                format!("unbalanced brackets in row {}: '{}'", i + 1, clip(s))
            })?;
            let row_inner = &s[1..rclose];
            let mut cells = split_top_level(row_inner, &[',']);
            if cells.len() == 1 && cells[0].trim().contains(char::is_whitespace) {
                cells = cells[0]
                    .split_whitespace()
                    .map(|c| c.to_string())
                    .collect();
            }
            let row: Vec<String> = cells
                .iter()
                .map(|c| unquote(c))
                .filter(|c| !c.is_empty())
                .collect();
            if !row.is_empty() {
                grid.push(row);
            }
        } else {
            if !grid.is_empty() {
                return Err(format!(
                    "row {} mixes bare values with nested rows; use either [[1,2],[3,4]] or [1,2]",
                    i + 1
                ));
            }
            flat.push(unquote(s));
        }
    }
    if !flat.is_empty() {
        grid.push(flat);
    }
    if grid.is_empty() {
        return Err("no matrix rows were found between the brackets".into());
    }
    Ok(grid)
}

fn parse_latex(body: &str) -> Result<Vec<Vec<String>>, String> {
    let mut t = body.replace("$$", " ").replace('$', " ");
    for token in ["\\left", "\\right", "\\displaystyle", "\\,", "\\;", "\\!"] {
        t = t.replace(token, " ");
    }
    t = strip_latex_env(&t, "\\begin{");
    t = strip_latex_env(&t, "\\end{");
    let mut grid = Vec::new();
    for raw_row in t.split("\\\\") {
        let row: Vec<String> = raw_row
            .split('&')
            .map(latex_cell)
            .filter(|c| !c.is_empty())
            .collect();
        if !row.is_empty() {
            grid.push(row);
        }
    }
    if grid.is_empty() {
        return Err(
            "no LaTeX matrix rows were found; expected cells separated by & and rows by \\\\".into(),
        );
    }
    Ok(grid)
}

/// Remove every `\begin{env}` / `\end{env}` token, plus an `array` column spec.
fn strip_latex_env(s: &str, marker: &str) -> String {
    let mut out = s.to_string();
    while let Some(start) = out.find(marker) {
        let after = &out[start + marker.len()..];
        let Some(rel) = after.find('}') else { break };
        let env = &after[..rel];
        let mut end = start + marker.len() + rel + 1;
        if env.trim_end_matches('*') == "array" {
            let rest = &out[end..];
            if rest.trim_start().starts_with('{') {
                let offset = rest.len() - rest.trim_start().len();
                if let Some(close) = matching_bracket(rest, offset) {
                    end += close + 1;
                }
            }
        }
        out.replace_range(start..end, " ");
    }
    out
}

fn latex_cell(cell: &str) -> String {
    let t = cell.trim();
    // \frac{a}{b} → a/b so the fraction parser can take it from here.
    if let Some(rest) = t.strip_prefix("\\frac") {
        let rest = rest.trim_start();
        if let Some(n_end) = matching_bracket(rest, 0) {
            let numer = &rest[1..n_end];
            let after = &rest[n_end + 1..];
            let offset = after.len() - after.trim_start().len();
            if after.trim_start().starts_with('{') {
                if let Some(d_end) = matching_bracket(after, offset) {
                    let denom = &after[offset + 1..d_end];
                    return format!("{}/{}", numer.trim(), denom.trim());
                }
            }
        }
    }
    unquote(t.trim_matches(|c| c == '{' || c == '}').trim())
}

/// Split on `seps` at bracket depth 0, respecting quotes.
fn split_top_level(s: &str, seps: &[char]) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut current = String::new();
    for ch in s.chars() {
        match quote {
            Some(q) => {
                current.push(ch);
                if ch == q {
                    quote = None;
                }
            }
            None => match ch {
                '\'' | '"' => {
                    quote = Some(ch);
                    current.push(ch);
                }
                '[' | '{' | '(' => {
                    depth += 1;
                    current.push(ch);
                }
                ']' | '}' | ')' => {
                    depth -= 1;
                    current.push(ch);
                }
                c if depth == 0 && seps.contains(&c) => {
                    out.push(current.clone());
                    current.clear();
                }
                c => current.push(c),
            },
        }
    }
    out.push(current);
    out
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    let chars: Vec<char> = t.chars().collect();
    if chars.len() >= 2 {
        let f = chars[0];
        let l = chars[chars.len() - 1];
        if (f == '"' && l == '"') || (f == '\'' && l == '\'') {
            return t[f.len_utf8()..t.len() - l.len_utf8()].to_string();
        }
    }
    t.to_string()
}

fn clip(s: &str) -> String {
    let t: String = s.chars().take(40).collect();
    if s.chars().count() > 40 {
        format!("{t}…")
    } else {
        t
    }
}

// ---------------------------------------------------------------------------
// cell typing
// ---------------------------------------------------------------------------

fn cell_value(raw: &str, cells: Cells, fractions: bool) -> Option<Value> {
    let t = unquote(raw);
    if cells == Cells::Text {
        return Some(Value::String(t));
    }
    match parse_number(&t, fractions) {
        Some(v) => Some(num_exact(v, &t)),
        None if cells == Cells::Number => None,
        None => Some(Value::String(t)),
    }
}

/// Keep integers as integers; keep decimals as f64 without re-formatting surprises.
fn num_exact(v: f64, raw: &str) -> Value {
    let looks_integral = !raw.contains('.') && !raw.contains('e') && !raw.contains('E');
    if looks_integral && v.fract() == 0.0 && v.abs() < 9.0e15 {
        return Value::Number(Number::from(v as i64));
    }
    Number::from_f64(v).map(Value::Number).unwrap_or(Value::Null)
}

fn parse_number(t: &str, fractions: bool) -> Option<f64> {
    if t.is_empty() {
        return None;
    }
    if let Ok(v) = t.parse::<f64>() {
        if v.is_finite() {
            return Some(v);
        }
        return None;
    }
    if fractions {
        if let Some((n, d)) = t.split_once('/') {
            let n = n.trim().parse::<f64>().ok()?;
            let d = d.trim().parse::<f64>().ok()?;
            if d != 0.0 && n.is_finite() && d.is_finite() {
                let q = n / d;
                if q.is_finite() {
                    return Some(q);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(matrix: &str) -> String {
        parse_matrix(
            matrix, "auto", "auto", "json", "auto", true, false, "error", "0", 0.0,
        )
        .unwrap()
    }

    fn out(matrix: &str, output: &str) -> String {
        parse_matrix(
            matrix, "auto", "auto", output, "auto", true, false, "error", "0", 0.0,
        )
        .unwrap()
    }

    #[test]
    fn whitespace_rows_parse_with_shape() {
        assert_eq!(
            p("1 2 3\n4 5 6"),
            r#"{"shape":[2,3],"rows":2,"columns":3,"square":false,"numeric":true,"input_format":"delimited","delimiter":"space","matrix":[[1,2,3],[4,5,6]]}"#
        );
    }

    #[test]
    fn python_nested_list_parses() {
        assert_eq!(
            p("[[1, 2], [3, 4]]"),
            r#"{"shape":[2,2],"rows":2,"columns":2,"square":true,"numeric":true,"input_format":"python","delimiter":"comma","matrix":[[1,2],[3,4]]}"#
        );
    }

    #[test]
    fn numpy_call_wrapper_is_stripped() {
        assert_eq!(
            out("np.array([[1, 2], [3, 4]], dtype=float)", "array"),
            "[[1,2],[3,4]]"
        );
    }

    #[test]
    fn matlab_semicolon_syntax_parses() {
        assert_eq!(
            p("[1 2; 3 4]"),
            r#"{"shape":[2,2],"rows":2,"columns":2,"square":true,"numeric":true,"input_format":"matlab","delimiter":"space","matrix":[[1,2],[3,4]]}"#
        );
    }

    #[test]
    fn bracketless_matlab_single_line_parses() {
        assert_eq!(out("1 2; 3 4", "array"), "[[1,2],[3,4]]");
    }

    #[test]
    fn curly_literal_with_commas_parses() {
        assert_eq!(out("{{1,3},{4,5}}", "array"), "[[1,3],[4,5]]");
    }

    #[test]
    fn latex_bmatrix_parses() {
        assert_eq!(
            out("\\begin{bmatrix} 1 & 2 \\\\ 3 & 4 \\end{bmatrix}", "array"),
            "[[1,2],[3,4]]"
        );
    }

    #[test]
    fn latex_array_column_spec_is_dropped() {
        assert_eq!(
            out("\\begin{array}{cc} 1 & 2 \\\\ 3 & 4 \\end{array}", "array"),
            "[[1,2],[3,4]]"
        );
    }

    #[test]
    fn latex_frac_becomes_a_decimal() {
        assert_eq!(
            out("\\begin{pmatrix} \\frac{1}{2} & 2 \\end{pmatrix}", "array"),
            "[[0.5,2]]"
        );
    }

    #[test]
    fn tab_and_comma_delimiters_are_detected() {
        assert_eq!(out("1\t2\n3\t4", "array"), "[[1,2],[3,4]]");
        assert_eq!(out("1,2\n3,4", "array"), "[[1,2],[3,4]]");
        assert_eq!(out("1|2\n3|4", "array"), "[[1,2],[3,4]]");
    }

    #[test]
    fn explicit_delimiter_overrides_detection() {
        // Auto would pick the comma; forcing space keeps the pair as one text cell.
        let json = parse_matrix(
            "1,2 3,4", "delimited", "space", "array", "text", true, false, "error", "0", 0.0,
        )
        .unwrap();
        assert_eq!(json, r#"[["1,2","3,4"]]"#);
    }

    #[test]
    fn scientific_notation_and_fractions_become_numbers() {
        assert_eq!(out("1.2e-4 3/4\n-5 +6", "array"), "[[0.00012,0.75],[-5,6]]");
    }

    #[test]
    fn fractions_off_keeps_the_text() {
        let json = parse_matrix(
            "3/4 2", "auto", "auto", "array", "auto", false, false, "error", "0", 0.0,
        )
        .unwrap();
        assert_eq!(json, r#"[["3/4",2]]"#);
    }

    #[test]
    fn non_numeric_cells_stay_text_in_auto_mode() {
        let json = out("a b\nc 2", "array");
        assert_eq!(json, r#"[["a","b"],["c",2]]"#);
        assert!(p("a b\nc 2").contains(r#""numeric":false"#));
    }

    #[test]
    fn strict_number_mode_rejects_text_with_a_location() {
        let err = parse_matrix(
            "1 2\n3 x", "auto", "auto", "json", "number", true, false, "error", "0", 0.0,
        )
        .unwrap_err();
        assert!(
            err.contains("cell 'x' at row 2, column 2 is not a number"),
            "got: {err}"
        );
    }

    #[test]
    fn ragged_rows_error_by_default() {
        let err = parse_matrix(
            "1 2 3\n4 5", "auto", "auto", "json", "auto", true, false, "error", "0", 0.0,
        )
        .unwrap_err();
        assert!(
            err.contains("ragged matrix: row 1 has 3 cells but row 2 has 2"),
            "got: {err}"
        );
    }

    #[test]
    fn ragged_pad_and_trim_normalize() {
        let padded = parse_matrix(
            "1 2 3\n4 5", "auto", "auto", "array", "auto", true, false, "pad", "0", 0.0,
        )
        .unwrap();
        assert_eq!(padded, "[[1,2,3],[4,5,0]]");
        let trimmed = parse_matrix(
            "1 2 3\n4 5", "auto", "auto", "array", "auto", true, false, "trim", "0", 0.0,
        )
        .unwrap();
        assert_eq!(trimmed, "[[1,2],[4,5]]");
        let report = parse_matrix(
            "1 2 3\n4 5", "auto", "auto", "json", "auto", true, false, "pad", "n/a", 0.0,
        )
        .unwrap();
        assert!(report.contains(r#""ragged_fixed":1"#), "got: {report}");
        assert!(report.contains(r#"[4,5,"n/a"]"#), "got: {report}");
    }

    #[test]
    fn header_row_is_reported_separately() {
        let report = parse_matrix(
            "x,y\n1,2\n3,4", "auto", "auto", "json", "auto", true, true, "error", "0", 0.0,
        )
        .unwrap();
        assert_eq!(
            report,
            r#"{"shape":[2,2],"rows":2,"columns":2,"square":true,"numeric":true,"input_format":"delimited","delimiter":"comma","header":["x","y"],"matrix":[[1,2],[3,4]]}"#
        );
    }

    #[test]
    fn header_requires_a_data_row() {
        let err = parse_matrix(
            "x,y", "auto", "auto", "json", "auto", true, true, "error", "0", 0.0,
        )
        .unwrap_err();
        assert!(err.contains("header is on but the matrix has only one row"), "got: {err}");
    }

    #[test]
    fn every_output_format_renders() {
        let m = "1 2\n30 4";
        assert_eq!(out(m, "csv"), "1,2\n30,4");
        assert_eq!(out(m, "tsv"), "1\t2\n30\t4");
        assert_eq!(out(m, "matlab"), "[1 2; 30 4]");
        assert_eq!(out(m, "numpy"), "np.array([[1, 2], [30, 4]])");
        assert_eq!(
            out(m, "latex"),
            "\\begin{bmatrix}\n1 & 2 \\\\\n30 & 4\n\\end{bmatrix}"
        );
        assert_eq!(out(m, "aligned"), " 1  2\n30  4");
    }

    #[test]
    fn header_rides_along_in_every_output() {
        let call = |o: &str| {
            parse_matrix("x,y\n1,2", "auto", "auto", o, "auto", true, true, "error", "0", 0.0)
                .unwrap()
        };
        assert_eq!(call("csv"), "x,y\n1,2");
        assert_eq!(call("matlab"), "% x, y\n[1 2]");
        assert_eq!(call("numpy"), "# x, y\nnp.array([[1, 2]])");
        assert_eq!(call("aligned"), "x  y\n1  2");
    }

    #[test]
    fn csv_output_quotes_cells_containing_the_separator() {
        let json = parse_matrix(
            "[[\"a,b\", 2]]", "auto", "auto", "csv", "auto", true, false, "error", "0", 0.0,
        )
        .unwrap();
        assert_eq!(json, "\"a,b\",2");
    }

    #[test]
    fn indent_controls_json_formatting() {
        let pretty = parse_matrix(
            "1 2", "auto", "auto", "array", "auto", true, false, "error", "0", 2.0,
        )
        .unwrap();
        assert_eq!(pretty, "[\n  [\n    1,\n    2\n  ]\n]");
    }

    #[test]
    fn comment_and_blank_lines_are_skipped() {
        assert_eq!(out("# shape 2x2\n1 2\n\n// note\n3 4", "array"), "[[1,2],[3,4]]");
    }

    #[test]
    fn code_paste_with_trailing_commas_and_row_brackets_parses() {
        assert_eq!(out("[1, 2],\n[3, 4],", "array"), "[[1,2],[3,4]]");
    }

    #[test]
    fn single_row_vector_parses() {
        assert_eq!(out("[1, 2, 3]", "array"), "[[1,2,3]]");
        assert!(p("[1, 2, 3]").contains(r#""shape":[1,3]"#));
    }

    #[test]
    fn empty_input_is_rejected() {
        let err = parse_matrix(
            "   \n  ", "auto", "auto", "json", "auto", true, false, "error", "0", 0.0,
        )
        .unwrap_err();
        assert!(err.contains("matrix is empty"), "got: {err}");
    }

    #[test]
    fn unknown_enum_values_are_rejected_by_name() {
        for (args, needle) in [
            (("bogus", "auto", "json", "auto", "error"), "unknown input_format 'bogus'"),
            (("auto", "bogus", "json", "auto", "error"), "unknown delimiter 'bogus'"),
            (("auto", "auto", "bogus", "auto", "error"), "unknown output 'bogus'"),
            (("auto", "auto", "json", "bogus", "error"), "unknown cells 'bogus'"),
            (("auto", "auto", "json", "auto", "bogus"), "unknown ragged 'bogus'"),
        ] {
            let err = parse_matrix(
                "1 2", args.0, args.1, args.2, args.3, true, false, args.4, "0", 0.0,
            )
            .unwrap_err();
            assert!(err.contains(needle), "got: {err}");
        }
    }

    #[test]
    fn indent_outside_the_range_is_rejected() {
        let err = parse_matrix(
            "1 2", "auto", "auto", "json", "auto", true, false, "error", "0", 9.0,
        )
        .unwrap_err();
        assert!(err.contains("indent must be a whole number of spaces between 0 and 8"), "got: {err}");
    }

    #[test]
    fn column_cap_is_enforced_at_the_boundary() {
        let at_cap = (1..=MAX_COLUMNS).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        assert!(parse_matrix(
            &at_cap, "auto", "auto", "array", "auto", true, false, "error", "0", 0.0
        )
        .is_ok());
        let over = format!("{at_cap},1");
        let err = parse_matrix(
            &over, "auto", "auto", "array", "auto", true, false, "error", "0", 0.0,
        )
        .unwrap_err();
        assert!(err.contains("over the 2000-column limit"), "got: {err}");
    }

    #[test]
    fn byte_cap_is_enforced() {
        let big = "1 2\n".repeat(MAX_BYTES / 4 + 1);
        let err = parse_matrix(
            &big, "auto", "auto", "array", "auto", true, false, "error", "0", 0.0,
        )
        .unwrap_err();
        assert!(err.contains("over the 2000000-byte limit"), "got: {err}");
    }

    #[test]
    fn mixed_nested_and_bare_values_are_rejected() {
        let err = parse_matrix(
            "[[1,2], 3]", "python", "auto", "array", "auto", true, false, "error", "0", 0.0,
        )
        .unwrap_err();
        assert!(err.contains("mixes bare values with nested rows"), "got: {err}");
    }

    #[test]
    fn non_finite_cells_stay_text() {
        assert_eq!(out("nan inf", "array"), r#"[["nan","inf"]]"#);
    }
}
