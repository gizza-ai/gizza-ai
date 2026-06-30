//! subtitle-merge core — combine two or more SubRip (.srt) / WebVTT (.vtt)
//! subtitle files into one, with an optional cumulative time-shift. No
//! wafer/wasm-bindgen deps; shared by the chat skill block and the web page.
//!
//! Inputs are concatenated into a single blob, each file separated by a line
//! that contains only `===` (three or more equals signs). Every cue from every
//! file is parsed into an absolute-millisecond `(start, end, text)` triple, the
//! whole set is sorted by start time (a *stable* sort, so cues that start at the
//! same instant keep their original file order — useful for overlaying a second
//! language track), renumbered `1..N`, and re-emitted in the chosen format.
//!
//! ```text
//! 1                                 1
//! 00:00:01,000 --> 00:00:02,000     00:00:01,000 --> 00:00:02,000
//! Hello.                            Hello.
//!                              →
//! ===                               2
//!                                   00:00:03,000 --> 00:00:04,000
//! 1                                 Bonjour.
//! 00:00:03,000 --> 00:00:04,000
//! Bonjour.
//! ```
//!
//! The optional `offset_ms` shifts each file *after the first* by a cumulative
//! multiple of the offset (file 2 by `offset_ms`, file 3 by `2 * offset_ms`, …),
//! which lines up sequentially-split parts (CD1/CD2) on one timeline. Leave it
//! at `0` to overlay tracks that already share a timeline.

/// Largest representable timestamp, `99:59:59.999`, in milliseconds.
pub const MAX_MS: i64 = (99 * 3600 + 59 * 60 + 59) * 1000 + 999;

/// Output subtitle format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutFormat {
    /// Match the format of the first input segment.
    Auto,
    /// SubRip (`.srt`): comma decimal separator, no header.
    Srt,
    /// WebVTT (`.vtt`): period decimal separator, `WEBVTT` header.
    Vtt,
}

impl OutFormat {
    /// Parse the page/CLI string form (`"auto"`/`"srt"`/`"vtt"`, case-insensitive,
    /// blank → `Auto`).
    pub fn parse(s: &str) -> Result<OutFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(OutFormat::Auto),
            "srt" | "subrip" => Ok(OutFormat::Srt),
            "vtt" | "webvtt" => Ok(OutFormat::Vtt),
            other => Err(format!(
                "invalid format {other:?}: expected \"auto\", \"srt\", or \"vtt\""
            )),
        }
    }
}

/// A single subtitle cue, normalized to absolute milliseconds.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Cue {
    start: i64,
    end: i64,
    text: String,
}

/// Merge the concatenated subtitle `input` (segments separated by a `===` line)
/// into one file.
///
/// - Each segment may be SubRip or WebVTT — the format is detected per segment,
///   so you can mix `.srt` and `.vtt` inputs.
/// - `offset_ms` is added cumulatively: segment index `i` (0-based) is shifted by
///   `i * offset_ms` milliseconds (clamped to `0..=MAX_MS`).
/// - All cues are merged, **stable-sorted** by start time, renumbered from 1, and
///   rendered in `out` (`Auto` follows the first segment's detected format).
///
/// Returns `Err` when the input is empty or no segment contains a recognizable
/// timing line.
pub fn merge(input: &str, offset_ms: i64, out: OutFormat) -> Result<String, String> {
    let segments = split_segments(input);
    if segments.is_empty() {
        return Err("input is empty: provide one or more subtitle files separated by a '===' line".into());
    }

    let first_is_vtt = detect_is_vtt(segments[0]);

    let mut cues: Vec<Cue> = Vec::new();
    for (i, seg) in segments.iter().enumerate() {
        let shift = (i as i64).saturating_mul(offset_ms);
        for mut cue in parse_cues(seg) {
            cue.start = clamp_ms(cue.start.saturating_add(shift));
            cue.end = clamp_ms(cue.end.saturating_add(shift));
            cues.push(cue);
        }
    }

    if cues.is_empty() {
        return Err(
            "no subtitle cues found: expected timing lines like \
             '00:00:01,000 --> 00:00:04,000'"
                .into(),
        );
    }

    // Stable sort: equal start times keep their original (file, position) order.
    cues.sort_by_key(|c| c.start);

    let as_vtt = match out {
        OutFormat::Auto => first_is_vtt,
        OutFormat::Srt => false,
        OutFormat::Vtt => true,
    };
    Ok(render(&cues, as_vtt))
}

