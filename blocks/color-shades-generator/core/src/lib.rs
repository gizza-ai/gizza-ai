//! color-shades-generator core — produce a ramp of tints, shades and tones for a
//! base color, including a Tailwind-style 50-900 named scale. Pure-Rust,
//! dependency-free besides serde for the output struct.
//!
//! Modes:
//! - `scale`  — a Tailwind-style 50,100,…,900 ramp: light tints at the top,
//!              dark shades at the bottom, with the base placed at the nearest
//!              step. Each step has a stable weight name.
//! - `tints`  — N steps lightening the base toward white.
//! - `shades` — N steps darkening the base toward black.
//! - `tones`  — N steps desaturating the base toward middle gray.

use serde::Serialize;

/// One color in the ramp.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Step {
    /// Step name: a Tailwind weight (`"500"`) for `scale`, or an index (`"1"`..)
    /// for the tints/shades/tones series.
    pub name: String,
    pub hex: String,
    pub rgb: String,
    pub hsl: String,
    /// True for the step that matches the base color (only set in `scale`).
    pub is_base: bool,
}

/// The generated ramp.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ramp {
    /// The mode that was applied.
    pub mode: String,
    /// The base color, normalized to #rrggbb.
    pub base: String,
    /// The ramp steps, lightest first.
    pub steps: Vec<Step>,
}

fn round(v: f64) -> i64 {
    v.round() as i64
}

/// Parse a color string into (r, g, b). Accepts `#rgb`, `#rrggbb`, bare hex,
/// `rgb(...)`, and `hsl(...)`.
fn parse(input: &str) -> Result<(u8, u8, u8), String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("no base color given".into());
    }
    let lower = s.to_ascii_lowercase();

    if let Some(hex) = lower.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(rest) = lower.strip_prefix("rgba").and_then(paren) {
        return parse_rgb(&rest);
    }
    if let Some(rest) = lower.strip_prefix("rgb").and_then(paren) {
        return parse_rgb(&rest);
    }
    if let Some(rest) = lower.strip_prefix("hsla").and_then(paren) {
        return parse_hsl(&rest);
    }
    if let Some(rest) = lower.strip_prefix("hsl").and_then(paren) {
        return parse_hsl(&rest);
    }
    if lower.chars().all(|c| c.is_ascii_hexdigit()) {
        return parse_hex(&lower);
    }
    Err("unrecognized color (use #hex, rgb(), or hsl())".into())
}

fn paren(s: &str) -> Option<String> {
    let s = s.trim();
    let inner = s.strip_prefix('(')?.strip_suffix(')')?;
    Some(inner.to_string())
}

fn parse_hex(hex: &str) -> Result<(u8, u8, u8), String> {
    let h = hex.trim();
    let expand = |c: char| -> String { format!("{c}{c}") };
    let (r, g, b) = match h.len() {
        3 | 4 => {
            let cs: Vec<char> = h.chars().collect();
            (expand(cs[0]), expand(cs[1]), expand(cs[2]))
        }
        6 | 8 => (h[0..2].into(), h[2..4].into(), h[4..6].into()),
        _ => return Err("hex color must be 3, 4, 6, or 8 digits".into()),
    };
    let p = |x: &str| u8::from_str_radix(x, 16).map_err(|_| "invalid hex digit".to_string());
    Ok((p(&r)?, p(&g)?, p(&b)?))
}

fn parse_rgb(inner: &str) -> Result<(u8, u8, u8), String> {
    let parts: Vec<&str> = inner
        .split(|c| c == ',' || c == '/')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() < 3 {
        return Err("rgb needs at least 3 components".into());
    }
    let comp = |x: &str| -> Result<u8, String> {
        if let Some(pct) = x.strip_suffix('%') {
            let v: f64 = pct.trim().parse().map_err(|_| "bad rgb percent".to_string())?;
            Ok(round(v / 100.0 * 255.0).clamp(0, 255) as u8)
        } else {
            let v: f64 = x.parse().map_err(|_| "bad rgb component".to_string())?;
            Ok(round(v).clamp(0, 255) as u8)
        }
    };
    Ok((comp(parts[0])?, comp(parts[1])?, comp(parts[2])?))
}

