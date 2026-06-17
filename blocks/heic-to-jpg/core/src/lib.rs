//! gizza-ai/heic-to-jpg core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Decodes an Apple HEIC/HEIF photo and re-encodes it to JPEG or PNG.
//!
//! NOTE (feasibility): HEIC decoding requires the ffmpeg build to include a HEIF
//! demuxer (ffmpeg 7.0+ native `heif` demuxer, or `--enable-libheif`). The argv
//! this module builds is correct, but whether ffmpeg can *open* the HEIC input
//! depends entirely on the runtime ffmpeg's HEIF support — see the PR for which
//! ffmpeg builds in this stack do/don't support it.

/// The chosen output format. The user-facing API value is `"jpg"` (default) or
/// `"png"` — these exact strings are used everywhere (chat schema enum, web
/// param, page field), so there is no per-surface magic mapping.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Jpg,
    Png,
}

impl Format {
    /// File extension (without dot) for this format.
    pub fn ext(self) -> &'static str {
        match self {
            Format::Jpg => "jpg",
            Format::Png => "png",
        }
    }

    /// MIME type for this format.
    pub fn mime(self) -> &'static str {
        match self {
            Format::Jpg => "image/jpeg",
            Format::Png => "image/png",
        }
    }
}

/// Parse the user-facing format string. Empty / absent defaults to `jpg`.
pub fn parse_format(s: Option<&str>) -> Result<Format, String> {
    match s.unwrap_or("jpg") {
        "" | "jpg" => Ok(Format::Jpg),
        "png" => Ok(Format::Png),
        other => Err(format!("invalid format {other:?}; expected jpg|png")),
    }
}

/// JPEG quality passed to ffmpeg's `-q:v` (2 = best … 31 = worst). A value of 3
/// is visually near-lossless while keeping files small — a sane photo default.
const JPG_QV: u8 = 3;

/// Build the ffmpeg argv (no leading "ffmpeg") to decode `in_name` (a HEIC/HEIF
/// file) and encode it to `fmt`. For JPEG, a sane `-q:v` is applied; PNG is
/// lossless so no quality flag is emitted.
pub fn build_argv(in_name: &str, out_name: &str, fmt: Format) -> Vec<String> {
    let mut argv = vec!["-i".to_string(), in_name.to_string()];
    if fmt == Format::Jpg {
        argv.push("-q:v".into());
        argv.push(JPG_QV.to_string());
    }
    argv.push(out_name.to_string());
    argv
}

/// Derive the output filename for a `format`: the input stem with the chosen ext.
/// `in_name` is the (sanitised) virtual input filename, e.g. `"in.heic"`.
pub fn out_name(in_name: &str, fmt: Format) -> String {
    let stem = in_name.rsplit_once('.').map(|(s, _)| s).unwrap_or(in_name);
    format!("{stem}.{}", fmt.ext())
}

/// Plan an HEIC→`format` conversion: returns `(argv, out_name)`. Used by both the
/// web page (file in → file out) and the chat block.
pub fn plan(in_name: &str, fmt: Format) -> (Vec<String>, String) {
    let out = out_name(in_name, fmt);
    (build_argv(in_name, &out, fmt), out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_format_default_is_jpg() {
        assert_eq!(parse_format(None).unwrap(), Format::Jpg);
    }

    #[test]
    fn parse_format_empty_is_jpg() {
        assert_eq!(parse_format(Some("")).unwrap(), Format::Jpg);
    }

    #[test]
    fn parse_format_png() {
        assert_eq!(parse_format(Some("png")).unwrap(), Format::Png);
    }

    #[test]
    fn parse_format_rejects_unknown() {
        assert!(parse_format(Some("gif")).is_err());
    }

    #[test]
    fn argv_jpg_includes_quality() {
        let argv = build_argv("in.heic", "in.jpg", Format::Jpg);
        assert_eq!(
            argv,
            vec![
                "-i".to_string(),
                "in.heic".to_string(),
                "-q:v".to_string(),
                "3".to_string(),
                "in.jpg".to_string(),
            ]
        );
    }

    #[test]
    fn argv_png_omits_quality() {
        let argv = build_argv("in.heic", "in.png", Format::Png);
        assert_eq!(
            argv,
            vec![
                "-i".to_string(),
                "in.heic".to_string(),
                "in.png".to_string(),
            ]
        );
        assert!(!argv.iter().any(|a| a == "-q:v"));
    }

    #[test]
    fn out_name_uses_stem_plus_ext() {
        assert_eq!(out_name("in.heic", Format::Jpg), "in.jpg");
        assert_eq!(out_name("photo.HEIC", Format::Png), "photo.png");
        assert_eq!(out_name("noext", Format::Jpg), "noext.jpg");
    }

    #[test]
    fn plan_jpg_default() {
        let (argv, out) = plan("in.heic", Format::Jpg);
        assert_eq!(out, "in.jpg");
        assert!(argv.iter().any(|a| a == "-q:v"));
    }

    #[test]
    fn plan_png() {
        let (argv, out) = plan("in.heic", Format::Png);
        assert_eq!(out, "in.png");
        assert!(!argv.iter().any(|a| a == "-q:v"));
    }

    #[test]
    fn format_mime_and_ext() {
        assert_eq!(Format::Jpg.ext(), "jpg");
        assert_eq!(Format::Jpg.mime(), "image/jpeg");
        assert_eq!(Format::Png.ext(), "png");
        assert_eq!(Format::Png.mime(), "image/png");
    }
}
