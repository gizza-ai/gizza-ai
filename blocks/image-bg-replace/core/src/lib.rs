//! gizza-ai/image-bg-replace core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Chroma-key background replacement. The user names a `key_color` (the
//! background color to remove — a green/blue screen or any solid backdrop);
//! ffmpeg's `colorkey` filter turns pixels near that color transparent, with a
//! `similarity` threshold (how wide a color range counts as background) and a
//! `blend` amount (how soft the cut-out edge is). The subject is then composited
//! onto a new background:
//! - **transparent** — keep the alpha; output PNG/WebP with a see-through
//!   background (`colorkey` alone).
//! - **solid** — flood-fill a `bg_color` frame (`lutrgb` constant map, sized to
//!   the input) and `overlay` the subject on top.
//! - **gradient** — synthesize a two-color linear gradient (`bg_color` →
//!   `bg_color2`, `vertical` or `horizontal`) with `geq` — sized to the input,
//!   no explicit dimensions needed — and `overlay` the subject on top.
//!
//! Every filter string is a single token (no spaces), so the whole `-vf`
//! argument passes as one argv element. Everything is deterministic, so the
//! chat block, CLI and page produce identical output.

/// An RGB color triple.
pub type Rgb = (u8, u8, u8);

/// The default key color to remove: pure chroma green (`0x00FF00`). Named
/// `green` in CSS is the darker `(0,128,0)`, so the default is stored as hex to
/// avoid the classic green-screen confusion.
pub const DEFAULT_KEY_COLOR: &str = "#00ff00";

/// Default similarity threshold (percent). 30 keys a moderately wide range of
/// greens — enough for a real green screen, tight enough to keep the subject.
pub const DEFAULT_SIMILARITY: f64 = 30.0;

/// Default edge blend (percent). 10 softens the cut-out edge slightly to hide
/// aliasing without eating into the subject.
pub const DEFAULT_BLEND: f64 = 10.0;

/// Canonical background-type names, in display order. KEEP IN SYNC with
/// `parse_bg_type`.
pub const BG_TYPES: [&str; 3] = ["transparent", "solid", "gradient"];

/// Default background type: composite onto a solid color.
pub const DEFAULT_BG_TYPE: &str = "solid";

/// Default solid fill / gradient start color (white).
pub const DEFAULT_BG_COLOR: &str = "#ffffff";

/// Default gradient end color (black).
pub const DEFAULT_BG_COLOR2: &str = "#000000";

/// Canonical gradient-direction names, in display order. KEEP IN SYNC with
/// `parse_direction`.
pub const DIRECTIONS: [&str; 2] = ["vertical", "horizontal"];

/// Default gradient direction (top → bottom).
pub const DEFAULT_DIRECTION: &str = "vertical";

/// Canonical output-format names, in display order. KEEP IN SYNC with
/// `parse_format`.
pub const FORMATS: [&str; 4] = ["keep", "png", "jpg", "webp"];

