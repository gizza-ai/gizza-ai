//! gizza-ai/multi-photo-scan-splitter core — detect the separate photos on one
//! flatbed scan, auto-straighten (deskew) each, crop it out, and bundle every
//! photo as its own image in a ZIP. Pure-Rust (`image` + `zip`), no ML / no
//! ffmpeg, so it runs on every backend including the chat Service Worker.
//!
//! Pipeline:
//!   decode → cap the working long side (memory budget) → sample the scanner
//!   background (auto / white / black) → foreground mask (pixels that differ
//!   from the background in luma or saturation) → 8-connected components →
//!   drop specks below `min_size` → per blob: min-area rectangle → rotate the
//!   crop upright (unless `straighten` is off) → optional `edge_trim` inset →
//!   encode → ZIP as photo_1.<ext>, photo_2.<ext>, …
//!
//! It is a CLASSICAL detector (threshold + connected components + min-area
//! rectangle), not ML: it works when the photos have a clear gap between them
//! and the scanner background contrasts with them. Touching photos merge into
//! one region; that is the standard "leave space around each photo" workflow.

use std::io::{Cursor, Write};

use image::{ImageFormat, Rgb, RgbImage};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// The scan is analysed and cropped at up to this long-side resolution so a
/// 600-dpi scan stays inside the wasm sandbox memory budget.
pub const MAX_DIM: u32 = 1600;

/// Which colour the scanner bed / lid is, so we know what "background" looks
/// like. `Auto` samples the scan border and decides light-vs-dark itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Background {
    Auto,
    White,
    Black,
}

impl Background {
    pub fn parse(s: &str) -> Result<Background, String> {
        match s {
            "auto" => Ok(Background::Auto),
            "white" => Ok(Background::White),
            "black" => Ok(Background::Black),
            other => Err(format!(
                "unknown background `{other}` (expected auto, white or black)"
            )),
        }
    }
}

/// Per-photo output encoding.
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
    pub background: Background,
    /// Auto-rotate each detected photo upright (removes scan skew). Default on.
    pub straighten: bool,
    /// Ignore detected regions whose width OR height is under this many pixels
    /// (dust, speckle, torn corners), measured at the working resolution.
    pub min_size: u32,
    /// Trim this many pixels inward on every side of each crop, to shave off
    /// residual scanner-bed bleed.
    pub edge_trim: u32,
    pub format: OutFormat,
    /// Filename base: photo -> photo_1.png, photo_2.png, …
    pub prefix: String,
    /// Stop after this many photos (largest first). `None` = all detected.
    pub max_photos: Option<usize>,
}

impl Default for SplitParams {
    fn default() -> Self {
        SplitParams {
            background: Background::Auto,
            straighten: true,
            min_size: 48,
            edge_trim: 0,
            format: OutFormat::Png,
            prefix: "photo".to_string(),
            max_photos: None,
        }
    }
}

/// What was produced, for the human/LLM summary.
#[derive(Clone, Debug)]
pub struct Summary {
    pub photos: usize,
    pub background: &'static str,
    pub straightened: bool,
    /// (width, height) of each written photo, in output order.
    pub sizes: Vec<(u32, u32)>,
}

