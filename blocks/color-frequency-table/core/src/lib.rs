//! gizza-ai/color-frequency-table core — an exact census of the colours in an
//! image: which RGBA values are present, how many pixels each one covers, and
//! what share of the frame that is. No wafer/wasm-bindgen deps: pure Rust
//! (`image` decode only), so the block runs on every backend including the chat
//! Service Worker. Shared by the chat skill block, the CLI and the unit tests.
//!
//! Approach — count exactly, then present:
//!   1. Decode to RGBA and walk the pixels, keying a hash map by the **exact**
//!      RGBA value (that is the whole point: a palette extractor reports
//!      centroids that need not occur in the image, this reports what is
//!      literally there).
//!   2. `quantize > 1` buckets each channel into `quantize`-level groups and
//!      reports the **mean** colour of each bucket, so a JPEG-noisy sky
//!      collapses into one row with a real average instead of thousands of
//!      near-duplicates. The default of 1 is exact.
//!   3. Select the top N **by frequency** always — `sort` only changes the order
//!      the selected rows are presented in, so "top 10" keeps meaning "the 10
//!      most common colours" whichever ordering is asked for.
//!   4. Report the tail explicitly (`unique_colors`, `remaining_colors`,
//!      `remaining_percent`) so a truncated table is never mistaken for the
//!      whole census.
//!
//! Transparency: fully transparent pixels carry encoder-dependent junk RGB, so
//! by default they are counted separately and kept out of the census rather than
//! being allowed to invent a colour that nothing visible uses. Alpha is part of
//! the colour key either way, and every row carries `hex_rgba`.
//!
//! Above [`MAX_ANALYZED_PIXELS`] the walk switches to a symmetric row/column
//! stride sample and the result is marked `sampled` with a warning — percentages
//! stay representative, absolute counts become estimates and say so.

use std::collections::HashMap;
use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageReader, RgbaImage};
use serde::Serialize;

/// Input bytes + decoded raster must fit alongside the runtime in the wasm sandbox.
const MAX_DECODE_BYTES: u64 = 48 * 1024 * 1024;
/// Pixels with alpha below this read as transparent.
pub const ALPHA_THRESHOLD: u8 = 16;
/// Above this many pixels the census is taken from a stride sample.
pub const MAX_ANALYZED_PIXELS: u64 = 4_000_000;
/// Largest `top` (rows listed) the report will return.
pub const MAX_TOP: u32 = 256;
/// Largest `quantize` bucket width, in levels per channel.
pub const MAX_QUANTIZE: u32 = 64;

/// Which order the selected rows are presented in. The selection itself is
/// always by frequency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    /// Most pixels first — the census order.
    #[default]
    Frequency,
    /// Darkest first, by Rec. 601 luminance — reads like a tone ramp.
    Luminance,
    /// Around the colour wheel from red; greys (no saturation) lead.
    Hue,
}

impl Sort {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "frequency" | "count" | "freq" => Ok(Sort::Frequency),
            "luminance" | "luma" | "brightness" => Ok(Sort::Luminance),
            "hue" | "rainbow" => Ok(Sort::Hue),
            other => Err(format!(
                "sort must be one of frequency, luminance, hue (got \"{other}\")"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Sort::Frequency => "frequency",
            Sort::Luminance => "luminance",
            Sort::Hue => "hue",
        }
    }
}

/// Which notation fills the single colour column of the rendered table and CSV.
/// The JSON rows always carry all four, so nothing is lost by picking one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorFormat {
    /// `#rrggbb`.
    #[default]
    Hex,
    /// CSS `rgb(r, g, b)`.
    Rgb,
    /// CSS `rgba(r, g, b, a)` with alpha 0-1.
    Rgba,
    /// CSS `hsl(h, s%, l%)`.
    Hsl,
}

