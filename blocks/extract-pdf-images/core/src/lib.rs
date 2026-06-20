//! gizza-ai/extract-pdf-images core — pure-Rust extraction of embedded raster
//! images from a PDF, bundled into a ZIP. No wafer/wasm-bindgen deps.
//!
//! Walks every object in the PDF, finds image XObjects (`/Subtype /Image`), and
//! recovers each one:
//!   - **DCTDecode** streams are already JPEG — written straight out as `.jpg`.
//!   - **JPXDecode** streams are JPEG 2000 — written as `.jp2` (raw).
//!   - **FlateDecode / LZW / raw** sample data for 8-bit DeviceGray or DeviceRGB
//!     is reconstructed into a `.png` via the `image` crate.
//!   - Anything else (CCITT/JBIG2 fax, 1/2/4/16-bit, Indexed/CMYK/ICC colour) is
//!     counted as skipped rather than emitting a corrupt file.
//! All recovered images are packed into a single ZIP (deflate).

use std::io::{Cursor, Write};

use image::{ImageFormat, ImageBuffer, Luma, Rgb};
use lopdf::{Document, Object};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// Summary of an extraction run (for the caller's user-facing message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// Number of images written into the ZIP.
    pub extracted: usize,
    /// Number of image XObjects that were found but could not be recovered.
    pub skipped: usize,
}

/// One recovered image: a suggested filename and its encoded bytes.
struct Image {
    name: String,
    bytes: Vec<u8>,
}

