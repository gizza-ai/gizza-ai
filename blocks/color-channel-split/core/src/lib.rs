//! gizza-ai/color-channel-split core — extract a single color channel (red, green,
//! blue, or alpha) from an image as its own PNG, for steganography and colour
//! analysis. Pure-Rust (`image`).
//!
//! Supports the RGBA channels plus the four CMYK (cyan / magenta / yellow / key)
//! separations derived from RGB — matching what online channel-splitter tools offer.
//!
//! A channel can be rendered two ways:
//! - `grayscale` (default): the channel's value becomes a gray level (R=G=B=value,
//!   alpha solid). This is the classic "channel image" view used in stego / image
//!   forensics, where the bit-planes of one channel are inspected on their own.
//! - `color`: the channel value is kept in its own colour slot and the other colour
//!   channels are zeroed (e.g. the red channel renders as a red-only image). The
//!   alpha channel in `color` mode renders as opacity over black; CMYK channels
//!   render as their ink colour at the channel's intensity.

use std::io::Cursor;

use image::ImageFormat;

/// Which channel to extract: RGBA, or a CMYK separation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Red,
    Green,
    Blue,
    Alpha,
    Cyan,
    Magenta,
    Yellow,
    Key,
}

impl Channel {
    pub fn parse(s: &str) -> Result<Channel, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "red" | "r" | "" => Ok(Channel::Red),
            "green" | "g" => Ok(Channel::Green),
            "blue" | "b" => Ok(Channel::Blue),
            "alpha" | "a" => Ok(Channel::Alpha),
            "cyan" | "c" => Ok(Channel::Cyan),
            "magenta" | "m" => Ok(Channel::Magenta),
            "yellow" | "y" => Ok(Channel::Yellow),
            "key" | "black" | "k" => Ok(Channel::Key),
            other => Err(format!(
                "unknown channel '{other}' (use red, green, blue, alpha, cyan, magenta, yellow, or key)"
            )),
        }
    }

    /// Extract this channel's 0..=255 intensity from an `[r, g, b, a]` pixel.
    /// RGBA channels are a direct lookup; CMYK channels are derived from RGB via
    /// the standard naive conversion (k = 1 - max(r,g,b); c = (1-r-k)/(1-k); …).
    fn value(self, px: [u8; 4]) -> u8 {
        match self {
            Channel::Red => px[0],
            Channel::Green => px[1],
            Channel::Blue => px[2],
            Channel::Alpha => px[3],
            Channel::Cyan | Channel::Magenta | Channel::Yellow | Channel::Key => {
                let r = px[0] as f32 / 255.0;
                let g = px[1] as f32 / 255.0;
                let b = px[2] as f32 / 255.0;
                let k = 1.0 - r.max(g).max(b);
                let comp = |x: f32| -> f32 {
                    if k >= 1.0 {
                        0.0
                    } else {
                        (1.0 - x - k) / (1.0 - k)
                    }
                };
                let v = match self {
                    Channel::Cyan => comp(r),
                    Channel::Magenta => comp(g),
                    Channel::Yellow => comp(b),
                    Channel::Key => k,
                    _ => unreachable!(),
                };
                (v.clamp(0.0, 1.0) * 255.0).round() as u8
            }
        }
    }

    /// In `color` mode, the RGBA tuple this channel's `v` intensity maps to.
    fn colorize(self, v: u8) -> [u8; 4] {
        match self {
            Channel::Red => [v, 0, 0, 255],
            Channel::Green => [0, v, 0, 255],
            Channel::Blue => [0, 0, v, 255],
            // alpha-as-colour: opacity over black
            Channel::Alpha => [0, 0, 0, v],
            // CMYK inks rendered at the channel's intensity over white-ish: show the
            // ink colour scaled by v. Cyan=(0,255,255), Magenta=(255,0,255),
            // Yellow=(255,255,0), Key(black)=gray ramp.
            Channel::Cyan => [255 - v, 255, 255, 255],
            Channel::Magenta => [255, 255 - v, 255, 255],
            Channel::Yellow => [255, 255, 255 - v, 255],
            Channel::Key => [255 - v, 255 - v, 255 - v, 255],
        }
    }
}

/// How to render the extracted channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Channel value as a gray level (R=G=B=value, opaque). Default.
    Grayscale,
    /// Channel value kept in its own colour slot, others zeroed.
    Color,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "grayscale" | "greyscale" | "gray" | "grey" | "" => Ok(Mode::Grayscale),
            "color" | "colour" | "colored" => Ok(Mode::Color),
            other => Err(format!("unknown mode '{other}' (use grayscale or color)")),
        }
    }
}

