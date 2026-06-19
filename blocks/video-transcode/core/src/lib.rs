//! gizza-ai/video-transcode core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! video-transcode re-encodes a video into a DIFFERENT container/codec target
//! (mp4 = H.264/AAC, webm = VP9/Opus). Unlike video-compress (which keeps the
//! input container), the output extension is the user-chosen target format. A
//! web-conventional `quality` 1-100 maps to ffmpeg's CRF scale.

/// Target container format video-transcode can transcode to.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Mp4,
    Webm,
}

impl Format {
    /// Lower-cased file extension this format writes (used for `out.<ext>`).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Mp4 => "mp4",
            Format::Webm => "webm",
        }
    }
}

/// Parse the user-facing format string (the values the chat schema + page accept).
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s {
        "mp4" => Ok(Format::Mp4),
        "webm" => Ok(Format::Webm),
        other => Err(format!("format {other:?} not supported (mp4|webm)")),
    }
}

/// Default quality used when none is supplied.
pub const DEFAULT_QUALITY: u8 = 75;

/// Map web-conventional quality 1-100 to ffmpeg's CRF range 0 (best) – 51 (worst).
pub fn quality_to_crf(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let crf = 51.0 - (q - 1.0) * (51.0 / 99.0);
    crf.round().clamp(0.0, 51.0) as u8
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to transcode `in_name` to
/// `out_name` in `format` at the given `crf`. mp4 → libx264/aac (+faststart),
/// webm → libvpx-vp9/libopus (`-b:v 0` for constant-quality VP9).
pub fn build_argv(in_name: &str, out_name: &str, format: Format, crf: u8) -> Vec<String> {
    match format {
        Format::Mp4 => vec![
            "-i".into(),
            in_name.into(),
            "-c:v".into(),
            "libx264".into(),
            "-c:a".into(),
            "aac".into(),
            "-crf".into(),
            crf.to_string(),
            "-movflags".into(),
            "+faststart".into(),
            out_name.into(),
        ],
        Format::Webm => vec![
            "-i".into(),
            in_name.into(),
            "-c:v".into(),
            "libvpx-vp9".into(),
            "-c:a".into(),
            "libopus".into(),
            "-crf".into(),
            crf.to_string(),
            "-b:v".into(),
            "0".into(),
            out_name.into(),
        ],
    }
}

/// Validate `quality`, parse `format`, and build `(argv, out_name)` for an input
/// file. `out_name` uses the TARGET format's extension. Single source shared by
/// the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_transcode(
    format: &str,
    quality: u8,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    if !(1..=100).contains(&quality) {
        return Err(format!("quality must be 1-100, got {quality}"));
    }
    let fmt = parse_format(format)?;
    let crf = quality_to_crf(quality);
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, fmt, crf), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_format_known() {
        assert_eq!(parse_format("mp4").unwrap(), Format::Mp4);
        assert_eq!(parse_format("webm").unwrap(), Format::Webm);
    }

    #[test]
    fn parse_format_rejects_unknown() {
        assert!(parse_format("mov").is_err());
        assert!(parse_format("avi").is_err());
    }

    #[test]
    fn quality_to_crf_endpoints() {
        assert_eq!(quality_to_crf(100), 0);
        assert_eq!(quality_to_crf(1), 51);
        let mid = quality_to_crf(75);
        assert!((12..=14).contains(&mid), "expected 12-14, got {mid}");
    }

    #[test]
    fn argv_mp4_uses_h264_aac_and_faststart() {
        let argv = build_argv("in.mp4", "out.mp4", Format::Mp4, 13);
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "in.mp4");
        assert!(argv.iter().any(|a| a == "libx264"));
        assert!(argv.iter().any(|a| a == "aac"));
        assert!(argv.iter().any(|a| a == "13"));
        assert!(argv.iter().any(|a| a == "+faststart"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn argv_webm_uses_vp9_opus_and_constant_quality() {
        let argv = build_argv("in.mp4", "out.webm", Format::Webm, 13);
        assert!(argv.iter().any(|a| a == "libvpx-vp9"));
        assert!(argv.iter().any(|a| a == "libopus"));
        assert!(argv.iter().any(|a| a == "13"));
        assert!(argv.windows(2).any(|w| w[0] == "-b:v" && w[1] == "0"));
        assert_eq!(argv.last().map(String::as_str), Some("out.webm"));
    }

    #[test]
    fn plan_transcode_uses_target_extension() {
        let (_argv, out) = plan_transcode("mp4", 75, "clip.webm").unwrap();
        assert_eq!(out, "out.mp4");
        let (_argv, out) = plan_transcode("webm", 75, "clip.mp4").unwrap();
        assert_eq!(out, "out.webm");
    }

    #[test]
    fn plan_transcode_rejects_out_of_range_quality_and_bad_format() {
        assert!(plan_transcode("mp4", 0, "a.mp4").is_err());
        assert!(plan_transcode("mp4", 101, "a.mp4").is_err());
        assert!(plan_transcode("mov", 75, "a.mp4").is_err());
    }
}
