//! gizza-ai/video-autocrop-bars core — pure pieces of the **two-pass** autocrop
//! flow shared by the chat block, the CLI, and the standalone page.
//!
//! # Why two-pass
//!
//! ffmpeg's `cropdetect` filter only *measures* the black bars (it logs a
//! running `crop=W:H:X:Y` suggestion per frame); the `crop` filter that actually
//! removes them needs those numbers up front. So the tool is:
//!
//!   1. **detect** — `-vf cropdetect=limit=<threshold>:round=<round>:reset=0
//!      -f null -` over the whole clip. `reset=0` keeps the running MAXIMUM box,
//!      so the last logged `crop=` line is the union of all content seen — a
//!      fade-from-black can only grow (never shrink) the kept picture.
//!   2. **crop** — `-vf crop=W:H:X:Y` + H.264/AAC re-encode (a stream copy can
//!      never crop: filters require re-encoding).
//!
//! This module owns the pure parts: argv builders, the cropdetect log parser,
//! and the crop/no-bars/error decision. The block dispatches ffmpeg twice
//! (`video-silence-cut` precedent); the page mirrors it in `page/custom.js`
//! (`video-target-filesize-encoder` precedent).
//!
//! Container rule (family invariant): the input container is kept when it can
//! hold H.264 + AAC (mp4/mov/m4v/mkv, audio stream-copied); anything else
//! (webm, …) switches to mp4 and the audio is re-encoded to AAC — see
//! `gizza_ai_block_utils::ffmpeg::h264_out_ext`.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;

/// cropdetect `limit` bounds (8-bit luma). 24 is ffmpeg's own default: safe for
/// true-black bars without eating dark scene content.
pub const MIN_THRESHOLD: f64 = 0.0;
pub const MAX_THRESHOLD: f64 = 255.0;
pub const DEFAULT_THRESHOLD: f64 = 24.0;

/// Allowed `round` choices (snap crop dims to a multiple). 2 removes the most
/// bar while keeping H.264/yuv420p-legal even dimensions; 16 is the classic
/// encoder-macroblock-friendly value.
pub const ROUNDS: [&str; 4] = ["2", "4", "8", "16"];
pub const DEFAULT_ROUND: &str = "2";

/// Validate the two params, normalizing to `(threshold_u32, round_u32)`.
pub fn validate(threshold: f64, round: &str) -> Result<(u32, u32), String> {
    if !threshold.is_finite() || !(MIN_THRESHOLD..=MAX_THRESHOLD).contains(&threshold) {
        return Err(format!(
            "threshold must be between {MIN_THRESHOLD:.0} and {MAX_THRESHOLD:.0} (got {threshold})"
        ));
    }
    let round_n: u32 = match round.trim() {
        "2" => 2,
        "4" => 4,
        "8" => 8,
        "16" => 16,
        other => return Err(format!("round must be one of 2, 4, 8, 16 (got '{other}')")),
    };
    Ok((threshold.round() as u32, round_n))
}

/// Pass-1 argv (no leading "ffmpeg"): measure the bars, no output file. The
/// result is the LOG (the ffmpeg bridge returns it even though `-f null -`
/// writes no file).
pub fn detect_argv(in_name: &str, threshold: u32, round: u32) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-vf".into(),
        format!("cropdetect=limit={threshold}:round={round}:reset=0"),
        "-f".into(),
        "null".into(),
        "-".into(),
    ]
}

/// The LAST `crop=W:H:X:Y` suggestion in a cropdetect log — with `reset=0`
/// that's the accumulated union box. Values are signed: cropdetect emits
/// NEGATIVE w/h when the whole frame reads as black (measured on ffmpeg 6.1:
/// `crop=-318:-238:320:240` at limit=255).
pub fn parse_last_crop(log: &str) -> Option<(i64, i64, i64, i64)> {
    let mut last = None;
    for line in log.lines() {
        // Token is ` crop=W:H:X:Y` at the end of each Parsed_cropdetect line.
        if let Some(pos) = line.rfind("crop=") {
            let spec = line[pos + 5..].trim();
            let parts: Vec<_> = spec.split(':').collect();
            if parts.len() == 4 {
                if let (Ok(w), Ok(h), Ok(x), Ok(y)) = (
                    parts[0].parse::<i64>(),
                    parts[1].parse::<i64>(),
                    parts[2].parse::<i64>(),
                    parts[3].parse::<i64>(),
                ) {
                    last = Some((w, h, x, y));
                }
            }
        }
    }
    last
}

