//! gizza-ai/scan-to-pdf core — turn one or more phone photos of documents into a
//! cleaned, deskewed, high-contrast multi-page PDF scan. Pure-Rust (`image` +
//! `lopdf` + `flate2`), no ffmpeg / no ML, so it runs on every backend.
//!
//! Per photo the pipeline is: decode → optional 90° rotate → optional
//! auto-deskew (projection-profile skew detection + bilinear rotation) →
//! enhancement mode (magic-color / grayscale / adaptive-threshold B&W / color)
//! → embed as one PDF page (DeviceGray for grayscale+B&W, DeviceRGB otherwise).
//! Pages are assembled in input order into a single PDF with lopdf.

use std::io::Write;

use flate2::{write::ZlibEncoder, Compression};
use image::{imageops, GrayImage, RgbImage};
use lopdf::{dictionary, Document, Object, Stream};

/// Cap the working (and embedded) dimension so a 12-MP phone photo becomes a
/// sensible ~200-300 DPI scan instead of a 30-MB PDF page.
const MAX_DIM: u32 = 2600;

/// Enhancement mode applied to each page (mirrors the mobile-scanner filters).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Whiten the paper, lift contrast and saturation — the everyday "office
    /// scan" look while keeping colour. Default.
    Magic,
    /// Perception-weighted grayscale (Rec. 601 luma), tuned by contrast.
    Grayscale,
    /// Adaptive (local-mean) threshold → crisp pure black-on-white, despeckled.
    BlackWhite,
    /// Keep colour; only apply brightness + contrast.
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

    /// Human label for the response summary.
    pub fn label(&self) -> &'static str {
        match self {
            Mode::Magic => "magic-colour",
            Mode::Grayscale => "grayscale",
            Mode::BlackWhite => "black-and-white",
            Mode::Color => "colour",
        }
    }
}

/// Output page geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageSize {
    /// One PDF page per image, sized exactly to that image (1 px = 1 pt).
    Fit,
    /// A4 portrait (595×842 pt); the image is scaled to fit and centred.
    A4,
    /// US Letter portrait (612×792 pt); the image is scaled to fit and centred.
    Letter,
}

impl PageSize {
    pub fn parse(s: &str) -> Result<PageSize, String> {
        match s {
            "fit" => Ok(PageSize::Fit),
            "a4" => Ok(PageSize::A4),
            "letter" => Ok(PageSize::Letter),
            other => Err(format!("unknown page_size `{other}` (expected fit, a4 or letter)")),
        }
    }
}

/// All scan knobs (see the block descriptor for the LLM/CLI-facing docs).
#[derive(Clone, Copy, Debug)]
pub struct ScanOptions {
    pub mode: Mode,
    /// Auto-straighten a small tilt (projection-profile deskew).
    pub deskew: bool,
    /// Manual pre-rotation in degrees; only 0/90/180/270 are honoured.
    pub rotate: u16,
    /// Contrast multiplier around mid-grey (1.0 = none). Applied in magic /
    /// grayscale / color modes.
    pub contrast: f32,
    /// Brightness offset in [-100, 100]; also biases the B&W threshold.
    pub brightness: f32,
    pub page_size: PageSize,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            mode: Mode::Magic,
            deskew: true,
            rotate: 0,
            contrast: 1.0,
            brightness: 0.0,
            page_size: PageSize::Fit,
        }
    }
}

/// One embedded page: raw samples + geometry + colour space.
struct PageBuf {
    raw: Vec<u8>,
    w: u32,
    h: u32,
    gray: bool,
}

/// Build a single multi-page PDF from the given photo byte buffers, in order.
pub fn scan_to_pdf(images: &[Vec<u8>], opts: &ScanOptions) -> Result<Vec<u8>, String> {
    if images.is_empty() {
        return Err("need at least one image".into());
    }
    let mut pages = Vec::with_capacity(images.len());
    for (i, bytes) in images.iter().enumerate() {
        let rgb = image::load_from_memory(bytes)
            .map_err(|e| format!("image #{} failed to decode: {e}", i + 1))?
            .to_rgb8();
        pages.push(process_one(rgb, opts));
    }
    assemble_pdf(&pages, opts.page_size)
}

/// Run the full per-image pipeline and return an embeddable page buffer.
fn process_one(rgb: RgbImage, opts: &ScanOptions) -> PageBuf {
    let rgb = downscale(rgb, MAX_DIM);
    let rgb = rotate_quarter(rgb, opts.rotate);
    let rgb = if opts.deskew { straighten(&rgb) } else { rgb };
    enhance(rgb, opts)
}

