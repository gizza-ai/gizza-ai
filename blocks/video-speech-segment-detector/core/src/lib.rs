//! gizza-ai/video-speech-segment-detector core — pure, native-testable logic
//! for energy-based voice-activity detection over an ffmpeg `silencedetect`
//! log. No wafer/wasm-bindgen deps.
//!
//! # How it works (single detect pass, no re-encode)
//!
//! 1. **detect** — run ffmpeg's `silencedetect` audio filter (optionally after
//!    a 200–3000 Hz band-pass that focuses the loudness measurement on the
//!    voice band, so rumble/hiss don't count as speech). No output file
//!    (`-f null -`); the result is the log with `silence_start`/`silence_end`
//!    markers. Log parsing + the silence complement are shared with
//!    `video-silence-cut`'s core (path dep) — same detect pass, different
//!    deliverable: that tool cuts, this one reports.
//! 2. **segment** — speech = complement of the detected silences, then drop
//!    speech bursts shorter than `min_speech`, pad each remaining segment by
//!    `pad` seconds on both sides (clamped, overlaps merged), and interleave
//!    with the non-speech gaps to a full labeled timeline.
//! 3. **render** — format the timeline as a human report, CSV, SRT, or an
//!    Audacity label track, optionally filtered to speech / non-speech rows.

use gizza_ai_video_silence_cut_core as silence_core;
pub use silence_core::{fmt_num, parse_duration, parse_silences, Silence};

pub const DEFAULT_THRESHOLD_DB: f64 = -30.0;
pub const DEFAULT_MIN_SILENCE_S: f64 = 0.5;
pub const DEFAULT_MIN_SPEECH_S: f64 = 0.25;
pub const DEFAULT_PAD_S: f64 = 0.0;

/// Segments (or gaps) shorter than this are noise from float arithmetic, not
/// real timeline pieces — matches video-silence-cut's sub-ms sliver rule.
const MIN_GAP_S: f64 = 1e-3;

/// One labeled piece of the timeline. `speech == false` is a non-speech gap.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub speech: bool,
}

impl Segment {
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
    pub fn label(&self) -> &'static str {
        if self.speech {
            "speech"
        } else {
            "non-speech"
        }
    }
}

/// Which rows the rendered output lists (the summary always covers both).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentFilter {
    Both,
    Speech,
    NonSpeech,
}

impl SegmentFilter {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "both" => Ok(Self::Both),
            "speech" => Ok(Self::Speech),
            "non-speech" => Ok(Self::NonSpeech),
            other => Err(format!(
                "unknown segments value '{other}' (expected both, speech, or non-speech)"
            )),
        }
    }
    fn keeps(self, seg: &Segment) -> bool {
        match self {
            Self::Both => true,
            Self::Speech => seg.speech,
            Self::NonSpeech => !seg.speech,
        }
    }
}

/// Output format for [`render`]. `extension()` drives the download filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Report,
    Csv,
    Srt,
    Audacity,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "report" => Ok(Self::Report),
            "csv" => Ok(Self::Csv),
            "srt" => Ok(Self::Srt),
            "audacity" => Ok(Self::Audacity),
            other => Err(format!(
                "unknown output value '{other}' (expected report, csv, srt, or audacity)"
            )),
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            Self::Report | Self::Audacity => "txt",
            Self::Csv => "csv",
            Self::Srt => "srt",
        }
    }
    pub fn mime(self) -> &'static str {
        match self {
            Self::Report | Self::Audacity => "text/plain",
            Self::Csv => "text/csv",
            Self::Srt => "application/x-subrip",
        }
    }
}

/// Validate user params. Thresholds are dB (≤ 0); durations are seconds.
pub fn validate(
    threshold_db: f64,
    min_silence_s: f64,
    min_speech_s: f64,
    pad_s: f64,
) -> Result<(), String> {
    if !threshold_db.is_finite() || threshold_db > 0.0 {
        return Err("threshold_db must be a finite value <= 0 dB (e.g. -30)".into());
    }
    if !min_silence_s.is_finite() || min_silence_s <= 0.0 {
        return Err("min_silence must be a finite number > 0 (seconds)".into());
    }
    if !min_speech_s.is_finite() || min_speech_s < 0.0 {
        return Err("min_speech must be a finite number >= 0 (seconds)".into());
    }
    if !pad_s.is_finite() || pad_s < 0.0 {
        return Err("pad must be a finite number >= 0 (seconds)".into());
    }
    Ok(())
}

