//! gizza-ai/voice-note-converter core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Converts chat voice messages both directions:
//!   * decode an incoming voice note (Opus/OGG from WhatsApp/Telegram/Signal —
//!     or any audio ffmpeg can decode) to `mp3` or `wav`, and
//!   * encode `mp3`/`wav` (or anything) BACK into a real `.opus` voice note
//!     (Opus codec in an Ogg container — the format messaging apps recognise).
//!
//! This is the piece `audio-convert` lacks: it writes mp3/wav/ogg-vorbis/flac/m4a
//! but has NO Opus encoder, and vorbis-in-ogg is not an interchangeable voice
//! note. `-vn` drops any attached-picture (video) stream so audio-only muxers
//! don't choke on cover art. `-ac 1` downmixes to mono (the voice-note standard);
//! when producing Opus mono we pass `-application voip` to tune libopus for speech.

/// Target formats voice-note-converter can write.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    /// Opus codec in an Ogg container (`.opus`) — a real messaging-app voice note.
    Opus,
    /// MP3 (libmp3lame) — plays everywhere.
    Mp3,
    /// WAV (16-bit PCM) — uncompressed, ideal for editing.
    Wav,
}

impl Format {
    /// Lower-cased file extension this format writes (used for `out.<ext>`).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Opus => "opus",
            Format::Mp3 => "mp3",
            Format::Wav => "wav",
        }
    }

    /// IANA media type for the produced file. Opus rides in an Ogg container, so
    /// `audio/ogg` gives the widest `<audio controls>` playback support.
    pub fn mime(self) -> &'static str {
        match self {
            Format::Opus => "audio/ogg",
            Format::Mp3 => "audio/mpeg",
            Format::Wav => "audio/wav",
        }
    }

    /// True for formats where `bitrate` applies (lossy encoders). WAV is PCM.
    pub fn is_lossy(self) -> bool {
        matches!(self, Format::Opus | Format::Mp3)
    }

    /// Sensible default bitrate (kbps) when the caller supplies none. Opus is
    /// efficient, so 32 kbps is already clean for speech; mp3 needs more.
    pub fn default_bitrate(self) -> u32 {
        match self {
            Format::Opus => 32,
            Format::Mp3 => 128,
            Format::Wav => 0,
        }
    }

    /// Clamp a requested bitrate to this encoder's sensible range. Opus accepts
    /// far lower rates than mp3 (WhatsApp voice notes sit near 16 kbps).
    pub fn clamp_bitrate(self, kbps: u32) -> u32 {
        match self {
            Format::Opus => kbps.clamp(6, 256),
            Format::Mp3 => kbps.clamp(32, 320),
            Format::Wav => kbps,
        }
    }

    /// Encoder argv fragment (`-c:a …`). `mono` selects libopus' speech-tuned
    /// `voip` application (vs `audio` for stereo/music). Lossy formats append
    /// `-b:a <kbps>k`; WAV is fixed 16-bit PCM.
    fn codec_args(self, kbps: u32, mono: bool) -> Vec<String> {
        match self {
            Format::Opus => vec![
                "-c:a".into(),
                "libopus".into(),
                "-b:a".into(),
                format!("{kbps}k"),
                "-application".into(),
                if mono { "voip".into() } else { "audio".into() },
            ],
            Format::Mp3 => vec![
                "-c:a".into(),
                "libmp3lame".into(),
                "-b:a".into(),
                format!("{kbps}k"),
            ],
            Format::Wav => vec!["-c:a".into(), "pcm_s16le".into()],
        }
    }
}

/// Parse the user-facing format string (the values the chat schema + page accept).
/// There is NO empty default — choosing an explicit target is the whole point.
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "opus" => Ok(Format::Opus),
        "mp3" => Ok(Format::Mp3),
        "wav" => Ok(Format::Wav),
        other => Err(format!("format {other:?} not supported (opus|mp3|wav)")),
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to convert `in_name` into
/// `out_name`. `-vn` drops video/cover-art streams; `-ac 1` (when `mono`)
/// downmixes to a single channel. Shared verbatim by the web page and chat block.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    format: Format,
    kbps: u32,
    mono: bool,
) -> Vec<String> {
    let mut argv = vec!["-i".to_string(), in_name.to_string(), "-vn".to_string()];
    if mono {
        argv.push("-ac".into());
        argv.push("1".into());
    }
    argv.extend(format.codec_args(format.clamp_bitrate(kbps), mono));
    argv.push(out_name.to_string());
    argv
}

