//! gizza-ai/video-caption-burner core — pure ffmpeg argv construction shared by
//! the chat skill block, the CLI, and the standalone web page. No wafer /
//! wasm-bindgen deps.
//!
//! Hard-burns ("opens") an SRT or WebVTT subtitle track onto a video so the
//! captions are baked into the pixels (the classic "open captions" look for
//! social clips that autoplay muted). Rather than depend on ffmpeg's
//! `subtitles`/libass filter — which the in-browser ffmpeg build does not ship
//! — the subtitle text is parsed here in pure Rust into timed cues, and each
//! cue becomes its own `drawtext` filter gated to its `start`..`end` window via
//! `enable='between(t,START,END)'`. The chain of drawtext filters is joined
//! into a single `-vf` string. This is the same freetype/`drawtext` path proven
//! by blocks/video-title-card, so it renders identically on the page and CLI.
//!
//! Two design choices keep this robust and injection-free across surfaces:
//!   * each cue's text is supplied to drawtext as a `textfile=` (written into
//!     ffmpeg's virtual FS by the caller), NOT inlined into the filter string —
//!     so apostrophes, colons, commas, `%`, `\`, quotes and newlines are drawn
//!     LITERALLY (`expansion=none`) and can never break or inject into the
//!     filtergraph;
//!   * the font is a bundled TTF ([`FONT_BYTES`]) written as `fontfile=` — the
//!     browser ffmpeg FS has no system fonts, and the native CLI ffmpeg writes
//!     the same file to its temp dir, so both surfaces render identically.
//!
//! Colors are validated through [`gizza_ai_block_utils::normalize_ffmpeg_color`]
//! (name table or `0xRRGGBB`) so the only user color that reaches the filter
//! string is a known-safe token. The output re-encodes video to H.264; the
//! input container is kept when it can hold H.264 + AAC (mp4/mov/m4v/mkv),
//! otherwise it switches to mp4 and the audio is re-encoded to AAC — see
//! `gizza_ai_block_utils::ffmpeg::h264_out_ext`.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;
use gizza_ai_block_utils::normalize_ffmpeg_color;

/// Liberation Sans Bold (SIL OFL 1.1, metric-compatible with Arial) — the
/// bundled caption face. See `assets/LICENSE-Liberation.txt`.
pub const FONT_BYTES: &[u8] = include_bytes!("assets/LiberationSans-Bold.ttf");

/// Fixed virtual-FS filename the bundled font is written to (referenced as
/// `fontfile=` in each drawtext filter). The caller writes [`FONT_BYTES`] here.
pub const FONT_FILE: &str = "font.ttf";

/// Vertical anchor presets → the drawtext `y` expression. `x` is always the
/// horizontal center, so captions read as a centered block.
pub const POSITIONS: &[&str] = &["bottom", "center", "top"];

/// Default colors applied when the caller passes a blank value — the page's
/// color TEXT fields start empty (only the swatch is pre-filled), so blank must
/// mean "the default", matching the descriptor defaults and the chat/CLI
/// `unwrap_or` defaults.
pub const DEFAULT_FONT_COLOR: &str = "#ffffff";
pub const DEFAULT_BACKGROUND_COLOR: &str = "#000000";

/// Smallest / largest accepted font size, in pixels.
pub const MIN_FONT_SIZE: u32 = 8;
pub const MAX_FONT_SIZE: u32 = 200;

/// Pixels between the caption box and the frame edge.
const MARGIN: u32 = 30;
/// Padding drawn around the text inside the background box.
const BOX_PAD: u32 = 10;

/// Longest accepted subtitle document, in bytes (a caption track, not a
/// transcript archive — a huge file would blow up the filtergraph).
pub const MAX_SUBTITLE_BYTES: usize = 200_000;
/// Most cues we will burn. Each cue is one drawtext filter; a few hundred is
/// plenty for a social clip and keeps the `-vf` string bounded.
pub const MAX_CUES: usize = 400;

/// One parsed subtitle cue: a `start`..`end` window (seconds) and the text to
/// show during it. `text` may contain newlines (multi-line cue).
#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// The virtual-FS filename a cue's text is written to (`cue0.txt`, `cue1.txt`,
/// …). Kept ASCII + index-only so it is always a safe drawtext `textfile=`.
pub fn cue_file(index: usize) -> String {
    format!("cue{index}.txt")
}

