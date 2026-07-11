//! gizza-ai/audio-waveform-video core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Turns an audio track into an MP4 waveform-animation VIDEO (audio in, video
//! out) via ffmpeg's `showwaves` filter, with the original audio muxed back in
//! so the wave is perfectly in sync. Single ffmpeg input, single MP4 output.
//!
//! The wave is drawn over an opaque `color=` background (a video frame has no
//! alpha the way `showwavespic` PNGs do, so a solid backdrop is always
//! composited): `color=c=<bg>:s=WxH[bg];…[wave];[bg][wave]overlay,format=yuv420p`.
//! The audio is downmixed to mono first (`aformat=channel_layouts=mono`) so a
//! stereo track renders ONE clean wave instead of two channels blended on top of
//! each other — the audiogram/soundwave-video look.
//!
//! `mode` selects the showwaves draw style (mirror = centered, bars, wave,
//! points). `scale` (lin|sqrt|cbrt|log) boosts quiet material so the wave keeps
//! moving. `fps` sets the video frame rate.
//!
//! `color2` turns the wave into a horizontal left→right gradient (same recipe as
//! the waveform-image block, adapted per-frame): the wave is drawn white, its
//! alpha plane is lifted out as a mask, and the mask is `alphamerge`d onto a
//! `gradients` source running at the same rate.
//!
//! Color strings are interpolated into `-filter_complex`, so they are validated
//! to strict hex (`#RGB`/`#RGBA`/`#RRGGBB`/`#RRGGBBAA`) — input hygiene that
//! doubles as filtergraph hardening (a stray `,`/`;`/`:` in a color would
//! rewrite the graph). Short forms are EXPANDED to the 6/8-digit form ffmpeg's
//! color parser understands (a 3-digit hex is silently dropped → white wave).

/// Default output width — 1280×720 (720p 16:9), the common social-video shape.
pub const DEFAULT_WIDTH: u32 = 1280;
/// Default output height.
pub const DEFAULT_HEIGHT: u32 = 720;
/// Default wave color (the site accent).
pub const DEFAULT_COLOR: &str = "#4f46e5";
/// Default background — a near-black slate that reads well behind the accent.
pub const DEFAULT_BACKGROUND: &str = "#101014";
/// Default video frame rate.
pub const DEFAULT_FPS: u32 = 25;

/// Inclusive dimension bounds. ffmpeg needs a few pixels to draw anything; the
/// caps keep the canvas (and the encoder's memory) sane (up to 4K).
pub const MIN_DIM: u32 = 64;
pub const MAX_WIDTH: u32 = 3840;
pub const MAX_HEIGHT: u32 = 2160;

/// Inclusive frame-rate bounds.
pub const MIN_FPS: u32 = 5;
pub const MAX_FPS: u32 = 60;

/// showwaves draw modes, keyed by the friendly names the descriptor exposes.
/// `mirror` = centered line (symmetric about the midline), `bars` = a vertical
/// line per sample from the baseline, `wave` = a line joining consecutive
/// samples, `points` = a dot per sample.
pub const MODES: [(&str, &str); 4] = [
    ("mirror", "cline"),
    ("bars", "line"),
    ("wave", "p2p"),
    ("points", "point"),
];

/// Amplitude scales showwaves accepts. `lin` is the true waveform; sqrt/cbrt/log
/// progressively boost quiet material so it stays visible / keeps moving.
pub const SCALES: [&str; 4] = ["lin", "sqrt", "cbrt", "log"];

/// Validate a strict hex color (`#RGB`/`#RGBA`/`#RRGGBB`/`#RRGGBBAA`,
/// case-insensitive) and NORMALIZE it to the 6/8-digit form ffmpeg's color
/// parser understands (a 3-digit hex is silently dropped → white). Strictness
/// doubles as filtergraph hardening.
fn parse_hex_color(value: &str, field: &str) -> Result<String, String> {
    let v = value.trim();
    let digits = v.strip_prefix('#').unwrap_or("");
    let ok = matches!(digits.len(), 3 | 4 | 6 | 8)
        && digits.bytes().all(|b| b.is_ascii_hexdigit())
        && v.starts_with('#');
    if !ok {
        return Err(format!(
            "{field} must be a hex color like #4f46e5, #f00 or #00000080, got {value:?}"
        ));
    }
    if digits.len() <= 4 {
        let mut out = String::with_capacity(1 + digits.len() * 2);
        out.push('#');
        for c in digits.chars() {
            out.push(c);
            out.push(c);
        }
        Ok(out)
    } else {
        Ok(v.to_string())
    }
}

