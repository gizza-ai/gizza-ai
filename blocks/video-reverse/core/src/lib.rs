//! video-reverse core — pure ffmpeg argv construction shared by the chat block
//! and the browser page. No wafer/wasm-bindgen deps.
//!
//! Reversing video requires decoding the whole clip and re-encoding it, so the
//! plan always writes H.264/AAC MP4 with an explicit CRF quality preset. The
//! three audio policies mirror common online reverse-video tools: reverse the
//! sound with the picture, keep a forward copy of the sound, or mute it.

/// Playback shape for the output video stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Reverse,
    ForwardReverse,
    ReverseForward,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "reverse" => Ok(Self::Reverse),
            "forward-reverse" | "boomerang" => Ok(Self::ForwardReverse),
            "reverse-forward" => Ok(Self::ReverseForward),
            other => Err(format!(
                "mode {other:?} not supported (reverse|forward-reverse|reverse-forward)"
            )),
        }
    }
}

/// How to handle the audio track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioMode {
    Reverse,
    Keep,
    Mute,
}

impl AudioMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "reverse" => Ok(Self::Reverse),
            "keep" => Ok(Self::Keep),
            "mute" | "silent" => Ok(Self::Mute),
            other => Err(format!("audio {other:?} not supported (reverse|keep|mute)")),
        }
    }
}

/// H.264 CRF quality preset. Lower CRF is larger/better.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    High,
    Balanced,
    Small,
}

impl Quality {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "balanced" => Ok(Self::Balanced),
            "high" => Ok(Self::High),
            "small" => Ok(Self::Small),
            other => Err(format!(
                "quality {other:?} not supported (high|balanced|small)"
            )),
        }
    }

    fn crf(self) -> &'static str {
        match self {
            Self::High => "18",
            Self::Balanced => "23",
            Self::Small => "28",
        }
    }
}

fn video_filter(mode: Mode) -> String {
    match mode {
        Mode::Reverse => "[0:v]reverse[v]".to_string(),
        Mode::ForwardReverse => {
            "[0:v]split[vf][vrsrc];[vrsrc]reverse[vr];[vf][vr]concat=n=2:v=1:a=0[v]".to_string()
        }
        Mode::ReverseForward => {
            "[0:v]split[vf][vrsrc];[vrsrc]reverse[vr];[vr][vf]concat=n=2:v=1:a=0[v]".to_string()
        }
    }
}

fn audio_filter(mode: Mode, audio: AudioMode) -> Option<String> {
    match (mode, audio) {
        (_, AudioMode::Mute) => None,
        (Mode::Reverse, AudioMode::Reverse) => Some("[0:a]areverse[a]".to_string()),
        (Mode::Reverse, AudioMode::Keep) => Some("[0:a]anull[a]".to_string()),
        (Mode::ForwardReverse, AudioMode::Reverse) => Some(
            "[0:a]asplit[af][arsrc];[arsrc]areverse[ar];[af][ar]concat=n=2:v=0:a=1[a]".to_string(),
        ),
        (Mode::ForwardReverse, AudioMode::Keep) => {
            Some("[0:a]asplit[a1][a2];[a1][a2]concat=n=2:v=0:a=1[a]".to_string())
        }
        (Mode::ReverseForward, AudioMode::Reverse) => Some(
            "[0:a]asplit[af][arsrc];[arsrc]areverse[ar];[ar][af]concat=n=2:v=0:a=1[a]".to_string(),
        ),
        (Mode::ReverseForward, AudioMode::Keep) => {
            Some("[0:a]asplit[a1][a2];[a1][a2]concat=n=2:v=0:a=1[a]".to_string())
        }
    }
}

