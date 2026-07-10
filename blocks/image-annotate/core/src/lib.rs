//! gizza-ai/image-annotate core — pure-Rust image markup shared by the chat
//! skill block and the CLI. No wafer/wasm-bindgen deps. Decodes an image, draws
//! a list of annotation marks (boxes, arrows, highlights, text labels) onto the
//! pixels at exact coordinates, and re-encodes to PNG. Text is rasterized with a
//! bundled font (fontdue — no freetype/system fonts), same stack as
//! add-text-to-image / code-screenshot.

use std::io::Cursor;

use fontdue::Font;
use image::{ImageFormat, RgbaImage};
use serde::Deserialize;

const FONT_BYTES: &[u8] = include_bytes!("assets/DejaVuSansMono.ttf");

/// One markup instruction. Tagged by `type`: `box`, `arrow`, `highlight`, or
/// `text`. Every variant may override the tool-level `color`; boxes/arrows may
/// override `stroke_width`, highlights take an `opacity`, and text takes a
/// `font_size`. Coordinates are pixels from the top-left of the image.
#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Annotation {
    /// Hollow (outlined) rectangle with its top-left at (x, y).
    Box {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        stroke_width: Option<f32>,
    },
    /// Line from (x1, y1) to (x2, y2) with an arrowhead drawn at (x2, y2).
    Arrow {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        stroke_width: Option<f32>,
    },
    /// Semi-transparent filled rectangle (a marker-pen wash) at (x, y).
    Highlight {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        #[serde(default)]
        color: Option<String>,
        /// 0.0 (invisible) .. 1.0 (opaque). Defaults to 0.35.
        #[serde(default)]
        opacity: Option<f32>,
    },
    /// A text label with its top-left at (x, y).
    Text {
        x: f32,
        y: f32,
        text: String,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        font_size: Option<f32>,
    },
}

/// Parse `#rgb` / `#rrggbb` / `#rrggbbaa` (leading `#` optional) into RGBA.
/// Empty string is an error (callers supply a concrete default). A 6/3-digit
/// value is opaque; an 8-digit value carries its own alpha.
pub fn parse_color(s: &str) -> Result<[u8; 4], String> {
    let t = s.trim();
    if t.is_empty() {
        return Err("empty color".into());
    }
    let h = t.trim_start_matches('#');
    let hex = |c: &str| u8::from_str_radix(c, 16).map_err(|_| format!("invalid color '{s}'"));
    match h.len() {
        3 => {
            let cs: Vec<char> = h.chars().collect();
            let d = |c: char| {
                u8::from_str_radix(&format!("{c}{c}"), 16).map_err(|_| format!("invalid color '{s}'"))
            };
            Ok([d(cs[0])?, d(cs[1])?, d(cs[2])?, 255])
        }
        6 => Ok([hex(&h[0..2])?, hex(&h[2..4])?, hex(&h[4..6])?, 255]),
        8 => Ok([
            hex(&h[0..2])?,
            hex(&h[2..4])?,
            hex(&h[4..6])?,
            hex(&h[6..8])?,
        ]),
        _ => Err(format!("invalid color '{s}' (use #rgb, #rrggbb, or #rrggbbaa)")),
    }
}

