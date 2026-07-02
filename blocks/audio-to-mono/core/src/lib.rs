//! gizza-ai/audio-to-mono core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Downmixes to a single channel. `mix` uses `-ac 1` (ffmpeg's standard
//! downmix law, works for stereo AND 5.1/7.1 sources); `left`/`right` keep
//! just that channel via `pan=mono|c0=c0` / `pan=mono|c0=c1` — handy when one
//! side of an interview recording is the only usable one. `-vn` drops
//! attached-picture streams (album art).

/// Output audio formats audio-to-mono can write (family-standard set).
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

/// Which source channel(s) end up in the mono output.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Channel {
    /// Standard downmix of all channels (ffmpeg `-ac 1`).
    Mix,
    /// Keep only the left (first) channel.
    Left,
    /// Keep only the right (second) channel.
    Right,
}

/// Parse the user-facing channel string. Empty defaults to mix.
pub fn parse_channel(s: &str) -> Result<Channel, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "mix" => Ok(Channel::Mix),
        "left" => Ok(Channel::Left),
        "right" => Ok(Channel::Right),
        other => Err(format!("channel {other:?} not supported (mix|left|right)")),
    }
}

impl Channel {
    /// The channel-selection argv fragment.
    fn args(self) -> Vec<String> {
        match self {
            Channel::Mix => vec!["-ac".into(), "1".into()],
            Channel::Left => vec!["-af".into(), "pan=mono|c0=c0".into()],
            Channel::Right => vec!["-af".into(), "pan=mono|c0=c1".into()],
        }
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to downmix `in_name` into
/// `out_name`. Shared verbatim by the web page (`build_argv`) and the chat block.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    channel: Channel,
    format: Format,
) -> Vec<String> {
    let mut argv = vec!["-i".to_string(), in_name.to_string(), "-vn".to_string()];
    argv.extend(channel.args());
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Parse `channel`/`format` and return `(argv, out_name)` for an input file.
/// `out_name` is `out.<ext>` for the chosen format. Single source shared by
/// the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_to_mono(
    in_name: &str,
    channel: &str,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    let ch = parse_channel(channel)?;
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, ch, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_argv_uses_ac_1() {
        let (argv, out) = plan_to_mono("in.mp3", "mix", "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-ac",
                "1",
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
    fn left_and_right_use_pan_filters() {
        let (argv, _) = plan_to_mono("in.wav", "left", "wav").unwrap();
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-af" && w[1] == "pan=mono|c0=c0"));
        let (argv, _) = plan_to_mono("in.wav", "right", "wav").unwrap();
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-af" && w[1] == "pan=mono|c0=c1"));
        // pan already produces mono — -ac 1 must not also appear.
        assert!(!argv.windows(2).any(|w| w[0] == "-ac"));
    }

    #[test]
    fn empty_channel_and_format_default_to_mix_mp3() {
        let (argv, out) = plan_to_mono("in.flac", "", "").unwrap();
        assert_eq!(out, "out.mp3");
        assert!(argv.windows(2).any(|w| w[0] == "-ac" && w[1] == "1"));
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
            let (argv, _) = plan_to_mono("in.mp3", "mix", f).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_to_mono("in.mp3", "mix", "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn rejects_unknown_channel_and_format() {
        let err = plan_to_mono("a.mp3", "center", "mp3").unwrap_err();
        assert!(err.contains("mix|left|right"));
        assert!(plan_to_mono("a.mp3", "mix", "aiff").is_err());
    }

    #[test]
    fn channel_parse_is_case_insensitive() {
        assert_eq!(parse_channel("LEFT").unwrap(), Channel::Left);
        assert_eq!(parse_channel("Mix").unwrap(), Channel::Mix);
    }
}
