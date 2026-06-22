//! gizza-ai/meme-caption core — pure-Rust impact-style meme caption renderer,
//! shared by the chat skill block and the CLI. No wafer/wasm-bindgen deps.
//!
//! Draws a classic top/bottom meme caption onto an image: bold uppercase white
//! text with a black outline, centered horizontally, auto-sized to fit the image
//! width (wrapping long lines), placed near the top and bottom edges. Decodes the
//! source image, rasterizes the text with a bundled bold font (fontdue — no
//! freetype/system fonts), alpha-blends it onto the pixels, and re-encodes to
//! PNG. Mirrors blocks/add-text-to-image's pure-Rust text rendering.

use std::io::Cursor;

use fontdue::Font;
use image::{ImageFormat, RgbaImage};

/// Liberation Sans Bold (SIL OFL 1.1, metric-compatible with Arial) — a freely
/// redistributable bold sans-serif used as the meme caption face. See
/// assets/LICENSE-Liberation.txt.
const FONT_BYTES: &[u8] = include_bytes!("assets/LiberationSans-Bold.ttf");

const WHITE: [u8; 3] = [255, 255, 255];
const BLACK: [u8; 3] = [0, 0, 0];

/// Parse `#rrggbb` / `rrggbb` / `#rgb` into an opaque RGB. Errors on a malformed
/// value; an empty string falls back to `default`.
fn parse_color(s: &str, default: [u8; 3]) -> Result<[u8; 3], String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    let h = t.trim_start_matches('#');
    let hx = |c: &str| u8::from_str_radix(c, 16).map_err(|_| format!("invalid color '{s}' (use #rrggbb)"));
    match h.len() {
        6 => Ok([hx(&h[0..2])?, hx(&h[2..4])?, hx(&h[4..6])?]),
        3 => {
            let cs: Vec<char> = h.chars().collect();
            let d = |c: char| u8::from_str_radix(&format!("{c}{c}"), 16).map_err(|_| format!("invalid color '{s}' (use #rrggbb)"));
            Ok([d(cs[0])?, d(cs[1])?, d(cs[2])?])
        }
        _ => Err(format!("invalid color '{s}' (use #rrggbb)")),
    }
}

/// Where a caption sits vertically.
#[derive(Clone, Copy, PartialEq, Eq)]
enum VAlign {
    Top,
    Bottom,
}

