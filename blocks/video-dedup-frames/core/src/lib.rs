//! gizza-ai/video-dedup-frames core — pure ffmpeg argv construction shared by
//! the chat block and the standalone web page. No wasm-bindgen deps.
//!
//! Drops **consecutive duplicate frames** with ffmpeg's `mpdecimate` filter.
//! Screen recordings, slideshow exports and animation renders hold the same
//! picture for many frames in a row; every one of those repeats still costs
//! bytes and clutters an editor timeline. `mpdecimate` compares each frame with
//! the previous kept one in 8×8 blocks and marks it a duplicate when:
//!
//! * no block differs by more than `hi`, **and**
//! * fewer than `frac` of the blocks differ by more than `lo`.
//!
//! ffmpeg's own defaults are `hi = 64*12 = 768`, `lo = 64*5 = 320`,
//! `frac = 0.33`. The tool exposes them as a single 1–100 **sensitivity**
//! (linear around the ffmpeg default at 50: `hi = 768 * s/50`,
//! `lo = 320 * s/50`) plus the raw `frac` knob, so a light touch only removes
//! near-identical frames while a high value also removes frames that merely
//! *look* the same (a blinking cursor, dithering noise, a compression shimmer).
//!
//! **Marking a frame is not dropping it.** ffmpeg re-inserts decimated frames to
//! keep a constant frame rate unless the frame-rate mode says otherwise — the
//! classic "mpdecimate did nothing" trap. The `timing` param picks the mode:
//!
//! * `keep` (default) — `-fps_mode vfr`: the duplicates are really gone and each
//!   kept frame stays at its original timestamp, so the clip plays with the same
//!   timing (variable frame rate).
//! * `constant` — `-fps_mode cfr`: the kept frames are re-held on an even grid,
//!   so the output is constant-frame-rate for editors that dislike VFR. Near
//!   duplicates become *exact* repeats, which cost the encoder almost nothing.
//! * `compact` — `setpts=N/FRAME_RATE/TB` after the decimate: the gaps are
//!   removed too, so the clip shortens to just the frames that changed (a
//!   "remove the pauses" pass). Audio can't follow that re-timing, so it is
//!   dropped.
//!
//! `max_fps` caps the frame rate BEFORE the decimate (`fps=N,mpdecimate=…`) —
//! the "halve the frame rate first" trick for 60 fps screen captures. Putting
//! it first is deliberate: if the source is slower than the cap, the `fps`
//! filter duplicates frames to reach it and `mpdecimate` immediately removes
//! them again, so the cap never *invents* frames in the output.
//!
//! Filtering forces a re-encode, so `format` picks the codec/container:
//! `auto` keeps the input container when it can hold H.264/AAC (mp4/mov/m4v/mkv,
//! everything else becomes MP4 — see
//! [`gizza_ai_block_utils::ffmpeg::h264_out_ext`]), `mp4` forces H.264/AAC and
//! `webm` forces VP9/Opus.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;

// ---------------------------------------------------------------------------
// Sensitivity / frac / fps
// ---------------------------------------------------------------------------

/// Accepted sensitivity range (percent-style intensity).
pub const MIN_SENSITIVITY: f64 = 1.0;
pub const MAX_SENSITIVITY: f64 = 100.0;
/// Sensitivity used when the request is unset (the page's `0` sentinel) or
/// non-finite. 50 reproduces ffmpeg's own `mpdecimate` thresholds.
pub const DEFAULT_SENSITIVITY: f64 = 50.0;

/// mpdecimate's default `hi` threshold (64 × 12) — reached at sensitivity 50.
pub const BASE_HI: f64 = 768.0;
/// mpdecimate's default `lo` threshold (64 × 5) — reached at sensitivity 50.
pub const BASE_LO: f64 = 320.0;

/// mpdecimate's default `frac`: the fraction of 8×8 blocks that must exceed
/// `lo` for a frame to count as *changed*.
pub const DEFAULT_FRAC: f64 = 0.33;
/// Smallest accepted `frac` (0 would call every frame changed).
pub const MIN_FRAC: f64 = 0.01;
/// Largest accepted `frac` (the filter's own upper bound).
pub const MAX_FRAC: f64 = 1.0;

