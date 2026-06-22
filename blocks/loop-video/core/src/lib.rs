//! gizza-ai/loop-video core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Repeats a video/GIF either a set number of times (`count`) or until a target
//! `duration` (seconds) is reached, in one continuous file, via ffmpeg's
//! `-stream_loop` input option. Output is stream-copied (`-c copy`) — fast, no
//! re-encode — and keeps the input container/extension.

pub const MAX_COUNT: u32 = 100;
pub const MAX_DURATION: f64 = 3600.0;

fn out_ext(in_name: &str) -> &str {
    in_name.rsplit_once('.').map(|(_, e)| e).filter(|e| !e.is_empty()).unwrap_or("mp4")
}

/// Build the ffmpeg argv (no leading `ffmpeg`). `stream_loop` is ffmpeg's
/// `-stream_loop` value (number of EXTRA plays; `-1` = infinite). When
/// `duration` is set, the output is trimmed to it with `-t`.
pub fn build_argv(in_name: &str, out_name: &str, stream_loop: i64, duration: Option<f64>) -> Vec<String> {
    // -stream_loop is an INPUT option, so it must precede -i.
    let mut a = vec![
        "-stream_loop".into(),
        stream_loop.to_string(),
        "-i".into(),
        in_name.into(),
    ];
    if let Some(d) = duration {
        a.push("-t".into());
        a.push(format!("{d}"));
    }
    a.push("-c".into());
    a.push("copy".into());
    a.push(out_name.into());
    a
}

/// Plan a loop. If `duration > 0` it takes precedence (loop infinitely, trimmed
/// to `duration`); otherwise repeat the clip `count` times total. Returns
/// `(argv, out_name)`; `out_name` keeps the input extension.
pub fn plan(in_name: &str, count: u32, duration: f64) -> Result<(Vec<String>, String), String> {
    let out_name = format!("out.{}", out_ext(in_name));
    if duration > 0.0 {
        if !duration.is_finite() || duration > MAX_DURATION {
            return Err(format!("duration must be between 0 and {MAX_DURATION} seconds"));
        }
        Ok((build_argv(in_name, &out_name, -1, Some(duration)), out_name))
    } else {
        if count < 1 || count > MAX_COUNT {
            return Err(format!("count must be between 1 and {MAX_COUNT}"));
        }
        // count = total plays → (count - 1) extra loops.
        Ok((build_argv(in_name, &out_name, (count - 1) as i64, None), out_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg_after<'a>(argv: &'a [String], key: &str) -> Option<&'a str> {
        argv.iter().position(|a| a == key).and_then(|i| argv.get(i + 1)).map(|s| s.as_str())
    }

    #[test]
    fn count_mode_sets_stream_loop_and_copy() {
        let (argv, out) = plan("clip.mp4", 3, 0.0).unwrap();
        assert_eq!(out, "out.mp4");
        // 3 total plays => 2 extra loops
        assert_eq!(arg_after(&argv, "-stream_loop"), Some("2"));
        // -stream_loop precedes -i
        let sl = argv.iter().position(|a| a == "-stream_loop").unwrap();
        let i = argv.iter().position(|a| a == "-i").unwrap();
        assert!(sl < i, "-stream_loop must come before -i");
        assert!(argv.windows(2).any(|w| w[0] == "-c" && w[1] == "copy"));
        assert!(!argv.iter().any(|a| a == "-t"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn duration_mode_loops_infinitely_and_trims() {
        let (argv, out) = plan("clip.webm", 5, 10.0).unwrap();
        assert_eq!(out, "out.webm");
        assert_eq!(arg_after(&argv, "-stream_loop"), Some("-1"));
        assert_eq!(arg_after(&argv, "-t"), Some("10"));
    }

    #[test]
    fn keeps_extension() {
        assert_eq!(plan("a.gif", 2, 0.0).unwrap().1, "out.gif");
        assert_eq!(plan("noext", 2, 0.0).unwrap().1, "out.mp4");
    }

    #[test]
    fn errors() {
        assert!(plan("a.mp4", 0, 0.0).is_err());
        assert!(plan("a.mp4", 1000, 0.0).is_err());
        assert!(plan("a.mp4", 2, 99999.0).is_err());
    }
}
