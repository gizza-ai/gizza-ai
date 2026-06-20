//! gizza-ai/video-mute core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Removes the audio track from a video. Uses stream-copy for the video
//! (`-c:v copy`) + `-an`, so it's lossless and fast — no re-encode, the picture
//! is byte-for-byte the same, just without audio.

fn out_ext(in_name: &str) -> &str {
    in_name.rsplit_once('.').map(|(_, e)| e).filter(|e| !e.is_empty()).unwrap_or("mp4")
}

/// Build the ffmpeg argv (no leading `ffmpeg`) + out_name for muting.
pub fn build_argv(in_name: &str) -> (Vec<String>, String) {
    let out_name = format!("out.{}", out_ext(in_name));
    let argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-c:v".to_string(),
        "copy".to_string(),
        "-an".to_string(),
        out_name.clone(),
    ];
    (argv, out_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_audio_and_copies_video() {
        let (argv, out) = build_argv("in.mp4");
        assert_eq!(out, "out.mp4");
        assert!(argv.iter().any(|a| a == "-an"), "must drop audio: {argv:?}");
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "copy"), "lossless video copy");
    }

    #[test]
    fn keeps_input_extension() {
        let (_, out) = build_argv("clip.webm");
        assert_eq!(out, "out.webm");
        let (_, out2) = build_argv("noext");
        assert_eq!(out2, "out.mp4");
    }
}
