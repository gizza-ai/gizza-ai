//! gizza-ai/device-mockup-framer core — frame a screenshot inside a clean device
//! mockup (phone, tablet, laptop, or browser window) drawn with vector-style
//! bezels, then drop it on a solid / gradient / transparent backdrop with an
//! optional soft shadow and padding. Pure-Rust `image` crate for composition +
//! pure-Rust `fontdue` for the browser address-bar URL text — no wafer /
//! wasm-bindgen deps, runs on every backend including the chat Service Worker.
//! Output is always PNG (the bezels, rounded corners, shadow, and transparent
//! backdrop all need an alpha channel JPEG can't carry).

use std::io::Cursor;

use fontdue::Font;
use image::{imageops, DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};

/// Embedded proportional-mono font for the browser URL bar (DejaVu Sans Mono,
/// Bitstream-Vera/Arev license — freely redistributable). Bundled so text
/// rendering never touches the host filesystem (wasm has none). See
/// `core/src/assets/LICENSE-DejaVu.txt`.
const FONT_BYTES: &[u8] = include_bytes!("assets/DejaVuSansMono.ttf");

const MAX_DIM: u32 = 8192;

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

/// Which device shell to draw around the shot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    Phone,
    Tablet,
    Laptop,
    Browser,
}

pub fn parse_device(s: &str) -> Result<Device, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "phone" | "mobile" | "smartphone" => Ok(Device::Phone),
        "tablet" | "ipad" => Ok(Device::Tablet),
        "laptop" | "macbook" | "notebook" => Ok(Device::Laptop),
        "browser" | "window" | "browser-window" => Ok(Device::Browser),
        other => Err(format!("device {other:?} not supported (phone|tablet|laptop|browser)")),
    }
}

/// Body / bezel color of the device shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameColor {
    Black,
    White,
    Silver,
}

pub fn parse_frame_color(s: &str) -> Result<FrameColor, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "black" | "graphite" | "space-gray" | "space-grey" => Ok(FrameColor::Black),
        "white" => Ok(FrameColor::White),
        "silver" | "gray" | "grey" => Ok(FrameColor::Silver),
        other => Err(format!("frame_color {other:?} not supported (black|white|silver)")),
    }
}

impl FrameColor {
    /// The solid bezel/body fill color.
    fn body(self) -> Rgba<u8> {
        match self {
            FrameColor::Black => Rgba([26, 26, 28, 255]),
            FrameColor::White => Rgba([244, 244, 247, 255]),
            FrameColor::Silver => Rgba([196, 198, 203, 255]),
        }
    }
    /// True when this shell should use a DARK browser chrome (only black does).
    fn dark_chrome(self) -> bool {
        matches!(self, FrameColor::Black)
    }
}

/// Backdrop style behind the framed device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Background {
    Gradient,
    Solid,
    Transparent,
}

pub fn parse_background(s: &str) -> Result<Background, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "gradient" => Ok(Background::Gradient),
        "solid" | "color" => Ok(Background::Solid),
        "transparent" | "none" => Ok(Background::Transparent),
        other => Err(format!("background {other:?} not supported (gradient|solid|transparent)")),
    }
}

/// All framing options. Numeric fields are pixel sizes / 0..=1 opacity / degrees,
/// already validated + clamped by the caller.
#[derive(Debug, Clone)]
pub struct Options {
    pub device: Device,
    pub frame_color: FrameColor,
    pub background: Background,
    pub bg_color: Rgba<u8>,
    pub bg_color2: Rgba<u8>,
    pub gradient_angle: f32,
    pub padding: u32,
    pub shadow: bool,
    pub shadow_blur: u32,
    pub shadow_opacity: f32,
    pub browser_url: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            device: Device::Phone,
            frame_color: FrameColor::Black,
            background: Background::Gradient,
            bg_color: Rgba([99, 102, 241, 255]),  // indigo
            bg_color2: Rgba([168, 85, 247, 255]),  // violet
            gradient_angle: 135.0,
            padding: 64,
            shadow: true,
            shadow_blur: 40,
            shadow_opacity: 0.35,
            browser_url: "example.com".into(),
        }
    }
}

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

