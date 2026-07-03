//! gizza-ai/video-crop core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wasm-bindgen deps.
//!
//! Crops a video to a `width`x`height` rectangle. With an explicit `x`/`y`
//! offset (top-left origin) it crops that region; without one it crops from the
//! center (ffmpeg's default `crop=w:h`). Re-encodes via libx264 + AAC; the
//! input container is kept when it can hold H.264 + AAC (mp4/mov/m4v/mkv),
//! anything else (webm, …) switches to mp4 — see
//! `gizza_ai_block_utils::ffmpeg::h264_out_ext`.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;

/// Build the ffmpeg argv (no leading `ffmpeg`) for a crop. `x`/`y` of `None`
/// means "center" (ffmpeg derives the centered offset from `crop=w:h`).
pub fn build_argv(in_name: &str, out_name: &str, w: u32, h: u32, x: Option<u32>, y: Option<u32>) -> Vec<String> {
    let crop = match (x, y) {
        (None, None) => format!("crop={w}:{h}"),
        (x, y) => format!("crop={w}:{h}:{}:{}", x.unwrap_or(0), y.unwrap_or(0)),
    };
    vec![
        "-i".into(),
        in_name.into(),
        "-vf".into(),
        crop,
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "medium".into(),
        "-c:a".into(),
        "aac".into(),
        out_name.into(),
    ]
}

/// Validate crop dimensions and return `(argv, out_name)`. `out_name` keeps
/// the input container when it can hold H.264 + AAC; otherwise it is `out.mp4`.
pub fn plan(in_name: &str, w: u32, h: u32, x: Option<u32>, y: Option<u32>) -> Result<(Vec<String>, String), String> {
    if w == 0 || h == 0 {
        return Err("width and height must be > 0".into());
    }
    let out_name = format!("out.{}", h264_out_ext(in_name).0);
    Ok((build_argv(in_name, &out_name, w, h, x, y), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_centered_when_no_offset() {
        let argv = build_argv("in.mp4", "out.mp4", 320, 240, None, None);
        let vf = argv.iter().position(|a| a == "-vf").unwrap();
        assert_eq!(argv[vf + 1], "crop=320:240");
    }

    #[test]
    fn argv_uses_offset_when_given() {
        let argv = build_argv("in.mp4", "out.mp4", 320, 240, Some(10), Some(20));
        let vf = argv.iter().position(|a| a == "-vf").unwrap();
        assert_eq!(argv[vf + 1], "crop=320:240:10:20");
    }

    #[test]
    fn argv_partial_offset_defaults_missing_to_zero() {
        let argv = build_argv("in.mp4", "out.mp4", 100, 100, Some(5), None);
        let vf = argv.iter().position(|a| a == "-vf").unwrap();
        assert_eq!(argv[vf + 1], "crop=100:100:5:0");
    }

    #[test]
    fn plan_keeps_h264_capable_containers_and_validates() {
        for ext in ["mp4", "mov", "m4v", "mkv"] {
            let (_, out) = plan(&format!("clip.{ext}"), 640, 360, None, None).unwrap();
            assert_eq!(out, format!("out.{ext}"));
        }
        let (_, out) = plan("CLIP.MP4", 640, 360, None, None).unwrap();
        assert_eq!(out, "out.mp4");
        assert!(plan("in.mp4", 0, 100, None, None).is_err());
        assert!(plan("in.mp4", 100, 0, None, None).is_err());
    }

    #[test]
    fn webm_input_switches_container_to_mp4() {
        // H.264 can't be muxed into WebM (VP8/VP9/AV1 + Vorbis/Opus only) —
        // the output container must switch to mp4.
        let (argv, out) = plan("clip.webm", 640, 360, None, None).unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
        assert!(argv.iter().any(|a| a == "crop=640:360"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
        assert_eq!(plan("noext", 100, 100, None, None).unwrap().1, "out.mp4");
    }
}
