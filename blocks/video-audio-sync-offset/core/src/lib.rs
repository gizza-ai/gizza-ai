//! gizza-ai/video-audio-sync-offset core — pure ffmpeg argv construction shared
//! by the chat skill block and the standalone web page. No wafer/wasm-bindgen
//! deps.
//!
//! Shifts a video's AUDIO track earlier or later relative to the picture by a
//! fixed time offset, to correct lip-sync drift. The picture is stream-copied
//! (`-c:v copy`, lossless, fast); only the audio is re-encoded (required, since
//! the timeline is rewritten). The output keeps the input container; the audio
//! codec matches it (webm → libopus, everything else → aac).
//!
//! Sign convention: a POSITIVE offset DELAYS the audio (it plays later — fixes
//! audio that runs ahead of the picture) by padding silence at the front
//! (`adelay`). A NEGATIVE offset ADVANCES the audio (it plays earlier — fixes
//! audio that lags) by trimming the head of the audio and resetting its
//! timestamps (`atrim` + `asetpts=PTS-STARTPTS`).

use gizza_ai_block_utils::ffmpeg::copy_out_ext;

/// How `offset` is interpreted.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Unit {
    /// Milliseconds (default). 200 = a fifth of a second.
    Ms,
    /// Seconds. 0.5 = half a second = 500 ms.
    Seconds,
}

/// Parse the user-facing unit string. Empty defaults to milliseconds.
pub fn parse_unit(s: &str) -> Result<Unit, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "ms" | "msec" | "msecs" | "millisecond" | "milliseconds" => Ok(Unit::Ms),
        "s" | "sec" | "secs" | "second" | "seconds" => Ok(Unit::Seconds),
        other => Err(format!("unit {other:?} not supported (ms|seconds)")),
    }
}

/// Largest shift we accept, in milliseconds (±60 s). Lip-sync drift is usually
/// well under a second; this cap keeps the padded/trimmed output sane.
pub const MAX_OFFSET_MS: f64 = 60_000.0;

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

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`0.5` stays
/// `0.5`, `2` not `2.0`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Build the `-af` audio filter for a signed millisecond `offset_ms`.
/// Positive delays the audio (`adelay`, integer ms, all channels); negative
/// advances it by trimming the head and resetting timestamps to 0. Callers
/// never pass exactly 0 (rejected in [`plan`] as a no-op).
pub fn build_filter(offset_ms: f64) -> String {
    if offset_ms > 0.0 {
        let ms = offset_ms.round() as i64;
        format!("adelay={ms}:all=1")
    } else {
        let seconds = (-offset_ms) / 1000.0;
        format!("atrim=start={},asetpts=PTS-STARTPTS", fmt_num(seconds))
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to shift `in_name`'s audio by
/// `offset_ms` into `out_name`, keeping the picture (`-c:v copy`). Shared
/// verbatim by the web page (`build_argv`) and the chat block.
pub fn build_argv(in_name: &str, out_name: &str, offset_ms: f64) -> Vec<String> {
    let out_ext = out_name.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    vec![
        "-i".to_string(),
        in_name.to_string(),
        "-c:v".to_string(),
        "copy".to_string(),
        "-af".to_string(),
        build_filter(offset_ms),
        "-c:a".to_string(),
        audio_codec(out_ext).to_string(),
        out_name.to_string(),
    ]
}

/// Validate `offset`/`unit`, convert to milliseconds, and return
/// `(argv, out_name)`. `out_name` keeps the input container when it can hold a
/// copied video stream; otherwise it is `out.mp4`. Single source shared by the
/// chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan(in_name: &str, offset: f64, unit: &str) -> Result<(Vec<String>, String), String> {
    let u = parse_unit(unit)?;
    if !offset.is_finite() {
        return Err(format!("offset must be a finite number, got {offset}"));
    }
    let offset_ms = match u {
        Unit::Ms => offset,
        Unit::Seconds => offset * 1000.0,
    };
    if offset_ms == 0.0 {
        return Err(
            "an offset of 0 wouldn't shift anything — use a positive value to delay the audio (play it later) or a negative one to advance it (play it earlier)"
                .into(),
        );
    }
    if offset_ms.abs() > MAX_OFFSET_MS {
        return Err(format!(
            "offset must be within ±{MAX_OFFSET_MS} ms (±60 s), got {offset_ms} ms"
        ));
    }
    let out_name = format!("out.{}", copy_out_ext(in_name));
    Ok((build_argv(in_name, &out_name, offset_ms), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_offset_delays_audio_with_adelay() {
        let (argv, out) = plan("in.mp4", 200.0, "ms").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i", "in.mp4", "-c:v", "copy", "-af", "adelay=200:all=1", "-c:a", "aac", "out.mp4",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn negative_offset_advances_audio_with_atrim() {
        let (argv, _) = plan("in.mp4", -200.0, "ms").unwrap();
        assert!(argv
            .iter()
            .any(|a| a == "atrim=start=0.2,asetpts=PTS-STARTPTS"));
        // video is kept, not dropped.
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
        assert!(!argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn seconds_unit_converts_to_milliseconds() {
        let (argv, _) = plan("in.mp4", 0.5, "seconds").unwrap();
        assert!(argv.iter().any(|a| a == "adelay=500:all=1"));
        // 1.6 s delay → 1600 ms.
        let (argv2, _) = plan("in.mp4", 1.6, "s").unwrap();
        assert!(argv2.iter().any(|a| a == "adelay=1600:all=1"));
    }

    #[test]
    fn fractional_ms_delay_rounds_to_integer() {
        // adelay wants integer ms; 555.5 rounds to 556.
        assert_eq!(build_filter(555.5), "adelay=556:all=1");
    }

    #[test]
    fn always_stream_copies_the_video() {
        let (argv, _) = plan("in.mp4", 300.0, "ms").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
        assert!(!argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn webm_input_keeps_webm_and_uses_opus_audio() {
        let (argv, out) = plan("clip.webm", 250.0, "ms").unwrap();
        assert_eq!(out, "out.webm");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libopus"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
    }

    #[test]
    fn container_kept_for_copy_capable_and_falls_back_to_mp4() {
        for ext in ["mp4", "mov", "m4v", "mkv", "webm"] {
            let (_, out) = plan(&format!("clip.{ext}"), 200.0, "ms").unwrap();
            assert_eq!(out, format!("out.{ext}"));
        }
        assert_eq!(plan("clip.avi", 200.0, "ms").unwrap().1, "out.mp4");
        assert_eq!(plan("noext", 200.0, "ms").unwrap().1, "out.mp4");
    }

    #[test]
    fn rejects_no_op_and_out_of_range_offsets() {
        assert!(plan("a.mp4", 0.0, "ms").is_err());
        assert!(plan("a.mp4", 0.0, "seconds").is_err());
        assert!(plan("a.mp4", 60_001.0, "ms").is_err());
        assert!(plan("a.mp4", -60_001.0, "ms").is_err());
        assert!(plan("a.mp4", 61.0, "seconds").is_err()); // 61_000 ms
        assert!(plan("a.mp4", f64::NAN, "ms").is_err());
        let err = plan("a.mp4", 0.0, "ms").unwrap_err();
        assert!(err.contains("wouldn't shift anything"));
    }

    #[test]
    fn rejects_unknown_unit() {
        assert!(plan("a.mp4", 200.0, "frames").is_err());
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(fmt_num(2.0), "2");
        assert_eq!(fmt_num(0.2), "0.2");
        assert_eq!(fmt_num(1.6), "1.6");
    }
}
