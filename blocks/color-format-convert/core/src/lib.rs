//! gizza-ai/color-format-convert core — parse a color in any common notation and
//! convert it between HEX, RGB(A), HSL, HSV and CMYK. Pure-Rust, dependency-free
//! (besides serde for the output struct).

use serde::Serialize;

/// All representations of a parsed color.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Color {
    pub hex: String,
    pub rgb: String,
    pub rgba: String,
    pub hsl: String,
    pub hsv: String,
    pub cmyk: String,
    /// Alpha 0.0–1.0.
    pub alpha: f64,
}

fn round(v: f64) -> i64 {
    v.round() as i64
}

/// Parse a color string. Accepts `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb(...)`,
/// `rgba(...)`, `hsl(...)`, `hsla(...)`. Returns (r, g, b, a) with r/g/b 0–255 and
/// a 0.0–1.0.
fn parse(input: &str) -> Result<(u8, u8, u8, f64), String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("no color given".into());
    }
    let lower = s.to_ascii_lowercase();

    if let Some(hex) = lower.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(rest) = lower.strip_prefix("rgba").and_then(|r| paren(r)) {
        return parse_rgb(&rest, true);
    }
    if let Some(rest) = lower.strip_prefix("rgb").and_then(|r| paren(r)) {
        return parse_rgb(&rest, false);
    }
    if let Some(rest) = lower.strip_prefix("hsla").and_then(|r| paren(r)) {
        return parse_hsl(&rest);
    }
    if let Some(rest) = lower.strip_prefix("hsl").and_then(|r| paren(r)) {
        return parse_hsl(&rest);
    }
    // Bare hex without '#'.
    if lower.chars().all(|c| c.is_ascii_hexdigit()) {
        return parse_hex(&lower);
    }
    Err("unrecognized color format (use #hex, rgb(), rgba(), hsl(), or hsla())".into())
}

fn paren(s: &str) -> Option<String> {
    let s = s.trim();
    let inner = s.strip_prefix('(')?.strip_suffix(')')?;
    Some(inner.to_string())
}

fn parse_hex(hex: &str) -> Result<(u8, u8, u8, f64), String> {
    let h = hex.trim();
    let expand = |c: char| -> String { format!("{c}{c}") };
    let (r, g, b, a) = match h.len() {
        3 => {
            let cs: Vec<char> = h.chars().collect();
            (expand(cs[0]), expand(cs[1]), expand(cs[2]), "ff".to_string())
        }
        4 => {
            let cs: Vec<char> = h.chars().collect();
            (expand(cs[0]), expand(cs[1]), expand(cs[2]), expand(cs[3]))
        }
        6 => (h[0..2].into(), h[2..4].into(), h[4..6].into(), "ff".to_string()),
        8 => (h[0..2].into(), h[2..4].into(), h[4..6].into(), h[6..8].into()),
        _ => return Err("hex color must be 3, 4, 6, or 8 digits".into()),
    };
    let p = |x: &str| u8::from_str_radix(x, 16).map_err(|_| "invalid hex digit".to_string());
    let a = p(&a)? as f64 / 255.0;
    Ok((p(&r)?, p(&g)?, p(&b)?, a))
}

fn parse_rgb(inner: &str, _has_a: bool) -> Result<(u8, u8, u8, f64), String> {
    let parts: Vec<&str> = inner.split(|c| c == ',' || c == '/').map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
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
    let r = comp(parts[0])?;
    let g = comp(parts[1])?;
    let b = comp(parts[2])?;
    let a = if parts.len() >= 4 { parse_alpha(parts[3])? } else { 1.0 };
    Ok((r, g, b, a))
}

fn parse_alpha(x: &str) -> Result<f64, String> {
    if let Some(pct) = x.strip_suffix('%') {
        Ok((pct.trim().parse::<f64>().map_err(|_| "bad alpha".to_string())? / 100.0).clamp(0.0, 1.0))
    } else {
        Ok(x.parse::<f64>().map_err(|_| "bad alpha".to_string())?.clamp(0.0, 1.0))
    }
}

