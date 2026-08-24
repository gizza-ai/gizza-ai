//! stl-format-converter core — pure compute, shared by the chat skill block and the web page.
//!
//! Converts an STL mesh between its two encodings, in EITHER direction:
//! binary STL -> ASCII STL and ASCII STL -> binary STL (plus binary->binary and
//! ascii->ascii re-writes, which normalise the solid name, facet normals and
//! number formatting). Binary STL is not text, so it is pasted in as base64 or
//! hex bytes and returned as a `data:model/stl;base64,…` URL (or raw base64 /
//! hex) that the page turns into a download.
//!
//! Geometry is carried as `f32` end to end — that is exactly what a binary STL
//! stores, so a binary -> ASCII -> binary round trip changes no bits as long as
//! the ASCII step prints enough digits (9 decimals in scientific notation is the
//! `f32` round-trip guarantee; the default 6 matches what most CAD exporters
//! write). Nothing else about the mesh is touched: no welding, no repair, no
//! scaling, no re-ordering — those belong to the stl-repair / mesh-convert
//! tools. No I/O, no external crates.

/// Hard cap, matching the other STL tools: 100000 triangles.
pub const MAX_TRIANGLES: usize = 100_000;

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

/// Which encoding to write.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    /// Convert to whichever encoding the input is NOT.
    Auto,
    Ascii,
    Binary,
}

impl Target {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "" | "flip" | "other" => Ok(Target::Auto),
            "ascii" | "text" => Ok(Target::Ascii),
            "binary" | "bin" => Ok(Target::Binary),
            other => Err(format!(
                "unknown target 'to={other}': expected 'auto', 'ascii' or 'binary'"
            )),
        }
    }
}

/// How binary output bytes are handed back (binary STL is not text).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputEncoding {
    DataUrl,
    Base64,
    Hex,
}

impl OutputEncoding {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "data-url" | "dataurl" | "data_url" | "" => Ok(OutputEncoding::DataUrl),
            "base64" | "b64" => Ok(OutputEncoding::Base64),
            "hex" => Ok(OutputEncoding::Hex),
            other => Err(format!(
                "unknown output_encoding '{other}': expected 'data-url', 'base64' or 'hex'"
            )),
        }
    }
}

/// What to do with each facet's stored normal vector.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Normals {
    /// Copy the stored normal through unchanged (lossless).
    Keep,
    /// Recompute from the triangle's own winding (right-hand rule).
    Recompute,
    /// Write 0 0 0, the "no normal declared, derive it from the winding" convention.
    Zero,
}

impl Normals {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "keep" | "" => Ok(Normals::Keep),
            "recompute" | "recalculate" => Ok(Normals::Recompute),
            "zero" | "zeroed" => Ok(Normals::Zero),
            other => Err(format!(
                "unknown normals '{other}': expected 'keep', 'recompute' or 'zero'"
            )),
        }
    }
}

/// How coordinates are written in ASCII output.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NumberFormat {
    /// `-2.648000e-002` — the sign-mantissa-e-sign-exponent form the STL spec shows.
    Scientific,
    /// `-0.002648` — plain decimals, trailing zeros trimmed.
    Decimal,
}

impl NumberFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "scientific" | "exponential" | "" => Ok(NumberFormat::Scientific),
            "decimal" | "plain" | "fixed" => Ok(NumberFormat::Decimal),
            other => Err(format!(
                "unknown number_format '{other}': expected 'scientific' or 'decimal'"
            )),
        }
    }
}

/// Return the converted mesh, or a report about the conversion.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    Stl,
    Summary,
}

impl Output {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "stl" | "" | "mesh" => Ok(Output::Stl),
            "summary" | "report" => Ok(Output::Summary),
            other => Err(format!(
                "unknown output '{other}': expected 'stl' or 'summary'"
            )),
        }
    }
}

pub struct Options {
    pub input_format: InputFormat,
    pub to: Target,
    pub output_encoding: OutputEncoding,
    pub solid_name: String,
    pub normals: Normals,
    pub precision: u32,
    pub number_format: NumberFormat,
    pub output: Output,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input_format: InputFormat::Auto,
            to: Target::Auto,
            output_encoding: OutputEncoding::DataUrl,
            solid_name: String::new(),
            normals: Normals::Keep,
            precision: 6,
            number_format: NumberFormat::Scientific,
            output: Output::Stl,
        }
    }
}

