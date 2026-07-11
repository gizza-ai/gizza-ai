//! image-split-overlay core — composite TWO images into ONE static image split
//! along a chosen line (vertical, horizontal, or either diagonal), showing image
//! A on one side of the line and image B on the other. This is the classic
//! before/after reveal, flattened to a single shareable image (not an interactive
//! drag slider). No wafer/wasm-bindgen deps — pure `image` crate.
//!
//! Pipeline: image A defines the output canvas (its native size, capped by a
//! pixel guard); image B is resized to that same canvas (stretch or cover). A
//! per-pixel mask decides side A vs side B from the split geometry; an optional
//! divider line is alpha-composited over the seam.

use std::io::Cursor;

use image::{imageops::FilterType, DynamicImage, ImageFormat, Rgba, RgbaImage};

/// The split line's geometry. `A` is always the "first" side: left (vertical),
/// top (horizontal), or the triangle containing the relevant corner (diagonals).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Vertical seam: image A on the left, image B on the right.
    Vertical,
    /// Horizontal seam: image A on top, image B on the bottom.
    Horizontal,
    /// Diagonal from the top-left corner to the bottom-right corner.
    Diagonal,
    /// Diagonal from the top-right corner to the bottom-left corner.
    DiagonalReverse,
}

pub fn parse_orientation(s: &str) -> Result<Orientation, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "vertical" | "v" => Ok(Orientation::Vertical),
        "horizontal" | "h" => Ok(Orientation::Horizontal),
        "diagonal" | "diag" | "diagonal-tlbr" => Ok(Orientation::Diagonal),
        "diagonal-reverse" | "diagonal_reverse" | "diagonal-trbl" | "antidiagonal" => {
            Ok(Orientation::DiagonalReverse)
        }
        other => Err(format!(
            "orientation {other:?} not supported (vertical|horizontal|diagonal|diagonal-reverse)"
        )),
    }
}

/// How image B is fitted to image A's canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Resize B to A's exact dimensions (may distort if aspect ratios differ).
    Stretch,
    /// Scale B to fill A's canvas and center-crop the overflow (aspect kept).
    Cover,
}

pub fn parse_fit(s: &str) -> Result<Fit, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "stretch" => Ok(Fit::Stretch),
        "cover" | "crop" => Ok(Fit::Cover),
        other => Err(format!("fit {other:?} not supported (stretch|cover)")),
    }
}

/// Output encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutFormat {
    Png,
    Jpeg,
}

pub fn parse_format(s: &str) -> Result<OutFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "png" => Ok(OutFormat::Png),
        "jpeg" | "jpg" => Ok(OutFormat::Jpeg),
        other => Err(format!("format {other:?} not supported (png|jpeg)")),
    }
}

impl OutFormat {
    pub fn mime(self) -> &'static str {
        match self {
            OutFormat::Png => "image/png",
            OutFormat::Jpeg => "image/jpeg",
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            OutFormat::Png => "png",
            OutFormat::Jpeg => "jpg",
        }
    }
}

/// Parse `#rgb`, `#rrggbb`, or `#rrggbbaa` (with or without `#`). Empty → white.
pub fn parse_color(s: &str) -> Result<[u8; 4], String> {
    let h = s.trim().trim_start_matches('#');
    let p = |a: &str| u8::from_str_radix(a, 16).map_err(|_| format!("invalid color '{s}'"));
    match h.len() {
        0 => Ok([255, 255, 255, 255]),
        3 => {
            let r = p(&h[0..1])?;
            let g = p(&h[1..2])?;
            let b = p(&h[2..3])?;
            Ok([r * 17, g * 17, b * 17, 255])
        }
        6 => Ok([p(&h[0..2])?, p(&h[2..4])?, p(&h[4..6])?, 255]),
        8 => Ok([p(&h[0..2])?, p(&h[2..4])?, p(&h[4..6])?, p(&h[6..8])?]),
        _ => Err(format!("invalid color '{s}' (use #rgb, #rrggbb, or #rrggbbaa)")),
    }
}

