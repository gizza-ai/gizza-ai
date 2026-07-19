//! gizza-ai/image-film-grain core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Wraps ffmpeg's `noise` filter to overlay analog film grain on a photo. Users
//! pass a friendly `amount` 0–100 that maps directly onto the noise filter's
//! strength (`alls`/`c0s`, also 0–100), plus a `monochrome` toggle:
//!
//! - **monochrome** (default) — grain is applied to the LUMA plane only
//!   (`format=yuv444p,noise=c0s=<n>:c0f=u`), so the chroma is untouched and the
//!   grain reads as neutral gray brightness speckle — the classic B&W film look
//!   that stays natural over color photos.
//! - **color** — grain is applied to every channel
//!   (`noise=alls=<n>:allf=u`), producing colored speckle (RGB channels drift
//!   independently), the look of pushed high-ISO color film.
//!
//! The `u` flag makes the noise uniformly distributed and the default fixed
//! seed keeps every surface (chat, CLI, page) deterministic for the same input.
//! `format` (`keep|png|jpg|webp`) optionally converts the output like the other
//! image filters. Every filter string is a single argv token (no spaces).

/// The canonical output-format names, in display order. Used for the schema
/// enum and the page `<select>`. KEEP IN SYNC with `parse_format`.
pub const FORMATS: [&str; 4] = ["keep", "png", "jpg", "webp"];

/// The default output format (keep the input container).
pub const DEFAULT_FORMAT: &str = "keep";

/// The default grain amount — a moderate, film-like speckle.
pub const DEFAULT_AMOUNT: f64 = 20.0;

/// The default grain mode: monochrome (luma-only) grain, the classic film look.
pub const DEFAULT_MONOCHROME: bool = true;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OutFormat {
    Keep,
    Png,
    Jpg,
    Webp,
}

/// Parse an output format name (case-insensitive; `jpeg` is an alias for
/// `jpg`). `None` / `""` default to `keep` (preserve the input container).
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

/// Parse a boolean toggle from a page/CLI string. `None`/`""` default to
/// `default_val`; positive-truthy (`true/1/on/yes`) → true, the negative forms
/// (`false/0/off/no`) → false; anything else is a guided error.
pub fn parse_bool(s: Option<&str>, default_val: bool) -> Result<bool, String> {
    match s.map(|v| v.trim().to_ascii_lowercase()) {
        None => Ok(default_val),
        Some(v) if v.is_empty() => Ok(default_val),
        Some(v) => match v.as_str() {
            "true" | "1" | "on" | "yes" => Ok(true),
            "false" | "0" | "off" | "no" => Ok(false),
            other => Err(format!(
                "invalid boolean {other:?}; expected true or false"
            )),
        },
    }
}

/// Validate the friendly `amount` (0–100) and return the integer noise strength
/// the ffmpeg filter takes (also 0–100). 0 = no grain (a re-encode), 100 = the
/// filter's maximum.
pub fn strength_for_amount(amount: f64) -> Result<u32, String> {
    if !amount.is_finite() || !(0.0..=100.0).contains(&amount) {
        return Err(format!(
            "amount must be a number between 0 and 100, got {amount}"
        ));
    }
    Ok(amount.round() as u32)
}

/// The ffmpeg `-vf` filter string that overlays film grain. Single argv token.
///
/// Monochrome grain rounds through `yuv444p` and noises only the luma plane
/// (`c0s`), leaving chroma flat — neutral gray speckle. Color grain noises all
/// channels (`alls`), so the RGB channels drift independently. The `u` flag =
/// uniform distribution; the filter's fixed default seed keeps it deterministic.
pub fn filter(amount: f64, monochrome: bool) -> Result<String, String> {
    let n = strength_for_amount(amount)?;
    Ok(if monochrome {
        format!("format=yuv444p,noise=c0s={n}:c0f=u")
    } else {
        format!("noise=alls={n}:allf=u")
    })
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

/// Build the ffmpeg argv (no leading "ffmpeg") and the output filename for
/// grain-ing `in_name`. `format` `keep` reuses the input extension; explicit
/// formats convert (still image only — first frame of an animated input).
pub fn plan(
    in_name: &str,
    amount: f64,
    monochrome: bool,
    format: OutFormat,
) -> Result<(Vec<String>, String), String> {
    let vf = filter(amount, monochrome)?;
    let in_ext = in_name
        .rsplit('.')
        .next()
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
        // mjpeg's default quality is visibly lossy; pin a high-quality encode so
        // the grain we just added isn't smeared by block artifacts.
        argv.push("-q:v".to_string());
        argv.push("2".to_string());
    }
    if format != OutFormat::Keep {
        // Explicit formats are still images: take one frame so an animated
        // input (GIF) converts cleanly instead of erroring in the image muxer.
        argv.push("-frames:v".to_string());
        argv.push("1".to_string());
    }
    argv.push(out_name.clone());
    Ok((argv, out_name))
}

