//! obj-vertices-to-csv core — pure compute, shared by the chat skill block and
//! the web page.
//!
//! Extracts the vertex (`v`) coordinates of a Wavefront OBJ document into a flat
//! `x,y,z` CSV, one row per vertex, in file order. Faces (`f`), texture
//! coordinates (`vt`), normals (`vn`), parameter-space vertices (`vp`), curves
//! and `.mtl` material definitions are ignored — only the stored positions come
//! out. Nothing is resampled: a row exists for each `v` line that survives the
//! filters.
//!
//! The extended `v x y z r g b` form (per-vertex colours, written by scanners
//! and by MeshLab) is understood, and the optional 4th `w` value on a `v` line
//! (rational-curve weight, defaults to 1) is parsed and ignored.

use std::collections::HashSet;

/// Largest OBJ document accepted (bytes).
pub const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
/// Largest number of CSV rows emitted.
pub const MAX_ROWS: usize = 500_000;
/// How many distinct object/group names an unmatched-filter error lists.
const MAX_LISTED_NAMES: usize = 20;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Columns {
    Xyz,
    Indexed,
    Object,
    Full,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UpAxis {
    Keep,
    YToZ,
    ZToY,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dedupe {
    None,
    Adjacent,
    All,
}

fn parse_columns(s: &str) -> Result<Columns, String> {
    match s.trim() {
        "" | "xyz" => Ok(Columns::Xyz),
        "indexed" => Ok(Columns::Indexed),
        "object" => Ok(Columns::Object),
        "full" => Ok(Columns::Full),
        other => Err(format!(
            "unknown columns '{other}': expected 'xyz', 'indexed', 'object' or 'full'"
        )),
    }
}

fn parse_color(s: &str) -> Result<ColorMode, String> {
    match s.trim() {
        "" | "auto" => Ok(ColorMode::Auto),
        "always" => Ok(ColorMode::Always),
        "never" => Ok(ColorMode::Never),
        other => Err(format!(
            "unknown color '{other}': expected 'auto', 'always' or 'never'"
        )),
    }
}

fn parse_up_axis(s: &str) -> Result<UpAxis, String> {
    match s.trim() {
        "" | "keep" => Ok(UpAxis::Keep),
        "y-to-z" => Ok(UpAxis::YToZ),
        "z-to-y" => Ok(UpAxis::ZToY),
        other => Err(format!(
            "unknown up_axis '{other}': expected 'keep', 'y-to-z' or 'z-to-y'"
        )),
    }
}

fn parse_dedupe(s: &str) -> Result<Dedupe, String> {
    match s.trim() {
        "" | "none" => Ok(Dedupe::None),
        "adjacent" => Ok(Dedupe::Adjacent),
        "all" => Ok(Dedupe::All),
        other => Err(format!(
            "unknown dedupe '{other}': expected 'none', 'adjacent' or 'all'"
        )),
    }
}

fn parse_delimiter(s: &str) -> Result<char, String> {
    match s.trim() {
        "" | "comma" => Ok(','),
        "semicolon" => Ok(';'),
        "tab" => Ok('\t'),
        "pipe" => Ok('|'),
        "space" => Ok(' '),
        other => Err(format!(
            "unknown delimiter '{other}': expected 'comma', 'semicolon', 'tab', 'pipe' or 'space'"
        )),
    }
}

/// Fold OBJ line continuations (`\` at end of line), strip `#` comments, and
/// hand each logical line to `f` with the 1-based number of the line it started
/// on. Blank/comment-only lines are skipped.
fn for_each_logical_line(
    input: &str,
    mut f: impl FnMut(usize, &str) -> Result<(), String>,
) -> Result<(), String> {
    let mut pending: Option<(usize, String)> = None;
    for (i, raw) in input.lines().enumerate() {
        let lineno = i + 1;
        // `#` starts a comment that runs to the end of the physical line.
        let body = match raw.find('#') {
            Some(pos) => &raw[..pos],
            None => raw,
        };
        let trimmed = body.trim_end();
        let (content, continued) = match trimmed.strip_suffix('\\') {
            Some(head) => (head, true),
            None => (trimmed, false),
        };
        match pending.as_mut() {
            Some((_, buf)) => {
                buf.push(' ');
                buf.push_str(content.trim());
            }
            None => {
                if continued {
                    pending = Some((lineno, content.trim().to_string()));
                    continue;
                }
                let line = content.trim();
                if !line.is_empty() {
                    f(lineno, line)?;
                }
                continue;
            }
        }
        if !continued {
            let (start, buf) = pending.take().expect("pending is Some in this branch");
            let line = buf.trim();
            if !line.is_empty() {
                f(start, line)?;
            }
        }
    }
    if let Some((start, buf)) = pending {
        let line = buf.trim();
        if !line.is_empty() {
            f(start, line)?;
        }
    }
    Ok(())
}

/// Parse the numeric fields of a `v` line into (x, y, z) plus optional r/g/b
/// source tokens. Accepts `x y z`, `x y z w`, `x y z r g b` and `x y z w r g b`.
fn parse_vertex<'a>(
    lineno: usize,
    rest: &'a str,
) -> Result<([&'a str; 3], [f64; 3], Option<[&'a str; 3]>), String> {
    let toks: Vec<&str> = rest.split_whitespace().collect();
    let color_at = match toks.len() {
        3 | 4 => None,
        6 => Some(3),
        7 => Some(4),
        n => {
            return Err(format!(
                "line {lineno}: `v` needs 3 coordinates (x y z), optionally a 4th w value and/or \
                 3 colour values (r g b) — got {n} value(s)"
            ))
        }
    };
    let mut vals = [0f64; 3];
    for (i, tok) in toks[..3].iter().enumerate() {
        let v: f64 = tok.parse().map_err(|_| {
            format!("line {lineno}: invalid coordinate '{tok}': expected a number (OBJ `v x y z`)")
        })?;
        if !v.is_finite() {
            return Err(format!(
                "line {lineno}: coordinate '{tok}' is not a finite number"
            ));
        }
        vals[i] = v;
    }
    let raw = [toks[0], toks[1], toks[2]];
    let color = color_at.map(|at| [toks[at], toks[at + 1], toks[at + 2]]);
    // A 4th `w` value (and the colour channels) must still be numeric to count
    // as a vertex line — reject garbage rather than silently mis-parsing.
    for tok in &toks[3..] {
        if tok.parse::<f64>().is_err() {
            return Err(format!(
                "line {lineno}: invalid value '{tok}' on a `v` line: expected a number"
            ));
        }
    }
    Ok((raw, vals, color))
}

/// Shortest round-trip formatting, with `-0` normalized to `0`.
fn fmt_shortest(v: f64) -> String {
    let s = format!("{v}");
    if s == "-0" {
        "0".to_string()
    } else {
        s
    }
}

fn fmt_fixed(v: f64, precision: i32) -> String {
    let s = format!("{:.*}", precision as usize, v);
    // `-0.000` is noise; render a zero as unsigned.
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s[1..].to_string()
    } else {
        s
    }
}

