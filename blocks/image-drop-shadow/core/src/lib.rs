//! gizza-ai/image-drop-shadow core — pure ffmpeg argv construction shared by
//! the chat skill block, the CLI and the standalone web page. No wafer /
//! wasm-bindgen deps, no I/O: the same params always produce the same argv.
//!
//! # What the filtergraph does
//!
//! A drop shadow follows the *alpha channel* of a cutout (the CSS
//! `filter: drop-shadow()` model), not the image's rectangular box. So the
//! graph copies the input, throws away its colors, keeps its (blurred, scaled)
//! alpha as the shadow silhouette, shifts it, and puts the original back on
//! top:
//!
//! ```text
//! format=rgba,pad=<expand>,split=2[fg][s0];
//! [s0]colorchannelmixer=aa=<opacity>,   // scale alpha -> shadow strength
//!     format=gbrap,gblur=sigma=<blur/2>,// blur the alpha plane (soft edge)
//!     lutrgb=r=R:g=G:b=B,               // paint every pixel the shadow color
//!     crop=<shift>,pad=<shift>[sh];     // move the silhouette by offset x/y
//! [sh][fg]overlay=0:0:format=auto       // subject back on top, unblurred
//! ```
//!
//! `gblur`'s default `planes` mask covers the alpha plane, and `lutrgb` never
//! touches alpha — so blurring first and colorizing after yields a solid-color
//! shadow with a soft alpha edge. Every filter used here (`format`, `pad`,
//! `split`, `colorchannelmixer`, `gblur`, `lutrgb`, `crop`, `overlay`) already
//! ships in the browser `@ffmpeg/core` build other gizza pages use.
//!
//! # Canvas
//!
//! By default the canvas is *expanded* so the shadow is never clipped: the
//! padding is derived from the blur reach (3 sigma) plus the offset. `padding`
//! overrides that with an explicit margin, and `clip_to_original` keeps the
//! input's exact dimensions instead (shadow clipped at the frame, like a
//! CSS shadow inside a fixed box).
//!
//! # Blur units
//!
//! `blur` is a CSS-style blur *radius*; the Gaussian sigma is `blur / 2`, the
//! same convention `filter: drop-shadow(x y blur)` uses. blur 0 = a hard,
//! perfectly sharp shadow (no `gblur` in the chain at all).

/// An sRGB color triple.
pub type Rgb = (u8, u8, u8);

/// Default horizontal shadow offset, px (positive = right).
pub const DEFAULT_OFFSET_X: f64 = 12.0;
/// Default vertical shadow offset, px (positive = down).
pub const DEFAULT_OFFSET_Y: f64 = 16.0;
/// Default blur radius, px (CSS convention: sigma = blur / 2).
pub const DEFAULT_BLUR: f64 = 24.0;
/// Default shadow color — neutral black, the safest cast-shadow color.
pub const DEFAULT_COLOR: &str = "#000000";
/// Default shadow opacity, percent. Two competitors recommend 30–50 %.
pub const DEFAULT_OPACITY: f64 = 35.0;
/// Default padding, px. 0 means "auto" — derived from blur reach + offset.
pub const DEFAULT_PADDING: f64 = 0.0;
/// Default canvas background — keep the PNG transparent.
pub const DEFAULT_BACKGROUND: &str = "transparent";
/// Default output format.
pub const DEFAULT_FORMAT: &str = "png";

/// Largest accepted absolute offset, px.
pub const MAX_OFFSET: f64 = 500.0;
/// Largest accepted blur radius, px.
pub const MAX_BLUR: f64 = 400.0;
/// Largest accepted explicit padding, px.
pub const MAX_PADDING: f64 = 2000.0;

/// The canonical output-format names, in display order. Used for the schema
/// enum and the page `<select>`. KEEP IN SYNC with `parse_format`.
pub const FORMATS: [&str; 3] = ["png", "webp", "jpg"];

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OutFormat {
    Png,
    Webp,
    Jpg,
}

