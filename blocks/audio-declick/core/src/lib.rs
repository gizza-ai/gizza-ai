//! gizza-ai/audio-declick core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Repairs impulsive noise — vinyl clicks and pops, tape ticks, digital buffer
//! glitches — with ffmpeg's `adeclick`, which detects offending samples and
//! replaces them with values interpolated from their neighbours by an
//! autoregressive model. An optional `adeclip` stage afterwards does the same
//! interpolation over clipped (flat-topped) peaks.
//!
//! Audio-only input (part of the `Input::Audio` family) — there is no video
//! stream, so `-vn` drops any attached picture (album art) that would otherwise
//! break audio muxers. The output is re-encoded to the chosen container
//! (repairing rewrites samples, so a lossless stream copy is impossible).

/// Overlap-processing method used to stitch repaired windows back together.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Method {
    /// Overlap-**add** (`m=a`, ffmpeg's default) — every output sample is
    /// recomputed from the overlapping windows. Smoothest across repairs, but
    /// it very slightly alters untouched audio too.
    Add,
    /// Overlap-**save** (`m=s`) — only samples the detector flagged are
    /// replaced; everything else passes through bit-identical.
    Save,
}

impl Method {
    /// The `m=` value ffmpeg expects.
    fn code(self) -> &'static str {
        match self {
            Method::Add => "a",
            Method::Save => "s",
        }
    }
}

/// Parse the user-facing method string. Empty defaults to overlap-add.
pub fn parse_method(s: &str) -> Result<Method, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "add" | "a" => Ok(Method::Add),
        "save" | "s" => Ok(Method::Save),
        other => Err(format!("method {other:?} not supported (add|save)")),
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

/// Accepted detection-strength range (percent-style intensity).
pub const MIN_STRENGTH: f64 = 1.0;
pub const MAX_STRENGTH: f64 = 100.0;
/// Default strength when the page field is empty / no value supplied.
pub const DEFAULT_STRENGTH: f64 = 50.0;

/// Accepted repair-window range, in milliseconds (ffmpeg's `adeclick=w`).
pub const MIN_WINDOW_MS: f64 = 10.0;
pub const MAX_WINDOW_MS: f64 = 100.0;
/// Default repair window (ffmpeg's own default).
pub const DEFAULT_WINDOW_MS: f64 = 55.0;

/// Accepted burst-fusion range (ffmpeg's `adeclick=b`, percent of the window).
pub const MIN_BURST: i64 = 0;
pub const MAX_BURST: i64 = 10;
/// Default burst fusion (ffmpeg's own default).
pub const DEFAULT_BURST: i64 = 2;

/// `adeclick`'s threshold band that actually repairs anything. The filter
/// accepts `t` up to 100, but measurements on real clicks show detections stop
/// around 15 — so the friendly 1–100 strength slider maps into 1–20 instead of
/// passing straight through (see the competitor-analysis engine spike).
pub const MIN_THRESHOLD: f64 = 1.0;
pub const MAX_THRESHOLD: f64 = 20.0;

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`55` not `55.0`,
/// `10.5` stays `10.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.5}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Map the friendly 1–100 detection strength to `adeclick`'s native threshold,
/// which runs the other way (LOWER `t` flags more samples as noise). Strength 1
/// → `t = 19.81` (only the most violent spikes), strength 50 → `t = 10.5` (the
/// default, repairs ordinary full-scale clicks), strength 100 → `t = 1` (the
/// filter's most sensitive setting). Clamped so float wobble can never land
/// outside ffmpeg's accepted `[1, 100]`.
pub fn strength_to_threshold(strength: f64) -> f64 {
    (MAX_THRESHOLD - 0.19 * strength).clamp(MIN_THRESHOLD, MAX_THRESHOLD)
}

