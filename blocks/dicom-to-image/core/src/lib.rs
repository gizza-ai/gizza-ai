//! dicom-to-image core — pure compute, shared by the chat skill block.
//! No wafer/wasm-bindgen deps.
//!
//! A minimal, hand-written DICOM (Part-10) parser for the common **uncompressed**
//! transfer syntaxes plus grayscale windowing to a PNG/JPEG via the `image` crate.
//! Scope (honest baseline — see the competitor-analysis doc):
//!
//!   * Transfer syntaxes: Implicit VR Little Endian (`1.2.840.10008.1.2`) and
//!     Explicit VR Little Endian (`1.2.840.10008.1.2.1`). Compressed/encapsulated
//!     pixel data (JPEG/JPEG2000/JPEG-LS/RLE) and big-endian/deflated syntaxes
//!     return a clear error rather than a wrong picture.
//!   * Single-sample (`SamplesPerPixel == 1`) grayscale: MONOCHROME1/MONOCHROME2.
//!     Colour (RGB/YBR/PALETTE) returns a clear error.
//!   * 8- or 16-bit, signed or unsigned pixels.
//!   * Rescale slope/intercept applied before windowing; embedded or manual
//!     window centre/width; multi-frame selection (1-based).

use std::io::Cursor;

use image::{GrayImage, ImageFormat, Luma};

/// Output container the caller wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Png,
    Jpeg,
}

impl OutputFormat {
    /// Parse the `format` param (default png). Errors on anything else so the
    /// LLM/CLI gets a precise message instead of a silent fallback.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "png" => Ok(OutputFormat::Png),
            "jpeg" | "jpg" => Ok(OutputFormat::Jpeg),
            other => Err(format!(
                "unsupported format {other:?} — use \"png\" or \"jpeg\""
            )),
        }
    }
    pub fn mime(self) -> &'static str {
        match self {
            OutputFormat::Png => "image/png",
            OutputFormat::Jpeg => "image/jpeg",
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Jpeg => "jpg",
        }
    }
}

/// Rendering options (all optional at the surface; defaults applied by caller).
#[derive(Debug, Clone)]
pub struct Options {
    pub format: OutputFormat,
    /// JPEG quality 1..=100 (ignored for PNG).
    pub quality: u8,
    /// Extra photometric inversion toggle (on top of MONOCHROME1 handling).
    pub invert: bool,
    /// 1-based frame index for multi-frame images.
    pub frame: u32,
    /// Manual window centre override (else embedded, else auto min/max).
    pub window_center: Option<f64>,
    /// Manual window width override (else embedded, else auto min/max).
    pub window_width: Option<f64>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: OutputFormat::Png,
            quality: 90,
            invert: false,
            frame: 1,
            window_center: None,
            window_width: None,
        }
    }
}

/// A single grayscale frame extracted from the DICOM: its dimensions plus the
/// rescaled pixel values ready for windowing.
#[derive(Debug)]
struct Frame {
    width: u32,
    height: u32,
    /// Rescaled (slope/intercept applied) pixel values, row-major.
    values: Vec<f64>,
    /// MONOCHROME1 → 0 is white (needs inversion vs the linear ramp).
    monochrome1: bool,
}

// ---------------------------------------------------------------------------
// Byte reader
// ---------------------------------------------------------------------------

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }
    fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }
    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| "corrupt DICOM: length overflow".to_string())?;
        if end > self.buf.len() {
            return Err("corrupt DICOM: unexpected end of file".to_string());
        }
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn skip(&mut self, n: usize) -> Result<(), String> {
        self.take(n).map(|_| ())
    }
    fn peek_u16(&self) -> Option<u16> {
        if self.pos + 2 <= self.buf.len() {
            Some(u16::from_le_bytes([self.buf[self.pos], self.buf[self.pos + 1]]))
        } else {
            None
        }
    }
}

const UNDEFINED_LEN: u32 = 0xFFFF_FFFF;
const TAG_ITEM: (u16, u16) = (0xFFFE, 0xE000);
const TAG_ITEM_DELIM: (u16, u16) = (0xFFFE, 0xE00D);
const TAG_SEQ_DELIM: (u16, u16) = (0xFFFE, 0xE0DD);