/// Alpha-blend `rgba` (straight alpha) over the pixel at `(x, y)`, scaling its
/// alpha by `coverage` (0..1 — the glyph/anti-alias coverage). Out-of-bounds
/// and fully-transparent writes are skipped.
fn blend(img: &mut RgbaImage, x: i64, y: i64, rgba: [u8; 4], coverage: f32) {
    if x < 0 || y < 0 || x >= img.width() as i64 || y >= img.height() as i64 {
        return;
    }
    let a = (rgba[3] as f32 / 255.0) * coverage.clamp(0.0, 1.0);
    if a <= 0.0 {
        return;
    }
    let px = img.get_pixel_mut(x as u32, y as u32);
    for c in 0..3 {
        px[c] = (rgba[c] as f32 * a + px[c] as f32 * (1.0 - a))
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    px[3] = ((a + (px[3] as f32 / 255.0) * (1.0 - a)) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
}

/// Stamp a filled disk of radius `r` centered at `(cx, cy)` — the brush used to
/// draw thick lines without gaps.
fn stamp(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, rgba: [u8; 4]) {
    let ri = r.ceil() as i64;
    let cxr = cx.round() as i64;
    let cyr = cy.round() as i64;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let d2 = (dx * dx + dy * dy) as f32;
            if d2 <= (r * r) + 0.5 {
                blend(img, cxr + dx, cyr + dy, rgba, 1.0);
            }
        }
    }
}

/// Draw a solid line of the given stroke width by stamping disks along it.
fn draw_line(img: &mut RgbaImage, x1: f32, y1: f32, x2: f32, y2: f32, stroke: f32, rgba: [u8; 4]) {
    let r = (stroke / 2.0).max(0.5);
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    let steps = len.ceil().max(1.0) as i64;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        stamp(img, x1 + dx * t, y1 + dy * t, r, rgba);
    }
}

/// Draw a hollow rectangle (four stroked edges).
fn draw_box(img: &mut RgbaImage, x: f32, y: f32, w: f32, h: f32, stroke: f32, rgba: [u8; 4]) {
    let (x2, y2) = (x + w, y + h);
    draw_line(img, x, y, x2, y, stroke, rgba); // top
    draw_line(img, x, y2, x2, y2, stroke, rgba); // bottom
    draw_line(img, x, y, x, y2, stroke, rgba); // left
    draw_line(img, x2, y, x2, y2, stroke, rgba); // right
}

/// Draw an arrow: a shaft from tail→tip plus a two-line arrowhead at the tip.
fn draw_arrow(img: &mut RgbaImage, x1: f32, y1: f32, x2: f32, y2: f32, stroke: f32, rgba: [u8; 4]) {
    draw_line(img, x1, y1, x2, y2, stroke, rgba);
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.5 {
        return;
    }
    let (ux, uy) = (dx / len, dy / len); // unit tail→tip
    let head = (stroke * 3.5).max(12.0).min(len); // arrowhead length
    let spread = 0.45_f32; // ~26° half-angle
    // Rotate the reversed direction (-ux,-uy) by ±spread and extend by `head`.
    let (bx, by) = (-ux, -uy);
    for sign in [-1.0_f32, 1.0] {
        let (c, s) = (spread.cos(), (spread * sign).sin());
        let hx = bx * c - by * s;
        let hy = bx * s + by * c;
        draw_line(img, x2, y2, x2 + hx * head, y2 + hy * head, stroke, rgba);
    }
}

/// Fill a rectangle with `rgba` scaled by `opacity` (a highlight wash).
fn fill_rect(img: &mut RgbaImage, x: f32, y: f32, w: f32, h: f32, mut rgba: [u8; 4], opacity: f32) {
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let x1 = (x + w).ceil() as i64;
    let y1 = (y + h).ceil() as i64;
    let a = ((rgba[3] as f32 / 255.0) * opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    rgba[3] = a;
    for py in y0..y1 {
        for px in x0..x1 {
            blend(img, px, py, rgba, 1.0);
        }
    }
}

/// Draw one glyph at the given pen position + baseline.
fn draw_glyph(img: &mut RgbaImage, font: &Font, ch: char, size: f32, pen_x: f32, baseline: f32, rgba: [u8; 4]) {
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
                blend(img, left as i64 + gx as i64, top as i64 + gy as i64, rgba, cov as f32 / 255.0);
            }
        }
    }
}

