//! gizza-ai/image-rounded-corners core — round the corners of an image at a
//! chosen radius. No wafer/wasm-bindgen deps. Pure-Rust `image` crate.
//!
//! Unlike `image-round-avatar` (which center-crops to a SQUARE), this keeps the
//! full rectangular image and only masks the four corners. The radius can be an
//! absolute pixel value or a percentage of the shorter side (50% fully rounds
//! the short side into a pill/circle). Corners can be rounded selectively
//! (all / top / bottom / left / right). Output is always PNG so the rounded
//! corners can be transparent; a solid `background` color can be filled in
//! instead. Edges are 1px anti-aliased.

use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageFormat};

/// Which corners to round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corners {
    All,
    Top,
    Bottom,
    Left,
    Right,
}

pub fn parse_corners(s: &str) -> Result<Corners, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "all" | "four" => Ok(Corners::All),
        "top" => Ok(Corners::Top),
        "bottom" => Ok(Corners::Bottom),
        "left" => Ok(Corners::Left),
        "right" => Ok(Corners::Right),
        other => Err(format!(
            "corners {other:?} not supported (all|top|bottom|left|right)"
        )),
    }
}

/// How to interpret `radius`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Px,
    Percent,
}

pub fn parse_unit(s: &str) -> Result<Unit, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "px" | "pixel" | "pixels" => Ok(Unit::Px),
        "percent" | "%" | "pct" => Ok(Unit::Percent),
        other => Err(format!("unit {other:?} not supported (px|percent)")),
    }
}

const MAX_DIM: u32 = 8192;

/// Parse a `background` value into an opaque RGB fill, or `None` for a
/// transparent background (the default). Accepts `transparent`/`none`/empty,
/// `#rgb`/`#rrggbb`/`#rrggbbaa` hex, or a small set of CSS color names.
pub fn parse_background(s: &str) -> Result<Option<[u8; 3]>, String> {
    let t = s.trim().to_ascii_lowercase();
    if t.is_empty() || t == "transparent" || t == "none" {
        return Ok(None);
    }
    if let Some(hex) = t.strip_prefix('#') {
        let expand = |c: &str| -> Result<[u8; 3], String> {
            let bytes: Vec<u8> = match c.len() {
                // #rgb / #rgba -> duplicate each nibble
                3 | 4 => c
                    .chars()
                    .take(3)
                    .map(|ch| {
                        let d = ch
                            .to_digit(16)
                            .ok_or_else(|| format!("invalid hex color {s:?}"))?
                            as u8;
                        Ok(d * 16 + d)
                    })
                    .collect::<Result<_, String>>()?,
                // #rrggbb / #rrggbbaa -> take rgb bytes
                6 | 8 => (0..3)
                    .map(|i| {
                        u8::from_str_radix(&c[i * 2..i * 2 + 2], 16)
                            .map_err(|_| format!("invalid hex color {s:?}"))
                    })
                    .collect::<Result<_, String>>()?,
                _ => return Err(format!("invalid hex color {s:?} (use #rgb or #rrggbb)")),
            };
            Ok([bytes[0], bytes[1], bytes[2]])
        };
        return Ok(Some(expand(hex)?));
    }
    let named: Option<[u8; 3]> = match t.as_str() {
        "white" => Some([255, 255, 255]),
        "black" => Some([0, 0, 0]),
        "red" => Some([255, 0, 0]),
        "green" => Some([0, 128, 0]),
        "blue" => Some([0, 0, 255]),
        "yellow" => Some([255, 255, 0]),
        "cyan" | "aqua" => Some([0, 255, 255]),
        "magenta" | "fuchsia" => Some([255, 0, 255]),
        "orange" => Some([255, 165, 0]),
        "purple" => Some([128, 0, 128]),
        "gray" | "grey" => Some([128, 128, 128]),
        "silver" => Some([192, 192, 192]),
        "navy" => Some([0, 0, 128]),
        "teal" => Some([0, 128, 128]),
        "pink" => Some([255, 192, 203]),
        _ => None,
    };
    named
        .map(Some)
        .ok_or_else(|| format!("unknown color {s:?} (use a hex value like #3366ff or a common name)"))
}

fn corner_flags(c: Corners) -> [bool; 4] {
    // order: [top-left, top-right, bottom-left, bottom-right]
    match c {
        Corners::All => [true, true, true, true],
        Corners::Top => [true, true, false, false],
        Corners::Bottom => [false, false, true, true],
        Corners::Left => [true, false, true, false],
        Corners::Right => [false, true, false, true],
    }
}

