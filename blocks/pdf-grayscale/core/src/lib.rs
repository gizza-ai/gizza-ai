//! pdf-grayscale core — pure PDF de-colorization shared by the chat skill block
//! and CLI. No wafer/wasm-bindgen deps, so it is unit-testable on the host.
//!
//! [`grayscale`] parses a PDF and, for the selected pages, rewrites the color
//! operators in each page's content stream so vector graphics and text are drawn
//! in gray instead of color:
//!   * `rg`/`RG` (DeviceRGB fill/stroke, 3 operands) → `g`/`G` (1 operand)
//!   * `k`/`K`   (DeviceCMYK fill/stroke, 4 operands) → `g`/`G` (1 operand)
//!   * `g`/`G`   (already gray) is left as-is in grayscale mode, or thresholded
//!     in black-white mode.
//! Gray is computed with the Rec. 601 luminance weights. In black-white mode the
//! resulting gray is snapped to pure black or white at `threshold` (0–255).
//!
//! Embedded raster images are handled separately and best-effort: when
//! `convert_images` is set, DCTDecode (JPEG) image XObjects referenced by the
//! selected pages are decoded, converted to gray (or thresholded), and
//! re-encoded as grayscale JPEGs. Images in other codecs are left unchanged
//! (reported via [`Summary::images_skipped`]) rather than failing the whole
//! operation — this is a deliberate, documented limit of the vector-first design.
//!
//! Limits: color set via a named/ICC color space + `sc`/`scn`/`SC`/`SCN` is not
//! rewritten (that needs full color-space tracking), and non-JPEG images are not
//! transcoded. Unselected pages are preserved byte-for-byte.

use std::collections::BTreeSet;

use image::{load_from_memory_with_format, ImageFormat};
use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId};

/// Output color model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Map every color to a continuous gray by luminance.
    Grayscale,
    /// Snap each gray value to pure black or white at `threshold`.
    BlackWhite,
}

impl Mode {
    /// Parse the chat/CLI `mode` string. Accepts a couple of friendly aliases.
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "grayscale" | "greyscale" | "gray" | "grey" | "" => Ok(Mode::Grayscale),
            "black-white" | "black-and-white" | "bw" | "blackwhite" | "monochrome" => {
                Ok(Mode::BlackWhite)
            }
            other => Err(format!(
                "invalid mode '{other}' (expected 'grayscale' or 'black-white')"
            )),
        }
    }
}

/// Conversion options mirroring the tool's chat/CLI params.
#[derive(Debug, Clone)]
pub struct Options {
    pub mode: Mode,
    /// Also grayscale embedded JPEG images on the selected pages.
    pub convert_images: bool,
    /// Black/white cutoff (0–255), only used in [`Mode::BlackWhite`].
    pub threshold: i64,
    /// 1-based page spec, e.g. "all", "1,3-5".
    pub pages: String,
}

/// What the conversion touched — surfaced honestly in the tool's `_for_llm`.
#[derive(Debug, Clone)]
pub struct Summary {
    pub bytes: Vec<u8>,
    pub pages_converted: usize,
    /// JPEG images converted to grayscale.
    pub images_converted: usize,
    /// Color images left unchanged (non-JPEG codecs we don't transcode).
    pub images_skipped: usize,
}

/// Parse a 1-based page spec ("all"/""/"1,3-5,8") into the set of pages to act on.
pub fn parse_pages(spec: &str, total: u32) -> Result<BTreeSet<u32>, String> {
    let s = spec.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("all") {
        return Ok((1..=total).collect());
    }
    let mut keep = BTreeSet::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            let (start, end): (u32, u32) = (
                a.trim().parse().map_err(|_| format!("invalid page '{a}'"))?,
                b.trim().parse().map_err(|_| format!("invalid page '{b}'"))?,
            );
            if start == 0 || end == 0 {
                return Err("page numbers are 1-based (>= 1)".into());
            }
            let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
            (lo..=hi).for_each(|p| {
                keep.insert(p);
            });
        } else {
            let p: u32 = part.parse().map_err(|_| format!("invalid page '{part}'"))?;
            if p == 0 {
                return Err("page numbers are 1-based (>= 1)".into());
            }
            keep.insert(p);
        }
    }
    if let Some(&max) = keep.iter().next_back() {
        if max > total {
            return Err(format!(
                "page {max} is out of range (document has {total} pages)"
            ));
        }
    }
    if keep.is_empty() {
        return Err("no pages selected".into());
    }
    Ok(keep)
}