/// Downscale so the longest side is at most `max`, preserving aspect. Small
/// images pass through untouched.
fn downscale(img: RgbImage, max: u32) -> RgbImage {
    let (w, h) = (img.width(), img.height());
    if w <= max && h <= max {
        return img;
    }
    let scale = f64::from(max) / f64::from(w.max(h));
    let nw = ((f64::from(w) * scale).round() as u32).max(1);
    let nh = ((f64::from(h) * scale).round() as u32).max(1);
    imageops::resize(&img, nw, nh, imageops::FilterType::Triangle)
}

/// Manual 90°-step rotation for phone orientation. Non-multiples of 90 are
/// treated as 0 (auto-deskew handles the small tilt).
fn rotate_quarter(img: RgbImage, deg: u16) -> RgbImage {
    match deg % 360 {
        90 => imageops::rotate90(&img),
        180 => imageops::rotate180(&img),
        270 => imageops::rotate270(&img),
        _ => img,
    }
}

// ---------------------------------------------------------------------------
// Auto-deskew: projection-profile skew detection + bilinear rotation.
// ---------------------------------------------------------------------------

const SKEW_LIMIT_DEG: f32 = 12.0;
const SKEW_STEP_DEG: f32 = 0.3;

/// Straighten a small tilt by detecting the skew angle and rotating to undo it.
/// A tilt under ~0.25° is left alone (no visible skew, avoid needless resampling).
fn straighten(rgb: &RgbImage) -> RgbImage {
    let luma = to_luma(rgb);
    let angle = detect_skew_deg(&luma);
    if angle.abs() < 0.25 {
        return rgb.clone();
    }
    rotate_rgb(rgb, angle.to_radians())
}

/// Rec. 601 luma of an RGB image.
fn to_luma(rgb: &RgbImage) -> GrayImage {
    let mut out = GrayImage::new(rgb.width(), rgb.height());
    for (o, p) in out.pixels_mut().zip(rgb.pixels()) {
        o[0] = luma601(p[0], p[1], p[2]);
    }
    out
}

fn luma601(r: u8, g: u8, b: u8) -> u8 {
    (0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b)).round() as u8
}

/// Detect the skew angle (degrees) whose correction rotation best aligns the
/// document's ink into horizontal rows. The returned angle can be passed
/// straight to `rotate_rgb` (in radians) to straighten the page.
fn detect_skew_deg(luma: &GrayImage) -> f32 {
    // Downscale the detection input for speed; skew is a global property.
    let small = downscale_luma(luma, 700);
    let (w, h) = (small.width(), small.height());
    if w < 8 || h < 8 {
        return 0.0;
    }
    // Ink = pixels darker than 90% of the mean brightness.
    let mean: f32 =
        small.pixels().map(|p| f32::from(p[0])).sum::<f32>() / (w as f32 * h as f32);
    let thr = mean * 0.9;
    let cx = (w - 1) as f32 / 2.0;
    let cy = (h - 1) as f32 / 2.0;
    let mut ink: Vec<(f32, f32)> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if f32::from(small.get_pixel(x, y)[0]) < thr {
                ink.push((x as f32 - cx, y as f32 - cy));
            }
        }
    }
    // Too little ink (blank / photo) → nothing to align.
    if ink.len() < (w as usize) {
        return 0.0;
    }

    let nbins = (w + h) as usize + 2;
    let offset = (w + h) as f32 / 2.0;
    let mut best_angle = 0.0_f32;
    let mut best_score = -1.0_f64;
    let steps = (2.0 * SKEW_LIMIT_DEG / SKEW_STEP_DEG).round() as i32;
    for k in 0..=steps {
        let deg = -SKEW_LIMIT_DEG + k as f32 * SKEW_STEP_DEG;
        let (s, c) = deg.to_radians().sin_cos();
        let mut hist = vec![0u32; nbins];
        for &(dx, dy) in &ink {
            let ry = dy * c + dx * s + offset;
            let bin = ry.round() as i32;
            if bin >= 0 && (bin as usize) < nbins {
                hist[bin as usize] += 1;
            }
        }
        // Concentrated rows (aligned text lines) maximise the sum of squares.
        let score: f64 = hist.iter().map(|&v| (v as f64) * (v as f64)).sum();
        if score > best_score {
            best_score = score;
            best_angle = deg;
        }
    }
    best_angle
}