/// The input video's `WxH` from the first `Stream … Video:` log line, e.g.
/// `Stream #0:0[0x1](und): Video: h264 …, yuv420p(progressive), 320x240 [SAR …`.
/// Guards against hex-id lookalikes on the same line (`[0x1]`,
/// `avc1 / 0x31637661` both parse as `0x…` → rejected because w must be ≥ 1).
pub fn parse_input_dims(log: &str) -> Option<(u32, u32)> {
    for line in log.lines() {
        if !line.contains("Video:") {
            continue;
        }
        for token in line.split(|c: char| c == ' ' || c == ',') {
            if let Some((w_s, h_s)) = token.split_once('x') {
                if let (Ok(w), Ok(h)) = (w_s.parse::<u32>(), h_s.parse::<u32>()) {
                    if (1..=16384).contains(&w) && (1..=16384).contains(&h) {
                        return Some((w, h));
                    }
                }
            }
        }
    }
    None
}

/// What pass 1 concluded.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// The detected box covers the whole frame — nothing to crop.
    NoBars { in_w: u32, in_h: u32 },
    /// Crop `w`x`h` at offset (`x`,`y`) out of an `in_w`x`in_h` frame.
    Crop { w: u32, h: u32, x: u32, y: u32, in_w: u32, in_h: u32 },
}

/// Turn a cropdetect log into a decision. Errors are user-facing strings.
pub fn decide(log: &str) -> Result<Decision, String> {
    let (in_w, in_h) = parse_input_dims(log)
        .ok_or_else(|| "could not read the video dimensions from ffmpeg output".to_string())?;
    let (w, h, x, y) = parse_last_crop(log).ok_or_else(|| {
        "could not detect bars — ffmpeg produced no cropdetect output (is this a valid video?)"
            .to_string()
    })?;
    if w <= 0 || h <= 0 {
        return Err(
            "the whole frame reads as black at this threshold — lower the threshold and try again"
                .to_string(),
        );
    }
    let (w, h) = (w as u32, h as u32);
    let (x, y) = (x.max(0) as u32, y.max(0) as u32);
    if w >= in_w && h >= in_h {
        return Ok(Decision::NoBars { in_w, in_h });
    }
    Ok(Decision::Crop { w, h, x, y, in_w, in_h })
}

