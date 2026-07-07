//! gizza-ai/video-target-filesize-encoder core — pure, native-testable logic for
//! encoding a video **under a chosen file-size budget** (target MB). No wafer /
//! wasm-bindgen deps; shared verbatim by the chat/CLI block and the web page.
//!
//! # How the size target is hit
//!
//! File size ≈ (video_bitrate + audio_bitrate) × duration ÷ 8. So given a target
//! size, the clip duration, and the audio budget, the required **video** bitrate
//! is:
//!
//! ```text
//! video_bps = (target_bytes × 8 × MARGIN − audio_bps × duration) ÷ duration
//! ```
//!
//! We keep a [`MARGIN`] of container/mux headroom so the muxed MP4 lands *under*
//! the budget rather than nudging over it. The encode is a **single pass** with
//! `-b:v` plus `-maxrate`/`-bufsize` (CBR-style bitrate capping): the gizza
//! ffmpeg bridge is one `build_argv → ffmpegExec` per call with no passlog
//! persisted across calls, so true two-pass VBR is out of model — this is
//! honest single-pass targeting, and highly-compressible clips land comfortably
//! under (never over) the cap.
//!
//! Duration is discovered per surface (the page reads `<video>.duration`; the
//! chat/CLI block probes `ffmpeg -i` and parses the log via [`parse_duration`]),
//! then handed to [`build_argv`]. The pure math lives here.

/// Fraction of the byte budget we actually allocate, leaving container/mux
/// headroom so the muxed file stays **under** the target.
pub const MARGIN: f64 = 0.95;

/// One target "MB" in bytes (mebibyte — the file-size convention). Combined with
/// [`MARGIN`], output reliably lands under the user's stated cap.
pub const BYTES_PER_MB: f64 = 1024.0 * 1024.0;

/// Smallest video bitrate (bits/s) we will encode at. Below this, H.264 output is
/// unwatchable and the target is better served by a lower resolution / dropped
/// audio — so we error with a clear message instead.
pub const MIN_VIDEO_BPS: f64 = 50_000.0;

/// Largest target we accept (MB). Guards against absurd inputs; the block also
/// caps the *output* bytes separately.
pub const MAX_TARGET_MB: f64 = 2048.0;

/// The output container/codec is always MP4 (H.264 + AAC) for maximum
/// compatibility — every scanned competitor does the same.
pub const OUT_NAME: &str = "out.mp4";

/// Parse the `audio_kbps` choice. `"none"` (or `"0"`) drops audio entirely
/// (`Ok(None)`); a bitrate keyword returns `Ok(Some(kbps))`. Anything else is a
/// clear error.
pub fn parse_audio_kbps(s: &str) -> Result<Option<u32>, String> {
    match s.trim() {
        "none" | "0" | "" => Ok(None),
        "64" => Ok(Some(64)),
        "96" => Ok(Some(96)),
        "128" => Ok(Some(128)),
        "192" => Ok(Some(192)),
        "320" => Ok(Some(320)),
        other => Err(format!(
            "audio_kbps must be one of none/64/96/128/192/320 (got {other:?})"
        )),
    }
}

/// Parse the `scale` (max output height) choice. `"keep"` keeps the source size
/// (`Ok(None)`); a number caps the height to that many pixels, **shrinking only**
/// (never upscaling — see [`build_argv`]).
pub fn parse_scale(s: &str) -> Result<Option<u32>, String> {
    match s.trim() {
        "keep" | "" | "0" => Ok(None),
        "1080" => Ok(Some(1080)),
        "720" => Ok(Some(720)),
        "480" => Ok(Some(480)),
        "360" => Ok(Some(360)),
        other => Err(format!(
            "scale must be one of keep/1080/720/480/360 (got {other:?})"
        )),
    }
}

/// Compute the required **video** bitrate (bits/s, floored) to land a clip of
/// `duration_s` seconds under `target_bytes` while spending `audio_kbps` on audio.
///
/// Errors when the duration is non-positive, or when the target is too small for
/// the duration + audio to leave a usable video bitrate (≥ [`MIN_VIDEO_BPS`]).
pub fn compute_video_bps(
    target_bytes: f64,
    duration_s: f64,
    audio_kbps: Option<u32>,
) -> Result<u64, String> {
    if !duration_s.is_finite() || duration_s <= 0.0 {
        return Err("could not determine a positive clip duration".into());
    }
    if !target_bytes.is_finite() || target_bytes <= 0.0 {
        return Err("target size must be a positive number of bytes".into());
    }
    let total_bits = target_bytes * 8.0 * MARGIN;
    let audio_bits = audio_kbps.map(|k| k as f64 * 1000.0 * duration_s).unwrap_or(0.0);
    let video_bits = total_bits - audio_bits;
    let video_bps = video_bits / duration_s;
    if !video_bps.is_finite() || video_bps < MIN_VIDEO_BPS {
        return Err(format!(
            "target is too small for a {duration_s:.1}s clip at this audio setting — \
             raise the target size, pick a lower resolution, or set audio to none"
        ));
    }
    Ok(video_bps.floor() as u64)
}