/// Parse `format`, resolve the bitrate (per-format default when `bitrate` is
/// `None`/0, else clamped), and return `(argv, out_name)` for an input file.
/// `out_name` is `out.<ext>`. Single source shared by the chat block + web page.
pub fn plan_convert(
    in_name: &str,
    format: &str,
    bitrate: Option<u32>,
    mono: bool,
) -> Result<(Vec<String>, String), String> {
    let fmt = parse_format(format)?;
    let kbps = match bitrate {
        Some(b) if b > 0 => b,
        _ => fmt.default_bitrate(),
    };
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, fmt, kbps, mono), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_voice_note_argv_order_and_values() {
        // mp3 in → mono opus voice note out (the "…and back" direction).
        let (argv, out) = plan_convert("in.mp3", "opus", Some(24), true).unwrap();
        assert_eq!(out, "out.opus");
        assert_eq!(
            argv,
            [
                "-i",
                "in.mp3",
                "-vn",
                "-ac",
                "1",
                "-c:a",
                "libopus",
                "-b:a",
                "24k",
                "-application",
                "voip",
                "out.opus",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn stereo_opus_uses_audio_application_and_no_ac_flag() {
        let (argv, _) = plan_convert("in.wav", "opus", Some(96), false).unwrap();
        assert!(!argv.iter().any(|a| a == "-ac"), "no downmix when stereo");
        assert!(
            argv.windows(2).any(|w| w[0] == "-application" && w[1] == "audio"),
            "stereo opus uses the audio application"
        );
    }

    #[test]
    fn decode_voice_note_to_mp3_and_wav() {
        let (argv, out) = plan_convert("in.opus", "mp3", None, true).unwrap();
        assert_eq!(out, "out.mp3");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libmp3lame"));
        assert!(argv.windows(2).any(|w| w[0] == "-b:a" && w[1] == "128k"), "mp3 default 128k");

        let (argv, out) = plan_convert("in.ogg", "wav", None, false).unwrap();
        assert_eq!(out, "out.wav");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "pcm_s16le"));
        assert!(!argv.iter().any(|a| a == "-b:a"), "wav takes no bitrate");
    }

    #[test]
    fn argv_always_drops_video_streams() {
        // A voice note may carry a waveform/cover picture; -vn keeps wav/opus
        // muxers from failing on that attached-picture stream.
        for f in ["opus", "mp3", "wav"] {
            let (argv, _) = plan_convert("in.m4a", f, None, true).unwrap();
            assert!(argv.iter().any(|a| a == "-vn"), "format {f} missing -vn");
        }
    }

    #[test]
    fn bitrate_clamps_per_format() {
        assert_eq!(Format::Opus.clamp_bitrate(1), 6);
        assert_eq!(Format::Opus.clamp_bitrate(9999), 256);
        assert_eq!(Format::Mp3.clamp_bitrate(1), 32);
        assert_eq!(Format::Mp3.clamp_bitrate(9999), 320);
        // opus accepts a lower rate than mp3's floor
        let (argv, _) = plan_convert("in.wav", "opus", Some(12), true).unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-b:a" && w[1] == "12k"));
        let (argv, _) = plan_convert("in.wav", "mp3", Some(12), true).unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-b:a" && w[1] == "32k"), "mp3 floored to 32k");
    }

    #[test]
    fn is_lossy_flags_bitrate_formats() {
        assert!(Format::Opus.is_lossy());
        assert!(Format::Mp3.is_lossy());
        assert!(!Format::Wav.is_lossy());
    }

    #[test]
    fn parse_format_has_no_empty_default() {
        assert!(parse_format("").is_err());
        assert!(parse_format("flac").is_err());
        assert_eq!(parse_format("OPUS").unwrap(), Format::Opus);
    }

    #[test]
    fn format_ext_and_mime_pairs() {
        assert_eq!(Format::Opus.ext(), "opus");
        assert_eq!(Format::Opus.mime(), "audio/ogg");
        assert_eq!(Format::Mp3.mime(), "audio/mpeg");
        assert_eq!(Format::Wav.mime(), "audio/wav");
    }
}
