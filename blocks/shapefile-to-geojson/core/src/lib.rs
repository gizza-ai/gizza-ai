//! shapefile-to-geojson core — turn an ESRI shapefile set (`.shp` geometry +
//! `.dbf` attributes + `.prj` CRS) into GeoJSON. No wafer/wasm-bindgen deps; pure
//! logic shared by the chat skill block and the CLI (and host-testable).
//!
//! **Input shapes.** A shapefile is not one file, it is a *set*, so a single-upload
//! tool accepts either:
//!   * a **`.zip`** holding the set (how Census/TIGER, Natural Earth and most
//!     government portals actually ship them) — the `.shp` is paired with the
//!     `.dbf`/`.prj`/`.cpg` sharing its stem; or
//!   * a **bare `.shp`**, which yields geometry with empty properties (the
//!     attributes live in the `.dbf`).
//! The `.shx` index is deliberately unused: it only maps record numbers to byte
//! offsets, and the `.shp` is fully sequential-readable, so a missing `.shx` is
//! never fatal here.
//!
//! **Geometry.** The `.shp` layout is parsed by hand (no reader crate): a 100-byte
//! header, then records of `[record number: i32 BE][content length in 16-bit words:
//! i32 BE][content]`, where the content starts with a little-endian shape type.
//! Supported types: Null (0), Point (1/11/21), MultiPoint (8/18/28), PolyLine
//! (3/13/23) and Polygon (5/15/25) — i.e. the 2D, Z and M variants of everything
//! GeoJSON can express. MultiPatch (31) is rejected with a named error: it encodes
//! triangle strips/fans, which RFC 7946 has no geometry for.
//!
//! **Rings.** A shapefile Polygon is a flat ring list with orientation carrying the
//! meaning (clockwise = outer, counter-clockwise = hole), so rings are regrouped
//! into GeoJSON `Polygon`/`MultiPolygon` by signed area, and (by default) rewound
//! to RFC 7946's opposite convention (exterior counter-clockwise, holes clockwise).
//!
//! **M values are dropped**: GeoJSON position arrays are `[x, y]` or `[x, y, z]`
//! only, with no measure slot in RFC 7946.

use gizza_ai_dbf_table_parser_core::{
    parse_dbf, Encoding as DbfEncoding, Format as DbfFormat, Options as DbfOptions,
};
use serde_json::{json, Map, Number, Value};
use std::io::Read;

/// How the converted features are serialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// One RFC 7946 `FeatureCollection` object.
    Geojson,
    /// Newline-delimited GeoJSON (GeoJSONL / GeoJSON Text Sequences): one
    /// `Feature` object per line, no collection wrapper — streamable line by line.
    Ndjson,
}

/// Text decoding for `.dbf` character fields (mirrors the DBF core's own set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// Honour a `.cpg` sidecar when present, else UTF-8 if valid, else Latin-1.
    Auto,
    Utf8,
    Latin1,
    Cp1252,
}

/// Conversion options (parsed from the block/CLI args upstream).
#[derive(Debug, Clone)]
pub struct Options {
    pub output: Output,
    /// Indent the GeoJSON. Ignored for `ndjson` (each line must stay one line).
    pub pretty: bool,
    /// Decimal places for coordinates; `-1` keeps full precision. 6 places is
    /// ~11 cm at the equator and typically shrinks a boundary file several-fold.
    pub precision: i64,
    /// Max features to emit; 0 = all.
    pub limit: usize,
    /// Attach the `.dbf` attribute row to each feature's `properties`.
    pub properties: bool,
    /// Comma-separated attribute columns to keep/reorder, by name or 0-based
    /// index. Empty = every column.
    pub columns: String,
    pub encoding: Encoding,
    /// Which `.shp` inside a zip: base name (no extension), case-insensitive.
    /// Empty = the first one in name order.
    pub layer: String,
    /// Emit a top-level `bbox` on the FeatureCollection.
    pub bbox: bool,
    /// Keep Z from PointZ/PolyLineZ/PolygonZ as a third coordinate.
    pub include_z: bool,
    /// Rewind rings to RFC 7946 winding (exterior CCW, holes CW).
    pub rewind: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            output: Output::Geojson,
            pretty: false,
            precision: 6,
            limit: 0,
            properties: true,
            columns: String::new(),
            encoding: Encoding::Auto,
            layer: String::new(),
            bbox: true,
            include_z: true,
            rewind: true,
        }
    }
}

/// The converted document plus what the caller needs to describe it.
#[derive(Debug, Clone)]
pub struct Conversion {
    /// The GeoJSON (or newline-delimited GeoJSON) text.
    pub geojson: String,
    /// Features actually emitted (after `limit`).
    pub feature_count: usize,
    /// Records present in the `.shp` before `limit`.
    pub total_records: usize,
    /// Human name of the shapefile's declared shape type, e.g. `"PolygonZ"`.
    pub shape_type: String,
    /// Base name of the `.shp` that was converted.
    pub layer: String,
    /// Every `.shp` base name found in the archive (1 entry for a bare `.shp`).
    pub layers: Vec<String>,
    /// Coordinate system named by the `.prj`, if one was present.
    pub crs: Option<String>,
    /// Non-fatal notes: missing `.dbf`, projected CRS, truncated tail, …
    pub warnings: Vec<String>,
}

/// Cap on a single decompressed member read out of a zip, so a small archive
/// cannot expand into an unbounded allocation.
const MAX_MEMBER_BYTES: u64 = 64 * 1024 * 1024;

const SHP_HEADER_LEN: usize = 100;
const SHP_FILE_CODE: i32 = 9994;
/// Shapefile "no data" sentinel: any value below this means absent (used for M,
/// and for Z in files that carry a Z range but no real elevations).
const NO_DATA: f64 = -1e38;

// ---------------------------------------------------------------------------
// Geometry model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
struct Pt {
    x: f64,
    y: f64,
    z: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
enum Geom {
    Null,
    Point(Pt),
    MultiPoint(Vec<Pt>),
    /// One entry per part.
    PolyLine(Vec<Vec<Pt>>),
    /// One entry per ring, orientation-significant (CW outer, CCW hole).
    Polygon(Vec<Vec<Pt>>),
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Convert `bytes` — a `.zip` of a shapefile set, or a bare `.shp` — to GeoJSON.
/// Returns `Err` with an actionable message on unreadable input.
pub fn convert(bytes: &[u8], opts: &Options) -> Result<Conversion, String> {
    let parts = load_parts(bytes, &opts.layer)?;
    convert_parts(&parts, opts)
}

/// The files a conversion needs, however they were sourced.
#[derive(Debug, Default, Clone)]
struct Parts {
    shp: Vec<u8>,
    dbf: Option<Vec<u8>>,
    prj: Option<String>,
    cpg: Option<String>,
    layer: String,
    layers: Vec<String>,
    warnings: Vec<String>,
}

fn load_parts(bytes: &[u8], want_layer: &str) -> Result<Parts, String> {
    if bytes.is_empty() {
        return Err("empty input: expected a .zip of a shapefile set, or a bare .shp".to_string());
    }
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        load_from_zip(bytes, want_layer)
    } else if is_shp(bytes) {
        Ok(Parts {
            shp: bytes.to_vec(),
            layer: "shapefile".to_string(),
            layers: vec!["shapefile".to_string()],
            warnings: vec![
                "no .dbf available (a bare .shp was uploaded), so features have empty properties — upload the shapefile set as a .zip to keep attributes".to_string(),
            ],
            ..Parts::default()
        })
    } else {
        Err(format!(
            "unrecognised input: expected a .zip containing a shapefile set, or a .shp whose header starts with the file code {SHP_FILE_CODE} (got bytes {:02x?})",
            &bytes[..bytes.len().min(4)]
        ))
    }
}

fn is_shp(bytes: &[u8]) -> bool {
    bytes.len() >= SHP_HEADER_LEN && be_i32(bytes, 0) == Ok(SHP_FILE_CODE)
}

/// Lowercase extension of a zip entry path, without the dot.
fn ext_of(name: &str) -> String {
    let base = name.rsplit('/').next().unwrap_or(name);
    match base.rsplit_once('.') {
        Some((_, e)) => e.to_ascii_lowercase(),
        None => String::new(),
    }
}

/// Path with its extension removed — the shapefile "stem" that ties a set together.
fn stem_of(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((s, _)) => s.to_string(),
        None => name.to_string(),
    }
}

