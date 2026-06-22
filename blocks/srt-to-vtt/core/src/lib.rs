//! srt-to-vtt core — convert subtitles between SubRip (.srt) and WebVTT (.vtt).
//! No wafer/wasm-bindgen deps; shared by the chat skill block and the web page.
//!
//! The two formats are nearly identical line-oriented cue lists; they differ in:
//!   - the **timestamp decimal separator**: SRT uses a comma (`00:00:01,000`),
//!     WebVTT uses a period (`00:00:01.000`);
//!   - the **hours field**: SRT always writes `HH:MM:SS,mmm`; WebVTT allows a
//!     short `MM:SS.mmm` form (no hours) and we expand it to `HH:MM:SS.mmm`;
//!   - WebVTT begins with a `WEBVTT` signature line (optionally followed by a
//!     header block) and SRT has none;
//!   - SRT numbers each cue (`1`, `2`, …); WebVTT cue numbers are optional.
//!
//! ```text
//! SRT                               WebVTT
//! 1                                 WEBVTT
//! 00:00:01,000 --> 00:00:04,000
//! Hello world.                      1
//!                                   00:00:01.000 --> 00:00:04.000
//! 2                                 Hello world.
//! 00:00:05,500 --> 00:00:07,250
//! Second line.                      2
//!                                   00:00:05.500 --> 00:00:07.250
//!                                   Second line.
//! ```
//!
//! The conversion preserves cue text, cue order, and the cue numbering verbatim;
//! it rewrites only the timing lines and adds/removes the `WEBVTT` signature.

/// Largest representable timestamp, `99:59:59.999`, in milliseconds.
pub const MAX_MS: i64 = (99 * 3600 + 59 * 60 + 59) * 1000 + 999;

/// Which direction to convert in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// SubRip (`.srt`) → WebVTT (`.vtt`).
    SrtToVtt,
    /// WebVTT (`.vtt`) → SubRip (`.srt`).
    VttToSrt,
}

/// Convert subtitle text from one format to the other.
///
/// - In **SrtToVtt** mode a `WEBVTT` signature line and a blank line are
///   prepended, and every timing line's comma separators become periods.
/// - In **VttToSrt** mode the leading `WEBVTT` header block (everything up to
///   the first blank line) is stripped, every timing line's period separators
///   become commas, and a short `MM:SS.mmm` timestamp is expanded to
///   `HH:MM:SS,mmm`. Inline cue settings after the end time (e.g. `line:90%`)
///   are dropped, since SubRip has no equivalent.
/// - Cue index numbers and the subtitle text are kept verbatim; only the timing
///   lines (and the header) change. Both Unix (`\n`) and Windows (`\r\n`) line
///   endings are preserved per line.
///
/// Returns `Err` when the input has no recognizable timing line (so a non-subtitle
/// blob or an empty string is rejected rather than silently echoed back), or when
/// a timing line is malformed.
pub fn convert(text: &str, dir: Direction) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("input is empty: provide SubRip (.srt) or WebVTT (.vtt) subtitle text".into());
    }

    // Split keeping per-line CR awareness so line endings round-trip.
    let lines: Vec<&str> = text.split('\n').collect();
    let last = lines.len() - 1;

    // For VttToSrt, locate the end of the leading WEBVTT header block: the
    // signature line plus any header lines up to (and including) the first blank
    // line. Those lines are dropped from the output.
    let header_end = match dir {
        Direction::VttToSrt => vtt_header_end(&lines),
        Direction::SrtToVtt => 0,
    };

    let mut out = String::with_capacity(text.len() + 16);
    if dir == Direction::SrtToVtt {
        out.push_str("WEBVTT\n\n");
    }

    let mut converted_any = false;
    for (i, raw) in lines.iter().enumerate() {
        if dir == Direction::VttToSrt && i < header_end {
            continue;
        }
        let had_cr = raw.ends_with('\r');
        let line = raw.strip_suffix('\r').unwrap_or(raw);

        if is_timing_line(line) {
            out.push_str(&convert_timing_line(line, dir)?);
            converted_any = true;
        } else {
            out.push_str(line);
        }

        if had_cr {
            out.push('\r');
        }
        if i != last {
            out.push('\n');
        }
    }

    if !converted_any {
        return Err(
            "no subtitle timing line found: expected lines like \
             '00:00:01,000 --> 00:00:04,000' (SRT) or '00:00:01.000 --> 00:00:04.000' (WebVTT)"
                .into(),
        );
    }
    Ok(out)
}

