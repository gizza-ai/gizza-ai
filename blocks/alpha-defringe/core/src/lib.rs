//! alpha-defringe core — pure compute, shared by the chat skill block.
//! Removes the dark/light/color halo (fringe) left on the anti-aliased edge of
//! a cutout against transparency. Pure-Rust (`image` for I/O); always returns
//! PNG bytes so the alpha channel is preserved.
//!
//! Two modes:
//!   * `bleed`   — repaint each translucent edge pixel (alpha < threshold) with
//!                 the color of the nearest clean pixels (alpha >= threshold)
//!                 within `radius`. Color-agnostic: removes a halo of any color
//!                 without naming the old background.
//!   * `unmatte` — algebraically remove a known flat matte color `M` from each
//!                 translucent pixel: `F = (C - (1-a)*M) / a`. Recovers the true
//!                 foreground for a cutout anti-aliased over a solid background.
//! In both modes the alpha channel is left untouched and fully-opaque pixels
//! (alpha >= threshold) are unchanged.

use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};

/// Which defringe algorithm to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Bleed nearby clean color outward over the translucent rim (any halo color).
    Bleed,
    /// Algebraically remove a flat `matte_color` from translucent pixels.
    Unmatte,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "bleed" | "decontaminate" | "spread" => Ok(Mode::Bleed),
            "unmatte" | "matte" | "remove-matte" | "remove_matte" => Ok(Mode::Unmatte),
            other => Err(format!("unknown mode '{other}' (use 'bleed' or 'unmatte')")),
        }
    }
}

/// Parse a matte color: `#rgb`, `#rrggbb`, or a name
/// (black/white/gray/grey/red/green/blue). Returns the RGB triple.
pub fn parse_color(s: &str) -> Result<[u8; 3], String> {
    let t = s.trim();
    match t.to_ascii_lowercase().as_str() {
        "black" => return Ok([0, 0, 0]),
        "white" => return Ok([255, 255, 255]),
        "gray" | "grey" => return Ok([128, 128, 128]),
        "red" => return Ok([255, 0, 0]),
        "green" => return Ok([0, 255, 0]),
        "blue" => return Ok([0, 0, 255]),
        _ => {}
    }
    let hex = t.strip_prefix('#').ok_or_else(|| {
        format!("invalid color '{t}' (use #rgb, #rrggbb, or black/white/gray/red/green/blue)")
    })?;
    let parse2 = |a: &str| u8::from_str_radix(a, 16).map_err(|_| format!("invalid hex color '{t}'"));
    match hex.len() {
        3 => {
            // #rgb → each nibble doubled (f → ff)
            let mut out = [0u8; 3];
            for (i, c) in hex.chars().enumerate() {
                let v = c.to_digit(16).ok_or_else(|| format!("invalid hex color '{t}'"))? as u8;
                out[i] = v << 4 | v;
            }
            Ok(out)
        }
        6 => Ok([
            parse2(&hex[0..2])?,
            parse2(&hex[2..4])?,
            parse2(&hex[4..6])?,
        ]),
        _ => Err(format!("invalid hex color '{t}' (expected #rgb or #rrggbb)")),
    }
}

/// Decode `bytes`, run the selected defringe, and re-encode as PNG (RGBA).
///
/// * `radius`    — bleed search radius in pixels (clamped to 1..=16).
/// * `threshold` — alpha at/above which a pixel is a clean color source
///   (clamped to 1..=255); pixels below it are the translucent edge to repair.
/// * `matte`     — flat background color removed in `Mode::Unmatte`.
pub fn defringe(
    bytes: &[u8],
    mode: Mode,
    radius: u32,
    threshold: u8,
    matte: [u8; 3],
) -> Result<Vec<u8>, String> {
    let radius = radius.clamp(1, 16);
    let threshold = threshold.max(1);

    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero dimension".into());
    }

    let src: RgbaImage = img.to_rgba8();
    let out = match mode {
        Mode::Bleed => bleed(&src, radius, threshold),
        Mode::Unmatte => unmatte(&src, threshold, matte),
    };

    let mut buf = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(out)
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(buf.into_inner())
}

