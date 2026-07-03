//! gizza-ai/image-vignette core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Wraps ffmpeg's `vignette` filter. The filter's own knob is an *angle* in
//! radians (0 = no effect, PI/2 = strongest); users get a friendly
//! `strength` 0–100 instead, mapped linearly onto [0, PI/2] here — strength 40
//! lands exactly on ffmpeg's default angle (PI/5). `mode` picks the classic
//! darkened vignette (`forward`) or a lightened/haze one (`backward`), and the
//! vignette center is set as a percentage of the image size so the same values
//! work for any resolution.
//!
//! Two extras beyond the raw filter:
//! - **Colored vignettes** (darken mode): for a non-black `color` the plan
//!   switches to a masked-merge chain — the vignette is applied to a white
//!   frame to obtain the exact per-pixel attenuation mask, then the image is
//!   merged toward a solid color frame using that mask
//!   (`out = image·m + color·(1−m)`). With black this is numerically identical
//!   to the plain filter (verified in tests against measured pixels), so black
//!   keeps the cheap single-filter path.
//! - **Output format** (`keep|png|jpg|webp`): `keep` preserves the input
//!   container; the rest convert. Explicit conversions are still images, so
//!   they take only the first frame of an animated input; jpg output pins
//!   `-q:v 2` for a high-quality re-encode.
//!
//! Everything is deterministic so the chat block, CLI and page produce
//! identical output; every filter string is a single token (no spaces) so it
//! passes cleanly as one argv element.

use std::f64::consts::FRAC_PI_2;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    Darken,
    Lighten,
}

/// The canonical mode names, in display order. Used for the schema enum and
/// the page `<select>` options. KEEP IN SYNC with `parse_mode` / `mode_name`.
pub const MODES: [&str; 2] = ["darken", "lighten"];

/// The default mode applied when no `mode` is supplied.
pub const DEFAULT_MODE: &str = "darken";

/// The default strength (maps to ffmpeg's own default vignette angle, PI/5).
pub const DEFAULT_STRENGTH: f64 = 40.0;

/// The default vignette center, in percent of the image size (the middle).
pub const DEFAULT_CENTER_PCT: f64 = 50.0;

/// The default vignette color (classic black edges → the plain filter path).
pub const DEFAULT_COLOR: &str = "black";

/// An RGB color triple for the vignette tint.
pub type Rgb = (u8, u8, u8);

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

/// Named colors accepted for `color`, with their sRGB values. A curated set of
/// the common CSS names (plus `sepia`, the classic photo tint) — anything else
/// is expressible as hex. KEEP the error message in `parse_color` honest.
const COLOR_NAMES: &[(&str, Rgb)] = &[
    ("black", (0, 0, 0)),
    ("white", (255, 255, 255)),
    ("gray", (128, 128, 128)),
    ("grey", (128, 128, 128)),
    ("silver", (192, 192, 192)),
    ("red", (255, 0, 0)),
    ("maroon", (128, 0, 0)),
    ("orange", (255, 165, 0)),
    ("gold", (255, 215, 0)),
    ("yellow", (255, 255, 0)),
    ("olive", (128, 128, 0)),
    ("green", (0, 128, 0)),
    ("lime", (0, 255, 0)),
    ("teal", (0, 128, 128)),
    ("cyan", (0, 255, 255)),
    ("aqua", (0, 255, 255)),
    ("blue", (0, 0, 255)),
    ("navy", (0, 0, 128)),
    ("purple", (128, 0, 128)),
    ("magenta", (255, 0, 255)),
    ("fuchsia", (255, 0, 255)),
    ("pink", (255, 192, 203)),
    ("brown", (165, 42, 42)),
    ("sepia", (112, 66, 20)),
    ("ivory", (255, 255, 240)),
    ("beige", (245, 245, 220)),
];

/// Parse a mode name (case-insensitive). `None` / `""` default to `darken`.
pub fn parse_mode(s: Option<&str>) -> Result<Mode, String> {
    let v = s.unwrap_or(DEFAULT_MODE).trim().to_ascii_lowercase();
    match v.as_str() {
        "" | "darken" | "dark" => Ok(Mode::Darken),
        "lighten" | "light" => Ok(Mode::Lighten),
        other => Err(format!(
            "invalid mode {other:?}; expected one of {}",
            MODES.join("|")
        )),
    }
}

/// The lower-cased canonical name of a mode (matches a `MODES` entry).
pub fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Darken => "darken",
        Mode::Lighten => "lighten",
    }
}