fn parse_hsl(inner: &str) -> Result<(u8, u8, u8), String> {
    let parts: Vec<&str> = inner
        .split(|c| c == ',' || c == '/')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() < 3 {
        return Err("hsl needs at least 3 components".into());
    }
    let h: f64 = parts[0]
        .trim_end_matches("deg")
        .trim()
        .parse()
        .map_err(|_| "bad hue".to_string())?;
    let s: f64 = parts[1]
        .trim_end_matches('%')
        .trim()
        .parse::<f64>()
        .map_err(|_| "bad saturation".to_string())?
        / 100.0;
    let l: f64 = parts[2]
        .trim_end_matches('%')
        .trim()
        .parse::<f64>()
        .map_err(|_| "bad lightness".to_string())?
        / 100.0;
    Ok(hsl_to_rgb(h.rem_euclid(360.0), s.clamp(0.0, 1.0), l.clamp(0.0, 1.0)))
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = match (h / 60.0) as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        round((r1 + m) * 255.0).clamp(0, 255) as u8,
        round((g1 + m) * 255.0).clamp(0, 255) as u8,
        round((b1 + m) * 255.0).clamp(0, 255) as u8,
    )
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let (rf, gf, bf) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let d = max - min;
    let l = (max + min) / 2.0;
    if d == 0.0 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == rf {
        60.0 * (((gf - bf) / d).rem_euclid(6.0))
    } else if max == gf {
        60.0 * ((bf - rf) / d + 2.0)
    } else {
        60.0 * ((rf - gf) / d + 4.0)
    };
    (h.rem_euclid(360.0), s, l)
}

fn step_from_hsl(name: String, h: f64, s: f64, l: f64, is_base: bool) -> Step {
    let h = h.rem_euclid(360.0);
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let (r, g, b) = hsl_to_rgb(h, s, l);
    Step {
        name,
        hex: format!("#{r:02x}{g:02x}{b:02x}"),
        rgb: format!("rgb({r}, {g}, {b})"),
        hsl: format!("hsl({}, {}%, {}%)", round(h), round(s * 100.0), round(l * 100.0)),
        is_base,
    }
}

/// All supported modes.
pub const MODES: &[&str] = &["scale", "tints", "shades", "tones"];

/// Tailwind-style weight labels and their target lightness (fraction 0..1).
/// 50 is near-white, 950 is near-black (Tailwind v3.3+/v4 added the 950 weight).
/// Saturation is preserved from the base.
const SCALE_WEIGHTS: &[(&str, f64)] = &[
    ("50", 0.97),
    ("100", 0.94),
    ("200", 0.86),
    ("300", 0.76),
    ("400", 0.65),
    ("500", 0.54),
    ("600", 0.45),
    ("700", 0.36),
    ("800", 0.27),
    ("900", 0.18),
    ("950", 0.11),
];

fn canonical_mode(s: &str) -> Result<String, String> {
    let key = s.trim().to_ascii_lowercase();
    let key = match key.as_str() {
        "tailwind" | "ramp" | "" => "scale".to_string(),
        "tint" => "tints".to_string(),
        "shade" => "shades".to_string(),
        "tone" => "tones".to_string(),
        other => other.to_string(),
    };
    if MODES.contains(&key.as_str()) {
        Ok(key)
    } else {
        Err(format!("unknown mode '{}' (try: {})", s, MODES.join(", ")))
    }
}

