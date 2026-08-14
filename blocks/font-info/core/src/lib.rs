//! font-info core — inspect a TTF/OTF/WOFF/WOFF2 font and report its name-table
//! metadata, glyph count, vertical metrics, style classes, embedding
//! permissions (OS/2 `fsType`) and table directory.
//!
//! Pipeline: detect the container from the leading 4 bytes → normalise WOFF /
//! WOFF2 back to raw SFNT (`wuff`, pure Rust, handles both `glyf` and CFF
//! fonts) → parse the SFNT with `ttf-parser` and walk the raw table directory
//! for the table list and the raw `fsType` bits.
//!
//! Everything runs on byte slices — no filesystem, no network — so it builds
//! and instantiates under wasm32-wasip1 / wasmi. Read-only: the input bytes are
//! never modified.

use serde::Serialize;
use ttf_parser::{name_id, Face, PlatformId, Tag};

const SIG_WOFF: u32 = 0x774F_4646; // 'wOFF'
const SIG_WOFF2: u32 = 0x774F_4632; // 'wOF2'
const FLAVOR_TRUETYPE: u32 = 0x0001_0000;
const FLAVOR_TRUE: u32 = 0x7472_7565; // 'true' (Apple TrueType)
const FLAVOR_OTTO: u32 = 0x4F54_544F; // 'OTTO' (CFF OpenType)
const FLAVOR_TTCF: u32 = 0x7474_6366; // 'ttcf' (font collection)

/// `name` table strings, one field per OpenType name ID. Every field is
/// omitted from the JSON when the font doesn't carry that record.
#[derive(Serialize, Debug, Default, PartialEq)]
pub struct FontNames {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subfamily: Option<String>,
    /// nameID 16 — the "typographic"/preferred family (set on fonts whose
    /// family has more than the four RIBBI styles).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typographic_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typographic_subfamily: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postscript_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unique_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trademark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub designer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub designer_url: Option<String>,
    /// nameID 13 — the embedded licence description (often the full licence
    /// text, e.g. the SIL OFL preamble).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_text: Option<String>,
}

/// Vertical metrics, in font design units (divide by `units_per_em`).
#[derive(Serialize, Debug, PartialEq)]
pub struct Metrics {
    pub units_per_em: u16,
    pub ascender: i16,
    pub descender: i16,
    pub line_gap: i16,
    /// `ascender - descender + line_gap` — the default line height.
    pub height: i16,
    /// OS/2 typographic (sTypo*) metrics, when the OS/2 table is present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typographic_ascender: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typographic_descender: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typographic_line_gap: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_height: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap_height: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline_position: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline_thickness: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strikeout_position: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strikeout_thickness: Option<i16>,
}

/// The global glyph bounding box (`head.xMin/yMin/xMax/yMax`), design units.
#[derive(Serialize, Debug, PartialEq)]
pub struct BoundingBox {
    pub x_min: i16,
    pub y_min: i16,
    pub x_max: i16,
    pub y_max: i16,
}

/// Style classification from OS/2 + `head.macStyle`.
#[derive(Serialize, Debug, PartialEq)]
pub struct Style {
    /// OS/2 `usWeightClass`, 1–1000 (400 = Regular, 700 = Bold).
    pub weight_class: u16,
    /// Nearest standard CSS weight name, e.g. `Regular`, `Bold`, `Black`.
    pub weight_name: String,
    /// OS/2 `usWidthClass`, 1–9 (5 = Normal).
    pub width_class: u16,
    pub width_name: &'static str,
    /// `Normal`, `Italic` or `Oblique`.
    pub slope: &'static str,
    pub is_bold: bool,
    pub is_italic: bool,
    pub is_oblique: bool,
    pub is_regular: bool,
    pub is_monospaced: bool,
    /// True when the font carries an `fvar` table (a variable font).
    pub is_variable: bool,
    /// `post.italicAngle`, degrees counter-clockwise from vertical.
    pub italic_angle: f32,
}

