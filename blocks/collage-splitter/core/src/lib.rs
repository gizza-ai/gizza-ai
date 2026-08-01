//! gizza-ai/collage-splitter core — recover the individual photos from a
//! regular photo-GRID collage (Instagram / grid-maker / MidJourney layout) and
//! bundle each cell as its own image in a ZIP. Pure-Rust (`image` + `zip`), no
//! ML / no ffmpeg, so it runs on every backend including the chat Service
//! Worker.
//!
//! Pipeline:
//!   decode (header-budget guarded) → determine the gutter/border colour
//!   (auto samples the border frame, or force white/black) → per-column and
//!   per-row "background fraction" profiles → a column/row is a GUTTER when it
//!   is mostly the gutter colour → the cell spans are the runs BETWEEN gutters
//!   (per axis) → OR, when the caller passes an explicit `rows`/`columns`, that
//!   axis is split into that many equal spans instead → crop the grid of
//!   (row-span × col-span) cells → optional `trim` inset → encode → ZIP as
//!   cell_1.<ext>, cell_2.<ext>, … in reading order (row-major).
//!
//! This is a GRID splitter, not a free-placement detector: it assumes the
//! photos are laid out on a grid separated by roughly-uniform gutters (or you
//! give it the row/column count). Photos that themselves contain large flat
//! gutter-coloured areas can confuse auto-detection — pass `rows`/`columns`
//! explicitly in that case. (For arbitrarily-placed, possibly-rotated photos on
//! a flatbed scan, use `multi-photo-scan-splitter` instead.)

use std::io::{Cursor, Write};

use image::{ImageFormat, Rgb, RgbImage};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// Hard cap on `rows`/`columns` (and on the number of auto-detected spans per
/// axis) — 20×20 = 400 cells matches the common grid-splitter ceiling.
pub const MAX_LINES: u32 = 20;

/// Reject inputs whose decoded raster + source bytes would exceed this, so a
/// huge collage errors with an actionable message instead of OOM-trapping the
/// 64 MiB wasm sandbox. ~40 MB leaves headroom for the crop/encode buffers.
pub const MAX_WORKING_BYTES: u64 = 40 * 1024 * 1024;

/// Which colour the collage gutters / borders are, so we know what "background"
/// (the separator between photos) looks like. `Auto` samples the outer frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gutter {
    Auto,
    White,
    Black,
}

impl Gutter {
    pub fn parse(s: &str) -> Result<Gutter, String> {
        match s {
            "auto" => Ok(Gutter::Auto),
            "white" => Ok(Gutter::White),
            "black" => Ok(Gutter::Black),
            other => Err(format!(
                "unknown gutter `{other}` (expected auto, white or black)"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Gutter::Auto => "auto",
            Gutter::White => "white",
            Gutter::Black => "black",
        }
    }
}

/// Per-cell output encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutFormat {
    Png,
    Jpeg,
    Webp,
    Bmp,
}

impl OutFormat {
    pub fn parse(s: &str) -> Result<OutFormat, String> {
        match s {
            "png" => Ok(OutFormat::Png),
            "jpeg" | "jpg" => Ok(OutFormat::Jpeg),
            "webp" => Ok(OutFormat::Webp),
            "bmp" => Ok(OutFormat::Bmp),
            other => Err(format!(
                "unknown format `{other}` (expected png, jpeg, webp or bmp)"
            )),
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            OutFormat::Png => "png",
            OutFormat::Jpeg => "jpg",
            OutFormat::Webp => "webp",
            OutFormat::Bmp => "bmp",
        }
    }

