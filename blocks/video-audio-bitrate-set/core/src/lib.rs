//! gizza-ai/video-audio-bitrate-set core — pure ffmpeg argv construction shared
//! by the chat skill block and the standalone web page. No wafer/wasm-bindgen
//! deps.
//!
//! Re-encodes ONLY the audio track of a video at a chosen constant bitrate
//! (`-c:a aac -b:a <N>k`) to shrink an oversized soundtrack, while the picture
//! is stream-copied (`-c:v copy`, lossless, fast — the video is never touched or
//! degraded). The output keeps the input container; the audio codec is chosen to
//! match it (webm → libopus, everything else → aac). This is distinct from
//! whole-video compressors (they re-encode the picture): here the video stays
//! byte-identical and only the audio bitrate changes.

use gizza_ai_block_utils::ffmpeg::copy_out_ext;

/// Common preset audio bitrates (kbps) surfaced as the fixed enum choices. The
/// page/chat/CLI restrict to these, but `parse_bitrate` accepts any value in the
/// wider valid range so an arbitrary CLI value still works.
pub const PRESET_BITRATES: [u32; 7] = [64, 96, 128, 160, 192, 256, 320];

/// Default target bitrate — a good balance for stereo music/speech.
pub const DEFAULT_BITRATE: u32 = 128;

/// Accepted bitrate bounds (kbps): below 8 is unusable, above 512 defeats the
/// point of shrinking (and exceeds what these lossy codecs sensibly use).
pub const MIN_BITRATE: u32 = 8;
pub const MAX_BITRATE: u32 = 512;

/// Audio encoder for the kept output container. WebM can only hold Opus/Vorbis,
/// so AAC is invalid there; every other container we keep (mp4/mov/m4v/mkv)
/// accepts AAC.
pub fn audio_codec(out_ext: &str) -> &'static str {
    if out_ext.eq_ignore_ascii_case("webm") {
        "libopus"
    } else {
        "aac"
    }
}

/// Parse the user-facing bitrate. Accepts `"128"`, `"128k"`, or `"128kbps"`; an
/// empty string falls back to the default. Validates the kbps range.
pub fn parse_bitrate(s: &str) -> Result<u32, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(DEFAULT_BITRATE);
    }
    let digits = t
        .strip_suffix("kbps")
        .or_else(|| t.strip_suffix('k'))
        .or_else(|| t.strip_suffix('K'))
        .unwrap_or(t)
        .trim();
    let n: u32 = digits
        .parse()
        .map_err(|_| format!("bitrate {s:?} is not a whole number of kbps (e.g. 128)"))?;
    if !(MIN_BITRATE..=MAX_BITRATE).contains(&n) {
        return Err(format!(
            "bitrate must be between {MIN_BITRATE} and {MAX_BITRATE} kbps, got {n}"
        ));
    }
    Ok(n)
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that re-encodes `in_name`'s audio
/// at `kbps` kbps into `out_name`, keeping the picture (`-c:v copy`). Shared
/// verbatim by the web page (`build_argv`) and the chat block.
pub fn build_argv(in_name: &str, out_name: &str, kbps: u32) -> Vec<String> {
    let out_ext = out_name.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    vec![
        "-i".to_string(),
        in_name.to_string(),
        "-c:v".to_string(),
        "copy".to_string(),
        "-c:a".to_string(),
        audio_codec(out_ext).to_string(),
        "-b:a".to_string(),
        format!("{kbps}k"),
        out_name.to_string(),
    ]
}

/// Validate the bitrate, choose the output container, and return
/// `(argv, out_name)`. `out_name` keeps the input container when it can hold a
/// copied video stream; otherwise it is `out.mp4`. Single source shared by the
/// chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan(in_name: &str, bitrate: &str) -> Result<(Vec<String>, String), String> {
    let kbps = parse_bitrate(bitrate)?;
    let out_name = format!("out.{}", copy_out_ext(in_name));
    Ok((build_argv(in_name, &out_name, kbps), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_order_and_values() {
        let (argv, out) = plan("in.mp4", "128").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i", "in.mp4", "-c:v", "copy", "-c:a", "aac", "-b:a", "128k", "out.mp4",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn always_stream_copies_the_video() {
        let (argv, _) = plan("in.mp4", "96").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
        // no -vn: the video track is kept, never re-encoded.
        assert!(!argv.iter().any(|a| a == "-vn"));
        assert!(!argv.iter().any(|a| a == "libx264"));
    }

    #[test]
    fn bitrate_flag_uses_k_suffix() {
        let (argv, _) = plan("in.mp4", "320").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-b:a" && w[1] == "320k"));
    }

    #[test]
    fn empty_bitrate_defaults_to_128() {
        assert_eq!(parse_bitrate(""), Ok(DEFAULT_BITRATE));
        let (argv, _) = plan("in.mp4", "").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-b:a" && w[1] == "128k"));
    }

    #[test]
    fn accepts_k_suffixed_and_bps_forms() {
        assert_eq!(parse_bitrate("192k"), Ok(192));
        assert_eq!(parse_bitrate("192K"), Ok(192));
        assert_eq!(parse_bitrate("192kbps"), Ok(192));
        assert_eq!(parse_bitrate(" 256 "), Ok(256));
    }

    #[test]
    fn webm_input_keeps_webm_and_uses_opus_audio() {
        let (argv, out) = plan("clip.webm", "128").unwrap();
        assert_eq!(out, "out.webm");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libopus"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
    }

    #[test]
    fn container_kept_for_copy_capable_and_falls_back_to_mp4() {
        for ext in ["mp4", "mov", "m4v", "mkv", "webm"] {
            let (_, out) = plan(&format!("clip.{ext}"), "128").unwrap();
            assert_eq!(out, format!("out.{ext}"));
        }
        // Unknown/absent extension → mp4.
        assert_eq!(plan("clip.avi", "128").unwrap().1, "out.mp4");
        assert_eq!(plan("noext", "128").unwrap().1, "out.mp4");
    }

    #[test]
    fn rejects_out_of_range_and_nonnumeric() {
        assert!(parse_bitrate("4").is_err()); // below MIN
        assert!(parse_bitrate("999").is_err()); // above MAX
        assert!(parse_bitrate("high").is_err());
        assert!(parse_bitrate("12.5").is_err()); // not a whole number
        let err = parse_bitrate("999").unwrap_err();
        assert!(err.contains("between"));
        assert!(plan("a.mp4", "0").is_err());
    }

    #[test]
    fn presets_are_within_bounds() {
        for b in PRESET_BITRATES {
            assert!((MIN_BITRATE..=MAX_BITRATE).contains(&b));
            assert_eq!(parse_bitrate(&b.to_string()), Ok(b));
        }
        assert!(PRESET_BITRATES.contains(&DEFAULT_BITRATE));
    }
}
