//! stl-repair core — pure compute, shared by the chat skill block and the web page.
//! Diagnoses and repairs a triangle mesh pasted as ASCII STL (or Wavefront OBJ)
//! text: welds coincident vertices, drops zero-area and duplicate triangles,
//! harmonises triangle winding so every shell's normals point outward, optionally
//! fan-fills open boundary loops and drops disconnected fragments, then reports
//! watertightness, non-manifold/open edges, shells, area, volume and bounds.
//!
//! Output is either a human-readable report, the repaired mesh (ASCII STL text or
//! a `data:model/stl;base64,…` URL for binary STL), or the report as JSON. Facet
//! normals are always recomputed from the triangle winding — the normals stored in
//! the input file are ignored, which is exactly what a slicer does.
//!
//! No I/O, no dependencies.

use std::collections::HashMap;

/// Hard cap on input size so a pasted mesh can't blow up memory in wasm.
pub const MAX_TRIANGLES: usize = 100_000;

/// What the tool returns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    Report,
    Stl,
    Json,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "report" | "" => Ok(Output::Report),
            "stl" | "mesh" => Ok(Output::Stl),
            "json" => Ok(Output::Json),
            other => Err(format!(
                "unknown output '{other}': expected 'report', 'stl', or 'json'"
            )),
        }
    }
}

/// STL byte encoding for `output=stl`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StlEncoding {
    Ascii,
    Binary,
}

impl StlEncoding {
    pub fn parse(s: &str) -> Result<StlEncoding, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "ascii" | "text" | "" => Ok(StlEncoding::Ascii),
            "binary" | "bin" => Ok(StlEncoding::Binary),
            other => Err(format!(
                "unknown stl_encoding '{other}': expected 'ascii' or 'binary'"
            )),
        }
    }
}

/// Detected input format.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SourceFormat {
    AsciiStl,
    Obj,
}

impl SourceFormat {
    fn label(self) -> &'static str {
        match self {
            SourceFormat::AsciiStl => "ASCII STL",
            SourceFormat::Obj => "Wavefront OBJ",
        }
    }
}

pub struct Options {
    pub output: Output,
    pub weld_tolerance: f64,
    pub remove_degenerate: bool,
    pub remove_duplicates: bool,
    pub fix_winding: bool,
    pub fill_holes: bool,
    pub keep_largest_shell: bool,
    pub stl_encoding: StlEncoding,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            output: Output::Report,
            weld_tolerance: 1e-6,
            remove_degenerate: true,
            remove_duplicates: true,
            fix_winding: true,
            fill_holes: false,
            keep_largest_shell: false,
            stl_encoding: StlEncoding::Ascii,
        }
    }
}

type Tri = [[f64; 3]; 3];
type ITri = [usize; 3];

/// Everything the report prints, in the order it is diagnosed.
#[derive(Default, Clone, Debug)]
pub struct Report {
    pub format: String,
    pub solid_name: String,
    pub in_triangles: usize,
    pub in_vertices: usize,

    pub degenerate_found: usize,
    pub duplicate_found: usize,
    pub coincident_vertices: usize,
    pub nonmanifold_before: usize,
    pub boundary_before: usize,
    pub inconsistent_before: usize,
    pub shells_before: usize,
    pub watertight_before: bool,

    pub vertices_welded: usize,
    pub degenerate_removed: usize,
    pub duplicates_removed: usize,
    pub windings_flipped: usize,
    pub shells_flipped: usize,
    pub holes_filled: usize,
    pub hole_triangles_added: usize,
    pub fragments_removed: usize,
    pub fragment_triangles_removed: usize,

    pub out_triangles: usize,
    pub out_vertices: usize,
    pub nonmanifold_after: usize,
    pub boundary_after: usize,
    pub shells_after: usize,
    pub watertight_after: bool,

    pub surface_area: f64,
    pub volume: Option<f64>,
    pub bbox_min: [f64; 3],
    pub bbox_max: [f64; 3],
}

/// Diagnose + repair the pasted mesh and render the requested output.
pub fn repair(input: &str, opt: &Options) -> Result<String, String> {
    let (report, mesh) = analyze(input, opt)?;
    match opt.output {
        Output::Report => Ok(render_report(&report)),
        Output::Json => Ok(render_json(&report)),
        Output::Stl => match opt.stl_encoding {
            StlEncoding::Ascii => Ok(emit_ascii_stl(&mesh, &report.solid_name)),
            StlEncoding::Binary => Ok(emit_binary_stl(&mesh, &report.solid_name)),
        },
    }
}

