//! gizza-ai/video-keyframe-timestamp-list core — pure keyframe-timestamp helpers
//! shared by the chat/CLI block. No wafer/wasm/ffmpeg-host deps: this crate only
//! builds the ffmpeg argv, parses the resulting log, and renders the list.
//!
//! Pipeline (the block drives ffmpeg, this crate does the pure parts):
//! 1. [`detect_argv`] → an ffmpeg command that keeps only I-frames
//!    (`select='eq(pict_type\,I)'`) and prints one `showinfo` line per kept frame
//!    to the log — no output file is written (`-f null -`).
//! 2. [`parse_keyframes`] reads the `pts_time:` values out of that log = the
//!    keyframe (I-frame) timestamps in seconds, sorted and de-duplicated.
//! 3. [`round_dedup`] rounds them to the requested decimal precision (two frames
//!    can't share a timestamp, but rounding can collapse them).
//! 4. [`render`] turns the list into the requested `text` / `csv` / `json` output,
//!    and [`stats`] derives the count/first/last/gap summary numbers.
//!
//! Everything here is pure Rust → it runs on every backend, including the chat
//! Service Worker (the ffmpeg exec itself is dispatched to gizza-ai/ffmpeg-runtime
//! by the block).

/// Output renderings accepted by [`render`], in schema order.
pub const FORMATS: [&str; 3] = ["json", "csv", "text"];
/// Default rendering — a JSON array of `{index, seconds, timecode, gap_seconds}`.
pub const DEFAULT_FORMAT: &str = "json";

/// Default decimal places for every timestamp (milliseconds).
pub const DEFAULT_PRECISION: u32 = 3;
/// Most decimal places a caller may ask for. Beyond this the digits are decode
/// noise, not timing information.
pub const MAX_PRECISION: u32 = 6;

/// Hard cap on reported keyframes. An all-intra source (ProRes, DNxHD, MJPEG,
/// `-g 1` H.264) makes EVERY frame a keyframe, so a long clip can produce a
/// pathologically large list; past this we refuse rather than emit it.
pub const MAX_KEYFRAMES: usize = 20_000;

/// Build the ffmpeg argv that lists `in_name`'s keyframes. The `select` filter
/// keeps only frames whose picture type is I, `showinfo` prints one line per kept
/// frame (carrying its `pts_time`), and `-f null -` discards the video so nothing
/// is written. Audio and subtitles are dropped; only the first video stream is
/// mapped, so a cover-art or thumbnail stream can't pollute the list.
///
/// The comma inside `eq(pict_type\,I)` is escaped because ffmpeg's filtergraph
/// parser treats a bare comma as the separator between filters.
pub fn detect_argv(in_name: &str) -> Vec<String> {
    [
        "-hide_banner",
        "-nostats",
        "-i",
        in_name,
        "-an",
        "-sn",
        "-map",
        "0:v:0",
        "-filter:v",
        r"select='eq(pict_type\,I)',showinfo",
        "-f",
        "null",
        "-",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Extract keyframe timestamps (seconds) from an ffmpeg `showinfo` log, sorted
/// ascending and de-duplicated at full precision.
///
/// Only lines emitted by the filter instance itself (tagged `[Parsed_showinfo_N @
/// 0x…]`) are read, so other `pts_time` mentions — input banners, decoder
/// warnings — are ignored. Non-numeric (`N/A`), negative, and non-finite values
/// are skipped rather than failing the whole parse: one unreadable frame header
/// shouldn't lose the other timestamps.
pub fn parse_keyframes(log: &str) -> Vec<f64> {
    let mut times: Vec<f64> = Vec::new();
    for line in log.lines() {
        if !line.contains("Parsed_showinfo") {
            continue;
        }
        let Some(idx) = line.find("pts_time:") else {
            continue;
        };
        let rest = &line[idx + "pts_time:".len()..];
        let num: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
        if let Ok(t) = num.parse::<f64>() {
            if t.is_finite() && t >= 0.0 {
                times.push(t);
            }
        }
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    times.dedup();
    times
}

/// Round `v` to `precision` decimal places (clamped to [`MAX_PRECISION`]).
pub fn round_to(v: f64, precision: u32) -> f64 {
    if !v.is_finite() {
        return v;
    }
    let scale = 10f64.powi(precision.min(MAX_PRECISION) as i32);
    (v * scale).round() / scale
}

/// Round every timestamp to `precision` and drop values that collapse onto their
/// predecessor. Input is assumed sorted ascending (as [`parse_keyframes`] returns
/// it); at `precision = 0` two keyframes 40 ms apart become the same second, and
/// the list must not report the same instant twice.
pub fn round_dedup(times: &[f64], precision: u32) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::with_capacity(times.len());
    for &t in times {
        let r = round_to(t, precision);
        if out.last().map(|&last| last == r) != Some(true) {
            out.push(r);
        }
    }
    out
}

/// Format `seconds` as `HH:MM:SS` with `precision` fractional digits (none at
/// `precision = 0`). Hours are not wrapped at 24 and are zero-padded to at least
/// two digits, so the strings sort lexicographically the same way they sort in
/// time.
pub fn timecode(seconds: f64, precision: u32) -> String {
    let p = precision.min(MAX_PRECISION) as usize;
    let total = if seconds.is_finite() && seconds > 0.0 {
        seconds
    } else {
        0.0
    };
    // Round FIRST, then split: 59.9999 at ms precision is 1:00.000, not 59:1.000.
    let rounded = round_to(total, precision);
    let whole = rounded.floor();
    let frac = rounded - whole;
    let whole = whole as u64;
    let (h, m, s) = (whole / 3600, (whole % 3600) / 60, whole % 60);
    if p == 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        // `frac` is < 1, so its own formatting starts with "0." — drop that.
        let frac_str = format!("{frac:.p$}");
        format!("{h:02}:{m:02}:{s:02}.{}", &frac_str[2..])
    }
}

/// Summary numbers derived from a rounded keyframe list.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stats {
    /// How many keyframes were found.
    pub count: usize,
    /// First keyframe timestamp (seconds), if any.
    pub first: Option<f64>,
    /// Last keyframe timestamp (seconds), if any.
    pub last: Option<f64>,
    /// Shortest gap between consecutive keyframes (seconds); `None` below two.
    pub min_gap: Option<f64>,
    /// Longest gap between consecutive keyframes (seconds); `None` below two.
    pub max_gap: Option<f64>,
    /// Mean gap between consecutive keyframes (seconds); `None` below two.
    pub avg_gap: Option<f64>,
}