/// Explicit-VR long-form VRs carry a 2-byte reserved field then a 4-byte length;
/// all others use a 2-byte length.
fn is_long_vr(vr: &[u8]) -> bool {
    matches!(
        vr,
        b"OB" | b"OW" | b"OF" | b"OD" | b"OL" | b"OV" | b"SQ" | b"UT" | b"UN" | b"UC" | b"UR"
    )
}

#[derive(Clone, Copy)]
enum Encoding {
    ImplicitLe,
    ExplicitLe,
}

struct Element {
    group: u16,
    elem: u16,
    length: u32,
    undefined: bool,
}

/// Read one element header. Group `0xFFFE` items/delimiters are always encoded
/// tag + 4-byte length (no VR) regardless of the dataset encoding.
fn read_header(r: &mut Reader, enc: Encoding) -> Result<Element, String> {
    let group = r.u16()?;
    let elem = r.u16()?;
    if group == 0xFFFE {
        let length = r.u32()?;
        return Ok(Element {
            group,
            elem,
            length,
            undefined: length == UNDEFINED_LEN,
        });
    }
    let length = match enc {
        Encoding::ImplicitLe => r.u32()?,
        Encoding::ExplicitLe => {
            let vr = r.take(2)?;
            if is_long_vr(vr) {
                r.skip(2)?; // reserved
                r.u32()?
            } else {
                r.u16()? as u32
            }
        }
    };
    Ok(Element {
        group,
        elem,
        length,
        undefined: length == UNDEFINED_LEN,
    })
}

/// Skip an undefined-length sequence (SQ) — read items until the sequence
/// delimitation item. Handles nested undefined-length items/sequences.
fn skip_undefined_sequence(r: &mut Reader, enc: Encoding) -> Result<(), String> {
    loop {
        let g = r.u16()?;
        let e = r.u16()?;
        let len = r.u32()?;
        if (g, e) == TAG_SEQ_DELIM {
            return Ok(());
        }
        if (g, e) == TAG_ITEM {
            if len == UNDEFINED_LEN {
                skip_undefined_item(r, enc)?;
            } else {
                r.skip(len as usize)?;
            }
        } else {
            return Err("corrupt DICOM: malformed sequence".to_string());
        }
    }
}

/// Skip an undefined-length item — parse its element contents until the item
/// delimitation item.
fn skip_undefined_item(r: &mut Reader, enc: Encoding) -> Result<(), String> {
    loop {
        let el = read_header(r, enc)?;
        if (el.group, el.elem) == TAG_ITEM_DELIM {
            return Ok(());
        }
        if el.undefined {
            skip_undefined_sequence(r, enc)?;
        } else {
            r.skip(el.length as usize)?;
        }
    }
}

// ---------------------------------------------------------------------------
// Tag collection
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Tags {
    rows: Option<u16>,
    columns: Option<u16>,
    bits_allocated: Option<u16>,
    bits_stored: Option<u16>,
    pixel_representation: Option<u16>,
    samples_per_pixel: Option<u16>,
    photometric: Option<String>,
    number_of_frames: Option<u32>,
    window_center: Option<f64>,
    window_width: Option<f64>,
    rescale_slope: Option<f64>,
    rescale_intercept: Option<f64>,
    pixel_data: Option<Vec<u8>>,
}

fn parse_u16(bytes: &[u8]) -> Option<u16> {
    if bytes.len() >= 2 {
        Some(u16::from_le_bytes([bytes[0], bytes[1]]))
    } else {
        None
    }
}

/// Parse a DICOM string value: strip trailing NUL/space padding.
fn parse_str(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    s.trim_matches(|c: char| c == '\0' || c == ' ').to_string()
}

/// Parse the first value of a possibly multi-valued numeric string (DS/IS).
fn parse_num(bytes: &[u8]) -> Option<f64> {
    let s = parse_str(bytes);
    let first = s.split('\\').next().unwrap_or("").trim();
    first.parse::<f64>().ok()
}

/// Determine the transfer-syntax encoding from a UID string.
fn encoding_for(uid: &str) -> Result<Encoding, String> {
    match uid {
        "1.2.840.10008.1.2" => Ok(Encoding::ImplicitLe),
        "1.2.840.10008.1.2.1" => Ok(Encoding::ExplicitLe),
        "1.2.840.10008.1.2.1.99" => {
            Err("unsupported: Deflated Explicit VR Little Endian transfer syntax".to_string())
        }
        "1.2.840.10008.1.2.2" => {
            Err("unsupported: Explicit VR Big Endian transfer syntax".to_string())
        }
        other => Err(format!(
            "unsupported transfer syntax {other:?} — this tool decodes only uncompressed \
             Implicit/Explicit VR Little Endian DICOM (compressed JPEG/JPEG2000/JPEG-LS/RLE \
             pixel data is not supported)"
        )),
    }
}

