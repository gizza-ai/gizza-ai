//! gizza-ai/text-image-card core — pure-Rust renderer that turns a quote or
//! short text into a styled, shareable PNG card. No wafer/wasm-bindgen deps so
//! it is shared by the chat skill block and the CLI.
//!
//! Pipeline: pick a theme (background + text colours), rasterize the body text
//! with a bundled font (fontdue — no freetype/system fonts), word-wrap it to fit
//! the card width, vertically + horizontally lay it out, optionally draw an
//! author byline, and encode to PNG. Mirrors blocks/add-text-to-image's text
//! rendering but generates the card from scratch instead of overlaying.

use std::io::Cursor;

use fontdue::Font;
use image::{ImageFormat, Rgba, RgbaImage};

const FONT_BYTES: &[u8] = include_bytes!("assets/DejaVuSansMono.ttf");

/// A named colour theme: background (top + bottom for a vertical gradient) and
/// the text/byline colours.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub bg_top: [u8; 3],
    pub bg_bottom: [u8; 3],
    pub text: [u8; 3],
    pub accent: [u8; 3],
}

/// Resolve a named theme. Unknown names error (reported with `theme_names()`).
pub fn theme(name: &str) -> Result<Theme, String> {
    let t = match name.trim().to_ascii_lowercase().as_str() {
        "dark" => Theme {
            bg_top: [0x1e, 0x29, 0x3b],
            bg_bottom: [0x0f, 0x17, 0x2a],
            text: [0xf1, 0xf5, 0xf9],
            accent: [0x93, 0xc5, 0xfd],
        },
        "light" => Theme {
            bg_top: [0xff, 0xff, 0xff],
            bg_bottom: [0xe9, 0xee, 0xf5],
            text: [0x1f, 0x2a, 0x37],
            accent: [0x4b, 0x5b, 0x6b],
        },
        "sunset" => Theme {
            bg_top: [0xff, 0x7e, 0x5f],
            bg_bottom: [0xfe, 0xb4, 0x7b],
            text: [0xff, 0xff, 0xff],
            accent: [0x3a, 0x1c, 0x12],
        },
        "ocean" => Theme {
            bg_top: [0x21, 0x96, 0xf3],
            bg_bottom: [0x0d, 0x47, 0xa1],
            text: [0xff, 0xff, 0xff],
            accent: [0xbb, 0xde, 0xfb],
        },
        "forest" => Theme {
            bg_top: [0x2e, 0x7d, 0x32],
            bg_bottom: [0x1b, 0x5e, 0x20],
            text: [0xf1, 0xf8, 0xe9],
            accent: [0xc5, 0xe1, 0xa5],
        },
        "grape" => Theme {
            bg_top: [0x8e, 0x2d, 0xe2],
            bg_bottom: [0x4a, 0x00, 0xe0],
            text: [0xff, 0xff, 0xff],
            accent: [0xe1, 0xc4, 0xff],
        },
        other => return Err(format!("unknown theme '{other}' (use one of: {})", theme_names())),
    };
    Ok(t)
}

/// Comma-separated list of valid theme names (for error messages + docs).
pub fn theme_names() -> &'static str {
    "dark, light, sunset, ocean, forest, grape"
}

/// Text horizontal alignment within the card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

pub fn parse_align(s: &str) -> Result<Align, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "left" => Ok(Align::Left),
        "center" | "centre" => Ok(Align::Center),
        "right" => Ok(Align::Right),
        other => Err(format!("unknown align '{other}' (use left, center, or right)")),
    }
}

/// Parse `#rrggbb` / `rrggbb` / `#rgb` into an opaque RGB. Empty → None (use
/// theme default). Errors on a malformed value.
fn parse_color(s: &str) -> Result<Option<[u8; 3]>, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let h = t.trim_start_matches('#');
    let hx = |c: &str| u8::from_str_radix(c, 16).map_err(|_| format!("invalid color '{s}'"));
    match h.len() {
        6 => Ok(Some([hx(&h[0..2])?, hx(&h[2..4])?, hx(&h[4..6])?])),
        3 => {
            let cs: Vec<char> = h.chars().collect();
            let d = |c: char| u8::from_str_radix(&format!("{c}{c}"), 16).map_err(|_| format!("invalid color '{s}'"));
            Ok(Some([d(cs[0])?, d(cs[1])?, d(cs[2])?]))
        }
        _ => Err(format!("invalid color '{s}' (use #rrggbb)")),
    }
}

/// Per-glyph advance width at `size` for `ch`.
fn advance(font: &Font, ch: char, size: f32) -> f32 {
    font.metrics(ch, size).advance_width
}

