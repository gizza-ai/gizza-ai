//! gizza-ai/video-ken-burns core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Turns a SINGLE still image into a short MP4 with a smooth pan-and-zoom (Ken
//! Burns) move. One ffmpeg `zoompan` pass: the source is pre-scaled to COVER the
//! output box at a 2× supersample (which suppresses zoompan's integer-grid
//! jitter), then `zoompan` walks a linear, `on`-driven z/x/y path — linear in the
//! output frame number `on`, not the drifty `min(zoom+step,max)` accumulator, so
//! the start/end framing is exact. `d = round(duration*fps)` frames are produced
//! from the one input picture, scaled to `s = WxH`, and encoded H.264
//! (`libx264 -pix_fmt yuv420p +faststart`) for universal playback.

/// Output is always an MP4 (H.264) regardless of the input image format.
pub const OUT_NAME: &str = "out.mp4";

/// Supersample factor for the pre-scale. The image is scaled to cover
/// `SUPERSAMPLE*W x SUPERSAMPLE*H` before zoompan crops/zooms it, so movement
/// lands on a finer grid than the output pixels (much less jitter) and the crop
/// window stays >= WxH for zoom factors up to `SUPERSAMPLE` (no upscaling blur).
const SUPERSAMPLE: u32 = 2;

/// The six supported Ken Burns moves.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Direction {
    ZoomIn,
    ZoomOut,
    PanLeft,
    PanRight,
    PanUp,
    PanDown,
}

pub fn parse_direction(s: &str) -> Result<Direction, String> {
    match s {
        "" | "zoom-in" => Ok(Direction::ZoomIn),
        "zoom-out" => Ok(Direction::ZoomOut),
        "pan-left" => Ok(Direction::PanLeft),
        "pan-right" => Ok(Direction::PanRight),
        "pan-up" => Ok(Direction::PanUp),
        "pan-down" => Ok(Direction::PanDown),
        other => Err(format!(
            "invalid direction {other:?}; expected zoom-in|zoom-out|pan-left|pan-right|pan-up|pan-down"
        )),
    }
}

