//! ply-to-obj core — pure compute, shared by the chat skill block and the web page.
//!
//! Converts a Stanford PLY mesh **or point cloud** into a Wavefront OBJ file.
//! All three PLY encodings are read: `ascii`, `binary_little_endian` and
//! `binary_big_endian`. ASCII PLY is pasted as text; a binary PLY is pasted as
//! base64 or hex bytes (a `data:…;base64,…` URL works too) because binary PLY
//! is not text — the same paste convention the STL tools use.
//!
//! Per-vertex extras that OBJ can express are carried over when present and
//! enabled: vertex colors (as the widely-supported `v x y z r g b` extension),
//! normals (`vn` + `f v//vn`) and texture coordinates (`vt` + `f v/vt`). A PLY
//! with no `face` element is a point cloud and emits `v` lines only.
//!
//! Unknown elements and unknown properties are skipped but still consumed, so a
//! PLY carrying edges, per-face colors or custom scanner properties parses
//! correctly instead of desynchronising the byte/token stream.
//!
//! No I/O, no external crates.

/// Hard caps, stated on the page: 200000 vertices and 200000 faces.
pub const MAX_VERTICES: usize = 200_000;
pub const MAX_FACES: usize = 200_000;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// How the pasted input is encoded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputFormat {
    Auto,
    Ascii,
    Base64,
    Hex,
}

impl InputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "" => Ok(InputFormat::Auto),
            "ascii" | "text" => Ok(InputFormat::Ascii),
            "base64" | "b64" => Ok(InputFormat::Base64),
            "hex" => Ok(InputFormat::Hex),
            other => Err(format!(
                "unknown input_format '{other}': expected 'auto', 'ascii', 'base64' or 'hex'"
            )),
        }
    }
}

/// Coordinate-frame reorientation (graphics Y-up <-> 3D-print/CAD Z-up).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    Keep,
    YupToZup,
    ZupToYup,
}

impl Axis {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "keep" | "" => Ok(Axis::Keep),
            "y-up-to-z-up" | "y-to-z" | "yup-to-zup" => Ok(Axis::YupToZup),
            "z-up-to-y-up" | "z-to-y" | "zup-to-yup" => Ok(Axis::ZupToYup),
            other => Err(format!(
                "unknown axis '{other}': expected 'keep', 'y-up-to-z-up', or 'z-up-to-y-up'"
            )),
        }
    }

    /// Rotate a direction vector. Positions and normals rotate identically
    /// (a pure rotation is its own normal matrix), so one helper serves both.
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

/// What to return.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    /// The converted OBJ text.
    Obj,
    /// A readable report about the source PLY and what was carried over.
    Summary,
}

impl Output {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "obj" | "" => Ok(Output::Obj),
            "summary" | "report" => Ok(Output::Summary),
            other => Err(format!(
                "unknown output '{other}': expected 'obj' or 'summary'"
            )),
        }
    }
}

pub struct Options {
    pub input_format: InputFormat,
    pub colors: bool,
    pub normals: bool,
    pub uvs: bool,
    pub triangulate: bool,
    pub scale: f64,
    pub axis: Axis,
    pub name: String,
    pub output: Output,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input_format: InputFormat::Auto,
            colors: true,
            normals: true,
            uvs: true,
            triangulate: false,
            scale: 1.0,
            axis: Axis::Keep,
            name: "mesh".to_string(),
            output: Output::Obj,
        }
    }
}

// ---------------------------------------------------------------------------
// PLY header model
// ---------------------------------------------------------------------------

/// A PLY scalar type. PLY 1.0 names and the newer explicit-width aliases are
/// both accepted (`uchar` == `uint8`, `float` == `float32`, …).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Scalar {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}

impl Scalar {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "char" | "int8" => Ok(Scalar::I8),
            "uchar" | "uint8" | "u8" => Ok(Scalar::U8),
            "short" | "int16" => Ok(Scalar::I16),
            "ushort" | "uint16" => Ok(Scalar::U16),
            "int" | "int32" => Ok(Scalar::I32),
            "uint" | "uint32" => Ok(Scalar::U32),
            "float" | "float32" => Ok(Scalar::F32),
            "double" | "float64" => Ok(Scalar::F64),
            other => Err(format!("unknown PLY property type '{other}'")),
        }
    }

    fn size(self) -> usize {
        match self {
            Scalar::I8 | Scalar::U8 => 1,
            Scalar::I16 | Scalar::U16 => 2,
            Scalar::I32 | Scalar::U32 | Scalar::F32 => 4,
            Scalar::F64 => 8,
        }
    }

    /// True for the integer types, whose color values run 0..=255 rather than 0..=1.
    fn is_integer(self) -> bool {
        !matches!(self, Scalar::F32 | Scalar::F64)
    }
}

#[derive(Clone, Debug)]
enum Property {
    Scalar { name: String, ty: Scalar },
    /// `property list <count_ty> <item_ty> <name>`
    List {
        name: String,
        count_ty: Scalar,
        item_ty: Scalar,
    },
}

impl Property {
    fn name(&self) -> &str {
        match self {
            Property::Scalar { name, .. } => name,
            Property::List { name, .. } => name,
        }
    }
}

