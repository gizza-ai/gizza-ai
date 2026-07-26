//! apply-alpha-mask core — use a second (grayscale) image as the alpha channel of
//! the first image, producing a transparent PNG cutout. Pure `image` crate; no
//! wafer/wasm-bindgen deps.
//!
//! Pipeline: the FIRST image is the picture whose RGB is kept. The SECOND image is
//! the mask; a per-pixel value is read from `channel` (luminance by default), the
//! mask is fitted to the picture's dimensions via `fit` (stretch / cover / contain),
//! optionally inverted and/or hard-thresholded, and that value becomes each output
//! pixel's alpha. Convention: WHITE (255) = fully opaque, BLACK (0) = fully
//! transparent (invert to swap). `existing_alpha` chooses whether the mask REPLACES
//! the picture's own alpha or MULTIPLIES into it (keeps existing transparency too).
//! Output is always PNG — the only common format that stores a full 8-bit alpha.

use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;

/// Which value of a mask pixel becomes the alpha level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskChannel {
    /// Perceptual luma (Rec.601: 0.299R + 0.587G + 0.114B). Default.
    Luminance,
    /// Unweighted mean of the R, G, B channels.
    Average,
    Red,
    Green,
    Blue,
    /// The mask image's OWN alpha channel.
    Alpha,
}

pub fn parse_channel(s: &str) -> Result<MaskChannel, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "luminance" | "luma" | "lightness" | "gray" | "grey" => Ok(MaskChannel::Luminance),
        "average" | "avg" | "mean" => Ok(MaskChannel::Average),
        "red" | "r" => Ok(MaskChannel::Red),
        "green" | "g" => Ok(MaskChannel::Green),
        "blue" | "b" => Ok(MaskChannel::Blue),
        "alpha" | "a" | "opacity" => Ok(MaskChannel::Alpha),
        other => Err(format!(
            "unknown mask channel `{other}` (use luminance, average, red, green, blue, or alpha)"
        )),
    }
}

/// How the mask is resized to the picture's dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskFit {
    /// Stretch the mask to exactly the picture size (ignores aspect ratio). Default.
    Stretch,
    /// Scale to COVER the picture (preserve aspect), center-crop the overflow.
    Cover,
    /// Scale to fit INSIDE the picture (preserve aspect); the uncovered border is
    /// fully transparent.
    Contain,
}

pub fn parse_fit(s: &str) -> Result<MaskFit, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "stretch" | "scale" | "fill" => Ok(MaskFit::Stretch),
        "cover" | "crop" => Ok(MaskFit::Cover),
        "contain" | "fit" | "pad" => Ok(MaskFit::Contain),
        other => Err(format!(
            "unknown fit `{other}` (use stretch, cover, or contain)"
        )),
    }
}

/// How the derived mask value combines with the picture's existing alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExistingAlpha {
    /// The mask value becomes the alpha outright (default).
    Replace,
    /// Multiply the mask value into whatever alpha the picture already had, so
    /// pixels transparent in EITHER stay transparent.
    Multiply,
}

pub fn parse_existing_alpha(s: &str) -> Result<ExistingAlpha, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "replace" | "set" | "overwrite" => Ok(ExistingAlpha::Replace),
        "multiply" | "combine" | "intersect" => Ok(ExistingAlpha::Multiply),
        other => Err(format!(
            "unknown existing_alpha `{other}` (use replace or multiply)"
        )),
    }
}

const MAX_DIM: u32 = 12_000;
const MAX_PIXELS: u64 = 40_000_000; // 40 MP output guard

fn channel_value(px: &Rgba<u8>, channel: MaskChannel) -> u8 {
    let [r, g, b, a] = px.0;
    match channel {
        MaskChannel::Luminance => {
            // Rec.601 luma, rounded.
            ((0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) + 0.5) as u8
        }
        MaskChannel::Average => ((r as u16 + g as u16 + b as u16) / 3) as u8,
        MaskChannel::Red => r,
        MaskChannel::Green => g,
        MaskChannel::Blue => b,
        MaskChannel::Alpha => a,
    }
}