impl OutFormat {
    /// The output file extension (also the muxer ffmpeg infers).
    pub fn ext(self) -> &'static str {
        match self {
            OutFormat::Png => "png",
            OutFormat::Webp => "webp",
            OutFormat::Jpg => "jpg",
        }
    }

    /// The MIME type of the produced file.
    pub fn mime(self) -> &'static str {
        match self {
            OutFormat::Png => "image/png",
            OutFormat::Webp => "image/webp",
            OutFormat::Jpg => "image/jpeg",
        }
    }

    /// Whether the format can carry the shadow's partial transparency.
    pub fn keeps_alpha(self) -> bool {
        !matches!(self, OutFormat::Jpg)
    }
}

/// Parse an output format name (case-insensitive). `None` / `""` → png.
pub fn parse_format(s: Option<&str>) -> Result<OutFormat, String> {
    match s.unwrap_or(DEFAULT_FORMAT).trim().to_ascii_lowercase().as_str() {
        "" | "png" => Ok(OutFormat::Png),
        "webp" => Ok(OutFormat::Webp),
        "jpg" | "jpeg" => Ok(OutFormat::Jpg),
        other => Err(format!(
            "invalid format {other:?}; expected one of {}",
            FORMATS.join("|")
        )),
    }
}

/// Named colors accepted for `color` / `background`, with their sRGB values —
/// the common CSS names plus a couple of neutral greys product shots use.
/// Anything else is expressible as hex. KEEP the `parse_color` error honest.
const COLOR_NAMES: &[(&str, Rgb)] = &[
    ("black", (0, 0, 0)),
    ("white", (255, 255, 255)),
    ("gray", (128, 128, 128)),
    ("grey", (128, 128, 128)),
    ("silver", (192, 192, 192)),
    ("charcoal", (51, 51, 51)),
    ("slate", (112, 128, 144)),
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
    ("purple", (128, 0, 128)),
    ("magenta", (255, 0, 255)),
    ("fuchsia", (255, 0, 255)),
    ("pink", (255, 192, 203)),
    ("brown", (165, 42, 42)),
    ("beige", (245, 245, 220)),
];

fn parse_hex(t: &str) -> Option<Rgb> {
    let lower = t.to_ascii_lowercase();
    let hex = lower
        .strip_prefix('#')
        .or_else(|| lower.strip_prefix("0x"))
        .unwrap_or(&lower);
    if hex.is_empty() || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let d = |i: usize| u8::from_str_radix(&hex[i..i + 1], 16).ok();
    let b = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    match hex.len() {
        // #rgb / #rgba — each nibble doubled; alpha is ignored (use `opacity`).
        3 | 4 => {
            let (r, g, bl) = (d(0)?, d(1)?, d(2)?);
            Some((r * 17, g * 17, bl * 17))
        }
        // #rrggbb / #rrggbbaa — alpha ignored (use `opacity`).
        6 | 8 => Some((b(0)?, b(2)?, b(4)?)),
        _ => None,
    }
}

/// Parse the shadow color: a name from `COLOR_NAMES`, or hex as `#RGB`,
/// `#RRGGBB`, `0xRRGGBB` or the bare digits. `None` / `""` default to black.
pub fn parse_color(s: Option<&str>) -> Result<Rgb, String> {
    let t = s.unwrap_or(DEFAULT_COLOR).trim();
    if t.is_empty() {
        return Ok((0, 0, 0));
    }
    let lower = t.to_ascii_lowercase();
    if lower == "transparent" || lower == "none" {
        return Err(
            "color must be a visible shadow color — use opacity=0 for no shadow, or set \
             background=transparent if you meant the canvas"
                .to_string(),
        );
    }
    if let Some((_, rgb)) = COLOR_NAMES.iter().find(|(n, _)| *n == lower) {
        return Ok(*rgb);
    }
    parse_hex(&lower).ok_or_else(|| {
        format!("invalid color {t:?}; use a hex value like #1A2B3C or #333, or a name like black")
    })
}