#[derive(Clone, Debug)]
struct Element {
    name: String,
    count: usize,
    props: Vec<Property>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Encoding {
    Ascii,
    BinaryLe,
    BinaryBe,
}

struct Header {
    encoding: Encoding,
    elements: Vec<Element>,
    /// Byte offset of the first body byte (binary) — for ASCII this is the
    /// offset of the first body character.
    body_offset: usize,
    comments: Vec<String>,
}

// ---------------------------------------------------------------------------
// Parsed mesh
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
struct Vertex {
    pos: [f64; 3],
    normal: Option<[f64; 3]>,
    uv: Option<[f64; 2]>,
    color: Option<[f64; 3]>,
}

/// Which optional per-vertex properties the source PLY actually declared.
#[derive(Clone, Copy, Debug, Default)]
pub struct Present {
    pub normals: bool,
    pub uvs: bool,
    pub colors: bool,
    pub alpha: bool,
}

/// Everything the converter learned about the source file, for `output=summary`.
#[derive(Clone, Debug)]
pub struct Report {
    pub encoding: String,
    pub vertex_count: usize,
    pub face_count: usize,
    /// Faces with more than 3 corners, before any triangulation.
    pub polygon_count: usize,
    pub present: Present,
    /// Element names in header order, with their counts.
    pub elements: Vec<(String, usize)>,
    /// Vertex properties the converter did not map to OBJ.
    pub ignored_vertex_props: Vec<String>,
    pub comments: Vec<String>,
    pub is_point_cloud: bool,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Convert a PLY mesh or point cloud into Wavefront OBJ (or a summary report).
pub fn convert(input: &str, opt: &Options) -> Result<String, String> {
    let (mesh, report) = parse_ply(input, opt)?;
    match opt.output {
        Output::Obj => Ok(emit_obj(&mesh.0, &mesh.1, opt, &report)),
        Output::Summary => Ok(emit_summary(&report, opt)),
    }
}

type Faces = Vec<Vec<usize>>;

/// Decode + parse the PLY, apply scale/axis, and return the mesh plus a report.
fn parse_ply(input: &str, opt: &Options) -> Result<((Vec<Vertex>, Faces), Report), String> {
    let bytes = decode(input, opt.input_format)?;
    let header = parse_header(&bytes)?;

    let mut verts: Vec<Vertex> = Vec::new();
    let mut faces: Faces = Vec::new();
    let mut present = Present::default();
    let mut ignored: Vec<String> = Vec::new();
    let mut polygon_count = 0usize;
    let mut saw_vertex_element = false;
    let mut saw_face_element = false;

    let elements: Vec<(String, usize)> = header
        .elements
        .iter()
        .map(|e| (e.name.clone(), e.count))
        .collect();

    // One cursor walks the whole body; every element is consumed in header
    // order, whether or not we care about it, so offsets stay in sync.
    let mut cur = Cursor::new(&bytes, header.body_offset, header.encoding);

    for el in &header.elements {
        let is_vertex = el.name == "vertex";
        let is_face = el.name == "face";

        if is_vertex {
            saw_vertex_element = true;
            if el.count > MAX_VERTICES {
                return Err(format!(
                    "PLY has {} vertices, over the {MAX_VERTICES} limit",
                    el.count
                ));
            }
            let map = VertexMap::build(&el.props);
            present = map.present;
            ignored = map.ignored.clone();
            verts.reserve(el.count);
            for i in 0..el.count {
                let v = map
                    .read(&mut cur, &el.props)
                    .map_err(|e| format!("vertex {}: {e}", i + 1))?;
                verts.push(v);
            }
            continue;
        }

        if is_face {
            saw_face_element = true;
            if el.count > MAX_FACES {
                return Err(format!(
                    "PLY has {} faces, over the {MAX_FACES} limit",
                    el.count
                ));
            }
            faces.reserve(el.count);
            for i in 0..el.count {
                let mut corners: Option<Vec<usize>> = None;
                for p in &el.props {
                    match p {
                        Property::List {
                            name,
                            count_ty,
                            item_ty,
                        } => {
                            let vals = cur
                                .read_list(*count_ty, *item_ty)
                                .map_err(|e| format!("face {}: {e}", i + 1))?;
                            // `vertex_indices` is the PLY 1.0 spec name;
                            // `vertex_index` is what several exporters write.
                            if corners.is_none()
                                && (name == "vertex_indices" || name == "vertex_index")
                            {
                                let mut idx = Vec::with_capacity(vals.len());
                                for v in &vals {
                                    let n = *v;
                                    if n < 0.0 || n as usize >= usize::MAX {
                                        return Err(format!(
                                            "face {}: vertex index {n} is negative",
                                            i + 1
                                        ));
                                    }
                                    idx.push(n as usize);
                                }
                                corners = Some(idx);
                            }
                        }
                        Property::Scalar { ty, .. } => {
                            cur.read_scalar(*ty)
                                .map_err(|e| format!("face {}: {e}", i + 1))?;
                        }
                    }
                }
                let corners = corners.ok_or_else(|| {
                    "PLY face element has no 'vertex_indices' list property".to_string()
                })?;
                if corners.len() < 3 {
                    return Err(format!(
                        "face {} has {} corners; a face needs at least 3",
                        i + 1,
                        corners.len()
                    ));
                }
                if corners.len() > 3 {
                    polygon_count += 1;
                }
                faces.push(corners);
            }
            continue;
        }

        // Unknown element (edge, material, custom scanner data): consume and drop.
        for _ in 0..el.count {
            for p in &el.props {
                match p {
                    Property::Scalar { ty, .. } => {
                        cur.read_scalar(*ty)
                            .map_err(|e| format!("element '{}': {e}", el.name))?;
                    }
                    Property::List {
                        count_ty, item_ty, ..
                    } => {
                        cur.read_list(*count_ty, *item_ty)
                            .map_err(|e| format!("element '{}': {e}", el.name))?;
                    }
                }
            }
        }
    }

    if !saw_vertex_element {
        return Err("PLY header declares no 'vertex' element — nothing to convert".to_string());
    }
    if verts.is_empty() {
        return Err("PLY has 0 vertices — nothing to convert".to_string());
    }

    // Every face index must resolve, or the OBJ would reference missing vertices.
    for (i, f) in faces.iter().enumerate() {
        for &c in f {
            if c >= verts.len() {
                return Err(format!(
                    "face {}: vertex index {c} is out of range (the file declares {} vertices)",
                    i + 1,
                    verts.len()
                ));
            }
        }
    }

    // Scale, then reorient. Normals are rotated but never scaled (a uniform
    // scale leaves a unit normal's direction unchanged).
    let scale = if opt.scale == 0.0 { 1.0 } else { opt.scale };
    for v in verts.iter_mut() {
        v.pos = opt
            .axis
            .apply([v.pos[0] * scale, v.pos[1] * scale, v.pos[2] * scale]);
        if let Some(n) = v.normal {
            v.normal = Some(opt.axis.apply(n));
        }
    }

    let report = Report {
        encoding: match header.encoding {
            Encoding::Ascii => "ascii".to_string(),
            Encoding::BinaryLe => "binary_little_endian".to_string(),
            Encoding::BinaryBe => "binary_big_endian".to_string(),
        },
        vertex_count: verts.len(),
        face_count: faces.len(),
        polygon_count,
        present,
        elements,
        ignored_vertex_props: ignored,
        comments: header.comments.clone(),
        is_point_cloud: !saw_face_element || faces.is_empty(),
    };
    Ok(((verts, faces), report))
}

// ---------------------------------------------------------------------------
// Header parsing
// ---------------------------------------------------------------------------

fn parse_header(bytes: &[u8]) -> Result<Header, String> {
    // The header is ASCII regardless of the body encoding, so it can be read
    // line by line off the raw bytes.
    if !bytes.starts_with(b"ply") {
        return Err("not a PLY file: it must start with the magic word 'ply'".to_string());
    }
    let end = find_subslice(bytes, b"end_header")
        .ok_or_else(|| "PLY header has no 'end_header' line".to_string())?;
    // Body starts after end_header's line terminator (\n or \r\n).
    let mut body_offset = end + b"end_header".len();
    while body_offset < bytes.len() && (bytes[body_offset] == b'\r') {
        body_offset += 1;
    }
    if body_offset < bytes.len() && bytes[body_offset] == b'\n' {
        body_offset += 1;
    }

    let header_text = String::from_utf8_lossy(&bytes[..end]).to_string();
    let mut encoding: Option<Encoding> = None;
    let mut elements: Vec<Element> = Vec::new();
    let mut comments: Vec<String> = Vec::new();

    for line in header_text.lines() {
        let line = line.trim();
        let mut it = line.split_whitespace();
        match it.next() {
            Some("ply") | None => {}
            Some("comment") | Some("obj_info") => {
                let rest = it.collect::<Vec<_>>().join(" ");
                if !rest.is_empty() {
                    comments.push(rest);
                }
            }
            Some("format") => {
                let kind = it.next().unwrap_or("");
                encoding = Some(match kind {
                    "ascii" => Encoding::Ascii,
                    "binary_little_endian" => Encoding::BinaryLe,
                    "binary_big_endian" => Encoding::BinaryBe,
                    other => {
                        return Err(format!(
                            "unknown PLY format '{other}': expected 'ascii', \
                             'binary_little_endian' or 'binary_big_endian'"
                        ))
                    }
                });
            }
            Some("element") => {
                let name = it
                    .next()
                    .ok_or_else(|| "PLY 'element' line has no name".to_string())?
                    .to_string();
                let count: usize = it
                    .next()
                    .ok_or_else(|| format!("PLY element '{name}' has no count"))?
                    .parse()
                    .map_err(|_| format!("PLY element '{name}' has a non-numeric count"))?;
                elements.push(Element {
                    name,
                    count,
                    props: Vec::new(),
                });
            }
            Some("property") => {
                let el = elements.last_mut().ok_or_else(|| {
                    "PLY 'property' line appears before any 'element' line".to_string()
                })?;
                let first = it
                    .next()
                    .ok_or_else(|| "PLY 'property' line is incomplete".to_string())?;
                if first == "list" {
                    let count_ty = Scalar::parse(
                        it.next()
                            .ok_or_else(|| "PLY list property has no count type".to_string())?,
                    )?;
                    let item_ty = Scalar::parse(
                        it.next()
                            .ok_or_else(|| "PLY list property has no item type".to_string())?,
                    )?;
                    let name = it
                        .next()
                        .ok_or_else(|| "PLY list property has no name".to_string())?
                        .to_string();
                    el.props.push(Property::List {
                        name,
                        count_ty,
                        item_ty,
                    });
                } else {
                    let ty = Scalar::parse(first)?;
                    let name = it
                        .next()
                        .ok_or_else(|| "PLY property has no name".to_string())?
                        .to_string();
                    el.props.push(Property::Scalar { name, ty });
                }
            }
            // Unknown header keywords are ignored, as the spec allows.
            Some(_) => {}
        }
    }

    let encoding = encoding.ok_or_else(|| "PLY header has no 'format' line".to_string())?;
    if elements.is_empty() {
        return Err("PLY header declares no elements".to_string());
    }
    Ok(Header {
        encoding,
        elements,
        body_offset,
        comments,
    })
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

// ---------------------------------------------------------------------------
// Body cursor — one reader for ASCII tokens and binary bytes alike
// ---------------------------------------------------------------------------

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
    encoding: Encoding,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], pos: usize, encoding: Encoding) -> Self {
        Cursor {
            bytes,
            pos,
            encoding,
        }
    }

