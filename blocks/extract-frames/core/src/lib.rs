//! gizza-ai/extract-frames core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Samples frames from a video at a fixed **interval** (one frame every N
//! seconds), a fixed **fps** (N frames per second), or at **scene**-change
//! points (frames whose visual difference from the previous one exceeds a
//! threshold), then tiles the sampled frames into a single **contact-sheet**
//! image (`columns` × `rows` grid). Because the page renders one output file, a
//! multi-frame batch is delivered as one grid image rather than a folder/zip of
//! separate PNGs.
//!
//! The sheet holds up to `columns` × `rows` thumbnails (the first grid's worth of
//! sampled frames); raise the interval / grid to cover a longer clip. Scene mode
//! always includes the opening frame (`n == 0`) so a cut-free clip still yields a
//! usable sheet instead of an empty output.

/// Selection strategies for which frames land on the sheet.
pub const MODES: &[&str] = &["interval", "fps", "scene"];

/// Grid bounds — a sheet is `columns` × `rows` thumbnails.
pub const MIN_GRID: u32 = 1;
pub const MAX_GRID: u32 = 8;

/// Per-thumbnail width bounds, in pixels (height follows the source aspect).
pub const MIN_WIDTH: u32 = 16;
pub const MAX_WIDTH: u32 = 800;

/// Largest accepted interval (seconds) / fps, a sanity guard against absurd
/// filter strings; scene thresholds are bounded separately to `(0, 1]`.
pub const MAX_RATE: f64 = 3600.0;

/// Gap (px) around the whole sheet and between thumbnails — baked, not exposed,
/// so the output dimensions stay predictable (`margin`/`padding` in `tile`).
const MARGIN: u32 = 4;
const PADDING: u32 = 2;

/// Output image formats. PNG is lossless (crisp text/UI frames); JPG is smaller
/// for photographic content.
pub const FORMATS: &[&str] = &["png", "jpg"];

/// Format an `f64` without a trailing `.0` for whole numbers (so `2.0` → `2`),
/// keeping the ffmpeg filter string and unit tests tidy.
fn trim_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Normalize the user-facing background/padding color into a form ffmpeg accepts
/// verbatim inside a filter option: a lower-cased CSS/X11 color **name**
/// (letters only), or `#RGB` / `#RRGGBB` / bare 6-digit hex → `0xRRGGBB`. Empty
/// means the default white. The strict charset (letters, or `#` + hex) is what
/// keeps the filtergraph string injection-free — a name is never trusted to
/// contain a comma, quote, or bracket that could break out of the `color=`
/// option (an unknown-but-well-formed name simply errors inside ffmpeg).
pub fn normalize_color(color: &str) -> Result<String, String> {
    let t = color.trim();
    if t.is_empty() {
        return Ok("white".to_string());
    }
    let hex = t.strip_prefix('#').unwrap_or(t);
    // #RRGGBB or bare RRGGBB (6 hex digits) → 0xRRGGBB.
    if hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Ok(format!("0x{}", hex.to_ascii_uppercase()));
    }
    // #RGB shorthand → expand to 0xRRGGBB (only when explicitly '#'-prefixed, so
    // a 3-letter word like "red" isn't mistaken for hex).
    if t.starts_with('#') && hex.len() == 3 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        let full: String = hex.chars().flat_map(|c| [c, c]).collect();
        return Ok(format!("0x{}", full.to_ascii_uppercase()));
    }
    // A plain color name: letters only (ffmpeg's own color table validates it).
    if t.bytes().all(|b| b.is_ascii_alphabetic()) {
        return Ok(t.to_ascii_lowercase());
    }
    Err(format!(
        "background {t:?} is not a color name (letters only, e.g. white, black, navy) or hex (#RGB / #RRGGBB)"
    ))
}

/// Map the requested output format to its file extension. Unknown → error.
pub fn format_ext(format: &str) -> Result<&'static str, String> {
    match format.trim().to_ascii_lowercase().as_str() {
        "png" => Ok("png"),
        "jpg" | "jpeg" => Ok("jpg"),
        other => Err(format!(
            "format {other:?} not supported (want one of {})",
            FORMATS.join(", ")
        )),
    }
}

