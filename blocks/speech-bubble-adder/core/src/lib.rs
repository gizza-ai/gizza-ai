//! gizza-ai/speech-bubble-adder core — pure-Rust comic speech/thought bubbles
//! shared by the chat skill block and the CLI. No wafer/wasm-bindgen deps.
//!
//! Pipeline: decode the image → for each bubble build a closed boundary polygon
//! from a radial shape function (superellipse for the rounded/oval shapes, with a
//! scallop modulation for the cloud and a triangle-wave modulation for the
//! starburst) → splice the tail triangle into that polygon so the outline has no
//! seam across the tail base → scanline-fill it → stroke it (dashed for whisper)
//! → lay the caption out inside with word wrap and optional auto-fit → re-encode
//! to PNG. Glyphs are rasterized with a bundled font (fontdue — no freetype and
//! no system fonts), the same text stack as add-text-to-image / meme-caption.

use std::io::Cursor;

use fontdue::Font;
use image::{ImageFormat, RgbaImage};
use serde::Deserialize;

const FONT_BYTES: &[u8] = include_bytes!("assets/LiberationSans-Bold.ttf");

/// Polygon samples around the bubble boundary. 256 keeps the scalloped cloud and
/// the 11-point starburst smooth at poster sizes without a visible facet.
const POLY_SAMPLES: usize = 256;
/// Vertical subsamples per scanline in the polygon fill (anti-aliasing).
const SUBSAMPLES: usize = 4;
/// Half-width of the arc the tail is spliced into, in radians.
const TAIL_SPREAD: f32 = 0.30;

/// Bubble outline shape. `Caption` is the narrator box and never gets a tail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Speech,
    Oval,
    Thought,
    Shout,
    Whisper,
    Caption,
}

impl Style {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "speech" => Ok(Style::Speech),
            "oval" => Ok(Style::Oval),
            "thought" => Ok(Style::Thought),
            "shout" => Ok(Style::Shout),
            "whisper" => Ok(Style::Whisper),
            "caption" => Ok(Style::Caption),
            other => Err(format!(
                "unknown style '{other}' (use speech, oval, thought, shout, whisper, or caption)"
            )),
        }
    }

    /// Fraction of the bubble box (width, height) usable for text — the rounder
    /// the shape, the less of its bounding box the text can occupy.
    fn text_frac(self) -> (f32, f32) {
        match self {
            Style::Caption => (0.90, 0.84),
            Style::Speech | Style::Whisper => (0.84, 0.76),
            Style::Oval => (0.68, 0.60),
            Style::Thought => (0.62, 0.56),
            Style::Shout => (0.56, 0.50),
        }
    }
}

/// Where the tail leaves the bubble. Screen coordinates (y grows downward).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tail {
    BottomLeft,
    BottomCenter,
    BottomRight,
    TopLeft,
    TopCenter,
    TopRight,
    Left,
    Right,
    None,
}

impl Tail {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "bottom-left" => Ok(Tail::BottomLeft),
            "bottom-center" => Ok(Tail::BottomCenter),
            "bottom-right" => Ok(Tail::BottomRight),
            "top-left" => Ok(Tail::TopLeft),
            "top-center" => Ok(Tail::TopCenter),
            "top-right" => Ok(Tail::TopRight),
            "left" => Ok(Tail::Left),
            "right" => Ok(Tail::Right),
            "none" => Ok(Tail::None),
            other => Err(format!(
                "unknown tail '{other}' (use bottom-left, bottom-center, bottom-right, \
top-left, top-center, top-right, left, right, or none)"
            )),
        }
    }

    /// Direction angle in radians, measured with y growing downward.
    fn angle(self) -> Option<f32> {
        use std::f32::consts::PI;
        Some(match self {
            Tail::Right => 0.0,
            Tail::BottomRight => PI * 0.25,
            Tail::BottomCenter => PI * 0.5,
            Tail::BottomLeft => PI * 0.75,
            Tail::Left => PI,
            Tail::TopLeft => PI * 1.25,
            Tail::TopCenter => PI * 1.5,
            Tail::TopRight => PI * 1.75,
            Tail::None => return None,
        })
    }
}

/// One bubble as it arrives from the `bubbles` JSON array. Every field except
/// `text` is optional and falls back to the tool-level default.
#[derive(Debug, Clone, Deserialize)]
pub struct BubbleSpec {
    pub text: String,
    #[serde(default)]
    pub x: Option<f32>,
    #[serde(default)]
    pub y: Option<f32>,
    #[serde(default)]
    pub width: Option<f32>,
    #[serde(default)]
    pub height: Option<f32>,
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default)]
    pub tail: Option<String>,
    #[serde(default)]
    pub tail_x: Option<f32>,
    #[serde(default)]
    pub tail_y: Option<f32>,
    #[serde(default)]
    pub fill_color: Option<String>,
    #[serde(default)]
    pub text_color: Option<String>,
    #[serde(default)]
    pub outline_color: Option<String>,
    #[serde(default)]
    pub outline_width: Option<f32>,
    #[serde(default)]
    pub font_size: Option<f32>,
    #[serde(default)]
    pub uppercase: Option<bool>,
    #[serde(default)]
    pub shadow: Option<bool>,
}

