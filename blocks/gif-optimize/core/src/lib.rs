//! gizza-ai/gif-optimize core — shrink an animated GIF by scaling it down,
//! dropping frames, and/or posterizing colors. Pure-Rust (`image` crate's GIF
//! decode/encode) — no ffmpeg/gifsicle. Re-encodes to GIF, preserving timing.

use std::io::Cursor;

use image::codecs::gif::{GifDecoder, GifEncoder, Repeat};
use image::imageops::FilterType;
use image::{AnimationDecoder, Delay, Frame, RgbaImage};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Optimized {
    pub bytes: Vec<u8>,
    pub frames_in: usize,
    pub frames_out: usize,
    pub width: u32,
    pub height: u32,
}

/// Posterize a frame to `bits` bits per channel (1-8). 8 = no change.
fn posterize(img: &mut RgbaImage, bits: u8) {
    if bits >= 8 {
        return;
    }
    let shift = 8 - bits;
    let mask = !((1u16 << shift) - 1) as u8;
    for p in img.pixels_mut() {
        // keep alpha; quantize RGB
        p.0[0] &= mask;
        p.0[1] &= mask;
        p.0[2] &= mask;
    }
}

/// Optimize a GIF.
/// * `scale` in (0,1] resizes each frame (1.0 = original size).
/// * `frame_step` keeps every Nth frame (1 = all). Dropped frames' delays roll
///   into the kept frame so playback duration stays roughly the same.
/// * `posterize_bits` (1-8) reduces colors; 8 disables it.
pub fn optimize(
    gif: &[u8],
    scale: f64,
    frame_step: usize,
    posterize_bits: u8,
) -> Result<Optimized, String> {
    if gif.is_empty() {
        return Err("input is empty".into());
    }
    let scale = if scale.is_finite() { scale.clamp(0.01, 1.0) } else { 1.0 };
    let step = frame_step.max(1);
    let bits = posterize_bits.clamp(1, 8);

    let decoder = GifDecoder::new(Cursor::new(gif)).map_err(|e| format!("not a valid GIF: {e}"))?;
    let frames: Vec<Frame> = decoder
        .into_frames()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("could not decode GIF frames: {e}"))?;
    if frames.is_empty() {
        return Err("GIF has no frames".into());
    }
    let frames_in = frames.len();

    let mut out_w = 0u32;
    let mut out_h = 0u32;
    let mut out_frames: Vec<Frame> = Vec::new();
    let mut pending_ms: u32 = 0; // accumulated delay from dropped frames

    for (i, frame) in frames.into_iter().enumerate() {
        let (numer, denom) = frame.delay().numer_denom_ms();
        let ms = if denom == 0 { 0 } else { numer / denom };
        if i % step != 0 {
            pending_ms += ms; // dropped → roll its time forward
            continue;
        }
        let buf = frame.into_buffer(); // RgbaImage
        let (w, h) = buf.dimensions();
        let nw = ((w as f64 * scale).round() as u32).max(1);
        let nh = ((h as f64 * scale).round() as u32).max(1);
        let mut resized = if (nw, nh) == (w, h) {
            buf
        } else {
            image::imageops::resize(&buf, nw, nh, FilterType::Lanczos3)
        };
        posterize(&mut resized, bits);
        out_w = nw;
        out_h = nh;

        let total_ms = ms + pending_ms;
        pending_ms = 0;
        let delay = Delay::from_numer_denom_ms(total_ms.max(10), 1);
        out_frames.push(Frame::from_parts(resized, 0, 0, delay));
    }

    if out_frames.is_empty() {
        return Err("frame_step dropped every frame".into());
    }
    let frames_out = out_frames.len();

    let mut out = Cursor::new(Vec::new());
    {
        let mut enc = GifEncoder::new(&mut out);
        enc.set_repeat(Repeat::Infinite).map_err(|e| format!("gif repeat: {e}"))?;
        for f in out_frames {
            enc.encode_frame(f).map_err(|e| format!("gif encode: {e}"))?;
        }
    }

    Ok(Optimized {
        bytes: out.into_inner(),
        frames_in,
        frames_out,
        width: out_w,
        height: out_h,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, Rgba};

    /// Build an N-frame WxH gif in memory.
    fn make_gif(n: usize, w: u32, h: u32) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        {
            let mut enc = GifEncoder::new(&mut out);
            enc.set_repeat(Repeat::Infinite).unwrap();
            for i in 0..n {
                let c = Rgba([(i * 40) as u8, 100, 200, 255]);
                let img = RgbaImage::from_pixel(w, h, c);
                enc.encode_frame(Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(100, 1)))
                    .unwrap();
            }
        }
        out.into_inner()
    }

    fn count_frames(gif: &[u8]) -> usize {
        let d = GifDecoder::new(Cursor::new(gif.to_vec())).unwrap();
        d.into_frames().collect::<Result<Vec<_>, _>>().unwrap().len()
    }

    #[test]
    fn scales_down() {
        let g = make_gif(2, 40, 40);
        let o = optimize(&g, 0.5, 1, 8).unwrap();
        assert_eq!((o.width, o.height), (20, 20));
        assert_eq!(o.frames_out, 2);
        assert_eq!(&o.bytes[0..3], b"GIF");
        // decoded frame really is 20x20
        let d = GifDecoder::new(Cursor::new(o.bytes)).unwrap();
        let fr = d.into_frames().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(fr[0].buffer().dimensions(), (20, 20));
    }

    #[test]
    fn drops_frames() {
        let g = make_gif(6, 10, 10);
        let o = optimize(&g, 1.0, 2, 8).unwrap();
        assert_eq!(o.frames_in, 6);
        assert_eq!(o.frames_out, 3); // keep indices 0,2,4
        assert_eq!(count_frames(&o.bytes), 3);
    }

    #[test]
    fn posterize_reduces_distinct_colors() {
        let mut img = RgbaImage::new(16, 1);
        for x in 0..16 {
            img.put_pixel(x, 0, Rgba([(x * 16) as u8, (x * 16) as u8, (x * 16) as u8, 255]));
        }
        let mut out = Cursor::new(Vec::new());
        {
            let mut enc = GifEncoder::new(&mut out);
            enc.encode_frame(Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(100, 1))).unwrap();
        }
        let g = out.into_inner();
        let o = optimize(&g, 1.0, 1, 2).unwrap();
        let d = GifDecoder::new(Cursor::new(o.bytes)).unwrap();
        let fr = d.into_frames().collect::<Result<Vec<_>, _>>().unwrap();
        let mut set = std::collections::HashSet::new();
        for p in fr[0].buffer().pixels() {
            set.insert((p.0[0], p.0[1], p.0[2]));
        }
        assert!(set.len() <= 5, "posterize should collapse the gradient, got {}", set.len());
    }

    #[test]
    fn errors() {
        assert!(optimize(&[], 1.0, 1, 8).is_err());
        let g = make_gif(1, 4, 4);
        assert!(optimize(&g, 1.0, 1, 8).is_ok()); // a valid 1-frame gif optimizes fine
        assert!(optimize(b"not a gif at all", 1.0, 1, 8).is_err()); // junk bytes rejected
    }
}
