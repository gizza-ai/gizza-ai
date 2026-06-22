//! color-contrast-checker core — compute the WCAG 2.x contrast ratio between a
//! foreground and a background colour and report AA / AAA pass/fail for normal
//! text, large text, and UI components. Pure compute, no I/O — shared by the
//! chat skill block and the web page.

/// An sRGB colour, 8-bit per channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// Normalised hex string, always `#rrggbb` lowercase.
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// Parse a colour from any of:
/// - a hex string (`#rgb`, `#rrggbb`, with or without the leading `#`, case-insensitive),
/// - an `rgb(r, g, b)` / `r,g,b` triple (channels 0–255, with or without the wrapper),
/// - an `hsl(h, s%, l%)` triple (hue 0–360, saturation/lightness 0–100%),
/// - a CSS named colour (`white`, `rebeccapurple`, `tomato`, …).
pub fn parse_color(input: &str) -> Result<Rgb, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("empty colour: expected a hex code like #1a2b3c, rgb(26,43,60), hsl(210,40%,17%), or a name like 'navy'".into());
    }

    let lower = s.to_ascii_lowercase();

    // hsl(...) form.
    if lower.starts_with("hsl(") || lower.starts_with("hsla(") {
        return parse_hsl(s);
    }

    // rgb(...) / r,g,b form: detect by a comma or the rgb( wrapper.
    if lower.starts_with("rgb(") || lower.starts_with("rgba(") || s.contains(',') {
        return parse_rgb_triple(s);
    }

    // A CSS named colour (only if it's all letters — keeps bare hex like "abc" hex).
    if s.chars().all(|c| c.is_ascii_alphabetic()) {
        if let Some(rgb) = named_color(&lower) {
            return Ok(rgb);
        }
        // Fall through to hex only if it looks hex-y; otherwise a clearer error.
        if !lower.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("unknown colour name {s:?}: not a CSS named colour"));
        }
    }

    parse_hex(s)
}

fn parse_hex(s: &str) -> Result<Rgb, String> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "invalid hex colour {s:?}: expected hex digits like #1a2b3c"
        ));
    }
    let parse = |slice: &str| u8::from_str_radix(slice, 16).map_err(|e| e.to_string());
    match hex.len() {
        // #rgb shorthand → each digit doubled (f → ff).
        3 => {
            let expand = |c: char| {
                let d = c.to_digit(16).unwrap() as u8;
                d << 4 | d
            };
            let mut it = hex.chars();
            Ok(Rgb {
                r: expand(it.next().unwrap()),
                g: expand(it.next().unwrap()),
                b: expand(it.next().unwrap()),
            })
        }
        6 => Ok(Rgb {
            r: parse(&hex[0..2])?,
            g: parse(&hex[2..4])?,
            b: parse(&hex[4..6])?,
        }),
        n => Err(format!(
            "invalid hex colour {s:?}: expected 3 or 6 hex digits, got {n}"
        )),
    }
}

fn parse_rgb_triple(s: &str) -> Result<Rgb, String> {
    // Strip an optional rgb( / rgba( wrapper and trailing ).
    let inner = {
        let lower = s.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("rgba(").or_else(|| lower.strip_prefix("rgb(")) {
            rest.strip_suffix(')').unwrap_or(rest).to_string()
        } else {
            s.to_string()
        }
    };
    let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
    if parts.len() < 3 {
        return Err(format!(
            "invalid rgb colour {s:?}: expected three channels like rgb(26, 43, 60)"
        ));
    }
    let chan = |p: &str| -> Result<u8, String> {
        let v: i64 = p
            .parse()
            .map_err(|_| format!("invalid rgb channel {p:?}: expected an integer 0-255"))?;
        if !(0..=255).contains(&v) {
            return Err(format!("rgb channel out of range: {v} (must be 0-255)"));
        }
        Ok(v as u8)
    };
    Ok(Rgb {
        r: chan(parts[0])?,
        g: chan(parts[1])?,
        b: chan(parts[2])?,
    })
}

