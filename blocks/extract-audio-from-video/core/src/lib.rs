//! gizza-ai/extract-audio-from-video core — pure ffmpeg argv construction shared
//! by the chat skill block and the standalone web page. No wafer/wasm-bindgen
//! deps.
//!
//! Pulls the audio track out of a video and writes it as MP3 (lossy, libmp3lame
//! at a chosen bitrate) or WAV (lossless 16-bit PCM). `-vn` drops the video
//! stream so only audio is encoded.

/// Output audio format extract-audio-from-video can write.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Mp3,
    Wav,
}

impl Format {
    /// Lower-cased file extension this format writes (used for `out.<ext>`).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Mp3 => "mp3",
            Format::Wav => "wav",
        }
    }

    /// IANA media type for the produced file.
    pub fn mime(self) -> &'static str {
        match self {
            Format::Mp3 => "audio/mpeg",
            Format::Wav => "audio/wav",
        }
    }
}

/// Parse the user-facing format string (the values the chat schema + page accept).
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "mp3" => Ok(Format::Mp3),
        "wav" => Ok(Format::Wav),
        other => Err(format!("format {other:?} not supported (mp3|wav)")),
    }
}

/// Default MP3 bitrate (kbps) when none is supplied.
pub const DEFAULT_BITRATE: u32 = 192;

/// Clamp an MP3 bitrate to the libmp3lame-sensible 32–320 kbps range.
pub fn clamp_bitrate(kbps: u32) -> u32 {
    kbps.clamp(32, 320)
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to extract audio from `in_name`
/// into `out_name`. `-vn` strips video. MP3 → libmp3lame at `bitrate` kbps; WAV
/// → 16-bit little-endian PCM (`bitrate` is ignored for lossless WAV).
pub fn build_argv(in_name: &str, out_name: &str, format: Format, bitrate: u32) -> Vec<String> {
    match format {
        Format::Mp3 => vec![
            "-i".into(),
            in_name.into(),
            "-vn".into(),
            "-c:a".into(),
            "libmp3lame".into(),
            "-b:a".into(),
            format!("{}k", clamp_bitrate(bitrate)),
            out_name.into(),
        ],
        Format::Wav => vec![
            "-i".into(),
            in_name.into(),
            "-vn".into(),
            "-c:a".into(),
            "pcm_s16le".into(),
            out_name.into(),
        ],
    }
}

/// Parse `format`, clamp `bitrate`, and build `(argv, out_name)` for an input
/// file. `out_name` uses the chosen format's extension. Single source shared by
/// the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_extract(
    format: &str,
    bitrate: u32,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, fmt, bitrate), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_format_known_and_default() {
        assert_eq!(parse_format("mp3").unwrap(), Format::Mp3);
        assert_eq!(parse_format("WAV").unwrap(), Format::Wav);
        assert_eq!(parse_format("").unwrap(), Format::Mp3); // default
    }

    #[test]
    fn parse_format_rejects_unknown() {
        assert!(parse_format("ogg").is_err());
        assert!(parse_format("flac").is_err());
    }

    #[test]
    fn format_ext_and_mime() {
        assert_eq!(Format::Mp3.ext(), "mp3");
        assert_eq!(Format::Mp3.mime(), "audio/mpeg");
        assert_eq!(Format::Wav.ext(), "wav");
        assert_eq!(Format::Wav.mime(), "audio/wav");
    }

    #[test]
    fn bitrate_clamps() {
        assert_eq!(clamp_bitrate(0), 32);
        assert_eq!(clamp_bitrate(192), 192);
        assert_eq!(clamp_bitrate(9999), 320);
    }

    #[test]
    fn argv_mp3_drops_video_and_sets_bitrate() {
        let argv = build_argv("in.mp4", "out.mp3", Format::Mp3, 192);
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "in.mp4");
        assert!(argv.iter().any(|a| a == "-vn"), "must drop video");
        assert!(argv.iter().any(|a| a == "libmp3lame"));
        assert!(argv.windows(2).any(|w| w[0] == "-b:a" && w[1] == "192k"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp3"));
    }

    #[test]
    fn argv_wav_uses_pcm_and_ignores_bitrate() {
        let argv = build_argv("in.mkv", "out.wav", Format::Wav, 192);
        assert!(argv.iter().any(|a| a == "-vn"));
        assert!(argv.iter().any(|a| a == "pcm_s16le"));
        assert!(!argv.iter().any(|a| a == "-b:a"), "wav is lossless, no bitrate");
        assert_eq!(argv.last().map(String::as_str), Some("out.wav"));
    }

    #[test]
    fn plan_extract_uses_format_extension() {
        let (_argv, out) = plan_extract("mp3", 192, "clip.mp4").unwrap();
        assert_eq!(out, "out.mp3");
        let (_argv, out) = plan_extract("wav", 192, "clip.webm").unwrap();
        assert_eq!(out, "out.wav");
    }

    #[test]
    fn plan_extract_rejects_bad_format() {
        assert!(plan_extract("ogg", 192, "a.mp4").is_err());
    }
}
