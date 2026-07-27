//! gizza-ai/video-audio-track-selector core — pure ffmpeg argv construction shared
//! by the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Keeps exactly ONE chosen audio track (by 0-based index) from a video that has
//! multiple audio tracks, and drops the rest. Lossless: everything kept is
//! stream-copied (`-c copy`), so the picture and the kept audio are byte-for-byte
//! the same — no re-encode. All video streams are kept; other audio tracks are
//! removed; subtitle tracks are dropped by default (opt in with `keep_subtitles`);
//! the kept track is flagged as the default audio unless `set_default` is off.

fn out_ext(in_name: &str) -> &str {
    in_name.rsplit_once('.').map(|(_, e)| e).filter(|e| !e.is_empty()).unwrap_or("mp4")
}

/// Build the ffmpeg argv (no leading `ffmpeg`) + out_name for keeping one audio
/// track. `track` is the 0-based audio-stream index to keep (0 = first audio
/// track). `keep_subtitles` also maps any subtitle streams (optional — skipped
/// gracefully if the input has none). `set_default` flags the kept audio track as
/// the default audio disposition in the output.
pub fn plan(
    in_name: &str,
    track: u32,
    keep_subtitles: bool,
    set_default: bool,
) -> Result<(Vec<String>, String), String> {
    let out_name = format!("out.{}", out_ext(in_name));
    let mut argv: Vec<String> = vec![
        "-i".into(),
        in_name.into(),
        // Keep every video stream; error out if there is somehow no video.
        "-map".into(),
        "0:v".into(),
        // Keep exactly the chosen audio track — ffmpeg errors if it doesn't exist.
        "-map".into(),
        format!("0:a:{track}"),
    ];
    if keep_subtitles {
        // Optional: skipped without error when the input has no subtitle streams.
        argv.push("-map".into());
        argv.push("0:s?".into());
    }
    // Lossless: stream-copy everything we kept.
    argv.push("-c".into());
    argv.push("copy".into());
    if set_default {
        // The kept track becomes output audio stream 0; flag it as the default.
        argv.push("-disposition:a:0".into());
        argv.push("default".into());
    }
    argv.push(out_name.clone());
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_chosen_track_and_copies() {
        let (argv, out) = plan("in.mp4", 1, false, true).unwrap();
        assert_eq!(out, "out.mp4");
        // maps all video + the second audio track (index 1)
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "0:v"), "keep video: {argv:?}");
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "0:a:1"), "keep audio 1: {argv:?}");
        // lossless stream copy
        assert!(argv.windows(2).any(|w| w[0] == "-c" && w[1] == "copy"), "stream copy: {argv:?}");
        // no subtitle map by default
        assert!(!argv.iter().any(|a| a == "0:s?"), "no subs by default: {argv:?}");
        // default disposition on by default
        assert!(argv.windows(2).any(|w| w[0] == "-disposition:a:0" && w[1] == "default"), "default flag: {argv:?}");
    }

    #[test]
    fn default_track_is_first_audio() {
        let (argv, _) = plan("in.mkv", 0, false, true).unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "0:a:0"), "keep audio 0: {argv:?}");
    }

    #[test]
    fn keep_subtitles_maps_optional_subs() {
        let (argv, _) = plan("in.mkv", 0, true, true).unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "0:s?"), "map subs: {argv:?}");
    }

    #[test]
    fn no_default_flag_when_disabled() {
        let (argv, _) = plan("in.mp4", 0, false, false).unwrap();
        assert!(!argv.iter().any(|a| a == "-disposition:a:0"), "no disposition: {argv:?}");
    }

    #[test]
    fn keeps_input_extension() {
        let (_, out) = plan("clip.webm", 0, false, true).unwrap();
        assert_eq!(out, "out.webm");
        let (_, out2) = plan("noext", 0, false, true).unwrap();
        assert_eq!(out2, "out.mp4");
    }
}