/// Fill a rounded rectangle of `color` at device-layer offset `(x0, y0)`,
/// size `w`×`h`, corner radius `r`, over-compositing with AA edges.
fn draw_rr(img: &mut RgbaImage, x0: i64, y0: i64, w: u32, h: u32, r: f32, color: Rgba<u8>) {
    let (iw, ih) = img.dimensions();
    for ly in 0..h {
        for lx in 0..w {
            let cov = rr_coverage(lx, ly, w, h, r);
            if cov <= 0.0 {
                continue;
            }
            let gx = x0 + lx as i64;
            let gy = y0 + ly as i64;
            if gx < 0 || gy < 0 || gx >= iw as i64 || gy >= ih as i64 {
                continue;
            }
            let mut src = color;
            src.0[3] = (color.0[3] as f32 * cov).round() as u8;
            let mut px = *img.get_pixel(gx as u32, gy as u32);
            over(&mut px, src);
            img.put_pixel(gx as u32, gy as u32, px);
        }
    }
}

/// Composite `shot` into the screen region at `(sx, sy)`, masked by a rounded
/// rectangle of radius `r` (so the screen corners follow the bezel).
fn composite_screen(dev: &mut RgbaImage, shot: &RgbaImage, sx: i64, sy: i64, r: f32) {
    let (sw, sh) = shot.dimensions();
    let (dw, dh) = dev.dimensions();
    for y in 0..sh {
        for x in 0..sw {
            let cov = rr_coverage(x, y, sw, sh, r);
            if cov <= 0.0 {
                continue;
            }
            let gx = sx + x as i64;
            let gy = sy + y as i64;
            if gx < 0 || gy < 0 || gx >= dw as i64 || gy >= dh as i64 {
                continue;
            }
            let mut src = *shot.get_pixel(x, y);
            src.0[3] = (src.0[3] as f32 * cov).round() as u8;
            let mut px = *dev.get_pixel(gx as u32, gy as u32);
            over(&mut px, src);
            dev.put_pixel(gx as u32, gy as u32, px);
        }
    }
}

/// Left-aligned single-line text, vertically centered in a band whose top is
/// `y0` and height `band_h`. Draws at pixel height `px`, clipped to `max_w`
/// (truncated with an ellipsis when it would overflow). Returns nothing — a
/// best-effort label, never an error path.
fn draw_text(
    img: &mut RgbaImage,
    font: &Font,
    text: &str,
    x0: f32,
    y0: f32,
    band_h: f32,
    px: f32,
    max_w: f32,
    color: Rgba<u8>,
) {
    // Baseline: center the em box in the band. fontdue lays glyphs from the
    // baseline; approximate the cap-to-baseline offset as ~0.72*px.
    let baseline = y0 + band_h / 2.0 + px * 0.34;
    let mut pen = x0;
    let ellipsis_w = font.metrics('…', px).advance_width;
    let mut chars: Vec<char> = text.chars().collect();
    // Pre-truncate: keep chars while the running advance (plus a trailing
    // ellipsis) fits inside max_w.
    let mut used = 0.0;
    let mut cut = chars.len();
    for (i, &c) in chars.iter().enumerate() {
        let adv = font.metrics(c, px).advance_width;
        if used + adv + ellipsis_w > max_w {
            cut = i;
            break;
        }
        used += adv;
    }
    let truncated = cut < chars.len();
    chars.truncate(cut);
    if truncated {
        chars.push('…');
    }
    for c in chars {
        let (m, bitmap) = font.rasterize(c, px);
        if m.width > 0 && m.height > 0 {
            let gx0 = pen + m.xmin as f32;
            let gy0 = baseline - m.height as f32 - m.ymin as f32;
            for gy in 0..m.height {
                for gx in 0..m.width {
                    let cov = bitmap[gy * m.width + gx] as f32 / 255.0;
                    if cov <= 0.0 {
                        continue;
                    }
                    let tx = (gx0 + gx as f32).round();
                    let ty = (gy0 + gy as f32).round();
                    if tx < 0.0 || ty < 0.0 || tx >= img.width() as f32 || ty >= img.height() as f32
                    {
                        continue;
                    }
                    let mut src = color;
                    src.0[3] = (color.0[3] as f32 * cov).round() as u8;
                    let mut dst = *img.get_pixel(tx as u32, ty as u32);
                    over(&mut dst, src);
                    img.put_pixel(tx as u32, ty as u32, dst);
                }
            }
        }
        pen += m.advance_width;
    }
}