fn csv_field(value: &str, delim: char) -> String {
    if value.contains(delim) || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Convert a Wavefront OBJ document into a CSV of its vertex coordinates.
///
/// * `columns` — `xyz` | `indexed` | `object` | `full`
/// * `color` — `auto` | `always` | `never` (per-vertex `r,g,b` columns)
/// * `up_axis` — `keep` | `y-to-z` | `z-to-y`
/// * `scale` — multiplier applied to every coordinate
/// * `precision` — `-1` keeps the source text, `0..=15` rounds
/// * `dedupe` — `none` | `adjacent` | `all`
/// * `objects` — comma-separated `o`/`g` names to keep (empty = all)
/// * `delimiter` — `comma` | `semicolon` | `tab` | `pipe` | `space`
/// * `header` — emit the column-name row
#[allow(clippy::too_many_arguments)]
pub fn convert_str(
    obj: &str,
    columns: &str,
    color: &str,
    up_axis: &str,
    scale: f64,
    precision: i32,
    dedupe: &str,
    objects: &str,
    delimiter: &str,
    header: bool,
) -> Result<String, String> {
    let columns = parse_columns(columns)?;
    let color = parse_color(color)?;
    let up_axis = parse_up_axis(up_axis)?;
    let dedupe = parse_dedupe(dedupe)?;
    let delim = parse_delimiter(delimiter)?;
    if !(-1..=15).contains(&precision) {
        return Err(format!(
            "invalid precision {precision}: expected -1 (keep the source value) or 0-15 decimal \
             places"
        ));
    }
    if !scale.is_finite() {
        return Err("invalid scale: expected a finite number (1 keeps the source size)".to_string());
    }
    if obj.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "OBJ is too large: {} bytes exceeds the {MAX_INPUT_BYTES} byte limit",
            obj.len()
        ));
    }
    if obj.trim().is_empty() {
        return Err(
            "empty input: paste Wavefront OBJ text (vertex lines look like `v 1.0 2.0 3.0`)"
                .to_string(),
        );
    }

    let wanted: Vec<&str> = objects
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    // Pass 1 — does any vertex carry colour? (decides the header before rows).
    let mut any_color = false;
    let mut any_vertex = false;
    for_each_logical_line(obj, |lineno, line| {
        if let Some(rest) = strip_keyword(line, "v") {
            any_vertex = true;
            let (_, _, c) = parse_vertex(lineno, rest)?;
            any_color |= c.is_some();
        }
        Ok(())
    })?;
    if !any_vertex {
        return Err(hint_no_vertices(obj));
    }
    let write_color = match color {
        ColorMode::Auto => any_color,
        ColorMode::Always => true,
        ColorMode::Never => false,
    };

    // Pass 2 — emit the rows.
    let transform = up_axis != UpAxis::Keep || scale != 1.0;
    let mut rows: Vec<String> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut last_key: Option<String> = None;
    let mut object = String::new();
    let mut group = String::new();
    let mut material = String::new();
    let mut index: usize = 0;

    for_each_logical_line(obj, |lineno, line| {
        if let Some(rest) = strip_keyword(line, "o") {
            object = rest.trim().to_string();
            remember(&mut names, &object);
            return Ok(());
        }
        if let Some(rest) = strip_keyword(line, "g") {
            group = rest.trim().to_string();
            for tok in group.split_whitespace() {
                remember(&mut names, tok);
            }
            return Ok(());
        }
        if let Some(rest) = strip_keyword(line, "usemtl") {
            material = rest.trim().to_string();
            return Ok(());
        }
        let Some(rest) = strip_keyword(line, "v") else {
            return Ok(());
        };

        // OBJ `f` indices are 1-based and count every `v` line in the file, so
        // the index is assigned before any filtering.
        index += 1;
        let (raw, mut vals, src_color) = parse_vertex(lineno, rest)?;

        if !wanted.is_empty() {
            let hit = wanted.iter().any(|w| {
                *w == object.as_str() || group.split_whitespace().any(|tok| tok == *w)
            });
            if !hit {
                return Ok(());
            }
        }

        if up_axis == UpAxis::YToZ {
            // Y-up (OBJ) → Z-up: rotate +90° about X.
            vals = [vals[0], -vals[2], vals[1]];
        } else if up_axis == UpAxis::ZToY {
            // Z-up → Y-up (OBJ): rotate −90° about X.
            vals = [vals[0], vals[2], -vals[1]];
        }
        if scale != 1.0 {
            vals = [vals[0] * scale, vals[1] * scale, vals[2] * scale];
        }

        let coords: [String; 3] = if !transform && precision < 0 {
            [raw[0].to_string(), raw[1].to_string(), raw[2].to_string()]
        } else if precision < 0 {
            [
                fmt_shortest(vals[0]),
                fmt_shortest(vals[1]),
                fmt_shortest(vals[2]),
            ]
        } else {
            [
                fmt_fixed(vals[0], precision),
                fmt_fixed(vals[1], precision),
                fmt_fixed(vals[2], precision),
            ]
        };

        if dedupe != Dedupe::None {
            let key = format!("{}\u{0}{}\u{0}{}", coords[0], coords[1], coords[2]);
            match dedupe {
                Dedupe::Adjacent => {
                    if last_key.as_deref() == Some(key.as_str()) {
                        return Ok(());
                    }
                    last_key = Some(key);
                }
                Dedupe::All => {
                    if !seen.insert(key) {
                        return Ok(());
                    }
                }
                Dedupe::None => unreachable!(),
            }
        }

        let mut fields: Vec<String> = Vec::with_capacity(10);
        match columns {
            Columns::Xyz => {}
            Columns::Indexed => fields.push(index.to_string()),
            Columns::Object => {
                fields.push(index.to_string());
                fields.push(csv_field(&object, delim));
                fields.push(csv_field(&group, delim));
            }
            Columns::Full => {
                fields.push(index.to_string());
                fields.push(csv_field(&object, delim));
                fields.push(csv_field(&group, delim));
                fields.push(csv_field(&material, delim));
            }
        }
        fields.extend(coords);
        if write_color {
            match src_color {
                Some(c) => fields.extend(c.iter().map(|s| s.to_string())),
                None => fields.extend(["".to_string(), "".to_string(), "".to_string()]),
            }
        }
        rows.push(fields.join(&delim.to_string()));
        if rows.len() > MAX_ROWS {
            return Err(format!(
                "too many vertices: the output would exceed the {MAX_ROWS} row limit — filter with \
                 `objects`, dedupe, or split the model"
            ));
        }
        Ok(())
    })?;

    if rows.is_empty() {
        if !wanted.is_empty() {
            let mut listed: Vec<String> = names.iter().take(MAX_LISTED_NAMES).cloned().collect();
            if names.len() > MAX_LISTED_NAMES {
                listed.push("…".to_string());
            }
            let available = if listed.is_empty() {
                "the OBJ declares no `o`/`g` sections".to_string()
            } else {
                format!("available: {}", listed.join(", "))
            };
            return Err(format!(
                "no vertices matched objects '{}' ({available})",
                objects.trim()
            ));
        }
        return Err(hint_no_vertices(obj));
    }

    let mut out = String::with_capacity(rows.iter().map(|r| r.len() + 1).sum());
    if header {
        let mut cols: Vec<&str> = Vec::with_capacity(10);
        match columns {
            Columns::Xyz => {}
            Columns::Indexed => cols.push("index"),
            Columns::Object => cols.extend(["index", "object", "group"]),
            Columns::Full => cols.extend(["index", "object", "group", "material"]),
        }
        cols.extend(["x", "y", "z"]);
        if write_color {
            cols.extend(["red", "green", "blue"]);
        }
        out.push_str(&cols.join(&delim.to_string()));
        out.push('\n');
    }
    out.push_str(&rows.join("\n"));
    Ok(out)
}