/// Rec. 601 luminance of an RGB triple (each component in 0.0..=1.0).
fn luminance(r: f32, g: f32, b: f32) -> f32 {
    0.299 * r + 0.587 * g + 0.114 * b
}

/// DeviceCMYK → gray, via the naive additive CMYK→RGB then luminance.
fn cmyk_luminance(c: f32, m: f32, y: f32, k: f32) -> f32 {
    let r = (1.0 - c) * (1.0 - k);
    let g = (1.0 - m) * (1.0 - k);
    let b = (1.0 - y) * (1.0 - k);
    luminance(r, g, b)
}

/// Map a continuous gray (0.0..=1.0) to the output gray for the chosen mode.
fn map_gray(gray: f32, mode: Mode, threshold: i64) -> f32 {
    let g = gray.clamp(0.0, 1.0);
    match mode {
        Mode::Grayscale => g,
        Mode::BlackWhite => {
            if g * 255.0 >= threshold as f32 {
                1.0
            } else {
                0.0
            }
        }
    }
}

/// Convert the selected pages of `pdf` to gray / black-white.
pub fn grayscale(pdf: &[u8], opts: &Options) -> Result<Summary, String> {
    if !(0..=255).contains(&opts.threshold) {
        return Err(format!(
            "threshold must be between 0 and 255 (got {})",
            opts.threshold
        ));
    }
    let mut doc = Document::load_mem(pdf).map_err(|e| format!("failed to parse PDF: {e}"))?;
    let pages = doc.get_pages();
    let total = pages.len() as u32;
    if total == 0 {
        return Err("PDF has no pages".into());
    }
    let selected = parse_pages(&opts.pages, total)?;

    // --- Vector/text pass: rewrite color operators in each selected page. ---
    for &num in &selected {
        let page_id = pages[&num];
        // A page may legitimately have no content stream (blank page); skip it
        // rather than failing the whole document.
        let content = match doc.get_and_decode_page_content(page_id) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let mut ops = content.operations;
        let mut changed = false;
        for op in ops.iter_mut() {
            match op.operator.as_str() {
                "rg" | "RG" if op.operands.len() == 3 => {
                    let r = op.operands[0].as_float().unwrap_or(0.0);
                    let g = op.operands[1].as_float().unwrap_or(0.0);
                    let b = op.operands[2].as_float().unwrap_or(0.0);
                    let out = map_gray(luminance(r, g, b), opts.mode, opts.threshold);
                    op.operator = gray_op(&op.operator);
                    op.operands = vec![Object::Real(out)];
                    changed = true;
                }
                "k" | "K" if op.operands.len() == 4 => {
                    let c = op.operands[0].as_float().unwrap_or(0.0);
                    let m = op.operands[1].as_float().unwrap_or(0.0);
                    let y = op.operands[2].as_float().unwrap_or(0.0);
                    let k = op.operands[3].as_float().unwrap_or(0.0);
                    let out = map_gray(cmyk_luminance(c, m, y, k), opts.mode, opts.threshold);
                    op.operator = gray_op(&op.operator);
                    op.operands = vec![Object::Real(out)];
                    changed = true;
                }
                // Already gray: only needs rewriting to snap to black/white.
                "g" | "G" if opts.mode == Mode::BlackWhite && op.operands.len() == 1 => {
                    let cur = op.operands[0].as_float().unwrap_or(0.0);
                    let out = map_gray(cur, opts.mode, opts.threshold);
                    op.operands = vec![Object::Real(out)];
                    changed = true;
                }
                _ => {}
            }
        }
        if changed {
            let bytes = Content { operations: ops }
                .encode()
                .map_err(|e| format!("page {num}: failed to encode content: {e}"))?;
            doc.change_page_content(page_id, bytes)
                .map_err(|e| format!("page {num}: failed to write content: {e}"))?;
        }
    }

    // --- Image pass: best-effort JPEG grayscaling on the selected pages. ---
    let mut images_converted = 0;
    let mut images_skipped = 0;
    if opts.convert_images {
        let mut image_ids: BTreeSet<ObjectId> = BTreeSet::new();
        for &num in &selected {
            image_ids.extend(collect_image_ids(&doc, pages[&num]));
        }
        for id in image_ids {
            match convert_image(&mut doc, id, opts.mode, opts.threshold) {
                ImageOutcome::Converted => images_converted += 1,
                ImageOutcome::SkippedColor => images_skipped += 1,
                ImageOutcome::AlreadyGray => {}
            }
        }
    }

    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|e| format!("failed to serialize PDF: {e}"))?;
    Ok(Summary {
        bytes: out,
        pages_converted: selected.len(),
        images_converted,
        images_skipped,
    })
}