/// Repaint every translucent pixel (alpha < threshold) with the average color of
/// the nearest clean pixels (alpha >= threshold) within `radius`. Reads only from
/// `src` so bleed does not propagate through freshly-repainted pixels. Alpha and
/// clean pixels are left untouched; a candidate with no clean neighbor is unchanged.
fn bleed(src: &RgbaImage, radius: u32, threshold: u8) -> RgbaImage {
    let (w, h) = (src.width(), src.height());
    let r = radius as i64;
    let mut out = src.clone();

    for y in 0..h as i64 {
        for x in 0..w as i64 {
            let px = src.get_pixel(x as u32, y as u32).0;
            if px[3] >= threshold {
                continue; // clean pixel — leave it
            }
            // Find the clean neighbor(s) at the smallest (squared) distance and
            // average their RGB.
            let mut best_d2 = i64::MAX;
            let mut sum = [0u64; 3];
            let mut count = 0u64;
            for dy in -r..=r {
                let ny = y + dy;
                if ny < 0 || ny >= h as i64 {
                    continue;
                }
                for dx in -r..=r {
                    let nx = x + dx;
                    if nx < 0 || nx >= w as i64 {
                        continue;
                    }
                    let d2 = dx * dx + dy * dy;
                    if d2 == 0 || d2 > r * r {
                        continue; // self, or outside the circular radius
                    }
                    let n = src.get_pixel(nx as u32, ny as u32).0;
                    if n[3] < threshold {
                        continue; // not a clean source
                    }
                    if d2 < best_d2 {
                        best_d2 = d2;
                        sum = [n[0] as u64, n[1] as u64, n[2] as u64];
                        count = 1;
                    } else if d2 == best_d2 {
                        sum[0] += n[0] as u64;
                        sum[1] += n[1] as u64;
                        sum[2] += n[2] as u64;
                        count += 1;
                    }
                }
            }
            if count > 0 {
                let avg = |c: u64| (c / count) as u8;
                // Preserve the original alpha.
                out.put_pixel(
                    x as u32,
                    y as u32,
                    Rgba([avg(sum[0]), avg(sum[1]), avg(sum[2]), px[3]]),
                );
            }
        }
    }
    out
}

