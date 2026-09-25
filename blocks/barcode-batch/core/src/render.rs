//! PNG + SVG rendering of an encoded module vector, and the colour parser both share.
//!
//! Geometry is identical across the two renderers so a PNG and an SVG of the same
//! row are the same symbol at the same proportions:
//!
//! ```text
//! width  = (quiet_zone + modules + quiet_zone) * module_width
//! height = bar_height + (text gap + text height, when the HRI is shown)
//! ```

use std::fmt::Write as _;
use std::io::Cursor;

use font8x8::legacy::BASIC_LEGACY;
use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};

use crate::symbology::Encoded;

/// An RGBA colour. Alpha 0 means "paint nothing" (the `transparent` keyword).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl Rgba {
    pub fn is_transparent(self) -> bool {
        self.3 == 0
    }

    /// `#rrggbb`, dropping any alpha — what SVG/PDF fills want.
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

const NAMED: &[(&str, (u8, u8, u8))] = &[
    ("black", (0, 0, 0)),
    ("white", (255, 255, 255)),
    ("red", (255, 0, 0)),
    ("green", (0, 128, 0)),
    ("blue", (0, 0, 255)),
    ("navy", (0, 0, 128)),
    ("yellow", (255, 255, 0)),
    ("orange", (255, 165, 0)),
    ("purple", (128, 0, 128)),
    ("gray", (128, 128, 128)),
    ("grey", (128, 128, 128)),
    ("silver", (192, 192, 192)),
];

/// Parse `#rgb`, `#rrggbb`, `#rrggbbaa`, a common colour name, or `transparent`.
pub fn parse_color(s: &str, what: &str) -> Result<Rgba, String> {
    let t = s.trim().to_ascii_lowercase();
    if t == "transparent" || t == "none" {
        return Ok(Rgba(255, 255, 255, 0));
    }
    if let Some((_, rgb)) = NAMED.iter().find(|(n, _)| *n == t) {
        return Ok(Rgba(rgb.0, rgb.1, rgb.2, 255));
    }
    let hex = t.strip_prefix('#').unwrap_or(&t);
    let nib = |c: char| c.to_digit(16).map(|v| v as u8);
    let bytes: Vec<u8> = match hex.len() {
        3 | 4 => hex
            .chars()
            .map(|c| nib(c).map(|v| v * 17))
            .collect::<Option<_>>()
            .ok_or_else(|| bad_color(s, what))?,
        6 | 8 => (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok())
            .collect::<Option<_>>()
            .ok_or_else(|| bad_color(s, what))?,
        _ => return Err(bad_color(s, what)),
    };
    Ok(Rgba(
        bytes[0],
        bytes[1],
        bytes[2],
        bytes.get(3).copied().unwrap_or(255),
    ))
}

fn bad_color(s: &str, what: &str) -> String {
    format!("{what} `{s}` is not a colour — use #rgb, #rrggbb, a common colour name, or transparent")
}

/// Pixel geometry shared by the PNG and SVG renderers.
#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    pub module_width: u32,
    pub bar_height: u32,
    pub quiet_zone: u32,
    pub show_text: bool,
    pub text_size: u32,
}

/// Integer upscale factor applied to the 8x8 bitmap glyphs.
fn glyph_scale(text_size: u32) -> u32 {
    (text_size / 8).max(1)
}

impl Geometry {
    fn text_block(&self) -> (u32, u32) {
        if !self.show_text {
            return (0, 0);
        }
        let s = glyph_scale(self.text_size);
        (2 * s, 8 * s) // (gap under the bars, glyph height)
    }

    pub fn width(&self, modules: usize) -> u32 {
        (modules as u32 + 2 * self.quiet_zone) * self.module_width
    }

    pub fn height(&self) -> u32 {
        let (gap, text) = self.text_block();
        self.bar_height + gap + text
    }
}

/// Merge consecutive `1` modules into `(start_module, module_count)` runs so the
/// SVG carries one `<rect>` per bar instead of one per module.
pub fn bar_runs(modules: &[u8]) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut i = 0;
    while i < modules.len() {
        if modules[i] == 1 {
            let start = i;
            while i < modules.len() && modules[i] == 1 {
                i += 1;
            }
            runs.push((start, i - start));
        } else {
            i += 1;
        }
    }
    runs
}

/// Characters the 8x8 bitmap font can draw; anything else prints as `?`.
fn glyph(c: char) -> [u8; 8] {
    let idx = if (c as u32) < 128 { c as usize } else { '?' as usize };
    BASIC_LEGACY[idx]
}

