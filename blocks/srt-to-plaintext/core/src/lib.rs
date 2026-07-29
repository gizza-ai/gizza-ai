//! srt-to-plaintext core — strip the structural scaffolding out of a SubRip
//! (`.srt`) subtitle file and return just the spoken transcript text. Pure
//! compute, shared by the chat skill block and the web page; no wafer /
//! wasm-bindgen deps.
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
//! `to_plaintext` drops the cue index numbers, the `-->` timing lines, and the
//! blank separators, keeping only the caption text. Optional cleaning removes
//! formatting tags, bracketed sound-effect/music cues, and leading speaker
//! labels, and can collapse consecutive duplicate cues (common in rolling
//! auto-captions). The `layout` chooses how the surviving text is joined:
//! one line per cue, the original per-cue segmentation, or one flowing
//! paragraph.
//!
//! Everything is DETERMINISTIC — fixed rules, no LLM, no network. The
//! speaker-label heuristic (a leading `NAME:`) can occasionally clip a genuine
//! `Word:` at the start of a line, so it ships off by default.
//!
//! WebVTT input is tolerated as well: timing lines with a `.` decimal separator
//! are recognized, and a leading `WEBVTT` signature plus `NOTE`/`STYLE` header
//! blocks are dropped. The dedicated converter is the sibling `srt-to-vtt` tool.

/// How the surviving caption text is laid out in the output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// One line per cue: a cue's internal line breaks are joined with a space,
    /// and cues are separated by a single newline. The default — a clean,
    /// one-caption-per-line transcript.
    Lines,
    /// Preserve each cue's original line breaks, with a blank line between
    /// cues (the SubRip segmentation, minus the numbers and timing lines).
    Blocks,
    /// Join every cue into one continuous, space-separated paragraph.
    Paragraph,
}

impl Layout {
    /// Parse the layout name used by the chat schema / CLI / page.
    pub fn parse(s: &str) -> Result<Layout, String> {
        match s.trim() {
            "" | "lines" => Ok(Layout::Lines),
            "blocks" => Ok(Layout::Blocks),
            "paragraph" => Ok(Layout::Paragraph),
            other => Err(format!(
                "invalid layout {other:?}: expected \"lines\", \"blocks\", or \"paragraph\""
            )),
        }
    }
}

/// The optional cleaning + layout knobs.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// How to join the surviving caption text.
    pub layout: Layout,
    /// Remove HTML-style formatting tags (`<i>`, `<b>`, `<font …>`) and ASS/SSA
    /// override blocks (`{\an8}`). Default on.
    pub strip_tags: bool,
    /// Remove bracketed non-speech cues — `[door slams]`, `(applause)` — and
    /// musical-note markers (♪ ♫). A line that becomes empty is dropped.
    pub remove_sound_effects: bool,
    /// Remove a leading speaker label at the start of a line — `NARRATOR:` or a
    /// dash-prefixed `- JOHN:`. Heuristic; off by default.
    pub remove_speaker_labels: bool,
    /// Collapse consecutive duplicate cues into one (rolling-caption dedupe).
    pub dedupe: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            layout: Layout::Lines,
            strip_tags: true,
            remove_sound_effects: false,
            remove_speaker_labels: false,
            dedupe: false,
        }
    }
}

/// Convert SubRip/WebVTT subtitle text to a plain-text transcript.
///
/// Returns `Err` when the input is empty, or when it contains no recognizable
/// timing line (so a non-subtitle blob is rejected rather than echoed back).
pub fn to_plaintext(text: &str, opts: Options) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("input is empty: provide SubRip (.srt) subtitle text".into());
    }

    // Normalize line endings and strip a leading BOM.
    let normalized = text
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");

    // Segment into blocks on blank lines. Each block holds the lines of one cue
    // (plus, in SubRip, a leading index and a timing line).
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in normalized.split('\n') {
        if line.trim().is_empty() {
            if !current.is_empty() {
                blocks.push(std::mem::take(&mut current));
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        blocks.push(current);
    }

    let mut saw_timing = false;
    // Each surviving cue's cleaned text lines.
    let mut cues: Vec<Vec<String>> = Vec::new();

    for block in &blocks {
        // Drop WebVTT signature / comment / style header blocks entirely.
        let head = block[0].trim();
        if head == "WEBVTT" || head.starts_with("WEBVTT ") || head.starts_with("WEBVTT\t") {
            continue;
        }
        if head == "NOTE" || head.starts_with("NOTE ") || head == "STYLE" {
            continue;
        }

        // A leading all-digits line immediately followed by a timing line is a
        // SubRip cue index — drop it. (A numeric caption like "911" with no
        // timing line after it is kept.)
        let start = if block.len() >= 2 && is_cue_index(block[0]) && is_timing_line(block[1]) {
            1
        } else {
            0
        };

        let mut lines: Vec<String> = Vec::new();
        for &raw in &block[start..] {
            if is_timing_line(raw) {
                saw_timing = true;
                continue;
            }
            if let Some(cleaned) = clean_line(raw, &opts) {
                lines.push(cleaned);
            }
        }
        if !lines.is_empty() {
            cues.push(lines);
        }
    }

    if !saw_timing {
        return Err(
            "no subtitle timing line found: expected SubRip cues with a timing line like \
             '00:00:01,000 --> 00:00:04,000'"
                .into(),
        );
    }

    // Collapse each cue to a single string, preserving internal breaks for the
    // Blocks layout and folding them to spaces otherwise.
    let mut rendered: Vec<String> = cues
        .iter()
        .map(|lines| match opts.layout {
            Layout::Blocks => lines.join("\n"),
            Layout::Lines | Layout::Paragraph => collapse_ws(&lines.join(" ")),
        })
        .filter(|s| !s.is_empty())
        .collect();

    if opts.dedupe {
        rendered.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    }

    if rendered.is_empty() {
        return Err("no caption text found: every cue was empty after cleaning".into());
    }

    Ok(match opts.layout {
        Layout::Blocks => rendered.join("\n\n"),
        Layout::Lines => rendered.join("\n"),
        Layout::Paragraph => rendered.join(" "),
    })
}

/// Convenience wrapper used by simple callers: default one-line-per-cue
/// transcript with formatting tags stripped.
pub fn run(input: &str) -> Result<String, String> {
    to_plaintext(input, Options::default())
}

/// Convert subtitle text using option values from the chat schema / CLI / page.
pub fn convert(
    input: &str,
    layout: &str,
    strip_tags: bool,
    remove_sound_effects: bool,
    remove_speaker_labels: bool,
    dedupe: bool,
) -> Result<String, String> {
    to_plaintext(
        input,
        Options {
            layout: Layout::parse(layout)?,
            strip_tags,
            remove_sound_effects,
            remove_speaker_labels,
            dedupe,
        },
    )
}

/// Is `line` a bare SubRip cue index (all ASCII digits)?
fn is_cue_index(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit())
}

