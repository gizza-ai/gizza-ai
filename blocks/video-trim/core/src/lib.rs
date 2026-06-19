//! gizza-ai/video-trim core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Trim a video to a `[start, start+duration]` window using stream-copy
//! (`-c copy`, no re-encode), writing mp4. Stream-copy preserves the source
//! codecs and is fast — but requires the source streams be mp4-compatible
//! (h264/aac); otherwise ffmpeg fails with a clear error. Placing `-ss` before
//! `-i` makes it an input-seek (fast, keyframe-accurate).

/// Output container — stream-copy always writes mp4.
pub const OUT_NAME: &str = "out.mp4";

/// Build the ffmpeg argv (no leading `ffmpeg`) for a stream-copy trim.
///
/// `-ss <start>` (input seek) → `-i <in>` → `-t <duration>` → `-c copy` →
/// `<out>`. Shared verbatim by the web page (`build_argv`) and the chat block.
pub fn build_argv(in_name: &str, out_name: &str, start: f64, duration: f64) -> Vec<String> {
    vec![
        "-ss".to_string(),
        format!("{start}"),
        "-i".to_string(),
        in_name.to_string(),
        "-t".to_string(),
        format!("{duration}"),
        "-c".to_string(),
        "copy".to_string(),
        out_name.to_string(),
    ]
}

/// Validate `start`/`duration` and return `(argv, out_name)` for an input file.
/// `out_name` is always `out.mp4` (stream-copy keeps the source codecs but the
/// container is mp4). Single source shared by the chat block (`src/lib.rs`) and
/// the web page (`web/src/lib.rs`).
pub fn plan_trim(in_name: &str, start: f64, duration: f64) -> Result<(Vec<String>, String), String> {
    if !start.is_finite() || start < 0.0 {
        return Err(format!(
            "start must be >= 0 and finite, got {start}"
        ));
    }
    if !duration.is_finite() || duration <= 0.0 {
        return Err(format!(
            "duration must be > 0 and finite, got {duration}"
        ));
    }
    let out_name = OUT_NAME.to_string();
    Ok((build_argv(in_name, &out_name, start, duration), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_order_and_values() {
        let argv = build_argv("in.mp4", "out.mp4", 1.5, 3.0);
        assert_eq!(
            argv,
            vec![
                "-ss", "1.5", "-i", "in.mp4", "-t", "3", "-c", "copy", "out.mp4",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn argv_uses_stream_copy_and_mp4_out() {
        let argv = build_argv("in.webm", "out.mp4", 0.0, 2.0);
        assert!(argv.windows(2).any(|w| w[0] == "-c" && w[1] == "copy"));
        assert_eq!(argv.first().map(String::as_str), Some("-ss"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn plan_trim_returns_mp4_and_valid_argv() {
        let (argv, out) = plan_trim("clip.mov", 5.0, 10.0).unwrap();
        assert_eq!(out, "out.mp4");
        let i = argv.iter().position(|a| a == "-ss").unwrap();
        assert_eq!(argv[i + 1], "5");
        let i = argv.iter().position(|a| a == "-t").unwrap();
        assert_eq!(argv[i + 1], "10");
    }

    #[test]
    fn plan_trim_rejects_negative_start() {
        assert!(plan_trim("in.mp4", -1.0, 2.0).is_err());
    }

    #[test]
    fn plan_trim_rejects_nonpositive_duration() {
        assert!(plan_trim("in.mp4", 0.0, 0.0).is_err());
        assert!(plan_trim("in.mp4", 0.0, -3.0).is_err());
    }

    #[test]
    fn plan_trim_rejects_non_finite() {
        assert!(plan_trim("in.mp4", f64::NAN, 2.0).is_err());
        assert!(plan_trim("in.mp4", 0.0, f64::INFINITY).is_err());
    }

    #[test]
    fn start_zero_is_accepted() {
        let (argv, _) = plan_trim("in.mp4", 0.0, 1.0).unwrap();
        let i = argv.iter().position(|a| a == "-ss").unwrap();
        assert_eq!(argv[i + 1], "0");
    }
}