impl ColorFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "hex" | "#" => Ok(ColorFormat::Hex),
            "rgb" => Ok(ColorFormat::Rgb),
            "rgba" => Ok(ColorFormat::Rgba),
            "hsl" => Ok(ColorFormat::Hsl),
            other => Err(format!(
                "color_format must be one of hex, rgb, rgba, hsl (got \"{other}\")"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ColorFormat::Hex => "hex",
            ColorFormat::Rgb => "rgb",
            ColorFormat::Rgba => "rgba",
            ColorFormat::Hsl => "hsl",
        }
    }

    fn column<'a>(self, row: &'a Row) -> &'a str {
        match self {
            ColorFormat::Hex => &row.hex,
            ColorFormat::Rgb => &row.rgb,
            ColorFormat::Rgba => &row.rgba,
            ColorFormat::Hsl => &row.hsl,
        }
    }
}

/// Everything the caller can tune. `Default` matches the descriptor defaults.
#[derive(Debug, Clone)]
pub struct Options {
    /// How many rows to list, 1-256.
    pub top: u32,
    /// Bucket width in levels per channel; 1 = exact colours.
    pub quantize: u32,
    /// Drop colours covering less than this share of the counted pixels.
    pub min_percent: f64,
    /// Keep near-transparent pixels out of the census (they are still counted).
    pub ignore_transparency: bool,
    /// Presentation order of the selected rows.
    pub sort: Sort,
    /// Which notation fills the table/CSV colour column.
    pub color_format: ColorFormat,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            top: 10,
            quantize: 1,
            min_percent: 0.0,
            ignore_transparency: true,
            sort: Sort::Frequency,
            color_format: ColorFormat::Hex,
        }
    }
}

/// One colour in the census, with every notation a caller might paste onward.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    /// 1-based position in the frequency ranking, assigned BEFORE `sort`
    /// reorders the rows — rank 1 is always the most common colour.
    pub rank: u32,
    /// `#rrggbb` (lowercase).
    pub hex: String,
    /// `#rrggbbaa` (lowercase) — alpha is part of the colour identity.
    pub hex_rgba: String,
    /// CSS `rgb(r, g, b)`.
    pub rgb: String,
    /// CSS `rgba(r, g, b, a)` with alpha 0-1 (2 decimals).
    pub rgba: String,
    /// CSS `hsl(h, s%, l%)`.
    pub hsl: String,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
    /// Nearest plain-English colour name, so the answer reads naturally.
    pub color_name: &'static str,
    /// Pixels of this colour among the pixels actually inspected.
    pub count: u64,
    /// Full-image estimate of `count`, present only when `sampled` is true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_count: Option<u64>,
    /// Share of the counted (non-excluded) pixels, 0-100.
    pub percent: f64,
    /// `r == g == b` — a neutral grey (or black/white).
    pub is_grayscale: bool,
    /// Rec. 601 luminance, 0-255.
    pub luminance: f64,
}

/// The whole census. Serialised straight into the tool response.
#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    pub width: u32,
    pub height: u32,
    /// Decoded container format (`png`, `jpeg`, …).
    pub format: String,
    pub total_pixels: u64,
    pub megapixels: f64,
    /// Pixels actually inspected — equals `total_pixels` unless `sampled`.
    pub analyzed_pixels: u64,
    /// Pixels included in the census (analysed minus excluded transparent ones).
    pub counted_pixels: u64,
    /// True when a stride sample was used because the image exceeds
    /// [`MAX_ANALYZED_PIXELS`]; counts are then estimates.
    pub sampled: bool,
    /// 1 when every pixel was read, otherwise the row/column step used.
    pub stride: u32,
    /// Distinct colours found — exact when `quantize` is 1, otherwise the number
    /// of distinct buckets.
    pub unique_colors: u64,
    /// Of `unique_colors`, how many are neutral greys (`r == g == b`).
    pub grayscale_unique_colors: u64,
    /// Of `unique_colors`, how many are fully opaque (alpha 255).
    pub opaque_unique_colors: u64,
    /// Of `unique_colors`, how many are partly see-through (alpha < 255).
    pub translucent_unique_colors: u64,
    pub transparent_pixels: u64,
    pub transparent_percent: f64,
    /// Whether the image carried any alpha below 255 at all.
    pub has_alpha: bool,
    /// The listed colours, in `sort` order.
    pub colors: Vec<Row>,
    /// How many rows `colors` holds.
    pub listed_colors: u64,
    /// Pixels covered by the listed rows.
    pub listed_pixels: u64,
    /// Share of the counted pixels covered by the listed rows, 0-100.
    pub listed_percent: f64,
    /// Distinct colours NOT listed (tail plus anything under `min_percent`).
    pub remaining_colors: u64,
    /// Share of the counted pixels those unlisted colours cover, 0-100.
    pub remaining_percent: f64,
    /// The most common colour, whatever `sort` and `min_percent` did to the table.
    pub dominant_hex: String,
    pub dominant_name: &'static str,
    pub dominant_percent: f64,
    /// Echoed settings, so a saved report explains itself.
    pub quantize: u32,
    pub sort: &'static str,
    pub color_format: &'static str,
    pub min_percent: f64,
    pub warnings: Vec<String>,
}

