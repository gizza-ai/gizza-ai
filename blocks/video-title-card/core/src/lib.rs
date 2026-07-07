//! gizza-ai/video-title-card core — pure ffmpeg argv construction shared by the
//! chat skill block, the CLI, and the standalone web page. No wafer/wasm-bindgen
//! deps.
//!
//! Burns a styled title / lower-third caption onto a video with ffmpeg's
//! `drawtext` filter (libfreetype), gated to a `start`..`end` time window via
//! `enable='between(t,START,END)'`. The text is placed at one of seven anchor
//! presets and can sit on a semi-transparent background box (the classic
//! lower-third bar look).
//!
//! Two design choices keep this robust and injection-free across surfaces:
//!   * the overlay text is supplied to drawtext as a `textfile=` (written into
//!     ffmpeg's virtual FS by the caller), NOT inlined into the filter string —
//!     so apostrophes, colons, commas, `%`, `\` and quotes are drawn LITERALLY
//!     (`expansion=none`) and can never break or inject into the filtergraph;
//!   * the font is a bundled TTF ([`FONT_BYTES`]) written as `fontfile=` — the
//!     browser ffmpeg FS has no system fonts, and the native CLI ffmpeg writes
//!     the same file to its temp dir, so both surfaces render identically.
//!
//! Colors are validated through [`gizza_ai_block_utils::normalize_ffmpeg_color`]
//! (name table or `0xRRGGBB`) so the only user value that reaches the filter
//! string is a known-safe token. The output re-encodes video to H.264; the
//! input container is kept when it can hold H.264 + AAC (mp4/mov/m4v/mkv),
//! otherwise it switches to mp4 and the audio is re-encoded to AAC — see
//! `gizza_ai_block_utils::ffmpeg::h264_out_ext`.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;
use gizza_ai_block_utils::normalize_ffmpeg_color;

/// Liberation Sans Bold (SIL OFL 1.1, metric-compatible with Arial) — the
/// bundled title face. See `assets/LICENSE-Liberation.txt`.
pub const FONT_BYTES: &[u8] = include_bytes!("assets/LiberationSans-Bold.ttf");

/// Fixed virtual-FS filename the bundled font is written to (referenced as
/// `fontfile=` in the drawtext filter). The caller writes [`FONT_BYTES`] here.
pub const FONT_FILE: &str = "font.ttf";

/// Fixed virtual-FS filename the overlay text is written to (referenced as
/// `textfile=`). The caller writes the user's (validated, non-empty) text here.
pub const TEXT_FILE: &str = "title.txt";

/// Anchor presets → the drawtext `x` / `y` expressions. `text_w`/`text_h` are
/// the rendered text-box size, `w`/`h` the video size; a fixed [`MARGIN`] keeps
/// the text (and its background box) off the edges.
pub const POSITIONS: &[&str] = &[
    "top-left",
    "top-center",
    "top-right",
    "center",
    "bottom-left",
    "bottom-center",
    "bottom-right",
];

/// Pixels between the text box and the frame edge for the edge presets.
const MARGIN: u32 = 24;
/// Padding drawn around the text inside the background box.
const BOX_PAD: u32 = 14;

/// Default colors applied when the caller passes a blank value — the page's
/// color TEXT fields start empty (only the swatch is pre-filled), so blank must
/// mean "the default", matching the descriptor defaults and the chat/CLI
/// `unwrap_or` defaults.
pub const DEFAULT_FONT_COLOR: &str = "#ffffff";
pub const DEFAULT_BACKGROUND_COLOR: &str = "#000000";

/// Smallest / largest accepted font size, in pixels.
pub const MIN_FONT_SIZE: u32 = 8;
pub const MAX_FONT_SIZE: u32 = 400;
/// Longest accepted overlay text, in characters (a title/lower-third, not a
/// paragraph — a huge string would render off-frame and bloat the filter).
pub const MAX_TEXT_LEN: usize = 500;

fn position_list() -> String {
    POSITIONS.join("|")
}

/// Resolve an anchor preset to `(x_expr, y_expr)` drawtext position
/// expressions. Errors on an unknown preset.
pub fn position_xy(position: &str) -> Result<(String, String), String> {
    let m = MARGIN;
    let (x, y): (String, String) = match position.trim() {
        "top-left" => (format!("{m}"), format!("{m}")),
        "top-center" => ("(w-text_w)/2".to_string(), format!("{m}")),
        "top-right" => (format!("w-text_w-{m}"), format!("{m}")),
        "center" => ("(w-text_w)/2".to_string(), "(h-text_h)/2".to_string()),
        "bottom-left" => (format!("{m}"), format!("h-text_h-{m}")),
        "bottom-center" => ("(w-text_w)/2".to_string(), format!("h-text_h-{m}")),
        "bottom-right" => (format!("w-text_w-{m}"), format!("h-text_h-{m}")),
        other => {
            return Err(format!(
                "position {other:?} not supported (use one of {})",
                position_list()
            ))
        }
    };
    Ok((x, y))
}