/// Last path segment of a stem, i.e. the user-facing layer name.
fn base_of(stem: &str) -> String {
    stem.rsplit('/').next().unwrap_or(stem).to_string()
}

/// macOS resource-fork litter that otherwise shadows the real members.
fn is_junk(name: &str) -> bool {
    name.starts_with("__MACOSX/")
        || name.contains("/__MACOSX/")
        || name.rsplit('/').next().unwrap_or(name).starts_with("._")
}

fn load_from_zip(bytes: &[u8], want_layer: &str) -> Result<Parts, String> {
    let reader = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader)
        .map_err(|e| format!("could not read the .zip archive: {e}"))?;

    // Index the archive first: names only, so the pick is deterministic before
    // anything is decompressed.
    let mut names: Vec<String> = Vec::new();
    for i in 0..zip.len() {
        let name = match zip.by_index_raw(i) {
            Ok(f) => f.name().to_string(),
            Err(_) => continue,
        };
        if !is_junk(&name) {
            names.push(name);
        }
    }

    let mut shp_stems: Vec<String> = names
        .iter()
        .filter(|n| ext_of(n) == "shp")
        .map(|n| stem_of(n))
        .collect();
    shp_stems.sort();
    shp_stems.dedup();

    if shp_stems.is_empty() {
        return Err(format!(
            "no .shp member in the archive — a shapefile set needs one (found: {})",
            summarize_names(&names)
        ));
    }

    let layers: Vec<String> = shp_stems.iter().map(|s| base_of(s)).collect();
    let want = want_layer.trim();
    let stem = if want.is_empty() {
        shp_stems[0].clone()
    } else {
        shp_stems
            .iter()
            .find(|s| base_of(s).eq_ignore_ascii_case(want) || s.eq_ignore_ascii_case(&want))
            .cloned()
            .ok_or_else(|| {
                format!(
                    "no layer named {want:?} in the archive; available layers: {}",
                    layers.join(", ")
                )
            })?
    };

    let mut warnings = Vec::new();
    if want.is_empty() && shp_stems.len() > 1 {
        warnings.push(format!(
            "the archive holds {} layers ({}); converted {:?} — set `layer` to pick another",
            shp_stems.len(),
            layers.join(", "),
            base_of(&stem)
        ));
    }

    let sibling = |ext: &str| -> Option<String> {
        names
            .iter()
            .find(|n| stem_of(n).eq_ignore_ascii_case(&stem) && ext_of(n) == ext)
            .cloned()
    };

    let shp_name = sibling("shp").ok_or_else(|| "no .shp member in the archive".to_string())?;
    let shp = read_member(&mut zip, &shp_name)?;
    let dbf = match sibling("dbf") {
        Some(n) => Some(read_member(&mut zip, &n)?),
        None => {
            warnings.push(format!(
                "the archive has no {}.dbf, so features have empty properties",
                base_of(&stem)
            ));
            None
        }
    };
    let prj = match sibling("prj") {
        Some(n) => String::from_utf8(read_member(&mut zip, &n)?).ok(),
        None => None,
    };
    let cpg = match sibling("cpg") {
        Some(n) => String::from_utf8(read_member(&mut zip, &n)?)
            .ok()
            .map(|s| s.trim().to_string()),
        None => None,
    };

    Ok(Parts {
        shp,
        dbf,
        prj,
        cpg,
        layer: base_of(&stem),
        layers,
        warnings,
    })
}

fn summarize_names(names: &[String]) -> String {
    if names.is_empty() {
        return "the archive is empty".to_string();
    }
    let shown: Vec<&str> = names.iter().take(8).map(|s| s.as_str()).collect();
    if names.len() > shown.len() {
        format!("{}, … ({} entries)", shown.join(", "), names.len())
    } else {
        shown.join(", ")
    }
}

fn read_member(
    zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    name: &str,
) -> Result<Vec<u8>, String> {
    let f = zip
        .by_name(name)
        .map_err(|e| format!("could not open {name} in the archive: {e}"))?;
    let declared = f.size();
    if declared > MAX_MEMBER_BYTES {
        return Err(format!(
            "{name} expands to {declared} bytes, over the {MAX_MEMBER_BYTES}-byte limit for one archive member"
        ));
    }
    let mut buf = Vec::with_capacity(declared.min(1 << 20) as usize);
    f.take(MAX_MEMBER_BYTES + 1)
        .read_to_end(&mut buf)
        .map_err(|e| format!("could not decompress {name}: {e}"))?;
    if buf.len() as u64 > MAX_MEMBER_BYTES {
        return Err(format!(
            "{name} expands past the {MAX_MEMBER_BYTES}-byte limit for one archive member"
        ));
    }
    Ok(buf)
}

// ---------------------------------------------------------------------------
// Byte readers (the .shp mixes big-endian header fields with little-endian data)
// ---------------------------------------------------------------------------

