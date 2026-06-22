//! gizza-ai/video-resize core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Scales a video to a target resolution (re-encodes H.264 + AAC). Provide width
//! and/or height; the omitted dimension uses `-2` so ffmpeg preserves the aspect
//! ratio AND yields an even number (libx264/yuv420p require even dimensions).

fn out_ext(in_name: &str) -> &str {
    in_name.rsplit_once('.').map(|(_, e)| e).filter(|e| !e.is_empty()).unwrap_or("mp4")
}

/// Build the ffmpeg argv (no leading `ffmpeg`) for the scale. `w`/`h` of `None`
/// means "auto" (preserve aspect for that axis).
pub fn build_argv(in_name: &str, out_name: &str, w: Option<u32>, h: Option<u32>) -> Vec<String> {
    let sw = w.map(|v| v.to_string()).unwrap_or_else(|| "-2".to_string());
    let sh = h.map(|v| v.to_string()).unwrap_or_else(|| "-2".to_string());
    vec![
        "-i".into(), in_name.into(),
        "-vf".into(), format!("scale={sw}:{sh}"),
        "-c:v".into(), "libx264".into(), "-preset".into(), "medium".into(), "-crf".into(), "23".into(),
        "-c:a".into(), "copy".into(),
        out_name.into(),
    ]
}

/// Validate dimensions and return `(argv, out_name)`. At least one of w/h is
/// required; both 0 / either 0 is rejected. `out_name` keeps the input extension.
pub fn plan(in_name: &str, w: Option<u32>, h: Option<u32>) -> Result<(Vec<String>, String), String> {
    if w.is_none() && h.is_none() {
        return Err("at least one of width/height is required".into());
    }
    if w == Some(0) || h == Some(0) {
        return Err("width/height must be > 0".into());
    }
    let out_name = format!("out.{}", out_ext(in_name));
    Ok((build_argv(in_name, &out_name, w, h), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vf(argv: &[String]) -> String {
        let i = argv.iter().position(|a| a == "-vf").unwrap();
        argv[i + 1].clone()
    }

    #[test]
    fn both_dims() {
        let argv = build_argv("in.mp4", "out.mp4", Some(640), Some(360));
        assert_eq!(vf(&argv), "scale=640:360");
    }

    #[test]
    fn width_only_keeps_aspect_even() {
        let argv = build_argv("in.mp4", "out.mp4", Some(640), None);
        assert_eq!(vf(&argv), "scale=640:-2");
    }

    #[test]
    fn height_only_keeps_aspect_even() {
        let argv = build_argv("in.mp4", "out.mp4", None, Some(480));
        assert_eq!(vf(&argv), "scale=-2:480");
    }

    #[test]
    fn plan_validates_and_keeps_extension() {
        let (argv, out) = plan("clip.webm", Some(1280), None).unwrap();
        assert_eq!(out, "out.webm");
        assert!(argv.iter().any(|a| a == "scale=1280:-2"));
        assert!(plan("i.mp4", None, None).is_err());
        assert!(plan("i.mp4", Some(0), Some(10)).is_err());
    }
}
