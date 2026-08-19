//! gizza-ai/edge-detection core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Turns a photo into an edge map — the outline drawing a computer-vision
//! pipeline sees. Three methods, all built on filters ffmpeg ships natively so
//! the CLI, the chat block and the browser page produce identical bytes:
//!
//! * `canny`    — ffmpeg's `edgedetect` in its default *wires* mode: a Gaussian
//!   pre-filter, Sobel gradients, non-maximum suppression and hysteresis
//!   between `low` and `high`. Output is a 1-pixel-wide white edge map on
//!   black. This is the classic Canny detector and the default.
//! * `sobel`    — the raw `sobel` operator: gradient magnitude per pixel, so
//!   edges come out as soft grey ramps whose brightness tracks how strong the
//!   contrast is. No thresholds, no thinning — faster and more forgiving on
//!   soft/blurry input, but noisier.
//! * `colormix` — `edgedetect=mode=colormix`, which keeps the original colors
//!   and paints the detected edges over them for a cartoon/inked-photo look
//!   rather than a bare edge map.
//!
//! Extra knobs beyond the raw filters:
//! - **`blur`** — a `gblur` pre-pass (sigma in pixels) applied BEFORE detection.
//!   Grain and JPEG noise produce spurious edges; 1–2 px of blur removes most of
//!   them. 0 (the default) skips the filter entirely.
//! - **`invert`** — appends `negate`, giving black lines on white: what you want
//!   for printing, coloring pages, laser engraving or vector tracing.
//! - **`format`** — png (default, lossless — a thresholded edge map is exactly
//!   the worst case for JPEG ringing), jpg or webp.
//!
//! `canny` and `sobel` convert to grayscale first (`format=gray`) so the edge
//! map is a genuine single-channel intensity image on every input; `colormix`
//! deliberately keeps color. Every filter string is a single space-free token so
//! it passes cleanly as one argv element.

/// The canonical method names, in display order. Used for the schema enum and
/// the page `<select>` options. KEEP IN SYNC with `parse_method` / `method_name`.
pub const METHODS: [&str; 3] = ["canny", "sobel", "colormix"];

/// The default method applied when no `method` is supplied.
pub const DEFAULT_METHOD: &str = "canny";

/// The canonical output-format names, in display order.
/// KEEP IN SYNC with `parse_format`.
pub const FORMATS: [&str; 3] = ["png", "jpg", "webp"];

/// The default output format. PNG, because an edge map is high-contrast line
/// art — exactly the content JPEG smears.
pub const DEFAULT_FORMAT: &str = "png";

/// ffmpeg's own `edgedetect` defaults, kept as ours so `method=canny` with no
/// thresholds reproduces the reference detector (20/255 and 50/255).
pub const DEFAULT_LOW: f64 = 0.078_431_372_549_019_6;
pub const DEFAULT_HIGH: f64 = 0.196_078_431_372_549;

/// The default pre-blur sigma (none).
pub const DEFAULT_BLUR: f64 = 0.0;

/// Largest accepted pre-blur sigma, in pixels. Beyond this the image is mush
/// and no edges survive, so it is a usability cap rather than a filter limit.
pub const MAX_BLUR: f64 = 10.0;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Method {
    Canny,
    Sobel,
    ColorMix,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OutFormat {
    Png,
    Jpg,
    Webp,
}

/// Accepts the canonical names plus the obvious aliases people type.
/// `None`/empty → the default (canny).
pub fn parse_method(s: Option<&str>) -> Result<Method, String> {
    match s.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "" | "canny" | "wires" | "edges" => Ok(Method::Canny),
        "sobel" | "gradient" | "magnitude" => Ok(Method::Sobel),
        "colormix" | "color" | "overlay" | "cartoon" => Ok(Method::ColorMix),
        other => Err(format!(
            "method {other:?} is not supported — use canny, sobel or colormix"
        )),
    }
}

/// The canonical name of a method (what the schema enum uses).
pub fn method_name(m: Method) -> &'static str {
    match m {
        Method::Canny => "canny",
        Method::Sobel => "sobel",
        Method::ColorMix => "colormix",
    }
}

/// `None`/empty → the default (png).
pub fn parse_format(s: Option<&str>) -> Result<OutFormat, String> {
    match s.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "" | "png" => Ok(OutFormat::Png),
        "jpg" | "jpeg" => Ok(OutFormat::Jpg),
        "webp" => Ok(OutFormat::Webp),
        other => Err(format!(
            "format {other:?} is not supported — use png, jpg or webp"
        )),
    }
}

/// The file extension a format writes.
pub fn format_ext(f: OutFormat) -> &'static str {
    match f {
        OutFormat::Png => "png",
        OutFormat::Jpg => "jpg",
        OutFormat::Webp => "webp",
    }
}

