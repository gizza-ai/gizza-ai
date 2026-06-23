//! gizza-ai/gradient-image-generator core — render a solid, linear-gradient, or
//! radial-gradient image at a chosen size, as a PNG (raster) or SVG (scalable).
//! Pure-Rust (`image` for PNG; hand-built SVG string), so it runs on ALL
//! backends incl. the chat Service Worker. No wafer/wasm-bindgen deps.

/// The kind of fill to render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientType {
    /// A single flat colour (the first stop).
    Solid,
    /// A linear gradient along a direction angle.
    Linear,
    /// A radial gradient from the centre outward.
    Radial,
}

impl GradientType {
    pub fn parse(s: &str) -> Result<GradientType, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "solid" | "flat" => Ok(GradientType::Solid),
            "linear" | "" => Ok(GradientType::Linear),
            "radial" | "circle" => Ok(GradientType::Radial),
            other => Err(format!("unknown gradient type '{other}' (use solid, linear, or radial)")),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            GradientType::Solid => "solid",
            GradientType::Linear => "linear",
            GradientType::Radial => "radial",
        }
    }
}

/// Output image format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Png,
    Svg,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "png" | "" => Ok(Format::Png),
            "svg" => Ok(Format::Svg),
            other => Err(format!("unknown format '{other}' (use png or svg)")),
        }
    }
    pub fn mime(self) -> &'static str {
        match self {
            Format::Png => "image/png",
            Format::Svg => "image/svg+xml",
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Svg => "svg",
        }
    }
}

/// An RGBA colour stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

/// Parse a CSS hex colour: #rgb, #rgba, #rrggbb, or #rrggbbaa (with or without
/// the leading '#'). Returns RGBA with alpha defaulting to opaque.
fn parse_hex(s: &str) -> Result<Rgba, String> {
    let body = s.trim().trim_start_matches('#');
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("invalid colour '{s}' (use #rgb, #rgba, #rrggbb, or #rrggbbaa hex)"));
    }
    let expand = |c: char| -> u8 {
        let v = c.to_digit(16).unwrap() as u8;
        v << 4 | v
    };
    let two = |a: char, b: char| -> u8 {
        (a.to_digit(16).unwrap() as u8) << 4 | (b.to_digit(16).unwrap() as u8)
    };
    let ch: Vec<char> = body.chars().collect();
    match ch.len() {
        3 => Ok(Rgba { r: expand(ch[0]), g: expand(ch[1]), b: expand(ch[2]), a: 255 }),
        4 => Ok(Rgba { r: expand(ch[0]), g: expand(ch[1]), b: expand(ch[2]), a: expand(ch[3]) }),
        6 => Ok(Rgba { r: two(ch[0], ch[1]), g: two(ch[2], ch[3]), b: two(ch[4], ch[5]), a: 255 }),
        8 => Ok(Rgba {
            r: two(ch[0], ch[1]),
            g: two(ch[2], ch[3]),
            b: two(ch[4], ch[5]),
            a: two(ch[6], ch[7]),
        }),
        _ => Err(format!("invalid colour '{s}' (use #rgb, #rgba, #rrggbb, or #rrggbbaa hex)")),
    }
}

