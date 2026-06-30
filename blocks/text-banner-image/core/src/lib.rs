//! gizza-ai/text-banner-image core — pure-Rust renderer that turns a short
//! headline into a wide, stylized PNG banner. No wafer/wasm-bindgen deps so it
//! is shared by the chat skill block and the CLI.
//!
//! Pipeline: paint a diagonal gradient background (the `bg_color` blended
//! toward the `accent_color`) + a vertical accent stripe on the left edge →
//! word-wrap the headline to the banner width with a bundled bold font
//! (fontdue — no freetype/system fonts) → auto-shrink so it fits → draw it with
//! an optional drop shadow and/or outline + an accent underline → encode PNG.
//! Mirrors blocks/text-image-card's text rendering but lays the type out for a
//! short wide headline rather than a square quote card.

use std::io::Cursor;

use fontdue::Font;
use image::{ImageFormat, Rgba, RgbaImage};

const FONT_BYTES: &[u8] = include_bytes!("assets/LiberationSans-Bold.ttf");

/// Text horizontal alignment within the banner.
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

/// Parse `#rrggbb` / `rrggbb` / `#rgb` into an opaque RGB. Empty → fall back to
/// the supplied default. Errors on a malformed value.
fn parse_color(s: &str, default: [u8; 3]) -> Result<[u8; 3], String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    let h = t.trim_start_matches('#');
    let hx = |c: &str| u8::from_str_radix(c, 16).map_err(|_| format!("invalid color '{s}'"));
    match h.len() {
        6 => Ok([hx(&h[0..2])?, hx(&h[2..4])?, hx(&h[4..6])?]),
        3 => {
            let cs: Vec<char> = h.chars().collect();
            let d = |c: char| u8::from_str_radix(&format!("{c}{c}"), 16).map_err(|_| format!("invalid color '{s}'"));
            Ok([d(cs[0])?, d(cs[1])?, d(cs[2])?])
        }
        _ => Err(format!("invalid color '{s}' (use #rrggbb)")),
    }
}

/// Linear blend of two colours, `t` in 0..=1 (0 → a, 1 → b).
fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t).round() as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t).round() as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t).round() as u8,
    ]
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
/// whole (auto-shrink later brings it within bounds).
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

/// Alpha-blend `color` over the pixel at `(x, y)` with coverage `a` (0..=255).
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

/// Draw one glyph at a pen position + baseline, blending `color` with the
/// glyph coverage scaled by `alpha` (0..=1, for soft drop shadows).
fn draw_glyph(img: &mut RgbaImage, font: &Font, ch: char, size: f32, pen_x: f32, baseline: f32, color: [u8; 3], alpha: f32) {
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
                let a = (cov as f32 * alpha).round().clamp(0.0, 255.0) as u8;
                blend(img, left as i64 + gx as i64, top as i64 + gy as i64, color, a);
            }
        }
    }
}

/// Draw a full line of text from pen origin `start_x` at `baseline`.
fn draw_line(img: &mut RgbaImage, font: &Font, line: &str, size: f32, start_x: f32, baseline: f32, color: [u8; 3], alpha: f32) {
    let mut pen_x = start_x;
    for ch in line.chars() {
        draw_glyph(img, font, ch, size, pen_x, baseline, color, alpha);
        pen_x += advance(font, ch, size);
    }
}

/// Fill an axis-aligned rectangle (clipped to the canvas) with a solid colour.
fn fill_rect(img: &mut RgbaImage, x0: i64, y0: i64, x1: i64, y1: i64, color: [u8; 3]) {
    let xs = x0.max(0);
    let ys = y0.max(0);
    let xe = x1.min(img.width() as i64);
    let ye = y1.min(img.height() as i64);
    for y in ys..ye {
        for x in xs..xe {
            img.put_pixel(x as u32, y as u32, Rgba([color[0], color[1], color[2], 255]));
        }
    }
}

/// All inputs needed to render a banner.
#[derive(Clone, Debug)]
pub struct Banner<'a> {
    pub text: &'a str,
    pub width: u32,
    pub height: u32,
    pub bg_color: &'a str,
    pub text_color: &'a str,
    pub accent_color: &'a str,
    pub align: &'a str,
    /// Starting font size in px; `0` (or anything < 8) means auto-size to fit.
    pub font_size: f32,
    pub shadow: bool,
    pub outline: bool,
}

