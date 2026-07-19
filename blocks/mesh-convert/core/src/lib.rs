//! mesh-convert core — pure compute, shared by the chat skill block and the web page.
//! Converts 3D meshes between Wavefront OBJ and STL (both directions, ASCII or
//! binary STL). Input is pasted TEXT: OBJ or ASCII STL (binary STL cannot be
//! pasted as text). Output is OBJ text, ASCII STL text, or — for binary STL — a
//! `data:model/stl;base64,…` URL the page turns into a download. No I/O, no deps.
//!
//! A mesh is reduced to a flat list of triangles (each three [x,y,z] vertices).
//! OBJ materials/UVs/normals and STL facet normals are discarded on input and
//! recomputed per-face on STL output — STL stores only raw triangles.

use std::collections::HashMap;

/// Output target format.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    Obj,
    Stl,
}

impl Target {
    pub fn parse(s: &str) -> Result<Target, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "obj" => Ok(Target::Obj),
            "stl" => Ok(Target::Stl),
            other => Err(format!("unknown target format '{other}': expected 'obj' or 'stl'")),
        }
    }
}

/// STL byte encoding (ignored when the target is OBJ).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StlEncoding {
    Ascii,
    Binary,
}

impl StlEncoding {
    pub fn parse(s: &str) -> Result<StlEncoding, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "ascii" | "text" => Ok(StlEncoding::Ascii),
            "binary" | "bin" => Ok(StlEncoding::Binary),
            other => Err(format!(
                "unknown STL encoding '{other}': expected 'ascii' or 'binary'"
            )),
        }
    }
}

/// Coordinate-frame reorientation (graphics Y-up <-> 3D-print Z-up).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    Keep,
    YupToZup,
    ZupToYup,
}

impl Axis {
    pub fn parse(s: &str) -> Result<Axis, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "keep" | "" => Ok(Axis::Keep),
            "y-up-to-z-up" | "y-to-z" | "yup-to-zup" => Ok(Axis::YupToZup),
            "z-up-to-y-up" | "z-to-y" | "zup-to-yup" => Ok(Axis::ZupToYup),
            other => Err(format!(
                "unknown axis '{other}': expected 'keep', 'y-up-to-z-up', or 'z-up-to-y-up'"
            )),
        }
    }

    fn apply(self, v: [f64; 3]) -> [f64; 3] {
        let [x, y, z] = v;
        match self {
            // Rotate +90 deg about X: old +Y becomes new +Z.
            Axis::YupToZup => [x, -z, y],
            // Rotate -90 deg about X: old +Z becomes new +Y.
            Axis::ZupToYup => [x, z, -y],
            Axis::Keep => [x, y, z],
        }
    }
}

pub struct Options {
    pub to: Target,
    pub stl_encoding: StlEncoding,
    pub scale: f64,
    pub axis: Axis,
    pub name: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            to: Target::Stl,
            stl_encoding: StlEncoding::Ascii,
            scale: 1.0,
            axis: Axis::Keep,
            name: "mesh".to_string(),
        }
    }
}

type Tri = [[f64; 3]; 3];

/// Auto-detect the input format and convert to `opt.to`.
pub fn convert(input: &str, opt: &Options) -> Result<String, String> {
    let mut tris = parse(input)?;
    if tris.is_empty() {
        return Err("no triangles found in the input mesh".to_string());
    }
    // Apply scale then axis reorientation to every vertex.
    let scale = if opt.scale == 0.0 { 1.0 } else { opt.scale };
    for t in tris.iter_mut() {
        for v in t.iter_mut() {
            let s = [v[0] * scale, v[1] * scale, v[2] * scale];
            *v = opt.axis.apply(s);
        }
    }
    let name = sanitize_name(&opt.name);
    match opt.to {
        Target::Obj => Ok(emit_obj(&tris, &name)),
        Target::Stl => match opt.stl_encoding {
            StlEncoding::Ascii => Ok(emit_stl_ascii(&tris, &name)),
            StlEncoding::Binary => Ok(emit_stl_binary_data_url(&tris, &name)),
        },
    }
}

fn sanitize_name(name: &str) -> String {
    let n = name.trim();
    if n.is_empty() {
        "mesh".to_string()
    } else {
        // Keep to a single line; STL solid names and OBJ `o` names are one token line.
        n.replace(['\n', '\r'], " ")
    }
}

