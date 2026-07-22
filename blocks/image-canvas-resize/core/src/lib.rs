//! gizza-ai/image-canvas-resize core — change an image's CANVAS to an exact
//! width × height WITHOUT scaling the pixels. Growing the canvas adds margin
//! (filled with a chosen colour) around the content; shrinking crops it. A
//! 9-point anchor decides where the content sits (and, on shrink, what is
//! cropped away). This is the ImageMagick `-extent` / Photoshop "Canvas Size"
//! operation — NOT a resize (image-resize) or a scaled fit (image-contain-fit /
//! image-cover-fit). Pure-Rust `image`; no wafer/wasm-bindgen deps. Returns PNG
//! bytes (alpha preserved when the fill colour is transparent).

use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};

/// Where the source content sits on the new canvas. On grow the margin is added
/// on the opposite side(s); on shrink the crop is taken from the opposite
/// side(s). `Center` splits the difference evenly on both axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Center,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// The nine accepted anchor keywords, in a stable order (used by the descriptor
/// enum + the drift guard).
pub const ANCHORS: [&str; 9] = [
    "center",
    "top",
    "bottom",
    "left",
    "right",
    "top-left",
    "top-right",
    "bottom-left",
    "bottom-right",
];

pub const DEFAULT_ANCHOR: &str = "center";
pub const DEFAULT_FILL: &str = "#ffffff";

impl Anchor {
    /// Parse a kebab/snake/space gravity name (case-insensitive). Accepts the
    /// nine standard gravities plus `centre`/`middle` aliases and the compass
    /// names (north/south/east/west + corners).
    pub fn parse(s: &str) -> Result<Anchor, String> {
        let norm: String = s
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        match norm.as_str() {
            "center" | "centre" | "middle" => Ok(Anchor::Center),
            "top" | "topcenter" | "topcentre" | "north" => Ok(Anchor::Top),
            "bottom" | "bottomcenter" | "bottomcentre" | "south" => Ok(Anchor::Bottom),
            "left" | "leftcenter" | "leftcentre" | "west" => Ok(Anchor::Left),
            "right" | "rightcenter" | "rightcentre" | "east" => Ok(Anchor::Right),
            "topleft" | "lefttop" | "northwest" => Ok(Anchor::TopLeft),
            "topright" | "righttop" | "northeast" => Ok(Anchor::TopRight),
            "bottomleft" | "leftbottom" | "southwest" => Ok(Anchor::BottomLeft),
            "bottomright" | "rightbottom" | "southeast" => Ok(Anchor::BottomRight),
            _ => Err(format!(
                "invalid anchor '{s}'; use one of: center, top, bottom, left, right, top-left, top-right, bottom-left, bottom-right"
            )),
        }
    }

    /// Horizontal weighting: (lead, trail). `lead` pins the content to the left
    /// (offset 0); `trail` pins it to the right (offset = full delta); neither =
    /// centred (delta / 2).
    fn horizontal(self) -> (bool, bool) {
        match self {
            Anchor::Left | Anchor::TopLeft | Anchor::BottomLeft => (true, false),
            Anchor::Right | Anchor::TopRight | Anchor::BottomRight => (false, true),
            _ => (false, false),
        }
    }

    /// Vertical weighting: (lead, trail). `lead` pins to the top; `trail` to the
    /// bottom; neither = centred.
    fn vertical(self) -> (bool, bool) {
        match self {
            Anchor::Top | Anchor::TopLeft | Anchor::TopRight => (true, false),
            Anchor::Bottom | Anchor::BottomLeft | Anchor::BottomRight => (false, true),
            _ => (false, false),
        }
    }

    /// Signed offset of the source's leading edge relative to the canvas along
    /// one axis, given `delta = canvas_len - src_len`. Positive delta (canvas
    /// bigger) → a margin; negative delta (canvas smaller) → the content is
    /// shifted off-canvas so the anchored side is cropped. `lead` → 0,
    /// `trail` → delta, otherwise → delta / 2 (even split; ties round toward 0).
    fn offset(delta: i64, lead: bool, trail: bool) -> i64 {
        if lead {
            0
        } else if trail {
            delta
        } else {
            delta / 2
        }
    }

    /// (dx, dy): where the source's top-left pixel lands on the canvas.
    /// `dw = canvas_w - src_w`, `dh = canvas_h - src_h` (either sign).
    pub fn placement(self, dw: i64, dh: i64) -> (i64, i64) {
        let (hl, ht) = self.horizontal();
        let (vl, vt) = self.vertical();
        (Self::offset(dw, hl, ht), Self::offset(dh, vl, vt))
    }
}

