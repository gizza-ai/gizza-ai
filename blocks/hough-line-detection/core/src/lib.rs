//! gizza-ai/hough-line-detection core — find the straight lines in an image with
//! the Hough transform and either report their geometry or draw them back over
//! the picture.
//!
//! No wafer/wasm-bindgen deps. Pure-Rust `image` crate for decode/encode; every
//! other stage is hand-rolled so the block stays wasmi-safe (`imageproc` and
//! `fast_image_resize` bake `v128` SIMD that wafer's runtime rejects — the same
//! reason document-scan / collage-splitter avoid them — and `imageproc::hough`
//! only returns infinite polar lines, not the segments this tool is about).
//!
//! Pipeline:
//!   1. header-first memory budget → decode → integer box-downscale so the
//!      longest side is at most `MAX_SIDE` (keeps a 25-MP scan inside the
//!      64 MiB sandbox); the downscale factor `k` is reported so every
//!      coordinate can be mapped back to ORIGINAL image pixels;
//!   2. Gaussian pre-blur → Sobel gradients → non-maximum suppression →
//!      hysteresis = a hand-rolled Canny edge map (thresholds default to
//!      automatic: Otsu over the suppressed gradient, low = 0.4 × high);
//!   3. classic (rho, theta) accumulator over the edge pixels, theta in
//!      [0, 180) so every line is represented exactly once;
//!   4. non-maximum suppression in accumulator space (9 px of rho, 5° of theta,
//!      wrapping across the 0/180 seam so a vertical line peaks once);
//!   5. `segments` mode walks each peak's line through the edge map, splitting
//!      it on gaps larger than `max_line_gap` and keeping runs at least
//!      `min_line_length` long (the HoughLinesP answer shape: real endpoints),
//!      consuming the ink it used so two near-identical peaks cannot report the
//!      same edge twice; `lines` mode reports the accumulator peak itself as an
//!      infinite line clipped to the image rectangle.
//!
//! Angle convention (matches document-skew-detector / image-horizon-tilt-checker):
//! `angle_degrees` is the segment's tilt away from horizontal in (-90, 90],
//! POSITIVE when the right-hand end sits LOWER on screen (clockwise); 90 is a
//! vertical line.

use std::io::Cursor;

use image::{
    codecs::jpeg::JpegEncoder, codecs::png::PngEncoder, ColorType, DynamicImage, ExtendedColorType,
    GrayImage, ImageDecoder, ImageEncoder, ImageReader, RgbImage,
};

// ---------------------------------------------------------------------------
// Tunables
// ---------------------------------------------------------------------------

/// Longest side after downscale. Bounds both memory and the accumulator work
/// (the transform is O(edge pixels × theta bins) and runs in an interpreter).
pub const MAX_SIDE: u32 = 1024;
/// Decode-memory budget: input bytes + decoded raster (+ any full-size copy)
/// must fit alongside the runtime in the 64 MiB wasm sandbox.
const MEM_BUDGET: u64 = 48 * 1024 * 1024;
/// Largest Sobel L2 gradient magnitude on an 8-bit image: 4·255·√2.
const MAX_GRADIENT: f32 = 1442.497;
/// Accumulator suppression window along rho, in analysis pixels.
const NMS_RHO_PX: f64 = 9.0;
/// Accumulator suppression window along theta, in degrees.
const NMS_THETA_DEG: f64 = 5.0;
/// A line within this many degrees of an axis is called horizontal / vertical.
pub const AXIS_TOLERANCE_DEG: f64 = 10.0;
/// Automatic `min_line_length` = this fraction of the image diagonal…
const AUTO_MIN_LEN_FRAC: f64 = 0.08;
/// …with this floor, in original pixels.
const AUTO_MIN_LEN_FLOOR: f64 = 20.0;
/// Automatic `max_line_gap` = this fraction of the image diagonal…
const AUTO_GAP_FRAC: f64 = 0.015;
/// …with this floor, in original pixels.
const AUTO_GAP_FLOOR: f64 = 3.0;
/// Automatic vote `threshold` = this fraction of the minimum line length…
const AUTO_THRESHOLD_FRAC: f64 = 0.6;
/// …with this floor, in votes.
const AUTO_THRESHOLD_FLOOR: u32 = 16;
/// Automatic Canny low threshold as a fraction of the high one.
const AUTO_LOW_RATIO: f64 = 0.4;
/// Never let the automatic high threshold collapse onto flat noise.
const AUTO_HIGH_FLOOR: f64 = 0.02;
/// Warn above this many edge pixels — the accumulator pass gets slow and the
/// peak list gets noisy.
const BUSY_EDGE_PIXELS: u64 = 250_000;

/// Accepted `mode` values.
pub const MODES: [&str; 2] = ["segments", "lines"];
/// Accepted `orientation` values.
pub const ORIENTATIONS: [&str; 3] = ["any", "horizontal", "vertical"];
/// Accepted `output` values.
pub const OUTPUTS: [&str; 2] = ["report", "overlay"];
/// Accepted `overlay_background` values.
pub const BACKGROUNDS: [&str; 4] = ["original", "edges", "black", "white"];
/// Accepted overlay `format` values.
pub const FORMATS: [&str; 2] = ["png", "jpg"];

pub const DEFAULT_MODE: &str = "segments";
pub const DEFAULT_ORIENTATION: &str = "any";
pub const DEFAULT_OUTPUT: &str = "report";
pub const DEFAULT_BACKGROUND: &str = "original";
pub const DEFAULT_FORMAT: &str = "png";
pub const DEFAULT_BLUR: f64 = 1.0;
pub const DEFAULT_ANGLE_RESOLUTION: f64 = 1.0;
pub const DEFAULT_RHO_RESOLUTION: f64 = 1.0;
pub const DEFAULT_MAX_LINES: i64 = 50;
pub const DEFAULT_COLOR: &str = "#ff0000";

pub const MAX_BLUR: f64 = 5.0;
pub const MAX_LINES_CAP: i64 = 500;
pub const MAX_LINE_WIDTH: i64 = 20;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Segments,
    Lines,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Any,
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Report,
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Background {
    Original,
    Edges,
    Black,
    White,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpg,
}

impl Format {
    pub fn mime(self) -> &'static str {
        match self {
            Format::Png => "image/png",
            Format::Jpg => "image/jpeg",
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpg => "jpg",
        }
    }
}

macro_rules! parse_enum {
    ($fn_name:ident, $ty:ty, $label:literal, $( $s:literal => $v:expr ),+ $(,)?) => {
        pub fn $fn_name(v: Option<&str>) -> Result<$ty, String> {
            match v.map(str::trim).filter(|s| !s.is_empty()) {
                None => Ok(default_of!($( $v ),+)),
                $( Some($s) => Ok($v), )+
                Some(other) => Err(format!(
                    concat!($label, " must be one of {}, got {:?}"),
                    [$( $s ),+].join(", "),
                    other
                )),
            }
        }
    };
}
macro_rules! default_of {
    ($first:expr $(, $rest:expr )*) => { $first };
}

