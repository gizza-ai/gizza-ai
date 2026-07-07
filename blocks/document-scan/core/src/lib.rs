//! gizza-ai/document-scan core — detect a document's four corners in a photo and
//! perspective-correct (dewarp) it into a flat, cropped, tonally-cleaned scan.
//! Pure-Rust (`image`), no ML / no ffmpeg, so it runs on every backend incl. the
//! chat Service Worker.
//!
//! Pipeline per photo:
//!   decode → find the page quadrilateral (explicit `corners`, else an
//!   Otsu-brightness auto-detect that assumes the page is lighter than its
//!   surroundings and fully in frame) → solve the 4-point projective homography
//!   (output-rectangle → source quad) → inverse-map every output pixel with
//!   bilinear sampling → tonal enhancement (magic / grayscale / black-&-white /
//!   colour) → optional 90° rotation → optional white margin → PNG.
//!
//! It intentionally does NOT do OCR/searchable text, and its auto-detection is a
//! classical contrast-quadrilateral finder (not ML), so cluttered or low-contrast
//! scenes should pass explicit `corners`.

use std::io::Cursor;

use image::{ImageFormat, Rgb, RgbImage};

/// Cap the output (and warp working) dimension so a 12-MP phone photo becomes a
/// sensible scan instead of a huge PNG.
pub const MAX_DIM: u32 = 2600;

/// A source-pixel corner (x, y).
pub type Corner = (f64, f64);
/// The page quadrilateral, ordered top-left, top-right, bottom-right, bottom-left.
pub type Quad = [Corner; 4];

/// Tonal-enhancement mode applied to the flattened page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Whiten the paper and lift contrast while keeping colour — the everyday
    /// "office scan" look. Default.
    Magic,
    /// Perception-weighted grayscale (Rec. 601 luma).
    Grayscale,
    /// Otsu threshold → crisp pure black-on-white, for forms/contracts.
    BlackWhite,
    /// Keep the warped colours unchanged.
    Color,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s {
            "magic" => Ok(Mode::Magic),
            "grayscale" => Ok(Mode::Grayscale),
            "blackwhite" => Ok(Mode::BlackWhite),
            "color" => Ok(Mode::Color),
            other => Err(format!(
                "unknown mode `{other}` (expected magic, grayscale, blackwhite or color)"
            )),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Mode::Magic => "magic-colour",
            Mode::Grayscale => "grayscale",
            Mode::BlackWhite => "black-and-white",
            Mode::Color => "colour",
        }
    }
}

/// Output-rectangle proportion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    /// Use the page's own measured proportions (average of the quad's opposite
    /// edge lengths). Default.
    Auto,
    /// ISO A4 (210:297).
    A4,
    /// US Letter (8.5:11).
    Letter,
    /// A square.
    Square,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        match s {
            "auto" => Ok(Output::Auto),
            "a4" => Ok(Output::A4),
            "letter" => Ok(Output::Letter),
            "square" => Ok(Output::Square),
            other => {
                Err(format!("unknown output `{other}` (expected auto, a4, letter or square)"))
            }
        }
    }
}

/// All scan knobs (see the block descriptor for the LLM/CLI-facing docs).
#[derive(Clone, Debug)]
pub struct ScanOptions {
    /// The page corners in source pixels (TL, TR, BR, BL). `None` = auto-detect.
    pub corners: Option<Quad>,
    pub mode: Mode,
    pub output: Output,
    /// Clockwise rotation of the final scan; only 0/90/180/270 are honoured.
    pub rotate: u16,
    /// White border added around the scan, as a percent of the scan's larger
    /// side (0..=25).
    pub margin: f32,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            corners: None,
            mode: Mode::Magic,
            output: Output::Auto,
            rotate: 0,
            margin: 0.0,
        }
    }
}

