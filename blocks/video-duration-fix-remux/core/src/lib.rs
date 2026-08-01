//! video-duration-fix-remux core — pure ffmpeg argv construction shared by the
//! chat block and the standalone web page.
//!
//! The tool repairs missing or wrong container-level duration metadata by
//! REMUXING the file with stream copy (`-c copy`). Remuxing parses every packet
//! and writes a fresh container header, so the new file carries a correct
//! duration/index without ever decoding or re-encoding the audio/video — quality
//! is bit-for-bit preserved. This fixes the classic `MediaRecorder`/screen-capture
//! WebM whose duration reads as `Infinity`, MP4/MOV files with a broken `moov`
//! atom, and clips whose header duration disagrees with the real content length.

/// Output container policy.
pub const CONTAINER_KEEP: &str = "keep";
pub const CONTAINERS: [&str; 5] = ["keep", "mp4", "mkv", "mov", "webm"];

fn extension(in_name: &str) -> &str {
    in_name
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .filter(|ext| !ext.is_empty())
        .unwrap_or("mp4")
}

/// True for MP4-family containers, where `-movflags +faststart` applies.
fn is_mp4_family(ext: &str) -> bool {
    matches!(ext, "mp4" | "mov" | "m4v" | "m4a")
}

/// Build the ffmpeg argv (without the leading `ffmpeg`) and output filename.
///
/// - `container=keep` keeps the input extension; any other value remuxes into
///   that container (only stream copy, so codecs must be container-compatible).
/// - `faststart=true` (MP4/MOV output only) moves the `moov` atom to the front so
///   players read the correct duration immediately and the file streams
///   progressively. Ignored for non-MP4 containers.
/// - `regen_timestamps=true` adds `-fflags +genpts` to regenerate missing/broken
///   presentation timestamps before remuxing — use when the duration reads as 0,
///   `N/A`, or `Infinity`.
pub fn plan(
    container: &str,
    faststart: bool,
    regen_timestamps: bool,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    let out_ext = match container {
        CONTAINER_KEEP => extension(in_name).to_ascii_lowercase(),
        c if CONTAINERS.contains(&c) => c.to_string(),
        other => {
            return Err(format!(
                "unknown container {other:?}; use one of keep, mp4, mkv, mov, webm"
            ))
        }
    };
    let out_name = format!("out.{out_ext}");

    let mut argv: Vec<String> = Vec::new();
    // Input-side flag: must precede -i so the demuxer regenerates PTS.
    if regen_timestamps {
        argv.extend(["-fflags".to_string(), "+genpts".to_string()]);
    }
    argv.extend(["-i".to_string(), in_name.to_string()]);
    // Keep every stream (video/audio/subtitle) in the rebuilt container.
    argv.extend(["-map".to_string(), "0".to_string()]);
    // Stream copy: no decode, no re-encode.
    argv.extend(["-c".to_string(), "copy".to_string()]);
    if faststart && is_mp4_family(&out_ext) {
        argv.extend(["-movflags".to_string(), "+faststart".to_string()]);
    }
    argv.push(out_name.clone());
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_container_preserves_extension_and_copies_streams() {
        let (argv, out) = plan("keep", true, false, "clip.webm").unwrap();
        assert_eq!(out, "out.webm");
        assert_eq!(
            argv,
            vec!["-i", "clip.webm", "-map", "0", "-c", "copy", "out.webm"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
        // faststart is a no-op on a webm output.
        assert!(!argv.iter().any(|a| a == "-movflags"));
    }

    #[test]
    fn mp4_output_with_faststart_moves_moov_atom() {
        let (argv, out) = plan("mp4", true, false, "recording.mkv").unwrap();
        assert_eq!(out, "out.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-movflags" && w[1] == "+faststart"));
        assert!(argv.windows(2).any(|w| w[0] == "-c" && w[1] == "copy"));
    }

    #[test]
    fn faststart_disabled_omits_movflags() {
        let (argv, _) = plan("mov", false, false, "in.mov").unwrap();
        assert!(!argv.iter().any(|a| a == "-movflags"));
    }

    #[test]
    fn regen_timestamps_adds_genpts_before_input() {
        let (argv, _) = plan("keep", true, true, "broken.webm").unwrap();
        // -fflags +genpts must come before -i.
        let genpts = argv.iter().position(|a| a == "+genpts").unwrap();
        let input = argv.iter().position(|a| a == "-i").unwrap();
        assert!(genpts < input, "genpts must precede -i");
        assert!(argv.windows(2).any(|w| w[0] == "-fflags" && w[1] == "+genpts"));
    }

    #[test]
    fn unknown_extension_defaults_to_mp4() {
        let (_, out) = plan("keep", true, false, "noext").unwrap();
        assert_eq!(out, "out.mp4");
    }

    #[test]
    fn rejects_unknown_container() {
        let err = plan("avi", true, false, "in.mp4").unwrap_err();
        assert!(err.contains("unknown container"), "got: {err}");
    }
}
