//! gizza-ai/audio-noise-reduce core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Reduces steady background hiss/hum from an audio file with ffmpeg's `afftdn`
//! (FFT-based) or `anlmdn` (non-local means) denoiser. Audio-only input (part of
//! the `Input::Audio` family) — there is no video stream, so `-vn` drops any
//! attached picture (album art) that would break audio muxers. An optional
//! `highpass=f=80` stage cuts low-frequency hum/rumble (mains buzz, HVAC).
//! The output is re-encoded to the chosen container (denoising rewrites samples,
//! so a lossless copy is impossible).

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

/// Output audio formats this tool can write (audio-family-standard set).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Mp3,
    Wav,
    Ogg,
    Flac,
    M4a,
}

impl Format {
    /// Lower-cased file extension this format writes (used for `out.<ext>`).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Mp3 => "mp3",
            Format::Wav => "wav",
            Format::Ogg => "ogg",
            Format::Flac => "flac",
            Format::M4a => "m4a",
        }
    }

    /// IANA media type for the produced file.
    pub fn mime(self) -> &'static str {
        match self {
            Format::Mp3 => "audio/mpeg",
            Format::Wav => "audio/wav",
            Format::Ogg => "audio/ogg",
            Format::Flac => "audio/flac",
            Format::M4a => "audio/mp4",
        }
    }

    /// Encoder argv fragment (`-c:a …`); lossy formats are fixed at 192 kbps.
    fn codec_args(self) -> Vec<String> {
        match self {
            Format::Mp3 => vec![
                "-c:a".into(),
                "libmp3lame".into(),
                "-b:a".into(),
                "192k".into(),
            ],
            Format::Wav => vec!["-c:a".into(), "pcm_s16le".into()],
            Format::Ogg => vec![
                "-c:a".into(),
                "libvorbis".into(),
                "-b:a".into(),
                "192k".into(),
            ],
            Format::Flac => vec!["-c:a".into(), "flac".into()],
            Format::M4a => vec!["-c:a".into(), "aac".into()],
        }
    }
}

/// Parse the user-facing output format string. Empty defaults to mp3.
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "mp3" => Ok(Format::Mp3),
        "wav" => Ok(Format::Wav),
        "ogg" => Ok(Format::Ogg),
        "flac" => Ok(Format::Flac),
        "m4a" => Ok(Format::M4a),
        other => Err(format!(
            "format {other:?} not supported (mp3|wav|ogg|flac|m4a)"
        )),
    }
}

/// Accepted strength range (percent-style intensity).
pub const MIN_STRENGTH: f64 = 1.0;
pub const MAX_STRENGTH: f64 = 100.0;
/// Default strength when the page field is empty / no value supplied.
pub const DEFAULT_STRENGTH: f64 = 12.0;

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

/// Build the ffmpeg argv (no leading `ffmpeg`) to denoise `in_name` into
/// `out_name`. Audio-only: `-vn` drops any attached picture. Shared verbatim by
/// the web page (`build_argv`) and the chat block.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    strength: f64,
    method: Method,
    remove_hum: bool,
    format: Format,
) -> Vec<String> {
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        build_filter(strength, method, remove_hum),
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate `strength`, parse `method`/`format`, and return `(argv, out_name)`.
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
pub fn plan(
    in_name: &str,
    strength: f64,
    method: &str,
    remove_hum: bool,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    let m = parse_method(method)?;
    let f = parse_format(format)?;
    if !strength.is_finite() || strength < MIN_STRENGTH || strength > MAX_STRENGTH {
        return Err(format!(
            "strength must be between {MIN_STRENGTH} and {MAX_STRENGTH} (higher = more aggressive), got {strength}"
        ));
    }
    let out_name = format!("out.{}", f.ext());
    Ok((
        build_argv(in_name, &out_name, strength, m, remove_hum, f),
        out_name,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn afftdn_argv_order_and_values() {
        let (argv, out) = plan("in.wav", 12.0, "afftdn", false, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.wav",
                "-vn",
                "-af",
                "afftdn=nr=11.64",
                "-c:a",
                "libmp3lame",
                "-b:a",
                "192k",
                "out.mp3",
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
    fn audio_only_drops_video_stream() {
        let (argv, _) = plan("in.mp3", 30.0, "afftdn", true, "mp3").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
        // never stream-copies a video track — this is an audio tool.
        assert!(!argv.windows(2).any(|w| w[0] == "-c:v"));
    }

    #[test]
    fn each_format_sets_extension_codec_and_mime() {
        let cases = [
            ("mp3", "out.mp3", "libmp3lame", "audio/mpeg"),
            ("wav", "out.wav", "pcm_s16le", "audio/wav"),
            ("ogg", "out.ogg", "libvorbis", "audio/ogg"),
            ("flac", "out.flac", "flac", "audio/flac"),
            ("m4a", "out.m4a", "aac", "audio/mp4"),
        ];
        for (fmt, out_name, codec, mime) in cases {
            let (argv, out) = plan("in.wav", 12.0, "afftdn", false, fmt).unwrap();
            assert_eq!(out, out_name);
            assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec));
            assert_eq!(parse_format(fmt).unwrap().mime(), mime);
        }
    }

    #[test]
    fn rejects_out_of_range_strength() {
        assert!(plan("a.wav", 0.0, "afftdn", false, "mp3").is_err());
        assert!(plan("a.wav", 0.5, "afftdn", false, "mp3").is_err());
        assert!(plan("a.wav", 101.0, "afftdn", false, "mp3").is_err());
        assert!(plan("a.wav", f64::NAN, "afftdn", false, "mp3").is_err());
        let err = plan("a.wav", 200.0, "afftdn", false, "mp3").unwrap_err();
        assert!(err.contains("strength must be between"));
    }

    #[test]
    fn rejects_unknown_method_and_format() {
        assert!(plan("a.wav", 12.0, "rnnoise", false, "mp3").is_err());
        assert!(plan("a.wav", 12.0, "afftdn", false, "aiff").is_err());
    }

    #[test]
    fn empty_method_and_format_default() {
        assert_eq!(parse_method("").unwrap(), Method::Afftdn);
        assert_eq!(parse_format("").unwrap(), Format::Mp3);
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(97.0), "97");
        assert_eq!(fmt_num(11.64), "11.64");
        assert_eq!(fmt_num(0.012), "0.012");
        assert_eq!(fmt_num(0.001), "0.001");
    }
}
