//! signature-extract core — pull a handwritten signature off its paper and return
//! a clean transparent PNG (just the ink) suitable for dropping into a PDF / e-sign
//! form.
//!
//! No wafer/wasm-bindgen deps. Pure-Rust `image` crate (decode → luminance
//! knock-out → optional recolor → optional trim → RGBA PNG), so it runs on every
//! backend including the chat Service Worker.
//!
//! Algorithm:
//!   1. Decode to RGB and compute each pixel's luminance (Rec. 601).
//!   2. Knock out the paper: pixels DARKER than the threshold cut-off are ink and
//!      become opaque; lighter pixels fade to transparent. `smooth` ramps the alpha
//!      over a narrow band below the cut-off for anti-aliased edges; otherwise the
//!      alpha is hard 1-bit.
//!   3. `ink`: keep the original stroke color, or recolor every ink pixel to a solid
//!      black / blue / red "wet signature" shade.
//!   4. `trim`: crop the fully-transparent margin down to the signature's bounding box.
//!
//! Targets the dominant case — dark ink on light paper. A busy or coloured
//! background is out of scope for a threshold approach (see the analysis doc).

use std::io::Cursor;

use image::{ImageFormat, Rgba, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ink {
    /// Keep each ink pixel's original colour.
    Original,
    Black,
    Blue,
    Red,
}

pub fn parse_ink(s: &str) -> Result<Ink, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "original" | "keep" | "auto" => Ok(Ink::Original),
        "black" => Ok(Ink::Black),
        "blue" => Ok(Ink::Blue),
        "red" => Ok(Ink::Red),
        other => Err(format!(
            "ink {other:?} not supported (original|black|blue|red)"
        )),
    }
}

impl Ink {
    /// The solid RGB a recolour maps to; `None` keeps the original pixel colour.
    fn rgb(self) -> Option<[u8; 3]> {
        match self {
            Ink::Original => None,
            Ink::Black => Some([0, 0, 0]),
            Ink::Blue => Some([26, 42, 128]),
            Ink::Red => Some([170, 20, 20]),
        }
    }
}

/// Rec. 601 luma of an RGB pixel, 0 (black) .. 255 (white).
fn luma(r: u8, g: u8, b: u8) -> f32 {
    0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32
}

/// Extract the signature from `bytes`.
///
/// * `threshold` 0..100 — ink sensitivity. Maps to a luminance cut-off
///   (`threshold/100 * 255`); pixels darker than the cut-off are treated as ink.
///   Higher keeps fainter / lighter strokes (default 50 ≈ mid-grey).
/// * `ink` — recolour the extracted strokes, or keep their original colour.
/// * `smooth` — anti-alias the edges with a soft alpha ramp (else hard 1-bit alpha).
/// * `trim` — crop the transparent margin to the signature's bounding box.
///
/// Returns RGBA PNG bytes.
pub fn extract_signature(
    bytes: &[u8],
    threshold: u32,
    ink: Ink,
    smooth: bool,
    trim: bool,
) -> Result<Vec<u8>, String> {
    let src = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let rgb = src.to_rgb8();
    let (w, h) = rgb.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimensions".into());
    }

    let cutoff = (threshold.min(100) as f32 / 100.0) * 255.0;
    // Anti-alias band: alpha ramps 0→255 as luma falls from `cutoff` to `cutoff-BAND`.
    const BAND: f32 = 48.0;
    let recolor = ink.rgb();

    let mut out = RgbaImage::new(w, h);
    for (x, y, p) in rgb.enumerate_pixels() {
        let [r, g, b] = p.0;
        let l = luma(r, g, b);
        let alpha = if smooth {
            // 255 when fully below the cut-off, ramping to 0 at the cut-off.
            (((cutoff - l) / BAND) * 255.0).clamp(0.0, 255.0).round() as u8
        } else if l < cutoff {
            255
        } else {
            0
        };
        let [or, og, ob] = recolor.unwrap_or([r, g, b]);
        out.put_pixel(x, y, Rgba([or, og, ob, alpha]));
    }

    let final_img = if trim { trim_transparent(&out) } else { out };

    let mut buf = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(final_img)
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(buf.into_inner())
}