/// Validate a hysteresis threshold. `edgedetect` takes 0–1 fractions of full
/// scale; anything else (or NaN) is a user error worth naming.
fn check_threshold(name: &str, v: f64) -> Result<f64, String> {
    if !v.is_finite() || !(0.0..=1.0).contains(&v) {
        return Err(format!(
            "{name} must be between 0 and 1 (a fraction of full brightness), got {v}"
        ));
    }
    Ok(v)
}

fn check_blur(v: f64) -> Result<f64, String> {
    if !v.is_finite() || !(0.0..=MAX_BLUR).contains(&v) {
        return Err(format!(
            "blur must be between 0 and {MAX_BLUR} pixels of Gaussian sigma, got {v}"
        ));
    }
    Ok(v)
}

/// Render a float for an ffmpeg option without a trailing `.0` where it is an
/// integer, and without scientific notation (which the parser rejects).
fn num(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() || s == "-" {
        "0".to_string()
    } else {
        s
    }
}

/// Build the `-vf` filter chain for the requested edge detection.
///
/// canny/sobel: `format=gray[,gblur=sigma=B],<detector>[,negate]`
/// colormix:    `[gblur=sigma=B,]edgedetect=mode=colormix:…[,negate]`
pub fn filter(
    method: Method,
    low: f64,
    high: f64,
    blur: f64,
    invert: bool,
) -> Result<String, String> {
    let low = check_threshold("low", low)?;
    let high = check_threshold("high", high)?;
    let blur = check_blur(blur)?;
    if high < low {
        return Err(format!(
            "high ({high}) must be greater than or equal to low ({low}) — \
             hysteresis keeps edges above high and grows them down to low"
        ));
    }

    let mut parts: Vec<String> = Vec::new();
    // Grayscale first for the edge-map methods so a color photo yields a true
    // single-channel map instead of three independently-detected channels.
    if method != Method::ColorMix {
        parts.push("format=gray".to_string());
    }
    if blur > 0.0 {
        parts.push(format!("gblur=sigma={}", num(blur)));
    }
    match method {
        Method::Canny => parts.push(format!(
            "edgedetect=low={}:high={}",
            num(low),
            num(high)
        )),
        Method::Sobel => parts.push("sobel".to_string()),
        Method::ColorMix => parts.push(format!(
            "edgedetect=mode=colormix:low={}:high={}",
            num(low),
            num(high)
        )),
    }
    if invert {
        parts.push("negate".to_string());
    }
    Ok(parts.join(","))
}

/// Build the full ffmpeg argv (no leading "ffmpeg") + the output file name.
pub fn plan(
    in_name: &str,
    method: Method,
    low: f64,
    high: f64,
    blur: f64,
    invert: bool,
    format: OutFormat,
) -> Result<(Vec<String>, String), String> {
    let vf = filter(method, low, high, blur, invert)?;
    let out_name = format!("out.{}", format_ext(format));
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vf".to_string(),
        vf,
    ];
    if format == OutFormat::Jpg {
        // mjpeg's default quality visibly rings around thin white lines.
        argv.push("-q:v".to_string());
        argv.push("2".to_string());
    }
    // The output is always a still image: take one frame so an animated input
    // (GIF/WebP) converts cleanly instead of erroring in the image muxer.
    argv.push("-frames:v".to_string());
    argv.push("1".to_string());
    argv.push(out_name.clone());
    Ok((argv, out_name))
}

