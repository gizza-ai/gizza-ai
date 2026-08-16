//! gizza-ai/video-vfr-to-cfr core — pure ffmpeg argv construction shared by the
//! chat block and the standalone web page. No wasm-bindgen deps.
//!
//! Converts a **variable-frame-rate** (VFR) video — phone footage, screen and
//! game recordings — into a **constant-frame-rate** (CFR) one, the form editors
//! and NLEs expect. VFR clips carry irregular gaps between frame timestamps; a
//! tool that assumes an even cadence slowly misreads that timing, which is the
//! classic "audio drifts further out of sync the longer the clip runs" bug.
//!
//! The conversion is `-fps_mode cfr`: ffmpeg regenerates the presentation
//! timestamps on an exact grid, duplicating a frame where the source had a long
//! gap and dropping one where it had a burst. Re-timing the frames means the
//! video stream must be re-encoded (H.264, `yuv420p`), so the quality knob picks
//! the `-crf`. Audio is stream-copied by default-off of the resync switch; with
//! `resync_audio` on (the default) it is resampled through
//! `aresample=async=1:first_pts=0`, which pads/trims the audio to keep it locked
//! to the new video timeline from the first frame.
//!
//! The output container follows the input when it can hold H.264/AAC
//! (mp4/mov/m4v/mkv); anything else (e.g. webm) becomes MP4 — see
//! [`gizza_ai_block_utils::ffmpeg::h264_out_ext`].

/// Target frame rates offered by the tool. `auto` keeps the source's own
/// nominal rate (ffmpeg's guessed `r_frame_rate`) and only makes the timing
/// constant — the safest choice, since it neither drops nor invents frames
/// beyond what the irregular timestamps force.
pub const FPS_CHOICES: [&str; 9] = [
    "auto", "23.976", "24", "25", "29.97", "30", "50", "59.94", "60",
];
/// Frame rate used when the request is empty/unset.
pub const DEFAULT_FPS: &str = "auto";

/// Quality presets, mapped to an x264 `-crf` by [`resolve_crf`].
pub const QUALITY_CHOICES: [&str; 3] = ["high", "balanced", "small"];
/// Quality preset used when the request is empty/unset.
pub const DEFAULT_QUALITY: &str = "balanced";
/// Whether audio is re-locked to the rebuilt video timeline by default.
pub const DEFAULT_RESYNC_AUDIO: bool = true;

/// Audio filter that re-locks the track to the regenerated video timeline:
/// `async=1` stretches/squeezes to correct drift, `first_pts=0` anchors the
/// track at the very first frame so a clip that started late doesn't stay late.
pub const RESYNC_FILTER: &str = "aresample=async=1:first_pts=0";
/// AAC bitrate used whenever the audio has to be re-encoded.
pub const AUDIO_BITRATE: &str = "192k";
/// x264 speed/compression preset.
pub const PRESET: &str = "medium";

/// Resolve a frame-rate request to one of [`FPS_CHOICES`].
///
/// Returns `Ok(None)` for `auto` (no explicit `-r`; ffmpeg locks the source's
/// own nominal rate) and `Ok(Some(rate))` for an explicit rate. Matching is
/// case-insensitive, trims whitespace, treats an empty request as the default,
/// and accepts the common spellings of the NTSC rates (`23.98`, `24000/1001`,
/// `30000/1001`, `60000/1001`) plus `source`/`same`/`keep` for `auto`.
pub fn resolve_fps(fps: &str) -> Result<Option<&'static str>, String> {
    let want = fps.trim().to_ascii_lowercase();
    let want = match want.as_str() {
        "" => DEFAULT_FPS,
        "source" | "same" | "keep" | "same-as-source" => "auto",
        "23.98" | "24000/1001" => "23.976",
        "30000/1001" => "29.97",
        "60000/1001" => "59.94",
        other => other,
    };
    if want == "auto" {
        return Ok(None);
    }
    FPS_CHOICES
        .iter()
        .copied()
        .find(|c| *c == want)
        .map(Some)
        .ok_or_else(|| {
            format!(
                "fps must be one of {} (got \"{}\")",
                FPS_CHOICES.join(", "),
                fps.trim()
            )
        })
}