/// One STL facet: the stored normal, three corners, and the 2 "attribute byte
/// count" bytes binary STL carries per triangle (some tools stash colour there).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Facet {
    pub normal: [f32; 3],
    pub verts: [[f32; 3]; 3],
    pub attr: u16,
}

/// Which encoding the input turned out to be.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SourceEncoding {
    Ascii,
    Binary,
}

/// Everything the summary needs about the parsed input.
pub struct Parsed {
    pub facets: Vec<Facet>,
    pub encoding: SourceEncoding,
    /// Human label for the summary, e.g. `binary STL (base64, auto-detected)`.
    pub encoding_label: String,
    /// Solid name / 80-byte header text carried by the source file.
    pub name: String,
    /// Size in bytes of the DECODED source file (not of the pasted text).
    pub bytes: usize,
    /// True when the 80-byte binary header carries a Materialise Magics `COLOR=` tag.
    pub magics_color: bool,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Convert `input` and render either the STL or a conversion summary.
pub fn convert(input: &str, opt: &Options) -> Result<String, String> {
    if opt.precision > 17 {
        return Err(format!(
            "precision {} is too high: ASCII STL coordinates are 32-bit floats, so 0-17 \
             decimals is the useful range (9 already round-trips exactly)",
            opt.precision
        ));
    }
    let parsed = decode(input, opt.input_format)?;
    if parsed.facets.is_empty() {
        return Err("the STL contains no triangles — there is nothing to convert".to_string());
    }

    let target = match opt.to {
        Target::Auto => match parsed.encoding {
            SourceEncoding::Ascii => Target::Binary,
            SourceEncoding::Binary => Target::Ascii,
        },
        other => other,
    };

    let facets = apply_normals(&parsed.facets, opt.normals);
    let name = pick_name(&opt.solid_name, &parsed.name);

    match target {
        Target::Ascii => {
            let text = emit_ascii(&facets, &name, opt);
            if opt.output == Output::Summary {
                Ok(summary(&parsed, target, &name, text.len(), opt))
            } else {
                Ok(text)
            }
        }
        Target::Binary => {
            let (header, renamed) = binary_header(&name);
            let bytes = emit_binary(&facets, &header);
            if opt.output == Output::Summary {
                let mut s = summary(&parsed, target, &header, bytes.len(), opt);
                if renamed {
                    s.push_str(
                        "\nNote: a binary STL must not start with the word \"solid\" (parsers \
                         use that to tell the two encodings apart), so the 80-byte header was \
                         written as \"",
                    );
                    s.push_str(&header);
                    s.push_str("\".\n");
                }
                Ok(s)
            } else {
                Ok(match opt.output_encoding {
                    OutputEncoding::DataUrl => {
                        format!("data:model/stl;base64,{}", to_base64(&bytes))
                    }
                    OutputEncoding::Base64 => to_base64(&bytes),
                    OutputEncoding::Hex => to_hex(&bytes),
                })
            }
        }
        Target::Auto => unreachable!("resolved above"),
    }
}

/// Blank `solid_name` keeps the source's own name; a placeholder header from an
/// unreadable binary header falls back to `mesh`.
fn pick_name(requested: &str, source: &str) -> String {
    let want = requested.trim();
    let chosen = if want.is_empty() { source.trim() } else { want };
    let cleaned: String = chosen
        .chars()
        .filter(|c| !c.is_control())
        .take(72)
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "mesh".to_string()
    } else {
        cleaned.to_string()
    }
}

fn apply_normals(facets: &[Facet], mode: Normals) -> Vec<Facet> {
    facets
        .iter()
        .map(|f| match mode {
            Normals::Keep => *f,
            Normals::Zero => Facet {
                normal: [0.0, 0.0, 0.0],
                ..*f
            },
            Normals::Recompute => Facet {
                normal: geometric_normal(&f.verts),
                ..*f
            },
        })
        .collect()
}

/// Right-hand-rule normal from the winding; a degenerate triangle gets 0 0 0.
fn geometric_normal(v: &[[f32; 3]; 3]) -> [f32; 3] {
    let a = [
        v[1][0] as f64 - v[0][0] as f64,
        v[1][1] as f64 - v[0][1] as f64,
        v[1][2] as f64 - v[0][2] as f64,
    ];
    let b = [
        v[2][0] as f64 - v[0][0] as f64,
        v[2][1] as f64 - v[0][1] as f64,
        v[2][2] as f64 - v[0][2] as f64,
    ];
    let n = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len == 0.0 || !len.is_finite() {
        return [0.0, 0.0, 0.0];
    }
    [
        (n[0] / len) as f32,
        (n[1] / len) as f32,
        (n[2] / len) as f32,
    ]
}