/// Tool-level options: the first bubble's settings, which double as the defaults
/// for every entry in `bubbles_json`.
#[derive(Debug, Clone)]
pub struct Options {
    pub text: String,
    /// JSON array of ADDITIONAL bubbles; empty string = none.
    pub bubbles_json: String,
    pub x: f32,
    pub y: f32,
    /// 0 = auto-size from the text.
    pub width: f32,
    /// 0 = auto-size from the text.
    pub height: f32,
    pub style: String,
    pub tail: String,
    pub tail_x: Option<f32>,
    pub tail_y: Option<f32>,
    pub fill_color: String,
    pub text_color: String,
    pub outline_color: String,
    pub outline_width: f32,
    /// 0 = auto-fit to the bubble.
    pub font_size: f32,
    pub uppercase: bool,
    pub shadow: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            text: String::new(),
            bubbles_json: String::new(),
            x: 20.0,
            y: 20.0,
            width: 0.0,
            height: 0.0,
            style: "speech".into(),
            tail: "bottom-left".into(),
            tail_x: None,
            tail_y: None,
            fill_color: "#ffffff".into(),
            text_color: "#000000".into(),
            outline_color: "#000000".into(),
            outline_width: 3.0,
            font_size: 0.0,
            uppercase: false,
            shadow: false,
        }
    }
}

/// A bubble with every field resolved to a concrete value.
#[derive(Debug, Clone)]
struct Resolved {
    text: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: Style,
    tail: Tail,
    tail_pt: Option<(f32, f32)>,
    fill: [u8; 4],
    text_color: [u8; 4],
    outline: [u8; 4],
    outline_width: f32,
    font_size: f32,
    uppercase: bool,
    shadow: bool,
}

/// Parse `#rgb` / `#rrggbb` / `#rrggbbaa` (leading `#` optional) into RGBA.
/// 3- and 6-digit values are opaque; an 8-digit value carries its own alpha.
pub fn parse_color(s: &str) -> Result<[u8; 4], String> {
    let t = s.trim();
    if t.is_empty() {
        return Err("empty color".into());
    }
    let h = t.trim_start_matches('#');
    let hex = |c: &str| u8::from_str_radix(c, 16).map_err(|_| format!("invalid color '{s}'"));
    match h.len() {
        3 => {
            let cs: Vec<char> = h.chars().collect();
            let d = |c: char| {
                u8::from_str_radix(&format!("{c}{c}"), 16).map_err(|_| format!("invalid color '{s}'"))
            };
            Ok([d(cs[0])?, d(cs[1])?, d(cs[2])?, 255])
        }
        6 => Ok([hex(&h[0..2])?, hex(&h[2..4])?, hex(&h[4..6])?, 255]),
        8 => Ok([hex(&h[0..2])?, hex(&h[2..4])?, hex(&h[4..6])?, hex(&h[6..8])?]),
        _ => Err(format!("invalid color '{s}' (use #rgb, #rrggbb, or #rrggbbaa)")),
    }
}

/// Alpha-blend `rgba` (straight alpha) over the pixel at `(x, y)`, scaling its
/// alpha by `coverage` (0..1). Out-of-bounds and zero-alpha writes are skipped.
fn blend(img: &mut RgbaImage, x: i64, y: i64, rgba: [u8; 4], coverage: f32) {
    if x < 0 || y < 0 || x >= img.width() as i64 || y >= img.height() as i64 {
        return;
    }
    let a = (rgba[3] as f32 / 255.0) * coverage.clamp(0.0, 1.0);
    if a <= 0.0 {
        return;
    }
    let px = img.get_pixel_mut(x as u32, y as u32);
    for c in 0..3 {
        px[c] = (rgba[c] as f32 * a + px[c] as f32 * (1.0 - a))
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    px[3] = ((a + (px[3] as f32 / 255.0) * (1.0 - a)) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
}

/// Stamp a filled disk of radius `r` — the brush used for thick outlines.
fn stamp(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, rgba: [u8; 4]) {
    let ri = r.ceil() as i64;
    let (cxr, cyr) = (cx.round() as i64, cy.round() as i64);
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            if (dx * dx + dy * dy) as f32 <= (r * r) + 0.5 {
                blend(img, cxr + dx, cyr + dy, rgba, 1.0);
            }
        }
    }
}