fn downscale_luma(img: &GrayImage, max: u32) -> GrayImage {
    let (w, h) = (img.width(), img.height());
    if w <= max && h <= max {
        return img.clone();
    }
    let scale = f64::from(max) / f64::from(w.max(h));
    let nw = ((f64::from(w) * scale).round() as u32).max(1);
    let nh = ((f64::from(h) * scale).round() as u32).max(1);
    imageops::resize(img, nw, nh, imageops::FilterType::Triangle)
}

/// Rotate the image content by `angle` (radians) about its centre, keeping the
/// canvas size and filling exposed corners with white (paper). Bilinear sample.
fn rotate_rgb(src: &RgbImage, angle: f32) -> RgbImage {
    let (w, h) = (src.width(), src.height());
    let cx = (w - 1) as f32 / 2.0;
    let cy = (h - 1) as f32 / 2.0;
    let (s, c) = angle.sin_cos();
    let mut out = RgbImage::from_pixel(w, h, image::Rgb([255, 255, 255]));
    for oy in 0..h {
        for ox in 0..w {
            let dx = ox as f32 - cx;
            let dy = oy as f32 - cy;
            // Inverse map (output → source) = forward rotation of content by `angle`.
            let sx = cx + dx * c + dy * s;
            let sy = cy - dx * s + dy * c;
            if sx < 0.0 || sy < 0.0 || sx > (w - 1) as f32 || sy > (h - 1) as f32 {
                continue; // stays white
            }
            *out.get_pixel_mut(ox, oy) = bilinear(src, sx, sy);
        }
    }
    out
}

fn bilinear(src: &RgbImage, x: f32, y: f32) -> image::Rgb<u8> {
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(src.width() - 1);
    let y1 = (y0 + 1).min(src.height() - 1);
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let mut out = [0u8; 3];
    for ch in 0..3 {
        let p00 = f32::from(src.get_pixel(x0, y0)[ch]);
        let p10 = f32::from(src.get_pixel(x1, y0)[ch]);
        let p01 = f32::from(src.get_pixel(x0, y1)[ch]);
        let p11 = f32::from(src.get_pixel(x1, y1)[ch]);
        let top = p00 + (p10 - p00) * fx;
        let bot = p01 + (p11 - p01) * fx;
        out[ch] = (top + (bot - top) * fy).round().clamp(0.0, 255.0) as u8;
    }
    image::Rgb(out)
}

// ---------------------------------------------------------------------------
// Enhancement modes.
// ---------------------------------------------------------------------------

fn enhance(rgb: RgbImage, opts: &ScanOptions) -> PageBuf {
    match opts.mode {
        Mode::Color => {
            let out = apply_contrast_brightness_rgb(rgb, opts.contrast, opts.brightness);
            rgb_page(out)
        }
        Mode::Magic => rgb_page(magic_color(rgb, opts.contrast, opts.brightness)),
        Mode::Grayscale => {
            let gray = to_gray_adjusted(&rgb, opts.contrast, opts.brightness);
            gray_page(gray)
        }
        Mode::BlackWhite => {
            let luma = to_luma(&rgb);
            let bw = adaptive_threshold(&luma, opts.brightness);
            gray_page(despeckle(&bw))
        }
    }
}

fn clamp_u8(f: f32) -> u8 {
    f.round().clamp(0.0, 255.0) as u8
}

/// `out = (v - 128) * contrast + 128 + brightness`, clamped.
fn adjust(v: u8, contrast: f32, brightness: f32) -> u8 {
    clamp_u8((f32::from(v) - 128.0) * contrast + 128.0 + brightness)
}

fn apply_contrast_brightness_rgb(mut img: RgbImage, contrast: f32, brightness: f32) -> RgbImage {
    for p in img.pixels_mut() {
        for ch in 0..3 {
            p[ch] = adjust(p[ch], contrast, brightness);
        }
    }
    img
}

fn to_gray_adjusted(rgb: &RgbImage, contrast: f32, brightness: f32) -> GrayImage {
    let mut out = GrayImage::new(rgb.width(), rgb.height());
    for (o, p) in out.pixels_mut().zip(rgb.pixels()) {
        o[0] = adjust(luma601(p[0], p[1], p[2]), contrast, brightness);
    }
    out
}

