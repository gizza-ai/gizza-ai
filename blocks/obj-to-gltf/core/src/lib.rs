//! obj-to-gltf core — pure compute, shared by the chat skill block and the web page.
//! Converts a pasted Wavefront OBJ model (plus an optional pasted MTL library)
//! into a self-contained glTF 2.0 asset: either pretty-printed `.gltf` JSON with
//! the binary buffer embedded as a `data:application/octet-stream;base64,…` URI,
//! or a binary `.glb` returned as a `data:model/gltf-binary;base64,…` URL the
//! page turns into a download. No I/O, no network.
//!
//! Pipeline: parse OBJ (v/vt/vn/f/usemtl, negative indices, fan-triangulated
//! polygons) → apply scale + optional Z-up→Y-up rotation → group triangles by
//! material → deduplicate vertices per group → emit accessors/bufferViews into
//! one buffer → assemble the glTF JSON (materials come from the MTL text).

use std::collections::HashMap;
use std::fmt::Write as _;

/// Maximum combined OBJ+MTL paste size, in bytes. Keeps the wasm sandbox
/// (64 MiB) well clear of the buffer growth a huge paste would cause.
pub const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
/// Maximum triangles after fan triangulation.
pub const MAX_TRIANGLES: usize = 200_000;

/// Output container.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    /// `.gltf` JSON with the buffer embedded as a base64 data URI.
    Gltf,
    /// `.glb` binary, returned as a base64 data URL.
    Glb,
}

impl Target {
    pub fn parse(s: &str) -> Result<Target, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "gltf" | "" => Ok(Target::Gltf),
            "glb" => Ok(Target::Glb),
            other => Err(format!(
                "unknown output format '{other}': expected 'gltf' or 'glb'"
            )),
        }
    }
}

/// Which axis points up in the SOURCE OBJ. glTF is always Y-up, so `Z` means
/// "rotate the model into glTF's frame".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UpAxis {
    Y,
    Z,
}

impl UpAxis {
    pub fn parse(s: &str) -> Result<UpAxis, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "y" | "y-up" | "" => Ok(UpAxis::Y),
            "z" | "z-up" => Ok(UpAxis::Z),
            other => Err(format!(
                "unknown up axis '{other}': expected 'y' (glTF-native) or 'z' (CAD/3D-print source)"
            )),
        }
    }

    /// Rotate -90° about X so the source +Z becomes glTF +Y. Determinant is +1,
    /// so triangle winding (and therefore face orientation) is preserved.
    fn apply(self, v: [f64; 3]) -> [f64; 3] {
        let [x, y, z] = v;
        match self {
            UpAxis::Y => [x, y, z],
            UpAxis::Z => [x, z, -y],
        }
    }
}

/// How the `NORMAL` attribute is produced.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Normals {
    /// Use the OBJ's own `vn` values where a face references them, otherwise
    /// compute a flat per-face normal.
    Auto,
    /// Ignore `vn` and always compute a flat per-face normal.
    Flat,
    /// Emit no `NORMAL` attribute; viewers shade the mesh flat themselves.
    None,
}

impl Normals {
    pub fn parse(s: &str) -> Result<Normals, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "" => Ok(Normals::Auto),
            "flat" => Ok(Normals::Flat),
            "none" => Ok(Normals::None),
            other => Err(format!(
                "unknown normals mode '{other}': expected 'auto', 'flat', or 'none'"
            )),
        }
    }
}

/// Conversion options (mirrors the descriptor params).
#[derive(Clone, Debug)]
pub struct Options {
    pub mtl: String,
    pub to: Target,
    pub up_axis: UpAxis,
    pub scale: f64,
    pub normals: Normals,
    pub name: String,
    pub unlit: bool,
    pub double_sided: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mtl: String::new(),
            to: Target::Gltf,
            up_axis: UpAxis::Y,
            scale: 1.0,
            normals: Normals::Auto,
            name: "model".to_string(),
            unlit: false,
            double_sided: false,
        }
    }
}

/// One OBJ face corner: vertex / texcoord / normal indices, already resolved to
/// 0-based positions into the parsed arrays.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Corner {
    v: usize,
    vt: Option<usize>,
    vn: Option<usize>,
}

/// A triangle plus the material it was assigned by the preceding `usemtl`.
#[derive(Clone, Debug)]
struct Triangle {
    corners: [Corner; 3],
    material: Option<usize>,
}

/// One material parsed out of the MTL text.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub name: String,
    /// Diffuse colour `Kd` (linear RGB, 0..1).
    pub base_color: [f64; 3],
    /// Dissolve `d` (or `1 - Tr`).
    pub alpha: f64,
    /// Emissive colour `Ke`.
    pub emissive: [f64; 3],
    /// Derived from the specular exponent `Ns`.
    pub roughness: f64,
    /// True when `map_Kd`/`map_*` referenced an image file we cannot embed.
    pub had_texture_ref: bool,
}

impl Default for Material {
    fn default() -> Self {
        Material {
            name: "default".to_string(),
            base_color: [1.0, 1.0, 1.0],
            alpha: 1.0,
            emissive: [0.0, 0.0, 0.0],
            roughness: 1.0,
            had_texture_ref: false,
        }
    }
}