/// Crop away the fully-transparent margin, returning the tight bounding box of any
/// pixel with alpha > 0. If nothing was detected (all transparent) the image is
/// returned unchanged so callers still get a valid PNG.
fn trim_transparent(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (w, h, 0u32, 0u32);
    let mut any = false;
    for (x, y, p) in img.enumerate_pixels() {
        if p[3] > 0 {
            any = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    if !any {
        return img.clone();
    }
    let cw = max_x - min_x + 1;
    let ch = max_y - min_y + 1;
    let mut cropped = RgbaImage::new(cw, ch);
    for y in 0..ch {
        for x in 0..cw {
            cropped.put_pixel(x, y, *img.get_pixel(min_x + x, min_y + y));
        }
    }
    cropped
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, Rgb, RgbImage};

    /// A white "page" with a dark diagonal stroke through the middle third.
    fn signed_page(w: u32, h: u32) -> Vec<u8> {
        let mut img = RgbImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgb([245, 245, 245])); // near-white paper
            }
        }
        // A dark stroke: a thick diagonal band across the centre.
        for x in (w / 4)..(3 * w / 4) {
            let yy = h / 4 + (x - w / 4) * (h / 2) / (w / 2).max(1);
            for dy in 0..3 {
                let y = (yy + dy).min(h - 1);
                img.put_pixel(x, y, Rgb([20, 20, 30])); // dark blue-black ink
            }
        }
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn parse_ink_values() {
        assert_eq!(parse_ink("original").unwrap(), Ink::Original);
        assert_eq!(parse_ink("").unwrap(), Ink::Original);
        assert_eq!(parse_ink("BLACK").unwrap(), Ink::Black);
        assert_eq!(parse_ink("blue").unwrap(), Ink::Blue);
        assert_eq!(parse_ink("red").unwrap(), Ink::Red);
        assert!(parse_ink("green").is_err());
    }

    #[test]
    fn paper_is_transparent_ink_is_opaque() {
        let png = signed_page(80, 60);
        // No trim so coordinates map 1:1 to the input.
        let out = extract_signature(&png, 50, Ink::Original, false, false).unwrap();
        assert_eq!(&out[..4], &[0x89, b'P', b'N', b'G'], "PNG magic");
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (80, 60), "no trim preserves dimensions");
        let rgba = img.to_rgba8();
        // A corner is paper → transparent.
        assert_eq!(rgba.get_pixel(1, 1)[3], 0, "paper must be transparent");
        // Somewhere on the stroke → opaque. Scan the centre column band.
        let inked = rgba.enumerate_pixels().any(|(_, _, p)| p[3] > 0);
        assert!(inked, "at least some ink must be opaque");
    }

    #[test]
    fn recolor_to_black_sets_ink_rgb() {
        let png = signed_page(80, 60);
        let out = extract_signature(&png, 60, Ink::Black, false, false).unwrap();
        let rgba = image::load_from_memory(&out).unwrap().to_rgba8();
        // Every opaque pixel must be pure black.
        for p in rgba.pixels() {
            if p[3] > 0 {
                assert_eq!([p[0], p[1], p[2]], [0, 0, 0], "recolored ink is black");
            }
        }
    }

    #[test]
    fn trim_crops_to_the_signature() {
        let png = signed_page(200, 200);
        let full = image::load_from_memory(&extract_signature(&png, 50, Ink::Original, false, false).unwrap())
            .unwrap();
        let trimmed = image::load_from_memory(&extract_signature(&png, 50, Ink::Original, false, true).unwrap())
            .unwrap();
        assert_eq!(full.dimensions(), (200, 200));
        let (tw, th) = trimmed.dimensions();
        assert!(tw < 200 && th < 200, "trim shrinks the canvas, got {tw}x{th}");
        assert!(tw > 0 && th > 0, "trim keeps the ink");
    }

    #[test]
    fn hard_alpha_is_binary() {
        let png = signed_page(80, 60);
        let out = extract_signature(&png, 50, Ink::Original, false, false).unwrap();
        let rgba = image::load_from_memory(&out).unwrap().to_rgba8();
        for p in rgba.pixels() {
            assert!(p[3] == 0 || p[3] == 255, "hard alpha must be 1-bit, got {}", p[3]);
        }
    }

    #[test]
    fn errors_on_bad_image() {
        assert!(extract_signature(b"not an image", 50, Ink::Original, true, true).is_err());
        assert!(extract_signature(&[], 50, Ink::Black, false, false).is_err());
    }
}
