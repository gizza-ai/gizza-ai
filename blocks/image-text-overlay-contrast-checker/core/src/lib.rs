//! image-text-overlay-contrast-checker core — find the worst-case place on a
//! photo where overlaid text of a given colour would fail WCAG contrast.
//!
//! A plain two-colour checker answers "does #ffffff pass over #3a3a3a?". Over a
//! photo there is no single background colour: a hero image can be safely dark
//! on the left and blown out on the right, so a caption that reads fine in the
//! mock-up disappears once it wraps. This core slides a text-shaped window over
//! the picture, takes each window's gamma-correct mean colour, and scores every
//! position against the WCAG 2.x contrast ratio for the chosen text colour.
//!
//! Pipeline: decode → box-downscale into a linear-light analysis raster →
//! summed-area table → O(1) mean per window → per-window contrast ratio →
//! worst/best window, per-placement verdicts, and the minimum scrim (overlay)
//! opacity that would rescue the whole area.
//!
//! Pure compute, no I/O. Colour parsing and the luminance/ratio maths are reused
//! from the `color-contrast-checker` core so the two tools never disagree.

use std::collections::BTreeSet;
use std::io::Cursor;

use gizza_ai_color_contrast_checker_core as cc;
use serde::Serialize;

pub use cc::{contrast_ratio, parse_color, relative_luminance, Rgb};

/// Longest edge of the analysis raster, in pixels. Contrast over a text-sized
/// area is an average, so a box-downscaled copy gives the same answer as the
/// full-resolution image while keeping the summed-area table a few MB.
pub const ANALYSIS_MAX: u32 = 512;

/// Decoded-raster budget. The wasm sandbox has 64 MiB; refuse politely above
/// this instead of trapping half-way through `decode`.
const DECODE_BUDGET: u64 = 44_000_000;

/// Most windows any single run will score. Reached only with a 1% × 1% window.
const MAX_WINDOWS: usize = 200_000;

/// Most windows that can be echoed back as a grid / CSV.
const MAX_GRID_WINDOWS: usize = 10_000;

/// WCAG conformance level the required ratio comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Aa,
    Aaa,
}

impl Level {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "aa" | "a" => Ok(Level::Aa),
            "aaa" => Ok(Level::Aaa),
            other => Err(format!("level must be aa or aaa (got \"{other}\")")),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Level::Aa => "aa",
            Level::Aaa => "aaa",
        }
    }
}

/// Which WCAG success criterion the overlaid text falls under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSize {
    /// Body text below 24 px / 18.66 px bold (SC 1.4.3).
    Normal,
    /// Headline text at 24 px+ or 18.66 px+ bold (SC 1.4.3).
    Large,
    /// Icons, logotype strokes, focus rings and other graphics (SC 1.4.11).
    Ui,
}

impl TextSize {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "normal" | "small" | "body" => Ok(TextSize::Normal),
            "large" | "heading" | "headline" => Ok(TextSize::Large),
            "ui" | "graphic" | "graphics" | "icon" => Ok(TextSize::Ui),
            other => Err(format!(
                "text_size must be one of normal, large, ui (got \"{other}\")"
            )),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            TextSize::Normal => "normal",
            TextSize::Large => "large",
            TextSize::Ui => "ui",
        }
    }
    /// The WCAG 2.x threshold for this criterion at this level. SC 1.4.11 has no
    /// AAA variant, so UI graphics stay at 3:1 either way.
    pub fn required_ratio(self, level: Level) -> f64 {
        match (self, level) {
            (TextSize::Normal, Level::Aa) => 4.5,
            (TextSize::Normal, Level::Aaa) => 7.0,
            (TextSize::Large, Level::Aa) => 3.0,
            (TextSize::Large, Level::Aaa) => 4.5,
            (TextSize::Ui, _) => 3.0,
        }
    }
}

/// Which slice of the picture is analysed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    Full,
    Top,
    Middle,
    Bottom,
    Left,
    Center,
    Right,
}

