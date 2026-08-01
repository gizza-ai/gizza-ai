//! gizza-ai/barcode-decode core — read 1D barcodes from an image.
//!
//! Pure-Rust: `image` decodes the raster to grayscale, `rxing` (a ZXing port)
//! recognises the barcode(s). No wasm-bindgen / filesystem deps on the decode
//! path — only the luma-buffer readers are reached, so it instantiates under
//! the wafer (wasm32-wasip1) runtime the CLI/MCP embed.
//!
//! Supports the common 1D symbologies: EAN-13, EAN-8, UPC-A, UPC-E, Code 128,
//! Code 39, Code 93, Codabar and ITF. For QR / 2D codes use the `qr-decode`
//! tool instead.

use std::collections::HashSet;
use std::io::Cursor;

use rxing::{BarcodeFormat, DecodeHints};

/// Reject images whose pixel count would blow the 64 MiB wasm sandbox once
/// decoded to a full raster + grayscale copy. 24 MP keeps the luma buffer at
/// ~24 MB with headroom for the decoded raster.
const MAX_PIXELS: u64 = 24_000_000;

/// One decoded barcode: the detected symbology and its encoded value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// Human-readable symbology name, e.g. `EAN-13`, `Code 128`.
    pub format: String,
    /// The encoded number/string.
    pub text: String,
}

/// The 1D symbologies `format = "auto"` will try. Deliberately 1D-only — QR /
/// DataMatrix / Aztec / PDF417 are out of scope (use `qr-decode` for QR).
fn oned_formats() -> HashSet<BarcodeFormat> {
    HashSet::from([
        BarcodeFormat::EAN_13,
        BarcodeFormat::EAN_8,
        BarcodeFormat::UPC_A,
        BarcodeFormat::UPC_E,
        BarcodeFormat::CODE_128,
        BarcodeFormat::CODE_39,
        BarcodeFormat::CODE_93,
        BarcodeFormat::CODABAR,
        BarcodeFormat::ITF,
    ])
}

/// Map a user-facing `format` value to the symbology to restrict decoding to.
/// `auto` (or empty) → `None` (try every 1D symbology). Unknown → error.
fn parse_format(format: &str) -> Result<Option<BarcodeFormat>, String> {
    let f = format.trim().to_ascii_lowercase();
    Ok(match f.as_str() {
        "" | "auto" => None,
        "ean-13" | "ean13" => Some(BarcodeFormat::EAN_13),
        "ean-8" | "ean8" => Some(BarcodeFormat::EAN_8),
        "upc-a" | "upca" => Some(BarcodeFormat::UPC_A),
        "code-128" | "code128" => Some(BarcodeFormat::CODE_128),
        "code-39" | "code39" => Some(BarcodeFormat::CODE_39),
        other => {
            return Err(format!(
                "unknown format '{other}': use one of auto, ean-13, ean-8, upc-a, code-128, code-39"
            ))
        }
    })
}

/// Friendly display name for a detected symbology.
fn display_format(f: &BarcodeFormat) -> String {
    match f {
        BarcodeFormat::EAN_13 => "EAN-13",
        BarcodeFormat::EAN_8 => "EAN-8",
        BarcodeFormat::UPC_A => "UPC-A",
        BarcodeFormat::UPC_E => "UPC-E",
        BarcodeFormat::CODE_128 => "Code 128",
        BarcodeFormat::CODE_39 => "Code 39",
        BarcodeFormat::CODE_93 => "Code 93",
        BarcodeFormat::CODABAR => "Codabar",
        BarcodeFormat::ITF => "ITF",
        other => return format!("{other:?}"),
    }
    .to_string()
}

