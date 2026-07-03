//! gizza-ai/video-rotate core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wasm-bindgen deps.
//!
//! Rotates a video by 90/180/270 degrees clockwise and/or flips it horizontally
//! or vertically, via ffmpeg `transpose`/`hflip`/`vflip` filters (re-encodes
//! H.264). The input container is kept (audio stream-copied) when it can hold
//! H.264 + AAC (mp4/mov/m4v/mkv); anything else (webm, …) switches to mp4 and
//! the audio is re-encoded to AAC — see
//! `gizza_ai_block_utils::ffmpeg::h264_out_ext`.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;

/// Build the `-vf` filter chain for a clockwise `rotate` (0/90/180/270) and a
/// `flip` ("none"/"horizontal"/"vertical"). Returns None if both are no-ops.
pub fn vf_chain(rotate: u32, flip: &str) -> Result<Option<String>, String> {
    let mut parts: Vec<&str> = Vec::new();
    match rotate {
        0 => {}
        90 => parts.push("transpose=1"),            // 90° clockwise
        270 => parts.push("transpose=2"),           // 90° counter-clockwise
        180 => { parts.push("transpose=1"); parts.push("transpose=1"); }
        other => return Err(format!("rotate must be 0, 90, 180, or 270 (got {other})")),
    }
    match flip.trim().to_ascii_lowercase().as_str() {
        "" | "none" => {}
        "horizontal" | "h" | "hflip" => parts.push("hflip"),
        "vertical" | "v" | "vflip" => parts.push("vflip"),
        other => return Err(format!("flip must be none, horizontal, or vertical (got '{other}')")),
    }
    if parts.is_empty() {
        return Ok(None);
    }
    Ok(Some(parts.join(",")))
}

/// Validate + build `(argv, out_name)`. Errors if both rotate and flip are
/// no-ops. `out_name` keeps the input container when it can hold H.264 + AAC
/// (audio stream-copied); otherwise it is `out.mp4` and the audio is
/// re-encoded to AAC.
pub fn plan(in_name: &str, rotate: u32, flip: &str) -> Result<(Vec<String>, String), String> {
    let vf = vf_chain(rotate, flip)?
        .ok_or_else(|| "nothing to do: set rotate (90/180/270) and/or flip".to_string())?;
    let (ext, transcode_audio) = h264_out_ext(in_name);
    let out_name = format!("out.{ext}");
    let argv = vec![
        "-i".into(), in_name.into(),
        "-vf".into(), vf,
        "-c:v".into(), "libx264".into(), "-preset".into(), "medium".into(), "-crf".into(), "23".into(),
        "-c:a".into(), if transcode_audio { "aac".into() } else { "copy".into() },
        out_name.clone(),
    ];
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_90_is_transpose_1() {
        assert_eq!(vf_chain(90, "none").unwrap().unwrap(), "transpose=1");
    }
    #[test]
    fn rotate_270_is_transpose_2() {
        assert_eq!(vf_chain(270, "none").unwrap().unwrap(), "transpose=2");
    }
    #[test]
    fn rotate_180_is_double_transpose() {
        assert_eq!(vf_chain(180, "none").unwrap().unwrap(), "transpose=1,transpose=1");
    }
    #[test]
    fn flip_horizontal_and_vertical() {
        assert_eq!(vf_chain(0, "horizontal").unwrap().unwrap(), "hflip");
        assert_eq!(vf_chain(0, "vertical").unwrap().unwrap(), "vflip");
    }
    #[test]
    fn rotate_and_flip_combine() {
        assert_eq!(vf_chain(90, "vertical").unwrap().unwrap(), "transpose=1,vflip");
    }
    #[test]
    fn noop_is_none_and_plan_errors() {
        assert!(vf_chain(0, "none").unwrap().is_none());
        assert!(plan("in.mp4", 0, "none").is_err());
    }
    #[test]
    fn invalid_values_error() {
        assert!(vf_chain(45, "none").is_err());
        assert!(vf_chain(90, "sideways").is_err());
    }
    #[test]
    fn plan_keeps_h264_capable_containers_and_copies_audio() {
        for ext in ["mp4", "mov", "m4v", "mkv"] {
            let (argv, out) = plan(&format!("clip.{ext}"), 90, "none").unwrap();
            assert_eq!(out, format!("out.{ext}"));
            assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "copy"), "{ext}");
        }
        assert_eq!(plan("CLIP.M4V", 90, "none").unwrap().1, "out.m4v");
    }

    #[test]
    fn webm_input_switches_to_mp4_and_reencodes_audio() {
        // WebM can't hold H.264, and its Opus/Vorbis audio can't be copied
        // into mp4 — so: out.mp4 + AAC re-encode.
        let (argv, out) = plan("clip.webm", 90, "none").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
        assert!(argv.iter().any(|a| a == "transpose=1"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
    }
}
