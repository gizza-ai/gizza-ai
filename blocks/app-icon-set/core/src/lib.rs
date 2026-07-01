//! gizza-ai/app-icon-set core — turn one source image into a complete
//! cross-platform app-icon set, bundled into a single ZIP:
//!
//!   * **iOS** — a drop-in Xcode `AppIcon.appiconset/` folder: every iPhone /
//!     iPad / App-Store size (20–1024 pt across @1x/@2x/@3x) plus the
//!     `Contents.json` catalog Xcode needs. The 1024px marketing icon is
//!     flattened onto the background colour (opaque — the App Store rejects
//!     alpha in the marketing icon).
//!   * **Android** — the standard `res/mipmap-*` launcher tree (mdpi 48 →
//!     xxxhdpi 192) plus the 512px Google Play store icon.
//!   * **Web / PWA** — 192 & 512 px icons, a padded 512px **maskable** icon
//!     (source inset into the 80% safe zone over the background), a 180px
//!     `apple-touch-icon.png`, and a ready `manifest.webmanifest`.
//!
//! Pure Rust (`image` + `zip`) — no I/O, no ffmpeg — so it instantiates in the
//! wafer chat runtime as well as the CLI. Downscaling uses Lanczos3.

use std::io::{Cursor, Write};

use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// One entry in the iOS `AppIcon.appiconset` catalog.
#[derive(Debug, Clone, Copy)]
pub struct IosIcon {
    /// Point size string as written in `Contents.json` (e.g. `"20x20"`).
    pub size: &'static str,
    /// Device idiom (`iphone`, `ipad`, `ios-marketing`).
    pub idiom: &'static str,
    /// Render scale (`1x`, `2x`, `3x`).
    pub scale: &'static str,
    /// Rendered pixel dimension (square).
    pub px: u32,
}

/// The full modern iOS app-icon matrix (Xcode "single size" catalogs still
/// accept this classic per-idiom layout, which also works on older toolchains).
pub const IOS_ICONS: &[IosIcon] = &[
    IosIcon { size: "20x20", idiom: "iphone", scale: "2x", px: 40 },
    IosIcon { size: "20x20", idiom: "iphone", scale: "3x", px: 60 },
    IosIcon { size: "29x29", idiom: "iphone", scale: "2x", px: 58 },
    IosIcon { size: "29x29", idiom: "iphone", scale: "3x", px: 87 },
    IosIcon { size: "40x40", idiom: "iphone", scale: "2x", px: 80 },
    IosIcon { size: "40x40", idiom: "iphone", scale: "3x", px: 120 },
    IosIcon { size: "60x60", idiom: "iphone", scale: "2x", px: 120 },
    IosIcon { size: "60x60", idiom: "iphone", scale: "3x", px: 180 },
    IosIcon { size: "20x20", idiom: "ipad", scale: "1x", px: 20 },
    IosIcon { size: "20x20", idiom: "ipad", scale: "2x", px: 40 },
    IosIcon { size: "29x29", idiom: "ipad", scale: "1x", px: 29 },
    IosIcon { size: "29x29", idiom: "ipad", scale: "2x", px: 58 },
    IosIcon { size: "40x40", idiom: "ipad", scale: "1x", px: 40 },
    IosIcon { size: "40x40", idiom: "ipad", scale: "2x", px: 80 },
    IosIcon { size: "76x76", idiom: "ipad", scale: "1x", px: 76 },
    IosIcon { size: "76x76", idiom: "ipad", scale: "2x", px: 152 },
    IosIcon { size: "83.5x83.5", idiom: "ipad", scale: "2x", px: 167 },
    IosIcon { size: "1024x1024", idiom: "ios-marketing", scale: "1x", px: 1024 },
];

/// An Android launcher-icon density bucket: `mipmap-*` qualifier and its size.
#[derive(Debug, Clone, Copy)]
pub struct Density {
    pub qualifier: &'static str,
    pub size: u32,
}

/// The standard launcher-icon density buckets, smallest to largest.
pub const ANDROID_DENSITIES: &[Density] = &[
    Density { qualifier: "mdpi", size: 48 },
    Density { qualifier: "hdpi", size: 72 },
    Density { qualifier: "xhdpi", size: 96 },
    Density { qualifier: "xxhdpi", size: 144 },
    Density { qualifier: "xxxhdpi", size: 192 },
];

/// The Google Play store listing icon size (512px PNG).
pub const PLAY_STORE_SIZE: u32 = 512;

