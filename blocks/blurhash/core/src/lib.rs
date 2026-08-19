//! gizza-ai/blurhash core — turn an image into a compact BlurHash or ThumbHash
//! placeholder string, plus the decoded preview that string expands to.
//!
//! Both algorithms are DCT-based lossy summaries of an image, small enough to
//! ship inside a JSON payload and render before the real image arrives:
//!
//! * **BlurHash** (Wolt) — a base83 ASCII string, `1 + 1 + 4 + 2*(nx*ny - 1)`
//!   characters (28 for the usual 4x3 grid). Carries colour only; the aspect
//!   ratio must be stored alongside it.
//! * **ThumbHash** (Evan Wallace) — a ~21-25 byte binary hash (surfaced here as
//!   base64 and as hex) that also encodes the aspect ratio and an alpha channel.
//!
//! Pure Rust (no wafer/wasm-bindgen deps), so this runs on every backend
//! including the chat Service Worker.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use image::{imageops::FilterType, DynamicImage, ImageFormat, RgbaImage};
use std::io::Cursor;

/// The base83 alphabet BlurHash uses (`Algorithm.md`, woltapp/blurhash).
const BASE83: &[u8] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz#$%*+,-.:;=?@[]^_{|}~";

/// Refuse anything whose decoded raster (+ the encoded bytes we already hold)
/// would blow the 64 MiB wasm sandbox. Reported as an actionable error rather
/// than an opaque OOM trap.
const MAX_DECODE_BYTES: u64 = 40 * 1024 * 1024;

/// BlurHash runs its DCT on the image at NATIVE resolution while it fits this
/// pixel budget (2.5 MP covers 1080p and virtually every web image), because
/// the hash string is only comparable with other implementations when the input
/// raster is identical — pre-downscaling silently changes it (a 256x256 test
/// image moved 4 of its 28 characters, DC included). Beyond the budget the
/// image is scaled down to fit so the transform stays bounded in wasm; the
/// dimensions actually used are reported as `sampled_width`/`sampled_height`.
pub const BLURHASH_SAMPLE_MAX_PIXELS: u64 = 2_500_000;

/// ThumbHash's reference encoder requires each edge to be <= 100px.
pub const THUMBHASH_SAMPLE_MAX: u32 = 100;

/// Which placeholder format to produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    BlurHash,
    ThumbHash,
}

impl Algorithm {
    /// Parse the `algorithm` argument (case/`-`/`_`/space insensitive).
    pub fn parse(s: &str) -> Result<Self, String> {
        let norm: String = s
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(|c| !matches!(c, '-' | '_' | ' '))
            .collect();
        match norm.as_str() {
            "blurhash" => Ok(Algorithm::BlurHash),
            "thumbhash" => Ok(Algorithm::ThumbHash),
            _ => Err(format!(
                "unknown algorithm '{}' — use \"blurhash\" or \"thumbhash\"",
                s.trim()
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Algorithm::BlurHash => "blurhash",
            Algorithm::ThumbHash => "thumbhash",
        }
    }
}

/// Encoding options. `components_x`/`components_y`/`punch` only affect BlurHash.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub algorithm: Algorithm,
    /// Horizontal DCT components, 1-9 (BlurHash only).
    pub components_x: u32,
    /// Vertical DCT components, 1-9 (BlurHash only).
    pub components_y: u32,
    /// Contrast multiplier applied when decoding the preview (BlurHash only).
    pub punch: f32,
    /// Longest edge of the rendered preview, in pixels.
    pub preview_size: u32,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            algorithm: Algorithm::BlurHash,
            components_x: 4,
            components_y: 3,
            punch: 1.0,
            preview_size: 64,
        }
    }
}

