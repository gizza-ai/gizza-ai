//! css-color-converter core — parse one color in any common CSS or mobile-platform
//! notation and emit every other notation for the SAME color: hex, rgb(a), hsl(a),
//! hwb, oklch, oklab, the CSS named color, and the `0xAARRGGBB` integer form used by
//! Flutter/Dart and Android.
//!
//! Pure Rust, no dependencies beyond serde for the output struct. All conversions
//! are done in f64 and the RGB channels are quantized to 8-bit sRGB once, up front,
//! so every emitted notation describes exactly the same renderable color.

use serde::Serialize;

/// Which CSS function syntax to emit for `rgb`/`hsl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    /// `rgba(52, 152, 219, 0.5)` — comma-separated, the pre-CSS-Color-4 form.
    Legacy,
    /// `rgb(52 152 219 / 0.5)` — space-separated with a slash before alpha.
    Modern,
}

impl Syntax {
    /// Parse the `syntax` parameter value.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "legacy" => Ok(Syntax::Legacy),
            "modern" => Ok(Syntax::Modern),
            other => Err(format!(
                "unknown syntax \"{other}\" (use \"legacy\" or \"modern\")"
            )),
        }
    }
}

/// Output-formatting options.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub syntax: Syntax,
    /// Decimal places kept on fractional components (HSL/HWB percentages,
    /// OKLCH/OKLab components, alpha). Trailing zeros are trimmed.
    pub precision: u32,
    /// Emit hex digits in upper case (`#3498DB`, `0xFF3498DB`).
    pub uppercase_hex: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            syntax: Syntax::Legacy,
            precision: 3,
            uppercase_hex: false,
        }
    }
}

/// Every notation for one parsed color.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Converted {
    /// `#rrggbb` — alpha dropped.
    pub hex: String,
    /// `#rrggbbaa` — CSS 8-digit hex, alpha LAST.
    pub hex_alpha: String,
    /// `rgb(...)` / `rgba(...)`, per the requested syntax.
    pub rgb: String,
    /// `hsl(...)` / `hsla(...)`, per the requested syntax.
    pub hsl: String,
    /// `hwb(h w% b%)` — CSS Color 4 only has the space-separated form.
    pub hwb: String,
    /// `hsv(h, s%, v%)` — HSB in design tools; not CSS, shown for reference.
    pub hsv: String,
    /// `cmyk(c%, m%, y%, k%)` — naive print conversion, not CSS.
    pub cmyk: String,
    /// `lab(l% a b)` — CSS Color 4 CIE Lab, D50 white point.
    pub lab: String,
    /// `lch(l% c h)` — CSS Color 4 CIE LCH, D50 white point.
    pub lch: String,
    /// `oklch(l% c h)` — CSS Color 4 perceptual polar form.
    pub oklch: String,
    /// `oklab(l% a b)` — CSS Color 4 perceptual rectangular form.
    pub oklab: String,
    /// `color(display-p3 r g b)` — the same color in the wider P3 gamut.
    pub display_p3: String,
    /// `0xAARRGGBB` — the Flutter/Dart and Android integer form, alpha FIRST.
    pub flutter_int: String,
    /// Ready-to-paste Dart: `Color(0xAARRGGBB)` (also valid Jetpack Compose).
    pub dart: String,
    /// Android XML color: `#AARRGGBB`, alpha FIRST.
    pub android_argb: String,
    /// Signed 32-bit ARGB integer — what Android/Java `Color` ints hold.
    pub argb_decimal: String,
    /// SwiftUI: `Color(red:green:blue:)` with 0–1 components.
    pub swiftui: String,
    /// WCAG 2.1 contrast against white, e.g. `3.05:1 (AA large)`.
    pub contrast_white: String,
    /// WCAG 2.1 contrast against black, e.g. `6.89:1 (AA)`.
    pub contrast_black: String,
    /// The exact CSS named color, when the color is one (alpha 1 only).
    pub css_name: Option<String>,
    /// The nearest CSS named color by OKLab perceptual distance — always present.
    pub closest_css_name: String,
    /// Alpha 0.0–1.0.
    pub alpha: f64,
    /// True when the input named a color outside the sRGB gamut (an OKLCH/OKLab
    /// input, typically) and the channels were clamped to the nearest sRGB color.
    pub clamped_to_srgb: bool,
}

// ---------------------------------------------------------------------------
// CSS named colors (CSS Color Module Level 4)
// ---------------------------------------------------------------------------

