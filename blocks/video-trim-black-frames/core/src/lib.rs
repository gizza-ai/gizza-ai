//! gizza-ai/video-trim-black-frames core — pure pieces of the **two-pass**
//! edge-black-trim flow shared by the chat block, the CLI, and the page.
//!
//! # Why two-pass
//!
//! ffmpeg's `blackdetect` filter only *measures* black runs (it logs
//! `black_start`/`black_end`/`black_duration` per detected run); the trim that
//! actually removes the leading/trailing black needs those timestamps up front.
//! So the tool is:
//!
//!   1. **detect** — `-vf blackdetect=d=<min>:pic_th=<ratio>:pix_th=<pixel>
//!      -an -f null -` over the whole clip. We parse its log for every black run
//!      plus the clip `Duration:`.
//!   2. **trim** — keep `[start, end]` where `start` is the end of the leading
//!      black run (if the clip opens black) and `end` is the start of the
//!      trailing black run (if the clip ends black). Cutting at exact timestamps
//!      requires a re-encode (H.264 CRF 18 + AAC), so the video track is always
//!      re-encoded.
//!
//! This module owns the pure parts: argv builders, the blackdetect log parser,
//! and the trim/no-edges/error decision. The block dispatches ffmpeg twice
//! (`video-autocrop-bars` / `video-silence-cut` precedent); the page mirrors it
//! in `page/custom.js`.
//!
//! Container rule (family invariant): the input container is kept when it can
//! hold H.264 + AAC (mp4/mov/m4v/mkv); anything else (webm, …) switches to mp4.
//! Because a trim always re-encodes the video AND cuts the audio at an exact
//! point, the audio is always re-encoded to AAC (a stream-copy can only cut on a
//! packet boundary, drifting the trim point) — see the crate's `h264_out_ext`.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;

/// blackdetect `pix_th` (max pixel luma, normalized 0-1, that counts as black).
/// 0.10 is ffmpeg's own default.
pub const DEFAULT_PIXEL_THRESHOLD: f64 = 0.10;
/// blackdetect `pic_th` (fraction of black pixels for a whole frame to count as
/// black). 0.98 is ffmpeg's own default.
pub const DEFAULT_BLACK_RATIO: f64 = 0.98;
/// blackdetect `d` (minimum black run, seconds). ffmpeg defaults to 2.0, far too
/// coarse for edge trimming — 0.10 catches brief intro/outro black.
pub const DEFAULT_MIN_DURATION: f64 = 0.10;
/// Upper bound on the min-duration knob (a 60 s black edge is already extreme).
pub const MAX_MIN_DURATION: f64 = 60.0;

/// Which ends to trim.
pub const ENDS: [&str; 3] = ["both", "start", "end"];
pub const DEFAULT_ENDS: &str = "both";

/// A detected black run is treated as a *leading* edge when it begins within
/// this many seconds of 0, and a *trailing* edge when it ends within this many
/// seconds of the clip duration. Keeps a mid-clip black run from being mistaken
/// for an edge.
pub const EDGE_EPS: f64 = 0.25;

/// The kept span must be at least this long, else "the whole clip is black".
pub const MIN_KEEP: f64 = 0.05;

/// Which ends to consider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ends {
    Both,
    Start,
    End,
}

impl Ends {
    fn includes_start(self) -> bool {
        matches!(self, Ends::Both | Ends::Start)
    }
    fn includes_end(self) -> bool {
        matches!(self, Ends::Both | Ends::End)
    }
}

/// Parse the `ends` choice.
pub fn parse_ends(s: &str) -> Result<Ends, String> {
    match s.trim() {
        "both" => Ok(Ends::Both),
        "start" => Ok(Ends::Start),
        "end" => Ok(Ends::End),
        other => Err(format!("ends must be one of both, start, end (got '{other}')")),
    }
}

