//! gizza-ai/audio-effects-rack core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! A small effects "rack": it chains up to five classic effects in one pass, in
//! a fixed musical signal order — dynamics first, then modulation, then time,
//! then space:
//!
//!   compression → chorus → tremolo → echo (delay) → reverb
//!
//! Each stage is independently switchable; a stage left at its "off" value
//! (enum `none`, or `0` for the numeric knobs) is omitted from the ffmpeg
//! filter chain. A request with every stage off is rejected as a no-op rather
//! than wasting a lossy re-encode on an unchanged file. `-vn` drops any
//! attached-picture (album-art) video stream.
//!
//! Filters used (all stock ffmpeg): `acompressor` (dynamics), `chorus`,
//! `tremolo`, and `aecho` for both the single-tap echo and the multi-tap
//! reverb presets (a lightweight reverb approximation — no impulse-response
//! file needed, so it runs the same everywhere).

/// Output audio formats the rack can write (family-standard set).
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

/// Reverb preset — a multi-tap `aecho` ambience approximation. `none` = the
/// reverb stage is skipped entirely.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Reverb {
    None,
    Room,
    Hall,
    Plate,
}

/// Parse the reverb preset. Empty defaults to `none`.
pub fn parse_reverb(s: &str) -> Result<Reverb, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "none" => Ok(Reverb::None),
        "room" => Ok(Reverb::Room),
        "hall" => Ok(Reverb::Hall),
        "plate" => Ok(Reverb::Plate),
        other => Err(format!(
            "reverb {other:?} not supported (none|room|hall|plate)"
        )),
    }
}

impl Reverb {
    /// The `aecho` filter string for this preset, or `None` when off. Longer
    /// delays + higher decays read as a bigger, more reverberant space.
    fn filter(self) -> Option<String> {
        let f = match self {
            Reverb::None => return None,
            // small, tight room: two short early reflections.
            Reverb::Room => "aecho=0.8:0.9:40|56:0.3|0.2",
            // large hall: longer, louder tail.
            Reverb::Hall => "aecho=0.85:0.9:80|120:0.5|0.4",
            // plate: three dense mid-length taps, bright and even.
            Reverb::Plate => "aecho=0.8:0.9:30|60|90:0.4|0.3|0.2",
        };
        Some(f.to_string())
    }
}

/// Chorus depth preset. `none` = the chorus stage is skipped.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Chorus {
    None,
    Light,
    Deep,
}

/// Parse the chorus preset. Empty defaults to `none`.
pub fn parse_chorus(s: &str) -> Result<Chorus, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "none" => Ok(Chorus::None),
        "light" => Ok(Chorus::Light),
        "deep" => Ok(Chorus::Deep),
        other => Err(format!(
            "chorus {other:?} not supported (none|light|deep)"
        )),
    }
}

impl Chorus {
    /// The `chorus` filter string, or `None` when off. `light` is one gentle
    /// voice; `deep` is two detuned voices for a lush, wide shimmer.
    fn filter(self) -> Option<String> {
        let f = match self {
            Chorus::None => return None,
            Chorus::Light => "chorus=0.7:0.9:55:0.4:0.25:2",
            Chorus::Deep => "chorus=0.6:0.9:50|60:0.4|0.32:0.25|0.4:2|1.3",
        };
        Some(f.to_string())
    }
}

/// Compression preset (dynamic-range, via `acompressor`). `none` = skipped.
/// This evens out loud/quiet swings; it is NOT file-size compression (that's
/// the separate audio-compress tool).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Compression {
    None,
    Light,
    Medium,
    Heavy,
}

/// Parse the compression preset. Empty defaults to `none`.
pub fn parse_compression(s: &str) -> Result<Compression, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "none" => Ok(Compression::None),
        "light" => Ok(Compression::Light),
        "medium" => Ok(Compression::Medium),
        "heavy" => Ok(Compression::Heavy),
        other => Err(format!(
            "compression {other:?} not supported (none|light|medium|heavy)"
        )),
    }
}

impl Compression {
    /// The `acompressor` filter string, or `None` when off. Heavier presets
    /// use a lower threshold + higher ratio + more make-up gain.
    fn filter(self) -> Option<String> {
        let f = match self {
            Compression::None => return None,
            Compression::Light => {
                "acompressor=threshold=-18dB:ratio=2:attack=20:release=250:makeup=2"
            }
            Compression::Medium => {
                "acompressor=threshold=-24dB:ratio=4:attack=10:release=200:makeup=4"
            }
            Compression::Heavy => {
                "acompressor=threshold=-30dB:ratio=8:attack=5:release=150:makeup=6"
            }
        };
        Some(f.to_string())
    }
}