/// OS/2 `fsType` — what the font's own metadata permits for embedding.
#[derive(Serialize, Debug, PartialEq)]
pub struct Embedding {
    /// Raw `fsType` bit field, e.g. `0x0000`.
    pub fs_type: String,
    /// `Installable`, `Restricted`, `Preview & Print`, `Editable`, or
    /// `Unknown` when the bits are malformed (mutually exclusive from v3 on).
    pub permission: &'static str,
    /// Plain-English reading of the permission bits.
    pub explanation: &'static str,
    /// `fsType` bit 8 clear — a subset of the font may be embedded.
    pub subsetting_allowed: bool,
    /// `fsType` bit 9 clear — outlines (not just bitmaps) may be embedded.
    pub outline_embedding_allowed: bool,
}

/// One entry of the SFNT table directory.
#[derive(Serialize, Debug, PartialEq)]
pub struct TableEntry {
    pub tag: String,
    /// Uncompressed length in bytes (the SFNT length, post WOFF/WOFF2 decode).
    pub length: u32,
}

/// One `cmap` character-to-glyph subtable.
#[derive(Serialize, Debug, PartialEq)]
pub struct CmapSubtable {
    pub platform: &'static str,
    pub encoding_id: u16,
    /// `cmap` subtable format number (0, 2, 4, 6, 8, 10, 12, 13 or 14).
    pub format: u8,
    pub is_unicode: bool,
}

/// One `fvar` variation axis of a variable font.
#[derive(Serialize, Debug, PartialEq)]
pub struct VariationAxis {
    pub tag: String,
    pub min: f32,
    pub default: f32,
    pub max: f32,
    pub hidden: bool,
}

/// Everything `inspect` reports about a font.
#[derive(Serialize, Debug, PartialEq)]
pub struct FontInfo {
    /// Detected input container: `TTF`, `OTF`, `WOFF` or `WOFF2`.
    pub input_format: &'static str,
    /// Glyph outline technology: `TrueType (glyf)` or `PostScript/CFF`.
    pub outline: &'static str,
    /// `sfnt` flavor tag of the decoded font: `0x00010000`, `true` or `OTTO`.
    pub sfnt_version: String,
    /// Size of the submitted file in bytes.
    pub input_size: usize,
    /// Size of the decoded SFNT in bytes (equals `input_size` for TTF/OTF).
    pub sfnt_size: usize,
    pub names: FontNames,
    pub glyph_count: u16,
    pub metrics: Metrics,
    pub bounding_box: BoundingBox,
    pub style: Style,
    pub embedding: Embedding,
    pub table_count: usize,
    pub tables: Vec<TableEntry>,
    pub cmap_subtables: Vec<CmapSubtable>,
    /// Number of distinct Unicode code points mapped by the `cmap` table.
    pub mapped_code_points: u32,
    /// `fvar` axes; empty for a static font.
    pub variation_axes: Vec<VariationAxis>,
    /// True when the font ships colour glyphs (`COLR`, `CBDT`, `sbix` or `SVG`).
    pub has_color_glyphs: bool,
    /// True when the font ships hinting programs (`fpgm`/`prep`/`cvt `).
    pub has_hinting: bool,
    /// True when the font ships OpenType layout features (`GSUB`/`GPOS`).
    pub has_opentype_layout: bool,
}

fn be_u32(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off + 4)
        .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn be_u16(b: &[u8], off: usize) -> Option<u16> {
    b.get(off..off + 2)
        .map(|s| u16::from_be_bytes([s[0], s[1]]))
}

