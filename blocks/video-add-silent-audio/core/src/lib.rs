//! gizza-ai/video-add-silent-audio core — pure ffmpeg argv construction shared
//! by the chat skill block and the standalone web page. No wafer/wasm-bindgen
//! deps.
//!
//! Gives a video that has NO audio stream a track of pure digital silence, so
//! uploaders/editors/players that require an audio stream accept the file. The
//! silence is synthesised inside the filtergraph by `anullsrc` (a filter SOURCE,
//! not a second media input), so this stays a single-input tool:
//!
//! ```text
//! -i in.mp4
//! -filter_complex anullsrc=channel_layout=stereo:sample_rate=48000[silence]
//! -map 0:v [-map 0:a?] -map [silence]
//! -c:v copy -c:a aac -b:a 128k -shortest out.mp4
//! ```
//!
//! The picture is stream-copied (`-c:v copy`) — lossless, fast, byte-identical.
//! `anullsrc` is infinite, so `-shortest` bounds the output to the video's
//! length. The audio codec follows the kept container (webm → Opus, everything
//! else → AAC).

use gizza_ai_block_utils::ffmpeg::copy_out_ext;

/// Channel layouts offered. Silence only needs one channel, but some validators
/// insist on stereo, so both are exposed.
pub const CHANNELS: [&str; 2] = ["mono", "stereo"];
/// Default layout — stereo is what most upload validators expect to see.
pub const DEFAULT_CHANNELS: &str = "stereo";

/// Sample rates offered (Hz). 48 kHz is the video-world standard, 44.1 kHz the
/// CD/editor-friendly one, 22.05 kHz a "make it as small as possible" choice.
pub const SAMPLE_RATES: [&str; 3] = ["22050", "44100", "48000"];
/// Default sample rate — 48 kHz, what video containers normally carry.
pub const DEFAULT_SAMPLE_RATE: &str = "48000";

/// Audio bitrates offered (kbps). Silence compresses to nearly nothing, so even
/// 32 kbps is inaudibly "correct"; 128 is the universally accepted default.
pub const BITRATES: [&str; 5] = ["32", "64", "96", "128", "192"];
/// Default bitrate (kbps).
pub const DEFAULT_BITRATE: &str = "128";

/// What to do with an audio track the input already has.
pub const EXISTING_AUDIO: [&str; 2] = ["replace", "keep"];
/// Default — the output carries exactly ONE audio track (the silence).
pub const DEFAULT_EXISTING_AUDIO: &str = "replace";

/// Opus (the only codec WebM can hold here) encodes at 48 kHz; ffmpeg would
/// silently resample anything else, so the plan pins it for deterministic output.
pub const OPUS_SAMPLE_RATE: &str = "48000";

/// Audio encoder for the kept output container. WebM can only hold Opus/Vorbis,
/// so AAC is invalid there; mp4/mov/m4v/mkv all take AAC.
pub fn audio_codec(out_ext: &str) -> &'static str {
    if out_ext.eq_ignore_ascii_case("webm") {
        "libopus"
    } else {
        "aac"
    }
}

fn pick<'a>(value: &'a str, allowed: &[&'a str], default: &'a str, label: &str) -> Result<&'a str, String> {
    let t = value.trim();
    if t.is_empty() {
        return Ok(default);
    }
    allowed
        .iter()
        .copied()
        .find(|a| a.eq_ignore_ascii_case(t))
        .ok_or_else(|| format!("{label} must be one of {}, got {t:?}", allowed.join(", ")))
}

/// Normalise + validate the channel layout (`mono` | `stereo`).
pub fn parse_channels(s: &str) -> Result<&'static str, String> {
    pick(s, &CHANNELS, DEFAULT_CHANNELS, "channels").map(|v| {
        if v.eq_ignore_ascii_case("mono") {
            "mono"
        } else {
            "stereo"
        }
    })
}

/// Normalise + validate the sample rate in Hz. Accepts a bare `"48000"` or the
/// friendlier `"48k"`/`"48000 Hz"` forms a CLI user might type.
pub fn parse_sample_rate(s: &str) -> Result<&'static str, String> {
    let t = s.trim().trim_end_matches("Hz").trim_end_matches("hz").trim();
    let t = match t.strip_suffix('k').or_else(|| t.strip_suffix('K')) {
        // "48k" / "44.1k" → Hz
        Some(khz) => match khz.trim() {
            "48" => "48000",
            "44.1" | "44" => "44100",
            "22.05" | "22" => "22050",
            other => other,
        },
        None => t,
    };
    pick(t, &SAMPLE_RATES, DEFAULT_SAMPLE_RATE, "sample_rate").map(|v| match v {
        "22050" => "22050",
        "44100" => "44100",
        _ => "48000",
    })
}

