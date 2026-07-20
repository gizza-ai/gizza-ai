//! gizza-ai/woff2-convert — convert a font between the TTF, OTF, WOFF and WOFF2
//! container formats.
//!
//! The input container is auto-detected from its leading bytes. Any WOFF/WOFF2
//! input is first decoded back to its underlying SFNT (`wuff`, pure Rust, handles
//! both TrueType `glyf` and PostScript/CFF fonts). The SFNT is then re-encoded to
//! the requested target:
//!
//! * **woff2** — `ttf2woff2` (glyf/loca transform + Brotli, smallest output) for
//!   TrueType fonts; a hand-rolled null-transform WOFF2 writer (Brotli, no glyf
//!   transform) for CFF/OTF fonts and any non-glyf SFNT.
//! * **woff**  — a hand-rolled WOFF v1 writer (per-table zlib via `flate2`).
//! * **ttf / otf** — the decoded SFNT bytes (decompress a web font to a desktop
//!   font). The glyph outline technology is PRESERVED — this tool wraps/unwraps
//!   and (de)compresses containers, it does not re-outline glyphs (glyf ↔ CFF
//!   conversion is out of scope), so the ttf/otf choice sets the file container.
//!
//! Everything runs on byte slices — no filesystem, no network — so it builds and
//! instantiates under wasm32-wasip1 / wasmi.

const SIG_WOFF: u32 = 0x774F_4646; // 'wOFF'
const SIG_WOFF2: u32 = 0x774F_4632; // 'wOF2'
const FLAVOR_TRUETYPE: u32 = 0x0001_0000;
const FLAVOR_TRUE: u32 = 0x7472_7565; // 'true' (Apple TrueType)
const FLAVOR_OTTO: u32 = 0x4F54_544F; // 'OTTO' (CFF OpenType)
const FLAVOR_TTCF: u32 = 0x7474_6366; // 'ttcf' (font collection)

/// Result of a conversion: the output bytes plus everything the caller needs to
/// label the download and describe what happened.
#[derive(Debug)]
pub struct Conversion {
    pub bytes: Vec<u8>,
    /// Detected input container: "TTF", "OTF", "WOFF" or "WOFF2".
    pub input_format: &'static str,
    /// Requested output container: "WOFF2", "WOFF", "TTF" or "OTF".
    pub output_format: &'static str,
    pub input_size: usize,
    pub output_size: usize,
    /// Outline technology of the font: "TrueType (glyf)" or "PostScript/CFF".
    pub outline: &'static str,
    /// Font family name read from the `name` table, if available.
    pub family: Option<String>,
    /// MIME type for the output container.
    pub mime: &'static str,
    /// File extension (without dot) for the output container.
    pub ext: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Woff2,
    Woff,
    Ttf,
    Otf,
}

impl Target {
    fn parse(s: &str) -> Result<Target, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "woff2" => Ok(Target::Woff2),
            "woff" => Ok(Target::Woff),
            "ttf" => Ok(Target::Ttf),
            "otf" => Ok(Target::Otf),
            other => Err(format!(
                "unknown format {other:?}; expected one of: woff2, woff, ttf, otf"
            )),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Target::Woff2 => "WOFF2",
            Target::Woff => "WOFF",
            Target::Ttf => "TTF",
            Target::Otf => "OTF",
        }
    }
    fn mime(self) -> &'static str {
        match self {
            Target::Woff2 => "font/woff2",
            Target::Woff => "font/woff",
            Target::Ttf => "font/ttf",
            Target::Otf => "font/otf",
        }
    }
    fn ext(self) -> &'static str {
        match self {
            Target::Woff2 => "woff2",
            Target::Woff => "woff",
            Target::Ttf => "ttf",
            Target::Otf => "otf",
        }
    }
}

fn be_u32(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off + 4)
        .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}