/// Derive [`Stats`] from a rounded, ascending keyframe list. Gaps are the
/// distances between consecutive keyframes — the GOP spacing a seek or a lossless
/// cut has to land on — and are themselves rounded to `precision`.
pub fn stats(times: &[f64], precision: u32) -> Stats {
    let mut st = Stats {
        count: times.len(),
        first: times.first().copied(),
        last: times.last().copied(),
        ..Default::default()
    };
    if times.len() < 2 {
        return st;
    }
    let gaps: Vec<f64> = times.windows(2).map(|w| w[1] - w[0]).collect();
    let sum: f64 = gaps.iter().sum();
    st.min_gap = Some(round_to(
        gaps.iter().copied().fold(f64::INFINITY, f64::min),
        precision,
    ));
    st.max_gap = Some(round_to(
        gaps.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        precision,
    ));
    st.avg_gap = Some(round_to(sum / gaps.len() as f64, precision));
    st
}

/// One-line human summary of the keyframe list.
pub fn summary(st: &Stats, precision: u32) -> String {
    match (st.count, st.first, st.last, st.avg_gap) {
        (0, ..) => "No keyframes found in this video's first video stream.".to_string(),
        (1, Some(first), _, _) => format!(
            "1 keyframe, at {}.",
            timecode(first, precision.max(DEFAULT_PRECISION))
        ),
        (n, Some(first), Some(last), Some(avg)) => format!(
            "{n} keyframes from {} to {} — average gap {:.2} s, longest {:.2} s.",
            timecode(first, precision.max(DEFAULT_PRECISION)),
            timecode(last, precision.max(DEFAULT_PRECISION)),
            avg,
            st.max_gap.unwrap_or(avg),
        ),
        _ => format!("{} keyframes.", st.count),
    }
}

