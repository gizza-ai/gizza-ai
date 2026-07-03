//! gizza-ai/video-trim core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Trim a video to a `[start, start+duration]` window using stream-copy
//! (`-c copy`, no re-encode). A stream copy is always valid back into the
//! source's OWN container, so the output keeps the input container (webm →
//! out.webm, mp4 → out.mp4, …) — see
//! `gizza_ai_block_utils::ffmpeg::copy_out_ext`. This is lossless and fast;
//! placing `-ss` before `-i` makes it an input-seek (fast, keyframe-accurate).

use gizza_ai_block_utils::ffmpeg::copy_out_ext;

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
/// `out_name` keeps the input container (a stream copy is always valid in its
/// own container); an unknown/missing input extension falls back to `out.mp4`
/// so the page still resolves a MIME. Single source shared by the chat block
/// (`src/lib.rs`) and the web page (`web/src/lib.rs`).
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
    let out_name = format!("out.{}", copy_out_ext(in_name));
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
    fn argv_uses_stream_copy() {
        let argv = build_argv("in.webm", "out.webm", 0.0, 2.0);
        assert!(argv.windows(2).any(|w| w[0] == "-c" && w[1] == "copy"));
        assert_eq!(argv.first().map(String::as_str), Some("-ss"));
        assert_eq!(argv.last().map(String::as_str), Some("out.webm"));
    }

    #[test]
    fn plan_trim_returns_valid_argv() {
        let (argv, out) = plan_trim("clip.mov", 5.0, 10.0).unwrap();
        assert_eq!(out, "out.mov");
        let i = argv.iter().position(|a| a == "-ss").unwrap();
        assert_eq!(argv[i + 1], "5");
        let i = argv.iter().position(|a| a == "-t").unwrap();
        assert_eq!(argv[i + 1], "10");
    }

    #[test]
    fn plan_trim_keeps_the_source_container() {
        // A stream copy is valid in its own container — webm stays webm (VP8/
        // Vorbis into mp4 hard-fails), mp4/mov/mkv/m4v keep theirs.
        for ext in ["mp4", "mov", "mkv", "m4v", "webm"] {
            let (argv, out) = plan_trim(&format!("in.{ext}"), 0.0, 1.0).unwrap();
            assert_eq!(out, format!("out.{ext}"), "{ext}");
            assert_eq!(argv.last().map(String::as_str), Some(out.as_str()));
        }
    }

    #[test]
    fn plan_trim_falls_back_to_mp4_for_unknown_container() {
        // Unknown/missing extension → out.mp4 so the page still resolves a MIME.
        let (_, out) = plan_trim("clip.avi", 0.0, 1.0).unwrap();
        assert_eq!(out, "out.mp4");
        let (_, out) = plan_trim("noext", 0.0, 1.0).unwrap();
        assert_eq!(out, "out.mp4");
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