/// Round the corners of `bytes`. `radius` is interpreted per `unit` (percent is
/// relative to the shorter side; 50% fully rounds it). When `radius` is `None`
/// a subtle default of 12% of the shorter side is used. `background` fills the
/// masked corners with a solid opaque color instead of transparency. Returns
/// PNG bytes at the ORIGINAL image dimensions (never cropped).
pub fn round_corners(
    bytes: &[u8],
    radius: Option<f64>,
    unit: Unit,
    corners: Corners,
    background: Option<[u8; 3]>,
) -> Result<Vec<u8>, String> {
    let src = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = src.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }
    if w > MAX_DIM || h > MAX_DIM {
        return Err(format!(
            "image is {w}x{h}; max supported dimension is {MAX_DIM}px per side"
        ));
    }
    let mut img = src.to_rgba8();

    let short = w.min(h) as f64;
    let rr = match unit {
        Unit::Px => radius.unwrap_or(short * 0.12),
        Unit::Percent => {
            let pct = radius.unwrap_or(12.0).clamp(0.0, 50.0);
            // percent is of the shorter side; 50% => short/2 => fully rounded.
            short * (pct / 50.0) * 0.5
        }
    };
    // Cap at half the shorter side (a full pill/circle on that axis).
    let rr = rr.clamp(0.0, short / 2.0);
    if rr < 0.5 {
        // No visible rounding: still fill/re-encode as PNG for a consistent surface.
        if let Some([br, bg, bb]) = background {
            for p in img.pixels_mut() {
                let a = p.0[3] as f64 / 255.0;
                p.0[0] = (p.0[0] as f64 * a + br as f64 * (1.0 - a)).round() as u8;
                p.0[1] = (p.0[1] as f64 * a + bg as f64 * (1.0 - a)).round() as u8;
                p.0[2] = (p.0[2] as f64 * a + bb as f64 * (1.0 - a)).round() as u8;
                p.0[3] = 255;
            }
        }
        return encode_png(img);
    }

    let flags = corner_flags(corners);
    let rri = rr.ceil() as u32;
    let wf = w as f64;
    let hf = h as f64;

    for y in 0..h {
        for x in 0..w {
            // Locate the corner region this pixel falls in (regions never
            // overlap because rr <= short/2). Pick the arc center accordingly.
            let (in_corner, cx, cy, rounded) = if x < rri && y < rri {
                (true, rr, rr, flags[0])
            } else if x >= w - rri && y < rri {
                (true, wf - rr, rr, flags[1])
            } else if x < rri && y >= h - rri {
                (true, rr, hf - rr, flags[2])
            } else if x >= w - rri && y >= h - rri {
                (true, wf - rr, hf - rr, flags[3])
            } else {
                (false, 0.0, 0.0, false)
            };

            let coverage = if !in_corner || !rounded {
                1.0
            } else {
                let px = x as f64 + 0.5;
                let py = y as f64 + 0.5;
                let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                // 1px anti-aliased edge: 1 inside the arc, 0 outside.
                (rr - d + 0.5).clamp(0.0, 1.0)
            };

            if coverage >= 1.0 && background.is_none() {
                continue;
            }
            let p = img.get_pixel_mut(x, y);
            match background {
                None => {
                    p.0[3] = (p.0[3] as f64 * coverage).round() as u8;
                }
                Some([br, bg, bb]) => {
                    // Composite the (coverage-masked) source over an opaque bg.
                    let fg_a = (p.0[3] as f64 / 255.0) * coverage;
                    p.0[0] = (p.0[0] as f64 * fg_a + br as f64 * (1.0 - fg_a)).round() as u8;
                    p.0[1] = (p.0[1] as f64 * fg_a + bg as f64 * (1.0 - fg_a)).round() as u8;
                    p.0[2] = (p.0[2] as f64 * fg_a + bb as f64 * (1.0 - fg_a)).round() as u8;
                    p.0[3] = 255;
                }
            }
        }
    }

    encode_png(img)
}