/// Normalise + validate the audio bitrate in kbps. Accepts `"128"`, `"128k"`,
/// or `"128kbps"`.
pub fn parse_bitrate(s: &str) -> Result<&'static str, String> {
    let t = s.trim();
    let t = t
        .strip_suffix("kbps")
        .or_else(|| t.strip_suffix('k'))
        .or_else(|| t.strip_suffix('K'))
        .unwrap_or(t)
        .trim();
    pick(t, &BITRATES, DEFAULT_BITRATE, "bitrate").map(|v| match v {
        "32" => "32",
        "64" => "64",
        "96" => "96",
        "192" => "192",
        _ => "128",
    })
}

/// Normalise + validate the existing-audio policy (`replace` | `keep`).
pub fn parse_existing_audio(s: &str) -> Result<&'static str, String> {
    pick(s, &EXISTING_AUDIO, DEFAULT_EXISTING_AUDIO, "existing_audio").map(|v| {
        if v.eq_ignore_ascii_case("keep") {
            "keep"
        } else {
            "replace"
        }
    })
}

/// The `anullsrc` filtergraph that synthesises the silent track.
pub fn filtergraph(channels: &str, sample_rate: &str) -> String {
    format!("anullsrc=channel_layout={channels}:sample_rate={sample_rate}[silence]")
}

/// Validate every option, choose the output container, and return
/// `(argv, out_name)`. `out_name` keeps the input container when it can hold a
/// copied video stream, else falls back to `out.mp4`. Single source shared by
/// the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan(
    in_name: &str,
    channels: &str,
    sample_rate: &str,
    bitrate: &str,
    existing_audio: &str,
) -> Result<(Vec<String>, String), String> {
    let channels = parse_channels(channels)?;
    let requested_rate = parse_sample_rate(sample_rate)?;
    let bitrate = parse_bitrate(bitrate)?;
    let existing_audio = parse_existing_audio(existing_audio)?;

    let out_ext = copy_out_ext(in_name);
    let codec = audio_codec(out_ext);
    // Opus only encodes at 48 kHz — pin it rather than let ffmpeg resample
    // behind the user's back, so the output rate always matches what we report.
    let sample_rate = if codec == "libopus" { OPUS_SAMPLE_RATE } else { requested_rate };
    let out_name = format!("out.{out_ext}");

    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-filter_complex".to_string(),
        filtergraph(channels, sample_rate),
        "-map".to_string(),
        "0:v".to_string(),
    ];
    if existing_audio == "keep" {
        // Optional (`?`) so a video with no audio — the whole point of this
        // tool — still maps cleanly instead of erroring on a missing stream.
        argv.push("-map".to_string());
        argv.push("0:a?".to_string());
    }
    argv.extend(
        [
            "-map",
            "[silence]",
            "-c:v",
            "copy",
            "-c:a",
            codec,
            "-b:a",
            &format!("{bitrate}k"),
            // anullsrc is infinite; bound the output to the video's length.
            "-shortest",
            &out_name,
        ]
        .into_iter()
        .map(String::from),
    );
    Ok((argv, out_name))
}

