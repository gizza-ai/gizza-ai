//! gizza-ai/tiff-to-pdf core — turn a multi-page (multi-IFD) TIFF directly into
//! a multi-page PDF. Pure Rust (`tiff` + `lopdf` + `flate2`), no ffmpeg and no
//! ML, so the same code runs in the chat block, the CLI and the browser page.
//!
//! Pipeline per selected TIFF page: seek to its IFD → decode → normalise to the
//! narrowest faithful PDF sample format (1-bit bilevel for fax/scan pages, 8-bit
//! DeviceGray for grayscale, 8-bit DeviceRGB otherwise) → Flate-compress the
//! samples into an image XObject → place it on its own PDF page with the
//! requested page size, orientation, margin and rotation. Rotation is applied
//! with the PDF placement matrix, so no pixels are ever resampled.
//!
//! Everything is streamed one page at a time and budgeted up front: the wasm
//! sandbox has 64 MiB, so an oversized page must produce an actionable error
//! rather than an opaque `wasm unreachable` trap.

use std::io::{Cursor, Write};

use flate2::{write::ZlibEncoder, Compression};
use lopdf::{dictionary, Document, Object, Stream};
use tiff::decoder::{Decoder, DecodingResult, Limits};
use tiff::tags::Tag;
use tiff::ColorType;

/// Largest decoded raster we will hold for a single page, in bytes. Sized so
/// decode + the RGB/gray copy + the compressed stream all fit the 64 MiB
/// sandbox with room to spare.
const MAX_PAGE_DECODED_BYTES: usize = 32 * 1024 * 1024;
/// Largest normalised (PDF-ready) sample buffer for a single page, in bytes.
const MAX_PAGE_SAMPLE_BYTES: usize = 24 * 1024 * 1024;
/// Largest PDF we will assemble, in bytes of accumulated compressed samples.
const MAX_OUTPUT_SAMPLE_BYTES: usize = 40 * 1024 * 1024;
/// Hard cap on pages written, so a malformed IFD chain can't loop forever.
const MAX_PAGES: usize = 500;
/// Resolution used when the TIFF carries no usable resolution tags.
const FALLBACK_DPI: f64 = 72.0;

/// Output page geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageSize {
    /// Page is exactly the image's physical size (plus margins). Default.
    Fit,
    A4,
    Letter,
    Legal,
    A3,
    Tabloid,
}

impl PageSize {
    pub fn parse(s: &str) -> Result<PageSize, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "fit" => Ok(PageSize::Fit),
            "a4" => Ok(PageSize::A4),
            "letter" => Ok(PageSize::Letter),
            "legal" => Ok(PageSize::Legal),
            "a3" => Ok(PageSize::A3),
            "tabloid" => Ok(PageSize::Tabloid),
            other => Err(format!(
                "unknown page_size `{other}` (expected fit, a4, letter, legal, a3 or tabloid)"
            )),
        }
    }

    /// Portrait dimensions in PDF points (1/72 inch); `None` for `Fit`.
    fn portrait_pt(self) -> Option<(f64, f64)> {
        match self {
            PageSize::Fit => None,
            PageSize::A4 => Some((595.28, 841.89)),
            PageSize::Letter => Some((612.0, 792.0)),
            PageSize::Legal => Some((612.0, 1008.0)),
            PageSize::A3 => Some((841.89, 1190.55)),
            PageSize::Tabloid => Some((792.0, 1224.0)),
        }
    }
}

/// How a fixed page size is oriented.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    /// Match each page to the shape of its own image. Default.
    Auto,
    Portrait,
    Landscape,
}

impl Orientation {
    pub fn parse(s: &str) -> Result<Orientation, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Orientation::Auto),
            "portrait" => Ok(Orientation::Portrait),
            "landscape" => Ok(Orientation::Landscape),
            other => Err(format!(
                "unknown orientation `{other}` (expected auto, portrait or landscape)"
            )),
        }
    }
}

/// How the source samples are carried into the PDF.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    /// Keep whatever the source page is: bilevel stays 1-bit, gray stays gray,
    /// colour stays colour. Smallest faithful output. Default.
    Auto,
    /// Force every page to 8-bit DeviceRGB.
    Color,
    /// Force every page to 8-bit DeviceGray (Rec. 601 luma for colour sources).
    Grayscale,
}

impl ColorMode {
    pub fn parse(s: &str) -> Result<ColorMode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(ColorMode::Auto),
            "color" | "colour" => Ok(ColorMode::Color),
            "grayscale" | "greyscale" => Ok(ColorMode::Grayscale),
            other => Err(format!(
                "unknown color `{other}` (expected auto, color or grayscale)"
            )),
        }
    }
}