/// Collect the `/Filter` entry as a list of filter names (it may be a single
/// `Name` or an `Array` of names). Returns lowercased-as-written names.
fn filter_names(doc: &Document, obj: &Object) -> Vec<String> {
    fn name_of(o: &Object) -> Option<String> {
        o.as_name().ok().map(|n| String::from_utf8_lossy(n).to_string())
    }
    match obj {
        Object::Name(_) => name_of(obj).into_iter().collect(),
        Object::Array(items) => items
            .iter()
            .filter_map(|it| match it {
                Object::Reference(id) => doc.get_object(*id).ok().and_then(name_of),
                other => name_of(other),
            })
            .collect(),
        Object::Reference(id) => doc
            .get_object(*id)
            .ok()
            .map(|o| filter_names(doc, o))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Resolve a colour-space object to a simple device name when it is one.
fn colorspace_name(doc: &Document, obj: &Object) -> Option<String> {
    match obj {
        Object::Name(n) => Some(String::from_utf8_lossy(n).to_string()),
        Object::Reference(id) => doc.get_object(*id).ok().and_then(|o| colorspace_name(doc, o)),
        _ => None,
    }
}

/// Encode raw 8-bit sample data into a PNG for the device colour spaces we
/// support. Returns `None` (→ skipped) for anything unsupported or size-mismatched.
fn samples_to_png(samples: &[u8], width: u32, height: u32, colorspace: &str, bpc: i64) -> Option<Vec<u8>> {
    if bpc != 8 {
        return None;
    }
    let (w, h) = (width as usize, height as usize);
    let mut out = Cursor::new(Vec::new());
    match colorspace {
        "DeviceGray" | "CalGray" | "G" => {
            if samples.len() < w * h {
                return None;
            }
            let buf: ImageBuffer<Luma<u8>, _> =
                ImageBuffer::from_raw(width, height, samples[..w * h].to_vec())?;
            image::DynamicImage::ImageLuma8(buf).write_to(&mut out, ImageFormat::Png).ok()?;
        }
        "DeviceRGB" | "CalRGB" | "RGB" => {
            if samples.len() < w * h * 3 {
                return None;
            }
            let buf: ImageBuffer<Rgb<u8>, _> =
                ImageBuffer::from_raw(width, height, samples[..w * h * 3].to_vec())?;
            image::DynamicImage::ImageRgb8(buf).write_to(&mut out, ImageFormat::Png).ok()?;
        }
        _ => return None,
    }
    Some(out.into_inner())
}

/// Recover a single image XObject stream, if we can. `idx` seeds the filename.
fn recover_image(doc: &Document, stream: &lopdf::Stream, idx: usize) -> Result<Image, ()> {
    let dict = &stream.dict;
    // Must be an image XObject.
    let is_image = dict
        .get(b"Subtype")
        .ok()
        .and_then(|o| o.as_name().ok())
        .map(|n| n == b"Image")
        .unwrap_or(false);
    if !is_image {
        return Err(());
    }

    let filters = dict.get(b"Filter").ok().map(|o| filter_names(doc, o)).unwrap_or_default();
    let has = |f: &str| filters.iter().any(|x| x == f);

    // JPEG: the raw stream content already IS a JPEG file.
    if has("DCTDecode") {
        return Ok(Image { name: format!("image-{idx:03}.jpg"), bytes: stream.content.clone() });
    }
    // JPEG 2000: emit the raw codestream as .jp2.
    if has("JPXDecode") {
        return Ok(Image { name: format!("image-{idx:03}.jp2"), bytes: stream.content.clone() });
    }
    // Fax/JBIG2 bilevel — not reconstructed here.
    if has("CCITTFaxDecode") || has("JBIG2Decode") {
        return Err(());
    }

    // Otherwise treat the (decompressed) stream as raw samples and rebuild a PNG.
    let width = dict.get(b"Width").and_then(|o| o.as_i64()).map_err(|_| ())?;
    let height = dict.get(b"Height").and_then(|o| o.as_i64()).map_err(|_| ())?;
    if width <= 0 || height <= 0 || width > 20_000 || height > 20_000 {
        return Err(());
    }
    let bpc = dict.get(b"BitsPerComponent").and_then(|o| o.as_i64()).unwrap_or(8);
    let cs = dict
        .get(b"ColorSpace")
        .ok()
        .and_then(|o| colorspace_name(doc, o))
        .unwrap_or_else(|| "DeviceRGB".to_string());

    // `get_plain_content` applies Flate/LZW/ASCII85 when a /Filter is present,
    // and returns the raw bytes when the image samples are stored uncompressed.
    let samples = stream.get_plain_content().map_err(|_| ())?;
    let png = samples_to_png(&samples, width as u32, height as u32, &cs, bpc).ok_or(())?;
    Ok(Image { name: format!("image-{idx:03}.png"), bytes: png })
}

/// Extract every recoverable embedded image from `pdf` and return a ZIP of them
/// plus a summary. Errors if the PDF can't be parsed or contains no recoverable
/// images.
pub fn extract_images_zip(pdf: &[u8]) -> Result<(Vec<u8>, Summary), String> {
    if pdf.is_empty() {
        return Err("input PDF is empty".into());
    }
    let doc = Document::load_mem(pdf).map_err(|e| format!("failed to parse PDF: {e}"))?;

    let mut images: Vec<Image> = Vec::new();
    let mut skipped = 0usize;
    let mut idx = 0usize;
    for obj in doc.objects.values() {
        if let Object::Stream(s) = obj {
            // Quick gate: only consider image XObjects.
            let is_image = s
                .dict
                .get(b"Subtype")
                .ok()
                .and_then(|o| o.as_name().ok())
                .map(|n| n == b"Image")
                .unwrap_or(false);
            if !is_image {
                continue;
            }
            idx += 1;
            match recover_image(&doc, s, idx) {
                Ok(img) => images.push(img),
                Err(()) => skipped += 1,
            }
        }
    }

    if images.is_empty() {
        if skipped > 0 {
            return Err(format!(
                "found {skipped} embedded image(s) but none in a recoverable format \
                 (only JPEG/JPEG2000 and 8-bit gray/RGB images are supported)"
            ));
        }
        return Err("no embedded raster images found in the PDF".into());
    }

    // Bundle into a single ZIP (deflate).
    let mut buf = Vec::new();
    {
        let mut zw = ZipWriter::new(Cursor::new(&mut buf));
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for img in &images {
            zw.start_file(&img.name, opts).map_err(|e| format!("zip start_file: {e}"))?;
            zw.write_all(&img.bytes).map_err(|e| format!("zip write: {e}"))?;
        }
        zw.finish().map_err(|e| format!("zip finish: {e}"))?;
    }

    Ok((buf, Summary { extracted: images.len(), skipped }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Stream};

    fn zip_entries(zip_bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes)).expect("valid zip");
        let mut out = Vec::new();
        for i in 0..archive.len() {
            let mut f = archive.by_index(i).unwrap();
            let name = f.name().to_string();
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut f, &mut bytes).unwrap();
            out.push((name, bytes));
        }
        out
    }

    /// Build a minimal one-image PDF: a single Gray 8-bit FlateDecode image
    /// XObject. Returns (pdf_bytes, raw_gray_samples).
    fn pdf_with_gray_image(w: u32, h: u32) -> (Vec<u8>, Vec<u8>) {
        let samples: Vec<u8> = (0..(w * h)).map(|i| (i % 256) as u8).collect();
        let mut comp = Vec::new();
        {
            use flate2::write::ZlibEncoder;
            use flate2::Compression;
            let mut enc = ZlibEncoder::new(&mut comp, Compression::default());
            enc.write_all(&samples).unwrap();
            enc.finish().unwrap();
        }
        let mut doc = Document::with_version("1.5");
        // /Filter is already FlateDecode and `content` is pre-deflated, so
        // `compress()` (which only acts when there is no /Filter) leaves it as-is.
        let img = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => w as i64,
                "Height" => h as i64,
                "ColorSpace" => "DeviceGray",
                "BitsPerComponent" => 8i64,
                "Filter" => "FlateDecode",
            },
            comp,
        );
        let _id = doc.add_object(Object::Stream(img));
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        (out, samples)
    }

    #[test]
    fn extracts_flate_gray_image_as_png() {
        let (pdf, _samples) = pdf_with_gray_image(8, 4);
        let (zip, summary) = extract_images_zip(&pdf).unwrap();
        assert_eq!(summary.extracted, 1, "should extract one image");
        let entries = zip_entries(&zip);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].0.ends_with(".png"), "got {}", entries[0].0);
        // Decodes back to an 8x4 image.
        let decoded = image::load_from_memory(&entries[0].1).expect("valid png");
        assert_eq!((decoded.width(), decoded.height()), (8, 4));
    }

    #[test]
    fn dctdecode_passthrough_is_raw_jpeg() {
        // Encode a tiny JPEG, embed it raw as a DCTDecode image stream.
        let jpeg = {
            let buf: ImageBuffer<Rgb<u8>, _> = ImageBuffer::from_fn(4, 4, |x, _| Rgb([(x * 60) as u8, 10, 20]));
            let mut out = Cursor::new(Vec::new());
            image::DynamicImage::ImageRgb8(buf)
                .write_to(&mut out, ImageFormat::Jpeg)
                .unwrap();
            out.into_inner()
        };
        let mut doc = Document::with_version("1.5");
        let img = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 4i64,
                "Height" => 4i64,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8i64,
                "Filter" => "DCTDecode",
            },
            jpeg.clone(),
        );
        doc.add_object(Object::Stream(img));
        let mut pdf = Vec::new();
        doc.save_to(&mut pdf).unwrap();

        let (zip, summary) = extract_images_zip(&pdf).unwrap();
        assert_eq!(summary.extracted, 1);
        let entries = zip_entries(&zip);
        assert!(entries[0].0.ends_with(".jpg"));
        assert_eq!(entries[0].1, jpeg, "DCTDecode bytes must pass through unchanged");
    }

    #[test]
    fn empty_input_errors() {
        assert!(extract_images_zip(&[]).is_err());
    }

    #[test]
    fn pdf_without_images_errors() {
        let mut doc = Document::with_version("1.5");
        doc.add_object(Object::Stream(Stream::new(dictionary! { "Type" => "Metadata" }, b"x".to_vec())));
        let mut pdf = Vec::new();
        doc.save_to(&mut pdf).unwrap();
        assert!(extract_images_zip(&pdf).is_err());
    }

    #[test]
    fn samples_to_png_rejects_non_8bit_and_unknown_cs() {
        assert!(samples_to_png(&[0; 32], 8, 4, "DeviceGray", 1).is_none());
        assert!(samples_to_png(&[0; 32], 8, 4, "DeviceCMYK", 8).is_none());
        assert!(samples_to_png(&[0; 4], 8, 4, "DeviceGray", 8).is_none()); // too few samples
        assert!(samples_to_png(&[0; 32], 8, 4, "DeviceGray", 8).is_some());
    }
}