/// Extract `channel` from `image_bytes`, rendering per `mode`. Returns PNG bytes of
/// the same dimensions as the input.
pub fn channel_split(image_bytes: &[u8], channel: Channel, mode: Mode) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("could not decode image: {e}"))?;
    let rgba = img.to_rgba8();
    let mut out = image::RgbaImage::new(rgba.width(), rgba.height());

    for (op, ip) in out.pixels_mut().zip(rgba.pixels()) {
        let v = channel.value(ip.0);
        op.0 = match mode {
            Mode::Grayscale => [v, v, v, 255],
            Mode::Color => channel.colorize(v),
        };
    }

    let mut buf = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(out)
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, Rgba, RgbaImage};

    fn solid_png(color: [u8; 4]) -> Vec<u8> {
        let img = RgbaImage::from_pixel(8, 8, Rgba(color));
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    fn first_pixel(png: &[u8]) -> [u8; 4] {
        image::load_from_memory(png).unwrap().get_pixel(0, 0).0
    }

    #[test]
    fn parse_channel() {
        assert_eq!(Channel::parse("red").unwrap(), Channel::Red);
        assert_eq!(Channel::parse("R").unwrap(), Channel::Red);
        assert_eq!(Channel::parse("").unwrap(), Channel::Red);
        assert_eq!(Channel::parse("GREEN").unwrap(), Channel::Green);
        assert_eq!(Channel::parse("b").unwrap(), Channel::Blue);
        assert_eq!(Channel::parse("alpha").unwrap(), Channel::Alpha);
        assert_eq!(Channel::parse("cyan").unwrap(), Channel::Cyan);
        assert_eq!(Channel::parse("M").unwrap(), Channel::Magenta);
        assert_eq!(Channel::parse("yellow").unwrap(), Channel::Yellow);
        assert_eq!(Channel::parse("black").unwrap(), Channel::Key);
        assert_eq!(Channel::parse("k").unwrap(), Channel::Key);
        assert!(Channel::parse("teal").is_err());
    }

    #[test]
    fn cmyk_separation_of_pure_red() {
        // pure red (255,0,0): k=0; cyan=0, magenta=255, yellow=255
        let png = solid_png([255, 0, 0, 255]);
        assert_eq!(
            first_pixel(&channel_split(&png, Channel::Cyan, Mode::Grayscale).unwrap()),
            [0, 0, 0, 255]
        );
        assert_eq!(
            first_pixel(&channel_split(&png, Channel::Magenta, Mode::Grayscale).unwrap()),
            [255, 255, 255, 255]
        );
        assert_eq!(
            first_pixel(&channel_split(&png, Channel::Yellow, Mode::Grayscale).unwrap()),
            [255, 255, 255, 255]
        );
        assert_eq!(
            first_pixel(&channel_split(&png, Channel::Key, Mode::Grayscale).unwrap()),
            [0, 0, 0, 255]
        );
    }

    #[test]
    fn cmyk_key_of_black_and_white() {
        // pure black → k=255; pure white → k=0
        let blk = first_pixel(
            &channel_split(&solid_png([0, 0, 0, 255]), Channel::Key, Mode::Grayscale).unwrap(),
        );
        assert_eq!(blk, [255, 255, 255, 255]);
        let wht = first_pixel(
            &channel_split(&solid_png([255, 255, 255, 255]), Channel::Key, Mode::Grayscale).unwrap(),
        );
        assert_eq!(wht, [0, 0, 0, 255]);
    }

    #[test]
    fn cmyk_color_mode_renders_ink() {
        // cyan ink at full intensity (v=255) → (0,255,255)
        let png = solid_png([0, 255, 255, 255]); // pure cyan colour: c=255
        let c = first_pixel(&channel_split(&png, Channel::Cyan, Mode::Color).unwrap());
        assert_eq!(c, [0, 255, 255, 255]);
    }

    #[test]
    fn parse_mode() {
        assert_eq!(Mode::parse("grayscale").unwrap(), Mode::Grayscale);
        assert_eq!(Mode::parse("").unwrap(), Mode::Grayscale);
        assert_eq!(Mode::parse("grey").unwrap(), Mode::Grayscale);
        assert_eq!(Mode::parse("color").unwrap(), Mode::Color);
        assert_eq!(Mode::parse("colour").unwrap(), Mode::Color);
        assert!(Mode::parse("rainbow").is_err());
    }

    #[test]
    fn grayscale_uses_channel_value_for_all_rgb() {
        // pixel: r=10 g=20 b=30 a=40
        let png = solid_png([10, 20, 30, 40]);
        let r = first_pixel(&channel_split(&png, Channel::Red, Mode::Grayscale).unwrap());
        assert_eq!(r, [10, 10, 10, 255]);
        let g = first_pixel(&channel_split(&png, Channel::Green, Mode::Grayscale).unwrap());
        assert_eq!(g, [20, 20, 20, 255]);
        let b = first_pixel(&channel_split(&png, Channel::Blue, Mode::Grayscale).unwrap());
        assert_eq!(b, [30, 30, 30, 255]);
        let a = first_pixel(&channel_split(&png, Channel::Alpha, Mode::Grayscale).unwrap());
        assert_eq!(a, [40, 40, 40, 255]);
    }

    #[test]
    fn color_mode_keeps_channel_in_own_slot() {
        let png = solid_png([10, 20, 30, 40]);
        assert_eq!(
            first_pixel(&channel_split(&png, Channel::Red, Mode::Color).unwrap()),
            [10, 0, 0, 255]
        );
        assert_eq!(
            first_pixel(&channel_split(&png, Channel::Green, Mode::Color).unwrap()),
            [0, 20, 0, 255]
        );
        assert_eq!(
            first_pixel(&channel_split(&png, Channel::Blue, Mode::Color).unwrap()),
            [0, 0, 30, 255]
        );
        // alpha-as-colour: black with the alpha value as opacity
        assert_eq!(
            first_pixel(&channel_split(&png, Channel::Alpha, Mode::Color).unwrap()),
            [0, 0, 0, 40]
        );
    }

    #[test]
    fn output_is_png_same_size() {
        let out = channel_split(&solid_png([1, 2, 3, 4]), Channel::Green, Mode::Grayscale).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (8, 8));
        assert_eq!(&out[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    }

    #[test]
    fn opaque_image_has_alpha_255() {
        // An opaque source has alpha=255 everywhere; grayscale alpha extraction is white.
        let png = solid_png([100, 150, 200, 255]);
        let a = first_pixel(&channel_split(&png, Channel::Alpha, Mode::Grayscale).unwrap());
        assert_eq!(a, [255, 255, 255, 255]);
    }

    #[test]
    fn errors_on_bad_image() {
        assert!(channel_split(b"not an image", Channel::Red, Mode::Grayscale).is_err());
    }
}