impl Region {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "full" | "all" | "whole" => Ok(Region::Full),
            "top" => Ok(Region::Top),
            "middle" | "centre-band" => Ok(Region::Middle),
            "bottom" => Ok(Region::Bottom),
            "left" => Ok(Region::Left),
            "center" | "centre" => Ok(Region::Center),
            "right" => Ok(Region::Right),
            other => Err(format!(
                "region must be one of full, top, middle, bottom, left, center, right (got \
                 \"{other}\")"
            )),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Region::Full => "full",
            Region::Top => "top",
            Region::Middle => "middle",
            Region::Bottom => "bottom",
            Region::Left => "left",
            Region::Center => "center",
            Region::Right => "right",
        }
    }
    /// Fractional box (x, y, w, h) of this region inside the image, in 0..1.
    fn fractions(self) -> (f64, f64, f64, f64) {
        let t = 1.0 / 3.0;
        match self {
            Region::Full => (0.0, 0.0, 1.0, 1.0),
            Region::Top => (0.0, 0.0, 1.0, t),
            Region::Middle => (0.0, t, 1.0, t),
            Region::Bottom => (0.0, 2.0 * t, 1.0, t),
            Region::Left => (0.0, 0.0, t, 1.0),
            Region::Center => (t, 0.0, t, 1.0),
            Region::Right => (2.0 * t, 0.0, t, 1.0),
        }
    }
}

/// What a partly transparent pixel is measured against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaBackground {
    White,
    Black,
}

impl AlphaBackground {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "white" | "#fff" | "#ffffff" => Ok(AlphaBackground::White),
            "black" | "#000" | "#000000" => Ok(AlphaBackground::Black),
            other => Err(format!(
                "alpha_background must be white or black (got \"{other}\")"
            )),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            AlphaBackground::White => "white",
            AlphaBackground::Black => "black",
        }
    }
    fn rgb(self) -> Rgb {
        match self {
            AlphaBackground::White => Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
            AlphaBackground::Black => Rgb { r: 0, g: 0, b: 0 },
        }
    }
}

/// Everything the scan needs beyond the image bytes.
#[derive(Debug, Clone)]
pub struct Options {
    pub text_color: Rgb,
    pub level: Level,
    pub text_size: TextSize,
    pub region: Region,
    /// Window width as a percent of the analysed region's width, 1–100.
    pub window_width: f64,
    /// Window height as a percent of the analysed region's height, 1–100.
    pub window_height: f64,
    pub alpha_background: AlphaBackground,
    /// Echo the full per-window ratio grid (bigger response, needed for charts).
    pub want_grid: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            text_color: Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
            level: Level::Aa,
            text_size: TextSize::Normal,
            region: Region::Full,
            window_width: 30.0,
            window_height: 10.0,
            alpha_background: AlphaBackground::White,
            want_grid: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Serialised report
// ---------------------------------------------------------------------------

/// A rectangle in ORIGINAL image pixels, plus its position as a percentage so
/// the answer is useful whatever the export size.
#[derive(Debug, Clone, Serialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub x_percent: f64,
    pub y_percent: f64,
}

/// One scored window position.
#[derive(Debug, Clone, Serialize)]
pub struct WindowHit {
    #[serde(flatten)]
    pub rect: Rect,
    /// Gamma-correct mean colour of the pixels under the window.
    pub mean_hex: String,
    pub mean_rgb: [u8; 3],
    pub contrast_ratio: f64,
    pub passes: bool,
    /// Which ninth of the analysed area the window's centre falls in.
    pub position: String,
}

/// Where a caption could go, ranked worst-window-first.
#[derive(Debug, Clone, Serialize)]
pub struct Placement {
    pub area: String,
    pub worst_ratio: f64,
    pub worst_hex: String,
    pub passes: bool,
    pub windows: u32,
}

/// The semi-transparent wash that would rescue the whole analysed area.
#[derive(Debug, Clone, Serialize)]
pub struct Scrim {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub black_opacity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub white_opacity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub css: Option<String>,
}

/// How pure black and pure white text would fare over the same area.
#[derive(Debug, Clone, Serialize)]
pub struct Alternative {
    pub hex: String,
    pub worst_ratio: f64,
    pub passes: bool,
}

/// The text colour under test.
#[derive(Debug, Clone, Serialize)]
pub struct ColorInfo {
    pub input: String,
    pub hex: String,
    pub rgb: [u8; 3],
    pub relative_luminance: f64,
}

/// The per-window ratio grid, emitted only for `output=full` / `csv`.
#[derive(Debug, Clone, Serialize)]
pub struct Grid {
    pub columns: u32,
    pub rows: u32,
    /// One row of contrast ratios per window row, left to right, top to bottom.
    pub ratios: Vec<Vec<f64>>,
}