/// Validate + normalize the four params.
pub fn validate(
    pixel_threshold: f64,
    black_ratio: f64,
    min_duration: f64,
    ends: &str,
) -> Result<(f64, f64, f64, Ends), String> {
    if !pixel_threshold.is_finite() || !(0.0..=1.0).contains(&pixel_threshold) {
        return Err(format!(
            "pixel_threshold must be between 0 and 1 (got {pixel_threshold})"
        ));
    }
    if !black_ratio.is_finite() || !(0.0..=1.0).contains(&black_ratio) {
        return Err(format!("black_ratio must be between 0 and 1 (got {black_ratio})"));
    }
    if !min_duration.is_finite() || !(0.0..=MAX_MIN_DURATION).contains(&min_duration) {
        return Err(format!(
            "min_duration must be between 0 and {MAX_MIN_DURATION:.0} seconds (got {min_duration})"
        ));
    }
    let ends = parse_ends(ends)?;
    Ok((pixel_threshold, black_ratio, min_duration, ends))
}

/// Format seconds compactly (up to 3 decimals, no trailing zeros) for an argv.
pub fn fmt_secs(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Pass-1 argv (no leading "ffmpeg"): measure black runs, no output file. The
/// result is the LOG (the ffmpeg bridge returns it even though `-f null -`
/// writes no file). `-an` skips audio for a faster detect pass.
pub fn detect_argv(
    in_name: &str,
    pixel_threshold: f64,
    black_ratio: f64,
    min_duration: f64,
) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-vf".into(),
        format!(
            "blackdetect=d={}:pic_th={}:pix_th={}",
            fmt_secs(min_duration),
            fmt_secs(black_ratio),
            fmt_secs(pixel_threshold)
        ),
        "-an".into(),
        "-f".into(),
        "null".into(),
        "-".into(),
    ]
}

/// Parse the clip duration (seconds) from an ffmpeg log's `Duration: HH:MM:SS.ss`.
pub fn parse_duration(log: &str) -> Option<f64> {
    let idx = log.find("Duration:")?;
    let rest = log[idx + "Duration:".len()..].trim_start();
    let hms = rest.split(',').next()?.trim();
    let mut parts = hms.split(':');
    let h: f64 = parts.next()?.trim().parse().ok()?;
    let m: f64 = parts.next()?.trim().parse().ok()?;
    let s: f64 = parts.next()?.trim().parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + s)
}

/// Extract the float that follows `marker` on a line, stopping at whitespace or `|`.
fn marker_value(line: &str, marker: &str) -> Option<f64> {
    let idx = line.find(marker)?;
    let rest = line[idx + marker.len()..].trim_start();
    let tok: String = rest
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != '|')
        .collect();
    tok.parse().ok()
}

/// Parse `black_start:`/`black_end:` runs from a blackdetect log into ordered
/// `(start, end)` pairs. ffmpeg logs both markers on one line per run.
pub fn parse_black_intervals(log: &str) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::new();
    for line in log.lines() {
        if let (Some(s), Some(e)) =
            (marker_value(line, "black_start:"), marker_value(line, "black_end:"))
        {
            if e > s {
                out.push((s, e));
            }
        }
    }
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// What pass 1 concluded.
#[derive(Debug, PartialEq)]
pub enum TrimDecision {
    /// No leading/trailing black to trim (for the requested ends).
    NoEdges { duration: f64 },
    /// Keep `[start, end]` out of a clip that runs `[0, duration]`.
    Trim { start: f64, end: f64, duration: f64 },
}

/// Seconds removed from the front / back for a `Trim`.
pub fn removed(start: f64, end: f64, duration: f64) -> (f64, f64) {
    (start, (duration - end).max(0.0))
}

