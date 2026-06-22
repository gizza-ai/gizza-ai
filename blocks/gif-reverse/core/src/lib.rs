//! gizza-ai/gif-reverse core — reverse the frame order of an animated GIF.
//! Pure Rust (`image` crate's GIF decode/encode, no ffmpeg/gifsicle) so it runs
//! on every backend including the chat Service Worker. Decodes all frames,
//! reverses their order (keeping each frame's own delay with it so playback
//! speed is unchanged), and re-encodes to a looping GIF. An optional boomerang
//! (ping-pong) mode plays the GIF forward then backward for a seamless loop.

use std::io::Cursor;

use image::codecs::gif::{GifDecoder, GifEncoder, Repeat};
use image::{AnimationDecoder, Delay, Frame};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reversed {
    pub bytes: Vec<u8>,
    /// Frames in the source GIF.
    pub frames_in: usize,
    /// Frames in the output GIF (= frames_in, or 2*frames_in-1 for boomerang).
    pub frames_out: usize,
    pub width: u32,
    pub height: u32,
}

/// Reverse the playback order of an animated GIF.
/// * `boomerang = false` → frames in reverse order (last frame first).
/// * `boomerang = true`  → forward then backward (ping-pong); the turnaround
///   frame is not duplicated, giving a seamless loop of `2*N-1` frames.
///
/// Each frame keeps its own delay, so the animation runs at its original speed.
/// The result always loops infinitely.
pub fn reverse(gif: &[u8], boomerang: bool) -> Result<Reversed, String> {
    if gif.is_empty() {
        return Err("input is empty".into());
    }

    let decoder = GifDecoder::new(Cursor::new(gif)).map_err(|e| format!("not a valid GIF: {e}"))?;
    let frames: Vec<Frame> = decoder
        .into_frames()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("could not decode GIF frames: {e}"))?;
    if frames.is_empty() {
        return Err("GIF has no frames".into());
    }
    let frames_in = frames.len();

    // Decompose into (rgba buffer, delay) so we can re-order while keeping each
    // frame's own timing.
    let parts: Vec<(image::RgbaImage, Delay)> = frames
        .into_iter()
        .map(|f| {
            let delay = f.delay();
            (f.into_buffer(), delay)
        })
        .collect();

    let (width, height) = parts[0].0.dimensions();

    // Build the output frame order.
    let mut out_frames: Vec<Frame> = Vec::with_capacity(if boomerang {
        parts.len().saturating_mul(2).saturating_sub(1)
    } else {
        parts.len()
    });

    // Reverse pass (always present).
    for (buf, delay) in parts.iter().rev() {
        out_frames.push(Frame::from_parts(buf.clone(), 0, 0, *delay));
    }
    if boomerang {
        // Forward pass, skipping the first frame which equals the last frame of
        // the reverse pass just emitted (avoids a stutter at the turnaround).
        for (buf, delay) in parts.iter().skip(1) {
            out_frames.push(Frame::from_parts(buf.clone(), 0, 0, *delay));
        }
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

    Ok(Reversed {
        bytes: out.into_inner(),
        frames_in,
        frames_out,
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    /// Build an N-frame WxH gif where frame i is a solid shade derived from i.
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

    fn decode_frames(gif: &[u8]) -> Vec<RgbaImage> {
        let d = GifDecoder::new(Cursor::new(gif.to_vec())).unwrap();
        d.into_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .into_iter()
            .map(|f| f.into_buffer())
            .collect()
    }

    #[test]
    fn reverses_frame_order() {
        let g = make_gif(4, 8, 8);
        let r = reverse(&g, false).unwrap();
        assert_eq!(r.frames_in, 4);
        assert_eq!(r.frames_out, 4);
        assert_eq!((r.width, r.height), (8, 8));
        assert_eq!(&r.bytes[0..3], b"GIF");

        let orig = decode_frames(&g);
        let rev = decode_frames(&r.bytes);
        // First output frame == last input frame; last output == first input.
        assert_eq!(rev[0].get_pixel(0, 0), orig[3].get_pixel(0, 0));
        assert_eq!(rev[3].get_pixel(0, 0), orig[0].get_pixel(0, 0));
    }

    #[test]
    fn boomerang_pings_and_pongs() {
        let g = make_gif(3, 4, 4);
        let r = reverse(&g, true).unwrap();
        assert_eq!(r.frames_in, 3);
        // 2*N-1 = 5 frames: reverse [2,1,0] then forward-skip-first [1,2].
        assert_eq!(r.frames_out, 5);

        let orig = decode_frames(&g);
        let bm = decode_frames(&r.bytes);
        let p = |img: &RgbaImage| *img.get_pixel(0, 0);
        assert_eq!(p(&bm[0]), p(&orig[2])); // last
        assert_eq!(p(&bm[1]), p(&orig[1]));
        assert_eq!(p(&bm[2]), p(&orig[0])); // first (turnaround)
        assert_eq!(p(&bm[3]), p(&orig[1]));
        assert_eq!(p(&bm[4]), p(&orig[2])); // back to last
    }

    #[test]
    fn single_frame_gif_keeps_one_frame() {
        let g = make_gif(1, 4, 4);
        let r = reverse(&g, false).unwrap();
        assert_eq!(r.frames_out, 1);
        let rb = reverse(&g, true).unwrap();
        // boomerang of a 1-frame gif = 2*1-1 = 1 frame.
        assert_eq!(rb.frames_out, 1);
    }

    #[test]
    fn errors() {
        assert!(reverse(&[], false).is_err());
        assert!(reverse(b"not a gif at all", false).is_err());
        let g = make_gif(2, 4, 4);
        assert!(reverse(&g, false).is_ok());
    }
}