/// Parse a CSS-style colour into RGBA.
///
/// Accepts `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` (with or without the leading
/// `#`) and a few common named colours. The alpha defaults to fully opaque
/// (255) when not given. `transparent`/`none` yields a fully transparent pixel.
pub fn parse_color(s: &str) -> Result<Rgba<u8>, String> {
    let t = s.trim();
    let lower = t.to_ascii_lowercase();
    match lower.as_str() {
        "transparent" | "none" => return Ok(Rgba([0, 0, 0, 0])),
        "white" => return Ok(Rgba([255, 255, 255, 255])),
        "black" => return Ok(Rgba([0, 0, 0, 255])),
        "red" => return Ok(Rgba([255, 0, 0, 255])),
        "green" => return Ok(Rgba([0, 128, 0, 255])),
        "blue" => return Ok(Rgba([0, 0, 255, 255])),
        "gray" | "grey" => return Ok(Rgba([128, 128, 128, 255])),
        _ => {}
    }
    let hex = lower.strip_prefix('#').unwrap_or(&lower);
    if hex.is_empty() || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "invalid colour '{s}'; use a hex value like #ffffff or #f00, or a name like white/black/transparent"
        ));
    }
    let parse2 = |a: &str| u8::from_str_radix(a, 16).map_err(|e| e.to_string());
    let dbl = |c: char| {
        let v = c.to_digit(16).unwrap() as u8;
        (v << 4) | v
    };
    match hex.len() {
        3 => {
            let n: Vec<u8> = hex.chars().map(dbl).collect();
            Ok(Rgba([n[0], n[1], n[2], 255]))
        }
        4 => {
            let n: Vec<u8> = hex.chars().map(dbl).collect();
            Ok(Rgba([n[0], n[1], n[2], n[3]]))
        }
        6 => Ok(Rgba([
            parse2(&hex[0..2])?,
            parse2(&hex[2..4])?,
            parse2(&hex[4..6])?,
            255,
        ])),
        8 => Ok(Rgba([
            parse2(&hex[0..2])?,
            parse2(&hex[2..4])?,
            parse2(&hex[4..6])?,
            parse2(&hex[6..8])?,
        ])),
        _ => Err(format!(
            "invalid hex colour '{s}'; expected 3, 4, 6, or 8 hex digits"
        )),
    }
}

/// Largest accepted canvas edge and total pixel count — a guard against the
/// 64 MiB wasm sandbox (an N-pixel RGBA canvas needs 4·N bytes plus the decoded
/// source raster; ~24 MP keeps the working set well under the limit).
pub const MAX_EDGE: u32 = 20_000;
pub const MAX_CANVAS_PIXELS: u64 = 24_000_000;

