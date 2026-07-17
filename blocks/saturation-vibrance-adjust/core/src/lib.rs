//! gizza-ai/saturation-vibrance-adjust core — SELECTIVE saturation. Unlike a flat
//! saturation multiply (see `image-hsl-adjust`), `vibrance` boosts muted pixels
//! hard while leaving already-vivid pixels almost untouched, and (optionally)
//! protects skin-tone hues so faces don't turn orange. A separate linear
//! `saturation` term is the classic global scale. Pure-Rust (`image` for I/O,
//! hand-rolled HSL math). Returns PNG bytes (alpha preserved).

use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};

/// RGB (0..=255) → HSL (h in 0..360, s/l in 0..1).
fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d.abs() < f32::EPSILON {
        return (0.0, 0.0, l); // achromatic
    }
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        ((g - b) / d) % 6.0
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } * 60.0;
    let h = if h < 0.0 { h + 360.0 } else { h };
    (h, s, l)
}

/// HSL → RGB (0..=255).
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    if s <= 0.0 {
        let v = (l * 255.0).round().clamp(0.0, 255.0) as u8;
        return (v, v, v);
    }
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let conv = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (conv(r1), conv(g1), conv(b1))
}

/// How much of the `vibrance` push a pixel keeps (1.0 = full, lower = protected)
/// when skin protection is on. Skin tones cluster around hue ~25° (orange) at
/// mid lightness; there we damp the boost to at most 30% so faces stay natural.
/// Away from skin hues, or at very dark/bright lightness, the factor is 1.0.
fn skin_weight(h: f32, l: f32) -> f32 {
    let hue_close = (1.0 - (h - 25.0).abs() / 25.0).clamp(0.0, 1.0); // 1 at 25°, 0 by 0°/50°
    let light_ok = if l > 0.2 && l < 0.9 { 1.0 } else { 0.0 };
    let protect = hue_close * light_ok; // 0..1
    1.0 - 0.7 * protect
}

/// Selective saturation adjust. `vibrance` in -1..1 nonlinearly pushes low-saturation
/// pixels (positive = boost muted colours, negative = mute); `saturation` in -1..1 is
/// a flat linear scale applied to every pixel (0 = unchanged, -1 = grayscale, 1 = 2×).
/// `protect_skin` damps the vibrance push on skin-tone hues. Returns PNG bytes.
pub fn adjust(bytes: &[u8], vibrance: f32, saturation: f32, protect_skin: bool) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }
    let vib = if vibrance.is_finite() { vibrance.clamp(-1.0, 1.0) } else { 0.0 };
    let sat = if saturation.is_finite() { saturation.clamp(-1.0, 1.0) } else { 0.0 };

    let mut buf: RgbaImage = img.to_rgba8();
    for p in buf.pixels_mut() {
        let [r, g, b, a] = p.0;
        let (hh, mut s, l) = rgb_to_hsl(r, g, b);

        // Vibrance: weight the push by how UN-saturated the pixel already is, so
        // muted colours move far and vivid ones barely budge. Skin optionally spared.
        if vib != 0.0 {
            let skin = if protect_skin { skin_weight(hh, l) } else { 1.0 };
            let delta = if vib > 0.0 {
                vib * (1.0 - s) // headroom-scaled boost: near-vivid pixels get little
            } else {
                vib * s // negative: mute proportional to current saturation
            };
            s = (s + delta * skin).clamp(0.0, 1.0);
        }

        // Global linear saturation scale (classic, uniform).
        if sat != 0.0 {
            s = (s * (1.0 + sat)).clamp(0.0, 1.0);
        }

        let (nr, ng, nb) = hsl_to_rgb(hh, s, l);
        *p = Rgba([nr, ng, nb, a]);
    }

    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(buf)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(c: Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_pixel(4, 4, c);
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }
    fn px0(png: &[u8]) -> Rgba<u8> {
        image::load_from_memory(png).unwrap().get_pixel(0, 0)
    }
    fn sat_of(px: Rgba<u8>) -> f32 {
        rgb_to_hsl(px.0[0], px.0[1], px.0[2]).1
    }

    #[test]
    fn hsl_roundtrip() {
        for c in [(255, 0, 0), (0, 128, 64), (12, 200, 240), (255, 255, 255), (0, 0, 0)] {
            let (h, s, l) = rgb_to_hsl(c.0, c.1, c.2);
            let back = hsl_to_rgb(h, s, l);
            assert!((back.0 as i32 - c.0 as i32).abs() <= 1);
            assert!((back.1 as i32 - c.1 as i32).abs() <= 1);
            assert!((back.2 as i32 - c.2 as i32).abs() <= 1);
        }
    }

    #[test]
    fn identity_keeps_color() {
        let out = adjust(&solid(Rgba([123, 45, 67, 255])), 0.0, 0.0, true).unwrap();
        let p = px0(&out);
        assert!((p.0[0] as i32 - 123).abs() <= 1);
        assert!((p.0[1] as i32 - 45).abs() <= 1);
        assert!((p.0[2] as i32 - 67).abs() <= 1);
    }

    #[test]
    fn vibrance_boosts_muted_more_than_vivid() {
        // A muted blue (low saturation) and a vivid blue (high saturation), same hue.
        let muted = Rgba([120, 130, 160, 255]); // low sat
        let vivid = Rgba([10, 10, 240, 255]); // high sat
        let ms0 = sat_of(px0(&solid(muted)));
        let vs0 = sat_of(px0(&solid(vivid)));
        let ms1 = sat_of(px0(&adjust(&solid(muted), 0.6, 0.0, false).unwrap()));
        let vs1 = sat_of(px0(&adjust(&solid(vivid), 0.6, 0.0, false).unwrap()));
        let muted_gain = ms1 - ms0;
        let vivid_gain = vs1 - vs0;
        assert!(muted_gain > 0.05, "muted should climb, got {muted_gain}");
        assert!(muted_gain > vivid_gain, "muted {muted_gain} should out-gain vivid {vivid_gain}");
    }

    #[test]
    fn skin_protection_dampens_boost() {
        // Skin-ish tone (orange hue ~25°, mid lightness).
        let skin = Rgba([210, 150, 120, 255]);
        let base = sat_of(px0(&solid(skin)));
        let protected = sat_of(px0(&adjust(&solid(skin), 0.8, 0.0, true).unwrap())) - base;
        let unprotected = sat_of(px0(&adjust(&solid(skin), 0.8, 0.0, false).unwrap())) - base;
        assert!(protected < unprotected, "protect_skin should reduce boost: {protected} vs {unprotected}");
    }

    #[test]
    fn negative_saturation_grays_out() {
        let out = adjust(&solid(Rgba([200, 50, 50, 255])), 0.0, -1.0, true).unwrap();
        let p = px0(&out);
        assert_eq!(p.0[0], p.0[1]);
        assert_eq!(p.0[1], p.0[2]); // saturation=-1 → gray
    }

    #[test]
    fn alpha_preserved() {
        let out = adjust(&solid(Rgba([100, 150, 200, 128])), 0.5, 0.2, true).unwrap();
        assert_eq!(px0(&out).0[3], 128);
    }

    #[test]
    fn errors() {
        assert!(adjust(b"not an image", 0.5, 0.0, true).is_err());
    }
}