/// Detect-pass argv (no leading "ffmpeg"): silencedetect over the audio only
/// (`-vn -sn -dn` skips decoding the picture — analysis is much faster), no
/// output file. `voice_band` inserts a 200–3000 Hz band-pass in front of the
/// detector so only voice-band energy counts.
pub fn detect_argv(
    in_name: &str,
    threshold_db: f64,
    min_silence_s: f64,
    voice_band: bool,
) -> Vec<String> {
    let detect = format!(
        "silencedetect=noise={}dB:d={}",
        fmt_num(threshold_db),
        fmt_num(min_silence_s)
    );
    let af = if voice_band {
        format!("highpass=f=200,lowpass=f=3000,{detect}")
    } else {
        detect
    };
    vec![
        "-i".into(),
        in_name.into(),
        "-vn".into(),
        "-sn".into(),
        "-dn".into(),
        "-af".into(),
        af,
        "-f".into(),
        "null".into(),
        "-".into(),
    ]
}

/// True when the detect pass failed because the file has no audio stream at
/// all (ffmpeg: "Output file does not contain any stream" / stream-matching
/// errors) — the one failure users hit constantly with screen recordings.
pub fn no_audio_error(log: &str) -> bool {
    log.contains("does not contain any stream")
        || log.contains("matches no streams")
        || log.contains("Cannot find a matching stream")
}

/// Parse the detect-pass log into (clip duration, ordered silences).
pub fn parse_log(log: &str) -> Result<(f64, Vec<Silence>), String> {
    let duration = parse_duration(log).ok_or_else(|| {
        "could not read the clip duration from ffmpeg's output — is the file a valid video or audio file?"
            .to_string()
    })?;
    Ok((duration, parse_silences(log)))
}

/// Build the full labeled timeline from the detected silences:
/// speech = complement of silences → drop bursts < `min_speech_s` →
/// pad ± `pad_s` (clamped to the clip, overlaps merged) → interleave with the
/// non-speech gaps. An empty result means a zero-length clip.
pub fn segments(
    duration: f64,
    silences: &[Silence],
    min_speech_s: f64,
    pad_s: f64,
) -> Vec<Segment> {
    let mut speech = silence_core::keep_segments(duration, silences);
    speech.retain(|(s, e)| e - s >= min_speech_s - 1e-9);

    let mut padded: Vec<(f64, f64)> = Vec::new();
    for (s, e) in speech {
        let s = (s - pad_s).max(0.0);
        let e = (e + pad_s).min(duration);
        match padded.last_mut() {
            Some(last) if s <= last.1 + MIN_GAP_S => last.1 = last.1.max(e),
            _ => padded.push((s, e)),
        }
    }

    let mut out = Vec::new();
    let mut cursor = 0.0_f64;
    for (s, e) in &padded {
        if *s - cursor > MIN_GAP_S {
            out.push(Segment { start: cursor, end: *s, speech: false });
        }
        out.push(Segment { start: *s, end: *e, speech: true });
        cursor = *e;
    }
    if duration - cursor > MIN_GAP_S {
        out.push(Segment { start: cursor, end: duration, speech: false });
    }
    out
}

/// Seconds → `M:SS.mmm` (or `H:MM:SS.mmm` past an hour) for the report.
fn clock(t: f64) -> String {
    let ms = (t * 1000.0).round() as u64;
    let (h, m, s, mil) = (
        ms / 3_600_000,
        (ms % 3_600_000) / 60_000,
        (ms % 60_000) / 1000,
        ms % 1000,
    );
    if h > 0 {
        format!("{h}:{m:02}:{s:02}.{mil:03}")
    } else {
        format!("{m}:{s:02}.{mil:03}")
    }
}

/// Seconds → SRT time `HH:MM:SS,mmm`.
fn srt_time(t: f64) -> String {
    let ms = (t * 1000.0).round() as u64;
    format!(
        "{:02}:{:02}:{:02},{:03}",
        ms / 3_600_000,
        (ms % 3_600_000) / 60_000,
        (ms % 60_000) / 1000,
        ms % 1000
    )
}

