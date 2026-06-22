//! srt-shift core — shift every timestamp in a SubRip (.srt) subtitle file
//! forward or backward by a fixed offset. No wafer/wasm-bindgen deps; shared by
//! the chat skill block and the web page.
//!
//! A SubRip file is a sequence of cues separated by blank lines:
//!
//! ```text
//! 1
//! 00:00:01,000 --> 00:00:04,000
//! First line of dialogue.
//!
//! 2
//! 00:00:05,500 --> 00:00:07,250
//! Second line.
//! ```
//!
//! `shift` rewrites only the timing line of each cue (`HH:MM:SS,mmm -->
//! HH:MM:SS,mmm`, optionally with trailing positioning coordinates), leaving the
//! index numbers and the subtitle text untouched. A negative offset moves the
//! subtitles earlier; any timestamp that would go below zero is clamped to
//! `00:00:00,000`.

/// Largest representable SubRip timestamp: `99:59:59,999`, in milliseconds.
pub const MAX_MS: i64 = (99 * 3600 + 59 * 60 + 59) * 1000 + 999;

/// Shift every cue timestamp in the SubRip text `srt` by `offset_ms`
/// milliseconds (positive = later, negative = earlier).
///
/// - The cue index lines and the subtitle text are preserved verbatim.
/// - A timing line may carry trailing SubRip positioning coordinates after the
///   end time (e.g. `X1:0 X2:100 Y1:0 Y2:50`); those are kept as-is.
/// - Timestamps that would become negative are clamped to `00:00:00,000`;
///   timestamps past `99:59:59,999` are clamped to that maximum.
/// - The original line endings (`\n` or `\r\n`) and the trailing newline (or
///   absence of one) are preserved.
///
/// Returns `Err` when the input contains no valid timing line (so a non-SRT
/// blob or an empty string is rejected rather than silently echoed back), or
/// when a timing line is malformed.
pub fn shift(srt: &str, offset_ms: i64) -> Result<String, String> {
    if srt.trim().is_empty() {
        return Err("input is empty: provide SubRip (.srt) subtitle text".into());
    }

    // Preserve the input's exact line endings by splitting on '\n' and tracking
    // whether each line carried a trailing '\r'.
    let mut out = String::with_capacity(srt.len());
    let mut shifted_any = false;
    let lines: Vec<&str> = srt.split('\n').collect();
    let last = lines.len() - 1;

    for (i, raw) in lines.iter().enumerate() {
        let had_cr = raw.ends_with('\r');
        let line = raw.strip_suffix('\r').unwrap_or(raw);

        if is_timing_line(line) {
            out.push_str(&shift_timing_line(line, offset_ms)?);
            shifted_any = true;
        } else {
            out.push_str(line);
        }

        if had_cr {
            out.push('\r');
        }
        // Re-add the '\n' separators we split on (but not a phantom one after
        // the final segment).
        if i != last {
            out.push('\n');
        }
    }

    if !shifted_any {
        return Err(
            "no subtitle timing line found: expected lines like \
             '00:00:01,000 --> 00:00:04,000'"
                .into(),
        );
    }
    Ok(out)
}

/// Does `line` look like a SubRip timing line (`<ts> --> <ts>[ trailing]`)?
fn is_timing_line(line: &str) -> bool {
    let line = line.trim();
    match line.split_once("-->") {
        Some((a, b)) => {
            parse_timestamp(a.trim()).is_ok()
                // The end side may carry trailing positioning coords after the
                // timestamp; only the first whitespace-delimited token is the ts.
                && parse_timestamp(b.trim().split_whitespace().next().unwrap_or("")).is_ok()
        }
        None => false,
    }
}

/// Rewrite a validated timing line, shifting both timestamps and keeping any
/// surrounding whitespace / trailing coordinates.
fn shift_timing_line(line: &str, offset_ms: i64) -> Result<String, String> {
    let (a, b) = line
        .split_once("-->")
        .ok_or_else(|| format!("malformed timing line: {line:?}"))?;

    let start = parse_timestamp(a.trim())?;
    let b = b.trim();
    let (end_tok, trailing) = match b.split_once(char::is_whitespace) {
        Some((t, rest)) => (t, Some(rest.trim_start())),
        None => (b, None),
    };
    let end = parse_timestamp(end_tok)?;

    let new_start = format_timestamp(clamp_ms(start + offset_ms));
    let new_end = format_timestamp(clamp_ms(end + offset_ms));

    Ok(match trailing {
        Some(rest) if !rest.is_empty() => {
            format!("{new_start} --> {new_end} {rest}")
        }
        _ => format!("{new_start} --> {new_end}"),
    })
}

fn clamp_ms(ms: i64) -> i64 {
    ms.clamp(0, MAX_MS)
}