    /// Read one value, as an `f64` regardless of its declared width — every PLY
    /// scalar type fits an `f64` exactly except `uint32`/`float64` extremes,
    /// which are outside any real mesh's index or coordinate range.
    fn read_scalar(&mut self, ty: Scalar) -> Result<f64, String> {
        match self.encoding {
            Encoding::Ascii => {
                let tok = self.next_token()?;
                if ty.is_integer() {
                    // Some exporters write "1.0" where an int is declared.
                    tok.parse::<f64>()
                        .map_err(|_| format!("'{tok}' is not a number"))
                } else {
                    tok.parse::<f64>()
                        .map_err(|_| format!("'{tok}' is not a number"))
                }
            }
            _ => {
                let n = ty.size();
                if self.pos + n > self.bytes.len() {
                    return Err("binary PLY body ended early (file truncated?)".to_string());
                }
                let raw = &self.bytes[self.pos..self.pos + n];
                self.pos += n;
                let le = self.encoding == Encoding::BinaryLe;
                Ok(decode_scalar(raw, ty, le))
            }
        }
    }

    fn read_list(&mut self, count_ty: Scalar, item_ty: Scalar) -> Result<Vec<f64>, String> {
        let n = self.read_scalar(count_ty)?;
        if !(0.0..=1_000_000.0).contains(&n) {
            return Err(format!("list length {n} is out of range"));
        }
        let n = n as usize;
        let mut out = Vec::with_capacity(n.min(1024));
        for _ in 0..n {
            out.push(self.read_scalar(item_ty)?);
        }
        Ok(out)
    }

