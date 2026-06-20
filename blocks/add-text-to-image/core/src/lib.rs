//! gizza-ai/add-text-to-image core — pure-Rust text overlay shared by the chat
//! skill block and the CLI. No wafer/wasm-bindgen deps. Decodes an image,
//! rasterizes the caption with a bundled font (fontdue — no freetype/system
//! fonts), alpha-blends it onto the pixels, and re-encodes to PNG. Mirrors
//! blocks/code-screenshot's pure-Rust text rendering.

use std::io::Cursor;

use fontdue::Font;
use image::{ImageFormat, RgbaImage};

const FONT_BYTES: &[u8] = include_bytes!("assets/DejaVuSansMono.ttf");

/// Parse `#rrggbb` / `rrggbb` (and `#rgb`) into an opaque RGBA. Defaults to white
/// on an empty string; errors on a malformed value.
fn parse_color(s: &str) -> Result<[u8; 4], String> {
    let h = s.trim().trim_start_matches('#');
    if s.trim().is_empty() {
        return Ok([255, 255, 255, 255]);
    }
    let expand = |c: &str| u8::from_str_radix(c, 16).map_err(|_| format!("invalid color '{s}'"));
    match h.len() {
        6 => Ok([
            expand(&h[0..2])?,
            expand(&h[2..4])?,
            expand(&h[4..6])?,
            255,
        ]),
        3 => {
            let cs: Vec<char> = h.chars().collect();
            let d = |c: char| u8::from_str_radix(&format!("{c}{c}"), 16).map_err(|_| format!("invalid color '{s}'"));
            Ok([d(cs[0])?, d(cs[1])?, d(cs[2])?, 255])
        }
        _ => Err(format!("invalid color '{s}' (use #rrggbb)")),
    }
}

/// Alpha-blend `color` (opaque RGB) over the pixel at `(x, y)` with coverage `a`.
fn blend(img: &mut RgbaImage, x: i64, y: i64, color: [u8; 4], a: u8) {
    if x < 0 || y < 0 || x >= img.width() as i64 || y >= img.height() as i64 || a == 0 {
        return;
    }
    let px = img.get_pixel_mut(x as u32, y as u32);
    let af = a as f32 / 255.0;
    for c in 0..3 {
        px[c] = (color[c] as f32 * af + px[c] as f32 * (1.0 - af)).round().clamp(0.0, 255.0) as u8;
    }
    px[3] = 255;
}

/// Draw one glyph at the given pen position + baseline, blending `color`.
fn draw_glyph(img: &mut RgbaImage, font: &Font, ch: char, size: f32, pen_x: f32, baseline: f32, color: [u8; 4]) {
    let (m, bitmap) = font.rasterize(ch, size);
    if m.width == 0 || m.height == 0 {
        return;
    }
    let left = pen_x + m.xmin as f32;
    let top = baseline - (m.height as f32 + m.ymin as f32);
    for gy in 0..m.height {
        for gx in 0..m.width {
            let cov = bitmap[gy * m.width + gx];
            if cov != 0 {
                blend(img, left as i64 + gx as i64, top as i64 + gy as i64, color, cov);
            }
        }
    }
}

/// Overlay `text` onto `img_bytes` at `(x, y)` (top-left of the first line) using
/// `font_size` px and `color` (#rrggbb). Supports `\n` for multiple lines.
/// Returns PNG bytes. `x`/`y`/`font_size` are clamped to sane minimums.
pub fn render(img_bytes: &[u8], text: &str, x: i64, y: i64, font_size: f32, color: &str) -> Result<Vec<u8>, String> {
    if text.is_empty() {
        return Err("text is empty".into());
    }
    let size = if font_size.is_finite() && font_size >= 4.0 { font_size } else { 32.0 };
    let rgba = parse_color(color)?;
    let mut img = image::load_from_memory(img_bytes)
        .map_err(|e| format!("failed to decode image: {e}"))?
        .to_rgba8();

    let font = Font::from_bytes(FONT_BYTES, fontdue::FontSettings::default())
        .map_err(|e| format!("font load failed: {e}"))?;
    let line_metrics = font.horizontal_line_metrics(size);
    let ascent = line_metrics.map(|m| m.ascent).unwrap_or(size * 0.8);
    let line_height = line_metrics.map(|m| m.new_line_size).unwrap_or(size * 1.2);

    for (li, line) in text.split('\n').enumerate() {
        let baseline = y as f32 + ascent + li as f32 * line_height;
        let mut pen_x = x as f32;
        for ch in line.chars() {
            draw_glyph(&mut img, &font, ch, size, pen_x, baseline, rgba);
            let m = font.metrics(ch, size);
            pen_x += m.advance_width;
        }
    }

    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn blank_png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, image::Rgba([0, 0, 0, 255]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    fn decode_dims(bytes: &[u8]) -> (u32, u32) {
        let img = image::load_from_memory(bytes).unwrap();
        (img.width(), img.height())
    }

    #[test]
    fn draws_text_and_keeps_dimensions() {
        let src = blank_png(200, 80);
        let out = render(&src, "Hello", 10, 10, 32.0, "#ffffff").unwrap();
        assert_eq!(decode_dims(&out), (200, 80));
        // output differs from the all-black source (text was drawn).
        assert_ne!(out, src);
        // some pixel is now non-black (text is white).
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        assert!(img.pixels().any(|p| p[0] > 0 || p[1] > 0 || p[2] > 0));
    }

    #[test]
    fn parse_color_variants() {
        assert_eq!(parse_color("#ff0000").unwrap(), [255, 0, 0, 255]);
        assert_eq!(parse_color("00ff00").unwrap(), [0, 255, 0, 255]);
        assert_eq!(parse_color("#abc").unwrap(), [170, 187, 204, 255]);
        assert_eq!(parse_color("").unwrap(), [255, 255, 255, 255]);
        assert!(parse_color("nope").is_err());
    }

    #[test]
    fn multiline_text_renders() {
        let src = blank_png(200, 120);
        let out = render(&src, "line1\nline2", 5, 5, 24.0, "#ffffff").unwrap();
        assert_eq!(decode_dims(&out), (200, 120));
    }

    #[test]
    fn empty_text_errors() {
        let src = blank_png(50, 50);
        assert!(render(&src, "", 0, 0, 32.0, "#fff").is_err());
    }

    #[test]
    fn bad_image_errors() {
        assert!(render(b"not an image", "hi", 0, 0, 32.0, "#fff").is_err());
    }

    #[test]
    fn bad_color_errors() {
        let src = blank_png(50, 50);
        assert!(render(&src, "hi", 0, 0, 32.0, "xyz").is_err());
    }
}
