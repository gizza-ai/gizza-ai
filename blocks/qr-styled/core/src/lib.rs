//! gizza-ai/qr-styled core — build a QR code and render it as a **styled** SVG:
//! custom foreground/background colours, an optional linear/radial gradient body,
//! square / rounded / dot module shapes, styled finder "eyes" (square / rounded /
//! circle) with an optional separate colour, and an optional embedded centre logo
//! (data:image URI) with a knockout behind it.
//!
//! Pure-Rust (`qrcode` only — the SVG is built by hand), so it runs on ALL
//! backends incl. the chat Service Worker. No wafer/wasm-bindgen deps.

use qrcode::types::Color;
use qrcode::{EcLevel, QrCode};
use std::fmt::Write as _;

/// Error-correction level. Higher levels survive more damage/occlusion at the
/// cost of denser codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ecc {
    Low,
    Medium,
    Quartile,
    High,
}

impl Ecc {
    pub fn parse(s: &str) -> Result<Ecc, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "l" | "low" => Ok(Ecc::Low),
            "m" | "medium" | "med" | "" => Ok(Ecc::Medium),
            "q" | "quartile" | "quart" => Ok(Ecc::Quartile),
            "h" | "high" => Ok(Ecc::High),
            other => Err(format!("unknown error correction '{other}' (use L, M, Q, or H)")),
        }
    }
    fn level(self) -> EcLevel {
        match self {
            Ecc::Low => EcLevel::L,
            Ecc::Medium => EcLevel::M,
            Ecc::Quartile => EcLevel::Q,
            Ecc::High => EcLevel::H,
        }
    }
}

/// Gradient applied to the body (module) fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gradient {
    None,
    Linear,
    Radial,
}

impl Gradient {
    pub fn parse(s: &str) -> Result<Gradient, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "none" | "" | "solid" => Ok(Gradient::None),
            "linear" => Ok(Gradient::Linear),
            "radial" => Ok(Gradient::Radial),
            other => Err(format!("unknown gradient '{other}' (use none, linear, or radial)")),
        }
    }
}

/// Shape drawn for each dark data module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleShape {
    Square,
    Rounded,
    Dots,
}

impl ModuleShape {
    pub fn parse(s: &str) -> Result<ModuleShape, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "square" | "" => Ok(ModuleShape::Square),
            "rounded" | "round" => Ok(ModuleShape::Rounded),
            "dots" | "dot" | "circle" => Ok(ModuleShape::Dots),
            other => Err(format!("unknown module_shape '{other}' (use square, rounded, or dots)")),
        }
    }
}

/// Shape drawn for the three finder-pattern "eyes".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EyeShape {
    Square,
    Rounded,
    Circle,
}

impl EyeShape {
    pub fn parse(s: &str) -> Result<EyeShape, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "square" | "" => Ok(EyeShape::Square),
            "rounded" | "round" => Ok(EyeShape::Rounded),
            "circle" | "circular" => Ok(EyeShape::Circle),
            other => Err(format!("unknown eye_shape '{other}' (use square, rounded, or circle)")),
        }
    }
}

/// All styling knobs. `data` is passed separately to [`generate`].
#[derive(Debug, Clone)]
pub struct Style {
    pub size: u32,
    pub margin: u32,
    pub ecc: Ecc,
    pub fg_color: String,
    /// Background colour hex, or the literal `transparent`.
    pub bg_color: String,
    pub gradient: Gradient,
    pub gradient_color: String,
    pub gradient_angle: f64,
    pub module_shape: ModuleShape,
    pub eye_shape: EyeShape,
    /// Empty = match `fg_color`.
    pub eye_color: String,
    /// Empty = no logo. Otherwise a `data:image/...` URI.
    pub logo: String,
    /// Logo edge as a fraction of the code width (0.1–0.35).
    pub logo_size: f64,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            size: 512,
            margin: 4,
            ecc: Ecc::Medium,
            fg_color: "#000000".into(),
            bg_color: "#ffffff".into(),
            gradient: Gradient::None,
            gradient_color: "#000000".into(),
            gradient_angle: 45.0,
            module_shape: ModuleShape::Square,
            eye_shape: EyeShape::Square,
            eye_color: String::new(),
            logo: String::new(),
            logo_size: 0.2,
        }
    }
}

/// Validate/normalise a hex colour like `#000` or `#ffffff`; returns it
/// lower-cased with a leading `#`.
fn norm_color(s: &str, field: &str) -> Result<String, String> {
    let s = s.trim();
    let body = s.strip_prefix('#').unwrap_or(s);
    let ok = matches!(body.len(), 3 | 6) && body.chars().all(|c| c.is_ascii_hexdigit());
    if !ok {
        return Err(format!("invalid {field} '{s}' (use #rgb or #rrggbb hex)"));
    }
    Ok(format!("#{}", body.to_ascii_lowercase()))
}

