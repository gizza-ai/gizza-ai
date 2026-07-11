//! gizza-ai/auto-white-balance core — remove a color cast from an image by
//! neutralizing it in RGB space. No wafer/wasm-bindgen deps. Pure-Rust `image`.
//!
//! Two classic statistical methods, both parameter-free:
//! - **gray-world**: assumes the average scene color is neutral gray. Scale each
//!   channel by `overall_gray / channel_mean` so the mean becomes gray.
//! - **white-patch** (White-Patch Retinex): assumes the brightest pixels should
//!   be white. Scale each channel by `255 / channel_max`.
//!
//! `strength` (0–100) blends the corrected pixels back with the original so the
//! correction can be dialed down; alpha is preserved.

use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};

/// White-balance algorithm.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Method {
    /// Scale each channel so the image's average color becomes neutral gray.
    GrayWorld,
    /// Scale each channel so its brightest pixel becomes white.
    WhitePatch,
}

impl Method {
    /// Parse the canonical descriptor value. Unknown values are an error so the
    /// caller can report bad args rather than silently defaulting.
    pub fn parse(s: &str) -> Result<Method, String> {
        match s {
            "gray-world" => Ok(Method::GrayWorld),
            "white-patch" => Ok(Method::WhitePatch),
            other => Err(format!(
                "unknown method '{other}' (expected 'gray-world' or 'white-patch')"
            )),
        }
    }
}

/// Per-channel multiplicative scale factors for the chosen method. A channel
/// with no signal (mean/max 0) keeps a scale of 1.0 so it is left untouched.
fn channel_scales(buf: &RgbaImage, method: Method) -> [f64; 3] {
    match method {
        Method::GrayWorld => {
            let mut sum = [0f64; 3];
            let mut n = 0f64;
            for p in buf.pixels() {
                for c in 0..3 {
                    sum[c] += p.0[c] as f64;
                }
                n += 1.0;
            }
            if n == 0.0 {
                return [1.0; 3];
            }
            let mean = [sum[0] / n, sum[1] / n, sum[2] / n];
            let gray = (mean[0] + mean[1] + mean[2]) / 3.0;
            let mut s = [1.0; 3];
            for c in 0..3 {
                if mean[c] > 0.0 {
                    s[c] = gray / mean[c];
                }
            }
            s
        }
        Method::WhitePatch => {
            let mut max = [0u8; 3];
            for p in buf.pixels() {
                for c in 0..3 {
                    if p.0[c] > max[c] {
                        max[c] = p.0[c];
                    }
                }
            }
            let mut s = [1.0; 3];
            for c in 0..3 {
                if max[c] > 0 {
                    s[c] = 255.0 / max[c] as f64;
                }
            }
            s
        }
    }
}

/// White-balance `bytes`. `method` is "gray-world" or "white-patch"; `strength`
/// (0–100, clamped) blends the corrected result with the original (100 = full
/// correction, 0 = unchanged). Returns PNG bytes. Alpha is preserved.
pub fn white_balance(bytes: &[u8], method: &str, strength: f64) -> Result<Vec<u8>, String> {
    let method = Method::parse(method)?;
    let blend = strength.clamp(0.0, 100.0) / 100.0;
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }
    let mut buf: RgbaImage = img.to_rgba8();
    let s = channel_scales(&buf, method);

    for p in buf.pixels_mut() {
        let a = p.0[3];
        let mut out = [0u8; 3];
        for c in 0..3 {
            let orig = p.0[c] as f64;
            let corrected = (orig * s[c]).clamp(0.0, 255.0);
            let mixed = orig + (corrected - orig) * blend;
            out[c] = mixed.round().clamp(0.0, 255.0) as u8;
        }
        *p = Rgba([out[0], out[1], out[2], a]);
    }

    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(buf)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode a solid-color RGBA image as PNG.
    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut img = RgbaImage::new(w, h);
        for p in img.pixels_mut() {
            *p = Rgba(rgba);
        }
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    fn pixels(w: u32, h: u32, data: &[[u8; 4]]) -> Vec<u8> {
        let mut img = RgbaImage::new(w, h);
        for (i, p) in img.pixels_mut().enumerate() {
            *p = Rgba(data[i]);
        }
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn gray_world_neutralizes_uniform_cast() {
        // A uniform reddish image: mean = (200,100,100), gray = 133.33.
        // Each channel is scaled to the gray mean, so the result is neutral.
        let png = solid(2, 2, [200, 100, 100, 255]);
        let out = white_balance(&png, "gray-world", 100.0).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        let p = img.get_pixel(0, 0).0;
        assert_eq!([p[0], p[1], p[2]], [133, 133, 133], "cast neutralized to gray");
        assert_eq!(p[3], 255, "alpha preserved");
    }

    #[test]
    fn white_patch_maps_brightest_pixel_to_white() {
        // Brightest per channel: R=200,G=100,B=50 → scales 1.275, 2.55, 5.1.
        // The pixel holding those maxima becomes pure white.
        let png = pixels(2, 1, &[[200, 100, 50, 255], [100, 50, 25, 255]]);
        let out = white_balance(&png, "white-patch", 100.0).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.get_pixel(0, 0).0, [255, 255, 255, 255], "brightest → white");
    }

    #[test]
    fn strength_zero_is_identity() {
        let png = pixels(2, 1, &[[200, 100, 100, 255], [40, 60, 90, 128]]);
        let out = white_balance(&png, "gray-world", 0.0).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.get_pixel(0, 0).0, [200, 100, 100, 255]);
        assert_eq!(img.get_pixel(1, 0).0, [40, 60, 90, 128], "unchanged + alpha kept");
    }

    #[test]
    fn already_neutral_image_is_unchanged() {
        // A gray image has equal channel means, so gray-world scales are all 1.
        let png = solid(3, 3, [128, 128, 128, 255]);
        let out = white_balance(&png, "gray-world", 100.0).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.get_pixel(1, 1).0, [128, 128, 128, 255]);
    }

    #[test]
    fn partial_strength_blends() {
        // 200 → gray-world corrected 133; at strength 50 the output is halfway.
        let png = solid(2, 2, [200, 100, 100, 255]);
        let out = white_balance(&png, "gray-world", 50.0).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        let p = img.get_pixel(0, 0).0;
        // R: 200 + (133-200)*0.5 = 166.5 → 167 ; G/B: 100 + (133-100)*0.5 = 116.5 → 117
        assert_eq!([p[0], p[1], p[2]], [167, 117, 117]);
    }

    #[test]
    fn errors_on_bad_image() {
        assert!(white_balance(b"not an image", "gray-world", 100.0).is_err());
    }

    #[test]
    fn errors_on_unknown_method() {
        let png = solid(1, 1, [10, 20, 30, 255]);
        assert!(white_balance(&png, "auto-magic", 100.0).is_err());
    }
}