/// The sample rate the output will actually carry, after the Opus pin. Used by
/// the block to describe the result honestly.
pub fn effective_sample_rate(in_name: &str, sample_rate: &str) -> Result<&'static str, String> {
    let requested = parse_sample_rate(sample_rate)?;
    Ok(if audio_codec(copy_out_ext(in_name)) == "libopus" {
        OPUS_SAMPLE_RATE
    } else {
        requested
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_plan_argv_is_exact() {
        let (argv, out) = plan("in.mp4", "", "", "", "").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp4",
                "-filter_complex",
                "anullsrc=channel_layout=stereo:sample_rate=48000[silence]",
                "-map",
                "0:v",
                "-map",
                "[silence]",
                "-c:v",
                "copy",
                "-c:a",
                "aac",
                "-b:a",
                "128k",
                "-shortest",
                "out.mp4",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn picture_is_always_stream_copied() {
        let (argv, _) = plan("in.mp4", "mono", "44100", "64", "replace").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
        assert!(!argv.iter().any(|a| a == "libx264"));
        // never drops the picture, never drops audio
        assert!(!argv.iter().any(|a| a == "-vn" || a == "-an"));
    }

    #[test]
    fn silence_is_bounded_to_the_video_length() {
        let (argv, _) = plan("in.mp4", "stereo", "48000", "128", "replace").unwrap();
        assert!(argv.iter().any(|a| a == "-shortest"), "anullsrc is infinite: {argv:?}");
    }

    #[test]
    fn channels_and_rate_land_in_the_filtergraph() {
        let (argv, _) = plan("in.mp4", "mono", "22050", "32", "replace").unwrap();
        let f = argv.iter().find(|a| a.starts_with("anullsrc")).unwrap();
        assert_eq!(f, "anullsrc=channel_layout=mono:sample_rate=22050[silence]");
        assert!(argv.windows(2).any(|w| w[0] == "-b:a" && w[1] == "32k"));
    }

    #[test]
    fn replace_maps_only_the_silent_track_keep_also_maps_existing() {
        let (replace, _) = plan("in.mp4", "stereo", "48000", "128", "replace").unwrap();
        assert!(!replace.iter().any(|a| a == "0:a?"), "replace drops existing audio");

        let (keep, _) = plan("in.mp4", "stereo", "48000", "128", "keep").unwrap();
        let map_positions: Vec<&String> = keep
            .windows(2)
            .filter(|w| w[0] == "-map")
            .map(|w| &w[1])
            .collect();
        assert_eq!(map_positions, vec!["0:v", "0:a?", "[silence]"]);
    }

    #[test]
    fn webm_keeps_webm_uses_opus_and_pins_48k() {
        let (argv, out) = plan("clip.webm", "mono", "44100", "96", "replace").unwrap();
        assert_eq!(out, "out.webm");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libopus"));
        let f = argv.iter().find(|a| a.starts_with("anullsrc")).unwrap();
        assert_eq!(f, "anullsrc=channel_layout=mono:sample_rate=48000[silence]");
        assert_eq!(effective_sample_rate("clip.webm", "44100").unwrap(), "48000");
        assert_eq!(effective_sample_rate("clip.mp4", "44100").unwrap(), "44100");
    }

    #[test]
    fn container_kept_for_copy_capable_and_falls_back_to_mp4() {
        for ext in ["mp4", "mov", "m4v", "mkv", "webm"] {
            let (_, out) = plan(&format!("clip.{ext}"), "", "", "", "").unwrap();
            assert_eq!(out, format!("out.{ext}"));
        }
        assert_eq!(plan("clip.avi", "", "", "", "").unwrap().1, "out.mp4");
        assert_eq!(plan("noext", "", "", "", "").unwrap().1, "out.mp4");
    }

    #[test]
    fn accepts_friendly_value_forms() {
        assert_eq!(parse_sample_rate("48k").unwrap(), "48000");
        assert_eq!(parse_sample_rate("44.1k").unwrap(), "44100");
        assert_eq!(parse_sample_rate(" 48000 Hz ").unwrap(), "48000");
        assert_eq!(parse_bitrate("128k").unwrap(), "128");
        assert_eq!(parse_bitrate("192kbps").unwrap(), "192");
        assert_eq!(parse_channels("STEREO").unwrap(), "stereo");
        assert_eq!(parse_existing_audio("Keep").unwrap(), "keep");
    }

    #[test]
    fn rejects_unknown_values_with_a_helpful_message() {
        let err = parse_channels("quad").unwrap_err();
        assert!(err.contains("channels must be one of mono, stereo"), "{err}");
        assert!(parse_sample_rate("96000").is_err());
        assert!(parse_bitrate("512").is_err());
        let err = parse_existing_audio("delete").unwrap_err();
        assert!(err.contains("replace, keep"), "{err}");
        assert!(plan("in.mp4", "surround", "", "", "").is_err());
    }

    #[test]
    fn defaults_are_within_the_advertised_choices() {
        assert!(CHANNELS.contains(&DEFAULT_CHANNELS));
        assert!(SAMPLE_RATES.contains(&DEFAULT_SAMPLE_RATE));
        assert!(BITRATES.contains(&DEFAULT_BITRATE));
        assert!(EXISTING_AUDIO.contains(&DEFAULT_EXISTING_AUDIO));
        for c in CHANNELS {
            assert_eq!(parse_channels(c).unwrap(), c);
        }
        for r in SAMPLE_RATES {
            assert_eq!(parse_sample_rate(r).unwrap(), r);
        }
        for b in BITRATES {
            assert_eq!(parse_bitrate(b).unwrap(), b);
        }
    }
}
