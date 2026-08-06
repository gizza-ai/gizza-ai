//! gizza-ai/image-skin-tone-analyzer core — find the skin-tone pixels in a photo,
//! measure their average hue, and suggest the warm/cool white-balance correction
//! that would bring that hue back to a reference. No wafer/wasm-bindgen deps;
//! pure-Rust `image` decode plus hand-rolled colour maths. Shared by the chat
//! skill block, the CLI and the unit tests.
//!
//! ## How it works
//!
//! 1. **Segment.** Skin is detected per pixel with a rule-based chroma test in
//!    YCbCr (BT.601) — the classic Cb/Cr band, which is far more robust across
//!    skin depths than an RGB threshold because it separates luma from colour.
//!    `Sensitivity::Strict` additionally demands the Kovac RGB rule
//!    (`R > G > B` with a minimum spread); `Loose` widens the band for photos
//!    that carry a heavy cast. There is no face detection here (that needs a
//!    model) — every skin-coloured pixel in the frame counts, and the reported
//!    coverage + confidence make a thin sample visible.
//! 2. **Average in linear light.** Channel means are accumulated after sRGB →
//!    linear decoding, so the mean is a real average of *light* rather than of
//!    gamma-encoded numbers.
//! 3. **Describe.** The mean is reported as hex/RGB, as an HSV hue, and in CIELAB
//!    (D65) — from which come the a\*b\* hue angle (the undertone axis) and the
//!    Individual Typology Angle, `ITA° = arctan((L* − 50) / b*)`, whose published
//!    bands give the depth label.
//! 4. **Suggest a correction.** The image is assumed to have been rendered at
//!    [`ASSUMED_CAPTURE_KELVIN`] (a JPEG/PNG carries no dependable white-balance
//!    tag). Candidate camera temperatures across [`MIN_KELVIN`]..=[`MAX_KELVIN`]
//!    are applied to the mean skin colour as a von-Kries gain — the componentwise
//!    ratio of the two illuminants' linear-sRGB white points, renormalised to
//!    preserve luminance — and the temperature whose corrected skin hue lands
//!    closest to `reference_hue` is reported. Raising the temperature warms the
//!    render, so skin that reads too yellow yields a negative shift, matching a
//!    raw editor's temperature slider.
//!
//! The suggestion is a starting point, not a calibrated measurement: an unknown
//! capture temperature and the subject's own skin hue are confounded in a single
//! rendered image.

use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageReader};

/// The white balance the input is assumed to have been rendered at, in kelvin.
/// Reported back so the suggested shift is interpretable.
pub const ASSUMED_CAPTURE_KELVIN: f64 = 5500.0;

/// Coolest camera temperature the solver will suggest.
pub const MIN_KELVIN: f64 = 2000.0;
/// Warmest camera temperature the solver will suggest.
pub const MAX_KELVIN: f64 = 12000.0;
/// Step of the temperature scan, in kelvin. Results are reported on this grid.
const KELVIN_STEP: f64 = 10.0;

/// Decoded-pixel budget. The block runs in a 64 MiB wasm sandbox, so the raster
/// plus the still-held input must stay well inside it (see the big-image note in
/// the create-next-tool wasm-crates reference).
const MAX_DECODE_BYTES: u64 = 48 * 1024 * 1024;

/// At most this many pixels are inspected; larger images are visited on a stride.
/// Skin coverage is a ratio, so sampling does not bias it.
const MAX_SAMPLED_PIXELS: u64 = 400_000;

/// Alpha below this is treated as transparent and skipped entirely.
const ALPHA_THRESHOLD: u8 = 128;

/// A skin pixel at or above this in any channel is counted as blown.
const CLIP_HIGH: u8 = 250;
/// A skin pixel at or below this luma is counted as crushed.
const CLIP_LOW_LUMA: f64 = 16.0;

/// Coverage (in percent) at which the coverage term of `confidence` saturates.
const FULL_CONFIDENCE_COVERAGE: f64 = 5.0;

/// Mean chroma below which the reading is treated as unreliable (a near-grey
/// average carries almost no hue information).
const LOW_CHROMA: f64 = 5.0;

/// How permissive the skin-pixel test is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sensitivity {
    /// Narrow chroma band **and** the Kovac RGB rule. Fewest false positives,
    /// misses skin in shadow or under a cast.
    Strict,
    /// The classic Cb/Cr band (Chai & Ngan). The default.
    Normal,
    /// Widened band, no RGB rule — use when a heavy colour cast has pushed real
    /// skin outside the normal band.
    Loose,
}

impl Sensitivity {
    /// Parse the canonical descriptor value. Unknown values are an error so the
    /// caller can report bad args rather than silently defaulting.
    pub fn parse(s: &str) -> Result<Sensitivity, String> {
        match s {
            "strict" => Ok(Sensitivity::Strict),
            "normal" => Ok(Sensitivity::Normal),
            "loose" => Ok(Sensitivity::Loose),
            other => Err(format!(
                "unknown sensitivity '{other}' (expected 'strict', 'normal' or 'loose')"
            )),
        }
    }

