//! gizza-ai/image-convert core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! image-convert transcodes an image to a DIFFERENT target format (jpeg, png,
//! webp). Unlike resize/compress, the output extension is the user-chosen target
//! format, not the input's. The web-conventional `quality` 1-100 (higher =
//! better) maps onto each lossy encoder's own scale:
//!
//! - JPEG → `-q:v` (mjpeg's inverted 2 = best … 31 = worst), via [`quality_to_qv`].
//! - WebP → `-quality` (libwebp's 0 = worst … 100 = best), passed straight
//!   through. libwebp also reads `-q:v`, but as that same 0-100 scale — so
//!   feeding it the inverted mjpeg value would make a high page quality encode
//!   a LOW-quality WebP.
//! - PNG is lossless and takes no quality flag.

/// Target image format image-convert can transcode to.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Jpeg,
    Png,
    Webp,
}

impl Format {
    /// Lower-cased file extension this format writes (used for `out.<ext>`).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Jpeg => "jpg",
            Format::Png => "png",
            Format::Webp => "webp",
        }
    }
}

/// Parse the user-facing format string (the values the chat schema + page accept).
/// JPEG is spelled `"jpeg"` (not `"jpg"`) to match the schema enum.
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s {
        "jpeg" => Ok(Format::Jpeg),
        "png" => Ok(Format::Png),
        "webp" => Ok(Format::Webp),
        other => Err(format!(
            "format {other:?} not supported (jpeg|png|webp)"
        )),
    }
}

/// Map web-conventional quality 1-100 to ffmpeg's JPEG (mjpeg) `-q:v` range 31
/// (worst) – 2 (best). JPEG only — WebP takes `-quality` 1-100 directly.
/// Mirrored by `gizza-ai/image-compress`'s `quality_to_qv` so the two tools agree.
pub fn quality_to_qv(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let qv = 31.0 - (q - 1.0) * (29.0 / 99.0);
    qv.round().clamp(2.0, 31.0) as u8
}

/// Build the ffmpeg argv (no leading "ffmpeg") to transcode `in_name` to
/// `out_name` in `format` at `quality` (1-100). JPEG sets mjpeg's inverted
/// `-q:v`, WebP sets libwebp's `-quality` (same 1-100 direction as the page),
/// and PNG is lossless and omits any quality flag. JPEG has no alpha, so a
/// transparent input is flattened onto white first (what it looks like on a
/// page) rather than letting the encoder expose the color stored under
/// transparent pixels — e.g. the background a background removal took out.
pub fn build_argv(in_name: &str, out_name: &str, format: Format, quality: u8) -> Vec<String> {
    let mut argv = vec!["-i".to_string(), in_name.to_string()];
    match format {
        Format::Jpeg => {
            argv.push("-vf".into());
            argv.push(gizza_ai_block_utils::ffmpeg::FLATTEN_ON_WHITE.into());
            argv.push("-q:v".into());
            argv.push(quality_to_qv(quality).to_string());
        }
        Format::Webp => {
            argv.push("-quality".into());
            argv.push(quality.clamp(1, 100).to_string());
        }
        Format::Png => {}
    }
    argv.push(out_name.to_string());
    argv
}

/// Validate `quality`, parse `format`, and build `(argv, out_name)` for an input
/// file. `out_name` uses the TARGET format's extension. Single source shared by
/// the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_convert(
    format: &str,
    quality: u8,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    if !(1..=100).contains(&quality) {
        return Err(format!("quality must be 1-100, got {quality}"));
    }
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, fmt, quality), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag_value(argv: &[String], flag: &str) -> Option<String> {
        argv.iter().position(|a| a == flag).map(|i| argv[i + 1].clone())
    }

    #[test]
    fn parse_format_known() {
        assert_eq!(parse_format("jpeg").unwrap(), Format::Jpeg);
        assert_eq!(parse_format("png").unwrap(), Format::Png);
        assert_eq!(parse_format("webp").unwrap(), Format::Webp);
    }

    #[test]
    fn parse_format_rejects_unknown() {
        assert!(parse_format("gif").is_err());
        assert!(parse_format("jpg").is_err());
    }

    #[test]
    fn quality_85_maps_to_qv_about_6() {
        let qv = quality_to_qv(85);
        assert!((5..=7).contains(&qv), "expected 5-7, got {qv}");
    }

    #[test]
    fn quality_100_maps_to_qv_2_best() {
        assert_eq!(quality_to_qv(100), 2);
    }

    #[test]
    fn quality_1_maps_to_qv_31_worst() {
        assert_eq!(quality_to_qv(1), 31);
    }

    #[test]
    fn argv_jpeg_includes_qv() {
        let argv = build_argv("in.png", "out.jpg", Format::Jpeg, 85);
        let qv: u8 = flag_value(&argv, "-q:v").expect("jpeg argv must set -q:v").parse().unwrap();
        assert!((5..=7).contains(&qv));
    }

    #[test]
    fn argv_jpeg_omits_webp_quality() {
        let argv = build_argv("in.png", "out.jpg", Format::Jpeg, 85);
        assert!(flag_value(&argv, "-quality").is_none(), "{argv:?}");
    }

    #[test]
    fn argv_jpeg_flattens_alpha_onto_white() {
        let argv = build_argv("in.png", "out.jpg", Format::Jpeg, 85);
        assert_eq!(
            flag_value(&argv, "-vf").as_deref(),
            Some(gizza_ai_block_utils::ffmpeg::FLATTEN_ON_WHITE),
            "{argv:?}"
        );
        // PNG and WebP keep the alpha channel: no flatten.
        for (out, fmt) in [("out.png", Format::Png), ("out.webp", Format::Webp)] {
            let argv = build_argv("in.png", out, fmt, 85);
            assert!(flag_value(&argv, "-vf").is_none(), "{argv:?}");
        }
    }

    #[test]
    fn argv_png_omits_quality_flags() {
        let argv = build_argv("in.jpg", "out.png", Format::Png, 85);
        assert!(!argv.iter().any(|a| a == "-q:v"), "png argv should not include -q:v: {argv:?}");
        assert!(!argv.iter().any(|a| a == "-quality"), "png argv should not include -quality: {argv:?}");
    }

    #[test]
    fn argv_webp_passes_quality_straight_through() {
        // libwebp's quality scale runs the same way as the page's (higher =
        // better), so it must NOT get mjpeg's inverted -q:v value.
        for q in [1u8, 10, 50, 90, 100] {
            let argv = build_argv("in.png", "out.webp", Format::Webp, q);
            assert_eq!(flag_value(&argv, "-quality"), Some(q.to_string()), "{argv:?}");
            assert!(!argv.iter().any(|a| a == "-q:v"), "webp argv must not set -q:v: {argv:?}");
        }
    }

    #[test]
    fn plan_convert_uses_target_extension() {
        let (_argv, out) = plan_convert("png", 85, "cat.jpg").unwrap();
        assert_eq!(out, "out.png");
        let (_argv, out) = plan_convert("jpeg", 85, "cat.png").unwrap();
        assert_eq!(out, "out.jpg");
        let (_argv, out) = plan_convert("webp", 85, "cat.png").unwrap();
        assert_eq!(out, "out.webp");
    }

    #[test]
    fn plan_convert_rejects_out_of_range_quality_and_bad_format() {
        assert!(plan_convert("png", 0, "a.png").is_err());
        assert!(plan_convert("png", 101, "a.png").is_err());
        assert!(plan_convert("gif", 85, "a.png").is_err());
    }
}
