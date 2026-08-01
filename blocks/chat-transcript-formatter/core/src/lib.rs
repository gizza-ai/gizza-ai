//! chat-transcript-formatter core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Takes a raw, messily-formatted chat log / conversation transcript and re-emits
//! it as ONE consistently-formatted transcript that preserves speaker, time, and
//! message. It parses the common line shapes deterministically:
//!   - WhatsApp bracket: `[2023-01-05, 10:04:11] Alice: hi`
//!   - WhatsApp dash:    `05/01/2023, 10:04 AM - Alice: hi`
//!   - Timestamped:      `[10:04] Alice: hi` / `(10:04 AM) <Bob> hey`
//!   - Bare-leading time: `10:04 Alice: hi`
//!   - IRC/Discord angle: `<Alice> hi`
//!   - Plain colon:      `Alice: hi`
//! Lines that match none of those are folded into the previous message as
//! continuations. Everything is deterministic (fixed regex parsing + reassembly),
//! no LLM and no network — so a plain `Word: text` line is always read as a
//! speaker label, which is inherent to unstructured chat logs.

use regex::Regex;
use std::sync::LazyLock;

// ---- Line-shape regexes -----------------------------------------------------

// Form A/B: a date + time header (WhatsApp bracket `]` OR dash `-` separator).
static WA_DATED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)^\s*\[?\s*
          (?P<date>\d{1,4}[/.\-]\d{1,2}[/.\-]\d{1,4})[,]?\s+
          (?P<time>\d{1,2}:\d{2}(?::\d{2})?)\s*
          (?P<ap>[AaPp][Mm])?\s*
          (?:\]|-)\s*
          (?P<rest>.*)$",
    )
    .unwrap()
});
// Form C: a bracketed/parenthesized time only, no date: `[10:04] …` / `(10:04 AM) …`.
static TS_BRACKET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)^\s*[\[(]\s*
          (?P<time>\d{1,2}:\d{2}(?::\d{2})?)\s*
          (?P<ap>[AaPp][Mm])?\s*
          [\])]\s*
          (?P<rest>.*)$",
    )
    .unwrap()
});
// Form E: a bare leading time then the rest (speaker required, checked in code).
static TS_LEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)^\s*
          (?P<time>\d{1,2}:\d{2}(?::\d{2})?)\s*
          (?P<ap>[AaPp][Mm])?\s+
          (?P<rest>\S.*)$",
    )
    .unwrap()
});
// Speaker sub-forms, applied to the message remainder.
static SPK_ANGLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^<(?P<spk>[^>]{1,40})>\s*(?P<msg>.*)$").unwrap());
// A `Name:` label — the colon must be followed by whitespace or end-of-line, so a
// bare URL (`http://x`) is never mistaken for a speaker named "http".
static SPK_COLON: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<spk>[\w][\w '.\-]{0,39}?)\s*:(?P<msg>\s.*|)$").unwrap());

/// A parsed time normalized to a 24-hour clock, plus its verbatim original text.
struct Stamp {
    h24: u32,
    m: u32,
    s: Option<u32>,
    raw: String,
}

/// One parsed transcript turn.
struct Turn {
    date: Option<String>,
    time: Option<Stamp>,
    speaker: Option<String>,
    message: String,
}

const OUTPUT_FORMATS: &[&str] = &["plain", "markdown", "bracketed", "screenplay"];
const TIME_FORMATS: &[&str] = &["keep", "24h", "12h", "none"];

/// Entry point shared by the chat block, the CLI, and the web page — a thin
/// wrapper over [`format_transcript`] so every surface calls the same code.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    output_format: &str,
    time_format: &str,
    include_dates: bool,
    merge_consecutive: bool,
    blank_line_between: bool,
) -> Result<String, String> {
    format_transcript(
        input,
        output_format,
        time_format,
        include_dates,
        merge_consecutive,
        blank_line_between,
    )
}

