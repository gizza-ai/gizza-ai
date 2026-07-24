//! gizza-ai/parametric-eq core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! A three-band PARAMETRIC equalizer: unlike a fixed tone control, every band's
//! centre frequency, gain and Q (bandwidth) are user-set. Each active band is a
//! peaking filter (`equalizer=f=<freq>:t=q:w=<q>:g=<gain>`). Bands whose gain is
//! 0 dB are omitted from the chain, and an all-zero request is rejected as a
//! no-op rather than wasting a lossy re-encode on nothing. `-vn` drops
//! attached-picture streams (album art).

/// Output audio formats parametric-eq can write (family-standard set).
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
/// Lowest settable centre frequency (Hz) — bottom of the audible range.
pub const MIN_FREQ_HZ: f64 = 20.0;
/// Highest settable centre frequency (Hz) — top of the audible range.
pub const MAX_FREQ_HZ: f64 = 20000.0;
/// Narrowest Q (widest bell). Below this the band is basically a tilt.
pub const MIN_Q: f64 = 0.1;
/// Widest Q (sharpest bell) — surgical notch/boost territory.
pub const MAX_Q: f64 = 10.0;

/// Default centre frequencies for the three bands (low / mid / high) when a
/// band's frequency is left unset (0). Chosen to span the spectrum sensibly.
pub const DEFAULT_FREQS: [f64; 3] = [100.0, 1000.0, 5000.0];
/// Default Q when a band's Q is left unset (0): a musical one-octave-ish bell.
pub const DEFAULT_Q: f64 = 1.0;

/// One parametric band: centre frequency (Hz), gain (dB) and Q (bandwidth).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    pub freq: f64,
    pub gain: f64,
    pub q: f64,
}

impl Band {
    pub fn new(freq: f64, gain: f64, q: f64) -> Self {
        Band { freq, gain, q }
    }
}

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

/// Validate one active band's parameters. `idx` is 1-based for messages.
fn validate_band(idx: usize, b: Band) -> Result<(), String> {
    if !b.gain.is_finite() || b.gain.abs() > MAX_GAIN_DB {
        return Err(format!(
            "band {idx} gain must be between -{MAX_GAIN_DB} and {MAX_GAIN_DB} dB, got {}",
            b.gain
        ));
    }
    if !b.freq.is_finite() || b.freq < MIN_FREQ_HZ || b.freq > MAX_FREQ_HZ {
        return Err(format!(
            "band {idx} frequency must be between {MIN_FREQ_HZ} and {MAX_FREQ_HZ} Hz, got {}",
            b.freq
        ));
    }
    if !b.q.is_finite() || b.q < MIN_Q || b.q > MAX_Q {
        return Err(format!(
            "band {idx} Q must be between {MIN_Q} and {MAX_Q}, got {}",
            b.q
        ));
    }
    Ok(())
}

/// Build the `-af` chain from the bands whose gain is non-zero, in band order.
/// Only active (non-zero-gain) bands are validated and emitted. Returns an
/// error when every band is 0 dB (a pointless re-encode).
pub fn build_filter(bands: &[Band]) -> Result<String, String> {
    let mut stages = Vec::new();
    for (i, &b) in bands.iter().enumerate() {
        if b.gain == 0.0 {
            continue; // inactive band — frequency/Q irrelevant
        }
        validate_band(i + 1, b)?;
        stages.push(format!(
            "equalizer=f={}:t=q:w={}:g={}",
            fmt_num(b.freq),
            fmt_num(b.q),
            fmt_num(b.gain)
        ));
    }
    if stages.is_empty() {
        return Err(
            "every band is at 0 dB — nothing to change; set a non-zero gain on at least one band \
             (e.g. band 2 gain 4 for vocal presence, band 1 gain -6 to cut boom)"
                .to_string(),
        );
    }
    Ok(stages.join(","))
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to equalize `in_name` into
/// `out_name`. Shared verbatim by the web page (`build_argv`) and the chat block.
pub fn build_argv(in_name: &str, out_name: &str, filter: &str, format: Format) -> Vec<String> {
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

/// Validate the bands, parse `format`, and return `(argv, out_name)`. Single
/// source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
pub fn plan_eq(
    in_name: &str,
    bands: &[Band],
    format: &str,
) -> Result<(Vec<String>, String), String> {
    let filter = build_filter(bands)?;
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, &filter, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(freq: f64, gain: f64, q: f64) -> Band {
        Band::new(freq, gain, q)
    }

    #[test]
    fn full_three_band_argv_order_and_values() {
        let bands = [b(120.0, 6.0, 0.7), b(2500.0, -4.0, 2.0), b(8000.0, 3.0, 1.0)];
        let (argv, out) = plan_eq("in.mp3", &bands, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "equalizer=f=120:t=q:w=0.7:g=6,equalizer=f=2500:t=q:w=2:g=-4,equalizer=f=8000:t=q:w=1:g=3",
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
    fn zero_gain_bands_are_omitted_from_the_chain() {
        // Only band 1 active.
        assert_eq!(
            build_filter(&[b(100.0, 6.0, 1.0), b(1000.0, 0.0, 1.0), b(5000.0, 0.0, 1.0)]).unwrap(),
            "equalizer=f=100:t=q:w=1:g=6"
        );
        // Only band 3 active, negative gain.
        assert_eq!(
            build_filter(&[b(100.0, 0.0, 1.0), b(1000.0, 0.0, 1.0), b(5000.0, -2.5, 3.0)]).unwrap(),
            "equalizer=f=5000:t=q:w=3:g=-2.5"
        );
    }

    #[test]
    fn all_zero_is_rejected_as_a_no_op() {
        let bands = [b(100.0, 0.0, 1.0), b(1000.0, 0.0, 1.0), b(5000.0, 0.0, 1.0)];
        let err = plan_eq("in.mp3", &bands, "mp3").unwrap_err();
        assert!(err.contains("nothing to change"), "{err}");
    }

    #[test]
    fn out_of_range_or_non_finite_params_are_rejected_only_when_band_active() {
        // Bad freq but gain 0 → band ignored, so this is only rejected because
        // it becomes a no-op, not for the freq.
        let noop = plan_eq("a.mp3", &[b(999999.0, 0.0, 1.0)], "mp3").unwrap_err();
        assert!(noop.contains("nothing to change"), "{noop}");
        // Active band with out-of-range freq is rejected and names the band.
        let e = plan_eq("a.mp3", &[b(999999.0, 4.0, 1.0)], "mp3").unwrap_err();
        assert!(e.contains("band 1") && e.contains("frequency"), "{e}");
        // Gain over the cap.
        assert!(plan_eq("a.mp3", &[b(1000.0, 21.0, 1.0)], "mp3").is_err());
        // Q out of range.
        let eq = plan_eq("a.mp3", &[b(1000.0, 4.0, 0.05)], "mp3").unwrap_err();
        assert!(eq.contains("Q"), "{eq}");
        // Non-finite.
        assert!(plan_eq("a.mp3", &[b(1000.0, f64::NAN, 1.0)], "mp3").is_err());
        // Boundaries are valid.
        assert!(plan_eq("a.mp3", &[b(20.0, 20.0, 0.1)], "mp3").is_ok());
        assert!(plan_eq("a.mp3", &[b(20000.0, -20.0, 10.0)], "mp3").is_ok());
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
            let (argv, _) = plan_eq("in.mp3", &[b(1000.0, 6.0, 1.0)], f).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_eq("in.mp3", &[b(5000.0, 4.0, 1.0)], "wav").unwrap();
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
        assert_eq!(fmt_num(0.7), "0.7");
    }
}