/// Detect the input container from its signature and return a human label.
fn detect_input(bytes: &[u8]) -> Result<&'static str, String> {
    let sig = be_u32(bytes, 0).ok_or("input is too short to be a font file")?;
    match sig {
        SIG_WOFF2 => Ok("WOFF2"),
        SIG_WOFF => Ok("WOFF"),
        FLAVOR_TRUETYPE | FLAVOR_TRUE => Ok("TTF"),
        FLAVOR_OTTO => Ok("OTF"),
        FLAVOR_TTCF => Err(
            "font collections (.ttc/.otc) are not supported — extract a single font first"
                .to_string(),
        ),
        _ => Err(format!(
            "unrecognised font format (leading bytes 0x{sig:08X}); \
             expected TTF, OTF, WOFF or WOFF2"
        )),
    }
}

/// Normalise any supported input to raw SFNT bytes (decoding WOFF/WOFF2).
fn to_sfnt(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let sig = be_u32(bytes, 0).ok_or("input is too short to be a font file")?;
    match sig {
        SIG_WOFF2 => wuff::decompress_woff2(bytes).map_err(|e| {
            format!(
                "could not decode WOFF2 (it may use the rarer hmtx transform, \
                 which is unsupported): {e:?}"
            )
        }),
        SIG_WOFF => {
            wuff::decompress_woff1(bytes).map_err(|e| format!("could not decode WOFF: {e:?}"))
        }
        _ => Ok(bytes.to_vec()),
    }
}

/// Read the SFNT table directory: `(flavor, [(tag, length)])` in directory order.
fn table_directory(sfnt: &[u8]) -> Result<(u32, Vec<TableEntry>), String> {
    let flavor = be_u32(sfnt, 0).ok_or("truncated SFNT header")?;
    let num_tables = be_u16(sfnt, 4).ok_or("truncated SFNT header")? as usize;
    if num_tables == 0 {
        return Err("font has no tables".to_string());
    }
    let mut tables = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let base = 12 + i * 16;
        let tag = sfnt
            .get(base..base + 4)
            .ok_or("truncated SFNT table directory")?;
        let length = be_u32(sfnt, base + 12).ok_or("truncated SFNT table directory")?;
        tables.push(TableEntry {
            tag: String::from_utf8_lossy(tag).to_string(),
            length,
        });
    }
    Ok((flavor, tables))
}

/// How much we prefer a given `name` record for a name ID. Windows/US-English
/// first (what every other tool shows), then any Windows or Unicode record,
/// then Mac English — so the choice is stable across runs.
fn name_priority(platform_id: PlatformId, language_id: u16) -> u8 {
    match platform_id {
        PlatformId::Windows if language_id == 0x0409 => 4,
        PlatformId::Windows => 3,
        PlatformId::Unicode => 2,
        PlatformId::Macintosh if language_id == 0 => 1,
        _ => 0,
    }
}

/// Decode a name record. `ttf-parser` only decodes the UTF-16BE (Unicode)
/// records; Mac Roman records are ASCII in practice, so decode those by hand
/// rather than dropping a font's only copy of a string.
fn name_string(name: &ttf_parser::name::Name<'_>) -> Option<String> {
    if let Some(s) = name.to_string() {
        return Some(s);
    }
    if name.platform_id == PlatformId::Macintosh
        && name.encoding_id == 0
        && name.name.iter().all(|b| b.is_ascii())
    {
        return Some(String::from_utf8_lossy(name.name).to_string());
    }
    None
}