/// Detect + perspective-correct the document in `image_bytes`, returning PNG bytes.
pub fn document_scan(image_bytes: &[u8], opts: &ScanOptions) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("could not decode image: {e}"))?
        .to_rgb8();
    let (iw, ih) = (img.width(), img.height());
    if iw < 2 || ih < 2 {
        return Err("image is too small to scan".into());
    }

    let quad = match opts.corners {
        Some(c) => validate_corners(c, iw, ih)?,
        None => detect_corners(&img)?,
    };

    let (ow, oh) = output_size(&quad, opts.output);
    let warped = warp_perspective(&img, &quad, ow, oh)?;
    let enhanced = enhance(warped, opts.mode);
    let rotated = rotate_quarter(enhanced, opts.rotate);
    let bordered = add_margin(rotated, opts.margin);

    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(bordered)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    Ok(out.into_inner())
}

// ---------------------------------------------------------------------------
// Corner acquisition.
// ---------------------------------------------------------------------------

/// Check user-supplied corners are finite, in-bounds and enclose a real area.
fn validate_corners(c: Quad, iw: u32, ih: u32) -> Result<Quad, String> {
    for (x, y) in c {
        if !x.is_finite() || !y.is_finite() {
            return Err("corners must be finite numbers".into());
        }
        if x < 0.0 || y < 0.0 || x > f64::from(iw) || y > f64::from(ih) {
            return Err(format!(
                "corner ({x:.0},{y:.0}) is outside the {iw}x{ih} image — give pixel \
                 coordinates within the photo"
            ));
        }
    }
    if quad_area(&c) < (f64::from(iw) * f64::from(ih)) * 0.005 {
        return Err("the four corners enclose almost no area — check their order \
                    (top-left, top-right, bottom-right, bottom-left)"
            .into());
    }
    Ok(c)
}

/// Shoelace area of a quad (absolute value).
fn quad_area(q: &Quad) -> f64 {
    let mut s = 0.0;
    for i in 0..4 {
        let (x0, y0) = q[i];
        let (x1, y1) = q[(i + 1) % 4];
        s += x0 * y1 - x1 * y0;
    }
    s.abs() / 2.0
}

/// Auto-detect the page quad by Otsu-thresholding a downscaled luma copy into a
/// "bright paper" mask, then taking the extreme corners of that mask. Works when
/// the page is lighter than its surroundings and fully in frame; errors clearly
/// otherwise so the caller passes explicit corners instead of getting garbage.
fn detect_corners(img: &RgbImage) -> Result<Quad, String> {
    let (iw, ih) = (img.width(), img.height());
    // Downscale the long side to ~600 px for speed + noise immunity.
    let scale = (600.0 / f64::from(iw.max(ih))).min(1.0);
    let sw = ((f64::from(iw) * scale).round() as u32).max(1);
    let sh = ((f64::from(ih) * scale).round() as u32).max(1);
    let small = image::imageops::resize(img, sw, sh, image::imageops::FilterType::Triangle);

    // Luma + 256-bin histogram.
    let mut luma = vec![0u8; (sw * sh) as usize];
    let mut hist = [0u32; 256];
    for (i, p) in small.pixels().enumerate() {
        let l = luma601(p.0[0], p.0[1], p.0[2]);
        luma[i] = l;
        hist[l as usize] += 1;
    }
    let total = (sw * sh) as f64;
    let thr = otsu_threshold(&hist, total as u32);

    // Bright-pixel extremes (rotated-bounding-box corners of the paper blob).
    let mut tl = (f64::MAX, (0.0, 0.0));
    let mut br = (f64::MIN, (0.0, 0.0));
    let mut tr = (f64::MIN, (0.0, 0.0));
    let mut bl = (f64::MAX, (0.0, 0.0));
    let mut bright = 0u32;
    for y in 0..sh {
        for x in 0..sw {
            if u32::from(luma[(y * sw + x) as usize]) <= u32::from(thr) {
                continue;
            }
            bright += 1;
            let (fx, fy) = (f64::from(x), f64::from(y));
            let sum = fx + fy;
            let dif = fx - fy;
            if sum < tl.0 {
                tl = (sum, (fx, fy));
            }
            if sum > br.0 {
                br = (sum, (fx, fy));
            }
            if dif > tr.0 {
                tr = (dif, (fx, fy));
            }
            if dif < bl.0 {
                bl = (dif, (fx, fy));
            }
        }
    }

    let frac = f64::from(bright) / total;
    if frac < 0.03 {
        return Err("could not auto-detect the document — no bright page region was \
                    found. Photograph the page against a darker background, or pass \
                    explicit corners (x0,y0,...,x3,y3)."
            .into());
    }
    if frac > 0.985 {
        return Err("could not auto-detect the document edges — the bright region \
                    fills the whole frame, so there is no visible page border to \
                    crop to. Pass explicit corners (x0,y0,...,x3,y3) instead."
            .into());
    }

    let inv = 1.0 / scale;
    let up = |(x, y): (f64, f64)| -> Corner {
        (
            (x * inv).clamp(0.0, f64::from(iw)),
            (y * inv).clamp(0.0, f64::from(ih)),
        )
    };
    let quad = [up(tl.1), up(tr.1), up(br.1), up(bl.1)];

    if quad_area(&quad) < (f64::from(iw) * f64::from(ih)) * 0.03 {
        return Err("could not auto-detect a confident document quadrilateral — the \
                    detected page is too small or too thin. Pass explicit corners \
                    (x0,y0,...,x3,y3)."
            .into());
    }
    Ok(quad)
}