/// Default output format: PNG (lossless, keeps transparency).
pub const DEFAULT_FORMAT: &str = "png";

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BgType {
    Transparent,
    Solid,
    Gradient,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Direction {
    Vertical,
    Horizontal,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OutFormat {
    Keep,
    Png,
    Jpg,
    Webp,
}

/// Named colors accepted for the color fields, with their sRGB values. A
/// curated set of common CSS names — anything else is expressible as hex.
/// `green` is CSS green `(0,128,0)`; use `lime` or `#00ff00` for a chroma-green
/// screen. KEEP the error message in `parse_color` honest.
const COLOR_NAMES: &[(&str, Rgb)] = &[
    ("black", (0, 0, 0)),
    ("white", (255, 255, 255)),
    ("gray", (128, 128, 128)),
    ("grey", (128, 128, 128)),
    ("silver", (192, 192, 192)),
    ("red", (255, 0, 0)),
    ("maroon", (128, 0, 0)),
    ("orange", (255, 165, 0)),
    ("gold", (255, 215, 0)),
    ("yellow", (255, 255, 0)),
    ("olive", (128, 128, 0)),
    ("green", (0, 128, 0)),
    ("lime", (0, 255, 0)),
    ("teal", (0, 128, 128)),
    ("cyan", (0, 255, 255)),
    ("aqua", (0, 255, 255)),
    ("blue", (0, 0, 255)),
    ("navy", (0, 0, 128)),
    ("royalblue", (65, 105, 225)),
    ("skyblue", (135, 206, 235)),
    ("purple", (128, 0, 128)),
    ("magenta", (255, 0, 255)),
    ("fuchsia", (255, 0, 255)),
    ("pink", (255, 192, 203)),
    ("brown", (165, 42, 42)),
    ("beige", (245, 245, 220)),
    ("ivory", (255, 255, 240)),
];

/// Parse a color: a name from `COLOR_NAMES`, or hex as `#RGB`, `#RRGGBB`,
/// `0xRRGGBB`, or bare `RGB`/`RRGGBB`. `None` / `""` map to the provided
/// default (already a valid color string).
pub fn parse_color(s: Option<&str>, default: &str) -> Result<Rgb, String> {
    let raw = s.map(|v| v.trim()).filter(|v| !v.is_empty()).unwrap_or(default);
    let lower = raw.to_ascii_lowercase();
    if let Some((_, rgb)) = COLOR_NAMES.iter().find(|(n, _)| *n == lower) {
        return Ok(*rgb);
    }
    let hex = lower
        .strip_prefix('#')
        .or_else(|| lower.strip_prefix("0x"))
        .unwrap_or(&lower);
    if !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let nib = |c: char| c.to_digit(16).unwrap() as u8;
        let b: Vec<u8> = hex.chars().map(nib).collect();
        match b.as_slice() {
            [r, g, bl] => return Ok((r * 17, g * 17, bl * 17)),
            [r1, r2, g1, g2, b1, b2] => {
                return Ok((r1 * 16 + r2, g1 * 16 + g2, b1 * 16 + b2))
            }
            _ => {}
        }
    }
    Err(format!(
        "color {raw:?} not recognized — use hex like #00FF00 or #0F0, or a name \
         (green, lime, blue, cyan, white, black, red, …)"
    ))
}

/// Parse a background type (case-insensitive). `None` / `""` default to `solid`.
pub fn parse_bg_type(s: Option<&str>) -> Result<BgType, String> {
    let v = s.unwrap_or(DEFAULT_BG_TYPE).trim().to_ascii_lowercase();
    match v.as_str() {
        "" | "solid" => Ok(BgType::Solid),
        "transparent" | "none" | "alpha" => Ok(BgType::Transparent),
        "gradient" => Ok(BgType::Gradient),
        other => Err(format!(
            "invalid background {other:?}; expected one of {}",
            BG_TYPES.join("|")
        )),
    }
}

/// Parse a gradient direction (case-insensitive). `None` / `""` default to
/// `vertical`.
pub fn parse_direction(s: Option<&str>) -> Result<Direction, String> {
    let v = s.unwrap_or(DEFAULT_DIRECTION).trim().to_ascii_lowercase();
    match v.as_str() {
        "" | "vertical" | "v" => Ok(Direction::Vertical),
        "horizontal" | "h" => Ok(Direction::Horizontal),
        other => Err(format!(
            "invalid direction {other:?}; expected one of {}",
            DIRECTIONS.join("|")
        )),
    }
}

/// Parse an output format (case-insensitive; `jpeg` is an alias for `jpg`).
/// `None` / `""` default to `png`.
pub fn parse_format(s: Option<&str>) -> Result<OutFormat, String> {
    let v = s.unwrap_or(DEFAULT_FORMAT).trim().to_ascii_lowercase();
    match v.as_str() {
        "" | "png" => Ok(OutFormat::Png),
        "keep" => Ok(OutFormat::Keep),
        "jpg" | "jpeg" => Ok(OutFormat::Jpg),
        "webp" => Ok(OutFormat::Webp),
        other => Err(format!(
            "invalid format {other:?}; expected one of {}",
            FORMATS.join("|")
        )),
    }
}

/// Validate a 0–100 percent value with a named field.
fn check_pct(name: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() || !(0.0..=100.0).contains(&v) {
        return Err(format!(
            "{name} must be a number between 0 and 100, got {v}"
        ));
    }
    Ok(())
}

/// `photo.jpeg` + `png` → `photo.png` (append when there is no extension).
pub fn rename_ext(name: &str, ext: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => format!("{stem}.{ext}"),
        _ => format!("{name}.{ext}"),
    }
}

