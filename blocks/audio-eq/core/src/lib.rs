//! gizza-ai/audio-eq core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Three-band equalizer: `bass` is ffmpeg's low shelf (corner ~100 Hz),
//! `mid` a 1 kHz peaking band (`equalizer`, Q=1), `treble` the high shelf
//! (corner ~3 kHz) — each a gain in dB, 0 = leave the band alone. Zero-gain
//! stages are omitted from the filter chain, and an all-zero request is
//! rejected as a no-op rather than wasting a lossy re-encode on nothing.
//! `-vn` drops attached-picture streams (album art).

/// Output audio formats audio-eq can write (family-standard set).
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

/// Parse the user-facing format string. Empty defaults to mp3.
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

/// Per-band gain range (dB, symmetric). ±20 dB is far beyond any musical
/// correction; bigger values are almost always a mistake.
pub const MAX_GAIN_DB: f64 = 20.0;

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`6` not `6.0`,
/// `1.5` stays `1.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn validate_gain(name: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() || v.abs() > MAX_GAIN_DB {
        return Err(format!(
            "{name} must be between -{MAX_GAIN_DB} and {MAX_GAIN_DB} dB, got {v}"
        ));
    }
    Ok(())
}

/// Build the `-af` chain from the non-zero bands, in bass → mid → treble
/// order. Returns an error when every band is 0 (a pointless re-encode).
pub fn build_filter(bass: f64, mid: f64, treble: f64) -> Result<String, String> {
    let mut stages = Vec::new();
    if bass != 0.0 {
        stages.push(format!("bass=g={}", fmt_num(bass)));
    }
    if mid != 0.0 {
        stages.push(format!("equalizer=f=1000:t=q:w=1:g={}", fmt_num(mid)));
    }
    if treble != 0.0 {
        stages.push(format!("treble=g={}", fmt_num(treble)));
    }
    if stages.is_empty() {
        return Err(
            "all bands are 0 — nothing to change; set at least one of bass, mid or treble \
             (e.g. bass 6 to warm it up, treble 4 to brighten)"
                .to_string(),
        );
    }
    Ok(stages.join(","))
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to equalize `in_name` into
/// `out_name`. Shared verbatim by the web page (`build_argv`) and the chat block.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    filter: &str,
    format: Format,
) -> Vec<String> {
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        filter.to_string(),
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate the three gains, parse `format`, and return `(argv, out_name)`.
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
pub fn plan_eq(
    in_name: &str,
    bass: f64,
    mid: f64,
    treble: f64,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    validate_gain("bass", bass)?;
    validate_gain("mid", mid)?;
    validate_gain("treble", treble)?;
    let filter = build_filter(bass, mid, treble)?;
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, &filter, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_three_band_argv_order_and_values() {
        let (argv, out) = plan_eq("in.mp3", 6.0, -4.0, 3.0, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "bass=g=6,equalizer=f=1000:t=q:w=1:g=-4,treble=g=3",
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
    fn zero_bands_are_omitted_from_the_chain() {
        assert_eq!(build_filter(6.0, 0.0, 0.0).unwrap(), "bass=g=6");
        assert_eq!(
            build_filter(0.0, 0.0, -2.5).unwrap(),
            "treble=g=-2.5"
        );
        assert_eq!(
            build_filter(0.0, 3.0, 4.0).unwrap(),
            "equalizer=f=1000:t=q:w=1:g=3,treble=g=4"
        );
    }

    #[test]
    fn all_zero_is_rejected_as_a_no_op() {
        let err = plan_eq("in.mp3", 0.0, 0.0, 0.0, "mp3").unwrap_err();
        assert!(err.contains("nothing to change"), "{err}");
    }

    #[test]
    fn out_of_range_or_non_finite_gains_are_rejected() {
        assert!(plan_eq("a.mp3", 21.0, 0.0, 0.0, "mp3").is_err());
        assert!(plan_eq("a.mp3", 0.0, -20.5, 0.0, "mp3").is_err());
        assert!(plan_eq("a.mp3", 0.0, 0.0, f64::NAN, "mp3").is_err());
        // Boundaries are valid.
        assert!(plan_eq("a.mp3", 20.0, 0.0, -20.0, "mp3").is_ok());
        let err = plan_eq("a.mp3", 0.0, 25.0, 0.0, "mp3").unwrap_err();
        assert!(err.contains("mid"), "names the offending band: {err}");
    }

    #[test]
    fn every_format_maps_to_its_codec() {
        for (f, codec) in [
            ("mp3", "libmp3lame"),
            ("wav", "pcm_s16le"),
            ("ogg", "libvorbis"),
            ("flac", "flac"),
            ("m4a", "aac"),
        ] {
            let (argv, _) = plan_eq("in.mp3", 6.0, 0.0, 0.0, f).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_eq("in.mp3", 0.0, 0.0, 4.0, "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn parse_format_defaults_empty_to_mp3() {
        assert_eq!(parse_format("").unwrap(), Format::Mp3);
        assert_eq!(parse_format("FLAC").unwrap(), Format::Flac);
        assert!(parse_format("aiff").is_err());
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(6.0), "6");
        assert_eq!(fmt_num(-12.0), "-12");
        assert_eq!(fmt_num(2.5), "2.5");
    }
}
