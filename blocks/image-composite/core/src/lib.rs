//! image-composite core — overlay one image (the foreground/overlay) onto another
//! (the background/base) with position, scale, opacity, and a Photoshop-style blend
//! mode. Pure `image` crate; no wafer/wasm-bindgen deps.
//!
//! Pipeline: image A (base) defines the output canvas at its native size (pixel
//! capped). Image B (overlay) is scaled by `scale` percent of its native size,
//! optionally flipped, positioned at an anchor plus a pixel offset, and clipped to
//! the canvas. Each overlapping pixel uses the W3C separable "blend then
//! source-over" formula so transparent overlay pixels and a transparent base both
//! composite correctly and `opacity` scales the overlay's contribution.

use std::io::Cursor;

use image::{imageops::FilterType, DynamicImage, ImageFormat, Rgba, RgbaImage};

/// Separable per-channel blend function applied to the overlapping region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Add,
}

pub fn parse_blend_mode(s: &str) -> Result<BlendMode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "normal" | "source-over" => Ok(BlendMode::Normal),
        "multiply" => Ok(BlendMode::Multiply),
        "screen" => Ok(BlendMode::Screen),
        "overlay" => Ok(BlendMode::Overlay),
        "darken" => Ok(BlendMode::Darken),
        "lighten" => Ok(BlendMode::Lighten),
        "hard-light" | "hardlight" | "hard_light" => Ok(BlendMode::HardLight),
        "soft-light" | "softlight" | "soft_light" => Ok(BlendMode::SoftLight),
        "difference" | "diff" => Ok(BlendMode::Difference),
        "exclusion" => Ok(BlendMode::Exclusion),
        "add" | "linear-dodge" | "linear_dodge" | "plus" => Ok(BlendMode::Add),
        other => Err(format!(
            "blend_mode {other:?} not supported (normal|multiply|screen|overlay|darken|lighten|hard-light|soft-light|difference|exclusion|add)"
        )),
    }
}

/// Overlay placement anchor on the base canvas (before the pixel offset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Center,
    TopLeft,
    Top,
    TopRight,
    Left,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

pub fn parse_position(s: &str) -> Result<Position, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "center" | "centre" | "middle" => Ok(Position::Center),
        "top-left" | "top_left" | "topleft" => Ok(Position::TopLeft),
        "top" | "top-center" => Ok(Position::Top),
        "top-right" | "top_right" | "topright" => Ok(Position::TopRight),
        "left" => Ok(Position::Left),
        "right" => Ok(Position::Right),
        "bottom-left" | "bottom_left" | "bottomleft" => Ok(Position::BottomLeft),
        "bottom" | "bottom-center" => Ok(Position::Bottom),
        "bottom-right" | "bottom_right" | "bottomright" => Ok(Position::BottomRight),
        other => Err(format!(
            "position {other:?} not supported (center|top-left|top|top-right|left|right|bottom-left|bottom|bottom-right)"
        )),
    }
}

/// Optional flip applied to the overlay before compositing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flip {
    None,
    Horizontal,
    Vertical,
    Both,
}

pub fn parse_flip(s: &str) -> Result<Flip, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "none" => Ok(Flip::None),
        "horizontal" | "h" | "hflip" => Ok(Flip::Horizontal),
        "vertical" | "v" | "vflip" => Ok(Flip::Vertical),
        "both" | "hv" => Ok(Flip::Both),
        other => Err(format!("flip {other:?} not supported (none|horizontal|vertical|both)")),
    }
}

/// Output encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutFormat {
    Png,
    Jpeg,
}

pub fn parse_format(s: &str) -> Result<OutFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "png" => Ok(OutFormat::Png),
        "jpeg" | "jpg" => Ok(OutFormat::Jpeg),
        other => Err(format!("format {other:?} not supported (png|jpeg)")),
    }
}

impl OutFormat {
    pub fn mime(self) -> &'static str {
        match self {
            OutFormat::Png => "image/png",
            OutFormat::Jpeg => "image/jpeg",
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            OutFormat::Png => "png",
            OutFormat::Jpeg => "jpg",
        }
    }
}

const MAX_DIM: u32 = 5000;
const MAX_PIXELS: u64 = 40_000_000; // ~40 MP output guard