parse_enum!(parse_mode, Mode, "mode", "segments" => Mode::Segments, "lines" => Mode::Lines);
parse_enum!(
    parse_orientation, Orientation, "orientation",
    "any" => Orientation::Any,
    "horizontal" => Orientation::Horizontal,
    "vertical" => Orientation::Vertical,
);
parse_enum!(parse_output, Output, "output", "report" => Output::Report, "overlay" => Output::Overlay);
parse_enum!(
    parse_background, Background, "overlay_background",
    "original" => Background::Original,
    "edges" => Background::Edges,
    "black" => Background::Black,
    "white" => Background::White,
);
parse_enum!(parse_format, Format, "format", "png" => Format::Png, "jpg" => Format::Jpg);

/// Every knob, already validated. Build one with [`Options::default`] and
/// override what the caller supplied.
#[derive(Debug, Clone)]
pub struct Options {
    pub mode: Mode,
    /// Canny lower hysteresis threshold, 0-1 fraction of the maximum possible
    /// gradient. 0 = automatic.
    pub canny_low: f64,
    /// Canny upper hysteresis threshold, 0-1. 0 = automatic (Otsu).
    pub canny_high: f64,
    /// Gaussian pre-blur sigma in analysis pixels, 0 = off.
    pub blur: f64,
    /// Minimum accumulator votes. 0 = automatic.
    pub threshold: u32,
    /// Minimum segment length in ORIGINAL image pixels. 0 = automatic.
    pub min_line_length: f64,
    /// Largest tolerated break inside one segment, ORIGINAL pixels. 0 = auto.
    pub max_line_gap: f64,
    /// Accumulator angle step, degrees.
    pub angle_resolution: f64,
    /// Accumulator distance step, analysis pixels.
    pub rho_resolution: f64,
    /// Cap on how many lines are returned.
    pub max_lines: u32,
    pub orientation: Orientation,
    pub output: Output,
    /// Overlay line color (`#rgb`, `#rrggbb` or a common color name).
    pub color: String,
    /// Overlay line thickness in analysis pixels. 0 = automatic.
    pub line_width: u32,
    pub overlay_background: Background,
    pub format: Format,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: Mode::Segments,
            canny_low: 0.0,
            canny_high: 0.0,
            blur: DEFAULT_BLUR,
            threshold: 0,
            min_line_length: 0.0,
            max_line_gap: 0.0,
            angle_resolution: DEFAULT_ANGLE_RESOLUTION,
            rho_resolution: DEFAULT_RHO_RESOLUTION,
            max_lines: DEFAULT_MAX_LINES as u32,
            orientation: Orientation::Any,
            output: Output::Report,
            color: DEFAULT_COLOR.to_string(),
            line_width: 0,
            overlay_background: Background::Original,
            format: Format::Png,
        }
    }
}

impl Options {
    fn validate(&self) -> Result<(), String> {
        range("canny_low", self.canny_low, 0.0, 1.0)?;
        range("canny_high", self.canny_high, 0.0, 1.0)?;
        range("blur", self.blur, 0.0, MAX_BLUR)?;
        range("min_line_length", self.min_line_length, 0.0, 20000.0)?;
        range("max_line_gap", self.max_line_gap, 0.0, 2000.0)?;
        range("angle_resolution", self.angle_resolution, 0.1, 5.0)?;
        range("rho_resolution", self.rho_resolution, 0.5, 20.0)?;
        if self.max_lines < 1 || i64::from(self.max_lines) > MAX_LINES_CAP {
            return Err(format!(
                "max_lines must be between 1 and {MAX_LINES_CAP}, got {}",
                self.max_lines
            ));
        }
        if i64::from(self.line_width) > MAX_LINE_WIDTH {
            return Err(format!(
                "line_width must be between 0 (automatic) and {MAX_LINE_WIDTH}, got {}",
                self.line_width
            ));
        }
        if self.canny_low > 0.0 && self.canny_high > 0.0 && self.canny_low > self.canny_high {
            return Err(format!(
                "canny_low ({}) must not exceed canny_high ({})",
                self.canny_low, self.canny_high
            ));
        }
        parse_color(&self.color)?;
        Ok(())
    }
}

