//! gizza-ai/flip-image core — flip an image horizontally (mirror) or vertically.
//! No wafer/wasm-bindgen deps. Pure-Rust `image`. Returns PNG bytes.

use std::io::Cursor;

use image::{DynamicImage, ImageFormat};

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
    use image::{GenericImageView, Rgba, RgbaImage};

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
    fn errors() {
        assert!(flip(b"not an image", Direction::Horizontal).is_err());
        assert!(Direction::parse("diagonal").is_err());
        assert_eq!(Direction::parse("V").unwrap(), Direction::Vertical);
    }
}