/// The output extension for `format`, given the input's extension.
fn out_ext<'a>(format: OutFormat, in_ext: &'a str) -> &'a str {
    match format {
        OutFormat::Keep => in_ext,
        OutFormat::Png => "png",
        OutFormat::Jpg => "jpg",
        OutFormat::Webp => "webp",
    }
}

/// The `colorkey` filter token: `colorkey=0xRRGGBB:similarity:blend`. Similarity
/// is clamped to ffmpeg's valid `[0.01, 1.0]` (0 would be rejected); blend to
/// `[0.0, 1.0]`.
fn colorkey_token(key: Rgb, similarity: f64, blend: f64) -> String {
    let (r, g, b) = key;
    let sim = (similarity / 100.0).clamp(0.01, 1.0);
    let bl = (blend / 100.0).clamp(0.0, 1.0);
    format!("colorkey=0x{r:02x}{g:02x}{b:02x}:{sim:.4}:{bl:.4}")
}

/// The `-vf` filter string that keys the background out and composites the
/// subject onto the chosen new background. Single argv token (no spaces).
pub fn filter(
    key: Rgb,
    similarity: f64,
    blend: f64,
    bg_type: BgType,
    bg_color: Rgb,
    bg_color2: Rgb,
    direction: Direction,
) -> Result<String, String> {
    check_pct("similarity", similarity)?;
    check_pct("blend", blend)?;
    let ck = colorkey_token(key, similarity, blend);
    match bg_type {
        // Keep the alpha: just key the background out (PNG/WebP carry it).
        BgType::Transparent => Ok(ck),
        // Flood a solid color frame (sized to the input via lutrgb) and overlay
        // the keyed subject on top. format=rgb24 drops any input alpha so the
        // fill is fully opaque even for a transparent-PNG source.
        BgType::Solid => {
            let (r, g, b) = bg_color;
            Ok(format!(
                "split[a][b];[b]{ck}[fg];\
                 [a]format=rgb24,lutrgb=r={r}:g={g}:b={b}[bg];[bg][fg]overlay"
            ))
        }
        // Synthesize a two-color linear gradient with geq (auto-sized to the
        // input frame), then overlay the keyed subject. Vertical interpolates
        // over Y/H, horizontal over X/W.
        BgType::Gradient => {
            let (axis, span) = match direction {
                Direction::Vertical => ("Y", "H"),
                Direction::Horizontal => ("X", "W"),
            };
            let (r1, g1, b1) = bg_color;
            let (r2, g2, b2) = bg_color2;
            let chan = |c1: u8, c2: u8| format!("{c1}+({c2}-{c1})*{axis}/{span}");
            Ok(format!(
                "split[a][b];[b]{ck}[fg];\
                 [a]format=gbrp,geq=r={}:g={}:b={}[bg];[bg][fg]overlay",
                chan(r1, r2),
                chan(g1, g2),
                chan(b1, b2),
            ))
        }
    }
}

/// Build the ffmpeg argv (no leading "ffmpeg") and the output filename for
/// replacing the background of `in_name`.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    in_name: &str,
    key: Rgb,
    similarity: f64,
    blend: f64,
    bg_type: BgType,
    bg_color: Rgb,
    bg_color2: Rgb,
    direction: Direction,
    format: OutFormat,
) -> Result<(Vec<String>, String), String> {
    let in_ext = in_name
        .rsplit('.')
        .next()
        .filter(|e| !e.is_empty())
        .unwrap_or("png");
    let ext = out_ext(format, in_ext);
    // A transparent background needs an alpha-capable container.
    if bg_type == BgType::Transparent && (ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg"))
    {
        return Err(format!(
            "a transparent background needs a png or webp output — jpg has no \
             alpha channel (input is .{in_ext}); pick format png or webp, or \
             choose a solid/gradient background"
        ));
    }
    let vf = filter(key, similarity, blend, bg_type, bg_color, bg_color2, direction)?;
    let out_name = format!("out.{ext}");
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vf".to_string(),
        vf,
    ];
    if ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg") {
        // mjpeg's default quality is visibly lossy; pin a high-quality encode.
        argv.push("-q:v".to_string());
        argv.push("2".to_string());
    }
    if format != OutFormat::Keep {
        // Explicit formats are single still images: take one frame so an
        // animated input converts cleanly instead of erroring in the muxer.
        argv.push("-frames:v".to_string());
        argv.push("1".to_string());
    }
    argv.push(out_name.clone());
    Ok((argv, out_name))
}