// ---- format detection + parsing ----------------------------------------------

fn parse(input: &str) -> Result<Vec<Tri>, String> {
    let trimmed = input.trim_start();
    if trimmed.is_empty() {
        return Err("empty input: paste Wavefront OBJ or ASCII STL text".to_string());
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("solid") && lower.contains("facet") {
        parse_stl_ascii(input)
    } else if has_obj_content(input) {
        parse_obj(input)
    } else {
        Err("unrecognized mesh format: expected Wavefront OBJ (v/f lines) or ASCII STL \
             (solid/facet/vertex) text. Binary STL cannot be pasted — re-export it as ASCII STL."
            .to_string())
    }
}

fn has_obj_content(input: &str) -> bool {
    input.lines().any(|l| {
        let t = l.trim_start();
        t.starts_with("v ") || t.starts_with("f ") || t.starts_with("v\t") || t.starts_with("f\t")
    })
}

/// Parse ASCII STL: collect every `vertex x y z`, group into triangles.
/// Facet normals are ignored (recomputed on output).
fn parse_stl_ascii(input: &str) -> Result<Vec<Tri>, String> {
    let mut verts: Vec<[f64; 3]> = Vec::new();
    for (i, line) in input.lines().enumerate() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("vertex") {
            let nums = parse_coords(rest).map_err(|e| format!("line {}: {e}", i + 1))?;
            verts.push(nums);
        }
    }
    if verts.is_empty() {
        return Err("ASCII STL has no vertex lines".to_string());
    }
    if verts.len() % 3 != 0 {
        return Err(format!(
            "ASCII STL vertex count {} is not a multiple of 3 (each facet needs exactly 3 vertices)",
            verts.len()
        ));
    }
    Ok(verts.chunks(3).map(|c| [c[0], c[1], c[2]]).collect())
}

/// Parse a whitespace-separated triple of floats.
fn parse_coords(s: &str) -> Result<[f64; 3], String> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 3 {
        return Err(format!("expected 3 coordinates, got {}", parts.len()));
    }
    let mut out = [0.0f64; 3];
    for (k, p) in parts.iter().take(3).enumerate() {
        out[k] = p
            .parse::<f64>()
            .map_err(|_| format!("'{p}' is not a number"))?;
    }
    Ok(out)
}

/// Parse Wavefront OBJ: `v x y z` vertices and `f …` faces (fan-triangulated).
fn parse_obj(input: &str) -> Result<Vec<Tri>, String> {
    let mut verts: Vec<[f64; 3]> = Vec::new();
    let mut tris: Vec<Tri> = Vec::new();
    for (i, line) in input.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("");
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let rest: Vec<&str> = it.collect();
                let coords = parse_coords(&rest.join(" "))
                    .map_err(|e| format!("line {}: vertex {e}", i + 1))?;
                verts.push(coords);
            }
            Some("f") => {
                let toks: Vec<&str> = it.collect();
                if toks.len() < 3 {
                    return Err(format!(
                        "line {}: face needs at least 3 vertices, got {}",
                        i + 1,
                        toks.len()
                    ));
                }
                let mut idx: Vec<usize> = Vec::with_capacity(toks.len());
                for tok in &toks {
                    let first = tok.split('/').next().unwrap_or("");
                    let n: i64 = first
                        .parse()
                        .map_err(|_| format!("line {}: bad face index '{tok}'", i + 1))?;
                    let resolved = if n > 0 {
                        (n - 1) as i64
                    } else if n < 0 {
                        verts.len() as i64 + n
                    } else {
                        return Err(format!("line {}: face index 0 is invalid (OBJ is 1-based)", i + 1));
                    };
                    if resolved < 0 || resolved as usize >= verts.len() {
                        return Err(format!(
                            "line {}: face index {n} out of range (1..={})",
                            i + 1,
                            verts.len()
                        ));
                    }
                    idx.push(resolved as usize);
                }
                // Fan triangulation of the polygon.
                for k in 1..idx.len() - 1 {
                    tris.push([verts[idx[0]], verts[idx[k]], verts[idx[k + 1]]]);
                }
            }
            _ => {}
        }
    }
    if tris.is_empty() {
        return Err("OBJ has no faces (f lines) to build triangles from".to_string());
    }
    Ok(tris)
}

// ---- emitters ----------------------------------------------------------------

