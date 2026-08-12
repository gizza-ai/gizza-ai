//! gizza-ai/video-set-keyframe-interval core — pure ffmpeg argv construction shared
//! by the chat block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Re-encodes a video with a **fixed keyframe (GOP) cadence** — e.g. one keyframe
//! every 2 seconds — so players can seek cleanly and HLS/DASH packagers can cut
//! evenly-sized segments on keyframe boundaries.
//!
//! Two cadence units, because the two ecosystems disagree:
//! * `seconds` (default) → `-force_key_frames expr:gte(t,n_forced*N)`. Placement
//!   is by TIMESTAMP, so it stays correct on variable-frame-rate sources and
//!   needs no knowledge of the input frame rate.
//! * `frames` → `-g N -keyint_min N`, the classic x264 `keyint`/`min-keyint`
//!   pair. Exact for constant-frame-rate sources.
//!
//! `-sc_threshold 0` disables scene-cut detection (the encoder would otherwise
//! insert EXTRA keyframes at scene changes and break segment alignment); pass
//! `scene_cut = true` to allow those. `-flags +cgop` asks for closed GOPs, which
//! is what seeking and bitrate switching want.
//!
//! Deliberately distinct from its neighbours:
//! - `video-fragmented-mp4` ALWAYS writes a fragmented MP4 (`+frag_keyframe
//!   +empty_moov+default_base_moof` are unconditional); this tool writes a normal
//!   progressive MP4 with `+faststart`.
//! - `video-compress` / `video-to-h264` / `video-transcode` expose no GOP control
//!   at all — they never pass `-g`, `keyint_min`, `sc_threshold` or
//!   `force_key_frames`.

/// Unit the `interval` value is expressed in.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Unit {
    /// Interval in SECONDS — keyframes are placed by timestamp
    /// (`-force_key_frames`), so the cadence is right whatever the frame rate,
    /// including variable-frame-rate sources.
    Seconds,
    /// Interval in FRAMES — the classic GOP size (`-g` / `-keyint_min`). Exact
    /// for constant-frame-rate sources.
    Frames,
}

/// Parse the user-facing unit string (the values the chat schema + page accept).
pub fn parse_unit(s: &str) -> Result<Unit, String> {
    match s {
        "seconds" => Ok(Unit::Seconds),
        "frames" => Ok(Unit::Frames),
        other => Err(format!("unit {other:?} not supported (seconds|frames)")),
    }
}

/// libx264 speed/compression preset. Slower presets compress better at the same
/// CRF and take longer; the cadence flags mean the same thing at every preset.
pub const PRESETS: [&str; 9] = [
    "ultrafast",
    "superfast",
    "veryfast",
    "faster",
    "fast",
    "medium",
    "slow",
    "slower",
    "veryslow",
];

/// Validate the preset name against [`PRESETS`].
pub fn parse_preset(s: &str) -> Result<&'static str, String> {
    PRESETS
        .iter()
        .copied()
        .find(|p| *p == s)
        .ok_or_else(|| format!("preset {s:?} not supported ({})", PRESETS.join("|")))
}

/// Default cadence: one keyframe every 2 seconds — the value streaming platforms
/// converge on (2 s segments, quick seeking, modest size cost).
pub const DEFAULT_INTERVAL: f64 = 2.0;
/// Default unit — users think in seconds, and seconds mode is frame-rate-safe.
pub const DEFAULT_UNIT: Unit = Unit::Seconds;
/// Default CRF. 23 is libx264's own default: a good size/quality balance.
pub const DEFAULT_QUALITY: u32 = 23;
/// Default preset.
pub const DEFAULT_PRESET: &str = "medium";

/// Longest accepted cadence in SECONDS. Beyond a minute a "fixed keyframe
/// interval" stops being useful for seeking or segmentation.
pub const MAX_SECONDS: f64 = 60.0;
/// Shortest accepted cadence in SECONDS. Below this every frame is nearly a
/// keyframe and the file balloons.
pub const MIN_SECONDS: f64 = 0.1;
/// Largest accepted GOP size in FRAMES (10 s at 300 fps, 60 s at 50 fps).
pub const MAX_FRAMES: f64 = 3000.0;
/// Lowest accepted CRF. Deliberately 1, not 0: CRF 0 is true-lossless libx264,
/// which produces enormous files that blow past the tool's output cap for even
/// short clips. 1 is already visually indistinguishable from lossless.
pub const MIN_CRF: u32 = 1;
/// Highest accepted CRF (51 = worst quality, smallest file) — the top of
/// libx264's range.
pub const MAX_CRF: u32 = 51;

