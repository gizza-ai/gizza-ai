//! gizza-ai/image-compress core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! image-compress re-encodes an image at a chosen quality to shrink its file
//! size while keeping the SAME format. The format is inferred from the input
//! file's extension, so the encoder flag differs per format:
//!
//! - JPEG (`.jpg`/`.jpeg`) → `-q:v` (lossy; 2 = best … 31 = worst). A lower
//!   `quality` raises `-q:v`, producing a smaller, lossier file.
//! - WebP (`.webp`)        → `-quality` (lossy; 0 = worst … 100 = best). The
//!   `quality` value is passed straight through.
//! - PNG  (`.png`)         → `-compression_level` (lossless; 0 = fast/large …
//!   9 = slow/small). PNG is lossless, so `quality` only tunes the encoder's
//!   zlib effort: a LOWER `quality` asks for HARDER compression (a higher
//!   `-compression_level`), trading encode time for a smaller file without any
//!   loss of image data.
//!
//! The single target-byte-size use case ("under 200 KB") needs a multi-pass
//! search the single `build_argv → ffmpegExec` call can't do; this tool exposes
//! the `quality` knob and leaves true target-size as a follow-up.

/// Image format inferred from the input file extension. These are the only
/// formats image-compress re-encodes (it always keeps the input format).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Jpeg,
    Png,
    Webp,
}

impl Format {
    /// Lower-cased file extension this format writes (used for `out.<ext>`).
    fn ext(self) -> &'static str {
        match self {
            Format::Jpeg => "jpg",
            Format::Png => "png",
            Format::Webp => "webp",
        }
    }
}

/// Infer the [`Format`] from a filename's extension. JPEG accepts both `jpg`
/// and `jpeg`. Returns an error for any other / missing extension.
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
            "unsupported image format {other:?}; image-compress keeps the input format and supports jpg/jpeg, png, webp"
        )),
        None => Err("input filename has no extension; cannot infer image format".into()),
    }
}

/// Map web-conventional quality 1-100 to ffmpeg JPEG `-q:v` range 31 (worst) – 2 (best).
/// Mirrors `gizza-ai/image-convert`'s `quality_to_qv` so the two tools agree.
fn quality_to_qv(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let qv = 31.0 - (q - 1.0) * (29.0 / 99.0);
    qv.round().clamp(2.0, 31.0) as u8
}

/// Map quality 1-100 to ffmpeg PNG `-compression_level` 0-9. PNG is lossless,
/// so quality only tunes encoder effort: a LOWER quality asks for HARDER
/// compression (higher level → smaller file, more CPU). 100 → 0, 1 → 9.
fn quality_to_png_level(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let level = (100.0 - q) / 100.0 * 9.0;
    level.round().clamp(0.0, 9.0) as u8
}

/// Build the ffmpeg argv (no leading "ffmpeg") to re-encode `in_name` at
/// `quality` (1-100) keeping the same format, writing `out_name`.
fn argv_for(in_name: &str, out_name: &str, fmt: Format, quality: u8) -> Vec<String> {
    let mut argv = vec!["-i".to_string(), in_name.to_string()];
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
    argv.push(out_name.to_string());
    argv
}

/// Validate `quality` and build `(argv, out_name)` for an input file, inferring
/// the format (and therefore the encoder flag) from `in_name`'s extension.
/// `out_name` keeps the input extension. This is the single source shared by the
/// chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_compress(quality: u8, in_name: &str) -> Result<(Vec<String>, String), String> {
    if !(1..=100).contains(&quality) {
        return Err(format!("quality must be 1-100, got {quality}"));
    }
    let fmt = format_from_name(in_name)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((argv_for(in_name, &out_name, fmt, quality), out_name))
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
        assert_eq!(format_from_name("a.png").unwrap(), Format::Png);
        assert_eq!(format_from_name("photo.final.webp").unwrap(), Format::Webp);
    }

    #[test]
    fn format_from_name_rejects_unknown_and_missing() {
        assert!(format_from_name("a.gif").is_err());
        assert!(format_from_name("noext").is_err());
    }

    #[test]
    fn jpeg_uses_qv_and_keeps_jpg_extension() {
        let (argv, out) = plan_compress(80, "photo.jpg").unwrap();
        assert_eq!(out, "out.jpg");
        let qv: u8 = flag_value(&argv, "-q:v").expect("jpeg argv must set -q:v").parse().unwrap();
        // quality 80 → roughly 8 on the 2..=31 scale.
        assert!((6..=10).contains(&qv), "expected -q:v ~8, got {qv}");
        assert!(!argv.iter().any(|a| a == "-quality"));
        assert!(!argv.iter().any(|a| a == "-compression_level"));
    }

    #[test]
    fn jpeg_lower_quality_means_higher_qv_smaller_file() {
        let (hi, _) = plan_compress(95, "p.jpg").unwrap();
        let (lo, _) = plan_compress(20, "p.jpg").unwrap();
        let qv_hi: u8 = flag_value(&hi, "-q:v").unwrap().parse().unwrap();
        let qv_lo: u8 = flag_value(&lo, "-q:v").unwrap().parse().unwrap();
        assert!(qv_lo > qv_hi, "lower quality should raise -q:v: {qv_lo} !> {qv_hi}");
    }

    #[test]
    fn png_uses_compression_level_and_keeps_png_extension() {
        let (argv, out) = plan_compress(80, "shot.png").unwrap();
        assert_eq!(out, "out.png");
        let lvl: u8 = flag_value(&argv, "-compression_level")
            .expect("png argv must set -compression_level")
            .parse()
            .unwrap();
        assert!((0..=9).contains(&lvl));
        assert!(!argv.iter().any(|a| a == "-q:v"));
        // PNG is lossless → lower quality asks for harder (higher) compression.
        let lo: u8 = flag_value(&plan_compress(10, "x.png").unwrap().0, "-compression_level")
            .unwrap()
            .parse()
            .unwrap();
        let hi: u8 = flag_value(&plan_compress(90, "x.png").unwrap().0, "-compression_level")
            .unwrap()
            .parse()
            .unwrap();
        assert!(lo > hi, "lower quality should raise -compression_level: {lo} !> {hi}");
    }

    #[test]
    fn webp_uses_quality_passthrough_and_keeps_webp_extension() {
        let (argv, out) = plan_compress(60, "pic.webp").unwrap();
        assert_eq!(out, "out.webp");
        assert_eq!(flag_value(&argv, "-quality").as_deref(), Some("60"));
        assert!(!argv.iter().any(|a| a == "-q:v"));
        assert!(!argv.iter().any(|a| a == "-compression_level"));
    }

    #[test]
    fn quality_endpoints_clamp_into_ffmpeg_ranges() {
        assert_eq!(quality_to_qv(100), 2);
        assert_eq!(quality_to_qv(1), 31);
        assert_eq!(quality_to_png_level(100), 0);
        assert_eq!(quality_to_png_level(1), 9);
    }

    #[test]
    fn plan_compress_rejects_out_of_range_quality() {
        assert!(plan_compress(0, "a.jpg").is_err());
        assert!(plan_compress(101, "a.jpg").is_err());
    }

    #[test]
    fn argv_starts_with_input_flag() {
        let (argv, _) = plan_compress(80, "a.jpg").unwrap();
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "a.jpg");
        assert_eq!(argv.last().unwrap(), "out.jpg");
    }
}
