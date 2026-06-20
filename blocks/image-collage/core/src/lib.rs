//! gizza-ai/image-collage core — combine several images into one collage (grid,
//! horizontal strip, or vertical stack). No wafer/wasm-bindgen deps. Pure-Rust
//! `image` crate: each input is scaled to fit a uniform cell (aspect preserved,
//! centered on the background), then composited onto a canvas.

use std::io::Cursor;

use image::{imageops, DynamicImage, ImageFormat, Rgba, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Grid,
    Horizontal,
    Vertical,
}

pub fn parse_layout(s: &str) -> Result<Layout, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "grid" => Ok(Layout::Grid),
        "horizontal" | "row" | "strip" => Ok(Layout::Horizontal),
        "vertical" | "column" | "stack" => Ok(Layout::Vertical),
        other => Err(format!("layout {other:?} not supported (grid|horizontal|vertical)")),
    }
}

/// Parse `#rgb`, `#rrggbb`, or `#rrggbbaa` (with or without `#`). Empty → white.
pub fn parse_color(s: &str) -> Result<[u8; 4], String> {
    let h = s.trim().trim_start_matches('#');
    let p = |a: &str| u8::from_str_radix(a, 16).map_err(|_| format!("invalid color '{s}'"));
    match h.len() {
        0 => Ok([255, 255, 255, 255]),
        3 => {
            let r = p(&h[0..1])?;
            let g = p(&h[1..2])?;
            let b = p(&h[2..3])?;
            Ok([r * 17, g * 17, b * 17, 255])
        }
        6 => Ok([p(&h[0..2])?, p(&h[2..4])?, p(&h[4..6])?, 255]),
        8 => Ok([p(&h[0..2])?, p(&h[2..4])?, p(&h[4..6])?, p(&h[6..8])?]),
        _ => Err(format!("invalid color '{s}' (use #rgb, #rrggbb, or #rrggbbaa)")),
    }
}

const MAX_CELL: u32 = 1200;
const MAX_PIXELS: u64 = 40_000_000; // ~40 MP canvas guard

/// Compute (cols, rows) for a layout and image count.
fn grid_shape(layout: Layout, n: usize, columns: Option<usize>) -> (usize, usize) {
    match layout {
        Layout::Horizontal => (n, 1),
        Layout::Vertical => (1, n),
        Layout::Grid => {
            let cols = columns
                .filter(|&c| c >= 1)
                .unwrap_or_else(|| (n as f64).sqrt().ceil() as usize)
                .max(1);
            let rows = n.div_ceil(cols);
            (cols, rows)
        }
    }
}

