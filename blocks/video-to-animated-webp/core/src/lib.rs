//! gizza-ai/video-to-animated-webp core — pure ffmpeg argv construction shared
//! by the chat block and standalone ffmpeg page. No wafer/wasm-bindgen deps.
//!
//! Encodes a short video section to animated WebP via libwebp. WebP is a modern
//! GIF alternative: smaller files at the same visual quality, alpha preserved by
//! the codec, and browser playback through ordinary `<img>`/image surfaces.

/// Output is always animated WebP.
pub const OUT_NAME: &str = "out.webp";
/// Default fps when caller leaves `fps` unset (0).
pub const DEFAULT_FPS: f64 = 12.0;
/// Default lossy quality for libwebp.
pub const DEFAULT_QUALITY: f64 = 80.0;

fn trim_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Build the video filter chain: optional `fps`, optional width-preserving scale.
pub fn build_filter(fps: f64, width: u32) -> String {
    let fps = if fps > 0.0 { fps } else { DEFAULT_FPS };
    let mut parts = vec![format!("fps={}", trim_num(fps))];
    if width > 0 {
        parts.push(format!("scale={width}:-2:flags=lanczos"));
    }
    parts.join(",")
}

/// Build ffmpeg argv (without leading `ffmpeg`).
pub fn build_argv(
    in_name: &str,
    start: f64,
    duration: f64,
    fps: f64,
    width: u32,
    quality: f64,
    lossless: bool,
) -> Vec<String> {
    let mut argv = Vec::new();
    if start > 0.0 {
        argv.push("-ss".to_string());
        argv.push(trim_num(start));
    }
    argv.push("-i".to_string());
    argv.push(in_name.to_string());
    if duration > 0.0 {
        argv.push("-t".to_string());
        argv.push(trim_num(duration));
    }
    argv.push("-an".to_string());
    argv.push("-vf".to_string());
    argv.push(build_filter(fps, width));
    argv.push("-c:v".to_string());
    argv.push("libwebp".to_string());
    argv.push("-loop".to_string());
    argv.push("0".to_string());
    argv.push("-preset".to_string());
    argv.push("default".to_string());
    argv.push("-compression_level".to_string());
    argv.push("6".to_string());
    argv.push("-lossless".to_string());
    argv.push(if lossless { "1" } else { "0" }.to_string());
    if !lossless {
        argv.push("-quality".to_string());
        argv.push(trim_num(if quality > 0.0 { quality } else { DEFAULT_QUALITY }));
    }
    argv.push("-vsync".to_string());
    argv.push("0".to_string());
    argv.push(OUT_NAME.to_string());
    argv
}

/// Validate params and return `(argv, out_name)`.
pub fn plan(
    in_name: &str,
    start: f64,
    duration: f64,
    fps: f64,
    width: f64,
    quality: f64,
    lossless: bool,
) -> Result<(Vec<String>, String), String> {
    if !start.is_finite() || start < 0.0 {
        return Err(format!("start must be >= 0 and finite, got {start}"));
    }
    if !duration.is_finite() || duration < 0.0 {
        return Err(format!("duration must be >= 0 and finite, got {duration}"));
    }
    if !fps.is_finite() || fps < 0.0 {
        return Err(format!("fps must be >= 0 and finite, got {fps}"));
    }
    if fps > 60.0 {
        return Err(format!("fps must be <= 60, got {fps}"));
    }
    if !width.is_finite() || width < 0.0 {
        return Err(format!("width must be >= 0 and finite, got {width}"));
    }
    if width > 4096.0 {
        return Err(format!("width must be <= 4096, got {width}"));
    }
    if !quality.is_finite() || quality < 0.0 {
        return Err(format!("quality must be >= 0 and finite, got {quality}"));
    }
    if quality > 100.0 {
        return Err(format!("quality must be <= 100, got {quality}"));
    }
    Ok((
        build_argv(
            in_name,
            start,
            duration,
            fps,
            width.round() as u32,
            quality,
            lossless,
        ),
        OUT_NAME.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_filter_uses_12fps_without_scale() {
        let f = build_filter(0.0, 0);
        assert_eq!(f, "fps=12");
    }

    #[test]
    fn filter_includes_fps_and_even_height_scale() {
        let f = build_filter(15.0, 320);
        assert_eq!(f, "fps=15,scale=320:-2:flags=lanczos");
    }

    #[test]
    fn argv_encodes_looping_lossy_webp() {
        let argv = build_argv("in.mp4", 1.5, 2.0, 10.0, 240, 75.0, false);
        assert!(argv.windows(2).any(|w| w[0] == "-ss" && w[1] == "1.5"));
        assert!(argv.windows(2).any(|w| w[0] == "-t" && w[1] == "2"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libwebp"));
        assert!(argv.windows(2).any(|w| w[0] == "-loop" && w[1] == "0"));
        assert!(argv.windows(2).any(|w| w[0] == "-quality" && w[1] == "75"));
        assert!(argv.windows(2).any(|w| w[0] == "-lossless" && w[1] == "0"));
        assert_eq!(argv.last().map(String::as_str), Some("out.webp"));
    }

    #[test]
    fn lossless_omits_quality() {
        let argv = build_argv("in.mov", 0.0, 0.0, 0.0, 0, 80.0, true);
        assert!(argv.windows(2).any(|w| w[0] == "-lossless" && w[1] == "1"));
        assert!(!argv.iter().any(|a| a == "-quality"));
    }

    #[test]
    fn plan_validates_bounds() {
        assert!(plan("in.mp4", -1.0, 0.0, 12.0, 0.0, 80.0, false).is_err());
        assert!(plan("in.mp4", 0.0, -1.0, 12.0, 0.0, 80.0, false).is_err());
        assert!(plan("in.mp4", 0.0, 0.0, 61.0, 0.0, 80.0, false).is_err());
        assert!(plan("in.mp4", 0.0, 0.0, 12.0, 4097.0, 80.0, false).is_err());
        assert!(plan("in.mp4", 0.0, 0.0, 12.0, 0.0, 101.0, false).is_err());
    }

    #[test]
    fn defaults_are_valid() {
        let (argv, out) = plan("clip.webm", 0.0, 0.0, 0.0, 0.0, 0.0, false).unwrap();
        assert_eq!(out, OUT_NAME);
        assert!(argv.iter().any(|a| a == "fps=12"));
        assert!(!argv.iter().any(|a| a == "-ss"));
        assert!(!argv.iter().any(|a| a == "-t"));
    }
}