/// Run the whole pipeline: parse → weld → diagnose → repair → re-diagnose.
pub fn analyze(input: &str, opt: &Options) -> Result<(Report, Mesh), String> {
    if opt.weld_tolerance < 0.0 || !opt.weld_tolerance.is_finite() {
        return Err("weld_tolerance must be a finite number >= 0".to_string());
    }
    let (raw, format, solid_name) = parse(input)?;
    if raw.is_empty() {
        return Err("no triangles found in the input mesh".to_string());
    }
    if raw.len() > MAX_TRIANGLES {
        return Err(format!(
            "mesh has {} triangles, which is over the {MAX_TRIANGLES} triangle limit for this tool",
            raw.len()
        ));
    }

    let mut rep = Report {
        format: format.label().to_string(),
        solid_name,
        in_triangles: raw.len(),
        ..Report::default()
    };

    // Exact-position vertex count (before tolerance welding) — this is what a
    // slicer would see as the file's distinct corner positions.
    let (exact_verts, _) = weld(&raw, 0.0)?;
    rep.in_vertices = exact_verts.len();

    let (verts, tris) = weld(&raw, opt.weld_tolerance)?;
    rep.vertices_welded = rep.in_vertices.saturating_sub(verts.len());
    rep.coincident_vertices = rep.vertices_welded;
    let mut mesh = Mesh { verts, tris };

    // ---- diagnose the mesh as parsed -------------------------------------
    rep.degenerate_found = mesh.tris.iter().filter(|t| is_degenerate(&mesh.verts, t)).count();
    rep.duplicate_found = count_duplicates(&mesh.tris);
    let before = topology(&mesh.tris);
    rep.nonmanifold_before = before.nonmanifold;
    rep.boundary_before = before.boundary;
    rep.inconsistent_before = before.inconsistent;
    rep.shells_before = shell_ids(&mesh.tris).1;
    rep.watertight_before =
        before.boundary == 0 && before.nonmanifold == 0 && before.inconsistent == 0;

    // ---- repair ----------------------------------------------------------
    if opt.remove_degenerate {
        let before_n = mesh.tris.len();
        let verts = &mesh.verts;
        mesh.tris.retain(|t| !is_degenerate(verts, t));
        rep.degenerate_removed = before_n - mesh.tris.len();
    }
    if opt.remove_duplicates {
        let before_n = mesh.tris.len();
        dedupe_triangles(&mut mesh.tris);
        rep.duplicates_removed = before_n - mesh.tris.len();
    }
    if mesh.tris.is_empty() {
        return Err(
            "every triangle in the mesh was degenerate or duplicated — nothing left to repair"
                .to_string(),
        );
    }
    if opt.keep_largest_shell {
        let (ids, count) = shell_ids(&mesh.tris);
        if count > 1 {
            let mut sizes = vec![0usize; count];
            for &id in &ids {
                sizes[id] += 1;
            }
            let keep = sizes
                .iter()
                .enumerate()
                .max_by_key(|(_, n)| **n)
                .map(|(i, _)| i)
                .unwrap_or(0);
            let before_n = mesh.tris.len();
            let mut i = 0usize;
            mesh.tris.retain(|_| {
                let k = ids[i];
                i += 1;
                k == keep
            });
            rep.fragments_removed = count - 1;
            rep.fragment_triangles_removed = before_n - mesh.tris.len();
        }
    }
    if opt.fix_winding {
        let (flipped, shells_flipped) = harmonise_winding(&mut mesh);
        rep.windings_flipped = flipped;
        rep.shells_flipped = shells_flipped;
    }
    if opt.fill_holes {
        let (loops, added) = fill_holes(&mut mesh);
        rep.holes_filled = loops;
        rep.hole_triangles_added = added;
    }

    // ---- re-diagnose + measure ------------------------------------------
    mesh.compact();
    let after = topology(&mesh.tris);
    rep.out_triangles = mesh.tris.len();
    rep.out_vertices = mesh.verts.len();
    rep.nonmanifold_after = after.nonmanifold;
    rep.boundary_after = after.boundary;
    rep.shells_after = shell_ids(&mesh.tris).1;
    rep.watertight_after =
        after.boundary == 0 && after.nonmanifold == 0 && after.inconsistent == 0;

    rep.surface_area = mesh
        .tris
        .iter()
        .map(|t| tri_area(&mesh.verts, t))
        .sum::<f64>();
    rep.volume = if rep.watertight_after {
        Some(signed_volume(&mesh.verts, &mesh.tris).abs())
    } else {
        None
    };
    let (lo, hi) = bounds(&mesh.verts);
    rep.bbox_min = lo;
    rep.bbox_max = hi;

    Ok((rep, mesh))
}

/// An indexed triangle mesh.
#[derive(Clone, Default, Debug)]
pub struct Mesh {
    pub verts: Vec<[f64; 3]>,
    pub tris: Vec<ITri>,
}