/// Remove a flat matte color `M` from every translucent pixel via
/// `F = (C - (1-a)*M) / a`, per channel. Alpha is preserved. Fully-opaque pixels
/// (alpha >= threshold) and fully-transparent pixels (alpha == 0) are unchanged.
fn unmatte(src: &RgbaImage, threshold: u8, matte: [u8; 3]) -> RgbaImage {
    let mut out = src.clone();
    let m = [
        matte[0] as f32 / 255.0,
        matte[1] as f32 / 255.0,
        matte[2] as f32 / 255.0,
    ];
    for (x, y, px) in src.enumerate_pixels() {
        let [r, g, b, a] = px.0;
        if a >= threshold || a == 0 {
            continue; // opaque source or nothing to recover
        }
        let af = a as f32 / 255.0;
        let recover = |c: u8, mc: f32| -> u8 {
            let cf = c as f32 / 255.0;
            let f = (cf - (1.0 - af) * mc) / af;
            (f.clamp(0.0, 1.0) * 255.0).round() as u8
        };
        out.put_pixel(
            x,
            y,
            Rgba([recover(r, m[0]), recover(g, m[1]), recover(b, m[2]), a]),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(img: &RgbaImage) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img.clone())
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }
    fn pixel(png: &[u8], x: u32, y: u32) -> [u8; 4] {
        image::load_from_memory(png).unwrap().get_pixel(x, y).0
    }

    #[test]
    fn mode_parse() {
        assert_eq!(Mode::parse("bleed").unwrap(), Mode::Bleed);
        assert_eq!(Mode::parse("  BLEED ").unwrap(), Mode::Bleed);
        assert_eq!(Mode::parse("unmatte").unwrap(), Mode::Unmatte);
        assert_eq!(Mode::parse("remove-matte").unwrap(), Mode::Unmatte);
        assert!(Mode::parse("nope").is_err());
    }

    #[test]
    fn color_parse() {
        assert_eq!(parse_color("black").unwrap(), [0, 0, 0]);
        assert_eq!(parse_color("WHITE").unwrap(), [255, 255, 255]);
        assert_eq!(parse_color("grey").unwrap(), [128, 128, 128]);
        assert_eq!(parse_color("green").unwrap(), [0, 255, 0]);
        assert_eq!(parse_color("#fff").unwrap(), [255, 255, 255]);
        assert_eq!(parse_color("#f00").unwrap(), [255, 0, 0]);
        assert_eq!(parse_color("#00ff00").unwrap(), [0, 255, 0]);
        assert_eq!(parse_color("#123456").unwrap(), [0x12, 0x34, 0x56]);
        assert!(parse_color("123456").is_err()); // no '#'
        assert!(parse_color("#12").is_err()); // wrong length
        assert!(parse_color("#gggggg").is_err()); // bad hex
    }

    #[test]
    fn bleed_fixes_colored_fringe_and_preserves_alpha() {
        // 3x3: all clean red foreground except the top-left corner, which is a
        // green translucent fringe pixel (alpha 100).
        let mut img = RgbaImage::from_pixel(3, 3, Rgba([255, 0, 0, 255]));
        img.put_pixel(0, 0, Rgba([0, 255, 0, 100]));
        let out = defringe(&encode(&img), Mode::Bleed, 2, 250, [0, 0, 0]).unwrap();
        let p = pixel(&out, 0, 0);
        // Green rim repainted red from the neighboring foreground...
        assert_eq!([p[0], p[1], p[2]], [255, 0, 0]);
        // ...and the original alpha is preserved.
        assert_eq!(p[3], 100);
        // A clean pixel is untouched.
        assert_eq!(pixel(&out, 2, 2), [255, 0, 0, 255]);
    }

    #[test]
    fn bleed_leaves_isolated_fringe_unchanged() {
        // A lone translucent pixel with no clean neighbor stays as-is.
        let img = RgbaImage::from_pixel(3, 3, Rgba([10, 20, 30, 40]));
        let out = defringe(&encode(&img), Mode::Bleed, 2, 250, [0, 0, 0]).unwrap();
        assert_eq!(pixel(&out, 1, 1), [10, 20, 30, 40]);
    }

    #[test]
    fn unmatte_black_recovers_red() {
        // Edge pixel anti-aliased over black: stored (128,0,0,128) → true red.
        let mut img = RgbaImage::from_pixel(2, 2, Rgba([255, 0, 0, 255]));
        img.put_pixel(0, 0, Rgba([128, 0, 0, 128]));
        let out = defringe(&encode(&img), Mode::Unmatte, 2, 250, [0, 0, 0]).unwrap();
        let p = pixel(&out, 0, 0);
        assert_eq!([p[0], p[1], p[2]], [255, 0, 0]);
        assert_eq!(p[3], 128); // alpha preserved
        // Fully-opaque pixel untouched.
        assert_eq!(pixel(&out, 1, 1), [255, 0, 0, 255]);
    }

    #[test]
    fn unmatte_white_recovers_foreground() {
        // Over white matte: F = (C - (1-a)*1)/a. Store (191,128,128,128).
        // a=128/255≈0.502, r: (0.749-0.498)/0.502≈0.5 → ~128.
        let mut img = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255]));
        img.put_pixel(0, 0, Rgba([191, 128, 128, 128]));
        let out = defringe(&encode(&img), Mode::Unmatte, 2, 255, [255, 255, 255]).unwrap();
        let p = pixel(&out, 0, 0);
        // red channel recovers toward mid-gray, others toward 0.
        assert!((120..=136).contains(&p[0]), "r was {}", p[0]);
        assert_eq!(p[3], 128);
    }

    #[test]
    fn unmatte_transparent_pixel_unchanged() {
        let img = RgbaImage::from_pixel(2, 2, Rgba([50, 60, 70, 0]));
        let out = defringe(&encode(&img), Mode::Unmatte, 2, 250, [0, 0, 0]).unwrap();
        // alpha 0 → left as stored (no divide-by-zero, RGB kept).
        assert_eq!(pixel(&out, 0, 0), [50, 60, 70, 0]);
    }

    #[test]
    fn invalid_image_errors() {
        assert!(defringe(b"not an image", Mode::Bleed, 2, 250, [0, 0, 0]).is_err());
    }

    #[test]
    fn radius_and_threshold_clamp() {
        // radius 0 and threshold 0 must not panic (clamped to valid ranges).
        let img = RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255]));
        assert!(defringe(&encode(&img), Mode::Bleed, 0, 0, [0, 0, 0]).is_ok());
        assert!(defringe(&encode(&img), Mode::Bleed, 999, 255, [0, 0, 0]).is_ok());
    }
}
