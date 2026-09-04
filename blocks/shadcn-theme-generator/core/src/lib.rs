//! shadcn-theme-generator core — turn one or two seed colors into a complete
//! shadcn/ui CSS-variable theme (light `:root` + dark `.dark`), ready to paste
//! into `globals.css`.
//!
//! Pure Rust, dependency-free besides serde for the output struct. All color
//! math runs through OKLab/OKLCH in f64 so lightness moves perceptually and the
//! light/dark pair stays visually matched; every emitted notation is quantized
//! from the SAME 8-bit sRGB triple, so hex/hsl/oklch describe one renderable
//! color.
//!
//! Foreground tokens are not guessed: for each surface the near-white and
//! near-black neutrals are scored with the WCAG 2.x contrast formula and the
//! higher-contrast one wins; when neither ladder end clears AA the token walks
//! on to the ladder's pure extreme, which always does. Every pair's ratio is
//! reported back so the AA claim is checkable rather than asserted — including
//! `ring on background`, which is brand-coloured and therefore reported rather
//! than repainted.

use serde::Serialize;

// ---------------------------------------------------------------------------
// Public output shape
// ---------------------------------------------------------------------------

/// One CSS variable, with its light-mode and dark-mode value.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Token {
    /// Variable name without the leading dashes, e.g. `"primary-foreground"`.
    pub name: String,
    /// The value emitted inside `:root` (absent when `mode = "dark"`).
    pub light: Option<String>,
    /// The value emitted inside `.dark` (absent when `mode = "light"`).
    pub dark: Option<String>,
}

/// A measured foreground/background contrast pair.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Contrast {
    /// Which pair, e.g. `"primary-foreground on primary"`.
    pub pair: String,
    /// `"light"` or `"dark"`.
    pub mode: String,
    /// WCAG 2.x contrast ratio, rounded to 2 decimals (1.0–21.0).
    pub ratio: f64,
    /// The AA minimum this pair is held to: 4.5 for text, 3.0 for the focus
    /// ring and other non-text UI (WCAG 2.x SC 1.4.11).
    pub minimum: f64,
    /// True when `ratio >= minimum`.
    pub passes_aa: bool,
}

/// The generated theme.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Theme {
    /// The primary seed, normalized to `#rrggbb`.
    pub primary: String,
    /// The accent seed normalized to `#rrggbb`, or `"neutral"` when no accent
    /// seed was given (upstream shadcn uses a neutral accent by default).
    pub accent: String,
    /// The neutral family that tinted the greys.
    pub neutral: String,
    /// The notation the values are written in.
    pub format: String,
    /// `"v4"` or `"v3"`.
    pub tailwind: String,
    /// The `--radius` value as emitted, e.g. `"0.625rem"`.
    pub radius: String,
    /// `"both"`, `"light"` or `"dark"`.
    pub mode: String,
    /// Every generated variable.
    pub tokens: Vec<Token>,
    /// The finished stylesheet, ready to paste into `globals.css`.
    pub css: String,
    /// Measured contrast for each checked pair.
    pub contrast: Vec<Contrast>,
    /// Human-readable notes — one per pair that misses its WCAG AA minimum.
    pub warnings: Vec<String>,
}

/// Largest accepted `--radius`, in rem. Above roughly 2rem every shadcn control
/// is already fully rounded, so the cap is where the token stops meaning
/// anything rather than an arbitrary limit.
pub const MAX_RADIUS_REM: f64 = 2.0;

/// WCAG 2.x AA threshold for normal-size text.
const AA: f64 = 4.5;

/// WCAG 2.x AA threshold for non-text UI — focus rings, control boundaries
/// (SC 1.4.11).
const AA_NON_TEXT: f64 = 3.0;

// ---------------------------------------------------------------------------
// Color plumbing: sRGB <-> linear <-> OKLab <-> OKLCH
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

/// Lightness 0–1, chroma, hue in degrees.
#[derive(Debug, Clone, Copy)]
struct Oklch {
    l: f64,
    c: f64,
    h: f64,
}

fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f64) -> f64 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

fn to_oklch(c: Rgb) -> Oklch {
    let r = srgb_to_linear(c.r as f64 / 255.0);
    let g = srgb_to_linear(c.g as f64 / 255.0);
    let b = srgb_to_linear(c.b as f64 / 255.0);

    let l_ = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m_ = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s_ = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();

    let l = 0.210_454_255_3 * l_ + 0.793_617_785_0 * m_ - 0.004_072_046_8 * s_;
    let a = 1.977_998_495_1 * l_ - 2.428_592_205_0 * m_ + 0.450_593_709_9 * s_;
    let bb = 0.025_904_037_1 * l_ + 0.782_771_766_2 * m_ - 0.808_675_766_0 * s_;

    let chroma = (a * a + bb * bb).sqrt();
    let mut hue = bb.atan2(a).to_degrees();
    if hue < 0.0 {
        hue += 360.0;
    }
    Oklch {
        l,
        c: chroma,
        h: hue,
    }
}

