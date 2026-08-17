//! chromatic-aberration-fix core — pure compute, shared by the chat skill block.
//!
//! Removes the purple/violet and green colour fringing that lenses leave along
//! high-contrast edges. Two independent stages run in the same order a raw
//! developer applies them:
//!
//!   1. **Lateral (transverse) CA** — the red and blue channels are focused at a
//!      slightly different magnification than green, so fringes grow radially
//!      toward the frame corners. Corrected by radially resampling the red and
//!      blue channels about the image centre (`red_shift` / `blue_shift`, in
//!      pixels measured at the corner). Off by default.
//!   2. **Axial (longitudinal) CA / defringe** — the purple or green halo that
//!      remains on a blown edge regardless of position. Corrected by finding
//!      high-contrast edges (a local luma max−min contrast map, dilated by
//!      `radius` with an exact O(1)-per-pixel sliding maximum), then, for pixels
//!      near such an edge whose hue falls inside a window around violet (285°)
//!      or green (120°), scaling away the offending channels' EXCESS over the
//!      reference channel.
//!
//! The excess-only rule is what keeps this from being a blunt desaturation: for
//! a purple fringe only `R−G` and `B−G` above zero are reduced, so red and blue
//! can never be pushed BELOW green, and a pixel that is only mildly tinted is
//! only mildly corrected. Green fringes are the mirror case (`G − max(R,B)`).
//!
//! Alpha is always preserved untouched. Output format is chosen by the caller
//! (`Format::Auto` keeps the input's container, so a PNG with transparency stays
//! a PNG).

use std::io::Cursor;

use image::{
    codecs::jpeg::JpegEncoder, DynamicImage, ImageEncoder, ImageFormat, ImageReader, RgbaImage,
};

/// Largest raster we will decode + process inside the 64 MiB wasm sandbox.
/// Budgeted header-first so an over-size input gets an actionable error instead
/// of an OOM trap.
const MAX_PIXEL_BYTES: u64 = 48 * 1024 * 1024;

/// Hue centre of violet/purple fringing, in degrees.
const PURPLE_HUE: f32 = 285.0;
/// Hue centre of green fringing, in degrees.
const GREEN_HUE: f32 = 120.0;

/// Output container for the corrected image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Keep the input's format (PNG/JPEG/WebP); anything else becomes PNG.
    Auto,
    Png,
    Jpeg,
    WebP,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "" => Ok(Format::Auto),
            "png" => Ok(Format::Png),
            "jpeg" | "jpg" => Ok(Format::Jpeg),
            "webp" => Ok(Format::WebP),
            other => Err(format!(
                "unknown format '{other}' (use 'auto', 'png', 'jpeg', or 'webp')"
            )),
        }
    }

    /// Resolve `Auto` against the format the input file was decoded as.
    fn resolve(self, input: ImageFormat) -> Format {
        match self {
            Format::Auto => match input {
                ImageFormat::Jpeg => Format::Jpeg,
                ImageFormat::WebP => Format::WebP,
                _ => Format::Png,
            },
            other => other,
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Format::Jpeg => "image/jpeg",
            Format::WebP => "image/webp",
            _ => "image/png",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Jpeg => "jpg",
            Format::WebP => "webp",
            _ => "png",
        }
    }
}

/// Everything the correction needs, already range-checked.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Purple/violet fringe strength, 0–20. 0 disables the purple pass.
    pub purple_amount: u32,
    /// Green fringe strength, 0–20. 0 disables the green pass.
    pub green_amount: u32,
    /// Minimum local luma contrast (0–255) for a pixel to count as an edge.
    pub edge_threshold: u32,
    /// How far (px) the correction reaches around a detected edge, 1–20.
    pub radius: u32,
    /// Hue half-window (degrees, 5–90) around the violet/green centres.
    pub hue_tolerance: u32,
    /// Lateral red-channel correction in px at the frame corner, −10…10.
    pub red_shift: f32,
    /// Lateral blue-channel correction in px at the frame corner, −10…10.
    pub blue_shift: f32,
    /// Output container.
    pub format: Format,
    /// Encoder quality 1–100 (JPEG only; PNG/WebP here are lossless).
    pub quality: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            purple_amount: 8,
            green_amount: 5,
            edge_threshold: 20,
            radius: 4,
            hue_tolerance: 40,
            red_shift: 0.0,
            blue_shift: 0.0,
            format: Format::Auto,
            quality: 90,
        }
    }
}