fn position_list() -> String {
    POSITIONS.join(", ")
}

/// Format a finite `f64` as a compact ffmpeg-friendly decimal: whole numbers
/// print without a fractional part (`5`), fractions round to 3 places with
/// trailing zeros trimmed (`0.5`, `1.25`). Used for the `enable` windows and the
/// box opacity so the filter string stays clean and deterministic.
fn num(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    let mut s = format!("{r:.3}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

/// Strip simple angle-bracket markup (`<i>`, `</i>`, `<b>`, `<font …>`, VTT
/// `<c.classname>`, `<00:00:01.000>` karaoke timestamps) from a cue line so the
/// visible caption is plain text. Anything between an unescaped `<` and the next
/// `>` is removed; a lone `<` with no closing `>` is kept verbatim.
fn strip_tags(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            // Consume up to and including the matching '>'. If there is no '>',
            // treat the '<' as a literal character.
            let mut tag = String::new();
            let mut closed = false;
            for t in chars.by_ref() {
                if t == '>' {
                    closed = true;
                    break;
                }
                tag.push(t);
            }
            if !closed {
                out.push('<');
                out.push_str(&tag);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parse a subtitle timestamp: `HH:MM:SS,mmm`, `HH:MM:SS.mmm`, `MM:SS.mmm`, or
/// with the milliseconds omitted. Both `,` (SRT) and `.` (VTT) decimal
/// separators are accepted. Returns seconds as `f64`.
fn parse_timestamp(raw: &str) -> Result<f64, String> {
    let t = raw.trim().replace(',', ".");
    if t.is_empty() {
        return Err("empty timestamp".to_string());
    }
    let parts: Vec<&str> = t.split(':').collect();
    let (h, m, s) = match parts.len() {
        3 => (parts[0], parts[1], parts[2]),
        2 => ("0", parts[0], parts[1]),
        1 => ("0", "0", parts[0]),
        _ => return Err(format!("bad timestamp {raw:?}")),
    };
    let h: f64 = h.trim().parse().map_err(|_| format!("bad hours in {raw:?}"))?;
    let m: f64 = m.trim().parse().map_err(|_| format!("bad minutes in {raw:?}"))?;
    let s: f64 = s.trim().parse().map_err(|_| format!("bad seconds in {raw:?}"))?;
    let total = h * 3600.0 + m * 60.0 + s;
    if !total.is_finite() || total < 0.0 {
        return Err(format!("bad timestamp {raw:?}"));
    }
    Ok(total)
}

/// Parse the cue-timing line — `START --> END [cue settings]` — into
/// `(start, end)` seconds. Trailing WebVTT cue settings after the end timestamp
/// (e.g. `align:middle position:50%`) are ignored.
fn parse_time_line(line: &str) -> Result<(f64, f64), String> {
    let (left, right) = line
        .split_once("-->")
        .ok_or_else(|| format!("cue timing line missing '-->': {line:?}"))?;
    let start = parse_timestamp(left)?;
    // Only the first whitespace-delimited token on the right is the timestamp;
    // the rest are VTT cue settings.
    let end_tok = right
        .trim()
        .split_whitespace()
        .next()
        .ok_or_else(|| format!("cue timing line missing end time: {line:?}"))?;
    let end = parse_timestamp(end_tok)?;
    Ok((start, end))
}

/// Parse an SRT or WebVTT document into timed cues. Robust to CRLF line
/// endings, the `WEBVTT` header, `NOTE` blocks, SRT index lines, blank-line cue
/// separators, and inline `<...>` markup. Cues with no visible text or a
/// non-positive duration are skipped. Errors on an empty document, one with no
/// recognizable cue, or one exceeding [`MAX_SUBTITLE_BYTES`] / [`MAX_CUES`].
pub fn parse_subtitles(raw: &str) -> Result<Vec<Cue>, String> {
    if raw.trim().is_empty() {
        return Err(
            "subtitles must not be empty — paste an SRT or WebVTT caption track".to_string(),
        );
    }
    if raw.len() > MAX_SUBTITLE_BYTES {
        return Err(format!(
            "subtitles are too large ({} bytes) — keep the track under {MAX_SUBTITLE_BYTES} bytes",
            raw.len()
        ));
    }
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut cues: Vec<Cue> = Vec::new();
    for block in normalized.split("\n\n") {
        if block.trim().is_empty() {
            continue;
        }
        let lines: Vec<&str> = block.lines().collect();
        // Find the timing line inside the block; the SRT index line (and the
        // VTT `WEBVTT`/`NOTE` headers) sit before it or in a block of their own.
        let Some(ti) = lines.iter().position(|l| l.contains("-->")) else {
            continue;
        };
        let (start, end) = parse_time_line(lines[ti])?;
        let text = lines[ti + 1..]
            .iter()
            .map(|l| strip_tags(l))
            .collect::<Vec<_>>()
            .join("\n");
        let text = text.trim_matches('\n').to_string();
        if text.trim().is_empty() {
            continue;
        }
        // Skip zero/negative-duration cues — ffmpeg's between() would never
        // enable them and a degenerate window is almost always a parse artefact.
        if end <= start {
            continue;
        }
        cues.push(Cue { start, end, text });
        if cues.len() > MAX_CUES {
            return Err(format!(
                "too many cues (>{MAX_CUES}) — split the track into shorter segments"
            ));
        }
    }
    if cues.is_empty() {
        return Err(
            "no subtitle cues found — expected SRT or WebVTT with `HH:MM:SS,mmm --> HH:MM:SS,mmm` timing lines"
                .to_string(),
        );
    }
    Ok(cues)
}

/// Resolve a vertical anchor preset to the drawtext `y` expression. `text_h` is
/// the rendered (possibly multi-line) text-box height; `h` the video height.
fn position_y(position: &str) -> Result<String, String> {
    let m = MARGIN;
    match position.trim() {
        "bottom" => Ok(format!("h-text_h-{m}")),
        "center" => Ok("(h-text_h)/2".to_string()),
        "top" => Ok(format!("{m}")),
        other => Err(format!(
            "position {other:?} not supported (use one of {})",
            position_list()
        )),
    }
}

/// Build one cue's `drawtext` filter fragment. The cue text is NOT part of the
/// string (it is read from `textfile=cueN.txt`); only known-safe tokens (font
/// size, normalized color, position expr, numeric window) are interpolated.
fn cue_filter(
    index: usize,
    cue: &Cue,
    y: &str,
    font_size: u32,
    font_color: &str,
    background: bool,
    background_color: &str,
    background_opacity: f64,
) -> String {
    let file = cue_file(index);
    let mut df = format!(
        "drawtext=fontfile={FONT_FILE}:textfile={file}:expansion=none:\
fontsize={font_size}:fontcolor={font_color}:x=(w-text_w)/2:y={y}"
    );
    if background {
        df.push_str(&format!(
            ":box=1:boxcolor={background_color}@{}:boxborderw={BOX_PAD}",
            num(background_opacity)
        ));
    }
    df.push_str(&format!(
        ":enable='between(t,{},{})'",
        num(cue.start),
        num(cue.end)
    ));
    df
}

/// Validate all inputs, parse the subtitle track, and build the full ffmpeg
/// argv (no leading `ffmpeg`) plus the per-cue textfiles the drawtext chain
/// references. Returns `(argv, out_name, files)` where `files` is
/// `(virtual_fs_name, text)` for each cue — the caller writes these (and the
/// font) into ffmpeg's FS. `out_name` keeps the input container when it can
/// hold H.264 + AAC, otherwise `out.mp4`.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    in_name: &str,
    subtitles: &str,
    position: &str,
    font_size: u32,
    font_color: &str,
    background: bool,
    background_color: &str,
    background_opacity: f64,
) -> Result<(Vec<String>, String, Vec<(String, String)>), String> {
    if !(MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&font_size) {
        return Err(format!(
            "font_size must be between {MIN_FONT_SIZE} and {MAX_FONT_SIZE} pixels, got {font_size}"
        ));
    }
    if background && !(0.0..=1.0).contains(&background_opacity) {
        return Err(format!(
            "background_opacity must be between 0 and 1, got {background_opacity}"
        ));
    }
    // The page's color text fields start EMPTY (only the swatch is pre-filled),
    // so a blank value must mean "the default color", not an error.
    let font_color = if font_color.trim().is_empty() { DEFAULT_FONT_COLOR } else { font_color };
    let fc = normalize_ffmpeg_color(font_color)?;
    let background_color = if background_color.trim().is_empty() {
        DEFAULT_BACKGROUND_COLOR
    } else {
        background_color
    };
    let bg = normalize_ffmpeg_color(background_color)?;
    let y = position_y(position)?;

    let cues = parse_subtitles(subtitles)?;

    let mut files: Vec<(String, String)> = Vec::with_capacity(cues.len());
    let filter = cues
        .iter()
        .enumerate()
        .map(|(i, cue)| {
            files.push((cue_file(i), cue.text.clone()));
            cue_filter(i, cue, &y, font_size, &fc, background, &bg, background_opacity)
        })
        .collect::<Vec<_>>()
        .join(",");

    let (out_ext, transcode_audio) = h264_out_ext(in_name);
    let out_name = format!("out.{out_ext}");

    let mut argv: Vec<String> = vec![
        "-i".into(),
        in_name.into(),
        "-vf".into(),
        filter,
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "medium".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        if transcode_audio { "aac".into() } else { "copy".into() },
    ];
    if out_name.ends_with(".mp4") || out_name.ends_with(".mov") {
        argv.push("-movflags".into());
        argv.push("+faststart".into());
    }
    argv.push(out_name.clone());
    Ok((argv, out_name, files))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vf(argv: &[String]) -> String {
        let i = argv.iter().position(|a| a == "-vf").expect("-vf present");
        argv[i + 1].clone()
    }

    const SRT: &str = "1\n\
00:00:01,000 --> 00:00:04,000\n\
Hello, world!\n\
\n\
2\n\
00:00:05,500 --> 00:00:08,000\n\
Second line: it's fine\n";

    #[test]
    fn parses_srt_into_cues() {
        let cues = parse_subtitles(SRT).unwrap();
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0], Cue { start: 1.0, end: 4.0, text: "Hello, world!".into() });
        assert_eq!(cues[1].start, 5.5);
        assert_eq!(cues[1].end, 8.0);
        assert_eq!(cues[1].text, "Second line: it's fine");
    }

    #[test]
    fn parses_webvtt_with_header_note_and_tags() {
        let vtt = "WEBVTT\n\
\n\
NOTE this is a comment block\n\
\n\
00:00.000 --> 00:02.000 align:middle position:50%\n\
<c.yellow>Styled</c> <i>caption</i>\n\
\n\
00:00:03.000 --> 00:00:05.000\n\
Line one\n\
Line two\n";
        let cues = parse_subtitles(vtt).unwrap();
        assert_eq!(cues.len(), 2);
        // MM:SS.mmm form (no hours) and inline tags stripped.
        assert_eq!(cues[0], Cue { start: 0.0, end: 2.0, text: "Styled caption".into() });
        // Multi-line cue kept as a newline-joined block.
        assert_eq!(cues[1].text, "Line one\nLine two");
    }

    #[test]
    fn happy_path_builds_one_drawtext_per_cue_with_textfiles() {
        let (argv, out, files) =
            plan("in.mp4", SRT, "bottom", 24, "#ffffff", true, "#000000", 0.5).unwrap();
        assert_eq!(out, "out.mp4");
        // One textfile per cue, carrying the literal (un-escaped) text.
        assert_eq!(files.len(), 2);
        assert_eq!(files[0], ("cue0.txt".to_string(), "Hello, world!".to_string()));
        assert_eq!(files[1].0, "cue1.txt");
        let f = vf(&argv);
        assert_eq!(f.matches("drawtext=").count(), 2, "{f}");
        assert!(f.contains("textfile=cue0.txt") && f.contains("textfile=cue1.txt"), "{f}");
        assert!(f.contains(&format!("fontfile={FONT_FILE}")), "{f}");
        assert!(f.contains("expansion=none"), "{f}");
        assert!(f.contains("fontsize=24"), "{f}");
        assert!(f.contains("fontcolor=0xFFFFFF"), "{f}");
        assert!(f.contains("x=(w-text_w)/2:y=h-text_h-30"), "{f}");
        assert!(f.contains("box=1:boxcolor=0x000000@0.5:boxborderw=10"), "{f}");
        assert!(f.contains("enable='between(t,1,4)'"), "{f}");
        assert!(f.contains("enable='between(t,5.5,8)'"), "{f}");
        // H.264 re-encode, audio copied for an mp4 input, faststart for mp4.
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "copy"));
        assert!(argv.iter().any(|a| a == "+faststart"));
    }

    #[test]
    fn no_background_omits_the_box() {
        let (argv, _, _) =
            plan("in.mp4", SRT, "top", 32, "white", false, "#000000", 0.5).unwrap();
        let f = vf(&argv);
        assert!(!f.contains("box=1"), "{f}");
        assert!(f.contains("fontcolor=white"), "{f}");
        assert!(f.contains(":y=30"), "{f}");
    }

    #[test]
    fn center_position_and_webm_switches_to_mp4_and_reencodes_audio() {
        let (argv, out, _) =
            plan("in.webm", SRT, "center", 40, "yellow", true, "navy", 0.8).unwrap();
        assert_eq!(out, "out.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
        let f = vf(&argv);
        assert!(f.contains("boxcolor=navy@0.8"), "{f}");
        assert!(f.contains("y=(h-text_h)/2"), "{f}");
    }

    #[test]
    fn mov_input_keeps_container() {
        let (_, out, _) =
            plan("clip.mov", SRT, "bottom", 24, "red", false, "black", 0.5).unwrap();
        assert_eq!(out, "out.mov");
    }

    #[test]
    fn rejects_empty_and_cueless_subtitles() {
        assert!(plan("in.mp4", "   ", "bottom", 24, "white", true, "black", 0.5)
            .unwrap_err()
            .contains("must not be empty"));
        assert!(plan("in.mp4", "just some prose\nwith no timings", "bottom", 24, "white", true, "black", 0.5)
            .unwrap_err()
            .contains("no subtitle cues"));
    }

    #[test]
    fn rejects_bad_font_size_position_color_and_opacity() {
        assert!(plan("in.mp4", SRT, "bottom", 4, "white", true, "black", 0.5)
            .unwrap_err()
            .contains("font_size"));
        // Exact cap boundary: 200 accepted, 201 rejected.
        assert!(plan("in.mp4", SRT, "bottom", 200, "white", true, "black", 0.5).is_ok());
        assert!(plan("in.mp4", SRT, "bottom", 201, "white", true, "black", 0.5)
            .unwrap_err()
            .contains("font_size"));
        assert!(plan("in.mp4", SRT, "nowhere", 24, "white", true, "black", 0.5)
            .unwrap_err()
            .contains("position"));
        assert!(plan("in.mp4", SRT, "bottom", 24, "notacolor", true, "black", 0.5).is_err());
        assert!(plan("in.mp4", SRT, "bottom", 24, "white", true, "black", 1.5)
            .unwrap_err()
            .contains("opacity"));
    }

    #[test]
    fn blank_colors_fall_back_to_defaults() {
        let (argv, _, _) =
            plan("in.mp4", SRT, "bottom", 24, "  ", true, "", 0.5).unwrap();
        let f = vf(&argv);
        assert!(f.contains("fontcolor=0xFFFFFF"), "{f}");
        assert!(f.contains("boxcolor=0x000000@0.5"), "{f}");
    }

    #[test]
    fn skips_degenerate_and_empty_cues() {
        // A zero-duration cue and a whitespace-only cue are dropped.
        let s = "1\n00:00:01,000 --> 00:00:01,000\nzero width\n\n\
2\n00:00:02,000 --> 00:00:03,000\n   \n\n\
3\n00:00:04,000 --> 00:00:06,000\nreal\n";
        let cues = parse_subtitles(s).unwrap();
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "real");
    }

    #[test]
    fn strip_tags_removes_markup_but_keeps_lone_lt() {
        assert_eq!(strip_tags("<i>hi</i>"), "hi");
        assert_eq!(strip_tags("a<font color=red>b</font>c"), "abc");
        assert_eq!(strip_tags("5 < 10 apples"), "5 < 10 apples");
    }

    #[test]
    fn num_formats_compactly() {
        assert_eq!(num(5.0), "5");
        assert_eq!(num(0.5), "0.5");
        assert_eq!(num(1.25), "1.25");
        assert_eq!(num(0.0), "0");
    }

    #[test]
    fn font_bytes_are_bundled() {
        // A real TTF starts with the 0x00010000 sfnt version tag.
        assert!(FONT_BYTES.len() > 10_000);
        assert_eq!(&FONT_BYTES[0..4], &[0x00, 0x01, 0x00, 0x00]);
    }
}