fn from_oklch(o: Oklch) -> Rgb {
    let h = o.h.to_radians();
    let a = o.c * h.cos();
    let b = o.c * h.sin();

    let l_ = (o.l + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m_ = (o.l - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s_ = (o.l - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);

    let lr = 4.076_741_662_1 * l_ - 3.307_711_591_3 * m_ + 0.230_969_929_2 * s_;
    let lg = -1.268_438_004_6 * l_ + 2.609_757_401_1 * m_ - 0.341_319_396_5 * s_;
    let lb = -0.004_196_086_3 * l_ - 0.703_418_614_7 * m_ + 1.707_614_701_0 * s_;

    Rgb {
        r: (clamp01(linear_to_srgb(lr)) * 255.0).round() as u8,
        g: (clamp01(linear_to_srgb(lg)) * 255.0).round() as u8,
        b: (clamp01(linear_to_srgb(lb)) * 255.0).round() as u8,
    }
}

/// WCAG 2.x relative luminance.
fn luminance(c: Rgb) -> f64 {
    0.2126 * srgb_to_linear(c.r as f64 / 255.0)
        + 0.7152 * srgb_to_linear(c.g as f64 / 255.0)
        + 0.0722 * srgb_to_linear(c.b as f64 / 255.0)
}

/// WCAG 2.x contrast ratio between two colors (1.0–21.0).
fn contrast_ratio(a: Rgb, b: Rgb) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    let (hi, lo) = if x > y { (x, y) } else { (y, x) };
    (hi + 0.05) / (lo + 0.05)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

// ---------------------------------------------------------------------------
// Input parsing
// ---------------------------------------------------------------------------

fn hex_pair(s: &str) -> Option<u8> {
    u8::from_str_radix(s, 16).ok()
}

/// Parse a color in any notation the tool advertises: `#rgb`, `#rrggbb`, bare
/// hex, `rgb(r, g, b)` / `rgb(r g b)`, and `hsl(h, s%, l%)`.
fn parse_color(input: &str) -> Result<Rgb, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("no color given — pass a seed like \"#6366f1\"".into());
    }
    let lower = s.to_ascii_lowercase();

    if let Some(rest) = lower.strip_prefix("rgb") {
        let inner = rest
            .trim_start_matches('a')
            .trim()
            .trim_start_matches('(')
            .trim_end_matches(')');
        let parts: Vec<f64> = inner
            .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
            .filter(|p| !p.is_empty())
            .filter_map(|p| p.trim().trim_end_matches('%').parse::<f64>().ok())
            .collect();
        if parts.len() < 3 {
            return Err(format!(
                "could not read \"{s}\" as an rgb() color — expected rgb(99, 102, 241)"
            ));
        }
        return Ok(Rgb {
            r: parts[0].clamp(0.0, 255.0).round() as u8,
            g: parts[1].clamp(0.0, 255.0).round() as u8,
            b: parts[2].clamp(0.0, 255.0).round() as u8,
        });
    }

    if let Some(rest) = lower.strip_prefix("hsl") {
        let inner = rest
            .trim_start_matches('a')
            .trim()
            .trim_start_matches('(')
            .trim_end_matches(')');
        let parts: Vec<f64> = inner
            .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
            .filter(|p| !p.is_empty())
            .filter_map(|p| {
                p.trim()
                    .trim_end_matches('%')
                    .trim_end_matches("deg")
                    .parse::<f64>()
                    .ok()
            })
            .collect();
        if parts.len() < 3 {
            return Err(format!(
                "could not read \"{s}\" as an hsl() color — expected hsl(239, 84%, 67%)"
            ));
        }
        return Ok(hsl_to_rgb(parts[0], parts[1] / 100.0, parts[2] / 100.0));
    }

    let hex = lower.strip_prefix('#').unwrap_or(&lower);
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "could not read \"{s}\" as a color — use #6366f1, rgb(99, 102, 241) or hsl(239, 84%, 67%)"
        ));
    }
    match hex.len() {
        3 => {
            let d: Vec<char> = hex.chars().collect();
            Ok(Rgb {
                r: hex_pair(&format!("{}{}", d[0], d[0])).unwrap_or(0),
                g: hex_pair(&format!("{}{}", d[1], d[1])).unwrap_or(0),
                b: hex_pair(&format!("{}{}", d[2], d[2])).unwrap_or(0),
            })
        }
        6 => Ok(Rgb {
            r: hex_pair(&hex[0..2]).unwrap_or(0),
            g: hex_pair(&hex[2..4]).unwrap_or(0),
            b: hex_pair(&hex[4..6]).unwrap_or(0),
        }),
        n => Err(format!(
            "hex color \"{s}\" has {n} digits — use 3 (#abc) or 6 (#aabbcc)"
        )),
    }
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> Rgb {
    let h = ((h % 360.0) + 360.0) % 360.0;
    let s = clamp01(s);
    let l = clamp01(l);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match h as u32 / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Rgb {
        r: ((r + m) * 255.0).round() as u8,
        g: ((g + m) * 255.0).round() as u8,
        b: ((b + m) * 255.0).round() as u8,
    }
}

// ---------------------------------------------------------------------------
// Output notations
// ---------------------------------------------------------------------------

/// Which notation the CSS values are written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// `oklch(0.585 0.233 277.12)` — the current shadcn/Tailwind v4 default.
    Oklch,
    /// `hsl(239 84% 67%)`, or a bare `239 84% 67%` triplet under Tailwind v3.
    Hsl,
    /// `#6366f1`.
    Hex,
}

impl Format {
    /// Parse the `format` parameter value.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "oklch" => Ok(Format::Oklch),
            "hsl" => Ok(Format::Hsl),
            "hex" => Ok(Format::Hex),
            other => Err(format!(
                "unknown format \"{other}\" (use \"oklch\", \"hsl\" or \"hex\")"
            )),
        }
    }
}

/// Which Tailwind era the stylesheet targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tailwind {
    /// Tailwind v4: plain `:root`/`.dark` blocks plus an `@theme inline` map.
    V4,
    /// Tailwind v3: `@layer base` with bare `H S% L%` triplets read through
    /// `hsl(var(--token))`.
    V3,
}

impl Tailwind {
    /// Parse the `tailwind` parameter value.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "v4" | "4" => Ok(Tailwind::V4),
            "v3" | "3" => Ok(Tailwind::V3),
            other => Err(format!(
                "unknown tailwind version \"{other}\" (use \"v4\" or \"v3\")"
            )),
        }
    }
}

/// Which blocks to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Both,
    Light,
    Dark,
}

impl Mode {
    /// Parse the `mode` parameter value.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "both" => Ok(Mode::Both),
            "light" => Ok(Mode::Light),
            "dark" => Ok(Mode::Dark),
            other => Err(format!(
                "unknown mode \"{other}\" (use \"both\", \"light\" or \"dark\")"
            )),
        }
    }
    fn has_light(self) -> bool {
        self != Mode::Dark
    }
    fn has_dark(self) -> bool {
        self != Mode::Light
    }
}