    /// Canonical string form, echoed back in the result.
    pub fn as_str(self) -> &'static str {
        match self {
            Sensitivity::Strict => "strict",
            Sensitivity::Normal => "normal",
            Sensitivity::Loose => "loose",
        }
    }

    /// `(cb_min, cb_max, cr_min, cr_max, luma_min, require_rgb_rule)`.
    fn band(self) -> (f64, f64, f64, f64, f64, bool) {
        match self {
            Sensitivity::Strict => (80.0, 120.0, 135.0, 170.0, 60.0, true),
            Sensitivity::Normal => (77.0, 127.0, 133.0, 173.0, 40.0, false),
            Sensitivity::Loose => (70.0, 135.0, 128.0, 180.0, 25.0, false),
        }
    }
}

/// Everything measured about the skin pixels, plus the suggested correction.
#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    /// Full image width in pixels.
    pub width: u32,
    /// Full image height in pixels.
    pub height: u32,
    /// Pixels actually inspected (the whole image unless it was strided).
    pub sampled_pixels: u64,
    /// Inspected pixels that passed the skin test.
    pub skin_pixels: u64,
    /// `skin_pixels / sampled_pixels`, in percent, 2 dp.
    pub skin_percent: f64,
    /// Sensitivity that produced this mask.
    pub sensitivity: &'static str,

    /// Mean skin colour as `#rrggbb` (lowercase).
    pub hex: String,
    /// Mean skin colour as CSS `rgb(r, g, b)`.
    pub rgb: String,
    /// Mean red channel, 0-255.
    pub r: u8,
    /// Mean green channel, 0-255.
    pub g: u8,
    /// Mean blue channel, 0-255.
    pub b: u8,

    /// HSV hue of the mean skin colour, degrees 0-360, 2 dp. The headline number.
    pub hue_degrees: f64,
    /// HSV saturation, percent, 2 dp.
    pub saturation_percent: f64,
    /// HSV value/brightness, percent, 2 dp.
    pub brightness_percent: f64,

    /// CIELAB L\* (D65), 2 dp.
    pub lab_l: f64,
    /// CIELAB a\* (green ↔ magenta), 2 dp.
    pub lab_a: f64,
    /// CIELAB b\* (blue ↔ yellow), 2 dp.
    pub lab_b: f64,
    /// CIELAB chroma `sqrt(a² + b²)`, 2 dp.
    pub lab_chroma: f64,
    /// CIELAB a\*b\* hue angle in degrees, 2 dp — the undertone/cast axis.
    pub lab_hue_degrees: f64,

    /// Individual Typology Angle in degrees, 2 dp.
    pub ita_degrees: f64,
    /// Depth band from the ITA: very light / light / intermediate / tan / brown / dark.
    pub depth: &'static str,
    /// Undertone from the a\*b\* hue angle: cool / neutral / warm / olive.
    pub undertone: &'static str,

    /// Reference hue the cast was measured against, degrees.
    pub reference_hue_degrees: f64,
    /// `lab_hue_degrees − reference_hue_degrees`, 2 dp. Positive = too yellow.
    pub hue_offset_degrees: f64,
    /// Cast direction: warm / cool / neutral.
    pub cast: &'static str,
    /// True when `|hue_offset_degrees|` is within the caller's tolerance.
    pub is_balanced: bool,

    /// Temperature the render is assumed to have been made at, kelvin.
    pub assumed_capture_kelvin: f64,
    /// Camera temperature that would land skin on the reference hue, kelvin.
    pub suggested_kelvin: f64,
    /// `suggested_kelvin − assumed_capture_kelvin`. Negative = cool it down.
    pub suggested_kelvin_shift: f64,
    /// True when the solver hit a scan bound and could not reach the reference.
    pub kelvin_clamped: bool,

    /// Share of skin pixels that were blown or crushed, percent, 2 dp.
    pub clipped_percent: f64,
    /// 0-1 reliability estimate, 2 dp.
    pub confidence: f64,
    /// Non-fatal problems with the reading.
    pub warnings: Vec<String>,
}

