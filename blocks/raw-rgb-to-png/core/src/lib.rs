//! gizza-ai/raw-rgb-to-png core — assemble raw RGB/RGBA pixel bytes plus a
//! width/height into a viewable PNG. Pure-Rust (`image` for the PNG encode, a
//! hand-rolled hex/decimal parser and `base64` for the input), so it runs on
//! ALL backends incl. the chat Service Worker. No wafer/wasm-bindgen deps.

use base64::{engine::general_purpose::STANDARD_NO_PAD as B64, Engine as _};
use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};

/// The largest image we will assemble, in pixels. 16 MP of RGBA is 64 MB of
/// pixel data — already far past what a data-URL envelope can carry, so a
/// bigger request is a mistake worth naming rather than an OOM.
pub const MAX_PIXELS: u64 = 16_000_000;
/// Per-axis cap, matching the other image tools.
pub const MAX_DIMENSION: u32 = 8192;

/// How the raw pixel bytes are written in the `data` string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// Hex digit pairs, e.g. `ff0000 00ff00` or `0xff,0x00,0x00`.
    Hex,
    /// Standard or URL-safe base64, padding optional.
    Base64,
    /// Decimal byte values 0-255, e.g. `255, 0, 0, 255`.
    Decimal,
}

impl Encoding {
    pub fn parse(s: &str) -> Result<Encoding, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "hex" | "" => Ok(Encoding::Hex),
            "base64" | "b64" => Ok(Encoding::Base64),
            "decimal" | "dec" => Ok(Encoding::Decimal),
            other => Err(format!(
                "unknown encoding '{other}' (use hex, base64, or decimal)"
            )),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Encoding::Hex => "hex",
            Encoding::Base64 => "base64",
            Encoding::Decimal => "decimal",
        }
    }
}

/// The channel layout of each pixel in the raw bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 3 bytes per pixel: red, green, blue.
    Rgb,
    /// 4 bytes per pixel: red, green, blue, alpha.
    Rgba,
}

impl PixelFormat {
    pub fn parse(s: &str) -> Result<PixelFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "rgb" | "rgb8" | "rgb24" | "" => Ok(PixelFormat::Rgb),
            "rgba" | "rgba8" | "rgba32" => Ok(PixelFormat::Rgba),
            other => Err(format!("unknown pixel format '{other}' (use rgb or rgba)")),
        }
    }
    /// Bytes per pixel.
    pub fn bpp(self) -> u32 {
        match self {
            PixelFormat::Rgb => 3,
            PixelFormat::Rgba => 4,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            PixelFormat::Rgb => "RGB",
            PixelFormat::Rgba => "RGBA",
        }
    }
    fn color_type(self) -> ExtendedColorType {
        match self {
            PixelFormat::Rgb => ExtendedColorType::Rgb8,
            PixelFormat::Rgba => ExtendedColorType::Rgba8,
        }
    }
}

/// The assembled PNG plus the facts worth reporting back.
#[derive(Debug, Clone)]
pub struct Assembled {
    /// The PNG file bytes.
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    /// How many raw bytes the `data` string decoded to.
    pub input_bytes: usize,
    /// The row stride actually used, in bytes (the tight `width * bpp` unless
    /// `row_stride` asked for padded rows).
    pub row_stride: u32,
}