/// Does `line` look like a timing line (`<ts> --> <ts>[ settings]`) in SubRip
/// or WebVTT form? Accepts either `,` or `.` as the millisecond separator.
fn is_timing_line(line: &str) -> bool {
    match line.trim().split_once("-->") {
        Some((a, b)) => {
            let b_first = b.trim().split_whitespace().next().unwrap_or("");
            is_timestamp(a.trim()) && is_timestamp(b_first)
        }
        None => false,
    }
}

/// `HH:MM:SS,mmm` / `HH:MM:SS.mmm` / `MM:SS.mmm` shaped token?
fn is_timestamp(ts: &str) -> bool {
    let (hms, millis) = match ts.split_once(|c| c == ',' || c == '.') {
        Some(x) => x,
        None => return false,
    };
    if millis.len() != 3 || !millis.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let parts: Vec<&str> = hms.split(':').collect();
    if parts.len() != 2 && parts.len() != 3 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Apply the enabled cleaning steps to one caption line. Returns `None` when the
/// line is empty after cleaning (so it is dropped).
fn clean_line(raw: &str, opts: &Options) -> Option<String> {
    let mut s = raw.to_string();
    if opts.strip_tags {
        s = strip_delimited(&s, '<', '>');
        s = strip_delimited(&s, '{', '}');
    }
    if opts.remove_sound_effects {
        s = strip_delimited(&s, '[', ']');
        s = strip_delimited(&s, '(', ')');
        s = s.replace(['\u{266a}', '\u{266b}', '\u{2669}', '\u{266c}'], " ");
    }
    if opts.remove_speaker_labels {
        s = strip_speaker_label(&s);
    }
    let s = s.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Remove every `open…close` span from `s`. An unmatched opener is left as-is so
/// legitimate text like `2 < 3` (no closing `>`) survives.
fn strip_delimited(s: &str, open: char, close: char) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == open {
            if let Some(rel) = chars[i + 1..].iter().position(|&c| c == close) {
                i = i + 1 + rel + 1; // skip the span and its closing char
                continue;
            }
            out.push(chars[i]); // no closer on this line — keep literally
        } else {
            out.push(chars[i]);
        }
        i += 1;
    }
    out
}

/// Strip a leading speaker label: an optional `- ` dash, then a short label of
/// letters/digits/spaces/`.`/`'`/`#`/`-`/`_` ending in `:`. Kept conservative
/// (label ≤ 30 chars, at least one letter) to avoid clipping real dialogue like
/// `5: the number`.
fn strip_speaker_label(s: &str) -> String {
    let trimmed = s.trim_start();
    let lead_ws = &s[..s.len() - trimmed.len()];
    let body = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    if let Some(colon) = body.find(':') {
        let label = &body[..colon];
        let ok = colon <= 30
            && !label.is_empty()
            && label.chars().any(|c| c.is_alphabetic())
            && label
                .chars()
                .all(|c| c.is_alphanumeric() || " .'#-_".contains(c));
        if ok {
            let rest = body[colon + 1..].trim_start();
            return format!("{lead_ws}{rest}");
        }
    }
    s.to_string()
}

