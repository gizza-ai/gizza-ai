//! gizza-ai/wav-to-alac core — pure ffmpeg argv construction shared by the chat
//! block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Encodes a WAV (or any audio ffmpeg can decode) to **ALAC — Apple Lossless —
//! inside an `.m4a` container**, the form iOS / Apple Music import expects.
//! ALAC has no quality knob: the decoded PCM samples are bit-for-bit identical
//! to the source, so the only choices are *which* PCM to encode — bit depth,
//! sample rate, channel count — plus whether the source's textual tags ride
//! along.
//!
//! Flags the plan always emits:
//! - `-vn` drops any attached-picture (cover-art) stream so the audio-only M4A
//!   mux never fails on it.
//! - `-c:a alac` selects the Apple Lossless encoder (never AAC — `.m4a` defaults
//!   to AAC, which would silently make the "lossless" output lossy).
//! - `-movflags +faststart` moves the moov atom to the front so the file starts
//!   playing before it has fully downloaded.
//!
//! Everything else is opt-in: each selector has a `source` pass-through value
//! that omits its flag entirely, which is what keeps the default run a straight
//! lossless re-wrap of the source PCM.

/// Output bit depth for the ALAC stream.
///
/// ffmpeg's ALAC encoder accepts exactly two sample formats — `s16p` (16-bit)
/// and `s32p` (which it tags as 24-bit raw). Omitting `-sample_fmt` lets ffmpeg
/// pick the closest match to the source (16-bit WAV → `s16p`, 24-bit WAV →
/// `s32p`), which is the pass-through behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitDepth {
    /// Follow the source — no `-sample_fmt` flag.
    Source,
    /// 16-bit ALAC (`-sample_fmt s16p`), CD depth.
    Bits16,
    /// 24-bit ALAC (`-sample_fmt s32p`), hi-res depth.
    Bits24,
}

impl BitDepth {
    /// The `-sample_fmt` value, or `None` for the pass-through case.
    pub fn sample_fmt(self) -> Option<&'static str> {
        match self {
            BitDepth::Source => None,
            BitDepth::Bits16 => Some("s16p"),
            BitDepth::Bits24 => Some("s32p"),
        }
    }
}

/// Channel layout of the ALAC stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channels {
    /// Follow the source — no `-ac` flag.
    Source,
    /// Fold down to one channel (`-ac 1`).
    Mono,
    /// Force two channels (`-ac 2`) — the safest layout for Apple playback.
    Stereo,
}

impl Channels {
    /// The `-ac` value, or `None` for the pass-through case.
    pub fn count(self) -> Option<u32> {
        match self {
            Channels::Source => None,
            Channels::Mono => Some(1),
            Channels::Stereo => Some(2),
        }
    }
}

/// Sample rates offered for the ALAC stream: the pass-through plus the
/// music/Apple-Music-lossless family. Telephony rates are deliberately absent —
/// downsampling an archival lossless target to 8 kHz is never the intent.
pub const SAMPLE_RATES: [u32; 6] = [44_100, 48_000, 88_200, 96_000, 176_400, 192_000];

/// Parse the user-facing `bit_depth` value (`source|16|24`).
pub fn parse_bit_depth(s: &str) -> Result<BitDepth, String> {
    match s.trim() {
        "" | "source" => Ok(BitDepth::Source),
        "16" => Ok(BitDepth::Bits16),
        "24" => Ok(BitDepth::Bits24),
        other => Err(format!(
            "bit_depth {other:?} not supported (source|16|24)"
        )),
    }
}

/// Parse the user-facing `channels` value (`source|mono|stereo`).
pub fn parse_channels(s: &str) -> Result<Channels, String> {
    match s.trim() {
        "" | "source" => Ok(Channels::Source),
        "mono" => Ok(Channels::Mono),
        "stereo" => Ok(Channels::Stereo),
        other => Err(format!(
            "channels {other:?} not supported (source|mono|stereo)"
        )),
    }
}