/// Largest accepted frame-rate cap.
pub const MAX_FPS_CAP: f64 = 240.0;
/// Smallest accepted frame-rate cap.
pub const MIN_FPS_CAP: f64 = 1.0;

/// x264 quality/speed used for the (unavoidable) re-encode.
pub const X264_CRF: &str = "20";
pub const X264_PRESET: &str = "medium";
/// VP9 constant-quality level used for `format = webm`.
pub const VP9_CRF: &str = "32";

/// Resolve a (possibly unset / non-finite) sensitivity request. `<= 0` (the
/// page's "unset" sentinel) and non-finite values fall back to
/// [`DEFAULT_SENSITIVITY`]; anything else is clamped to 1–100.
pub fn resolve_sensitivity(sensitivity: f64) -> f64 {
    if !sensitivity.is_finite() || sensitivity <= 0.0 {
        return DEFAULT_SENSITIVITY;
    }
    sensitivity.clamp(MIN_SENSITIVITY, MAX_SENSITIVITY)
}

/// Map a 1–100 sensitivity onto mpdecimate's `(hi, lo)` thresholds, linear
/// around ffmpeg's defaults at 50: `hi = 768 · s/50`, `lo = 320 · s/50`.
/// Both are floored at 1 so the filter never gets a 0 threshold.
pub fn sensitivity_to_thresholds(sensitivity: f64) -> (i64, i64) {
    let s = resolve_sensitivity(sensitivity) / DEFAULT_SENSITIVITY;
    let hi = (BASE_HI * s).round().max(1.0) as i64;
    let lo = (BASE_LO * s).round().max(1.0) as i64;
    (hi, lo)
}

/// Resolve a (possibly unset / non-finite) `frac`. `<= 0` (the page's "unset"
/// sentinel) and non-finite values fall back to [`DEFAULT_FRAC`]; anything else
/// is clamped to 0.01–1.
pub fn resolve_frac(frac: f64) -> f64 {
    if !frac.is_finite() || frac <= 0.0 {
        return DEFAULT_FRAC;
    }
    frac.clamp(MIN_FRAC, MAX_FRAC)
}

/// Resolve the frame-rate cap: `None` keeps the source rate (`<= 0`, the page's
/// "unset" sentinel, or non-finite), otherwise the value clamped to 1–240.
pub fn resolve_max_fps(max_fps: f64) -> Option<f64> {
    if !max_fps.is_finite() || max_fps <= 0.0 {
        return None;
    }
    Some(max_fps.clamp(MIN_FPS_CAP, MAX_FPS_CAP))
}

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`30` not `30.0`,
/// `29.97` stays `29.97`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

// ---------------------------------------------------------------------------
// Timing + output format
// ---------------------------------------------------------------------------

/// What happens to the timeline once the duplicates are marked.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Timing {
    /// Really drop them, keep every remaining frame at its original timestamp
    /// (variable frame rate). Same duration, same timing.
    Keep,
    /// Re-hold the kept frames on an even grid (constant frame rate).
    Constant,
    /// Drop them AND close the gaps, shortening the clip to the frames that
    /// changed. Audio is dropped (it can't follow the re-timing).
    Compact,
}

/// Parse the user-facing timing string. Empty defaults to [`Timing::Keep`].
pub fn parse_timing(s: &str) -> Result<Timing, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "keep" | "vfr" => Ok(Timing::Keep),
        "constant" | "cfr" => Ok(Timing::Constant),
        "compact" => Ok(Timing::Compact),
        other => Err(format!(
            "timing {other:?} not supported (keep|constant|compact)"
        )),
    }
}

/// Output container/codec choice.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    /// Keep the input container when it can hold H.264/AAC, else MP4.
    Auto,
    /// Force MP4 (H.264 + AAC).
    Mp4,
    /// Force WebM (VP9 + Opus).
    Webm,
}

