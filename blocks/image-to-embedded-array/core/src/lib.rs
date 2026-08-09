//! gizza-ai/image-to-embedded-array core — turn an image into C/C++ source code
//! you can paste into embedded firmware for a TFT / OLED / e-paper display.
//! No wafer/wasm-bindgen deps: pure-Rust `image` decode + resize, then pixel
//! packing (RGB565/BGR565/RGB888/RGB332/grayscale/1-bit mono/XBM) and text
//! emission.
//!
//! Pipeline: decode → composite alpha over `background` → optional resize →
//! pack pixels into the target pixel format → render as a C array in the
//! requested declaration style.

use image::{imageops::FilterType, GenericImageView, RgbImage};

/// Largest accepted output edge, in pixels. Embedded panels are far smaller;
/// the cap stops a 12-megapixel photo from becoming a 70 MB source file.
pub const MAX_EDGE: u32 = 4096;
/// Largest accepted output pixel count (1024x1024).
pub const MAX_PIXELS: u64 = 1_048_576;
/// Largest accepted `per_line` value.
pub const MAX_PER_LINE: u32 = 64;

/// How pixels are packed into bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 16-bit 5R-6G-5B, the usual TFT format (Adafruit GFX, TFT_eSPI, LovyanGFX).
    Rgb565,
    /// 16-bit 5B-6G-5R — RGB565 with the red/blue channels swapped.
    Bgr565,
    /// 24-bit, 3 bytes per pixel in R, G, B order.
    Rgb888,
    /// 8-bit 3R-3G-2B.
    Rgb332,
    /// 8-bit luminance.
    Grayscale,
    /// 1 bit per pixel, rows padded to whole bytes.
    Mono,
    /// 1 bit per pixel emitted as an X BitMap (`.xbm`) source file.
    Xbm,
}

impl PixelFormat {
    pub fn parse(s: &str) -> Result<PixelFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "rgb565" => Ok(PixelFormat::Rgb565),
            "bgr565" => Ok(PixelFormat::Bgr565),
            "rgb888" => Ok(PixelFormat::Rgb888),
            "rgb332" => Ok(PixelFormat::Rgb332),
            "grayscale" | "gray" | "greyscale" | "grey" => Ok(PixelFormat::Grayscale),
            "mono" | "mono1" => Ok(PixelFormat::Mono),
            "xbm" => Ok(PixelFormat::Xbm),
            other => Err(format!(
                "unknown format \"{other}\" (expected rgb565, bgr565, rgb888, rgb332, grayscale, mono, or xbm)"
            )),
        }
    }

    /// Bits of flash each pixel costs in this format.
    pub fn bits_per_pixel(self) -> u32 {
        match self {
            PixelFormat::Rgb565 | PixelFormat::Bgr565 => 16,
            PixelFormat::Rgb888 => 24,
            PixelFormat::Rgb332 | PixelFormat::Grayscale => 8,
            PixelFormat::Mono | PixelFormat::Xbm => 1,
        }
    }

    fn is_16bit(self) -> bool {
        matches!(self, PixelFormat::Rgb565 | PixelFormat::Bgr565)
    }

    pub fn label(self) -> &'static str {
        match self {
            PixelFormat::Rgb565 => "RGB565",
            PixelFormat::Bgr565 => "BGR565",
            PixelFormat::Rgb888 => "RGB888",
            PixelFormat::Rgb332 => "RGB332",
            PixelFormat::Grayscale => "grayscale",
            PixelFormat::Mono => "1-bit mono",
            PixelFormat::Xbm => "XBM",
        }
    }
}

/// How each 16-bit pixel reaches the array. Ignored by the 8/24/1-bit formats,
/// which are always `uint8_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    /// One `uint16_t` element per pixel (`0xF800`) — what `pushImage`/
    /// `drawRGBBitmap` expect.
    Word,
    /// Two `uint8_t` elements per pixel, high byte first (`0xF8, 0x00`).
    Big,
    /// Two `uint8_t` elements per pixel, low byte first (`0x00, 0xF8`).
    Little,
}

