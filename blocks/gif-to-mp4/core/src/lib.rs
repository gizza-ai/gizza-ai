//! gizza-ai/gif-to-mp4 core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Converts an animated GIF to a much smaller MP4 (H.264) or WebM (VP9). H.264
//! and yuv420p both require even dimensions, so the frames are scaled to even
//! width/height. All GIF frames are encoded, so the animation is preserved
//! (looping itself is a player attribute, not stored in the video stream).

/// Even-dimension scale filter (H.264 / yuv420p reject odd width/height).
const EVEN_SCALE: &str = "scale=trunc(iw/2)*2:trunc(ih/2)*2:flags=lanczos";

/// Parse the output format; defaults to mp4.
pub fn parse_format(s: &str) -> Result<&'static str, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "mp4" => Ok("mp4"),
        "webm" => Ok("webm"),
        other => Err(format!("invalid format '{other}' (use mp4 or webm)")),
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) + out_name for the given format.
pub fn build_argv(in_name: &str, format: &str) -> (Vec<String>, String) {
    let out_name = format!("out.{format}");
    let mut argv = vec!["-i".to_string(), in_name.to_string(), "-vf".to_string(), EVEN_SCALE.to_string()];
    match format {
        "webm" => argv.extend([
            "-c:v".into(), "libvpx-vp9".into(),
            "-b:v".into(), "0".into(), "-crf".into(), "32".into(),
            "-pix_fmt".into(), "yuv420p".into(),
        ]),
        // mp4 / default → H.264
        _ => argv.extend([
            "-c:v".into(), "libx264".into(),
            "-preset".into(), "medium".into(), "-crf".into(), "26".into(),
            "-pix_fmt".into(), "yuv420p".into(),
            "-movflags".into(), "+faststart".into(),
        ]),
    }
    argv.push(out_name.clone());
    (argv, out_name)
}

/// Validate the format and return `(argv, out_name)`.
pub fn plan(in_name: &str, format: &str) -> Result<(Vec<String>, String), String> {
    let fmt = parse_format(format)?;
    Ok(build_argv(in_name, fmt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mp4_uses_h264_yuv420p_faststart() {
        let (argv, out) = build_argv("in.gif", "mp4");
        assert_eq!(out, "out.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(argv.iter().any(|a| a == "+faststart"));
        assert!(argv.iter().any(|a| a == EVEN_SCALE));
    }

    #[test]
    fn webm_uses_vp9() {
        let (argv, out) = build_argv("in.gif", "webm");
        assert_eq!(out, "out.webm");
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libvpx-vp9"));
    }

    #[test]
    fn parse_format_defaults_and_validates() {
        assert_eq!(parse_format("").unwrap(), "mp4");
        assert_eq!(parse_format("MP4").unwrap(), "mp4");
        assert_eq!(parse_format("webm").unwrap(), "webm");
        assert!(parse_format("avi").is_err());
    }

    #[test]
    fn plan_rejects_bad_format() {
        assert!(plan("in.gif", "mkv").is_err());
        assert!(plan("in.gif", "mp4").is_ok());
    }
}
