//! gizza-ai/levels-adjust core — apply a photographic "Levels" transfer curve
//! to a list of numeric sample values.
//!
//! This is the numeric math behind the Photoshop/GIMP Levels adjustment,
//! operating on a list of tone values (0–255 by default) rather than a raster
//! image. Three steps, matching the classic reference (Adobe Levels, the GLSL
//! levels shader):
//!
//! 1. **Normalize** each value against the input black/white points:
//!    `norm = (x − input_black) / (input_white − input_black)`.
//!    When `clamp` is set, `norm` is clamped to `[0, 1]` so values below the
//!    black point saturate to black and values above the white point saturate to
//!    white.
//! 2. **Gamma** (midtone) as a power function: `g = norm^(1/gamma)`. gamma > 1
//!    brightens the midtones, gamma < 1 darkens them, without moving pure black
//!    or white. Applied sign-preserving so `clamp = false` extrapolation stays
//!    finite instead of NaN for negative normalized values.
//! 3. **Remap** to the output range: `out = output_black + g · (output_white −
//!    output_black)`. Swapping output black/white (e.g. 255 → 0) inverts tones.
//!
//! Pure-Rust, dependency-free besides serde for the output struct.

use serde::Serialize;

/// Result of a levels adjustment over a list of values.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Adjusted {
    pub count: usize,
    pub input_black: f64,
    pub input_white: f64,
    pub gamma: f64,
    pub output_black: f64,
    pub output_white: f64,
    pub clamp: bool,
    /// The remapped values, in input order.
    pub values: Vec<f64>,
}

fn round6(v: f64) -> f64 {
    let r = (v * 1e6).round() / 1e6;
    // avoid emitting "-0.0"
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// Sign-preserving power: `sign(base) · |base|^exp`. Keeps the result finite for
/// negative bases (which occur only when `clamp` is false and a value sits below
/// the input black point); for the clamped, in-range case `base ∈ [0, 1]` this
/// is exactly `base.powf(exp)`.
fn signed_pow(base: f64, exp: f64) -> f64 {
    if base < 0.0 {
        -(-base).powf(exp)
    } else {
        base.powf(exp)
    }
}

/// Parse whitespace/comma/semicolon-separated numbers.
fn parse_numbers(text: &str) -> Result<Vec<f64>, String> {
    let mut nums = Vec::new();
    for tok in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        let n: f64 = t.parse().map_err(|_| format!("'{t}' is not a number"))?;
        if !n.is_finite() {
            return Err(format!("'{t}' is not a finite number"));
        }
        nums.push(n);
    }
    if nums.is_empty() {
        return Err(
            "no numbers found — provide a list separated by spaces, commas, or newlines".into(),
        );
    }
    Ok(nums)
}

/// Map a single value through the levels transfer curve.
fn map_value(
    x: f64,
    input_black: f64,
    input_white: f64,
    inv_gamma: f64,
    output_black: f64,
    output_white: f64,
    clamp: bool,
) -> f64 {
    let mut norm = (x - input_black) / (input_white - input_black);
    if clamp {
        norm = norm.clamp(0.0, 1.0);
    }
    let g = signed_pow(norm, inv_gamma);
    let mut out = output_black + g * (output_white - output_black);
    if clamp {
        let lo = output_black.min(output_white);
        let hi = output_black.max(output_white);
        out = out.clamp(lo, hi);
    }
    out
}

/// Apply the levels adjustment to a text list of numbers.
///
/// `input_black`/`input_white` set which input values map to output black/white
/// (they must differ). `gamma` is the midtone power (> 0; >1 brightens midtones,
/// <1 darkens). `output_black`/`output_white` set the output range (swap them to
/// invert). `clamp` saturates out-of-range inputs to the output endpoints when
/// true; when false, values extrapolate linearly past the endpoints.
#[allow(clippy::too_many_arguments)]
pub fn adjust(
    text: &str,
    input_black: f64,
    input_white: f64,
    gamma: f64,
    output_black: f64,
    output_white: f64,
    clamp: bool,
) -> Result<Adjusted, String> {
    if !input_black.is_finite()
        || !input_white.is_finite()
        || !output_black.is_finite()
        || !output_white.is_finite()
        || !gamma.is_finite()
    {
        return Err("levels parameters must be finite numbers".into());
    }
    if input_white == input_black {
        return Err(format!(
            "input white ({input_white}) must differ from input black ({input_black}) — the input range cannot be zero"
        ));
    }
    if gamma <= 0.0 {
        return Err(format!("gamma must be greater than 0 (got {gamma})"));
    }
    let nums = parse_numbers(text)?;
    let inv_gamma = 1.0 / gamma;
    let values = nums
        .iter()
        .map(|&x| {
            round6(map_value(
                x,
                input_black,
                input_white,
                inv_gamma,
                output_black,
                output_white,
                clamp,
            ))
        })
        .collect();
    Ok(Adjusted {
        count: nums.len(),
        input_black: round6(input_black),
        input_white: round6(input_white),
        gamma: round6(gamma),
        output_black: round6(output_black),
        output_white: round6(output_white),
        clamp,
        values,
    })
}

