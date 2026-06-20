//! gizza-ai/video-crop core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Crops a video to a `width`x`height` rectangle. With an explicit `x`/`y`
//! offset (top-left origin) it crops that region; without one it crops from the
//! center (ffmpeg's default `crop=w:h`). Re-encodes via libx264 + AAC, keeping
//! the input container extension.

/// Pick the output container extension, preserving the input's; fall back to mp4.
fn out_ext(in_name: &str) -> &str {
    in_name
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .filter(|ext| !ext.is_empty())
        .unwrap_or("mp4")
}

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

/// Validate crop dimensions and return `(argv, out_name)`. `out_name` keeps the
/// input extension.
pub fn plan(in_name: &str, w: u32, h: u32, x: Option<u32>, y: Option<u32>) -> Result<(Vec<String>, String), String> {
    if w == 0 || h == 0 {
        return Err("width and height must be > 0".into());
    }
    let out_name = format!("out.{}", out_ext(in_name));
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
    fn plan_keeps_extension_and_validates() {
        let (argv, out) = plan("clip.webm", 640, 360, None, None).unwrap();
        assert_eq!(out, "out.webm");
        assert!(argv.iter().any(|a| a == "crop=640:360"));
        assert!(plan("in.mp4", 0, 100, None, None).is_err());
        assert!(plan("in.mp4", 100, 0, None, None).is_err());
    }
}
