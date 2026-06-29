//! gizza-ai/spritesheet-to-gif core — pure-Rust conversion of a grid sprite
//! sheet into a single animated GIF. No wafer/wasm-bindgen deps.
//!
//! The sheet is sliced into frames exactly like `spritesheet-slice` (grid mode
//! `columns`+`rows`, or tile-size mode `tile_width`+`tile_height`, with optional
//! outer `margin` and inter-frame `spacing`), in row-major order. Each cropped
//! frame becomes one GIF frame; `delay_ms` sets the per-frame delay and
//! `loop_count` controls playback (0 = loop forever). `skip_empty` drops
//! fully-transparent cells; `max_frames` caps how many frames are written.
//!
//! Built on the `image` crate's GIF encoder so it runs on every backend
//! (including the chat Service Worker) — no ffmpeg.

use std::io::Cursor;

use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, GenericImageView, RgbaImage};

/// How the caller described the grid + animation.
#[derive(Debug, Clone)]
pub struct GifParams {
    /// Number of columns (grid mode).
    pub columns: Option<u32>,
    /// Number of rows (grid mode).
    pub rows: Option<u32>,
    /// Frame width in pixels (tile-size mode).
    pub tile_width: Option<u32>,
    /// Frame height in pixels (tile-size mode).
    pub tile_height: Option<u32>,
    /// Border in pixels around all four edges of the sheet.
    pub margin: u32,
    /// Gap in pixels between adjacent frames.
    pub spacing: u32,
    /// Drop frames that are fully transparent (every pixel alpha == 0).
    pub skip_empty: bool,
    /// Per-frame delay in milliseconds.
    pub delay_ms: u16,
    /// Loop count: 0 = infinite, N = play N+1 times then stop.
    pub loop_count: u16,
    /// Stop after writing this many frames (`None` → all cells).
    pub max_frames: Option<usize>,
}

impl Default for GifParams {
    fn default() -> Self {
        Self {
            columns: None,
            rows: None,
            tile_width: None,
            tile_height: None,
            margin: 0,
            spacing: 0,
            skip_empty: false,
            delay_ms: 100,
            loop_count: 0,
            max_frames: None,
        }
    }
}

/// Summary of a conversion run (for the caller's user-facing message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// Frames written into the GIF.
    pub frames: usize,
    /// Frames skipped because they were fully transparent (`skip_empty`).
    pub skipped_empty: usize,
    /// Grid columns used.
    pub columns: u32,
    /// Grid rows used.
    pub rows: u32,
    /// Width of each frame in pixels.
    pub tile_width: u32,
    /// Height of each frame in pixels.
    pub tile_height: u32,
    /// Per-frame delay actually used (ms, after clamping).
    pub delay_ms: u16,
}

/// Resolve the requested grid into concrete `(columns, rows, tile_w, tile_h)`,
/// validating against the sheet's `(width, height)`. Mirrors the slicing math in
/// `spritesheet-slice`.
fn resolve_grid(p: &GifParams, width: u32, height: u32) -> Result<(u32, u32, u32, u32), String> {
    let margin = p.margin;
    let spacing = p.spacing;

    // The usable region after removing the outer margin from both sides.
    let inner_w = width
        .checked_sub(margin.saturating_mul(2))
        .filter(|&w| w > 0)
        .ok_or_else(|| format!("margin {margin} is too large for a {width}px-wide sheet"))?;
    let inner_h = height
        .checked_sub(margin.saturating_mul(2))
        .filter(|&h| h > 0)
        .ok_or_else(|| format!("margin {margin} is too large for a {height}px-tall sheet"))?;

    match (p.columns, p.rows, p.tile_width, p.tile_height) {
        // Grid mode — columns + rows given, derive tile size.
        (Some(cols), Some(rows), _, _) => {
            if cols == 0 || rows == 0 {
                return Err("columns and rows must be at least 1".into());
            }
            let cols_gap = spacing.saturating_mul(cols - 1);
            let rows_gap = spacing.saturating_mul(rows - 1);
            let tile_w = inner_w
                .checked_sub(cols_gap)
                .filter(|&t| t > 0)
                .ok_or_else(|| {
                    format!("{cols} columns with {spacing}px spacing do not fit in the sheet width")
                })?
                / cols;
            let tile_h = inner_h
                .checked_sub(rows_gap)
                .filter(|&t| t > 0)
                .ok_or_else(|| {
                    format!("{rows} rows with {spacing}px spacing do not fit in the sheet height")
                })?
                / rows;
            if tile_w == 0 || tile_h == 0 {
                return Err("computed frame size is 0 — too many columns/rows".into());
            }
            Ok((cols, rows, tile_w, tile_h))
        }
        // Tile-size mode — fixed tile, derive how many fit.
        (_, _, Some(tw), Some(th)) => {
            if tw == 0 || th == 0 {
                return Err("tile_width and tile_height must be at least 1".into());
            }
            let cols = (inner_w + spacing) / (tw + spacing);
            let rows = (inner_h + spacing) / (th + spacing);
            if cols == 0 || rows == 0 {
                return Err(format!(
                    "a {tw}x{th} tile does not fit in the {width}x{height} sheet (after margin/spacing)"
                ));
            }
            Ok((cols, rows, tw, th))
        }
        _ => Err("specify the grid either as columns + rows, or as tile_width + tile_height".into()),
    }
}

