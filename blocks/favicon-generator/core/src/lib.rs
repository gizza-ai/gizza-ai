//! gizza-ai/favicon-generator core — turn one source image into a complete
//! favicon bundle: a multi-resolution `favicon.ico`, a set of square PNG icons
//! (16–512 px), an `apple-touch-icon.png`, and a `site.webmanifest` — all packed
//! into a single ZIP. Pure Rust (`image` + `zip`), no I/O / no ffmpeg, so it
//! instantiates in the wafer chat runtime as well as the CLI.

use std::io::{Cursor, Write};

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, ImageEncoder, Rgba, RgbaImage};

/// Default square PNG sizes emitted in the bundle (px). Covers the common
/// browser / PWA / Android Chrome set.
pub const DEFAULT_PNG_SIZES: &[u32] = &[16, 32, 48, 64, 96, 128, 192, 256, 512];

/// Sizes packed into the multi-resolution `favicon.ico` (the classic set).
pub const ICO_SIZES: &[u32] = &[16, 32, 48];

/// Size of the iOS `apple-touch-icon.png`.
pub const APPLE_TOUCH_SIZE: u32 = 180;

/// One generated file in the bundle, ready to be zipped.
#[derive(Debug, Clone, PartialEq)]
pub struct BundleFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Options controlling how the source image is fit into each square icon.
#[derive(Debug, Clone)]
pub struct Options {
    /// How to fit a non-square source into a square icon.
    pub fit: Fit,
    /// Background fill (RGBA) used behind transparent / letterboxed areas when
    /// `fit == Contain`. Ignored for `Cover`.
    pub background: Rgba<u8>,
    /// PNG sizes to emit. Empty → `DEFAULT_PNG_SIZES`.
    pub png_sizes: Vec<u32>,
    /// `name` written into the web manifest.
    pub app_name: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            fit: Fit::Contain,
            background: Rgba([0, 0, 0, 0]),
            png_sizes: DEFAULT_PNG_SIZES.to_vec(),
            app_name: "My App".to_string(),
        }
    }
}

/// Strategy for fitting a non-square source into a square icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Scale to fit entirely inside the square, padding the remainder with the
    /// background color (keeps the whole image, may add margins).
    Contain,
    /// Scale to fill the square then center-crop the overflow (fills the icon,
    /// may trim edges).
    Cover,
}

/// Parse the fit mode from a string (case-insensitive).
pub fn parse_fit(s: &str) -> Result<Fit, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "contain" | "fit" | "pad" => Ok(Fit::Contain),
        "cover" | "crop" | "fill" => Ok(Fit::Cover),
        other => Err(format!(
            "unknown fit '{other}' (expected 'contain' or 'cover')"
        )),
    }
}

/// Parse a `#rgb`, `#rrggbb`, or `#rrggbbaa` color into RGBA.
pub fn parse_color(s: &str) -> Result<Rgba<u8>, String> {
    let h = s.trim().trim_start_matches('#');
    let hx = |a: &str| u8::from_str_radix(a, 16).map_err(|_| format!("bad color '{s}'"));
    let v = match h.len() {
        3 => {
            let r = hx(&h[0..1])?;
            let g = hx(&h[1..2])?;
            let b = hx(&h[2..3])?;
            [r * 17, g * 17, b * 17, 255]
        }
        6 => [hx(&h[0..2])?, hx(&h[2..4])?, hx(&h[4..6])?, 255],
        8 => [
            hx(&h[0..2])?,
            hx(&h[2..4])?,
            hx(&h[4..6])?,
            hx(&h[6..8])?,
        ],
        _ => return Err(format!("bad color '{s}' (use #rgb, #rrggbb, or #rrggbbaa)")),
    };
    Ok(Rgba(v))
}