/// PWA / web icon sizes emitted alongside the manifest.
pub const WEB_ICON_SIZES: &[u32] = &[192, 512];
/// Size of the padded PWA maskable icon and the plain 512 icon.
pub const MASKABLE_SIZE: u32 = 512;
/// iOS home-screen `apple-touch-icon.png` size (also useful for PWAs on iOS).
pub const APPLE_TOUCH_SIZE: u32 = 180;
/// Fraction of the maskable canvas the source occupies (the 80% safe zone).
pub const MASKABLE_CONTENT_FRAC: f32 = 0.8;

/// Which platforms to include in the bundle.
#[derive(Debug, Clone, Copy)]
pub struct Platforms {
    pub ios: bool,
    pub android: bool,
    pub web: bool,
}

impl Default for Platforms {
    fn default() -> Self {
        Platforms { ios: true, android: true, web: true }
    }
}

/// Options controlling the generated set.
#[derive(Debug, Clone)]
pub struct Options {
    /// Which platform folders to emit.
    pub platforms: Platforms,
    /// `name` / `short_name` written into the PWA manifest.
    pub app_name: String,
    /// Background colour used to (a) flatten the opaque iOS 1024 marketing icon
    /// and (b) fill the padding around the maskable PWA icon.
    pub background: Rgba<u8>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            platforms: Platforms::default(),
            app_name: "My App".to_string(),
            background: Rgba([255, 255, 255, 255]),
        }
    }
}

/// Summary of a generation run (for the caller's user-facing message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// Number of PNG icon files written into the ZIP.
    pub icons: usize,
    /// Platform folders included, in a stable order (e.g. `["ios","android","web"]`).
    pub platforms: Vec<String>,
    /// Source image dimensions (width, height) before resizing.
    pub source_dims: (u32, u32),
}

/// Parse a CSS-style hex colour (`#rgb`, `#rrggbb`, `#rrggbbaa`, with or without
/// the leading `#`) into an RGBA pixel. Unparseable input falls back to opaque
/// white so a bad value never fails the whole run.
pub fn parse_hex_color(raw: &str) -> Rgba<u8> {
    let s = raw.trim().trim_start_matches('#');
    let white = Rgba([255, 255, 255, 255]);
    let hex = |a: &str| u8::from_str_radix(a, 16).ok();
    match s.len() {
        3 => {
            let d: Option<Vec<u8>> = s.chars().map(|c| hex(&format!("{c}{c}"))).collect();
            d.map(|v| Rgba([v[0], v[1], v[2], 255])).unwrap_or(white)
        }
        6 => {
            let r = hex(&s[0..2]);
            let g = hex(&s[2..4]);
            let b = hex(&s[4..6]);
            match (r, g, b) {
                (Some(r), Some(g), Some(b)) => Rgba([r, g, b, 255]),
                _ => white,
            }
        }
        8 => {
            let r = hex(&s[0..2]);
            let g = hex(&s[2..4]);
            let b = hex(&s[4..6]);
            let a = hex(&s[6..8]);
            match (r, g, b, a) {
                (Some(r), Some(g), Some(b), Some(a)) => Rgba([r, g, b, a]),
                _ => white,
            }
        }
        _ => white,
    }
}

/// Resize `img` to a square `size`x`size` PNG (RGBA, Lanczos3) and return bytes.
fn render_png(img: &DynamicImage, size: u32) -> Result<Vec<u8>, String> {
    let resized = img.resize_exact(size, size, FilterType::Lanczos3);
    let mut buf = Cursor::new(Vec::new());
    resized
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("encode {size}px PNG: {e}"))?;
    Ok(buf.into_inner())
}

/// Resize `img` to `size`x`size`, then composite it over an **opaque** `bg` so
/// the result has no alpha channel (required for the iOS App-Store marketing
/// icon, which Apple rejects if it contains transparency).
fn render_png_flat(img: &DynamicImage, size: u32, bg: Rgba<u8>) -> Result<Vec<u8>, String> {
    let fg = img.resize_exact(size, size, FilterType::Lanczos3).to_rgba8();
    let mut canvas = RgbaImage::from_pixel(size, size, Rgba([bg[0], bg[1], bg[2], 255]));
    for (x, y, px) in fg.enumerate_pixels() {
        let a = px[3] as f32 / 255.0;
        let base = canvas.get_pixel(x, y);
        let blend = |f: u8, b: u8| ((f as f32 * a) + (b as f32 * (1.0 - a))).round() as u8;
        canvas.put_pixel(
            x,
            y,
            Rgba([blend(px[0], base[0]), blend(px[1], base[1]), blend(px[2], base[2]), 255]),
        );
    }
    let mut buf = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(canvas)
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("encode flattened {size}px PNG: {e}"))?;
    Ok(buf.into_inner())
}

