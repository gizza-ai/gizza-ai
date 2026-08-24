//! cmyk-jpeg-to-rgb core — pure ffmpeg argv construction plus JPEG colour-space
//! detection, shared by the chat/CLI block and the standalone web page. No
//! wafer/wasm-bindgen deps.
//!
//! ## Why this is not just a format transcode
//!
//! A CMYK JPEG (four components) is what print workflows export. ffmpeg decodes
//! Adobe YCCK as `yuva444p` and plain Adobe CMYK as `gbrap` — in both cases the
//! black/K component lands in an ALPHA slot. A naive transcode therefore writes
//! an RGBA PNG carrying a pointless fully-opaque alpha channel. Pinning
//! `-pix_fmt rgb24` (PNG) / `yuvj420p`|`yuvj444p` (JPEG) forces a true
//! three-channel RGB result, which is the whole job of this tool.
//!
//! ## Chroma subsampling
//!
//! CMYK sources are usually print artwork — logos, type, flat colour — where
//! 4:2:0 chroma subsampling smears coloured edges. `chroma = "4:4:4"` keeps
//! full-resolution colour for JPEG output. PNG is always full RGB and libwebp
//! always writes 4:2:0, so the setting only changes JPEG.

/// Target RGB format this tool can write.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Png,
    Jpeg,
    Webp,
}

impl Format {
    /// Lower-cased file extension written for `out.<ext>`.
    pub fn ext(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg => "jpg",
            Format::Webp => "webp",
        }
    }

    /// MIME type of the encoded output.
    pub fn mime(self) -> &'static str {
        match self {
            Format::Png => "image/png",
            Format::Jpeg => "image/jpeg",
            Format::Webp => "image/webp",
        }
    }
}

/// Chroma subsampling for JPEG output.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Chroma {
    /// 4:2:0 — colour at half resolution. Smallest file, the web default.
    Half,
    /// 4:4:4 — full-resolution colour. Keeps print artwork's coloured edges crisp.
    Full,
}

/// Default output format: PNG. Lossless, and print-origin CMYK art is usually
/// flat colour and type where a lossy re-encode is the wrong default.
pub const DEFAULT_FORMAT: Format = Format::Png;

/// Default quality for the lossy formats (JPEG/WebP). Higher than the repo's
/// usual 85 because a CMYK original is normally a print master being re-used.
pub const DEFAULT_QUALITY: u8 = 90;

/// Default chroma subsampling: 4:2:0, matching every other JPEG writer here.
pub const DEFAULT_CHROMA: Chroma = Chroma::Half;

/// Parse the user-facing format string (the values the chat schema, CLI and
/// page all accept). JPEG is spelled `"jpeg"` to match the schema enum.
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim() {
        "" => Ok(DEFAULT_FORMAT),
        "png" => Ok(Format::Png),
        "jpeg" => Ok(Format::Jpeg),
        "webp" => Ok(Format::Webp),
        other => Err(format!("format {other:?} not supported (png|jpeg|webp)")),
    }
}

/// Parse the chroma subsampling string. Values are spelled with colons
/// (`"4:2:0"`) rather than as bare digits on purpose: the ffmpeg page coerces
/// numeric-LOOKING field strings to numbers before calling `build_argv`, so
/// `"420"` would arrive as a JS number and fail the string parameter.
pub fn parse_chroma(s: &str) -> Result<Chroma, String> {
    match s.trim() {
        "" => Ok(DEFAULT_CHROMA),
        "4:2:0" => Ok(Chroma::Half),
        "4:4:4" => Ok(Chroma::Full),
        other => Err(format!(
            "chroma {other:?} not supported (4:2:0|4:4:4)"
        )),
    }
}

/// Map web-conventional quality 1-100 to ffmpeg's `-q:v` range 31 (worst) – 2
/// (best). Identical to `gizza-ai/image-convert` / `image-compress` /
/// `png-to-jpg` so every tool here agrees on what "quality 90" means.
pub fn quality_to_qv(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let qv = 31.0 - (q - 1.0) * (29.0 / 99.0);
    qv.round().clamp(2.0, 31.0) as u8
}