/// A phone: uniform rounded bezel + a centered top notch pill.
fn build_phone(shot: &RgbaImage, opts: &Options) -> RgbaImage {
    let (sw, sh) = shot.dimensions();
    let b = ((sw.min(sh) as f32) * 0.028).round().max(12.0) as u32;
    let dw = sw + 2 * b;
    let dh = sh + 2 * b;
    let outer_r = (dw.min(dh) as f32) * 0.12;
    let inner_r = (sw.min(sh) as f32) * 0.10;
    let mut dev = RgbaImage::from_pixel(dw, dh, Rgba([0, 0, 0, 0]));
    draw_rr(&mut dev, 0, 0, dw, dh, outer_r, opts.frame_color.body());
    composite_screen(&mut dev, shot, b as i64, b as i64, inner_r);
    // Notch pill sitting on the top bezel (earpiece / camera).
    let notch_w = ((dw as f32) * 0.30).round() as u32;
    let notch_h = ((b as f32) * 0.55).round().max(6.0) as u32;
    let notch_x = ((dw - notch_w) / 2) as i64;
    let notch_y = ((b as f32) * 0.30).round() as i64;
    draw_rr(&mut dev, notch_x, notch_y, notch_w, notch_h, notch_h as f32 / 2.0, Rgba([12, 12, 14, 255]));
    dev
}

/// A tablet: thicker uniform bezel + a small top camera dot.
fn build_tablet(shot: &RgbaImage, opts: &Options) -> RgbaImage {
    let (sw, sh) = shot.dimensions();
    let b = ((sw.min(sh) as f32) * 0.045).round().max(16.0) as u32;
    let dw = sw + 2 * b;
    let dh = sh + 2 * b;
    let outer_r = (dw.min(dh) as f32) * 0.045;
    let inner_r = (sw.min(sh) as f32) * 0.02;
    let mut dev = RgbaImage::from_pixel(dw, dh, Rgba([0, 0, 0, 0]));
    draw_rr(&mut dev, 0, 0, dw, dh, outer_r, opts.frame_color.body());
    composite_screen(&mut dev, shot, b as i64, b as i64, inner_r);
    // Camera dot centered on the top bezel.
    draw_disc(&mut dev, dw as f32 / 2.0, (b as f32) * 0.5, ((b as f32) * 0.16).max(3.0), Rgba([40, 40, 44, 255]));
    dev
}

/// A laptop: a thin-bezel screen lid above a wider rounded base deck.
fn build_laptop(shot: &RgbaImage, opts: &Options) -> RgbaImage {
    let (sw, sh) = shot.dimensions();
    let b = ((sw.min(sh) as f32) * 0.02).round().max(8.0) as u32;
    let dw = sw + 2 * b;
    let lidh = sh + 2 * b;
    let lid_r = (dw.min(lidh) as f32) * 0.03;
    let extra = ((dw as f32) * 0.14).round() as u32;
    let base_w = dw + extra;
    let base_h = ((b as f32) * 1.9).round().max(16.0) as u32;
    let dev_w = base_w;
    let dev_h = lidh + base_h;
    let lid_x = ((base_w - dw) / 2) as i64;
    let mut dev = RgbaImage::from_pixel(dev_w, dev_h, Rgba([0, 0, 0, 0]));
    // Screen lid.
    draw_rr(&mut dev, lid_x, 0, dw, lidh, lid_r, opts.frame_color.body());
    composite_screen(&mut dev, shot, lid_x + b as i64, b as i64, (sw.min(sh) as f32) * 0.01);
    // Base deck (slightly darker for depth), rounded bottom.
    let base_body = darken(opts.frame_color.body(), 0.08);
    draw_rr(&mut dev, 0, lidh as i64, base_w, base_h, (base_h as f32) * 0.45, base_body);
    // Trackpad / lid-open notch centered on the base's top edge.
    let notch_w = ((dw as f32) * 0.14).round() as u32;
    let notch_h = ((base_h as f32) * 0.42).round().max(4.0) as u32;
    let notch_x = ((base_w - notch_w) / 2) as i64;
    draw_rr(&mut dev, notch_x, lidh as i64, notch_w, notch_h, notch_h as f32 / 2.0, darken(base_body, 0.25));
    dev
}

