//! gizza-ai/code-screenshot core — pure, wasm-safe rendering of a
//! syntax-highlighted code snippet into a PNG (carbon-style window chrome).
//!
//! No wafer / wasm-bindgen / network deps. The chat skill block, the CLI, and
//! the unit tests all call [`render_png`]. Verified to compile + link on
//! wasm32-wasip1 (syntect uses the pure-Rust `fancy-regex` engine + bundled
//! binary-dump syntaxes/themes; `fontdue` is a pure-Rust TrueType rasterizer;
//! `png` is a pure-Rust encoder — no onig / freetype / system fonts).

use fontdue::Font;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SynStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

/// Embedded monospace font (DejaVu Sans Mono, Bitstream-Vera/Arev license —
/// freely redistributable). Bundled so rendering never touches the host
/// filesystem (wasm has none). See `core/src/assets/LICENSE-DejaVu.txt`.
const FONT_BYTES: &[u8] = include_bytes!("assets/DejaVuSansMono.ttf");

/// Rendering knobs. Chosen for a tasteful, minimal "carbon"-style card.
const FONT_PX: f32 = 26.0; // glyph cell height in pixels
const LINE_HEIGHT: u32 = 34; // baseline-to-baseline spacing
const PAD: u32 = 36; // padding between the code and the window edge
const MARGIN: u32 = 28; // outer padded background around the window card
const CHROME_BAR: u32 = 44; // height of the title-bar with traffic-light dots
const MAX_LINES: usize = 400; // guard against pathological inputs
const MAX_COLS: usize = 240; // truncate absurdly long lines

/// An error from [`render_png`]. The block maps this onto a `SkillError`.
#[derive(Debug, PartialEq, Eq)]
pub enum RenderError {
    /// `code` was empty (or only whitespace).
    EmptyCode,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::EmptyCode => write!(f, "code is empty"),
        }
    }
}

impl std::error::Error for RenderError {}

/// Resolve a user-supplied language hint to a syntect syntax. Tries, in order:
/// exact token (extension or name, case-insensitive), then a few friendly
/// aliases, finally falling back to plain text. Never fails — an unknown
/// language renders as plaintext (still a valid image).
fn resolve_syntax<'a>(ss: &'a SyntaxSet, language: Option<&str>) -> &'a SyntaxReference {
    let plain = ss.find_syntax_plain_text();
    let Some(lang) = language.map(|l| l.trim()).filter(|l| !l.is_empty()) else {
        return plain;
    };
    let lower = lang.to_ascii_lowercase();
    // Friendly aliases → a token syntect's defaults recognise by extension.
    let token = match lower.as_str() {
        "rust" | "rs" => "rs",
        "python" | "py" => "py",
        "javascript" | "js" => "js",
        "typescript" | "ts" => "ts",
        "shell" | "bash" | "sh" | "zsh" => "sh",
        "c++" | "cpp" | "cxx" => "cpp",
        "c#" | "csharp" | "cs" => "cs",
        "golang" | "go" => "go",
        "yml" | "yaml" => "yaml",
        "markdown" | "md" => "md",
        "html" | "htm" => "html",
        other => other,
    };
    ss.find_syntax_by_extension(token)
        .or_else(|| ss.find_syntax_by_token(token))
        .or_else(|| ss.find_syntax_by_name(lang))
        .unwrap_or(plain)
}

/// Resolve a theme name to a syntect theme, falling back to a sensible dark
/// theme (`base16-ocean.dark`) for unknown / empty names.
fn resolve_theme<'a>(ts: &'a ThemeSet, theme: Option<&str>) -> &'a Theme {
    const DEFAULT: &str = "base16-ocean.dark";
    let name = theme
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .unwrap_or(DEFAULT);
    ts.themes
        .get(name)
        .or_else(|| ts.themes.get(DEFAULT))
        .unwrap_or_else(|| ts.themes.values().next().expect("themeset is non-empty"))
}

/// A laid-out RGBA canvas backed by a flat `Vec<u8>` (4 bytes/pixel).
struct Canvas {
    width: u32,
    height: u32,
    px: Vec<u8>,
}

impl Canvas {
    fn new(width: u32, height: u32, bg: [u8; 3]) -> Self {
        let mut px = vec![0u8; (width * height * 4) as usize];
        for chunk in px.chunks_exact_mut(4) {
            chunk[0] = bg[0];
            chunk[1] = bg[1];
            chunk[2] = bg[2];
            chunk[3] = 255;
        }
        Canvas { width, height, px }
    }

    /// Fill an axis-aligned rectangle with an opaque colour (clipped to bounds).
    fn fill_rect(&mut self, x0: u32, y0: u32, w: u32, h: u32, color: [u8; 3]) {
        let x1 = (x0 + w).min(self.width);
        let y1 = (y0 + h).min(self.height);
        for y in y0..y1 {
            for x in x0..x1 {
                self.put(x, y, color, 255);
            }
        }
    }