fn be_i32(b: &[u8], o: usize) -> Result<i32, String> {
    let s = b
        .get(o..o + 4)
        .ok_or_else(|| format!("truncated .shp: wanted 4 bytes at offset {o}, file ends first"))?;
    Ok(i32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn le_i32(b: &[u8], o: usize) -> Result<i32, String> {
    let s = b
        .get(o..o + 4)
        .ok_or_else(|| format!("truncated shape record: wanted 4 bytes at offset {o}"))?;
    Ok(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn le_f64(b: &[u8], o: usize) -> Result<f64, String> {
    let s = b
        .get(o..o + 8)
        .ok_or_else(|| format!("truncated shape record: wanted 8 bytes at offset {o}"))?;
    Ok(f64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}

/// Shapefile type code → the name used in errors and the summary.
fn shape_type_name(code: i32) -> &'static str {
    match code {
        0 => "Null",
        1 => "Point",
        3 => "PolyLine",
        5 => "Polygon",
        8 => "MultiPoint",
        11 => "PointZ",
        13 => "PolyLineZ",
        15 => "PolygonZ",
        18 => "MultiPointZ",
        21 => "PointM",
        23 => "PolyLineM",
        25 => "PolygonM",
        28 => "MultiPointM",
        31 => "MultiPatch",
        _ => "Unknown",
    }
}

// ---------------------------------------------------------------------------
// .shp record parsing
// ---------------------------------------------------------------------------

/// Read every shape record. Returns the shapes plus any non-fatal warning (a
/// truncated tail stops the scan instead of failing the whole file).
fn read_shapes(shp: &[u8]) -> Result<(i32, Vec<Geom>, Vec<String>), String> {
    if shp.len() < SHP_HEADER_LEN {
        return Err(format!(
            "not a valid .shp file: header is {} bytes, need at least {SHP_HEADER_LEN}",
            shp.len()
        ));
    }
    if be_i32(shp, 0)? != SHP_FILE_CODE {
        return Err(format!(
            "not a valid .shp file: header file code is {}, expected {SHP_FILE_CODE}",
            be_i32(shp, 0)?
        ));
    }
    let header_type = le_i32(shp, 32)?;
    if shape_type_name(header_type) == "Unknown" {
        return Err(format!(
            "unsupported .shp shape type {header_type} in the file header"
        ));
    }
    if header_type == 31 {
        return Err(
            "MultiPatch (shape type 31) shapefiles are not supported: they store 3D triangle strips/fans, which GeoJSON has no geometry type for".to_string(),
        );
    }

    // The header's file length is in 16-bit words and counts the header itself.
    // Trust it only when it fits inside the bytes we actually have.
    let declared = be_i32(shp, 24)? as i64 * 2;
    let mut warnings = Vec::new();
    let end = if declared >= SHP_HEADER_LEN as i64 && (declared as usize) <= shp.len() {
        declared as usize
    } else {
        if declared > shp.len() as i64 {
            warnings.push(format!(
                "the .shp header declares {declared} bytes but the file is {} — reading what is present",
                shp.len()
            ));
        }
        shp.len()
    };

    let mut shapes = Vec::new();
    let mut off = SHP_HEADER_LEN;
    while off + 8 <= end {
        let content_words = be_i32(shp, off + 4)?;
        if content_words < 0 {
            return Err(format!(
                "corrupt .shp: record {} declares a negative content length",
                shapes.len() + 1
            ));
        }
        let content_len = content_words as usize * 2;
        let start = off + 8;
        if start + content_len > end {
            warnings.push(format!(
                "the .shp ends mid-record after {} shapes — the truncated tail was skipped",
                shapes.len()
            ));
            break;
        }
        // A zero-length record carries no shape type at all; treat it as Null.
        let geom = if content_len < 4 {
            Geom::Null
        } else {
            parse_shape(&shp[start..start + content_len], shapes.len() + 1)?
        };
        shapes.push(geom);
        off = start + content_len;
    }

    Ok((header_type, shapes, warnings))
}

fn parse_shape(c: &[u8], record_no: usize) -> Result<Geom, String> {
    let t = le_i32(c, 0)?;
    let bad = |e: String| format!("record {record_no}: {e}");
    match t {
        0 => Ok(Geom::Null),
        1 => Ok(Geom::Point(Pt {
            x: le_f64(c, 4).map_err(bad)?,
            y: le_f64(c, 12).map_err(bad)?,
            z: None,
        })),
        21 => Ok(Geom::Point(Pt {
            // PointM: x, y, m — the measure has no GeoJSON slot, so it is dropped.
            x: le_f64(c, 4).map_err(bad)?,
            y: le_f64(c, 12).map_err(bad)?,
            z: None,
        })),
        11 => Ok(Geom::Point(Pt {
            x: le_f64(c, 4).map_err(bad)?,
            y: le_f64(c, 12).map_err(bad)?,
            z: keep_z(le_f64(c, 20).map_err(bad)?),
        })),
        8 | 18 | 28 => parse_multipoint(c, t).map_err(bad),
        3 | 13 | 23 => Ok(Geom::PolyLine(parse_parts(c, t).map_err(bad)?)),
        5 | 15 | 25 => Ok(Geom::Polygon(parse_parts(c, t).map_err(bad)?)),
        31 => Err(bad(
            "MultiPatch (shape type 31) is not supported: it stores 3D triangle strips/fans, which GeoJSON has no geometry type for".to_string(),
        )),
        other => Err(bad(format!("unsupported shape type {other}"))),
    }
}

/// Shapefile stores "no measure/elevation" as a large negative sentinel.
fn keep_z(z: f64) -> Option<f64> {
    if z.is_finite() && z > NO_DATA {
        Some(z)
    } else {
        None
    }
}

fn parse_multipoint(c: &[u8], t: i32) -> Result<Geom, String> {
    // [type][bbox: 4 f64][numPoints: i32][points: 2 f64 each]
    let n = le_i32(c, 36)?;
    if n < 0 {
        return Err("negative point count".to_string());
    }
    let n = n as usize;
    let mut pts = Vec::with_capacity(n);
    for i in 0..n {
        pts.push(Pt {
            x: le_f64(c, 40 + i * 16)?,
            y: le_f64(c, 48 + i * 16)?,
            z: None,
        });
    }
    // MultiPointZ appends [Zmin, Zmax][Z per point] after the XY block.
    if t == 18 {
        let zbase = 40 + n * 16 + 16;
        for (i, p) in pts.iter_mut().enumerate() {
            if let Ok(z) = le_f64(c, zbase + i * 8) {
                p.z = keep_z(z);
            }
        }
    }
    Ok(Geom::MultiPoint(pts))
}

/// Shared reader for the part-indexed layout used by PolyLine and Polygon (and
/// their Z/M variants), which are byte-identical apart from the type code.
fn parse_parts(c: &[u8], t: i32) -> Result<Vec<Vec<Pt>>, String> {
    // [type][bbox: 4 f64][numParts: i32][numPoints: i32][parts: i32 each][points]
    let num_parts = le_i32(c, 36)?;
    let num_points = le_i32(c, 40)?;
    if num_parts < 0 || num_points < 0 {
        return Err("negative part/point count".to_string());
    }
    let (num_parts, num_points) = (num_parts as usize, num_points as usize);

    let parts_base = 44;
    let points_base = parts_base + num_parts * 4;
    let mut starts = Vec::with_capacity(num_parts);
    for i in 0..num_parts {
        let s = le_i32(c, parts_base + i * 4)?;
        if s < 0 || s as usize > num_points {
            return Err(format!(
                "part {i} starts at point index {s}, outside the {num_points} points in the record"
            ));
        }
        starts.push(s as usize);
    }

    let mut xy = Vec::with_capacity(num_points);
    for i in 0..num_points {
        xy.push(Pt {
            x: le_f64(c, points_base + i * 16)?,
            y: le_f64(c, points_base + i * 16 + 8)?,
            z: None,
        });
    }
    // PolyLineZ / PolygonZ append [Zmin, Zmax][Z per point] after the XY block.
    if t == 13 || t == 15 {
        let zbase = points_base + num_points * 16 + 16;
        for (i, p) in xy.iter_mut().enumerate() {
            if let Ok(z) = le_f64(c, zbase + i * 8) {
                p.z = keep_z(z);
            }
        }
    }

    // Slice the flat point run into parts using the start indices.
    let mut out = Vec::with_capacity(num_parts);
    for (i, &s) in starts.iter().enumerate() {
        let e = starts.get(i + 1).copied().unwrap_or(num_points);
        if e > s {
            out.push(xy[s..e].to_vec());
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Rings → GeoJSON polygons
// ---------------------------------------------------------------------------

/// Shoelace area. Positive = counter-clockwise, negative = clockwise. Shapefile
/// outer rings are clockwise; GeoJSON (RFC 7946) exterior rings are the opposite.
fn signed_area(ring: &[Pt]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        s += a.x * b.y - b.x * a.y;
    }
    s / 2.0
}

/// Regroup a shapefile's flat ring list into polygons: each clockwise ring opens
/// a new polygon and the counter-clockwise rings that follow are its holes.
fn group_rings(rings: &[Vec<Pt>]) -> Vec<Vec<Vec<Pt>>> {
    // Some writers emit every ring counter-clockwise. With no clockwise ring to
    // anchor on, treating them all as holes would drop the whole feature, so fall
    // back to one polygon per ring.
    if !rings.iter().any(|r| signed_area(r) < 0.0) {
        return rings.iter().map(|r| vec![r.clone()]).collect();
    }
    let mut polys: Vec<Vec<Vec<Pt>>> = Vec::new();
    for ring in rings {
        if signed_area(ring) < 0.0 || polys.is_empty() {
            polys.push(vec![ring.clone()]);
        } else {
            polys.last_mut().expect("non-empty").push(ring.clone());
        }
    }
    polys
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

/// Round to `precision` decimals via formatting, so the result is the shortest
/// float that prints as those digits (arithmetic rounding leaves 1e-16 fuzz that
/// serializes back as `1.2345670000000001`).
fn round(v: f64, precision: i64) -> f64 {
    if precision < 0 || !v.is_finite() {
        return v;
    }
    let r: f64 = format!("{:.*}", precision.min(17) as usize, v)
        .parse()
        .unwrap_or(v);
    // Normalize -0.0, which prints as "-0.0" and reads oddly in coordinates.
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

fn num(v: f64) -> Result<Value, String> {
    Number::from_f64(v)
        .map(Value::Number)
        .ok_or_else(|| format!("coordinate {v} is not a finite number"))
}

fn pos(p: &Pt, opts: &Options) -> Result<Value, String> {
    let mut a = vec![
        num(round(p.x, opts.precision))?,
        num(round(p.y, opts.precision))?,
    ];
    if opts.include_z {
        if let Some(z) = p.z {
            a.push(num(round(z, opts.precision))?);
        }
    }
    Ok(Value::Array(a))
}

fn ring_json(ring: &[Pt], opts: &Options, want_ccw: bool) -> Result<Value, String> {
    let mut pts: Vec<Pt> = ring.to_vec();
    // GeoJSON rings must be explicitly closed; shapefile rings normally are, but
    // re-close defensively rather than emit an invalid ring.
    if let (Some(&first), Some(&last)) = (pts.first(), pts.last()) {
        if first.x != last.x || first.y != last.y {
            pts.push(first);
        }
    }
    if opts.rewind {
        let ccw = signed_area(&pts) > 0.0;
        if ccw != want_ccw {
            pts.reverse();
        }
    }
    Ok(Value::Array(
        pts.iter().map(|p| pos(p, opts)).collect::<Result<_, _>>()?,
    ))
}

fn geom_json(g: &Geom, opts: &Options) -> Result<Value, String> {
    Ok(match g {
        Geom::Null => Value::Null,
        Geom::Point(p) => json!({ "type": "Point", "coordinates": pos(p, opts)? }),
        Geom::MultiPoint(pts) if pts.is_empty() => Value::Null,
        Geom::MultiPoint(pts) => json!({
            "type": "MultiPoint",
            "coordinates": pts.iter().map(|p| pos(p, opts)).collect::<Result<Vec<_>, _>>()?,
        }),
        Geom::PolyLine(parts) if parts.is_empty() => Value::Null,
        Geom::PolyLine(parts) => {
            let lines: Vec<Value> = parts
                .iter()
                .map(|part| {
                    Ok(Value::Array(
                        part.iter()
                            .map(|p| pos(p, opts))
                            .collect::<Result<_, _>>()?,
                    ))
                })
                .collect::<Result<_, String>>()?;
            if lines.len() == 1 {
                json!({ "type": "LineString", "coordinates": lines.into_iter().next() })
            } else {
                json!({ "type": "MultiLineString", "coordinates": lines })
            }
        }
        Geom::Polygon(rings) if rings.is_empty() => Value::Null,
        Geom::Polygon(rings) => {
            let polys = group_rings(rings);
            let coords: Vec<Value> = polys
                .iter()
                .map(|p| {
                    let mut out = Vec::with_capacity(p.len());
                    for (i, ring) in p.iter().enumerate() {
                        // First ring is the exterior (CCW in RFC 7946), rest are holes.
                        out.push(ring_json(ring, opts, i == 0)?);
                    }
                    Ok(Value::Array(out))
                })
                .collect::<Result<_, String>>()?;
            if coords.len() == 1 {
                json!({ "type": "Polygon", "coordinates": coords.into_iter().next() })
            } else {
                json!({ "type": "MultiPolygon", "coordinates": coords })
            }
        }
    })
}

/// Grow `bb` = [minX, minY, maxX, maxY] to cover every position in `v`.
fn extend_bbox(bb: &mut [f64; 4], v: &Value) {
    match v {
        Value::Array(a) => {
            // A position is an array of >= 2 plain numbers; anything else nests.
            if a.len() >= 2 && a.iter().all(|e| e.is_number()) {
                let (x, y) = (
                    a[0].as_f64().unwrap_or(f64::NAN),
                    a[1].as_f64().unwrap_or(f64::NAN),
                );
                if x.is_finite() && y.is_finite() {
                    bb[0] = bb[0].min(x);
                    bb[1] = bb[1].min(y);
                    bb[2] = bb[2].max(x);
                    bb[3] = bb[3].max(y);
                }
            } else {
                for e in a {
                    extend_bbox(bb, e);
                }
            }
        }
        Value::Object(o) => {
            if let Some(c) = o.get("coordinates") {
                extend_bbox(bb, c);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Attributes (.dbf) and CRS (.prj)
// ---------------------------------------------------------------------------

/// Pull the attribute rows out of the `.dbf`, one per `.shp` record.
///
/// `include_deleted` is forced ON: shapefile record N pairs with DBF record N by
/// position, and silently dropping deleted rows would shift every later feature's
/// attributes onto the wrong geometry.
fn read_attributes(
    dbf: &[u8],
    opts: &Options,
    cpg: Option<&str>,
) -> Result<Vec<Map<String, Value>>, String> {
    let dbf_opts = DbfOptions {
        format: DbfFormat::Json,
        delimiter: ',',
        header: true,
        columns: opts.columns.clone(),
        include_deleted: true,
        trim: true,
        encoding: dbf_encoding(opts.encoding, cpg),
        limit: 0,
    };
    let text = parse_dbf(dbf, &dbf_opts).map_err(|e| format!("attribute table (.dbf): {e}"))?;
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| format!("attribute table (.dbf) produced unreadable JSON: {e}"))?;
    Ok(v.get("rows")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|r| r.as_object().cloned().unwrap_or_default())
                .collect()
        })
        .unwrap_or_default())
}

/// Resolve the DBF text encoding, letting a `.cpg` sidecar win when the caller
/// asked for `auto` (that file exists precisely to declare the code page).
fn dbf_encoding(enc: Encoding, cpg: Option<&str>) -> DbfEncoding {
    match enc {
        Encoding::Utf8 => DbfEncoding::Utf8,
        Encoding::Latin1 => DbfEncoding::Latin1,
        Encoding::Cp1252 => DbfEncoding::Cp1252,
        Encoding::Auto => match cpg.map(|c| c.trim().to_ascii_uppercase()) {
            Some(c) if c.contains("UTF-8") || c.contains("UTF8") || c.contains("65001") => {
                DbfEncoding::Utf8
            }
            Some(c) if c.contains("1252") => DbfEncoding::Cp1252,
            Some(c) if c.contains("8859-1") || c.contains("LATIN1") || c.contains("ISO88591") => {
                DbfEncoding::Latin1
            }
            _ => DbfEncoding::Auto,
        },
    }
}

/// Name the `.prj`'s coordinate system, and flag the common gotcha: GeoJSON
/// (RFC 7946) is defined in WGS 84 lon/lat, but plenty of published shapefiles
/// are in a projected CRS whose coordinates are metres, not degrees.
fn read_crs(prj: &str) -> (Option<String>, Option<String>) {
    let trimmed = prj.trim();
    if trimmed.is_empty() {
        return (None, None);
    }
    let projected = trimmed.starts_with("PROJCS");
    let name = trimmed
        .split_once('"')
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(n, _)| n.to_string());
    let warn = if projected {
        Some(format!(
            "the .prj declares the projected coordinate system {} — its coordinates are linear units (usually metres), not WGS 84 longitude/latitude, so the GeoJSON will not line up on a web map until it is reprojected to EPSG:4326",
            name.clone().unwrap_or_else(|| "(unnamed)".to_string())
        ))
    } else {
        None
    };
    (name, warn)
}

// ---------------------------------------------------------------------------
// Assembly
// ---------------------------------------------------------------------------

fn convert_parts(parts: &Parts, opts: &Options) -> Result<Conversion, String> {
    let (header_type, shapes, shp_warnings) = read_shapes(&parts.shp)?;
    let mut warnings = parts.warnings.clone();
    warnings.extend(shp_warnings);

    let (crs, crs_warning) = match &parts.prj {
        Some(p) => read_crs(p),
        None => (None, None),
    };
    if let Some(w) = crs_warning {
        warnings.push(w);
    }

    let attrs = match (&parts.dbf, opts.properties) {
        (Some(dbf), true) => {
            let rows = read_attributes(dbf, opts, parts.cpg.as_deref())?;
            if rows.len() != shapes.len() {
                warnings.push(format!(
                    "the .dbf has {} attribute rows but the .shp has {} shapes — features past row {} carry empty properties",
                    rows.len(),
                    shapes.len(),
                    rows.len()
                ));
            }
            rows
        }
        _ => Vec::new(),
    };

    let total_records = shapes.len();
    let take = if opts.limit == 0 {
        total_records
    } else {
        opts.limit.min(total_records)
    };

    let mut features = Vec::with_capacity(take);
    let mut bb = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for (i, g) in shapes.iter().take(take).enumerate() {
        let geometry = geom_json(g, opts)?;
        if opts.bbox {
            extend_bbox(&mut bb, &geometry);
        }
        let props = attrs.get(i).cloned().unwrap_or_default();
        features.push(json!({
            "type": "Feature",
            "geometry": geometry,
            "properties": Value::Object(props),
        }));
    }

    let has_bbox = opts.bbox && bb[0].is_finite() && bb[2].is_finite();
    let geojson = match opts.output {
        Output::Ndjson => features
            .iter()
            .map(|f| serde_json::to_string(f).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n"),
        Output::Geojson => {
            let mut root = Map::new();
            root.insert("type".into(), json!("FeatureCollection"));
            if has_bbox {
                root.insert(
                    "bbox".into(),
                    json!([
                        num(round(bb[0], opts.precision))?,
                        num(round(bb[1], opts.precision))?,
                        num(round(bb[2], opts.precision))?,
                        num(round(bb[3], opts.precision))?,
                    ]),
                );
            }
            root.insert("features".into(), Value::Array(features.clone()));
            let root = Value::Object(root);
            if opts.pretty {
                serde_json::to_string_pretty(&root)
            } else {
                serde_json::to_string(&root)
            }
            .map_err(|e| format!("could not serialize the GeoJSON: {e}"))?
        }
    };

    Ok(Conversion {
        geojson,
        feature_count: features.len(),
        total_records,
        shape_type: shape_type_name(header_type).to_string(),
        layer: parts.layer.clone(),
        layers: parts.layers.clone(),
        crs,
        warnings,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a .shp file: 100-byte header + one record per shape content blob.
    fn shp_file(shape_type: i32, records: &[Vec<u8>]) -> Vec<u8> {
        let mut body = Vec::new();
        for (i, c) in records.iter().enumerate() {
            body.extend_from_slice(&((i as i32) + 1).to_be_bytes());
            body.extend_from_slice(&((c.len() / 2) as i32).to_be_bytes());
            body.extend_from_slice(c);
        }
        let mut f = vec![0u8; SHP_HEADER_LEN];
        f[0..4].copy_from_slice(&SHP_FILE_CODE.to_be_bytes());
        let words = ((SHP_HEADER_LEN + body.len()) / 2) as i32;
        f[24..28].copy_from_slice(&words.to_be_bytes());
        f[28..32].copy_from_slice(&1000i32.to_le_bytes());
        f[32..36].copy_from_slice(&shape_type.to_le_bytes());
        f.extend_from_slice(&body);
        f
    }

    fn point_rec(x: f64, y: f64) -> Vec<u8> {
        let mut c = 1i32.to_le_bytes().to_vec();
        c.extend_from_slice(&x.to_le_bytes());
        c.extend_from_slice(&y.to_le_bytes());
        c
    }

    fn pointz_rec(x: f64, y: f64, z: f64) -> Vec<u8> {
        let mut c = 11i32.to_le_bytes().to_vec();
        c.extend_from_slice(&x.to_le_bytes());
        c.extend_from_slice(&y.to_le_bytes());
        c.extend_from_slice(&z.to_le_bytes());
        c.extend_from_slice(&0f64.to_le_bytes()); // measure
        c
    }

    /// Part-indexed record (PolyLine/Polygon and their Z variants).
    fn parts_rec(shape_type: i32, parts: &[Vec<(f64, f64)>]) -> Vec<u8> {
        let all: Vec<(f64, f64)> = parts.iter().flatten().copied().collect();
        let mut c = shape_type.to_le_bytes().to_vec();
        for v in [0f64, 0.0, 0.0, 0.0] {
            c.extend_from_slice(&v.to_le_bytes()); // bbox
        }
        c.extend_from_slice(&(parts.len() as i32).to_le_bytes());
        c.extend_from_slice(&(all.len() as i32).to_le_bytes());
        let mut start = 0i32;
        for p in parts {
            c.extend_from_slice(&start.to_le_bytes());
            start += p.len() as i32;
        }
        for (x, y) in &all {
            c.extend_from_slice(&x.to_le_bytes());
            c.extend_from_slice(&y.to_le_bytes());
        }
        c
    }

    fn multipoint_rec(pts: &[(f64, f64)]) -> Vec<u8> {
        let mut c = 8i32.to_le_bytes().to_vec();
        for v in [0f64, 0.0, 0.0, 0.0] {
            c.extend_from_slice(&v.to_le_bytes());
        }
        c.extend_from_slice(&(pts.len() as i32).to_le_bytes());
        for (x, y) in pts {
            c.extend_from_slice(&x.to_le_bytes());
            c.extend_from_slice(&y.to_le_bytes());
        }
        c
    }

    /// A clockwise square (shapefile outer-ring winding).
    fn cw_square(x: f64, y: f64, s: f64) -> Vec<(f64, f64)> {
        vec![(x, y), (x, y + s), (x + s, y + s), (x + s, y), (x, y)]
    }

    /// A counter-clockwise square (shapefile hole winding).
    fn ccw_square(x: f64, y: f64, s: f64) -> Vec<(f64, f64)> {
        let mut v = cw_square(x, y, s);
        v.reverse();
        v
    }

    fn parse(shp: Vec<u8>, opts: &Options) -> Conversion {
        convert_parts(
            &Parts {
                shp,
                layer: "t".into(),
                layers: vec!["t".into()],
                ..Parts::default()
            },
            opts,
        )
        .unwrap()
    }

    #[test]
    fn points_become_a_feature_collection() {
        let c = parse(
            shp_file(
                1,
                &[point_rec(-122.4194, 37.7749), point_rec(2.3522, 48.8566)],
            ),
            &Options::default(),
        );
        assert_eq!(c.feature_count, 2);
        assert_eq!(c.total_records, 2);
        assert_eq!(c.shape_type, "Point");
        let v: Value = serde_json::from_str(&c.geojson).unwrap();
        assert_eq!(v["type"], "FeatureCollection");
        assert_eq!(v["features"][0]["geometry"]["type"], "Point");
        assert_eq!(v["features"][0]["geometry"]["coordinates"][0], -122.4194);
        assert_eq!(v["features"][0]["geometry"]["coordinates"][1], 37.7749);
        // No .dbf → properties present but empty (RFC 7946 requires the member).
        assert!(v["features"][0]["properties"]
            .as_object()
            .unwrap()
            .is_empty());
        assert_eq!(v["bbox"], json!([-122.4194, 37.7749, 2.3522, 48.8566]));
    }

    #[test]
    fn error_on_non_shapefile_input() {
        let err = convert(b"hello world, not a shapefile", &Options::default()).unwrap_err();
        assert!(err.contains("unrecognised input"), "got: {err}");
        assert!(err.contains("9994"), "got: {err}");
    }

    #[test]
    fn error_on_empty_input() {
        let err = convert(&[], &Options::default()).unwrap_err();
        assert!(err.contains("empty input"), "got: {err}");
    }

    #[test]
    fn bare_shp_warns_about_missing_attributes() {
        let c = convert(&shp_file(1, &[point_rec(1.0, 2.0)]), &Options::default()).unwrap();
        assert_eq!(c.feature_count, 1);
        assert!(
            c.warnings.iter().any(|w| w.contains("no .dbf available")),
            "got: {:?}",
            c.warnings
        );
    }

    #[test]
    fn single_part_polyline_is_a_linestring() {
        let c = parse(
            shp_file(
                3,
                &[parts_rec(3, &[vec![(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)]])],
            ),
            &Options::default(),
        );
        let v: Value = serde_json::from_str(&c.geojson).unwrap();
        assert_eq!(v["features"][0]["geometry"]["type"], "LineString");
        assert_eq!(
            v["features"][0]["geometry"]["coordinates"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn multi_part_polyline_is_a_multilinestring() {
        let c = parse(
            shp_file(
                3,
                &[parts_rec(
                    3,
                    &[vec![(0.0, 0.0), (1.0, 1.0)], vec![(5.0, 5.0), (6.0, 6.0)]],
                )],
            ),
            &Options::default(),
        );
        let v: Value = serde_json::from_str(&c.geojson).unwrap();
        assert_eq!(v["features"][0]["geometry"]["type"], "MultiLineString");
        assert_eq!(
            v["features"][0]["geometry"]["coordinates"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn polygon_with_a_hole_keeps_one_polygon_and_two_rings() {
        let c = parse(
            shp_file(
                5,
                &[parts_rec(
                    5,
                    &[cw_square(0.0, 0.0, 10.0), ccw_square(2.0, 2.0, 3.0)],
                )],
            ),
            &Options::default(),
        );
        let v: Value = serde_json::from_str(&c.geojson).unwrap();
        let g = &v["features"][0]["geometry"];
        assert_eq!(g["type"], "Polygon");
        assert_eq!(
            g["coordinates"].as_array().unwrap().len(),
            2,
            "outer + hole"
        );
    }

    #[test]
    fn two_outer_rings_become_a_multipolygon() {
        let c = parse(
            shp_file(
                5,
                &[parts_rec(
                    5,
                    &[cw_square(0.0, 0.0, 1.0), cw_square(5.0, 5.0, 1.0)],
                )],
            ),
            &Options::default(),
        );
        let v: Value = serde_json::from_str(&c.geojson).unwrap();
        let g = &v["features"][0]["geometry"];
        assert_eq!(g["type"], "MultiPolygon");
        assert_eq!(g["coordinates"].as_array().unwrap().len(), 2);
    }

    /// RFC 7946 §3.1.6: exterior rings counter-clockwise, holes clockwise — the
    /// opposite of the shapefile convention, so the default must flip both.
    #[test]
    fn rewind_flips_shapefile_winding_to_rfc7946() {
        let c = parse(
            shp_file(
                5,
                &[parts_rec(
                    5,
                    &[cw_square(0.0, 0.0, 10.0), ccw_square(2.0, 2.0, 3.0)],
                )],
            ),
            &Options::default(),
        );
        let v: Value = serde_json::from_str(&c.geojson).unwrap();
        let rings = v["features"][0]["geometry"]["coordinates"]
            .as_array()
            .unwrap();
        let area = |r: &Value| {
            let pts: Vec<Pt> = r
                .as_array()
                .unwrap()
                .iter()
                .map(|p| Pt {
                    x: p[0].as_f64().unwrap(),
                    y: p[1].as_f64().unwrap(),
                    z: None,
                })
                .collect();
            signed_area(&pts)
        };
        assert!(area(&rings[0]) > 0.0, "exterior must be counter-clockwise");
        assert!(area(&rings[1]) < 0.0, "hole must be clockwise");
    }

    #[test]
    fn rewind_off_preserves_the_source_winding() {
        let opts = Options {
            rewind: false,
            ..Options::default()
        };
        let c = parse(
            shp_file(5, &[parts_rec(5, &[cw_square(0.0, 0.0, 10.0)])]),
            &opts,
        );
        let v: Value = serde_json::from_str(&c.geojson).unwrap();
        let ring = &v["features"][0]["geometry"]["coordinates"][0];
        assert_eq!(ring[0], json!([0.0, 0.0]));
        assert_eq!(ring[1], json!([0.0, 10.0]), "still clockwise as stored");
    }

    #[test]
    fn all_ccw_rings_fall_back_to_one_polygon_each() {
        let c = parse(
            shp_file(
                5,
                &[parts_rec(
                    5,
                    &[ccw_square(0.0, 0.0, 1.0), ccw_square(5.0, 5.0, 1.0)],
                )],
            ),
            &Options::default(),
        );
        let v: Value = serde_json::from_str(&c.geojson).unwrap();
        assert_eq!(v["features"][0]["geometry"]["type"], "MultiPolygon");
    }

    #[test]
    fn multipoint_round_trips() {
        let c = parse(
            shp_file(8, &[multipoint_rec(&[(1.0, 2.0), (3.0, 4.0)])]),
            &Options::default(),
        );
        let v: Value = serde_json::from_str(&c.geojson).unwrap();
        assert_eq!(v["features"][0]["geometry"]["type"], "MultiPoint");
        assert_eq!(
            v["features"][0]["geometry"]["coordinates"][1],
            json!([3.0, 4.0])
        );
    }

    #[test]
    fn pointz_emits_a_third_coordinate_and_can_be_dropped() {
        let shp = shp_file(11, &[pointz_rec(1.0, 2.0, 30.5)]);
        let v: Value =
            serde_json::from_str(&parse(shp.clone(), &Options::default()).geojson).unwrap();
        assert_eq!(
            v["features"][0]["geometry"]["coordinates"],
            json!([1.0, 2.0, 30.5])
        );

        let flat = Options {
            include_z: false,
            ..Options::default()
        };
        let v: Value = serde_json::from_str(&parse(shp, &flat).geojson).unwrap();
        assert_eq!(
            v["features"][0]["geometry"]["coordinates"],
            json!([1.0, 2.0])
        );
    }

    #[test]
    fn precision_rounds_without_float_fuzz() {
        let opts = Options {
            precision: 3,
            ..Options::default()
        };
        let c = parse(shp_file(1, &[point_rec(-122.41941234, 37.77492789)]), &opts);
        assert!(
            c.geojson.contains("[-122.419,37.775]"),
            "got: {}",
            c.geojson
        );
    }

    #[test]
    fn full_precision_keeps_every_digit() {
        let opts = Options {
            precision: -1,
            ..Options::default()
        };
        let c = parse(shp_file(1, &[point_rec(-122.41941234, 37.77492789)]), &opts);
        assert!(c.geojson.contains("-122.41941234"), "got: {}", c.geojson);
    }

    #[test]
    fn limit_caps_features_but_reports_the_total() {
        let opts = Options {
            limit: 1,
            ..Options::default()
        };
        let c = parse(
            shp_file(
                1,
                &[
                    point_rec(1.0, 1.0),
                    point_rec(2.0, 2.0),
                    point_rec(3.0, 3.0),
                ],
            ),
            &opts,
        );
        assert_eq!(c.feature_count, 1);
        assert_eq!(c.total_records, 3);
        let v: Value = serde_json::from_str(&c.geojson).unwrap();
        assert_eq!(v["features"].as_array().unwrap().len(), 1);
        // The bbox covers what was emitted, not what was skipped.
        assert_eq!(v["bbox"], json!([1.0, 1.0, 1.0, 1.0]));
    }

    #[test]
    fn ndjson_emits_one_feature_per_line() {
        let opts = Options {
            output: Output::Ndjson,
            ..Options::default()
        };
        let c = parse(
            shp_file(1, &[point_rec(1.0, 2.0), point_rec(3.0, 4.0)]),
            &opts,
        );
        let lines: Vec<&str> = c.geojson.lines().collect();
        assert_eq!(lines.len(), 2);
        for l in lines {
            let v: Value = serde_json::from_str(l).unwrap();
            assert_eq!(v["type"], "Feature");
        }
        assert!(!c.geojson.contains("FeatureCollection"));
    }

    #[test]
    fn pretty_indents_the_output() {
        let opts = Options {
            pretty: true,
            ..Options::default()
        };
        let c = parse(shp_file(1, &[point_rec(1.0, 2.0)]), &opts);
        assert!(c.geojson.contains("\n  \"features\""), "got: {}", c.geojson);
    }

    #[test]
    fn bbox_can_be_omitted() {
        let opts = Options {
            bbox: false,
            ..Options::default()
        };
        let c = parse(shp_file(1, &[point_rec(1.0, 2.0)]), &opts);
        let v: Value = serde_json::from_str(&c.geojson).unwrap();
        assert!(v.get("bbox").is_none());
    }

    #[test]
    fn null_shapes_emit_a_null_geometry() {
        let c = parse(
            shp_file(1, &[0i32.to_le_bytes().to_vec()]),
            &Options::default(),
        );
        let v: Value = serde_json::from_str(&c.geojson).unwrap();
        assert!(v["features"][0]["geometry"].is_null());
    }

    #[test]
    fn multipatch_is_rejected_by_name() {
        let err = read_shapes(&shp_file(31, &[])).unwrap_err();
        assert!(err.contains("MultiPatch"), "got: {err}");
        assert!(err.contains("GeoJSON"), "got: {err}");
    }

    #[test]
    fn bad_file_code_is_rejected() {
        let mut shp = shp_file(1, &[point_rec(1.0, 2.0)]);
        shp[0..4].copy_from_slice(&1234i32.to_be_bytes());
        let err = read_shapes(&shp).unwrap_err();
        assert!(err.contains("file code"), "got: {err}");
    }

    #[test]
    fn truncated_tail_warns_instead_of_failing() {
        let mut shp = shp_file(1, &[point_rec(1.0, 2.0), point_rec(3.0, 4.0)]);
        shp.truncate(shp.len() - 6); // clip the last record mid-coordinate
        let (_t, shapes, warns) = read_shapes(&shp).unwrap();
        assert_eq!(shapes.len(), 1);
        assert!(
            warns.iter().any(|w| w.contains("truncated tail")),
            "got: {warns:?}"
        );
    }

    #[test]
    fn header_length_larger_than_the_file_is_tolerated() {
        let mut shp = shp_file(1, &[point_rec(1.0, 2.0)]);
        shp[24..28].copy_from_slice(&999_999i32.to_be_bytes());
        let (_t, shapes, warns) = read_shapes(&shp).unwrap();
        assert_eq!(shapes.len(), 1);
        assert!(
            warns.iter().any(|w| w.contains("declares")),
            "got: {warns:?}"
        );
    }

    #[test]
    fn part_index_out_of_range_is_an_actionable_error() {
        let mut rec = parts_rec(3, &[vec![(0.0, 0.0), (1.0, 1.0)]]);
        rec[44..48].copy_from_slice(&99i32.to_le_bytes()); // part start beyond numPoints
        let err = read_shapes(&shp_file(3, &[rec])).unwrap_err();
        assert!(err.contains("outside the 2 points"), "got: {err}");
    }

    // -- .prj -----------------------------------------------------------------

    #[test]
    fn geographic_prj_names_the_crs_without_warning() {
        let (name, warn) = read_crs(r#"GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984"]]"#);
        assert_eq!(name.as_deref(), Some("GCS_WGS_1984"));
        assert!(warn.is_none());
    }

    #[test]
    fn projected_prj_warns_that_coordinates_are_not_lon_lat() {
        let (name, warn) = read_crs(r#"PROJCS["NAD83 / UTM zone 10N",GEOGCS["NAD83"]]"#);
        assert_eq!(name.as_deref(), Some("NAD83 / UTM zone 10N"));
        let w = warn.unwrap();
        assert!(w.contains("EPSG:4326"), "got: {w}");
        assert!(w.contains("NAD83 / UTM zone 10N"), "got: {w}");
    }

    // -- .cpg -----------------------------------------------------------------

    #[test]
    fn cpg_sidecar_drives_auto_encoding() {
        assert_eq!(
            dbf_encoding(Encoding::Auto, Some("UTF-8")),
            DbfEncoding::Utf8
        );
        assert_eq!(
            dbf_encoding(Encoding::Auto, Some("65001")),
            DbfEncoding::Utf8
        );
        assert_eq!(
            dbf_encoding(Encoding::Auto, Some("ISO-8859-1")),
            DbfEncoding::Latin1
        );
        assert_eq!(
            dbf_encoding(Encoding::Auto, Some("WINDOWS-1252")),
            DbfEncoding::Cp1252
        );
        assert_eq!(dbf_encoding(Encoding::Auto, None), DbfEncoding::Auto);
        // An explicit choice always wins over the sidecar.
        assert_eq!(
            dbf_encoding(Encoding::Latin1, Some("UTF-8")),
            DbfEncoding::Latin1
        );
    }

    // -- zip / layer selection ------------------------------------------------

    #[test]
    fn zip_junk_entries_are_ignored() {
        assert!(is_junk("__MACOSX/foo.shp"));
        assert!(is_junk("dir/._foo.shp"));
        assert!(!is_junk("dir/foo.shp"));
    }

    #[test]
    fn path_helpers_split_stems_and_extensions() {
        assert_eq!(ext_of("a/b/tracts.SHP"), "shp");
        assert_eq!(ext_of("noext"), "");
        assert_eq!(stem_of("a/b/tracts.shp"), "a/b/tracts");
        assert_eq!(base_of("a/b/tracts"), "tracts");
    }

    #[test]
    fn zip_input_converts_and_attaches_attributes() {
        let zip = super::tests_zip::build();
        let c = convert(&zip, &Options::default()).unwrap();
        assert_eq!(c.layer, "places");
        assert_eq!(c.layers, vec!["places".to_string()]);
        assert_eq!(c.feature_count, 2);
        assert_eq!(c.crs.as_deref(), Some("GCS_WGS_1984"));
        let v: Value = serde_json::from_str(&c.geojson).unwrap();
        assert_eq!(v["features"][0]["properties"]["NAME"], "Alice");
        assert_eq!(v["features"][1]["properties"]["NAME"], "Bob");
        assert_eq!(v["features"][0]["properties"]["POP"], 30);
    }

    #[test]
    fn properties_off_leaves_empty_property_bags() {
        let opts = Options {
            properties: false,
            ..Options::default()
        };
        let c = convert(&super::tests_zip::build(), &opts).unwrap();
        let v: Value = serde_json::from_str(&c.geojson).unwrap();
        assert!(v["features"][0]["properties"]
            .as_object()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn columns_selects_a_subset_of_attributes() {
        let opts = Options {
            columns: "POP".to_string(),
            ..Options::default()
        };
        let c = convert(&super::tests_zip::build(), &opts).unwrap();
        let v: Value = serde_json::from_str(&c.geojson).unwrap();
        let props = v["features"][0]["properties"].as_object().unwrap();
        assert_eq!(props.len(), 1);
        assert_eq!(props["POP"], 30);
    }

    #[test]
    fn unknown_layer_lists_the_available_ones() {
        let err = convert(
            &super::tests_zip::build(),
            &Options {
                layer: "roads".to_string(),
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("no layer named"), "got: {err}");
        assert!(err.contains("places"), "got: {err}");
    }

    #[test]
    fn zip_without_a_shp_is_an_actionable_error() {
        let zip = super::tests_zip::build_named(&[("readme.txt", b"hi".to_vec())]);
        let err = convert(&zip, &Options::default()).unwrap_err();
        assert!(err.contains("no .shp member"), "got: {err}");
        assert!(err.contains("readme.txt"), "got: {err}");
    }
}

/// Test-only zip/DBF fixture builders. Kept out of `mod tests` so the zip crate's
/// writer half is only referenced under `cfg(test)`.
#[cfg(test)]
mod tests_zip {
    use super::tests_support::*;

    /// A one-layer archive: `places.shp` (2 points) + `places.dbf` (NAME, POP) +
    /// `places.prj` (WGS 84) — the shape a real download has.
    pub fn build() -> Vec<u8> {
        build_named(&[
            ("places.shp", two_point_shp()),
            ("places.dbf", sample_dbf()),
            (
                "places.prj",
                br#"GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984"]]"#.to_vec(),
            ),
        ])
    }

    pub fn build_named(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
        use std::io::Write;
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for (name, bytes) in entries {
                w.start_file(*name, opts).unwrap();
                w.write_all(bytes).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }
}

#[cfg(test)]
mod tests_support {
    use super::*;

    pub fn two_point_shp() -> Vec<u8> {
        let mut body = Vec::new();
        for (i, (x, y)) in [(-122.4194f64, 37.7749f64), (2.3522, 48.8566)]
            .iter()
            .enumerate()
        {
            let mut c = 1i32.to_le_bytes().to_vec();
            c.extend_from_slice(&x.to_le_bytes());
            c.extend_from_slice(&y.to_le_bytes());
            body.extend_from_slice(&((i as i32) + 1).to_be_bytes());
            body.extend_from_slice(&((c.len() / 2) as i32).to_be_bytes());
            body.extend_from_slice(&c);
        }
        let mut f = vec![0u8; SHP_HEADER_LEN];
        f[0..4].copy_from_slice(&SHP_FILE_CODE.to_be_bytes());
        f[24..28].copy_from_slice(&(((SHP_HEADER_LEN + body.len()) / 2) as i32).to_be_bytes());
        f[28..32].copy_from_slice(&1000i32.to_le_bytes());
        f[32..36].copy_from_slice(&1i32.to_le_bytes());
        f.extend_from_slice(&body);
        f
    }

    /// A dBase III table with NAME (C10) and POP (N5), two records.
    pub fn sample_dbf() -> Vec<u8> {
        let cols: [(&[u8], u8, u8); 2] = [(b"NAME", b'C', 10), (b"POP", b'N', 5)];
        let header_size = 32 + 32 * cols.len() + 1;
        let record_size = 1 + cols.iter().map(|c| c.2 as usize).sum::<usize>();
        let mut f = vec![0u8; 32];
        f[0] = 0x03;
        f[4..8].copy_from_slice(&2u32.to_le_bytes());
        f[8..10].copy_from_slice(&(header_size as u16).to_le_bytes());
        f[10..12].copy_from_slice(&(record_size as u16).to_le_bytes());
        for (name, ty, len) in cols {
            let mut d = vec![0u8; 32];
            d[..name.len()].copy_from_slice(name);
            d[11] = ty;
            d[16] = len;
            f.extend_from_slice(&d);
        }
        f.push(0x0D);
        for (name, pop) in [("Alice", "30"), ("Bob", "45")] {
            f.push(b' ');
            let mut n = vec![b' '; 10];
            n[..name.len()].copy_from_slice(name.as_bytes());
            f.extend_from_slice(&n);
            let mut p = vec![b' '; 5];
            p[5 - pop.len()..].copy_from_slice(pop.as_bytes());
            f.extend_from_slice(&p);
        }
        f.push(0x1A);
        f
    }
}