/// Distance from the bubble centre to its boundary along `theta`, for a box with
/// half-extents `rx`/`ry`. Every style is a radial function of the angle, which
/// is what lets the tail splice cleanly into the boundary polygon.
fn radius_at(style: Style, rx: f32, ry: f32, theta: f32) -> f32 {
    let (c, s) = (theta.cos().abs().max(1e-6), theta.sin().abs().max(1e-6));
    // Superellipse |x/rx|^n + |y/ry|^n = 1; n=2 is an ellipse, larger n squarer.
    let superellipse = |n: f32| ((c / rx).powf(n) + (s / ry).powf(n)).powf(-1.0 / n);
    match style {
        // A true rectangle is the n → ∞ limit; compute it exactly.
        Style::Caption => (rx / c).min(ry / s),
        Style::Speech | Style::Whisper => superellipse(4.0),
        Style::Oval => superellipse(2.0),
        // Cloud: 11 outward scallops with cusps between them.
        Style::Thought => superellipse(2.0) * (0.90 + 0.10 * (11.0 * theta).sin().abs()),
        // Starburst: a triangle wave gives straight spike edges, not sine lobes.
        Style::Shout => {
            let tri = (2.0 / std::f32::consts::PI) * (11.0 * theta).sin().asin();
            superellipse(2.0) * (0.78 + 0.22 * tri)
        }
    }
}

/// Boundary point along the ray at `theta`.
fn boundary_point(style: Style, cx: f32, cy: f32, rx: f32, ry: f32, theta: f32) -> (f32, f32) {
    let r = radius_at(style, rx, ry, theta);
    (cx + r * theta.cos(), cy + r * theta.sin())
}

/// Normalize an angle into `[0, 2π)`.
fn norm_angle(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let m = a % tau;
    if m < 0.0 {
        m + tau
    } else {
        m
    }
}

/// Is `a` inside the arc centred on `mid` with half-width `half` (wrap-safe)?
fn in_arc(a: f32, mid: f32, half: f32) -> bool {
    let tau = std::f32::consts::TAU;
    let d = (norm_angle(a) - norm_angle(mid) + std::f32::consts::PI + tau) % tau - std::f32::consts::PI;
    d.abs() <= half
}

