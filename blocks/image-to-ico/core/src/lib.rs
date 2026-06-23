//! image-to-ico core — turn one source raster image into a single
//! multi-resolution Windows `.ico` favicon. Pure Rust (`image` crate only),
//! no I/O / no ffmpeg, so it instantiates in the wafer chat runtime and the CLI.
//!
//! An `.ico` file embeds several square bitmaps at different resolutions; the OS
//! / browser picks the best-fitting one. We render the source into each
//! requested square size (PNG-compressed frames, as modern `.ico` allows) and
//! pack them into one ICO container.

use std::io::Cursor;

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, Rgba, RgbaImage};

/// Default resolutions embedded in the ICO (px). Covers the classic Windows /
/// browser favicon set; 256 is the largest a single ICO frame supports.
pub const DEFAULT_SIZES: &[u32] = &[16, 32, 48, 64, 128, 256];

/// Strategy for fitting a non-square source into a square icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Scale to fit entirely inside the square, padding the remainder with the
    /// background color (keeps the whole image, may add margins).
    Contain,
    /// Scale to fill the square then center-crop the overflow (fills the icon,
    /// may trim edges).
    Cover,
}

/// Options controlling how the source image is fit into each square frame.
#[derive(Debug, Clone)]
pub struct Options {
    /// How to fit a non-square source into a square frame.
    pub fit: Fit,
    /// Background fill (RGBA) used behind transparent / letterboxed areas when
    /// `fit == Contain`. Ignored for `Cover`.
    pub background: Rgba<u8>,
    /// Square sizes (px) to embed. Empty → `DEFAULT_SIZES`. Values are clamped
    /// to 1..=256 (ICO max), then sorted + de-duplicated.
    pub sizes: Vec<u32>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            fit: Fit::Contain,
            background: Rgba([0, 0, 0, 0]),
            sizes: DEFAULT_SIZES.to_vec(),
        }
    }
}

/// Parse the fit mode from a string (case-insensitive).
pub fn parse_fit(s: &str) -> Result<Fit, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "contain" | "fit" | "pad" => Ok(Fit::Contain),
        "cover" | "crop" | "fill" => Ok(Fit::Cover),
        other => Err(format!(
            "unknown fit '{other}' (expected 'contain' or 'cover')"
        )),
    }
}

/// Parse a `#rgb`, `#rrggbb`, or `#rrggbbaa` color into RGBA (the `#` is optional).
pub fn parse_color(s: &str) -> Result<Rgba<u8>, String> {
    let h = s.trim().trim_start_matches('#');
    let hx = |a: &str| u8::from_str_radix(a, 16).map_err(|_| format!("bad color '{s}'"));
    let v = match h.len() {
        3 => {
            let r = hx(&h[0..1])?;
            let g = hx(&h[1..2])?;
            let b = hx(&h[2..3])?;
            [r * 17, g * 17, b * 17, 255]
        }
        6 => [hx(&h[0..2])?, hx(&h[2..4])?, hx(&h[4..6])?, 255],
        8 => [hx(&h[0..2])?, hx(&h[2..4])?, hx(&h[4..6])?, hx(&h[6..8])?],
        _ => return Err(format!("bad color '{s}' (use #rgb, #rrggbb, or #rrggbbaa)")),
    };
    Ok(Rgba(v))
}

/// Parse a comma-separated size list (e.g. `"16,32,48"`) into a `Vec<u32>`.
pub fn parse_sizes(s: &str) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    for tok in s.split(',') {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        let n: u32 = t.parse().map_err(|_| format!("invalid size '{t}'"))?;
        out.push(n);
    }
    if out.is_empty() {
        return Err("provide at least one size".into());
    }
    Ok(out)
}

/// Render the source image into a square `size`×`size` RGBA canvas per the fit mode.
fn render_square(src: &DynamicImage, size: u32, opts: &Options) -> RgbaImage {
    match opts.fit {
        Fit::Cover => src.resize_to_fill(size, size, FilterType::Lanczos3).to_rgba8(),
        Fit::Contain => {
            let fitted = src.resize(size, size, FilterType::Lanczos3).to_rgba8();
            let mut canvas = RgbaImage::from_pixel(size, size, opts.background);
            let ox = (size - fitted.width()) / 2;
            let oy = (size - fitted.height()) / 2;
            image::imageops::overlay(&mut canvas, &fitted, ox as i64, oy as i64);
            canvas
        }
    }
}

/// Normalize the requested size list: clamp to 1..=256, sort, de-dup. Returns an
/// error if nothing remains.
fn normalize_sizes(sizes: &[u32]) -> Result<Vec<u32>, String> {
    let mut v: Vec<u32> = if sizes.is_empty() {
        DEFAULT_SIZES.to_vec()
    } else {
        sizes.to_vec()
    };
    v.retain(|s| (1..=256).contains(s));
    v.sort_unstable();
    v.dedup();
    if v.is_empty() {
        return Err("no valid icon sizes (1..=256)".into());
    }
    Ok(v)
}

