//! gizza-ai/video-audio-to-mono core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Downmixes a VIDEO's audio to one mono channel while the picture is
//! stream-copied (`-c:v copy`, lossless and fast) — the video-preserving
//! sibling of the audio-only `audio-to-mono` block. `mix` uses ffmpeg's
//! standard downmix law (`-ac 1`, correct for stereo AND 5.1/7.1);
//! `left`/`right` keep just that side via `pan=mono|c0=c0` / `pan=mono|c0=c1`,
//! which is the fix for a recording where the mic only reached one channel;
//! `difference` writes L−R (`pan=mono|c0=c0-1*c1`), the side signal, which
//! cancels centred content and exposes out-of-phase material.
//!
//! Halving the channel count also halves what the audio needs, so the bitrate
//! and sample rate are exposed as size knobs. WebM keeps its Opus codec, and
//! libopus only accepts 8/12/16/24/48 kHz — a requested rate is SNAPPED to the
//! nearest supported one there rather than failing the encode.

use gizza_ai_block_utils::ffmpeg::copy_out_ext;

/// Which source channel(s) end up in the mono output.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Channel {
    /// Standard downmix of every channel (ffmpeg `-ac 1`).
    Mix,
    /// Keep only the left (first) channel.
    Left,
    /// Keep only the right (second) channel.
    Right,
    /// The side signal, L−R: centred content cancels out.
    Difference,
}

/// Parse the user-facing channel string. Empty defaults to mix.
pub fn parse_channel(s: &str) -> Result<Channel, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "mix" => Ok(Channel::Mix),
        "left" => Ok(Channel::Left),
        "right" => Ok(Channel::Right),
        "difference" => Ok(Channel::Difference),
        other => Err(format!(
            "channel {other:?} not supported (mix|left|right|difference)"
        )),
    }
}

impl Channel {
    /// The channel-selection argv fragment. `pan` already yields one channel,
    /// so `-ac 1` is only needed for the plain downmix.
    fn args(self) -> Vec<String> {
        match self {
            Channel::Mix => vec!["-ac".into(), "1".into()],
            Channel::Left => vec!["-af".into(), "pan=mono|c0=c0".into()],
            Channel::Right => vec!["-af".into(), "pan=mono|c0=c1".into()],
            Channel::Difference => vec!["-af".into(), "pan=mono|c0=c0-1*c1".into()],
        }
    }
}

/// Accepted audio bitrate range, in kbps. 16 is intelligible speech, 320 is
/// transparent for mono music.
pub const MIN_BITRATE_KBPS: i64 = 16;
pub const MAX_BITRATE_KBPS: i64 = 320;

/// Sample rates offered on top of "keep the source rate".
pub const SAMPLE_RATES: [i64; 5] = [48000, 44100, 32000, 22050, 16000];

/// Rates the libopus encoder (WebM's audio codec here) will accept.
const OPUS_RATES: [i64; 5] = [8000, 12000, 16000, 24000, 48000];

/// Audio encoder for the kept output container. WebM can only hold Opus or
/// Vorbis, so AAC is invalid there; mp4/mov/m4v/mkv all take AAC.
pub fn audio_codec(out_ext: &str) -> &'static str {
    if out_ext.eq_ignore_ascii_case("webm") {
        "libopus"
    } else {
        "aac"
    }
}

/// Snap `rate` to something the chosen encoder can actually open. libopus
/// rejects anything outside [`OPUS_RATES`] (44100 is a hard error, not a
/// warning), so pick the nearest supported rate instead of failing the run.
pub fn effective_sample_rate(rate: i64, codec: &str) -> i64 {
    if codec != "libopus" {
        return rate;
    }
    *OPUS_RATES
        .iter()
        .min_by_key(|r| (*r - rate).abs())
        .expect("OPUS_RATES is non-empty")
}

/// Parse the user-facing sample-rate string. Empty or `keep` means "leave the
/// source rate alone" (`None`).
pub fn parse_sample_rate(s: &str) -> Result<Option<i64>, String> {
    let t = s.trim().to_ascii_lowercase();
    if t.is_empty() || t == "keep" {
        return Ok(None);
    }
    let n: i64 = t.parse().map_err(|_| {
        format!("sample_rate {s:?} is not a number (keep|48000|44100|32000|22050|16000)")
    })?;
    if SAMPLE_RATES.contains(&n) {
        Ok(Some(n))
    } else {
        Err(format!(
            "sample_rate {n} not supported (keep|48000|44100|32000|22050|16000)"
        ))
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that downmixes `in_name`'s audio
/// into `out_name` while copying the picture. Shared verbatim by the web page
/// (`build_argv`) and the chat block.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    channel: Channel,
    bitrate_kbps: i64,
    sample_rate: Option<i64>,
) -> Vec<String> {
    let out_ext = out_name.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let codec = audio_codec(out_ext);
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-c:v".to_string(),
        "copy".to_string(),
    ];
    argv.extend(channel.args());
    argv.push("-c:a".to_string());
    argv.push(codec.to_string());
    argv.push("-b:a".to_string());
    argv.push(format!("{bitrate_kbps}k"));
    if let Some(sr) = sample_rate {
        argv.push("-ar".to_string());
        argv.push(effective_sample_rate(sr, codec).to_string());
    }
    argv.push(out_name.to_string());
    argv
}

