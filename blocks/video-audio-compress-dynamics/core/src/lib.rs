//! gizza-ai/video-audio-compress-dynamics core — pure ffmpeg argv construction
//! shared by the chat skill block and the standalone web page. No wafer/
//! wasm-bindgen deps.
//!
//! Applies dynamic-range compression to a video's audio with ffmpeg's
//! `acompressor` filter so quiet and loud passages sit closer together (evens
//! out the audio; this is NOT file-size compression — that's the separate
//! audio-compress tool). The picture is stream-copied (`-c:v copy`, lossless,
//! fast); only the audio is re-encoded (required, since `acompressor` rewrites
//! samples). An optional make-up gain (on by default) restores the overall
//! loudness the compressor pulled down. The output keeps the input container;
//! the audio codec is chosen to match it (webm → libopus, everything else →
//! aac).

use gizza_ai_block_utils::ffmpeg::copy_out_ext;

/// How hard to squeeze the dynamic range. Heavier presets use a lower
/// threshold + higher ratio + a faster attack (and more make-up gain), so more
/// of the signal is compressed and the loud/quiet gap shrinks further.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Preset {
    /// Gentle levelling — keeps most of the natural dynamics.
    Light,
    /// A balanced, broadcast-style evening-out (the default).
    Medium,
    /// Aggressive levelling — quiet and loud parts end up close together.
    Heavy,
}

/// Parse the user-facing preset string. Empty defaults to `medium`.
pub fn parse_preset(s: &str) -> Result<Preset, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "medium" => Ok(Preset::Medium),
        "light" => Ok(Preset::Light),
        "heavy" => Ok(Preset::Heavy),
        other => Err(format!("preset {other:?} not supported (light|medium|heavy)")),
    }
}

impl Preset {
    /// `acompressor` parameters for this preset, as `(threshold, ratio, attack,
    /// release, makeup)`. `makeup` is the linear make-up gain applied when the
    /// make-up toggle is on. These mirror the proven presets in the
    /// audio-effects-rack tool so behaviour is consistent across the toolkit.
    fn params(self) -> (&'static str, u32, u32, u32, u32) {
        match self {
            Preset::Light => ("-18dB", 2, 20, 250, 2),
            Preset::Medium => ("-24dB", 4, 10, 200, 4),
            Preset::Heavy => ("-30dB", 8, 5, 150, 6),
        }
    }
}

/// Build the `acompressor` filter string for `preset`. When `makeup` is off the
/// make-up gain is pinned to `1` (unity) so the output isn't boosted back up —
/// useful when you only want to tame peaks without raising the overall level.
pub fn build_filter(preset: Preset, makeup: bool) -> String {
    let (threshold, ratio, attack, release, mk) = preset.params();
    let makeup_gain = if makeup { mk } else { 1 };
    format!(
        "acompressor=threshold={threshold}:ratio={ratio}:attack={attack}:release={release}:makeup={makeup_gain}"
    )
}

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

/// Build the ffmpeg argv (no leading `ffmpeg`) to compress `in_name`'s audio
/// dynamics into `out_name`, keeping the picture (`-c:v copy`). Shared verbatim
/// by the web page (`build_argv`) and the chat block.
pub fn build_argv(in_name: &str, out_name: &str, preset: Preset, makeup: bool) -> Vec<String> {
    let out_ext = out_name.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    vec![
        "-i".to_string(),
        in_name.to_string(),
        "-c:v".to_string(),
        "copy".to_string(),
        "-af".to_string(),
        build_filter(preset, makeup),
        "-c:a".to_string(),
        audio_codec(out_ext).to_string(),
        out_name.to_string(),
    ]
}

/// Parse the preset, then return `(argv, out_name)`. `out_name` keeps the input
/// container when it can hold a copied video stream; otherwise it is `out.mp4`.
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
pub fn plan(in_name: &str, preset: &str, makeup: bool) -> Result<(Vec<String>, String), String> {
    let p = parse_preset(preset)?;
    let out_name = format!("out.{}", copy_out_ext(in_name));
    Ok((build_argv(in_name, &out_name, p, makeup), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medium_argv_order_and_values() {
        let (argv, out) = plan("in.mp4", "medium", true).unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp4",
                "-c:v",
                "copy",
                "-af",
                "acompressor=threshold=-24dB:ratio=4:attack=10:release=200:makeup=4",
                "-c:a",
                "aac",
                "out.mp4",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn preset_default_is_medium() {
        assert_eq!(parse_preset("").unwrap(), Preset::Medium);
        assert_eq!(
            build_filter(Preset::Medium, true),
            build_filter(parse_preset("").unwrap(), true)
        );
    }

    #[test]
    fn heavier_presets_lower_threshold_and_raise_ratio() {
        assert_eq!(
            build_filter(Preset::Light, true),
            "acompressor=threshold=-18dB:ratio=2:attack=20:release=250:makeup=2"
        );
        assert_eq!(
            build_filter(Preset::Heavy, true),
            "acompressor=threshold=-30dB:ratio=8:attack=5:release=150:makeup=6"
        );
    }

    #[test]
    fn makeup_off_pins_gain_to_unity() {
        let f = build_filter(Preset::Medium, false);
        assert!(f.ends_with(":makeup=1"), "got {f}");
        // ratio/threshold are unchanged — only the make-up gain differs.
        assert!(f.contains("threshold=-24dB:ratio=4"));
    }

    #[test]
    fn always_stream_copies_the_video() {
        let (argv, _) = plan("in.mp4", "medium", true).unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
        // no -vn: the video track is kept.
        assert!(!argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn webm_input_keeps_webm_and_uses_opus_audio() {
        let (argv, out) = plan("clip.webm", "light", true).unwrap();
        assert_eq!(out, "out.webm");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libopus"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
    }

    #[test]
    fn container_kept_for_copy_capable_and_falls_back_to_mp4() {
        for ext in ["mp4", "mov", "m4v", "mkv", "webm"] {
            let (_, out) = plan(&format!("clip.{ext}"), "medium", true).unwrap();
            assert_eq!(out, format!("out.{ext}"));
        }
        // Unknown/absent extension → mp4.
        assert_eq!(plan("clip.avi", "medium", true).unwrap().1, "out.mp4");
        assert_eq!(plan("noext", "medium", true).unwrap().1, "out.mp4");
    }

    #[test]
    fn rejects_unknown_preset() {
        assert!(plan("a.mp4", "extreme", true).is_err());
        let err = plan("a.mp4", "extreme", true).unwrap_err();
        assert!(err.contains("light|medium|heavy"));
    }
}
