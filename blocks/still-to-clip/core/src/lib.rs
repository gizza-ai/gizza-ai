//! gizza-ai/still-to-clip core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Turns a SINGLE still image into a fixed-duration STATIC clip: the image2
//! demuxer holds the picture (`-loop 1 -framerate FPS`), `-t` fixes the length,
//! and one scale/pad/crop chain puts it in the requested frame. Nothing moves —
//! deliberately, so the output is a clean "hold" you can drop into a timeline.
//! (Pan/zoom motion is `video-ken-burns`' job.)

/// How the source picture is placed inside the output frame.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Fit {
    /// Fit the whole image inside WxH, padding the leftover with `background`.
    Contain,
    /// Fill WxH completely, center-cropping whatever overflows.
    Cover,
    /// Force WxH exactly, distorting the aspect ratio.
    Stretch,
    /// Ignore WxH: keep the image's own size (snapped even, capped at 3840).
    Original,
}

pub fn parse_fit(s: &str) -> Result<Fit, String> {
    match s {
        "" | "contain" => Ok(Fit::Contain),
        "cover" => Ok(Fit::Cover),
        "stretch" => Ok(Fit::Stretch),
        "original" => Ok(Fit::Original),
        other => Err(format!(
            "invalid fit {other:?}; expected contain|cover|stretch|original"
        )),
    }
}

/// Output container. The codec follows from the container: mp4/mov → H.264,
/// webm → VP9. Both are in the browser ffmpeg build and play everywhere.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Mp4,
    Webm,
    Mov,
}

impl Format {
    pub fn ext(self) -> &'static str {
        match self {
            Format::Mp4 => "mp4",
            Format::Webm => "webm",
            Format::Mov => "mov",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Format::Mp4 => "video/mp4",
            Format::Webm => "video/webm",
            Format::Mov => "video/quicktime",
        }
    }
}

pub fn parse_format(s: &str) -> Result<Format, String> {
    match s {
        "" | "mp4" => Ok(Format::Mp4),
        "webm" => Ok(Format::Webm),
        "mov" => Ok(Format::Mov),
        other => Err(format!("invalid format {other:?}; expected mp4|webm|mov")),
    }
}

/// Normalize the user-facing padding color into something ffmpeg accepts
/// verbatim and that is safe to inject into a filtergraph: a lower-cased name
/// from ffmpeg's own table, or `#RRGGBB` / `#RGB` / bare hex → `0xRRGGBB`.
/// Blank means this tool's default, black. Every surface calls this before
/// [`plan`], which only sees an already-normalized value.
pub fn normalize_color(color: &str) -> Result<String, String> {
    if color.trim().is_empty() {
        return Ok(DEFAULT_BACKGROUND.to_string());
    }
    gizza_ai_block_utils::normalize_ffmpeg_color(color)
}

/// Defaults shared by every surface (chat, CLI, page) so a blank field means
/// the same thing everywhere.
pub const DEFAULT_DURATION: f64 = 5.0;
pub const DEFAULT_WIDTH: u32 = 1920;
pub const DEFAULT_HEIGHT: u32 = 1080;
pub const DEFAULT_FIT: &str = "contain";
pub const DEFAULT_BACKGROUND: &str = "black";
pub const DEFAULT_FPS: f64 = 30.0;
pub const DEFAULT_FORMAT: &str = "mp4";
pub const DEFAULT_QUALITY: u8 = 80;

/// Enum variant lists, single-sourced so the descriptor and the parsers agree.
pub const FITS: [&str; 4] = ["contain", "cover", "stretch", "original"];
pub const FORMATS: [&str; 3] = ["mp4", "webm", "mov"];

/// Output name is fixed per container so every surface agrees on it.
pub fn out_name_for(format: Format) -> String {
    format!("out.{}", format.ext())
}

/// Largest dimension we will encode. Also the cap applied to `fit = original`,
/// so a 12 MiB 8000px PNG can't turn into a browser-hanging 8K encode.
pub const MAX_DIM: u32 = 3840;