/// Format a finite `f64` as a compact ffmpeg-friendly decimal: whole numbers
/// print without a fractional part (`5`), fractions round to 3 places with
/// trailing zeros trimmed (`0.5`, `1.25`). Used for the `enable` window and the
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

/// Validate all inputs and build the drawtext `-vf` filter string. The overlay
/// text itself is NOT part of this string (it is read from [`TEXT_FILE`]); the
/// `text` argument is validated (non-empty, length-capped) and its emptiness
/// gates the whole tool.
#[allow(clippy::too_many_arguments)]
pub fn build_filter(
    text: &str,
    position: &str,
    font_size: u32,
    font_color: &str,
    background: bool,
    background_color: &str,
    background_opacity: f64,
    start: f64,
    end: f64,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("text must not be empty — provide the title / lower-third caption".to_string());
    }
    if text.chars().count() > MAX_TEXT_LEN {
        return Err(format!(
            "text is too long ({} chars) — keep it under {MAX_TEXT_LEN} (a title, not a paragraph)",
            text.chars().count()
        ));
    }
    if !(MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&font_size) {
        return Err(format!(
            "font_size must be between {MIN_FONT_SIZE} and {MAX_FONT_SIZE} pixels, got {font_size}"
        ));
    }
    if !start.is_finite() || start < 0.0 {
        return Err(format!("start must be a time in seconds ≥ 0, got {start}"));
    }
    if !end.is_finite() || end <= start {
        return Err(format!(
            "end ({end}) must be a time in seconds greater than start ({start})"
        ));
    }
    // The page's color text fields start EMPTY (only the swatch is pre-filled),
    // so a blank value must mean "the default color", not an error — otherwise
    // running without touching the color fields fails. Matches video-aspect-pad.
    let font_color = if font_color.trim().is_empty() { DEFAULT_FONT_COLOR } else { font_color };
    let fc = normalize_ffmpeg_color(font_color)?;
    let (x, y) = position_xy(position)?;

    let mut df = format!(
        "drawtext=fontfile={FONT_FILE}:textfile={TEXT_FILE}:expansion=none:\
fontsize={font_size}:fontcolor={fc}:x={x}:y={y}"
    );
    if background {
        if !(0.0..=1.0).contains(&background_opacity) {
            return Err(format!(
                "background_opacity must be between 0 and 1, got {background_opacity}"
            ));
        }
        let background_color = if background_color.trim().is_empty() {
            DEFAULT_BACKGROUND_COLOR
        } else {
            background_color
        };
        let bg = normalize_ffmpeg_color(background_color)?;
        df.push_str(&format!(
            ":box=1:boxcolor={bg}@{}:boxborderw={BOX_PAD}",
            num(background_opacity)
        ));
    }
    df.push_str(&format!(":enable='between(t,{},{})'", num(start), num(end)));
    Ok(df)
}

