//! psd-to-png core — pure compute, shared by the chat skill block.
//! No wafer/wasm-bindgen deps.
//!
//! Decode an Adobe Photoshop `.psd` document and render its **flattened
//! composite** (the merged image Photoshop stores in every save) to a viewable
//! PNG or JPEG via the `psd` + `image` crates.
//!
//! Scope (honest baseline — see the competitor-analysis doc):
//!
//!   * We emit the FLATTENED composite only — one image, the whole canvas as it
//!     looks with all visible layers merged. Per-layer extraction to separate
//!     files is intentionally NOT offered (that is a multi-output job; a single
//!     chat/CLI call returns one image).
//!   * PNG preserves the document's alpha channel; JPEG has no alpha, so the
//!     image is composited over a solid `background` colour (default white).
//!   * Colour data is read from the PSD's pre-composited image-data section, so
//!     what you get matches Photoshop's own preview. RGB/RGBA documents render
//!     faithfully; other colour modes fall back to that stored composite.

use std::io::Cursor;

use image::{ImageFormat, RgbImage, RgbaImage};
use psd::Psd;

/// Output container the caller wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Png,
    Jpeg,
}

impl OutputFormat {
    /// Parse the `format` param (default png). Errors on anything else so the
    /// LLM/CLI gets a precise message instead of a silent fallback.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "png" => Ok(OutputFormat::Png),
            "jpeg" | "jpg" => Ok(OutputFormat::Jpeg),
            other => Err(format!(
                "unsupported format {other:?} — use \"png\" or \"jpeg\""
            )),
        }
    }
    pub fn mime(self) -> &'static str {
        match self {
            OutputFormat::Png => "image/png",
            OutputFormat::Jpeg => "image/jpeg",
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Jpeg => "jpg",
        }
    }
}

/// Rendering options (all optional at the surface; defaults applied by caller).
#[derive(Debug, Clone)]
pub struct Options {
    pub format: OutputFormat,
    /// JPEG quality 1..=100 (ignored for PNG).
    pub quality: u8,
    /// RGB fill used when flattening onto a non-alpha format (JPEG).
    pub background: [u8; 3],
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: OutputFormat::Png,
            quality: 90,
            background: [255, 255, 255],
        }
    }
}

/// Parse a `#rgb` / `#rrggbb` (or bare, no-`#`) hex colour into an RGB triple.
/// Used for the JPEG `background` fill. Errors precisely on bad input.
pub fn parse_hex_color(s: &str) -> Result<[u8; 3], String> {
    let t = s.trim();
    let h = t.strip_prefix('#').unwrap_or(t);
    let expand = |c: u8| (c << 4) | c;
    let byte = |a: char, b: char| -> Result<u8, String> {
        let hi = a
            .to_digit(16)
            .ok_or_else(|| format!("invalid hex colour {s:?} — expected e.g. \"#ffffff\""))?;
        let lo = b
            .to_digit(16)
            .ok_or_else(|| format!("invalid hex colour {s:?} — expected e.g. \"#ffffff\""))?;
        Ok(((hi << 4) | lo) as u8)
    };
    let ch: Vec<char> = h.chars().collect();
    match ch.len() {
        3 => {
            let nib = |c: char| -> Result<u8, String> {
                c.to_digit(16)
                    .map(|d| expand(d as u8))
                    .ok_or_else(|| format!("invalid hex colour {s:?} — expected e.g. \"#fff\""))
            };
            Ok([nib(ch[0])?, nib(ch[1])?, nib(ch[2])?])
        }
        6 => Ok([
            byte(ch[0], ch[1])?,
            byte(ch[2], ch[3])?,
            byte(ch[4], ch[5])?,
        ]),
        _ => Err(format!(
            "invalid hex colour {s:?} — expected 3 or 6 hex digits, e.g. \"#ffffff\""
        )),
    }
}

/// Dimensions of a PSD without a full render — cheap header read for callers
/// that want to report size in a description line.
pub fn dimensions(bytes: &[u8]) -> Result<(u32, u32), String> {
    let psd = open(bytes)?;
    Ok((psd.width(), psd.height()))
}

fn open(bytes: &[u8]) -> Result<Psd, String> {
    if bytes.len() < 4 || &bytes[0..4] != b"8BPS" {
        return Err(
            "not a Photoshop document — the file does not start with the PSD \"8BPS\" signature"
                .to_string(),
        );
    }
    Psd::from_bytes(bytes).map_err(|e| format!("failed to parse PSD: {e}"))
}

