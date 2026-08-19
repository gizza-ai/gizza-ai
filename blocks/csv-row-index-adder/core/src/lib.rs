//! csv-row-index-adder core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps. Adds ONE generated key column to a CSV:
//! a sequential number, a UUID (v4 random or v7 time-ordered), or a composite key
//! built by joining existing columns.
//!
//! Everything else round-trips: existing cells are copied verbatim, the field
//! separator is preserved, quoted fields keep RFC 4180 semantics, and ragged rows
//! keep their own width (the new column is still added to every row).
//!
//! UUID v4 uses platform randomness. UUID v7 embeds a caller-supplied timestamp,
//! so the core never reads a clock itself: each surface passes `now_ms` (chat/CLI
//! use the system clock, the page uses `Date.now()`), keeping this function
//! deterministic given its inputs.

use std::collections::HashSet;

/// Hard cap on the pasted table, so a runaway paste can't wedge the tab.
pub const MAX_INPUT_BYTES: usize = 5_000_000;

/// Largest zero-padding width accepted, so a typo can't allocate a huge string.
pub const MAX_PAD_WIDTH: i64 = 64;

/// Which kind of key column to generate.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Sequential,
    Uuid,
    Composite,
}

impl Mode {
    fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "sequential" | "seq" | "index" => Ok(Mode::Sequential),
            "uuid" | "guid" => Ok(Mode::Uuid),
            "composite" | "key" => Ok(Mode::Composite),
            other => Err(format!(
                "unknown mode '{other}' (use sequential, uuid, or composite)"
            )),
        }
    }

    /// Column name used when `column_name` is left blank.
    fn default_column_name(self) -> &'static str {
        match self {
            Mode::Sequential => "index",
            Mode::Uuid => "uuid",
            Mode::Composite => "key",
        }
    }
}

/// Where the generated column goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Position {
    Start,
    End,
    Before,
    After,
}

impl Position {
    fn parse(s: &str) -> Result<Position, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "start" | "first" | "left" => Ok(Position::Start),
            "end" | "last" | "right" | "append" => Ok(Position::End),
            "before" => Ok(Position::Before),
            "after" => Ok(Position::After),
            other => Err(format!(
                "unknown position '{other}' (use start, end, before, or after)"
            )),
        }
    }
}

/// Resolve a delimiter spec to its byte. `auto` sniffs the first non-empty line.
fn delim_byte(spec: &str, data: &str) -> Result<u8, String> {
    let s = spec.trim();
    Ok(match s {
        "auto" => sniff_delimiter(data),
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            return Err(format!(
                "delimiter must be 'auto', ',', 'tab', ';' or '|', got '{other}'"
            ))
        }
    })
}

/// Pick the delimiter that occurs most often outside quotes on the first
/// non-empty line. Ties (and a line with none of them) fall back to a comma.
fn sniff_delimiter(data: &str) -> u8 {
    let line = data.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut in_quote = false;
    let (mut comma, mut tab, mut semi, mut pipe) = (0usize, 0usize, 0usize, 0usize);
    for ch in line.chars() {
        match ch {
            '"' => in_quote = !in_quote,
            _ if in_quote => {}
            ',' => comma += 1,
            '\t' => tab += 1,
            ';' => semi += 1,
            '|' => pipe += 1,
            _ => {}
        }
    }
    let best = comma.max(tab).max(semi).max(pipe);
    if best == 0 || comma == best {
        b','
    } else if tab == best {
        b'\t'
    } else if semi == best {
        b';'
    } else {
        b'|'
    }
}

