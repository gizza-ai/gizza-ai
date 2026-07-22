//! gizza-ai/content-trim-bounds-detector core — find the tight bounding box of
//! the non-background / non-transparent content in an image and report the crop
//! (and per-side trim margins) that would remove the surrounding empty margin.
//!
//! No wafer/wasm-bindgen deps. Pure-Rust `image` crate, so the block runs on ALL
//! backends including the chat Service Worker. This tool MEASURES only — it never
//! re-encodes or crops the image (the sibling `image-trim` tool does that).
//!
//! The "background" is either the alpha channel (transparent padding), a solid
//! color auto-detected from the 4 corner pixels (ImageMagick `-trim` convention),
//! or a user-specified hex color. Edge rows/columns count as background while at
//! least `background_percent` % of their pixels match the background within
//! `tolerance` (max per-channel distance). `padding` px of the original border can
//! be kept back around the detected content in the suggested crop (clamped to the
//! image edges — no synthetic pixels are invented).

use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageReader, Rgba, RgbaImage};

/// Decode-memory budget: input bytes + decoded raster + the RGBA working copy
/// must fit alongside the runtime in the 64 MiB wasm sandbox.
const MEM_BUDGET: u64 = 48 * 1024 * 1024;

/// What counts as "background" while scanning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Background {
    /// Fully/nearly transparent pixels (alpha <= tolerance).
    Alpha,
    /// Pixels within `tolerance` of this opaque RGB color.
    Color([u8; 3]),
}

/// The measured content-bounds report.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundsReport {
    /// Original image dimensions.
    pub orig_w: u32,
    pub orig_h: u32,
    /// True when any non-background content was found. False = whole image is
    /// background (all margins 0, content box equals the full image).
    pub has_content: bool,
    /// Tight content bounding box (before `padding`): top-left corner + size.
    pub content_x: u32,
    pub content_y: u32,
    pub content_width: u32,
    pub content_height: u32,
    /// Suggested crop box after keeping `padding` px of border back (clamped).
    pub crop_x: u32,
    pub crop_y: u32,
    pub crop_width: u32,
    pub crop_height: u32,
    /// Pixels of background to trim from each side to reach the suggested crop.
    pub trim_left: u32,
    pub trim_top: u32,
    pub trim_right: u32,
    pub trim_bottom: u32,
    /// True when any suggested trim margin > 0 (i.e. there is margin to remove).
    pub needs_trim: bool,
    /// Fraction 0..1 of the image area occupied by the tight content box.
    pub content_fraction: f64,
    /// `"transparent"` or `"#rrggbb"` — the background that was detected.
    pub background: String,
}

/// Parse `#rgb` / `#rrggbb` (case-insensitive, `#` optional) into RGB.
pub fn parse_hex(s: &str) -> Result<[u8; 3], String> {
    let t = s.trim().trim_start_matches('#');
    let err = || format!("invalid color '{s}': expected hex like #fff or #ffffff");
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(err());
    }
    match t.len() {
        3 => {
            let mut c = [0u8; 3];
            for (i, ch) in t.chars().enumerate() {
                let v = ch.to_digit(16).unwrap() as u8;
                c[i] = v * 16 + v; // f -> ff
            }
            Ok(c)
        }
        6 => {
            let mut c = [0u8; 3];
            for (i, item) in c.iter_mut().enumerate() {
                *item = u8::from_str_radix(&t[2 * i..2 * i + 2], 16).map_err(|_| err())?;
            }
            Ok(c)
        }
        _ => Err(err()),
    }
}

/// True when `p` counts as background within `tol`. For colors, the distance is
/// the max per-channel difference over R/G/B plus the distance from fully opaque
/// (a semi-transparent pixel is NOT the solid border color).
fn is_bg(p: &Rgba<u8>, bg: Background, tol: u8) -> bool {
    match bg {
        Background::Alpha => p.0[3] <= tol,
        Background::Color(c) => {
            let d = (0..3)
                .map(|i| (p.0[i] as i32 - c[i] as i32).unsigned_abs())
                .max()
                .unwrap();
            let da = 255 - p.0[3] as u32;
            d.max(da) <= tol as u32
        }
    }
}

