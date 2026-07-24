//! gizza-ai/seamless-loop-video core — pure ffmpeg argv construction shared by
//! the chat block and standalone page.
//!
//! A plain repeated clip still jumps from its last frame back to its first.
//! This planner rotates the clip at its midpoint, then cross-dissolves the
//! original end into the original beginning in the middle of the output:
//!
//! ```text
//! input:   [ beginning ........ midpoint ........ end ]
//! output:  [ midpoint .... end ] xfade [ beginning .... midpoint ]
//! ```
//!
//! The output's outer boundary is therefore two adjacent frames from the
//! source midpoint, while the discontinuous end→beginning boundary is hidden
//! inside the clip by the dissolve. The result stays approximately the source
//! duration and can repeat indefinitely without a hard cut.

pub const MIN_DURATION: f64 = 0.5;
pub const MAX_DURATION: f64 = 600.0;
pub const MIN_CROSSFADE: f64 = 0.05;
pub const MAX_CROSSFADE: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioMode {
    Remove,
    Crossfade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    High,
    Balanced,
    Small,
}

impl Quality {
    fn crf(self) -> &'static str {
        match self {
            Self::High => "18",
            Self::Balanced => "23",
            Self::Small => "28",
        }
    }
}

pub fn parse_audio_mode(value: &str) -> Result<AudioMode, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "remove" => Ok(AudioMode::Remove),
        "crossfade" => Ok(AudioMode::Crossfade),
        other => Err(format!(
            "audio {other:?} not supported; expected remove or crossfade"
        )),
    }
}

pub fn parse_quality(value: &str) -> Result<Quality, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "high" => Ok(Quality::High),
        "" | "balanced" => Ok(Quality::Balanced),
        "small" => Ok(Quality::Small),
        other => Err(format!(
            "quality {other:?} not supported; expected high, balanced, or small"
        )),
    }
}