/// Resolve one column reference — a header name, or a 1-based position — to a
/// 0-based column index. Names are matched exactly first, then case-insensitively.
fn resolve_column(
    reference: &str,
    header: Option<&[String]>,
    width: usize,
) -> Result<usize, String> {
    let r = reference.trim();
    if r.is_empty() {
        return Err("column reference is empty".into());
    }
    if let Some(cols) = header {
        if let Some(i) = cols.iter().position(|c| c == r) {
            return Ok(i);
        }
        if let Some(i) = cols.iter().position(|c| c.eq_ignore_ascii_case(r)) {
            return Ok(i);
        }
    }
    match r.parse::<usize>() {
        Ok(n) if n >= 1 && n <= width.max(1) => Ok(n - 1),
        Ok(n) if n >= 1 => Err(format!(
            "column {n} is past the end of the table (it has {width} column(s))"
        )),
        Ok(_) => Err("column positions are 1-based (>= 1)".into()),
        Err(_) => {
            if header.is_some() {
                Err(format!(
                    "no column named '{r}' (use a header name or a 1-based position)"
                ))
            } else {
                Err(format!(
                    "'{r}' is not a 1-based column position — with has_header off, columns can only be referenced by number"
                ))
            }
        }
    }
}

/// Zero-pad the decimal digits of `n` to `width`, keeping a leading minus outside
/// the padding (`-7` at width 3 → `-007`).
fn pad_number(n: i64, width: usize) -> String {
    let neg = n < 0;
    let digits = n.unsigned_abs().to_string();
    let padded = if digits.len() >= width {
        digits
    } else {
        let mut s = String::with_capacity(width);
        for _ in 0..(width - digits.len()) {
            s.push('0');
        }
        s.push_str(&digits);
        s
    };
    if neg {
        format!("-{padded}")
    } else {
        padded
    }
}

fn format_uuid(mut b: [u8; 16], version: u8, style: &str) -> Result<String, String> {
    b[6] = (b[6] & 0x0f) | (version << 4);
    b[8] = (b[8] & 0x3f) | 0x80;
    let canonical = format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    );
    Ok(match style.trim().to_ascii_lowercase().as_str() {
        "" | "standard" | "lowercase" => canonical,
        "uppercase" => canonical.to_ascii_uppercase(),
        "compact" | "no-hyphens" => canonical.replace('-', ""),
        "braces" => format!("{{{canonical}}}"),
        "urn" => format!("urn:uuid:{canonical}"),
        other => {
            return Err(format!(
                "unknown uuid_format '{other}' (use standard, uppercase, compact, braces, or urn)"
            ))
        }
    })
}

/// Generate one UUID per data row, in row order. v7 embeds `now_ms + row_index`
/// in the first 48 bits so the produced IDs sort in row order within one run.
fn make_uuids(
    version: &str,
    count: usize,
    now_ms: u64,
    style: &str,
) -> Result<Vec<String>, String> {
    let v = version.trim();
    let v = if v.is_empty() { "4" } else { v };
    if !matches!(v, "4" | "v4" | "7" | "v7") {
        return Err(format!("unknown uuid_version '{v}' (use 4 or 7)"));
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let mut b = [0u8; 16];
        getrandom::getrandom(&mut b)
            .map_err(|e| format!("could not generate UUID randomness: {e}"))?;
        let version_nibble = if v == "7" || v == "v7" {
            let ts = now_ms.wrapping_add(i as u64) & 0x0000_ffff_ffff_ffff;
            b[0] = (ts >> 40) as u8;
            b[1] = (ts >> 32) as u8;
            b[2] = (ts >> 24) as u8;
            b[3] = (ts >> 16) as u8;
            b[4] = (ts >> 8) as u8;
            b[5] = ts as u8;
            7
        } else {
            4
        };
        out.push(format_uuid(b, version_nibble, style)?);
    }
    Ok(out)
}