/// Human-readable summary (used by the page): a one-line header describing the
/// levels applied, then the remapped values one per line.
#[allow(clippy::too_many_arguments)]
pub fn summary(
    text: &str,
    input_black: f64,
    input_white: f64,
    gamma: f64,
    output_black: f64,
    output_white: f64,
    clamp: bool,
) -> Result<String, String> {
    let r = adjust(
        text,
        input_black,
        input_white,
        gamma,
        output_black,
        output_white,
        clamp,
    )?;
    let values = r
        .values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let header = format!(
        "levels: input {}–{}, gamma {}, output {}–{}{}",
        r.input_black,
        r.input_white,
        r.gamma,
        r.output_black,
        r.output_white,
        if r.clamp { "" } else { " (unclamped)" },
    );
    Ok(format!("{header}\n\n{values}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_default_levels() {
        // 0–255 in, 0–255 out, gamma 1 → values unchanged.
        let r = adjust("0, 64, 128, 192, 255", 0.0, 255.0, 1.0, 0.0, 255.0, true).unwrap();
        assert_eq!(r.count, 5);
        assert_eq!(r.values, vec![0.0, 64.0, 128.0, 192.0, 255.0]);
    }

    #[test]
    fn stretch_contrast() {
        // Map input 64..192 onto 0..255: 64→0, 192→255, 128→127.5.
        let r = adjust("64, 128, 192", 64.0, 192.0, 1.0, 0.0, 255.0, true).unwrap();
        assert_eq!(r.values, vec![0.0, 127.5, 255.0]);
    }

    #[test]
    fn clamp_saturates_out_of_range() {
        // 32 is below the 64 black point, 224 above the 192 white point → saturate.
        let r = adjust("32, 224", 64.0, 192.0, 1.0, 0.0, 255.0, true).unwrap();
        assert_eq!(r.values, vec![0.0, 255.0]);
    }

    #[test]
    fn unclamped_extrapolates() {
        // gamma 1, no clamp → linear extrapolation past the endpoints.
        // 32 → (32-64)/128 = -0.25 → 0 + -0.25*255 = -63.75.
        let r = adjust("32, 224", 64.0, 192.0, 1.0, 0.0, 255.0, false).unwrap();
        assert_eq!(r.values, vec![-63.75, 318.75]);
    }

    #[test]
    fn gamma_brightens_midtones() {
        // gamma 2, midtone 128/255 → norm 0.501961^(1/2) ≈ 0.708 → ~180.6.
        // Endpoints stay put.
        let r = adjust("0, 128, 255", 0.0, 255.0, 2.0, 0.0, 255.0, true).unwrap();
        assert_eq!(r.values.first(), Some(&0.0));
        assert_eq!(r.values.last(), Some(&255.0));
        assert!(r.values[1] > 128.0, "gamma>1 must brighten the midtone");
        let norm = 128.0f64 / 255.0;
        assert_eq!(r.values[1], round6(norm.powf(0.5) * 255.0));
    }

    #[test]
    fn gamma_below_one_darkens_midtones() {
        let r = adjust("128", 0.0, 255.0, 0.5, 0.0, 255.0, true).unwrap();
        assert!(r.values[0] < 128.0, "gamma<1 must darken the midtone");
    }

    #[test]
    fn output_range_inverts_when_swapped() {
        // output black 255, white 0 → invert: 0→255, 255→0, 128→127.
        let r = adjust("0, 128, 255", 0.0, 255.0, 1.0, 255.0, 0.0, true).unwrap();
        assert_eq!(r.values, vec![255.0, 127.0, 0.0]);
    }

    #[test]
    fn output_range_compresses() {
        // Lift blacks / lower whites: 0..255 in → 32..224 out.
        let r = adjust("0, 255", 0.0, 255.0, 1.0, 32.0, 224.0, true).unwrap();
        assert_eq!(r.values, vec![32.0, 224.0]);
    }

    #[test]
    fn zero_input_range_is_error() {
        assert!(adjust("1 2 3", 128.0, 128.0, 1.0, 0.0, 255.0, true).is_err());
    }

    #[test]
    fn nonpositive_gamma_is_error() {
        assert!(adjust("1 2 3", 0.0, 255.0, 0.0, 0.0, 255.0, true).is_err());
        assert!(adjust("1 2 3", 0.0, 255.0, -1.0, 0.0, 255.0, true).is_err());
    }

    #[test]
    fn parse_errors() {
        assert!(adjust("", 0.0, 255.0, 1.0, 0.0, 255.0, true).is_err());
        assert!(adjust("1 2 abc", 0.0, 255.0, 1.0, 0.0, 255.0, true).is_err());
    }

    #[test]
    fn summary_has_header_and_values() {
        let s = summary("0, 128, 255", 0.0, 255.0, 1.0, 0.0, 255.0, true).unwrap();
        assert!(s.contains("levels: input 0–255"));
        assert!(s.contains("gamma 1"));
        assert!(s.contains("128"));
    }

    #[test]
    fn summary_notes_unclamped() {
        let s = summary("100", 0.0, 255.0, 1.0, 0.0, 255.0, false).unwrap();
        assert!(s.contains("(unclamped)"));
    }
}
