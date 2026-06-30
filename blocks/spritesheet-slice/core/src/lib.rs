//! gizza-ai/spritesheet-slice core — pure-Rust slicing of a grid sprite sheet
//! into individual frame PNGs, bundled into a ZIP. No wafer/wasm-bindgen deps.
//!
//! Two ways to describe the grid:
//!   - **Grid mode** — give `columns` + `rows`; each frame's size is computed
//!     from the sheet dimensions (minus margin/spacing).
//!   - **Tile-size mode** — give `tile_width` + `tile_height`; the number of
//!     columns/rows is computed from how many fixed-size tiles fit.
//! `margin` is a border around all four edges; `spacing` is the gap between
//! adjacent frames (both default 0 → a plain even grid). Frames are emitted in
//! row-major order as `<prefix>_000.<ext>`, `<prefix>_001.<ext>`, … (`prefix`
//! defaults to `frame`). Optionally drops fully-transparent frames (common for
//! padded sheets) when `skip_empty` is set, caps the count with `max_frames`,
//! and encodes each frame as PNG (default), JPEG, WebP, or BMP.

use std::io::{Cursor, Write};

use image::{GenericImageView, ImageFormat, RgbaImage};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// Per-frame output encoding. PNG/WebP/BMP keep the alpha channel; JPEG is
/// opaque (alpha is dropped onto an RGB frame).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutFormat {
    /// Lossless PNG, alpha preserved (default).
    #[default]
    Png,
    /// Lossy JPEG, no alpha.
    Jpeg,
    /// Lossless WebP, alpha preserved.
    Webp,
    /// Uncompressed BMP, alpha preserved.
    Bmp,
}

impl OutFormat {
    /// Parse a case-insensitive format name; `None`/blank → PNG.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "png" => Ok(Self::Png),
            "jpeg" | "jpg" => Ok(Self::Jpeg),
            "webp" => Ok(Self::Webp),
            "bmp" => Ok(Self::Bmp),
            other => Err(format!("unknown format '{other}' (use png, jpeg, webp, or bmp)")),
        }
    }

    /// Filename extension for this format.
    pub fn ext(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Bmp => "bmp",
        }
    }

    fn image_format(self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
            Self::Webp => ImageFormat::WebP,
            Self::Bmp => ImageFormat::Bmp,
        }
    }
}

/// Encode one cropped frame to `fmt`. JPEG can't carry alpha, so the frame is
/// flattened to RGB first; the others keep RGBA.
fn encode_frame(frame: RgbaImage, fmt: OutFormat) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    let img = image::DynamicImage::ImageRgba8(frame);
    let result = match fmt {
        OutFormat::Jpeg => img.to_rgb8().write_to(&mut out, ImageFormat::Jpeg),
        _ => img.write_to(&mut out, fmt.image_format()),
    };
    result.map_err(|e| format!("could not encode frame: {e}"))?;
    Ok(out.into_inner())
}

/// How the caller described the grid.
#[derive(Debug, Clone)]
pub struct SliceParams {
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
    /// Per-frame output encoding (default PNG).
    pub format: OutFormat,
    /// Filename base for each frame, e.g. `frame` → `frame_000.png`.
    pub prefix: String,
    /// Stop after writing this many frames (`None` → all cells).
    pub max_frames: Option<usize>,
}

impl Default for SliceParams {
    fn default() -> Self {
        Self {
            columns: None,
            rows: None,
            tile_width: None,
            tile_height: None,
            margin: 0,
            spacing: 0,
            skip_empty: false,
            format: OutFormat::Png,
            prefix: "frame".to_string(),
            max_frames: None,
        }
    }
}

/// Summary of a slice run (for the caller's user-facing message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// Frames written into the ZIP.
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
}

