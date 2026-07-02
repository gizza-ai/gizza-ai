//! gizza-ai/audio-volume-adjust core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Changes gain with ffmpeg's `volume` filter — `volume=6dB` (decibels) or
//! `volume=2` (linear factor). An optional `alimiter` stage (on by default)
//! caps peaks at 0 dBFS so boosts don't clip. `-vn` drops attached-picture
//! streams (album art).

/// Output audio formats audio-volume-adjust can write (family-standard set).
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

/// How `amount` is interpreted.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Unit {
    /// Decibels: +6 doubles perceived-ish level, -6 halves it.
    Db,
    /// Linear factor: 2 doubles the amplitude, 0.5 halves it.
    Factor,
}

/// Parse the user-facing unit string. Empty defaults to db.
pub fn parse_unit(s: &str) -> Result<Unit, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "db" => Ok(Unit::Db),
        "factor" => Ok(Unit::Factor),
        other => Err(format!("unit {other:?} not supported (db|factor)")),
    }
}

/// Accepted gain ranges: ±60 dB, or a linear factor in (0, 16].
pub const MAX_DB: f64 = 60.0;
pub const MAX_FACTOR: f64 = 16.0;

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

/// Build the `-af` chain: the `volume` gain stage plus, when `limiter` is on,
/// an `alimiter` stage that caps peaks at 0 dBFS so boosts don't clip.
pub fn build_filter(amount: f64, unit: Unit, limiter: bool) -> String {
    let mut f = match unit {
        Unit::Db => format!("volume={}dB", fmt_num(amount)),
        Unit::Factor => format!("volume={}", fmt_num(amount)),
    };
    if limiter {
        f.push_str(",alimiter=limit=1:level=disabled");
    }
    f
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to re-gain `in_name` into
/// `out_name`. Shared verbatim by the web page (`build_argv`) and the chat block.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    amount: f64,
    unit: Unit,
    limiter: bool,
    format: Format,
) -> Vec<String> {
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        build_filter(amount, unit, limiter),
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate `amount` for the given `unit`, parse everything, and return
/// `(argv, out_name)`. Single source shared by the chat block (`src/lib.rs`)
/// and the web page (`web/src/lib.rs`).
pub fn plan_volume(
    in_name: &str,
    amount: f64,
    unit: &str,
    limiter: bool,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    let u = parse_unit(unit)?;
    match u {
        Unit::Db => {
            if !amount.is_finite() || amount.abs() > MAX_DB {
                return Err(format!(
                    "amount must be between -{MAX_DB} and {MAX_DB} dB, got {amount}"
                ));
            }
            if amount == 0.0 {
                return Err("amount of 0 dB wouldn't change anything — use e.g. 6 to boost or -6 to cut".into());
            }
        }
        Unit::Factor => {
            if !amount.is_finite() || amount <= 0.0 || amount > MAX_FACTOR {
                return Err(format!(
                    "amount must be a factor in (0, {MAX_FACTOR}] (e.g. 2 doubles, 0.5 halves), got {amount}"
                ));
            }
            if amount == 1.0 {
                return Err(
                    "amount factor 1 wouldn't change anything — use e.g. 2 to double or 0.5 to halve"
                        .into(),
                );
            }
        }
    }
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((
        build_argv(in_name, &out_name, amount, u, limiter, fmt),
        out_name,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_argv_order_and_values() {
        let (argv, out) = plan_volume("in.mp3", 6.0, "db", true, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "volume=6dB,alimiter=limit=1:level=disabled",
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
    fn factor_unit_omits_db_suffix() {
        let f = build_filter(2.0, Unit::Factor, false);
        assert_eq!(f, "volume=2");
        let f = build_filter(0.5, Unit::Factor, false);
        assert_eq!(f, "volume=0.5");
    }

    #[test]
    fn limiter_appends_alimiter_stage() {
        assert_eq!(build_filter(-12.0, Unit::Db, false), "volume=-12dB");
        assert_eq!(
            build_filter(-12.0, Unit::Db, true),
            "volume=-12dB,alimiter=limit=1:level=disabled"
        );
    }

    #[test]
    fn negative_db_cuts_are_accepted() {
        let (argv, _) = plan_volume("in.wav", -20.0, "db", false, "wav").unwrap();
        assert!(argv.iter().any(|a| a == "volume=-20dB"));
    }

    #[test]
    fn rejects_no_op_and_out_of_range_amounts() {
        assert!(plan_volume("a.mp3", 0.0, "db", true, "mp3").is_err());
        assert!(plan_volume("a.mp3", 61.0, "db", true, "mp3").is_err());
        assert!(plan_volume("a.mp3", f64::NAN, "db", true, "mp3").is_err());
        assert!(plan_volume("a.mp3", 1.0, "factor", true, "mp3").is_err());
        assert!(plan_volume("a.mp3", 0.0, "factor", true, "mp3").is_err());
        assert!(plan_volume("a.mp3", 17.0, "factor", true, "mp3").is_err());
        let err = plan_volume("a.mp3", 0.0, "db", true, "mp3").unwrap_err();
        assert!(err.contains("wouldn't change anything"));
    }

    #[test]
    fn rejects_unknown_unit_and_format() {
        assert!(plan_volume("a.mp3", 6.0, "percent", true, "mp3").is_err());
        assert!(plan_volume("a.mp3", 6.0, "db", true, "aiff").is_err());
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
            let (argv, _) = plan_volume("in.mp3", 6.0, "db", true, f).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_volume("in.mp3", 6.0, "db", true, "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(6.0), "6");
        assert_eq!(fmt_num(-12.0), "-12");
        assert_eq!(fmt_num(1.5), "1.5");
        assert_eq!(fmt_num(0.25), "0.25");
    }
}
