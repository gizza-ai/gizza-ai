//! gizza-ai/svg-placeholder-generator core — render a placeholder image as a
//! scalable SVG at a chosen size, with a centred label (the dimensions by
//! default, or custom text), a background colour, and a text colour.
//!
//! Pure-Rust (a hand-built SVG string — no image/render dep), so it runs on ALL
//! backends including the chat Service Worker. No wafer/wasm-bindgen deps.

/// An RGB colour (alpha is dropped — placeholders are opaque, but we still
/// accept #rgba/#rrggbbaa hex and ignore the alpha channel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

/// Parse a CSS hex colour: #rgb, #rgba, #rrggbb, or #rrggbbaa (with or without
/// the leading '#'; alpha, if present, is ignored).
fn parse_hex(s: &str) -> Result<Rgb, String> {
    let body = s.trim().trim_start_matches('#');
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("invalid colour '{s}' (use #rgb, #rgba, #rrggbb, or #rrggbbaa hex)"));
    }
    let expand = |c: char| -> u8 {
        let v = c.to_digit(16).unwrap() as u8;
        v << 4 | v
    };
    let two = |a: char, b: char| -> u8 {
        (a.to_digit(16).unwrap() as u8) << 4 | (b.to_digit(16).unwrap() as u8)
    };
    let ch: Vec<char> = body.chars().collect();
    match ch.len() {
        3 => Ok(Rgb { r: expand(ch[0]), g: expand(ch[1]), b: expand(ch[2]) }),
        4 => Ok(Rgb { r: expand(ch[0]), g: expand(ch[1]), b: expand(ch[2]) }),
        6 => Ok(Rgb { r: two(ch[0], ch[1]), g: two(ch[2], ch[3]), b: two(ch[4], ch[5]) }),
        8 => Ok(Rgb { r: two(ch[0], ch[1]), g: two(ch[2], ch[3]), b: two(ch[4], ch[5]) }),
        _ => Err(format!("invalid colour '{s}' (use #rgb, #rgba, #rrggbb, or #rrggbbaa hex)")),
    }
}

fn hex6(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// Perceptual luminance (ITU-R BT.601), 0..=255. Used to pick a readable text
/// colour when none is given.
fn luminance(c: Rgb) -> f32 {
    0.299 * c.r as f32 + 0.587 * c.g as f32 + 0.114 * c.b as f32
}

/// Escape text for inclusion as XML/SVG character data and attribute values.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Format an `f32` for SVG without a trailing `.0` for whole numbers (keeps the
/// markup tidy: `12` not `12.0`, but `12.5` stays).
fn num(v: f32) -> String {
    if (v - v.round()).abs() < 1e-4 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    }
}

/// Estimate a font size (px) that visually fits the label inside the box. Starts
/// from a fraction of the shorter side, then shrinks so the text width (roughly
/// 0.6em per character for a sans-serif) stays within ~90% of the width.
fn auto_font_size(w: u32, h: u32, label_chars: usize) -> f32 {
    let base = (w.min(h) as f32 / 5.0).max(8.0);
    let chars = label_chars.max(1) as f32;
    let by_width = (w as f32 * 0.9) / (chars * 0.6);
    base.min(by_width).max(8.0)
}

/// The rendered placeholder SVG.
pub struct Generated {
    pub svg: String,
    pub width: u32,
    pub height: u32,
    pub label: String,
    pub font_size: f32,
}