/// Conversion settings. `Default` is the everyday "one PDF page per TIFF page,
/// same physical size, nothing re-encoded" behaviour.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub page_size: PageSize,
    pub orientation: Orientation,
    pub color: ColorMode,
    /// Blank border around the image, in PDF points (72 pt = 1 inch), 0-144.
    pub margin_pt: f64,
    /// 1-based page selection, e.g. `"1-3,7"`. Empty means every page.
    pub pages: String,
    /// Clockwise rotation applied to every page: 0, 90, 180 or 270 degrees.
    pub rotate: u32,
    /// Assumed input resolution in DPI; 0 reads it from the TIFF's own
    /// resolution tags and falls back to 72.
    pub dpi: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            page_size: PageSize::Fit,
            orientation: Orientation::Auto,
            color: ColorMode::Auto,
            margin_pt: 0.0,
            pages: String::new(),
            rotate: 0,
            dpi: 0.0,
        }
    }
}

impl Options {
    pub fn validate(&self) -> Result<(), String> {
        if !self.margin_pt.is_finite() || !(0.0..=144.0).contains(&self.margin_pt) {
            return Err(format!(
                "margin_pt must be between 0 and 144 points, got {}",
                self.margin_pt
            ));
        }
        if !matches!(self.rotate, 0 | 90 | 180 | 270) {
            return Err(format!(
                "rotate must be 0, 90, 180 or 270 degrees, got {}",
                self.rotate
            ));
        }
        if !self.dpi.is_finite() || !(0.0..=2400.0).contains(&self.dpi) {
            return Err(format!(
                "dpi must be 0 (read it from the file) or between 1 and 2400, got {}",
                self.dpi
            ));
        }
        Ok(())
    }
}

/// What one source page contributed to the PDF.
#[derive(Clone, Debug, PartialEq)]
pub struct PageReport {
    /// 1-based index of this page inside the source TIFF.
    pub source_page: usize,
    pub width_px: u32,
    pub height_px: u32,
    /// Resolution used to size the page, in DPI.
    pub dpi: f64,
    /// How the samples were embedded: `bilevel`, `grayscale` or `rgb`.
    pub color: &'static str,
    pub page_width_pt: f64,
    pub page_height_pt: f64,
}

/// A finished conversion.
#[derive(Clone, Debug)]
pub struct Conversion {
    pub pdf: Vec<u8>,
    /// Pages found in the source TIFF.
    pub source_pages: usize,
    /// Pages written to the PDF (differs when `pages` selects a subset).
    pub pages_written: usize,
    pub pages: Vec<PageReport>,
}

/// Expand a 1-based page selection like `"1-3,7,10-"` against `total` pages.
/// An empty (or whitespace-only) spec selects every page, in file order.
pub fn parse_pages(spec: &str, total: usize) -> Result<Vec<usize>, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Ok((1..=total).collect());
    }
    let mut out: Vec<usize> = Vec::new();
    for raw in spec.split(',') {
        let part = raw.trim();
        if part.is_empty() {
            continue;
        }
        let (from, to) = match part.split_once('-') {
            None => {
                let n = parse_page_number(part)?;
                (n, n)
            }
            Some((lhs, rhs)) => {
                let from = if lhs.trim().is_empty() {
                    1
                } else {
                    parse_page_number(lhs)?
                };
                let to = if rhs.trim().is_empty() {
                    total
                } else {
                    parse_page_number(rhs)?
                };
                (from, to)
            }
        };
        if from > to {
            return Err(format!(
                "page range `{part}` runs backwards: {from} is after {to}"
            ));
        }
        if to > total {
            return Err(format!(
                "page range `{part}` asks for page {to}, but this TIFF has {total} page(s)"
            ));
        }
        for n in from..=to {
            if !out.contains(&n) {
                out.push(n);
            }
        }
    }
    if out.is_empty() {
        return Err(format!("page selection `{spec}` selected no pages"));
    }
    Ok(out)
}

fn parse_page_number(s: &str) -> Result<usize, String> {
    let t = s.trim();
    let n: usize = t
        .parse()
        .map_err(|_| format!("page numbers must be whole numbers, got `{t}`"))?;
    if n == 0 {
        return Err("page numbers start at 1, got `0`".into());
    }
    Ok(n)
}

/// How one page's samples are laid out for PDF embedding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Samples {
    /// 1 bit per pixel, DeviceGray, rows padded to whole bytes.
    Bilevel,
    /// 8 bits per pixel, DeviceGray.
    Gray8,
    /// 24 bits per pixel, DeviceRGB.
    Rgb8,
}

impl Samples {
    fn label(self) -> &'static str {
        match self {
            Samples::Bilevel => "bilevel",
            Samples::Gray8 => "grayscale",
            Samples::Rgb8 => "rgb",
        }
    }

    fn bits_per_component(self) -> i64 {
        match self {
            Samples::Bilevel => 1,
            _ => 8,
        }
    }

    fn color_space(self) -> &'static str {
        match self {
            Samples::Rgb8 => "DeviceRGB",
            _ => "DeviceGray",
        }
    }

    /// Bytes one row of `width` pixels occupies.
    fn row_bytes(self, width: u32) -> usize {
        let w = width as usize;
        match self {
            Samples::Bilevel => w.div_ceil(8),
            Samples::Gray8 => w,
            Samples::Rgb8 => w * 3,
        }
    }
}