/// Validate everything, parse it, and return `(argv, out_name)`. `out_name`
/// keeps the input container when it can hold a copied video stream, else
/// `out.mp4`. Single source shared by the chat block (`src/lib.rs`) and the web
/// page (`web/src/lib.rs`).
pub fn plan(
    in_name: &str,
    channel: &str,
    bitrate_kbps: f64,
    sample_rate: &str,
) -> Result<(Vec<String>, String), String> {
    let ch = parse_channel(channel)?;
    let sr = parse_sample_rate(sample_rate)?;
    if !bitrate_kbps.is_finite() || bitrate_kbps.fract() != 0.0 {
        return Err(format!(
            "bitrate must be a whole number of kbps between {MIN_BITRATE_KBPS} and {MAX_BITRATE_KBPS}, got {bitrate_kbps}"
        ));
    }
    let bitrate = bitrate_kbps as i64;
    if !(MIN_BITRATE_KBPS..=MAX_BITRATE_KBPS).contains(&bitrate) {
        return Err(format!(
            "bitrate must be between {MIN_BITRATE_KBPS} and {MAX_BITRATE_KBPS} kbps, got {bitrate}"
        ));
    }
    let out_name = format!("out.{}", copy_out_ext(in_name));
    Ok((build_argv(in_name, &out_name, ch, bitrate, sr), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_argv_order_and_values() {
        let (argv, out) = plan("in.mp4", "mix", 128.0, "keep").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i", "in.mp4", "-c:v", "copy", "-ac", "1", "-c:a", "aac", "-b:a", "128k",
                "out.mp4",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn left_right_and_difference_use_pan_filters() {
        for (ch, expect) in [
            ("left", "pan=mono|c0=c0"),
            ("right", "pan=mono|c0=c1"),
            ("difference", "pan=mono|c0=c0-1*c1"),
        ] {
            let (argv, _) = plan("in.mp4", ch, 128.0, "keep").unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-af" && w[1] == expect),
                "{ch} should use {expect}"
            );
            // pan already produces mono — -ac must not also appear.
            assert!(!argv.iter().any(|a| a == "-ac"), "{ch} must not set -ac");
        }
    }

    #[test]
    fn always_stream_copies_the_picture() {
        let (argv, _) = plan("in.mp4", "mix", 96.0, "keep").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
        // no -vn: unlike audio-to-mono, the video track is kept.
        assert!(!argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn empty_channel_and_sample_rate_default_to_mix_and_keep() {
        let (argv, _) = plan("in.mp4", "", 128.0, "").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-ac" && w[1] == "1"));
        assert!(!argv.iter().any(|a| a == "-ar"));
    }

    #[test]
    fn sample_rate_emits_ar_after_the_codec() {
        let (argv, _) = plan("in.mp4", "mix", 64.0, "22050").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-ar" && w[1] == "22050"));
        assert!(argv.windows(2).any(|w| w[0] == "-b:a" && w[1] == "64k"));
    }

    #[test]
    fn webm_keeps_webm_and_uses_opus() {
        let (argv, out) = plan("clip.webm", "mix", 128.0, "keep").unwrap();
        assert_eq!(out, "out.webm");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libopus"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
    }

    /// libopus hard-errors on 44100 ("Specified sample rate 44100 is not
    /// supported"), so webm output must snap to the nearest legal rate.
    #[test]
    fn opus_sample_rates_snap_to_supported_values() {
        assert_eq!(effective_sample_rate(44100, "libopus"), 48000);
        assert_eq!(effective_sample_rate(32000, "libopus"), 24000);
        assert_eq!(effective_sample_rate(22050, "libopus"), 24000);
        assert_eq!(effective_sample_rate(16000, "libopus"), 16000);
        assert_eq!(effective_sample_rate(48000, "libopus"), 48000);
        // AAC takes every offered rate verbatim.
        assert_eq!(effective_sample_rate(44100, "aac"), 44100);

        let (argv, _) = plan("clip.webm", "mix", 96.0, "44100").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-ar" && w[1] == "48000"));
        let (argv, _) = plan("clip.mp4", "mix", 96.0, "44100").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-ar" && w[1] == "44100"));
    }

    #[test]
    fn container_kept_for_copy_capable_and_falls_back_to_mp4() {
        for ext in ["mp4", "mov", "m4v", "mkv", "webm"] {
            let (_, out) = plan(&format!("clip.{ext}"), "mix", 128.0, "keep").unwrap();
            assert_eq!(out, format!("out.{ext}"));
        }
        assert_eq!(plan("clip.avi", "mix", 128.0, "keep").unwrap().1, "out.mp4");
        assert_eq!(plan("noext", "mix", 128.0, "keep").unwrap().1, "out.mp4");
    }

    #[test]
    fn rejects_out_of_range_or_fractional_bitrate() {
        assert!(plan("a.mp4", "mix", 15.0, "keep").is_err());
        assert!(plan("a.mp4", "mix", 321.0, "keep").is_err());
        assert!(plan("a.mp4", "mix", 96.5, "keep").is_err());
        assert!(plan("a.mp4", "mix", f64::NAN, "keep").is_err());
        assert!(plan("a.mp4", "mix", 16.0, "keep").is_ok());
        assert!(plan("a.mp4", "mix", 320.0, "keep").is_ok());
    }

    #[test]
    fn rejects_unknown_channel_and_sample_rate() {
        let err = plan("a.mp4", "middle", 128.0, "keep").unwrap_err();
        assert!(err.contains("mix|left|right|difference"), "{err}");
        let err = plan("a.mp4", "mix", 128.0, "96000").unwrap_err();
        assert!(err.contains("not supported"), "{err}");
        assert!(plan("a.mp4", "mix", 128.0, "fast").is_err());
    }
}
