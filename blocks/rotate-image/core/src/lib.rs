//! gizza-ai/rotate-image core — rotate an image by 90/180/270 degrees (lossless)
//! or by an arbitrary angle (bilinear resample into an enlarged canvas, with a
//! configurable background fill for the exposed corners).
//! No wafer/wasm-bindgen deps. Pure-Rust `image`. Returns PNG bytes.

use std::io::Cursor;

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

/// Parse a `#rrggbb`, `#rrggbbaa`, `#rgb`, or `transparent` background color.
/// Returns an RGBA pixel. Empty / "transparent" => fully transparent.
pub fn parse_color(s: &str) -> Result<Rgba<u8>, String> {
    let t = s.trim().to_ascii_lowercase();
    if t.is_empty() || t == "transparent" || t == "none" {
        return Ok(Rgba([0, 0, 0, 0]));
    }
    if t == "white" {
        return Ok(Rgba([255, 255, 255, 255]));
    }
    if t == "black" {
        return Ok(Rgba([0, 0, 0, 255]));
    }
    let hex = t.strip_prefix('#').unwrap_or(&t);
    let parse2 = |h: &str| -> Result<u8, String> {
        u8::from_str_radix(h, 16).map_err(|_| format!("invalid hex color '{s}'"))
    };
    match hex.len() {
        3 => {
            let r = parse2(&hex[0..1].repeat(2))?;
            let g = parse2(&hex[1..2].repeat(2))?;
            let b = parse2(&hex[2..3].repeat(2))?;
            Ok(Rgba([r, g, b, 255]))
        }
        6 => {
            let r = parse2(&hex[0..2])?;
            let g = parse2(&hex[2..4])?;
            let b = parse2(&hex[4..6])?;
            Ok(Rgba([r, g, b, 255]))
        }
        8 => {
            let r = parse2(&hex[0..2])?;
            let g = parse2(&hex[2..4])?;
            let b = parse2(&hex[4..6])?;
            let a = parse2(&hex[6..8])?;
            Ok(Rgba([r, g, b, a]))
        }
        _ => Err(format!(
            "invalid background '{s}' (use #rgb, #rrggbb, #rrggbbaa, transparent, white, or black)"
        )),
    }
}

/// Normalize an angle (degrees, clockwise) into [0, 360).
fn norm_angle(deg: f64) -> f64 {
    let mut a = deg % 360.0;
    if a < 0.0 {
        a += 360.0;
    }
    a
}

/// Rotate `bytes` clockwise by `degrees`. 90/180/270 are lossless; other angles
/// resample with bilinear interpolation into an enlarged canvas, filling exposed
/// corners with `background`. Returns PNG bytes.
pub fn rotate(bytes: &[u8], degrees: f64, background: Rgba<u8>) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let a = norm_angle(degrees);

    let rotated: DynamicImage = if a.abs() < f64::EPSILON {
        img
    } else if (a - 90.0).abs() < f64::EPSILON {
        img.rotate90()
    } else if (a - 180.0).abs() < f64::EPSILON {
        img.rotate180()
    } else if (a - 270.0).abs() < f64::EPSILON {
        img.rotate270()
    } else {
        DynamicImage::ImageRgba8(rotate_arbitrary(&img.to_rgba8(), a, background))
    };

    let mut out = Cursor::new(Vec::new());
    rotated
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

/// Rotate an RGBA image clockwise by an arbitrary angle (degrees) using inverse
/// mapping + bilinear sampling. The output canvas is enlarged to contain the
/// whole rotated image; pixels outside the source are set to `background`.
fn rotate_arbitrary(src: &RgbaImage, degrees: f64, background: Rgba<u8>) -> RgbaImage {
    let (sw, sh) = (src.width() as f64, src.height() as f64);
    let theta = degrees.to_radians();
    let (sin, cos) = (theta.sin(), theta.cos());

    // New bounding box that contains the rotated image.
    let new_w = (sw * cos.abs() + sh * sin.abs()).ceil().max(1.0);
    let new_h = (sw * sin.abs() + sh * cos.abs()).ceil().max(1.0);
    let (ow, oh) = (new_w as u32, new_h as u32);

    let mut out = RgbaImage::from_pixel(ow, oh, background);

    let (scx, scy) = (sw / 2.0, sh / 2.0);
    let (ocx, ocy) = (new_w / 2.0, new_h / 2.0);

    for oy in 0..oh {
        for ox in 0..ow {
            // Output pixel center relative to output center.
            let dx = ox as f64 + 0.5 - ocx;
            let dy = oy as f64 + 0.5 - ocy;
            // Inverse rotation maps output -> source coords.
            let sx = cos * dx + sin * dy + scx;
            let sy = -sin * dx + cos * dy + scy;
            if let Some(px) = sample_bilinear(src, sx - 0.5, sy - 0.5, background) {
                out.put_pixel(ox, oy, px);
            }
        }
    }
    out
}

