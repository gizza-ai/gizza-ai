//! gizza-ai/image-histogram-analyzer core — per-channel RGB + luminance
//! histograms and the exposure verdict that comes with them (clipping, dynamic
//! range, contrast, colour cast). No wafer/wasm-bindgen deps: pure Rust (`image`
//! decode only), so the block runs on every backend including the chat Service
//! Worker. Shared by the chat skill block, the CLI and the unit tests.
//!
//! Approach — count first, judge second, always show the evidence:
//!   1. Decode to RGBA and walk **every** pixel (no sub-sampling: a blown sky in
//!      2% of the frame is exactly what a sampled histogram would miss).
//!   2. Accumulate four exact 256-level counters — red, green, blue and luma —
//!      where luma comes from the requested coefficients (Rec. 601 by default,
//!      Rec. 709, a plain channel average, or the max channel / HSV "value").
//!   3. Derive every statistic from those counters, so `bins` only ever changes
//!      the SHAPE of the reported histogram, never the numbers: min, max, mean,
//!      median, standard deviation, the 1/5/95/99th percentiles, the modal
//!      level, distinct levels used, and the share of pixels pinned at each end.
//!   4. Turn the numbers into the judgements a photographer actually wants —
//!      is anything clipped, how many stops of range are left, is the frame
//!      flat, is there a colour cast — each reported next to the number that
//!      decided it.
//!
//! Transparency: fully transparent pixels carry encoder-dependent junk RGB, so
//! by default they are excluded from the counts (and reported separately)
//! rather than being allowed to spike level 0.

use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageReader, RgbaImage};
use serde::Serialize;

/// Input bytes + decoded raster must fit alongside the runtime in the wasm sandbox.
const MAX_DECODE_BYTES: u64 = 48 * 1024 * 1024;
/// Pixels with alpha below this read as transparent.
pub const ALPHA_THRESHOLD: u8 = 16;
/// Below this luma standard deviation a frame reads as flat/low contrast.
const LOW_CONTRAST_STDDEV: f64 = 25.0;
/// Above this luma standard deviation a frame reads as high contrast.
const HIGH_CONTRAST_STDDEV: f64 = 65.0;
/// Mean luma below this reads as underexposed.
const DARK_MEAN: f64 = 60.0;
/// Mean luma above this reads as overexposed.
const BRIGHT_MEAN: f64 = 195.0;
/// A channel mean this far (levels) from the average of the three channel means
/// counts as a colour cast rather than sensor/JPEG noise.
const CAST_LEVELS: f64 = 2.0;

/// Luma weighting used to build the brightness histogram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Luma {
    /// Rec. 601 (0.299 R, 0.587 G, 0.114 B) — what most photo histograms show.
    Rec601,
    /// Rec. 709 (0.2126 R, 0.7152 G, 0.0722 B) — the sRGB/HD luma coefficients.
    Rec709,
    /// Unweighted mean of R, G and B.
    Average,
    /// Largest of R, G and B (HSV "value") — the safest highlight-clipping view.
    Max,
}

impl Luma {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().replace(['-', ' '], "_").as_str() {
            "rec601" | "rec_601" | "601" | "bt601" => Ok(Luma::Rec601),
            "rec709" | "rec_709" | "709" | "bt709" => Ok(Luma::Rec709),
            "average" | "avg" | "mean" => Ok(Luma::Average),
            "max" | "value" | "maximum" => Ok(Luma::Max),
            other => Err(format!(
                "luma must be one of rec601, rec709, average, max (got \"{other}\")"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Luma::Rec601 => "rec601",
            Luma::Rec709 => "rec709",
            Luma::Average => "average",
            Luma::Max => "max",
        }
    }

    /// Human-readable formula, for the report.
    pub fn formula(self) -> &'static str {
        match self {
            Luma::Rec601 => "0.299R + 0.587G + 0.114B (Rec. 601)",
            Luma::Rec709 => "0.2126R + 0.7152G + 0.0722B (Rec. 709)",
            Luma::Average => "(R + G + B) / 3",
            Luma::Max => "max(R, G, B)",
        }
    }