impl Settings {
    /// Clamp every field into its advertised range. Callers pass user input
    /// straight through; nothing here can panic.
    pub fn clamped(mut self) -> Settings {
        self.purple_amount = self.purple_amount.min(20);
        self.green_amount = self.green_amount.min(20);
        self.edge_threshold = self.edge_threshold.min(255);
        self.radius = self.radius.clamp(1, 20);
        self.hue_tolerance = self.hue_tolerance.clamp(5, 90);
        self.red_shift = self.red_shift.clamp(-10.0, 10.0);
        self.blue_shift = self.blue_shift.clamp(-10.0, 10.0);
        self.quality = self.quality.clamp(1, 100);
        self
    }
}

/// What the correction actually did, for the caller's summary line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Report {
    pub width: u32,
    pub height: u32,
    /// Pixels whose purple fringe was reduced.
    pub purple_pixels: u64,
    /// Pixels whose green fringe was reduced.
    pub green_pixels: u64,
    /// True if a lateral (radial) channel resample ran.
    pub lateral_applied: bool,
    pub format: Format,
}

/// Decode `bytes`, correct chromatic aberration, and re-encode.
///
/// Returns the encoded image plus a [`Report`]. Every error is a caller-facing
/// string that names what was expected.
pub fn fix_chromatic_aberration(
    bytes: &[u8],
    settings: Settings,
) -> Result<(Vec<u8>, Report), String> {
    let s = settings.clamped();
    if bytes.is_empty() {
        return Err("empty image input: expected PNG, JPEG, WebP, GIF, or BMP bytes".to_string());
    }

    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("could not read the image header: {e}"))?;
    let input_format = reader.format().ok_or_else(|| {
        "unrecognized image format: expected PNG, JPEG, WebP, GIF, or BMP".to_string()
    })?;

    // Budget BEFORE decoding: a 25 MP scan would otherwise OOM-trap the sandbox
    // as a bare `wasm unreachable` rather than an explainable error.
    let decoder = reader
        .into_decoder()
        .map_err(|e| format!("could not decode the image: {e}"))?;
    let (width, height) = image::ImageDecoder::dimensions(&decoder);
    if width == 0 || height == 0 {
        return Err(format!("image has zero area ({width}x{height})"));
    }
    // RGBA8 working copy + the u8 contrast map + the dilated map.
    let working = (width as u64) * (height as u64) * 6;
    if working > MAX_PIXEL_BYTES {
        return Err(format!(
            "image is too large to process here: {width}x{height} needs about {} MB of working \
             memory but the limit is {} MB — re-export it at a lower resolution",
            working / (1024 * 1024),
            MAX_PIXEL_BYTES / (1024 * 1024)
        ));
    }
    let mut img: RgbaImage = DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("could not decode the image: {e}"))?
        .to_rgba8();

    let lateral_applied = s.red_shift != 0.0 || s.blue_shift != 0.0;
    if lateral_applied {
        img = correct_lateral(&img, s.red_shift, s.blue_shift);
    }

    let (purple_pixels, green_pixels) = defringe(&mut img, &s);

    let out_format = s.format.resolve(input_format);
    let encoded = encode(&img, out_format, s.quality)?;

    Ok((
        encoded,
        Report {
            width,
            height,
            purple_pixels,
            green_pixels,
            lateral_applied,
            format: out_format,
        },
    ))
}

