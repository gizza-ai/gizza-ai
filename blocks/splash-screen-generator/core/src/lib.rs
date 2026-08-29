//! gizza-ai/splash-screen-generator core — composite one logo over a solid
//! background and export it at every common launch/splash-screen resolution,
//! bundled into a single ZIP:
//!
//!   * **iOS / iPadOS** — one launch image per current iPhone and iPad
//!     resolution (iPhone SE 1st gen → iPhone 16 Pro Max, iPad 9.7" → iPad Pro
//!     12.9"), portrait and/or landscape, plus a ready-to-paste
//!     `apple-touch-startup-image.html` snippet whose `<link>` media queries
//!     match each device's CSS size and pixel ratio.
//!   * **Android** — the classic `res/drawable-port-*` / `res/drawable-land-*`
//!     density buckets (mdpi → xxxhdpi) plus the Android 12+ splash **icon**
//!     (1152 px canvas with the artwork inside the 768 px / 192 dp safe box).
//!   * An optional **dark** variant of everything (`prefers-color-scheme: dark`
//!     link tags on iOS, `-night-` resource qualifiers on Android).
//!
//! Pure Rust (`image` + `zip`) — no ffmpeg, no I/O — so the block instantiates
//! in the wafer chat runtime as well as the CLI. Launch screens are opaque, so
//! every canvas is rendered as RGB with the logo alpha-composited over the
//! background colour.

use std::collections::HashMap;
use std::io::{Cursor, Write};

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, ImageEncoder, ImageReader, Rgb, RgbImage, RgbaImage};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// Largest source logo we will decode, in pixels. A 8 MP cap keeps the decoded
/// RGBA raster (4 bytes/px) inside the 64 MiB wasm sandbox alongside the
/// largest splash canvas.
pub const MAX_SOURCE_PIXELS: u64 = 8_000_000;

/// One Apple device family sharing a launch-image resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Device {
    /// Human-readable device family (used in the HTML snippet's comments).
    pub name: &'static str,
    /// Portrait CSS width in points — the `device-width` in the media query.
    pub css_w: u32,
    /// Portrait CSS height in points — the `device-height` in the media query.
    pub css_h: u32,
    /// Device pixel ratio (`-webkit-device-pixel-ratio`).
    pub dpr: u32,
}

impl Device {
    /// Portrait launch-image size in real pixels.
    pub fn portrait(&self) -> (u32, u32) {
        (self.css_w * self.dpr, self.css_h * self.dpr)
    }
    /// Landscape launch-image size in real pixels (the transpose).
    pub fn landscape(&self) -> (u32, u32) {
        (self.css_h * self.dpr, self.css_w * self.dpr)
    }
}