/// Magic Color: per-channel white-point normalisation (whiten paper, cut a mild
/// colour cast) → saturation lift → contrast S-curve → brightness.
fn magic_color(rgb: RgbImage, contrast: f32, brightness: f32) -> RgbImage {
    let scale = white_point_scale(&rgb);
    let boost = contrast * 1.35; // magic mode contrasts harder than "color"
    let sat = 1.25_f32;
    let mut out = rgb;
    for p in out.pixels_mut() {
        // 1. white balance / paper whitening
        let mut c = [0f32; 3];
        for ch in 0..3 {
            c[ch] = (f32::from(p[ch]) * scale[ch]).clamp(0.0, 255.0);
        }
        // 2. saturation lift around this pixel's luma
        let g = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
        for ch in 0..3 {
            c[ch] = (g + (c[ch] - g) * sat).clamp(0.0, 255.0);
        }
        // 3. contrast S-curve + brightness
        for ch in 0..3 {
            p[ch] = clamp_u8((c[ch] - 128.0) * boost + 128.0 + brightness);
        }
    }
    out
}

/// Per-channel scale that maps the ~80th brightness percentile to white, so the
/// paper background lifts to 255. Bounded to [1.0, 2.5] to avoid blowing up dark
/// photos with no bright paper.
fn white_point_scale(rgb: &RgbImage) -> [f32; 3] {
    let mut scale = [1.0f32; 3];
    for ch in 0..3 {
        let mut hist = [0u32; 256];
        for p in rgb.pixels() {
            hist[p[ch] as usize] += 1;
        }
        let total: u32 = hist.iter().sum();
        let target = (total as f32 * 0.80) as u32;
        let mut acc = 0u32;
        let mut pct = 255u32;
        for (v, &count) in hist.iter().enumerate() {
            acc += count;
            if acc >= target {
                pct = v as u32;
                break;
            }
        }
        let wp = pct.max(1) as f32;
        scale[ch] = (255.0 / wp).clamp(1.0, 2.5);
    }
    scale
}

// ---------------------------------------------------------------------------
// Adaptive threshold (local mean via integral image) + median despeckle.
// ---------------------------------------------------------------------------

/// Local-mean adaptive threshold: a pixel is black iff it is darker than the
/// mean of its neighbourhood minus a bias. `brightness` shifts the bias so a
/// brighter setting keeps less ink. Produces a 0/255 grayscale image.
fn adaptive_threshold(luma: &GrayImage, brightness: f32) -> GrayImage {
    let (w, h) = (luma.width(), luma.height());
    // Window radius ~ 1/20 of the short side, clamped to a sane range.
    let radius = (w.min(h) / 20).clamp(6, 40) as i64;
    let bias = (10.0 - brightness * 0.2).clamp(-40.0, 60.0);

    // Integral image: sum[y*(w+1)+x] = sum of luma over the rect (0,0)..(x,y).
    let iw = (w + 1) as usize;
    let mut sum = vec![0u64; iw * (h as usize + 1)];
    for y in 0..h as usize {
        let mut row = 0u64;
        for x in 0..w as usize {
            row += u64::from(luma.get_pixel(x as u32, y as u32)[0]);
            sum[(y + 1) * iw + (x + 1)] = sum[y * iw + (x + 1)] + row;
        }
    }
    let region_sum = |x0: i64, y0: i64, x1: i64, y1: i64| -> u64 {
        let x0 = x0.clamp(0, w as i64) as usize;
        let y0 = y0.clamp(0, h as i64) as usize;
        let x1 = x1.clamp(0, w as i64) as usize;
        let y1 = y1.clamp(0, h as i64) as usize;
        sum[y1 * iw + x1] + sum[y0 * iw + x0] - sum[y0 * iw + x1] - sum[y1 * iw + x0]
    };

    let mut out = GrayImage::new(w, h);
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            let x0 = x - radius;
            let y0 = y - radius;
            let x1 = x + radius + 1;
            let y1 = y + radius + 1;
            let area = ((x1.clamp(0, w as i64) - x0.clamp(0, w as i64))
                * (y1.clamp(0, h as i64) - y0.clamp(0, h as i64))) as u64;
            let s = region_sum(x0, y0, x1, y1);
            let mean = s as f32 / area.max(1) as f32;
            let v = f32::from(luma.get_pixel(x as u32, y as u32)[0]);
            let px = if v < mean - bias { 0 } else { 255 };
            out.put_pixel(x as u32, y as u32, image::Luma([px]));
        }
    }
    out
}

