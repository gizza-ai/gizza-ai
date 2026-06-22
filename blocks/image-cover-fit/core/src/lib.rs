//! gizza-ai/image-cover-fit core — scale-and-centre-crop ("cover") an image so
//! it completely fills target dimensions while preserving aspect ratio. The
//! source is scaled to the smallest size that covers the box, then the overflow
//! is cropped away around a chosen anchor (default centre). Pure-Rust `image`;
//! no wafer/wasm-bindgen deps. Returns PNG bytes.

use std::io::Cursor;

use image::imageops::FilterType;
use image::ImageFormat;

/// Where to anchor the crop when the scaled image overflows the target box.
/// `Center` keeps the middle; the edge/corner variants bias toward that side.
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

impl Anchor {
    /// Parse a kebab/snake/space gravity name (case-insensitive). Accepts the
    /// nine standard gravities plus `centre` and `middle` aliases.
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

    /// Resolve the crop offset for an `overflow` (extra pixels) along one axis.
    /// `lead` biases toward the start (top/left), `trail` toward the end.
    fn offset(overflow: u32, lead: bool, trail: bool) -> u32 {
        if lead {
            0
        } else if trail {
            overflow
        } else {
            overflow / 2
        }
    }

    /// (x, y) top-left crop offset given the horizontal/vertical overflow.
    pub fn crop_offset(self, overflow_x: u32, overflow_y: u32) -> (u32, u32) {
        let (left, right, top, bottom) = match self {
            Anchor::Center => (false, false, false, false),
            Anchor::Top => (false, false, true, false),
            Anchor::Bottom => (false, false, false, true),
            Anchor::Left => (true, false, false, false),
            Anchor::Right => (false, true, false, false),
            Anchor::TopLeft => (true, false, true, false),
            Anchor::TopRight => (false, true, true, false),
            Anchor::BottomLeft => (true, false, false, true),
            Anchor::BottomRight => (false, true, false, true),
        };
        (
            Self::offset(overflow_x, left, right),
            Self::offset(overflow_y, top, bottom),
        )
    }
}

/// Compute the scaled dimensions that make `(src_w, src_h)` completely cover
/// `(target_w, target_h)` preserving aspect ratio (the "cover" rule). The
/// result always equals or exceeds the target on both axes when upscaling is
/// allowed; one axis matches exactly and the other overflows (to be cropped).
/// Pass `allow_upscale = false` to cap the scale at 1:1 — the returned cover
/// size may then be smaller than the target (the caller clamps the crop).
pub fn cover_size(
    src_w: u32,
    src_h: u32,
    target_w: u32,
    target_h: u32,
    allow_upscale: bool,
) -> (u32, u32) {
    if src_w == 0 || src_h == 0 {
        return (0, 0);
    }
    let mut scale = (target_w as f64 / src_w as f64).max(target_h as f64 / src_h as f64);
    if !allow_upscale && scale > 1.0 {
        scale = 1.0;
    }
    let w = ((src_w as f64) * scale).round().max(1.0) as u32;
    let h = ((src_h as f64) * scale).round().max(1.0) as u32;
    // Guarantee full coverage even after rounding when we are allowed to scale up.
    if allow_upscale {
        (w.max(target_w), h.max(target_h))
    } else {
        (w, h)
    }
}