/// A grey family. shadcn ships five; pinning one would be wrong for four out of
/// five projects, so it is a parameter. Each is a (hue, peak chroma) tint
/// applied to the neutral ladder, taken from where the matching Tailwind family
/// is most saturated — its 500 step, which is also where shadcn's
/// `--muted-foreground` sits.
fn neutral_tint(name: &str) -> Result<(f64, f64), String> {
    match name.trim().to_ascii_lowercase().as_str() {
        "slate" => Ok((257.42, 0.046)),
        "gray" | "grey" => Ok((264.36, 0.027)),
        "zinc" => Ok((286.0, 0.016)),
        "neutral" => Ok((0.0, 0.0)),
        "stone" => Ok((58.07, 0.013)),
        other => Err(format!(
            "unknown neutral \"{other}\" (use \"slate\", \"gray\", \"zinc\", \"neutral\" or \"stone\")"
        )),
    }
}

fn write_color(c: Rgb, f: Format, tw: Tailwind) -> String {
    match f {
        Format::Hex => format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b),
        Format::Oklch => {
            let o = to_oklch(c);
            // Achromatic colors get a 0 hue so the value stays stable and short.
            let (chroma, hue) = if o.c < 0.0005 {
                (0.0, 0.0)
            } else {
                (round3(o.c), round2(o.h))
            };
            format!("oklch({} {} {})", round3(o.l), chroma, hue)
        }
        Format::Hsl => {
            let (h, s, l) = rgb_to_hsl(c);
            let body = format!("{} {}% {}%", h.round(), (s * 100.0).round(), (l * 100.0).round());
            match tw {
                // v3 consumes the token through hsl(var(--token)), so the
                // variable itself must hold a bare triplet.
                Tailwind::V3 => body,
                Tailwind::V4 => format!("hsl({body})"),
            }
        }
    }
}