impl Mesh {
    /// Drop vertices no triangle references and renumber, so the reported vertex
    /// count matches what the exported file actually contains.
    fn compact(&mut self) {
        let mut map = vec![usize::MAX; self.verts.len()];
        let mut kept: Vec<[f64; 3]> = Vec::new();
        for t in &mut self.tris {
            for i in t.iter_mut() {
                if map[*i] == usize::MAX {
                    map[*i] = kept.len();
                    kept.push(self.verts[*i]);
                }
                *i = map[*i];
            }
        }
        self.verts = kept;
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Auto-detect ASCII STL vs Wavefront OBJ and read out a flat triangle list.
fn parse(input: &str) -> Result<(Vec<Tri>, SourceFormat, String), String> {
    let trimmed = input.trim_start();
    if trimmed.is_empty() {
        return Err("the mesh input is empty — paste an ASCII STL or OBJ file".to_string());
    }
    if trimmed.as_bytes().starts_with(b"solid") || lower_contains(trimmed, "facet normal") {
        let (tris, name) = parse_ascii_stl(trimmed)?;
        return Ok((tris, SourceFormat::AsciiStl, name));
    }
    if trimmed
        .lines()
        .any(|l| l.trim_start().starts_with("v ") || l.trim_start().starts_with("f "))
    {
        return Ok((parse_obj(trimmed)?, SourceFormat::Obj, "mesh".to_string()));
    }
    Err("could not detect the mesh format: paste ASCII STL text (solid/facet/vertex lines) or \
         Wavefront OBJ text (v/f lines). Binary STL cannot be pasted as text — re-export it as \
         ASCII STL first."
        .to_string())
}

fn lower_contains(s: &str, needle: &str) -> bool {
    s.to_ascii_lowercase().contains(needle)
}

fn parse_ascii_stl(input: &str) -> Result<(Vec<Tri>, String), String> {
    let mut name = "repaired".to_string();
    let mut pts: Vec<[f64; 3]> = Vec::new();
    for line in input.lines() {
        let l = line.trim();
        let lower = l.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("solid") {
            if name == "repaired" {
                let n = l[l.len() - rest.len()..].trim();
                if !n.is_empty() {
                    name = n.to_string();
                }
            }
            continue;
        }
        if let Some(rest) = lower.strip_prefix("vertex") {
            let start = l.len() - rest.len();
            pts.push(parse_triple(&l[start..])?);
        }
    }
    if pts.len() % 3 != 0 {
        return Err(format!(
            "malformed ASCII STL: found {} vertex lines, which is not a multiple of 3",
            pts.len()
        ));
    }
    Ok((
        pts.chunks(3).map(|c| [c[0], c[1], c[2]]).collect(),
        sanitize_name(&name),
    ))
}

fn parse_triple(s: &str) -> Result<[f64; 3], String> {
    let mut it = s.split_whitespace();
    let mut out = [0.0f64; 3];
    for (i, slot) in out.iter_mut().enumerate() {
        let tok = it
            .next()
            .ok_or_else(|| format!("expected 3 coordinates in '{}', found {i}", s.trim()))?;
        *slot = tok
            .parse::<f64>()
            .map_err(|_| format!("'{tok}' is not a number in '{}'", s.trim()))?;
        if !slot.is_finite() {
            return Err(format!("'{tok}' is not a finite coordinate"));
        }
    }
    Ok(out)
}

fn parse_obj(input: &str) -> Result<Vec<Tri>, String> {
    let mut verts: Vec<[f64; 3]> = Vec::new();
    let mut tris: Vec<Tri> = Vec::new();
    for line in input.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("v ") {
            verts.push(parse_triple(rest)?);
        } else if let Some(rest) = l.strip_prefix("f ") {
            let mut idx: Vec<usize> = Vec::new();
            for tok in rest.split_whitespace() {
                let first = tok.split('/').next().unwrap_or("");
                let n: i64 = first
                    .parse()
                    .map_err(|_| format!("bad OBJ face index '{tok}'"))?;
                let resolved = if n > 0 {
                    (n - 1) as i64
                } else if n < 0 {
                    verts.len() as i64 + n
                } else {
                    return Err("OBJ face index 0 is invalid (indices are 1-based)".to_string())
                };
                if resolved < 0 || resolved as usize >= verts.len() {
                    return Err(format!("OBJ face index '{tok}' is out of range"));
                }
                idx.push(resolved as usize);
            }
            // Fan-triangulate polygons.
            for w in 1..idx.len().saturating_sub(1) {
                tris.push([verts[idx[0]], verts[idx[w]], verts[idx[w + 1]]]);
            }
        }
    }
    Ok(tris)
}

fn sanitize_name(name: &str) -> String {
    let s: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        .take(60)
        .collect();
    if s.is_empty() {
        "repaired".to_string()
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// Welding
// ---------------------------------------------------------------------------

/// Merge coincident corners into a shared vertex list. `tol <= 0` merges only
/// bit-identical positions; a positive tolerance buckets space into `tol`-sized
/// cells and also checks the 26 neighbouring cells, so a pair straddling a cell
/// boundary still welds.
fn weld(raw: &[Tri], tol: f64) -> Result<(Vec<[f64; 3]>, Vec<ITri>), String> {
    let mut verts: Vec<[f64; 3]> = Vec::with_capacity(raw.len());
    let mut tris: Vec<ITri> = Vec::with_capacity(raw.len());
    if tol <= 0.0 {
        let mut exact: HashMap<(u64, u64, u64), usize> = HashMap::new();
        for t in raw {
            let mut it = [0usize; 3];
            for (k, v) in t.iter().enumerate() {
                let key = (bits(v[0]), bits(v[1]), bits(v[2]));
                it[k] = *exact.entry(key).or_insert_with(|| {
                    verts.push(*v);
                    verts.len() - 1
                });
            }
            tris.push(it);
        }
        return Ok((verts, tris));
    }
    let mut cells: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    let tol2 = tol * tol;
    for t in raw {
        let mut it = [0usize; 3];
        for (k, v) in t.iter().enumerate() {
            let cell = cell_of(*v, tol)?;
            let mut found: Option<usize> = None;
            'search: for dx in -1..=1i64 {
                for dy in -1..=1i64 {
                    for dz in -1..=1i64 {
                        let key = (cell.0 + dx, cell.1 + dy, cell.2 + dz);
                        if let Some(list) = cells.get(&key) {
                            for &vi in list {
                                if dist2(&verts[vi], v) <= tol2 {
                                    found = Some(vi);
                                    break 'search;
                                }
                            }
                        }
                    }
                }
            }
            it[k] = match found {
                Some(vi) => vi,
                None => {
                    verts.push(*v);
                    let vi = verts.len() - 1;
                    cells.entry(cell).or_default().push(vi);
                    vi
                }
            };
        }
        tris.push(it);
    }
    Ok((verts, tris))
}

/// Normalise `-0.0` so it hashes with `0.0`.
fn bits(v: f64) -> u64 {
    (if v == 0.0 { 0.0 } else { v }).to_bits()
}

fn cell_of(v: [f64; 3], tol: f64) -> Result<(i64, i64, i64), String> {
    let mut out = [0i64; 3];
    for i in 0..3 {
        let q = (v[i] / tol).floor();
        if !q.is_finite() || q.abs() > 9.0e18 {
            return Err(format!(
                "weld_tolerance {tol} is too small for a coordinate of {} — use a larger tolerance",
                v[i]
            ));
        }
        out[i] = q as i64;
    }
    Ok((out[0], out[1], out[2]))
}

fn dist2(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

// ---------------------------------------------------------------------------
// Diagnosis
// ---------------------------------------------------------------------------

/// A triangle is degenerate when two corners weld to the same vertex, or when the
/// three corners are exactly collinear (zero cross product → zero area).
fn is_degenerate(verts: &[[f64; 3]], t: &ITri) -> bool {
    if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
        return true;
    }
    let n = face_normal_raw(verts, t);
    n[0] == 0.0 && n[1] == 0.0 && n[2] == 0.0
}

fn face_normal_raw(verts: &[[f64; 3]], t: &ITri) -> [f64; 3] {
    let (a, b, c) = (verts[t[0]], verts[t[1]], verts[t[2]]);
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

fn face_normal(verts: &[[f64; 3]], t: &ITri) -> [f64; 3] {
    let n = face_normal_raw(verts, t);
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len == 0.0 || !len.is_finite() {
        [0.0, 0.0, 0.0]
    } else {
        [n[0] / len, n[1] / len, n[2] / len]
    }
}

fn tri_area(verts: &[[f64; 3]], t: &ITri) -> f64 {
    let n = face_normal_raw(verts, t);
    0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
}

fn signed_volume(verts: &[[f64; 3]], tris: &[ITri]) -> f64 {
    let mut sum = 0.0;
    for t in tris {
        let (a, b, c) = (verts[t[0]], verts[t[1]], verts[t[2]]);
        sum += a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]);
    }
    sum / 6.0
}

fn bounds(verts: &[[f64; 3]]) -> ([f64; 3], [f64; 3]) {
    if verts.is_empty() {
        return ([0.0; 3], [0.0; 3]);
    }
    let mut lo = verts[0];
    let mut hi = verts[0];
    for v in verts {
        for i in 0..3 {
            if v[i] < lo[i] {
                lo[i] = v[i];
            }
            if v[i] > hi[i] {
                hi[i] = v[i];
            }
        }
    }
    (lo, hi)
}

/// Triangles that use the same three vertices — in any rotation and either
/// winding — are duplicates; only the first occurrence is real geometry.
fn tri_key(t: &ITri) -> (usize, usize, usize) {
    let mut k = *t;
    k.sort_unstable();
    (k[0], k[1], k[2])
}

fn count_duplicates(tris: &[ITri]) -> usize {
    let mut seen: HashMap<(usize, usize, usize), ()> = HashMap::new();
    let mut dups = 0;
    for t in tris {
        if seen.insert(tri_key(t), ()).is_some() {
            dups += 1;
        }
    }
    dups
}

fn dedupe_triangles(tris: &mut Vec<ITri>) {
    let mut seen: HashMap<(usize, usize, usize), ()> = HashMap::new();
    tris.retain(|t| seen.insert(tri_key(t), ()).is_none());
}

struct Topo {
    /// Edges used by exactly one triangle — the mesh is open there.
    boundary: usize,
    /// Edges shared by three or more triangles — no slicer can decide inside/out.
    nonmanifold: usize,
    /// Edges shared by exactly two triangles that traverse them the same way,
    /// i.e. one of the two faces is wound backwards relative to its neighbour.
    inconsistent: usize,
}

/// `(forward uses, backward uses)` per undirected edge, keyed low→high vertex.
fn edge_counts(tris: &[ITri]) -> HashMap<(usize, usize), (u32, u32)> {
    let mut m: HashMap<(usize, usize), (u32, u32)> = HashMap::with_capacity(tris.len() * 3);
    for t in tris {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (key, fwd) = if a < b { ((a, b), true) } else { ((b, a), false) };
            let e = m.entry(key).or_insert((0, 0));
            if fwd {
                e.0 += 1;
            } else {
                e.1 += 1;
            }
        }
    }
    m
}

