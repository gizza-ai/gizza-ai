//! gizza-ai/video-eq-adjust core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wasm-bindgen deps.
//!
//! Adjusts a video's brightness, contrast, saturation, and gamma in a single
//! pass with ffmpeg's `eq` filter, then re-encodes to H.264 video + AAC audio.
//! The input container is kept when it can hold H.264 + AAC (mp4/mov/m4v/mkv);
//! anything else (webm, …) switches to mp4 — see
//! `gizza_ai_block_utils::ffmpeg::h264_out_ext`.
//!
//! Identity (no visible change) is `brightness=0, contrast=1, saturation=1,
//! gamma=1`. Ranges follow ffmpeg's `eq` filter:
//! - brightness ∈ [-1, 1] (0 = none)
//! - contrast   ∈ [0, 4]  (1 = none, 0 = flat gray)
//! - saturation ∈ [0, 3]  (1 = none, 0 = grayscale)
//! - gamma      ∈ [0.1, 10] (1 = none, <1 brightens midtones)

use gizza_ai_block_utils::ffmpeg::h264_out_ext;

pub const BRIGHTNESS_MIN: f64 = -1.0;
pub const BRIGHTNESS_MAX: f64 = 1.0;
pub const CONTRAST_MIN: f64 = 0.0;
pub const CONTRAST_MAX: f64 = 4.0;
pub const SATURATION_MIN: f64 = 0.0;
pub const SATURATION_MAX: f64 = 3.0;
pub const GAMMA_MIN: f64 = 0.1;
pub const GAMMA_MAX: f64 = 10.0;

/// Format an `eq` value as ffmpeg expects it: shortest round-trip decimal, and
/// never a signed zero (`-0` → `0`).
fn fmt(v: f64) -> String {
    let v = if v == 0.0 { 0.0 } else { v }; // collapse -0.0 → 0.0
    v.to_string()
}

fn check(name: &str, v: f64, min: f64, max: f64) -> Result<(), String> {
    if !v.is_finite() {
        return Err(format!("{name} must be a finite number, got {v}"));
    }
    if v < min || v > max {
        return Err(format!("{name} must be between {min} and {max}, got {v}"));
    }
    Ok(())
}

/// Build the ffmpeg argv (no leading `ffmpeg`) for an eq adjustment.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    brightness: f64,
    contrast: f64,
    saturation: f64,
    gamma: f64,
) -> Vec<String> {
    let eq = format!(
        "eq=brightness={}:contrast={}:saturation={}:gamma={}",
        fmt(brightness),
        fmt(contrast),
        fmt(saturation),
        fmt(gamma),
    );
    vec![
        "-i".into(),
        in_name.into(),
        "-vf".into(),
        eq,
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "medium".into(),
        "-c:a".into(),
        "aac".into(),
        out_name.into(),
    ]
}

/// Validate the four eq parameters and return `(argv, out_name)`. `out_name`
/// keeps the input container when it can hold H.264 + AAC; otherwise `out.mp4`.
pub fn plan(
    in_name: &str,
    brightness: f64,
    contrast: f64,
    saturation: f64,
    gamma: f64,
) -> Result<(Vec<String>, String), String> {
    check("brightness", brightness, BRIGHTNESS_MIN, BRIGHTNESS_MAX)?;
    check("contrast", contrast, CONTRAST_MIN, CONTRAST_MAX)?;
    check("saturation", saturation, SATURATION_MIN, SATURATION_MAX)?;
    check("gamma", gamma, GAMMA_MIN, GAMMA_MAX)?;
    let out_name = format!("out.{}", h264_out_ext(in_name).0);
    Ok((
        build_argv(in_name, &out_name, brightness, contrast, saturation, gamma),
        out_name,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vf(argv: &[String]) -> String {
        let i = argv.iter().position(|a| a == "-vf").unwrap();
        argv[i + 1].clone()
    }

    #[test]
    fn identity_builds_eq_with_all_terms() {
        let argv = build_argv("in.mp4", "out.mp4", 0.0, 1.0, 1.0, 1.0);
        assert_eq!(vf(&argv), "eq=brightness=0:contrast=1:saturation=1:gamma=1");
        // re-encodes to H.264 + AAC
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
    }

    #[test]
    fn worked_example_from_competitor_analysis() {
        let argv = build_argv("in.mp4", "out.mp4", 0.1, 1.2, 1.4, 0.9);
        assert_eq!(
            vf(&argv),
            "eq=brightness=0.1:contrast=1.2:saturation=1.4:gamma=0.9"
        );
    }

    #[test]
    fn negative_brightness_and_signed_zero() {
        let argv = build_argv("in.mp4", "out.mp4", -0.3, 1.0, 1.0, 1.0);
        assert_eq!(vf(&argv), "eq=brightness=-0.3:contrast=1:saturation=1:gamma=1");
        // -0.0 collapses to 0
        let argv = build_argv("in.mp4", "out.mp4", -0.0, 1.0, 1.0, 1.0);
        assert!(vf(&argv).contains("brightness=0:"));
    }

    #[test]
    fn plan_keeps_h264_capable_containers_and_validates() {
        for ext in ["mp4", "mov", "m4v", "mkv"] {
            let (_, out) = plan(&format!("clip.{ext}"), 0.0, 1.0, 1.0, 1.0).unwrap();
            assert_eq!(out, format!("out.{ext}"));
        }
        // Uppercase + no extension both normalize to mp4.
        assert_eq!(plan("CLIP.MP4", 0.0, 1.0, 1.0, 1.0).unwrap().1, "out.mp4");
        assert_eq!(plan("noext", 0.0, 1.0, 1.0, 1.0).unwrap().1, "out.mp4");
    }

    #[test]
    fn webm_input_switches_container_to_mp4() {
        // H.264/AAC can't be muxed into WebM — output must switch to mp4.
        let (argv, out) = plan("clip.webm", 0.0, 1.2, 1.0, 1.0).unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn boundary_values_are_accepted() {
        assert!(plan("in.mp4", -1.0, 0.0, 0.0, 0.1).is_ok());
        assert!(plan("in.mp4", 1.0, 4.0, 3.0, 10.0).is_ok());
    }

    #[test]
    fn out_of_range_values_are_rejected() {
        assert!(plan("in.mp4", 1.5, 1.0, 1.0, 1.0).is_err()); // brightness > 1
        assert!(plan("in.mp4", -1.1, 1.0, 1.0, 1.0).is_err()); // brightness < -1
        assert!(plan("in.mp4", 0.0, 5.0, 1.0, 1.0).is_err()); // contrast > 4
        assert!(plan("in.mp4", 0.0, 1.0, 3.5, 1.0).is_err()); // saturation > 3
        assert!(plan("in.mp4", 0.0, 1.0, 1.0, 0.0).is_err()); // gamma < 0.1
        assert!(plan("in.mp4", 0.0, 1.0, 1.0, 11.0).is_err()); // gamma > 10
    }

    #[test]
    fn non_finite_is_rejected() {
        assert!(plan("in.mp4", f64::NAN, 1.0, 1.0, 1.0).is_err());
        assert!(plan("in.mp4", 0.0, f64::INFINITY, 1.0, 1.0).is_err());
    }
}