/// Lowercase fill ops (`rg`/`k`) map to `g`; uppercase stroke ops map to `G`.
fn gray_op(op: &str) -> String {
    if op.chars().next().is_some_and(|c| c.is_uppercase()) {
        "G".to_string()
    } else {
        "g".to_string()
    }
}

/// Collect the object ids of image XObjects referenced by a page (including
/// resources inherited from parent `Pages` nodes).
fn collect_image_ids(doc: &Document, page_id: ObjectId) -> BTreeSet<ObjectId> {
    let mut ids = BTreeSet::new();
    let (inline, refd) = match doc.get_page_resources(page_id) {
        Ok(r) => r,
        Err(_) => return ids,
    };
    let mut resource_dicts: Vec<&Dictionary> = Vec::new();
    if let Some(d) = inline {
        resource_dicts.push(d);
    }
    for rid in &refd {
        if let Ok(d) = doc.get_dictionary(*rid) {
            resource_dicts.push(d);
        }
    }
    for res in resource_dicts {
        let xobjects = match res.get(b"XObject") {
            Ok(Object::Reference(id)) => doc.get_dictionary(*id).ok(),
            Ok(Object::Dictionary(d)) => Some(d),
            _ => None,
        };
        if let Some(xobjects) = xobjects {
            for (_, value) in xobjects.iter() {
                if let Ok(id) = value.as_reference() {
                    if let Ok(stream) = doc.get_object(id).and_then(Object::as_stream) {
                        let is_image = matches!(
                            stream.dict.get(b"Subtype").and_then(Object::as_name),
                            Ok(name) if name == b"Image"
                        );
                        if is_image {
                            ids.insert(id);
                        }
                    }
                }
            }
        }
    }
    ids
}

enum ImageOutcome {
    Converted,
    /// Color image we don't transcode (non-JPEG codec).
    SkippedColor,
    /// Already grayscale — nothing to do.
    AlreadyGray,
}

/// Whether a `/Filter` entry is (or ends in) DCTDecode (baseline JPEG).
fn filter_is_dct(filter: &Object) -> bool {
    match filter {
        Object::Name(n) => n == b"DCTDecode",
        Object::Array(a) => a
            .last()
            .and_then(|o| o.as_name().ok())
            .is_some_and(|n| n == b"DCTDecode"),
        _ => false,
    }
}

/// Whether a `/ColorSpace` entry is a single-channel gray space already.
fn colorspace_is_gray(cs: &Object) -> bool {
    match cs {
        Object::Name(n) => n == b"DeviceGray" || n == b"G" || n == b"CalGray",
        _ => false,
    }
}