/// One resolved colour in the notations a front-end actually pastes.
#[derive(Debug, Clone)]
pub struct Color {
    pub hex: String,
    pub rgb: String,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Everything the tool reports about one encoded image.
#[derive(Debug, Clone)]
pub struct Placeholder {
    pub algorithm: &'static str,
    /// The placeholder string: base83 for BlurHash, base64 for ThumbHash.
    pub hash: String,
    /// Hex form of the ThumbHash bytes (`None` for BlurHash, which is text).
    pub hash_hex: Option<String>,
    /// Characters in `hash`.
    pub hash_length: usize,
    /// Bytes you actually have to store (BlurHash: ASCII chars; ThumbHash: binary).
    pub hash_bytes: usize,
    /// DCT grid actually used (`None` for ThumbHash, which derives its own).
    pub components_x: Option<u32>,
    pub components_y: Option<u32>,
    /// Source image dimensions.
    pub width: u32,
    pub height: u32,
    /// `width / height`, rounded to 4 decimals — BlurHash does NOT store this.
    pub aspect_ratio: f32,
    /// Detected source format, e.g. `png`.
    pub source_format: String,
    /// Dimensions the DCT was actually sampled at (after the internal downscale).
    pub sampled_width: u32,
    pub sampled_height: u32,
    /// The hash's own average colour — a solid one-pixel fallback.
    pub average_color: Color,
    /// True when the average colour is dark (put white text/spinners on top).
    pub is_dark: bool,
    /// Rendered preview of what the hash decodes to.
    pub preview_width: u32,
    pub preview_height: u32,
    /// `data:image/png;base64,...` — usable directly as an `<img src>`.
    pub preview_data_url: String,
    /// Ready-to-paste CSS that shows the preview until the real image loads.
    pub css_background_image: String,
}

/// Encode `bytes` (an encoded PNG/JPEG/WebP/GIF/BMP image) to a placeholder.
pub fn encode(bytes: &[u8], opts: &Options) -> Result<Placeholder, String> {
    if bytes.is_empty() {
        return Err("the image is empty (0 bytes)".to_string());
    }
    if !(1..=9).contains(&opts.components_x) || !(1..=9).contains(&opts.components_y) {
        return Err(format!(
            "components_x/components_y must each be between 1 and 9 (got {}x{})",
            opts.components_x, opts.components_y
        ));
    }
    if !opts.punch.is_finite() || opts.punch <= 0.0 || opts.punch > 10.0 {
        return Err(format!(
            "punch must be greater than 0 and at most 10 (got {})",
            opts.punch
        ));
    }
    if !(8..=512).contains(&opts.preview_size) {
        return Err(format!(
            "preview_size must be between 8 and 512 pixels (got {})",
            opts.preview_size
        ));
    }

    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("could not read the image: {e}"))?;
    let source_format = reader
        .format()
        .and_then(|f| f.extensions_str().first().copied())
        .unwrap_or("unknown")
        .to_string();
    let (width, height) = reader.into_dimensions().map_err(|e| {
        format!("not a supported image (expected PNG, JPEG, WebP, GIF or BMP): {e}")
    })?;
    if width == 0 || height == 0 {
        return Err("the image has a zero width or height".to_string());
    }
    let budget = width as u64 * height as u64 * 4 + bytes.len() as u64;
    if budget > MAX_DECODE_BYTES {
        return Err(format!(
            "image is too large to decode in the sandbox ({width}x{height} needs ~{} MB, the limit is {} MB) — re-export it at a lower resolution",
            budget / (1024 * 1024),
            MAX_DECODE_BYTES / (1024 * 1024)
        ));
    }

    let img =
        image::load_from_memory(bytes).map_err(|e| format!("could not decode the image: {e}"))?;

    let (sampled_width, sampled_height) = match opts.algorithm {
        Algorithm::BlurHash => fit_pixels(width, height, BLURHASH_SAMPLE_MAX_PIXELS),
        Algorithm::ThumbHash => fit(width, height, THUMBHASH_SAMPLE_MAX),
    };
    let rgba = img.to_rgba8();
    let sample: RgbaImage = if (sampled_width, sampled_height) == (width, height) {
        rgba
    } else {
        image::imageops::resize(&rgba, sampled_width, sampled_height, FilterType::Triangle)
    };

    let (preview_width, preview_height) = fit_up(width, height, opts.preview_size);

    let (hash, hash_hex, hash_bytes, components, preview, average) = match opts.algorithm {
        Algorithm::BlurHash => {
            let hash = blurhash::encode(
                opts.components_x,
                opts.components_y,
                sampled_width,
                sampled_height,
                sample.as_raw(),
            )
            .map_err(|e| format!("blurhash encode failed: {e}"))?;
            let pixels = blurhash::decode(&hash, preview_width, preview_height, opts.punch)
                .map_err(|e| format!("blurhash preview decode failed: {e}"))?;
            let preview = RgbaImage::from_raw(preview_width, preview_height, pixels)
                .ok_or_else(|| "blurhash preview has an unexpected pixel count".to_string())?;
            let dc = base83_decode(&hash[2..6])?;
            let average = rgb_color(
                ((dc >> 16) & 0xff) as u8,
                ((dc >> 8) & 0xff) as u8,
                (dc & 0xff) as u8,
                255,
            );
            let len = hash.len();
            (
                hash,
                None,
                len,
                (Some(opts.components_x), Some(opts.components_y)),
                preview,
                average,
            )
        }
        Algorithm::ThumbHash => {
            // The reference encoder asserts on >100px edges; `fit` already
            // guarantees that, but keep the check so a future change traps here
            // with a message instead of panicking inside the crate.
            if sampled_width > THUMBHASH_SAMPLE_MAX || sampled_height > THUMBHASH_SAMPLE_MAX {
                return Err("internal error: thumbhash sample exceeds 100px".to_string());
            }
            let raw = thumbhash::rgba_to_thumb_hash(
                sampled_width as usize,
                sampled_height as usize,
                sample.as_raw(),
            );
            let (tw, th, pixels) = thumbhash::thumb_hash_to_rgba(&raw)
                .map_err(|_| "thumbhash preview decode failed".to_string())?;
            let small = RgbaImage::from_raw(tw as u32, th as u32, pixels)
                .ok_or_else(|| "thumbhash preview has an unexpected pixel count".to_string())?;
            let preview = image::imageops::resize(
                &small,
                preview_width,
                preview_height,
                FilterType::CatmullRom,
            );
            let (r, g, b, a) = thumbhash::thumb_hash_to_average_rgba(&raw)
                .map_err(|_| "thumbhash average colour decode failed".to_string())?;
            let average = rgb_color(to_u8(r), to_u8(g), to_u8(b), to_u8(a));
            let hex: String = raw.iter().map(|b| format!("{b:02x}")).collect();
            let hash = B64.encode(&raw);
            (hash, Some(hex), raw.len(), (None, None), preview, average)
        }
    };