fn parse_hsl(s: &str) -> Result<Rgb, String> {
    let lower = s.to_ascii_lowercase();
    let inner = lower
        .strip_prefix("hsla(")
        .or_else(|| lower.strip_prefix("hsl("))
        .map(|rest| rest.strip_suffix(')').unwrap_or(rest))
        .ok_or_else(|| format!("invalid hsl colour {s:?}"))?;
    let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
    if parts.len() < 3 {
        return Err(format!(
            "invalid hsl colour {s:?}: expected hsl(hue, saturation%, lightness%)"
        ));
    }
    let h: f64 = parts[0]
        .trim_end_matches("deg")
        .trim()
        .parse()
        .map_err(|_| format!("invalid hsl hue {:?}: expected 0-360", parts[0]))?;
    let pct = |p: &str, what: &str| -> Result<f64, String> {
        let v: f64 = p
            .trim_end_matches('%')
            .trim()
            .parse()
            .map_err(|_| format!("invalid hsl {what} {p:?}: expected 0-100%"))?;
        if !(0.0..=100.0).contains(&v) {
            return Err(format!("hsl {what} out of range: {v} (must be 0-100)"));
        }
        Ok(v / 100.0)
    };
    let sat = pct(parts[1], "saturation")?;
    let light = pct(parts[2], "lightness")?;
    Ok(hsl_to_rgb(h.rem_euclid(360.0), sat, light))
}

/// Convert HSL (hue in degrees 0–360, sat/light in 0.0–1.0) to 8-bit sRGB.
pub fn hsl_to_rgb(h: f64, s: f64, l: f64) -> Rgb {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h6 = h / 60.0;
    let x = c * (1.0 - (h6.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = match h6 as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to8 = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb {
        r: to8(r1),
        g: to8(g1),
        b: to8(b1),
    }
}

/// Convert 8-bit sRGB to HSL (hue 0–360 degrees, sat/light 0.0–1.0).
pub fn rgb_to_hsl(c: Rgb) -> (f64, f64, f64) {
    let r = c.r as f64 / 255.0;
    let g = c.g as f64 / 255.0;
    let b = c.b as f64 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d.abs() < 1e-12 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h.rem_euclid(360.0), s, l)
}

/// A small curated set of common CSS named colours (the most-used subset). Returns
/// `None` for an unknown name so the caller can produce a clear error.
fn named_color(name: &str) -> Option<Rgb> {
    let rgb = |r, g, b| Rgb { r, g, b };
    Some(match name {
        "black" => rgb(0, 0, 0),
        "white" => rgb(255, 255, 255),
        "red" => rgb(255, 0, 0),
        "green" => rgb(0, 128, 0),
        "lime" => rgb(0, 255, 0),
        "blue" => rgb(0, 0, 255),
        "yellow" => rgb(255, 255, 0),
        "cyan" | "aqua" => rgb(0, 255, 255),
        "magenta" | "fuchsia" => rgb(255, 0, 255),
        "silver" => rgb(192, 192, 192),
        "gray" | "grey" => rgb(128, 128, 128),
        "maroon" => rgb(128, 0, 0),
        "olive" => rgb(128, 128, 0),
        "purple" => rgb(128, 0, 128),
        "teal" => rgb(0, 128, 128),
        "navy" => rgb(0, 0, 128),
        "orange" => rgb(255, 165, 0),
        "pink" => rgb(255, 192, 203),
        "tomato" => rgb(255, 99, 71),
        "gold" => rgb(255, 215, 0),
        "indigo" => rgb(75, 0, 130),
        "violet" => rgb(238, 130, 238),
        "brown" => rgb(165, 42, 42),
        "coral" => rgb(255, 127, 80),
        "salmon" => rgb(250, 128, 114),
        "khaki" => rgb(240, 230, 140),
        "crimson" => rgb(220, 20, 60),
        "skyblue" => rgb(135, 206, 235),
        "royalblue" => rgb(65, 105, 225),
        "steelblue" => rgb(70, 130, 180),
        "slategray" | "slategrey" => rgb(112, 128, 144),
        "dimgray" | "dimgrey" => rgb(105, 105, 105),
        "darkgray" | "darkgrey" => rgb(169, 169, 169),
        "lightgray" | "lightgrey" => rgb(211, 211, 211),
        "gainsboro" => rgb(220, 220, 220),
        "whitesmoke" => rgb(245, 245, 245),
        "rebeccapurple" => rgb(102, 51, 153),
        "darkblue" => rgb(0, 0, 139),
        "darkgreen" => rgb(0, 100, 0),
        "darkred" => rgb(139, 0, 0),
        "forestgreen" => rgb(34, 139, 34),
        "seagreen" => rgb(46, 139, 87),
        "midnightblue" => rgb(25, 25, 112),
        _ => return None,
    })
}

/// Target WCAG level for [`suggest_passing`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// AA normal text: 4.5:1.
    Aa,
    /// AAA normal text: 7:1.
    Aaa,
    /// AA large text / UI components: 3:1.
    Large,
}