/// Split the input on separator lines (a line whose only non-whitespace content
/// is three or more `=`). Empty/whitespace-only segments are dropped.
fn split_segments(input: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = 0usize; // byte index where the current segment begins
    let mut idx = 0usize; // running byte index at each line start

    for line in input.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']).trim();
        let is_sep = trimmed.len() >= 3 && trimmed.bytes().all(|b| b == b'=');
        if is_sep {
            let seg = &input[start..idx];
            if !seg.trim().is_empty() {
                segments.push(seg);
            }
            start = idx + line.len();
        }
        idx += line.len();
    }
    // Trailing segment after the last separator (or the whole input if none).
    let seg = &input[start..];
    if !seg.trim().is_empty() {
        segments.push(seg);
    }
    segments
}

/// Heuristic: a WebVTT segment begins (after any BOM/leading whitespace) with the
/// `WEBVTT` signature line.
fn detect_is_vtt(text: &str) -> bool {
    let t = text.trim_start_matches('\u{feff}').trim_start();
    let first = t.lines().next().unwrap_or("").trim();
    first == "WEBVTT" || first.starts_with("WEBVTT ") || first.starts_with("WEBVTT\t")
}

/// Parse every cue in one subtitle segment into absolute-millisecond triples.
/// Cue index numbers and WebVTT cue identifiers/headers are discarded (the merge
/// renumbers from 1); only the timing line + the dialogue lines that follow it
/// (up to the next blank line or timing line) are kept.
fn parse_cues(text: &str) -> Vec<Cue> {
    let lines: Vec<&str> = text.lines().collect();
    let mut cues = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i].trim();
        if let Some((start, end)) = parse_timing_line(line) {
            i += 1;
            let mut body: Vec<&str> = Vec::new();
            while i < lines.len() {
                let raw = lines[i];
                if raw.trim().is_empty() {
                    break;
                }
                // A new timing line starts the next cue without a blank separator.
                if parse_timing_line(raw.trim()).is_some() {
                    break;
                }
                body.push(raw);
                i += 1;
            }
            cues.push(Cue {
                start,
                end,
                text: body.join("\n"),
            });
        } else {
            i += 1;
        }
    }
    cues
}

/// If `line` is a timing line (`<ts> --> <ts>[ settings]`) return its
/// `(start, end)` in ms; otherwise `None`.
fn parse_timing_line(line: &str) -> Option<(i64, i64)> {
    let (a, b) = line.split_once("-->")?;
    let start = parse_timestamp(a.trim()).ok()?;
    // The end side may carry trailing cue settings / positioning coords after
    // the timestamp; only the first whitespace token is the end timestamp.
    let end_tok = b.trim().split_whitespace().next().unwrap_or("");
    let end = parse_timestamp(end_tok).ok()?;
    Some((start, end))
}

/// Parse a `HH:MM:SS,mmm` / `HH:MM:SS.mmm` / `MM:SS.mmm` timestamp into ms.
/// Either `,` or `.` is accepted as the decimal separator; the short `MM:SS`
/// (no hours) WebVTT form is accepted too.
pub fn parse_timestamp(ts: &str) -> Result<i64, String> {
    let (hms, millis) = ts
        .split_once(|c| c == ',' || c == '.')
        .ok_or_else(|| format!("timestamp {ts:?} missing the milliseconds part"))?;

    let parts: Vec<&str> = hms.split(':').collect();
    let (h, m, s) = match parts.len() {
        3 => (
            parse_field(parts[0], "hours", ts)?,
            parse_field(parts[1], "minutes", ts)?,
            parse_field(parts[2], "seconds", ts)?,
        ),
        2 => (
            0,
            parse_field(parts[0], "minutes", ts)?,
            parse_field(parts[1], "seconds", ts)?,
        ),
        n => {
            return Err(format!(
                "timestamp {ts:?} must be HH:MM:SS or MM:SS (got {n} colon-separated parts)"
            ))
        }
    };
    if millis.len() != 3 {
        return Err(format!(
            "timestamp {ts:?}: milliseconds must be exactly 3 digits"
        ));
    }
    let ms: i64 = parse_field(millis, "milliseconds", ts)?;
    if m > 59 || s > 59 {
        return Err(format!("timestamp {ts:?}: minutes/seconds must be 0-59"));
    }
    Ok(((h * 60 + m) * 60 + s) * 1000 + ms)
}

