//! gizza-ai/video-audio-fade core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Adds an audio-only fade-in at the start and/or fade-out at the end of a
//! video WITHOUT touching the picture: the video stream is copied losslessly
//! (`-c:v copy`, fast, byte-for-byte identical) and only the audio is
//! re-encoded (required — its samples are being ramped). `afade=t=in` handles
//! the start; the end is faded with the `areverse,afade=t=in,areverse` trick
//! because the argv is built BEFORE the input is decoded, so the fade-out start
//! time (duration − fade) isn't knowable here — but fading-in a reversed stream
//! needs no duration at all. The output keeps the input container; the audio
//! codec matches it (webm → libopus, everything else → aac).

use gizza_ai_block_utils::ffmpeg::copy_out_ext;

/// Longest accepted fade per side, in seconds.
pub const MAX_FADE_S: f64 = 30.0;

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

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`3` not `3.0`,
/// `0.5` stays `0.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn validate_fade(name: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() || !(0.0..=MAX_FADE_S).contains(&v) {
        return Err(format!(
            "{name} must be between 0 and {MAX_FADE_S} seconds, got {v}"
        ));
    }
    Ok(())
}

/// Build the `-af` chain: an `afade=t=in` stage when `fade_in` > 0, and the
/// duration-free `areverse,afade=t=in,areverse` stage when `fade_out` > 0.
/// A side at 0 is skipped; both at 0 is rejected as a no-op.
pub fn build_filter(fade_in: f64, fade_out: f64) -> Result<String, String> {
    let mut stages = Vec::new();
    if fade_in > 0.0 {
        stages.push(format!("afade=t=in:st=0:d={}", fmt_num(fade_in)));
    }
    if fade_out > 0.0 {
        stages.push(format!(
            "areverse,afade=t=in:st=0:d={},areverse",
            fmt_num(fade_out)
        ));
    }
    if stages.is_empty() {
        return Err(
            "both fades are 0 — nothing to change; set fade_in and/or fade_out in seconds \
             (e.g. 3 for a gentle three-second fade)"
                .to_string(),
        );
    }
    Ok(stages.join(","))
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to fade `in_name`'s audio into
/// `out_name`, keeping the picture (`-c:v copy`; never `-vn`). Shared verbatim
/// by the web page (`build_argv`) and the chat block.
pub fn build_argv(in_name: &str, out_name: &str, filter: &str) -> Vec<String> {
    let out_ext = out_name.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    vec![
        "-i".to_string(),
        in_name.to_string(),
        "-c:v".to_string(),
        "copy".to_string(),
        "-af".to_string(),
        filter.to_string(),
        "-c:a".to_string(),
        audio_codec(out_ext).to_string(),
        out_name.to_string(),
    ]
}

/// Validate both fade lengths and return `(argv, out_name)`. `out_name` keeps
/// the input container when it can hold a copied video stream; otherwise it is
/// `out.mp4`. Single source shared by the chat block (`src/lib.rs`) and the web
/// page (`web/src/lib.rs`).
pub fn plan(in_name: &str, fade_in: f64, fade_out: f64) -> Result<(Vec<String>, String), String> {
    validate_fade("fade_in", fade_in)?;
    validate_fade("fade_out", fade_out)?;
    let filter = build_filter(fade_in, fade_out)?;
    let out_name = format!("out.{}", copy_out_ext(in_name));
    Ok((build_argv(in_name, &out_name, &filter), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_fades_argv_order_and_values() {
        let (argv, out) = plan("in.mp4", 3.0, 3.0).unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp4",
                "-c:v",
                "copy",
                "-af",
                "afade=t=in:st=0:d=3,areverse,afade=t=in:st=0:d=3,areverse",
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
    fn single_sided_fades_skip_the_other_stage() {
        assert_eq!(build_filter(2.0, 0.0).unwrap(), "afade=t=in:st=0:d=2");
        assert_eq!(
            build_filter(0.0, 1.5).unwrap(),
            "areverse,afade=t=in:st=0:d=1.5,areverse"
        );
    }

    #[test]
    fn fade_out_never_needs_a_start_time() {
        // The whole point of the areverse trick: no `st=<duration - d>` term
        // may appear, because the input duration is unknown at argv-build time.
        let f = build_filter(0.0, 5.0).unwrap();
        assert!(!f.contains("t=out"), "{f}");
        assert!(f.starts_with("areverse,") && f.ends_with(",areverse"), "{f}");
    }

    #[test]
    fn always_stream_copies_the_video() {
        let (argv, _) = plan("in.mp4", 3.0, 0.0).unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
        // Picture is preserved, never dropped.
        assert!(!argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn webm_input_keeps_webm_and_uses_opus_audio() {
        let (argv, out) = plan("clip.webm", 2.0, 2.0).unwrap();
        assert_eq!(out, "out.webm");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libopus"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
    }

    #[test]
    fn container_kept_for_copy_capable_and_falls_back_to_mp4() {
        for ext in ["mp4", "mov", "m4v", "mkv", "webm"] {
            let (_, out) = plan(&format!("clip.{ext}"), 3.0, 0.0).unwrap();
            assert_eq!(out, format!("out.{ext}"));
        }
        assert_eq!(plan("clip.avi", 3.0, 0.0).unwrap().1, "out.mp4");
        assert_eq!(plan("noext", 3.0, 0.0).unwrap().1, "out.mp4");
    }

    #[test]
    fn both_zero_is_rejected_as_a_no_op() {
        let err = plan("in.mp4", 0.0, 0.0).unwrap_err();
        assert!(err.contains("nothing to change"), "{err}");
    }

    #[test]
    fn out_of_range_or_non_finite_fades_are_rejected() {
        assert!(plan("a.mp4", -1.0, 3.0).is_err());
        assert!(plan("a.mp4", 3.0, 31.0).is_err());
        assert!(plan("a.mp4", f64::NAN, 3.0).is_err());
        assert!(plan("a.mp4", 3.0, f64::INFINITY).is_err());
        // Boundaries are valid.
        assert!(plan("a.mp4", 30.0, 0.0).is_ok());
        let err = plan("a.mp4", 3.0, 31.0).unwrap_err();
        assert!(err.contains("fade_out"), "names the offending side: {err}");
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(3.0), "3");
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(fmt_num(12.25), "12.25");
    }
}