/// One separable blend channel in normalized 0..=1 space (cb = base, cs = overlay).
fn blend_channel(mode: BlendMode, cb: f32, cs: f32) -> f32 {
    match mode {
        BlendMode::Normal => cs,
        BlendMode::Multiply => cb * cs,
        BlendMode::Screen => cb + cs - cb * cs,
        BlendMode::Overlay => hard_light(cs, cb), // overlay = hard-light with roles swapped
        BlendMode::Darken => cb.min(cs),
        BlendMode::Lighten => cb.max(cs),
        BlendMode::HardLight => hard_light(cb, cs),
        BlendMode::SoftLight => soft_light(cb, cs),
        BlendMode::Difference => (cb - cs).abs(),
        BlendMode::Exclusion => cb + cs - 2.0 * cb * cs,
        BlendMode::Add => (cb + cs).min(1.0),
    }
}

fn hard_light(cb: f32, cs: f32) -> f32 {
    if cs <= 0.5 {
        2.0 * cb * cs
    } else {
        1.0 - 2.0 * (1.0 - cb) * (1.0 - cs)
    }
}

fn soft_light(cb: f32, cs: f32) -> f32 {
    // W3C compositing soft-light.
    if cs <= 0.5 {
        cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
    } else {
        let d = if cb <= 0.25 {
            ((16.0 * cb - 12.0) * cb + 4.0) * cb
        } else {
            cb.sqrt()
        };
        cb + (2.0 * cs - 1.0) * (d - cb)
    }
}

/// Composite overlay pixel `top` onto base pixel `base` using `mode` and an extra
/// `opacity` multiplier on the overlay's alpha. Straight (non-premultiplied) RGBA in,
/// straight RGBA out (W3C blend-then-source-over).
fn composite_pixel(mode: BlendMode, base: Rgba<u8>, top: Rgba<u8>, opacity: f32) -> Rgba<u8> {
    let ab = base[3] as f32 / 255.0;
    let asrc = (top[3] as f32 / 255.0) * opacity;
    if asrc <= 0.0 {
        return base;
    }
    let ao = asrc + ab * (1.0 - asrc);
    if ao <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let ch = |i: usize| -> u8 {
        let cb = base[i] as f32 / 255.0;
        let cs = top[i] as f32 / 255.0;
        // Blended color the overlay contributes, accounting for the base's own alpha.
        let blended = (1.0 - ab) * cs + ab * blend_channel(mode, cb, cs);
        let co = (asrc * blended + ab * (1.0 - asrc) * cb) / ao;
        (co * 255.0).round().clamp(0.0, 255.0) as u8
    };
    Rgba([ch(0), ch(1), ch(2), (ao * 255.0).round().clamp(0.0, 255.0) as u8])
}

/// Compute the capped canvas size from the base image's native dimensions.
fn canvas_size(aw: u32, ah: u32) -> (u32, u32) {
    let mut scale = 1.0f64;
    if aw > MAX_DIM {
        scale = scale.min(MAX_DIM as f64 / aw as f64);
    }
    if ah > MAX_DIM {
        scale = scale.min(MAX_DIM as f64 / ah as f64);
    }
    let px = aw as u64 * ah as u64;
    if px > MAX_PIXELS {
        scale = scale.min((MAX_PIXELS as f64 / px as f64).sqrt());
    }
    let w = ((aw as f64 * scale) as u32).max(1);
    let h = ((ah as f64 * scale) as u32).max(1);
    (w, h)
}

/// Top-left origin of the overlay on the canvas for a given anchor (before offset).
fn anchor_origin(pos: Position, cw: u32, ch: u32, ow: u32, oh: u32) -> (i64, i64) {
    let (cw, ch, ow, oh) = (cw as i64, ch as i64, ow as i64, oh as i64);
    let (x, y) = match pos {
        Position::TopLeft => (0, 0),
        Position::Top => ((cw - ow) / 2, 0),
        Position::TopRight => (cw - ow, 0),
        Position::Left => (0, (ch - oh) / 2),
        Position::Center => ((cw - ow) / 2, (ch - oh) / 2),
        Position::Right => (cw - ow, (ch - oh) / 2),
        Position::BottomLeft => (0, ch - oh),
        Position::Bottom => ((cw - ow) / 2, ch - oh),
        Position::BottomRight => (cw - ow, ch - oh),
    };
    (x, y)
}

