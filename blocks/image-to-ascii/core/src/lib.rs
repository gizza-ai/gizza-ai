//! gizza-ai/image-to-ascii core — convert an image into ASCII / ANSI character art.
//! No wafer/wasm-bindgen deps. Pure-Rust `image` crate decode + luminance ramp.
//!
//! The image is scaled to `cols` characters wide (rows derived from the aspect
//! ratio, with a vertical correction because text cells are ~2x taller than wide),
//! then each cell's brightness is mapped through a character ramp from dark→light.
//! Optionally each character is wrapped in a 24-bit ANSI color escape so the art
//! keeps the source colors in a truecolor terminal.

use image::{imageops::FilterType, GenericImageView};

/// Character ramp ordered dark → light (10 levels). The default ramp.
pub const RAMP_STANDARD: &str = "@%#*+=-:. ";
/// A finer 70-level ramp for more detail.
pub const RAMP_DETAILED: &str =
    "$@B%8&WM#*oahkbdpqwmZO0QLCJUYXzcvunxrjft/\\|()1{}[]?-_+~<>i!lI;:,\"^`'. ";
/// A minimal blocky ramp.
pub const RAMP_BLOCKS: &str = "█▓▒░ ";

/// Terminal text cells are taller than they are wide; this factor corrects the
/// vertical sampling so the art keeps the image's aspect ratio. ~0.5 = cells are
/// twice as tall as wide.
const CELL_ASPECT: f32 = 0.5;

/// Which built-in character ramp to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ramp {
    Standard,
    Detailed,
    Blocks,
}

impl Ramp {
    pub fn parse(s: &str) -> Result<Ramp, String> {
        match s {
            "standard" => Ok(Ramp::Standard),
            "detailed" => Ok(Ramp::Detailed),
            "blocks" => Ok(Ramp::Blocks),
            other => Err(format!(
                "unknown ramp \"{other}\" (expected standard, detailed, or blocks)"
            )),
        }
    }

    fn chars(self) -> Vec<char> {
        match self {
            Ramp::Standard => RAMP_STANDARD.chars().collect(),
            Ramp::Detailed => RAMP_DETAILED.chars().collect(),
            Ramp::Blocks => RAMP_BLOCKS.chars().collect(),
        }
    }
}

/// Options controlling the conversion.
#[derive(Debug, Clone)]
pub struct Options {
    /// Output width in characters (1..=400).
    pub cols: u32,
    /// Character ramp.
    pub ramp: Ramp,
    /// Custom character ramp ordered dark→light. When `Some` and non-empty it
    /// overrides `ramp`. Must contain at least 2 characters.
    pub charset: Option<String>,
    /// Emit 24-bit ANSI truecolor escape codes around each character.
    pub color: bool,
    /// Invert brightness (treat dark as light) — useful for light-on-dark terminals.
    pub invert: bool,
    /// Brightness adjustment in -1.0..=1.0 (added to normalized luminance). 0 = none.
    pub brightness: f32,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            cols: 80,
            ramp: Ramp::Standard,
            charset: None,
            color: false,
            invert: false,
            brightness: 0.0,
        }
    }
}

/// Result of a conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Art {
    /// The rendered art (newline-separated rows; ANSI escapes if `color`).
    pub text: String,
    /// Output dimensions in characters.
    pub cols: u32,
    pub rows: u32,
    /// Source image dimensions in pixels.
    pub src_width: u32,
    pub src_height: u32,
}

const ANSI_RESET: &str = "\x1b[0m";

/// Convert image `bytes` to character art per `opts`.
pub fn convert(bytes: &[u8], opts: &Options) -> Result<Art, String> {
    if opts.cols == 0 || opts.cols > 400 {
        return Err(format!("cols must be between 1 and 400 (got {})", opts.cols));
    }
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (src_width, src_height) = img.dimensions();
    if src_width == 0 || src_height == 0 {
        return Err("image has zero dimensions".into());
    }

    let cols = opts.cols;
    // Rows preserve aspect ratio, corrected for non-square text cells.
    let rows = (((cols as f32) * (src_height as f32) / (src_width as f32)) * CELL_ASPECT)
        .round()
        .max(1.0) as u32;

    let small = img
        .resize_exact(cols, rows, FilterType::Triangle)
        .to_rgba8();

    let ramp: Vec<char> = match &opts.charset {
        Some(s) if !s.is_empty() => {
            let v: Vec<char> = s.chars().collect();
            if v.len() < 2 {
                return Err("charset must contain at least 2 characters (dark→light)".into());
            }
            v
        }
        _ => opts.ramp.chars(),
    };
    let ramp_last = (ramp.len() - 1) as f32;
    let brightness = opts.brightness.clamp(-1.0, 1.0);

    let mut text = String::with_capacity((cols * rows + rows) as usize);
    for y in 0..rows {
        if opts.color {
            for x in 0..cols {
                let px = small.get_pixel(x, y).0;
                let (r, g, b) = (px[0], px[1], px[2]);
                let mut lum = (luminance(r, g, b) + brightness).clamp(0.0, 1.0);
                if opts.invert {
                    lum = 1.0 - lum;
                }
                let idx = (lum * ramp_last).round() as usize;
                let ch = ramp[idx.min(ramp.len() - 1)];
                text.push_str(&format!("\x1b[38;2;{r};{g};{b}m{ch}"));
            }
            // Reset color at the end of each row so it doesn't bleed.
            text.push_str(ANSI_RESET);
        } else {
            for x in 0..cols {
                let px = small.get_pixel(x, y).0;
                let mut lum = (luminance(px[0], px[1], px[2]) + brightness).clamp(0.0, 1.0);
                if opts.invert {
                    lum = 1.0 - lum;
                }
                let idx = (lum * ramp_last).round() as usize;
                text.push(ramp[idx.min(ramp.len() - 1)]);
            }
        }
        if y + 1 < rows {
            text.push('\n');
        }
    }

    Ok(Art {
        text,
        cols,
        rows,
        src_width,
        src_height,
    })
}

