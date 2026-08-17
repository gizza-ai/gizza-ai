//! csv-whitespace-normalizer core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps. Normalizes the whitespace *inside
//! every cell* of a delimited table: trims the ends, and — the part a plain
//! `trim()` cannot do — collapses or removes runs of whitespace in the middle of a
//! value, all inside a real RFC 4180 parse so quoting, embedded separators and
//! embedded newlines never break the record.
//!
//! The whitespace vocabulary defaults to the full Unicode `White_Space` set, so a
//! non-breaking space (U+00A0), a narrow NBSP (U+202F) or an ideographic space
//! (U+3000) pasted out of a spreadsheet is normalized like a plain space. Those are
//! exactly the characters that survive a copy-paste and then silently break a join.
//!
//! Nothing else about the table changes: the field separator round-trips, ragged
//! rows keep their length, cell *content* other than whitespace is copied
//! verbatim, and no row is added or dropped.

use std::collections::HashSet;

/// Hard cap on the pasted table, so a runaway paste can't wedge the tab.
pub const MAX_INPUT_BYTES: usize = 5_000_000;

/// Which end(s) of a cell get their whitespace stripped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Trim {
    /// Strip both ends (default).
    Both,
    /// Strip the start only — right-padding is meaningful in some fixed-width exports.
    Leading,
    /// Strip the end only.
    Trailing,
    /// Leave both edges alone; only the interior is touched.
    None,
}

/// What happens to whitespace *between* the first and last non-whitespace
/// character of a cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Internal {
    /// Every run becomes one plain space (default) — `"a   b"` → `"a b"`.
    Collapse,
    /// Every run is deleted — `"AB 12 CD"` → `"AB12CD"`, for IDs and part numbers.
    Remove,
    /// Interior spacing is preserved byte-for-byte.
    Keep,
}

/// Which characters count as whitespace.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WsSet {
    /// The full Unicode `White_Space` property: ASCII plus NBSP (U+00A0), narrow
    /// NBSP (U+202F), ideographic space (U+3000), the en/em space family, … (default).
    Unicode,
    /// ASCII only: space, tab, newline, carriage return, form feed. Leaves NBSP
    /// and friends untouched, for data where they are load-bearing.
    Ascii,
}

impl WsSet {
    fn is_ws(self, c: char) -> bool {
        match self {
            WsSet::Unicode => c.is_whitespace(),
            WsSet::Ascii => c.is_ascii_whitespace(),
        }
    }
}

fn parse_trim(s: &str) -> Result<Trim, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "both" => Ok(Trim::Both),
        "leading" => Ok(Trim::Leading),
        "trailing" => Ok(Trim::Trailing),
        "none" => Ok(Trim::None),
        other => Err(format!(
            "trim must be 'both', 'leading', 'trailing', or 'none', got '{other}'"
        )),
    }
}

fn parse_internal(s: &str) -> Result<Internal, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "collapse" => Ok(Internal::Collapse),
        "remove" => Ok(Internal::Remove),
        "keep" => Ok(Internal::Keep),
        other => Err(format!(
            "internal must be 'collapse', 'remove', or 'keep', got '{other}'"
        )),
    }
}

fn parse_ws_set(s: &str) -> Result<WsSet, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "unicode" => Ok(WsSet::Unicode),
        "ascii" => Ok(WsSet::Ascii),
        other => Err(format!(
            "whitespace must be 'unicode' or 'ascii', got '{other}'"
        )),
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
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be 'auto', a single character, or comma/tab/semicolon/pipe, got '{other}'"
                ));
            }
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
    // Comma first so it wins every tie.
    [(b',', comma), (b'\t', tab), (b';', semi), (b'|', pipe)]
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .max_by_key(|(_, n)| *n)
        .map(|(b, _)| b)
        .unwrap_or(b',')
}