/// Parse the canvas background: `transparent` / `none` / `""` → `None` (keep
/// the alpha), otherwise a solid opaque fill parsed like `parse_color`.
pub fn parse_background(s: Option<&str>) -> Result<Option<Rgb>, String> {
    let t = s.unwrap_or(DEFAULT_BACKGROUND).trim();
    let lower = t.to_ascii_lowercase();
    if t.is_empty() || lower == "transparent" || lower == "none" {
        return Ok(None);
    }
    if let Some((_, rgb)) = COLOR_NAMES.iter().find(|(n, _)| *n == lower) {
        return Ok(Some(*rgb));
    }
    parse_hex(&lower).map(Some).ok_or_else(|| {
        format!(
            "invalid background {t:?}; use transparent, a hex value like #FFFFFF, or a name \
             like white"
        )
    })
}

fn check_range(name: &str, v: f64, lo: f64, hi: f64, unit: &str) -> Result<(), String> {
    if !v.is_finite() || v < lo || v > hi {
        return Err(format!(
            "{name} must be a number between {lo} and {hi} {unit}, got {v}"
        ));
    }
    Ok(())
}

/// Every knob of the effect, already parsed. Built by the block / page wrappers
/// so `plan` stays a pure function of validated values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    /// Horizontal offset in px; positive moves the shadow right.
    pub offset_x: f64,
    /// Vertical offset in px; positive moves the shadow down.
    pub offset_y: f64,
    /// CSS-style blur radius in px (sigma = blur / 2). 0 = hard edge.
    pub blur: f64,
    /// Shadow color.
    pub color: Rgb,
    /// Shadow opacity in percent, 0–100.
    pub opacity: f64,
    /// Explicit margin in px on every side; 0 = auto (fit the shadow).
    pub padding: f64,
    /// Solid canvas fill, or `None` to keep the background transparent.
    pub background: Option<Rgb>,
    /// Keep the input's exact dimensions and clip the shadow at the frame.
    pub clip_to_original: bool,
    /// Output container.
    pub format: OutFormat,
}

impl Default for Shadow {
    fn default() -> Self {
        Self {
            offset_x: DEFAULT_OFFSET_X,
            offset_y: DEFAULT_OFFSET_Y,
            blur: DEFAULT_BLUR,
            color: (0, 0, 0),
            opacity: DEFAULT_OPACITY,
            padding: DEFAULT_PADDING,
            background: None,
            clip_to_original: false,
            format: OutFormat::Png,
        }
    }
}

impl Shadow {
    fn validate(&self) -> Result<(), String> {
        check_range("offset_x", self.offset_x, -MAX_OFFSET, MAX_OFFSET, "px")?;
        check_range("offset_y", self.offset_y, -MAX_OFFSET, MAX_OFFSET, "px")?;
        check_range("blur", self.blur, 0.0, MAX_BLUR, "px")?;
        check_range("opacity", self.opacity, 0.0, 100.0, "percent")?;
        check_range("padding", self.padding, 0.0, MAX_PADDING, "px")?;
        Ok(())
    }

    /// The Gaussian sigma the blur radius maps to (CSS `drop-shadow()`).
    pub fn sigma(&self) -> f64 {
        self.blur / 2.0
    }

    /// The margin added to every side of the canvas, in px. `clip_to_original`
    /// forces 0; an explicit non-zero `padding` wins; otherwise it is derived
    /// so the blurred, offset silhouette always fits: 3 sigma of blur reach
    /// (a Gaussian is visually done by then) plus the larger offset.
    pub fn margin(&self) -> u32 {
        if self.clip_to_original {
            return 0;
        }
        if self.padding > 0.0 {
            return self.padding.round() as u32;
        }
        let reach = (3.0 * self.sigma()).ceil();
        let shift = self.offset_x.abs().max(self.offset_y.abs()).ceil();
        (reach + shift) as u32
    }
}

