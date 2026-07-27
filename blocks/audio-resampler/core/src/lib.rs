//! gizza-ai/audio-resampler core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Changes an audio file's SAMPLE RATE (Hz) with ffmpeg's high-quality
//! swresample resampler (`-ar <rate>`, a windowed-sinc filter with anti-alias
//! low-pass). This is a different job from audio-convert (which changes the
//! container/codec): here the rate is the point, and the output format only
//! decides how the resampled audio is stored. `-vn` drops any embedded
//! album-art (attached-picture video) stream so audio-only muxers don't choke.
//!
//! Output defaults to lossless WAV so the resample isn't degraded by a lossy
//! re-encode; pick a compressed format when you need a smaller file. Lossy
//! targets encode at a fixed transparent bitrate (bitrate itself is
//! audio-convert's job, kept out of this tool's schema on purpose).

/// Output audio formats audio-resampler can write.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Wav,
    Flac,
    Mp3,
    Ogg,
    M4a,
}

impl Format {
    /// Lower-cased file extension this format writes (used for `out.<ext>`).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Wav => "wav",
            Format::Flac => "flac",
            Format::Mp3 => "mp3",
            Format::Ogg => "ogg",
            Format::M4a => "m4a",
        }
    }

    /// IANA media type for the produced file.
    pub fn mime(self) -> &'static str {
        match self {
            Format::Wav => "audio/wav",
            Format::Flac => "audio/flac",
            Format::Mp3 => "audio/mpeg",
            Format::Ogg => "audio/ogg",
            Format::M4a => "audio/mp4",
        }
    }

    /// Encoder argv fragment (`-c:a …`). Lossy formats append a fixed,
    /// transparent `-b:a` — this tool doesn't expose bitrate (that's
    /// audio-convert's job); it just needs a sensible non-degrading default.
    fn codec_args(self) -> Vec<String> {
        match self {
            Format::Wav => vec!["-c:a".into(), "pcm_s16le".into()],
            Format::Flac => vec!["-c:a".into(), "flac".into()],
            Format::Mp3 => vec!["-c:a".into(), "libmp3lame".into(), "-b:a".into(), "192k".into()],
            Format::Ogg => vec!["-c:a".into(), "libvorbis".into(), "-b:a".into(), "192k".into()],
            Format::M4a => vec!["-c:a".into(), "aac".into(), "-b:a".into(), "192k".into()],
        }
    }
}

/// Parse the user-facing format string. `wav` is the default target and is
/// applied by the caller before parsing, so an empty string is an error here.
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "wav" => Ok(Format::Wav),
        "flac" => Ok(Format::Flac),
        "mp3" => Ok(Format::Mp3),
        "ogg" => Ok(Format::Ogg),
        "m4a" => Ok(Format::M4a),
        other => Err(format!(
            "format {other:?} not supported (wav|flac|mp3|ogg|m4a)"
        )),
    }
}

/// The default output format when none is supplied — lossless so a resample
/// never loses quality to a lossy re-encode.
pub const DEFAULT_FORMAT: &str = "wav";

/// Standard sample rates offered as page presets / the LLM-facing suggestions.
/// The tool accepts any rate in [`MIN_RATE`, `MAX_RATE`], not only these.
pub const COMMON_RATES: [u32; 10] = [
    8_000, 11_025, 16_000, 22_050, 32_000, 44_100, 48_000, 88_200, 96_000, 192_000,
];

/// Lowest sample rate ffmpeg encoders reliably accept here.
pub const MIN_RATE: u32 = 3_000;
/// Highest sample rate this tool allows (well past studio 192 kHz).
pub const MAX_RATE: u32 = 384_000;

