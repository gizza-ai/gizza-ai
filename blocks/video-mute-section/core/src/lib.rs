//! gizza-ai/video-mute-section core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Silences the audio over ONE chosen `[start, end]` time range (seconds) while
//! leaving the rest of the soundtrack intact and the PICTURE untouched: the
//! video stream is copied losslessly (`-c:v copy`, fast, byte-for-byte
//! identical) and only the audio is re-encoded (required — its samples inside
//! the window are being zeroed). The muting uses the `volume` filter's timeline
//! `enable='between(t,start,end)'` gate, so only that window drops to zero and
//! everything outside passes through unchanged. The output keeps the input
//! container; the audio codec matches it (webm → libopus, everything else →
//! aac).

use gizza_ai_block_utils::ffmpeg::copy_out_ext;

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

/// Validate the `[start, end]` window (seconds) and build the `-af` filter that
/// silences exactly that interval. `start` must be finite and `>= 0`; `end`
/// must be finite and strictly greater than `start` (a zero-length window would
/// be a no-op).
pub fn build_filter(start: f64, end: f64) -> Result<String, String> {
    if !start.is_finite() || start < 0.0 {
        return Err(format!(
            "start must be a finite number of seconds >= 0, got {start}"
        ));
    }
    if !end.is_finite() {
        return Err(format!("end must be a finite number of seconds, got {end}"));
    }
    if end <= start {
        return Err(format!(
            "end ({end}) must be greater than start ({start}) — give a non-empty range in seconds \
             (e.g. start 5, end 10 to silence seconds 5-10)"
        ));
    }
    // `volume` supports timeline editing: when the `enable` expression is true
    // (t inside the window) the gain drops to 0; outside it, the frame passes
    // through untouched.
    Ok(format!(
        "volume=enable='between(t,{},{})':volume=0",
        fmt_num(start),
        fmt_num(end)
    ))
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to silence `in_name`'s audio
/// over the window into `out_name`, keeping the picture (`-c:v copy`; never
/// `-vn`). Shared verbatim by the web page (`build_argv`) and the chat block.
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

/// Validate the window and return `(argv, out_name)`. `out_name` keeps the
/// input container when it can hold a copied video stream; otherwise `out.mp4`.
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
pub fn plan(in_name: &str, start: f64, end: f64) -> Result<(Vec<String>, String), String> {
    let filter = build_filter(start, end)?;
    let out_name = format!("out.{}", copy_out_ext(in_name));
    Ok((build_argv(in_name, &out_name, &filter), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_order_and_values() {
        let (argv, out) = plan("in.mp4", 5.0, 10.0).unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp4",
                "-c:v",
                "copy",
                "-af",
                "volume=enable='between(t,5,10)':volume=0",
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
    fn fractional_seconds_kept_compact() {
        let f = build_filter(1.5, 12.25).unwrap();
        assert_eq!(f, "volume=enable='between(t,1.5,12.25)':volume=0");
    }

    #[test]
    fn always_stream_copies_the_video() {
        let (argv, _) = plan("in.mp4", 0.0, 3.0).unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
        // Picture is preserved, never dropped.
        assert!(!argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn webm_input_keeps_webm_and_uses_opus_audio() {
        let (argv, out) = plan("clip.webm", 2.0, 4.0).unwrap();
        assert_eq!(out, "out.webm");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libopus"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
    }

    #[test]
    fn container_kept_for_copy_capable_and_falls_back_to_mp4() {
        for ext in ["mp4", "mov", "m4v", "mkv", "webm"] {
            let (_, out) = plan(&format!("clip.{ext}"), 1.0, 2.0).unwrap();
            assert_eq!(out, format!("out.{ext}"));
        }
        assert_eq!(plan("clip.avi", 1.0, 2.0).unwrap().1, "out.mp4");
        assert_eq!(plan("noext", 1.0, 2.0).unwrap().1, "out.mp4");
    }

    #[test]
    fn rejects_empty_or_reversed_window() {
        let err = plan("in.mp4", 5.0, 5.0).unwrap_err();
        assert!(err.contains("greater than start"), "{err}");
        assert!(plan("in.mp4", 10.0, 3.0).is_err());
    }

    #[test]
    fn rejects_negative_or_non_finite_bounds() {
        assert!(plan("in.mp4", -1.0, 3.0).is_err());
        assert!(plan("in.mp4", f64::NAN, 3.0).is_err());
        assert!(plan("in.mp4", 1.0, f64::INFINITY).is_err());
        let err = plan("in.mp4", -2.0, 3.0).unwrap_err();
        assert!(err.contains("start"), "names the offending bound: {err}");
    }

    #[test]
    fn zero_start_is_valid() {
        assert!(plan("in.mp4", 0.0, 4.0).is_ok());
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(3.0), "3");
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(fmt_num(12.25), "12.25");
    }
}