/// The single-token `-vf` filtergraph implementing the effect.
///
/// Kept free of spaces so it passes as one argv element on every surface.
pub fn filter(s: &Shadow) -> Result<String, String> {
    s.validate()?;

    let margin = s.margin();
    let (r, g, b) = s.color;
    let alpha = s.opacity / 100.0;
    let sigma = s.sigma();

    // Shift the silhouette by cropping off the leading edge and padding the
    // same amount back on the opposite side. max(...,1) keeps a degenerate
    // crop (an offset wider than the canvas in clip mode) from aborting
    // ffmpeg; the comma inside the expression must be escaped in a filtergraph.
    let dx = s.offset_x.round() as i64;
    let dy = s.offset_y.round() as i64;
    let (adx, ady) = (dx.unsigned_abs(), dy.unsigned_abs());
    let crop_x = if dx < 0 { adx } else { 0 };
    let crop_y = if dy < 0 { ady } else { 0 };
    let pad_x = if dx > 0 { adx } else { 0 };
    let pad_y = if dy > 0 { ady } else { 0 };

    // Source leg: expand the canvas (transparent) so nothing is clipped.
    let mut head = String::from("format=rgba");
    if margin > 0 {
        let two = margin * 2;
        head.push_str(&format!(
            ",pad=iw+{two}:ih+{two}:{margin}:{margin}:color=0x00000000"
        ));
    }

    // Shadow leg: alpha -> opacity -> blur -> colorize -> shift.
    let mut leg = format!("[s0]colorchannelmixer=aa={alpha:.4},format=gbrap");
    if sigma > 0.0 {
        leg.push_str(&format!(",gblur=sigma={sigma:.4}:steps=3"));
    }
    leg.push_str(&format!(",lutrgb=r={r}:g={g}:b={b}"));
    if adx > 0 || ady > 0 {
        leg.push_str(&format!(
            ",crop=w=max(iw-{adx}\\,1):h=max(ih-{ady}\\,1):x={crop_x}:y={crop_y}\
             ,pad=iw+{adx}:ih+{ady}:{pad_x}:{pad_y}:color=0x00000000"
        ));
    }
    leg.push_str("[sh]");

    Ok(match s.background {
        // Transparent canvas: shadow underneath, subject on top.
        None => format!("{head},split=2[fg][s0];{leg};[sh][fg]overlay=0:0:format=auto"),
        // Solid canvas: a third copy flooded to an opaque plate goes first.
        Some((br, bg, bb)) => format!(
            "{head},split=3[fg][s0][bgc];\
             [bgc]lutrgb=r={br}:g={bg}:b={bb}:a=255[bg];\
             {leg};\
             [bg][sh]overlay=0:0:format=auto[bs];\
             [bs][fg]overlay=0:0:format=auto"
        ),
    })
}

/// Build the ffmpeg argv (no leading `ffmpeg`) plus the output filename.
///
/// `in_name` is the virtual-FS name of the uploaded/fetched image. JPEG output
/// cannot hold alpha, so a transparent background is flattened onto white and
/// the encode is pinned to a high quality.
pub fn plan(in_name: &str, shadow: &Shadow) -> Result<(Vec<String>, String), String> {
    let mut s = *shadow;
    if !s.format.keeps_alpha() && s.background.is_none() {
        // A shadow on a transparent canvas would come out black in a JPEG.
        s.background = Some((255, 255, 255));
    }
    let vf = filter(&s)?;
    let out_name = format!("out.{}", s.format.ext());
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vf".to_string(),
        vf,
    ];
    if s.format == OutFormat::Jpg {
        // mjpeg's default quality is visibly lossy; pin a high-quality encode.
        argv.push("-q:v".to_string());
        argv.push("2".to_string());
    }
    // Still image: take one frame so an animated input converts cleanly
    // instead of erroring in the single-image muxer.
    argv.push("-frames:v".to_string());
    argv.push("1".to_string());
    argv.push(out_name.clone());
    Ok((argv, out_name))
}