/// Format a float compactly: whole numbers print without a decimal point,
/// otherwise up to 6 decimals with trailing zeros trimmed. `-0` normalizes to `0`.
fn fnum(x: f64) -> String {
    if x == 0.0 || !x.is_finite() {
        return "0".to_string();
    }
    let s = format!("{x:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn emit_obj(tris: &[Tri], name: &str) -> String {
    let mut out = String::new();
    out.push_str("# OBJ generated by gizza mesh-convert\n");
    out.push_str(&format!("o {name}\n"));
    // Dedupe vertices by their printed coordinate triple so shared corners merge.
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut vlines = String::new();
    let mut flines = String::new();
    for t in tris {
        let mut fi = [0usize; 3];
        for (k, v) in t.iter().enumerate() {
            let key = format!("{} {} {}", fnum(v[0]), fnum(v[1]), fnum(v[2]));
            let next_id = index.len();
            let id = *index.entry(key.clone()).or_insert_with(|| {
                vlines.push_str(&format!("v {key}\n"));
                next_id
            });
            fi[k] = id;
        }
        flines.push_str(&format!("f {} {} {}\n", fi[0] + 1, fi[1] + 1, fi[2] + 1));
    }
    out.push_str(&vlines);
    out.push_str(&flines);
    out
}

/// Per-face normal from the triangle winding; zero vector for degenerate faces.
fn face_normal(t: &Tri) -> [f64; 3] {
    let [a, b, c] = *t;
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len == 0.0 || !len.is_finite() {
        [0.0, 0.0, 0.0]
    } else {
        [n[0] / len, n[1] / len, n[2] / len]
    }
}

fn emit_stl_ascii(tris: &[Tri], name: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("solid {name}\n"));
    for t in tris {
        let n = face_normal(t);
        out.push_str(&format!(
            "  facet normal {} {} {}\n",
            fnum(n[0]),
            fnum(n[1]),
            fnum(n[2])
        ));
        out.push_str("    outer loop\n");
        for v in t {
            out.push_str(&format!(
                "      vertex {} {} {}\n",
                fnum(v[0]),
                fnum(v[1]),
                fnum(v[2])
            ));
        }
        out.push_str("    endloop\n");
        out.push_str("  endfacet\n");
    }
    out.push_str(&format!("endsolid {name}\n"));
    out
}

/// Encode a binary STL and wrap it in a `data:model/stl;base64,…` URL so the
/// page can offer it as a download and the CLI/chat can hand back a saveable blob.
fn emit_stl_binary_data_url(tris: &[Tri], name: &str) -> String {
    let mut buf: Vec<u8> = Vec::with_capacity(84 + tris.len() * 50);
    // 80-byte header (ASCII label, zero-padded).
    let header = format!("mesh-convert STL: {name}");
    let mut hbytes = [0u8; 80];
    for (i, b) in header.bytes().take(80).enumerate() {
        hbytes[i] = b;
    }
    buf.extend_from_slice(&hbytes);
    buf.extend_from_slice(&(tris.len() as u32).to_le_bytes());
    for t in tris {
        let n = face_normal(t);
        for c in n {
            buf.extend_from_slice(&(c as f32).to_le_bytes());
        }
        for v in t {
            for c in v {
                buf.extend_from_slice(&(*c as f32).to_le_bytes());
            }
        }
        buf.extend_from_slice(&0u16.to_le_bytes()); // attribute byte count
    }
    format!("data:model/stl;base64,{}", base64_encode(&buf))
}

