//! gizza-ai/bit-plane-view core — extract and render a single bit plane of a
//! chosen colour channel from an image, to reveal bit-level patterns and hidden
//! data (steganography / image forensics). Pure-Rust (`image`).
//!
//! An 8-bit channel value has 8 bit planes, from bit 0 (the least-significant
//! bit, LSB) to bit 7 (the most-significant bit, MSB). LSB planes carry the
//! finest detail and are the classic hiding place for steganographic payloads —
//! isolating a single plane makes an otherwise-invisible embedded message pop
//! out as structured noise against the natural image's randomness.
//!
//! The extracted plane can be rendered three ways:
//! - `binary` (default): a set bit renders white (255,255,255) and a clear bit
//!   renders black — the maximum-contrast view used in stego / forensics.
//! - `weighted`: the bit renders at its positional weight (value `bit << plane`)
//!   as a gray level, showing the bit's actual contribution to the pixel.
//! - `color`: a set bit renders in the channel's own colour (red plane → red,
//!   etc.; gray/alpha → white) and a clear bit renders black — a coloured overlay
//!   view for compositing several planes.

use std::io::Cursor;

use image::ImageFormat;

/// Which channel's bit plane to extract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Red,
    Green,
    Blue,
    Alpha,
    /// Rec. 601 luminance of the RGB pixel (0.299R + 0.587G + 0.114B).
    Gray,
}

impl Channel {
    pub fn parse(s: &str) -> Result<Channel, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "red" | "r" | "" => Ok(Channel::Red),
            "green" | "g" => Ok(Channel::Green),
            "blue" | "b" => Ok(Channel::Blue),
            "alpha" | "a" => Ok(Channel::Alpha),
            "gray" | "grey" | "luma" | "luminance" | "l" => Ok(Channel::Gray),
            other => Err(format!(
                "unknown channel '{other}' (use red, green, blue, alpha, or gray)"
            )),
        }
    }

    /// This channel's 0..=255 intensity for an `[r, g, b, a]` pixel.
    fn value(self, px: [u8; 4]) -> u8 {
        match self {
            Channel::Red => px[0],
            Channel::Green => px[1],
            Channel::Blue => px[2],
            Channel::Alpha => px[3],
            Channel::Gray => {
                let y = 0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32;
                y.round().clamp(0.0, 255.0) as u8
            }
        }
    }

    /// The full-intensity RGBA a set bit renders as in `color` mode.
    fn color_full(self) -> [u8; 4] {
        match self {
            Channel::Red => [255, 0, 0, 255],
            Channel::Green => [0, 255, 0, 255],
            Channel::Blue => [0, 0, 255, 255],
            // alpha / gray have no colour of their own → white
            Channel::Alpha | Channel::Gray => [255, 255, 255, 255],
        }
    }
}

/// How to render the extracted bit plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Set bit → white, clear bit → black. Maximum-contrast view. Default.
    Binary,
    /// Bit rendered at its positional weight (`bit << plane`) as a gray level.
    Weighted,
    /// Set bit → the channel's colour, clear bit → black.
    Color,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "binary" | "mono" | "bw" | "" => Ok(Mode::Binary),
            "weighted" | "weight" | "value" => Ok(Mode::Weighted),
            "color" | "colour" | "colored" => Ok(Mode::Color),
            other => Err(format!(
                "unknown mode '{other}' (use binary, weighted, or color)"
            )),
        }
    }
}