// ---------------------------------------------------------------------------
// Decoding — ASCII text vs binary bytes (as base64 or hex)
// ---------------------------------------------------------------------------

fn decode(input: &str, fmt: InputFormat) -> Result<Parsed, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(
            "no input: paste an ASCII STL, or a binary STL's bytes as base64 or hex".to_string(),
        );
    }
    match fmt {
        InputFormat::Ascii => parse_ascii(trimmed, "ASCII STL (text)"),
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
                return parse_ascii(trimmed, "ASCII STL (text, auto-detected)");
            }
            // Hex's alphabet is a strict subset of base64's, so try hex first or
            // a hex dump would be mis-read as base64.
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

/// Binary STL headers routinely start with the word `solid` too, so the keywords
/// an ASCII STL must contain are what identifies text.
fn looks_like_ascii_stl(s: &str) -> bool {
    let head: String = s.chars().take(4096).collect::<String>().to_ascii_lowercase();
    head.contains("facet normal") || (head.contains("outer loop") && head.contains("vertex "))
}

/// Decoded bytes are normally a binary STL, but an ASCII STL that was base64'd
/// or hex-dumped still decodes cleanly — accept that rather than erroring.
fn from_bytes(bytes: &[u8], how: &str) -> Result<Parsed, String> {
    match parse_binary(bytes) {
        Ok(p) => Ok(Parsed {
            encoding_label: format!("binary STL ({how})"),
            ..p
        }),
        Err(bin_err) => {
            if let Ok(text) = std::str::from_utf8(bytes) {
                if looks_like_ascii_stl(text) {
                    return parse_ascii(text.trim(), &format!("ASCII STL ({how}-encoded)"));
                }
            }
            Err(bin_err)
        }
    }
}

fn parse_binary(bytes: &[u8]) -> Result<Parsed, String> {
    if bytes.len() < 84 {
        return Err(format!(
            "binary STL too short: {} bytes, but the 80-byte header plus the 4-byte triangle \
             count already need 84",
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
            "truncated binary STL: the header declares {count} triangles ({needed} bytes) but \
             only {} bytes were given",
            bytes.len()
        ));
    }
    let mut facets = Vec::with_capacity(count);
    for i in 0..count {
        let base = 84 + i * 50;
        let f = |k: usize| -> f32 {
            let o = base + k * 4;
            f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]])
        };
        let mut vals = [0.0f32; 12];
        for (k, val) in vals.iter_mut().enumerate() {
            *val = f(k);
        }
        if vals.iter().any(|v| !v.is_finite()) {
            return Err(format!(
                "triangle {} contains a non-finite coordinate (NaN or infinity) — the file is \
                 corrupt",
                i + 1
            ));
        }
        facets.push(Facet {
            normal: [vals[0], vals[1], vals[2]],
            verts: [
                [vals[3], vals[4], vals[5]],
                [vals[6], vals[7], vals[8]],
                [vals[9], vals[10], vals[11]],
            ],
            attr: u16::from_le_bytes([bytes[base + 48], bytes[base + 49]]),
        });
    }
    let header = &bytes[..80];
    Ok(Parsed {
        facets,
        encoding: SourceEncoding::Binary,
        encoding_label: "binary STL".to_string(),
        name: header_text(header),
        bytes: needed,
        magics_color: contains_subslice(header, b"COLOR="),
    })
}

fn contains_subslice(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// The 80-byte header as a readable label, or a placeholder when it is empty or
/// binary junk.
fn header_text(header: &[u8]) -> String {
    let text: String = header
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as char)
        .collect();
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.chars().any(|c| c.is_control()) {
        String::new()
    } else {
        trimmed.to_string()
    }
}

