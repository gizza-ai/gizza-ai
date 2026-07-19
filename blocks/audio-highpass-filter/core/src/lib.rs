//! gizza-ai/audio-highpass-filter core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Applies a high-pass filter to an audio clip: frequencies above `cutoff` pass
//! through, everything below is attenuated. This removes low-frequency rumble,
//! mains hum, HVAC drone, table thumps, and handling noise while leaving speech
//! and most instruments intact. Audio-only input (part of the `Input::Audio`
//! family) — there is no video stream, so `-vn` drops any attached picture
//! (album art) that would break audio muxers.
//!
//! The `rolloff` is expressed in dB/octave (how steeply the filter cuts below the
//! cutoff) and mapped onto cascaded ffmpeg `highpass` biquads: a single 1-pole
//! stage is ~6 dB/oct, a single 2-pole stage ~12 dB/oct, and steeper slopes are
//! built by chaining identical 2-pole stages (24 = 2×, 48 = 4×). The output is
//! re-encoded to the chosen container (filtering rewrites samples, so a lossless
//! stream copy is impossible).

/// Filter steepness, in dB per octave, below the cutoff frequency.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Rolloff {
    /// ~6 dB/oct — a single 1-pole stage; the gentlest, most transparent slope.
    Db6,
    /// ~12 dB/oct — a single 2-pole stage; the natural default for voice.
    Db12,
    /// ~24 dB/oct — two cascaded 2-pole stages; tighter low-end control.
    Db24,
    /// ~48 dB/oct — four cascaded 2-pole stages; a steep brick-wall style cut.
    Db48,
}

impl Rolloff {
    /// The ffmpeg `highpass` pole count per stage (1 or 2) and the number of
    /// identical stages to chain to reach this slope.
    fn stages(self) -> (u32, u32) {
        match self {
            Rolloff::Db6 => (1, 1),
            Rolloff::Db12 => (2, 1),
            Rolloff::Db24 => (2, 2),
            Rolloff::Db48 => (2, 4),
        }
    }
}

/// Parse the user-facing rolloff string (dB/octave). Empty defaults to 12 dB/oct.
pub fn parse_rolloff(s: &str) -> Result<Rolloff, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "12" => Ok(Rolloff::Db12),
        "6" => Ok(Rolloff::Db6),
        "24" => Ok(Rolloff::Db24),
        "48" => Ok(Rolloff::Db48),
        other => Err(format!(
            "rolloff {other:?} not supported (6|12|24|48 dB/octave)"
        )),
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

/// Accepted cutoff-frequency range, in Hz. The low bound stays above DC; the high
/// bound keeps this a low-cut/rumble tool rather than a general band filter.
pub const MIN_CUTOFF: f64 = 10.0;
pub const MAX_CUTOFF: f64 = 2000.0;
/// Default cutoff when the page field is empty / no value supplied — the industry
/// standard for removing rumble without touching speech.
pub const DEFAULT_CUTOFF: f64 = 80.0;

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`80` not `80.0`,
/// `85.5` stays `85.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.5}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Build the ffmpeg `-af` filter chain: one or more `highpass=f=<cutoff>:p=<poles>`
/// stages cascaded to reach the requested slope.
pub fn build_filter(cutoff: f64, rolloff: Rolloff) -> String {
    let (poles, count) = rolloff.stages();
    let stage = format!("highpass=f={}:p={}", fmt_num(cutoff), poles);
    vec![stage; count as usize].join(",")
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to high-pass `in_name` into
/// `out_name`. Audio-only: `-vn` drops any attached picture. Shared verbatim by
/// the web page (`build_argv`) and the chat block.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    cutoff: f64,
    rolloff: Rolloff,
    format: Format,
) -> Vec<String> {
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        build_filter(cutoff, rolloff),
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate `cutoff`, parse `rolloff`/`format`, and return `(argv, out_name)`.
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
pub fn plan(
    in_name: &str,
    cutoff: f64,
    rolloff: &str,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    let r = parse_rolloff(rolloff)?;
    let f = parse_format(format)?;
    if !cutoff.is_finite() || cutoff < MIN_CUTOFF || cutoff > MAX_CUTOFF {
        return Err(format!(
            "cutoff must be between {MIN_CUTOFF} and {MAX_CUTOFF} Hz, got {cutoff}"
        ));
    }
    let out_name = format!("out.{}", f.ext());
    Ok((build_argv(in_name, &out_name, cutoff, r, f), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_argv_order_and_values() {
        let (argv, out) = plan("in.wav", 80.0, "12", "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.wav",
                "-vn",
                "-af",
                "highpass=f=80:p=2",
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
    fn rolloff_maps_to_pole_count_and_cascade() {
        assert_eq!(build_filter(100.0, Rolloff::Db6), "highpass=f=100:p=1");
        assert_eq!(build_filter(100.0, Rolloff::Db12), "highpass=f=100:p=2");
        assert_eq!(
            build_filter(100.0, Rolloff::Db24),
            "highpass=f=100:p=2,highpass=f=100:p=2"
        );
        assert_eq!(
            build_filter(100.0, Rolloff::Db48),
            "highpass=f=100:p=2,highpass=f=100:p=2,highpass=f=100:p=2,highpass=f=100:p=2"
        );
    }

    #[test]
    fn fractional_cutoff_kept_compact() {
        assert_eq!(build_filter(85.5, Rolloff::Db12), "highpass=f=85.5:p=2");
    }

    #[test]
    fn audio_only_drops_video_stream() {
        let (argv, _) = plan("in.mp3", 120.0, "24", "mp3").unwrap();
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
            let (argv, out) = plan("in.wav", 80.0, "12", fmt).unwrap();
            assert_eq!(out, out_name);
            assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec));
            assert_eq!(parse_format(fmt).unwrap().mime(), mime);
        }
    }

    #[test]
    fn rejects_out_of_range_cutoff() {
        assert!(plan("a.wav", 9.0, "12", "mp3").is_err());
        assert!(plan("a.wav", 0.0, "12", "mp3").is_err());
        assert!(plan("a.wav", 2001.0, "12", "mp3").is_err());
        assert!(plan("a.wav", f64::NAN, "12", "mp3").is_err());
        let err = plan("a.wav", 5000.0, "12", "mp3").unwrap_err();
        assert!(err.contains("cutoff must be between"));
    }

    #[test]
    fn rejects_unknown_rolloff_and_format() {
        assert!(plan("a.wav", 80.0, "18", "mp3").is_err());
        assert!(plan("a.wav", 80.0, "12", "aiff").is_err());
    }

    #[test]
    fn empty_rolloff_and_format_default() {
        assert_eq!(parse_rolloff("").unwrap(), Rolloff::Db12);
        assert_eq!(parse_format("").unwrap(), Format::Mp3);
    }

    #[test]
    fn boundary_cutoffs_accepted() {
        assert!(plan("a.wav", MIN_CUTOFF, "12", "mp3").is_ok());
        assert!(plan("a.wav", MAX_CUTOFF, "12", "mp3").is_ok());
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(80.0), "80");
        assert_eq!(fmt_num(85.5), "85.5");
        assert_eq!(fmt_num(120.0), "120");
    }
}