/// Build the collage from already-decoded images. Returns PNG bytes.
pub fn collage(
    images: Vec<DynamicImage>,
    layout: Layout,
    columns: Option<usize>,
    gap: u32,
    bg: [u8; 4],
) -> Result<Vec<u8>, String> {
    if images.is_empty() {
        return Err("need at least one image".into());
    }
    let cell_w = images.iter().map(|i| i.width()).max().unwrap().min(MAX_CELL).max(1);
    let cell_h = images.iter().map(|i| i.height()).max().unwrap().min(MAX_CELL).max(1);
    let n = images.len();
    let (cols, rows) = grid_shape(layout, n, columns);

    let gap = gap.min(200);
    let w = cols as u32 * cell_w + (cols as u32 + 1) * gap;
    let h = rows as u32 * cell_h + (rows as u32 + 1) * gap;
    if w as u64 * h as u64 > MAX_PIXELS {
        return Err(format!(
            "collage would be {w}x{h} (> {MAX_PIXELS} px); use fewer/smaller images or a smaller gap"
        ));
    }

    let mut canvas = RgbaImage::from_pixel(w, h, Rgba(bg));
    for (idx, img) in images.into_iter().enumerate() {
        let col = idx % cols;
        let row = idx / cols;
        // Scale to fit the cell, preserving aspect ratio.
        let thumb = img.resize(cell_w, cell_h, imageops::FilterType::Lanczos3).to_rgba8();
        let cell_x = gap + col as u32 * (cell_w + gap);
        let cell_y = gap + row as u32 * (cell_h + gap);
        // Center within the cell.
        let ox = cell_x + (cell_w.saturating_sub(thumb.width())) / 2;
        let oy = cell_y + (cell_h.saturating_sub(thumb.height())) / 2;
        imageops::overlay(&mut canvas, &thumb, ox as i64, oy as i64);
    }

    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(canvas)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

/// Decode raw image bytes then build the collage.
pub fn collage_from_bytes(
    raw: &[Vec<u8>],
    layout: Layout,
    columns: Option<usize>,
    gap: u32,
    bg: [u8; 4],
) -> Result<Vec<u8>, String> {
    let mut imgs = Vec::with_capacity(raw.len());
    for (i, bytes) in raw.iter().enumerate() {
        let img = image::load_from_memory(bytes)
            .map_err(|e| format!("image {} could not be decoded: {e}", i + 1))?;
        imgs.push(img);
    }
    collage(imgs, layout, columns, gap, bg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, c: [u8; 4]) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, Rgba(c)))
    }

    fn png_bytes(img: &DynamicImage) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn parse_layout_values() {
        assert_eq!(parse_layout("grid").unwrap(), Layout::Grid);
        assert_eq!(parse_layout("").unwrap(), Layout::Grid);
        assert_eq!(parse_layout("horizontal").unwrap(), Layout::Horizontal);
        assert_eq!(parse_layout("stack").unwrap(), Layout::Vertical);
        assert!(parse_layout("diagonal").is_err());
    }

    #[test]
    fn parse_color_forms() {
        assert_eq!(parse_color("").unwrap(), [255, 255, 255, 255]);
        assert_eq!(parse_color("#000000").unwrap(), [0, 0, 0, 255]);
        assert_eq!(parse_color("ff8800").unwrap(), [255, 136, 0, 255]);
        assert_eq!(parse_color("#fff").unwrap(), [255, 255, 255, 255]);
        assert_eq!(parse_color("#00000080").unwrap(), [0, 0, 0, 128]);
        assert!(parse_color("#xyz123").is_err());
    }

    #[test]
    fn grid_shape_math() {
        assert_eq!(grid_shape(Layout::Horizontal, 4, None), (4, 1));
        assert_eq!(grid_shape(Layout::Vertical, 4, None), (1, 4));
        assert_eq!(grid_shape(Layout::Grid, 4, None), (2, 2)); // ceil(sqrt(4))=2
        assert_eq!(grid_shape(Layout::Grid, 5, None), (3, 2)); // ceil(sqrt(5))=3, rows=2
        assert_eq!(grid_shape(Layout::Grid, 5, Some(2)), (2, 3));
    }

    #[test]
    fn horizontal_collage_dimensions() {
        let imgs = vec![solid(40, 30, [255, 0, 0, 255]), solid(40, 30, [0, 255, 0, 255])];
        let png = collage(imgs, Layout::Horizontal, None, 10, [0, 0, 0, 255]).unwrap();
        let decoded = image::load_from_memory(&png).unwrap();
        // 2 cells of 40 wide + 3 gaps of 10 = 110; 1 row of 30 + 2 gaps = 50.
        assert_eq!((decoded.width(), decoded.height()), (110, 50));
    }

    #[test]
    fn grid_collage_dimensions() {
        let imgs = vec![
            solid(20, 20, [255, 0, 0, 255]),
            solid(20, 20, [0, 255, 0, 255]),
            solid(20, 20, [0, 0, 255, 255]),
        ];
        // 3 images, grid cols=ceil(sqrt(3))=2 rows=2, gap=0 → 40x40.
        let png = collage(imgs, Layout::Grid, None, 0, [255, 255, 255, 255]).unwrap();
        let decoded = image::load_from_memory(&png).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (40, 40));
    }

    #[test]
    fn from_bytes_decodes_and_builds() {
        let a = png_bytes(&solid(16, 16, [10, 20, 30, 255]));
        let b = png_bytes(&solid(16, 16, [40, 50, 60, 255]));
        let png = collage_from_bytes(&[a, b], Layout::Vertical, None, 0, [0, 0, 0, 255]).unwrap();
        let decoded = image::load_from_memory(&png).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (16, 32));
    }

    #[test]
    fn errors() {
        assert!(collage(vec![], Layout::Grid, None, 0, [0, 0, 0, 255]).is_err());
        assert!(collage_from_bytes(&[b"not an image".to_vec()], Layout::Grid, None, 0, [0; 4]).is_err());
    }
}