fn parse_ascii(text: &str, label: &str) -> Result<Parsed, String> {
    let mut facets: Vec<Facet> = Vec::new();
    let mut name = String::new();
    let mut normal = [0.0f32; 3];
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut in_facet = false;
    let mut saw_facet = false;

    for (idx, raw) in text.lines().enumerate() {
        let lineno = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("solid") && name.is_empty() && facets.is_empty() && !in_facet {
            name = line[5.min(line.len())..].trim().to_string();
        } else if let Some(rest) = lower.strip_prefix("facet normal") {
            saw_facet = true;
            in_facet = true;
            verts.clear();
            let span = line.len() - rest.len();
            normal = parse_triple(&line[span..], lineno, "facet normal")?;
        } else if lower.starts_with("facet") {
            saw_facet = true;
            in_facet = true;
            verts.clear();
            normal = [0.0, 0.0, 0.0];
        } else if let Some(rest) = lower.strip_prefix("vertex") {
            let span = line.len() - rest.len();
            let v = parse_triple(&line[span..], lineno, "vertex")?;
            if verts.len() == 3 {
                return Err(format!(
                    "line {lineno}: a facet has more than 3 vertex lines — STL triangles have \
                     exactly 3 corners"
                ));
            }
            verts.push(v);
        } else if lower.starts_with("endfacet") {
            if verts.len() != 3 {
                return Err(format!(
                    "line {lineno}: this facet has {} vertex lines, but an STL triangle needs \
                     exactly 3",
                    verts.len()
                ));
            }
            facets.push(Facet {
                normal,
                verts: [verts[0], verts[1], verts[2]],
                attr: 0,
            });
            in_facet = false;
        }
        if facets.len() > MAX_TRIANGLES {
            return Err(format!(
                "too many triangles: this tool handles up to {MAX_TRIANGLES}"
            ));
        }
    }

    if in_facet {
        return Err(
            "the last facet is missing its 'endfacet' line — the ASCII STL is truncated"
                .to_string(),
        );
    }
    if !saw_facet {
        return Err(
            "no facets found: an ASCII STL needs 'facet normal' / 'outer loop' / 'vertex' lines. \
             If this is a BINARY STL, paste its bytes as base64 or hex instead — binary STL is \
             not text."
                .to_string(),
        );
    }
    let bytes = text.len();
    Ok(Parsed {
        facets,
        encoding: SourceEncoding::Ascii,
        encoding_label: label.to_string(),
        name,
        bytes,
        magics_color: false,
    })
}