/// Render a PWA **maskable** icon: fill a `size`x`size` opaque `bg`, then centre
/// the source scaled to `frac` of the canvas so its content sits inside the
/// maskable safe zone (Android/Chrome crop up to ~10% off each edge).
fn render_maskable(
    img: &DynamicImage,
    size: u32,
    bg: Rgba<u8>,
    frac: f32,
) -> Result<Vec<u8>, String> {
    let inner = ((size as f32) * frac).round().max(1.0) as u32;
    let fg = img.resize_exact(inner, inner, FilterType::Lanczos3).to_rgba8();
    let mut canvas = RgbaImage::from_pixel(size, size, Rgba([bg[0], bg[1], bg[2], 255]));
    let off = (size - inner) / 2;
    for (x, y, px) in fg.enumerate_pixels() {
        let a = px[3] as f32 / 255.0;
        let base = canvas.get_pixel(off + x, off + y);
        let blend = |f: u8, b: u8| ((f as f32 * a) + (b as f32 * (1.0 - a))).round() as u8;
        canvas.put_pixel(
            off + x,
            off + y,
            Rgba([blend(px[0], base[0]), blend(px[1], base[1]), blend(px[2], base[2]), 255]),
        );
    }
    let mut buf = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(canvas)
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("encode maskable {size}px PNG: {e}"))?;
    Ok(buf.into_inner())
}

/// Build the iOS `AppIcon.appiconset/Contents.json` catalog referencing one
/// PNG per pixel size (`icon-<px>.png`).
fn ios_contents_json() -> String {
    let mut images = String::new();
    for (i, ic) in IOS_ICONS.iter().enumerate() {
        if i > 0 {
            images.push_str(",\n");
        }
        images.push_str(&format!(
            "    {{\n      \"size\" : \"{}\",\n      \"idiom\" : \"{}\",\n      \"filename\" : \"icon-{}.png\",\n      \"scale\" : \"{}\"\n    }}",
            ic.size, ic.idiom, ic.px, ic.scale
        ));
    }
    format!(
        "{{\n  \"images\" : [\n{images}\n  ],\n  \"info\" : {{\n    \"version\" : 1,\n    \"author\" : \"gizza-ai\"\n  }}\n}}\n"
    )
}

/// Escape a string for embedding as a JSON string value.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
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

/// Build the PWA `manifest.webmanifest` referencing the emitted web icons.
fn web_manifest(app_name: &str) -> String {
    let name = json_escape(app_name.trim());
    format!(
        r##"{{
  "name": "{name}",
  "short_name": "{name}",
  "icons": [
    {{ "src": "icon-192.png", "sizes": "192x192", "type": "image/png", "purpose": "any" }},
    {{ "src": "icon-512.png", "sizes": "512x512", "type": "image/png", "purpose": "any" }},
    {{ "src": "icon-maskable-512.png", "sizes": "512x512", "type": "image/png", "purpose": "maskable" }}
  ],
  "display": "standalone",
  "background_color": "#ffffff",
  "theme_color": "#ffffff"
}}
"##
    )
}