/// Build the frame-selection stage of the filtergraph for a mode + value.
///
/// - `interval`: `fps=1/<value>` — one frame every `value` seconds.
/// - `fps`: `fps=<value>` — `value` frames sampled per second.
/// - `scene`: `select=eq(n\,0)+gt(scene\,<value>)` — the opening frame plus any
///   frame whose scene-change score exceeds `value` (0–1). The `\,` escapes the
///   commas so ffmpeg parses `gt(...)`/`eq(...)` as expressions, not as filter
///   separators (the whole graph is a single argv token, no shell).
fn select_stage(mode: &str, value: f64) -> Result<String, String> {
    match mode {
        "interval" => {
            if !(value.is_finite() && value > 0.0 && value <= MAX_RATE) {
                return Err(format!(
                    "interval must be > 0 and <= {} seconds, got {value}",
                    trim_num(MAX_RATE)
                ));
            }
            Ok(format!("fps=1/{}", trim_num(value)))
        }
        "fps" => {
            if !(value.is_finite() && value > 0.0 && value <= MAX_RATE) {
                return Err(format!(
                    "fps must be > 0 and <= {}, got {value}",
                    trim_num(MAX_RATE)
                ));
            }
            Ok(format!("fps={}", trim_num(value)))
        }
        "scene" => {
            if !(value.is_finite() && value > 0.0 && value <= 1.0) {
                return Err(format!(
                    "scene threshold must be > 0 and <= 1 (typical 0.2–0.4), got {value}"
                ));
            }
            Ok(format!("select=eq(n\\,0)+gt(scene\\,{})", trim_num(value)))
        }
        other => Err(format!(
            "mode {other:?} not supported (want one of {})",
            MODES.join(", ")
        )),
    }
}

/// Build the full `-vf` filter string: frame selection → scale each thumbnail to
/// `width` (height auto, source aspect preserved) → `tile` into a
/// `columns`×`rows` grid with a colored `margin`/`padding` gap.
fn build_filter(
    mode: &str,
    value: f64,
    columns: u32,
    rows: u32,
    width: u32,
    color: &str,
) -> Result<String, String> {
    let select = select_stage(mode, value)?;
    Ok(format!(
        "{select},scale={width}:-1,tile={columns}x{rows}:margin={MARGIN}:padding={PADDING}:color={color}"
    ))
}

