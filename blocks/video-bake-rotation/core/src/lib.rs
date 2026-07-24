//! gizza-ai/video-bake-rotation core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page.
//!
//! This tool fixes the common "sideways phone video" class where orientation is
//! stored as container metadata / display matrix instead of pixels. ffmpeg
//! autorotates by default when decoding; by forcing a video filter + H.264
//! re-encode, that decoded orientation is baked into the actual frames. The
//! output also clears the rotate metadata so players do not double-rotate it.
//! Audio is copied when the output container can keep it; otherwise it is
//! re-encoded to AAC, matching the video tool family.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;

/// Fixed filter used to force a video filtering stage while preserving normal
/// display compatibility. With ffmpeg's default autorotate, decode applies the
/// input display matrix before this filter, so the re-encoded pixels are upright.
pub const BAKE_FILTER: &str = "format=yuv420p";

/// Build the ffmpeg argv (no leading `ffmpeg`) to bake any embedded rotation
/// metadata/display matrix into pixels and clear the rotate tag in `out_name`.
pub fn build_argv(in_name: &str, out_name: &str, transcode_audio: bool) -> Vec<String> {
    vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vf".to_string(),
        BAKE_FILTER.to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "medium".to_string(),
        "-crf".to_string(),
        "23".to_string(),
        "-metadata:s:v:0".to_string(),
        "rotate=0".to_string(),
        "-c:a".to_string(),
        if transcode_audio { "aac" } else { "copy" }.to_string(),
        out_name.to_string(),
    ]
}

/// Build `(argv, out_name)` for the input filename. H.264-capable containers are
/// kept; webm/other containers fall back to mp4 with AAC audio, via the same
/// helper used by `video-rotate`.
pub fn plan(in_name: &str) -> Result<(Vec<String>, String), String> {
    let (ext, transcode_audio) = h264_out_ext(in_name);
    let out_name = format!("out.{ext}");
    Ok((build_argv(in_name, &out_name, transcode_audio), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bakes_by_forcing_video_filter_and_h264_encode() {
        let (argv, out) = plan("clip.mp4").unwrap();
        assert_eq!(out, "out.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-vf" && w[1] == BAKE_FILTER));
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(argv.windows(2).any(|w| w[0] == "-preset" && w[1] == "medium"));
        assert!(argv.windows(2).any(|w| w[0] == "-crf" && w[1] == "23"));
    }

    #[test]
    fn clears_rotate_metadata() {
        let (argv, _) = plan("clip.mp4").unwrap();
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-metadata:s:v:0" && w[1] == "rotate=0"));
    }

    #[test]
    fn copies_audio_for_h264_capable_containers() {
        for ext in ["mp4", "mov", "m4v", "mkv"] {
            let (argv, out) = plan(&format!("clip.{ext}")).unwrap();
            assert_eq!(out, format!("out.{ext}"));
            assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "copy"), "{ext}");
        }
    }

    #[test]
    fn webm_switches_to_mp4_and_reencodes_audio() {
        let (argv, out) = plan("clip.webm").unwrap();
        assert_eq!(out, "out.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
    }

    #[test]
    fn argv_has_no_stream_copy_video_path() {
        let (argv, _) = plan("clip.mp4").unwrap();
        assert!(!argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
        assert!(!argv.iter().any(|a| a == "-noautorotate"));
    }
}