/// The complete scan result.
#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    pub width: u32,
    pub height: u32,
    pub analysis_width: u32,
    pub analysis_height: u32,
    pub text_color: ColorInfo,
    pub level: String,
    pub text_size: String,
    pub required_ratio: f64,
    pub region: String,
    pub region_box: Rect,
    pub window: Rect,
    pub windows_checked: u32,
    pub failing_windows: u32,
    pub failing_percent: f64,
    pub passes: bool,
    pub worst: WindowHit,
    pub best: WindowHit,
    pub region_mean_hex: String,
    pub region_mean_ratio: f64,
    pub placements: Vec<Placement>,
    pub scrim: Scrim,
    pub alternatives: Vec<Alternative>,
    pub transparent_pixels: u64,
    pub semi_transparent_pixels: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid: Option<Grid>,
}

// ---------------------------------------------------------------------------
// sRGB helpers
// ---------------------------------------------------------------------------

fn srgb_to_linear(c: u8) -> f64 {
    let s = c as f64 / 255.0;
    if s <= 0.03928 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(l: f64) -> u8 {
    let l = l.clamp(0.0, 1.0);
    let s = if l <= 0.0031308 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// Composite `c` under an `opacity` wash of `over`, the way a CSS overlay does
/// it — straight alpha blending in sRGB space, not in linear light.
fn composite(c: Rgb, over: Rgb, opacity: f64) -> Rgb {
    let mix = |a: u8, b: u8| ((a as f64) * (1.0 - opacity) + (b as f64) * opacity).round() as u8;
    Rgb {
        r: mix(c.r, over.r),
        g: mix(c.g, over.g),
        b: mix(c.b, over.b),
    }
}

// ---------------------------------------------------------------------------
// Decode + downscale
// ---------------------------------------------------------------------------

struct Raster {
    src_w: u32,
    src_h: u32,
    w: u32,
    h: u32,
    /// Linear-light R,G,B means, row-major, three values per cell.
    lin: Vec<f64>,
    transparent: u64,
    semi_transparent: u64,
}

fn decode(bytes: &[u8], alpha_bg: AlphaBackground) -> Result<Raster, String> {
    if bytes.is_empty() {
        return Err("empty image: expected PNG, JPEG, WebP, GIF, BMP or TIFF bytes".into());
    }
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("could not read the image header: {e}"))?;
    let decoder = reader
        .into_decoder()
        .map_err(|e| format!("unsupported or corrupt image (PNG, JPEG, WebP, GIF, BMP and TIFF are supported): {e}"))?;
    let (src_w, src_h) = image::ImageDecoder::dimensions(&decoder);
    if src_w == 0 || src_h == 0 {
        return Err("image has zero width or height".into());
    }
    let raster_bytes = image::ImageDecoder::total_bytes(&decoder);
    if bytes.len() as u64 + raster_bytes > DECODE_BUDGET {
        return Err(format!(
            "image is too large to decode in the sandbox: {src_w}x{src_h} needs about {} MB \
             decoded (budget {} MB) — re-export it at a lower resolution, the scan only needs \
             enough detail to average text-sized areas",
            raster_bytes / 1_000_000,
            DECODE_BUDGET / 1_000_000
        ));
    }
    let img = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("could not decode the image: {e}"))?;

    // Box-downscale into a linear-light raster: contrast under a text-sized area
    // is an average, so averaging first costs nothing and bounds the memory.
    let long = src_w.max(src_h);
    let (w, h) = if long > ANALYSIS_MAX {
        let scale = ANALYSIS_MAX as f64 / long as f64;
        (
            ((src_w as f64 * scale).round() as u32).clamp(1, src_w),
            ((src_h as f64 * scale).round() as u32).clamp(1, src_h),
        )
    } else {
        (src_w, src_h)
    };

    let cells = (w as usize) * (h as usize);
    let mut acc = vec![0.0f64; cells * 3];
    let mut count = vec![0u32; cells];
    let bg = alpha_bg.rgb();
    let mut transparent = 0u64;
    let mut semi_transparent = 0u64;

    // Cache the 256 linearisation results — powf per channel per pixel is the
    // hot loop on a multi-megapixel photo.
    let lut: Vec<f64> = (0..=255u16).map(|c| srgb_to_linear(c as u8)).collect();

    use image::GenericImageView;
    for (x, y, px) in img.pixels() {
        let a = px.0[3];
        if a == 0 {
            transparent += 1;
        } else if a < 255 {
            semi_transparent += 1;
        }
        let blend = |c: u8, b: u8| {
            if a == 255 {
                c
            } else {
                (((c as u32) * (a as u32) + (b as u32) * (255 - a as u32) + 127) / 255) as u8
            }
        };
        let r = blend(px.0[0], bg.r);
        let g = blend(px.0[1], bg.g);
        let b = blend(px.0[2], bg.b);
        let cx = ((x as u64 * w as u64) / src_w as u64).min(w as u64 - 1) as usize;
        let cy = ((y as u64 * h as u64) / src_h as u64).min(h as u64 - 1) as usize;
        let idx = cy * w as usize + cx;
        acc[idx * 3] += lut[r as usize];
        acc[idx * 3 + 1] += lut[g as usize];
        acc[idx * 3 + 2] += lut[b as usize];
        count[idx] += 1;
    }

    let mut lin = vec![0.0f64; cells * 3];
    for i in 0..cells {
        let n = count[i].max(1) as f64;
        lin[i * 3] = acc[i * 3] / n;
        lin[i * 3 + 1] = acc[i * 3 + 1] / n;
        lin[i * 3 + 2] = acc[i * 3 + 2] / n;
    }

    Ok(Raster {
        src_w,
        src_h,
        w,
        h,
        lin,
        transparent,
        semi_transparent,
    })
}

