//! gizza-ai/video-frame-extract core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Seek to a timestamp and grab a single video frame, encoded as PNG. The input
//! is a video (any container ffmpeg can demux) but the output is always an
//! image (`out.png`), so the page renders the result as an `<img>` even though
//! the file input accepts `video/*`.

/// The fixed output filename — always PNG regardless of the input container.
pub const OUTPUT_NAME: &str = "out.png";

/// Validate a timestamp (seconds). Must be finite and `>= 0`.
pub fn validate_timestamp(timestamp: f64) -> Result<(), String> {
    if !timestamp.is_finite() || timestamp < 0.0 {
        return Err(format!(
            "timestamp must be >= 0 and finite, got {timestamp}"
        ));
    }
    Ok(())
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to seek to `timestamp` seconds
/// and write a single frame to `out_name` as PNG. `-ss` precedes `-i` for a
/// fast input-seek; `-frames:v 1 -update 1` writes exactly one image.
pub fn build_argv(in_name: &str, out_name: &str, timestamp: f64) -> Vec<String> {
    vec![
        "-ss".into(),
        format!("{timestamp}"),
        "-i".into(),
        in_name.into(),
        "-frames:v".into(),
        "1".into(),
        "-update".into(),
        "1".into(),
        "-y".into(),
        out_name.into(),
    ]
}

/// Validate the timestamp and return `(argv, out_name)` for an input file.
/// The output is always `out.png` (the extracted frame is a PNG image). Used by
/// the web page (video file in → image file out).
pub fn plan_extract(in_name: &str, timestamp: f64) -> Result<(Vec<String>, String), String> {
    validate_timestamp(timestamp)?;
    let out_name = OUTPUT_NAME.to_string();
    Ok((build_argv(in_name, &out_name, timestamp), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_seeks_before_input_and_grabs_one_frame() {
        let argv = build_argv("in.mp4", "out.png", 5.0);
        assert_eq!(argv[0], "-ss");
        assert_eq!(argv[1], "5");
        assert_eq!(argv[2], "-i");
        assert_eq!(argv[3], "in.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-frames:v" && w[1] == "1"));
        assert!(argv.windows(2).any(|w| w[0] == "-update" && w[1] == "1"));
        assert_eq!(argv.last().map(String::as_str), Some("out.png"));
    }

    #[test]
    fn argv_renders_fractional_timestamp() {
        let argv = build_argv("in.webm", "out.png", 1.5);
        assert_eq!(argv[1], "1.5");
    }

    #[test]
    fn validate_timestamp_accepts_zero_and_positive() {
        assert!(validate_timestamp(0.0).is_ok());
        assert!(validate_timestamp(12.5).is_ok());
    }

    #[test]
    fn validate_timestamp_rejects_negative_and_non_finite() {
        assert!(validate_timestamp(-1.0).is_err());
        assert!(validate_timestamp(f64::NAN).is_err());
        assert!(validate_timestamp(f64::INFINITY).is_err());
    }

    #[test]
    fn plan_extract_outputs_png_and_validates() {
        let (argv, out) = plan_extract("clip.mov", 3.0).unwrap();
        assert_eq!(out, "out.png");
        assert_eq!(argv[0], "-ss");
        assert_eq!(argv[1], "3");
        assert!(argv.iter().any(|a| a == "clip.mov"));
        // Output is always PNG regardless of input container.
        assert_eq!(argv.last().map(String::as_str), Some("out.png"));
        assert!(plan_extract("clip.mp4", -2.0).is_err());
        assert!(plan_extract("clip.mp4", f64::NAN).is_err());
    }
}