/// Round + range-check a dimension. `0` (an empty page field) selects the
/// default; anything else must land in `[MIN_DIM, max]` after rounding.
fn parse_dim(value: f64, default: u32, max: u32, field: &str) -> Result<u32, String> {
    if value == 0.0 {
        return Ok(default);
    }
    if !value.is_finite() {
        return Err(format!("{field} must be a number of pixels, got {value}"));
    }
    let rounded = value.round();
    if rounded < MIN_DIM as f64 || rounded > max as f64 {
        return Err(format!(
            "{field} must be between {MIN_DIM} and {max} pixels (0 or empty = {default}), got {value}"
        ));
    }
    Ok(rounded as u32)
}

/// Round + range-check the frame rate. `0` (empty field) selects the default.
fn parse_fps(value: f64) -> Result<u32, String> {
    if value == 0.0 {
        return Ok(DEFAULT_FPS);
    }
    if !value.is_finite() {
        return Err(format!("fps must be a number, got {value}"));
    }
    let rounded = value.round();
    if rounded < MIN_FPS as f64 || rounded > MAX_FPS as f64 {
        return Err(format!(
            "fps must be between {MIN_FPS} and {MAX_FPS} (0 or empty = {DEFAULT_FPS}), got {value}"
        ));
    }
    Ok(rounded as u32)
}

/// Map a friendly `mode` name to the showwaves keyword. Empty defaults to
/// `mirror` (cline).
pub fn parse_mode(s: &str) -> Result<&'static str, String> {
    let v = s.trim().to_ascii_lowercase();
    if v.is_empty() {
        return Ok("cline");
    }
    MODES
        .iter()
        .find(|(name, _)| *name == v)
        .map(|(_, kw)| *kw)
        .ok_or_else(|| format!("mode {s:?} not supported (mirror|bars|wave|points)"))
}

/// Parse the amplitude scale. Empty defaults to `lin`.
pub fn parse_scale(s: &str) -> Result<&'static str, String> {
    let v = s.trim().to_ascii_lowercase();
    if v.is_empty() {
        return Ok("lin");
    }
    SCALES
        .iter()
        .find(|k| **k == v)
        .copied()
        .ok_or_else(|| format!("scale {s:?} not supported (lin|sqrt|cbrt|log)"))
}

/// Build the `-filter_complex` graph. Always composites the wave over an opaque
/// `color=` background (video has no alpha) and finishes with `format=yuv420p`
/// for universal playback. `gradient` = `Some((from, to))` fills the wave with a
/// horizontal color→color2 gradient instead of a solid color.
fn build_graph(
    width: u32,
    height: u32,
    color: &str,
    gradient: Option<(&str, &str)>,
    background: &str,
    mode: &str,
    scale: &str,
    fps: u32,
) -> String {
    let wave_color = if gradient.is_some() { "#ffffff" } else { color };
    let wave = format!(
        "[0:a]aformat=channel_layouts=mono,showwaves=s={width}x{height}:mode={mode}:colors={wave_color}:rate={fps}:scale={scale}"
    );
    match gradient {
        None => format!(
            "color=c={background}:s={width}x{height}[bg];{wave}[w];[bg][w]overlay=format=auto,format=yuv420p[v]"
        ),
        Some((from, to)) => {
            let mid = height / 2;
            format!(
                "color=c={background}:s={width}x{height}[bg];\
                 gradients=s={width}x{height}:c0={from}:c1={to}:x0=0:y0={mid}:x1={width}:y1={mid}:rate={fps}[grad];\
                 {wave}[w];[w]alphaextract[mask];[grad][mask]alphamerge[gw];\
                 [bg][gw]overlay=format=auto,format=yuv420p[v]"
            )
        }
    }
}