/// Convert pasted OBJ text into a glTF 2.0 asset.
///
/// Returns pretty-printed glTF JSON for [`Target::Gltf`], or a
/// `data:model/gltf-binary;base64,…` URL for [`Target::Glb`].
pub fn convert(obj: &str, opt: &Options) -> Result<String, String> {
    if !opt.scale.is_finite() || opt.scale == 0.0 {
        return Err(format!(
            "scale must be a non-zero finite number, got '{}'",
            opt.scale
        ));
    }
    let total = obj.len() + opt.mtl.len();
    if total > MAX_INPUT_BYTES {
        return Err(format!(
            "pasted model is {:.1} MB, over the {} MB limit for this tool — convert large models with a desktop exporter",
            total as f64 / (1024.0 * 1024.0),
            MAX_INPUT_BYTES / (1024 * 1024)
        ));
    }
    if obj.trim().is_empty() {
        return Err(
            "no OBJ text supplied — paste a Wavefront OBJ model (lines like 'v 0 0 0' and 'f 1 2 3')"
                .to_string(),
        );
    }

    let materials = parse_mtl(&opt.mtl)?;
    let parsed = parse_obj(obj, &materials)?;

    build_gltf(&parsed, &materials, opt)
}

fn defaulted(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Flat entry point shared by chat, CLI and the browser wrapper.
#[allow(clippy::too_many_arguments)]
pub fn run(
    obj: &str,
    mtl: &str,
    to: &str,
    up_axis: &str,
    scale: f64,
    normals: &str,
    name: &str,
    unlit: bool,
    double_sided: bool,
) -> Result<String, String> {
    let opt = Options {
        mtl: mtl.to_string(),
        to: Target::parse(to)?,
        up_axis: UpAxis::parse(up_axis)?,
        scale,
        normals: Normals::parse(normals)?,
        name: defaulted(name, "model"),
        unlit,
        double_sided,
    };
    convert(obj, &opt)
}

/// Everything the OBJ parser hands to the glTF writer.
struct ParsedObj {
    positions: Vec<[f64; 3]>,
    texcoords: Vec<[f64; 2]>,
    normals: Vec<[f64; 3]>,
    triangles: Vec<Triangle>,
}

/// Parse Wavefront OBJ text. Unknown statements (`mtllib`, `s`, `o`, `g`,
/// curves, …) are skipped; `usemtl` selects the material for later faces.
fn parse_obj(src: &str, materials: &[Material]) -> Result<ParsedObj, String> {
    let mut positions: Vec<[f64; 3]> = Vec::new();
    let mut texcoords: Vec<[f64; 2]> = Vec::new();
    let mut normals: Vec<[f64; 3]> = Vec::new();
    let mut triangles: Vec<Triangle> = Vec::new();
    let mut current_material: Option<usize> = None;

    for (idx, raw) in src.lines().enumerate() {
        let lineno = idx + 1;
        // Strip comments, then split on any whitespace.
        let line = match raw.find('#') {
            Some(p) => &raw[..p],
            None => raw,
        };
        let mut parts = line.split_whitespace();
        let Some(kw) = parts.next() else { continue };
        match kw {
            "v" => positions.push(parse_vec3(&mut parts, lineno, "v")?),
            "vt" => {
                let u = parse_f64(parts.next(), lineno, "vt")?;
                // A missing V (1-D texture) is legal OBJ; treat it as 0.
                let v = match parts.next() {
                    Some(s) => parse_f64(Some(s), lineno, "vt")?,
                    None => 0.0,
                };
                texcoords.push([u, v]);
            }
            "vn" => normals.push(parse_vec3(&mut parts, lineno, "vn")?),
            "usemtl" => {
                let name = parts.next().unwrap_or("").trim();
                current_material = materials.iter().position(|m| m.name == name);
                if current_material.is_none() && !materials.is_empty() && !name.is_empty() {
                    // Unknown name: fall back to the default material rather
                    // than failing — OBJ files often reference an MTL the user
                    // did not paste.
                    current_material = None;
                }
            }
            "f" => {
                let corners: Vec<Corner> = parts
                    .map(|tok| {
                        parse_corner(tok, lineno, positions.len(), texcoords.len(), normals.len())
                    })
                    .collect::<Result<_, _>>()?;
                if corners.len() < 3 {
                    return Err(format!(
                        "line {lineno}: face 'f' needs at least 3 vertices, got {}",
                        corners.len()
                    ));
                }
                // Fan-triangulate: (0,1,2), (0,2,3), …
                for i in 1..corners.len() - 1 {
                    if triangles.len() >= MAX_TRIANGLES {
                        return Err(format!(
                            "model exceeds the {MAX_TRIANGLES} triangle limit for this tool — decimate it or convert it with a desktop exporter"
                        ));
                    }
                    triangles.push(Triangle {
                        corners: [corners[0], corners[i], corners[i + 1]],
                        material: current_material,
                    });
                }
            }
            _ => {}
        }
    }

    if triangles.is_empty() {
        return Err(format!(
            "OBJ has {} vertices but no faces — glTF needs triangles, so the model must contain 'f' lines",
            positions.len()
        ));
    }
    Ok(ParsedObj {
        positions,
        texcoords,
        normals,
        triangles,
    })
}

fn parse_vec3<'a>(
    parts: &mut impl Iterator<Item = &'a str>,
    lineno: usize,
    kw: &str,
) -> Result<[f64; 3], String> {
    let x = parse_f64(parts.next(), lineno, kw)?;
    let y = parse_f64(parts.next(), lineno, kw)?;
    let z = parse_f64(parts.next(), lineno, kw)?;
    Ok([x, y, z])
}