/// Composite two decoded images into one.
///
/// * `base` — background; defines the output canvas (native size, pixel capped).
/// * `overlay` — foreground; scaled by `scale` percent of its native size (1..=1000),
///   optionally flipped, placed at `position` + (`offset_x`, `offset_y`) pixels.
/// * `opacity` — 0..=1 multiplier on the overlay's alpha.
#[allow(clippy::too_many_arguments)]
pub fn composite(
    base: DynamicImage,
    overlay: DynamicImage,
    blend: BlendMode,
    opacity: f64,
    scale: f64,
    position: Position,
    offset_x: i64,
    offset_y: i64,
    flip: Flip,
    format: OutFormat,
) -> Result<Vec<u8>, String> {
    let (cw, ch) = canvas_size(base.width().max(1), base.height().max(1));
    let mut canvas = if base.width() == cw && base.height() == ch {
        base.to_rgba8()
    } else {
        base.resize_exact(cw, ch, FilterType::Lanczos3).to_rgba8()
    };

    let opacity = opacity.clamp(0.0, 1.0) as f32;
    let scale = scale.clamp(1.0, 1000.0) / 100.0;

    // Scale the overlay by percent of its native size, capped to the canvas guards.
    let ow0 = overlay.width().max(1);
    let oh0 = overlay.height().max(1);
    let mut ow = ((ow0 as f64 * scale) as u32).max(1);
    let mut oh = ((oh0 as f64 * scale) as u32).max(1);
    if ow > MAX_DIM || oh > MAX_DIM {
        let s = (MAX_DIM as f64 / ow.max(oh) as f64).min(1.0);
        ow = ((ow as f64 * s) as u32).max(1);
        oh = ((oh as f64 * s) as u32).max(1);
    }
    let mut overlay = if ow == ow0 && oh == oh0 {
        overlay.to_rgba8()
    } else {
        overlay.resize_exact(ow, oh, FilterType::Lanczos3).to_rgba8()
    };
    match flip {
        Flip::None => {}
        Flip::Horizontal => image::imageops::flip_horizontal_in_place(&mut overlay),
        Flip::Vertical => image::imageops::flip_vertical_in_place(&mut overlay),
        Flip::Both => {
            image::imageops::flip_horizontal_in_place(&mut overlay);
            image::imageops::flip_vertical_in_place(&mut overlay);
        }
    }

    let (ax, ay) = anchor_origin(position, cw, ch, ow, oh);
    let ox = ax + offset_x;
    let oy = ay + offset_y;

    for by in 0..oh {
        let cy = oy + by as i64;
        if cy < 0 || cy >= ch as i64 {
            continue;
        }
        for bx in 0..ow {
            let cx = ox + bx as i64;
            if cx < 0 || cx >= cw as i64 {
                continue;
            }
            let base_px = *canvas.get_pixel(cx as u32, cy as u32);
            let top_px = *overlay.get_pixel(bx, by);
            let out = composite_pixel(blend, base_px, top_px, opacity);
            canvas.put_pixel(cx as u32, cy as u32, out);
        }
    }

    encode(canvas, format)
}

fn encode(img: RgbaImage, format: OutFormat) -> Result<Vec<u8>, String> {
    let mut buf = Cursor::new(Vec::new());
    match format {
        OutFormat::Png => DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .map_err(|e| format!("PNG encode failed: {e}"))?,
        OutFormat::Jpeg => {
            // JPEG has no alpha; drop it (transparent areas become opaque black).
            let rgb = DynamicImage::ImageRgba8(img).to_rgb8();
            DynamicImage::ImageRgb8(rgb)
                .write_to(&mut buf, ImageFormat::Jpeg)
                .map_err(|e| format!("JPEG encode failed: {e}"))?;
        }
    }
    Ok(buf.into_inner())
}