/// End-to-end: source image bytes → multi-resolution `.ico` bytes.
pub fn to_ico(src_bytes: &[u8], opts: &Options) -> Result<Vec<u8>, String> {
    let src = image::load_from_memory(src_bytes).map_err(|e| format!("decode image: {e}"))?;
    let sizes = normalize_sizes(&opts.sizes)?;

    let mut frames: Vec<IcoFrame> = Vec::with_capacity(sizes.len());
    for &s in &sizes {
        let img = render_square(&src, s, opts);
        let frame = IcoFrame::as_png(
            img.as_raw(),
            img.width(),
            img.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("ico frame {s}px: {e}"))?;
        frames.push(frame);
    }

    let mut out = Vec::new();
    IcoEncoder::new(Cursor::new(&mut out))
        .encode_images(&frames)
        .map_err(|e| format!("ico encode: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::png::PngEncoder;
    use image::ImageEncoder;

    fn sample_png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba([10, 120, 220, 255]));
        let mut out = Vec::new();
        PngEncoder::new(&mut out)
            .write_image(img.as_raw(), w, h, ExtendedColorType::Rgba8)
            .unwrap();
        out
    }

    #[test]
    fn builds_multi_resolution_ico() {
        let src = sample_png(300, 300);
        let opts = Options {
            sizes: vec![16, 32, 48],
            ..Default::default()
        };
        let ico = to_ico(&src, &opts).unwrap();
        // ICO magic: 00 00 (reserved) 01 00 (type=icon).
        assert_eq!(&ico[0..4], &[0, 0, 1, 0]);
        // Image count (little-endian u16) at bytes 4..6 must equal 3 frames.
        let count = u16::from_le_bytes([ico[4], ico[5]]);
        assert_eq!(count, 3);
        // Re-decode round-trips through the `image` ICO reader.
        let decoded = image::load_from_memory(&ico).unwrap();
        assert!(decoded.width() >= 16);
    }

    #[test]
    fn default_sizes_used_when_empty() {
        let src = sample_png(64, 64);
        let opts = Options {
            sizes: vec![],
            ..Default::default()
        };
        let ico = to_ico(&src, &opts).unwrap();
        let count = u16::from_le_bytes([ico[4], ico[5]]);
        assert_eq!(count as usize, DEFAULT_SIZES.len());
    }

    #[test]
    fn sizes_clamped_sorted_deduped() {
        // 999 is over the 256 cap (dropped); duplicates collapse; order normalizes.
        let src = sample_png(64, 64);
        let opts = Options {
            sizes: vec![32, 16, 32, 999, 48],
            ..Default::default()
        };
        let ico = to_ico(&src, &opts).unwrap();
        let count = u16::from_le_bytes([ico[4], ico[5]]);
        assert_eq!(count, 3); // 16, 32, 48
    }

    #[test]
    fn non_square_source_makes_square_frame() {
        let src = sample_png(120, 80); // non-square
        let opts = Options {
            sizes: vec![32],
            ..Default::default()
        };
        let ico = to_ico(&src, &opts).unwrap();
        let decoded = image::load_from_memory(&ico).unwrap();
        assert_eq!(decoded.width(), 32);
        assert_eq!(decoded.height(), 32);
    }

    #[test]
    fn rejects_garbage_input() {
        assert!(to_ico(b"not an image", &Options::default()).is_err());
    }

    #[test]
    fn all_sizes_over_cap_is_error() {
        let src = sample_png(64, 64);
        let opts = Options {
            sizes: vec![512, 1024],
            ..Default::default()
        };
        assert!(to_ico(&src, &opts).is_err());
    }

    #[test]
    fn fit_modes_parse() {
        assert_eq!(parse_fit("contain").unwrap(), Fit::Contain);
        assert_eq!(parse_fit("COVER").unwrap(), Fit::Cover);
        assert_eq!(parse_fit("pad").unwrap(), Fit::Contain);
        assert!(parse_fit("nope").is_err());
    }

    #[test]
    fn color_parses() {
        assert_eq!(parse_color("#fff").unwrap(), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_color("#10203040").unwrap(), Rgba([16, 32, 48, 64]));
        assert_eq!(parse_color("00000000").unwrap(), Rgba([0, 0, 0, 0]));
        assert!(parse_color("#zz").is_err());
    }

    #[test]
    fn sizes_parse() {
        assert_eq!(parse_sizes("16, 32 ,48").unwrap(), vec![16, 32, 48]);
        assert!(parse_sizes("  ").is_err());
        assert!(parse_sizes("16,abc").is_err());
    }
}
