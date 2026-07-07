//! gizza-ai/image-shrink-for-sharing core — pure ffmpeg argv construction shared
//! by the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! This is the one-step "shrink an image for messaging / upload" pipeline: in a
//! single ffmpeg pass it
//!   1. **downscales** the longest side to `max_dimension` px (aspect ratio kept,
//!      never upscaled; `0` skips the resize),
//!   2. **strips metadata** (EXIF / GPS / comments) when `strip_metadata`,
//!   3. **re-encodes** at `quality` (1–100), optionally converting the output
//!      `format` (keep / jpeg / png / webp).
//!
//! It deliberately overlaps `image-resize` (scale only), `image-compress`
//! (quality only) and `strip-exif` (metadata only) — its value is doing all
//! three at once with sharing-friendly defaults.
//!
//! The single-pass design means a true "compress to exactly N KB" target is out
//! of scope (that needs an iterative quality search) — the `quality` +
//! `max_dimension` + `format` knobs are the levers.

/// Image format inferred from a filename extension. These are the only formats
/// this tool decodes (input) and can emit (output).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Jpeg,
    Png,
    Webp,
}

impl Format {
    /// Lower-cased extension this format writes (used for `out.<ext>`).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Jpeg => "jpg",
            Format::Png => "png",
            Format::Webp => "webp",
        }
    }

    /// The MIME type the produced file carries (drives the chat/CLI envelope;
    /// the page derives its own MIME from `out_name`'s extension).
    pub fn mime(self) -> &'static str {
        match self {
            Format::Jpeg => "image/jpeg",
            Format::Png => "image/png",
            Format::Webp => "image/webp",
        }
    }
}

/// Infer the input [`Format`] from a filename's extension. JPEG accepts both
/// `jpg` and `jpeg`. Returns an error for any other / missing extension.
pub fn format_from_name(in_name: &str) -> Result<Format, String> {
    let ext = in_name
        .rsplit('.')
        .next()
        .filter(|e| !e.is_empty() && *e != in_name)
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("jpg") | Some("jpeg") => Ok(Format::Jpeg),
        Some("png") => Ok(Format::Png),
        Some("webp") => Ok(Format::Webp),
        Some(other) => Err(format!(
            "unsupported image format {other:?}; image-shrink-for-sharing supports jpg/jpeg, png, webp"
        )),
        None => Err("input filename has no extension; cannot infer image format".into()),
    }
}

/// Resolve the requested output `format` against the input's format. `"keep"`
/// (or empty) keeps the input format; `jpeg`/`jpg`/`png`/`webp` convert.
pub fn resolve_out_format(format: &str, in_fmt: Format) -> Result<Format, String> {
    match format.trim().to_ascii_lowercase().as_str() {
        "" | "keep" => Ok(in_fmt),
        "jpeg" | "jpg" => Ok(Format::Jpeg),
        "png" => Ok(Format::Png),
        "webp" => Ok(Format::Webp),
        other => Err(format!(
            "invalid format {other:?}; expected keep|jpeg|png|webp"
        )),
    }
}

/// Map web-conventional quality 1-100 to ffmpeg JPEG `-q:v` range 31 (worst) – 2
/// (best). Mirrors `image-compress`/`image-convert` so the tools agree.
fn quality_to_qv(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let qv = 31.0 - (q - 1.0) * (29.0 / 99.0);
    qv.round().clamp(2.0, 31.0) as u8
}

/// Map quality 1-100 to ffmpeg PNG `-compression_level` 0-9. PNG is lossless, so
/// quality only tunes encoder effort: a LOWER quality asks for HARDER compression
/// (higher level → smaller file, more CPU). 100 → 0, 1 → 9. Mirrors image-compress.
fn quality_to_png_level(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let level = (100.0 - q) / 100.0 * 9.0;
    level.round().clamp(0.0, 9.0) as u8
}

/// The `-vf` scale filtergraph that caps the longest side to `n` px, preserves
/// the aspect ratio, never upscales (the box is capped at the source size), and
/// rounds the output to even dimensions so JPEG (yuvj420p) encodes cleanly.
fn scale_filter(n: u32) -> String {
    format!(
        "scale='min({n},iw)':'min({n},ih)':force_original_aspect_ratio=decrease:force_divisible_by=2"
    )
}

/// Push the per-format encoder quality flag onto `argv`.
fn push_quality_flag(argv: &mut Vec<String>, fmt: Format, quality: u8) {
    match fmt {
        Format::Jpeg => {
            argv.push("-q:v".into());
            argv.push(quality_to_qv(quality).to_string());
        }
        Format::Webp => {
            argv.push("-quality".into());
            argv.push(quality.clamp(1, 100).to_string());
        }
        Format::Png => {
            argv.push("-compression_level".into());
            argv.push(quality_to_png_level(quality).to_string());
        }
    }
}