/// Add a generated key column to a CSV table.
///
/// * `data` — the CSV/delimited table text (max [`MAX_INPUT_BYTES`]).
/// * `mode` — `sequential` | `uuid` | `composite`.
/// * `column_name` — header for the new column; blank picks `index`/`uuid`/`key`.
/// * `position` — `start` (default) | `end` | `before` | `after`.
/// * `reference_column` — the existing column `before`/`after` is relative to.
/// * `has_header` — treat row 1 as a header (it receives `column_name`).
/// * `start` / `step` — sequential numbering; `step` must not be 0.
/// * `pad_width` — zero-pad the sequential number to this many digits (0 = none).
/// * `prefix` / `suffix` — wrapped around every generated value, in every mode.
/// * `columns` — composite mode: comma-separated source columns (names or 1-based).
/// * `separator` — composite mode: the string joining those source values.
/// * `uuid_version` — `4` (random) or `7` (time-ordered).
/// * `uuid_format` — `standard` | `uppercase` | `compact` | `braces` | `urn`.
/// * `delimiter` — `auto` | `,` | `tab` | `;` | `|`; the output reuses it.
/// * `now_ms` — unix epoch in milliseconds, supplied by the calling surface (v7 only).
#[allow(clippy::too_many_arguments)]
pub fn add_index(
    data: &str,
    mode: &str,
    column_name: &str,
    position: &str,
    reference_column: &str,
    has_header: bool,
    start: i64,
    step: i64,
    pad_width: i64,
    prefix: &str,
    suffix: &str,
    columns: &str,
    separator: &str,
    uuid_version: &str,
    uuid_format: &str,
    delimiter: &str,
    now_ms: u64,
) -> Result<String, String> {
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {MAX_INPUT_BYTES} bytes",
            data.len()
        ));
    }
    if data.trim().is_empty() {
        return Err("input CSV is empty".into());
    }
    let mode = Mode::parse(mode)?;
    let position = Position::parse(position)?;
    if mode == Mode::Sequential && step == 0 {
        return Err("step must not be 0 (every row would get the same number)".into());
    }
    if !(0..=MAX_PAD_WIDTH).contains(&pad_width) {
        return Err(format!(
            "pad_width must be between 0 and {MAX_PAD_WIDTH}, got {pad_width}"
        ));
    }
    let pad = pad_width as usize;
    let delim = delim_byte(delimiter, data)?;

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let mut rows: Vec<Vec<String>> = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("could not parse CSV: {e}"))?;
        rows.push(rec.iter().map(|f| f.to_string()).collect());
    }
    if rows.is_empty() {
        return Err("input CSV has no rows".into());
    }

    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let header: Option<Vec<String>> = if has_header {
        Some(rows[0].clone())
    } else {
        None
    };
    let name = if column_name.trim().is_empty() {
        mode.default_column_name().to_string()
    } else {
        column_name.to_string()
    };
    if let Some(h) = header.as_ref() {
        if h.iter().any(|c| c == &name) {
            return Err(format!(
                "the table already has a column named '{name}' — pick a different column_name"
            ));
        }
    }

    // Where the new column is inserted, as a 0-based index into each row.
    let insert_at = match position {
        Position::Start => 0,
        Position::End => usize::MAX, // clamped per row below
        Position::Before | Position::After => {
            if reference_column.trim().is_empty() {
                return Err(format!(
                    "position '{}' needs reference_column set to an existing column",
                    if position == Position::Before {
                        "before"
                    } else {
                        "after"
                    }
                ));
            }
            let i = resolve_column(reference_column, header.as_deref(), width)?;
            if position == Position::Before {
                i
            } else {
                i + 1
            }
        }
    };

    // Composite mode: resolve the source columns once, against the header.
    let source_cols: Vec<usize> = if mode == Mode::Composite {
        let refs: Vec<&str> = columns
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if refs.is_empty() {
            return Err(
                "composite mode needs `columns` — a comma-separated list of the columns to join"
                    .into(),
            );
        }
        let mut seen = HashSet::new();
        let mut out = Vec::with_capacity(refs.len());
        for r in refs {
            let i = resolve_column(r, header.as_deref(), width)?;
            if !seen.insert(i) {
                return Err(format!("column '{r}' is listed twice in `columns`"));
            }
            out.push(i);
        }
        out
    } else {
        Vec::new()
    };

    // UUID mode: generate the whole column up front (one id per data row), which
    // also validates uuid_version/uuid_format before any output is written.
    let data_rows = rows.len() - usize::from(has_header);
    let uuids = if mode == Mode::Uuid {
        make_uuids(uuid_version, data_rows, now_ms, uuid_format)?
    } else {
        Vec::new()
    };

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(Vec::new());

    let mut counter = start;
    for (row_no, row) in rows.iter().enumerate() {
        let value = if has_header && row_no == 0 {
            name.clone()
        } else {
            let raw = match mode {
                Mode::Sequential => {
                    let v = pad_number(counter, pad);
                    counter = counter.checked_add(step).ok_or_else(|| {
                        "the sequence overflowed — reduce `start` or `step`".to_string()
                    })?;
                    v
                }
                Mode::Uuid => uuids[row_no - usize::from(has_header)].clone(),
                Mode::Composite => source_cols
                    .iter()
                    .map(|&i| row.get(i).map(String::as_str).unwrap_or(""))
                    .collect::<Vec<_>>()
                    .join(separator),
            };
            format!("{prefix}{raw}{suffix}")
        };

        let mut out: Vec<String> = row.clone();
        let at = insert_at.min(out.len());
        out.insert(at, value);
        wtr.write_record(&out)
            .map_err(|e| format!("could not write CSV: {e}"))?;
    }

    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("could not finish CSV: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("output was not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Defaults everywhere: sequential 1.. in a leading `index` column.
    fn seq(data: &str) -> Result<String, String> {
        add_index(
            data,
            "sequential",
            "",
            "start",
            "",
            true,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
    }

    #[test]
    fn sequential_prepends_numbered_column() {
        let out = seq("name,city\nada,london\nlin,taipei").unwrap();
        assert_eq!(out, "index,name,city\n1,ada,london\n2,lin,taipei\n");
    }

    #[test]
    fn start_and_step_follow_power_query_semantics() {
        let out = add_index(
            "a\nx\ny\nz",
            "sequential",
            "n",
            "start",
            "",
            true,
            0,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap();
        assert_eq!(out, "n,a\n0,x\n1,y\n2,z\n");

        let out = add_index(
            "a\nx\ny\nz",
            "sequential",
            "n",
            "start",
            "",
            true,
            10,
            5,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap();
        assert_eq!(out, "n,a\n10,x\n15,y\n20,z\n");
    }

    #[test]
    fn negative_step_counts_down_and_pads_after_the_sign() {
        let out = add_index(
            "a\nx\ny",
            "sequential",
            "n",
            "start",
            "",
            true,
            1,
            -4,
            3,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap();
        assert_eq!(out, "n,a\n001,x\n-003,y\n");
    }

    #[test]
    fn padding_prefix_and_suffix_build_an_invoice_number() {
        let out = add_index(
            "customer\nada\nlin",
            "sequential",
            "invoice",
            "start",
            "",
            true,
            1,
            1,
            4,
            "INV-",
            "-26",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap();
        assert_eq!(out, "invoice,customer\nINV-0001-26,ada\nINV-0002-26,lin\n");
    }

    #[test]
    fn headerless_input_numbers_every_row() {
        let out = add_index(
            "ada,london\nlin,taipei",
            "sequential",
            "",
            "start",
            "",
            false,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap();
        assert_eq!(out, "1,ada,london\n2,lin,taipei\n");
    }

    #[test]
    fn position_end_appends_and_before_after_target_a_named_column() {
        let d = "name,city\nada,london";
        let out = add_index(
            d,
            "sequential",
            "n",
            "end",
            "",
            true,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap();
        assert_eq!(out, "name,city,n\nada,london,1\n");

        let out = add_index(
            d,
            "sequential",
            "n",
            "before",
            "city",
            true,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap();
        assert_eq!(out, "name,n,city\nada,1,london\n");

        let out = add_index(
            d,
            "sequential",
            "n",
            "after",
            "name",
            true,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap();
        assert_eq!(out, "name,n,city\nada,1,london\n");
    }

    #[test]
    fn reference_column_accepts_a_1_based_position() {
        let out = add_index(
            "a,b\n1,2",
            "sequential",
            "n",
            "after",
            "2",
            true,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap();
        assert_eq!(out, "a,b,n\n1,2,1\n");
    }

    #[test]
    fn composite_joins_named_columns() {
        let out = add_index(
            "region,dept,spend\nEU,ops,10\nUS,eng,20",
            "composite",
            "key",
            "start",
            "",
            true,
            1,
            1,
            0,
            "",
            "",
            "region,dept",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap();
        assert_eq!(
            out,
            "key,region,dept,spend\nEU-ops,EU,ops,10\nUS-eng,US,eng,20\n"
        );
    }

    #[test]
    fn composite_uses_a_custom_separator_and_positions() {
        let out = add_index(
            "a,b\n1,2",
            "composite",
            "k",
            "end",
            "",
            true,
            1,
            1,
            0,
            "",
            "",
            "2,1",
            "::",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap();
        assert_eq!(out, "a,b,k\n1,2,2::1\n");
    }

    #[test]
    fn uuid_v4_is_unique_per_row_and_well_formed() {
        let out = add_index(
            "a\nx\ny\nz",
            "uuid",
            "",
            "start",
            "",
            true,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap();
        let lines: Vec<&str> = out.trim_end().lines().collect();
        assert_eq!(lines[0], "uuid,a");
        let ids: Vec<&str> = lines[1..]
            .iter()
            .map(|l| l.split(',').next().unwrap())
            .collect();
        assert_eq!(ids.len(), 3);
        for id in &ids {
            assert_eq!(id.len(), 36, "canonical hyphenated length");
            assert_eq!(&id[14..15], "4", "version nibble");
            assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
        }
        assert_eq!(ids.iter().collect::<HashSet<_>>().len(), 3, "all distinct");
    }

    #[test]
    fn uuid_v7_rows_sort_in_row_order() {
        let out = add_index(
            "a\nx\ny\nz",
            "uuid",
            "",
            "start",
            "",
            true,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "7",
            "standard",
            ",",
            1_700_000_000_000,
        )
        .unwrap();
        let ids: Vec<String> = out
            .trim_end()
            .lines()
            .skip(1)
            .map(|l| l.split(',').next().unwrap().to_string())
            .collect();
        assert_eq!(&ids[0][14..15], "7", "version nibble");
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "v7 ids sort into row order");
    }

    #[test]
    fn uuid_format_styles_all_render() {
        for (style, check) in [
            (
                "uppercase",
                (|s: &str| s.chars().all(|c| !c.is_ascii_lowercase())) as fn(&str) -> bool,
            ),
            (
                "compact",
                (|s: &str| s.len() == 32 && !s.contains('-')) as fn(&str) -> bool,
            ),
            (
                "braces",
                (|s: &str| s.starts_with('{') && s.ends_with('}')) as fn(&str) -> bool,
            ),
            (
                "urn",
                (|s: &str| s.starts_with("urn:uuid:")) as fn(&str) -> bool,
            ),
        ] {
            let out = add_index(
                "a\nx", "uuid", "id", "start", "", true, 1, 1, 0, "", "", "", "-", "4", style, ",",
                0,
            )
            .unwrap();
            let id = out.lines().nth(1).unwrap().split(',').next().unwrap();
            assert!(check(id), "style {style} produced {id}");
        }
    }

    #[test]
    fn tab_input_round_trips_its_delimiter() {
        let out = add_index(
            "a\tb\n1\t2",
            "sequential",
            "n",
            "start",
            "",
            true,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            "tab",
            0,
        )
        .unwrap();
        assert_eq!(out, "n\ta\tb\n1\t1\t2\n");
    }

    #[test]
    fn auto_detects_a_semicolon_table() {
        let out = add_index(
            "a;b\n1;2",
            "sequential",
            "n",
            "start",
            "",
            true,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            "auto",
            0,
        )
        .unwrap();
        assert_eq!(out, "n;a;b\n1;1;2\n");
    }

    #[test]
    fn quoted_fields_and_ragged_rows_are_preserved() {
        let out = add_index(
            "a,b\n\"x,1\",2\nlonely",
            "sequential",
            "n",
            "start",
            "",
            true,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap();
        assert_eq!(out, "n,a,b\n1,\"x,1\",2\n2,lonely\n");
    }

    #[test]
    fn position_end_on_a_short_row_still_appends_last() {
        let out = add_index(
            "a,b\nlonely",
            "sequential",
            "n",
            "end",
            "",
            true,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap();
        assert_eq!(out, "a,b,n\nlonely,1\n");
    }

    #[test]
    fn duplicate_column_name_is_rejected() {
        let err = add_index(
            "index,a\n1,x",
            "sequential",
            "index",
            "start",
            "",
            true,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap_err();
        assert!(err.contains("already has a column named"), "{err}");
    }

    #[test]
    fn unknown_reference_column_is_rejected() {
        let err = add_index(
            "a,b\n1,2",
            "sequential",
            "n",
            "before",
            "nope",
            true,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap_err();
        assert!(err.contains("no column named 'nope'"), "{err}");
    }

    #[test]
    fn before_without_a_reference_column_is_rejected() {
        let err = add_index(
            "a,b\n1,2",
            "sequential",
            "n",
            "before",
            "",
            true,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap_err();
        assert!(err.contains("needs reference_column"), "{err}");
    }

    #[test]
    fn composite_without_columns_is_rejected() {
        let err = add_index(
            "a,b\n1,2",
            "composite",
            "k",
            "start",
            "",
            true,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap_err();
        assert!(err.contains("needs `columns`"), "{err}");
    }

    #[test]
    fn zero_step_is_rejected() {
        let err = add_index(
            "a\nx",
            "sequential",
            "n",
            "start",
            "",
            true,
            1,
            0,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap_err();
        assert!(err.contains("step must not be 0"), "{err}");
    }

    #[test]
    fn bad_mode_pad_and_delimiter_are_rejected() {
        assert!(seq("a\nx").is_ok());
        let err = add_index(
            "a\nx", "bogus", "n", "start", "", true, 1, 1, 0, "", "", "", "-", "4", "standard",
            ",", 0,
        )
        .unwrap_err();
        assert!(err.contains("unknown mode"), "{err}");

        let err = add_index(
            "a\nx",
            "sequential",
            "n",
            "start",
            "",
            true,
            1,
            1,
            99,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            ",",
            0,
        )
        .unwrap_err();
        assert!(err.contains("pad_width"), "{err}");

        let err = add_index(
            "a\nx",
            "sequential",
            "n",
            "start",
            "",
            true,
            1,
            1,
            0,
            "",
            "",
            "",
            "-",
            "4",
            "standard",
            "!!",
            0,
        )
        .unwrap_err();
        assert!(err.contains("delimiter must be"), "{err}");
    }

    #[test]
    fn empty_input_is_rejected() {
        let err = seq("   \n").unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn oversize_input_is_rejected_at_the_cap() {
        let at_cap = "a\n".to_string() + &"x\n".repeat((MAX_INPUT_BYTES - 2) / 2);
        assert!(at_cap.len() <= MAX_INPUT_BYTES);
        assert!(seq(&at_cap).is_ok(), "exactly at the cap is accepted");

        let over = "a\n".to_string() + &"x".repeat(MAX_INPUT_BYTES);
        let err = seq(&over).unwrap_err();
        assert!(err.contains("the limit is"), "{err}");
    }
}