/// Render the keyframe list in `format` (`json`, `csv`, or `text`).
///
/// All three carry the same columns: the 1-based index, the timestamp in seconds,
/// its `HH:MM:SS.mmm` timecode, and the gap since the previous keyframe (absent
/// for the first). `text` is one timestamp per line — the plainest thing to paste
/// into a seek list — with the timecode and gap alongside.
pub fn render(times: &[f64], format: &str, precision: u32) -> Result<String, String> {
    let p = precision.min(MAX_PRECISION) as usize;
    let gap_of = |i: usize| -> Option<f64> {
        if i == 0 {
            None
        } else {
            Some(round_to(times[i] - times[i - 1], precision))
        }
    };
    match format {
        "text" => {
            let mut out = String::new();
            for (i, &t) in times.iter().enumerate() {
                let gap = match gap_of(i) {
                    Some(g) => format!("  (+{g:.p$} s)"),
                    None => String::new(),
                };
                out.push_str(&format!(
                    "{:>4}  {t:.p$}  {}{gap}\n",
                    i + 1,
                    timecode(t, precision)
                ));
            }
            Ok(out)
        }
        "csv" => {
            let mut out = String::from("index,seconds,timecode,gap_seconds\n");
            for (i, &t) in times.iter().enumerate() {
                let gap = match gap_of(i) {
                    Some(g) => format!("{g:.p$}"),
                    None => String::new(),
                };
                out.push_str(&format!(
                    "{},{t:.p$},{},{gap}\n",
                    i + 1,
                    timecode(t, precision)
                ));
            }
            Ok(out)
        }
        "json" => {
            let mut rows = String::new();
            for (i, &t) in times.iter().enumerate() {
                let gap = match gap_of(i) {
                    Some(g) => format!("{g:.p$}"),
                    None => "null".to_string(),
                };
                rows.push_str(&format!(
                    "  {{\"index\": {}, \"seconds\": {t:.p$}, \"timecode\": \"{}\", \"gap_seconds\": {gap}}}{}\n",
                    i + 1,
                    timecode(t, precision),
                    if i + 1 == times.len() { "" } else { "," }
                ));
            }
            if rows.is_empty() {
                Ok("[]".to_string())
            } else {
                Ok(format!("[\n{rows}]"))
            }
        }
        other => Err(format!(
            "unknown format '{other}' — use one of: {}",
            FORMATS.join(", ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_argv_selects_i_frames_through_showinfo() {
        let argv = detect_argv("in.mp4");
        let joined = argv.join(" ");
        assert!(joined.contains(r"eq(pict_type\,I)"), "{joined}");
        assert!(joined.contains("showinfo"), "{joined}");
        assert!(joined.contains("-f null"), "{joined}");
        assert!(joined.contains("-map 0:v:0"), "{joined}");
        // No audio/subtitles decoded, and never an output file.
        assert!(argv.iter().any(|a| a == "-an"));
        assert!(argv.iter().any(|a| a == "-sn"));
        assert_eq!(argv.last().map(String::as_str), Some("-"));
        // The input name is passed straight through after -i.
        let i = argv.iter().position(|a| a == "-i").unwrap();
        assert_eq!(argv[i + 1], "in.mp4");
    }

    #[test]
    fn detect_argv_uses_the_given_input_name() {
        let argv = detect_argv("in.webm");
        assert!(argv.contains(&"in.webm".to_string()));
    }

    #[test]
    fn parse_reads_showinfo_pts_time_only() {
        let log = "\
[Parsed_showinfo_1 @ 0x55d0] n:0 pts:0 pts_time:0 pos:48 fmt:yuv420p type:I
Input #0, mov,mp4, from 'in.mp4': Duration: 00:00:12.00, pts_time:99.0 not a filter line
[Parsed_showinfo_1 @ 0x55d0] n:1 pts:48048 pts_time:2.002 pos:9012 fmt:yuv420p type:I
[Parsed_showinfo_1 @ 0x55d0] n:2 pts:96096 pts_time:4.004 pos:19000 fmt:yuv420p type:I
frame=    3 fps=0.0 q=-0.0 size=N/A time=00:00:04.00 bitrate=N/A speed=8x";
        assert_eq!(parse_keyframes(log), vec![0.0, 2.002, 4.004]);
    }

    #[test]
    fn parse_sorts_and_dedups() {
        let log = "\
[Parsed_showinfo_0 @ 0x1] pts_time:4.004 type:I
[Parsed_showinfo_0 @ 0x1] pts_time:0.000 type:I
[Parsed_showinfo_0 @ 0x1] pts_time:4.004 type:I
[Parsed_showinfo_0 @ 0x1] pts_time:2.002 type:I";
        assert_eq!(parse_keyframes(log), vec![0.0, 2.002, 4.004]);
    }

    #[test]
    fn parse_skips_unreadable_and_negative_timestamps() {
        let log = "\
[Parsed_showinfo_0 @ 0x1] n:0 pts:N/A pts_time:N/A type:I
[Parsed_showinfo_0 @ 0x1] n:1 pts:-100 pts_time:-0.5 type:I
[Parsed_showinfo_0 @ 0x1] n:2 pts:0 pts_time:1.5 type:I
[Parsed_showinfo_0 @ 0x1] n:3 missing the field entirely type:I";
        assert_eq!(parse_keyframes(log), vec![1.5]);
    }

    #[test]
    fn parse_empty_log_is_empty_not_an_error() {
        assert!(parse_keyframes("").is_empty());
        assert!(parse_keyframes("ffmpeg version 6.0\nStream #0:0: Video: h264").is_empty());
    }

    #[test]
    fn round_dedup_collapses_only_after_rounding() {
        let raw = vec![0.0, 0.041708, 2.002, 2.043];
        // Milliseconds keep all four apart.
        assert_eq!(round_dedup(&raw, 3), vec![0.0, 0.042, 2.002, 2.043]);
        // Whole seconds collapse each pair into one instant.
        assert_eq!(round_dedup(&raw, 0), vec![0.0, 2.0]);
    }

    #[test]
    fn timecode_formats_hours_and_fraction() {
        assert_eq!(timecode(0.0, 3), "00:00:00.000");
        assert_eq!(timecode(2.002, 3), "00:00:02.002");
        assert_eq!(timecode(3725.5, 1), "01:02:05.5");
        // At whole-second precision the timestamp is rounded to the nearest
        // second, the same way the `seconds` column is.
        assert_eq!(timecode(3725.4, 0), "01:02:05");
        assert_eq!(timecode(3725.6, 0), "01:02:06");
        // Rounds up across the second boundary rather than printing 59.1000.
        assert_eq!(timecode(59.9999, 3), "00:01:00.000");
    }

    #[test]
    fn stats_reports_count_span_and_gaps() {
        let st = stats(&[0.0, 2.0, 5.0], 3);
        assert_eq!(st.count, 3);
        assert_eq!(st.first, Some(0.0));
        assert_eq!(st.last, Some(5.0));
        assert_eq!(st.min_gap, Some(2.0));
        assert_eq!(st.max_gap, Some(3.0));
        assert_eq!(st.avg_gap, Some(2.5));
    }

    #[test]
    fn stats_of_short_lists_have_no_gaps() {
        let empty = stats(&[], 3);
        assert_eq!(empty.count, 0);
        assert_eq!(empty.first, None);
        assert_eq!(empty.avg_gap, None);

        let one = stats(&[1.25], 3);
        assert_eq!(one.count, 1);
        assert_eq!(one.first, Some(1.25));
        assert_eq!(one.last, Some(1.25));
        assert_eq!(one.max_gap, None);
    }

    #[test]
    fn render_text_is_one_keyframe_per_line() {
        let out = render(&[0.0, 2.002], "text", 3).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("0.000"), "{}", lines[0]);
        assert!(lines[0].contains("00:00:00.000"), "{}", lines[0]);
        assert!(
            !lines[0].contains('+'),
            "first line has no gap: {}",
            lines[0]
        );
        assert!(lines[1].contains("2.002"), "{}", lines[1]);
        assert!(lines[1].contains("(+2.002 s)"), "{}", lines[1]);
    }

    #[test]
    fn render_csv_has_a_header_and_a_blank_first_gap() {
        let out = render(&[0.0, 2.002, 4.004], "csv", 3).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "index,seconds,timecode,gap_seconds");
        assert_eq!(lines[1], "1,0.000,00:00:00.000,");
        assert_eq!(lines[2], "2,2.002,00:00:02.002,2.002");
        assert_eq!(lines[3], "3,4.004,00:00:04.004,2.002");
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn render_json_is_parseable_and_carries_the_same_columns() {
        let out = render(&[0.0, 2.002], "json", 3).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
        assert_eq!(v[0]["index"], 1);
        assert_eq!(v[0]["seconds"], 0.0);
        assert_eq!(v[0]["timecode"], "00:00:00.000");
        assert!(v[0]["gap_seconds"].is_null());
        assert_eq!(v[1]["gap_seconds"], 2.002);
    }

    #[test]
    fn render_of_an_empty_list_is_still_valid_output() {
        assert_eq!(render(&[], "json", 3).unwrap(), "[]");
        assert_eq!(render(&[], "text", 3).unwrap(), "");
        assert_eq!(
            render(&[], "csv", 3).unwrap(),
            "index,seconds,timecode,gap_seconds\n"
        );
    }

    #[test]
    fn render_honours_precision() {
        let out = render(&[1.5], "csv", 0).unwrap();
        assert!(out.contains("1,2,00:00:02,"), "{out}");
        let out6 = render(&[1.234567], "text", 6).unwrap();
        assert!(out6.contains("1.234567"), "{out6}");
    }

    #[test]
    fn render_rejects_an_unknown_format() {
        let err = render(&[0.0], "srt", 3).unwrap_err();
        assert!(err.contains("unknown format 'srt'"), "{err}");
        assert!(err.contains("json"), "{err}");
    }

    #[test]
    fn summary_covers_empty_single_and_many() {
        assert!(summary(&stats(&[], 3), 3).contains("No keyframes"));
        assert!(summary(&stats(&[1.0], 3), 3).contains("1 keyframe, at 00:00:01.000"));
        let many = summary(&stats(&[0.0, 2.0, 5.0], 3), 3);
        assert!(many.contains("3 keyframes"), "{many}");
        assert!(many.contains("average gap 2.50 s"), "{many}");
        assert!(many.contains("longest 3.00 s"), "{many}");
    }
}