/// Best-effort: convert a single image XObject to grayscale JPEG in place.
fn convert_image(doc: &mut Document, id: ObjectId, mode: Mode, threshold: i64) -> ImageOutcome {
    let stream = match doc.get_object_mut(id) {
        Ok(Object::Stream(s)) => s,
        _ => return ImageOutcome::AlreadyGray,
    };
    if stream
        .dict
        .get(b"ColorSpace")
        .map(colorspace_is_gray)
        .unwrap_or(false)
    {
        return ImageOutcome::AlreadyGray;
    }
    let is_jpeg = stream
        .dict
        .get(b"Filter")
        .map(filter_is_dct)
        .unwrap_or(false);
    if !is_jpeg {
        // Some other color codec (FlateDecode raw samples, JPXDecode, …). We
        // don't have a raster pipeline for those — leave untouched, but report.
        return ImageOutcome::SkippedColor;
    }
    let decoded = match load_from_memory_with_format(&stream.content, ImageFormat::Jpeg) {
        Ok(img) => img,
        Err(_) => return ImageOutcome::SkippedColor,
    };
    let mut luma = decoded.to_luma8();
    if mode == Mode::BlackWhite {
        for pixel in luma.pixels_mut() {
            pixel.0[0] = if pixel.0[0] as i64 >= threshold { 255 } else { 0 };
        }
    }
    let mut buf = Vec::new();
    if luma
        .write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Jpeg)
        .is_err()
    {
        return ImageOutcome::SkippedColor;
    }
    stream.set_content(buf);
    stream.dict.set("ColorSpace", Object::Name(b"DeviceGray".to_vec()));
    stream.dict.set("BitsPerComponent", 8_i64);
    stream.dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
    // A /Decode array or /DecodeParms tuned for the old color space would now be
    // wrong; drop them so the gray samples render straight.
    stream.dict.remove(b"Decode");
    stream.dict.remove(b"DecodeParms");
    ImageOutcome::Converted
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, RgbImage};
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Object, Stream};

    /// Build a one-page PDF whose content stream paints with the given color
    /// operator (e.g. RGB `rg`), so the vector pass has something to rewrite.
    fn pdf_with_ops(ops: Vec<Operation>) -> Vec<u8> {
        pdf_with_pages(vec![ops])
    }

    /// Build an N-page PDF; each page gets its own content ops.
    fn pdf_with_pages(pages_ops: Vec<Vec<Operation>>) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut kids: Vec<Object> = Vec::new();
        let count = pages_ops.len() as i64;
        for ops in pages_ops {
            let content = Content { operations: ops };
            let content_id =
                doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            });
            kids.push(page_id.into());
        }
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => kids, "Count" => count,
            }),
        );
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    /// Decode page `num`'s content ops from a serialized PDF.
    fn ops_of(pdf: &[u8], num: u32) -> Vec<Operation> {
        let doc = Document::load_mem(pdf).unwrap();
        let page_id = doc.get_pages()[&num];
        doc.get_and_decode_page_content(page_id).unwrap().operations
    }

    fn opts(mode: Mode, convert_images: bool, threshold: i64, pages: &str) -> Options {
        Options {
            mode,
            convert_images,
            threshold,
            pages: pages.to_string(),
        }
    }

    #[test]
    fn mode_parse_accepts_expected_values() {
        assert_eq!(Mode::parse("grayscale").unwrap(), Mode::Grayscale);
        assert_eq!(Mode::parse("black-white").unwrap(), Mode::BlackWhite);
        assert!(Mode::parse("sepia").is_err());
    }

    #[test]
    fn rewrites_rgb_fill_to_gray() {
        // 1.0 0.0 0.0 rg  → g with luminance of pure red (0.299).
        let pdf = pdf_with_ops(vec![Operation::new(
            "rg",
            vec![1.0.into(), 0.0.into(), 0.0.into()],
        )]);
        let out = grayscale(&pdf, &opts(Mode::Grayscale, false, 128, "all")).unwrap();
        let ops = ops_of(&out.bytes, 1);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operator, "g");
        let v = ops[0].operands[0].as_float().unwrap();
        assert!((v - 0.299).abs() < 1e-3, "expected ~0.299, got {v}");
    }

    #[test]
    fn rewrites_rgb_stroke_and_cmyk_fill() {
        let pdf = pdf_with_ops(vec![
            Operation::new("RG", vec![0.0.into(), 1.0.into(), 0.0.into()]),
            Operation::new(
                "k",
                vec![0.0.into(), 0.0.into(), 0.0.into(), 1.0.into()],
            ),
        ]);
        let out = grayscale(&pdf, &opts(Mode::Grayscale, false, 128, "all")).unwrap();
        let ops = ops_of(&out.bytes, 1);
        assert_eq!(ops[0].operator, "G");
        assert!((ops[0].operands[0].as_float().unwrap() - 0.587).abs() < 1e-3);
        // CMYK 0,0,0,1 is pure black → gray 0.0.
        assert_eq!(ops[1].operator, "g");
        assert!(ops[1].operands[0].as_float().unwrap().abs() < 1e-3);
    }

    #[test]
    fn black_white_snaps_gray_at_threshold() {
        // Mid gray 0.5 (~127.5/255). With threshold 128 it should snap to black.
        let pdf = pdf_with_ops(vec![Operation::new("g", vec![0.5.into()])]);
        let out = grayscale(&pdf, &opts(Mode::BlackWhite, false, 128, "all")).unwrap();
        let ops = ops_of(&out.bytes, 1);
        assert_eq!(ops[0].operator, "g");
        assert_eq!(ops[0].operands[0].as_float().unwrap(), 0.0);

        // A brighter gray 0.9 snaps to white.
        let pdf2 = pdf_with_ops(vec![Operation::new("g", vec![0.9.into()])]);
        let out2 = grayscale(&pdf2, &opts(Mode::BlackWhite, false, 128, "all")).unwrap();
        assert_eq!(ops_of(&out2.bytes, 1)[0].operands[0].as_float().unwrap(), 1.0);
    }

    #[test]
    fn preserves_unselected_pages() {
        // Two pages both with red rg; convert only page 2.
        let pdf = pdf_with_pages(vec![
            vec![Operation::new("rg", vec![1.0.into(), 0.0.into(), 0.0.into()])],
            vec![Operation::new("rg", vec![1.0.into(), 0.0.into(), 0.0.into()])],
        ]);
        let out = grayscale(&pdf, &opts(Mode::Grayscale, false, 128, "2")).unwrap();
        assert_eq!(out.pages_converted, 1);
        assert_eq!(ops_of(&out.bytes, 1)[0].operator, "rg"); // page 1 untouched
        assert_eq!(ops_of(&out.bytes, 2)[0].operator, "g"); // page 2 converted
    }

    /// Build a one-page PDF embedding a small color JPEG image XObject.
    fn pdf_with_jpeg_image() -> Vec<u8> {
        // A 4x4 solid red RGB JPEG.
        let img = RgbImage::from_pixel(4, 4, image::Rgb([220, 20, 20]));
        let mut jpeg = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut jpeg), ImageFormat::Jpeg)
            .unwrap();

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let image_stream = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 4,
                "Height" => 4,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
                "Filter" => "DCTDecode",
            },
            jpeg,
        )
        .with_compression(false);
        let image_id = doc.add_object(image_stream);
        let resources_id = doc.add_object(dictionary! {
            "XObject" => dictionary! { "Im0" => image_id },
        });
        let content = Content {
            operations: vec![
                Operation::new("q", vec![]),
                Operation::new("Do", vec![Object::Name(b"Im0".to_vec())]),
                Operation::new("Q", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1,
            }),
        );
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    fn image_colorspace(pdf: &[u8]) -> String {
        let doc = Document::load_mem(pdf).unwrap();
        for (_, obj) in doc.objects.iter() {
            if let Ok(stream) = obj.as_stream() {
                if matches!(stream.dict.get(b"Subtype").and_then(Object::as_name), Ok(n) if n == b"Image")
                {
                    if let Ok(name) = stream.dict.get(b"ColorSpace").and_then(Object::as_name) {
                        return String::from_utf8_lossy(name).into_owned();
                    }
                }
            }
        }
        String::new()
    }

    #[test]
    fn converts_embedded_jpeg_to_devicegray() {
        let pdf = pdf_with_jpeg_image();
        assert_eq!(image_colorspace(&pdf), "DeviceRGB");
        let out = grayscale(&pdf, &opts(Mode::Grayscale, true, 128, "all")).unwrap();
        assert_eq!(out.images_converted, 1);
        assert_eq!(out.images_skipped, 0);
        assert_eq!(image_colorspace(&out.bytes), "DeviceGray");
    }

    #[test]
    fn leaves_images_untouched_when_convert_images_false() {
        let pdf = pdf_with_jpeg_image();
        let out = grayscale(&pdf, &opts(Mode::Grayscale, false, 128, "all")).unwrap();
        assert_eq!(out.images_converted, 0);
        assert_eq!(out.images_skipped, 0);
        assert_eq!(image_colorspace(&out.bytes), "DeviceRGB");
    }

    #[test]
    fn rejects_non_pdf_bytes() {
        let err = grayscale(b"not a pdf", &opts(Mode::Grayscale, true, 128, "all")).unwrap_err();
        assert!(err.contains("failed to parse"), "got: {err}");
    }

    #[test]
    fn rejects_out_of_range_threshold() {
        let pdf = pdf_with_ops(vec![Operation::new("g", vec![0.5.into()])]);
        assert!(grayscale(&pdf, &opts(Mode::BlackWhite, false, 999, "all")).is_err());
    }

    #[test]
    fn rejects_out_of_range_page() {
        let pdf = pdf_with_ops(vec![Operation::new("g", vec![0.5.into()])]);
        assert!(grayscale(&pdf, &opts(Mode::Grayscale, false, 128, "5")).is_err());
    }

    #[test]
    fn parse_pages_handles_ranges_and_all() {
        assert_eq!(parse_pages("all", 3).unwrap(), BTreeSet::from([1, 2, 3]));
        assert_eq!(parse_pages("1,3-4", 5).unwrap(), BTreeSet::from([1, 3, 4]));
        assert!(parse_pages("0", 3).is_err());
    }
}