/// Map web-conventional quality 1-100 onto ffmpeg's CRF 0 (best) – 51 (worst).
/// Same curve as `video-transcode`, so "quality 80" means the same thing across
/// the toolkit.
pub fn quality_to_crf(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let crf = 51.0 - (q - 1.0) * (51.0 / 99.0);
    crf.round().clamp(0.0, 51.0) as u8
}

/// Format an f64 for an ffmpeg argument: round to 3 decimals and strip trailing
/// zeros so `5.0` becomes `5` and `2.5` stays `2.5`.
fn fnum(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Snap a dimension down to an even value (yuv420p/libx264 require even W/H),
/// clamped to a sane [16, MAX_DIM] range.
fn even_dim(v: u32) -> u32 {
    let v = v.clamp(16, MAX_DIM);
    v & !1
}

/// Build the `-vf` chain for a fit mode. `w`/`h` are already even.
/// `background` must already be normalized (a color name or `0xRRGGBB`).
fn build_vf(fit: Fit, w: u32, h: u32, background: &str) -> String {
    match fit {
        Fit::Contain => format!(
            "scale={w}:{h}:force_original_aspect_ratio=decrease,\
pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color={background},setsar=1"
        ),
        Fit::Cover => format!(
            "scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},setsar=1"
        ),
        Fit::Stretch => format!("scale={w}:{h},setsar=1"),
        // Downscale only if the picture is bigger than MAX_DIM, then snap to
        // even dimensions. Quoted expressions keep the inner commas out of the
        // filtergraph separator.
        Fit::Original => format!(
            "scale='min(iw,{MAX_DIM})':'min(ih,{MAX_DIM})':force_original_aspect_ratio=decrease,\
scale=trunc(iw/2)*2:trunc(ih/2)*2,setsar=1"
        ),
    }
}

/// Validate params and return `(argv, out_name)`.
///
/// `duration` seconds (0.1..=60); `width`/`height` the output size (snapped
/// even, 16..=MAX_DIM, ignored when `fit = original`); `fit` one of
/// contain|cover|stretch|original; `background` an already-normalized ffmpeg
/// color used for `contain` padding; `fps` frames per second (1..=60); `format`
/// mp4|webm|mov; `quality` 1..=100.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    duration: f64,
    width: u32,
    height: u32,
    fit: &str,
    background: &str,
    fps: f64,
    format: &str,
    quality: u8,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    let fit = parse_fit(fit)?;
    let fmt = parse_format(format)?;
    if !(0.1..=60.0).contains(&duration) {
        return Err(format!(
            "duration must be between 0.1 and 60 seconds (got {})",
            fnum(duration)
        ));
    }
    if !(1.0..=60.0).contains(&fps) {
        return Err(format!("fps must be between 1 and 60 (got {})", fnum(fps)));
    }
    if !(16..=MAX_DIM).contains(&width) || !(16..=MAX_DIM).contains(&height) {
        return Err(format!(
            "width and height must be between 16 and {MAX_DIM} pixels"
        ));
    }
    if !(1..=100).contains(&quality) {
        return Err(format!("quality must be between 1 and 100 (got {quality})"));
    }
    if background.trim().is_empty() {
        return Err("background color must not be empty".to_string());
    }

    let w = even_dim(width);
    let h = even_dim(height);
    let fps_s = fnum(fps);
    let crf = quality_to_crf(quality);
    let out_name = out_name_for(fmt);

    let mut argv: Vec<String> = vec![
        "-loop".into(),
        "1".into(),
        "-framerate".into(),
        fps_s.clone(),
        "-i".into(),
        in_name.into(),
        "-t".into(),
        fnum(duration),
        "-vf".into(),
        build_vf(fit, w, h, background),
        "-r".into(),
        fps_s,
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-crf".into(),
        crf.to_string(),
        "-c:v".into(),
    ];
    match fmt {
        Format::Mp4 | Format::Mov => {
            argv.push("libx264".into());
            // A static picture only needs one keyframe per couple of seconds;
            // the default GOP is fine, but faststart makes the file streamable.
            argv.push("-movflags".into());
            argv.push("+faststart".into());
        }
        Format::Webm => {
            argv.push("libvpx-vp9".into());
            // Constant-quality VP9 (CRF only) needs an explicit zero bitrate.
            argv.push("-b:v".into());
            argv.push("0".into());
        }
    }
    argv.push(out_name.clone());

    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vf_of(argv: &[String]) -> String {
        let i = argv.iter().position(|a| a == "-vf").expect("-vf present");
        argv[i + 1].clone()
    }

    fn val_after(argv: &[String], flag: &str) -> String {
        let i = argv.iter().position(|a| a == flag).expect("flag present");
        argv[i + 1].clone()
    }

    #[test]
    fn default_contain_plan_holds_the_image_for_the_requested_time() {
        let (argv, out) = plan(5.0, 1920, 1080, "contain", "black", 30.0, "mp4", 80, "in.png")
            .expect("valid plan");
        assert_eq!(out, "out.mp4");
        // The still is held by the image2 demuxer, not by a motion filter.
        assert_eq!(argv[0], "-loop");
        assert_eq!(argv[1], "1");
        assert_eq!(val_after(&argv, "-framerate"), "30");
        assert_eq!(val_after(&argv, "-t"), "5");
        assert_eq!(val_after(&argv, "-r"), "30");
        assert_eq!(val_after(&argv, "-c:v"), "libx264");
        assert_eq!(val_after(&argv, "-pix_fmt"), "yuv420p");
        assert_eq!(val_after(&argv, "-crf"), quality_to_crf(80).to_string());
        assert!(argv.iter().any(|a| a == "+faststart"));
        assert_eq!(argv.last().unwrap(), "out.mp4");
        let vf = vf_of(&argv);
        assert!(vf.contains("scale=1920:1080:force_original_aspect_ratio=decrease"));
        assert!(vf.contains("pad=1920:1080:(ow-iw)/2:(oh-ih)/2:color=black"));
        assert!(vf.ends_with("setsar=1"));
        // Nothing animates: no zoompan/loop-filter motion in the chain.
        assert!(!vf.contains("zoompan"));
    }

    #[test]
    fn cover_crops_and_stretch_forces_exact_size() {
        let (cover, _) = plan(2.0, 640, 480, "cover", "black", 24.0, "mp4", 80, "in.jpg").unwrap();
        let vf = vf_of(&cover);
        assert!(vf.contains("force_original_aspect_ratio=increase"));
        assert!(vf.contains("crop=640:480"));
        assert!(!vf.contains("pad="));

        let (stretch, _) =
            plan(2.0, 640, 480, "stretch", "black", 24.0, "mp4", 80, "in.jpg").unwrap();
        assert_eq!(vf_of(&stretch), "scale=640:480,setsar=1");
    }

    #[test]
    fn original_ignores_width_height_and_caps_at_max_dim() {
        let (argv, _) = plan(3.0, 640, 480, "original", "white", 30.0, "mp4", 80, "in.png").unwrap();
        let vf = vf_of(&argv);
        assert!(vf.contains(&format!("scale='min(iw,{MAX_DIM})':'min(ih,{MAX_DIM})'")));
        assert!(vf.contains("scale=trunc(iw/2)*2:trunc(ih/2)*2"));
        // The requested 640x480 must NOT leak into the chain.
        assert!(!vf.contains("640"));
    }

    #[test]
    fn odd_dimensions_snap_down_to_even() {
        let (argv, _) = plan(1.0, 641, 481, "contain", "black", 30.0, "mp4", 80, "in.png").unwrap();
        assert!(vf_of(&argv).contains("scale=640:480"));
    }

    #[test]
    fn webm_uses_vp9_constant_quality_and_mov_uses_h264() {
        let (webm, out) = plan(1.0, 320, 240, "contain", "black", 25.0, "webm", 60, "in.png")
            .expect("valid webm plan");
        assert_eq!(out, "out.webm");
        assert_eq!(val_after(&webm, "-c:v"), "libvpx-vp9");
        assert_eq!(val_after(&webm, "-b:v"), "0");
        assert!(!webm.iter().any(|a| a == "+faststart"));

        let (mov, out) = plan(1.0, 320, 240, "contain", "black", 25.0, "mov", 60, "in.png").unwrap();
        assert_eq!(out, "out.mov");
        assert_eq!(val_after(&mov, "-c:v"), "libx264");
    }

    #[test]
    fn hex_background_reaches_the_pad_filter() {
        let (argv, _) =
            plan(1.0, 320, 240, "contain", "0x1A2B3C", 30.0, "mp4", 80, "in.png").unwrap();
        assert!(vf_of(&argv).contains("color=0x1A2B3C"));
    }

    #[test]
    fn quality_maps_to_the_expected_crf_range() {
        assert_eq!(quality_to_crf(100), 0);
        assert_eq!(quality_to_crf(1), 51);
        assert_eq!(quality_to_crf(80), 10);
    }

    #[test]
    fn invalid_params_are_rejected_with_a_message() {
        assert!(plan(0.0, 320, 240, "contain", "black", 30.0, "mp4", 80, "in.png")
            .unwrap_err()
            .contains("duration"));
        assert!(plan(61.0, 320, 240, "contain", "black", 30.0, "mp4", 80, "in.png").is_err());
        assert!(plan(5.0, 320, 240, "contain", "black", 0.0, "mp4", 80, "in.png")
            .unwrap_err()
            .contains("fps"));
        assert!(plan(5.0, 8, 240, "contain", "black", 30.0, "mp4", 80, "in.png")
            .unwrap_err()
            .contains("width and height"));
        assert!(plan(5.0, 320, 240, "contain", "black", 30.0, "mp4", 0, "in.png")
            .unwrap_err()
            .contains("quality"));
        assert!(plan(5.0, 320, 240, "wobble", "black", 30.0, "mp4", 80, "in.png")
            .unwrap_err()
            .contains("invalid fit"));
        assert!(plan(5.0, 320, 240, "contain", "black", 30.0, "avi", 80, "in.png")
            .unwrap_err()
            .contains("invalid format"));
        assert!(plan(5.0, 320, 240, "contain", "  ", 30.0, "mp4", 80, "in.png")
            .unwrap_err()
            .contains("background"));
    }

    #[test]
    fn blank_fit_and_format_fall_back_to_the_defaults() {
        assert_eq!(parse_fit("").unwrap(), Fit::Contain);
        assert_eq!(parse_format("").unwrap(), Format::Mp4);
        assert_eq!(Format::Webm.mime(), "video/webm");
        assert_eq!(out_name_for(Format::Mov), "out.mov");
    }

    #[test]
    fn background_normalizes_names_and_hex_and_defaults_to_black() {
        assert_eq!(normalize_color("").unwrap(), "black");
        assert_eq!(normalize_color("   ").unwrap(), "black");
        assert_eq!(normalize_color("White").unwrap(), "white");
        assert_eq!(normalize_color("#1a2b3c").unwrap(), "0x1A2B3C");
        assert_eq!(normalize_color("#fff").unwrap(), "0xFFFFFF");
        assert!(normalize_color("not-a-color").is_err());
        // A normalized color round-trips into the pad filter unchanged.
        let (argv, _) = plan(
            1.0,
            320,
            240,
            "contain",
            &normalize_color("#fff").unwrap(),
            30.0,
            "mp4",
            80,
            "in.png",
        )
        .unwrap();
        assert!(vf_of(&argv).contains("color=0xFFFFFF"));
    }

    #[test]
    fn declared_defaults_produce_a_valid_plan() {
        let (argv, out) = plan(
            DEFAULT_DURATION,
            DEFAULT_WIDTH,
            DEFAULT_HEIGHT,
            DEFAULT_FIT,
            DEFAULT_BACKGROUND,
            DEFAULT_FPS,
            DEFAULT_FORMAT,
            DEFAULT_QUALITY,
            "in.png",
        )
        .expect("defaults are a valid plan");
        assert_eq!(out, "out.mp4");
        assert!(vf_of(&argv).contains("scale=1920:1080"));
        // Every advertised enum variant is actually accepted.
        for fit in FITS {
            assert!(parse_fit(fit).is_ok(), "fit {fit} must parse");
        }
        for fmt in FORMATS {
            assert!(parse_format(fmt).is_ok(), "format {fmt} must parse");
        }
    }
}