    fn level(self, r: u8, g: u8, b: u8) -> u8 {
        let (r, g, b) = (f64::from(r), f64::from(g), f64::from(b));
        let v = match self {
            Luma::Rec601 => 0.299 * r + 0.587 * g + 0.114 * b,
            Luma::Rec709 => 0.2126 * r + 0.7152 * g + 0.0722 * b,
            Luma::Average => (r + g + b) / 3.0,
            Luma::Max => r.max(g).max(b),
        };
        v.round().clamp(0.0, 255.0) as u8
    }
}

/// Everything the analyser was asked to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Number of buckets in the REPORTED histogram, 2-256. Statistics are always
    /// computed at full 256-level precision.
    pub bins: u32,
    /// Luma weighting for the brightness histogram.
    pub luma: Luma,
    /// Levels within this distance of an end count as clipped (0 = only the
    /// exact 0 / 255 levels).
    pub clip_margin: u8,
    /// Percent of pixels that must be pinned at an end before clipping is
    /// FLAGGED, 0-100.
    pub clip_percent: f64,
    /// Exclude fully transparent pixels from the counts.
    pub ignore_transparent: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            bins: 256,
            luma: Luma::Rec601,
            clip_margin: 0,
            clip_percent: 0.5,
            ignore_transparent: true,
        }
    }
}

/// One channel's distribution, all of it derived from the exact 256-level counts.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChannelStats {
    /// "red", "green", "blue" or "luma".
    pub channel: &'static str,
    pub min: u8,
    pub max: u8,
    pub mean: f64,
    pub median: u8,
    /// Standard deviation of the level, 0-127.5.
    pub stddev: f64,
    /// 1st percentile level — the black point that ignores stray dark pixels.
    pub p1: u8,
    pub p5: u8,
    pub p95: u8,
    /// 99th percentile level — the white point that ignores stray specular dots.
    pub p99: u8,
    /// The most common level (lowest on a tie).
    pub peak_level: u8,
    /// Share of pixels at `peak_level`.
    pub peak_percent: f64,
    /// Distinct levels that occur at least once, 1-256.
    pub levels_used: u32,
    pub clipped_shadow_pixels: u64,
    pub clipped_shadow_percent: f64,
    pub clipped_highlight_pixels: u64,
    pub clipped_highlight_percent: f64,
}

/// The reported histogram: `bins` buckets per channel, plus the level range each
/// bucket covers so a chart or CSV can be drawn straight from it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Histogram {
    pub bins: u32,
    /// Inclusive first level of each bucket.
    pub bin_start: Vec<u16>,
    /// Inclusive last level of each bucket.
    pub bin_end: Vec<u16>,
    pub red: Vec<u64>,
    pub green: Vec<u64>,
    pub blue: Vec<u64>,
    pub luma: Vec<u64>,
}

