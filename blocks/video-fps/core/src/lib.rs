//! gizza-ai/video-fps core — pure ffmpeg argv construction shared by the chat
//! block and the standalone web page. No wasm-bindgen deps.
//!
//! Changes a video's frame rate to a fixed target (e.g. 60→30, or any chosen
//! fps) using ffmpeg's `fps` video filter, which drops frames when lowering the
//! rate and duplicates frames when raising it — the clip's DURATION is
//! unchanged, only how many frames per second it holds. Because the frame
//! timing changes, the video stream must be re-encoded (H.264, `-crf 20` for a
//! visually near-transparent result). Audio is untouched — it is stream-copied
//! when the container is kept, and only re-encoded to AAC when the input
//! container can't hold H.264/AAC (e.g. webm), in which case the output
//! switches to MP4 (see `gizza_ai_block_utils::ffmpeg::h264_out_ext`).

/// Default target fps when the request is unset (the page's `0` sentinel) or
/// non-finite — 30 fps is the most common re-timing target.
pub const DEFAULT_FPS: f64 = 30.0;
/// Lowest target frame rate we accept.
pub const MIN_FPS: f64 = 1.0;
/// Highest target frame rate we accept.
pub const MAX_FPS: f64 = 240.0;

/// Resolve a (possibly unset / non-finite) fps request into an accepted target.
///
/// `fps <= 0` (the page's "unset" sentinel) and any non-finite value fall back
/// to [`DEFAULT_FPS`]; otherwise the value is clamped into `MIN_FPS..=MAX_FPS`.
pub fn resolve_fps(fps: f64) -> f64 {
    if !fps.is_finite() || fps <= 0.0 {
        return DEFAULT_FPS;
    }
    fps.clamp(MIN_FPS, MAX_FPS)
}

/// Format an fps value for the ffmpeg `fps=` filter. Whole numbers render
/// without a decimal (`30`), fractional rates keep their decimals (`29.97`).
fn fps_arg(fps: f64) -> String {
    format!("fps={fps}")
}

/// Build the ffmpeg argv (no leading `ffmpeg`) and the output filename for a
/// frame-rate change. Shared verbatim by the web page (`build_argv`) and the
/// chat block.
///
/// The `fps` filter re-times the video to `fps` frames/second (drop/dup);
/// H.264 video (`libx264`, `-preset medium`, `-crf 20`). The input container
/// extension is kept when it can hold H.264/AAC, otherwise the output is
/// `out.mp4`. `fps` is resolved via [`resolve_fps`].
pub fn build_argv(fps: f64, in_name: &str) -> (Vec<String>, String) {
    let fps = resolve_fps(fps);
    let (ext, reencode_audio) = gizza_ai_block_utils::ffmpeg::h264_out_ext(in_name);
    let out_name = format!("out.{ext}");
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vf".to_string(),
        fps_arg(fps),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "medium".to_string(),
        "-crf".to_string(),
        "20".to_string(),
        "-c:a".to_string(),
    ];
    // Keep the audio bit-for-bit when the container is preserved; only re-encode
    // to AAC when we had to switch to MP4 (the source codec may not fit).
    argv.push(if reencode_audio { "aac" } else { "copy" }.to_string());
    argv.push(out_name.clone());
    (argv, out_name)
}

/// Validate the fps request and return `(argv, out_name)`. A non-finite value
/// is rejected up front (a clear user error); an unset (`0`) or out-of-range
/// request is resolved/clamped by [`resolve_fps`].
pub fn plan(fps: f64, in_name: &str) -> Result<(Vec<String>, String), String> {
    if fps.is_nan() || fps.is_infinite() {
        return Err("fps must be a finite number".into());
    }
    Ok(build_argv(fps, in_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_fps_applied_when_unset() {
        let (argv, out) = build_argv(0.0, "in.mp4");
        assert_eq!(out, "out.mp4");
        let i = argv.iter().position(|a| a == "-vf").unwrap();
        assert_eq!(argv[i + 1], "fps=30");
    }

    #[test]
    fn full_default_argv() {
        let (argv, _) = build_argv(30.0, "in.mp4");
        assert_eq!(
            argv,
            vec![
                "-i", "in.mp4", "-vf", "fps=30", "-c:v", "libx264", "-preset", "medium", "-crf",
                "20", "-c:a", "copy", "out.mp4",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn explicit_fps_is_used() {
        let (argv, _) = build_argv(24.0, "in.mp4");
        let i = argv.iter().position(|a| a == "-vf").unwrap();
        assert_eq!(argv[i + 1], "fps=24");
    }

    #[test]
    fn fractional_fps_keeps_decimals() {
        let (argv, _) = build_argv(29.97, "in.mp4");
        let i = argv.iter().position(|a| a == "-vf").unwrap();
        assert_eq!(argv[i + 1], "fps=29.97");
    }

    #[test]
    fn fps_clamped_below_min_and_above_max() {
        assert_eq!(resolve_fps(0.5), MIN_FPS);
        assert_eq!(resolve_fps(1000.0), MAX_FPS);
        assert_eq!(resolve_fps(1.0), 1.0);
        assert_eq!(resolve_fps(240.0), 240.0);
    }

    #[test]
    fn non_finite_and_nonpositive_fall_back_to_default() {
        assert_eq!(resolve_fps(f64::NAN), DEFAULT_FPS);
        assert_eq!(resolve_fps(f64::INFINITY), DEFAULT_FPS);
        assert_eq!(resolve_fps(0.0), DEFAULT_FPS);
        assert_eq!(resolve_fps(-5.0), DEFAULT_FPS);
    }

    #[test]
    fn keeps_h264_capable_container_extensions() {
        for ext in ["mp4", "mov", "m4v", "mkv"] {
            let (argv, out) = build_argv(30.0, &format!("clip.{ext}"));
            assert_eq!(out, format!("out.{ext}"));
            // Container kept → audio is stream-copied, not re-encoded.
            assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "copy"));
        }
    }

    #[test]
    fn webm_input_switches_to_mp4_and_reencodes_audio() {
        let (argv, out) = build_argv(30.0, "clip.webm");
        assert_eq!(out, "out.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
    }

    #[test]
    fn argv_uses_fps_filter_and_h264() {
        let (argv, _) = build_argv(30.0, "in.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(argv.windows(2).any(|w| w[0] == "-crf" && w[1] == "20"));
        assert_eq!(argv.first().map(String::as_str), Some("-i"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn plan_rejects_non_finite() {
        assert!(plan(f64::NAN, "in.mp4").is_err());
        assert!(plan(f64::INFINITY, "in.mp4").is_err());
        assert!(plan(30.0, "in.mp4").is_ok());
        // Plain unset sentinel is accepted (resolves to default).
        assert!(plan(0.0, "in.mp4").is_ok());
    }
}
