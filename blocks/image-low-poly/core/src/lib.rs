//! gizza-ai/image-low-poly core — turn a photo into low-poly triangle art.
//!
//! The low-poly look = a triangle mesh laid over the image, every triangle
//! flat-filled with the colour of the source region beneath it. We build the
//! mesh deterministically instead of running a Delaunay triangulation, which
//! keeps the wasm small and the output reproducible:
//!
//!   1. Pick a grid of `cols x rows` quads whose count matches the requested
//!      triangle budget (2 triangles per quad) and whose cells stay roughly
//!      square for the image's aspect ratio.
//!   2. Move every interior grid vertex. Two forces combine: a seeded pseudo-
//!      random scatter (so `seed` reshuffles the mesh like a "regenerate"
//!      button) and a pull towards the strongest edge in a Sobel map of the
//!      image (so `edge_focus` makes triangle corners land on contours). The
//!      total offset is capped below half a cell, so the quads still tile the
//!      image exactly — no folds, no gaps.
//!   3. Split each quad into two triangles along a seed-chosen diagonal and
//!      flat-fill each one (`average` = mean of the covered source pixels,
//!      `centroid` = the single pixel under the triangle's centroid).
//!   4. Optionally stroke the mesh wireframe on top.
//!
//! Triangles are rasterised with a sub-pixel overlap so adjacent fills never
//! leave the hairline seams this effect is prone to.
//!
//! Pure-Rust (`image` only) so it runs on every backend incl. the chat SW.
//! Returns PNG bytes at the source dimensions.

use std::io::Cursor;

use image::{imageops::FilterType, DynamicImage, GrayImage, ImageFormat, Rgba, RgbaImage};

/// How each triangle picks its flat fill colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// Mean of the source pixels the triangle covers — smoother, closer to the photo.
    Average,
    /// The single source pixel under the triangle's centroid — punchier, higher contrast.
    Centroid,
}

impl ColorMode {
    /// Parse the schema enum. An empty string means "unset" and maps to the default.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "average" => Ok(ColorMode::Average),
            "centroid" => Ok(ColorMode::Centroid),
            other => Err(format!(
                "invalid color_mode '{other}' (use 'average' or 'centroid')"
            )),
        }
    }
}

/// Everything [`low_poly`] needs besides the image bytes. Values are clamped to
/// the documented ranges inside `low_poly`, so out-of-range input never panics.
#[derive(Debug, Clone)]
pub struct Options {
    /// Approximate triangle count, 50..=4000. Rounded to an even grid.
    pub triangles: u32,
    /// 0..=100. Higher pulls mesh corners onto high-contrast edges and damps the
    /// random scatter; lower gives a looser, more evenly scattered mesh.
    pub edge_focus: u32,
    pub color_mode: ColorMode,
    /// Wireframe colour, `#rgb` / `#rrggbb` (with or without the `#`).
    pub stroke: String,
    /// Wireframe width in pixels, 0..=6. 0 draws no wireframe.
    pub stroke_width: f32,
    /// Reshuffles vertex scatter and diagonal choice. Same seed = same output.
    pub seed: u64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            triangles: 800,
            edge_focus: 60,
            color_mode: ColorMode::Average,
            stroke: "#1f2937".into(),
            stroke_width: 0.0,
            seed: 1,
        }
    }
}

/// Parse `#rrggbb` / `rrggbb` / `#rgb` into an opaque RGBA.
fn parse_color(s: &str) -> Result<[u8; 4], String> {
    let h = s.trim().trim_start_matches('#');
    let byte = |c: &str| u8::from_str_radix(c, 16).map_err(|_| format!("invalid color '{s}'"));
    match h.len() {
        6 => Ok([byte(&h[0..2])?, byte(&h[2..4])?, byte(&h[4..6])?, 255]),
        3 => {
            let cs: Vec<char> = h.chars().collect();
            let d = |c: char| {
                u8::from_str_radix(&format!("{c}{c}"), 16).map_err(|_| format!("invalid color '{s}'"))
            };
            Ok([d(cs[0])?, d(cs[1])?, d(cs[2])?, 255])
        }
        _ => Err(format!("invalid color '{s}' (use #rrggbb or #rgb)")),
    }
}