/// The full analysis.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Analysis {
    pub width: u32,
    pub height: u32,
    /// Every pixel in the frame.
    pub total_pixels: u64,
    /// Pixels that went into the histograms (all of them unless transparent
    /// pixels were excluded).
    pub counted_pixels: u64,
    pub transparent_pixels: u64,
    pub transparent_percent: f64,
    pub has_alpha: bool,
    /// Decoded source format, e.g. "png".
    pub format: String,
    pub luma_mode: &'static str,
    pub luma_formula: &'static str,
    pub red: ChannelStats,
    pub green: ChannelStats,
    pub blue: ChannelStats,
    pub luma: ChannelStats,
    /// p99 - p1 of luma, in levels (0-255).
    pub dynamic_range_levels: u16,
    /// The same spread expressed in photographic stops, log2((p99+1)/(p1+1)).
    pub dynamic_range_stops: f64,
    /// Shannon entropy of the 256-level luma histogram, 0-8 bits.
    pub entropy: f64,
    /// Share of counted pixels with luma 0-84 / 85-169 / 170-255.
    pub shadow_percent: f64,
    pub midtone_percent: f64,
    pub highlight_percent: f64,
    /// True when the shadow-clipped share of ANY channel (or luma) exceeds
    /// `clip_percent`.
    pub shadow_clipped: bool,
    pub highlight_clipped: bool,
    /// "underexposed", "balanced" or "overexposed".
    pub exposure: &'static str,
    /// "low", "normal" or "high".
    pub contrast: &'static str,
    /// "neutral", "warm", "cool", "green", "magenta" or a single channel name.
    pub color_cast: &'static str,
    /// How far, in levels, the most deviant channel mean sits from the average
    /// of the three channel means.
    pub color_cast_levels: f64,
    /// Plain-English justification of the exposure/clipping verdict.
    pub reason: String,
    pub warnings: Vec<String>,
    pub histogram: Histogram,
    /// The clip margin actually used (echoed so a report is self-describing).
    pub clip_margin: u8,
    pub clip_percent: f64,
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Level counts for one channel, always at full 256-level precision.
#[derive(Clone)]
struct Counter {
    counts: [u64; 256],
    n: u64,
}

impl Counter {
    fn new() -> Self {
        Counter {
            counts: [0; 256],
            n: 0,
        }
    }

    fn add(&mut self, level: u8) {
        self.counts[level as usize] += 1;
        self.n += 1;
    }

    /// Lowest level whose cumulative share reaches `p` percent.
    fn percentile(&self, p: f64) -> u8 {
        if self.n == 0 {
            return 0;
        }
        let target = (p / 100.0 * self.n as f64).ceil().max(1.0) as u64;
        let mut seen = 0u64;
        for (level, c) in self.counts.iter().enumerate() {
            seen += c;
            if seen >= target {
                return level as u8;
            }
        }
        255
    }

    fn stats(&self, channel: &'static str, margin: u8) -> ChannelStats {
        let n = self.n.max(1) as f64;
        let sum: f64 = self
            .counts
            .iter()
            .enumerate()
            .map(|(l, c)| l as f64 * *c as f64)
            .sum();
        let mean = sum / n;
        let var: f64 = self
            .counts
            .iter()
            .enumerate()
            .map(|(l, c)| (l as f64 - mean).powi(2) * *c as f64)
            .sum::<f64>()
            / n;
        let min = self.counts.iter().position(|c| *c > 0).unwrap_or(0) as u8;
        let max = self.counts.iter().rposition(|c| *c > 0).unwrap_or(0) as u8;
        let (peak_level, peak_count) =
            self.counts
                .iter()
                .enumerate()
                .fold((0usize, 0u64), |(bl, bc), (l, c)| {
                    if *c > bc {
                        (l, *c)
                    } else {
                        (bl, bc)
                    }
                });
        let low_end = usize::from(margin);
        let high_start = 255 - usize::from(margin);
        let shadow: u64 = self.counts[..=low_end].iter().sum();
        let highlight: u64 = self.counts[high_start..].iter().sum();
        ChannelStats {
            channel,
            min,
            max,
            mean: round2(mean),
            median: self.percentile(50.0),
            stddev: round2(var.sqrt()),
            p1: self.percentile(1.0),
            p5: self.percentile(5.0),
            p95: self.percentile(95.0),
            p99: self.percentile(99.0),
            peak_level: peak_level as u8,
            peak_percent: round2(peak_count as f64 / n * 100.0),
            levels_used: self.counts.iter().filter(|c| **c > 0).count() as u32,
            clipped_shadow_pixels: shadow,
            clipped_shadow_percent: round3(shadow as f64 / n * 100.0),
            clipped_highlight_pixels: highlight,
            clipped_highlight_percent: round3(highlight as f64 / n * 100.0),
        }
    }

    /// Fold the 256 levels into `bins` buckets, level `l` landing in bucket
    /// `l * bins / 256`.
    fn binned(&self, bins: u32) -> Vec<u64> {
        let mut out = vec![0u64; bins as usize];
        for (l, c) in self.counts.iter().enumerate() {
            let idx = (l as u32 * bins / 256) as usize;
            out[idx] += *c;
        }
        out
    }