/// Bilinear sample at floating source coords. Returns None if fully outside.
fn sample_bilinear(src: &RgbaImage, x: f64, y: f64, background: Rgba<u8>) -> Option<Rgba<u8>> {
    let (w, h) = (src.width() as i64, src.height() as i64);
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    // Reject if the whole 2x2 footprint lies outside the source.
    if x1 < 0 || y1 < 0 || x0 >= w || y0 >= h {
        return None;
    }
    let fx = x - x0 as f64;
    let fy = y - y0 as f64;
    let at = |xx: i64, yy: i64| -> [f64; 4] {
        if xx < 0 || yy < 0 || xx >= w || yy >= h {
            let b = background.0;
            [b[0] as f64, b[1] as f64, b[2] as f64, b[3] as f64]
        } else {
            let p = src.get_pixel(xx as u32, yy as u32).0;
            [p[0] as f64, p[1] as f64, p[2] as f64, p[3] as f64]
        }
    };
    let p00 = at(x0, y0);
    let p10 = at(x1, y0);
    let p01 = at(x0, y1);
    let p11 = at(x1, y1);
    let mut out = [0u8; 4];
    for c in 0..4 {
        let top = p00[c] * (1.0 - fx) + p10[c] * fx;
        let bot = p01[c] * (1.0 - fx) + p11[c] * fx;
        let v = top * (1.0 - fy) + bot * fy;
        out[c] = v.round().clamp(0.0, 255.0) as u8;
    }
    Some(Rgba(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    fn png(w: u32, h: u32, f: impl Fn(u32, u32) -> Rgba<u8>) -> Vec<u8> {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, f(x, y));
            }
        }
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn rotate90_swaps_dims_and_moves_pixel() {
        let red = Rgba([255, 0, 0, 255]);
        let blue = Rgba([0, 0, 255, 255]);
        let bytes = png(2, 1, |x, _| if x == 0 { red } else { blue });
        let out = rotate(&bytes, 90.0, Rgba([0, 0, 0, 0])).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (1, 2));
        // 90° clockwise: top-left red goes to top.
        assert_eq!(img.get_pixel(0, 0), red);
        assert_eq!(img.get_pixel(0, 1), blue);
    }

    #[test]
    fn rotate180_flips_both() {
        let red = Rgba([255, 0, 0, 255]);
        let blue = Rgba([0, 0, 255, 255]);
        let bytes = png(2, 1, |x, _| if x == 0 { red } else { blue });
        let out = rotate(&bytes, 180.0, Rgba([0, 0, 0, 0])).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (2, 1));
        assert_eq!(img.get_pixel(0, 0), blue);
        assert_eq!(img.get_pixel(1, 0), red);
    }

    #[test]
    fn rotate270_dims() {
        let bytes = png(3, 2, |_, _| Rgba([10, 20, 30, 255]));
        let out = rotate(&bytes, 270.0, Rgba([0, 0, 0, 0])).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (2, 3));
    }

    #[test]
    fn zero_and_360_are_noops() {
        let bytes = png(4, 3, |x, y| Rgba([x as u8, y as u8, 0, 255]));
        for ang in [0.0, 360.0, -360.0] {
            let out = rotate(&bytes, ang, Rgba([0, 0, 0, 0])).unwrap();
            let img = image::load_from_memory(&out).unwrap();
            assert_eq!(img.dimensions(), (4, 3));
            assert_eq!(img.get_pixel(2, 1), Rgba([2, 1, 0, 255]));
        }
    }

    #[test]
    fn arbitrary_45_enlarges_canvas_and_fills_corners() {
        let solid = Rgba([0, 200, 0, 255]);
        let bytes = png(10, 10, |_, _| solid);
        let bg = Rgba([255, 0, 0, 255]);
        let out = rotate(&bytes, 45.0, bg).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        let (w, h) = img.dimensions();
        // 10*sqrt(2) ~= 14.14 -> ~15 each side.
        assert!((14..=16).contains(&w), "w={w}");
        assert!((14..=16).contains(&h), "h={h}");
        // Corner is exposed background.
        assert_eq!(img.get_pixel(0, 0), bg);
        // Center is still the source color.
        assert_eq!(img.get_pixel(w / 2, h / 2), solid);
    }

    #[test]
    fn negative_angle_normalizes() {
        let bytes = png(3, 2, |_, _| Rgba([1, 2, 3, 255]));
        // -90 == 270 -> 2x3.
        let out = rotate(&bytes, -90.0, Rgba([0, 0, 0, 0])).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (2, 3));
    }

    #[test]
    fn color_parsing() {
        assert_eq!(parse_color("#fff").unwrap(), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_color("#ff0000").unwrap(), Rgba([255, 0, 0, 255]));
        assert_eq!(parse_color("#00ff0080").unwrap(), Rgba([0, 255, 0, 128]));
        assert_eq!(parse_color("transparent").unwrap(), Rgba([0, 0, 0, 0]));
        assert_eq!(parse_color("").unwrap(), Rgba([0, 0, 0, 0]));
        assert_eq!(parse_color("white").unwrap(), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_color("black").unwrap(), Rgba([0, 0, 0, 255]));
        assert!(parse_color("#zzz").is_err());
        assert!(parse_color("notacolor").is_err());
    }

    #[test]
    fn errors_on_bad_image() {
        assert!(rotate(b"not an image", 90.0, Rgba([0, 0, 0, 0])).is_err());
    }
}