/// Collapse control characters/whitespace runs so a multi-line licence string
/// stays one readable JSON value.
fn tidy(s: String) -> Option<String> {
    let joined = s
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

fn collect_names(face: &Face<'_>) -> FontNames {
    // (name_id -> (priority, value)); a higher-priority record wins.
    let mut best: Vec<(u16, u8, String)> = Vec::new();
    for name in face.names() {
        let Some(value) = name_string(&name).and_then(tidy) else {
            continue;
        };
        let priority = name_priority(name.platform_id, name.language_id);
        match best.iter_mut().find(|(id, _, _)| *id == name.name_id) {
            Some(slot) if priority > slot.1 => {
                slot.1 = priority;
                slot.2 = value;
            }
            Some(_) => {}
            None => best.push((name.name_id, priority, value)),
        }
    }
    let get = |id: u16| {
        best.iter()
            .find(|(i, _, _)| *i == id)
            .map(|(_, _, v)| v.clone())
    };
    FontNames {
        family: get(name_id::FAMILY),
        subfamily: get(name_id::SUBFAMILY),
        typographic_family: get(name_id::TYPOGRAPHIC_FAMILY),
        typographic_subfamily: get(name_id::TYPOGRAPHIC_SUBFAMILY),
        full_name: get(name_id::FULL_NAME),
        postscript_name: get(name_id::POST_SCRIPT_NAME),
        version: get(name_id::VERSION),
        unique_id: get(name_id::UNIQUE_ID),
        description: get(name_id::DESCRIPTION),
        copyright: get(name_id::COPYRIGHT_NOTICE),
        trademark: get(name_id::TRADEMARK),
        manufacturer: get(name_id::MANUFACTURER),
        designer: get(name_id::DESIGNER),
        vendor_url: get(name_id::VENDOR_URL),
        designer_url: get(name_id::DESIGNER_URL),
        license: get(name_id::LICENSE),
        license_url: get(name_id::LICENSE_URL),
        sample_text: get(name_id::SAMPLE_TEXT),
    }
}

/// Nearest CSS weight name for an OS/2 `usWeightClass`.
fn weight_name(n: u16) -> String {
    match n {
        100 => "Thin".to_string(),
        200 => "Extra Light".to_string(),
        300 => "Light".to_string(),
        400 => "Regular".to_string(),
        500 => "Medium".to_string(),
        600 => "Semi Bold".to_string(),
        700 => "Bold".to_string(),
        800 => "Extra Bold".to_string(),
        900 => "Black".to_string(),
        other => format!("Custom ({other})"),
    }
}

fn width_name(n: u16) -> &'static str {
    match n {
        1 => "Ultra Condensed",
        2 => "Extra Condensed",
        3 => "Condensed",
        4 => "Semi Condensed",
        5 => "Normal",
        6 => "Semi Expanded",
        7 => "Expanded",
        8 => "Extra Expanded",
        9 => "Ultra Expanded",
        _ => "Unknown",
    }
}

/// Read the raw OS/2 `fsType` bit field (offset 8 in the OS/2 table).
fn raw_fs_type(face: &Face<'_>) -> Option<u16> {
    let os2 = face.table_data(Tag::from_bytes(b"OS/2"))?;
    be_u16(os2, 8)
}

fn embedding(face: &Face<'_>) -> Embedding {
    use ttf_parser::Permissions;
    let fs_type = raw_fs_type(face);
    let (permission, explanation) = match face.permissions() {
        Some(Permissions::Installable) => (
            "Installable",
            "The font may be embedded and permanently installed on the receiving device.",
        ),
        Some(Permissions::Restricted) => (
            "Restricted",
            "Restricted licence: the font must not be embedded, modified or exchanged \
             without the vendor's permission.",
        ),
        Some(Permissions::PreviewAndPrint) => (
            "Preview & Print",
            "The font may be embedded and temporarily loaded to view or print the \
             document, but the document must not be edited with it.",
        ),
        Some(Permissions::Editable) => (
            "Editable",
            "The font may be embedded and temporarily loaded, and the document may be \
             edited with it on the receiving device.",
        ),
        None if fs_type.is_none() => (
            "Unspecified",
            "The font has no OS/2 table, so it declares no embedding permission.",
        ),
        None => (
            "Unknown",
            "The fsType bits are malformed (from OS/2 version 3 the permission bits are \
             mutually exclusive) — check the font's own licence.",
        ),
    };
    Embedding {
        fs_type: fs_type.map_or_else(|| "none".to_string(), |v| format!("0x{v:04X}")),
        permission,
        explanation,
        subsetting_allowed: face.is_subsetting_allowed(),
        outline_embedding_allowed: face.is_outline_embedding_allowed(),
    }
}