/// Parse + plan in one step from raw strings (used by the web page and CLI
/// paths where every field arrives as text).
pub fn plan_named(
    in_name: &str,
    amount: f64,
    monochrome: Option<&str>,
    format: Option<&str>,
) -> Result<(Vec<String>, String), String> {
    let monochrome = parse_bool(monochrome, DEFAULT_MONOCHROME)?;
    let format = parse_format(format)?;
    plan(in_name, amount, monochrome, format)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strength_maps_amount_to_integer_noise() {
        assert_eq!(strength_for_amount(0.0).unwrap(), 0);
        assert_eq!(strength_for_amount(20.0).unwrap(), 20);
        assert_eq!(strength_for_amount(100.0).unwrap(), 100);
        // Rounds to the nearest integer noise strength.
        assert_eq!(strength_for_amount(19.4).unwrap(), 19);
        assert_eq!(strength_for_amount(19.6).unwrap(), 20);
    }

    #[test]
    fn strength_out_of_range_is_rejected() {
        for bad in [-1.0, 100.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(strength_for_amount(bad).is_err(), "{bad} should be rejected");
        }
        let err = strength_for_amount(250.0).unwrap_err();
        assert!(err.contains("0") && err.contains("100"));
    }

    #[test]
    fn monochrome_filter_noises_luma_only() {
        // Luma-only path rounds through yuv444p and uses c0s (component 0 = Y).
        let f = filter(20.0, true).unwrap();
        assert_eq!(f, "format=yuv444p,noise=c0s=20:c0f=u");
        assert!(!f.contains("alls"), "monochrome must not noise all channels: {f}");
    }

    #[test]
    fn color_filter_noises_all_channels() {
        let f = filter(30.0, false).unwrap();
        assert_eq!(f, "noise=alls=30:allf=u");
        assert!(!f.contains("c0s"), "color grain uses alls, not a single plane: {f}");
    }

    #[test]
    fn filter_is_a_single_argv_token() {
        assert!(!filter(100.0, true).unwrap().contains(' '));
        assert!(!filter(100.0, false).unwrap().contains(' '));
    }

    #[test]
    fn parse_bool_default_and_forms() {
        assert!(parse_bool(None, true).unwrap());
        assert!(!parse_bool(None, false).unwrap());
        assert!(parse_bool(Some(""), true).unwrap());
        assert!(parse_bool(Some("true"), false).unwrap());
        assert!(parse_bool(Some(" TRUE "), false).unwrap());
        assert!(parse_bool(Some("1"), false).unwrap());
        assert!(!parse_bool(Some("false"), true).unwrap());
        assert!(!parse_bool(Some("0"), true).unwrap());
        assert!(!parse_bool(Some("off"), true).unwrap());
    }

    #[test]
    fn parse_bool_rejects_unknown() {
        let err = parse_bool(Some("maybe"), true).unwrap_err();
        assert!(err.contains("true") && err.contains("false"), "{err}");
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
    fn plan_argv_structure_and_extension() {
        let (argv, out) = plan("photo.png", 20.0, true, OutFormat::Keep).unwrap();
        assert_eq!(out, "out.png");
        assert_eq!(&argv[0], "-i");
        assert_eq!(&argv[1], "photo.png");
        assert_eq!(&argv[2], "-vf");
        assert_eq!(&argv[3], "format=yuv444p,noise=c0s=20:c0f=u");
        assert_eq!(argv.last().unwrap(), "out.png");
        // png keep: no jpg quality flag, no frame cap.
        assert!(!argv.contains(&"-q:v".to_string()));
        assert!(!argv.contains(&"-frames:v".to_string()));
    }

    #[test]
    fn plan_keeps_jpg_quality_and_extension() {
        let (argv, out) = plan("photo.jpg", 40.0, false, OutFormat::Keep).unwrap();
        assert_eq!(out, "out.jpg");
        assert_eq!(&argv[3], "noise=alls=40:allf=u");
        // Keeping a jpg still pins the high-quality re-encode…
        assert!(argv.windows(2).any(|w| w == ["-q:v", "2"]), "{argv:?}");
        // …but no -frames cap: keep never drops animation.
        assert!(!argv.contains(&"-frames:v".to_string()), "{argv:?}");
    }

    #[test]
    fn plan_converts_formats_as_single_frames() {
        let (argv, out) = plan("in.png", 20.0, true, OutFormat::Jpg).unwrap();
        assert_eq!(out, "out.jpg");
        assert!(argv.windows(2).any(|w| w == ["-q:v", "2"]), "{argv:?}");
        assert!(argv.windows(2).any(|w| w == ["-frames:v", "1"]), "{argv:?}");
        let (argv, out) = plan("anim.gif", 20.0, true, OutFormat::Png).unwrap();
        assert_eq!(out, "out.png");
        assert!(argv.windows(2).any(|w| w == ["-frames:v", "1"]), "{argv:?}");
        let (argv, out) = plan("in.jpg", 20.0, true, OutFormat::Webp).unwrap();
        assert_eq!(out, "out.webp");
        assert!(!argv.contains(&"-q:v".to_string()), "webp keeps encoder defaults: {argv:?}");
    }

    #[test]
    fn plan_propagates_validation_errors() {
        assert!(plan("in.png", 101.0, true, OutFormat::Keep).is_err());
        assert!(plan("in.png", -5.0, false, OutFormat::Keep).is_err());
    }

    #[test]
    fn plan_named_parses_and_plans() {
        let (argv, out) = plan_named("in.webp", 20.0, Some("false"), None).unwrap();
        assert_eq!(out, "out.webp");
        assert!(argv.iter().any(|a| a == "noise=alls=20:allf=u"));
        // Default monochrome (None) uses the luma-only path.
        let (argv, _) = plan_named("in.png", 20.0, None, None).unwrap();
        assert!(argv.iter().any(|a| a == "format=yuv444p,noise=c0s=20:c0f=u"));
        // Page-style call: every field as text, converting to jpg.
        let (argv, out) = plan_named("photo.jpeg", 60.0, Some("true"), Some("jpg")).unwrap();
        assert_eq!(out, "out.jpg");
        assert!(argv.iter().any(|a| a == "format=yuv444p,noise=c0s=60:c0f=u"));
        // Bad values are rejected.
        assert!(plan_named("in.png", 20.0, Some("maybe"), None).is_err());
        assert!(plan_named("in.png", 20.0, None, Some("tiff")).is_err());
        assert!(plan_named("in.png", 200.0, None, None).is_err());
    }
}
