//! css-gradient-generator core — builds a CSS gradient declaration from a
//! gradient type, an orientation (angle for linear / position for radial &
//! conic), and a list of color stops. Pure compute, no wafer/wasm-bindgen deps;
//! shared by the chat skill block and the web page.
//!
//! The output is a CSS `background-image` value, e.g.
//! `linear-gradient(90deg, #ff0000 0%, #0000ff 100%)`, plus a full
//! `background-image: …;` declaration line. No I/O — runs entirely in-sandbox.

/// Maximum number of color stops accepted. Generous but bounded so a runaway
/// list can't blow up the output.
pub const MAX_STOPS: usize = 64;

/// A parsed color stop: the color token plus an optional explicit position
/// (already normalised to a CSS length/percentage string such as `0%`).
#[derive(Debug, Clone, PartialEq)]
struct Stop {
    color: String,
    position: Option<String>,
}

/// Build a CSS gradient.
///
/// - `gradient_type` (`"linear"` | `"radial"` | `"conic"`, blank → `"linear"`).
/// - `angle`: for `linear`, the direction in degrees (e.g. `90` → `90deg`,
///   left-to-right). Ignored for `radial`. For `conic`, used as the starting
///   angle (`from <angle>deg`). Blank → `180` for linear (top-to-bottom, the CSS
///   default) and `0` for conic.
/// - `shape` (`radial` only, `"circle"` | `"ellipse"`, blank → `"ellipse"`):
///   the radial gradient shape.
/// - `colors`: the color stops, one per line OR comma-separated. Each entry is a
///   CSS color (`#f00`, `#ff0000`, `red`, `rgb(255,0,0)`, `hsl(...)`) optionally
///   followed by a position (`#f00 25%`, `red 0px`). With 2+ colors and no
///   explicit positions, evenly-spaced percentages are added.
/// - `repeating`: when `true`, emit a `repeating-*-gradient(...)` (needs at least
///   one explicit position to be meaningful, but is allowed regardless).
/// - `interpolation` (blank → none): the color-interpolation color space, e.g.
///   `"oklch"`, `"oklab"`, `"srgb"`, `"hsl"`. When set, an `in <space>` clause is
///   prepended to the gradient (e.g. `linear-gradient(in oklch 90deg, …)`) for a
///   perceptually-smoother blend on modern browsers. For `hsl`/`hwb`/`lch`/`oklch`
///   a hue-interpolation method (`shorter`/`longer`/`increasing`/`decreasing hue`)
///   may be appended.
///
/// Returns the full CSS declaration: `background-image: <gradient>;`.
pub fn generate(
    gradient_type: &str,
    angle: f64,
    shape: &str,
    colors: &str,
    repeating: bool,
    interpolation: &str,
) -> Result<String, String> {
    let gt = gradient_type.trim().to_ascii_lowercase();
    let gt = if gt.is_empty() { "linear" } else { gt.as_str() };
    if !matches!(gt, "linear" | "radial" | "conic") {
        return Err(format!(
            "invalid gradient_type '{gradient_type}': use 'linear', 'radial', or 'conic'"
        ));
    }

    let stops = parse_stops(colors)?;
    if stops.len() < 2 {
        return Err("at least 2 color stops are required".into());
    }

    let func = match (gt, repeating) {
        ("linear", false) => "linear-gradient",
        ("linear", true) => "repeating-linear-gradient",
        ("radial", false) => "radial-gradient",
        ("radial", true) => "repeating-radial-gradient",
        ("conic", false) => "conic-gradient",
        ("conic", true) => "repeating-conic-gradient",
        _ => unreachable!(),
    };

    // Add evenly-spaced positions only when NO stop carries an explicit one;
    // mixing auto + explicit would be surprising, so leave authored positions
    // untouched.
    let any_explicit = stops.iter().any(|s| s.position.is_some());
    let stop_strs: Vec<String> = if any_explicit {
        stops
            .iter()
            .map(|s| match &s.position {
                Some(p) => format!("{} {}", s.color, p),
                None => s.color.clone(),
            })
            .collect()
    } else {
        let n = stops.len();
        stops
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let pct = (i as f64) * 100.0 / ((n - 1) as f64);
                format!("{} {}%", s.color, fmt_num(pct))
            })
            .collect()
    };
    let stops_joined = stop_strs.join(", ");

    let interp = normalize_interpolation(interpolation)?;

    let inner = match gt {
        "linear" => {
            let a = if angle.is_nan() { 180.0 } else { angle };
            format!("{}{}deg, {}", interp, fmt_num(a), stops_joined)
        }
        "radial" => {
            let sh = shape.trim().to_ascii_lowercase();
            let sh = if sh.is_empty() { "ellipse" } else { sh.as_str() };
            if !matches!(sh, "circle" | "ellipse") {
                return Err(format!(
                    "invalid shape '{shape}': use 'circle' or 'ellipse'"
                ));
            }
            format!("{}{} at center, {}", interp, sh, stops_joined)
        }
        "conic" => {
            let a = if angle.is_nan() { 0.0 } else { angle };
            format!("{}from {}deg at center, {}", interp, fmt_num(a), stops_joined)
        }
        _ => unreachable!(),
    };

    let gradient = format!("{func}({inner})");
    Ok(format!("background-image: {gradient};"))
}

