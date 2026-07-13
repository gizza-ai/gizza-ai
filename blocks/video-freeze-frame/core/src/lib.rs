//! gizza-ai/video-freeze-frame core — pure ffmpeg argv construction shared by the
//! chat skill block, the CLI, and the standalone web page. No wafer/wasm-bindgen
//! deps.
//!
//! Creates a freeze-frame effect: the video plays normally up to a chosen
//! timestamp, holds the frame at that instant as a still for `duration` seconds,
//! then resumes from the same point. The result is `duration` seconds longer than
//! the source.
//!
//! How the filtergraph works (fps-agnostic, no probing needed):
//!   * `[0:v]trim=0:T,setpts=PTS-STARTPTS,tpad=stop_mode=clone:stop_duration=D`
//!     plays `[0, T]` then clones its LAST frame (the frame at ~T) for `D` more
//!     seconds — that clone IS the freeze, and `tpad` clones at the source frame
//!     rate so no fps is needed;
//!   * `[0:v]trim=T,setpts=PTS-STARTPTS` is the tail `[T, end]`;
//!   * `concat=n=2:v=1:a=0` joins hold + tail into one stream.
//! Output duration = T + D + (input - T) = input + D. The video is re-encoded to
//! H.264 (the split/concat breaks stream-copy) and the input container is kept
//! when it can hold H.264, otherwise mp4.
//!
//! AUDIO IS DROPPED (`-an`): re-timing an audio track around an inserted still
//! (silence for the hold, then the tail shifted by D) is out of scope here, so the
//! freeze is a video-only visual effect. This is stated on the page and in the
//! chat/CLI descriptions.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;

/// Longest hold (freeze) duration accepted, in seconds. A still held longer than
/// this is almost always a mistake and would bloat the re-encoded output.
pub const MAX_DURATION: f64 = 60.0;
/// Largest freeze timestamp accepted, in seconds (a sanity cap; a timestamp past
/// the clip's end simply holds the last frame).
pub const MAX_TIME: f64 = 36000.0;