/// Accumulator for one quantised colour bucket. `u32` sums are safe: the largest
/// possible term is [`MAX_ANALYZED_PIXELS`] * 255 ≈ 1.02e9.
#[derive(Default, Clone, Copy)]
struct Acc {
    n: u32,
    sum: [u32; 4],
}

impl Acc {
    fn mean(&self) -> [u8; 4] {
        let n = f64::from(self.n.max(1));
        let mut out = [0u8; 4];
        for i in 0..4 {
            out[i] = (f64::from(self.sum[i]) / n).round().clamp(0.0, 255.0) as u8;
        }
        out
    }
}

/// A small plain-English colour vocabulary so the report reads naturally — the
/// same list `background-color-detector` uses, for consistent naming across the
/// image-analysis blocks.
const NAMES: &[(&str, [u8; 3])] = &[
    ("black", [0, 0, 0]),
    ("white", [255, 255, 255]),
    ("gray", [128, 128, 128]),
    ("light gray", [211, 211, 211]),
    ("dark gray", [64, 64, 64]),
    ("off-white", [245, 245, 240]),
    ("beige", [222, 205, 175]),
    ("brown", [140, 90, 50]),
    ("red", [220, 40, 40]),
    ("maroon", [128, 0, 0]),
    ("orange", [255, 150, 40]),
    ("yellow", [245, 225, 60]),
    ("olive", [128, 128, 0]),
    ("green", [50, 170, 70]),
    ("dark green", [0, 90, 40]),
    ("teal", [0, 128, 128]),
    ("cyan", [80, 210, 220]),
    ("light blue", [150, 200, 235]),
    ("blue", [50, 100, 220]),
    ("navy", [20, 30, 90]),
    ("purple", [130, 70, 180]),
    ("magenta", [220, 60, 200]),
    ("pink", [245, 170, 190]),
];

fn nearest_name(r: u8, g: u8, b: u8) -> &'static str {
    let mut best = ("black", f64::MAX);
    for (name, [nr, ng, nb]) in NAMES {
        let d = (f64::from(r) - f64::from(*nr)).powi(2) * 0.3
            + (f64::from(g) - f64::from(*ng)).powi(2) * 0.59
            + (f64::from(b) - f64::from(*nb)).powi(2) * 0.11;
        if d < best.1 {
            best = (name, d);
        }
    }
    best.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (u16, u8, u8) {
    let (rf, gf, bf) = (
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
    );
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d.abs() < f64::EPSILON {
        return (0, 0, (l * 100.0).round() as u8);
    }
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - rf).abs() < f64::EPSILON {
        ((gf - bf) / d).rem_euclid(6.0)
    } else if (max - gf).abs() < f64::EPSILON {
        (bf - rf) / d + 2.0
    } else {
        (rf - gf) / d + 4.0
    } * 60.0;
    (
        h.round().rem_euclid(360.0) as u16,
        (s * 100.0).round() as u8,
        (l * 100.0).round() as u8,
    )
}

/// Rec. 601 luminance, the weighting camera and editor histograms use.
fn luminance(r: u8, g: u8, b: u8) -> f64 {
    0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b)
}

fn pack(c: [u8; 4]) -> u32 {
    u32::from_be_bytes(c)
}

fn unpack(k: u32) -> [u8; 4] {
    k.to_be_bytes()
}

/// Smallest symmetric stride keeping the inspected pixel count under
/// [`MAX_ANALYZED_PIXELS`].
fn stride_for(total: u64) -> u32 {
    if total <= MAX_ANALYZED_PIXELS {
        return 1;
    }
    let mut s = 2u32;
    while total / (u64::from(s) * u64::from(s)) > MAX_ANALYZED_PIXELS {
        s += 1;
    }
    s
}

