//! gizza-ai/video-rounded-corners core — pure ffmpeg argv construction shared by
//! the chat skill block, the CLI, and the standalone web page. No wafer/
//! wasm-bindgen deps.
//!
//! Rounds the four corners of every frame of a video, turning a plain clip into
//! the card-style rounded rectangle used in app demos, UI mockups and social
//! embeds. The corner cut-outs are either TRANSPARENT (an alpha-capable output:
//! VP9/WebM or ProRes 4444/MOV) or filled with a solid background color (any
//! output, including H.264/MP4).
//!
//! How it works: a per-pixel alpha mask is built with ffmpeg's `geq` filter.
//! For a radius `R`, the horizontal/vertical distances INTO the corner box are
//!
//! ```text
//! dx = max(max(R-X, X-(W-1-R)), 0)      dy = max(max(R-Y, Y-(H-1-R)), 0)
//! alpha = clip((R - hypot(dx, dy) + 0.5) * 255, 0, 255)
//! ```
//!
//! so a pixel outside the quarter-circle gets alpha 0, one inside gets 255, and
//! the half-pixel term gives a 1px anti-aliased edge. Restricting `dx`/`dy` to
//! one side of the frame rounds only the top/bottom/left/right pair of corners.
//! `W`/`H` are geq's plane dimensions, so the mask adapts to ANY input size and
//! a percentage radius works without probing the video first.
//!
//! When a solid background is chosen the same mask is composited over a
//! `drawbox`-filled copy of the frame (`t=fill` paints the whole frame in the
//! chosen color at the input's own size), so no second input file is needed.
//!
//! Colors are validated through [`gizza_ai_block_utils::normalize_ffmpeg_color`]
//! (CSS name table or `0xRRGGBB`), so the only user-supplied value that ever
//! reaches the filter string is a known-safe token.

use gizza_ai_block_utils::normalize_ffmpeg_color;

/// Which corners get rounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corners {
    /// All four corners (default).
    All,
    /// Top-left + top-right only.
    Top,
    /// Bottom-left + bottom-right only.
    Bottom,
    /// Top-left + bottom-left only.
    Left,
    /// Top-right + bottom-right only.
    Right,
}

/// Parse the user-facing `corners` value (blank → `all`).
pub fn parse_corners(s: &str) -> Result<Corners, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "all" => Ok(Corners::All),
        "top" => Ok(Corners::Top),
        "bottom" => Ok(Corners::Bottom),
        "left" => Ok(Corners::Left),
        "right" => Ok(Corners::Right),
        other => Err(format!(
            "corners {other:?} not supported (all|top|bottom|left|right)"
        )),
    }
}

/// How `radius` is measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// Absolute pixels.
    Px,
    /// Percent of the SHORTER side (50 = fully rounded ends).
    Percent,
}

/// Parse the user-facing `radius_unit` value (blank → `px`).
pub fn parse_unit(s: &str) -> Result<Unit, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "px" | "pixels" => Ok(Unit::Px),
        "percent" | "%" | "pct" => Ok(Unit::Percent),
        other => Err(format!("radius_unit {other:?} not supported (px|percent)")),
    }
}

/// Output container/codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// VP9 in WebM — the only web-playable format that keeps real transparency.
    Webm,
    /// H.264 in MP4 — universal, but cannot store alpha (needs a background).
    Mp4,
    /// ProRes 4444 in MOV — alpha for editors (very large files).
    Mov,
}

impl Format {
    /// File extension this format writes (used for `out.<ext>`).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Webm => "webm",
            Format::Mp4 => "mp4",
            Format::Mov => "mov",
        }
    }

    /// Whether the format can carry a transparent (alpha) channel.
    pub fn supports_alpha(self) -> bool {
        matches!(self, Format::Webm | Format::Mov)
    }
}

/// Parse the user-facing `format` value (blank → `webm`).
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "webm" => Ok(Format::Webm),
        "mp4" => Ok(Format::Mp4),
        "mov" => Ok(Format::Mov),
        other => Err(format!("format {other:?} not supported (webm|mp4|mov)")),
    }
}