struct Raster {
    width: u32,
    height: u32,
    samples: Samples,
    data: Vec<u8>,
}

/// Convert a multi-page TIFF into a multi-page PDF.
pub fn tiff_to_pdf(bytes: &[u8], opts: &Options) -> Result<Conversion, String> {
    opts.validate()?;
    if bytes.is_empty() {
        return Err("no TIFF data was provided".into());
    }

    let mut limits = Limits::default();
    limits.decoding_buffer_size = MAX_PAGE_DECODED_BYTES;
    limits.intermediate_buffer_size = MAX_PAGE_DECODED_BYTES;

    let mut dec = Decoder::new(Cursor::new(bytes))
        .map_err(|e| format!("this does not look like a readable TIFF file: {e}"))?
        .with_limits(limits);

    let source_pages = count_pages(&mut dec)?;
    let selection = parse_pages(&opts.pages, source_pages)?;

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids: Vec<Object> = Vec::with_capacity(selection.len());
    let mut reports: Vec<PageReport> = Vec::with_capacity(selection.len());
    let mut compressed_total = 0usize;

    for &page_no in &selection {
        dec.seek_to_image(page_no - 1)
            .map_err(|e| format!("could not open page {page_no} of the TIFF: {e}"))?;

        let dpi = effective_dpi(&mut dec, opts.dpi);
        let raster = decode_page(&mut dec, page_no, opts.color)?;

        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&raster.data)
            .and_then(|_| enc.flush())
            .map_err(|e| format!("could not compress page {page_no}: {e}"))?;
        let compressed = enc
            .finish()
            .map_err(|e| format!("could not compress page {page_no}: {e}"))?;
        compressed_total += compressed.len();
        if compressed_total > MAX_OUTPUT_SAMPLE_BYTES {
            return Err(format!(
                "the PDF grew past the {} MB working limit at page {page_no}; convert fewer pages \
                 at a time with the `pages` option, or set color=grayscale",
                MAX_OUTPUT_SAMPLE_BYTES / (1024 * 1024)
            ));
        }

        let mut img_stream = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => raster.width as i64,
                "Height" => raster.height as i64,
                "ColorSpace" => raster.samples.color_space(),
                "BitsPerComponent" => raster.samples.bits_per_component(),
                "Filter" => "FlateDecode",
            },
            compressed,
        );
        // The samples are already deflated and `Filter` is set by hand, so stop
        // lopdf from compressing the stream a second time on save.
        img_stream.allows_compression = false;
        let img_id = doc.add_object(img_stream);

        let layout = layout_page(raster.width, raster.height, dpi, opts);
        let content_id = doc.add_object(Stream::new(
            dictionary! {},
            placement_operators(&layout).into_bytes(),
        ));
        let resources_id = doc.add_object(dictionary! {
            "XObject" => dictionary! { "Img" => img_id },
        });
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![
                0.into(),
                0.into(),
                round_pt(layout.page_w).into(),
                round_pt(layout.page_h).into(),
            ],
            "Resources" => resources_id,
            "Contents" => content_id,
        });
        page_ids.push(page_id.into());

        reports.push(PageReport {
            source_page: page_no,
            width_px: raster.width,
            height_px: raster.height,
            dpi: (dpi * 100.0).round() / 100.0,
            color: raster.samples.label(),
            page_width_pt: round_pt(layout.page_w),
            page_height_pt: round_pt(layout.page_h),
        });
    }

    let count = page_ids.len();
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Count" => count as i64,
            "Kids" => page_ids,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut pdf = Vec::new();
    doc.save_to(&mut pdf)
        .map_err(|e| format!("could not write the PDF: {e}"))?;

    Ok(Conversion {
        pdf,
        source_pages,
        pages_written: count,
        pages: reports,
    })
}

/// Walk the IFD chain to count pages, then rewind to the first one.
fn count_pages<R: std::io::Read + std::io::Seek>(dec: &mut Decoder<R>) -> Result<usize, String> {
    let mut count = 1usize;
    while dec.more_images() {
        if count >= MAX_PAGES {
            return Err(format!(
                "this TIFF declares more than {MAX_PAGES} pages, which is past what this tool \
                 converts in one pass"
            ));
        }
        dec.next_image()
            .map_err(|e| format!("the TIFF page chain is damaged after page {count}: {e}"))?;
        count += 1;
    }
    dec.seek_to_image(0)
        .map_err(|e| format!("could not rewind to the first TIFF page: {e}"))?;
    Ok(count)
}