/// Decode `data` under `encoding`, lay the bytes out as `width` x `height`
/// pixels of `format`, and encode the result as a PNG.
///
/// `row_stride` is the number of bytes from the start of one row to the start
/// of the next; 0 means tightly packed (`width * bpp`). Padding bytes between
/// rows are dropped. Trailing bytes after the last row's pixels are allowed
/// (a padded final row is common in framebuffer dumps) but nothing more.
pub fn assemble(
    data: &str,
    width: u32,
    height: u32,
    format: PixelFormat,
    encoding: Encoding,
    row_stride: u32,
) -> Result<Assembled, String> {
    if width == 0 || height == 0 {
        return Err(format!(
            "width and height must both be at least 1 (got {width}x{height})"
        ));
    }
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(format!(
            "{width}x{height} is too large — each side must be at most {MAX_DIMENSION} pixels"
        ));
    }
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(format!(
            "{width}x{height} is {} pixels, over the {MAX_PIXELS}-pixel limit",
            u64::from(width) * u64::from(height)
        ));
    }

    let bpp = format.bpp();
    let row_bytes = width * bpp; // <= 8192 * 4, no overflow
    let stride = if row_stride == 0 {
        row_bytes
    } else {
        row_stride
    };
    if stride < row_bytes {
        return Err(format!(
            "row_stride {stride} is smaller than one row of pixels ({width} px x {bpp} bytes = {row_bytes} bytes)"
        ));
    }

    let raw = decode(data, encoding)?;
    if raw.is_empty() {
        return Err("data decoded to 0 bytes — provide the raw pixel bytes".to_string());
    }

    // The last row needs its pixels but not its padding, so the acceptable
    // length is a range: [stride*(h-1) + row_bytes, stride*h].
    let min_len = u64::from(stride) * u64::from(height - 1) + u64::from(row_bytes);
    let max_len = u64::from(stride) * u64::from(height);
    let got = raw.len() as u64;
    if got < min_len {
        return Err(format!(
            "data is {got} bytes but {width}x{height} {} needs {min_len} bytes ({} bytes short) — check width, height, and pixel_format",
            format.label(),
            min_len - got
        ));
    }
    if got > max_len {
        return Err(format!(
            "data is {got} bytes but {width}x{height} {} needs {} bytes ({} bytes extra) — check width, height, and pixel_format",
            format.label(),
            if min_len == max_len { min_len.to_string() } else { format!("{min_len}-{max_len}") },
            got - max_len
        ));
    }

    // Copy row by row so padded strides collapse to a tight buffer.
    let mut pixels = Vec::with_capacity((row_bytes as usize) * (height as usize));
    for row in 0..height as usize {
        let start = row * stride as usize;
        pixels.extend_from_slice(&raw[start..start + row_bytes as usize]);
    }

    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(&pixels, width, height, format.color_type())
        .map_err(|e| format!("PNG encode failed: {e}"))?;

    Ok(Assembled {
        bytes: png,
        width,
        height,
        format,
        input_bytes: raw.len(),
        row_stride: stride,
    })
}

/// Decode the `data` string into raw bytes under `encoding`.
fn decode(data: &str, encoding: Encoding) -> Result<Vec<u8>, String> {
    match encoding {
        Encoding::Hex => decode_hex(data),
        Encoding::Base64 => decode_base64(data),
        Encoding::Decimal => decode_decimal(data),
    }
}

/// Split on the separators people actually paste between bytes: whitespace,
/// commas, semicolons, and square/curly brackets (so a pasted `[255, 0, 0]`
/// array literal just works).
fn tokens(data: &str) -> impl Iterator<Item = &str> {
    data.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '[' | ']' | '{' | '}'))
        .filter(|t| !t.is_empty())
}

fn decode_hex(data: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for tok in tokens(data) {
        let digits = tok
            .strip_prefix("0x")
            .or_else(|| tok.strip_prefix("0X"))
            .unwrap_or(tok);
        if digits.is_empty() {
            continue;
        }
        if digits.len() % 2 != 0 {
            return Err(format!(
                "hex value '{tok}' has an odd number of digits — each byte needs two (e.g. 0f, not f)"
            ));
        }
        let bytes = digits.as_bytes();
        for pair in bytes.chunks(2) {
            let hi = hex_nibble(pair[0], tok)?;
            let lo = hex_nibble(pair[1], tok)?;
            out.push(hi << 4 | lo);
        }
    }
    Ok(out)
}

fn hex_nibble(c: u8, tok: &str) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!(
            "'{tok}' is not valid hex — expected hex digit pairs like ff0000, got '{}'",
            c as char
        )),
    }
}

fn decode_base64(data: &str) -> Result<Vec<u8>, String> {
    // Accept the URL-safe alphabet and optional padding by normalising both away.
    let cleaned: String = data
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '=')
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();
    // A data URL prefix is a common paste; drop it rather than failing on ':'.
    let cleaned = match cleaned.split_once(";base64,") {
        Some((_, tail)) => tail.to_string(),
        None => cleaned,
    };
    B64.decode(cleaned.as_bytes())
        .map_err(|e| format!("data is not valid base64: {e} — or set encoding to hex/decimal"))
}