/// Resolve the requested grid into concrete `(columns, rows, tile_w, tile_h)`,
/// validating against the sheet's `(width, height)`.
fn resolve_grid(
    p: &SliceParams,
    width: u32,
    height: u32,
) -> Result<(u32, u32, u32, u32), String> {
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
            // inner = cols*tile + (cols-1)*spacing  ⇒  tile = (inner - (cols-1)*spacing) / cols
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
            // n tiles fit when n*tile + (n-1)*spacing <= inner
            //   ⇒ n <= (inner + spacing) / (tile + spacing)
            let cols = (inner_w + spacing) / (tw + spacing);
            let rows = (inner_h + spacing) / (th + spacing);
            if cols == 0 || rows == 0 {
                return Err(format!(
                    "a {tw}x{th} tile does not fit in the {width}x{height} sheet (after margin/spacing)"
                ));
            }
            Ok((cols, rows, tw, th))
        }
        _ => Err(
            "specify the grid either as columns + rows, or as tile_width + tile_height".into(),
        ),
    }
}

/// Slice `bytes` (an encoded image) into individual frame PNGs per `params` and
/// return `(zip_bytes, summary)`. Frames are cropped row-major and re-encoded as
/// PNG (preserving alpha). Errors on undecodable input or an impossible grid.
pub fn slice_spritesheet(bytes: &[u8], params: &SliceParams) -> Result<(Vec<u8>, Summary), String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (width, height) = img.dimensions();
    let img = img.to_rgba8();

    let (cols, rows, tile_w, tile_h) = resolve_grid(params, width, height)?;

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let prefix = if params.prefix.trim().is_empty() {
        "frame"
    } else {
        params.prefix.trim()
    };

    let mut frames = 0usize;
    let mut skipped_empty = 0usize;
    // Use a stable, zero-padded index so files sort in order.
    let total = (cols as usize) * (rows as usize);
    let pad = total.to_string().len().max(3);
    let ext = params.format.ext();

    let mut index = 0usize;
    'outer: for r in 0..rows {
        for c in 0..cols {
            if params.max_frames.is_some_and(|m| frames >= m) {
                break 'outer;
            }
            let x0 = params.margin + c * (tile_w + params.spacing);
            let y0 = params.margin + r * (tile_h + params.spacing);
            // Clamp so a partial final cell never reads past the edge.
            let w = tile_w.min(width.saturating_sub(x0));
            let h = tile_h.min(height.saturating_sub(y0));
            if w == 0 || h == 0 {
                index += 1;
                continue;
            }

            let frame = image::imageops::crop_imm(&img, x0, y0, w, h).to_image();

            if params.skip_empty && frame.pixels().all(|px| px.0[3] == 0) {
                skipped_empty += 1;
                index += 1;
                continue;
            }

            let encoded = encode_frame(frame, params.format)?;

            let name = format!("{prefix}_{index:0pad$}.{ext}");
            zip.start_file(name, opts)
                .map_err(|e| format!("zip error: {e}"))?;
            zip.write_all(&encoded)
                .map_err(|e| format!("zip write error: {e}"))?;
            frames += 1;
            index += 1;
        }
    }

    if frames == 0 {
        return Err("no frames were produced (all cells were empty or out of bounds)".into());
    }

    let zip_bytes = zip
        .finish()
        .map_err(|e| format!("zip finalize error: {e}"))?
        .into_inner();

    Ok((
        zip_bytes,
        Summary {
            frames,
            skipped_empty,
            columns: cols,
            rows,
            tile_width: tile_w,
            tile_height: tile_h,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    /// Build a `w`x`h` test sheet: each cell of `cell`x`cell` painted a distinct
    /// solid colour so we can confirm crops land on cell boundaries.
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

    fn zip_names(bytes: &[u8]) -> Vec<String> {
        let mut a = zip::ZipArchive::new(Cursor::new(bytes.to_vec())).unwrap();
        (0..a.len()).map(|i| a.by_index(i).unwrap().name().to_string()).collect()
    }

    #[test]
    fn grid_mode_slices_all_cells() {
        let sheet = checker_sheet(4, 2, 16); // 64x32
        let p = SliceParams { columns: Some(4), rows: Some(2), ..Default::default() };
        let (zip, sum) = slice_spritesheet(&sheet, &p).unwrap();
        assert_eq!(sum.frames, 8);
        assert_eq!((sum.tile_width, sum.tile_height), (16, 16));
        let names = zip_names(&zip);
        assert_eq!(names.len(), 8);
        assert_eq!(names[0], "frame_000.png");
        assert_eq!(names[7], "frame_007.png");
    }

    #[test]
    fn tile_size_mode_computes_grid() {
        let sheet = checker_sheet(3, 3, 20); // 60x60
        let p = SliceParams {
            tile_width: Some(20),
            tile_height: Some(20),
            ..Default::default()
        };
        let (_zip, sum) = slice_spritesheet(&sheet, &p).unwrap();
        assert_eq!((sum.columns, sum.rows), (3, 3));
        assert_eq!(sum.frames, 9);
    }

    #[test]
    fn each_frame_is_a_valid_png_of_tile_size() {
        let sheet = checker_sheet(2, 2, 24);
        let p = SliceParams { columns: Some(2), rows: Some(2), ..Default::default() };
        let (zip, _sum) = slice_spritesheet(&sheet, &p).unwrap();
        let mut a = zip::ZipArchive::new(Cursor::new(zip)).unwrap();
        let mut buf = Vec::new();
        std::io::copy(&mut a.by_index(0).unwrap(), &mut buf).unwrap();
        let frame = image::load_from_memory(&buf).unwrap();
        assert_eq!(frame.dimensions(), (24, 24));
    }

    #[test]
    fn margin_and_spacing_in_tile_mode() {
        // 2px margin + a 4-wide gap; two 10px tiles across: 2 + 10 + 4 + 10 + 2 = 38.
        // Height fits one 10px row inside a 2px margin: 2 + 10 + 2 = 14.
        let mut img = ImageBuffer::<Rgba<u8>, _>::new(38, 14);
        for px in img.pixels_mut() {
            *px = Rgba([10, 20, 30, 255]);
        }
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        let p = SliceParams {
            tile_width: Some(10),
            tile_height: Some(10),
            margin: 2,
            spacing: 4,
            ..Default::default()
        };
        let (_zip, sum) = slice_spritesheet(&out.into_inner(), &p).unwrap();
        assert_eq!(sum.columns, 2);
        assert_eq!(sum.tile_width, 10);
    }

    #[test]
    fn skip_empty_drops_transparent_frames() {
        // 2x1 grid: left cell opaque, right cell fully transparent.
        let mut img = ImageBuffer::<Rgba<u8>, _>::new(20, 10);
        for (x, _y, px) in img.enumerate_pixels_mut() {
            *px = if x < 10 { Rgba([255, 0, 0, 255]) } else { Rgba([0, 0, 0, 0]) };
        }
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        let p = SliceParams {
            columns: Some(2),
            rows: Some(1),
            skip_empty: true,
            ..Default::default()
        };
        let (zip, sum) = slice_spritesheet(&out.into_inner(), &p).unwrap();
        assert_eq!(sum.frames, 1);
        assert_eq!(sum.skipped_empty, 1);
        assert_eq!(zip_names(&zip).len(), 1);
    }

    #[test]
    fn errors_when_no_grid_specified() {
        let sheet = checker_sheet(2, 2, 8);
        let err = slice_spritesheet(&sheet, &SliceParams::default()).unwrap_err();
        assert!(err.contains("columns + rows"), "got: {err}");
    }

    #[test]
    fn errors_on_undecodable_input() {
        let p = SliceParams { columns: Some(1), rows: Some(1), ..Default::default() };
        let err = slice_spritesheet(b"not an image", &p).unwrap_err();
        assert!(err.contains("decode"), "got: {err}");
    }

    #[test]
    fn jpeg_format_changes_extension_and_decodes() {
        let sheet = checker_sheet(2, 1, 16);
        let p = SliceParams {
            columns: Some(2),
            rows: Some(1),
            format: OutFormat::Jpeg,
            ..Default::default()
        };
        let (zip, _sum) = slice_spritesheet(&sheet, &p).unwrap();
        let names = zip_names(&zip);
        assert_eq!(names[0], "frame_000.jpg");
        // The bytes must be a valid JPEG.
        let mut a = zip::ZipArchive::new(Cursor::new(zip)).unwrap();
        let mut buf = Vec::new();
        std::io::copy(&mut a.by_index(0).unwrap(), &mut buf).unwrap();
        assert_eq!(
            image::guess_format(&buf).unwrap(),
            ImageFormat::Jpeg
        );
    }

    #[test]
    fn webp_and_bmp_formats_encode() {
        let sheet = checker_sheet(2, 1, 12);
        for (fmt, want, expect_ext) in [
            (OutFormat::Webp, ImageFormat::WebP, "webp"),
            (OutFormat::Bmp, ImageFormat::Bmp, "bmp"),
        ] {
            let p = SliceParams {
                columns: Some(2),
                rows: Some(1),
                format: fmt,
                ..Default::default()
            };
            let (zip, _sum) = slice_spritesheet(&sheet, &p).unwrap();
            let names = zip_names(&zip);
            assert!(names[0].ends_with(expect_ext), "got {}", names[0]);
            let mut a = zip::ZipArchive::new(Cursor::new(zip)).unwrap();
            let mut buf = Vec::new();
            std::io::copy(&mut a.by_index(0).unwrap(), &mut buf).unwrap();
            assert_eq!(image::guess_format(&buf).unwrap(), want);
        }
    }

    #[test]
    fn custom_prefix_renames_frames() {
        let sheet = checker_sheet(2, 1, 8);
        let p = SliceParams {
            columns: Some(2),
            rows: Some(1),
            prefix: "hero".to_string(),
            ..Default::default()
        };
        let (zip, _sum) = slice_spritesheet(&sheet, &p).unwrap();
        assert_eq!(zip_names(&zip)[0], "hero_000.png");
    }

    #[test]
    fn blank_prefix_falls_back_to_frame() {
        let sheet = checker_sheet(2, 1, 8);
        let p = SliceParams {
            columns: Some(2),
            rows: Some(1),
            prefix: "   ".to_string(),
            ..Default::default()
        };
        let (zip, _sum) = slice_spritesheet(&sheet, &p).unwrap();
        assert_eq!(zip_names(&zip)[0], "frame_000.png");
    }

    #[test]
    fn max_frames_caps_output() {
        let sheet = checker_sheet(4, 4, 8); // 16 cells
        let p = SliceParams {
            columns: Some(4),
            rows: Some(4),
            max_frames: Some(5),
            ..Default::default()
        };
        let (zip, sum) = slice_spritesheet(&sheet, &p).unwrap();
        assert_eq!(sum.frames, 5);
        assert_eq!(zip_names(&zip).len(), 5);
    }

    #[test]
    fn format_parse_accepts_aliases_and_rejects_unknown() {
        assert_eq!(OutFormat::parse("JPG").unwrap(), OutFormat::Jpeg);
        assert_eq!(OutFormat::parse("").unwrap(), OutFormat::Png);
        assert!(OutFormat::parse("tiff").is_err());
    }

    #[test]
    fn errors_when_margin_too_large() {
        let sheet = checker_sheet(2, 2, 8); // 16x16
        let p = SliceParams { columns: Some(2), rows: Some(2), margin: 20, ..Default::default() };
        let err = slice_spritesheet(&sheet, &p).unwrap_err();
        assert!(err.contains("margin"), "got: {err}");
    }
}
