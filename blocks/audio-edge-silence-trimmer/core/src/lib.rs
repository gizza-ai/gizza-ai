//! gizza-ai/audio-edge-silence-trimmer core — pure ffmpeg argv construction
//! shared by the chat skill block and the standalone web page. No wafer/
//! wasm-bindgen deps.
//!
//! Removes ONLY the leading and trailing silence below a threshold, leaving the
//! body — including any pauses in the middle — completely untouched. Unlike
//! audio-silence-remove (which strips every gap with `stop_periods=-1`), this
//! uses the `silenceremove … areverse … silenceremove … areverse` idiom: trim
//! leading silence, reverse, trim what-was-trailing silence, reverse back. No
//! `stop_periods`, so internal quiet passages survive intact. A small `pad` of
//! silence is kept at each edge (`start_silence`) so the cut isn't abrupt.
//! `-vn` drops attached-picture streams (album art).

/// Output audio formats this tool can write (family-standard set).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Mp3,
    Wav,
    Ogg,
    Flac,
    M4a,
}

impl Format {
    /// Lower-cased file extension this format writes (used for `out.<ext>`).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Mp3 => "mp3",
            Format::Wav => "wav",
            Format::Ogg => "ogg",
            Format::Flac => "flac",
            Format::M4a => "m4a",
        }
    }

    /// IANA media type for the produced file.
    pub fn mime(self) -> &'static str {
        match self {
            Format::Mp3 => "audio/mpeg",
            Format::Wav => "audio/wav",
            Format::Ogg => "audio/ogg",
            Format::Flac => "audio/flac",
            Format::M4a => "audio/mp4",
        }
    }

    /// Encoder argv fragment (`-c:a …`); lossy formats are fixed at 192 kbps.
    fn codec_args(self) -> Vec<String> {
        match self {
            Format::Mp3 => vec![
                "-c:a".into(),
                "libmp3lame".into(),
                "-b:a".into(),
                "192k".into(),
            ],
            Format::Wav => vec!["-c:a".into(), "pcm_s16le".into()],
            Format::Ogg => vec![
                "-c:a".into(),
                "libvorbis".into(),
                "-b:a".into(),
                "192k".into(),
            ],
            Format::Flac => vec!["-c:a".into(), "flac".into()],
            Format::M4a => vec!["-c:a".into(), "aac".into()],
        }
    }
}

/// Parse the user-facing format string. Empty defaults to mp3.
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "mp3" => Ok(Format::Mp3),
        "wav" => Ok(Format::Wav),
        "ogg" => Ok(Format::Ogg),
        "flac" => Ok(Format::Flac),
        "m4a" => Ok(Format::M4a),
        other => Err(format!(
            "format {other:?} not supported (mp3|wav|ogg|flac|m4a)"
        )),
    }
}

/// Audio quieter than -50 dB counts as silence at the edges. Edge trimming can
/// use a quieter cutoff than gap-removal because it only ever touches the very
/// start/end, so faint room tone doesn't get mistaken for the body.
pub const DEFAULT_THRESHOLD_DB: f64 = -50.0;
/// Seconds of silence kept at each edge so the trim isn't abrupt.
pub const DEFAULT_PAD_S: f64 = 0.1;

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`-50` not
/// `-50.0`, `0.1` stays `0.1`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Build the edge-only `-af` filter string. One `silenceremove` trims leading
/// silence (`start_periods=1`, keeping `pad_s` seconds), `areverse` flips the
/// stream so the original TAIL is now the head, a second identical
/// `silenceremove` trims it, and a final `areverse` restores playback order.
/// No `stop_periods` is used, so silence in the middle of the recording is
/// never touched.
pub fn build_filter(threshold_db: f64, pad_s: f64) -> String {
    let t = fmt_num(threshold_db);
    let k = fmt_num(pad_s);
    let one =
        format!("silenceremove=start_periods=1:start_threshold={t}dB:start_silence={k}:detection=peak");
    format!("{one},areverse,{one},areverse")
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to edge-trim `in_name` into
/// `out_name`. Shared verbatim by the web page (`build_argv`) and the chat block.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    threshold_db: f64,
    pad_s: f64,
    format: Format,
) -> Vec<String> {
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        build_filter(threshold_db, pad_s),
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate params, parse `format`, and return `(argv, out_name)`. Single
/// source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
pub fn plan_edge_trim(
    in_name: &str,
    threshold_db: f64,
    pad_s: f64,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    if !threshold_db.is_finite() || threshold_db > 0.0 {
        return Err(format!(
            "threshold_db must be <= 0 dB and finite (silence is below 0 dB; e.g. -50), got {threshold_db}"
        ));
    }
    if !pad_s.is_finite() || pad_s < 0.0 {
        return Err(format!(
            "pad must be >= 0 seconds and finite, got {pad_s}"
        ));
    }
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((
        build_argv(in_name, &out_name, threshold_db, pad_s, fmt),
        out_name,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_order_and_default_filter() {
        let (argv, out) = plan_edge_trim("in.mp3", -50.0, 0.1, "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "silenceremove=start_periods=1:start_threshold=-50dB:start_silence=0.1:detection=peak,areverse,silenceremove=start_periods=1:start_threshold=-50dB:start_silence=0.1:detection=peak,areverse",
                "-c:a",
                "libmp3lame",
                "-b:a",
                "192k",
                "out.mp3",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn filter_has_no_stop_periods_so_body_is_kept() {
        // The defining property: never emit stop_periods (which would remove
        // middle gaps too, i.e. audio-silence-remove behaviour).
        let f = build_filter(-40.0, 0.25);
        assert!(!f.contains("stop_periods"), "edge trim must not use stop_periods: {f}");
        assert_eq!(f.matches("areverse").count(), 2, "needs the reverse-trim-reverse idiom");
    }

    #[test]
    fn fmt_num_drops_trailing_zero_but_keeps_fractions() {
        assert_eq!(fmt_num(-50.0), "-50");
        assert_eq!(fmt_num(0.1), "0.1");
        assert_eq!(fmt_num(-40.5), "-40.5");
        assert_eq!(fmt_num(0.25), "0.25");
    }

    #[test]
    fn zero_pad_is_valid_hard_cut() {
        let (_, out) = plan_edge_trim("in.wav", -50.0, 0.0, "wav").unwrap();
        assert_eq!(out, "out.wav");
    }

    #[test]
    fn rejects_positive_threshold() {
        let err = plan_edge_trim("in.mp3", 5.0, 0.1, "mp3").unwrap_err();
        assert!(err.contains("threshold_db"), "{err}");
    }

    #[test]
    fn rejects_negative_pad() {
        let err = plan_edge_trim("in.mp3", -50.0, -1.0, "mp3").unwrap_err();
        assert!(err.contains("pad"), "{err}");
    }

    #[test]
    fn rejects_unknown_format() {
        let err = plan_edge_trim("in.mp3", -50.0, 0.1, "aiff").unwrap_err();
        assert!(err.contains("not supported"), "{err}");
    }

    #[test]
    fn each_format_maps_to_ext_and_mime() {
        for (s, ext, mime) in [
            ("mp3", "mp3", "audio/mpeg"),
            ("wav", "wav", "audio/wav"),
            ("ogg", "ogg", "audio/ogg"),
            ("flac", "flac", "audio/flac"),
            ("m4a", "m4a", "audio/mp4"),
        ] {
            let f = parse_format(s).unwrap();
            assert_eq!(f.ext(), ext);
            assert_eq!(f.mime(), mime);
        }
    }
}
