//! gizza-ai/gamma-correct core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Applies a gamma (power-law) correction to an image's midtones with ffmpeg's
//! `eq` filter. Gamma is the classic non-linear brightness knob: it lifts or
//! drops the MIDTONES while leaving pure black and pure white fixed, so it never
//! clips the endpoints the way a flat brightness add does. ffmpeg's `eq` maps
//! each channel `out = in^(1/gamma)` (measured: mid-gray 128 → 183 at gamma 1.8,
//! → 54 at gamma 0.5), so:
//! - `gamma > 1` brightens midtones (recovers a dark/underexposed photo)
//! - `gamma < 1` darkens midtones
//! - `gamma = 1` is the identity (no change)
//!
//! Beyond the single overall `gamma`, three extras from the same filter:
//! - **Per-channel gamma** (`gamma_r`/`gamma_g`/`gamma_b`, each 0.1..10, default
//!   1): correct a color cast by lifting one channel's midtones independently.
//!   The overall `gamma` composes with the per-channel one on that channel.
//! - **Gamma weight** (`gamma_weight`, 0..1, default 1): reduces the correction
//!   on the brightest pixels so a strong gamma doesn't blow out highlights. `1`
//!   applies the full curve everywhere; `0` fades the correction out entirely on
//!   the brightest areas.
//! - **Output format** (`keep|png|jpg|webp`): `keep` preserves the input
//!   container; the rest convert (still image — first frame of an animated
//!   input); jpg pins `-q:v 2` for a high-quality re-encode.
//!
//! Everything is deterministic so the chat block, CLI and page produce identical
//! output; the filter string is a single token (no spaces) so it passes cleanly
//! as one argv element.

pub const GAMMA_MIN: f64 = 0.1;
pub const GAMMA_MAX: f64 = 10.0;
pub const WEIGHT_MIN: f64 = 0.0;
pub const WEIGHT_MAX: f64 = 1.0;

pub const DEFAULT_GAMMA: f64 = 1.0;
pub const DEFAULT_WEIGHT: f64 = 1.0;

/// The canonical output-format names, in display order. Used for the schema
/// enum and the page `<select>`. KEEP IN SYNC with `parse_format`.
pub const FORMATS: [&str; 4] = ["keep", "png", "jpg", "webp"];

/// The default output format (keep the input container).
pub const DEFAULT_FORMAT: &str = "keep";

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OutFormat {
    Keep,
    Png,
    Jpg,
    Webp,
}

/// Parse an output format name (case-insensitive; `jpeg` is an alias for `jpg`).
/// `None` / `""` default to `keep` (preserve the input container).
pub fn parse_format(s: Option<&str>) -> Result<OutFormat, String> {
    let v = s.unwrap_or(DEFAULT_FORMAT).trim().to_ascii_lowercase();
    match v.as_str() {
        "" | "keep" => Ok(OutFormat::Keep),
        "png" => Ok(OutFormat::Png),
        "jpg" | "jpeg" => Ok(OutFormat::Jpg),
        "webp" => Ok(OutFormat::Webp),
        other => Err(format!(
            "invalid format {other:?}; expected one of {}",
            FORMATS.join("|")
        )),
    }
}

/// Format an `eq` value as ffmpeg expects it: shortest round-trip decimal, and
/// never a signed zero (`-0` → `0`).
fn fmt(v: f64) -> String {
    let v = if v == 0.0 { 0.0 } else { v }; // collapse -0.0 → 0.0
    v.to_string()
}

fn check_gamma(name: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() || !(GAMMA_MIN..=GAMMA_MAX).contains(&v) {
        return Err(format!(
            "{name} must be a number between {GAMMA_MIN} and {GAMMA_MAX}, got {v}"
        ));
    }
    Ok(())
}

fn check_weight(v: f64) -> Result<(), String> {
    if !v.is_finite() || !(WEIGHT_MIN..=WEIGHT_MAX).contains(&v) {
        return Err(format!(
            "gamma_weight must be a number between {WEIGHT_MIN} and {WEIGHT_MAX}, got {v}"
        ));
    }
    Ok(())
}

/// The ffmpeg `-vf` filter string for the gamma correction. Single argv token.
/// All five `eq` terms are always emitted; at the defaults
/// (`gamma=1:gamma_r=1:gamma_g=1:gamma_b=1:gamma_weight=1`) it is the identity.
pub fn filter(
    gamma: f64,
    gamma_r: f64,
    gamma_g: f64,
    gamma_b: f64,
    gamma_weight: f64,
) -> Result<String, String> {
    check_gamma("gamma", gamma)?;
    check_gamma("gamma_r", gamma_r)?;
    check_gamma("gamma_g", gamma_g)?;
    check_gamma("gamma_b", gamma_b)?;
    check_weight(gamma_weight)?;
    Ok(format!(
        "eq=gamma={}:gamma_r={}:gamma_g={}:gamma_b={}:gamma_weight={}",
        fmt(gamma),
        fmt(gamma_r),
        fmt(gamma_g),
        fmt(gamma_b),
        fmt(gamma_weight),
    ))
}