fn channel_dist(a: [u8; 3], b: [u8; 3]) -> u32 {
    (0..3)
        .map(|i| (a[i] as i32 - b[i] as i32).unsigned_abs())
        .max()
        .unwrap()
}

/// Auto-detect the background from the 4 corner pixels: mostly-transparent
/// corners -> alpha; otherwise the majority corner color (tie -> top-left).
fn detect_background(img: &RgbaImage, tol: u8) -> Background {
    let (w, h) = img.dimensions();
    let corners = [
        *img.get_pixel(0, 0),
        *img.get_pixel(w - 1, 0),
        *img.get_pixel(0, h - 1),
        *img.get_pixel(w - 1, h - 1),
    ];
    let transparent = corners.iter().filter(|p| p.0[3] <= tol).count();
    if transparent >= 3 {
        return Background::Alpha;
    }
    let opaque: Vec<[u8; 3]> = corners
        .iter()
        .filter(|p| p.0[3] > tol)
        .map(|p| [p.0[0], p.0[1], p.0[2]])
        .collect();
    if opaque.is_empty() {
        return Background::Alpha;
    }
    let mut best = 0usize;
    let mut best_votes = 0usize;
    for (i, c) in opaque.iter().enumerate() {
        let votes = opaque
            .iter()
            .filter(|o| channel_dist(*c, **o) <= tol as u32)
            .count();
        if votes > best_votes {
            best = i;
            best_votes = votes;
        }
    }
    Background::Color(opaque[best])
}

/// Is row `y` (over columns x0..=x1) a background row? A row counts while at
/// least `bp` % of its pixels match (bp = 100 -> every pixel must match).
fn row_is_bg(img: &RgbaImage, y: u32, x0: u32, x1: u32, bg: Background, tol: u8, bp: u32) -> bool {
    let total = (x1 - x0 + 1) as u64;
    let matching = (x0..=x1)
        .filter(|&x| is_bg(img.get_pixel(x, y), bg, tol))
        .count() as u64;
    matching * 100 >= bp as u64 * total
}

/// Is column `x` (over rows y0..=y1) a background column?
fn col_is_bg(img: &RgbaImage, x: u32, y0: u32, y1: u32, bg: Background, tol: u8, bp: u32) -> bool {
    let total = (y1 - y0 + 1) as u64;
    let matching = (y0..=y1)
        .filter(|&y| is_bg(img.get_pixel(x, y), bg, tol))
        .count() as u64;
    matching * 100 >= bp as u64 * total
}