/// Decode every 1D barcode in `image_bytes`.
///
/// * `format` — `auto` (try all 1D symbologies) or one of `ean-13`, `ean-8`,
///   `upc-a`, `code-128`, `code-39` to restrict decoding.
/// * `try_harder` — spend more time (rotations / harder binarization). Slower
///   but reads photos and skewed scans that the fast pass misses.
///
/// Returns each barcode found, in detection order, deduplicated by
/// (format, text). Errors if the image can't be decoded or no barcode is found.
pub fn run(image_bytes: &[u8], format: &str, try_harder: bool) -> Result<Vec<Decoded>, String> {
    let restrict = parse_format(format)?;

    // Header-first size guard so a huge image errors cleanly instead of
    // OOM-trapping the sandbox.
    let reader = image::ImageReader::new(Cursor::new(image_bytes))
        .with_guessed_format()
        .map_err(|e| format!("could not read image: {e}"))?;
    let (w, h) = reader
        .into_dimensions()
        .map_err(|e| format!("could not read image (unsupported or corrupt): {e}"))?;
    if w == 0 || h == 0 {
        return Err("image has zero size".into());
    }
    if (w as u64) * (h as u64) > MAX_PIXELS {
        return Err(format!(
            "image is too large ({w}x{h}); re-export at a smaller resolution (max {MAX_PIXELS} pixels)"
        ));
    }

    let img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("could not decode image: {e}"))?;
    let luma = img.to_luma8();
    let (lw, lh) = (luma.width(), luma.height());
    let buf = luma.into_raw();

    let possible = match &restrict {
        Some(f) => HashSet::from([*f]),
        None => oned_formats(),
    };

    let mut hints = DecodeHints {
        PossibleFormats: Some(possible),
        TryHarder: Some(try_harder),
        ..Default::default()
    };

    // First try to find every barcode in the frame; fall back to a single-code
    // scan if the multi-reader comes up empty (it tiles the image and can miss
    // a clean full-frame code that the plain reader gets).
    let mut found: Vec<Decoded> = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut push = |f: &BarcodeFormat, text: String, out: &mut Vec<Decoded>| {
        let fmt = display_format(f);
        if seen.insert((fmt.clone(), text.clone())) {
            out.push(Decoded { format: fmt, text });
        }
    };

    if let Ok(results) =
        rxing::helpers::detect_multiple_in_luma_with_hints(buf.clone(), lw, lh, &mut hints)
    {
        for r in &results {
            push(r.getBarcodeFormat(), r.getText().to_string(), &mut found);
        }
    }

    if found.is_empty() {
        if let Ok(r) = rxing::helpers::detect_in_luma_with_hints(buf, lw, lh, restrict, &mut hints) {
            push(r.getBarcodeFormat(), r.getText().to_string(), &mut found);
        }
    }

    if found.is_empty() {
        return Err(
            "no 1D barcode found in the image (check the image is clear and high-contrast; \
             use the qr-decode tool for QR codes)"
                .into(),
        );
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rxing::{MultiFormatWriter, Writer};

    /// Render `text` as `fmt` to a PNG with a white quiet-zone border so the
    /// reader has margins to lock onto (mirrors how real barcodes are printed).
    fn barcode_png(text: &str, fmt: BarcodeFormat) -> Vec<u8> {
        let bm = MultiFormatWriter
            .encode(text, &fmt, 500, 200)
            .expect("encode barcode");
        let border = 40u32;
        let (bw, bh) = (bm.getWidth(), bm.getHeight());
        let mut img =
            image::GrayImage::from_pixel(bw + border * 2, bh + border * 2, image::Luma([255u8]));
        for y in 0..bh {
            for x in 0..bw {
                if bm.get(x, y) {
                    img.put_pixel(x + border, y + border, image::Luma([0u8]));
                }
            }
        }
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageLuma8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .expect("encode png");
        buf.into_inner()
    }

    #[test]
    fn decodes_ean13_when_restricted() {
        let png = barcode_png("1234567890128", BarcodeFormat::EAN_13);
        let out = run(&png, "ean-13", true).expect("should decode");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].format, "EAN-13");
        assert_eq!(out[0].text, "1234567890128");
    }

    #[test]
    fn auto_detects_code128() {
        let png = barcode_png("Gizza-128!", BarcodeFormat::CODE_128);
        let out = run(&png, "auto", true).expect("should decode");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].format, "Code 128");
        assert_eq!(out[0].text, "Gizza-128!");
    }

    #[test]
    fn auto_detects_upca_and_code39() {
        let upc = barcode_png("036000291452", BarcodeFormat::UPC_A);
        let out = run(&upc, "auto", true).expect("should decode upc");
        assert_eq!(out[0].format, "UPC-A");
        assert_eq!(out[0].text, "036000291452");

        let c39 = barcode_png("CODE39", BarcodeFormat::CODE_39);
        let out = run(&c39, "code-39", true).expect("should decode code39");
        assert_eq!(out[0].format, "Code 39");
        assert_eq!(out[0].text, "CODE39");
    }

    #[test]
    fn errors_on_image_with_no_barcode() {
        // A plain white 200x200 image has nothing to decode.
        let img = image::GrayImage::from_pixel(200, 200, image::Luma([255u8]));
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageLuma8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        let err = run(&buf.into_inner(), "auto", true).unwrap_err();
        assert!(err.contains("no 1D barcode"), "got: {err}");
    }

    #[test]
    fn errors_on_unknown_format() {
        let png = barcode_png("1234567890128", BarcodeFormat::EAN_13);
        let err = run(&png, "qr", true).unwrap_err();
        assert!(err.contains("unknown format"), "got: {err}");
    }

    #[test]
    fn errors_on_non_image_bytes() {
        let err = run(b"not an image at all", "auto", true).unwrap_err();
        assert!(err.contains("could not read image"), "got: {err}");
    }
}