/// SplitMix64 over (seed, a, b) — a cheap, portable, fully deterministic hash so
/// the same seed always reproduces the same mesh on every backend.
fn hash2(seed: u64, a: u32, b: u32) -> u64 {
    let mut x = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(((a as u64) << 32) | b as u64)
        .wrapping_add(0x2545_F491_4F6C_DD1D);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A hash mapped into `0.0..1.0`.
fn unit(h: u64) -> f32 {
    ((h >> 40) as f32) / ((1u64 << 24) as f32)
}

/// A downscaled Sobel gradient-magnitude map. Computed on a small copy of the
/// image (<=256px on the long side) so the edge search is cheap regardless of
/// the source resolution.
struct EdgeMap {
    w: u32,
    h: u32,
    mag: Vec<f32>,
}

impl EdgeMap {
    fn build(gray: &GrayImage) -> EdgeMap {
        let (w, h) = gray.dimensions();
        let mut mag = vec![0.0f32; (w * h) as usize];
        let at = |x: u32, y: u32| gray.get_pixel(x, y).0[0] as f32;
        for y in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                let gx = at(x + 1, y - 1) + 2.0 * at(x + 1, y) + at(x + 1, y + 1)
                    - at(x - 1, y - 1)
                    - 2.0 * at(x - 1, y)
                    - at(x - 1, y + 1);
                let gy = at(x - 1, y + 1) + 2.0 * at(x, y + 1) + at(x + 1, y + 1)
                    - at(x - 1, y - 1)
                    - 2.0 * at(x, y - 1)
                    - at(x + 1, y - 1);
                mag[(y * w + x) as usize] = (gx * gx + gy * gy).sqrt();
            }
        }
        EdgeMap { w, h, mag }
    }

    /// Strongest gradient pixel within `±rx, ±ry` of `(cx, cy)`, in map
    /// coordinates. `None` when the whole window is flat (nothing to snap to).
    /// Ties resolve to the first pixel scanned, so the result is deterministic.
    fn strongest(&self, cx: f32, cy: f32, rx: f32, ry: f32) -> Option<(f32, f32)> {
        let x0 = (cx - rx).floor().max(0.0) as u32;
        let x1 = ((cx + rx).ceil() as i64).min(self.w as i64 - 1);
        let y0 = (cy - ry).floor().max(0.0) as u32;
        let y1 = ((cy + ry).ceil() as i64).min(self.h as i64 - 1);
        if x1 < x0 as i64 || y1 < y0 as i64 {
            return None;
        }
        let mut best = 0.0f32;
        let mut at = None;
        for y in y0..=(y1 as u32) {
            for x in x0..=(x1 as u32) {
                let m = self.mag[(y * self.w + x) as usize];
                if m > best {
                    best = m;
                    at = Some((x as f32 + 0.5, y as f32 + 0.5));
                }
            }
        }
        // A near-flat window has no contour worth snapping to; keep the scatter.
        if best < 8.0 {
            None
        } else {
            at
        }
    }
}