fn rgb_to_hsl(c: Rgb) -> (f64, f64, f64) {
    let (r, g, b) = (c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d.abs() < f64::EPSILON {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if (max - r).abs() < f64::EPSILON {
        60.0 * (((g - b) / d) % 6.0)
    } else if (max - g).abs() < f64::EPSILON {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    ((h + 360.0) % 360.0, s, l)
}

// ---------------------------------------------------------------------------
// Theme derivation
// ---------------------------------------------------------------------------

/// Everything the generator needs, already parsed.
struct Plan {
    primary: Rgb,
    accent_seed: Option<Rgb>,
    hue: f64,
    chroma: f64,
    format: Format,
    tailwind: Tailwind,
    charts: bool,
    sidebar: bool,
}

impl Plan {
    /// A neutral surface at lightness `l`. Chroma follows the same arc a real
    /// Tailwind grey does: strongest in the mid steps, fading to nothing at both
    /// ends, so the lightest surfaces stay clean white and the tint is still
    /// visible where there is room for it.
    fn grey(&self, l: f64) -> Rgb {
        let fade = (4.0 * l * (1.0 - l)).clamp(0.0, 1.0);
        from_oklch(Oklch {
            l,
            c: self.chroma * fade,
            h: self.hue,
        })
    }

    /// Muted text is deliberately quieter than `foreground`, but "quiet" must
    /// not mean unreadable: start at shadcn's step and keep stepping away from
    /// the surface until the pair clears AA.
    fn quiet_foreground(&self, surface: Rgb, start_l: f64, darker: bool) -> Rgb {
        let step = if darker { -0.008 } else { 0.008 };
        let mut l = start_l;
        while (0.0..=1.0).contains(&l) {
            let c = self.grey(l);
            if contrast_ratio(c, surface) >= AA {
                return c;
            }
            l += step;
        }
        self.grey(if darker { 0.0 } else { 1.0 })
    }
}

/// Pick the foreground for a surface by measurement. First choice is the
/// near-white / near-black end of the neutral ladder, whichever scores higher —
/// that is the shadcn look. When neither end clears AA (a mid-tone seed such as
/// `#6366f1` sits right in that gap) the token walks on to the end of the
/// ladder, pure white or pure black. Every colour has at least 4.58:1 against
/// one of those, so the escalation always lands a passing pair.
fn foreground_for(surface: Rgb, light_end: Rgb, dark_end: Rgb) -> Rgb {
    let better = |a: Rgb, b: Rgb| {
        if contrast_ratio(surface, a) >= contrast_ratio(surface, b) {
            a
        } else {
            b
        }
    };
    let ladder = better(light_end, dark_end);
    if contrast_ratio(surface, ladder) >= AA {
        return ladder;
    }
    better(WHITE, BLACK)
}

const WHITE: Rgb = Rgb {
    r: 255,
    g: 255,
    b: 255,
};
const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };

/// Lift a seed color so it still reads against a dark surface. Colors that are
/// already light are left alone; dark seeds are raised into the band where
/// shadcn's dark themes sit.
fn lift_for_dark(c: Rgb) -> Rgb {
    let o = to_oklch(c);
    let l = if o.l < 0.62 {
        (o.l * 0.45 + 0.40).clamp(0.62, 0.86)
    } else {
        o.l.min(0.88)
    };
    from_oklch(Oklch { l, ..o })
}

/// Move a seed color to a target lightness, keeping hue and easing chroma so
/// very light tints don't turn neon.
fn at_lightness(c: Rgb, l: f64) -> Rgb {
    let o = to_oklch(c);
    let squeeze = if l > 0.85 {
        0.22
    } else if l < 0.30 {
        0.55
    } else {
        1.0
    };
    from_oklch(Oklch {
        l,
        c: o.c * squeeze,
        h: o.h,
    })
}

/// Rotate a hue by `deg`, wrapping into 0–360.
fn rotate(c: Rgb, deg: f64, l: f64, chroma: f64) -> Rgb {
    let o = to_oklch(c);
    from_oklch(Oklch {
        l,
        c: chroma,
        h: (o.h + deg).rem_euclid(360.0),
    })
}

/// Destructive stays a red regardless of the seed — a brand-tinted "delete" is
/// a usability bug, not a feature. These are the upstream light/dark reds.
const DESTRUCTIVE_LIGHT: Rgb = Rgb {
    r: 220,
    g: 38,
    b: 38,
};
const DESTRUCTIVE_DARK: Rgb = Rgb {
    r: 248,
    g: 113,
    b: 113,
};

/// Build every token as a light/dark pair of concrete colors, plus the
/// (label, mode, foreground, background, AA minimum) pairs to measure.
#[allow(clippy::type_complexity)]
fn derive(plan: &Plan) -> (Vec<(String, Rgb, Rgb)>, Vec<(String, String, Rgb, Rgb, f64)>) {
    let light_end = plan.grey(0.985);
    let dark_end = plan.grey(0.145);

    // Light-mode surfaces.
    let l_bg = plan.grey(1.0);
    let l_fg = plan.grey(0.145);
    let l_card = l_bg;
    let l_muted = plan.grey(0.968);
    let l_muted_fg = plan.quiet_foreground(l_muted, 0.556, true);
    let l_secondary = plan.grey(0.968);
    let l_border = plan.grey(0.922);
    let l_sidebar = plan.grey(0.985);

    // Dark-mode surfaces.
    let d_bg = plan.grey(0.145);
    let d_fg = plan.grey(0.985);
    let d_card = plan.grey(0.205);
    let d_muted = plan.grey(0.269);
    let d_muted_fg = plan.quiet_foreground(d_muted, 0.708, false);
    let d_secondary = plan.grey(0.269);
    let d_border = plan.grey(0.30);
    let d_sidebar = plan.grey(0.205);

    // Primary keeps the seed exactly in light mode; only the dark block lifts it.
    let l_primary = plan.primary;
    let d_primary = lift_for_dark(plan.primary);

    // Accent: a tint of the second seed when given, otherwise the neutral
    // surface upstream uses.
    let (l_accent, d_accent) = match plan.accent_seed {
        Some(a) => (at_lightness(a, 0.94), at_lightness(a, 0.32)),
        None => (plan.grey(0.968), plan.grey(0.269)),
    };

    // The focus ring is the brand colour itself, the way shadcn's own coloured
    // themes set it — and it is the one token the generator will not repaint to
    // hit a threshold, so a ring too faint to see is reported instead.
    let l_ring = l_primary;
    let d_ring = d_primary;

    let mut tokens: Vec<(String, Rgb, Rgb)> = vec![
        ("background".into(), l_bg, d_bg),
        ("foreground".into(), l_fg, d_fg),
        ("card".into(), l_card, d_card),
        (
            "card-foreground".into(),
            foreground_for(l_card, light_end, dark_end),
            foreground_for(d_card, light_end, dark_end),
        ),
        ("popover".into(), l_card, d_card),
        (
            "popover-foreground".into(),
            foreground_for(l_card, light_end, dark_end),
            foreground_for(d_card, light_end, dark_end),
        ),
        ("primary".into(), l_primary, d_primary),
        (
            "primary-foreground".into(),
            foreground_for(l_primary, light_end, dark_end),
            foreground_for(d_primary, light_end, dark_end),
        ),
        ("secondary".into(), l_secondary, d_secondary),
        (
            "secondary-foreground".into(),
            foreground_for(l_secondary, light_end, dark_end),
            foreground_for(d_secondary, light_end, dark_end),
        ),
        ("muted".into(), l_muted, d_muted),
        ("muted-foreground".into(), l_muted_fg, d_muted_fg),
        ("accent".into(), l_accent, d_accent),
        (
            "accent-foreground".into(),
            foreground_for(l_accent, light_end, dark_end),
            foreground_for(d_accent, light_end, dark_end),
        ),
        ("destructive".into(), DESTRUCTIVE_LIGHT, DESTRUCTIVE_DARK),
        (
            "destructive-foreground".into(),
            foreground_for(DESTRUCTIVE_LIGHT, light_end, dark_end),
            foreground_for(DESTRUCTIVE_DARK, light_end, dark_end),
        ),
        ("border".into(), l_border, d_border),
        ("input".into(), l_border, d_border),
        ("ring".into(), l_ring, d_ring),
    ];

    if plan.charts {
        // Five hues fanned out from the seed: near, adjacent, complementary and
        // two in between, so a chart legend stays distinguishable.
        for (i, deg) in [0.0, 40.0, 200.0, 260.0, 100.0].into_iter().enumerate() {
            let chroma = 0.14;
            tokens.push((
                format!("chart-{}", i + 1),
                rotate(plan.primary, deg, 0.62, chroma),
                rotate(plan.primary, deg, 0.70, chroma),
            ));
        }
    }

    if plan.sidebar {
        let l_sb_fg = foreground_for(l_sidebar, light_end, dark_end);
        let d_sb_fg = foreground_for(d_sidebar, light_end, dark_end);
        tokens.extend([
            ("sidebar".to_string(), l_sidebar, d_sidebar),
            ("sidebar-foreground".to_string(), l_sb_fg, d_sb_fg),
            ("sidebar-primary".to_string(), l_primary, d_primary),
            (
                "sidebar-primary-foreground".to_string(),
                foreground_for(l_primary, light_end, dark_end),
                foreground_for(d_primary, light_end, dark_end),
            ),
            ("sidebar-accent".to_string(), l_accent, d_accent),
            (
                "sidebar-accent-foreground".to_string(),
                foreground_for(l_accent, light_end, dark_end),
                foreground_for(d_accent, light_end, dark_end),
            ),
            ("sidebar-border".to_string(), l_border, d_border),
            ("sidebar-ring".to_string(), l_ring, d_ring),
        ]);
    }

    // (foreground token, background token, AA minimum) pairs worth checking.
    // Text pairs are held to 4.5:1; the focus ring is non-text UI, so its bar is
    // the 3:1 of SC 1.4.11.
    let checks: Vec<(&str, &str, f64)> = vec![
        ("foreground", "background", AA),
        ("primary-foreground", "primary", AA),
        ("secondary-foreground", "secondary", AA),
        ("accent-foreground", "accent", AA),
        ("muted-foreground", "muted", AA),
        ("destructive-foreground", "destructive", AA),
        ("ring", "background", AA_NON_TEXT),
    ];
    let find = |n: &str| tokens.iter().find(|(name, _, _)| name == n).cloned();
    let mut pairs = Vec::new();
    for (fg, bg, min) in checks {
        if let (Some((_, lf, df)), Some((_, lb, db))) = (find(fg), find(bg)) {
            pairs.push((format!("{fg} on {bg}"), "light".to_string(), lf, lb, min));
            pairs.push((format!("{fg} on {bg}"), "dark".to_string(), df, db, min));
        }
    }

    (tokens, pairs)
}

// ---------------------------------------------------------------------------
// CSS emission
// ---------------------------------------------------------------------------

fn block(selector: &str, radius: Option<&str>, rows: &[(String, String)]) -> String {
    let mut s = format!("{selector} {{\n");
    if let Some(r) = radius {
        s.push_str(&format!("  --radius: {r};\n"));
    }
    for (name, value) in rows {
        s.push_str(&format!("  --{name}: {value};\n"));
    }
    s.push_str("}\n");
    s
}

fn theme_inline(names: &[String]) -> String {
    let mut s = String::from("@theme inline {\n");
    s.push_str("  --radius-sm: calc(var(--radius) - 4px);\n");
    s.push_str("  --radius-md: calc(var(--radius) - 2px);\n");
    s.push_str("  --radius-lg: var(--radius);\n");
    s.push_str("  --radius-xl: calc(var(--radius) + 4px);\n");
    for n in names {
        s.push_str(&format!("  --color-{n}: var(--{n});\n"));
    }
    s.push_str("}\n");
    s
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Generate a shadcn/ui theme.
///
/// * `primary` — the seed color (`#rgb`, `#rrggbb`, bare hex, `rgb()`, `hsl()`).
/// * `accent` — optional second seed; empty/blank uses a neutral accent.
/// * `neutral` — grey family: `slate`, `gray`, `zinc`, `neutral`, `stone`.
/// * `format` — `oklch`, `hsl` or `hex`.
/// * `tailwind` — `v4` or `v3`.
/// * `radius_rem` — the `--radius` value in rem, 0 to [`MAX_RADIUS_REM`].
/// * `mode` — `both`, `light` or `dark`.
/// * `charts` / `sidebar` — include the `--chart-*` / `--sidebar-*` groups.
#[allow(clippy::too_many_arguments)]
pub fn generate(
    primary: &str,
    accent: &str,
    neutral: &str,
    format: &str,
    tailwind: &str,
    radius_rem: f64,
    mode: &str,
    charts: bool,
    sidebar: bool,
) -> Result<Theme, String> {
    let primary_rgb = parse_color(primary)?;
    let accent_seed = if accent.trim().is_empty() {
        None
    } else {
        Some(parse_color(accent)?)
    };
    let (hue, chroma) = neutral_tint(neutral)?;
    let format_e = Format::parse(format)?;
    let tailwind_e = Tailwind::parse(tailwind)?;
    let mode_e = Mode::parse(mode)?;

    if !radius_rem.is_finite() || radius_rem < 0.0 {
        return Err("radius must be 0 or more (in rem)".into());
    }
    if radius_rem > MAX_RADIUS_REM {
        return Err(format!(
            "radius {radius_rem}rem is above the {MAX_RADIUS_REM}rem cap — every shadcn control is already fully rounded there"
        ));
    }

    let plan = Plan {
        primary: primary_rgb,
        accent_seed,
        hue,
        chroma,
        format: format_e,
        tailwind: tailwind_e,
        charts,
        sidebar,
    };

    let (raw, pairs) = derive(&plan);

    let radius = format!("{}rem", round3(radius_rem));

    let tokens: Vec<Token> = raw
        .iter()
        .map(|(name, l, d)| Token {
            name: name.clone(),
            light: mode_e
                .has_light()
                .then(|| write_color(*l, plan.format, plan.tailwind)),
            dark: mode_e
                .has_dark()
                .then(|| write_color(*d, plan.format, plan.tailwind)),
        })
        .collect();

    let light_rows: Vec<(String, String)> = raw
        .iter()
        .map(|(n, l, _)| (n.clone(), write_color(*l, plan.format, plan.tailwind)))
        .collect();
    let dark_rows: Vec<(String, String)> = raw
        .iter()
        .map(|(n, _, d)| (n.clone(), write_color(*d, plan.format, plan.tailwind)))
        .collect();

    let mut css = String::new();
    match tailwind_e {
        Tailwind::V4 => {
            if mode_e.has_light() {
                css.push_str(&block(":root", Some(&radius), &light_rows));
            }
            if mode_e.has_dark() {
                if !css.is_empty() {
                    css.push('\n');
                }
                let r = (!mode_e.has_light()).then_some(radius.as_str());
                css.push_str(&block(".dark", r, &dark_rows));
            }
            css.push('\n');
            let names: Vec<String> = raw.iter().map(|(n, _, _)| n.clone()).collect();
            css.push_str(&theme_inline(&names));
        }
        Tailwind::V3 => {
            css.push_str("@layer base {\n");
            let indent = |b: String| {
                b.lines()
                    .map(|l| {
                        if l.is_empty() {
                            String::new()
                        } else {
                            format!("  {l}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            if mode_e.has_light() {
                css.push_str(&indent(block(":root", Some(&radius), &light_rows)));
                css.push('\n');
            }
            if mode_e.has_dark() {
                let r = (!mode_e.has_light()).then_some(radius.as_str());
                css.push_str(&indent(block(".dark", r, &dark_rows)));
                css.push('\n');
            }
            css.push_str("}\n");
        }
    }

    let mut contrast = Vec::new();
    let mut warnings = Vec::new();
    for (pair, m, fg, bg, minimum) in pairs {
        if (m == "light" && !mode_e.has_light()) || (m == "dark" && !mode_e.has_dark()) {
            continue;
        }
        let ratio = round2(contrast_ratio(fg, bg));
        let passes = ratio >= minimum;
        if !passes {
            let what = if minimum == AA {
                "normal text"
            } else {
                "focus rings and other non-text UI"
            };
            warnings.push(format!(
                "{pair} ({m}) is {ratio}:1 — below the {minimum}:1 WCAG AA minimum for {what}; try a deeper or lighter seed"
            ));
        }
        contrast.push(Contrast {
            pair,
            mode: m,
            ratio,
            minimum,
            passes_aa: passes,
        });
    }

    Ok(Theme {
        primary: format!(
            "#{:02x}{:02x}{:02x}",
            primary_rgb.r, primary_rgb.g, primary_rgb.b
        ),
        accent: match accent_seed {
            Some(a) => format!("#{:02x}{:02x}{:02x}", a.r, a.g, a.b),
            None => "neutral".to_string(),
        },
        neutral: neutral.trim().to_ascii_lowercase(),
        format: format.trim().to_ascii_lowercase(),
        tailwind: tailwind.trim().to_ascii_lowercase(),
        radius,
        mode: mode.trim().to_ascii_lowercase(),
        tokens,
        css,
        contrast,
        warnings,
    })
}

// ---------------------------------------------------------------------------
// Rendering — the pasteable text every surface returns
// ---------------------------------------------------------------------------

/// Render a [`Theme`] as one pasteable block: a provenance header, the
/// stylesheet, and the measured contrast table. Everything is CSS comment
/// syntax around real CSS, so the whole output can go straight into
/// `globals.css` without editing.
pub fn render(t: &Theme) -> String {
    let accent = if t.accent == "neutral" {
        "neutral accent".to_string()
    } else {
        format!("accent {}", t.accent)
    };
    let mut s = format!(
        "/* shadcn/ui theme — primary {}, {}, {} greys, Tailwind {}, {} values, radius {} */\n\n",
        t.primary, accent, t.neutral, t.tailwind, t.format, t.radius
    );
    s.push_str(&t.css);

    s.push_str(
        "\n/* Contrast check (WCAG 2.x AA — 4.5:1 for normal text, 3:1 for the focus ring)\n",
    );
    for c in &t.contrast {
        let ratio = format!("{:.2}", c.ratio);
        let verdict = if c.passes_aa { "AA pass" } else { "BELOW AA" };
        s.push_str(&format!(
            " * {:<5}  {:<44} {:>6}:1  {verdict}\n",
            c.mode, c.pair, ratio
        ));
    }
    for w in &t.warnings {
        s.push_str(&format!(" *\n * ! {w}\n"));
    }
    s.push_str(" */\n");
    s
}

/// Generate a theme and render it. This is what the chat block, the CLI and the
/// browser page all call; see [`generate`] for the parameters.
#[allow(clippy::too_many_arguments)]
pub fn run(
    primary: &str,
    accent: &str,
    neutral: &str,
    format: &str,
    tailwind: &str,
    radius_rem: f64,
    mode: &str,
    charts: bool,
    sidebar: bool,
) -> Result<String, String> {
    generate(
        primary, accent, neutral, format, tailwind, radius_rem, mode, charts, sidebar,
    )
    .map(|t| render(&t))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Theme {
        generate(
            "#6366f1", "", "zinc", "oklch", "v4", 0.625, "both", true, true,
        )
        .expect("default theme")
    }

    #[test]
    fn emits_the_full_shadcn_token_set_for_both_modes() {
        let t = theme();
        let names: Vec<&str> = t.tokens.iter().map(|x| x.name.as_str()).collect();
        for want in [
            "background",
            "foreground",
            "card",
            "card-foreground",
            "popover",
            "popover-foreground",
            "primary",
            "primary-foreground",
            "secondary",
            "secondary-foreground",
            "muted",
            "muted-foreground",
            "accent",
            "accent-foreground",
            "destructive",
            "border",
            "input",
            "ring",
            "chart-1",
            "chart-5",
            "sidebar",
            "sidebar-ring",
        ] {
            assert!(names.contains(&want), "missing --{want}");
        }
        assert!(t.tokens.iter().all(|x| x.light.is_some() && x.dark.is_some()));
        assert!(t.css.contains(":root {"), "light block");
        assert!(t.css.contains(".dark {"), "dark block");
        assert!(t.css.contains("--radius: 0.625rem;"));
        assert!(t.css.contains("@theme inline"), "v4 emits the theme map");
    }

    #[test]
    fn light_primary_is_the_seed_verbatim() {
        let t = generate("#6366f1", "", "zinc", "hex", "v4", 0.5, "both", true, true).unwrap();
        let p = t.tokens.iter().find(|x| x.name == "primary").unwrap();
        assert_eq!(p.light.as_deref(), Some("#6366f1"));
        // The dark block lifts it so it reads on a dark surface.
        assert_ne!(p.dark.as_deref(), Some("#6366f1"));
    }

    #[test]
    fn dark_primary_is_lighter_than_a_dark_seed() {
        let t = generate("#1e1b4b", "", "slate", "hex", "v4", 0.5, "both", true, true).unwrap();
        let p = t.tokens.iter().find(|x| x.name == "primary").unwrap();
        let l = to_oklch(parse_color(p.light.as_ref().unwrap()).unwrap()).l;
        let d = to_oklch(parse_color(p.dark.as_ref().unwrap()).unwrap()).l;
        assert!(d > l, "dark primary {d} should outshine light {l}");
    }

    #[test]
    fn foregrounds_meet_wcag_aa_on_a_mid_tone_seed() {
        let t = theme();
        for c in &t.contrast {
            assert!(
                c.passes_aa,
                "{} ({}) only reached {}:1",
                c.pair, c.mode, c.ratio
            );
        }
        assert!(t.warnings.is_empty());
        // A body-text pair on a near-white background is comfortably past AA.
        let fg = t
            .contrast
            .iter()
            .find(|c| c.pair == "foreground on background" && c.mode == "light")
            .unwrap();
        assert!(fg.ratio > 15.0, "got {}", fg.ratio);
    }

    #[test]
    fn a_mid_tone_seed_escalates_its_foreground_to_clear_aa() {
        // #6366f1 sits in the gap where neither ladder end reaches 4.5:1
        // (4.30 near-white, 4.43 near-black), so the token walks to the end of
        // the ladder rather than shipping a pair that misses AA.
        let t = generate("#6366f1", "", "zinc", "hex", "v4", 0.5, "both", true, true).unwrap();
        let fg = t
            .tokens
            .iter()
            .find(|x| x.name == "primary-foreground")
            .unwrap();
        assert_eq!(fg.light.as_deref(), Some("#000000"));
        let p = t
            .contrast
            .iter()
            .find(|c| c.pair == "primary-foreground on primary" && c.mode == "light")
            .unwrap();
        assert!(p.ratio >= 4.5, "got {}", p.ratio);
        // A surface either end clears comfortably keeps the tinted neutral —
        // pure black is the fallback, not the default.
        let s = generate("#0f172a", "", "zinc", "hex", "v4", 0.5, "both", true, true).unwrap();
        let sfg = s
            .tokens
            .iter()
            .find(|x| x.name == "primary-foreground")
            .unwrap();
        assert_ne!(sfg.light.as_deref(), Some("#ffffff"));
    }

    #[test]
    fn a_faint_focus_ring_is_reported_not_silently_darkened() {
        // --ring is the brand colour itself, so a pale seed produces a focus
        // ring under the 3:1 bar for non-text UI. The generator says so instead
        // of repainting the brand.
        let t = generate("#facc15", "", "neutral", "hex", "v4", 0.5, "both", true, true).unwrap();
        let r = t
            .contrast
            .iter()
            .find(|c| c.pair == "ring on background" && c.mode == "light")
            .unwrap();
        assert_eq!(r.minimum, 3.0);
        assert!(!r.passes_aa, "expected a sub-3:1 ratio, got {}", r.ratio);
        assert!(
            t.warnings
                .iter()
                .any(|w| w.contains("ring on background") && w.contains("non-text UI")),
            "warnings: {:?}",
            t.warnings
        );
        // The text pairs on that same pale seed still clear AA.
        assert!(t
            .contrast
            .iter()
            .filter(|c| c.minimum == 4.5)
            .all(|c| c.passes_aa));
    }

    #[test]
    fn accent_seed_tints_the_accent_tokens() {
        let plain = generate("#6366f1", "", "zinc", "hex", "v4", 0.5, "both", true, true).unwrap();
        let tinted =
            generate("#6366f1", "#10b981", "zinc", "hex", "v4", 0.5, "both", true, true).unwrap();
        let a1 = plain.tokens.iter().find(|x| x.name == "accent").unwrap();
        let a2 = tinted.tokens.iter().find(|x| x.name == "accent").unwrap();
        assert_ne!(a1.light, a2.light);
        assert_eq!(plain.accent, "neutral");
        assert_eq!(tinted.accent, "#10b981");
        // The accent keeps the second seed's hue family (green), not the seed's.
        let h = to_oklch(parse_color(a2.light.as_ref().unwrap()).unwrap()).h;
        assert!((100.0..200.0).contains(&h), "accent hue was {h}");
    }

    #[test]
    fn v3_emits_bare_hsl_triplets_inside_layer_base() {
        let t = generate("#6366f1", "", "slate", "hsl", "v3", 0.5, "both", true, true).unwrap();
        assert!(t.css.starts_with("@layer base {"));
        let p = t.tokens.iter().find(|x| x.name == "primary").unwrap();
        let v = p.light.clone().unwrap();
        assert!(!v.contains("hsl("), "v3 triplets are bare, got {v}");
        assert!(v.ends_with('%'), "got {v}");
        // v4 + hsl wraps the same value in hsl().
        let t4 = generate("#6366f1", "", "slate", "hsl", "v4", 0.5, "both", true, true).unwrap();
        let p4 = t4.tokens.iter().find(|x| x.name == "primary").unwrap();
        assert!(p4.light.as_ref().unwrap().starts_with("hsl("));
    }

    #[test]
    fn mode_light_drops_the_dark_block() {
        let t = generate("#6366f1", "", "zinc", "oklch", "v4", 0.5, "light", true, true).unwrap();
        assert!(t.css.contains(":root {"));
        assert!(!t.css.contains(".dark {"));
        assert!(t.tokens.iter().all(|x| x.dark.is_none()));
        assert!(t.contrast.iter().all(|c| c.mode == "light"));

        let d = generate("#6366f1", "", "zinc", "oklch", "v4", 0.5, "dark", true, true).unwrap();
        assert!(!d.css.contains(":root {"));
        assert!(d.css.contains(".dark {"));
        // The radius still has to land somewhere when :root is skipped.
        assert!(d.css.contains("--radius: 0.5rem;"));
    }

    #[test]
    fn toggles_drop_the_chart_and_sidebar_groups() {
        let t = generate("#6366f1", "", "zinc", "oklch", "v4", 0.5, "both", false, false).unwrap();
        let names: Vec<&str> = t.tokens.iter().map(|x| x.name.as_str()).collect();
        assert!(!names.iter().any(|n| n.starts_with("chart-")));
        assert!(!names.iter().any(|n| n.starts_with("sidebar")));
        assert!(!t.css.contains("--chart-1"));
        assert!(names.contains(&"primary"), "core tokens stay");
    }

    #[test]
    fn neutral_family_tints_the_greys() {
        let zinc = generate("#6366f1", "", "zinc", "hex", "v4", 0.5, "both", true, true).unwrap();
        let stone = generate("#6366f1", "", "stone", "hex", "v4", 0.5, "both", true, true).unwrap();
        let pick = |t: &Theme| {
            t.tokens
                .iter()
                .find(|x| x.name == "muted")
                .unwrap()
                .light
                .clone()
                .unwrap()
        };
        assert_ne!(pick(&zinc), pick(&stone));
        // "neutral" is the untinted family: a pure grey.
        let n = generate("#6366f1", "", "neutral", "hex", "v4", 0.5, "both", true, true).unwrap();
        let m = parse_color(&pick(&n)).unwrap();
        assert_eq!(m.r, m.g);
        assert_eq!(m.g, m.b);
    }

    #[test]
    fn accepts_every_advertised_color_notation() {
        let want = "#6366f1";
        for input in [
            "#6366f1",
            "6366F1",
            "rgb(99, 102, 241)",
            "rgb(99 102 241)",
        ] {
            let t = generate(input, "", "zinc", "hex", "v4", 0.5, "both", true, true).unwrap();
            assert_eq!(t.primary, want, "input {input}");
        }
        // Short hex expands.
        let t = generate("#f00", "", "zinc", "hex", "v4", 0.5, "both", true, true).unwrap();
        assert_eq!(t.primary, "#ff0000");
        // hsl() round-trips within rounding distance: hsl(239, 84%, 67%) is the
        // 2-decimal HSL form of #6366f1, so it lands a channel or two away.
        let t = generate("hsl(239, 84%, 67%)", "", "zinc", "hex", "v4", 0.5, "both", true, true)
            .unwrap();
        let got = parse_color(&t.primary).unwrap();
        let want_rgb = parse_color(want).unwrap();
        for (g, w) in [
            (got.r, want_rgb.r),
            (got.g, want_rgb.g),
            (got.b, want_rgb.b),
        ] {
            assert!(
                g.abs_diff(w) <= 2,
                "hsl round-trip gave {} vs {want}",
                t.primary
            );
        }
    }

    #[test]
    fn radius_cap_is_inclusive_and_the_step_past_it_errors() {
        assert!(generate("#6366f1", "", "zinc", "oklch", "v4", MAX_RADIUS_REM, "both", true, true)
            .is_ok());
        let e = generate(
            "#6366f1",
            "",
            "zinc",
            "oklch",
            "v4",
            MAX_RADIUS_REM + 0.1,
            "both",
            true,
            true,
        )
        .unwrap_err();
        assert!(e.contains("cap"), "got {e}");
        assert!(generate("#6366f1", "", "zinc", "oklch", "v4", -0.1, "both", true, true).is_err());
    }

    #[test]
    fn rejects_a_color_it_cannot_read() {
        let e = generate("not-a-color", "", "zinc", "oklch", "v4", 0.5, "both", true, true)
            .unwrap_err();
        assert!(e.contains("could not read"), "got {e}");
        let e = generate("#12345", "", "zinc", "oklch", "v4", 0.5, "both", true, true).unwrap_err();
        assert!(e.contains("5 digits"), "got {e}");
        let e = generate("", "", "zinc", "oklch", "v4", 0.5, "both", true, true).unwrap_err();
        assert!(e.contains("no color given"), "got {e}");
    }

    #[test]
    fn rejects_unknown_enum_values_with_the_allowed_list() {
        for (args, needle) in [
            (("bogus", "v4", "both"), "unknown format"),
            (("oklch", "v9", "both"), "unknown tailwind"),
            (("oklch", "v4", "sideways"), "unknown mode"),
        ] {
            let e = generate("#6366f1", "", "zinc", args.0, args.1, 0.5, args.2, true, true)
                .unwrap_err();
            assert!(e.contains(needle), "got {e}");
        }
        let e =
            generate("#6366f1", "", "taupe", "oklch", "v4", 0.5, "both", true, true).unwrap_err();
        assert!(e.contains("unknown neutral"), "got {e}");
    }

    #[test]
    fn rendered_output_is_a_header_plus_css_plus_a_contrast_table() {
        let out = run(
            "#6366f1", "", "zinc", "oklch", "v4", 0.625, "both", true, true,
        )
        .unwrap();
        assert!(
            out.starts_with(
                "/* shadcn/ui theme — primary #6366f1, neutral accent, zinc greys, Tailwind v4, oklch values, radius 0.625rem */"
            ),
            "header was: {}",
            out.lines().next().unwrap()
        );
        assert!(out.contains(":root {\n  --radius: 0.625rem;\n"));
        assert!(out.contains(".dark {"));
        assert!(out.contains("@theme inline {"));
        assert!(out
            .contains("/* Contrast check (WCAG 2.x AA — 4.5:1 for normal text, 3:1 for the focus ring)"));
        assert!(out.contains("foreground on background"));
        assert!(out.contains("AA pass"));
        assert!(out.trim_end().ends_with("*/"));
        // An accent seed is named in the header instead of "neutral accent".
        let tinted = run(
            "#6366f1", "#10b981", "zinc", "oklch", "v4", 0.5, "both", true, true,
        )
        .unwrap();
        assert!(tinted.lines().next().unwrap().contains("accent #10b981"));
    }

    #[test]
    fn rendered_output_flags_a_failing_pair_in_the_comment_block() {
        let out = run(
            "#facc15", "", "neutral", "hex", "v4", 0.5, "both", true, true,
        )
        .unwrap();
        assert!(out.contains("BELOW AA"), "{out}");
        assert!(out.contains(" * ! ring on background"), "{out}");
        // The table still reports every pair, passing ones included.
        assert!(out.contains("AA pass"), "{out}");
    }

    #[test]
    fn run_propagates_input_errors_verbatim() {
        let e = run("nope", "", "zinc", "oklch", "v4", 0.5, "both", true, true).unwrap_err();
        assert!(e.contains("could not read"), "got {e}");
        let e = run("#6366f1", "", "zinc", "oklch", "v4", 9.0, "both", true, true).unwrap_err();
        assert!(e.contains("cap"), "got {e}");
    }

    #[test]
    fn oklch_values_are_well_formed_and_greys_are_achromatic() {
        let t = generate("#6366f1", "", "neutral", "oklch", "v4", 0.5, "both", true, true).unwrap();
        let bg = t
            .tokens
            .iter()
            .find(|x| x.name == "background")
            .unwrap()
            .light
            .clone()
            .unwrap();
        assert_eq!(bg, "oklch(1 0 0)", "pure white background");
        for tok in &t.tokens {
            let v = tok.light.clone().unwrap();
            assert!(v.starts_with("oklch(") && v.ends_with(')'), "{v}");
            assert_eq!(v.split_whitespace().count(), 3, "{v}");
        }
    }
}