/// Largest accepted pixel radius.
pub const MAX_RADIUS_PX: f64 = 1000.0;
/// Largest accepted percentage radius (50% of the shorter side = fully rounded).
pub const MAX_RADIUS_PERCENT: f64 = 50.0;
/// Radius applied when the caller passes nothing (px).
pub const DEFAULT_RADIUS: f64 = 24.0;
/// Quality applied when the caller passes nothing.
pub const DEFAULT_QUALITY: u32 = 75;

/// Format a finite `f64` as a compact ffmpeg-friendly decimal: whole numbers
/// print without a fractional part (`40`), fractions round to 4 places with
/// trailing zeros trimmed (`0.125`).
fn num(v: f64) -> String {
    let r = (v * 10_000.0).round() / 10_000.0;
    let mut s = format!("{r:.4}");
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

/// Resolve the `background` value: blank / `transparent` / `none` → `None`
/// (keep the corners transparent), anything else → a normalized ffmpeg color
/// token (`0xRRGGBB` or a CSS name).
pub fn parse_background(s: &str) -> Result<Option<String>, String> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("transparent") || t.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    normalize_ffmpeg_color(t).map(Some)
}

/// Map web-conventional quality 1-100 to an encoder CRF.
/// H.264 uses 0-51, VP9 uses 0-63 (both: lower = better).
pub fn quality_to_crf(quality: u32, format: Format) -> u32 {
    let q = quality.clamp(1, 100) as f64;
    let top = match format {
        Format::Webm => 63.0,
        _ => 51.0,
    };
    let crf = top - (q - 1.0) * (top / 99.0);
    crf.round().clamp(0.0, top) as u32
}

/// The geq sub-expression for the corner radius, clamped so it can never exceed
/// half of the shorter side (a bigger request just yields fully rounded ends).
fn radius_expr(radius: f64, unit: Unit) -> String {
    match unit {
        Unit::Px => format!("min({},min(W,H)/2)", num(radius)),
        Unit::Percent => format!("{}*min(W,H)", num(radius / 100.0)),
    }
}

/// Build the geq alpha expression: 255 inside the rounded rectangle, 0 outside,
/// with a 1px anti-aliased edge. Only the selected corners are rounded.
pub fn alpha_expr(radius: f64, unit: Unit, corners: Corners) -> String {
    let r = radius_expr(radius, unit);
    let dx_both = format!("max(max({r}-X,X-(W-1-{r})),0)");
    let dy_both = format!("max(max({r}-Y,Y-(H-1-{r})),0)");
    let (dx, dy) = match corners {
        Corners::All => (dx_both, dy_both),
        Corners::Top => (dx_both, format!("max({r}-Y,0)")),
        Corners::Bottom => (dx_both, format!("max(Y-(H-1-{r}),0)")),
        Corners::Left => (format!("max({r}-X,0)"), dy_both),
        Corners::Right => (format!("max(X-(W-1-{r}),0)"), dy_both),
    };
    format!("clip(({r}-hypot({dx},{dy})+0.5)*255,0,255)")
}

/// Pixel format the filter chain (and the encoder) targets.
fn pix_fmt(format: Format, transparent: bool) -> &'static str {
    match (format, transparent) {
        (Format::Mov, true) => "yuva444p10le",
        (Format::Mov, false) => "yuv444p10le",
        (Format::Webm, true) => "yuva420p",
        _ => "yuv420p",
    }
}