fn topology(tris: &[ITri]) -> Topo {
    let mut t = Topo {
        boundary: 0,
        nonmanifold: 0,
        inconsistent: 0,
    };
    for (_, (f, b)) in edge_counts(tris) {
        match f + b {
            1 => t.boundary += 1,
            2 => {
                if f != 1 {
                    t.inconsistent += 1;
                }
            }
            _ => t.nonmanifold += 1,
        }
    }
    t
}

/// Union-find grouping of triangles connected through a shared edge.
/// Returns `(shell id per triangle, shell count)`.
fn shell_ids(tris: &[ITri]) -> (Vec<usize>, usize) {
    let mut parent: Vec<usize> = (0..tris.len()).collect();
    fn find(parent: &mut Vec<usize>, mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    let mut first: HashMap<(usize, usize), usize> = HashMap::with_capacity(tris.len() * 3);
    for (ti, t) in tris.iter().enumerate() {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let key = if a < b { (a, b) } else { (b, a) };
            match first.get(&key) {
                Some(&other) => {
                    let (ra, rb) = (find(&mut parent, ti), find(&mut parent, other));
                    if ra != rb {
                        parent[ra] = rb;
                    }
                }
                None => {
                    first.insert(key, ti);
                }
            }
        }
    }
    let mut ids = vec![0usize; tris.len()];
    let mut map: HashMap<usize, usize> = HashMap::new();
    for i in 0..tris.len() {
        let r = find(&mut parent, i);
        let next = map.len();
        ids[i] = *map.entry(r).or_insert(next);
    }
    (ids, map.len())
}

// ---------------------------------------------------------------------------
// Repair steps
// ---------------------------------------------------------------------------