/// Build a `w x h` grayscale grid of mask VALUES (pre-invert/threshold) plus a
/// coverage grid (`true` where the mask actually covers, used by `contain` so the
/// padded border stays transparent regardless of invert).
fn fit_mask(
    mask: &RgbaImage,
    channel: MaskChannel,
    fit: MaskFit,
    w: u32,
    h: u32,
) -> (Vec<u8>, Vec<bool>) {
    let (mw, mh) = mask.dimensions();
    let mut value = vec![0u8; (w as usize) * (h as usize)];
    let mut covered = vec![false; (w as usize) * (h as usize)];

    match fit {
        MaskFit::Stretch => {
            let resized = if (mw, mh) == (w, h) {
                mask.clone()
            } else {
                image::imageops::resize(mask, w, h, FilterType::Triangle)
            };
            for y in 0..h {
                for x in 0..w {
                    let idx = (y as usize) * (w as usize) + x as usize;
                    value[idx] = channel_value(resized.get_pixel(x, y), channel);
                    covered[idx] = true;
                }
            }
        }
        MaskFit::Cover => {
            let scale = (w as f32 / mw as f32).max(h as f32 / mh as f32);
            let nw = ((mw as f32 * scale).round() as u32).max(w);
            let nh = ((mh as f32 * scale).round() as u32).max(h);
            let resized = image::imageops::resize(mask, nw, nh, FilterType::Triangle);
            let ox = (nw - w) / 2;
            let oy = (nh - h) / 2;
            for y in 0..h {
                for x in 0..w {
                    let idx = (y as usize) * (w as usize) + x as usize;
                    value[idx] = channel_value(resized.get_pixel(x + ox, y + oy), channel);
                    covered[idx] = true;
                }
            }
        }
        MaskFit::Contain => {
            let scale = (w as f32 / mw as f32).min(h as f32 / mh as f32);
            let nw = ((mw as f32 * scale).round() as u32).clamp(1, w);
            let nh = ((mh as f32 * scale).round() as u32).clamp(1, h);
            let resized = image::imageops::resize(mask, nw, nh, FilterType::Triangle);
            let ox = (w - nw) / 2;
            let oy = (h - nh) / 2;
            for y in 0..nh {
                for x in 0..nw {
                    let idx = ((y + oy) as usize) * (w as usize) + (x + ox) as usize;
                    value[idx] = channel_value(resized.get_pixel(x, y), channel);
                    covered[idx] = true;
                }
            }
        }
    }
    (value, covered)
}

/// Apply `mask` as the alpha channel of `picture`, returning PNG bytes.
#[allow(clippy::too_many_arguments)]
pub fn apply_mask(
    picture: DynamicImage,
    mask: DynamicImage,
    channel: MaskChannel,
    invert: bool,
    fit: MaskFit,
    threshold: u8,
    existing: ExistingAlpha,
) -> Result<Vec<u8>, String> {
    let mut img = picture.to_rgba8();
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("picture image has zero dimensions".into());
    }
    if w > MAX_DIM || h > MAX_DIM {
        return Err(format!(
            "picture is {w}x{h}; each side must be <= {MAX_DIM}px"
        ));
    }
    if (w as u64) * (h as u64) > MAX_PIXELS {
        return Err(format!(
            "picture has {}px; the {MAX_PIXELS}px (40 MP) cap protects browser memory",
            (w as u64) * (h as u64)
        ));
    }

    let mask = mask.to_rgba8();
    let (value, covered) = fit_mask(&mask, channel, fit, w, h);

    for y in 0..h {
        for x in 0..w {
            let idx = (y as usize) * (w as usize) + x as usize;
            let mut v = value[idx];
            if invert {
                v = 255 - v;
            }
            if threshold > 0 {
                v = if v >= threshold { 255 } else { 0 };
            }
            // Padding outside a `contain` mask is always fully transparent.
            if !covered[idx] {
                v = 0;
            }
            let px = img.get_pixel_mut(x, y);
            let new_alpha = match existing {
                ExistingAlpha::Replace => v,
                ExistingAlpha::Multiply => ((px.0[3] as u16 * v as u16) / 255) as u8,
            };
            px.0[3] = new_alpha;
        }
    }

    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(img)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

