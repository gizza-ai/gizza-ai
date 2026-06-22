//! gizza-ai/image-border-frame core — add a solid border/frame of a chosen color
//! and thickness around an image. Pure-Rust `image`. Returns PNG bytes (alpha
//! preserved). Independent top/right/bottom/left thickness via `thickness` (all
//! sides) — a uniform frame.

use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};

/// Parse `#rgb`, `#rrggbb`, or `#rrggbbaa` into RGBA.
pub fn parse_color(s: &str) -> Result<Rgba<u8>, String> {
    let h = s.trim().trim_start_matches('#');
    let v = |a: &str| u8::from_str_radix(a, 16).map_err(|_| format!("invalid color '{s}'"));
    let (r, g, b, a) = match h.len() {
        3 => {
            let cs: Vec<char> = h.chars().collect();
            let d = |c: char| v(&c.to_string().repeat(2));
            (d(cs[0])?, d(cs[1])?, d(cs[2])?, 255)
        }
        6 => (v(&h[0..2])?, v(&h[2..4])?, v(&h[4..6])?, 255),
        8 => (v(&h[0..2])?, v(&h[2..4])?, v(&h[4..6])?, v(&h[6..8])?),
        _ => return Err(format!("invalid color '{s}' (use #rgb, #rrggbb, or #rrggbbaa)")),
    };
    Ok(Rgba([r, g, b, a]))
}

/// Add a uniform border of `thickness` pixels in `color` around `bytes`.
/// Returns PNG bytes. The output is `(w + 2t) × (h + 2t)`.
pub fn add_border(bytes: &[u8], thickness: u32, color: Rgba<u8>) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }
    let t = thickness;
    let nw = w + 2 * t;
    let nh = h + 2 * t;
    let mut canvas: RgbaImage = RgbaImage::from_pixel(nw, nh, color);
    let src = img.to_rgba8();
    image::imageops::overlay(&mut canvas, &src, t as i64, t as i64);

    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(canvas)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, c: Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, c);
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#fff").unwrap(), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_color("#ff8800").unwrap(), Rgba([255, 136, 0, 255]));
        assert_eq!(parse_color("#00000080").unwrap(), Rgba([0, 0, 0, 128]));
        assert!(parse_color("zzz").is_err());
    }

    #[test]
    fn grows_by_twice_thickness() {
        let src = png(10, 6, Rgba([0, 0, 0, 255]));
        let out = add_border(&src, 4, Rgba([255, 0, 0, 255])).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (18, 14)); // 10+8, 6+8
    }

    #[test]
    fn corner_is_border_color_center_is_original() {
        let src = png(4, 4, Rgba([0, 0, 255, 255])); // blue image
        let out = add_border(&src, 2, Rgba([255, 0, 0, 255])).unwrap(); // red border
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.get_pixel(0, 0), Rgba([255, 0, 0, 255])); // corner = border
        assert_eq!(img.get_pixel(4, 4), Rgba([0, 0, 255, 255])); // inside = original
    }

    #[test]
    fn zero_thickness_is_noop_size() {
        let src = png(5, 5, Rgba([1, 2, 3, 255]));
        let out = add_border(&src, 0, Rgba([0, 0, 0, 255])).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (5, 5));
    }

    #[test]
    fn errors() {
        assert!(add_border(b"not an image", 2, Rgba([0, 0, 0, 255])).is_err());
    }
}