    /// Shannon entropy of the distribution, in bits.
    fn entropy(&self) -> f64 {
        if self.n == 0 {
            return 0.0;
        }
        let n = self.n as f64;
        -self
            .counts
            .iter()
            .filter(|c| **c > 0)
            .map(|c| {
                let p = *c as f64 / n;
                p * p.log2()
            })
            .sum::<f64>()
    }

    /// Share of pixels in the inclusive level range.
    fn share(&self, lo: usize, hi: usize) -> f64 {
        if self.n == 0 {
            return 0.0;
        }
        let s: u64 = self.counts[lo..=hi].iter().sum();
        s as f64 / self.n as f64 * 100.0
    }
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

fn cast_of(dr: f64, dg: f64, db: f64) -> &'static str {
    let strongest = [("red", dr), ("green", dg), ("blue", db)]
        .into_iter()
        .fold(("red", 0.0f64), |best, (name, d)| {
            if d.abs() > best.1.abs() {
                (name, d)
            } else {
                best
            }
        });
    match strongest {
        _ if dr > 0.0 && db < 0.0 => "warm",
        _ if db > 0.0 && dr < 0.0 => "cool",
        _ if dg > 0.0 && dr < 0.0 && db < 0.0 => "green",
        _ if dg < 0.0 && dr > 0.0 && db > 0.0 => "magenta",
        (name, d) if d > 0.0 => match name {
            "red" => "red",
            "green" => "green",
            _ => "blue",
        },
        (name, _) => match name {
            // The most deviant channel is the one that is MISSING, so the cast
            // is the complement of that channel.
            "red" => "cool",
            "green" => "magenta",
            _ => "warm",
        },
    }
}

