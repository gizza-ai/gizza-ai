//! gizza-ai/mp4-to-m4a core — pure ffmpeg argv construction shared by the chat
//! block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! mp4-to-m4a pulls the audio track out of an MP4 (ISO-BMFF) and rewraps it into
//! an `.m4a` container by *stream-copying* — `-i in.mp4 -vn -map 0:a:0 -c:a copy
//! out.m4a`. Nothing is re-encoded: the already-compressed audio packets (AAC,
//! ALAC, …) are copied straight across, so the extraction is lossless and
//! near-instant.
//!
//! `-vn` drops the video stream, `-map 0:a:0` selects the *first* audio stream
//! only, and `-c:a copy` copies its packets with no encoder. Because `.m4a` is
//! the audio-only profile of the very same MP4/ISO-BMFF container the source
//! uses, whatever audio codec the MP4 held fits unchanged — so this tool has NO
//! re-encode mode. To change codec/bitrate (e.g. AAC → MP3), use audio-convert
//! or extract-audio-from-video, which genuinely re-encode.

/// Build the ffmpeg argv (no leading `ffmpeg`) to remux the first audio stream
/// of `in_name` → `out_name`.
///
/// `-vn` discards video, `-map 0:a:0` selects only the first audio stream, and
/// `-c:a copy` stream-copies it into the M4A container — lossless, no re-encode.
pub fn build_argv(in_name: &str, out_name: &str) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-vn".into(),
        "-map".into(),
        "0:a:0".into(),
        "-c:a".into(),
        "copy".into(),
        out_name.into(),
    ]
}

/// Build `(argv, out_name)` for an input file. `out_name` is always `out.m4a`.
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
pub fn plan(in_name: &str) -> Result<(Vec<String>, String), String> {
    let out_name = "out.m4a".to_string();
    Ok((build_argv(in_name, &out_name), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_outputs_m4a() {
        let (argv, out) = plan("in.mp4").unwrap();
        assert_eq!(out, "out.m4a");
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "in.mp4");
        assert_eq!(argv.last().map(String::as_str), Some("out.m4a"));
    }

    #[test]
    fn argv_is_the_exact_lossless_audio_remux_plan() {
        let argv = build_argv("in.mp4", "out.m4a");
        assert_eq!(
            argv,
            vec!["-i", "in.mp4", "-vn", "-map", "0:a:0", "-c:a", "copy", "out.m4a"]
        );
    }

    #[test]
    fn argv_drops_video_and_never_re_encodes() {
        let argv = build_argv("in.mp4", "out.m4a");
        // -vn drops video; -map 0:a:0 keeps only the first audio stream.
        assert!(argv.iter().any(|a| a == "-vn"));
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "0:a:0"));
        // -c:a copy means stream-copy — no audio encoder is ever invoked.
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "copy"));
        assert!(!argv.iter().any(|a| a == "aac" || a == "libmp3lame" || a == "libfdk_aac"));
        assert!(!argv.iter().any(|a| a == "-b:a"));
    }

    #[test]
    fn argv_preserves_input_name() {
        let argv = build_argv("in.m4v", "out.m4a");
        assert_eq!(argv[1], "in.m4v");
    }
}