    fn next_token(&mut self) -> Result<&'a str, String> {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
        if self.pos >= self.bytes.len() {
            return Err("ASCII PLY body ended early (fewer values than the header declares)"
                .to_string());
        }
        let start = self.pos;
        while self.pos < self.bytes.len() && !self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
        std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| "ASCII PLY body contains non-UTF-8 bytes".to_string())
    }
}

fn decode_scalar(raw: &[u8], ty: Scalar, le: bool) -> f64 {
    /// Reorder to native (little-endian) order so the `from_le_bytes` family applies.
    fn ord<const N: usize>(raw: &[u8], le: bool) -> [u8; N] {
        let mut b = [0u8; N];
        b.copy_from_slice(raw);
        if !le {
            b.reverse();
        }
        b
    }
    match ty {
        Scalar::I8 => raw[0] as i8 as f64,
        Scalar::U8 => raw[0] as f64,
        Scalar::I16 => i16::from_le_bytes(ord::<2>(raw, le)) as f64,
        Scalar::U16 => u16::from_le_bytes(ord::<2>(raw, le)) as f64,
        Scalar::I32 => i32::from_le_bytes(ord::<4>(raw, le)) as f64,
        Scalar::U32 => u32::from_le_bytes(ord::<4>(raw, le)) as f64,
        Scalar::F32 => f32::from_le_bytes(ord::<4>(raw, le)) as f64,
        Scalar::F64 => f64::from_le_bytes(ord::<8>(raw, le)),
    }
}

// ---------------------------------------------------------------------------
// Vertex property mapping
// ---------------------------------------------------------------------------

/// Resolves the vertex element's declared properties to the OBJ-expressible
/// slots once, up front, so the per-vertex read is a straight walk.
struct VertexMap {
    /// Index into `props` for each slot: x,y,z / nx,ny,nz / u,v / r,g,b.
    pos: [Option<usize>; 3],
    normal: [Option<usize>; 3],
    uv: [Option<usize>; 2],
    color: [Option<usize>; 3],
    /// Whether each color channel came from an integer type (0..=255).
    color_is_int: [bool; 3],
    present: Present,
    ignored: Vec<String>,
}

impl VertexMap {
    fn build(props: &[Property]) -> VertexMap {
        let mut m = VertexMap {
            pos: [None; 3],
            normal: [None; 3],
            uv: [None; 2],
            color: [None; 3],
            color_is_int: [true; 3],
            present: Present::default(),
            ignored: Vec::new(),
        };
        for (i, p) in props.iter().enumerate() {
            let name = p.name().to_ascii_lowercase();
            let ty = match p {
                Property::Scalar { ty, .. } => *ty,
                // A list property on the vertex element is never a coordinate.
                Property::List { .. } => {
                    m.ignored.push(p.name().to_string());
                    continue;
                }
            };
            // Slot names follow the PLY conventions in the wild: the spec's
            // x/y/z + nx/ny/nz, the two common UV spellings, and the several
            // color spellings MeshLab / scanners emit.
            let slot: Option<(&mut Option<usize>, usize)> = match name.as_str() {
                "x" => Some((&mut m.pos[0], 0)),
                "y" => Some((&mut m.pos[1], 0)),
                "z" => Some((&mut m.pos[2], 0)),
                "nx" => Some((&mut m.normal[0], 1)),
                "ny" => Some((&mut m.normal[1], 1)),
                "nz" => Some((&mut m.normal[2], 1)),
                "s" | "u" | "texture_u" | "texture_s" => Some((&mut m.uv[0], 2)),
                "t" | "v" | "texture_v" | "texture_t" => Some((&mut m.uv[1], 2)),
                "red" | "r" | "diffuse_red" => Some((&mut m.color[0], 3)),
                "green" | "g" | "diffuse_green" => Some((&mut m.color[1], 3)),
                "blue" | "b" | "diffuse_blue" => Some((&mut m.color[2], 3)),
                "alpha" | "a" | "diffuse_alpha" => {
                    // OBJ has no per-vertex alpha; note it and drop it.
                    m.present.alpha = true;
                    m.ignored.push(p.name().to_string());
                    continue;
                }
                _ => None,
            };
            match slot {
                Some((cell, group)) => {
                    if cell.is_none() {
                        *cell = Some(i);
                        if group == 3 {
                            // Record the channel's numeric range for scaling.
                            let ch = match name.as_str() {
                                "red" | "r" | "diffuse_red" => 0,
                                "green" | "g" | "diffuse_green" => 1,
                                _ => 2,
                            };
                            m.color_is_int[ch] = ty.is_integer();
                        }
                    }
                }
                None => m.ignored.push(p.name().to_string()),
            }
        }
        m.present.normals = m.normal.iter().all(|s| s.is_some());
        m.present.uvs = m.uv.iter().all(|s| s.is_some());
        m.present.colors = m.color.iter().all(|s| s.is_some());
        // A partially-declared triple can't be used; report it as ignored.
        if !m.present.normals {
            m.normal = [None; 3];
        }
        if !m.present.uvs {
            m.uv = [None; 2];
        }
        if !m.present.colors {
            m.color = [None; 3];
        }
        m
    }