/// The output extension for `format`, given the input's extension.
fn out_ext<'a>(format: OutFormat, in_ext: &'a str) -> &'a str {
    match format {
        OutFormat::Keep => in_ext,
        OutFormat::Png => "png",
        OutFormat::Jpg => "jpg",
        OutFormat::Webp => "webp",
    }
}

/// Build the ffmpeg argv (no leading "ffmpeg") and the output filename for a
/// gamma correction of `in_name`. `format` `keep` reuses the input extension;
/// explicit formats convert (still image only — first frame of an animated
/// input).
pub fn plan(
    in_name: &str,
    gamma: f64,
    gamma_r: f64,
    gamma_g: f64,
    gamma_b: f64,
    gamma_weight: f64,
    format: OutFormat,
) -> Result<(Vec<String>, String), String> {
    let vf = filter(gamma, gamma_r, gamma_g, gamma_b, gamma_weight)?;
    let in_ext = in_name
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .filter(|e| !e.is_empty())
        .unwrap_or("png");
    let ext = out_ext(format, in_ext);
    let out_name = format!("out.{ext}");
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vf".to_string(),
        vf,
    ];
    if ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg") {
        // mjpeg's default quality is visibly lossy; pin a high-quality encode.
        argv.push("-q:v".to_string());
        argv.push("2".to_string());
    }
    if format != OutFormat::Keep {
        // Explicit formats are still images: take one frame so an animated input
        // (GIF) converts cleanly instead of erroring in the image muxer.
        argv.push("-frames:v".to_string());
        argv.push("1".to_string());
    }
    argv.push(out_name.clone());
    Ok((argv, out_name))
}