/// Slice the sheet `bytes` into frames per `params` (row-major) and encode them
/// into one animated GIF. Returns `(gif_bytes, summary)`. Errors on undecodable
/// input, an impossible grid, or an all-empty result.
pub fn spritesheet_to_gif(bytes: &[u8], params: &GifParams) -> Result<(Vec<u8>, Summary), String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (width, height) = img.dimensions();
    let img = img.to_rgba8();

    let (cols, rows, tile_w, tile_h) = resolve_grid(params, width, height)?;

    // GIF frame delays have 10ms (centisecond) granularity; clamp to a sane band.
    let delay_ms = params.delay_ms.clamp(10, 60_000);

    // Collect cropped frames first so we can detect an empty result before encoding.
    let mut crops: Vec<RgbaImage> = Vec::new();
    let mut skipped_empty = 0usize;

    'outer: for r in 0..rows {
        for c in 0..cols {
            if params.max_frames.is_some_and(|m| crops.len() >= m) {
                break 'outer;
            }
            let x0 = params.margin + c * (tile_w + params.spacing);
            let y0 = params.margin + r * (tile_h + params.spacing);
            // Clamp so a partial final cell never reads past the edge.
            let w = tile_w.min(width.saturating_sub(x0));
            let h = tile_h.min(height.saturating_sub(y0));
            if w == 0 || h == 0 {
                continue;
            }

            let frame = image::imageops::crop_imm(&img, x0, y0, w, h).to_image();

            if params.skip_empty && frame.pixels().all(|px| px.0[3] == 0) {
                skipped_empty += 1;
                continue;
            }

            crops.push(frame);
        }
    }

    if crops.is_empty() {
        return Err("no frames were produced (all cells were empty or out of bounds)".into());
    }

    // All GIF frames must share one canvas; pad each crop onto a tile-sized,
    // top-left-aligned transparent canvas so a clamped final cell still lines up.
    let mut out = Cursor::new(Vec::new());
    {
        let mut enc = GifEncoder::new(&mut out);
        let repeat = if params.loop_count == 0 {
            Repeat::Infinite
        } else {
            Repeat::Finite(params.loop_count)
        };
        enc.set_repeat(repeat).map_err(|e| format!("gif repeat: {e}"))?;
        for crop in crops.iter() {
            let mut canvas: RgbaImage = RgbaImage::new(tile_w, tile_h);
            image::imageops::overlay(&mut canvas, crop, 0, 0);
            let frame =
                Frame::from_parts(canvas, 0, 0, Delay::from_numer_denom_ms(delay_ms as u32, 1));
            enc.encode_frame(frame)
                .map_err(|e| format!("gif encode frame: {e}"))?;
        }
    }

    Ok((
        out.into_inner(),
        Summary {
            frames: crops.len(),
            skipped_empty,
            columns: cols,
            rows,
            tile_width: tile_w,
            tile_height: tile_h,
            delay_ms,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, ImageFormat, Rgba};

    /// Build a `cols`x`rows` grid sheet of `cell`x`cell` cells, each painted a
    /// distinct solid colour so we can confirm crops land on cell boundaries.
    fn checker_sheet(cols: u32, rows: u32, cell: u32) -> Vec<u8> {
        let w = cols * cell;
        let h = rows * cell;
        let mut img = ImageBuffer::<Rgba<u8>, _>::new(w, h);
        for (x, y, px) in img.enumerate_pixels_mut() {
            let c = x / cell;
            let r = y / cell;
            let v = ((r * cols + c) * 17 % 200 + 30) as u8;
            *px = Rgba([v, v, v, 255]);
        }
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    /// Decode the produced GIF back into its frames for assertions.
    fn gif_frames(bytes: &[u8]) -> Vec<image::Frame> {
        use image::AnimationDecoder;
        let dec = image::codecs::gif::GifDecoder::new(Cursor::new(bytes.to_vec())).unwrap();
        dec.into_frames().collect_frames().unwrap()
    }

    #[test]
    fn grid_mode_produces_one_frame_per_cell() {
        let sheet = checker_sheet(4, 2, 16); // 64x32
        let p = GifParams {
            columns: Some(4),
            rows: Some(2),
            ..Default::default()
        };
        let (gif, sum) = spritesheet_to_gif(&sheet, &p).unwrap();
        assert_eq!(sum.frames, 8);
        assert_eq!((sum.tile_width, sum.tile_height), (16, 16));
        let frames = gif_frames(&gif);
        assert_eq!(frames.len(), 8);
        // Each GIF frame is the tile size.
        assert_eq!(frames[0].buffer().dimensions(), (16, 16));
    }

    #[test]
    fn tile_size_mode_computes_grid() {
        let sheet = checker_sheet(3, 3, 20); // 60x60
        let p = GifParams {
            tile_width: Some(20),
            tile_height: Some(20),
            ..Default::default()
        };
        let (gif, sum) = spritesheet_to_gif(&sheet, &p).unwrap();
        assert_eq!((sum.columns, sum.rows), (3, 3));
        assert_eq!(sum.frames, 9);
        assert_eq!(gif_frames(&gif).len(), 9);
    }

    #[test]
    fn delay_is_clamped_and_reported() {
        let sheet = checker_sheet(2, 1, 10);
        let p = GifParams {
            columns: Some(2),
            rows: Some(1),
            delay_ms: 1, // below the 10ms floor
            ..Default::default()
        };
        let (_gif, sum) = spritesheet_to_gif(&sheet, &p).unwrap();
        assert_eq!(sum.delay_ms, 10);
    }

    #[test]
    fn max_frames_caps_the_output() {
        let sheet = checker_sheet(4, 4, 8); // 16 cells
        let p = GifParams {
            columns: Some(4),
            rows: Some(4),
            max_frames: Some(5),
            ..Default::default()
        };
        let (gif, sum) = spritesheet_to_gif(&sheet, &p).unwrap();
        assert_eq!(sum.frames, 5);
        assert_eq!(gif_frames(&gif).len(), 5);
    }

    #[test]
    fn skip_empty_drops_transparent_cells() {
        // 2x1 grid: left cell opaque, right cell fully transparent.
        let mut img = ImageBuffer::<Rgba<u8>, _>::new(20, 10);
        for (x, _y, px) in img.enumerate_pixels_mut() {
            *px = if x < 10 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 0, 0])
            };
        }
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        let p = GifParams {
            columns: Some(2),
            rows: Some(1),
            skip_empty: true,
            ..Default::default()
        };
        let (_gif, sum) = spritesheet_to_gif(&out.into_inner(), &p).unwrap();
        assert_eq!(sum.frames, 1);
        assert_eq!(sum.skipped_empty, 1);
    }

    #[test]
    fn missing_grid_is_an_error() {
        let sheet = checker_sheet(2, 2, 10);
        let err = spritesheet_to_gif(&sheet, &GifParams::default()).unwrap_err();
        assert!(err.contains("specify the grid"), "got: {err}");
    }

    #[test]
    fn undecodable_input_errors() {
        let p = GifParams {
            columns: Some(1),
            rows: Some(1),
            ..Default::default()
        };
        let err = spritesheet_to_gif(b"not an image", &p).unwrap_err();
        assert!(err.contains("could not decode"), "got: {err}");
    }
}