/// Parse a vignette color: a name from `COLOR_NAMES`, or hex as `#RGB`,
/// `#RRGGBB`, `0xRRGGBB`, or bare `RGB`/`RRGGBB`. `None` / `""` default to
/// black (the classic vignette).
pub fn parse_color(s: Option<&str>) -> Result<Rgb, String> {
    let t = s.unwrap_or(DEFAULT_COLOR).trim();
    if t.is_empty() {
        return Ok((0, 0, 0));
    }
    let lower = t.to_ascii_lowercase();
    if let Some((_, rgb)) = COLOR_NAMES.iter().find(|(n, _)| *n == lower) {
        return Ok(*rgb);
    }
    let hex = lower
        .strip_prefix('#')
        .or_else(|| lower.strip_prefix("0x"))
        .unwrap_or(&lower);
    if hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let nib = |c: char| c.to_digit(16).unwrap() as u8;
        let b: Vec<u8> = hex.chars().map(nib).collect();
        match b.as_slice() {
            [r, g, bl] => return Ok((r * 17, g * 17, bl * 17)),
            [r1, r2, g1, g2, b1, b2] => {
                return Ok((r1 * 16 + r2, g1 * 16 + g2, b1 * 16 + b2))
            }
            _ => {}
        }
    }
    Err(format!(
        "color {t:?} not recognized — use hex like #1A2B3C or #A52, or a name \
         (black, white, gray, sepia, navy, red, orange, …)"
    ))
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

/// Map a user-facing strength (0–100) onto the ffmpeg `vignette` angle in
/// radians: linear over [0, PI/2]. 0 = no visible change, 100 = corners fully
/// black (or white in lighten mode); 40 = ffmpeg's default angle PI/5.
pub fn angle_for_strength(strength: f64) -> Result<f64, String> {
    if !strength.is_finite() || !(0.0..=100.0).contains(&strength) {
        return Err(format!(
            "strength must be a number between 0 and 100, got {strength}"
        ));
    }
    Ok(strength / 100.0 * FRAC_PI_2)
}

/// Validate a vignette-center coordinate given in percent of the image size.
fn check_center_pct(name: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() || !(0.0..=100.0).contains(&v) {
        return Err(format!(
            "{name} must be a number between 0 and 100 (percent of the image size), got {v}"
        ));
    }
    Ok(())
}