/// Analyse `bytes` and report the skin-tone reading plus the suggested correction.
///
/// * `sensitivity` — how permissive the skin mask is.
/// * `reference_hue` — the CIELAB a\*b\* hue angle, in degrees, that correctly
///   balanced skin is expected to sit at (52 is a good default for facial skin).
/// * `tolerance` — hue offset, in degrees, still considered balanced.
/// * `min_skin_percent` — coverage gate; below this the reading is refused
///   rather than reported from a handful of pixels. Pass 0 to disable.
///
/// Errors carry an actionable message (empty input, undecodable bytes, an image
/// too large for the sandbox, or too little skin found).
pub fn analyze(
    bytes: &[u8],
    sensitivity: Sensitivity,
    reference_hue: f64,
    tolerance: f64,
    min_skin_percent: f64,
) -> Result<Analysis, String> {
    if bytes.is_empty() {
        return Err("image is empty — pass a PNG, JPEG, WebP, GIF or BMP".into());
    }
    if !reference_hue.is_finite() || !(0.0..=90.0).contains(&reference_hue) {
        return Err(format!(
            "reference_hue must be between 0 and 90 degrees (got {reference_hue})"
        ));
    }
    if !tolerance.is_finite() || !(0.0..=30.0).contains(&tolerance) {
        return Err(format!(
            "tolerance must be between 0 and 30 degrees (got {tolerance})"
        ));
    }
    if !min_skin_percent.is_finite() || !(0.0..=100.0).contains(&min_skin_percent) {
        return Err(format!(
            "min_skin_percent must be between 0 and 100 (got {min_skin_percent})"
        ));
    }

    let rgba = decode(bytes)?;
    let (width, height) = (rgba.width(), rgba.height());

    // Stride so at most MAX_SAMPLED_PIXELS are inspected. Coverage is a ratio, so
    // a uniform stride leaves it unbiased.
    let total = u64::from(width) * u64::from(height);
    let step = stride_for(total);

    let (mut lin_r, mut lin_g, mut lin_b) = (0.0f64, 0.0f64, 0.0f64);
    let (mut sampled, mut skin, mut clipped) = (0u64, 0u64, 0u64);

    let mut y = 0u32;
    while y < height {
        let mut x = 0u32;
        while x < width {
            let px = rgba.get_pixel(x, y).0;
            if px[3] >= ALPHA_THRESHOLD {
                sampled += 1;
                if is_skin(px[0], px[1], px[2], sensitivity) {
                    skin += 1;
                    lin_r += srgb_to_linear(px[0]);
                    lin_g += srgb_to_linear(px[1]);
                    lin_b += srgb_to_linear(px[2]);
                    if px[0].max(px[1]).max(px[2]) >= CLIP_HIGH
                        || luma(px[0], px[1], px[2]) <= CLIP_LOW_LUMA
                    {
                        clipped += 1;
                    }
                }
            }
            x += step;
        }
        y += step;
    }

    if sampled == 0 {
        return Err("every sampled pixel was transparent — the image has no opaque content".into());
    }
    if skin == 0 {
        return Err(format!(
            "no skin-tone pixels found at sensitivity '{}' — try sensitivity 'loose' (a heavy \
             colour cast can push real skin outside the detection band), or use a photo where \
             skin fills more of the frame",
            sensitivity.as_str()
        ));
    }

    let skin_percent = round2(skin as f64 / sampled as f64 * 100.0);
    if skin_percent < min_skin_percent {
        return Err(format!(
            "only {skin_percent}% of pixels read as skin, below the min_skin_percent gate of \
             {min_skin_percent}% — lower min_skin_percent (0 disables the gate), use sensitivity \
             'loose', or crop closer to the subject"
        ));
    }

    let n = skin as f64;
    let (mr, mg, mb) = (lin_r / n, lin_g / n, lin_b / n);
    let (r8, g8, b8) = (linear_to_srgb(mr), linear_to_srgb(mg), linear_to_srgb(mb));

    let (l, a, b) = linear_rgb_to_lab(mr, mg, mb);
    let chroma = (a * a + b * b).sqrt();
    let lab_hue = norm_deg(b.atan2(a).to_degrees());
    let (hsv_h, hsv_s, hsv_v) = rgb_to_hsv(r8, g8, b8);
    let ita = ita_degrees(l, b);

    let offset = lab_hue - reference_hue;
    let cast = if offset.abs() <= tolerance {
        "neutral"
    } else if offset > 0.0 {
        "warm"
    } else {
        "cool"
    };

    let (kelvin, clamped) = solve_kelvin(mr, mg, mb, reference_hue);

    let clipped_percent = round2(clipped as f64 / n * 100.0);
    let confidence = confidence_score(skin_percent, clipped_percent, chroma);

    let mut warnings = Vec::new();
    if skin_percent < 1.0 {
        warnings.push(format!(
            "skin covers only {skin_percent}% of the frame — the reading is from a small sample"
        ));
    }
    if clipped_percent >= 10.0 {
        warnings.push(format!(
            "{clipped_percent}% of the skin pixels are blown or crushed — clipped pixels carry no \
             reliable hue, so re-expose and re-run"
        ));
    }
    if chroma < LOW_CHROMA {
        warnings.push(
            "the mean skin colour is nearly neutral grey, so its hue is not meaningful".into(),
        );
    }
    if clamped {
        warnings.push(format!(
            "the reference hue is unreachable within {MIN_KELVIN:.0}-{MAX_KELVIN:.0} K — \
             suggested_kelvin is clamped and the residual cast is not purely a temperature error"
        ));
    }

    Ok(Analysis {
        width,
        height,
        sampled_pixels: sampled,
        skin_pixels: skin,
        skin_percent,
        sensitivity: sensitivity.as_str(),
        hex: format!("#{r8:02x}{g8:02x}{b8:02x}"),
        rgb: format!("rgb({r8}, {g8}, {b8})"),
        r: r8,
        g: g8,
        b: b8,
        hue_degrees: round2(hsv_h),
        saturation_percent: round2(hsv_s),
        brightness_percent: round2(hsv_v),
        lab_l: round2(l),
        lab_a: round2(a),
        lab_b: round2(b),
        lab_chroma: round2(chroma),
        lab_hue_degrees: round2(lab_hue),
        ita_degrees: round2(ita),
        depth: depth_label(ita),
        undertone: undertone_label(lab_hue),
        reference_hue_degrees: reference_hue,
        hue_offset_degrees: round2(offset),
        cast,
        is_balanced: offset.abs() <= tolerance,
        assumed_capture_kelvin: ASSUMED_CAPTURE_KELVIN,
        suggested_kelvin: kelvin,
        suggested_kelvin_shift: kelvin - ASSUMED_CAPTURE_KELVIN,
        kelvin_clamped: clamped,
        clipped_percent,
        confidence,
        warnings,
    })
}

