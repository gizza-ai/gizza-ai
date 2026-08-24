//! gizza-ai/aiff-to-wav core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Re-containers an AIFF (Apple/Mac/Pro Tools) file as a RIFF/WAVE `.wav`.
//! Both formats carry uncompressed linear PCM; the only intrinsic difference is
//! byte order — AIFF stores samples big-endian (`pcm_s24be`), WAV little-endian
//! (`pcm_s24le`). At a matching bit depth the decoded sample VALUES come out
//! bit-for-bit identical, so the conversion is lossless.
//!
//! The one real trap: ffmpeg's wav muxer defaults to `pcm_s16le` whenever no
//! codec is given, which silently truncates a 24-bit Pro Tools session to 16
//! bits. This core therefore ALWAYS emits an explicit `-c:a`, and defaults to
//! 24-bit — which preserves every bit of both 16- and 24-bit sources.
//!
//! `-vn` drops any attached-picture (cover-art) stream, which rides as a video
//! stream and would otherwise break the audio-only wav mux.

/// Output PCM encodings, as the `bit_depth` param's accepted values.
pub const BIT_DEPTHS: [&str; 6] = ["16", "24", "32", "float32", "alaw", "mulaw"];
/// 24-bit never truncates a 16- or 24-bit AIFF source (see module docs).
pub const DEFAULT_BIT_DEPTH: &str = "24";

/// Output sample rates, as the `sample_rate` param's accepted values.
/// `keep` omits `-ar` entirely so the source rate passes through untouched.
pub const SAMPLE_RATES: [&str; 9] = [
    "keep", "8000", "16000", "22050", "44100", "48000", "88200", "96000", "192000",
];
/// Pass the source rate through — resampling is never lossless.
pub const DEFAULT_SAMPLE_RATE: &str = "keep";

/// Output channel layouts, as the `channels` param's accepted values.
pub const CHANNELS: [&str; 3] = ["keep", "mono", "stereo"];
/// Pass the source layout through — downmixing is never lossless.
pub const DEFAULT_CHANNELS: &str = "keep";

/// Copy the source's textual tags into the WAV's LIST/INFO chunk by default.
pub const DEFAULT_KEEP_METADATA: bool = true;

/// Map a `bit_depth` value to the ffmpeg encoder that writes it into a WAV.
///
/// `16`/`24`/`32` are linear little-endian integer PCM; `float32` is IEEE
/// 32-bit float (the format most DAWs mix in); `alaw`/`mulaw` are the G.711
/// 8-bit companded encodings used for telephony-grade audio.
pub fn codec_for(bit_depth: &str) -> Result<&'static str, String> {
    match bit_depth {
        "16" => Ok("pcm_s16le"),
        "24" => Ok("pcm_s24le"),
        "32" => Ok("pcm_s32le"),
        "float32" => Ok("pcm_f32le"),
        "alaw" => Ok("pcm_alaw"),
        "mulaw" => Ok("pcm_mulaw"),
        other => Err(format!(
            "bit_depth must be one of {} (got \"{other}\")",
            BIT_DEPTHS.join(", ")
        )),
    }
}

/// Validate `sample_rate` and return the `-ar` value, or `None` for `keep`.
fn resolve_sample_rate(sample_rate: &str) -> Result<Option<&str>, String> {
    if sample_rate == "keep" {
        return Ok(None);
    }
    if SAMPLE_RATES.contains(&sample_rate) {
        return Ok(Some(sample_rate));
    }
    Err(format!(
        "sample_rate must be one of {} (got \"{sample_rate}\")",
        SAMPLE_RATES.join(", ")
    ))
}

/// Validate `channels` and return the `-ac` count, or `None` for `keep`.
fn resolve_channels(channels: &str) -> Result<Option<&'static str>, String> {
    match channels {
        "keep" => Ok(None),
        "mono" => Ok(Some("1")),
        "stereo" => Ok(Some("2")),
        other => Err(format!(
            "channels must be one of {} (got \"{other}\")",
            CHANNELS.join(", ")
        )),
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that converts `in_name` to
/// `out_name` as RIFF/WAVE. Shared verbatim by the web page (`build_argv`) and
/// the chat/CLI block (`run`).
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    bit_depth: &str,
    sample_rate: &str,
    channels: &str,
    keep_metadata: bool,
) -> Result<Vec<String>, String> {
    let codec = codec_for(bit_depth)?;
    let rate = resolve_sample_rate(sample_rate)?;
    let layout = resolve_channels(channels)?;

    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        // Drop any attached-picture (cover-art) stream — audio-only WAV.
        "-vn".to_string(),
        // Keep or clear the source's textual tags (title/artist/album/…).
        "-map_metadata".to_string(),
        if keep_metadata { "0" } else { "-1" }.to_string(),
        // ALWAYS explicit: the wav muxer's default (pcm_s16le) would silently
        // truncate a 24-bit source.
        "-c:a".to_string(),
        codec.to_string(),
    ];
    if let Some(rate) = rate {
        argv.push("-ar".to_string());
        argv.push(rate.to_string());
    }
    if let Some(layout) = layout {
        argv.push("-ac".to_string());
        argv.push(layout.to_string());
    }
    argv.push(out_name.to_string());
    Ok(argv)
}