/// Parse the `colors` field into stops. Entries are separated by newlines or
/// commas (but a comma INSIDE `rgb(...)`/`hsl(...)` is not a separator). Each
/// entry is `<color>` or `<color> <position>`.
fn parse_stops(colors: &str) -> Result<Vec<Stop>, String> {
    let mut out = Vec::new();
    for entry in split_entries(colors) {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        out.push(parse_stop(entry)?);
        if out.len() > MAX_STOPS {
            return Err(format!("too many color stops (max {MAX_STOPS})"));
        }
    }
    Ok(out)
}

/// Split on newlines and on commas that are NOT inside parentheses (so
/// `rgb(255, 0, 0)` stays one token).
fn split_entries(colors: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth: i32 = 0;
    for ch in colors.chars() {
        match ch {
            '(' => {
                depth += 1;
                cur.push(ch);
            }
            ')' => {
                depth = (depth - 1).max(0);
                cur.push(ch);
            }
            ',' | '\n' if depth == 0 => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(ch),
        }
    }
    out.push(cur);
    out
}

/// Parse one `<color>` or `<color> <position>` entry. The position is detected
/// as a trailing token ending in `%` or a CSS length unit, or a bare number.
/// A `rgb(...)`/`hsl(...)` token keeps its inner spaces.
fn parse_stop(entry: &str) -> Result<Stop, String> {
    // If the entry ends with `)`, it's a single function color with no position.
    if entry.ends_with(')') {
        validate_color(entry)?;
        return Ok(Stop {
            color: entry.to_string(),
            position: None,
        });
    }
    // Otherwise split on the LAST whitespace run; if the tail looks like a
    // position, treat it as one — else the whole thing is the color.
    if let Some(idx) = entry.rfind(char::is_whitespace) {
        let (head, tail) = entry.split_at(idx);
        let head = head.trim();
        let tail = tail.trim();
        if !head.is_empty() && looks_like_position(tail) {
            validate_color(head)?;
            return Ok(Stop {
                color: head.to_string(),
                position: Some(normalize_position(tail)),
            });
        }
    }
    validate_color(entry)?;
    Ok(Stop {
        color: entry.to_string(),
        position: None,
    })
}

/// Is `t` a CSS gradient stop position? Accepts `<num>%`, a bare `<num>` (treated
/// as a percentage), or `<num><css-length-unit>`.
fn looks_like_position(t: &str) -> bool {
    if t.is_empty() {
        return false;
    }
    if let Some(num) = t.strip_suffix('%') {
        return is_number(num);
    }
    for unit in ["px", "em", "rem", "vw", "vh", "vmin", "vmax", "cm", "mm", "in", "pt", "pc", "ch", "ex"] {
        if let Some(num) = t.strip_suffix(unit) {
            return is_number(num);
        }
    }
    is_number(t)
}

/// A bare number → treat as a percentage; a unit-bearing/`%` value is kept verbatim.
fn normalize_position(t: &str) -> String {
    if is_number(t) {
        format!("{t}%")
    } else {
        t.to_string()
    }
}

fn is_number(s: &str) -> bool {
    let s = s.trim();
    !s.is_empty() && s.parse::<f64>().is_ok()
}

