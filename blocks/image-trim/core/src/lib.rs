//! gizza-ai/image-trim core — auto-crop uniform borders or whitespace around an
//! image. No wafer/wasm-bindgen deps. Pure-Rust `image` crate, so the block runs
//! on ALL backends including the chat Service Worker.
//!
//! The border ("background") is either the alpha channel (transparent padding),
//! a solid color auto-detected from the 4 corner pixels (ImageMagick `-trim`
//! convention), or a user-specified hex color. Edge rows/columns are removed
//! while at least `background_percent` % of their pixels match the background
//! within `tolerance` (max per-channel distance); `padding` pixels of the
//! ORIGINAL border are kept back around the detected content (clamped to the
//! image edges — no synthetic pixels are invented).

use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Rgba, RgbaImage};

/// Decode-memory budget: input bytes + decoded raster + the RGBA working copy
/// must fit alongside the runtime in the 64 MiB wasm sandbox.
const MEM_BUDGET: u64 = 48 * 1024 * 1024;
/// JPEG re-encode quality for `format = "jpeg"` / JPEG passthrough.
const JPEG_QUALITY: u8 = 90;

/// What counts as "background" while scanning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Background {
    /// Fully/nearly transparent pixels (alpha <= tolerance).
    Alpha,
    /// Pixels within `tolerance` of this opaque RGB color.
    Color([u8; 3]),
}