/// Detect the format of `text` and convert it to the *other* one.
pub fn auto_convert(text: &str) -> Result<String, String> {
    let dir = if detect_is_vtt(text) {
        Direction::VttToSrt
    } else {
        Direction::SrtToVtt
    };
    convert(text, dir)
}

/// Heuristic: WebVTT input begins (after any BOM/leading whitespace) with the
/// `WEBVTT` signature line.
pub fn detect_is_vtt(text: &str) -> bool {
    let t = text.trim_start_matches('\u{feff}');
    let first = t.lines().next().unwrap_or("").trim();
    first == "WEBVTT" || first.starts_with("WEBVTT ") || first.starts_with("WEBVTT\t")
}

/// Index (into the `\n`-split lines) of the first line *after* the WebVTT header
/// block. Everything before it (`WEBVTT …`, header metadata, the blank
/// separator) is dropped when converting to SRT. If there's no `WEBVTT`
/// signature, nothing is treated as header.
fn vtt_header_end(lines: &[&str]) -> usize {
    let first = lines.first().map(|l| l.trim()).unwrap_or("");
    let is_sig = first == "WEBVTT" || first.starts_with("WEBVTT ") || first.starts_with("WEBVTT\t");
    if !is_sig {
        return 0;
    }
    // Header runs until the first blank line (inclusive). If the file is all
    // header with no blank line, drop just the signature line.
    for (i, raw) in lines.iter().enumerate() {
        if i == 0 {
            continue;
        }
        if raw.trim().is_empty() {
            return i + 1;
        }
    }
    1
}

/// Does `line` look like a timing line (`<ts> --> <ts>[ settings]`) in either
/// format?
fn is_timing_line(line: &str) -> bool {
    let line = line.trim();
    match line.split_once("-->") {
        Some((a, b)) => {
            parse_timestamp(a.trim()).is_ok()
                && parse_timestamp(b.trim().split_whitespace().next().unwrap_or("")).is_ok()
        }
        None => false,
    }
}

/// Rewrite a validated timing line into the target format.
///
/// For SrtToVtt the cue settings after the end timestamp (rare in SRT — usually
/// positioning coords) are preserved; for VttToSrt they are dropped (SubRip has
/// no cue settings).
fn convert_timing_line(line: &str, dir: Direction) -> Result<String, String> {
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

    let (new_start, new_end) = match dir {
        Direction::SrtToVtt => (format_vtt(start), format_vtt(end)),
        Direction::VttToSrt => (format_srt(start), format_srt(end)),
    };

    match dir {
        // Keep trailing tokens (positioning) when producing WebVTT.
        Direction::SrtToVtt => Ok(match trailing {
            Some(rest) if !rest.is_empty() => format!("{new_start} --> {new_end} {rest}"),
            _ => format!("{new_start} --> {new_end}"),
        }),
        // Drop cue settings when producing SubRip.
        Direction::VttToSrt => Ok(format!("{new_start} --> {new_end}")),
    }
}