/// Parse the comma/whitespace/newline-separated colour-stop list. Requires at
/// least one stop; the stops are spread evenly across the gradient axis.
fn parse_stops(colors: &str) -> Result<Vec<Rgba>, String> {
    let stops: Vec<Rgba> = colors
        .split([',', '\n', ' ', '\t'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(parse_hex)
        .collect::<Result<Vec<_>, _>>()?;
    if stops.is_empty() {
        return Err("at least one colour is required (e.g. \"#ff0000,#0000ff\")".into());
    }
    Ok(stops)
}

/// Linearly interpolate one channel.
fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8
}

/// Sample the colour ramp at position `t` in [0,1] across the evenly-spaced
/// stops. With one stop the ramp is that flat colour.
fn sample(stops: &[Rgba], t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    if stops.len() == 1 {
        return stops[0];
    }
    let n = stops.len() - 1;
    let scaled = t * n as f32;
    let i = (scaled.floor() as usize).min(n - 1);
    let frac = scaled - i as f32;
    let a = stops[i];
    let b = stops[i + 1];
    Rgba {
        r: lerp(a.r, b.r, frac),
        g: lerp(a.g, b.g, frac),
        b: lerp(a.b, b.b, frac),
        a: lerp(a.a, b.a, frac),
    }
}

/// The rendered gradient image bytes.
pub struct Generated {
    pub bytes: Vec<u8>,
    pub format: Format,
    pub width: u32,
    pub height: u32,
    pub gradient_type: GradientType,
    pub stop_count: usize,
}

/// Generate the gradient image.
///
/// * `gradient_type` — solid / linear / radial.
/// * `colors` — comma-separated hex stops (solid uses the first).
/// * `width`/`height` — image size in pixels (clamped to 1..=4096).
/// * `angle` — for linear: the direction in degrees (0 = left→right,
///   90 = top→bottom, measured clockwise). Ignored otherwise.
/// * `format` — PNG or SVG.
pub fn generate(
    gradient_type: GradientType,
    colors: &str,
    width: u32,
    height: u32,
    angle: f64,
    format: Format,
) -> Result<Generated, String> {
    let stops = parse_stops(colors)?;
    let w = width.clamp(1, 4096);
    let h = height.clamp(1, 4096);

    let bytes = match format {
        Format::Png => render_png(gradient_type, &stops, w, h, angle)?,
        Format::Svg => render_svg(gradient_type, &stops, w, h, angle).into_bytes(),
    };
    Ok(Generated {
        bytes,
        format,
        width: w,
        height: h,
        gradient_type,
        stop_count: stops.len(),
    })
}

/// The normalised position [0,1] of a pixel along the gradient axis.
fn axis_pos(
    gradient_type: GradientType,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    dir: (f32, f32),
) -> f32 {
    match gradient_type {
        GradientType::Solid => 0.0,
        GradientType::Linear => {
            // Project the pixel centre (normalised to [0,1] in each axis) onto
            // the unit direction vector, then map the projection range to [0,1].
            let nx = if w > 1 { x as f32 / (w - 1) as f32 } else { 0.0 };
            let ny = if h > 1 { y as f32 / (h - 1) as f32 } else { 0.0 };
            let (dx, dy) = dir;
            let proj = nx * dx + ny * dy;
            // Range of nx*dx + ny*dy over the unit square.
            let lo = dx.min(0.0) + dy.min(0.0);
            let hi = dx.max(0.0) + dy.max(0.0);
            if (hi - lo).abs() < f32::EPSILON {
                0.0
            } else {
                (proj - lo) / (hi - lo)
            }
        }
        GradientType::Radial => {
            // Distance from centre, normalised so the farthest corner = 1.
            let cx = (w - 1) as f32 / 2.0;
            let cy = (h - 1) as f32 / 2.0;
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let max = (cx * cx + cy * cy).sqrt();
            if max < f32::EPSILON {
                0.0
            } else {
                dist / max
            }
        }
    }
}

fn render_png(
    gradient_type: GradientType,
    stops: &[Rgba],
    w: u32,
    h: u32,
    angle: f64,
) -> Result<Vec<u8>, String> {
    use image::{ImageEncoder, RgbaImage};
    let dir = angle_to_dir(angle);
    let mut img = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let t = axis_pos(gradient_type, x, y, w, h, dir);
            let c = sample(stops, t);
            img.put_pixel(x, y, image::Rgba([c.r, c.g, c.b, c.a]));
        }
    }
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&img, w, h, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    Ok(out)
}

/// Convert a direction angle (degrees, 0 = left→right, clockwise) to a unit vector.
fn angle_to_dir(angle: f64) -> (f32, f32) {
    let rad = (angle as f32).to_radians();
    (rad.cos(), rad.sin())
}