/// What happened, for the caller's summary + tests.
#[derive(Debug, Clone, PartialEq)]
pub struct TrimReport {
    pub orig_w: u32,
    pub orig_h: u32,
    /// Output dimensions (after padding was added back).
    pub w: u32,
    pub h: u32,
    /// Pixels removed from each side (after padding was kept back).
    pub removed_left: u32,
    pub removed_top: u32,
    pub removed_right: u32,
    pub removed_bottom: u32,
    /// False when no matching border was found (output = input dimensions).
    pub trimmed: bool,
    /// `"transparent"` or `"#rrggbb"` — the background that was trimmed.
    pub background: String,
    /// `"png"` or `"jpeg"` — the encoded output format.
    pub format: &'static str,
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
/// the max per-channel difference over R/G/B plus the distance from fully
/// opaque (a semi-transparent pixel is NOT the solid border color).
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
/// corners -> alpha trim; otherwise the majority corner color (tie -> top-left).
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

fn all_background_err(tol: u8) -> String {
    format!(
        "the whole image matches the background at tolerance {tol} — nothing would remain after trimming. Lower tolerance, raise background_percent, or check mode/color."
    )
}

/// Trim `bytes`. See the crate docs for parameter semantics. Returns the
/// encoded output bytes + a report of what was removed.
pub fn trim(
    bytes: &[u8],
    mode: &str,
    color: Option<&str>,
    tolerance: u64,
    padding: u64,
    background_percent: u64,
    format: &str,
) -> Result<(Vec<u8>, TrimReport), String> {
    if !matches!(mode, "auto" | "transparent" | "color") {
        return Err(format!(
            "invalid mode '{mode}': expected auto, transparent or color"
        ));
    }
    if !matches!(format, "auto" | "png" | "jpeg") {
        return Err(format!(
            "invalid format '{format}': expected auto, png or jpeg"
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
    let in_format = reader.format();
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

    // Scan: rows over the full width, then columns within the surviving rows.
    let mut top = 0u32;
    while top < h && row_is_bg(&img, top, 0, w - 1, bg, tol, bp) {
        top += 1;
    }
    if top == h {
        return Err(all_background_err(tol));
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
        return Err(all_background_err(tol));
    }
    let mut right = w - 1;
    while right > left && col_is_bg(&img, right, top, bottom, bg, tol, bp) {
        right -= 1;
    }

    // Keep `padding` px of the original border back, clamped to the edges.
    let pad = padding as u32;
    let x0 = left.saturating_sub(pad);
    let y0 = top.saturating_sub(pad);
    let x1 = (right + pad).min(w - 1);
    let y1 = (bottom + pad).min(h - 1);
    let (cw, ch) = (x1 - x0 + 1, y1 - y0 + 1);

    let cropped: RgbaImage = if (cw, ch) == (w, h) {
        img
    } else {
        image::imageops::crop_imm(&img, x0, y0, cw, ch).to_image()
    };

    let out_format: &'static str = match format {
        "jpeg" => "jpeg",
        "png" => "png",
        _ => {
            if in_format == Some(ImageFormat::Jpeg) {
                "jpeg"
            } else {
                "png"
            }
        }
    };
    let out = encode(cropped, out_format)?;

    let report = TrimReport {
        orig_w: w,
        orig_h: h,
        w: cw,
        h: ch,
        removed_left: x0,
        removed_top: y0,
        removed_right: w - 1 - x1,
        removed_bottom: h - 1 - y1,
        trimmed: (cw, ch) != (w, h),
        background: match bg {
            Background::Alpha => "transparent".to_string(),
            Background::Color(c) => format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]),
        },
        format: out_format,
    };
    Ok((out, report))
}

/// Encode RGBA as PNG (alpha kept) or JPEG q90 (alpha flattened onto white).
fn encode(img: RgbaImage, format: &'static str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    if format == "jpeg" {
        let (w, h) = img.dimensions();
        let mut rgb = image::RgbImage::new(w, h);
        for (src, dst) in img.pixels().zip(rgb.pixels_mut()) {
            let a = src.0[3] as u32;
            for i in 0..3 {
                dst.0[i] = ((src.0[i] as u32 * a + 255 * (255 - a) + 127) / 255) as u8;
            }
        }
        let mut cursor = Cursor::new(&mut out);
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, JPEG_QUALITY);
        enc.encode_image(&rgb)
            .map_err(|e| format!("could not encode JPEG: {e}"))?;
    } else {
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .map_err(|e| format!("could not encode PNG: {e}"))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn decode(bytes: &[u8]) -> RgbaImage {
        image::load_from_memory(bytes).unwrap().into_rgba8()
    }

    #[test]
    fn trims_white_border_auto() {
        let bytes = png(&framed(WHITE, RED));
        let (out, r) = trim(&bytes, "auto", None, 16, 0, 100, "auto").unwrap();
        assert_eq!((r.orig_w, r.orig_h, r.w, r.h), (12, 10, 4, 3));
        assert_eq!(
            (
                r.removed_left,
                r.removed_top,
                r.removed_right,
                r.removed_bottom
            ),
            (3, 2, 5, 5)
        );
        assert!(r.trimmed);
        assert_eq!(r.background, "#ffffff");
        assert_eq!(r.format, "png");
        let img = decode(&out);
        assert_eq!(img.dimensions(), (4, 3));
        assert!(
            img.pixels().all(|p| *p == RED),
            "output is the pure-red content"
        );
    }

    #[test]
    fn trims_transparent_border_auto() {
        let bytes = png(&framed(CLEAR, RED));
        let (out, r) = trim(&bytes, "auto", None, 16, 0, 100, "auto").unwrap();
        assert_eq!((r.w, r.h), (4, 3));
        assert_eq!(r.background, "transparent");
        assert!(decode(&out).pixels().all(|p| *p == RED));
    }

    #[test]
    fn transparent_mode_explicit() {
        let bytes = png(&framed(CLEAR, WHITE));
        let (_, r) = trim(&bytes, "transparent", None, 16, 0, 100, "auto").unwrap();
        assert_eq!((r.w, r.h), (4, 3));
        assert_eq!(r.background, "transparent");
    }

    #[test]
    fn color_mode_short_and_long_hex_match() {
        let bytes = png(&framed(RED, WHITE));
        let (_, short) = trim(&bytes, "color", Some("#f00"), 16, 0, 100, "auto").unwrap();
        let (_, long) = trim(&bytes, "color", Some("#ff0000"), 16, 0, 100, "auto").unwrap();
        assert_eq!(short, long);
        assert_eq!((short.w, short.h), (4, 3));
        assert_eq!(short.background, "#ff0000");
    }

    #[test]
    fn color_in_auto_mode_overrides_corner_detection() {
        // Corners are red; ask for white explicitly -> nothing matches -> unchanged.
        let bytes = png(&framed(RED, WHITE));
        let (_, r) = trim(&bytes, "auto", Some("#ffffff"), 16, 0, 100, "auto").unwrap();
        assert!(!r.trimmed);
        assert_eq!((r.w, r.h), (12, 10));
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
    fn tolerance_absorbs_near_background_noise() {
        // Border is 250-gray noise on a 255-white background.
        let mut img = framed(WHITE, RED);
        img.put_pixel(1, 1, Rgba([250, 250, 250, 255]));
        img.put_pixel(10, 8, Rgba([250, 250, 250, 255]));
        let bytes = png(&img);
        let (_, loose) = trim(&bytes, "auto", None, 16, 0, 100, "auto").unwrap();
        assert_eq!(
            (loose.w, loose.h),
            (4, 3),
            "tol 16 absorbs the near-white pixels"
        );
        let (_, strict) = trim(&bytes, "auto", None, 0, 0, 100, "auto").unwrap();
        assert_eq!(
            (strict.removed_left, strict.removed_top),
            (1, 1),
            "tol 0 treats 250-gray as content"
        );
    }

    #[test]
    fn background_percent_trims_noisy_rows() {
        // One stray black pixel in the border at (1, 1).
        let mut img = framed(WHITE, RED);
        img.put_pixel(1, 1, Rgba([0, 0, 0, 255]));
        let bytes = png(&img);
        let (_, strict) = trim(&bytes, "auto", None, 16, 0, 100, "auto").unwrap();
        assert_eq!(
            (strict.removed_left, strict.removed_top),
            (1, 1),
            "bp 100 keeps the stray pixel's row/col"
        );
        // Row 1 of 12 px has 11 matching (91.7%); col 1 of rows 1..=4 has 3/4 (75%).
        let (_, loose) = trim(&bytes, "auto", None, 16, 0, 75, "auto").unwrap();
        assert_eq!(
            (loose.w, loose.h),
            (4, 3),
            "bp 75 trims through the stray pixel"
        );
    }

    #[test]
    fn padding_kept_and_clamped() {
        let bytes = png(&framed(WHITE, RED));
        let (_, r) = trim(&bytes, "auto", None, 16, 2, 100, "auto").unwrap();
        assert_eq!((r.w, r.h), (8, 7));
        assert_eq!(
            (
                r.removed_left,
                r.removed_top,
                r.removed_right,
                r.removed_bottom
            ),
            (1, 0, 3, 3),
            "2px of the original border kept; top clamped at the edge"
        );
        let (_, all) = trim(&bytes, "auto", None, 16, 500, 100, "auto").unwrap();
        assert!(!all.trimmed, "padding 500 swallows the whole trim");
        assert_eq!((all.w, all.h), (12, 10));
    }

    #[test]
    fn no_matching_border_returns_unchanged() {
        // Four distinct corner colors: the vote picks top-left (red), but row 0
        // holds other colors too, so nothing is trimmed.
        let mut img = RgbaImage::from_pixel(8, 6, Rgba([10, 10, 10, 255]));
        img.put_pixel(0, 0, RED);
        img.put_pixel(7, 0, Rgba([0, 255, 0, 255]));
        img.put_pixel(0, 5, Rgba([0, 0, 255, 255]));
        img.put_pixel(7, 5, Rgba([255, 255, 0, 255]));
        let (out, r) = trim(&png(&img), "auto", None, 16, 0, 100, "auto").unwrap();
        assert!(!r.trimmed);
        assert_eq!((r.w, r.h), (8, 6));
        assert_eq!(
            (
                r.removed_left,
                r.removed_top,
                r.removed_right,
                r.removed_bottom
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(decode(&out).dimensions(), (8, 6));
    }

    #[test]
    fn semi_transparent_pixels_are_content_in_color_mode() {
        // White-ish pixels at alpha 200 differ from the OPAQUE white border by
        // 255-200=55 > tol, so they survive as content.
        let mut img = RgbaImage::from_pixel(12, 10, WHITE);
        for y in 2..=4 {
            for x in 3..=6 {
                img.put_pixel(x, y, Rgba([255, 255, 255, 200]));
            }
        }
        let (_, r) = trim(&png(&img), "color", Some("#fff"), 16, 0, 100, "auto").unwrap();
        assert_eq!((r.w, r.h), (4, 3));
    }

    #[test]
    fn whole_image_background_errors() {
        let bytes = png(&RgbaImage::from_pixel(6, 6, WHITE));
        let err = trim(&bytes, "auto", None, 16, 0, 100, "auto").unwrap_err();
        assert!(err.contains("nothing would remain"), "got: {err}");
    }

    #[test]
    fn mode_color_requires_color() {
        let bytes = png(&framed(WHITE, RED));
        let err = trim(&bytes, "color", None, 16, 0, 100, "auto").unwrap_err();
        assert!(err.contains("requires the color parameter"), "got: {err}");
    }

    #[test]
    fn transparent_mode_rejects_color() {
        let bytes = png(&framed(CLEAR, RED));
        let err = trim(&bytes, "transparent", Some("#fff"), 16, 0, 100, "auto").unwrap_err();
        assert!(err.contains("not used with mode=transparent"), "got: {err}");
    }

    #[test]
    fn rejects_bad_ranges_and_values() {
        let bytes = png(&framed(WHITE, RED));
        assert!(trim(&bytes, "magic", None, 16, 0, 100, "auto").is_err());
        assert!(trim(&bytes, "auto", None, 256, 0, 100, "auto").is_err());
        assert!(trim(&bytes, "auto", None, 16, 501, 100, "auto").is_err());
        assert!(trim(&bytes, "auto", None, 16, 0, 49, "auto").is_err());
        assert!(trim(&bytes, "auto", None, 16, 0, 101, "auto").is_err());
        assert!(trim(&bytes, "auto", None, 16, 0, 100, "gif").is_err());
        assert!(trim(&bytes, "color", Some("teal"), 16, 0, 100, "auto").is_err());
        assert!(trim(b"not an image", "auto", None, 16, 0, 100, "auto").is_err());
    }

    #[test]
    fn jpeg_in_jpeg_out_with_format_auto() {
        // JPEG input (white border, red block) -> auto keeps JPEG; png forces PNG.
        let img = framed(WHITE, RED);
        let mut jpg = Vec::new();
        let rgb = DynamicImage::ImageRgba8(img).to_rgb8();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut Cursor::new(&mut jpg), 95)
            .encode_image(&rgb)
            .unwrap();
        let (out, r) = trim(&jpg, "auto", None, 24, 0, 100, "auto").unwrap();
        assert_eq!(r.format, "jpeg");
        assert_eq!(&out[..2], &[0xff, 0xd8], "JPEG magic");
        assert!(r.trimmed);
        assert!(
            r.w <= 6 && r.h <= 5,
            "JPEG artifacts stay near the content box: {r:?}"
        );
        let (out2, r2) = trim(&jpg, "auto", None, 24, 0, 100, "png").unwrap();
        assert_eq!(r2.format, "png");
        assert_eq!(&out2[..4], b"\x89PNG");
    }

    #[test]
    fn jpeg_output_flattens_alpha_onto_white() {
        // Red content on a transparent border, forced to JPEG: border flattens
        // to white, so the decoded corner is near-white.
        let bytes = png(&framed(CLEAR, RED));
        let (out, r) = trim(&bytes, "transparent", None, 16, 1, 100, "jpeg").unwrap();
        assert_eq!(r.format, "jpeg");
        let img = decode(&out);
        let corner = img.get_pixel(0, 0);
        assert!(
            corner.0[0] > 220 && corner.0[1] > 220 && corner.0[2] > 220,
            "{corner:?}"
        );
    }

    #[test]
    fn rejects_raster_over_memory_budget() {
        // 5000x5000 RGBA = 100 MB raster > the 48 MB budget; constant color
        // compresses to a tiny PNG, so only the header check can catch it.
        let bytes = png(&RgbaImage::from_pixel(5000, 5000, WHITE));
        let err = trim(&bytes, "auto", None, 16, 0, 100, "auto").unwrap_err();
        assert!(err.contains("too large"), "got: {err}");
    }
}
