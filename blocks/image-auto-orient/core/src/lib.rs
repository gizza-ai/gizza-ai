//! gizza-ai/image-auto-orient core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! A camera writes the photo's pixels in the sensor's own order and records how
//! the phone was held in the EXIF `Orientation` tag (values 1-8). Viewers that
//! honour the tag show the photo upright; viewers that ignore it (many web
//! uploads, older editors, some print pipelines) show it sideways or mirrored.
//! This tool BAKES the correction into the pixels so the image is upright
//! everywhere, and the resulting file carries no orientation tag at all — so it
//! can never be double-rotated by a viewer that does honour the tag.
//!
//! Two paths, both a single ffmpeg invocation:
//!
//! - `orientation = "auto"` (default) — ffmpeg's own EXIF autorotation is on by
//!   default, so plain `-i in.jpg out.jpg` already applies the tag's transform
//!   (verified for all 8 values on both surfaces: native ffmpeg 7.1 and the
//!   page's @ffmpeg/core 0.12.10, including the mirrored values 2/4/5/7).
//! - `orientation = "1".."8"` — force a transform when the tag is missing,
//!   wrong, or was already stripped. `-noautorotate` goes BEFORE `-i` so any
//!   existing tag is ignored, and the equivalent filter chain is applied
//!   explicitly.
//!
//! The EXIF value names the transform needed to DISPLAY the stored pixels
//! upright, which is exactly what the filter chain applies:
//!
//! | value | meaning                         | filter          |
//! |-------|---------------------------------|-----------------|
//! | 1     | already upright                 | (none)          |
//! | 2     | mirrored left-right             | `hflip`         |
//! | 3     | rotated 180°                    | `hflip,vflip`   |
//! | 4     | mirrored top-bottom             | `vflip`         |
//! | 5     | mirrored + rotated 90° CW       | `transpose=0`   |
//! | 6     | rotated 90° clockwise           | `transpose=1`   |
//! | 7     | mirrored + rotated 90° CCW      | `transpose=3`   |
//! | 8     | rotated 90° counter-clockwise   | `transpose=2`   |

/// How the correction is chosen: from the file's own EXIF tag, or forced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Read the EXIF `Orientation` tag and apply it (ffmpeg autorotation).
    Auto,
    /// Ignore any tag and apply EXIF orientation value `1..=8` explicitly.
    Exif(u8),
}

impl Orientation {
    /// Parse the descriptor enum value (`"auto"` or `"1"`..`"8"`).
    pub fn parse(s: &str) -> Result<Orientation, String> {
        let t = s.trim().to_ascii_lowercase();
        if t.is_empty() || t == "auto" {
            return Ok(Orientation::Auto);
        }
        match t.parse::<u8>() {
            Ok(v @ 1..=8) => Ok(Orientation::Exif(v)),
            _ => Err(format!(
                "orientation must be \"auto\" or an EXIF orientation value 1-8, got {s:?}"
            )),
        }
    }

    /// The ffmpeg filter chain that applies this orientation, or `None` when no
    /// pixels need moving (auto, or an explicit "already upright" 1).
    pub fn filter(self) -> Option<&'static str> {
        match self {
            Orientation::Auto => None,
            Orientation::Exif(v) => match v {
                2 => Some("hflip"),
                3 => Some("hflip,vflip"),
                4 => Some("vflip"),
                5 => Some("transpose=0"),
                6 => Some("transpose=1"),
                7 => Some("transpose=3"),
                8 => Some("transpose=2"),
                // 1 = identity; anything else is rejected by `parse`.
                _ => None,
            },
        }
    }
}

/// Output container. `Same` keeps the uploaded file's format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutFormat {
    Same,
    Jpeg,
    Png,
    Webp,
}

