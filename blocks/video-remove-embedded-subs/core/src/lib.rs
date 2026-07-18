//! video-remove-embedded-subs core — pure ffmpeg argv construction shared by the
//! chat block and the standalone web page.
//!
//! The tool remuxes with stream copy (`-c copy`) so the video and audio streams
//! keep their original quality, while every embedded subtitle/caption stream is
//! dropped. `-map 0 -map -0:s` selects all input streams then negatively unmaps
//! the subtitle streams, so attachments (fonts) and data streams are preserved —
//! only soft subtitles are removed. There is no re-encode.

/// Output container policy.
pub const CONTAINER_KEEP: &str = "keep";
pub const CONTAINER_MP4: &str = "mp4";
pub const CONTAINER_MKV: &str = "mkv";

fn extension(in_name: &str) -> &str {
    in_name
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .filter(|ext| !ext.is_empty())
        .unwrap_or("mp4")
}

/// Build the ffmpeg argv (without the leading `ffmpeg`) and the output filename.
///
/// - `container=keep` keeps the input extension (mp4/mov/webm/mkv/etc.).
/// - `container=mp4` remuxes to `out.mp4` without re-encoding; it succeeds only
///   when the remaining streams are MP4-compatible.
/// - `container=mkv` remuxes to `out.mkv` (Matroska accepts nearly any codec).
///
/// Only soft (stream-based) subtitles are removed. Hardcoded/burned-in subtitles
/// are baked into the video pixels and cannot be removed by remuxing.
pub fn plan(container: &str, in_name: &str) -> Result<(Vec<String>, String), String> {
    let out_ext = match container {
        CONTAINER_KEEP => extension(in_name).to_ascii_lowercase(),
        CONTAINER_MP4 => "mp4".to_string(),
        CONTAINER_MKV => "mkv".to_string(),
        other => return Err(format!(
            "unknown container {other:?}; use \"keep\", \"mp4\", or \"mkv\""
        )),
    };

    let out_name = format!("out.{out_ext}");
    let argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        // Map every input stream, then drop all subtitle streams.
        "-map".to_string(),
        "0".to_string(),
        "-map".to_string(),
        "-0:s".to_string(),
        // Belt-and-suspenders: also disable subtitle stream selection.
        "-sn".to_string(),
        // Copy the surviving streams — no re-encode, no quality loss.
        "-c".to_string(),
        "copy".to_string(),
        out_name.clone(),
    ];
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_container_preserves_extension_and_removes_subs() {
        let (argv, out) = plan("keep", "in.mkv").unwrap();
        assert_eq!(out, "out.mkv");
        assert_eq!(
            argv,
            vec![
                "-i", "in.mkv", "-map", "0", "-map", "-0:s", "-sn", "-c", "copy", "out.mkv",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn mp4_policy_forces_mp4_extension() {
        let (argv, out) = plan("mp4", "clip.webm").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn mkv_policy_forces_mkv_extension() {
        let (_, out) = plan("mkv", "clip.mov").unwrap();
        assert_eq!(out, "out.mkv");
    }

    #[test]
    fn always_stream_copies_and_drops_subtitles() {
        let (argv, _) = plan("keep", "in.mp4").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-c" && w[1] == "copy"));
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "-0:s"));
        assert!(argv.iter().any(|a| a == "-sn"));
    }

    #[test]
    fn unknown_extension_defaults_to_mp4() {
        let (_, out) = plan("keep", "input").unwrap();
        assert_eq!(out, "out.mp4");
    }

    #[test]
    fn rejects_unknown_container() {
        let err = plan("avi", "in.mp4").unwrap_err();
        assert!(err.contains("unknown container"));
    }
}
