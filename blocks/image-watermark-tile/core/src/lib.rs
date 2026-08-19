//! gizza-ai/image-watermark-tile core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Stamps a repeating (tiled) text watermark over the WHOLE image, the way stock
//! agencies mark preview files: an anti-theft pattern that can't be cropped off.
//!
//! How the filtergraph works (the non-obvious parts are load-bearing):
//!
//! 1. `split` the source into the base frame and a second copy that
//!    `colorchannelmixer=rr=0:gg=0:bb=0:aa=0` turns into a fully transparent
//!    layer of exactly the same size — that is how a watermark layer is created
//!    without knowing the image dimensions at argv-build time.
//! 2. Every tile is one `drawtext` positioned RELATIVELY (`x=w*0.125-text_w/2`),
//!    so the same plan tiles a 400×300 avatar and a 6000×4000 photo identically.
//!    Text comes from `textfile=` and the font from `fontfile=`, so user text is
//!    never interpolated into the filtergraph (no escaping bugs, no injection).
//! 3. Text is drawn FULLY OPAQUE and the whole layer's alpha is scaled once at
//!    the end (`colorchannelmixer=aa=<opacity>`). Drawing at `fontcolor=white@0.3`
//!    instead would blend the glyphs against the layer's transparent BLACK first
//!    and then again during overlay — measured output was ~25% white muddied
//!    toward black instead of the requested 30%. Opaque-then-scale composites
//!    exactly (white@0.5 over pure blue measures (128,128,255)).
//! 4. For a non-zero `angle` the layer is first `pad`ded to 1.5× (≥ √2, so a
//!    rotation by any angle still covers every corner of the frame), the grid is
//!    generated across that larger canvas at the SAME visible density, `rotate`
//!    spins it with a transparent fill, and `overlay` re-centers it with negative
//!    offsets so the excess is clipped. Without the pad, rotation would leave
//!    bare triangles in the corners — exactly where a thief would crop.
//!
//! Everything is deterministic, so chat, CLI and the page produce identical
//! bytes, and every filter string is a single space-free argv token.

use gizza_ai_block_utils::normalize_ffmpeg_color;

/// Bundled font: user machines (and the browser's empty ffmpeg FS) have none.
pub const FONT_BYTES: &[u8] = include_bytes!("assets/LiberationSans-Bold.ttf");
/// Virtual-FS name the filtergraph references with `fontfile=`.
pub const FONT_FILE: &str = "font.ttf";
/// Virtual-FS name the filtergraph references with `textfile=`.
pub const TEXT_FILE: &str = "watermark.txt";

/// Tile layouts, in display order. KEEP IN SYNC with [`parse_pattern`].
pub const PATTERNS: [&str; 2] = ["grid", "brick"];
/// The default tile layout (offset rows read as less machine-made).
pub const DEFAULT_PATTERN: &str = "brick";

/// Output containers, in display order. KEEP IN SYNC with [`parse_format`].
pub const FORMATS: [&str; 4] = ["keep", "png", "jpg", "webp"];
/// The default output container (keep the input's).
pub const DEFAULT_FORMAT: &str = "keep";

pub const DEFAULT_FONT_SIZE: f64 = 32.0;
pub const DEFAULT_COLOR: &str = "#ffffff";
pub const DEFAULT_OPACITY: f64 = 0.3;
pub const DEFAULT_ANGLE: f64 = 30.0;
pub const DEFAULT_COLUMNS: u32 = 4;
pub const DEFAULT_ROWS: u32 = 5;

/// Longest accepted watermark text. Long strings make each tile wider than the
/// image, which reads as one smeared line rather than a tiled pattern.
pub const MAX_TEXT_LEN: usize = 120;

pub const MIN_FONT_SIZE: f64 = 6.0;
pub const MAX_FONT_SIZE: f64 = 400.0;
pub const MIN_OPACITY: f64 = 0.02;
pub const MAX_OPACITY: f64 = 1.0;
pub const MAX_ANGLE: f64 = 90.0;
pub const MAX_TILES: u32 = 12;

