//! gizza-ai/colorblind-simulator core — simulate how an image looks to people with
//! protanopia, deuteranopia, or tritanopia by applying the standard colour-vision
//! deficiency transform to each pixel. Pure-Rust (`image`). Output is PNG.
//!
//! Uses the widely-used approximate sRGB 3x3 simulation matrices (the same ones in
//! common colour-blindness libraries). Alpha is preserved.

use std::io::Cursor;

use image::ImageFormat;

/// Type of colour-vision deficiency to simulate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Red-deficient.
    Protanopia,
    /// Green-deficient (most common).
    Deuteranopia,
    /// Blue-deficient.
    Tritanopia,
}

impl Kind {
    pub fn parse(s: &str) -> Result<Kind, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "protanopia" | "protan" | "red" => Ok(Kind::Protanopia),
            "deuteranopia" | "deuteran" | "green" | "" => Ok(Kind::Deuteranopia),
            "tritanopia" | "tritan" | "blue" => Ok(Kind::Tritanopia),
            other => Err(format!(
                "unknown type '{other}' (use protanopia, deuteranopia, or tritanopia)"
            )),
        }
    }

    /// Row-major 3x3 sRGB simulation matrix.
    fn matrix(self) -> [[f32; 3]; 3] {
        match self {
            Kind::Protanopia => {
                [[0.567, 0.433, 0.0], [0.558, 0.442, 0.0], [0.0, 0.242, 0.758]]
            }
            Kind::Deuteranopia => {
                [[0.625, 0.375, 0.0], [0.70, 0.30, 0.0], [0.0, 0.30, 0.70]]
            }
            Kind::Tritanopia => {
                [[0.95, 0.05, 0.0], [0.0, 0.433, 0.567], [0.0, 0.475, 0.525]]
            }
        }
    }
}

fn clamp_u8(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// Simulate `kind` colour-blindness on `image_bytes`, returning PNG bytes.
pub fn simulate(image_bytes: &[u8], kind: Kind) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("could not decode image: {e}"))?;
    let mut rgba = img.to_rgba8();
    let m = kind.matrix();
    for px in rgba.pixels_mut() {
        let [r, g, b, a] = px.0;
        let (rf, gf, bf) = (r as f32, g as f32, b as f32);
        let nr = m[0][0] * rf + m[0][1] * gf + m[0][2] * bf;
        let ng = m[1][0] * rf + m[1][1] * gf + m[1][2] * bf;
        let nb = m[2][0] * rf + m[2][1] * gf + m[2][2] * bf;
        px.0 = [clamp_u8(nr), clamp_u8(ng), clamp_u8(nb), a];
    }

    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, Rgba, RgbaImage};

    fn solid_png(color: [u8; 4]) -> Vec<u8> {
        let img = RgbaImage::from_pixel(8, 8, Rgba(color));
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    fn first_pixel(png: &[u8]) -> [u8; 4] {
        image::load_from_memory(png).unwrap().get_pixel(0, 0).0
    }

    #[test]
    fn output_is_png_same_size_keeps_alpha() {
        let out = simulate(&solid_png([200, 50, 50, 128]), Kind::Deuteranopia).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (8, 8));
        assert_eq!(&out[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert_eq!(first_pixel(&out)[3], 128, "alpha preserved");
    }

    #[test]
    fn deuteranopia_red_shifts() {
        // Pure red under deuteranopia: r' = 0.625*255, g' = 0.70*255, b' = 0.
        let out = simulate(&solid_png([255, 0, 0, 255]), Kind::Deuteranopia).unwrap();
        let p = first_pixel(&out);
        assert_eq!(p[0], (0.625f32 * 255.0).round() as u8);
        assert_eq!(p[1], (0.70f32 * 255.0).round() as u8);
        assert_eq!(p[2], 0);
    }

    #[test]
    fn white_stays_white() {
        // Rows of each matrix sum to ~1, so white maps to ~white.
        for k in [Kind::Protanopia, Kind::Deuteranopia, Kind::Tritanopia] {
            let p = first_pixel(&simulate(&solid_png([255, 255, 255, 255]), k).unwrap());
            assert!(p[0] >= 253 && p[1] >= 253 && p[2] >= 253, "white preserved for {k:?}: {p:?}");
        }
    }

    #[test]
    fn parse_and_errors() {
        assert_eq!(Kind::parse("protan").unwrap(), Kind::Protanopia);
        assert_eq!(Kind::parse("").unwrap(), Kind::Deuteranopia);
        assert!(Kind::parse("monochrome").is_err());
        assert!(simulate(b"not an image", Kind::Protanopia).is_err());
    }
}