/// The ffmpeg `-vf` filter string realising the vignette. Single argv token.
///
/// Black (the default) and lighten mode use the plain `vignette` filter. A
/// non-black `color` in darken mode uses a masked-merge chain: the vignette
/// applied to a white frame yields the exact attenuation mask `m` (255 at the
/// center), and `maskedmerge` computes `color·(1−m) + image·m` — so at
/// strength 100 the corners are exactly `color`, and with black the result
/// matches the plain path. `lighten` + a color is rejected with guidance,
/// since lighten always brightens toward white.
pub fn filter(
    strength: f64,
    mode: Mode,
    center_x_pct: f64,
    center_y_pct: f64,
    color: Rgb,
) -> Result<String, String> {
    let angle = angle_for_strength(strength)?;
    check_center_pct("center_x", center_x_pct)?;
    check_center_pct("center_y", center_y_pct)?;
    let fx = center_x_pct / 100.0;
    let fy = center_y_pct / 100.0;
    if color == (0, 0, 0) || mode == Mode::Lighten {
        if color != (0, 0, 0) {
            return Err(
                "a color tint only applies in darken mode — drop the color (lighten always \
                 brightens toward white), or use mode=darken with a light color like #FFF for \
                 a colored fade"
                    .to_string(),
            );
        }
        let mode_tok = match mode {
            Mode::Darken => "forward",
            Mode::Lighten => "backward",
        };
        // x0/y0 are ffmpeg expressions; w/h are the input's dimensions, so a
        // percentage center works for any resolution.
        return Ok(format!(
            "vignette=angle={angle:.6}:x0=w*{fx:.4}:y0=h*{fy:.4}:mode={mode_tok}"
        ));
    }
    let (r, g, b) = color;
    // Planar RGB throughout so lutrgb fills are exact (drawbox would blend
    // through YUV and shift the color). The mask leg round-trips through
    // full-range yuvj444p because that is the only family the vignette filter
    // accepts; white→235→255 restores exactly, so strength 0 stays a no-op.
    Ok(format!(
        "format=gbrp,split=3[img][a][b];\
         [a]lutrgb=r={r}:g={g}:b={b}[cf];\
         [b]lutrgb=r=255:g=255:b=255,format=yuvj444p,\
         vignette=angle={angle:.6}:x0=w*{fx:.4}:y0=h*{fy:.4},format=gbrp[mask];\
         [cf][img][mask]maskedmerge"
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

/// Build the ffmpeg argv (no leading "ffmpeg") and the output filename for
/// vignetting `in_name`. `format` `keep` reuses the input extension; explicit
/// formats convert (still image only — first frame of an animated input).
pub fn plan(
    in_name: &str,
    strength: f64,
    mode: Mode,
    center_x_pct: f64,
    center_y_pct: f64,
    color: Rgb,
    format: OutFormat,
) -> Result<(Vec<String>, String), String> {
    let vf = filter(strength, mode, center_x_pct, center_y_pct, color)?;
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
        // mjpeg's default quality is visibly lossy; pin a high-quality encode.
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
    strength: f64,
    mode: Option<&str>,
    center_x_pct: f64,
    center_y_pct: f64,
    color: Option<&str>,
    format: Option<&str>,
) -> Result<(Vec<String>, String), String> {
    let mode = parse_mode(mode)?;
    let color = parse_color(color)?;
    let format = parse_format(format)?;
    plan(in_name, strength, mode, center_x_pct, center_y_pct, color, format)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const BLACK: Rgb = (0, 0, 0);

    #[test]
    fn strength_maps_linearly_onto_the_ffmpeg_angle() {
        // Endpoints: 0 → no effect, 100 → the filter's max angle PI/2.
        assert_eq!(angle_for_strength(0.0).unwrap(), 0.0);
        assert_eq!(angle_for_strength(100.0).unwrap(), FRAC_PI_2);
        // The default strength lands exactly on ffmpeg's default angle (PI/5).
        let a = angle_for_strength(DEFAULT_STRENGTH).unwrap();
        assert!((a - PI / 5.0).abs() < 1e-12, "strength 40 should be PI/5, got {a}");
        // Halfway is halfway.
        assert!((angle_for_strength(50.0).unwrap() - PI / 4.0).abs() < 1e-12);
    }

    #[test]
    fn strength_mapping_is_monotonic() {
        let a20 = angle_for_strength(20.0).unwrap();
        let a40 = angle_for_strength(40.0).unwrap();
        let a80 = angle_for_strength(80.0).unwrap();
        assert!(a20 < a40 && a40 < a80);
    }

    #[test]
    fn strength_out_of_range_is_rejected() {
        for bad in [-1.0, 100.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(angle_for_strength(bad).is_err(), "{bad} should be rejected");
        }
        // The error names the valid range so the LLM/CLI user can recover.
        let err = angle_for_strength(250.0).unwrap_err();
        assert!(err.contains("0") && err.contains("100"));
    }

    #[test]
    fn parse_mode_default_is_darken() {
        assert_eq!(parse_mode(None).unwrap(), Mode::Darken);
        assert_eq!(parse_mode(Some("")).unwrap(), Mode::Darken);
        assert_eq!(DEFAULT_MODE, "darken");
    }

    #[test]
    fn parse_mode_names_and_aliases() {
        assert_eq!(parse_mode(Some("darken")).unwrap(), Mode::Darken);
        assert_eq!(parse_mode(Some("lighten")).unwrap(), Mode::Lighten);
        assert_eq!(parse_mode(Some(" LIGHTEN ")).unwrap(), Mode::Lighten);
        assert_eq!(parse_mode(Some("dark")).unwrap(), Mode::Darken);
        assert_eq!(parse_mode(Some("light")).unwrap(), Mode::Lighten);
    }

    #[test]
    fn parse_mode_rejects_unknown() {
        let err = parse_mode(Some("sepia")).unwrap_err();
        assert!(err.contains("darken") && err.contains("lighten"));
    }

    #[test]
    fn modes_const_round_trips_parser_and_names() {
        for name in MODES {
            let m = parse_mode(Some(name)).expect("MODES entry must parse");
            assert_eq!(mode_name(m), name, "mode_name round-trips MODES");
        }
    }

    #[test]
    fn parse_color_default_is_black() {
        assert_eq!(parse_color(None).unwrap(), (0, 0, 0));
        assert_eq!(parse_color(Some("")).unwrap(), (0, 0, 0));
        assert_eq!(parse_color(Some(" black ")).unwrap(), (0, 0, 0));
        assert_eq!(DEFAULT_COLOR, "black");
    }

    #[test]
    fn parse_color_names() {
        assert_eq!(parse_color(Some("white")).unwrap(), (255, 255, 255));
        assert_eq!(parse_color(Some("SEPIA")).unwrap(), (112, 66, 20));
        assert_eq!(parse_color(Some("navy")).unwrap(), (0, 0, 128));
        // Both spellings of gray, both magenta aliases.
        assert_eq!(parse_color(Some("grey")).unwrap(), parse_color(Some("gray")).unwrap());
        assert_eq!(
            parse_color(Some("fuchsia")).unwrap(),
            parse_color(Some("magenta")).unwrap()
        );
    }

    #[test]
    fn parse_color_hex_forms() {
        // Long hex with/without # and with 0x; case-insensitive.
        assert_eq!(parse_color(Some("#B08050")).unwrap(), (176, 128, 80));
        assert_eq!(parse_color(Some("b08050")).unwrap(), (176, 128, 80));
        assert_eq!(parse_color(Some("0xB08050")).unwrap(), (176, 128, 80));
        // Short hex doubles each nibble: #A52 → AA5522.
        assert_eq!(parse_color(Some("#A52")).unwrap(), (170, 85, 34));
        assert_eq!(parse_color(Some("fff")).unwrap(), (255, 255, 255));
        // Digits-only hex is still a color, never a number.
        assert_eq!(parse_color(Some("112233")).unwrap(), (17, 34, 51));
    }

    #[test]
    fn parse_color_rejects_unknown_with_guidance() {
        for bad in ["#12", "#12345", "#1234567", "chartreuse-ish", "rgb(1,2,3)"] {
            let err = parse_color(Some(bad)).unwrap_err();
            assert!(err.contains("hex") && err.contains("black"), "{bad}: {err}");
        }
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
    fn filter_uses_raw_radians_never_shown_to_users() {
        // strength 40 → PI/5 ≈ 0.628319; centered; classic dark edges.
        let f = filter(40.0, Mode::Darken, 50.0, 50.0, BLACK).unwrap();
        assert_eq!(f, "vignette=angle=0.628319:x0=w*0.5000:y0=h*0.5000:mode=forward");
    }

    #[test]
    fn filter_lighten_uses_backward_mode() {
        let f = filter(60.0, Mode::Lighten, 50.0, 50.0, BLACK).unwrap();
        assert!(f.ends_with(":mode=backward"), "{f}");
        assert!(f.contains("angle=0.942478"), "60% of PI/2: {f}");
    }

    #[test]
    fn filter_center_is_a_fraction_of_the_image_size() {
        let f = filter(40.0, Mode::Darken, 25.0, 75.0, BLACK).unwrap();
        assert!(f.contains("x0=w*0.2500:y0=h*0.7500"), "{f}");
    }

    #[test]
    fn filter_tinted_uses_masked_merge_chain() {
        let f = filter(70.0, Mode::Darken, 50.0, 50.0, (112, 66, 20)).unwrap();
        assert_eq!(
            f,
            "format=gbrp,split=3[img][a][b];\
             [a]lutrgb=r=112:g=66:b=20[cf];\
             [b]lutrgb=r=255:g=255:b=255,format=yuvj444p,\
             vignette=angle=1.099557:x0=w*0.5000:y0=h*0.5000,format=gbrp[mask];\
             [cf][img][mask]maskedmerge"
        );
    }

    #[test]
    fn filter_tinted_honors_center() {
        let f = filter(40.0, Mode::Darken, 25.0, 75.0, (255, 0, 0)).unwrap();
        assert!(f.contains("x0=w*0.2500:y0=h*0.7500"), "{f}");
    }

    #[test]
    fn filter_black_tint_takes_the_plain_path() {
        // Black is the classic vignette — no masked-merge machinery.
        let f = filter(40.0, Mode::Darken, 50.0, 50.0, BLACK).unwrap();
        assert!(!f.contains("maskedmerge"), "{f}");
    }

    #[test]
    fn filter_rejects_color_with_lighten() {
        let err = filter(40.0, Mode::Lighten, 50.0, 50.0, (255, 0, 0)).unwrap_err();
        assert!(err.contains("darken"), "guides toward darken mode: {err}");
        // Lighten without a color stays fine.
        assert!(filter(40.0, Mode::Lighten, 50.0, 50.0, BLACK).is_ok());
    }

    #[test]
    fn filter_is_a_single_argv_token() {
        let plain = filter(100.0, Mode::Lighten, 0.0, 100.0, BLACK).unwrap();
        assert!(!plain.contains(' '), "filter must be one argv token: {plain}");
        let tinted = filter(100.0, Mode::Darken, 0.0, 100.0, (1, 2, 3)).unwrap();
        assert!(!tinted.contains(' '), "tint chain must be one argv token: {tinted}");
    }

    #[test]
    fn filter_rejects_bad_center() {
        assert!(filter(40.0, Mode::Darken, -1.0, 50.0, BLACK).is_err());
        assert!(filter(40.0, Mode::Darken, 50.0, 101.0, BLACK).is_err());
        assert!(filter(40.0, Mode::Darken, f64::NAN, 50.0, BLACK).is_err());
    }

    #[test]
    fn plan_argv_structure_and_extension() {
        let (argv, out) =
            plan("photo.jpg", 80.0, Mode::Darken, 50.0, 50.0, BLACK, OutFormat::Keep).unwrap();
        assert_eq!(out, "out.jpg");
        assert_eq!(&argv[0], "-i");
        assert_eq!(&argv[1], "photo.jpg");
        assert_eq!(&argv[2], "-vf");
        assert_eq!(&argv[3], "vignette=angle=1.256637:x0=w*0.5000:y0=h*0.5000:mode=forward");
        // Keeping a jpg still pins the high-quality re-encode…
        assert_eq!(&argv[4..6], &["-q:v".to_string(), "2".to_string()][..]);
        // …but no -frames cap: keep never drops animation.
        assert!(!argv.contains(&"-frames:v".to_string()), "{argv:?}");
        assert_eq!(argv.last().unwrap(), "out.jpg");
    }

    #[test]
    fn plan_keeps_png_extension() {
        let (argv, out) =
            plan("in.png", 40.0, Mode::Darken, 50.0, 50.0, BLACK, OutFormat::Keep).unwrap();
        assert_eq!(out, "out.png");
        // png keep: no jpg quality flag, no frame cap.
        assert!(!argv.contains(&"-q:v".to_string()));
        assert!(!argv.contains(&"-frames:v".to_string()));
    }

    #[test]
    fn plan_converts_formats_as_single_frames() {
        let (argv, out) =
            plan("in.png", 40.0, Mode::Darken, 50.0, 50.0, BLACK, OutFormat::Jpg).unwrap();
        assert_eq!(out, "out.jpg");
        assert!(argv.windows(2).any(|w| w == ["-q:v", "2"]), "{argv:?}");
        assert!(argv.windows(2).any(|w| w == ["-frames:v", "1"]), "{argv:?}");
        let (argv, out) =
            plan("anim.gif", 40.0, Mode::Darken, 50.0, 50.0, BLACK, OutFormat::Png).unwrap();
        assert_eq!(out, "out.png");
        assert!(argv.windows(2).any(|w| w == ["-frames:v", "1"]), "{argv:?}");
        let (argv, out) =
            plan("in.jpg", 40.0, Mode::Darken, 50.0, 50.0, BLACK, OutFormat::Webp).unwrap();
        assert_eq!(out, "out.webp");
        assert!(!argv.contains(&"-q:v".to_string()), "webp keeps encoder defaults: {argv:?}");
    }

    #[test]
    fn plan_propagates_validation_errors() {
        assert!(plan("in.png", 101.0, Mode::Darken, 50.0, 50.0, BLACK, OutFormat::Keep).is_err());
        assert!(plan("in.png", 40.0, Mode::Darken, 50.0, 200.0, BLACK, OutFormat::Keep).is_err());
        assert!(
            plan("in.png", 40.0, Mode::Lighten, 50.0, 50.0, (255, 0, 0), OutFormat::Keep).is_err()
        );
    }

    #[test]
    fn plan_named_parses_and_plans() {
        let (argv, out) =
            plan_named("in.webp", 40.0, Some("lighten"), 50.0, 50.0, None, None).unwrap();
        assert_eq!(out, "out.webp");
        assert!(argv.iter().any(|a| a.contains("mode=backward")));
        assert!(plan_named("in.png", 40.0, Some("nope"), 50.0, 50.0, None, None).is_err());
        assert!(plan_named("in.png", 40.0, None, 50.0, 50.0, Some("no-color"), None).is_err());
        assert!(plan_named("in.png", 40.0, None, 50.0, 50.0, None, Some("tiff")).is_err());
        // Page-style call with every field supplied as text.
        let (argv, out) = plan_named(
            "photo.jpeg",
            100.0,
            Some("darken"),
            50.0,
            50.0,
            Some("#A52"),
            Some("png"),
        )
        .unwrap();
        assert_eq!(out, "out.png");
        assert!(argv.iter().any(|a| a.contains("lutrgb=r=170:g=85:b=34")), "{argv:?}");
    }
}
