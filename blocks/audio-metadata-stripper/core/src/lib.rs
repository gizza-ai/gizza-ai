//! gizza-ai/audio-metadata-stripper core — pure ffmpeg argv construction shared
//! by the chat skill block and the standalone web page. No wafer/wasm-bindgen
//! deps, so it compiles natively for unit tests and to wasm for both surfaces.
//!
//! Strips every embedded tag from an audio file **without re-encoding**: ID3v1/
//! ID3v2, Vorbis comments, ASF/RIFF INFO, chapters and (by default) the embedded
//! cover-art image. The audio stream is copied through bit-for-bit (`-c copy`),
//! so the sound is bit-identical and the container/codec are preserved — only
//! the metadata is dropped.
//!
//! ffmpeg mechanics:
//!   - `-map_metadata -1`  drops all global + per-stream metadata (the text tags).
//!   - `-map_chapters -1`  drops chapter markers.
//!   - `-bitexact`         stops the muxer writing its own `encoder`/version tag.
//!   - `-map 0:a`          keeps ONLY the audio stream → the attached-picture
//!                         (cover art) stream is dropped. With `CoverArt::Keep`
//!                         we use `-map 0` instead so the picture rides along.
//!   - `-c copy`           stream-copy — no re-encode, bit-identical audio.

/// What to do with an embedded cover-art / attached-picture stream.
///
/// A fixed two-value choice (mirrored by the chat schema's `cover_art` enum and
/// the page's `<select>`): either strip the image along with the text tags, or
/// keep the picture while still removing every text tag and chapter.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CoverArt {
    /// Drop the attached-picture stream (the default) — a fully bare audio file.
    Remove,
    /// Keep the embedded cover image; only text tags and chapters are removed.
    Keep,
}

impl CoverArt {
    /// True when the embedded picture stream should survive the strip.
    pub fn keeps_cover(self) -> bool {
        matches!(self, CoverArt::Keep)
    }
}

/// Parse the user-facing `cover_art` string (the values the chat schema + page
/// `<select>` accept). Empty defaults to `Remove` so an omitted param strips a
/// bare copy; any other value is a hard error surfaced to the caller.
pub fn parse_cover_art(s: &str) -> Result<CoverArt, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "remove" | "strip" => Ok(CoverArt::Remove),
        "keep" => Ok(CoverArt::Keep),
        other => Err(format!("cover_art {other:?} not supported (remove|keep)")),
    }
}