/// Generate the full cross-platform app-icon set from `image_bytes` and return
/// `(zip_bytes, summary)`.
pub fn generate_zip(image_bytes: &[u8], opts: &Options) -> Result<(Vec<u8>, Summary), String> {
    if !opts.platforms.ios && !opts.platforms.android && !opts.platforms.web {
        return Err("no platforms selected: enable at least one of ios, android, web".into());
    }
    let img =
        image::load_from_memory(image_bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let source_dims = (img.width(), img.height());
    if source_dims.0 == 0 || source_dims.1 == 0 {
        return Err("source image has zero dimensions".into());
    }

    let mut cursor = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut cursor);
    let fopts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut icons = 0usize;
    let mut platforms = Vec::new();

    let put = |zip: &mut ZipWriter<&mut Cursor<Vec<u8>>>, path: &str, bytes: &[u8]| {
        zip.start_file(path, fopts)
            .map_err(|e| format!("zip start {path}: {e}"))?;
        zip.write_all(bytes)
            .map_err(|e| format!("zip write {path}: {e}"))?;
        Ok::<(), String>(())
    };

    // ---- iOS: Xcode AppIcon.appiconset ----
    if opts.platforms.ios {
        platforms.push("ios".to_string());
        // Dedup by pixel size — the catalog references one file per size.
        let mut seen: Vec<u32> = Vec::new();
        for ic in IOS_ICONS {
            if seen.contains(&ic.px) {
                continue;
            }
            seen.push(ic.px);
            let png = if ic.idiom == "ios-marketing" {
                render_png_flat(&img, ic.px, opts.background)?
            } else {
                render_png(&img, ic.px)?
            };
            put(&mut zip, &format!("ios/AppIcon.appiconset/icon-{}.png", ic.px), &png)?;
            icons += 1;
        }
        put(
            &mut zip,
            "ios/AppIcon.appiconset/Contents.json",
            ios_contents_json().as_bytes(),
        )?;
    }

    // ---- Android: res/mipmap-* launcher tree + Play store icon ----
    if opts.platforms.android {
        platforms.push("android".to_string());
        for d in ANDROID_DENSITIES {
            let png = render_png(&img, d.size)?;
            put(
                &mut zip,
                &format!("android/res/mipmap-{}/ic_launcher.png", d.qualifier),
                &png,
            )?;
            icons += 1;
        }
        let play = render_png(&img, PLAY_STORE_SIZE)?;
        put(&mut zip, "android/play-store-icon.png", &play)?;
        icons += 1;
    }

    // ---- Web / PWA: icons + maskable + apple-touch + manifest ----
    if opts.platforms.web {
        platforms.push("web".to_string());
        for &size in WEB_ICON_SIZES {
            let png = render_png(&img, size)?;
            put(&mut zip, &format!("web/icon-{size}.png"), &png)?;
            icons += 1;
        }
        let maskable = render_maskable(&img, MASKABLE_SIZE, opts.background, MASKABLE_CONTENT_FRAC)?;
        put(&mut zip, "web/icon-maskable-512.png", &maskable)?;
        icons += 1;

        let apple = render_png(&img, APPLE_TOUCH_SIZE)?;
        put(&mut zip, "web/apple-touch-icon.png", &apple)?;
        icons += 1;

        put(
            &mut zip,
            "web/manifest.webmanifest",
            web_manifest(&opts.app_name).as_bytes(),
        )?;
    }

    zip.finish().map_err(|e| format!("finalize zip: {e}"))?;

    Ok((cursor.into_inner(), Summary { icons, platforms, source_dims }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

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

    fn names(zip_bytes: &[u8]) -> Vec<String> {
        let mut a = zip::ZipArchive::new(Cursor::new(zip_bytes.to_vec())).unwrap();
        (0..a.len())
            .map(|i| a.by_index(i).unwrap().name().to_string())
            .collect()
    }

    fn read_entry(zip_bytes: &[u8], name: &str) -> Vec<u8> {
        let mut a = zip::ZipArchive::new(Cursor::new(zip_bytes.to_vec())).unwrap();
        let mut f = a.by_name(name).unwrap();
        let mut v = Vec::new();
        f.read_to_end(&mut v).unwrap();
        v
    }

    #[test]
    fn hex_parses_all_forms() {
        assert_eq!(parse_hex_color("#fff"), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_hex_color("000000"), Rgba([0, 0, 0, 255]));
        assert_eq!(parse_hex_color("#1080dc"), Rgba([16, 128, 220, 255]));
        assert_eq!(parse_hex_color("#10203040"), Rgba([16, 32, 48, 64]));
        // invalid → white fallback
        assert_eq!(parse_hex_color("nope"), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_hex_color("#12"), Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn ios_table_covers_marketing_and_home_screen() {
        let px: Vec<u32> = IOS_ICONS.iter().map(|i| i.px).collect();
        assert!(px.contains(&1024), "App Store marketing icon present");
        assert!(px.contains(&180), "iPhone @3x app icon present");
        assert!(px.contains(&167), "iPad Pro icon present");
        assert!(IOS_ICONS.iter().any(|i| i.idiom == "ios-marketing"));
    }

    #[test]
    fn full_set_lays_out_all_three_platforms() {
        let src = sample_png(1024, 1024);
        let (zip, summary) = generate_zip(&src, &Options::default()).unwrap();
        assert_eq!(summary.platforms, vec!["ios", "android", "web"]);
        assert_eq!(summary.source_dims, (1024, 1024));

        let n = names(&zip);
        assert!(n.contains(&"ios/AppIcon.appiconset/Contents.json".to_string()));
        assert!(n.contains(&"ios/AppIcon.appiconset/icon-1024.png".to_string()));
        assert!(n.contains(&"android/res/mipmap-xxxhdpi/ic_launcher.png".to_string()));
        assert!(n.contains(&"android/play-store-icon.png".to_string()));
        assert!(n.contains(&"web/icon-512.png".to_string()));
        assert!(n.contains(&"web/icon-maskable-512.png".to_string()));
        assert!(n.contains(&"web/apple-touch-icon.png".to_string()));
        assert!(n.contains(&"web/manifest.webmanifest".to_string()));

        // Contents.json is valid-ish JSON with an images array.
        let contents = read_entry(&zip, "ios/AppIcon.appiconset/Contents.json");
        let s = String::from_utf8(contents).unwrap();
        assert!(s.contains("\"images\""));
        assert!(s.contains("\"icon-1024.png\""));

        // xxxhdpi launcher really is 192x192.
        let bytes = read_entry(&zip, "android/res/mipmap-xxxhdpi/ic_launcher.png");
        let d = image::load_from_memory(&bytes).unwrap();
        assert_eq!((d.width(), d.height()), (192, 192));
    }

    #[test]
    fn marketing_icon_is_opaque() {
        let src = sample_png(512, 512);
        let (zip, _) = generate_zip(&src, &Options::default()).unwrap();
        let bytes = read_entry(&zip, "ios/AppIcon.appiconset/icon-1024.png");
        let rgba = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert!(
            rgba.pixels().all(|p| p[3] == 255),
            "App Store marketing icon must have no transparency"
        );
    }

    #[test]
    fn maskable_pads_source_into_safe_zone() {
        // An opaque green source over a red background: the corners must show
        // the background (padding) and the centre must show the source colour.
        let src_img = image::RgbaImage::from_pixel(256, 256, image::Rgba([0, 200, 0, 255]));
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(src_img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let opts = Options { background: Rgba([255, 0, 0, 255]), ..Options::default() };
        let (zip, _) = generate_zip(&buf.into_inner(), &opts).unwrap();
        let bytes = read_entry(&zip, "web/icon-maskable-512.png");
        let rgba = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!((rgba.width(), rgba.height()), (512, 512));
        // Corner = red background padding.
        assert_eq!(rgba.get_pixel(0, 0), &Rgba([255, 0, 0, 255]));
        // Centre = green source.
        assert_eq!(rgba.get_pixel(256, 256), &Rgba([0, 200, 0, 255]));
    }

    #[test]
    fn platform_toggles_scope_the_bundle() {
        let src = sample_png(300, 300);

        let ios_only = Options {
            platforms: Platforms { ios: true, android: false, web: false },
            ..Options::default()
        };
        let (zip, s) = generate_zip(&src, &ios_only).unwrap();
        assert_eq!(s.platforms, vec!["ios"]);
        let n = names(&zip);
        assert!(n.iter().all(|p| p.starts_with("ios/")));

        let web_only = Options {
            platforms: Platforms { ios: false, android: false, web: true },
            ..Options::default()
        };
        let (zip, s) = generate_zip(&src, &web_only).unwrap();
        assert_eq!(s.platforms, vec!["web"]);
        assert!(names(&zip).contains(&"web/manifest.webmanifest".to_string()));
    }

    #[test]
    fn manifest_carries_the_app_name() {
        let src = sample_png(300, 300);
        let opts = Options { app_name: "My \"Cool\" App".to_string(), ..Options::default() };
        let (zip, _) = generate_zip(&src, &opts).unwrap();
        let m = String::from_utf8(read_entry(&zip, "web/manifest.webmanifest")).unwrap();
        assert!(m.contains(r#""name": "My \"Cool\" App""#), "manifest: {m}");
    }

    #[test]
    fn no_platforms_selected_is_an_error() {
        let src = sample_png(128, 128);
        let opts = Options {
            platforms: Platforms { ios: false, android: false, web: false },
            ..Options::default()
        };
        assert!(generate_zip(&src, &opts).is_err());
    }

    #[test]
    fn rejects_non_image_input() {
        assert!(generate_zip(b"not an image at all", &Options::default()).is_err());
    }
}
