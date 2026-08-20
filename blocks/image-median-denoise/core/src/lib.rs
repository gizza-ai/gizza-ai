//! gizza-ai/image-median-denoise core — pure ffmpeg argv construction shared by
//! the chat block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Wraps ffmpeg's `median` filter: every pixel is replaced by the median of the
//! square window around it. Unlike a blur, the median is an ACTUAL pixel value
//! from the neighbourhood, so isolated outliers (salt-and-pepper dots, scanner
//! dust, speckle) vanish while straight edges stay straight.
//!
//! The knobs, and how they map onto the filter:
//!
//! * `radius` 1–20 → `median=radius=`. The window is `2·radius+1` square, so
//!   radius 1 = 3×3 (the classic salt-and-pepper kernel), radius 3 = 7×7,
//!   radius 20 = 41×41. Bigger windows erase bigger specks and more detail.
//! * `target` → the filter's `percentile`. `both` (0.5) is the true median and
//!   removes bright AND dark specks. `bright` (0.3) leans toward the darker
//!   values in the window, so white dust/salt is wiped out harder (at the cost
//!   of a slight overall darkening); `dark` (0.7) is the mirror image for black
//!   specks/pepper on light paper.
//! * `channels` → the filter's `planes` bitmask. `all` (15) filters everything
//!   in whatever planar format ffmpeg negotiates (planar RGB for a PNG — no
//!   color-space conversion). `luma` (1) and `chroma` (6) only make sense in
//!   Y'CbCr, so those two prepend `format=yuva444p`: `luma` smooths brightness
//!   noise but keeps color detail, `chroma` kills blotchy color noise while
//!   leaving luminance detail pin-sharp. That conversion is a round trip
//!   through Y'CbCr, so expect ±1 sRGB rounding on the untouched channels.
//! * `passes` 1–3 → the median token repeated. Two mild passes remove dense
//!   noise more gently than one big radius (GIMP's Despeckle calls this
//!   "recursive").
//! * `format` `keep|png|jpg|webp` + `quality` 1–100 → the output encoder. `keep`
//!   reuses the input container (and any animation); explicit formats convert
//!   and take only the first frame. `quality` applies to jpg (mapped onto
//!   mjpeg's inverted 2–31 `-q:v`) and webp (`-quality`, same 1–100 scale);
//!   PNG is lossless and ignores it.
//! * `strip_metadata` → `-map_metadata -1`, which drops EXIF/ICC/comments
//!   instead of carrying them into the output.
//!
//! Everything is deterministic so chat, CLI and page produce identical output,
//! and every filter string is a single space-free token so it passes as one
//! argv element.

/// Which specks the filter should lean on removing (the `percentile` knob).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Target {
    /// True median (percentile 0.5) — removes bright and dark specks alike.
    Both,
    /// Percentile 0.3 — biased toward darker neighbours, so BRIGHT specks go.
    Bright,
    /// Percentile 0.7 — biased toward brighter neighbours, so DARK specks go.
    Dark,
}

/// Which image planes the median is applied to (the `planes` bitmask).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Channels {
    /// Every plane, in the format ffmpeg negotiates (planar RGB for a PNG).
    All,
    /// Y only — smooths brightness noise, keeps color detail.
    Luma,
    /// Cb+Cr only — removes blotchy color noise, keeps luminance detail.
    Chroma,
}

/// The output container/encoder.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OutFormat {
    Keep,
    Png,
    Jpg,
    Webp,
}

/// Canonical `target` names, in display order. Used for the schema enum and the
/// page `<select>`. KEEP IN SYNC with `parse_target` / `target_name`.
pub const TARGETS: [&str; 3] = ["both", "bright", "dark"];
/// Canonical `channels` names, in display order. KEEP IN SYNC with the parser.
pub const CHANNELS: [&str; 3] = ["all", "luma", "chroma"];
/// Canonical `format` names, in display order. KEEP IN SYNC with the parser.
pub const FORMATS: [&str; 4] = ["keep", "png", "jpg", "webp"];