/// Derive the lower-cased file extension of `in_name` (`in.mp3` → `mp3`).
/// Falls back to `mp3` when the name carries no extension, so the output always
/// has a sensible container. Shared by both surfaces to name `out.<ext>`.
pub fn ext_of(in_name: &str) -> String {
    match in_name.rsplit_once('.') {
        Some((_, ext)) if !ext.is_empty() => ext.to_ascii_lowercase(),
        _ => "mp3".to_string(),
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that strips metadata from
/// `in_name` into `out_name`. When `keep_cover_art` is true the embedded cover
/// image is preserved (only text tags/chapters are removed); otherwise the
/// picture stream is dropped as well. Shared verbatim by the web page and the
/// chat block.
pub fn build_argv(in_name: &str, out_name: &str, keep_cover_art: bool) -> Vec<String> {
    let mut argv = vec!["-i".to_string(), in_name.to_string()];
    // Stream selection: audio-only by default, everything when keeping cover art.
    if keep_cover_art {
        argv.push("-map".into());
        argv.push("0".into());
    } else {
        argv.push("-map".into());
        argv.push("0:a".into());
    }
    argv.extend(
        [
            "-map_metadata",
            "-1",
            "-map_chapters",
            "-1",
            "-bitexact",
            "-c",
            "copy",
        ]
        .into_iter()
        .map(String::from),
    );
    argv.push(out_name.to_string());
    argv
}

/// Plan a strip for `in_name`: parse the `cover_art` choice, derive the
/// same-container `out.<ext>` output name and build the argv. Single source
/// shared by the chat block (`src/lib.rs`) and page (`web/src/lib.rs`).
pub fn plan(in_name: &str, cover_art: &str) -> Result<(Vec<String>, String), String> {
    if in_name.trim().is_empty() {
        return Err("input filename is empty".to_string());
    }
    let mode = parse_cover_art(cover_art)?;
    let out_name = format!("out.{}", ext_of(in_name));
    Ok((build_argv(in_name, &out_name, mode.keeps_cover()), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_all_metadata_and_cover_by_default() {
        let (argv, out) = plan("in.mp3", "remove").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-map",
                "0:a",
                "-map_metadata",
                "-1",
                "-map_chapters",
                "-1",
                "-bitexact",
                "-c",
                "copy",
                "out.mp3",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn keep_cover_art_maps_all_streams() {
        let (argv, _) = plan("in.flac", "keep").unwrap();
        // Keeps the whole input (`-map 0`) so the picture stream survives...
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "0"));
        assert!(!argv.windows(2).any(|w| w[0] == "-map" && w[1] == "0:a"));
        // ...but still drops the text tags + chapters.
        assert!(argv.windows(2).any(|w| w[0] == "-map_metadata" && w[1] == "-1"));
        assert!(argv.windows(2).any(|w| w[0] == "-map_chapters" && w[1] == "-1"));
    }

    #[test]
    fn always_stream_copies_and_never_reencodes() {
        for keep in ["remove", "keep"] {
            let (argv, _) = plan("in.ogg", keep).unwrap();
            assert!(
                argv.windows(2).any(|w| w[0] == "-c" && w[1] == "copy"),
                "cover_art={keep} must stream-copy"
            );
            assert!(
                !argv.iter().any(|a| a == "-b:a" || a == "libmp3lame"),
                "cover_art={keep} must not re-encode"
            );
        }
    }

    #[test]
    fn always_drops_metadata_and_chapters_for_both_modes() {
        for keep in ["remove", "keep"] {
            let (argv, _) = plan("in.m4a", keep).unwrap();
            assert!(argv.windows(2).any(|w| w[0] == "-map_metadata" && w[1] == "-1"));
            assert!(argv.windows(2).any(|w| w[0] == "-map_chapters" && w[1] == "-1"));
            assert!(argv.iter().any(|a| a == "-bitexact"));
        }
    }

    #[test]
    fn output_keeps_input_container() {
        for (name, out) in [
            ("in.mp3", "out.mp3"),
            ("in.flac", "out.flac"),
            ("in.ogg", "out.ogg"),
            ("in.m4a", "out.m4a"),
            ("in.wav", "out.wav"),
        ] {
            let (_, out_name) = plan(name, "remove").unwrap();
            assert_eq!(out_name, out, "input {name}");
        }
    }

    #[test]
    fn ext_of_lowercases_and_defaults() {
        assert_eq!(ext_of("song.MP3"), "mp3");
        assert_eq!(ext_of("in.flac"), "flac");
        assert_eq!(ext_of("noext"), "mp3");
    }

    #[test]
    fn parse_cover_art_maps_values_and_default() {
        assert_eq!(parse_cover_art("").unwrap(), CoverArt::Remove);
        assert_eq!(parse_cover_art("remove").unwrap(), CoverArt::Remove);
        assert_eq!(parse_cover_art("KEEP").unwrap(), CoverArt::Keep);
        assert!(parse_cover_art("maybe").is_err());
        assert!(CoverArt::Keep.keeps_cover());
        assert!(!CoverArt::Remove.keeps_cover());
    }

    #[test]
    fn empty_input_is_an_error() {
        assert!(plan("", "remove").is_err());
        assert!(plan("   ", "remove").is_err());
    }

    #[test]
    fn unknown_cover_art_is_an_error() {
        assert!(plan("in.mp3", "bogus").is_err());
    }
}
