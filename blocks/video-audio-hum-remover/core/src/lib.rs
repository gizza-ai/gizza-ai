//! gizza-ai/video-audio-hum-remover core — pure ffmpeg argv construction shared
//! by the chat block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Removes 50/60 Hz mains ("electrical") hum and its harmonics from a video's
//! audio track with a chain of narrow band-reject (notch) filters. The picture
//! is stream-copied (`-c:v copy`, lossless, fast); only the audio is re-encoded
//! (the notch chain rewrites samples). The output keeps the input container; the
//! audio codec is chosen to match it (webm → libopus, everything else → aac).
//!
//! One notch is placed at the fundamental (50 or 60 Hz) and one at each of the
//! next `harmonics` multiples (100/150/… or 120/180/…), so a single pass cleans
//! the whole hum comb instead of the manual per-harmonic approach desktop
//! editors need. `q` sets how narrow each notch is (higher = narrower = less
//! damage to nearby audio).

use gizza_ai_block_utils::ffmpeg::copy_out_ext;

/// Accepted mains fundamentals. A fixed choice — mains hum is 50 Hz (Europe,
/// Asia, Africa, most of the world) or 60 Hz (North & most of South America).
pub const FREQUENCIES: [f64; 2] = [50.0, 60.0];

/// Accepted number of ADDITIONAL harmonics to notch beyond the fundamental.
pub const MIN_HARMONICS: i64 = 0;
pub const MAX_HARMONICS: i64 = 12;

/// Accepted notch Q (narrowness). Higher = narrower notch = less damage to
/// nearby audio, but less forgiving if the hum drifts slightly off frequency.
pub const MIN_Q: f64 = 1.0;
pub const MAX_Q: f64 = 100.0;

/// Parse the user-facing fundamental string ("50"/"60"). Empty defaults to 50.
pub fn parse_frequency(s: &str) -> Result<f64, String> {
    match s.trim() {
        "" | "50" => Ok(50.0),
        "60" => Ok(60.0),
        other => Err(format!("frequency {other:?} not supported (50|60)")),
    }
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

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`50` not `50.0`,
/// `12.5` stays `12.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.5}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Build the ffmpeg `-af` notch chain: one `bandreject` per hum line from the
/// fundamental through the `harmonics`-th multiple. `q` is the shared width.
/// e.g. `frequency=50, harmonics=2, q=10` →
/// `bandreject=f=50:width_type=q:w=10,bandreject=f=100:width_type=q:w=10,bandreject=f=150:width_type=q:w=10`.
pub fn build_filter(frequency: f64, harmonics: i64, q: f64) -> String {
    (0..=harmonics)
        .map(|n| {
            let f = frequency * (n as f64 + 1.0);
            format!("bandreject=f={}:width_type=q:w={}", fmt_num(f), fmt_num(q))
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to remove hum from `in_name`'s
/// audio into `out_name`, keeping the picture (`-c:v copy`). Shared verbatim by
/// the web page (`build_argv`) and the chat block.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    frequency: f64,
    harmonics: i64,
    q: f64,
) -> Vec<String> {
    let out_ext = out_name.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    vec![
        "-i".to_string(),
        in_name.to_string(),
        "-c:v".to_string(),
        "copy".to_string(),
        "-af".to_string(),
        build_filter(frequency, harmonics, q),
        "-c:a".to_string(),
        audio_codec(out_ext).to_string(),
        out_name.to_string(),
    ]
}

/// Validate `frequency`/`harmonics`/`q`, and return `(argv, out_name)`.
/// `out_name` keeps the input container when it can hold a copied video stream;
/// otherwise it is `out.mp4`. Single source shared by the chat block
/// (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan(
    in_name: &str,
    frequency: &str,
    harmonics: i64,
    q: f64,
) -> Result<(Vec<String>, String), String> {
    let f = parse_frequency(frequency)?;
    if !(MIN_HARMONICS..=MAX_HARMONICS).contains(&harmonics) {
        return Err(format!(
            "harmonics must be between {MIN_HARMONICS} and {MAX_HARMONICS}, got {harmonics}"
        ));
    }
    if !q.is_finite() || q < MIN_Q || q > MAX_Q {
        return Err(format!(
            "q must be between {MIN_Q} and {MAX_Q} (higher = narrower notch), got {q}"
        ));
    }
    let out_name = format!("out.{}", copy_out_ext(in_name));
    Ok((build_argv(in_name, &out_name, f, harmonics, q), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_order_and_values_50hz() {
        let (argv, out) = plan("in.mp4", "50", 2, 10.0).unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp4",
                "-c:v",
                "copy",
                "-af",
                "bandreject=f=50:width_type=q:w=10,bandreject=f=100:width_type=q:w=10,bandreject=f=150:width_type=q:w=10",
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
    fn zero_harmonics_notches_only_the_fundamental() {
        assert_eq!(
            build_filter(60.0, 0, 12.0),
            "bandreject=f=60:width_type=q:w=12"
        );
    }

    #[test]
    fn sixty_hz_harmonics_are_multiples_of_sixty() {
        assert_eq!(
            build_filter(60.0, 3, 8.0),
            "bandreject=f=60:width_type=q:w=8,bandreject=f=120:width_type=q:w=8,bandreject=f=180:width_type=q:w=8,bandreject=f=240:width_type=q:w=8"
        );
    }

    #[test]
    fn fractional_q_is_preserved() {
        assert_eq!(
            build_filter(50.0, 0, 2.5),
            "bandreject=f=50:width_type=q:w=2.5"
        );
    }

    #[test]
    fn always_stream_copies_the_video() {
        let (argv, _) = plan("in.mp4", "50", 4, 10.0).unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
        // no -vn: the video track is kept.
        assert!(!argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn webm_input_keeps_webm_and_uses_opus_audio() {
        let (argv, out) = plan("clip.webm", "50", 4, 10.0).unwrap();
        assert_eq!(out, "out.webm");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libopus"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
    }

    #[test]
    fn container_kept_for_copy_capable_and_falls_back_to_mp4() {
        for ext in ["mp4", "mov", "m4v", "mkv", "webm"] {
            let (_, out) = plan(&format!("clip.{ext}"), "50", 4, 10.0).unwrap();
            assert_eq!(out, format!("out.{ext}"));
        }
        assert_eq!(plan("clip.avi", "50", 4, 10.0).unwrap().1, "out.mp4");
        assert_eq!(plan("noext", "50", 4, 10.0).unwrap().1, "out.mp4");
    }

    #[test]
    fn rejects_bad_frequency() {
        assert!(plan("a.mp4", "55", 4, 10.0).is_err());
        let err = plan("a.mp4", "55", 4, 10.0).unwrap_err();
        assert!(err.contains("not supported"));
    }

    #[test]
    fn rejects_out_of_range_harmonics() {
        assert!(plan("a.mp4", "50", -1, 10.0).is_err());
        assert!(plan("a.mp4", "50", 13, 10.0).is_err());
        let err = plan("a.mp4", "50", 99, 10.0).unwrap_err();
        assert!(err.contains("harmonics must be between"));
    }

    #[test]
    fn rejects_out_of_range_q() {
        assert!(plan("a.mp4", "50", 4, 0.5).is_err());
        assert!(plan("a.mp4", "50", 4, 101.0).is_err());
        assert!(plan("a.mp4", "50", 4, f64::NAN).is_err());
        let err = plan("a.mp4", "50", 4, 200.0).unwrap_err();
        assert!(err.contains("q must be between"));
    }
}