fn parse_hsl(inner: &str) -> Result<(u8, u8, u8, f64), String> {
    let parts: Vec<&str> = inner.split(|c| c == ',' || c == '/').map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
    if parts.len() < 3 {
        return Err("hsl needs at least 3 components".into());
    }
    let h: f64 = parts[0].trim_end_matches("deg").trim().parse().map_err(|_| "bad hue".to_string())?;
    let s: f64 = parts[1].trim_end_matches('%').trim().parse::<f64>().map_err(|_| "bad saturation".to_string())? / 100.0;
    let l: f64 = parts[2].trim_end_matches('%').trim().parse::<f64>().map_err(|_| "bad lightness".to_string())? / 100.0;
    let a = if parts.len() >= 4 { parse_alpha(parts[3])? } else { 1.0 };
    let (r, g, b) = hsl_to_rgb(h.rem_euclid(360.0), s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
    Ok((r, g, b, a))
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

fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let (rf, gf, bf) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let d = max - min;
    let v = max;
    let s = if max == 0.0 { 0.0 } else { d / max };
    let h = if d == 0.0 {
        0.0
    } else if max == rf {
        60.0 * (((gf - bf) / d).rem_euclid(6.0))
    } else if max == gf {
        60.0 * ((bf - rf) / d + 2.0)
    } else {
        60.0 * ((rf - gf) / d + 4.0)
    };
    (h.rem_euclid(360.0), s, v)
}

fn rgb_to_cmyk(r: u8, g: u8, b: u8) -> (f64, f64, f64, f64) {
    let (rf, gf, bf) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let k = 1.0 - rf.max(gf).max(bf);
    if (1.0 - k).abs() < 1e-9 {
        return (0.0, 0.0, 0.0, 1.0);
    }
    let c = (1.0 - rf - k) / (1.0 - k);
    let m = (1.0 - gf - k) / (1.0 - k);
    let y = (1.0 - bf - k) / (1.0 - k);
    (c, m, y, k)
}

/// Parse a color and return all representations.
pub fn convert(input: &str) -> Result<Color, String> {
    let (r, g, b, a) = parse(input)?;
    let (h, s, l) = rgb_to_hsl(r, g, b);
    let (hv, sv, vv) = rgb_to_hsv(r, g, b);
    let (c, m, y, k) = rgb_to_cmyk(r, g, b);
    let pct = |x: f64| round(x * 100.0);

    Ok(Color {
        hex: format!("#{r:02x}{g:02x}{b:02x}"),
        rgb: format!("rgb({r}, {g}, {b})"),
        rgba: format!("rgba({r}, {g}, {b}, {})", trim_f(a)),
        hsl: format!("hsl({}, {}%, {}%)", round(h), pct(s), pct(l)),
        hsv: format!("hsv({}, {}%, {}%)", round(hv), pct(sv), pct(vv)),
        cmyk: format!("cmyk({}%, {}%, {}%, {}%)", pct(c), pct(m), pct(y), pct(k)),
        alpha: a,
    })
}

fn trim_f(a: f64) -> String {
    let s = format!("{a:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn red_hex() {
        let c = convert("#ff0000").unwrap();
        assert_eq!(c.rgb, "rgb(255, 0, 0)");
        assert_eq!(c.hsl, "hsl(0, 100%, 50%)");
        assert_eq!(c.hsv, "hsv(0, 100%, 100%)");
        assert_eq!(c.cmyk, "cmyk(0%, 100%, 100%, 0%)");
        assert_eq!(c.hex, "#ff0000");
        assert_eq!(c.alpha, 1.0);
    }

    #[test]
    fn short_hex_and_alpha() {
        let c = convert("#0f0").unwrap();
        assert_eq!(c.hex, "#00ff00");
        assert_eq!(c.rgb, "rgb(0, 255, 0)");
        let d = convert("#ff000080").unwrap();
        assert!((d.alpha - 0.502).abs() < 0.01);
    }

    #[test]
    fn rgb_and_rgba_input() {
        let c = convert("rgb(0, 0, 255)").unwrap();
        assert_eq!(c.hex, "#0000ff");
        assert_eq!(c.hsl, "hsl(240, 100%, 50%)");
        let d = convert("rgba(255,255,255,0.5)").unwrap();
        assert_eq!(d.hex, "#ffffff");
        assert!((d.alpha - 0.5).abs() < 1e-9);
    }

    #[test]
    fn hsl_input_roundtrips() {
        let c = convert("hsl(120, 100%, 50%)").unwrap();
        assert_eq!(c.hex, "#00ff00");
    }

    #[test]
    fn gray_cmyk() {
        let c = convert("#808080").unwrap();
        // mid gray -> k ~50%, no chroma
        assert_eq!(c.cmyk, "cmyk(0%, 0%, 0%, 50%)");
    }

    #[test]
    fn errors() {
        assert!(convert("").is_err());
        assert!(convert("#12345").is_err()); // bad length
        assert!(convert("notacolor").is_err());
        assert!(convert("rgb(1,2)").is_err());
    }
}