/// Generate a ramp from a base color.
/// - `mode` is one of `scale|tints|shades|tones` (aliases accepted).
/// - `count` controls the number of steps for the series modes
///   (`tints`/`shades`/`tones`), clamped to 2..=12; it is ignored by `scale`
///   (which always has the 10 Tailwind weights).
pub fn generate(base: &str, mode: &str, count: usize) -> Result<Ramp, String> {
    let (r, g, b) = parse(base)?;
    let mode = canonical_mode(mode)?;
    let (h, s, l) = rgb_to_hsl(r, g, b);
    let n = count.clamp(2, 12);
    let base_hex = format!("#{r:02x}{g:02x}{b:02x}");

    let mut steps: Vec<Step> = Vec::new();
    match mode.as_str() {
        "scale" => {
            // Place the base at the weight whose target lightness is closest.
            let base_idx = SCALE_WEIGHTS
                .iter()
                .enumerate()
                .min_by(|(_, (_, la)), (_, (_, lb))| {
                    (la - l).abs().partial_cmp(&(lb - l).abs()).unwrap()
                })
                .map(|(i, _)| i)
                .unwrap_or(5);
            for (i, (name, target_l)) in SCALE_WEIGHTS.iter().enumerate() {
                if i == base_idx {
                    // Keep the base color exactly at its slot.
                    steps.push(step_from_hsl((*name).to_string(), h, s, l, true));
                } else {
                    steps.push(step_from_hsl((*name).to_string(), h, s, *target_l, false));
                }
            }
        }
        "tints" => {
            // Lighten toward white. Step 1 = base, last = nearly white.
            for i in 0..n {
                let t = i as f64 / (n as f64 - 1.0);
                let li = l + (1.0 - l) * t;
                steps.push(step_from_hsl((i + 1).to_string(), h, s, li, false));
            }
        }
        "shades" => {
            // Darken toward black. Step 1 = base, last = nearly black.
            for i in 0..n {
                let t = i as f64 / (n as f64 - 1.0);
                let li = l * (1.0 - t);
                steps.push(step_from_hsl((i + 1).to_string(), h, s, li, false));
            }
        }
        "tones" => {
            // Desaturate toward gray (saturation -> 0), keeping lightness.
            for i in 0..n {
                let t = i as f64 / (n as f64 - 1.0);
                let si = s * (1.0 - t);
                steps.push(step_from_hsl((i + 1).to_string(), h, si, l, false));
            }
        }
        _ => unreachable!(),
    }

    Ok(Ramp {
        mode,
        base: base_hex,
        steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_has_eleven_named_weights() {
        let r = generate("#3498db", "scale", 0).unwrap();
        assert_eq!(r.mode, "scale");
        assert_eq!(r.base, "#3498db");
        assert_eq!(r.steps.len(), 11);
        let names: Vec<&str> = r.steps.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            ["50", "100", "200", "300", "400", "500", "600", "700", "800", "900", "950"]
        );
    }

    #[test]
    fn scale_marks_exactly_one_base_step_with_base_color() {
        let r = generate("#3498db", "scale", 0).unwrap();
        let base_steps: Vec<&Step> = r.steps.iter().filter(|s| s.is_base).collect();
        assert_eq!(base_steps.len(), 1, "exactly one base step");
        assert_eq!(base_steps[0].hex.to_lowercase(), "#3498db");
    }

    #[test]
    fn scale_goes_light_to_dark() {
        let r = generate("#3498db", "scale", 0).unwrap();
        let lum = |s: &Step| {
            let hex = s.hex.trim_start_matches('#');
            let v = u32::from_str_radix(hex, 16).unwrap();
            ((v >> 16) & 0xff) + ((v >> 8) & 0xff) + (v & 0xff)
        };
        assert!(lum(&r.steps[0]) > lum(&r.steps[10]), "50 lighter than 950");
    }

    #[test]
    fn tints_lighten_shades_darken() {
        let t = generate("#3498db", "tints", 5).unwrap();
        assert_eq!(t.steps.len(), 5);
        assert_eq!(t.steps[0].hex.to_lowercase(), "#3498db");
        assert_eq!(t.steps[4].hex.to_lowercase(), "#ffffff");

        let s = generate("#3498db", "shades", 5).unwrap();
        assert_eq!(s.steps[0].hex.to_lowercase(), "#3498db");
        assert_eq!(s.steps[4].hex.to_lowercase(), "#000000");
    }

    #[test]
    fn tones_desaturate_to_gray() {
        let r = generate("hsl(204, 70%, 50%)", "tones", 4).unwrap();
        assert_eq!(r.steps.len(), 4);
        assert!(r.steps[0].hsl.contains("70%"), "got {}", r.steps[0].hsl);
        assert!(r.steps[3].hsl.contains("0%,"), "got {}", r.steps[3].hsl);
    }

    #[test]
    fn count_clamped() {
        assert_eq!(generate("#3498db", "tints", 99).unwrap().steps.len(), 12);
        assert_eq!(generate("#3498db", "shades", 1).unwrap().steps.len(), 2);
    }

    #[test]
    fn mode_aliases() {
        assert_eq!(generate("#fff", "tailwind", 0).unwrap().mode, "scale");
        assert_eq!(generate("#fff", "tint", 3).unwrap().mode, "tints");
        assert_eq!(generate("#fff", "shade", 3).unwrap().mode, "shades");
    }

    #[test]
    fn accepts_rgb_and_bare_hex() {
        assert_eq!(generate("rgb(255,0,0)", "scale", 0).unwrap().base, "#ff0000");
        assert_eq!(generate("00ff00", "scale", 0).unwrap().base, "#00ff00");
    }

    #[test]
    fn errors() {
        assert!(generate("", "scale", 0).is_err());
        assert!(generate("notacolor", "scale", 0).is_err());
        assert!(generate("#ff0000", "bogus-mode", 0).is_err());
        assert!(generate("#12345", "scale", 0).is_err());
    }
}
