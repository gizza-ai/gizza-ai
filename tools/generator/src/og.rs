//! Build-time Open Graph card renderer — one 1200×630 `og.png` per tool page
//! (plus one for the `/tools/` landing), so shares on social/chat surfaces get
//! a branded, per-tool preview instead of the generic site icon.
//!
//! Cards are composed as SVG and rasterized with resvg using the DejaVu fonts
//! bundled under `assets/fonts/` (never system fonts), so output is identical
//! across dev machines and CI.

use crate::meta::ToolMeta;
use crate::site::SiteConfig;
use resvg::{tiny_skia, usvg};

pub const WIDTH: u32 = 1200;
pub const HEIGHT: u32 = 630;

const FONT_BOLD: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf");
const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");

/// Horizontal padding and usable text width.
const PAD: f32 = 80.0;
const TEXT_WIDTH: f32 = WIDTH as f32 - 2.0 * PAD;
/// Average glyph advance as a fraction of font-size for DejaVu Sans — used
/// for conservative line wrapping (slightly wide so lines never overflow).
const AVG_ADVANCE: f32 = 0.62;

/// Holds the parsed font database; build once, render many.
pub struct OgRenderer {
    opt: usvg::Options<'static>,
}

impl OgRenderer {
    pub fn new() -> Self {
        let mut opt = usvg::Options::default();
        let db = opt.fontdb_mut();
        db.load_font_data(FONT_BOLD.to_vec());
        db.load_font_data(FONT_REGULAR.to_vec());
        Self { opt }
    }

    /// Render the card for one tool page.
    pub fn tool_card(&self, cfg: &SiteConfig, meta: &ToolMeta) -> Result<Vec<u8>, String> {
        let tagline = if meta.hero_subtitle.is_empty() {
            &meta.description
        } else {
            &meta.hero_subtitle
        };
        self.render(&card_svg(
            &meta.h1,
            tagline,
            &cfg.og_label(&format!("tools/{}", meta.slug)),
            &cfg.brand_name,
        ))
    }

    /// Render the card for a conversion pair page
    /// (`/tools/<parent>/<src>-to-<tgt>/`).
    pub fn pair_card(
        &self,
        cfg: &SiteConfig,
        title: &str,
        tagline: &str,
        footer: &str,
    ) -> Result<Vec<u8>, String> {
        self.render(&card_svg(title, tagline, footer, &cfg.brand_name))
    }

    /// Render the card for a category hub page (`/tools/<category>/`).
    pub fn hub_card(
        &self,
        cfg: &SiteConfig,
        category: &crate::categories::Category,
    ) -> Result<Vec<u8>, String> {
        self.render(&card_svg(
            category.title,
            category.blurb,
            &cfg.og_label(&format!("tools/{}", category.slug)),
            &cfg.brand_name,
        ))
    }

    /// Render the card for the `/tools/` landing page.
    pub fn index_card(&self, cfg: &SiteConfig, tool_count: usize) -> Result<Vec<u8>, String> {
        self.render(&card_svg(
            "All tools",
            &format!(
                "{tool_count} free, private, browser-local tools — no sign-up, works offline."
            ),
            &cfg.og_label("tools"),
            &cfg.brand_name,
        ))
    }

    fn render(&self, svg: &str) -> Result<Vec<u8>, String> {
        let tree =
            usvg::Tree::from_str(svg, &self.opt).map_err(|e| format!("og card svg: {e}"))?;
        let mut pixmap = tiny_skia::Pixmap::new(WIDTH, HEIGHT)
            .ok_or_else(|| "og card pixmap alloc".to_string())?;
        resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
        pixmap.encode_png().map_err(|e| format!("og card png: {e}"))
    }
}

/// Compose the card SVG. Palette mirrors the site (`--tool-ink` slate,
/// `--tool-accent` indigo). Kept `pub(crate)` so tests can assert on layout
/// without rasterizing. `brand_name` labels the top-left brand row; empty
/// omits the label text (generic/unbranded cards carry no brand name).
pub(crate) fn card_svg(title: &str, tagline: &str, footer: &str, brand_name: &str) -> String {
    // Title: bigger for short names, wrapped up to 3 lines for long ones.
    let title_size: f32 = match title.chars().count() {
        0..=22 => 88.0,
        23..=48 => 68.0,
        _ => 56.0,
    };
    let title_lines = wrap(title, chars_per_line(title_size), 3);
    let tagline_size: f32 = 32.0;
    // A 3-line title leaves room for only one tagline line above the footer.
    let tagline_max_lines = if title_lines.len() >= 3 { 1 } else { 2 };
    let tagline_lines = wrap(tagline, chars_per_line(tagline_size), tagline_max_lines);

    let mut text_elems = String::new();
    // Brand row. The brand-name text is omitted for a generic/unbranded card;
    // the accent dot and right-aligned tagline are universal either way.
    let brand_text = if brand_name.is_empty() {
        String::new()
    } else {
        format!(
            r##"<text x="{bx}" y="122" font-family="DejaVu Sans" font-weight="bold" font-size="40" fill="#e2e8f0">{}</text>
"##,
            escape_xml(brand_name),
            bx = PAD + 44.0,
        )
    };
    text_elems.push_str(&format!(
        r##"<circle cx="{cx}" cy="108" r="14" fill="#4f46e5"/>
{brand_text}<text x="{rx}" y="120" text-anchor="end" font-family="DejaVu Sans" font-size="26" fill="#64748b">Free · Private · In your browser</text>
"##,
        cx = PAD + 14.0,
        rx = WIDTH as f32 - PAD,
    ));
    // Title block.
    let mut y = 268.0;
    for line in &title_lines {
        text_elems.push_str(&format!(
            r##"<text x="{PAD}" y="{y}" font-family="DejaVu Sans" font-weight="bold" font-size="{title_size}" fill="#f8fafc">{}</text>
"##,
            escape_xml(line),
        ));
        y += title_size * 1.18;
    }
    // Tagline under the title.
    y += 18.0;
    for line in &tagline_lines {
        text_elems.push_str(&format!(
            r##"<text x="{PAD}" y="{y}" font-family="DejaVu Sans" font-size="{tagline_size}" fill="#94a3b8">{}</text>
"##,
            escape_xml(line),
        ));
        y += tagline_size * 1.45;
    }
    // Footer URL.
    text_elems.push_str(&format!(
        r##"<text x="{PAD}" y="{fy}" font-family="DejaVu Sans" font-weight="bold" font-size="30" fill="#818cf8">{}</text>
"##,
        escape_xml(footer),
        fy = HEIGHT as f32 - 48.0,
    ));

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}">
<defs>
<linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
<stop offset="0" stop-color="#0f172a"/>
<stop offset="1" stop-color="#1e1b4b"/>
</linearGradient>
</defs>
<rect width="{WIDTH}" height="{HEIGHT}" fill="url(#bg)"/>
<rect width="{WIDTH}" height="10" fill="#4f46e5"/>
{text_elems}</svg>
"##
    )
}

