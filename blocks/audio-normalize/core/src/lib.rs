//! gizza-ai/audio-normalize core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Levels audio to a target integrated loudness with ffmpeg's single-pass
//! `loudnorm` (EBU R128). True peak is fixed at -1.5 dBTP and loudness range
//! at 11 LU — the values every streaming/podcast guide recommends — so the one
//! exposed knob is the target LUFS. `-vn` drops attached-picture streams
//! (album art) that break audio-only muxers.

/// Output audio formats audio-normalize can write (family-standard set).
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

/// Default target loudness: -14 LUFS (Spotify / YouTube). Podcasts commonly
/// use -16, EU broadcast -23.
pub const DEFAULT_LUFS: f64 = -14.0;

/// loudnorm's accepted integrated-loudness range.
pub const MIN_LUFS: f64 = -70.0;
pub const MAX_LUFS: f64 = -5.0;

/// Build the `loudnorm` filter string for a target LUFS (TP/LRA fixed).
pub fn build_filter(lufs: f64) -> String {
    format!("loudnorm=I={lufs}:TP=-1.5:LRA=11")
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to normalize `in_name` into
/// `out_name`. Shared verbatim by the web page (`build_argv`) and the chat block.
pub fn build_argv(in_name: &str, out_name: &str, lufs: f64, format: Format) -> Vec<String> {
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        build_filter(lufs),
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate `lufs`, parse `format`, and return `(argv, out_name)` for an input
/// file. `out_name` is `out.<ext>` for the chosen format. Single source shared
/// by the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_normalize(
    in_name: &str,
    lufs: f64,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    if !lufs.is_finite() || !(MIN_LUFS..=MAX_LUFS).contains(&lufs) {
        return Err(format!(
            "lufs must be between {MIN_LUFS} and {MAX_LUFS} (e.g. -14 for streaming, -16 for podcasts), got {lufs}"
        ));
    }
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, lufs, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_order_and_values() {
        let (argv, out) = plan_normalize("in.mp3", -14.0, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "loudnorm=I=-14:TP=-1.5:LRA=11",
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
    fn podcast_and_broadcast_targets_render_in_filter() {
        assert_eq!(build_filter(-16.0), "loudnorm=I=-16:TP=-1.5:LRA=11");
        assert_eq!(build_filter(-23.0), "loudnorm=I=-23:TP=-1.5:LRA=11");
    }

    #[test]
    fn every_format_maps_to_its_codec_and_out_name() {
        for (f, codec, out) in [
            ("mp3", "libmp3lame", "out.mp3"),
            ("wav", "pcm_s16le", "out.wav"),
            ("ogg", "libvorbis", "out.ogg"),
            ("flac", "flac", "out.flac"),
            ("m4a", "aac", "out.m4a"),
        ] {
            let (argv, out_name) = plan_normalize("in.mp3", -14.0, f).unwrap();
            assert_eq!(out_name, out, "format {f}");
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_normalize("in.mp3", -14.0, "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn empty_format_defaults_to_mp3() {
        let (_, out) = plan_normalize("in.ogg", -16.0, "").unwrap();
        assert_eq!(out, "out.mp3");
    }

    #[test]
    fn rejects_lufs_outside_loudnorm_range() {
        assert!(plan_normalize("a.mp3", -4.0, "mp3").is_err());
        assert!(plan_normalize("a.mp3", -71.0, "mp3").is_err());
        assert!(plan_normalize("a.mp3", 0.0, "mp3").is_err());
        assert!(plan_normalize("a.mp3", f64::NAN, "mp3").is_err());
        let err = plan_normalize("a.mp3", 5.0, "mp3").unwrap_err();
        assert!(err.contains("-14 for streaming"));
    }

    #[test]
    fn boundary_lufs_values_accepted() {
        assert!(plan_normalize("a.mp3", -70.0, "mp3").is_ok());
        assert!(plan_normalize("a.mp3", -5.0, "mp3").is_ok());
    }

    #[test]
    fn rejects_unknown_format() {
        assert!(plan_normalize("a.mp3", -14.0, "aiff").is_err());
    }
}
