//! video-strip-metadata core — pure ffmpeg argv construction shared by the chat
//! block and the standalone web page.
//!
//! The tool remuxes with stream copy (`-c copy`) so video/audio/subtitle streams
//! keep their original quality. Metadata is removed at global and per-stream
//! scope. `-bitexact` prevents ffmpeg from writing a fresh encoder/version tag.

/// Output container policy.
pub const CONTAINER_KEEP: &str = "keep";
pub const CONTAINER_MP4: &str = "mp4";

/// Chapter handling policy.
pub const CHAPTERS_REMOVE: &str = "remove";
pub const CHAPTERS_KEEP: &str = "keep";

fn extension(in_name: &str) -> &str {
    in_name
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .filter(|ext| !ext.is_empty())
        .unwrap_or("mp4")
}

/// Build the ffmpeg argv (without the leading `ffmpeg`) and output filename.
///
/// - `container=keep` keeps the input extension (mp4/mov/webm/mkv/etc.).
/// - `container=mp4` remuxes to `out.mp4` without re-encoding; it succeeds only
///   when the input streams are MP4-compatible.
/// - `chapters=remove` drops chapter markers; `keep` preserves them.
pub fn build_argv(container: &str, chapters: &str, in_name: &str) -> Result<(Vec<String>, String), String> {
    let out_ext = match container {
        CONTAINER_KEEP => extension(in_name).to_ascii_lowercase(),
        CONTAINER_MP4 => "mp4".to_string(),
        other => return Err(format!("unknown container {other:?}; use \"keep\" or \"mp4\"")),
    };
    match chapters {
        CHAPTERS_REMOVE | CHAPTERS_KEEP => {}
        other => return Err(format!("unknown chapters {other:?}; use \"remove\" or \"keep\"")),
    }

    let out_name = format!("out.{out_ext}");
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-map".to_string(),
        "0".to_string(),
        "-map_metadata".to_string(),
        "-1".to_string(),
        "-map_metadata:s".to_string(),
        "-1".to_string(),
    ];
    if chapters == CHAPTERS_REMOVE {
        argv.extend(["-map_chapters".to_string(), "-1".to_string()]);
    }
    argv.extend([
        "-c".to_string(),
        "copy".to_string(),
        "-bitexact".to_string(),
        out_name.clone(),
    ]);
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_keep_removes_metadata_and_chapters_losslessly() {
        let (argv, out) = build_argv("keep", "remove", "in.mov").unwrap();
        assert_eq!(out, "out.mov");
        assert_eq!(
            argv,
            vec![
                "-i", "in.mov", "-map", "0", "-map_metadata", "-1", "-map_metadata:s", "-1",
                "-map_chapters", "-1", "-c", "copy", "-bitexact", "out.mov",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn mp4_policy_forces_mp4_extension() {
        let (argv, out) = build_argv("mp4", "remove", "clip.webm").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn keep_chapters_omits_map_chapters_drop() {
        let (argv, out) = build_argv("keep", "keep", "clip.mkv").unwrap();
        assert_eq!(out, "out.mkv");
        assert!(!argv.windows(2).any(|w| w[0] == "-map_chapters" && w[1] == "-1"));
        assert!(argv.windows(2).any(|w| w[0] == "-c" && w[1] == "copy"));
    }

    #[test]
    fn unknown_extension_defaults_to_mp4() {
        let (_, out) = build_argv("keep", "remove", "input").unwrap();
        assert_eq!(out, "out.mp4");
    }

    #[test]
    fn rejects_unknown_enums() {
        assert!(build_argv("avi", "remove", "in.mp4").unwrap_err().contains("unknown container"));
        assert!(build_argv("keep", "preserve", "in.mp4").unwrap_err().contains("unknown chapters"));
    }
}