/// Decode with a header-first size budget so an oversized raster is refused with
/// an actionable message instead of trapping the wasm sandbox on allocation.
fn decode(bytes: &[u8]) -> Result<image::RgbaImage, String> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("could not read the image header: {e}"))?;
    let decoder = reader.into_decoder().map_err(|e| {
        format!("could not decode the image (PNG, JPEG, WebP, GIF and BMP are supported): {e}")
    })?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 {
        return Err("the image has zero width or height".into());
    }
    let needed = bytes.len() as u64 + decoder.total_bytes();
    if needed > MAX_DECODE_BYTES {
        return Err(format!(
            "image is too large to analyse in the sandbox ({w}x{h} needs about {} MB, the limit is \
             {} MB) — re-export it at a lower resolution",
            needed / (1024 * 1024),
            MAX_DECODE_BYTES / (1024 * 1024)
        ));
    }
    let img = DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("could not decode the image: {e}"))?;
    Ok(img.to_rgba8())
}

/// Smallest stride keeping the inspected pixel count under [`MAX_SAMPLED_PIXELS`].
fn stride_for(total_pixels: u64) -> u32 {
    if total_pixels <= MAX_SAMPLED_PIXELS {
        return 1;
    }
    let ratio = total_pixels as f64 / MAX_SAMPLED_PIXELS as f64;
    ratio.sqrt().ceil().max(1.0) as u32
}

/// BT.601 luma.
fn luma(r: u8, g: u8, b: u8) -> f64 {
    0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b)
}

/// Rule-based skin test in YCbCr (BT.601), optionally plus the Kovac RGB rule.
pub fn is_skin(r: u8, g: u8, b: u8, sensitivity: Sensitivity) -> bool {
    let (cb_min, cb_max, cr_min, cr_max, luma_min, rgb_rule) = sensitivity.band();
    let (rf, gf, bf) = (f64::from(r), f64::from(g), f64::from(b));
    let y = luma(r, g, b);
    if y < luma_min {
        return false;
    }
    let cb = 128.0 - 0.168_736 * rf - 0.331_264 * gf + 0.5 * bf;
    let cr = 128.0 + 0.5 * rf - 0.418_688 * gf - 0.081_312 * bf;
    if !(cb_min..=cb_max).contains(&cb) || !(cr_min..=cr_max).contains(&cr) {
        return false;
    }
    if rgb_rule {
        let mx = r.max(g).max(b);
        let mn = r.min(g).min(b);
        let spread = f64::from(mx) - f64::from(mn);
        if !(rf > 95.0
            && gf > 40.0
            && bf > 20.0
            && spread > 15.0
            && (rf - gf).abs() > 15.0
            && r > g
            && r > b)
        {
            return false;
        }
    }
    true
}

/// sRGB 0-255 → linear 0-1.
fn srgb_to_linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear 0-1 → sRGB 0-255, rounded.
fn linear_to_srgb(c: f64) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Linear sRGB → CIELAB (D65).
fn linear_rgb_to_lab(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let x = 0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
    let z = 0.019_333_9 * r + 0.119_192_0 * g + 0.950_304_1 * b;
    const XN: f64 = 0.95047;
    const YN: f64 = 1.0;
    const ZN: f64 = 1.08883;
    fn f(t: f64) -> f64 {
        if t > 0.008_856_451_679_035_631 {
            t.cbrt()
        } else {
            7.787_037_037_037_035 * t + 16.0 / 116.0
        }
    }
    let (fx, fy, fz) = (f(x / XN), f(y / YN), f(z / ZN));
    (116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))
}

/// sRGB 0-255 → HSV (degrees, percent, percent).
fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let (rf, gf, bf) = (
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
    );
    let mx = rf.max(gf).max(bf);
    let mn = rf.min(gf).min(bf);
    let d = mx - mn;
    let h = if d <= f64::EPSILON {
        0.0
    } else if (mx - rf).abs() < f64::EPSILON {
        60.0 * (((gf - bf) / d) % 6.0)
    } else if (mx - gf).abs() < f64::EPSILON {
        60.0 * ((bf - rf) / d + 2.0)
    } else {
        60.0 * ((rf - gf) / d + 4.0)
    };
    let s = if mx <= f64::EPSILON {
        0.0
    } else {
        d / mx * 100.0
    };
    (norm_deg(h), s, mx * 100.0)
}

