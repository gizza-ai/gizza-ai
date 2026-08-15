//! stl-inspector core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps, no external crates.
//!
//! Reads a triangle mesh in **STL** — either ASCII STL pasted as text, or a
//! *binary* STL pasted as base64 or hex bytes — and reports it without changing
//! it: triangle and welded-vertex counts, bounding box / bounds / centre, surface
//! area, signed and absolute volume, watertight and manifold status, boundary and
//! non-manifold edge counts, disconnected shells, degenerate and duplicate
//! triangles, winding consistency, and how the facet normals stored in the file
//! compare with the normals implied by the geometry.
//!
//! Read-only by design: repairs live in `blocks/stl-repair`.
//!
//! Auto-detection deliberately trusts the binary **length** rule
//! (`84 + 50 * triangle_count == byte length`) over the leading `solid` keyword,
//! because plenty of exporters write "solid" into a binary STL's 80-byte header.

use std::collections::HashMap;

/// Hard cap on triangles, matching `blocks/stl-repair`. A binary STL at the cap is
/// ~5 MB of bytes (~6.7 MB of base64), which is already an awkward paste.
pub const MAX_TRIANGLES: usize = 100_000;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// How the pasted `stl` value is encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    /// Sniff ASCII text vs base64 vs hex (the default).
    Auto,
    /// ASCII STL text (`solid` / `facet normal` / `vertex` lines).
    Ascii,
    /// Binary STL bytes, base64-encoded.
    Base64,
    /// Binary STL bytes, hex-encoded.
    Hex,
}

impl InputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(InputFormat::Auto),
            "ascii" | "text" => Ok(InputFormat::Ascii),
            "base64" | "b64" => Ok(InputFormat::Base64),
            "hex" => Ok(InputFormat::Hex),
            other => Err(format!(
                "unknown input_format '{other}': expected 'auto', 'ascii', 'base64' or 'hex'"
            )),
        }
    }
}

/// What to render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// A readable, sectioned inspection report (the default).
    Report,
    /// The same numbers as a machine-readable JSON object.
    Json,
}

impl Output {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "report" => Ok(Output::Report),
            "json" => Ok(Output::Json),
            other => Err(format!(
                "unknown output '{other}': expected 'report' or 'json'"
            )),
        }
    }
}

/// The unit the mesh's raw coordinates are taken to be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Units {
    Mm,
    Cm,
    In,
}

impl Units {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "mm" => Ok(Units::Mm),
            "cm" => Ok(Units::Cm),
            "in" | "inch" | "inches" => Ok(Units::In),
            other => Err(format!(
                "unknown units '{other}': expected 'mm', 'cm' or 'in'"
            )),
        }
    }

    /// Suffix used for lengths (areas add `²`, volumes `³`).
    pub fn label(self) -> &'static str {
        match self {
            Units::Mm => "mm",
            Units::Cm => "cm",
            Units::In => "in",
        }
    }

    /// How many cubic centimetres one cubic unit is worth — the conversion the
    /// density-based weight estimate needs.
    fn cubic_cm_per_unit(self) -> f64 {
        match self {
            Units::Mm => 0.001,
            Units::Cm => 1.0,
            Units::In => 16.387_064,
        }
    }
}

/// Everything the caller can tune.
#[derive(Debug, Clone)]
pub struct Options {
    pub input_format: InputFormat,
    pub output: Output,
    pub units: Units,
    /// Multiplier applied to every coordinate before measuring.
    pub scale: f64,
    /// Material density in g/cm³; `0` (the default) skips the weight estimate.
    pub density: f64,
    /// Distance below which two corners count as the same vertex.
    pub weld_tolerance: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input_format: InputFormat::Auto,
            output: Output::Report,
            units: Units::Mm,
            scale: 1.0,
            density: 0.0,
            weld_tolerance: 0.000_001,
        }
    }
}

// ---------------------------------------------------------------------------
// Parsed mesh + report
// ---------------------------------------------------------------------------

type Vec3 = [f64; 3];

/// One triangle: three corners plus the facet normal as stored in the file.
#[derive(Debug, Clone)]
struct Tri {
    v: [Vec3; 3],
    stored_normal: Vec3,
    attr: u16,
}

/// Everything `inspect` measures. Public so callers can render it themselves.
#[derive(Debug, Clone)]
pub struct Report {
    pub encoding: String,
    pub solid_name: String,
    pub units: Units,
    pub scale: f64,
    pub triangles: usize,
    pub distinct_vertices: usize,
    pub bbox_min: Vec3,
    pub bbox_max: Vec3,
    pub surface_area: f64,
    pub signed_volume: f64,
    pub volume: f64,
    pub watertight: bool,
    pub manifold: bool,
    pub boundary_edges: usize,
    pub non_manifold_edges: usize,
    pub inconsistent_edges: usize,
    pub shells: usize,
    pub degenerate_triangles: usize,
    pub duplicate_triangles: usize,
    pub normals_mismatched: usize,
    pub normals_unset: usize,
    pub attribute_bytes_set: usize,
    pub outward_normals: bool,
    pub print_ready: bool,
    /// Defects that make the mesh unsafe to slice, in the order they are found.
    pub issues: Vec<String>,
    /// Things worth knowing that do not block printing (e.g. several shells).
    pub notes: Vec<String>,
    /// Estimated mass in grams, when a density was supplied.
    pub weight_grams: Option<f64>,
    pub density: f64,
}

impl Report {
    /// Bounding-box extent along each axis.
    pub fn size(&self) -> Vec3 {
        [
            self.bbox_max[0] - self.bbox_min[0],
            self.bbox_max[1] - self.bbox_min[1],
            self.bbox_max[2] - self.bbox_min[2],
        ]
    }