fn cmap_format_number(format: &ttf_parser::cmap::Format<'_>) -> u8 {
    use ttf_parser::cmap::Format as F;
    match format {
        F::ByteEncodingTable(_) => 0,
        F::HighByteMappingThroughTable(_) => 2,
        F::SegmentMappingToDeltaValues(_) => 4,
        F::TrimmedTableMapping(_) => 6,
        F::MixedCoverage => 8,
        F::TrimmedArray(_) => 10,
        F::SegmentedCoverage(_) => 12,
        F::ManyToOneRangeMappings(_) => 13,
        F::UnicodeVariationSequences(_) => 14,
    }
}

fn platform_name(platform_id: PlatformId) -> &'static str {
    match platform_id {
        PlatformId::Unicode => "Unicode",
        PlatformId::Macintosh => "Macintosh",
        PlatformId::Iso => "ISO",
        PlatformId::Windows => "Windows",
        PlatformId::Custom => "Custom",
    }
}

/// `cmap` subtables plus the number of distinct code points they map. Counted
/// in a 1.1 M-bit bitset rather than a set, so a large CJK font stays cheap.
fn cmap_summary(face: &Face<'_>) -> (Vec<CmapSubtable>, u32) {
    let Some(cmap) = face.tables().cmap else {
        return (Vec::new(), 0);
    };
    let mut seen = vec![0u64; (0x11_0000 + 63) / 64];
    let mut subtables = Vec::new();
    for subtable in cmap.subtables {
        subtables.push(CmapSubtable {
            platform: platform_name(subtable.platform_id),
            encoding_id: subtable.encoding_id,
            format: cmap_format_number(&subtable.format),
            is_unicode: subtable.is_unicode(),
        });
        if !subtable.is_unicode() {
            continue;
        }
        subtable.codepoints(|cp| {
            if cp < 0x11_0000 {
                seen[(cp / 64) as usize] |= 1u64 << (cp % 64);
            }
        });
    }
    let count = seen.iter().map(|w| w.count_ones()).sum();
    (subtables, count)
}

/// Inspect a TTF/OTF/WOFF/WOFF2 font and report everything about it.
pub fn inspect(bytes: &[u8]) -> Result<FontInfo, String> {
    let input_format = detect_input(bytes)?;
    let sfnt = to_sfnt(bytes)?;
    let (flavor, tables) = table_directory(&sfnt)?;

    let face = Face::parse(&sfnt, 0).map_err(|e| format!("could not parse the font: {e}"))?;

    let outline = if face.tables().cff.is_some() {
        "PostScript/CFF"
    } else {
        "TrueType (glyf)"
    };
    let sfnt_version = match flavor {
        FLAVOR_TRUETYPE => "0x00010000".to_string(),
        FLAVOR_TRUE => "true".to_string(),
        FLAVOR_OTTO => "OTTO".to_string(),
        other => format!("0x{other:08X}"),
    };

    let underline = face.underline_metrics();
    let strikeout = face.strikeout_metrics();
    let bbox = face.global_bounding_box();
    let has_table = |tag: &[u8; 4]| tables.iter().any(|t| t.tag.as_bytes() == tag);

    let (cmap_subtables, mapped_code_points) = cmap_summary(&face);

    Ok(FontInfo {
        input_format,
        outline,
        sfnt_version,
        input_size: bytes.len(),
        sfnt_size: sfnt.len(),
        names: collect_names(&face),
        glyph_count: face.number_of_glyphs(),
        metrics: Metrics {
            units_per_em: face.units_per_em(),
            ascender: face.ascender(),
            descender: face.descender(),
            line_gap: face.line_gap(),
            height: face.height(),
            typographic_ascender: face.typographic_ascender(),
            typographic_descender: face.typographic_descender(),
            typographic_line_gap: face.typographic_line_gap(),
            x_height: face.x_height(),
            cap_height: face.capital_height(),
            underline_position: underline.map(|m| m.position),
            underline_thickness: underline.map(|m| m.thickness),
            strikeout_position: strikeout.map(|m| m.position),
            strikeout_thickness: strikeout.map(|m| m.thickness),
        },
        bounding_box: BoundingBox {
            x_min: bbox.x_min,
            y_min: bbox.y_min,
            x_max: bbox.x_max,
            y_max: bbox.y_max,
        },
        style: Style {
            weight_class: face.weight().to_number(),
            weight_name: weight_name(face.weight().to_number()),
            width_class: face.width().to_number(),
            width_name: width_name(face.width().to_number()),
            slope: match face.style() {
                ttf_parser::Style::Normal => "Normal",
                ttf_parser::Style::Italic => "Italic",
                ttf_parser::Style::Oblique => "Oblique",
            },
            is_bold: face.is_bold(),
            is_italic: face.is_italic(),
            is_oblique: face.is_oblique(),
            is_regular: face.is_regular(),
            is_monospaced: face.is_monospaced(),
            is_variable: face.is_variable(),
            italic_angle: face.italic_angle(),
        },
        embedding: embedding(&face),
        table_count: tables.len(),
        cmap_subtables,
        mapped_code_points,
        variation_axes: face
            .variation_axes()
            .into_iter()
            .map(|a| VariationAxis {
                tag: a.tag.to_string(),
                min: a.min_value,
                default: a.def_value,
                max: a.max_value,
                hidden: a.hidden,
            })
            .collect(),
        has_color_glyphs: has_table(b"COLR")
            || has_table(b"CBDT")
            || has_table(b"sbix")
            || has_table(b"SVG "),
        has_hinting: has_table(b"fpgm") || has_table(b"prep") || has_table(b"cvt "),
        has_opentype_layout: has_table(b"GSUB") || has_table(b"GPOS"),
        tables,
    })
}