/// Parse a `HH:MM:SS,mmm` / `HH:MM:SS.mmm` / `MM:SS.mmm` timestamp into
/// milliseconds. Either `,` or `.` is accepted as the decimal separator so the
/// parser is format-agnostic; the short `MM:SS` (no hours) WebVTT form is
/// accepted too.
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
        // WebVTT short form: MM:SS (no hours).
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

    const SRT: &str =
        "1\n00:00:01,000 --> 00:00:04,000\nFirst line.\n\n2\n00:00:05,500 --> 00:00:07,250\nSecond line.\n";

    #[test]
    fn srt_to_vtt_adds_signature_and_dots() {
        let out = convert(SRT, Direction::SrtToVtt).unwrap();
        assert!(out.starts_with("WEBVTT\n\n"), "{out:?}");
        assert!(out.contains("00:00:01.000 --> 00:00:04.000"), "{out}");
        assert!(out.contains("00:00:05.500 --> 00:00:07.250"), "{out}");
        // Cue numbers + text untouched.
        assert!(out.contains("\n1\n00:00:01.000"));
        assert!(out.contains("\nFirst line.\n"));
    }

    #[test]
    fn vtt_to_srt_strips_header_and_uses_commas() {
        let vtt = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:04.000\nFirst line.\n";
        let out = convert(vtt, Direction::VttToSrt).unwrap();
        assert!(!out.contains("WEBVTT"), "{out:?}");
        assert!(out.starts_with("1\n00:00:01,000 --> 00:00:04,000"), "{out:?}");
        assert!(out.contains("\nFirst line.\n"));
    }

    #[test]
    fn round_trip_srt_vtt_srt() {
        let vtt = convert(SRT, Direction::SrtToVtt).unwrap();
        let back = convert(&vtt, Direction::VttToSrt).unwrap();
        // Same timings + text restored (the original has no trailing blank-line
        // weirdness, so it should match exactly).
        assert_eq!(back, SRT, "round trip changed content:\n{back}");
    }

    #[test]
    fn vtt_to_srt_expands_short_form_and_drops_cue_settings() {
        let vtt = "WEBVTT\n\n00:01.000 --> 00:04.000 line:90% align:center\nHi\n";
        let out = convert(vtt, Direction::VttToSrt).unwrap();
        assert!(out.contains("00:00:01,000 --> 00:00:04,000"), "{out}");
        assert!(!out.contains("line:90%"), "cue settings should be dropped: {out}");
    }

    #[test]
    fn vtt_to_srt_drops_header_metadata_block() {
        let vtt = "WEBVTT - Some title\nKind: captions\nLanguage: en\n\n1\n00:00:01.000 --> 00:00:02.000\nHi\n";
        let out = convert(vtt, Direction::VttToSrt).unwrap();
        assert!(out.starts_with("1\n00:00:01,000"), "{out:?}");
        assert!(!out.contains("Kind:"), "{out}");
    }

    #[test]
    fn preserves_crlf_line_endings() {
        let crlf = "1\r\n00:00:01,000 --> 00:00:02,000\r\nHi\r\n";
        let out = convert(crlf, Direction::SrtToVtt).unwrap();
        assert!(out.contains("00:00:01.000 --> 00:00:02.000\r\n"), "{out:?}");
        assert!(out.contains("Hi\r\n"), "{out:?}");
    }

    #[test]
    fn srt_to_vtt_preserves_trailing_positioning() {
        let src = "1\n00:00:01,000 --> 00:00:02,000 X1:0 X2:100\nHi\n";
        let out = convert(src, Direction::SrtToVtt).unwrap();
        assert!(out.contains("00:00:01.000 --> 00:00:02.000 X1:0 X2:100"), "{out}");
    }

    #[test]
    fn auto_detects_direction() {
        // SRT input → auto produces VTT.
        let v = auto_convert(SRT).unwrap();
        assert!(v.starts_with("WEBVTT"), "{v:?}");
        // VTT input → auto produces SRT.
        let s = auto_convert("WEBVTT\n\n1\n00:00:01.000 --> 00:00:02.000\nHi\n").unwrap();
        assert!(!s.contains("WEBVTT"), "{s:?}");
        assert!(s.contains("00:00:01,000 --> 00:00:02,000"), "{s}");
    }

    #[test]
    fn detect_handles_bom_and_header() {
        assert!(detect_is_vtt("\u{feff}WEBVTT\n"));
        assert!(detect_is_vtt("WEBVTT - title\n"));
        assert!(!detect_is_vtt("1\n00:00:01,000 --> 00:00:02,000\nHi\n"));
    }

    #[test]
    fn rejects_empty_input() {
        let err = convert("  \n  ", Direction::SrtToVtt).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn rejects_non_subtitle_input() {
        let err = convert("just some text\nno timings", Direction::SrtToVtt).unwrap_err();
        assert!(err.contains("no subtitle timing line"), "{err}");
    }

    #[test]
    fn parse_accepts_both_separators_and_short_form() {
        assert_eq!(parse_timestamp("01:02:03,004").unwrap(), 3_723_004);
        assert_eq!(parse_timestamp("01:02:03.004").unwrap(), 3_723_004);
        assert_eq!(parse_timestamp("02:03.004").unwrap(), 123_004);
    }

    #[test]
    fn format_helpers() {
        assert_eq!(format_srt(3_723_004), "01:02:03,004");
        assert_eq!(format_vtt(3_723_004), "01:02:03.004");
    }
}
