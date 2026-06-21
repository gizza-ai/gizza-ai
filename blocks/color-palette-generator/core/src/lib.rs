//! color-palette-generator core — generate a harmonious color palette from a base
//! color using classic color-theory schemes (complementary, analogous, triadic,
//! split-complementary, tetradic, square, monochromatic, shades, tints). Pure-Rust,
//! dependency-free besides serde for the output struct.

use serde::Serialize;

/// One color in the palette, in every common notation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Swatch {
    pub hex: String,
    pub rgb: String,
    pub hsl: String,
}

/// The generated palette.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Palette {
    /// The scheme that was applied.
    pub scheme: String,
    /// The base color (echoed back, normalized to #rrggbb).
    pub base: String,
    /// The palette swatches, base color first.
    pub colors: Vec<Swatch>,
}

fn round(v: f64) -> i64 {
    v.round() as i64
}

/// Parse a color string into (r, g, b), each 0–255. Accepts `#rgb`, `#rrggbb`,
/// bare hex, `rgb(...)`, and `hsl(...)`.
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
        3 => {
            let cs: Vec<char> = h.chars().collect();
            (expand(cs[0]), expand(cs[1]), expand(cs[2]))
        }
        4 => {
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

fn swatch_from_hsl(h: f64, s: f64, l: f64) -> Swatch {
    let h = h.rem_euclid(360.0);
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let (r, g, b) = hsl_to_rgb(h, s, l);
    Swatch {
        hex: format!("#{r:02x}{g:02x}{b:02x}"),
        rgb: format!("rgb({r}, {g}, {b})"),
        hsl: format!("hsl({}, {}%, {}%)", round(h), round(s * 100.0), round(l * 100.0)),
    }
}

/// All supported schemes.
pub const SCHEMES: &[&str] = &[
    "complementary",
    "analogous",
    "triadic",
    "split-complementary",
    "tetradic",
    "square",
    "monochromatic",
    "shades",
    "tints",
];

fn canonical_scheme(s: &str) -> Result<String, String> {
    let key = s.trim().to_ascii_lowercase().replace(['_', ' '], "-");
    let key = match key.as_str() {
        "split" | "splitcomplementary" | "split-complement" => "split-complementary".to_string(),
        "mono" | "monochrome" => "monochromatic".to_string(),
        "rectangle" => "tetradic".to_string(),
        "complement" | "comp" => "complementary".to_string(),
        other => other.to_string(),
    };
    if SCHEMES.contains(&key.as_str()) {
        Ok(key)
    } else {
        Err(format!("unknown scheme '{}' (try: {})", s, SCHEMES.join(", ")))
    }
}

/// Generate a palette from a base color and scheme. `count` only affects the
/// analogous, monochromatic, shades and tints schemes (which produce a series);
/// the fixed harmony schemes ignore it. `count` is clamped to 2..=12.
pub fn generate(base: &str, scheme: &str, count: usize) -> Result<Palette, String> {
    let (r, g, b) = parse(base)?;
    let scheme = canonical_scheme(scheme)?;
    let (h, s, l) = rgb_to_hsl(r, g, b);
    let n = count.clamp(2, 12);

    let mut colors: Vec<Swatch> = Vec::new();
    match scheme.as_str() {
        "complementary" => {
            colors.push(swatch_from_hsl(h, s, l));
            colors.push(swatch_from_hsl(h + 180.0, s, l));
        }
        "triadic" => {
            for off in [0.0, 120.0, 240.0] {
                colors.push(swatch_from_hsl(h + off, s, l));
            }
        }
        "split-complementary" => {
            for off in [0.0, 150.0, 210.0] {
                colors.push(swatch_from_hsl(h + off, s, l));
            }
        }
        "tetradic" => {
            for off in [0.0, 60.0, 180.0, 240.0] {
                colors.push(swatch_from_hsl(h + off, s, l));
            }
        }
        "square" => {
            for off in [0.0, 90.0, 180.0, 270.0] {
                colors.push(swatch_from_hsl(h + off, s, l));
            }
        }
        "analogous" => {
            // n colors centered on the base, 30deg apart.
            let step = 30.0;
            let start = -((n as f64 - 1.0) / 2.0) * step;
            for i in 0..n {
                colors.push(swatch_from_hsl(h + start + step * i as f64, s, l));
            }
        }
        "monochromatic" => {
            // Vary lightness across a visible range, keeping hue & saturation.
            for i in 0..n {
                let t = if n == 1 { 0.5 } else { i as f64 / (n as f64 - 1.0) };
                let li = 0.15 + t * 0.70;
                colors.push(swatch_from_hsl(h, s, li));
            }
        }
        "shades" => {
            // Toward black: lightness from base down to ~0.
            for i in 0..n {
                let t = i as f64 / n as f64;
                colors.push(swatch_from_hsl(h, s, l * (1.0 - t)));
            }
        }
        "tints" => {
            // Toward white: lightness from base up to ~1.
            for i in 0..n {
                let t = i as f64 / n as f64;
                colors.push(swatch_from_hsl(h, s, l + (1.0 - l) * t));
            }
        }
        _ => unreachable!(),
    }

    let base_hex = format!("#{r:02x}{g:02x}{b:02x}");

    Ok(Palette {
        scheme,
        base: base_hex,
        colors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complementary_of_red_is_cyan() {
        let p = generate("#ff0000", "complementary", 5).unwrap();
        assert_eq!(p.scheme, "complementary");
        assert_eq!(p.base, "#ff0000");
        assert_eq!(p.colors.len(), 2);
        assert_eq!(p.colors[0].hex, "#ff0000");
        assert_eq!(p.colors[1].hex, "#00ffff");
    }

    #[test]
    fn triadic_red_green_blue() {
        let p = generate("#ff0000", "triadic", 3).unwrap();
        assert_eq!(p.colors.len(), 3);
        assert_eq!(p.colors[0].hex, "#ff0000");
        assert_eq!(p.colors[1].hex, "#00ff00");
        assert_eq!(p.colors[2].hex, "#0000ff");
    }

    #[test]
    fn square_has_four() {
        let p = generate("#ff0000", "square", 5).unwrap();
        assert_eq!(p.colors.len(), 4);
        assert_eq!(p.colors[0].hex, "#ff0000");
        assert_eq!(p.colors[2].hex, "#00ffff");
    }

    #[test]
    fn analogous_count_respected() {
        let p = generate("#3498db", "analogous", 5).unwrap();
        assert_eq!(p.colors.len(), 5);
        assert_eq!(p.colors[2].hex.to_lowercase(), "#3498db");
    }

    #[test]
    fn monochromatic_keeps_hue() {
        let p = generate("hsl(204, 70%, 50%)", "monochromatic", 6).unwrap();
        assert_eq!(p.colors.len(), 6);
        for c in &p.colors {
            assert!(c.hsl.starts_with("hsl(204"), "got {}", c.hsl);
        }
    }

    #[test]
    fn shades_go_darker() {
        let p = generate("#3498db", "shades", 4).unwrap();
        assert_eq!(p.colors.len(), 4);
        assert_eq!(p.colors[0].hex.to_lowercase(), "#3498db");
        assert!(p.colors[3].hex != p.colors[0].hex);
    }

    #[test]
    fn tints_count_clamped() {
        let p = generate("#3498db", "tints", 99).unwrap();
        assert_eq!(p.colors.len(), 12);
    }

    #[test]
    fn scheme_aliases() {
        assert_eq!(generate("#fff", "mono", 3).unwrap().scheme, "monochromatic");
        assert_eq!(generate("#fff", "split", 3).unwrap().scheme, "split-complementary");
    }

    #[test]
    fn accepts_rgb_and_bare_hex() {
        assert_eq!(generate("rgb(255,0,0)", "complementary", 2).unwrap().base, "#ff0000");
        assert_eq!(generate("00ff00", "complementary", 2).unwrap().base, "#00ff00");
    }

    #[test]
    fn errors() {
        assert!(generate("", "complementary", 5).is_err());
        assert!(generate("notacolor", "complementary", 5).is_err());
        assert!(generate("#ff0000", "bogus-scheme", 5).is_err());
        assert!(generate("#12345", "complementary", 5).is_err());
    }
}