    fn image_format(self) -> ImageFormat {
        match self {
            OutFormat::Png => ImageFormat::Png,
            OutFormat::Jpeg => ImageFormat::Jpeg,
            OutFormat::Webp => ImageFormat::WebP,
            OutFormat::Bmp => ImageFormat::Bmp,
        }
    }
}

/// All splitter knobs (see the block descriptor for the LLM/CLI-facing docs).
#[derive(Clone, Debug)]
pub struct SplitParams {
    /// Number of grid ROWS. `0` = auto-detect rows from the gutters.
    pub rows: u32,
    /// Number of grid COLUMNS. `0` = auto-detect columns from the gutters.
    pub columns: u32,
    /// Gutter/border colour used for auto-detection.
    pub gutter: Gutter,
    /// Trim this many pixels inward on every side of each cell, to shave off
    /// leftover gutter/border bleed.
    pub trim: u32,
    pub format: OutFormat,
    /// Filename base: cell -> cell_1.png, cell_2.png, …
    pub prefix: String,
}

impl Default for SplitParams {
    fn default() -> Self {
        SplitParams {
            rows: 0,
            columns: 0,
            gutter: Gutter::Auto,
            trim: 0,
            format: OutFormat::Png,
            prefix: "cell".to_string(),
        }
    }
}

/// What was produced, for the human/LLM summary.
#[derive(Clone, Debug)]
pub struct Summary {
    /// Grid dimensions actually used: `rows` × `columns`.
    pub rows: usize,
    pub columns: usize,
    /// Total cells written (`rows * columns`).
    pub cells: usize,
    /// Gutter colour setting used ("auto" / "white" / "black").
    pub gutter: &'static str,
    /// Whether either axis was auto-detected (vs. an explicit row/col count).
    pub auto_detected: bool,
    /// (width, height) of each written cell, in output (row-major) order.
    pub sizes: Vec<(u32, u32)>,
}

/// Split the grid collage in `image_bytes` into its cells, returning
/// (ZIP bytes, summary).
pub fn split_collage(
    image_bytes: &[u8],
    params: &SplitParams,
) -> Result<(Vec<u8>, Summary), String> {
    let img = decode_within_budget(image_bytes)?;
    let (w, h) = (img.width(), img.height());
    if w < 4 || h < 4 {
        return Err("image is too small to split".into());
    }

    let rows_req = params.rows.min(MAX_LINES);
    let cols_req = params.columns.min(MAX_LINES);

    // Gutter reference colour + a per-pixel "is background" predicate.
    let gutter_ref = gutter_reference(&img, params.gutter);

    // Column spans (x ranges of cells) and row spans (y ranges of cells).
    let col_spans = axis_spans(
        &img,
        Axis::Columns,
        cols_req,
        gutter_ref,
    )?;
    let row_spans = axis_spans(
        &img,
        Axis::Rows,
        rows_req,
        gutter_ref,
    )?;

    let auto_detected = rows_req == 0 || cols_req == 0;
    if auto_detected && col_spans.len() == 1 && row_spans.len() == 1 {
        return Err(format!(
            "no grid gutters were detected on the {} background. Make sure the collage has clear \
             gutters/borders between photos, set gutter to match the border colour \
             (auto/white/black), or pass rows and columns explicitly.",
            params.gutter.label()
        ));
    }

    // Crop the grid of cells (row-major reading order) into a ZIP.
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let prefix = {
        let p = params.prefix.trim();
        if p.is_empty() {
            "cell"
        } else {
            p
        }
    };
    let total = row_spans.len() * col_spans.len();
    let pad = total.to_string().len().max(1);
    let ext = params.format.ext();

    let mut sizes: Vec<(u32, u32)> = Vec::new();
    let mut idx = 0usize;
    for &(y0, y1) in &row_spans {
        for &(x0, x1) in &col_spans {
            idx += 1;
            let cw = x1 - x0;
            let ch = y1 - y0;
            let crop = image::imageops::crop_imm(&img, x0, y0, cw, ch).to_image();
            let crop = apply_trim(crop, params.trim);
            if crop.width() == 0 || crop.height() == 0 {
                continue;
            }
            let encoded = encode(&crop, params.format)?;
            let name = format!("{prefix}_{idx:0pad$}.{ext}");
            zip.start_file(name, opts)
                .map_err(|e| format!("zip error: {e}"))?;
            zip.write_all(&encoded)
                .map_err(|e| format!("zip write error: {e}"))?;
            sizes.push((crop.width(), crop.height()));
        }
    }

    if sizes.is_empty() {
        return Err(
            "the grid was found but every cell was empty after trim — lower the trim value.".into(),
        );
    }

    let zip_bytes = zip
        .finish()
        .map_err(|e| format!("zip finalize error: {e}"))?
        .into_inner();

    Ok((
        zip_bytes,
        Summary {
            rows: row_spans.len(),
            columns: col_spans.len(),
            cells: sizes.len(),
            gutter: params.gutter.label(),
            auto_detected,
            sizes,
        },
    ))
}

// ---------------------------------------------------------------------------
// Decode (header-first memory budget).
// ---------------------------------------------------------------------------

/// Decode `image_bytes`, but read the header first and refuse an image whose
/// decoded raster (plus the source bytes) would blow the wasm sandbox — with an
/// actionable "re-export smaller" message instead of a bare OOM trap.
fn decode_within_budget(image_bytes: &[u8]) -> Result<RgbImage, String> {
    let decoder = image::ImageReader::new(Cursor::new(image_bytes))
        .with_guessed_format()
        .map_err(|e| format!("could not read image: {e}"))?
        .into_decoder()
        .map_err(|e| format!("could not decode image: {e}"))?;
    let total = image::ImageDecoder::total_bytes(&decoder);
    if image_bytes.len() as u64 + total > MAX_WORKING_BYTES {
        return Err(
            "image is too large to split in the browser sandbox — re-export it at a lower \
             resolution (roughly 13 megapixels / ~40 MB decoded max)."
                .into(),
        );
    }
    let dynimg = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("could not decode image: {e}"))?;
    Ok(dynimg.into_rgb8())
}

