//! gizza-ai/video-silence-cut core — pure, native-testable logic for a *correct*
//! two-pass silence cut. No wafer/wasm-bindgen deps.
//!
//! # Why two-pass (and why this is correct, unlike the earlier draft)
//!
//! Removing silent gaps from a video while keeping audio and video in sync is
//! classically a **two-pass** operation:
//!
//!   1. **detect** — run ffmpeg's `silencedetect` audio filter, which prints
//!      `silence_start` / `silence_end` timestamps to stderr. Produces no output
//!      file (`-f null -`); the result is the log.
//!   2. **cut** — from the detected silences (and the clip duration) compute the
//!      complementary **keep** segments, then re-encode keeping only those time
//!      ranges of *both* streams via `select`/`aselect` with matching `between(t,…)`
//!      expressions and `setpts`/`asetpts` to close the gaps. Because video and
//!      audio select the *same* time ranges, they stay in sync.
//!
//! A single ffmpeg invocation cannot read its own first-pass stderr, and the
//! single-pass `silenceremove` filter is audio-only (it desyncs the video). The
//! gizza ffmpeg bridge returns each exec's log and the block can dispatch twice,
//! so the two-pass flow runs in chat + CLI. This module owns the pure pieces:
//! argv construction, log parsing, and keep-segment computation.

/// A detected silent range. `end` is `None` when the silence runs to EOF (ffmpeg
/// prints a trailing `silence_start` with no matching `silence_end`).
#[derive(Debug, Clone, PartialEq)]
pub struct Silence {
    pub start: f64,
    pub end: Option<f64>,
}

/// Keep segments shorter than this (seconds) are dropped — a sub-10ms sliver
/// between two silences is not worth a frame and would produce empty output.
const MIN_KEEP_S: f64 = 1e-3;

/// Validate the user params. `threshold_db` must be a finite value `<= 0`
/// (silence is below 0 dB); `min_silence` must be a finite value `> 0` seconds.
pub fn validate(threshold_db: f64, min_silence_s: f64) -> Result<(), String> {
    if !threshold_db.is_finite() {
        return Err("threshold_db must be a finite number (e.g. -30)".into());
    }
    if threshold_db > 0.0 {
        return Err("threshold_db must be <= 0 dB (silence is below 0 dB; e.g. -30)".into());
    }
    if !min_silence_s.is_finite() || min_silence_s <= 0.0 {
        return Err("min_silence must be a finite number > 0 (seconds)".into());
    }
    Ok(())
}

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`-30` not `-30.0`,
/// `0.5` stays `0.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Pass 1 argv (no leading "ffmpeg"): detect silent ranges, no output file.
pub fn detect_argv(in_name: &str, threshold_db: f64, min_silence_s: f64) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-af".into(),
        format!(
            "silencedetect=noise={}dB:d={}",
            fmt_num(threshold_db),
            fmt_num(min_silence_s)
        ),
        "-f".into(),
        "null".into(),
        "-".into(),
    ]
}

/// Parse the clip duration (seconds) from an ffmpeg log's `Duration: HH:MM:SS.ss`
/// line. Returns `None` if absent/unparseable.
pub fn parse_duration(log: &str) -> Option<f64> {
    let idx = log.find("Duration:")?;
    let rest = log[idx + "Duration:".len()..].trim_start();
    // Up to the first comma: "00:00:12.50"
    let hms = rest.split(',').next()?.trim();
    let mut parts = hms.split(':');
    let h: f64 = parts.next()?.trim().parse().ok()?;
    let m: f64 = parts.next()?.trim().parse().ok()?;
    let s: f64 = parts.next()?.trim().parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + s)
}

/// Parse `silence_start:` / `silence_end:` markers from an ffmpeg silencedetect
/// log into ordered `Silence` ranges. A `silence_end` attaches to the most recent
/// open `silence_start`.
pub fn parse_silences(log: &str) -> Vec<Silence> {
    let mut out: Vec<Silence> = Vec::new();
    for line in log.lines() {
        if let Some(v) = marker_value(line, "silence_start:") {
            out.push(Silence {
                start: v,
                end: None,
            });
        } else if let Some(v) = marker_value(line, "silence_end:") {
            if let Some(last) = out.last_mut() {
                if last.end.is_none() {
                    last.end = Some(v);
                }
            }
        }
    }
    out
}