/// Default target: the true median.
pub const DEFAULT_TARGET: &str = "both";
/// Default channels: filter every plane.
pub const DEFAULT_CHANNELS: &str = "all";
/// Default output format: keep the input's.
pub const DEFAULT_FORMAT: &str = "keep";
/// Default radius: 1 → the classic 3×3 salt-and-pepper window.
pub const DEFAULT_RADIUS: f64 = 1.0;
/// Largest accepted radius: 20 → a 41×41 window (same ceiling as GIMP's
/// Despeckle). Bigger than this is a smear, and quadratically slower.
pub const MAX_RADIUS: f64 = 20.0;
/// Default number of passes.
pub const DEFAULT_PASSES: f64 = 1.0;
/// Largest accepted number of passes.
pub const MAX_PASSES: f64 = 3.0;
/// Default encoder quality for jpg/webp output.
pub const DEFAULT_QUALITY: f64 = 92.0;

/// Parse a target name (case-insensitive). `None`/`""` → `both`.
pub fn parse_target(s: Option<&str>) -> Result<Target, String> {
    match s.unwrap_or(DEFAULT_TARGET).trim().to_ascii_lowercase().as_str() {
        "" | "both" | "median" => Ok(Target::Both),
        "bright" | "light" | "salt" | "white" => Ok(Target::Bright),
        "dark" | "pepper" | "black" => Ok(Target::Dark),
        other => Err(format!(
            "invalid target {other:?}; expected one of {}",
            TARGETS.join("|")
        )),
    }
}

/// The canonical name of a target (matches a `TARGETS` entry).
pub fn target_name(t: Target) -> &'static str {
    match t {
        Target::Both => "both",
        Target::Bright => "bright",
        Target::Dark => "dark",
    }
}

/// The ffmpeg `percentile` value for a target.
pub fn percentile(t: Target) -> f64 {
    match t {
        Target::Both => 0.5,
        Target::Bright => 0.3,
        Target::Dark => 0.7,
    }
}

/// Parse a channels name (case-insensitive). `None`/`""` → `all`.
pub fn parse_channels(s: Option<&str>) -> Result<Channels, String> {
    match s.unwrap_or(DEFAULT_CHANNELS).trim().to_ascii_lowercase().as_str() {
        "" | "all" | "rgb" => Ok(Channels::All),
        "luma" | "luminance" | "y" | "brightness" => Ok(Channels::Luma),
        "chroma" | "color" | "colour" | "uv" => Ok(Channels::Chroma),
        other => Err(format!(
            "invalid channels {other:?}; expected one of {}",
            CHANNELS.join("|")
        )),
    }
}

/// The canonical name of a channels choice (matches a `CHANNELS` entry).
pub fn channels_name(c: Channels) -> &'static str {
    match c {
        Channels::All => "all",
        Channels::Luma => "luma",
        Channels::Chroma => "chroma",
    }
}

/// The ffmpeg `planes` bitmask (bit 0 = Y/G, 1 = U/B, 2 = V/R, 3 = alpha).
pub fn planes(c: Channels) -> u32 {
    match c {
        Channels::All => 15,
        Channels::Luma => 1,
        Channels::Chroma => 6,
    }
}

/// Parse an output format name (case-insensitive). `None`/`""` → `keep`.
pub fn parse_format(s: Option<&str>) -> Result<OutFormat, String> {
    match s.unwrap_or(DEFAULT_FORMAT).trim().to_ascii_lowercase().as_str() {
        "" | "keep" | "same" => Ok(OutFormat::Keep),
        "png" => Ok(OutFormat::Png),
        "jpg" | "jpeg" => Ok(OutFormat::Jpg),
        "webp" => Ok(OutFormat::Webp),
        other => Err(format!(
            "invalid format {other:?}; expected one of {}",
            FORMATS.join("|")
        )),
    }
}

/// Validate + round a radius. `0` (a cleared page field) means "the default".
pub fn resolve_radius(radius: f64) -> Result<u32, String> {
    if !radius.is_finite() {
        return Err(format!(
            "radius must be a number between 1 and {}, got {radius}",
            MAX_RADIUS as u32
        ));
    }
    let r = if radius == 0.0 { DEFAULT_RADIUS } else { radius };
    let r = r.round();
    if !(1.0..=MAX_RADIUS).contains(&r) {
        return Err(format!(
            "radius must be between 1 and {} pixels (window {}x{} to {}x{}), got {radius}",
            MAX_RADIUS as u32,
            3,
            3,
            MAX_RADIUS as u32 * 2 + 1,
            MAX_RADIUS as u32 * 2 + 1
        ));
    }
    Ok(r as u32)
}

/// The square window side a radius produces: `2·radius+1`.
pub fn window_size(radius: u32) -> u32 {
    radius * 2 + 1
}

