//! gizza-ai/video-to-gif core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Converts a section of a video into an optimized animated GIF. Quality comes
//! from a single-pass `filter_complex` that generates a per-clip palette
//! (`palettegen`) and applies it (`paletteuse`) — far better than the default
//! fixed 256-colour quantization. The clip can be windowed with `start` /
//! `duration` (input seek via `-ss`/`-t`), down-sampled in time with `fps`, and
//! down-scaled with `width` (height auto, preserving aspect ratio, forced even).

/// Output is always a GIF.
pub const OUT_NAME: &str = "out.gif";

/// Default GIF frame rate when the caller leaves `fps` unset (0). A GIF at the
/// source's full frame rate is huge; 12 fps is a good size/smoothness balance.
pub const DEFAULT_FPS: f64 = 12.0;

/// Build the `filter_complex` graph string for the given fps / width.
///
/// `fps=<n>` thins frames; `scale=<w>:-2:flags=lanczos` resizes preserving
/// aspect ratio (`-2` keeps height even, which some decoders prefer) — when
/// `width` is 0 the scale stage is omitted so the source size is kept. The graph
/// splits the stream, generates a palette from one branch and applies it to the
/// other (`paletteuse` with `dither=bayer` for smooth gradients at small sizes).
pub fn build_filter(fps: f64, width: u32) -> String {
    let fps = if fps > 0.0 { fps } else { DEFAULT_FPS };
    let mut pre = format!("fps={}", trim_num(fps));
    if width > 0 {
        pre.push_str(&format!(",scale={width}:-2:flags=lanczos"));
    }
    // split → [palettegen] and → [paletteuse]; single decode pass.
    format!(
        "[0:v]{pre},split[s0][s1];\
         [s0]palettegen=stats_mode=diff[p];\
         [s1][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle"
    )
}

/// Format an f64 without a trailing `.0` for whole numbers (so `12.0` → `12`,
/// keeping the ffmpeg filter string and unit tests tidy).
fn trim_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) + out_name.
///
/// `-ss <start>` (input seek, fast) → `-i <in>` → `-t <duration>` (only when
/// > 0) → `-filter_complex <graph>` → `-loop 0` (loop forever) → `<out.gif>`.
pub fn build_argv(in_name: &str, start: f64, duration: f64, fps: f64, width: u32) -> Vec<String> {
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
    argv.push("-filter_complex".to_string());
    argv.push(build_filter(fps, width));
    // Animated GIF that loops forever.
    argv.push("-loop".to_string());
    argv.push("0".to_string());
    argv.push(OUT_NAME.to_string());
    argv
}

/// Validate params and return `(argv, out_name)`. `start >= 0`, `duration >= 0`
/// (0 = to the end of the clip), `fps >= 0` (0 = default), `width >= 0`
/// (0 = keep source size). Shared by the chat block and the page.
pub fn plan(
    in_name: &str,
    start: f64,
    duration: f64,
    fps: f64,
    width: f64,
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
    let w = width.round() as u32;
    Ok((build_argv(in_name, start, duration, fps, w), OUT_NAME.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_fps_when_zero() {
        let f = build_filter(0.0, 0);
        assert!(f.contains("fps=12"), "got {f}");
    }

    #[test]
    fn filter_includes_palettegen_and_paletteuse() {
        let f = build_filter(15.0, 320);
        assert!(f.contains("palettegen"));
        assert!(f.contains("paletteuse"));
        assert!(f.contains("fps=15"));
        assert!(f.contains("scale=320:-2"));
    }

    #[test]
    fn no_scale_when_width_zero() {
        let f = build_filter(10.0, 0);
        // bayer_scale= is part of paletteuse; the resize stage is ",scale=".
        assert!(!f.contains(",scale="), "got {f}");
    }

    #[test]
    fn argv_omits_ss_when_start_zero() {
        let argv = build_argv("in.mp4", 0.0, 3.0, 12.0, 320);
        assert!(!argv.iter().any(|a| a == "-ss"));
        assert!(argv.windows(2).any(|w| w[0] == "-t" && w[1] == "3"));
    }

    #[test]
    fn argv_includes_ss_and_omits_t_when_duration_zero() {
        let argv = build_argv("in.mp4", 2.5, 0.0, 12.0, 0);
        assert!(argv.windows(2).any(|w| w[0] == "-ss" && w[1] == "2.5"));
        assert!(!argv.iter().any(|a| a == "-t"));
    }

    #[test]
    fn argv_loops_forever_and_outputs_gif() {
        let argv = build_argv("in.webm", 0.0, 0.0, 0.0, 0);
        assert!(argv.windows(2).any(|w| w[0] == "-loop" && w[1] == "0"));
        assert_eq!(argv.last().map(String::as_str), Some("out.gif"));
        assert!(argv.iter().any(|a| a == "-filter_complex"));
    }

    #[test]
    fn plan_returns_gif_and_valid_argv() {
        let (argv, out) = plan("clip.mov", 5.0, 4.0, 15.0, 480.0).unwrap();
        assert_eq!(out, "out.gif");
        let i = argv.iter().position(|a| a == "-ss").unwrap();
        assert_eq!(argv[i + 1], "5");
        let i = argv.iter().position(|a| a == "-t").unwrap();
        assert_eq!(argv[i + 1], "4");
    }

    #[test]
    fn plan_rejects_negative_start() {
        assert!(plan("in.mp4", -1.0, 2.0, 12.0, 0.0).is_err());
    }

    #[test]
    fn plan_rejects_negative_duration() {
        assert!(plan("in.mp4", 0.0, -1.0, 12.0, 0.0).is_err());
    }

    #[test]
    fn plan_rejects_bad_fps() {
        assert!(plan("in.mp4", 0.0, 0.0, -1.0, 0.0).is_err());
        assert!(plan("in.mp4", 0.0, 0.0, 120.0, 0.0).is_err());
    }

    #[test]
    fn plan_rejects_bad_width() {
        assert!(plan("in.mp4", 0.0, 0.0, 12.0, -5.0).is_err());
        assert!(plan("in.mp4", 0.0, 0.0, 12.0, 9000.0).is_err());
    }

    #[test]
    fn plan_rejects_non_finite() {
        assert!(plan("in.mp4", f64::NAN, 0.0, 12.0, 0.0).is_err());
        assert!(plan("in.mp4", 0.0, f64::INFINITY, 12.0, 0.0).is_err());
    }

    #[test]
    fn defaults_all_zero_ok() {
        let (argv, out) = plan("in.mp4", 0.0, 0.0, 0.0, 0.0).unwrap();
        assert_eq!(out, "out.gif");
        // no -ss, no -t, default fps inside the filter graph
        assert!(!argv.iter().any(|a| a == "-ss"));
        assert!(!argv.iter().any(|a| a == "-t"));
    }
}