/// Detect + straighten + crop every photo in `image_bytes`, returning
/// (ZIP bytes, summary).
pub fn split_photos(
    image_bytes: &[u8],
    params: &SplitParams,
) -> Result<(Vec<u8>, Summary), String> {
    let decoded = image::load_from_memory(image_bytes)
        .map_err(|e| format!("could not decode image: {e}"))?
        .to_rgb8();
    let (iw, ih) = (decoded.width(), decoded.height());
    if iw < 8 || ih < 8 {
        return Err("image is too small to split".into());
    }

    // Cap the working long side to bound memory.
    let img = downscale_to(decoded, MAX_DIM);
    let (w, h) = (img.width(), img.height());

    // 1. Background colour + polarity.
    let (bg_luma, _bg_rgb) = border_background(&img);
    let white_bg = match params.background {
        Background::White => true,
        Background::Black => false,
        Background::Auto => bg_luma >= 128,
    };
    let bg_label = if white_bg { "white" } else { "black" };
    let fill = if white_bg { Rgb([255, 255, 255]) } else { Rgb([0, 0, 0]) };

    // 2. Foreground mask: pixels that differ from the background.
    let mask = foreground_mask(&img, white_bg, bg_luma);

    // 3. Connected components (8-connected), collecting bbox + boundary points.
    let mut blobs = connected_components(&mask, w, h);

    // 4. Drop specks smaller than min_size in either dimension.
    let min_size = params.min_size.max(1);
    blobs.retain(|b| (b.max_x - b.min_x + 1) >= min_size && (b.max_y - b.min_y + 1) >= min_size);
    if blobs.is_empty() {
        return Err(format!(
            "no separate photos were detected on the {bg_label} background. Make sure each photo \
             has a clear gap around it, the scan background contrasts with the photos, and lower \
             min_size if the photos are small."
        ));
    }

    // 5. Largest first for max_photos, then reading order (top→bottom, left→right).
    blobs.sort_by(|a, b| b.area.cmp(&a.area));
    if let Some(max) = params.max_photos {
        blobs.truncate(max.max(1));
    }
    let row_tol = (h / 20).max(8) as i64;
    blobs.sort_by(|a, b| {
        let ay = (a.min_y as i64) / row_tol;
        let by = (b.min_y as i64) / row_tol;
        ay.cmp(&by).then(a.min_x.cmp(&b.min_x))
    });

    // 6. Crop / straighten each photo → ZIP.
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let prefix = {
        let p = params.prefix.trim();
        if p.is_empty() {
            "photo"
        } else {
            p
        }
    };
    let pad = blobs.len().to_string().len().max(1);
    let ext = params.format.ext();

    let mut sizes: Vec<(u32, u32)> = Vec::new();
    for (i, blob) in blobs.iter().enumerate() {
        let crop = if params.straighten {
            straighten_crop(&img, blob, fill)
        } else {
            axis_crop(&img, blob)
        };
        let crop = apply_trim(crop, params.edge_trim, fill);
        if crop.width() == 0 || crop.height() == 0 {
            continue;
        }
        let encoded = encode(&crop, params.format)?;
        let idx = i + 1;
        let name = format!("{prefix}_{idx:0pad$}.{ext}");
        zip.start_file(name, opts)
            .map_err(|e| format!("zip error: {e}"))?;
        zip.write_all(&encoded)
            .map_err(|e| format!("zip write error: {e}"))?;
        sizes.push((crop.width(), crop.height()));
    }

    if sizes.is_empty() {
        return Err("photos were detected but every crop was empty after edge_trim — lower \
                    edge_trim."
            .into());
    }

    let zip_bytes = zip
        .finish()
        .map_err(|e| format!("zip finalize error: {e}"))?
        .into_inner();

    Ok((
        zip_bytes,
        Summary {
            photos: sizes.len(),
            background: bg_label,
            straightened: params.straighten,
            sizes,
        },
    ))
}

// ---------------------------------------------------------------------------
// Working image + background.
// ---------------------------------------------------------------------------

fn downscale_to(img: RgbImage, max_dim: u32) -> RgbImage {
    let (w, h) = (img.width(), img.height());
    if w.max(h) <= max_dim {
        return img;
    }
    let scale = f64::from(max_dim) / f64::from(w.max(h));
    let nw = ((f64::from(w) * scale).round() as u32).max(1);
    let nh = ((f64::from(h) * scale).round() as u32).max(1);
    image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle)
}

fn luma601(p: &Rgb<u8>) -> u8 {
    (0.299 * f64::from(p.0[0]) + 0.587 * f64::from(p.0[1]) + 0.114 * f64::from(p.0[2])).round() as u8
}

/// Sample a thin frame of border pixels to estimate the scanner-bed colour and
/// its mean luma.
fn border_background(img: &RgbImage) -> (u8, Rgb<u8>) {
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
    let rgb = Rgb([
        (sum[0] / n) as u8,
        (sum[1] / n) as u8,
        (sum[2] / n) as u8,
    ]);
    (luma601(&rgb), rgb)
}