/// Layer over-provisioning factor used when rotating. Must be ≥ √2 ≈ 1.4142 so
/// that a 45° rotation still covers the frame's corners.
const PAD: f64 = 1.5;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Pattern {
    /// Every row aligned — a plain rectangular lattice.
    Grid,
    /// Alternate rows offset by half a cell (brick / checker-wise).
    Brick,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OutFormat {
    Keep,
    Png,
    Jpg,
    Webp,
}

pub fn parse_pattern(s: Option<&str>) -> Result<Pattern, String> {
    match s.map(str::trim).unwrap_or("").to_ascii_lowercase().as_str() {
        "" => Ok(Pattern::Brick),
        "grid" => Ok(Pattern::Grid),
        "brick" | "diagonal" | "checker" => Ok(Pattern::Brick),
        other => Err(format!(
            "pattern {other:?} not supported — use grid or brick"
        )),
    }
}

pub fn pattern_name(p: Pattern) -> &'static str {
    match p {
        Pattern::Grid => "grid",
        Pattern::Brick => "brick",
    }
}

pub fn parse_format(s: Option<&str>) -> Result<OutFormat, String> {
    match s.map(str::trim).unwrap_or("").to_ascii_lowercase().as_str() {
        "" | "keep" | "same" => Ok(OutFormat::Keep),
        "png" => Ok(OutFormat::Png),
        "jpg" | "jpeg" => Ok(OutFormat::Jpg),
        "webp" => Ok(OutFormat::Webp),
        other => Err(format!(
            "format {other:?} not supported — use keep, png, jpg or webp"
        )),
    }
}