/// Validate + round a pass count. `0` (a cleared page field) means "1".
pub fn resolve_passes(passes: f64) -> Result<u32, String> {
    if !passes.is_finite() {
        return Err(format!("passes must be a whole number 1-{}, got {passes}", MAX_PASSES as u32));
    }
    let p = if passes == 0.0 { DEFAULT_PASSES } else { passes }.round();
    if !(1.0..=MAX_PASSES).contains(&p) {
        return Err(format!(
            "passes must be between 1 and {}, got {passes}",
            MAX_PASSES as u32
        ));
    }
    Ok(p as u32)
}

/// Validate + round an encoder quality. `0` (a cleared page field) → default.
pub fn resolve_quality(quality: f64) -> Result<u32, String> {
    if !quality.is_finite() {
        return Err(format!("quality must be a number 1-100, got {quality}"));
    }
    let q = if quality == 0.0 { DEFAULT_QUALITY } else { quality }.round();
    if !(1.0..=100.0).contains(&q) {
        return Err(format!("quality must be between 1 and 100, got {quality}"));
    }
    Ok(q as u32)
}

/// Map a friendly 1–100 quality onto mjpeg's inverted `-q:v` scale (2 = best,
/// 31 = worst), so quality 100 → 2 and quality 1 → 31.
pub fn jpeg_qscale(quality: u32) -> u32 {
    let q = quality.clamp(1, 100) as f64;
    let scaled = 2.0 + (100.0 - q) * (29.0 / 99.0);
    scaled.round().clamp(2.0, 31.0) as u32
}

/// Build the `-vf` filter chain (a single space-free token).
pub fn filter(radius: u32, target: Target, channels: Channels, passes: u32) -> String {
    let mut chain = String::new();
    // luma/chroma are Y'CbCr concepts — force a planar YUV+alpha format so the
    // `planes` bitmask means brightness/color rather than green/blue-red.
    if channels != Channels::All {
        chain.push_str("format=yuva444p,");
    }
    let one = format!(
        "median=radius={radius}:planes={}:percentile={:.2}",
        planes(channels),
        percentile(target)
    );
    for i in 0..passes {
        if i > 0 {
            chain.push(',');
        }
        chain.push_str(&one);
    }
    chain
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

/// Build the ffmpeg argv (no leading `ffmpeg`) plus the output filename.
///
/// `radius`/`passes`/`quality` accept `0` as "use the default" so a cleared
/// page field still runs; anything else out of range is an error naming the
/// range. `format` `keep` reuses the input extension (and its animation);
/// explicit formats convert the first frame only.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    in_name: &str,
    radius: f64,
    target: Target,
    channels: Channels,
    passes: f64,
    format: OutFormat,
    quality: f64,
    strip_metadata: bool,
) -> Result<(Vec<String>, String), String> {
    let radius = resolve_radius(radius)?;
    let passes = resolve_passes(passes)?;
    let quality = resolve_quality(quality)?;

    // Only a real `.ext` counts — an extensionless upload would otherwise turn
    // the whole filename into the "extension" and ffmpeg could not pick a muxer.
    let in_ext = in_name
        .rsplit_once('.')
        .map(|(_, e)| e)
        .filter(|e| !e.is_empty())
        .unwrap_or("png");
    let ext = out_ext(format, in_ext);
    let out_name = format!("out.{ext}");

    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vf".to_string(),
        filter(radius, target, channels, passes),
    ];
    if ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg") {
        argv.push("-q:v".to_string());
        argv.push(jpeg_qscale(quality).to_string());
    } else if ext.eq_ignore_ascii_case("webp") {
        argv.push("-quality".to_string());
        argv.push(quality.to_string());
    }
    if format != OutFormat::Keep {
        // Explicit formats are still images: one frame, so an animated input
        // converts cleanly instead of erroring in the image muxer.
        argv.push("-frames:v".to_string());
        argv.push("1".to_string());
    }
    if strip_metadata {
        argv.push("-map_metadata".to_string());
        argv.push("-1".to_string());
    }
    argv.push(out_name.clone());
    Ok((argv, out_name))
}