/// Re-canvas `bytes` to exactly `canvas_w × canvas_h` WITHOUT scaling the source
/// pixels. The source is placed at native size according to `anchor`; the new
/// area is filled with `fill`; any source region outside the canvas is cropped.
/// Returns PNG bytes of exactly the requested dimensions.
pub fn canvas_resize(
    bytes: &[u8],
    canvas_w: u32,
    canvas_h: u32,
    anchor: Anchor,
    fill: Rgba<u8>,
) -> Result<Vec<u8>, String> {
    if canvas_w == 0 || canvas_h == 0 {
        return Err("width and height must be > 0".into());
    }
    if canvas_w > MAX_EDGE || canvas_h > MAX_EDGE {
        return Err(format!(
            "canvas too large: {canvas_w}x{canvas_h}; each side must be <= {MAX_EDGE}px"
        ));
    }
    if canvas_w as u64 * canvas_h as u64 > MAX_CANVAS_PIXELS {
        return Err(format!(
            "canvas too large: {canvas_w}x{canvas_h} exceeds the {MAX_CANVAS_PIXELS}-pixel limit"
        ));
    }

    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (src_w, src_h) = img.dimensions();
    if src_w == 0 || src_h == 0 {
        return Err("source image has zero dimensions".into());
    }
    let src = img.to_rgba8();

    let (dx, dy) = anchor.placement(
        canvas_w as i64 - src_w as i64,
        canvas_h as i64 - src_h as i64,
    );

    let mut canvas = RgbaImage::from_pixel(canvas_w, canvas_h, fill);

    // Copy the overlap of the placed source and the canvas. Iterate over the
    // destination window so out-of-canvas source pixels are simply skipped
    // (that is the crop), and untouched cells keep the fill (that is the pad).
    let x0 = dx.max(0);
    let y0 = dy.max(0);
    let x1 = (dx + src_w as i64).min(canvas_w as i64);
    let y1 = (dy + src_h as i64).min(canvas_h as i64);
    if x1 > x0 && y1 > y0 {
        for cy in y0..y1 {
            let sy = (cy - dy) as u32;
            for cx in x0..x1 {
                let sx = (cx - dx) as u32;
                canvas.put_pixel(cx as u32, cy as u32, *src.get_pixel(sx, sy));
            }
        }
    }

    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(canvas)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, color: Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, color);
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    // ---- colour parsing ----

    #[test]
    fn parse_color_hex_and_names() {
        assert_eq!(parse_color("#ffffff").unwrap(), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_color("000000").unwrap(), Rgba([0, 0, 0, 255]));
        assert_eq!(parse_color("#fff").unwrap(), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_color("#f00").unwrap(), Rgba([255, 0, 0, 255]));
        assert_eq!(parse_color("#1a2b3c").unwrap(), Rgba([26, 43, 60, 255]));
        assert_eq!(parse_color("#11223344").unwrap(), Rgba([17, 34, 51, 68]));
        assert_eq!(parse_color("#0f08").unwrap(), Rgba([0, 255, 0, 136]));
        assert_eq!(parse_color("white").unwrap(), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_color("BLACK").unwrap(), Rgba([0, 0, 0, 255]));
        assert_eq!(parse_color("transparent").unwrap(), Rgba([0, 0, 0, 0]));
        assert_eq!(parse_color("none").unwrap(), Rgba([0, 0, 0, 0]));
    }

    #[test]
    fn parse_color_rejects_garbage() {
        assert!(parse_color("#xyz").is_err());
        assert!(parse_color("notacolour").is_err());
        assert!(parse_color("#12345").is_err()); // 5 digits
        assert!(parse_color("").is_err());
    }

    // ---- anchor parsing + placement ----

    #[test]
    fn anchor_parses_aliases() {
        assert_eq!(Anchor::parse("center").unwrap(), Anchor::Center);
        assert_eq!(Anchor::parse("Centre").unwrap(), Anchor::Center);
        assert_eq!(Anchor::parse("middle").unwrap(), Anchor::Center);
        assert_eq!(Anchor::parse("top-left").unwrap(), Anchor::TopLeft);
        assert_eq!(Anchor::parse("TOP_RIGHT").unwrap(), Anchor::TopRight);
        assert_eq!(Anchor::parse("bottom right").unwrap(), Anchor::BottomRight);
        assert_eq!(Anchor::parse("north").unwrap(), Anchor::Top);
        assert_eq!(Anchor::parse("southeast").unwrap(), Anchor::BottomRight);
    }

    #[test]
    fn anchor_rejects_garbage() {
        assert!(Anchor::parse("nowhere").is_err());
        assert!(Anchor::parse("").is_err());
    }

    #[test]
    fn placement_grow_center_and_corners() {
        // Canvas 20px bigger horizontally, 40px bigger vertically.
        assert_eq!(Anchor::Center.placement(20, 40), (10, 20));
        assert_eq!(Anchor::TopLeft.placement(20, 40), (0, 0));
        assert_eq!(Anchor::BottomRight.placement(20, 40), (20, 40));
        assert_eq!(Anchor::Top.placement(20, 40), (10, 0));
        assert_eq!(Anchor::Right.placement(20, 40), (20, 20));
    }

    #[test]
    fn placement_shrink_is_negative_offset() {
        // Canvas 20px smaller each axis → content shifts off-canvas for crop.
        assert_eq!(Anchor::Center.placement(-20, -20), (-10, -10));
        assert_eq!(Anchor::TopLeft.placement(-20, -20), (0, 0));
        assert_eq!(Anchor::BottomRight.placement(-20, -20), (-20, -20));
    }

    // ---- canvas_resize: grow (pad) ----

    #[test]
    fn grow_pads_with_fill_centered() {
        // 2x2 red onto a 4x4 white canvas, centered → red occupies the middle
        // 2x2; the border is white.
        let src = Rgba([255, 0, 0, 255]);
        let out = canvas_resize(
            &png(2, 2, src),
            4,
            4,
            Anchor::Center,
            Rgba([255, 255, 255, 255]),
        )
        .unwrap();
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (4, 4));
        // Corners are the fill.
        assert_eq!(img.get_pixel(0, 0), &Rgba([255, 255, 255, 255]));
        assert_eq!(img.get_pixel(3, 3), &Rgba([255, 255, 255, 255]));
        // Middle is the source.
        assert_eq!(img.get_pixel(1, 1), &src);
        assert_eq!(img.get_pixel(2, 2), &src);
    }

    #[test]
    fn grow_top_left_anchor_places_source_at_origin() {
        let src = Rgba([0, 0, 255, 255]);
        let out = canvas_resize(
            &png(2, 2, src),
            4,
            4,
            Anchor::TopLeft,
            Rgba([0, 0, 0, 255]),
        )
        .unwrap();
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(0, 0), &src); // source pinned top-left
        assert_eq!(img.get_pixel(1, 1), &src);
        assert_eq!(img.get_pixel(2, 2), &Rgba([0, 0, 0, 255])); // fill beyond source
        assert_eq!(img.get_pixel(3, 3), &Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn grow_transparent_fill_keeps_alpha() {
        let src = Rgba([10, 20, 30, 255]);
        let out = canvas_resize(&png(1, 1, src), 3, 3, Anchor::Center, Rgba([0, 0, 0, 0])).unwrap();
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(0, 0), &Rgba([0, 0, 0, 0])); // transparent margin
        assert_eq!(img.get_pixel(1, 1), &src); // opaque source pixel
    }

    // ---- canvas_resize: shrink (crop) ----

    #[test]
    fn shrink_crops_by_anchor_no_scaling() {
        // 4x4 image: left 2 columns red, right 2 columns blue. Shrink canvas to
        // 2x4. Left anchor keeps the red half; right anchor keeps the blue half.
        let mut img = RgbaImage::new(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                let c = if x < 2 {
                    Rgba([255, 0, 0, 255])
                } else {
                    Rgba([0, 0, 255, 255])
                };
                img.put_pixel(x, y, c);
            }
        }
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let bytes = buf.into_inner();

        let left = canvas_resize(&bytes, 2, 4, Anchor::Left, Rgba([0, 0, 0, 0])).unwrap();
        let li = image::load_from_memory(&left).unwrap().to_rgba8();
        assert_eq!(li.dimensions(), (2, 4));
        assert_eq!(li.get_pixel(0, 0), &Rgba([255, 0, 0, 255]));
        assert_eq!(li.get_pixel(1, 0), &Rgba([255, 0, 0, 255]));

        let right = canvas_resize(&bytes, 2, 4, Anchor::Right, Rgba([0, 0, 0, 0])).unwrap();
        let ri = image::load_from_memory(&right).unwrap().to_rgba8();
        assert_eq!(ri.get_pixel(0, 0), &Rgba([0, 0, 255, 255]));
        assert_eq!(ri.get_pixel(1, 0), &Rgba([0, 0, 255, 255]));
    }

    #[test]
    fn mixed_grow_one_axis_shrink_other() {
        // 4x2 source → 2x4 canvas: width shrinks (crop), height grows (pad).
        let src = Rgba([100, 150, 200, 255]);
        let out =
            canvas_resize(&png(4, 2, src), 2, 4, Anchor::Center, Rgba([1, 2, 3, 255])).unwrap();
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (2, 4));
        // Vertically centered: rows 0 and 3 are pad, rows 1-2 are source.
        assert_eq!(img.get_pixel(0, 0), &Rgba([1, 2, 3, 255]));
        assert_eq!(img.get_pixel(0, 3), &Rgba([1, 2, 3, 255]));
        assert_eq!(img.get_pixel(0, 1), &src);
        assert_eq!(img.get_pixel(1, 2), &src);
    }

    #[test]
    fn same_size_is_identity() {
        let src = Rgba([7, 8, 9, 255]);
        let out = canvas_resize(&png(3, 3, src), 3, 3, Anchor::Center, Rgba([0, 0, 0, 0])).unwrap();
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (3, 3));
        for y in 0..3 {
            for x in 0..3 {
                assert_eq!(img.get_pixel(x, y), &src);
            }
        }
    }

    // ---- errors ----

    #[test]
    fn rejects_zero_dims_and_bad_input() {
        assert!(canvas_resize(&png(4, 4, Rgba([0, 0, 0, 255])), 0, 4, Anchor::Center, Rgba([0, 0, 0, 255])).is_err());
        assert!(canvas_resize(&png(4, 4, Rgba([0, 0, 0, 255])), 4, 0, Anchor::Center, Rgba([0, 0, 0, 255])).is_err());
        assert!(canvas_resize(b"not an image", 4, 4, Anchor::Center, Rgba([0, 0, 0, 255])).is_err());
    }

    #[test]
    fn rejects_oversized_canvas() {
        assert!(canvas_resize(&png(2, 2, Rgba([0, 0, 0, 255])), MAX_EDGE + 1, 4, Anchor::Center, Rgba([0, 0, 0, 255])).is_err());
        // Within per-edge limit but over the pixel budget.
        assert!(canvas_resize(&png(2, 2, Rgba([0, 0, 0, 255])), 20_000, 20_000, Anchor::Center, Rgba([0, 0, 0, 255])).is_err());
    }
}