/// A browser window: a chrome bar with traffic-light dots + an address bar
/// showing `browser_url`, over the shot, all with rounded outer corners.
fn build_browser(shot: &RgbaImage, opts: &Options, font: &Font) -> RgbaImage {
    let (sw, sh) = shot.dimensions();
    let bar_h = ((sw as f32) * 0.06).round().clamp(34.0, 96.0) as u32;
    let dev_w = sw;
    let dev_h = sh + bar_h;
    let win_r = ((sw as f32) * 0.018).round().clamp(6.0, 28.0);
    let dark = opts.frame_color.dark_chrome();
    let (bar_bg, pill_bg, url_col) = if dark {
        (Rgba([48, 48, 52, 255]), Rgba([30, 30, 34, 255]), Rgba([176, 178, 186, 255]))
    } else {
        (Rgba([236, 236, 239, 255]), Rgba([255, 255, 255, 255]), Rgba([92, 94, 100, 255]))
    };
    let mut dev = RgbaImage::from_pixel(dev_w, dev_h, Rgba([0, 0, 0, 0]));
    // Chrome bar background (full-width rect; corners rounded at the end).
    for y in 0..bar_h {
        for x in 0..dev_w {
            dev.put_pixel(x, y, bar_bg);
        }
    }
    // The shot below the bar.
    for y in 0..sh {
        for x in 0..sw {
            dev.put_pixel(x, y + bar_h, *shot.get_pixel(x, y));
        }
    }
    // Traffic-light dots at the left of the bar.
    let dot_r = ((bar_h as f32) * 0.11).round().max(4.0);
    let cy = bar_h as f32 / 2.0;
    let gap = dot_r * 3.2;
    let start = bar_h as f32 * 0.9;
    for (i, dot) in [
        Rgba([255, 95, 86, 255]),
        Rgba([255, 189, 46, 255]),
        Rgba([39, 201, 63, 255]),
    ]
    .iter()
    .enumerate()
    {
        draw_disc(&mut dev, start + i as f32 * gap, cy, dot_r, *dot);
    }
    // Address bar pill + URL text.
    let pill_h = ((bar_h as f32) * 0.52).round().max(10.0) as u32;
    let pill_x = (start + 2.6 * gap).round() as i64;
    let right_margin = bar_h as f32 * 0.6;
    let pill_w = ((dev_w as i64 - pill_x) as f32 - right_margin).max(0.0) as u32;
    let pill_y = ((bar_h - pill_h) / 2) as i64;
    if pill_w > 8 {
        draw_rr(&mut dev, pill_x, pill_y, pill_w, pill_h, pill_h as f32 / 2.0, pill_bg);
        let url = opts.browser_url.trim();
        if !url.is_empty() {
            let text_px = (pill_h as f32) * 0.56;
            let text_x = pill_x as f32 + pill_h as f32 * 0.55;
            let text_max = pill_w as f32 - pill_h as f32 * 0.9;
            draw_text(
                &mut dev, font, url, text_x, pill_y as f32, pill_h as f32, text_px,
                text_max.max(0.0), url_col,
            );
        }
    }
    // Round the window's outer corners (chrome top + shot bottom).
    for y in 0..dev_h {
        for x in 0..dev_w {
            let cov = rr_coverage(x, y, dev_w, dev_h, win_r);
            let p = dev.get_pixel_mut(x, y);
            p.0[3] = (p.0[3] as f32 * cov).round() as u8;
        }
    }
    dev
}