/// Individual Typology Angle: `arctan((L* − 50) / b*)` in degrees.
fn ita_degrees(l: f64, b: f64) -> f64 {
    if b.abs() < 1e-9 {
        return if l >= 50.0 { 90.0 } else { -90.0 };
    }
    ((l - 50.0) / b).atan().to_degrees()
}

/// Published ITA depth bands (Chardon et al.).
pub fn depth_label(ita: f64) -> &'static str {
    if ita > 55.0 {
        "very light"
    } else if ita > 41.0 {
        "light"
    } else if ita > 28.0 {
        "intermediate"
    } else if ita > 10.0 {
        "tan"
    } else if ita > -30.0 {
        "brown"
    } else {
        "dark"
    }
}

/// Undertone from the CIELAB a\*b\* hue angle. Boundaries are stated in the docs
/// and echoed on every surface so the classification is never a black box.
pub fn undertone_label(lab_hue: f64) -> &'static str {
    if lab_hue < 45.0 {
        "cool"
    } else if lab_hue < 58.0 {
        "neutral"
    } else if lab_hue < 68.0 {
        "warm"
    } else {
        "olive"
    }
}

/// Linear-sRGB white point of a Planckian radiator at `kelvin`, normalised to
/// unit luminance. `xy` uses the standard cubic approximation of the locus,
/// valid 1667-25000 K.
fn planckian_white(kelvin: f64) -> (f64, f64, f64) {
    let t = kelvin.clamp(1667.0, 25000.0);
    let (t1, t2, t3) = (1.0e3 / t, 1.0e6 / (t * t), 1.0e9 / (t * t * t));
    let x = if t <= 4000.0 {
        -0.266_123_9 * t3 - 0.234_358_9 * t2 + 0.877_695_6 * t1 + 0.179_910
    } else {
        -3.025_846_9 * t3 + 2.107_037_9 * t2 + 0.222_634_7 * t1 + 0.240_390
    };
    let y = if t <= 2222.0 {
        -1.106_381_4 * x * x * x - 1.348_110_20 * x * x + 2.185_558_32 * x - 0.202_196_83
    } else if t <= 4000.0 {
        -0.954_947_6 * x * x * x - 1.374_185_93 * x * x + 2.091_370_15 * x - 0.167_488_67
    } else {
        3.081_758_0 * x * x * x - 5.873_386_70 * x * x + 3.751_129_97 * x - 0.370_014_83
    };
    // xyY (Y = 1) → XYZ → linear sRGB.
    let big_x = x / y;
    let big_z = (1.0 - x - y) / y;
    let r = 3.240_454_2 * big_x - 1.537_138_5 - 0.498_531_4 * big_z;
    let g = -0.969_266_0 * big_x + 1.876_010_8 + 0.041_556_0 * big_z;
    let b = 0.055_643_4 * big_x - 0.204_025_9 + 1.057_225_2 * big_z;
    (r.max(1e-6), g.max(1e-6), b.max(1e-6))
}

/// Apply the white-balance gain for rendering at `kelvin` instead of
/// [`ASSUMED_CAPTURE_KELVIN`], preserving luminance.
fn rebalance(r: f64, g: f64, b: f64, kelvin: f64) -> (f64, f64, f64) {
    let (sr, sg, sb) = planckian_white(ASSUMED_CAPTURE_KELVIN);
    let (dr, dg, db) = planckian_white(kelvin);
    let (mut nr, mut ng, mut nb) = (r * sr / dr, g * sg / dg, b * sb / db);
    let before = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
    let after = 0.212_672_9 * nr + 0.715_152_2 * ng + 0.072_175_0 * nb;
    if after > 1e-12 {
        let k = before / after;
        nr *= k;
        ng *= k;
        nb *= k;
    }
    (nr, ng, nb)
}

/// Scan the temperature grid for the render that lands skin closest to
/// `reference_hue`. Returns `(kelvin, hit_a_scan_bound)`.
fn solve_kelvin(r: f64, g: f64, b: f64, reference_hue: f64) -> (f64, bool) {
    let mut best_k = ASSUMED_CAPTURE_KELVIN;
    let mut best_err = f64::INFINITY;
    let steps = ((MAX_KELVIN - MIN_KELVIN) / KELVIN_STEP).round() as u32;
    for i in 0..=steps {
        let k = MIN_KELVIN + f64::from(i) * KELVIN_STEP;
        let (cr, cg, cb) = rebalance(r, g, b, k);
        let (_, a, bb) = linear_rgb_to_lab(cr, cg, cb);
        let err = (norm_deg(bb.atan2(a).to_degrees()) - reference_hue).abs();
        if err < best_err {
            best_err = err;
            best_k = k;
        }
    }
    // A bound is only "clamped" if the reference was genuinely not reachable —
    // landing exactly on a bound with a near-zero residual is a real solution.
    let clamped = (best_k <= MIN_KELVIN || best_k >= MAX_KELVIN) && best_err > 0.5;
    (best_k, clamped)
}