/// Build the ffmpeg argv (no leading `ffmpeg`) and the output filename for a
/// single-pass, size-targeted H.264/AAC encode.
///
/// - `target_mb`  — the file-size budget in MB (mebibytes). Must be finite, `> 0`,
///   and `≤ MAX_TARGET_MB`.
/// - `duration_s` — the clip duration in seconds (probed per surface).
/// - `audio` / `scale` — the raw `audio_kbps` / `scale` choices ([`parse_audio_kbps`]
///   / [`parse_scale`]).
/// - `in_name`    — the ffmpeg input filename (e.g. `in.mp4`).
///
/// Output is always `out.mp4`. The scale filter (`scale=-2:min(H,ih)`) shrinks
/// only — a cap taller than the source leaves it untouched — and `-2` keeps the
/// aspect ratio with an even width (required by `yuv420p`).
pub fn build_argv(
    target_mb: f64,
    duration_s: f64,
    audio: &str,
    scale: &str,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    if !target_mb.is_finite() || target_mb <= 0.0 {
        return Err("target size must be a positive number of MB".into());
    }
    if target_mb > MAX_TARGET_MB {
        return Err(format!("target size must be at most {MAX_TARGET_MB} MB"));
    }
    let audio_kbps = parse_audio_kbps(audio)?;
    let scale_h = parse_scale(scale)?;
    let target_bytes = target_mb * BYTES_PER_MB;
    let vbps = compute_video_bps(target_bytes, duration_s, audio_kbps)?;

    let mut argv: Vec<String> = vec![
        "-i".into(),
        in_name.into(),
        "-c:v".into(),
        "libx264".into(),
        "-b:v".into(),
        vbps.to_string(),
        "-maxrate".into(),
        vbps.to_string(),
        "-bufsize".into(),
        (vbps.saturating_mul(2)).to_string(),
        "-preset".into(),
        "medium".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
    ];
    if let Some(h) = scale_h {
        argv.push("-vf".into());
        // Backslash-escape the comma so ffmpeg's filtergraph parser reads
        // min(H,ih) as one expression (verified via execve, no shell).
        argv.push(format!("scale=-2:min({h}\\,ih)"));
    }
    match audio_kbps {
        Some(k) => {
            argv.push("-c:a".into());
            argv.push("aac".into());
            argv.push("-b:a".into());
            argv.push(format!("{k}k"));
        }
        None => argv.push("-an".into()),
    }
    // Web-friendly playback (moov atom up front).
    argv.push("-movflags".into());
    argv.push("+faststart".into());
    argv.push(OUT_NAME.into());
    Ok((argv, OUT_NAME.to_string()))
}

/// Parse the clip duration (seconds) from an ffmpeg log's
/// `Duration: HH:MM:SS.ss` line. Returns `None` if absent/unparseable.
pub fn parse_duration(log: &str) -> Option<f64> {
    let idx = log.find("Duration:")?;
    let rest = log[idx + "Duration:".len()..].trim_start();
    let hms = rest.split(',').next()?.trim();
    let mut parts = hms.split(':');
    let h: f64 = parts.next()?.trim().parse().ok()?;
    let m: f64 = parts.next()?.trim().parse().ok()?;
    let s: f64 = parts.next()?.trim().parse().ok()?;
    let dur = h * 3600.0 + m * 60.0 + s;
    if dur.is_finite() && dur > 0.0 {
        Some(dur)
    } else {
        None
    }
}

