//! gizza-ai/screenshot-beautify core — wrap a raw screenshot in a share-ready
//! frame: padding, rounded corners, an optional soft drop shadow, an optional
//! macOS-style window title bar with traffic-light dots, and a solid or linear
//! gradient backdrop. Pads out to an aspect ratio without ever cropping the shot.
//! Pure-Rust `image` crate — no wafer/wasm-bindgen deps. Output is always PNG
//! (the shadow + rounded corners need an alpha channel JPEG can't carry).

use std::io::Cursor;

use image::{imageops, DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};

/// Parse `#rgb`, `#rrggbb`, or `#rrggbbaa` into RGBA.
pub fn parse_color(s: &str) -> Result<Rgba<u8>, String> {
    let h = s.trim().trim_start_matches('#');
    let v = |a: &str| u8::from_str_radix(a, 16).map_err(|_| format!("invalid color '{s}'"));
    let (r, g, b, a) = match h.len() {
        3 => {
            let cs: Vec<char> = h.chars().collect();
            let d = |c: char| v(&c.to_string().repeat(2));
            (d(cs[0])?, d(cs[1])?, d(cs[2])?, 255)
        }
        6 => (v(&h[0..2])?, v(&h[2..4])?, v(&h[4..6])?, 255),
        8 => (v(&h[0..2])?, v(&h[2..4])?, v(&h[4..6])?, v(&h[6..8])?),
        _ => return Err(format!("invalid color '{s}' (use #rgb, #rrggbb, or #rrggbbaa)")),
    };
    Ok(Rgba([r, g, b, a]))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Background {
    Solid,
    Gradient,
}

pub fn parse_background(s: &str) -> Result<Background, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "gradient" => Ok(Background::Gradient),
        "solid" | "color" => Ok(Background::Solid),
        other => Err(format!("background {other:?} not supported (solid|gradient)")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aspect {
    Auto,
    R16x9,
    R4x3,
    R1x1,
    R9x16,
}

pub fn parse_aspect(s: &str) -> Result<Aspect, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(Aspect::Auto),
        "16:9" | "16x9" => Ok(Aspect::R16x9),
        "4:3" | "4x3" => Ok(Aspect::R4x3),
        "1:1" | "1x1" | "square" => Ok(Aspect::R1x1),
        "9:16" | "9x16" | "story" => Ok(Aspect::R9x16),
        other => Err(format!("aspect {other:?} not supported (auto|16:9|4:3|1:1|9:16)")),
    }
}

impl Aspect {
    /// Target width:height ratio, or None for `auto` (keep the padded size).
    fn ratio(self) -> Option<f32> {
        match self {
            Aspect::Auto => None,
            Aspect::R16x9 => Some(16.0 / 9.0),
            Aspect::R4x3 => Some(4.0 / 3.0),
            Aspect::R1x1 => Some(1.0),
            Aspect::R9x16 => Some(9.0 / 16.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Titlebar {
    None,
    Light,
    Dark,
}

pub fn parse_titlebar(s: &str) -> Result<Titlebar, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "none" | "off" => Ok(Titlebar::None),
        "light" | "macos-light" | "mac-light" => Ok(Titlebar::Light),
        "dark" | "macos-dark" | "mac-dark" => Ok(Titlebar::Dark),
        other => Err(format!("titlebar {other:?} not supported (none|light|dark)")),
    }
}

/// All beautify options. Numeric fields are pixel sizes / 0..=1 opacity /
/// degrees, already validated + clamped by the caller.
#[derive(Debug, Clone)]
pub struct Options {
    pub padding: u32,
    pub corner_radius: u32,
    pub shadow: bool,
    pub shadow_blur: u32,
    pub shadow_opacity: f32,
    pub background: Background,
    pub bg_color: Rgba<u8>,
    pub bg_color2: Rgba<u8>,
    pub gradient_angle: f32,
    pub aspect: Aspect,
    pub titlebar: Titlebar,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            padding: 64,
            corner_radius: 16,
            shadow: true,
            shadow_blur: 24,
            shadow_opacity: 0.35,
            background: Background::Gradient,
            bg_color: Rgba([79, 70, 229, 255]),   // indigo
            bg_color2: Rgba([147, 51, 234, 255]), // violet
            gradient_angle: 135.0,
            aspect: Aspect::Auto,
            titlebar: Titlebar::None,
        }
    }
}

const MAX_DIM: u32 = 8192;