fn parse_f64(tok: Option<&str>, lineno: usize, kw: &str) -> Result<f64, String> {
    let t = tok.ok_or_else(|| {
        format!("line {lineno}: '{kw}' needs 3 numbers (e.g. '{kw} 0 1 0'), found fewer")
    })?;
    t.parse::<f64>()
        .map_err(|_| format!("line {lineno}: '{kw}' expected a number, got '{t}'"))
}

/// Parse one `f` token: `v`, `v/vt`, `v//vn`, or `v/vt/vn`. OBJ indices are
/// 1-based; negative indices count backwards from the most recent element.
fn parse_corner(
    tok: &str,
    lineno: usize,
    nv: usize,
    nvt: usize,
    nvn: usize,
) -> Result<Corner, String> {
    let mut it = tok.split('/');
    let v_tok = it.next().unwrap_or("");
    let vt_tok = it.next().unwrap_or("");
    let vn_tok = it.next().unwrap_or("");

    let v = resolve_index(v_tok, nv, lineno, "vertex", tok)?
        .ok_or_else(|| format!("line {lineno}: face corner '{tok}' has no vertex index"))?;
    let vt = resolve_index(vt_tok, nvt, lineno, "texture coordinate", tok)?;
    let vn = resolve_index(vn_tok, nvn, lineno, "normal", tok)?;
    Ok(Corner { v, vt, vn })
}

fn resolve_index(
    tok: &str,
    count: usize,
    lineno: usize,
    what: &str,
    whole: &str,
) -> Result<Option<usize>, String> {
    if tok.is_empty() {
        return Ok(None);
    }
    let n: i64 = tok.parse().map_err(|_| {
        format!("line {lineno}: face corner '{whole}' has a non-numeric {what} index '{tok}'")
    })?;
    if n == 0 {
        return Err(format!(
            "line {lineno}: face corner '{whole}' uses {what} index 0, but OBJ indices start at 1"
        ));
    }
    let zero_based = if n < 0 {
        // Relative: -1 is the most recently declared element.
        let back = (-n) as usize;
        if back > count {
            return Err(format!(
                "line {lineno}: face corner '{whole}' refers back {back} {what}s but only {count} have been declared"
            ));
        }
        count - back
    } else {
        let i = n as usize;
        if i > count {
            return Err(format!(
                "line {lineno}: face corner '{whole}' uses {what} index {i}, but the OBJ declares only {count}"
            ));
        }
        i - 1
    };
    Ok(Some(zero_based))
}

/// Parse an MTL library. Only the statements glTF's metallic-roughness model can
/// represent are read; `map_*` lines are noted so the caller can say the texture
/// image was dropped.
pub fn parse_mtl(src: &str) -> Result<Vec<Material>, String> {
    let mut out: Vec<Material> = Vec::new();
    for (idx, raw) in src.lines().enumerate() {
        let lineno = idx + 1;
        let line = match raw.find('#') {
            Some(p) => &raw[..p],
            None => raw,
        };
        let mut parts = line.split_whitespace();
        let Some(kw) = parts.next() else { continue };
        if kw == "newmtl" {
            let name = parts.next().unwrap_or("material").to_string();
            out.push(Material {
                name,
                ..Material::default()
            });
            continue;
        }
        let Some(m) = out.last_mut() else {
            // Statements before the first `newmtl` are not attached to any
            // material; OBJ exporters do not emit these, so just skip them.
            continue;
        };
        match kw {
            "Kd" => m.base_color = parse_color(&mut parts, lineno, "Kd")?,
            "Ke" => m.emissive = parse_color(&mut parts, lineno, "Ke")?,
            "d" => m.alpha = clamp01(parse_f64(parts.next(), lineno, "d")?),
            "Tr" => m.alpha = clamp01(1.0 - parse_f64(parts.next(), lineno, "Tr")?),
            "Ns" => {
                // Blinn-Phong exponent → glTF roughness. The usual mapping:
                // roughness = sqrt(2 / (Ns + 2)); Ns=0 → 1.0 (fully rough).
                let ns = parse_f64(parts.next(), lineno, "Ns")?.max(0.0);
                m.roughness = clamp01((2.0 / (ns + 2.0)).sqrt());
            }
            _ if kw.starts_with("map_") || kw == "bump" || kw == "refl" => {
                m.had_texture_ref = true;
            }
            _ => {}
        }
    }
    Ok(out)
}

fn parse_color<'a>(
    parts: &mut impl Iterator<Item = &'a str>,
    lineno: usize,
    kw: &str,
) -> Result<[f64; 3], String> {
    let r = parse_f64(parts.next(), lineno, kw)?;
    // `Kd 0.8` (greyscale) is legal MTL.
    let g = match parts.next() {
        Some(s) => parse_f64(Some(s), lineno, kw)?,
        None => r,
    };
    let b = match parts.next() {
        Some(s) => parse_f64(Some(s), lineno, kw)?,
        None => r,
    };
    Ok([clamp01(r), clamp01(g), clamp01(b)])
}

fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// A deduplicated vertex key: position index + texcoord index + quantized normal.
#[derive(PartialEq, Eq, Hash)]
struct VertexKey {
    v: usize,
    vt: Option<usize>,
    n: Option<[u32; 3]>,
}