/// The 148 CSS named colors. Order matters for `closest_css_name`: with two names
/// for one value (`gray`/`grey`, `aqua`/`cyan`) the first listed wins, keeping the
/// nearest-name answer deterministic.
const NAMED: &[(&str, u8, u8, u8)] = &[
    ("black", 0, 0, 0),
    ("silver", 192, 192, 192),
    ("gray", 128, 128, 128),
    ("white", 255, 255, 255),
    ("maroon", 128, 0, 0),
    ("red", 255, 0, 0),
    ("purple", 128, 0, 128),
    ("fuchsia", 255, 0, 255),
    ("green", 0, 128, 0),
    ("lime", 0, 255, 0),
    ("olive", 128, 128, 0),
    ("yellow", 255, 255, 0),
    ("navy", 0, 0, 128),
    ("blue", 0, 0, 255),
    ("teal", 0, 128, 128),
    ("aqua", 0, 255, 255),
    ("aliceblue", 240, 248, 255),
    ("antiquewhite", 250, 235, 215),
    ("aquamarine", 127, 255, 212),
    ("azure", 240, 255, 255),
    ("beige", 245, 245, 220),
    ("bisque", 255, 228, 196),
    ("blanchedalmond", 255, 235, 205),
    ("blueviolet", 138, 43, 226),
    ("brown", 165, 42, 42),
    ("burlywood", 222, 184, 135),
    ("cadetblue", 95, 158, 160),
    ("chartreuse", 127, 255, 0),
    ("chocolate", 210, 105, 30),
    ("coral", 255, 127, 80),
    ("cornflowerblue", 100, 149, 237),
    ("cornsilk", 255, 248, 220),
    ("crimson", 220, 20, 60),
    ("cyan", 0, 255, 255),
    ("darkblue", 0, 0, 139),
    ("darkcyan", 0, 139, 139),
    ("darkgoldenrod", 184, 134, 11),
    ("darkgray", 169, 169, 169),
    ("darkgreen", 0, 100, 0),
    ("darkgrey", 169, 169, 169),
    ("darkkhaki", 189, 183, 107),
    ("darkmagenta", 139, 0, 139),
    ("darkolivegreen", 85, 107, 47),
    ("darkorange", 255, 140, 0),
    ("darkorchid", 153, 50, 204),
    ("darkred", 139, 0, 0),
    ("darksalmon", 233, 150, 122),
    ("darkseagreen", 143, 188, 143),
    ("darkslateblue", 72, 61, 139),
    ("darkslategray", 47, 79, 79),
    ("darkslategrey", 47, 79, 79),
    ("darkturquoise", 0, 206, 209),
    ("darkviolet", 148, 0, 211),
    ("deeppink", 255, 20, 147),
    ("deepskyblue", 0, 191, 255),
    ("dimgray", 105, 105, 105),
    ("dimgrey", 105, 105, 105),
    ("dodgerblue", 30, 144, 255),
    ("firebrick", 178, 34, 34),
    ("floralwhite", 255, 250, 240),
    ("forestgreen", 34, 139, 34),
    ("gainsboro", 220, 220, 220),
    ("ghostwhite", 248, 248, 255),
    ("gold", 255, 215, 0),
    ("goldenrod", 218, 165, 32),
    ("greenyellow", 173, 255, 47),
    ("grey", 128, 128, 128),
    ("honeydew", 240, 255, 240),
    ("hotpink", 255, 105, 180),
    ("indianred", 205, 92, 92),
    ("indigo", 75, 0, 130),
    ("ivory", 255, 255, 240),
    ("khaki", 240, 230, 140),
    ("lavender", 230, 230, 250),
    ("lavenderblush", 255, 240, 245),
    ("lawngreen", 124, 252, 0),
    ("lemonchiffon", 255, 250, 205),
    ("lightblue", 173, 216, 230),
    ("lightcoral", 240, 128, 128),
    ("lightcyan", 224, 255, 255),
    ("lightgoldenrodyellow", 250, 250, 210),
    ("lightgray", 211, 211, 211),
    ("lightgreen", 144, 238, 144),
    ("lightgrey", 211, 211, 211),
    ("lightpink", 255, 182, 193),
    ("lightsalmon", 255, 160, 122),
    ("lightseagreen", 32, 178, 170),
    ("lightskyblue", 135, 206, 250),
    ("lightslategray", 119, 136, 153),
    ("lightslategrey", 119, 136, 153),
    ("lightsteelblue", 176, 196, 222),
    ("lightyellow", 255, 255, 224),
    ("limegreen", 50, 205, 50),
    ("linen", 250, 240, 230),
    ("magenta", 255, 0, 255),
    ("mediumaquamarine", 102, 205, 170),
    ("mediumblue", 0, 0, 205),
    ("mediumorchid", 186, 85, 211),
    ("mediumpurple", 147, 112, 219),
    ("mediumseagreen", 60, 179, 113),
    ("mediumslateblue", 123, 104, 238),
    ("mediumspringgreen", 0, 250, 154),
    ("mediumturquoise", 72, 209, 204),
    ("mediumvioletred", 199, 21, 133),
    ("midnightblue", 25, 25, 112),
    ("mintcream", 245, 255, 250),
    ("mistyrose", 255, 228, 225),
    ("moccasin", 255, 228, 181),
    ("navajowhite", 255, 222, 173),
    ("oldlace", 253, 245, 230),
    ("olivedrab", 107, 142, 35),
    ("orange", 255, 165, 0),
    ("orangered", 255, 69, 0),
    ("orchid", 218, 112, 214),
    ("palegoldenrod", 238, 232, 170),
    ("palegreen", 152, 251, 152),
    ("paleturquoise", 175, 238, 238),
    ("palevioletred", 219, 112, 147),
    ("papayawhip", 255, 239, 213),
    ("peachpuff", 255, 218, 185),
    ("peru", 205, 133, 63),
    ("pink", 255, 192, 203),
    ("plum", 221, 160, 221),
    ("powderblue", 176, 224, 230),
    ("rebeccapurple", 102, 51, 153),
    ("rosybrown", 188, 143, 143),
    ("royalblue", 65, 105, 225),
    ("saddlebrown", 139, 69, 19),
    ("salmon", 250, 128, 114),
    ("sandybrown", 244, 164, 96),
    ("seagreen", 46, 139, 87),
    ("seashell", 255, 245, 238),
    ("sienna", 160, 82, 45),
    ("skyblue", 135, 206, 235),
    ("slateblue", 106, 90, 205),
    ("slategray", 112, 128, 144),
    ("slategrey", 112, 128, 144),
    ("snow", 255, 250, 250),
    ("springgreen", 0, 255, 127),
    ("steelblue", 70, 130, 180),
    ("tan", 210, 180, 140),
    ("thistle", 216, 191, 216),
    ("tomato", 255, 99, 71),
    ("turquoise", 64, 224, 208),
    ("violet", 238, 130, 238),
    ("wheat", 245, 222, 179),
    ("whitesmoke", 245, 245, 245),
    ("yellowgreen", 154, 205, 50),
];