/// Anti-aliased coverage (0..1) of a rounded rectangle of size `w`×`h` with
/// corner radius `r`, sampled at pixel center `(x, y)`. 1 inside, 0 outside.
fn rr_coverage(x: u32, y: u32, w: u32, h: u32, r: f32) -> f32 {
    let hx = w as f32 / 2.0;
    let hy = h as f32 / 2.0;
    let r = r.min(hx).min(hy).max(0.0);
    let px = x as f32 + 0.5 - hx;
    let py = y as f32 + 0.5 - hy;
    let qx = px.abs() - (hx - r);
    let qy = py.abs() - (hy - r);
    let ax = qx.max(0.0);
    let ay = qy.max(0.0);
    let d = (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - r;
    (0.5 - d).clamp(0.0, 1.0)
}

/// Linear-interpolate two colors at `t` in 0..1.
fn lerp_rgba(a: Rgba<u8>, b: Rgba<u8>, t: f32) -> Rgba<u8> {
    let t = t.clamp(0.0, 1.0);
    let m = |i: usize| (a.0[i] as f32 + (b.0[i] as f32 - a.0[i] as f32) * t).round() as u8;
    Rgba([m(0), m(1), m(2), m(3)])
}

/// Alpha-over composite `src` onto `dst` in place.
fn over(dst: &mut Rgba<u8>, src: Rgba<u8>) {
    let sa = src.0[3] as f32 / 255.0;
    if sa <= 0.0 {
        return;
    }
    let da = dst.0[3] as f32 / 255.0;
    let oa = sa + da * (1.0 - sa);
    if oa <= 0.0 {
        *dst = Rgba([0, 0, 0, 0]);
        return;
    }
    let mut out = [0u8; 4];
    for i in 0..3 {
        let c = (src.0[i] as f32 * sa + dst.0[i] as f32 * da * (1.0 - sa)) / oa;
        out[i] = c.round().clamp(0.0, 255.0) as u8;
    }
    out[3] = (oa * 255.0).round().clamp(0.0, 255.0) as u8;
    *dst = Rgba(out);
}

/// Draw an anti-aliased filled disc of `color` centered at `(cx, cy)`.
fn draw_disc(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, color: Rgba<u8>) {
    let (w, h) = img.dimensions();
    let x0 = (cx - r - 1.0).floor().max(0.0) as u32;
    let x1 = ((cx + r + 1.0).ceil() as u32).min(w);
    let y0 = (cy - r - 1.0).floor().max(0.0) as u32;
    let y1 = ((cy + r + 1.0).ceil() as u32).min(h);
    for y in y0..y1 {
        for x in x0..x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt() - r;
            let cov = (0.5 - d).clamp(0.0, 1.0);
            if cov > 0.0 {
                let mut src = color;
                src.0[3] = (color.0[3] as f32 * cov).round() as u8;
                let mut px = *img.get_pixel(x, y);
                over(&mut px, src);
                img.put_pixel(x, y, px);
            }
        }
    }
}

/// Build the "card": the shot with an optional macOS title bar stacked on top,
/// then rounded corners applied to the whole card. Returns an RGBA image whose
/// alpha is the rounded-card silhouette.
fn build_card(shot: &RgbaImage, opts: &Options) -> RgbaImage {
    let (sw, sh) = shot.dimensions();
    let bar_h = match opts.titlebar {
        Titlebar::None => 0,
        _ => ((sw as f32 * 0.055).round() as u32).clamp(24, 96),
    };
    let cw = sw;
    let ch = sh + bar_h;
    let mut card = RgbaImage::from_pixel(cw, ch, Rgba([0, 0, 0, 0]));

    // Title bar background + traffic-light dots.
    if bar_h > 0 {
        let (bar_bg, border) = match opts.titlebar {
            Titlebar::Light => (Rgba([236, 236, 236, 255]), Rgba([210, 210, 210, 255])),
            Titlebar::Dark => (Rgba([54, 54, 56, 255]), Rgba([40, 40, 42, 255])),
            Titlebar::None => unreachable!(),
        };
        for y in 0..bar_h {
            for x in 0..cw {
                // 1px separator line at the bottom of the bar.
                let c = if y + 1 == bar_h { border } else { bar_bg };
                card.put_pixel(x, y, c);
            }
        }
        let dots = [
            Rgba([255, 95, 86, 255]),  // red
            Rgba([255, 189, 46, 255]), // amber
            Rgba([39, 201, 63, 255]),  // green
        ];
        let radius = (bar_h as f32 * 0.16).round().max(3.0);
        let cy = bar_h as f32 / 2.0;
        let gap = radius * 3.2;
        let start = radius * 3.0;
        for (i, dot) in dots.iter().enumerate() {
            draw_disc(&mut card, start + i as f32 * gap, cy, radius, *dot);
        }
    }

    // The shot itself, below the bar.
    for y in 0..sh {
        for x in 0..sw {
            card.put_pixel(x, y + bar_h, *shot.get_pixel(x, y));
        }
    }

    // Rounded-corner alpha mask over the whole card.
    if opts.corner_radius > 0 {
        let r = opts.corner_radius as f32;
        for y in 0..ch {
            for x in 0..cw {
                let cov = rr_coverage(x, y, cw, ch, r);
                let p = card.get_pixel_mut(x, y);
                p.0[3] = (p.0[3] as f32 * cov).round() as u8;
            }
        }
    }
    card
}

