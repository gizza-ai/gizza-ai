//! gizza-ai/video-audio-denoise core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Reduces background hiss/hum/noise in a video's audio track with ffmpeg's
//! `afftdn` (FFT-based) or `anlmdn` (non-local means) denoiser. The picture is
//! stream-copied (`-c:v copy`, lossless, fast); only the audio is re-encoded
//! (required — the denoiser rewrites samples). An optional `highpass=f=80`
//! stage cuts low-frequency hum/rumble. The output keeps the input container;
//! the audio codec is chosen to match it (webm → libopus, everything else →
//! aac).

use gizza_ai_block_utils::ffmpeg::copy_out_ext;

/// Denoiser algorithm.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Method {
    /// FFT-based denoiser (`afftdn`) — fast, general-purpose, adapts a noise
    /// floor. Good default for steady hiss/hum.
    Afftdn,
    /// Non-local means denoiser (`anlmdn`) — slower, can preserve transients
    /// better on broadband noise.
    Anlmdn,
}

/// Parse the user-facing method string. Empty defaults to afftdn.
pub fn parse_method(s: &str) -> Result<Method, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "afftdn" => Ok(Method::Afftdn),
        "anlmdn" => Ok(Method::Anlmdn),
        other => Err(format!("method {other:?} not supported (afftdn|anlmdn)")),
    }
}

/// Accepted strength range (percent-style intensity).
pub const MIN_STRENGTH: f64 = 1.0;
pub const MAX_STRENGTH: f64 = 100.0;

/// Audio encoder for the kept output container. WebM can only hold Opus/Vorbis,
/// so AAC is invalid there; every other container we keep (mp4/mov/m4v/mkv)
/// accepts AAC.
pub fn audio_codec(out_ext: &str) -> &'static str {
    if out_ext.eq_ignore_ascii_case("webm") {
        "libopus"
    } else {
        "aac"
    }
}

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`12` not `12.0`,
/// `11.64` stays `11.64`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.5}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Map the 1–100 intensity to the method's native denoise parameter and build
/// the ffmpeg `-af` chain. For `afftdn`, strength is scaled to `nr` (noise
/// reduction in dB, 0.97–97 — the filter's useful range). For `anlmdn`,
/// strength maps to `s` (0.001–0.1) — the filter's tiny native default barely
/// denoises, so the slider lands in its useful band. When `remove_hum` is on, a
/// `highpass=f=80` stage is prepended to cut low-frequency hum/rumble.
pub fn build_filter(strength: f64, method: Method, remove_hum: bool) -> String {
    let stage = match method {
        Method::Afftdn => format!("afftdn=nr={}", fmt_num(strength * 0.97)),
        Method::Anlmdn => format!("anlmdn=s={}", fmt_num(strength / 1000.0)),
    };
    if remove_hum {
        format!("highpass=f=80,{stage}")
    } else {
        stage
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to denoise `in_name`'s audio into
/// `out_name`, keeping the picture (`-c:v copy`). Shared verbatim by the web
/// page (`build_argv`) and the chat block.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    strength: f64,
    method: Method,
    remove_hum: bool,
) -> Vec<String> {
    let out_ext = out_name.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    vec![
        "-i".to_string(),
        in_name.to_string(),
        "-c:v".to_string(),
        "copy".to_string(),
        "-af".to_string(),
        build_filter(strength, method, remove_hum),
        "-c:a".to_string(),
        audio_codec(out_ext).to_string(),
        out_name.to_string(),
    ]
}

/// Validate `strength`, parse `method`, and return `(argv, out_name)`.
/// `out_name` keeps the input container when it can hold a copied video stream;
/// otherwise it is `out.mp4`. Single source shared by the chat block
/// (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan(
    in_name: &str,
    strength: f64,
    method: &str,
    remove_hum: bool,
) -> Result<(Vec<String>, String), String> {
    let m = parse_method(method)?;
    if !strength.is_finite() || strength < MIN_STRENGTH || strength > MAX_STRENGTH {
        return Err(format!(
            "strength must be between {MIN_STRENGTH} and {MAX_STRENGTH} (higher = more aggressive), got {strength}"
        ));
    }
    let out_name = format!("out.{}", copy_out_ext(in_name));
    Ok((build_argv(in_name, &out_name, strength, m, remove_hum), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn afftdn_argv_order_and_values() {
        let (argv, out) = plan("in.mp4", 12.0, "afftdn", false).unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i", "in.mp4", "-c:v", "copy", "-af", "afftdn=nr=11.64", "-c:a", "aac", "out.mp4",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn anlmdn_maps_strength_to_s() {
        assert_eq!(build_filter(12.0, Method::Anlmdn, false), "anlmdn=s=0.012");
        assert_eq!(build_filter(100.0, Method::Anlmdn, false), "anlmdn=s=0.1");
        assert_eq!(build_filter(1.0, Method::Anlmdn, false), "anlmdn=s=0.001");
    }

    #[test]
    fn afftdn_strength_scales_to_nr_db() {
        assert_eq!(build_filter(100.0, Method::Afftdn, false), "afftdn=nr=97");
        assert_eq!(build_filter(50.0, Method::Afftdn, false), "afftdn=nr=48.5");
    }

    #[test]
    fn remove_hum_prepends_highpass() {
        assert_eq!(
            build_filter(12.0, Method::Afftdn, true),
            "highpass=f=80,afftdn=nr=11.64"
        );
        assert_eq!(
            build_filter(12.0, Method::Anlmdn, true),
            "highpass=f=80,anlmdn=s=0.012"
        );
    }

    #[test]
    fn always_stream_copies_the_video() {
        let (argv, _) = plan("in.mp4", 30.0, "afftdn", true).unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
        // no -vn: the video track is kept.
        assert!(!argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn webm_input_keeps_webm_and_uses_opus_audio() {
        let (argv, out) = plan("clip.webm", 20.0, "afftdn", false).unwrap();
        assert_eq!(out, "out.webm");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libopus"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
    }

    #[test]
    fn container_kept_for_copy_capable_and_falls_back_to_mp4() {
        for ext in ["mp4", "mov", "m4v", "mkv", "webm"] {
            let (_, out) = plan(&format!("clip.{ext}"), 12.0, "afftdn", false).unwrap();
            assert_eq!(out, format!("out.{ext}"));
        }
        assert_eq!(plan("clip.avi", 12.0, "afftdn", false).unwrap().1, "out.mp4");
        assert_eq!(plan("noext", 12.0, "afftdn", false).unwrap().1, "out.mp4");
    }

    #[test]
    fn rejects_out_of_range_strength() {
        assert!(plan("a.mp4", 0.0, "afftdn", false).is_err());
        assert!(plan("a.mp4", 0.5, "afftdn", false).is_err());
        assert!(plan("a.mp4", 101.0, "afftdn", false).is_err());
        assert!(plan("a.mp4", f64::NAN, "afftdn", false).is_err());
        let err = plan("a.mp4", 200.0, "afftdn", false).unwrap_err();
        assert!(err.contains("strength must be between"));
    }

    #[test]
    fn rejects_unknown_method() {
        assert!(plan("a.mp4", 12.0, "rnnoise", false).is_err());
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(97.0), "97");
        assert_eq!(fmt_num(11.64), "11.64");
        assert_eq!(fmt_num(0.012), "0.012");
        assert_eq!(fmt_num(0.001), "0.001");
    }
}
