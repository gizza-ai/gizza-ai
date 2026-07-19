//! gizza-ai/audio-time-stretch core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Time-stretching speeds audio up or slows it down WITHOUT changing the pitch
//! — the inverse of audio-pitch-shift (which changes pitch, keeps the tempo).
//! ffmpeg's `atempo` filter is exactly this: a WSOLA time-stretch that scales
//! the tempo while preserving pitch. `atempo` accepts 0.5..=2.0 per instance,
//! so a `factor` outside that window is decomposed into a chain of `atempo=`
//! filters whose product equals the requested factor (e.g. 4.0 → `atempo=2.0,
//! atempo=2.0`; 0.25 → `atempo=0.5,atempo=0.5`).
//!
//! `factor` is the speed multiplier: `2.0` plays twice as fast (half the
//! duration), `0.5` plays half as fast (double the duration). Because it is a
//! ratio, a BPM change maps directly: factor = target_bpm / source_bpm.

/// Output audio formats audio-time-stretch can write. All five encoders are
/// present in both the native ffmpeg runtime (chat/CLI) and the browser
/// @ffmpeg/core build (family-proven: libmp3lame, pcm_s16le, libvorbis, flac, aac).
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

    /// Encoder argv fragment (`-c:a …`). MP3 is fixed at 192 kbps (family
    /// standard); WAV is lossless 16-bit PCM; FLAC is lossless compressed;
    /// M4A uses ffmpeg's built-in AAC encoder.
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

/// Parse the user-facing format string (the values the chat schema + page
/// accept). Empty defaults to mp3.
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

/// Slowest playback (quarter speed → four times the duration). Beyond this the
/// WSOLA time-stretch artifacts dominate and the result no longer sounds like
/// the source.
pub const MIN_FACTOR: f64 = 0.25;
/// Fastest playback (quadruple speed → a quarter of the duration).
pub const MAX_FACTOR: f64 = 4.0;