/// Format a finite `f64` as a compact ffmpeg-friendly decimal: whole numbers
/// print without a fractional part (`5`), fractions round to 3 places with
/// trailing zeros trimmed (`0.5`, `1.25`). Keeps the filter string clean and
/// deterministic across surfaces.
fn num(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    let mut s = format!("{r:.3}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

/// Build the `-filter_complex` string that holds the frame at `time` for
/// `duration` seconds. Validated by [`plan`]; separated out so the argv test can
/// assert the graph directly.
pub fn build_filter(time: f64, duration: f64) -> Result<String, String> {
    if !time.is_finite() || time < 0.0 {
        return Err(format!("time must be a timestamp in seconds ≥ 0, got {time}"));
    }
    if time > MAX_TIME {
        return Err(format!("time must be ≤ {MAX_TIME} seconds, got {time}"));
    }
    if !duration.is_finite() || duration <= 0.0 {
        return Err(format!(
            "duration must be a hold length in seconds greater than 0, got {duration}"
        ));
    }
    if duration > MAX_DURATION {
        return Err(format!(
            "duration must be ≤ {MAX_DURATION} seconds, got {duration}"
        ));
    }
    let d = num(duration);
    if time == 0.0 {
        // Freeze at the very start: clone the FIRST frame for D seconds, then play
        // the whole clip. `trim=0:0` selects zero frames (the trim end is
        // exclusive), leaving `tpad=stop_mode=clone` with no frame to hold — so the
        // hold branch would be empty and the output would just be the source. Use
        // `tpad`'s start_mode instead to prepend the cloned opening frame; no
        // split/concat is needed because the tail is the entire clip.
        return Ok(format!(
            "[0:v]tpad=start_mode=clone:start_duration={d},setpts=PTS-STARTPTS[outv]"
        ));
    }
    let t = num(time);
    // hold = [0,T] then clone the frame at T for D seconds; tail = [T,end].
    Ok(format!(
        "[0:v]trim=0:{t},setpts=PTS-STARTPTS,tpad=stop_mode=clone:stop_duration={d}[hold];\
         [0:v]trim={t},setpts=PTS-STARTPTS[tail];\
         [hold][tail]concat=n=2:v=1:a=0[outv]"
    ))
}

/// Validate the inputs and build the full ffmpeg argv (no leading `ffmpeg`) plus
/// the output filename. Returns `(argv, out_name)`; `out_name` keeps the input
/// container when it can hold H.264, otherwise `out.mp4`.
pub fn plan(in_name: &str, time: f64, duration: f64) -> Result<(Vec<String>, String), String> {
    let filter = build_filter(time, duration)?;
    let (out_ext, _keeps_audio) = h264_out_ext(in_name);
    let out_name = format!("out.{out_ext}");

    let mut argv: Vec<String> = vec![
        "-i".into(),
        in_name.into(),
        "-filter_complex".into(),
        filter,
        "-map".into(),
        "[outv]".into(),
        // Audio is dropped — a freeze-frame is a video-only visual effect here.
        "-an".into(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "medium".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
    ];
    if out_name.ends_with(".mp4") || out_name.ends_with(".mov") {
        argv.push("-movflags".into());
        argv.push("+faststart".into());
    }
    argv.push(out_name.clone());
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fc(argv: &[String]) -> String {
        let i = argv.iter().position(|a| a == "-filter_complex").expect("-filter_complex present");
        argv[i + 1].clone()
    }

    #[test]
    fn happy_path_builds_split_hold_concat_and_drops_audio() {
        let (argv, out) = plan("in.mp4", 2.0, 3.0).unwrap();
        assert_eq!(out, "out.mp4");
        let f = fc(&argv);
        // hold segment: play [0,2] then clone the frame for 3s.
        assert!(f.contains("trim=0:2,setpts=PTS-STARTPTS,tpad=stop_mode=clone:stop_duration=3[hold]"), "{f}");
        // tail segment starts at the freeze point.
        assert!(f.contains("trim=2,setpts=PTS-STARTPTS[tail]"), "{f}");
        // joined into one video stream.
        assert!(f.contains("[hold][tail]concat=n=2:v=1:a=0[outv]"), "{f}");
        // H.264 re-encode, mapped output, audio dropped, faststart for mp4.
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "[outv]"));
        assert!(argv.iter().any(|a| a == "-an"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(argv.iter().any(|a| a == "+faststart"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn compact_decimals_in_filter() {
        let f = build_filter(1.5, 0.25).unwrap();
        assert!(f.contains("trim=0:1.5,"), "{f}");
        assert!(f.contains("stop_duration=0.25[hold]"), "{f}");
        assert!(f.contains("trim=1.5,setpts"), "{f}");
    }

    #[test]
    fn webm_switches_to_mp4_mov_keeps_container() {
        assert_eq!(plan("clip.webm", 1.0, 1.0).unwrap().1, "out.mp4");
        assert_eq!(plan("clip.mov", 1.0, 1.0).unwrap().1, "out.mov");
        assert_eq!(plan("clip.mkv", 1.0, 1.0).unwrap().1, "out.mkv");
        assert_eq!(plan("noext", 1.0, 1.0).unwrap().1, "out.mp4");
    }

    #[test]
    fn rejects_bad_time_and_duration() {
        assert!(plan("in.mp4", -1.0, 2.0).unwrap_err().contains("time"));
        assert!(plan("in.mp4", 1.0, 0.0).unwrap_err().contains("duration"));
        assert!(plan("in.mp4", 1.0, -3.0).unwrap_err().contains("duration"));
        // Exact cap boundaries: 60 accepted, 60.001 one over → rejected.
        assert!(plan("in.mp4", 1.0, MAX_DURATION).is_ok());
        assert!(plan("in.mp4", 1.0, MAX_DURATION + 0.001).unwrap_err().contains("duration"));
        assert!(plan("in.mp4", MAX_TIME, 1.0).is_ok());
        assert!(plan("in.mp4", MAX_TIME + 1.0, 1.0).unwrap_err().contains("time"));
    }

    #[test]
    fn freeze_at_zero_holds_first_frame() {
        let f = build_filter(0.0, 2.0).unwrap();
        // At time=0 we clone the FIRST frame via tpad's start_mode: `trim=0:0`
        // would select zero frames, leaving the clone with nothing to hold.
        assert!(f.contains("tpad=start_mode=clone:start_duration=2"), "{f}");
        assert!(f.contains("[outv]"), "{f}");
        assert!(!f.contains("trim=0:0"), "{f}");
        assert!(!f.contains("concat"), "{f}");
    }

    #[test]
    fn num_formats_compactly() {
        assert_eq!(num(5.0), "5");
        assert_eq!(num(0.5), "0.5");
        assert_eq!(num(1.25), "1.25");
        assert_eq!(num(0.0), "0");
        assert_eq!(num(2.500), "2.5");
    }
}
