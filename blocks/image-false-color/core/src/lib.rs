//! gizza-ai/image-false-color core — map an image's per-pixel luminance through a
//! scientific colormap to produce a false-colour heatmap. Pure-Rust (`image`).
//! Output is PNG. Alpha is preserved.
//!
//! Each pixel's luminance (Rec. 601 grayscale) in [0,1] is looked up in the chosen
//! colormap (piecewise-linear interpolation over a small set of control points) to
//! get the output RGB. Useful for thermal / grayscale / depth / scientific imagery.

use std::io::Cursor;

use image::ImageFormat;

/// A scientific colormap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Colormap {
    /// Perceptually-uniform blue→green→yellow (default).
    Viridis,
    /// Perceptually-uniform black→purple→yellow.
    Magma,
    /// Perceptually-uniform black→purple→orange→yellow.
    Inferno,
    /// Perceptually-uniform blue→purple→pink→yellow.
    Plasma,
    /// Perceptually-uniform, colour-blind-friendly blue→gray→yellow.
    Cividis,
    /// Improved rainbow (blue→cyan→green→yellow→red).
    Turbo,
    /// Classic rainbow (blue→cyan→green→yellow→red). Not perceptually uniform.
    Jet,
    /// Black→red→yellow→white thermal ramp.
    Hot,
    /// Identity grayscale ramp (black→white).
    Grayscale,
}