/// Darken an RGB color toward black by `amt` (0..1), keeping alpha.
fn darken(c: Rgba<u8>, amt: f32) -> Rgba<u8> {
    let f = (1.0 - amt).clamp(0.0, 1.0);
    Rgba([
        (c.0[0] as f32 * f).round() as u8,
        (c.0[1] as f32 * f).round() as u8,
        (c.0[2] as f32 * f).round() as u8,
        c.0[3],
    ])
}

/// Fill `canvas` with the backdrop (solid, linear gradient, or transparent).
fn fill_background(canvas: &mut RgbaImage, opts: &Options) {
    let (w, h) = canvas.dimensions();
    match opts.background {
        Background::Transparent => {} // leave the transparent init untouched
        Background::Solid => {
            for p in canvas.pixels_mut() {
                *p = opts.bg_color;
            }
        }
        Background::Gradient => {
            let rad = opts.gradient_angle.to_radians();
            let (dx, dy) = (rad.cos(), rad.sin());
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

/// Frame `bytes` (an encoded screenshot) inside the chosen device mockup and
/// return the composited PNG per `opts`.
pub fn frame(bytes: &[u8], opts: &Options) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }
    let shot = img.to_rgba8();

    let dev = match opts.device {
        Device::Phone => build_phone(&shot, opts),
        Device::Tablet => build_tablet(&shot, opts),
        Device::Laptop => build_laptop(&shot, opts),
        Device::Browser => {
            let font = Font::from_bytes(FONT_BYTES, fontdue::FontSettings::default())
                .map_err(|e| format!("font load failed: {e}"))?;
            build_browser(&shot, opts, &font)
        }
    };
    let (cw, ch) = dev.dimensions();

    let pad = opts.padding;
    let canvas_w = cw + 2 * pad;
    let canvas_h = ch + 2 * pad;
    if canvas_w > MAX_DIM || canvas_h > MAX_DIM {
        return Err(format!(
            "output {canvas_w}×{canvas_h} exceeds the {MAX_DIM}px limit; reduce padding or the input size"
        ));
    }

    let off_x = pad;
    let off_y = pad;

    let mut canvas = RgbaImage::from_pixel(canvas_w, canvas_h, Rgba([0, 0, 0, 0]));
    fill_background(&mut canvas, opts);

    // Soft drop shadow: a blurred, offset black silhouette of the device.
    if opts.shadow && opts.shadow_opacity > 0.0 {
        let shadow_off_y = ((opts.shadow_blur as f32) * 0.5).round().max(4.0) as i64;
        let mut layer = RgbaImage::from_pixel(canvas_w, canvas_h, Rgba([0, 0, 0, 0]));
        let sy = off_y as i64 + shadow_off_y;
        for y in 0..ch {
            let ty = sy + y as i64;
            if ty < 0 || ty >= canvas_h as i64 {
                continue;
            }
            for x in 0..cw {
                let a = dev.get_pixel(x, y).0[3];
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

    // Composite the device centered on the canvas.
    for y in 0..ch {
        for x in 0..cw {
            let src = *dev.get_pixel(x, y);
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
        // Deterministic solid-background, no-shadow baseline.
        Options {
            background: Background::Solid,
            bg_color: Rgba([255, 255, 255, 255]),
            shadow: false,
            padding: 40,
            ..Options::default()
        }
    }

    #[test]
    fn parse_helpers() {
        assert_eq!(parse_color("#fff").unwrap(), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_color("#ff8800").unwrap(), Rgba([255, 136, 0, 255]));
        assert!(parse_color("zzz").is_err());
        assert_eq!(parse_device("phone").unwrap(), Device::Phone);
        assert_eq!(parse_device("BROWSER").unwrap(), Device::Browser);
        assert!(parse_device("watch").is_err());
        assert_eq!(parse_frame_color("").unwrap(), FrameColor::Black);
        assert_eq!(parse_frame_color("silver").unwrap(), FrameColor::Silver);
        assert!(parse_frame_color("gold").is_err());
        assert_eq!(parse_background("transparent").unwrap(), Background::Transparent);
        assert_eq!(parse_background("gradient").unwrap(), Background::Gradient);
        assert!(parse_background("plaid").is_err());
    }

    #[test]
    fn phone_adds_bezel_and_padding() {
        let src = png(200, 400, Rgba([0, 120, 255, 255]));
        let out = frame(&src, &opts()).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        let (w, h) = img.dimensions();
        // Bezel grows the device beyond the shot, then padding adds 40 each side.
        assert!(w > 200 + 80, "phone wider than shot + padding");
        assert!(h > 400 + 80, "phone taller than shot + padding");
        // Solid-white backdrop shows in the corner.
        assert_eq!(img.get_pixel(0, 0), Rgba([255, 255, 255, 255]));
        // The shot color survives at the device center.
        let c = img.get_pixel(w / 2, h / 2);
        assert!(c.0[2] > 200 && c.0[0] < 60, "center is the blue shot");
    }

    #[test]
    fn laptop_is_wider_than_tall_base() {
        let src = png(400, 250, Rgba([10, 200, 90, 255]));
        let mut o = opts();
        o.device = Device::Laptop;
        let out = frame(&src, &o).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        let (w, h) = img.dimensions();
        // The base deck is wider than the lid, so total width exceeds shot+bezel+pad.
        assert!(w > 400 + 80, "laptop base widens the frame");
        assert!(h > 250 + 80, "laptop adds the base height");
    }

    #[test]
    fn browser_adds_chrome_bar_height() {
        let src = png(600, 300, Rgba([0, 0, 0, 255]));
        let mut o = opts();
        o.device = Device::Browser;
        o.browser_url = "gizza.example/tool".into();
        let out = frame(&src, &o).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        let (_w, h) = img.dimensions();
        // Bar height added above the shot; padding both sides.
        assert!(h > 300 + 80, "browser chrome bar adds height");
    }

    #[test]
    fn transparent_background_keeps_alpha_corner() {
        let src = png(120, 240, Rgba([255, 0, 0, 255]));
        let mut o = opts();
        o.background = Background::Transparent;
        let out = frame(&src, &o).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        // The padded corner is fully transparent (no backdrop drawn).
        assert_eq!(img.get_pixel(0, 0).0[3], 0, "transparent backdrop keeps alpha");
    }

    #[test]
    fn gradient_endpoints_differ() {
        let src = png(120, 240, Rgba([0, 0, 0, 255]));
        let mut o = opts();
        o.background = Background::Gradient;
        o.bg_color = Rgba([255, 0, 0, 255]);
        o.bg_color2 = Rgba([0, 0, 255, 255]);
        o.gradient_angle = 0.0; // left -> right
        let out = frame(&src, &o).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        let (w, h) = img.dimensions();
        let left = img.get_pixel(0, h / 2);
        let right = img.get_pixel(w - 1, h / 2);
        assert!(left.0[0] > right.0[0], "red dominates the left edge");
        assert!(right.0[2] > left.0[2], "blue dominates the right edge");
    }

    #[test]
    fn shadow_darkens_backdrop() {
        let src = png(160, 320, Rgba([0, 120, 255, 255]));
        let mut o = opts();
        o.shadow = true;
        o.shadow_blur = 16;
        o.shadow_opacity = 0.7;
        let out = frame(&src, &o).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        let (w, h) = img.dimensions();
        // Somewhere in the bottom padding band the white backdrop is darkened.
        let mut darkened = false;
        for x in (0..w).step_by(7) {
            let p = img.get_pixel(x, h - 8);
            if p.0[0] < 250 {
                darkened = true;
                break;
            }
        }
        assert!(darkened, "shadow should darken the backdrop below the device");
    }

    #[test]
    fn errors_on_bad_image() {
        assert!(frame(b"not an image", &opts()).is_err());
    }

    #[test]
    fn errors_on_oversize_output() {
        // A shot wider than MAX_DIM (8192) once the bezel + padding are added.
        let src = png(8300, 300, Rgba([0, 0, 0, 255]));
        let mut o = opts();
        o.padding = 64;
        assert!(frame(&src, &o).is_err(), "output beyond MAX_DIM must error");
    }
}