/// Perceptual luminance in 0.0..=1.0 (Rec. 601 weights).
fn luminance(r: u8, g: u8, b: u8) -> f32 {
    (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn png_solid(r: u8, g: u8, b: u8, w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba([r, g, b, 255]));
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn black_image_is_darkest_char() {
        let png = png_solid(0, 0, 0, 20, 20);
        let art = convert(&png, &Options { cols: 10, ..Default::default() }).unwrap();
        // Standard ramp dark char is '@'.
        assert!(art.text.chars().filter(|c| *c == '@').count() > 0);
        assert!(!art.text.contains(' '));
        assert_eq!(art.cols, 10);
    }

    #[test]
    fn white_image_is_lightest_char() {
        let png = png_solid(255, 255, 255, 20, 20);
        let art = convert(&png, &Options { cols: 10, ..Default::default() }).unwrap();
        // Standard ramp light char is ' ' (space).
        assert!(art.text.lines().all(|l| l.chars().all(|c| c == ' ')));
    }

    #[test]
    fn invert_flips_mapping() {
        let png = png_solid(255, 255, 255, 20, 20);
        let normal = convert(&png, &Options { cols: 8, ..Default::default() }).unwrap();
        let inverted = convert(
            &png,
            &Options { cols: 8, invert: true, ..Default::default() },
        )
        .unwrap();
        assert_ne!(normal.text, inverted.text);
        assert!(inverted.text.contains('@'));
    }

    #[test]
    fn color_emits_ansi_escapes() {
        let png = png_solid(255, 0, 0, 10, 10);
        let art = convert(
            &png,
            &Options { cols: 6, color: true, ..Default::default() },
        )
        .unwrap();
        assert!(art.text.contains("\x1b[38;2;255;0;0m"));
        assert!(art.text.contains("\x1b[0m"));
    }

    #[test]
    fn rows_preserve_aspect_with_cell_correction() {
        // A wide image: 40x20 px, cols=20 → rows ≈ 20 * (20/40) * 0.5 = 5.
        let png = png_solid(120, 120, 120, 40, 20);
        let art = convert(&png, &Options { cols: 20, ..Default::default() }).unwrap();
        assert_eq!(art.cols, 20);
        assert_eq!(art.rows, 5);
        assert_eq!(art.text.lines().count(), 5);
    }

    #[test]
    fn ramp_parse() {
        assert_eq!(Ramp::parse("standard").unwrap(), Ramp::Standard);
        assert_eq!(Ramp::parse("detailed").unwrap(), Ramp::Detailed);
        assert_eq!(Ramp::parse("blocks").unwrap(), Ramp::Blocks);
        assert!(Ramp::parse("nope").is_err());
    }

    #[test]
    fn cols_out_of_range_errors() {
        let png = png_solid(0, 0, 0, 5, 5);
        assert!(convert(&png, &Options { cols: 0, ..Default::default() }).is_err());
        assert!(convert(&png, &Options { cols: 401, ..Default::default() }).is_err());
    }

    #[test]
    fn custom_charset_overrides_ramp() {
        let png = png_solid(0, 0, 0, 10, 10);
        // dark→light "AB": black should map to 'A'.
        let art = convert(
            &png,
            &Options { cols: 6, charset: Some("AB".into()), ..Default::default() },
        )
        .unwrap();
        assert!(art.text.lines().all(|l| l.chars().all(|c| c == 'A')));
    }

    #[test]
    fn charset_too_short_errors() {
        let png = png_solid(0, 0, 0, 5, 5);
        assert!(convert(
            &png,
            &Options { cols: 4, charset: Some("X".into()), ..Default::default() }
        )
        .is_err());
    }

    #[test]
    fn brightness_pushes_toward_light() {
        // A mid-gray image; full positive brightness should reach the lightest char (space).
        let png = png_solid(128, 128, 128, 10, 10);
        let bright = convert(
            &png,
            &Options { cols: 6, brightness: 1.0, ..Default::default() },
        )
        .unwrap();
        assert!(bright.text.lines().all(|l| l.chars().all(|c| c == ' ')));
        // Full negative brightness should reach the darkest char ('@').
        let dark = convert(
            &png,
            &Options { cols: 6, brightness: -1.0, ..Default::default() },
        )
        .unwrap();
        assert!(dark.text.contains('@'));
    }

    #[test]
    fn decode_error() {
        assert!(convert(b"not an image", &Options::default()).is_err());
    }
}