/// Map a quality preset to the x264 `-crf` it encodes with. Lower crf = better
/// picture and a bigger file.
pub fn resolve_crf(quality: &str) -> Result<&'static str, String> {
    match quality.trim().to_ascii_lowercase().as_str() {
        "" | "balanced" => Ok("20"),
        "high" => Ok("18"),
        "small" => Ok("24"),
        other => Err(format!(
            "quality must be one of {} (got \"{}\")",
            QUALITY_CHOICES.join(", "),
            other
        )),
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) and the output filename.
///
/// `fps` is one of [`FPS_CHOICES`], `quality` one of [`QUALITY_CHOICES`], and
/// `resync_audio` decides whether the audio track is resampled onto the new
/// timeline (`true`, the default) or stream-copied untouched (`false`). Audio
/// is re-encoded to AAC regardless when the input container can't be kept.
pub fn plan(
    fps: &str,
    quality: &str,
    resync_audio: bool,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    let rate = resolve_fps(fps)?;
    let crf = resolve_crf(quality)?;
    let (ext, must_reencode_audio) = gizza_ai_block_utils::ffmpeg::h264_out_ext(in_name);
    let out_name = format!("out.{ext}");

    let mut argv: Vec<String> = vec!["-i".into(), in_name.into()];
    // Regenerate the timestamps on an exact grid — this IS the VFR→CFR step.
    argv.push("-fps_mode".into());
    argv.push("cfr".into());
    if let Some(r) = rate {
        argv.push("-r".into());
        argv.push(r.into());
    }
    argv.extend([
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        PRESET.into(),
        "-crf".into(),
        crf.into(),
        // 8-bit 4:2:0 is what every editor and browser can decode.
        "-pix_fmt".into(),
        "yuv420p".into(),
    ]);
    if resync_audio {
        // Re-locking the audio needs a filter, and a filtered stream can't be
        // copied — AAC is the container-safe re-encode.
        argv.extend([
            "-af".into(),
            RESYNC_FILTER.into(),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            AUDIO_BITRATE.into(),
        ]);
    } else if must_reencode_audio {
        argv.extend(["-c:a".into(), "aac".into(), "-b:a".into(), AUDIO_BITRATE.into()]);
    } else {
        argv.extend(["-c:a".into(), "copy".into()]);
    }
    argv.push(out_name.clone());
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg_after<'a>(argv: &'a [String], flag: &str) -> Option<&'a str> {
        argv.iter()
            .position(|a| a == flag)
            .and_then(|i| argv.get(i + 1))
            .map(String::as_str)
    }

    #[test]
    fn default_plan_is_auto_balanced_with_audio_resync() {
        let (argv, out) = plan("", "", true, "in.mp4").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            [
                "-i",
                "in.mp4",
                "-fps_mode",
                "cfr",
                "-c:v",
                "libx264",
                "-preset",
                "medium",
                "-crf",
                "20",
                "-pix_fmt",
                "yuv420p",
                "-af",
                "aresample=async=1:first_pts=0",
                "-c:a",
                "aac",
                "-b:a",
                "192k",
                "out.mp4",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn auto_emits_no_explicit_rate() {
        let (argv, _) = plan("auto", "balanced", true, "in.mp4").unwrap();
        assert!(!argv.iter().any(|a| a == "-r"), "auto must not force -r");
        assert_eq!(arg_after(&argv, "-fps_mode"), Some("cfr"));
    }

    #[test]
    fn explicit_rate_is_passed_through() {
        for rate in ["23.976", "24", "25", "29.97", "30", "50", "59.94", "60"] {
            let (argv, _) = plan(rate, "balanced", true, "in.mp4").unwrap();
            assert_eq!(arg_after(&argv, "-r"), Some(rate), "{rate}");
        }
    }

    #[test]
    fn ntsc_and_source_aliases_resolve() {
        assert_eq!(resolve_fps("23.98").unwrap(), Some("23.976"));
        assert_eq!(resolve_fps("24000/1001").unwrap(), Some("23.976"));
        assert_eq!(resolve_fps("30000/1001").unwrap(), Some("29.97"));
        assert_eq!(resolve_fps("60000/1001").unwrap(), Some("59.94"));
        assert_eq!(resolve_fps("source").unwrap(), None);
        assert_eq!(resolve_fps("  KEEP ").unwrap(), None);
        assert_eq!(resolve_fps("").unwrap(), None);
    }

    #[test]
    fn unknown_rate_is_rejected_with_the_accepted_list() {
        let err = plan("42", "balanced", true, "in.mp4").unwrap_err();
        assert!(err.contains("fps must be one of"), "{err}");
        assert!(err.contains("29.97"), "{err}");
        assert!(err.contains("got \"42\""), "{err}");
    }

    #[test]
    fn unknown_quality_is_rejected_with_the_accepted_list() {
        let err = plan("30", "lossless", true, "in.mp4").unwrap_err();
        assert!(err.contains("quality must be one of"), "{err}");
        assert!(err.contains("balanced"), "{err}");
    }

    #[test]
    fn quality_presets_map_to_crf() {
        assert_eq!(resolve_crf("high").unwrap(), "18");
        assert_eq!(resolve_crf("balanced").unwrap(), "20");
        assert_eq!(resolve_crf("Small").unwrap(), "24");
        assert_eq!(resolve_crf("").unwrap(), "20");
        let (argv, _) = plan("30", "small", true, "in.mp4").unwrap();
        assert_eq!(arg_after(&argv, "-crf"), Some("24"));
    }

    #[test]
    fn resync_off_stream_copies_audio_when_the_container_is_kept() {
        let (argv, out) = plan("30", "balanced", false, "clip.mov").unwrap();
        assert_eq!(out, "out.mov");
        assert_eq!(arg_after(&argv, "-c:a"), Some("copy"));
        assert!(!argv.iter().any(|a| a == "-af"), "no filter without resync");
    }

    #[test]
    fn resync_off_still_reencodes_audio_when_the_container_changes() {
        // webm can't hold H.264/AAC → output switches to MP4 and the (Opus/Vorbis)
        // audio can't be copied into it.
        let (argv, out) = plan("30", "balanced", false, "clip.webm").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(arg_after(&argv, "-c:a"), Some("aac"));
        assert!(!argv.iter().any(|a| a == "-af"));
    }

    #[test]
    fn h264_capable_containers_are_kept() {
        for ext in ["mp4", "mov", "m4v", "mkv"] {
            let (_, out) = plan("30", "balanced", true, &format!("clip.{ext}")).unwrap();
            assert_eq!(out, format!("out.{ext}"));
        }
    }

    #[test]
    fn argv_shape_is_stable() {
        let (argv, _) = plan("60", "high", true, "in.mp4").unwrap();
        assert_eq!(argv.first().map(String::as_str), Some("-i"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(argv.windows(2).any(|w| w[0] == "-pix_fmt" && w[1] == "yuv420p"));
        assert_eq!(arg_after(&argv, "-crf"), Some("18"));
        assert_eq!(arg_after(&argv, "-r"), Some("60"));
    }
}