/// Pass-1 probe argv (no leading `ffmpeg`): decode to null so the log carries the
/// `Duration:` line the block parses. Produces no output file.
pub fn probe_argv(in_name: &str) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-f".into(),
        "null".into(),
        "-".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(argv: &[String], flag: &str) -> Option<usize> {
        argv.iter().position(|a| a == flag)
    }

    #[test]
    fn computes_expected_video_bitrate() {
        // 1 MB (MiB) target, 12s, 128 kbps audio:
        // (1048576*8*0.95 - 128000*12)/12 = 536_098 bps.
        let vbps = compute_video_bps(BYTES_PER_MB, 12.0, Some(128)).unwrap();
        assert_eq!(vbps, 536_098);
    }

    #[test]
    fn dropping_audio_raises_video_budget() {
        let with_audio = compute_video_bps(BYTES_PER_MB, 12.0, Some(128)).unwrap();
        let no_audio = compute_video_bps(BYTES_PER_MB, 12.0, None).unwrap();
        assert!(no_audio > with_audio);
        // no audio: 1048576*8*0.95/12 = 664_098.
        assert_eq!(no_audio, 664_098);
    }

    #[test]
    fn target_too_small_errors() {
        // 0.05 MB over 60s can't leave >= 50 kbps of video → error.
        let err = compute_video_bps(0.05 * BYTES_PER_MB, 60.0, Some(128)).unwrap_err();
        assert!(err.contains("too small"), "got: {err}");
    }

    #[test]
    fn nonpositive_duration_errors() {
        assert!(compute_video_bps(BYTES_PER_MB, 0.0, None).is_err());
        assert!(compute_video_bps(BYTES_PER_MB, -3.0, None).is_err());
    }

    #[test]
    fn build_argv_happy_path_keep_scale_with_audio() {
        let (argv, out) = build_argv(10.0, 30.0, "128", "keep", "in.mp4").unwrap();
        assert_eq!(out, "out.mp4");
        // codec + rate control present
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
        assert!(argv.windows(2).any(|w| w[0] == "-b:a" && w[1] == "128k"));
        // an explicit numeric bitrate got set
        let i = pos(&argv, "-b:v").unwrap();
        let vbps: u64 = argv[i + 1].parse().unwrap();
        assert!(vbps >= MIN_VIDEO_BPS as u64);
        // maxrate mirrors -b:v; bufsize is double
        let mr = pos(&argv, "-maxrate").unwrap();
        assert_eq!(argv[mr + 1], vbps.to_string());
        let bs = pos(&argv, "-bufsize").unwrap();
        assert_eq!(argv[bs + 1], (vbps * 2).to_string());
        // no scale filter when keep
        assert!(pos(&argv, "-vf").is_none());
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn build_argv_scale_shrinks_and_escapes_comma() {
        let (argv, _) = build_argv(10.0, 30.0, "128", "480", "in.mp4").unwrap();
        let i = pos(&argv, "-vf").unwrap();
        assert_eq!(argv[i + 1], "scale=-2:min(480\\,ih)");
    }

    #[test]
    fn build_argv_none_audio_uses_an() {
        let (argv, _) = build_argv(10.0, 30.0, "none", "keep", "in.mp4").unwrap();
        assert!(argv.iter().any(|a| a == "-an"));
        assert!(!argv.iter().any(|a| a == "-c:a"));
    }

    #[test]
    fn build_argv_rejects_bad_target_and_choices() {
        assert!(build_argv(0.0, 30.0, "128", "keep", "in.mp4").is_err());
        assert!(build_argv(f64::NAN, 30.0, "128", "keep", "in.mp4").is_err());
        assert!(build_argv(MAX_TARGET_MB + 1.0, 30.0, "128", "keep", "in.mp4").is_err());
        assert!(build_argv(10.0, 30.0, "999", "keep", "in.mp4").is_err());
        assert!(build_argv(10.0, 30.0, "128", "240", "in.mp4").is_err());
    }

    #[test]
    fn parse_audio_and_scale_choices() {
        assert_eq!(parse_audio_kbps("none").unwrap(), None);
        assert_eq!(parse_audio_kbps("0").unwrap(), None);
        assert_eq!(parse_audio_kbps("192").unwrap(), Some(192));
        assert!(parse_audio_kbps("100").is_err());
        assert_eq!(parse_scale("keep").unwrap(), None);
        assert_eq!(parse_scale("720").unwrap(), Some(720));
        assert!(parse_scale("240").is_err());
    }

    #[test]
    fn parse_duration_reads_hms() {
        let log = "  Duration: 00:01:02.50, start: 0.000000, bitrate: 500 kb/s";
        assert_eq!(parse_duration(log), Some(62.5));
        assert_eq!(parse_duration("no duration here"), None);
    }

    #[test]
    fn probe_argv_targets_null() {
        let a = probe_argv("in.mp4");
        assert_eq!(a, vec!["-i", "in.mp4", "-f", "null", "-"]);
    }
}