impl ByteOrder {
    pub fn parse(s: &str) -> Result<ByteOrder, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "word" => Ok(ByteOrder::Word),
            "big" | "big-endian" | "be" => Ok(ByteOrder::Big),
            "little" | "little-endian" | "le" => Ok(ByteOrder::Little),
            other => Err(format!(
                "unknown byte_order \"{other}\" (expected word, big, or little)"
            )),
        }
    }
}

/// Which end of each byte the first pixel of a 1-bit row lands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitOrder {
    /// msb for `mono` (Adafruit GFX), lsb for `xbm` (the XBM spec).
    Auto,
    /// Leftmost pixel in bit 7.
    Msb,
    /// Leftmost pixel in bit 0.
    Lsb,
}

impl BitOrder {
    pub fn parse(s: &str) -> Result<BitOrder, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(BitOrder::Auto),
            "msb" | "msb-first" => Ok(BitOrder::Msb),
            "lsb" | "lsb-first" => Ok(BitOrder::Lsb),
            other => Err(format!(
                "unknown bit_order \"{other}\" (expected auto, msb, or lsb)"
            )),
        }
    }

    fn resolve(self, format: PixelFormat) -> bool {
        // returns true when the leftmost pixel goes in the most-significant bit
        match self {
            BitOrder::Msb => true,
            BitOrder::Lsb => false,
            BitOrder::Auto => format != PixelFormat::Xbm,
        }
    }
}

/// The declaration wrapped around the numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// `static const uint16_t name[] = { … };`
    Array,
    /// The array plus `PROGMEM` and an `<pgmspace.h>` include, for AVR/ESP.
    Arduino,
    /// A full `.h` file: include guard, `<stdint.h>`, width/height defines.
    Header,
    /// The comma-separated numbers only — no declaration, for pasting into
    /// an existing array or piping elsewhere.
    Raw,
}

impl Style {
    pub fn parse(s: &str) -> Result<Style, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "array" => Ok(Style::Array),
            "arduino" | "progmem" => Ok(Style::Arduino),
            "header" => Ok(Style::Header),
            "raw" | "values" => Ok(Style::Raw),
            other => Err(format!(
                "unknown style \"{other}\" (expected array, arduino, header, or raw)"
            )),
        }
    }
}

/// How each element is spelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberFormat {
    /// `0x1F`, `0xF800`.
    Hex,
    /// `31`, `63488`.
    Decimal,
}

impl NumberFormat {
    pub fn parse(s: &str) -> Result<NumberFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "hex" => Ok(NumberFormat::Hex),
            "decimal" | "dec" => Ok(NumberFormat::Decimal),
            other => Err(format!(
                "unknown number_format \"{other}\" (expected hex or decimal)"
            )),
        }
    }
}

/// Everything the conversion needs beyond the encoded image bytes.
#[derive(Debug, Clone)]
pub struct Options {
    pub format: PixelFormat,
    /// Output width in pixels; 0 keeps the source width (or derives it from
    /// `height` when only `height` is set).
    pub width: u32,
    /// Output height in pixels; 0 derives it from `width` and the aspect ratio.
    pub height: u32,
    pub byte_order: ByteOrder,
    pub bit_order: BitOrder,
    pub style: Style,
    pub number_format: NumberFormat,
    /// C identifier for the generated array.
    pub name: String,
    /// Elements per source line; 0 puts one image row per line.
    pub per_line: u32,
    /// 1-bit cutoff: pixels darker than this become ink bits.
    pub threshold: u8,
    /// Floyd–Steinberg dithering for the 1-bit formats.
    pub dither: bool,
    /// Flip ink/background for the 1-bit formats.
    pub invert: bool,
    /// Opaque colour transparent pixels are composited over, as R, G, B.
    pub background: [u8; 3],
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: PixelFormat::Rgb565,
            width: 0,
            height: 0,
            byte_order: ByteOrder::Word,
            bit_order: BitOrder::Auto,
            style: Style::Array,
            number_format: NumberFormat::Hex,
            name: "image".into(),
            per_line: 12,
            threshold: 128,
            dither: false,
            invert: false,
            background: [0, 0, 0],
        }
    }
}