/// Twice the signed area of triangle `(a, b, p)`. Positive when `p` is left of
/// `a -> b`. Doubles as the barycentric edge function during rasterisation.
fn edge_fn(a: (f32, f32), b: (f32, f32), p: (f32, f32)) -> f32 {
    (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
}

fn len(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
}

/// Convert `bytes` into low-poly triangle art. Returns PNG bytes at the source
/// dimensions. Errors on undecodable input or a malformed `stroke` colour.
pub fn low_poly(bytes: &[u8], opts: &Options) -> Result<Vec<u8>, String> {
    // Parse the stroke colour up front so a typo fails fast even at width 0.
    let stroke_color = parse_color(&opts.stroke)?;

    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let src = img.to_rgba8();
    let (w, h) = src.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }

    let triangles = opts.triangles.clamp(50, 4000);
    let ef = opts.edge_focus.min(100) as f32 / 100.0;
    let stroke_width = opts.stroke_width.clamp(0.0, 6.0);

    // 1. Grid sized to the triangle budget (2 per quad), cells kept ~square.
    let cells = (triangles / 2).max(1) as f32;
    let aspect = (w as f32 / h as f32).max(0.01);
    let rows = (cells / aspect).sqrt().round().max(1.0);
    let cols = (cells / rows).round().max(1.0);
    let (cols, rows) = (cols as u32, rows as u32);
    let cw = w as f32 / cols as f32;
    let ch = h as f32 / rows as f32;

    // 2. Jittered + edge-snapped vertices, row-major over (cols+1) x (rows+1).
    let emap = {
        let long = w.max(h).max(1);
        let scale = (256.0 / long as f32).min(1.0);
        let ew = ((w as f32 * scale).round() as u32).max(3);
        let eh = ((h as f32 * scale).round() as u32).max(3);
        EdgeMap::build(&img.resize_exact(ew, eh, FilterType::Triangle).to_luma8())
    };
    let (sx, sy) = (emap.w as f32 / w as f32, emap.h as f32 / h as f32);
    // Random scatter shrinks as edge_focus rises: high focus = ordered mesh
    // following contours, low focus = loose scatter.
    let amp = 0.42 * (1.0 - 0.6 * ef);
    let (cap_x, cap_y) = (0.45 * cw, 0.45 * ch);

    let mut verts = Vec::with_capacity(((cols + 1) * (rows + 1)) as usize);
    for j in 0..=rows {
        for i in 0..=cols {
            let (bx, by) = (i as f32 * cw, j as f32 * ch);
            if i == 0 || j == 0 || i == cols || j == rows {
                // Border vertices stay pinned so the mesh covers the full frame.
                verts.push((bx, by));
                continue;
            }
            let mut ox = (unit(hash2(opts.seed, i, j)) - 0.5) * 2.0 * amp * cw;
            let mut oy = (unit(hash2(opts.seed ^ 0xA5A5_5A5A, i, j)) - 0.5) * 2.0 * amp * ch;
            if ef > 0.0 {
                if let Some((ex, ey)) =
                    emap.strongest(bx * sx, by * sy, (cap_x * sx).max(1.0), (cap_y * sy).max(1.0))
                {
                    ox = ox * (1.0 - ef) + (ex / sx - bx) * ef;
                    oy = oy * (1.0 - ef) + (ey / sy - by) * ef;
                }
            }
            // Capping below half a cell keeps the quads a valid tiling.
            verts.push((bx + ox.clamp(-cap_x, cap_x), by + oy.clamp(-cap_y, cap_y)));
        }
    }
    let v = |i: u32, j: u32| verts[(j * (cols + 1) + i) as usize];

    // 3. Two triangles per quad, diagonal chosen by the seed.
    let mut out = RgbaImage::new(w, h);
    for j in 0..rows {
        for i in 0..cols {
            let (a, b, c, d) = (v(i, j), v(i + 1, j), v(i + 1, j + 1), v(i, j + 1));
            let tris = if hash2(opts.seed ^ 0x5EED_5EED, i, j) & 1 == 0 {
                [[a, b, c], [a, c, d]]
            } else {
                [[a, b, d], [b, c, d]]
            };
            for t in tris {
                fill_triangle(&mut out, &src, t, opts.color_mode);
            }
        }
    }

    // 4. Optional wireframe. Each mesh edge is drawn exactly once (all
    //    horizontals, all verticals, then one diagonal per quad) so shared
    //    edges don't blend twice.
    if stroke_width > 0.0 {
        for j in 0..=rows {
            for i in 0..cols {
                draw_line(&mut out, v(i, j), v(i + 1, j), stroke_color, stroke_width);
            }
        }
        for j in 0..rows {
            for i in 0..=cols {
                draw_line(&mut out, v(i, j), v(i, j + 1), stroke_color, stroke_width);
            }
        }
        for j in 0..rows {
            for i in 0..cols {
                let (p, q) = if hash2(opts.seed ^ 0x5EED_5EED, i, j) & 1 == 0 {
                    (v(i, j), v(i + 1, j + 1))
                } else {
                    (v(i + 1, j), v(i, j + 1))
                };
                draw_line(&mut out, p, q, stroke_color, stroke_width);
            }
        }
    }

    let mut buf = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(out)
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(buf.into_inner())
}