    /// Read one vertex, consuming every declared property in order.
    fn read(&self, cur: &mut Cursor, props: &[Property]) -> Result<Vertex, String> {
        let mut vals: Vec<f64> = Vec::with_capacity(props.len());
        for p in props {
            match p {
                Property::Scalar { ty, .. } => vals.push(cur.read_scalar(*ty)?),
                Property::List {
                    count_ty, item_ty, ..
                } => {
                    cur.read_list(*count_ty, *item_ty)?;
                    // Placeholder so `vals` stays index-aligned with `props`.
                    vals.push(f64::NAN);
                }
            }
        }
        let get = |slot: Option<usize>| slot.map(|i| vals[i]);
        let pos = [
            get(self.pos[0]).ok_or("PLY vertex element has no 'x' property")?,
            get(self.pos[1]).ok_or("PLY vertex element has no 'y' property")?,
            get(self.pos[2]).ok_or("PLY vertex element has no 'z' property")?,
        ];
        let normal = match (get(self.normal[0]), get(self.normal[1]), get(self.normal[2])) {
            (Some(a), Some(b), Some(c)) => Some([a, b, c]),
            _ => None,
        };
        let uv = match (get(self.uv[0]), get(self.uv[1])) {
            (Some(a), Some(b)) => Some([a, b]),
            _ => None,
        };
        let color = match (get(self.color[0]), get(self.color[1]), get(self.color[2])) {
            (Some(r), Some(g), Some(b)) => {
                // Integer channels are 0..=255; float channels are already 0..=1.
                let norm = |v: f64, is_int: bool| {
                    let x = if is_int { v / 255.0 } else { v };
                    x.clamp(0.0, 1.0)
                };
                Some([
                    norm(r, self.color_is_int[0]),
                    norm(g, self.color_is_int[1]),
                    norm(b, self.color_is_int[2]),
                ])
            }
            _ => None,
        };
        Ok(Vertex {
            pos,
            normal,
            uv,
            color,
        })
    }
}

// ---------------------------------------------------------------------------
// OBJ emitter
// ---------------------------------------------------------------------------

/// Format a float compactly: whole numbers print without a decimal point,
/// otherwise up to 6 decimals with trailing zeros trimmed. `-0` prints as `0`.
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

fn sanitize_name(name: &str) -> String {
    let n = name.trim();
    if n.is_empty() {
        "mesh".to_string()
    } else {
        // OBJ `o` is a single line.
        n.replace(['\n', '\r'], " ")
    }
}