fn chars_per_line(font_size: f32) -> usize {
    (TEXT_WIDTH / (font_size * AVG_ADVANCE)) as usize
}

/// Greedy word wrap to at most `max_lines`; the last line gets an ellipsis
/// when text is cut. A single overlong word is hard-truncated rather than
/// overflowing the card.
fn wrap(text: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut truncated = false;
    for word in text.split_whitespace() {
        let word: String = if word.chars().count() > max_chars {
            truncated = true;
            word.chars().take(max_chars.saturating_sub(1)).collect()
        } else {
            word.to_string()
        };
        let candidate_len = current.chars().count() + !current.is_empty() as usize + word.chars().count();
        if candidate_len > max_chars && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            if lines.len() == max_lines {
                truncated = true;
                break;
            }
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(&word);
    }
    if !current.is_empty() && lines.len() < max_lines {
        lines.push(current);
    }
    if truncated {
        if let Some(last) = lines.last_mut() {
            *last = format!("{}…", last.trim_end_matches(['.', ',', ' ']));
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_and_ellipsizes() {
        assert_eq!(wrap("one two three", 20, 3), vec!["one two three"]);
        assert_eq!(wrap("aaaa bbbb cccc", 9, 3), vec!["aaaa bbbb", "cccc"]);
        let cut = wrap("aaaa bbbb cccc dddd eeee", 9, 2);
        assert_eq!(cut.len(), 2);
        assert!(cut[1].ends_with('…'), "cut text ends with ellipsis: {cut:?}");
        // one overlong word is truncated, not overflowed
        let long = wrap("abcdefghijklmnop", 8, 2);
        assert!(long[0].chars().count() <= 8);
    }

    #[test]
    fn svg_escapes_and_lays_out() {
        let svg = card_svg("A & B <fast>", "Convert \"x\" locally.", "gizza.ai/tools/x", "gizza.ai");
        assert!(svg.contains("A &amp; B &lt;fast&gt;"));
        assert!(svg.contains("&quot;x&quot;"));
        assert!(svg.contains("gizza.ai/tools/x"));
        assert!(svg.contains(r#"width="1200""#));
    }

    #[test]
    fn brand_text_omitted_when_brand_name_empty() {
        let branded = card_svg("Title", "Tagline", "tools/x", "gizza.ai");
        assert!(branded.contains(">gizza.ai</text>"), "brand name rendered when set");
        let generic = card_svg("Title", "Tagline", "tools/x", "");
        assert!(!generic.contains("gizza.ai"), "no brand text when brand_name is empty");
        // the accent dot and right-aligned tagline stay either way
        assert!(generic.contains("<circle"), "accent dot still rendered");
        assert!(generic.contains("Free · Private · In your browser"));
    }

    #[test]
    fn renders_a_valid_png() {
        let r = OgRenderer::new();
        let cfg = SiteConfig { brand_name: "gizza.ai".into(), ..Default::default() };
        let meta = ToolMeta::from_toml(
            r#"
slug          = "calculator"
title         = "Free Online Calculator"
description   = "Evaluate expressions instantly."
h1            = "Free Online Calculator"
hero_subtitle = "Type a math expression — result updates instantly."
wasm          = "w"
export        = "evaluate"
output_label  = "Result"
format        = "number"
"#,
        )
        .unwrap();
        let png = r.tool_card(&cfg, &meta).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "PNG magic");
        assert!(png.len() > 5_000, "non-trivial image ({} bytes)", png.len());
        let idx = r.index_card(&cfg, 324).unwrap();
        assert_eq!(&idx[..8], b"\x89PNG\r\n\x1a\n");

        // a fully generic (default) config renders too, with no brand label.
        let generic = r.index_card(&SiteConfig::default(), 324).unwrap();
        assert_eq!(&generic[..8], b"\x89PNG\r\n\x1a\n");
    }
}