fn encode_png(img: image::RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(img)
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
        DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn parse_helpers() {
        assert_eq!(parse_corners("all").unwrap(), Corners::All);
        assert_eq!(parse_corners("").unwrap(), Corners::All);
        assert_eq!(parse_corners("TOP").unwrap(), Corners::Top);
        assert!(parse_corners("diagonal").is_err());
        assert_eq!(parse_unit("px").unwrap(), Unit::Px);
        assert_eq!(parse_unit("percent").unwrap(), Unit::Percent);
        assert!(parse_unit("furlongs").is_err());
        assert_eq!(parse_background("transparent").unwrap(), None);
        assert_eq!(parse_background("").unwrap(), None);
        assert_eq!(parse_background("#f00").unwrap(), Some([255, 0, 0]));
        assert_eq!(parse_background("#3366ff").unwrap(), Some([51, 102, 255]));
        assert_eq!(parse_background("white").unwrap(), Some([255, 255, 255]));
        assert!(parse_background("boguscolor").is_err());
        assert!(parse_background("#12345").is_err());
    }

    #[test]
    fn keeps_original_dimensions_not_cropped() {
        // A wide banner keeps its full rectangle (unlike the avatar tool).
        let png = opaque_png(200, 100);
        let out = round_corners(&png, Some(20.0), Unit::Px, Corners::All, None).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (200, 100), "rectangle preserved");
    }

    #[test]
    fn all_corners_transparent_center_and_edges_kept() {
        let png = opaque_png(100, 100);
        let out = round_corners(&png, Some(20.0), Unit::Px, Corners::All, None).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.get_pixel(0, 0).0[3], 0, "top-left corner transparent");
        assert_eq!(img.get_pixel(99, 0).0[3], 0, "top-right corner transparent");
        assert_eq!(img.get_pixel(0, 99).0[3], 0, "bottom-left corner transparent");
        assert_eq!(img.get_pixel(99, 99).0[3], 0, "bottom-right corner transparent");
        assert_eq!(img.get_pixel(50, 50).0[3], 255, "center opaque");
        // Straight edges (away from corners) are untouched.
        assert_eq!(img.get_pixel(0, 50).0[3], 255, "left-edge midpoint kept");
        assert_eq!(img.get_pixel(50, 0).0[3], 255, "top-edge midpoint kept");
    }

    #[test]
    fn selective_top_rounds_only_top_corners() {
        let png = opaque_png(100, 100);
        let out = round_corners(&png, Some(25.0), Unit::Px, Corners::Top, None).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.get_pixel(0, 0).0[3], 0, "top-left rounded (transparent)");
        assert_eq!(img.get_pixel(99, 0).0[3], 0, "top-right rounded (transparent)");
        assert_eq!(img.get_pixel(0, 99).0[3], 255, "bottom-left square (opaque)");
        assert_eq!(img.get_pixel(99, 99).0[3], 255, "bottom-right square (opaque)");
    }

    #[test]
    fn percent_fifty_fully_rounds_short_side() {
        // 200x100: 50% => radius 50 => the short (100) side fully rounded; the
        // left/right edge midpoints (x=0,y=50) sit on the arc, so ~half-covered
        // and the corners well outside are clear.
        let png = opaque_png(200, 100);
        let out = round_corners(&png, Some(50.0), Unit::Percent, Corners::All, None).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.get_pixel(0, 0).0[3], 0, "corner transparent at full round");
        assert_eq!(img.get_pixel(100, 50).0[3], 255, "horizontal center opaque");
    }

    #[test]
    fn background_fill_makes_corners_opaque_color() {
        let png = opaque_png(100, 100);
        let out =
            round_corners(&png, Some(20.0), Unit::Px, Corners::All, Some([255, 255, 255])).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        let corner = img.get_pixel(0, 0);
        assert_eq!(corner.0[3], 255, "background is opaque, no transparency");
        assert_eq!(
            [corner.0[0], corner.0[1], corner.0[2]],
            [255, 255, 255],
            "corner filled with white background"
        );
        // Center still shows the source color.
        let center = img.get_pixel(50, 50);
        assert_eq!([center.0[0], center.0[1], center.0[2]], [200, 100, 50]);
    }

    #[test]
    fn default_radius_rounds_something() {
        let png = opaque_png(100, 100);
        let out = round_corners(&png, None, Unit::Px, Corners::All, None).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.get_pixel(0, 0).0[3], 0, "default radius rounds the corner");
        assert_eq!(img.get_pixel(50, 50).0[3], 255, "center kept");
    }

    #[test]
    fn errors_on_bad_image() {
        assert!(round_corners(b"not an image", Some(10.0), Unit::Px, Corners::All, None).is_err());
    }
}