/// Parse + plan in one step from raw strings (the page and CLI paths, where
/// every field arrives as text).
#[allow(clippy::too_many_arguments)]
pub fn plan_named(
    in_name: &str,
    radius: f64,
    target: Option<&str>,
    channels: Option<&str>,
    passes: f64,
    format: Option<&str>,
    quality: f64,
    strip_metadata: bool,
) -> Result<(Vec<String>, String), String> {
    let target = parse_target(target)?;
    let channels = parse_channels(channels)?;
    let format = parse_format(format)?;
    plan(
        in_name,
        radius,
        target,
        channels,
        passes,
        format,
        quality,
        strip_metadata,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_plan(in_name: &str) -> (Vec<String>, String) {
        plan(
            in_name,
            DEFAULT_RADIUS,
            Target::Both,
            Channels::All,
            DEFAULT_PASSES,
            OutFormat::Keep,
            DEFAULT_QUALITY,
            false,
        )
        .unwrap()
    }

    #[test]
    fn happy_path_default_plan() {
        let (argv, out) = default_plan("in.png");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.png",
                "-vf",
                "median=radius=1:planes=15:percentile=0.50",
                "out.png"
            ]
        );
        assert_eq!(out, "out.png");
        // A single space-free token, so it passes as one argv element.
        assert!(!argv[3].contains(' '));
    }

    #[test]
    fn radius_out_of_range_is_rejected_with_the_range_in_the_message() {
        let err = plan_named("in.png", 21.0, None, None, 1.0, None, 0.0, false).unwrap_err();
        assert!(err.contains("1") && err.contains("20"), "{err}");
        assert!(plan_named("in.png", -3.0, None, None, 1.0, None, 0.0, false).is_err());
        assert!(plan_named("in.png", f64::NAN, None, None, 1.0, None, 0.0, false).is_err());
        // The exact cap boundary is accepted.
        let (argv, _) = plan_named("in.png", 20.0, None, None, 1.0, None, 0.0, false).unwrap();
        assert!(argv[3].contains("radius=20"), "{argv:?}");
    }

    #[test]
    fn window_size_matches_the_documented_kernel() {
        assert_eq!(window_size(1), 3);
        assert_eq!(window_size(2), 5);
        assert_eq!(window_size(20), 41);
    }

    #[test]
    fn cleared_numeric_fields_fall_back_to_defaults() {
        // The page sends 0 for a cleared number box — that must not error.
        let (argv, _) = plan_named("in.png", 0.0, Some(""), Some(""), 0.0, Some(""), 0.0, false).unwrap();
        assert_eq!(argv[3], "median=radius=1:planes=15:percentile=0.50");
    }

    #[test]
    fn target_maps_onto_percentile() {
        for (name, pct) in [("both", "0.50"), ("bright", "0.30"), ("dark", "0.70")] {
            let t = parse_target(Some(name)).unwrap();
            let (argv, _) =
                plan("in.png", 1.0, t, Channels::All, 1.0, OutFormat::Keep, 0.0, false).unwrap();
            assert!(argv[3].ends_with(&format!("percentile={pct}")), "{argv:?}");
            assert_eq!(target_name(t), name);
        }
    }

    #[test]
    fn targets_and_aliases_parse_and_unknown_is_rejected() {
        assert_eq!(parse_target(None).unwrap(), Target::Both);
        assert_eq!(parse_target(Some("SALT")).unwrap(), Target::Bright);
        assert_eq!(parse_target(Some(" pepper ")).unwrap(), Target::Dark);
        let err = parse_target(Some("rainbow")).unwrap_err();
        assert!(err.contains("both") && err.contains("bright") && err.contains("dark"));
    }

    #[test]
    fn channels_set_planes_and_force_yuv_when_needed() {
        let (all, _) =
            plan("in.png", 1.0, Target::Both, Channels::All, 1.0, OutFormat::Keep, 0.0, false)
                .unwrap();
        assert!(all[3].starts_with("median="), "all keeps the native format: {all:?}");
        assert!(all[3].contains("planes=15"));

        let (luma, _) =
            plan("in.png", 1.0, Target::Both, Channels::Luma, 1.0, OutFormat::Keep, 0.0, false)
                .unwrap();
        assert_eq!(luma[3], "format=yuva444p,median=radius=1:planes=1:percentile=0.50");

        let (chroma, _) =
            plan("in.png", 1.0, Target::Both, Channels::Chroma, 1.0, OutFormat::Keep, 0.0, false)
                .unwrap();
        assert!(chroma[3].contains("planes=6"), "{chroma:?}");
    }

    #[test]
    fn channels_names_round_trip_and_unknown_is_rejected() {
        for name in CHANNELS {
            let c = parse_channels(Some(name)).unwrap();
            assert_eq!(channels_name(c), name);
        }
        assert_eq!(parse_channels(Some("colour")).unwrap(), Channels::Chroma);
        assert!(parse_channels(Some("alpha")).is_err());
    }

    #[test]
    fn passes_repeat_the_median_token() {
        let (argv, _) =
            plan("in.png", 2.0, Target::Both, Channels::All, 3.0, OutFormat::Keep, 0.0, false)
                .unwrap();
        assert_eq!(argv[3].matches("median=").count(), 3, "{argv:?}");
        // One prepended format token even with several passes.
        let (luma, _) =
            plan("in.png", 2.0, Target::Both, Channels::Luma, 2.0, OutFormat::Keep, 0.0, false)
                .unwrap();
        assert_eq!(luma[3].matches("format=yuva444p").count(), 1, "{luma:?}");
        assert_eq!(luma[3].matches("median=").count(), 2);
        // Out of range is rejected, the boundary is not.
        assert!(plan_named("in.png", 1.0, None, None, 4.0, None, 0.0, false).is_err());
        assert!(plan_named("in.png", 1.0, None, None, 3.0, None, 0.0, false).is_ok());
    }

    #[test]
    fn keep_reuses_the_input_extension_and_its_encoder_quality() {
        let (argv, out) = default_plan("photo.jpeg");
        assert_eq!(out, "out.jpeg");
        // Keeping a jpeg still pins an explicit quality (mjpeg's default is lossy):
        // the default quality 92 maps onto qscale 4.
        let q = jpeg_qscale(DEFAULT_QUALITY as u32).to_string();
        assert_eq!(q, "4");
        assert!(argv.windows(2).any(|w| w == ["-q:v", q.as_str()]), "{argv:?}");
        // keep does not cap frames, so an animated GIF stays animated.
        assert!(!argv.contains(&"-frames:v".to_string()));
        // PNG is lossless: no quality flag at all.
        let (png, _) = default_plan("scan.png");
        assert!(!png.contains(&"-q:v".to_string()) && !png.contains(&"-quality".to_string()));
    }

    #[test]
    fn explicit_formats_convert_and_cap_frames() {
        let (jpg, out) = plan(
            "in.png", 1.0, Target::Both, Channels::All, 1.0, OutFormat::Jpg, 50.0, false,
        )
        .unwrap();
        assert_eq!(out, "out.jpg");
        assert!(jpg.windows(2).any(|w| w == ["-frames:v", "1"]), "{jpg:?}");
        let q50 = jpeg_qscale(50).to_string();
        assert!(jpg.windows(2).any(|w| w == ["-q:v", q50.as_str()]), "{jpg:?}");

        let (webp, out) = plan(
            "in.png", 1.0, Target::Both, Channels::All, 1.0, OutFormat::Webp, 80.0, false,
        )
        .unwrap();
        assert_eq!(out, "out.webp");
        assert!(webp.windows(2).any(|w| w == ["-quality", "80"]), "{webp:?}");
    }

    #[test]
    fn quality_maps_onto_the_inverted_mjpeg_scale() {
        assert_eq!(jpeg_qscale(100), 2);
        assert_eq!(jpeg_qscale(1), 31);
        assert!(jpeg_qscale(50) > jpeg_qscale(90), "lower quality = higher qscale");
        assert!(resolve_quality(101.0).is_err());
        assert!(resolve_quality(100.0).is_ok());
        assert_eq!(resolve_quality(0.0).unwrap(), DEFAULT_QUALITY as u32);
    }

    #[test]
    fn strip_metadata_adds_the_map_flag_only_when_asked() {
        let (with, _) = plan(
            "in.jpg", 1.0, Target::Both, Channels::All, 1.0, OutFormat::Keep, 0.0, true,
        )
        .unwrap();
        assert!(with.windows(2).any(|w| w == ["-map_metadata", "-1"]), "{with:?}");
        let (without, _) = default_plan("in.jpg");
        assert!(!without.contains(&"-map_metadata".to_string()));
    }

    #[test]
    fn unknown_format_is_rejected() {
        let err = plan_named("in.png", 1.0, None, None, 1.0, Some("tiff"), 0.0, false).unwrap_err();
        assert!(err.contains("keep") && err.contains("webp"), "{err}");
    }

    #[test]
    fn extensionless_input_defaults_to_png() {
        let (_, out) = default_plan("upload");
        assert_eq!(out, "out.png");
    }
}
