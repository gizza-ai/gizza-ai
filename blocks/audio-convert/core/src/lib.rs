//! gizza-ai/audio-convert core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Converts audio between mp3, wav, ogg, flac and m4a. `-vn` drops any video
//! stream — audio files with embedded album art carry an attached-picture
//! (video) stream that would otherwise break audio-only muxers like wav.
//! Lossy targets (mp3/ogg/m4a) take a bitrate; lossless (wav/flac) ignore it.

/// Output audio formats audio-convert can write.
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

    /// True for formats where `bitrate` applies (lossy encoders).
    pub fn is_lossy(self) -> bool {
        matches!(self, Format::Mp3 | Format::Ogg | Format::M4a)
    }

    /// Encoder argv fragment (`-c:a …`). Lossy formats append `-b:a <kbps>k`.
    fn codec_args(self, kbps: u32) -> Vec<String> {
        match self {
            Format::Mp3 => vec![
                "-c:a".into(),
                "libmp3lame".into(),
                "-b:a".into(),
                format!("{kbps}k"),
            ],
            Format::Ogg => vec![
                "-c:a".into(),
                "libvorbis".into(),
                "-b:a".into(),
                format!("{kbps}k"),
            ],
            Format::M4a => vec![
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                format!("{kbps}k"),
            ],
            Format::Wav => vec!["-c:a".into(), "pcm_s16le".into()],
            Format::Flac => vec!["-c:a".into(), "flac".into()],
        }
    }
}

/// Parse the user-facing format string (the values the chat schema + page
/// accept). Unlike trim-audio there is NO empty default — converting to an
/// explicit target format is the whole point of this tool.
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "mp3" => Ok(Format::Mp3),
        "wav" => Ok(Format::Wav),
        "ogg" => Ok(Format::Ogg),
        "flac" => Ok(Format::Flac),
        "m4a" => Ok(Format::M4a),
        other => Err(format!(
            "format {other:?} not supported (mp3|wav|ogg|flac|m4a)"
        )),
    }
}

/// Default bitrate (kbps) for lossy targets when none is supplied.
pub const DEFAULT_BITRATE: u32 = 192;

/// Clamp a lossy bitrate to the encoder-sensible 32–320 kbps range.
pub fn clamp_bitrate(kbps: u32) -> u32 {
    kbps.clamp(32, 320)
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to convert `in_name` into
/// `out_name`. Shared verbatim by the web page (`build_argv`) and the chat block.
pub fn build_argv(in_name: &str, out_name: &str, format: Format, kbps: u32) -> Vec<String> {
    let mut argv = vec!["-i".to_string(), in_name.to_string(), "-vn".to_string()];
    argv.extend(format.codec_args(clamp_bitrate(kbps)));
    argv.push(out_name.to_string());
    argv
}

/// Parse `format`, clamp `bitrate`, and return `(argv, out_name)` for an input
/// file. `out_name` is `out.<ext>` for the chosen format. Single source shared
/// by the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_convert(
    in_name: &str,
    format: &str,
    bitrate: u32,
) -> Result<(Vec<String>, String), String> {
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, fmt, bitrate), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mp3_argv_order_and_values() {
        let (argv, out) = plan_convert("in.wav", "mp3", 192).unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.wav",
                "-vn",
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
    fn every_format_maps_to_its_codec_and_out_name() {
        for (f, codec, out) in [
            ("mp3", "libmp3lame", "out.mp3"),
            ("wav", "pcm_s16le", "out.wav"),
            ("ogg", "libvorbis", "out.ogg"),
            ("flac", "flac", "out.flac"),
            ("m4a", "aac", "out.m4a"),
        ] {
            let (argv, out_name) = plan_convert("in.mp3", f, 192).unwrap();
            assert_eq!(out_name, out, "format {f}");
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        // Album-art mp3s carry an attached-picture video stream; -vn is what
        // keeps audio-only muxers (wav especially) from failing on it.
        for f in ["mp3", "wav", "ogg", "flac", "m4a"] {
            let (argv, _) = plan_convert("in.mp3", f, 192).unwrap();
            assert!(argv.iter().any(|a| a == "-vn"), "format {f} missing -vn");
        }
    }

    #[test]
    fn lossless_formats_take_no_bitrate() {
        for f in ["wav", "flac"] {
            let (argv, _) = plan_convert("in.mp3", f, 256).unwrap();
            assert!(
                !argv.iter().any(|a| a == "-b:a"),
                "{f} is lossless, no bitrate"
            );
        }
    }

    #[test]
    fn bitrate_clamps_to_encoder_range() {
        assert_eq!(clamp_bitrate(0), 32);
        assert_eq!(clamp_bitrate(192), 192);
        assert_eq!(clamp_bitrate(9999), 320);
        let (argv, _) = plan_convert("in.wav", "mp3", 9999).unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-b:a" && w[1] == "320k"));
    }

    #[test]
    fn is_lossy_flags_bitrate_formats() {
        assert!(Format::Mp3.is_lossy());
        assert!(Format::Ogg.is_lossy());
        assert!(Format::M4a.is_lossy());
        assert!(!Format::Wav.is_lossy());
        assert!(!Format::Flac.is_lossy());
    }

    #[test]
    fn parse_format_has_no_empty_default() {
        assert!(parse_format("").is_err());
        assert!(parse_format("aiff").is_err());
        assert_eq!(parse_format("FLAC").unwrap(), Format::Flac);
    }

    #[test]
    fn format_ext_and_mime_pairs() {
        assert_eq!(Format::Mp3.mime(), "audio/mpeg");
        assert_eq!(Format::Wav.mime(), "audio/wav");
        assert_eq!(Format::Ogg.mime(), "audio/ogg");
        assert_eq!(Format::Flac.mime(), "audio/flac");
        assert_eq!(Format::M4a.mime(), "audio/mp4");
        assert_eq!(Format::Ogg.ext(), "ogg");
    }
}