/// Format a raw chat log into a consistent transcript.
///
/// * `output_format` — `plain` | `markdown` | `bracketed` | `screenplay`.
/// * `time_format` — `keep` | `24h` | `12h` | `none`.
/// * `include_dates` — keep the date part (when present) alongside the time.
/// * `merge_consecutive` — join consecutive turns from the same speaker.
/// * `blank_line_between` — put a blank line between turns.
pub fn format_transcript(
    input: &str,
    output_format: &str,
    time_format: &str,
    include_dates: bool,
    merge_consecutive: bool,
    blank_line_between: bool,
) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty — paste a chat log to format".into());
    }
    let out = output_format.trim().to_ascii_lowercase();
    let out = if out.is_empty() { "plain" } else { out.as_str() };
    if !OUTPUT_FORMATS.contains(&out) {
        return Err(format!(
            "output_format must be one of plain, markdown, bracketed, screenplay (got '{output_format}')"
        ));
    }
    let tf = time_format.trim().to_ascii_lowercase();
    let tf = if tf.is_empty() { "keep" } else { tf.as_str() };
    if !TIME_FORMATS.contains(&tf) {
        return Err(format!(
            "time_format must be one of keep, 24h, 12h, none (got '{time_format}')"
        ));
    }

    // Normalize the odd spaces WhatsApp exports use (narrow / non-breaking).
    let normalized = input.replace('\u{202f}', " ").replace('\u{00a0}', " ");

    let mut turns: Vec<Turn> = Vec::new();
    for raw_line in normalized.lines() {
        if raw_line.trim().is_empty() {
            continue;
        }
        match parse_line(raw_line) {
            Some(turn) => turns.push(turn),
            None => {
                // Continuation: fold into the previous turn's message.
                let piece = raw_line.trim();
                if let Some(last) = turns.last_mut() {
                    if last.message.is_empty() {
                        last.message = piece.to_string();
                    } else {
                        last.message.push(' ');
                        last.message.push_str(piece);
                    }
                } else {
                    turns.push(Turn {
                        date: None,
                        time: None,
                        speaker: None,
                        message: piece.to_string(),
                    });
                }
            }
        }
    }

    if merge_consecutive {
        turns = merge_turns(turns);
    }

    let sep = if blank_line_between { "\n\n" } else { "\n" };
    let rendered: Vec<String> = turns
        .iter()
        .map(|t| render_turn(t, out, tf, include_dates))
        .filter(|l| !l.is_empty())
        .collect();
    Ok(rendered.join(sep))
}

/// Try every timestamped/speaker line shape; `None` means a continuation line.
fn parse_line(line: &str) -> Option<Turn> {
    // Form A/B — date + time.
    if let Some(c) = WA_DATED.captures(line) {
        let (speaker, message) = split_speaker(&c["rest"]);
        return Some(Turn {
            date: Some(c["date"].to_string()),
            time: Some(parse_stamp(&c["time"], c.name("ap").map(|m| m.as_str()))),
            speaker,
            message,
        });
    }
    // Form C — bracketed/parenthesized time only.
    if let Some(c) = TS_BRACKET.captures(line) {
        let (speaker, message) = split_speaker(&c["rest"]);
        return Some(Turn {
            date: None,
            time: Some(parse_stamp(&c["time"], c.name("ap").map(|m| m.as_str()))),
            speaker,
            message,
        });
    }
    // Form E — bare leading time; only accept if a speaker actually follows
    // (otherwise a message that merely starts with a clock time would be eaten).
    if let Some(c) = TS_LEADING.captures(line) {
        let (speaker, message) = split_speaker(&c["rest"]);
        if speaker.is_some() {
            return Some(Turn {
                date: None,
                time: Some(parse_stamp(&c["time"], c.name("ap").map(|m| m.as_str()))),
                speaker,
                message,
            });
        }
    }
    // Forms D/F — `<Name> msg` or `Name: msg`, no timestamp.
    let (speaker, message) = split_speaker(line.trim());
    if speaker.is_some() {
        return Some(Turn {
            date: None,
            time: None,
            speaker,
            message,
        });
    }
    None
}

