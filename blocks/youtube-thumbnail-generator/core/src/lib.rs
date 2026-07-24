//! gizza-ai/youtube-thumbnail-generator core — pure ffmpeg argv construction.
//!
//! Grabs one frame from a video, scales/crops it to a thumbnail canvas, draws a
//! colored accent bar, and overlays a bold headline with outline/shadow via
//! ffmpeg `drawtext`. Text is supplied via `textfile=` and the bundled font via
//! `fontfile=` so user text is never interpolated into the filtergraph.

use gizza_ai_block_utils::normalize_ffmpeg_color;

pub const OUT_NAME: &str = "out.png";
pub const FONT_BYTES: &[u8] = include_bytes!("assets/LiberationSans-Bold.ttf");
pub const FONT_FILE: &str = "font.ttf";
pub const TEXT_FILE: &str = "headline.txt";

pub const POSITIONS: &[&str] = &["bottom", "top", "center"];
pub const BAR_POSITIONS: &[&str] = &["bottom", "top", "left", "right", "none"];
const MAX_TEXT_LEN: usize = 120;

fn num(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    let mut s = format!("{r:.3}");
    while s.contains('.') && s.ends_with('0') { s.pop(); }
    if s.ends_with('.') { s.pop(); }
    s
}

fn positive_u32(v: f64, default: u32, min: u32, max: u32, name: &str) -> Result<u32, String> {
    let n = if v.is_finite() && v > 0.0 { v.round() as u32 } else { default };
    if n < min || n > max {
        return Err(format!("{name} must be between {min} and {max}, got {n}"));
    }
    Ok(n)
}

fn position_xy(position: &str, margin: u32) -> Result<(&'static str, String), String> {
    match position.trim() {
        "bottom" | "" => Ok(("(w-text_w)/2", format!("h-text_h-{margin}"))),
        "top" => Ok(("(w-text_w)/2", format!("{margin}"))),
        "center" => Ok(("(w-text_w)/2", "(h-text_h)/2".to_string())),
        other => Err(format!("text_position {other:?} not supported (bottom|top|center)")),
    }
}

fn bar_filter(position: &str, color: &str, size: u32) -> Result<Option<String>, String> {
    let pos = position.trim();
    if pos.is_empty() || pos == "none" || size == 0 {
        return Ok(None);
    }
    let c = normalize_ffmpeg_color(if color.trim().is_empty() { "#ffcc00" } else { color })?;
    let f = match pos {
        "bottom" => format!("drawbox=x=0:y=ih-{size}:w=iw:h={size}:color={c}:t=fill"),
        "top" => format!("drawbox=x=0:y=0:w=iw:h={size}:color={c}:t=fill"),
        "left" => format!("drawbox=x=0:y=0:w={size}:h=ih:color={c}:t=fill"),
        "right" => format!("drawbox=x=iw-{size}:y=0:w={size}:h=ih:color={c}:t=fill"),
        other => return Err(format!("accent_position {other:?} not supported (bottom|top|left|right|none)")),
    };
    Ok(Some(f))
}

#[allow(clippy::too_many_arguments)]
pub fn build_filter(
    headline: &str,
    width: u32,
    height: u32,
    text_position: &str,
    font_size: u32,
    text_color: &str,
    outline_color: &str,
    accent_color: &str,
    accent_position: &str,
    accent_size: u32,
) -> Result<String, String> {
    if headline.trim().is_empty() {
        return Err("headline must not be empty".into());
    }
    if headline.chars().count() > MAX_TEXT_LEN {
        return Err(format!("headline is too long (max {MAX_TEXT_LEN} characters)"));
    }
    if !(16..=220).contains(&font_size) {
        return Err(format!("font_size must be between 16 and 220, got {font_size}"));
    }
    if !(320..=4096).contains(&width) || !(180..=4096).contains(&height) {
        return Err("width/height must be between 320x180 and 4096x4096".into());
    }
    if accent_size > height.max(width) / 2 {
        return Err("accent_size is too large for the canvas".into());
    }

    let text_color = normalize_ffmpeg_color(if text_color.trim().is_empty() { "#ffffff" } else { text_color })?;
    let outline_color = normalize_ffmpeg_color(if outline_color.trim().is_empty() { "#000000" } else { outline_color })?;
    let (x, y) = position_xy(text_position, (height / 12).max(24))?;
    let mut filters = vec![format!(
        "scale={width}:{height}:force_original_aspect_ratio=increase,crop={width}:{height}"
    )];
    if let Some(bar) = bar_filter(accent_position, accent_color, accent_size)? {
        filters.push(bar);
    }
    filters.push(format!(
        "drawtext=fontfile={FONT_FILE}:textfile={TEXT_FILE}:expansion=none:fontsize={font_size}:fontcolor={text_color}:borderw={}:bordercolor={outline_color}:shadowx={}:shadowy={}:shadowcolor=black@0.55:x={x}:y={y}",
        (font_size / 12).max(3),
        (font_size / 18).max(3),
        (font_size / 18).max(3),
    ));
    Ok(filters.join(","))
}

