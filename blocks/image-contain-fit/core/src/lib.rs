//! gizza-ai/image-contain-fit core — fit (letterbox) an image inside target
//! dimensions preserving aspect ratio, padding the remainder with a chosen
//! background colour. Pure-Rust `image`; no wafer/wasm-bindgen deps. Returns PNG
//! bytes (alpha preserved when the pad colour is transparent).

use std::io::Cursor;

use image::imageops::FilterType;
use image::{DynamicImage, GenericImage, ImageFormat, Rgba, RgbaImage};

/// Parse a CSS-style colour into RGBA.
///
/// Accepts `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` (with or without the leading
/// `#`), and a few common named colours (white, black, transparent, …). The
/// alpha defaults to fully opaque (255) when not given. `transparent` yields a
/// fully transparent black.
pub fn parse_color(s: &str) -> Result<Rgba<u8>, String> {
    let t = s.trim();
    let lower = t.to_ascii_lowercase();
    match lower.as_str() {
        "transparent" | "none" => return Ok(Rgba([0, 0, 0, 0])),
        "white" => return Ok(Rgba([255, 255, 255, 255])),
        "black" => return Ok(Rgba([0, 0, 0, 255])),
        "red" => return Ok(Rgba([255, 0, 0, 255])),
        "green" => return Ok(Rgba([0, 128, 0, 255])),
        "blue" => return Ok(Rgba([0, 0, 255, 255])),
        "gray" | "grey" => return Ok(Rgba([128, 128, 128, 255])),
        _ => {}
    }
    let hex = lower.strip_prefix('#').unwrap_or(&lower);
    if hex.is_empty() || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "invalid colour '{s}'; use a hex value like #ffffff or a name like white/black/transparent"
        ));
    }
    let parse2 = |a: &str| u8::from_str_radix(a, 16).map_err(|e| e.to_string());
    match hex.len() {
        3 => {
            // #rgb → each nibble doubled.
            let n: Vec<u8> = hex
                .chars()
                .map(|c| {
                    let v = c.to_digit(16).unwrap() as u8;
                    (v << 4) | v
                })
                .collect();
            Ok(Rgba([n[0], n[1], n[2], 255]))
        }
        4 => {
            let n: Vec<u8> = hex
                .chars()
                .map(|c| {
                    let v = c.to_digit(16).unwrap() as u8;
                    (v << 4) | v
                })
                .collect();
            Ok(Rgba([n[0], n[1], n[2], n[3]]))
        }
        6 => Ok(Rgba([
            parse2(&hex[0..2])?,
            parse2(&hex[2..4])?,
            parse2(&hex[4..6])?,
            255,
        ])),
        8 => Ok(Rgba([
            parse2(&hex[0..2])?,
            parse2(&hex[2..4])?,
            parse2(&hex[4..6])?,
            parse2(&hex[6..8])?,
        ])),
        _ => Err(format!(
            "invalid hex colour '{s}'; expected 3, 4, 6, or 8 hex digits"
        )),
    }
}

/// Compute the scaled (inner) dimensions that fit `(src_w, src_h)` inside
/// `(target_w, target_h)` preserving aspect ratio (the "contain" rule). The
/// result never exceeds the target and never shrinks below 1px. By default the
/// image is allowed to grow to fill the box; pass `allow_upscale = false` to cap
/// the inner size at the source size (smaller images stay centred at 1:1).
pub fn contain_size(
    src_w: u32,
    src_h: u32,
    target_w: u32,
    target_h: u32,
    allow_upscale: bool,
) -> (u32, u32) {
    if src_w == 0 || src_h == 0 {
        return (0, 0);
    }
    let mut scale = (target_w as f64 / src_w as f64).min(target_h as f64 / src_h as f64);
    if !allow_upscale && scale > 1.0 {
        scale = 1.0;
    }
    let w = ((src_w as f64) * scale).round().max(1.0) as u32;
    let h = ((src_h as f64) * scale).round().max(1.0) as u32;
    // Never exceed the target box (rounding could push it 1px over).
    (w.min(target_w), h.min(target_h))
}