// ---------------------------------------------------------------------------
// Stage 1 — lateral CA: radial per-channel resample
// ---------------------------------------------------------------------------

/// Radially rescale the red and blue channels about the image centre. `red`
/// and `blue` are the correction measured in pixels at the frame CORNER; a
/// positive value pulls that channel inward (fixing a fringe of that colour on
/// the outer side of an edge). Green and alpha are left exactly as they were,
/// so green is the geometric reference the other two are matched to.
fn correct_lateral(src: &RgbaImage, red: f32, blue: f32) -> RgbaImage {
    let (w, h) = src.dimensions();
    let cx = (w as f32 - 1.0) / 2.0;
    let cy = (h as f32 - 1.0) / 2.0;
    let corner = (cx * cx + cy * cy).sqrt().max(1.0);
    // shift px at the corner → a scale factor applied to the sampling radius.
    let red_scale = 1.0 + red / corner;
    let blue_scale = 1.0 + blue / corner;

    let mut out = src.clone();
    for y in 0..h {
        let dy = y as f32 - cy;
        for x in 0..w {
            let dx = x as f32 - cx;
            let p = out.get_pixel_mut(x, y);
            p[0] = sample_channel(src, 0, cx + dx * red_scale, cy + dy * red_scale);
            p[2] = sample_channel(src, 2, cx + dx * blue_scale, cy + dy * blue_scale);
        }
    }
    out
}

/// Bilinear sample of one channel, clamping to the edge outside the raster.
fn sample_channel(src: &RgbaImage, ch: usize, x: f32, y: f32) -> u8 {
    let (w, h) = src.dimensions();
    let xc = x.clamp(0.0, w as f32 - 1.0);
    let yc = y.clamp(0.0, h as f32 - 1.0);
    let x0 = xc.floor() as u32;
    let y0 = yc.floor() as u32;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let fx = xc - x0 as f32;
    let fy = yc - y0 as f32;
    let v = |px: u32, py: u32| src.get_pixel(px, py)[ch] as f32;
    let top = v(x0, y0) * (1.0 - fx) + v(x1, y0) * fx;
    let bottom = v(x0, y1) * (1.0 - fx) + v(x1, y1) * fx;
    (top * (1.0 - fy) + bottom * fy).round().clamp(0.0, 255.0) as u8
}

// ---------------------------------------------------------------------------
// Stage 2 — axial CA: edge-gated, hue-windowed defringe
// ---------------------------------------------------------------------------

/// Rec.601 luma, the contrast signal the edge map is built from.
fn luma(r: u8, g: u8, b: u8) -> u8 {
    ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000).min(255) as u8
}

/// Hue in degrees (0–360) and chroma (max−min, 0–255) of an RGB triple.
fn hue_chroma(r: u8, g: u8, b: u8) -> (f32, f32) {
    let (rf, gf, bf) = (r as f32, g as f32, b as f32);
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let c = max - min;
    if c <= 0.0 {
        return (0.0, 0.0);
    }
    let h = if max == rf {
        60.0 * (((gf - bf) / c) % 6.0)
    } else if max == gf {
        60.0 * ((bf - rf) / c + 2.0)
    } else {
        60.0 * ((rf - gf) / c + 4.0)
    };
    ((h + 360.0) % 360.0, c)
}

/// Shortest distance between two hues on the colour circle, in degrees.
fn hue_distance(a: f32, b: f32) -> f32 {
    let d = (a - b).abs() % 360.0;
    if d > 180.0 {
        360.0 - d
    } else {
        d
    }
}

/// 1 inside the inner 70 % of the window, ramping linearly to 0 at `tol`.
fn hue_falloff(dist: f32, tol: f32) -> f32 {
    let inner = tol * 0.7;
    if dist <= inner {
        1.0
    } else if dist >= tol {
        0.0
    } else {
        1.0 - (dist - inner) / (tol - inner)
    }
}

