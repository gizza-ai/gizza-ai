//! gizza-ai/video-denoise core — pure ffmpeg argv construction shared by the
//! chat block and the standalone web page. No wasm-bindgen deps.
//!
//! Reduces visual noise/grain in a video's PICTURE with a temporal-spatial
//! denoise filter. Two methods:
//!
//! * `hqdn3d` (default) — high-quality 3D denoiser: it averages each pixel both
//!   across neighbouring pixels (spatial) AND across time (temporal), so
//!   grain/sensor noise is smoothed while static detail is preserved. Fast,
//!   general-purpose.
//! * `nlmeans` — non-local means: a slower, spatial-only denoiser that finds
//!   similar patches across the frame; it keeps edges/texture crisper on heavy
//!   noise at the cost of speed.
//!
//! Because the picture is rewritten, the video stream is re-encoded to H.264
//! (`-crf 20`, visually near-transparent). Audio is stream-copied when the
//! container is kept (mp4/mov/m4v/mkv) and only re-encoded to AAC when the
//! input container can't hold H.264/AAC (e.g. webm), in which case the output
//! switches to MP4 — see `gizza_ai_block_utils::ffmpeg::h264_out_ext`.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;

/// Denoise algorithm.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Method {
    /// `hqdn3d` — temporal + spatial 3D denoiser. Fast, good default.
    Hqdn3d,
    /// `nlmeans` — non-local means, spatial only. Slower, detail-preserving.
    Nlmeans,
}

/// Parse the user-facing method string. Empty defaults to hqdn3d.
pub fn parse_method(s: &str) -> Result<Method, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "hqdn3d" => Ok(Method::Hqdn3d),
        "nlmeans" => Ok(Method::Nlmeans),
        other => Err(format!("method {other:?} not supported (hqdn3d|nlmeans)")),
    }
}

/// Accepted strength range (percent-style intensity).
pub const MIN_STRENGTH: f64 = 1.0;
pub const MAX_STRENGTH: f64 = 100.0;
/// Default strength when the request is unset (the page's `0` sentinel) or
/// non-finite. 30 maps to a moderate hqdn3d that clears grain without smearing.
pub const DEFAULT_STRENGTH: f64 = 30.0;

/// Resolve a (possibly unset / non-finite) strength request into an accepted
/// value. `strength <= 0` (the page's "unset" sentinel) and any non-finite
/// value fall back to [`DEFAULT_STRENGTH`]; otherwise it is clamped into
/// `MIN_STRENGTH..=MAX_STRENGTH`.
pub fn resolve_strength(strength: f64) -> f64 {
    if !strength.is_finite() || strength <= 0.0 {
        return DEFAULT_STRENGTH;
    }
    strength.clamp(MIN_STRENGTH, MAX_STRENGTH)
}

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`4` not `4.0`,
/// `3.75` stays `3.75`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.5}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Map the 1–100 intensity to the method's native denoise parameters and build
/// the ffmpeg `-vf` filter string.
///
/// For `hqdn3d` the strength scales `luma_spatial` (`ls = strength / 10`, i.e.
/// 0.1–10; ffmpeg's own default is `4:3:6:4.5`, which this reproduces at
/// strength 40). The other three components keep hqdn3d's default proportions
/// relative to `luma_spatial`: `chroma_spatial = 0.75·ls`, `luma_tmp = 1.5·ls`,
/// `chroma_tmp = 1.125·ls`.
///
/// For `nlmeans` the strength maps to the filter's denoise parameter `s` in the
/// 1–15 band (`s = 1 + (strength-1)·14/99`), landing in nlmeans' useful range
/// (its own default is a very light `1.0`).
pub fn build_filter(strength: f64, method: Method) -> String {
    let s = resolve_strength(strength);
    match method {
        Method::Hqdn3d => {
            let ls = s / 10.0;
            let cs = ls * 0.75;
            let lt = ls * 1.5;
            let ct = ls * 1.125;
            format!(
                "hqdn3d={}:{}:{}:{}",
                fmt_num(ls),
                fmt_num(cs),
                fmt_num(lt),
                fmt_num(ct)
            )
        }
        Method::Nlmeans => {
            let param = 1.0 + (s - 1.0) * 14.0 / 99.0;
            format!("nlmeans=s={}", fmt_num(param))
        }
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) and the output filename for a
/// visual denoise. Shared verbatim by the web page (`build_argv`) and the chat
/// block.
///
/// The chosen denoise filter rewrites the picture, so the video is re-encoded to
/// H.264 (`libx264`, `-preset medium`, `-crf 20`). The input container is kept
/// when it can hold H.264/AAC, otherwise the output is `out.mp4`; audio is
/// stream-copied when the container is kept and re-encoded to AAC otherwise.
/// `strength` is resolved via [`resolve_strength`].
pub fn build_argv(strength: f64, method: Method, in_name: &str) -> (Vec<String>, String) {
    let filter = build_filter(strength, method);
    let (ext, reencode_audio) = h264_out_ext(in_name);
    let out_name = format!("out.{ext}");
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vf".to_string(),
        filter,
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "medium".to_string(),
        "-crf".to_string(),
        "20".to_string(),
        "-c:a".to_string(),
    ];
    argv.push(if reencode_audio { "aac" } else { "copy" }.to_string());
    argv.push(out_name.clone());
    (argv, out_name)
}