// ---------------------------------------------------------------------------
// Gutter colour + background predicate.
// ---------------------------------------------------------------------------

/// Determine the reference gutter colour: white/black are fixed extremes; auto
/// samples a thin frame of border pixels (the collage's outer border is the
/// gutter colour) and decides light-vs-dark itself.
fn gutter_reference(img: &RgbImage, gutter: Gutter) -> Rgb<u8> {
    match gutter {
        Gutter::White => Rgb([255, 255, 255]),
        Gutter::Black => Rgb([0, 0, 0]),
        Gutter::Auto => border_mean(img),
    }
}

/// Mean colour of a thin frame of border pixels.
fn border_mean(img: &RgbImage) -> Rgb<u8> {
    let (w, h) = (img.width(), img.height());
    let band = (w.min(h) / 40).max(1);
    let mut sum = [0u64; 3];
    let mut n = 0u64;
    for y in 0..h {
        let edge_row = y < band || y >= h - band;
        for x in 0..w {
            if edge_row || x < band || x >= w - band {
                let p = img.get_pixel(x, y);
                sum[0] += u64::from(p.0[0]);
                sum[1] += u64::from(p.0[1]);
                sum[2] += u64::from(p.0[2]);
                n += 1;
            }
        }
    }
    let n = n.max(1);
    Rgb([(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8])
}

/// A pixel counts as "gutter/background" when every channel is within
/// `CHANNEL_TOL` of the reference gutter colour.
fn is_gutter(p: &Rgb<u8>, gref: Rgb<u8>) -> bool {
    const CHANNEL_TOL: i32 = 40;
    (0..3).all(|c| (i32::from(p.0[c]) - i32::from(gref.0[c])).abs() <= CHANNEL_TOL)
}

// ---------------------------------------------------------------------------
// Axis spans: the x-ranges (columns) or y-ranges (rows) of the cells.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    Columns,
    Rows,
}

