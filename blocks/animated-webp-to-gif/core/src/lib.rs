//! gizza-ai/animated-webp-to-gif core — convert an animated (or still) WebP into
//! an animated GIF for maximum compatibility. Pure-Rust: the `image` crate decodes
//! the WebP frames (with per-frame delays) and re-encodes them as a looping GIF —
//! no ffmpeg, so it runs on every backend (incl. the chat Service Worker).

use std::io::Cursor;

use image::codecs::gif::{GifEncoder, Repeat};
use image::codecs::webp::WebPDecoder;
use image::imageops::FilterType;
use image::{AnimationDecoder, Delay, Frame};

/// Loop behaviour for the produced GIF.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopMode {
    /// Repeat forever (the WebP/GIF default for animations).
    Infinite,
    /// Play the animation exactly once.
    Once,
}

impl LoopMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "infinite" | "forever" | "loop" | "" => Ok(LoopMode::Infinite),
            "once" | "no" | "none" => Ok(LoopMode::Once),
            other => Err(format!(
                "invalid loop '{other}' (use 'infinite' or 'once')"
            )),
        }
    }

    fn repeat(self) -> Repeat {
        match self {
            LoopMode::Infinite => Repeat::Infinite,
            LoopMode::Once => Repeat::Finite(0),
        }
    }
}

/// Convert WebP `bytes` (animated or still) into GIF bytes.
///
/// `loop_mode` controls GIF looping. `speed` (1..=30) is the GIF encoder's
/// quality/speed knob — 1 = best palette/quality (slowest), 30 = fastest. When
/// `force_delay_ms` is `Some(ms)`, every frame uses that delay instead of the
/// WebP's own per-frame timing (handy for fixing broken/zero timings); when
/// `None`, original frame delays are preserved (falling back to 100 ms for any
/// frame that reports a zero delay). When `width` is `Some(w)`, every frame is
/// resized to width `w` with the height scaled to preserve aspect ratio; `None`
/// keeps the source dimensions.
pub fn webp_to_gif(
    bytes: &[u8],
    loop_mode: LoopMode,
    speed: i32,
    force_delay_ms: Option<u32>,
    width: Option<u32>,
) -> Result<Vec<u8>, String> {
    if bytes.is_empty() {
        return Err("no input bytes — provide an animated WebP file".into());
    }
    let speed = speed.clamp(1, 30);

    let decoder = WebPDecoder::new(Cursor::new(bytes))
        .map_err(|e| format!("could not read WebP: {e}"))?;

    // Animated WebP → per-frame timing; still WebP → decode the single image as
    // one frame (the `image` animation iterator yields nothing for a still).
    let frames: Vec<Frame> = if decoder.has_animation() {
        decoder
            .into_frames()
            .collect::<Result<Vec<Frame>, _>>()
            .map_err(|e| format!("could not decode WebP frames: {e}"))?
    } else {
        let img = image::DynamicImage::from_decoder(decoder)
            .map_err(|e| format!("could not decode WebP image: {e}"))?
            .to_rgba8();
        vec![Frame::new(img)]
    };

    if frames.is_empty() {
        return Err("WebP contained no frames".into());
    }

    let mut out = Cursor::new(Vec::new());
    {
        let mut enc = GifEncoder::new_with_speed(&mut out, speed);
        enc.set_repeat(loop_mode.repeat())
            .map_err(|e| format!("gif repeat: {e}"))?;
        for f in frames {
            let delay = match force_delay_ms {
                Some(ms) => Delay::from_numer_denom_ms(ms.max(10), 1),
                None => {
                    let (num, den) = f.delay().numer_denom_ms();
                    if num == 0 {
                        // Some WebP frames report a zero delay; GIF needs a
                        // sensible default so the animation is visible.
                        Delay::from_numer_denom_ms(100, 1)
                    } else {
                        Delay::from_numer_denom_ms(num, den)
                    }
                }
            };
            let buf = f.into_buffer();
            let buf = match width {
                Some(target_w) if target_w >= 1 && target_w != buf.width() => {
                    let scale = target_w as f64 / buf.width() as f64;
                    let target_h = ((buf.height() as f64 * scale).round() as u32).max(1);
                    image::imageops::resize(&buf, target_w, target_h, FilterType::Lanczos3)
                }
                _ => buf,
            };
            let frame = Frame::from_parts(buf, 0, 0, delay);
            enc.encode_frame(frame)
                .map_err(|e| format!("gif encode frame: {e}"))?;
        }
    }
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::webp::WebPEncoder;
    use image::{ExtendedColorType, ImageDecoder};

    /// Encode a tiny solid still WebP (lossless) for round-trip tests.
    fn still_webp(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let pixels: Vec<u8> = (0..(w * h)).flat_map(|_| rgba).collect();
        let mut out = Cursor::new(Vec::new());
        WebPEncoder::new_lossless(&mut out)
            .encode(&pixels, w, h, ExtendedColorType::Rgba8)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn loop_mode_parse() {
        assert_eq!(LoopMode::parse("infinite").unwrap(), LoopMode::Infinite);
        assert_eq!(LoopMode::parse("FOREVER").unwrap(), LoopMode::Infinite);
        assert_eq!(LoopMode::parse("").unwrap(), LoopMode::Infinite);
        assert_eq!(LoopMode::parse("once").unwrap(), LoopMode::Once);
        assert!(LoopMode::parse("twice").is_err());
    }

    #[test]
    fn still_webp_converts_to_valid_gif() {
        let webp = still_webp(8, 6, [200, 30, 30, 255]);
        let gif = webp_to_gif(&webp, LoopMode::Infinite, 10, None, None).unwrap();
        // GIF magic header.
        assert_eq!(&gif[0..6], b"GIF89a");
        // Decodes back to a frame of the same size.
        let dec = image::codecs::gif::GifDecoder::new(Cursor::new(&gif)).unwrap();
        let (w, h) = dec.dimensions();
        assert_eq!((w, h), (8, 6));
    }

    #[test]
    fn force_delay_overrides_timing() {
        let webp = still_webp(4, 4, [0, 0, 255, 255]);
        let gif = webp_to_gif(&webp, LoopMode::Once, 5, Some(250), None).unwrap();
        let dec = image::codecs::gif::GifDecoder::new(Cursor::new(&gif)).unwrap();
        let frames = dec.into_frames().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(frames.len(), 1);
        // GIF stores delay in 10ms units, so 250ms -> 25 -> 250ms back.
        let (num, den) = frames[0].delay().numer_denom_ms();
        assert_eq!(num / den, 250);
    }

    #[test]
    fn width_resizes_preserving_aspect() {
        let webp = still_webp(20, 10, [10, 200, 50, 255]);
        let gif = webp_to_gif(&webp, LoopMode::Infinite, 10, None, Some(10)).unwrap();
        let dec = image::codecs::gif::GifDecoder::new(Cursor::new(&gif)).unwrap();
        let (w, h) = dec.dimensions();
        // 20x10 scaled to width 10 -> 10x5.
        assert_eq!((w, h), (10, 5));
    }

    #[test]
    fn empty_input_errors() {
        assert!(webp_to_gif(&[], LoopMode::Infinite, 10, None, None).is_err());
    }
}
