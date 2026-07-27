//! gizza-ai/image-format-validator core — verify an image's REAL format against
//! a claimed one, check decode integrity, and report dimensions / colour depth.
//! Pure-Rust (`image` crate), no wafer/wasm-bindgen deps. Unlike image-info this
//! is a *validator*: it never throws on corrupt or mislabelled input — it returns
//! a structured `valid` / `matches_claim` verdict with a corruption diagnostic.

use image::{ColorType, GenericImageView, ImageFormat};
use serde::Serialize;

/// The outcome of validating one image blob.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// True when the bytes decode cleanly as a recognised raster image.
    pub valid: bool,
    /// Format detected from the leading magic bytes, e.g. "PNG"; "unknown" when
    /// no image signature matches.
    pub detected_format: String,
    /// MIME type of the detected format, e.g. "image/png"; empty when unknown.
    pub detected_mime: String,
    /// File size in bytes.
    pub bytes: usize,

    /// Normalised format the file *claims* to be — from the `claimed_format`
    /// parameter, else the filename extension. Omitted when nothing was claimed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimed_format: Option<String>,
    /// Where the claim came from: "parameter" or "filename". Omitted with no claim.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_source: Option<String>,
    /// Whether the detected format matches the claim (spoof check). Omitted when
    /// nothing was claimed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matches_claim: Option<bool>,

    // Decoded raster properties — present only when `valid` is true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub megapixels: Option<f64>,
    /// Reduced aspect ratio like "16:9".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,
    /// Human colour type, e.g. "RGBA (8-bit)".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bits_per_pixel: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_alpha: Option<bool>,

    /// Diagnostic set when `valid` is false — unrecognised signature, or a known
    /// signature whose payload is corrupt / truncated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corruption: Option<String>,
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a.max(1)
}

/// Friendly display name for a detected format.
fn format_name(f: ImageFormat) -> &'static str {
    match f {
        ImageFormat::Png => "PNG",
        ImageFormat::Jpeg => "JPEG",
        ImageFormat::Gif => "GIF",
        ImageFormat::WebP => "WebP",
        ImageFormat::Bmp => "BMP",
        ImageFormat::Tiff => "TIFF",
        ImageFormat::Ico => "ICO",
        ImageFormat::Avif => "AVIF",
        ImageFormat::Tga => "TGA",
        ImageFormat::Qoi => "QOI",
        _ => "image",
    }
}

/// Canonical comparison token for a detected format.
fn detected_token(f: ImageFormat) -> &'static str {
    match f {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpeg",
        ImageFormat::Gif => "gif",
        ImageFormat::WebP => "webp",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Tiff => "tiff",
        ImageFormat::Ico => "ico",
        ImageFormat::Avif => "avif",
        ImageFormat::Tga => "tga",
        ImageFormat::Qoi => "qoi",
        _ => "other",
    }
}

/// Fold well-known aliases so jpg/jpeg and tif/tiff compare equal.
fn canonical(token: &str) -> &str {
    match token {
        "jpg" | "jpe" | "jfif" => "jpeg",
        "tif" => "tiff",
        "dib" => "bmp",
        "cur" => "ico",
        other => other,
    }
}

fn color_desc(c: ColorType) -> (String, bool) {
    let (name, alpha) = match c {
        ColorType::L8 => ("Grayscale (8-bit)", false),
        ColorType::La8 => ("Grayscale+Alpha (8-bit)", true),
        ColorType::Rgb8 => ("RGB (8-bit)", false),
        ColorType::Rgba8 => ("RGBA (8-bit)", true),
        ColorType::L16 => ("Grayscale (16-bit)", false),
        ColorType::La16 => ("Grayscale+Alpha (16-bit)", true),
        ColorType::Rgb16 => ("RGB (16-bit)", false),
        ColorType::Rgba16 => ("RGBA (16-bit)", true),
        ColorType::Rgb32F => ("RGB (32-bit float)", false),
        ColorType::Rgba32F => ("RGBA (32-bit float)", true),
        _ => ("unknown", false),
    };
    (name.to_string(), alpha)
}