/// `line` starts with `keyword` followed by whitespace → the remainder.
fn strip_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(keyword)?;
    match rest.chars().next() {
        Some(c) if c.is_whitespace() => Some(rest),
        _ => None,
    }
}

fn remember(names: &mut Vec<String>, name: &str) {
    if name.is_empty() || names.iter().any(|n| n == name) || names.len() >= 200 {
        return;
    }
    names.push(name.to_string());
}

/// A pointed error for input that parsed but held no `v` lines.
fn hint_no_vertices(obj: &str) -> String {
    let lower = obj.trim_start().to_ascii_lowercase();
    if lower.starts_with("solid") || lower.contains("facet normal") {
        return "no vertex (`v`) lines found: this looks like ASCII STL, not Wavefront OBJ — \
                convert it to OBJ first (see the mesh converter)"
            .to_string();
    }
    if lower.starts_with('{') || lower.starts_with('[') {
        return "no vertex (`v`) lines found: this looks like JSON, not Wavefront OBJ text"
            .to_string();
    }
    "no vertex (`v`) lines found: Wavefront OBJ vertices look like `v 1.0 2.0 3.0`, one per line"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE_CORNER: &str = "# a corner\nv 0.0 0.0 0.0\nv 1.0 0.0 0.0\nv 0.0 1.0 0.0\nf 1 2 3\n";

    fn convert(obj: &str) -> Result<String, String> {
        convert_str(obj, "xyz", "auto", "keep", 1.0, -1, "none", "", "comma", true)
    }

    #[test]
    fn extracts_vertices_with_header() {
        assert_eq!(
            convert(CUBE_CORNER).unwrap(),
            "x,y,z\n0.0,0.0,0.0\n1.0,0.0,0.0\n0.0,1.0,0.0"
        );
    }

    #[test]
    fn keeps_source_text_verbatim_by_default() {
        // 1.500 must not be reformatted to 1.5 when precision is -1.
        let out = convert("v 1.500 -2 3e2\n").unwrap();
        assert_eq!(out, "x,y,z\n1.500,-2,3e2");
    }

    #[test]
    fn ignores_faces_normals_and_texcoords() {
        let obj = "v 1 2 3\nvt 0.5 0.5\nvn 0 1 0\nvp 0.1\nf 1/1/1 1/1/1 1/1/1\n";
        assert_eq!(convert(obj).unwrap(), "x,y,z\n1,2,3");
    }

    #[test]
    fn empty_input_errors() {
        let err = convert("   \n").unwrap_err();
        assert!(err.contains("empty input"), "{err}");
    }

    #[test]
    fn malformed_vertex_errors_with_line_number() {
        let err = convert("v 1 2 3\nv 4 5\n").unwrap_err();
        assert!(err.contains("line 2"), "{err}");
        assert!(err.contains("got 2 value(s)"), "{err}");
    }

    #[test]
    fn non_numeric_coordinate_errors() {
        let err = convert("v 1 two 3\n").unwrap_err();
        assert!(err.contains("line 1"), "{err}");
        assert!(err.contains("invalid coordinate 'two'"), "{err}");
    }

    #[test]
    fn nan_coordinate_rejected() {
        let err = convert("v 1 NaN 3\n").unwrap_err();
        assert!(err.contains("not a finite number"), "{err}");
    }

    #[test]
    fn stl_input_gets_a_pointed_error() {
        let err = convert("solid cube\n facet normal 0 0 1\n").unwrap_err();
        assert!(err.contains("ASCII STL"), "{err}");
    }

    #[test]
    fn no_vertices_errors() {
        let err = convert("# only comments\nvt 0 0\n").unwrap_err();
        assert!(err.contains("no vertex (`v`) lines"), "{err}");
    }

    #[test]
    fn header_can_be_dropped_and_delimiter_chosen() {
        let out =
            convert_str(CUBE_CORNER, "xyz", "auto", "keep", 1.0, -1, "none", "", "space", false)
                .unwrap();
        assert_eq!(out, "0.0 0.0 0.0\n1.0 0.0 0.0\n0.0 1.0 0.0");
    }

    #[test]
    fn indexed_columns_use_the_obj_vertex_index() {
        let obj = "o first\nv 1 1 1\no second\nv 2 2 2\n";
        let out = convert_str(obj, "indexed", "auto", "keep", 1.0, -1, "none", "second", "comma", true)
            .unwrap();
        // Index 2 is preserved even though vertex 1 was filtered out.
        assert_eq!(out, "index,x,y,z\n2,2,2,2");
    }

    #[test]
    fn object_columns_carry_section_context() {
        let obj = "o hull\ng deck rail\nusemtl paint\nv 1 2 3\n";
        let out =
            convert_str(obj, "full", "auto", "keep", 1.0, -1, "none", "", "comma", true).unwrap();
        assert_eq!(out, "index,object,group,material,x,y,z\n1,hull,deck rail,paint,1,2,3");
    }

    #[test]
    fn group_name_matches_the_objects_filter() {
        let obj = "g wheels\nv 1 1 1\ng body\nv 2 2 2\n";
        let out = convert_str(obj, "xyz", "auto", "keep", 1.0, -1, "none", "wheels", "comma", false)
            .unwrap();
        assert_eq!(out, "1,1,1");
    }

    #[test]
    fn unmatched_objects_filter_lists_available_names() {
        let obj = "o hull\nv 1 1 1\no mast\nv 2 2 2\n";
        let err = convert_str(obj, "xyz", "auto", "keep", 1.0, -1, "none", "keel", "comma", true)
            .unwrap_err();
        assert!(err.contains("no vertices matched objects 'keel'"), "{err}");
        assert!(err.contains("hull, mast"), "{err}");
    }

    #[test]
    fn names_containing_the_delimiter_are_quoted() {
        let obj = "o hull, port\nv 1 2 3\n";
        let out =
            convert_str(obj, "object", "auto", "keep", 1.0, -1, "none", "", "comma", false).unwrap();
        assert_eq!(out, "1,\"hull, port\",,1,2,3");
    }

    #[test]
    fn precision_rounds_and_pads() {
        let out = convert_str(
            "v 1.23456 2 -0.0004\n",
            "xyz",
            "auto",
            "keep",
            1.0,
            3,
            "none",
            "",
            "comma",
            false,
        )
        .unwrap();
        assert_eq!(out, "1.235,2.000,0.000");
    }

    #[test]
    fn scale_converts_units() {
        let out = convert_str(
            "v 1 2.5 -3\n",
            "xyz",
            "auto",
            "keep",
            1000.0,
            -1,
            "none",
            "",
            "comma",
            false,
        )
        .unwrap();
        assert_eq!(out, "1000,2500,-3000");
    }

    #[test]
    fn up_axis_y_to_z_rotates_about_x() {
        let out = convert_str(
            "v 1 2 3\n",
            "xyz",
            "auto",
            "y-to-z",
            1.0,
            -1,
            "none",
            "",
            "comma",
            false,
        )
        .unwrap();
        assert_eq!(out, "1,-3,2");
    }

    #[test]
    fn up_axis_z_to_y_is_the_inverse() {
        let out = convert_str(
            "v 1 -3 2\n",
            "xyz",
            "auto",
            "z-to-y",
            1.0,
            -1,
            "none",
            "",
            "comma",
            false,
        )
        .unwrap();
        assert_eq!(out, "1,2,3");
    }

    #[test]
    fn colour_columns_appear_when_present() {
        let obj = "v 0 0 0 1.0 0.0 0.0\nv 1 0 0\n";
        let out = convert(obj).unwrap();
        assert_eq!(out, "x,y,z,red,green,blue\n0,0,0,1.0,0.0,0.0\n1,0,0,,,");
    }

    #[test]
    fn colour_can_be_forced_off_and_on() {
        let with_color = "v 0 0 0 1.0 0.0 0.0\n";
        let off = convert_str(
            with_color, "xyz", "never", "keep", 1.0, -1, "none", "", "comma", true,
        )
        .unwrap();
        assert_eq!(off, "x,y,z\n0,0,0");
        let on = convert_str("v 1 2 3\n", "xyz", "always", "keep", 1.0, -1, "none", "", "comma", true)
            .unwrap();
        assert_eq!(on, "x,y,z,red,green,blue\n1,2,3,,,");
    }

    #[test]
    fn fourth_w_value_is_ignored() {
        assert_eq!(convert("v 1 2 3 0.5\n").unwrap(), "x,y,z\n1,2,3");
    }

    #[test]
    fn dedupe_adjacent_and_all() {
        let obj = "v 1 1 1\nv 1 1 1\nv 2 2 2\nv 1 1 1\n";
        let adjacent =
            convert_str(obj, "xyz", "auto", "keep", 1.0, -1, "adjacent", "", "comma", false)
                .unwrap();
        assert_eq!(adjacent, "1,1,1\n2,2,2\n1,1,1");
        let all =
            convert_str(obj, "xyz", "auto", "keep", 1.0, -1, "all", "", "comma", false).unwrap();
        assert_eq!(all, "1,1,1\n2,2,2");
    }

    #[test]
    fn line_continuations_are_folded() {
        let out = convert("v 1 \\\n  2 3\n").unwrap();
        assert_eq!(out, "x,y,z\n1,2,3");
    }

    #[test]
    fn trailing_comments_are_stripped() {
        assert_eq!(convert("v 1 2 3 # corner\n").unwrap(), "x,y,z\n1,2,3");
    }

    #[test]
    fn crlf_input_parses() {
        assert_eq!(convert("v 1 2 3\r\nv 4 5 6\r\n").unwrap(), "x,y,z\n1,2,3\n4,5,6");
    }

    #[test]
    fn bad_enum_values_are_rejected() {
        let err = convert_str(CUBE_CORNER, "xyz", "auto", "keep", 1.0, -1, "none", "", "pipes", true)
            .unwrap_err();
        assert!(err.contains("unknown delimiter 'pipes'"), "{err}");
        let err = convert_str(CUBE_CORNER, "xyz", "auto", "sideways", 1.0, -1, "none", "", "comma", true)
            .unwrap_err();
        assert!(err.contains("unknown up_axis"), "{err}");
    }

    #[test]
    fn precision_out_of_range_is_rejected() {
        let err = convert_str(CUBE_CORNER, "xyz", "auto", "keep", 1.0, 16, "none", "", "comma", true)
            .unwrap_err();
        assert!(err.contains("invalid precision 16"), "{err}");
    }

    #[test]
    fn row_cap_boundary() {
        let at_cap: String = std::iter::repeat("v 1 2 3\n").take(MAX_ROWS).collect();
        let out = convert_str(
            &at_cap, "xyz", "auto", "keep", 1.0, -1, "none", "", "comma", false,
        )
        .unwrap();
        assert_eq!(out.lines().count(), MAX_ROWS);

        let over_cap: String = std::iter::repeat("v 1 2 3\n").take(MAX_ROWS + 1).collect();
        let err = convert_str(
            &over_cap, "xyz", "auto", "keep", 1.0, -1, "none", "", "comma", false,
        )
        .unwrap_err();
        assert!(err.contains("too many vertices"), "{err}");
    }

    #[test]
    fn oversize_input_is_rejected() {
        let big = "v ".to_string() + &"1 ".repeat(1) + &" ".repeat(MAX_INPUT_BYTES);
        let err = convert(&big).unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }
}