/// `inspect` rendered as deterministic pretty JSON (field order is the struct's).
pub fn inspect_json(bytes: &[u8]) -> Result<String, String> {
    let info = inspect(bytes)?;
    serde_json::to_string_pretty(&info).map_err(|e| format!("serialize font-info report: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TTF: &[u8] = include_bytes!("../tests/fixtures/sample.ttf");
    const OTF: &[u8] = include_bytes!("../tests/fixtures/sample.otf");
    const TTF_WOFF: &[u8] = include_bytes!("../tests/fixtures/sample_ttf.woff");
    const TTF_WOFF2: &[u8] = include_bytes!("../tests/fixtures/sample_ttf.woff2");
    const OTF_WOFF2: &[u8] = include_bytes!("../tests/fixtures/sample_otf.woff2");

    #[test]
    fn inspects_a_truetype_font() {
        let info = inspect(TTF).unwrap();
        assert_eq!(info.input_format, "TTF");
        assert_eq!(info.outline, "TrueType (glyf)");
        assert_eq!(info.sfnt_version, "0x00010000");
        assert_eq!(info.input_size, TTF.len());
        // A raw SFNT input is passed through untouched.
        assert_eq!(info.sfnt_size, TTF.len());
        assert_eq!(info.names.family.as_deref(), Some("Gizza Sample"));
        assert_eq!(info.names.subfamily.as_deref(), Some("Regular"));
        assert_eq!(info.glyph_count, 4);
        assert_eq!(info.metrics.units_per_em, 1000);
        assert_eq!(info.metrics.ascender, 800);
        assert_eq!(info.metrics.descender, -200);
        assert_eq!(info.metrics.line_gap, 0);
        assert_eq!(info.metrics.height, 1000);
        assert_eq!(info.style.weight_class, 400);
        assert_eq!(info.style.weight_name, "Regular");
        assert_eq!(info.style.width_class, 5);
        assert_eq!(info.style.width_name, "Normal");
        assert_eq!(info.style.slope, "Normal");
        assert!(!info.style.is_bold);
        assert!(!info.style.is_variable);
        assert_eq!(info.table_count, info.tables.len());
        assert!(info.tables.iter().any(|t| t.tag == "glyf"));
        assert!(info.tables.iter().any(|t| t.tag == "OS/2"));
        assert!(info.variation_axes.is_empty());
        assert!(!info.has_opentype_layout);
    }

    #[test]
    fn reports_embedding_permissions() {
        let info = inspect(TTF).unwrap();
        assert_eq!(info.embedding.fs_type, "0x0004");
        assert_eq!(info.embedding.permission, "Preview & Print");
        assert!(info.embedding.explanation.contains("view or print"));
        assert!(info.embedding.subsetting_allowed);
        assert!(info.embedding.outline_embedding_allowed);
    }

    #[test]
    fn reports_cmap_coverage() {
        let info = inspect(TTF).unwrap();
        assert!(!info.cmap_subtables.is_empty());
        assert!(info.cmap_subtables.iter().all(|s| s.format == 4));
        assert!(info.cmap_subtables.iter().any(|s| s.is_unicode));
        // The fixture maps A-C onto its outline glyphs.
        assert_eq!(info.mapped_code_points, 3);
    }

    #[test]
    fn detects_a_postscript_cff_font() {
        let info = inspect(OTF).unwrap();
        assert_eq!(info.input_format, "OTF");
        assert_eq!(info.outline, "PostScript/CFF");
        assert_eq!(info.sfnt_version, "OTTO");
        assert!(info.tables.iter().any(|t| t.tag == "CFF "));
        assert!(!info.tables.iter().any(|t| t.tag == "glyf"));
    }

    #[test]
    fn woff_and_woff2_report_the_same_font_as_the_ttf() {
        let ttf = inspect(TTF).unwrap();
        for (bytes, label) in [(TTF_WOFF, "WOFF"), (TTF_WOFF2, "WOFF2")] {
            let info = inspect(bytes).unwrap();
            assert_eq!(info.input_format, label);
            assert_eq!(info.outline, ttf.outline);
            assert_eq!(info.names.family, ttf.names.family);
            assert_eq!(info.glyph_count, ttf.glyph_count);
            assert_eq!(info.metrics, ttf.metrics);
            assert_eq!(info.bounding_box, ttf.bounding_box);
            // The container is compressed, so the file is smaller than the SFNT
            // it decodes back to.
            assert_eq!(info.input_size, bytes.len());
            assert!(info.sfnt_size >= info.input_size);
        }
    }

    #[test]
    fn woff2_cff_font_is_decoded() {
        let info = inspect(OTF_WOFF2).unwrap();
        assert_eq!(info.input_format, "WOFF2");
        assert_eq!(info.outline, "PostScript/CFF");
        assert_eq!(info.names.family.as_deref(), Some("Gizza Sample CFF"));
    }

    #[test]
    fn json_is_pretty_and_deterministic() {
        let first = inspect_json(TTF).unwrap();
        let second = inspect_json(TTF).unwrap();
        assert_eq!(first, second);
        assert!(first.starts_with("{\n  \"input_format\": \"TTF\","));
        // Absent name records are omitted rather than serialized as null.
        assert!(!first.contains("null"));
    }

    #[test]
    fn rejects_a_font_collection() {
        let mut ttc = b"ttcf".to_vec();
        ttc.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 2]);
        let err = inspect(&ttc).unwrap_err();
        assert!(err.contains("font collections"), "got: {err}");
    }

    #[test]
    fn rejects_a_non_font() {
        let err = inspect(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n").unwrap_err();
        assert!(err.contains("unrecognised font format"), "got: {err}");
        assert!(
            err.contains("expected TTF, OTF, WOFF or WOFF2"),
            "got: {err}"
        );
    }

    #[test]
    fn rejects_a_truncated_input() {
        let err = inspect(b"ab").unwrap_err();
        assert!(err.contains("too short"), "got: {err}");
    }
}