/// Fill `canvas` with the backdrop (solid or linear gradient across
/// `gradient_angle` degrees).
fn fill_background(canvas: &mut RgbaImage, opts: &Options) {
    let (w, h) = canvas.dimensions();
    match opts.background {
        Background::Solid => {
            for p in canvas.pixels_mut() {
                *p = opts.bg_color;
            }
        }
        Background::Gradient => {
            let rad = opts.gradient_angle.to_radians();
            let (dx, dy) = (rad.cos(), rad.sin());
            // Project the 4 corners to find the gradient's extent.
            let corners = [(0.0, 0.0), (w as f32, 0.0), (0.0, h as f32), (w as f32, h as f32)];
            let mut lo = f32::INFINITY;
            let mut hi = f32::NEG_INFINITY;
            for (cx, cy) in corners {
                let p = cx * dx + cy * dy;
                lo = lo.min(p);
                hi = hi.max(p);
            }
            let span = (hi - lo).max(1e-6);
            for y in 0..h {
                for x in 0..w {
                    let proj = (x as f32 + 0.5) * dx + (y as f32 + 0.5) * dy;
                    let t = (proj - lo) / span;
                    canvas.put_pixel(x, y, lerp_rgba(opts.bg_color, opts.bg_color2, t));
                }
            }
        }
    }
}