    /// Centre of the bounding box.
    pub fn center(&self) -> Vec3 {
        [
            (self.bbox_max[0] + self.bbox_min[0]) / 2.0,
            (self.bbox_max[1] + self.bbox_min[1]) / 2.0,
            (self.bbox_max[2] + self.bbox_min[2]) / 2.0,
        ]
    }
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Inspect `input` and render it per `opt.output`.
pub fn inspect(input: &str, opt: &Options) -> Result<String, String> {
    let r = analyze(input, opt)?;
    Ok(match opt.output {
        Output::Report => render_report(&r),
        Output::Json => render_json(&r),
    })
}

/// Parse + measure, returning the raw numbers.
pub fn analyze(input: &str, opt: &Options) -> Result<Report, String> {
    if !opt.scale.is_finite() || opt.scale <= 0.0 {
        return Err(format!(
            "scale must be a positive number, got {}",
            fmt_num(opt.scale)
        ));
    }
    if !opt.density.is_finite() || opt.density < 0.0 {
        return Err(format!(
            "density must be zero or a positive number of g/cm³, got {}",
            fmt_num(opt.density)
        ));
    }
    if !opt.weld_tolerance.is_finite() || opt.weld_tolerance < 0.0 {
        return Err(format!(
            "weld_tolerance must be zero or a positive distance, got {}",
            fmt_num(opt.weld_tolerance)
        ));
    }

    let (mut tris, encoding, solid_name) = decode(input, opt.input_format)?;
    if tris.is_empty() {
        return Err(
            "no triangles found — the mesh parsed but contains zero facets".to_string(),
        );
    }
    if opt.scale != 1.0 {
        for t in &mut tris {
            for v in &mut t.v {
                v[0] *= opt.scale;
                v[1] *= opt.scale;
                v[2] *= opt.scale;
            }
        }
    }

    Ok(measure(&tris, encoding, solid_name, opt))
}

// ---------------------------------------------------------------------------
// Decoding — pick ASCII vs binary, and base64 vs hex for binary
// ---------------------------------------------------------------------------

fn decode(input: &str, fmt: InputFormat) -> Result<(Vec<Tri>, String, String), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("no input: paste an ASCII STL, or a binary STL as base64 or hex".to_string());
    }

    match fmt {
        InputFormat::Ascii => {
            let (tris, name) = parse_ascii(trimmed)?;
            Ok((tris, "ASCII STL (text)".to_string(), name))
        }
        InputFormat::Base64 => {
            let bytes = decode_base64(trimmed)?;
            from_bytes(&bytes, "base64")
        }
        InputFormat::Hex => {
            let bytes = decode_hex(trimmed)?;
            from_bytes(&bytes, "hex")
        }
        InputFormat::Auto => {
            if looks_like_ascii_stl(trimmed) {
                let (tris, name) = parse_ascii(trimmed)?;
                return Ok((tris, "ASCII STL (text, auto-detected)".to_string(), name));
            }
            // Not obviously text, so it should be encoded bytes. Try hex first —
            // its alphabet is a strict subset of base64's, so a hex-looking blob
            // would otherwise be mis-read as base64.
            let hex_err = match decode_hex(trimmed) {
                Ok(bytes) => match from_bytes(&bytes, "hex, auto-detected") {
                    Ok(v) => return Ok(v),
                    Err(e) => e,
                },
                Err(e) => e,
            };
            match decode_base64(trimmed) {
                Ok(bytes) => from_bytes(&bytes, "base64, auto-detected"),
                Err(b64_err) => Err(format!(
                    "could not read the input as an ASCII STL, hex bytes or base64 bytes. \
                     As hex: {hex_err}. As base64: {b64_err}. Set input_format explicitly if \
                     auto-detection is guessing wrong."
                )),
            }
        }
    }
}

/// A blob is treated as ASCII STL when it carries the keywords a text STL must
/// have. `solid` alone is NOT enough — binary headers routinely start with it.
fn looks_like_ascii_stl(s: &str) -> bool {
    let head: String = s.chars().take(4096).collect::<String>().to_ascii_lowercase();
    head.contains("facet normal") || (head.contains("outer loop") && head.contains("vertex "))
}

/// Turn decoded bytes into triangles. Binary STL is the expectation, but an
/// ASCII STL that was base64'd or hex-dumped still decodes cleanly, so accept it.
fn from_bytes(bytes: &[u8], how: &str) -> Result<(Vec<Tri>, String, String), String> {
    match parse_binary(bytes) {
        Ok((tris, header)) => Ok((tris, format!("binary STL ({how})"), header)),
        Err(bin_err) => {
            if let Ok(text) = std::str::from_utf8(bytes) {
                if looks_like_ascii_stl(text) {
                    let (tris, name) = parse_ascii(text.trim())?;
                    return Ok((tris, format!("ASCII STL ({how}-encoded)"), name));
                }
            }
            Err(bin_err)
        }
    }
}

// ---------------------------------------------------------------------------
// ASCII STL
// ---------------------------------------------------------------------------

fn parse_ascii(text: &str) -> Result<(Vec<Tri>, String), String> {
    let mut tris: Vec<Tri> = Vec::new();
    let mut name = String::new();
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
            "solid" => {
                if name.is_empty() {
                    name = line[5..].trim().to_string();
                }
            }
            "facet" => {
                in_facet = true;
                corners.clear();
                normal = [0.0; 3];
                // `facet normal ni nj nk` — a missing/short normal is tolerated
                // and reported later as an unset normal.
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
                    stored_normal: normal,
                    attr: 0,
                });
                corners.clear();
                in_facet = false;
            }
            // `outer loop`, `endloop`, `endsolid`, and anything else, are noise.
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
    if name.is_empty() {
        name = "(unnamed)".to_string();
    }
    Ok((tris, name))
}

fn parse_f64(s: &str, lineno: usize, what: &str) -> Result<f64, String> {
    s.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("line {lineno}: '{s}' is not a finite number in a {what}"))
}