/// Build the complete `-filter_complex` graph. `background = None` keeps the
/// corners transparent; `Some(color)` composites the rounded frame over a solid
/// fill of that color.
pub fn build_filter(
    radius: f64,
    unit: Unit,
    corners: Corners,
    background: Option<&str>,
    format: Format,
) -> String {
    let alpha = alpha_expr(radius, unit, corners);
    let pix = pix_fmt(format, background.is_none());
    // H.264 + yuv420p rejects odd dimensions; trim at most one row/column BEFORE
    // the mask so the rounded geometry matches the real output size.
    let pre = if format == Format::Mp4 {
        "crop=trunc(iw/2)*2:trunc(ih/2)*2,"
    } else {
        ""
    };
    let round = format!("format=rgba,geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':a='{alpha}'");
    match background {
        None => format!("[0:v]{pre}{round},format={pix}[v]"),
        Some(color) => format!(
            "[0:v]{pre}split[fgin][bgin];\
             [fgin]{round}[fg];\
             [bgin]drawbox=x=0:y=0:w=iw:h=ih:color={color}:t=fill,format=rgba[bg];\
             [bg][fg]overlay=0:0:format=auto,format={pix}[v]"
        ),
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) for an already-validated plan.
#[allow(clippy::too_many_arguments)]
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    radius: f64,
    unit: Unit,
    corners: Corners,
    background: Option<&str>,
    format: Format,
    quality: u32,
    keep_audio: bool,
) -> Vec<String> {
    let filter = build_filter(radius, unit, corners, background, format);
    let pix = pix_fmt(format, background.is_none());
    let mut argv: Vec<String> = vec![
        "-i".into(),
        in_name.into(),
        "-filter_complex".into(),
        filter,
        "-map".into(),
        "[v]".into(),
    ];
    if keep_audio {
        // `0:a?` keeps the audio track when there is one and stays silent when
        // the clip has none.
        argv.push("-map".into());
        argv.push("0:a?".into());
        argv.push("-c:a".into());
        argv.push(
            match format {
                Format::Webm => "libopus",
                Format::Mp4 => "aac",
                Format::Mov => "pcm_s16le",
            }
            .into(),
        );
    } else {
        argv.push("-an".into());
    }
    match format {
        Format::Webm => {
            argv.extend([
                "-c:v".into(),
                "libvpx-vp9".into(),
                "-pix_fmt".into(),
                pix.into(),
                "-crf".into(),
                quality_to_crf(quality, format).to_string(),
                "-b:v".into(),
                "0".into(),
            ]);
        }
        Format::Mp4 => {
            argv.extend([
                "-c:v".into(),
                "libx264".into(),
                "-pix_fmt".into(),
                pix.into(),
                "-preset".into(),
                "veryfast".into(),
                "-crf".into(),
                quality_to_crf(quality, format).to_string(),
                "-movflags".into(),
                "+faststart".into(),
            ]);
        }
        Format::Mov => {
            argv.extend([
                "-c:v".into(),
                "prores_ks".into(),
                "-profile:v".into(),
                "4444".into(),
                "-pix_fmt".into(),
                pix.into(),
            ]);
        }
    }
    argv.push(out_name.into());
    argv
}