fn named_lookup(name: &str) -> Option<(u8, u8, u8)> {
    NAMED
        .iter()
        .find(|(n, ..)| *n == name)
        .map(|(_, r, g, b)| (*r, *g, *b))
}

// ---------------------------------------------------------------------------
// Color space math
// ---------------------------------------------------------------------------

/// sRGB gamma-encoded channel (0–1) → linear-light.
fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear-light channel → sRGB gamma-encoded (0–1).
fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB (0–1 each) → OKLab (Björn Ottosson's matrices).
fn srgb_to_oklab(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let (r, g, b) = (srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b));
    let l = 0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b;
    let m = 0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b;
    let s = 0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b;
    let (l_, m_, s_) = (l.cbrt(), m.cbrt(), s.cbrt());
    (
        0.210_454_255_3 * l_ + 0.793_617_785_0 * m_ - 0.004_072_046_8 * s_,
        1.977_998_495_1 * l_ - 2.428_592_205_0 * m_ + 0.450_593_709_9 * s_,
        0.025_904_037_1 * l_ + 0.782_771_766_2 * m_ - 0.808_675_766_0 * s_,
    )
}

/// OKLab → sRGB (0–1 each, NOT clamped — the caller decides).
fn oklab_to_srgb(l: f64, a: f64, b: f64) -> (f64, f64, f64) {
    let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = l - 0.089_484_177_5 * a - 1.291_485_548_0 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let lr = 4.076_741_662_1 * l3 - 3.307_711_591_3 * m3 + 0.230_969_929_2 * s3;
    let lg = -1.268_438_004_6 * l3 + 2.609_757_401_1 * m3 - 0.341_319_396_5 * s3;
    let lb = -0.004_196_086_3 * l3 - 0.703_418_614_7 * m3 + 1.707_614_701_0 * s3;
    (linear_to_srgb(lr), linear_to_srgb(lg), linear_to_srgb(lb))
}

/// Linear-light sRGB → CIE XYZ, D65 white point.
fn linear_srgb_to_xyz(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    (
        0.412_390_799_3 * r + 0.357_584_339_4 * g + 0.180_480_788_4 * b,
        0.212_639_005_9 * r + 0.715_168_678_8 * g + 0.072_192_315_4 * b,
        0.019_330_818_7 * r + 0.119_194_779_8 * g + 0.950_532_152_2 * b,
    )
}

/// CIE XYZ D65 → CIE Lab with the D50 white point CSS `lab()`/`lch()` use
/// (Bradford-adapted, per CSS Color Module Level 4).
fn xyz_d65_to_lab_d50(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    // Bradford chromatic adaptation D65 → D50.
    let xd = 1.047_929_820_8 * x + 0.022_946_793_3 * y - 0.050_192_229_5 * z;
    let yd = 0.029_627_815_7 * x + 0.990_434_484_6 * y - 0.017_073_825_0 * z;
    let zd = -0.009_243_058_2 * x + 0.015_055_144_9 * y + 0.751_874_290_0 * z;
    // D50 reference white.
    let (wx, wy, wz) = (0.964_295_676_4, 1.0, 0.825_104_602_5);
    const EPSILON: f64 = 216.0 / 24389.0;
    const KAPPA: f64 = 24389.0 / 27.0;
    let f = |t: f64| {
        if t > EPSILON {
            t.cbrt()
        } else {
            (KAPPA * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(xd / wx), f(yd / wy), f(zd / wz));
    (116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))
}

/// CIE XYZ D65 → Display P3 channels (0–1, gamma-encoded).
fn xyz_to_display_p3(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let r = 2.493_496_911_9 * x - 0.931_383_617_9 * y - 0.402_710_784_5 * z;
    let g = -0.829_488_969_6 * x + 1.762_664_060_3 * y + 0.023_624_685_8 * z;
    let b = 0.035_845_830_2 * x - 0.076_172_389_3 * y + 0.956_884_524_0 * z;
    // Display P3 uses the sRGB transfer function.
    (linear_to_srgb(r), linear_to_srgb(g), linear_to_srgb(b))
}

/// sRGB bytes → HSV, hue in degrees, s/v as 0–1.
fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let (rf, gf, bf) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let max = rf.max(gf).max(bf);
    let d = max - rf.min(gf).min(bf);
    let (h, ..) = rgb_to_hsl(r, g, b);
    (h, if max == 0.0 { 0.0 } else { d / max }, max)
}

/// sRGB bytes → naive CMYK (each 0–1). Not colour-managed; print workflows
/// need an ICC profile, so this is the same approximation every web tool shows.
fn rgb_to_cmyk(r: u8, g: u8, b: u8) -> (f64, f64, f64, f64) {
    let (rf, gf, bf) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let k = 1.0 - rf.max(gf).max(bf);
    if (k - 1.0).abs() < 1e-12 {
        return (0.0, 0.0, 0.0, 1.0);
    }
    let f = |c: f64| (1.0 - c - k) / (1.0 - k);
    (f(rf), f(gf), f(bf), k)
}

/// WCAG 2.1 relative luminance of an 8-bit sRGB color.
fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    let l = |c: u8| srgb_to_linear(c as f64 / 255.0);
    0.2126 * l(r) + 0.7152 * l(g) + 0.0722 * l(b)
}

/// `"4.53:1 (AA)"` — the WCAG grade the ratio earns for body text.
fn contrast_line(ratio: f64) -> String {
    let grade = if ratio >= 7.0 {
        "AAA"
    } else if ratio >= 4.5 {
        "AA"
    } else if ratio >= 3.0 {
        "AA large text only"
    } else {
        "fails WCAG"
    };
    format!("{:.2}:1 ({grade})", (ratio * 100.0).floor() / 100.0)
}