/// Trim a float to at most 5 decimals with no trailing zeros, so filter strings
/// stay short and byte-identical across surfaces.
fn num(v: f64) -> String {
    let r = (v * 100_000.0).round() / 100_000.0;
    let mut s = format!("{r:.5}");
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

fn bounded_u32(v: f64, default: u32, min: u32, max: u32, name: &str) -> Result<u32, String> {
    let n = if v.is_finite() && v > 0.0 {
        v.round() as i64
    } else if v == 0.0 {
        0
    } else {
        default as i64
    };
    if n < min as i64 || n > max as i64 {
        return Err(format!("{name} must be between {min} and {max}, got {n}"));
    }
    Ok(n as u32)
}

/// The relative (0..1) centre of every tile, in draw order.
///
/// `spread` is how many cells span the canvas being drawn on: for the rotated
/// path that canvas is `PAD`× the frame, so the count grows but the on-screen
/// cell size — and therefore the user-visible density — stays exactly
/// `columns` × `rows`.
fn tile_centers(columns: u32, rows: u32, pattern: Pattern, padded: bool) -> Vec<(f64, f64)> {
    let scale = if padded { PAD } else { 1.0 };
    let n_cols = ((columns as f64 * scale).round() as u32).max(1);
    let n_rows = ((rows as f64 * scale).round() as u32).max(1);
    let mut out = Vec::new();
    for j in 0..n_rows {
        let fy = (j as f64 + 0.5) / n_rows as f64;
        let offset = if pattern == Pattern::Brick && j % 2 == 1 { 0.5 } else { 0.0 };
        // A half-cell shift leaves a gap at one edge, so offset rows draw one
        // extra tile; the two end tiles straddle the border (or, when padded,
        // fall outside the visible frame entirely).
        let count = if offset > 0.0 { n_cols + 1 } else { n_cols };
        for i in 0..count {
            let fx = (i as f64 + 0.5 - offset) / n_cols as f64;
            out.push((fx, fy));
        }
    }
    out
}

/// Build the full `-filter_complex` graph. Returns a single space-free token.
#[allow(clippy::too_many_arguments)]
pub fn build_filter(
    font_size: u32,
    color: &str,
    opacity: f64,
    angle: f64,
    columns: u32,
    rows: u32,
    pattern: Pattern,
    outline: bool,
) -> Result<String, String> {
    if !(MIN_FONT_SIZE as u32..=MAX_FONT_SIZE as u32).contains(&font_size) {
        return Err(format!(
            "font_size must be between {} and {}, got {font_size}",
            MIN_FONT_SIZE as u32, MAX_FONT_SIZE as u32
        ));
    }
    if !opacity.is_finite() || !(MIN_OPACITY..=MAX_OPACITY).contains(&opacity) {
        return Err(format!(
            "opacity must be between {MIN_OPACITY} and {MAX_OPACITY}, got {opacity}"
        ));
    }
    if !angle.is_finite() || angle.abs() > MAX_ANGLE {
        return Err(format!(
            "angle must be between -{MAX_ANGLE} and {MAX_ANGLE} degrees, got {angle}"
        ));
    }
    if columns == 0 || rows == 0 || columns > MAX_TILES || rows > MAX_TILES {
        return Err(format!(
            "columns and rows must each be between 1 and {MAX_TILES}, got {columns}x{rows}"
        ));
    }
    let color = normalize_ffmpeg_color(if color.trim().is_empty() { DEFAULT_COLOR } else { color })?;

    let rotated = angle != 0.0;
    let border = if outline {
        format!(":borderw={}:bordercolor=black", (font_size / 14).max(2))
    } else {
        String::new()
    };
    let draws: Vec<String> = tile_centers(columns, rows, pattern, rotated)
        .into_iter()
        .map(|(fx, fy)| {
            format!(
                "drawtext=fontfile={FONT_FILE}:textfile={TEXT_FILE}:expansion=none:fontsize={font_size}:fontcolor={color}{border}:x=w*{}-text_w/2:y=h*{}-text_h/2",
                num(fx),
                num(fy)
            )
        })
        .collect();

    let mut layer = vec!["colorchannelmixer=rr=0:gg=0:bb=0:aa=0".to_string()];
    if rotated {
        layer.push(format!(
            "pad=w=iw*{p}:h=ih*{p}:x=(ow-iw)/2:y=(oh-ih)/2:color=black@0",
            p = num(PAD)
        ));
    }
    layer.extend(draws);
    if rotated {
        layer.push(format!(
            "rotate={}:c=black@0",
            num(angle * std::f64::consts::PI / 180.0)
        ));
    }
    layer.push(format!("colorchannelmixer=aa={}", num(opacity)));

    // A padded layer is larger than the base, so overlay it with negative
    // offsets; ffmpeg clips the excess. Un-padded layers align at 0:0.
    let place = if rotated { "x=(W-w)/2:y=(H-h)/2" } else { "x=0:y=0" };
    Ok(format!(
        "[0:v]format=rgba,split=2[wmbase][wmlay];[wmlay]{};[wmbase][wmtile]overlay={place}:format=rgb",
        format!("{}[wmtile]", layer.join(","))
    ))
}

fn in_ext(in_name: &str) -> &str {
    in_name
        .rsplit_once('.')
        .map(|(_, e)| e)
        .filter(|e| !e.is_empty() && e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("png")
}

fn out_ext(format: OutFormat, in_name: &str) -> &str {
    match format {
        OutFormat::Keep => in_ext(in_name),
        OutFormat::Png => "png",
        OutFormat::Jpg => "jpg",
        OutFormat::Webp => "webp",
    }
}

/// Build the ffmpeg argv (no leading "ffmpeg") + the output filename.
///
/// `text` is validated here but never enters the filtergraph — the caller writes
/// it to [`TEXT_FILE`] in the (virtual) FS.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    in_name: &str,
    text: &str,
    font_size: f64,
    color: &str,
    opacity: f64,
    angle: f64,
    columns: f64,
    rows: f64,
    pattern: Pattern,
    outline: bool,
    format: OutFormat,
) -> Result<(Vec<String>, String), String> {
    if text.trim().is_empty() {
        return Err("text must not be empty — pass the watermark to repeat".into());
    }
    if text.chars().count() > MAX_TEXT_LEN {
        return Err(format!(
            "text is too long (max {MAX_TEXT_LEN} characters, got {})",
            text.chars().count()
        ));
    }
    let font_size = bounded_u32(
        font_size,
        DEFAULT_FONT_SIZE as u32,
        MIN_FONT_SIZE as u32,
        MAX_FONT_SIZE as u32,
        "font_size",
    )?;
    let columns = bounded_u32(columns, DEFAULT_COLUMNS, 1, MAX_TILES, "columns")?;
    let rows = bounded_u32(rows, DEFAULT_ROWS, 1, MAX_TILES, "rows")?;
    let filter = build_filter(font_size, color, opacity, angle, columns, rows, pattern, outline)?;

    let ext = out_ext(format, in_name);
    let out_name = format!("out.{ext}");
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-filter_complex".to_string(),
        filter,
    ];
    if format == OutFormat::Jpg {
        argv.push("-q:v".to_string());
        argv.push("2".to_string());
    }
    if format != OutFormat::Keep {
        // An explicit conversion is a still image: take the first frame only.
        argv.push("-frames:v".to_string());
        argv.push("1".to_string());
        argv.push("-update".to_string());
        argv.push("1".to_string());
    }
    argv.push("-y".to_string());
    argv.push(out_name.clone());
    Ok((argv, out_name))
}

