//! gizza-ai/audio-pitch-shift core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Pitch shifting without a tempo change uses the classic resample trick,
//! entirely with stock ffmpeg filters (no librubberband in either the native
//! runtime or the browser @ffmpeg/core build):
//!
//! 1. `aresample=44100` — normalize any input to a known working rate so the
//!    rest of the chain is input-independent.
//! 2. `asetrate=<44100 × 2^(semitones/12)>` — relabel the sample rate, which
//!    raises/lowers pitch AND speed together (the tape-speed effect).
//! 3. `aresample=44100` — physically resample back to the working rate; the
//!    audio is now pitch-shifted but plays too fast/slow.
//! 4. `atempo=<44100 / relabeled-rate>` — undo exactly the speed change while
//!    keeping the new pitch (WSOLA time-stretch). `atempo` only accepts
//!    0.5..=2.0 per instance in older builds, so values outside that range are
//!    chained (±12 semitones fits one instance; ±24 needs two).
//!
//! The `atempo` value is computed from the ROUNDED `asetrate` integer so the
//! output duration matches the input exactly; the sub-cent pitch error from
//! rounding (< 0.04 cents at 44.1 kHz) is far below audibility.

/// Output audio formats audio-pitch-shift can write. All five encoders are
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

/// Working sample rate the whole chain is normalized to (CD standard — most
/// music input already sits here, so step 1 is usually a no-op).
pub const WORK_RATE: u32 = 44_100;

/// Largest shift in either direction, in semitones (two octaves — matches the
/// widest range seen across competitor pitch shifters). Beyond ±24 the
/// resample/WSOLA artifacts dominate and the result is noise, not music.
pub const MAX_SEMITONES: f64 = 24.0;

/// Frequency ratio for a shift of `semitones` (12-tone equal temperament):
/// `2^(semitones/12)`. +12 → 2.0, -12 → 0.5, +7 → ≈1.4983.
pub fn pitch_factor(semitones: f64) -> f64 {
    (semitones / 12.0).exp2()
}

/// The relabeled sample rate for step 2 — `WORK_RATE × pitch_factor`, rounded
/// to an integer (asetrate takes integer Hz).
pub fn shifted_rate(semitones: f64) -> u32 {
    (f64::from(WORK_RATE) * pitch_factor(semitones)).round() as u32
}

