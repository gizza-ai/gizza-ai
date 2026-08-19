//! image-region-change-grid core — divide two aligned images into a
//! `columns × rows` grid and report which cells changed and by how much.
//!
//! No wafer/wasm-bindgen deps; pure compute over already-fetched image bytes,
//! so it runs on every backend (chat Service Worker + native CLI).
//!
//! Pipeline:
//!   - Decode both images with the pure-Rust `image` crate (no
//!     `fast_image_resize`, so the module instantiates under `wafer build` with
//!     wasm SIMD off) and flatten each to RGBA8.
//!   - The FIRST image ("before") defines the canvas. If the second differs in
//!     size it is either resized onto that canvas (`SizeMismatch::Resize`,
//!     the default) or rejected (`SizeMismatch::Error`).
//!   - Per pixel, compute a difference in PERCENT of the maximum possible
//!     difference (0 = identical, 100 = maximally different) using the chosen
//!     [`Metric`]. A pixel counts as *changed* when that difference is strictly
//!     greater than `threshold`.
//!   - The canvas is partitioned into `columns × rows` cells (boundaries are
//!     computed with integer math so every pixel belongs to exactly one cell,
//!     even when the dimensions do not divide evenly). Each cell reports its
//!     changed-pixel count/percentage plus the mean and max per-pixel delta.
//!   - A cell is FLAGGED as changed when it has at least one changed pixel and
//!     its changed-percentage is at or above `min_change` — the noise filter
//!     that block-wise comparison exists for.
//!   - Optionally an ASCII density map renders the grid at a glance.

use std::io::Cursor;

use image::{imageops::FilterType, GenericImageView, RgbaImage};
use serde::Serialize;

/// Decode guard: reject any single image larger than this many pixels so a huge
/// input can't OOM-trap the 64 MiB chat runtime. 24 MP ≈ a 6000×4000 photo.
pub const MAX_PIXELS: u64 = 24_000_000;

/// Upper bound on each grid axis. 32 × 32 = 1024 cells, which is already far
/// past "compact summary" territory but keeps the report bounded.
pub const MAX_AXIS: u32 = 32;

/// How a single pixel's difference is measured. Every metric is normalised to
/// 0-100 percent of the maximum possible difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    /// Root-mean-square difference across the red, green, blue AND alpha
    /// channels. The strict pixel-diff default.
    Rgb,
    /// Absolute difference of perceived brightness only (Rec. 601 luma:
    /// 0.299 R + 0.587 G + 0.114 B). Ignores hue and alpha, so a recolour that
    /// keeps the same brightness reads as unchanged.
    Luma,
    /// The single largest per-channel difference (R, G, B or alpha). The
    /// strictest metric: one badly-off channel is enough.
    MaxChannel,
}

impl Metric {
    pub fn as_str(self) -> &'static str {
        match self {
            Metric::Rgb => "rgb",
            Metric::Luma => "luma",
            Metric::MaxChannel => "max-channel",
        }
    }
}

pub fn parse_metric(s: &str) -> Result<Metric, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "rgb" => Ok(Metric::Rgb),
        "luma" | "brightness" | "gray" | "grey" => Ok(Metric::Luma),
        "max-channel" | "max_channel" | "maxchannel" | "max" => Ok(Metric::MaxChannel),
        other => Err(format!(
            "metric {other:?} not supported (rgb|luma|max-channel)"
        )),
    }
}

/// What to do when the two images do not have identical dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeMismatch {
    /// Scale the second image onto the first image's canvas (Lanczos3).
    Resize,
    /// Refuse to compare images of different sizes.
    Error,
}

pub fn parse_size_mismatch(s: &str) -> Result<SizeMismatch, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "resize" | "scale" => Ok(SizeMismatch::Resize),
        "error" | "fail" | "strict" => Ok(SizeMismatch::Error),
        other => Err(format!(
            "size_mismatch {other:?} not supported (resize|error)"
        )),
    }
}

/// Everything the comparison needs beyond the two images themselves.
#[derive(Debug, Clone)]
pub struct Options {
    pub columns: u32,
    pub rows: u32,
    /// Percent (0-100). A pixel counts as changed when its difference is
    /// strictly greater than this.
    pub threshold: f64,
    /// Percent (0-100). A cell is flagged when its changed-pixel percentage is
    /// at or above this (and it has at least one changed pixel).
    pub min_change: f64,
    pub metric: Metric,
    pub size_mismatch: SizeMismatch,
    pub map: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            columns: 4,
            rows: 4,
            threshold: 2.0,
            min_change: 1.0,
            metric: Metric::Rgb,
            size_mismatch: SizeMismatch::Resize,
            map: true,
        }
    }
}

