//! gizza-ai/mp4-to-mkv core — pure ffmpeg argv construction shared by the chat
//! block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! mp4-to-mkv rewraps an MP4 (ISO-BMFF) into a Matroska `.mkv` container by
//! *stream-copying* every track — `-i in.mp4 -map 0 -c copy out.mkv`. Nothing is
//! re-encoded: the already-compressed video, audio, subtitle and data packets
//! are copied straight across, so the conversion is lossless and near-instant.
//!
//! Unlike `mov-to-mp4` (which needs a `transcode` fallback because MP4 can't hold
//! ProRes and a few other MOV codecs), this tool has NO re-encode mode: MKV is a
//! superset container that accepts essentially every codec MP4 can carry
//! (H.264/HEVC/AV1/MPEG-4 video, AAC/AC-3/MP3 audio, …), so the remux always
//! succeeds. The point of moving to MKV is to be able to later add soft
//! subtitles or extra audio tracks — things MP4 handles poorly. Re-encoding
//! (changing codec/quality) is out of scope; use video-transcode / video-compress.

/// Build the ffmpeg argv (no leading `ffmpeg`) to remux `in_name` → `out_name`.
///
/// `-map 0` selects **every** stream from the input (not just the default video
/// + first audio), and `-c copy` stream-copies them all into the Matroska
/// container — lossless, no re-encode.
pub fn build_argv(in_name: &str, out_name: &str) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-map".into(),
        "0".into(),
        "-c".into(),
        "copy".into(),
        out_name.into(),
    ]
}

/// Build `(argv, out_name)` for an input file. `out_name` is always `out.mkv`.
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
pub fn plan(in_name: &str) -> Result<(Vec<String>, String), String> {
    let out_name = "out.mkv".to_string();
    Ok((build_argv(in_name, &out_name), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_outputs_mkv() {
        let (argv, out) = plan("in.mp4").unwrap();
        assert_eq!(out, "out.mkv");
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "in.mp4");
        assert_eq!(argv.last().map(String::as_str), Some("out.mkv"));
    }

    #[test]
    fn argv_stream_copies_every_track_no_reencode() {
        let argv = build_argv("in.mp4", "out.mkv");
        // -map 0 keeps every stream; -c copy means no re-encode.
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "0"));
        assert!(argv.windows(2).any(|w| w[0] == "-c" && w[1] == "copy"));
        // A remux must never invoke an encoder.
        assert!(!argv.iter().any(|a| a == "libx264" || a == "libvpx-vp9" || a == "aac"));
        assert!(!argv.iter().any(|a| a == "-crf"));
    }

    #[test]
    fn argv_preserves_input_name() {
        let argv = build_argv("in.m4v", "out.mkv");
        assert_eq!(argv[1], "in.m4v");
    }
}