/// Light validation: reject obviously-empty or unbalanced color tokens. We don't
/// fully validate every CSS color name (the browser does that) — just guard the
/// structural cases so the output is never malformed.
fn validate_color(c: &str) -> Result<(), String> {
    let c = c.trim();
    if c.is_empty() {
        return Err("empty color".into());
    }
    let opens = c.matches('(').count();
    let closes = c.matches(')').count();
    if opens != closes {
        return Err(format!("malformed color '{c}': unbalanced parentheses"));
    }
    if let Some(hex) = c.strip_prefix('#') {
        let n = hex.len();
        if !(n == 3 || n == 4 || n == 6 || n == 8) || !hex.chars().all(|h| h.is_ascii_hexdigit()) {
            return Err(format!(
                "invalid hex color '{c}': use #rgb, #rgba, #rrggbb, or #rrggbbaa"
            ));
        }
    }
    Ok(())
}

/// Build the `in <space> [<hue> hue] ` clause from the `interpolation` field, or
/// an empty string when blank. Accepts a color space optionally followed by a hue
/// keyword, e.g. `"oklch"`, `"hsl longer"`, `"oklch increasing"`. Returns a
/// trailing-space string so it slots in directly before the angle/shape.
fn normalize_interpolation(interpolation: &str) -> Result<String, String> {
    let t = interpolation.trim().to_ascii_lowercase();
    if t.is_empty() || t == "none" {
        return Ok(String::new());
    }
    let mut it = t.split_whitespace();
    let space = it.next().unwrap();
    const SPACES: &[&str] = &[
        "srgb", "srgb-linear", "display-p3", "a98-rgb", "prophoto-rgb", "rec2020", "lab", "oklab",
        "xyz", "xyz-d50", "xyz-d65", "hsl", "hwb", "lch", "oklch",
    ];
    if !SPACES.contains(&space) {
        return Err(format!(
            "invalid interpolation color space '{space}': use one of srgb, srgb-linear, lab, oklab, hsl, hwb, lch, oklch, display-p3, rec2020, xyz, …"
        ));
    }
    // Polar spaces (hue-bearing) may carry a hue-interpolation keyword.
    let polar = matches!(space, "hsl" | "hwb" | "lch" | "oklch");
    let mut clause = format!("in {space}");
    if let Some(hue) = it.next() {
        if !polar {
            return Err(format!(
                "color space '{space}' has no hue channel, so a hue keyword '{hue}' is not allowed"
            ));
        }
        if !matches!(hue, "shorter" | "longer" | "increasing" | "decreasing") {
            return Err(format!(
                "invalid hue interpolation '{hue}': use shorter, longer, increasing, or decreasing"
            ));
        }
        clause.push_str(&format!(" {hue} hue"));
    }
    if it.next().is_some() {
        return Err(format!(
            "interpolation '{interpolation}' has too many tokens: expected '<space>' or '<space> <hue>'"
        ));
    }
    clause.push(' ');
    Ok(clause)
}