/// A pixel is foreground when it is clearly darker/lighter than the background
/// (luma delta) OR noticeably saturated (a colour photo on a neutral bed).
fn foreground_mask(img: &RgbImage, white_bg: bool, bg_luma: u8) -> Vec<bool> {
    const LUMA_DELTA: i32 = 32;
    const SAT_DELTA: i32 = 40;
    let (w, h) = (img.width(), img.height());
    let mut mask = vec![false; (w * h) as usize];
    for (i, p) in img.pixels().enumerate() {
        let l = i32::from(luma601(p));
        let bg = i32::from(bg_luma);
        let luma_fg = if white_bg { bg - l } else { l - bg } > LUMA_DELTA;
        let mx = i32::from(p.0.iter().copied().max().unwrap_or(0));
        let mn = i32::from(p.0.iter().copied().min().unwrap_or(0));
        let sat_fg = (mx - mn) > SAT_DELTA;
        mask[i] = luma_fg || sat_fg;
    }
    mask
}

// ---------------------------------------------------------------------------
// Connected components.
// ---------------------------------------------------------------------------

/// One connected foreground region.
#[derive(Clone, Debug)]
struct Blob {
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
    area: u32,
    /// Boundary pixels (fg pixels with a non-fg 4-neighbour) — enough for the
    /// convex hull / min-area rectangle.
    boundary: Vec<(i64, i64)>,
}

/// 8-connected flood-fill labelling. `mask` stays immutable; a separate visited
/// vec keeps the BFS stack bounded to one region at a time.
fn connected_components(mask: &[bool], w: u32, h: u32) -> Vec<Blob> {
    let idx = |x: u32, y: u32| (y * w + x) as usize;
    let mut visited = vec![false; mask.len()];
    let mut blobs = Vec::new();
    let mut stack: Vec<(u32, u32)> = Vec::new();

    for sy in 0..h {
        for sx in 0..w {
            if !mask[idx(sx, sy)] || visited[idx(sx, sy)] {
                continue;
            }
            visited[idx(sx, sy)] = true;
            stack.clear();
            stack.push((sx, sy));
            let mut blob = Blob {
                min_x: sx,
                min_y: sy,
                max_x: sx,
                max_y: sy,
                area: 0,
                boundary: Vec::new(),
            };
            while let Some((x, y)) = stack.pop() {
                blob.area += 1;
                blob.min_x = blob.min_x.min(x);
                blob.min_y = blob.min_y.min(y);
                blob.max_x = blob.max_x.max(x);
                blob.max_y = blob.max_y.max(y);

                // Boundary test on the 4-neighbourhood.
                let is_boundary = x == 0
                    || y == 0
                    || x == w - 1
                    || y == h - 1
                    || !mask[idx(x - 1, y)]
                    || !mask[idx(x + 1, y)]
                    || !mask[idx(x, y - 1)]
                    || !mask[idx(x, y + 1)];
                if is_boundary {
                    blob.boundary.push((i64::from(x), i64::from(y)));
                }

                // 8-connected expansion.
                let x0 = x.saturating_sub(1);
                let y0 = y.saturating_sub(1);
                let x1 = (x + 1).min(w - 1);
                let y1 = (y + 1).min(h - 1);
                for ny in y0..=y1 {
                    for nx in x0..=x1 {
                        let n = idx(nx, ny);
                        if mask[n] && !visited[n] {
                            visited[n] = true;
                            stack.push((nx, ny));
                        }
                    }
                }
            }
            blobs.push(blob);
        }
    }
    blobs
}

// ---------------------------------------------------------------------------
// Cropping + straightening.
// ---------------------------------------------------------------------------

fn axis_crop(img: &RgbImage, b: &Blob) -> RgbImage {
    let w = b.max_x - b.min_x + 1;
    let h = b.max_y - b.min_y + 1;
    image::imageops::crop_imm(img, b.min_x, b.min_y, w, h).to_image()
}

/// A rotated rectangle: centre, full width/height, and the angle of the width
/// axis (radians, normalised into (-π/4, π/4]).
struct RotRect {
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
    angle: f64,
}