    /// Alpha-blend `color` over the pixel at `(x, y)` with coverage `a` (0-255).
    fn put(&mut self, x: u32, y: u32, color: [u8; 3], a: u8) {
        if x >= self.width || y >= self.height {
            return;
        }
        let i = ((y * self.width + x) * 4) as usize;
        let af = a as f32 / 255.0;
        for (c, &src) in color.iter().enumerate() {
            let dst = self.px[i + c] as f32;
            self.px[i + c] = (src as f32 * af + dst * (1.0 - af))
                .round()
                .clamp(0.0, 255.0) as u8;
        }
        self.px[i + 3] = 255;
    }
}

/// Encode an RGBA [`Canvas`] to PNG bytes (pure-Rust `png` encoder).
fn encode_png(canvas: &Canvas) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, canvas.width, canvas.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().expect("png header");
        writer
            .write_image_data(&canvas.px)
            .expect("png image data");
    }
    out
}

/// Render `code` to a syntax-highlighted PNG (window chrome + padded
/// background). `language` is a syntect token / friendly alias (unknown →
/// plaintext); `theme` names a syntect theme (unknown → a dark default).
///
/// Returns the PNG bytes plus the rendered `(width, height)`. Errors only on
/// empty input.
pub fn render_png(
    code: &str,
    language: Option<&str>,
    theme: Option<&str>,
) -> Result<(Vec<u8>, u32, u32), RenderError> {
    if code.trim().is_empty() {
        return Err(RenderError::EmptyCode);
    }

    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let syntax = resolve_syntax(&ss, language);
    let theme = resolve_theme(&ts, theme);

    // Background / chrome colours derive from the theme so the card always
    // matches the highlighted text. The outer margin is a slightly darker shade.
    let code_bg = theme
        .settings
        .background
        .map(|c| [c.r, c.g, c.b])
        .unwrap_or([40, 44, 52]);
    let outer_bg = scale(code_bg, 0.55);
    let chrome_bg = scale(code_bg, 0.82);
    let default_fg = theme
        .settings
        .foreground
        .map(|c| [c.r, c.g, c.b])
        .unwrap_or([220, 223, 228]);

    let font = Font::from_bytes(FONT_BYTES, fontdue::FontSettings::default())
        .expect("embedded font parses");
    // Monospace advance: measure a representative glyph at the render size.
    let advance = font.metrics('M', FONT_PX).advance_width.ceil().max(1.0) as u32;
    let ascent = font_ascent(&font);

    // Highlight every line up front so we know the longest one (→ canvas width).
    let mut h = HighlightLines::new(syntax, theme);
    let mut lines: Vec<Vec<(SynStyle, String)>> = Vec::new();
    let mut max_cols = 1usize;
    for raw in LinesWithEndings::from(code).take(MAX_LINES) {
        let spans = h.highlight_line(raw, &ss).unwrap_or_default();
        let mut cols = 0usize;
        let mut out_spans = Vec::new();
        for (style, text) in spans {
            // Strip the trailing newline; expand tabs to 4 spaces for layout.
            let cleaned: String = text
                .trim_end_matches(['\n', '\r'])
                .replace('\t', "    ");
            cols += cleaned.chars().count();
            out_spans.push((style, cleaned));
        }
        max_cols = max_cols.max(cols.min(MAX_COLS));
        lines.push(out_spans);
    }

    let code_w = advance * max_cols as u32;
    let code_h = LINE_HEIGHT * lines.len() as u32;
    let card_w = code_w + 2 * PAD;
    let card_h = code_h + 2 * PAD + CHROME_BAR;
    let width = card_w + 2 * MARGIN;
    let height = card_h + 2 * MARGIN;

    let mut canvas = Canvas::new(width, height, outer_bg);
    // Window card.
    canvas.fill_rect(MARGIN, MARGIN, card_w, card_h, code_bg);
    // Title bar.
    canvas.fill_rect(MARGIN, MARGIN, card_w, CHROME_BAR, chrome_bg);
    // Traffic-light dots (red / amber / green).
    let dot_r = 7u32;
    let dot_y = MARGIN + CHROME_BAR / 2;
    let dots = [[255, 95, 86], [255, 189, 46], [39, 201, 63]];
    for (i, color) in dots.iter().enumerate() {
        let cx = MARGIN + PAD + dot_r + i as u32 * (dot_r * 2 + 10);
        fill_circle(&mut canvas, cx, dot_y, dot_r, *color);
    }

    // Draw the highlighted text below the chrome bar.
    let origin_x = MARGIN + PAD;
    let origin_y = MARGIN + CHROME_BAR + PAD;
    for (row, spans) in lines.iter().enumerate() {
        let baseline = origin_y + row as u32 * LINE_HEIGHT + ascent;
        let mut col = 0u32;
        for (style, text) in spans {
            let fg = style_fg(style, default_fg);
            for ch in text.chars() {
                if col as usize >= MAX_COLS {
                    break;
                }
                if !ch.is_whitespace() {
                    draw_glyph(&mut canvas, &font, ch, origin_x + col * advance, baseline, fg);
                }
                col += 1;
            }
        }
    }

    let png = encode_png(&canvas);
    Ok((png, width, height))
}