/// Background may also be the literal `transparent`.
fn norm_bg(s: &str) -> Result<Option<String>, String> {
    let t = s.trim();
    if t.eq_ignore_ascii_case("transparent") || t.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    Ok(Some(norm_color(t, "bg_color")?))
}

/// Escape a value for use inside an XML double-quoted attribute.
fn xml_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Compact float formatting (trims trailing zeros) so the SVG stays small and
/// unit tests stay legible.
fn f(x: f64) -> String {
    let s = format!("{x:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Path `d` for a rounded rectangle (r=0 → a plain rectangle).
fn rrect(x: f64, y: f64, w: f64, h: f64, r: f64) -> String {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    if r <= 0.0 {
        return format!(
            "M{} {}h{}v{}h{}z",
            f(x),
            f(y),
            f(w),
            f(h),
            f(-w),
        );
    }
    format!(
        "M{} {}h{}a{r0} {r0} 0 0 1 {r0} {r0}v{}a{r0} {r0} 0 0 1 {nr} {r0}h{}a{r0} {r0} 0 0 1 {nr} {nr}v{}a{r0} {r0} 0 0 1 {r0} {nr}z",
        f(x + r),
        f(y),
        f(w - 2.0 * r),
        f(h - 2.0 * r),
        f(-(w - 2.0 * r)),
        f(-(h - 2.0 * r)),
        r0 = f(r),
        nr = f(-r),
    )
}

/// Path `d` for a full circle centred at (cx,cy) using two arcs.
fn circle_path(cx: f64, cy: f64, r: f64) -> String {
    format!(
        "M{} {}a{r0} {r0} 0 1 0 {d} 0a{r0} {r0} 0 1 0 {nd} 0z",
        f(cx - r),
        f(cy),
        r0 = f(r),
        d = f(2.0 * r),
        nd = f(-2.0 * r),
    )
}

/// Is module (row, col) inside one of the three 7×7 finder patterns?
fn in_finder(row: usize, col: usize, n: usize) -> bool {
    let hit = |rs: usize, cs: usize| row >= rs && row < rs + 7 && col >= cs && col < cs + 7;
    hit(0, 0) || hit(0, n - 7) || hit(n - 7, 0)
}

/// Render one styled eye (finder pattern) at module origin (ex, ey).
fn render_eye(out: &mut String, ex: f64, ey: f64, shape: EyeShape, color: &str) {
    let cx = ex + 3.5;
    let cy = ey + 3.5;
    // Outer ring (7×7 outer, 5×5 hole) via even-odd, plus a 3×3 pupil.
    let (ring, pupil) = match shape {
        EyeShape::Square => (
            format!("{}{}", rrect(ex, ey, 7.0, 7.0, 0.0), rrect(ex + 1.0, ey + 1.0, 5.0, 5.0, 0.0)),
            rrect(ex + 2.0, ey + 2.0, 3.0, 3.0, 0.0),
        ),
        EyeShape::Rounded => (
            format!("{}{}", rrect(ex, ey, 7.0, 7.0, 2.0), rrect(ex + 1.0, ey + 1.0, 5.0, 5.0, 1.4)),
            rrect(ex + 2.0, ey + 2.0, 3.0, 3.0, 0.9),
        ),
        EyeShape::Circle => (
            format!("{}{}", circle_path(cx, cy, 3.5), circle_path(cx, cy, 2.5)),
            circle_path(cx, cy, 1.5),
        ),
    };
    let _ = write!(
        out,
        "<path fill-rule=\"evenodd\" fill=\"{c}\" d=\"{ring}\"/><path fill=\"{c}\" d=\"{pupil}\"/>",
        c = color,
    );
}

/// The rendered result.
pub struct Generated {
    /// UTF-8 SVG bytes.
    pub bytes: Vec<u8>,
    /// The raw payload that was encoded (handy for the summary).
    pub payload: String,
    /// The effective error-correction level (may be raised to High for a logo).
    pub ecc: Ecc,
}

/// Generate a styled QR code SVG. Returns the SVG bytes plus metadata.
pub fn generate(data: &str, style: &Style) -> Result<Generated, String> {
    if data.is_empty() {
        return Err("data is required".into());
    }
    let fg = norm_color(&style.fg_color, "fg_color")?;
    let bg = norm_bg(&style.bg_color)?;
    let grad_color = norm_color(&style.gradient_color, "gradient_color")?;
    let eye = if style.eye_color.trim().is_empty() {
        fg.clone()
    } else {
        norm_color(&style.eye_color, "eye_color")?
    };

    // A logo occludes the centre, so force High EC for reliable scanning.
    let has_logo = !style.logo.trim().is_empty();
    if has_logo && !style.logo.trim_start().starts_with("data:image/") {
        return Err("logo must be a data:image/... URI (no network fetch)".into());
    }
    let ecc = if has_logo { Ecc::High } else { style.ecc };

    let code = QrCode::with_error_correction_level(data.as_bytes(), ecc.level())
        .map_err(|e| format!("failed to build QR code: {e} (data may be too long)"))?;
    let colors = code.to_colors();
    let n = code.width();
    let margin = style.margin.min(64) as f64;
    let total = n as f64 + 2.0 * margin;
    let size = style.size.clamp(64, 4096);

    let body_fill = if style.gradient == Gradient::None {
        fg.clone()
    } else {
        "url(#g)".to_string()
    };

    let mut svg = String::with_capacity(4096);
    let _ = write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" viewBox=\"0 0 {vb} {vb}\" shape-rendering=\"crispEdges\">",
        vb = f(total),
    );

    // Gradient defs.
    if style.gradient != Gradient::None {
        svg.push_str("<defs>");
        match style.gradient {
            Gradient::Linear => {
                let ang = style.gradient_angle.to_radians();
                let (dx, dy) = (ang.cos(), ang.sin());
                let c = total / 2.0;
                let l = total / 2.0;
                let _ = write!(
                    svg,
                    "<linearGradient id=\"g\" gradientUnits=\"userSpaceOnUse\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"><stop offset=\"0\" stop-color=\"{fg}\"/><stop offset=\"1\" stop-color=\"{grad_color}\"/></linearGradient>",
                    f(c - dx * l),
                    f(c - dy * l),
                    f(c + dx * l),
                    f(c + dy * l),
                );
            }
            Gradient::Radial => {
                let _ = write!(
                    svg,
                    "<radialGradient id=\"g\" gradientUnits=\"userSpaceOnUse\" cx=\"{c}\" cy=\"{c}\" r=\"{r}\"><stop offset=\"0\" stop-color=\"{fg}\"/><stop offset=\"1\" stop-color=\"{grad_color}\"/></radialGradient>",
                    c = f(total / 2.0),
                    r = f(total / 2.0),
                );
            }
            Gradient::None => {}
        }
        svg.push_str("</defs>");
    }

    // Background.
    if let Some(bg) = &bg {
        let _ = write!(svg, "<rect width=\"{vb}\" height=\"{vb}\" fill=\"{bg}\"/>", vb = f(total));
    }

    // Body modules (skip the finder regions — eyes are drawn separately).
    let _ = write!(svg, "<g fill=\"{body_fill}\">");
    for row in 0..n {
        for col in 0..n {
            if colors[row * n + col] != Color::Dark {
                continue;
            }
            if in_finder(row, col, n) {
                continue;
            }
            let x = margin + col as f64;
            let y = margin + row as f64;
            match style.module_shape {
                ModuleShape::Square => {
                    let _ = write!(svg, "<rect x=\"{}\" y=\"{}\" width=\"1\" height=\"1\"/>", f(x), f(y));
                }
                ModuleShape::Rounded => {
                    let _ = write!(
                        svg,
                        "<rect x=\"{}\" y=\"{}\" width=\"1\" height=\"1\" rx=\"0.35\" ry=\"0.35\"/>",
                        f(x),
                        f(y),
                    );
                }
                ModuleShape::Dots => {
                    let _ = write!(
                        svg,
                        "<circle cx=\"{}\" cy=\"{}\" r=\"0.45\"/>",
                        f(x + 0.5),
                        f(y + 0.5),
                    );
                }
            }
        }
    }
    svg.push_str("</g>");

    // Styled eyes at the three finder origins.
    render_eye(&mut svg, margin, margin, style.eye_shape, &eye);
    render_eye(&mut svg, margin + (n - 7) as f64, margin, style.eye_shape, &eye);
    render_eye(&mut svg, margin, margin + (n - 7) as f64, style.eye_shape, &eye);

    // Centre logo with a knockout behind it.
    if has_logo {
        let frac = style.logo_size.clamp(0.1, 0.35);
        let ls = frac * n as f64;
        let center = margin + n as f64 / 2.0;
        let pad = ls * 0.14;
        let ko = ls + 2.0 * pad;
        let knockout = bg.clone().unwrap_or_else(|| "#ffffff".to_string());
        let _ = write!(
            svg,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"{knockout}\"/>",
            f(center - ko / 2.0),
            f(center - ko / 2.0),
            f(ko),
            f(ko),
            f(ko * 0.12),
            f(ko * 0.12),
        );
        let _ = write!(
            svg,
            "<image href=\"{href}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"xMidYMid meet\"/>",
            f(center - ls / 2.0),
            f(center - ls / 2.0),
            f(ls),
            f(ls),
            href = xml_attr(style.logo.trim()),
        );
    }

    svg.push_str("</svg>");

    Ok(Generated { bytes: svg.into_bytes(), payload: data.to_string(), ecc })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svg_of(g: &Generated) -> String {
        String::from_utf8(g.bytes.clone()).unwrap()
    }

    #[test]
    fn generates_basic_svg() {
        let g = generate("hello world", &Style::default()).unwrap();
        let s = svg_of(&g);
        assert!(s.starts_with("<svg"));
        assert!(s.ends_with("</svg>"));
        assert!(s.contains("fill=\"#000000\"")); // solid fg, no gradient
        assert!(s.contains("<rect")); // background + square modules
        assert!(!s.contains("url(#g)"));
        assert_eq!(g.payload, "hello world");
        assert_eq!(g.ecc, Ecc::Medium);
    }

    #[test]
    fn linear_gradient_defs_and_fill() {
        let style = Style { gradient: Gradient::Linear, gradient_color: "#f0f".into(), ..Style::default() };
        let s = svg_of(&generate("data", &style).unwrap());
        assert!(s.contains("<linearGradient id=\"g\""));
        assert!(s.contains("stop-color=\"#f0f\"")); // 3-char hex kept as-is
        assert!(s.contains("fill=\"url(#g)\""));
    }

    #[test]
    fn radial_gradient_defs() {
        let style = Style { gradient: Gradient::Radial, ..Style::default() };
        let s = svg_of(&generate("data", &style).unwrap());
        assert!(s.contains("<radialGradient id=\"g\""));
        assert!(s.contains("fill=\"url(#g)\""));
    }

    #[test]
    fn transparent_bg_has_no_bg_rect() {
        let opaque = svg_of(&generate("x", &Style::default()).unwrap());
        let bg_rects = opaque.matches("width=\"").count();
        let style = Style { bg_color: "transparent".into(), ..Style::default() };
        let s = svg_of(&generate("x", &style).unwrap());
        // No full-canvas background rect when transparent.
        assert!(!s.contains(&format!("<rect width=")));
        assert!(bg_rects >= 1);
    }

    #[test]
    fn dot_modules_and_circle_eyes() {
        let style = Style {
            module_shape: ModuleShape::Dots,
            eye_shape: EyeShape::Circle,
            ..Style::default()
        };
        let s = svg_of(&generate("dots please", &style).unwrap());
        assert!(s.contains("<circle")); // dot modules
        assert!(s.contains("fill-rule=\"evenodd\"")); // circular eye ring
    }

    #[test]
    fn eye_color_separate_from_body() {
        let style = Style { eye_color: "#f00".into(), ..Style::default() };
        let s = svg_of(&generate("x", &style).unwrap());
        assert!(s.contains("fill=\"#f00\" d=")); // eye ring/pupil use eye colour
    }

    #[test]
    fn logo_forces_high_ecc_and_embeds() {
        let uri = "data:image/png;base64,iVBORw0KGgo=";
        let style = Style { logo: uri.into(), ecc: Ecc::Low, ..Style::default() };
        let g = generate("https://example.com", &style).unwrap();
        let s = svg_of(&g);
        assert_eq!(g.ecc, Ecc::High); // raised despite requesting Low
        assert!(s.contains(&format!("<image href=\"{uri}\"")));
    }

    #[test]
    fn errors() {
        // empty data
        assert!(generate("", &Style::default()).is_err());
        // bad fg colour
        let bad = Style { fg_color: "#xyz".into(), ..Style::default() };
        assert!(generate("x", &bad).is_err());
        // logo must be a data:image URI
        let net = Style { logo: "https://evil.example/x.png".into(), ..Style::default() };
        assert!(generate("x", &net).is_err());
    }

    #[test]
    fn parsers() {
        assert_eq!(Ecc::parse("H").unwrap(), Ecc::High);
        assert!(Ecc::parse("z").is_err());
        assert_eq!(Gradient::parse("linear").unwrap(), Gradient::Linear);
        assert!(Gradient::parse("diagonal").is_err());
        assert_eq!(ModuleShape::parse("dots").unwrap(), ModuleShape::Dots);
        assert!(ModuleShape::parse("star").is_err());
        assert_eq!(EyeShape::parse("circle").unwrap(), EyeShape::Circle);
        assert!(EyeShape::parse("hex").is_err());
    }
}