/// Validate a requested sample rate is a positive integer inside the supported
/// window. Returns the rate unchanged on success.
pub fn validate_rate(rate: u32) -> Result<u32, String> {
    if rate < MIN_RATE || rate > MAX_RATE {
        return Err(format!(
            "sample rate {rate} Hz out of range (expected {MIN_RATE}-{MAX_RATE} Hz, e.g. 44100 or 48000)"
        ));
    }
    Ok(rate)
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to resample `in_name` to
/// `out_name` at `rate` Hz. Shared verbatim by the web page (`build_argv`) and
/// the chat block. `-ar` is what triggers swresample's high-quality windowed-
/// sinc resampling with an anti-alias low-pass.
pub fn build_argv(in_name: &str, out_name: &str, rate: u32, format: Format) -> Vec<String> {
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-ar".to_string(),
        rate.to_string(),
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate `rate`, parse `format`, and return `(argv, out_name)` for an input
/// file. `out_name` is `out.<ext>` for the chosen format. Single source shared
/// by the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_resample(
    in_name: &str,
    rate: u32,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    let rate = validate_rate(rate)?;
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, rate, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_argv_order_and_values() {
        let (argv, out) = plan_resample("in.mp3", 16_000, "wav").unwrap();
        assert_eq!(out, "out.wav");
        assert_eq!(
            argv,
            vec![
                "-i", "in.mp3", "-vn", "-ar", "16000", "-c:a", "pcm_s16le", "out.wav",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn rate_precedes_the_codec_for_every_format() {
        // `-ar` must land before `-c:a` so the encoder sees the resampled rate.
        for f in ["wav", "flac", "mp3", "ogg", "m4a"] {
            let (argv, _) = plan_resample("in.wav", 48_000, f).unwrap();
            let ar = argv.iter().position(|a| a == "-ar").expect("has -ar");
            let ca = argv.iter().position(|a| a == "-c:a").expect("has -c:a");
            assert!(ar < ca, "format {f}: -ar must precede -c:a");
            assert_eq!(argv[ar + 1], "48000", "format {f}: rate value");
        }
    }

    #[test]
    fn every_format_maps_to_its_codec_and_out_name() {
        for (f, codec, out) in [
            ("wav", "pcm_s16le", "out.wav"),
            ("flac", "flac", "out.flac"),
            ("mp3", "libmp3lame", "out.mp3"),
            ("ogg", "libvorbis", "out.ogg"),
            ("m4a", "aac", "out.m4a"),
        ] {
            let (argv, out_name) = plan_resample("in.mp3", 44_100, f).unwrap();
            assert_eq!(out_name, out, "format {f}");
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        // Album-art files carry an attached-picture video stream; -vn keeps
        // audio-only muxers (wav especially) from failing on it.
        for f in ["wav", "flac", "mp3", "ogg", "m4a"] {
            let (argv, _) = plan_resample("in.mp3", 22_050, f).unwrap();
            assert!(argv.iter().any(|a| a == "-vn"), "format {f} missing -vn");
        }
    }

    #[test]
    fn lossless_formats_carry_no_bitrate() {
        for f in ["wav", "flac"] {
            let (argv, _) = plan_resample("in.mp3", 44_100, f).unwrap();
            assert!(
                !argv.iter().any(|a| a == "-b:a"),
                "{f} is lossless, no bitrate"
            );
        }
    }

    #[test]
    fn lossy_formats_carry_the_fixed_bitrate() {
        for f in ["mp3", "ogg", "m4a"] {
            let (argv, _) = plan_resample("in.wav", 48_000, f).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-b:a" && w[1] == "192k"),
                "{f} must encode at 192k"
            );
        }
    }

    #[test]
    fn rate_out_of_range_is_rejected() {
        assert!(plan_resample("in.wav", 0, "wav").is_err());
        assert!(plan_resample("in.wav", MIN_RATE - 1, "wav").is_err());
        assert!(plan_resample("in.wav", MAX_RATE + 1, "wav").is_err());
        // Boundaries and a common studio rate all pass.
        assert!(validate_rate(MIN_RATE).is_ok());
        assert!(validate_rate(MAX_RATE).is_ok());
        assert_eq!(validate_rate(48_000).unwrap(), 48_000);
    }

    #[test]
    fn parse_format_has_no_empty_default() {
        assert!(parse_format("").is_err());
        assert!(parse_format("aiff").is_err());
        assert_eq!(parse_format("WAV").unwrap(), Format::Wav);
        assert_eq!(parse_format(" flac ").unwrap(), Format::Flac);
    }

    #[test]
    fn format_ext_and_mime_pairs() {
        assert_eq!(Format::Wav.mime(), "audio/wav");
        assert_eq!(Format::Flac.mime(), "audio/flac");
        assert_eq!(Format::Mp3.mime(), "audio/mpeg");
        assert_eq!(Format::Ogg.mime(), "audio/ogg");
        assert_eq!(Format::M4a.mime(), "audio/mp4");
        assert_eq!(Format::M4a.ext(), "m4a");
    }

    #[test]
    fn common_rates_are_all_in_range() {
        for r in COMMON_RATES {
            assert!(validate_rate(r).is_ok(), "{r} should be valid");
        }
    }
}