/// Validate + build the full ffmpeg argv (no leading `ffmpeg`) and pick the
/// output filename. Returns `(argv, out_name)`. `out_name` keeps the input
/// container when it can hold H.264 + AAC, otherwise `out.mp4`.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    in_name: &str,
    text: &str,
    position: &str,
    font_size: u32,
    font_color: &str,
    background: bool,
    background_color: &str,
    background_opacity: f64,
    start: f64,
    end: f64,
) -> Result<(Vec<String>, String), String> {
    let filter = build_filter(
        text,
        position,
        font_size,
        font_color,
        background,
        background_color,
        background_opacity,
        start,
        end,
    )?;
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
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vf(argv: &[String]) -> String {
        let i = argv.iter().position(|a| a == "-vf").expect("-vf present");
        argv[i + 1].clone()
    }

    #[test]
    fn happy_path_builds_drawtext_with_textfile_font_box_and_enable() {
        let (argv, out) = plan(
            "in.mp4",
            "Breaking News",
            "bottom-center",
            48,
            "#ffffff",
            true,
            "#000000",
            0.5,
            0.0,
            5.0,
        )
        .unwrap();
        assert_eq!(out, "out.mp4");
        let f = vf(&argv);
        // text is a file, never inlined → no escaping needed, no injection.
        assert!(f.contains(&format!("textfile={TEXT_FILE}")), "{f}");
        assert!(f.contains(&format!("fontfile={FONT_FILE}")), "{f}");
        assert!(f.contains("expansion=none"), "{f}");
        assert!(f.contains("fontsize=48"), "{f}");
        assert!(f.contains("fontcolor=0xFFFFFF"), "{f}");
        assert!(f.contains("x=(w-text_w)/2"), "{f}");
        assert!(f.contains("y=h-text_h-24"), "{f}");
        assert!(f.contains("box=1:boxcolor=0x000000@0.5:boxborderw=14"), "{f}");
        assert!(f.contains("enable='between(t,0,5)'"), "{f}");
        // H.264 re-encode, audio copied for an mp4 input, faststart for mp4.
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "copy"));
        assert!(argv.iter().any(|a| a == "+faststart"));
    }

    #[test]
    fn no_background_omits_the_box() {
        let (argv, _) = plan(
            "in.mp4", "Hi", "top-left", 32, "white", false, "#000000", 0.5, 1.0, 2.5,
        )
        .unwrap();
        let f = vf(&argv);
        assert!(!f.contains("box=1"), "{f}");
        assert!(f.contains("fontcolor=white"), "{f}");
        assert!(f.contains("x=24"), "{f}");
        assert!(f.contains("y=24"), "{f}");
        assert!(f.contains("enable='between(t,1,2.5)'"), "{f}");
    }

    #[test]
    fn webm_input_switches_to_mp4_and_reencodes_audio() {
        let (argv, out) = plan(
            "in.webm",
            "Caption",
            "center",
            40,
            "yellow",
            true,
            "navy",
            0.8,
            0.0,
            3.0,
        )
        .unwrap();
        assert_eq!(out, "out.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
        let f = vf(&argv);
        assert!(f.contains("boxcolor=navy@0.8"), "{f}");
        assert!(f.contains("x=(w-text_w)/2") && f.contains("y=(h-text_h)/2"), "{f}");
    }

    #[test]
    fn mov_input_keeps_container() {
        let (_, out) = plan(
            "clip.mov", "T", "bottom-right", 20, "red", false, "black", 0.5, 0.0, 1.0,
        )
        .unwrap();
        assert_eq!(out, "out.mov");
    }

    #[test]
    fn rejects_empty_text() {
        let err = plan(
            "in.mp4", "   ", "center", 48, "white", true, "black", 0.5, 0.0, 5.0,
        )
        .unwrap_err();
        assert!(err.contains("text must not be empty"), "{err}");
    }

    #[test]
    fn rejects_bad_time_window_font_size_and_color() {
        assert!(plan("in.mp4", "x", "center", 48, "white", true, "black", 0.5, 5.0, 5.0)
            .unwrap_err()
            .contains("end"));
        assert!(plan("in.mp4", "x", "center", 48, "white", true, "black", 0.5, -1.0, 5.0)
            .unwrap_err()
            .contains("start"));
        assert!(plan("in.mp4", "x", "center", 4, "white", true, "black", 0.5, 0.0, 5.0)
            .unwrap_err()
            .contains("font_size"));
        // Exact cap boundary: 400 is accepted, 401 is one over → rejected.
        assert!(plan("in.mp4", "x", "center", 400, "white", true, "black", 0.5, 0.0, 5.0).is_ok());
        assert!(plan("in.mp4", "x", "center", 401, "white", true, "black", 0.5, 0.0, 5.0)
            .unwrap_err()
            .contains("font_size"));
        assert!(plan("in.mp4", "x", "center", 48, "notacolor", true, "black", 0.5, 0.0, 5.0)
            .is_err());
        assert!(plan("in.mp4", "x", "nowhere", 48, "white", true, "black", 0.5, 0.0, 5.0)
            .unwrap_err()
            .contains("position"));
        assert!(plan("in.mp4", "x", "center", 48, "white", true, "black", 1.5, 0.0, 5.0)
            .unwrap_err()
            .contains("opacity"));
    }

    #[test]
    fn blank_colors_fall_back_to_defaults() {
        // The page's color text fields start empty — blank must mean the
        // default color, not an error.
        let (argv, _) = plan("in.mp4", "Hi", "center", 48, "  ", true, "", 0.5, 0.0, 5.0).unwrap();
        let f = vf(&argv);
        assert!(f.contains("fontcolor=0xFFFFFF"), "{f}");
        assert!(f.contains("boxcolor=0x000000@0.5"), "{f}");
    }

    #[test]
    fn num_formats_compactly() {
        assert_eq!(num(5.0), "5");
        assert_eq!(num(0.5), "0.5");
        assert_eq!(num(1.25), "1.25");
        assert_eq!(num(0.0), "0");
        assert_eq!(num(2.500), "2.5");
    }

    #[test]
    fn every_position_preset_resolves() {
        for p in POSITIONS {
            let (x, y) = position_xy(p).unwrap();
            assert!(!x.is_empty() && !y.is_empty(), "{p}");
        }
    }

    #[test]
    fn font_bytes_are_bundled() {
        // A real TTF starts with the 0x00010000 sfnt version tag.
        assert!(FONT_BYTES.len() > 10_000);
        assert_eq!(&FONT_BYTES[0..4], &[0x00, 0x01, 0x00, 0x00]);
    }
}