const MAX_DIM: u32 = 5000;
const MAX_PIXELS: u64 = 40_000_000; // ~40 MP output guard
const MAX_DIVIDER: u32 = 400;

/// Source-over alpha compositing of `top` onto `base` (both straight RGBA).
fn over(base: Rgba<u8>, top: Rgba<u8>) -> Rgba<u8> {
    let ta = top[3] as f32 / 255.0;
    if ta >= 1.0 {
        return top;
    }
    if ta <= 0.0 {
        return base;
    }
    let ba = base[3] as f32 / 255.0;
    let out_a = ta + ba * (1.0 - ta);
    if out_a <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let mix = |t: u8, b: u8| {
        let v = (t as f32 * ta + b as f32 * ba * (1.0 - ta)) / out_a;
        v.round().clamp(0.0, 255.0) as u8
    };
    Rgba([
        mix(top[0], base[0]),
        mix(top[1], base[1]),
        mix(top[2], base[2]),
        (out_a * 255.0).round().clamp(0.0, 255.0) as u8,
    ])
}

/// Compute the capped canvas size from image A's native dimensions.
fn canvas_size(aw: u32, ah: u32) -> (u32, u32) {
    let mut scale = 1.0f64;
    if aw > MAX_DIM {
        scale = scale.min(MAX_DIM as f64 / aw as f64);
    }
    if ah > MAX_DIM {
        scale = scale.min(MAX_DIM as f64 / ah as f64);
    }
    let px = aw as u64 * ah as u64;
    if px > MAX_PIXELS {
        scale = scale.min((MAX_PIXELS as f64 / px as f64).sqrt());
    }
    let w = ((aw as f64 * scale) as u32).max(1);
    let h = ((ah as f64 * scale) as u32).max(1);
    (w, h)
}

/// Composite two decoded images into a single split-overlay image.
///
/// * `position` — 0..=100. For vertical/horizontal it is the seam's percentage
///   across the width/height. For diagonals it slides the line perpendicular
///   between the two corners (0 = all B, 50 = corner-to-corner, 100 = all A).
/// * `divider_width` — pixel width of the seam line (0 = none).
#[allow(clippy::too_many_arguments)]
pub fn split_overlay(
    a: DynamicImage,
    b: DynamicImage,
    orientation: Orientation,
    position: f64,
    fit: Fit,
    divider_width: u32,
    divider_color: [u8; 4],
    format: OutFormat,
) -> Result<Vec<u8>, String> {
    let (w, h) = canvas_size(a.width().max(1), a.height().max(1));

    let a_rgba = if a.width() == w && a.height() == h {
        a.to_rgba8()
    } else {
        a.resize_exact(w, h, FilterType::Lanczos3).to_rgba8()
    };
    let b_rgba = match fit {
        Fit::Stretch => b.resize_exact(w, h, FilterType::Lanczos3),
        Fit::Cover => b.resize_to_fill(w, h, FilterType::Lanczos3),
    }
    .to_rgba8();

    let p = (position.clamp(0.0, 100.0)) / 100.0;
    let dw = divider_width.min(MAX_DIVIDER);
    let half = dw as f64 / 2.0;
    let dcol = Rgba(divider_color);

    let wf = w as f64;
    let hf = h as f64;
    let nlen = (wf * wf + hf * hf).sqrt().max(1.0);
    // Half-range of the diagonal signed distance (see the `match` below).
    let diag_range = hf * wf / nlen;

    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let xf = x as f64;
            let yf = y as f64;
            // `value` is a signed perpendicular distance (in pixels) from the
            // split line; `thresh` is the line's offset. side A where value < thresh.
            let (value, thresh) = match orientation {
                Orientation::Vertical => (xf, p * wf),
                Orientation::Horizontal => (yf, p * hf),
                Orientation::Diagonal => {
                    ((hf * xf - wf * yf) / nlen, (p - 0.5) * 2.0 * diag_range)
                }
                Orientation::DiagonalReverse => (
                    (hf * xf + wf * yf - hf * wf) / nlen,
                    (p - 0.5) * 2.0 * diag_range,
                ),
            };
            let base = if value < thresh {
                *a_rgba.get_pixel(x, y)
            } else {
                *b_rgba.get_pixel(x, y)
            };
            let px = if dw > 0 && (value - thresh).abs() <= half {
                over(base, dcol)
            } else {
                base
            };
            out.put_pixel(x, y, px);
        }
    }

    encode(out, format)
}