/// Total speech seconds in a timeline. Folds from +0.0 (an empty f64 `.sum()`
/// is -0.0, which would leak a "-0.000" into formatted output) — shared by the
/// report renderer and both surface wrappers' summary numbers.
pub fn speech_seconds(segs: &[Segment]) -> f64 {
    segs.iter()
        .filter(|s| s.speech)
        .fold(0.0, |a, s| a + s.duration())
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        "segment"
    } else {
        "segments"
    }
}

/// Render the labeled timeline in the requested format, listing only the rows
/// `filter` keeps (renumbered 1..n). The report's summary always covers both
/// classes.
pub fn render(
    segs: &[Segment],
    duration: f64,
    filter: SegmentFilter,
    format: OutputFormat,
) -> String {
    let rows: Vec<&Segment> = segs.iter().filter(|s| filter.keeps(s)).collect();
    match format {
        OutputFormat::Report => {
            let speech_n = segs.iter().filter(|s| s.speech).count();
            let non_n = segs.len() - speech_n;
            // fold from +0.0: an empty f64 `.sum()` is -0.0, which would
            // format as "-0.000s" in the zero-speech summary.
            let speech_s = speech_seconds(segs);
            let non_s: f64 = segs
                .iter()
                .filter(|s| !s.speech)
                .fold(0.0, |a, s| a + s.duration());
            let pct = |part: f64| {
                if duration > 0.0 {
                    100.0 * part / duration
                } else {
                    0.0
                }
            };
            let mut out = format!(
                "speech: {speech_n} {}, {speech_s:.3}s of {duration:.3}s ({:.1}%)\n\
                 non-speech: {non_n} {}, {non_s:.3}s ({:.1}%)\n",
                plural(speech_n),
                pct(speech_s),
                plural(non_n),
                pct(non_s),
            );
            if !rows.is_empty() {
                out.push('\n');
            }
            for (i, seg) in rows.iter().enumerate() {
                out.push_str(&format!(
                    "#{}  {} \u{2192} {}  {:<10}  {:.3}s\n",
                    i + 1,
                    clock(seg.start),
                    clock(seg.end),
                    seg.label(),
                    seg.duration()
                ));
            }
            if speech_n == 0 {
                out.push_str(
                    "\nNo speech detected at this threshold \u{2014} try a lower threshold_db (e.g. -45).\n",
                );
            }
            out
        }
        OutputFormat::Csv => {
            let mut out = String::from("start,end,duration,label\n");
            for seg in &rows {
                out.push_str(&format!(
                    "{:.3},{:.3},{:.3},{}\n",
                    seg.start,
                    seg.end,
                    seg.duration(),
                    seg.label()
                ));
            }
            out
        }
        OutputFormat::Srt => {
            let mut out = String::new();
            for (i, seg) in rows.iter().enumerate() {
                out.push_str(&format!(
                    "{}\n{} --> {}\n{}\n\n",
                    i + 1,
                    srt_time(seg.start),
                    srt_time(seg.end),
                    seg.label()
                ));
            }
            out
        }
        OutputFormat::Audacity => {
            let mut out = String::new();
            for seg in &rows {
                out.push_str(&format!(
                    "{:.6}\t{:.6}\t{}\n",
                    seg.start,
                    seg.end,
                    seg.label()
                ));
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sil(start: f64, end: f64) -> Silence {
        Silence { start, end: Some(end) }
    }

    // --- validate ----------------------------------------------------------
    #[test]
    fn validate_accepts_defaults() {
        assert!(validate(
            DEFAULT_THRESHOLD_DB,
            DEFAULT_MIN_SILENCE_S,
            DEFAULT_MIN_SPEECH_S,
            DEFAULT_PAD_S
        )
        .is_ok());
    }
    #[test]
    fn validate_rejects_bad_values() {
        assert!(validate(5.0, 0.5, 0.25, 0.0).is_err()); // positive dB
        assert!(validate(-30.0, 0.0, 0.25, 0.0).is_err()); // zero min_silence
        assert!(validate(-30.0, 0.5, -1.0, 0.0).is_err()); // negative min_speech
        assert!(validate(-30.0, 0.5, 0.25, -0.1).is_err()); // negative pad
        assert!(validate(f64::NAN, 0.5, 0.25, 0.0).is_err());
    }

    // --- detect_argv -------------------------------------------------------
    #[test]
    fn detect_argv_voice_band_bandpasses_before_silencedetect() {
        assert_eq!(
            detect_argv("in.mp4", -30.0, 0.5, true),
            vec![
                "-i",
                "in.mp4",
                "-vn",
                "-sn",
                "-dn",
                "-af",
                "highpass=f=200,lowpass=f=3000,silencedetect=noise=-30dB:d=0.5",
                "-f",
                "null",
                "-"
            ]
        );
    }
    #[test]
    fn detect_argv_full_band_when_voice_band_off() {
        let argv = detect_argv("in.webm", -42.5, 0.3, false);
        assert_eq!(argv[6], "silencedetect=noise=-42.5dB:d=0.3");
    }

    // --- parse_log / no_audio_error ---------------------------------------
    #[test]
    fn parse_log_reads_duration_and_silences() {
        let log = "  Duration: 00:00:06.00, start: 0.0, bitrate: 100 kb/s\n\
                   [silencedetect @ 0x1] silence_start: 2\n\
                   [silencedetect @ 0x1] silence_end: 4 | silence_duration: 2\n";
        let (dur, silences) = parse_log(log).unwrap();
        assert_eq!(dur, 6.0);
        assert_eq!(silences, vec![sil(2.0, 4.0)]);
    }
    #[test]
    fn parse_log_errors_without_duration() {
        assert!(parse_log("garbage").is_err());
    }
    #[test]
    fn no_audio_error_matches_ffmpeg_wording() {
        assert!(no_audio_error("Output file #0 does not contain any stream"));
        assert!(!no_audio_error("frame=  1 fps=0.0"));
    }

    // --- segments ----------------------------------------------------------
    #[test]
    fn segments_interleaves_speech_and_gaps() {
        let segs = segments(6.0, &[sil(2.0, 4.0)], 0.25, 0.0);
        assert_eq!(
            segs,
            vec![
                Segment { start: 0.0, end: 2.0, speech: true },
                Segment { start: 2.0, end: 4.0, speech: false },
                Segment { start: 4.0, end: 6.0, speech: true },
            ]
        );
    }
    #[test]
    fn segments_all_silence_is_one_gap() {
        let segs = segments(6.0, &[sil(0.0, 6.0)], 0.25, 0.0);
        assert_eq!(segs, vec![Segment { start: 0.0, end: 6.0, speech: false }]);
    }
    #[test]
    fn segments_no_silence_is_one_speech_block() {
        let segs = segments(6.0, &[], 0.25, 0.0);
        assert_eq!(segs, vec![Segment { start: 0.0, end: 6.0, speech: true }]);
    }
    #[test]
    fn segments_drops_bursts_shorter_than_min_speech() {
        // 0.1s blip between two silences is folded into one non-speech gap.
        let segs = segments(6.0, &[sil(0.0, 2.0), sil(2.1, 6.0)], 0.25, 0.0);
        assert_eq!(segs, vec![Segment { start: 0.0, end: 6.0, speech: false }]);
    }
    #[test]
    fn segments_min_speech_zero_keeps_bursts() {
        let segs = segments(6.0, &[sil(0.0, 2.0), sil(2.1, 6.0)], 0.0, 0.0);
        assert_eq!(segs.iter().filter(|s| s.speech).count(), 1);
        assert_eq!(segs[1], Segment { start: 2.0, end: 2.1, speech: true });
    }
    #[test]
    fn segments_pad_extends_and_clamps() {
        let segs = segments(6.0, &[sil(0.0, 1.0), sil(5.0, 6.0)], 0.25, 0.5);
        // speech 1..5 padded to 0.5..5.5, clamped inside the clip
        assert_eq!(
            segs,
            vec![
                Segment { start: 0.0, end: 0.5, speech: false },
                Segment { start: 0.5, end: 5.5, speech: true },
                Segment { start: 5.5, end: 6.0, speech: false },
            ]
        );
    }
    #[test]
    fn segments_pad_merges_overlapping_speech() {
        // Two speech blocks 0..2 and 2.4..6 with pad 0.3 overlap across the
        // 0.4s gap → one merged speech segment covering the whole clip.
        let segs = segments(6.0, &[sil(2.0, 2.4)], 0.25, 0.3);
        assert_eq!(segs, vec![Segment { start: 0.0, end: 6.0, speech: true }]);
    }
    #[test]
    fn segments_trailing_silence_to_eof() {
        let segs = segments(6.0, &[Silence { start: 4.0, end: None }], 0.25, 0.0);
        assert_eq!(
            segs,
            vec![
                Segment { start: 0.0, end: 4.0, speech: true },
                Segment { start: 4.0, end: 6.0, speech: false },
            ]
        );
    }

    // --- parses ------------------------------------------------------------
    #[test]
    fn filter_parse_roundtrip_and_error() {
        assert_eq!(SegmentFilter::parse("both").unwrap(), SegmentFilter::Both);
        assert_eq!(SegmentFilter::parse("speech").unwrap(), SegmentFilter::Speech);
        assert_eq!(
            SegmentFilter::parse("non-speech").unwrap(),
            SegmentFilter::NonSpeech
        );
        assert!(SegmentFilter::parse("silence").is_err());
    }
    #[test]
    fn output_parse_extension_mime() {
        assert_eq!(OutputFormat::parse("report").unwrap(), OutputFormat::Report);
        assert_eq!(OutputFormat::parse("csv").unwrap().extension(), "csv");
        assert_eq!(OutputFormat::parse("srt").unwrap().mime(), "application/x-subrip");
        assert_eq!(OutputFormat::parse("audacity").unwrap().extension(), "txt");
        assert!(OutputFormat::parse("json5").is_err());
    }

    // --- render -------------------------------------------------------------
    fn demo_segs() -> Vec<Segment> {
        vec![
            Segment { start: 0.0, end: 2.0, speech: true },
            Segment { start: 2.0, end: 4.0, speech: false },
            Segment { start: 4.0, end: 6.0, speech: true },
        ]
    }

    #[test]
    fn render_report_exact() {
        let out = render(&demo_segs(), 6.0, SegmentFilter::Both, OutputFormat::Report);
        assert_eq!(
            out,
            "speech: 2 segments, 4.000s of 6.000s (66.7%)\n\
             non-speech: 1 segment, 2.000s (33.3%)\n\
             \n\
             #1  0:00.000 \u{2192} 0:02.000  speech      2.000s\n\
             #2  0:02.000 \u{2192} 0:04.000  non-speech  2.000s\n\
             #3  0:04.000 \u{2192} 0:06.000  speech      2.000s\n"
        );
    }
    #[test]
    fn render_report_no_speech_hint() {
        let segs = vec![Segment { start: 0.0, end: 6.0, speech: false }];
        let out = render(&segs, 6.0, SegmentFilter::Both, OutputFormat::Report);
        assert!(out.contains("No speech detected at this threshold"));
        assert!(out.starts_with("speech: 0 segments, 0.000s of 6.000s (0.0%)\n"));
    }
    #[test]
    fn render_csv_exact_and_filtered() {
        let out = render(&demo_segs(), 6.0, SegmentFilter::Speech, OutputFormat::Csv);
        assert_eq!(
            out,
            "start,end,duration,label\n0.000,2.000,2.000,speech\n4.000,6.000,2.000,speech\n"
        );
    }
    #[test]
    fn render_srt_exact() {
        let out = render(&demo_segs(), 6.0, SegmentFilter::Speech, OutputFormat::Srt);
        assert_eq!(
            out,
            "1\n00:00:00,000 --> 00:00:02,000\nspeech\n\n\
             2\n00:00:04,000 --> 00:00:06,000\nspeech\n\n"
        );
    }
    #[test]
    fn render_audacity_labels_non_speech_filter() {
        let out = render(&demo_segs(), 6.0, SegmentFilter::NonSpeech, OutputFormat::Audacity);
        assert_eq!(out, "2.000000\t4.000000\tnon-speech\n");
    }
    #[test]
    fn clock_formats_past_an_hour() {
        assert_eq!(clock(3723.5), "1:02:03.500");
        assert_eq!(clock(65.25), "1:05.250");
    }
    #[test]
    fn srt_time_formats() {
        assert_eq!(srt_time(3723.508), "01:02:03,508");
    }
}