/// The pixel format pinned for each output. This is the load-bearing part: it
/// is what collapses ffmpeg's 4-component CMYK/YCCK decode (`yuva444p` /
/// `gbrap`, K in the alpha slot) down to real RGB.
fn pix_fmt(format: Format, chroma: Chroma) -> &'static str {
    match (format, chroma) {
        (Format::Png, _) => "rgb24",
        (Format::Jpeg, Chroma::Half) => "yuvj420p",
        (Format::Jpeg, Chroma::Full) => "yuvj444p",
        // libwebp only writes 4:2:0; asking for anything else is silently
        // downgraded by the encoder, so pin the format it actually uses.
        (Format::Webp, _) => "yuv420p",
    }
}

/// Build the ffmpeg argv (no leading `"ffmpeg"`) and output filename that
/// convert `in_name` from CMYK/YCCK to RGB in `format`.
///
/// `quality` is 1-100 for the lossy formats; `0.0` means "unset" (the page
/// sends 0 for a cleared field) and falls back to [`DEFAULT_QUALITY`]. PNG is
/// lossless and ignores it. `chroma` only affects JPEG.
pub fn plan(
    in_name: &str,
    format: &str,
    quality: f64,
    chroma: &str,
) -> Result<(Vec<String>, String), String> {
    let fmt = parse_format(format)?;
    let chroma = parse_chroma(chroma)?;
    let q = if quality == 0.0 {
        DEFAULT_QUALITY
    } else if quality.is_finite() && (1.0..=100.0).contains(&quality) {
        quality.round() as u8
    } else {
        return Err(format!(
            "quality must be a number between 1 and 100, got {quality}"
        ));
    };

    let out_name = format!("out.{}", fmt.ext());
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        // A still image out: take one frame so an animated input (GIF/APNG/
        // animated WebP) converts cleanly instead of erroring.
        "-frames:v".to_string(),
        "1".to_string(),
        "-pix_fmt".to_string(),
        pix_fmt(fmt, chroma).to_string(),
    ];
    if fmt != Format::Png {
        argv.push("-q:v".to_string());
        argv.push(quality_to_qv(q).to_string());
    }
    argv.push(out_name.clone());
    Ok((argv, out_name))
}

/// What a JPEG's markers say its pixels are stored as.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum InputColor {
    /// 4 components, Adobe APP14 transform 2 — YCCK, the usual Photoshop CMYK export.
    AdobeYcck,
    /// 4 components, Adobe APP14 transform 0 — plain (inverted) CMYK.
    AdobeCmyk,
    /// 4 components with no Adobe marker.
    Cmyk,
    /// 3 components — already RGB/YCbCr.
    Rgb,
    /// 1 component — greyscale.
    Grayscale,
    /// Not a JPEG, or the markers could not be read.
    Unknown,
}

impl InputColor {
    /// Human-readable phrase for the result summary.
    pub fn label(self) -> &'static str {
        match self {
            InputColor::AdobeYcck => "4-component Adobe YCCK (CMYK)",
            InputColor::AdobeCmyk => "4-component Adobe CMYK",
            InputColor::Cmyk => "4-component CMYK",
            InputColor::Rgb => "3-component RGB/YCbCr",
            InputColor::Grayscale => "single-component greyscale",
            InputColor::Unknown => "an unrecognised colour layout",
        }
    }

    /// True when the input really carried four ink channels — i.e. this run is
    /// a genuine colour-space conversion rather than a plain re-encode.
    pub fn is_cmyk(self) -> bool {
        matches!(
            self,
            InputColor::AdobeYcck | InputColor::AdobeCmyk | InputColor::Cmyk
        )
    }
}