impl OutFormat {
    /// Parse the descriptor enum value.
    pub fn parse(s: &str) -> Result<OutFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "same" => Ok(OutFormat::Same),
            "jpeg" | "jpg" => Ok(OutFormat::Jpeg),
            "png" => Ok(OutFormat::Png),
            "webp" => Ok(OutFormat::Webp),
            other => Err(format!(
                "format {other:?} not supported (same|jpeg|png|webp)"
            )),
        }
    }

    /// The output file extension, given the input file's extension for `Same`.
    pub fn ext(self, in_ext: &str) -> &str {
        match self {
            OutFormat::Same => in_ext,
            OutFormat::Jpeg => "jpg",
            OutFormat::Png => "png",
            OutFormat::Webp => "webp",
        }
    }
}

/// Lower-cased extension of `in_name`, or an error when it has none (the output
/// container is inferred from it whenever `format = same`).
pub fn ext_of(in_name: &str) -> Result<String, String> {
    in_name
        .rsplit('.')
        .next()
        .filter(|e| !e.is_empty() && *e != in_name)
        .map(|e| e.to_ascii_lowercase())
        .ok_or_else(|| {
            format!("input filename {in_name:?} has no extension; cannot infer the image format")
        })
}

/// Map web-conventional quality 1-100 to ffmpeg's JPEG `-q:v` range 31 (worst)
/// – 2 (best). Mirrors `gizza-ai/image-convert`'s `quality_to_qv` so the tools
/// agree on what "quality 85" means.
pub fn quality_to_qv(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let qv = 31.0 - (q - 1.0) * (29.0 / 99.0);
    qv.round().clamp(2.0, 31.0) as u8
}

