//! gizza-ai/aiff-to-flac core — pure ffmpeg argv construction shared by the chat
//! block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Re-encodes an AIFF (or any audio ffmpeg can decode) to **FLAC**, a lossless
//! compressed codec. The decoded PCM samples are bit-for-bit identical to the
//! source at every compression level — FLAC only trades encode time for a
//! smaller file, never audio fidelity. `-map_metadata 0` carries the source's
//! textual tags (title/artist/album/year/…) into the FLAC's Vorbis comments;
//! `-vn` drops any attached-picture (cover-art) stream so the audio-only FLAC
//! mux never fails on it.

/// Default FLAC compression level (ffmpeg's own default). A good balance of
/// speed and size; higher levels shrink the file a little more for more CPU.
pub const DEFAULT_COMPRESSION_LEVEL: u32 = 5;

/// Highest compression level ffmpeg's FLAC encoder accepts.
pub const MAX_COMPRESSION_LEVEL: u32 = 12;

/// Clamp a requested compression level into the encoder-valid 0..=12 range.
pub fn clamp_level(level: u32) -> u32 {
    level.min(MAX_COMPRESSION_LEVEL)
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that losslessly transcodes
/// `in_name` to `out.flac` at the given compression level. Shared verbatim by
/// the web page (`build_argv`) and the chat block (`run`).
///
/// The samples are identical regardless of `compression_level`; the flag only
/// affects the encoder's search effort (size vs. speed).
pub fn build_argv(in_name: &str, out_name: &str, level: u32) -> Vec<String> {
    vec![
        "-i".to_string(),
        in_name.to_string(),
        // Drop any attached-picture (cover-art) video stream — audio-only FLAC.
        "-vn".to_string(),
        // Copy the source container's textual metadata tags into the FLAC.
        "-map_metadata".to_string(),
        "0".to_string(),
        "-c:a".to_string(),
        "flac".to_string(),
        "-compression_level".to_string(),
        clamp_level(level).to_string(),
        out_name.to_string(),
    ]
}

/// Clamp `compression_level` and return `(argv, out_name)` for an input file.
/// `out_name` is always `out.flac`. Single source shared by the chat block
/// (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan(in_name: &str, compression_level: u32) -> Result<(Vec<String>, String), String> {
    let out_name = "out.flac".to_string();
    Ok((build_argv(in_name, &out_name, compression_level), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_order_and_values() {
        let (argv, out) = plan("in.aiff", 5).unwrap();
        assert_eq!(out, "out.flac");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.aiff",
                "-vn",
                "-map_metadata",
                "0",
                "-c:a",
                "flac",
                "-compression_level",
                "5",
                "out.flac",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn always_uses_flac_encoder_and_flac_out_name() {
        let (argv, out) = plan("in.aiff", 8).unwrap();
        assert_eq!(out, "out.flac");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "flac"));
    }

    #[test]
    fn always_preserves_tags_and_drops_cover_art() {
        // -map_metadata 0 carries textual tags; -vn keeps cover-art (a video
        // stream) from breaking the audio-only FLAC mux.
        let (argv, _) = plan("in.aiff", 0).unwrap();
        assert!(argv.iter().any(|a| a == "-vn"), "missing -vn");
        assert!(
            argv.windows(2).any(|w| w[0] == "-map_metadata" && w[1] == "0"),
            "missing -map_metadata 0"
        );
    }

    #[test]
    fn compression_level_flows_into_argv() {
        let (argv, _) = plan("in.aiff", 0).unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-compression_level" && w[1] == "0"));
        let (argv, _) = plan("in.aiff", 12).unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-compression_level" && w[1] == "12"));
    }

    #[test]
    fn compression_level_clamps_to_encoder_range() {
        assert_eq!(clamp_level(0), 0);
        assert_eq!(clamp_level(5), 5);
        assert_eq!(clamp_level(12), 12);
        assert_eq!(clamp_level(99), 12);
        let (argv, _) = plan("in.aiff", 99).unwrap();
        assert!(
            argv.windows(2).any(|w| w[0] == "-compression_level" && w[1] == "12"),
            "over-range level must clamp to 12"
        );
    }

    #[test]
    fn defaults_are_sane() {
        assert_eq!(DEFAULT_COMPRESSION_LEVEL, 5);
        assert_eq!(MAX_COMPRESSION_LEVEL, 12);
    }
}