// ---------------------------------------------------------------------------
// Binary STL
// ---------------------------------------------------------------------------

fn parse_binary(bytes: &[u8]) -> Result<(Vec<Tri>, String), String> {
    if bytes.len() < 84 {
        return Err(format!(
            "binary STL too short: {} bytes, but the 80-byte header plus the 4-byte \
             triangle count already need 84",
            bytes.len()
        ));
    }
    let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    if count > MAX_TRIANGLES {
        return Err(format!(
            "too many triangles: the header declares {count}, this tool handles up to {MAX_TRIANGLES}"
        ));
    }
    let needed = 84usize + count * 50;
    if bytes.len() < needed {
        return Err(format!(
            "truncated binary STL: the header declares {count} triangles ({needed} bytes) \
             but only {} bytes were given",
            bytes.len()
        ));
    }

    let mut tris = Vec::with_capacity(count);
    for i in 0..count {
        let base = 84 + i * 50;
        let f = |k: usize| -> f64 {
            let o = base + k * 4;
            f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]) as f64
        };
        let mut vals = [0.0f64; 12];
        for (k, val) in vals.iter_mut().enumerate() {
            *val = f(k);
        }
        if vals.iter().any(|v| !v.is_finite()) {
            return Err(format!(
                "triangle {} contains a non-finite coordinate (NaN or infinity)",
                i + 1
            ));
        }
        let attr = u16::from_le_bytes([bytes[base + 48], bytes[base + 49]]);
        tris.push(Tri {
            v: [
                [vals[3], vals[4], vals[5]],
                [vals[6], vals[7], vals[8]],
                [vals[9], vals[10], vals[11]],
            ],
            stored_normal: [vals[0], vals[1], vals[2]],
            attr,
        });
    }

    Ok((tris, header_text(&bytes[..80])))
}

/// Render the 80-byte header as a readable label, or a placeholder when it is
/// empty/binary junk.
fn header_text(header: &[u8]) -> String {
    let text: String = header
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as char)
        .collect();
    let trimmed = text.trim();
    if trimmed.is_empty() || !trimmed.chars().all(|c| c == ' ' || !c.is_control()) {
        "(binary header, no readable name)".to_string()
    } else {
        trimmed.to_string()
    }
}

// ---------------------------------------------------------------------------
// base64 / hex decoding (no external crates)
// ---------------------------------------------------------------------------

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

/// Hex bytes; whitespace, `:`, `-` and a leading `0x` are ignored.
fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let mut nibbles: Vec<u8> = Vec::with_capacity(s.len());
    let body = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
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

// ---------------------------------------------------------------------------
// Measurement
// ---------------------------------------------------------------------------

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: Vec3) -> f64 {
    dot(a, a).sqrt()
}

/// Weld key: a lattice cell at `tol`, or the exact bit pattern when `tol == 0`.
fn weld_key(v: Vec3, tol: f64) -> (i64, i64, i64) {
    if tol <= 0.0 {
        let q = |x: f64| (if x == 0.0 { 0.0 } else { x }).to_bits() as i64;
        (q(v[0]), q(v[1]), q(v[2]))
    } else {
        let q = |x: f64| (x / tol).round() as i64;
        (q(v[0]), q(v[1]), q(v[2]))
    }
}

struct DisjointSet(Vec<usize>);

impl DisjointSet {
    fn new(n: usize) -> Self {
        DisjointSet((0..n).collect())
    }
    fn find(&mut self, mut x: usize) -> usize {
        while self.0[x] != x {
            self.0[x] = self.0[self.0[x]];
            x = self.0[x];
        }
        x
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.0[ra] = rb;
        }
    }
}