/// The encoder flag pair for `out_ext` at `quality`, or `None` for formats where
/// quality does not apply (PNG is lossless; anything else is left to ffmpeg's
/// own default so a `same`-format passthrough of e.g. TIFF/BMP can't be broken
/// by an encoder flag it doesn't understand).
fn quality_args(out_ext: &str, quality: u8) -> Option<(String, String)> {
    match out_ext {
        "jpg" | "jpeg" => Some(("-q:v".into(), quality_to_qv(quality).to_string())),
        // libwebp takes the 1-100 scale directly.
        "webp" => Some(("-quality".into(), quality.to_string())),
        _ => None,
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) and output filename.
///
/// `orientation` is `auto` or `1`-`8`, `format` is `same|jpeg|png|webp`,
/// `quality` is 1-100 (applied to JPEG/WebP output only). Returns
/// `(argv, out_name)`.
pub fn plan(
    orientation: &str,
    format: &str,
    quality: u8,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    if !(1..=100).contains(&quality) {
        return Err(format!("quality must be 1-100, got {quality}"));
    }
    let orient = Orientation::parse(orientation)?;
    let fmt = OutFormat::parse(format)?;
    let in_ext = ext_of(in_name)?;
    let out_ext = fmt.ext(&in_ext).to_string();
    let out_name = format!("out.{out_ext}");

    let mut argv: Vec<String> = Vec::new();
    // `-noautorotate` is an INPUT option: it must precede `-i`.
    if orient != Orientation::Auto {
        argv.push("-noautorotate".into());
    }
    argv.push("-i".into());
    argv.push(in_name.to_string());
    if let Some(chain) = orient.filter() {
        argv.push("-vf".into());
        argv.push(chain.to_string());
    }
    if let Some((flag, value)) = quality_args(&out_ext, quality) {
        argv.push(flag);
        argv.push(value);
    }
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
    fn auto_is_a_plain_transcode_with_autorotation_left_on() {
        let (argv, out) = plan("auto", "same", 90, "in.jpg").unwrap();
        assert_eq!(out, "out.jpg");
        // No -noautorotate: ffmpeg applies the EXIF tag itself.
        assert!(!argv.contains(&"-noautorotate".to_string()), "{argv:?}");
        assert!(!argv.contains(&"-vf".to_string()), "{argv:?}");
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "in.jpg");
        assert_eq!(flag_value(&argv, "-q:v").as_deref(), Some("5"));
        assert_eq!(argv.last().unwrap(), "out.jpg");
    }

    #[test]
    fn forced_orientation_disables_autorotate_before_the_input() {
        let (argv, _) = plan("6", "same", 90, "in.jpg").unwrap();
        let na = argv.iter().position(|a| a == "-noautorotate").unwrap();
        let i = argv.iter().position(|a| a == "-i").unwrap();
        assert!(na < i, "-noautorotate must be an input option: {argv:?}");
        assert_eq!(flag_value(&argv, "-vf").as_deref(), Some("transpose=1"));
    }

    #[test]
    fn every_exif_value_maps_to_its_documented_filter() {
        let expected = [
            (1, None),
            (2, Some("hflip")),
            (3, Some("hflip,vflip")),
            (4, Some("vflip")),
            (5, Some("transpose=0")),
            (6, Some("transpose=1")),
            (7, Some("transpose=3")),
            (8, Some("transpose=2")),
        ];
        for (value, chain) in expected {
            let (argv, _) = plan(&value.to_string(), "same", 90, "in.jpg").unwrap();
            assert_eq!(flag_value(&argv, "-vf").as_deref(), chain, "exif {value}");
            // Even the identity case must drop the tag rather than trust it.
            assert!(argv.contains(&"-noautorotate".to_string()), "exif {value}");
        }
    }

    #[test]
    fn format_overrides_the_output_container() {
        assert_eq!(plan("auto", "png", 90, "in.jpg").unwrap().1, "out.png");
        assert_eq!(plan("auto", "webp", 90, "photo.jpeg").unwrap().1, "out.webp");
        assert_eq!(plan("auto", "jpeg", 90, "in.png").unwrap().1, "out.jpg");
        assert_eq!(plan("auto", "same", 90, "in.webp").unwrap().1, "out.webp");
    }

    #[test]
    fn quality_applies_to_jpeg_and_webp_only() {
        let (jpg, _) = plan("auto", "jpeg", 100, "in.png").unwrap();
        assert_eq!(flag_value(&jpg, "-q:v").as_deref(), Some("2"));
        let (webp, _) = plan("auto", "webp", 55, "in.png").unwrap();
        assert_eq!(flag_value(&webp, "-quality").as_deref(), Some("55"));
        let (png, _) = plan("auto", "png", 55, "in.jpg").unwrap();
        assert!(!png.contains(&"-q:v".to_string()), "{png:?}");
        assert!(!png.contains(&"-quality".to_string()), "{png:?}");
        // An unusual `same`-format container gets no encoder flag it may not know.
        let (tif, out) = plan("auto", "same", 55, "scan.tif").unwrap();
        assert_eq!(out, "out.tif");
        assert!(!tif.contains(&"-q:v".to_string()), "{tif:?}");
    }

    #[test]
    fn quality_maps_the_web_scale_onto_ffmpegs_inverted_jpeg_scale() {
        assert_eq!(quality_to_qv(100), 2);
        assert_eq!(quality_to_qv(1), 31);
        assert!(quality_to_qv(85) < quality_to_qv(40), "higher quality = lower -q:v");
    }

    #[test]
    fn rejects_bad_input() {
        assert!(plan("auto", "same", 0, "in.jpg").is_err(), "quality 0");
        assert!(plan("auto", "same", 101, "in.jpg").is_err(), "quality 101");
        assert!(plan("9", "same", 90, "in.jpg").is_err(), "exif 9");
        assert!(plan("0", "same", 90, "in.jpg").is_err(), "exif 0");
        assert!(plan("sideways", "same", 90, "in.jpg").is_err());
        assert!(plan("auto", "tiff", 90, "in.jpg").is_err(), "unsupported format");
        assert!(plan("auto", "same", 90, "noextension").is_err());
        // The error text names the accepted values.
        let err = plan("9", "same", 90, "in.jpg").unwrap_err();
        assert!(err.contains("1-8"), "{err}");
    }

    #[test]
    fn parse_accepts_case_and_whitespace_variants() {
        assert_eq!(Orientation::parse(" AUTO ").unwrap(), Orientation::Auto);
        assert_eq!(Orientation::parse("6").unwrap(), Orientation::Exif(6));
        assert_eq!(OutFormat::parse("JPG").unwrap(), OutFormat::Jpeg);
        assert_eq!(OutFormat::parse("").unwrap(), OutFormat::Same);
    }
}