/// Otsu's between-class-variance threshold on a 256-bin histogram; returns the
/// grey level `t` such that pixels with luma > t are "foreground/bright".
fn otsu_threshold(hist: &[u32; 256], total: u32) -> u8 {
    let total = f64::from(total);
    let sum: f64 = (0..256).map(|i| i as f64 * f64::from(hist[i])).sum();
    let mut sum_b = 0.0;
    let mut w_b = 0.0;
    let mut max_between = -1.0;
    let mut threshold = 127u8;
    for i in 0..256 {
        w_b += f64::from(hist[i]);
        if w_b == 0.0 {
            continue;
        }
        let w_f = total - w_b;
        if w_f == 0.0 {
            break;
        }
        sum_b += i as f64 * f64::from(hist[i]);
        let m_b = sum_b / w_b;
        let m_f = (sum - sum_b) / w_f;
        let between = w_b * w_f * (m_b - m_f) * (m_b - m_f);
        if between > max_between {
            max_between = between;
            threshold = i as u8;
        }
    }
    threshold
}

fn luma601(r: u8, g: u8, b: u8) -> u8 {
    (0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b)).round() as u8
}

// ---------------------------------------------------------------------------
// Output geometry.
// ---------------------------------------------------------------------------

fn dist(a: Corner, b: Corner) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// Pick the output rectangle (w, h) from the quad's measured proportions and the
/// requested `Output`, capped to `MAX_DIM`.
fn output_size(q: &Quad, out: Output) -> (u32, u32) {
    let [tl, tr, br, bl] = *q;
    // Measured page width (top & bottom edges) and height (left & right edges).
    let mw = ((dist(tl, tr) + dist(bl, br)) / 2.0).max(1.0);
    let mh = ((dist(tl, bl) + dist(tr, br)) / 2.0).max(1.0);

    let (w, h) = match out {
        Output::Auto => (mw, mh),
        Output::Square => {
            let s = mw.max(mh);
            (s, s)
        }
        Output::A4 | Output::Letter => {
            let ratio = if matches!(out, Output::A4) {
                210.0 / 297.0
            } else {
                8.5 / 11.0
            };
            if mw >= mh {
                // landscape: long side = width
                (mw, mw * ratio)
            } else {
                // portrait: long side = height
                (mh * ratio, mh)
            }
        }
    };

    // Cap the larger side to MAX_DIM, scaling both.
    let longer = w.max(h);
    let k = if longer > f64::from(MAX_DIM) {
        f64::from(MAX_DIM) / longer
    } else {
        1.0
    };
    (
        ((w * k).round() as u32).max(1),
        ((h * k).round() as u32).max(1),
    )
}