/// Every distinct iPhone / iPad launch-image resolution in current use.
/// `device-width` / `device-height` stay portrait-oriented in the media query
/// even for the landscape image — that is how Safari reports them.
pub const IOS_DEVICES: &[Device] = &[
    Device {
        name: "iPhone SE (1st gen), 5, 5s",
        css_w: 320,
        css_h: 568,
        dpr: 2,
    },
    Device {
        name: "iPhone SE (2nd/3rd gen), 8, 7, 6s",
        css_w: 375,
        css_h: 667,
        dpr: 2,
    },
    Device {
        name: "iPhone 8 Plus, 7 Plus, 6s Plus",
        css_w: 414,
        css_h: 736,
        dpr: 3,
    },
    Device {
        name: "iPhone X, XS, 11 Pro, 12 mini, 13 mini",
        css_w: 375,
        css_h: 812,
        dpr: 3,
    },
    Device {
        name: "iPhone XR, 11",
        css_w: 414,
        css_h: 896,
        dpr: 2,
    },
    Device {
        name: "iPhone XS Max, 11 Pro Max",
        css_w: 414,
        css_h: 896,
        dpr: 3,
    },
    Device {
        name: "iPhone 12, 12 Pro, 13, 13 Pro, 14",
        css_w: 390,
        css_h: 844,
        dpr: 3,
    },
    Device {
        name: "iPhone 12 Pro Max, 13 Pro Max, 14 Plus",
        css_w: 428,
        css_h: 926,
        dpr: 3,
    },
    Device {
        name: "iPhone 14 Pro, 15, 15 Pro, 16",
        css_w: 393,
        css_h: 852,
        dpr: 3,
    },
    Device {
        name: "iPhone 14 Pro Max, 15 Plus, 15 Pro Max, 16 Plus",
        css_w: 430,
        css_h: 932,
        dpr: 3,
    },
    Device {
        name: "iPhone 16 Pro",
        css_w: 402,
        css_h: 874,
        dpr: 3,
    },
    Device {
        name: "iPhone 16 Pro Max",
        css_w: 440,
        css_h: 956,
        dpr: 3,
    },
    Device {
        name: "iPad (9.7-inch)",
        css_w: 768,
        css_h: 1024,
        dpr: 2,
    },
    Device {
        name: "iPad (10.2-inch)",
        css_w: 810,
        css_h: 1080,
        dpr: 2,
    },
    Device {
        name: "iPad Air / iPad (10.9-inch)",
        css_w: 820,
        css_h: 1180,
        dpr: 2,
    },
    Device {
        name: "iPad Pro (10.5-inch), Air (3rd gen)",
        css_w: 834,
        css_h: 1112,
        dpr: 2,
    },
    Device {
        name: "iPad Pro (11-inch), Air (11-inch)",
        css_w: 834,
        css_h: 1194,
        dpr: 2,
    },
    Device {
        name: "iPad Pro (12.9-inch)",
        css_w: 1024,
        css_h: 1366,
        dpr: 2,
    },
];

/// One Android density bucket and its portrait splash bitmap size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Density {
    /// Resource qualifier (`mdpi`, `hdpi`, …).
    pub qualifier: &'static str,
    /// Portrait bitmap width in pixels.
    pub w: u32,
    /// Portrait bitmap height in pixels.
    pub h: u32,
}

/// The classic full-bleed splash bitmap buckets shipped by the Cordova/Capacitor
/// generators, smallest to largest.
pub const ANDROID_DENSITIES: &[Density] = &[
    Density {
        qualifier: "mdpi",
        w: 320,
        h: 480,
    },
    Density {
        qualifier: "hdpi",
        w: 480,
        h: 800,
    },
    Density {
        qualifier: "xhdpi",
        w: 720,
        h: 1280,
    },
    Density {
        qualifier: "xxhdpi",
        w: 960,
        h: 1600,
    },
    Density {
        qualifier: "xxxhdpi",
        w: 1280,
        h: 1920,
    },
];

/// Android 12+ splash-screen icon canvas: 288 dp at xxxhdpi.
pub const ANDROID_ICON_CANVAS: u32 = 1152;
/// Artwork safe box inside that canvas: 192 dp at xxxhdpi.
pub const ANDROID_ICON_CONTENT: u32 = 768;

/// Which orientations to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Portrait,
    Landscape,
    Both,
}

impl Orientation {
    /// Parse the descriptor's `orientation` enum value.
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "portrait" => Ok(Orientation::Portrait),
            "landscape" => Ok(Orientation::Landscape),
            "both" => Ok(Orientation::Both),
            other => Err(format!(
                "orientation must be portrait, landscape or both, got \"{other}\""
            )),
        }
    }
    fn portrait(self) -> bool {
        matches!(self, Orientation::Portrait | Orientation::Both)
    }
    fn landscape(self) -> bool {
        matches!(self, Orientation::Landscape | Orientation::Both)
    }
}

/// Output encoding for every generated screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpeg,
}

impl Format {
    /// Parse the descriptor's `format` enum value.
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "png" => Ok(Format::Png),
            "jpeg" | "jpg" => Ok(Format::Jpeg),
            other => Err(format!("format must be png or jpeg, got \"{other}\"")),
        }
    }
    /// File extension used inside the ZIP.
    pub fn ext(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg => "jpg",
        }
    }
}