fn be_u16(b: &[u8], off: usize) -> Option<u16> {
    b.get(off..off + 2).map(|s| u16::from_be_bytes([s[0], s[1]]))
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
        SIG_WOFF => wuff::decompress_woff1(bytes)
            .map_err(|e| format!("could not decode WOFF: {e:?}")),
        FLAVOR_TRUETYPE | FLAVOR_TRUE | FLAVOR_OTTO => Ok(bytes.to_vec()),
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

/// The 63 WOFF2 "known table tags", indexed 0..=62 (W3C WOFF2 spec, Table 5).
const KNOWN_TAGS: [&[u8; 4]; 63] = [
    b"cmap", b"head", b"hhea", b"hmtx", b"maxp", b"name", b"OS/2", b"post", b"cvt ",
    b"fpgm", b"glyf", b"loca", b"prep", b"CFF ", b"VORG", b"EBDT", b"EBLC", b"gasp",
    b"hdmx", b"kern", b"LTSH", b"PCLT", b"VDMX", b"vhea", b"vmtx", b"BASE", b"GDEF",
    b"GPOS", b"GSUB", b"EBSC", b"JSTF", b"MATH", b"CBDT", b"CBLC", b"COLR", b"CPAL",
    b"SVG ", b"sbix", b"acnt", b"avar", b"bdat", b"bloc", b"bsln", b"cvar", b"fdsc",
    b"feat", b"fmtx", b"fvar", b"gvar", b"hsty", b"just", b"lcar", b"mort", b"morx",
    b"opbd", b"prop", b"trak", b"Zapf", b"Silf", b"Glat", b"Gloc", b"Feat", b"Sill",
];

fn known_tag_index(tag: &[u8; 4]) -> Option<u8> {
    KNOWN_TAGS.iter().position(|t| *t == tag).map(|i| i as u8)
}

fn round4(n: usize) -> usize {
    (n + 3) & !3
}

/// UIntBase128 (big-endian base-128, minimum bytes, continuation bit on all but
/// the last byte) — WOFF2 spec §Data Types.
fn write_base128(out: &mut Vec<u8>, v: u32) {
    let mut len = 1;
    let mut tmp = v >> 7;
    while tmp != 0 {
        len += 1;
        tmp >>= 7;
    }
    for i in (0..len).rev() {
        let mut b = ((v >> (7 * i)) & 0x7f) as u8;
        if i != 0 {
            b |= 0x80;
        }
        out.push(b);
    }
}

struct SfntTable {
    tag: [u8; 4],
    offset: usize,
    length: usize,
    checksum: u32,
}

/// Parse an SFNT table directory. Returns (flavor, tables-in-directory-order).
fn parse_sfnt(sfnt: &[u8]) -> Result<(u32, Vec<SfntTable>), String> {
    let flavor = be_u32(sfnt, 0).ok_or("truncated SFNT header")?;
    let num_tables = be_u16(sfnt, 4).ok_or("truncated SFNT header")? as usize;
    if num_tables == 0 {
        return Err("font has no tables".to_string());
    }
    let mut tables = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let base = 12 + i * 16;
        let tag_slice = sfnt
            .get(base..base + 4)
            .ok_or("truncated SFNT table directory")?;
        let mut tag = [0u8; 4];
        tag.copy_from_slice(tag_slice);
        let checksum = be_u32(sfnt, base + 4).ok_or("truncated SFNT table directory")?;
        let offset = be_u32(sfnt, base + 8).ok_or("truncated SFNT table directory")? as usize;
        let length = be_u32(sfnt, base + 12).ok_or("truncated SFNT table directory")? as usize;
        if offset.checked_add(length).map_or(true, |end| end > sfnt.len()) {
            return Err(format!(
                "table {:?} extends beyond the font data",
                String::from_utf8_lossy(&tag)
            ));
        }
        tables.push(SfntTable {
            tag,
            offset,
            length,
            checksum,
        });
    }
    Ok((flavor, tables))
}