// ---------------------------------------------------------------------------
// Perspective warp.
// ---------------------------------------------------------------------------

/// Solve the 8-DOF projective homography mapping the four `from` points to the
/// four `to` points: (x,y) = ((a u + b v + c)/(g u + h v + 1),
/// (d u + e v + f)/(g u + h v + 1)). Returns `[a..h]` (8 coefficients) or `None`
/// if the system is singular.
fn homography(from: &Quad, to: &Quad) -> Option<[f64; 8]> {
    // 8 equations, 8 unknowns. Rows fill an augmented [8 x 9] matrix.
    let mut m = [[0.0f64; 9]; 8];
    for i in 0..4 {
        let (u, v) = from[i];
        let (x, y) = to[i];
        // x equation
        m[2 * i] = [u, v, 1.0, 0.0, 0.0, 0.0, -u * x, -v * x, x];
        // y equation
        m[2 * i + 1] = [0.0, 0.0, 0.0, u, v, 1.0, -u * y, -v * y, y];
    }
    gaussian_solve8(&mut m)
}

/// Gauss-Jordan elimination with partial pivoting on an [8 x 9] augmented matrix.
fn gaussian_solve8(m: &mut [[f64; 9]; 8]) -> Option<[f64; 8]> {
    for col in 0..8 {
        // Partial pivot: largest magnitude in this column at/under the diagonal.
        let mut piv = col;
        for r in (col + 1)..8 {
            if m[r][col].abs() > m[piv][col].abs() {
                piv = r;
            }
        }
        if m[piv][col].abs() < 1e-12 {
            return None; // singular
        }
        m.swap(col, piv);
        let d = m[col][col];
        for k in col..9 {
            m[col][k] /= d;
        }
        for r in 0..8 {
            if r == col {
                continue;
            }
            let f = m[r][col];
            if f != 0.0 {
                for k in col..9 {
                    m[r][k] -= f * m[col][k];
                }
            }
        }
    }
    let mut sol = [0.0f64; 8];
    for (i, s) in sol.iter_mut().enumerate() {
        *s = m[i][8];
    }
    Some(sol)
}

/// Warp the source `img` so the `quad` (TL,TR,BR,BL) fills a flat `ow`×`oh`
/// rectangle, using inverse mapping + bilinear sampling.
fn warp_perspective(img: &RgbImage, quad: &Quad, ow: u32, oh: u32) -> Result<RgbImage, String> {
    // Map every OUTPUT pixel back to the SOURCE quad, so `from` = output corners,
    // `to` = source quad corners.
    let out_corners: Quad = [
        (0.0, 0.0),
        (f64::from(ow - 1), 0.0),
        (f64::from(ow - 1), f64::from(oh - 1)),
        (0.0, f64::from(oh - 1)),
    ];
    let h = homography(&out_corners, quad)
        .ok_or("the four corners are degenerate (collinear) — cannot compute the warp")?;
    let [a, b, c, d, e, f, g, hh] = h;

    let mut out = RgbImage::from_pixel(ow, oh, Rgb([255, 255, 255]));
    let (iw, ih) = (img.width(), img.height());
    for oy in 0..oh {
        let vy = f64::from(oy);
        for ox in 0..ow {
            let vx = f64::from(ox);
            let den = g * vx + hh * vy + 1.0;
            if den.abs() < 1e-9 {
                continue; // stays white
            }
            let sx = (a * vx + b * vy + c) / den;
            let sy = (d * vx + e * vy + f) / den;
            if sx < 0.0 || sy < 0.0 || sx > f64::from(iw - 1) || sy > f64::from(ih - 1) {
                continue; // outside source → white
            }
            *out.get_pixel_mut(ox, oy) = bilinear(img, sx, sy);
        }
    }
    Ok(out)
}