/// Flat-fill one triangle of `out` with the colour of the source region it covers.
fn fill_triangle(out: &mut RgbaImage, src: &RgbaImage, tri: [(f32, f32); 3], mode: ColorMode) {
    let (w, h) = src.dimensions();
    let [a, mut b, mut c] = tri;
    let mut area2 = edge_fn(a, b, c);
    if area2 < 0.0 {
        // Normalise winding so the inside test is a single sign comparison.
        std::mem::swap(&mut b, &mut c);
        area2 = -area2;
    }
    if area2 < 1e-3 {
        return; // degenerate sliver — nothing to fill
    }

    let x0 = a.0.min(b.0).min(c.0).floor().max(0.0) as u32;
    let y0 = a.1.min(b.1).min(c.1).floor().max(0.0) as u32;
    let x1 = (a.0.max(b.0).max(c.0).ceil() as i64).min(w as i64 - 1);
    let y1 = (a.1.max(b.1).max(c.1).ceil() as i64).min(h as i64 - 1);
    if x1 < x0 as i64 || y1 < y0 as i64 {
        return;
    }
    let (x1, y1) = (x1 as u32, y1 as u32);

    // The edge function equals (edge length x signed distance), so a bias of
    // 0.7 x length admits pixel centres up to ~0.7px outside. Neighbouring
    // triangles therefore overlap slightly instead of leaving hairline seams.
    let (bias_ab, bias_bc, bias_ca) = (0.7 * len(a, b), 0.7 * len(b, c), 0.7 * len(c, a));
    let inside = |p: (f32, f32)| {
        edge_fn(a, b, p) >= -bias_ab
            && edge_fn(b, c, p) >= -bias_bc
            && edge_fn(c, a, p) >= -bias_ca
    };

    let centroid = (
        (a.0 + b.0 + c.0) / 3.0,
        (a.1 + b.1 + c.1) / 3.0,
    );
    let sample = |x: f32, y: f32| {
        *src.get_pixel(
            (x.max(0.0) as u32).min(w - 1),
            (y.max(0.0) as u32).min(h - 1),
        )
    };

    let color = match mode {
        ColorMode::Centroid => sample(centroid.0, centroid.1).0,
        ColorMode::Average => {
            // Sample on a stride so a big triangle costs about as much as a
            // small one (~256 samples), then average in premultiplied space so
            // transparent regions don't drag the colour toward black.
            let step = (((area2 * 0.5) / 256.0).sqrt().ceil() as u32).max(1);
            let (mut sr, mut sg, mut sb, mut sa) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            let mut n = 0.0f64;
            let mut y = y0;
            while y <= y1 {
                let mut x = x0;
                while x <= x1 {
                    if inside((x as f32 + 0.5, y as f32 + 0.5)) {
                        let p = src.get_pixel(x, y).0;
                        let al = p[3] as f64 / 255.0;
                        sr += p[0] as f64 * al;
                        sg += p[1] as f64 * al;
                        sb += p[2] as f64 * al;
                        sa += al;
                        n += 1.0;
                    }
                    x += step;
                }
                y += step;
            }
            if n == 0.0 {
                // Thinner than the stride — fall back to the centroid pixel.
                sample(centroid.0, centroid.1).0
            } else if sa <= 0.0 {
                [0, 0, 0, 0] // fully transparent region
            } else {
                [
                    (sr / sa).round().clamp(0.0, 255.0) as u8,
                    (sg / sa).round().clamp(0.0, 255.0) as u8,
                    (sb / sa).round().clamp(0.0, 255.0) as u8,
                    ((sa / n) * 255.0).round().clamp(0.0, 255.0) as u8,
                ]
            }
        }
    };

    for y in y0..=y1 {
        for x in x0..=x1 {
            if inside((x as f32 + 0.5, y as f32 + 0.5)) {
                out.put_pixel(x, y, Rgba(color));
            }
        }
    }
}