/// Everything that shapes a generation run.
#[derive(Debug, Clone)]
pub struct Options {
    /// Background colour behind the logo (opaque RGB).
    pub background: [u8; 3],
    /// Optional second background producing a full dark-variant set.
    pub dark_background: Option<[u8; 3]>,
    /// Logo long edge as a fraction of the canvas's SHORTER side (0.05–0.9).
    pub logo_scale: f32,
    pub orientation: Orientation,
    pub format: Format,
    /// JPEG quality 1–100 (ignored for PNG).
    pub quality: u8,
    pub ios: bool,
    pub android: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            background: [255, 255, 255],
            dark_background: None,
            logo_scale: 0.4,
            orientation: Orientation::Portrait,
            format: Format::Png,
            quality: 82,
            ios: true,
            android: true,
        }
    }
}

/// Summary of a generation run, for the caller's user-facing message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// Number of rendered splash images written into the ZIP.
    pub images: usize,
    /// Total number of files (images + snippet + README).
    pub files: usize,
    /// Platform folders included, in a stable order.
    pub platforms: Vec<String>,
    /// Source logo dimensions before scaling.
    pub source_dims: (u32, u32),
    /// True when a dark-variant set was also emitted.
    pub dark: bool,
}

/// Parse a CSS-ish colour into opaque RGB. Accepts `#rgb`, `#rrggbb`,
/// `#rrggbbaa` (alpha is flattened over white — launch screens are opaque),
/// `rgb(r, g, b)`, and the handful of names people actually type for a splash
/// background. The `#` is optional.
pub fn parse_color(raw: &str) -> Result<[u8; 3], String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("colour is empty: expected a hex value like #101418".to_string());
    }
    let lower = s.to_ascii_lowercase();
    match lower.as_str() {
        "white" => return Ok([255, 255, 255]),
        "black" => return Ok([0, 0, 0]),
        "red" => return Ok([255, 0, 0]),
        "green" => return Ok([0, 128, 0]),
        "blue" => return Ok([0, 0, 255]),
        "gray" | "grey" => return Ok([128, 128, 128]),
        _ => {}
    }
    if let Some(inner) = lower.strip_prefix("rgb(").and_then(|v| v.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() != 3 {
            return Err(format!(
                "expected rgb(r, g, b) with three 0-255 channels, got \"{s}\""
            ));
        }
        let mut out = [0u8; 3];
        for (i, p) in parts.iter().enumerate() {
            out[i] = p.parse::<u8>().map_err(|_| {
                format!(
                    "expected an integer 0-255 for rgb() channel {}, got \"{p}\"",
                    i + 1
                )
            })?;
        }
        return Ok(out);
    }
    let hex = lower.strip_prefix('#').unwrap_or(&lower);
    let byte = |a: &str| -> Result<u8, String> {
        u8::from_str_radix(a, 16).map_err(|_| format!("\"{a}\" is not a hex byte in \"{s}\""))
    };
    match hex.len() {
        3 => {
            let c: Vec<char> = hex.chars().collect();
            Ok([
                byte(&format!("{}{}", c[0], c[0]))?,
                byte(&format!("{}{}", c[1], c[1]))?,
                byte(&format!("{}{}", c[2], c[2]))?,
            ])
        }
        6 => Ok([byte(&hex[0..2])?, byte(&hex[2..4])?, byte(&hex[4..6])?]),
        8 => {
            let a = byte(&hex[6..8])? as f32 / 255.0;
            let over_white = |v: u8| ((v as f32 * a) + 255.0 * (1.0 - a)).round() as u8;
            Ok([
                over_white(byte(&hex[0..2])?),
                over_white(byte(&hex[2..4])?),
                over_white(byte(&hex[4..6])?),
            ])
        }
        _ => Err(format!(
            "expected a colour like #101418, #fff, #rrggbbaa, rgb(16, 20, 24) or a basic name, got \"{s}\""
        )),
    }
}

