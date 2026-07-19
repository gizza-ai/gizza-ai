//! gizza-ai/video-stabilize core — pure ffmpeg argv construction shared by the
//! chat block and the standalone web page. No wasm-bindgen deps.
//!
//! Smooths camera shake with ffmpeg's single-pass `deshake` filter: it detects
//! the dominant frame-to-frame motion and shifts each frame to compensate, so
//! handheld wobble is smoothed while intentional pans are kept. The `strength`
//! (1–100) sets the motion **search radius** (`rx`/`ry` — snapped to deshake's
//! accepted multiples of 16, i.e. 16/32/48/64 px) — higher compensates bigger
//! shakes but risks visible edge artifacts.
//!
//! Shifting frames exposes their edges; the `borders` mode decides what fills
//! them:
//!
//! * `mirror` (default) — reflect the frame into the exposed band (no zoom,
//!   original resolution kept).
//! * `crop` — zoom in slightly and center-crop so the moving edges fall outside
//!   the visible frame (what most online stabilizers call "crop & zoom"). The
//!   zoom grows with `strength` (~2.8 % at strength 10 up to 10 % at 100).
//! * `blank` — fill the exposed band with black.
//! * `original` — keep the unstabilized source pixels at the edges.
//!
//! Because the picture is rewritten, the video stream is re-encoded to H.264
//! (`-crf 20`, visually near-transparent). Audio is stream-copied when the
//! container is kept (mp4/mov/m4v/mkv) and only re-encoded to AAC when the
//! input container can't hold H.264/AAC (e.g. webm), in which case the output
//! switches to MP4 — see `gizza_ai_block_utils::ffmpeg::h264_out_ext`.
//!
//! NOTE: the two-pass vid.stab pipeline (`vidstabdetect` + `vidstabtransform`)
//! is deliberately NOT used — it needs two ffmpeg invocations plus an external
//! library that the browser ffmpeg build doesn't ship. `deshake` is single-pass,
//! built into every ffmpeg, and handles mild-to-moderate handheld shake well.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;

/// What fills the frame edges exposed by the stabilizing shift.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Borders {
    /// Reflect the picture into the exposed band (deshake `edge=mirror`).
    Mirror,
    /// Zoom in slightly + center-crop so moving edges are hidden.
    Crop,
    /// Black bands (deshake `edge=blank`).
    Blank,
    /// Keep the unstabilized source pixels at the edges (deshake `edge=original`).
    Original,
}

/// Parse the user-facing borders string. Empty defaults to mirror.
pub fn parse_borders(s: &str) -> Result<Borders, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "mirror" => Ok(Borders::Mirror),
        "crop" => Ok(Borders::Crop),
        "blank" => Ok(Borders::Blank),
        "original" => Ok(Borders::Original),
        other => Err(format!(
            "borders {other:?} not supported (mirror|crop|blank|original)"
        )),
    }
}

/// Accepted strength range (percent-style intensity).
pub const MIN_STRENGTH: f64 = 1.0;
pub const MAX_STRENGTH: f64 = 100.0;
/// Default strength when the request is unset (the page's `0` sentinel) or
/// non-finite. 25 maps to a 16 px search radius — ffmpeg deshake's own default.
pub const DEFAULT_STRENGTH: f64 = 25.0;

/// Resolve a (possibly unset / non-finite) strength request into an accepted
/// value. `strength <= 0` (the page's "unset" sentinel) and any non-finite
/// value fall back to [`DEFAULT_STRENGTH`]; otherwise it is clamped into
/// `MIN_STRENGTH..=MAX_STRENGTH`.
pub fn resolve_strength(strength: f64) -> f64 {
    if !strength.is_finite() || strength <= 0.0 {
        return DEFAULT_STRENGTH;
    }
    strength.clamp(MIN_STRENGTH, MAX_STRENGTH)
}

/// Map the 1–100 strength to deshake's motion search radius `rx`/`ry`.
///
/// deshake only accepts radii that are a **multiple of 16** (it matches
/// 16 px macroblocks; ffmpeg errors with "rx must be a multiple of 16"
/// otherwise), so the strength quartiles snap to 16/32/48/64 px:
/// 1–25 → 16 (the filter's own default), 26–50 → 32, 51–75 → 48, 76–100 → 64.
pub fn strength_to_radius(strength: f64) -> i64 {
    let s = resolve_strength(strength);
    (((s / 25.0).ceil() as i64) * 16).clamp(16, 64)
}