/// Detect the content bounds of `bytes`. See the crate docs for parameter
/// semantics. Returns a report of the tight content box, the suggested crop
/// (after `padding`), and the per-side trim margins.
pub fn detect(
    bytes: &[u8],
    mode: &str,
    color: Option<&str>,
    tolerance: u64,
    background_percent: u64,
    padding: u64,
) -> Result<BoundsReport, String> {
    if !matches!(mode, "auto" | "transparent" | "color") {
        return Err(format!(
            "invalid mode '{mode}': expected auto, transparent or color"
        ));
    }
    if tolerance > 255 {
        return Err(format!("tolerance must be 0-255 (got {tolerance})"));
    }
    if padding > 500 {
        return Err(format!("padding must be 0-500 pixels (got {padding})"));
    }
    if !(50..=100).contains(&background_percent) {
        return Err(format!(
            "background_percent must be 50-100 (got {background_percent})"
        ));
    }
    let tol = tolerance as u8;
    let bp = background_percent as u32;

    // Header-first decode budget: reject oversized rasters with an actionable
    // error instead of an OOM trap in the 64 MiB sandbox.
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("could not read image: {e}"))?;
    let decoder = reader
        .into_decoder()
        .map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }
    let raster = decoder.total_bytes();
    let rgba_copy = w as u64 * h as u64 * 4;
    if bytes.len() as u64 + raster + rgba_copy > MEM_BUDGET {
        return Err(format!(
            "image too large to process here ({w}x{h}): the decoded pixels exceed the ~48 MB sandbox budget — re-export at a lower resolution and retry"
        ));
    }
    let img: RgbaImage = DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("could not decode image: {e}"))?
        .into_rgba8();

    // Resolve the background.
    let bg = match mode {
        "transparent" => {
            if color.is_some() {
                return Err(
                    "color is not used with mode=transparent — remove it, or use mode=color".into(),
                );
            }
            Background::Alpha
        }
        "color" => Background::Color(parse_hex(
            color.ok_or("mode=color requires the color parameter (hex like #fff or #ffffff)")?,
        )?),
        _ => match color {
            Some(c) => Background::Color(parse_hex(c)?),
            None => detect_background(&img, tol),
        },
    };

    let background = match bg {
        Background::Alpha => "transparent".to_string(),
        Background::Color(c) => format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]),
    };

    // Scan: rows over the full width, then columns within the surviving rows.
    let mut top = 0u32;
    while top < h && row_is_bg(&img, top, 0, w - 1, bg, tol, bp) {
        top += 1;
    }
    if top == h {
        // Entire image is background — nothing to keep. Report "no content"
        // rather than erroring: this is a measurement tool.
        return Ok(no_content_report(w, h, background));
    }
    let mut bottom = h - 1;
    while bottom > top && row_is_bg(&img, bottom, 0, w - 1, bg, tol, bp) {
        bottom -= 1;
    }
    let mut left = 0u32;
    while left < w && col_is_bg(&img, left, top, bottom, bg, tol, bp) {
        left += 1;
    }
    if left == w {
        // Only reachable with background_percent < 100.
        return Ok(no_content_report(w, h, background));
    }
    let mut right = w - 1;
    while right > left && col_is_bg(&img, right, top, bottom, bg, tol, bp) {
        right -= 1;
    }

    // Tight content box.
    let content_w = right - left + 1;
    let content_h = bottom - top + 1;

    // Suggested crop keeps `padding` px of the original border back, clamped.
    let pad = padding as u32;
    let x0 = left.saturating_sub(pad);
    let y0 = top.saturating_sub(pad);
    let x1 = (right + pad).min(w - 1);
    let y1 = (bottom + pad).min(h - 1);
    let crop_w = x1 - x0 + 1;
    let crop_h = y1 - y0 + 1;

    let trim_left = x0;
    let trim_top = y0;
    let trim_right = w - 1 - x1;
    let trim_bottom = h - 1 - y1;

    let content_fraction =
        ((content_w as f64 * content_h as f64) / (w as f64 * h as f64) * 10000.0).round() / 10000.0;

    Ok(BoundsReport {
        orig_w: w,
        orig_h: h,
        has_content: true,
        content_x: left,
        content_y: top,
        content_width: content_w,
        content_height: content_h,
        crop_x: x0,
        crop_y: y0,
        crop_width: crop_w,
        crop_height: crop_h,
        trim_left,
        trim_top,
        trim_right,
        trim_bottom,
        needs_trim: trim_left + trim_top + trim_right + trim_bottom > 0,
        content_fraction,
        background,
    })
}