/// Resolve the `columns` selector into the set of 0-based column indices to
/// normalize. An empty selector means "every column".
///
/// Accepts, comma-separated: 1-based positions (`2`), inclusive ranges (`2-4`),
/// and header names (`first name`). A token that parses as a position or a range
/// is read as one, so a header literally named `3` or `2-4` has to be selected by
/// its position instead.
fn parse_columns(
    spec: &str,
    header_row: Option<&[String]>,
    ncols: usize,
) -> Result<Option<HashSet<usize>>, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Ok(None);
    }
    let mut out = HashSet::new();
    for raw in spec.split(',') {
        let name = raw.trim();
        if name.is_empty() {
            continue;
        }
        // A bare positive integer is always a 1-based column position.
        if let Ok(idx) = name.parse::<usize>() {
            out.insert(check_pos(idx, ncols)? - 1);
            continue;
        }
        // `2-4` — an inclusive range of 1-based positions.
        if let Some(range) = parse_range(name, ncols)? {
            out.extend(range);
            continue;
        }
        let Some(h) = header_row else {
            return Err(format!(
                "column '{name}' is a name, but the header option is off — use 1-based column positions instead"
            ));
        };
        let found = h
            .iter()
            .position(|c| c.trim() == name)
            .or_else(|| h.iter().position(|c| c.trim().eq_ignore_ascii_case(name)));
        match found {
            Some(i) => {
                out.insert(i);
            }
            None => {
                return Err(format!(
                    "column '{name}' is not in the header (available: {})",
                    h.iter().map(|c| c.trim()).collect::<Vec<_>>().join(", ")
                ))
            }
        }
    }
    if out.is_empty() {
        return Err("columns selector resolved to no columns".to_string());
    }
    Ok(Some(out))
}

/// Validate a 1-based column position against the table's width.
fn check_pos(idx: usize, ncols: usize) -> Result<usize, String> {
    if idx == 0 {
        return Err("column positions are 1-based, got '0'".to_string());
    }
    if idx > ncols {
        return Err(format!(
            "column position {idx} is out of range (the table has {ncols} columns)"
        ));
    }
    Ok(idx)
}

/// `2-4` → the 0-based indices 1,2,3. `Ok(None)` when the token isn't a range at
/// all (so it can still be tried as a header name).
fn parse_range(token: &str, ncols: usize) -> Result<Option<Vec<usize>>, String> {
    let Some((lo, hi)) = token.split_once('-') else {
        return Ok(None);
    };
    let (lo, hi) = (lo.trim(), hi.trim());
    let (Ok(lo), Ok(hi)) = (lo.parse::<usize>(), hi.parse::<usize>()) else {
        return Ok(None);
    };
    let lo = check_pos(lo, ncols)?;
    let hi = check_pos(hi, ncols)?;
    if lo > hi {
        return Err(format!(
            "column range '{token}' runs backwards — write it low-to-high, e.g. '{hi}-{lo}'"
        ));
    }
    Ok(Some((lo - 1..hi).collect()))
}

/// Apply the whitespace pass to a single cell.
///
/// The cell is split into a leading whitespace run, a core (first through last
/// non-whitespace character), and a trailing run. `internal` rewrites the core;
/// `trim` decides whether each edge run survives, and an edge run that survives is
/// copied verbatim. A cell that is entirely whitespace is emptied by every `trim`
/// setting except `none`.
fn normalize_cell(cell: &str, trim: Trim, internal: Internal, ws: WsSet) -> String {
    let Some(start) = cell.char_indices().find(|(_, c)| !ws.is_ws(*c)).map(|(i, _)| i) else {
        // Empty, or nothing but whitespace: there is no interior to rewrite.
        return if trim == Trim::None {
            cell.to_string()
        } else {
            String::new()
        };
    };
    let end = cell
        .char_indices()
        .rev()
        .find(|(_, c)| !ws.is_ws(*c))
        .map(|(i, c)| i + c.len_utf8())
        .expect("a non-whitespace char was found forwards, so one exists backwards");

    let core = match internal {
        Internal::Keep => cell[start..end].to_string(),
        Internal::Remove => cell[start..end].chars().filter(|c| !ws.is_ws(*c)).collect(),
        Internal::Collapse => {
            let mut s = String::with_capacity(end - start);
            let mut in_run = false;
            for c in cell[start..end].chars() {
                if ws.is_ws(c) {
                    if !in_run {
                        s.push(' ');
                        in_run = true;
                    }
                } else {
                    s.push(c);
                    in_run = false;
                }
            }
            s
        }
    };

    let keep_lead = matches!(trim, Trim::None | Trim::Trailing);
    let keep_trail = matches!(trim, Trim::None | Trim::Leading);
    let mut out = String::with_capacity(cell.len());
    if keep_lead {
        out.push_str(&cell[..start]);
    }
    out.push_str(&core);
    if keep_trail {
        out.push_str(&cell[end..]);
    }
    out
}

