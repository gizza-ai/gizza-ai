//! gizza-ai/android-asset-generator core — pure-Rust generation of a full set of
//! Android launcher-icon assets from one source image. No wafer/wasm-bindgen deps.
//!
//! Android ships a separate raster icon for each screen-density bucket. From a
//! single (ideally square, >=512px) source image we downscale to every bucket
//! with a high-quality Lanczos3 filter and lay the results out in the standard
//! Android resource tree, plus the 512px Google Play store listing icon:
//!
//!   res/mipmap-mdpi/<name>.png      48x48
//!   res/mipmap-hdpi/<name>.png      72x72
//!   res/mipmap-xhdpi/<name>.png     96x96
//!   res/mipmap-xxhdpi/<name>.png   144x144
//!   res/mipmap-xxxhdpi/<name>.png  192x192
//!   play-store-icon.png            512x512
//!
//! Everything is bundled into a single ZIP (deflate). Pure `image` + `zip` →
//! instantiates in wafer and runs on every backend.

use std::io::{Cursor, Write};

use image::imageops::FilterType;
use image::ImageFormat;
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// A density bucket: the Android `mipmap-*` qualifier and its launcher-icon size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Density {
    pub qualifier: &'static str,
    pub size: u32,
}

/// The standard launcher-icon density buckets, smallest to largest.
pub const DENSITIES: &[Density] = &[
    Density { qualifier: "mdpi", size: 48 },
    Density { qualifier: "hdpi", size: 72 },
    Density { qualifier: "xhdpi", size: 96 },
    Density { qualifier: "xxhdpi", size: 144 },
    Density { qualifier: "xxxhdpi", size: 192 },
];

/// The Google Play store listing icon size (a square 512px PNG).
pub const PLAY_STORE_SIZE: u32 = 512;

/// Summary of a generation run (for the caller's user-facing message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// Number of PNG assets written into the ZIP.
    pub assets: usize,
    /// The base icon name used inside the mipmap folders (without extension).
    pub icon_name: String,
    /// Source image dimensions (width, height) before resizing.
    pub source_dims: (u32, u32),
}

/// Sanitize a caller-supplied icon name into a valid Android resource name:
/// lowercase, `[a-z0-9_]` only (other runs collapse to a single `_`), must start
/// with a letter. Empty / invalid input falls back to `ic_launcher`.
pub fn sanitize_icon_name(raw: &str) -> String {
    let mut out = String::new();
    let mut prev_us = false;
    for c in raw.trim().chars() {
        let lc = c.to_ascii_lowercase();
        if lc.is_ascii_lowercase() || lc.is_ascii_digit() {
            out.push(lc);
            prev_us = false;
        } else if !out.is_empty() && !prev_us {
            out.push('_');
            prev_us = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    // Android resource names must start with a letter.
    if out.is_empty() || !out.chars().next().unwrap().is_ascii_alphabetic() {
        return "ic_launcher".to_string();
    }
    out
}

/// Resize `img` to a square `size`x`size` PNG using Lanczos3 and return the bytes.
fn render_png(img: &image::DynamicImage, size: u32) -> Result<Vec<u8>, String> {
    let resized = img.resize_exact(size, size, FilterType::Lanczos3);
    let mut buf = Cursor::new(Vec::new());
    resized
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("encode {size}px PNG: {e}"))?;
    Ok(buf.into_inner())
}

/// Resize `img` to `size`x`size`, then apply a circular alpha mask (pixels
/// outside the inscribed circle become fully transparent) and encode as PNG.
/// This matches the `ic_launcher_round` variant launchers use on circular masks.
fn render_round_png(img: &image::DynamicImage, size: u32) -> Result<Vec<u8>, String> {
    let mut rgba = img.resize_exact(size, size, FilterType::Lanczos3).to_rgba8();
    let r = size as f32 / 2.0;
    // Anti-aliased edge over a 1px band at the circle boundary.
    for (x, y, px) in rgba.enumerate_pixels_mut() {
        let dx = x as f32 + 0.5 - r;
        let dy = y as f32 + 0.5 - r;
        let dist = (dx * dx + dy * dy).sqrt();
        let alpha = if dist <= r - 1.0 {
            1.0
        } else if dist >= r {
            0.0
        } else {
            r - dist // smooth 0..1 across the last pixel
        };
        let a = (px[3] as f32 * alpha).round().clamp(0.0, 255.0) as u8;
        px[3] = a;
    }
    let mut buf = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba)
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("encode {size}px round PNG: {e}"))?;
    Ok(buf.into_inner())
}

