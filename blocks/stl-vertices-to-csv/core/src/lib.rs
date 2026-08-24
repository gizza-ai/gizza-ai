//! stl-vertices-to-csv core — pure compute, shared by the chat skill block and
//! the web page.
//!
//! Flattens an STL mesh into a CSV table of its triangle vertex coordinates.
//! ASCII STL is read as text; a binary STL is read from its bytes pasted as
//! base64 or hex (the same convention `blocks/stl-inspector` uses), because STL
//! is a binary format far more often than a textual one.
//!
//! Every facet stores its three corners explicitly, so the default output has
//! three rows per triangle, in file order — nothing is welded, indexed,
//! resampled or reordered. `rows = "triangle"` folds each facet onto a single
//! nine-coordinate row instead, which is the shape CAD and spreadsheet importers
//! usually ask for.

/// Largest number of triangles accepted.
pub const MAX_TRIANGLES: usize = 100_000;
/// Largest pasted input accepted (bytes).
pub const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;
/// Largest `every_nth` stride accepted.
pub const MAX_EVERY_NTH: i64 = 1_000_000;

type Vec3 = [f64; 3];

/// One facet: three corners plus the normal as stored in the file.
#[derive(Clone, Copy)]
struct Tri {
    v: [Vec3; 3],
    normal: Vec3,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputFormat {
    Auto,
    Ascii,
    Base64,
    Hex,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Rows {
    Vertex,
    Triangle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Columns {
    Xyz,
    Indexed,
    Normals,
    Full,
}

impl Columns {
    fn wants_index(self) -> bool {
        matches!(self, Columns::Indexed | Columns::Full)
    }
    fn wants_normal(self) -> bool {
        matches!(self, Columns::Normals | Columns::Full)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NormalSource {
    Stored,
    Computed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UpAxis {
    Keep,
    ZToY,
    YToZ,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dedupe {
    None,
    Adjacent,
    All,
}

fn parse_input_format(s: &str) -> Result<InputFormat, String> {
    match s.trim() {
        "" | "auto" => Ok(InputFormat::Auto),
        "ascii" => Ok(InputFormat::Ascii),
        "base64" => Ok(InputFormat::Base64),
        "hex" => Ok(InputFormat::Hex),
        other => Err(format!(
            "unknown input_format '{other}': expected 'auto', 'ascii', 'base64' or 'hex'"
        )),
    }
}

fn parse_rows(s: &str) -> Result<Rows, String> {
    match s.trim() {
        "" | "vertex" => Ok(Rows::Vertex),
        "triangle" => Ok(Rows::Triangle),
        other => Err(format!(
            "unknown rows '{other}': expected 'vertex' (one row per corner) or 'triangle' (one row \
             per facet)"
        )),
    }
}

fn parse_columns(s: &str) -> Result<Columns, String> {
    match s.trim() {
        "" | "xyz" => Ok(Columns::Xyz),
        "indexed" => Ok(Columns::Indexed),
        "normals" => Ok(Columns::Normals),
        "full" => Ok(Columns::Full),
        other => Err(format!(
            "unknown columns '{other}': expected 'xyz', 'indexed', 'normals' or 'full'"
        )),
    }
}

fn parse_normal_source(s: &str) -> Result<NormalSource, String> {
    match s.trim() {
        "" | "stored" => Ok(NormalSource::Stored),
        "computed" => Ok(NormalSource::Computed),
        other => Err(format!(
            "unknown normal_source '{other}': expected 'stored' or 'computed'"
        )),
    }
}

fn parse_up_axis(s: &str) -> Result<UpAxis, String> {
    match s.trim() {
        "" | "keep" => Ok(UpAxis::Keep),
        "z-to-y" => Ok(UpAxis::ZToY),
        "y-to-z" => Ok(UpAxis::YToZ),
        other => Err(format!(
            "unknown up_axis '{other}': expected 'keep', 'z-to-y' or 'y-to-z'"
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

// ---------------------------------------------------------------------------
// Number formatting
// ---------------------------------------------------------------------------

/// Shortest text that round-trips the stored value, with `-0` normalized to `0`.
/// Binary STL stores 32-bit floats, so those are formatted at `f32` width —
/// otherwise every coordinate would print a 17-digit binary-to-decimal tail.
fn fmt_shortest(v: f64, f32_source: bool) -> String {
    let narrowed = v as f32;
    let s = if f32_source && narrowed.is_finite() {
        format!("{narrowed}")
    } else {
        format!("{v}")
    };
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

fn fmt(v: f64, precision: i32, f32_source: bool) -> String {
    if precision < 0 {
        fmt_shortest(v, f32_source)
    } else {
        fmt_fixed(v, precision)
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Flatten an STL mesh into a CSV of its triangle vertex coordinates.
///
/// * `input_format` — `auto` | `ascii` | `base64` | `hex`
/// * `rows` — `vertex` (3 rows per facet) | `triangle` (1 row per facet)
/// * `columns` — `xyz` | `indexed` | `normals` | `full`
/// * `normal_source` — `stored` | `computed`
/// * `up_axis` — `keep` | `z-to-y` | `y-to-z`
/// * `scale` — multiplier applied to every coordinate
/// * `precision` — `-1` for shortest round-trip text, `0..=15` to round
/// * `dedupe` — `none` | `adjacent` | `all`, compared on the coordinate columns
/// * `every_nth` — keep one row out of every N that survives dedupe
/// * `delimiter` — `comma` | `semicolon` | `tab` | `pipe` | `space`
/// * `header` — emit the column-name row
#[allow(clippy::too_many_arguments)]
pub fn convert_str(
    stl: &str,
    input_format: &str,
    rows: &str,
    columns: &str,
    normal_source: &str,
    up_axis: &str,
    scale: f64,
    precision: i32,
    dedupe: &str,
    every_nth: i64,
    delimiter: &str,
    header: bool,
) -> Result<String, String> {
    let input_format = parse_input_format(input_format)?;
    let rows = parse_rows(rows)?;
    let columns = parse_columns(columns)?;
    let normal_source = parse_normal_source(normal_source)?;
    let up_axis = parse_up_axis(up_axis)?;
    let dedupe = parse_dedupe(dedupe)?;
    let delim = parse_delimiter(delimiter)?;
    if !(-1..=15).contains(&precision) {
        return Err(format!(
            "invalid precision {precision}: expected -1 (shortest text that round-trips the stored \
             value) or 0-15 decimal places"
        ));
    }
    if !scale.is_finite() {
        return Err("invalid scale: expected a finite number (1 keeps the source size)".to_string());
    }
    if !(1..=MAX_EVERY_NTH).contains(&every_nth) {
        return Err(format!(
            "invalid every_nth {every_nth}: expected 1 (keep every row) to {MAX_EVERY_NTH}"
        ));
    }
    if stl.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "STL is too large: {} bytes exceeds the {MAX_INPUT_BYTES} byte limit",
            stl.len()
        ));
    }

    let (mut tris, f32_source) = decode(stl, input_format)?;

    // Geometry first: rotate, then scale. Stored normals rotate with the mesh
    // but are never scaled — they are directions, not positions.
    for t in &mut tris {
        for v in &mut t.v {
            *v = rotate(*v, up_axis);
            if scale != 1.0 {
                *v = [v[0] * scale, v[1] * scale, v[2] * scale];
            }
        }
        t.normal = rotate(t.normal, up_axis);
        if normal_source == NormalSource::Computed {
            t.normal = face_normal(&t.v);
        }
    }

    let mut out_rows: Vec<String> = Vec::new();
    let mut last_key: Option<String> = None;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut kept: i64 = 0;

    let mut push = |index: usize, corner: Option<usize>, coords: Vec<String>, normal: Vec3| {
        let key = coords.join("\u{0}");
        match dedupe {
            Dedupe::None => {}
            Dedupe::Adjacent => {
                if last_key.as_deref() == Some(key.as_str()) {
                    return;
                }
                last_key = Some(key);
            }
            Dedupe::All => {
                if !seen.insert(key) {
                    return;
                }
            }
        }
        // The stride runs over the rows that survived dedupe, so a deduped
        // point cloud thins evenly rather than in clumps.
        let position = kept;
        kept += 1;
        if position % every_nth != 0 {
            return;
        }

        let mut fields: Vec<String> = Vec::with_capacity(13);
        if columns.wants_index() {
            fields.push(index.to_string());
            if let Some(c) = corner {
                fields.push(c.to_string());
            }
        }
        fields.extend(coords);
        if columns.wants_normal() {
            for n in normal {
                fields.push(fmt(n, precision, f32_source));
            }
        }
        out_rows.push(fields.join(&delim.to_string()));
    };

    for (i, t) in tris.iter().enumerate() {
        match rows {
            Rows::Vertex => {
                for (c, v) in t.v.iter().enumerate() {
                    let coords = vec![
                        fmt(v[0], precision, f32_source),
                        fmt(v[1], precision, f32_source),
                        fmt(v[2], precision, f32_source),
                    ];
                    push(i + 1, Some(c + 1), coords, t.normal);
                }
            }
            Rows::Triangle => {
                let mut coords = Vec::with_capacity(9);
                for v in t.v {
                    for c in v {
                        coords.push(fmt(c, precision, f32_source));
                    }
                }
                push(i + 1, None, coords, t.normal);
            }
        }
    }

    let mut out = String::with_capacity(out_rows.iter().map(|r| r.len() + 1).sum());
    if header {
        let mut cols: Vec<&str> = Vec::with_capacity(13);
        if columns.wants_index() {
            cols.push("triangle");
            if rows == Rows::Vertex {
                cols.push("corner");
            }
        }
        match rows {
            Rows::Vertex => cols.extend(["x", "y", "z"]),
            Rows::Triangle => cols.extend([
                "v1x", "v1y", "v1z", "v2x", "v2y", "v2z", "v3x", "v3y", "v3z",
            ]),
        }
        if columns.wants_normal() {
            cols.extend(["nx", "ny", "nz"]);
        }
        out.push_str(&cols.join(&delim.to_string()));
        out.push('\n');
    }
    out.push_str(&out_rows.join("\n"));
    Ok(out)
}

fn rotate(v: Vec3, up_axis: UpAxis) -> Vec3 {
    match up_axis {
        UpAxis::Keep => v,
        // Z-up (the STL convention) → Y-up (glTF/three.js): rotate −90° about X.
        UpAxis::ZToY => [v[0], v[2], -v[1]],
        // Y-up → Z-up: rotate +90° about X.
        UpAxis::YToZ => [v[0], -v[2], v[1]],
    }
}

/// Right-hand-rule normal of the three corners; the zero vector when the
/// triangle is degenerate (two corners coincide or all three are collinear).
fn face_normal(v: &[Vec3; 3]) -> Vec3 {
    let a = [v[1][0] - v[0][0], v[1][1] - v[0][1], v[1][2] - v[0][2]];
    let b = [v[2][0] - v[0][0], v[2][1] - v[0][1], v[2][2] - v[0][2]];
    let n = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len == 0.0 || !len.is_finite() {
        [0.0; 3]
    } else {
        [n[0] / len, n[1] / len, n[2] / len]
    }
}

// ---------------------------------------------------------------------------
// Decoding — ASCII vs binary, and base64 vs hex for binary
// ---------------------------------------------------------------------------

/// Returns the facets plus whether they came from 32-bit binary floats.
fn decode(input: &str, fmt: InputFormat) -> Result<(Vec<Tri>, bool), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(
            "empty input: paste an ASCII STL (solid / facet normal / vertex lines), or a binary \
             STL's bytes as base64 or hex"
                .to_string(),
        );
    }

    match fmt {
        InputFormat::Ascii => Ok((parse_ascii(trimmed)?, false)),
        InputFormat::Base64 => from_bytes(&decode_base64(trimmed)?),
        InputFormat::Hex => from_bytes(&decode_hex(trimmed)?),
        InputFormat::Auto => {
            if looks_like_ascii_stl(trimmed) {
                return Ok((parse_ascii(trimmed)?, false));
            }
            if let Some(hint) = looks_like_other_format(trimmed) {
                return Err(hint);
            }
            // Try hex before base64: the hex alphabet is a strict subset of
            // base64's, so a hex blob would otherwise decode as base64 garbage.
            let hex_err = match decode_hex(trimmed) {
                Ok(bytes) => match from_bytes(&bytes) {
                    Ok(v) => return Ok(v),
                    Err(e) => e,
                },
                Err(e) => e,
            };
            match decode_base64(trimmed) {
                Ok(bytes) => from_bytes(&bytes),
                Err(b64_err) => Err(format!(
                    "could not read the input as an ASCII STL, hex bytes or base64 bytes. As hex: \
                     {hex_err}. As base64: {b64_err}. Set input_format explicitly if \
                     auto-detection is guessing wrong."
                )),
            }
        }
    }
}

/// A blob is ASCII STL when it carries the keywords a text STL must have.
/// `solid` alone is NOT enough — binary headers routinely start with it.
fn looks_like_ascii_stl(s: &str) -> bool {
    let head: String = s.chars().take(4096).collect::<String>().to_ascii_lowercase();
    head.contains("facet normal") || (head.contains("outer loop") && head.contains("vertex "))
}

/// Point at the right tool when the paste is clearly some other 3D/text format,
/// instead of failing with a base64 complaint.
fn looks_like_other_format(s: &str) -> Option<String> {
    let head: String = s.chars().take(4096).collect::<String>().to_ascii_lowercase();
    if head.starts_with("ply") && head.contains("element vertex") {
        return Some(
            "this looks like a PLY mesh, not an STL — convert it to STL first".to_string(),
        );
    }
    if head.starts_with('{') || head.starts_with('[') {
        return Some("this looks like JSON, not an STL mesh".to_string());
    }
    let obj_like = head
        .lines()
        .filter(|l| l.starts_with("v ") || l.starts_with("f ") || l.starts_with("vn "))
        .count();
    if obj_like >= 2 {
        return Some(
            "this looks like a Wavefront OBJ, not an STL — use the OBJ vertex extractor, or \
             convert the model to STL first"
                .to_string(),
        );
    }
    None
}

/// Turn decoded bytes into facets. Binary STL is the expectation, but an ASCII
/// STL that was base64'd or hex-dumped still decodes cleanly, so accept it.
fn from_bytes(bytes: &[u8]) -> Result<(Vec<Tri>, bool), String> {
    match parse_binary(bytes) {
        Ok(tris) => Ok((tris, true)),
        Err(bin_err) => {
            if let Ok(text) = std::str::from_utf8(bytes) {
                if looks_like_ascii_stl(text) {
                    return Ok((parse_ascii(text.trim())?, false));
                }
            }
            Err(bin_err)
        }
    }
}

fn parse_ascii(text: &str) -> Result<Vec<Tri>, String> {
    let mut tris: Vec<Tri> = Vec::new();
    let mut normal: Vec3 = [0.0; 3];
    let mut corners: Vec<Vec3> = Vec::new();
    let mut in_facet = false;

    for (i, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let lineno = i + 1;
        let mut parts = line.split_ascii_whitespace();
        let key = parts.next().unwrap_or("").to_ascii_lowercase();

        match key.as_str() {
            "facet" => {
                in_facet = true;
                corners.clear();
                normal = [0.0; 3];
                // `facet normal ni nj nk` — a missing or short normal is
                // tolerated and simply stays the zero vector.
                if parts.next().map(|w| w.eq_ignore_ascii_case("normal")) == Some(true) {
                    let nums: Vec<&str> = parts.collect();
                    if nums.len() >= 3 {
                        for (k, n) in nums.iter().take(3).enumerate() {
                            normal[k] = parse_f64(n, lineno, "facet normal")?;
                        }
                    }
                }
            }
            "vertex" => {
                let nums: Vec<&str> = parts.collect();
                if nums.len() < 3 {
                    return Err(format!(
                        "line {lineno}: 'vertex' needs 3 coordinates, found {}",
                        nums.len()
                    ));
                }
                let mut v: Vec3 = [0.0; 3];
                for (k, n) in nums.iter().take(3).enumerate() {
                    v[k] = parse_f64(n, lineno, "vertex")?;
                }
                if corners.len() >= 3 {
                    return Err(format!(
                        "line {lineno}: facet has more than 3 vertices — STL facets are triangles"
                    ));
                }
                corners.push(v);
            }
            "endfacet" => {
                if corners.len() != 3 {
                    return Err(format!(
                        "line {lineno}: facet ended with {} vertices, expected 3",
                        corners.len()
                    ));
                }
                if tris.len() >= MAX_TRIANGLES {
                    return Err(format!(
                        "too many triangles: this tool handles up to {MAX_TRIANGLES}"
                    ));
                }
                tris.push(Tri {
                    v: [corners[0], corners[1], corners[2]],
                    normal,
                });
                corners.clear();
                in_facet = false;
            }
            // `solid`, `outer loop`, `endloop`, `endsolid` and anything else are noise.
            _ => {}
        }
    }

    if in_facet {
        return Err("the last facet is unterminated (no 'endfacet' line)".to_string());
    }
    if tris.is_empty() {
        return Err(
            "no facets found — an ASCII STL needs 'facet normal' / 'vertex' / 'endfacet' lines"
                .to_string(),
        );
    }
    Ok(tris)
}

fn parse_f64(s: &str, lineno: usize, what: &str) -> Result<f64, String> {
    s.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("line {lineno}: '{s}' is not a finite number in a {what}"))
}

fn parse_binary(bytes: &[u8]) -> Result<Vec<Tri>, String> {
    if bytes.len() < 84 {
        return Err(format!(
            "binary STL too short: {} bytes, but the 80-byte header plus the 4-byte triangle count \
             already need 84",
            bytes.len()
        ));
    }
    let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    if count > MAX_TRIANGLES {
        return Err(format!(
            "too many triangles: the header declares {count}, this tool handles up to \
             {MAX_TRIANGLES}"
        ));
    }
    let needed = 84usize + count * 50;
    if bytes.len() < needed {
        return Err(format!(
            "truncated binary STL: the header declares {count} triangles ({needed} bytes) but only \
             {} bytes were given",
            bytes.len()
        ));
    }
    if count == 0 {
        return Err("no facets found — the binary STL header declares 0 triangles".to_string());
    }

    let mut tris = Vec::with_capacity(count);
    for i in 0..count {
        let base = 84 + i * 50;
        let mut vals = [0.0f64; 12];
        for (k, val) in vals.iter_mut().enumerate() {
            let o = base + k * 4;
            *val = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]) as f64;
        }
        if vals.iter().any(|v| !v.is_finite()) {
            return Err(format!(
                "triangle {} contains a non-finite coordinate (NaN or infinity)",
                i + 1
            ));
        }
        tris.push(Tri {
            v: [
                [vals[3], vals[4], vals[5]],
                [vals[6], vals[7], vals[8]],
                [vals[9], vals[10], vals[11]],
            ],
            normal: [vals[0], vals[1], vals[2]],
        });
    }
    Ok(tris)
}

fn b64_value(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        // Standard and URL-safe alphabets are both accepted.
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    }
}

/// Standard or URL-safe base64, padding optional, whitespace ignored.
fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    let mut out = Vec::with_capacity(s.len() / 4 * 3 + 3);
    for (i, c) in s.bytes().enumerate() {
        if c.is_ascii_whitespace() || c == b'=' {
            continue;
        }
        let v = b64_value(c).ok_or_else(|| {
            format!(
                "not valid base64: byte {} is '{}'",
                i + 1,
                (c as char).escape_default()
            )
        })?;
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    if out.is_empty() {
        return Err("not valid base64: no data decoded".to_string());
    }
    Ok(out)
}

/// Hex bytes; whitespace, `:`, `-`, `,` and a leading `0x` are ignored.
fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let mut nibbles: Vec<u8> = Vec::with_capacity(s.len());
    let body = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    for (i, c) in body.bytes().enumerate() {
        if c.is_ascii_whitespace() || c == b':' || c == b'-' || c == b',' {
            continue;
        }
        let v = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => {
                return Err(format!(
                    "not valid hex: byte {} is '{}'",
                    i + 1,
                    (c as char).escape_default()
                ))
            }
        };
        nibbles.push(v);
    }
    if nibbles.is_empty() {
        return Err("not valid hex: no data decoded".to_string());
    }
    if nibbles.len() % 2 != 0 {
        return Err(format!(
            "not valid hex: {} hex digits is an odd count — every byte needs two",
            nibbles.len()
        ));
    }
    Ok(nibbles.chunks(2).map(|p| (p[0] << 4) | p[1]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRIANGLE: &str = "solid tri\n\
         facet normal 0 0 1\n\
         outer loop\n\
         vertex 0 0 0\n\
         vertex 1 0 0\n\
         vertex 0 1 0\n\
         endloop\n\
         endfacet\n\
         endsolid tri\n";

    fn convert(stl: &str) -> Result<String, String> {
        convert_str(
            stl, "auto", "vertex", "xyz", "stored", "keep", 1.0, -1, "none", 1, "comma", true,
        )
    }

    /// Little-endian binary STL from `(normal, corners)` facets.
    fn binary_stl(facets: &[([f32; 3], [[f32; 3]; 3])]) -> Vec<u8> {
        let mut bytes = vec![0u8; 80];
        bytes[..5].copy_from_slice(b"fixtu");
        bytes.extend_from_slice(&(facets.len() as u32).to_le_bytes());
        for (n, vs) in facets {
            for c in n {
                bytes.extend_from_slice(&c.to_le_bytes());
            }
            for v in vs {
                for c in v {
                    bytes.extend_from_slice(&c.to_le_bytes());
                }
            }
            bytes.extend_from_slice(&0u16.to_le_bytes());
        }
        bytes
    }

    fn to_base64(bytes: &[u8]) -> String {
        const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            out.push(A[(n >> 18) as usize & 63] as char);
            out.push(A[(n >> 12) as usize & 63] as char);
            out.push(if chunk.len() > 1 {
                A[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            out.push(if chunk.len() > 2 {
                A[n as usize & 63] as char
            } else {
                '='
            });
        }
        out
    }

    fn to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    const UNIT: [([f32; 3], [[f32; 3]; 3]); 1] = [(
        [0.0, 0.0, 1.0],
        [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
    )];

    #[test]
    fn ascii_triangle_gives_three_rows() {
        assert_eq!(
            convert(TRIANGLE).unwrap(),
            "x,y,z\n0,0,0\n1,0,0\n0,1,0"
        );
    }

    #[test]
    fn empty_input_errors() {
        let err = convert("   \n").unwrap_err();
        assert!(err.contains("empty input"), "{err}");
    }

    #[test]
    fn short_vertex_line_errors_with_line_number() {
        let stl = "solid s\nfacet normal 0 0 1\nouter loop\nvertex 0 0\nendloop\nendfacet\n";
        let err = convert(stl).unwrap_err();
        assert!(err.contains("line 4"), "{err}");
        assert!(err.contains("needs 3 coordinates"), "{err}");
    }

    #[test]
    fn non_numeric_vertex_errors() {
        let stl = "solid s\nfacet normal 0 0 1\nvertex 0 zero 0\nendfacet\n";
        let err = convert(stl).unwrap_err();
        assert!(err.contains("'zero' is not a finite number"), "{err}");
    }

    #[test]
    fn unterminated_facet_errors() {
        let stl = "solid s\nfacet normal 0 0 1\nvertex 0 0 0\nvertex 1 0 0\n";
        let err = convert(stl).unwrap_err();
        assert!(err.contains("unterminated"), "{err}");
    }

    #[test]
    fn facet_with_four_vertices_errors() {
        let stl = "solid s\nfacet normal 0 0 1\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\n\
                   vertex 1 1 0\nendfacet\n";
        let err = convert(stl).unwrap_err();
        assert!(err.contains("more than 3 vertices"), "{err}");
    }

    #[test]
    fn scientific_notation_is_normalized() {
        let stl = "facet normal 0 0 1\nvertex 1.500000e+000 -2.0 3\nvertex 1 0 0\nvertex 0 1 0\n\
                   endfacet\n";
        let out = convert(stl).unwrap();
        assert_eq!(out.lines().nth(1).unwrap(), "1.5,-2,3");
    }

    #[test]
    fn triangle_rows_fold_nine_coordinates() {
        let out = convert_str(
            TRIANGLE, "ascii", "triangle", "xyz", "stored", "keep", 1.0, -1, "none", 1, "comma",
            true,
        )
        .unwrap();
        assert_eq!(
            out,
            "v1x,v1y,v1z,v2x,v2y,v2z,v3x,v3y,v3z\n0,0,0,1,0,0,0,1,0"
        );
    }

    #[test]
    fn indexed_columns_number_triangles_and_corners() {
        let out = convert_str(
            TRIANGLE, "auto", "vertex", "indexed", "stored", "keep", 1.0, -1, "none", 1, "comma",
            true,
        )
        .unwrap();
        assert_eq!(
            out,
            "triangle,corner,x,y,z\n1,1,0,0,0\n1,2,1,0,0\n1,3,0,1,0"
        );
    }

    #[test]
    fn triangle_rows_drop_the_corner_column() {
        let out = convert_str(
            TRIANGLE, "auto", "triangle", "indexed", "stored", "keep", 1.0, -1, "none", 1, "comma",
            true,
        )
        .unwrap();
        assert_eq!(
            out,
            "triangle,v1x,v1y,v1z,v2x,v2y,v2z,v3x,v3y,v3z\n1,0,0,0,1,0,0,0,1,0"
        );
    }

    #[test]
    fn stored_normals_are_copied_and_computed_normals_replace_them() {
        // The fixture stores a deliberately wrong normal; 'computed' fixes it.
        let stl = "facet normal 0 0 -1\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendfacet\n";
        let stored = convert_str(
            stl, "ascii", "triangle", "normals", "stored", "keep", 1.0, -1, "none", 1, "comma",
            false,
        )
        .unwrap();
        assert_eq!(stored, "0,0,0,1,0,0,0,1,0,0,0,-1");
        let computed = convert_str(
            stl, "ascii", "triangle", "normals", "computed", "keep", 1.0, -1, "none", 1, "comma",
            false,
        )
        .unwrap();
        assert_eq!(computed, "0,0,0,1,0,0,0,1,0,0,0,1");
    }

    #[test]
    fn degenerate_triangle_computes_a_zero_normal() {
        let stl = "facet normal 0 0 1\nvertex 0 0 0\nvertex 1 0 0\nvertex 2 0 0\nendfacet\n";
        let out = convert_str(
            stl, "ascii", "triangle", "normals", "computed", "keep", 1.0, -1, "none", 1, "comma",
            false,
        )
        .unwrap();
        assert!(out.ends_with(",0,0,0"), "{out}");
    }

    #[test]
    fn full_columns_carry_index_and_normal() {
        let out = convert_str(
            TRIANGLE, "auto", "vertex", "full", "stored", "keep", 1.0, -1, "none", 1, "comma", true,
        )
        .unwrap();
        assert_eq!(
            out,
            "triangle,corner,x,y,z,nx,ny,nz\n1,1,0,0,0,0,0,1\n1,2,1,0,0,0,0,1\n1,3,0,1,0,0,0,1"
        );
    }

    #[test]
    fn up_axis_z_to_y_rotates_coordinates_and_normals() {
        let out = convert_str(
            TRIANGLE, "auto", "triangle", "normals", "stored", "z-to-y", 1.0, -1, "none", 1,
            "comma", false,
        )
        .unwrap();
        // (x,y,z) -> (x,z,-y); the stored +Z normal becomes +Y.
        assert_eq!(out, "0,0,0,1,0,0,0,0,-1,0,1,0");
    }

    #[test]
    fn up_axis_y_to_z_is_the_inverse() {
        let stl = "facet normal 0 1 0\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 0 -1\nendfacet\n";
        let out = convert_str(
            stl, "ascii", "triangle", "normals", "stored", "y-to-z", 1.0, -1, "none", 1, "comma",
            false,
        )
        .unwrap();
        assert_eq!(out, "0,0,0,1,0,0,0,1,0,0,0,1");
    }

    #[test]
    fn scale_changes_units_without_touching_normals() {
        let out = convert_str(
            TRIANGLE, "auto", "triangle", "normals", "stored", "keep", 25.4, -1, "none", 1, "comma",
            false,
        )
        .unwrap();
        assert_eq!(out, "0,0,0,25.4,0,0,0,25.4,0,0,0,1");
    }

    #[test]
    fn precision_rounds_and_pads() {
        let stl = "facet normal 0 0 1\nvertex 1.23456 2 -0.0004\nvertex 1 0 0\nvertex 0 1 0\n\
                   endfacet\n";
        let out = convert_str(
            stl, "ascii", "vertex", "xyz", "stored", "keep", 1.0, 3, "none", 1, "comma", false,
        )
        .unwrap();
        assert_eq!(out.lines().next().unwrap(), "1.235,2.000,0.000");
    }

    #[test]
    fn dedupe_all_welds_shared_corners() {
        // Two facets of a square share two corners: 6 rows collapse to 4 points.
        let stl = "facet normal 0 0 1\nvertex 0 0 0\nvertex 1 0 0\nvertex 1 1 0\nendfacet\n\
                   facet normal 0 0 1\nvertex 0 0 0\nvertex 1 1 0\nvertex 0 1 0\nendfacet\n";
        let out = convert_str(
            stl, "ascii", "vertex", "xyz", "stored", "keep", 1.0, -1, "all", 1, "comma", false,
        )
        .unwrap();
        assert_eq!(out, "0,0,0\n1,0,0\n1,1,0\n0,1,0");
    }

    #[test]
    fn dedupe_adjacent_only_drops_consecutive_repeats() {
        let stl = "facet normal 0 0 1\nvertex 0 0 0\nvertex 0 0 0\nvertex 1 0 0\nendfacet\n\
                   facet normal 0 0 1\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendfacet\n";
        let out = convert_str(
            stl, "ascii", "vertex", "xyz", "stored", "keep", 1.0, -1, "adjacent", 1, "comma", false,
        )
        .unwrap();
        assert_eq!(out, "0,0,0\n1,0,0\n0,0,0\n1,0,0\n0,1,0");
    }

    #[test]
    fn every_nth_thins_the_surviving_rows() {
        let out = convert_str(
            TRIANGLE, "auto", "vertex", "xyz", "stored", "keep", 1.0, -1, "none", 2, "comma", false,
        )
        .unwrap();
        assert_eq!(out, "0,0,0\n0,1,0");
    }

    #[test]
    fn every_nth_runs_after_dedupe() {
        let stl = "facet normal 0 0 1\nvertex 0 0 0\nvertex 0 0 0\nvertex 1 0 0\nendfacet\n";
        let out = convert_str(
            stl, "ascii", "vertex", "xyz", "stored", "keep", 1.0, -1, "adjacent", 2, "comma", false,
        )
        .unwrap();
        // Dedupe leaves (0,0,0),(1,0,0); the stride then keeps the first only.
        assert_eq!(out, "0,0,0");
    }

    #[test]
    fn delimiter_and_header_make_a_plain_xyz_point_cloud() {
        let out = convert_str(
            TRIANGLE, "auto", "vertex", "xyz", "stored", "keep", 1.0, 2, "all", 1, "space", false,
        )
        .unwrap();
        assert_eq!(out, "0.00 0.00 0.00\n1.00 0.00 0.00\n0.00 1.00 0.00");
    }

    #[test]
    fn every_delimiter_is_accepted() {
        for (name, sep) in [
            ("comma", ","),
            ("semicolon", ";"),
            ("tab", "\t"),
            ("pipe", "|"),
            ("space", " "),
        ] {
            let out = convert_str(
                TRIANGLE, "auto", "vertex", "xyz", "stored", "keep", 1.0, -1, "none", 1, name,
                false,
            )
            .unwrap();
            assert_eq!(out.lines().next().unwrap(), format!("0{sep}0{sep}0"));
        }
    }

    #[test]
    fn binary_stl_reads_from_base64_and_hex() {
        let bytes = binary_stl(&UNIT);
        let expected = "x,y,z\n0,0,0\n1,0,0\n0,1,0";
        assert_eq!(convert(&to_base64(&bytes)).unwrap(), expected);
        assert_eq!(convert(&to_hex(&bytes)).unwrap(), expected);
        // And with the format forced rather than auto-detected.
        assert_eq!(
            convert_str(
                &to_base64(&bytes), "base64", "vertex", "xyz", "stored", "keep", 1.0, -1, "none",
                1, "comma", true
            )
            .unwrap(),
            expected
        );
        assert_eq!(
            convert_str(
                &to_hex(&bytes), "hex", "vertex", "xyz", "stored", "keep", 1.0, -1, "none", 1,
                "comma", true
            )
            .unwrap(),
            expected
        );
    }

    #[test]
    fn binary_floats_print_at_f32_width() {
        // 0.1f32 as f64 is 0.10000000149011612 — printing that would be noise.
        let bytes = binary_stl(&[(
            [0.0, 0.0, 1.0],
            [[0.1, 0.2, 0.3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        )]);
        let out = convert(&to_base64(&bytes)).unwrap();
        assert_eq!(out.lines().nth(1).unwrap(), "0.1,0.2,0.3");
    }

    #[test]
    fn hex_accepts_separators_and_a_0x_prefix() {
        let bytes = binary_stl(&UNIT);
        let spaced: String = bytes
            .iter()
            .map(|b| format!("{b:02X}:"))
            .collect::<String>();
        let out = convert_str(
            &format!("0x{spaced}"), "hex", "vertex", "xyz", "stored", "keep", 1.0, -1, "none", 1,
            "comma", false,
        )
        .unwrap();
        assert_eq!(out, "0,0,0\n1,0,0\n0,1,0");
    }

    #[test]
    fn base64_wrapped_ascii_stl_still_parses() {
        let out = convert(&to_base64(TRIANGLE.as_bytes())).unwrap();
        assert_eq!(out, "x,y,z\n0,0,0\n1,0,0\n0,1,0");
    }

    #[test]
    fn binary_header_starting_with_solid_is_not_read_as_ascii() {
        let mut bytes = binary_stl(&UNIT);
        bytes[..6].copy_from_slice(b"solid ");
        assert_eq!(convert(&to_base64(&bytes)).unwrap(), "x,y,z\n0,0,0\n1,0,0\n0,1,0");
    }

    #[test]
    fn truncated_binary_stl_errors() {
        let mut bytes = binary_stl(&UNIT);
        bytes.truncate(bytes.len() - 10);
        let err = convert(&to_base64(&bytes)).unwrap_err();
        assert!(err.contains("truncated binary STL"), "{err}");
    }

    #[test]
    fn zero_facet_binary_stl_errors() {
        let err = convert(&to_base64(&binary_stl(&[]))).unwrap_err();
        assert!(err.contains("declares 0 triangles"), "{err}");
    }

    #[test]
    fn triangle_cap_boundary() {
        // A header-only blob declaring exactly the cap is truncated, not capped…
        let mut at_cap = vec![0u8; 80];
        at_cap.extend_from_slice(&(MAX_TRIANGLES as u32).to_le_bytes());
        let err = convert(&to_base64(&at_cap)).unwrap_err();
        assert!(err.contains("truncated binary STL"), "{err}");
        // …one more than the cap is rejected before any body is read.
        let mut over_cap = vec![0u8; 80];
        over_cap.extend_from_slice(&(MAX_TRIANGLES as u32 + 1).to_le_bytes());
        let err = convert(&to_base64(&over_cap)).unwrap_err();
        assert!(err.contains("too many triangles"), "{err}");
        assert!(err.contains("100001"), "{err}");
    }

    #[test]
    fn ascii_triangle_cap_is_enforced() {
        let facet = "facet normal 0 0 1\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendfacet\n";
        let at_cap: String = std::iter::repeat(facet).take(MAX_TRIANGLES).collect();
        let out = convert_str(
            &at_cap, "ascii", "triangle", "xyz", "stored", "keep", 1.0, -1, "none", 1, "comma",
            false,
        )
        .unwrap();
        assert_eq!(out.lines().count(), MAX_TRIANGLES);
        let over_cap = at_cap + facet;
        let err = convert_str(
            &over_cap, "ascii", "triangle", "xyz", "stored", "keep", 1.0, -1, "none", 1, "comma",
            false,
        )
        .unwrap_err();
        assert!(err.contains("too many triangles"), "{err}");
    }

    #[test]
    fn obj_input_gets_a_pointed_error() {
        let err = convert("v 0 0 0\nv 1 0 0\nf 1 2 3\n").unwrap_err();
        assert!(err.contains("Wavefront OBJ"), "{err}");
    }

    #[test]
    fn ply_and_json_inputs_get_pointed_errors() {
        let err = convert("ply\nformat ascii 1.0\nelement vertex 3\n").unwrap_err();
        assert!(err.contains("PLY mesh"), "{err}");
        let err = convert("{\"vertices\": []}").unwrap_err();
        assert!(err.contains("JSON"), "{err}");
    }

    #[test]
    fn undecodable_input_explains_both_attempts() {
        let err = convert("!!! not a mesh !!!").unwrap_err();
        assert!(err.contains("As hex:"), "{err}");
        assert!(err.contains("As base64:"), "{err}");
    }

    #[test]
    fn bad_enum_values_are_rejected() {
        let err = convert_str(
            TRIANGLE, "auto", "corners", "xyz", "stored", "keep", 1.0, -1, "none", 1, "comma", true,
        )
        .unwrap_err();
        assert!(err.contains("unknown rows 'corners'"), "{err}");
        let err = convert_str(
            TRIANGLE, "auto", "vertex", "xyz", "guessed", "keep", 1.0, -1, "none", 1, "comma", true,
        )
        .unwrap_err();
        assert!(err.contains("unknown normal_source"), "{err}");
        let err = convert_str(
            TRIANGLE, "yaml", "vertex", "xyz", "stored", "keep", 1.0, -1, "none", 1, "comma", true,
        )
        .unwrap_err();
        assert!(err.contains("unknown input_format 'yaml'"), "{err}");
    }

    #[test]
    fn out_of_range_numbers_are_rejected() {
        let err = convert_str(
            TRIANGLE, "auto", "vertex", "xyz", "stored", "keep", 1.0, 16, "none", 1, "comma", true,
        )
        .unwrap_err();
        assert!(err.contains("invalid precision 16"), "{err}");
        let err = convert_str(
            TRIANGLE, "auto", "vertex", "xyz", "stored", "keep", f64::NAN, -1, "none", 1, "comma",
            true,
        )
        .unwrap_err();
        assert!(err.contains("invalid scale"), "{err}");
        let err = convert_str(
            TRIANGLE, "auto", "vertex", "xyz", "stored", "keep", 1.0, -1, "none", 0, "comma", true,
        )
        .unwrap_err();
        assert!(err.contains("invalid every_nth 0"), "{err}");
    }

    #[test]
    fn oversize_input_is_rejected() {
        let big = " ".repeat(MAX_INPUT_BYTES + 1);
        let err = convert(&big).unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }
}