fn bilinear(src: &RgbImage, x: f64, y: f64) -> Rgb<u8> {
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
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
// Enhancement, rotation, margin.
// ---------------------------------------------------------------------------

fn enhance(img: RgbImage, mode: Mode) -> RgbImage {
    match mode {
        Mode::Color => img,
        Mode::Grayscale => map_pixels(img, |[r, g, b]| {
            let l = luma601(r, g, b);
            [l, l, l]
        }),
        Mode::BlackWhite => {
            let mut hist = [0u32; 256];
            for p in img.pixels() {
                hist[luma601(p.0[0], p.0[1], p.0[2]) as usize] += 1;
            }
            let thr = otsu_threshold(&hist, img.width() * img.height());
            map_pixels(img, |[r, g, b]| {
                let v = if luma601(r, g, b) > thr { 255 } else { 0 };
                [v, v, v]
            })
        }
        Mode::Magic => magic(img),
    }
}

/// Whiten the paper (stretch so a high luma percentile maps to white) and lift
/// contrast a touch, keeping colour.
fn magic(img: RgbImage) -> RgbImage {
    let mut hist = [0u32; 256];
    for p in img.pixels() {
        hist[luma601(p.0[0], p.0[1], p.0[2]) as usize] += 1;
    }
    let total = img.width() * img.height();
    // 92nd-percentile luma = the paper white point.
    let target = (f64::from(total) * 0.92) as u32;
    let mut acc = 0u32;
    let mut wp = 255u32;
    for (i, &h) in hist.iter().enumerate() {
        acc += h;
        if acc >= target {
            wp = i as u32;
            break;
        }
    }
    let wp = wp.max(1);
    let gain = (255.0 / f64::from(wp)).clamp(1.0, 3.0);
    map_pixels(img, |[r, g, b]| {
        let stretch = |v: u8| {
            let s = f64::from(v) * gain;
            // Mild contrast around mid-grey.
            let c = 128.0 + (s - 128.0) * 1.12;
            c.round().clamp(0.0, 255.0) as u8
        };
        [stretch(r), stretch(g), stretch(b)]
    })
}

fn map_pixels(mut img: RgbImage, f: impl Fn([u8; 3]) -> [u8; 3]) -> RgbImage {
    for p in img.pixels_mut() {
        p.0 = f(p.0);
    }
    img
}

fn rotate_quarter(img: RgbImage, deg: u16) -> RgbImage {
    match deg {
        90 => image::imageops::rotate90(&img),
        180 => image::imageops::rotate180(&img),
        270 => image::imageops::rotate270(&img),
        _ => img,
    }
}

fn add_margin(img: RgbImage, percent: f32) -> RgbImage {
    if percent <= 0.0 {
        return img;
    }
    let (w, h) = (img.width(), img.height());
    let m = ((f64::from(w.max(h)) * f64::from(percent) / 100.0).round() as u32).min(MAX_DIM);
    if m == 0 {
        return img;
    }
    let mut out = RgbImage::from_pixel(w + 2 * m, h + 2 * m, Rgb([255, 255, 255]));
    image::imageops::overlay(&mut out, &img, i64::from(m), i64::from(m));
    out
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

    /// A photo where a solid-colour page sits, perspective-skewed, on a dark
    /// background. `paint(nx, ny)` colours the page by its normalised (0..1)
    /// position so warp orientation can be checked.
    fn skewed_page(
        w: u32,
        h: u32,
        quad: &Quad,
        bg: [u8; 3],
        paint: impl Fn(f64, f64) -> [u8; 3],
    ) -> RgbImage {
        // homography(quad -> unit square) maps photo(x,y) -> page(nx,ny).
        let unit: Quad = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let hpar = homography(quad, &unit).unwrap();
        let [a, b, c, d, e, f, g, hh] = hpar;
        let mut img = RgbImage::from_pixel(w, h, Rgb(bg));
        for y in 0..h {
            for x in 0..w {
                let (fx, fy) = (f64::from(x), f64::from(y));
                let den = g * fx + hh * fy + 1.0;
                let nx = (a * fx + b * fy + c) / den;
                let ny = (d * fx + e * fy + f) / den;
                if (0.0..=1.0).contains(&nx) && (0.0..=1.0).contains(&ny) {
                    img.put_pixel(x, y, Rgb(paint(nx, ny)));
                }
            }
        }
        img
    }

    #[test]
    fn homography_round_trips_corners() {
        let out: Quad = [(0.0, 0.0), (99.0, 0.0), (99.0, 149.0), (0.0, 149.0)];
        let quad: Quad = [(12.0, 20.0), (180.0, 5.0), (195.0, 210.0), (5.0, 190.0)];
        let h = homography(&out, &quad).unwrap();
        let [a, b, c, d, e, f, g, hh] = h;
        for i in 0..4 {
            let (u, v) = out[i];
            let den = g * u + hh * v + 1.0;
            let x = (a * u + b * v + c) / den;
            let y = (d * u + e * v + f) / den;
            assert!((x - quad[i].0).abs() < 1e-6, "corner {i} x");
            assert!((y - quad[i].1).abs() < 1e-6, "corner {i} y");
        }
    }

    #[test]
    fn manual_corners_flatten_a_skewed_page() {
        // A red page skewed inside a black photo.
        let quad: Quad = [(30.0, 25.0), (260.0, 40.0), (250.0, 220.0), (20.0, 200.0)];
        let img = skewed_page(300, 240, &quad, [0, 0, 0], |_, _| [220, 30, 30]);
        let opts = ScanOptions {
            corners: Some(quad),
            mode: Mode::Color,
            output: Output::Auto,
            ..Default::default()
        };
        let png = document_scan(&png_of(&img), &opts).unwrap();
        let out = image::load_from_memory(&png).unwrap().to_rgb8();
        // Sample the centre — must be the page red, not the black background.
        let (cw, ch) = (out.width() / 2, out.height() / 2);
        let p = out.get_pixel(cw, ch).0;
        assert!(p[0] > 180 && p[1] < 80 && p[2] < 80, "centre should be page red, got {p:?}");
        // PNG magic.
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    }

    #[test]
    fn warp_preserves_left_right_orientation() {
        // Left half of the page green, right half blue → no mirroring after warp.
        let quad: Quad = [(30.0, 25.0), (260.0, 40.0), (250.0, 220.0), (20.0, 200.0)];
        let img = skewed_page(300, 240, &quad, [0, 0, 0], |nx, _| {
            if nx < 0.5 {
                [20, 200, 20]
            } else {
                [20, 20, 200]
            }
        });
        let opts = ScanOptions {
            corners: Some(quad),
            mode: Mode::Color,
            output: Output::Square,
            ..Default::default()
        };
        let png = document_scan(&png_of(&img), &opts).unwrap();
        let out = image::load_from_memory(&png).unwrap().to_rgb8();
        let mid = out.height() / 2;
        let left = out.get_pixel(out.width() / 4, mid).0;
        let right = out.get_pixel(out.width() * 3 / 4, mid).0;
        assert!(left[1] > 150 && left[2] < 90, "left quarter should be green, got {left:?}");
        assert!(right[2] > 150 && right[1] < 90, "right quarter should be blue, got {right:?}");
    }

    #[test]
    fn auto_detect_finds_a_bright_page() {
        // A near-white page skewed on a dark grey background — the auto path.
        let quad: Quad = [(40.0, 30.0), (250.0, 50.0), (240.0, 210.0), (30.0, 190.0)];
        let img = skewed_page(300, 240, &quad, [25, 25, 25], |_, _| [235, 235, 235]);
        let opts = ScanOptions { corners: None, mode: Mode::Color, ..Default::default() };
        let png = document_scan(&png_of(&img), &opts).unwrap();
        let out = image::load_from_memory(&png).unwrap().to_rgb8();
        // The flattened result should be predominantly the bright page.
        let mut bright = 0u32;
        for p in out.pixels() {
            if p.0[0] > 180 {
                bright += 1;
            }
        }
        let frac = f64::from(bright) / f64::from(out.width() * out.height());
        assert!(frac > 0.7, "auto-detected scan should be mostly page, bright frac = {frac:.2}");
    }

    #[test]
    fn detect_corners_are_close_to_truth() {
        let quad: Quad = [(40.0, 30.0), (250.0, 50.0), (240.0, 210.0), (30.0, 190.0)];
        let img = skewed_page(300, 240, &quad, [20, 20, 20], |_, _| [240, 240, 240]);
        let found = detect_corners(&img).unwrap();
        for i in 0..4 {
            assert!(
                dist(found[i], quad[i]) < 12.0,
                "corner {i} off: found {:?} vs {:?}",
                found[i],
                quad[i]
            );
        }
    }

    #[test]
    fn auto_detect_errors_on_a_flat_scene() {
        // Uniform mid-grey — no page/background contrast to latch onto.
        let img = RgbImage::from_pixel(120, 90, Rgb([128, 128, 128]));
        let opts = ScanOptions::default();
        assert!(document_scan(&png_of(&img), &opts).is_err());
    }

    #[test]
    fn grayscale_and_blackwhite_modes() {
        let quad: Quad = [(20.0, 20.0), (180.0, 20.0), (180.0, 140.0), (20.0, 140.0)];
        let img = skewed_page(200, 160, &quad, [0, 0, 0], |nx, _| {
            // A colour gradient across the page.
            [(nx * 255.0) as u8, 40, 200]
        });
        let src = png_of(&img);
        // Grayscale: r == g == b everywhere.
        let g = document_scan(
            &src,
            &ScanOptions { corners: Some(quad), mode: Mode::Grayscale, ..Default::default() },
        )
        .unwrap();
        let go = image::load_from_memory(&g).unwrap().to_rgb8();
        let p = go.get_pixel(go.width() / 2, go.height() / 2).0;
        assert!(p[0] == p[1] && p[1] == p[2], "grayscale pixel not neutral: {p:?}");
        // Black & white: every pixel is pure 0 or 255.
        let bw = document_scan(
            &src,
            &ScanOptions { corners: Some(quad), mode: Mode::BlackWhite, ..Default::default() },
        )
        .unwrap();
        let bwo = image::load_from_memory(&bw).unwrap().to_rgb8();
        for p in bwo.pixels() {
            assert!(p.0[0] == 0 || p.0[0] == 255, "bw pixel not binarised: {:?}", p.0);
        }
    }

    #[test]
    fn magic_whitens_a_dull_page() {
        // A uniform dull-grey (150) page → magic should brighten it toward white.
        let quad: Quad = [(20.0, 20.0), (180.0, 20.0), (180.0, 140.0), (20.0, 140.0)];
        let img = skewed_page(200, 160, &quad, [0, 0, 0], |_, _| [150, 150, 150]);
        let m = document_scan(
            &png_of(&img),
            &ScanOptions { corners: Some(quad), mode: Mode::Magic, ..Default::default() },
        )
        .unwrap();
        let mo = image::load_from_memory(&m).unwrap().to_rgb8();
        let p = mo.get_pixel(mo.width() / 2, mo.height() / 2).0;
        assert!(p[0] > 170, "magic should whiten the dull page, got {p:?}");
    }

    #[test]
    fn output_size_respects_page_size_and_cap() {
        // A wide-ish quad; A4 landscape → w:h ≈ 297:210.
        let quad: Quad = [(0.0, 0.0), (400.0, 0.0), (400.0, 200.0), (0.0, 200.0)];
        let (w, h) = output_size(&quad, Output::A4);
        let ratio = f64::from(w) / f64::from(h);
        assert!((ratio - 297.0 / 210.0).abs() < 0.05, "A4 landscape ratio off: {ratio}");
        // Square is square.
        let (sw, sh) = output_size(&quad, Output::Square);
        assert_eq!(sw, sh);
        // Cap: a huge quad is clamped to MAX_DIM.
        let big: Quad = [(0.0, 0.0), (9000.0, 0.0), (9000.0, 6000.0), (0.0, 6000.0)];
        let (bw, bh) = output_size(&big, Output::Auto);
        assert!(bw.max(bh) == MAX_DIM, "not capped: {bw}x{bh}");
    }

    #[test]
    fn rotate_swaps_dimensions() {
        let quad: Quad = [(0.0, 0.0), (200.0, 0.0), (200.0, 100.0), (0.0, 100.0)];
        let img = skewed_page(220, 120, &quad, [0, 0, 0], |_, _| [200, 200, 200]);
        let base = ScanOptions {
            corners: Some(quad),
            mode: Mode::Color,
            output: Output::Auto,
            ..Default::default()
        };
        let a = image::load_from_memory(&document_scan(&png_of(&img), &base).unwrap())
            .unwrap()
            .to_rgb8();
        let r = image::load_from_memory(
            &document_scan(&png_of(&img), &ScanOptions { rotate: 90, ..base.clone() }).unwrap(),
        )
        .unwrap()
        .to_rgb8();
        assert_eq!(a.width(), r.height());
        assert_eq!(a.height(), r.width());
    }

    #[test]
    fn margin_adds_a_white_border() {
        let quad: Quad = [(0.0, 0.0), (200.0, 0.0), (200.0, 100.0), (0.0, 100.0)];
        let img = skewed_page(220, 120, &quad, [0, 0, 0], |_, _| [10, 10, 10]);
        let opts = ScanOptions {
            corners: Some(quad),
            mode: Mode::Color,
            output: Output::Auto,
            margin: 10.0,
            ..Default::default()
        };
        let out = image::load_from_memory(&document_scan(&png_of(&img), &opts).unwrap())
            .unwrap()
            .to_rgb8();
        // Top-left pixel is in the added border → white.
        assert_eq!(out.get_pixel(0, 0).0, [255, 255, 255]);
    }

    #[test]
    fn rejects_bad_corners() {
        let img = RgbImage::from_pixel(100, 100, Rgb([200, 200, 200]));
        // Out of bounds.
        let opts = ScanOptions {
            corners: Some([(0.0, 0.0), (500.0, 0.0), (100.0, 100.0), (0.0, 100.0)]),
            ..Default::default()
        };
        assert!(document_scan(&png_of(&img), &opts).is_err());
        // Degenerate (all corners identical).
        let opts2 = ScanOptions {
            corners: Some([(5.0, 5.0), (5.0, 5.0), (5.0, 5.0), (5.0, 5.0)]),
            ..Default::default()
        };
        assert!(document_scan(&png_of(&img), &opts2).is_err());
    }

    #[test]
    fn rejects_undecodable_input() {
        assert!(document_scan(b"not an image", &ScanOptions::default()).is_err());
    }

    #[test]
    fn mode_and_output_parse() {
        assert_eq!(Mode::parse("blackwhite").unwrap(), Mode::BlackWhite);
        assert!(Mode::parse("sepia").is_err());
        assert_eq!(Output::parse("a4").unwrap(), Output::A4);
        assert!(Output::parse("a3").is_err());
    }
}