/// String-typed entry point for the surfaces that hand through raw user values
/// (page fields, CLI/chat JSON). Applies every default.
#[allow(clippy::too_many_arguments)]
pub fn plan_named(
    in_name: &str,
    text: &str,
    font_size: f64,
    color: &str,
    opacity: f64,
    angle: f64,
    columns: f64,
    rows: f64,
    pattern: Option<&str>,
    outline: bool,
    format: Option<&str>,
) -> Result<(Vec<String>, String), String> {
    let pattern = parse_pattern(pattern)?;
    let format = parse_format(format)?;
    plan(
        in_name, text, font_size, color, opacity, angle, columns, rows, pattern, outline, format,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fc(argv: &[String]) -> String {
        argv[argv.iter().position(|a| a == "-filter_complex").unwrap() + 1].clone()
    }

    fn count(hay: &str, needle: &str) -> usize {
        hay.matches(needle).count()
    }

    #[test]
    fn happy_path_tiles_the_whole_image() {
        let (argv, out) = plan(
            "in.png", "SAMPLE", 32.0, "#ffffff", 0.3, 30.0, 4.0, 5.0, Pattern::Brick,
            false, OutFormat::Keep,
        )
        .unwrap();
        assert_eq!(out, "out.png");
        assert_eq!(&argv[0..2], &["-i".to_string(), "in.png".to_string()][..]);
        let f = fc(&argv);
        // The user's text is never interpolated into the graph.
        assert!(!f.contains("SAMPLE"));
        assert!(f.contains(&format!("textfile={TEXT_FILE}")));
        assert!(f.contains(&format!("fontfile={FONT_FILE}")));
        // Transparent layer → opaque glyphs → one alpha scale → recentred overlay.
        assert!(f.contains("split=2[wmbase][wmlay]"));
        assert!(f.contains("colorchannelmixer=rr=0:gg=0:bb=0:aa=0"));
        assert!(f.contains("fontcolor=0xFFFFFF:x="), "{f}");
        assert!(f.contains("colorchannelmixer=aa=0.3[wmtile]"));
        assert!(f.contains("overlay=x=(W-w)/2:y=(H-h)/2:format=rgb"));
        // 30° → padded canvas + rotation in radians.
        assert!(f.contains("pad=w=iw*1.5:h=ih*1.5"));
        assert!(f.contains("rotate=0.5236:c=black@0"), "{f}");
        // 4x5 brick on a 1.5x canvas = 6 cols x 8 rows, offset rows get +1 tile.
        assert_eq!(count(&f, "drawtext="), 6 * 8 + 4);
        // Single space-free token, safe as one argv element.
        assert!(!f.contains(' '));
    }

    #[test]
    fn zero_angle_skips_the_pad_and_rotate_hop() {
        let (argv, _) = plan(
            "in.jpg", "© STUDIO", 24.0, "black", 0.5, 0.0, 3.0, 3.0, Pattern::Grid, false,
            OutFormat::Keep,
        )
        .unwrap();
        let f = fc(&argv);
        assert!(!f.contains("pad="));
        assert!(!f.contains("rotate="));
        assert!(f.contains("overlay=x=0:y=0"));
        assert_eq!(count(&f, "drawtext="), 9);
        assert!(f.contains("fontcolor=black"));
    }

    #[test]
    fn tile_centers_stay_inside_the_frame_and_keep_visible_density() {
        // Un-padded grid: every centre strictly inside 0..1.
        for (fx, fy) in tile_centers(4, 4, Pattern::Grid, false) {
            assert!(fx > 0.0 && fx < 1.0 && fy > 0.0 && fy < 1.0);
        }
        // Brick rows straddle the edges at exactly 0 and 1, never beyond.
        for (fx, _) in tile_centers(4, 4, Pattern::Brick, false) {
            assert!((0.0..=1.0).contains(&fx));
        }
        // Padded: 1/6 cell width on the padded canvas = 1/4 of the visible frame.
        let padded = tile_centers(4, 4, Pattern::Grid, true);
        assert_eq!(padded.len(), 6 * 6);
        let first_row: Vec<f64> = padded.iter().filter(|(_, fy)| *fy == padded[0].1).map(|(fx, _)| *fx).collect();
        assert_eq!(first_row.len(), 6);
        assert!((first_row[1] - first_row[0] - 1.0 / 6.0).abs() < 1e-9);
    }

    #[test]
    fn outline_adds_a_border_scaled_to_the_font() {
        let f = build_filter(56, "#fff", 0.3, 0.0, 2, 2, Pattern::Grid, true).unwrap();
        assert!(f.contains("borderw=4:bordercolor=black"), "{f}");
        let small = build_filter(10, "#fff", 0.3, 0.0, 2, 2, Pattern::Grid, true).unwrap();
        assert!(small.contains("borderw=2:bordercolor=black"), "{small}");
        let none = build_filter(56, "#fff", 0.3, 0.0, 2, 2, Pattern::Grid, false).unwrap();
        assert!(!none.contains("borderw"));
    }

    #[test]
    fn formats_pick_the_extension_and_encoder_flags() {
        for (fmt, ext) in [
            (OutFormat::Keep, "webp"),
            (OutFormat::Png, "png"),
            (OutFormat::Jpg, "jpg"),
            (OutFormat::Webp, "webp"),
        ] {
            let (argv, out) = plan(
                "photo.webp", "X", 32.0, "white", 0.3, 0.0, 2.0, 2.0, Pattern::Grid, false, fmt,
            )
            .unwrap();
            assert_eq!(out, format!("out.{ext}"));
            assert_eq!(argv.last().unwrap(), &out);
        }
        let (jpg, _) = plan(
            "a.png", "X", 32.0, "white", 0.3, 0.0, 2.0, 2.0, Pattern::Grid, false, OutFormat::Jpg,
        )
        .unwrap();
        assert!(jpg.windows(2).any(|w| w == ["-q:v", "2"]), "{jpg:?}");
        assert!(jpg.windows(2).any(|w| w == ["-frames:v", "1"]));
        // keep must not cap frames — an animated GIF stays animated.
        let (keep, _) = plan(
            "a.gif", "X", 32.0, "white", 0.3, 0.0, 2.0, 2.0, Pattern::Grid, false, OutFormat::Keep,
        )
        .unwrap();
        assert!(!keep.contains(&"-frames:v".to_string()), "{keep:?}");
    }

    #[test]
    fn keep_falls_back_to_png_for_an_extensionless_input() {
        let (_, out) = plan(
            "download", "X", 32.0, "white", 0.3, 0.0, 2.0, 2.0, Pattern::Grid, false,
            OutFormat::Keep,
        )
        .unwrap();
        assert_eq!(out, "out.png");
    }

    #[test]
    fn rejects_bad_inputs() {
        let bad = |text: &str, fs: f64, color: &str, op: f64, ang: f64, c: f64, r: f64| {
            plan(
                "in.png", text, fs, color, op, ang, c, r, Pattern::Grid, false, OutFormat::Keep,
            )
        };
        assert!(bad("", 32.0, "white", 0.3, 0.0, 4.0, 4.0).is_err(), "empty text");
        assert!(bad("   ", 32.0, "white", 0.3, 0.0, 4.0, 4.0).is_err(), "blank text");
        assert!(bad(&"x".repeat(121), 32.0, "white", 0.3, 0.0, 4.0, 4.0).is_err(), "long text");
        assert!(bad("X", 4.0, "white", 0.3, 0.0, 4.0, 4.0).is_err(), "font too small");
        assert!(bad("X", 500.0, "white", 0.3, 0.0, 4.0, 4.0).is_err(), "font too large");
        assert!(bad("X", 32.0, "chartreusey", 0.3, 0.0, 4.0, 4.0).is_err(), "bad color");
        assert!(bad("X", 32.0, "white", 0.0, 0.0, 4.0, 4.0).is_err(), "opacity 0");
        assert!(bad("X", 32.0, "white", 1.5, 0.0, 4.0, 4.0).is_err(), "opacity > 1");
        assert!(bad("X", 32.0, "white", 0.3, 120.0, 4.0, 4.0).is_err(), "angle out of range");
        assert!(bad("X", 32.0, "white", 0.3, 0.0, 0.0, 4.0).is_err(), "0 columns");
        assert!(bad("X", 32.0, "white", 0.3, 0.0, 13.0, 4.0).is_err(), "too many columns");
        assert!(bad("X", 32.0, "white", 0.3, 0.0, 4.0, 13.0).is_err(), "too many rows");
        assert!(parse_pattern(Some("spiral")).is_err());
        assert!(parse_format(Some("tiff")).is_err());
    }

    #[test]
    fn every_enum_value_parses_and_plans() {
        for p in PATTERNS {
            let pat = parse_pattern(Some(p)).unwrap();
            assert_eq!(pattern_name(pat), p);
            build_filter(32, "white", 0.3, 30.0, 4, 4, pat, false).unwrap();
        }
        for f in FORMATS {
            let fmt = parse_format(Some(f)).unwrap();
            plan(
                "in.png", "X", 32.0, "white", 0.3, 0.0, 2.0, 2.0, Pattern::Grid, false, fmt,
            )
            .unwrap();
        }
        assert_eq!(parse_pattern(None).unwrap(), Pattern::Brick);
        assert_eq!(parse_format(None).unwrap(), OutFormat::Keep);
        assert_eq!(parse_format(Some("JPEG")).unwrap(), OutFormat::Jpg);
    }

    #[test]
    fn negative_angle_rotates_the_other_way() {
        let f = build_filter(32, "white", 0.3, -45.0, 3, 3, Pattern::Grid, false).unwrap();
        assert!(f.contains("rotate=-0.7854:c=black@0"), "{f}");
    }

    #[test]
    fn font_bytes_are_bundled() {
        assert!(FONT_BYTES.len() > 10_000);
        assert_eq!(&FONT_BYTES[0..4], &[0x00, 0x01, 0x00, 0x00]);
    }

    #[test]
    fn plan_named_applies_string_defaults() {
        let (argv, out) = plan_named(
            "in.png", "SAMPLE", DEFAULT_FONT_SIZE, DEFAULT_COLOR, DEFAULT_OPACITY, DEFAULT_ANGLE,
            DEFAULT_COLUMNS as f64, DEFAULT_ROWS as f64, None, false, None,
        )
        .unwrap();
        assert_eq!(out, "out.png");
        assert!(fc(&argv).contains("rotate="));
    }
}