/// Resolution to size the page with: the explicit override, else the TIFF's own
/// resolution tags, else 72 DPI (1 pixel = 1 point).
fn effective_dpi<R: std::io::Read + std::io::Seek>(dec: &mut Decoder<R>, override_dpi: f64) -> f64 {
    if override_dpi > 0.0 {
        return override_dpi;
    }
    let x_res = dec
        .find_tag(Tag::XResolution)
        .ok()
        .flatten()
        .and_then(|v| v.into_f64().ok());
    let unit = dec
        .find_tag(Tag::ResolutionUnit)
        .ok()
        .flatten()
        .and_then(|v| v.into_u32().ok())
        .unwrap_or(2);
    match x_res {
        // ResolutionUnit 2 = inch, 3 = centimetre; 1 = "no absolute unit", in
        // which case the numbers are only an aspect ratio and mean nothing here.
        Some(r) if r.is_finite() && r > 0.0 && unit == 2 && r <= 2400.0 => r,
        Some(r) if r.is_finite() && r > 0.0 && unit == 3 && r * 2.54 <= 2400.0 => r * 2.54,
        _ => FALLBACK_DPI,
    }
}

/// Decode the decoder's current page and normalise it to a PDF sample format.
fn decode_page<R: std::io::Read + std::io::Seek>(
    dec: &mut Decoder<R>,
    page_no: usize,
    mode: ColorMode,
) -> Result<Raster, String> {
    let (width, height) = dec
        .dimensions()
        .map_err(|e| format!("could not read the size of page {page_no}: {e}"))?;
    if width == 0 || height == 0 {
        return Err(format!("page {page_no} has no pixels ({width}x{height})"));
    }
    let color = dec
        .colortype()
        .map_err(|e| format!("could not read the colour layout of page {page_no}: {e}"))?;

    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| format!("page {page_no} is too large to convert ({width}x{height})"))?;
    let channels = channel_count(color, page_no)?;
    let depth = color.bit_depth();
    let decoded_bytes = pixels
        .saturating_mul(channels)
        .saturating_mul(depth.max(8) as usize / 8);
    if decoded_bytes > MAX_PAGE_DECODED_BYTES {
        return Err(format!(
            "page {page_no} is {width}x{height} and needs about {} MB to decode, past the {} MB \
             per-page limit; re-export it at a lower resolution or convert it on its own",
            decoded_bytes / (1024 * 1024),
            MAX_PAGE_DECODED_BYTES / (1024 * 1024)
        ));
    }

    let samples = target_samples(color, mode);
    let out_bytes = samples.row_bytes(width).saturating_mul(height as usize);
    if out_bytes > MAX_PAGE_SAMPLE_BYTES {
        return Err(format!(
            "page {page_no} would need about {} MB of PDF image data, past the {} MB per-page \
             limit; try color=grayscale or re-export the page smaller",
            out_bytes / (1024 * 1024),
            MAX_PAGE_SAMPLE_BYTES / (1024 * 1024)
        ));
    }

    let mut buffer = DecodingResult::U8(Vec::new());
    let layout = dec
        .read_image_to_buffer(&mut buffer)
        .map_err(|e| format!("could not decode page {page_no}: {e}"))?;
    let row_stride = layout
        .row_stride
        .map(|s| s.get())
        .unwrap_or_else(|| raw_row_bytes(width, channels, depth));

    let data = normalise(&buffer, width, height, row_stride, color, samples, page_no)?;
    Ok(Raster {
        width,
        height,
        samples,
        data,
    })
}

/// Photometric channels the decoder hands back for a colour type.
fn channel_count(color: ColorType, page_no: usize) -> Result<usize, String> {
    Ok(match color {
        ColorType::Gray(_) | ColorType::Palette(_) => 1,
        ColorType::GrayA(_) => 2,
        ColorType::RGB(_) | ColorType::YCbCr(_) => 3,
        ColorType::RGBA(_) | ColorType::CMYK(_) => 4,
        ColorType::CMYKA(_) => 5,
        other => {
            return Err(format!(
                "page {page_no} uses a colour layout this tool cannot convert yet ({other:?}); \
                 re-export the TIFF as grayscale or RGB"
            ))
        }
    })
}

fn raw_row_bytes(width: u32, channels: usize, depth: u8) -> usize {
    ((width as usize) * channels * depth as usize).div_ceil(8)
}

/// Pick the narrowest PDF sample format that stays faithful to the source.
fn target_samples(color: ColorType, mode: ColorMode) -> Samples {
    let source_is_gray = matches!(color, ColorType::Gray(_) | ColorType::GrayA(_));
    match mode {
        ColorMode::Color => Samples::Rgb8,
        ColorMode::Grayscale => {
            if matches!(color, ColorType::Gray(1)) {
                Samples::Bilevel
            } else {
                Samples::Gray8
            }
        }
        ColorMode::Auto => match color {
            // A fax/scan page is 1 bit in, 1 bit out — 24x smaller than RGB and
            // pixel-for-pixel identical.
            ColorType::Gray(1) => Samples::Bilevel,
            _ if source_is_gray => Samples::Gray8,
            _ => Samples::Rgb8,
        },
    }
}