/// Analyse the encoded image `bytes` and return its histograms plus the
/// exposure verdict they support.
pub fn analyze(bytes: &[u8], opts: &Options) -> Result<Analysis, String> {
    if bytes.is_empty() {
        return Err("no image data was provided".into());
    }
    if !(2..=256).contains(&opts.bins) {
        return Err(format!(
            "bins must be between 2 and 256 (got {}) — 256 keeps every level, 64 or 32 make a \
             chart-sized summary",
            opts.bins
        ));
    }
    if opts.clip_margin > 32 {
        return Err(format!(
            "clip_margin must be between 0 and 32 levels (got {})",
            opts.clip_margin
        ));
    }
    if !(0.0..=100.0).contains(&opts.clip_percent) {
        return Err(format!(
            "clip_percent must be between 0 and 100 (got {})",
            opts.clip_percent
        ));
    }

    let (img, format) = decode(bytes)?;
    let (w, h) = (img.width(), img.height());
    let total = u64::from(w) * u64::from(h);

    let mut red = Counter::new();
    let mut green = Counter::new();
    let mut blue = Counter::new();
    let mut luma = Counter::new();
    let mut transparent = 0u64;
    let mut has_alpha = false;

    for px in img.pixels() {
        let [r, g, b, a] = px.0;
        if a < 255 {
            has_alpha = true;
        }
        if a < ALPHA_THRESHOLD {
            transparent += 1;
            if opts.ignore_transparent {
                continue;
            }
        }
        red.add(r);
        green.add(g);
        blue.add(b);
        luma.add(opts.luma.level(r, g, b));
    }

    let counted = luma.n;
    if counted == 0 {
        return Err(
            "every pixel is fully transparent, so there is nothing to measure — pass \
             ignore_transparent=false to analyse the RGB values stored underneath the alpha"
                .into(),
        );
    }

    let mut warnings: Vec<String> = Vec::new();
    if transparent > 0 && opts.ignore_transparent {
        warnings.push(format!(
            "{transparent} transparent pixel(s) ({:.2}% of the frame) were excluded from the \
             histogram; pass ignore_transparent=false to include them",
            transparent as f64 / total as f64 * 100.0
        ));
    }
    if total < 100 {
        warnings.push(format!(
            "only {total} pixel(s) — percentiles and the exposure verdict are coarse on an image \
             this small"
        ));
    }

    let r_stats = red.stats("red", opts.clip_margin);
    let g_stats = green.stats("green", opts.clip_margin);
    let b_stats = blue.stats("blue", opts.clip_margin);
    let l_stats = luma.stats("luma", opts.clip_margin);

    let shadow_clipped = [&r_stats, &g_stats, &b_stats, &l_stats]
        .iter()
        .any(|s| s.clipped_shadow_percent > opts.clip_percent);
    let highlight_clipped = [&r_stats, &g_stats, &b_stats, &l_stats]
        .iter()
        .any(|s| s.clipped_highlight_percent > opts.clip_percent);

    let exposure = if l_stats.mean < DARK_MEAN {
        "underexposed"
    } else if l_stats.mean > BRIGHT_MEAN {
        "overexposed"
    } else {
        "balanced"
    };
    let contrast = if l_stats.stddev < LOW_CONTRAST_STDDEV {
        "low"
    } else if l_stats.stddev > HIGH_CONTRAST_STDDEV {
        "high"
    } else {
        "normal"
    };

    let channel_avg = (r_stats.mean + g_stats.mean + b_stats.mean) / 3.0;
    let (dr, dg, db) = (
        r_stats.mean - channel_avg,
        g_stats.mean - channel_avg,
        b_stats.mean - channel_avg,
    );
    let cast_levels = dr.abs().max(dg.abs()).max(db.abs());
    let color_cast = if cast_levels < CAST_LEVELS {
        "neutral"
    } else {
        cast_of(dr, dg, db)
    };

    let dynamic_range_levels = u16::from(l_stats.p99).saturating_sub(u16::from(l_stats.p1));
    let dynamic_range_stops =
        ((f64::from(l_stats.p99) + 1.0) / (f64::from(l_stats.p1) + 1.0)).log2();

    let reason = {
        let mut parts = Vec::new();
        parts.push(format!(
            "mean luma {:.1} of 255 reads as {exposure}",
            l_stats.mean
        ));
        parts.push(format!(
            "{contrast} contrast (standard deviation {:.1})",
            l_stats.stddev
        ));
        if highlight_clipped {
            parts.push(format!(
                "highlights are clipped ({:.2}% of pixels at level {}+)",
                l_stats.clipped_highlight_percent,
                255 - u16::from(opts.clip_margin)
            ));
        }
        if shadow_clipped {
            parts.push(format!(
                "shadows are clipped ({:.2}% of pixels at level {} or below)",
                l_stats.clipped_shadow_percent,
                opts.clip_margin
            ));
        }
        if !highlight_clipped && !shadow_clipped {
            parts.push(format!(
                "nothing is clipped beyond the {:.2}% tolerance",
                opts.clip_percent
            ));
        }
        if color_cast != "neutral" {
            parts.push(format!(
                "a {color_cast} colour cast ({cast_levels:.1} levels between the channel means)"
            ));
        }
        format!("{}.", parts.join("; "))
    };

    let bins = opts.bins;
    let bin_start: Vec<u16> = (0..bins).map(|i| (i * 256 / bins) as u16).collect();
    let bin_end: Vec<u16> = (0..bins)
        .map(|i| (((i + 1) * 256 / bins).saturating_sub(1).min(255)) as u16)
        .collect();

    Ok(Analysis {
        width: w,
        height: h,
        total_pixels: total,
        counted_pixels: counted,
        transparent_pixels: transparent,
        transparent_percent: round2(transparent as f64 / total as f64 * 100.0),
        has_alpha,
        format,
        luma_mode: opts.luma.as_str(),
        luma_formula: opts.luma.formula(),
        red: r_stats,
        green: g_stats,
        blue: b_stats,
        dynamic_range_levels,
        dynamic_range_stops: round2(dynamic_range_stops),
        entropy: round2(luma.entropy()),
        shadow_percent: round2(luma.share(0, 84)),
        midtone_percent: round2(luma.share(85, 169)),
        highlight_percent: round2(luma.share(170, 255)),
        shadow_clipped,
        highlight_clipped,
        exposure,
        contrast,
        color_cast,
        color_cast_levels: round2(cast_levels),
        reason,
        warnings,
        histogram: Histogram {
            bins,
            bin_start,
            bin_end,
            red: red.binned(bins),
            green: green.binned(bins),
            blue: blue.binned(bins),
            luma: luma.binned(bins),
        },
        luma: l_stats,
        clip_margin: opts.clip_margin,
        clip_percent: opts.clip_percent,
    })
}