/// Build the ffmpeg `-af` chain: one `adeclick` stage, optionally followed by
/// an `adeclip` stage that interpolates over clipped peaks the same way.
pub fn build_filter(
    strength: f64,
    window_ms: f64,
    burst: i64,
    method: Method,
    declip: bool,
) -> String {
    let stage = format!(
        "adeclick=w={}:t={}:b={}:m={}",
        fmt_num(window_ms),
        fmt_num(strength_to_threshold(strength)),
        burst,
        method.code()
    );
    if declip {
        format!(
            "{stage},adeclip=w={}:m={}",
            fmt_num(window_ms),
            method.code()
        )
    } else {
        stage
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to declick `in_name` into
/// `out_name`. Audio-only: `-vn` drops any attached picture. Shared verbatim by
/// the web page (`build_argv`) and the chat block.
#[allow(clippy::too_many_arguments)]
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    strength: f64,
    window_ms: f64,
    burst: i64,
    method: Method,
    declip: bool,
    format: Format,
) -> Vec<String> {
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        build_filter(strength, window_ms, burst, method, declip),
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate every option, then return `(argv, out_name)`. Single source shared
/// by the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan(
    in_name: &str,
    strength: f64,
    window_ms: f64,
    burst: i64,
    method: &str,
    declip: bool,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    let m = parse_method(method)?;
    let f = parse_format(format)?;
    if !strength.is_finite() || strength < MIN_STRENGTH || strength > MAX_STRENGTH {
        return Err(format!(
            "strength must be between {MIN_STRENGTH} and {MAX_STRENGTH} (higher = more aggressive detection), got {strength}"
        ));
    }
    if !window_ms.is_finite() || window_ms < MIN_WINDOW_MS || window_ms > MAX_WINDOW_MS {
        return Err(format!(
            "window must be between {MIN_WINDOW_MS} and {MAX_WINDOW_MS} milliseconds, got {window_ms}"
        ));
    }
    if !(MIN_BURST..=MAX_BURST).contains(&burst) {
        return Err(format!(
            "burst must be between {MIN_BURST} and {MAX_BURST} (0 disables burst fusion), got {burst}"
        ));
    }
    let out_name = format!("out.{}", f.ext());
    Ok((
        build_argv(in_name, &out_name, strength, window_ms, burst, m, declip, f),
        out_name,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_argv_order_and_values() {
        let (argv, out) = plan("in.wav", 50.0, 55.0, 2, "add", false, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.wav",
                "-vn",
                "-af",
                "adeclick=w=55:t=10.5:b=2:m=a",
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
    fn strength_maps_inversely_into_the_useful_threshold_band() {
        // Verified against real ffmpeg runs: t <= 10.5 repairs full-scale
        // clicks, t >= 15 leaves them in place.
        assert_eq!(fmt_num(strength_to_threshold(50.0)), "10.5");
        assert_eq!(fmt_num(strength_to_threshold(1.0)), "19.81");
        assert_eq!(fmt_num(strength_to_threshold(100.0)), "1");
        assert!(strength_to_threshold(100.0) >= MIN_THRESHOLD);
        // Monotonically more sensitive as strength rises.
        assert!(strength_to_threshold(80.0) < strength_to_threshold(20.0));
    }

    #[test]
    fn overlap_save_method_uses_m_s() {
        assert_eq!(
            build_filter(50.0, 55.0, 2, Method::Save, false),
            "adeclick=w=55:t=10.5:b=2:m=s"
        );
        assert_eq!(parse_method("save").unwrap(), Method::Save);
        assert_eq!(parse_method("s").unwrap(), Method::Save);
        assert_eq!(parse_method("ADD").unwrap(), Method::Add);
    }

    #[test]
    fn declip_appends_an_adeclip_stage_with_the_same_window_and_method() {
        assert_eq!(
            build_filter(60.0, 40.0, 5, Method::Save, true),
            "adeclick=w=40:t=8.6:b=5:m=s,adeclip=w=40:m=s"
        );
        assert!(!build_filter(60.0, 40.0, 5, Method::Save, false).contains("adeclip"));
    }

    #[test]
    fn burst_zero_disables_fusion_and_is_still_emitted() {
        assert_eq!(
            build_filter(85.0, 25.0, 0, Method::Add, false),
            "adeclick=w=25:t=3.85:b=0:m=a"
        );
    }

    #[test]
    fn audio_only_drops_video_stream() {
        let (argv, _) = plan("in.mp3", 70.0, 55.0, 5, "add", true, "wav").unwrap();
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
            let (argv, out) = plan("in.wav", 50.0, 55.0, 2, "add", false, fmt).unwrap();
            assert_eq!(out, out_name);
            assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec));
            assert_eq!(parse_format(fmt).unwrap().mime(), mime);
        }
    }

    #[test]
    fn rejects_out_of_range_strength() {
        assert!(plan("a.wav", 0.0, 55.0, 2, "add", false, "mp3").is_err());
        assert!(plan("a.wav", 100.5, 55.0, 2, "add", false, "mp3").is_err());
        assert!(plan("a.wav", f64::NAN, 55.0, 2, "add", false, "mp3").is_err());
        let err = plan("a.wav", 200.0, 55.0, 2, "add", false, "mp3").unwrap_err();
        assert!(err.contains("strength must be between"));
    }

    #[test]
    fn rejects_out_of_range_window_and_burst() {
        assert!(plan("a.wav", 50.0, 9.0, 2, "add", false, "mp3").is_err());
        assert!(plan("a.wav", 50.0, 101.0, 2, "add", false, "mp3").is_err());
        assert!(plan("a.wav", 50.0, f64::INFINITY, 2, "add", false, "mp3").is_err());
        assert!(plan("a.wav", 50.0, 55.0, -1, "add", false, "mp3").is_err());
        let err = plan("a.wav", 50.0, 55.0, 11, "add", false, "mp3").unwrap_err();
        assert!(err.contains("burst must be between"));
    }

    #[test]
    fn rejects_unknown_method_and_format() {
        assert!(plan("a.wav", 50.0, 55.0, 2, "adaptive", false, "mp3").is_err());
        assert!(plan("a.wav", 50.0, 55.0, 2, "add", false, "aiff").is_err());
    }

    #[test]
    fn empty_method_and_format_default() {
        assert_eq!(parse_method("").unwrap(), Method::Add);
        assert_eq!(parse_format("").unwrap(), Format::Mp3);
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(55.0), "55");
        assert_eq!(fmt_num(10.5), "10.5");
        assert_eq!(fmt_num(19.81), "19.81");
        assert_eq!(fmt_num(3.85), "3.85");
    }
}
