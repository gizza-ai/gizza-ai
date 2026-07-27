//! gizza-ai/audio-filter core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Applies one of four classic audio filters to an audio clip:
//!   * **low-pass**  — frequencies *below* the cutoff pass, highs are attenuated
//!     (ffmpeg `lowpass=f=<freq>`);
//!   * **high-pass** — frequencies *above* the cutoff pass, lows are attenuated
//!     (ffmpeg `highpass=f=<freq>`);
//!   * **band-pass** — only a band centred on `frequency`, `width` Hz wide, passes
//!     (ffmpeg `bandpass=f=<freq>:width_type=h:w=<width>`);
//!   * **notch** (band-reject) — the opposite: a band centred on `frequency`,
//!     `width` Hz wide, is removed while everything else passes
//!     (ffmpeg `bandreject=f=<freq>:width_type=h:w=<width>`).
//!
//! For low-pass/high-pass, `frequency` is the corner (cutoff) frequency and
//! `width` is ignored. For band-pass/notch, `frequency` is the band centre and
//! `width` is the band's width in Hz. Audio-only input (`Input::Audio`); `-vn`
//! drops any attached picture (album art) that would break audio muxers. The
//! output is re-encoded (filtering rewrites samples, so a lossless copy is
//! impossible).

/// The four filter shapes this tool can apply.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FilterType {
    /// Pass frequencies below the cutoff; attenuate highs.
    LowPass,
    /// Pass frequencies above the cutoff; attenuate lows.
    HighPass,
    /// Pass only a band centred on the frequency; attenuate outside it.
    BandPass,
    /// Remove a narrow band centred on the frequency; pass everything else.
    Notch,
}

impl FilterType {
    /// Whether this filter uses the `width` (bandwidth) parameter. Only the two
    /// band filters do; low-pass/high-pass ignore it.
    pub fn uses_width(self) -> bool {
        matches!(self, FilterType::BandPass | FilterType::Notch)
    }
}