/// Compute the cell spans along one axis. When `requested > 0`, the axis is
/// divided into that many equal spans (a classic manual grid split). When
/// `requested == 0`, the gutters are auto-detected and the spans are the runs
/// of non-gutter lines between them.
fn axis_spans(
    img: &RgbImage,
    axis: Axis,
    requested: u32,
    gref: Rgb<u8>,
) -> Result<Vec<(u32, u32)>, String> {
    let (w, h) = (img.width(), img.height());
    let extent = match axis {
        Axis::Columns => w,
        Axis::Rows => h,
    };

    if requested > 0 {
        return Ok(equal_spans(extent, requested));
    }

    // Auto: "background fraction" of each line (column or row).
    let (lines, cross) = match axis {
        Axis::Columns => (w, h),
        Axis::Rows => (h, w),
    };
    // Fraction of the perpendicular extent that is gutter-coloured.
    let mut bg_frac = vec![0f32; lines as usize];
    for line in 0..lines {
        let mut hits = 0u32;
        for c in 0..cross {
            let (x, y) = match axis {
                Axis::Columns => (line, c),
                Axis::Rows => (c, line),
            };
            if is_gutter(img.get_pixel(x, y), gref) {
                hits += 1;
            }
        }
        bg_frac[line as usize] = hits as f32 / cross as f32;
    }

    // A line is a gutter when it is overwhelmingly the gutter colour.
    const GUTTER_FRAC: f32 = 0.80;
    // A cell span must be at least this many lines wide to count (drops noise).
    let min_cell = (extent / 100).max(3);

    let mut spans: Vec<(u32, u32)> = Vec::new();
    let mut start: Option<u32> = None;
    for line in 0..lines {
        let is_g = bg_frac[line as usize] >= GUTTER_FRAC;
        if is_g {
            if let Some(s) = start.take() {
                if line - s >= min_cell {
                    spans.push((s, line));
                }
            }
        } else if start.is_none() {
            start = Some(line);
        }
    }
    if let Some(s) = start.take() {
        if lines - s >= min_cell {
            spans.push((s, lines));
        }
    }

    if spans.is_empty() {
        // The whole axis read as gutter (e.g. a blank image) — fall back to the
        // full extent so the caller's "no grid detected" branch can fire.
        return Ok(vec![(0, extent)]);
    }
    if spans.len() as u32 > MAX_LINES {
        return Err(format!(
            "detected {} cells along one axis (max {MAX_LINES}) — the image may not be a clean \
             grid; pass rows and columns explicitly.",
            spans.len()
        ));
    }
    Ok(spans)
}