/// Minimal standard base64 (no external dep in `core`).
fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRI_OBJ: &str = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";

    #[test]
    fn obj_to_stl_ascii_exact() {
        let opt = Options::default();
        let out = convert(TRI_OBJ, &opt).unwrap();
        let expected = "solid mesh\n  facet normal 0 0 1\n    outer loop\n      vertex 0 0 0\n      vertex 1 0 0\n      vertex 0 1 0\n    endloop\n  endfacet\nendsolid mesh\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn stl_ascii_to_obj_roundtrips_geometry() {
        let stl = convert(TRI_OBJ, &Options::default()).unwrap();
        let opt = Options { to: Target::Obj, ..Options::default() };
        let obj = convert(&stl, &opt).unwrap();
        assert!(obj.contains("v 0 0 0"));
        assert!(obj.contains("v 1 0 0"));
        assert!(obj.contains("v 0 1 0"));
        assert!(obj.contains("f 1 2 3"));
    }

    #[test]
    fn quad_face_is_fan_triangulated() {
        let obj = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n";
        let tris = parse(obj).unwrap();
        assert_eq!(tris.len(), 2);
    }

    #[test]
    fn negative_face_indices_resolve() {
        let obj = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3 -2 -1\n";
        let tris = parse(obj).unwrap();
        assert_eq!(tris.len(), 1);
        assert_eq!(tris[0][0], [0.0, 0.0, 0.0]);
        assert_eq!(tris[0][2], [0.0, 1.0, 0.0]);
    }

    #[test]
    fn scale_multiplies_vertices() {
        let opt = Options { scale: 2.0, ..Options::default() };
        let out = convert(TRI_OBJ, &opt).unwrap();
        assert!(out.contains("vertex 2 0 0"));
    }

    #[test]
    fn axis_y_up_to_z_up_rotates() {
        let opt = Options { to: Target::Obj, axis: Axis::YupToZup, ..Options::default() };
        let out = convert("v 0 1 0\nv 0 0 0\nv 1 0 0\nf 1 2 3\n", &opt).unwrap();
        // old +Y (0,1,0) -> new +Z (0,0,1)
        assert!(out.contains("v 0 0 1"));
    }

    #[test]
    fn binary_stl_is_data_url() {
        let opt = Options { stl_encoding: StlEncoding::Binary, ..Options::default() };
        let out = convert(TRI_OBJ, &opt).unwrap();
        assert!(out.starts_with("data:model/stl;base64,"));
        // header(80) + count(4) + 1 tri * 50 = 134 bytes -> base64 len 180.
        let b64 = out.strip_prefix("data:model/stl;base64,").unwrap();
        assert_eq!(b64.len(), 180);
    }

    #[test]
    fn empty_input_errors() {
        let err = convert("   \n  ", &Options::default()).unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn garbage_input_errors() {
        let err = convert("hello world, not a mesh", &Options::default()).unwrap_err();
        assert!(err.contains("unrecognized"));
    }

    #[test]
    fn obj_without_faces_errors() {
        let err = convert("v 0 0 0\nv 1 0 0\n", &Options::default()).unwrap_err();
        assert!(err.contains("no faces"));
    }

    const TRI_STL: &str = "solid mesh\n  facet normal 0 0 1\n    outer loop\n      vertex 0 0 0\n      vertex 1 0 0\n      vertex 0 1 0\n    endloop\n  endfacet\nendsolid mesh\n";

    #[test]
    fn stl_ascii_to_stl_binary_data_url() {
        // ASCII STL input, binary STL output: exercises the STL parse + binary emit path.
        let opt = Options { stl_encoding: StlEncoding::Binary, ..Options::default() };
        let out = convert(TRI_STL, &opt).unwrap();
        assert!(out.starts_with("data:model/stl;base64,"));
        assert_eq!(out.strip_prefix("data:model/stl;base64,").unwrap().len(), 180);
    }

    #[test]
    fn stl_ascii_passthrough_recomputes_normal() {
        // STL input with a bogus facet normal → normal is recomputed from geometry.
        let bad = "solid m\n facet normal 9 9 9\n  outer loop\n   vertex 0 0 0\n   vertex 1 0 0\n   vertex 0 1 0\n  endloop\n endfacet\nendsolid m\n";
        let out = convert(bad, &Options::default()).unwrap();
        assert!(out.contains("facet normal 0 0 1"));
    }

    #[test]
    fn stl_bad_vertex_count_errors() {
        let bad = "solid m\n facet normal 0 0 1\n  outer loop\n   vertex 0 0 0\n   vertex 1 0 0\n  endloop\n endfacet\nendsolid m\n";
        let err = convert(bad, &Options::default()).unwrap_err();
        assert!(err.contains("multiple of 3"), "got: {err}");
    }

    #[test]
    fn stl_non_numeric_vertex_errors() {
        let bad = "solid m\n facet normal 0 0 1\n  outer loop\n   vertex a b c\n   vertex 1 0 0\n   vertex 0 1 0\n  endloop\n endfacet\nendsolid m\n";
        let err = convert(bad, &Options::default()).unwrap_err();
        assert!(err.contains("not a number"), "got: {err}");
    }
}