/// Build the closed bubble boundary, with the tail triangle spliced in so the
/// filled shape and its outline are one seamless path (no line across the tail
/// base). Returns the polygon in angle order.
fn bubble_polygon(b: &Resolved, cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<(f32, f32)> {
    let tau = std::f32::consts::TAU;
    // Thought bubbles trail puffs instead of a triangle; captions never point.
    let tail_dir = if b.style == Style::Thought || b.style == Style::Caption {
        None
    } else if let Some((tx, ty)) = b.tail_pt {
        Some((norm_angle((ty - cy).atan2(tx - cx)), (tx, ty)))
    } else {
        b.tail.angle().map(|a| {
            let (bx, by) = boundary_point(b.style, cx, cy, rx, ry, a);
            let len = (rx.min(ry) * 0.55).max(18.0);
            (a, (bx + len * a.cos(), by + len * a.sin()))
        })
    };

    let mut poly: Vec<(f32, f32)> = Vec::with_capacity(POLY_SAMPLES + 3);
    let inside = |theta: f32| match tail_dir {
        Some((mid, _)) => in_arc(theta, mid, TAIL_SPREAD),
        None => false,
    };
    let mut prev_inside = inside(tau * (POLY_SAMPLES - 1) as f32 / POLY_SAMPLES as f32);
    for i in 0..POLY_SAMPLES {
        let theta = tau * i as f32 / POLY_SAMPLES as f32;
        let now_inside = inside(theta);
        if now_inside {
            if !prev_inside {
                if let Some((mid, tip)) = tail_dir {
                    poly.push(boundary_point(b.style, cx, cy, rx, ry, mid - TAIL_SPREAD));
                    poly.push(tip);
                    poly.push(boundary_point(b.style, cx, cy, rx, ry, mid + TAIL_SPREAD));
                }
            }
        } else {
            poly.push(boundary_point(b.style, cx, cy, rx, ry, theta));
        }
        prev_inside = now_inside;
    }
    poly
}

/// Add horizontal coverage for the span `[x0, x1)` into a scanline accumulator.
fn add_span(cov: &mut [f32], bx0: i64, x0: f32, x1: f32, amt: f32) {
    let lo = x0.max(bx0 as f32);
    let hi = x1.min(bx0 as f32 + cov.len() as f32);
    if hi <= lo {
        return;
    }
    let i0 = lo.floor() as i64;
    let i1 = (hi.ceil() as i64) - 1;
    for i in i0..=i1 {
        let l = (i as f32).max(lo);
        let r = ((i + 1) as f32).min(hi);
        if r > l {
            let idx = i - bx0;
            if idx >= 0 && (idx as usize) < cov.len() {
                cov[idx as usize] += (r - l) * amt;
            }
        }
    }
}

/// Anti-aliased even-odd scanline fill of a closed polygon.
fn fill_poly(img: &mut RgbaImage, poly: &[(f32, f32)], rgba: [u8; 4]) {
    if poly.len() < 3 {
        return;
    }
    let (mut minx, mut miny, mut maxx, mut maxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(x, y) in poly {
        minx = minx.min(x);
        maxx = maxx.max(x);
        miny = miny.min(y);
        maxy = maxy.max(y);
    }
    let bx0 = (minx.floor() as i64).max(0);
    let bx1 = (maxx.ceil() as i64).min(img.width() as i64);
    let by0 = (miny.floor() as i64).max(0);
    let by1 = (maxy.ceil() as i64).min(img.height() as i64);
    if bx1 <= bx0 || by1 <= by0 {
        return;
    }
    let mut cov = vec![0.0f32; (bx1 - bx0) as usize];
    let mut xs: Vec<f32> = Vec::with_capacity(16);
    for py in by0..by1 {
        cov.iter_mut().for_each(|c| *c = 0.0);
        for s in 0..SUBSAMPLES {
            let sy = py as f32 + (s as f32 + 0.5) / SUBSAMPLES as f32;
            xs.clear();
            for i in 0..poly.len() {
                let (ax, ay) = poly[i];
                let (bx, by) = poly[(i + 1) % poly.len()];
                if (ay <= sy && by > sy) || (by <= sy && ay > sy) {
                    xs.push(ax + (sy - ay) / (by - ay) * (bx - ax));
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for pair in xs.chunks(2) {
                if let [x0, x1] = pair {
                    add_span(&mut cov, bx0, *x0, *x1, 1.0 / SUBSAMPLES as f32);
                }
            }
        }
        for (i, c) in cov.iter().enumerate() {
            if *c > 0.0 {
                blend(img, bx0 + i as i64, py, rgba, *c);
            }
        }
    }
}

/// Stroke a closed polygon with disk stamps. `dash` > 0 draws a dashed outline
/// (whisper style) by skipping every other `dash`-long run of arc length.
fn stroke_poly(img: &mut RgbaImage, poly: &[(f32, f32)], width: f32, rgba: [u8; 4], dash: f32) {
    if poly.len() < 2 || width <= 0.0 {
        return;
    }
    let r = (width / 2.0).max(0.5);
    let mut travelled = 0.0f32;
    for i in 0..poly.len() {
        let (ax, ay) = poly[i];
        let (bx, by) = poly[(i + 1) % poly.len()];
        let (dx, dy) = (bx - ax, by - ay);
        let len = (dx * dx + dy * dy).sqrt();
        if len <= 0.0 {
            continue;
        }
        let steps = (len / 0.5).ceil().max(1.0) as i64;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let at = travelled + len * t;
            if dash > 0.0 && ((at / dash) as i64) % 2 == 1 {
                continue;
            }
            stamp(img, ax + dx * t, ay + dy * t, r, rgba);
        }
        travelled += len;
    }
}

/// A circle as a polygon, used for the thought-bubble trailing puffs.
fn circle_poly(cx: f32, cy: f32, r: f32) -> Vec<(f32, f32)> {
    (0..48)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / 48.0;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// Width of `line` at `size` px in the bundled font.
fn text_width(font: &Font, line: &str, size: f32) -> f32 {
    line.chars().map(|c| font.metrics(c, size).advance_width).sum()
}

fn line_height(font: &Font, size: f32) -> f32 {
    font.horizontal_line_metrics(size)
        .map(|m| m.new_line_size)
        .unwrap_or(size * 1.25)
}

/// Greedy word wrap to `max_w` px. Honours `\n` as a hard break and splits words
/// that cannot fit on a line of their own.
fn wrap(font: &Font, text: &str, size: f32, max_w: f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if text_width(font, &candidate, size) <= max_w || line.is_empty() {
                // A single word wider than the box still has to be broken up.
                if line.is_empty() && text_width(font, word, size) > max_w {
                    let mut chunk = String::new();
                    for ch in word.chars() {
                        let trial = format!("{chunk}{ch}");
                        if !chunk.is_empty() && text_width(font, &trial, size) > max_w {
                            out.push(std::mem::take(&mut chunk));
                        }
                        chunk.push(ch);
                    }
                    line = chunk;
                } else {
                    line = candidate;
                }
            } else {
                out.push(std::mem::take(&mut line));
                line = word.to_string();
            }
        }
        out.push(line);
    }
    out
}

/// Widest wrapped line + total block height.
fn block_size(font: &Font, lines: &[String], size: f32) -> (f32, f32) {
    let w = lines
        .iter()
        .map(|l| text_width(font, l, size))
        .fold(0.0f32, f32::max);
    (w, line_height(font, size) * lines.len() as f32)
}

/// Merge a `bubbles` JSON entry over the tool-level defaults and validate it.
fn resolve(spec: &BubbleSpec, o: &Options) -> Result<Resolved, String> {
    if spec.text.trim().is_empty() {
        return Err("bubble text is empty".into());
    }
    let style = Style::parse(spec.style.as_deref().unwrap_or(&o.style))?;
    let tail = Tail::parse(spec.tail.as_deref().unwrap_or(&o.tail))?;
    let tx = spec.tail_x.or(o.tail_x);
    let ty = spec.tail_y.or(o.tail_y);
    let tail_pt = match (tx, ty) {
        (Some(a), Some(b)) => Some((a, b)),
        (None, None) => None,
        _ => return Err("tail_x and tail_y must be given together".into()),
    };
    let width = spec.width.unwrap_or(o.width).max(0.0);
    let height = spec.height.unwrap_or(o.height).max(0.0);
    let outline_width = spec.outline_width.unwrap_or(o.outline_width);
    if !(0.0..=64.0).contains(&outline_width) {
        return Err(format!("outline_width {outline_width} is out of range (0-64)"));
    }
    let font_size = spec.font_size.unwrap_or(o.font_size);
    if !(0.0..=512.0).contains(&font_size) {
        return Err(format!("font_size {font_size} is out of range (0-512, 0 = auto-fit)"));
    }
    Ok(Resolved {
        text: spec.text.clone(),
        x: spec.x.unwrap_or(o.x),
        y: spec.y.unwrap_or(o.y),
        width,
        height,
        style,
        tail,
        tail_pt,
        fill: parse_color(spec.fill_color.as_deref().unwrap_or(&o.fill_color))?,
        text_color: parse_color(spec.text_color.as_deref().unwrap_or(&o.text_color))?,
        outline: parse_color(spec.outline_color.as_deref().unwrap_or(&o.outline_color))?,
        outline_width,
        font_size,
        uppercase: spec.uppercase.unwrap_or(o.uppercase),
        shadow: spec.shadow.unwrap_or(o.shadow),
    })
}

/// Draw one resolved bubble onto `img`.
fn draw_bubble(img: &mut RgbaImage, font: &Font, b: &Resolved) {
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    let (fx, fy) = b.style.text_frac();
    let text = if b.uppercase {
        b.text.to_uppercase()
    } else {
        b.text.clone()
    };

    // ---- size the bubble + pick a font size -------------------------------
    // Auto font size scales with the image so a bubble reads on any resolution.
    let auto_size = (iw.min(ih) / 16.0).clamp(14.0, 72.0);
    let (mut w, mut h) = (b.width, b.height);
    let mut size = if b.font_size > 0.0 { b.font_size } else { auto_size };
    let mut lines;
    if w <= 0.0 {
        // Auto width: wrap at ~55% of the image, then grow the box around it.
        lines = wrap(font, &text, size, iw * 0.55);
        let (tw, th) = block_size(font, &lines, size);
        w = (tw / fx + size * 0.6).min(iw.max(1.0));
        if h <= 0.0 {
            h = th / fy + size * 0.6;
        }
    } else {
        if b.font_size <= 0.0 && h > 0.0 {
            // Both box dimensions given and no explicit size: shrink to fit.
            let mut best = 4.0f32;
            let mut probe = 4.0f32;
            while probe <= 512.0 {
                let ls = wrap(font, &text, probe, w * fx);
                let (tw, th) = block_size(font, &ls, probe);
                if tw <= w * fx && th <= h * fy {
                    best = probe;
                } else {
                    break;
                }
                probe += 1.0;
            }
            size = best;
        }
        lines = wrap(font, &text, size, w * fx);
        if h <= 0.0 {
            let (_, th) = block_size(font, &lines, size);
            h = th / fy + size * 0.6;
        }
    }
    lines = wrap(font, &text, size, w * fx);

    let (cx, cy) = (b.x + w / 2.0, b.y + h / 2.0);
    let (rx, ry) = (w / 2.0, h / 2.0);
    let poly = bubble_polygon(b, cx, cy, rx, ry);

    // ---- shadow, fill, outline -------------------------------------------
    if b.shadow {
        let off = (b.outline_width * 1.5).max(4.0);
        let shifted: Vec<(f32, f32)> = poly.iter().map(|&(x, y)| (x + off, y + off)).collect();
        fill_poly(img, &shifted, [0, 0, 0, 90]);
    }
    fill_poly(img, &poly, b.fill);
    let dash = if b.style == Style::Whisper {
        (b.outline_width * 2.5).max(6.0)
    } else {
        0.0
    };
    stroke_poly(img, &poly, b.outline_width, b.outline, dash);

    // ---- thought-bubble trailing puffs ------------------------------------
    if b.style == Style::Thought {
        if let Some(dir) = b
            .tail_pt
            .map(|(tx, ty)| norm_angle((ty - cy).atan2(tx - cx)))
            .or_else(|| b.tail.angle())
        {
            let (dx, dy) = (dir.cos(), dir.sin());
            let edge = radius_at(b.style, rx, ry, dir);
            let base = rx.min(ry);
            let reach = match b.tail_pt {
                Some((tx, ty)) => (((tx - cx).powi(2) + (ty - cy).powi(2)).sqrt() - edge).max(base * 0.5),
                None => (base * 0.75).max(24.0),
            };
            for (i, frac) in [0.28f32, 0.62, 0.95].iter().enumerate() {
                let r = base * (0.20 - 0.05 * i as f32);
                if r < 1.5 {
                    continue;
                }
                let d = edge + reach * frac + r;
                let puff = circle_poly(cx + dx * d, cy + dy * d, r);
                if b.shadow {
                    let off = (b.outline_width * 1.5).max(4.0);
                    let shifted: Vec<(f32, f32)> =
                        puff.iter().map(|&(x, y)| (x + off, y + off)).collect();
                    fill_poly(img, &shifted, [0, 0, 0, 90]);
                }
                fill_poly(img, &puff, b.fill);
                stroke_poly(img, &puff, b.outline_width, b.outline, 0.0);
            }
        }
    }

    // ---- caption text, centred in the bubble ------------------------------
    let lh = line_height(font, size);
    let (_, th) = block_size(font, &lines, size);
    let top = cy - th / 2.0;
    let ascent = font
        .horizontal_line_metrics(size)
        .map(|m| m.ascent)
        .unwrap_or(size * 0.8);
    for (li, line) in lines.iter().enumerate() {
        let lw = text_width(font, line, size);
        let mut pen_x = cx - lw / 2.0;
        let baseline = top + ascent + li as f32 * lh;
        for ch in line.chars() {
            let (m, bitmap) = font.rasterize(ch, size);
            if m.width > 0 && m.height > 0 {
                let left = pen_x + m.xmin as f32;
                let gtop = baseline - (m.height as f32 + m.ymin as f32);
                for gy in 0..m.height {
                    for gx in 0..m.width {
                        let cov = bitmap[gy * m.width + gx];
                        if cov != 0 {
                            blend(
                                img,
                                left as i64 + gx as i64,
                                gtop as i64 + gy as i64,
                                b.text_color,
                                cov as f32 / 255.0,
                            );
                        }
                    }
                }
            }
            pen_x += font.metrics(ch, size).advance_width;
        }
    }
}

/// Draw comic speech/thought bubbles onto `img_bytes` and return PNG bytes.
///
/// `o.text` is the first bubble; `o.bubbles_json` (a JSON array, may be empty)
/// adds more, each inheriting any field it omits from the tool-level options.
/// Bubbles are drawn in order, so later ones paint over earlier ones.
/// Errors on an undecodable image, empty text, a bad colour/style/tail value, a
/// half-specified tail aim point, or malformed `bubbles` JSON.
pub fn render(img_bytes: &[u8], o: &Options) -> Result<Vec<u8>, String> {
    if o.text.trim().is_empty() {
        return Err("text is empty".into());
    }
    let mut specs = vec![BubbleSpec {
        text: o.text.clone(),
        x: None,
        y: None,
        width: None,
        height: None,
        style: None,
        tail: None,
        tail_x: None,
        tail_y: None,
        fill_color: None,
        text_color: None,
        outline_color: None,
        outline_width: None,
        font_size: None,
        uppercase: None,
        shadow: None,
    }];
    let extra = o.bubbles_json.trim();
    if !extra.is_empty() {
        let parsed: Vec<BubbleSpec> = serde_json::from_str(extra).map_err(|e| {
            format!("failed to parse bubbles JSON (expected an array of bubble objects): {e}")
        })?;
        specs.extend(parsed);
    }
    let resolved: Vec<Resolved> = specs
        .iter()
        .map(|s| resolve(s, o))
        .collect::<Result<_, _>>()?;

    let mut img = image::load_from_memory(img_bytes)
        .map_err(|e| format!("failed to decode image: {e}"))?
        .to_rgba8();
    let font = Font::from_bytes(FONT_BYTES, fontdue::FontSettings::default())
        .map_err(|e| format!("font load failed: {e}"))?;

    for b in &resolved {
        draw_bubble(&mut img, &font, b);
    }

    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, image::Rgba(rgba));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    /// Black canvas — white bubble ink stands out against it.
    fn blank_png(w: u32, h: u32) -> Vec<u8> {
        solid_png(w, h, [0, 0, 0, 255])
    }

    fn decode(bytes: &[u8]) -> RgbaImage {
        image::load_from_memory(bytes).unwrap().to_rgba8()
    }

    fn opts(text: &str) -> Options {
        Options {
            text: text.into(),
            ..Default::default()
        }
    }

    /// Count pixels matching `rgb` exactly (the fill is drawn opaque, so the
    /// bubble interior lands on the exact colour).
    fn count(img: &RgbaImage, rgb: [u8; 3]) -> usize {
        img.pixels()
            .filter(|p| p[0] == rgb[0] && p[1] == rgb[1] && p[2] == rgb[2])
            .count()
    }

    #[test]
    fn draws_a_white_bubble_and_keeps_dimensions() {
        let src = blank_png(400, 300);
        let out = render(&src, &opts("Hello there")).unwrap();
        let img = decode(&out);
        assert_eq!((img.width(), img.height()), (400, 300));
        // A solid white balloon body now covers a real area of the black source.
        assert!(count(&img, [255, 255, 255]) > 2000, "bubble fill missing");
        // The default outline is black and the source is black, so check the
        // caption glyphs instead: black text sits inside the white fill.
        assert_ne!(out, src);
    }

    #[test]
    fn every_style_renders_and_differs() {
        let src = blank_png(400, 300);
        let mut seen: Vec<Vec<u8>> = Vec::new();
        for style in ["speech", "oval", "thought", "shout", "whisper", "caption"] {
            let out = render(
                &src,
                &Options {
                    style: style.into(),
                    width: 240.0,
                    height: 140.0,
                    font_size: 22.0,
                    ..opts("Pow")
                },
            )
            .unwrap();
            let img = decode(&out);
            assert!(
                count(&img, [255, 255, 255]) > 1000,
                "style {style} drew no fill"
            );
            assert!(
                !seen.contains(&out),
                "style {style} rendered identically to an earlier style"
            );
            seen.push(out);
        }
    }

    #[test]
    fn tail_direction_moves_ink() {
        let src = blank_png(400, 300);
        let base = Options {
            width: 160.0,
            height: 90.0,
            font_size: 20.0,
            x: 120.0,
            y: 100.0,
            ..opts("Hi")
        };
        let down = render(
            &src,
            &Options { tail: "bottom-center".into(), ..base.clone() },
        )
        .unwrap();
        let up = render(&src, &Options { tail: "top-center".into(), ..base.clone() }).unwrap();
        let none = render(&src, &Options { tail: "none".into(), ..base.clone() }).unwrap();
        assert_ne!(down, up, "tail direction had no effect");
        assert_ne!(down, none, "tail=none drew the same as a tail");
        // The box spans y 100..190, so ink outside that band can only be the tail.
        let (di, ui, ni) = (decode(&down), decode(&up), decode(&none));
        let ink = |img: &RgbaImage, y0: u32, y1: u32| {
            (y0..=y1)
                .flat_map(|y| (0..img.width()).map(move |x| (x, y)))
                .filter(|&(x, y)| img.get_pixel(x, y)[0] > 200)
                .count()
        };
        assert!(ink(&di, 192, 230) > 0, "bottom tail did not reach below the bubble");
        assert_eq!(ink(&di, 60, 98), 0, "bottom tail leaked above the bubble");
        assert!(ink(&ui, 60, 98) > 0, "top tail did not reach above the bubble");
        assert_eq!(ink(&ui, 192, 230), 0, "top tail leaked below the bubble");
        assert_eq!(ink(&ni, 192, 230) + ink(&ni, 60, 98), 0, "tail=none drew a tail");
    }

    #[test]
    fn tail_aim_point_overrides_direction() {
        let src = blank_png(400, 300);
        let base = Options {
            width: 140.0,
            height: 80.0,
            x: 40.0,
            y: 40.0,
            font_size: 18.0,
            ..opts("Look")
        };
        let aimed = render(
            &src,
            &Options { tail_x: Some(330.0), tail_y: Some(250.0), ..base.clone() },
        )
        .unwrap();
        let plain = render(&src, &base).unwrap();
        assert_ne!(aimed, plain);
        // The tip must land near (330, 250), far outside the bubble box.
        let img = decode(&aimed);
        let near_tip = (240..=330)
            .flat_map(|x| (180..=250).map(move |y| (x, y)))
            .filter(|&(x, y)| img.get_pixel(x, y)[0] > 200)
            .count();
        assert!(near_tip > 20, "tail did not reach the aim point");
    }

    #[test]
    fn colors_and_uppercase_apply() {
        let src = blank_png(300, 200);
        let out = render(
            &src,
            &Options {
                fill_color: "#f00".into(),
                text_color: "#0000ff".into(),
                outline_color: "#00ff00".into(),
                outline_width: 5.0,
                uppercase: true,
                width: 200.0,
                height: 120.0,
                font_size: 28.0,
                ..opts("hey")
            },
        )
        .unwrap();
        let img = decode(&out);
        // Short-hex fill, long-hex outline, and the blue caption all present.
        assert!(count(&img, [255, 0, 0]) > 1000, "short-hex fill missing");
        assert!(count(&img, [0, 255, 0]) > 200, "outline color missing");
        assert!(count(&img, [0, 0, 255]) > 5, "text color missing");
    }

    #[test]
    fn multiple_bubbles_are_all_drawn() {
        let src = blank_png(600, 300);
        let one = render(
            &src,
            &Options { width: 180.0, height: 100.0, font_size: 20.0, ..opts("First") },
        )
        .unwrap();
        let two = render(
            &src,
            &Options {
                width: 180.0,
                height: 100.0,
                font_size: 20.0,
                bubbles_json: r#"[{"text":"Second","x":340,"y":150,"style":"thought"}]"#.into(),
                ..opts("First")
            },
        )
        .unwrap();
        let (i1, i2) = (decode(&one), decode(&two));
        assert!(
            count(&i2, [255, 255, 255]) > count(&i1, [255, 255, 255]) + 1000,
            "the second bubble added no fill"
        );
    }

    #[test]
    fn auto_size_grows_with_longer_text() {
        let src = blank_png(600, 400);
        let short = decode(&render(&src, &opts("Hi")).unwrap());
        let long = decode(
            &render(
                &src,
                &opts("This caption is considerably longer and must wrap onto several lines"),
            )
            .unwrap(),
        );
        assert!(
            count(&long, [255, 255, 255]) > count(&short, [255, 255, 255]),
            "auto-sized bubble did not grow with the text"
        );
    }

    #[test]
    fn shadow_adds_dark_ink_outside_the_bubble() {
        // On a WHITE page: the shadow is translucent black, so it is only visible
        // against a light background (over the black canvas it is a no-op).
        let src = solid_png(300, 200, [255, 255, 255, 255]);
        // outline_width 12 => the shadow is offset 18px, well clear of the 6px stroke.
        let base = Options {
            width: 160.0,
            height: 90.0,
            font_size: 20.0,
            outline_width: 12.0,
            ..opts("Boo")
        };
        let plain = render(&src, &base).unwrap();
        let shadowed = render(&src, &Options { shadow: true, ..base }).unwrap();
        assert_ne!(plain, shadowed, "shadow toggle had no effect");
        // The box spans x 20..180; ink from x=188 rightward is shadow only.
        let grey = |bytes: &[u8]| {
            let img = decode(bytes);
            (55..=75)
                .flat_map(|y| (188..=196).map(move |x| (x, y)))
                .filter(|&(x, y)| (120..=210).contains(&img.get_pixel(x, y)[0]))
                .count()
        };
        assert!(grey(&shadowed) > 0, "no drop shadow beside the bubble");
        assert_eq!(grey(&plain), 0, "shadow ink present without shadow=true");
    }

    #[test]
    fn parse_color_variants() {
        assert_eq!(parse_color("#ff0000").unwrap(), [255, 0, 0, 255]);
        assert_eq!(parse_color("00ff00").unwrap(), [0, 255, 0, 255]);
        assert_eq!(parse_color("#abc").unwrap(), [170, 187, 204, 255]);
        assert_eq!(parse_color("#ff000080").unwrap(), [255, 0, 0, 128]);
        assert!(parse_color("nope").is_err());
        assert!(parse_color("").is_err());
    }

    #[test]
    fn wrap_breaks_on_words_and_hard_newlines() {
        let font = Font::from_bytes(FONT_BYTES, fontdue::FontSettings::default()).unwrap();
        // Just too narrow for "alpha beta" on one line, but wide enough for "gamma",
        // so the result isolates the word wrap from the hard `\n` break.
        let max_w = text_width(&font, "alpha beta", 20.0) - 1.0;
        assert!(text_width(&font, "gamma", 20.0) <= max_w);
        let lines = wrap(&font, "alpha beta\ngamma", 20.0, max_w);
        assert_eq!(lines, ["alpha", "beta", "gamma"]);
    }

    #[test]
    fn wrap_breaks_a_word_too_wide_for_the_box() {
        let font = Font::from_bytes(FONT_BYTES, fontdue::FontSettings::default()).unwrap();
        let narrow = text_width(&font, "gamma", 20.0) / 2.0;
        let lines = wrap(&font, "gamma", 20.0, narrow);
        assert!(lines.len() > 1, "over-wide word was not split: {lines:?}");
        assert_eq!(lines.concat(), "gamma", "splitting a word lost characters");
        for l in &lines {
            assert!(text_width(&font, l, 20.0) <= narrow || l.chars().count() == 1, "{l:?} still overflows");
        }
    }

    #[test]
    fn empty_text_errors() {
        let src = blank_png(50, 50);
        assert!(render(&src, &opts("   ")).unwrap_err().contains("text is empty"));
    }

    #[test]
    fn bad_image_errors() {
        assert!(render(b"not an image", &opts("hi"))
            .unwrap_err()
            .contains("failed to decode image"));
    }

    #[test]
    fn unknown_style_errors() {
        let src = blank_png(50, 50);
        let err = render(&src, &Options { style: "sparkle".into(), ..opts("hi") }).unwrap_err();
        assert!(err.contains("unknown style 'sparkle'"), "{err}");
    }

    #[test]
    fn unknown_tail_errors() {
        let src = blank_png(50, 50);
        let err = render(&src, &Options { tail: "sideways".into(), ..opts("hi") }).unwrap_err();
        assert!(err.contains("unknown tail 'sideways'"), "{err}");
    }

    #[test]
    fn half_specified_tail_point_errors() {
        let src = blank_png(50, 50);
        let err = render(&src, &Options { tail_x: Some(10.0), ..opts("hi") }).unwrap_err();
        assert!(err.contains("tail_x and tail_y must be given together"), "{err}");
    }

    #[test]
    fn malformed_bubbles_json_errors() {
        let src = blank_png(50, 50);
        let err = render(
            &src,
            &Options { bubbles_json: "{\"text\":\"x\"}".into(), ..opts("hi") },
        )
        .unwrap_err();
        assert!(err.contains("failed to parse bubbles JSON"), "{err}");
    }

    #[test]
    fn empty_bubble_text_in_array_errors() {
        let src = blank_png(50, 50);
        let err = render(
            &src,
            &Options { bubbles_json: r#"[{"text":"  "}]"#.into(), ..opts("hi") },
        )
        .unwrap_err();
        assert!(err.contains("bubble text is empty"), "{err}");
    }

    #[test]
    fn out_of_range_outline_width_errors() {
        let src = blank_png(50, 50);
        let err = render(&src, &Options { outline_width: 99.0, ..opts("hi") }).unwrap_err();
        assert!(err.contains("out of range (0-64)"), "{err}");
    }

    #[test]
    fn bad_color_errors() {
        let src = blank_png(50, 50);
        assert!(render(&src, &Options { fill_color: "xyz".into(), ..opts("hi") }).is_err());
    }
}

