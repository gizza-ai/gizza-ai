//! gizza-ai/image-round-avatar core — crop an image into a circle or
//! rounded-square avatar with transparent corners. No wafer/wasm-bindgen deps.
//! Pure-Rust `image` crate: center-crop to a square, optionally resize, then
//! apply an anti-aliased circle / rounded-rect alpha mask. Output is always PNG
//! (alpha preserved).

use std::io::Cursor;

use image::{imageops, DynamicImage, GenericImageView, ImageFormat, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Circle,
    Rounded,
}

pub fn parse_shape(s: &str) -> Result<Shape, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "circle" | "round" => Ok(Shape::Circle),
        "rounded" | "rounded-square" | "squircle" => Ok(Shape::Rounded),
        other => Err(format!("shape {other:?} not supported (circle|rounded)")),
    }
}

const MAX_DIM: u32 = 4096;

/// Crop `bytes` to a square avatar of the given `shape`. `size` resizes the
/// square output (default = the cropped square side); `radius` is the corner
/// radius in px for `rounded` (default 20% of the side; ignored for `circle`).
/// Returns PNG bytes.
pub fn avatar(
    bytes: &[u8],
    shape: Shape,
    size: Option<u32>,
    radius: Option<u32>,
) -> Result<Vec<u8>, String> {
    let src = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = src.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }

    // Center-crop to the largest centered square.
    let s = w.min(h);
    let ox = (w - s) / 2;
    let oy = (h - s) / 2;
    let mut square = src.crop_imm(ox, oy, s, s).to_rgba8();

    // Optional resize to a target side.
    if let Some(sz) = size {
        let sz = sz.clamp(1, MAX_DIM);
        if sz != s {
            square = imageops::resize(&square, sz, sz, imageops::FilterType::Lanczos3);
        }
    }
    let dim = square.width();
    let half = dim as f32 / 2.0;

    let rr = match shape {
        Shape::Circle => half,
        Shape::Rounded => {
            let default = (dim as f32 * 0.2).round() as u32;
            radius.unwrap_or(default).min(dim / 2) as f32
        }
    };

    for y in 0..dim {
        for x in 0..dim {
            let px = x as f32 + 0.5 - half;
            let py = y as f32 + 0.5 - half;
            // Signed distance to a rounded rectangle (negative = inside). With
            // rr == half this is exactly a circle.
            let qx = px.abs() - (half - rr);
            let qy = py.abs() - (half - rr);
            let ax = qx.max(0.0);
            let ay = qy.max(0.0);
            let d = (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - rr;
            // 1px anti-aliased coverage: 1 inside, 0 outside.
            let coverage = (0.5 - d).clamp(0.0, 1.0);
            let p = square.get_pixel_mut(x, y);
            p.0[3] = (p.0[3] as f32 * coverage).round() as u8;
        }
    }

    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(square)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn opaque_png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba([200, 100, 50, 255]));
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn parse_shape_values() {
        assert_eq!(parse_shape("circle").unwrap(), Shape::Circle);
        assert_eq!(parse_shape("").unwrap(), Shape::Circle);
        assert_eq!(parse_shape("rounded").unwrap(), Shape::Rounded);
        assert!(parse_shape("triangle").is_err());
    }

    #[test]
    fn circle_clears_corners_keeps_center() {
        let png = opaque_png(100, 100);
        let out = avatar(&png, Shape::Circle, None, None).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (100, 100));
        assert_eq!(img.get_pixel(0, 0).0[3], 0, "corner must be transparent");
        assert_eq!(img.get_pixel(50, 50).0[3], 255, "center must be opaque");
        // A diagonal point near the corner is outside the inscribed circle.
        assert_eq!(img.get_pixel(7, 7).0[3], 0, "diagonal near-corner outside circle");
        // Edge midpoints are inside the inscribed circle (it's tangent to them).
        assert_eq!(img.get_pixel(1, 50).0[3], 255, "edge midpoint inside circle");
    }

    #[test]
    fn rounded_keeps_edges_clears_only_corners() {
        let png = opaque_png(100, 100);
        let out = avatar(&png, Shape::Rounded, None, Some(20)).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.get_pixel(0, 0).0[3], 0, "corner transparent");
        assert_eq!(img.get_pixel(50, 50).0[3], 255, "center opaque");
        // Unlike a circle, the straight-edge midpoint is kept.
        assert_eq!(img.get_pixel(0, 50).0[3], 255, "left-edge midpoint kept");
    }

    #[test]
    fn size_resizes_output() {
        let png = opaque_png(80, 120);
        let out = avatar(&png, Shape::Circle, Some(64), None).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (64, 64), "resized to square size");
    }

    #[test]
    fn non_square_is_center_cropped_square() {
        let png = opaque_png(200, 100);
        let out = avatar(&png, Shape::Circle, None, None).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (100, 100), "cropped to min side");
    }

    #[test]
    fn errors_on_bad_image() {
        assert!(avatar(b"not an image", Shape::Circle, None, None).is_err());
    }
}
