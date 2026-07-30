//! gizza-ai/chorus core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Thickens a sound by mixing in several short, pitch-modulated delay voices —
//! the classic chorus effect. Built on stock ffmpeg's `chorus` filter,
//! `chorus=in_gain:out_gain:delays:decays:speeds:depths`, where the last four
//! lists carry one `|`-separated entry per voice. The dry input is kept at a
//! fixed `in_gain` and the whole mix scaled by a fixed `out_gain`; the
//! user-facing controls drive the per-voice `delays`/`decays`/`speeds`/`depths`.
//! Each extra voice is staggered (later start delay, slightly faster
//! modulation) so voices stay distinct instead of phase-cancelling into one.
//! Inputs are capped at 10 MiB. `-vn` drops attached-picture streams (album art).

/// Output audio formats chorus can write (family-standard set).
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

// Fixed mix levels: dry signal at 0.6, whole result scaled by 0.9 so a stack of
// voices stays below clipping. These aren't user-facing — the per-voice level is
// controlled by `decay`.
const IN_GAIN: f64 = 0.6;
const OUT_GAIN: f64 = 0.9;

/// Per-voice delay stagger (ms): voice `i` starts `i * VOICE_DELAY_STEP_MS`
/// later than the base so voices don't line up and cancel.
const VOICE_DELAY_STEP_MS: f64 = 8.0;
/// Per-voice speed spread: voice `i` modulates at `base * (1 + i * this)` Hz so
/// the copies drift against one another instead of locking in phase.
const VOICE_SPEED_STEP: f64 = 0.3;

// Defaults (kept in sync with the descriptor + the drift-guard schema).
pub const DEFAULT_VOICES: i64 = 2;
pub const DEFAULT_DELAY_MS: f64 = 50.0;
pub const DEFAULT_DEPTH_MS: f64 = 2.0;
pub const DEFAULT_SPEED_HZ: f64 = 0.4;
pub const DEFAULT_DECAY: f64 = 0.4;

// Accepted ranges (kept in sync with the descriptor's .min()/.max()).
pub const MIN_VOICES: i64 = 2;
pub const MAX_VOICES: i64 = 4;
pub const MIN_DELAY_MS: f64 = 20.0;
pub const MAX_DELAY_MS: f64 = 80.0;
pub const MIN_DEPTH_MS: f64 = 1.0;
pub const MAX_DEPTH_MS: f64 = 8.0;
pub const MIN_SPEED_HZ: f64 = 0.1;
pub const MAX_SPEED_HZ: f64 = 5.0;
pub const MIN_DECAY: f64 = 0.1;
pub const MAX_DECAY: f64 = 0.9;

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

fn validate_range(name: &str, v: f64, lo: f64, hi: f64) -> Result<(), String> {
    if !v.is_finite() || !(lo..=hi).contains(&v) {
        return Err(format!(
            "{name} must be between {} and {}, got {v}",
            fmt_num(lo),
            fmt_num(hi)
        ));
    }
    Ok(())
}

