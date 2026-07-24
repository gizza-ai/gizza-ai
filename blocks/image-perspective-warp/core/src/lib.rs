//! image-perspective-warp core — pure ffmpeg argv construction shared by the chat
//! block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Wraps ffmpeg's `perspective` filter. Corners are ffmpeg coordinate expressions
//! (most commonly pixel numbers plus the built-in `W` and `H` frame constants).
//! The filter accepts `W`/`H` as full-frame defaults, but does not reliably
//! evaluate arithmetic like `0.12*W` across the browser/runtime builds, so this
//! tool intentionally exposes direct coordinates instead of pretending normalized
//! percentages are portable. Corner order is top-left(0), top-right(1),
//! bottom-left(2), bottom-right(3).

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Interp {
    Linear,
    Cubic,
}

impl Interp {
    fn as_ffmpeg(self) -> &'static str {
        match self {
            Interp::Linear => "linear",
            Interp::Cubic => "cubic",
        }
    }
}

pub fn parse_interp(s: Option<&str>) -> Result<Interp, String> {
    match s.unwrap_or("linear") {
        "" | "linear" => Ok(Interp::Linear),
        "cubic" => Ok(Interp::Cubic),
        other => Err(format!(
            "invalid interpolation {other:?}; expected linear|cubic"
        )),
    }
}

/// Warp mode → ffmpeg `perspective` `sense`.
/// - `Correct` (`sense=source`): the four points are where a rectangle currently
///   sits in the (distorted) source; they are stretched to the full output frame.
/// - `Distort` (`sense=destination`): the four points are where the source
///   corners are pushed TO — i.e. deliberately add a perspective tilt.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    Correct,
    Distort,
}

impl Mode {
    fn as_ffmpeg(self) -> &'static str {
        match self {
            Mode::Correct => "source",
            Mode::Distort => "destination",
        }
    }
}

pub fn parse_mode(s: Option<&str>) -> Result<Mode, String> {
    match s.unwrap_or("correct") {
        "" | "correct" => Ok(Mode::Correct),
        "distort" => Ok(Mode::Distort),
        other => Err(format!("invalid mode {other:?}; expected correct|distort")),
    }
}

/// Corners as ffmpeg coordinate expressions in filter order:
/// `[tl_x, tl_y, tr_x, tr_y, bl_x, bl_y, br_x, br_y]`.
pub type Corners<'a> = [&'a str; 8];

fn clean_coord(raw: &str, name: &str) -> Result<String, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(format!("{name} must not be empty"));
    }
    // Keep the public surface deliberately small and portable: pixel numbers and
    // the two constants ffmpeg documents for the perspective filter defaults.
    if matches!(s, "W" | "H") {
        return Ok(s.to_string());
    }
    let n: f64 = s
        .parse()
        .map_err(|_| format!("{name} must be a pixel number or W/H"))?;
    if !n.is_finite() {
        return Err(format!("{name} is not a finite number"));
    }
    if !(-100_000.0..=100_000.0).contains(&n) {
        return Err(format!("{name} is outside the supported -100000..100000 pixel range"));
    }
    Ok(s.to_string())
}

fn numeric_area(coords: &[String; 8]) -> Option<f64> {
    let nums: Option<Vec<f64>> = coords.iter().map(|s| s.parse::<f64>().ok()).collect();
    let n = nums?;
    let poly = [(n[0], n[1]), (n[2], n[3]), (n[6], n[7]), (n[4], n[5])];
    let mut a = 0.0;
    for i in 0..4 {
        let (x1, y1) = poly[i];
        let (x2, y2) = poly[(i + 1) % 4];
        a += x1 * y2 - x2 * y1;
    }
    Some((a / 2.0).abs())
}

/// Validate the corners and build the ffmpeg argv (no leading "ffmpeg").
pub fn build_argv(
    corners: &Corners<'_>,
    interp: Interp,
    mode: Mode,
    in_name: &str,
    out_name: &str,
) -> Result<Vec<String>, String> {
    let names = ["tl_x", "tl_y", "tr_x", "tr_y", "bl_x", "bl_y", "br_x", "br_y"];
    let coords: [String; 8] = std::array::from_fn(|i| clean_coord(corners[i], names[i]).unwrap_or_default());
    for (i, c) in coords.iter().enumerate() {
        if c.is_empty() {
            return Err(clean_coord(corners[i], names[i]).unwrap_err());
        }
    }
    if let Some(area) = numeric_area(&coords) {
        if area < 1.0 {
            return Err(
                "the four corners are collinear or collapsed (near-zero area); adjust them so they form a quadrilateral"
                    .into(),
            );
        }
    }
    let vf = format!(
        "perspective=x0={}:y0={}:x1={}:y1={}:x2={}:y2={}:x3={}:y3={}:interpolation={}:sense={}",
        coords[0], coords[1], coords[2], coords[3], coords[4], coords[5], coords[6], coords[7],
        interp.as_ffmpeg(), mode.as_ffmpeg(),
    );
    Ok(vec!["-i".into(), in_name.into(), "-vf".into(), vf, out_name.into()])
}

