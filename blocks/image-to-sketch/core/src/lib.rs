//! gizza-ai/image-to-sketch core — turn a photo into a pencil or line-art sketch.
//! No wafer/wasm-bindgen deps. Pure-Rust `image` crate (decode → grayscale →
//! filter → PNG encode), so it runs on every backend including the chat SW.
//!
//! Two looks:
//! * `pencil`  — the classic "color-dodge" pencil-shading effect: grayscale, blur
//!   the inverted gray, then color-dodge blend it back over the gray. Produces a
//!   soft, hand-shaded graphite look on a near-white background.
//! * `lineart` — a clean black-on-white outline drawing from a Sobel edge map
//!   (inverted), good for colouring-page / contour-style sketches.
//!
//! `strength` controls the blur radius (pencil) or the edge sensitivity
//! (lineart); `0` picks a sensible default. Output is always a grayscale PNG.

use std::io::Cursor;

use image::{DynamicImage, GrayImage, ImageFormat, Luma};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Pencil,
    LineArt,
}

pub fn parse_mode(s: &str) -> Result<Mode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "pencil" | "graphite" | "sketch" => Ok(Mode::Pencil),
        "lineart" | "line-art" | "line" | "outline" | "contour" => Ok(Mode::LineArt),
        other => Err(format!("mode {other:?} not supported (pencil|lineart)")),
    }
}

/// Convert `bytes` into a sketch. `strength` is the pencil blur radius (in px) or
/// the line-art edge sensitivity; `0` uses a default. Returns grayscale PNG bytes.
pub fn sketch(bytes: &[u8], mode: Mode, strength: f32) -> Result<Vec<u8>, String> {
    let src = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let gray: GrayImage = src.to_luma8();
    if gray.width() == 0 || gray.height() == 0 {
        return Err("image has zero dimensions".into());
    }

    let out = match mode {
        Mode::Pencil => pencil(&gray, strength),
        Mode::LineArt => lineart(&gray, strength),
    };

    let mut buf = Cursor::new(Vec::new());
    DynamicImage::ImageLuma8(out)
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(buf.into_inner())
}

/// Pencil-shading: gray color-dodged with a blurred, inverted copy of itself.
/// dodge(base, mask) = base == 255 ? 255 : min(255, base*256/(255-mask)).
fn pencil(gray: &GrayImage, strength: f32) -> GrayImage {
    let sigma = if strength <= 0.0 { 8.0 } else { strength }.clamp(0.5, 100.0);
    // Invert, blur, then dodge-blend the blurred invert over the gray.
    let inverted = invert(gray);
    let blurred = image::imageops::blur(&inverted, sigma);

    let (w, h) = (gray.width(), gray.height());
    let mut out = GrayImage::new(w, h);
    for (x, y, p) in gray.enumerate_pixels() {
        let base = p[0] as u32;
        let mask = blurred.get_pixel(x, y)[0] as u32;
        let v = if mask >= 255 {
            255
        } else {
            ((base * 256) / (255 - mask)).min(255)
        };
        out.put_pixel(x, y, Luma([v as u8]));
    }
    out
}

/// Line-art: invert a Sobel gradient-magnitude edge map so edges are dark on a
/// white page. `strength` scales the gradient (higher = thicker/darker lines).
fn lineart(gray: &GrayImage, strength: f32) -> GrayImage {
    let scale = if strength <= 0.0 { 1.0 } else { strength }.clamp(0.1, 16.0);
    let (w, h) = (gray.width(), gray.height());
    let at = |x: i64, y: i64| -> i64 {
        let xc = x.clamp(0, w as i64 - 1) as u32;
        let yc = y.clamp(0, h as i64 - 1) as u32;
        gray.get_pixel(xc, yc)[0] as i64
    };
    let mut out = GrayImage::new(w, h);
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            // Sobel kernels.
            let gx = -at(x - 1, y - 1) - 2 * at(x - 1, y) - at(x - 1, y + 1)
                + at(x + 1, y - 1)
                + 2 * at(x + 1, y)
                + at(x + 1, y + 1);
            let gy = -at(x - 1, y - 1) - 2 * at(x, y - 1) - at(x + 1, y - 1)
                + at(x - 1, y + 1)
                + 2 * at(x, y + 1)
                + at(x + 1, y + 1);
            let mag = (((gx * gx + gy * gy) as f64).sqrt() * scale as f64).min(255.0);
            // Invert so strong edges (high mag) become dark lines on white.
            let v = (255.0 - mag).round() as u8;
            out.put_pixel(x as u32, y as u32, Luma([v]));
        }
    }
    out
}

fn invert(gray: &GrayImage) -> GrayImage {
    let mut out = gray.clone();
    for p in out.pixels_mut() {
        p[0] = 255 - p[0];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};

    /// A PNG with a sharp black/white vertical split — guarantees a strong edge.
    fn split_png(w: u32, h: u32) -> Vec<u8> {
        let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));
        for y in 0..h {
            for x in 0..w / 2 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn parse_mode_values() {
        assert_eq!(parse_mode("pencil").unwrap(), Mode::Pencil);
        assert_eq!(parse_mode("").unwrap(), Mode::Pencil);
        assert_eq!(parse_mode("lineart").unwrap(), Mode::LineArt);
        assert_eq!(parse_mode("outline").unwrap(), Mode::LineArt);
        assert!(parse_mode("watercolor").is_err());
    }

    #[test]
    fn pencil_preserves_dimensions_and_is_decodable() {
        let png = split_png(32, 24);
        let out = sketch(&png, Mode::Pencil, 0.0).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (32, 24));
        // PNG magic.
        assert_eq!(&out[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn pencil_background_is_light() {
        // A flat region away from the edge should dodge to near-white.
        let png = split_png(40, 40);
        let out = sketch(&png, Mode::Pencil, 4.0).unwrap();
        let img = image::load_from_memory(&out).unwrap().to_luma8();
        // Far inside the white half (no nearby edge) → near white.
        assert!(img.get_pixel(38, 20)[0] > 200, "white interior should stay light");
    }

    #[test]
    fn lineart_marks_the_edge_dark() {
        let png = split_png(40, 40);
        let out = sketch(&png, Mode::LineArt, 0.0).unwrap();
        let img = image::load_from_memory(&out).unwrap().to_luma8();
        // The boundary column (~x=20) should be a dark line.
        let mut min_at_edge = 255u8;
        for x in 18..=21 {
            min_at_edge = min_at_edge.min(img.get_pixel(x, 20)[0]);
        }
        assert!(min_at_edge < 100, "edge should produce a dark line, got {min_at_edge}");
        // A flat interior far from the edge should stay white.
        assert!(img.get_pixel(38, 20)[0] > 200, "flat area should stay white");
    }

    #[test]
    fn errors_on_bad_image() {
        assert!(sketch(b"not an image", Mode::Pencil, 0.0).is_err());
        assert!(sketch(&[], Mode::LineArt, 0.0).is_err());
    }
}
