//! Convert a CSV of 2D points between Cartesian (x, y) and polar (r, θ) coordinates.
//!
//! The engine is header-aware: it picks the coordinate columns by name or 1-based
//! index (auto-detecting the usual spellings when none is given), keeps every other
//! column intact, and re-emits the table as CSV, TSV, JSON or an aligned text table.
//! Angles use `f64::atan2`, so all four quadrants are correct, and can be reported in
//! degrees, radians, gradians or turns, either signed or wrapped into a positive range.

/// Largest accepted input size, in bytes.
pub const MAX_INPUT_BYTES: usize = 5 * 1024 * 1024;
/// Largest accepted number of data rows.
pub const MAX_ROWS: usize = 200_000;
/// Largest accepted `decimals` value.
pub const MAX_DECIMALS: usize = 15;

/// Which way the conversion runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Direction {
    ToPolar,
    ToCartesian,
}

/// Unit used for the angular value (output when converting to polar, input when
/// converting back to Cartesian).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AngleUnit {
    Degrees,
    Radians,
    Gradians,
    Turns,
}

impl AngleUnit {
    /// One full revolution expressed in this unit.
    fn turn(self) -> f64 {
        match self {
            AngleUnit::Degrees => 360.0,
            AngleUnit::Radians => std::f64::consts::TAU,
            AngleUnit::Gradians => 400.0,
            AngleUnit::Turns => 1.0,
        }
    }

    fn from_radians(self, radians: f64) -> f64 {
        radians / std::f64::consts::TAU * self.turn()
    }

    fn to_radians(self, value: f64) -> f64 {
        value / self.turn() * std::f64::consts::TAU
    }
}

