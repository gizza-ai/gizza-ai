//! gizza-ai/video-timelapse core — pure ffmpeg argv construction shared by the
//! chat block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Turns long footage into a timelapse: the video is sped up by `speed`
//! (`setpts=PTS/speed` compresses the presentation timestamps) and then
//! re-sampled to a fixed output rate with the `fps` filter, which DROPS the
//! excess frames the speed-up crammed in — that frame-drop is what makes a
//! timelapse cheap and smooth rather than a stuttering all-frames blur. Audio is
//! always dropped (`-an`): a 20×-fast soundtrack is noise, and dropping it keeps
//! the output small. The sped-up stream is re-encoded to H.264 (`-crf 20`,
//! visually near-transparent). mp4/mov/m4v/mkv inputs keep their container; any
//! other input (e.g. webm) comes out as MP4 — see
//! `gizza_ai_block_utils::ffmpeg::h264_out_ext`.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;

/// Default speed-up factor when the request is unset (the page's `0` sentinel)
/// or non-finite — 10× is a good general timelapse starting point.
pub const DEFAULT_SPEED: f64 = 10.0;
/// Slowest speed-up we accept (anything ≤1 isn't a timelapse).
pub const MIN_SPEED: f64 = 2.0;
/// Fastest speed-up we accept.
pub const MAX_SPEED: f64 = 300.0;

/// Default output frame rate when `fps` is unset (0) or non-finite.
pub const DEFAULT_FPS: f64 = 30.0;
/// Lowest output frame rate we accept.
pub const MIN_FPS: f64 = 1.0;
/// Highest output frame rate we accept.
pub const MAX_FPS: f64 = 60.0;

/// Resolve a (possibly unset / non-finite) speed request into an accepted
/// factor. `speed <= 0` (the page's "unset" sentinel) and any non-finite value
/// fall back to [`DEFAULT_SPEED`]; otherwise the value is clamped into
/// `MIN_SPEED..=MAX_SPEED`.
pub fn resolve_speed(speed: f64) -> f64 {
    if !speed.is_finite() || speed <= 0.0 {
        return DEFAULT_SPEED;
    }
    speed.clamp(MIN_SPEED, MAX_SPEED)
}

/// Resolve a (possibly unset / non-finite) fps request into an accepted rate.
pub fn resolve_fps(fps: f64) -> f64 {
    if !fps.is_finite() || fps <= 0.0 {
        return DEFAULT_FPS;
    }
    fps.clamp(MIN_FPS, MAX_FPS)
}

/// Format an f64 for an ffmpeg filter without a trailing `.0` on whole numbers
/// (`10.0` → `10`, `29.97` stays `29.97`) — keeps the filter string and tests tidy.
fn num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) and the output filename for a
/// timelapse. Shared verbatim by the web page (`build_argv`) and the chat block.
///
/// `setpts=PTS/{speed}` compresses the timeline (a 60 s clip at 10× → 6 s), then
/// `fps={fps}` re-times the result to a fixed output rate, dropping the surplus
/// frames. Audio is dropped (`-an`). Video is H.264 (`libx264`, `-preset
/// medium`, `-crf 20`). The input container extension is kept when it can hold
/// H.264, otherwise the output is `out.mp4`. `speed`/`fps` are resolved via
/// [`resolve_speed`]/[`resolve_fps`].
pub fn build_argv(speed: f64, fps: f64, in_name: &str) -> (Vec<String>, String) {
    let speed = resolve_speed(speed);
    let fps = resolve_fps(fps);
    // Audio is dropped, so the `h264_out_ext` audio-reencode flag is irrelevant.
    let (ext, _reencode_audio) = h264_out_ext(in_name);
    let out_name = format!("out.{ext}");
    let vf = format!("setpts=PTS/{},fps={}", num(speed), num(fps));
    let argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vf".to_string(),
        vf,
        "-an".to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "medium".to_string(),
        "-crf".to_string(),
        "20".to_string(),
        // 8-bit 4:2:0 chroma for universal playback (some sources are yuv444);
        // move the moov atom up front so the result streams/scrubs instantly.
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        out_name.clone(),
    ];
    (argv, out_name)
}

/// Validate the requests and return `(argv, out_name)`. Non-finite `speed`/`fps`
/// are rejected up front (a clear user error); an unset (`0`) or out-of-range
/// request is resolved/clamped by [`resolve_speed`]/[`resolve_fps`].
pub fn plan(speed: f64, fps: f64, in_name: &str) -> Result<(Vec<String>, String), String> {
    if speed.is_nan() || speed.is_infinite() {
        return Err("speed must be a finite number".into());
    }
    if fps.is_nan() || fps.is_infinite() {
        return Err("fps must be a finite number".into());
    }
    Ok(build_argv(speed, fps, in_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vf(argv: &[String]) -> String {
        let i = argv.iter().position(|a| a == "-vf").unwrap();
        argv[i + 1].clone()
    }

    #[test]
    fn builds_setpts_fps_and_drops_audio() {
        let (argv, out) = build_argv(10.0, 30.0, "in.mp4");
        assert_eq!(vf(&argv), "setpts=PTS/10,fps=30");
        assert!(argv.iter().any(|a| a == "-an"), "audio must be dropped");
        assert!(argv.iter().any(|a| a == "libx264"));
        assert_eq!(out, "out.mp4");
    }

    #[test]
    fn keeps_mov_container_switches_webm_to_mp4() {
        let (_, mov) = build_argv(8.0, 24.0, "clip.mov");
        assert_eq!(mov, "out.mov");
        let (_, webm) = build_argv(8.0, 24.0, "clip.webm");
        assert_eq!(webm, "out.mp4");
    }

    #[test]
    fn fractional_fps_keeps_decimals() {
        let (argv, _) = build_argv(12.0, 29.97, "in.mp4");
        assert_eq!(vf(&argv), "setpts=PTS/12,fps=29.97");
    }

    #[test]
    fn unset_and_out_of_range_are_resolved() {
        // page "unset" sentinel 0 → defaults
        assert_eq!(resolve_speed(0.0), DEFAULT_SPEED);
        assert_eq!(resolve_fps(0.0), DEFAULT_FPS);
        // clamped
        assert_eq!(resolve_speed(1.0), MIN_SPEED);
        assert_eq!(resolve_speed(1000.0), MAX_SPEED);
        assert_eq!(resolve_fps(0.5), MIN_FPS);
        assert_eq!(resolve_fps(120.0), MAX_FPS);
        // build with the sentinels applies the defaults
        let (argv, _) = build_argv(0.0, 0.0, "in.mp4");
        assert_eq!(vf(&argv), "setpts=PTS/10,fps=30");
    }

    #[test]
    fn plan_rejects_non_finite_but_accepts_valid() {
        assert!(plan(f64::NAN, 30.0, "i.mp4").is_err());
        assert!(plan(10.0, f64::INFINITY, "i.mp4").is_err());
        let (argv, out) = plan(20.0, 25.0, "clip.mp4").unwrap();
        assert_eq!(out, "out.mp4");
        assert!(argv.iter().any(|a| a == "setpts=PTS/20,fps=25"));
    }
}
