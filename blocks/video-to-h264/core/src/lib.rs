//! gizza-ai/video-to-h264 core — pure ffmpeg argv construction shared by the chat
//! block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! video-to-h264 force-transcodes ANY input video into the single most
//! universally playable form: **H.264** video (libx264) in an **MP4** container,
//! **`yuv420p`** 8-bit 4:2:0 chroma, **`+faststart`** (moov atom moved to the
//! front for progressive web playback), and **AAC** audio. That specific combo
//! is what decodes on essentially every browser, phone, TV and old player —
//! 10-bit / 4:2:2 / 4:4:4 / HEVC / VP9 / AV1 sources that "won't play" are
//! normalized down to it.
//!
//! Deliberately distinct from its neighbours:
//! - `mov-to-mp4` REMUXES (`-c copy`) when it can — it keeps whatever codec/pixel
//!   format the MOV already has, so it does NOT guarantee H.264 High + yuv420p.
//! - `video-transcode` switches the target CONTAINER/codec (mp4 ⇆ webm) and never
//!   forces a profile or pixel format.
//! - `video-compress` is a size/quality knob (CRF) and never forces a profile or
//!   pixel format either.
//!
//! This tool ALWAYS re-encodes and ALWAYS pins profile + `yuv420p` + faststart —
//! that forced normalize is the whole point.

/// H.264 profile the output is pinned to. Higher profiles compress better and
/// play on all modern devices; lower profiles trade compression for reach into
/// older / embedded hardware decoders.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Profile {
    /// `high` — best compression, plays on every browser and modern device
    /// (2012+). The default; the right pick unless you must reach ancient hardware.
    High,
    /// `main` — slightly wider legacy reach than high, marginally larger files.
    Main,
    /// `baseline` — maximum compatibility with very old / embedded players
    /// (no B-frames, no CABAC). Largest file for the same quality; use only when
    /// something genuinely refuses `high`.
    Baseline,
}

impl Profile {
    /// The exact string passed to ffmpeg's `-profile:v`.
    pub fn as_arg(self) -> &'static str {
        match self {
            Profile::High => "high",
            Profile::Main => "main",
            Profile::Baseline => "baseline",
        }
    }
}

/// Parse the user-facing profile string (the values the chat schema + page accept).
pub fn parse_profile(s: &str) -> Result<Profile, String> {
    match s {
        "high" => Ok(Profile::High),
        "main" => Ok(Profile::Main),
        "baseline" => Ok(Profile::Baseline),
        other => Err(format!(
            "profile {other:?} not supported (high|main|baseline)"
        )),
    }
}

/// Default profile when none is supplied — `high` is universally supported on
/// modern browsers/devices and compresses best.
pub const DEFAULT_PROFILE: Profile = Profile::High;

/// Default quality when none is supplied (≈ CRF 24 — a good size/quality balance).
pub const DEFAULT_QUALITY: u8 = 75;

/// Lowest CRF the quality slider maps to (`quality = 100`). 18 is "visually
/// lossless" for libx264 — deliberately NOT 0 (true-lossless), which produces
/// enormous files that blow past the tool's 10 MiB output cap for even short
/// clips.
pub const MIN_CRF: f32 = 18.0;
/// Highest CRF the quality slider maps to (`quality = 1`) — low quality, small
/// file. Above ~40 libx264 output degrades sharply, so the range stops here.
pub const MAX_CRF: f32 = 40.0;

/// Map web-conventional quality 1-100 to a practical ffmpeg libx264 CRF, high
/// quality → low CRF: `quality = 100` → CRF 18 (visually lossless), `quality =
/// 1` → CRF 40 (small, low quality), default 75 ≈ CRF 24.
pub fn quality_to_crf(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let crf = MAX_CRF - (q - 1.0) * (MAX_CRF - MIN_CRF) / 99.0;
    crf.round().clamp(MIN_CRF, MAX_CRF) as u8
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that force-transcodes `in_name`
/// → `out_name` to universally-playable H.264: libx264 at the chosen `profile`,
/// forced `-pix_fmt yuv420p`, `-crf`, `-preset medium`, AAC audio, and
/// `-movflags +faststart`. `-c:a aac` is a no-op when the input has no audio.
pub fn build_argv(in_name: &str, out_name: &str, profile: Profile, crf: u8) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-c:v".into(),
        "libx264".into(),
        "-profile:v".into(),
        profile.as_arg().into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-crf".into(),
        crf.to_string(),
        "-preset".into(),
        "medium".into(),
        "-c:a".into(),
        "aac".into(),
        "-movflags".into(),
        "+faststart".into(),
        out_name.into(),
    ]
}

/// Validate `quality`, parse `profile`, and build `(argv, out_name)` for an input
/// file. `out_name` is always `out.mp4`. Single source shared by the chat block
/// (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan(profile: &str, quality: u8, in_name: &str) -> Result<(Vec<String>, String), String> {
    if !(1..=100).contains(&quality) {
        return Err(format!("quality must be 1-100, got {quality}"));
    }
    let p = parse_profile(profile)?;
    let crf = quality_to_crf(quality);
    let out_name = "out.mp4".to_string();
    Ok((build_argv(in_name, &out_name, p, crf), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_profile_known() {
        assert_eq!(parse_profile("high").unwrap(), Profile::High);
        assert_eq!(parse_profile("main").unwrap(), Profile::Main);
        assert_eq!(parse_profile("baseline").unwrap(), Profile::Baseline);
    }

    #[test]
    fn parse_profile_rejects_unknown() {
        assert!(parse_profile("high444").is_err());
        assert!(parse_profile("hevc").is_err());
        assert!(parse_profile("").is_err());
    }

    #[test]
    fn argv_always_forces_h264_high_yuv420p_faststart_aac() {
        let (argv, out) = plan("high", DEFAULT_QUALITY, "in.webm").unwrap();
        assert_eq!(out, "out.mp4");
        // The forced-normalize invariants — every one must always be present.
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-profile:v" && w[1] == "high"));
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-pix_fmt" && w[1] == "yuv420p"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-movflags" && w[1] == "+faststart"));
        assert_eq!(argv.first().map(String::as_str), Some("-i"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn baseline_profile_is_passed_through() {
        let (argv, _) = plan("baseline", 60, "in.mkv").unwrap();
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-profile:v" && w[1] == "baseline"));
        // Still forces yuv420p (baseline supports 4:2:0).
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-pix_fmt" && w[1] == "yuv420p"));
    }

    #[test]
    fn quality_75_maps_to_crf_around_24() {
        let (argv, _) = plan("high", 75, "in.mp4").unwrap();
        let i = argv.iter().position(|a| a == "-crf").unwrap();
        let crf: u8 = argv[i + 1].parse().unwrap();
        assert!((23..=25).contains(&crf), "expected CRF 23-25, got {crf}");
    }

    #[test]
    fn quality_to_crf_endpoints() {
        // Practical range: visually-lossless at the top, small at the bottom —
        // never CRF 0 (which overflows the tool's 10 MiB output cap).
        assert_eq!(quality_to_crf(100), 18);
        assert_eq!(quality_to_crf(1), 40);
    }

    #[test]
    fn plan_rejects_out_of_range_quality() {
        assert!(plan("high", 0, "in.mp4").is_err());
        assert!(plan("high", 101, "in.mp4").is_err());
    }

    #[test]
    fn plan_rejects_unknown_profile() {
        assert!(plan("ultra", 75, "in.mp4").is_err());
    }
}