/// Turn the decoder's buffer into packed PDF samples.
#[allow(clippy::too_many_arguments)]
fn normalise(
    buffer: &DecodingResult,
    width: u32,
    height: u32,
    row_stride: usize,
    color: ColorType,
    samples: Samples,
    page_no: usize,
) -> Result<Vec<u8>, String> {
    let bytes = match buffer {
        DecodingResult::U8(v) => Bytes::U8(v),
        DecodingResult::U16(v) => Bytes::U16(v),
        other => {
            return Err(format!(
                "page {page_no} stores samples in a numeric format this tool cannot convert \
                 ({}); re-export the TIFF with 8- or 16-bit samples",
                sample_kind(other)
            ))
        }
    };
    let depth = color.bit_depth();
    let channels = channel_count(color, page_no)?;

    // 1-bit gray straight through to 1-bit PDF: copy the packed rows verbatim.
    if samples == Samples::Bilevel {
        let out_row = samples.row_bytes(width);
        let src = bytes.as_u8().ok_or_else(|| {
            format!("page {page_no} is bilevel but was decoded as wide samples")
        })?;
        let mut out = Vec::with_capacity(out_row * height as usize);
        for y in 0..height as usize {
            let start = y * row_stride;
            let end = start + out_row;
            if end > src.len() {
                return Err(format!("page {page_no} ended early while reading row {y}"));
            }
            out.extend_from_slice(&src[start..end]);
        }
        return Ok(out);
    }

    let out_row = samples.row_bytes(width);
    let mut out = vec![0u8; out_row * height as usize];
    let mut px = [0u8; 5];
    for y in 0..height as usize {
        for x in 0..width as usize {
            read_pixel(&bytes, row_stride, depth, channels, x, y, &mut px, page_no)?;
            let (r, g, b) = to_rgb(color, &px);
            let o = y * out_row + x * if samples == Samples::Rgb8 { 3 } else { 1 };
            match samples {
                Samples::Rgb8 => {
                    out[o] = r;
                    out[o + 1] = g;
                    out[o + 2] = b;
                }
                // Rec. 601 luma, the same weighting the rest of the toolkit uses.
                Samples::Gray8 => {
                    out[o] = ((r as f32 * 0.299) + (g as f32 * 0.587) + (b as f32 * 0.114))
                        .round()
                        .clamp(0.0, 255.0) as u8;
                }
                Samples::Bilevel => unreachable!("bilevel is copied verbatim above"),
            }
        }
    }
    Ok(out)
}

enum Bytes<'a> {
    U8(&'a [u8]),
    U16(&'a [u16]),
}

impl Bytes<'_> {
    fn as_u8(&self) -> Option<&[u8]> {
        match self {
            Bytes::U8(v) => Some(v),
            Bytes::U16(_) => None,
        }
    }
}

fn sample_kind(r: &DecodingResult) -> &'static str {
    match r {
        DecodingResult::U8(_) => "8-bit unsigned",
        DecodingResult::U16(_) => "16-bit unsigned",
        DecodingResult::U32(_) => "32-bit unsigned",
        DecodingResult::U64(_) => "64-bit unsigned",
        DecodingResult::F16(_) => "16-bit float",
        DecodingResult::F32(_) => "32-bit float",
        DecodingResult::F64(_) => "64-bit float",
        DecodingResult::I8(_) => "8-bit signed",
        DecodingResult::I16(_) => "16-bit signed",
        DecodingResult::I32(_) => "32-bit signed",
        _ => "an unsupported numeric",
    }
}

/// Read one pixel's channels, scaled to 8 bits each.
#[allow(clippy::too_many_arguments)]
fn read_pixel(
    bytes: &Bytes,
    row_stride: usize,
    depth: u8,
    channels: usize,
    x: usize,
    y: usize,
    out: &mut [u8; 5],
    page_no: usize,
) -> Result<(), String> {
    match bytes {
        Bytes::U16(v) => {
            // `row_stride` is in bytes; a u16 buffer indexes half as far.
            let base = (y * row_stride) / 2 + x * channels;
            for c in 0..channels {
                let s = *v
                    .get(base + c)
                    .ok_or_else(|| format!("page {page_no} ended early at pixel ({x}, {y})"))?;
                out[c] = (s >> 8) as u8;
            }
        }
        Bytes::U8(v) => match depth {
            8 => {
                let base = y * row_stride + x * channels;
                for c in 0..channels {
                    out[c] = *v
                        .get(base + c)
                        .ok_or_else(|| format!("page {page_no} ended early at pixel ({x}, {y})"))?;
                }
            }
            // Sub-byte depths stay packed in the decoder's buffer: pull each
            // sample out bit by bit and stretch it to full 8-bit range.
            1 | 2 | 4 => {
                let max = ((1u16 << depth) - 1) as u32;
                for c in 0..channels {
                    let bit = (x * channels + c) * depth as usize;
                    let byte = *v.get(y * row_stride + bit / 8).ok_or_else(|| {
                        format!("page {page_no} ended early at pixel ({x}, {y})")
                    })?;
                    let shift = 8 - depth as usize - (bit % 8);
                    let raw = ((byte >> shift) as u32) & max;
                    out[c] = ((raw * 255 + max / 2) / max) as u8;
                }
            }
            other => {
                return Err(format!(
                    "page {page_no} uses {other}-bit samples, which this tool cannot convert; \
                     re-export the TIFF with 1-, 2-, 4-, 8- or 16-bit samples"
                ))
            }
        },
    }
    Ok(())
}