/// Validate + return `(argv, out_name)` for an input file. `out_name` keeps the
/// input extension. Used by both the chat block and the web page.
pub fn plan(
    corners: &Corners<'_>,
    interp: Interp,
    mode: Mode,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    let ext = in_name
        .rsplit('.')
        .next()
        .filter(|e| !e.is_empty())
        .unwrap_or("png");
    let out_name = format!("out.{ext}");
    let argv = build_argv(corners, interp, mode, in_name, &out_name)?;
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    const INSET: Corners<'static> = ["12", "12", "56", "12", "12", "56", "56", "56"];

    #[test]
    fn build_argv_emits_coordinate_expressions_and_named_options() {
        let (argv, out) = plan(&INSET, Interp::Linear, Mode::Correct, "in.png").unwrap();
        assert_eq!(out, "out.png");
        let vf = argv.iter().find(|a| a.starts_with("perspective=")).unwrap();
        assert_eq!(
            vf,
            "perspective=x0=12:y0=12:x1=56:y1=12:x2=12:y2=56:x3=56:y3=56:interpolation=linear:sense=source"
        );
        assert_eq!(argv.first().unwrap(), "-i");
        assert_eq!(argv.last().unwrap(), "out.png");
    }

    #[test]
    fn accepts_frame_constants_for_identity_defaults() {
        let identity: Corners<'static> = ["0", "0", "W", "0", "0", "H", "W", "H"];
        let (argv, _) = plan(&identity, Interp::Linear, Mode::Correct, "in.png").unwrap();
        let vf = argv.iter().find(|a| a.starts_with("perspective=")).unwrap();
        assert!(vf.contains("x1=W"));
        assert!(vf.contains("y2=H"));
        assert!(vf.contains("x3=W:y3=H"));
    }

    #[test]
    fn distort_and_cubic_map_to_ffmpeg_names() {
        let (argv, _) = plan(&INSET, Interp::Cubic, Mode::Distort, "in.jpg").unwrap();
        let vf = argv.iter().find(|a| a.starts_with("perspective=")).unwrap();
        assert!(vf.ends_with("interpolation=cubic:sense=destination"));
    }

    #[test]
    fn plan_keeps_input_extension() {
        let (_, out) = plan(&INSET, Interp::Linear, Mode::Correct, "scan.jpeg").unwrap();
        assert_eq!(out, "out.jpeg");
    }

    #[test]
    fn rejects_collapsed_numeric_quad() {
        let collapsed: Corners<'static> = ["50", "50", "50", "50", "50", "50", "50", "50"];
        let err = plan(&collapsed, Interp::Linear, Mode::Correct, "in.png").unwrap_err();
        assert!(err.contains("collinear or collapsed"), "got: {err}");
    }

    #[test]
    fn rejects_bad_coordinate_expression() {
        let bad: Corners<'static> = ["0", "0", "W*0.8", "0", "0", "H", "W", "H"];
        let err = plan(&bad, Interp::Linear, Mode::Correct, "in.png").unwrap_err();
        assert!(err.contains("pixel number or W/H"), "got: {err}");
    }

    #[test]
    fn parse_interp_and_mode_defaults_and_errors() {
        assert_eq!(parse_interp(None).unwrap(), Interp::Linear);
        assert_eq!(parse_interp(Some("")).unwrap(), Interp::Linear);
        assert_eq!(parse_interp(Some("cubic")).unwrap(), Interp::Cubic);
        assert!(parse_interp(Some("nearest")).is_err());
        assert_eq!(parse_mode(None).unwrap(), Mode::Correct);
        assert_eq!(parse_mode(Some("distort")).unwrap(), Mode::Distort);
        assert!(parse_mode(Some("warp")).is_err());
    }
}