/// Foreground for a syntect style, falling back to the theme default when the
/// token carries no explicit colour.
fn style_fg(style: &SynStyle, default_fg: [u8; 3]) -> [u8; 3] {
    let c = style.foreground;
    // syntect uses alpha 0 to mean "inherit"; treat it as the default fg.
    if c.a == 0 {
        default_fg
    } else {
        [c.r, c.g, c.b]
    }
}

/// Rasterize one glyph with fontdue and alpha-composite it onto the canvas.
/// `baseline` is the y of the text baseline; fontdue gives us the bitmap top
/// via `ymin` relative to the baseline.
fn draw_glyph(canvas: &mut Canvas, font: &Font, ch: char, pen_x: u32, baseline: u32, fg: [u8; 3]) {
    let (metrics, bitmap) = font.rasterize(ch, FONT_PX);
    if metrics.width == 0 || metrics.height == 0 {
        return;
    }
    let left = pen_x as i64 + metrics.xmin as i64;
    // ymin is the offset of the bitmap's bottom edge from the baseline (negative
    // = below). Top of the bitmap = baseline - (height + ymin).
    let top = baseline as i64 - (metrics.height as i64 + metrics.ymin as i64);
    for gy in 0..metrics.height {
        for gx in 0..metrics.width {
            let coverage = bitmap[gy * metrics.width + gx];
            if coverage == 0 {
                continue;
            }
            let px = left + gx as i64;
            let py = top + gy as i64;
            if px < 0 || py < 0 {
                continue;
            }
            canvas.put(px as u32, py as u32, fg, coverage);
        }
    }
}

/// Filled circle via a simple radius test (used for the traffic-light dots).
fn fill_circle(canvas: &mut Canvas, cx: u32, cy: u32, r: u32, color: [u8; 3]) {
    let r2 = (r * r) as i64;
    for dy in -(r as i64)..=(r as i64) {
        for dx in -(r as i64)..=(r as i64) {
            if dx * dx + dy * dy <= r2 {
                let x = cx as i64 + dx;
                let y = cy as i64 + dy;
                if x >= 0 && y >= 0 {
                    canvas.put(x as u32, y as u32, color, 255);
                }
            }
        }
    }
}

/// The font's ascent in pixels at [`FONT_PX`] — used to place the first
/// baseline below the top padding. Derived from a tall glyph's metrics.
fn font_ascent(font: &Font) -> u32 {
    // Use the line metrics if available, else fall back to ~80% of the size.
    font.horizontal_line_metrics(FONT_PX)
        .map(|m| m.ascent.ceil() as u32)
        .unwrap_or((FONT_PX * 0.8) as u32)
}

/// Scale an RGB colour toward black by `factor` (0..=1).
fn scale(c: [u8; 3], factor: f32) -> [u8; 3] {
    [
        (c[0] as f32 * factor) as u8,
        (c[1] as f32 * factor) as u8,
        (c[2] as f32 * factor) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PNG files start with this 8-byte magic signature.
    const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'];

    #[test]
    fn renders_rust_to_valid_nonempty_png() {
        let (png, w, h) = render_png("fn main(){}", Some("rust"), None).unwrap();
        assert!(!png.is_empty(), "png should be non-empty");
        assert_eq!(&png[..8], &PNG_MAGIC, "png magic bytes");
        assert!(w > 0 && h > 0, "non-zero dimensions, got {w}x{h}");
    }

    #[test]
    fn unknown_language_falls_back_to_plaintext_and_still_renders() {
        let (png, w, h) =
            render_png("hello world\nsecond line", Some("not-a-real-lang"), None).unwrap();
        assert_eq!(&png[..8], &PNG_MAGIC);
        assert!(w > 0 && h > 0);
    }

    #[test]
    fn empty_code_is_an_error() {
        assert_eq!(render_png("", Some("rust"), None), Err(RenderError::EmptyCode));
        assert_eq!(render_png("   \n  \t", None, None), Err(RenderError::EmptyCode));
    }

    #[test]
    fn unknown_theme_falls_back_and_renders() {
        let (png, ..) = render_png("x = 1", Some("python"), Some("no-such-theme")).unwrap();
        assert_eq!(&png[..8], &PNG_MAGIC);
    }

    #[test]
    fn more_lines_make_a_taller_image() {
        let (_p1, _w1, h1) = render_png("a", Some("rust"), None).unwrap();
        let (_p2, _w2, h2) = render_png("a\nb\nc\nd", Some("rust"), None).unwrap();
        assert!(h2 > h1, "4 lines ({h2}) should be taller than 1 line ({h1})");
    }

    #[test]
    fn resolve_syntax_aliases_map_to_real_syntaxes() {
        let ss = SyntaxSet::load_defaults_newlines();
        // Known aliases resolve to a non-plaintext syntax.
        for lang in ["rust", "python", "js", "bash"] {
            let s = resolve_syntax(&ss, Some(lang));
            assert_ne!(s.name, "Plain Text", "{lang} should not be plaintext");
        }
        // Unknown → plaintext.
        assert_eq!(resolve_syntax(&ss, Some("zzz")).name, "Plain Text");
        // None → plaintext.
        assert_eq!(resolve_syntax(&ss, None).name, "Plain Text");
    }
}