/// The histogram as CSV — one row per reported bin, ready for a spreadsheet.
pub fn histogram_csv(a: &Analysis) -> String {
    let h = &a.histogram;
    let mut out = String::from("bin,level_start,level_end,red,green,blue,luma\n");
    for i in 0..h.bins as usize {
        out.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            i, h.bin_start[i], h.bin_end[i], h.red[i], h.green[i], h.blue[i], h.luma[i]
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, RgbaImage};

    /// Encode a generated raster as PNG so the tests exercise the real decode path.
    fn png(img: &RgbaImage) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img.clone())
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    fn solid(w: u32, h: u32, px: [u8; 4]) -> Vec<u8> {
        png(&RgbaImage::from_pixel(w, h, image::Rgba(px)))
    }

    /// Left half pure black, right half pure white.
    fn half_and_half() -> Vec<u8> {
        let mut img = RgbaImage::new(16, 16);
        for (x, _y, p) in img.enumerate_pixels_mut() {
            *p = if x < 8 {
                image::Rgba([0, 0, 0, 255])
            } else {
                image::Rgba([255, 255, 255, 255])
            };
        }
        png(&img)
    }

    /// A 256x1 ramp: exactly one pixel per level.
    fn ramp() -> Vec<u8> {
        let mut img = RgbaImage::new(256, 1);
        for (x, _y, p) in img.enumerate_pixels_mut() {
            let v = x as u8;
            *p = image::Rgba([v, v, v, 255]);
        }
        png(&img)
    }

    #[test]
    fn flat_gray_has_no_spread_and_no_clipping() {
        let a = analyze(&solid(8, 8, [128, 128, 128, 255]), &Options::default()).unwrap();
        assert_eq!((a.width, a.height, a.total_pixels), (8, 8, 64));
        assert_eq!(a.counted_pixels, 64);
        assert_eq!(a.luma.mean, 128.0);
        assert_eq!(a.luma.stddev, 0.0);
        assert_eq!(a.luma.levels_used, 1);
        assert_eq!(a.luma.peak_level, 128);
        assert_eq!(a.luma.peak_percent, 100.0);
        assert_eq!(a.entropy, 0.0);
        assert_eq!(a.dynamic_range_levels, 0);
        assert_eq!(a.exposure, "balanced");
        assert_eq!(a.contrast, "low");
        assert_eq!(a.color_cast, "neutral");
        assert!(!a.shadow_clipped && !a.highlight_clipped);
        assert_eq!(a.format, "png");
        assert_eq!(a.midtone_percent, 100.0);
    }

    #[test]
    fn black_and_white_halves_clip_at_both_ends() {
        let a = analyze(&half_and_half(), &Options::default()).unwrap();
        assert_eq!(a.luma.mean, 127.5);
        assert_eq!(a.luma.clipped_shadow_percent, 50.0);
        assert_eq!(a.luma.clipped_highlight_percent, 50.0);
        assert!(a.shadow_clipped && a.highlight_clipped);
        assert_eq!(a.contrast, "high");
        assert_eq!(a.dynamic_range_levels, 255);
        assert_eq!(a.entropy, 1.0, "two equally likely levels = 1 bit");
        assert!(a.reason.contains("highlights are clipped"), "{}", a.reason);
        assert!(a.reason.contains("shadows are clipped"), "{}", a.reason);
    }

    #[test]
    fn a_ramp_is_evenly_distributed_and_stays_under_the_clip_tolerance() {
        let a = analyze(&ramp(), &Options::default()).unwrap();
        assert_eq!(a.luma.levels_used, 256);
        assert_eq!(a.luma.min, 0);
        assert_eq!(a.luma.max, 255);
        assert_eq!(a.luma.median, 127);
        // One pixel per level = 0.39%, under the 0.5% default tolerance.
        assert_eq!(a.luma.clipped_shadow_percent, 0.391);
        assert!(
            !a.shadow_clipped && !a.highlight_clipped,
            "0.39% must not trip the 0.5% tolerance"
        );
        assert_eq!(a.entropy, 8.0, "256 equally likely levels = 8 bits");
        assert_eq!(a.histogram.luma.iter().sum::<u64>(), 256);
    }

    #[test]
    fn clip_percent_zero_flags_any_pinned_pixel() {
        let opts = Options {
            clip_percent: 0.0,
            ..Options::default()
        };
        let a = analyze(&ramp(), &opts).unwrap();
        assert!(a.shadow_clipped && a.highlight_clipped);
    }

    #[test]
    fn clip_margin_widens_what_counts_as_clipped() {
        let opts = Options {
            clip_margin: 4,
            ..Options::default()
        };
        let a = analyze(&ramp(), &opts).unwrap();
        // Levels 0-4 and 251-255: 5 of 256 pixels at each end.
        assert_eq!(a.luma.clipped_shadow_pixels, 5);
        assert_eq!(a.luma.clipped_highlight_pixels, 5);
        assert_eq!(a.clip_margin, 4);
    }

    #[test]
    fn bins_reshape_the_histogram_without_changing_the_statistics() {
        let full = analyze(&ramp(), &Options::default()).unwrap();
        let binned = analyze(
            &ramp(),
            &Options {
                bins: 16,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(full.luma.mean, binned.luma.mean);
        assert_eq!(full.luma.stddev, binned.luma.stddev);
        assert_eq!(binned.histogram.bins, 16);
        assert_eq!(binned.histogram.luma.len(), 16);
        assert!(binned.histogram.luma.iter().all(|c| *c == 16));
        assert_eq!(binned.histogram.bin_start[0], 0);
        assert_eq!(binned.histogram.bin_end[0], 15);
        assert_eq!(binned.histogram.bin_end[15], 255);
    }

    #[test]
    fn luma_mode_changes_the_brightness_reading() {
        let red_frame = solid(4, 4, [255, 0, 0, 255]);
        let r601 = analyze(&red_frame, &Options::default()).unwrap();
        let r709 = analyze(
            &red_frame,
            &Options {
                luma: Luma::Rec709,
                ..Options::default()
            },
        )
        .unwrap();
        let avg = analyze(
            &red_frame,
            &Options {
                luma: Luma::Average,
                ..Options::default()
            },
        )
        .unwrap();
        let max = analyze(
            &red_frame,
            &Options {
                luma: Luma::Max,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(r601.luma.mean, 76.0); // 0.299 * 255
        assert_eq!(r709.luma.mean, 54.0); // 0.2126 * 255
        assert_eq!(avg.luma.mean, 85.0);
        assert_eq!(max.luma.mean, 255.0);
        assert_eq!(r601.luma_mode, "rec601");
        assert!(r709.luma_formula.contains("Rec. 709"));
        // max() sees the blown red channel as a clipped highlight, weighted luma
        // does not — which is exactly why the mode is exposed. The RED channel is
        // pinned either way, so the frame-level verdict stays clipped.
        assert_eq!(max.luma.clipped_highlight_percent, 100.0);
        assert_eq!(r601.luma.clipped_highlight_percent, 0.0);
        assert_eq!(r601.red.clipped_highlight_percent, 100.0);
        assert!(r601.highlight_clipped && max.highlight_clipped);
    }

    #[test]
    fn a_channel_imbalance_is_reported_as_a_colour_cast() {
        let warm = analyze(&solid(4, 4, [200, 128, 100, 255]), &Options::default()).unwrap();
        assert_eq!(warm.color_cast, "warm");
        assert!(warm.color_cast_levels > 0.0);
        assert!(warm.reason.contains("warm colour cast"), "{}", warm.reason);

        let cool = analyze(&solid(4, 4, [90, 120, 200, 255]), &Options::default()).unwrap();
        assert_eq!(cool.color_cast, "cool");

        let neutral = analyze(&solid(4, 4, [120, 121, 120, 255]), &Options::default()).unwrap();
        assert_eq!(neutral.color_cast, "neutral");
    }

    #[test]
    fn exposure_verdicts_follow_the_mean() {
        let dark = analyze(&solid(4, 4, [20, 20, 20, 255]), &Options::default()).unwrap();
        assert_eq!(dark.exposure, "underexposed");
        assert_eq!(dark.shadow_percent, 100.0);
        let bright = analyze(&solid(4, 4, [250, 250, 250, 255]), &Options::default()).unwrap();
        assert_eq!(bright.exposure, "overexposed");
        assert_eq!(bright.highlight_percent, 100.0);
    }

    #[test]
    fn transparent_pixels_are_excluded_by_default_and_countable_on_request() {
        let mut img = RgbaImage::new(4, 4);
        for (x, _y, p) in img.enumerate_pixels_mut() {
            *p = if x < 2 {
                image::Rgba([255, 255, 255, 0]) // transparent, junk-white RGB
            } else {
                image::Rgba([10, 10, 10, 255])
            };
        }
        let bytes = png(&img);

        let skipped = analyze(&bytes, &Options::default()).unwrap();
        assert_eq!(skipped.transparent_pixels, 8);
        assert_eq!(skipped.counted_pixels, 8);
        assert_eq!(skipped.luma.mean, 10.0);
        assert!(skipped.has_alpha);
        assert!(skipped.warnings.iter().any(|w| w.contains("excluded")));

        let included = analyze(
            &bytes,
            &Options {
                ignore_transparent: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(included.counted_pixels, 16);
        assert_eq!(included.luma.mean, 132.5);
    }

    #[test]
    fn a_fully_transparent_image_errors_with_the_way_out() {
        let err = analyze(&solid(4, 4, [0, 0, 0, 0]), &Options::default()).unwrap_err();
        assert!(err.contains("ignore_transparent=false"), "{err}");
    }

    #[test]
    fn bad_input_is_rejected_with_an_actionable_message() {
        assert!(analyze(b"", &Options::default())
            .unwrap_err()
            .contains("no image data"));
        assert!(analyze(b"not an image at all", &Options::default())
            .unwrap_err()
            .contains("could not"));
        let err = analyze(
            &solid(2, 2, [0, 0, 0, 255]),
            &Options {
                bins: 300,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("bins must be between 2 and 256"), "{err}");
        let err = analyze(
            &solid(2, 2, [0, 0, 0, 255]),
            &Options {
                clip_margin: 40,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("clip_margin"), "{err}");
    }

    #[test]
    fn csv_export_has_a_header_and_one_row_per_bin() {
        let a = analyze(
            &half_and_half(),
            &Options {
                bins: 4,
                ..Options::default()
            },
        )
        .unwrap();
        let csv = histogram_csv(&a);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "bin,level_start,level_end,red,green,blue,luma");
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[1], "0,0,63,128,128,128,128");
        assert_eq!(lines[4], "3,192,255,128,128,128,128");
    }

    #[test]
    fn luma_mode_parsing_accepts_the_common_spellings() {
        assert_eq!(Luma::parse("rec601").unwrap(), Luma::Rec601);
        assert_eq!(Luma::parse("REC-709").unwrap(), Luma::Rec709);
        assert_eq!(Luma::parse(" average ").unwrap(), Luma::Average);
        assert_eq!(Luma::parse("value").unwrap(), Luma::Max);
        assert!(Luma::parse("hsl").unwrap_err().contains("rec601"));
    }
}