/// Zoom factor for [`Borders::Crop`]: 1.02 at strength 0 growing linearly to
/// 1.10 at strength 100, so the cropped-away margin roughly tracks the shake
/// radius on typical HD frames.
pub fn crop_zoom(strength: f64) -> f64 {
    let s = resolve_strength(strength);
    1.0 + (2.0 + s * 0.08) / 100.0
}

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`4` not `4.0`,
/// `1.044` stays `1.044`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.5}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Build the ffmpeg `-vf` filter string for the given strength + borders mode.
///
/// All modes run `deshake=rx=R:ry=R` with `R = strength_to_radius(strength)`.
/// `crop` additionally zooms by [`crop_zoom`] (scale up, then center-crop back
/// to ~the original size, both truncated to even dimensions for H.264) so the
/// exposed edge band falls outside the visible frame.
pub fn build_filter(strength: f64, borders: Borders) -> String {
    let r = strength_to_radius(strength);
    match borders {
        Borders::Mirror => format!("deshake=rx={r}:ry={r}:edge=mirror"),
        Borders::Blank => format!("deshake=rx={r}:ry={r}:edge=blank"),
        Borders::Original => format!("deshake=rx={r}:ry={r}:edge=original"),
        Borders::Crop => {
            let z = fmt_num(crop_zoom(strength));
            format!(
                "deshake=rx={r}:ry={r}:edge=blank,scale=trunc(iw*{z}/2)*2:trunc(ih*{z}/2)*2,crop=trunc(iw/{z}/2)*2:trunc(ih/{z}/2)*2"
            )
        }
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) and the output filename for a
/// stabilize pass. Shared verbatim by the web page (`build_argv`) and the chat
/// block.
///
/// The stabilizer rewrites the picture, so the video is re-encoded to H.264
/// (`libx264`, `-preset medium`, `-crf 20`). The input container is kept when
/// it can hold H.264/AAC, otherwise the output is `out.mp4`; audio is
/// stream-copied when the container is kept and re-encoded to AAC otherwise.
/// `strength` is resolved via [`resolve_strength`].
pub fn build_argv(strength: f64, borders: Borders, in_name: &str) -> (Vec<String>, String) {
    let filter = build_filter(strength, borders);
    let (ext, reencode_audio) = h264_out_ext(in_name);
    let out_name = format!("out.{ext}");
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vf".to_string(),
        filter,
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "medium".to_string(),
        "-crf".to_string(),
        "20".to_string(),
        "-c:a".to_string(),
    ];
    argv.push(if reencode_audio { "aac" } else { "copy" }.to_string());
    argv.push(out_name.clone());
    (argv, out_name)
}