/// Validate params and return `(argv, out_name)`. Width/height/fps arrive as f64
/// (the page sends 0 for an empty field → default); `color` is one hex (empty →
/// default accent); `color2` is an optional gradient end color (empty → solid);
/// `background` is a hex (empty → default slate); `mode` is
/// mirror|bars|wave|points (empty → mirror); `scale` is lin|sqrt|cbrt|log
/// (empty → lin). All hex colors accept `#RGB`/`#RGBA`/`#RRGGBB`/`#RRGGBBAA` and
/// are normalized for ffmpeg. Single source shared by the chat block
/// (`src/lib.rs`) and the web page (`web/src/lib.rs`). Output is always
/// `out.mp4` (H.264 video + AAC audio muxed from the source).
#[allow(clippy::too_many_arguments)] // mirrors the page field order 1:1
pub fn plan_audio_waveform_video(
    in_name: &str,
    out_name: &str,
    mode: &str,
    width: f64,
    height: f64,
    color: &str,
    color2: &str,
    background: &str,
    scale: &str,
    fps: f64,
) -> Result<(Vec<String>, String), String> {
    let mode = parse_mode(mode)?;
    let w = parse_dim(width, DEFAULT_WIDTH, MAX_WIDTH, "width")?;
    let h = parse_dim(height, DEFAULT_HEIGHT, MAX_HEIGHT, "height")?;
    let color = if color.trim().is_empty() {
        DEFAULT_COLOR.to_string()
    } else {
        parse_hex_color(color, "color")?
    };
    let gradient_to = if color2.trim().is_empty() {
        None
    } else {
        Some(parse_hex_color(color2, "color2")?)
    };
    let background = if background.trim().is_empty() {
        DEFAULT_BACKGROUND.to_string()
    } else {
        parse_hex_color(background, "background")?
    };
    let scale = parse_scale(scale)?;
    let fps = parse_fps(fps)?;
    let gradient = gradient_to.as_deref().map(|to| (color.as_str(), to));
    let graph = build_graph(w, h, &color, gradient, &background, mode, scale, fps);

    let argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-filter_complex".to_string(),
        graph,
        "-map".to_string(),
        "[v]".to_string(),
        "-map".to_string(),
        "0:a".to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-shortest".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        out_name.to_string(),
    ];
    Ok((argv, out_name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(
        mode: &str,
        width: f64,
        height: f64,
        color: &str,
        color2: &str,
        background: &str,
        scale: &str,
        fps: f64,
    ) -> Result<(Vec<String>, String), String> {
        plan_audio_waveform_video(
            "in.mp3", "out.mp4", mode, width, height, color, color2, background, scale, fps,
        )
    }

    #[test]
    fn default_argv_is_mono_mirror_1280x720_accent_on_slate() {
        let (argv, out) = plan("", 0.0, 0.0, "", "", "", "", 0.0).unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-filter_complex",
                "color=c=#101014:s=1280x720[bg];\
                 [0:a]aformat=channel_layouts=mono,showwaves=s=1280x720:mode=cline:colors=#4f46e5:rate=25:scale=lin[w];\
                 [bg][w]overlay=format=auto,format=yuv420p[v]",
                "-map",
                "[v]",
                "-map",
                "0:a",
                "-c:v",
                "libx264",
                "-c:a",
                "aac",
                "-shortest",
                "-movflags",
                "+faststart",
                "out.mp4",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn modes_map_to_showwaves_keywords() {
        assert_eq!(parse_mode("").unwrap(), "cline");
        assert_eq!(parse_mode("MIRROR").unwrap(), "cline");
        assert_eq!(parse_mode("bars").unwrap(), "line");
        assert_eq!(parse_mode(" wave ").unwrap(), "p2p");
        assert_eq!(parse_mode("points").unwrap(), "point");
        assert!(parse_mode("circular").is_err());
    }

    #[test]
    fn bars_mode_and_custom_size_color_bg_fps() {
        let (argv, _) = plan("bars", 640.0, 200.0, "#FF0000", "", "#000000", "sqrt", 30.0).unwrap();
        assert_eq!(
            argv[3],
            "color=c=#000000:s=640x200[bg];\
             [0:a]aformat=channel_layouts=mono,showwaves=s=640x200:mode=line:colors=#FF0000:rate=30:scale=sqrt[w];\
             [bg][w]overlay=format=auto,format=yuv420p[v]"
        );
    }

    #[test]
    fn scale_variants_accepted_and_defaulted() {
        assert_eq!(parse_scale("").unwrap(), "lin");
        assert_eq!(parse_scale("LOG").unwrap(), "log");
        assert_eq!(parse_scale(" cbrt ").unwrap(), "cbrt");
        assert!(parse_scale("linear").is_err());
    }

    #[test]
    fn gradient_wave_uses_the_alphamerge_recipe() {
        let (argv, _) = plan("mirror", 320.0, 100.0, "#ff0000", "#0000ff", "", "", 25.0).unwrap();
        assert_eq!(
            argv[3],
            "color=c=#101014:s=320x100[bg];\
             gradients=s=320x100:c0=#ff0000:c1=#0000ff:x0=0:y0=50:x1=320:y1=50:rate=25[grad];\
             [0:a]aformat=channel_layouts=mono,showwaves=s=320x100:mode=cline:colors=#ffffff:rate=25:scale=lin[w];\
             [w]alphaextract[mask];[grad][mask]alphamerge[gw];\
             [bg][gw]overlay=format=auto,format=yuv420p[v]"
        );
    }

    /// REGRESSION guard (from waveform-image): ffmpeg's color parser only
    /// understands `#RRGGBB[AA]` — a raw `#f00` is silently replaced with WHITE.
    /// Short forms must be expanded before interpolation.
    #[test]
    fn three_and_four_digit_hex_expand_to_ffmpeg_form() {
        let (argv, _) = plan("mirror", 0.0, 0.0, "#f00", "", "#ABC", "", 0.0).unwrap();
        assert!(argv[3].contains("colors=#ff0000"), "{}", argv[3]);
        assert!(argv[3].contains("color=c=#AABBCC"), "{}", argv[3]);
        let (argv, _) = plan("mirror", 0.0, 0.0, "#f008", "", "", "", 0.0).unwrap();
        assert!(argv[3].contains("colors=#ff000088"), "{}", argv[3]);
    }

    #[test]
    fn six_and_eight_digit_hex_keep_their_case_and_value() {
        let (argv, _) = plan("mirror", 0.0, 0.0, "#ABCdef", "", "#0b1220", "", 0.0).unwrap();
        assert!(argv[3].contains("colors=#ABCdef"), "{}", argv[3]);
        assert!(argv[3].contains("color=c=#0b1220"), "{}", argv[3]);
    }

    #[test]
    fn hex_validation_blocks_filtergraph_injection() {
        for evil in ["#fff;movie=x", "#fff:s=9x9", "white", "#fff|#000", "#fff,anullsink"] {
            assert!(
                plan("mirror", 0.0, 0.0, evil, "", "", "", 0.0).is_err(),
                "{evil} should be rejected as color"
            );
            assert!(
                plan("mirror", 0.0, 0.0, "", "", evil, "", 0.0).is_err(),
                "{evil} should be rejected as background"
            );
            assert!(
                plan("mirror", 0.0, 0.0, "#fff", evil, "", "", 0.0).is_err(),
                "{evil} should be rejected as color2"
            );
        }
    }

    #[test]
    fn zero_or_empty_dimensions_and_fps_take_the_defaults() {
        let (argv, _) = plan("mirror", 0.0, 480.0, "", "", "", "", 0.0).unwrap();
        assert!(argv[3].contains("s=1280x480"), "{}", argv[3]);
        let (argv, _) = plan("mirror", 800.0, 0.0, "", "", "", "", 15.0).unwrap();
        assert!(argv[3].contains("s=800x720"), "{}", argv[3]);
        assert!(argv[3].contains("rate=15"), "{}", argv[3]);
    }

    #[test]
    fn fractional_dimensions_and_fps_are_rounded() {
        let (argv, _) = plan("mirror", 799.6, 200.4, "", "", "", "", 29.7).unwrap();
        assert!(argv[3].contains("s=800x200"), "{}", argv[3]);
        assert!(argv[3].contains("rate=30"), "{}", argv[3]);
    }

    #[test]
    fn rejects_out_of_range_dimensions_and_fps_with_bounds_in_message() {
        let err = plan("mirror", 3841.0, 720.0, "", "", "", "", 0.0).unwrap_err();
        assert!(err.contains("width") && err.contains("3840"), "{err}");
        let err = plan("mirror", 1280.0, 2161.0, "", "", "", "", 0.0).unwrap_err();
        assert!(err.contains("height") && err.contains("2160"), "{err}");
        let err = plan("mirror", 32.0, 720.0, "", "", "", "", 0.0).unwrap_err();
        assert!(err.contains("64"), "{err}");
        let err = plan("mirror", 0.0, 0.0, "", "", "", "", 120.0).unwrap_err();
        assert!(err.contains("fps") && err.contains("60"), "{err}");
        assert!(plan("mirror", f64::NAN, 0.0, "", "", "", "", 0.0).is_err());
        assert!(plan("mirror", -100.0, 0.0, "", "", "", "", 0.0).is_err());
    }

    #[test]
    fn bounds_are_inclusive() {
        assert!(plan("mirror", 64.0, 64.0, "", "", "", "", 5.0).is_ok());
        assert!(plan("mirror", 3840.0, 2160.0, "", "", "", "", 60.0).is_ok());
    }

    #[test]
    fn rejects_bad_hex_colors_with_field_name() {
        let err = plan("mirror", 0.0, 0.0, "red", "", "", "", 0.0).unwrap_err();
        assert!(err.contains("color") && err.contains("#4f46e5"), "{err}");
        let err = plan("mirror", 0.0, 0.0, "", "", "#12345", "", 0.0).unwrap_err();
        assert!(err.contains("background"), "{err}");
        let err = plan("mirror", 0.0, 0.0, "#f00", "nope", "", "", 0.0).unwrap_err();
        assert!(err.contains("color2"), "{err}");
    }

    #[test]
    fn out_name_is_always_mp4_and_argv_muxes_source_audio() {
        let (argv, out) =
            plan_audio_waveform_video("song.flac", "out.mp4", "", 0.0, 0.0, "", "", "", "", 0.0)
                .unwrap();
        assert_eq!(out, "out.mp4");
        // -map [v] (generated video) + -map 0:a (source audio) → synced output.
        let joined = argv.join(" ");
        assert!(joined.contains("-map [v] -map 0:a"), "{joined}");
        assert!(joined.contains("-c:a aac"), "{joined}");
        assert!(joined.contains("-shortest"), "{joined}");
    }
}