#[allow(clippy::too_many_arguments)]
pub fn plan(
    in_name: &str,
    headline: &str,
    timestamp: f64,
    width: f64,
    height: f64,
    text_position: &str,
    font_size: f64,
    text_color: &str,
    outline_color: &str,
    accent_color: &str,
    accent_position: &str,
    accent_size: f64,
) -> Result<(Vec<String>, String), String> {
    if !timestamp.is_finite() || timestamp < 0.0 {
        return Err(format!("timestamp must be >= 0 and finite, got {timestamp}"));
    }
    let width = positive_u32(width, 1280, 320, 4096, "width")?;
    let height = positive_u32(height, 720, 180, 4096, "height")?;
    let font_size = positive_u32(font_size, 96, 16, 220, "font_size")?;
    let accent_size = positive_u32(accent_size, 22, 0, 1024, "accent_size")?;
    let filter = build_filter(
        headline, width, height, text_position, font_size, text_color, outline_color,
        accent_color, accent_position, accent_size,
    )?;
    Ok((
        vec![
            "-ss".into(), num(timestamp), "-i".into(), in_name.into(),
            "-frames:v".into(), "1".into(), "-vf".into(), filter,
            "-update".into(), "1".into(), "-y".into(), OUT_NAME.into(),
        ],
        OUT_NAME.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vf(argv: &[String]) -> String {
        argv[argv.iter().position(|a| a == "-vf").unwrap() + 1].clone()
    }

    #[test]
    fn happy_path_builds_thumbnail_filter() {
        let (argv, out) = plan("in.mp4", "BIG NEWS", 1.5, 1280.0, 720.0, "bottom", 96.0, "#fff", "#000", "#fc0", "bottom", 22.0).unwrap();
        assert_eq!(out, OUT_NAME);
        assert_eq!(argv[0], "-ss");
        assert_eq!(argv[1], "1.5");
        assert!(argv.windows(2).any(|w| w[0] == "-frames:v" && w[1] == "1"));
        let f = vf(&argv);
        assert!(f.contains("scale=1280:720"));
        assert!(f.contains("crop=1280:720"));
        assert!(f.contains("drawbox=x=0:y=ih-22"));
        assert!(f.contains("drawtext="));
        assert!(f.contains(&format!("fontfile={FONT_FILE}")));
        assert!(f.contains(&format!("textfile={TEXT_FILE}")));
        assert!(!f.contains("BIG NEWS"));
    }

    #[test]
    fn every_position_and_bar_variant_is_supported() {
        for p in POSITIONS {
            build_filter("T", 1280, 720, p, 80, "white", "black", "#ffcc00", "none", 0).unwrap();
        }
        for b in BAR_POSITIONS {
            build_filter("T", 1280, 720, "center", 80, "white", "black", "#ffcc00", b, 20).unwrap();
        }
    }

    #[test]
    fn validates_bad_inputs() {
        assert!(plan("in.mp4", "", 0.0, 1280.0, 720.0, "bottom", 96.0, "white", "black", "#fc0", "bottom", 22.0).is_err());
        assert!(plan("in.mp4", "T", -1.0, 1280.0, 720.0, "bottom", 96.0, "white", "black", "#fc0", "bottom", 22.0).is_err());
        assert!(plan("in.mp4", "T", 0.0, 100.0, 720.0, "bottom", 96.0, "white", "black", "#fc0", "bottom", 22.0).is_err());
        assert!(plan("in.mp4", "T", 0.0, 1280.0, 720.0, "left", 96.0, "white", "black", "#fc0", "bottom", 22.0).is_err());
        assert!(plan("in.mp4", "T", 0.0, 1280.0, 720.0, "bottom", 96.0, "notacolor", "black", "#fc0", "bottom", 22.0).is_err());
        assert!(plan("in.mp4", "T", 0.0, 1280.0, 720.0, "bottom", 96.0, "white", "black", "#fc0", "diagonal", 22.0).is_err());
    }

    #[test]
    fn font_bytes_are_bundled() {
        assert!(FONT_BYTES.len() > 10_000);
        assert_eq!(&FONT_BYTES[0..4], &[0x00, 0x01, 0x00, 0x00]);
    }
}