/// Decode the source logo, refusing anything too large to raster inside the
/// wasm sandbox. The dimensions are read from the header first so an oversized
/// file errors with advice instead of OOM-trapping mid-decode.
fn decode_logo(bytes: &[u8]) -> Result<DynamicImage, String> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("could not read the image header: {e}"))?;
    if reader.format().is_none() {
        return Err(
            "unrecognised image format: expected PNG, JPEG, WebP, GIF or BMP (SVG is not supported)"
                .to_string(),
        );
    }
    let (w, h) = reader
        .into_dimensions()
        .map_err(|e| format!("could not read the image dimensions: {e}"))?;
    if w == 0 || h == 0 {
        return Err("source logo has zero dimensions".to_string());
    }
    let px = w as u64 * h as u64;
    if px > MAX_SOURCE_PIXELS {
        return Err(format!(
            "source logo is {w}x{h} ({:.1} megapixels), over the {:.0} megapixel limit — re-export it smaller (1024x1024 is plenty for a splash logo)",
            px as f64 / 1e6,
            MAX_SOURCE_PIXELS as f64 / 1e6
        ));
    }
    ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("could not read the image header: {e}"))?
        .decode()
        .map_err(|e| format!("could not decode the image: {e}"))
}

/// Scale the logo so its LONGEST edge is `max_edge`, preserving aspect ratio.
fn scaled_logo(src: &DynamicImage, max_edge: u32) -> RgbaImage {
    let max_edge = max_edge.max(1);
    src.resize(max_edge, max_edge, FilterType::Lanczos3)
        .to_rgba8()
}

/// Fill a `w`x`h` canvas with `bg` and alpha-composite `logo` in the centre.
fn compose(w: u32, h: u32, bg: [u8; 3], logo: &RgbaImage) -> RgbImage {
    let mut canvas = RgbImage::from_pixel(w, h, Rgb(bg));
    let ox = (w.saturating_sub(logo.width())) / 2;
    let oy = (h.saturating_sub(logo.height())) / 2;
    for (x, y, px) in logo.enumerate_pixels() {
        let (cx, cy) = (ox + x, oy + y);
        if cx >= w || cy >= h {
            continue;
        }
        let a = px[3] as f32 / 255.0;
        if a <= 0.0 {
            continue;
        }
        let base = canvas.get_pixel(cx, cy).0;
        let blend = |f: u8, b: u8| ((f as f32 * a) + (b as f32 * (1.0 - a))).round() as u8;
        canvas.put_pixel(
            cx,
            cy,
            Rgb([
                blend(px[0], base[0]),
                blend(px[1], base[1]),
                blend(px[2], base[2]),
            ]),
        );
    }
    canvas
}

/// Encode a composited canvas. PNG uses fast deflate with the `Up` filter —
/// splash canvases are mostly identical rows, so that pair is both the quickest
/// and the smallest here. JPEG honours `quality`.
fn encode(img: &RgbImage, fmt: Format, quality: u8) -> Result<Vec<u8>, String> {
    let (w, h) = (img.width(), img.height());
    let mut buf: Vec<u8> = Vec::new();
    match fmt {
        Format::Png => PngEncoder::new_with_quality(&mut buf, CompressionType::Fast, PngFilter::Up)
            .write_image(img.as_raw(), w, h, ExtendedColorType::Rgb8)
            .map_err(|e| format!("encode {w}x{h} PNG: {e}"))?,
        Format::Jpeg => JpegEncoder::new_with_quality(&mut buf, quality)
            .write_image(img.as_raw(), w, h, ExtendedColorType::Rgb8)
            .map_err(|e| format!("encode {w}x{h} JPEG: {e}"))?,
    }
    Ok(buf)
}

/// Renders and caches one composited canvas per (width, height) per theme —
/// several devices share a resolution and iOS landscape is the transpose of
/// portrait, so each distinct canvas is only ever encoded once.
struct Renderer<'a> {
    src: &'a DynamicImage,
    opts: &'a Options,
    logos: HashMap<u32, RgbaImage>,
    cache: HashMap<(bool, u32, u32), Vec<u8>>,
    rendered: usize,
}

impl<'a> Renderer<'a> {
    fn new(src: &'a DynamicImage, opts: &'a Options) -> Self {
        Renderer {
            src,
            opts,
            logos: HashMap::new(),
            cache: HashMap::new(),
            rendered: 0,
        }
    }

    /// Encoded bytes for a `w`x`h` screen whose logo occupies `logo_scale` of
    /// the shorter side. `dark` picks the background.
    fn screen(&mut self, w: u32, h: u32, dark: bool) -> Result<&[u8], String> {
        let edge = ((w.min(h) as f32) * self.opts.logo_scale).round().max(1.0) as u32;
        self.canvas(w, h, dark, edge)
    }