/// Lowercase image extension token from a filename, if it names a known image
/// type. Returns the canonical comparison token (e.g. "jpeg" for `.jpg`).
fn image_ext(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let (_, ext) = base.rsplit_once('.')?;
    let ext = ext.trim().to_ascii_lowercase();
    let tok = match ext.as_str() {
        "png" => "png",
        "jpg" | "jpeg" | "jpe" | "jfif" => "jpeg",
        "gif" => "gif",
        "webp" => "webp",
        "bmp" | "dib" => "bmp",
        "tif" | "tiff" => "tiff",
        "ico" | "cur" => "ico",
        _ => return None,
    };
    Some(tok.to_string())
}

/// Resolve the claimed format: the explicit parameter (anything but "auto")
/// wins; otherwise fall back to the filename extension.
fn resolve_claim(param: Option<&str>, filename: Option<&str>) -> (Option<String>, Option<String>) {
    if let Some(p) = param {
        let p = p.trim().to_ascii_lowercase();
        if !p.is_empty() && p != "auto" {
            return (Some(canonical(&p).to_string()), Some("parameter".to_string()));
        }
    }
    if let Some(name) = filename {
        if let Some(ext) = image_ext(name) {
            return (Some(ext), Some("filename".to_string()));
        }
    }
    (None, None)
}