/// Parse the File Meta Information group (group 0002, always Explicit VR LE) and
/// return the transfer-syntax UID.
fn read_meta_transfer_syntax(r: &mut Reader) -> Result<String, String> {
    let mut transfer_syntax: Option<String> = None;
    while let Some(group) = r.peek_u16() {
        if group != 0x0002 {
            break;
        }
        let el = read_header(r, Encoding::ExplicitLe)?;
        if el.undefined {
            return Err("corrupt DICOM: undefined length in file meta group".to_string());
        }
        let value = r.take(el.length as usize)?;
        if (el.group, el.elem) == (0x0002, 0x0010) {
            transfer_syntax = Some(parse_str(value));
        }
    }
    transfer_syntax.ok_or_else(|| {
        "not a supported DICOM file: missing Transfer Syntax UID (0002,0010)".to_string()
    })
}

/// Walk the dataset collecting the tags we need, stopping after PixelData.
fn collect_tags(r: &mut Reader, enc: Encoding) -> Result<Tags, String> {
    let mut tags = Tags::default();
    while r.remaining() > 0 {
        let el = read_header(r, enc)?;
        let key = (el.group, el.elem);
        // PixelData — stop here (everything we need precedes it in tag order).
        if key == (0x7FE0, 0x0010) {
            if el.undefined {
                return Err(
                    "unsupported: encapsulated (compressed) pixel data — this tool decodes only \
                     uncompressed DICOM"
                        .to_string(),
                );
            }
            tags.pixel_data = Some(r.take(el.length as usize)?.to_vec());
            break;
        }
        if el.undefined {
            // Undefined length ⇒ a sequence (SQ). Skip it.
            skip_undefined_sequence(r, enc)?;
            continue;
        }
        let value = r.take(el.length as usize)?;
        match key {
            (0x0028, 0x0002) => tags.samples_per_pixel = parse_u16(value),
            (0x0028, 0x0004) => tags.photometric = Some(parse_str(value)),
            (0x0028, 0x0008) => tags.number_of_frames = parse_num(value).map(|n| n as u32),
            (0x0028, 0x0010) => tags.rows = parse_u16(value),
            (0x0028, 0x0011) => tags.columns = parse_u16(value),
            (0x0028, 0x0100) => tags.bits_allocated = parse_u16(value),
            (0x0028, 0x0101) => tags.bits_stored = parse_u16(value),
            (0x0028, 0x0103) => tags.pixel_representation = parse_u16(value),
            (0x0028, 0x1050) => tags.window_center = parse_num(value),
            (0x0028, 0x1051) => tags.window_width = parse_num(value),
            (0x0028, 0x1052) => tags.rescale_intercept = parse_num(value),
            (0x0028, 0x1053) => tags.rescale_slope = parse_num(value),
            _ => {}
        }
    }
    Ok(tags)
}