/// Fit `bytes` inside a `target_w` × `target_h` canvas preserving aspect ratio,
/// padding the leftover space with `pad`. The scaled image is centred. Returns
/// PNG bytes of exactly the target dimensions.
pub fn contain_fit(
    bytes: &[u8],
    target_w: u32,
    target_h: u32,
    pad: Rgba<u8>,
    allow_upscale: bool,
) -> Result<Vec<u8>, String> {
    if target_w == 0 || target_h == 0 {
        return Err("width and height must be > 0".into());
    }
    let img =
        image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (src_w, src_h) = (img.width(), img.height());
    let (inner_w, inner_h) = contain_size(src_w, src_h, target_w, target_h, allow_upscale);

    // Background canvas filled with the pad colour.
    let mut canvas = RgbaImage::from_pixel(target_w, target_h, pad);

    // Scale the source (Lanczos3 for quality) and paste it centred.
    let scaled = img
        .resize_exact(inner_w, inner_h, FilterType::Lanczos3)
        .to_rgba8();
    let off_x = (target_w - inner_w) / 2;
    let off_y = (target_h - inner_h) / 2;
    canvas
        .copy_from(&scaled, off_x, off_y)
        .map_err(|e| format!("compositing failed: {e}"))?;

    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(canvas)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    fn png(w: u32, h: u32, color: Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, color);
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn parse_color_hex_and_names() {
        assert_eq!(parse_color("#ffffff").unwrap(), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_color("000000").unwrap(), Rgba([0, 0, 0, 255]));
        assert_eq!(parse_color("#fff").unwrap(), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_color("#f00").unwrap(), Rgba([255, 0, 0, 255]));
        assert_eq!(parse_color("#11223344").unwrap(), Rgba([17, 34, 51, 68]));
        assert_eq!(parse_color("white").unwrap(), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_color("BLACK").unwrap(), Rgba([0, 0, 0, 255]));
        assert_eq!(parse_color("transparent").unwrap(), Rgba([0, 0, 0, 0]));
    }

    #[test]
    fn parse_color_rejects_garbage() {
        assert!(parse_color("#xyz").is_err());
        assert!(parse_color("notacolour").is_err());
        assert!(parse_color("#12345").is_err()); // 5 digits
        assert!(parse_color("").is_err());
    }

    #[test]
    fn contain_size_landscape_into_square() {
        // 200x100 into 100x100 → width-bound → 100x50.
        assert_eq!(contain_size(200, 100, 100, 100, true), (100, 50));
    }

    #[test]
    fn contain_size_portrait_into_square() {
        // 100x200 into 100x100 → height-bound → 50x100.
        assert_eq!(contain_size(100, 200, 100, 100, true), (50, 100));
    }

    #[test]
    fn contain_size_no_upscale_caps_at_source() {
        // 50x50 into 100x100 with upscale off → stays 50x50.
        assert_eq!(contain_size(50, 50, 100, 100, false), (50, 50));
        // With upscale on → grows to 100x100.
        assert_eq!(contain_size(50, 50, 100, 100, true), (100, 100));
    }

    #[test]
    fn output_is_exact_target_size() {
        let out = contain_fit(
            &png(200, 100, Rgba([255, 0, 0, 255])),
            100,
            100,
            Rgba([0, 0, 0, 255]),
            true,
        )
        .unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (100, 100));
    }

    #[test]
    fn padding_uses_chosen_colour() {
        // 200x100 red into 100x100 with blue pad → top-left corner is blue pad,
        // centre row contains the red image.
        let pad = Rgba([0, 0, 255, 255]);
        let out = contain_fit(
            &png(200, 100, Rgba([255, 0, 0, 255])),
            100,
            100,
            pad,
            true,
        )
        .unwrap();
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        // Inner image is 100x50 centred → rows 0..25 and 75..100 are pad.
        assert_eq!(img.get_pixel(0, 0), &pad);
        assert_eq!(img.get_pixel(99, 99), &pad);
        // Centre is the (red) image.
        assert_eq!(img.get_pixel(50, 50), &Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn transparent_pad_preserves_alpha() {
        let out = contain_fit(
            &png(200, 100, Rgba([255, 0, 0, 255])),
            100,
            100,
            Rgba([0, 0, 0, 0]),
            true,
        )
        .unwrap();
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(0, 0)[3], 0, "corner pad is transparent");
    }

    #[test]
    fn rejects_zero_dims_and_bad_input() {
        assert!(contain_fit(
            &png(10, 10, Rgba([0, 0, 0, 255])),
            0,
            10,
            Rgba([0, 0, 0, 255]),
            true
        )
        .is_err());
        assert!(contain_fit(b"not an image", 10, 10, Rgba([0, 0, 0, 255]), true).is_err());
    }
}