fn decode(bytes: &[u8]) -> Result<(RgbaImage, String), String> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("could not read the image header: {e}"))?;
    let format = reader
        .format()
        .map(|f| format!("{f:?}").to_ascii_lowercase())
        .unwrap_or_else(|| "unknown".into());
    let decoder = reader.into_decoder().map_err(|e| {
        format!("could not decode the image (PNG, JPEG, WebP, GIF, BMP and TIFF are supported): {e}")
    })?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 {
        return Err("the image has zero width or height".into());
    }
    let needed = bytes.len() as u64 + decoder.total_bytes();
    if needed > MAX_DECODE_BYTES {
        return Err(format!(
            "image is too large to analyse in the sandbox ({w}x{h} needs about {} MB, the limit is \
             {} MB) — re-export it at a lower resolution",
            needed / (1024 * 1024),
            MAX_DECODE_BYTES / (1024 * 1024)
        ));
    }
    let img = DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("could not decode the image: {e}"))?;
    Ok((img.to_rgba8(), format))
}

fn validate(opts: &Options) -> Result<(), String> {
    if !(1..=MAX_TOP).contains(&opts.top) {
        return Err(format!(
            "top must be between 1 and {MAX_TOP} colours (got {})",
            opts.top
        ));
    }
    if !(1..=MAX_QUANTIZE).contains(&opts.quantize) {
        return Err(format!(
            "quantize must be between 1 and {MAX_QUANTIZE} levels per channel (got {}) — 1 counts \
             exact colours",
            opts.quantize
        ));
    }
    if !(0.0..=100.0).contains(&opts.min_percent) {
        return Err(format!(
            "min_percent must be between 0 and 100 (got {})",
            opts.min_percent
        ));
    }
    Ok(())
}

/// Build one report row from a colour and its pixel count.
fn row(rank: u32, c: [u8; 4], count: u64, counted: u64, scale: Option<f64>) -> Row {
    let [r, g, b, a] = c;
    let (h, s, l) = rgb_to_hsl(r, g, b);
    let percent = if counted == 0 {
        0.0
    } else {
        count as f64 * 100.0 / counted as f64
    };
    Row {
        rank,
        hex: format!("#{r:02x}{g:02x}{b:02x}"),
        hex_rgba: format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
        rgb: format!("rgb({r}, {g}, {b})"),
        rgba: format!("rgba({r}, {g}, {b}, {:.2})", f64::from(a) / 255.0),
        hsl: format!("hsl({h}, {s}%, {l}%)"),
        r,
        g,
        b,
        a,
        color_name: nearest_name(r, g, b),
        count,
        estimated_count: scale.map(|f| (count as f64 * f).round() as u64),
        percent: round4(percent),
        is_grayscale: r == g && g == b,
        luminance: round2(luminance(r, g, b)),
    }
}