fn range(name: &str, v: f64, lo: f64, hi: f64) -> Result<(), String> {
    if !v.is_finite() || v < lo || v > hi {
        return Err(format!("{name} must be between {lo} and {hi}, got {v}"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// One detected line, in ORIGINAL image pixel coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub x1: i64,
    pub y1: i64,
    pub x2: i64,
    pub y2: i64,
    /// Endpoint distance in original pixels.
    pub length: f64,
    /// Tilt from horizontal in (-90, 90]; positive = right end lower.
    pub angle_degrees: f64,
    /// Hough radius in original pixels (signed distance from the origin to the
    /// line along its normal).
    pub rho: f64,
    /// Hough normal angle in [0, 180).
    pub theta_degrees: f64,
    /// Accumulator votes behind this line's peak.
    pub votes: u32,
    /// "horizontal", "vertical" or "diagonal".
    pub orientation: &'static str,
}

/// The full detection report.
#[derive(Debug, Clone, PartialEq)]
pub struct Detection {
    /// Original image size.
    pub width: u32,
    pub height: u32,
    /// Size the transform actually ran at (after the integer box downscale).
    pub analysis_width: u32,
    pub analysis_height: u32,
    /// Original pixels per analysis pixel (1 when the image was not downscaled).
    pub downscale_factor: u32,
    pub edge_pixels: u64,
    /// Effective Canny thresholds (0-1 fractions), after `auto` resolution.
    pub canny_low_used: f64,
    pub canny_high_used: f64,
    /// Effective vote threshold, after `auto` resolution.
    pub threshold_used: u32,
    /// Effective lengths in ORIGINAL pixels, after `auto` resolution.
    pub min_line_length_used: f64,
    pub max_line_gap_used: f64,
    pub line_count: usize,
    pub lines: Vec<Line>,
    /// Angle of the longest detected line, if any.
    pub dominant_angle_degrees: Option<f64>,
    pub horizontal_count: usize,
    pub vertical_count: usize,
    pub diagonal_count: usize,
    pub warnings: Vec<String>,
}

/// What [`detect`] produced: always a report, plus an encoded overlay image
/// when `output = overlay`.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub detection: Detection,
    /// `(bytes, width, height)` of the rendered overlay, at ANALYSIS size.
    pub overlay: Option<(Vec<u8>, u32, u32)>,
}

fn round2(x: f64) -> f64 {
    let r = (x * 100.0).round() / 100.0;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

fn round4(x: f64) -> f64 {
    let r = (x * 10_000.0).round() / 10_000.0;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

// ---------------------------------------------------------------------------
// Color
// ---------------------------------------------------------------------------

/// Parse `#rgb`, `#rrggbb` (with or without the `#`) or a common color name.
pub fn parse_color(s: &str) -> Result<[u8; 3], String> {
    let t = s.trim();
    let named: &[(&str, [u8; 3])] = &[
        ("red", [255, 0, 0]),
        ("green", [0, 128, 0]),
        ("lime", [0, 255, 0]),
        ("blue", [0, 0, 255]),
        ("yellow", [255, 255, 0]),
        ("cyan", [0, 255, 255]),
        ("magenta", [255, 0, 255]),
        ("orange", [255, 165, 0]),
        ("white", [255, 255, 255]),
        ("black", [0, 0, 0]),
        ("gray", [128, 128, 128]),
        ("grey", [128, 128, 128]),
    ];
    let lower = t.to_ascii_lowercase();
    if let Some((_, rgb)) = named.iter().find(|(n, _)| *n == lower) {
        return Ok(*rgb);
    }
    let hex = lower.strip_prefix('#').unwrap_or(&lower);
    let ok = !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit());
    let nib = |c: u8| -> u8 { (c as char).to_digit(16).unwrap() as u8 };
    if ok && hex.len() == 3 {
        let b = hex.as_bytes();
        return Ok([nib(b[0]) * 17, nib(b[1]) * 17, nib(b[2]) * 17]);
    }
    if ok && hex.len() == 6 {
        let b = hex.as_bytes();
        return Ok([
            nib(b[0]) * 16 + nib(b[1]),
            nib(b[2]) * 16 + nib(b[3]),
            nib(b[4]) * 16 + nib(b[5]),
        ]);
    }
    Err(format!(
        "color must be #rgb, #rrggbb or a name (red, green, lime, blue, yellow, cyan, magenta, \
         orange, white, black, gray), got {t:?}"
    ))
}

// ---------------------------------------------------------------------------
// Decode + downscale
// ---------------------------------------------------------------------------

/// Streaming luma conversion + integer box downscale from a raw 8-bit
/// interleaved buffer. Allocates only the output plus one row of accumulators.
fn gray_box_downscale(raw: &[u8], w: u32, h: u32, ch: usize, k: u32) -> GrayImage {
    let ow = (w / k).max(1);
    let oh = (h / k).max(1);
    let mut out = GrayImage::new(ow, oh);
    let ks = k as usize;
    let stride = w as usize * ch;
    let mut acc = vec![0u32; ow as usize];
    for oy in 0..oh as usize {
        acc.fill(0);
        for iy in oy * ks..(oy + 1) * ks {
            let row = &raw[iy * stride..(iy + 1) * stride];
            for ox in 0..ow as usize {
                let mut sum = 0u32;
                for ix in ox * ks..(ox + 1) * ks {
                    let p = &row[ix * ch..];
                    sum += if ch >= 3 {
                        (299 * u32::from(p[0]) + 587 * u32::from(p[1]) + 114 * u32::from(p[2]))
                            / 1000
                    } else {
                        u32::from(p[0])
                    };
                }
                acc[ox] += sum;
            }
        }
        let norm = (ks * ks) as u32;
        for ox in 0..ow as usize {
            out.put_pixel(ox as u32, oy as u32, image::Luma([(acc[ox] / norm) as u8]));
        }
    }
    out
}

/// Same box downscale, keeping color — only built when an `original`-background
/// overlay is actually requested.
fn rgb_box_downscale(raw: &[u8], w: u32, h: u32, ch: usize, k: u32) -> RgbImage {
    let ow = (w / k).max(1);
    let oh = (h / k).max(1);
    let mut out = RgbImage::new(ow, oh);
    let ks = k as usize;
    let stride = w as usize * ch;
    let mut acc = vec![[0u32; 3]; ow as usize];
    for oy in 0..oh as usize {
        acc.fill([0; 3]);
        for iy in oy * ks..(oy + 1) * ks {
            let row = &raw[iy * stride..(iy + 1) * stride];
            for ox in 0..ow as usize {
                for ix in ox * ks..(ox + 1) * ks {
                    let p = &row[ix * ch..];
                    let (r, g, b) = if ch >= 3 {
                        (u32::from(p[0]), u32::from(p[1]), u32::from(p[2]))
                    } else {
                        let v = u32::from(p[0]);
                        (v, v, v)
                    };
                    acc[ox][0] += r;
                    acc[ox][1] += g;
                    acc[ox][2] += b;
                }
            }
        }
        let norm = (ks * ks) as u32;
        for ox in 0..ow as usize {
            let a = acc[ox];
            out.put_pixel(
                ox as u32,
                oy as u32,
                image::Rgb([
                    (a[0] / norm) as u8,
                    (a[1] / norm) as u8,
                    (a[2] / norm) as u8,
                ]),
            );
        }
    }
    out
}

fn gray_from_rgb(rgb: &RgbImage) -> GrayImage {
    let mut out = GrayImage::new(rgb.width(), rgb.height());
    for (px, op) in rgb.pixels().zip(out.pixels_mut()) {
        let [r, g, b] = px.0;
        op.0[0] =
            ((299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000).min(255) as u8;
    }
    out
}

// ---------------------------------------------------------------------------
// Edge detection (hand-rolled Canny)
// ---------------------------------------------------------------------------

/// Separable Gaussian blur. `sigma <= 0` returns the input unchanged.
fn gaussian_blur(src: &GrayImage, sigma: f64) -> GrayImage {
    if sigma <= 0.0 {
        return src.clone();
    }
    let radius = (sigma * 3.0).ceil().max(1.0) as i32;
    let mut kernel = Vec::with_capacity((radius * 2 + 1) as usize);
    let denom = 2.0 * sigma * sigma;
    for i in -radius..=radius {
        kernel.push((-(f64::from(i) * f64::from(i)) / denom).exp() as f32);
    }
    let sum: f32 = kernel.iter().sum();
    for k in kernel.iter_mut() {
        *k /= sum;
    }
    let (w, h) = (src.width() as i32, src.height() as i32);
    let raw = src.as_raw();
    let mut tmp = vec![0f32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0f32;
            for (ki, kv) in kernel.iter().enumerate() {
                let sx = (x + ki as i32 - radius).clamp(0, w - 1);
                acc += *kv * f32::from(raw[(y * w + sx) as usize]);
            }
            tmp[(y * w + x) as usize] = acc;
        }
    }
    let mut out = GrayImage::new(w as u32, h as u32);
    let dst = out.as_mut();
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0f32;
            for (ki, kv) in kernel.iter().enumerate() {
                let sy = (y + ki as i32 - radius).clamp(0, h - 1);
                acc += *kv * tmp[(sy * w + x) as usize];
            }
            dst[(y * w + x) as usize] = acc.round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// Otsu's threshold over a 256-bin histogram (ignoring bin 0, which is the
/// suppressed background).
fn otsu_nonzero(hist: &[u64; 256]) -> u8 {
    let total: u64 = hist.iter().skip(1).sum();
    if total == 0 {
        return 0;
    }
    let sum_all: f64 = hist
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, &c)| i as f64 * c as f64)
        .sum();
    let total = total as f64;
    let (mut w_b, mut sum_b, mut best_t, mut best_var) = (0.0f64, 0.0f64, 1u8, -1.0f64);
    for t in 1..256usize {
        w_b += hist[t] as f64;
        if w_b == 0.0 {
            continue;
        }
        let w_f = total - w_b;
        if w_f <= 0.0 {
            break;
        }
        sum_b += t as f64 * hist[t] as f64;
        let m_b = sum_b / w_b;
        let m_f = (sum_all - sum_b) / w_f;
        let between = w_b * w_f * (m_b - m_f) * (m_b - m_f);
        if between > best_var {
            best_var = between;
            best_t = t as u8;
        }
    }
    best_t
}

struct Edges {
    map: Vec<u8>,
    count: u64,
    low_used: f64,
    high_used: f64,
}

/// Blur → Sobel → non-maximum suppression → hysteresis.
fn canny(gray: &GrayImage, blur: f64, low: f64, high: f64) -> Edges {
    let blurred = gaussian_blur(gray, blur);
    let (w, h) = (blurred.width() as usize, blurred.height() as usize);
    let src = blurred.as_raw();
    let mut mag = vec![0f32; w * h];
    let mut sector = vec![0u8; w * h];
    if w >= 3 && h >= 3 {
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let i = y * w + x;
                let (nw, n, ne) = (
                    f32::from(src[i - w - 1]),
                    f32::from(src[i - w]),
                    f32::from(src[i - w + 1]),
                );
                let (we, ea) = (f32::from(src[i - 1]), f32::from(src[i + 1]));
                let (sw, s, se) = (
                    f32::from(src[i + w - 1]),
                    f32::from(src[i + w]),
                    f32::from(src[i + w + 1]),
                );
                let gx = (ne + 2.0 * ea + se) - (nw + 2.0 * we + sw);
                let gy = (sw + 2.0 * s + se) - (nw + 2.0 * n + ne);
                mag[i] = (gx * gx + gy * gy).sqrt();
                // Sector without atan2: tan(22.5°)=0.4142, tan(67.5°)=2.4142.
                let (ax, ay) = (gx.abs(), gy.abs());
                sector[i] = if ay <= 0.414_213_6 * ax {
                    0 // gradient runs east-west
                } else if ay >= 2.414_213_6 * ax {
                    2 // gradient runs north-south
                } else if (gx >= 0.0) == (gy >= 0.0) {
                    1
                } else {
                    3
                };
            }
        }
    }

    // Non-maximum suppression, normalized to 0-255.
    let mut nms = vec![0u8; w * h];
    let mut hist = [0u64; 256];
    if w >= 3 && h >= 3 {
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let i = y * w + x;
                let m = mag[i];
                let (a, b) = match sector[i] {
                    0 => (mag[i - 1], mag[i + 1]),
                    2 => (mag[i - w], mag[i + w]),
                    1 => (mag[i - w - 1], mag[i + w + 1]),
                    _ => (mag[i - w + 1], mag[i + w - 1]),
                };
                if m >= a && m >= b {
                    let v = ((m / MAX_GRADIENT) * 255.0).round().clamp(0.0, 255.0) as u8;
                    nms[i] = v;
                    hist[v as usize] += 1;
                }
            }
        }
    }
    drop(mag);
    drop(sector);

    // Resolve automatic thresholds from the suppressed gradient histogram.
    let auto_high = f64::from(otsu_nonzero(&hist)) / 255.0;
    let high_used = if high > 0.0 {
        high
    } else {
        auto_high.max(AUTO_HIGH_FLOOR)
    };
    let low_used = if low > 0.0 {
        low.min(high_used)
    } else {
        (high_used * AUTO_LOW_RATIO).max(AUTO_HIGH_FLOOR * AUTO_LOW_RATIO)
    };
    let hi_level = (high_used * 255.0).round().clamp(1.0, 255.0) as u8;
    let lo_level = (low_used * 255.0).round().clamp(1.0, 255.0) as u8;

    // Hysteresis: flood from strong pixels through weak ones.
    let mut map = vec![0u8; w * h];
    let mut stack: Vec<u32> = Vec::new();
    for i in 0..w * h {
        if nms[i] >= hi_level {
            map[i] = 1;
            stack.push(i as u32);
        }
    }
    while let Some(i) = stack.pop() {
        let i = i as usize;
        let (x, y) = (i % w, i / w);
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let j = ny as usize * w + nx as usize;
                if map[j] == 0 && nms[j] >= lo_level {
                    map[j] = 1;
                    stack.push(j as u32);
                }
            }
        }
    }
    let count = map.iter().filter(|&&v| v == 1).count() as u64;
    Edges {
        map,
        count,
        low_used,
        high_used,
    }
}