/// Parse the user-facing format string. Empty defaults to [`Format::Auto`].
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" | "keep" | "same" => Ok(Format::Auto),
        "mp4" => Ok(Format::Mp4),
        "webm" => Ok(Format::Webm),
        other => Err(format!("format {other:?} not supported (auto|mp4|webm)")),
    }
}

/// Resolve `(out_ext, copy_audio)` for the requested output format.
///
/// * `auto` — [`h264_out_ext`] decides (input container kept when it can hold
///   H.264/AAC; audio is stream-copied only then).
/// * `mp4` — always `out.mp4`; the audio is stream-copied only when the input is
///   already an MP4-family file (`mp4`/`m4v`), because other containers may
///   carry audio codecs MP4 can't hold (webm's Opus/Vorbis, a mov's PCM).
/// * `webm` — always `out.webm` and always an Opus re-encode (WebM can't hold
///   AAC at all).
pub fn resolve_output(format: Format, in_name: &str) -> (&'static str, bool) {
    match format {
        Format::Auto => {
            let (ext, reencode_audio) = h264_out_ext(in_name);
            (ext, !reencode_audio)
        }
        Format::Mp4 => {
            let ext = in_name
                .rsplit_once('.')
                .map(|(_, e)| e.to_ascii_lowercase())
                .unwrap_or_default();
            ("mp4", ext == "mp4" || ext == "m4v")
        }
        Format::Webm => ("webm", false),
    }
}

// ---------------------------------------------------------------------------
// Filter graph + argv
// ---------------------------------------------------------------------------

/// Build the `-vf` filter chain: an optional frame-rate cap, the `mpdecimate`
/// itself, and — for [`Timing::Compact`] — the `setpts` that closes the gaps.
pub fn build_filter(sensitivity: f64, frac: f64, max_fps: f64, timing: Timing) -> String {
    let (hi, lo) = sensitivity_to_thresholds(sensitivity);
    let frac = fmt_num(resolve_frac(frac));
    let mut chain = String::new();
    // The cap goes FIRST: frames it duplicates (source slower than the cap) are
    // removed again by mpdecimate, so the cap can only ever reduce frames.
    if let Some(fps) = resolve_max_fps(max_fps) {
        chain.push_str(&format!("fps={},", fmt_num(fps)));
    }
    chain.push_str(&format!("mpdecimate=hi={hi}:lo={lo}:frac={frac}"));
    if timing == Timing::Compact {
        // N = index of the frame leaving mpdecimate, FRAME_RATE = the rate of
        // that link → the kept frames are re-stamped back-to-back.
        chain.push_str(",setpts=N/FRAME_RATE/TB");
    }
    chain
}

/// Build the ffmpeg argv (no leading `ffmpeg`) and the output filename.
///
/// The filter chain forces a re-encode: H.264 (`-crf 20`, `-preset medium`,
/// `yuv420p`) for mp4-family output, VP9 (`-crf 32 -b:v 0`) for WebM. Audio is
/// stream-copied when the container can keep it, re-encoded to AAC/Opus when it
/// can't, and dropped entirely for [`Timing::Compact`] (the video timeline is
/// rewritten, so a copied track would desync).
pub fn build_argv(
    sensitivity: f64,
    frac: f64,
    max_fps: f64,
    timing: Timing,
    format: Format,
    in_name: &str,
) -> (Vec<String>, String) {
    let filter = build_filter(sensitivity, frac, max_fps, timing);
    let (ext, copy_audio) = resolve_output(format, in_name);
    let out_name = format!("out.{ext}");
    let webm = ext == "webm";

    let mut argv: Vec<String> = vec![
        "-i".into(),
        in_name.into(),
        "-vf".into(),
        filter,
        // Without an explicit frame-rate mode ffmpeg re-inserts the decimated
        // frames — this line is what makes the tool actually drop them.
        "-fps_mode".into(),
        if timing == Timing::Constant {
            "cfr".into()
        } else {
            "vfr".into()
        },
        "-c:v".into(),
    ];
    if webm {
        argv.extend([
            "libvpx-vp9".into(),
            "-crf".into(),
            VP9_CRF.into(),
            "-b:v".into(),
            "0".into(),
        ]);
    } else {
        argv.extend([
            "libx264".into(),
            "-preset".into(),
            X264_PRESET.into(),
            "-crf".into(),
            X264_CRF.into(),
        ]);
    }
    argv.extend(["-pix_fmt".into(), "yuv420p".into()]);

    if timing == Timing::Compact {
        argv.push("-an".into());
    } else if copy_audio {
        argv.extend(["-c:a".into(), "copy".into()]);
    } else if webm {
        argv.extend(["-c:a".into(), "libopus".into()]);
    } else {
        argv.extend(["-c:a".into(), "aac".into()]);
    }
    argv.push(out_name.clone());
    (argv, out_name)
}

