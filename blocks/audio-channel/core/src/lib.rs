//! gizza-ai/audio-channel core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Channel router for stereo/mono audio. Five operations built from a single
//! ffmpeg pass:
//!   - `swap`   — exchange the left/right channels of a stereo file
//!                (`pan=stereo|c0=c1|c1=c0`).
//!   - `mono`   — downmix every channel to a single mono channel (`-ac 1`,
//!                ffmpeg's standard fold-down, stereo and 5.1/7.1 alike).
//!   - `stereo` — up-mix a mono file to two identical channels (`-ac 2`);
//!                a source that is already stereo passes through.
//!   - `left`   — copy the LEFT channel onto BOTH sides so a one-sided
//!                recording is audible in both ears (`pan=stereo|c0=c0|c1=c0`).
//!   - `right`  — copy the RIGHT channel onto both sides
//!                (`pan=stereo|c0=c1|c1=c1`).
//! `-vn` drops attached-picture streams (album art).

/// Output audio formats audio-channel can write (family-standard set).
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

/// The channel operation to perform.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Operation {
    /// Exchange the left and right channels (stereo output).
    Swap,
    /// Downmix all channels to a single mono channel.
    Mono,
    /// Up-mix a mono source to two identical stereo channels.
    Stereo,
    /// Copy the left channel onto both output channels.
    Left,
    /// Copy the right channel onto both output channels.
    Right,
}

/// Parse the user-facing operation string. Empty defaults to swap.
pub fn parse_operation(s: &str) -> Result<Operation, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "swap" => Ok(Operation::Swap),
        "mono" => Ok(Operation::Mono),
        "stereo" => Ok(Operation::Stereo),
        "left" => Ok(Operation::Left),
        "right" => Ok(Operation::Right),
        other => Err(format!(
            "operation {other:?} not supported (swap|mono|stereo|left|right)"
        )),
    }
}

impl Operation {
    /// The channel-routing argv fragment for this operation.
    fn args(self) -> Vec<String> {
        match self {
            Operation::Swap => vec!["-af".into(), "pan=stereo|c0=c1|c1=c0".into()],
            Operation::Mono => vec!["-ac".into(), "1".into()],
            Operation::Stereo => vec!["-ac".into(), "2".into()],
            Operation::Left => vec!["-af".into(), "pan=stereo|c0=c0|c1=c0".into()],
            Operation::Right => vec!["-af".into(), "pan=stereo|c0=c1|c1=c1".into()],
        }
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that routes the channels of
/// `in_name` into `out_name`. Shared verbatim by the web page (`build_argv`)
/// and the chat block.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    op: Operation,
    format: Format,
) -> Vec<String> {
    let mut argv = vec!["-i".to_string(), in_name.to_string(), "-vn".to_string()];
    argv.extend(op.args());
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Parse `operation`/`format` and return `(argv, out_name)` for an input file.
/// `out_name` is `out.<ext>` for the chosen format. Single source shared by
/// the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_channels(
    in_name: &str,
    operation: &str,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    let op = parse_operation(operation)?;
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, op, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swap_uses_pan_exchange_filter() {
        let (argv, out) = plan_channels("in.mp3", "swap", "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "pan=stereo|c0=c1|c1=c0",
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
    fn mono_uses_ac_1_and_stereo_uses_ac_2() {
        let (argv, _) = plan_channels("in.wav", "mono", "wav").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-ac" && w[1] == "1"));
        assert!(!argv.iter().any(|a| a == "pan=stereo|c0=c1|c1=c0"));
        let (argv, _) = plan_channels("in.wav", "stereo", "wav").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-ac" && w[1] == "2"));
    }

    #[test]
    fn left_and_right_copy_one_side_to_both() {
        let (argv, _) = plan_channels("in.wav", "left", "wav").unwrap();
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-af" && w[1] == "pan=stereo|c0=c0|c1=c0"));
        let (argv, _) = plan_channels("in.wav", "right", "wav").unwrap();
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-af" && w[1] == "pan=stereo|c0=c1|c1=c1"));
        // pan sets the layout — -ac must not also appear.
        assert!(!argv.iter().any(|a| a == "-ac"));
    }

    #[test]
    fn empty_operation_and_format_default_to_swap_mp3() {
        let (argv, out) = plan_channels("in.flac", "", "").unwrap();
        assert_eq!(out, "out.mp3");
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-af" && w[1] == "pan=stereo|c0=c1|c1=c0"));
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
            let (argv, _) = plan_channels("in.mp3", "swap", f).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_channels("in.mp3", "swap", "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn rejects_unknown_operation_and_format() {
        let err = plan_channels("a.mp3", "invert", "mp3").unwrap_err();
        assert!(err.contains("swap|mono|stereo|left|right"));
        assert!(plan_channels("a.mp3", "swap", "aiff").is_err());
    }

    #[test]
    fn operation_parse_is_case_insensitive() {
        assert_eq!(parse_operation("LEFT").unwrap(), Operation::Left);
        assert_eq!(parse_operation("Swap").unwrap(), Operation::Swap);
    }
}