// ---------------------------------------------------------------------------
// Hough accumulator
// ---------------------------------------------------------------------------

struct Accumulator {
    votes: Vec<u32>,
    n_theta: usize,
    n_rho: usize,
    theta_step: f64,
    rho_res: f64,
    rho_offset: f64,
}

impl Accumulator {
    fn at(&self, k: i64, r: i64) -> u32 {
        // theta wraps at 180°, where rho flips sign — so the seam suppresses
        // correctly and a vertical line peaks exactly once.
        let (mut k, mut r) = (k, r);
        if k < 0 {
            k += self.n_theta as i64;
            r = self.n_rho as i64 - 1 - r;
        } else if k >= self.n_theta as i64 {
            k -= self.n_theta as i64;
            r = self.n_rho as i64 - 1 - r;
        }
        if r < 0 || r >= self.n_rho as i64 {
            return 0;
        }
        self.votes[k as usize * self.n_rho + r as usize]
    }
    fn theta(&self, k: usize) -> f64 {
        k as f64 * self.theta_step
    }
    fn rho(&self, r: usize) -> f64 {
        r as f64 * self.rho_res - self.rho_offset
    }
}

fn accumulate(edges: &[u8], w: usize, h: usize, angle_res: f64, rho_res: f64) -> Accumulator {
    let n_theta = (180.0 / angle_res).round().max(1.0) as usize;
    let theta_step = 180.0 / n_theta as f64;
    let diag = ((w * w + h * h) as f64).sqrt();
    let n_rho = ((2.0 * diag) / rho_res).ceil() as usize + 3;
    let rho_offset = diag + rho_res;
    let mut sin_t = Vec::with_capacity(n_theta);
    let mut cos_t = Vec::with_capacity(n_theta);
    for k in 0..n_theta {
        let t = (k as f64 * theta_step).to_radians();
        sin_t.push(t.sin() as f32);
        cos_t.push(t.cos() as f32);
    }
    let mut votes = vec![0u32; n_theta * n_rho];
    let inv_res = (1.0 / rho_res) as f32;
    let off = rho_offset as f32;
    for y in 0..h {
        let fy = y as f32;
        for x in 0..w {
            if edges[y * w + x] == 0 {
                continue;
            }
            let fx = x as f32;
            for k in 0..n_theta {
                let rho = fx * cos_t[k] + fy * sin_t[k];
                let idx = ((rho + off) * inv_res + 0.5) as usize;
                if idx < n_rho {
                    votes[k * n_rho + idx] += 1;
                }
            }
        }
    }
    Accumulator {
        votes,
        n_theta,
        n_rho,
        theta_step,
        rho_res,
        rho_offset,
    }
}