/// 0-1 reliability: coverage, minus clipping, halved when the mean is near-grey.
fn confidence_score(skin_percent: f64, clipped_percent: f64, chroma: f64) -> f64 {
    let coverage = (skin_percent / FULL_CONFIDENCE_COVERAGE).clamp(0.0, 1.0);
    let clean = (1.0 - clipped_percent / 100.0).clamp(0.0, 1.0);
    let chroma_factor = if chroma < LOW_CHROMA { 0.5 } else { 1.0 };
    round2(coverage * clean * chroma_factor)
}

/// Wrap into 0-360.
fn norm_deg(d: f64) -> f64 {
    let m = d % 360.0;
    if m < 0.0 {
        m + 360.0
    } else {
        m
    }
}

/// Round to 2 decimal places (the reporting precision everywhere).
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};

    /// A solid-colour PNG — every pixel identical, so the mean is exact.
    fn solid_png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba(rgba));
        let mut out = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    /// Half skin colour, half a flat blue that no band accepts.
    fn half_skin_png(w: u32, h: u32, skin: [u8; 4]) -> Vec<u8> {
        let mut img = RgbaImage::from_pixel(w, h, Rgba([40, 80, 200, 255]));
        for y in 0..h / 2 {
            for x in 0..w {
                img.put_pixel(x, y, Rgba(skin));
            }
        }
        let mut out = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    fn analyze_default(bytes: &[u8]) -> Result<Analysis, String> {
        analyze(bytes, Sensitivity::Normal, 52.0, 4.0, 0.5)
    }

    #[test]
    fn balanced_skin_swatch_reports_its_hue_and_needs_no_correction() {
        let png = solid_png(40, 30, [204, 136, 102, 255]);
        let a = analyze_default(&png).unwrap();

        assert_eq!((a.width, a.height), (40, 30));
        assert_eq!(a.sampled_pixels, 1200);
        assert_eq!(a.skin_pixels, 1200);
        assert_eq!(a.skin_percent, 100.0);
        assert_eq!(a.hex, "#cc8866");
        assert_eq!(a.rgb, "rgb(204, 136, 102)");

        // Independently computed from the sRGB→linear→XYZ→LAB chain.
        assert_eq!(a.hue_degrees, 20.0);
        assert_eq!(a.saturation_percent, 50.0);
        assert_eq!(a.brightness_percent, 80.0);
        assert_eq!(a.lab_l, 62.85);
        assert_eq!(a.lab_a, 22.25);
        assert_eq!(a.lab_b, 28.83);
        assert_eq!(a.lab_hue_degrees, 52.35);
        assert_eq!(a.ita_degrees, 24.02);
        assert_eq!(a.depth, "tan");
        assert_eq!(a.undertone, "neutral");

        // 52.35 sits 0.35 off the 52 reference — inside the 4 tolerance.
        assert_eq!(a.hue_offset_degrees, 0.35);
        assert_eq!(a.cast, "neutral");
        assert!(a.is_balanced);
        assert!(
            (a.suggested_kelvin - ASSUMED_CAPTURE_KELVIN).abs() <= 160.0,
            "already-balanced skin should need only a small shift, got {}",
            a.suggested_kelvin
        );
        assert!(!a.kelvin_clamped);
        assert_eq!(a.clipped_percent, 0.0);
        assert_eq!(a.confidence, 1.0);
        assert!(
            a.warnings.is_empty(),
            "unexpected warnings: {:?}",
            a.warnings
        );
    }

    #[test]
    fn warm_cast_asks_for_a_lower_colour_temperature() {
        // Pushed toward yellow: b* rises, so the a*b* hue angle climbs past 52.
        let png = solid_png(20, 20, [214, 146, 88, 255]);
        let a = analyze_default(&png).unwrap();
        assert!(
            a.lab_hue_degrees > 58.0,
            "expected a yellow-leaning hue, got {}",
            a.lab_hue_degrees
        );
        assert_eq!(a.cast, "warm");
        assert_eq!(a.undertone, "warm");
        assert!(!a.is_balanced);
        assert!(a.hue_offset_degrees > 0.0);
        assert!(
            a.suggested_kelvin < ASSUMED_CAPTURE_KELVIN,
            "a warm cast must be cooled down, got {} K",
            a.suggested_kelvin
        );
        assert!(a.suggested_kelvin_shift < 0.0);
    }

    #[test]
    fn cool_cast_asks_for_a_higher_colour_temperature() {
        // Pushed toward pink/blue: b* falls, so the hue angle drops below 52.
        let png = solid_png(20, 20, [206, 132, 130, 255]);
        let a = analyze_default(&png).unwrap();
        assert!(
            a.lab_hue_degrees < 45.0,
            "expected a pink-leaning hue, got {}",
            a.lab_hue_degrees
        );
        assert_eq!(a.cast, "cool");
        assert_eq!(a.undertone, "cool");
        assert!(a.hue_offset_degrees < 0.0);
        assert!(
            a.suggested_kelvin > ASSUMED_CAPTURE_KELVIN,
            "a cool cast must be warmed up, got {} K",
            a.suggested_kelvin
        );
        assert!(a.suggested_kelvin_shift > 0.0);
    }

    #[test]
    fn correcting_by_the_suggestion_lands_on_the_reference_hue() {
        // End-to-end check of the solver: apply its own answer and re-measure.
        let png = solid_png(16, 16, [214, 146, 88, 255]);
        let a = analyze_default(&png).unwrap();
        let (lr, lg, lb) = (srgb_to_linear(214), srgb_to_linear(146), srgb_to_linear(88));
        let (cr, cg, cb) = rebalance(lr, lg, lb, a.suggested_kelvin);
        let (_, la, lbb) = linear_rgb_to_lab(cr, cg, cb);
        let corrected_hue = lbb.atan2(la).to_degrees();
        assert!(
            (corrected_hue - 52.0).abs() < 0.5,
            "corrected hue {corrected_hue} should sit on the 52 reference"
        );
    }

    #[test]
    fn coverage_is_measured_and_the_mask_ignores_non_skin_pixels() {
        let png = half_skin_png(40, 40, [204, 136, 102, 255]);
        let a = analyze_default(&png).unwrap();
        assert_eq!(a.sampled_pixels, 1600);
        assert_eq!(a.skin_pixels, 800);
        assert_eq!(a.skin_percent, 50.0);
        // The blue half must not drag the mean — it is still the exact swatch.
        assert_eq!(a.hex, "#cc8866");
    }

    #[test]
    fn sensitivity_changes_which_pixels_qualify() {
        // A desaturated, shadowed skin tone: inside the loose band, rejected by
        // the strict rule's luma floor and RGB spread.
        let dim = [96, 78, 70, 255];
        assert!(is_skin(dim[0], dim[1], dim[2], Sensitivity::Loose));
        assert!(!is_skin(dim[0], dim[1], dim[2], Sensitivity::Strict));

        let png = solid_png(10, 10, dim);
        assert!(analyze(&png, Sensitivity::Loose, 52.0, 4.0, 0.5).is_ok());
        let err = analyze(&png, Sensitivity::Strict, 52.0, 4.0, 0.5).unwrap_err();
        assert!(err.contains("no skin-tone pixels found"), "{err}");
    }

    #[test]
    fn low_coverage_trips_the_gate_and_the_message_says_how_to_proceed() {
        let png = half_skin_png(40, 40, [204, 136, 102, 255]);
        let err = analyze(&png, Sensitivity::Normal, 52.0, 4.0, 80.0).unwrap_err();
        assert!(err.contains("50%"), "{err}");
        assert!(err.contains("min_skin_percent"), "{err}");
        // Gate disabled → the same image reports fine.
        assert!(analyze(&png, Sensitivity::Normal, 52.0, 4.0, 0.0).is_ok());
    }

    #[test]
    fn transparent_pixels_are_skipped_not_counted_as_skin() {
        let mut img = RgbaImage::from_pixel(20, 20, Rgba([204, 136, 102, 0]));
        for y in 0..10 {
            for x in 0..20 {
                img.put_pixel(x, y, Rgba([204, 136, 102, 255]));
            }
        }
        let mut png = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        let a = analyze_default(&png).unwrap();
        assert_eq!(a.sampled_pixels, 200);
        assert_eq!(a.skin_pixels, 200);
        assert_eq!(a.skin_percent, 100.0);
    }

    #[test]
    fn blown_skin_pixels_are_reported_and_lower_the_confidence() {
        let png = solid_png(20, 20, [252, 214, 200, 255]);
        let a = analyze(&png, Sensitivity::Normal, 52.0, 4.0, 0.0).unwrap();
        assert_eq!(a.clipped_percent, 100.0);
        assert_eq!(a.confidence, 0.0);
        assert!(
            a.warnings.iter().any(|w| w.contains("blown or crushed")),
            "{:?}",
            a.warnings
        );
    }

    #[test]
    fn tolerance_widens_what_counts_as_balanced() {
        let png = solid_png(20, 20, [214, 146, 88, 255]);
        let tight = analyze(&png, Sensitivity::Normal, 52.0, 4.0, 0.0).unwrap();
        assert!(!tight.is_balanced);
        let loose = analyze(&png, Sensitivity::Normal, 52.0, 30.0, 0.0).unwrap();
        assert!(loose.is_balanced);
        assert_eq!(loose.cast, "neutral");
        // The measurement itself is unchanged — only the verdict moves.
        assert_eq!(tight.lab_hue_degrees, loose.lab_hue_degrees);
    }

    #[test]
    fn reference_hue_moves_the_target() {
        let png = solid_png(20, 20, [204, 136, 102, 255]);
        let at52 = analyze(&png, Sensitivity::Normal, 52.0, 4.0, 0.0).unwrap();
        let at40 = analyze(&png, Sensitivity::Normal, 40.0, 4.0, 0.0).unwrap();
        assert_eq!(at40.reference_hue_degrees, 40.0);
        assert!(at40.hue_offset_degrees > at52.hue_offset_degrees);
        assert_eq!(at40.cast, "warm");
        // Wanting a pinker target means cooling the render down.
        assert!(at40.suggested_kelvin < at52.suggested_kelvin);
    }

    #[test]
    fn depth_bands_follow_the_published_ita_boundaries() {
        assert_eq!(depth_label(60.0), "very light");
        assert_eq!(depth_label(50.0), "light");
        assert_eq!(depth_label(35.0), "intermediate");
        assert_eq!(depth_label(20.0), "tan");
        assert_eq!(depth_label(0.0), "brown");
        assert_eq!(depth_label(-40.0), "dark");
    }

    #[test]
    fn undertone_bands_are_the_documented_ones() {
        assert_eq!(undertone_label(30.0), "cool");
        assert_eq!(undertone_label(44.9), "cool");
        assert_eq!(undertone_label(45.0), "neutral");
        assert_eq!(undertone_label(57.9), "neutral");
        assert_eq!(undertone_label(58.0), "warm");
        assert_eq!(undertone_label(67.9), "warm");
        assert_eq!(undertone_label(68.0), "olive");
    }

    #[test]
    fn a_dark_skin_swatch_is_measured_not_rejected() {
        let png = solid_png(20, 20, [96, 62, 48, 255]);
        let a = analyze(&png, Sensitivity::Loose, 52.0, 4.0, 0.0).unwrap();
        assert!(a.lab_l < 40.0, "expected a dark L*, got {}", a.lab_l);
        assert!(
            matches!(a.depth, "brown" | "dark"),
            "expected a deep band, got {}",
            a.depth
        );
        assert!(a.hue_degrees > 0.0);
    }

    #[test]
    fn raising_the_temperature_warms_the_render() {
        // Direction convention: higher kelvin ⇒ more yellow ⇒ higher a*b* hue.
        let (r, g, b) = (
            srgb_to_linear(204),
            srgb_to_linear(136),
            srgb_to_linear(102),
        );
        let hue_at = |k: f64| {
            let (cr, cg, cb) = rebalance(r, g, b, k);
            let (_, a, bb) = linear_rgb_to_lab(cr, cg, cb);
            bb.atan2(a).to_degrees()
        };
        assert!(hue_at(3000.0) < hue_at(5500.0));
        assert!(hue_at(5500.0) < hue_at(9000.0));
        // The assumed capture temperature is a no-op.
        let (cr, cg, cb) = rebalance(r, g, b, ASSUMED_CAPTURE_KELVIN);
        assert!((cr - r).abs() < 1e-9 && (cg - g).abs() < 1e-9 && (cb - b).abs() < 1e-9);
    }

    #[test]
    fn large_images_are_strided_rather_than_walked_pixel_by_pixel() {
        assert_eq!(stride_for(1000), 1);
        assert_eq!(stride_for(MAX_SAMPLED_PIXELS), 1);
        assert_eq!(stride_for(MAX_SAMPLED_PIXELS * 4), 2);
        assert_eq!(stride_for(MAX_SAMPLED_PIXELS * 9), 3);
    }

    #[test]
    fn bad_parameters_are_rejected_with_a_usable_message() {
        let png = solid_png(8, 8, [204, 136, 102, 255]);
        assert!(analyze(&png, Sensitivity::Normal, 120.0, 4.0, 0.5)
            .unwrap_err()
            .contains("reference_hue"));
        assert!(analyze(&png, Sensitivity::Normal, 52.0, 99.0, 0.5)
            .unwrap_err()
            .contains("tolerance"));
        assert!(analyze(&png, Sensitivity::Normal, 52.0, 4.0, 200.0)
            .unwrap_err()
            .contains("min_skin_percent"));
        assert_eq!(
            Sensitivity::parse("medium").unwrap_err(),
            "unknown sensitivity 'medium' (expected 'strict', 'normal' or 'loose')"
        );
    }

    #[test]
    fn empty_and_undecodable_input_error_instead_of_panicking() {
        assert!(analyze_default(b"").unwrap_err().contains("empty"));
        let err = analyze_default(b"this is not an image at all").unwrap_err();
        assert!(err.contains("decode") || err.contains("header"), "{err}");
        // Truncated PNG: a real header, no pixels.
        let png = solid_png(8, 8, [204, 136, 102, 255]);
        let err = analyze_default(&png[..png.len() / 2]).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn a_fully_transparent_image_says_so() {
        let png = solid_png(8, 8, [204, 136, 102, 0]);
        let err = analyze_default(&png).unwrap_err();
        assert!(err.contains("transparent"), "{err}");
    }

    #[test]
    fn a_photo_with_no_skin_at_all_errors_with_the_loose_hint() {
        let png = solid_png(16, 16, [40, 80, 200, 255]);
        let err = analyze_default(&png).unwrap_err();
        assert!(err.contains("no skin-tone pixels found"), "{err}");
        assert!(err.contains("loose"), "{err}");
    }
}