    fn canvas(&mut self, w: u32, h: u32, dark: bool, logo_edge: u32) -> Result<&[u8], String> {
        let key = (dark, w, h);
        if !self.cache.contains_key(&key) {
            if !self.logos.contains_key(&logo_edge) {
                self.logos
                    .insert(logo_edge, scaled_logo(self.src, logo_edge));
            }
            let logo = &self.logos[&logo_edge];
            let bg = if dark {
                self.opts.dark_background.unwrap_or(self.opts.background)
            } else {
                self.opts.background
            };
            let bytes = encode(
                &compose(w, h, bg, logo),
                self.opts.format,
                self.opts.quality,
            )?;
            self.cache.insert(key, bytes);
            self.rendered += 1;
        }
        Ok(&self.cache[&key])
    }
}

/// Build the `<link rel="apple-touch-startup-image">` snippet. When a dark set
/// exists, both variants are qualified with `prefers-color-scheme` so Safari
/// picks the right one.
fn ios_html(opts: &Options, ext: &str) -> String {
    let mut out = String::new();
    out.push_str(
        "<!-- iOS launch images. Paste inside <head> and adjust the href prefix to wherever you host the PNGs. -->\n",
    );
    let section = |dark: bool, out: &mut String| {
        let dir = if dark { "dark/" } else { "" };
        let scheme = match (opts.dark_background.is_some(), dark) {
            (false, _) => String::new(),
            (true, false) => " and (prefers-color-scheme: light)".to_string(),
            (true, true) => " and (prefers-color-scheme: dark)".to_string(),
        };
        for d in IOS_DEVICES {
            for (is_portrait, (w, h)) in [(true, d.portrait()), (false, d.landscape())] {
                if is_portrait && !opts.orientation.portrait() {
                    continue;
                }
                if !is_portrait && !opts.orientation.landscape() {
                    continue;
                }
                let orient = if is_portrait { "portrait" } else { "landscape" };
                out.push_str(&format!(
                    "<!-- {} -->\n<link rel=\"apple-touch-startup-image\" media=\"screen and (device-width: {}px) and (device-height: {}px) and (-webkit-device-pixel-ratio: {}) and (orientation: {orient}){scheme}\" href=\"{dir}apple-touch-startup-image-{w}x{h}.{ext}\">\n",
                    d.name, d.css_w, d.css_h, d.dpr
                ));
            }
        }
    };
    section(false, &mut out);
    if opts.dark_background.is_some() {
        out.push('\n');
        section(true, &mut out);
    }
    out
}

/// Human-readable placement instructions shipped inside the archive.
fn readme(opts: &Options, summary_images: usize, ext: &str) -> String {
    let mut s = String::new();
    s.push_str("Splash / launch screens\n=======================\n\n");
    s.push_str(&format!(
        "{summary_images} image(s), background {}, logo at {:.0}% of the shorter edge, {} orientation, .{ext} output.\n\n",
        hex_of(opts.background),
        opts.logo_scale * 100.0,
        match opts.orientation {
            Orientation::Portrait => "portrait",
            Orientation::Landscape => "landscape",
            Orientation::Both => "portrait + landscape",
        }
    ));
    if opts.ios {
        s.push_str(
            "ios/\n  One launch image per iPhone / iPad resolution, named by pixel size.\n  \
             apple-touch-startup-image.html holds the <link> tags for a PWA: paste them into\n  \
             <head> and fix up the href prefix. For a native app, drop the images into your\n  \
             asset catalog (modern Xcode projects normally use a LaunchScreen storyboard\n  \
             instead, in which case only the PWA snippet is relevant).\n\n",
        );
    }
    if opts.android {
        s.push_str(
            "android/res/\n  drawable-port-*/ and drawable-land-*/ hold the classic full-bleed\n  \
             splash bitmaps for each density bucket — reference them from your theme's\n  \
             windowBackground. drawable/splash_icon.png is the Android 12+ splash icon:\n  \
             a 1152px canvas (288dp at xxxhdpi) with the artwork inside the central 768px\n  \
             (192dp) safe box. Wire it up with windowSplashScreenAnimatedIcon; if you also\n  \
             set windowSplashScreenIconBackgroundColor the system masks to 160dp, so keep\n  \
             important detail central.\n\n",
        );
    }
    if opts.dark_background.is_some() {
        s.push_str(
            "Dark variants live in ios/dark/ and in the Android -night- resource folders.\n\n",
        );
    }
    s.push_str("Generated locally — the logo was never uploaded anywhere.\n");
    s
}

