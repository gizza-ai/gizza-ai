//! gizza-ai/image-info core — report an image's format, dimensions, and color
//! type from its bytes. No wafer/wasm-bindgen deps. Pure-Rust `image` crate.

use image::{ColorType, GenericImageView, ImageFormat};

/// Metadata about a decoded image (file size is added by the caller).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageInfo {
    /// Friendly format name, e.g. "PNG".
    pub format: String,
    /// MIME type, e.g. "image/png".
    pub mime: String,
    pub width: u32,
    pub height: u32,
    /// Human color type, e.g. "RGBA (8-bit)".
    pub color_type: String,
    pub channels: u8,
    pub bits_per_pixel: u16,
    pub has_alpha: bool,
    /// Reduced aspect ratio like "16:9".
    pub aspect_ratio: String,
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a.max(1)
}

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

/// Inspect image `bytes`. Errors if the format is unrecognized or undecodable.
pub fn info(bytes: &[u8]) -> Result<ImageInfo, String> {
    if bytes.is_empty() {
        return Err("input is empty".into());
    }
    let format = image::guess_format(bytes).map_err(|_| "unrecognized image format".to_string())?;
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (width, height) = img.dimensions();
    let ct = img.color();
    let (color_type, has_alpha) = color_desc(ct);

    let g = gcd(width, height);
    let aspect_ratio = format!("{}:{}", width / g, height / g);

    Ok(ImageInfo {
        format: format_name(format).to_string(),
        mime: format.to_mime_type().to_string(),
        width,
        height,
        color_type,
        channels: ct.channel_count(),
        bits_per_pixel: ct.bits_per_pixel(),
        has_alpha,
        aspect_ratio,
    })
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
    fn reports_png_rgba() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 6, Rgba([1, 2, 3, 4])));
        let png = encode(img, ImageFormat::Png);
        let i = info(&png).unwrap();
        assert_eq!(i.format, "PNG");
        assert_eq!(i.mime, "image/png");
        assert_eq!((i.width, i.height), (8, 6));
        assert_eq!(i.channels, 4);
        assert_eq!(i.bits_per_pixel, 32);
        assert!(i.has_alpha);
        assert_eq!(i.color_type, "RGBA (8-bit)");
        assert_eq!(i.aspect_ratio, "4:3"); // 8:6 reduced
    }

    #[test]
    fn reports_jpeg_rgb_no_alpha() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(16, 9, Rgb([10, 20, 30])));
        let jpg = encode(img, ImageFormat::Jpeg);
        let i = info(&jpg).unwrap();
        assert_eq!(i.format, "JPEG");
        assert_eq!(i.channels, 3);
        assert!(!i.has_alpha);
        assert_eq!(i.aspect_ratio, "16:9");
    }

    #[test]
    fn gcd_reduces() {
        assert_eq!(gcd(1920, 1080), 120);
        assert_eq!(gcd(7, 0), 7);
    }

    #[test]
    fn errors() {
        assert!(info(&[]).is_err());
        assert!(info(b"not an image at all").is_err());
    }
}