/// The generated source plus the numbers a firmware author needs to budget flash.
#[derive(Debug, Clone)]
pub struct Generated {
    /// The C/C++ source text.
    pub code: String,
    /// Emitted image dimensions (after any resize).
    pub width: u32,
    pub height: u32,
    /// Dimensions of the decoded source image.
    pub src_width: u32,
    pub src_height: u32,
    /// Flash cost of the pixel data in bytes.
    pub bytes: usize,
    /// Number of array elements (pixels for `word`, bytes otherwise).
    pub elements: usize,
    /// C type of one element.
    pub data_type: String,
    pub bits_per_pixel: u32,
}

/// Parse `#rrggbb`, `#rgb` (with or without `#`), or a handful of common names.
pub fn parse_color(s: &str) -> Result<[u8; 3], String> {
    let t = s.trim();
    match t.to_ascii_lowercase().as_str() {
        "" | "black" => return Ok([0, 0, 0]),
        "white" => return Ok([255, 255, 255]),
        "red" => return Ok([255, 0, 0]),
        "green" => return Ok([0, 128, 0]),
        "blue" => return Ok([0, 0, 255]),
        "magenta" => return Ok([255, 0, 255]),
        _ => {}
    }
    let hex = t.strip_prefix('#').unwrap_or(t);
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "invalid background \"{s}\" (expected a hex colour like #101820 or #fff, or a name like black/white)"
        ));
    }
    let nibble = |c: char| c.to_digit(16).unwrap_or(0) as u8;
    let b: Vec<char> = hex.chars().collect();
    match b.len() {
        3 => Ok([nibble(b[0]) * 17, nibble(b[1]) * 17, nibble(b[2]) * 17]),
        6 => Ok([
            nibble(b[0]) * 16 + nibble(b[1]),
            nibble(b[2]) * 16 + nibble(b[3]),
            nibble(b[4]) * 16 + nibble(b[5]),
        ]),
        n => Err(format!(
            "invalid background \"{s}\": expected 3 or 6 hex digits, got {n}"
        )),
    }
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("name must not be empty (expected a C identifier like logo_data)".into());
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(format!(
            "invalid name \"{name}\": a C identifier must start with a letter or underscore"
        ));
    }
    if let Some(bad) = chars.find(|c| !(c.is_ascii_alphanumeric() || *c == '_')) {
        return Err(format!(
            "invalid name \"{name}\": '{bad}' is not allowed in a C identifier (use letters, digits, and underscores)"
        ));
    }
    Ok(())
}

/// Resolve the requested output size against the source size, preserving the
/// aspect ratio when only one edge is given.
fn target_size(src_w: u32, src_h: u32, want_w: u32, want_h: u32) -> Result<(u32, u32), String> {
    let (w, h) = match (want_w, want_h) {
        (0, 0) => (src_w, src_h),
        (w, 0) => {
            let h = ((w as f64) * (src_h as f64) / (src_w as f64)).round() as u32;
            (w, h.max(1))
        }
        (0, h) => {
            let w = ((h as f64) * (src_w as f64) / (src_h as f64)).round() as u32;
            (w.max(1), h)
        }
        (w, h) => (w, h),
    };
    if w == 0 || h == 0 {
        return Err(
            "output size resolved to 0 pixels; set width and/or height to 1 or more".into(),
        );
    }
    if w > MAX_EDGE || h > MAX_EDGE {
        return Err(format!(
            "output {w}x{h} exceeds the {MAX_EDGE}px edge cap; set width/height to resize"
        ));
    }
    if (w as u64) * (h as u64) > MAX_PIXELS {
        return Err(format!(
            "output {w}x{h} is {} pixels, over the {MAX_PIXELS}-pixel cap; set width (and/or height) to resize first",
            (w as u64) * (h as u64)
        ));
    }
    Ok((w, h))
}