/// Summed-area table over the linear raster: three channels, `(w+1)*(h+1)`.
struct Sat {
    w: usize,
    sum: Vec<f64>,
}

impl Sat {
    fn build(r: &Raster) -> Sat {
        let w = r.w as usize;
        let h = r.h as usize;
        let stride = w + 1;
        let mut sum = vec![0.0f64; stride * (h + 1) * 3];
        for y in 0..h {
            for x in 0..w {
                let src = (y * w + x) * 3;
                for c in 0..3 {
                    let up = sum[(y * stride + (x + 1)) * 3 + c];
                    let left = sum[((y + 1) * stride + x) * 3 + c];
                    let diag = sum[(y * stride + x) * 3 + c];
                    sum[((y + 1) * stride + (x + 1)) * 3 + c] = r.lin[src + c] + up + left - diag;
                }
            }
        }
        Sat { w: stride, sum }
    }

    /// Gamma-correct mean colour of the box `[x, x+bw) x [y, y+bh)`.
    fn mean(&self, x: usize, y: usize, bw: usize, bh: usize) -> Rgb {
        let n = (bw * bh) as f64;
        let at = |px: usize, py: usize, c: usize| self.sum[(py * self.w + px) * 3 + c];
        let mut out = [0u8; 3];
        for c in 0..3 {
            let total =
                at(x + bw, y + bh, c) - at(x, y + bh, c) - at(x + bw, y, c) + at(x, y, c);
            out[c] = linear_to_srgb(total / n);
        }
        Rgb {
            r: out[0],
            g: out[1],
            b: out[2],
        }
    }
}

// ---------------------------------------------------------------------------
// Scan
// ---------------------------------------------------------------------------

/// Name the ninth of the analysed region a point falls in.
fn ninth(cx: f64, cy: f64, rw: f64, rh: f64) -> String {
    let col = ((cx / rw) * 3.0).floor().clamp(0.0, 2.0) as usize;
    let row = ((cy / rh) * 3.0).floor().clamp(0.0, 2.0) as usize;
    let rows = ["top", "middle", "bottom"];
    let cols = ["left", "center", "right"];
    format!("{}-{}", rows[row], cols[col])
}