/// Wrap `bytes` (an encoded screenshot) into a beautified PNG per `opts`.
pub fn beautify(bytes: &[u8], opts: &Options) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }
    let shot = img.to_rgba8();

    let card = build_card(&shot, opts);
    let (cw, ch) = card.dimensions();

    // Base canvas = card + uniform padding on every side.
    let pad = opts.padding;
    let mut canvas_w = cw + 2 * pad;
    let mut canvas_h = ch + 2 * pad;

    // Pad out (never crop) to the requested aspect ratio.
    if let Some(ratio) = opts.aspect.ratio() {
        let cur = canvas_w as f32 / canvas_h as f32;
        if cur < ratio {
            canvas_w = (canvas_h as f32 * ratio).round() as u32;
        } else if cur > ratio {
            canvas_h = (canvas_w as f32 / ratio).round() as u32;
        }
    }

    if canvas_w > MAX_DIM || canvas_h > MAX_DIM {
        return Err(format!(
            "output {canvas_w}×{canvas_h} exceeds the {MAX_DIM}px limit; reduce padding or the input size"
        ));
    }

    // Center the card on the canvas.
    let off_x = (canvas_w - cw) / 2;
    let off_y = (canvas_h - ch) / 2;

    let mut canvas = RgbaImage::from_pixel(canvas_w, canvas_h, Rgba([0, 0, 0, 0]));
    fill_background(&mut canvas, opts);

    // Soft drop shadow: a blurred, offset black silhouette of the card.
    if opts.shadow && opts.shadow_opacity > 0.0 {
        let shadow_off_y = (opts.shadow_blur as f32 * 0.55).round().max(4.0) as i64;
        let mut layer = RgbaImage::from_pixel(canvas_w, canvas_h, Rgba([0, 0, 0, 0]));
        let sy = off_y as i64 + shadow_off_y;
        for y in 0..ch {
            let ty = sy + y as i64;
            if ty < 0 || ty >= canvas_h as i64 {
                continue;
            }
            for x in 0..cw {
                let a = card.get_pixel(x, y).0[3];
                if a > 0 {
                    layer.put_pixel(off_x + x, ty as u32, Rgba([0, 0, 0, a]));
                }
            }
        }
        let blurred = imageops::blur(&layer, opts.shadow_blur.max(1) as f32);
        for y in 0..canvas_h {
            for x in 0..canvas_w {
                let mut s = *blurred.get_pixel(x, y);
                if s.0[3] == 0 {
                    continue;
                }
                s.0[3] = (s.0[3] as f32 * opts.shadow_opacity).round() as u8;
                let mut base = *canvas.get_pixel(x, y);
                over(&mut base, s);
                canvas.put_pixel(x, y, base);
            }
        }
    }

    // Composite the card centered on the canvas.
    for y in 0..ch {
        for x in 0..cw {
            let src = *card.get_pixel(x, y);
            if src.0[3] == 0 {
                continue;
            }
            let mut base = *canvas.get_pixel(off_x + x, off_y + y);
            over(&mut base, src);
            canvas.put_pixel(off_x + x, off_y + y, base);
        }
    }

    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(canvas)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, c: Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, c);
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn opts() -> Options {
        // A deterministic solid-background, no-shadow, no-titlebar baseline.
        Options {
            shadow: false,
            background: Background::Solid,
            bg_color: Rgba([255, 255, 255, 255]),
            aspect: Aspect::Auto,
            titlebar: Titlebar::None,
            corner_radius: 0,
            padding: 40,
            ..Options::default()
        }
    }

    #[test]
    fn parse_helpers() {
        assert_eq!(parse_color("#fff").unwrap(), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_color("#ff8800").unwrap(), Rgba([255, 136, 0, 255]));
        assert!(parse_color("zzz").is_err());
        assert_eq!(parse_background("").unwrap(), Background::Gradient);
        assert_eq!(parse_background("solid").unwrap(), Background::Solid);
        assert!(parse_background("plaid").is_err());
        assert_eq!(parse_aspect("16:9").unwrap(), Aspect::R16x9);
        assert_eq!(parse_aspect("auto").unwrap(), Aspect::Auto);
        assert!(parse_aspect("3:2").is_err());
        assert_eq!(parse_titlebar("dark").unwrap(), Titlebar::Dark);
        assert_eq!(parse_titlebar("none").unwrap(), Titlebar::None);
        assert!(parse_titlebar("floating").is_err());
    }

    #[test]
    fn padding_grows_by_twice() {
        let src = png(60, 40, Rgba([0, 120, 255, 255]));
        let out = beautify(&src, &opts()).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (60 + 80, 40 + 80)); // 40px padding each side
    }

    #[test]
    fn background_fills_corners_center_is_shot() {
        let src = png(60, 40, Rgba([0, 120, 255, 255]));
        let out = beautify(&src, &opts()).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        // Corner is the solid backdrop, fully opaque.
        assert_eq!(img.get_pixel(0, 0), Rgba([255, 255, 255, 255]));
        // Center is the original shot color.
        assert_eq!(img.get_pixel(70, 60), Rgba([0, 120, 255, 255]));
    }

    #[test]
    fn rounded_corner_reveals_background() {
        let mut o = opts();
        o.corner_radius = 16;
        let src = png(80, 80, Rgba([0, 120, 255, 255]));
        let out = beautify(&src, &o).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        // The card's own top-left corner (at padding, padding) is rounded away,
        // so the backdrop shows through there.
        assert_eq!(img.get_pixel(40, 40), Rgba([255, 255, 255, 255]));
        // ...while the shot's center is intact.
        assert_eq!(img.get_pixel(80, 80), Rgba([0, 120, 255, 255]));
    }

    #[test]
    fn aspect_1x1_makes_square() {
        let src = png(120, 40, Rgba([0, 0, 0, 255]));
        let mut o = opts();
        o.aspect = Aspect::R1x1;
        let out = beautify(&src, &o).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        let (w, h) = img.dimensions();
        assert_eq!(w, h, "1:1 aspect must be square");
        // Never crops: square side >= the padded content's larger dimension.
        assert!(w >= 120 + 80);
    }

    #[test]
    fn titlebar_adds_height() {
        let src = png(200, 100, Rgba([0, 0, 0, 255]));
        let base = {
            let out = beautify(&src, &opts()).unwrap();
            image::load_from_memory(&out).unwrap().dimensions()
        };
        let mut o = opts();
        o.titlebar = Titlebar::Dark;
        let out = beautify(&src, &o).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert!(img.dimensions().1 > base.1, "title bar must add height");
    }

    #[test]
    fn shadow_darkens_below_card() {
        let src = png(80, 60, Rgba([0, 120, 255, 255]));
        let mut o = opts();
        o.shadow = true;
        o.shadow_blur = 12;
        o.shadow_opacity = 0.6;
        let out = beautify(&src, &o).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        // Just below the card's bottom edge, inside the padding band, the white
        // backdrop is darkened by the shadow.
        let px = img.get_pixel(40 + 40, 40 + 60 + 6);
        assert!(px.0[0] < 255, "shadow should darken the backdrop below the card");
    }

    #[test]
    fn gradient_endpoints_differ() {
        let src = png(40, 40, Rgba([0, 0, 0, 255]));
        let mut o = opts();
        o.background = Background::Gradient;
        o.bg_color = Rgba([255, 0, 0, 255]);
        o.bg_color2 = Rgba([0, 0, 255, 255]);
        o.gradient_angle = 0.0; // left -> right
        let out = beautify(&src, &o).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        let (w, h) = img.dimensions();
        let left = img.get_pixel(0, h / 2);
        let right = img.get_pixel(w - 1, h / 2);
        assert!(left.0[0] > right.0[0], "red should dominate the left edge");
        assert!(right.0[2] > left.0[2], "blue should dominate the right edge");
    }

    #[test]
    fn errors_on_bad_image() {
        assert!(beautify(b"not an image", &opts()).is_err());
    }
}