/// Split a message remainder into an optional speaker and the text.
fn split_speaker(rest: &str) -> (Option<String>, String) {
    let rest = rest.trim_start();
    if let Some(c) = SPK_ANGLE.captures(rest) {
        return (Some(c["spk"].trim().to_string()), c["msg"].trim().to_string());
    }
    if let Some(c) = SPK_COLON.captures(rest) {
        let spk = c["spk"].trim();
        if !spk.is_empty() {
            return (Some(spk.to_string()), c["msg"].trim().to_string());
        }
    }
    (None, rest.trim().to_string())
}

/// Parse an `H:MM[:SS]` token + optional am/pm into a 24-hour Stamp.
fn parse_stamp(time: &str, ap: Option<&str>) -> Stamp {
    let mut it = time.split(':');
    let h: u32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let m: u32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let s: Option<u32> = it.next().and_then(|x| x.parse().ok());
    let ap_l = ap.map(|a| a.to_ascii_lowercase());
    let mut h24 = h;
    match ap_l.as_deref() {
        Some("pm") if h != 12 => h24 = h + 12,
        Some("am") if h == 12 => h24 = 0,
        _ => {}
    }
    if h24 > 23 {
        h24 %= 24;
    }
    let raw = match ap {
        Some(a) => format!("{time} {}", a.to_ascii_uppercase()),
        None => time.to_string(),
    };
    Stamp { h24, m, s, raw }
}

/// Render a Stamp under the chosen time format (`none` handled by the caller).
fn format_stamp(st: &Stamp, tf: &str) -> String {
    match tf {
        "24h" => match st.s {
            Some(s) => format!("{:02}:{:02}:{:02}", st.h24, st.m, s),
            None => format!("{:02}:{:02}", st.h24, st.m),
        },
        "12h" => {
            let (h12, ap) = match st.h24 {
                0 => (12, "AM"),
                1..=11 => (st.h24, "AM"),
                12 => (12, "PM"),
                _ => (st.h24 - 12, "PM"),
            };
            match st.s {
                Some(s) => format!("{}:{:02}:{:02} {}", h12, st.m, s, ap),
                None => format!("{}:{:02} {}", h12, st.m, ap),
            }
        }
        // "keep"
        _ => st.raw.clone(),
    }
}

/// Merge consecutive turns from the same speaker into one block.
fn merge_turns(turns: Vec<Turn>) -> Vec<Turn> {
    let mut out: Vec<Turn> = Vec::with_capacity(turns.len());
    for t in turns {
        if let (Some(prev), Some(spk)) = (out.last_mut(), t.speaker.as_ref()) {
            if prev
                .speaker
                .as_ref()
                .is_some_and(|p| p.eq_ignore_ascii_case(spk))
            {
                if !t.message.is_empty() {
                    if prev.message.is_empty() {
                        prev.message = t.message;
                    } else {
                        prev.message.push(' ');
                        prev.message.push_str(&t.message);
                    }
                }
                continue;
            }
        }
        out.push(t);
    }
    out
}