/// Extract the requested frame's rescaled grayscale values from the collected tags.
fn extract_frame(tags: &Tags, frame: u32) -> Result<Frame, String> {
    let rows = tags
        .rows
        .filter(|&v| v > 0)
        .ok_or("missing or zero Rows (0028,0010)")? as u32;
    let columns = tags
        .columns
        .filter(|&v| v > 0)
        .ok_or("missing or zero Columns (0028,0011)")? as u32;
    let samples = tags.samples_per_pixel.unwrap_or(1);
    if samples != 1 {
        return Err(format!(
            "unsupported: SamplesPerPixel = {samples} — this tool decodes only single-sample \
             grayscale DICOM (colour images are not supported)"
        ));
    }
    let monochrome1 = match tags.photometric.as_deref() {
        None | Some("MONOCHROME2") => false,
        Some("MONOCHROME1") => true,
        Some(other) => {
            return Err(format!(
                "unsupported PhotometricInterpretation {other:?} — this tool decodes only \
                 MONOCHROME1/MONOCHROME2 grayscale (colour images are not supported)"
            ))
        }
    };
    let bits_allocated = tags.bits_allocated.unwrap_or(16);
    if bits_allocated != 8 && bits_allocated != 16 {
        return Err(format!(
            "unsupported BitsAllocated = {bits_allocated} — only 8-bit and 16-bit are supported"
        ));
    }
    let bytes_per_pixel = (bits_allocated / 8) as usize;
    let signed = tags.pixel_representation.unwrap_or(0) == 1;

    let num_frames = tags.number_of_frames.unwrap_or(1).max(1);
    if frame < 1 || frame > num_frames {
        return Err(format!(
            "frame {frame} out of range — image has {num_frames} frame(s) (frame is 1-based)"
        ));
    }

    let pixel_data = tags
        .pixel_data
        .as_ref()
        .ok_or("missing PixelData (7FE0,0010)")?;
    let pixels_per_frame = rows as usize * columns as usize;
    let frame_bytes = pixels_per_frame * bytes_per_pixel;
    let start = (frame as usize - 1) * frame_bytes;
    let end = start + frame_bytes;
    if end > pixel_data.len() {
        return Err(format!(
            "PixelData too short: need {end} bytes for frame {frame}, have {}",
            pixel_data.len()
        ));
    }
    let raw = &pixel_data[start..end];

    let slope = tags.rescale_slope.unwrap_or(1.0);
    let intercept = tags.rescale_intercept.unwrap_or(0.0);

    let mut values = Vec::with_capacity(pixels_per_frame);
    for i in 0..pixels_per_frame {
        let off = i * bytes_per_pixel;
        let stored: f64 = match (bytes_per_pixel, signed) {
            (1, false) => raw[off] as f64,
            (1, true) => (raw[off] as i8) as f64,
            (2, false) => u16::from_le_bytes([raw[off], raw[off + 1]]) as f64,
            (2, true) => i16::from_le_bytes([raw[off], raw[off + 1]]) as f64,
            _ => unreachable!(),
        };
        values.push(stored * slope + intercept);
    }

    Ok(Frame {
        width: columns,
        height: rows,
        values,
        monochrome1,
    })
}

/// Apply a linear VOI LUT (window centre/width) mapping to 8-bit grayscale,
/// following the DICOM PS3.3 C.11.2.1.2 formula.
fn window_to_luma(frame: &Frame, opts: &Options, tags: &Tags) -> GrayImage {
    // Resolve centre/width: manual override, else embedded, else auto min/max.
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    for &v in &frame.values {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }
    if !min.is_finite() || !max.is_finite() {
        min = 0.0;
        max = 0.0;
    }
    let auto_center = (max + min) / 2.0;
    let auto_width = (max - min).max(1.0);

    let center = opts
        .window_center
        .or(tags.window_center)
        .unwrap_or(auto_center);
    let mut width = opts.window_width.or(tags.window_width).unwrap_or(auto_width);
    if width < 1.0 {
        width = 1.0;
    }

    let range = width - 1.0;
    let lo = center - 0.5 - range / 2.0;
    let hi = center - 0.5 + range / 2.0;

    let mut img = GrayImage::new(frame.width, frame.height);
    for (i, &v) in frame.values.iter().enumerate() {
        let mut y = if range <= 0.0 {
            if v <= center - 0.5 {
                0.0
            } else {
                255.0
            }
        } else if v <= lo {
            0.0
        } else if v > hi {
            255.0
        } else {
            ((v - (center - 0.5)) / range + 0.5) * 255.0
        };
        y = y.clamp(0.0, 255.0);
        let mut px = y.round() as u8;
        if frame.monochrome1 {
            px = 255 - px;
        }
        if opts.invert {
            px = 255 - px;
        }
        let x = (i as u32) % frame.width;
        let row = (i as u32) / frame.width;
        img.put_pixel(x, row, Luma([px]));
    }
    img
}

fn encode(img: &GrayImage, opts: &Options) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    match opts.format {
        OutputFormat::Png => img
            .write_to(&mut out, ImageFormat::Png)
            .map_err(|e| format!("failed to encode PNG: {e}"))?,
        OutputFormat::Jpeg => {
            let q = opts.quality.clamp(1, 100);
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, q);
            enc.encode_image(img)
                .map_err(|e| format!("failed to encode JPEG: {e}"))?;
        }
    }
    Ok(out.into_inner())
}