/// Draw a text label with its top-left at `(x, y)` (supports `\n`).
fn draw_text(img: &mut RgbaImage, font: &Font, text: &str, x: f32, y: f32, size: f32, rgba: [u8; 4]) {
    let lm = font.horizontal_line_metrics(size);
    let ascent = lm.map(|m| m.ascent).unwrap_or(size * 0.8);
    let line_height = lm.map(|m| m.new_line_size).unwrap_or(size * 1.2);
    for (li, line) in text.split('\n').enumerate() {
        let baseline = y + ascent + li as f32 * line_height;
        let mut pen_x = x;
        for ch in line.chars() {
            draw_glyph(img, font, ch, size, pen_x, baseline, rgba);
            pen_x += font.metrics(ch, size).advance_width;
        }
    }
}

/// Draw the `annotations` (a JSON array) onto `img_bytes` and return PNG bytes.
///
/// `default_color` (`#rgb`/`#rrggbb`/`#rrggbbaa`), `default_stroke` (px), and
/// `default_font_size` (px) are the per-annotation fallbacks. Each annotation
/// may override its own `color` / `stroke_width` / `font_size` / `opacity`.
/// Marks are drawn in list order (later marks paint over earlier ones).
/// Errors on an undecodable image, malformed JSON, an empty list, or a bad color.
pub fn render(
    img_bytes: &[u8],
    annotations_json: &str,
    default_color: &str,
    default_stroke: f32,
    default_font_size: f32,
) -> Result<Vec<u8>, String> {
    let anns: Vec<Annotation> = serde_json::from_str(annotations_json)
        .map_err(|e| format!("failed to parse annotations JSON (expected an array of marks): {e}"))?;
    if anns.is_empty() {
        return Err("annotations is empty — provide at least one mark".into());
    }
    let def_color = parse_color(default_color)?;
    let def_stroke = if default_stroke.is_finite() && default_stroke > 0.0 { default_stroke } else { 3.0 };
    let def_font = if default_font_size.is_finite() && default_font_size >= 4.0 { default_font_size } else { 24.0 };

    let mut img = image::load_from_memory(img_bytes)
        .map_err(|e| format!("failed to decode image: {e}"))?
        .to_rgba8();
    let font = Font::from_bytes(FONT_BYTES, fontdue::FontSettings::default())
        .map_err(|e| format!("font load failed: {e}"))?;

    // Resolve a per-annotation color/stroke/size, falling back to the defaults.
    let color_of = |c: &Option<String>| -> Result<[u8; 4], String> {
        match c {
            Some(s) => parse_color(s),
            None => Ok(def_color),
        }
    };

    for ann in &anns {
        match ann {
            Annotation::Box { x, y, w, h, color, stroke_width } => {
                let rgba = color_of(color)?;
                let sw = stroke_width.filter(|s| s.is_finite() && *s > 0.0).unwrap_or(def_stroke);
                draw_box(&mut img, *x, *y, *w, *h, sw, rgba);
            }
            Annotation::Arrow { x1, y1, x2, y2, color, stroke_width } => {
                let rgba = color_of(color)?;
                let sw = stroke_width.filter(|s| s.is_finite() && *s > 0.0).unwrap_or(def_stroke);
                draw_arrow(&mut img, *x1, *y1, *x2, *y2, sw, rgba);
            }
            Annotation::Highlight { x, y, w, h, color, opacity } => {
                let rgba = color_of(color)?;
                let op = opacity.filter(|o| o.is_finite()).unwrap_or(0.35);
                fill_rect(&mut img, *x, *y, *w, *h, rgba, op);
            }
            Annotation::Text { x, y, text, color, font_size } => {
                let rgba = color_of(color)?;
                let fs = font_size.filter(|s| s.is_finite() && *s >= 4.0).unwrap_or(def_font);
                draw_text(&mut img, &font, text, *x, *y, fs, rgba);
            }
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

    fn blank_png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, image::Rgba([0, 0, 0, 255]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    fn decode(bytes: &[u8]) -> RgbaImage {
        image::load_from_memory(bytes).unwrap().to_rgba8()
    }

    #[test]
    fn parse_color_variants() {
        assert_eq!(parse_color("#ff0000").unwrap(), [255, 0, 0, 255]);
        assert_eq!(parse_color("00ff00").unwrap(), [0, 255, 0, 255]);
        assert_eq!(parse_color("#abc").unwrap(), [170, 187, 204, 255]);
        assert_eq!(parse_color("#ff000080").unwrap(), [255, 0, 0, 128]);
        assert!(parse_color("").is_err());
        assert!(parse_color("nope").is_err());
    }

    #[test]
    fn draws_all_four_primitives_and_keeps_dimensions() {
        let src = blank_png(240, 160);
        let anns = r##"[
            {"type":"box","x":20,"y":15,"w":120,"h":60},
            {"type":"arrow","x1":200,"y1":10,"x2":150,"y2":45},
            {"type":"highlight","x":20,"y":90,"w":160,"h":24,"color":"#ffff00"},
            {"type":"text","x":22,"y":92,"text":"Look here","color":"#00ff00"}
        ]"##;
        let out = render(&src, anns, "#ff0000", 3.0, 24.0).unwrap();
        let img = decode(&out);
        assert_eq!((img.width(), img.height()), (240, 160));
        // Something red was drawn (the default-colored box + arrow).
        assert!(img.pixels().any(|p| p[0] > 200 && p[1] < 80 && p[2] < 80), "red mark present");
        // The yellow highlight blended over black.
        assert!(img.pixels().any(|p| p[0] > 40 && p[1] > 40 && p[2] < 40), "yellow wash present");
        // The green label glyphs.
        assert!(img.pixels().any(|p| p[1] > 120 && p[0] < 120 && p[2] < 120), "green text present");
    }

    #[test]
    fn per_annotation_color_overrides_default() {
        let src = blank_png(60, 60);
        let out = render(&src, r##"[{"type":"box","x":5,"y":5,"w":40,"h":40,"color":"#0000ff"}]"##, "#ff0000", 4.0, 24.0).unwrap();
        let img = decode(&out);
        assert!(img.pixels().any(|p| p[2] > 200 && p[0] < 80), "blue box drawn, not red");
        assert!(!img.pixels().any(|p| p[0] > 200 && p[2] < 80), "no red pixels");
    }

    #[test]
    fn highlight_opacity_one_is_solid() {
        let src = blank_png(40, 40);
        let out = render(&src, r##"[{"type":"highlight","x":0,"y":0,"w":40,"h":40,"color":"#ffffff","opacity":1.0}]"##, "#ff0000", 3.0, 24.0).unwrap();
        let img = decode(&out);
        // Fully opaque white wash → the center pixel is white.
        let c = img.get_pixel(20, 20);
        assert!(c[0] > 250 && c[1] > 250 && c[2] > 250, "solid white fill");
    }

    #[test]
    fn empty_list_errors() {
        let src = blank_png(20, 20);
        assert!(render(&src, "[]", "#ff0000", 3.0, 24.0).is_err());
    }

    #[test]
    fn malformed_json_errors() {
        let src = blank_png(20, 20);
        assert!(render(&src, "not json", "#ff0000", 3.0, 24.0).is_err());
    }

    #[test]
    fn bad_default_color_errors() {
        let src = blank_png(20, 20);
        assert!(render(&src, r#"[{"type":"box","x":0,"y":0,"w":5,"h":5}]"#, "xyz", 3.0, 24.0).is_err());
    }

    #[test]
    fn bad_image_errors() {
        assert!(render(b"not an image", r#"[{"type":"box","x":0,"y":0,"w":5,"h":5}]"#, "#ff0000", 3.0, 24.0).is_err());
    }

    #[test]
    fn unknown_annotation_type_errors() {
        let src = blank_png(20, 20);
        assert!(render(&src, r#"[{"type":"circle","x":0,"y":0,"r":5}]"#, "#ff0000", 3.0, 24.0).is_err());
    }
}