/// Decode, composite over the background, and resize to the target size.
fn prepare(bytes: &[u8], opts: &Options) -> Result<(RgbImage, u32, u32), String> {
    if bytes.is_empty() {
        return Err("empty image input: expected PNG, JPEG, GIF, BMP, or WebP bytes".into());
    }
    let img = image::load_from_memory(bytes).map_err(|e| {
        format!("could not decode image: {e} (expected PNG, JPEG, GIF, BMP, or WebP)")
    })?;
    let (src_w, src_h) = img.dimensions();
    let (w, h) = target_size(src_w, src_h, opts.width, opts.height)?;

    // Flatten alpha first so the resize filter never blends into transparency.
    let rgba = img.to_rgba8();
    let [br, bg, bb] = opts.background;
    let mut flat = RgbImage::new(src_w, src_h);
    for (x, y, px) in rgba.enumerate_pixels() {
        let a = px[3] as u32;
        let mix = |c: u8, b: u8| ((c as u32 * a + b as u32 * (255 - a)) / 255) as u8;
        flat.put_pixel(
            x,
            y,
            image::Rgb([mix(px[0], br), mix(px[1], bg), mix(px[2], bb)]),
        );
    }

    let resized = if (w, h) == (src_w, src_h) {
        flat
    } else {
        image::imageops::resize(&flat, w, h, FilterType::Lanczos3)
    };
    Ok((resized, src_w, src_h))
}

fn luma(p: &image::Rgb<u8>) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

fn pack_565(r: u8, g: u8, b: u8) -> u16 {
    ((r as u16 & 0xF8) << 8) | ((g as u16 & 0xFC) << 3) | (b as u16 >> 3)
}

/// Pack the 1-bit formats: one bit per pixel, each row padded to whole bytes.
/// A set bit is ink (a pixel darker than `threshold`), matching XBM and the
/// Adafruit GFX bitmap convention; `invert` flips it.
fn pack_1bit(img: &RgbImage, opts: &Options) -> Vec<u64> {
    let (w, h) = img.dimensions();
    let msb_first = opts.bit_order.resolve(opts.format);
    let row_bytes = w.div_ceil(8) as usize;
    let mut out = vec![0u8; row_bytes * h as usize];

    // Grayscale plane, optionally error-diffused before thresholding.
    let mut gray: Vec<f32> = img.pixels().map(luma).collect();
    let thr = opts.threshold as f32;
    for y in 0..h as usize {
        for x in 0..w as usize {
            let i = y * w as usize + x;
            let old = gray[i];
            let new = if old < thr { 0.0 } else { 255.0 };
            if opts.dither {
                let err = old - new;
                let mut spread = |dx: usize, dy: usize, right: bool, factor: f32| {
                    let nx = if right { x + dx } else { x.wrapping_sub(dx) };
                    let ny = y + dy;
                    if nx < w as usize && ny < h as usize {
                        gray[ny * w as usize + nx] += err * factor;
                    }
                };
                spread(1, 0, true, 7.0 / 16.0);
                spread(1, 1, false, 3.0 / 16.0);
                spread(0, 1, true, 5.0 / 16.0);
                spread(1, 1, true, 1.0 / 16.0);
            }
            gray[i] = new;
            let mut ink = new == 0.0;
            if opts.invert {
                ink = !ink;
            }
            if ink {
                let bit = if msb_first { 7 - (x % 8) } else { x % 8 };
                out[y * row_bytes + x / 8] |= 1 << bit;
            }
        }
    }
    out.into_iter().map(u64::from).collect()
}

/// Turn the prepared image into the array's numeric elements.
fn pack(img: &RgbImage, opts: &Options) -> Vec<u64> {
    match opts.format {
        PixelFormat::Mono | PixelFormat::Xbm => pack_1bit(img, opts),
        PixelFormat::Rgb565 | PixelFormat::Bgr565 => {
            let mut out = Vec::with_capacity(img.pixels().len() * 2);
            for p in img.pixels() {
                let v = if opts.format == PixelFormat::Rgb565 {
                    pack_565(p[0], p[1], p[2])
                } else {
                    pack_565(p[2], p[1], p[0])
                };
                match opts.byte_order {
                    ByteOrder::Word => out.push(v as u64),
                    ByteOrder::Big => {
                        out.push((v >> 8) as u64);
                        out.push((v & 0xFF) as u64);
                    }
                    ByteOrder::Little => {
                        out.push((v & 0xFF) as u64);
                        out.push((v >> 8) as u64);
                    }
                }
            }
            out
        }
        PixelFormat::Rgb888 => {
            let mut out = Vec::with_capacity(img.pixels().len() * 3);
            for p in img.pixels() {
                out.extend([p[0] as u64, p[1] as u64, p[2] as u64]);
            }
            out
        }
        PixelFormat::Rgb332 => img
            .pixels()
            .map(|p| ((p[0] as u64) & 0xE0) | ((p[1] as u64 & 0xE0) >> 3) | ((p[2] as u64) >> 6))
            .collect(),
        PixelFormat::Grayscale => img.pixels().map(|p| luma(p).round() as u64).collect(),
    }
}