fn hex_of(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// Generate the full splash-screen set from `image_bytes` and return
/// `(zip_bytes, summary)`.
pub fn generate_zip(image_bytes: &[u8], opts: &Options) -> Result<(Vec<u8>, Summary), String> {
    if !opts.ios && !opts.android {
        return Err("no platforms selected: enable at least one of ios, android".to_string());
    }
    if !(0.05..=0.9).contains(&opts.logo_scale) {
        return Err(format!(
            "logo_scale must be between 0.05 and 0.9, got {}",
            opts.logo_scale
        ));
    }
    if !(1..=100).contains(&opts.quality) {
        return Err(format!(
            "quality must be between 1 and 100, got {}",
            opts.quality
        ));
    }

    let src = decode_logo(image_bytes)?;
    let source_dims = (src.width(), src.height());
    let ext = opts.format.ext();
    let themes: Vec<bool> = if opts.dark_background.is_some() {
        vec![false, true]
    } else {
        vec![false]
    };

    let mut renderer = Renderer::new(&src, opts);
    let mut cursor = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut cursor);
    let fopts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut files = 0usize;
    let mut images = 0usize;
    let mut platforms: Vec<String> = Vec::new();

    macro_rules! put {
        ($path:expr, $bytes:expr) => {{
            let path: String = $path;
            zip.start_file(&path, fopts)
                .map_err(|e| format!("zip start {path}: {e}"))?;
            zip.write_all($bytes)
                .map_err(|e| format!("zip write {path}: {e}"))?;
            files += 1;
        }};
    }

    // ---- iOS / iPadOS launch images + the meta-tag snippet ----
    if opts.ios {
        platforms.push("ios".to_string());
        for &dark in &themes {
            let dir = if dark { "ios/dark" } else { "ios" };
            // Distinct resolutions only — several devices share one.
            let mut seen: Vec<(u32, u32)> = Vec::new();
            for d in IOS_DEVICES {
                let mut sizes: Vec<(u32, u32)> = Vec::new();
                if opts.orientation.portrait() {
                    sizes.push(d.portrait());
                }
                if opts.orientation.landscape() {
                    sizes.push(d.landscape());
                }
                for (w, h) in sizes {
                    if seen.contains(&(w, h)) {
                        continue;
                    }
                    seen.push((w, h));
                    let bytes = renderer.screen(w, h, dark)?.to_vec();
                    put!(
                        format!("{dir}/apple-touch-startup-image-{w}x{h}.{ext}"),
                        &bytes
                    );
                    images += 1;
                }
            }
        }
        put!(
            "ios/apple-touch-startup-image.html".to_string(),
            ios_html(opts, ext).as_bytes()
        );
    }

    // ---- Android density buckets + the Android 12+ splash icon ----
    if opts.android {
        platforms.push("android".to_string());
        for &dark in &themes {
            let night = if dark { "-night" } else { "" };
            for d in ANDROID_DENSITIES {
                if opts.orientation.portrait() {
                    let bytes = renderer.screen(d.w, d.h, dark)?.to_vec();
                    put!(
                        format!(
                            "android/res/drawable-port{night}-{}/splash.{ext}",
                            d.qualifier
                        ),
                        &bytes
                    );
                    images += 1;
                }
                if opts.orientation.landscape() {
                    let bytes = renderer.screen(d.h, d.w, dark)?.to_vec();
                    put!(
                        format!(
                            "android/res/drawable-land{night}-{}/splash.{ext}",
                            d.qualifier
                        ),
                        &bytes
                    );
                    images += 1;
                }
            }
            let icon = renderer
                .canvas(
                    ANDROID_ICON_CANVAS,
                    ANDROID_ICON_CANVAS,
                    dark,
                    ANDROID_ICON_CONTENT,
                )?
                .to_vec();
            put!(
                format!("android/res/drawable{night}/splash_icon.{ext}"),
                &icon
            );
            images += 1;
        }
    }

    put!(
        "README.txt".to_string(),
        readme(opts, images, ext).as_bytes()
    );

    zip.finish().map_err(|e| format!("finalize zip: {e}"))?;

    Ok((
        cursor.into_inner(),
        Summary {
            images,
            files,
            platforms,
            source_dims,
            dark: opts.dark_background.is_some(),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use zip::ZipArchive;

    /// A tiny opaque red logo.
    fn logo_png(w: u32, h: u32) -> Vec<u8> {
        let img =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, image::Rgba([220, 20, 40, 255])));
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    fn names(zip: &[u8]) -> Vec<String> {
        let mut a = ZipArchive::new(Cursor::new(zip.to_vec())).unwrap();
        (0..a.len())
            .map(|i| a.by_index(i).unwrap().name().to_string())
            .collect()
    }

    fn entry(zip: &[u8], name: &str) -> Vec<u8> {
        let mut a = ZipArchive::new(Cursor::new(zip.to_vec())).unwrap();
        let mut f = a.by_name(name).unwrap();
        let mut out = Vec::new();
        f.read_to_end(&mut out).unwrap();
        out
    }

    #[test]
    fn defaults_generate_portrait_ios_and_android_screens() {
        let opts = Options {
            background: [16, 20, 24],
            ..Options::default()
        };
        let (zip, summary) = generate_zip(&logo_png(256, 256), &opts).unwrap();

        // 18 distinct portrait iOS resolutions + 5 Android buckets + 1 splash icon.
        assert_eq!(summary.images, 18 + 5 + 1);
        assert_eq!(
            summary.platforms,
            vec!["ios".to_string(), "android".to_string()]
        );
        assert_eq!(summary.source_dims, (256, 256));
        assert!(!summary.dark);

        let n = names(&zip);
        assert!(
            n.contains(&"ios/apple-touch-startup-image-1179x2556.png".to_string()),
            "{n:?}"
        );
        assert!(n.contains(&"ios/apple-touch-startup-image.html".to_string()));
        assert!(n.contains(&"android/res/drawable-port-xxxhdpi/splash.png".to_string()));
        assert!(n.contains(&"android/res/drawable/splash_icon.png".to_string()));
        assert!(n.contains(&"README.txt".to_string()));
        // Portrait-only default: no landscape resource folder, no landscape image.
        assert!(!n.iter().any(|p| p.contains("drawable-land")), "{n:?}");
        assert!(!n.contains(&"ios/apple-touch-startup-image-2556x1179.png".to_string()));

        // The image is really the advertised size, with the background painted
        // in the corner and the logo composited in the middle.
        let png = entry(&zip, "ios/apple-touch-startup-image-1179x2556.png");
        let img = image::load_from_memory(&png).unwrap().to_rgb8();
        assert_eq!((img.width(), img.height()), (1179, 2556));
        assert_eq!(img.get_pixel(0, 0).0, [16, 20, 24]);
        assert_eq!(img.get_pixel(1179 / 2, 2556 / 2).0, [220, 20, 40]);
        // logo_scale 0.4 of the 1179px short side => a 471px logo, so a point
        // 300px off-centre horizontally is still background.
        assert_eq!(img.get_pixel(1179 / 2 + 300, 2556 / 2).0, [16, 20, 24]);

        let html = String::from_utf8(entry(&zip, "ios/apple-touch-startup-image.html")).unwrap();
        assert!(html.contains(
            "(device-width: 393px) and (device-height: 852px) and (-webkit-device-pixel-ratio: 3) and (orientation: portrait)"
        ));
        assert!(html.contains("href=\"apple-touch-startup-image-1179x2556.png\""));
        assert!(!html.contains("prefers-color-scheme"));
    }

    #[test]
    fn both_orientations_dark_variant_and_jpeg() {
        let opts = Options {
            background: [255, 255, 255],
            dark_background: Some([0, 0, 0]),
            orientation: Orientation::Both,
            format: Format::Jpeg,
            quality: 70,
            ios: true,
            android: false,
            ..Options::default()
        };
        let (zip, summary) = generate_zip(&logo_png(128, 64), &opts).unwrap();
        assert!(summary.dark);
        // 18 portrait + 18 landscape resolutions, light and dark.
        assert_eq!(summary.images, 36 * 2);

        let n = names(&zip);
        assert!(
            n.contains(&"ios/apple-touch-startup-image-2556x1179.jpg".to_string()),
            "{n:?}"
        );
        assert!(n.contains(&"ios/dark/apple-touch-startup-image-1179x2556.jpg".to_string()));
        assert!(!n.iter().any(|p| p.starts_with("android/")), "{n:?}");

        let jpg = entry(&zip, "ios/dark/apple-touch-startup-image-1179x2556.jpg");
        assert_eq!(&jpg[0..2], &[0xff, 0xd8], "JPEG SOI marker");
        let img = image::load_from_memory(&jpg).unwrap().to_rgb8();
        assert_eq!((img.width(), img.height()), (1179, 2556));

        let html = String::from_utf8(entry(&zip, "ios/apple-touch-startup-image.html")).unwrap();
        assert!(html.contains("(orientation: landscape) and (prefers-color-scheme: light)"));
        assert!(html.contains("href=\"dark/apple-touch-startup-image-1179x2556.jpg\""));
    }

    #[test]
    fn logo_scale_controls_how_much_of_the_canvas_the_logo_covers() {
        let small = Options {
            logo_scale: 0.1,
            android: false,
            ..Options::default()
        };
        let (zip, _) = generate_zip(&logo_png(256, 256), &small).unwrap();
        let img =
            image::load_from_memory(&entry(&zip, "ios/apple-touch-startup-image-640x1136.png"))
                .unwrap()
                .to_rgb8();
        // 0.1 * 640 = 64px logo => +100px from centre is background again.
        assert_eq!(img.get_pixel(320, 568).0, [220, 20, 40]);
        assert_eq!(img.get_pixel(420, 568).0, [255, 255, 255]);
    }

    #[test]
    fn rejects_undecodable_input() {
        let err = generate_zip(b"this is not an image at all", &Options::default()).unwrap_err();
        assert!(err.contains("unrecognised image format"), "{err}");
    }

    #[test]
    fn rejects_out_of_range_and_empty_selections() {
        let bad_scale = Options {
            logo_scale: 1.5,
            ..Options::default()
        };
        assert!(generate_zip(&logo_png(8, 8), &bad_scale)
            .unwrap_err()
            .contains("logo_scale must be between 0.05 and 0.9"));

        let no_platform = Options {
            ios: false,
            android: false,
            ..Options::default()
        };
        assert!(generate_zip(&logo_png(8, 8), &no_platform)
            .unwrap_err()
            .contains("no platforms selected"));

        let bad_quality = Options {
            quality: 0,
            ..Options::default()
        };
        assert!(generate_zip(&logo_png(8, 8), &bad_quality)
            .unwrap_err()
            .contains("quality must be between 1 and 100"));
    }

    #[test]
    fn parses_every_advertised_colour_form() {
        assert_eq!(parse_color("#101418").unwrap(), [16, 20, 24]);
        assert_eq!(parse_color("101418").unwrap(), [16, 20, 24]);
        assert_eq!(parse_color("#f00").unwrap(), [255, 0, 0]);
        assert_eq!(parse_color("rgb(16, 20, 24)").unwrap(), [16, 20, 24]);
        assert_eq!(parse_color("BLACK").unwrap(), [0, 0, 0]);
        // 50% alpha over white.
        assert_eq!(parse_color("#00000080").unwrap(), [127, 127, 127]);
        let err = parse_color("#12345").unwrap_err();
        assert!(err.contains("expected a colour like"), "{err}");
    }

    #[test]
    fn parses_orientation_and_format_enums() {
        assert_eq!(Orientation::parse("both").unwrap(), Orientation::Both);
        assert_eq!(Format::parse("JPG").unwrap(), Format::Jpeg);
        assert!(Orientation::parse("sideways")
            .unwrap_err()
            .contains("portrait, landscape or both"));
        assert!(Format::parse("tiff").unwrap_err().contains("png or jpeg"));
    }
}