/// Build the `-af` chain for a `semitones` shift (validated by the caller).
/// The tempo correction is derived from the ROUNDED rate so duration is exact,
/// and chained through as many `atempo` instances as needed to keep each one
/// within the conservative 0.5..=2.0 per-instance range.
pub fn build_filter(semitones: f64) -> String {
    let rate = shifted_rate(semitones);
    let mut parts = vec![
        format!("aresample={WORK_RATE}"),
        format!("asetrate={rate}"),
        format!("aresample={WORK_RATE}"),
    ];
    let mut tempo = f64::from(WORK_RATE) / f64::from(rate);
    while tempo > 2.0 {
        parts.push("atempo=2".to_string());
        tempo /= 2.0;
    }
    while tempo < 0.5 {
        parts.push("atempo=0.5".to_string());
        tempo *= 2.0;
    }
    parts.push(format!("atempo={tempo}"));
    parts.join(",")
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to pitch-shift `in_name` into
/// `out_name`. Shared verbatim by the web page (`build_argv`) and the chat block.
pub fn build_argv(in_name: &str, out_name: &str, semitones: f64, format: Format) -> Vec<String> {
    // -vn drops any video stream: audio files with embedded album art carry an
    // attached-picture (video) stream that breaks audio-only muxers like wav.
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        build_filter(semitones),
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate `semitones`, parse `format`, and return `(argv, out_name)` for an
/// input file. `out_name` is `out.<ext>` for the chosen format. Single source
/// shared by the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_pitch_shift(
    in_name: &str,
    semitones: f64,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    if !semitones.is_finite() || semitones.abs() > MAX_SEMITONES {
        return Err(format!(
            "semitones must be between -{MAX_SEMITONES} and {MAX_SEMITONES} \
             (12 = one octave up, -12 = one octave down), got {semitones}"
        ));
    }
    if semitones == 0.0 {
        return Err(
            "semitones 0 leaves the pitch unchanged — nothing to do; use a positive \
             value to raise the pitch (e.g. 12 = one octave up) or a negative one to \
             lower it (e.g. -12 = one octave down)"
                .to_string(),
        );
    }
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, semitones, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Extract the single `-af` value from an argv.
    fn af(argv: &[String]) -> &str {
        let i = argv.iter().position(|a| a == "-af").unwrap();
        &argv[i + 1]
    }

    #[test]
    fn octave_up_argv_order_and_values() {
        let (argv, out) = plan_pitch_shift("in.mp3", 12.0, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "aresample=44100,asetrate=88200,aresample=44100,atempo=0.5",
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
    fn octave_down_uses_half_rate_and_double_tempo() {
        let (argv, _) = plan_pitch_shift("in.wav", -12.0, "wav").unwrap();
        assert_eq!(
            af(&argv),
            "aresample=44100,asetrate=22050,aresample=44100,atempo=2"
        );
    }

    #[test]
    fn two_octaves_chain_atempo_within_per_instance_range() {
        // +24 → factor 4 → rate 176400, tempo 0.25 → 0.5 × 0.5.
        let (argv, _) = plan_pitch_shift("in.mp3", 24.0, "mp3").unwrap();
        assert_eq!(
            af(&argv),
            "aresample=44100,asetrate=176400,aresample=44100,atempo=0.5,atempo=0.5"
        );
        // -24 → factor 0.25 → rate 11025, tempo 4 → 2 × 2.
        let (argv, _) = plan_pitch_shift("in.mp3", -24.0, "mp3").unwrap();
        assert_eq!(
            af(&argv),
            "aresample=44100,asetrate=11025,aresample=44100,atempo=2,atempo=2"
        );
    }

    #[test]
    fn fractional_semitones_round_rate_and_correct_tempo_exactly() {
        // +7 semitones: 44100 × 2^(7/12) = 66074.58… → asetrate=66075, and the
        // single atempo must invert the ROUNDED rate so duration stays exact.
        let f = build_filter(7.0);
        assert!(f.contains("asetrate=66075"), "{f}");
        let tempo: f64 = f.rsplit("atempo=").next().unwrap().parse().unwrap();
        let ratio = tempo * (66075.0 / 44100.0);
        assert!(
            (ratio - 1.0).abs() < 1e-12,
            "tempo must invert the rounded rate, got {tempo}"
        );
        assert_eq!(
            f.matches("atempo=").count(),
            1,
            "±12 fits one atempo instance"
        );
    }

    #[test]
    fn half_semitone_shift_is_supported() {
        // 50 cents: 44100 × 2^(0.5/12) = 45392.24… → 45392.
        let f = build_filter(0.5);
        assert!(f.contains("asetrate=45392"), "{f}");
    }

    #[test]
    fn pitch_factor_reference_points() {
        assert_eq!(pitch_factor(12.0), 2.0);
        assert_eq!(pitch_factor(-12.0), 0.5);
        assert_eq!(pitch_factor(24.0), 4.0);
        assert!((pitch_factor(7.0) - 1.498_307_076_876_681_5).abs() < 1e-12);
    }

    #[test]
    fn format_selects_codec_and_out_name() {
        let (argv, out) = plan_pitch_shift("in.mp3", 3.0, "wav").unwrap();
        assert_eq!(out, "out.wav");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "pcm_s16le"));
        let (argv, out) = plan_pitch_shift("in.mp3", 3.0, "flac").unwrap();
        assert_eq!(out, "out.flac");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "flac"));
        let (argv, out) = plan_pitch_shift("in.mp3", 3.0, "m4a").unwrap();
        assert_eq!(out, "out.m4a");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
        let (argv, out) = plan_pitch_shift("in.mp3", 3.0, "ogg").unwrap();
        assert_eq!(out, "out.ogg");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libvorbis"));
    }

    #[test]
    fn empty_format_defaults_to_mp3() {
        let (_, out) = plan_pitch_shift("in.ogg", -5.0, "").unwrap();
        assert_eq!(out, "out.mp3");
    }

    #[test]
    fn rejects_zero_shift_with_guiding_error() {
        let err = plan_pitch_shift("a.mp3", 0.0, "mp3").unwrap_err();
        assert!(err.contains("nothing to do"), "{err}");
        assert!(err.contains("octave up"), "{err}");
    }

    #[test]
    fn rejects_out_of_range_or_non_finite_semitones() {
        assert!(plan_pitch_shift("a.mp3", 25.0, "mp3").is_err());
        assert!(plan_pitch_shift("a.mp3", -24.5, "mp3").is_err());
        assert!(plan_pitch_shift("a.mp3", f64::NAN, "mp3").is_err());
        assert!(plan_pitch_shift("a.mp3", f64::INFINITY, "mp3").is_err());
        let err = plan_pitch_shift("a.mp3", 100.0, "mp3").unwrap_err();
        assert!(err.contains("between -24 and 24"), "{err}");
        // Bounds themselves are inclusive.
        assert!(plan_pitch_shift("a.mp3", 24.0, "mp3").is_ok());
        assert!(plan_pitch_shift("a.mp3", -24.0, "mp3").is_ok());
    }

    #[test]
    fn rejects_unknown_format() {
        let err = plan_pitch_shift("a.mp3", 3.0, "aiff").unwrap_err();
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