/// Count every colour in `bytes` and shape the census `opts` asked for.
pub fn analyze(bytes: &[u8], opts: &Options) -> Result<Analysis, String> {
    if bytes.is_empty() {
        return Err("no image data was provided".into());
    }
    validate(opts)?;

    let (img, format) = decode(bytes)?;
    let (w, h) = (img.width(), img.height());
    let total = u64::from(w) * u64::from(h);
    let stride = stride_for(total);
    let q = opts.quantize;

    let mut exact: HashMap<u32, u32> = HashMap::new();
    let mut grouped: HashMap<u32, Acc> = HashMap::new();
    let mut analyzed = 0u64;
    let mut transparent = 0u64;
    let mut counted = 0u64;
    let mut has_alpha = false;

    let step = stride as usize;
    for y in (0..h).step_by(step) {
        for x in (0..w).step_by(step) {
            let px = img.get_pixel(x, y).0;
            analyzed += 1;
            if px[3] < 255 {
                has_alpha = true;
            }
            if px[3] < ALPHA_THRESHOLD {
                transparent += 1;
                if opts.ignore_transparency {
                    continue;
                }
            }
            counted += 1;
            if q == 1 {
                *exact.entry(pack(px)).or_insert(0) += 1;
            } else {
                let bucket = [
                    (px[0] as u32 / q) as u8,
                    (px[1] as u32 / q) as u8,
                    (px[2] as u32 / q) as u8,
                    (px[3] as u32 / q) as u8,
                ];
                let acc = grouped.entry(pack(bucket)).or_default();
                acc.n += 1;
                for i in 0..4 {
                    acc.sum[i] += u32::from(px[i]);
                }
            }
        }
    }

    // (colour, pixels) pairs, whichever path filled them.
    let mut entries: Vec<([u8; 4], u64)> = if q == 1 {
        exact
            .into_iter()
            .map(|(k, n)| (unpack(k), u64::from(n)))
            .collect()
    } else {
        grouped
            .into_iter()
            .map(|(_, acc)| (acc.mean(), u64::from(acc.n)))
            .collect()
    };
    // Most common first; the packed colour breaks ties so the order is stable.
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| pack(a.0).cmp(&pack(b.0))));

    let unique = entries.len() as u64;
    let grayscale_unique = entries
        .iter()
        .filter(|(c, _)| c[0] == c[1] && c[1] == c[2])
        .count() as u64;
    let opaque_unique = entries.iter().filter(|(c, _)| c[3] == 255).count() as u64;

    let scale = (stride > 1 && analyzed > 0).then(|| total as f64 / analyzed as f64);

    let mut warnings = Vec::new();
    if stride > 1 {
        warnings.push(format!(
            "image is larger than {} MP, so the census reads every {stride}th row and column \
             ({analyzed} of {total} pixels): percentages are representative estimates and the \
             per-colour counts are sample counts (estimated_count scales them to the full image)",
            MAX_ANALYZED_PIXELS / 1_000_000
        ));
    }
    if q > 1 {
        warnings.push(format!(
            "quantize={q} groups colours into {q}-level buckets per channel and reports each \
             bucket's mean colour, so a listed colour need not appear literally in the image — \
             use quantize=1 for an exact census"
        ));
    }
    if counted == 0 {
        warnings.push(
            "every pixel is transparent, so there is nothing to count — pass \
             ignore_transparency=false to census the RGB values stored under the alpha"
                .into(),
        );
    }

    let (dominant_hex, dominant_name, dominant_percent) = match entries.first() {
        Some((c, n)) => (
            format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]),
            nearest_name(c[0], c[1], c[2]),
            round4(*n as f64 * 100.0 / counted.max(1) as f64),
        ),
        None => ("".to_string(), "none", 0.0),
    };

    // Rank by frequency, drop anything under min_percent, then take the top N.
    let mut rows: Vec<Row> = entries
        .iter()
        .enumerate()
        .map(|(i, (c, n))| row(i as u32 + 1, *c, *n, counted, scale))
        .filter(|r| r.percent >= opts.min_percent)
        .take(opts.top as usize)
        .collect();

    let listed_pixels: u64 = rows.iter().map(|r| r.count).sum();
    let listed_percent = if counted == 0 {
        0.0
    } else {
        listed_pixels as f64 * 100.0 / counted as f64
    };

    match opts.sort {
        Sort::Frequency => {}
        Sort::Luminance => rows.sort_by(|a, b| {
            a.luminance
                .partial_cmp(&b.luminance)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.rank.cmp(&b.rank))
        }),
        Sort::Hue => rows.sort_by(|a, b| {
            hue_key(a)
                .partial_cmp(&hue_key(b))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.rank.cmp(&b.rank))
        }),
    }

    Ok(Analysis {
        width: w,
        height: h,
        format,
        total_pixels: total,
        megapixels: round2(total as f64 / 1_000_000.0),
        analyzed_pixels: analyzed,
        counted_pixels: counted,
        sampled: stride > 1,
        stride,
        unique_colors: unique,
        grayscale_unique_colors: grayscale_unique,
        opaque_unique_colors: opaque_unique,
        translucent_unique_colors: unique - opaque_unique,
        transparent_pixels: transparent,
        transparent_percent: if analyzed == 0 {
            0.0
        } else {
            round4(transparent as f64 * 100.0 / analyzed as f64)
        },
        has_alpha,
        listed_colors: rows.len() as u64,
        listed_pixels,
        listed_percent: round4(listed_percent),
        remaining_colors: unique - rows.len() as u64,
        remaining_percent: round4((100.0 - listed_percent).max(0.0)),
        colors: rows,
        dominant_hex,
        dominant_name,
        dominant_percent,
        quantize: q,
        sort: opts.sort.as_str(),
        color_format: opts.color_format.as_str(),
        min_percent: opts.min_percent,
        warnings,
    })
}