/// Turn a blackdetect log + the requested ends into a decision. Errors are
/// user-facing strings.
pub fn decide(log: &str, ends: Ends) -> Result<TrimDecision, String> {
    let duration = parse_duration(log).ok_or_else(|| {
        "could not read the video duration from ffmpeg output (is this a valid video?)".to_string()
    })?;
    let intervals = parse_black_intervals(log);

    let mut new_start = 0.0_f64;
    let mut new_end = duration;
    let mut trimmed_lead = false;
    let mut trimmed_trail = false;

    if ends.includes_start() {
        if let Some(&(s, e)) = intervals.first() {
            if s <= EDGE_EPS {
                new_start = e;
                trimmed_lead = true;
            }
        }
    }
    if ends.includes_end() {
        if let Some(&(s, e)) = intervals.last() {
            if e >= duration - EDGE_EPS {
                new_end = s;
                trimmed_trail = true;
            }
        }
    }

    if !trimmed_lead && !trimmed_trail {
        return Ok(TrimDecision::NoEdges { duration });
    }

    let new_start = new_start.max(0.0);
    let new_end = new_end.min(duration);
    if new_end <= new_start + MIN_KEEP {
        return Err(
            "the entire clip reads as black at these settings — nothing would remain after \
             trimming. Lower the pixel threshold, lower the black-pixel ratio, or raise the \
             minimum black duration."
                .to_string(),
        );
    }
    Ok(TrimDecision::Trim { start: new_start, end: new_end, duration })
}