/// Read a JPEG's frame header + Adobe APP14 marker to report what colour space
/// the file actually stores. Non-JPEG input (PNG/WebP) returns
/// [`InputColor::Unknown`] — those formats are never CMYK here.
///
/// Deliberately a tolerant scan, not a parser: it walks the marker chain to the
/// start-of-frame and stops. No competitor reviewed reports this, and it is what
/// keeps the tool from passing off an already-RGB file as a conversion.
pub fn detect_input_color(bytes: &[u8]) -> InputColor {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return InputColor::Unknown;
    }
    let mut i = 2usize;
    let mut adobe_transform: Option<u8> = None;
    while i + 3 < bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = bytes[i + 1];
        // Standalone markers carry no length payload.
        if marker == 0x01 || marker == 0xD8 || (0xD0..=0xD7).contains(&marker) {
            i += 2;
            continue;
        }
        if marker == 0xD9 || marker == 0xDA {
            break; // end of image / start of scan — the frame header is behind us
        }
        let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if len < 2 || i + 2 + len > bytes.len() {
            return InputColor::Unknown;
        }
        let seg = &bytes[i + 4..i + 2 + len];
        // APP14 "Adobe": 'Adobe' + version(2) + flags0(2) + flags1(2) + transform(1)
        if marker == 0xEE && seg.len() >= 12 && seg.starts_with(b"Adobe") {
            adobe_transform = Some(seg[11]);
        }
        // Any SOFn except the non-frame DHT/JPG/DAC markers: [precision, h(2), w(2), ncomp]
        let is_sof = (0xC0..=0xCF).contains(&marker)
            && marker != 0xC4
            && marker != 0xC8
            && marker != 0xCC;
        if is_sof {
            if seg.len() < 6 {
                return InputColor::Unknown;
            }
            return match seg[5] {
                4 => match adobe_transform {
                    Some(2) => InputColor::AdobeYcck,
                    Some(_) => InputColor::AdobeCmyk,
                    None => InputColor::Cmyk,
                },
                3 => InputColor::Rgb,
                1 => InputColor::Grayscale,
                _ => InputColor::Unknown,
            };
        }
        i += 2 + len;
    }
    InputColor::Unknown
}