/// Parse the user-facing `sample_rate` value: `source` (or empty) for
/// pass-through, otherwise one of [`SAMPLE_RATES`] in Hz.
pub fn parse_sample_rate(s: &str) -> Result<Option<u32>, String> {
    let s = s.trim();
    if s.is_empty() || s == "source" {
        return Ok(None);
    }
    let hz: u32 = s
        .parse()
        .map_err(|_| format!("sample_rate {s:?} is not a number (source|44100|48000|88200|96000|176400|192000)"))?;
    if SAMPLE_RATES.contains(&hz) {
        Ok(Some(hz))
    } else {
        Err(format!(
            "sample_rate {hz} not supported (source|44100|48000|88200|96000|176400|192000)"
        ))
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that encodes `in_name` to ALAC in
/// `out_name`. Shared verbatim by the web page (`build_argv`) and the chat block
/// (`run`).
///
/// Optional flags are emitted only when the matching selector is not the
/// pass-through value, so the default run is `-i in.wav -vn -c:a alac
/// -map_metadata 0 -movflags +faststart out.m4a`.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    bit_depth: BitDepth,
    sample_rate: Option<u32>,
    channels: Channels,
    keep_metadata: bool,
) -> Vec<String> {
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        // Drop any attached-picture (cover-art) video stream — audio-only M4A.
        "-vn".to_string(),
        // Apple Lossless, never the container's default AAC.
        "-c:a".to_string(),
        "alac".to_string(),
    ];
    if let Some(fmt) = bit_depth.sample_fmt() {
        argv.push("-sample_fmt".to_string());
        argv.push(fmt.to_string());
    }
    if let Some(hz) = sample_rate {
        argv.push("-ar".to_string());
        argv.push(hz.to_string());
    }
    if let Some(n) = channels.count() {
        argv.push("-ac".to_string());
        argv.push(n.to_string());
    }
    // `0` copies the source's textual tags into the M4A; `-1` writes none.
    argv.push("-map_metadata".to_string());
    argv.push(if keep_metadata { "0" } else { "-1" }.to_string());
    // moov atom up front so the .m4a streams/scrubs immediately.
    argv.push("-movflags".to_string());
    argv.push("+faststart".to_string());
    argv.push(out_name.to_string());
    argv
}