/// What we can say about one of the two inputs.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ImageFacts {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub bytes: u64,
}

/// One grid cell's verdict.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Cell {
    /// Spreadsheet-style label: column letter + 1-based row number (e.g. `C2`).
    pub cell: String,
    /// 1-based row index, top to bottom.
    pub row: u32,
    /// 1-based column index, left to right.
    pub column: u32,
    /// Pixel rectangle this cell covers on the canvas.
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub pixels: u64,
    pub changed_pixels: u64,
    /// Share of this cell's pixels that changed, 0-100.
    pub changed_percent: f64,
    /// Average per-pixel difference over the whole cell, 0-100.
    pub mean_delta_percent: f64,
    /// Largest single-pixel difference in the cell, 0-100.
    pub max_delta_percent: f64,
    /// True when the cell is flagged (>= `min_change`, with real changed pixels).
    pub changed: bool,
}

/// A ranked entry in the "biggest changes first" shortlist.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TopCell {
    pub cell: String,
    pub row: u32,
    pub column: u32,
    pub changed_percent: f64,
    pub mean_delta_percent: f64,
}

/// The ASCII density map: one string per grid row, plus what each glyph means.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ChangeMap {
    pub grid: Vec<String>,
    pub legend: Vec<String>,
}

/// The full report.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Report {
    /// Canvas the comparison ran on (the first image's dimensions).
    pub width: u32,
    pub height: u32,
    pub columns: u32,
    pub rows: u32,
    pub metric: String,
    pub threshold_percent: f64,
    pub min_change_percent: f64,
    /// True when the second image had to be scaled onto the canvas.
    pub resized: bool,
    pub before: ImageFacts,
    pub after: ImageFacts,
    pub total_pixels: u64,
    pub changed_pixels: u64,
    pub changed_percent: f64,
    pub mean_delta_percent: f64,
    pub max_delta_percent: f64,
    pub total_cells: u32,
    pub changed_cells: u32,
    pub cells: Vec<Cell>,
    /// Up to 5 flagged cells, most-changed first. Empty when nothing changed.
    pub top_cells: Vec<TopCell>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map: Option<ChangeMap>,
    pub summary: String,
}

/// Round to 2 decimals so reports are stable and exact-output testable.
fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Spreadsheet column label: 0 → `A`, 25 → `Z`, 26 → `AA`.
fn column_label(index: u32) -> String {
    let mut i = index;
    let mut s = String::new();
    loop {
        s.insert(0, (b'A' + (i % 26) as u8) as char);
        if i < 26 {
            break;
        }
        i = i / 26 - 1;
    }
    s
}

/// Density glyph for a cell's changed-percentage.
fn glyph(changed_percent: f64) -> char {
    if changed_percent <= 0.0 {
        '.'
    } else if changed_percent < 10.0 {
        ':'
    } else if changed_percent < 25.0 {
        '-'
    } else if changed_percent < 50.0 {
        '='
    } else if changed_percent < 75.0 {
        '+'
    } else if changed_percent < 90.0 {
        '*'
    } else {
        '#'
    }
}