/// Parse + plan in one step from raw strings (used by the web page and CLI
/// paths where every field arrives as text).
#[allow(clippy::too_many_arguments)]
pub fn plan_named(
    in_name: &str,
    key_color: Option<&str>,
    similarity: f64,
    blend: f64,
    bg_type: Option<&str>,
    bg_color: Option<&str>,
    bg_color2: Option<&str>,
    direction: Option<&str>,
    format: Option<&str>,
) -> Result<(Vec<String>, String), String> {
    let key = parse_color(key_color, DEFAULT_KEY_COLOR)?;
    let bg_type = parse_bg_type(bg_type)?;
    let bg_color = parse_color(bg_color, DEFAULT_BG_COLOR)?;
    let bg_color2 = parse_color(bg_color2, DEFAULT_BG_COLOR2)?;
    let direction = parse_direction(direction)?;
    let format = parse_format(format)?;
    plan(
        in_name, key, similarity, blend, bg_type, bg_color, bg_color2, direction, format,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn green() -> Rgb {
        (0, 255, 0)
    }

    #[test]
    fn solid_composite_argv_is_wellformed() {
        let (argv, out) = plan(
            "in.png",
            green(),
            30.0,
            10.0,
            BgType::Solid,
            (255, 255, 255),
            (0, 0, 0),
            Direction::Vertical,
            OutFormat::Png,
        )
        .unwrap();
        assert_eq!(out, "out.png");
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "in.png");
        assert_eq!(argv[2], "-vf");
        let vf = &argv[3];
        // key green, solid white fill, overlay, and one still frame.
        assert!(vf.contains("colorkey=0x00ff00:0.3000:0.1000"), "vf: {vf}");
        assert!(vf.contains("lutrgb=r=255:g=255:b=255"), "vf: {vf}");
        assert!(vf.contains("overlay"), "vf: {vf}");
        assert!(!vf.contains(' '), "filter must be a single token: {vf}");
        assert!(argv.contains(&"-frames:v".to_string()));
        assert_eq!(argv.last().unwrap(), "out.png");
    }

    #[test]
    fn transparent_keeps_only_colorkey_and_alpha_output() {
        let (argv, out) = plan(
            "photo.png",
            green(),
            25.0,
            5.0,
            BgType::Transparent,
            (255, 255, 255),
            (0, 0, 0),
            Direction::Vertical,
            OutFormat::Keep,
        )
        .unwrap();
        assert_eq!(out, "out.png");
        let vf = &argv[3];
        assert_eq!(vf, "colorkey=0x00ff00:0.2500:0.0500");
        assert!(!vf.contains("overlay"));
        // keep + png input → no forced -frames:v
        assert!(!argv.contains(&"-frames:v".to_string()));
    }

    #[test]
    fn gradient_vertical_interpolates_over_y() {
        let vf = filter(
            green(),
            30.0,
            10.0,
            BgType::Gradient,
            (0, 0, 255),
            (255, 0, 0),
            Direction::Vertical,
        )
        .unwrap();
        assert!(vf.contains("geq=r=0+(255-0)*Y/H"), "vf: {vf}");
        assert!(vf.contains("b=255+(0-255)*Y/H"), "vf: {vf}");
        assert!(vf.contains("overlay"));
    }

    #[test]
    fn gradient_horizontal_interpolates_over_x() {
        let vf = filter(
            green(),
            30.0,
            10.0,
            BgType::Gradient,
            (0, 0, 255),
            (255, 0, 0),
            Direction::Horizontal,
        )
        .unwrap();
        assert!(vf.contains("*X/W"), "vf: {vf}");
        assert!(!vf.contains("*Y/H"), "vf: {vf}");
    }

    #[test]
    fn similarity_zero_is_clamped_to_ffmpeg_minimum() {
        // colorkey rejects similarity 0; the plan clamps to 0.01 so a user who
        // clears the field still gets valid argv.
        let vf = filter(green(), 0.0, 0.0, BgType::Transparent, (0, 0, 0), (0, 0, 0), Direction::Vertical)
            .unwrap();
        assert!(vf.contains(":0.0100:0.0000"), "vf: {vf}");
    }

    #[test]
    fn jpg_output_pins_quality() {
        let (argv, out) = plan(
            "in.jpg",
            green(),
            30.0,
            10.0,
            BgType::Solid,
            (255, 255, 255),
            (0, 0, 0),
            Direction::Vertical,
            OutFormat::Keep,
        )
        .unwrap();
        assert_eq!(out, "out.jpg");
        assert!(argv.windows(2).any(|w| w[0] == "-q:v" && w[1] == "2"));
    }

    #[test]
    fn transparent_with_jpg_output_is_rejected_with_guidance() {
        let err = plan(
            "in.png",
            green(),
            30.0,
            10.0,
            BgType::Transparent,
            (0, 0, 0),
            (0, 0, 0),
            Direction::Vertical,
            OutFormat::Jpg,
        )
        .unwrap_err();
        assert!(err.contains("transparent"), "err: {err}");
        assert!(err.contains("png or webp"), "err: {err}");
    }

    #[test]
    fn transparent_keep_over_jpg_input_is_rejected() {
        let err = plan(
            "photo.jpg",
            green(),
            30.0,
            10.0,
            BgType::Transparent,
            (0, 0, 0),
            (0, 0, 0),
            Direction::Vertical,
            OutFormat::Keep,
        )
        .unwrap_err();
        assert!(err.contains("alpha"), "err: {err}");
        assert!(err.contains("png or webp"), "err: {err}");
    }

    #[test]
    fn out_of_range_similarity_is_rejected() {
        let err = filter(green(), 150.0, 10.0, BgType::Solid, (0, 0, 0), (0, 0, 0), Direction::Vertical)
            .unwrap_err();
        assert!(err.contains("similarity"), "err: {err}");
        assert!(err.contains("between 0 and 100"), "err: {err}");
    }

    #[test]
    fn parse_color_names_and_hex() {
        assert_eq!(parse_color(Some("lime"), DEFAULT_KEY_COLOR).unwrap(), (0, 255, 0));
        assert_eq!(parse_color(Some("#0F0"), DEFAULT_KEY_COLOR).unwrap(), (0, 255, 0));
        assert_eq!(parse_color(Some("0x0000FF"), DEFAULT_KEY_COLOR).unwrap(), (0, 0, 255));
        assert_eq!(parse_color(Some("112233"), DEFAULT_KEY_COLOR).unwrap(), (17, 34, 51));
        // empty → default (green key)
        assert_eq!(parse_color(Some(""), DEFAULT_KEY_COLOR).unwrap(), (0, 255, 0));
        assert_eq!(parse_color(None, DEFAULT_BG_COLOR).unwrap(), (255, 255, 255));
    }

    #[test]
    fn parse_color_rejects_garbage() {
        let err = parse_color(Some("nope"), DEFAULT_KEY_COLOR).unwrap_err();
        assert!(err.contains("not recognized"), "err: {err}");
    }

    #[test]
    fn parse_enums_default_and_reject() {
        assert_eq!(parse_bg_type(None).unwrap(), BgType::Solid);
        assert_eq!(parse_bg_type(Some("transparent")).unwrap(), BgType::Transparent);
        assert!(parse_bg_type(Some("blur")).is_err());
        assert_eq!(parse_direction(None).unwrap(), Direction::Vertical);
        assert_eq!(parse_direction(Some("horizontal")).unwrap(), Direction::Horizontal);
        assert!(parse_direction(Some("diagonal")).is_err());
        assert_eq!(parse_format(None).unwrap(), OutFormat::Png);
        assert_eq!(parse_format(Some("jpeg")).unwrap(), OutFormat::Jpg);
        assert!(parse_format(Some("gif")).is_err());
    }

    #[test]
    fn plan_named_end_to_end_defaults() {
        let (argv, out) = plan_named(
            "in.png", None, DEFAULT_SIMILARITY, DEFAULT_BLEND, None, None, None, None, None,
        )
        .unwrap();
        assert_eq!(out, "out.png");
        let vf = &argv[3];
        assert!(vf.contains("colorkey=0x00ff00:0.3000:0.1000"), "vf: {vf}");
        assert!(vf.contains("lutrgb=r=255:g=255:b=255"), "vf: {vf}");
    }
}