/// Normalize the whitespace inside every cell of a CSV/delimited table.
///
/// * `data` — the table text.
/// * `delimiter` — `auto`, a single character, or `comma`/`tab`/`semicolon`/`pipe`.
/// * `trim` — `both` (default), `leading`, `trailing`, or `none`.
/// * `internal` — `collapse` (default), `remove`, or `keep`.
/// * `whitespace` — `unicode` (default, the full `White_Space` set) or `ascii`.
/// * `columns` — comma-separated header names, 1-based positions and `2-4` ranges;
///   empty = every column.
/// * `header` — the first row is a header (it supplies the names for `columns`).
/// * `normalize_header` — also apply the pass to the header cells (default on).
#[allow(clippy::too_many_arguments)]
pub fn normalize(
    data: &str,
    delimiter: &str,
    trim: &str,
    internal: &str,
    whitespace: &str,
    columns: &str,
    header: bool,
    normalize_header: bool,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty — paste a CSV table with at least one row".to_string());
    }
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, which exceeds the {MAX_INPUT_BYTES}-byte limit",
            data.len()
        ));
    }

    let delim = delim_byte(delimiter, data)?;
    let trim = parse_trim(trim)?;
    let internal = parse_internal(internal)?;
    let ws = parse_ws_set(whitespace)?;

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .has_headers(false)
        .from_reader(data.as_bytes());

    let mut rows: Vec<Vec<String>> = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("CSV parse error: {e}"))?;
        rows.push(rec.iter().map(|f| f.to_string()).collect());
    }
    if rows.is_empty() {
        return Err("input is empty — paste a CSV table with at least one row".to_string());
    }

    let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let header_row = if header { Some(rows[0].clone()) } else { None };
    let selected = parse_columns(columns, header_row.as_deref(), ncols)?;

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .quote_style(csv::QuoteStyle::Necessary)
        .flexible(true)
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());

    for (r, row) in rows.iter().enumerate() {
        let is_header = header && r == 0;
        if is_header && !normalize_header {
            wtr.write_record(row)
                .map_err(|e| format!("CSV write error: {e}"))?;
            continue;
        }
        let out: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(c, cell)| {
                let in_scope = selected.as_ref().is_none_or(|s| s.contains(&c));
                if in_scope {
                    normalize_cell(cell, trim, internal, ws)
                } else {
                    cell.clone()
                }
            })
            .collect();
        wtr.write_record(&out)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }

    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("output is not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Defaults: comma, trim both ends, collapse the interior, Unicode whitespace,
    /// every column, header present and normalized.
    fn run(data: &str) -> Result<String, String> {
        normalize(data, ",", "both", "collapse", "unicode", "", true, true)
    }

    #[test]
    fn trims_the_ends_and_collapses_the_interior() {
        let got = run("name,city\n  Ada   Lovelace  ,  New    York \n").unwrap();
        assert_eq!(got, "name,city\nAda Lovelace,New York\n");
    }

    #[test]
    fn interior_keep_is_a_pure_trim() {
        let got = normalize(
            "a,b\n  x   y  ,  z \n",
            ",",
            "both",
            "keep",
            "unicode",
            "",
            true,
            true,
        )
        .unwrap();
        assert_eq!(got, "a,b\nx   y,z\n");
    }

    #[test]
    fn interior_remove_strips_every_inner_space() {
        let got = normalize(
            "sku,label\n  AB 12  CD ,keep me\n",
            ",",
            "both",
            "remove",
            "unicode",
            "",
            true,
            true,
        )
        .unwrap();
        assert_eq!(got, "sku,label\nAB12CD,keepme\n");
    }

    #[test]
    fn trim_leading_and_trailing_are_one_sided() {
        let lead = normalize("a\n  x  \n", ",", "leading", "keep", "unicode", "", true, true).unwrap();
        assert_eq!(lead, "a\nx  \n");
        let trail =
            normalize("a\n  x  \n", ",", "trailing", "keep", "unicode", "", true, true).unwrap();
        assert_eq!(trail, "a\n  x\n");
    }

    #[test]
    fn trim_none_leaves_the_edges_and_only_touches_the_middle() {
        let got = normalize(
            "a\n  x    y  \n",
            ",",
            "none",
            "collapse",
            "unicode",
            "",
            true,
            true,
        )
        .unwrap();
        assert_eq!(got, "a\n  x y  \n");
    }

    #[test]
    fn trim_none_with_keep_round_trips_the_table() {
        let src = "a,b\n  x   y  ,\" z \"\n";
        let got = normalize(src, ",", "none", "keep", "unicode", "", true, true).unwrap();
        // The quoted cell no longer needs quoting once nothing changed inside it,
        // so minimal quoting drops the quotes; the bytes of the value are identical.
        assert_eq!(got, "a,b\n  x   y  , z \n");
    }

    #[test]
    fn whitespace_only_cells_become_empty_unless_trim_is_none() {
        let got = run("a,b\n   ,x\n").unwrap();
        assert_eq!(got, "a,b\n,x\n");
        let kept = normalize(
            "a,b\n\"   \",x\n",
            ",",
            "none",
            "collapse",
            "unicode",
            "",
            true,
            true,
        )
        .unwrap();
        assert_eq!(kept, "a,b\n   ,x\n");
    }

    // --- the Unicode differentiator ---

    #[test]
    fn unicode_mode_normalizes_nbsp_and_ideographic_space() {
        // U+00A0 padding, U+3000 in the middle, U+202F narrow NBSP padding.
        let got = run("name\n\u{a0}Ada\u{3000}\u{3000}Lovelace\u{202f}\n").unwrap();
        assert_eq!(got, "name\nAda Lovelace\n");
    }

    #[test]
    fn ascii_mode_leaves_nbsp_alone() {
        let got = normalize(
            "name\n \u{a0}Ada  Lovelace \n",
            ",",
            "both",
            "collapse",
            "ascii",
            "",
            true,
            true,
        )
        .unwrap();
        // The NBSP is not whitespace here, so it is content: it survives, and it
        // also stops the leading ASCII space from being adjacent to "Ada".
        assert_eq!(got, "name\n\u{a0}Ada Lovelace\n");
    }

    #[test]
    fn zero_width_characters_are_not_whitespace() {
        // U+200B ZERO WIDTH SPACE has White_Space=No — a different tool's job.
        let got = run("a\n x\u{200b}y \n").unwrap();
        assert_eq!(got, "a\nx\u{200b}y\n");
    }

    // --- header handling ---

    #[test]
    fn header_is_normalized_by_default() {
        let got = run("  first name  ,b\n  1 ,2\n").unwrap();
        assert_eq!(got, "first name,b\n1,2\n");
    }

    #[test]
    fn normalize_header_off_preserves_the_header_bytes() {
        let got = normalize(
            "  first  name  ,b\n  1 ,2\n",
            ",",
            "both",
            "collapse",
            "unicode",
            "",
            true,
            false,
        )
        .unwrap();
        assert_eq!(got, "  first  name  ,b\n1,2\n");
    }

    #[test]
    fn header_off_treats_row_one_as_data() {
        let got = normalize("  a  ,b\n c ,d\n", ",", "both", "collapse", "unicode", "", false, false)
            .unwrap();
        assert_eq!(got, "a,b\nc,d\n");
    }

    // --- column scoping ---

    #[test]
    fn columns_by_name_scopes_the_pass() {
        let got = normalize(
            "a,b\n  x  ,  y  \n",
            ",",
            "both",
            "collapse",
            "unicode",
            "b",
            true,
            true,
        )
        .unwrap();
        assert_eq!(got, "a,b\n  x  ,y\n");
    }

    #[test]
    fn columns_accept_positions_and_ranges() {
        let got = normalize(
            "a,b,c,d\n 1 , 2 , 3 , 4 \n",
            ",",
            "both",
            "collapse",
            "unicode",
            "2-3",
            true,
            true,
        )
        .unwrap();
        assert_eq!(got, "a,b,c,d\n 1 ,2,3, 4 \n");

        let mixed = normalize(
            "a,b,c,d\n 1 , 2 , 3 , 4 \n",
            ",",
            "both",
            "collapse",
            "unicode",
            "1,3-4",
            true,
            true,
        )
        .unwrap();
        assert_eq!(mixed, "a,b,c,d\n1, 2 ,3,4\n");
    }

    #[test]
    fn scoped_columns_still_skip_the_header_when_asked() {
        let got = normalize(
            " a , b \n 1 , 2 \n",
            ",",
            "both",
            "collapse",
            "unicode",
            "2",
            true,
            false,
        )
        .unwrap();
        assert_eq!(got, " a , b \n 1 ,2\n");
    }

    // --- CSV grammar ---

    #[test]
    fn embedded_separators_and_newlines_survive_the_parse() {
        let got = normalize(
            "a,b\n\"x,  y\",\"line1\nline2\"\n",
            ",",
            "both",
            "keep",
            "unicode",
            "",
            true,
            true,
        )
        .unwrap();
        assert_eq!(got, "a,b\n\"x,  y\",\"line1\nline2\"\n");
    }

    #[test]
    fn collapse_folds_an_embedded_newline_into_one_space() {
        let got = run("a,b\n\"line1\nline2\",x\n").unwrap();
        assert_eq!(got, "a,b\nline1 line2,x\n");
    }

    #[test]
    fn ragged_rows_keep_their_length() {
        let got = run("a,b,c\n 1 , 2 \n 3 \n").unwrap();
        assert_eq!(got, "a,b,c\n1,2\n3\n");
    }

    #[test]
    fn tab_and_auto_delimiters_round_trip() {
        let tsv = "a\tb\n  x  \t  y  \n";
        let by_name = normalize(tsv, "tab", "both", "collapse", "unicode", "", true, true).unwrap();
        let by_sniff = normalize(tsv, "auto", "both", "collapse", "unicode", "", true, true).unwrap();
        assert_eq!(by_name, "a\tb\nx\ty\n");
        assert_eq!(by_sniff, by_name);
    }

    #[test]
    fn semicolon_and_pipe_sniffing() {
        assert_eq!(
            normalize("a;b\n x ; y \n", "auto", "both", "collapse", "unicode", "", true, true)
                .unwrap(),
            "a;b\nx;y\n"
        );
        assert_eq!(
            normalize("a|b\n x | y \n", "auto", "both", "collapse", "unicode", "", true, true)
                .unwrap(),
            "a|b\nx|y\n"
        );
    }

    #[test]
    fn a_cell_that_still_needs_quoting_keeps_its_quotes() {
        // Collapsing does not remove the comma, so the field stays quoted.
        let got = run("a\n\"  x ,  y  \"\n").unwrap();
        assert_eq!(got, "a\n\"x , y\"\n");
    }

    // --- errors ---

    #[test]
    fn empty_input_is_an_error() {
        let err = run("   \n").unwrap_err();
        assert!(err.contains("input is empty"), "got: {err}");
    }

    #[test]
    fn unknown_column_name_is_an_error_listing_the_header() {
        let err = normalize(
            "a,b\n1,2\n",
            ",",
            "both",
            "collapse",
            "unicode",
            "zzz",
            true,
            true,
        )
        .unwrap_err();
        assert!(err.contains("'zzz' is not in the header"), "got: {err}");
        assert!(err.contains("available: a, b"), "got: {err}");
    }

    #[test]
    fn column_name_without_a_header_is_an_error() {
        let err = normalize(
            "1,2\n",
            ",",
            "both",
            "collapse",
            "unicode",
            "score",
            false,
            false,
        )
        .unwrap_err();
        assert!(err.contains("header option is off"), "got: {err}");
    }

    #[test]
    fn out_of_range_positions_and_ranges_are_errors() {
        let err = normalize("a,b\n1,2\n", ",", "both", "collapse", "unicode", "5", true, true)
            .unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
        assert!(err.contains("2 columns"), "got: {err}");

        let err = normalize("a,b\n1,2\n", ",", "both", "collapse", "unicode", "1-9", true, true)
            .unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
    }

    #[test]
    fn zero_and_backwards_ranges_are_errors() {
        let err = normalize("a,b\n1,2\n", ",", "both", "collapse", "unicode", "0", true, true)
            .unwrap_err();
        assert!(err.contains("1-based"), "got: {err}");

        let err = normalize("a,b\n1,2\n", ",", "both", "collapse", "unicode", "2-1", true, true)
            .unwrap_err();
        assert!(err.contains("runs backwards"), "got: {err}");
    }

    #[test]
    fn bad_mode_values_are_errors() {
        let e = normalize("a\n1\n", ",", "sides", "collapse", "unicode", "", true, true).unwrap_err();
        assert!(e.contains("trim must be"), "got: {e}");
        let e = normalize("a\n1\n", ",", "both", "squash", "unicode", "", true, true).unwrap_err();
        assert!(e.contains("internal must be"), "got: {e}");
        let e = normalize("a\n1\n", ",", "both", "collapse", "utf8", "", true, true).unwrap_err();
        assert!(e.contains("whitespace must be"), "got: {e}");
        let e = normalize("a\n1\n", "commas", "both", "collapse", "unicode", "", true, true)
            .unwrap_err();
        assert!(e.contains("delimiter must be"), "got: {e}");
    }

    #[test]
    fn input_size_cap_is_exact() {
        // One row of "a\n" repeated; at the cap it succeeds, one byte over it fails.
        let at_cap = "a\n".repeat(MAX_INPUT_BYTES / 2);
        assert_eq!(at_cap.len(), MAX_INPUT_BYTES);
        assert!(run(&at_cap).is_ok());

        let over = format!("{at_cap}b");
        assert_eq!(over.len(), MAX_INPUT_BYTES + 1);
        let err = run(&over).unwrap_err();
        assert!(err.contains("exceeds the"), "got: {err}");
    }
}