/// sRGB bytes → HSL, hue in degrees, s/l as 0–1.
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
        60.0 * ((gf - bf) / d).rem_euclid(6.0)
    } else if max == gf {
        60.0 * ((bf - rf) / d + 2.0)
    } else {
        60.0 * ((rf - gf) / d + 4.0)
    };
    (h.rem_euclid(360.0), s.clamp(0.0, 1.0), l)
}

/// HSL (hue degrees, s/l 0–1) → sRGB 0–1.
fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r1 + m, g1 + m, b1 + m)
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// A parsed color: sRGB 0–1 (possibly out of gamut) plus alpha 0–1.
struct Parsed {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

/// Split a CSS function's arguments on commas, spaces and the alpha slash.
fn components(inner: &str) -> Vec<String> {
    inner
        .split([',', '/', ' ', '\t', '\n'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

/// Strip `name(` … `)` and return the inside.
fn func_args<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let rest = s.strip_prefix(name)?;
    rest.trim_start().strip_prefix('(')?.strip_suffix(')')
}

/// A hue with optional `deg`/`turn`/`rad`/`grad` unit → degrees.
fn parse_hue(x: &str) -> Result<f64, String> {
    let bad = || format!("bad hue \"{x}\"");
    let (num, scale) = if let Some(v) = x.strip_suffix("turn") {
        (v, 360.0)
    } else if let Some(v) = x.strip_suffix("grad") {
        (v, 0.9)
    } else if let Some(v) = x.strip_suffix("rad") {
        (v, 180.0 / std::f64::consts::PI)
    } else if let Some(v) = x.strip_suffix("deg") {
        (v, 1.0)
    } else {
        (x, 1.0)
    };
    let v: f64 = num.trim().parse().map_err(|_| bad())?;
    Ok((v * scale).rem_euclid(360.0))
}

/// A `0–100%` or `0–1` alpha.
fn parse_alpha(x: &str) -> Result<f64, String> {
    let bad = || format!("bad alpha \"{x}\"");
    if let Some(pct) = x.strip_suffix('%') {
        Ok((pct.trim().parse::<f64>().map_err(|_| bad())? / 100.0).clamp(0.0, 1.0))
    } else {
        Ok(x.parse::<f64>().map_err(|_| bad())?.clamp(0.0, 1.0))
    }
}

/// A percentage-or-number component scaled so that `100%` maps to `full`.
fn parse_scaled(x: &str, full: f64, what: &str) -> Result<f64, String> {
    let bad = || format!("bad {what} \"{x}\"");
    if let Some(pct) = x.strip_suffix('%') {
        Ok(pct.trim().parse::<f64>().map_err(|_| bad())? / 100.0 * full)
    } else {
        x.parse::<f64>().map_err(|_| bad())
    }
}

fn parse_hex_digits(h: &str) -> Result<Parsed, String> {
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "\"{h}\" is not a hex color (only 0-9 and a-f are allowed)"
        ));
    }
    let dup = |c: char| -> String { format!("{c}{c}") };
    let cs: Vec<char> = h.chars().collect();
    let (r, g, b, a) = match h.len() {
        3 => (dup(cs[0]), dup(cs[1]), dup(cs[2]), "ff".to_string()),
        4 => (dup(cs[0]), dup(cs[1]), dup(cs[2]), dup(cs[3])),
        6 => (
            h[0..2].into(),
            h[2..4].into(),
            h[4..6].into(),
            "ff".to_string(),
        ),
        8 => (
            h[0..2].into(),
            h[2..4].into(),
            h[4..6].into(),
            h[6..8].into(),
        ),
        n => {
            return Err(format!(
                "hex color has {n} digits — it must have 3, 4, 6, or 8"
            ))
        }
    };
    let byte = |x: &str| u8::from_str_radix(x, 16).unwrap() as f64 / 255.0;
    Ok(Parsed {
        r: byte(&r),
        g: byte(&g),
        b: byte(&b),
        a: byte(&a),
    })
}

/// `0xAARRGGBB` / `0xRRGGBB` — the Flutter/Dart and Android integer form (alpha FIRST).
fn parse_argb_int(h: &str) -> Result<Parsed, String> {
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("\"0x{h}\" is not a hex integer"));
    }
    let (a, r, g, b) = match h.len() {
        8 => (
            h[0..2].into(),
            h[2..4].to_string(),
            h[4..6].to_string(),
            h[6..8].to_string(),
        ),
        6 => (
            "ff".to_string(),
            h[0..2].to_string(),
            h[2..4].to_string(),
            h[4..6].to_string(),
        ),
        n => {
            return Err(format!(
                "0x color has {n} digits — it must have 8 (0xAARRGGBB) or 6 (0xRRGGBB)"
            ))
        }
    };
    let byte = |x: &str| u8::from_str_radix(x, 16).unwrap() as f64 / 255.0;
    Ok(Parsed {
        r: byte(&r),
        g: byte(&g),
        b: byte(&b),
        a: byte(&a),
    })
}

fn parse_rgb_fn(inner: &str) -> Result<Parsed, String> {
    let p = components(inner);
    if p.len() < 3 {
        return Err("rgb() needs 3 channels (r g b), plus an optional alpha".into());
    }
    let chan = |x: &str| -> Result<f64, String> {
        let v = parse_scaled(x, 255.0, "rgb channel")?;
        Ok((v / 255.0).clamp(0.0, 1.0))
    };
    Ok(Parsed {
        r: chan(&p[0])?,
        g: chan(&p[1])?,
        b: chan(&p[2])?,
        a: if p.len() > 3 {
            parse_alpha(&p[3])?
        } else {
            1.0
        },
    })
}