/// Scale `bytes` to cover a `target_w` x `target_h` box preserving aspect ratio,
/// then centre-crop (around `anchor`) the overflow so the result is exactly the
/// target dimensions. When `allow_upscale` is false and the source is smaller
/// than the target on an axis, the output is clamped to what the source can
/// cover at 1:1 (it is never padded — use image-contain-fit for letterboxing).
/// Returns PNG bytes.
pub fn cover_fit(
    bytes: &[u8],
    target_w: u32,
    target_h: u32,
    anchor: Anchor,
    allow_upscale: bool,
) -> Result<Vec<u8>, String> {
    if target_w == 0 || target_h == 0 {
        return Err("width and height must be > 0".into());
    }
    let img =
        image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (src_w, src_h) = (img.width(), img.height());
    if src_w == 0 || src_h == 0 {
        return Err("source image has zero dimensions".into());
    }
    let (scaled_w, scaled_h) = cover_size(src_w, src_h, target_w, target_h, allow_upscale);

    // Scale the source (Lanczos3 for quality).
    let scaled = img.resize_exact(scaled_w, scaled_h, FilterType::Lanczos3);

    // The crop is the intersection of the scaled image and the target box.
    let crop_w = scaled_w.min(target_w);
    let crop_h = scaled_h.min(target_h);
    let overflow_x = scaled_w.saturating_sub(target_w);
    let overflow_y = scaled_h.saturating_sub(target_h);
    let (off_x, off_y) = anchor.crop_offset(overflow_x, overflow_y);

    let cropped = scaled.crop_imm(off_x, off_y, crop_w, crop_h);

    let mut out = Cursor::new(Vec::new());
    cropped
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};

    fn png(w: u32, h: u32, color: Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, color);
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

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
    fn anchor_crop_offsets() {
        // Center splits the overflow evenly.
        assert_eq!(Anchor::Center.crop_offset(10, 20), (5, 10));
        // Top-left clamps both to the start.
        assert_eq!(Anchor::TopLeft.crop_offset(10, 20), (0, 0));
        // Bottom-right takes the full overflow.
        assert_eq!(Anchor::BottomRight.crop_offset(10, 20), (10, 20));
        // Top keeps top (y=0), centres x.
        assert_eq!(Anchor::Top.crop_offset(10, 20), (5, 0));
        // Right pushes x to the end, centres y.
        assert_eq!(Anchor::Right.crop_offset(10, 20), (10, 10));
    }

    #[test]
    fn cover_size_landscape_into_square() {
        // 200x100 into 100x100 → height-bound (max ratio) → 200x100.
        assert_eq!(cover_size(200, 100, 100, 100, true), (200, 100));
    }

    #[test]
    fn cover_size_portrait_into_square() {
        // 100x200 into 100x100 → width-bound → 100x200.
        assert_eq!(cover_size(100, 200, 100, 100, true), (100, 200));
    }

    #[test]
    fn cover_size_no_upscale_caps_at_source() {
        // 50x50 into 100x100 with upscale off → stays 50x50.
        assert_eq!(cover_size(50, 50, 100, 100, false), (50, 50));
        // With upscale on → grows to cover → 100x100.
        assert_eq!(cover_size(50, 50, 100, 100, true), (100, 100));
    }

    #[test]
    fn output_is_exact_target_size() {
        let out = cover_fit(
            &png(200, 100, Rgba([255, 0, 0, 255])),
            100,
            100,
            Anchor::Center,
            true,
        )
        .unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (100, 100));
    }

    #[test]
    fn output_fills_with_no_padding() {
        // A solid image scaled+cropped to cover should have NO background pad —
        // every corner pixel is the source colour.
        let src = Rgba([12, 200, 90, 255]);
        let out = cover_fit(&png(300, 150, src), 100, 100, Anchor::Center, true).unwrap();
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        for (x, y) in [(0, 0), (99, 0), (0, 99), (99, 99), (50, 50)] {
            assert_eq!(img.get_pixel(x, y), &src, "no pad at ({x},{y})");
        }
    }

    #[test]
    fn anchor_changes_crop_region() {
        // Build a 200x100 image: left half red, right half blue. Cover into a
        // 100x100 square scales to 200x100 (overflow_x = 100). Left anchor keeps
        // the red half; right anchor keeps the blue half.
        let mut img = RgbaImage::new(200, 100);
        for y in 0..100 {
            for x in 0..200 {
                let c = if x < 100 {
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

        let left = cover_fit(&bytes, 100, 100, Anchor::Left, true).unwrap();
        let left_img = image::load_from_memory(&left).unwrap().to_rgba8();
        assert_eq!(left_img.get_pixel(0, 50), &Rgba([255, 0, 0, 255]));
        assert_eq!(left_img.get_pixel(99, 50), &Rgba([255, 0, 0, 255]));

        let right = cover_fit(&bytes, 100, 100, Anchor::Right, true).unwrap();
        let right_img = image::load_from_memory(&right).unwrap().to_rgba8();
        assert_eq!(right_img.get_pixel(0, 50), &Rgba([0, 0, 255, 255]));
        assert_eq!(right_img.get_pixel(99, 50), &Rgba([0, 0, 255, 255]));
    }

    #[test]
    fn no_upscale_clamps_small_source() {
        // 50x50 source into 100x100 with upscale off → output is at most the
        // source size (no padding to the target).
        let out =
            cover_fit(&png(50, 50, Rgba([1, 2, 3, 255])), 100, 100, Anchor::Center, false).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (50, 50));
    }

    #[test]
    fn rejects_zero_dims_and_bad_input() {
        assert!(cover_fit(
            &png(10, 10, Rgba([0, 0, 0, 255])),
            0,
            10,
            Anchor::Center,
            true
        )
        .is_err());
        assert!(cover_fit(b"not an image", 10, 10, Anchor::Center, true).is_err());
    }
}