/// Collapse runs of ASCII whitespace to single spaces and trim the ends.
fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRT: &str = "1\n00:00:01,000 --> 00:00:04,000\nFirst line.\n\n2\n00:00:05,500 --> 00:00:07,250\nSecond line\nthat wraps.\n";

    #[test]
    fn lines_layout_strips_numbers_and_timings() {
        let out = to_plaintext(SRT, Options::default()).unwrap();
        assert_eq!(out, "First line.\nSecond line that wraps.");
    }

    #[test]
    fn blocks_layout_keeps_segmentation() {
        let opts = Options {
            layout: Layout::Blocks,
            ..Options::default()
        };
        let out = to_plaintext(SRT, opts).unwrap();
        assert_eq!(out, "First line.\n\nSecond line\nthat wraps.");
    }

    #[test]
    fn paragraph_layout_joins_everything() {
        let opts = Options {
            layout: Layout::Paragraph,
            ..Options::default()
        };
        let out = to_plaintext(SRT, opts).unwrap();
        assert_eq!(out, "First line. Second line that wraps.");
    }

    #[test]
    fn strips_html_tags_by_default() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\n<i>Italic</i> and <b>bold</b>.\n";
        let out = to_plaintext(srt, Options::default()).unwrap();
        assert_eq!(out, "Italic and bold.");
    }

    #[test]
    fn keeps_tags_when_disabled() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\n<i>Kept</i>\n";
        let opts = Options {
            strip_tags: false,
            ..Options::default()
        };
        let out = to_plaintext(srt, opts).unwrap();
        assert_eq!(out, "<i>Kept</i>");
    }

    #[test]
    fn keeps_less_than_when_no_closer() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\n2 < 3 always\n";
        let out = to_plaintext(srt, Options::default()).unwrap();
        assert_eq!(out, "2 < 3 always");
    }

    #[test]
    fn removes_sound_effects_when_enabled() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\n[door slams] Hello (softly)\n\n2\n00:00:03,000 --> 00:00:04,000\n\u{266a} la la \u{266a}\n";
        let opts = Options {
            remove_sound_effects: true,
            ..Options::default()
        };
        let out = to_plaintext(srt, opts).unwrap();
        // The music cue keeps its "la la" words; brackets/notes are stripped.
        assert_eq!(out, "Hello\nla la");
    }

    #[test]
    fn drops_cue_that_is_only_a_sound_effect() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\n[applause]\n\n2\n00:00:03,000 --> 00:00:04,000\nReal line.\n";
        let opts = Options {
            remove_sound_effects: true,
            ..Options::default()
        };
        let out = to_plaintext(srt, opts).unwrap();
        assert_eq!(out, "Real line.");
    }

    #[test]
    fn removes_speaker_labels_when_enabled() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nNARRATOR: It begins.\n\n2\n00:00:03,000 --> 00:00:04,000\n- JOHN: Hi there.\n";
        let opts = Options {
            remove_speaker_labels: true,
            ..Options::default()
        };
        let out = to_plaintext(srt, opts).unwrap();
        assert_eq!(out, "It begins.\nHi there.");
    }

    #[test]
    fn speaker_label_leaves_ordinary_colon_lines() {
        // A prefix over 30 chars must NOT be treated as a label.
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nthis is a very long clause indeed here: and more text\n";
        let opts = Options {
            remove_speaker_labels: true,
            ..Options::default()
        };
        let out = to_plaintext(srt, opts).unwrap();
        assert!(out.starts_with("this is a very long clause"), "{out}");
    }

    #[test]
    fn dedupe_collapses_rolling_captions() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nHello world\n\n2\n00:00:02,000 --> 00:00:03,000\nhello world\n\n3\n00:00:03,000 --> 00:00:04,000\nnext line\n";
        let opts = Options {
            dedupe: true,
            ..Options::default()
        };
        let out = to_plaintext(srt, opts).unwrap();
        assert_eq!(out, "Hello world\nnext line");
    }

    #[test]
    fn keeps_numeric_caption_that_is_not_an_index() {
        // "911" here is caption text (no timing line follows it), so it stays.
        let srt = "1\n00:00:01,000 --> 00:00:02,000\n911\n";
        let out = to_plaintext(srt, Options::default()).unwrap();
        assert_eq!(out, "911");
    }

    #[test]
    fn tolerates_webvtt_input() {
        let vtt =
            "WEBVTT\n\nNOTE some comment\n\n1\n00:00:01.000 --> 00:00:04.000 line:90%\nWeb line.\n";
        let out = to_plaintext(vtt, Options::default()).unwrap();
        assert_eq!(out, "Web line.");
    }

    #[test]
    fn rejects_empty_input() {
        let err = to_plaintext("   \n  ", Options::default()).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn rejects_non_subtitle_input() {
        let err = to_plaintext("just some notes\nno timings here", Options::default()).unwrap_err();
        assert!(err.contains("no subtitle timing line"), "{err}");
    }

    #[test]
    fn layout_parse_roundtrip() {
        assert_eq!(Layout::parse("").unwrap(), Layout::Lines);
        assert_eq!(Layout::parse("blocks").unwrap(), Layout::Blocks);
        assert_eq!(Layout::parse("paragraph").unwrap(), Layout::Paragraph);
        assert!(Layout::parse("sideways").is_err());
    }
}