/// Straighten one blob: fit its min-area rectangle, then resample an upright
/// crop of that rectangle from the source. Falls back to an axis crop when the
/// blob is too thin for a meaningful rectangle.
fn straighten_crop(img: &RgbImage, b: &Blob, fill: Rgb<u8>) -> RgbImage {
    let hull = convex_hull(b.boundary.clone());
    let rect = match min_area_rect(&hull) {
        Some(r) => normalize_rect(r),
        None => return axis_crop(img, b),
    };
    let ow = (rect.w.round() as i64).max(1) as u32;
    let oh = (rect.h.round() as i64).max(1) as u32;
    // Guard against a runaway allocation from a degenerate rectangle.
    if ow > img.width() * 2 || oh > img.height() * 2 {
        return axis_crop(img, b);
    }
    let (ca, sa) = (rect.angle.cos(), rect.angle.sin());
    // width axis u = (cos, sin); height axis v = (-sin, cos).
    let mut out = RgbImage::from_pixel(ow, oh, fill);
    let (iw, ih) = (img.width(), img.height());
    let (hw, hh) = (f64::from(ow) / 2.0, f64::from(oh) / 2.0);
    for oy in 0..oh {
        let dy = f64::from(oy) + 0.5 - hh;
        for ox in 0..ow {
            let dx = f64::from(ox) + 0.5 - hw;
            let sx = rect.cx + dx * ca - dy * sa;
            let sy = rect.cy + dx * sa + dy * ca;
            if sx < 0.0 || sy < 0.0 || sx > f64::from(iw - 1) || sy > f64::from(ih - 1) {
                continue; // stays `fill`
            }
            *out.get_pixel_mut(ox, oy) = bilinear(img, sx, sy);
        }
    }
    out
}

fn apply_trim(img: RgbImage, trim: u32, fill: Rgb<u8>) -> RgbImage {
    if trim == 0 {
        return img;
    }
    let (w, h) = (img.width(), img.height());
    if 2 * trim >= w || 2 * trim >= h {
        // Trim would erase the image — return a 1×1 matte instead of failing.
        return RgbImage::from_pixel(1, 1, fill);
    }
    image::imageops::crop_imm(&img, trim, trim, w - 2 * trim, h - 2 * trim).to_image()
}

fn bilinear(src: &RgbImage, x: f64, y: f64) -> Rgb<u8> {
    let x0 = x.floor().max(0.0) as u32;
    let y0 = y.floor().max(0.0) as u32;
    let x1 = (x0 + 1).min(src.width() - 1);
    let y1 = (y0 + 1).min(src.height() - 1);
    let fx = x - f64::from(x0);
    let fy = y - f64::from(y0);
    let mut o = [0u8; 3];
    for ch in 0..3 {
        let p00 = f64::from(src.get_pixel(x0, y0)[ch]);
        let p10 = f64::from(src.get_pixel(x1, y0)[ch]);
        let p01 = f64::from(src.get_pixel(x0, y1)[ch]);
        let p11 = f64::from(src.get_pixel(x1, y1)[ch]);
        let top = p00 + (p10 - p00) * fx;
        let bot = p01 + (p11 - p01) * fx;
        o[ch] = (top + (bot - top) * fy).round().clamp(0.0, 255.0) as u8;
    }
    Rgb(o)
}

// ---------------------------------------------------------------------------
// Geometry: convex hull + rotating-calipers min-area rectangle.
// ---------------------------------------------------------------------------

/// Andrew's monotone-chain convex hull (counter-clockwise, no collinear points).
fn convex_hull(mut pts: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    pts.sort();
    pts.dedup();
    let n = pts.len();
    if n < 3 {
        return pts;
    }
    let cross = |o: (i64, i64), a: (i64, i64), b: (i64, i64)| {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    };
    let mut hull: Vec<(i64, i64)> = Vec::with_capacity(2 * n);
    for &p in &pts {
        while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0 {
            hull.pop();
        }
        hull.push(p);
    }
    let lower = hull.len() + 1;
    for &p in pts.iter().rev() {
        while hull.len() >= lower && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0 {
            hull.pop();
        }
        hull.push(p);
    }
    hull.pop();
    hull
}