/// Render the source image into a square `size`×`size` RGBA canvas per the fit mode.
fn render_square(src: &DynamicImage, size: u32, opts: &Options) -> RgbaImage {
    match opts.fit {
        Fit::Cover => {
            // Scale to fill (cover) then center-crop to a square.
            let resized = src.resize_to_fill(size, size, FilterType::Lanczos3);
            resized.to_rgba8()
        }
        Fit::Contain => {
            // Scale to fit inside the square (preserving aspect), then paste
            // centered onto a background-filled canvas.
            let fitted = src.resize(size, size, FilterType::Lanczos3).to_rgba8();
            let mut canvas = RgbaImage::from_pixel(size, size, opts.background);
            let ox = (size - fitted.width()) / 2;
            let oy = (size - fitted.height()) / 2;
            image::imageops::overlay(&mut canvas, &fitted, ox as i64, oy as i64);
            canvas
        }
    }
}

/// Encode an RGBA image as PNG bytes.
fn png_bytes(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("png encode: {e}"))?;
    Ok(out)
}

/// Build the multi-resolution ICO from the given square RGBA images.
fn ico_bytes(frames: &[RgbaImage]) -> Result<Vec<u8>, String> {
    let mut encoded: Vec<IcoFrame> = Vec::with_capacity(frames.len());
    for img in frames {
        let frame = IcoFrame::as_png(
            img.as_raw(),
            img.width(),
            img.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("ico frame: {e}"))?;
        encoded.push(frame);
    }
    let mut out = Vec::new();
    IcoEncoder::new(Cursor::new(&mut out))
        .encode_images(&encoded)
        .map_err(|e| format!("ico encode: {e}"))?;
    Ok(out)
}