impl Colormap {
    pub fn parse(s: &str) -> Result<Colormap, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "viridis" | "" => Ok(Colormap::Viridis),
            "magma" => Ok(Colormap::Magma),
            "inferno" => Ok(Colormap::Inferno),
            "plasma" => Ok(Colormap::Plasma),
            "cividis" => Ok(Colormap::Cividis),
            "turbo" => Ok(Colormap::Turbo),
            "jet" | "rainbow" => Ok(Colormap::Jet),
            "hot" | "thermal" => Ok(Colormap::Hot),
            "grayscale" | "greyscale" | "gray" | "grey" => Ok(Colormap::Grayscale),
            other => Err(format!(
                "unknown colormap '{other}' (use viridis, magma, inferno, plasma, turbo, jet, hot, or grayscale)"
            )),
        }
    }

    /// Control points (RGB in [0,1]) sampled at equal intervals across [0,1].
    fn stops(self) -> &'static [[f32; 3]] {
        match self {
            // matplotlib viridis sampled at 0, .25, .5, .75, 1
            Colormap::Viridis => &[
                [0.267, 0.005, 0.329],
                [0.230, 0.322, 0.546],
                [0.128, 0.567, 0.551],
                [0.370, 0.789, 0.383],
                [0.993, 0.906, 0.144],
            ],
            Colormap::Magma => &[
                [0.001, 0.000, 0.014],
                [0.232, 0.060, 0.439],
                [0.550, 0.161, 0.506],
                [0.886, 0.310, 0.387],
                [0.987, 0.991, 0.749],
            ],
            Colormap::Inferno => &[
                [0.001, 0.000, 0.014],
                [0.258, 0.039, 0.406],
                [0.578, 0.148, 0.404],
                [0.865, 0.317, 0.226],
                [0.988, 0.998, 0.645],
            ],
            Colormap::Plasma => &[
                [0.050, 0.030, 0.528],
                [0.417, 0.000, 0.658],
                [0.692, 0.165, 0.564],
                [0.881, 0.392, 0.383],
                [0.940, 0.975, 0.131],
            ],
            // matplotlib cividis, sampled at 0,.25,.5,.75,1
            Colormap::Cividis => &[
                [0.000, 0.135, 0.305],
                [0.265, 0.302, 0.379],
                [0.473, 0.471, 0.435],
                [0.715, 0.671, 0.380],
                [1.000, 0.910, 0.157],
            ],
            // Google turbo, sampled coarsely
            Colormap::Turbo => &[
                [0.190, 0.072, 0.232],
                [0.275, 0.748, 0.911],
                [0.475, 0.978, 0.319],
                [0.929, 0.751, 0.149],
                [0.480, 0.016, 0.011],
            ],
            // classic jet
            Colormap::Jet => &[
                [0.000, 0.000, 0.500],
                [0.000, 0.500, 1.000],
                [0.500, 1.000, 0.500],
                [1.000, 0.500, 0.000],
                [0.500, 0.000, 0.000],
            ],
            // hot: black -> red -> yellow -> white
            Colormap::Hot => &[
                [0.000, 0.000, 0.000],
                [0.667, 0.000, 0.000],
                [1.000, 0.333, 0.000],
                [1.000, 1.000, 0.200],
                [1.000, 1.000, 1.000],
            ],
            Colormap::Grayscale => &[
                [0.000, 0.000, 0.000],
                [0.250, 0.250, 0.250],
                [0.500, 0.500, 0.500],
                [0.750, 0.750, 0.750],
                [1.000, 1.000, 1.000],
            ],
        }
    }

    /// Look up `t` in [0,1] → RGB u8, by piecewise-linear interpolation over stops.
    fn lookup(self, t: f32) -> [u8; 3] {
        let stops = self.stops();
        let n = stops.len();
        let t = t.clamp(0.0, 1.0);
        // position along the n-1 segments
        let scaled = t * (n - 1) as f32;
        let i = (scaled.floor() as usize).min(n - 2);
        let frac = scaled - i as f32;
        let a = stops[i];
        let b = stops[i + 1];
        let mut out = [0u8; 3];
        for c in 0..3 {
            let v = a[c] + (b[c] - a[c]) * frac;
            out[c] = (v * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        out
    }
}

/// Rec. 601 luma weights.
fn luminance(r: u8, g: u8, b: u8) -> f32 {
    (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0
}

/// Apply `cmap` to the luminance of every pixel in `image_bytes`. When `invert`
/// is true the luminance is flipped (1 - t) before lookup. Returns PNG bytes.
pub fn false_color(image_bytes: &[u8], cmap: Colormap, invert: bool) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("could not decode image: {e}"))?;
    let mut rgba = img.to_rgba8();
    for px in rgba.pixels_mut() {
        let [r, g, b, a] = px.0;
        let mut t = luminance(r, g, b);
        if invert {
            t = 1.0 - t;
        }
        let [nr, ng, nb] = cmap.lookup(t);
        px.0 = [nr, ng, nb, a];
    }

    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, Rgba, RgbaImage};

    fn solid_png(color: [u8; 4]) -> Vec<u8> {
        let img = RgbaImage::from_pixel(8, 8, Rgba(color));
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    fn first_pixel(png: &[u8]) -> [u8; 4] {
        image::load_from_memory(png).unwrap().get_pixel(0, 0).0
    }

    #[test]
    fn output_is_png_same_size_keeps_alpha() {
        let out = false_color(&solid_png([128, 128, 128, 200]), Colormap::Viridis, false).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!(img.dimensions(), (8, 8));
        assert_eq!(&out[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert_eq!(first_pixel(&out)[3], 200, "alpha preserved");
    }

    #[test]
    fn black_maps_to_low_end_white_to_high_end() {
        // viridis: black(t=0) -> dark purple [68,1,84], white(t=1) -> yellow [253,231,37]
        let lo = first_pixel(&false_color(&solid_png([0, 0, 0, 255]), Colormap::Viridis, false).unwrap());
        let hi = first_pixel(&false_color(&solid_png([255, 255, 255, 255]), Colormap::Viridis, false).unwrap());
        assert_eq!([lo[0], lo[1], lo[2]], [68, 1, 84]);
        assert_eq!([hi[0], hi[1], hi[2]], [253, 231, 37]);
    }

    #[test]
    fn invert_swaps_endpoints() {
        let normal = first_pixel(&false_color(&solid_png([0, 0, 0, 255]), Colormap::Viridis, false).unwrap());
        let inverted = first_pixel(&false_color(&solid_png([0, 0, 0, 255]), Colormap::Viridis, true).unwrap());
        // black inverted (t=1) should equal white's normal mapping (yellow)
        assert_eq!([inverted[0], inverted[1], inverted[2]], [253, 231, 37]);
        assert_ne!(normal, inverted);
    }

    #[test]
    fn grayscale_is_identity_on_gray() {
        // a mid gray (128) → luminance 0.502 → grayscale lookup ~128
        let p = first_pixel(&false_color(&solid_png([128, 128, 128, 255]), Colormap::Grayscale, false).unwrap());
        assert!((p[0] as i32 - 128).abs() <= 2, "got {p:?}");
        assert_eq!(p[0], p[1]);
        assert_eq!(p[1], p[2]);
    }

    #[test]
    fn hot_endpoints() {
        let lo = first_pixel(&false_color(&solid_png([0, 0, 0, 255]), Colormap::Hot, false).unwrap());
        let hi = first_pixel(&false_color(&solid_png([255, 255, 255, 255]), Colormap::Hot, false).unwrap());
        assert_eq!([lo[0], lo[1], lo[2]], [0, 0, 0], "hot starts black");
        assert_eq!([hi[0], hi[1], hi[2]], [255, 255, 255], "hot ends white");
    }

    #[test]
    fn parse_and_errors() {
        assert_eq!(Colormap::parse("viridis").unwrap(), Colormap::Viridis);
        assert_eq!(Colormap::parse("").unwrap(), Colormap::Viridis);
        assert_eq!(Colormap::parse("THERMAL").unwrap(), Colormap::Hot);
        assert_eq!(Colormap::parse("rainbow").unwrap(), Colormap::Jet);
        assert_eq!(Colormap::parse("cividis").unwrap(), Colormap::Cividis);
        assert!(Colormap::parse("bogus").is_err());
        assert!(false_color(b"not an image", Colormap::Jet, false).is_err());
    }
}
