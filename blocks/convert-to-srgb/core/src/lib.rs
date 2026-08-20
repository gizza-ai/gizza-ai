//! convert-to-srgb core — decode an image, apply its embedded ICC profile to
//! the pixels, and emit plain sRGB PNG bytes. Pure Rust (`image` + `moxcms`), no
//! wafer/wasm-bindgen deps.

use std::io::Cursor;

use image::codecs::png::PngEncoder;
use image::{DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, ImageReader};
use moxcms::{ColorProfile, Layout, TransformOptions};

const MAX_PIXELS: u64 = 40_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertReport {
    pub width: u32,
    pub height: u32,
    pub input_bytes: usize,
    pub output_bytes: usize,
    pub icc_bytes: usize,
}

impl ConvertReport {
    pub fn summary(&self) -> String {
        format!(
            "converted {}x{} image from embedded ICC profile ({} bytes) to plain sRGB PNG ({} -> {} bytes)",
            self.width, self.height, self.icc_bytes, self.input_bytes, self.output_bytes
        )
    }
}

/// Convert an image carrying an embedded ICC profile to sRGB and return PNG bytes.
pub fn convert_to_srgb_png(input: &[u8]) -> Result<(Vec<u8>, ConvertReport), String> {
    let mut reader = ImageReader::new(Cursor::new(input))
        .with_guessed_format()
        .map_err(|e| format!("could not identify image format: {e}"))?;
    reader.no_limits();
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| format!("could not decode image header: {e}"))?;
    let (width, height) = decoder.dimensions();
    let pixels = width as u64 * height as u64;
    if pixels > MAX_PIXELS {
        return Err(format!(
            "image is too large: {width}x{height} is {pixels} pixels; limit is {MAX_PIXELS}"
        ));
    }
    let icc = decoder
        .icc_profile()
        .map_err(|e| format!("could not read embedded ICC profile: {e}"))?
        .ok_or_else(|| "image has no embedded ICC profile to convert from".to_string())?;

    let dyn_img = DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("could not decode image pixels: {e}"))?;
    let mut rgba = dyn_img.to_rgba8();

    let src_profile = ColorProfile::new_from_slice(&icc)
        .map_err(|e| format!("embedded ICC profile is not supported: {e}"))?;
    let dst_profile = ColorProfile::new_srgb();
    let transform = src_profile
        .create_transform_8bit(
            Layout::Rgba,
            &dst_profile,
            Layout::Rgba,
            TransformOptions::default(),
        )
        .map_err(|e| format!("could not create ICC-to-sRGB transform: {e}"))?;
    let mut converted = vec![0u8; rgba.len()];
    transform
        .transform(rgba.as_raw(), &mut converted)
        .map_err(|e| format!("ICC transform failed: {e}"))?;
    rgba.as_mut().copy_from_slice(&converted);

    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(rgba.as_raw(), width, height, ExtendedColorType::Rgba8)
        .map_err(|e| format!("could not encode sRGB PNG: {e}"))?;
    let report = ConvertReport {
        width,
        height,
        input_bytes: input.len(),
        output_bytes: out.len(),
        icc_bytes: icc.len(),
    };
    Ok((out, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, ImageEncoder, Rgba};

    fn tiny_png_without_icc() -> Vec<u8> {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(1, 1, Rgba([1, 2, 3, 255]));
        let mut out = Vec::new();
        PngEncoder::new(&mut out)
            .write_image(img.as_raw(), 1, 1, ExtendedColorType::Rgba8)
            .unwrap();
        out
    }

    #[test]
    fn errors_when_image_has_no_embedded_profile() {
        let err = convert_to_srgb_png(&tiny_png_without_icc()).unwrap_err();
        assert!(err.contains("no embedded ICC profile"), "{err}");
    }

    #[test]
    fn converts_png_with_embedded_srgb_profile_to_unprofiled_png() {
        let icc = ColorProfile::new_srgb().encode().unwrap();
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(1, 1, Rgba([100, 120, 140, 255]));
        let mut input = Vec::new();
        let mut enc = PngEncoder::new(&mut input);
        enc.set_icc_profile(icc).unwrap();
        enc.write_image(img.as_raw(), 1, 1, ExtendedColorType::Rgba8)
            .unwrap();

        let (out, report) = convert_to_srgb_png(&input).unwrap();
        assert_eq!((report.width, report.height), (1, 1));
        assert!(report.icc_bytes > 0);
        let decoded = image::load_from_memory(&out).unwrap().to_rgba8();
        let px = decoded.get_pixel(0, 0).0;
        for (actual, expected) in px[..3].iter().zip([100u8, 120, 140]) {
            assert!(actual.abs_diff(expected) <= 1, "{px:?}");
        }
        assert_eq!(px[3], 255);
    }

    #[test]
    fn srgb_identity_transform_is_nearly_lossless_and_preserves_alpha() {
        let src = ColorProfile::new_srgb();
        let dst = ColorProfile::new_srgb();
        let transform = src
            .create_transform_8bit(Layout::Rgba, &dst, Layout::Rgba, TransformOptions::default())
            .unwrap();
        let input = [12u8, 34, 56, 200, 240, 128, 64, 255];
        let mut out = [0u8; 8];
        transform.transform(&input, &mut out).unwrap();
        for (actual, expected) in out.iter().zip(input.iter()).take(3) {
            assert!(actual.abs_diff(*expected) <= 1, "{out:?} vs {input:?}");
        }
        assert_eq!(out[3], input[3]);
        for (actual, expected) in out[4..7].iter().zip(input[4..7].iter()) {
            assert!(actual.abs_diff(*expected) <= 1, "{out:?} vs {input:?}");
        }
        assert_eq!(out[7], input[7]);
    }
}