/// Pass-2 `(argv, out_name)`: keep `[start, end]` and re-encode. Input-side
/// `-ss` + `-t` gives a frame-accurate cut with a re-encode; H.264 CRF 18
/// (visually lossless tier — the point is trimming, not compression); audio is
/// re-encoded to AAC so the cut point can't drift to a packet boundary.
pub fn trim_argv(in_name: &str, start: f64, end: f64) -> (Vec<String>, String) {
    let (ext, _) = h264_out_ext(in_name);
    let out_name = format!("out.{ext}");
    let dur = (end - start).max(0.0);
    let argv = vec![
        "-ss".into(),
        fmt_secs(start),
        "-i".into(),
        in_name.into(),
        "-t".into(),
        fmt_secs(dur),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "medium".into(),
        "-crf".into(),
        "18".into(),
        "-c:a".into(),
        "aac".into(),
        out_name.clone(),
    ];
    (argv, out_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Leading black [0,1.5], trailing black [8.2,10] on a 10 s clip.
    const SAMPLE_LOG: &str = "\
Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'in.mp4':
  Duration: 00:00:10.00, start: 0.000000, bitrate: 142 kb/s
  Stream #0:0[0x1](und): Video: h264 (High), yuv420p(progressive), 320x240, 142 kb/s, 10 fps
[blackdetect @ 0x55] black_start:0 black_end:1.5 black_duration:1.5
[blackdetect @ 0x55] black_start:8.2 black_end:10 black_duration:1.8
";

    // --- validate -------------------------------------------------------------

    #[test]
    fn validate_accepts_defaults() {
        assert_eq!(
            validate(
                DEFAULT_PIXEL_THRESHOLD,
                DEFAULT_BLACK_RATIO,
                DEFAULT_MIN_DURATION,
                DEFAULT_ENDS
            ),
            Ok((0.10, 0.98, 0.10, Ends::Both))
        );
        for (s, e) in [("both", Ends::Both), ("start", Ends::Start), ("end", Ends::End)] {
            assert_eq!(validate(0.1, 0.98, 0.1, s).unwrap().3, e);
        }
        assert_eq!(validate(0.0, 0.0, 0.0, "both").unwrap().0, 0.0);
        assert_eq!(validate(1.0, 1.0, 60.0, "both").unwrap().2, 60.0);
    }

    #[test]
    fn validate_rejects_out_of_range() {
        assert!(validate(-0.1, 0.98, 0.1, "both").is_err());
        assert!(validate(1.1, 0.98, 0.1, "both").is_err());
        assert!(validate(0.1, 2.0, 0.1, "both").is_err());
        assert!(validate(0.1, 0.98, 61.0, "both").is_err());
        assert!(validate(f64::NAN, 0.98, 0.1, "both").is_err());
        assert!(validate(0.1, 0.98, 0.1, "middle").is_err());
    }

    // --- fmt_secs -------------------------------------------------------------

    #[test]
    fn fmt_secs_trims_trailing_zeros() {
        assert_eq!(fmt_secs(1.5), "1.5");
        assert_eq!(fmt_secs(10.0), "10");
        assert_eq!(fmt_secs(0.1), "0.1");
        assert_eq!(fmt_secs(0.0), "0");
        assert_eq!(fmt_secs(2.125), "2.125");
    }

    // --- detect_argv ----------------------------------------------------------

    #[test]
    fn detect_argv_builds_blackdetect_null_sink() {
        assert_eq!(
            detect_argv("in.mp4", 0.10, 0.98, 0.10),
            vec![
                "-i", "in.mp4", "-vf", "blackdetect=d=0.1:pic_th=0.98:pix_th=0.1", "-an", "-f",
                "null", "-"
            ]
        );
    }

    // --- parsers --------------------------------------------------------------

    #[test]
    fn parse_duration_reads_hms() {
        assert_eq!(parse_duration(SAMPLE_LOG), Some(10.0));
        assert_eq!(parse_duration("nope"), None);
    }

    #[test]
    fn parse_black_intervals_orders_runs() {
        assert_eq!(parse_black_intervals(SAMPLE_LOG), vec![(0.0, 1.5), (8.2, 10.0)]);
    }

    #[test]
    fn parse_black_intervals_empty_without_markers() {
        assert!(parse_black_intervals("frame=  20 fps=0.0\n").is_empty());
    }

    // --- decide ---------------------------------------------------------------

    #[test]
    fn decide_trims_both_edges() {
        assert_eq!(
            decide(SAMPLE_LOG, Ends::Both),
            Ok(TrimDecision::Trim { start: 1.5, end: 8.2, duration: 10.0 })
        );
    }

    #[test]
    fn decide_start_only_leaves_trailing_black() {
        assert_eq!(
            decide(SAMPLE_LOG, Ends::Start),
            Ok(TrimDecision::Trim { start: 1.5, end: 10.0, duration: 10.0 })
        );
    }

    #[test]
    fn decide_end_only_leaves_leading_black() {
        assert_eq!(
            decide(SAMPLE_LOG, Ends::End),
            Ok(TrimDecision::Trim { start: 0.0, end: 8.2, duration: 10.0 })
        );
    }

    #[test]
    fn decide_no_edges_when_black_is_mid_clip() {
        let log = SAMPLE_LOG
            .replace("black_start:0 black_end:1.5", "black_start:4 black_end:5")
            .replace("black_start:8.2 black_end:10", "black_start:6 black_end:6.5");
        assert_eq!(decide(&log, Ends::Both), Ok(TrimDecision::NoEdges { duration: 10.0 }));
    }

    #[test]
    fn decide_errors_when_whole_clip_black() {
        let log = "\
  Duration: 00:00:10.00, start: 0.000000
[blackdetect @ 0x55] black_start:0 black_end:10 black_duration:10
";
        let err = decide(log, Ends::Both).unwrap_err();
        assert!(err.contains("entire clip reads as black"), "{err}");
    }

    #[test]
    fn decide_errors_without_duration() {
        let err =
            decide("[blackdetect @ 0x55] black_start:0 black_end:1\n", Ends::Both).unwrap_err();
        assert!(err.contains("could not read the video duration"), "{err}");
    }

    #[test]
    fn removed_reports_front_and_back() {
        let (front, back) = removed(1.5, 8.2, 10.0);
        assert_eq!(front, 1.5);
        assert!((back - 1.8).abs() < 1e-9);
    }

    // --- trim_argv ------------------------------------------------------------

    #[test]
    fn trim_argv_keeps_container_and_reencodes() {
        let (argv, out_name) = trim_argv("in.mp4", 1.5, 8.2);
        assert_eq!(out_name, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-ss", "1.5", "-i", "in.mp4", "-t", "6.7", "-c:v", "libx264", "-preset", "medium",
                "-crf", "18", "-c:a", "aac", "out.mp4"
            ]
        );
    }

    #[test]
    fn trim_argv_switches_webm_to_mp4() {
        let (argv, out_name) = trim_argv("in.webm", 0.0, 5.0);
        assert_eq!(out_name, "out.mp4");
        assert!(argv.contains(&"aac".to_string()));
    }
}