/// Parsed, validated options for one conversion run.
struct Options {
    direction: Direction,
    angle_unit: AngleUnit,
    positive_range: bool,
    decimals: usize,
    delimiter: u8,
    has_header: bool,
    keep_columns: bool,
    output: Output,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Output {
    Csv,
    Tsv,
    Json,
    Table,
}

/// Column names the auto-detector accepts for each coordinate slot.
const X_ALIASES: &[&str] = &["x", "x_coord", "xcoord", "x coordinate", "easting", "east"];
const Y_ALIASES: &[&str] = &["y", "y_coord", "ycoord", "y coordinate", "northing", "north"];
const R_ALIASES: &[&str] = &["r", "rho", "radius", "magnitude", "mag", "dist", "distance"];
const T_ALIASES: &[&str] = &["theta", "t", "phi", "angle", "azimuth", "bearing", "deg", "rad"];

/// Convert `csv_text` between Cartesian and polar coordinates.
///
/// `x_column` / `y_column` name the two coordinate columns (header name or 1-based
/// index); empty strings auto-detect. Returns the rendered table or a human-readable
/// error naming the offending row.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    csv_text: &str,
    direction: &str,
    x_column: &str,
    y_column: &str,
    angle_unit: &str,
    angle_range: &str,
    decimals: i64,
    delimiter: &str,
    has_header: bool,
    keep_columns: bool,
    output: &str,
) -> Result<String, String> {
    if csv_text.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "CSV input is {} bytes, above the {} byte limit",
            csv_text.len(),
            MAX_INPUT_BYTES
        ));
    }
    if csv_text.trim().is_empty() {
        return Err("CSV input is empty — paste at least one row of coordinates".to_string());
    }

    let direction = match direction.trim().to_ascii_lowercase().as_str() {
        "" | "cartesian_to_polar" | "to_polar" | "xy_to_rt" => Direction::ToPolar,
        "polar_to_cartesian" | "to_cartesian" | "rt_to_xy" => Direction::ToCartesian,
        other => {
            return Err(format!(
                "unknown direction \"{other}\" — use cartesian_to_polar or polar_to_cartesian"
            ))
        }
    };
    let angle_unit = match angle_unit.trim().to_ascii_lowercase().as_str() {
        "" | "degrees" | "deg" => AngleUnit::Degrees,
        "radians" | "rad" => AngleUnit::Radians,
        "gradians" | "grad" | "gon" => AngleUnit::Gradians,
        "turns" | "turn" | "rev" => AngleUnit::Turns,
        other => {
            return Err(format!(
                "unknown angle_unit \"{other}\" — use degrees, radians, gradians or turns"
            ))
        }
    };
    let positive_range = match angle_range.trim().to_ascii_lowercase().as_str() {
        "" | "signed" => false,
        "positive" => true,
        other => {
            return Err(format!(
                "unknown angle_range \"{other}\" — use signed or positive"
            ))
        }
    };
    if !(0..=MAX_DECIMALS as i64).contains(&decimals) {
        return Err(format!(
            "decimals must be between 0 and {MAX_DECIMALS}, got {decimals}"
        ));
    }
    let output = match output.trim().to_ascii_lowercase().as_str() {
        "" | "csv" => Output::Csv,
        "tsv" => Output::Tsv,
        "json" => Output::Json,
        "table" => Output::Table,
        other => {
            return Err(format!(
                "unknown output \"{other}\" — use csv, tsv, json or table"
            ))
        }
    };
    let delimiter = resolve_delimiter(delimiter, csv_text)?;

    let opts = Options {
        direction,
        angle_unit,
        positive_range,
        decimals: decimals as usize,
        delimiter,
        has_header,
        keep_columns,
        output,
    };

    let records = read_records(csv_text, opts.delimiter)?;
    if records.is_empty() {
        return Err("CSV input is empty — paste at least one row of coordinates".to_string());
    }

    let (headers, rows) = if opts.has_header {
        let (head, rest) = records.split_first().expect("records is non-empty");
        (head.clone(), rest.to_vec())
    } else {
        let width = records.iter().map(Vec::len).max().unwrap_or(0);
        let head = (1..=width).map(|i| format!("column{i}")).collect::<Vec<_>>();
        (head, records)
    };
    if rows.is_empty() {
        return Err(
            "CSV has a header but no data rows — add at least one row of coordinates".to_string(),
        );
    }
    if rows.len() > MAX_ROWS {
        return Err(format!(
            "CSV has {} data rows, above the {MAX_ROWS} row limit",
            rows.len()
        ));
    }

    let (first_aliases, second_aliases) = match opts.direction {
        Direction::ToPolar => (X_ALIASES, Y_ALIASES),
        Direction::ToCartesian => (R_ALIASES, T_ALIASES),
    };
    let first_idx = resolve_column(x_column, &headers, first_aliases, "x_column", opts.direction)?;
    let second_idx = resolve_column(y_column, &headers, second_aliases, "y_column", opts.direction)?;
    if first_idx == second_idx {
        return Err(format!(
            "x_column and y_column both resolve to column {} — pick two different columns",
            first_idx + 1
        ));
    }

    let (out_a, out_b) = match opts.direction {
        Direction::ToPolar => ("r", "theta"),
        Direction::ToCartesian => ("x", "y"),
    };
    let mut out_headers: Vec<String> = Vec::new();
    if opts.keep_columns {
        for (i, h) in headers.iter().enumerate() {
            if i != first_idx && i != second_idx {
                out_headers.push(h.clone());
            }
        }
    }
    out_headers.push(out_a.to_string());
    out_headers.push(out_b.to_string());

    let mut out_rows: Vec<Vec<String>> = Vec::with_capacity(rows.len());
    for (n, row) in rows.iter().enumerate() {
        let line_no = n + 1 + usize::from(opts.has_header);
        let a = cell(row, first_idx, line_no, &headers)?;
        let b = cell(row, second_idx, line_no, &headers)?;
        let (va, vb) = match opts.direction {
            Direction::ToPolar => {
                let x = parse_number(a, line_no, &headers, first_idx)?;
                let y = parse_number(b, line_no, &headers, second_idx)?;
                let r = x.hypot(y);
                let mut theta = opts.angle_unit.from_radians(y.atan2(x));
                if opts.positive_range && theta < 0.0 {
                    theta += opts.angle_unit.turn();
                }
                (r, theta)
            }
            Direction::ToCartesian => {
                let r = parse_number(a, line_no, &headers, first_idx)?;
                let theta = opts.angle_unit.to_radians(parse_number(
                    b,
                    line_no,
                    &headers,
                    second_idx,
                )?);
                (r * theta.cos(), r * theta.sin())
            }
        };
        let mut out_row: Vec<String> = Vec::with_capacity(out_headers.len());
        if opts.keep_columns {
            for (i, _) in headers.iter().enumerate() {
                if i != first_idx && i != second_idx {
                    out_row.push(row.get(i).cloned().unwrap_or_default());
                }
            }
        }
        out_row.push(format_number(va, opts.decimals));
        out_row.push(format_number(vb, opts.decimals));
        out_rows.push(out_row);
    }

    Ok(render(&out_headers, &out_rows, &opts))
}