/// Map one pixel's channels to 8-bit RGB.
fn to_rgb(color: ColorType, px: &[u8; 5]) -> (u8, u8, u8) {
    match color {
        ColorType::Gray(_) | ColorType::GrayA(_) | ColorType::Palette(_) => (px[0], px[0], px[0]),
        ColorType::RGB(_) | ColorType::RGBA(_) => (px[0], px[1], px[2]),
        ColorType::CMYK(_) | ColorType::CMYKA(_) => {
            // TIFF CMYK is stored as ink coverage: 0 = no ink.
            let k = px[3] as u32;
            let f = |c: u8| (255 - ((c as u32 * (255 - k) / 255) + k).min(255)) as u8;
            (f(px[0]), f(px[1]), f(px[2]))
        }
        // JPEG-in-TIFF is usually YCbCr; the decoder hands the channels through
        // untouched, so apply the JFIF conversion here.
        ColorType::YCbCr(_) => {
            let y = px[0] as f32;
            let cb = px[1] as f32 - 128.0;
            let cr = px[2] as f32 - 128.0;
            let cl = |v: f32| v.round().clamp(0.0, 255.0) as u8;
            (
                cl(y + 1.402 * cr),
                cl(y - 0.344136 * cb - 0.714136 * cr),
                cl(y + 1.772 * cb),
            )
        }
        _ => (px[0], px[0], px[0]),
    }
}

/// Where the image sits on its page, in PDF points.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Layout {
    page_w: f64,
    page_h: f64,
    /// Drawn box, already accounting for rotation.
    draw_w: f64,
    draw_h: f64,
    x: f64,
    y: f64,
    rotate: u32,
}

/// Size the page and place the image on it.
fn layout_page(width_px: u32, height_px: u32, dpi: f64, opts: &Options) -> Layout {
    let scale = 72.0 / dpi;
    let img_w = width_px as f64 * scale;
    let img_h = height_px as f64 * scale;
    // A quarter turn swaps which image axis runs across the page.
    let quarter = opts.rotate == 90 || opts.rotate == 270;
    let (nat_w, nat_h) = if quarter { (img_h, img_w) } else { (img_w, img_h) };
    let m = opts.margin_pt;

    match opts.page_size.portrait_pt() {
        // "Fit": the page IS the image, plus whatever margin was asked for.
        None => Layout {
            page_w: nat_w + 2.0 * m,
            page_h: nat_h + 2.0 * m,
            draw_w: nat_w,
            draw_h: nat_h,
            x: m,
            y: m,
            rotate: opts.rotate,
        },
        Some((pw, ph)) => {
            let landscape = match opts.orientation {
                Orientation::Portrait => false,
                Orientation::Landscape => true,
                Orientation::Auto => nat_w > nat_h,
            };
            let (page_w, page_h) = if landscape { (ph, pw) } else { (pw, ph) };
            // Never let the margin eat the whole page.
            let avail_w = (page_w - 2.0 * m).max(1.0);
            let avail_h = (page_h - 2.0 * m).max(1.0);
            let k = (avail_w / nat_w).min(avail_h / nat_h);
            let draw_w = nat_w * k;
            let draw_h = nat_h * k;
            Layout {
                page_w,
                page_h,
                draw_w,
                draw_h,
                x: (page_w - draw_w) / 2.0,
                y: (page_h - draw_h) / 2.0,
                rotate: opts.rotate,
            }
        }
    }
}

/// The content stream that paints the image, rotating it via the placement
/// matrix so no pixel is ever resampled. A PDF image occupies the unit square
/// with its first sample row along the top edge.
fn placement_operators(l: &Layout) -> String {
    let (w, h, x, y) = (l.draw_w, l.draw_h, l.x, l.y);
    let m = match l.rotate {
        90 => format!("0 {} {} 0 {} {}", -h, w, x, y + h),
        180 => format!("{} 0 0 {} {} {}", -w, -h, x + w, y + h),
        270 => format!("0 {} {} 0 {} {}", h, -w, x + w, y),
        _ => format!("{w} 0 0 {h} {x} {y}"),
    };
    format!("q {m} cm /Img Do Q")
}

