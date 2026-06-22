//! gizza-ai/animated-heatmap core — take a time-ordered sequence of numeric
//! matrices and render an animated heatmap (GIF) showing how the values evolve
//! frame by frame. Pure-Rust (`image` crate's GIF encoder) — no ffmpeg, so it
//! runs on every backend including the chat Service Worker.
//!
//! Each matrix becomes one frame. A SINGLE global color scale (the min/max over
//! ALL frames, unless an explicit scale is given) is used so a cell of the same
//! value has the same color across frames — that's what makes the animation a
//! meaningful comparison rather than per-frame re-normalized noise.

use std::io::Cursor;

use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, Rgba, RgbaImage};

/// Sequential colormap: t in [0,1] -> RGB. RdYlBu reversed
/// (low = blue #2c7bb6, mid = pale yellow #ffffbf, high = red #d7191c).
pub fn color_at(t: f64) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    const STOPS: [(f64, (f64, f64, f64)); 3] = [
        (0.0, (44.0, 123.0, 182.0)),
        (0.5, (255.0, 255.0, 191.0)),
        (1.0, (215.0, 25.0, 28.0)),
    ];
    let (mut lo, mut hi) = (&STOPS[0], &STOPS[1]);
    if t > 0.5 {
        lo = &STOPS[1];
        hi = &STOPS[2];
    }
    let span = hi.0 - lo.0;
    let f = if span == 0.0 { 0.0 } else { (t - lo.0) / span };
    let lerp = |a: f64, b: f64| a + (b - a) * f;
    (
        lerp(lo.1 .0, hi.1 .0).round() as u8,
        lerp(lo.1 .1, hi.1 .1).round() as u8,
        lerp(lo.1 .2, hi.1 .2).round() as u8,
    )
}

/// Parse one matrix block: rows of comma/space/tab-separated numbers.
fn parse_matrix(block: &str, frame_idx: usize) -> Result<Vec<Vec<f64>>, String> {
    let mut rows: Vec<Vec<f64>> = Vec::new();
    for (li, line) in block.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut row = Vec::new();
        for tok in line.split([',', ' ', '\t']).filter(|t| !t.trim().is_empty()) {
            let v: f64 = tok.trim().parse().map_err(|_| {
                format!("frame {}, row {}: '{tok}' is not a number", frame_idx + 1, li + 1)
            })?;
            if !v.is_finite() {
                return Err(format!(
                    "frame {}, row {}: non-finite value",
                    frame_idx + 1,
                    li + 1
                ));
            }
            row.push(v);
        }
        if !row.is_empty() {
            rows.push(row);
        }
    }
    if rows.is_empty() {
        return Err(format!("frame {}: no numeric rows found", frame_idx + 1));
    }
    let ncol = rows[0].len();
    if rows.iter().any(|r| r.len() != ncol) {
        return Err(format!(
            "frame {}: all rows must have the same number of columns",
            frame_idx + 1
        ));
    }
    Ok(rows)
}

/// Split the input into matrix blocks separated by one or more BLANK lines.
/// Each non-empty group of consecutive lines is one frame.
pub fn parse_frames(data: &str) -> Result<Vec<Vec<Vec<f64>>>, String> {
    let mut blocks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for line in data.lines() {
        if line.trim().is_empty() {
            if !cur.trim().is_empty() {
                blocks.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push_str(line);
            cur.push('\n');
        }
    }
    if !cur.trim().is_empty() {
        blocks.push(cur);
    }
    if blocks.is_empty() {
        return Err("no matrices found (provide one or more numeric matrices separated by a blank line)".into());
    }

    let frames: Vec<Vec<Vec<f64>>> = blocks
        .iter()
        .enumerate()
        .map(|(i, b)| parse_matrix(b, i))
        .collect::<Result<_, _>>()?;

    // All frames must share the same dimensions.
    let (nr, nc) = (frames[0].len(), frames[0][0].len());
    for (i, f) in frames.iter().enumerate() {
        if f.len() != nr || f[0].len() != nc {
            return Err(format!(
                "frame {} is {}x{} but frame 1 is {nr}x{nc} — every frame (matrix) must have the same shape",
                i + 1,
                f.len(),
                f[0].len()
            ));
        }
    }
    Ok(frames)
}

/// Render one matrix to an RGBA frame of the given pixel size.
fn render_frame(matrix: &[Vec<f64>], cell: u32, border: u32, min: f64, range: f64) -> RgbaImage {
    let nr = matrix.len() as u32;
    let nc = matrix[0].len() as u32;
    let w = nc * cell;
    let h = nr * cell;
    // Background (cell-border color) white.
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));
    for (i, row) in matrix.iter().enumerate() {
        for (j, &v) in row.iter().enumerate() {
            let t = if range == 0.0 { 0.5 } else { (v - min) / range };
            let (r, g, b) = color_at(t);
            let x0 = j as u32 * cell;
            let y0 = i as u32 * cell;
            for y in y0..y0 + cell {
                for x in x0..x0 + cell {
                    // leave a `border`-px white gap on the top/left edges of each cell
                    let on_border = border > 0 && ((x - x0) < border || (y - y0) < border);
                    if !on_border {
                        img.put_pixel(x, y, Rgba([r, g, b, 255]));
                    }
                }
            }
        }
    }
    img
}