/// Format an f64 for an ffmpeg expression: round to 6 decimals and strip the
/// trailing zeros/point so `0.3` stays `0.3` (not `0.30000000000000004` from
/// float subtraction) and `2.0` becomes `2`.
fn fnum(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Snap a dimension down to an even value (yuv420p/libx264 require even W/H),
/// clamped to a sane [16, 3840] range.
fn even_dim(v: u32) -> u32 {
    let v = v.clamp(16, 3840);
    v & !1
}

/// Build the `-vf` filter string: `scale(cover)`,`crop`,`zoompan`.
/// `w`/`h` are the (already even) output dimensions. The literal `FPS` token is
/// substituted by `plan` once the frame rate is formatted.
fn build_vf(dir: Direction, zoom: f64, w: u32, h: u32, total: i64) -> String {
    let pw = w * SUPERSAMPLE;
    let ph = h * SUPERSAMPLE;
    let den = (total - 1).max(1); // frames-1; guards against divide-by-zero
    let z = fnum(zoom);
    let zm1 = fnum(zoom - 1.0);
    let den = den.to_string();

    // Keep the crop window centered as the zoom changes (uses the per-frame `zoom`).
    let cx = "iw/2-(iw/zoom/2)";
    let cy = "ih/2-(ih/zoom/2)";

    let (zexpr, xexpr, yexpr) = match dir {
        // Zoom from 1 -> zoom, centered.
        Direction::ZoomIn => (format!("1+{zm1}*on/{den}"), cx.to_string(), cy.to_string()),
        // Zoom from zoom -> 1, centered.
        Direction::ZoomOut => (format!("{z}-{zm1}*on/{den}"), cx.to_string(), cy.to_string()),
        // Constant zoom, slide the crop window across the frame.
        Direction::PanRight => (z.clone(), format!("(iw-iw/zoom)*on/{den}"), cy.to_string()),
        Direction::PanLeft => (
            z.clone(),
            format!("(iw-iw/zoom)*({den}-on)/{den}"),
            cy.to_string(),
        ),
        Direction::PanDown => (z.clone(), cx.to_string(), format!("(ih-ih/zoom)*on/{den}")),
        Direction::PanUp => (
            z.clone(),
            cx.to_string(),
            format!("(ih-ih/zoom)*({den}-on)/{den}"),
        ),
    };

    format!(
        "scale={pw}:{ph}:force_original_aspect_ratio=increase,crop={pw}:{ph},\
zoompan=z='{zexpr}':x='{xexpr}':y='{yexpr}':d={total}:s={w}x{h}:fps=FPS"
    )
}

/// Validate params and return `(argv, out_name)`.
///
/// `direction` is one of the six moves; `duration` seconds (1..=30); `zoom` is
/// the peak zoom factor (1.0..=2.0 — panning needs > 1 to have room to move);
/// `width`/`height` the output size (snapped even, 16..=3840); `fps` frames per
/// second (1..=60). The output is always `out.mp4` (H.264).
pub fn plan(
    direction: &str,
    duration: f64,
    zoom: f64,
    width: u32,
    height: u32,
    fps: f64,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    let dir = parse_direction(direction)?;
    if !(1.0..=30.0).contains(&duration) {
        return Err(format!(
            "duration must be between 1 and 30 seconds (got {})",
            fnum(duration)
        ));
    }
    if !(1.0..=2.0).contains(&zoom) {
        return Err(format!("zoom must be between 1.0 and 2.0 (got {})", fnum(zoom)));
    }
    if !(1.0..=60.0).contains(&fps) {
        return Err(format!("fps must be between 1 and 60 (got {})", fnum(fps)));
    }
    if !(16..=3840).contains(&width) || !(16..=3840).contains(&height) {
        return Err("width and height must be between 16 and 3840 pixels".into());
    }

    let w = even_dim(width);
    let h = even_dim(height);
    let total = ((duration * fps).round() as i64).max(2);

    let vf = build_vf(dir, zoom, w, h, total).replace("FPS", &fnum(fps));

    let argv = vec![
        "-i".into(),
        in_name.into(),
        "-vf".into(),
        vf,
        "-c:v".into(),
        "libx264".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-movflags".into(),
        "+faststart".into(),
        "-t".into(),
        fnum(duration),
        OUT_NAME.into(),
    ];
    Ok((argv, OUT_NAME.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vf_of(argv: &[String]) -> String {
        let i = argv.iter().position(|a| a == "-vf").unwrap();
        argv[i + 1].clone()
    }

    #[test]
    fn zoom_in_builds_centered_linear_zoom() {
        let (argv, out) = plan("zoom-in", 2.0, 1.3, 1280, 720, 30.0, "in.png").unwrap();
        assert_eq!(out, "out.mp4");
        let vf = vf_of(&argv);
        // 2s * 30fps = 60 frames, den = 59.
        assert!(vf.contains("d=60"), "vf={vf}");
        assert!(vf.contains("s=1280x720"), "vf={vf}");
        assert!(vf.contains("fps=30"), "vf={vf}");
        // Prescale covers 2x the output box.
        assert!(vf.starts_with("scale=2560:1440:force_original_aspect_ratio=increase,crop=2560:1440,"));
        // Linear zoom 1 -> 1.3 over on/59, centered crop.
        assert!(vf.contains("z='1+0.3*on/59'"), "vf={vf}");
        assert!(vf.contains("x='iw/2-(iw/zoom/2)'"), "vf={vf}");
        assert!(vf.contains("y='ih/2-(ih/zoom/2)'"), "vf={vf}");
        // H.264 output, exact duration bound.
        assert!(argv.windows(2).any(|w| w == ["-c:v", "libx264"]));
        assert!(argv.windows(2).any(|w| w == ["-pix_fmt", "yuv420p"]));
        assert!(argv.windows(2).any(|w| w == ["-t", "2"]));
    }

    #[test]
    fn zoom_out_starts_zoomed_in() {
        let (argv, _) = plan("zoom-out", 1.0, 1.5, 640, 480, 25.0, "in.jpg").unwrap();
        let vf = vf_of(&argv);
        assert!(vf.contains("z='1.5-0.5*on/24'"), "vf={vf}");
        assert!(vf.contains("d=25"), "vf={vf}");
    }

    #[test]
    fn pan_right_holds_zoom_and_slides_x() {
        let (argv, _) = plan("pan-right", 2.0, 1.4, 800, 600, 30.0, "in.png").unwrap();
        let vf = vf_of(&argv);
        // Constant zoom, x slides 0 -> (iw - iw/zoom).
        assert!(vf.contains("z='1.4'"), "vf={vf}");
        assert!(vf.contains("x='(iw-iw/zoom)*on/59'"), "vf={vf}");
        assert!(vf.contains("y='ih/2-(ih/zoom/2)'"), "vf={vf}");
    }

    #[test]
    fn pan_up_slides_y_backwards() {
        let (argv, _) = plan("pan-up", 3.0, 1.3, 720, 1280, 30.0, "in.png").unwrap();
        let vf = vf_of(&argv);
        assert!(vf.contains("y='(ih-ih/zoom)*(89-on)/89'"), "vf={vf}");
        assert!(vf.contains("x='iw/2-(iw/zoom/2)'"), "vf={vf}");
    }

    #[test]
    fn odd_dimensions_are_snapped_even() {
        let (argv, _) = plan("zoom-in", 2.0, 1.2, 641, 481, 30.0, "in.png").unwrap();
        let vf = vf_of(&argv);
        assert!(vf.contains("s=640x480"), "vf={vf}");
        assert!(vf.starts_with("scale=1280:960:"), "vf={vf}");
    }

    #[test]
    fn rejects_unknown_direction() {
        assert!(plan("spin", 2.0, 1.3, 640, 480, 30.0, "in.png").is_err());
    }

    #[test]
    fn rejects_out_of_range_duration() {
        assert!(plan("zoom-in", 0.5, 1.3, 640, 480, 30.0, "in.png").is_err());
        assert!(plan("zoom-in", 45.0, 1.3, 640, 480, 30.0, "in.png").is_err());
    }

    #[test]
    fn rejects_out_of_range_zoom() {
        assert!(plan("zoom-in", 2.0, 0.9, 640, 480, 30.0, "in.png").is_err());
        assert!(plan("zoom-in", 2.0, 3.0, 640, 480, 30.0, "in.png").is_err());
    }

    #[test]
    fn rejects_out_of_range_fps_and_dims() {
        assert!(plan("zoom-in", 2.0, 1.3, 640, 480, 0.0, "in.png").is_err());
        assert!(plan("zoom-in", 2.0, 1.3, 640, 480, 120.0, "in.png").is_err());
        assert!(plan("zoom-in", 2.0, 1.3, 8, 480, 30.0, "in.png").is_err());
        assert!(plan("zoom-in", 2.0, 1.3, 640, 5000, 30.0, "in.png").is_err());
    }

    #[test]
    fn parse_direction_default_is_zoom_in() {
        assert_eq!(parse_direction("").unwrap(), Direction::ZoomIn);
    }
}