fn encode(img: RgbaImage, format: OutFormat) -> Result<Vec<u8>, String> {
    let mut buf = Cursor::new(Vec::new());
    match format {
        OutFormat::Png => DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .map_err(|e| format!("PNG encode failed: {e}"))?,
        OutFormat::Jpeg => {
            // JPEG has no alpha; drop it (transparent areas become opaque).
            let rgb = DynamicImage::ImageRgba8(img).to_rgb8();
            DynamicImage::ImageRgb8(rgb)
                .write_to(&mut buf, ImageFormat::Jpeg)
                .map_err(|e| format!("JPEG encode failed: {e}"))?;
        }
    }
    Ok(buf.into_inner())
}

/// Decode two raw image byte buffers, then composite them.
#[allow(clippy::too_many_arguments)]
pub fn split_overlay_from_bytes(
    a: &[u8],
    b: &[u8],
    orientation: Orientation,
    position: f64,
    fit: Fit,
    divider_width: u32,
    divider_color: [u8; 4],
    format: OutFormat,
) -> Result<Vec<u8>, String> {
    let a = image::load_from_memory(a).map_err(|e| format!("image A could not be decoded: {e}"))?;
    let b = image::load_from_memory(b).map_err(|e| format!("image B could not be decoded: {e}"))?;
    split_overlay(a, b, orientation, position, fit, divider_width, divider_color, format)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, c: [u8; 4]) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, Rgba(c)))
    }
    fn png_bytes(img: &DynamicImage) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    const RED: [u8; 4] = [255, 0, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    #[test]
    fn parse_orientation_values() {
        assert_eq!(parse_orientation("").unwrap(), Orientation::Vertical);
        assert_eq!(parse_orientation("Vertical").unwrap(), Orientation::Vertical);
        assert_eq!(parse_orientation("horizontal").unwrap(), Orientation::Horizontal);
        assert_eq!(parse_orientation("diagonal").unwrap(), Orientation::Diagonal);
        assert_eq!(
            parse_orientation("diagonal-reverse").unwrap(),
            Orientation::DiagonalReverse
        );
        assert!(parse_orientation("triangle").is_err());
    }

    #[test]
    fn parse_fit_and_format_and_color() {
        assert_eq!(parse_fit("").unwrap(), Fit::Stretch);
        assert_eq!(parse_fit("cover").unwrap(), Fit::Cover);
        assert!(parse_fit("nope").is_err());
        assert_eq!(parse_format("").unwrap(), OutFormat::Png);
        assert_eq!(parse_format("JPG").unwrap(), OutFormat::Jpeg);
        assert!(parse_format("gif").is_err());
        assert_eq!(parse_color("").unwrap(), [255, 255, 255, 255]);
        assert_eq!(parse_color("#000").unwrap(), [0, 0, 0, 255]);
        assert_eq!(parse_color("ff8800").unwrap(), [255, 136, 0, 255]);
        assert_eq!(parse_color("#00000080").unwrap(), [0, 0, 0, 128]);
        assert!(parse_color("#zzz").is_err());
    }

    #[test]
    fn canvas_matches_image_a_dimensions() {
        // Output canvas comes from A (40x30), B (20x20) is fitted to it.
        let a = solid(40, 30, RED);
        let b = solid(20, 20, BLUE);
        let png = split_overlay(
            a, b, Orientation::Vertical, 50.0, Fit::Stretch, 0, [255, 255, 255, 255], OutFormat::Png,
        )
        .unwrap();
        let decoded = image::load_from_memory(&png).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (40, 30));
    }

    #[test]
    fn vertical_split_reveals_a_left_b_right() {
        let a = solid(100, 20, RED);
        let b = solid(100, 20, BLUE);
        let png = split_overlay(
            a, b, Orientation::Vertical, 50.0, Fit::Stretch, 0, [255, 255, 255, 255], OutFormat::Png,
        )
        .unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(10, 10).0, RED, "left should be image A");
        assert_eq!(img.get_pixel(90, 10).0, BLUE, "right should be image B");
    }

    #[test]
    fn horizontal_split_reveals_a_top_b_bottom() {
        let a = solid(20, 100, RED);
        let b = solid(20, 100, BLUE);
        let png = split_overlay(
            a, b, Orientation::Horizontal, 50.0, Fit::Stretch, 0, [0, 0, 0, 255], OutFormat::Png,
        )
        .unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(10, 10).0, RED, "top should be image A");
        assert_eq!(img.get_pixel(10, 90).0, BLUE, "bottom should be image B");
    }

    #[test]
    fn diagonal_split_corners() {
        let a = solid(80, 80, RED);
        let b = solid(80, 80, BLUE);
        let png = split_overlay(
            a, b, Orientation::Diagonal, 50.0, Fit::Stretch, 0, [0, 0, 0, 255], OutFormat::Png,
        )
        .unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        // tlbr line: lower-left triangle is A, upper-right is B.
        assert_eq!(img.get_pixel(5, 75).0, RED, "bottom-left is image A");
        assert_eq!(img.get_pixel(75, 5).0, BLUE, "top-right is image B");
    }

    #[test]
    fn position_shifts_the_seam() {
        let a = solid(100, 20, RED);
        let b = solid(100, 20, BLUE);
        // Seam at 20% → x=30 is on B's side already.
        let png = split_overlay(
            a, b, Orientation::Vertical, 20.0, Fit::Stretch, 0, [0, 0, 0, 255], OutFormat::Png,
        )
        .unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(10, 10).0, RED);
        assert_eq!(img.get_pixel(30, 10).0, BLUE);
    }

    #[test]
    fn divider_line_is_drawn_on_the_seam() {
        let a = solid(100, 20, RED);
        let b = solid(100, 20, BLUE);
        let green = [0, 255, 0, 255];
        let png = split_overlay(
            a, b, Orientation::Vertical, 50.0, Fit::Stretch, 6, green, OutFormat::Png,
        )
        .unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(50, 10).0, green, "seam pixel should be the divider color");
        assert_eq!(img.get_pixel(10, 10).0, RED);
        assert_eq!(img.get_pixel(90, 10).0, BLUE);
    }

    #[test]
    fn jpeg_output_is_valid_jpeg() {
        let a = solid(40, 40, RED);
        let b = solid(40, 40, BLUE);
        let jpg = split_overlay(
            a, b, Orientation::Vertical, 50.0, Fit::Stretch, 0, [0, 0, 0, 255], OutFormat::Jpeg,
        )
        .unwrap();
        assert_eq!(&jpg[0..2], &[0xFF, 0xD8], "JPEG SOI marker");
        let decoded = image::load_from_memory(&jpg).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (40, 40));
    }

    #[test]
    fn from_bytes_decodes_both() {
        let a = png_bytes(&solid(16, 16, RED));
        let b = png_bytes(&solid(16, 16, BLUE));
        let png = split_overlay_from_bytes(
            &a, &b, Orientation::Vertical, 50.0, Fit::Stretch, 0, [0, 0, 0, 255], OutFormat::Png,
        )
        .unwrap();
        let decoded = image::load_from_memory(&png).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (16, 16));
    }

    #[test]
    fn from_bytes_rejects_garbage() {
        let good = png_bytes(&solid(16, 16, RED));
        let err = split_overlay_from_bytes(
            b"not an image",
            &good,
            Orientation::Vertical,
            50.0,
            Fit::Stretch,
            0,
            [0, 0, 0, 255],
            OutFormat::Png,
        )
        .unwrap_err();
        assert!(err.contains("image A could not be decoded"), "got: {err}");
    }
}