/// Divide `extent` into `n` equal, exactly-tiling spans (last span absorbs the
/// integer-division remainder).
fn equal_spans(extent: u32, n: u32) -> Vec<(u32, u32)> {
    let n = n.max(1).min(extent.max(1));
    (0..n)
        .map(|i| {
            let a = (u64::from(extent) * u64::from(i) / u64::from(n)) as u32;
            let b = (u64::from(extent) * u64::from(i + 1) / u64::from(n)) as u32;
            (a, b.max(a + 1).min(extent))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Crop trim + encoding.
// ---------------------------------------------------------------------------

fn apply_trim(img: RgbImage, trim: u32) -> RgbImage {
    if trim == 0 {
        return img;
    }
    let (w, h) = (img.width(), img.height());
    if 2 * trim >= w || 2 * trim >= h {
        // Trim would erase the cell — return an empty image the caller skips.
        return RgbImage::new(0, 0);
    }
    image::imageops::crop_imm(&img, trim, trim, w - 2 * trim, h - 2 * trim).to_image()
}

fn encode(img: &RgbImage, fmt: OutFormat) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img.clone())
        .write_to(&mut out, fmt.image_format())
        .map_err(|e| format!("failed to encode {}: {e}", fmt.ext()))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_of(img: &RgbImage) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img.clone())
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    /// Build a `cols`×`rows` grid collage: a `bg`-coloured canvas with
    /// `cell`×`cell` photos separated by `gutter`-wide gutters and a `gutter`
    /// outer border. Each cell is painted a distinct solid colour.
    fn grid_collage(
        rows: u32,
        cols: u32,
        cell: u32,
        gutter: u32,
        bg: [u8; 3],
        colors: &[[u8; 3]],
    ) -> RgbImage {
        let w = gutter + cols * cell + (cols - 1) * gutter + gutter;
        let h = gutter + rows * cell + (rows - 1) * gutter + gutter;
        let mut img = RgbImage::from_pixel(w, h, Rgb(bg));
        let mut k = 0usize;
        for r in 0..rows {
            for c in 0..cols {
                let x0 = gutter + c * (cell + gutter);
                let y0 = gutter + r * (cell + gutter);
                let color = Rgb(colors[k % colors.len()]);
                for yy in y0..y0 + cell {
                    for xx in x0..x0 + cell {
                        img.put_pixel(xx, yy, color);
                    }
                }
                k += 1;
            }
        }
        img
    }

    fn unzip_names(zip: &[u8]) -> Vec<String> {
        let mut ar = zip::ZipArchive::new(Cursor::new(zip.to_vec())).unwrap();
        (0..ar.len())
            .map(|i| ar.by_index(i).unwrap().name().to_string())
            .collect()
    }

    fn unzip_image(zip: &[u8], i: usize) -> RgbImage {
        use std::io::Read;
        let mut ar = zip::ZipArchive::new(Cursor::new(zip.to_vec())).unwrap();
        let mut f = ar.by_index(i).unwrap();
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).unwrap();
        image::load_from_memory(&bytes).unwrap().to_rgb8()
    }

    const CELL_COLORS: &[[u8; 3]] = &[
        [220, 40, 40],
        [40, 200, 60],
        [40, 60, 220],
        [200, 160, 30],
        [160, 40, 200],
        [30, 190, 190],
    ];

    #[test]
    fn auto_detects_a_3x2_grid_on_white() {
        // 2 rows × 3 columns, white gutters.
        let img = grid_collage(2, 3, 100, 20, [255, 255, 255], CELL_COLORS);
        let (zip, s) = split_collage(&png_of(&img), &SplitParams::default()).unwrap();
        assert_eq!(s.rows, 2, "rows: {s:?}");
        assert_eq!(s.columns, 3, "columns: {s:?}");
        assert_eq!(s.cells, 6);
        assert!(s.auto_detected);
        let names = unzip_names(&zip);
        assert_eq!(
            names,
            vec![
                "cell_1.png",
                "cell_2.png",
                "cell_3.png",
                "cell_4.png",
                "cell_5.png",
                "cell_6.png"
            ]
        );
        // Each cell is ~100x100 and its centre is the painted colour, in
        // row-major reading order.
        for (i, expected) in CELL_COLORS.iter().enumerate() {
            let out = unzip_image(&zip, i);
            assert!(
                (out.width() as i64 - 100).abs() <= 6 && (out.height() as i64 - 100).abs() <= 6,
                "cell {i} dims off: {}x{}",
                out.width(),
                out.height()
            );
            let p = out.get_pixel(out.width() / 2, out.height() / 2).0;
            let close = (0..3).all(|c| (i32::from(p[c]) - i32::from(expected[c])).abs() <= 30);
            assert!(close, "cell {i} centre {p:?} != {expected:?}");
        }
    }

    #[test]
    fn detects_grid_on_black_gutters() {
        let img = grid_collage(2, 2, 80, 16, [0, 0, 0], CELL_COLORS);
        let params = SplitParams {
            gutter: Gutter::Black,
            ..Default::default()
        };
        let (_zip, s) = split_collage(&png_of(&img), &params).unwrap();
        assert_eq!((s.rows, s.columns, s.cells), (2, 2, 4));
        assert_eq!(s.gutter, "black");
    }

    #[test]
    fn manual_rows_and_columns_force_an_even_split() {
        // A plain single photo (no gutters) split 2×2 by explicit request.
        let img = RgbImage::from_pixel(200, 160, Rgb([120, 90, 30]));
        let params = SplitParams {
            rows: 2,
            columns: 2,
            ..Default::default()
        };
        let (zip, s) = split_collage(&png_of(&img), &params).unwrap();
        assert_eq!((s.rows, s.columns, s.cells), (2, 2, 4));
        assert!(!s.auto_detected);
        // 200/2 = 100 wide, 160/2 = 80 tall per cell.
        let out = unzip_image(&zip, 0);
        assert_eq!((out.width(), out.height()), (100, 80));
    }

    #[test]
    fn manual_columns_only_auto_detects_rows() {
        // 3 rows of photos, white gutters; force 1 column, auto rows.
        let img = grid_collage(3, 1, 100, 20, [255, 255, 255], CELL_COLORS);
        let params = SplitParams {
            columns: 1,
            ..Default::default()
        };
        let (_zip, s) = split_collage(&png_of(&img), &params).unwrap();
        assert_eq!((s.rows, s.columns), (3, 1));
        assert!(s.auto_detected, "rows were auto-detected");
    }

    #[test]
    fn trim_shrinks_each_cell() {
        let img = grid_collage(1, 2, 100, 20, [255, 255, 255], CELL_COLORS);
        let base = split_collage(&png_of(&img), &SplitParams::default()).unwrap();
        let base_dims = base.1.sizes[0];
        let trimmed = split_collage(
            &png_of(&img),
            &SplitParams {
                trim: 5,
                ..Default::default()
            },
        )
        .unwrap();
        let td = trimmed.1.sizes[0];
        assert_eq!(td.0, base_dims.0 - 10);
        assert_eq!(td.1, base_dims.1 - 10);
    }

    #[test]
    fn jpeg_format_names_files_jpg() {
        let img = grid_collage(1, 2, 80, 16, [255, 255, 255], CELL_COLORS);
        let params = SplitParams {
            format: OutFormat::Jpeg,
            ..Default::default()
        };
        let (zip, _s) = split_collage(&png_of(&img), &params).unwrap();
        assert_eq!(unzip_names(&zip), vec!["cell_1.jpg", "cell_2.jpg"]);
        assert!(unzip_image(&zip, 0).width() > 0);
    }

    #[test]
    fn custom_prefix_and_zero_padding() {
        // 12 columns → two-digit zero padding, custom prefix.
        let img = grid_collage(1, 12, 20, 8, [255, 255, 255], CELL_COLORS);
        let params = SplitParams {
            prefix: "photo".into(),
            ..Default::default()
        };
        let (zip, s) = split_collage(&png_of(&img), &params).unwrap();
        assert_eq!(s.columns, 12);
        let names = unzip_names(&zip);
        assert_eq!(names[0], "photo_01.png");
        assert_eq!(names[11], "photo_12.png");
    }

    #[test]
    fn errors_when_no_grid_and_no_manual_counts() {
        // A blank canvas: no gutters between anything → not a grid.
        let img = RgbImage::from_pixel(200, 150, Rgb([255, 255, 255]));
        let err = split_collage(&png_of(&img), &SplitParams::default()).unwrap_err();
        assert!(err.contains("no grid gutters"), "unexpected error: {err}");
    }

    #[test]
    fn errors_on_undecodable_input() {
        assert!(split_collage(b"not an image", &SplitParams::default()).is_err());
    }

    #[test]
    fn parsers_reject_bad_values() {
        assert_eq!(Gutter::parse("white").unwrap(), Gutter::White);
        assert!(Gutter::parse("grey").is_err());
        assert_eq!(OutFormat::parse("jpg").unwrap(), OutFormat::Jpeg);
        assert!(OutFormat::parse("tiff").is_err());
    }

    #[test]
    fn equal_spans_tile_exactly() {
        let spans = equal_spans(200, 3);
        assert_eq!(spans.first().unwrap().0, 0);
        assert_eq!(spans.last().unwrap().1, 200);
        // Contiguous, non-overlapping.
        for w in spans.windows(2) {
            assert_eq!(w[0].1, w[1].0);
        }
    }
}