/// Decode two raw image byte buffers, then composite them.
#[allow(clippy::too_many_arguments)]
pub fn composite_from_bytes(
    base: &[u8],
    overlay: &[u8],
    blend: BlendMode,
    opacity: f64,
    scale: f64,
    position: Position,
    offset_x: i64,
    offset_y: i64,
    flip: Flip,
    format: OutFormat,
) -> Result<Vec<u8>, String> {
    let base = image::load_from_memory(base)
        .map_err(|e| format!("base image (first) could not be decoded: {e}"))?;
    let overlay = image::load_from_memory(overlay)
        .map_err(|e| format!("overlay image (second) could not be decoded: {e}"))?;
    composite(base, overlay, blend, opacity, scale, position, offset_x, offset_y, flip, format)
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

    const WHITE: [u8; 4] = [255, 255, 255, 255];
    const RED: [u8; 4] = [255, 0, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];
    const GRAY: [u8; 4] = [128, 128, 128, 255];

    #[test]
    fn parse_enums() {
        assert_eq!(parse_blend_mode("").unwrap(), BlendMode::Normal);
        assert_eq!(parse_blend_mode("Multiply").unwrap(), BlendMode::Multiply);
        assert_eq!(parse_blend_mode("soft-light").unwrap(), BlendMode::SoftLight);
        assert_eq!(parse_blend_mode("linear-dodge").unwrap(), BlendMode::Add);
        assert!(parse_blend_mode("nope").is_err());
        assert_eq!(parse_position("").unwrap(), Position::Center);
        assert_eq!(parse_position("bottom-right").unwrap(), Position::BottomRight);
        assert!(parse_position("nowhere").is_err());
        assert_eq!(parse_flip("").unwrap(), Flip::None);
        assert_eq!(parse_flip("horizontal").unwrap(), Flip::Horizontal);
        assert!(parse_flip("diagonal").is_err());
        assert_eq!(parse_format("JPG").unwrap(), OutFormat::Jpeg);
        assert!(parse_format("gif").is_err());
    }

    #[test]
    fn canvas_matches_base_dimensions() {
        // Output canvas comes from the base (40x30); overlay (100x100) is clipped.
        let base = solid(40, 30, RED);
        let over = solid(100, 100, BLUE);
        let png = composite(
            base, over, BlendMode::Normal, 1.0, 100.0, Position::Center, 0, 0, Flip::None,
            OutFormat::Png,
        )
        .unwrap();
        let d = image::load_from_memory(&png).unwrap();
        assert_eq!((d.width(), d.height()), (40, 30));
    }

    #[test]
    fn normal_full_opacity_replaces_base() {
        let base = solid(20, 20, RED);
        let over = solid(20, 20, BLUE);
        let png = composite(
            base, over, BlendMode::Normal, 1.0, 100.0, Position::Center, 0, 0, Flip::None,
            OutFormat::Png,
        )
        .unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(10, 10).0, BLUE, "opaque overlay hides the base");
    }

    #[test]
    fn zero_opacity_keeps_base() {
        let base = solid(20, 20, RED);
        let over = solid(20, 20, BLUE);
        let png = composite(
            base, over, BlendMode::Normal, 0.0, 100.0, Position::Center, 0, 0, Flip::None,
            OutFormat::Png,
        )
        .unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(10, 10).0, RED, "opacity 0 leaves the base untouched");
    }

    #[test]
    fn multiply_darkens() {
        // multiply(white base, gray overlay) = gray.
        let base = solid(10, 10, WHITE);
        let over = solid(10, 10, GRAY);
        let png = composite(
            base, over, BlendMode::Multiply, 1.0, 100.0, Position::Center, 0, 0, Flip::None,
            OutFormat::Png,
        )
        .unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let p = img.get_pixel(5, 5).0;
        assert_eq!(p, GRAY, "multiply by white base is the overlay color, got {p:?}");
        // multiply(red base, blue overlay) → red*blue per channel = black.
        let png2 = composite(
            solid(10, 10, RED), solid(10, 10, BLUE), BlendMode::Multiply, 1.0, 100.0,
            Position::Center, 0, 0, Flip::None, OutFormat::Png,
        )
        .unwrap();
        let img2 = image::load_from_memory(&png2).unwrap().to_rgba8();
        assert_eq!(img2.get_pixel(5, 5).0, [0, 0, 0, 255], "red*blue = black");
    }

    #[test]
    fn screen_lightens() {
        // screen(red base, blue overlay) = magenta (each channel max of the two here).
        let png = composite(
            solid(10, 10, RED), solid(10, 10, BLUE), BlendMode::Screen, 1.0, 100.0,
            Position::Center, 0, 0, Flip::None, OutFormat::Png,
        )
        .unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(5, 5).0, [255, 0, 255, 255], "screen red+blue = magenta");
    }

    #[test]
    fn position_top_left_only_covers_corner() {
        // 40x40 red base, 10x10 blue overlay at top-left: corner is blue, far side red.
        let png = composite(
            solid(40, 40, RED), solid(10, 10, BLUE), BlendMode::Normal, 1.0, 100.0,
            Position::TopLeft, 0, 0, Flip::None, OutFormat::Png,
        )
        .unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(2, 2).0, BLUE, "top-left corner is the overlay");
        assert_eq!(img.get_pixel(35, 35).0, RED, "far corner stays base");
    }

    #[test]
    fn offset_moves_overlay() {
        // Overlay at top-left + offset (20,20) lands in the middle, not the corner.
        let png = composite(
            solid(40, 40, RED), solid(10, 10, BLUE), BlendMode::Normal, 1.0, 100.0,
            Position::TopLeft, 20, 20, Flip::None, OutFormat::Png,
        )
        .unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(2, 2).0, RED, "corner is now base again");
        assert_eq!(img.get_pixel(25, 25).0, BLUE, "overlay shifted to the offset");
    }

    #[test]
    fn scale_shrinks_overlay() {
        // 40x40 base, 20x20 overlay scaled to 50% = 10x10 at top-left.
        let png = composite(
            solid(40, 40, RED), solid(20, 20, BLUE), BlendMode::Normal, 1.0, 50.0,
            Position::TopLeft, 0, 0, Flip::None, OutFormat::Png,
        )
        .unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(2, 2).0, BLUE, "scaled overlay covers the 10x10 corner");
        assert_eq!(img.get_pixel(15, 15).0, RED, "beyond 10px is base");
    }

    #[test]
    fn half_opacity_blends_halfway() {
        // Normal blend, 50% opacity: red base + blue overlay → ~ (128,0,128).
        let png = composite(
            solid(10, 10, RED), solid(10, 10, BLUE), BlendMode::Normal, 0.5, 100.0,
            Position::Center, 0, 0, Flip::None, OutFormat::Png,
        )
        .unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let p = img.get_pixel(5, 5).0;
        assert!((p[0] as i32 - 128).abs() <= 2, "R ~128, got {}", p[0]);
        assert_eq!(p[1], 0);
        assert!((p[2] as i32 - 128).abs() <= 2, "B ~128, got {}", p[2]);
        assert_eq!(p[3], 255);
    }

    #[test]
    fn jpeg_output_is_valid_jpeg() {
        let png = composite(
            solid(32, 32, RED), solid(16, 16, BLUE), BlendMode::Normal, 1.0, 100.0,
            Position::Center, 0, 0, Flip::None, OutFormat::Jpeg,
        )
        .unwrap();
        assert_eq!(&png[0..2], &[0xFF, 0xD8], "JPEG SOI marker");
        let d = image::load_from_memory(&png).unwrap();
        assert_eq!((d.width(), d.height()), (32, 32));
    }

    #[test]
    fn from_bytes_decodes_both() {
        let base = png_bytes(&solid(16, 16, RED));
        let over = png_bytes(&solid(16, 16, BLUE));
        let png = composite_from_bytes(
            &base, &over, BlendMode::Normal, 1.0, 100.0, Position::Center, 0, 0, Flip::None,
            OutFormat::Png,
        )
        .unwrap();
        let d = image::load_from_memory(&png).unwrap();
        assert_eq!((d.width(), d.height()), (16, 16));
    }

    #[test]
    fn from_bytes_rejects_garbage() {
        let good = png_bytes(&solid(16, 16, RED));
        let err = composite_from_bytes(
            b"not an image", &good, BlendMode::Normal, 1.0, 100.0, Position::Center, 0, 0,
            Flip::None, OutFormat::Png,
        )
        .unwrap_err();
        assert!(err.contains("base image"), "got: {err}");
    }
}