    let mut png = Vec::new();
    DynamicImage::ImageRgba8(preview)
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|e| format!("could not encode the preview PNG: {e}"))?;
    let preview_data_url = format!("data:image/png;base64,{}", B64.encode(&png));

    let luma = 0.299 * average.r as f32 + 0.587 * average.g as f32 + 0.114 * average.b as f32;

    Ok(Placeholder {
        algorithm: opts.algorithm.as_str(),
        hash_length: hash.chars().count(),
        hash_bytes,
        hash,
        hash_hex,
        components_x: components.0,
        components_y: components.1,
        width,
        height,
        aspect_ratio: round4(width as f32 / height as f32),
        source_format,
        sampled_width,
        sampled_height,
        average_color: average,
        is_dark: luma < 128.0,
        preview_width,
        preview_height,
        css_background_image: format!(
            "background-image: url(\"{preview_data_url}\"); background-size: cover; background-position: center;"
        ),
        preview_data_url,
    })
}

fn rgb_color(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color {
        hex: format!("#{r:02x}{g:02x}{b:02x}"),
        rgb: format!("rgb({r}, {g}, {b})"),
        r,
        g,
        b,
        a,
    }
}

fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn round4(v: f32) -> f32 {
    (v * 10_000.0).round() / 10_000.0
}

/// Shrink `w`x`h` so its long edge is at most `max_edge` (never enlarges).
fn fit(w: u32, h: u32, max_edge: u32) -> (u32, u32) {
    if w.max(h) <= max_edge {
        return (w, h);
    }
    scale_to(w, h, max_edge)
}

/// Shrink `w`x`h` so total pixels are at most `max_pixels` (never enlarges).
fn fit_pixels(w: u32, h: u32, max_pixels: u64) -> (u32, u32) {
    let pixels = w as u64 * h as u64;
    if pixels <= max_pixels {
        return (w, h);
    }
    let scale = (max_pixels as f64 / pixels as f64).sqrt();
    let nw = ((w as f64 * scale).round() as u32).max(1);
    let nh = ((h as f64 * scale).round() as u32).max(1);
    (nw, nh)
}

/// Scale `w`x`h` so its long edge is exactly `max_edge` (may enlarge — the
/// preview is rendered at whatever size was asked for).
fn fit_up(w: u32, h: u32, max_edge: u32) -> (u32, u32) {
    scale_to(w, h, max_edge)
}

fn scale_to(w: u32, h: u32, max_edge: u32) -> (u32, u32) {
    let long = w.max(h) as f64;
    let s = max_edge as f64 / long;
    let nw = ((w as f64 * s).round() as u32).max(1);
    let nh = ((h as f64 * s).round() as u32).max(1);
    (nw, nh)
}

