//! gizza-ai/image-convert core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! image-convert transcodes an image to a DIFFERENT target format (jpeg, png,
//! webp). Unlike resize/compress, the output extension is the user-chosen target
//! format, not the input's. JPEG/WebP take a lossy `-q:v` derived from a
//! web-conventional `quality` 1-100; PNG is lossless and omits `-q:v`.

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

/// Map web-conventional quality 1-100 to ffmpeg's -q:v range 31 (worst) – 2 (best).
/// Mirrored by `gizza-ai/image-compress`'s `quality_to_qv` so the two tools agree.
pub fn quality_to_qv(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let qv = 31.0 - (q - 1.0) * (29.0 / 99.0);
    qv.round().clamp(2.0, 31.0) as u8
}

/// Build the ffmpeg argv (no leading "ffmpeg") to transcode `in_name` to
/// `out_name` in `format` at `quality` (1-100). PNG is lossless and omits the
/// quality flag; jpeg/webp set `-q:v`.
pub fn build_argv(in_name: &str, out_name: &str, format: Format, quality: u8) -> Vec<String> {
    let mut argv = vec!["-i".to_string(), in_name.to_string()];
    if format != Format::Png {
        argv.push("-q:v".into());
        argv.push(quality_to_qv(quality).to_string());
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
    fn argv_png_omits_qv() {
        let argv = build_argv("in.jpg", "out.png", Format::Png, 85);
        assert!(!argv.iter().any(|a| a == "-q:v"), "png argv should not include -q:v: {argv:?}");
    }

    #[test]
    fn argv_webp_includes_qv() {
        let argv = build_argv("in.png", "out.webp", Format::Webp, 50);
        assert!(argv.iter().any(|a| a == "-q:v"));
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