fn decode_decimal(data: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for tok in tokens(data) {
        let n: u32 = tok
            .parse()
            .map_err(|_| format!("'{tok}' is not a decimal byte value — expected 0-255"))?;
        if n > 255 {
            return Err(format!(
                "{n} is out of range — decimal byte values run 0-255"
            ));
        }
        out.push(n as u8);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    fn decode_png(bytes: &[u8]) -> image::DynamicImage {
        image::load_from_memory_with_format(bytes, image::ImageFormat::Png).unwrap()
    }

    #[test]
    fn rgb_hex_round_trips_dimensions_and_pixels() {
        // 2x2: red, green / blue, white.
        let data = "ff0000 00ff00 0000ff ffffff";
        let a = assemble(data, 2, 2, PixelFormat::Rgb, Encoding::Hex, 0).unwrap();
        assert_eq!(
            &a.bytes[0..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
        );
        assert_eq!((a.width, a.height), (2, 2));
        assert_eq!(a.input_bytes, 12);
        assert_eq!(a.row_stride, 6);

        let img = decode_png(&a.bytes);
        assert_eq!(img.dimensions(), (2, 2));
        let rgba = img.to_rgba8();
        assert_eq!(rgba.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(rgba.get_pixel(1, 0).0, [0, 255, 0, 255]);
        assert_eq!(rgba.get_pixel(0, 1).0, [0, 0, 255, 255]);
        assert_eq!(rgba.get_pixel(1, 1).0, [255, 255, 255, 255]);
    }

    #[test]
    fn rgba_keeps_alpha_channel() {
        // 2x1: opaque red, half-transparent blue.
        let data = "ff0000ff 0000ff80";
        let a = assemble(data, 2, 1, PixelFormat::Rgba, Encoding::Hex, 0).unwrap();
        assert_eq!(a.input_bytes, 8);
        let rgba = decode_png(&a.bytes).to_rgba8();
        assert_eq!(rgba.dimensions(), (2, 1));
        assert_eq!(rgba.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(rgba.get_pixel(1, 0).0, [0, 0, 255, 128]);
    }

    #[test]
    fn decimal_and_base64_match_hex() {
        let hex = assemble("ff0000 00ff00", 2, 1, PixelFormat::Rgb, Encoding::Hex, 0).unwrap();
        let dec = assemble(
            "255,0,0, 0,255,0",
            2,
            1,
            PixelFormat::Rgb,
            Encoding::Decimal,
            0,
        )
        .unwrap();
        // base64 of ff 00 00 00 ff 00
        let b64 = assemble("/wAAAP8A", 2, 1, PixelFormat::Rgb, Encoding::Base64, 0).unwrap();
        assert_eq!(hex.bytes, dec.bytes);
        assert_eq!(hex.bytes, b64.bytes);
    }

    #[test]
    fn base64_accepts_url_safe_padding_and_data_url() {
        let plain = assemble("/wAAAP8A", 2, 1, PixelFormat::Rgb, Encoding::Base64, 0).unwrap();
        let urlsafe = assemble("_wAAAP8A=", 2, 1, PixelFormat::Rgb, Encoding::Base64, 0).unwrap();
        let dataurl = assemble(
            "data:application/octet-stream;base64,/wAAAP8A",
            2,
            1,
            PixelFormat::Rgb,
            Encoding::Base64,
            0,
        )
        .unwrap();
        assert_eq!(plain.bytes, urlsafe.bytes);
        assert_eq!(plain.bytes, dataurl.bytes);
    }

    #[test]
    fn row_stride_drops_padding_bytes() {
        // 2x2 RGB with 8-byte rows: 6 pixel bytes + 2 pad bytes per row.
        let data = "ff0000 00ff00 dead 0000ff ffffff beef";
        let a = assemble(data, 2, 2, PixelFormat::Rgb, Encoding::Hex, 8).unwrap();
        assert_eq!(a.row_stride, 8);
        assert_eq!(a.input_bytes, 16);
        let rgba = decode_png(&a.bytes).to_rgba8();
        assert_eq!(rgba.dimensions(), (2, 2));
        assert_eq!(rgba.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(rgba.get_pixel(1, 1).0, [255, 255, 255, 255]);
    }

    #[test]
    fn final_row_padding_may_be_omitted() {
        // Same as above but the last row's 2 pad bytes are missing.
        let data = "ff0000 00ff00 dead 0000ff ffffff";
        let a = assemble(data, 2, 2, PixelFormat::Rgb, Encoding::Hex, 8).unwrap();
        assert_eq!(a.input_bytes, 14);
        assert_eq!(decode_png(&a.bytes).dimensions(), (2, 2));
    }

    #[test]
    fn too_few_bytes_names_the_shortfall() {
        let err = assemble("ff0000 00ff00", 2, 2, PixelFormat::Rgb, Encoding::Hex, 0).unwrap_err();
        assert!(err.contains("6 bytes"), "{err}");
        assert!(err.contains("needs 12 bytes"), "{err}");
        assert!(err.contains("6 bytes short"), "{err}");
    }

    #[test]
    fn too_many_bytes_names_the_excess() {
        let err = assemble(
            "ff0000 00ff00 0000ff",
            2,
            1,
            PixelFormat::Rgb,
            Encoding::Hex,
            0,
        )
        .unwrap_err();
        assert!(err.contains("9 bytes"), "{err}");
        assert!(err.contains("6 bytes"), "{err}");
        assert!(err.contains("3 bytes extra"), "{err}");
    }

    #[test]
    fn rgb_data_rejected_as_rgba() {
        // 12 bytes is a valid 2x2 RGB but not a valid 2x2 RGBA (needs 16).
        let data = "ff0000 00ff00 0000ff ffffff";
        let err = assemble(data, 2, 2, PixelFormat::Rgba, Encoding::Hex, 0).unwrap_err();
        assert!(err.contains("RGBA"), "{err}");
        assert!(err.contains("needs 16 bytes"), "{err}");
    }

    #[test]
    fn rejects_bad_dimensions_and_stride() {
        let e = assemble("ff0000", 0, 1, PixelFormat::Rgb, Encoding::Hex, 0).unwrap_err();
        assert!(e.contains("at least 1"), "{e}");
        let e = assemble("ff0000", 9000, 1, PixelFormat::Rgb, Encoding::Hex, 0).unwrap_err();
        assert!(e.contains("at most 8192"), "{e}");
        let e = assemble("ff0000", 4001, 4001, PixelFormat::Rgb, Encoding::Hex, 0).unwrap_err();
        assert!(e.contains("pixel limit"), "{e}");
        let e = assemble("ff0000", 2, 1, PixelFormat::Rgb, Encoding::Hex, 4).unwrap_err();
        assert!(e.contains("row_stride 4 is smaller"), "{e}");
    }

    #[test]
    fn rejects_malformed_input_bytes() {
        let e = assemble("ff000", 1, 1, PixelFormat::Rgb, Encoding::Hex, 0).unwrap_err();
        assert!(e.contains("odd number of digits"), "{e}");
        let e = assemble("ff00zz", 1, 1, PixelFormat::Rgb, Encoding::Hex, 0).unwrap_err();
        assert!(e.contains("not valid hex"), "{e}");
        let e = assemble("255,0,300", 1, 1, PixelFormat::Rgb, Encoding::Decimal, 0).unwrap_err();
        assert!(e.contains("0-255"), "{e}");
        let e = assemble("!!!!", 1, 1, PixelFormat::Rgb, Encoding::Base64, 0).unwrap_err();
        assert!(e.contains("not valid base64"), "{e}");
        let e = assemble("   ", 1, 1, PixelFormat::Rgb, Encoding::Hex, 0).unwrap_err();
        assert!(e.contains("0 bytes"), "{e}");
    }

    #[test]
    fn parses_format_and_encoding_aliases() {
        assert_eq!(PixelFormat::parse("RGBA").unwrap(), PixelFormat::Rgba);
        assert_eq!(PixelFormat::parse("rgb24").unwrap(), PixelFormat::Rgb);
        assert!(PixelFormat::parse("bgr").is_err());
        assert_eq!(Encoding::parse("Base64").unwrap(), Encoding::Base64);
        assert_eq!(Encoding::parse("dec").unwrap(), Encoding::Decimal);
        assert!(Encoding::parse("uuencode").is_err());
    }

    #[test]
    fn larger_image_keeps_every_pixel_exact() {
        // 16x16 RGB gradient — checks the row walk, not just the 2x2 corner case.
        let w = 16u32;
        let h = 16u32;
        let mut hex = String::new();
        for y in 0..h {
            for x in 0..w {
                hex.push_str(&format!("{:02x}{:02x}{:02x}", x * 16, y * 16, (x + y) * 8));
            }
        }
        let a = assemble(&hex, w, h, PixelFormat::Rgb, Encoding::Hex, 0).unwrap();
        assert_eq!(a.input_bytes, (w * h * 3) as usize);
        let rgba = decode_png(&a.bytes).to_rgba8();
        assert_eq!(rgba.dimensions(), (w, h));
        for y in 0..h {
            for x in 0..w {
                assert_eq!(
                    rgba.get_pixel(x, y).0,
                    [(x * 16) as u8, (y * 16) as u8, ((x + y) * 8) as u8, 255],
                    "pixel {x},{y}"
                );
            }
        }
    }
}