/// Validate the params and return `(argv, out_name)` for an input file.
/// `out_name` is always `out.wav`. Single source shared by the chat block
/// (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan(
    in_name: &str,
    bit_depth: &str,
    sample_rate: &str,
    channels: &str,
    keep_metadata: bool,
) -> Result<(Vec<String>, String), String> {
    let out_name = "out.wav".to_string();
    let argv = build_argv(
        in_name,
        &out_name,
        bit_depth,
        sample_rate,
        channels,
        keep_metadata,
    )?;
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_emit_explicit_24_bit_and_keep_source_rate_layout_tags() {
        let (argv, out) = plan(
            "in.aiff",
            DEFAULT_BIT_DEPTH,
            DEFAULT_SAMPLE_RATE,
            DEFAULT_CHANNELS,
            DEFAULT_KEEP_METADATA,
        )
        .unwrap();
        assert_eq!(out, "out.wav");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.aiff",
                "-vn",
                "-map_metadata",
                "0",
                "-c:a",
                "pcm_s24le",
                "out.wav",
            ]
        );
        // No -ar/-ac at all: "keep" must not touch rate or layout.
        assert!(!argv.iter().any(|a| a == "-ar" || a == "-ac"));
    }

    #[test]
    fn every_advertised_bit_depth_maps_to_a_wav_capable_encoder() {
        let expected = [
            ("16", "pcm_s16le"),
            ("24", "pcm_s24le"),
            ("32", "pcm_s32le"),
            ("float32", "pcm_f32le"),
            ("alaw", "pcm_alaw"),
            ("mulaw", "pcm_mulaw"),
        ];
        for (value, codec) in expected {
            assert_eq!(codec_for(value).unwrap(), codec, "bit_depth {value}");
        }
        // The advertised list and the mapping must not drift apart.
        for value in BIT_DEPTHS {
            assert!(codec_for(value).is_ok(), "advertised {value} must map");
        }
    }

    #[test]
    fn resample_and_downmix_append_ar_and_ac_after_the_codec() {
        let (argv, _) = plan("in.aif", "16", "48000", "mono", true).unwrap();
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.aif",
                "-vn",
                "-map_metadata",
                "0",
                "-c:a",
                "pcm_s16le",
                "-ar",
                "48000",
                "-ac",
                "1",
                "out.wav",
            ]
        );
    }

    #[test]
    fn stereo_upmix_and_metadata_strip() {
        let (argv, _) = plan("in.aifc", "float32", "96000", "stereo", false).unwrap();
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.aifc",
                "-vn",
                "-map_metadata",
                "-1",
                "-c:a",
                "pcm_f32le",
                "-ar",
                "96000",
                "-ac",
                "2",
                "out.wav",
            ]
        );
    }

    #[test]
    fn every_advertised_sample_rate_and_channel_value_is_accepted() {
        for rate in SAMPLE_RATES {
            let (argv, _) = plan("in.aiff", "24", rate, "keep", true).unwrap();
            if rate == "keep" {
                assert!(!argv.iter().any(|a| a == "-ar"), "keep must omit -ar");
            } else {
                let i = argv.iter().position(|a| a == "-ar").expect("-ar present");
                assert_eq!(argv[i + 1], rate);
            }
        }
        for layout in CHANNELS {
            let (argv, _) = plan("in.aiff", "24", "keep", layout, true).unwrap();
            match layout {
                "keep" => assert!(!argv.iter().any(|a| a == "-ac")),
                "mono" => assert!(argv.windows(2).any(|w| w == ["-ac", "1"])),
                _ => assert!(argv.windows(2).any(|w| w == ["-ac", "2"])),
            }
        }
    }

    #[test]
    fn unknown_bit_depth_is_rejected_with_the_accepted_values() {
        let err = plan("in.aiff", "12", "keep", "keep", true).unwrap_err();
        assert_eq!(
            err,
            "bit_depth must be one of 16, 24, 32, float32, alaw, mulaw (got \"12\")"
        );
    }

    #[test]
    fn unknown_sample_rate_and_channels_are_rejected() {
        let err = plan("in.aiff", "24", "44101", "keep", true).unwrap_err();
        assert!(err.starts_with("sample_rate must be one of keep, 8000,"), "{err}");
        assert!(err.ends_with("(got \"44101\")"), "{err}");

        let err = plan("in.aiff", "24", "keep", "5.1", true).unwrap_err();
        assert_eq!(
            err,
            "channels must be one of keep, mono, stereo (got \"5.1\")"
        );
    }
}