fn brotli_compress(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut params = brotli::enc::BrotliEncoderParams::default();
    params.quality = 11;
    params.lgwin = 22;
    let mut out = Vec::new();
    brotli::BrotliCompress(&mut &data[..], &mut out, &params)
        .map_err(|e| format!("Brotli compression failed: {e}"))?;
    Ok(out)
}

/// Encode an SFNT as WOFF2 with a null transform (no glyf/loca transform). Works
/// for any SFNT flavor — used for CFF/OTF fonts (which `ttf2woff2` rejects) and as
/// a fallback for non-standard TrueType flavors.
fn sfnt_to_woff2_null(sfnt: &[u8]) -> Result<Vec<u8>, String> {
    let (flavor, tables) = parse_sfnt(sfnt)?;
    let num_tables = tables.len();

    let mut directory = Vec::new();
    let mut table_data = Vec::new();
    let mut total_sfnt = 12 + 16 * num_tables;

    for t in &tables {
        // Null transform: transformVersion is 3 for glyf/loca, 0 for everything
        // else. In both cases the table is stored untransformed and no
        // transformLength field is written.
        let transform_version: u8 = if &t.tag == b"glyf" || &t.tag == b"loca" {
            3
        } else {
            0
        };
        let flags = match known_tag_index(&t.tag) {
            Some(idx) => idx | (transform_version << 6),
            None => 63 | (transform_version << 6),
        };
        directory.push(flags);
        if known_tag_index(&t.tag).is_none() {
            directory.extend_from_slice(&t.tag);
        }
        write_base128(&mut directory, t.length as u32);
        table_data.extend_from_slice(&sfnt[t.offset..t.offset + t.length]);
        total_sfnt += round4(t.length);
    }

    let compressed = brotli_compress(&table_data)?;

    let mut out = Vec::with_capacity(48 + directory.len() + compressed.len());
    out.extend_from_slice(&SIG_WOFF2.to_be_bytes());
    out.extend_from_slice(&flavor.to_be_bytes());
    let length_pos = out.len();
    out.extend_from_slice(&0u32.to_be_bytes()); // length (patched below)
    out.extend_from_slice(&(num_tables as u16).to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // reserved
    out.extend_from_slice(&(total_sfnt as u32).to_be_bytes());
    out.extend_from_slice(&(compressed.len() as u32).to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // majorVersion
    out.extend_from_slice(&0u16.to_be_bytes()); // minorVersion
    out.extend_from_slice(&0u32.to_be_bytes()); // metaOffset
    out.extend_from_slice(&0u32.to_be_bytes()); // metaLength
    out.extend_from_slice(&0u32.to_be_bytes()); // metaOrigLength
    out.extend_from_slice(&0u32.to_be_bytes()); // privOffset
    out.extend_from_slice(&0u32.to_be_bytes()); // privLength
    out.extend_from_slice(&directory);
    out.extend_from_slice(&compressed);

    // The WOFF2 file must end on a 4-byte boundary (W3C WOFF2 spec) — pad with up
    // to 3 zero bytes. `totalCompressedSize` stays the unpadded Brotli length; the
    // header `length` counts the padding.
    while out.len() % 4 != 0 {
        out.push(0);
    }
    let total_len = out.len() as u32;
    out[length_pos..length_pos + 4].copy_from_slice(&total_len.to_be_bytes());
    Ok(out)
}

fn sfnt_to_woff2(sfnt: &[u8]) -> Result<Vec<u8>, String> {
    let flavor = be_u32(sfnt, 0).ok_or("truncated SFNT header")?;
    if flavor == FLAVOR_TRUETYPE {
        // TrueType/glyf: use the transforming encoder for the smallest output.
        match ttf2woff2::encode(sfnt, ttf2woff2::BrotliQuality::default()) {
            Ok(v) => Ok(v),
            // Fall back to the universal null-transform writer on any encoder
            // hiccup (e.g. an unusual table layout).
            Err(_) => sfnt_to_woff2_null(sfnt),
        }
    } else {
        // CFF/OTF and other flavors: null-transform WOFF2.
        sfnt_to_woff2_null(sfnt)
    }
}

fn zlib_compress(data: &[u8]) -> Vec<u8> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
    // Writing to a Vec never fails.
    let _ = enc.write_all(data);
    enc.finish().unwrap_or_default()
}

/// Encode an SFNT as WOFF v1 (per-table zlib), for any SFNT flavor.
fn sfnt_to_woff(sfnt: &[u8]) -> Result<Vec<u8>, String> {
    let (flavor, tables) = parse_sfnt(sfnt)?;
    let num_tables = tables.len();

    // Directory entries sorted by tag (SFNT/WOFF requirement).
    let mut sorted: Vec<&SfntTable> = tables.iter().collect();
    sorted.sort_by(|a, b| a.tag.cmp(&b.tag));

    let header_size = 44;
    let dir_size = num_tables * 20;
    let mut directory = Vec::with_capacity(dir_size);
    let mut body = Vec::new();
    let mut total_sfnt = 12 + 16 * num_tables;
    let mut cursor = header_size + dir_size;

    for t in &sorted {
        let raw = &sfnt[t.offset..t.offset + t.length];
        let z = zlib_compress(raw);
        // Store uncompressed if zlib didn't help.
        let stored: &[u8] = if z.len() < raw.len() { &z } else { raw };
        let comp_len = stored.len();

        directory.extend_from_slice(&t.tag);
        directory.extend_from_slice(&(cursor as u32).to_be_bytes());
        directory.extend_from_slice(&(comp_len as u32).to_be_bytes());
        directory.extend_from_slice(&(t.length as u32).to_be_bytes());
        directory.extend_from_slice(&t.checksum.to_be_bytes());

        body.extend_from_slice(stored);
        let pad = round4(comp_len) - comp_len;
        body.extend(std::iter::repeat(0u8).take(pad));

        cursor += comp_len + pad;
        total_sfnt += round4(t.length);
    }

    let total_len = header_size + dir_size + body.len();
    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(&SIG_WOFF.to_be_bytes());
    out.extend_from_slice(&flavor.to_be_bytes());
    out.extend_from_slice(&(total_len as u32).to_be_bytes());
    out.extend_from_slice(&(num_tables as u16).to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // reserved
    out.extend_from_slice(&(total_sfnt as u32).to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // majorVersion
    out.extend_from_slice(&0u16.to_be_bytes()); // minorVersion
    out.extend_from_slice(&0u32.to_be_bytes()); // metaOffset
    out.extend_from_slice(&0u32.to_be_bytes()); // metaLength
    out.extend_from_slice(&0u32.to_be_bytes()); // metaOrigLength
    out.extend_from_slice(&0u32.to_be_bytes()); // privOffset
    out.extend_from_slice(&0u32.to_be_bytes()); // privLength
    out.extend_from_slice(&directory);
    out.extend_from_slice(&body);
    Ok(out)
}

/// Best-effort font family name from the SFNT `name` table (nameID 1),
/// preferring a Windows (UTF-16BE) record, then a Mac (ASCII) record.
pub fn family_name(sfnt: &[u8]) -> Option<String> {
    let (_flavor, tables) = parse_sfnt(sfnt).ok()?;
    let name = tables.iter().find(|t| &t.tag == b"name")?;
    let nt = sfnt.get(name.offset..name.offset + name.length)?;
    let count = be_u16(nt, 2)? as usize;
    let storage = be_u16(nt, 4)? as usize;

    let mut best: Option<(u8, String)> = None; // (priority, value); higher priority wins
    for i in 0..count {
        let rb = 6 + i * 12;
        let platform = be_u16(nt, rb)?;
        let name_id = be_u16(nt, rb + 6)?;
        if name_id != 1 {
            continue;
        }
        let len = be_u16(nt, rb + 8)? as usize;
        let off = be_u16(nt, rb + 10)? as usize;
        let s = nt.get(storage + off..storage + off + len)?;
        let (priority, value) = if platform == 3 || platform == 0 {
            // UTF-16BE
            let u16s: Vec<u16> = s
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            (2u8, String::from_utf16_lossy(&u16s))
        } else {
            // Mac / other: treat as ASCII-ish (Latin-1).
            (1u8, s.iter().map(|&b| b as char).collect())
        };
        let value = value.trim().to_string();
        if value.is_empty() {
            continue;
        }
        if best.as_ref().map_or(true, |(p, _)| priority > *p) {
            best = Some((priority, value));
        }
    }
    best.map(|(_, v)| v)
}

/// Detect the outline technology of an SFNT for labelling.
fn outline_label(sfnt: &[u8]) -> &'static str {
    match be_u32(sfnt, 0) {
        Some(FLAVOR_OTTO) => "PostScript/CFF",
        _ => "TrueType (glyf)",
    }
}

/// Convert `bytes` (a TTF/OTF/WOFF/WOFF2 font) to `format`
/// (`"woff2"|"woff"|"ttf"|"otf"`).
pub fn convert(bytes: &[u8], format: &str) -> Result<Conversion, String> {
    let target = Target::parse(format)?;
    let input_format = detect_input(bytes)?;
    let sfnt = to_sfnt(bytes)?;
    let outline = outline_label(&sfnt);
    let family = family_name(&sfnt);

    let out = match target {
        Target::Woff2 => sfnt_to_woff2(&sfnt)?,
        Target::Woff => sfnt_to_woff(&sfnt)?,
        Target::Ttf | Target::Otf => sfnt,
    };

    Ok(Conversion {
        input_size: bytes.len(),
        output_size: out.len(),
        bytes: out,
        input_format,
        output_format: target.label(),
        outline,
        family,
        mime: target.mime(),
        ext: target.ext(),
    })
}

/// Read a WOFF header field: total number of tables (for tests/inspection).
#[allow(dead_code)]
fn sig_of(bytes: &[u8]) -> u32 {
    be_u32(bytes, 0).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TTF: &[u8] = include_bytes!("../tests/fixtures/sample.ttf");
    const OTF: &[u8] = include_bytes!("../tests/fixtures/sample.otf");
    const TTF_WOFF: &[u8] = include_bytes!("../tests/fixtures/sample_ttf.woff");
    const TTF_WOFF2: &[u8] = include_bytes!("../tests/fixtures/sample_ttf.woff2");
    const OTF_WOFF: &[u8] = include_bytes!("../tests/fixtures/sample_otf.woff");
    const OTF_WOFF2: &[u8] = include_bytes!("../tests/fixtures/sample_otf.woff2");

    #[test]
    fn detects_all_input_formats() {
        assert_eq!(detect_input(TTF).unwrap(), "TTF");
        assert_eq!(detect_input(OTF).unwrap(), "OTF");
        assert_eq!(detect_input(TTF_WOFF).unwrap(), "WOFF");
        assert_eq!(detect_input(TTF_WOFF2).unwrap(), "WOFF2");
    }

    #[test]
    fn ttf_to_woff2_is_smaller_and_well_formed() {
        let c = convert(TTF, "woff2").unwrap();
        assert_eq!(c.input_format, "TTF");
        assert_eq!(c.output_format, "WOFF2");
        assert_eq!(sig_of(&c.bytes), SIG_WOFF2);
        assert!(c.output_size < c.input_size, "woff2 should compress the ttf");
        // Round-trips back to a valid SFNT via an independent decoder.
        let back = wuff::decompress_woff2(&c.bytes).unwrap();
        assert_eq!(sig_of(&back), FLAVOR_TRUETYPE);
    }

    #[test]
    fn otf_to_woff2_null_transform_round_trips() {
        let c = convert(OTF, "woff2").unwrap();
        assert_eq!(c.output_format, "WOFF2");
        assert_eq!(c.outline, "PostScript/CFF");
        assert_eq!(sig_of(&c.bytes), SIG_WOFF2);
        // The hand-rolled null-transform WOFF2 must decode back to the CFF SFNT
        // via the independent `wuff` decoder, preserving the OTTO flavor.
        let back = wuff::decompress_woff2(&c.bytes).unwrap();
        assert_eq!(sig_of(&back), FLAVOR_OTTO);
        assert!(back.iter().eq(OTF.iter()) || back.len() == OTF.len());
    }

    #[test]
    fn ttf_to_woff_round_trips() {
        let c = convert(TTF, "woff").unwrap();
        assert_eq!(c.output_format, "WOFF");
        assert_eq!(sig_of(&c.bytes), SIG_WOFF);
        let back = wuff::decompress_woff1(&c.bytes).unwrap();
        assert_eq!(sig_of(&back), FLAVOR_TRUETYPE);
    }

    #[test]
    fn otf_to_woff_round_trips() {
        let c = convert(OTF, "woff").unwrap();
        assert_eq!(c.output_format, "WOFF");
        let back = wuff::decompress_woff1(&c.bytes).unwrap();
        assert_eq!(sig_of(&back), FLAVOR_OTTO);
    }

    #[test]
    fn woff2_to_ttf_extracts_sfnt() {
        let c = convert(TTF_WOFF2, "ttf").unwrap();
        assert_eq!(c.input_format, "WOFF2");
        assert_eq!(c.output_format, "TTF");
        assert_eq!(sig_of(&c.bytes), FLAVOR_TRUETYPE);
        assert_eq!(c.mime, "font/ttf");
    }

    #[test]
    fn woff_to_otf_extracts_cff_sfnt() {
        let c = convert(OTF_WOFF, "otf").unwrap();
        assert_eq!(c.input_format, "WOFF");
        assert_eq!(c.output_format, "OTF");
        assert_eq!(sig_of(&c.bytes), FLAVOR_OTTO);
    }

    #[test]
    fn woff_to_woff2_cross_container() {
        // WOFF (CFF) -> WOFF2 goes WOFF -> SFNT -> WOFF2 (null transform).
        let c = convert(OTF_WOFF, "woff2").unwrap();
        assert_eq!(c.input_format, "WOFF");
        assert_eq!(c.output_format, "WOFF2");
        let back = wuff::decompress_woff2(&c.bytes).unwrap();
        assert_eq!(sig_of(&back), FLAVOR_OTTO);
    }

    #[test]
    fn woff2_to_woff_cross_container() {
        let c = convert(OTF_WOFF2, "woff").unwrap();
        assert_eq!(c.output_format, "WOFF");
        let back = wuff::decompress_woff1(&c.bytes).unwrap();
        assert_eq!(sig_of(&back), FLAVOR_OTTO);
    }

    #[test]
    fn reads_family_name() {
        // The fixtures are synthetic fonts with family "Gizza Sample".
        let fam = family_name(TTF).unwrap();
        assert!(fam.to_ascii_lowercase().contains("gizza sample"), "got {fam:?}");
        assert!(family_name(OTF).unwrap().to_ascii_lowercase().contains("gizza"));
    }

    #[test]
    fn rejects_non_font_input() {
        let err = convert(b"not a font at all", "woff2").unwrap_err();
        assert!(err.contains("unrecognised"), "got {err}");
    }

    #[test]
    fn rejects_unknown_target_format() {
        let err = convert(TTF, "eot").unwrap_err();
        assert!(err.contains("unknown format"), "got {err}");
    }

    #[test]
    fn rejects_font_collections() {
        let mut ttc = b"ttcf".to_vec();
        ttc.extend_from_slice(&[0, 1, 0, 0]);
        let err = convert(&ttc, "woff2").unwrap_err();
        assert!(err.contains("collection"), "got {err}");
    }
}
