//! gizza-ai/video-audio-gain core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Raises or lowers a video's audio volume with ffmpeg's `volume` filter —
//! `volume=6dB` (decibels) or `volume=2` (linear factor). The picture is
//! stream-copied (`-c:v copy`, lossless, fast); only the audio is re-encoded
//! (required, since the `volume` filter rewrites samples). An optional
//! `alimiter` stage (on by default) caps peaks at 0 dBFS so boosts don't clip.
//! The output keeps the input container; the audio codec is chosen to match it
//! (webm → libopus, everything else → aac).

use gizza_ai_block_utils::ffmpeg::copy_out_ext;

/// How `amount` is interpreted.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Unit {
    /// Decibels: +6 roughly doubles perceived level, -6 halves it.
    Db,
    /// Linear factor: 2 doubles the amplitude, 0.5 halves it.
    Factor,
}

/// Parse the user-facing unit string. Empty defaults to db.
pub fn parse_unit(s: &str) -> Result<Unit, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "db" => Ok(Unit::Db),
        "factor" => Ok(Unit::Factor),
        other => Err(format!("unit {other:?} not supported (db|factor)")),
    }
}

/// Accepted gain ranges: ±60 dB, or a linear factor in (0, 16].
pub const MAX_DB: f64 = 60.0;
pub const MAX_FACTOR: f64 = 16.0;

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

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`6` not `6.0`,
/// `1.5` stays `1.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Build the `-af` chain: the `volume` gain stage plus, when `limiter` is on,
/// an `alimiter` stage that caps peaks at 0 dBFS so boosts don't clip.
pub fn build_filter(amount: f64, unit: Unit, limiter: bool) -> String {
    let mut f = match unit {
        Unit::Db => format!("volume={}dB", fmt_num(amount)),
        Unit::Factor => format!("volume={}", fmt_num(amount)),
    };
    if limiter {
        f.push_str(",alimiter=limit=1:level=disabled");
    }
    f
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to re-gain `in_name`'s audio into
/// `out_name`, keeping the picture (`-c:v copy`). Shared verbatim by the web
/// page (`build_argv`) and the chat block.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    amount: f64,
    unit: Unit,
    limiter: bool,
) -> Vec<String> {
    let out_ext = out_name.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    vec![
        "-i".to_string(),
        in_name.to_string(),
        "-c:v".to_string(),
        "copy".to_string(),
        "-af".to_string(),
        build_filter(amount, unit, limiter),
        "-c:a".to_string(),
        audio_codec(out_ext).to_string(),
        out_name.to_string(),
    ]
}

/// Validate `amount` for the given `unit`, parse everything, and return
/// `(argv, out_name)`. `out_name` keeps the input container when it can hold a
/// copied video stream; otherwise it is `out.mp4`. Single source shared by the
/// chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan(
    in_name: &str,
    amount: f64,
    unit: &str,
    limiter: bool,
) -> Result<(Vec<String>, String), String> {
    let u = parse_unit(unit)?;
    match u {
        Unit::Db => {
            if !amount.is_finite() || amount.abs() > MAX_DB {
                return Err(format!(
                    "amount must be between -{MAX_DB} and {MAX_DB} dB, got {amount}"
                ));
            }
            if amount == 0.0 {
                return Err("amount of 0 dB wouldn't change anything — use e.g. 6 to boost or -6 to cut".into());
            }
        }
        Unit::Factor => {
            if !amount.is_finite() || amount <= 0.0 || amount > MAX_FACTOR {
                return Err(format!(
                    "amount must be a factor in (0, {MAX_FACTOR}] (e.g. 2 doubles, 0.5 halves), got {amount}"
                ));
            }
            if amount == 1.0 {
                return Err(
                    "amount factor 1 wouldn't change anything — use e.g. 2 to double or 0.5 to halve"
                        .into(),
                );
            }
        }
    }
    let out_name = format!("out.{}", copy_out_ext(in_name));
    Ok((build_argv(in_name, &out_name, amount, u, limiter), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_argv_order_and_values() {
        let (argv, out) = plan("in.mp4", 6.0, "db", true).unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp4",
                "-c:v",
                "copy",
                "-af",
                "volume=6dB,alimiter=limit=1:level=disabled",
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
    fn factor_unit_omits_db_suffix() {
        assert_eq!(build_filter(2.0, Unit::Factor, false), "volume=2");
        assert_eq!(build_filter(0.5, Unit::Factor, false), "volume=0.5");
    }

    #[test]
    fn limiter_appends_alimiter_stage() {
        assert_eq!(build_filter(-12.0, Unit::Db, false), "volume=-12dB");
        assert_eq!(
            build_filter(-12.0, Unit::Db, true),
            "volume=-12dB,alimiter=limit=1:level=disabled"
        );
    }

    #[test]
    fn negative_db_cuts_are_accepted() {
        let (argv, _) = plan("in.mp4", -20.0, "db", false).unwrap();
        assert!(argv.iter().any(|a| a == "volume=-20dB"));
    }

    #[test]
    fn always_stream_copies_the_video() {
        let (argv, _) = plan("in.mp4", 6.0, "db", true).unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
        // no -vn: the video track is kept.
        assert!(!argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn webm_input_keeps_webm_and_uses_opus_audio() {
        let (argv, out) = plan("clip.webm", 3.0, "db", true).unwrap();
        assert_eq!(out, "out.webm");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libopus"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"));
    }

    #[test]
    fn container_kept_for_copy_capable_and_falls_back_to_mp4() {
        for ext in ["mp4", "mov", "m4v", "mkv", "webm"] {
            let (_, out) = plan(&format!("clip.{ext}"), 6.0, "db", false).unwrap();
            assert_eq!(out, format!("out.{ext}"));
        }
        // Unknown/absent extension → mp4.
        assert_eq!(plan("clip.avi", 6.0, "db", false).unwrap().1, "out.mp4");
        assert_eq!(plan("noext", 6.0, "db", false).unwrap().1, "out.mp4");
    }

    #[test]
    fn rejects_no_op_and_out_of_range_amounts() {
        assert!(plan("a.mp4", 0.0, "db", true).is_err());
        assert!(plan("a.mp4", 61.0, "db", true).is_err());
        assert!(plan("a.mp4", f64::NAN, "db", true).is_err());
        assert!(plan("a.mp4", 1.0, "factor", true).is_err());
        assert!(plan("a.mp4", 0.0, "factor", true).is_err());
        assert!(plan("a.mp4", 17.0, "factor", true).is_err());
        let err = plan("a.mp4", 0.0, "db", true).unwrap_err();
        assert!(err.contains("wouldn't change anything"));
    }

    #[test]
    fn rejects_unknown_unit() {
        assert!(plan("a.mp4", 6.0, "percent", true).is_err());
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(6.0), "6");
        assert_eq!(fmt_num(-12.0), "-12");
        assert_eq!(fmt_num(1.5), "1.5");
        assert_eq!(fmt_num(0.25), "0.25");
    }
}