/// Render the banner to PNG bytes.
pub fn render(banner: &Banner) -> Result<Vec<u8>, String> {
    if banner.text.trim().is_empty() {
        return Err("text is empty".into());
    }
    let w = banner.width.clamp(200, 4000);
    let h = banner.height.clamp(100, 4000);
    let align = parse_align(banner.align)?;
    let bg = parse_color(banner.bg_color, [0x11, 0x18, 0x27])?;
    let text_color = parse_color(banner.text_color, [0xff, 0xff, 0xff])?;
    let accent = parse_color(banner.accent_color, [0x60, 0xa5, 0xfa])?;

    let font = Font::from_bytes(FONT_BYTES, fontdue::FontSettings::default())
        .map_err(|e| format!("font load failed: {e}"))?;

    // Background: a diagonal gradient from `bg` toward an accent-tinted, slightly
    // deepened shade so the banner reads as stylized but the headline stays
    // legible (the tint is subtle — 28% accent at the far corner).
    let far = mix(mix(bg, accent, 0.28), [0, 0, 0], 0.08);
    let mut img = RgbaImage::new(w, h);
    let wd = (w.max(2) - 1) as f32;
    let hd = (h.max(2) - 1) as f32;
    for y in 0..h {
        for x in 0..w {
            let t = (x as f32 / wd + y as f32 / hd) * 0.5;
            let c = mix(bg, far, t);
            img.put_pixel(x, y, Rgba([c[0], c[1], c[2], 255]));
        }
    }

    // Vertical accent stripe on the left edge — a stylized banner marker.
    let bar_w = ((w as f32 * 0.012).round() as i64).clamp(5, 18);
    fill_rect(&mut img, 0, 0, bar_w, h as i64, accent);

    let pad = (w.min(h) as f32 * 0.12).max(20.0);
    let left_inset = pad + bar_w as f32;
    let max_text_w = (w as f32 - left_inset - pad).max(10.0);
    let avail_h = (h as f32 - 2.0 * pad).max(10.0);

    // Starting size: the requested font_size, or auto (a fraction of height).
    let mut size = if banner.font_size.is_finite() && banner.font_size >= 8.0 {
        banner.font_size
    } else {
        (h as f32 * 0.45).clamp(16.0, 200.0)
    };

    let line_metrics = |s: f32| font.horizontal_line_metrics(s);

    // Auto-shrink until the wrapped text fits both width and height. Never grow
    // a fixed font_size; never drop below 8px (then it clips rather than fails).
    let (lines, line_h, ascent) = loop {
        let lm = line_metrics(size);
        let line_h = lm.map(|m| m.new_line_size).unwrap_or(size * 1.3);
        let ascent = lm.map(|m| m.ascent).unwrap_or(size * 0.8);
        let lines = wrap(&font, banner.text.trim(), size, max_text_w);
        let longest = lines.iter().map(|l| text_width(&font, l, size)).fold(0.0_f32, f32::max);
        let total = lines.len() as f32 * line_h;
        if (total <= avail_h && longest <= max_text_w) || size <= 8.0 {
            break (lines, line_h, ascent);
        }
        size *= 0.92;
    };

    let body_h = lines.len() as f32 * line_h;
    // Reserve room for the accent underline so the block stays vertically centred.
    let underline_gap = size * 0.45;
    let underline_h = (size * 0.12).max(3.0);
    let total_h = body_h + underline_gap + underline_h;
    let mut baseline = ((h as f32 - total_h) / 2.0).max(pad) + ascent;

    let line_start = |lw: f32| -> f32 {
        match align {
            Align::Left => left_inset,
            Align::Center => left_inset + (max_text_w - lw) / 2.0,
            Align::Right => left_inset + (max_text_w - lw),
        }
    };

    // Shadow + outline offsets scale with the font so they read at any size.
    let shadow_off = (size * 0.06).max(2.0);
    let outline_off = (size * 0.035).max(1.0);
    let shadow_color = [0u8, 0, 0];
    let outline_color = mix(bg, [0, 0, 0], 0.55);

    let mut last_baseline = baseline;
    for line in &lines {
        let lw = text_width(&font, line, size);
        let sx = line_start(lw);
        if banner.shadow {
            draw_line(&mut img, &font, line, size, sx + shadow_off, baseline + shadow_off, shadow_color, 0.45);
        }
        if banner.outline {
            for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                draw_line(&mut img, &font, line, size, sx + dx * outline_off, baseline + dy * outline_off, outline_color, 1.0);
            }
        }
        draw_line(&mut img, &font, line, size, sx, baseline, text_color, 1.0);
        last_baseline = baseline;
        baseline += line_h;
    }

    // Accent underline beneath the headline, aligned to the text block.
    let longest = lines.iter().map(|l| text_width(&font, l, size)).fold(0.0_f32, f32::max);
    let ul_w = longest.min(max_text_w).max(20.0) * 0.5;
    let ul_y = (last_baseline + underline_gap).round() as i64;
    let ul_x = match align {
        Align::Left => left_inset,
        Align::Center => left_inset + (max_text_w - ul_w) / 2.0,
        Align::Right => left_inset + (max_text_w - ul_w),
    };
    fill_rect(&mut img, ul_x as i64, ul_y, (ul_x + ul_w) as i64, ul_y + underline_h.round() as i64, accent);

    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_banner(text: &str) -> Banner {
        Banner {
            text,
            width: 1200,
            height: 400,
            bg_color: "#111827",
            text_color: "#ffffff",
            accent_color: "#60a5fa",
            align: "center",
            font_size: 0.0,
            shadow: true,
            outline: false,
        }
    }

    fn dims(bytes: &[u8]) -> (u32, u32) {
        let img = image::load_from_memory(bytes).unwrap();
        (img.width(), img.height())
    }

    #[test]
    fn renders_at_requested_size() {
        let mut b = default_banner("Launch Day");
        b.width = 1000;
        b.height = 300;
        let png = render(&b).unwrap();
        assert_eq!(dims(&png), (1000, 300));
        assert_eq!(&png[0..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn default_dimensions_are_banner_shaped() {
        let png = render(&default_banner("Big News")).unwrap();
        assert_eq!(dims(&png), (1200, 400));
    }

    #[test]
    fn draws_many_colors() {
        let png = render(&default_banner("Hello world")).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let mut colors = std::collections::HashSet::new();
        for p in img.pixels() {
            colors.insert((p[0], p[1], p[2]));
        }
        assert!(colors.len() > 10, "expected text + gradient + accent to produce many colours");
    }

    #[test]
    fn accent_stripe_painted_on_left_edge() {
        let png = render(&default_banner("X")).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let p = img.get_pixel(1, img.height() / 2);
        assert_eq!([p[0], p[1], p[2]], [0x60, 0xa5, 0xfa], "left edge should be the accent colour");
    }

    #[test]
    fn shadow_toggle_changes_output() {
        let with = render(&default_banner("Shadows on")).unwrap();
        let mut b = default_banner("Shadows on");
        b.shadow = false;
        let without = render(&b).unwrap();
        assert_ne!(with, without, "toggling the shadow should change the image");
    }

    #[test]
    fn outline_toggle_changes_output() {
        let base = render(&default_banner("Outline me")).unwrap();
        let mut b = default_banner("Outline me");
        b.outline = true;
        let outlined = render(&b).unwrap();
        assert_ne!(base, outlined, "enabling the outline should change the image");
    }

    #[test]
    fn long_text_wraps_and_fits() {
        let long = "This is a fairly long banner headline that absolutely must wrap \
                    across multiple lines and still fit within the banner without panicking.";
        let png = render(&default_banner(long)).unwrap();
        assert_eq!(dims(&png), (1200, 400));
    }

    #[test]
    fn fixed_font_size_is_used() {
        let mut b = default_banner("Hi");
        b.font_size = 48.0;
        let png = render(&b).unwrap();
        assert_eq!(dims(&png), (1200, 400));
    }

    #[test]
    fn custom_colors_apply_to_background() {
        let mut b = default_banner("Hi");
        b.bg_color = "#000000";
        let png = render(&b).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        // Top-left interior pixel (just right of the accent stripe) is the bg.
        let p = img.get_pixel(40, 4);
        assert!(p[0] < 30 && p[1] < 30 && p[2] < 30, "top-left should be near-black bg, got {:?}", p);
    }

    #[test]
    fn all_alignments_render() {
        for a in ["left", "center", "right"] {
            let mut b = default_banner("Aligned");
            b.align = a;
            assert!(render(&b).is_ok(), "{a} should render");
        }
    }

    #[test]
    fn empty_text_errors() {
        assert!(render(&default_banner("   ")).is_err());
    }

    #[test]
    fn bad_color_errors() {
        let mut b = default_banner("hi");
        b.bg_color = "zzz";
        assert!(render(&b).is_err());
    }

    #[test]
    fn bad_align_errors() {
        let mut b = default_banner("hi");
        b.align = "diagonal";
        assert!(render(&b).is_err());
    }

    #[test]
    fn align_parsing() {
        assert_eq!(parse_align("LEFT").unwrap(), Align::Left);
        assert_eq!(parse_align("centre").unwrap(), Align::Center);
        assert_eq!(parse_align("Right").unwrap(), Align::Right);
    }

    #[test]
    fn color_parsing_variants() {
        assert_eq!(parse_color("#ff0000", [0, 0, 0]).unwrap(), [255, 0, 0]);
        assert_eq!(parse_color("0f0", [0, 0, 0]).unwrap(), [0, 255, 0]);
        assert_eq!(parse_color("", [1, 2, 3]).unwrap(), [1, 2, 3]);
        assert!(parse_color("xyz", [0, 0, 0]).is_err());
    }
}