/// Elements per emitted line: `per_line`, or one image row when it is 0.
fn line_width(opts: &Options, width: u32, elements: usize) -> usize {
    if opts.per_line > 0 {
        return opts.per_line as usize;
    }
    match opts.format {
        PixelFormat::Mono | PixelFormat::Xbm => width.div_ceil(8) as usize,
        PixelFormat::Rgb888 => width as usize * 3,
        PixelFormat::Rgb565 | PixelFormat::Bgr565 => match opts.byte_order {
            ByteOrder::Word => width as usize,
            _ => width as usize * 2,
        },
        _ => width as usize,
    }
    .clamp(1, elements.max(1))
}

fn render_number(v: u64, opts: &Options, wide: bool) -> String {
    match opts.number_format {
        NumberFormat::Decimal => v.to_string(),
        NumberFormat::Hex if wide => format!("0x{v:04X}"),
        NumberFormat::Hex => format!("0x{v:02X}"),
    }
}

fn render_body(
    values: &[u64],
    opts: &Options,
    wide: bool,
    per_line: usize,
    indent: &str,
) -> String {
    let mut s = String::new();
    for (i, chunk) in values.chunks(per_line).enumerate() {
        if i > 0 {
            s.push('\n');
        }
        s.push_str(indent);
        let last_chunk = (i + 1) * per_line >= values.len();
        for (j, v) in chunk.iter().enumerate() {
            s.push_str(&render_number(*v, opts, wide));
            if !(last_chunk && j + 1 == chunk.len()) {
                s.push(',');
                if j + 1 != chunk.len() {
                    s.push(' ');
                }
            }
        }
    }
    s
}