/// Validate `bytes` against an optional claimed format (`claimed_param`, e.g.
/// "png"/"auto") and/or the source `filename`. Only truly empty input errors;
/// unrecognised or corrupt images resolve to a `valid = false` verdict.
pub fn validate(
    bytes: &[u8],
    claimed_param: Option<&str>,
    filename: Option<&str>,
) -> Result<Report, String> {
    if bytes.is_empty() {
        return Err("input is empty — nothing to validate".into());
    }

    let (claimed_format, claim_source) = resolve_claim(claimed_param, filename);

    let guessed = image::guess_format(bytes).ok();
    let detected_format = guessed.map(format_name).unwrap_or("unknown").to_string();
    let detected_mime = guessed
        .map(|f| f.to_mime_type().to_string())
        .unwrap_or_default();
    let detected_tok = guessed.map(detected_token);

    // Compare canonical tokens; an unknown detection never matches a claim.
    let matches_claim = claimed_format
        .as_deref()
        .map(|c| detected_tok.map(canonical) == Some(canonical(c)));

    let mut report = Report {
        valid: false,
        detected_format,
        detected_mime,
        bytes: bytes.len(),
        claimed_format,
        claim_source,
        matches_claim,
        width: None,
        height: None,
        megapixels: None,
        aspect_ratio: None,
        color_type: None,
        channels: None,
        bits_per_pixel: None,
        has_alpha: None,
        corruption: None,
    };

    if guessed.is_none() {
        report.corruption =
            Some("no known image signature — the bytes are not a recognised image format".into());
        return Ok(report);
    }

    match image::load_from_memory(bytes) {
        Ok(img) => {
            let (w, h) = img.dimensions();
            let ct = img.color();
            let (color_type, has_alpha) = color_desc(ct);
            let g = gcd(w, h);
            report.valid = true;
            report.width = Some(w);
            report.height = Some(h);
            report.megapixels =
                Some(((w as f64 * h as f64) / 1_000_000.0 * 100.0).round() / 100.0);
            report.aspect_ratio = Some(format!("{}:{}", w / g, h / g));
            report.color_type = Some(color_type);
            report.channels = Some(ct.channel_count());
            report.bits_per_pixel = Some(ct.bits_per_pixel());
            report.has_alpha = Some(has_alpha);
        }
        Err(e) => {
            report.corruption = Some(format!(
                "recognised as {} but could not be decoded (corrupt or truncated): {e}",
                report.detected_format
            ));
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
    use std::io::Cursor;

    fn encode(img: DynamicImage, fmt: ImageFormat) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, fmt).unwrap();
        out.into_inner()
    }

    #[test]
    fn valid_png_no_claim() {
        let png = encode(
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 6, Rgba([1, 2, 3, 4]))),
            ImageFormat::Png,
        );
        let r = validate(&png, None, None).unwrap();
        assert!(r.valid);
        assert_eq!(r.detected_format, "PNG");
        assert_eq!(r.detected_mime, "image/png");
        assert_eq!((r.width, r.height), (Some(8), Some(6)));
        assert_eq!(r.channels, Some(4));
        assert_eq!(r.bits_per_pixel, Some(32));
        assert_eq!(r.has_alpha, Some(true));
        assert_eq!(r.color_type.as_deref(), Some("RGBA (8-bit)"));
        assert_eq!(r.aspect_ratio.as_deref(), Some("4:3"));
        assert!(r.claimed_format.is_none());
        assert!(r.matches_claim.is_none());
        assert!(r.corruption.is_none());
    }

    #[test]
    fn param_claim_matches() {
        let png = encode(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(16, 9, Rgb([9, 9, 9]))),
            ImageFormat::Png,
        );
        let r = validate(&png, Some("png"), None).unwrap();
        assert!(r.valid);
        assert_eq!(r.claimed_format.as_deref(), Some("png"));
        assert_eq!(r.claim_source.as_deref(), Some("parameter"));
        assert_eq!(r.matches_claim, Some(true));
        assert_eq!(r.aspect_ratio.as_deref(), Some("16:9"));
    }

    #[test]
    fn param_claim_mismatch_is_spoof() {
        // A real JPEG the caller claims is a PNG.
        let jpg = encode(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(16, 9, Rgb([10, 20, 30]))),
            ImageFormat::Jpeg,
        );
        let r = validate(&jpg, Some("png"), None).unwrap();
        assert!(r.valid, "the bytes still decode — only the claim is wrong");
        assert_eq!(r.detected_format, "JPEG");
        assert_eq!(r.claimed_format.as_deref(), Some("png"));
        assert_eq!(r.claim_source.as_deref(), Some("parameter"));
        assert_eq!(r.matches_claim, Some(false));
    }

    #[test]
    fn filename_claim_mismatch() {
        // JPEG bytes named ".png" (renamed file) — no explicit parameter.
        let jpg = encode(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 4, Rgb([1, 1, 1]))),
            ImageFormat::Jpeg,
        );
        let r = validate(&jpg, Some("auto"), Some("/uploads/photo.png")).unwrap();
        assert_eq!(r.detected_format, "JPEG");
        assert_eq!(r.claimed_format.as_deref(), Some("png"));
        assert_eq!(r.claim_source.as_deref(), Some("filename"));
        assert_eq!(r.matches_claim, Some(false));
    }

    #[test]
    fn filename_jpg_alias_matches_jpeg() {
        let jpg = encode(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 4, Rgb([1, 1, 1]))),
            ImageFormat::Jpeg,
        );
        let r = validate(&jpg, None, Some("holiday.JPG")).unwrap();
        assert_eq!(r.claimed_format.as_deref(), Some("jpeg"));
        assert_eq!(r.claim_source.as_deref(), Some("filename"));
        assert_eq!(r.matches_claim, Some(true));
    }

    #[test]
    fn param_beats_filename() {
        let png = encode(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 4, Rgb([1, 1, 1]))),
            ImageFormat::Png,
        );
        // explicit param "png" wins over the misleading ".gif" name.
        let r = validate(&png, Some("png"), Some("a.gif")).unwrap();
        assert_eq!(r.claim_source.as_deref(), Some("parameter"));
        assert_eq!(r.matches_claim, Some(true));
    }

    #[test]
    fn truncated_png_is_corrupt_not_a_panic() {
        let png = encode(
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(32, 32, Rgba([5, 6, 7, 8]))),
            ImageFormat::Png,
        );
        let truncated = &png[..png.len() / 2]; // signature intact, payload cut
        let r = validate(truncated, Some("png"), None).unwrap();
        assert!(!r.valid);
        assert_eq!(r.detected_format, "PNG", "signature still identifies PNG");
        assert_eq!(r.matches_claim, Some(true), "claim still matches the signature");
        assert!(r.corruption.is_some());
        assert!(r.width.is_none());
    }

    #[test]
    fn unrecognised_bytes() {
        let r = validate(b"this is definitely not an image", Some("png"), None).unwrap();
        assert!(!r.valid);
        assert_eq!(r.detected_format, "unknown");
        assert_eq!(r.detected_mime, "");
        assert_eq!(r.matches_claim, Some(false), "unknown never matches a claim");
        assert!(r.corruption.is_some());
    }

    #[test]
    fn empty_input_errors() {
        assert!(validate(&[], None, None).is_err());
    }

    #[test]
    fn gcd_reduces() {
        assert_eq!(gcd(1920, 1080), 120);
        assert_eq!(gcd(7, 0), 7);
    }
}