/// Decode a base83 run (BlurHash's own alphabet) into an integer.
fn base83_decode(s: &str) -> Result<u32, String> {
    let mut value: u32 = 0;
    for c in s.bytes() {
        let digit = BASE83
            .iter()
            .position(|&b| b == c)
            .ok_or_else(|| format!("invalid base83 character '{}'", c as char))?;
        value = value
            .checked_mul(83)
            .and_then(|v| v.checked_add(digit as u32))
            .ok_or_else(|| "base83 value overflowed".to_string())?;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An encoded PNG of a single solid colour, `w` x `h`.
    fn solid_png(w: u32, h: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, image::Rgba([r, g, b, 255]));
        let mut out = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }
    #[test]
    fn solid_red_matches_the_reference_encoder_for_the_decoded_pixels() {
        let png = solid_png(32, 24, 255, 0, 0);
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        let expected = blurhash::encode(4, 3, 32, 24, decoded.as_raw()).unwrap();
        let out = encode(&png, &Options::default()).unwrap();
        assert_eq!(out.hash, expected);
        assert_eq!(out.hash_length, 28);
        assert!(out.average_color.r >= 250);
        assert!(out.average_color.g <= 2);
        assert!(out.average_color.b <= 2);
        assert_eq!(out.width, 32);
        assert_eq!(out.height, 24);
        assert!(out.is_dark);
        assert!(out.preview_data_url.starts_with("data:image/png;base64,"));
        assert_eq!((out.preview_width, out.preview_height), (64, 48));
    }

    #[test]
    fn hash_length_follows_the_component_grid() {
        let png = solid_png(20, 20, 10, 120, 200);
        for (cx, cy) in [(1, 1), (3, 3), (9, 9), (6, 2)] {
            let out = encode(
                &png,
                &Options {
                    components_x: cx,
                    components_y: cy,
                    ..Options::default()
                },
            )
            .unwrap();
            assert_eq!(
                out.hash_length,
                6 + 2 * (cx as usize * cy as usize - 1),
                "{cx}x{cy}"
            );
            assert_eq!(out.average_color.hex, "#0a78c8");
            assert!(out.is_dark);
        }
    }

    #[test]
    fn thumbhash_round_trips_to_the_source_colour() {
        let png = solid_png(64, 32, 20, 160, 90);
        let out = encode(
            &png,
            &Options {
                algorithm: Algorithm::ThumbHash,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(out.algorithm, "thumbhash");
        assert!(out.components_x.is_none());
        // ThumbHash is binary: the string form is base64 and round-trips.
        let raw = B64.decode(out.hash.as_bytes()).unwrap();
        assert_eq!(raw.len(), out.hash_bytes);
        assert!(raw.len() <= 25, "thumbhash is ~25 bytes, got {}", raw.len());
        assert_eq!(out.hash_hex.unwrap().len(), raw.len() * 2);
        // The average colour survives the round trip to within quantisation.
        let c = &out.average_color;
        assert!(
            (c.r as i32 - 20).abs() <= 12
                && (c.g as i32 - 160).abs() <= 12
                && (c.b as i32 - 90).abs() <= 12,
            "average {} drifted too far from rgb(20, 160, 90)",
            c.hex
        );
        // ThumbHash stores the aspect ratio, so a 2:1 source stays 2:1.
        assert_eq!(out.aspect_ratio, 2.0);
        assert_eq!((out.preview_width, out.preview_height), (64, 32));
    }

    #[test]
    fn sampling_limits_match_each_algorithm() {
        let png = solid_png(400, 100, 200, 200, 200);
        let bh = encode(&png, &Options::default()).unwrap();
        assert_eq!((bh.sampled_width, bh.sampled_height), (400, 100));
        assert_eq!(
            fit_pixels(4_000, 1_000, BLURHASH_SAMPLE_MAX_PIXELS),
            (3162, 791)
        );
        let th = encode(
            &png,
            &Options {
                algorithm: Algorithm::ThumbHash,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!((th.sampled_width, th.sampled_height), (100, 25));
    }

    #[test]
    fn rejects_bytes_that_are_not_an_image() {
        let err = encode(b"this is not an image at all", &Options::default()).unwrap_err();
        assert!(
            err.contains("not a supported image") || err.contains("could not read"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_out_of_range_options() {
        let png = solid_png(8, 8, 1, 2, 3);
        let err = encode(
            &png,
            &Options {
                components_x: 12,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("between 1 and 9"), "unexpected error: {err}");

        let err = encode(
            &png,
            &Options {
                punch: 0.0,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("punch"), "unexpected error: {err}");

        let err = encode(
            &png,
            &Options {
                preview_size: 4,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("preview_size"), "unexpected error: {err}");
    }

    #[test]
    fn algorithm_parsing_is_forgiving_but_bounded() {
        assert_eq!(Algorithm::parse("BlurHash").unwrap(), Algorithm::BlurHash);
        assert_eq!(
            Algorithm::parse(" thumb-hash ").unwrap(),
            Algorithm::ThumbHash
        );
        assert!(Algorithm::parse("blurhash2")
            .unwrap_err()
            .contains("unknown algorithm"));
    }

    #[test]
    fn base83_decodes_the_reference_alphabet() {
        assert_eq!(base83_decode("0").unwrap(), 0);
        assert_eq!(base83_decode("L").unwrap(), 21);
        assert_eq!(base83_decode("TI:j").unwrap(), 0x00ff_0000);
        assert!(base83_decode("!!").is_err());
    }
}