/// Parse the user-facing filter-type string. Empty defaults to low-pass.
pub fn parse_filter_type(s: &str) -> Result<FilterType, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "lowpass" | "low-pass" | "low_pass" => Ok(FilterType::LowPass),
        "highpass" | "high-pass" | "high_pass" => Ok(FilterType::HighPass),
        "bandpass" | "band-pass" | "band_pass" => Ok(FilterType::BandPass),
        "notch" | "bandreject" | "band-reject" | "bandstop" | "band-stop" => Ok(FilterType::Notch),
        other => Err(format!(
            "filter type {other:?} not supported (lowpass|highpass|bandpass|notch)"
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

/// Accepted frequency range, in Hz — spans the audible band so any of the four
/// filters can be placed anywhere a user would reasonably want.
pub const MIN_FREQ: f64 = 20.0;
pub const MAX_FREQ: f64 = 20000.0;
/// Default corner/centre frequency when the page field is empty / no value given.
pub const DEFAULT_FREQ: f64 = 1000.0;

/// Accepted band width range, in Hz (band-pass / notch only).
pub const MIN_WIDTH: f64 = 1.0;
pub const MAX_WIDTH: f64 = 10000.0;
/// Default band width when the page field is empty / no value given.
pub const DEFAULT_WIDTH: f64 = 200.0;

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`1000` not
/// `1000.0`, `86.5` stays `86.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.5}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Build the ffmpeg `-af` filter expression for the chosen filter. `width` is
/// only consulted for the two band filters.
pub fn build_filter(filter_type: FilterType, frequency: f64, width: f64) -> String {
    let f = fmt_num(frequency);
    match filter_type {
        FilterType::LowPass => format!("lowpass=f={f}"),
        FilterType::HighPass => format!("highpass=f={f}"),
        FilterType::BandPass => {
            format!("bandpass=f={f}:width_type=h:w={}", fmt_num(width))
        }
        FilterType::Notch => {
            format!("bandreject=f={f}:width_type=h:w={}", fmt_num(width))
        }
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to filter `in_name` into
/// `out_name`. Audio-only: `-vn` drops any attached picture. Shared verbatim by
/// the web page (`build_argv`) and the chat block.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    filter_type: FilterType,
    frequency: f64,
    width: f64,
    format: Format,
) -> Vec<String> {
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        build_filter(filter_type, frequency, width),
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate inputs, parse `filter_type`/`format`, and return `(argv, out_name)`.
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
pub fn plan(
    in_name: &str,
    filter_type: &str,
    frequency: f64,
    width: f64,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    let ft = parse_filter_type(filter_type)?;
    let fmt = parse_format(format)?;
    if !frequency.is_finite() || frequency < MIN_FREQ || frequency > MAX_FREQ {
        return Err(format!(
            "frequency must be between {MIN_FREQ} and {MAX_FREQ} Hz, got {frequency}"
        ));
    }
    if ft.uses_width() && (!width.is_finite() || width < MIN_WIDTH || width > MAX_WIDTH) {
        return Err(format!(
            "width must be between {MIN_WIDTH} and {MAX_WIDTH} Hz for band-pass/notch, got {width}"
        ));
    }
    let out_name = format!("out.{}", fmt.ext());
    Ok((
        build_argv(in_name, &out_name, ft, frequency, width, fmt),
        out_name,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_lowpass_argv_order_and_values() {
        let (argv, out) = plan("in.wav", "lowpass", 1000.0, 200.0, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.wav",
                "-vn",
                "-af",
                "lowpass=f=1000",
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
    fn each_filter_type_maps_to_ffmpeg_filter() {
        assert_eq!(
            build_filter(FilterType::LowPass, 3000.0, 200.0),
            "lowpass=f=3000"
        );
        assert_eq!(
            build_filter(FilterType::HighPass, 80.0, 200.0),
            "highpass=f=80"
        );
        assert_eq!(
            build_filter(FilterType::BandPass, 300.0, 3000.0),
            "bandpass=f=300:width_type=h:w=3000"
        );
        assert_eq!(
            build_filter(FilterType::Notch, 60.0, 20.0),
            "bandreject=f=60:width_type=h:w=20"
        );
    }

    #[test]
    fn lowpass_highpass_ignore_width() {
        // width is irrelevant for LP/HP — the produced filter must not embed it.
        assert_eq!(
            build_filter(FilterType::LowPass, 5000.0, 9999.0),
            "lowpass=f=5000"
        );
        assert_eq!(
            build_filter(FilterType::HighPass, 100.0, 9999.0),
            "highpass=f=100"
        );
    }

    #[test]
    fn filter_type_aliases_parse() {
        assert_eq!(parse_filter_type("low-pass").unwrap(), FilterType::LowPass);
        assert_eq!(parse_filter_type("HIGHPASS").unwrap(), FilterType::HighPass);
        assert_eq!(parse_filter_type("band-pass").unwrap(), FilterType::BandPass);
        assert_eq!(parse_filter_type("bandreject").unwrap(), FilterType::Notch);
        assert_eq!(parse_filter_type("band-stop").unwrap(), FilterType::Notch);
        assert_eq!(parse_filter_type("").unwrap(), FilterType::LowPass);
    }

    #[test]
    fn fractional_frequency_and_width_kept_compact() {
        assert_eq!(
            build_filter(FilterType::BandPass, 440.5, 12.5),
            "bandpass=f=440.5:width_type=h:w=12.5"
        );
    }

    #[test]
    fn audio_only_drops_video_stream() {
        let (argv, _) = plan("in.mp3", "notch", 60.0, 20.0, "mp3").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
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
            let (argv, out) = plan("in.wav", "lowpass", 1000.0, 200.0, fmt).unwrap();
            assert_eq!(out, out_name);
            assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec));
            assert_eq!(parse_format(fmt).unwrap().mime(), mime);
        }
    }

    #[test]
    fn rejects_out_of_range_frequency() {
        assert!(plan("a.wav", "lowpass", 19.0, 200.0, "mp3").is_err());
        assert!(plan("a.wav", "lowpass", 0.0, 200.0, "mp3").is_err());
        assert!(plan("a.wav", "lowpass", 20001.0, 200.0, "mp3").is_err());
        assert!(plan("a.wav", "lowpass", f64::NAN, 200.0, "mp3").is_err());
        let err = plan("a.wav", "lowpass", 30000.0, 200.0, "mp3").unwrap_err();
        assert!(err.contains("frequency must be between"));
    }

    #[test]
    fn rejects_out_of_range_width_for_band_filters_only() {
        // band filters validate width…
        assert!(plan("a.wav", "bandpass", 1000.0, 0.0, "mp3").is_err());
        assert!(plan("a.wav", "notch", 1000.0, 20001.0, "mp3").is_err());
        let err = plan("a.wav", "bandpass", 1000.0, 0.5, "mp3").unwrap_err();
        assert!(err.contains("width must be between"));
        // …but LP/HP don't, so an out-of-range width is harmless there.
        assert!(plan("a.wav", "lowpass", 1000.0, 0.0, "mp3").is_ok());
        assert!(plan("a.wav", "highpass", 1000.0, 99999.0, "mp3").is_ok());
    }

    #[test]
    fn rejects_unknown_filter_type_and_format() {
        assert!(plan("a.wav", "shelf", 1000.0, 200.0, "mp3").is_err());
        assert!(plan("a.wav", "lowpass", 1000.0, 200.0, "aiff").is_err());
    }

    #[test]
    fn boundary_values_accepted() {
        assert!(plan("a.wav", "lowpass", MIN_FREQ, 200.0, "mp3").is_ok());
        assert!(plan("a.wav", "lowpass", MAX_FREQ, 200.0, "mp3").is_ok());
        assert!(plan("a.wav", "bandpass", 1000.0, MIN_WIDTH, "mp3").is_ok());
        assert!(plan("a.wav", "bandpass", 1000.0, MAX_WIDTH, "mp3").is_ok());
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(1000.0), "1000");
        assert_eq!(fmt_num(440.5), "440.5");
        assert_eq!(fmt_num(60.0), "60");
    }

    #[test]
    fn uses_width_only_for_band_filters() {
        assert!(!FilterType::LowPass.uses_width());
        assert!(!FilterType::HighPass.uses_width());
        assert!(FilterType::BandPass.uses_width());
        assert!(FilterType::Notch.uses_width());
    }
}