/// Echo delay range (ms). 0 = off; the upper bound keeps the tail inside the
/// ffmpeg `aecho` sanity range and avoids absurdly long single taps.
pub const MAX_ECHO_MS: f64 = 1000.0;
/// Tremolo rate range (Hz). 0 = off. Above ~20 Hz the modulation stops sounding
/// like tremolo and starts adding audible sidebands.
pub const MAX_TREMOLO_HZ: f64 = 20.0;
/// Tremolo below this rate is effectively DC (one slow swell) — `tremolo`
/// itself rejects `f < 0.1`, so we treat anything in `(0, 0.1)` as an error
/// rather than silently snapping it.
pub const MIN_TREMOLO_HZ: f64 = 0.1;

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`250` not
/// `250.0`, `5.5` stays `5.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Build the single-tap echo filter for a delay of `ms`, or `None` when 0.
/// Fixed input/output gains + a 0.5 decay give a clear, non-runaway repeat.
fn echo_filter(ms: f64) -> Result<Option<String>, String> {
    if ms == 0.0 {
        return Ok(None);
    }
    if !ms.is_finite() || ms < 0.0 || ms > MAX_ECHO_MS {
        return Err(format!(
            "echo delay must be 0 (off) to {MAX_ECHO_MS} ms, got {ms}"
        ));
    }
    Ok(Some(format!("aecho=0.8:0.88:{}:0.5", fmt_num(ms))))
}

/// Build the tremolo filter for a rate of `hz`, or `None` when 0. Depth is
/// fixed at a musical 0.7 (70% amplitude sweep).
fn tremolo_filter(hz: f64) -> Result<Option<String>, String> {
    if hz == 0.0 {
        return Ok(None);
    }
    if !hz.is_finite() || hz < MIN_TREMOLO_HZ || hz > MAX_TREMOLO_HZ {
        return Err(format!(
            "tremolo rate must be 0 (off) or {MIN_TREMOLO_HZ} to {MAX_TREMOLO_HZ} Hz, got {hz}"
        ));
    }
    Ok(Some(format!("tremolo=f={}:d=0.7", fmt_num(hz))))
}

/// Build the `-af` chain from every enabled stage, in signal order
/// (compression → chorus → tremolo → echo → reverb). Returns an error when no
/// stage is enabled (a pointless re-encode).
pub fn build_filter(
    reverb: Reverb,
    echo_ms: f64,
    chorus: Chorus,
    tremolo_hz: f64,
    compression: Compression,
) -> Result<String, String> {
    let mut stages: Vec<String> = Vec::new();
    if let Some(f) = compression.filter() {
        stages.push(f);
    }
    if let Some(f) = chorus.filter() {
        stages.push(f);
    }
    if let Some(f) = tremolo_filter(tremolo_hz)? {
        stages.push(f);
    }
    if let Some(f) = echo_filter(echo_ms)? {
        stages.push(f);
    }
    if let Some(f) = reverb.filter() {
        stages.push(f);
    }
    if stages.is_empty() {
        return Err(
            "no effects selected — turn on at least one of reverb, echo, chorus, tremolo or \
             compression (e.g. reverb=hall, or echo=250 for a quarter-second slapback)"
                .to_string(),
        );
    }
    Ok(stages.join(","))
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to process `in_name` into
/// `out_name`. Shared verbatim by the web page (`build_argv`) and the chat block.
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

/// Parse every preset/knob, validate, and return `(argv, out_name)` for an
/// input file. `out_name` is `out.<ext>` for the chosen format. Single source
/// shared by the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_effects(
    in_name: &str,
    reverb: &str,
    echo_ms: f64,
    chorus: &str,
    tremolo_hz: f64,
    compression: &str,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    let reverb = parse_reverb(reverb)?;
    let chorus = parse_chorus(chorus)?;
    let compression = parse_compression(compression)?;
    let fmt = parse_format(format)?;
    let filter = build_filter(reverb, echo_ms, chorus, tremolo_hz, compression)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, &filter, fmt), out_name))
}