/// Parse a `HH:MM:SS,mmm` SubRip timestamp into milliseconds. A `.` is accepted
/// in place of the `,` decimal separator (some tools emit the VTT-style dot).
pub fn parse_timestamp(ts: &str) -> Result<i64, String> {
    let (hms, millis) = ts
        .split_once(|c| c == ',' || c == '.')
        .ok_or_else(|| format!("timestamp {ts:?} missing the ',mmm' milliseconds part"))?;

    let parts: Vec<&str> = hms.split(':').collect();
    if parts.len() != 3 {
        return Err(format!(
            "timestamp {ts:?} must be HH:MM:SS,mmm (got {} colon-separated parts)",
            parts.len()
        ));
    }
    let h: i64 = parse_field(parts[0], "hours", ts)?;
    let m: i64 = parse_field(parts[1], "minutes", ts)?;
    let s: i64 = parse_field(parts[2], "seconds", ts)?;
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

/// Format a millisecond count back into `HH:MM:SS,mmm`.
pub fn format_timestamp(ms: i64) -> String {
    let ms = ms.max(0);
    let millis = ms % 1000;
    let total_secs = ms / 1000;
    let s = total_secs % 60;
    let total_mins = total_secs / 60;
    let m = total_mins % 60;
    let h = total_mins / 60;
    format!("{h:02}:{m:02}:{s:02},{millis:03}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "1\n00:00:01,000 --> 00:00:04,000\nFirst line.\n\n2\n00:00:05,500 --> 00:00:07,250\nSecond line.\n";

    #[test]
    fn shifts_forward_by_seconds() {
        let out = shift(SAMPLE, 2000).unwrap();
        assert!(out.contains("00:00:03,000 --> 00:00:06,000"), "{out}");
        assert!(out.contains("00:00:07,500 --> 00:00:09,250"), "{out}");
        // Index lines + text are untouched.
        assert!(out.contains("\nFirst line.\n"));
        assert!(out.starts_with("1\n"));
    }

    #[test]
    fn shifts_backward_and_preserves_subsecond() {
        let out = shift(SAMPLE, -500).unwrap();
        assert!(out.contains("00:00:00,500 --> 00:00:03,500"), "{out}");
        assert!(out.contains("00:00:05,000 --> 00:00:06,750"), "{out}");
    }

    #[test]
    fn negative_result_clamps_to_zero() {
        let out = shift(SAMPLE, -10_000).unwrap();
        // 1s - 10s would be negative → clamped to 0.
        assert!(out.contains("00:00:00,000 --> 00:00:00,000"), "{out}");
    }

    #[test]
    fn preserves_crlf_line_endings() {
        let crlf = "1\r\n00:00:01,000 --> 00:00:02,000\r\nHi\r\n";
        let out = shift(crlf, 1000).unwrap();
        assert!(out.contains("00:00:02,000 --> 00:00:03,000\r\n"), "{out:?}");
        assert!(out.contains("Hi\r\n"));
    }

    #[test]
    fn preserves_trailing_positioning_coords() {
        let src = "1\n00:00:01,000 --> 00:00:02,000 X1:0 X2:100 Y1:0 Y2:50\nHi\n";
        let out = shift(src, 1000).unwrap();
        assert!(
            out.contains("00:00:02,000 --> 00:00:03,000 X1:0 X2:100 Y1:0 Y2:50"),
            "{out}"
        );
    }

    #[test]
    fn accepts_dot_separator() {
        let src = "1\n00:00:01.000 --> 00:00:02.000\nHi\n";
        let out = shift(src, 500).unwrap();
        // Output is normalized to the SubRip comma form.
        assert!(out.contains("00:00:01,500 --> 00:00:02,500"), "{out}");
    }

    #[test]
    fn no_trailing_newline_is_preserved() {
        let src = "1\n00:00:01,000 --> 00:00:02,000\nHi";
        let out = shift(src, 1000).unwrap();
        assert!(out.ends_with("Hi"), "{out:?}");
        assert!(!out.ends_with('\n'), "{out:?}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = shift("   \n  ", 1000).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn rejects_non_srt_input() {
        let err = shift("just some plain text\nno timings here", 1000).unwrap_err();
        assert!(err.contains("no subtitle timing line"), "{err}");
    }

    #[test]
    fn clamps_to_max_timestamp() {
        let src = "1\n99:59:59,000 --> 99:59:59,500\nHi\n";
        let out = shift(src, 5000).unwrap();
        assert!(out.contains("99:59:59,999 --> 99:59:59,999"), "{out}");
    }

    #[test]
    fn parse_and_format_round_trip() {
        assert_eq!(parse_timestamp("01:02:03,004").unwrap(), 3_723_004);
        assert_eq!(format_timestamp(3_723_004), "01:02:03,004");
    }

    #[test]
    fn out_of_range_minutes_is_not_a_timing_line() {
        // 99 minutes is out of the 0-59 range, so the line isn't recognized as
        // a valid timing line — and a file with no valid timing line is rejected.
        let src = "1\n00:99:00,000 --> 00:00:01,000\nHi\n";
        let err = shift(src, 0).unwrap_err();
        assert!(err.contains("no subtitle timing line"), "{err}");
    }

    #[test]
    fn parse_rejects_out_of_range_minutes() {
        // The parser itself enforces the 0-59 range.
        let err = parse_timestamp("00:99:00,000").unwrap_err();
        assert!(err.contains("0-59"), "{err}");
    }
}