fn emit_obj(verts: &[Vertex], faces: &Faces, opt: &Options, report: &Report) -> String {
    let name = sanitize_name(&opt.name);
    let want_colors = opt.colors && report.present.colors;
    let want_normals = opt.normals && report.present.normals;
    let want_uvs = opt.uvs && report.present.uvs;

    let mut out = String::new();
    out.push_str("# OBJ generated by gizza ply-to-obj\n");
    out.push_str(&format!(
        "# source: PLY ({}), {} vertices, {} faces\n",
        report.encoding, report.vertex_count, report.face_count
    ));
    if want_colors {
        out.push_str("# vertex colors are the r g b values after each v (0..1)\n");
    }
    out.push_str(&format!("o {name}\n"));

    for v in verts {
        if want_colors {
            let c = v.color.unwrap_or([1.0, 1.0, 1.0]);
            out.push_str(&format!(
                "v {} {} {} {} {} {}\n",
                fnum(v.pos[0]),
                fnum(v.pos[1]),
                fnum(v.pos[2]),
                fnum(c[0]),
                fnum(c[1]),
                fnum(c[2])
            ));
        } else {
            out.push_str(&format!(
                "v {} {} {}\n",
                fnum(v.pos[0]),
                fnum(v.pos[1]),
                fnum(v.pos[2])
            ));
        }
    }
    if want_uvs {
        for v in verts {
            let t = v.uv.unwrap_or([0.0, 0.0]);
            out.push_str(&format!("vt {} {}\n", fnum(t[0]), fnum(t[1])));
        }
    }
    if want_normals {
        for v in verts {
            let n = v.normal.unwrap_or([0.0, 0.0, 0.0]);
            out.push_str(&format!(
                "vn {} {} {}\n",
                fnum(n[0]),
                fnum(n[1]),
                fnum(n[2])
            ));
        }
    }

    // OBJ indices are 1-based; PLY's are 0-based. `vt`/`vn` are emitted one per
    // vertex above, so all three indices of a corner share the same number.
    let corner = |i: usize| -> String {
        let n = i + 1;
        match (want_uvs, want_normals) {
            (true, true) => format!("{n}/{n}/{n}"),
            (true, false) => format!("{n}/{n}"),
            (false, true) => format!("{n}//{n}"),
            (false, false) => format!("{n}"),
        }
    };
    for f in faces {
        if opt.triangulate && f.len() > 3 {
            // Fan triangulation from the first corner.
            for k in 1..f.len() - 1 {
                out.push_str(&format!(
                    "f {} {} {}\n",
                    corner(f[0]),
                    corner(f[k]),
                    corner(f[k + 1])
                ));
            }
        } else {
            let line: Vec<String> = f.iter().map(|&i| corner(i)).collect();
            out.push_str(&format!("f {}\n", line.join(" ")));
        }
    }
    out
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn emit_summary(r: &Report, opt: &Options) -> String {
    let mut out = String::new();
    out.push_str("PLY → OBJ conversion summary\n");
    out.push_str(&format!("  source encoding:   PLY {}\n", r.encoding));
    out.push_str(&format!(
        "  kind:              {}\n",
        if r.is_point_cloud {
            "point cloud (no face element)"
        } else {
            "mesh"
        }
    ));
    out.push_str(&format!("  vertices:          {}\n", r.vertex_count));
    out.push_str(&format!("  faces:             {}\n", r.face_count));
    out.push_str(&format!(
        "  polygon faces:     {} (more than 3 corners){}\n",
        r.polygon_count,
        if r.polygon_count > 0 && opt.triangulate {
            " — fan-triangulated"
        } else if r.polygon_count > 0 {
            " — kept as OBJ n-gons"
        } else {
            ""
        }
    ));
    out.push_str(&format!(
        "  vertex colors:     {} in source, {} written\n",
        yes_no(r.present.colors),
        yes_no(r.present.colors && opt.colors)
    ));
    out.push_str(&format!(
        "  vertex normals:    {} in source, {} written\n",
        yes_no(r.present.normals),
        yes_no(r.present.normals && opt.normals)
    ));
    out.push_str(&format!(
        "  texture coords:    {} in source, {} written\n",
        yes_no(r.present.uvs),
        yes_no(r.present.uvs && opt.uvs)
    ));
    let els: Vec<String> = r
        .elements
        .iter()
        .map(|(n, c)| format!("{n}={c}"))
        .collect();
    out.push_str(&format!("  header elements:   {}\n", els.join(", ")));
    out.push_str(&format!(
        "  ignored vertex properties: {}\n",
        if r.ignored_vertex_props.is_empty() {
            "none".to_string()
        } else {
            r.ignored_vertex_props.join(", ")
        }
    ));
    if r.present.alpha {
        out.push_str("  note: per-vertex alpha was dropped — OBJ has no alpha channel\n");
    }
    if !r.comments.is_empty() {
        out.push_str(&format!("  comments:          {}\n", r.comments.join(" | ")));
    }
    out
}

// ---------------------------------------------------------------------------
// Input decoding — ASCII text vs binary bytes (as base64 or hex)
// ---------------------------------------------------------------------------

fn decode(input: &str, fmt: InputFormat) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(
            "no input: paste an ASCII PLY, or a binary PLY's bytes as base64 or hex".to_string(),
        );
    }
    match fmt {
        InputFormat::Ascii => Ok(trimmed.as_bytes().to_vec()),
        InputFormat::Base64 => decode_base64(trimmed),
        InputFormat::Hex => decode_hex(trimmed),
        InputFormat::Auto => {
            // A PLY always starts with the magic word `ply`, so raw text is
            // recognisable up front and never mistaken for base64/hex.
            if trimmed.starts_with("ply") || trimmed.starts_with("PLY") {
                return Ok(trimmed.as_bytes().to_vec());
            }
            // Hex's alphabet is a strict subset of base64's, so try hex first
            // or a hex dump would be mis-read as base64.
            let hex_err = match decode_hex(trimmed) {
                Ok(b) if b.starts_with(b"ply") => return Ok(b),
                Ok(_) => "decoded bytes do not start with the PLY magic word".to_string(),
                Err(e) => e,
            };
            let b64_err = match decode_base64(trimmed) {
                Ok(b) if b.starts_with(b"ply") => return Ok(b),
                Ok(_) => "decoded bytes do not start with the PLY magic word".to_string(),
                Err(e) => e,
            };
            Err(format!(
                "could not read the input as an ASCII PLY, hex bytes or base64 bytes. \
                 As hex: {hex_err}. As base64: {b64_err}. Set input_format explicitly if \
                 the input really is one of those."
            ))
        }
    }
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
/// A `data:…;base64,…` URL is accepted and its prefix stripped.
fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    let body = s.strip_prefix("data:").map_or(s, |_| {
        s.split_once("base64,").map_or(s, |(_, rest)| rest)
    });
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    let mut out = Vec::with_capacity(body.len() / 4 * 3 + 3);
    for (i, c) in body.bytes().enumerate() {
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
    let body = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    let mut nibbles: Vec<u8> = Vec::with_capacity(body.len());
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
        return Err("not valid hex: an odd number of hex digits".to_string());
    }
    Ok(nibbles.chunks(2).map(|p| (p[0] << 4) | p[1]).collect())
}