fn parse_triple(s: &str, lineno: usize, what: &str) -> Result<[f32; 3], String> {
    let nums: Vec<&str> = s.split_whitespace().collect();
    if nums.len() != 3 {
        return Err(format!(
            "line {lineno}: '{what}' needs exactly 3 numbers, found {}",
            nums.len()
        ));
    }
    let mut out = [0.0f32; 3];
    for (i, t) in nums.iter().enumerate() {
        let v: f32 = t.parse().map_err(|_| {
            format!("line {lineno}: '{what}' value '{t}' is not a number")
        })?;
        if !v.is_finite() {
            return Err(format!(
                "line {lineno}: '{what}' value '{t}' is not a finite number"
            ));
        }
        out[i] = v;
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Emitting
// ---------------------------------------------------------------------------

fn emit_ascii(facets: &[Facet], name: &str, opt: &Options) -> String {
    let p = opt.precision as usize;
    let f = opt.number_format;
    // ~7 lines x ~40 chars per facet is a close-enough starting capacity.
    let mut out = String::with_capacity(facets.len() * 280 + 32);
    out.push_str("solid ");
    out.push_str(name);
    out.push('\n');
    for facet in facets {
        out.push_str("  facet normal ");
        push_triple(&mut out, &facet.normal, p, f);
        out.push_str("\n    outer loop\n");
        for v in &facet.verts {
            out.push_str("      vertex ");
            push_triple(&mut out, v, p, f);
            out.push('\n');
        }
        out.push_str("    endloop\n  endfacet\n");
    }
    out.push_str("endsolid ");
    out.push_str(name);
    out.push('\n');
    out
}

fn push_triple(out: &mut String, v: &[f32; 3], p: usize, f: NumberFormat) {
    out.push_str(&fmt_num(v[0], p, f));
    out.push(' ');
    out.push_str(&fmt_num(v[1], p, f));
    out.push(' ');
    out.push_str(&fmt_num(v[2], p, f));
}

/// Format one coordinate. Scientific output uses the STL spec's
/// sign-mantissa-`e`-sign-3-digit-exponent shape (`2.648000e-002`).
pub fn fmt_num(v: f32, precision: usize, f: NumberFormat) -> String {
    match f {
        NumberFormat::Scientific => {
            let s = format!("{:.*e}", precision, v);
            match s.split_once('e') {
                Some((mantissa, exp)) => {
                    let (sign, digits) = match exp.strip_prefix('-') {
                        Some(d) => ('-', d),
                        None => ('+', exp),
                    };
                    format!("{mantissa}e{sign}{digits:0>3}")
                }
                None => s,
            }
        }
        NumberFormat::Decimal => {
            let s = format!("{:.*}", precision, v);
            let s = if s.contains('.') {
                let t = s.trim_end_matches('0');
                t.trim_end_matches('.').to_string()
            } else {
                s
            };
            // -0 and -0.000000 both mean zero; print one canonical form.
            if s == "-0" || s.is_empty() {
                "0".to_string()
            } else {
                s
            }
        }
    }
}

/// The 80-byte header text for binary output. Returns `(header, renamed)` —
/// `renamed` is true when the name had to be prefixed because a binary STL must
/// not begin with the ASCII word `solid`.
fn binary_header(name: &str) -> (String, bool) {
    if name.trim_start().to_ascii_lowercase().starts_with("solid") {
        let h: String = format!("STL {name}").chars().take(79).collect();
        (h, true)
    } else {
        (name.chars().take(79).collect(), false)
    }
}

fn emit_binary(facets: &[Facet], header: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(84 + facets.len() * 50);
    let mut head = [0u8; 80];
    for (i, b) in header.bytes().take(80).enumerate() {
        head[i] = b;
    }
    out.extend_from_slice(&head);
    out.extend_from_slice(&(facets.len() as u32).to_le_bytes());
    for f in facets {
        for c in f.normal {
            out.extend_from_slice(&c.to_le_bytes());
        }
        for v in &f.verts {
            for c in v {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
        out.extend_from_slice(&f.attr.to_le_bytes());
    }
    out
}

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

fn summary(parsed: &Parsed, target: Target, name: &str, out_bytes: usize, opt: &Options) -> String {
    let coloured = parsed.facets.iter().filter(|f| f.attr != 0).count();
    let viscam = parsed
        .facets
        .iter()
        .filter(|f| f.attr & 0x8000 != 0)
        .count();

    let mut s = String::from("STL format conversion\n\n");
    s.push_str(&row("Input encoding", parsed.encoding_label.clone()));
    s.push_str(&row("Input size", bytes_label(parsed.bytes)));
    s.push_str(&row("Triangles", parsed.facets.len().to_string()));
    s.push_str(&row(
        "Source name",
        if parsed.name.is_empty() {
            "(none)".to_string()
        } else {
            parsed.name.clone()
        },
    ));
    s.push_str(&row(
        "Output encoding",
        match target {
            Target::Ascii => "ASCII STL (text)".to_string(),
            Target::Binary => format!(
                "binary STL ({})",
                match opt.output_encoding {
                    OutputEncoding::DataUrl => "data:model/stl;base64 URL",
                    OutputEncoding::Base64 => "base64",
                    OutputEncoding::Hex => "hex",
                }
            ),
            Target::Auto => "auto".to_string(),
        },
    ));
    s.push_str(&row("Output name", name.to_string()));
    s.push_str(&row("Output size", bytes_label(out_bytes)));
    s.push_str(&row(
        "Size change",
        size_change(parsed.bytes, out_bytes),
    ));
    s.push_str(&row(
        "Facet normals",
        match opt.normals {
            Normals::Keep => "kept from the source file".to_string(),
            Normals::Recompute => "recomputed from each triangle's winding".to_string(),
            Normals::Zero => "written as 0 0 0".to_string(),
        },
    ));
    if target == Target::Ascii {
        s.push_str(&row(
            "Number format",
            format!(
                "{}, {} decimals",
                match opt.number_format {
                    NumberFormat::Scientific => "scientific",
                    NumberFormat::Decimal => "decimal",
                },
                opt.precision
            ),
        ));
    }
    s.push_str(&row(
        "Attribute bytes",
        attribute_label(parsed, target, coloured, viscam),
    ));
    s
}

fn attribute_label(parsed: &Parsed, target: Target, coloured: usize, viscam: usize) -> String {
    if parsed.encoding == SourceEncoding::Ascii {
        return "none — ASCII STL has no per-triangle attribute field".to_string();
    }
    let mut base = if coloured == 0 {
        "all zero (no per-triangle colour)".to_string()
    } else if viscam > 0 {
        format!(
            "{coloured} of {} triangles carry attribute bytes; {viscam} look like VisCAM/SolidView \
             15-bit colour",
            parsed.facets.len()
        )
    } else {
        format!(
            "{coloured} of {} triangles carry non-zero attribute bytes",
            parsed.facets.len()
        )
    };
    if parsed.magics_color {
        base.push_str("; the header carries a Materialise Magics COLOR= tag");
    }
    if target == Target::Ascii && (coloured > 0 || parsed.magics_color) {
        base.push_str(" — ASCII STL cannot store this, so it is dropped");
    }
    base
}

fn row(label: &str, value: String) -> String {
    format!("  {label:<17} {value}\n")
}

fn bytes_label(n: usize) -> String {
    if n >= 1024 {
        format!("{n} bytes ({:.1} KB)", n as f64 / 1024.0)
    } else {
        format!("{n} bytes")
    }
}

fn size_change(from: usize, to: usize) -> String {
    if from == 0 || to == from {
        return "unchanged".to_string();
    }
    if to > from {
        format!("{:.1}x larger", to as f64 / from as f64)
    } else {
        format!("{:.1}x smaller", from as f64 / to as f64)
    }
}

// ---------------------------------------------------------------------------
// base64 / hex (no external crates)
// ---------------------------------------------------------------------------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn to_base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

pub fn to_hex(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() * 2);
    for b in data {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        out.push(char::from_digit((b & 15) as u32, 16).unwrap());
    }
    out
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
        return Err(format!(
            "not valid hex: {} hex digits is an odd count — every byte needs two",
            nibbles.len()
        ));
    }
    Ok(nibbles.chunks(2).map(|p| (p[0] << 4) | p[1]).collect())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const TRI: &str = "solid demo\n  facet normal 0 0 1\n    outer loop\n      vertex 0 0 0\n      \
                       vertex 1 0 0\n      vertex 0 1 0\n    endloop\n  endfacet\nendsolid demo\n";

    fn opts() -> Options {
        Options::default()
    }

    /// Build a one-triangle binary STL with the given header and attribute word.
    fn bin_tri(header: &str, attr: u16) -> Vec<u8> {
        let f = Facet {
            normal: [0.0, 0.0, 1.0],
            verts: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            attr,
        };
        emit_binary(&[f], header)
    }

    #[test]
    fn ascii_to_binary_is_a_data_url_of_the_right_size() {
        let out = convert(TRI, &opts()).unwrap();
        let b64 = out
            .strip_prefix("data:model/stl;base64,")
            .expect("binary output should be a data URL");
        let bytes = decode_base64(b64).unwrap();
        // 80-byte header + 4-byte count + 50 bytes per triangle.
        assert_eq!(bytes.len(), 134);
        assert_eq!(u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]), 1);
    }

    #[test]
    fn binary_to_ascii_exact_output() {
        // The direction no other block covers: binary bytes in, readable text out.
        let bin = bin_tri("demo", 0);
        let opt = Options {
            input_format: InputFormat::Base64,
            number_format: NumberFormat::Decimal,
            ..opts()
        };
        let out = convert(&to_base64(&bin), &opt).unwrap();
        assert_eq!(
            out,
            "solid demo\n  facet normal 0 0 1\n    outer loop\n      vertex 0 0 0\n      \
             vertex 1 0 0\n      vertex 0 1 0\n    endloop\n  endfacet\nendsolid demo\n"
        );
    }

    #[test]
    fn scientific_is_the_spec_shape() {
        assert_eq!(fmt_num(0.0, 6, NumberFormat::Scientific), "0.000000e+000");
        assert_eq!(fmt_num(-1.0, 6, NumberFormat::Scientific), "-1.000000e+000");
        assert_eq!(fmt_num(0.02648, 6, NumberFormat::Scientific), "2.648000e-002");
        assert_eq!(fmt_num(-0.0, 6, NumberFormat::Decimal), "0");
        assert_eq!(fmt_num(0.5, 6, NumberFormat::Decimal), "0.5");
    }

    #[test]
    fn auto_target_flips_the_encoding_both_ways() {
        // ASCII in -> binary out.
        assert!(convert(TRI, &opts()).unwrap().starts_with("data:model/stl;base64,"));
        // Binary in -> ASCII out.
        let bin = to_base64(&bin_tri("demo", 0));
        assert!(convert(&bin, &opts()).unwrap().starts_with("solid demo\n"));
    }

    #[test]
    fn auto_detect_reads_hex_and_base64_bytes() {
        let bin = bin_tri("hexy", 0);
        for encoded in [to_hex(&bin), to_base64(&bin)] {
            let out = convert(&encoded, &opts()).unwrap();
            assert!(out.starts_with("solid hexy\n"), "got {out}");
        }
    }

    #[test]
    fn binary_round_trip_is_bit_exact_at_precision_9() {
        let bin = bin_tri("cube", 0);
        let ascii = convert(
            &to_base64(&bin),
            &Options {
                precision: 9,
                ..opts()
            },
        )
        .unwrap();
        let back = convert(
            &ascii,
            &Options {
                to: Target::Binary,
                output_encoding: OutputEncoding::Base64,
                ..opts()
            },
        )
        .unwrap();
        let bytes = decode_base64(&back).unwrap();
        // Header text is carried through, so the whole file matches byte for byte.
        assert_eq!(bytes, bin);
    }

    #[test]
    fn odd_coordinates_survive_the_round_trip_at_precision_9() {
        let f = Facet {
            normal: [0.577_350_3, -0.577_350_3, 0.577_350_3],
            verts: [
                [1.234_567_9, -0.000_012_345, 98_765.43],
                [-3.141_592_7, 2.718_281_8, 1.414_213_6],
                [0.1, 0.2, 0.3],
            ],
            attr: 0,
        };
        let bin = emit_binary(&[f], "odd");
        let ascii = convert(
            &to_base64(&bin),
            &Options {
                precision: 9,
                ..opts()
            },
        )
        .unwrap();
        let back = convert(
            &ascii,
            &Options {
                to: Target::Binary,
                output_encoding: OutputEncoding::Base64,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(decode_base64(&back).unwrap(), bin);
    }

    #[test]
    fn binary_header_never_starts_with_solid() {
        let out = convert(
            TRI,
            &Options {
                solid_name: "solidus".to_string(),
                output_encoding: OutputEncoding::Base64,
                ..opts()
            },
        )
        .unwrap();
        let bytes = decode_base64(&out).unwrap();
        let head = String::from_utf8_lossy(&bytes[..12]).to_string();
        assert!(head.starts_with("STL solidus"), "header was {head:?}");
    }

    #[test]
    fn normals_modes_change_only_the_normal() {
        let bad = "solid m\n facet normal 9 9 9\n outer loop\n vertex 0 0 0\n vertex 1 0 0\n \
                   vertex 0 1 0\n endloop\n endfacet\nendsolid m\n";
        let base = Options {
            to: Target::Ascii,
            number_format: NumberFormat::Decimal,
            ..opts()
        };
        assert!(convert(bad, &base).unwrap().contains("facet normal 9 9 9"));
        assert!(convert(
            bad,
            &Options {
                normals: Normals::Recompute,
                ..Options {
                    to: Target::Ascii,
                    number_format: NumberFormat::Decimal,
                    ..opts()
                }
            }
        )
        .unwrap()
        .contains("facet normal 0 0 1"));
        assert!(convert(
            bad,
            &Options {
                normals: Normals::Zero,
                ..Options {
                    to: Target::Ascii,
                    number_format: NumberFormat::Decimal,
                    ..opts()
                }
            }
        )
        .unwrap()
        .contains("facet normal 0 0 0"));
    }

    #[test]
    fn attribute_bytes_survive_a_binary_to_binary_rewrite() {
        let bin = bin_tri("coloured", 0x8000 | (31 << 10));
        let out = convert(
            &to_base64(&bin),
            &Options {
                to: Target::Binary,
                output_encoding: OutputEncoding::Base64,
                ..opts()
            },
        )
        .unwrap();
        let bytes = decode_base64(&out).unwrap();
        assert_eq!(u16::from_le_bytes([bytes[132], bytes[133]]), 0x8000 | (31 << 10));
    }

    #[test]
    fn summary_reports_dropped_colour_on_the_way_to_ascii() {
        let bin = bin_tri("coloured", 0x8000 | 31);
        let out = convert(
            &to_base64(&bin),
            &Options {
                output: Output::Summary,
                ..opts()
            },
        )
        .unwrap();
        assert!(out.contains("Triangles         1"), "got {out}");
        assert!(out.contains("VisCAM/SolidView"), "got {out}");
        assert!(out.contains("it is dropped"), "got {out}");
        assert!(out.contains("Output encoding   ASCII STL (text)"), "got {out}");
    }

    #[test]
    fn summary_shows_the_size_change_for_ascii_to_binary() {
        let out = convert(
            TRI,
            &Options {
                output: Output::Summary,
                ..opts()
            },
        )
        .unwrap();
        assert!(out.contains("Output size       134 bytes"), "got {out}");
        assert!(out.contains("smaller"), "got {out}");
    }

    #[test]
    fn hex_output_encoding_is_raw_hex() {
        let out = convert(
            TRI,
            &Options {
                output_encoding: OutputEncoding::Hex,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out.len(), 134 * 2);
        assert!(out.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn a_binary_stl_whose_header_says_solid_is_not_mistaken_for_text() {
        let bin = bin_tri("solid exported by CAD", 0);
        let out = convert(&to_base64(&bin), &opts()).unwrap();
        assert!(out.starts_with("solid solid exported by CAD\n"), "got {out}");
    }

    #[test]
    fn blank_name_keeps_the_source_name_and_a_set_name_overrides_it() {
        let opt = Options {
            to: Target::Ascii,
            ..opts()
        };
        assert!(convert(TRI, &opt).unwrap().starts_with("solid demo\n"));
        let renamed = convert(
            TRI,
            &Options {
                solid_name: "part-a".to_string(),
                ..Options {
                    to: Target::Ascii,
                    ..opts()
                }
            },
        )
        .unwrap();
        assert!(renamed.starts_with("solid part-a\n"));
        assert!(renamed.trim_end().ends_with("endsolid part-a"));
    }

    // --- error paths -------------------------------------------------------

    #[test]
    fn a_binary_stl_pasted_as_text_gets_an_actionable_error() {
        let err = convert("\u{0}\u{1}\u{2} not a mesh", &Options {
            input_format: InputFormat::Ascii,
            ..opts()
        })
        .unwrap_err();
        assert!(err.contains("paste its bytes as base64 or hex"), "got {err}");
    }

    #[test]
    fn truncated_binary_is_reported_with_both_sizes() {
        let mut bin = bin_tri("demo", 0);
        bin.truncate(100);
        let err = convert(
            &to_base64(&bin),
            &Options {
                input_format: InputFormat::Base64,
                ..opts()
            },
        )
        .unwrap_err();
        assert!(err.contains("truncated binary STL"), "got {err}");
        assert!(err.contains("134 bytes"), "got {err}");
    }

    #[test]
    fn a_facet_with_two_vertices_is_rejected() {
        let bad = "solid m\n facet normal 0 0 1\n outer loop\n vertex 0 0 0\n vertex 1 0 0\n \
                   endloop\n endfacet\nendsolid m\n";
        let err = convert(bad, &opts()).unwrap_err();
        assert!(err.contains("needs exactly 3"), "got {err}");
    }

    #[test]
    fn a_non_numeric_vertex_names_the_line() {
        let bad = "solid m\n facet normal 0 0 1\n outer loop\n vertex a b c\n vertex 1 0 0\n \
                   vertex 0 1 0\n endloop\n endfacet\nendsolid m\n";
        let err = convert(bad, &opts()).unwrap_err();
        assert!(err.contains("line 4"), "got {err}");
        assert!(err.contains("is not a number"), "got {err}");
    }

    #[test]
    fn empty_input_is_rejected() {
        assert!(convert("   ", &opts()).unwrap_err().contains("no input"));
    }

    #[test]
    fn precision_above_17_is_rejected() {
        let err = convert(
            TRI,
            &Options {
                precision: 25,
                ..opts()
            },
        )
        .unwrap_err();
        assert!(err.contains("too high"), "got {err}");
    }

    #[test]
    fn too_many_triangles_is_rejected() {
        // Claim 200000 triangles in the header without supplying the bytes: the
        // count cap must fire before the truncation check.
        let mut bin = bin_tri("big", 0);
        bin[80..84].copy_from_slice(&200_000u32.to_le_bytes());
        let err = convert(&to_base64(&bin), &opts()).unwrap_err();
        assert!(err.contains("too many triangles"), "got {err}");
    }

    #[test]
    fn parsers_reject_unknown_option_values() {
        assert!(InputFormat::parse("yaml").is_err());
        assert!(Target::parse("obj").is_err());
        assert!(OutputEncoding::parse("gzip").is_err());
        assert!(Normals::parse("smooth").is_err());
        assert!(NumberFormat::parse("roman").is_err());
        assert!(Output::parse("csv").is_err());
    }
}