/// Extract bit plane `bit` (0 = LSB .. 7 = MSB) of `channel` from `image_bytes`,
/// rendering per `mode`. Returns PNG bytes of the same dimensions as the input.
pub fn bit_plane(
    image_bytes: &[u8],
    channel: Channel,
    bit: u32,
    mode: Mode,
) -> Result<Vec<u8>, String> {
    if bit > 7 {
        return Err(format!("bit must be 0..=7 (0 = LSB, 7 = MSB), got {bit}"));
    }
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("could not decode image: {e}"))?;
    let rgba = img.to_rgba8();
    let mut out = image::RgbaImage::new(rgba.width(), rgba.height());

    for (op, ip) in out.pixels_mut().zip(rgba.pixels()) {
        let v = channel.value(ip.0);
        let set = (v >> bit) & 1; // 0 or 1
        op.0 = match mode {
            Mode::Binary => {
                let g = if set == 1 { 255 } else { 0 };
                [g, g, g, 255]
            }
            Mode::Weighted => {
                let w = set << bit; // 0 or 2^bit
                [w, w, w, 255]
            }
            Mode::Color => {
                if set == 1 {
                    channel.color_full()
                } else {
                    [0, 0, 0, 255]
                }
            }
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
        assert_eq!(Channel::parse("gray").unwrap(), Channel::Gray);
        assert_eq!(Channel::parse("luma").unwrap(), Channel::Gray);
        assert!(Channel::parse("teal").is_err());
    }

    #[test]
    fn parse_mode() {
        assert_eq!(Mode::parse("binary").unwrap(), Mode::Binary);
        assert_eq!(Mode::parse("").unwrap(), Mode::Binary);
        assert_eq!(Mode::parse("mono").unwrap(), Mode::Binary);
        assert_eq!(Mode::parse("weighted").unwrap(), Mode::Weighted);
        assert_eq!(Mode::parse("color").unwrap(), Mode::Color);
        assert_eq!(Mode::parse("colour").unwrap(), Mode::Color);
        assert!(Mode::parse("rainbow").is_err());
    }

    #[test]
    fn binary_lsb_of_odd_red_is_white() {
        // red = 0b1010_0101 = 165 → LSB (bit 0) set
        let png = solid_png([165, 0, 0, 255]);
        assert_eq!(
            first_pixel(&bit_plane(&png, Channel::Red, 0, Mode::Binary).unwrap()),
            [255, 255, 255, 255]
        );
        // bit 1 of 165 (…0101) is clear
        assert_eq!(
            first_pixel(&bit_plane(&png, Channel::Red, 1, Mode::Binary).unwrap()),
            [0, 0, 0, 255]
        );
    }

    #[test]
    fn binary_msb_of_high_value_is_white() {
        // green = 200 = 0b1100_1000 → MSB (bit 7) set
        let png = solid_png([0, 200, 0, 255]);
        assert_eq!(
            first_pixel(&bit_plane(&png, Channel::Green, 7, Mode::Binary).unwrap()),
            [255, 255, 255, 255]
        );
        // blue channel is 0 → its MSB is clear
        assert_eq!(
            first_pixel(&bit_plane(&png, Channel::Blue, 7, Mode::Binary).unwrap()),
            [0, 0, 0, 255]
        );
    }

    #[test]
    fn weighted_shows_positional_weight() {
        // blue = 0b0100_0000 = 64 → bit 6 set; weighted render = 1<<6 = 64
        let png = solid_png([0, 0, 64, 255]);
        assert_eq!(
            first_pixel(&bit_plane(&png, Channel::Blue, 6, Mode::Weighted).unwrap()),
            [64, 64, 64, 255]
        );
        // bit 0 of 64 is clear → 0
        assert_eq!(
            first_pixel(&bit_plane(&png, Channel::Blue, 0, Mode::Weighted).unwrap()),
            [0, 0, 0, 255]
        );
    }

    #[test]
    fn color_mode_uses_channel_colour_for_set_bit() {
        // red = 1 → LSB set; color render = pure red
        let png = solid_png([1, 0, 0, 255]);
        assert_eq!(
            first_pixel(&bit_plane(&png, Channel::Red, 0, Mode::Color).unwrap()),
            [255, 0, 0, 255]
        );
        // clear bit → black
        assert_eq!(
            first_pixel(&bit_plane(&png, Channel::Red, 1, Mode::Color).unwrap()),
            [0, 0, 0, 255]
        );
    }

    #[test]
    fn alpha_plane_of_transparent_pixel() {
        // alpha = 128 = 0b1000_0000 → bit 7 set, bit 0 clear
        let png = solid_png([0, 0, 0, 128]);
        assert_eq!(
            first_pixel(&bit_plane(&png, Channel::Alpha, 7, Mode::Binary).unwrap()),
            [255, 255, 255, 255]
        );
        assert_eq!(
            first_pixel(&bit_plane(&png, Channel::Alpha, 0, Mode::Binary).unwrap()),
            [0, 0, 0, 255]
        );
    }

    #[test]
    fn gray_luma_lsb() {
        // pure white → luma 255 = all bits set → any plane is white
        let png = solid_png([255, 255, 255, 255]);
        assert_eq!(
            first_pixel(&bit_plane(&png, Channel::Gray, 0, Mode::Binary).unwrap()),
            [255, 255, 255, 255]
        );
        assert_eq!(
            first_pixel(&bit_plane(&png, Channel::Gray, 3, Mode::Binary).unwrap()),
            [255, 255, 255, 255]
        );
        // pure black → luma 0 → every plane clear
        let blk = solid_png([0, 0, 0, 255]);
        assert_eq!(
            first_pixel(&bit_plane(&blk, Channel::Gray, 4, Mode::Binary).unwrap()),
            [0, 0, 0, 255]
        );
    }

    #[test]
    fn output_is_png_same_size() {
        let out = bit_plane(&solid_png([1, 2, 3, 4]), Channel::Green, 0, Mode::Binary).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (8, 8));
        assert_eq!(&out[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    }

    #[test]
    fn errors_on_bad_bit() {
        assert!(bit_plane(&solid_png([1, 2, 3, 4]), Channel::Red, 8, Mode::Binary).is_err());
    }

    #[test]
    fn errors_on_bad_image() {
        assert!(bit_plane(b"not an image", Channel::Red, 0, Mode::Binary).is_err());
    }
}