/// Generate the full Android icon set from `image_bytes` and return
/// `(zip_bytes, summary)`. `icon_name` is the base file name inside the mipmap
/// folders (sanitized; defaults to `ic_launcher`). When `round` is true, an
/// additional circular-masked `<name>_round.png` is emitted in every bucket
/// (the `ic_launcher_round` variant used by circular launcher masks).
pub fn generate_zip(
    image_bytes: &[u8],
    icon_name: &str,
    round: bool,
) -> Result<(Vec<u8>, Summary), String> {
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("could not decode image: {e}"))?;
    let source_dims = (img.width(), img.height());
    if source_dims.0 == 0 || source_dims.1 == 0 {
        return Err("source image has zero dimensions".into());
    }
    let name = sanitize_icon_name(icon_name);

    let mut cursor = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut cursor);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut assets = 0usize;

    for d in DENSITIES {
        let png = render_png(&img, d.size)?;
        let path = format!("res/mipmap-{}/{}.png", d.qualifier, name);
        zip.start_file(&path, opts)
            .map_err(|e| format!("zip start {path}: {e}"))?;
        zip.write_all(&png)
            .map_err(|e| format!("zip write {path}: {e}"))?;
        assets += 1;

        if round {
            let rpng = render_round_png(&img, d.size)?;
            let rpath = format!("res/mipmap-{}/{}_round.png", d.qualifier, name);
            zip.start_file(&rpath, opts)
                .map_err(|e| format!("zip start {rpath}: {e}"))?;
            zip.write_all(&rpng)
                .map_err(|e| format!("zip write {rpath}: {e}"))?;
            assets += 1;
        }
    }

    // Google Play store listing icon (512px).
    let play = render_png(&img, PLAY_STORE_SIZE)?;
    zip.start_file("play-store-icon.png", opts)
        .map_err(|e| format!("zip start play-store-icon.png: {e}"))?;
    zip.write_all(&play)
        .map_err(|e| format!("zip write play-store-icon.png: {e}"))?;
    assets += 1;

    zip.finish().map_err(|e| format!("finalize zip: {e}"))?;

    Ok((
        cursor.into_inner(),
        Summary { assets, icon_name: name, source_dims },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    /// Build a tiny solid-colour PNG to feed the generator.
    fn sample_png(w: u32, h: u32) -> Vec<u8> {
        let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            w,
            h,
            image::Rgba([10, 120, 220, 255]),
        ));
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    #[test]
    fn sanitize_lowercases_and_replaces_invalid() {
        assert_eq!(sanitize_icon_name("My App Icon!"), "my_app_icon");
        assert_eq!(sanitize_icon_name("ic_launcher"), "ic_launcher");
        assert_eq!(sanitize_icon_name("LOGO-2024"), "logo_2024");
    }

    #[test]
    fn sanitize_empty_or_leading_digit_falls_back() {
        assert_eq!(sanitize_icon_name(""), "ic_launcher");
        assert_eq!(sanitize_icon_name("   "), "ic_launcher");
        assert_eq!(sanitize_icon_name("123"), "ic_launcher");
        assert_eq!(sanitize_icon_name("!!!"), "ic_launcher");
    }

    #[test]
    fn density_table_is_the_standard_set() {
        let sizes: Vec<u32> = DENSITIES.iter().map(|d| d.size).collect();
        assert_eq!(sizes, vec![48, 72, 96, 144, 192]);
        // strictly ascending
        assert!(sizes.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn generate_produces_all_buckets_plus_play_store() {
        let src = sample_png(256, 256);
        let (zip_bytes, summary) = generate_zip(&src, "My App", false).unwrap();
        assert_eq!(summary.assets, DENSITIES.len() + 1);
        assert_eq!(summary.icon_name, "my_app");
        assert_eq!(summary.source_dims, (256, 256));

        // Open the produced ZIP and confirm the entry layout + that each PNG
        // decodes at exactly the expected square size.
        let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes)).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        for d in DENSITIES {
            assert!(
                names.contains(&format!("res/mipmap-{}/my_app.png", d.qualifier)),
                "missing bucket {}",
                d.qualifier
            );
        }
        assert!(names.contains(&"play-store-icon.png".to_string()));

        // Verify the xxxhdpi bucket really is 192x192.
        let mut f = archive.by_name("res/mipmap-xxxhdpi/my_app.png").unwrap();
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (192, 192));
    }

    #[test]
    fn play_store_icon_is_512() {
        let src = sample_png(600, 600);
        let (zip_bytes, _) = generate_zip(&src, "ic_launcher", false).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes)).unwrap();
        let mut f = archive.by_name("play-store-icon.png").unwrap();
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (512, 512));
    }

    #[test]
    fn round_adds_a_masked_variant_per_bucket() {
        let src = sample_png(256, 256);
        let (zip_bytes, summary) = generate_zip(&src, "ic_launcher", true).unwrap();
        // one square + one round per density, plus the play-store icon.
        assert_eq!(summary.assets, DENSITIES.len() * 2 + 1);

        let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes)).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        for d in DENSITIES {
            assert!(names.contains(&format!("res/mipmap-{}/ic_launcher_round.png", d.qualifier)));
        }

        // The round mask must zero the corner pixels (fully transparent) while
        // the centre stays opaque.
        let mut f = archive.by_name("res/mipmap-xxxhdpi/ic_launcher_round.png").unwrap();
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).unwrap();
        let rgba = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(rgba.get_pixel(0, 0)[3], 0, "corner must be transparent");
        let (cx, cy) = (rgba.width() / 2, rgba.height() / 2);
        assert_eq!(rgba.get_pixel(cx, cy)[3], 255, "centre must be opaque");
    }

    #[test]
    fn rejects_non_image_input() {
        assert!(generate_zip(b"not an image at all", "ic", false).is_err());
    }
}