/// Convert an encoded image into embeddable C/C++ source.
pub fn convert(bytes: &[u8], opts: &Options) -> Result<Generated, String> {
    validate_name(&opts.name)?;
    if opts.per_line > MAX_PER_LINE {
        return Err(format!(
            "per_line {} is over the cap of {MAX_PER_LINE} (use 0 for one image row per line)",
            opts.per_line
        ));
    }

    let (img, src_w, src_h) = prepare(bytes, opts)?;
    let (w, h) = img.dimensions();
    let values = pack(&img, opts);

    let wide = opts.format.is_16bit() && opts.byte_order == ByteOrder::Word;
    let bytes_len = if wide { values.len() * 2 } else { values.len() };
    let data_type = if opts.format == PixelFormat::Xbm {
        "unsigned char"
    } else if wide {
        "uint16_t"
    } else {
        "uint8_t"
    };
    let per_line = line_width(opts, w, values.len());
    let body = render_body(&values, opts, wide, per_line, "  ");

    let name = &opts.name;
    let upper = name.to_ascii_uppercase();
    let order = match (opts.format.is_16bit(), opts.byte_order) {
        (true, ByteOrder::Word) => " as uint16_t words",
        (true, ByteOrder::Big) => " as big-endian bytes",
        (true, ByteOrder::Little) => " as little-endian bytes",
        _ => "",
    };
    let banner = format!(
        "// {name} — {w}x{h} {}{order}, {bytes_len} bytes ({} elements)",
        opts.format.label(),
        values.len()
    );

    let code = match opts.format {
        // XBM is a file format with its own fixed syntax, so `style` doesn't apply.
        PixelFormat::Xbm => format!(
            "#define {name}_width {w}\n#define {name}_height {h}\nstatic unsigned char {name}_bits[] = {{\n{body}\n}};\n"
        ),
        _ => match opts.style {
            Style::Raw => format!("{body}\n"),
            Style::Array => {
                format!("{banner}\nstatic const {data_type} {name}[] = {{\n{body}\n}};\n")
            }
            Style::Arduino => format!(
                "{banner}\n#include <pgmspace.h>\n\n#define {upper}_WIDTH  {w}\n#define {upper}_HEIGHT {h}\n\nstatic const {data_type} {name}[] PROGMEM = {{\n{body}\n}};\n"
            ),
            Style::Header => format!(
                "{banner}\n#ifndef {upper}_H\n#define {upper}_H\n\n#include <stdint.h>\n\n#define {upper}_WIDTH  {w}\n#define {upper}_HEIGHT {h}\n\nstatic const {data_type} {name}[] = {{\n{body}\n}};\n\n#endif // {upper}_H\n"
            ),
        },
    };

    Ok(Generated {
        code,
        width: w,
        height: h,
        src_width: src_w,
        src_height: src_h,
        bytes: bytes_len,
        elements: values.len(),
        data_type: data_type.to_string(),
        bits_per_pixel: opts.format.bits_per_pixel(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgb, Rgba, RgbaImage};

    /// 2x1 PNG: pure red, pure blue.
    fn png_2x1() -> Vec<u8> {
        let mut img = RgbImage::new(2, 1);
        img.put_pixel(0, 0, Rgb([255, 0, 0]));
        img.put_pixel(1, 0, Rgb([0, 0, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    /// 8x2 PNG: left half black, right half white.
    fn png_8x2() -> Vec<u8> {
        let mut img = RgbImage::new(8, 2);
        for y in 0..2 {
            for x in 0..8 {
                let v = if x < 4 { 0 } else { 255 };
                img.put_pixel(x, y, Rgb([v, v, v]));
            }
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn rgb565_words_are_exact() {
        let g = convert(&png_2x1(), &Options::default()).unwrap();
        // red = 0xF800, blue = 0x001F
        assert!(g.code.contains("0xF800, 0x001F"), "{}", g.code);
        assert!(g.code.contains("static const uint16_t image[] = {"));
        assert_eq!((g.width, g.height), (2, 1));
        assert_eq!(g.bytes, 4);
        assert_eq!(g.elements, 2);
        assert_eq!(g.data_type, "uint16_t");
        assert_eq!(g.bits_per_pixel, 16);
    }

    #[test]
    fn byte_order_splits_words_and_swaps_ends() {
        let big = convert(
            &png_2x1(),
            &Options {
                byte_order: ByteOrder::Big,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(big.code.contains("0xF8, 0x00, 0x00, 0x1F"), "{}", big.code);
        assert_eq!(big.data_type, "uint8_t");
        assert_eq!(big.elements, 4);
        assert_eq!(big.bytes, 4);

        let little = convert(
            &png_2x1(),
            &Options {
                byte_order: ByteOrder::Little,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            little.code.contains("0x00, 0xF8, 0x1F, 0x00"),
            "{}",
            little.code
        );
    }

    #[test]
    fn bgr565_swaps_red_and_blue() {
        let g = convert(
            &png_2x1(),
            &Options {
                format: PixelFormat::Bgr565,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(g.code.contains("0x001F, 0xF800"), "{}", g.code);
    }

    #[test]
    fn rgb888_emits_three_bytes_per_pixel() {
        let g = convert(
            &png_2x1(),
            &Options {
                format: PixelFormat::Rgb888,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            g.code.contains("0xFF, 0x00, 0x00, 0x00, 0x00, 0xFF"),
            "{}",
            g.code
        );
        assert_eq!(g.bytes, 6);
        assert_eq!(g.bits_per_pixel, 24);
    }

    #[test]
    fn rgb332_and_grayscale_are_one_byte_per_pixel() {
        let g332 = convert(
            &png_2x1(),
            &Options {
                format: PixelFormat::Rgb332,
                ..Default::default()
            },
        )
        .unwrap();
        // red -> 0xE0, blue -> 0x03
        assert!(g332.code.contains("0xE0, 0x03"), "{}", g332.code);
        assert_eq!(g332.bytes, 2);

        let gray = convert(
            &png_2x1(),
            &Options {
                format: PixelFormat::Grayscale,
                ..Default::default()
            },
        )
        .unwrap();
        // 0.299*255 = 76.2 -> 76 (0x4C); 0.114*255 = 29.07 -> 29 (0x1D)
        assert!(gray.code.contains("0x4C, 0x1D"), "{}", gray.code);
    }

    #[test]
    fn mono_packs_msb_first_and_xbm_lsb_first() {
        // Left half black (ink = 1), right half white.
        let mono = convert(
            &png_8x2(),
            &Options {
                format: PixelFormat::Mono,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(mono.code.contains("0xF0, 0xF0"), "{}", mono.code);
        assert_eq!(mono.bytes, 2);
        assert_eq!(mono.bits_per_pixel, 1);

        let xbm = convert(
            &png_8x2(),
            &Options {
                format: PixelFormat::Xbm,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(xbm.code.contains("0x0F, 0x0F"), "{}", xbm.code);
        assert!(xbm.code.contains("#define image_width 8"));
        assert!(xbm.code.contains("#define image_height 2"));
        assert!(xbm.code.contains("static unsigned char image_bits[] = {"));
    }

    #[test]
    fn bit_order_override_beats_the_per_format_default() {
        let g = convert(
            &png_8x2(),
            &Options {
                format: PixelFormat::Mono,
                bit_order: BitOrder::Lsb,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(g.code.contains("0x0F, 0x0F"), "{}", g.code);
    }

    #[test]
    fn invert_flips_the_ink_bits() {
        let g = convert(
            &png_8x2(),
            &Options {
                format: PixelFormat::Mono,
                invert: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(g.code.contains("0x0F, 0x0F"), "{}", g.code);
    }

    #[test]
    fn threshold_moves_the_black_white_cutoff() {
        // Mid grey (128) is ink at threshold 200, background at threshold 100.
        let mut img = RgbImage::new(8, 1);
        for x in 0..8 {
            img.put_pixel(x, 0, Rgb([128, 128, 128]));
        }
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let png = buf.into_inner();

        let dark = convert(
            &png,
            &Options {
                format: PixelFormat::Mono,
                threshold: 200,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(dark.code.contains("0xFF"), "{}", dark.code);

        let light = convert(
            &png,
            &Options {
                format: PixelFormat::Mono,
                threshold: 100,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(light.code.contains("0x00"), "{}", light.code);
    }

    #[test]
    fn dither_breaks_a_flat_grey_into_a_pattern() {
        // 100 sits clearly below the default 128 cutoff, so plain thresholding
        // makes every pixel ink; 128 would land exactly on the boundary.
        let mut img = RgbImage::new(8, 4);
        for y in 0..4 {
            for x in 0..8 {
                img.put_pixel(x, y, Rgb([100, 100, 100]));
            }
        }
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let png = buf.into_inner();

        let plain = convert(
            &png,
            &Options {
                format: PixelFormat::Mono,
                ..Default::default()
            },
        )
        .unwrap();
        let dithered = convert(
            &png,
            &Options {
                format: PixelFormat::Mono,
                dither: true,
                ..Default::default()
            },
        )
        .unwrap();
        // Flat grey thresholds to a solid block; dithering must not.
        assert!(
            plain.code.contains("0xFF, 0xFF, 0xFF, 0xFF"),
            "{}",
            plain.code
        );
        assert_ne!(plain.code, dithered.code);
    }

    #[test]
    fn width_only_resize_preserves_aspect_ratio() {
        let mut img = RgbImage::new(40, 20);
        for p in img.pixels_mut() {
            *p = Rgb([255, 255, 255]);
        }
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let g = convert(
            &buf.into_inner(),
            &Options {
                width: 10,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!((g.width, g.height), (10, 5));
        assert_eq!((g.src_width, g.src_height), (40, 20));
    }

    #[test]
    fn transparent_pixels_composite_over_the_background() {
        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 0]));
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let png = buf.into_inner();

        let on_white = convert(
            &png,
            &Options {
                background: [255, 255, 255],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(on_white.code.contains("0xFFFF"), "{}", on_white.code);

        let on_black = convert(&png, &Options::default()).unwrap();
        assert!(on_black.code.contains("0x0000"), "{}", on_black.code);
    }

    #[test]
    fn styles_wrap_the_same_numbers_differently() {
        let png = png_2x1();
        let raw = convert(
            &png,
            &Options {
                style: Style::Raw,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(raw.code.trim(), "0xF800, 0x001F");

        let arduino = convert(
            &png,
            &Options {
                style: Style::Arduino,
                name: "logo".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(arduino.code.contains("PROGMEM"));
        assert!(arduino.code.contains("#include <pgmspace.h>"));
        assert!(arduino.code.contains("#define LOGO_WIDTH  2"));

        let header = convert(
            &png,
            &Options {
                style: Style::Header,
                name: "logo".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(header.code.contains("#ifndef LOGO_H"));
        assert!(header.code.contains("#include <stdint.h>"));
        assert!(header.code.contains("#endif // LOGO_H"));
    }

    #[test]
    fn decimal_numbers_and_per_line_wrapping() {
        let png = png_2x1();
        let dec = convert(
            &png,
            &Options {
                number_format: NumberFormat::Decimal,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(dec.code.contains("63488, 31"), "{}", dec.code);

        let wrapped = convert(
            &png,
            &Options {
                per_line: 1,
                style: Style::Raw,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(wrapped.code.trim_end(), "  0xF800,\n  0x001F");
    }

    #[test]
    fn per_line_zero_puts_one_image_row_per_line() {
        let g = convert(
            &png_8x2(),
            &Options {
                format: PixelFormat::Grayscale,
                per_line: 0,
                style: Style::Raw,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(g.code.trim_end().lines().count(), 2);
    }

    #[test]
    fn parsers_accept_aliases_and_reject_junk() {
        assert_eq!(PixelFormat::parse("").unwrap(), PixelFormat::Rgb565);
        assert_eq!(PixelFormat::parse("XBM").unwrap(), PixelFormat::Xbm);
        assert_eq!(PixelFormat::parse("grey").unwrap(), PixelFormat::Grayscale);
        assert!(PixelFormat::parse("rgb555").is_err());
        assert_eq!(ByteOrder::parse("le").unwrap(), ByteOrder::Little);
        assert!(ByteOrder::parse("middle").is_err());
        assert_eq!(BitOrder::parse("").unwrap(), BitOrder::Auto);
        assert!(BitOrder::parse("wsb").is_err());
        assert_eq!(Style::parse("progmem").unwrap(), Style::Arduino);
        assert!(Style::parse("rust").is_err());
        assert_eq!(NumberFormat::parse("dec").unwrap(), NumberFormat::Decimal);
        assert!(NumberFormat::parse("octal").is_err());
        assert_eq!(parse_color("#fff").unwrap(), [255, 255, 255]);
        assert_eq!(parse_color("101820").unwrap(), [16, 24, 32]);
        assert_eq!(parse_color("white").unwrap(), [255, 255, 255]);
        assert!(parse_color("#gg").is_err());
        assert!(parse_color("#12345").is_err());
    }

    #[test]
    fn undecodable_input_is_a_clear_error() {
        let err = convert(b"not an image", &Options::default()).unwrap_err();
        assert!(err.contains("could not decode image"), "{err}");
        let empty = convert(&[], &Options::default()).unwrap_err();
        assert!(empty.contains("empty image input"), "{empty}");
    }

    #[test]
    fn bad_name_is_rejected_with_the_reason() {
        let err = convert(
            &png_2x1(),
            &Options {
                name: "2fast".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("must start with a letter"), "{err}");

        let err = convert(
            &png_2x1(),
            &Options {
                name: "my logo".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("not allowed in a C identifier"), "{err}");
    }

    #[test]
    fn oversized_output_is_rejected_with_a_hint() {
        let err = convert(
            &png_2x1(),
            &Options {
                width: 2000,
                height: 2000,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("pixel cap"), "{err}");

        let err = convert(
            &png_2x1(),
            &Options {
                width: 5000,
                height: 1,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("edge cap"), "{err}");
    }

    #[test]
    fn per_line_over_the_cap_is_rejected() {
        let err = convert(
            &png_2x1(),
            &Options {
                per_line: 999,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("over the cap"), "{err}");
    }
}