/// Greys have no meaningful hue, so they sort ahead of the coloured rows rather
/// than landing arbitrarily at red.
fn hue_key(r: &Row) -> f64 {
    let (h, s, _) = rgb_to_hsl(r.r, r.g, r.b);
    if s == 0 {
        -1.0
    } else {
        f64::from(h)
    }
}

/// The report as a fixed-width table — one copy-paste answer for a chat reply.
pub fn render_table(a: &Analysis, format: ColorFormat) -> String {
    let head = ("#", "COLOR", "PIXELS", "SHARE", "NAME");
    let color_w = a
        .colors
        .iter()
        .map(|r| format.column(r).len())
        .chain([head.1.len()])
        .max()
        .unwrap_or(head.1.len());
    let count_w = a
        .colors
        .iter()
        .map(|r| r.count.to_string().len())
        .chain([head.2.len()])
        .max()
        .unwrap_or(head.2.len());
    let rank_w = a
        .colors
        .iter()
        .map(|r| r.rank.to_string().len())
        .chain([head.0.len()])
        .max()
        .unwrap_or(head.0.len());

    let mut out = format!(
        "{:>rank_w$}  {:<color_w$}  {:>count_w$}  {:>7}  {}\n",
        head.0, head.1, head.2, head.3, head.4
    );
    for r in &a.colors {
        out.push_str(&format!(
            "{:>rank_w$}  {:<color_w$}  {:>count_w$}  {:>6.2}%  {}\n",
            r.rank,
            format.column(r),
            r.count,
            r.percent,
            r.color_name
        ));
    }
    out
}