/// One material group's interleave-free vertex arrays plus its index list.
struct Primitive {
    material: Option<usize>,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    texcoords: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

/// Build the glTF document (JSON + binary buffer) from parsed geometry.
fn build_gltf(parsed: &ParsedObj, materials: &[Material], opt: &Options) -> Result<String, String> {
    // Transform each source position once: scale, then reorient to Y-up.
    let xf: Vec<[f64; 3]> = parsed
        .positions
        .iter()
        .map(|p| {
            opt.up_axis
                .apply([p[0] * opt.scale, p[1] * opt.scale, p[2] * opt.scale])
        })
        .collect();
    // Normals are rotated but never scaled (uniform scale keeps them unit).
    let xn: Vec<[f64; 3]> = parsed
        .normals
        .iter()
        .map(|n| normalize(opt.up_axis.apply(*n)))
        .collect();
    // A negative scale mirrors the model, which flips the effective winding;
    // the normals must be flipped with it so lighting stays consistent.
    let flip = opt.scale < 0.0;

    // Group triangles by material, preserving first-use order.
    let mut order: Vec<Option<usize>> = Vec::new();
    let mut groups: HashMap<Option<usize>, Vec<&Triangle>> = HashMap::new();
    for t in &parsed.triangles {
        if !groups.contains_key(&t.material) {
            order.push(t.material);
        }
        groups.entry(t.material).or_default().push(t);
    }

    let mut prims: Vec<Primitive> = Vec::new();
    for mat in &order {
        let tris = &groups[mat];
        let mut prim = Primitive {
            material: *mat,
            positions: Vec::new(),
            normals: Vec::new(),
            texcoords: Vec::new(),
            indices: Vec::new(),
        };
        let mut seen: HashMap<VertexKey, u32> = HashMap::new();
        let has_uv = tris
            .iter()
            .any(|t| t.corners.iter().any(|c| c.vt.is_some()));
        for t in tris.iter() {
            // Flat normal for this face, from the already-transformed positions.
            let face_n = face_normal(
                xf[t.corners[0].v],
                xf[t.corners[1].v],
                xf[t.corners[2].v],
                flip,
            );
            for c in &t.corners {
                let n: Option<[f64; 3]> = match opt.normals {
                    Normals::None => None,
                    Normals::Flat => Some(face_n),
                    Normals::Auto => Some(match c.vn {
                        Some(i) => {
                            let v = xn[i];
                            if flip {
                                [-v[0], -v[1], -v[2]]
                            } else {
                                v
                            }
                        }
                        None => face_n,
                    }),
                };
                let key = VertexKey {
                    v: c.v,
                    vt: if has_uv { c.vt } else { None },
                    n: n.map(|v| {
                        [
                            (v[0] as f32).to_bits(),
                            (v[1] as f32).to_bits(),
                            (v[2] as f32).to_bits(),
                        ]
                    }),
                };
                let next = prim.positions.len() as u32;
                let index = *seen.entry(key).or_insert_with(|| {
                    let p = xf[c.v];
                    prim.positions.push([p[0] as f32, p[1] as f32, p[2] as f32]);
                    if let Some(v) = n {
                        prim.normals.push([v[0] as f32, v[1] as f32, v[2] as f32]);
                    }
                    if has_uv {
                        let uv = c.vt.map(|i| parsed.texcoords[i]).unwrap_or([0.0, 0.0]);
                        // OBJ V runs bottom-up, glTF V runs top-down.
                        prim.texcoords.push([uv[0] as f32, (1.0 - uv[1]) as f32]);
                    }
                    next
                });
                prim.indices.push(index);
            }
        }
        prims.push(prim);
    }

    // ---- Serialize the binary buffer + accessors/bufferViews ----
    let mut bin: Vec<u8> = Vec::new();
    let mut buffer_views = String::new();
    let mut accessors = String::new();
    let mut primitives_json: Vec<String> = Vec::new();
    let mut nviews = 0usize;
    let mut naccessors = 0usize;

    for prim in &prims {
        let mut attrs: Vec<String> = Vec::new();

        // POSITION (with the spec-required min/max bounds).
        let (min, max) = bounds(&prim.positions);
        let off = push_pad(&mut bin);
        for p in &prim.positions {
            for c in p {
                bin.extend_from_slice(&c.to_le_bytes());
            }
        }
        push_view(&mut buffer_views, &mut nviews, off, bin.len() - off, 34962);
        push_accessor(
            &mut accessors,
            &mut naccessors,
            nviews - 1,
            5126,
            prim.positions.len(),
            "VEC3",
            Some((
                format!("[{},{},{}]", num(min[0]), num(min[1]), num(min[2])),
                format!("[{},{},{}]", num(max[0]), num(max[1]), num(max[2])),
            )),
        );
        attrs.push(format!("\"POSITION\": {}", naccessors - 1));

        if !prim.normals.is_empty() {
            let off = push_pad(&mut bin);
            for p in &prim.normals {
                for c in p {
                    bin.extend_from_slice(&c.to_le_bytes());
                }
            }
            push_view(&mut buffer_views, &mut nviews, off, bin.len() - off, 34962);
            push_accessor(
                &mut accessors,
                &mut naccessors,
                nviews - 1,
                5126,
                prim.normals.len(),
                "VEC3",
                None,
            );
            attrs.push(format!("\"NORMAL\": {}", naccessors - 1));
        }

        if !prim.texcoords.is_empty() {
            let off = push_pad(&mut bin);
            for p in &prim.texcoords {
                for c in p {
                    bin.extend_from_slice(&c.to_le_bytes());
                }
            }
            push_view(&mut buffer_views, &mut nviews, off, bin.len() - off, 34962);
            push_accessor(
                &mut accessors,
                &mut naccessors,
                nviews - 1,
                5126,
                prim.texcoords.len(),
                "VEC2",
                None,
            );
            attrs.push(format!("\"TEXCOORD_0\": {}", naccessors - 1));
        }

        // Indices (unsigned int — always valid, no 65 535 vertex ceiling).
        let off = push_pad(&mut bin);
        for i in &prim.indices {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        push_view(&mut buffer_views, &mut nviews, off, bin.len() - off, 34963);
        push_accessor(
            &mut accessors,
            &mut naccessors,
            nviews - 1,
            5125,
            prim.indices.len(),
            "SCALAR",
            None,
        );
        let idx_accessor = naccessors - 1;

        let mut p = format!(
            "{{\n        \"attributes\": {{ {} }},\n        \"indices\": {idx_accessor},\n        \"mode\": 4",
            attrs.join(", ")
        );
        if let Some(m) = prim.material {
            let _ = write!(p, ",\n        \"material\": {m}");
        } else if !materials.is_empty() {
            // Faces before any `usemtl` get the implicit default material,
            // which is appended after the MTL ones.
            let _ = write!(p, ",\n        \"material\": {}", materials.len());
        }
        p.push_str("\n      }");
        primitives_json.push(p);
    }

    // A model with no `usemtl` still gets one material so viewers show the
    // configured unlit/double-sided flags rather than their own fallback.
    let mut mats: Vec<Material> = materials.to_vec();
    let needs_default = prims.iter().any(|p| p.material.is_none());
    if needs_default || mats.is_empty() {
        mats.push(Material::default());
    }

    let materials_json: Vec<String> = mats.iter().map(|m| material_json(m, opt)).collect();

    let name = json_string(if opt.name.trim().is_empty() {
        "model"
    } else {
        opt.name.trim()
    });

    let extensions = if opt.unlit {
        "\n  \"extensionsUsed\": [\"KHR_materials_unlit\"],"
    } else {
        ""
    };

    let (buffer_uri, bin_for_glb) = match opt.to {
        Target::Gltf => (
            Some(format!(
                "data:application/octet-stream;base64,{}",
                base64_encode(&bin)
            )),
            None,
        ),
        Target::Glb => (None, Some(bin.clone())),
    };

    let mut buffer_json = format!("{{ \"byteLength\": {}", bin.len());
    if let Some(uri) = &buffer_uri {
        let _ = write!(buffer_json, ", \"uri\": {}", json_string(uri));
    }
    buffer_json.push_str(" }");

    let json = format!(
        "{{\n  \"asset\": {{ \"version\": \"2.0\", \"generator\": \"gizza-ai/obj-to-gltf\" }},{extensions}\n  \"scene\": 0,\n  \"scenes\": [\n    {{ \"nodes\": [0], \"name\": {name} }}\n  ],\n  \"nodes\": [\n    {{ \"mesh\": 0, \"name\": {name} }}\n  ],\n  \"meshes\": [\n    {{\n      \"name\": {name},\n      \"primitives\": [\n      {}\n      ]\n    }}\n  ],\n  \"materials\": [\n    {}\n  ],\n  \"accessors\": [\n    {}\n  ],\n  \"bufferViews\": [\n    {}\n  ],\n  \"buffers\": [\n    {buffer_json}\n  ]\n}}\n",
        primitives_json.join(",\n      "),
        materials_json.join(",\n    "),
        accessors,
        buffer_views,
    );

    match bin_for_glb {
        None => Ok(json),
        Some(bin) => Ok(format!(
            "data:model/gltf-binary;base64,{}",
            base64_encode(&glb_container(&json, &bin))
        )),
    }
}

/// Wrap glTF JSON + binary payload in the 12-byte GLB header + two chunks.
/// Both chunks are padded to a 4-byte boundary (JSON with spaces, BIN with 0s).
fn glb_container(json: &str, bin: &[u8]) -> Vec<u8> {
    let mut json_chunk = json.as_bytes().to_vec();
    while json_chunk.len() % 4 != 0 {
        json_chunk.push(b' ');
    }
    let mut bin_chunk = bin.to_vec();
    while bin_chunk.len() % 4 != 0 {
        bin_chunk.push(0);
    }
    let total = 12
        + 8
        + json_chunk.len()
        + if bin_chunk.is_empty() {
            0
        } else {
            8 + bin_chunk.len()
        };
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_chunk);
    if !bin_chunk.is_empty() {
        out.extend_from_slice(&(bin_chunk.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin_chunk);
    }
    out
}

fn material_json(m: &Material, opt: &Options) -> String {
    let mut s = format!(
        "{{\n      \"name\": {},\n      \"pbrMetallicRoughness\": {{\n        \"baseColorFactor\": [{}, {}, {}, {}],\n        \"metallicFactor\": 0,\n        \"roughnessFactor\": {}\n      }}",
        json_string(&m.name),
        num(m.base_color[0]),
        num(m.base_color[1]),
        num(m.base_color[2]),
        num(m.alpha),
        num(m.roughness),
    );
    if m.emissive != [0.0, 0.0, 0.0] {
        let _ = write!(
            s,
            ",\n      \"emissiveFactor\": [{}, {}, {}]",
            num(m.emissive[0]),
            num(m.emissive[1]),
            num(m.emissive[2])
        );
    }
    if m.alpha < 1.0 {
        s.push_str(",\n      \"alphaMode\": \"BLEND\"");
    }
    if opt.double_sided {
        s.push_str(",\n      \"doubleSided\": true");
    }
    if opt.unlit {
        s.push_str(",\n      \"extensions\": { \"KHR_materials_unlit\": {} }");
    }
    s.push_str("\n    }");
    s
}

/// Pad the buffer to a 4-byte boundary and return the next write offset.
fn push_pad(bin: &mut Vec<u8>) -> usize {
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    bin.len()
}

fn push_view(out: &mut String, n: &mut usize, offset: usize, len: usize, target: u32) {
    if *n > 0 {
        out.push_str(",\n    ");
    }
    let _ = write!(
        out,
        "{{ \"buffer\": 0, \"byteOffset\": {offset}, \"byteLength\": {len}, \"target\": {target} }}"
    );
    *n += 1;
}

fn push_accessor(
    out: &mut String,
    n: &mut usize,
    view: usize,
    component_type: u32,
    count: usize,
    ty: &str,
    minmax: Option<(String, String)>,
) {
    if *n > 0 {
        out.push_str(",\n    ");
    }
    let _ = write!(
        out,
        "{{ \"bufferView\": {view}, \"componentType\": {component_type}, \"count\": {count}, \"type\": \"{ty}\""
    );
    if let Some((min, max)) = minmax {
        let _ = write!(out, ", \"min\": {min}, \"max\": {max}");
    }
    out.push_str(" }");
    *n += 1;
}

fn bounds(pts: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for p in pts {
        for i in 0..3 {
            if p[i] < min[i] {
                min[i] = p[i];
            }
            if p[i] > max[i] {
                max[i] = p[i];
            }
        }
    }
    if pts.is_empty() {
        return ([0.0; 3], [0.0; 3]);
    }
    (min, max)
}

fn face_normal(a: [f64; 3], b: [f64; 3], c: [f64; 3], flip: bool) -> [f64; 3] {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let n = normalize(n);
    if flip {
        [-n[0], -n[1], -n[2]]
    } else {
        n
    }
}

fn normalize(n: [f64; 3]) -> [f64; 3] {
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len > 0.0 {
        [n[0] / len, n[1] / len, n[2] / len]
    } else {
        [0.0, 0.0, 0.0]
    }
}

/// Render an f64/f32 as compact JSON: integral values without a trailing `.0`,
/// everything else rounded to 6 decimals with trailing zeros trimmed.
fn num(v: impl Into<f64>) -> String {
    let v: f64 = v.into();
    if !v.is_finite() {
        return "0".to_string();
    }
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let mut s = format!("{v:.6}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

/// Minimal JSON string escaping (no external dep in `core`).
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
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
        out.push(if chunk.len() > 1 {
            T[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRI_OBJ: &str = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";

    fn base64_decode(s: &str) -> Vec<u8> {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let vals: Vec<u8> = s
            .bytes()
            .filter(|b| *b != b'=')
            .map(|b| T.iter().position(|c| *c == b).expect("base64 alphabet") as u8)
            .collect();
        let mut out = Vec::new();
        for chunk in vals.chunks(4) {
            let mut n = 0u32;
            for (i, v) in chunk.iter().enumerate() {
                n |= (*v as u32) << (18 - 6 * i);
            }
            out.push((n >> 16) as u8);
            if chunk.len() > 2 {
                out.push((n >> 8) as u8);
            }
            if chunk.len() > 3 {
                out.push(n as u8);
            }
        }
        out
    }

    /// f32 little-endian floats out of the embedded buffer.
    fn floats(bytes: &[u8]) -> Vec<f32> {
        bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    #[test]
    fn triangle_to_gltf_happy_path() {
        let out = convert(TRI_OBJ, &Options::default()).unwrap();
        assert!(out.contains("\"version\": \"2.0\""));
        assert!(out.contains("\"generator\": \"gizza-ai/obj-to-gltf\""));
        // One primitive, POSITION + NORMAL, unsigned-int indices, triangles.
        assert!(out.contains("\"POSITION\": 0"));
        assert!(out.contains("\"NORMAL\": 1"));
        assert!(out.contains("\"mode\": 4"));
        assert!(out.contains("\"componentType\": 5125"));
        // POSITION bounds are mandatory in glTF 2.0.
        assert!(out.contains("\"min\": [0,0,0], \"max\": [1,1,0]"));
        assert!(out.contains("data:application/octet-stream;base64,"));
        // 3 positions + 3 normals (VEC3 f32) + 3 u32 indices = 84 bytes.
        assert!(out.contains("\"byteLength\": 84"));
    }

    #[test]
    fn buffer_holds_the_actual_vertex_data() {
        let out = convert(TRI_OBJ, &Options::default()).unwrap();
        let start = out.find("base64,").unwrap() + "base64,".len();
        let end = out[start..].find('"').unwrap() + start;
        let bin = base64_decode(&out[start..end]);
        assert_eq!(bin.len(), 84);
        // Positions come first, in declaration order.
        assert_eq!(
            floats(&bin[0..36]),
            vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]
        );
        // The face is CCW in the XY plane → +Z normal for all three vertices.
        assert_eq!(
            floats(&bin[36..72]),
            vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]
        );
        let idx: Vec<u32> = bin[72..]
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(idx, vec![0, 1, 2]);
    }

    #[test]
    fn glb_output_is_a_valid_container() {
        let opt = Options {
            to: Target::Glb,
            ..Options::default()
        };
        let out = convert(TRI_OBJ, &opt).unwrap();
        let b64 = out
            .strip_prefix("data:model/gltf-binary;base64,")
            .expect("glb data URL prefix");
        let glb = base64_decode(b64);
        assert_eq!(&glb[0..4], b"glTF");
        assert_eq!(u32::from_le_bytes([glb[4], glb[5], glb[6], glb[7]]), 2);
        // Header length field must equal the real byte length.
        assert_eq!(
            u32::from_le_bytes([glb[8], glb[9], glb[10], glb[11]]) as usize,
            glb.len()
        );
        let json_len = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
        assert_eq!(&glb[16..20], b"JSON");
        assert_eq!(json_len % 4, 0);
        let json = std::str::from_utf8(&glb[20..20 + json_len]).unwrap();
        // The GLB buffer has no uri — it points at the BIN chunk.
        assert!(json.contains("\"byteLength\": 84"));
        assert!(!json.contains("\"uri\""));
        let bin_off = 20 + json_len;
        assert_eq!(&glb[bin_off + 4..bin_off + 8], b"BIN\0");
        assert_eq!(
            u32::from_le_bytes([
                glb[bin_off],
                glb[bin_off + 1],
                glb[bin_off + 2],
                glb[bin_off + 3]
            ]) as usize,
            84
        );
    }

    #[test]
    fn quads_are_fan_triangulated_and_vertices_deduplicated() {
        let quad = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n";
        let out = convert(quad, &Options::default()).unwrap();
        // Two triangles → 6 indices; both share the same flat normal, so the
        // 4 corners dedupe back to 4 vertices.
        assert!(out.contains("\"count\": 6, \"type\": \"SCALAR\""));
        assert!(out.contains("\"count\": 4, \"type\": \"VEC3\""));
    }

    #[test]
    fn negative_indices_refer_backwards() {
        let rel = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3 -2 -1\n";
        let out = convert(rel, &Options::default()).unwrap();
        assert!(out.contains("\"min\": [0,0,0], \"max\": [1,1,0]"));
        assert!(out.contains("\"count\": 3, \"type\": \"VEC3\""));
    }

    #[test]
    fn uvs_are_emitted_with_a_flipped_v() {
        let obj = "v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nf 1/1 2/2 3/3\n";
        let out = convert(obj, &Options::default()).unwrap();
        assert!(out.contains("\"TEXCOORD_0\""));
        let start = out.find("base64,").unwrap() + "base64,".len();
        let end = out[start..].find('"').unwrap() + start;
        let bin = base64_decode(&out[start..end]);
        // positions(36) + normals(36) then 3 VEC2 texcoords.
        assert_eq!(floats(&bin[72..96]), vec![0.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn z_up_source_is_rotated_into_gltf_y_up() {
        let obj = "v 0 0 0\nv 1 0 0\nv 0 0 1\nf 1 2 3\n";
        let opt = Options {
            up_axis: UpAxis::Z,
            ..Options::default()
        };
        let out = convert(obj, &opt).unwrap();
        // Source (0,0,1) [up in Z] becomes (0,1,0) [up in Y].
        assert!(out.contains("\"min\": [0,0,0], \"max\": [1,1,0]"));
    }

    #[test]
    fn scale_multiplies_every_vertex() {
        let opt = Options {
            scale: 1000.0,
            ..Options::default()
        };
        let out = convert(TRI_OBJ, &opt).unwrap();
        assert!(out.contains("\"min\": [0,0,0], \"max\": [1000,1000,0]"));
    }

    #[test]
    fn mtl_materials_become_pbr_materials_and_split_primitives() {
        let obj =
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nv 1 1 0\nusemtl red\nf 1 2 3\nusemtl glass\nf 2 4 3\n";
        let mtl = "newmtl red\nKd 1 0 0\nNs 0\nnewmtl glass\nKd 0.2 0.4 0.9\nd 0.5\nKe 0 0 0.1\n";
        let opt = Options {
            mtl: mtl.to_string(),
            ..Options::default()
        };
        let out = convert(obj, &opt).unwrap();
        assert!(out.contains("\"name\": \"red\""));
        assert!(out.contains("\"baseColorFactor\": [1, 0, 0, 1]"));
        assert!(out.contains("\"baseColorFactor\": [0.2, 0.4, 0.9, 0.5]"));
        assert!(out.contains("\"alphaMode\": \"BLEND\""));
        assert!(out.contains("\"emissiveFactor\": [0, 0, 0.1]"));
        // Two usemtl groups → two primitives referencing materials 0 and 1.
        assert!(out.contains("\"material\": 0"));
        assert!(out.contains("\"material\": 1"));
    }

    #[test]
    fn unlit_and_double_sided_flags_reach_the_material() {
        let opt = Options {
            unlit: true,
            double_sided: true,
            ..Options::default()
        };
        let out = convert(TRI_OBJ, &opt).unwrap();
        assert!(out.contains("\"extensionsUsed\": [\"KHR_materials_unlit\"]"));
        assert!(out.contains("\"KHR_materials_unlit\": {}"));
        assert!(out.contains("\"doubleSided\": true"));
    }

    #[test]
    fn normals_none_omits_the_attribute() {
        let opt = Options {
            normals: Normals::None,
            ..Options::default()
        };
        let out = convert(TRI_OBJ, &opt).unwrap();
        assert!(!out.contains("\"NORMAL\""));
        // positions(36) + indices(12) only.
        assert!(out.contains("\"byteLength\": 48"));
    }

    #[test]
    fn normals_flat_overrides_the_objs_own_vn() {
        let obj = "v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 0 1 0\nf 1//1 2//1 3//1\n";
        let auto = convert(obj, &Options::default()).unwrap();
        let flat = convert(
            obj,
            &Options {
                normals: Normals::Flat,
                ..Options::default()
            },
        )
        .unwrap();
        let grab = |s: &str| {
            let start = s.find("base64,").unwrap() + "base64,".len();
            let end = s[start..].find('"').unwrap() + start;
            base64_decode(&s[start..end])
        };
        // auto keeps the authored +Y normal; flat recomputes +Z from winding.
        assert_eq!(floats(&grab(&auto)[36..48]), vec![0.0, 1.0, 0.0]);
        assert_eq!(floats(&grab(&flat)[36..48]), vec![0.0, 0.0, 1.0]);
    }

    #[test]
    fn custom_name_lands_on_scene_node_and_mesh() {
        let opt = Options {
            name: "teapot".to_string(),
            ..Options::default()
        };
        let out = convert(TRI_OBJ, &opt).unwrap();
        assert_eq!(out.matches("\"name\": \"teapot\"").count(), 3);
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = convert("   \n\n", &Options::default()).unwrap_err();
        assert!(err.contains("no OBJ text supplied"), "{err}");
    }

    #[test]
    fn obj_without_faces_is_an_error() {
        let err = convert("v 0 0 0\nv 1 0 0\n", &Options::default()).unwrap_err();
        assert!(err.contains("no faces"), "{err}");
        assert!(err.contains("2 vertices"), "{err}");
    }

    #[test]
    fn out_of_range_face_index_is_an_error() {
        let err = convert("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 9\n", &Options::default()).unwrap_err();
        assert!(err.contains("line 4"), "{err}");
        assert!(err.contains("index 9"), "{err}");
        assert!(err.contains("only 3"), "{err}");
    }

    #[test]
    fn non_numeric_vertex_is_an_error() {
        let err = convert("v 0 zero 0\nf 1 1 1\n", &Options::default()).unwrap_err();
        assert!(err.contains("line 1"), "{err}");
        assert!(err.contains("got 'zero'"), "{err}");
    }

    #[test]
    fn degenerate_face_is_an_error() {
        let err = convert("v 0 0 0\nv 1 0 0\nf 1 2\n", &Options::default()).unwrap_err();
        assert!(err.contains("at least 3 vertices"), "{err}");
    }

    #[test]
    fn zero_scale_is_rejected() {
        let opt = Options {
            scale: 0.0,
            ..Options::default()
        };
        let err = convert(TRI_OBJ, &opt).unwrap_err();
        assert!(err.contains("non-zero finite"), "{err}");
    }

    #[test]
    fn oversized_paste_is_rejected() {
        let big = "v 0 0 0\n".repeat(MAX_INPUT_BYTES / 8 + 2);
        let err = convert(&big, &Options::default()).unwrap_err();
        assert!(err.contains("over the 8 MB limit"), "{err}");
    }

    #[test]
    fn triangle_cap_is_enforced() {
        let mut obj = String::from("v 0 0 0\nv 1 0 0\nv 0 1 0\n");
        for _ in 0..=MAX_TRIANGLES {
            obj.push_str("f 1 2 3\n");
        }
        let err = convert(&obj, &Options::default()).unwrap_err();
        assert!(err.contains("triangle limit"), "{err}");
    }

    #[test]
    fn parsers_reject_unknown_enum_values() {
        assert!(Target::parse("fbx")
            .unwrap_err()
            .contains("expected 'gltf'"));
        assert!(UpAxis::parse("x").unwrap_err().contains("expected 'y'"));
        assert!(Normals::parse("smooth")
            .unwrap_err()
            .contains("expected 'auto'"));
    }

    #[test]
    fn mtl_texture_maps_are_flagged_not_embedded() {
        let mats = parse_mtl("newmtl skin\nKd 1 1 1\nmap_Kd skin.png\n").unwrap();
        assert_eq!(mats.len(), 1);
        assert!(mats[0].had_texture_ref);
    }

    #[test]
    fn ns_maps_to_roughness() {
        let mats = parse_mtl("newmtl shiny\nNs 998\nnewmtl matte\nNs 0\n").unwrap();
        // Ns 998 → sqrt(2/1000) ≈ 0.0447; Ns 0 → 1.0.
        assert!((mats[0].roughness - 0.044721).abs() < 1e-5, "{:?}", mats[0]);
        assert_eq!(mats[1].roughness, 1.0);
    }

    #[test]
    fn unknown_usemtl_falls_back_to_the_default_material() {
        let opt = Options {
            mtl: "newmtl red\nKd 1 0 0\n".to_string(),
            ..Options::default()
        };
        let out = convert("v 0 0 0\nv 1 0 0\nv 0 1 0\nusemtl missing\nf 1 2 3\n", &opt).unwrap();
        // Material 0 is "red" (unused); the default is appended at index 1.
        assert!(out.contains("\"name\": \"default\""));
        assert!(out.contains("\"material\": 1"));
    }
}