#[derive(Debug, Clone, Copy)]
struct Peak {
    theta: f64,
    rho: f64,
    votes: u32,
}

fn find_peaks(acc: &Accumulator, threshold: u32) -> Vec<Peak> {
    let rho_win = (NMS_RHO_PX / acc.rho_res).round().max(1.0) as i64;
    let theta_win = (NMS_THETA_DEG / acc.theta_step).round().max(1.0) as i64;
    let mut peaks = Vec::new();
    for k in 0..acc.n_theta {
        for r in 0..acc.n_rho {
            let v = acc.votes[k * acc.n_rho + r];
            if v < threshold || v == 0 {
                continue;
            }
            let mut is_peak = true;
            'win: for dk in -theta_win..=theta_win {
                for dr in -rho_win..=rho_win {
                    if dk == 0 && dr == 0 {
                        continue;
                    }
                    let n = acc.at(k as i64 + dk, r as i64 + dr);
                    // Strict on the "earlier" side, permissive on the later one:
                    // plateaus resolve to a single, deterministic winner.
                    let earlier = dk < 0 || (dk == 0 && dr < 0);
                    if (earlier && n >= v) || (!earlier && n > v) {
                        is_peak = false;
                        break 'win;
                    }
                }
            }
            if is_peak {
                peaks.push(Peak {
                    theta: acc.theta(k),
                    rho: acc.rho(r),
                    votes: v,
                });
            }
        }
    }
    peaks.sort_by(|a, b| {
        b.votes
            .cmp(&a.votes)
            .then(a.theta.total_cmp(&b.theta))
            .then(a.rho.total_cmp(&b.rho))
    });
    peaks
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// Tilt from horizontal in (-90, 90], positive = right end lower on screen.
fn angle_from_theta(theta_deg: f64) -> f64 {
    let mut a = theta_deg + 90.0;
    while a > 90.0 {
        a -= 180.0;
    }
    while a <= -90.0 {
        a += 180.0;
    }
    a
}

fn classify(angle: f64) -> &'static str {
    let a = angle.abs();
    if a <= AXIS_TOLERANCE_DEG {
        "horizontal"
    } else if a >= 90.0 - AXIS_TOLERANCE_DEG {
        "vertical"
    } else {
        "diagonal"
    }
}

fn keeps(orientation: Orientation, class: &str) -> bool {
    match orientation {
        Orientation::Any => true,
        Orientation::Horizontal => class == "horizontal",
        Orientation::Vertical => class == "vertical",
    }
}

/// Walk the line `(rho, theta)` across the image, one analysis pixel per step,
/// calling `f(t, x, y)` for each in-bounds sample. `t` is arc length along the
/// line, so segment lengths fall straight out of it.
fn walk_line(w: usize, h: usize, theta_deg: f64, rho: f64, mut f: impl FnMut(f64, usize, usize)) {
    let t = theta_deg.to_radians();
    let (s, c) = (t.sin(), t.cos());
    let (px, py) = (rho * c, rho * s);
    let (dx, dy) = (-s, c);
    let span = ((w * w + h * h) as f64).sqrt().ceil() as i64 + 2;
    for ti in -span..=span {
        let tf = ti as f64;
        let x = (px + tf * dx).round();
        let y = (py + tf * dy).round();
        if x < 0.0 || y < 0.0 {
            continue;
        }
        let (xu, yu) = (x as usize, y as usize);
        if xu >= w || yu >= h {
            continue;
        }
        f(tf, xu, yu);
    }
}

fn point_at(theta_deg: f64, rho: f64, t: f64) -> (f64, f64) {
    let th = theta_deg.to_radians();
    let (s, c) = (th.sin(), th.cos());
    (rho * c - t * s, rho * s + t * c)
}

/// Perpendicular (normal) unit step, rounded to whole pixels — used as the
/// ±1 px tolerance when testing whether the edge map is "on" at a sample.
fn normal_step(theta_deg: f64) -> (i64, i64) {
    let th = theta_deg.to_radians();
    (th.cos().round() as i64, th.sin().round() as i64)
}