/// Validate the params and build `(argv, out_name)` for an input file. The argv
/// has NO leading `"ffmpeg"`; `out_name` carries the OUTPUT format's extension
/// (which may differ from the input when `format` converts). This is the single
/// source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
///
/// - `max_dimension`: cap the longest side to this many px (0 = keep original size).
/// - `quality`: 1-100 (validated).
/// - `format`: keep | jpeg | png | webp.
/// - `strip_metadata`: drop EXIF/GPS/comments when true.
pub fn plan_shrink(
    max_dimension: u32,
    quality: u8,
    format: &str,
    strip_metadata: bool,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    if !(1..=100).contains(&quality) {
        return Err(format!("quality must be 1-100, got {quality}"));
    }
    let in_fmt = format_from_name(in_name)?;
    let out_fmt = resolve_out_format(format, in_fmt)?;
    let out_name = format!("out.{}", out_fmt.ext());

    let mut argv = vec!["-i".to_string(), in_name.to_string()];
    if strip_metadata {
        argv.push("-map_metadata".into());
        argv.push("-1".into());
    }
    if max_dimension > 0 {
        argv.push("-vf".into());
        argv.push(scale_filter(max_dimension));
    }
    push_quality_flag(&mut argv, out_fmt, quality);
    argv.push(out_name.clone());
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag_value(argv: &[String], flag: &str) -> Option<String> {
        argv.iter().position(|a| a == flag).map(|i| argv[i + 1].clone())
    }

    #[test]
    fn format_from_name_infers_each_type() {
        assert_eq!(format_from_name("a.jpg").unwrap(), Format::Jpeg);
        assert_eq!(format_from_name("a.JPEG").unwrap(), Format::Jpeg);
        assert_eq!(format_from_name("shot.png").unwrap(), Format::Png);
        assert_eq!(format_from_name("photo.final.webp").unwrap(), Format::Webp);
    }

    #[test]
    fn format_from_name_rejects_unknown_and_missing() {
        assert!(format_from_name("a.gif").is_err());
        assert!(format_from_name("noext").is_err());
    }

    #[test]
    fn resolve_out_format_keep_and_convert() {
        assert_eq!(resolve_out_format("keep", Format::Png).unwrap(), Format::Png);
        assert_eq!(resolve_out_format("", Format::Webp).unwrap(), Format::Webp);
        assert_eq!(resolve_out_format("jpeg", Format::Png).unwrap(), Format::Jpeg);
        assert_eq!(resolve_out_format("JPG", Format::Png).unwrap(), Format::Jpeg);
        assert_eq!(resolve_out_format("webp", Format::Jpeg).unwrap(), Format::Webp);
        assert!(resolve_out_format("tiff", Format::Png).is_err());
    }

    #[test]
    fn happy_path_downscales_strips_and_compresses_keeping_format() {
        let (argv, out) = plan_shrink(1600, 80, "keep", true, "photo.jpg").unwrap();
        assert_eq!(out, "out.jpg");
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "photo.jpg");
        assert_eq!(argv.last().unwrap(), "out.jpg");
        // strips metadata
        assert_eq!(flag_value(&argv, "-map_metadata").as_deref(), Some("-1"));
        // downscales via the capped, aspect-preserving, even-dim scale filter
        let vf = flag_value(&argv, "-vf").expect("must set -vf");
        assert!(vf.contains("min(1600,iw)"), "vf was {vf}");
        assert!(vf.contains("force_original_aspect_ratio=decrease"), "vf was {vf}");
        assert!(vf.contains("force_divisible_by=2"), "vf was {vf}");
        // jpeg quality flag
        let qv: u8 = flag_value(&argv, "-q:v").unwrap().parse().unwrap();
        assert!((6..=10).contains(&qv), "quality 80 → -q:v ~8, got {qv}");
    }

    #[test]
    fn max_dimension_zero_skips_resize() {
        let (argv, _) = plan_shrink(0, 80, "keep", true, "photo.jpg").unwrap();
        assert!(!argv.iter().any(|a| a == "-vf"), "0 must skip the scale filter");
    }

    #[test]
    fn strip_metadata_false_omits_map_metadata() {
        let (argv, _) = plan_shrink(1000, 80, "keep", false, "a.png").unwrap();
        assert!(!argv.iter().any(|a| a == "-map_metadata"));
    }

    #[test]
    fn format_conversion_changes_out_ext_and_encoder_flag() {
        // png in → webp out: uses -quality and out.webp
        let (argv, out) = plan_shrink(1200, 70, "webp", true, "logo.png").unwrap();
        assert_eq!(out, "out.webp");
        assert_eq!(flag_value(&argv, "-quality").as_deref(), Some("70"));
        assert!(!argv.iter().any(|a| a == "-compression_level"));
        // jpg in → png out: uses -compression_level and out.png
        let (argv, out) = plan_shrink(1200, 90, "png", true, "photo.jpg").unwrap();
        assert_eq!(out, "out.png");
        let lvl: u8 = flag_value(&argv, "-compression_level").unwrap().parse().unwrap();
        assert!((0..=9).contains(&lvl));
        assert!(!argv.iter().any(|a| a == "-q:v"));
    }

    #[test]
    fn quality_endpoints_map_into_ffmpeg_ranges() {
        assert_eq!(quality_to_qv(100), 2);
        assert_eq!(quality_to_qv(1), 31);
        assert_eq!(quality_to_png_level(100), 0);
        assert_eq!(quality_to_png_level(1), 9);
    }

    #[test]
    fn plan_shrink_rejects_out_of_range_quality_and_bad_input() {
        assert!(plan_shrink(1600, 0, "keep", true, "a.jpg").is_err());
        assert!(plan_shrink(1600, 101, "keep", true, "a.jpg").is_err());
        assert!(plan_shrink(1600, 80, "keep", true, "a.gif").is_err());
        assert!(plan_shrink(1600, 80, "avif", true, "a.jpg").is_err());
    }
}
