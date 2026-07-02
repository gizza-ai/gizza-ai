//! gizza-ai/audio-compress core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Shrinks an audio file by re-encoding it at a lower lossy bitrate. Only
//! lossy targets (mp3/ogg/m4a) make sense here — re-encoding *to* lossless
//! wav/flac never reduces size, that's audio-convert's job. Out-of-range
//! bitrates are rejected, not clamped: a user asking for 8 kbps or 1000 kbps
//! should learn the supported range, not silently get a different file.
//! `-vn` drops any attached-picture (album-art) video stream that would
//! otherwise be re-muxed into the output.

/// Lossy output formats audio-compress can write.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Mp3,
    Ogg,
    M4a,
}

impl Format {
    /// Lower-cased file extension this format writes (used for `out.<ext>`).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Mp3 => "mp3",
            Format::Ogg => "ogg",
            Format::M4a => "m4a",
        }
    }

    /// IANA media type for the produced file.
    pub fn mime(self) -> &'static str {
        match self {
            Format::Mp3 => "audio/mpeg",
            Format::Ogg => "audio/ogg",
            Format::M4a => "audio/mp4",
        }
    }

    /// Encoder for `-c:a`.
    fn codec(self) -> &'static str {
        match self {
            Format::Mp3 => "libmp3lame",
            Format::Ogg => "libvorbis",
            Format::M4a => "aac",
        }
    }
}

/// Parse the user-facing format string. Empty defaults to mp3 (the most
/// portable target). Lossless formats get a pointer to audio-convert instead
/// of a generic "not supported".
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "mp3" => Ok(Format::Mp3),
        "ogg" => Ok(Format::Ogg),
        "m4a" => Ok(Format::M4a),
        "wav" | "flac" => Err(
            "wav/flac are lossless — re-encoding to them never shrinks a file; \
             use the audio-convert tool for lossless targets"
            .to_string(),
        ),
        other => Err(format!("format {other:?} not supported (mp3|ogg|m4a)")),
    }
}

/// Default target bitrate (kbps) — small enough to clearly shrink typical
/// 128–320 kbps sources, high enough to stay listenable for music.
pub const DEFAULT_BITRATE: u32 = 96;
/// Encoder-supported bitrate range (kbps), inclusive.
pub const MIN_BITRATE: u32 = 32;
pub const MAX_BITRATE: u32 = 320;

/// Reject (don't clamp) bitrates outside the supported range.
pub fn validate_bitrate(kbps: u32) -> Result<u32, String> {
    if (MIN_BITRATE..=MAX_BITRATE).contains(&kbps) {
        Ok(kbps)
    } else {
        Err(format!(
            "bitrate {kbps} kbps out of range ({MIN_BITRATE}-{MAX_BITRATE})"
        ))
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to re-encode `in_name` into
/// `out_name` at `kbps`. Shared verbatim by the web page and the chat block.
pub fn build_argv(in_name: &str, out_name: &str, format: Format, kbps: u32) -> Vec<String> {
    vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vn".to_string(),
        "-c:a".to_string(),
        format.codec().to_string(),
        "-b:a".to_string(),
        format!("{kbps}k"),
        out_name.to_string(),
    ]
}

/// Parse `format`, validate `bitrate`, and return `(argv, out_name)` for an
/// input file. `out_name` is `out.<ext>` for the chosen format. Single source
/// shared by the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_compress(
    in_name: &str,
    format: &str,
    bitrate: u32,
) -> Result<(Vec<String>, String), String> {
    let fmt = parse_format(format)?;
    let kbps = validate_bitrate(bitrate)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, fmt, kbps), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mp3_argv_order_and_values() {
        let (argv, out) = plan_compress("in.mp3", "", DEFAULT_BITRATE).unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i", "in.mp3", "-vn", "-c:a", "libmp3lame", "-b:a", "96k", "out.mp3",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn every_format_maps_to_its_codec_and_out_name() {
        for (f, codec, out) in [
            ("mp3", "libmp3lame", "out.mp3"),
            ("ogg", "libvorbis", "out.ogg"),
            ("m4a", "aac", "out.m4a"),
        ] {
            let (argv, out_name) = plan_compress("in.wav", f, 64).unwrap();
            assert_eq!(out_name, out, "format {f}");
            assert!(
                argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == codec),
                "format {f} must use {codec}"
            );
            assert!(
                argv.windows(2).any(|w| w[0] == "-b:a" && w[1] == "64k"),
                "format {f} must carry the bitrate"
            );
        }
    }

    #[test]
    fn argv_always_drops_video_streams() {
        // Album-art mp3s carry an attached-picture video stream; without -vn
        // it would be re-muxed into the output and inflate it.
        for f in ["mp3", "ogg", "m4a"] {
            let (argv, _) = plan_compress("in.mp3", f, 96).unwrap();
            assert!(argv.iter().any(|a| a == "-vn"), "format {f} missing -vn");
        }
    }

    #[test]
    fn out_of_range_bitrate_is_an_error_not_a_clamp() {
        for bad in [0, 31, 321, 9999] {
            let err = plan_compress("in.mp3", "mp3", bad).unwrap_err();
            assert!(err.contains("out of range"), "bitrate {bad}: {err}");
        }
        // Boundaries are valid.
        assert!(plan_compress("in.mp3", "mp3", 32).is_ok());
        assert!(plan_compress("in.mp3", "mp3", 320).is_ok());
    }

    #[test]
    fn lossless_targets_point_at_audio_convert() {
        for f in ["wav", "flac", "WAV"] {
            let err = plan_compress("in.mp3", f, 96).unwrap_err();
            assert!(err.contains("audio-convert"), "format {f}: {err}");
        }
    }

    #[test]
    fn parse_format_defaults_empty_to_mp3_and_rejects_unknown() {
        assert_eq!(parse_format("").unwrap(), Format::Mp3);
        assert_eq!(parse_format("OGG").unwrap(), Format::Ogg);
        assert!(parse_format("aiff").is_err());
    }

    #[test]
    fn format_ext_and_mime_pairs() {
        assert_eq!(Format::Mp3.ext(), "mp3");
        assert_eq!(Format::Mp3.mime(), "audio/mpeg");
        assert_eq!(Format::Ogg.ext(), "ogg");
        assert_eq!(Format::Ogg.mime(), "audio/ogg");
        assert_eq!(Format::M4a.ext(), "m4a");
        assert_eq!(Format::M4a.mime(), "audio/mp4");
    }
}