/// Parse + plan in one step from raw values (used by the web page and CLI paths).
/// The five gamma values arrive already coerced to `f64`; `format` is text.
pub fn plan_named(
    in_name: &str,
    gamma: f64,
    gamma_r: f64,
    gamma_g: f64,
    gamma_b: f64,
    gamma_weight: f64,
    format: Option<&str>,
) -> Result<(Vec<String>, String), String> {
    let format = parse_format(format)?;
    plan(in_name, gamma, gamma_r, gamma_g, gamma_b, gamma_weight, format)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vf(argv: &[String]) -> String {
        let i = argv.iter().position(|a| a == "-vf").unwrap();
        argv[i + 1].clone()
    }

    #[test]
    fn identity_builds_eq_with_all_terms() {
        let f = filter(1.0, 1.0, 1.0, 1.0, 1.0).unwrap();
        assert_eq!(f, "eq=gamma=1:gamma_r=1:gamma_g=1:gamma_b=1:gamma_weight=1");
    }

    #[test]
    fn worked_example_brightens_midtones() {
        // "Apply gamma 1.8 to this dark photo" → overall 1.8, others identity.
        let f = filter(1.8, 1.0, 1.0, 1.0, 1.0).unwrap();
        assert_eq!(f, "eq=gamma=1.8:gamma_r=1:gamma_g=1:gamma_b=1:gamma_weight=1");
    }

    #[test]
    fn per_channel_and_weight_compose() {
        let f = filter(1.5, 1.2, 1.0, 0.8, 0.5).unwrap();
        assert_eq!(
            f,
            "eq=gamma=1.5:gamma_r=1.2:gamma_g=1:gamma_b=0.8:gamma_weight=0.5"
        );
    }

    #[test]
    fn signed_zero_weight_collapses() {
        let f = filter(1.0, 1.0, 1.0, 1.0, -0.0).unwrap();
        assert!(f.ends_with(":gamma_weight=0"), "{f}");
    }

    #[test]
    fn filter_is_a_single_argv_token() {
        let f = filter(2.2, 0.9, 1.1, 1.3, 0.7).unwrap();
        assert!(!f.contains(' '), "filter must be one argv token: {f}");
    }

    #[test]
    fn boundary_values_are_accepted() {
        assert!(filter(0.1, 0.1, 0.1, 0.1, 0.0).is_ok());
        assert!(filter(10.0, 10.0, 10.0, 10.0, 1.0).is_ok());
    }

    #[test]
    fn out_of_range_values_are_rejected() {
        assert!(filter(0.0, 1.0, 1.0, 1.0, 1.0).is_err()); // gamma < 0.1
        assert!(filter(10.1, 1.0, 1.0, 1.0, 1.0).is_err()); // gamma > 10
        assert!(filter(1.0, 0.05, 1.0, 1.0, 1.0).is_err()); // gamma_r < 0.1
        assert!(filter(1.0, 1.0, 11.0, 1.0, 1.0).is_err()); // gamma_g > 10
        assert!(filter(1.0, 1.0, 1.0, 1.0, -0.1).is_err()); // weight < 0
        assert!(filter(1.0, 1.0, 1.0, 1.0, 1.1).is_err()); // weight > 1
    }

    #[test]
    fn non_finite_is_rejected() {
        assert!(filter(f64::NAN, 1.0, 1.0, 1.0, 1.0).is_err());
        assert!(filter(1.0, f64::INFINITY, 1.0, 1.0, 1.0).is_err());
        assert!(filter(1.0, 1.0, 1.0, 1.0, f64::NAN).is_err());
    }

    #[test]
    fn error_names_the_valid_range() {
        let err = filter(50.0, 1.0, 1.0, 1.0, 1.0).unwrap_err();
        assert!(err.contains("gamma") && err.contains("0.1") && err.contains("10"), "{err}");
        let err = filter(1.0, 1.0, 1.0, 1.0, 5.0).unwrap_err();
        assert!(err.contains("gamma_weight") && err.contains('0') && err.contains('1'), "{err}");
    }

    #[test]
    fn parse_format_default_and_aliases() {
        assert_eq!(parse_format(None).unwrap(), OutFormat::Keep);
        assert_eq!(parse_format(Some("")).unwrap(), OutFormat::Keep);
        assert_eq!(parse_format(Some("PNG")).unwrap(), OutFormat::Png);
        assert_eq!(parse_format(Some("jpeg")).unwrap(), OutFormat::Jpg);
        assert_eq!(parse_format(Some("webp")).unwrap(), OutFormat::Webp);
        assert_eq!(DEFAULT_FORMAT, "keep");
    }

    #[test]
    fn parse_format_rejects_unknown() {
        let err = parse_format(Some("tiff")).unwrap_err();
        assert!(err.contains("keep") && err.contains("webp"), "{err}");
    }

    #[test]
    fn formats_const_round_trips_parser() {
        for name in FORMATS {
            assert!(parse_format(Some(name)).is_ok(), "FORMATS entry {name} must parse");
        }
    }

    #[test]
    fn plan_keeps_input_extension() {
        let (argv, out) = plan("photo.jpg", 1.8, 1.0, 1.0, 1.0, 1.0, OutFormat::Keep).unwrap();
        assert_eq!(out, "out.jpg");
        assert_eq!(&argv[0], "-i");
        assert_eq!(&argv[1], "photo.jpg");
        assert_eq!(&argv[2], "-vf");
        assert_eq!(vf(&argv), "eq=gamma=1.8:gamma_r=1:gamma_g=1:gamma_b=1:gamma_weight=1");
        // Keeping a jpg still pins the high-quality re-encode…
        assert!(argv.windows(2).any(|w| w == ["-q:v", "2"]), "{argv:?}");
        // …but no -frames cap: keep never drops animation.
        assert!(!argv.contains(&"-frames:v".to_string()), "{argv:?}");
        assert_eq!(argv.last().unwrap(), "out.jpg");
    }

    #[test]
    fn plan_keeps_png_extension_without_extra_flags() {
        let (argv, out) = plan("in.png", 1.0, 1.0, 1.0, 1.0, 1.0, OutFormat::Keep).unwrap();
        assert_eq!(out, "out.png");
        assert!(!argv.contains(&"-q:v".to_string()));
        assert!(!argv.contains(&"-frames:v".to_string()));
    }

    #[test]
    fn plan_no_extension_defaults_to_png() {
        let (_, out) = plan("noext", 1.2, 1.0, 1.0, 1.0, 1.0, OutFormat::Keep).unwrap();
        assert_eq!(out, "out.png");
    }

    #[test]
    fn plan_converts_formats_as_single_frames() {
        let (argv, out) = plan("in.png", 1.5, 1.0, 1.0, 1.0, 1.0, OutFormat::Jpg).unwrap();
        assert_eq!(out, "out.jpg");
        assert!(argv.windows(2).any(|w| w == ["-q:v", "2"]), "{argv:?}");
        assert!(argv.windows(2).any(|w| w == ["-frames:v", "1"]), "{argv:?}");
        let (argv, out) = plan("anim.gif", 1.5, 1.0, 1.0, 1.0, 1.0, OutFormat::Png).unwrap();
        assert_eq!(out, "out.png");
        assert!(argv.windows(2).any(|w| w == ["-frames:v", "1"]), "{argv:?}");
        let (argv, out) = plan("in.jpg", 1.5, 1.0, 1.0, 1.0, 1.0, OutFormat::Webp).unwrap();
        assert_eq!(out, "out.webp");
        assert!(!argv.contains(&"-q:v".to_string()), "webp keeps encoder defaults: {argv:?}");
    }

    #[test]
    fn plan_propagates_validation_errors() {
        assert!(plan("in.png", 0.0, 1.0, 1.0, 1.0, 1.0, OutFormat::Keep).is_err());
        assert!(plan("in.png", 1.0, 1.0, 1.0, 1.0, 2.0, OutFormat::Keep).is_err());
    }

    #[test]
    fn plan_named_parses_and_plans() {
        let (argv, out) = plan_named("in.webp", 1.8, 1.0, 1.0, 1.0, 1.0, None).unwrap();
        assert_eq!(out, "out.webp");
        assert!(vf(&argv).starts_with("eq=gamma=1.8"));
        assert!(plan_named("in.png", 1.8, 1.0, 1.0, 1.0, 1.0, Some("tiff")).is_err());
        // Page-style call → convert to png.
        let (_, out) = plan_named("photo.jpeg", 1.2, 1.0, 1.0, 1.0, 1.0, Some("png")).unwrap();
        assert_eq!(out, "out.png");
    }
}