/// Generate a placeholder SVG.
///
/// * `width`/`height` — document size in pixels (clamped to 1..=4096).
/// * `text` — the centred label; if empty, the dimensions `"{w}×{h}"` are used.
/// * `bg_color` — background hex colour (default supplied by the caller).
/// * `text_color` — label hex colour; if empty, a readable colour (dark or
///   white) is chosen automatically from the background luminance.
/// * `font_size` — label size in px; if `0`, a size that fits the box is chosen.
/// * `font_family` — CSS font-family for the label (e.g. `"sans-serif"`).
pub fn generate(
    width: u32,
    height: u32,
    text: &str,
    bg_color: &str,
    text_color: &str,
    font_size: f64,
    font_family: &str,
) -> Result<Generated, String> {
    let w = width.clamp(1, 4096);
    let h = height.clamp(1, 4096);

    let bg = parse_hex(bg_color)?;
    let fg = if text_color.trim().is_empty() {
        // Auto contrast: dark text on light backgrounds, white on dark.
        if luminance(bg) > 140.0 {
            Rgb { r: 0x33, g: 0x33, b: 0x33 }
        } else {
            Rgb { r: 0xff, g: 0xff, b: 0xff }
        }
    } else {
        parse_hex(text_color)?
    };

    let label = if text.trim().is_empty() { format!("{w}\u{00d7}{h}") } else { text.to_string() };

    let fs = if font_size > 0.0 {
        (font_size as f32).clamp(1.0, 4096.0)
    } else {
        auto_font_size(w, h, label.chars().count())
    };

    let family = if font_family.trim().is_empty() { "sans-serif" } else { font_family.trim() };

    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;

    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" preserveAspectRatio=\"xMidYMid meet\">\
<rect width=\"{w}\" height=\"{h}\" fill=\"{bg}\"/>\
<text x=\"{cx}\" y=\"{cy}\" font-family=\"{family}\" font-size=\"{fs}\" fill=\"{fg}\" text-anchor=\"middle\" dominant-baseline=\"central\">{label}</text>\
</svg>",
        bg = hex6(bg),
        fg = hex6(fg),
        cx = num(cx),
        cy = num(cy),
        family = xml_escape(family),
        fs = num(fs),
        label = xml_escape(&label),
    );

    Ok(Generated { svg, width: w, height: h, label, font_size: fs })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_variants() {
        assert_eq!(parse_hex("#f00").unwrap(), Rgb { r: 255, g: 0, b: 0 });
        assert_eq!(parse_hex("00ff00").unwrap(), Rgb { r: 0, g: 255, b: 0 });
        assert_eq!(parse_hex("#112233").unwrap(), Rgb { r: 0x11, g: 0x22, b: 0x33 });
        // alpha is accepted but ignored
        assert_eq!(parse_hex("#00000080").unwrap(), Rgb { r: 0, g: 0, b: 0 });
        assert!(parse_hex("#xyz").is_err());
        assert!(parse_hex("").is_err());
        assert!(parse_hex("#12345").is_err());
    }

    #[test]
    fn default_label_is_dimensions() {
        let g = generate(300, 150, "", "#cccccc", "", 0.0, "").unwrap();
        assert_eq!(g.label, "300\u{00d7}150");
        assert!(g.svg.contains(">300\u{00d7}150<"));
        assert!(g.svg.contains("width=\"300\""));
        assert!(g.svg.contains("height=\"150\""));
        assert!(g.svg.contains("viewBox=\"0 0 300 150\""));
    }

    #[test]
    fn custom_text_and_colours() {
        let g = generate(200, 200, "Hello", "#000000", "#ffffff", 0.0, "").unwrap();
        assert_eq!(g.label, "Hello");
        assert!(g.svg.contains(">Hello<"));
        assert!(g.svg.contains("fill=\"#000000\"")); // rect
        assert!(g.svg.contains("fill=\"#ffffff\"")); // text
    }

    #[test]
    fn auto_text_colour_contrasts_background() {
        // Light bg → dark text.
        let light = generate(100, 100, "x", "#eeeeee", "", 12.0, "").unwrap();
        assert!(light.svg.contains("fill=\"#333333\""));
        // Dark bg → white text.
        let dark = generate(100, 100, "x", "#222222", "", 12.0, "").unwrap();
        assert!(dark.svg.contains("fill=\"#ffffff\""));
    }

    #[test]
    fn explicit_font_size_used() {
        let g = generate(400, 300, "Hi", "#ccc", "#000", 48.0, "").unwrap();
        assert_eq!(g.font_size, 48.0);
        assert!(g.svg.contains("font-size=\"48\""));
    }

    #[test]
    fn auto_font_size_fits_and_is_positive() {
        let g = generate(600, 400, "", "#ccc", "", 0.0, "").unwrap();
        assert!(g.font_size >= 8.0);
        // shorter side 400 → base 80, but the "600×400" label width caps it
        assert!(g.font_size <= 80.0);
    }

    #[test]
    fn xml_special_chars_escaped() {
        let g = generate(100, 100, "a<b>&\"'", "#ccc", "#000", 12.0, "").unwrap();
        assert!(g.svg.contains("a&lt;b&gt;&amp;&quot;&apos;"));
        assert!(!g.svg.contains("a<b>"));
    }

    #[test]
    fn size_is_clamped() {
        let g = generate(0, 99999, "", "#ccc", "", 12.0, "").unwrap();
        assert_eq!((g.width, g.height), (1, 4096));
    }

    #[test]
    fn bad_colour_errors() {
        assert!(generate(10, 10, "", "not-a-colour", "", 0.0, "").is_err());
        assert!(generate(10, 10, "", "#ccc", "#zzz", 0.0, "").is_err());
    }

    #[test]
    fn custom_font_family_escaped_and_used() {
        let g = generate(100, 100, "x", "#ccc", "#000", 12.0, "Georgia, serif").unwrap();
        assert!(g.svg.contains("font-family=\"Georgia, serif\""));
        let d = generate(100, 100, "x", "#ccc", "#000", 12.0, "").unwrap();
        assert!(d.svg.contains("font-family=\"sans-serif\""));
    }
}