/// Extract the float that follows `marker` on a line, stopping at whitespace or
/// `|` (ffmpeg appends `| silence_duration: …` to silence_end lines).
fn marker_value(line: &str, marker: &str) -> Option<f64> {
    let idx = line.find(marker)?;
    let rest = line[idx + marker.len()..].trim_start();
    let tok: String = rest
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != '|')
        .collect();
    tok.parse().ok()
}

/// Compute the non-silent **keep** segments within `[0, duration]` as the
/// complement of `silences` (assumed ordered, non-overlapping). Segments shorter
/// than [`MIN_KEEP_S`] are dropped. An empty result means the whole clip is silent.
pub fn keep_segments(duration: f64, silences: &[Silence]) -> Vec<(f64, f64)> {
    let mut keeps = Vec::new();
    let mut cursor = 0.0_f64;
    for s in silences {
        if s.start > cursor {
            push_keep(&mut keeps, cursor, s.start.min(duration));
        }
        match s.end {
            Some(e) => cursor = cursor.max(e),
            None => {
                cursor = duration; // silence runs to EOF — nothing left to keep
                break;
            }
        }
    }
    if cursor < duration {
        push_keep(&mut keeps, cursor, duration);
    }
    keeps
}

fn push_keep(keeps: &mut Vec<(f64, f64)>, start: f64, end: f64) {
    if end - start > MIN_KEEP_S {
        keeps.push((start, end));
    }
}

/// The shared `select`/`aselect` predicate keeping the union of `keeps`:
/// `between(t,s0,e0)+between(t,s1,e1)+…`.
pub fn keep_expr(keeps: &[(f64, f64)]) -> String {
    keeps
        .iter()
        .map(|(s, e)| format!("between(t,{},{})", fmt_num(*s), fmt_num(*e)))
        .collect::<Vec<_>>()
        .join("+")
}

