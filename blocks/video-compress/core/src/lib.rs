//! gizza-ai/video-compress core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Single-pass CRF re-encode to shrink a video's file size while keeping the
//! container format. Higher CRF = smaller file / lower quality. True
//! target-byte-size compression needs a 2-pass encode (a follow-up — a single
//! `build_argv → ffmpegExec` call can only do one pass), so this exposes a
//! quality (CRF) knob instead.

/// Default CRF — a sensible "noticeably smaller, still good" setting for H.264.
pub const DEFAULT_CRF: u32 = 28;
/// Lowest CRF we accept (higher quality / larger file).
pub const MIN_CRF: u32 = 18;
/// Highest CRF we accept (lower quality / smaller file).
pub const MAX_CRF: u32 = 34;

/// Clamp a (possibly non-finite) CRF request into `MIN_CRF..=MAX_CRF`.
///
/// `crf <= 0` (the page's "unset" sentinel) and any non-finite value fall back
/// to [`DEFAULT_CRF`]. Fractional values are floored to an integer because
/// libx264's `-crf` is an integer scale.
pub fn clamp_crf(crf: f64) -> u32 {
    if !crf.is_finite() || crf <= 0.0 {
        return DEFAULT_CRF;
    }
    (crf as u32).clamp(MIN_CRF, MAX_CRF)
}

/// Pick the output container extension. Keep the input file's extension so the
/// format is preserved; fall back to `mp4` when the input has no usable one.
fn out_ext(in_name: &str) -> &str {
    in_name
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .filter(|ext| !ext.is_empty())
        .unwrap_or("mp4")
}

/// Build the ffmpeg argv (no leading `ffmpeg`) and the output filename for a
/// single-pass CRF re-encode. Shared verbatim by the web page (`build_argv`)
/// and the chat block.
///
/// H.264 video (`libx264`, `-preset medium`) + AAC audio, keeping the input
/// container extension. `crf` is clamped via [`clamp_crf`].
pub fn build_argv(crf: f64, in_name: &str) -> (Vec<String>, String) {
    let crf = clamp_crf(crf);
    let ext = out_ext(in_name);
    let out_name = format!("out.{ext}");
    let argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-crf".to_string(),
        crf.to_string(),
        "-preset".to_string(),
        "medium".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        out_name.clone(),
    ];
    (argv, out_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_crf_applied_when_unset() {
        // 0.0 is the page's "unset" sentinel → DEFAULT_CRF (28).
        let (argv, out) = build_argv(0.0, "in.mp4");
        assert_eq!(out, "out.mp4");
        let i = argv.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(argv[i + 1], "28");
        // Full expected argv for the default case.
        assert_eq!(
            argv,
            vec![
                "-i", "in.mp4", "-c:v", "libx264", "-crf", "28", "-preset", "medium", "-c:a",
                "aac", "out.mp4",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn argv_contains_explicit_crf() {
        let (argv, _) = build_argv(28.0, "in.mp4");
        let i = argv.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(argv[i + 1], "28");
    }

    #[test]
    fn higher_crf_is_accepted_within_range() {
        let (argv, _) = build_argv(30.0, "in.mp4");
        let i = argv.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(argv[i + 1], "30");
    }

    #[test]
    fn crf_clamped_below_min_and_above_max() {
        assert_eq!(clamp_crf(5.0), MIN_CRF); // below 18 → 18
        assert_eq!(clamp_crf(99.0), MAX_CRF); // above 34 → 34
        assert_eq!(clamp_crf(18.0), 18);
        assert_eq!(clamp_crf(34.0), 34);
    }

    #[test]
    fn non_finite_and_nonpositive_crf_fall_back_to_default() {
        assert_eq!(clamp_crf(f64::NAN), DEFAULT_CRF);
        assert_eq!(clamp_crf(f64::INFINITY), DEFAULT_CRF);
        assert_eq!(clamp_crf(0.0), DEFAULT_CRF);
        assert_eq!(clamp_crf(-3.0), DEFAULT_CRF);
    }

    #[test]
    fn fractional_crf_is_floored() {
        assert_eq!(clamp_crf(23.9), 23);
    }

    #[test]
    fn keeps_input_container_extension() {
        let (_, out) = build_argv(28.0, "clip.webm");
        assert_eq!(out, "out.webm");
        let (_, out) = build_argv(28.0, "clip.mov");
        assert_eq!(out, "out.mov");
    }

    #[test]
    fn defaults_extension_to_mp4_when_unknown() {
        let (_, out) = build_argv(28.0, "in");
        assert_eq!(out, "out.mp4");
        let (_, out) = build_argv(28.0, "in.");
        assert_eq!(out, "out.mp4");
    }

    #[test]
    fn argv_uses_h264_and_aac() {
        let (argv, _) = build_argv(28.0, "in.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
        assert!(argv.windows(2).any(|w| w[0] == "-preset" && w[1] == "medium"));
        assert_eq!(argv.first().map(String::as_str), Some("-i"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }
}
