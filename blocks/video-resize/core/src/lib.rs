//! gizza-ai/video-resize core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wasm-bindgen deps.
//!
//! Scales a video to a target resolution (re-encodes H.264). Provide width
//! and/or height; the omitted dimension uses `-2` so ffmpeg preserves the aspect
//! ratio AND yields an even number (libx264/yuv420p require even dimensions).
//! The input container is kept (audio stream-copied) when it can hold
//! H.264 + AAC (mp4/mov/m4v/mkv); anything else (webm, …) switches to mp4 and
//! the audio is re-encoded to AAC — see
//! `gizza_ai_block_utils::ffmpeg::h264_out_ext`.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;

/// Build the ffmpeg argv (no leading `ffmpeg`) for the scale. `w`/`h` of `None`
/// means "auto" (preserve aspect for that axis). `transcode_audio` re-encodes
/// the audio to AAC instead of stream-copying — required when the output
/// container differs from the input's (a copied Opus/Vorbis track is invalid
/// or unplayable in mp4).
pub fn build_argv(in_name: &str, out_name: &str, w: Option<u32>, h: Option<u32>, transcode_audio: bool) -> Vec<String> {
    let sw = w.map(|v| v.to_string()).unwrap_or_else(|| "-2".to_string());
    let sh = h.map(|v| v.to_string()).unwrap_or_else(|| "-2".to_string());
    vec![
        "-i".into(), in_name.into(),
        "-vf".into(), format!("scale={sw}:{sh}"),
        "-c:v".into(), "libx264".into(), "-preset".into(), "medium".into(), "-crf".into(), "23".into(),
        "-c:a".into(), if transcode_audio { "aac".into() } else { "copy".into() },
        out_name.into(),
    ]
}

/// Validate dimensions and return `(argv, out_name)`. At least one of w/h is
/// required; both 0 / either 0 is rejected. `out_name` keeps the input
/// container when it can hold H.264 + AAC; otherwise it is `out.mp4`.
pub fn plan(in_name: &str, w: Option<u32>, h: Option<u32>) -> Result<(Vec<String>, String), String> {
    if w.is_none() && h.is_none() {
        return Err("at least one of width/height is required".into());
    }
    if w == Some(0) || h == Some(0) {
        return Err("width/height must be > 0".into());
    }
    let (ext, transcode_audio) = h264_out_ext(in_name);
    let out_name = format!("out.{ext}");
    Ok((build_argv(in_name, &out_name, w, h, transcode_audio), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vf(argv: &[String]) -> String {
        let i = argv.iter().position(|a| a == "-vf").unwrap();
        argv[i + 1].clone()
    }

    fn ca(argv: &[String]) -> String {
        let i = argv.iter().position(|a| a == "-c:a").unwrap();
        argv[i + 1].clone()
    }

    #[test]
    fn both_dims() {
        let argv = build_argv("in.mp4", "out.mp4", Some(640), Some(360), false);
        assert_eq!(vf(&argv), "scale=640:360");
    }

    #[test]
    fn width_only_keeps_aspect_even() {
        let argv = build_argv("in.mp4", "out.mp4", Some(640), None, false);
        assert_eq!(vf(&argv), "scale=640:-2");
    }

    #[test]
    fn height_only_keeps_aspect_even() {
        let argv = build_argv("in.mp4", "out.mp4", None, Some(480), false);
        assert_eq!(vf(&argv), "scale=-2:480");
    }

    #[test]
    fn plan_validates_and_keeps_h264_capable_containers() {
        for ext in ["mp4", "mov", "m4v", "mkv"] {
            let (argv, out) = plan(&format!("in.{ext}"), Some(640), None).unwrap();
            assert_eq!(out, format!("out.{ext}"));
            assert_eq!(ca(&argv), "copy", "{ext}: kept container keeps audio copy");
        }
        assert_eq!(plan("IN.MP4", Some(640), None).unwrap().1, "out.mp4");
        assert!(plan("i.mp4", None, None).is_err());
        assert!(plan("i.mp4", Some(0), Some(10)).is_err());
    }

    #[test]
    fn webm_input_switches_to_mp4_and_reencodes_audio() {
        // WebM can't hold H.264, and its Opus/Vorbis audio can't be copied
        // into mp4 — so: out.mp4 + AAC re-encode.
        let (argv, out) = plan("clip.webm", Some(1280), None).unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
        assert!(argv.iter().any(|a| a == "scale=1280:-2"));
        assert_eq!(ca(&argv), "aac");
        // Extension-less input: container unknown → same safe fallback.
        let (argv, out) = plan("noext", Some(64), None).unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(ca(&argv), "aac");
    }
}