/// One-line result summary for the chat/CLI envelope.
pub fn summarize(color: InputColor, format: Format, out_bytes: usize) -> String {
    let what = if color.is_cmyk() {
        format!("converted {} to RGB", color.label())
    } else {
        format!("re-encoded {} (no CMYK data to convert)", color.label())
    };
    format!("{what}; wrote {} ({out_bytes} bytes)", format.mime())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag_value(argv: &[String], flag: &str) -> Option<String> {
        argv.iter()
            .position(|a| a == flag)
            .map(|i| argv[i + 1].clone())
    }

    // ---- happy path -------------------------------------------------------

    #[test]
    fn default_plan_is_lossless_rgb24_png() {
        let (argv, out) = plan("in.jpg", "", 0.0, "").unwrap();
        assert_eq!(out, "out.png");
        // The whole point: never let the CMYK K channel survive as alpha.
        assert_eq!(flag_value(&argv, "-pix_fmt").unwrap(), "rgb24");
        assert!(
            flag_value(&argv, "-q:v").is_none(),
            "PNG is lossless and must not carry -q:v: {argv:?}"
        );
        assert_eq!(flag_value(&argv, "-frames:v").unwrap(), "1");
        assert_eq!(argv.last().unwrap(), "out.png");
    }

    #[test]
    fn jpeg_uses_full_range_pixfmt_and_quality() {
        let (argv, out) = plan("in.jpg", "jpeg", 90.0, "4:2:0").unwrap();
        assert_eq!(out, "out.jpg");
        assert_eq!(flag_value(&argv, "-pix_fmt").unwrap(), "yuvj420p");
        // quality 90 → -q:v 5 (31 - 89*29/99 ≈ 4.9)
        assert_eq!(flag_value(&argv, "-q:v").unwrap(), "5");
    }

    #[test]
    fn chroma_444_selects_full_chroma_jpeg() {
        let (argv, _) = plan("in.jpg", "jpeg", 90.0, "4:4:4").unwrap();
        assert_eq!(flag_value(&argv, "-pix_fmt").unwrap(), "yuvj444p");
    }

    #[test]
    fn chroma_is_ignored_for_png_and_webp() {
        let (png, _) = plan("in.jpg", "png", 0.0, "4:4:4").unwrap();
        assert_eq!(flag_value(&png, "-pix_fmt").unwrap(), "rgb24");
        let (webp, out) = plan("in.jpg", "webp", 90.0, "4:4:4").unwrap();
        assert_eq!(out, "out.webp");
        assert_eq!(flag_value(&webp, "-pix_fmt").unwrap(), "yuv420p");
        assert_eq!(flag_value(&webp, "-q:v").unwrap(), "5");
    }

    #[test]
    fn cleared_quality_field_falls_back_to_default() {
        let (a, _) = plan("in.jpg", "jpeg", 0.0, "4:2:0").unwrap();
        let (b, _) = plan("in.jpg", "jpeg", DEFAULT_QUALITY as f64, "4:2:0").unwrap();
        assert_eq!(flag_value(&a, "-q:v"), flag_value(&b, "-q:v"));
    }

    #[test]
    fn quality_endpoints_map_to_qv_bounds() {
        assert_eq!(quality_to_qv(100), 2);
        assert_eq!(quality_to_qv(1), 31);
    }

    // ---- error path -------------------------------------------------------

    #[test]
    fn unknown_format_is_rejected() {
        let err = plan("in.jpg", "tiff", 0.0, "").unwrap_err();
        assert!(err.contains("tiff"), "{err}");
        assert!(err.contains("png|jpeg|webp"), "{err}");
        // "jpg" is the extension, not the schema value — must not silently pass.
        assert!(plan("in.jpg", "jpg", 0.0, "").is_err());
    }

    #[test]
    fn unknown_chroma_is_rejected() {
        let err = plan("in.jpg", "jpeg", 90.0, "4:1:1").unwrap_err();
        assert!(err.contains("4:2:0|4:4:4"), "{err}");
    }

    #[test]
    fn out_of_range_quality_is_rejected() {
        assert!(plan("in.jpg", "jpeg", 101.0, "").is_err());
        assert!(plan("in.jpg", "jpeg", -1.0, "").is_err());
        assert!(plan("in.jpg", "jpeg", f64::NAN, "").is_err());
    }

    // ---- colour-space detection ------------------------------------------

    /// Minimal JPEG marker chain: SOI, optional APP14 Adobe, SOF0, SOS.
    fn fake_jpeg(components: u8, adobe_transform: Option<u8>) -> Vec<u8> {
        let mut d = vec![0xFF, 0xD8];
        if let Some(t) = adobe_transform {
            let payload: Vec<u8> = b"Adobe"
                .iter()
                .copied()
                .chain([0x00, 0x64, 0x40, 0x00, 0x00, 0x00, t])
                .collect();
            d.extend_from_slice(&[0xFF, 0xEE]);
            d.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
            d.extend_from_slice(&payload);
        }
        // SOF0: precision, height, width, ncomp (component specs omitted — the
        // detector only reads the fixed head of the segment).
        let sof = [0x08, 0x00, 0x40, 0x00, 0x40, components];
        d.extend_from_slice(&[0xFF, 0xC0]);
        d.extend_from_slice(&((sof.len() + 2) as u16).to_be_bytes());
        d.extend_from_slice(&sof);
        d.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x02]);
        d
    }

    #[test]
    fn detects_adobe_ycck_cmyk_rgb_and_gray() {
        assert_eq!(
            detect_input_color(&fake_jpeg(4, Some(2))),
            InputColor::AdobeYcck
        );
        assert_eq!(
            detect_input_color(&fake_jpeg(4, Some(0))),
            InputColor::AdobeCmyk
        );
        assert_eq!(detect_input_color(&fake_jpeg(4, None)), InputColor::Cmyk);
        assert_eq!(detect_input_color(&fake_jpeg(3, None)), InputColor::Rgb);
        assert_eq!(
            detect_input_color(&fake_jpeg(1, None)),
            InputColor::Grayscale
        );
    }

    #[test]
    fn non_jpeg_input_is_unknown_not_a_panic() {
        // PNG magic, a truncated JPEG, and empty input must all be tolerated.
        assert_eq!(
            detect_input_color(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A]),
            InputColor::Unknown
        );
        assert_eq!(detect_input_color(&[0xFF, 0xD8]), InputColor::Unknown);
        assert_eq!(detect_input_color(&[]), InputColor::Unknown);
        assert!(!InputColor::Unknown.is_cmyk());
    }

    #[test]
    fn summary_distinguishes_conversion_from_reencode() {
        let converted = summarize(InputColor::AdobeYcck, Format::Png, 2606);
        assert!(converted.contains("converted"), "{converted}");
        assert!(converted.contains("image/png"), "{converted}");
        let reencoded = summarize(InputColor::Rgb, Format::Jpeg, 1983);
        assert!(reencoded.contains("no CMYK data"), "{reencoded}");
    }
}
