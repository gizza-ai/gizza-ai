//! gizza-ai/gif-resize core — resize an animated GIF to new pixel dimensions,
//! preserving every frame, its timing, and the loop. Pure-Rust (`image` crate's
//! GIF codec). Give a width and/or a height; if only one is given the other is
//! computed to preserve the aspect ratio.

use std::io::Cursor;

use image::codecs::gif::{GifDecoder, GifEncoder, Repeat};
use image::imageops::FilterType;
use image::{AnimationDecoder, Frame, GenericImageView};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Resized {
    pub bytes: Vec<u8>,
    pub frames: usize,
    pub orig_width: u32,
    pub orig_height: u32,
    pub width: u32,
    pub height: u32,
}

/// Resize a GIF. `width`/`height` are target pixels; pass `None` for one to
/// preserve the aspect ratio. At least one must be `Some`.
pub fn resize_gif(gif: &[u8], width: Option<u32>, height: Option<u32>) -> Result<Resized, String> {
    if gif.is_empty() {
        return Err("input is empty".into());
    }
    if width.is_none() && height.is_none() {
        return Err("provide a target width and/or height".into());
    }
    if width == Some(0) {
        return Err("width must be greater than 0".into());
    }
    if height == Some(0) {
        return Err("height must be greater than 0".into());
    }

    let decoder = GifDecoder::new(Cursor::new(gif)).map_err(|e| format!("not a valid GIF: {e}"))?;
    let frames: Vec<Frame> = decoder
        .into_frames()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("could not decode GIF frames: {e}"))?;
    if frames.is_empty() {
        return Err("GIF has no frames".into());
    }

    let (ow, oh) = frames[0].buffer().dimensions();
    if ow == 0 || oh == 0 {
        return Err("GIF has zero dimension".into());
    }
    // Resolve the target dimensions.
    let (tw, th) = match (width, height) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => {
            let h = ((w as f64) * (oh as f64) / (ow as f64)).round() as u32;
            (w, h.max(1))
        }
        (None, Some(h)) => {
            let w = ((h as f64) * (ow as f64) / (oh as f64)).round() as u32;
            (w.max(1), h)
        }
        (None, None) => unreachable!(),
    };

    let n = frames.len();
    let mut out = Cursor::new(Vec::new());
    {
        let mut enc = GifEncoder::new(&mut out);
        enc.set_repeat(Repeat::Infinite).map_err(|e| format!("gif repeat: {e}"))?;
        for frame in frames {
            let delay = frame.delay();
            let buf = frame.into_buffer();
            let resized = image::imageops::resize(&buf, tw, th, FilterType::Lanczos3);
            enc.encode_frame(Frame::from_parts(resized, 0, 0, delay))
                .map_err(|e| format!("gif encode: {e}"))?;
        }
    }

    Ok(Resized {
        bytes: out.into_inner(),
        frames: n,
        orig_width: ow,
        orig_height: oh,
        width: tw,
        height: th,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Delay, RgbaImage};

    fn make_gif(n: usize, w: u32, h: u32) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        {
            let mut enc = GifEncoder::new(&mut out);
            enc.set_repeat(Repeat::Infinite).unwrap();
            for i in 0..n {
                let c = image::Rgba([(i * 30) as u8, 80, 160, 255]);
                let img = RgbaImage::from_pixel(w, h, c);
                enc.encode_frame(Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(120, 1)))
                    .unwrap();
            }
        }
        out.into_inner()
    }

    fn decoded_dims(gif: &[u8]) -> (usize, (u32, u32)) {
        let d = GifDecoder::new(Cursor::new(gif.to_vec())).unwrap();
        let fr = d.into_frames().collect::<Result<Vec<_>, _>>().unwrap();
        let dims = fr[0].buffer().dimensions();
        (fr.len(), dims)
    }

    #[test]
    fn exact_dimensions() {
        let g = make_gif(3, 40, 20);
        let r = resize_gif(&g, Some(80), Some(80)).unwrap();
        assert_eq!((r.width, r.height), (80, 80));
        assert_eq!(r.frames, 3);
        let (n, dims) = decoded_dims(&r.bytes);
        assert_eq!(n, 3);
        assert_eq!(dims, (80, 80));
    }

    #[test]
    fn width_only_preserves_aspect() {
        let g = make_gif(2, 40, 20); // 2:1
        let r = resize_gif(&g, Some(100), None).unwrap();
        assert_eq!((r.width, r.height), (100, 50));
    }

    #[test]
    fn height_only_preserves_aspect() {
        let g = make_gif(2, 40, 20);
        let r = resize_gif(&g, None, Some(10)).unwrap();
        assert_eq!((r.width, r.height), (20, 10));
        assert_eq!((r.orig_width, r.orig_height), (40, 20));
    }

    #[test]
    fn errors() {
        let g = make_gif(1, 8, 8);
        assert!(resize_gif(&g, None, None).is_err());
        assert!(resize_gif(&g, Some(0), None).is_err());
        assert!(resize_gif(&[], Some(10), None).is_err());
        assert!(resize_gif(b"not a gif", Some(10), None).is_err());
    }
}