/// Decode a PSD and render its flattened composite to the requested format.
/// Returns the encoded image bytes.
pub fn render(bytes: &[u8], opts: &Options) -> Result<Vec<u8>, String> {
    let psd = open(bytes)?;
    let (w, h) = (psd.width(), psd.height());
    if w == 0 || h == 0 {
        return Err(format!("PSD has an empty canvas ({w}x{h})"));
    }

    // The PSD's stored merged-image section: the flattened composite Photoshop
    // writes on every save, as tightly-packed RGBA (w*h*4).
    let rgba = psd.rgba();
    let expected = (w as usize)
        .checked_mul(h as usize)
        .and_then(|p| p.checked_mul(4))
        .ok_or_else(|| "PSD dimensions overflow".to_string())?;
    if rgba.len() != expected {
        return Err(format!(
            "PSD composite data is {} bytes but {w}x{h} needs {expected} — the file may lack a \
             flattened preview or use an unsupported colour mode",
            rgba.len()
        ));
    }

    let mut out = Vec::new();
    match opts.format {
        OutputFormat::Png => {
            let img = RgbaImage::from_raw(w, h, rgba)
                .ok_or_else(|| "failed to build RGBA image buffer".to_string())?;
            img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
                .map_err(|e| format!("PNG encode failed: {e}"))?;
        }
        OutputFormat::Jpeg => {
            // JPEG has no alpha: composite each pixel over the background colour.
            let [br, bg, bb] = opts.background;
            let mut rgb = Vec::with_capacity((w as usize) * (h as usize) * 3);
            for px in rgba.chunks_exact(4) {
                let a = px[3] as u32;
                let ia = 255 - a;
                let blend = |fg: u8, bgc: u8| ((fg as u32 * a + bgc as u32 * ia) / 255) as u8;
                rgb.push(blend(px[0], br));
                rgb.push(blend(px[1], bg));
                rgb.push(blend(px[2], bb));
            }
            let img = RgbImage::from_raw(w, h, rgb)
                .ok_or_else(|| "failed to build RGB image buffer".to_string())?;
            let mut enc =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, opts.quality.max(1));
            enc.encode_image(&img)
                .map_err(|e| format!("JPEG encode failed: {e}"))?;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal, valid RGB PSD in memory (raw/uncompressed image data)
    /// so tests don't depend on binary fixtures. `8BPS`, version 1, 8-bit,
    /// 3 planar channels (R,G,B); the crate reconstructs RGBA with alpha=255.
    fn tiny_psd(w: u16, h: u16, rgb: [u8; 3]) -> Vec<u8> {
        let mut v = Vec::new();
        // File header
        v.extend_from_slice(b"8BPS"); // signature
        v.extend_from_slice(&1u16.to_be_bytes()); // version = 1 (PSD)
        v.extend_from_slice(&[0u8; 6]); // reserved
        v.extend_from_slice(&3u16.to_be_bytes()); // channels = 3 (RGB)
        v.extend_from_slice(&(h as u32).to_be_bytes()); // rows
        v.extend_from_slice(&(w as u32).to_be_bytes()); // cols
        v.extend_from_slice(&8u16.to_be_bytes()); // depth = 8
        v.extend_from_slice(&3u16.to_be_bytes()); // color mode = 3 (RGB)
        // Color mode data (empty)
        v.extend_from_slice(&0u32.to_be_bytes());
        // Image resources (empty)
        v.extend_from_slice(&0u32.to_be_bytes());
        // Layer and mask info (empty)
        v.extend_from_slice(&0u32.to_be_bytes());
        // Image data: compression = 0 (raw), then planar R, G, B
        v.extend_from_slice(&0u16.to_be_bytes());
        let n = (w as usize) * (h as usize);
        for &c in &rgb {
            v.extend(std::iter::repeat(c).take(n));
        }
        v
    }

    #[test]
    fn png_happy_path_dimensions() {
        let psd = tiny_psd(4, 3, [10, 20, 30]);
        let (w, h) = dimensions(&psd).unwrap();
        assert_eq!((w, h), (4, 3));
        let out = render(&psd, &Options::default()).unwrap();
        // PNG magic
        assert_eq!(&out[0..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        let decoded = image::load_from_memory(&out).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (4, 3));
        assert_eq!(decoded.get_pixel(0, 0).0, [10, 20, 30, 255]);
    }

    #[test]
    fn jpeg_happy_path_and_format_parse() {
        let psd = tiny_psd(8, 8, [200, 100, 50]);
        let opts = Options {
            format: OutputFormat::Jpeg,
            quality: 92,
            background: [255, 255, 255],
        };
        let out = render(&psd, &opts).unwrap();
        // JPEG SOI marker
        assert_eq!(&out[0..2], &[0xFF, 0xD8]);
        let decoded = image::load_from_memory(&out).unwrap();
        assert_eq!(decoded.width(), 8);
        assert_eq!(OutputFormat::parse("jpg").unwrap(), OutputFormat::Jpeg);
        assert_eq!(OutputFormat::parse("PNG").unwrap(), OutputFormat::Png);
    }

    #[test]
    fn not_a_psd_errors() {
        let err = render(b"not a psd file at all", &Options::default()).unwrap_err();
        assert!(err.contains("8BPS"), "got: {err}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(render(&[], &Options::default()).is_err());
    }

    #[test]
    fn bad_format_errors() {
        let err = OutputFormat::parse("tiff").unwrap_err();
        assert!(err.contains("tiff"));
    }

    #[test]
    fn hex_color_parsing() {
        assert_eq!(parse_hex_color("#ffffff").unwrap(), [255, 255, 255]);
        assert_eq!(parse_hex_color("000000").unwrap(), [0, 0, 0]);
        assert_eq!(parse_hex_color("#f00").unwrap(), [255, 0, 0]);
        assert_eq!(parse_hex_color("#0a0").unwrap(), [0, 170, 0]);
        assert!(parse_hex_color("#gg0000").is_err());
        assert!(parse_hex_color("#12").is_err());
    }
}