/// Width in px of a string at `size`.
fn text_width(font: &Font, s: &str, size: f32) -> f32 {
    s.chars().map(|c| advance(font, c, size)).sum()
}

/// Greedy word-wrap `text` to at most `max_w` px wide at `size`. Honours
/// explicit `\n` as hard breaks. A single word longer than the line is left
/// whole (the card renders it slightly overflowing rather than splitting
/// mid-word).
fn wrap(font: &Font, text: &str, size: f32, max_w: f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if text_width(font, &candidate, size) <= max_w || line.is_empty() {
                line = candidate;
            } else {
                out.push(std::mem::take(&mut line));
                line = word.to_string();
            }
        }
        out.push(line);
    }
    out
}

/// Alpha-blend `color` over the pixel at `(x, y)` with coverage `a`.
fn blend(img: &mut RgbaImage, x: i64, y: i64, color: [u8; 3], a: u8) {
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

/// Draw one glyph at a pen position + baseline, blending `color`.
fn draw_glyph(img: &mut RgbaImage, font: &Font, ch: char, size: f32, pen_x: f32, baseline: f32, color: [u8; 3]) {
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

/// Draw a full line of text. `start_x` is the pen origin.
fn draw_line(img: &mut RgbaImage, font: &Font, line: &str, size: f32, start_x: f32, baseline: f32, color: [u8; 3]) {
    let mut pen_x = start_x;
    for ch in line.chars() {
        draw_glyph(img, font, ch, size, pen_x, baseline, color);
        pen_x += advance(font, ch, size);
    }
}

/// All inputs needed to render a card.
#[derive(Clone, Debug)]
pub struct Card<'a> {
    pub text: &'a str,
    pub author: &'a str,
    pub theme: &'a str,
    pub width: u32,
    pub height: u32,
    pub font_size: f32,
    pub align: &'a str,
    /// Override the theme text colour (empty = theme default).
    pub text_color: &'a str,
    /// Override the theme background — solid colour, disables the gradient
    /// (empty = theme gradient).
    pub bg_color: &'a str,
}

/// Render the card to PNG bytes.
pub fn render(card: &Card) -> Result<Vec<u8>, String> {
    if card.text.trim().is_empty() {
        return Err("text is empty".into());
    }
    let w = card.width.clamp(200, 4000);
    let h = card.height.clamp(200, 4000);
    let mut size = if card.font_size.is_finite() && card.font_size >= 8.0 {
        card.font_size
    } else {
        48.0
    };
    let align = parse_align(card.align)?;
    let th = theme(card.theme)?;

    let text_color = parse_color(card.text_color)?.unwrap_or(th.text);
    let bg_override = parse_color(card.bg_color)?;

    let font = Font::from_bytes(FONT_BYTES, fontdue::FontSettings::default())
        .map_err(|e| format!("font load failed: {e}"))?;

    // Background: solid override, or a vertical gradient from the theme.
    let mut img = RgbaImage::new(w, h);
    for y in 0..h {
        let row = match bg_override {
            Some(c) => c,
            None => {
                let t = if h > 1 { y as f32 / (h - 1) as f32 } else { 0.0 };
                let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
                [
                    lerp(th.bg_top[0], th.bg_bottom[0]),
                    lerp(th.bg_top[1], th.bg_bottom[1]),
                    lerp(th.bg_top[2], th.bg_bottom[2]),
                ]
            }
        };
        for x in 0..w {
            img.put_pixel(x, y, Rgba([row[0], row[1], row[2], 255]));
        }
    }

    let pad = (w.min(h) as f32 * 0.10).max(24.0);
    let max_text_w = (w as f32 - 2.0 * pad).max(10.0);

    let line_metrics = |s: f32| font.horizontal_line_metrics(s);
    let author_size = |body: f32| (body * 0.5).max(12.0);

    // Auto-shrink the font so the wrapped text + byline fit vertically. The body
    // never drops below 8px; if it still overflows we keep the smallest size and
    // let it clip rather than failing.
    let (lines, line_h, ascent) = loop {
        let lm = line_metrics(size);
        let line_h = lm.map(|m| m.new_line_size).unwrap_or(size * 1.3);
        let ascent = lm.map(|m| m.ascent).unwrap_or(size * 0.8);
        let lines = wrap(&font, card.text.trim(), size, max_text_w);
        let mut total = lines.len() as f32 * line_h;
        if !card.author.trim().is_empty() {
            let am = line_metrics(author_size(size));
            total += am.map(|m| m.new_line_size).unwrap_or(author_size(size) * 1.3) + size * 0.5;
        }
        if total <= (h as f32 - 2.0 * pad) || size <= 8.0 {
            break (lines, line_h, ascent);
        }
        size *= 0.92;
    };

    let asz = author_size(size);
    let author_block = if card.author.trim().is_empty() {
        0.0
    } else {
        let am = line_metrics(asz);
        am.map(|m| m.new_line_size).unwrap_or(asz * 1.3) + size * 0.5
    };
    let body_h = lines.len() as f32 * line_h;
    let total_h = body_h + author_block;
    let mut baseline = (h as f32 - total_h) / 2.0 + ascent;

    let line_start = |lw: f32| -> f32 {
        match align {
            Align::Left => pad,
            Align::Center => (w as f32 - lw) / 2.0,
            Align::Right => w as f32 - pad - lw,
        }
    };

    for line in &lines {
        let lw = text_width(&font, line, size);
        draw_line(&mut img, &font, line, size, line_start(lw), baseline, text_color);
        baseline += line_h;
    }

    if !card.author.trim().is_empty() {
        let byline = format!("\u{2014} {}", card.author.trim());
        let aw = text_width(&font, &byline, asz);
        let a_ascent = line_metrics(asz).map(|m| m.ascent).unwrap_or(asz * 0.8);
        // baseline is now one line past the last body line.
        let a_baseline = baseline - line_h + ascent + size * 0.5 + a_ascent;
        let a_start = match align {
            Align::Left => pad,
            Align::Center => (w as f32 - aw) / 2.0,
            Align::Right => w as f32 - pad - aw,
        };
        draw_line(&mut img, &font, &byline, asz, a_start, a_baseline, th.accent);
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

    fn default_card(text: &str) -> Card {
        Card {
            text,
            author: "",
            theme: "dark",
            width: 1080,
            height: 1080,
            font_size: 48.0,
            align: "center",
            text_color: "",
            bg_color: "",
        }
    }

    fn dims(bytes: &[u8]) -> (u32, u32) {
        let img = image::load_from_memory(bytes).unwrap();
        (img.width(), img.height())
    }

    #[test]
    fn renders_a_card_at_requested_size() {
        let mut c = default_card("Stay hungry, stay foolish.");
        c.width = 800;
        c.height = 600;
        let png = render(&c).unwrap();
        assert_eq!(dims(&png), (800, 600));
        assert_eq!(&png[0..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn draws_text_pixels() {
        let png = render(&default_card("Hello world")).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let mut colors = std::collections::HashSet::new();
        for p in img.pixels() {
            colors.insert((p[0], p[1], p[2]));
        }
        assert!(colors.len() > 5, "expected text + gradient to produce many colours");
    }

    #[test]
    fn author_byline_changes_output() {
        let base = render(&default_card("A quote")).unwrap();
        let mut c = default_card("A quote");
        c.author = "Someone";
        let withauthor = render(&c).unwrap();
        assert_ne!(base, withauthor, "adding an author should change the image");
    }

    #[test]
    fn long_text_wraps_and_fits() {
        let long = "This is a fairly long quote that absolutely must wrap across \
                    multiple lines and still fit within the bounds of the card without panicking.";
        let mut c = default_card(long);
        c.width = 600;
        c.height = 600;
        let png = render(&c).unwrap();
        assert_eq!(dims(&png), (600, 600));
    }

    #[test]
    fn solid_bg_override_applies() {
        let mut c = default_card("hi");
        c.bg_color = "#ff0000";
        let png = render(&c).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let corner = img.get_pixel(2, 2);
        assert_eq!([corner[0], corner[1], corner[2]], [255, 0, 0]);
    }

    #[test]
    fn all_themes_resolve() {
        for name in ["dark", "light", "sunset", "ocean", "forest", "grape"] {
            assert!(theme(name).is_ok(), "{name} should be a theme");
        }
        assert!(theme("nope").is_err());
    }

    #[test]
    fn empty_text_errors() {
        assert!(render(&default_card("   ")).is_err());
    }

    #[test]
    fn bad_color_errors() {
        let mut c = default_card("hi");
        c.text_color = "zzz";
        assert!(render(&c).is_err());
    }

    #[test]
    fn bad_align_errors() {
        let mut c = default_card("hi");
        c.align = "diagonal";
        assert!(render(&c).is_err());
    }

    #[test]
    fn align_parsing() {
        assert_eq!(parse_align("LEFT").unwrap(), Align::Left);
        assert_eq!(parse_align("centre").unwrap(), Align::Center);
        assert_eq!(parse_align("Right").unwrap(), Align::Right);
    }

    #[test]
    fn color_parsing_variants() {
        assert_eq!(parse_color("#ff0000").unwrap(), Some([255, 0, 0]));
        assert_eq!(parse_color("0f0").unwrap(), Some([0, 255, 0]));
        assert_eq!(parse_color("").unwrap(), None);
        assert!(parse_color("xyz").is_err());
    }
}