/// Parse the user-facing selector strings and return `(argv, out_name)` for an
/// input file. `out_name` is always `out.m4a` (ALAC's container). Single source
/// shared by the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan(
    in_name: &str,
    bit_depth: &str,
    sample_rate: &str,
    channels: &str,
    keep_metadata: bool,
) -> Result<(Vec<String>, String), String> {
    let depth = parse_bit_depth(bit_depth)?;
    let rate = parse_sample_rate(sample_rate)?;
    let chans = parse_channels(channels)?;
    let out_name = "out.m4a".to_string();
    Ok((
        build_argv(in_name, &out_name, depth, rate, chans, keep_metadata),
        out_name,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv_of(argv: &[String]) -> Vec<&str> {
        argv.iter().map(String::as_str).collect()
    }

    #[test]
    fn default_plan_is_a_straight_lossless_rewrap() {
        let (argv, out) = plan("in.wav", "source", "source", "source", true).unwrap();
        assert_eq!(out, "out.m4a");
        assert_eq!(
            argv_of(&argv),
            vec![
                "-i",
                "in.wav",
                "-vn",
                "-c:a",
                "alac",
                "-map_metadata",
                "0",
                "-movflags",
                "+faststart",
                "out.m4a",
            ]
        );
    }

    #[test]
    fn every_selector_flows_into_argv() {
        let (argv, out) = plan("in.wav", "24", "96000", "stereo", true).unwrap();
        assert_eq!(out, "out.m4a");
        assert_eq!(
            argv_of(&argv),
            vec![
                "-i",
                "in.wav",
                "-vn",
                "-c:a",
                "alac",
                "-sample_fmt",
                "s32p",
                "-ar",
                "96000",
                "-ac",
                "2",
                "-map_metadata",
                "0",
                "-movflags",
                "+faststart",
                "out.m4a",
            ]
        );
    }

    #[test]
    fn bit_depth_maps_to_the_two_encoder_sample_formats() {
        assert_eq!(parse_bit_depth("source").unwrap().sample_fmt(), None);
        assert_eq!(parse_bit_depth("").unwrap().sample_fmt(), None);
        assert_eq!(parse_bit_depth("16").unwrap().sample_fmt(), Some("s16p"));
        assert_eq!(parse_bit_depth("24").unwrap().sample_fmt(), Some("s32p"));
        let (argv, _) = plan("in.wav", "16", "source", "source", true).unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-sample_fmt" && w[1] == "s16p"));
    }

    #[test]
    fn channels_map_to_ac_counts() {
        assert_eq!(parse_channels("source").unwrap().count(), None);
        assert_eq!(parse_channels("mono").unwrap().count(), Some(1));
        assert_eq!(parse_channels("stereo").unwrap().count(), Some(2));
        let (argv, _) = plan("in.wav", "source", "source", "mono", true).unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-ac" && w[1] == "1"));
    }

    #[test]
    fn every_advertised_sample_rate_parses_and_reaches_ar() {
        assert_eq!(parse_sample_rate("source").unwrap(), None);
        assert_eq!(parse_sample_rate("").unwrap(), None);
        for hz in SAMPLE_RATES {
            assert_eq!(parse_sample_rate(&hz.to_string()).unwrap(), Some(hz));
            let (argv, _) = plan("in.wav", "source", &hz.to_string(), "source", true).unwrap();
            assert!(
                argv.windows(2)
                    .any(|w| w[0] == "-ar" && w[1] == hz.to_string()),
                "sample rate {hz} missing from argv"
            );
        }
    }

    #[test]
    fn metadata_toggle_flips_map_metadata() {
        let (kept, _) = plan("in.wav", "source", "source", "source", true).unwrap();
        assert!(kept.windows(2).any(|w| w[0] == "-map_metadata" && w[1] == "0"));
        let (dropped, _) = plan("in.wav", "source", "source", "source", false).unwrap();
        assert!(dropped
            .windows(2)
            .any(|w| w[0] == "-map_metadata" && w[1] == "-1"));
    }

    #[test]
    fn always_alac_never_aac_and_always_faststart() {
        // `.m4a` defaults to AAC in ffmpeg — the encoder must be pinned, or the
        // "lossless" output would quietly be lossy.
        for depth in ["source", "16", "24"] {
            let (argv, _) = plan("in.wav", depth, "source", "source", true).unwrap();
            assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "alac"));
            assert!(!argv.iter().any(|a| a == "aac"));
            assert!(argv.iter().any(|a| a == "-vn"), "missing -vn");
            assert!(argv
                .windows(2)
                .any(|w| w[0] == "-movflags" && w[1] == "+faststart"));
        }
    }

    #[test]
    fn unsupported_bit_depth_is_an_error() {
        let err = plan("in.wav", "32", "source", "source", true).unwrap_err();
        assert!(err.contains("bit_depth"), "unexpected error: {err}");
        assert!(err.contains("source|16|24"), "error must list the choices: {err}");
    }

    #[test]
    fn unsupported_sample_rate_and_channels_are_errors() {
        let err = plan("in.wav", "source", "8000", "source", true).unwrap_err();
        assert!(err.contains("sample_rate 8000 not supported"), "got: {err}");
        let err = plan("in.wav", "source", "fast", "source", true).unwrap_err();
        assert!(err.contains("is not a number"), "got: {err}");
        let err = plan("in.wav", "source", "source", "surround", true).unwrap_err();
        assert!(err.contains("channels"), "got: {err}");
    }
}
