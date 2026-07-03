//! gizza-ai/image-vignette core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Wraps ffmpeg's `vignette` filter. The filter's own knob is an *angle* in
//! radians (0 = no effect, PI/2 = strongest); users get a friendly
//! `strength` 0–100 instead, mapped linearly onto [0, PI/2] here — strength 40
//! lands exactly on ffmpeg's default angle (PI/5). `mode` picks the classic
//! darkened vignette (`forward`) or a lightened/haze one (`backward`), and the
//! vignette center is set as a percentage of the image size so the same values
//! work for any resolution. Everything is deterministic so the chat block, CLI
//! and page produce identical output; the filter string is a single token (no
//! spaces) so it passes cleanly as one argv element.

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
pub fn filter(
    strength: f64,
    mode: Mode,
    center_x_pct: f64,
    center_y_pct: f64,
) -> Result<String, String> {
    let angle = angle_for_strength(strength)?;
    check_center_pct("center_x", center_x_pct)?;
    check_center_pct("center_y", center_y_pct)?;
    let mode_tok = match mode {
        Mode::Darken => "forward",
        Mode::Lighten => "backward",
    };
    // x0/y0 are ffmpeg expressions; w/h are the input's dimensions, so a
    // percentage center works for any resolution.
    Ok(format!(
        "vignette=angle={angle:.6}:x0=w*{:.4}:y0=h*{:.4}:mode={mode_tok}",
        center_x_pct / 100.0,
        center_y_pct / 100.0,
    ))
}

/// Build the ffmpeg argv (no leading "ffmpeg") and the output filename for
/// vignetting `in_name`. `out_name` keeps the input extension.
pub fn plan(
    in_name: &str,
    strength: f64,
    mode: Mode,
    center_x_pct: f64,
    center_y_pct: f64,
) -> Result<(Vec<String>, String), String> {
    let vf = filter(strength, mode, center_x_pct, center_y_pct)?;
    let ext = in_name
        .rsplit('.')
        .next()
        .filter(|e| !e.is_empty())
        .unwrap_or("png");
    let out_name = format!("out.{ext}");
    let argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vf".to_string(),
        vf,
        out_name.clone(),
    ];
    Ok((argv, out_name))
}

/// Parse + plan in one step from a raw mode string (used by the web page).
pub fn plan_named(
    in_name: &str,
    strength: f64,
    mode: Option<&str>,
    center_x_pct: f64,
    center_y_pct: f64,
) -> Result<(Vec<String>, String), String> {
    let mode = parse_mode(mode)?;
    plan(in_name, strength, mode, center_x_pct, center_y_pct)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

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
    fn filter_uses_raw_radians_never_shown_to_users() {
        // strength 40 → PI/5 ≈ 0.628319; centered; classic dark edges.
        let f = filter(40.0, Mode::Darken, 50.0, 50.0).unwrap();
        assert_eq!(f, "vignette=angle=0.628319:x0=w*0.5000:y0=h*0.5000:mode=forward");
    }

    #[test]
    fn filter_lighten_uses_backward_mode() {
        let f = filter(60.0, Mode::Lighten, 50.0, 50.0).unwrap();
        assert!(f.ends_with(":mode=backward"), "{f}");
        assert!(f.contains("angle=0.942478"), "60% of PI/2: {f}");
    }

    #[test]
    fn filter_center_is_a_fraction_of_the_image_size() {
        let f = filter(40.0, Mode::Darken, 25.0, 75.0).unwrap();
        assert!(f.contains("x0=w*0.2500:y0=h*0.7500"), "{f}");
    }

    #[test]
    fn filter_is_a_single_argv_token() {
        let f = filter(100.0, Mode::Lighten, 0.0, 100.0).unwrap();
        assert!(!f.contains(' '), "filter must be one argv token: {f}");
    }

    #[test]
    fn filter_rejects_bad_center() {
        assert!(filter(40.0, Mode::Darken, -1.0, 50.0).is_err());
        assert!(filter(40.0, Mode::Darken, 50.0, 101.0).is_err());
        assert!(filter(40.0, Mode::Darken, f64::NAN, 50.0).is_err());
    }

    #[test]
    fn plan_argv_structure_and_extension() {
        let (argv, out) = plan("photo.jpg", 80.0, Mode::Darken, 50.0, 50.0).unwrap();
        assert_eq!(out, "out.jpg");
        assert_eq!(&argv[0], "-i");
        assert_eq!(&argv[1], "photo.jpg");
        assert_eq!(&argv[2], "-vf");
        assert_eq!(&argv[3], "vignette=angle=1.256637:x0=w*0.5000:y0=h*0.5000:mode=forward");
        assert_eq!(&argv[4], "out.jpg");
    }

    #[test]
    fn plan_keeps_png_extension() {
        let (_argv, out) = plan("in.png", 40.0, Mode::Darken, 50.0, 50.0).unwrap();
        assert_eq!(out, "out.png");
    }

    #[test]
    fn plan_propagates_validation_errors() {
        assert!(plan("in.png", 101.0, Mode::Darken, 50.0, 50.0).is_err());
        assert!(plan("in.png", 40.0, Mode::Darken, 50.0, 200.0).is_err());
    }

    #[test]
    fn plan_named_parses_and_plans() {
        let (argv, out) = plan_named("in.webp", 40.0, Some("lighten"), 50.0, 50.0).unwrap();
        assert_eq!(out, "out.webp");
        assert!(argv.iter().any(|a| a.contains("mode=backward")));
        assert!(plan_named("in.png", 40.0, Some("nope"), 50.0, 50.0).is_err());
    }
}