/// Validate the request and return `(argv, out_name)`. A non-finite strength is
/// rejected up front (a clear user error); an unset (`0`) or out-of-range
/// request is resolved/clamped by [`resolve_strength`]. `borders` is parsed
/// from the user string (empty → mirror).
pub fn plan(strength: f64, borders: &str, in_name: &str) -> Result<(Vec<String>, String), String> {
    if strength.is_nan() || strength.is_infinite() {
        return Err("strength must be a finite number".into());
    }
    let borders = parse_borders(borders)?;
    Ok(build_argv(strength, borders, in_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_borders_variants() {
        assert_eq!(parse_borders("").unwrap(), Borders::Mirror);
        assert_eq!(parse_borders("mirror").unwrap(), Borders::Mirror);
        assert_eq!(parse_borders("MIRROR").unwrap(), Borders::Mirror);
        assert_eq!(parse_borders(" crop ").unwrap(), Borders::Crop);
        assert_eq!(parse_borders("blank").unwrap(), Borders::Blank);
        assert_eq!(parse_borders("original").unwrap(), Borders::Original);
        assert!(parse_borders("zoom").is_err());
    }

    #[test]
    fn default_strength_reproduces_deshake_default_radius() {
        // strength 25 → rx=ry=16, ffmpeg deshake's own default search radius.
        assert_eq!(strength_to_radius(25.0), 16);
        assert_eq!(
            build_filter(25.0, Borders::Mirror),
            "deshake=rx=16:ry=16:edge=mirror"
        );
    }

    #[test]
    fn radius_snaps_to_deshake_multiples_of_16() {
        // deshake rejects rx/ry that aren't multiples of 16.
        assert_eq!(strength_to_radius(1.0), 16);
        assert_eq!(strength_to_radius(25.0), 16);
        assert_eq!(strength_to_radius(26.0), 32);
        assert_eq!(strength_to_radius(50.0), 32);
        assert_eq!(strength_to_radius(60.0), 48);
        assert_eq!(strength_to_radius(75.0), 48);
        assert_eq!(strength_to_radius(76.0), 64);
        assert_eq!(strength_to_radius(100.0), 64);
        for s in 1..=100 {
            assert_eq!(strength_to_radius(s as f64) % 16, 0, "strength {s}");
        }
        // Unset sentinel resolves to the default strength first.
        assert_eq!(strength_to_radius(0.0), 16);
        // Out-of-range clamps.
        assert_eq!(strength_to_radius(1000.0), 64);
    }

    #[test]
    fn unset_and_non_finite_fall_back_to_default() {
        assert_eq!(resolve_strength(0.0), DEFAULT_STRENGTH);
        assert_eq!(resolve_strength(-5.0), DEFAULT_STRENGTH);
        assert_eq!(resolve_strength(f64::NAN), DEFAULT_STRENGTH);
        assert_eq!(resolve_strength(f64::INFINITY), DEFAULT_STRENGTH);
        assert_eq!(resolve_strength(0.5), MIN_STRENGTH);
        assert_eq!(resolve_strength(1000.0), MAX_STRENGTH);
    }

    #[test]
    fn crop_zoom_tracks_strength() {
        assert_eq!(fmt_num(crop_zoom(25.0)), "1.04");
        assert_eq!(fmt_num(crop_zoom(100.0)), "1.1");
        assert_eq!(fmt_num(crop_zoom(1.0)), "1.0208");
    }

    #[test]
    fn crop_mode_builds_zoom_chain() {
        assert_eq!(
            build_filter(25.0, Borders::Crop),
            "deshake=rx=16:ry=16:edge=blank,scale=trunc(iw*1.04/2)*2:trunc(ih*1.04/2)*2,crop=trunc(iw/1.04/2)*2:trunc(ih/1.04/2)*2"
        );
    }

    #[test]
    fn edge_only_modes_have_no_zoom_chain() {
        assert_eq!(
            build_filter(50.0, Borders::Blank),
            "deshake=rx=32:ry=32:edge=blank"
        );
        assert_eq!(
            build_filter(50.0, Borders::Original),
            "deshake=rx=32:ry=32:edge=original"
        );
    }

    #[test]
    fn full_default_argv() {
        let (argv, out) = build_argv(25.0, Borders::Mirror, "in.mp4");
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp4",
                "-vf",
                "deshake=rx=16:ry=16:edge=mirror",
                "-c:v",
                "libx264",
                "-preset",
                "medium",
                "-crf",
                "20",
                "-c:a",
                "copy",
                "out.mp4",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn keeps_h264_capable_containers() {
        for ext in ["mp4", "mov", "m4v", "mkv"] {
            let (argv, out) = build_argv(25.0, Borders::Mirror, &format!("clip.{ext}"));
            assert_eq!(out, format!("out.{ext}"));
            assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "copy"));
        }
    }

    #[test]
    fn webm_switches_to_mp4_and_reencodes_audio() {
        let (argv, out) = build_argv(25.0, Borders::Mirror, "clip.webm");
        assert_eq!(out, "out.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
    }

    #[test]
    fn plan_validates() {
        assert!(plan(f64::NAN, "mirror", "in.mp4").is_err());
        assert!(plan(f64::INFINITY, "mirror", "in.mp4").is_err());
        assert!(plan(25.0, "zoom", "in.mp4").is_err());
        assert!(plan(25.0, "mirror", "in.mp4").is_ok());
        // Unset sentinels resolve to defaults.
        assert!(plan(0.0, "", "in.mp4").is_ok());
    }
}