/// Format a number with no trailing `.0` and at most 4 decimals.
fn fmt_num(n: f64) -> String {
    if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        let s = format!("{n:.4}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_default_angle_and_even_stops() {
        let out = generate("linear", f64::NAN, "", "#f00\n#00f", false, "").unwrap();
        assert_eq!(
            out,
            "background-image: linear-gradient(180deg, #f00 0%, #00f 100%);"
        );
    }

    #[test]
    fn linear_explicit_angle_and_three_stops() {
        let out = generate("linear", 90.0, "", "red, green, blue", false, "").unwrap();
        assert_eq!(
            out,
            "background-image: linear-gradient(90deg, red 0%, green 50%, blue 100%);"
        );
    }

    #[test]
    fn explicit_positions_preserved() {
        let out = generate("linear", 45.0, "", "#fff 10%\n#000 90%", false, "").unwrap();
        assert_eq!(
            out,
            "background-image: linear-gradient(45deg, #fff 10%, #000 90%);"
        );
    }

    #[test]
    fn bare_number_position_becomes_percent() {
        let out = generate("linear", 0.0, "", "red 0, blue 100", false, "").unwrap();
        assert_eq!(
            out,
            "background-image: linear-gradient(0deg, red 0%, blue 100%);"
        );
    }

    #[test]
    fn radial_circle() {
        let out = generate("radial", 0.0, "circle", "#f00, #00f", false, "").unwrap();
        assert_eq!(
            out,
            "background-image: radial-gradient(circle at center, #f00 0%, #00f 100%);"
        );
    }

    #[test]
    fn radial_default_ellipse() {
        let out = generate("radial", 0.0, "", "#f00, #00f", false, "").unwrap();
        assert!(out.contains("radial-gradient(ellipse at center,"));
    }

    #[test]
    fn conic_with_from_angle() {
        let out = generate("conic", 90.0, "", "red, yellow, red", false, "").unwrap();
        assert_eq!(
            out,
            "background-image: conic-gradient(from 90deg at center, red 0%, yellow 50%, red 100%);"
        );
    }

    #[test]
    fn repeating_linear() {
        let out = generate("linear", 45.0, "", "#000 0px, #fff 10px", true, "").unwrap();
        assert_eq!(
            out,
            "background-image: repeating-linear-gradient(45deg, #000 0px, #fff 10px);"
        );
    }

    #[test]
    fn rgb_function_color_with_inner_commas() {
        let out = generate("linear", 90.0, "", "rgb(255, 0, 0), rgb(0, 0, 255)", false, "").unwrap();
        assert_eq!(
            out,
            "background-image: linear-gradient(90deg, rgb(255, 0, 0) 0%, rgb(0, 0, 255) 100%);"
        );
    }

    #[test]
    fn fractional_even_positions() {
        let out = generate("linear", 90.0, "", "a, b, c, d", false, "").unwrap();
        // 0, 33.3333, 66.6667, 100
        assert_eq!(
            out,
            "background-image: linear-gradient(90deg, a 0%, b 33.3333%, c 66.6667%, d 100%);"
        );
    }

    #[test]
    fn err_too_few_stops() {
        let e = generate("linear", 90.0, "", "#f00", false, "").unwrap_err();
        assert!(e.contains("at least 2"));
    }

    #[test]
    fn err_invalid_type() {
        let e = generate("spiral", 90.0, "", "#f00, #00f", false, "").unwrap_err();
        assert!(e.contains("invalid gradient_type"));
    }

    #[test]
    fn err_invalid_hex() {
        let e = generate("linear", 90.0, "", "#xyz, #00f", false, "").unwrap_err();
        assert!(e.contains("invalid hex color"));
    }

    #[test]
    fn err_invalid_shape() {
        let e = generate("radial", 0.0, "square", "#f00, #00f", false, "").unwrap_err();
        assert!(e.contains("invalid shape"));
    }

    #[test]
    fn fmt_num_strips_trailing_zeros() {
        assert_eq!(fmt_num(90.0), "90");
        assert_eq!(fmt_num(33.333333), "33.3333");
        assert_eq!(fmt_num(50.5), "50.5");
    }

    #[test]
    fn hex_with_alpha_ok() {
        let out = generate("linear", 90.0, "", "#ff000080, #0000ffff", false, "").unwrap();
        assert!(out.contains("#ff000080 0%"));
    }

    #[test]
    fn interpolation_oklch_clause() {
        let out = generate("linear", 90.0, "", "#f00, #00f", false, "oklch").unwrap();
        assert_eq!(
            out,
            "background-image: linear-gradient(in oklch 90deg, #f00 0%, #00f 100%);"
        );
    }

    #[test]
    fn interpolation_with_hue_method() {
        let out = generate("conic", 0.0, "", "red, blue", false, "hsl longer").unwrap();
        assert_eq!(
            out,
            "background-image: conic-gradient(in hsl longer hue from 0deg at center, red 0%, blue 100%);"
        );
    }

    #[test]
    fn interpolation_radial_clause() {
        let out = generate("radial", 0.0, "circle", "#f00, #00f", false, "srgb").unwrap();
        assert_eq!(
            out,
            "background-image: radial-gradient(in srgb circle at center, #f00 0%, #00f 100%);"
        );
    }

    #[test]
    fn interpolation_blank_is_omitted() {
        let out = generate("linear", 90.0, "", "#f00, #00f", false, "  ").unwrap();
        assert_eq!(
            out,
            "background-image: linear-gradient(90deg, #f00 0%, #00f 100%);"
        );
    }

    #[test]
    fn err_invalid_interpolation_space() {
        let e = generate("linear", 90.0, "", "#f00, #00f", false, "rgb-bad").unwrap_err();
        assert!(e.contains("invalid interpolation color space"));
    }

    #[test]
    fn err_hue_on_non_polar_space() {
        let e = generate("linear", 90.0, "", "#f00, #00f", false, "srgb longer").unwrap_err();
        assert!(e.contains("no hue channel"));
    }

    #[test]
    fn err_invalid_hue_keyword() {
        let e = generate("linear", 90.0, "", "#f00, #00f", false, "oklch sideways").unwrap_err();
        assert!(e.contains("invalid hue interpolation"));
    }
}