fn parse_field(field: &str, what: &str, ts: &str) -> Result<i64, String> {
    field
        .parse::<i64>()
        .map_err(|_| format!("timestamp {ts:?}: invalid {what} {field:?}"))
}

fn clamp_ms(ms: i64) -> i64 {
    ms.clamp(0, MAX_MS)
}

/// Render the (already sorted) cues as a single subtitle file.
fn render(cues: &[Cue], as_vtt: bool) -> String {
    let mut out = String::with_capacity(cues.len() * 48 + 8);
    if as_vtt {
        out.push_str("WEBVTT\n\n");
    }
    for (idx, cue) in cues.iter().enumerate() {
        out.push_str(&(idx + 1).to_string());
        out.push('\n');
        let (start, end) = if as_vtt {
            (format_vtt(cue.start), format_vtt(cue.end))
        } else {
            (format_srt(cue.start), format_srt(cue.end))
        };
        out.push_str(&start);
        out.push_str(" --> ");
        out.push_str(&end);
        out.push('\n');
        if !cue.text.is_empty() {
            out.push_str(&cue.text);
            out.push('\n');
        }
        out.push('\n');
    }
    // Drop the final trailing blank line so output ends with a single newline.
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}

/// Format milliseconds as a SubRip timestamp: `HH:MM:SS,mmm`.
pub fn format_srt(ms: i64) -> String {
    let (h, m, s, millis) = split_ms(ms);
    format!("{h:02}:{m:02}:{s:02},{millis:03}")
}

/// Format milliseconds as a WebVTT timestamp: `HH:MM:SS.mmm`.
pub fn format_vtt(ms: i64) -> String {
    let (h, m, s, millis) = split_ms(ms);
    format!("{h:02}:{m:02}:{s:02}.{millis:03}")
}