/// 3×3 majority (median for binary) filter — removes isolated black specks and
/// fills isolated white pinholes in a thresholded scan.
fn despeckle(bw: &GrayImage) -> GrayImage {
    let (w, h) = (bw.width(), bw.height());
    let mut out = GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut black = 0;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = (x as i32 + dx).clamp(0, w as i32 - 1) as u32;
                    let ny = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
                    if bw.get_pixel(nx, ny)[0] < 128 {
                        black += 1;
                    }
                }
            }
            out.put_pixel(x, y, image::Luma([if black >= 5 { 0 } else { 255 }]));
        }
    }
    out
}

fn rgb_page(img: RgbImage) -> PageBuf {
    PageBuf { w: img.width(), h: img.height(), raw: img.into_raw(), gray: false }
}

fn gray_page(img: GrayImage) -> PageBuf {
    PageBuf { w: img.width(), h: img.height(), raw: img.into_raw(), gray: true }
}

// ---------------------------------------------------------------------------
// PDF assembly.
// ---------------------------------------------------------------------------

/// Page geometry for the chosen size: page box (pw,ph) and the placed image
/// rectangle (draw_w, draw_h, ox, oy) in points.
fn page_geometry(size: PageSize, iw: u32, ih: u32) -> (f32, f32, f32, f32, f32, f32) {
    let (iw, ih) = (iw as f32, ih as f32);
    match size {
        PageSize::Fit => (iw, ih, iw, ih, 0.0, 0.0),
        PageSize::A4 => fit_into(595.0, 842.0, iw, ih),
        PageSize::Letter => fit_into(612.0, 792.0, iw, ih),
    }
}

fn fit_into(pw: f32, ph: f32, iw: f32, ih: f32) -> (f32, f32, f32, f32, f32, f32) {
    let scale = (pw / iw).min(ph / ih);
    let dw = iw * scale;
    let dh = ih * scale;
    (pw, ph, dw, dh, (pw - dw) / 2.0, (ph - dh) / 2.0)
}