/// Build the `chorus=…` filter string for `voices` voices. Assumes the inputs
/// have already been validated (see `plan_chorus`). Voice `i` uses delay
/// `delay_ms + i*8`, the same `decay`/`depth_ms`, and speed `speed_hz*(1+i*0.3)`.
pub fn build_chorus_filter(
    voices: i64,
    delay_ms: f64,
    depth_ms: f64,
    speed_hz: f64,
    decay: f64,
) -> String {
    let n = voices.max(1) as usize;
    let mut delays = Vec::with_capacity(n);
    let mut decays = Vec::with_capacity(n);
    let mut speeds = Vec::with_capacity(n);
    let mut depths = Vec::with_capacity(n);
    for i in 0..n {
        let fi = i as f64;
        delays.push(fmt_num(delay_ms + fi * VOICE_DELAY_STEP_MS));
        decays.push(fmt_num(decay));
        speeds.push(fmt_num(speed_hz * (1.0 + fi * VOICE_SPEED_STEP)));
        depths.push(fmt_num(depth_ms));
    }
    format!(
        "chorus={}:{}:{}:{}:{}:{}",
        fmt_num(IN_GAIN),
        fmt_num(OUT_GAIN),
        delays.join("|"),
        decays.join("|"),
        speeds.join("|"),
        depths.join("|"),
    )
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to apply the chorus `filter` to
/// `in_name`, writing `out_name`. Shared verbatim by the web page (`build_argv`)
/// and the chat block.
pub fn build_argv(in_name: &str, out_name: &str, filter: &str, format: Format) -> Vec<String> {
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-af".to_string(),
        filter.to_string(),
    ];
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate every control, parse `format`, and return `(argv, out_name)`.
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`). `voices` is a whole number (2-4).
pub fn plan_chorus(
    in_name: &str,
    voices: i64,
    delay_ms: f64,
    depth_ms: f64,
    speed_hz: f64,
    decay: f64,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    if !(MIN_VOICES..=MAX_VOICES).contains(&voices) {
        return Err(format!(
            "voices must be a whole number between {MIN_VOICES} and {MAX_VOICES}, got {voices}"
        ));
    }
    validate_range("delay_ms", delay_ms, MIN_DELAY_MS, MAX_DELAY_MS)?;
    validate_range("depth_ms", depth_ms, MIN_DEPTH_MS, MAX_DEPTH_MS)?;
    validate_range("speed_hz", speed_hz, MIN_SPEED_HZ, MAX_SPEED_HZ)?;
    validate_range("decay", decay, MIN_DECAY, MAX_DECAY)?;
    let fmt = parse_format(format)?;
    let filter = build_chorus_filter(voices, delay_ms, depth_ms, speed_hz, decay);
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, &filter, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_two_voice_argv_order_and_values() {
        let (argv, out) = plan_chorus(
            "in.mp3",
            DEFAULT_VOICES,
            DEFAULT_DELAY_MS,
            DEFAULT_DEPTH_MS,
            DEFAULT_SPEED_HZ,
            DEFAULT_DECAY,
            "mp3",
        )
        .unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                // voice 0: delay 50, speed 0.4; voice 1: delay 58, speed 0.52
                "chorus=0.6:0.9:50|58:0.4|0.4:0.4|0.52:2|2",
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
    fn voice_count_drives_the_number_of_entries() {
        let f = build_chorus_filter(4, 40.0, 3.0, 0.5, 0.5);
        // 4 voices → 4 pipe-joined entries in each of the last four lists.
        let body = f.strip_prefix("chorus=0.6:0.9:").unwrap();
        for list in body.split(':') {
            assert_eq!(
                list.split('|').count(),
                4,
                "list {list:?} must have 4 voices"
            );
        }
        // delays staggered by 8 ms; speeds spread per voice.
        assert_eq!(
            f,
            "chorus=0.6:0.9:40|48|56|64:0.5|0.5|0.5|0.5:0.5|0.65|0.8|0.95:3|3|3|3"
        );
    }

    #[test]
    fn single_voice_filter_has_no_pipes() {
        // build_chorus_filter itself accepts 1 (internal edge); one voice
        // produces flat lists with no `|`.
        let f = build_chorus_filter(1, 50.0, 2.0, 0.4, 0.4);
        assert_eq!(f, "chorus=0.6:0.9:50:0.4:0.4:2");
    }

    #[test]
    fn out_of_range_controls_are_rejected_by_name() {
        // voices below/above range
        assert!(plan_chorus("a.mp3", 1, 50.0, 2.0, 0.4, 0.4, "mp3").is_err());
        assert!(plan_chorus("a.mp3", 5, 50.0, 2.0, 0.4, 0.4, "mp3").is_err());
        // each numeric control, one over the top edge, names itself
        for (bad, needle) in [
            ((90.0, 2.0, 0.4, 0.4), "delay_ms"),
            ((50.0, 9.0, 0.4, 0.4), "depth_ms"),
            ((50.0, 2.0, 6.0, 0.4), "speed_hz"),
            ((50.0, 2.0, 0.4, 1.0), "decay"),
        ] {
            let (d, dep, sp, dc) = bad;
            let err = plan_chorus("a.mp3", 2, d, dep, sp, dc, "mp3").unwrap_err();
            assert!(err.contains(needle), "expected {needle} in {err:?}");
        }
        // NaN is rejected
        assert!(plan_chorus("a.mp3", 2, f64::NAN, 2.0, 0.4, 0.4, "mp3").is_err());
    }

    #[test]
    fn range_boundaries_are_valid() {
        assert!(plan_chorus(
            "a.mp3",
            MIN_VOICES,
            MIN_DELAY_MS,
            MIN_DEPTH_MS,
            MIN_SPEED_HZ,
            MIN_DECAY,
            "mp3"
        )
        .is_ok());
        assert!(plan_chorus(
            "a.mp3",
            MAX_VOICES,
            MAX_DELAY_MS,
            MAX_DEPTH_MS,
            MAX_SPEED_HZ,
            MAX_DECAY,
            "flac"
        )
        .is_ok());
    }

    #[test]
    fn every_format_maps_to_its_codec() {
        for (f, codec) in [
            ("mp3", "libmp3lame"),
            ("wav", "pcm_s16le"),
            ("ogg", "libvorbis"),
            ("flac", "flac"),
            ("m4a", "aac"),
        ] {
            let (argv, out) = plan_chorus("in.wav", 2, 50.0, 2.0, 0.4, 0.4, f).unwrap();
            assert!(out.ends_with(f), "out name {out} uses .{f}");
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_chorus("in.mp3", 2, 50.0, 2.0, 0.4, 0.4, "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn bad_format_is_rejected() {
        let err = plan_chorus("in.mp3", 2, 50.0, 2.0, 0.4, 0.4, "aiff").unwrap_err();
        assert!(err.contains("not supported"), "{err}");
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(3.0), "3");
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(fmt_num(0.52), "0.52");
        assert_eq!(fmt_num(0.65), "0.65");
    }
}
