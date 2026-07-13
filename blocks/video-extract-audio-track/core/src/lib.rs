//! gizza-ai/video-extract-audio-track core — pure ffmpeg argv construction shared
//! by the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Demuxes (stream-copies) one audio track out of a video and rewraps it into a
//! chosen container **without re-encoding** — `-i in -vn -map 0:a:<track> -c:a
//! copy out.<ext>`. `-vn` drops the video stream, `-map 0:a:<track>` selects a
//! single audio stream (0 = the first one, so language/commentary tracks on
//! multi-track files are reachable), and `-c:a copy` copies the already-compressed
//! packets straight across — lossless, near-instant, no quality change.
//!
//! Distinct from `extract-audio-from-video` (re-encodes to MP3/WAV) and
//! `mp4-to-m4a` (fixed MP4→M4A): this is the general lossless demuxer — any input
//! container, pick the output container that fits the source codec, default MKA
//! (Matroska audio) which accepts any codec so it never errors on the container.

/// Output container `video-extract-audio-track` can copy the audio stream into.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Container {
    /// Matroska audio — accepts virtually any codec; the safe universal default.
    Mka,
    /// MP4 audio profile — fits AAC/ALAC (the codecs MP4/MOV usually carry).
    M4a,
    /// Ogg — fits Vorbis/Opus (the codecs WebM/OGG usually carry).
    Ogg,
}

impl Container {
    /// Lower-cased file extension this container writes (used for `out.<ext>`).
    pub fn ext(self) -> &'static str {
        match self {
            Container::Mka => "mka",
            Container::M4a => "m4a",
            Container::Ogg => "ogg",
        }
    }

    /// IANA media type for the produced file (page renders `<audio>` from it).
    pub fn mime(self) -> &'static str {
        match self {
            Container::Mka => "audio/x-matroska",
            Container::M4a => "audio/mp4",
            Container::Ogg => "audio/ogg",
        }
    }
}

/// Parse the user-facing container string (the values the chat schema + page
/// accept). Empty defaults to the universal MKA container.
pub fn parse_container(s: &str) -> Result<Container, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "mka" => Ok(Container::Mka),
        "m4a" => Ok(Container::M4a),
        "ogg" => Ok(Container::Ogg),
        other => Err(format!("container {other:?} not supported (mka|m4a|ogg)")),
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to stream-copy audio stream
/// `track` of `in_name` into `out_name`.
///
/// `-vn` drops video, `-map 0:a:<track>` selects only the chosen audio stream,
/// and `-c:a copy` copies its packets with no encoder — lossless, no re-encode.
pub fn build_argv(in_name: &str, out_name: &str, track: u32) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-vn".into(),
        "-map".into(),
        format!("0:a:{track}"),
        "-c:a".into(),
        "copy".into(),
        out_name.into(),
    ]
}

/// Parse `container`, build `(argv, out_name)` for an input file. `out_name` uses
/// the chosen container's extension. Single source shared by the chat block
/// (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan(container: &str, track: u32, in_name: &str) -> Result<(Vec<String>, String), String> {
    let c = parse_container(container)?;
    let out_name = format!("out.{}", c.ext());
    Ok((build_argv(in_name, &out_name, track), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_container_known_and_default() {
        assert_eq!(parse_container("mka").unwrap(), Container::Mka);
        assert_eq!(parse_container("M4A").unwrap(), Container::M4a);
        assert_eq!(parse_container("ogg").unwrap(), Container::Ogg);
        assert_eq!(parse_container("").unwrap(), Container::Mka); // default
    }

    #[test]
    fn parse_container_rejects_unknown() {
        assert!(parse_container("mp3").is_err());
        assert!(parse_container("flac").is_err());
    }

    #[test]
    fn container_ext_and_mime() {
        assert_eq!(Container::Mka.ext(), "mka");
        assert_eq!(Container::Mka.mime(), "audio/x-matroska");
        assert_eq!(Container::M4a.ext(), "m4a");
        assert_eq!(Container::M4a.mime(), "audio/mp4");
        assert_eq!(Container::Ogg.ext(), "ogg");
        assert_eq!(Container::Ogg.mime(), "audio/ogg");
    }

    #[test]
    fn argv_is_the_exact_lossless_demux_plan() {
        let argv = build_argv("in.mp4", "out.mka", 0);
        assert_eq!(
            argv,
            vec!["-i", "in.mp4", "-vn", "-map", "0:a:0", "-c:a", "copy", "out.mka"]
        );
    }

    #[test]
    fn argv_drops_video_and_never_re_encodes() {
        let argv = build_argv("in.mkv", "out.ogg", 0);
        assert!(argv.iter().any(|a| a == "-vn"), "must drop video");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "copy"));
        // No audio encoder is ever named, and no bitrate is set.
        assert!(!argv.iter().any(|a| a == "aac" || a == "libmp3lame" || a == "libvorbis"));
        assert!(!argv.iter().any(|a| a == "-b:a"));
    }

    #[test]
    fn argv_selects_the_requested_track() {
        let argv = build_argv("in.mkv", "out.mka", 2);
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "0:a:2"));
    }

    #[test]
    fn plan_uses_container_extension() {
        let (_argv, out) = plan("mka", 0, "clip.mp4").unwrap();
        assert_eq!(out, "out.mka");
        let (_argv, out) = plan("m4a", 0, "clip.mov").unwrap();
        assert_eq!(out, "out.m4a");
        let (_argv, out) = plan("ogg", 1, "clip.webm").unwrap();
        assert_eq!(out, "out.ogg");
    }

    #[test]
    fn plan_rejects_bad_container() {
        assert!(plan("mp3", 0, "a.mp4").is_err());
    }
}
