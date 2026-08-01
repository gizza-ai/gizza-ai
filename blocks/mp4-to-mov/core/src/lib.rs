//! gizza-ai/mp4-to-mov core — pure ffmpeg argv construction shared by the chat
//! block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! mp4-to-mov rewraps an MP4 (ISO-BMFF) into a QuickTime `.mov` container by
//! *stream-copying* every track — `-i in.mp4 -map 0 -c copy -movflags +faststart
//! out.mov`. Nothing is re-encoded: the already-compressed video, audio and data
//! packets are copied straight across, so the conversion is lossless and
//! near-instant. `-movflags +faststart` moves the `moov` atom to the front so the
//! `.mov` starts playing before it has fully downloaded.
//!
//! QuickTime `.mov` is the ISO-BMFF ancestor of MP4 and a *superset* container:
//! it accepts every codec MP4 can carry (H.264/HEVC/MPEG-4 video, AAC/AC-3/PCM
//! audio) plus Apple-only ones like ProRes. So the remux always succeeds and, as
//! with `mp4-to-mkv`, there is NO re-encode mode. The point of moving to `.mov`
//! is Apple-native workflows — Final Cut Pro, iMovie, QuickTime Player and other
//! macOS tools prefer/expect the `.mov` wrapper. Re-encoding (changing
//! codec/quality) is out of scope; use video-transcode / video-compress.

/// Build the ffmpeg argv (no leading `ffmpeg`) to remux `in_name` → `out_name`.
///
/// `-map 0` selects **every** stream from the input (not just the default video
/// + first audio), `-c copy` stream-copies them all into the QuickTime container
/// (lossless, no re-encode), and `-movflags +faststart` relocates the `moov`
/// atom to the front for progressive playback.
pub fn build_argv(in_name: &str, out_name: &str) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-map".into(),
        "0".into(),
        "-c".into(),
        "copy".into(),
        "-movflags".into(),
        "+faststart".into(),
        out_name.into(),
    ]
}

/// Build `(argv, out_name)` for an input file. `out_name` is always `out.mov`.
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
pub fn plan(in_name: &str) -> Result<(Vec<String>, String), String> {
    let out_name = "out.mov".to_string();
    Ok((build_argv(in_name, &out_name), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_outputs_mov() {
        let (argv, out) = plan("in.mp4").unwrap();
        assert_eq!(out, "out.mov");
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "in.mp4");
        assert_eq!(argv.last().map(String::as_str), Some("out.mov"));
    }

    #[test]
    fn argv_stream_copies_every_track_no_reencode() {
        let argv = build_argv("in.mp4", "out.mov");
        // -map 0 keeps every stream; -c copy means no re-encode.
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "0"));
        assert!(argv.windows(2).any(|w| w[0] == "-c" && w[1] == "copy"));
        // faststart so the .mov plays before fully downloading.
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-movflags" && w[1] == "+faststart"));
        // A remux must never invoke an encoder.
        assert!(!argv.iter().any(|a| a == "libx264" || a == "prores_ks" || a == "aac"));
        assert!(!argv.iter().any(|a| a == "-crf"));
    }

    #[test]
    fn argv_preserves_input_name() {
        let argv = build_argv("in.m4v", "out.mov");
        assert_eq!(argv[1], "in.m4v");
    }
}