/// Validate the request and return `(argv, out_name)`. A non-finite strength is
/// rejected up front (a clear user error); an unset (`0`) or out-of-range
/// request is resolved/clamped by [`resolve_strength`]. `method` is parsed from
/// the user string (empty → hqdn3d).
pub fn plan(strength: f64, method: &str, in_name: &str) -> Result<(Vec<String>, String), String> {
    if strength.is_nan() || strength.is_infinite() {
        return Err("strength must be a finite number".into());
    }
    let method = parse_method(method)?;
    Ok(build_argv(strength, method, in_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_method_variants() {
        assert_eq!(parse_method("").unwrap(), Method::Hqdn3d);
        assert_eq!(parse_method("hqdn3d").unwrap(), Method::Hqdn3d);
        assert_eq!(parse_method("HQDN3D").unwrap(), Method::Hqdn3d);
        assert_eq!(parse_method(" nlmeans ").unwrap(), Method::Nlmeans);
        assert!(parse_method("bm3d").is_err());
    }

    #[test]
    fn hqdn3d_default_strength_reproduces_ffmpeg_default_at_40() {
        // strength 40 → ls=4 → 4:3:6:4.5 (ffmpeg's own hqdn3d default).
        assert_eq!(build_filter(40.0, Method::Hqdn3d), "hqdn3d=4:3:6:4.5");
    }

    #[test]
    fn hqdn3d_scales_with_strength() {
        assert_eq!(build_filter(10.0, Method::Hqdn3d), "hqdn3d=1:0.75:1.5:1.125");
        assert_eq!(build_filter(100.0, Method::Hqdn3d), "hqdn3d=10:7.5:15:11.25");
    }

    #[test]
    fn nlmeans_maps_strength_to_s() {
        assert_eq!(build_filter(1.0, Method::Nlmeans), "nlmeans=s=1");
        assert_eq!(build_filter(100.0, Method::Nlmeans), "nlmeans=s=15");
    }

    #[test]
    fn full_default_argv() {
        let (argv, out) = build_argv(30.0, Method::Hqdn3d, "in.mp4");
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i", "in.mp4", "-vf", "hqdn3d=3:2.25:4.5:3.375", "-c:v", "libx264", "-preset",
                "medium", "-crf", "20", "-c:a", "copy", "out.mp4",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn unset_and_non_finite_fall_back_to_default() {
        assert_eq!(resolve_strength(0.0), DEFAULT_STRENGTH);
        assert_eq!(resolve_strength(-5.0), DEFAULT_STRENGTH);
        assert_eq!(resolve_strength(f64::NAN), DEFAULT_STRENGTH);
        assert_eq!(resolve_strength(f64::INFINITY), DEFAULT_STRENGTH);
    }

    #[test]
    fn strength_clamped() {
        assert_eq!(resolve_strength(0.5), MIN_STRENGTH);
        assert_eq!(resolve_strength(1000.0), MAX_STRENGTH);
        assert_eq!(resolve_strength(1.0), 1.0);
        assert_eq!(resolve_strength(100.0), 100.0);
    }

    #[test]
    fn keeps_h264_capable_containers() {
        for ext in ["mp4", "mov", "m4v", "mkv"] {
            let (argv, out) = build_argv(30.0, Method::Hqdn3d, &format!("clip.{ext}"));
            assert_eq!(out, format!("out.{ext}"));
            assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "copy"));
        }
    }

    #[test]
    fn webm_switches_to_mp4_and_reencodes_audio() {
        let (argv, out) = build_argv(30.0, Method::Hqdn3d, "clip.webm");
        assert_eq!(out, "out.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
    }

    #[test]
    fn argv_reencodes_video_to_h264() {
        // strength 30 → nlmeans s = 1 + 29·14/99 = 5.10101…
        let (argv, _) = build_argv(30.0, Method::Nlmeans, "in.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(argv.windows(2).any(|w| w[0] == "-crf" && w[1] == "20"));
        let i = argv.iter().position(|a| a == "-vf").unwrap();
        assert_eq!(argv[i + 1], "nlmeans=s=5.10101");
        assert_eq!(argv.first().map(String::as_str), Some("-i"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn plan_validates() {
        assert!(plan(f64::NAN, "hqdn3d", "in.mp4").is_err());
        assert!(plan(f64::INFINITY, "hqdn3d", "in.mp4").is_err());
        assert!(plan(30.0, "bm3d", "in.mp4").is_err());
        assert!(plan(30.0, "hqdn3d", "in.mp4").is_ok());
        // Unset sentinel resolves to the default.
        assert!(plan(0.0, "", "in.mp4").is_ok());
    }
}