impl Target {
    fn ratio(self) -> f64 {
        match self {
            Target::Aa => 4.5,
            Target::Aaa => 7.0,
            Target::Large => 3.0,
        }
    }
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "aa" => Ok(Target::Aa),
            "aaa" => Ok(Target::Aaa),
            "large" | "aa-large" => Ok(Target::Large),
            other => Err(format!(
                "invalid target {other:?}: expected \"aa\", \"aaa\", or \"large\""
            )),
        }
    }
}

/// Find the foreground colour closest to `fg` (same hue & saturation, lightness
/// adjusted) that reaches `target` contrast against `background`. Returns the
/// original colour unchanged if it already passes, or `None` if no lightness of
/// that hue/saturation can reach the target against this background (e.g. asking
/// for AAA against a mid-grey). Adjusts lightness only, preserving the hue intent.
pub fn suggest_passing(fg: Rgb, background: Rgb, target: Target) -> Option<Rgb> {
    let need = target.ratio();
    if contrast_ratio(fg, background) >= need {
        return Some(fg);
    }
    let (h, s, _) = rgb_to_hsl(fg);
    // Scan lightness from the original outward, both darker and lighter, and keep
    // the candidate nearest the original lightness that meets the target.
    let (_, _, l0) = rgb_to_hsl(fg);
    let mut best: Option<(Rgb, f64)> = None;
    // 0..=1000 → lightness step 0.001 for good precision.
    for i in 0..=1000 {
        let l = i as f64 / 1000.0;
        let cand = hsl_to_rgb(h, s, l);
        if contrast_ratio(cand, background) >= need {
            let dist = (l - l0).abs();
            if best.as_ref().map(|(_, d)| dist < *d).unwrap_or(true) {
                best = Some((cand, dist));
            }
        }
    }
    best.map(|(c, _)| c)
}