/// Scan `bytes` for the worst place to put `opts.text_color` text.
pub fn analyze(bytes: &[u8], opts: &Options) -> Result<Analysis, String> {
    if !(1.0..=100.0).contains(&opts.window_width) || !opts.window_width.is_finite() {
        return Err(format!(
            "window_width must be between 1 and 100 percent of the analysed area (got {})",
            opts.window_width
        ));
    }
    if !(1.0..=100.0).contains(&opts.window_height) || !opts.window_height.is_finite() {
        return Err(format!(
            "window_height must be between 1 and 100 percent of the analysed area (got {})",
            opts.window_height
        ));
    }

    let raster = decode(bytes, opts.alpha_background)?;
    let sat = Sat::build(&raster);

    // Region box, in analysis-raster cells.
    let (fx, fy, fw, fh) = opts.region.fractions();
    let rx = ((raster.w as f64 * fx).floor() as usize).min(raster.w as usize - 1);
    let ry = ((raster.h as f64 * fy).floor() as usize).min(raster.h as usize - 1);
    let rw = (((raster.w as f64 * fw).round() as usize).max(1)).min(raster.w as usize - rx);
    let rh = (((raster.h as f64 * fh).round() as usize).max(1)).min(raster.h as usize - ry);

    let bw = (((rw as f64) * opts.window_width / 100.0).round() as usize).clamp(1, rw);
    let bh = (((rh as f64) * opts.window_height / 100.0).round() as usize).clamp(1, rh);
    let sx = (bw / 4).max(1);
    let sy = (bh / 4).max(1);

    // Window origins, always including the flush-right / flush-bottom position
    // so an edge-hugging bright corner can never be stepped over.
    let origins = |span: usize, size: usize, step: usize, base: usize| -> Vec<usize> {
        let mut v = Vec::new();
        let last = span - size;
        let mut p = 0usize;
        while p < last {
            v.push(base + p);
            p += step;
        }
        v.push(base + last);
        v
    };
    let xs = origins(rw, bw, sx, rx);
    let ys = origins(rh, bh, sy, ry);
    if xs.len() * ys.len() > MAX_WINDOWS {
        return Err(format!(
            "that window size samples {} positions (cap {MAX_WINDOWS}) — raise window_width / \
             window_height",
            xs.len() * ys.len()
        ));
    }
    if opts.want_grid && xs.len() * ys.len() > MAX_GRID_WINDOWS {
        return Err(format!(
            "output=full/csv would return {} windows (cap {MAX_GRID_WINDOWS}) — raise \
             window_width / window_height, or use output=summary",
            xs.len() * ys.len()
        ));
    }

    let required = opts.text_size.required_ratio(opts.level);
    let scale_x = raster.src_w as f64 / raster.w as f64;
    let scale_y = raster.src_h as f64 / raster.h as f64;
    let to_rect = |x: usize, y: usize, w: usize, h: usize| Rect {
        x: (x as f64 * scale_x).round() as u32,
        y: (y as f64 * scale_y).round() as u32,
        width: ((w as f64 * scale_x).round() as u32).max(1),
        height: ((h as f64 * scale_y).round() as u32).max(1),
        x_percent: round2(x as f64 * 100.0 / raster.w as f64),
        y_percent: round2(y as f64 * 100.0 / raster.h as f64),
    };

    // Twelve candidate caption areas: the three full-width bands plus the nine
    // thirds cells, each tracking its own worst window.
    let areas: Vec<String> = ["top", "middle", "bottom"]
        .iter()
        .map(|s| s.to_string())
        .chain(["top", "middle", "bottom"].iter().flat_map(|r| {
            ["left", "center", "right"]
                .iter()
                .map(move |c| format!("{r}-{c}"))
        }))
        .collect();
    let mut area_worst: Vec<Option<(f64, Rgb, u32)>> = vec![None; areas.len()];

    let mut worst: Option<(f64, usize, usize, Rgb)> = None;
    let mut best: Option<(f64, usize, usize, Rgb)> = None;
    let mut failing = 0u32;
    let mut distinct: BTreeSet<u32> = BTreeSet::new();
    let mut grid_rows: Vec<Vec<f64>> = Vec::new();

    for &y in &ys {
        let mut row = Vec::with_capacity(xs.len());
        for &x in &xs {
            let mean = sat.mean(x, y, bw, bh);
            let ratio = cc::contrast_ratio(opts.text_color, mean);
            row.push(round2(ratio));
            distinct.insert(((mean.r as u32) << 16) | ((mean.g as u32) << 8) | mean.b as u32);
            if ratio + 1e-9 < required {
                failing += 1;
            }
            if worst.map_or(true, |(r, ..)| ratio < r) {
                worst = Some((ratio, x, y, mean));
            }
            if best.map_or(true, |(r, ..)| ratio > r) {
                best = Some((ratio, x, y, mean));
            }
            // Attribute the window to the bands/cells its centre lands in.
            let cx = (x - rx) as f64 + bw as f64 / 2.0;
            let cy = (y - ry) as f64 + bh as f64 / 2.0;
            let band = ((cy / rh as f64) * 3.0).floor().clamp(0.0, 2.0) as usize;
            let cell = 3 + band * 3 + ((cx / rw as f64) * 3.0).floor().clamp(0.0, 2.0) as usize;
            for slot in [band, cell] {
                let e = area_worst[slot].get_or_insert((f64::INFINITY, mean, 0));
                if ratio < e.0 {
                    e.0 = ratio;
                    e.1 = mean;
                }
                e.2 += 1;
            }
        }
        if opts.want_grid {
            grid_rows.push(row);
        }
    }

    let (worst_ratio, wx, wy, worst_mean) = worst.expect("at least one window is always scanned");
    let (best_ratio, bx, by, best_mean) = best.expect("at least one window is always scanned");
    let checked = (xs.len() * ys.len()) as u32;

    let hit = |ratio: f64, x: usize, y: usize, mean: Rgb| WindowHit {
        rect: to_rect(x, y, bw, bh),
        mean_hex: mean.to_hex(),
        mean_rgb: [mean.r, mean.g, mean.b],
        contrast_ratio: round2(ratio),
        passes: ratio + 1e-9 >= required,
        position: ninth(
            (x - rx) as f64 + bw as f64 / 2.0,
            (y - ry) as f64 + bh as f64 / 2.0,
            rw as f64,
            rh as f64,
        ),
    };

    let mut placements: Vec<Placement> = areas
        .iter()
        .zip(area_worst.iter())
        .filter_map(|(name, e)| {
            e.as_ref().map(|(r, c, n)| Placement {
                area: name.clone(),
                worst_ratio: round2(*r),
                worst_hex: c.to_hex(),
                passes: *r + 1e-9 >= required,
                windows: *n,
            })
        })
        .collect();
    placements.sort_by(|a, b| {
        b.worst_ratio
            .partial_cmp(&a.worst_ratio)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.area.cmp(&b.area))
    });

    let palette: Vec<Rgb> = distinct
        .iter()
        .map(|k| Rgb {
            r: (k >> 16) as u8,
            g: ((k >> 8) & 0xff) as u8,
            b: (k & 0xff) as u8,
        })
        .collect();

    let min_ratio_over = |fg: Rgb, colors: &[Rgb]| {
        colors.iter().fold(f64::INFINITY, |acc, &c| {
            acc.min(cc::contrast_ratio(fg, c))
        })
    };
    let scrim_opacity = |over: Rgb| -> Option<f64> {
        // Straight alpha is not monotone for a mid-tone text colour (darkening
        // can walk the background past the text and back down), so walk every
        // 1% step and take the first that lifts EVERY window over the bar.
        (0..=100).find_map(|step| {
            let a = step as f64 / 100.0;
            let ok = palette.iter().all(|&c| {
                cc::contrast_ratio(opts.text_color, composite(c, over, a)) + 1e-9 >= required
            });
            ok.then_some(a)
        })
    };
    let black = Rgb { r: 0, g: 0, b: 0 };
    let white = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    let black_opacity = scrim_opacity(black);
    let white_opacity = scrim_opacity(white);
    let (recommended, css) = match (black_opacity, white_opacity) {
        (Some(b), Some(w)) if b <= w => (Some("black".into()), Some(format!("rgba(0, 0, 0, {b})"))),
        (_, Some(w)) => (
            Some("white".into()),
            Some(format!("rgba(255, 255, 255, {w})")),
        ),
        (Some(b), None) => (Some("black".into()), Some(format!("rgba(0, 0, 0, {b})"))),
        (None, None) => (None, None),
    };

    let region_mean = sat.mean(rx, ry, rw, rh);

    Ok(Analysis {
        width: raster.src_w,
        height: raster.src_h,
        analysis_width: raster.w,
        analysis_height: raster.h,
        text_color: ColorInfo {
            input: String::new(),
            hex: opts.text_color.to_hex(),
            rgb: [
                opts.text_color.r,
                opts.text_color.g,
                opts.text_color.b,
            ],
            relative_luminance: round4(cc::relative_luminance(opts.text_color)),
        },
        level: opts.level.name().to_string(),
        text_size: opts.text_size.name().to_string(),
        required_ratio: required,
        region: opts.region.name().to_string(),
        region_box: to_rect(rx, ry, rw, rh),
        window: to_rect(0, 0, bw, bh),
        windows_checked: checked,
        failing_windows: failing,
        failing_percent: round2(failing as f64 * 100.0 / checked as f64),
        passes: failing == 0,
        worst: hit(worst_ratio, wx, wy, worst_mean),
        best: hit(best_ratio, bx, by, best_mean),
        region_mean_hex: region_mean.to_hex(),
        region_mean_ratio: round2(cc::contrast_ratio(opts.text_color, region_mean)),
        placements,
        scrim: Scrim {
            black_opacity,
            white_opacity,
            recommended,
            css,
        },
        alternatives: vec![
            Alternative {
                hex: black.to_hex(),
                worst_ratio: round2(min_ratio_over(black, &palette)),
                passes: min_ratio_over(black, &palette) + 1e-9 >= required,
            },
            Alternative {
                hex: white.to_hex(),
                worst_ratio: round2(min_ratio_over(white, &palette)),
                passes: min_ratio_over(white, &palette) + 1e-9 >= required,
            },
        ],
        transparent_pixels: raster.transparent,
        semi_transparent_pixels: raster.semi_transparent,
        grid: opts.want_grid.then(|| Grid {
            columns: xs.len() as u32,
            rows: ys.len() as u32,
            ratios: grid_rows,
        }),
    })
}

fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

/// The window grid as a spreadsheet table: one row per window position.
pub fn grid_csv(a: &Analysis) -> String {
    let mut out = String::from("row,column,contrast_ratio,passes\n");
    if let Some(g) = &a.grid {
        for (r, row) in g.ratios.iter().enumerate() {
            for (c, ratio) in row.iter().enumerate() {
                out.push_str(&format!(
                    "{r},{c},{ratio},{}\n",
                    if *ratio + 1e-9 >= a.required_ratio {
                        "yes"
                    } else {
                        "no"
                    }
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn png(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let mut img = image::RgbaImage::new(w, h);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgba(f(x, y));
        }
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    /// Top half black, bottom half white — white text is safe up top and
    /// invisible below.
    fn split() -> Vec<u8> {
        png(120, 120, |_, y| {
            if y < 60 {
                [0, 0, 0, 255]
            } else {
                [255, 255, 255, 255]
            }
        })
    }

    #[test]
    fn white_text_fails_over_the_white_half_and_passes_over_the_black_half() {
        let a = analyze(&split(), &Options::default()).unwrap();
        assert_eq!((a.width, a.height), (120, 120));
        assert_eq!(a.required_ratio, 4.5);
        assert!(!a.passes, "half the picture is white");
        assert_eq!(a.worst.mean_hex, "#ffffff");
        assert_eq!(a.worst.contrast_ratio, 1.0);
        assert!(!a.worst.position.starts_with("top"), "{:?}", a.worst);
        assert_eq!(a.best.mean_hex, "#000000");
        assert_eq!(a.best.contrast_ratio, 21.0);
        assert!(a.best.position.starts_with("top"));
        // The top band is safe, the bottom band is not.
        let top = a.placements.iter().find(|p| p.area == "top").unwrap();
        let bottom = a.placements.iter().find(|p| p.area == "bottom").unwrap();
        assert!(top.passes && top.worst_ratio == 21.0, "{top:?}");
        assert!(!bottom.passes && bottom.worst_ratio == 1.0, "{bottom:?}");
        // Ranked best-first.
        assert_eq!(a.placements[0].area, "top");
        // Black text is no better here (the black half kills it).
        assert!(a.alternatives.iter().all(|alt| !alt.passes));
    }

    #[test]
    fn a_region_can_be_scanned_on_its_own() {
        let opts = Options {
            region: Region::Top,
            ..Options::default()
        };
        let a = analyze(&split(), &opts).unwrap();
        assert!(a.passes, "the top third is solid black");
        assert_eq!(a.failing_windows, 0);
        assert_eq!(a.worst.contrast_ratio, 21.0);
        assert_eq!(a.region_box.height, 40);
        assert_eq!(a.region_mean_hex, "#000000");
    }

    #[test]
    fn a_dark_scrim_rescues_white_text_over_a_mid_grey() {
        // Flat #808080: white text scores 3.95:1, just under AA.
        let bytes = png(64, 64, |_, _| [128, 128, 128, 255]);
        let a = analyze(&bytes, &Options::default()).unwrap();
        assert!(!a.passes);
        assert_eq!(a.worst.mean_hex, "#808080");
        let black = a.scrim.black_opacity.expect("black scrim must be possible");
        assert!((0.05..=0.35).contains(&black), "{black}");
        // Compositing at that opacity really does clear the bar.
        let rescued = composite(Rgb { r: 128, g: 128, b: 128 }, Rgb { r: 0, g: 0, b: 0 }, black);
        assert!(contrast_ratio(a_white(), rescued) >= 4.5);
        // One step less must still fail — the answer is the MINIMUM opacity.
        let under = composite(
            Rgb { r: 128, g: 128, b: 128 },
            Rgb { r: 0, g: 0, b: 0 },
            black - 0.01,
        );
        assert!(contrast_ratio(a_white(), under) < 4.5);
        assert_eq!(a.scrim.recommended.as_deref(), Some("black"));
    }

    fn a_white() -> Rgb {
        Rgb {
            r: 255,
            g: 255,
            b: 255,
        }
    }

    #[test]
    fn large_text_uses_the_three_to_one_threshold() {
        let bytes = png(64, 64, |_, _| [128, 128, 128, 255]);
        let normal = analyze(&bytes, &Options::default()).unwrap();
        let large = analyze(
            &bytes,
            &Options {
                text_size: TextSize::Large,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(normal.required_ratio, 4.5);
        assert_eq!(large.required_ratio, 3.0);
        assert!(!normal.passes);
        assert!(large.passes, "3.95:1 clears the large-text bar");
    }

    #[test]
    fn transparency_is_composited_over_the_chosen_page_colour() {
        let bytes = png(32, 32, |_, _| [0, 0, 0, 0]);
        let over_white = analyze(&bytes, &Options::default()).unwrap();
        assert_eq!(over_white.worst.mean_hex, "#ffffff");
        assert_eq!(over_white.transparent_pixels, 32 * 32);
        let over_black = analyze(
            &bytes,
            &Options {
                alpha_background: AlphaBackground::Black,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(over_black.worst.mean_hex, "#000000");
        assert!(over_black.passes);
    }

    #[test]
    fn the_grid_and_its_csv_only_ship_when_asked_for() {
        let opts = Options {
            window_width: 50.0,
            window_height: 50.0,
            want_grid: true,
            ..Options::default()
        };
        let a = analyze(&split(), &opts).unwrap();
        let g = a.grid.as_ref().unwrap();
        assert_eq!(g.rows as usize, g.ratios.len());
        assert_eq!(g.columns as usize, g.ratios[0].len());
        assert_eq!(g.ratios[0][0], 21.0, "top-left window is over black");
        assert_eq!(*g.ratios.last().unwrap().last().unwrap(), 1.0);
        let csv = grid_csv(&a);
        assert!(csv.starts_with("row,column,contrast_ratio,passes\n"));
        assert!(csv.contains("0,0,21,yes\n"));

        let plain = analyze(&split(), &Options::default()).unwrap();
        assert!(plain.grid.is_none());
        assert_eq!(grid_csv(&plain), "row,column,contrast_ratio,passes\n");
    }

    #[test]
    fn bad_inputs_say_what_was_expected() {
        assert!(analyze(b"", &Options::default())
            .unwrap_err()
            .contains("empty image"));
        assert!(analyze(b"not an image at all", &Options::default())
            .unwrap_err()
            .to_lowercase()
            .contains("image"));
        let bad = Options {
            window_width: 0.0,
            ..Options::default()
        };
        assert!(analyze(&split(), &bad)
            .unwrap_err()
            .contains("window_width must be between 1 and 100"));
        let bad = Options {
            window_height: 250.0,
            ..Options::default()
        };
        assert!(analyze(&split(), &bad)
            .unwrap_err()
            .contains("window_height must be between 1 and 100"));
        assert!(Level::parse("aab").unwrap_err().contains("aa or aaa"));
        assert!(TextSize::parse("huge").unwrap_err().contains("normal"));
        assert!(Region::parse("corner").unwrap_err().contains("full"));
        assert!(AlphaBackground::parse("grey")
            .unwrap_err()
            .contains("white or black"));
    }

    #[test]
    fn a_tiny_window_grid_is_refused_before_it_blows_up_the_response() {
        let opts = Options {
            window_width: 1.0,
            window_height: 1.0,
            want_grid: true,
            ..Options::default()
        };
        let err = analyze(&split(), &opts).unwrap_err();
        assert!(err.contains("cap 10000"), "{err}");
    }
}