fn assemble_pdf(pages: &[PageBuf], size: PageSize) -> Result<Vec<u8>, String> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids: Vec<Object> = Vec::with_capacity(pages.len());

    for (i, page) in pages.iter().enumerate() {
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&page.raw)
            .and_then(|_| enc.flush())
            .map_err(|e| format!("compress page #{}: {e}", i + 1))?;
        let compressed = enc.finish().map_err(|e| format!("compress page #{}: {e}", i + 1))?;

        let colorspace = if page.gray { "DeviceGray" } else { "DeviceRGB" };
        let mut img_stream = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => page.w as i64,
                "Height" => page.h as i64,
                "ColorSpace" => colorspace,
                "BitsPerComponent" => 8,
                "Filter" => "FlateDecode",
            },
            compressed,
        );
        img_stream.allows_compression = false;
        let img_id = doc.add_object(img_stream);

        let (pw, ph, dw, dh, ox, oy) = page_geometry(size, page.w, page.h);
        let content = format!("q {dw:.2} 0 0 {dh:.2} {ox:.2} {oy:.2} cm /Img Do Q");
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));

        let resources_id = doc.add_object(dictionary! {
            "XObject" => dictionary! { "Img" => img_id },
        });
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![
                0.into(),
                0.into(),
                Object::Real(pw),
                Object::Real(ph),
            ],
            "Contents" => content_id,
            "Resources" => resources_id,
        });
        page_ids.push(page_id.into());
    }

    let count = page_ids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids,
            "Count" => count,
        }),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);

    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| format!("failed to serialize PDF: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn png(img: &RgbImage) -> Vec<u8> {
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(img.clone())
            .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    /// White page with a few horizontal black bars — a stand-in for text lines.
    fn barred(w: u32, h: u32) -> RgbImage {
        let mut img = RgbImage::from_pixel(w, h, image::Rgb([255, 255, 255]));
        for y in 0..h {
            if (y / 8) % 3 == 0 {
                for x in 0..w {
                    img.put_pixel(x, y, image::Rgb([0, 0, 0]));
                }
            }
        }
        img
    }

    fn page_count(pdf: &[u8]) -> usize {
        Document::load_mem(pdf).expect("output should parse").get_pages().len()
    }

    /// Variance of per-row darkness — high when horizontal structure is aligned
    /// to rows (straight), low when it is smeared across rows (skewed).
    fn row_dark_variance(rgb: &RgbImage) -> f64 {
        let (w, h) = (rgb.width(), rgb.height());
        let mut rows = vec![0f64; h as usize];
        for y in 0..h {
            let mut dark = 0f64;
            for x in 0..w {
                if luma601_px(rgb.get_pixel(x, y)) < 128 {
                    dark += 1.0;
                }
            }
            rows[y as usize] = dark;
        }
        let mean = rows.iter().sum::<f64>() / rows.len() as f64;
        rows.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / rows.len() as f64
    }

    fn luma601_px(p: &image::Rgb<u8>) -> u8 {
        luma601(p[0], p[1], p[2])
    }

    #[test]
    fn happy_one_photo_one_page() {
        let pdf = scan_to_pdf(&[png(&barred(120, 160))], &ScanOptions::default()).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
        assert_eq!(page_count(&pdf), 1);
    }

    #[test]
    fn three_photos_three_pages_in_order() {
        let imgs = vec![png(&barred(80, 100)), png(&barred(90, 110)), png(&barred(70, 90))];
        let pdf = scan_to_pdf(&imgs, &ScanOptions::default()).unwrap();
        assert_eq!(page_count(&pdf), 3);
    }

    #[test]
    fn error_on_empty_input() {
        let err = scan_to_pdf(&[], &ScanOptions::default()).unwrap_err();
        assert!(err.contains("at least one"), "got: {err}");
    }

    #[test]
    fn error_on_undecodable_image() {
        let err = scan_to_pdf(&[b"not an image".to_vec()], &ScanOptions::default()).unwrap_err();
        assert!(err.contains("failed to decode"), "got: {err}");
    }

    #[test]
    fn mode_and_page_size_parse_and_reject() {
        assert_eq!(Mode::parse("magic").unwrap(), Mode::Magic);
        assert_eq!(Mode::parse("blackwhite").unwrap(), Mode::BlackWhite);
        assert!(Mode::parse("sepia").is_err());
        assert_eq!(PageSize::parse("a4").unwrap(), PageSize::A4);
        assert!(PageSize::parse("legal").is_err());
    }

    #[test]
    fn blackwhite_is_pure_black_and_white() {
        // A grey gradient thresholds to only 0 or 255 samples.
        let mut img = RgbImage::new(64, 64);
        for (x, _y, p) in img.enumerate_pixels_mut() {
            let v = (x * 4) as u8;
            *p = image::Rgb([v, v, v]);
        }
        let page = enhance(img, &ScanOptions { mode: Mode::BlackWhite, ..Default::default() });
        assert!(page.gray, "B&W embeds as DeviceGray");
        assert!(page.raw.iter().all(|&v| v == 0 || v == 255), "only pure black/white samples");
    }

    #[test]
    fn a4_page_has_a4_mediabox() {
        let pdf = scan_to_pdf(
            &[png(&barred(200, 300))],
            &ScanOptions { page_size: PageSize::A4, ..Default::default() },
        )
        .unwrap();
        let doc = Document::load_mem(&pdf).unwrap();
        let (_, page_id) = doc.get_pages().into_iter().next().unwrap();
        let mb =
            doc.get_dictionary(page_id).unwrap().get(b"MediaBox").unwrap().as_array().unwrap();
        let w = mb[2].as_float().unwrap();
        let h = mb[3].as_float().unwrap();
        assert!((w - 595.0).abs() < 0.5 && (h - 842.0).abs() < 0.5, "A4 box, got {w}x{h}");
    }

    #[test]
    fn deskew_straightens_a_tilted_page() {
        let base = barred(160, 160);
        // Introduce a known +6° skew, then let the pipeline auto-correct it.
        let skewed = rotate_rgb(&base, 6f32.to_radians());
        let detected = detect_skew_deg(&to_luma(&skewed));
        // The correction angle should undo the +6° tilt (≈ -6°).
        assert!((detected + 6.0).abs() < 2.0, "detected {detected}, expected ~ -6");
        let corrected = rotate_rgb(&skewed, detected.to_radians());
        // Straightened output has far sharper per-row structure than the skew.
        assert!(
            row_dark_variance(&corrected) > row_dark_variance(&skewed) * 2.0,
            "corrected variance {} should beat skewed {}",
            row_dark_variance(&corrected),
            row_dark_variance(&skewed)
        );
    }

    #[test]
    fn deskew_leaves_a_straight_page_alone() {
        // A perfectly straight page should detect ~0° and not be rotated away.
        let base = barred(160, 160);
        let angle = detect_skew_deg(&to_luma(&base));
        assert!(angle.abs() < 1.0, "straight page should detect ~0, got {angle}");
    }
}