/// Build the `site.webmanifest` JSON referencing the generated PNG icons.
fn web_manifest(app_name: &str, sizes: &[u32]) -> String {
    let mut icons = String::new();
    for (i, s) in sizes.iter().enumerate() {
        if i > 0 {
            icons.push_str(",\n");
        }
        icons.push_str(&format!(
            "    {{ \"src\": \"/favicon-{s}x{s}.png\", \"sizes\": \"{s}x{s}\", \"type\": \"image/png\" }}"
        ));
    }
    let name = json_escape(app_name);
    format!(
        "{{\n  \"name\": \"{name}\",\n  \"short_name\": \"{name}\",\n  \"icons\": [\n{icons}\n  ],\n  \"theme_color\": \"#ffffff\",\n  \"background_color\": \"#ffffff\",\n  \"display\": \"standalone\"\n}}\n"
    )
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Generate the full favicon bundle as a list of named files (un-zipped).
pub fn generate_files(src_bytes: &[u8], opts: &Options) -> Result<Vec<BundleFile>, String> {
    let src = image::load_from_memory(src_bytes).map_err(|e| format!("decode image: {e}"))?;

    let mut png_sizes: Vec<u32> = if opts.png_sizes.is_empty() {
        DEFAULT_PNG_SIZES.to_vec()
    } else {
        opts.png_sizes.clone()
    };
    png_sizes.retain(|s| (1..=1024).contains(s));
    png_sizes.sort_unstable();
    png_sizes.dedup();
    if png_sizes.is_empty() {
        return Err("no valid icon sizes (1..=1024)".into());
    }

    let mut files: Vec<BundleFile> = Vec::new();

    // Square PNGs at each requested size.
    for &s in &png_sizes {
        let img = render_square(&src, s, opts);
        files.push(BundleFile {
            name: format!("favicon-{s}x{s}.png"),
            bytes: png_bytes(&img)?,
        });
    }

    // Multi-resolution favicon.ico (ICO frames cap at 256px).
    let ico_frames: Vec<RgbaImage> = ICO_SIZES
        .iter()
        .map(|&s| render_square(&src, s, opts))
        .collect();
    files.push(BundleFile {
        name: "favicon.ico".to_string(),
        bytes: ico_bytes(&ico_frames)?,
    });

    // apple-touch-icon.png (iOS home-screen icon).
    let apple = render_square(&src, APPLE_TOUCH_SIZE, opts);
    files.push(BundleFile {
        name: "apple-touch-icon.png".to_string(),
        bytes: png_bytes(&apple)?,
    });

    // Web manifest referencing the PNGs.
    files.push(BundleFile {
        name: "site.webmanifest".to_string(),
        bytes: web_manifest(&opts.app_name, &png_sizes).into_bytes(),
    });

    Ok(files)
}

/// Pack the generated bundle files into a single ZIP archive (stored/deflated).
pub fn zip_bundle(files: &[BundleFile]) -> Result<Vec<u8>, String> {
    use zip::write::{SimpleFileOptions, ZipWriter};
    use zip::CompressionMethod;

    let mut buf = Vec::new();
    {
        let mut zw = ZipWriter::new(Cursor::new(&mut buf));
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for f in files {
            zw.start_file(&f.name, opts)
                .map_err(|e| format!("zip start_file: {e}"))?;
            zw.write_all(&f.bytes)
                .map_err(|e| format!("zip write: {e}"))?;
        }
        zw.finish().map_err(|e| format!("zip finish: {e}"))?;
    }
    Ok(buf)
}

/// End-to-end: source image bytes → favicon bundle ZIP bytes.
pub fn generate(src_bytes: &[u8], opts: &Options) -> Result<Vec<u8>, String> {
    let files = generate_files(src_bytes, opts)?;
    zip_bundle(&files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn sample_png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba([10, 120, 220, 255]));
        png_bytes(&img).unwrap()
    }

    #[test]
    fn generates_full_bundle_zip() {
        let src = sample_png(300, 300);
        let opts = Options {
            app_name: "Test".into(),
            ..Default::default()
        };
        let files = generate_files(&src, &opts).unwrap();
        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"favicon.ico"));
        assert!(names.contains(&"favicon-16x16.png"));
        assert!(names.contains(&"favicon-512x512.png"));
        assert!(names.contains(&"apple-touch-icon.png"));
        assert!(names.contains(&"site.webmanifest"));

        // The ZIP must be a readable archive containing every file.
        let zip = zip_bundle(&files).unwrap();
        let mut ar = zip::ZipArchive::new(Cursor::new(zip)).unwrap();
        assert_eq!(ar.len(), files.len());
        let mut ico = ar.by_name("favicon.ico").unwrap();
        let mut ib = Vec::new();
        ico.read_to_end(&mut ib).unwrap();
        // ICO magic: 00 00 01 00.
        assert_eq!(&ib[0..4], &[0, 0, 1, 0]);
    }

    #[test]
    fn png_size_is_exact_square() {
        let src = sample_png(120, 80); // non-square
        let files = generate_files(&src, &Options::default()).unwrap();
        let f = files
            .iter()
            .find(|f| f.name == "favicon-32x32.png")
            .unwrap();
        let img = image::load_from_memory(&f.bytes).unwrap();
        assert_eq!(img.width(), 32);
        assert_eq!(img.height(), 32);
    }

    #[test]
    fn manifest_references_sizes() {
        let m = web_manifest("Acme \"App\"", &[16, 32, 192]);
        assert!(m.contains("Acme \\\"App\\\"")); // escaped name
        assert!(m.contains("\"sizes\": \"192x192\""));
        assert!(m.contains("/favicon-16x16.png"));
    }

    #[test]
    fn rejects_garbage_input() {
        assert!(generate(b"not an image", &Options::default()).is_err());
    }

    #[test]
    fn fit_modes_parse() {
        assert_eq!(parse_fit("contain").unwrap(), Fit::Contain);
        assert_eq!(parse_fit("COVER").unwrap(), Fit::Cover);
        assert!(parse_fit("nope").is_err());
    }

    #[test]
    fn color_parses() {
        assert_eq!(parse_color("#fff").unwrap(), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_color("#10203040").unwrap(), Rgba([16, 32, 48, 64]));
        assert!(parse_color("#zz").is_err());
    }
}