/// Minimum-area enclosing rectangle via rotating calipers over each hull edge.
fn min_area_rect(hull: &[(i64, i64)]) -> Option<RotRect> {
    let n = hull.len();
    if n < 3 {
        return None;
    }
    let mut best: Option<RotRect> = None;
    for i in 0..n {
        let (x1, y1) = hull[i];
        let (x2, y2) = hull[(i + 1) % n];
        let dx = (x2 - x1) as f64;
        let dy = (y2 - y1) as f64;
        let len = dx.hypot(dy);
        if len < 1e-9 {
            continue;
        }
        let (ux, uy) = (dx / len, dy / len); // edge direction
        let (vx, vy) = (-uy, ux); // perpendicular
        let (mut min_u, mut max_u, mut min_v, mut max_v) =
            (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for &(px, py) in hull {
            let (fx, fy) = (px as f64, py as f64);
            let pu = fx * ux + fy * uy;
            let pv = fx * vx + fy * vy;
            min_u = min_u.min(pu);
            max_u = max_u.max(pu);
            min_v = min_v.min(pv);
            max_v = max_v.max(pv);
        }
        let rw = max_u - min_u;
        let rh = max_v - min_v;
        let area = rw * rh;
        if best.as_ref().map_or(true, |bb| area < bb.w * bb.h) {
            let cu = (min_u + max_u) / 2.0;
            let cv = (min_v + max_v) / 2.0;
            best = Some(RotRect {
                cx: cu * ux + cv * vx,
                cy: cu * uy + cv * vy,
                w: rw,
                h: rh,
                angle: uy.atan2(ux),
            });
        }
    }
    best
}

/// Bring the rectangle's width-axis angle into (-π/4, π/4] (least rotation),
/// swapping w/h whenever we turn the axis by 90°, so the straightened crop keeps
/// the photo's natural orientation instead of tipping it on its side.
fn normalize_rect(mut r: RotRect) -> RotRect {
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_4};
    while r.angle > FRAC_PI_4 {
        r.angle -= FRAC_PI_2;
        std::mem::swap(&mut r.w, &mut r.h);
    }
    while r.angle <= -FRAC_PI_4 {
        r.angle += FRAC_PI_2;
        std::mem::swap(&mut r.w, &mut r.h);
    }
    r
}