fn round_pt(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiff::encoder::{colortype, TiffEncoder};

    /// Build a two-page TIFF: an 8x6 grayscale page then a 4x4 RGB page.
    fn two_page_tiff() -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut enc = TiffEncoder::new(&mut buf).unwrap();
            let gray: Vec<u8> = (0..8 * 6).map(|i| (i * 5) as u8).collect();
            enc.write_image::<colortype::Gray8>(8, 6, &gray).unwrap();
            let mut rgb = vec![0u8; 4 * 4 * 3];
            for (i, px) in rgb.chunks_mut(3).enumerate() {
                px[0] = 255;
                px[1] = i as u8;
                px[2] = 0;
            }
            enc.write_image::<colortype::RGB8>(4, 4, &rgb).unwrap();
        }
        buf.into_inner()
    }

    fn one_page_rgb(w: u32, h: u32) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut enc = TiffEncoder::new(&mut buf).unwrap();
            let rgb = vec![128u8; (w * h * 3) as usize];
            enc.write_image::<colortype::RGB8>(w, h, &rgb).unwrap();
        }
        buf.into_inner()
    }

    fn pdf_text(pdf: &[u8]) -> String {
        pdf.iter().map(|&b| b as char).collect()
    }

    #[test]
    fn multi_page_tiff_becomes_a_multi_page_pdf() {
        let out = tiff_to_pdf(&two_page_tiff(), &Options::default()).unwrap();
        assert_eq!(out.source_pages, 2);
        assert_eq!(out.pages_written, 2);
        assert!(out.pdf.starts_with(b"%PDF-"));

        let text = pdf_text(&out.pdf);
        assert_eq!(text.matches("/MediaBox").count(), 2, "one MediaBox per page");
        assert!(text.contains("/Type/Catalog"));
        // Each page keeps its own source page's geometry (8x6 pt then 4x4 pt).
        assert!(text.contains("/MediaBox[0 0 8 6]"), "page 1 box");
        assert!(text.contains("/MediaBox[0 0 4 4]"), "page 2 box");

        // Page 1 is grayscale in, grayscale out; page 2 is RGB in, RGB out.
        assert_eq!(out.pages[0].color, "grayscale");
        assert_eq!(out.pages[0].width_px, 8);
        assert_eq!(out.pages[0].height_px, 6);
        assert_eq!(out.pages[1].color, "rgb");
        // No resolution tags were written, so 72 DPI => 1 px = 1 pt.
        assert_eq!(out.pages[0].dpi, 72.0);
        assert_eq!(out.pages[0].page_width_pt, 8.0);
        assert_eq!(out.pages[0].page_height_pt, 6.0);
    }

    #[test]
    fn page_selection_keeps_only_the_requested_pages() {
        let opts = Options {
            pages: "2".into(),
            ..Options::default()
        };
        let out = tiff_to_pdf(&two_page_tiff(), &opts).unwrap();
        assert_eq!(out.source_pages, 2);
        assert_eq!(out.pages_written, 1);
        assert_eq!(out.pages[0].source_page, 2);
        assert_eq!(pdf_text(&out.pdf).matches("/MediaBox").count(), 1);
    }

    #[test]
    fn fixed_page_size_letterboxes_and_centres_the_image() {
        let opts = Options {
            page_size: PageSize::A4,
            orientation: Orientation::Portrait,
            margin_pt: 36.0,
            ..Options::default()
        };
        // A 200x100 image on portrait A4 with 36 pt margins: the width is the
        // binding constraint, so it scales to 523.28 pt wide.
        let out = tiff_to_pdf(&one_page_rgb(200, 100), &opts).unwrap();
        assert_eq!(out.pages[0].page_width_pt, 595.28);
        assert_eq!(out.pages[0].page_height_pt, 841.89);
    }

    #[test]
    fn auto_orientation_turns_a_wide_page_landscape() {
        let opts = Options {
            page_size: PageSize::Letter,
            ..Options::default()
        };
        let out = tiff_to_pdf(&one_page_rgb(200, 100), &opts).unwrap();
        assert_eq!(out.pages[0].page_width_pt, 792.0);
        assert_eq!(out.pages[0].page_height_pt, 612.0);
    }

    #[test]
    fn a_quarter_turn_swaps_the_fitted_page_dimensions() {
        let opts = Options {
            rotate: 90,
            ..Options::default()
        };
        let out = tiff_to_pdf(&one_page_rgb(20, 10), &opts).unwrap();
        assert_eq!(out.pages[0].page_width_pt, 10.0);
        assert_eq!(out.pages[0].page_height_pt, 20.0);
    }

    #[test]
    fn grayscale_mode_shrinks_a_colour_page() {
        let colour = tiff_to_pdf(&one_page_rgb(64, 64), &Options::default()).unwrap();
        let gray = tiff_to_pdf(
            &one_page_rgb(64, 64),
            &Options {
                color: ColorMode::Grayscale,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(colour.pages[0].color, "rgb");
        assert_eq!(gray.pages[0].color, "grayscale");
        assert!(
            gray.pdf.len() < colour.pdf.len(),
            "grayscale {} should be smaller than rgb {}",
            gray.pdf.len(),
            colour.pdf.len()
        );
    }

    #[test]
    fn dpi_override_scales_the_page_down() {
        let opts = Options {
            dpi: 300.0,
            ..Options::default()
        };
        // 300 px at 300 DPI is exactly one inch = 72 points.
        let out = tiff_to_pdf(&one_page_rgb(300, 150), &opts).unwrap();
        assert_eq!(out.pages[0].dpi, 300.0);
        assert_eq!(out.pages[0].page_width_pt, 72.0);
        assert_eq!(out.pages[0].page_height_pt, 36.0);
    }

    #[test]
    fn non_tiff_input_is_rejected_with_a_readable_message() {
        let err = tiff_to_pdf(b"%PDF-1.5 not a tiff at all", &Options::default()).unwrap_err();
        assert!(
            err.contains("does not look like a readable TIFF"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn empty_input_is_rejected() {
        let err = tiff_to_pdf(&[], &Options::default()).unwrap_err();
        assert!(err.contains("no TIFF data"), "unexpected error: {err}");
    }

    #[test]
    fn selecting_a_page_past_the_end_names_the_real_page_count() {
        let opts = Options {
            pages: "5".into(),
            ..Options::default()
        };
        let err = tiff_to_pdf(&two_page_tiff(), &opts).unwrap_err();
        assert!(err.contains("this TIFF has 2 page(s)"), "unexpected: {err}");
    }

    #[test]
    fn page_ranges_expand_dedupe_and_validate() {
        assert_eq!(parse_pages("", 3).unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_pages("1-3,5", 6).unwrap(), vec![1, 2, 3, 5]);
        assert_eq!(parse_pages("3-", 5).unwrap(), vec![3, 4, 5]);
        assert_eq!(parse_pages("-2", 5).unwrap(), vec![1, 2]);
        // Later duplicates are dropped, and order follows the spec.
        assert_eq!(parse_pages("3,1,3", 3).unwrap(), vec![3, 1]);
        assert!(parse_pages("0", 3).unwrap_err().contains("start at 1"));
        assert!(parse_pages("3-1", 3).unwrap_err().contains("backwards"));
        assert!(parse_pages("x", 3).unwrap_err().contains("whole numbers"));
    }

    #[test]
    fn options_are_validated_before_any_decoding() {
        let bad = Options {
            rotate: 45,
            ..Options::default()
        };
        assert!(bad.validate().unwrap_err().contains("rotate must be"));
        let bad = Options {
            margin_pt: 500.0,
            ..Options::default()
        };
        assert!(bad.validate().unwrap_err().contains("margin_pt"));
        let bad = Options {
            dpi: 5000.0,
            ..Options::default()
        };
        assert!(bad.validate().unwrap_err().contains("dpi"));
    }

    #[test]
    fn enum_parsing_rejects_unknown_values_by_name() {
        assert_eq!(PageSize::parse("A4").unwrap(), PageSize::A4);
        assert_eq!(Orientation::parse(" auto ").unwrap(), Orientation::Auto);
        assert_eq!(ColorMode::parse("greyscale").unwrap(), ColorMode::Grayscale);
        assert!(PageSize::parse("a5").unwrap_err().contains("unknown page_size"));
        assert!(Orientation::parse("sideways")
            .unwrap_err()
            .contains("unknown orientation"));
        assert!(ColorMode::parse("cmyk").unwrap_err().contains("unknown color"));
    }

    #[test]
    fn placement_matrix_maps_each_turn_onto_the_same_box() {
        // The drawn box is always [x, x+w] x [y, y+h]; only the image inside turns.
        let l = Layout {
            page_w: 30.0,
            page_h: 20.0,
            draw_w: 20.0,
            draw_h: 10.0,
            x: 5.0,
            y: 5.0,
            rotate: 0,
        };
        assert_eq!(placement_operators(&l), "q 20 0 0 10 5 5 cm /Img Do Q");
        let l90 = Layout { rotate: 90, ..l };
        assert_eq!(placement_operators(&l90), "q 0 -10 20 0 5 15 cm /Img Do Q");
        let l180 = Layout { rotate: 180, ..l };
        assert_eq!(placement_operators(&l180), "q -20 0 0 -10 25 15 cm /Img Do Q");
        let l270 = Layout { rotate: 270, ..l };
        assert_eq!(placement_operators(&l270), "q 0 10 -20 0 25 5 cm /Img Do Q");
    }
}
