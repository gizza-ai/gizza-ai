//! gizza-ai/extract-rgba core — decode an image and export its raw per-pixel
//! RGBA8 values as text, CSV, or JSON. No wafer/wasm-bindgen deps. Pure-Rust
//! `image` crate.

use image::GenericImageView;

/// Output serialization for the per-pixel RGBA values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// One `r,g,b,a` triple-comma line per pixel, in row-major (left→right,
    /// top→bottom) order. Compact and easy to pipe.
    Text,
    /// `x,y,r,g,b,a` rows with a header — spreadsheet / scripting friendly.
    Csv,
    /// `{"width":W,"height":H,"pixels":[[r,g,b,a],...]}` (row-major).
    Json,
}

impl Format {
    /// Parse the `format` param. Defaults to JSON on an empty string.
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "json" => Ok(Format::Json),
            "text" | "txt" | "plain" => Ok(Format::Text),
            "csv" => Ok(Format::Csv),
            other => Err(format!(
                "unknown format '{other}' (expected: text, csv, or json)"
            )),
        }
    }
}

/// Result of extracting RGBA values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extraction {
    /// Rendered output in the requested format.
    pub data: String,
    pub width: u32,
    pub height: u32,
    /// Total pixels in the image (width * height).
    pub total_pixels: u64,
    /// Number of pixels actually emitted (may be < total when capped).
    pub emitted_pixels: u64,
    /// True when `max_pixels` truncated the output.
    pub truncated: bool,
}

/// Decode `bytes` and render the first `max_pixels` pixels' RGBA values in
/// `format` (row-major order). `max_pixels == 0` means no cap (emit all).
///
/// Errors if the input is empty or not a decodable image.
pub fn extract(bytes: &[u8], format: Format, max_pixels: u64) -> Result<Extraction, String> {
    if bytes.is_empty() {
        return Err("input is empty".into());
    }
    image::guess_format(bytes).map_err(|_| "unrecognized image format".to_string())?;
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (width, height) = img.dimensions();
    let total_pixels = width as u64 * height as u64;
    let rgba = img.to_rgba8();

    let cap = if max_pixels == 0 {
        total_pixels
    } else {
        max_pixels.min(total_pixels)
    };
    let truncated = cap < total_pixels;

    // Iterate row-major: y outer, x inner. enumerate_pixels yields (x, y, &Rgba).
    let mut emitted: u64 = 0;
    let data = match format {
        Format::Text => {
            let mut s = String::new();
            for (_, _, px) in rgba.enumerate_pixels() {
                if emitted >= cap {
                    break;
                }
                let [r, g, b, a] = px.0;
                s.push_str(&format!("{r},{g},{b},{a}\n"));
                emitted += 1;
            }
            s
        }
        Format::Csv => {
            let mut s = String::from("x,y,r,g,b,a\n");
            for (x, y, px) in rgba.enumerate_pixels() {
                if emitted >= cap {
                    break;
                }
                let [r, g, b, a] = px.0;
                s.push_str(&format!("{x},{y},{r},{g},{b},{a}\n"));
                emitted += 1;
            }
            s
        }
        Format::Json => {
            let mut s = String::new();
            s.push_str(&format!(
                "{{\"width\":{width},\"height\":{height},\"pixels\":["
            ));
            for (_, _, px) in rgba.enumerate_pixels() {
                if emitted >= cap {
                    break;
                }
                let [r, g, b, a] = px.0;
                if emitted > 0 {
                    s.push(',');
                }
                s.push_str(&format!("[{r},{g},{b},{a}]"));
                emitted += 1;
            }
            s.push_str("]}");
            s
        }
    };

    Ok(Extraction {
        data,
        width,
        height,
        total_pixels,
        emitted_pixels: emitted,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn png_2x1() -> Vec<u8> {
        // Pixel (0,0) = red opaque, (1,0) = green half-alpha.
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 255, 0, 128]));
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn format_parse() {
        assert_eq!(Format::parse("").unwrap(), Format::Json);
        assert_eq!(Format::parse("JSON").unwrap(), Format::Json);
        assert_eq!(Format::parse("Text").unwrap(), Format::Text);
        assert_eq!(Format::parse("csv").unwrap(), Format::Csv);
        assert!(Format::parse("yaml").is_err());
    }

    #[test]
    fn text_rows_in_order() {
        let e = extract(&png_2x1(), Format::Text, 0).unwrap();
        assert_eq!((e.width, e.height), (2, 1));
        assert_eq!(e.total_pixels, 2);
        assert_eq!(e.emitted_pixels, 2);
        assert!(!e.truncated);
        assert_eq!(e.data, "255,0,0,255\n0,255,0,128\n");
    }

    #[test]
    fn csv_has_header_and_coords() {
        let e = extract(&png_2x1(), Format::Csv, 0).unwrap();
        assert_eq!(e.data, "x,y,r,g,b,a\n0,0,255,0,0,255\n1,0,0,255,0,128\n");
    }

    #[test]
    fn json_shape() {
        let e = extract(&png_2x1(), Format::Json, 0).unwrap();
        assert_eq!(
            e.data,
            "{\"width\":2,\"height\":1,\"pixels\":[[255,0,0,255],[0,255,0,128]]}"
        );
    }

    #[test]
    fn max_pixels_truncates() {
        let e = extract(&png_2x1(), Format::Text, 1).unwrap();
        assert!(e.truncated);
        assert_eq!(e.emitted_pixels, 1);
        assert_eq!(e.total_pixels, 2);
        assert_eq!(e.data, "255,0,0,255\n");
    }

    #[test]
    fn max_pixels_above_total_is_not_truncated() {
        let e = extract(&png_2x1(), Format::Json, 999).unwrap();
        assert!(!e.truncated);
        assert_eq!(e.emitted_pixels, 2);
    }

    #[test]
    fn errors_on_bad_input() {
        assert!(extract(&[], Format::Json, 0).is_err());
        assert!(extract(b"not an image", Format::Json, 0).is_err());
    }
}