/// Validate every user value, then build `(argv, out_name)`. Single source
/// shared by the chat block (`src/lib.rs`), the CLI and the web page
/// (`web/src/lib.rs`).
#[allow(clippy::too_many_arguments)]
pub fn plan(
    in_name: &str,
    radius: f64,
    radius_unit: &str,
    corners: &str,
    background: &str,
    format: &str,
    quality: u32,
    keep_audio: bool,
) -> Result<(Vec<String>, String), String> {
    let unit = parse_unit(radius_unit)?;
    let corners = parse_corners(corners)?;
    let fmt = parse_format(format)?;
    let bg = parse_background(background)?;

    if !radius.is_finite() || radius <= 0.0 {
        return Err(format!(
            "radius must be greater than 0, got {}",
            num(if radius.is_finite() { radius } else { 0.0 })
        ));
    }
    let cap = match unit {
        Unit::Px => MAX_RADIUS_PX,
        Unit::Percent => MAX_RADIUS_PERCENT,
    };
    if radius > cap {
        return Err(match unit {
            Unit::Px => format!("radius must be 1-{} px, got {}", num(cap), num(radius)),
            Unit::Percent => format!(
                "radius must be 1-{}% of the shorter side, got {}%",
                num(cap),
                num(radius)
            ),
        });
    }
    if !(1..=100).contains(&quality) {
        return Err(format!("quality must be 1-100, got {quality}"));
    }
    if bg.is_none() && !fmt.supports_alpha() {
        return Err(format!(
            "{} cannot store transparency — choose format webm or mov, or set a solid background color",
            fmt.ext()
        ));
    }

    let out_name = format!("out.{}", fmt.ext());
    let argv = build_argv(
        in_name,
        &out_name,
        radius,
        unit,
        corners,
        bg.as_deref(),
        fmt,
        quality,
        keep_audio,
    );
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter_of(argv: &[String]) -> String {
        let i = argv.iter().position(|a| a == "-filter_complex").unwrap();
        argv[i + 1].clone()
    }

    #[test]
    fn parses_enums_and_blank_defaults() {
        assert_eq!(parse_corners("").unwrap(), Corners::All);
        assert_eq!(parse_corners("Top").unwrap(), Corners::Top);
        assert_eq!(parse_unit("").unwrap(), Unit::Px);
        assert_eq!(parse_unit("percent").unwrap(), Unit::Percent);
        assert_eq!(parse_format("").unwrap(), Format::Webm);
        assert_eq!(parse_format("mov").unwrap(), Format::Mov);
    }

    #[test]
    fn rejects_unknown_enums() {
        assert!(parse_corners("top-left").is_err());
        assert!(parse_unit("em").is_err());
        assert!(parse_format("avi").is_err());
    }

    #[test]
    fn background_transparent_and_colors() {
        assert_eq!(parse_background("").unwrap(), None);
        assert_eq!(parse_background(" Transparent ").unwrap(), None);
        assert_eq!(parse_background("none").unwrap(), None);
        assert_eq!(parse_background("#f00").unwrap().unwrap(), "0xFF0000");
        assert_eq!(parse_background("#1E90FF").unwrap().unwrap(), "0x1E90FF");
        assert_eq!(parse_background("white").unwrap().unwrap(), "white");
        assert!(parse_background("chartruse").is_err());
    }

    #[test]
    fn alpha_expression_rounds_the_selected_corners() {
        let all = alpha_expr(40.0, Unit::Px, Corners::All);
        assert!(all.contains("min(40,min(W,H)/2)"), "{all}");
        assert!(all.contains("hypot("), "{all}");
        assert!(all.contains("X-(W-1-"), "{all}");
        assert!(all.contains("Y-(H-1-"), "{all}");

        // "top" must not reference the BOTTOM edge at all.
        let top = alpha_expr(40.0, Unit::Px, Corners::Top);
        assert!(!top.contains("Y-(H-1-"), "{top}");
        assert!(top.contains("X-(W-1-"), "{top}");

        // "left" must not reference the RIGHT edge at all.
        let left = alpha_expr(40.0, Unit::Px, Corners::Left);
        assert!(!left.contains("X-(W-1-"), "{left}");
        assert!(left.contains("Y-(H-1-"), "{left}");

        // percent radius scales off the shorter side, no px literal.
        let pct = alpha_expr(12.5, Unit::Percent, Corners::All);
        assert!(pct.contains("0.125*min(W,H)"), "{pct}");
    }

    #[test]
    fn transparent_webm_plan_is_exact() {
        let (argv, out) =
            plan("in.mp4", 40.0, "px", "all", "transparent", "webm", 75, true).unwrap();
        assert_eq!(out, "out.webm");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp4",
                "-filter_complex",
                "[0:v]format=rgba,geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':a='clip((min(40,min(W,H)/2)-hypot(max(max(min(40,min(W,H)/2)-X,X-(W-1-min(40,min(W,H)/2))),0),max(max(min(40,min(W,H)/2)-Y,Y-(H-1-min(40,min(W,H)/2))),0))+0.5)*255,0,255)',format=yuva420p[v]",
                "-map",
                "[v]",
                "-map",
                "0:a?",
                "-c:a",
                "libopus",
                "-c:v",
                "libvpx-vp9",
                "-pix_fmt",
                "yuva420p",
                "-crf",
                "16",
                "-b:v",
                "0",
                "out.webm",
            ]
        );
    }

    #[test]
    fn solid_background_composites_over_a_drawbox_fill() {
        let (argv, out) =
            plan("clip.webm", 24.0, "px", "all", "#1E90FF", "mp4", 75, false).unwrap();
        assert_eq!(out, "out.mp4");
        let f = filter_of(&argv);
        assert!(
            f.starts_with("[0:v]crop=trunc(iw/2)*2:trunc(ih/2)*2,split[fgin][bgin];"),
            "{f}"
        );
        assert!(
            f.contains("drawbox=x=0:y=0:w=iw:h=ih:color=0x1E90FF:t=fill"),
            "{f}"
        );
        assert!(
            f.contains("[bg][fg]overlay=0:0:format=auto,format=yuv420p[v]"),
            "{f}"
        );
        assert!(argv.iter().any(|a| a == "libx264"));
        assert!(argv.iter().any(|a| a == "-an"));
        assert!(argv.windows(2).any(|w| w[0] == "-crf" && w[1] == "13"));
        assert!(argv.iter().any(|a| a == "+faststart"));
    }

    #[test]
    fn mov_uses_prores_4444_with_alpha() {
        let (argv, out) = plan("in.mov", 10.0, "percent", "top", "", "mov", 75, true).unwrap();
        assert_eq!(out, "out.mov");
        assert!(argv.iter().any(|a| a == "prores_ks"));
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-profile:v" && w[1] == "4444"));
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-pix_fmt" && w[1] == "yuva444p10le"));
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-c:a" && w[1] == "pcm_s16le"));
    }

    #[test]
    fn webm_with_a_solid_background_drops_the_alpha_pix_fmt() {
        let (argv, _) = plan("in.mp4", 30.0, "px", "all", "black", "webm", 50, true).unwrap();
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-pix_fmt" && w[1] == "yuv420p"));
        assert!(argv.iter().any(|a| a == "libvpx-vp9"));
    }

    #[test]
    fn mp4_rejects_a_transparent_background() {
        let err = plan("in.mp4", 24.0, "px", "all", "transparent", "mp4", 75, true).unwrap_err();
        assert!(err.contains("cannot store transparency"), "{err}");
        assert!(err.contains("webm"), "{err}");
    }

    #[test]
    fn rejects_bad_radius_and_quality() {
        assert!(plan("in.mp4", 0.0, "px", "all", "", "webm", 75, true)
            .unwrap_err()
            .contains("greater than 0"));
        assert!(plan("in.mp4", -5.0, "px", "all", "", "webm", 75, true).is_err());
        assert!(plan("in.mp4", 1001.0, "px", "all", "", "webm", 75, true)
            .unwrap_err()
            .contains("1-1000 px"));
        assert!(plan("in.mp4", 51.0, "percent", "all", "", "webm", 75, true)
            .unwrap_err()
            .contains("1-50%"));
        assert!(plan("in.mp4", 24.0, "px", "all", "", "webm", 0, true)
            .unwrap_err()
            .contains("quality must be 1-100"));
        assert!(plan("in.mp4", 24.0, "px", "all", "", "webm", 101, true).is_err());
    }

    #[test]
    fn boundary_radii_are_accepted() {
        assert!(plan("in.mp4", MAX_RADIUS_PX, "px", "all", "", "webm", 75, true).is_ok());
        assert!(plan(
            "in.mp4",
            MAX_RADIUS_PERCENT,
            "percent",
            "all",
            "",
            "webm",
            75,
            true
        )
        .is_ok());
        assert!(plan("in.mp4", 24.0, "px", "all", "", "webm", 1, true).is_ok());
        assert!(plan("in.mp4", 24.0, "px", "all", "", "webm", 100, true).is_ok());
    }

    #[test]
    fn quality_maps_to_per_codec_crf_ranges() {
        assert_eq!(quality_to_crf(100, Format::Webm), 0);
        assert_eq!(quality_to_crf(1, Format::Webm), 63);
        assert_eq!(quality_to_crf(100, Format::Mp4), 0);
        assert_eq!(quality_to_crf(1, Format::Mp4), 51);
    }

    #[test]
    fn every_corner_choice_builds_a_filter() {
        for c in ["all", "top", "bottom", "left", "right"] {
            let (argv, _) = plan("in.mp4", 32.0, "px", c, "", "webm", 75, true).unwrap();
            let f = filter_of(&argv);
            assert!(f.contains("geq="), "{c}: {f}");
        }
    }
}