fn parse_hsl_fn(inner: &str) -> Result<Parsed, String> {
    let p = components(inner);
    if p.len() < 3 {
        return Err("hsl() needs a hue plus saturation and lightness percentages".into());
    }
    let h = parse_hue(&p[0])?;
    let s = (parse_scaled(&p[1], 100.0, "saturation")? / 100.0).clamp(0.0, 1.0);
    let l = (parse_scaled(&p[2], 100.0, "lightness")? / 100.0).clamp(0.0, 1.0);
    let (r, g, b) = hsl_to_rgb(h, s, l);
    Ok(Parsed {
        r,
        g,
        b,
        a: if p.len() > 3 {
            parse_alpha(&p[3])?
        } else {
            1.0
        },
    })
}

fn parse_hwb_fn(inner: &str) -> Result<Parsed, String> {
    let p = components(inner);
    if p.len() < 3 {
        return Err("hwb() needs a hue plus whiteness and blackness percentages".into());
    }
    let h = parse_hue(&p[0])?;
    let mut w = (parse_scaled(&p[1], 100.0, "whiteness")? / 100.0).clamp(0.0, 1.0);
    let mut bl = (parse_scaled(&p[2], 100.0, "blackness")? / 100.0).clamp(0.0, 1.0);
    if w + bl > 1.0 {
        // Per CSS Color 4: normalize so the pair sums to 1 (a pure grey).
        let sum = w + bl;
        w /= sum;
        bl /= sum;
    }
    let (r, g, b) = hsl_to_rgb(h, 1.0, 0.5);
    let mix = |c: f64| c * (1.0 - w - bl) + w;
    Ok(Parsed {
        r: mix(r),
        g: mix(g),
        b: mix(b),
        a: if p.len() > 3 {
            parse_alpha(&p[3])?
        } else {
            1.0
        },
    })
}

fn parse_oklch_fn(inner: &str) -> Result<Parsed, String> {
    let p = components(inner);
    if p.len() < 3 {
        return Err("oklch() needs lightness, chroma and hue".into());
    }
    let l = parse_scaled(&p[0], 1.0, "lightness")?.clamp(0.0, 1.0);
    let c = parse_scaled(&p[1], 0.4, "chroma")?.max(0.0);
    let h = parse_hue(&p[2])?.to_radians();
    let (r, g, b) = oklab_to_srgb(l, c * h.cos(), c * h.sin());
    Ok(Parsed {
        r,
        g,
        b,
        a: if p.len() > 3 {
            parse_alpha(&p[3])?
        } else {
            1.0
        },
    })
}

fn parse_oklab_fn(inner: &str) -> Result<Parsed, String> {
    let p = components(inner);
    if p.len() < 3 {
        return Err("oklab() needs lightness plus the a and b axes".into());
    }
    let l = parse_scaled(&p[0], 1.0, "lightness")?.clamp(0.0, 1.0);
    let a = parse_scaled(&p[1], 0.4, "a axis")?;
    let b_axis = parse_scaled(&p[2], 0.4, "b axis")?;
    let (r, g, b) = oklab_to_srgb(l, a, b_axis);
    Ok(Parsed {
        r,
        g,
        b,
        a: if p.len() > 3 {
            parse_alpha(&p[3])?
        } else {
            1.0
        },
    })
}

/// Parse any supported notation into sRGB 0–1 (possibly out of gamut) + alpha.
fn parse(input: &str) -> Result<Parsed, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("no color given".into());
    }
    // `Color(0xFF3498DB)` — accept the Dart snippet people copy out of their code.
    let s = s
        .strip_prefix("Color(")
        .or_else(|| s.strip_prefix("color("))
        .and_then(|r| r.strip_suffix(')'))
        .map(str::trim)
        .filter(|r| r.to_ascii_lowercase().starts_with("0x"))
        .unwrap_or(s);
    let lower = s.to_ascii_lowercase();
    let lower = lower.trim();

    if let Some(h) = lower.strip_prefix("0x") {
        return parse_argb_int(h);
    }
    if let Some(h) = lower.strip_prefix('#') {
        return parse_hex_digits(h);
    }
    for (name, f) in [
        ("rgba", parse_rgb_fn as fn(&str) -> Result<Parsed, String>),
        ("rgb", parse_rgb_fn),
        ("hsla", parse_hsl_fn),
        ("hsl", parse_hsl_fn),
        ("hwb", parse_hwb_fn),
        ("oklch", parse_oklch_fn),
        ("oklab", parse_oklab_fn),
    ] {
        if let Some(inner) = func_args(lower, name) {
            return f(inner);
        }
    }
    if lower == "transparent" {
        return Ok(Parsed {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        });
    }
    if let Some((r, g, b)) = named_lookup(lower) {
        return Ok(Parsed {
            r: r as f64 / 255.0,
            g: g as f64 / 255.0,
            b: b as f64 / 255.0,
            a: 1.0,
        });
    }
    // A bare `52, 152, 219` triple and bare hex digits are both common pastes.
    if lower.contains(',') {
        return parse_rgb_fn(lower);
    }
    if lower.chars().all(|c| c.is_ascii_hexdigit()) {
        return parse_hex_digits(lower);
    }
    Err(format!(
        "could not parse \"{}\" — try #hex, 0xAARRGGBB, rgb(), hsl(), hwb(), oklch(), oklab(), or a CSS color name",
        input.trim()
    ))
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// Round to `precision` decimals and trim trailing zeros (and a bare trailing dot).
fn num(v: f64, precision: u32) -> String {
    let s = format!("{:.*}", precision as usize, v);
    // `-0` reads as a bug in output; normalize it away.
    let s = if s
        .trim_start_matches('-')
        .chars()
        .all(|c| c == '0' || c == '.')
    {
        s.trim_start_matches('-').to_string()
    } else {
        s
    };
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

fn hex2(v: u8, upper: bool) -> String {
    if upper {
        format!("{v:02X}")
    } else {
        format!("{v:02x}")
    }
}

/// Perceptual OKLab distance between two 8-bit sRGB colors.
fn oklab_distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let f = |(r, g, bb): (u8, u8, u8)| {
        srgb_to_oklab(r as f64 / 255.0, g as f64 / 255.0, bb as f64 / 255.0)
    };
    let (l1, a1, b1) = f(a);
    let (l2, a2, b2) = f(b);
    ((l1 - l2).powi(2) + (a1 - a2).powi(2) + (b1 - b2).powi(2)).sqrt()
}