pub fn render_png(
    enc: &Encoded,
    geo: &Geometry,
    fg: Rgba,
    bg: Rgba,
) -> Result<Vec<u8>, String> {
    let w = geo.width(enc.modules.len());
    let h = geo.height();
    if w == 0 || h == 0 {
        return Err("barcode geometry collapsed to zero pixels".to_string());
    }
    let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
    let put = |buf: &mut [u8], x: u32, y: u32, c: Rgba| {
        let i = ((y as usize) * (w as usize) + x as usize) * 4;
        buf[i] = c.0;
        buf[i + 1] = c.1;
        buf[i + 2] = c.2;
        buf[i + 3] = c.3;
    };
    if !bg.is_transparent() {
        for y in 0..h {
            for x in 0..w {
                put(&mut buf, x, y, bg);
            }
        }
    }

    for (start, len) in bar_runs(&enc.modules) {
        let x0 = (geo.quiet_zone + start as u32) * geo.module_width;
        let x1 = x0 + len as u32 * geo.module_width;
        for y in 0..geo.bar_height {
            for x in x0..x1.min(w) {
                put(&mut buf, x, y, fg);
            }
        }
    }

    let (gap, text_h) = geo.text_block();
    if geo.show_text && text_h > 0 {
        let s = glyph_scale(geo.text_size);
        let chars: Vec<char> = enc.hri.chars().collect();
        let text_w = chars.len() as u32 * 8 * s;
        // Centre the HRI, clipping symmetrically if it is wider than the symbol.
        let x0 = (w as i64 - text_w as i64) / 2;
        let y0 = geo.bar_height + gap;
        for (ci, ch) in chars.iter().enumerate() {
            let g = glyph(*ch);
            let cx = x0 + (ci as i64) * (8 * s) as i64;
            for (row, bits) in g.iter().enumerate() {
                for col in 0..8u32 {
                    if bits & (1 << col) == 0 {
                        continue;
                    }
                    for dy in 0..s {
                        for dx in 0..s {
                            let px = cx + (col * s + dx) as i64;
                            let py = y0 + row as u32 * s + dy;
                            if px >= 0 && (px as u32) < w && py < h {
                                put(&mut buf, px as u32, py, fg);
                            }
                        }
                    }
                }
            }
        }
    }

    let mut out = Vec::new();
    PngEncoder::new(Cursor::new(&mut out))
        .write_image(&buf, w, h, ExtendedColorType::Rgba8)
        .map_err(|e| format!("could not encode PNG: {e}"))?;
    Ok(out)
}

pub fn render_svg(enc: &Encoded, geo: &Geometry, fg: Rgba, bg: Rgba) -> String {
    let w = geo.width(enc.modules.len());
    let h = geo.height();
    let mut s = String::new();
    let _ = write!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" shape-rendering="crispEdges">"#
    );
    if !bg.is_transparent() {
        let _ = write!(
            s,
            r#"<rect width="{w}" height="{h}" fill="{}"/>"#,
            bg.hex()
        );
    }
    let fill = fg.hex();
    for (start, len) in bar_runs(&enc.modules) {
        let x = (geo.quiet_zone + start as u32) * geo.module_width;
        let bw = len as u32 * geo.module_width;
        let _ = write!(
            s,
            r#"<rect x="{x}" y="0" width="{bw}" height="{}" fill="{fill}"/>"#,
            geo.bar_height
        );
    }
    let (gap, text_h) = geo.text_block();
    if geo.show_text && text_h > 0 {
        let y = geo.bar_height + gap + text_h;
        let _ = write!(
            s,
            r#"<text x="{}" y="{y}" font-family="monospace" font-size="{}" fill="{fill}" text-anchor="middle">{}</text>"#,
            w / 2,
            geo.text_size,
            xml_escape(&enc.hri)
        );
    }
    s.push_str("</svg>");
    s
}

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbology::{encode, Symbology};

    fn geo() -> Geometry {
        Geometry {
            module_width: 2,
            bar_height: 100,
            quiet_zone: 10,
            show_text: true,
            text_size: 20,
        }
    }

    #[test]
    fn png_is_a_real_png_with_expected_dimensions() {
        let enc = encode(Symbology::Ean13, "5901234123457", true).unwrap();
        let g = geo();
        let png = render_png(&enc, &g, Rgba(0, 0, 0, 255), Rgba(255, 255, 255, 255)).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        // (95 modules + 2x10 quiet) * 2 px = 230 px wide.
        assert_eq!(g.width(enc.modules.len()), 230);
        // 100 bar + 2*2 gap + 8*2 glyph = 120 px tall.
        assert_eq!(g.height(), 120);
        let w = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let h = u32::from_be_bytes(png[20..24].try_into().unwrap());
        assert_eq!((w, h), (230, 120));
    }

    #[test]
    fn svg_carries_merged_bars_and_the_hri() {
        let enc = encode(Symbology::Code128, "SKU-1001", true).unwrap();
        let svg = render_svg(&enc, &geo(), Rgba(0, 0, 0, 255), Rgba(255, 255, 255, 255));
        assert!(svg.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg""#));
        assert!(svg.contains(r##"fill="#000000""##));
        assert!(svg.contains(">SKU-1001</text>"));
        // Merged runs: strictly fewer rects than modules.
        let rects = svg.matches("<rect").count();
        assert!(rects < enc.modules.len(), "{rects} rects");
    }

    #[test]
    fn transparent_background_paints_no_backdrop() {
        let enc = encode(Symbology::Code128, "AB", true).unwrap();
        let bg = parse_color("transparent", "Background colour").unwrap();
        assert!(bg.is_transparent());
        let svg = render_svg(&enc, &geo(), Rgba(0, 0, 0, 255), bg);
        assert!(!svg.contains(r#"<rect width="#), "{svg}");
    }

    #[test]
    fn color_parser_accepts_the_advertised_forms() {
        assert_eq!(parse_color("#f00", "c").unwrap(), Rgba(255, 0, 0, 255));
        assert_eq!(parse_color("#ff0000", "c").unwrap(), Rgba(255, 0, 0, 255));
        assert_eq!(parse_color("navy", "c").unwrap(), Rgba(0, 0, 128, 255));
        assert!(parse_color("zzz", "Foreground colour").is_err());
    }

    #[test]
    fn hidden_text_shortens_the_image() {
        let mut g = geo();
        g.show_text = false;
        assert_eq!(g.height(), 100);
    }
}