/// Human-readable one-line summary of the active stages (for the chat/LLM
/// envelope). Empty stages are omitted; the caller guarantees ≥1 is active.
pub fn describe_stages(
    reverb: &str,
    echo_ms: f64,
    chorus: &str,
    tremolo_hz: f64,
    compression: &str,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    if matches!(parse_compression(compression), Ok(c) if c != Compression::None) {
        parts.push(format!("{} compression", compression.trim().to_lowercase()));
    }
    if matches!(parse_chorus(chorus), Ok(c) if c != Chorus::None) {
        parts.push(format!("{} chorus", chorus.trim().to_lowercase()));
    }
    if tremolo_hz != 0.0 {
        parts.push(format!("{} Hz tremolo", fmt_num(tremolo_hz)));
    }
    if echo_ms != 0.0 {
        parts.push(format!("{} ms echo", fmt_num(echo_ms)));
    }
    if matches!(parse_reverb(reverb), Ok(r) if r != Reverb::None) {
        parts.push(format!("{} reverb", reverb.trim().to_lowercase()));
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_chain_argv_order_and_values() {
        let (argv, out) =
            plan_effects("in.mp3", "hall", 250.0, "light", 5.0, "medium", "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                // signal order: compression → chorus → tremolo → echo → reverb
                "acompressor=threshold=-24dB:ratio=4:attack=10:release=200:makeup=4,\
                 chorus=0.7:0.9:55:0.4:0.25:2,\
                 tremolo=f=5:d=0.7,\
                 aecho=0.8:0.88:250:0.5,\
                 aecho=0.85:0.9:80|120:0.5|0.4",
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
    fn single_stage_only_that_stage_in_chain() {
        assert_eq!(
            build_filter(Reverb::Room, 0.0, Chorus::None, 0.0, Compression::None).unwrap(),
            "aecho=0.8:0.9:40|56:0.3|0.2"
        );
        assert_eq!(
            build_filter(Reverb::None, 0.0, Chorus::None, 0.0, Compression::Heavy).unwrap(),
            "acompressor=threshold=-30dB:ratio=8:attack=5:release=150:makeup=6"
        );
        assert_eq!(
            build_filter(Reverb::None, 500.0, Chorus::None, 0.0, Compression::None).unwrap(),
            "aecho=0.8:0.88:500:0.5"
        );
    }

    #[test]
    fn all_off_is_rejected_as_a_no_op() {
        let err = plan_effects("in.mp3", "none", 0.0, "none", 0.0, "none", "mp3").unwrap_err();
        assert!(err.contains("no effects selected"), "{err}");
    }

    #[test]
    fn out_of_range_echo_and_tremolo_are_rejected() {
        assert!(plan_effects("a.mp3", "none", 1001.0, "none", 0.0, "none", "mp3").is_err());
        assert!(plan_effects("a.mp3", "none", -5.0, "none", 0.0, "none", "mp3").is_err());
        assert!(plan_effects("a.mp3", "none", 0.0, "none", 25.0, "none", "mp3").is_err());
        assert!(plan_effects("a.mp3", "none", 0.0, "none", 0.05, "none", "mp3").is_err());
        // Boundaries are valid.
        assert!(plan_effects("a.mp3", "none", 1000.0, "none", 20.0, "none", "mp3").is_ok());
        assert!(plan_effects("a.mp3", "none", 0.0, "none", 0.1, "none", "mp3").is_ok());
    }

    #[test]
    fn unknown_presets_are_rejected() {
        assert!(plan_effects("a.mp3", "cave", 0.0, "none", 0.0, "none", "mp3").is_err());
        assert!(plan_effects("a.mp3", "none", 0.0, "warble", 0.0, "none", "mp3").is_err());
        assert!(plan_effects("a.mp3", "none", 0.0, "none", 0.0, "brutal", "mp3").is_err());
        assert!(plan_effects("a.mp3", "room", 0.0, "none", 0.0, "none", "aiff").is_err());
    }

    #[test]
    fn deep_chorus_uses_two_voices() {
        let f = build_filter(Reverb::None, 0.0, Chorus::Deep, 0.0, Compression::None).unwrap();
        assert_eq!(f, "chorus=0.6:0.9:50|60:0.4|0.32:0.25|0.4:2|1.3");
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
            let (argv, _) = plan_effects("in.mp3", "room", 0.0, "none", 0.0, "none", f).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        let (argv, _) = plan_effects("in.mp3", "plate", 0.0, "none", 0.0, "none", "wav").unwrap();
        assert!(argv.iter().any(|a| a == "-vn"));
    }

    #[test]
    fn out_name_tracks_the_format() {
        let (_, out) = plan_effects("in.wav", "room", 0.0, "none", 0.0, "none", "flac").unwrap();
        assert_eq!(out, "out.flac");
    }

    #[test]
    fn fmt_num_compact() {
        assert_eq!(fmt_num(250.0), "250");
        assert_eq!(fmt_num(5.5), "5.5");
        assert_eq!(fmt_num(0.1), "0.1");
    }

    #[test]
    fn describe_stages_lists_active_only_in_order() {
        assert_eq!(
            describe_stages("hall", 250.0, "light", 5.0, "medium"),
            "medium compression, light chorus, 5 Hz tremolo, 250 ms echo, hall reverb"
        );
        assert_eq!(
            describe_stages("none", 0.0, "none", 0.0, "heavy"),
            "heavy compression"
        );
        assert_eq!(
            describe_stages("room", 0.0, "none", 0.0, "none"),
            "room reverb"
        );
    }

    #[test]
    fn parse_helpers_default_empty() {
        assert_eq!(parse_reverb("").unwrap(), Reverb::None);
        assert_eq!(parse_chorus("").unwrap(), Chorus::None);
        assert_eq!(parse_compression("").unwrap(), Compression::None);
        assert_eq!(parse_format("").unwrap(), Format::Mp3);
        assert_eq!(parse_reverb("HALL").unwrap(), Reverb::Hall);
    }
}