/// Pick the field delimiter, sniffing the first line when `spec` is `auto`.
fn resolve_delimiter(spec: &str, csv_text: &str) -> Result<u8, String> {
    match spec.trim().to_ascii_lowercase().as_str() {
        "comma" => Ok(b','),
        "semicolon" => Ok(b';'),
        "tab" => Ok(b'\t'),
        "pipe" => Ok(b'|'),
        "" | "auto" => {
            let first = csv_text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
            let best = [b',', b';', b'\t', b'|']
                .into_iter()
                .map(|d| (first.bytes().filter(|b| *b == d).count(), d))
                .max_by_key(|(count, _)| *count)
                .map(|(count, d)| if count == 0 { b',' } else { d })
                .unwrap_or(b',');
            Ok(best)
        }
        other => Err(format!(
            "unknown delimiter \"{other}\" — use auto, comma, semicolon, tab or pipe"
        )),
    }
}

/// Parse the CSV text into records, skipping blank lines.
fn read_records(csv_text: &str, delimiter: u8) -> Result<Vec<Vec<String>>, String> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(csv_text.as_bytes());
    let mut out = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| format!("CSV parse error: {e}"))?;
        let fields: Vec<String> = record.iter().map(str::to_string).collect();
        if fields.iter().all(|f| f.is_empty()) {
            continue;
        }
        out.push(fields);
    }
    Ok(out)
}

/// Resolve a coordinate column from a header name, a 1-based index, or the aliases.
fn resolve_column(
    spec: &str,
    headers: &[String],
    aliases: &[&str],
    param: &str,
    direction: Direction,
) -> Result<usize, String> {
    let spec = spec.trim();
    if !spec.is_empty() {
        if let Ok(n) = spec.parse::<usize>() {
            if n == 0 || n > headers.len() {
                return Err(format!(
                    "{param} index {n} is out of range — the CSV has {} columns",
                    headers.len()
                ));
            }
            return Ok(n - 1);
        }
        if let Some(i) = headers
            .iter()
            .position(|h| h.trim().eq_ignore_ascii_case(spec))
        {
            return Ok(i);
        }
        return Err(format!(
            "{param} \"{spec}\" is not a column — available columns: {}",
            headers.join(", ")
        ));
    }
    if let Some(i) = headers
        .iter()
        .position(|h| aliases.iter().any(|a| h.trim().eq_ignore_ascii_case(a)))
    {
        return Ok(i);
    }
    // No header match: fall back to the first two columns in reading order.
    let fallback = if param == "x_column" { 0 } else { 1 };
    if fallback < headers.len() {
        return Ok(fallback);
    }
    let wanted = match direction {
        Direction::ToPolar => "x and y",
        Direction::ToCartesian => "r and theta",
    };
    Err(format!(
        "could not find the {wanted} columns — the CSV has {} column(s); set x_column and y_column explicitly",
        headers.len()
    ))
}

/// Fetch one cell, reporting a ragged row rather than silently padding it.
fn cell<'a>(
    row: &'a [String],
    idx: usize,
    line_no: usize,
    headers: &[String],
) -> Result<&'a str, String> {
    row.get(idx).map(String::as_str).ok_or_else(|| {
        format!(
            "row {line_no} has {} field(s) but column \"{}\" is #{}",
            row.len(),
            headers.get(idx).map(String::as_str).unwrap_or("?"),
            idx + 1
        )
    })
}

/// Parse one numeric cell, naming the row and column when it is not a number.
fn parse_number(
    raw: &str,
    line_no: usize,
    headers: &[String],
    idx: usize,
) -> Result<f64, String> {
    let cleaned = raw.trim();
    let name = headers.get(idx).map(String::as_str).unwrap_or("?");
    if cleaned.is_empty() {
        return Err(format!("row {line_no}: column \"{name}\" is empty"));
    }
    let value: f64 = cleaned
        .parse()
        .map_err(|_| format!("row {line_no}: column \"{name}\" value \"{cleaned}\" is not a number"))?;
    if !value.is_finite() {
        return Err(format!(
            "row {line_no}: column \"{name}\" value \"{cleaned}\" is not finite"
        ));
    }
    Ok(value)
}

