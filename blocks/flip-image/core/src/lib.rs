//! gizza-ai/flip-image core — flip an image horizontally (mirror) or vertically.
//! No wafer/wasm-bindgen deps. Pure-Rust `image`. Returns PNG bytes.

use std::io::Cursor;

use image::ImageFormat;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Horizontal,
    Vertical,
}

impl Direction {
    pub fn parse(s: &str) -> Result<Direction, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "horizontal" | "h" | "mirror" | "" => Ok(Direction::Horizontal),
            "vertical" | "v" => Ok(Direction::Vertical),
            other => Err(format!("unknown direction '{other}' (use horizontal or vertical)")),
        }
    }
}

/// The tool's options, shared by every surface: the chat/CLI handler flattens
/// them from its JSON args, the browser page deserializes them from its params
/// object. A missing `direction` takes the schema default (`horizontal`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Params {
    /// `horizontal` (left-right mirror) or `vertical` (top-bottom).
    pub direction: String,
}

impl Default for Params {
    fn default() -> Self {
        Params { direction: "horizontal".to_string() }
    }
}

impl Params {
    /// The parsed flip axis.
    pub fn direction(&self) -> Result<Direction, String> {
        Direction::parse(&self.direction)
    }
}

/// Output filename for every surface.
pub const OUT_NAME: &str = "flipped.png";

/// Apply `params` to an encoded image. Returns the PNG bytes and their filename.
pub fn apply(bytes: &[u8], params: &Params) -> Result<(Vec<u8>, String), String> {
    let direction = params.direction()?;
    Ok((flip(bytes, direction)?, OUT_NAME.to_string()))
}

/// Flip `bytes` in the given direction. Returns PNG bytes (alpha preserved).
pub fn flip(bytes: &[u8], direction: Direction) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let flipped = match direction {
        Direction::Horizontal => img.fliph(),
        Direction::Vertical => img.flipv(),
    };
    let mut out = Cursor::new(Vec::new());
    flipped
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};

    fn png_2x1(left: Rgba<u8>, right: Rgba<u8>) -> Vec<u8> {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, left);
        img.put_pixel(1, 0, right);
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn png_1x2(top: Rgba<u8>, bottom: Rgba<u8>) -> Vec<u8> {
        let mut img = RgbaImage::new(1, 2);
        img.put_pixel(0, 0, top);
        img.put_pixel(0, 1, bottom);
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn horizontal_swaps_columns() {
        let red = Rgba([255, 0, 0, 255]);
        let blue = Rgba([0, 0, 255, 255]);
        let out = flip(&png_2x1(red, blue), Direction::Horizontal).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.get_pixel(0, 0), blue);
        assert_eq!(img.get_pixel(1, 0), red);
    }

    #[test]
    fn vertical_swaps_rows() {
        let red = Rgba([255, 0, 0, 255]);
        let blue = Rgba([0, 0, 255, 255]);
        let out = flip(&png_1x2(red, blue), Direction::Vertical).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.get_pixel(0, 0), blue);
        assert_eq!(img.get_pixel(0, 1), red);
    }

    #[test]
    fn dimensions_preserved() {
        let out = flip(&png_2x1(Rgba([1, 2, 3, 255]), Rgba([4, 5, 6, 255])), Direction::Vertical).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (2, 1));
    }

    #[test]
    fn params_default_to_horizontal() {
        assert_eq!(Params::default().direction, "horizontal");
        assert_eq!(Params::default().direction().unwrap(), Direction::Horizontal);
    }

    #[test]
    fn params_bad_direction_errors() {
        let p = Params { direction: "diagonal".to_string() };
        let err = p.direction().unwrap_err();
        assert!(err.contains("unknown direction 'diagonal'"), "{err}");
        assert!(apply(&png_2x1(Rgba([0, 0, 0, 255]), Rgba([0, 0, 0, 255])), &p).is_err());
    }

    #[test]
    fn params_deserialize_with_missing_fields() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p, Params::default());
        let v: Params = serde_json::from_str(r#"{"direction": "vertical"}"#).unwrap();
        assert_eq!(v.direction().unwrap(), Direction::Vertical);
    }

    #[test]
    fn apply_horizontal_mirrors_asymmetric_image() {
        // 3x2: distinct colors per column so a mirror is unambiguous.
        let mut img = RgbaImage::new(3, 2);
        for y in 0..2 {
            img.put_pixel(0, y, Rgba([255, 0, 0, 255]));
            img.put_pixel(1, y, Rgba([0, 255, 0, 255]));
            img.put_pixel(2, y, Rgba([0, 0, 255, 128]));
        }
        let mut src = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut src, ImageFormat::Png).unwrap();
        let (out, name) = apply(&src.into_inner(), &Params::default()).unwrap();
        assert_eq!(name, "flipped.png");
        let img = image::load_from_memory_with_format(&out, ImageFormat::Png).unwrap();
        assert_eq!(img.dimensions(), (3, 2));
        assert_eq!(img.get_pixel(0, 0), Rgba([0, 0, 255, 128]));
        assert_eq!(img.get_pixel(1, 1), Rgba([0, 255, 0, 255]));
        assert_eq!(img.get_pixel(2, 1), Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn errors() {
        assert!(flip(b"not an image", Direction::Horizontal).is_err());
        assert!(Direction::parse("diagonal").is_err());
        assert_eq!(Direction::parse("V").unwrap(), Direction::Vertical);
    }
}