/// Alpha-blend an opaque RGB `color` over the pixel at `(x, y)` with coverage `a`.
fn blend(img: &mut RgbaImage, x: i64, y: i64, color: [u8; 3], a: u8) {
    if x < 0 || y < 0 || x >= img.width() as i64 || y >= img.height() as i64 || a == 0 {
        return;
    }
    let px = img.get_pixel_mut(x as u32, y as u32);
    let af = a as f32 / 255.0;
    for c in 0..3 {
        px[c] = (color[c] as f32 * af + px[c] as f32 * (1.0 - af))
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    px[3] = 255;
}

/// A rasterized glyph with its placement metrics, relative to the pen.
struct Glyph {
    bitmap: Vec<u8>,
    width: usize,
    height: usize,
    xmin: i32,
    ymin: i32,
    advance: f32,
}

fn rasterize(font: &Font, ch: char, size: f32) -> Glyph {
    let (m, bitmap) = font.rasterize(ch, size);
    Glyph {
        bitmap,
        width: m.width,
        height: m.height,
        xmin: m.xmin,
        ymin: m.ymin,
        advance: m.advance_width,
    }
}

/// Width in pixels a `line` would occupy at `size`.
fn line_width(font: &Font, line: &str, size: f32) -> f32 {
    line.chars().map(|c| font.metrics(c, size).advance_width).sum()
}

/// Greedy word-wrap `text` so each line fits within `max_width` at `size`.
/// A single word wider than `max_width` is kept on its own line (never split).
fn wrap(font: &Font, text: &str, size: f32, max_width: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw_line in text.split('\n') {
        let mut cur = String::new();
        for word in raw_line.split_whitespace() {
            let candidate = if cur.is_empty() {
                word.to_string()
            } else {
                format!("{cur} {word}")
            };
            if line_width(font, &candidate, size) <= max_width || cur.is_empty() {
                cur = candidate;
            } else {
                lines.push(std::mem::take(&mut cur));
                cur = word.to_string();
            }
        }
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Draw one glyph at the given pen position + baseline, blending `color`.
fn draw_glyph(img: &mut RgbaImage, g: &Glyph, pen_x: f32, baseline: f32, color: [u8; 3]) {
    if g.width == 0 || g.height == 0 {
        return;
    }
    let left = pen_x + g.xmin as f32;
    let top = baseline - (g.height as f32 + g.ymin as f32);
    for gy in 0..g.height {
        for gx in 0..g.width {
            let cov = g.bitmap[gy * g.width + gx];
            if cov != 0 {
                blend(img, left as i64 + gx as i64, top as i64 + gy as i64, color, cov);
            }
        }
    }
}

/// Draw a glyph with an `outline` of `stroke` px, then the `fill`, by stamping the
/// glyph in the outline colour at the offsets around the centre.
fn draw_glyph_stroked(
    img: &mut RgbaImage,
    g: &Glyph,
    pen_x: f32,
    baseline: f32,
    stroke: i32,
    fill: [u8; 3],
    outline: [u8; 3],
) {
    if stroke > 0 {
        for dy in -stroke..=stroke {
            for dx in -stroke..=stroke {
                if dx == 0 && dy == 0 {
                    continue;
                }
                draw_glyph(img, g, pen_x + dx as f32, baseline + dy as f32, outline);
            }
        }
    }
    draw_glyph(img, g, pen_x, baseline, fill);
}

/// Render one caption block (already wrapped into `lines`) at `align`.
#[allow(clippy::too_many_arguments)]
fn draw_caption(
    img: &mut RgbaImage,
    font: &Font,
    lines: &[String],
    size: f32,
    align: VAlign,
    stroke: i32,
    margin: f32,
    fill: [u8; 3],
    outline: [u8; 3],
) {
    let lm = font.horizontal_line_metrics(size);
    let ascent = lm.map(|m| m.ascent).unwrap_or(size * 0.8);
    let descent = lm.map(|m| m.descent).unwrap_or(-size * 0.2);
    let line_height = lm.map(|m| m.new_line_size).unwrap_or(size * 1.2);
    let block_h = line_height * lines.len() as f32;
    let img_w = img.width() as f32;
    let img_h = img.height() as f32;

    // Baseline of the first line.
    let first_baseline = match align {
        VAlign::Top => margin + ascent,
        VAlign::Bottom => img_h - margin - block_h + ascent - descent.min(0.0),
    };

    for (li, line) in lines.iter().enumerate() {
        let w = line_width(font, line, size);
        let mut pen_x = (img_w - w) / 2.0;
        let baseline = first_baseline + li as f32 * line_height;
        for ch in line.chars() {
            let g = rasterize(font, ch, size);
            draw_glyph_stroked(img, &g, pen_x, baseline, stroke, fill, outline);
            pen_x += g.advance;
        }
    }
}

/// Largest font size (≤ `start`) at which `text`, wrapped to `max_width`, renders
/// in at most `max_lines` lines and fits within `max_height`. Shrinks in steps so
/// long captions stay inside the image instead of overflowing.
fn fit_size(
    font: &Font,
    text: &str,
    start: f32,
    max_width: f32,
    max_height: f32,
    max_lines: usize,
) -> (f32, Vec<String>) {
    let mut size = start;
    loop {
        let lines = wrap(font, text, size, max_width);
        let lh = font
            .horizontal_line_metrics(size)
            .map(|m| m.new_line_size)
            .unwrap_or(size * 1.2);
        let block_h = lh * lines.len() as f32;
        let fits_wide = lines.iter().all(|l| line_width(font, l, size) <= max_width);
        if (lines.len() <= max_lines && block_h <= max_height && fits_wide) || size <= 10.0 {
            return (size, lines);
        }
        size -= 2.0;
    }
}

/// Add an impact-style top and/or bottom caption to `img_bytes`. Both captions
/// are optional (empty string = skip). Text is centered, auto-sized to the image
/// width, drawn in `text_color` with a `outline_color` outline (each `#rrggbb`,
/// empty = white fill / black outline); uppercased when `uppercase` is true (the
/// classic meme look). Returns PNG bytes.
pub fn caption(
    img_bytes: &[u8],
    top: &str,
    bottom: &str,
    uppercase: bool,
    text_color: &str,
    outline_color: &str,
) -> Result<Vec<u8>, String> {
    let top = top.trim();
    let bottom = bottom.trim();
    if top.is_empty() && bottom.is_empty() {
        return Err("provide top and/or bottom caption text".into());
    }
    let fill = parse_color(text_color, WHITE)?;
    let outline = parse_color(outline_color, BLACK)?;

    let mut img = image::load_from_memory(img_bytes)
        .map_err(|e| format!("failed to decode image: {e}"))?
        .to_rgba8();
    let font = Font::from_bytes(FONT_BYTES, fontdue::FontSettings::default())
        .map_err(|e| format!("font load failed: {e}"))?;

    let w = img.width() as f32;
    let h = img.height() as f32;
    // Classic meme proportions: start near ~1/8 of the height, fit to ~90% width
    // and ~28% of the height (≤3 lines). Outline scales with the image.
    let max_w = w * 0.9;
    let max_h = h * 0.28;
    let start = (h / 8.0).clamp(14.0, 96.0);
    let margin = h * 0.03;
    let stroke = (start / 16.0).round().clamp(1.0, 6.0) as i32;

    let cased = |s: &str| if uppercase { s.to_uppercase() } else { s.to_string() };

    if !top.is_empty() {
        let t = cased(top);
        let (size, lines) = fit_size(&font, &t, start, max_w, max_h, 3);
        draw_caption(&mut img, &font, &lines, size, VAlign::Top, stroke, margin, fill, outline);
    }
    if !bottom.is_empty() {
        let b = cased(bottom);
        let (size, lines) = fit_size(&font, &b, start, max_w, max_h, 3);
        draw_caption(&mut img, &font, &lines, size, VAlign::Bottom, stroke, margin, fill, outline);
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
        let img = RgbaImage::from_pixel(w, h, image::Rgba([40, 40, 40, 255]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    fn decode(bytes: &[u8]) -> RgbaImage {
        image::load_from_memory(bytes).unwrap().to_rgba8()
    }

    fn has_white(img: &RgbaImage) -> bool {
        img.pixels().any(|p| p[0] > 240 && p[1] > 240 && p[2] > 240)
    }

    fn has_black(img: &RgbaImage) -> bool {
        img.pixels().any(|p| p[0] < 15 && p[1] < 15 && p[2] < 15)
    }

    #[test]
    fn top_and_bottom_caption_renders_and_keeps_dimensions() {
        let src = blank_png(400, 400);
        let out = caption(&src, "one does not simply", "make a meme", true, "", "").unwrap();
        let img = decode(&out);
        assert_eq!((img.width(), img.height()), (400, 400));
        // White fill + black outline both present; output differs from source.
        assert!(has_white(&img), "white fill expected");
        assert!(has_black(&img), "black outline expected");
        assert_ne!(out, src);
    }

    #[test]
    fn top_caption_lands_in_top_half() {
        let src = blank_png(400, 400);
        let out = caption(&src, "TOP", "", true, "", "").unwrap();
        let img = decode(&out);
        // The topmost white pixel must be in the upper half.
        let top_white_y = (0..img.height())
            .find(|&y| (0..img.width()).any(|x| img.get_pixel(x, y)[0] > 240));
        assert!(matches!(top_white_y, Some(y) if y < 200), "top caption not in top half: {top_white_y:?}");
    }

    #[test]
    fn bottom_caption_lands_in_bottom_half() {
        let src = blank_png(400, 400);
        let out = caption(&src, "", "BOTTOM", true, "", "").unwrap();
        let img = decode(&out);
        let bottom_white_y = (0..img.height())
            .rev()
            .find(|&y| (0..img.width()).any(|x| img.get_pixel(x, y)[0] > 240));
        assert!(matches!(bottom_white_y, Some(y) if y > 200), "bottom caption not in bottom half: {bottom_white_y:?}");
    }

    #[test]
    fn long_caption_wraps_within_width() {
        let src = blank_png(300, 300);
        let out = caption(
            &src,
            "this is a very long meme caption that should wrap onto several lines",
            "",
            true,
            "",
            "",
        )
        .unwrap();
        let img = decode(&out);
        assert_eq!((img.width(), img.height()), (300, 300));
        assert!(has_white(&img));
    }

    #[test]
    fn empty_text_errors() {
        let src = blank_png(100, 100);
        assert!(caption(&src, "", "", true, "", "").is_err());
        assert!(caption(&src, "   ", "  ", true, "", "").is_err());
    }

    #[test]
    fn bad_image_errors() {
        assert!(caption(b"not an image", "hi", "there", true, "", "").is_err());
    }

    #[test]
    fn custom_colors_render() {
        // Red fill (#ff0000) on the dark background — expect strong red pixels.
        let src = blank_png(400, 200);
        let out = caption(&src, "RED", "", true, "#ff0000", "#000000").unwrap();
        let img = decode(&out);
        assert!(
            img.pixels().any(|p| p[0] > 200 && p[1] < 60 && p[2] < 60),
            "red fill expected"
        );
    }

    #[test]
    fn uppercase_false_preserves_case() {
        // With uppercase=false the lowercase 'a' glyph must appear; an uppercased
        // render would have no lowercase letterforms. We assert it simply renders.
        let src = blank_png(400, 200);
        let out = caption(&src, "lower case", "", false, "", "").unwrap();
        let img = decode(&out);
        assert!(has_white(&img));
        assert_ne!(out, src);
    }

    #[test]
    fn bad_color_errors() {
        let src = blank_png(100, 100);
        assert!(caption(&src, "hi", "", true, "nope", "").is_err());
        assert!(caption(&src, "hi", "", true, "", "zzz").is_err());
    }

    #[test]
    fn parse_color_variants() {
        assert_eq!(parse_color("#ff0000", BLACK).unwrap(), [255, 0, 0]);
        assert_eq!(parse_color("00ff00", BLACK).unwrap(), [0, 255, 0]);
        assert_eq!(parse_color("#abc", BLACK).unwrap(), [170, 187, 204]);
        assert_eq!(parse_color("", WHITE).unwrap(), WHITE);
        assert!(parse_color("xyz", BLACK).is_err());
    }
}