/// Build an animated heatmap GIF from `data` (matrices separated by blank lines).
///
/// * `delay_ms` — per-frame delay (10-60000).
/// * `cell_px`  — pixel size of each matrix cell (4-200).
/// * `scale_min` / `scale_max` — optional fixed color-scale bounds; when both are
///   `None` the global min/max over all frames is used so colors are comparable
///   across frames.
pub fn animated_heatmap(
    data: &str,
    delay_ms: u16,
    cell_px: u32,
    scale_min: Option<f64>,
    scale_max: Option<f64>,
) -> Result<Vec<u8>, String> {
    let frames = parse_frames(data)?;
    let cell = cell_px.clamp(4, 200);
    let border = if cell >= 8 { 1 } else { 0 };
    let delay = delay_ms.clamp(10, 60_000);

    // Global color scale across all frames (unless explicitly given).
    let mut g_min = f64::INFINITY;
    let mut g_max = f64::NEG_INFINITY;
    for f in &frames {
        for r in f {
            for &v in r {
                g_min = g_min.min(v);
                g_max = g_max.max(v);
            }
        }
    }
    let min = scale_min.unwrap_or(g_min);
    let max = scale_max.unwrap_or(g_max);
    if scale_min.is_some() && scale_max.is_some() && min >= max {
        return Err("scale_min must be less than scale_max".into());
    }
    let range = max - min;

    let mut out = Cursor::new(Vec::new());
    {
        let mut enc = GifEncoder::new(&mut out);
        enc.set_repeat(Repeat::Infinite).map_err(|e| format!("gif repeat: {e}"))?;
        for f in &frames {
            let img = render_frame(f, cell, border, min, range);
            let frame = Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(delay as u32, 1));
            enc.encode_frame(frame).map_err(|e| format!("gif encode frame: {e}"))?;
        }
    }
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::AnimationDecoder;

    #[test]
    fn colormap_endpoints() {
        assert_eq!(color_at(0.0), (44, 123, 182));
        assert_eq!(color_at(0.5), (255, 255, 191));
        assert_eq!(color_at(1.0), (215, 25, 28));
    }

    #[test]
    fn parses_blank_separated_frames() {
        let frames = parse_frames("1,2\n3,4\n\n5,6\n7,8\n\n9,10\n11,12").unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0], vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        assert_eq!(frames[2][1], vec![11.0, 12.0]);
    }

    #[test]
    fn single_frame_ok() {
        let frames = parse_frames("1 2 3\n4 5 6").unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].len(), 2);
        assert_eq!(frames[0][0].len(), 3);
    }

    #[test]
    fn builds_multiframe_gif() {
        let gif = animated_heatmap("0,1\n2,3\n\n3,2\n1,0\n\n1,1\n1,1", 200, 16, None, None).unwrap();
        assert_eq!(&gif[0..6], b"GIF89a");
        let dec = image::codecs::gif::GifDecoder::new(Cursor::new(gif)).unwrap();
        let decoded: Vec<_> = dec.into_frames().collect::<Result<_, _>>().unwrap();
        assert_eq!(decoded.len(), 3);
        let (w, h) = decoded[0].buffer().dimensions();
        assert_eq!((w, h), (2 * 16, 2 * 16));
    }

    #[test]
    fn global_scale_is_consistent_across_frames() {
        // Same value (3) appears in frame 1 and frame 2; with a global scale it
        // must map to the same color, so the two frames' matching cells match.
        let gif = animated_heatmap("0,3\n0,0\n\n3,0\n0,0", 100, 20, None, None).unwrap();
        let dec = image::codecs::gif::GifDecoder::new(Cursor::new(gif)).unwrap();
        let frames: Vec<_> = dec.into_frames().collect::<Result<_, _>>().unwrap();
        let f0 = frames[0].buffer();
        let f1 = frames[1].buffer();
        // value 3 is top-right cell in frame 0, top-left cell in frame 1.
        let p0 = f0.get_pixel(30, 10); // top-right cell center
        let p1 = f1.get_pixel(10, 10); // top-left cell center
        assert_eq!(p0, p1);
    }

    #[test]
    fn explicit_scale_clamps() {
        let gif = animated_heatmap("0,5\n10,5", 100, 12, Some(0.0), Some(10.0)).unwrap();
        assert_eq!(&gif[0..3], b"GIF");
    }

    #[test]
    fn errors() {
        assert!(animated_heatmap("", 100, 16, None, None).is_err());
        // ragged within a frame
        assert!(animated_heatmap("1,2\n3", 100, 16, None, None).is_err());
        // non-numeric
        assert!(animated_heatmap("1,x", 100, 16, None, None).is_err());
        // mismatched frame shapes
        assert!(animated_heatmap("1,2\n3,4\n\n5,6,7", 100, 16, None, None).is_err());
        // bad explicit scale
        assert!(animated_heatmap("1,2\n3,4", 100, 16, Some(5.0), Some(1.0)).is_err());
    }
}