fn legend() -> Vec<String> {
    [
        ". no changed pixels",
        ": up to 10% of the cell changed",
        "- 10-25%",
        "= 25-50%",
        "+ 50-75%",
        "* 75-90%",
        "# 90-100%",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Per-pixel difference in percent of the maximum possible difference.
fn delta_percent(a: &[u8; 4], b: &[u8; 4], metric: Metric) -> f64 {
    match metric {
        Metric::Rgb => {
            let mut sum = 0.0f64;
            for i in 0..4 {
                let d = a[i] as f64 - b[i] as f64;
                sum += d * d;
            }
            (sum / 4.0).sqrt() / 255.0 * 100.0
        }
        Metric::Luma => {
            let l = |p: &[u8; 4]| 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64;
            (l(a) - l(b)).abs() / 255.0 * 100.0
        }
        Metric::MaxChannel => {
            let mut m = 0u8;
            for i in 0..4 {
                let d = a[i].abs_diff(b[i]);
                if d > m {
                    m = d;
                }
            }
            m as f64 / 255.0 * 100.0
        }
    }
}

/// Decode one input to RGBA8, reporting its declared format and byte size.
fn decode(bytes: &[u8], which: &str) -> Result<(RgbaImage, ImageFacts), String> {
    if bytes.is_empty() {
        return Err(format!("the {which} image is empty"));
    }
    let format = image::guess_format(bytes)
        .map(|f| format!("{f:?}").to_ascii_lowercase())
        .unwrap_or_else(|_| "unknown".to_string());
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("could not read the {which} image: {e}"))?;
    let (w, h) = reader
        .into_dimensions()
        .map_err(|e| format!("could not read the {which} image header: {e}"))?;
    if w == 0 || h == 0 {
        return Err(format!("the {which} image has a zero dimension"));
    }
    if w as u64 * h as u64 > MAX_PIXELS {
        return Err(format!(
            "the {which} image is {w}x{h} ({} MP), over the {} MP limit",
            r2(w as f64 * h as f64 / 1_000_000.0),
            MAX_PIXELS / 1_000_000
        ));
    }
    let decoded = image::load_from_memory(bytes)
        .map_err(|e| format!("could not decode the {which} image: {e}"))?;
    let (dw, dh) = decoded.dimensions();
    Ok((
        decoded.to_rgba8(),
        ImageFacts {
            width: dw,
            height: dh,
            format,
            bytes: bytes.len() as u64,
        },
    ))
}

/// Compare two aligned images cell by cell.
///
/// `before` defines the canvas; `after` is the image being checked against it.
pub fn compare(before: &[u8], after: &[u8], opts: &Options) -> Result<Report, String> {
    if opts.columns == 0 || opts.rows == 0 {
        return Err("columns and rows must be at least 1".to_string());
    }
    if opts.columns > MAX_AXIS || opts.rows > MAX_AXIS {
        return Err(format!(
            "columns and rows must be at most {MAX_AXIS} (got {}x{})",
            opts.columns, opts.rows
        ));
    }
    if !(0.0..=100.0).contains(&opts.threshold) {
        return Err(format!(
            "threshold must be between 0 and 100 percent (got {})",
            opts.threshold
        ));
    }
    if !(0.0..=100.0).contains(&opts.min_change) {
        return Err(format!(
            "min_change must be between 0 and 100 percent (got {})",
            opts.min_change
        ));
    }

    let (base, before_facts) = decode(before, "first")?;
    let (mut other, after_facts) = decode(after, "second")?;

    let (w, h) = (base.width(), base.height());
    if opts.columns > w {
        return Err(format!(
            "columns ({}) cannot exceed the image width ({w}px)",
            opts.columns
        ));
    }
    if opts.rows > h {
        return Err(format!(
            "rows ({}) cannot exceed the image height ({h}px)",
            opts.rows
        ));
    }

    let mut resized = false;
    if other.width() != w || other.height() != h {
        match opts.size_mismatch {
            SizeMismatch::Error => {
                return Err(format!(
                    "the images are different sizes ({}x{} vs {}x{}); set size_mismatch=resize to scale the second onto the first",
                    w,
                    h,
                    other.width(),
                    other.height()
                ));
            }
            SizeMismatch::Resize => {
                other = image::imageops::resize(&other, w, h, FilterType::Lanczos3);
                resized = true;
            }
        }
    }

    // Integer cell boundaries: every pixel lands in exactly one cell even when
    // the dimensions do not divide evenly.
    let bound = |i: u32, n: u32, total: u32| -> u32 { (i as u64 * total as u64 / n as u64) as u32 };

    let mut cells: Vec<Cell> = Vec::with_capacity((opts.columns * opts.rows) as usize);
    let mut total_changed: u64 = 0;
    let mut total_delta_sum: f64 = 0.0;
    let mut overall_max: f64 = 0.0;

    for r in 0..opts.rows {
        let y0 = bound(r, opts.rows, h);
        let y1 = bound(r + 1, opts.rows, h);
        for c in 0..opts.columns {
            let x0 = bound(c, opts.columns, w);
            let x1 = bound(c + 1, opts.columns, w);

            let mut changed_pixels: u64 = 0;
            let mut delta_sum: f64 = 0.0;
            let mut max_delta: f64 = 0.0;
            for y in y0..y1 {
                for x in x0..x1 {
                    let a = base.get_pixel(x, y).0;
                    let b = other.get_pixel(x, y).0;
                    let d = delta_percent(&a, &b, opts.metric);
                    delta_sum += d;
                    if d > max_delta {
                        max_delta = d;
                    }
                    if d > opts.threshold {
                        changed_pixels += 1;
                    }
                }
            }

            let pixels = (x1 - x0) as u64 * (y1 - y0) as u64;
            let changed_percent = if pixels == 0 {
                0.0
            } else {
                changed_pixels as f64 / pixels as f64 * 100.0
            };
            let mean_delta = if pixels == 0 {
                0.0
            } else {
                delta_sum / pixels as f64
            };

            total_changed += changed_pixels;
            total_delta_sum += delta_sum;
            if max_delta > overall_max {
                overall_max = max_delta;
            }

            cells.push(Cell {
                cell: format!("{}{}", column_label(c), r + 1),
                row: r + 1,
                column: c + 1,
                x: x0,
                y: y0,
                width: x1 - x0,
                height: y1 - y0,
                pixels,
                changed_pixels,
                changed_percent: r2(changed_percent),
                mean_delta_percent: r2(mean_delta),
                max_delta_percent: r2(max_delta),
                changed: changed_pixels > 0 && changed_percent >= opts.min_change,
            });
        }
    }

    let total_pixels = w as u64 * h as u64;
    let changed_cells = cells.iter().filter(|c| c.changed).count() as u32;

    // Shortlist: flagged cells, biggest change first, ties broken by mean delta
    // then by reading order so the output is deterministic.
    let mut ranked: Vec<&Cell> = cells.iter().filter(|c| c.changed).collect();
    ranked.sort_by(|a, b| {
        b.changed_percent
            .partial_cmp(&a.changed_percent)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                b.mean_delta_percent
                    .partial_cmp(&a.mean_delta_percent)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.row.cmp(&b.row))
            .then(a.column.cmp(&b.column))
    });
    let top_cells: Vec<TopCell> = ranked
        .iter()
        .take(5)
        .map(|c| TopCell {
            cell: c.cell.clone(),
            row: c.row,
            column: c.column,
            changed_percent: c.changed_percent,
            mean_delta_percent: c.mean_delta_percent,
        })
        .collect();

    let map = if opts.map {
        let grid = (0..opts.rows)
            .map(|r| {
                (0..opts.columns)
                    .map(|c| glyph(cells[(r * opts.columns + c) as usize].changed_percent))
                    .collect::<String>()
            })
            .collect();
        Some(ChangeMap {
            grid,
            legend: legend(),
        })
    } else {
        None
    };

    let overall_changed_percent = total_changed as f64 / total_pixels as f64 * 100.0;
    let total_cells = opts.columns * opts.rows;
    let summary = if changed_cells == 0 {
        format!(
            "No cell changed by {}% or more ({}% of pixels differ across the {}x{} grid).",
            r2(opts.min_change),
            r2(overall_changed_percent),
            opts.columns,
            opts.rows
        )
    } else {
        let top = &top_cells[0];
        format!(
            "{} of {} cells changed ({}% of all pixels differ); the biggest change is in {} at {}% of that cell.",
            changed_cells,
            total_cells,
            r2(overall_changed_percent),
            top.cell,
            top.changed_percent
        )
    };

    Ok(Report {
        width: w,
        height: h,
        columns: opts.columns,
        rows: opts.rows,
        metric: opts.metric.as_str().to_string(),
        threshold_percent: r2(opts.threshold),
        min_change_percent: r2(opts.min_change),
        resized,
        before: before_facts,
        after: after_facts,
        total_pixels,
        changed_pixels: total_changed,
        changed_percent: r2(overall_changed_percent),
        mean_delta_percent: r2(total_delta_sum / total_pixels as f64),
        max_delta_percent: r2(overall_max),
        total_cells,
        changed_cells,
        cells,
        top_cells,
        map,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageEncoder, Rgba};

    /// Encode an RgbaImage to PNG bytes.
    fn png(img: &RgbaImage) -> Vec<u8> {
        let mut out = Vec::new();
        image::codecs::png::PngEncoder::new(&mut out)
            .write_image(
                img.as_raw(),
                img.width(),
                img.height(),
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        out
    }

    /// A solid-white 40x40 canvas.
    fn white() -> RgbaImage {
        RgbaImage::from_pixel(40, 40, Rgba([255, 255, 255, 255]))
    }

    /// White canvas with a solid black square filling one 10x10 grid cell.
    fn white_with_black_cell(col: u32, row: u32) -> RgbaImage {
        let mut img = white();
        for y in row * 10..row * 10 + 10 {
            for x in col * 10..col * 10 + 10 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        img
    }

    #[test]
    fn happy_path_flags_exactly_the_changed_cell() {
        let a = png(&white());
        let b = png(&white_with_black_cell(2, 1)); // column C, row 2
        let report = compare(&a, &b, &Options::default()).unwrap();

        assert_eq!((report.width, report.height), (40, 40));
        assert_eq!((report.columns, report.rows), (4, 4));
        assert_eq!(report.total_cells, 16);
        assert_eq!(report.cells.len(), 16);
        assert_eq!(report.changed_cells, 1);
        assert_eq!(report.changed_pixels, 100);
        assert_eq!(report.total_pixels, 1600);
        assert_eq!(report.changed_percent, 6.25);
        assert!(!report.resized);

        let hit = report.cells.iter().find(|c| c.cell == "C2").unwrap();
        assert!(hit.changed);
        assert_eq!(hit.changed_pixels, 100);
        assert_eq!(hit.changed_percent, 100.0);
        // White -> opaque black differs on 3 of the 4 channels, so the RMS over
        // R/G/B/A is sqrt(3/4) = 86.6% of the maximum possible difference.
        assert_eq!(hit.max_delta_percent, 86.6);
        assert_eq!(hit.mean_delta_percent, 86.6);
        assert_eq!((hit.x, hit.y, hit.width, hit.height), (20, 10, 10, 10));
        assert_eq!(report.cells.iter().filter(|c| c.changed).count(), 1);

        assert_eq!(report.top_cells[0].cell, "C2");
        assert_eq!(
            report.map.as_ref().unwrap().grid,
            vec![
                "....".to_string(),
                "..#.".to_string(),
                "....".to_string(),
                "....".to_string()
            ]
        );
        assert_eq!(
            report.summary,
            "1 of 16 cells changed (6.25% of all pixels differ); the biggest change is in C2 at 100% of that cell."
        );
    }

    #[test]
    fn identical_images_report_no_change() {
        let a = png(&white());
        let report = compare(&a, &a, &Options::default()).unwrap();
        assert_eq!(report.changed_cells, 0);
        assert_eq!(report.changed_pixels, 0);
        assert_eq!(report.changed_percent, 0.0);
        assert_eq!(report.max_delta_percent, 0.0);
        assert!(report.top_cells.is_empty());
        assert!(report.map.unwrap().grid.iter().all(|r| r == "...."));
        assert!(report.summary.starts_with("No cell changed"));
    }

    #[test]
    fn luma_metric_ignores_an_equal_brightness_recolour() {
        // Rec.601 luma of pure blue (0,0,255) is 29.07; of a gray at 29 it is
        // 29.0 — a 0.03% delta, well under any sane threshold. Under `rgb` the
        // same swap is a large change.
        let a = RgbaImage::from_pixel(40, 40, Rgba([0, 0, 255, 255]));
        let b = RgbaImage::from_pixel(40, 40, Rgba([29, 29, 29, 255]));
        let (pa, pb) = (png(&a), png(&b));

        let luma = compare(
            &pa,
            &pb,
            &Options {
                metric: Metric::Luma,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(luma.changed_pixels, 0);
        assert_eq!(luma.changed_cells, 0);

        let rgb = compare(&pa, &pb, &Options::default()).unwrap();
        assert_eq!(rgb.changed_pixels, 1600);
        assert_eq!(rgb.changed_cells, 16);
    }

    #[test]
    fn max_channel_metric_is_stricter_than_rgb() {
        // One channel off by 40/255 (15.7%). Under rgb the RMS over 4 channels
        // is 40/2/255 = 7.8%; under max-channel it is the full 15.7%.
        let a = RgbaImage::from_pixel(40, 40, Rgba([100, 100, 100, 255]));
        let b = RgbaImage::from_pixel(40, 40, Rgba([140, 100, 100, 255]));
        let (pa, pb) = (png(&a), png(&b));
        let opts = |m: Metric| Options {
            metric: m,
            threshold: 10.0,
            ..Options::default()
        };
        assert_eq!(
            compare(&pa, &pb, &opts(Metric::Rgb))
                .unwrap()
                .changed_pixels,
            0
        );
        assert_eq!(
            compare(&pa, &pb, &opts(Metric::MaxChannel))
                .unwrap()
                .changed_pixels,
            1600
        );
    }

    #[test]
    fn min_change_filters_out_a_speck() {
        let mut b = white();
        b.put_pixel(0, 0, Rgba([0, 0, 0, 255])); // 1 of 100 pixels in cell A1
        let (pa, pb) = (png(&white()), png(&b));

        // 1% of the cell changed: at the default min_change of 1 it is flagged.
        let flagged = compare(&pa, &pb, &Options::default()).unwrap();
        assert_eq!(flagged.changed_cells, 1);
        assert!(flagged.cells[0].changed);

        // Raise the floor above it and the speck is reported but not flagged.
        let filtered = compare(
            &pa,
            &pb,
            &Options {
                min_change: 5.0,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(filtered.changed_cells, 0);
        assert_eq!(filtered.cells[0].changed_pixels, 1);
        assert!(!filtered.cells[0].changed);
        assert!(filtered.top_cells.is_empty());
    }

    #[test]
    fn uneven_grid_partitions_every_pixel_exactly_once() {
        let a = RgbaImage::from_pixel(10, 7, Rgba([1, 2, 3, 255]));
        let pa = png(&a);
        let report = compare(
            &pa,
            &pa,
            &Options {
                columns: 3,
                rows: 3,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(report.cells.len(), 9);
        let covered: u64 = report.cells.iter().map(|c| c.pixels).sum();
        assert_eq!(covered, 70);
        assert!(report.cells.iter().all(|c| c.width > 0 && c.height > 0));
    }

    #[test]
    fn map_can_be_switched_off() {
        let a = png(&white());
        let report = compare(
            &a,
            &a,
            &Options {
                map: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(report.map.is_none());
    }

    #[test]
    fn different_sizes_resize_by_default_and_error_on_request() {
        let a = png(&white());
        let b = png(&RgbaImage::from_pixel(80, 80, Rgba([255, 255, 255, 255])));

        let resized = compare(&a, &b, &Options::default()).unwrap();
        assert!(resized.resized);
        assert_eq!((resized.width, resized.height), (40, 40));
        assert_eq!(resized.after.width, 80);
        assert_eq!(resized.changed_cells, 0);

        let err = compare(
            &a,
            &b,
            &Options {
                size_mismatch: SizeMismatch::Error,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("different sizes"), "{err}");
    }

    #[test]
    fn rejects_bad_grid_and_bad_percentages() {
        let a = png(&white());
        let bad = |o: Options| compare(&a, &a, &o).unwrap_err();

        assert!(bad(Options {
            columns: 0,
            ..Options::default()
        })
        .contains("at least 1"));
        assert!(bad(Options {
            rows: 99,
            ..Options::default()
        })
        .contains("at most 32"));
        assert!(bad(Options {
            threshold: 101.0,
            ..Options::default()
        })
        .contains("threshold"));
        assert!(bad(Options {
            min_change: -1.0,
            ..Options::default()
        })
        .contains("min_change"));
        // 40px wide image cannot host 32 columns? it can (40 >= 32) — use a
        // narrow canvas to hit the guard.
        let narrow = png(&RgbaImage::from_pixel(4, 40, Rgba([0, 0, 0, 255])));
        let err = compare(
            &narrow,
            &narrow,
            &Options {
                columns: 8,
                rows: 4,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("cannot exceed the image width"), "{err}");
    }

    #[test]
    fn rejects_undecodable_and_empty_input() {
        let a = png(&white());
        let err = compare(&a, b"not an image at all", &Options::default()).unwrap_err();
        assert!(err.contains("second"), "{err}");
        let err = compare(b"", &a, &Options::default()).unwrap_err();
        assert_eq!(err, "the first image is empty");
    }

    #[test]
    fn parsers_accept_aliases_and_reject_junk() {
        assert_eq!(parse_metric("RGB").unwrap(), Metric::Rgb);
        assert_eq!(parse_metric("brightness").unwrap(), Metric::Luma);
        assert_eq!(parse_metric("max_channel").unwrap(), Metric::MaxChannel);
        assert_eq!(parse_metric("").unwrap(), Metric::Rgb);
        assert!(parse_metric("ssim").unwrap_err().contains("not supported"));

        assert_eq!(parse_size_mismatch("").unwrap(), SizeMismatch::Resize);
        assert_eq!(parse_size_mismatch("Error").unwrap(), SizeMismatch::Error);
        assert!(parse_size_mismatch("crop").unwrap_err().contains("resize"));
    }

    #[test]
    fn column_labels_go_past_z() {
        assert_eq!(column_label(0), "A");
        assert_eq!(column_label(25), "Z");
        assert_eq!(column_label(26), "AA");
        assert_eq!(column_label(31), "AF");
    }
}