/// Make every triangle in a shell agree on winding (flood fill across manifold
/// edges), then flip whole closed shells whose signed volume is negative so the
/// recomputed normals point outward. Returns `(triangles flipped, shells flipped)`.
///
/// The flood fill's seed is arbitrary, so it can end up "correcting" the majority
/// to match one bad face. After the fill each shell is restored to whichever
/// winding the majority of its triangles started with, which makes the reported
/// flip count the number of genuinely misoriented faces rather than a seed artefact.
fn harmonise_winding(mesh: &mut Mesh) -> (usize, usize) {
    let n = mesh.tris.len();
    let original = mesh.tris.clone();
    let (ids, shells) = shell_ids(&mesh.tris);
    // Adjacency across edges used by exactly two triangles.
    let mut edge_tris: HashMap<(usize, usize), Vec<usize>> = HashMap::with_capacity(n * 3);
    for (ti, t) in mesh.tris.iter().enumerate() {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let key = if a < b { (a, b) } else { (b, a) };
            edge_tris.entry(key).or_default().push(ti);
        }
    }
    let mut adj: Vec<Vec<(usize, (usize, usize))>> = vec![Vec::new(); n];
    for (key, list) in &edge_tris {
        if list.len() == 2 {
            adj[list[0]].push((list[1], *key));
            adj[list[1]].push((list[0], *key));
        }
    }

    let mut visited = vec![false; n];
    for seed in 0..n {
        if visited[seed] {
            continue;
        }
        visited[seed] = true;
        let mut stack = vec![seed];
        while let Some(cur) = stack.pop() {
            let neighbours = adj[cur].clone();
            for (nb, key) in neighbours {
                if visited[nb] {
                    continue;
                }
                visited[nb] = true;
                // Consistent orientation ⇒ the two faces traverse the shared
                // edge in OPPOSITE directions.
                if edge_dir(&mesh.tris[cur], key) == edge_dir(&mesh.tris[nb], key) {
                    mesh.tris[nb].swap(1, 2);
                }
                stack.push(nb);
            }
        }
    }

    // The flood fill's seed is arbitrary, so a shell can come out globally
    // inverted just because the seed happened to be the one bad face. Put each
    // shell back on whichever winding most of its faces started with.
    let mut changed = vec![0usize; shells];
    let mut total = vec![0usize; shells];
    for i in 0..n {
        total[ids[i]] += 1;
        if mesh.tris[i] != original[i] {
            changed[ids[i]] += 1;
        }
    }
    for i in 0..n {
        if changed[ids[i]] * 2 > total[ids[i]] {
            mesh.tris[i].swap(1, 2);
        }
    }
    let flips = (0..n).filter(|&i| mesh.tris[i] != original[i]).count();

    // Outward orientation: a closed shell wound inside-out has negative volume.
    let mut shells_flipped = 0usize;
    if shells > 0 {
        let mut per_shell: Vec<Vec<ITri>> = vec![Vec::new(); shells];
        for (ti, t) in mesh.tris.iter().enumerate() {
            per_shell[ids[ti]].push(*t);
        }
        let mut flip_shell = vec![false; shells];
        for (si, group) in per_shell.iter().enumerate() {
            let topo = topology(group);
            let closed = topo.boundary == 0 && topo.nonmanifold == 0 && topo.inconsistent == 0;
            if closed && signed_volume(&mesh.verts, group) < 0.0 {
                flip_shell[si] = true;
                shells_flipped += 1;
            }
        }
        if shells_flipped > 0 {
            for (ti, t) in mesh.tris.iter_mut().enumerate() {
                if flip_shell[ids[ti]] {
                    t.swap(1, 2);
                }
            }
        }
    }
    (flips, shells_flipped)
}

/// Which way triangle `t` traverses undirected edge `key = (lo, hi)`.
fn edge_dir(t: &ITri, key: (usize, usize)) -> bool {
    for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
        if (a, b) == key {
            return true;
        }
        if (b, a) == key {
            return false;
        }
    }
    false
}