/// Exact sliding maximum over a window of `2*radius+1`, via a monotonic deque —
/// O(1) amortized per pixel, so the edge dilation cost does not grow with
/// `radius` (a naive box max at radius 20 is 41× slower per axis).
fn sliding_max_1d(
    src: &[u8],
    dst: &mut [u8],
    len: usize,
    stride: usize,
    base: usize,
    radius: usize,
) {
    let mut deque: Vec<usize> = Vec::with_capacity(len.min(2 * radius + 2));
    let mut head = 0usize;
    for i in 0..len + radius {
        if i < len {
            let v = src[base + i * stride];
            while deque.len() > head && src[base + deque[deque.len() - 1] * stride] <= v {
                deque.pop();
            }
            deque.push(i);
        }
        if i >= radius {
            let out_idx = i - radius;
            while deque.len() > head && deque[head] + radius < out_idx {
                head += 1;
            }
            if deque.len() > head {
                dst[base + out_idx * stride] = src[base + deque[head] * stride];
            }
        }
    }
}

/// Local luma contrast (3x3 max − min) dilated by `radius` in both axes.
fn edge_map(img: &RgbaImage, radius: usize) -> Vec<u8> {
    let (w, h) = img.dimensions();
    let (wu, hu) = (w as usize, h as usize);
    let mut lum = vec![0u8; wu * hu];
    for y in 0..hu {
        for x in 0..wu {
            let p = img.get_pixel(x as u32, y as u32);
            lum[y * wu + x] = luma(p[0], p[1], p[2]);
        }
    }
    // 3x3 max − min contrast.
    let mut contrast = vec![0u8; wu * hu];
    for y in 0..hu {
        let y0 = y.saturating_sub(1);
        let y1 = (y + 1).min(hu - 1);
        for x in 0..wu {
            let x0 = x.saturating_sub(1);
            let x1 = (x + 1).min(wu - 1);
            let (mut mn, mut mx) = (255u8, 0u8);
            for yy in y0..=y1 {
                for xx in x0..=x1 {
                    let v = lum[yy * wu + xx];
                    mn = mn.min(v);
                    mx = mx.max(v);
                }
            }
            contrast[y * wu + x] = mx - mn;
        }
    }
    // Dilate: horizontal pass then vertical pass.
    let mut tmp = vec![0u8; wu * hu];
    for y in 0..hu {
        sliding_max_1d(&contrast, &mut tmp, wu, 1, y * wu, radius);
    }
    let mut out = vec![0u8; wu * hu];
    for x in 0..wu {
        sliding_max_1d(&tmp, &mut out, hu, wu, x, radius);
    }
    out
}

/// Apply the hue-windowed, edge-gated fringe reduction in place. Returns
/// (purple pixels touched, green pixels touched).
fn defringe(img: &mut RgbaImage, s: &Settings) -> (u64, u64) {
    if s.purple_amount == 0 && s.green_amount == 0 {
        return (0, 0);
    }
    let (w, h) = img.dimensions();
    let wu = w as usize;
    let edges = edge_map(img, s.radius as usize);
    let thr = s.edge_threshold as f32;
    let tol = s.hue_tolerance as f32;
    let purple_max = s.purple_amount as f32 / 20.0;
    let green_max = s.green_amount as f32 / 20.0;

    let (mut purple_hits, mut green_hits) = (0u64, 0u64);
    for y in 0..h {
        for x in 0..w {
            // How strong is the nearest edge, relative to the threshold?
            let e = edges[y as usize * wu + x as usize] as f32;
            if e < thr {
                continue;
            }
            // Ramp from 0 at the threshold to 1 at twice the threshold, so a
            // barely-qualifying edge is corrected gently. A threshold of 0
            // means "everywhere, at full strength".
            let edge_gain = if thr <= 0.0 {
                1.0
            } else {
                ((e - thr) / thr).clamp(0.0, 1.0)
            };
            if edge_gain <= 0.0 {
                continue;
            }

            let px = img.get_pixel_mut(x, y);
            let (r, g, b) = (px[0] as f32, px[1] as f32, px[2] as f32);
            let (hue, chroma) = hue_chroma(px[0], px[1], px[2]);
            if chroma < 1.0 {
                continue; // already neutral — nothing to defringe
            }

            // Purple/violet: red AND blue both sit above green.
            if purple_max > 0.0 && r > g && b > g {
                let fall = hue_falloff(hue_distance(hue, PURPLE_HUE), tol);
                if fall > 0.0 {
                    let k = purple_max * fall * edge_gain;
                    px[0] = (r - (r - g) * k).round().clamp(0.0, 255.0) as u8;
                    px[2] = (b - (b - g) * k).round().clamp(0.0, 255.0) as u8;
                    purple_hits += 1;
                    continue;
                }
            }
            // Green: green sits above both red and blue.
            if green_max > 0.0 && g > r && g > b {
                let fall = hue_falloff(hue_distance(hue, GREEN_HUE), tol);
                if fall > 0.0 {
                    let reference = r.max(b);
                    let k = green_max * fall * edge_gain;
                    px[1] = (g - (g - reference) * k).round().clamp(0.0, 255.0) as u8;
                    green_hits += 1;
                }
            }
        }
    }
    (purple_hits, green_hits)
}

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------