/// Render a float without a trailing `.0` so the argv reads `2` not `2.0`
/// (both work, but `expr:gte(t,n_forced*2)` is what users see in every recipe).
pub fn trim_number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that re-encodes `in_name` to
/// `out_name` with a fixed keyframe cadence.
///
/// `interval` is already validated and is in the unit given by `unit`.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    interval: f64,
    unit: Unit,
    scene_cut: bool,
    closed_gop: bool,
    crf: u32,
    preset: &str,
) -> Vec<String> {
    let mut argv: Vec<String> = vec!["-i".into(), in_name.into()];

    // Cadence first — it is the whole point of the tool.
    match unit {
        Unit::Seconds => {
            argv.push("-force_key_frames".into());
            argv.push(format!("expr:gte(t,n_forced*{})", trim_number(interval)));
        }
        Unit::Frames => {
            let g = trim_number(interval.round());
            argv.push("-g".into());
            argv.push(g.clone());
            // A minimum equal to the maximum is what pins the cadence; without
            // it the encoder may still close a GOP early.
            if !scene_cut {
                argv.push("-keyint_min".into());
                argv.push(g);
            }
        }
    }
    if !scene_cut {
        // Stop the encoder adding extra keyframes at scene changes.
        argv.push("-sc_threshold".into());
        argv.push("0".into());
    }
    if closed_gop {
        argv.push("-flags".into());
        argv.push("+cgop".into());
    }

    // Encode settings. A fixed cadence cannot be applied by stream copy, so this
    // tool always re-encodes.
    argv.extend([
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        preset.into(),
        "-crf".into(),
        crf.to_string(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        // No-op when the input has no audio track.
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "128k".into(),
        "-movflags".into(),
        "+faststart".into(),
        out_name.into(),
    ]);
    argv
}

/// Validate every param, then build `(argv, out_name)`. `out_name` is always
/// `out.mp4` — the progressive MP4 that players and packagers expect. Single
/// source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
#[allow(clippy::too_many_arguments)]
pub fn plan(
    interval: f64,
    unit: &str,
    scene_cut: bool,
    closed_gop: bool,
    quality: u32,
    preset: &str,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    let u = parse_unit(unit)?;
    if !interval.is_finite() || interval <= 0.0 {
        return Err(format!(
            "interval must be a positive number, got {}",
            trim_number(interval)
        ));
    }
    match u {
        Unit::Seconds => {
            if !(MIN_SECONDS..=MAX_SECONDS).contains(&interval) {
                return Err(format!(
                    "interval must be {MIN_SECONDS}-{MAX_SECONDS} seconds when unit=seconds, got {}",
                    trim_number(interval)
                ));
            }
        }
        Unit::Frames => {
            if !(1.0..=MAX_FRAMES).contains(&interval) {
                return Err(format!(
                    "interval must be 1-{} frames when unit=frames, got {}",
                    trim_number(MAX_FRAMES),
                    trim_number(interval)
                ));
            }
        }
    }
    if !(MIN_CRF..=MAX_CRF).contains(&quality) {
        return Err(format!(
            "quality (CRF) must be {MIN_CRF}-{MAX_CRF}, got {quality}"
        ));
    }
    let p = parse_preset(preset)?;
    let out_name = "out.mp4".to_string();
    let argv = build_argv(
        in_name, &out_name, interval, u, scene_cut, closed_gop, quality, p,
    );
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_unit_known_and_unknown() {
        assert_eq!(parse_unit("seconds").unwrap(), Unit::Seconds);
        assert_eq!(parse_unit("frames").unwrap(), Unit::Frames);
        assert!(parse_unit("ms").is_err());
        assert!(parse_unit("").is_err());
    }

    #[test]
    fn parse_preset_known_and_unknown() {
        assert_eq!(parse_preset("medium").unwrap(), "medium");
        assert_eq!(parse_preset("veryslow").unwrap(), "veryslow");
        assert!(parse_preset("placebo").is_err());
        assert!(parse_preset("Medium").is_err());
    }

    #[test]
    fn trims_trailing_zeros() {
        assert_eq!(trim_number(2.0), "2");
        assert_eq!(trim_number(1.5), "1.5");
        assert_eq!(trim_number(0.25), "0.25");
    }

    #[test]
    fn default_plan_is_two_second_time_based_cadence() {
        let (argv, out) = plan(2.0, "seconds", false, true, 23, "medium", "in.mp4").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp4",
                "-force_key_frames",
                "expr:gte(t,n_forced*2)",
                "-sc_threshold",
                "0",
                "-flags",
                "+cgop",
                "-c:v",
                "libx264",
                "-preset",
                "medium",
                "-crf",
                "23",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-b:a",
                "128k",
                "-movflags",
                "+faststart",
                "out.mp4",
            ]
        );
    }

    #[test]
    fn frames_mode_pins_g_and_keyint_min() {
        let (argv, _) = plan(60.0, "frames", false, true, 20, "fast", "clip.webm").unwrap();
        assert!(argv.windows(2).any(|w| w == ["-g", "60"]));
        assert!(argv.windows(2).any(|w| w == ["-keyint_min", "60"]));
        assert!(argv.windows(2).any(|w| w == ["-sc_threshold", "0"]));
        assert!(!argv.iter().any(|a| a == "-force_key_frames"));
        assert!(argv.windows(2).any(|w| w == ["-preset", "fast"]));
        assert!(argv.windows(2).any(|w| w == ["-crf", "20"]));
    }

    #[test]
    fn scene_cut_allows_extra_keyframes() {
        let (argv, _) = plan(4.0, "frames", true, true, 23, "medium", "in.mp4").unwrap();
        assert!(argv.windows(2).any(|w| w == ["-g", "4"]));
        // Both the minimum pin and the scene-cut disable are dropped.
        assert!(!argv.iter().any(|a| a == "-keyint_min"));
        assert!(!argv.iter().any(|a| a == "-sc_threshold"));
    }

    #[test]
    fn open_gop_drops_the_cgop_flag() {
        let (argv, _) = plan(2.0, "seconds", false, false, 23, "medium", "in.mp4").unwrap();
        assert!(!argv.iter().any(|a| a == "+cgop"));
        assert!(argv.windows(2).any(|w| w == ["-sc_threshold", "0"]));
    }

    #[test]
    fn fractional_seconds_render_without_trailing_zeros() {
        let (argv, _) = plan(1.5, "seconds", false, true, 23, "medium", "in.mov").unwrap();
        assert!(argv
            .windows(2)
            .any(|w| w == ["-force_key_frames", "expr:gte(t,n_forced*1.5)"]));
    }

    #[test]
    fn frames_interval_rounds_to_a_whole_gop() {
        let (argv, _) = plan(59.6, "frames", false, true, 23, "medium", "in.mp4").unwrap();
        assert!(argv.windows(2).any(|w| w == ["-g", "60"]));
    }

    #[test]
    fn boundaries_are_accepted() {
        assert!(plan(0.1, "seconds", false, true, 1, "ultrafast", "in.mp4").is_ok());
        assert!(plan(60.0, "seconds", false, true, 51, "veryslow", "in.mp4").is_ok());
        assert!(plan(1.0, "frames", false, true, 23, "medium", "in.mp4").is_ok());
        assert!(plan(3000.0, "frames", false, true, 23, "medium", "in.mp4").is_ok());
    }

    #[test]
    fn rejects_out_of_range_and_bad_values() {
        assert!(plan(0.0, "seconds", false, true, 23, "medium", "in.mp4").is_err());
        assert!(plan(-2.0, "seconds", false, true, 23, "medium", "in.mp4").is_err());
        assert!(plan(0.05, "seconds", false, true, 23, "medium", "in.mp4").is_err());
        assert!(plan(60.1, "seconds", false, true, 23, "medium", "in.mp4").is_err());
        assert!(plan(0.5, "frames", false, true, 23, "medium", "in.mp4").is_err());
        assert!(plan(3001.0, "frames", false, true, 23, "medium", "in.mp4").is_err());
        assert!(plan(2.0, "seconds", false, true, 52, "medium", "in.mp4").is_err());
        assert!(plan(2.0, "seconds", false, true, 0, "medium", "in.mp4").is_err());
        assert!(plan(2.0, "seconds", false, true, 23, "sloth", "in.mp4").is_err());
        assert!(plan(2.0, "minutes", false, true, 23, "medium", "in.mp4").is_err());
        assert!(plan(f64::NAN, "seconds", false, true, 23, "medium", "in.mp4").is_err());
    }

    #[test]
    fn error_messages_say_what_was_expected() {
        let e = plan(90.0, "seconds", false, true, 23, "medium", "in.mp4").unwrap_err();
        assert!(e.contains("0.1-60 seconds"), "{e}");
        let e = plan(2.0, "seconds", false, true, 99, "medium", "in.mp4").unwrap_err();
        assert!(e.contains("1-51"), "{e}");
    }
}