/// Build ffmpeg argv (no leading `ffmpeg`) and an output filename.
pub fn plan(
    in_name: &str,
    mode: &str,
    audio: &str,
    quality: &str,
) -> Result<(Vec<String>, String), String> {
    if in_name.trim().is_empty() {
        return Err("input filename is empty".into());
    }
    let mode = Mode::parse(mode)?;
    let audio = AudioMode::parse(audio)?;
    let quality = Quality::parse(quality)?;
    let out_name = "out.mp4".to_string();

    let vf = video_filter(mode);
    let af = audio_filter(mode, audio);
    let filter = match af {
        Some(af) => format!("{vf};{af}"),
        None => vf,
    };

    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-filter_complex".to_string(),
        filter,
        "-map".to_string(),
        "[v]".to_string(),
    ];

    if audio == AudioMode::Mute {
        argv.push("-an".to_string());
    } else {
        argv.extend(["-map", "[a]", "-c:a", "aac", "-b:a", "128k"].map(str::to_string));
    }

    argv.extend(
        [
            "-c:v",
            "libx264",
            "-preset",
            "medium",
            "-crf",
            quality.crf(),
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
            &out_name,
        ]
        .map(str::to_string),
    );
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_pair(argv: &[String], a: &str, b: &str) -> bool {
        argv.windows(2).any(|w| w[0] == a && w[1] == b)
    }

    #[test]
    fn default_reverses_video_and_audio_to_mp4() {
        let (argv, out) = plan("in.mov", "", "", "").unwrap();
        assert_eq!(out, "out.mp4");
        assert!(has_pair(&argv, "-i", "in.mov"));
        assert!(argv.iter().any(|a| a.contains("[0:v]reverse[v]")));
        assert!(argv.iter().any(|a| a.contains("[0:a]areverse[a]")));
        assert!(has_pair(&argv, "-map", "[v]"));
        assert!(has_pair(&argv, "-map", "[a]"));
        assert!(has_pair(&argv, "-c:v", "libx264"));
        assert!(has_pair(&argv, "-c:a", "aac"));
    }

    #[test]
    fn keep_audio_does_not_reverse_audio() {
        let (argv, _) = plan("clip.mp4", "reverse", "keep", "balanced").unwrap();
        let filter = argv.iter().find(|a| a.contains("[0:v]")).unwrap();
        assert!(filter.contains("[0:a]anull[a]"));
        assert!(!filter.contains("areverse[a]"));
    }

    #[test]
    fn mute_audio_maps_only_video_and_sets_an() {
        let (argv, _) = plan("clip.mp4", "reverse", "mute", "balanced").unwrap();
        assert!(argv.iter().any(|a| a == "-an"));
        assert!(!has_pair(&argv, "-map", "[a]"));
        assert!(!argv.iter().any(|a| a == "-c:a"));
    }

    #[test]
    fn boomerang_modes_concat_in_the_requested_order() {
        let (argv, _) = plan("clip.mp4", "forward-reverse", "reverse", "balanced").unwrap();
        let filter = argv.iter().find(|a| a.contains("concat=n=2:v=1")).unwrap();
        assert!(filter.contains("[vf][vr]concat=n=2:v=1:a=0[v]"));
        assert!(filter.contains("[af][ar]concat=n=2:v=0:a=1[a]"));

        let (argv, _) = plan("clip.mp4", "reverse-forward", "reverse", "balanced").unwrap();
        let filter = argv.iter().find(|a| a.contains("concat=n=2:v=1")).unwrap();
        assert!(filter.contains("[vr][vf]concat=n=2:v=1:a=0[v]"));
        assert!(filter.contains("[ar][af]concat=n=2:v=0:a=1[a]"));
    }

    #[test]
    fn quality_selects_crf() {
        for (quality, crf) in [
            ("high", "18"),
            ("balanced", "23"),
            ("small", "28"),
            ("", "23"),
        ] {
            let (argv, _) = plan("clip.mp4", "reverse", "reverse", quality).unwrap();
            assert!(has_pair(&argv, "-crf", crf));
        }
    }

    #[test]
    fn rejects_unknown_options() {
        assert!(plan("clip.mp4", "sideways", "reverse", "balanced")
            .unwrap_err()
            .contains("mode"));
        assert!(plan("clip.mp4", "reverse", "loud", "balanced")
            .unwrap_err()
            .contains("audio"));
        assert!(plan("clip.mp4", "reverse", "reverse", "huge")
            .unwrap_err()
            .contains("quality"));
        assert!(plan("", "reverse", "reverse", "balanced")
            .unwrap_err()
            .contains("empty"));
    }
}