/// Render a value at fixed precision, collapsing negative zero to `0`.
fn format_number(value: f64, decimals: usize) -> String {
    let rendered = format!("{value:.decimals$}");
    if rendered.bytes().all(|b| matches!(b, b'-' | b'0' | b'.')) && rendered.starts_with('-') {
        return rendered[1..].to_string();
    }
    rendered
}

/// Serialize the converted table in the requested output shape.
fn render(headers: &[String], rows: &[Vec<String>], opts: &Options) -> String {
    match opts.output {
        Output::Csv | Output::Tsv => {
            let delim = if opts.output == Output::Tsv {
                b'\t'
            } else {
                opts.delimiter
            };
            let mut writer = csv::WriterBuilder::new()
                .delimiter(delim)
                .from_writer(Vec::new());
            if opts.has_header {
                let _ = writer.write_record(headers);
            }
            for row in rows {
                let _ = writer.write_record(row);
            }
            let bytes = writer.into_inner().unwrap_or_default();
            String::from_utf8_lossy(&bytes).into_owned()
        }
        Output::Json => {
            let mut out = String::from("[\n");
            for (n, row) in rows.iter().enumerate() {
                out.push_str("  {");
                for (i, header) in headers.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    let value = row.get(i).map(String::as_str).unwrap_or("");
                    out.push_str(&json_string(header));
                    out.push_str(": ");
                    // Converted coordinates are always numeric; kept columns stay strings
                    // unless they parse cleanly as a number.
                    if value.parse::<f64>().map(f64::is_finite).unwrap_or(false) {
                        out.push_str(value);
                    } else {
                        out.push_str(&json_string(value));
                    }
                }
                out.push('}');
                if n + 1 < rows.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str("]\n");
            out
        }
        Output::Table => {
            let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
            for row in rows {
                for (i, value) in row.iter().enumerate() {
                    if i < widths.len() {
                        widths[i] = widths[i].max(value.chars().count());
                    }
                }
            }
            let line = |cells: &[String]| -> String {
                cells
                    .iter()
                    .enumerate()
                    .map(|(i, c)| format!("{:>width$}", c, width = widths[i]))
                    .collect::<Vec<_>>()
                    .join("  ")
                    .trim_end()
                    .to_string()
            };
            let mut out = String::new();
            if opts.has_header {
                out.push_str(&line(headers));
                out.push('\n');
                out.push_str(
                    &widths
                        .iter()
                        .map(|w| "-".repeat(*w))
                        .collect::<Vec<_>>()
                        .join("  "),
                );
                out.push('\n');
            }
            for row in rows {
                out.push_str(&line(row));
                out.push('\n');
            }
            out
        }
    }
}