/// Validate the request and return `(argv, out_name)`.
///
/// Non-finite numbers are rejected up front (a clear user error); unset (`0`)
/// or out-of-range values are resolved/clamped by the `resolve_*` helpers.
/// `timing` and `format` are parsed from the user strings (empty → the
/// defaults `keep` / `auto`). Param ORDER matches the page field order.
pub fn plan(
    sensitivity: f64,
    timing: &str,
    max_fps: f64,
    format: &str,
    frac: f64,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    for (label, v) in [
        ("sensitivity", sensitivity),
        ("max_fps", max_fps),
        ("frac", frac),
    ] {
        if !v.is_finite() {
            return Err(format!("{label} must be a finite number"));
        }
    }
    if frac > MAX_FRAC {
        return Err(format!(
            "frac must be between {MIN_FRAC} and {MAX_FRAC} (got {})",
            fmt_num(frac)
        ));
    }
    let timing = parse_timing(timing)?;
    let format = parse_format(format)?;
    Ok(build_argv(sensitivity, frac, max_fps, timing, format, in_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg_after<'a>(argv: &'a [String], flag: &str) -> Option<&'a str> {
        argv.iter()
            .position(|a| a == flag)
            .and_then(|i| argv.get(i + 1))
            .map(String::as_str)
    }

    #[test]
    fn default_sensitivity_reproduces_ffmpeg_mpdecimate_defaults() {
        assert_eq!(sensitivity_to_thresholds(50.0), (768, 320));
        assert_eq!(
            build_filter(0.0, 0.0, 0.0, Timing::Keep),
            "mpdecimate=hi=768:lo=320:frac=0.33"
        );
    }

    #[test]
    fn sensitivity_scales_thresholds_linearly() {
        assert_eq!(sensitivity_to_thresholds(25.0), (384, 160));
        assert_eq!(sensitivity_to_thresholds(100.0), (1536, 640));
        // Lowest setting stays a usable (non-zero) threshold pair.
        assert_eq!(sensitivity_to_thresholds(1.0), (15, 6));
        // Unset sentinel + out-of-range requests resolve/clamp.
        assert_eq!(sensitivity_to_thresholds(0.0), (768, 320));
        assert_eq!(sensitivity_to_thresholds(1000.0), (1536, 640));
        assert_eq!(sensitivity_to_thresholds(-3.0), (768, 320));
        assert_eq!(sensitivity_to_thresholds(f64::NAN), (768, 320));
    }

    #[test]
    fn frac_resolves_and_clamps() {
        assert_eq!(resolve_frac(0.0), DEFAULT_FRAC);
        assert_eq!(resolve_frac(-1.0), DEFAULT_FRAC);
        assert_eq!(resolve_frac(f64::INFINITY), DEFAULT_FRAC);
        assert_eq!(resolve_frac(0.001), MIN_FRAC);
        assert_eq!(resolve_frac(5.0), MAX_FRAC);
        assert_eq!(resolve_frac(0.5), 0.5);
        assert_eq!(
            build_filter(50.0, 0.9, 0.0, Timing::Keep),
            "mpdecimate=hi=768:lo=320:frac=0.9"
        );
        assert_eq!(
            build_filter(50.0, 1.0, 0.0, Timing::Keep),
            "mpdecimate=hi=768:lo=320:frac=1"
        );
    }

    #[test]
    fn max_fps_cap_is_applied_before_the_decimate() {
        // The cap must come FIRST so frames it duplicates get decimated away.
        assert_eq!(
            build_filter(50.0, 0.0, 15.0, Timing::Keep),
            "fps=15,mpdecimate=hi=768:lo=320:frac=0.33"
        );
        assert_eq!(
            build_filter(50.0, 0.0, 29.97, Timing::Keep),
            "fps=29.97,mpdecimate=hi=768:lo=320:frac=0.33"
        );
        // Unset / non-finite → no fps filter at all.
        assert!(!build_filter(50.0, 0.0, 0.0, Timing::Keep).contains("fps="));
        assert!(!build_filter(50.0, 0.0, f64::NAN, Timing::Keep).contains("fps="));
        // Out of range clamps to the 1-240 window.
        assert_eq!(resolve_max_fps(1000.0), Some(MAX_FPS_CAP));
        assert_eq!(resolve_max_fps(0.2), Some(MIN_FPS_CAP));
    }

    #[test]
    fn timing_modes_map_to_frame_rate_mode_and_setpts() {
        // keep → vfr, no setpts: the duplicates are really gone, timing kept.
        let (keep, _) = build_argv(50.0, 0.0, 0.0, Timing::Keep, Format::Auto, "in.mp4");
        assert_eq!(arg_after(&keep, "-fps_mode"), Some("vfr"));
        assert!(!arg_after(&keep, "-vf").unwrap().contains("setpts"));
        assert_eq!(arg_after(&keep, "-c:a"), Some("copy"));

        // constant → cfr, still no setpts (ffmpeg re-holds the kept frames).
        let (cfr, _) = build_argv(50.0, 0.0, 0.0, Timing::Constant, Format::Auto, "in.mp4");
        assert_eq!(arg_after(&cfr, "-fps_mode"), Some("cfr"));
        assert!(!arg_after(&cfr, "-vf").unwrap().contains("setpts"));

        // compact → vfr + setpts (gaps closed) and NO audio (it can't follow).
        let (compact, _) = build_argv(50.0, 0.0, 0.0, Timing::Compact, Format::Auto, "in.mp4");
        assert_eq!(arg_after(&compact, "-fps_mode"), Some("vfr"));
        assert_eq!(
            arg_after(&compact, "-vf"),
            Some("mpdecimate=hi=768:lo=320:frac=0.33,setpts=N/FRAME_RATE/TB")
        );
        assert!(compact.iter().any(|a| a == "-an"));
        assert!(!compact.iter().any(|a| a == "-c:a"));
    }

    #[test]
    fn parse_timing_variants() {
        assert_eq!(parse_timing("").unwrap(), Timing::Keep);
        assert_eq!(parse_timing(" KEEP ").unwrap(), Timing::Keep);
        assert_eq!(parse_timing("vfr").unwrap(), Timing::Keep);
        assert_eq!(parse_timing("constant").unwrap(), Timing::Constant);
        assert_eq!(parse_timing("cfr").unwrap(), Timing::Constant);
        assert_eq!(parse_timing("compact").unwrap(), Timing::Compact);
        assert!(parse_timing("shorten").is_err());
    }

    #[test]
    fn parse_format_variants() {
        assert_eq!(parse_format("").unwrap(), Format::Auto);
        assert_eq!(parse_format("AUTO").unwrap(), Format::Auto);
        assert_eq!(parse_format("keep").unwrap(), Format::Auto);
        assert_eq!(parse_format("mp4").unwrap(), Format::Mp4);
        assert_eq!(parse_format("webm").unwrap(), Format::Webm);
        assert!(parse_format("gif").is_err());
    }

    #[test]
    fn auto_format_keeps_h264_capable_containers() {
        for ext in ["mp4", "mov", "m4v", "mkv"] {
            let (argv, out) = build_argv(
                50.0,
                0.0,
                0.0,
                Timing::Keep,
                Format::Auto,
                &format!("clip.{ext}"),
            );
            assert_eq!(out, format!("out.{ext}"));
            assert_eq!(arg_after(&argv, "-c:a"), Some("copy"));
            assert_eq!(arg_after(&argv, "-c:v"), Some("libx264"));
        }
    }

    #[test]
    fn auto_format_switches_webm_input_to_mp4_and_reencodes_audio() {
        let (argv, out) = build_argv(50.0, 0.0, 0.0, Timing::Keep, Format::Auto, "screen.webm");
        assert_eq!(out, "out.mp4");
        assert_eq!(arg_after(&argv, "-c:a"), Some("aac"));
    }

    #[test]
    fn forced_mp4_copies_audio_only_from_mp4_family_inputs() {
        let (argv, out) = build_argv(50.0, 0.0, 0.0, Timing::Keep, Format::Mp4, "clip.mp4");
        assert_eq!(out, "out.mp4");
        assert_eq!(arg_after(&argv, "-c:a"), Some("copy"));
        // A mov/mkv may carry audio mp4 can't hold → AAC re-encode.
        for ext in ["mov", "mkv", "webm", "avi"] {
            let (argv, out) = build_argv(
                50.0,
                0.0,
                0.0,
                Timing::Keep,
                Format::Mp4,
                &format!("clip.{ext}"),
            );
            assert_eq!(out, "out.mp4");
            assert_eq!(arg_after(&argv, "-c:a"), Some("aac"), "{ext}");
        }
    }

    #[test]
    fn webm_output_uses_vp9_and_opus() {
        let (argv, out) = build_argv(50.0, 0.0, 0.0, Timing::Keep, Format::Webm, "clip.mp4");
        assert_eq!(out, "out.webm");
        assert_eq!(arg_after(&argv, "-c:v"), Some("libvpx-vp9"));
        assert_eq!(arg_after(&argv, "-crf"), Some(VP9_CRF));
        assert_eq!(arg_after(&argv, "-b:v"), Some("0"));
        assert_eq!(arg_after(&argv, "-c:a"), Some("libopus"));
    }

    #[test]
    fn full_default_argv() {
        let (argv, out) = build_argv(0.0, 0.0, 0.0, Timing::Keep, Format::Auto, "in.mp4");
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp4",
                "-vf",
                "mpdecimate=hi=768:lo=320:frac=0.33",
                "-fps_mode",
                "vfr",
                "-c:v",
                "libx264",
                "-preset",
                "medium",
                "-crf",
                "20",
                "-pix_fmt",
                "yuv420p",
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
    fn plan_validates_and_maps_page_field_order() {
        // plan(sensitivity, timing, max_fps, format, frac, in_name)
        let (argv, out) = plan(80.0, "compact", 30.0, "webm", 0.5, "in.mkv").unwrap();
        assert_eq!(out, "out.webm");
        assert_eq!(
            arg_after(&argv, "-vf"),
            Some("fps=30,mpdecimate=hi=1229:lo=512:frac=0.5,setpts=N/FRAME_RATE/TB")
        );
        assert!(argv.iter().any(|a| a == "-an"));

        assert!(plan(f64::NAN, "keep", 0.0, "auto", 0.0, "in.mp4").is_err());
        assert!(plan(50.0, "keep", f64::INFINITY, "auto", 0.0, "in.mp4").is_err());
        assert!(plan(50.0, "keep", 0.0, "auto", f64::NAN, "in.mp4").is_err());
        assert!(plan(50.0, "keep", 0.0, "auto", 1.5, "in.mp4").is_err());
        assert!(plan(50.0, "shorten", 0.0, "auto", 0.0, "in.mp4").is_err());
        assert!(plan(50.0, "keep", 0.0, "gif", 0.0, "in.mp4").is_err());
        // Every "unset" sentinel resolves to the documented defaults.
        assert!(plan(0.0, "", 0.0, "", 0.0, "in.mp4").is_ok());
    }

    #[test]
    fn fmt_num_is_compact() {
        assert_eq!(fmt_num(30.0), "30");
        assert_eq!(fmt_num(29.97), "29.97");
        assert_eq!(fmt_num(0.33), "0.33");
        assert_eq!(fmt_num(0.125), "0.125");
    }
}