/// Parse + plan in one step from raw strings (used by the web page and the CLI
/// paths, where every field arrives as text). Empty/absent values take the
/// documented defaults so a cleared field behaves like an unset one.
#[allow(clippy::too_many_arguments)]
pub fn plan_named(
    in_name: &str,
    method: Option<&str>,
    low: f64,
    high: f64,
    blur: f64,
    invert: bool,
    format: Option<&str>,
) -> Result<(Vec<String>, String), String> {
    let method = parse_method(method)?;
    let format = parse_format(format)?;
    plan(in_name, method, low, high, blur, invert, format)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canny_is_the_default_plan() {
        let (argv, out) = plan_named(
            "in.png",
            None,
            DEFAULT_LOW,
            DEFAULT_HIGH,
            DEFAULT_BLUR,
            false,
            None,
        )
        .unwrap();
        assert_eq!(out, "out.png");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.png",
                "-vf",
                "format=gray,edgedetect=low=0.078431:high=0.196078",
                "-frames:v",
                "1",
                "out.png",
            ]
        );
    }

    #[test]
    fn sobel_needs_no_thresholds_and_stays_gray() {
        let vf = filter(Method::Sobel, 0.5, 0.9, 0.0, false).unwrap();
        assert_eq!(vf, "format=gray,sobel");
    }

    #[test]
    fn colormix_keeps_color_and_carries_thresholds() {
        let vf = filter(Method::ColorMix, 0.1, 0.4, 0.0, false).unwrap();
        assert_eq!(vf, "edgedetect=mode=colormix:low=0.1:high=0.4");
        assert!(!vf.contains("format=gray"));
    }

    #[test]
    fn blur_runs_before_the_detector_and_invert_after() {
        let vf = filter(Method::Canny, 0.1, 0.4, 1.5, true).unwrap();
        assert_eq!(
            vf,
            "format=gray,gblur=sigma=1.5,edgedetect=low=0.1:high=0.4,negate"
        );
    }

    #[test]
    fn zero_blur_omits_the_filter_entirely() {
        assert!(!filter(Method::Canny, 0.1, 0.4, 0.0, false)
            .unwrap()
            .contains("gblur"));
    }

    #[test]
    fn filter_strings_have_no_spaces() {
        for m in [Method::Canny, Method::Sobel, Method::ColorMix] {
            let vf = filter(m, 0.2, 0.6, 2.0, true).unwrap();
            assert!(!vf.contains(' '), "{vf} must be one argv token");
        }
    }

    #[test]
    fn jpg_pins_quality_and_renames_the_output() {
        let (argv, out) =
            plan("in.webp", Method::Canny, 0.1, 0.4, 0.0, false, OutFormat::Jpg).unwrap();
        assert_eq!(out, "out.jpg");
        assert!(argv.windows(2).any(|w| w == ["-q:v", "2"]));
    }

    #[test]
    fn webp_output_is_planned_without_quality_pin() {
        let (argv, out) =
            plan("in.png", Method::Sobel, 0.1, 0.4, 0.0, false, OutFormat::Webp).unwrap();
        assert_eq!(out, "out.webp");
        assert!(!argv.iter().any(|a| a == "-q:v"));
    }

    #[test]
    fn animated_input_is_reduced_to_one_frame() {
        let (argv, _) =
            plan("in.gif", Method::Canny, 0.1, 0.4, 0.0, false, OutFormat::Png).unwrap();
        assert!(argv.windows(2).any(|w| w == ["-frames:v", "1"]));
    }

    #[test]
    fn parse_method_defaults_and_aliases() {
        assert_eq!(parse_method(None).unwrap(), Method::Canny);
        assert_eq!(parse_method(Some("")).unwrap(), Method::Canny);
        assert_eq!(parse_method(Some(" CANNY ")).unwrap(), Method::Canny);
        assert_eq!(parse_method(Some("gradient")).unwrap(), Method::Sobel);
        assert_eq!(parse_method(Some("cartoon")).unwrap(), Method::ColorMix);
        assert_eq!(DEFAULT_METHOD, method_name(Method::Canny));
    }

    #[test]
    fn unknown_method_is_rejected_with_the_valid_list() {
        let err = parse_method(Some("laplacian")).unwrap_err();
        assert!(err.contains("laplacian"), "{err}");
        assert!(err.contains("canny") && err.contains("sobel") && err.contains("colormix"));
    }

    #[test]
    fn unknown_format_is_rejected_with_the_valid_list() {
        let err = parse_format(Some("tiff")).unwrap_err();
        assert!(err.contains("tiff") && err.contains("png"), "{err}");
        assert_eq!(parse_format(Some("JPEG")).unwrap(), OutFormat::Jpg);
    }

    #[test]
    fn thresholds_outside_zero_to_one_are_rejected() {
        for bad in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
            assert!(
                filter(Method::Canny, bad, 0.5, 0.0, false).is_err(),
                "low {bad} should be rejected"
            );
            assert!(
                filter(Method::Canny, 0.1, bad, 0.0, false).is_err(),
                "high {bad} should be rejected"
            );
        }
        let err = filter(Method::Canny, 50.0, 0.5, 0.0, false).unwrap_err();
        assert!(err.contains("low") && err.contains("between 0 and 1"), "{err}");
    }

    #[test]
    fn high_below_low_is_rejected() {
        let err = filter(Method::Canny, 0.6, 0.2, 0.0, false).unwrap_err();
        assert!(err.contains("high") && err.contains("low"), "{err}");
        // Equal is fine (degenerate but valid hysteresis).
        assert!(filter(Method::Canny, 0.3, 0.3, 0.0, false).is_ok());
    }

    #[test]
    fn blur_outside_the_cap_is_rejected() {
        assert!(filter(Method::Canny, 0.1, 0.4, -1.0, false).is_err());
        let err = filter(Method::Canny, 0.1, 0.4, 10.1, false).unwrap_err();
        assert!(err.contains("blur") && err.contains("10"), "{err}");
        // The cap boundary itself is accepted.
        assert!(filter(Method::Canny, 0.1, 0.4, MAX_BLUR, false).is_ok());
    }

    #[test]
    fn num_never_emits_scientific_notation_or_trailing_zeros() {
        assert_eq!(num(0.0), "0");
        assert_eq!(num(1.0), "1");
        assert_eq!(num(0.5), "0.5");
        assert_eq!(num(0.0000001), "0");
        assert_eq!(num(DEFAULT_LOW), "0.078431");
    }
}