/// Minimal JSON string escaping for header names and passthrough cells.
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_polar(csv: &str) -> Result<String, String> {
        convert(
            csv,
            "cartesian_to_polar",
            "",
            "",
            "degrees",
            "signed",
            4,
            "auto",
            true,
            true,
            "csv",
        )
    }

    #[test]
    fn converts_a_headered_csv_to_polar_degrees() {
        let out = to_polar("x,y\n3,4\n0,1\n-1,0\n").unwrap();
        assert_eq!(
            out,
            "r,theta\n5.0000,53.1301\n1.0000,90.0000\n1.0000,180.0000\n"
        );
    }

    #[test]
    fn keeps_extra_columns_and_uses_atan2_quadrants() {
        let out = to_polar("id,x,y\na,-3,-4\nb,3,-4\n").unwrap();
        assert_eq!(out, "id,r,theta\na,5.0000,-126.8699\nb,5.0000,-53.1301\n");
    }

    #[test]
    fn positive_range_wraps_negative_angles() {
        let out = convert(
            "x,y\n-3,-4\n",
            "cartesian_to_polar",
            "",
            "",
            "degrees",
            "positive",
            2,
            "auto",
            true,
            true,
            "csv",
        )
        .unwrap();
        assert_eq!(out, "r,theta\n5.00,233.13\n");
    }

    #[test]
    fn radians_gradians_and_turns_share_one_quarter_turn() {
        for (unit, expected) in [
            ("radians", "1.0000,1.5708\n"),
            ("gradians", "1.0000,100.0000\n"),
            ("turns", "1.0000,0.2500\n"),
        ] {
            let out = convert(
                "x,y\n0,1\n",
                "cartesian_to_polar",
                "",
                "",
                unit,
                "signed",
                4,
                "auto",
                true,
                true,
                "csv",
            )
            .unwrap();
            assert_eq!(out, format!("r,theta\n{expected}"), "unit {unit}");
        }
    }

    #[test]
    fn polar_to_cartesian_round_trips() {
        let out = convert(
            "r,theta\n5,53.1301\n",
            "polar_to_cartesian",
            "",
            "",
            "degrees",
            "signed",
            3,
            "auto",
            true,
            true,
            "csv",
        )
        .unwrap();
        assert_eq!(out, "x,y\n3.000,4.000\n");
    }

    #[test]
    fn semicolon_delimiter_is_sniffed_and_echoed() {
        let out = to_polar("x;y\n3;4\n").unwrap();
        assert_eq!(out, "r;theta\n5.0000;53.1301\n");
    }

    #[test]
    fn columns_can_be_selected_by_name_or_index() {
        let by_name = convert(
            "east,north,label\n3,4,p\n",
            "cartesian_to_polar",
            "east",
            "north",
            "degrees",
            "signed",
            1,
            "auto",
            true,
            true,
            "csv",
        )
        .unwrap();
        assert_eq!(by_name, "label,r,theta\np,5.0,53.1\n");

        let by_index = convert(
            "a,b,c\n9,3,4\n",
            "cartesian_to_polar",
            "2",
            "3",
            "degrees",
            "signed",
            1,
            "auto",
            true,
            true,
            "csv",
        )
        .unwrap();
        assert_eq!(by_index, "a,r,theta\n9,5.0,53.1\n");
    }

    #[test]
    fn headerless_input_uses_the_first_two_columns() {
        let out = convert(
            "3,4\n0,-2\n",
            "cartesian_to_polar",
            "",
            "",
            "degrees",
            "signed",
            1,
            "auto",
            false,
            false,
            "csv",
        )
        .unwrap();
        assert_eq!(out, "5.0,53.1\n2.0,-90.0\n");
    }

    #[test]
    fn json_and_table_outputs_render() {
        let json = convert(
            "id,x,y\np1,3,4\n",
            "cartesian_to_polar",
            "",
            "",
            "degrees",
            "signed",
            2,
            "auto",
            true,
            true,
            "json",
        )
        .unwrap();
        assert_eq!(json, "[\n  {\"id\": \"p1\", \"r\": 5.00, \"theta\": 53.13}\n]\n");

        let table = convert(
            "x,y\n3,4\n",
            "cartesian_to_polar",
            "",
            "",
            "degrees",
            "signed",
            2,
            "auto",
            true,
            true,
            "table",
        )
        .unwrap();
        assert_eq!(table, "   r  theta\n----  -----\n5.00  53.13\n");
    }

    #[test]
    fn origin_yields_zero_radius_and_zero_angle() {
        let out = to_polar("x,y\n0,0\n").unwrap();
        assert_eq!(out, "r,theta\n0.0000,0.0000\n");
    }

    #[test]
    fn non_numeric_cells_name_the_row_and_column() {
        let err = to_polar("x,y\n3,4\nnope,1\n").unwrap_err();
        assert_eq!(
            err,
            "row 3: column \"x\" value \"nope\" is not a number"
        );
    }

    #[test]
    fn unknown_column_lists_the_available_columns() {
        let err = convert(
            "a,b\n1,2\n",
            "cartesian_to_polar",
            "lon",
            "b",
            "degrees",
            "signed",
            4,
            "auto",
            true,
            true,
            "csv",
        )
        .unwrap_err();
        assert_eq!(err, "x_column \"lon\" is not a column — available columns: a, b");
    }

    #[test]
    fn empty_input_and_bad_options_are_rejected() {
        assert!(to_polar("   ").unwrap_err().contains("empty"));
        let err = convert(
            "x,y\n1,2\n",
            "sideways",
            "",
            "",
            "degrees",
            "signed",
            4,
            "auto",
            true,
            true,
            "csv",
        )
        .unwrap_err();
        assert!(err.contains("unknown direction"), "{err}");
        let err = convert(
            "x,y\n1,2\n",
            "cartesian_to_polar",
            "",
            "",
            "degrees",
            "signed",
            99,
            "auto",
            true,
            true,
            "csv",
        )
        .unwrap_err();
        assert!(err.contains("decimals must be between 0 and 15"), "{err}");
    }

    #[test]
    fn header_only_input_is_reported() {
        let err = to_polar("x,y\n").unwrap_err();
        assert!(err.contains("no data rows"), "{err}");
    }
}
