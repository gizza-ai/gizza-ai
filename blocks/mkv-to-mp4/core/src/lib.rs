//! gizza-ai/mkv-to-mp4 core — pure ffmpeg argv construction shared by the chat
//! block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! mkv-to-mp4 converts a Matroska `.mkv` into an `.mp4` container. When the MKV's
//! tracks are already MP4-legal (H.264/HEVC video + AAC audio) the packets can be
//! *stream-copied* into the new container with `-c copy`: no re-encode, no quality
//! change, near-instant. MKV is a superset container, so it commonly carries
//! codecs MP4 can't hold — VP8/VP9/AV1 video, FLAC/Vorbis/Opus/PCM audio, soft
//! subtitles — and those need a real transcode to H.264/AAC, which the
//! `transcode` mode does as an explicit fallback.
//!
//! This is deliberately distinct from video-transcode / video-compress, which
//! ALWAYS re-encode with libx264 (lossy, slow); the headline here is the lossless
//! `-c copy` remux for the compatible-codec case.

/// How mkv-to-mp4 produces the MP4: lossless container remux or a full re-encode.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    /// `-c copy`: stream-copy every MP4-legal track into the MP4 container.
    /// Lossless and fast; requires the MKV's codecs to be MP4-legal
    /// (H.264/HEVC video + AAC audio).
    Copy,
    /// `-c:v libx264 -c:a aac`: re-encode to H.264/AAC. Always produces a valid
    /// MP4 (use for VP9/AV1 video, FLAC/Vorbis/Opus audio, and other non-MP4-legal
    /// MKV codecs); lossy + slower.
    Transcode,
}

/// Parse the user-facing mode string (the values the chat schema + page accept).
pub fn parse_mode(s: &str) -> Result<Mode, String> {
    match s {
        "copy" => Ok(Mode::Copy),
        "transcode" => Ok(Mode::Transcode),
        other => Err(format!("mode {other:?} not supported (copy|transcode)")),
    }
}

/// Default transcode quality used when none is supplied. Only affects `transcode`.
pub const DEFAULT_QUALITY: u8 = 75;

/// Lowest CRF the quality slider maps to (`quality = 100`). 18 is "visually
/// lossless" for libx264 — deliberately NOT 0 (true-lossless), which produces
/// enormous files that blow past the tool's output-size cap for even short
/// clips. Only meaningful in `transcode` mode.
pub const MIN_CRF: f32 = 18.0;
/// Highest CRF the quality slider maps to (`quality = 1`) — low quality, small
/// file. Above ~40 libx264 output degrades sharply, so the range stops here.
pub const MAX_CRF: f32 = 40.0;

/// Map web-conventional quality 1-100 to a practical ffmpeg libx264 CRF, high
/// quality → low CRF: `quality = 100` → CRF 18 (visually lossless), `quality =
/// 1` → CRF 40 (small, low quality). Only meaningful in `transcode` mode.
pub fn quality_to_crf(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let crf = MAX_CRF - (q - 1.0) * (MAX_CRF - MIN_CRF) / 99.0;
    crf.round().clamp(MIN_CRF, MAX_CRF) as u8
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to convert `in_name` → `out_name`
/// as MP4. `copy` stream-copies (`-map 0:v? -map 0:a? -c copy`); `transcode`
/// re-encodes to libx264/aac at `crf`. Both set `-movflags +faststart` so the
/// moov atom is at the front for progressive web playback.
///
/// Both modes select only the video + audio streams (`-map 0:v? -map 0:a?`) and
/// drop subtitles/data/attachments, because MP4 can't legally hold most MKV
/// subtitle (SRT/ASS/PGS) and attachment (font) tracks — including them makes an
/// otherwise-convertible file error out. `?` marks each map optional so a
/// video-only or audio-only MKV still converts.
pub fn build_argv(in_name: &str, out_name: &str, mode: Mode, crf: u8) -> Vec<String> {
    match mode {
        Mode::Copy => vec![
            "-i".into(),
            in_name.into(),
            "-map".into(),
            "0:v?".into(),
            "-map".into(),
            "0:a?".into(),
            "-c".into(),
            "copy".into(),
            "-movflags".into(),
            "+faststart".into(),
            out_name.into(),
        ],
        Mode::Transcode => vec![
            "-i".into(),
            in_name.into(),
            "-map".into(),
            "0:v?".into(),
            "-map".into(),
            "0:a?".into(),
            "-c:v".into(),
            "libx264".into(),
            "-crf".into(),
            crf.to_string(),
            "-c:a".into(),
            "aac".into(),
            "-movflags".into(),
            "+faststart".into(),
            out_name.into(),
        ],
    }
}

/// Validate `quality`, parse `mode`, and build `(argv, out_name)` for an input
/// file. `out_name` is always `out.mp4`. Single source shared by the chat block
/// (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan(mode: &str, quality: u8, in_name: &str) -> Result<(Vec<String>, String), String> {
    if !(1..=100).contains(&quality) {
        return Err(format!("quality must be 1-100, got {quality}"));
    }
    let m = parse_mode(mode)?;
    let crf = quality_to_crf(quality);
    let out_name = "out.mp4".to_string();
    Ok((build_argv(in_name, &out_name, m, crf), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mode_known() {
        assert_eq!(parse_mode("copy").unwrap(), Mode::Copy);
        assert_eq!(parse_mode("transcode").unwrap(), Mode::Transcode);
    }

    #[test]
    fn parse_mode_rejects_unknown() {
        assert!(parse_mode("remux").is_err());
        assert!(parse_mode("webm").is_err());
    }

    #[test]
    fn copy_mode_stream_copies_no_reencode() {
        let (argv, out) = plan("copy", DEFAULT_QUALITY, "in.mkv").unwrap();
        assert_eq!(out, "out.mp4");
        // Lossless remux: `-c copy`, no libx264, faststart for web playback.
        assert!(
            argv.windows(2).any(|w| w[0] == "-c" && w[1] == "copy"),
            "copy mode must stream-copy: {argv:?}"
        );
        assert!(
            !argv.iter().any(|a| a == "libx264"),
            "copy must not re-encode"
        );
        // Video + audio selected, subtitles/attachments dropped (MP4-illegal).
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "0:v?"));
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "0:a?"));
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-movflags" && w[1] == "+faststart"));
        assert_eq!(argv.first().map(String::as_str), Some("-i"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn transcode_mode_reencodes_h264_aac() {
        let (argv, _) = plan("transcode", 75, "in.mkv").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
        // quality 75 → CRF ~24.
        let i = argv.iter().position(|a| a == "-crf").unwrap();
        let crf: u8 = argv[i + 1].parse().unwrap();
        assert!((23..=25).contains(&crf), "expected CRF 23-25, got {crf}");
    }

    #[test]
    fn quality_to_crf_endpoints() {
        // Practical range: visually-lossless at the top, small at the bottom —
        // never CRF 0 (which overflows the tool's output-size cap).
        assert_eq!(quality_to_crf(100), 18);
        assert_eq!(quality_to_crf(1), 40);
    }

    #[test]
    fn plan_rejects_out_of_range_quality() {
        assert!(plan("transcode", 0, "in.mkv").is_err());
        assert!(plan("transcode", 101, "in.mkv").is_err());
    }

    #[test]
    fn plan_rejects_unknown_mode() {
        assert!(plan("bogus", 75, "in.mkv").is_err());
    }
}