/// Compact, locale-independent ffmpeg number formatting.
pub fn fmt_num(value: f64) -> String {
    if value.fract() == 0.0 && value.is_finite() {
        return format!("{}", value as i64);
    }
    let text = format!("{value:.6}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn video_filter(duration: f64, crossfade: f64) -> String {
    let midpoint = duration / 2.0;
    let head_end = midpoint + crossfade;
    let fade_offset = midpoint - crossfade;
    format!(
        "[0:v]split=2[tail_src][head_src];\
         [tail_src]trim=start={}:end={},setpts=PTS-STARTPTS[tail];\
         [head_src]trim=start=0:end={},setpts=PTS-STARTPTS[head];\
         [tail][head]xfade=transition=fade:duration={}:offset={},format=yuv420p[v]",
        fmt_num(midpoint),
        fmt_num(duration),
        fmt_num(head_end),
        fmt_num(crossfade),
        fmt_num(fade_offset),
    )
}

fn audio_filter(duration: f64, crossfade: f64) -> String {
    let midpoint = duration / 2.0;
    let head_end = midpoint + crossfade;
    format!(
        "[0:a]asplit=2[atail_src][ahead_src];\
         [atail_src]atrim=start={}:end={},asetpts=PTS-STARTPTS[atail];\
         [ahead_src]atrim=start=0:end={},asetpts=PTS-STARTPTS[ahead];\
         [atail][ahead]acrossfade=d={}:c1=tri:c2=tri[a]",
        fmt_num(midpoint),
        fmt_num(duration),
        fmt_num(head_end),
        fmt_num(crossfade),
    )
}

fn validate(duration: f64, crossfade: f64) -> Result<(), String> {
    if !duration.is_finite() || !(MIN_DURATION..=MAX_DURATION).contains(&duration) {
        return Err(format!(
            "duration must be a finite source-clip length between {MIN_DURATION} and {MAX_DURATION} seconds, got {duration}"
        ));
    }
    if !crossfade.is_finite() || !(MIN_CROSSFADE..=MAX_CROSSFADE).contains(&crossfade) {
        return Err(format!(
            "crossfade must be between {MIN_CROSSFADE} and {MAX_CROSSFADE} seconds, got {crossfade}"
        ));
    }
    if crossfade >= duration / 2.0 {
        return Err(format!(
            "crossfade must be shorter than half the clip duration ({:.3} seconds for this clip), got {crossfade}",
            duration / 2.0
        ));
    }
    Ok(())
}

/// Build one foreground ffmpeg invocation. Output is always H.264/AAC MP4 for
/// consistent decoding across the page and CLI.
pub fn plan(
    in_name: &str,
    duration: f64,
    crossfade: f64,
    audio: &str,
    quality: &str,
) -> Result<(Vec<String>, String), String> {
    validate(duration, crossfade)?;
    let audio = parse_audio_mode(audio)?;
    let quality = parse_quality(quality)?;

    let mut filter = video_filter(duration, crossfade);
    if audio == AudioMode::Crossfade {
        filter.push(';');
        filter.push_str(&audio_filter(duration, crossfade));
    }

    let out_name = "out.mp4".to_string();
    let mut argv = vec![
        "-i".into(),
        in_name.into(),
        "-filter_complex".into(),
        filter,
        "-map".into(),
        "[v]".into(),
    ];
    match audio {
        AudioMode::Remove => argv.push("-an".into()),
        AudioMode::Crossfade => {
            argv.extend(["-map".into(), "[a]".into(), "-c:a".into(), "aac".into()]);
        }
    }
    argv.extend([
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "medium".into(),
        "-crf".into(),
        quality.crf().into(),
        "-movflags".into(),
        "+faststart".into(),
        out_name.clone(),
    ]);
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(argv: &[String]) -> &str {
        let index = argv
            .iter()
            .position(|arg| arg == "-filter_complex")
            .unwrap();
        &argv[index + 1]
    }

    #[test]
    fn rotates_at_midpoint_and_crossfades_original_seam() {
        let (argv, out) = plan("in.mp4", 8.0, 1.0, "remove", "balanced").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            filter(&argv),
            "[0:v]split=2[tail_src][head_src];[tail_src]trim=start=4:end=8,setpts=PTS-STARTPTS[tail];[head_src]trim=start=0:end=5,setpts=PTS-STARTPTS[head];[tail][head]xfade=transition=fade:duration=1:offset=3,format=yuv420p[v]"
        );
        assert!(argv.iter().any(|arg| arg == "-an"));
        assert!(argv.windows(2).any(|w| w == ["-crf", "23"]));
    }

    #[test]
    fn crossfades_audio_when_requested() {
        let (argv, _) = plan("in.mov", 6.0, 0.5, "crossfade", "high").unwrap();
        let graph = filter(&argv);
        assert!(graph.contains("[0:a]asplit=2"));
        assert!(graph.contains("acrossfade=d=0.5:c1=tri:c2=tri[a]"));
        assert!(argv.windows(2).any(|w| w == ["-map", "[a]"]));
        assert!(argv.windows(2).any(|w| w == ["-c:a", "aac"]));
        assert!(argv.windows(2).any(|w| w == ["-crf", "18"]));
    }

    #[test]
    fn accepts_secondary_container_and_small_quality() {
        let (argv, out) = plan("clip.webm", 2.0, 0.25, "remove", "small").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(argv[1], "clip.webm");
        assert!(argv.windows(2).any(|w| w == ["-crf", "28"]));
    }

    #[test]
    fn exact_duration_and_fade_caps_are_accepted() {
        assert!(plan("in.mp4", MAX_DURATION, MAX_CROSSFADE, "remove", "high").is_ok());
        assert!(plan("in.mp4", 1.0, MIN_CROSSFADE, "remove", "small").is_ok());
    }

    #[test]
    fn rejects_invalid_ranges_with_actionable_errors() {
        assert!(plan("in.mp4", 0.4, 0.1, "remove", "balanced")
            .unwrap_err()
            .contains("duration"));
        assert!(plan("in.mp4", 2.0, 1.0, "remove", "balanced")
            .unwrap_err()
            .contains("shorter than half"));
        assert!(plan("in.mp4", 2.0, 0.01, "remove", "balanced")
            .unwrap_err()
            .contains("crossfade"));
        assert!(plan("in.mp4", f64::NAN, 0.1, "remove", "balanced").is_err());
    }

    #[test]
    fn rejects_unknown_fixed_choices() {
        assert!(plan("in.mp4", 2.0, 0.2, "copy", "balanced")
            .unwrap_err()
            .contains("remove or crossfade"));
        assert!(plan("in.mp4", 2.0, 0.2, "remove", "lossless")
            .unwrap_err()
            .contains("high, balanced, or small"));
    }
}