/// The same rows as CSV, for a spreadsheet. The colour column is quoted because
/// `rgb(…)`/`hsl(…)` notations contain commas.
pub fn render_csv(a: &Analysis, format: ColorFormat) -> String {
    let mut out = String::from("rank,color,pixels,percent,name\n");
    for r in &a.colors {
        out.push_str(&format!(
            "{},\"{}\",{},{},{}\n",
            r.rank,
            format.column(r),
            r.count,
            r.percent,
            r.color_name
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// 4x4: 8 red, 4 green, 4 blue.
    fn three_colors() -> Vec<u8> {
        png(4, 4, |_, y| match y {
            0 | 1 => [255, 0, 0, 255],
            2 => [0, 128, 0, 255],
            _ => [0, 0, 255, 255],
        })
    }

    #[test]
    fn counts_exact_colors_with_shares_most_common_first() {
        let a = analyze(&three_colors(), &Options::default()).unwrap();
        assert_eq!((a.width, a.height), (4, 4));
        assert_eq!(a.format, "png");
        assert_eq!(a.total_pixels, 16);
        assert_eq!(a.counted_pixels, 16);
        assert_eq!(a.unique_colors, 3);
        assert_eq!(a.opaque_unique_colors, 3);
        assert_eq!(a.translucent_unique_colors, 0);
        assert!(!a.has_alpha);
        assert!(!a.sampled);
        assert_eq!(a.stride, 1);

        assert_eq!(a.colors.len(), 3);
        let first = &a.colors[0];
        assert_eq!(first.rank, 1);
        assert_eq!(first.hex, "#ff0000");
        assert_eq!(first.hex_rgba, "#ff0000ff");
        assert_eq!(first.rgb, "rgb(255, 0, 0)");
        assert_eq!(first.rgba, "rgba(255, 0, 0, 1.00)");
        assert_eq!(first.hsl, "hsl(0, 100%, 50%)");
        assert_eq!(first.color_name, "red");
        assert_eq!(first.count, 8);
        assert_eq!(first.percent, 50.0);
        assert!(!first.is_grayscale);
        assert_eq!(first.estimated_count, None);
        assert_eq!(a.colors[1].count, 4);
        assert_eq!(a.colors[2].count, 4);
        // Ties break on the packed RGBA, so blue (0x0000ffff) precedes green.
        assert_eq!(a.colors[1].hex, "#0000ff");
        assert_eq!(a.colors[2].hex, "#008000");

        assert_eq!(a.dominant_hex, "#ff0000");
        assert_eq!(a.dominant_name, "red");
        assert_eq!(a.dominant_percent, 50.0);
        assert_eq!(a.listed_percent, 100.0);
        assert_eq!(a.remaining_colors, 0);
        assert_eq!(a.remaining_percent, 0.0);
        assert!(a.warnings.is_empty());
    }

    #[test]
    fn empty_input_and_out_of_range_options_are_rejected() {
        assert!(analyze(&[], &Options::default())
            .unwrap_err()
            .contains("no image data"));
        assert!(analyze(b"not an image at all", &Options::default())
            .unwrap_err()
            .contains("could not"));

        let img = three_colors();
        let err = analyze(
            &img,
            &Options {
                top: 0,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("top must be between 1 and 256"), "{err}");
        let err = analyze(
            &img,
            &Options {
                quantize: 65,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("quantize must be between 1 and 64"), "{err}");
        let err = analyze(
            &img,
            &Options {
                min_percent: 101.0,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("min_percent"), "{err}");
        assert!(Sort::parse("size").unwrap_err().contains("frequency"));
        assert!(ColorFormat::parse("lab").unwrap_err().contains("hex"));
    }

    #[test]
    fn top_truncates_and_reports_the_tail() {
        let a = analyze(
            &three_colors(),
            &Options {
                top: 1,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(a.colors.len(), 1);
        assert_eq!(a.listed_colors, 1);
        assert_eq!(a.listed_pixels, 8);
        assert_eq!(a.listed_percent, 50.0);
        assert_eq!(a.unique_colors, 3);
        assert_eq!(a.remaining_colors, 2);
        assert_eq!(a.remaining_percent, 50.0);
    }

    #[test]
    fn min_percent_drops_the_rare_colors_before_the_top_n() {
        let a = analyze(
            &three_colors(),
            &Options {
                min_percent: 30.0,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(a.colors.len(), 1, "only red covers 30% or more");
        assert_eq!(a.colors[0].hex, "#ff0000");
        assert_eq!(a.remaining_colors, 2);
    }

    #[test]
    fn quantize_groups_near_duplicates_and_reports_the_bucket_mean() {
        // Four near-identical reds that no exact census would ever merge.
        let bytes = png(2, 2, |x, y| [250 + x as u8 * 2, y as u8, 0, 255]);
        let exact = analyze(&bytes, &Options::default()).unwrap();
        assert_eq!(exact.unique_colors, 4);

        let grouped = analyze(
            &bytes,
            &Options {
                quantize: 16,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(grouped.unique_colors, 1);
        let row = &grouped.colors[0];
        assert_eq!(row.count, 4);
        assert_eq!(row.percent, 100.0);
        // Mean of (250,0),(252,0),(250,1),(252,1) → (251, 0.5→1).
        assert_eq!(row.r, 251);
        assert_eq!(row.g, 1);
        assert!(grouped.warnings.iter().any(|w| w.contains("quantize=16")));
    }

    #[test]
    fn transparent_pixels_are_reported_separately_and_kept_out_by_default() {
        // Half opaque white, half fully transparent black.
        let bytes = png(2, 2, |_, y| {
            if y == 0 {
                [255, 255, 255, 255]
            } else {
                [0, 0, 0, 0]
            }
        });
        let a = analyze(&bytes, &Options::default()).unwrap();
        assert_eq!(a.transparent_pixels, 2);
        assert_eq!(a.transparent_percent, 50.0);
        assert!(a.has_alpha);
        assert_eq!(a.counted_pixels, 2);
        assert_eq!(a.unique_colors, 1, "the junk RGB under alpha=0 is excluded");
        assert_eq!(a.colors[0].hex, "#ffffff");
        assert_eq!(a.colors[0].percent, 100.0);

        let kept = analyze(
            &bytes,
            &Options {
                ignore_transparency: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(kept.counted_pixels, 4);
        assert_eq!(kept.unique_colors, 2);
        assert_eq!(kept.translucent_unique_colors, 1);
        assert_eq!(kept.colors[0].percent, 50.0);
        assert_eq!(kept.colors[0].hex_rgba, "#00000000");
    }

    #[test]
    fn a_fully_transparent_image_warns_instead_of_dividing_by_zero() {
        let a = analyze(&png(2, 2, |_, _| [9, 9, 9, 0]), &Options::default()).unwrap();
        assert_eq!(a.counted_pixels, 0);
        assert_eq!(a.unique_colors, 0);
        assert!(a.colors.is_empty());
        assert_eq!(a.dominant_percent, 0.0);
        assert_eq!(a.listed_percent, 0.0);
        assert!(a.warnings.iter().any(|w| w.contains("every pixel is transparent")));
    }

    #[test]
    fn sort_reorders_the_rows_but_never_changes_the_selection() {
        let by_luma = analyze(
            &three_colors(),
            &Options {
                sort: Sort::Luminance,
                ..Options::default()
            },
        )
        .unwrap();
        let hexes: Vec<&str> = by_luma.colors.iter().map(|r| r.hex.as_str()).collect();
        // blue (lum 29) < red (76) < green (75.1)? — Rec. 601: blue 29.07,
        // green(0,128,0) 75.14, red 76.25.
        assert_eq!(hexes, vec!["#0000ff", "#008000", "#ff0000"]);
        // Ranks still describe the frequency order.
        assert_eq!(by_luma.colors[2].rank, 1);
        assert_eq!(by_luma.sort, "luminance");

        let by_hue = analyze(
            &three_colors(),
            &Options {
                sort: Sort::Hue,
                ..Options::default()
            },
        )
        .unwrap();
        let hexes: Vec<&str> = by_hue.colors.iter().map(|r| r.hex.as_str()).collect();
        assert_eq!(hexes, vec!["#ff0000", "#008000", "#0000ff"], "0° 120° 240°");
    }

    #[test]
    fn grays_lead_the_hue_ordering() {
        let bytes = png(2, 2, |x, y| match (x, y) {
            (0, 0) => [255, 0, 0, 255],
            (1, 0) => [128, 128, 128, 255],
            (0, 1) => [0, 0, 255, 255],
            _ => [0, 200, 0, 255],
        });
        let a = analyze(
            &bytes,
            &Options {
                sort: Sort::Hue,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(a.colors[0].hex, "#808080");
        assert!(a.colors[0].is_grayscale);
        assert_eq!(a.grayscale_unique_colors, 1);
    }

    #[test]
    fn table_and_csv_use_the_requested_notation() {
        let a = analyze(&three_colors(), &Options::default()).unwrap();
        let table = render_table(&a, ColorFormat::Hex);
        let mut lines = table.lines();
        assert_eq!(lines.next().unwrap(), "#  COLOR    PIXELS    SHARE  NAME");
        assert_eq!(lines.next().unwrap(), "1  #ff0000       8   50.00%  red");
        assert_eq!(lines.next().unwrap(), "2  #0000ff       4   25.00%  navy");

        let csv = render_csv(&a, ColorFormat::Rgb);
        assert!(csv.starts_with("rank,color,pixels,percent,name\n"));
        assert!(csv.contains("1,\"rgb(255, 0, 0)\",8,50,red\n"));

        let hsl = render_csv(&a, ColorFormat::Hsl);
        assert!(hsl.contains("\"hsl(0, 100%, 50%)\""));
        let rgba = render_table(&a, ColorFormat::Rgba);
        assert!(rgba.contains("rgba(255, 0, 0, 1.00)"));
    }

    #[test]
    fn large_images_fall_back_to_a_marked_stride_sample() {
        assert_eq!(stride_for(MAX_ANALYZED_PIXELS), 1);
        assert_eq!(stride_for(MAX_ANALYZED_PIXELS * 4), 2);
        assert_eq!(stride_for(MAX_ANALYZED_PIXELS * 9), 3);
        assert_eq!(stride_for(MAX_ANALYZED_PIXELS * 5), 3);
    }
}