/// Pass 2 argv (no leading "ffmpeg"): keep only `keeps` of both streams, closing
/// the gaps (`setpts`/`asetpts`), re-encode to H.264/AAC mp4.
pub fn cut_argv(in_name: &str, out_name: &str, keeps: &[(f64, f64)]) -> Vec<String> {
    let expr = keep_expr(keeps);
    vec![
        "-i".into(),
        in_name.into(),
        "-vf".into(),
        format!("select='{expr}',setpts=N/FRAME_RATE/TB"),
        "-af".into(),
        format!("aselect='{expr}',asetpts=N/SR/TB"),
        "-c:v".into(),
        "libx264".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "aac".into(),
        out_name.into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate ---------------------------------------------------------
    #[test]
    fn validate_accepts_sane_defaults() {
        assert!(validate(-30.0, 0.5).is_ok());
    }
    #[test]
    fn validate_rejects_positive_threshold() {
        assert!(validate(5.0, 0.5).is_err());
    }
    #[test]
    fn validate_rejects_nonpositive_min_silence() {
        assert!(validate(-30.0, 0.0).is_err());
        assert!(validate(-30.0, -1.0).is_err());
    }
    #[test]
    fn validate_rejects_nonfinite() {
        assert!(validate(f64::NAN, 0.5).is_err());
        assert!(validate(-30.0, f64::INFINITY).is_err());
    }

    // --- fmt_num ----------------------------------------------------------
    #[test]
    fn fmt_num_drops_trailing_zero() {
        assert_eq!(fmt_num(-30.0), "-30");
        assert_eq!(fmt_num(8.0), "8");
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(fmt_num(9.75), "9.75");
    }

    // --- detect_argv ------------------------------------------------------
    #[test]
    fn detect_argv_builds_silencedetect_null_sink() {
        assert_eq!(
            detect_argv("in.mp4", -30.0, 0.5),
            vec![
                "-i", "in.mp4", "-af", "silencedetect=noise=-30dB:d=0.5", "-f", "null", "-"
            ]
        );
    }

    // --- parse_duration ---------------------------------------------------
    #[test]
    fn parse_duration_reads_hms() {
        let log = "  Duration: 00:00:12.50, start: 0.000000, bitrate: 1000 kb/s\n";
        assert_eq!(parse_duration(log), Some(12.5));
    }
    #[test]
    fn parse_duration_reads_minutes_hours() {
        let log = "  Duration: 01:02:03.00, start: 0\n";
        assert_eq!(parse_duration(log), Some(3723.0));
    }
    #[test]
    fn parse_duration_none_when_absent() {
        assert_eq!(parse_duration("no duration here"), None);
    }

    // --- parse_silences ---------------------------------------------------
    const SAMPLE_LOG: &str = "\
Input #0, mov,mp4,m4a
  Duration: 00:00:12.50, start: 0.000000, bitrate: 1000 kb/s
[silencedetect @ 0x55] silence_start: 2.5
[silencedetect @ 0x55] silence_end: 4.3 | silence_duration: 1.8
[silencedetect @ 0x55] silence_start: 8
[silencedetect @ 0x55] silence_end: 9.75 | silence_duration: 1.75
";

    #[test]
    fn parse_silences_pairs_start_end() {
        assert_eq!(
            parse_silences(SAMPLE_LOG),
            vec![
                Silence { start: 2.5, end: Some(4.3) },
                Silence { start: 8.0, end: Some(9.75) },
            ]
        );
    }
    #[test]
    fn parse_silences_trailing_silence_to_eof_has_no_end() {
        let log = "[silencedetect @ 0x1] silence_start: 5\n"; // no silence_end
        assert_eq!(parse_silences(log), vec![Silence { start: 5.0, end: None }]);
    }
    #[test]
    fn parse_silences_empty_when_none() {
        assert!(parse_silences("nothing").is_empty());
    }

    // --- keep_segments ----------------------------------------------------
    #[test]
    fn keep_segments_complement_in_middle() {
        let s = parse_silences(SAMPLE_LOG);
        assert_eq!(
            keep_segments(12.5, &s),
            vec![(0.0, 2.5), (4.3, 8.0), (9.75, 12.5)]
        );
    }
    #[test]
    fn keep_segments_silence_at_start_has_no_leading_keep() {
        let s = vec![Silence { start: 0.0, end: Some(3.0) }];
        assert_eq!(keep_segments(10.0, &s), vec![(3.0, 10.0)]);
    }
    #[test]
    fn keep_segments_trailing_silence_to_eof_drops_tail() {
        let s = vec![Silence { start: 5.0, end: None }];
        assert_eq!(keep_segments(10.0, &s), vec![(0.0, 5.0)]);
    }
    #[test]
    fn keep_segments_no_silence_keeps_whole_clip() {
        assert_eq!(keep_segments(10.0, &[]), vec![(0.0, 10.0)]);
    }
    #[test]
    fn keep_segments_all_silence_keeps_nothing() {
        let s = vec![Silence { start: 0.0, end: Some(10.0) }];
        assert!(keep_segments(10.0, &s).is_empty());
    }
    #[test]
    fn keep_segments_drops_subms_sliver() {
        // 0.5ms gap between two silences — below MIN_KEEP_S, dropped.
        let s = vec![
            Silence { start: 0.0, end: Some(4.9995) },
            Silence { start: 5.0, end: Some(10.0) },
        ];
        assert!(keep_segments(10.0, &s).is_empty());
    }

    // --- cut_argv ---------------------------------------------------------
    #[test]
    fn keep_expr_sums_between_clauses() {
        let keeps = vec![(0.0, 2.5), (4.3, 8.0)];
        assert_eq!(keep_expr(&keeps), "between(t,0,2.5)+between(t,4.3,8)");
    }
    #[test]
    fn cut_argv_selects_both_streams_and_reencodes() {
        let argv = cut_argv("in.mp4", "out.mp4", &[(0.0, 2.5)]);
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp4",
                "-vf",
                "select='between(t,0,2.5)',setpts=N/FRAME_RATE/TB",
                "-af",
                "aselect='between(t,0,2.5)',asetpts=N/SR/TB",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "out.mp4",
            ]
        );
    }
}