/// Validate all params and return `(argv, out_name)`. `argv` has NO leading
/// `ffmpeg`; `out_name` is `contact-sheet.<png|jpg>`.
///
/// - `mode` ∈ {interval, fps, scene}; `value`'s meaning is mode-dependent
///   (seconds / frames-per-second / scene threshold 0–1).
/// - `columns`, `rows` ∈ [1, 8] (up to 64 thumbnails).
/// - `width` ∈ [16, 800] px per thumbnail.
/// - `background` a color name or hex for the grid gaps.
/// - `format` ∈ {png, jpg}.
///
/// `-frames:v 1 -update 1` writes exactly one image (the first full sheet).
#[allow(clippy::too_many_arguments)]
pub fn plan(
    in_name: &str,
    mode: &str,
    value: f64,
    columns: u32,
    rows: u32,
    width: u32,
    background: &str,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    if !(MIN_GRID..=MAX_GRID).contains(&columns) {
        return Err(format!(
            "columns must be between {MIN_GRID} and {MAX_GRID}, got {columns}"
        ));
    }
    if !(MIN_GRID..=MAX_GRID).contains(&rows) {
        return Err(format!(
            "rows must be between {MIN_GRID} and {MAX_GRID}, got {rows}"
        ));
    }
    if !(MIN_WIDTH..=MAX_WIDTH).contains(&width) {
        return Err(format!(
            "thumbnail width must be between {MIN_WIDTH} and {MAX_WIDTH} pixels, got {width}"
        ));
    }
    let color = normalize_color(background)?;
    let ext = format_ext(format)?;
    let filter = build_filter(mode, value, columns, rows, width, &color)?;
    let out_name = format!("contact-sheet.{ext}");
    let argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vf".to_string(),
        filter,
        "-frames:v".to_string(),
        "1".to_string(),
        "-update".to_string(),
        "1".to_string(),
        "-y".to_string(),
        out_name.clone(),
    ];
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vf(argv: &[String]) -> String {
        let i = argv.iter().position(|a| a == "-vf").unwrap();
        argv[i + 1].clone()
    }

    #[test]
    fn interval_mode_builds_reciprocal_fps_and_tile() {
        let (argv, out) = plan("in.mp4", "interval", 2.0, 4, 3, 240, "white", "png").unwrap();
        assert_eq!(out, "contact-sheet.png");
        let f = vf(&argv);
        assert!(f.starts_with("fps=1/2,"), "got {f}");
        assert!(f.contains("scale=240:-1"));
        assert!(f.contains("tile=4x3:margin=4:padding=2:color=white"), "got {f}");
        // input is seek-less (whole video sampled) and exactly one image out.
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "in.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-frames:v" && w[1] == "1"));
        assert!(argv.windows(2).any(|w| w[0] == "-update" && w[1] == "1"));
        assert_eq!(argv.last().map(String::as_str), Some("contact-sheet.png"));
    }

    #[test]
    fn fps_mode_passes_rate_through() {
        let (argv, _) = plan("in.webm", "fps", 1.0, 2, 2, 160, "black", "png").unwrap();
        assert!(vf(&argv).starts_with("fps=1,"));
    }

    #[test]
    fn scene_mode_includes_first_frame_and_escapes_commas() {
        let (argv, _) = plan("clip.mov", "scene", 0.3, 4, 3, 200, "white", "png").unwrap();
        let f = vf(&argv);
        // Commas inside the expression are backslash-escaped, and frame 0 is
        // always kept so a cut-free clip still yields a sheet.
        assert!(f.starts_with(r"select=eq(n\,0)+gt(scene\,0.3),"), "got {f}");
    }

    #[test]
    fn jpg_format_changes_extension() {
        let (_, out) = plan("in.mp4", "fps", 2.0, 4, 3, 240, "white", "jpg").unwrap();
        assert_eq!(out, "contact-sheet.jpg");
    }

    #[test]
    fn hex_background_becomes_ffmpeg_hex() {
        let (argv, _) = plan("in.mp4", "interval", 5.0, 4, 3, 240, "#1A2B3C", "png").unwrap();
        assert!(vf(&argv).contains("color=0x1A2B3C"), "{}", vf(&argv));
        // short hex expands
        assert_eq!(normalize_color("#f0a").unwrap(), "0xFF00AA");
        // bare 6-digit hex accepted
        assert_eq!(normalize_color("112233").unwrap(), "0x112233");
        // name lower-cased
        assert_eq!(normalize_color("Navy").unwrap(), "navy");
        // empty → default
        assert_eq!(normalize_color("").unwrap(), "white");
    }

    #[test]
    fn rejects_injection_and_out_of_range() {
        // filtergraph-injection attempts are rejected by the color charset.
        assert!(normalize_color("white,crop=1:1").is_err());
        assert!(normalize_color("red'").is_err());
        // grid + width bounds enforced.
        assert!(plan("in.mp4", "fps", 2.0, 0, 3, 240, "white", "png").is_err());
        assert!(plan("in.mp4", "fps", 2.0, 9, 3, 240, "white", "png").is_err());
        assert!(plan("in.mp4", "fps", 2.0, 4, 3, 8, "white", "png").is_err());
        assert!(plan("in.mp4", "fps", 2.0, 4, 3, 801, "white", "png").is_err());
    }

    #[test]
    fn rejects_bad_mode_and_values() {
        assert!(plan("in.mp4", "keyframe", 2.0, 4, 3, 240, "white", "png").is_err());
        // interval/fps must be > 0
        assert!(plan("in.mp4", "interval", 0.0, 4, 3, 240, "white", "png").is_err());
        assert!(plan("in.mp4", "fps", -1.0, 4, 3, 240, "white", "png").is_err());
        // scene threshold must be in (0, 1]
        assert!(plan("in.mp4", "scene", 0.0, 4, 3, 240, "white", "png").is_err());
        assert!(plan("in.mp4", "scene", 1.5, 4, 3, 240, "white", "png").is_err());
        // bad format
        assert!(plan("in.mp4", "fps", 2.0, 4, 3, 240, "white", "gif").is_err());
    }
}