fn hex6(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn alpha(c: Rgba) -> f32 {
    c.a as f32 / 255.0
}

fn render_svg(
    gradient_type: GradientType,
    stops: &[Rgba],
    w: u32,
    h: u32,
    angle: f64,
) -> String {
    let head = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">"
    );
    if gradient_type == GradientType::Solid || stops.len() == 1 {
        let c = stops[0];
        return format!(
            "{head}<rect width=\"{w}\" height=\"{h}\" fill=\"{}\" fill-opacity=\"{}\"/></svg>",
            hex6(c),
            alpha(c)
        );
    }

    // Build evenly-spaced stop elements.
    let n = stops.len() - 1;
    let mut stop_tags = String::new();
    for (i, c) in stops.iter().enumerate() {
        let off = (i as f32 / n as f32 * 100.0).round();
        stop_tags.push_str(&format!(
            "<stop offset=\"{off}%\" stop-color=\"{}\" stop-opacity=\"{}\"/>",
            hex6(*c),
            alpha(*c)
        ));
    }

    let def = match gradient_type {
        GradientType::Linear => {
            // Map the angle to x1/y1→x2/y2 endpoints on the unit square so the
            // gradient fills the box edge-to-edge.
            let (dx, dy) = angle_to_dir(angle);
            let x1 = if dx < 0.0 { 1.0 } else { 0.0 };
            let y1 = if dy < 0.0 { 1.0 } else { 0.0 };
            let x2 = if dx < 0.0 { 0.0 } else { 1.0 };
            let y2 = if dy < 0.0 { 0.0 } else { 1.0 };
            // If a component is ~0, collapse that axis so it doesn't skew.
            let (x1, x2) = if dx.abs() < f32::EPSILON { (0.0, 0.0) } else { (x1, x2) };
            let (y1, y2) = if dy.abs() < f32::EPSILON { (0.0, 0.0) } else { (y1, y2) };
            format!(
                "<defs><linearGradient id=\"g\" x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\">{stop_tags}</linearGradient></defs>"
            )
        }
        GradientType::Radial => format!(
            "<defs><radialGradient id=\"g\" cx=\"0.5\" cy=\"0.5\" r=\"0.7071\">{stop_tags}</radialGradient></defs>"
        ),
        GradientType::Solid => unreachable!(),
    };

    format!("{head}{def}<rect width=\"{w}\" height=\"{h}\" fill=\"url(#g)\"/></svg>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_variants() {
        assert_eq!(parse_hex("#f00").unwrap(), Rgba { r: 255, g: 0, b: 0, a: 255 });
        assert_eq!(parse_hex("00ff00").unwrap(), Rgba { r: 0, g: 255, b: 0, a: 255 });
        assert_eq!(parse_hex("#0000ff80").unwrap(), Rgba { r: 0, g: 0, b: 255, a: 128 });
        assert_eq!(parse_hex("#f008").unwrap(), Rgba { r: 255, g: 0, b: 0, a: 136 });
        assert!(parse_hex("#xyz").is_err());
        assert!(parse_hex("").is_err());
        assert!(parse_hex("#12345").is_err());
    }

    #[test]
    fn parses_stop_list() {
        let s = parse_stops("#f00, #0f0\n#00f").unwrap();
        assert_eq!(s.len(), 3);
        assert_eq!(s[1], Rgba { r: 0, g: 255, b: 0, a: 255 });
        assert!(parse_stops("   ").is_err());
    }

    #[test]
    fn sample_endpoints_and_midpoint() {
        let stops = vec![
            Rgba { r: 0, g: 0, b: 0, a: 255 },
            Rgba { r: 255, g: 255, b: 255, a: 255 },
        ];
        assert_eq!(sample(&stops, 0.0), stops[0]);
        assert_eq!(sample(&stops, 1.0), stops[1]);
        let mid = sample(&stops, 0.5);
        assert!((mid.r as i32 - 128).abs() <= 1);
    }

    #[test]
    fn generates_png_with_size() {
        let g = generate(GradientType::Linear, "#ff0000,#0000ff", 64, 32, 0.0, Format::Png).unwrap();
        assert_eq!(g.format, Format::Png);
        assert_eq!((g.width, g.height), (64, 32));
        // PNG magic bytes.
        assert_eq!(&g.bytes[0..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    }

    #[test]
    fn generates_svg_linear_and_radial() {
        let lin = generate(GradientType::Linear, "#f00,#00f", 100, 100, 90.0, Format::Svg).unwrap();
        let s = String::from_utf8(lin.bytes).unwrap();
        assert!(s.contains("<linearGradient"));
        assert!(s.contains("stop-color=\"#ff0000\""));
        assert!(s.contains("stop-color=\"#0000ff\""));

        let rad = generate(GradientType::Radial, "#fff,#000", 50, 50, 0.0, Format::Svg).unwrap();
        let s2 = String::from_utf8(rad.bytes).unwrap();
        assert!(s2.contains("<radialGradient"));
    }

    #[test]
    fn solid_uses_first_stop() {
        let g = generate(GradientType::Solid, "#112233,#445566", 10, 10, 0.0, Format::Svg).unwrap();
        let s = String::from_utf8(g.bytes).unwrap();
        assert!(s.contains("fill=\"#112233\""));
        assert!(!s.contains("Gradient"));
    }

    #[test]
    fn size_is_clamped() {
        let g = generate(GradientType::Solid, "#fff", 0, 99999, 0.0, Format::Png).unwrap();
        assert_eq!((g.width, g.height), (1, 4096));
    }

    #[test]
    fn parsers_and_errors() {
        assert_eq!(GradientType::parse("RADIAL").unwrap(), GradientType::Radial);
        assert!(GradientType::parse("spiral").is_err());
        assert_eq!(Format::parse("PNG").unwrap(), Format::Png);
        assert!(Format::parse("gif").is_err());
        // empty colours error
        assert!(generate(GradientType::Linear, "", 10, 10, 0.0, Format::Png).is_err());
        // bad colour errors
        assert!(generate(GradientType::Linear, "#zzz", 10, 10, 0.0, Format::Png).is_err());
    }

    #[test]
    fn linear_axis_spans_full_range() {
        // Horizontal (angle 0): leftmost ~0, rightmost ~1.
        let dir = angle_to_dir(0.0);
        let left = axis_pos(GradientType::Linear, 0, 0, 100, 100, dir);
        let right = axis_pos(GradientType::Linear, 99, 0, 100, 100, dir);
        assert!(left < 0.01);
        assert!(right > 0.99);
    }
}
