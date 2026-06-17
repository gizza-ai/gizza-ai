//! gizza-ai/video-silence-cut core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! # IMPORTANT — single-pass approximation (read before changing the argv)
//!
//! The gizza ffmpeg page/chat model is exactly **one** `build_argv → one
//! ffmpegExec` invocation. A *correct* silence-cut (jump-cut editing that removes
//! silent gaps while keeping audio and video in sync) is classically a **two-pass**
//! operation: run `silencedetect` (an `A->A` filter that prints silence_start/
//! silence_end to stderr), parse those timestamps, then a second pass that
//! `select`/`atrim`s the non-silent segments. A single ffmpeg invocation cannot
//! read its own first-pass stderr, so that approach is impossible here.
//!
//! ffmpeg's only single-pass silence remover, `silenceremove`, is strictly
//! `A->A`: it drops audio samples but has no way to drop the *corresponding*
//! video frames, and no filter exposes its per-sample decisions as per-video-frame
//! metadata that a synchronized video `select` could read. The result is A/V
//! **desync** — the audio shortens while the video does not.
//!
//! This tool therefore ships the best honest single-pass **approximation**:
//! `silenceremove` de-silences the audio and the video is truncated to the kept
//! audio length (`-shortest`). The output is a valid, shorter video whose audio is
//! correctly de-silenced, but whose **video content drifts out of sync after the
//! first removed gap** (the kept video frames are the front of the source, not the
//! frames that belong under the kept audio). A correct cut needs runtime support
//! for a two-pass / stderr-feedback flow (see the PR for the producer ask).

/// Build the ffmpeg argv for the silence-cut **approximation** (no leading
/// "ffmpeg"). `threshold_db` is the silence threshold in dB (e.g. -30.0 → quieter
/// than -30 dB counts as silence); `min_silence_s` is the minimum silent-gap
/// length in seconds that gets trimmed. `out_name` is the produced filename.
///
/// The graph: `silenceremove=stop_periods=-1:stop_duration=<min>:stop_threshold=<db>dB`
/// removes every internal silent run longer than `min_silence_s`; the video is
/// re-encoded (h264/yuv420p) and the container truncated to the shorter stream via
/// `-fflags +shortest`. See the module docs for why this is only an approximation.
pub fn build_argv(in_name: &str, out_name: &str, threshold_db: f64, min_silence_s: f64) -> Vec<String> {
    // ffmpeg accepts a plain number for stop_threshold's dB form when suffixed
    // with "dB"; emit a stable, locale-independent representation.
    let af = format!(
        "silenceremove=stop_periods=-1:stop_duration={}:stop_threshold={}dB",
        fmt_num(min_silence_s),
        fmt_num(threshold_db),
    );
    vec![
        "-i".into(),
        in_name.into(),
        "-af".into(),
        af,
        "-c:v".into(),
        "libx264".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "aac".into(),
        "-fflags".into(),
        "+shortest".into(),
        out_name.into(),
    ]
}

/// Format an f64 without a trailing ".0" so the argv matches what a human would
/// type (`-30` not `-30.0`, `0.5` stays `0.5`). Keeps the ffmpeg arg compact and
/// deterministic across platforms.
fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        // Trim to 3 decimals; silence params don't need more precision.
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Validate params and return `(argv, out_name)` for an input file. Output is mp4
/// (the video is re-encoded to h264/aac, so the container is normalized regardless
/// of the input). Used by the web page (file in → file out) and the chat block.
pub fn plan(in_name: &str, threshold_db: f64, min_silence_s: f64) -> Result<(Vec<String>, String), String> {
    if !threshold_db.is_finite() {
        return Err("threshold_db must be a finite number (e.g. -30)".into());
    }
    if threshold_db > 0.0 {
        return Err("threshold_db must be <= 0 dB (silence is below 0 dB; e.g. -30)".into());
    }
    if !min_silence_s.is_finite() || min_silence_s <= 0.0 {
        return Err("min_silence must be a finite number > 0 (seconds)".into());
    }
    let _ = in_name;
    let out_name = "out.mp4".to_string();
    Ok((build_argv(in_name, &out_name, threshold_db, min_silence_s), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_has_expected_shape() {
        let argv = build_argv("in.mp4", "out.mp4", -30.0, 0.5);
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "in.mp4");
        assert_eq!(argv[2], "-af");
        assert_eq!(
            argv[3],
            "silenceremove=stop_periods=-1:stop_duration=0.5:stop_threshold=-30dB"
        );
        // re-encode + normalize container, truncate to shortest stream.
        assert!(argv.iter().any(|a| a == "libx264"));
        assert!(argv.iter().any(|a| a == "aac"));
        assert!(argv.windows(2).any(|w| w[0] == "-fflags" && w[1] == "+shortest"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn argv_formats_threshold_and_duration_cleanly() {
        // Integer-valued floats lose the ".0"; fractional values keep their digits.
        let argv = build_argv("in.mp4", "out.mp4", -25.0, 1.0);
        assert_eq!(
            argv[3],
            "silenceremove=stop_periods=-1:stop_duration=1:stop_threshold=-25dB"
        );
        let argv2 = build_argv("in.mp4", "out.mp4", -33.5, 0.25);
        assert_eq!(
            argv2[3],
            "silenceremove=stop_periods=-1:stop_duration=0.25:stop_threshold=-33.5dB"
        );
    }

    #[test]
    fn plan_returns_mp4_out_and_argv() {
        let (argv, out) = plan("in.webm", -30.0, 0.5).unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(argv[1], "in.webm");
        assert!(argv.iter().any(|a| a.starts_with("silenceremove=")));
    }

    #[test]
    fn plan_rejects_positive_threshold() {
        assert!(plan("in.mp4", 5.0, 0.5).is_err());
    }

    #[test]
    fn plan_rejects_non_positive_min_silence() {
        assert!(plan("in.mp4", -30.0, 0.0).is_err());
        assert!(plan("in.mp4", -30.0, -1.0).is_err());
    }

    #[test]
    fn plan_rejects_non_finite() {
        assert!(plan("in.mp4", f64::NAN, 0.5).is_err());
        assert!(plan("in.mp4", -30.0, f64::INFINITY).is_err());
    }
}