/// Decode both inputs from bytes, then apply the mask.
#[allow(clippy::too_many_arguments)]
pub fn apply_mask_from_bytes(
    picture: &[u8],
    mask: &[u8],
    channel: MaskChannel,
    invert: bool,
    fit: MaskFit,
    threshold: u8,
    existing: ExistingAlpha,
) -> Result<Vec<u8>, String> {
    let picture = image::load_from_memory(picture)
        .map_err(|e| format!("picture image (first) could not be decoded: {e}"))?;
    let mask = image::load_from_memory(mask)
        .map_err(|e| format!("mask image (second) could not be decoded: {e}"))?;
    apply_mask(picture, mask, channel, invert, fit, threshold, existing)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, c: [u8; 4]) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, Rgba(c)))
    }
    fn decode(png: &[u8]) -> RgbaImage {
        image::load_from_memory(png).unwrap().to_rgba8()
    }

    const RED: [u8; 4] = [255, 0, 0, 255];
    const WHITE: [u8; 4] = [255, 255, 255, 255];
    const BLACK: [u8; 4] = [0, 0, 0, 255];
    const GRAY: [u8; 4] = [128, 128, 128, 255];

    #[test]
    fn parse_enums_and_errors() {
        assert_eq!(parse_channel("").unwrap(), MaskChannel::Luminance);
        assert_eq!(parse_channel("Alpha").unwrap(), MaskChannel::Alpha);
        assert_eq!(parse_channel("g").unwrap(), MaskChannel::Green);
        assert!(parse_channel("cyan").is_err());
        assert_eq!(parse_fit("").unwrap(), MaskFit::Stretch);
        assert_eq!(parse_fit("cover").unwrap(), MaskFit::Cover);
        assert!(parse_fit("tile").is_err());
        assert_eq!(parse_existing_alpha("").unwrap(), ExistingAlpha::Replace);
        assert_eq!(parse_existing_alpha("multiply").unwrap(), ExistingAlpha::Multiply);
        assert!(parse_existing_alpha("add").is_err());
    }

    #[test]
    fn white_mask_is_opaque_black_mask_is_transparent() {
        let pic = solid(4, 4, RED);
        // Left half white (opaque), right half black (transparent).
        let mut mask = RgbaImage::from_pixel(4, 4, Rgba(WHITE));
        for y in 0..4 {
            for x in 2..4 {
                mask.put_pixel(x, y, Rgba(BLACK));
            }
        }
        let png = apply_mask(
            pic,
            DynamicImage::ImageRgba8(mask),
            MaskChannel::Luminance,
            false,
            MaskFit::Stretch,
            0,
            ExistingAlpha::Replace,
        )
        .unwrap();
        let out = decode(&png);
        assert_eq!(out.get_pixel(0, 0).0, [255, 0, 0, 255], "white mask -> opaque, RGB kept");
        assert_eq!(out.get_pixel(3, 0).0[3], 0, "black mask -> transparent");
    }

    #[test]
    fn gray_mask_gives_partial_alpha() {
        let png = apply_mask(
            solid(2, 2, RED),
            solid(2, 2, GRAY),
            MaskChannel::Luminance,
            false,
            MaskFit::Stretch,
            0,
            ExistingAlpha::Replace,
        )
        .unwrap();
        assert_eq!(decode(&png).get_pixel(0, 0).0[3], 128, "gray 128 -> alpha 128");
    }

    #[test]
    fn invert_swaps_opacity() {
        let png = apply_mask(
            solid(2, 2, RED),
            solid(2, 2, BLACK),
            MaskChannel::Luminance,
            true,
            MaskFit::Stretch,
            0,
            ExistingAlpha::Replace,
        )
        .unwrap();
        assert_eq!(decode(&png).get_pixel(0, 0).0[3], 255, "inverted black -> opaque");
    }

    #[test]
    fn threshold_binarizes() {
        // Gray 128 with threshold 200 -> below cutoff -> transparent.
        let png = apply_mask(
            solid(2, 2, RED),
            solid(2, 2, GRAY),
            MaskChannel::Luminance,
            false,
            MaskFit::Stretch,
            200,
            ExistingAlpha::Replace,
        )
        .unwrap();
        assert_eq!(decode(&png).get_pixel(0, 0).0[3], 0, "128 < 200 -> hard transparent");
    }

    #[test]
    fn multiply_keeps_existing_transparency() {
        // Picture already 50% transparent; white mask should NOT make it opaque.
        let pic = solid(2, 2, [255, 0, 0, 128]);
        let png = apply_mask(
            pic,
            solid(2, 2, WHITE),
            MaskChannel::Luminance,
            false,
            MaskFit::Stretch,
            0,
            ExistingAlpha::Multiply,
        )
        .unwrap();
        assert_eq!(decode(&png).get_pixel(0, 0).0[3], 128, "128 * 255/255 = 128");
    }

    #[test]
    fn mask_is_resized_to_picture() {
        // 1x1 white mask stretched over a 8x8 picture -> all opaque.
        let png = apply_mask(
            solid(8, 8, RED),
            solid(1, 1, WHITE),
            MaskChannel::Luminance,
            false,
            MaskFit::Stretch,
            0,
            ExistingAlpha::Replace,
        )
        .unwrap();
        let out = decode(&png);
        assert_eq!(out.dimensions(), (8, 8));
        assert_eq!(out.get_pixel(7, 7).0[3], 255);
    }

    #[test]
    fn contain_border_is_transparent() {
        // A wide mask contained in a tall picture leaves transparent top/bottom bands.
        let png = apply_mask(
            solid(4, 8, RED),
            solid(4, 4, WHITE),
            MaskChannel::Luminance,
            false,
            MaskFit::Contain,
            0,
            ExistingAlpha::Replace,
        )
        .unwrap();
        let out = decode(&png);
        assert_eq!(out.get_pixel(0, 0).0[3], 0, "top band uncovered -> transparent");
        assert_eq!(out.get_pixel(0, 4).0[3], 255, "centered band covered -> opaque");
    }

    #[test]
    fn alpha_channel_source() {
        // Mask whose RGB is black but alpha is full -> using channel=alpha is opaque.
        let png = apply_mask(
            solid(2, 2, RED),
            solid(2, 2, [0, 0, 0, 255]),
            MaskChannel::Alpha,
            false,
            MaskFit::Stretch,
            0,
            ExistingAlpha::Replace,
        )
        .unwrap();
        assert_eq!(decode(&png).get_pixel(0, 0).0[3], 255, "alpha channel = 255 -> opaque");
    }

    #[test]
    fn bad_bytes_error() {
        let err = apply_mask_from_bytes(
            b"not an image",
            b"also not",
            MaskChannel::Luminance,
            false,
            MaskFit::Stretch,
            0,
            ExistingAlpha::Replace,
        )
        .unwrap_err();
        assert!(err.contains("picture image"), "got: {err}");
    }
}