/// Decompose `factor` into a chain of `atempo=` filters each within the
/// conservative 0.5..=2.0 per-instance range, whose product is `factor`.
/// `1.0` still emits `atempo=1` (an explicit no-op keeps the chain non-empty
/// and the argv shape uniform).
pub fn atempo_chain(factor: f64) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut f = factor;
    while f > 2.0 {
        parts.push("atempo=2.0".to_string());
        f /= 2.0;
    }
    while f < 0.5 {
        parts.push("atempo=0.5".to_string());
        f /= 0.5;
    }
    parts.push(format!("atempo={f}"));
    parts.join(",")
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to time-stretch `in_name` into
/// `out_name` by `factor`. Shared verbatim by the web page (`build_argv`) and
/// the chat block. `-vn` drops any attached-picture (album-art) video stream so
/// audio-only muxers like wav don't choke on it.
pub fn build_argv(in_name: &str, out_name: &str, factor: f64, format: Format) -> Vec<String> {
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-filter:a".to_string(),
        atempo_chain(factor),
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate `factor`, parse `format`, and return `(argv, out_name)` for an
/// input file. `out_name` is `out.<ext>` for the chosen format. Single source
/// shared by the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_time_stretch(
    in_name: &str,
    factor: f64,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    if !factor.is_finite() || factor <= 0.0 {
        return Err(format!(
            "factor must be a positive number (2 = twice as fast, 0.5 = half speed), got {factor}"
        ));
    }
    if factor < MIN_FACTOR || factor > MAX_FACTOR {
        return Err(format!(
            "factor must be between {MIN_FACTOR} and {MAX_FACTOR} \
             (2 = twice as fast, 0.5 = half speed), got {factor}"
        ));
    }
    if factor == 1.0 {
        return Err(
            "factor 1 leaves the speed unchanged — nothing to do; use a value above 1 to \
             speed up (e.g. 1.5 = 50% faster, 2 = twice as fast) or below 1 to slow down \
             (e.g. 0.75 = 25% slower, 0.5 = half speed)"
                .to_string(),
        );
    }
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, factor, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Extract the single `-filter:a` value from an argv.
    fn af(argv: &[String]) -> &str {
        let i = argv.iter().position(|a| a == "-filter:a").unwrap();
        &argv[i + 1]
    }

    #[test]
    fn double_speed_argv_order_and_values() {
        let (argv, out) = plan_time_stretch("in.mp3", 2.0, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-filter:a",
                "atempo=2",
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
    fn half_speed_single_atempo() {
        let (argv, _) = plan_time_stretch("in.wav", 0.5, "wav").unwrap();
        assert_eq!(af(&argv), "atempo=0.5");
    }

    #[test]
    fn large_factor_chains_atempo_within_range() {
        // 4x → atempo=2.0,atempo=2
        assert_eq!(atempo_chain(4.0), "atempo=2.0,atempo=2");
        // 3x → atempo=2.0,atempo=1.5
        assert_eq!(atempo_chain(3.0), "atempo=2.0,atempo=1.5");
    }

    #[test]
    fn small_factor_chains_atempo_within_range() {
        // 0.25x → atempo=0.5,atempo=0.5
        assert_eq!(atempo_chain(0.25), "atempo=0.5,atempo=0.5");
    }

    #[test]
    fn fractional_factor_single_atempo() {
        // 1.25x podcast speed fits one instance.
        assert_eq!(atempo_chain(1.25), "atempo=1.25");
        assert_eq!(atempo_chain(0.75), "atempo=0.75");
    }

    #[test]
    fn drops_video_stream() {
        let (argv, _) = plan_time_stretch("in.mp3", 1.5, "mp3").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"), "must drop album-art video: {argv:?}");
    }

    #[test]
    fn format_selects_codec_and_out_name() {
        let (argv, out) = plan_time_stretch("in.mp3", 1.5, "wav").unwrap();
        assert_eq!(out, "out.wav");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "pcm_s16le"));
        let (argv, out) = plan_time_stretch("in.mp3", 1.5, "flac").unwrap();
        assert_eq!(out, "out.flac");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "flac"));
        let (argv, out) = plan_time_stretch("in.mp3", 1.5, "m4a").unwrap();
        assert_eq!(out, "out.m4a");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
        let (argv, out) = plan_time_stretch("in.mp3", 1.5, "ogg").unwrap();
        assert_eq!(out, "out.ogg");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libvorbis"));
    }

    #[test]
    fn empty_format_defaults_to_mp3() {
        let (_, out) = plan_time_stretch("in.ogg", 1.5, "").unwrap();
        assert_eq!(out, "out.mp3");
    }

    #[test]
    fn rejects_no_op_factor_with_guiding_error() {
        let err = plan_time_stretch("a.mp3", 1.0, "mp3").unwrap_err();
        assert!(err.contains("nothing to do"), "{err}");
        assert!(err.contains("speed up"), "{err}");
    }

    #[test]
    fn rejects_out_of_range_or_non_finite_factor() {
        assert!(plan_time_stretch("a.mp3", 0.0, "mp3").is_err());
        assert!(plan_time_stretch("a.mp3", -2.0, "mp3").is_err());
        assert!(plan_time_stretch("a.mp3", 5.0, "mp3").is_err());
        assert!(plan_time_stretch("a.mp3", 0.1, "mp3").is_err());
        assert!(plan_time_stretch("a.mp3", f64::NAN, "mp3").is_err());
        assert!(plan_time_stretch("a.mp3", f64::INFINITY, "mp3").is_err());
        let err = plan_time_stretch("a.mp3", 8.0, "mp3").unwrap_err();
        assert!(err.contains("between 0.25 and 4"), "{err}");
        // Bounds themselves are inclusive.
        assert!(plan_time_stretch("a.mp3", 4.0, "mp3").is_ok());
        assert!(plan_time_stretch("a.mp3", 0.25, "mp3").is_ok());
    }

    #[test]
    fn rejects_unknown_format() {
        let err = plan_time_stretch("a.mp3", 1.5, "aiff").unwrap_err();
        assert!(err.contains("mp3|wav|ogg|flac|m4a"), "{err}");
    }

    #[test]
    fn format_ext_and_mime_pairs() {
        assert_eq!(Format::Mp3.mime(), "audio/mpeg");
        assert_eq!(Format::Wav.mime(), "audio/wav");
        assert_eq!(Format::Ogg.mime(), "audio/ogg");
        assert_eq!(Format::Flac.mime(), "audio/flac");
        assert_eq!(Format::M4a.mime(), "audio/mp4");
        assert_eq!(Format::M4a.ext(), "m4a");
    }
}