/// Standard base64 — exposed so the tests (and callers building fixtures) can
/// round-trip a binary PLY through the same alphabet the decoder accepts.
pub fn to_base64(data: &[u8]) -> String {
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

/// Lowercase hex — the companion to [`to_base64`] for the same purpose.
pub fn to_hex(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() * 2);
    for b in data {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        out.push(char::from_digit((b & 15) as u32, 16).unwrap());
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// One ASCII triangle, positions only — the minimal valid PLY.
    const TRI: &str = "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n0 1 0\n3 0 1 2\n";

    #[test]
    fn ascii_triangle_exact_obj() {
        let out = convert(TRI, &Options::default()).unwrap();
        let expected = "# OBJ generated by gizza ply-to-obj\n# source: PLY (ascii), 3 vertices, 1 faces\no mesh\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn vertex_colors_are_normalized_to_0_1() {
        let ply = "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0 255 0 0\n1 0 0 0 255 0\n0 1 0 0 0 255\n3 0 1 2\n";
        let out = convert(ply, &Options::default()).unwrap();
        assert!(out.contains("v 0 0 0 1 0 0"), "got: {out}");
        assert!(out.contains("v 1 0 0 0 1 0"), "got: {out}");
        // Disabling colors falls back to bare positions.
        let plain = convert(
            ply,
            &Options {
                colors: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(plain.contains("v 0 0 0\n"), "got: {plain}");
        assert!(!plain.contains("v 0 0 0 1 0 0"));
    }

    #[test]
    fn float_colors_pass_through_unscaled() {
        let ply = "ply\nformat ascii 1.0\nelement vertex 1\nproperty float x\nproperty float y\nproperty float z\nproperty float red\nproperty float green\nproperty float blue\nend_header\n0 0 0 0.5 0.25 1\n";
        let out = convert(ply, &Options::default()).unwrap();
        assert!(out.contains("v 0 0 0 0.5 0.25 1"), "got: {out}");
    }

    #[test]
    fn normals_and_uvs_become_vn_vt_and_indexed_faces() {
        let ply = "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nproperty float nx\nproperty float ny\nproperty float nz\nproperty float s\nproperty float t\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0 0 0 1 0 0\n1 0 0 0 0 1 1 0\n0 1 0 0 0 1 0 1\n3 0 1 2\n";
        let out = convert(ply, &Options::default()).unwrap();
        assert!(out.contains("vn 0 0 1"), "got: {out}");
        assert!(out.contains("vt 1 0"), "got: {out}");
        assert!(out.contains("f 1/1/1 2/2/2 3/3/3"), "got: {out}");

        // Turning normals off leaves the `v/vt` form.
        let no_n = convert(
            ply,
            &Options {
                normals: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(no_n.contains("f 1/1 2/2 3/3"), "got: {no_n}");
        assert!(!no_n.contains("vn "));

        // Turning UVs off leaves the `v//vn` form.
        let no_uv = convert(
            ply,
            &Options {
                uvs: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(no_uv.contains("f 1//1 2//2 3//3"), "got: {no_uv}");
    }

    #[test]
    fn point_cloud_without_face_element_emits_vertices_only() {
        let ply = "ply\nformat ascii 1.0\nelement vertex 2\nproperty float x\nproperty float y\nproperty float z\nend_header\n0 0 0\n1 2 3\n";
        let out = convert(ply, &Options::default()).unwrap();
        assert!(out.contains("v 1 2 3"), "got: {out}");
        assert!(!out.contains("\nf "), "point clouds have no faces: {out}");
        let sum = convert(
            ply,
            &Options {
                output: Output::Summary,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(sum.contains("point cloud"), "got: {sum}");
    }

    #[test]
    fn quad_face_is_kept_as_ngon_but_triangulates_on_request() {
        let ply = "ply\nformat ascii 1.0\nelement vertex 4\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n1 1 0\n0 1 0\n4 0 1 2 3\n";
        let ngon = convert(ply, &Options::default()).unwrap();
        assert!(ngon.contains("f 1 2 3 4"), "got: {ngon}");
        let tri = convert(
            ply,
            &Options {
                triangulate: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(tri.contains("f 1 2 3\n"), "got: {tri}");
        assert!(tri.contains("f 1 3 4\n"), "got: {tri}");
    }

    /// Build a `binary_little_endian` PLY with one triangle, f32 coords.
    fn binary_tri(le: bool) -> Vec<u8> {
        let fmt = if le {
            "binary_little_endian"
        } else {
            "binary_big_endian"
        };
        let header = format!(
            "ply\nformat {fmt} 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n"
        );
        let mut b = header.into_bytes();
        let coords: [f32; 9] = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        for c in coords {
            let bytes = if le {
                c.to_le_bytes()
            } else {
                c.to_be_bytes()
            };
            b.extend_from_slice(&bytes);
        }
        b.push(3u8); // uchar corner count
        for i in 0i32..3 {
            let bytes = if le {
                i.to_le_bytes()
            } else {
                i.to_be_bytes()
            };
            b.extend_from_slice(&bytes);
        }
        b
    }

    #[test]
    fn binary_little_endian_via_base64_matches_ascii() {
        let b64 = to_base64(&binary_tri(true));
        let out = convert(&b64, &Options::default()).unwrap();
        assert!(out.contains("v 1 0 0"), "got: {out}");
        assert!(out.contains("f 1 2 3"), "got: {out}");
        assert!(out.contains("binary_little_endian"), "got: {out}");
    }

    #[test]
    fn binary_big_endian_via_hex_matches_ascii() {
        let hex = to_hex(&binary_tri(false));
        let out = convert(&hex, &Options::default()).unwrap();
        assert!(out.contains("v 0 1 0"), "got: {out}");
        assert!(out.contains("f 1 2 3"), "got: {out}");
    }

    #[test]
    fn binary_accepts_a_data_url() {
        let url = format!("data:application/octet-stream;base64,{}", to_base64(&binary_tri(true)));
        let out = convert(
            &url,
            &Options {
                input_format: InputFormat::Base64,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(out.contains("f 1 2 3"), "got: {out}");
    }

    #[test]
    fn unknown_elements_and_properties_are_consumed_not_desynced() {
        // A trailing `edge` element plus an unmapped per-vertex `quality`
        // property: if either were skipped without consuming, the face indices
        // below would be misread.
        let ply = "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nproperty float quality\nelement face 1\nproperty list uchar int vertex_indices\nproperty uchar flags\nelement edge 1\nproperty int vertex1\nproperty int vertex2\nend_header\n0 0 0 0.9\n1 0 0 0.8\n0 1 0 0.7\n3 0 1 2 7\n0 1\n";
        let out = convert(ply, &Options::default()).unwrap();
        assert!(out.contains("v 1 0 0"), "got: {out}");
        assert!(out.contains("f 1 2 3"), "got: {out}");
        let sum = convert(
            ply,
            &Options {
                output: Output::Summary,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(sum.contains("quality"), "ignored props reported: {sum}");
        assert!(sum.contains("edge=1"), "elements reported: {sum}");
    }

    #[test]
    fn vertex_index_singular_spelling_is_accepted() {
        let ply = TRI.replace("vertex_indices", "vertex_index");
        let out = convert(&ply, &Options::default()).unwrap();
        assert!(out.contains("f 1 2 3"), "got: {out}");
    }

    #[test]
    fn scale_and_axis_apply_to_positions_and_normals() {
        let ply = "ply\nformat ascii 1.0\nelement vertex 1\nproperty float x\nproperty float y\nproperty float z\nproperty float nx\nproperty float ny\nproperty float nz\nend_header\n0 1 0 0 1 0\n";
        let out = convert(
            &ply.to_string(),
            &Options {
                scale: 2.0,
                axis: Axis::YupToZup,
                ..Options::default()
            },
        )
        .unwrap();
        // position (0,1,0) scaled to (0,2,0) then Y-up→Z-up gives (0,0,2)
        assert!(out.contains("v 0 0 2"), "got: {out}");
        // the normal rotates but is NOT scaled
        assert!(out.contains("vn 0 0 1"), "got: {out}");
    }

    #[test]
    fn crlf_header_is_accepted() {
        let ply = TRI.replace('\n', "\r\n");
        let out = convert(&ply, &Options::default()).unwrap();
        assert!(out.contains("f 1 2 3"), "got: {out}");
    }

    #[test]
    fn name_is_written_as_the_obj_object_line() {
        let out = convert(
            TRI,
            &Options {
                name: "scan_01".to_string(),
                ..Options::default()
            },
        )
        .unwrap();
        assert!(out.contains("o scan_01\n"), "got: {out}");
    }

    #[test]
    fn summary_reports_source_properties() {
        let ply = "ply\nformat ascii 1.0\ncomment made by a scanner\nelement vertex 1\nproperty float x\nproperty float y\nproperty float z\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nproperty uchar alpha\nend_header\n0 0 0 10 20 30 255\n";
        let sum = convert(
            ply,
            &Options {
                output: Output::Summary,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(sum.contains("vertex colors:     yes in source, yes written"), "got: {sum}");
        assert!(sum.contains("vertex normals:    no in source"), "got: {sum}");
        assert!(sum.contains("alpha was dropped"), "got: {sum}");
        assert!(sum.contains("made by a scanner"), "got: {sum}");
    }

    // ---- error paths ----

    #[test]
    fn empty_input_errors() {
        let err = convert("   \n ", &Options::default()).unwrap_err();
        assert!(err.contains("no input"), "got: {err}");
    }

    #[test]
    fn non_ply_input_errors() {
        let err = convert("hello world, not a mesh", &Options::default()).unwrap_err();
        assert!(err.contains("could not read the input"), "got: {err}");
    }

    #[test]
    fn missing_end_header_errors() {
        let err = convert("ply\nformat ascii 1.0\nelement vertex 1\n", &Options::default())
            .unwrap_err();
        assert!(err.contains("end_header"), "got: {err}");
    }

    #[test]
    fn missing_xyz_property_errors() {
        let ply = "ply\nformat ascii 1.0\nelement vertex 1\nproperty float x\nproperty float y\nend_header\n0 0\n";
        let err = convert(ply, &Options::default()).unwrap_err();
        assert!(err.contains("no 'z' property"), "got: {err}");
    }

    #[test]
    fn truncated_ascii_body_errors() {
        let ply = "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nend_header\n0 0 0\n1 0 0\n";
        let err = convert(ply, &Options::default()).unwrap_err();
        assert!(err.contains("ended early"), "got: {err}");
    }

    #[test]
    fn out_of_range_face_index_errors() {
        let ply = "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n0 1 0\n3 0 1 9\n";
        let err = convert(ply, &Options::default()).unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
    }

    #[test]
    fn unknown_format_errors() {
        let ply = "ply\nformat binary_middle_endian 1.0\nelement vertex 1\nproperty float x\nproperty float y\nproperty float z\nend_header\n0 0 0\n";
        let err = convert(ply, &Options::default()).unwrap_err();
        assert!(err.contains("unknown PLY format"), "got: {err}");
    }

    #[test]
    fn non_numeric_ascii_value_errors() {
        let ply = "ply\nformat ascii 1.0\nelement vertex 1\nproperty float x\nproperty float y\nproperty float z\nend_header\n0 zero 0\n";
        let err = convert(ply, &Options::default()).unwrap_err();
        assert!(err.contains("not a number"), "got: {err}");
    }

    #[test]
    fn vertex_cap_boundary_is_exact() {
        // At the cap the file converts; one over is rejected before any parsing.
        let header = |n: usize| {
            format!("ply\nformat ascii 1.0\nelement vertex {n}\nproperty float x\nproperty float y\nproperty float z\nend_header\n")
        };
        let body = "0 0 0\n".repeat(MAX_VERTICES);
        let at = format!("{}{}", header(MAX_VERTICES), body);
        let out = convert(&at, &Options::default()).unwrap();
        assert!(out.contains("v 0 0 0"));

        let over = format!("{}{}0 0 0\n", header(MAX_VERTICES + 1), body);
        let err = convert(&over, &Options::default()).unwrap_err();
        assert!(err.contains("over the 200000 limit"), "got: {err}");
    }

    #[test]
    fn face_cap_is_enforced() {
        let ply = format!(
            "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face {}\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n0 1 0\n",
            MAX_FACES + 1
        );
        let err = convert(&ply, &Options::default()).unwrap_err();
        assert!(err.contains("over the 200000 limit"), "got: {err}");
    }

    #[test]
    fn enum_parsers_reject_unknown_values() {
        assert!(InputFormat::parse("yaml").is_err());
        assert!(Axis::parse("sideways").is_err());
        assert!(Output::parse("xml").is_err());
        assert_eq!(InputFormat::parse("b64").unwrap(), InputFormat::Base64);
        assert_eq!(Axis::parse("z-to-y").unwrap(), Axis::ZupToYup);
        assert_eq!(Output::parse("report").unwrap(), Output::Summary);
    }
}