fn split_ms(ms: i64) -> (i64, i64, i64, i64) {
    let ms = ms.clamp(0, MAX_MS);
    let millis = ms % 1000;
    let total_secs = ms / 1000;
    let s = total_secs % 60;
    let total_mins = total_secs / 60;
    let m = total_mins % 60;
    let h = total_mins / 60;
    (h, m, s, millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "1\n00:00:05,000 --> 00:00:07,000\nSecond.\n";
    const B: &str = "1\n00:00:01,000 --> 00:00:02,000\nFirst.\n";

    #[test]
    fn merges_and_sorts_by_start_time() {
        let input = format!("{A}\n===\n{B}");
        let out = merge(&input, 0, OutFormat::Srt).unwrap();
        // B (starts at 1s) sorts before A (starts at 5s); renumbered 1,2.
        assert_eq!(
            out,
            "1\n00:00:01,000 --> 00:00:02,000\nFirst.\n\n2\n00:00:05,000 --> 00:00:07,000\nSecond.\n"
        );
    }

    #[test]
    fn cumulative_offset_shifts_each_file_after_the_first() {
        // Two identical files, offset 10s → second file's cue lands 10s later.
        let input = format!("{B}\n===\n{B}");
        let out = merge(&input, 10_000, OutFormat::Srt).unwrap();
        assert!(out.contains("00:00:01,000 --> 00:00:02,000"), "{out}");
        assert!(out.contains("00:00:11,000 --> 00:00:12,000"), "{out}");
    }

    #[test]
    fn offset_clamps_at_zero() {
        let input = format!("{B}\n===\n{B}");
        // Negative cumulative offset would push the second file below zero → clamp.
        let out = merge(&input, -10_000, OutFormat::Srt).unwrap();
        assert!(out.contains("00:00:00,000 --> 00:00:00,000"), "{out}");
    }

    #[test]
    fn auto_format_follows_first_segment_vtt() {
        let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nHi.\n";
        let out = merge(vtt, 0, OutFormat::Auto).unwrap();
        assert!(out.starts_with("WEBVTT\n\n"), "{out:?}");
        assert!(out.contains("00:00:01.000 --> 00:00:02.000"), "{out}");
    }

    #[test]
    fn forces_srt_output_from_vtt_input() {
        let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nHi.\n";
        let out = merge(vtt, 0, OutFormat::Srt).unwrap();
        assert!(!out.contains("WEBVTT"), "{out:?}");
        assert!(out.contains("00:00:01,000 --> 00:00:02,000"), "{out}");
    }

    #[test]
    fn mixes_srt_and_vtt_inputs() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nFrom SRT.\n";
        let vtt = "WEBVTT\n\n00:00:03.000 --> 00:00:04.000\nFrom VTT.\n";
        let input = format!("{srt}\n===\n{vtt}");
        let out = merge(&input, 0, OutFormat::Vtt).unwrap();
        assert!(out.starts_with("WEBVTT"), "{out:?}");
        assert!(out.contains("From SRT."), "{out}");
        assert!(out.contains("From VTT."), "{out}");
        assert!(out.contains("00:00:01.000 --> 00:00:02.000"), "{out}");
    }

    #[test]
    fn multiline_cue_text_preserved() {
        let s = "1\n00:00:01,000 --> 00:00:02,000\nLine one\nLine two\n";
        let out = merge(s, 0, OutFormat::Srt).unwrap();
        assert!(out.contains("Line one\nLine two"), "{out}");
    }

    #[test]
    fn single_file_no_separator_just_renumbers() {
        let s = "7\n00:00:02,000 --> 00:00:03,000\nB.\n\n3\n00:00:01,000 --> 00:00:01,500\nA.\n";
        let out = merge(s, 0, OutFormat::Srt).unwrap();
        // Sorted by start and renumbered 1,2 regardless of input indices.
        assert!(out.starts_with("1\n00:00:01,000 --> 00:00:01,500\nA.\n"), "{out:?}");
        assert!(out.contains("2\n00:00:02,000 --> 00:00:03,000\nB."), "{out}");
    }

    #[test]
    fn stable_order_for_equal_start_times() {
        // Same start → original file order is kept (overlay use case).
        let a = "1\n00:00:01,000 --> 00:00:02,000\nEnglish.\n";
        let b = "1\n00:00:01,000 --> 00:00:02,000\nFrench.\n";
        let input = format!("{a}\n===\n{b}");
        let out = merge(&input, 0, OutFormat::Srt).unwrap();
        let eng = out.find("English.").unwrap();
        let fre = out.find("French.").unwrap();
        assert!(eng < fre, "stable order broken: {out}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = merge("   \n === \n  ", 0, OutFormat::Srt).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn rejects_non_subtitle_input() {
        let err = merge("just some text\nno timings here", 0, OutFormat::Srt).unwrap_err();
        assert!(err.contains("no subtitle cues"), "{err}");
    }

    #[test]
    fn separator_needs_three_equals() {
        // Two equals is NOT a separator — it stays part of the (single) segment,
        // and since it's not a timing line it's simply ignored.
        let input = "1\n00:00:01,000 --> 00:00:02,000\nA.\n==\n1\n00:00:03,000 --> 00:00:04,000\nB.";
        let out = merge(input, 1000, OutFormat::Srt).unwrap();
        // One segment → offset never applied; both cues at their original times.
        assert!(out.contains("00:00:01,000 --> 00:00:02,000"), "{out}");
        assert!(out.contains("00:00:03,000 --> 00:00:04,000"), "{out}");
    }

    #[test]
    fn drops_vtt_cue_settings() {
        let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000 line:90% align:center\nHi\n";
        let out = merge(vtt, 0, OutFormat::Srt).unwrap();
        assert!(out.contains("00:00:01,000 --> 00:00:02,000"), "{out}");
        assert!(!out.contains("line:90%"), "{out}");
    }

    #[test]
    fn parse_and_format_round_trip() {
        assert_eq!(parse_timestamp("01:02:03,004").unwrap(), 3_723_004);
        assert_eq!(parse_timestamp("01:02:03.004").unwrap(), 3_723_004);
        assert_eq!(parse_timestamp("02:03.004").unwrap(), 123_004);
        assert_eq!(format_srt(3_723_004), "01:02:03,004");
        assert_eq!(format_vtt(3_723_004), "01:02:03.004");
    }

    #[test]
    fn out_format_parse() {
        assert_eq!(OutFormat::parse("").unwrap(), OutFormat::Auto);
        assert_eq!(OutFormat::parse("AUTO").unwrap(), OutFormat::Auto);
        assert_eq!(OutFormat::parse("srt").unwrap(), OutFormat::Srt);
        assert_eq!(OutFormat::parse("WebVTT").unwrap(), OutFormat::Vtt);
        assert!(OutFormat::parse("xml").is_err());
    }
}