/// Parse a color in any supported notation and render every other notation.
pub fn convert(input: &str, opts: &Options) -> Result<Converted, String> {
    let precision = opts.precision.min(8);
    let p = parse(input)?;
    let clamped_to_srgb = [p.r, p.g, p.b]
        .iter()
        .any(|c| *c < -1e-6 || *c > 1.0 + 1e-6);

    // Quantize the channels once, up front: every notation below then describes
    // exactly the same renderable 8-bit sRGB color. Alpha stays exact.
    let q = |c: f64| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, g, b) = (q(p.r), q(p.g), q(p.b));
    let a = p.a.clamp(0.0, 1.0);
    let a_byte = (a * 255.0).round() as u8;
    let up = opts.uppercase_hex;

    let (rf, gf, bf) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let (h, s, l) = rgb_to_hsl(r, g, b);
    let white = rf.min(gf).min(bf);
    let black = 1.0 - rf.max(gf).max(bf);
    let (ol, oa, ob) = srgb_to_oklab(rf, gf, bf);
    let chroma = (oa * oa + ob * ob).sqrt();
    let ohue = if chroma < 1e-6 {
        0.0
    } else {
        ob.atan2(oa).to_degrees().rem_euclid(360.0)
    };
    let (hsv_h, hsv_s, hsv_v) = rgb_to_hsv(r, g, b);
    let (cy, mg, yl, kk) = rgb_to_cmyk(r, g, b);
    let (x, y, z) = linear_srgb_to_xyz(srgb_to_linear(rf), srgb_to_linear(gf), srgb_to_linear(bf));
    let (ll, la, lb) = xyz_d65_to_lab_d50(x, y, z);
    let lch_c = (la * la + lb * lb).sqrt();
    let lch_h = if lch_c < 1e-6 {
        0.0
    } else {
        lb.atan2(la).to_degrees().rem_euclid(360.0)
    };
    let (p3r, p3g, p3b) = xyz_to_display_p3(x, y, z);
    let lum = relative_luminance(r, g, b);

    let n = |v: f64| num(v, precision);
    // Channels expressed as 0–1 fractions need decimals to mean anything, so
    // they ignore a precision setting below 3.
    let frac = |v: f64| num(v, precision.max(3));
    let alpha_s = n(a);
    let opaque = (a - 1.0).abs() < 1e-9;

    let (rgb, hsl) = match opts.syntax {
        Syntax::Legacy if opaque => (
            format!("rgb({r}, {g}, {b})"),
            format!("hsl({}, {}%, {}%)", n(h), n(s * 100.0), n(l * 100.0)),
        ),
        Syntax::Legacy => (
            format!("rgba({r}, {g}, {b}, {alpha_s})"),
            format!(
                "hsla({}, {}%, {}%, {alpha_s})",
                n(h),
                n(s * 100.0),
                n(l * 100.0)
            ),
        ),
        Syntax::Modern if opaque => (
            format!("rgb({r} {g} {b})"),
            format!("hsl({} {}% {}%)", n(h), n(s * 100.0), n(l * 100.0)),
        ),
        Syntax::Modern => (
            format!("rgb({r} {g} {b} / {alpha_s})"),
            format!(
                "hsl({} {}% {}% / {alpha_s})",
                n(h),
                n(s * 100.0),
                n(l * 100.0)
            ),
        ),
    };
    // hwb()/oklch()/oklab() only exist in the space-separated CSS Color 4 form.
    let tail = if opaque {
        String::new()
    } else {
        format!(" / {alpha_s}")
    };
    let hwb = format!(
        "hwb({} {}% {}%{tail})",
        n(h),
        n(white * 100.0),
        n(black * 100.0)
    );
    let oklch = format!("oklch({}% {} {}{tail})", n(ol * 100.0), n(chroma), n(ohue));
    let oklab = format!("oklab({}% {} {}{tail})", n(ol * 100.0), n(oa), n(ob));
    let lab = format!("lab({}% {} {}{tail})", n(ll), n(la), n(lb));
    let lch = format!("lch({}% {} {}{tail})", n(ll), n(lch_c), n(lch_h));
    let display_p3 = format!(
        "color(display-p3 {} {} {}{tail})",
        frac(p3r),
        frac(p3g),
        frac(p3b)
    );
    // HSV/CMYK are not CSS functions; both are shown in the notation design
    // tools use, comma-separated regardless of the CSS syntax setting.
    let hsv = format!(
        "hsv({}, {}%, {}%)",
        n(hsv_h),
        n(hsv_s * 100.0),
        n(hsv_v * 100.0)
    );
    let cmyk = format!(
        "cmyk({}%, {}%, {}%, {}%)",
        n(cy * 100.0),
        n(mg * 100.0),
        n(yl * 100.0),
        n(kk * 100.0)
    );
    let swiftui = if opaque {
        format!(
            "Color(red: {}, green: {}, blue: {})",
            frac(rf),
            frac(gf),
            frac(bf)
        )
    } else {
        format!(
            "Color(red: {}, green: {}, blue: {}, opacity: {alpha_s})",
            frac(rf),
            frac(gf),
            frac(bf)
        )
    };

    let hx = |v: u8| hex2(v, up);
    let hex = format!("#{}{}{}", hx(r), hx(g), hx(b));
    let hex_alpha = format!("#{}{}{}{}", hx(r), hx(g), hx(b), hx(a_byte));
    let argb = format!("{}{}{}{}", hx(a_byte), hx(r), hx(g), hx(b));
    let flutter_int = format!("0x{argb}");

    let css_name = if a_byte == 0 && (r, g, b) == (0, 0, 0) {
        Some("transparent".to_string())
    } else if opaque {
        NAMED
            .iter()
            .find(|(_, nr, ng, nb)| (*nr, *ng, *nb) == (r, g, b))
            .map(|(name, ..)| name.to_string())
    } else {
        None
    };
    let closest_css_name = NAMED
        .iter()
        .min_by(|x, y| {
            oklab_distance((r, g, b), (x.1, x.2, x.3))
                .total_cmp(&oklab_distance((r, g, b), (y.1, y.2, y.3)))
        })
        .map(|(name, ..)| name.to_string())
        .unwrap_or_default();

    let argb_u32 = ((a_byte as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32;

    Ok(Converted {
        hex,
        hex_alpha,
        rgb,
        hsl,
        hwb,
        hsv,
        cmyk,
        lab,
        lch,
        oklch,
        oklab,
        display_p3,
        flutter_int,
        dart: format!("Color(0x{argb})"),
        android_argb: format!("#{argb}"),
        argb_decimal: (argb_u32 as i32).to_string(),
        swiftui,
        contrast_white: contrast_line(1.05 / (lum + 0.05)),
        contrast_black: contrast_line((lum + 0.05) / 0.05),
        css_name,
        closest_css_name,
        alpha: a,
        clamped_to_srgb,
    })
}

/// The plain-text block the page and CLI show: one notation per line, grouped
/// by where you would paste it.
pub fn render_text(c: &Converted) -> String {
    let name_row = match &c.css_name {
        Some(name) => ("CSS name", name.clone()),
        None => ("Closest name", c.closest_css_name.clone()),
    };
    let rows: Vec<(&str, &str)> = vec![
        ("CSS", ""),
        ("HEX", &c.hex),
        ("HEX + alpha", &c.hex_alpha),
        ("RGB", &c.rgb),
        ("HSL", &c.hsl),
        ("HWB", &c.hwb),
        ("LAB", &c.lab),
        ("LCH", &c.lch),
        ("OKLCH", &c.oklch),
        ("OKLab", &c.oklab),
        ("Display P3", &c.display_p3),
        (name_row.0, &name_row.1),
        ("", ""),
        ("Design", ""),
        ("HSV / HSB", &c.hsv),
        ("CMYK", &c.cmyk),
        ("", ""),
        ("App code", ""),
        ("Flutter / Dart", &c.dart),
        ("Jetpack Compose", &c.dart),
        ("SwiftUI", &c.swiftui),
        ("Android XML", &c.android_argb),
        ("ARGB int", &c.argb_decimal),
        ("", ""),
        ("Contrast (WCAG 2.1)", ""),
        ("On white", &c.contrast_white),
        ("On black", &c.contrast_black),
    ];
    let mut out = String::new();
    for (label, value) in rows {
        if value.is_empty() {
            // A group heading, or the blank line before one.
            out.push_str(label);
        } else {
            out.push_str(&format!("  {label:<20}{value}"));
        }
        out.push('\n');
    }
    if c.clamped_to_srgb {
        out.push_str("\nNote: that color is outside the sRGB gamut — the values above are the nearest sRGB color.\n");
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(input: &str) -> Converted {
        convert(input, &Options::default()).unwrap()
    }

    #[test]
    fn hex_converts_to_every_notation() {
        let c = conv("#3498db");
        assert_eq!(c.hex, "#3498db");
        assert_eq!(c.hex_alpha, "#3498dbff");
        assert_eq!(c.rgb, "rgb(52, 152, 219)");
        assert_eq!(c.hsl, "hsl(204.072, 69.874%, 53.137%)");
        assert_eq!(c.hwb, "hwb(204.072 20.392% 14.118%)");
        assert_eq!(c.oklch, "oklch(65.309% 0.135 242.687)");
        assert_eq!(c.oklab, "oklab(65.309% -0.062 -0.12)");
        assert_eq!(c.flutter_int, "0xff3498db");
        assert_eq!(c.dart, "Color(0xff3498db)");
        assert_eq!(c.android_argb, "#ff3498db");
        assert_eq!(c.alpha, 1.0);
        assert!(!c.clamped_to_srgb);
    }

    #[test]
    fn short_and_long_hex_agree() {
        assert_eq!(conv("#f00").hex, conv("#ff0000").hex);
        assert_eq!(conv("f00").hex, "#ff0000");
        assert_eq!(conv("#F00").rgb, "rgb(255, 0, 0)");
    }

    #[test]
    fn flutter_int_round_trips() {
        // 0xAARRGGBB has alpha FIRST; the CSS 8-digit hex has it LAST.
        let c = conv("0xFF6750A4");
        assert_eq!(c.hex, "#6750a4");
        assert_eq!(c.hex_alpha, "#6750a4ff");
        assert_eq!(c.flutter_int, "0xff6750a4");
        // Half-transparent: 0x80 alpha.
        let c = conv("0x806750A4");
        assert_eq!(c.hex_alpha, "#6750a480");
        assert_eq!(c.rgb, "rgba(103, 80, 164, 0.502)");
        // The Dart snippet form parses too.
        assert_eq!(conv("Color(0xFF6750A4)").hex, "#6750a4");
    }

    #[test]
    fn oklch_round_trips_through_hex() {
        // hex → oklch → hex must land back on the same 8-bit color.
        let precise = convert(
            "#3498db",
            &Options {
                precision: 6,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(conv(&precise.oklch).hex, "#3498db");
        assert_eq!(conv(&precise.oklab).hex, "#3498db");
        // CSS allows lightness as either a 0–1 number or a percentage.
        assert_eq!(
            conv("oklch(0.65309 0.135 242.687)").hex,
            conv("oklch(65.309% 0.135 242.687)").hex
        );
    }

    #[test]
    fn named_colors_both_directions() {
        let c = conv("rebeccapurple");
        assert_eq!(c.hex, "#663399");
        assert_eq!(c.css_name.as_deref(), Some("rebeccapurple"));
        // Not a named color → nearest by OKLab distance, no exact name.
        let c = conv("#3498db");
        assert_eq!(c.css_name, None);
        assert_eq!(c.closest_css_name, "cornflowerblue");
        assert_eq!(conv("transparent").css_name.as_deref(), Some("transparent"));
    }

    #[test]
    fn alpha_and_modern_syntax() {
        let opts = Options {
            syntax: Syntax::Modern,
            ..Options::default()
        };
        let c = convert("rgba(52, 152, 219, 0.5)", &opts).unwrap();
        assert_eq!(c.rgb, "rgb(52 152 219 / 0.5)");
        assert_eq!(c.hsl, "hsl(204.072 69.874% 53.137% / 0.5)");
        assert_eq!(c.hex_alpha, "#3498db80");
        // Modern space+slash input parses identically to the legacy comma form.
        assert_eq!(
            convert("rgb(52 152 219 / 50%)", &opts).unwrap().hex_alpha,
            "#3498db80"
        );
    }

    #[test]
    fn precision_and_uppercase_are_honored() {
        let opts = Options {
            precision: 0,
            uppercase_hex: true,
            ..Options::default()
        };
        let c = convert("#3498db", &opts).unwrap();
        assert_eq!(c.hex, "#3498DB");
        assert_eq!(c.flutter_int, "0xFF3498DB");
        assert_eq!(c.hsl, "hsl(204, 70%, 53%)");
    }

    #[test]
    fn out_of_gamut_oklch_is_clamped_and_flagged() {
        // Chroma 0.35 at hue 150 is well outside sRGB.
        let c = conv("oklch(85% 0.35 150)");
        assert!(c.clamped_to_srgb, "expected the out-of-gamut flag");
        assert!(!conv("oklch(64.756% 0.113 245.79)").clamped_to_srgb);
    }

    #[test]
    fn hsl_hwb_and_bare_triples_parse() {
        assert_eq!(conv("hsl(204, 70%, 53%)").hex, "#3398db");
        assert_eq!(conv("hwb(0 0% 0%)").hex, "#ff0000");
        assert_eq!(conv("hwb(0 50% 0%)").hex, "#ff8080");
        assert_eq!(conv("52, 152, 219").hex, "#3498db");
        // Hue units.
        assert_eq!(
            conv("hsl(0.5turn, 100%, 50%)").hex,
            conv("hsl(180, 100%, 50%)").hex
        );
    }

    #[test]
    fn rejects_unparseable_input() {
        let err = convert("not a color", &Options::default()).unwrap_err();
        assert!(err.contains("could not parse"), "unexpected error: {err}");
        assert!(convert("", &Options::default()).is_err());
        assert!(convert("#12345", &Options::default())
            .unwrap_err()
            .contains("3, 4, 6, or 8"));
        assert!(convert("0x12345", &Options::default())
            .unwrap_err()
            .contains("8 (0xAARRGGBB)"));
        assert!(Syntax::parse("bogus").is_err());
    }

    #[test]
    fn print_and_cie_spaces_are_emitted() {
        let c = conv("#3498db");
        assert_eq!(c.hsv, "hsv(204.072, 76.256%, 85.882%)");
        assert_eq!(c.cmyk, "cmyk(76.256%, 30.594%, 0%, 14.118%)");
        assert_eq!(c.lab, "lab(59.496% -12.103 -43.135)");
        assert_eq!(c.lch, "lch(59.496% 44.801 254.326)");
        assert_eq!(c.display_p3, "color(display-p3 0.321 0.588 0.837)");
        // Pure black/white are the CMYK and contrast edge cases.
        assert_eq!(conv("#000").cmyk, "cmyk(0%, 0%, 0%, 100%)");
        assert_eq!(conv("#fff").lab, "lab(100% 0 0)");
    }

    #[test]
    fn app_code_and_contrast_lines() {
        let c = conv("#3498db");
        assert_eq!(c.swiftui, "Color(red: 0.204, green: 0.596, blue: 0.859)");
        assert_eq!(c.argb_decimal, "-13330213");
        assert_eq!(c.contrast_white, "3.15:1 (AA large text only)");
        assert_eq!(c.contrast_black, "6.66:1 (AA)");
        // Extremes get the extreme grades.
        assert_eq!(conv("#000").contrast_white, "21.00:1 (AAA)");
        assert_eq!(conv("#fff").contrast_white, "1.00:1 (fails WCAG)");
        // Alpha reaches SwiftUI as an opacity argument.
        assert_eq!(
            conv("#3498db80").swiftui,
            "Color(red: 0.204, green: 0.596, blue: 0.859, opacity: 0.502)"
        );
    }

    #[test]
    fn render_text_lists_every_notation() {
        let t = render_text(&conv("#3498db"));
        for want in [
            "HEX ",
            "RGB ",
            "HSL ",
            "HWB ",
            "LAB ",
            "LCH ",
            "OKLCH ",
            "OKLab ",
            "Display P3 ",
            "HSV / HSB ",
            "CMYK ",
            "Flutter / Dart ",
            "Jetpack Compose ",
            "SwiftUI ",
            "Android XML ",
            "ARGB int ",
            "On white ",
            "On black ",
        ] {
            assert!(t.contains(want), "missing {want} in:\n{t}");
        }
        assert!(
            t.contains("Closest name        cornflowerblue"),
            "got:\n{t}"
        );
        assert!(render_text(&conv("oklch(85% 0.35 150)")).contains("outside the sRGB gamut"));
    }
}
