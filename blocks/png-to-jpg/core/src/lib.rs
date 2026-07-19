//! png-to-jpg core — pure ffmpeg argv construction shared by the chat skill
//! block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! JPEG has no alpha channel, so a straight PNG→JPG transcode drops
//! transparency onto black (ffmpeg's default). This tool makes the fill
//! explicit: the input is split, one copy is flood-filled with a user-chosen
//! background color (`format=rgb24` drops the source alpha so the fill is
//! fully opaque, `drawbox=t=fill` paints the whole frame), and the original —
//! alpha intact — is overlaid on top. Fully transparent pixels become the
//! background color and semi-transparent pixels blend onto it, exactly like a
//! browser renders the PNG over that color. The result is encoded as JPEG at
//! a web-conventional `quality` 1-100 mapped to mjpeg's `-q:v` (the same
//! mapping image-convert / image-compress use, so the tools agree).
//!
//! Colors go through [`gizza_ai_block_utils::normalize_ffmpeg_color`] (CSS
//! names or `#RGB`/`#RRGGBB`/`0xRRGGBB` hex → an ffmpeg-safe token), so the
//! filtergraph stays a single injection-free argv element.

use gizza_ai_block_utils::normalize_ffmpeg_color;

/// Default background color for transparent areas: white — what browsers and
/// every mainstream converter assume when flattening for JPEG.
pub const DEFAULT_BACKGROUND: &str = "#ffffff";

/// Default JPEG quality (1-100). 85 is the common "visually lossless on the
/// web" default and matches image-convert's default.
pub const DEFAULT_QUALITY: u8 = 85;

/// Map web-conventional quality 1-100 to ffmpeg's `-q:v` range 31 (worst) – 2
/// (best). Mirrors `gizza-ai/image-convert` / `image-compress` so all three
/// tools agree on what e.g. "quality 85" means.
pub fn quality_to_qv(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let qv = 31.0 - (q - 1.0) * (29.0 / 99.0);
    qv.round().clamp(2.0, 31.0) as u8
}

/// The `-vf` filter that flattens transparency onto `color` (an already
/// normalized ffmpeg color token). Single argv token, no spaces: split the
/// input, flood one copy with the background (alpha dropped via rgb24 so the
/// fill is opaque), overlay the original on top.
fn flatten_filter(color: &str) -> String {
    format!("split[a][b];[a]format=rgb24,drawbox=color={color}:t=fill[bg];[bg][b]overlay")
}

/// Build the ffmpeg argv (no leading "ffmpeg") and output filename that
/// flatten `in_name`'s transparency onto `background` and encode JPEG.
///
/// `background` is a CSS color name or hex (`#RGB`/`#RRGGBB`/`0xRRGGBB`);
/// `None`/empty falls back to [`DEFAULT_BACKGROUND`] (white). `quality` is
/// 1-100; `0.0` means "unset" (the page sends 0 for a cleared field) and falls
/// back to [`DEFAULT_QUALITY`]. Animated inputs (GIF/APNG/WebP) take the first
/// frame — JPEG is a single still image.
pub fn plan(
    in_name: &str,
    background: Option<&str>,
    quality: f64,
) -> Result<(Vec<String>, String), String> {
    let raw = background
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_BACKGROUND);
    let color = normalize_ffmpeg_color(raw)?;
    let q = if quality == 0.0 {
        DEFAULT_QUALITY
    } else if quality.is_finite() && (1.0..=100.0).contains(&quality) {
        quality.round() as u8
    } else {
        return Err(format!(
            "quality must be a number between 1 and 100, got {quality}"
        ));
    };
    let out_name = "out.jpg".to_string();
    let argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vf".to_string(),
        flatten_filter(&color),
        "-q:v".to_string(),
        quality_to_qv(q).to_string(),
        // JPEG is a single still image: take one frame so an animated input
        // (GIF/APNG/animated WebP) converts cleanly instead of erroring.
        "-frames:v".to_string(),
        "1".to_string(),
        out_name.clone(),
    ];
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag_value(argv: &[String], flag: &str) -> Option<String> {
        argv.iter().position(|a| a == flag).map(|i| argv[i + 1].clone())
    }

    #[test]
    fn default_background_is_white_and_quality_85() {
        let (argv, out) = plan("in.png", None, 0.0).unwrap();
        assert_eq!(out, "out.jpg");
        let vf = flag_value(&argv, "-vf").expect("must set -vf");
        assert!(vf.contains("drawbox=color=0xFFFFFF:t=fill"), "white fill: {vf}");
        assert!(vf.contains("overlay"), "original overlaid on the fill: {vf}");
        assert!(!vf.contains(' '), "-vf must be a single space-free token: {vf}");
        // quality 85 → -q:v 6 (31 - 84*29/99 ≈ 6.4 → 6)
        assert_eq!(flag_value(&argv, "-q:v").unwrap(), "6");
        assert_eq!(argv.last().unwrap(), "out.jpg");
    }

    #[test]
    fn empty_background_string_falls_back_to_white() {
        let (argv, _) = plan("in.png", Some("   "), 85.0).unwrap();
        assert!(flag_value(&argv, "-vf").unwrap().contains("0xFFFFFF"));
    }

    #[test]
    fn short_hex_long_hex_and_names_normalize() {
        let (argv, _) = plan("in.png", Some("#00f"), 85.0).unwrap();
        assert!(flag_value(&argv, "-vf").unwrap().contains("drawbox=color=0x0000FF"));
        let (argv, _) = plan("in.png", Some("#1a2b3c"), 85.0).unwrap();
        assert!(flag_value(&argv, "-vf").unwrap().contains("drawbox=color=0x1A2B3C"));
        let (argv, _) = plan("in.png", Some("Black"), 85.0).unwrap();
        assert!(flag_value(&argv, "-vf").unwrap().contains("drawbox=color=black"));
    }

    #[test]
    fn fill_happens_before_overlay_and_takes_one_frame() {
        let (argv, _) = plan("in.png", None, 0.0).unwrap();
        let vf = flag_value(&argv, "-vf").unwrap();
        let fill = vf.find("drawbox").unwrap();
        let over = vf.find("overlay").unwrap();
        assert!(fill < over, "background fill must be built before the overlay: {vf}");
        assert_eq!(flag_value(&argv, "-frames:v").unwrap(), "1");
    }

    #[test]
    fn quality_bounds_map_to_qv_extremes() {
        let (argv, _) = plan("in.png", None, 1.0).unwrap();
        assert_eq!(flag_value(&argv, "-q:v").unwrap(), "31"); // worst
        let (argv, _) = plan("in.png", None, 100.0).unwrap();
        assert_eq!(flag_value(&argv, "-q:v").unwrap(), "2"); // best
    }

    #[test]
    fn rejects_unknown_color() {
        let err = plan("in.png", Some("notacolor"), 85.0).unwrap_err();
        assert!(err.contains("not recognized"), "helpful color error: {err}");
    }

    #[test]
    fn rejects_out_of_range_quality() {
        assert!(plan("in.png", None, 101.0).is_err());
        assert!(plan("in.png", None, -3.0).is_err());
        assert!(plan("in.png", None, 0.5).is_err());
        assert!(plan("in.png", None, f64::NAN).is_err());
    }
}