fn edge_on(map: &[u8], w: usize, h: usize, x: usize, y: usize, n: (i64, i64)) -> bool {
    if map[y * w + x] == 1 {
        return true;
    }
    for sign in [-1i64, 1] {
        let nx = x as i64 + sign * n.0;
        let ny = y as i64 + sign * n.1;
        if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h && map[ny as usize * w + nx as usize] == 1
        {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Overlay rendering
// ---------------------------------------------------------------------------

fn draw_thick_line(img: &mut RgbImage, p0: (f64, f64), p1: (f64, f64), rgb: [u8; 3], width: u32) {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let steps = ((p1.0 - p0.0).abs().max((p1.1 - p0.1).abs())).ceil().max(1.0) as i64;
    let half = (width.max(1) as i64 - 1) / 2;
    for i in 0..=steps {
        let f = i as f64 / steps as f64;
        let x = (p0.0 + (p1.0 - p0.0) * f).round() as i64;
        let y = (p0.1 + (p1.1 - p0.1) * f).round() as i64;
        for oy in -half..=half + (width.max(1) as i64 - 1) % 2 {
            for ox in -half..=half + (width.max(1) as i64 - 1) % 2 {
                let (px, py) = (x + ox, y + oy);
                if px >= 0 && py >= 0 && px < w && py < h {
                    img.put_pixel(px as u32, py as u32, image::Rgb(rgb));
                }
            }
        }
    }
}

fn encode(img: &RgbImage, format: Format) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    let (w, h) = (img.width(), img.height());
    match format {
        Format::Png => PngEncoder::new(&mut buf)
            .write_image(img.as_raw(), w, h, ExtendedColorType::Rgb8)
            .map_err(|e| format!("could not encode the overlay PNG: {e}"))?,
        Format::Jpg => JpegEncoder::new_with_quality(&mut buf, 90)
            .write_image(img.as_raw(), w, h, ExtendedColorType::Rgb8)
            .map_err(|e| format!("could not encode the overlay JPEG: {e}"))?,
    }
    Ok(buf)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Detect the straight lines in the encoded image `bytes`.
pub fn detect(bytes: &[u8], opts: &Options) -> Result<Outcome, String> {
    opts.validate()?;
    if bytes.is_empty() {
        return Err("no image data: provide a PNG, JPEG, WebP, GIF or BMP image".into());
    }

    // Header-first: reject oversized rasters BEFORE allocating them.
    let decoder = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("could not read image data: {e}"))?
        .into_decoder()
        .map_err(|e| {
            format!("could not decode image (PNG, JPEG, WebP, GIF or BMP expected): {e}")
        })?;
    let (w0, h0) = decoder.dimensions();
    if w0 < 16 || h0 < 16 {
        return Err(format!(
            "image too small to analyze: {w0}x{h0} (need at least 16x16 pixels)"
        ));
    }
    let ct = decoder.color_type();
    let decoded_bytes = decoder.total_bytes();
    let eight_bit = matches!(
        ct,
        ColorType::L8 | ColorType::La8 | ColorType::Rgb8 | ColorType::Rgba8
    );
    let want_color = opts.output == Output::Overlay && opts.overlay_background == Background::Original;
    let extra = if eight_bit {
        0
    } else {
        u64::from(w0) * u64::from(h0) * if want_color { 3 } else { 1 }
    };
    if bytes.len() as u64 + decoded_bytes + extra > MEM_BUDGET {
        let mp = f64::from(w0) * f64::from(h0) / 1.0e6;
        return Err(format!(
            "image too large to analyze in the sandbox: {w0}x{h0} ({mp:.1} megapixels, ~{} MB decoded); re-export it at a lower resolution (2000 px on the long side is plenty)",
            (decoded_bytes + extra) / (1024 * 1024)
        ));
    }

    let k = w0.max(h0).div_ceil(MAX_SIDE).max(1);
    let (gray, rgb): (GrayImage, Option<RgbImage>) = if eight_bit {
        let mut raw = vec![0u8; decoded_bytes as usize];
        decoder
            .read_image(&mut raw)
            .map_err(|e| format!("could not decode image: {e}"))?;
        let ch = usize::from(ct.channel_count());
        if want_color {
            let c = rgb_box_downscale(&raw, w0, h0, ch, k);
            (gray_from_rgb(&c), Some(c))
        } else {
            (gray_box_downscale(&raw, w0, h0, ch, k), None)
        }
    } else {
        let img = DynamicImage::from_decoder(decoder)
            .map_err(|e| format!("could not decode image: {e}"))?;
        if want_color {
            let full = img.into_rgb8();
            let c = if k > 1 {
                rgb_box_downscale(full.as_raw(), w0, h0, 3, k)
            } else {
                full
            };
            (gray_from_rgb(&c), Some(c))
        } else {
            let g = img.into_luma8();
            let g = if k > 1 {
                gray_box_downscale(g.as_raw(), w0, h0, 1, k)
            } else {
                g
            };
            (g, None)
        }
    };

    let (aw, ah) = (gray.width() as usize, gray.height() as usize);
    let kf = f64::from(k);
    let diag_orig = (f64::from(w0) * f64::from(w0) + f64::from(h0) * f64::from(h0)).sqrt();

    let mut warnings: Vec<String> = Vec::new();
    if k > 1 {
        warnings.push(format!(
            "image was analyzed at {aw}x{ah} (downscaled {k}x); reported coordinates are accurate to about {k} original pixels"
        ));
    }

    // Effective lengths, in ORIGINAL pixels.
    let min_len_orig = if opts.min_line_length > 0.0 {
        opts.min_line_length
    } else {
        (diag_orig * AUTO_MIN_LEN_FRAC).max(AUTO_MIN_LEN_FLOOR)
    };
    let gap_orig = if opts.max_line_gap > 0.0 {
        opts.max_line_gap
    } else {
        (diag_orig * AUTO_GAP_FRAC).max(AUTO_GAP_FLOOR)
    };
    let min_len_a = (min_len_orig / kf).max(2.0);
    let gap_a = (gap_orig / kf).max(1.0);

    let mut edges = canny(&gray, opts.blur, opts.canny_low, opts.canny_high);
    if edges.count > BUSY_EDGE_PIXELS {
        warnings.push(format!(
            "{} edge pixels is a very busy edge map; raise canny_high (or blur) if the results look noisy",
            edges.count
        ));
    }

    let threshold_used = if opts.threshold > 0 {
        opts.threshold
    } else {
        ((min_len_a * AUTO_THRESHOLD_FRAC).round() as u32).max(AUTO_THRESHOLD_FLOOR)
    };

    let mut lines: Vec<Line> = Vec::new();
    if edges.count > 0 {
        let acc = accumulate(&edges.map, aw, ah, opts.angle_resolution, opts.rho_resolution);
        let peaks = find_peaks(&acc, threshold_used);
        let kept: Vec<Peak> = peaks
            .into_iter()
            .filter(|p| keeps(opts.orientation, classify(angle_from_theta(p.theta))))
            .take(opts.max_lines as usize)
            .collect();

        match opts.mode {
            Mode::Lines => {
                for p in &kept {
                    let (mut t_min, mut t_max) = (f64::INFINITY, f64::NEG_INFINITY);
                    walk_line(aw, ah, p.theta, p.rho, |t, _, _| {
                        t_min = t_min.min(t);
                        t_max = t_max.max(t);
                    });
                    if !t_min.is_finite() || t_max <= t_min {
                        continue;
                    }
                    lines.push(make_line(p, t_min, t_max, kf, w0, h0));
                }
            }
            Mode::Segments => {
                let n = normal_step(0.0);
                let _ = n;
                for p in &kept {
                    let n = normal_step(p.theta);
                    let mut runs: Vec<(f64, f64, u32)> = Vec::new();
                    let mut cur: Option<(f64, f64, u32)> = None;
                    walk_line(aw, ah, p.theta, p.rho, |t, x, y| {
                        if !edge_on(&edges.map, aw, ah, x, y, n) {
                            return;
                        }
                        match cur.as_mut() {
                            None => cur = Some((t, t, 1)),
                            Some(r) => {
                                if t - r.1 > gap_a {
                                    runs.push(*r);
                                    cur = Some((t, t, 1));
                                } else {
                                    r.1 = t;
                                    r.2 += 1;
                                }
                            }
                        }
                    });
                    if let Some(r) = cur {
                        runs.push(r);
                    }
                    for (start, end, _) in runs {
                        if end - start < min_len_a {
                            continue;
                        }
                        // Consume the ink this segment used so a neighbouring
                        // peak cannot report the same edge a second time.
                        walk_line(aw, ah, p.theta, p.rho, |t, x, y| {
                            if t < start - 1.0 || t > end + 1.0 {
                                return;
                            }
                            for sign in [-1i64, 0, 1] {
                                let nx = x as i64 + sign * n.0;
                                let ny = y as i64 + sign * n.1;
                                if nx >= 0 && ny >= 0 && (nx as usize) < aw && (ny as usize) < ah {
                                    edges.map[ny as usize * aw + nx as usize] = 0;
                                }
                            }
                        });
                        lines.push(make_line(p, start, end, kf, w0, h0));
                    }
                }
                lines.sort_by(|a, b| {
                    b.length
                        .total_cmp(&a.length)
                        .then(b.votes.cmp(&a.votes))
                        .then(a.y1.cmp(&b.y1))
                        .then(a.x1.cmp(&b.x1))
                });
                lines.truncate(opts.max_lines as usize);
            }
        }
    }

    if lines.is_empty() {
        warnings.push(
            "no lines found — lower threshold or min_line_length, lower canny_high to keep fainter \
             edges, or check that the image really contains straight lines"
                .to_string(),
        );
    }

    let horizontal_count = lines.iter().filter(|l| l.orientation == "horizontal").count();
    let vertical_count = lines.iter().filter(|l| l.orientation == "vertical").count();
    let diagonal_count = lines.len() - horizontal_count - vertical_count;
    let dominant_angle_degrees = lines
        .iter()
        .max_by(|a, b| a.length.total_cmp(&b.length))
        .map(|l| l.angle_degrees);

    let overlay = if opts.output == Output::Overlay {
        let mut canvas = match opts.overlay_background {
            Background::Original => rgb.unwrap_or_else(|| RgbImage::new(aw as u32, ah as u32)),
            Background::Edges => {
                let mut c = RgbImage::new(aw as u32, ah as u32);
                for y in 0..ah {
                    for x in 0..aw {
                        if edges.map[y * aw + x] == 1 {
                            c.put_pixel(x as u32, y as u32, image::Rgb([255, 255, 255]));
                        }
                    }
                }
                c
            }
            Background::Black => RgbImage::new(aw as u32, ah as u32),
            Background::White => {
                let mut c = RgbImage::new(aw as u32, ah as u32);
                for p in c.pixels_mut() {
                    p.0 = [255, 255, 255];
                }
                c
            }
        };
        let rgbv = parse_color(&opts.color)?;
        let width = if opts.line_width > 0 {
            opts.line_width
        } else {
            (aw.max(ah) as f64 / 400.0).round().clamp(1.0, 6.0) as u32
        };
        for l in &lines {
            draw_thick_line(
                &mut canvas,
                (l.x1 as f64 / kf, l.y1 as f64 / kf),
                (l.x2 as f64 / kf, l.y2 as f64 / kf),
                rgbv,
                width,
            );
        }
        let bytes = encode(&canvas, opts.format)?;
        Some((bytes, canvas.width(), canvas.height()))
    } else {
        None
    };

    Ok(Outcome {
        detection: Detection {
            width: w0,
            height: h0,
            analysis_width: aw as u32,
            analysis_height: ah as u32,
            downscale_factor: k,
            edge_pixels: edges.count,
            canny_low_used: round4(edges.low_used),
            canny_high_used: round4(edges.high_used),
            threshold_used,
            min_line_length_used: round2(min_len_orig),
            max_line_gap_used: round2(gap_orig),
            line_count: lines.len(),
            lines,
            dominant_angle_degrees,
            horizontal_count,
            vertical_count,
            diagonal_count,
            warnings,
        },
        overlay,
    })
}

/// Build the reported line from a peak and an arc-length span, converting the
/// analysis-space geometry back to ORIGINAL image pixels.
fn make_line(p: &Peak, t_start: f64, t_end: f64, k: f64, w0: u32, h0: u32) -> Line {
    let a = point_at(p.theta, p.rho, t_start);
    let b = point_at(p.theta, p.rho, t_end);
    let to_orig = |v: f64, max: u32| -> i64 {
        (((v + 0.5) * k - 0.5).round() as i64).clamp(0, i64::from(max) - 1)
    };
    let (x1, y1) = (to_orig(a.0, w0), to_orig(a.1, h0));
    let (x2, y2) = (to_orig(b.0, w0), to_orig(b.1, h0));
    let angle = angle_from_theta(p.theta);
    let dx = (x2 - x1) as f64;
    let dy = (y2 - y1) as f64;
    Line {
        x1,
        y1,
        x2,
        y2,
        length: round2((dx * dx + dy * dy).sqrt()),
        angle_degrees: round2(angle),
        rho: round2(p.rho * k),
        theta_degrees: round2(p.theta),
        votes: p.votes,
        orientation: classify(angle),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Luma, Rgb};

    /// White canvas with black lines drawn on it, encoded as PNG.
    fn png_with(w: u32, h: u32, draw: impl Fn(&mut GrayImage)) -> Vec<u8> {
        let mut img = GrayImage::from_pixel(w, h, Luma([255]));
        draw(&mut img);
        let mut buf = Vec::new();
        PngEncoder::new(&mut buf)
            .write_image(img.as_raw(), w, h, ExtendedColorType::L8)
            .unwrap();
        buf
    }

    fn hline(img: &mut GrayImage, y: u32, x0: u32, x1: u32) {
        for x in x0..x1 {
            img.put_pixel(x, y, Luma([0]));
            img.put_pixel(x, y + 1, Luma([0]));
        }
    }

    fn vline(img: &mut GrayImage, x: u32, y0: u32, y1: u32) {
        for y in y0..y1 {
            img.put_pixel(x, y, Luma([0]));
            img.put_pixel(x + 1, y, Luma([0]));
        }
    }

    #[test]
    fn finds_a_single_horizontal_rule() {
        let png = png_with(240, 160, |img| hline(img, 80, 20, 220));
        let out = detect(&png, &Options::default()).unwrap();
        let d = &out.detection;
        assert_eq!((d.width, d.height), (240, 160));
        assert_eq!(d.downscale_factor, 1);
        assert!(d.line_count >= 1, "expected a line, got {d:?}");
        let l = &d.lines[0];
        assert_eq!(l.orientation, "horizontal");
        assert!(l.angle_degrees.abs() < 1.0, "angle was {}", l.angle_degrees);
        assert!((l.y1 - 80).abs() <= 3, "y was {}", l.y1);
        assert!(l.length > 150.0, "length was {}", l.length);
        assert_eq!(d.horizontal_count, d.line_count);
        assert!(out.overlay.is_none(), "report mode returns no image");
    }

    #[test]
    fn finds_both_axes_of_a_cross() {
        let png = png_with(240, 240, |img| {
            hline(img, 120, 10, 230);
            vline(img, 60, 10, 230);
        });
        let out = detect(&png, &Options::default()).unwrap();
        let d = &out.detection;
        assert!(d.horizontal_count >= 1, "{d:?}");
        assert!(d.vertical_count >= 1, "{d:?}");
        let v = d.lines.iter().find(|l| l.orientation == "vertical").unwrap();
        assert!((v.x1 - 60).abs() <= 3, "x was {}", v.x1);
        assert!(v.angle_degrees.abs() > 85.0, "angle was {}", v.angle_degrees);
    }

    #[test]
    fn orientation_filter_drops_the_other_axis() {
        let png = png_with(240, 240, |img| {
            hline(img, 120, 10, 230);
            vline(img, 60, 10, 230);
        });
        let opts = Options {
            orientation: Orientation::Horizontal,
            ..Options::default()
        };
        let d = detect(&png, &opts).unwrap().detection;
        assert!(d.line_count >= 1);
        assert_eq!(d.vertical_count, 0);
        assert!(d.lines.iter().all(|l| l.orientation == "horizontal"));
    }

    #[test]
    fn max_lines_caps_the_result() {
        let png = png_with(240, 240, |img| {
            for i in 0..8 {
                hline(img, 20 + i * 25, 10, 230);
            }
        });
        let opts = Options {
            max_lines: 3,
            ..Options::default()
        };
        let d = detect(&png, &opts).unwrap().detection;
        assert_eq!(d.line_count, 3, "{d:?}");
    }

    #[test]
    fn lines_mode_reports_a_full_width_chord() {
        let png = png_with(240, 160, |img| hline(img, 80, 20, 220));
        let opts = Options {
            mode: Mode::Lines,
            ..Options::default()
        };
        let d = detect(&png, &opts).unwrap().detection;
        assert!(d.line_count >= 1, "{d:?}");
        let l = &d.lines[0];
        // The infinite line is clipped to the image, so it spans the full width
        // even though the drawn rule stops short of both edges.
        assert!(l.length > 230.0, "length was {}", l.length);
    }

    #[test]
    fn segments_mode_respects_min_line_length() {
        let png = png_with(240, 160, |img| hline(img, 80, 20, 90));
        let opts = Options {
            min_line_length: 200.0,
            ..Options::default()
        };
        let d = detect(&png, &opts).unwrap().detection;
        assert_eq!(d.line_count, 0, "a 70 px rule must not satisfy min 200 px");
        assert_eq!(d.min_line_length_used, 200.0);
        assert!(d.warnings.iter().any(|w| w.contains("no lines found")));
    }

    #[test]
    fn overlay_returns_an_image_that_differs_from_the_source() {
        let png = png_with(240, 160, |img| hline(img, 80, 20, 220));
        let opts = Options {
            output: Output::Overlay,
            color: "#0f0".into(),
            line_width: 3,
            ..Options::default()
        };
        let out = detect(&png, &opts).unwrap();
        let (bytes, w, h) = out.overlay.expect("overlay mode returns an image");
        assert_eq!((w, h), (240, 160));
        let img = image::load_from_memory(&bytes).unwrap().into_rgb8();
        // The drawn line is pure green somewhere along row 80 (±line width).
        let mut greens = 0;
        for y in 76..85 {
            for x in 20..220 {
                if *img.get_pixel(x, y) == Rgb([0, 255, 0]) {
                    greens += 1;
                }
            }
        }
        assert!(greens > 100, "expected a green overlay stroke, got {greens} px");
    }

    #[test]
    fn overlay_jpg_format_round_trips() {
        let png = png_with(240, 160, |img| hline(img, 80, 20, 220));
        let opts = Options {
            output: Output::Overlay,
            format: Format::Jpg,
            overlay_background: Background::Edges,
            ..Options::default()
        };
        let (bytes, _, _) = detect(&png, &opts).unwrap().overlay.unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8], "JPEG SOI marker");
        assert!(image::load_from_memory(&bytes).is_ok());
    }

    #[test]
    fn long_hex_and_short_hex_and_names_all_parse() {
        assert_eq!(parse_color("#f00").unwrap(), [255, 0, 0]);
        assert_eq!(parse_color("#ff0000").unwrap(), [255, 0, 0]);
        assert_eq!(parse_color("ff0000").unwrap(), [255, 0, 0]);
        assert_eq!(parse_color("Lime").unwrap(), [0, 255, 0]);
        assert!(parse_color("#ff00").is_err());
        assert!(parse_color("chartreuse").is_err());
    }

    #[test]
    fn angle_convention_is_clockwise_positive() {
        // theta = 90° is a horizontal line; theta = 0° is vertical.
        assert_eq!(angle_from_theta(90.0), 0.0);
        assert_eq!(angle_from_theta(0.0), 90.0);
        // A normal at 45° means the line runs down-left to up-right → the right
        // end is HIGHER → negative.
        assert_eq!(angle_from_theta(45.0), -45.0);
        assert_eq!(angle_from_theta(135.0), 45.0);
        assert_eq!(classify(0.0), "horizontal");
        assert_eq!(classify(89.0), "vertical");
        assert_eq!(classify(40.0), "diagonal");
    }

    #[test]
    fn rejects_bad_options() {
        let png = png_with(64, 64, |_| {});
        let bad = |o: Options| detect(&png, &o).unwrap_err();
        assert!(bad(Options {
            angle_resolution: 0.0,
            ..Options::default()
        })
        .contains("angle_resolution"));
        assert!(bad(Options {
            max_lines: 0,
            ..Options::default()
        })
        .contains("max_lines"));
        assert!(bad(Options {
            canny_low: 0.9,
            canny_high: 0.2,
            ..Options::default()
        })
        .contains("canny_low"));
        assert!(bad(Options {
            color: "notacolor".into(),
            ..Options::default()
        })
        .contains("color must be"));
    }

    #[test]
    fn rejects_undecodable_and_tiny_input() {
        assert!(detect(b"", &Options::default())
            .unwrap_err()
            .contains("no image data"));
        assert!(detect(b"this is not an image", &Options::default())
            .unwrap_err()
            .contains("could not"));
        let tiny = png_with(8, 8, |_| {});
        assert!(detect(&tiny, &Options::default())
            .unwrap_err()
            .contains("too small"));
    }

    #[test]
    fn enum_parsers_accept_defaults_and_reject_junk() {
        assert_eq!(parse_mode(None).unwrap(), Mode::Segments);
        assert_eq!(parse_mode(Some("lines")).unwrap(), Mode::Lines);
        assert!(parse_mode(Some("probabilistic")).unwrap_err().contains("mode must be one of"));
        assert_eq!(parse_output(Some("overlay")).unwrap(), Output::Overlay);
        assert_eq!(parse_background(Some("edges")).unwrap(), Background::Edges);
        assert_eq!(parse_format(Some("jpg")).unwrap(), Format::Jpg);
        assert_eq!(parse_orientation(Some("vertical")).unwrap(), Orientation::Vertical);
        assert!(parse_format(Some("webp")).unwrap_err().contains("format must be one of"));
    }

    #[test]
    fn a_blank_image_reports_no_lines_not_an_error() {
        let png = png_with(120, 120, |_| {});
        let d = detect(&png, &Options::default()).unwrap().detection;
        assert_eq!(d.line_count, 0);
        assert_eq!(d.edge_pixels, 0);
        assert!(d.dominant_angle_degrees.is_none());
    }

    #[test]
    fn effective_thresholds_are_reported_back() {
        let png = png_with(240, 160, |img| hline(img, 80, 20, 220));
        let d = detect(&png, &Options::default()).unwrap().detection;
        // diag of 240x160 ≈ 288.44 → 8% ≈ 23.08, gap 1.5% ≈ 4.33 (floor 3).
        assert_eq!(d.min_line_length_used, 23.08);
        assert_eq!(d.max_line_gap_used, 4.33);
        assert_eq!(d.threshold_used, AUTO_THRESHOLD_FLOOR);
        assert!(d.canny_high_used > 0.0 && d.canny_low_used < d.canny_high_used);
    }
}