/// The "whole image is background" report: content box = full image, no trim.
fn no_content_report(w: u32, h: u32, background: String) -> BoundsReport {
    BoundsReport {
        orig_w: w,
        orig_h: h,
        has_content: false,
        content_x: 0,
        content_y: 0,
        content_width: w,
        content_height: h,
        crop_x: 0,
        crop_y: 0,
        crop_width: w,
        crop_height: h,
        trim_left: 0,
        trim_top: 0,
        trim_right: 0,
        trim_bottom: 0,
        needs_trim: false,
        content_fraction: 0.0,
        background,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageFormat;

    const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);
    const RED: Rgba<u8> = Rgba([255, 0, 0, 255]);
    const CLEAR: Rgba<u8> = Rgba([0, 0, 0, 0]);

    /// 12x10 `fill` background with a `content` rect at x 3..=6, y 2..=4.
    fn framed(fill: Rgba<u8>, content: Rgba<u8>) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(12, 10, fill);
        for y in 2..=4 {
            for x in 3..=6 {
                img.put_pixel(x, y, content);
            }
        }
        img
    }

    fn png(img: &RgbaImage) -> Vec<u8> {
        let mut out = Vec::new();
        DynamicImage::ImageRgba8(img.clone())
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn finds_content_box_on_white_border_auto() {
        let bytes = png(&framed(WHITE, RED));
        let r = detect(&bytes, "auto", None, 16, 100, 0).unwrap();
        assert!(r.has_content);
        assert_eq!((r.orig_w, r.orig_h), (12, 10));
        // content rect is x 3..=6 (w 4), y 2..=4 (h 3).
        assert_eq!(
            (r.content_x, r.content_y, r.content_width, r.content_height),
            (3, 2, 4, 3)
        );
        // suggested crop with padding 0 == tight box.
        assert_eq!(
            (r.crop_x, r.crop_y, r.crop_width, r.crop_height),
            (3, 2, 4, 3)
        );
        // margins: 3 left, 2 top, 5 right (12-1-6), 5 bottom (10-1-4).
        assert_eq!(
            (r.trim_left, r.trim_top, r.trim_right, r.trim_bottom),
            (3, 2, 5, 5)
        );
        assert!(r.needs_trim);
        assert_eq!(r.background, "#ffffff");
        // 12 / 120 = 0.1 coverage.
        assert_eq!(r.content_fraction, 0.1);
    }

    #[test]
    fn finds_content_box_on_transparent_border_auto() {
        let bytes = png(&framed(CLEAR, RED));
        let r = detect(&bytes, "auto", None, 16, 100, 0).unwrap();
        assert!(r.has_content);
        assert_eq!(
            (r.content_x, r.content_y, r.content_width, r.content_height),
            (3, 2, 4, 3)
        );
        assert_eq!(r.background, "transparent");
    }

    #[test]
    fn transparent_mode_explicit() {
        let bytes = png(&framed(CLEAR, WHITE));
        let r = detect(&bytes, "transparent", None, 16, 100, 0).unwrap();
        assert_eq!((r.content_width, r.content_height), (4, 3));
        assert_eq!(r.background, "transparent");
    }

    #[test]
    fn color_mode_short_and_long_hex_match() {
        let bytes = png(&framed(RED, WHITE));
        let short = detect(&bytes, "color", Some("#f00"), 16, 100, 0).unwrap();
        let long = detect(&bytes, "color", Some("#ff0000"), 16, 100, 0).unwrap();
        assert_eq!(short, long);
        assert_eq!((short.content_width, short.content_height), (4, 3));
        assert_eq!(short.background, "#ff0000");
    }

    #[test]
    fn color_in_auto_mode_overrides_corner_detection() {
        // Corners are red; ask for white explicitly -> nothing matches -> no margin.
        let bytes = png(&framed(RED, WHITE));
        let r = detect(&bytes, "auto", Some("#ffffff"), 16, 100, 0).unwrap();
        assert!(r.has_content);
        assert!(!r.needs_trim);
        assert_eq!((r.content_width, r.content_height), (12, 10));
        assert_eq!(r.background, "#ffffff");
    }

    #[test]
    fn padding_kept_and_clamped() {
        let bytes = png(&framed(WHITE, RED));
        let r = detect(&bytes, "auto", None, 16, 100, 2).unwrap();
        // tight box stays reported.
        assert_eq!(
            (r.content_x, r.content_y, r.content_width, r.content_height),
            (3, 2, 4, 3)
        );
        // crop grows by 2px each side, clamped at the top edge (top was 2).
        assert_eq!(
            (r.crop_x, r.crop_y, r.crop_width, r.crop_height),
            (1, 0, 8, 7)
        );
        assert_eq!(
            (r.trim_left, r.trim_top, r.trim_right, r.trim_bottom),
            (1, 0, 3, 3)
        );
        // padding big enough to swallow all margin.
        let all = detect(&bytes, "auto", None, 16, 100, 500).unwrap();
        assert!(!all.needs_trim);
        assert_eq!((all.crop_width, all.crop_height), (12, 10));
    }

    #[test]
    fn tolerance_absorbs_near_background_noise() {
        // Border is 250-gray noise on a 255-white background.
        let mut img = framed(WHITE, RED);
        img.put_pixel(1, 1, Rgba([250, 250, 250, 255]));
        img.put_pixel(10, 8, Rgba([250, 250, 250, 255]));
        let bytes = png(&img);
        let loose = detect(&bytes, "auto", None, 16, 100, 0).unwrap();
        assert_eq!((loose.content_width, loose.content_height), (4, 3));
        let strict = detect(&bytes, "auto", None, 0, 100, 0).unwrap();
        assert_eq!((strict.content_x, strict.content_y), (1, 1));
    }

    #[test]
    fn background_percent_trims_noisy_rows() {
        // One stray black pixel in the border at (1, 1).
        let mut img = framed(WHITE, RED);
        img.put_pixel(1, 1, Rgba([0, 0, 0, 255]));
        let bytes = png(&img);
        let strict = detect(&bytes, "auto", None, 16, 100, 0).unwrap();
        assert_eq!((strict.content_x, strict.content_y), (1, 1));
        let loose = detect(&bytes, "auto", None, 16, 75, 0).unwrap();
        assert_eq!((loose.content_width, loose.content_height), (4, 3));
    }

    #[test]
    fn whole_image_background_reports_no_content() {
        let bytes = png(&RgbaImage::from_pixel(6, 6, WHITE));
        let r = detect(&bytes, "auto", None, 16, 100, 0).unwrap();
        assert!(!r.has_content);
        assert!(!r.needs_trim);
        assert_eq!((r.content_width, r.content_height), (6, 6));
        assert_eq!(r.content_fraction, 0.0);
        assert_eq!(r.background, "#ffffff");
    }

    #[test]
    fn parse_hex_forms() {
        assert_eq!(parse_hex("#fff").unwrap(), [255, 255, 255]);
        assert_eq!(parse_hex("F00").unwrap(), [255, 0, 0]);
        assert_eq!(parse_hex("#1a2B3c").unwrap(), [26, 43, 60]);
        assert!(parse_hex("#ff00").is_err());
        assert!(parse_hex("red").is_err());
        assert!(parse_hex("").is_err());
    }

    #[test]
    fn mode_color_requires_color() {
        let bytes = png(&framed(WHITE, RED));
        let err = detect(&bytes, "color", None, 16, 100, 0).unwrap_err();
        assert!(err.contains("requires the color parameter"), "got: {err}");
    }

    #[test]
    fn transparent_mode_rejects_color() {
        let bytes = png(&framed(CLEAR, RED));
        let err = detect(&bytes, "transparent", Some("#fff"), 16, 100, 0).unwrap_err();
        assert!(err.contains("not used with mode=transparent"), "got: {err}");
    }

    #[test]
    fn rejects_bad_ranges_and_values() {
        let bytes = png(&framed(WHITE, RED));
        assert!(detect(&bytes, "magic", None, 16, 100, 0).is_err());
        assert!(detect(&bytes, "auto", None, 256, 100, 0).is_err());
        assert!(detect(&bytes, "auto", None, 16, 100, 501).is_err());
        assert!(detect(&bytes, "auto", None, 16, 49, 0).is_err());
        assert!(detect(&bytes, "auto", None, 16, 101, 0).is_err());
        assert!(detect(&bytes, "color", Some("teal"), 16, 100, 0).is_err());
        assert!(detect(b"not an image", "auto", None, 16, 100, 0).is_err());
    }

    #[test]
    fn rejects_raster_over_memory_budget() {
        // 5000x5000 RGBA = 100 MB raster > the 48 MB budget; constant color
        // compresses to a tiny PNG, so only the header check can catch it.
        let bytes = png(&RgbaImage::from_pixel(5000, 5000, WHITE));
        let err = detect(&bytes, "auto", None, 16, 100, 0).unwrap_err();
        assert!(err.contains("too large"), "got: {err}");
    }
}