/// Draw an anti-aliased line segment of `width` px.
fn draw_line(out: &mut RgbaImage, p0: (f32, f32), p1: (f32, f32), color: [u8; 4], width: f32) {
    let (w, h) = out.dimensions();
    let hw = width * 0.5;
    let pad = hw + 1.0;
    let x0 = (p0.0.min(p1.0) - pad).floor().max(0.0) as u32;
    let y0 = (p0.1.min(p1.1) - pad).floor().max(0.0) as u32;
    let x1 = ((p0.0.max(p1.0) + pad).ceil() as i64).min(w as i64 - 1);
    let y1 = ((p0.1.max(p1.1) + pad).ceil() as i64).min(h as i64 - 1);
    if x1 < x0 as i64 || y1 < y0 as i64 {
        return;
    }

    let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
    let seg2 = dx * dx + dy * dy;
    for y in y0..=(y1 as u32) {
        for x in x0..=(x1 as u32) {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // Distance from the pixel centre to the segment.
            let t = if seg2 <= f32::EPSILON {
                0.0
            } else {
                (((px - p0.0) * dx + (py - p0.1) * dy) / seg2).clamp(0.0, 1.0)
            };
            let (nx, ny) = (p0.0 + t * dx - px, p0.1 + t * dy - py);
            let dist = (nx * nx + ny * ny).sqrt();
            let cov = (hw + 0.5 - dist).clamp(0.0, 1.0);
            if cov <= 0.0 {
                continue;
            }
            let dst = out.get_pixel(x, y).0;
            let mix = |s: u8, d: u8| (s as f32 * cov + d as f32 * (1.0 - cov)).round() as u8;
            out.put_pixel(
                x,
                y,
                Rgba([
                    mix(color[0], dst[0]),
                    mix(color[1], dst[1]),
                    mix(color[2], dst[2]),
                    dst[3].max((cov * 255.0).round() as u8),
                ]),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(img: RgbaImage) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    /// A 96x72 image with a smooth gradient plus a hard-edged block, so both the
    /// flat-region and the edge-snapping paths get exercised.
    fn gradient() -> Vec<u8> {
        let mut img = RgbaImage::new(96, 72);
        for y in 0..72u32 {
            for x in 0..96u32 {
                let p = if x > 40 && x < 60 && y > 20 && y < 50 {
                    Rgba([255, 0, 0, 255])
                } else {
                    Rgba([(x * 2) as u8, (y * 3) as u8, ((x + y)) as u8, 255])
                };
                img.put_pixel(x, y, p);
            }
        }
        encode(img)
    }

    fn decode(png: &[u8]) -> RgbaImage {
        image::load_from_memory(png).unwrap().to_rgba8()
    }

    fn distinct_colors(png: &[u8]) -> usize {
        let mut set = std::collections::HashSet::new();
        for p in decode(png).pixels() {
            set.insert(p.0);
        }
        set.len()
    }

    #[test]
    fn happy_path_returns_png_with_nonzero_dimensions() {
        let out = low_poly(&gradient(), &Options::default()).unwrap();
        assert_eq!(&out[..8], b"\x89PNG\r\n\x1a\n", "output is a PNG");
        let img = decode(&out);
        let (w, h) = img.dimensions();
        assert!(w > 0 && h > 0);
        assert_eq!((w, h), (96, 72), "output keeps the source dimensions");
    }

    #[test]
    fn flattens_into_polygon_facets() {
        // The whole frame is covered by flat triangles, so a smooth gradient
        // collapses to far fewer colours than it started with — and never
        // leaves an uncovered (fully transparent) pixel.
        let src = gradient();
        let out = low_poly(&src, &Options::default()).unwrap();
        assert!(
            distinct_colors(&out) < distinct_colors(&src),
            "should flatten the gradient into facets"
        );
        assert!(
            decode(&out).pixels().all(|p| p.0[3] == 255),
            "opaque input must produce full coverage — no seams or holes"
        );
    }

    #[test]
    fn more_triangles_means_more_detail() {
        let src = gradient();
        let coarse = low_poly(&src, &Options { triangles: 50, ..Default::default() }).unwrap();
        let fine = low_poly(&src, &Options { triangles: 4000, ..Default::default() }).unwrap();
        assert!(
            distinct_colors(&fine) > distinct_colors(&coarse),
            "a higher triangle budget must keep more detail"
        );
    }

    #[test]
    fn stroke_width_boundaries() {
        let src = gradient();
        let ink = Rgba([31, 41, 55, 255]);
        let none = low_poly(&src, &Options { stroke_width: 0.0, ..Default::default() }).unwrap();
        assert!(
            !decode(&none).pixels().any(|p| *p == ink),
            "stroke_width 0 draws no wireframe"
        );
        // Both ends of the documented range must render, and the max must be
        // visibly heavier than a hairline.
        let thin = low_poly(&src, &Options { stroke_width: 1.0, ..Default::default() }).unwrap();
        let thick = low_poly(&src, &Options { stroke_width: 6.0, ..Default::default() }).unwrap();
        let count = |png: &[u8]| decode(png).pixels().filter(|p| **p == ink).count();
        assert!(count(&thin) > 0, "stroke_width 1 draws a wireframe");
        assert!(count(&thick) > count(&thin), "6 must be heavier than 1");
    }

    #[test]
    fn color_modes_both_render_and_differ() {
        let src = gradient();
        let avg = low_poly(&src, &Options { color_mode: ColorMode::Average, ..Default::default() }).unwrap();
        let cen = low_poly(&src, &Options { color_mode: ColorMode::Centroid, ..Default::default() }).unwrap();
        assert_ne!(avg, cen, "average and centroid must not be the same image");
        assert_eq!(decode(&cen).dimensions(), (96, 72));
    }

    #[test]
    fn seed_is_deterministic_and_reshuffles() {
        let src = gradient();
        let a = low_poly(&src, &Options { seed: 1, ..Default::default() }).unwrap();
        let b = low_poly(&src, &Options { seed: 1, ..Default::default() }).unwrap();
        assert_eq!(a, b, "same seed must give byte-identical output");
        // A low edge_focus leaves the scatter free, so the seed really moves the mesh.
        let o = Options { edge_focus: 0, ..Default::default() };
        let c = low_poly(&src, &Options { seed: 7, ..o.clone() }).unwrap();
        let d = low_poly(&src, &Options { seed: 8, ..o }).unwrap();
        assert_ne!(c, d, "a different seed must reshuffle the mesh");
    }

    #[test]
    fn edge_focus_changes_the_mesh() {
        let src = gradient();
        let loose = low_poly(&src, &Options { edge_focus: 0, ..Default::default() }).unwrap();
        let tight = low_poly(&src, &Options { edge_focus: 100, ..Default::default() }).unwrap();
        assert_ne!(loose, tight, "edge_focus must affect vertex placement");
    }

    #[test]
    fn solid_image_stays_solid() {
        let src = encode(RgbaImage::from_pixel(48, 48, Rgba([10, 120, 200, 255])));
        let out = low_poly(&src, &Options::default()).unwrap();
        assert_eq!(distinct_colors(&out), 1);
    }

    #[test]
    fn clamps_out_of_range_without_panicking() {
        let src = gradient();
        assert!(low_poly(&src, &Options { triangles: 0, edge_focus: 9999, stroke_width: 99.0, ..Default::default() }).is_ok());
        assert!(low_poly(&src, &Options { triangles: 100_000, stroke_width: -5.0, ..Default::default() }).is_ok());
    }

    #[test]
    fn tiny_image_is_handled() {
        let out = low_poly(&encode(RgbaImage::new(1, 1)), &Options::default()).unwrap();
        assert_eq!(decode(&out).dimensions(), (1, 1));
    }

    #[test]
    fn errors_on_undecodable_image() {
        let err = low_poly(b"not an image", &Options::default()).unwrap_err();
        assert!(err.contains("could not decode image"), "got: {err}");
    }

    #[test]
    fn errors_on_invalid_stroke_color() {
        let src = gradient();
        // Rejected even at width 0, so a typo never silently does nothing.
        let err = low_poly(&src, &Options { stroke: "nope".into(), ..Default::default() }).unwrap_err();
        assert!(err.contains("invalid color"), "got: {err}");
        assert!(low_poly(&src, &Options { stroke: "#f0a".into(), stroke_width: 2.0, ..Default::default() }).is_ok());
        assert!(low_poly(&src, &Options { stroke: "ff00aa".into(), stroke_width: 2.0, ..Default::default() }).is_ok());
    }

    #[test]
    fn color_mode_parses_and_rejects() {
        assert_eq!(ColorMode::parse("average").unwrap(), ColorMode::Average);
        assert_eq!(ColorMode::parse("centroid").unwrap(), ColorMode::Centroid);
        assert_eq!(ColorMode::parse("").unwrap(), ColorMode::Average);
        assert!(ColorMode::parse("delaunay").unwrap_err().contains("invalid color_mode"));
    }
}