/// Decode a DICOM byte buffer and render the requested frame to PNG/JPEG bytes.
pub fn render(dicom: &[u8], opts: &Options) -> Result<Vec<u8>, String> {
    // Part-10: 128-byte preamble then the "DICM" magic.
    if dicom.len() < 132 || &dicom[128..132] != b"DICM" {
        return Err(
            "not a DICOM file: missing the 128-byte preamble + \"DICM\" magic (only DICOM \
             Part-10 files are supported)"
                .to_string(),
        );
    }
    let mut r = Reader::new(dicom);
    r.pos = 132;
    let uid = read_meta_transfer_syntax(&mut r)?;
    let enc = encoding_for(&uid)?;
    let tags = collect_tags(&mut r, enc)?;
    let frame = extract_frame(&tags, opts.frame)?;
    let img = window_to_luma(&frame, opts, &tags);
    encode(&img, opts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    // --- tiny DICOM byte builder -----------------------------------------

    fn push_tag(out: &mut Vec<u8>, g: u16, e: u16) {
        out.extend_from_slice(&g.to_le_bytes());
        out.extend_from_slice(&e.to_le_bytes());
    }

    /// Explicit VR LE element with a short-form VR (2-byte length).
    fn explicit_short(out: &mut Vec<u8>, g: u16, e: u16, vr: &[u8; 2], value: &[u8]) {
        push_tag(out, g, e);
        out.extend_from_slice(vr);
        out.extend_from_slice(&(value.len() as u16).to_le_bytes());
        out.extend_from_slice(value);
    }

    /// Explicit VR LE element with a long-form VR (reserved + 4-byte length).
    fn explicit_long(out: &mut Vec<u8>, g: u16, e: u16, vr: &[u8; 2], value: &[u8]) {
        push_tag(out, g, e);
        out.extend_from_slice(vr);
        out.extend_from_slice(&[0, 0]); // reserved
        out.extend_from_slice(&(value.len() as u32).to_le_bytes());
        out.extend_from_slice(value);
    }

    /// Implicit VR LE element (4-byte length, no VR).
    fn implicit(out: &mut Vec<u8>, g: u16, e: u16, value: &[u8]) {
        push_tag(out, g, e);
        out.extend_from_slice(&(value.len() as u32).to_le_bytes());
        out.extend_from_slice(value);
    }

    fn us(v: u16) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }

    /// DS/IS/CS strings are even-length padded with a space.
    fn dcm_str(s: &str) -> Vec<u8> {
        let mut b = s.as_bytes().to_vec();
        if b.len() % 2 == 1 {
            b.push(b' ');
        }
        b
    }

    fn preamble_and_meta(transfer_syntax: &str) -> Vec<u8> {
        let mut out = vec![0u8; 128];
        out.extend_from_slice(b"DICM");
        // (0002,0010) Transfer Syntax UID, VR = UI (short form).
        explicit_short(&mut out, 0x0002, 0x0010, b"UI", &dcm_str(transfer_syntax));
        out
    }

    /// Build an Explicit VR LE 16-bit MONOCHROME2 image, `w`x`h`, from `pixels`.
    fn explicit_mono16(w: u16, h: u16, pixels: &[u16]) -> Vec<u8> {
        let mut out = preamble_and_meta("1.2.840.10008.1.2.1");
        explicit_short(&mut out, 0x0028, 0x0002, b"US", &us(1)); // SamplesPerPixel
        explicit_short(&mut out, 0x0028, 0x0004, b"CS", &dcm_str("MONOCHROME2"));
        explicit_short(&mut out, 0x0028, 0x0010, b"US", &us(h)); // Rows
        explicit_short(&mut out, 0x0028, 0x0011, b"US", &us(w)); // Columns
        explicit_short(&mut out, 0x0028, 0x0100, b"US", &us(16)); // BitsAllocated
        explicit_short(&mut out, 0x0028, 0x0101, b"US", &us(16)); // BitsStored
        explicit_short(&mut out, 0x0028, 0x0103, b"US", &us(0)); // PixelRepresentation
        let mut pd = Vec::new();
        for &p in pixels {
            pd.extend_from_slice(&p.to_le_bytes());
        }
        explicit_long(&mut out, 0x7FE0, 0x0010, b"OW", &pd);
        out
    }

    fn ramp_pixels(w: u16, h: u16) -> Vec<u16> {
        (0..(w as usize * h as usize))
            .map(|i| (i * 16) as u16)
            .collect()
    }

    #[test]
    fn png_happy_path_dimensions() {
        let (w, h) = (4u16, 3u16);
        let dcm = explicit_mono16(w, h, &ramp_pixels(w, h));
        let png = render(&dcm, &Options::default()).unwrap();
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!(img.dimensions(), (w as u32, h as u32));
    }

    #[test]
    fn jpeg_happy_path_dimensions() {
        let (w, h) = (8u16, 8u16);
        let dcm = explicit_mono16(w, h, &ramp_pixels(w, h));
        let opts = Options {
            format: OutputFormat::Jpeg,
            quality: 80,
            ..Options::default()
        };
        let jpg = render(&dcm, &opts).unwrap();
        assert_eq!(&jpg[..2], &[0xFF, 0xD8]); // JPEG SOI
        let img = image::load_from_memory(&jpg).unwrap();
        assert_eq!(img.dimensions(), (w as u32, h as u32));
    }

    #[test]
    fn implicit_vr_le_decodes() {
        // Same shape but Implicit VR LE.
        let mut out = vec![0u8; 128];
        out.extend_from_slice(b"DICM");
        // meta group is ALWAYS explicit VR
        {
            let ts = dcm_str("1.2.840.10008.1.2");
            push_tag(&mut out, 0x0002, 0x0010);
            out.extend_from_slice(b"UI");
            out.extend_from_slice(&(ts.len() as u16).to_le_bytes());
            out.extend_from_slice(&ts);
        }
        implicit(&mut out, 0x0028, 0x0002, &us(1));
        implicit(&mut out, 0x0028, 0x0004, &dcm_str("MONOCHROME2"));
        implicit(&mut out, 0x0028, 0x0010, &us(2)); // Rows
        implicit(&mut out, 0x0028, 0x0011, &us(2)); // Columns
        implicit(&mut out, 0x0028, 0x0100, &us(16));
        implicit(&mut out, 0x0028, 0x0101, &us(16));
        implicit(&mut out, 0x0028, 0x0103, &us(0));
        let pixels: [u16; 4] = [0, 1000, 2000, 4000];
        let mut pd = Vec::new();
        for p in pixels {
            pd.extend_from_slice(&p.to_le_bytes());
        }
        implicit(&mut out, 0x7FE0, 0x0010, &pd);

        let png = render(&out, &Options::default()).unwrap();
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!(img.dimensions(), (2, 2));
    }

    #[test]
    fn monochrome1_inverts_relative_to_monochrome2() {
        // A ramp: with MONOCHROME2 the first pixel (min) is darkest; MONOCHROME1
        // inverts so the first pixel is brightest.
        let (w, h) = (4u16, 1u16);
        let px = ramp_pixels(w, h);
        let mono2 = explicit_mono16(w, h, &px);
        let png2 = render(&mono2, &Options::default()).unwrap();
        let img2 = image::load_from_memory(&png2).unwrap().to_luma8();

        // Build the MONOCHROME1 variant.
        let mut mono1 = preamble_and_meta("1.2.840.10008.1.2.1");
        explicit_short(&mut mono1, 0x0028, 0x0002, b"US", &us(1));
        explicit_short(&mut mono1, 0x0028, 0x0004, b"CS", &dcm_str("MONOCHROME1"));
        explicit_short(&mut mono1, 0x0028, 0x0010, b"US", &us(h));
        explicit_short(&mut mono1, 0x0028, 0x0011, b"US", &us(w));
        explicit_short(&mut mono1, 0x0028, 0x0100, b"US", &us(16));
        explicit_short(&mut mono1, 0x0028, 0x0101, b"US", &us(16));
        explicit_short(&mut mono1, 0x0028, 0x0103, b"US", &us(0));
        let mut pd = Vec::new();
        for &p in &px {
            pd.extend_from_slice(&p.to_le_bytes());
        }
        explicit_long(&mut mono1, 0x7FE0, 0x0010, b"OW", &pd);
        let png1 = render(&mono1, &Options::default()).unwrap();
        let img1 = image::load_from_memory(&png1).unwrap().to_luma8();

        // Darkest pixel in mono2 should be the brightest in mono1.
        assert!(img2.get_pixel(0, 0).0[0] < img2.get_pixel(3, 0).0[0]);
        assert!(img1.get_pixel(0, 0).0[0] > img1.get_pixel(3, 0).0[0]);
    }

    #[test]
    fn compressed_transfer_syntax_errors() {
        // JPEG Baseline transfer syntax → unsupported.
        let mut out = preamble_and_meta("1.2.840.10008.1.2.4.50");
        explicit_short(&mut out, 0x0028, 0x0010, b"US", &us(2));
        explicit_short(&mut out, 0x0028, 0x0011, b"US", &us(2));
        let err = render(&out, &Options::default()).unwrap_err();
        assert!(err.contains("unsupported transfer syntax"), "got: {err}");
    }

    #[test]
    fn colour_samples_per_pixel_errors() {
        let mut out = preamble_and_meta("1.2.840.10008.1.2.1");
        explicit_short(&mut out, 0x0028, 0x0002, b"US", &us(3)); // RGB
        explicit_short(&mut out, 0x0028, 0x0004, b"CS", &dcm_str("RGB"));
        explicit_short(&mut out, 0x0028, 0x0010, b"US", &us(2));
        explicit_short(&mut out, 0x0028, 0x0011, b"US", &us(2));
        explicit_short(&mut out, 0x0028, 0x0100, b"US", &us(8));
        explicit_short(&mut out, 0x0028, 0x0101, b"US", &us(8));
        explicit_short(&mut out, 0x0028, 0x0103, b"US", &us(0));
        explicit_long(&mut out, 0x7FE0, 0x0010, b"OB", &[0u8; 12]);
        let err = render(&out, &Options::default()).unwrap_err();
        assert!(err.contains("SamplesPerPixel"), "got: {err}");
    }

    #[test]
    fn not_a_dicom_errors() {
        assert!(render(b"not a dicom file at all............", &Options::default()).is_err());
    }

    #[test]
    fn frame_out_of_range_errors() {
        let (w, h) = (2u16, 2u16);
        let dcm = explicit_mono16(w, h, &ramp_pixels(w, h));
        let opts = Options {
            frame: 5,
            ..Options::default()
        };
        let err = render(&dcm, &opts).unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
    }

    #[test]
    fn skips_sequence_before_pixel_data() {
        // An undefined-length sequence (0008,1140) with one undefined-length item
        // sitting between the image tags and PixelData must be skipped cleanly.
        let mut out = preamble_and_meta("1.2.840.10008.1.2.1");
        explicit_short(&mut out, 0x0028, 0x0002, b"US", &us(1));
        explicit_short(&mut out, 0x0028, 0x0004, b"CS", &dcm_str("MONOCHROME2"));
        explicit_short(&mut out, 0x0028, 0x0010, b"US", &us(2));
        explicit_short(&mut out, 0x0028, 0x0011, b"US", &us(2));
        explicit_short(&mut out, 0x0028, 0x0100, b"US", &us(16));
        explicit_short(&mut out, 0x0028, 0x0101, b"US", &us(16));
        explicit_short(&mut out, 0x0028, 0x0103, b"US", &us(0));
        // SQ with undefined length, one undefined-length item containing one element.
        push_tag(&mut out, 0x0008, 0x1140);
        out.extend_from_slice(b"SQ");
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&UNDEFINED_LEN.to_le_bytes());
        // item, undefined length
        push_tag(&mut out, 0xFFFE, 0xE000);
        out.extend_from_slice(&UNDEFINED_LEN.to_le_bytes());
        explicit_short(&mut out, 0x0008, 0x1150, b"UI", &dcm_str("1.2.3"));
        // item delimitation
        push_tag(&mut out, 0xFFFE, 0xE00D);
        out.extend_from_slice(&0u32.to_le_bytes());
        // sequence delimitation
        push_tag(&mut out, 0xFFFE, 0xE0DD);
        out.extend_from_slice(&0u32.to_le_bytes());
        // PixelData
        let px = ramp_pixels(2, 2);
        let mut pd = Vec::new();
        for p in px {
            pd.extend_from_slice(&p.to_le_bytes());
        }
        explicit_long(&mut out, 0x7FE0, 0x0010, b"OW", &pd);

        let png = render(&out, &Options::default()).unwrap();
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!(img.dimensions(), (2, 2));
    }

    #[test]
    fn format_parse() {
        assert_eq!(OutputFormat::parse("png").unwrap(), OutputFormat::Png);
        assert_eq!(OutputFormat::parse("JPEG").unwrap(), OutputFormat::Jpeg);
        assert_eq!(OutputFormat::parse("jpg").unwrap(), OutputFormat::Jpeg);
        assert!(OutputFormat::parse("gif").is_err());
    }
}