/// Pass-2 `(argv, out_name)`: apply the crop and re-encode. CRF 18 (visually
/// lossless tier — the point of this tool is bar removal, not compression);
/// audio stream-copied when the container is kept, AAC on a container switch.
pub fn crop_argv(in_name: &str, w: u32, h: u32, x: u32, y: u32) -> (Vec<String>, String) {
    let (ext, transcode_audio) = h264_out_ext(in_name);
    let out_name = format!("out.{ext}");
    let argv = vec![
        "-i".into(),
        in_name.into(),
        "-vf".into(),
        format!("crop={w}:{h}:{x}:{y}"),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "medium".into(),
        "-crf".into(),
        "18".into(),
        "-c:a".into(),
        if transcode_audio { "aac".into() } else { "copy".into() },
        out_name.clone(),
    ];
    (argv, out_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_LOG: &str = "\
Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'in.mp4':
  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(progressive), 320x240 [SAR 1:1 DAR 4:3], 142 kb/s, 10 fps, 10 tbr, 10240 tbn (default)
[Parsed_cropdetect_0 @ 0x55] x1:0 x2:319 y1:30 y2:199 w:320 h:170 x:0 y:30 pts:1024 t:0.1 limit:24.000000 crop=320:170:0:34
[Parsed_cropdetect_0 @ 0x55] x1:0 x2:319 y1:30 y2:209 w:320 h:180 x:0 y:30 pts:19456 t:1.9 limit:24.000000 crop=320:180:0:30
";

    // --- validate -------------------------------------------------------------

    #[test]
    fn validate_accepts_defaults_and_all_rounds() {
        assert_eq!(validate(DEFAULT_THRESHOLD, DEFAULT_ROUND), Ok((24, 2)));
        for (s, n) in [("2", 2u32), ("4", 4), ("8", 8), ("16", 16)] {
            assert_eq!(validate(24.0, s), Ok((24, n)));
        }
        assert_eq!(validate(0.0, "2"), Ok((0, 2)));
        assert_eq!(validate(255.0, "2"), Ok((255, 2)));
    }

    #[test]
    fn validate_rejects_bad_threshold_and_round() {
        assert!(validate(-1.0, "2").is_err());
        assert!(validate(256.0, "2").is_err());
        assert!(validate(f64::NAN, "2").is_err());
        assert!(validate(24.0, "3").is_err());
        assert!(validate(24.0, "").is_err());
    }

    // --- detect_argv ----------------------------------------------------------

    #[test]
    fn detect_argv_builds_cropdetect_null_sink() {
        assert_eq!(
            detect_argv("in.mp4", 24, 2),
            vec!["-i", "in.mp4", "-vf", "cropdetect=limit=24:round=2:reset=0", "-f", "null", "-"]
        );
    }

    // --- parsers --------------------------------------------------------------

    #[test]
    fn parse_last_crop_takes_the_accumulated_final_line() {
        assert_eq!(parse_last_crop(SAMPLE_LOG), Some((320, 180, 0, 30)));
    }

    #[test]
    fn parse_last_crop_handles_negative_whole_frame_black() {
        // Measured on ffmpeg 6.1 at limit=255.
        let log = "[Parsed_cropdetect_0 @ 0x55] ... crop=-318:-238:320:240\n";
        assert_eq!(parse_last_crop(log), Some((-318, -238, 320, 240)));
    }

    #[test]
    fn parse_last_crop_none_on_log_without_cropdetect() {
        assert_eq!(parse_last_crop("frame=  20 fps=0.0 q=-0.0 size=N/A\n"), None);
    }

    #[test]
    fn parse_input_dims_skips_hex_ids_on_the_stream_line() {
        // `[0x1]` and `avc1 / 0x31637661` both contain digit-x-digit tokens;
        // the real 320x240 must win.
        assert_eq!(parse_input_dims(SAMPLE_LOG), Some((320, 240)));
    }

    #[test]
    fn parse_input_dims_none_without_video_stream() {
        assert_eq!(parse_input_dims("Stream #0:0: Audio: aac, 44100 Hz\n"), None);
    }

    // --- decide ---------------------------------------------------------------

    #[test]
    fn decide_crops_when_bars_found() {
        assert_eq!(
            decide(SAMPLE_LOG),
            Ok(Decision::Crop { w: 320, h: 180, x: 0, y: 30, in_w: 320, in_h: 240 })
        );
    }

    #[test]
    fn decide_no_bars_when_box_covers_frame() {
        let log = SAMPLE_LOG.replace("crop=320:180:0:30", "crop=320:240:0:0");
        assert_eq!(decide(&log), Ok(Decision::NoBars { in_w: 320, in_h: 240 }));
    }

    #[test]
    fn decide_errors_when_whole_frame_reads_black() {
        let log = SAMPLE_LOG.replace("crop=320:180:0:30", "crop=-318:-238:320:240");
        let err = decide(&log).unwrap_err();
        assert!(err.contains("whole frame reads as black"), "{err}");
    }

    #[test]
    fn decide_errors_without_cropdetect_output() {
        let log = "Stream #0:0: Video: h264, yuv420p, 320x240, 10 fps\n";
        let err = decide(log).unwrap_err();
        assert!(err.contains("no cropdetect output"), "{err}");
    }

    // --- crop_argv ------------------------------------------------------------

    #[test]
    fn crop_argv_keeps_container_and_stream_copies_audio() {
        let (argv, out_name) = crop_argv("in.mp4", 320, 180, 0, 30);
        assert_eq!(out_name, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i", "in.mp4", "-vf", "crop=320:180:0:30", "-c:v", "libx264", "-preset",
                "medium", "-crf", "18", "-c:a", "copy", "out.mp4"
            ]
        );
    }

    #[test]
    fn crop_argv_switches_webm_to_mp4_and_reencodes_audio() {
        let (argv, out_name) = crop_argv("in.webm", 240, 180, 40, 0);
        assert_eq!(out_name, "out.mp4");
        assert!(argv.contains(&"aac".to_string()));
        assert!(!argv.contains(&"copy".to_string()));
    }
}