/// Fan-fill every closed boundary loop from its own centroid. Returns
/// `(loops filled, triangles added)`. Chains that do not close (branching or
/// self-touching boundaries) are left alone rather than patched wrongly.
fn fill_holes(mesh: &mut Mesh) -> (usize, usize) {
    let counts = edge_counts(&mesh.tris);
    // Directed boundary edges, taken from the single triangle that owns each.
    let mut out_edges: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut total = 0usize;
    for t in &mesh.tris {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let key = if a < b { (a, b) } else { (b, a) };
            if let Some((f, bw)) = counts.get(&key) {
                if f + bw == 1 {
                    out_edges.entry(a).or_default().push(b);
                    total += 1;
                }
            }
        }
    }
    if total == 0 {
        return (0, 0);
    }
    let mut starts: Vec<usize> = out_edges.keys().copied().collect();
    starts.sort_unstable();

    let mut loops = 0usize;
    let mut added = 0usize;
    for s in starts {
        loop {
            let first = match out_edges.get_mut(&s).and_then(|v| v.pop()) {
                Some(b) => b,
                None => break,
            };
            let mut chain: Vec<(usize, usize)> = vec![(s, first)];
            let mut cur = first;
            let mut ok = false;
            while chain.len() <= total {
                if cur == s {
                    ok = true;
                    break;
                }
                match out_edges.get_mut(&cur).and_then(|v| v.pop()) {
                    Some(next) => {
                        chain.push((cur, next));
                        cur = next;
                    }
                    None => break,
                }
            }
            if !ok || chain.len() < 3 {
                continue;
            }
            // Centroid of the loop rim; each patch triangle reverses its boundary
            // edge so the patch winds consistently with the surrounding surface.
            let mut c = [0.0f64; 3];
            for (a, _) in &chain {
                let v = mesh.verts[*a];
                c[0] += v[0];
                c[1] += v[1];
                c[2] += v[2];
            }
            let k = chain.len() as f64;
            let centroid = [c[0] / k, c[1] / k, c[2] / k];
            mesh.verts.push(centroid);
            let ci = mesh.verts.len() - 1;
            for (a, b) in &chain {
                mesh.tris.push([ci, *b, *a]);
                added += 1;
            }
            loops += 1;
        }
    }
    (loops, added)
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Compact float: whole numbers print without a decimal point, otherwise up to 6
/// decimals with trailing zeros trimmed. `-0` normalises to `0`.
fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Areas/volumes can be far below the 6-decimal window; fall back to exponent form.
fn fmt_measure(v: f64) -> String {
    if v != 0.0 && v.abs() < 1e-4 {
        format!("{v:.6e}")
    } else {
        fmt_num(v)
    }
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn render_report(r: &Report) -> String {
    let mut s = String::new();
    s.push_str("STL repair report\n=================\n\nInput\n");
    s.push_str(&row("Format", r.format.clone()));
    s.push_str(&row("Solid name", r.solid_name.clone()));
    s.push_str(&row("Triangles", r.in_triangles.to_string()));
    s.push_str(&row("Distinct vertices", r.in_vertices.to_string()));

    s.push_str("\nProblems found\n");
    s.push_str(&row("Degenerate triangles", r.degenerate_found.to_string()));
    s.push_str(&row("Duplicate triangles", r.duplicate_found.to_string()));
    s.push_str(&row("Coincident vertices", r.coincident_vertices.to_string()));
    s.push_str(&row("Non-manifold edges", r.nonmanifold_before.to_string()));
    s.push_str(&row("Open (boundary) edges", r.boundary_before.to_string()));
    s.push_str(&row("Flipped triangles", r.inconsistent_before.to_string()));
    s.push_str(&row("Disconnected shells", r.shells_before.to_string()));
    s.push_str(&row("Watertight", yes_no(r.watertight_before).to_string()));

    s.push_str("\nRepairs applied\n");
    s.push_str(&row("Vertices welded", r.vertices_welded.to_string()));
    s.push_str(&row("Degenerate removed", r.degenerate_removed.to_string()));
    s.push_str(&row("Duplicates removed", r.duplicates_removed.to_string()));
    s.push_str(&row("Triangles re-wound", r.windings_flipped.to_string()));
    s.push_str(&row("Shells turned outward", r.shells_flipped.to_string()));
    s.push_str(&row(
        "Holes filled",
        format!(
            "{} ({} triangles added)",
            r.holes_filled, r.hole_triangles_added
        ),
    ));
    s.push_str(&row(
        "Fragments removed",
        format!(
            "{} ({} triangles dropped)",
            r.fragments_removed, r.fragment_triangles_removed
        ),
    ));

    s.push_str("\nResult\n");
    s.push_str(&row("Triangles", r.out_triangles.to_string()));
    s.push_str(&row("Distinct vertices", r.out_vertices.to_string()));
    s.push_str(&row("Non-manifold edges", r.nonmanifold_after.to_string()));
    s.push_str(&row("Open (boundary) edges", r.boundary_after.to_string()));
    s.push_str(&row("Disconnected shells", r.shells_after.to_string()));
    s.push_str(&row("Watertight", yes_no(r.watertight_after).to_string()));
    s.push_str(&row("Surface area", fmt_measure(r.surface_area)));
    s.push_str(&row(
        "Volume",
        match r.volume {
            Some(v) => fmt_measure(v),
            None => "n/a (mesh is not closed)".to_string(),
        },
    ));
    s.push_str(&row(
        "Bounding box",
        format!(
            "{} x {} x {}",
            fmt_num(r.bbox_max[0] - r.bbox_min[0]),
            fmt_num(r.bbox_max[1] - r.bbox_min[1]),
            fmt_num(r.bbox_max[2] - r.bbox_min[2])
        ),
    ));
    s.push_str(&row(
        "Bounds",
        format!(
            "min {} {} {} / max {} {} {}",
            fmt_num(r.bbox_min[0]),
            fmt_num(r.bbox_min[1]),
            fmt_num(r.bbox_min[2]),
            fmt_num(r.bbox_max[0]),
            fmt_num(r.bbox_max[1]),
            fmt_num(r.bbox_max[2])
        ),
    ));
    s.push_str("\nLengths, areas and volumes are in the mesh's own units (STL stores no unit).\n");
    s
}

fn row(label: &str, value: String) -> String {
    format!("  {label:<24}{value}\n")
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

fn jnum(v: f64) -> String {
    if v.is_finite() {
        fmt_num(v)
    } else {
        "0".to_string()
    }
}

fn render_json(r: &Report) -> String {
    format!(
        concat!(
            "{{\n",
            "  \"input\": {{ \"format\": \"{}\", \"solid_name\": \"{}\", \"triangles\": {}, \"vertices\": {} }},\n",
            "  \"problems\": {{ \"degenerate_triangles\": {}, \"duplicate_triangles\": {}, \"coincident_vertices\": {}, ",
            "\"non_manifold_edges\": {}, \"boundary_edges\": {}, \"flipped_triangles\": {}, \"shells\": {}, \"watertight\": {} }},\n",
            "  \"repairs\": {{ \"vertices_welded\": {}, \"degenerate_removed\": {}, \"duplicates_removed\": {}, ",
            "\"triangles_rewound\": {}, \"shells_turned_outward\": {}, \"holes_filled\": {}, \"hole_triangles_added\": {}, ",
            "\"fragments_removed\": {}, \"fragment_triangles_removed\": {} }},\n",
            "  \"result\": {{ \"triangles\": {}, \"vertices\": {}, \"non_manifold_edges\": {}, \"boundary_edges\": {}, ",
            "\"shells\": {}, \"watertight\": {}, \"surface_area\": {}, \"volume\": {}, ",
            "\"bbox_min\": [{}, {}, {}], \"bbox_max\": [{}, {}, {}], \"dimensions\": [{}, {}, {}] }}\n",
            "}}"
        ),
        json_escape(&r.format),
        json_escape(&r.solid_name),
        r.in_triangles,
        r.in_vertices,
        r.degenerate_found,
        r.duplicate_found,
        r.coincident_vertices,
        r.nonmanifold_before,
        r.boundary_before,
        r.inconsistent_before,
        r.shells_before,
        r.watertight_before,
        r.vertices_welded,
        r.degenerate_removed,
        r.duplicates_removed,
        r.windings_flipped,
        r.shells_flipped,
        r.holes_filled,
        r.hole_triangles_added,
        r.fragments_removed,
        r.fragment_triangles_removed,
        r.out_triangles,
        r.out_vertices,
        r.nonmanifold_after,
        r.boundary_after,
        r.shells_after,
        r.watertight_after,
        jnum(r.surface_area),
        match r.volume {
            Some(v) => jnum(v),
            None => "null".to_string(),
        },
        jnum(r.bbox_min[0]),
        jnum(r.bbox_min[1]),
        jnum(r.bbox_min[2]),
        jnum(r.bbox_max[0]),
        jnum(r.bbox_max[1]),
        jnum(r.bbox_max[2]),
        jnum(r.bbox_max[0] - r.bbox_min[0]),
        jnum(r.bbox_max[1] - r.bbox_min[1]),
        jnum(r.bbox_max[2] - r.bbox_min[2]),
    )
}

fn emit_ascii_stl(mesh: &Mesh, name: &str) -> String {
    let mut s = format!("solid {name}\n");
    for t in &mesh.tris {
        let n = face_normal(&mesh.verts, t);
        s.push_str(&format!(
            "  facet normal {} {} {}\n    outer loop\n",
            fmt_num(n[0]),
            fmt_num(n[1]),
            fmt_num(n[2])
        ));
        for &i in t {
            let v = mesh.verts[i];
            s.push_str(&format!(
                "      vertex {} {} {}\n",
                fmt_num(v[0]),
                fmt_num(v[1]),
                fmt_num(v[2])
            ));
        }
        s.push_str("    endloop\n  endfacet\n");
    }
    s.push_str(&format!("endsolid {name}\n"));
    s
}

/// Binary STL wrapped in a data URL so the page can offer it as a download and
/// the CLI/chat can hand back a saveable blob.
fn emit_binary_stl(mesh: &Mesh, name: &str) -> String {
    let mut buf: Vec<u8> = Vec::with_capacity(84 + mesh.tris.len() * 50);
    let mut header = [0u8; 80];
    let label = format!("gizza stl-repair {name}");
    for (i, b) in label.bytes().take(79).enumerate() {
        header[i] = b;
    }
    buf.extend_from_slice(&header);
    buf.extend_from_slice(&(mesh.tris.len() as u32).to_le_bytes());
    for t in &mesh.tris {
        let n = face_normal(&mesh.verts, t);
        for c in n {
            buf.extend_from_slice(&(c as f32).to_le_bytes());
        }
        for &i in t {
            for c in mesh.verts[i] {
                buf.extend_from_slice(&(c as f32).to_le_bytes());
            }
        }
        buf.extend_from_slice(&0u16.to_le_bytes());
    }
    format!("data:model/stl;base64,{}", base64(&buf))
}

/// Minimal standard base64 (no external dep in `core`).
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A closed unit cube as ASCII STL, 12 triangles, outward winding.
    fn cube_stl() -> String {
        let tris = cube_tris();
        let mut s = String::from("solid cube\n");
        for t in tris {
            s.push_str("  facet normal 0 0 0\n    outer loop\n");
            for v in t {
                s.push_str(&format!("      vertex {} {} {}\n", v[0], v[1], v[2]));
            }
            s.push_str("    endloop\n  endfacet\n");
        }
        s.push_str("endsolid cube\n");
        s
    }

    fn cube_tris() -> Vec<Tri> {
        let v = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        // Outward-facing quads, each split into two triangles.
        let quads = [
            [0, 3, 2, 1], // bottom (-Z)
            [4, 5, 6, 7], // top (+Z)
            [0, 1, 5, 4], // front (-Y)
            [1, 2, 6, 5], // right (+X)
            [2, 3, 7, 6], // back (+Y)
            [3, 0, 4, 7], // left (-X)
        ];
        let mut out = Vec::new();
        for q in quads {
            out.push([v[q[0]], v[q[1]], v[q[2]]]);
            out.push([v[q[0]], v[q[2]], v[q[3]]]);
        }
        out
    }

    fn tris_to_stl(tris: &[Tri], name: &str) -> String {
        let mut s = format!("solid {name}\n");
        for t in tris {
            s.push_str("  facet normal 0 0 0\n    outer loop\n");
            for v in t {
                s.push_str(&format!("      vertex {} {} {}\n", v[0], v[1], v[2]));
            }
            s.push_str("    endloop\n  endfacet\n");
        }
        s.push_str(&format!("endsolid {name}\n"));
        s
    }

    #[test]
    fn clean_cube_is_watertight() {
        let (r, mesh) = analyze(&cube_stl(), &Options::default()).unwrap();
        assert_eq!(r.in_triangles, 12);
        assert_eq!(r.in_vertices, 8);
        assert!(r.watertight_before);
        assert!(r.watertight_after);
        assert_eq!(r.nonmanifold_after, 0);
        assert_eq!(r.boundary_after, 0);
        assert_eq!(r.shells_after, 1);
        assert!((r.volume.unwrap() - 1.0).abs() < 1e-12);
        assert!((r.surface_area - 6.0).abs() < 1e-12);
        assert_eq!(mesh.tris.len(), 12);
        assert_eq!(mesh.verts.len(), 8);
    }

    #[test]
    fn degenerate_and_duplicate_triangles_are_removed() {
        let mut tris = cube_tris();
        // Duplicate of the first face, plus a zero-area sliver.
        tris.push(tris[0]);
        tris.push([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
        let (r, mesh) = analyze(&tris_to_stl(&tris, "dirty"), &Options::default()).unwrap();
        assert_eq!(r.in_triangles, 14);
        assert_eq!(r.degenerate_found, 1);
        assert_eq!(r.duplicate_found, 1);
        assert_eq!(r.degenerate_removed, 1);
        assert_eq!(r.duplicates_removed, 1);
        assert_eq!(r.out_triangles, 12);
        assert!(r.watertight_after);
        assert_eq!(mesh.tris.len(), 12);
    }

    #[test]
    fn flipped_face_is_rewound() {
        let mut tris = cube_tris();
        tris[0].swap(1, 2); // flip one face inward
        let stl = tris_to_stl(&tris, "flipped");
        let (bad, _) = analyze(
            &stl,
            &Options {
                fix_winding: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(bad.inconsistent_before, 3);
        assert!(!bad.watertight_after);

        let (r, _) = analyze(&stl, &Options::default()).unwrap();
        assert_eq!(r.inconsistent_before, 3);
        assert_eq!(r.windings_flipped, 1);
        assert_eq!(r.shells_flipped, 0);
        assert!(r.watertight_after);
        assert!((r.volume.unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn inside_out_cube_is_turned_outward() {
        let tris: Vec<Tri> = cube_tris()
            .into_iter()
            .map(|mut t| {
                t.swap(1, 2);
                t
            })
            .collect();
        let (r, mesh) = analyze(&tris_to_stl(&tris, "inverted"), &Options::default()).unwrap();
        assert_eq!(r.windings_flipped, 0);
        assert_eq!(r.shells_flipped, 1);
        assert!(r.watertight_after);
        // Outward winding ⇒ positive signed volume.
        assert!(signed_volume(&mesh.verts, &mesh.tris) > 0.0);
    }

    #[test]
    fn open_cube_reports_holes_and_can_be_filled() {
        let mut tris = cube_tris();
        tris.truncate(10); // drop the two triangles of the left face
        let stl = tris_to_stl(&tris, "open");
        let (r, _) = analyze(&stl, &Options::default()).unwrap();
        assert_eq!(r.boundary_before, 4);
        assert!(!r.watertight_before);
        assert!(!r.watertight_after);
        assert_eq!(r.volume, None);

        let (f, mesh) = analyze(
            &stl,
            &Options {
                fill_holes: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(f.holes_filled, 1);
        assert_eq!(f.hole_triangles_added, 4);
        assert_eq!(f.boundary_after, 0);
        assert!(f.watertight_after);
        assert_eq!(mesh.tris.len(), 14);
        // The fan centroid sits in the middle of the missing face, so the closed
        // volume is the cube minus the pyramid the patch pushes inward.
        assert!(f.volume.unwrap() > 0.0);
    }

    #[test]
    fn coincident_vertices_weld_within_tolerance() {
        let mut tris = cube_tris();
        // Nudge one shared corner apart by less than the default tolerance.
        tris[0][0] = [1e-9, 0.0, 0.0];
        let stl = tris_to_stl(&tris, "seams");
        let (loose, _) = analyze(&stl, &Options::default()).unwrap();
        assert_eq!(loose.in_vertices, 9);
        assert_eq!(loose.vertices_welded, 1);
        assert!(loose.watertight_after);

        let (exact, _) = analyze(
            &stl,
            &Options {
                weld_tolerance: 0.0,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(exact.vertices_welded, 0);
        // The un-welded corner tears four edges open: the two the nudged face
        // abandoned, and the two it created at its new position.
        assert_eq!(exact.boundary_after, 4);
        assert!(!exact.watertight_after);
    }

    #[test]
    fn fragments_can_be_dropped() {
        let mut tris = cube_tris();
        tris.push([[9.0, 9.0, 9.0], [10.0, 9.0, 9.0], [9.0, 10.0, 9.0]]);
        let stl = tris_to_stl(&tris, "frag");
        let (keep, _) = analyze(&stl, &Options::default()).unwrap();
        assert_eq!(keep.shells_before, 2);
        assert_eq!(keep.shells_after, 2);
        assert!(!keep.watertight_after);

        let (drop, mesh) = analyze(
            &stl,
            &Options {
                keep_largest_shell: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(drop.fragments_removed, 1);
        assert_eq!(drop.fragment_triangles_removed, 1);
        assert_eq!(drop.shells_after, 1);
        assert!(drop.watertight_after);
        assert_eq!(mesh.verts.len(), 8);
    }

    #[test]
    fn obj_input_is_accepted() {
        let obj = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
        let (r, _) = analyze(obj, &Options::default()).unwrap();
        assert_eq!(r.format, "Wavefront OBJ");
        assert_eq!(r.in_triangles, 1);
        assert_eq!(r.boundary_after, 3);
        assert!(!r.watertight_after);
    }

    #[test]
    fn ascii_stl_output_round_trips() {
        let out = repair(
            &cube_stl(),
            &Options {
                output: Output::Stl,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(out.starts_with("solid cube\n"));
        assert_eq!(out.matches("facet normal").count(), 12);
        assert!(out.contains("facet normal 0 0 -1"));
        let (r, _) = analyze(&out, &Options::default()).unwrap();
        assert!(r.watertight_after);
        assert_eq!(r.out_triangles, 12);
    }

    #[test]
    fn binary_stl_output_is_a_data_url_of_the_right_size() {
        let out = repair(
            &cube_stl(),
            &Options {
                output: Output::Stl,
                stl_encoding: StlEncoding::Binary,
                ..Options::default()
            },
        )
        .unwrap();
        let b64 = out.strip_prefix("data:model/stl;base64,").expect("data url");
        // 84-byte header + 50 bytes per triangle = 684 bytes → 912 base64 chars.
        assert_eq!(b64.len(), 912);
    }

    #[test]
    fn json_output_is_parseable_shape() {
        let out = repair(
            &cube_stl(),
            &Options {
                output: Output::Json,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(out.starts_with('{') && out.ends_with('}'));
        assert!(out.contains("\"watertight\": true"));
        assert!(out.contains("\"volume\": 1"));
    }

    #[test]
    fn report_lists_the_headline_numbers() {
        let out = repair(&cube_stl(), &Options::default()).unwrap();
        assert!(out.contains("STL repair report"));
        assert!(out
            .lines()
            .any(|l| l.trim_start().starts_with("Watertight") && l.trim_end().ends_with("yes")));
        assert!(out
            .lines()
            .any(|l| l.trim_start().starts_with("Bounding box") && l.contains("1 x 1 x 1")));
        assert!(out
            .lines()
            .any(|l| l.trim_start().starts_with("Volume") && l.trim_end().ends_with('1')));
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = analyze("   ", &Options::default()).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn unrecognised_input_is_an_error() {
        let err = analyze("hello world", &Options::default()).unwrap_err();
        assert!(err.contains("could not detect the mesh format"), "{err}");
    }

    #[test]
    fn bad_stl_vertex_count_is_an_error() {
        let stl = "solid x\n facet normal 0 0 1\n  outer loop\n   vertex 0 0 0\n   vertex 1 0 0\n  endloop\n endfacet\nendsolid x\n";
        let err = analyze(stl, &Options::default()).unwrap_err();
        assert!(err.contains("not a multiple of 3"), "{err}");
    }

    #[test]
    fn negative_tolerance_is_an_error() {
        let err = analyze(
            &cube_stl(),
            &Options {
                weld_tolerance: -1.0,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("weld_tolerance"), "{err}");
    }

    #[test]
    fn unknown_enum_values_are_errors() {
        assert!(Output::parse("pdf").is_err());
        assert!(StlEncoding::parse("gzip").is_err());
        assert_eq!(Output::parse("REPORT").unwrap(), Output::Report);
        assert_eq!(StlEncoding::parse("Binary").unwrap(), StlEncoding::Binary);
    }
}
