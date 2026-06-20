//! gizza-ai/image-pixelate-censor core — censor a rectangular region of an image
//! by pixelating (mosaic) or blurring it. No wafer/wasm-bindgen deps. Pure-Rust
//! `image` crate. Only the chosen region is modified; the rest is untouched.

use std::io::Cursor;

use image::{imageops, DynamicImage, GenericImage, GenericImageView, ImageFormat, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Pixelate,
    Blur,
}

pub fn parse_mode(s: &str) -> Result<Mode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "pixelate" | "pixel" | "mosaic" => Ok(Mode::Pixelate),
        "blur" => Ok(Mode::Blur),
        other => Err(format!("mode {other:?} not supported (pixelate|blur)")),
    }
}

/// Censor the rectangle `(x, y, w, h)` of `bytes` using `mode`. `strength` is the
/// mosaic tile size (pixelate) or blur sigma (blur); `0` picks a sensible default.
/// The region is clamped to the image bounds. Returns PNG bytes.
pub fn censor(
    bytes: &[u8],
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    mode: Mode,
    strength: f32,
) -> Result<Vec<u8>, String> {
    let src = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (iw, ih) = src.dimensions();
    if x >= iw || y >= ih {
        return Err(format!("region origin ({x}, {y}) is outside the image ({iw}x{ih})"));
    }
    if w == 0 || h == 0 {
        return Err("region width and height must be greater than 0".into());
    }
    let rw = w.min(iw - x);
    let rh = h.min(ih - y);

    let mut canvas: RgbaImage = src.to_rgba8();
    let region = canvas.view(x, y, rw, rh).to_image();

    let processed: RgbaImage = match mode {
        Mode::Pixelate => {
            let block = if strength <= 0.0 { 16.0 } else { strength };
            let block = (block.round() as u32).clamp(2, 1024);
            let sw = (rw / block).max(1);
            let sh = (rh / block).max(1);
            // Downscale (averaging) then upscale with nearest → mosaic tiles.
            let small = imageops::resize(&region, sw, sh, imageops::FilterType::Triangle);
            imageops::resize(&small, rw, rh, imageops::FilterType::Nearest)
        }
        Mode::Blur => {
            let sigma = if strength <= 0.0 { 12.0 } else { strength };
            let sigma = sigma.clamp(0.5, 100.0);
            imageops::blur(&region, sigma)
        }
    };

    canvas
        .copy_from(&processed, x, y)
        .map_err(|e| format!("failed to composite censored region: {e}"))?;

    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(canvas)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    /// An image: left half region (0..rw) one color, rest another.
    fn two_zone_png(w: u32, h: u32, region_w: u32) -> Vec<u8> {
        let mut img = RgbaImage::from_pixel(w, h, Rgba([0, 0, 255, 255])); // blue background
        for yy in 0..h {
            for xx in 0..region_w {
                // checkerboard inside the region so a mosaic averages it
                let c = if (xx + yy) % 2 == 0 { 255 } else { 0 };
                img.put_pixel(xx, yy, Rgba([c, 0, 0, 255]));
            }
        }
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn parse_mode_values() {
        assert_eq!(parse_mode("pixelate").unwrap(), Mode::Pixelate);
        assert_eq!(parse_mode("").unwrap(), Mode::Pixelate);
        assert_eq!(parse_mode("blur").unwrap(), Mode::Blur);
        assert!(parse_mode("swirl").is_err());
    }

    #[test]
    fn pixelate_whole_region_is_uniform() {
        // 20x20, region 10x20 checkerboard. Pixelate with a tile >= region →
        // one tile → every region pixel becomes the same averaged color.
        let png = two_zone_png(20, 20, 10);
        let out = censor(&png, 0, 0, 10, 20, Mode::Pixelate, 100.0).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (20, 20));
        let a = img.get_pixel(0, 0);
        let b = img.get_pixel(9, 19);
        assert_eq!(a, b, "the whole pixelated region should be one flat color");
        // And it should differ from the original sharp checkerboard corner.
        assert_ne!(a, Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn region_only_outside_is_untouched() {
        let png = two_zone_png(20, 20, 10);
        let out = censor(&png, 0, 0, 10, 20, Mode::Blur, 8.0).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        // A pixel in the untouched (blue) zone is unchanged.
        assert_eq!(img.get_pixel(15, 5), Rgba([0, 0, 255, 255]));
    }

    #[test]
    fn region_is_clamped_to_bounds() {
        let png = two_zone_png(20, 20, 10);
        // w/h overflow the image — should clamp, not panic/error.
        let out = censor(&png, 5, 5, 1000, 1000, Mode::Pixelate, 8.0).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (20, 20));
    }

    #[test]
    fn errors() {
        let png = two_zone_png(10, 10, 5);
        assert!(censor(&png, 20, 0, 5, 5, Mode::Pixelate, 8.0).is_err()); // origin outside
        assert!(censor(&png, 0, 0, 0, 5, Mode::Pixelate, 8.0).is_err()); // zero width
        assert!(censor(b"nope", 0, 0, 5, 5, Mode::Pixelate, 8.0).is_err()); // bad image
    }
}