/// String-argument entry point shared by the page wrapper and the CLI/chat
/// block: parses the loose values, then delegates to [`plan`].
#[allow(clippy::too_many_arguments)]
pub fn plan_named(
    in_name: &str,
    offset_x: f64,
    offset_y: f64,
    blur: f64,
    color: Option<&str>,
    opacity: f64,
    padding: f64,
    background: Option<&str>,
    clip_to_original: bool,
    format: Option<&str>,
) -> Result<(Vec<String>, String), String> {
    let shadow = Shadow {
        offset_x,
        offset_y,
        blur,
        color: parse_color(color)?,
        opacity,
        padding,
        background: parse_background(background)?,
        clip_to_original,
        format: parse_format(format)?,
    };
    plan(in_name, &shadow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_build_a_soft_black_shadow_below_right() {
        let (argv, out) = plan("in.png", &Shadow::default()).unwrap();
        assert_eq!(out, "out.png");
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "in.png");
        assert_eq!(argv[2], "-vf");
        let vf = &argv[3];
        // opacity 35 % -> alpha 0.35; blur 24 -> sigma 12.
        assert!(vf.contains("colorchannelmixer=aa=0.3500"), "vf: {vf}");
        assert!(vf.contains("gblur=sigma=12.0000:steps=3"), "vf: {vf}");
        assert!(vf.contains("lutrgb=r=0:g=0:b=0"), "vf: {vf}");
        // Auto margin: 3*12 reach + max(12,16) offset = 52 -> pad 104 total.
        assert!(vf.contains("pad=iw+104:ih+104:52:52:color=0x00000000"), "vf: {vf}");
        // Positive offsets crop the leading edge and pad the trailing one.
        assert!(vf.contains("crop=w=max(iw-12\\,1):h=max(ih-16\\,1):x=0:y=0"), "vf: {vf}");
        assert!(vf.contains("pad=iw+12:ih+16:12:16:color=0x00000000"), "vf: {vf}");
        assert!(vf.ends_with("[sh][fg]overlay=0:0:format=auto"), "vf: {vf}");
        assert!(!vf.contains(' '), "argv token must have no spaces: {vf}");
        assert_eq!(argv.last().unwrap(), "out.png");
    }

    #[test]
    fn negative_offsets_shift_up_and_left() {
        let s = Shadow {
            offset_x: -20.0,
            offset_y: -10.0,
            ..Shadow::default()
        };
        let vf = filter(&s).unwrap();
        // Crop from the far side, pad back at the near side.
        assert!(vf.contains("crop=w=max(iw-20\\,1):h=max(ih-10\\,1):x=20:y=10"), "vf: {vf}");
        assert!(vf.contains("pad=iw+20:ih+10:0:0:color=0x00000000"), "vf: {vf}");
    }

    #[test]
    fn blur_zero_drops_gblur_for_a_hard_shadow() {
        let s = Shadow {
            blur: 0.0,
            ..Shadow::default()
        };
        let vf = filter(&s).unwrap();
        assert!(!vf.contains("gblur"), "no blur filter at blur=0: {vf}");
        // Margin is then purely the offset (16), so 32 total.
        assert!(vf.contains("pad=iw+32:ih+32:16:16"), "vf: {vf}");
    }

    #[test]
    fn zero_offsets_skip_the_shift_stage() {
        let s = Shadow {
            offset_x: 0.0,
            offset_y: 0.0,
            ..Shadow::default()
        };
        let vf = filter(&s).unwrap();
        assert!(!vf.contains("crop="), "no crop when centered: {vf}");
        // Only the canvas-expanding pad remains (3*12 reach, no offset).
        assert_eq!(vf.matches("pad=").count(), 1, "vf: {vf}");
        assert!(vf.contains("pad=iw+72:ih+72:36:36"), "vf: {vf}");
    }

    #[test]
    fn clip_to_original_expands_nothing() {
        let s = Shadow {
            clip_to_original: true,
            ..Shadow::default()
        };
        let vf = filter(&s).unwrap();
        assert_eq!(s.margin(), 0);
        // The only pad left is the offset shift's own re-pad.
        assert!(!vf.contains("pad=iw+104"), "vf: {vf}");
        assert!(vf.contains("pad=iw+12:ih+16:12:16"), "vf: {vf}");
    }

    #[test]
    fn explicit_padding_overrides_the_auto_margin() {
        let s = Shadow {
            padding: 8.0,
            ..Shadow::default()
        };
        assert_eq!(s.margin(), 8);
        let vf = filter(&s).unwrap();
        assert!(vf.contains("pad=iw+16:ih+16:8:8:color=0x00000000"), "vf: {vf}");
    }

    #[test]
    fn solid_background_adds_an_opaque_plate_under_everything() {
        let s = Shadow {
            background: Some((255, 255, 255)),
            ..Shadow::default()
        };
        let vf = filter(&s).unwrap();
        assert!(vf.contains("split=3[fg][s0][bgc]"), "vf: {vf}");
        assert!(vf.contains("[bgc]lutrgb=r=255:g=255:b=255:a=255[bg]"), "vf: {vf}");
        assert!(vf.contains("[bg][sh]overlay=0:0:format=auto[bs]"), "vf: {vf}");
        assert!(vf.ends_with("[bs][fg]overlay=0:0:format=auto"), "vf: {vf}");
    }

    #[test]
    fn jpg_flattens_a_transparent_canvas_onto_white_and_pins_quality() {
        let s = Shadow {
            format: OutFormat::Jpg,
            ..Shadow::default()
        };
        let (argv, out) = plan("in.png", &s).unwrap();
        assert_eq!(out, "out.jpg");
        assert!(argv[3].contains("lutrgb=r=255:g=255:b=255:a=255"), "{argv:?}");
        assert!(argv.windows(2).any(|w| w == ["-q:v", "2"]), "{argv:?}");
    }

    #[test]
    fn jpg_keeps_an_explicit_background() {
        let s = Shadow {
            format: OutFormat::Jpg,
            background: Some((17, 34, 51)),
            ..Shadow::default()
        };
        let (argv, _) = plan("in.png", &s).unwrap();
        assert!(argv[3].contains("lutrgb=r=17:g=34:b=51:a=255"), "{argv:?}");
    }

    #[test]
    fn webp_keeps_alpha_and_needs_no_quality_pin() {
        let s = Shadow {
            format: OutFormat::Webp,
            ..Shadow::default()
        };
        let (argv, out) = plan("in.png", &s).unwrap();
        assert_eq!(out, "out.webp");
        assert!(!argv.iter().any(|a| a == "-q:v"), "{argv:?}");
        assert!(argv[3].ends_with("[sh][fg]overlay=0:0:format=auto"), "{argv:?}");
    }

    #[test]
    fn plan_named_parses_loose_strings() {
        let (argv, out) = plan_named(
            "photo.png",
            5.0,
            5.0,
            10.0,
            Some("#333"),
            50.0,
            0.0,
            Some("transparent"),
            false,
            Some("png"),
        )
        .unwrap();
        assert_eq!(out, "out.png");
        assert!(argv[3].contains("lutrgb=r=51:g=51:b=51"), "{argv:?}");
        assert!(argv[3].contains("colorchannelmixer=aa=0.5000"), "{argv:?}");
        assert!(argv[3].contains("gblur=sigma=5.0000"), "{argv:?}");
    }

    #[test]
    fn color_parsing_accepts_names_and_every_hex_form() {
        assert_eq!(parse_color(None).unwrap(), (0, 0, 0));
        assert_eq!(parse_color(Some("")).unwrap(), (0, 0, 0));
        assert_eq!(parse_color(Some("Black")).unwrap(), (0, 0, 0));
        assert_eq!(parse_color(Some("charcoal")).unwrap(), (51, 51, 51));
        assert_eq!(parse_color(Some("#333")).unwrap(), (51, 51, 51));
        assert_eq!(parse_color(Some("#112233")).unwrap(), (17, 34, 51));
        assert_eq!(parse_color(Some("112233")).unwrap(), (17, 34, 51));
        assert_eq!(parse_color(Some("0x112233")).unwrap(), (17, 34, 51));
        // Leading zeros must survive (a hex string, never a number).
        assert_eq!(parse_color(Some("011733")).unwrap(), (1, 23, 51));
        // Alpha hex is accepted but ignored — `opacity` owns transparency.
        assert_eq!(parse_color(Some("#11223344")).unwrap(), (17, 34, 51));
    }

    #[test]
    fn background_parsing_maps_transparent_to_none() {
        assert_eq!(parse_background(None).unwrap(), None);
        assert_eq!(parse_background(Some("")).unwrap(), None);
        assert_eq!(parse_background(Some("Transparent")).unwrap(), None);
        assert_eq!(parse_background(Some("none")).unwrap(), None);
        assert_eq!(parse_background(Some("white")).unwrap(), Some((255, 255, 255)));
        assert_eq!(parse_background(Some("#f5f5f5")).unwrap(), Some((245, 245, 245)));
    }

    #[test]
    fn every_advertised_format_parses_and_round_trips() {
        for name in FORMATS {
            let f = parse_format(Some(name)).unwrap();
            assert_eq!(f.ext(), if name == "jpg" { "jpg" } else { name });
            let (_, out) = plan("in.png", &Shadow { format: f, ..Shadow::default() }).unwrap();
            assert_eq!(out, format!("out.{}", f.ext()));
        }
        assert_eq!(parse_format(Some("JPEG")).unwrap(), OutFormat::Jpg);
    }

    // ---- error paths -----------------------------------------------------

    #[test]
    fn rejects_an_unknown_color_with_guidance() {
        let e = parse_color(Some("chartreusey")).unwrap_err();
        assert!(e.contains("invalid color"), "{e}");
        assert!(e.contains("#1A2B3C"), "error should show the accepted form: {e}");
    }

    #[test]
    fn rejects_a_transparent_shadow_color_and_names_the_fix() {
        let e = parse_color(Some("transparent")).unwrap_err();
        assert!(e.contains("opacity=0"), "{e}");
        assert!(e.contains("background=transparent"), "{e}");
    }

    #[test]
    fn rejects_an_unknown_background() {
        let e = parse_background(Some("#12345")).unwrap_err();
        assert!(e.contains("invalid background"), "{e}");
    }

    #[test]
    fn rejects_an_unknown_format() {
        let e = parse_format(Some("tiff")).unwrap_err();
        assert!(e.contains("png|webp|jpg"), "{e}");
    }

    #[test]
    fn rejects_out_of_range_numbers() {
        let too_far = Shadow { offset_x: 501.0, ..Shadow::default() };
        assert!(filter(&too_far).unwrap_err().contains("offset_x"));
        let too_blurry = Shadow { blur: 401.0, ..Shadow::default() };
        assert!(filter(&too_blurry).unwrap_err().contains("blur"));
        let negative_blur = Shadow { blur: -1.0, ..Shadow::default() };
        assert!(filter(&negative_blur).unwrap_err().contains("blur"));
        let bad_opacity = Shadow { opacity: 120.0, ..Shadow::default() };
        assert!(filter(&bad_opacity).unwrap_err().contains("opacity"));
        let bad_padding = Shadow { padding: 5000.0, ..Shadow::default() };
        assert!(filter(&bad_padding).unwrap_err().contains("padding"));
        let nan = Shadow { offset_y: f64::NAN, ..Shadow::default() };
        assert!(filter(&nan).unwrap_err().contains("offset_y"));
    }
}