fn encode(img: &RgbaImage, format: Format, quality: u32) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    match format {
        Format::Jpeg => {
            // JPEG has no alpha channel — flatten to RGB.
            let rgb = DynamicImage::ImageRgba8(img.clone()).to_rgb8();
            JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100) as u8)
                .write_image(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    image::ExtendedColorType::Rgb8,
                )
                .map_err(|e| format!("could not encode JPEG: {e}"))?;
        }
        Format::WebP => {
            DynamicImage::ImageRgba8(img.clone())
                .write_to(&mut out, ImageFormat::WebP)
                .map_err(|e| format!("could not encode WebP: {e}"))?;
        }
        _ => {
            DynamicImage::ImageRgba8(img.clone())
                .write_to(&mut out, ImageFormat::Png)
                .map_err(|e| format!("could not encode PNG: {e}"))?;
        }
    }
    Ok(out.into_inner())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// A black→white vertical edge with a 1-px violet fringe on the light side.
    fn fringed_edge() -> Vec<u8> {
        let mut img = RgbaImage::from_pixel(8, 4, Rgba([10, 10, 10, 255]));
        for y in 0..4 {
            img.put_pixel(4, y, Rgba([150, 60, 190, 255])); // violet fringe
            for x in 5..8 {
                img.put_pixel(x, y, Rgba([240, 240, 240, 255]));
            }
        }
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    fn decode(bytes: &[u8]) -> RgbaImage {
        image::load_from_memory(bytes).unwrap().to_rgba8()
    }

    #[test]
    fn happy_path_removes_the_violet_fringe() {
        let (out, report) = fix_chromatic_aberration(&fringed_edge(), Settings::default()).unwrap();
        let img = decode(&out);
        let p = img.get_pixel(4, 1);
        // Red and blue are pulled toward green by purple_amount/20 = 0.4.
        assert_eq!(p[1], 60, "green is the reference and must not move");
        assert_eq!(p[0], 114, "red excess (150-60) reduced by 40%");
        assert_eq!(p[2], 138, "blue excess (190-60) reduced by 40%");
        assert_eq!(p[3], 255, "alpha preserved");
        assert!(report.purple_pixels >= 4, "one fringe pixel per row");
        assert_eq!(report.format, Format::Png);
        assert_eq!((report.width, report.height), (8, 4));
    }

    #[test]
    fn full_strength_lands_exactly_on_neutral() {
        let s = Settings {
            purple_amount: 20,
            ..Settings::default()
        };
        let img = decode(&fix_chromatic_aberration(&fringed_edge(), s).unwrap().0);
        assert_eq!(img.get_pixel(4, 1).0, [60, 60, 60, 255]);
    }

    #[test]
    fn flat_areas_far_from_an_edge_are_untouched() {
        // A uniform violet field: no luma contrast anywhere, so nothing qualifies.
        let mut img = RgbaImage::from_pixel(16, 16, Rgba([150, 60, 190, 255]));
        img.put_pixel(0, 0, Rgba([150, 60, 190, 255]));
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let (out, report) =
            fix_chromatic_aberration(&buf.into_inner(), Settings::default()).unwrap();
        assert_eq!(report.purple_pixels, 0, "no edge ⇒ no correction");
        assert_eq!(decode(&out).get_pixel(8, 8).0, [150, 60, 190, 255]);
    }

    #[test]
    fn green_fringe_is_reduced_toward_the_brighter_of_red_and_blue() {
        let mut img = RgbaImage::from_pixel(8, 4, Rgba([10, 10, 10, 255]));
        for y in 0..4 {
            img.put_pixel(4, y, Rgba([80, 200, 60, 255])); // green fringe
            for x in 5..8 {
                img.put_pixel(x, y, Rgba([240, 240, 240, 255]));
            }
        }
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let (out, report) =
            fix_chromatic_aberration(&buf.into_inner(), Settings::default()).unwrap();
        // green_amount 5/20 = 0.25 of the excess (200-80=120) removed → 170.
        assert_eq!(decode(&out).get_pixel(4, 1)[1], 170);
        assert!(report.green_pixels >= 4);
    }

    #[test]
    fn amount_zero_is_a_no_op() {
        let s = Settings {
            purple_amount: 0,
            green_amount: 0,
            ..Settings::default()
        };
        let (out, report) = fix_chromatic_aberration(&fringed_edge(), s).unwrap();
        assert_eq!((report.purple_pixels, report.green_pixels), (0, 0));
        assert_eq!(decode(&out).get_pixel(4, 1).0, [150, 60, 190, 255]);
    }

    #[test]
    fn hue_window_protects_a_legitimately_red_edge() {
        // Pure red (hue 0) is 285° away from violet — far outside any window.
        let mut img = RgbaImage::from_pixel(8, 4, Rgba([10, 10, 10, 255]));
        for y in 0..4 {
            img.put_pixel(4, y, Rgba([220, 20, 20, 255]));
            for x in 5..8 {
                img.put_pixel(x, y, Rgba([240, 240, 240, 255]));
            }
        }
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let (out, report) =
            fix_chromatic_aberration(&buf.into_inner(), Settings::default()).unwrap();
        assert_eq!(report.purple_pixels, 0);
        assert_eq!(decode(&out).get_pixel(4, 1).0, [220, 20, 20, 255]);
    }

    #[test]
    fn lateral_shift_resamples_red_and_blue_but_never_green() {
        let mut img = RgbaImage::from_pixel(21, 21, Rgba([0, 0, 0, 255]));
        for y in 0..21 {
            for x in 0..21 {
                // A red ramp increasing outward; flat green so green can be checked.
                img.put_pixel(x, y, Rgba([(x * 12) as u8, 90, 40, 255]));
            }
        }
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let s = Settings {
            purple_amount: 0,
            green_amount: 0,
            red_shift: 4.0,
            ..Settings::default()
        };
        let (out, report) = fix_chromatic_aberration(&buf.into_inner(), s).unwrap();
        assert!(report.lateral_applied);
        let got = decode(&out);
        assert_eq!(
            got.get_pixel(2, 10)[1],
            90,
            "green channel is the reference"
        );
        assert_ne!(
            got.get_pixel(2, 10)[0],
            24,
            "red channel was radially resampled"
        );
    }

    #[test]
    fn format_auto_keeps_jpeg_and_explicit_png_overrides_it() {
        let png = fringed_edge();
        let jpeg = {
            let img = decode(&png);
            let mut buf = Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(img)
                .to_rgb8()
                .write_to(&mut buf, ImageFormat::Jpeg)
                .unwrap();
            buf.into_inner()
        };
        let (out, report) = fix_chromatic_aberration(&jpeg, Settings::default()).unwrap();
        assert_eq!(report.format, Format::Jpeg);
        assert_eq!(&out[0..2], &[0xFF, 0xD8], "JPEG SOI marker");

        let s = Settings {
            format: Format::Png,
            ..Settings::default()
        };
        let (out, report) = fix_chromatic_aberration(&jpeg, s).unwrap();
        assert_eq!(report.format, Format::Png);
        assert_eq!(&out[1..4], b"PNG");
    }

    #[test]
    fn error_on_non_image_bytes() {
        let err = fix_chromatic_aberration(b"this is not an image at all", Settings::default())
            .unwrap_err();
        assert!(
            err.contains("unrecognized image format") || err.contains("could not"),
            "unhelpful error: {err}"
        );
    }

    #[test]
    fn error_on_empty_input() {
        let err = fix_chromatic_aberration(&[], Settings::default()).unwrap_err();
        assert!(err.contains("empty image input"), "got: {err}");
    }

    #[test]
    fn format_parse_rejects_unknown_and_accepts_aliases() {
        assert_eq!(Format::parse("JPG").unwrap(), Format::Jpeg);
        assert_eq!(Format::parse("auto").unwrap(), Format::Auto);
        let err = Format::parse("tiff").unwrap_err();
        assert!(err.contains("unknown format 'tiff'"), "got: {err}");
    }

    #[test]
    fn settings_are_clamped_into_range() {
        let s = Settings {
            purple_amount: 99,
            green_amount: 99,
            edge_threshold: 900,
            radius: 0,
            hue_tolerance: 1,
            red_shift: 50.0,
            blue_shift: -50.0,
            quality: 0,
            ..Settings::default()
        }
        .clamped();
        assert_eq!(s.purple_amount, 20);
        assert_eq!(s.green_amount, 20);
        assert_eq!(s.edge_threshold, 255);
        assert_eq!(s.radius, 1);
        assert_eq!(s.hue_tolerance, 5);
        assert_eq!(s.red_shift, 10.0);
        assert_eq!(s.blue_shift, -10.0);
        assert_eq!(s.quality, 1);
    }

    #[test]
    fn sliding_max_matches_a_naive_window() {
        let src: Vec<u8> = vec![3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5];
        for radius in [0usize, 1, 2, 4, 20] {
            let mut dst = vec![0u8; src.len()];
            sliding_max_1d(&src, &mut dst, src.len(), 1, 0, radius);
            for i in 0..src.len() {
                let lo = i.saturating_sub(radius);
                let hi = (i + radius).min(src.len() - 1);
                let want = *src[lo..=hi].iter().max().unwrap();
                assert_eq!(dst[i], want, "radius {radius} index {i}");
            }
        }
    }

    #[test]
    fn radius_widens_the_corrected_band_around_an_edge() {
        // A violet band 12 px wide: its own outer boundaries are the only
        // high-contrast edges, so the band's INTERIOR is only reached once the
        // dilation radius grows. (A band narrow enough to be fully covered at
        // radius 1 saturates and measures nothing.)
        let mut img = RgbaImage::from_pixel(40, 4, Rgba([10, 10, 10, 255]));
        for y in 0..4 {
            for x in 20..40 {
                img.put_pixel(x, y, Rgba([240, 240, 240, 255]));
            }
            for x in 20..32 {
                img.put_pixel(x, y, Rgba([150, 60, 190, 255]));
            }
        }
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let bytes = buf.into_inner();
        let tight = fix_chromatic_aberration(
            &bytes,
            Settings {
                radius: 1,
                ..Settings::default()
            },
        )
        .unwrap()
        .1;
        let wide = fix_chromatic_aberration(
            &bytes,
            Settings {
                radius: 20,
                ..Settings::default()
            },
        )
        .unwrap()
        .1;
        assert_eq!(
            wide.purple_pixels, 48,
            "radius 20 reaches every pixel of the 12x4 violet band"
        );
        assert!(
            wide.purple_pixels > tight.purple_pixels,
            "radius 20 ({}) should reach more fringe than radius 1 ({})",
            wide.purple_pixels,
            tight.purple_pixels
        );
    }

    #[test]
    fn webp_output_encodes_and_round_trips() {
        let s = Settings {
            format: Format::WebP,
            ..Settings::default()
        };
        let (out, report) = fix_chromatic_aberration(&fringed_edge(), s).unwrap();
        assert_eq!(report.format, Format::WebP);
        assert_eq!(&out[0..4], b"RIFF");
        assert_eq!(&out[8..12], b"WEBP");
        // WebP here is lossless, so the corrected pixel survives exactly.
        assert_eq!(decode(&out).get_pixel(4, 1).0, [114, 60, 138, 255]);
    }

    #[test]
    fn jpeg_quality_changes_the_encoded_size() {
        let low = fix_chromatic_aberration(
            &fringed_edge(),
            Settings {
                format: Format::Jpeg,
                quality: 10,
                ..Settings::default()
            },
        )
        .unwrap()
        .0;
        let high = fix_chromatic_aberration(
            &fringed_edge(),
            Settings {
                format: Format::Jpeg,
                quality: 100,
                ..Settings::default()
            },
        )
        .unwrap()
        .0;
        assert!(
            high.len() > low.len(),
            "quality 100 ({}) should be larger than quality 10 ({})",
            high.len(),
            low.len()
        );
        // JPEG has no alpha: the result must still decode at the right size.
        assert_eq!(decode(&high).dimensions(), (8, 4));
    }

    #[test]
    fn hue_tolerance_widens_which_hues_count_as_a_fringe() {
        // A magenta-leaning fringe at hue ≈ 320° — 35° from the violet centre,
        // so it is inside a 40° window but outside a 20° one.
        let mut img = RgbaImage::from_pixel(8, 4, Rgba([10, 10, 10, 255]));
        for y in 0..4 {
            img.put_pixel(4, y, Rgba([190, 60, 165, 255]));
            for x in 5..8 {
                img.put_pixel(x, y, Rgba([240, 240, 240, 255]));
            }
        }
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let bytes = buf.into_inner();
        let narrow = fix_chromatic_aberration(
            &bytes,
            Settings {
                hue_tolerance: 20,
                ..Settings::default()
            },
        )
        .unwrap()
        .1;
        let wide = fix_chromatic_aberration(
            &bytes,
            Settings {
                hue_tolerance: 90,
                ..Settings::default()
            },
        )
        .unwrap()
        .1;
        assert_eq!(narrow.purple_pixels, 0, "hue 320° is outside a ±20° window");
        assert!(wide.purple_pixels >= 4, "a ±90° window covers it");
    }

    #[test]
    fn edge_threshold_gates_how_much_is_corrected() {
        // A soft (low-contrast) edge: only a low threshold should qualify it.
        let mut img = RgbaImage::from_pixel(8, 4, Rgba([90, 90, 90, 255]));
        for y in 0..4 {
            img.put_pixel(4, y, Rgba([150, 60, 190, 255]));
            for x in 5..8 {
                img.put_pixel(x, y, Rgba([120, 120, 120, 255]));
            }
        }
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let bytes = buf.into_inner();
        let strict = fix_chromatic_aberration(
            &bytes,
            Settings {
                edge_threshold: 200,
                ..Settings::default()
            },
        )
        .unwrap()
        .1;
        let loose = fix_chromatic_aberration(
            &bytes,
            Settings {
                edge_threshold: 5,
                ..Settings::default()
            },
        )
        .unwrap()
        .1;
        assert_eq!(
            strict.purple_pixels, 0,
            "no edge clears a 200 threshold here"
        );
        assert!(loose.purple_pixels >= 4, "a threshold of 5 does");
    }
}