/// WCAG relative luminance of an sRGB colour (0.0 = black, 1.0 = white).
/// Per WCAG 2.x: linearise each channel then weight 0.2126/0.7152/0.0722.
pub fn relative_luminance(c: Rgb) -> f64 {
    fn lin(channel: u8) -> f64 {
        let s = channel as f64 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// WCAG contrast ratio between two colours: (L_lighter + 0.05) / (L_darker +
/// 0.05). Ranges 1.0 (identical) to 21.0 (black on white). Order-independent.
pub fn contrast_ratio(a: Rgb, b: Rgb) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Round a ratio to two decimals for display (e.g. 4.5 → "4.5", 7.004 → "7").
fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// The full result of a contrast check, serialised to a small JSON report.
#[derive(Debug)]
pub struct Report {
    pub foreground: Rgb,
    pub background: Rgb,
    pub ratio: f64,
    /// AA / AAA pass for each text-size category + UI components.
    pub aa_normal: bool,
    pub aa_large: bool,
    pub aaa_normal: bool,
    pub aaa_large: bool,
    pub ui_components: bool,
}

impl Report {
    /// Highest WCAG level fully passed for normal body text: "AAA", "AA", or "Fail".
    pub fn summary_level(&self) -> &'static str {
        if self.aaa_normal {
            "AAA"
        } else if self.aa_normal {
            "AA"
        } else {
            "Fail"
        }
    }

    /// Pretty multi-line text report.
    pub fn to_text(&self) -> String {
        let yn = |b: bool| if b { "Pass" } else { "Fail" };
        format!(
            "Foreground: {fg}\nBackground: {bg}\nContrast ratio: {ratio}:1\n\nWCAG 2.1 results\n  Normal text (AA, >= 4.5): {aan}\n  Normal text (AAA, >= 7): {aaan}\n  Large text (AA, >= 3): {aal}\n  Large text (AAA, >= 4.5): {aaal}\n  UI components / graphics (>= 3): {ui}\n\nBest level for normal text: {best}",
            fg = self.foreground.to_hex(),
            bg = self.background.to_hex(),
            ratio = round2(self.ratio),
            aan = yn(self.aa_normal),
            aaan = yn(self.aaa_normal),
            aal = yn(self.aa_large),
            aaal = yn(self.aaa_large),
            ui = yn(self.ui_components),
            best = self.summary_level(),
        )
    }

    /// Compact JSON object string (stable key order; ratio rounded to 2 dp).
    pub fn to_json(&self) -> String {
        format!(
            r#"{{"foreground":"{fg}","background":"{bg}","ratio":{ratio},"aa_normal":{aan},"aa_large":{aal},"aaa_normal":{aaan},"aaa_large":{aaal},"ui_components":{ui},"summary":"{best}"}}"#,
            fg = self.foreground.to_hex(),
            bg = self.background.to_hex(),
            ratio = round2(self.ratio),
            aan = self.aa_normal,
            aal = self.aa_large,
            aaan = self.aaa_normal,
            aaal = self.aaa_large,
            ui = self.ui_components,
            best = self.summary_level(),
        )
    }
}

/// Build a contrast [`Report`] from a foreground and background colour.
pub fn check(foreground: Rgb, background: Rgb) -> Report {
    let ratio = contrast_ratio(foreground, background);
    Report {
        foreground,
        background,
        ratio,
        aa_normal: ratio >= 4.5,
        aa_large: ratio >= 3.0,
        aaa_normal: ratio >= 7.0,
        aaa_large: ratio >= 4.5,
        ui_components: ratio >= 3.0,
    }
}

/// Parse both colours, compute the contrast report, and render it.
///
/// - `foreground` / `background`: a hex code, rgb/hsl triple, or CSS colour name
///   (see [`parse_color`]).
/// - `format` (`"text"` | `"json"` | `"suggest"`, blank → `"text"`): output shape.
///   `suggest` reports the check AND the nearest accessible foreground (same
///   hue/saturation, lightness nudged) that reaches the `target` level.
/// - `target` (`"aa"` | `"aaa"` | `"large"`, blank → `"aa"`): which threshold the
///   `suggest` mode aims for. Ignored for `text`/`json`.
pub fn run(
    foreground: &str,
    background: &str,
    format: &str,
    target: &str,
) -> Result<String, String> {
    let fg = parse_color(foreground).map_err(|e| format!("foreground: {e}"))?;
    let bg = parse_color(background).map_err(|e| format!("background: {e}"))?;
    let report = check(fg, bg);
    match format.trim().to_ascii_lowercase().as_str() {
        "" | "text" => Ok(report.to_text()),
        "json" => Ok(report.to_json()),
        "suggest" => {
            let tgt = Target::parse(target)?;
            let label = match tgt {
                Target::Aa => "AA (normal text, 4.5:1)",
                Target::Aaa => "AAA (normal text, 7:1)",
                Target::Large => "AA large / UI (3:1)",
            };
            let body = report.to_text();
            match suggest_passing(fg, bg, tgt) {
                Some(c) if c == fg => Ok(format!(
                    "{body}\n\nSuggestion: already meets {label} — no change needed."
                )),
                Some(c) => {
                    let r = round2(contrast_ratio(c, bg));
                    Ok(format!(
                        "{body}\n\nSuggested foreground for {label}: {hex} (rgb {r0},{g0},{b0}) — ratio {r}:1, same hue, lightness adjusted.",
                        hex = c.to_hex(),
                        r0 = c.r,
                        g0 = c.g,
                        b0 = c.b,
                    ))
                }
                None => Ok(format!(
                    "{body}\n\nNo foreground of this hue/saturation can reach {label} against {bg_hex}; try changing the background.",
                    bg_hex = bg.to_hex(),
                )),
            }
        }
        other => Err(format!(
            "invalid format {other:?}: expected \"text\", \"json\", or \"suggest\""
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(r: u8, g: u8, b: u8) -> Rgb {
        Rgb { r, g, b }
    }

    #[test]
    fn black_on_white_is_max_ratio() {
        let ratio = contrast_ratio(rgb(0, 0, 0), rgb(255, 255, 255));
        assert!((ratio - 21.0).abs() < 1e-9, "got {ratio}");
        // order-independent
        let rev = contrast_ratio(rgb(255, 255, 255), rgb(0, 0, 0));
        assert!((ratio - rev).abs() < 1e-12);
    }

    #[test]
    fn identical_colors_are_ratio_one() {
        assert!((contrast_ratio(rgb(18, 52, 86), rgb(18, 52, 86)) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn known_wcag_pair() {
        // #767676 on #ffffff is the canonical "just passes AA normal" grey (4.54:1).
        let r = contrast_ratio(rgb(0x76, 0x76, 0x76), rgb(255, 255, 255));
        assert!((r - 4.54).abs() < 0.02, "got {r}");
    }

    #[test]
    fn check_flags_levels() {
        // black on white passes everything.
        let rep = check(rgb(0, 0, 0), rgb(255, 255, 255));
        assert!(
            rep.aa_normal && rep.aaa_normal && rep.aa_large && rep.aaa_large && rep.ui_components
        );
        assert_eq!(rep.summary_level(), "AAA");
    }

    #[test]
    fn check_fails_low_contrast() {
        // light grey on white — fails normal AA.
        let rep = check(rgb(0xaa, 0xaa, 0xaa), rgb(255, 255, 255));
        assert!(!rep.aa_normal);
        assert_eq!(rep.summary_level(), "Fail");
    }

    #[test]
    fn parse_hex_forms() {
        assert_eq!(parse_color("#1a2b3c").unwrap(), rgb(0x1a, 0x2b, 0x3c));
        assert_eq!(parse_color("1A2B3C").unwrap(), rgb(0x1a, 0x2b, 0x3c));
        // #rgb shorthand expands each digit.
        assert_eq!(parse_color("#fff").unwrap(), rgb(255, 255, 255));
        assert_eq!(parse_color("#0a0").unwrap(), rgb(0, 0xaa, 0));
    }

    #[test]
    fn parse_rgb_forms() {
        assert_eq!(parse_color("rgb(26, 43, 60)").unwrap(), rgb(26, 43, 60));
        assert_eq!(parse_color("26,43,60").unwrap(), rgb(26, 43, 60));
        assert_eq!(parse_color("rgba(0, 170, 0)").unwrap(), rgb(0, 170, 0));
    }

    #[test]
    fn to_hex_round_trips() {
        assert_eq!(rgb(0x1a, 0x2b, 0x3c).to_hex(), "#1a2b3c");
        assert_eq!(parse_color("rgb(255,255,255)").unwrap().to_hex(), "#ffffff");
    }

    #[test]
    fn rejects_bad_hex() {
        assert!(parse_color("#12345").is_err());
        assert!(parse_color("#xyzxyz").is_err());
        assert!(parse_color("").is_err());
    }

    #[test]
    fn rejects_out_of_range_rgb() {
        assert!(parse_color("rgb(300, 0, 0)").is_err());
        assert!(parse_color("rgb(1, 2)").is_err());
    }

    #[test]
    fn parse_hsl_and_named() {
        // hsl(0,100%,50%) is pure red.
        assert_eq!(parse_color("hsl(0, 100%, 50%)").unwrap(), rgb(255, 0, 0));
        // hsl(120,100%,25%) ~ dark green (#008000).
        assert_eq!(parse_color("hsl(120, 100%, 25%)").unwrap(), rgb(0, 128, 0));
        // Named colours, case-insensitive.
        assert_eq!(parse_color("white").unwrap(), rgb(255, 255, 255));
        assert_eq!(parse_color("Navy").unwrap(), rgb(0, 0, 128));
        assert_eq!(parse_color("rebeccapurple").unwrap(), rgb(102, 51, 153));
    }

    #[test]
    fn hsl_round_trips_through_rgb() {
        let c = rgb(70, 130, 180); // steelblue
        let (h, s, l) = rgb_to_hsl(c);
        let back = hsl_to_rgb(h, s, l);
        // Allow a 1-unit rounding wobble per channel.
        assert!((back.r as i32 - c.r as i32).abs() <= 1);
        assert!((back.g as i32 - c.g as i32).abs() <= 1);
        assert!((back.b as i32 - c.b as i32).abs() <= 1);
    }

    #[test]
    fn rejects_unknown_name() {
        let e = parse_color("notacolorname").unwrap_err();
        assert!(e.contains("unknown colour name"), "got: {e}");
    }

    #[test]
    fn suggest_keeps_passing_color() {
        // black on white already passes AAA — returned unchanged.
        let got = suggest_passing(rgb(0, 0, 0), rgb(255, 255, 255), Target::Aaa);
        assert_eq!(got, Some(rgb(0, 0, 0)));
    }

    #[test]
    fn suggest_nudges_failing_color_to_pass() {
        // light grey on white fails AA — suggestion must reach >= 4.5:1, same hue.
        let fg = rgb(0xaa, 0xaa, 0xaa);
        let bg = rgb(255, 255, 255);
        let s = suggest_passing(fg, bg, Target::Aa).expect("a darker grey passes");
        assert!(contrast_ratio(s, bg) >= 4.5, "ratio {}", contrast_ratio(s, bg));
        // Greyscale hue/sat preserved (all channels equal).
        assert!(s.r == s.g && s.g == s.b, "stayed grey: {s:?}");
        // It got darker than the original.
        assert!(s.r < fg.r);
    }

    #[test]
    fn run_text_and_json() {
        let txt = run("#000000", "#ffffff", "text", "").unwrap();
        assert!(txt.contains("21:1"));
        assert!(txt.contains("Best level for normal text: AAA"));

        let json = run("#000", "#fff", "json", "").unwrap();
        assert!(json.contains(r##""ratio":21"##), "got: {json}");
        assert!(json.contains(r##""foreground":"#000000""##), "got: {json}");
        assert!(json.contains(r##""aa_normal":true"##), "got: {json}");
        assert!(json.contains(r##""summary":"AAA""##), "got: {json}");
    }

    #[test]
    fn run_suggest_mode() {
        // failing pair → a suggestion line with a passing colour.
        let out = run("#aaaaaa", "#ffffff", "suggest", "aa").unwrap();
        assert!(out.contains("Suggested foreground"), "got: {out}");
        // already-passing pair → "no change needed".
        let out = run("#000", "#fff", "suggest", "aaa").unwrap();
        assert!(out.contains("no change needed"), "got: {out}");
    }

    #[test]
    fn run_suggest_rejects_bad_target() {
        let e = run("#000", "#fff", "suggest", "platinum").unwrap_err();
        assert!(e.contains("invalid target"), "got: {e}");
    }

    #[test]
    fn run_propagates_color_errors() {
        let e = run("nothex", "#fff", "text", "").unwrap_err();
        assert!(e.starts_with("foreground:"), "got: {e}");
        let e = run("#fff", "zzz", "text", "").unwrap_err();
        assert!(e.starts_with("background:"), "got: {e}");
    }

    #[test]
    fn run_rejects_bad_format() {
        let e = run("#000", "#fff", "yaml", "").unwrap_err();
        assert!(e.contains("invalid format"), "got: {e}");
    }
}
