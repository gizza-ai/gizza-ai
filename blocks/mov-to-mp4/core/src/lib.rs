//! gizza-ai/mov-to-mp4 core — pure ffmpeg argv construction shared by the chat
//! block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! mov-to-mp4 rewraps a QuickTime `.mov` into an `.mp4` container. A `.mov` is
//! structurally an ISO-BMFF container just like `.mp4`, so when its streams are
//! already MP4-legal (H.264/HEVC video + AAC audio — true for essentially every
//! iPhone / camera recording) the packets can be *stream-copied* into the new
//! container with `-c copy`: no re-encode, no quality change, near-instant. Only
//! MOVs carrying codecs MP4 can't hold (e.g. Apple ProRes) need a real transcode
//! to H.264/AAC, which the `transcode` mode does as an explicit fallback.
//!
//! This is deliberately distinct from video-transcode / video-compress, which
//! ALWAYS re-encode with libx264 (lossy, slow); the headline here is the lossless
//! `-c copy` remux.

/// How mov-to-mp4 produces the MP4: lossless container remux or a full re-encode.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    /// `-c copy`: stream-copy every track into the MP4 container. Lossless and
    /// fast; requires the MOV's codecs to be MP4-legal (H.264/HEVC + AAC).
    Copy,
    /// `-c:v libx264 -c:a aac`: re-encode to H.264/AAC. Always produces a valid
    /// MP4 (use for ProRes and other non-MP4-legal MOV codecs); lossy + slower.
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

/// Build the ffmpeg argv (no leading `ffmpeg`) to rewrap `in_name` → `out_name`
/// as MP4. `copy` stream-copies (`-c copy`); `transcode` re-encodes to
/// libx264/aac at `crf`. Both set `-movflags +faststart` so the moov atom is at
/// the front for progressive web playback.
pub fn build_argv(in_name: &str, out_name: &str, mode: Mode, crf: u8) -> Vec<String> {
    match mode {
        Mode::Copy => vec![
            "-i".into(),
            in_name.into(),
            "-c".into(),
            "copy".into(),
            "-movflags".into(),
            "+faststart".into(),
            out_name.into(),
        ],
        Mode::Transcode => vec![
            "-i".into(),
            in_name.into(),
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
        let (argv, out) = plan("copy", DEFAULT_QUALITY, "in.mov").unwrap();
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
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-movflags" && w[1] == "+faststart"));
        assert_eq!(argv.first().map(String::as_str), Some("-i"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn transcode_mode_reencodes_h264_aac() {
        let (argv, _) = plan("transcode", 75, "in.mov").unwrap();
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
        assert!(plan("transcode", 0, "in.mov").is_err());
        assert!(plan("transcode", 101, "in.mov").is_err());
    }

    #[test]
    fn plan_rejects_unknown_mode() {
        assert!(plan("bogus", 75, "in.mov").is_err());
    }
}