fn measure(tris: &[Tri], encoding: String, solid_name: String, opt: &Options) -> Report {
    // --- weld corners into distinct vertices -------------------------------
    let mut ids: HashMap<(i64, i64, i64), usize> = HashMap::new();
    let mut faces: Vec<[usize; 3]> = Vec::with_capacity(tris.len());
    for t in tris {
        let mut f = [0usize; 3];
        for (k, v) in t.v.iter().enumerate() {
            let key = weld_key(*v, opt.weld_tolerance);
            let next = ids.len();
            f[k] = *ids.entry(key).or_insert(next);
        }
        faces.push(f);
    }

    // --- per-triangle quantities ------------------------------------------
    let mut bbox_min = [f64::INFINITY; 3];
    let mut bbox_max = [f64::NEG_INFINITY; 3];
    let mut surface_area = 0.0f64;
    let mut signed_volume = 0.0f64;
    let mut degenerate_triangles = 0usize;
    let mut normals_mismatched = 0usize;
    let mut normals_unset = 0usize;
    let mut attribute_bytes_set = 0usize;
    // Facet normals within 1° of the geometric normal count as agreeing.
    let cos_tol = (std::f64::consts::PI / 180.0).cos();

    for (i, t) in tris.iter().enumerate() {
        for v in &t.v {
            for k in 0..3 {
                bbox_min[k] = bbox_min[k].min(v[k]);
                bbox_max[k] = bbox_max[k].max(v[k]);
            }
        }
        let c = cross(sub(t.v[1], t.v[0]), sub(t.v[2], t.v[0]));
        let twice_area = norm(c);
        surface_area += twice_area / 2.0;
        signed_volume += dot(t.v[0], cross(t.v[1], t.v[2])) / 6.0;

        let f = faces[i];
        let repeated = f[0] == f[1] || f[1] == f[2] || f[0] == f[2];
        if repeated || twice_area == 0.0 {
            degenerate_triangles += 1;
        }

        let stored_len = norm(t.stored_normal);
        if stored_len == 0.0 {
            normals_unset += 1;
        } else if twice_area > 0.0 {
            let agreement = dot(t.stored_normal, c) / (stored_len * twice_area);
            if agreement < cos_tol {
                normals_mismatched += 1;
            }
        }
        if t.attr != 0 {
            attribute_bytes_set += 1;
        }
    }

    // --- duplicate triangles (same 3 welded corners, any rotation/winding) --
    let mut seen: HashMap<[usize; 3], usize> = HashMap::new();
    let mut duplicate_triangles = 0usize;
    for f in &faces {
        let mut key = *f;
        key.sort_unstable();
        let e = seen.entry(key).or_insert(0);
        *e += 1;
        if *e > 1 {
            duplicate_triangles += 1;
        }
    }

    // --- edge topology (degenerate faces carry no usable edges) ------------
    let mut edges: HashMap<(usize, usize), (usize, usize)> = HashMap::new(); // (forward, backward)
    for f in &faces {
        if f[0] == f[1] || f[1] == f[2] || f[0] == f[2] {
            continue;
        }
        for k in 0..3 {
            let (a, b) = (f[k], f[(k + 1) % 3]);
            let e = edges.entry((a.min(b), a.max(b))).or_insert((0, 0));
            if a < b {
                e.0 += 1;
            } else {
                e.1 += 1;
            }
        }
    }
    let mut boundary_edges = 0usize;
    let mut non_manifold_edges = 0usize;
    let mut inconsistent_edges = 0usize;
    for (fwd, bwd) in edges.values() {
        let total = fwd + bwd;
        if total == 1 {
            boundary_edges += 1;
        } else if total > 2 {
            non_manifold_edges += 1;
        } else if *fwd != 1 || *bwd != 1 {
            // Two uses in the SAME direction: the two faces disagree on winding.
            inconsistent_edges += 1;
        }
    }

    // --- connected shells over shared welded vertices ----------------------
    let mut ds = DisjointSet::new(ids.len().max(1));
    for f in &faces {
        ds.union(f[0], f[1]);
        ds.union(f[1], f[2]);
    }
    let mut roots: Vec<usize> = faces.iter().map(|f| ds.find(f[0])).collect();
    roots.sort_unstable();
    roots.dedup();
    let shells = roots.len();

    let watertight = boundary_edges == 0 && non_manifold_edges == 0 && !faces.is_empty();
    let manifold = non_manifold_edges == 0;
    let volume = signed_volume.abs();
    let outward_normals = signed_volume >= 0.0;

    let mut issues = Vec::new();
    if boundary_edges > 0 {
        issues.push(format!("{boundary_edges} open (boundary) edges"));
    }
    if non_manifold_edges > 0 {
        issues.push(format!("{non_manifold_edges} non-manifold edges"));
    }
    if inconsistent_edges > 0 {
        issues.push(format!("{inconsistent_edges} edges with inconsistent winding"));
    }
    if degenerate_triangles > 0 {
        issues.push(format!("{degenerate_triangles} degenerate triangles"));
    }
    if duplicate_triangles > 0 {
        issues.push(format!("{duplicate_triangles} duplicate triangles"));
    }
    if watertight && !outward_normals {
        issues.push("normals point inward (the mesh is inside-out)".to_string());
    }
    // Several closed shells slice fine as separate parts on one plate — worth
    // surfacing, but not a reason to withhold the print-ready verdict.
    let mut notes = Vec::new();
    if shells > 1 {
        notes.push(format!(
            "{shells} disconnected shells — they will print as separate parts"
        ));
    }
    if normals_mismatched > 0 {
        notes.push(format!(
            "{normals_mismatched} stored facet normals disagree with the geometry by more than 1° \
             (slicers recompute normals, so this rarely matters)"
        ));
    }
    if normals_unset > 0 {
        notes.push(format!(
            "{normals_unset} facets store a (0,0,0) normal — allowed, and recomputed from the winding"
        ));
    }
    let print_ready = watertight
        && manifold
        && inconsistent_edges == 0
        && degenerate_triangles == 0
        && duplicate_triangles == 0
        && outward_normals;

    let weight_grams = if opt.density > 0.0 {
        Some(volume * opt.units.cubic_cm_per_unit() * opt.density)
    } else {
        None
    };

    Report {
        encoding,
        solid_name,
        units: opt.units,
        scale: opt.scale,
        triangles: tris.len(),
        distinct_vertices: ids.len(),
        bbox_min,
        bbox_max,
        surface_area,
        signed_volume,
        volume,
        watertight,
        manifold,
        boundary_edges,
        non_manifold_edges,
        inconsistent_edges,
        shells,
        degenerate_triangles,
        duplicate_triangles,
        normals_mismatched,
        normals_unset,
        attribute_bytes_set,
        outward_normals,
        print_ready,
        issues,
        notes,
        weight_grams,
        density: opt.density,
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "n/a".to_string();
    }
    if v != 0.0 && v.abs() < 1e-4 {
        return format!("{v:.4e}");
    }
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn row(label: &str, value: String) -> String {
    format!("  {label:<22}{value}\n")
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn triple(v: Vec3) -> String {
    format!("{} {} {}", fmt_num(v[0]), fmt_num(v[1]), fmt_num(v[2]))
}

fn render_report(r: &Report) -> String {
    let u = r.units.label();
    let size = r.size();
    let mut s = String::new();
    s.push_str("STL inspection report\n=====================\n\nInput\n");
    s.push_str(&row("Encoding", r.encoding.clone()));
    s.push_str(&row("Solid name / header", r.solid_name.clone()));
    s.push_str(&row("Units", u.to_string()));
    s.push_str(&row("Scale factor", fmt_num(r.scale)));

    s.push_str("\nGeometry\n");
    s.push_str(&row("Triangles", r.triangles.to_string()));
    s.push_str(&row("Distinct vertices", r.distinct_vertices.to_string()));
    s.push_str(&row(
        "Bounding box",
        format!(
            "{} x {} x {} {u}",
            fmt_num(size[0]),
            fmt_num(size[1]),
            fmt_num(size[2])
        ),
    ));
    s.push_str(&row(
        "Bounds",
        format!(
            "min {} / max {} {u}",
            triple(r.bbox_min),
            triple(r.bbox_max)
        ),
    ));
    s.push_str(&row("Center", format!("{} {u}", triple(r.center()))));
    s.push_str(&row(
        "Surface area",
        format!("{} {u}²", fmt_num(r.surface_area)),
    ));
    let mut vol = format!("{} {u}³", fmt_num(r.volume));
    if r.units != Units::Cm {
        vol.push_str(&format!(
            " ({} cm³)",
            fmt_num(r.volume * r.units.cubic_cm_per_unit())
        ));
    }
    if !r.watertight {
        vol.push_str(" — approximate, the mesh is not closed");
    }
    s.push_str(&row("Volume", vol));
    s.push_str(&row(
        "Signed volume",
        format!(
            "{} {u}³ ({})",
            fmt_num(r.signed_volume),
            if !r.watertight {
                "open mesh — the sign is not conclusive"
            } else if r.outward_normals {
                "positive — normals face outward"
            } else {
                "negative — normals face inward"
            }
        ),
    ));
    if let Some(g) = r.weight_grams {
        s.push_str(&row(
            "Estimated weight",
            format!("{} g at {} g/cm³", fmt_num(g), fmt_num(r.density)),
        ));
    }

    s.push_str("\nMesh integrity\n");
    s.push_str(&row("Watertight", yes_no(r.watertight).to_string()));
    s.push_str(&row("Manifold", yes_no(r.manifold).to_string()));
    s.push_str(&row("Open (boundary) edges", r.boundary_edges.to_string()));
    s.push_str(&row("Non-manifold edges", r.non_manifold_edges.to_string()));
    s.push_str(&row("Inconsistent winding", r.inconsistent_edges.to_string()));
    s.push_str(&row("Disconnected shells", r.shells.to_string()));
    s.push_str(&row(
        "Degenerate triangles",
        r.degenerate_triangles.to_string(),
    ));
    s.push_str(&row(
        "Duplicate triangles",
        r.duplicate_triangles.to_string(),
    ));
    s.push_str(&row("Normals mismatched", r.normals_mismatched.to_string()));
    s.push_str(&row("Normals unset (0,0,0)", r.normals_unset.to_string()));
    s.push_str(&row(
        "Attribute bytes set",
        r.attribute_bytes_set.to_string(),
    ));

    s.push_str("\nVerdict\n");
    s.push_str(&row(
        "Print-ready",
        if r.print_ready {
            "yes — closed, manifold, consistently wound, normals outward".to_string()
        } else if r.issues.is_empty() {
            "no".to_string()
        } else {
            format!("no — {}", r.issues.join("; "))
        },
    ));
    for n in &r.notes {
        s.push_str(&row("Note", n.clone()));
    }
    s
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn json_num(v: f64) -> String {
    if v.is_finite() {
        fmt_num(v)
    } else {
        "null".to_string()
    }
}

fn render_json(r: &Report) -> String {
    let size = r.size();
    let center = r.center();
    let arr = |v: Vec3| format!("[{}, {}, {}]", json_num(v[0]), json_num(v[1]), json_num(v[2]));
    let mut s = String::new();
    s.push_str("{\n");
    s.push_str(&format!(
        "  \"encoding\": \"{}\",\n  \"solid_name\": \"{}\",\n",
        json_escape(&r.encoding),
        json_escape(&r.solid_name)
    ));
    s.push_str(&format!(
        "  \"units\": \"{}\",\n  \"scale\": {},\n",
        r.units.label(),
        json_num(r.scale)
    ));
    s.push_str(&format!(
        "  \"triangles\": {},\n  \"distinct_vertices\": {},\n",
        r.triangles, r.distinct_vertices
    ));
    s.push_str(&format!(
        "  \"bbox_min\": {},\n  \"bbox_max\": {},\n  \"size\": {},\n  \"center\": {},\n",
        arr(r.bbox_min),
        arr(r.bbox_max),
        arr(size),
        arr(center)
    ));
    s.push_str(&format!(
        "  \"surface_area\": {},\n  \"volume\": {},\n  \"signed_volume\": {},\n  \"volume_cm3\": {},\n",
        json_num(r.surface_area),
        json_num(r.volume),
        json_num(r.signed_volume),
        json_num(r.volume * r.units.cubic_cm_per_unit())
    ));
    s.push_str(&format!(
        "  \"weight_grams\": {},\n  \"density\": {},\n",
        match r.weight_grams {
            Some(g) => json_num(g),
            None => "null".to_string(),
        },
        json_num(r.density)
    ));
    s.push_str(&format!(
        "  \"watertight\": {},\n  \"manifold\": {},\n  \"outward_normals\": {},\n",
        r.watertight, r.manifold, r.outward_normals
    ));
    s.push_str(&format!(
        "  \"boundary_edges\": {},\n  \"non_manifold_edges\": {},\n  \"inconsistent_winding_edges\": {},\n  \"shells\": {},\n",
        r.boundary_edges, r.non_manifold_edges, r.inconsistent_edges, r.shells
    ));
    s.push_str(&format!(
        "  \"degenerate_triangles\": {},\n  \"duplicate_triangles\": {},\n  \"normals_mismatched\": {},\n  \"normals_unset\": {},\n  \"attribute_bytes_set\": {},\n",
        r.degenerate_triangles,
        r.duplicate_triangles,
        r.normals_mismatched,
        r.normals_unset,
        r.attribute_bytes_set
    ));
    s.push_str(&format!("  \"print_ready\": {},\n", r.print_ready));
    let strings = |v: &[String]| -> String {
        v.iter()
            .map(|i| format!("\"{}\"", json_escape(i)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    s.push_str(&format!("  \"issues\": [{}],\n", strings(&r.issues)));
    s.push_str(&format!("  \"notes\": [{}]\n}}\n", strings(&r.notes)));
    s
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The 12 triangles of the unit cube [0,1]³, wound counter-clockwise seen
    /// from outside, as (v0, v1, v2) corner triples.
    fn unit_cube_tris() -> Vec<[Vec3; 3]> {
        let p = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        let quads: [[usize; 4]; 6] = [
            [0, 3, 2, 1], // z = 0, normal -Z
            [4, 5, 6, 7], // z = 1, normal +Z
            [0, 1, 5, 4], // y = 0, normal -Y
            [2, 3, 7, 6], // y = 1, normal +Y
            [0, 4, 7, 3], // x = 0, normal -X
            [1, 2, 6, 5], // x = 1, normal +X
        ];
        let mut out = Vec::new();
        for q in quads {
            out.push([p[q[0]], p[q[1]], p[q[2]]]);
            out.push([p[q[0]], p[q[2]], p[q[3]]]);
        }
        out
    }

    fn geometric_normal(t: &[Vec3; 3]) -> Vec3 {
        let c = cross(sub(t[1], t[0]), sub(t[2], t[0]));
        let l = norm(c);
        if l == 0.0 {
            [0.0; 3]
        } else {
            [c[0] / l, c[1] / l, c[2] / l]
        }
    }

    /// Write triangles as ASCII STL text.
    fn ascii_stl(name: &str, tris: &[[Vec3; 3]]) -> String {
        let mut s = format!("solid {name}\n");
        for t in tris {
            let n = geometric_normal(t);
            s.push_str(&format!(
                "  facet normal {} {} {}\n    outer loop\n",
                n[0], n[1], n[2]
            ));
            for v in t {
                s.push_str(&format!("      vertex {} {} {}\n", v[0], v[1], v[2]));
            }
            s.push_str("    endloop\n  endfacet\n");
        }
        s.push_str(&format!("endsolid {name}\n"));
        s
    }

    /// Build real BINARY STL bytes in test code — no committed binary fixture.
    fn binary_stl(header: &str, tris: &[[Vec3; 3]]) -> Vec<u8> {
        let mut buf = vec![0u8; 80];
        for (i, b) in header.bytes().take(80).enumerate() {
            buf[i] = b;
        }
        buf.extend_from_slice(&(tris.len() as u32).to_le_bytes());
        for t in tris {
            let n = geometric_normal(t);
            for c in n {
                buf.extend_from_slice(&(c as f32).to_le_bytes());
            }
            for v in t {
                for c in v {
                    buf.extend_from_slice(&(*c as f32).to_le_bytes());
                }
            }
            buf.extend_from_slice(&0u16.to_le_bytes());
        }
        buf
    }

    fn b64(bytes: &[u8]) -> String {
        const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for c in bytes.chunks(3) {
            let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            let idx = [n >> 18 & 63, n >> 12 & 63, n >> 6 & 63, n & 63];
            for (i, k) in idx.iter().enumerate() {
                if i <= c.len() {
                    out.push(A[*k as usize] as char);
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    // --- happy paths -------------------------------------------------------

    #[test]
    fn ascii_cube_is_watertight_with_unit_volume() {
        let text = ascii_stl("cube", &unit_cube_tris());
        let r = analyze(&text, &Options::default()).unwrap();
        assert_eq!(r.triangles, 12);
        assert_eq!(r.distinct_vertices, 8);
        assert!(r.watertight);
        assert!(r.manifold);
        assert_eq!(r.boundary_edges, 0);
        assert_eq!(r.non_manifold_edges, 0);
        assert_eq!(r.inconsistent_edges, 0);
        assert_eq!(r.shells, 1);
        assert!((r.surface_area - 6.0).abs() < 1e-9);
        assert!((r.volume - 1.0).abs() < 1e-9);
        assert!(r.outward_normals);
        assert!(r.print_ready);
        assert_eq!(r.normals_mismatched, 0);
    }

    #[test]
    fn binary_cube_matches_the_ascii_cube() {
        let bytes = binary_stl("cube from a slicer", &unit_cube_tris());
        assert_eq!(bytes.len(), 84 + 12 * 50);
        let r = analyze(&b64(&bytes), &Options::default()).unwrap();
        assert_eq!(r.triangles, 12);
        assert_eq!(r.solid_name, "cube from a slicer");
        assert!(r.encoding.starts_with("binary STL"));
        assert!(r.watertight);
        assert!((r.volume - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hex_binary_input_is_auto_detected() {
        let bytes = binary_stl("hexcube", &unit_cube_tris());
        let r = analyze(&hex(&bytes), &Options::default()).unwrap();
        assert_eq!(r.triangles, 12);
        assert!(r.encoding.contains("hex"));
        assert!(r.watertight);
    }

    #[test]
    fn binary_header_starting_with_solid_is_not_mistaken_for_ascii() {
        // The classic trap: a binary STL whose header text begins "solid".
        let bytes = binary_stl("solid exported by some CAD tool", &unit_cube_tris());
        let r = analyze(&b64(&bytes), &Options::default()).unwrap();
        assert_eq!(r.triangles, 12);
        assert!(r.encoding.starts_with("binary STL"));
    }

    #[test]
    fn base64_wrapped_ascii_stl_still_parses() {
        let text = ascii_stl("cube", &unit_cube_tris());
        let r = analyze(&b64(text.as_bytes()), &Options::default()).unwrap();
        assert_eq!(r.triangles, 12);
        assert!(r.encoding.contains("ASCII STL"));
    }

    #[test]
    fn explicit_input_format_is_honored() {
        let text = ascii_stl("cube", &unit_cube_tris());
        let opt = Options {
            input_format: InputFormat::Ascii,
            ..Options::default()
        };
        assert_eq!(analyze(&text, &opt).unwrap().triangles, 12);

        let bytes = binary_stl("cube", &unit_cube_tris());
        let opt = Options {
            input_format: InputFormat::Base64,
            ..Options::default()
        };
        assert_eq!(analyze(&b64(&bytes), &opt).unwrap().triangles, 12);
    }

    #[test]
    fn scale_and_units_change_the_measurements() {
        let text = ascii_stl("cube", &unit_cube_tris());
        let opt = Options {
            scale: 10.0,
            ..Options::default()
        };
        let r = analyze(&text, &opt).unwrap();
        assert!((r.volume - 1000.0).abs() < 1e-6); // 10³
        assert!((r.surface_area - 600.0).abs() < 1e-6); // 6 × 10²
        assert_eq!(r.size(), [10.0, 10.0, 10.0]);
    }

    #[test]
    fn density_produces_a_weight_estimate() {
        // A 10 mm cube is 1000 mm³ = 1 cm³; at 1.24 g/cm³ (PLA) that is 1.24 g.
        let text = ascii_stl("cube", &unit_cube_tris());
        let opt = Options {
            scale: 10.0,
            density: 1.24,
            ..Options::default()
        };
        let r = analyze(&text, &opt).unwrap();
        assert!((r.weight_grams.unwrap() - 1.24).abs() < 1e-6);

        // Same cube read as inches weighs 16.387064 cm³ × 1.24 g/cm³ per cubic inch.
        let opt = Options {
            units: Units::In,
            density: 1.24,
            ..Options::default()
        };
        let r = analyze(&text, &opt).unwrap();
        assert!((r.weight_grams.unwrap() - 16.387_064 * 1.24).abs() < 1e-6);
    }

    #[test]
    fn json_output_carries_the_headline_numbers() {
        let text = ascii_stl("cube", &unit_cube_tris());
        let opt = Options {
            output: Output::Json,
            ..Options::default()
        };
        let out = inspect(&text, &opt).unwrap();
        assert!(out.contains("\"triangles\": 12"));
        assert!(out.contains("\"watertight\": true"));
        assert!(out.contains("\"volume\": 1"));
        assert!(out.contains("\"print_ready\": true"));
        assert!(out.contains("\"weight_grams\": null"));
    }

    #[test]
    fn report_lists_the_headline_rows() {
        let text = ascii_stl("cube", &unit_cube_tris());
        let out = inspect(&text, &Options::default()).unwrap();
        assert!(out.lines().any(|l| l.trim_start().starts_with("Triangles")
            && l.trim_end().ends_with("12")));
        assert!(out
            .lines()
            .any(|l| l.trim_start().starts_with("Watertight") && l.trim_end().ends_with("yes")));
        assert!(out.contains("1 x 1 x 1 mm"));
        assert!(out.contains("Surface area"));
        assert!(out.contains("6 mm²"));
    }

    // --- mesh defects ------------------------------------------------------

    #[test]
    fn an_open_mesh_is_not_watertight() {
        let mut tris = unit_cube_tris();
        tris.truncate(10); // drop one face's two triangles
        let text = ascii_stl("open", &tris);
        let r = analyze(&text, &Options::default()).unwrap();
        assert!(!r.watertight);
        assert_eq!(r.boundary_edges, 4);
        assert!(!r.print_ready);
        assert!(r.issues.iter().any(|i| i.contains("open")));
    }

    #[test]
    fn inside_out_mesh_reports_negative_signed_volume() {
        let flipped: Vec<[Vec3; 3]> = unit_cube_tris()
            .into_iter()
            .map(|t| [t[0], t[2], t[1]])
            .collect();
        let text = ascii_stl("inverted", &flipped);
        let r = analyze(&text, &Options::default()).unwrap();
        assert!(r.watertight);
        assert!(r.signed_volume < 0.0);
        assert!((r.volume - 1.0).abs() < 1e-9);
        assert!(!r.outward_normals);
        assert!(!r.print_ready);
    }

    #[test]
    fn degenerate_and_duplicate_triangles_are_counted() {
        let mut tris = unit_cube_tris();
        // A zero-area triangle (two identical corners).
        tris.push([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0]]);
        // An exact repeat of the first face.
        tris.push(tris[0]);
        let text = ascii_stl("dirty", &tris);
        let r = analyze(&text, &Options::default()).unwrap();
        assert_eq!(r.degenerate_triangles, 1);
        assert_eq!(r.duplicate_triangles, 1);
        assert!(!r.print_ready);
    }

    #[test]
    fn a_wrong_stored_normal_is_reported() {
        let tris = unit_cube_tris();
        let mut text = String::from("solid bad\n");
        for (i, t) in tris.iter().enumerate() {
            let n = if i == 0 {
                [0.0, 0.0, 1.0] // deliberately opposite the -Z face
            } else {
                geometric_normal(t)
            };
            text.push_str(&format!(
                "  facet normal {} {} {}\n    outer loop\n",
                n[0], n[1], n[2]
            ));
            for v in t {
                text.push_str(&format!("      vertex {} {} {}\n", v[0], v[1], v[2]));
            }
            text.push_str("    endloop\n  endfacet\n");
        }
        text.push_str("endsolid bad\n");
        let r = analyze(&text, &Options::default()).unwrap();
        assert_eq!(r.normals_mismatched, 1);
        assert!(r.watertight); // geometry is fine, only the stored normal lies
    }

    #[test]
    fn unset_normals_are_counted_separately() {
        let tris = unit_cube_tris();
        let mut text = String::from("solid zeros\n");
        for t in &tris {
            text.push_str("  facet normal 0 0 0\n    outer loop\n");
            for v in t {
                text.push_str(&format!("      vertex {} {} {}\n", v[0], v[1], v[2]));
            }
            text.push_str("    endloop\n  endfacet\n");
        }
        text.push_str("endsolid zeros\n");
        let r = analyze(&text, &Options::default()).unwrap();
        assert_eq!(r.normals_unset, 12);
        assert_eq!(r.normals_mismatched, 0);
    }

    #[test]
    fn two_separate_cubes_count_as_two_shells() {
        let mut tris = unit_cube_tris();
        let far: Vec<[Vec3; 3]> = unit_cube_tris()
            .into_iter()
            .map(|t| t.map(|v| [v[0] + 10.0, v[1], v[2]]))
            .collect();
        tris.extend(far);
        let text = ascii_stl("two", &tris);
        let r = analyze(&text, &Options::default()).unwrap();
        assert_eq!(r.shells, 2);
        assert!(r.watertight);
        assert!((r.volume - 2.0).abs() < 1e-9);
        // Two closed shells still slice — reported as a note, not a defect.
        assert!(r.print_ready);
        assert!(r.issues.is_empty());
        assert!(r.notes.iter().any(|n| n.contains("2 disconnected shells")));
    }

    #[test]
    fn weld_tolerance_closes_a_hairline_crack() {
        // Nudge one shared corner by 1e-4 so the default tolerance keeps it apart.
        let mut tris = unit_cube_tris();
        for t in tris.iter_mut() {
            for v in t.iter_mut() {
                if *v == [1.0, 1.0, 1.0] {
                    *v = [1.0, 1.0, 1.000_1];
                }
            }
        }
        let text = ascii_stl("cracked", &tris);
        let tight = analyze(&text, &Options::default()).unwrap();
        assert_eq!(tight.distinct_vertices, 8); // moved consistently, still 8

        let loose = analyze(
            &text,
            &Options {
                weld_tolerance: 0.001,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(loose.watertight);
        assert_eq!(loose.distinct_vertices, 8);
    }

    // --- error paths -------------------------------------------------------

    #[test]
    fn empty_input_is_an_error() {
        let e = analyze("   \n ", &Options::default()).unwrap_err();
        assert!(e.contains("no input"), "{e}");
    }

    #[test]
    fn truncated_binary_stl_reports_the_shortfall() {
        let mut bytes = binary_stl("cube", &unit_cube_tris());
        bytes.truncate(bytes.len() - 20);
        let opt = Options {
            input_format: InputFormat::Base64,
            ..Options::default()
        };
        let e = analyze(&b64(&bytes), &opt).unwrap_err();
        assert!(e.contains("truncated"), "{e}");
        assert!(e.contains("12 triangles"), "{e}");
    }

    #[test]
    fn a_short_blob_is_not_a_binary_stl() {
        let opt = Options {
            input_format: InputFormat::Hex,
            ..Options::default()
        };
        let e = analyze("00112233", &opt).unwrap_err();
        assert!(e.contains("too short"), "{e}");
    }

    #[test]
    fn a_facet_with_two_vertices_is_rejected() {
        let text = "solid bad\n facet normal 0 0 1\n  outer loop\n   vertex 0 0 0\n   \
                    vertex 1 0 0\n  endloop\n endfacet\nendsolid bad\n";
        let e = analyze(text, &Options::default()).unwrap_err();
        assert!(e.contains("expected 3"), "{e}");
    }

    #[test]
    fn a_non_numeric_vertex_names_its_line() {
        let text = "solid bad\n facet normal 0 0 1\n  outer loop\n   vertex 0 0 0\n   \
                    vertex oops 0 0\n   vertex 1 1 0\n  endloop\n endfacet\nendsolid bad\n";
        let e = analyze(text, &Options::default()).unwrap_err();
        assert!(e.contains("line 5"), "{e}");
        assert!(e.contains("oops"), "{e}");
    }

    #[test]
    fn unterminated_facet_is_rejected() {
        let text = "solid bad\n facet normal 0 0 1\n  outer loop\n   vertex 0 0 0\n";
        let e = analyze(text, &Options::default()).unwrap_err();
        assert!(e.contains("unterminated"), "{e}");
    }

    #[test]
    fn garbage_input_explains_all_three_attempts() {
        let e = analyze("!!! not a mesh !!!", &Options::default()).unwrap_err();
        assert!(e.contains("ASCII STL"), "{e}");
        assert!(e.contains("hex"), "{e}");
        assert!(e.contains("base64"), "{e}");
    }

    #[test]
    fn odd_hex_digit_count_is_rejected() {
        let opt = Options {
            input_format: InputFormat::Hex,
            ..Options::default()
        };
        let e = analyze("abc", &opt).unwrap_err();
        assert!(e.contains("odd count"), "{e}");
    }

    #[test]
    fn a_bad_scale_is_rejected() {
        let text = ascii_stl("cube", &unit_cube_tris());
        let opt = Options {
            scale: 0.0,
            ..Options::default()
        };
        let e = analyze(&text, &opt).unwrap_err();
        assert!(e.contains("scale must be a positive number"), "{e}");
    }

    #[test]
    fn a_negative_density_is_rejected() {
        let text = ascii_stl("cube", &unit_cube_tris());
        let opt = Options {
            density: -1.0,
            ..Options::default()
        };
        let e = analyze(&text, &opt).unwrap_err();
        assert!(e.contains("density"), "{e}");
    }

    #[test]
    fn over_the_triangle_cap_is_rejected() {
        // Only the 84-byte header is needed to trip the declared-count check.
        let mut bytes = vec![0u8; 84];
        bytes[80..84].copy_from_slice(&(MAX_TRIANGLES as u32 + 1).to_le_bytes());
        let opt = Options {
            input_format: InputFormat::Hex,
            ..Options::default()
        };
        let e = analyze(&hex(&bytes), &opt).unwrap_err();
        assert!(e.contains("too many triangles"), "{e}");
    }

    #[test]
    fn parse_errors_mention_the_expected_shapes() {
        assert!(Output::parse("nope").unwrap_err().contains("report"));
        assert!(Units::parse("furlong").unwrap_err().contains("mm"));
        assert!(InputFormat::parse("uuencode")
            .unwrap_err()
            .contains("base64"));
    }
}
