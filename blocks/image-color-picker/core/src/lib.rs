//! gizza-ai/image-color-picker core — read the color of a pixel in an image. No
//! wafer/wasm-bindgen deps. Pure-Rust `image` crate decode + pixel lookup, plus a
//! small nearest-named-color helper.

use image::GenericImageView;

/// The color sampled at a pixel, plus the source image dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
    pub width: u32,
    pub height: u32,
}

/// Decode `bytes` and read the RGBA color at `(x, y)`. Errors if the image can't
/// be decoded or the coordinate is outside the image.
pub fn pick(bytes: &[u8], x: u32, y: u32) -> Result<Pixel, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (width, height) = img.dimensions();
    if x >= width || y >= height {
        return Err(format!(
            "coordinate ({x}, {y}) is outside the image ({width}x{height})"
        ));
    }
    let p = img.get_pixel(x, y).0;
    Ok(Pixel { r: p[0], g: p[1], b: p[2], a: p[3], width, height })
}

/// `#rrggbb` for the pixel.
pub fn hex(p: &Pixel) -> String {
    format!("#{:02x}{:02x}{:02x}", p.r, p.g, p.b)
}

/// `#rrggbbaa` for the pixel (includes alpha).
pub fn hex_rgba(p: &Pixel) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", p.r, p.g, p.b, p.a)
}

/// A small palette of common color names for a friendly "nearest color" label.
const NAMED: &[(&str, (u8, u8, u8))] = &[
    ("black", (0, 0, 0)),
    ("white", (255, 255, 255)),
    ("red", (255, 0, 0)),
    ("lime", (0, 255, 0)),
    ("blue", (0, 0, 255)),
    ("yellow", (255, 255, 0)),
    ("cyan", (0, 255, 255)),
    ("magenta", (255, 0, 255)),
    ("gray", (128, 128, 128)),
    ("silver", (192, 192, 192)),
    ("maroon", (128, 0, 0)),
    ("olive", (128, 128, 0)),
    ("green", (0, 128, 0)),
    ("purple", (128, 0, 128)),
    ("teal", (0, 128, 128)),
    ("navy", (0, 0, 128)),
    ("orange", (255, 165, 0)),
    ("pink", (255, 192, 203)),
    ("brown", (165, 42, 42)),
];

/// Nearest named color (Euclidean distance in RGB).
pub fn nearest_name(r: u8, g: u8, b: u8) -> &'static str {
    let mut best = NAMED[0].0;
    let mut best_d = i64::MAX;
    for &(name, (nr, ng, nb)) in NAMED {
        let dr = r as i64 - nr as i64;
        let dg = g as i64 - ng as i64;
        let db = b as i64 - nb as i64;
        let d = dr * dr + dg * dg + db * db;
        if d < best_d {
            best_d = d;
            best = name;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn png_pixel(r: u8, g: u8, b: u8, a: u8, w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba([r, g, b, a]));
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn picks_solid_color() {
        let png = png_pixel(255, 0, 0, 255, 4, 3);
        let p = pick(&png, 0, 0).unwrap();
        assert_eq!((p.r, p.g, p.b, p.a), (255, 0, 0, 255));
        assert_eq!((p.width, p.height), (4, 3));
        assert_eq!(hex(&p), "#ff0000");
        assert_eq!(hex_rgba(&p), "#ff0000ff");
    }

    #[test]
    fn picks_with_alpha() {
        let png = png_pixel(16, 32, 48, 128, 2, 2);
        let p = pick(&png, 1, 1).unwrap();
        assert_eq!((p.r, p.g, p.b, p.a), (16, 32, 48, 128));
        assert_eq!(hex_rgba(&p), "#10203080");
    }

    #[test]
    fn out_of_bounds_errors() {
        let png = png_pixel(0, 0, 0, 255, 2, 2);
        assert!(pick(&png, 2, 0).is_err());
        assert!(pick(&png, 0, 5).is_err());
    }

    #[test]
    fn decode_error() {
        assert!(pick(b"not an image", 0, 0).is_err());
    }

    #[test]
    fn nearest_names() {
        assert_eq!(nearest_name(255, 0, 0), "red");
        assert_eq!(nearest_name(0, 0, 0), "black");
        assert_eq!(nearest_name(255, 255, 255), "white");
        assert_eq!(nearest_name(250, 160, 10), "orange");
        assert_eq!(nearest_name(20, 20, 20), "black");
    }
}
