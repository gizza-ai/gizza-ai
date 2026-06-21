//! gizza-ai/gif-from-images core — combine a set of images into a single
//! animated GIF. Pure-Rust (`image` crate's GIF encoder) — no ffmpeg, so it runs
//! on every backend. Each frame is scaled to fit a common canvas (aspect ratio
//! preserved) and padded with a background color.

use std::io::Cursor;

use image::codecs::gif::{GifEncoder, Repeat};
use image::imageops::FilterType;
use image::{Delay, DynamicImage, Frame, GenericImageView, Rgba, RgbaImage};

/// Parse `#rgb`, `#rrggbb`, or `#rrggbbaa` into RGBA.
pub fn parse_color(s: &str) -> Result<Rgba<u8>, String> {
    let h = s.trim().trim_start_matches('#');
    let v = |a: &str| u8::from_str_radix(a, 16).map_err(|_| format!("invalid color '{s}'"));
    let (r, g, b, a) = match h.len() {
        3 => {
            let d = |c: char| u8::from_str_radix(&c.to_string().repeat(2), 16).unwrap();
            let cs: Vec<char> = h.chars().collect();
            (d(cs[0]), d(cs[1]), d(cs[2]), 255)
        }
        6 => (v(&h[0..2])?, v(&h[2..4])?, v(&h[4..6])?, 255),
        8 => (v(&h[0..2])?, v(&h[2..4])?, v(&h[4..6])?, v(&h[6..8])?),
        _ => return Err(format!("invalid color '{s}' (use #rgb, #rrggbb, or #rrggbbaa)")),
    };
    Ok(Rgba([r, g, b, a]))
}

/// Build an animated GIF from `images` (encoded bytes). `delay_ms` is the
/// per-frame delay; `bg` fills the letterbox/padding. Returns GIF bytes.
pub fn gif_from_bytes(images: &[Vec<u8>], delay_ms: u16, bg: Rgba<u8>) -> Result<Vec<u8>, String> {
    if images.is_empty() {
        return Err("provide at least one image".into());
    }
    let delay = delay_ms.clamp(10, 60_000);

    // Decode all frames; compute the common canvas as the max width/height.
    let decoded: Vec<DynamicImage> = images
        .iter()
        .enumerate()
        .map(|(i, b)| {
            image::load_from_memory(b).map_err(|e| format!("image #{}: could not decode: {e}", i + 1))
        })
        .collect::<Result<_, _>>()?;

    let canvas_w = decoded.iter().map(|d| d.width()).max().unwrap_or(0).max(1);
    let canvas_h = decoded.iter().map(|d| d.height()).max().unwrap_or(0).max(1);

    let mut out = Cursor::new(Vec::new());
    {
        let mut enc = GifEncoder::new(&mut out);
        enc.set_repeat(Repeat::Infinite).map_err(|e| format!("gif repeat: {e}"))?;
        for d in &decoded {
            let mut canvas: RgbaImage = RgbaImage::from_pixel(canvas_w, canvas_h, bg);
            let (iw, ih) = d.dimensions();
            // Scale to fit inside the canvas, preserving aspect ratio.
            let scale = (canvas_w as f64 / iw as f64).min(canvas_h as f64 / ih as f64);
            let nw = ((iw as f64 * scale).round() as u32).max(1);
            let nh = ((ih as f64 * scale).round() as u32).max(1);
            let resized = d.resize_exact(nw, nh, FilterType::Lanczos3).to_rgba8();
            let ox = (canvas_w - nw) / 2;
            let oy = (canvas_h - nh) / 2;
            image::imageops::overlay(&mut canvas, &resized, ox as i64, oy as i64);

            let frame = Frame::from_parts(canvas, 0, 0, Delay::from_numer_denom_ms(delay as u32, 1));
            enc.encode_frame(frame).map_err(|e| format!("gif encode frame: {e}"))?;
        }
    }
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageFormat;

    fn solid_png(w: u32, h: u32, color: Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, color);
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#fff").unwrap(), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_color("#ff0000").unwrap(), Rgba([255, 0, 0, 255]));
        assert_eq!(parse_color("#00000080").unwrap(), Rgba([0, 0, 0, 128]));
        assert!(parse_color("nope").is_err());
    }

    #[test]
    fn builds_multiframe_gif() {
        let frames = vec![
            solid_png(20, 20, Rgba([255, 0, 0, 255])),
            solid_png(20, 20, Rgba([0, 255, 0, 255])),
            solid_png(10, 30, Rgba([0, 0, 255, 255])),
        ];
        let gif = gif_from_bytes(&frames, 200, Rgba([0, 0, 0, 255])).unwrap();
        // GIF magic header.
        assert_eq!(&gif[0..6], b"GIF89a");
        // Decode back: should have 3 frames at the common 20x30 canvas.
        let cursor = Cursor::new(gif);
        let dec = image::codecs::gif::GifDecoder::new(cursor).unwrap();
        use image::AnimationDecoder;
        let frames: Vec<_> = dec.into_frames().collect::<Result<_, _>>().unwrap();
        assert_eq!(frames.len(), 3);
        let (w, h) = frames[0].buffer().dimensions();
        assert_eq!((w, h), (20, 30));
    }

    #[test]
    fn single_image_ok() {
        let gif = gif_from_bytes(&[solid_png(8, 8, Rgba([1, 2, 3, 255]))], 500, Rgba([255, 255, 255, 255])).unwrap();
        assert_eq!(&gif[0..3], b"GIF");
    }

    #[test]
    fn errors() {
        assert!(gif_from_bytes(&[], 100, Rgba([0, 0, 0, 255])).is_err());
        assert!(gif_from_bytes(&[b"not an image".to_vec()], 100, Rgba([0, 0, 0, 255])).is_err());
    }
}