// ---------------------------------------------------------------------------
// Encoding.
// ---------------------------------------------------------------------------

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

    /// Paint a solid-colour rectangle rotated by `deg` degrees, centred at
    /// (cx,cy) with the given (rect_w, rect_h), onto `canvas`.
    fn paint_rotated_rect(
        canvas: &mut RgbImage,
        cx: f64,
        cy: f64,
        rect_w: f64,
        rect_h: f64,
        deg: f64,
        color: [u8; 3],
    ) {
        let a = deg.to_radians();
        let (ca, sa) = (a.cos(), a.sin());
        let (w, h) = (canvas.width(), canvas.height());
        for y in 0..h {
            for x in 0..w {
                let dx = f64::from(x) - cx;
                let dy = f64::from(y) - cy;
                // rotate the sample point into the rect's local frame.
                let lx = dx * ca + dy * sa;
                let ly = -dx * sa + dy * ca;
                if lx.abs() <= rect_w / 2.0 && ly.abs() <= rect_h / 2.0 {
                    canvas.put_pixel(x, y, Rgb(color));
                }
            }
        }
    }

    fn unzip_names(zip: &[u8]) -> Vec<String> {
        let mut ar = zip::ZipArchive::new(Cursor::new(zip.to_vec())).unwrap();
        (0..ar.len()).map(|i| ar.by_index(i).unwrap().name().to_string()).collect()
    }

    fn unzip_image(zip: &[u8], i: usize) -> RgbImage {
        use std::io::Read;
        let mut ar = zip::ZipArchive::new(Cursor::new(zip.to_vec())).unwrap();
        let mut f = ar.by_index(i).unwrap();
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).unwrap();
        image::load_from_memory(&bytes).unwrap().to_rgb8()
    }

    #[test]
    fn splits_two_photos_on_white() {
        let mut scan = RgbImage::from_pixel(400, 300, Rgb([255, 255, 255]));
        paint_rotated_rect(&mut scan, 110.0, 90.0, 120.0, 80.0, 0.0, [220, 40, 40]);
        paint_rotated_rect(&mut scan, 300.0, 210.0, 100.0, 70.0, 0.0, [40, 60, 220]);
        let (zip, summary) = split_photos(&png_of(&scan), &SplitParams::default()).unwrap();
        assert_eq!(summary.photos, 2, "expected two photos, got sizes {:?}", summary.sizes);
        assert_eq!(summary.background, "white");
        let names = unzip_names(&zip);
        assert_eq!(names, vec!["photo_1.png", "photo_2.png"]);
    }

    #[test]
    fn crop_dims_match_and_is_solid() {
        // One axis-aligned red photo → the crop should be ~120x80 and mostly red.
        let mut scan = RgbImage::from_pixel(300, 260, Rgb([255, 255, 255]));
        paint_rotated_rect(&mut scan, 150.0, 130.0, 120.0, 80.0, 0.0, [210, 30, 30]);
        let (zip, _s) = split_photos(&png_of(&scan), &SplitParams::default()).unwrap();
        let out = unzip_image(&zip, 0);
        assert!(
            (out.width() as i64 - 120).abs() <= 4 && (out.height() as i64 - 80).abs() <= 4,
            "crop dims off: {}x{}",
            out.width(),
            out.height()
        );
        // Centre pixel is the photo red.
        let p = out.get_pixel(out.width() / 2, out.height() / 2).0;
        assert!(p[0] > 170 && p[1] < 90 && p[2] < 90, "centre not red: {p:?}");
    }

    #[test]
    fn straightens_a_rotated_photo() {
        // A green photo rotated 18° on white → after straightening the crop should
        // be nearly axis-aligned (its four corners are the photo colour, not the
        // white background matte).
        let mut scan = RgbImage::from_pixel(340, 320, Rgb([255, 255, 255]));
        paint_rotated_rect(&mut scan, 170.0, 160.0, 150.0, 100.0, 18.0, [30, 170, 60]);
        let (zip, s) = split_photos(&png_of(&scan), &SplitParams::default()).unwrap();
        assert_eq!(s.photos, 1);
        let out = unzip_image(&zip, 0);
        // Straightened dims ≈ the photo's 150x100 (order kept upright).
        let (ow, oh) = (out.width() as i64, out.height() as i64);
        assert!(
            (ow - 150).abs() <= 10 && (oh - 100).abs() <= 10,
            "straightened dims off: {ow}x{oh}"
        );
        // Sample a point 20% in from each corner — should be green, proving the
        // photo fills the frame (a mis-rotated crop would show white corners).
        let green = |p: [u8; 3]| p[1] > 130 && p[0] < 120 && p[2] < 120;
        let (w, h) = (out.width(), out.height());
        for &(fx, fy) in &[(0.2, 0.2), (0.8, 0.2), (0.2, 0.8), (0.8, 0.8)] {
            let x = (fx * f64::from(w)) as u32;
            let y = (fy * f64::from(h)) as u32;
            let p = out.get_pixel(x, y).0;
            assert!(green(p), "corner ({fx},{fy}) not green after straighten: {p:?}");
        }
    }

    #[test]
    fn straighten_off_keeps_the_skew_bbox() {
        // With straighten off, a rotated photo yields its axis-aligned bounding
        // box, which is LARGER than the photo and has white (background) corners.
        let mut scan = RgbImage::from_pixel(340, 320, Rgb([255, 255, 255]));
        paint_rotated_rect(&mut scan, 170.0, 160.0, 150.0, 100.0, 18.0, [30, 170, 60]);
        let params = SplitParams { straighten: false, ..Default::default() };
        let (zip, _s) = split_photos(&png_of(&scan), &params).unwrap();
        let out = unzip_image(&zip, 0);
        // A corner of the bbox is background white (the rotated photo doesn't reach it).
        let corner = out.get_pixel(1, 1).0;
        assert!(corner[0] > 200 && corner[1] > 200 && corner[2] > 200, "corner not bg: {corner:?}");
    }

    #[test]
    fn detects_photo_on_black_background() {
        let mut scan = RgbImage::from_pixel(300, 240, Rgb([0, 0, 0]));
        paint_rotated_rect(&mut scan, 150.0, 120.0, 120.0, 90.0, 0.0, [230, 230, 210]);
        let params = SplitParams { background: Background::Black, ..Default::default() };
        let (_zip, s) = split_photos(&png_of(&scan), &params).unwrap();
        assert_eq!(s.photos, 1);
        assert_eq!(s.background, "black");
    }

    #[test]
    fn min_size_filters_specks() {
        let mut scan = RgbImage::from_pixel(300, 240, Rgb([255, 255, 255]));
        paint_rotated_rect(&mut scan, 150.0, 120.0, 120.0, 80.0, 0.0, [200, 40, 40]);
        // A tiny 10px speck that should be filtered out.
        paint_rotated_rect(&mut scan, 20.0, 20.0, 10.0, 10.0, 0.0, [10, 10, 10]);
        let (_zip, s) = split_photos(&png_of(&scan), &SplitParams::default()).unwrap();
        assert_eq!(s.photos, 1, "speck should be filtered, sizes {:?}", s.sizes);
    }

    #[test]
    fn max_photos_caps_output() {
        let mut scan = RgbImage::from_pixel(500, 200, Rgb([255, 255, 255]));
        paint_rotated_rect(&mut scan, 90.0, 100.0, 100.0, 100.0, 0.0, [220, 40, 40]);
        paint_rotated_rect(&mut scan, 250.0, 100.0, 100.0, 100.0, 0.0, [40, 220, 40]);
        paint_rotated_rect(&mut scan, 410.0, 100.0, 100.0, 100.0, 0.0, [40, 40, 220]);
        let params = SplitParams { max_photos: Some(2), ..Default::default() };
        let (_zip, s) = split_photos(&png_of(&scan), &params).unwrap();
        assert_eq!(s.photos, 2);
    }

    #[test]
    fn jpeg_output_format() {
        let mut scan = RgbImage::from_pixel(300, 240, Rgb([255, 255, 255]));
        paint_rotated_rect(&mut scan, 150.0, 120.0, 120.0, 80.0, 0.0, [200, 40, 40]);
        let params = SplitParams { format: OutFormat::Jpeg, ..Default::default() };
        let (zip, _s) = split_photos(&png_of(&scan), &params).unwrap();
        let names = unzip_names(&zip);
        assert_eq!(names, vec!["photo_1.jpg"]);
        // Decodes back as a valid image.
        let out = unzip_image(&zip, 0);
        assert!(out.width() > 0);
    }

    #[test]
    fn edge_trim_shrinks_the_crop() {
        let mut scan = RgbImage::from_pixel(300, 260, Rgb([255, 255, 255]));
        paint_rotated_rect(&mut scan, 150.0, 130.0, 120.0, 80.0, 0.0, [210, 30, 30]);
        let base = split_photos(&png_of(&scan), &SplitParams::default()).unwrap();
        let base_dims = base.1.sizes[0];
        let trimmed = split_photos(
            &png_of(&scan),
            &SplitParams { edge_trim: 6, ..Default::default() },
        )
        .unwrap();
        let td = trimmed.1.sizes[0];
        assert_eq!(td.0, base_dims.0 - 12);
        assert_eq!(td.1, base_dims.1 - 12);
    }

    #[test]
    fn errors_on_blank_scan() {
        let scan = RgbImage::from_pixel(200, 150, Rgb([255, 255, 255]));
        assert!(split_photos(&png_of(&scan), &SplitParams::default()).is_err());
    }

    #[test]
    fn errors_on_undecodable_input() {
        assert!(split_photos(b"not an image", &SplitParams::default()).is_err());
    }

    #[test]
    fn parsers_reject_bad_values() {
        assert_eq!(Background::parse("black").unwrap(), Background::Black);
        assert!(Background::parse("grey").is_err());
        assert_eq!(OutFormat::parse("jpg").unwrap(), OutFormat::Jpeg);
        assert!(OutFormat::parse("tiff").is_err());
    }

    #[test]
    fn min_area_rect_of_a_square() {
        // Convex hull of a 10x10 axis square → rect 10x10, angle ~0.
        let pts = vec![(0, 0), (10, 0), (10, 10), (0, 10)];
        let hull = convex_hull(pts);
        let r = normalize_rect(min_area_rect(&hull).unwrap());
        assert!((r.w - 10.0).abs() < 1e-6 && (r.h - 10.0).abs() < 1e-6, "{}x{}", r.w, r.h);
        assert!(r.angle.abs() < 1e-6, "angle {}", r.angle);
    }
}