/// Assemble one output line.
fn render_turn(t: &Turn, out: &str, tf: &str, include_dates: bool) -> String {
    let mut stamp = String::new();
    if include_dates {
        if let Some(d) = &t.date {
            stamp.push_str(d);
        }
    }
    if tf != "none" {
        if let Some(st) = &t.time {
            let ts = format_stamp(st, tf);
            if !stamp.is_empty() {
                stamp.push(' ');
            }
            stamp.push_str(&ts);
        }
    }

    let mut line = String::new();
    if !stamp.is_empty() {
        line.push('[');
        line.push_str(&stamp);
        line.push_str("] ");
    }
    if let Some(spk) = &t.speaker {
        match out {
            "markdown" => {
                line.push_str("**");
                line.push_str(spk);
                line.push_str(":** ");
            }
            "bracketed" => {
                line.push('<');
                line.push_str(spk);
                line.push_str("> ");
            }
            "screenplay" => {
                line.push_str(&spk.to_uppercase());
                line.push_str(": ");
            }
            // "plain"
            _ => {
                line.push_str(spk);
                line.push_str(": ");
            }
        }
    }
    line.push_str(&t.message);
    line.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(input: &str) -> String {
        format_transcript(input, "plain", "keep", false, false, false).unwrap()
    }

    #[test]
    fn mixed_forms_normalize() {
        // `Alice:hey` has no space after the colon → not a speaker label → folds
        // as a leading continuation with no prior turn (speakerless). `<Bob>` and
        // the timestamped `Carol:` parse normally.
        let got = fmt("Alice:hey there\n<Bob> hi\n[10:04] Carol: yo");
        assert_eq!(got, "Alice:hey there\nBob: hi\n[10:04] Carol: yo");
    }

    #[test]
    fn whatsapp_dash_and_bracket_parse() {
        let input = "05/01/2023, 10:04 AM - Alice: hello\n[2023-01-05, 22:15:30] Bob: hey";
        let got = format_transcript(input, "plain", "24h", false, false, false).unwrap();
        assert_eq!(got, "[10:04] Alice: hello\n[22:15:30] Bob: hey");
    }

    #[test]
    fn time_format_12h_and_none() {
        let input = "[13:30] Alice: afternoon\n[00:05] Bob: midnight";
        assert_eq!(
            format_transcript(input, "plain", "12h", false, false, false).unwrap(),
            "[1:30 PM] Alice: afternoon\n[12:05 AM] Bob: midnight"
        );
        assert_eq!(
            format_transcript(input, "plain", "none", false, false, false).unwrap(),
            "Alice: afternoon\nBob: midnight"
        );
    }

    #[test]
    fn output_formats_render() {
        let input = "[10:04] Alice: hi";
        assert_eq!(
            format_transcript(input, "markdown", "keep", false, false, false).unwrap(),
            "[10:04] **Alice:** hi"
        );
        assert_eq!(
            format_transcript(input, "bracketed", "keep", false, false, false).unwrap(),
            "[10:04] <Alice> hi"
        );
        assert_eq!(
            format_transcript(input, "screenplay", "keep", false, false, false).unwrap(),
            "[10:04] ALICE: hi"
        );
    }

    #[test]
    fn merge_consecutive_same_speaker() {
        let input = "Alice: one\nAlice: two\nBob: three";
        assert_eq!(
            format_transcript(input, "plain", "none", false, true, false).unwrap(),
            "Alice: one two\nBob: three"
        );
        assert_eq!(
            format_transcript(input, "plain", "none", false, false, false).unwrap(),
            "Alice: one\nAlice: two\nBob: three"
        );
    }

    #[test]
    fn continuation_folds_into_previous_message() {
        let input = "Alice: first line\nwrapped continuation\nBob: reply";
        assert_eq!(fmt(input), "Alice: first line wrapped continuation\nBob: reply");
    }

    #[test]
    fn include_dates_keeps_date() {
        let input = "05/01/2023, 10:04 - Alice: hi";
        assert_eq!(
            format_transcript(input, "plain", "24h", true, false, false).unwrap(),
            "[05/01/2023 10:04] Alice: hi"
        );
    }

    #[test]
    fn blank_line_between_turns() {
        let input = "Alice: a\nBob: b";
        assert_eq!(
            format_transcript(input, "plain", "none", false, false, true).unwrap(),
            "Alice: a\n\nBob: b"
        );
    }

    #[test]
    fn url_is_not_a_speaker() {
        let input = "Alice: see http://example.com/x now";
        assert_eq!(fmt(input), "Alice: see http://example.com/x now");
    }

    #[test]
    fn error_on_empty_and_bad_enum() {
        assert!(format_transcript("   ", "plain", "keep", false, false, false).is_err());
        assert!(format_transcript("Alice: hi", "fancy", "keep", false, false, false).is_err());
        assert!(format_transcript("Alice: hi", "plain", "military", false, false, false).is_err());
    }
}
