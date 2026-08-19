//! gizza-ai/audio-reverse core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Plays an audio clip backwards. `areverse` buffers the decoded stream and
//! emits its samples in reverse order, so the result is sample-exact (no
//! resampling, no pitch change — a backwards clip has the same spectrum, just
//! reversed envelopes).
//!
//! Three modes:
//! * `reverse` — the whole clip backwards (`-af areverse`).
//! * `forward-reverse` — the clip, then its reversal (a palindrome/boomerang).
//! * `reverse-forward` — the reversal first, then the original. The classic
//!   reverse-cymbal / riser build-up: the swell rises into the downbeat.
//!
//! The two combined modes need `asplit` (one decode feeds both branches) plus
//! `concat`, so they use `-filter_complex` + `-map [out]` rather than `-af` —
//! the same shape audio-bleep-censor already ships and @ffmpeg/core runs.
//! `-vn` drops attached-picture streams (album art), which would otherwise make
//! the concat graph fail on a mismatched stream count.

/// Output audio formats this tool can write (family-standard set — the same
/// five every competitor reverse tool with a format selector offers).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Mp3,
    Wav,
    Ogg,
    Flac,
    M4a,
}

impl Format {
    /// Lower-cased file extension this format writes (used for `out.<ext>`).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Mp3 => "mp3",
            Format::Wav => "wav",
            Format::Ogg => "ogg",
            Format::Flac => "flac",
            Format::M4a => "m4a",
        }
    }

    /// IANA media type for the produced file.
    pub fn mime(self) -> &'static str {
        match self {
            Format::Mp3 => "audio/mpeg",
            Format::Wav => "audio/wav",
            Format::Ogg => "audio/ogg",
            Format::Flac => "audio/flac",
            Format::M4a => "audio/mp4",
        }
    }

    /// Encoder argv fragment (`-c:a …`); lossy formats are fixed at 192 kbps,
    /// matching the rest of the audio block family.
    fn codec_args(self) -> Vec<String> {
        match self {
            Format::Mp3 => vec![
                "-c:a".into(),
                "libmp3lame".into(),
                "-b:a".into(),
                "192k".into(),
            ],
            Format::Wav => vec!["-c:a".into(), "pcm_s16le".into()],
            Format::Ogg => vec![
                "-c:a".into(),
                "libvorbis".into(),
                "-b:a".into(),
                "192k".into(),
            ],
            Format::Flac => vec!["-c:a".into(), "flac".into()],
            Format::M4a => vec!["-c:a".into(), "aac".into()],
        }
    }
}

/// Parse the user-facing format string. Empty defaults to mp3.
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "mp3" => Ok(Format::Mp3),
        "wav" => Ok(Format::Wav),
        "ogg" => Ok(Format::Ogg),
        "flac" => Ok(Format::Flac),
        "m4a" => Ok(Format::M4a),
        other => Err(format!(
            "format {other:?} not supported (mp3|wav|ogg|flac|m4a)"
        )),
    }
}

/// What to write out: the reversal alone, or the clip and its reversal joined.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    /// Just the clip backwards (default).
    Reverse,
    /// Original first, reversal second — a palindrome that plays out backwards.
    ForwardReverse,
    /// Reversal first, original second — the reverse-cymbal swell into the hit.
    ReverseForward,
}

impl Mode {
    /// Filename suffix describing the produced file.
    pub fn suffix(self) -> &'static str {
        match self {
            Mode::Reverse => "-reversed",
            Mode::ForwardReverse => "-forward-reverse",
            Mode::ReverseForward => "-reverse-forward",
        }
    }

    /// Human phrasing for the chat/CLI summary line.
    pub fn describe(self) -> &'static str {
        match self {
            Mode::Reverse => "reversed",
            Mode::ForwardReverse => "original followed by its reversal",
            Mode::ReverseForward => "reversal followed by the original",
        }
    }

    /// True when the output is roughly twice as long as the input.
    pub fn is_doubled(self) -> bool {
        !matches!(self, Mode::Reverse)
    }
}

/// Parse the user-facing mode string. Empty defaults to plain reverse.
/// Underscores are accepted as separators so `forward_reverse` also works.
pub fn parse_mode(s: &str) -> Result<Mode, String> {
    match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "" | "reverse" => Ok(Mode::Reverse),
        "forward-reverse" => Ok(Mode::ForwardReverse),
        "reverse-forward" => Ok(Mode::ReverseForward),
        other => Err(format!(
            "mode {other:?} not supported (reverse|forward-reverse|reverse-forward)"
        )),
    }
}

/// The `-filter_complex` graph for the two combined modes: split the decoded
/// audio in two, reverse one branch, then concat the branches in the requested
/// order. `concat=n=2:v=0:a=1` joins audio-only streams end to end.
fn concat_graph(reverse_first: bool) -> String {
    let order = if reverse_first { "[r][a]" } else { "[a][r]" };
    format!("[0:a]asplit=2[a][b];[b]areverse[r];{order}concat=n=2:v=0:a=1[out]")
}

/// Build the ffmpeg argv (no leading `ffmpeg`) to reverse `in_name` into
/// `out_name`. Shared verbatim by the web page (`build_argv`) and the chat block.
pub fn build_argv(in_name: &str, out_name: &str, mode: Mode, format: Format) -> Vec<String> {
    let mut argv = vec!["-i".to_string(), in_name.to_string(), "-vn".to_string()];
    match mode {
        Mode::Reverse => {
            argv.push("-af".to_string());
            argv.push("areverse".to_string());
        }
        Mode::ForwardReverse | Mode::ReverseForward => {
            argv.push("-filter_complex".to_string());
            argv.push(concat_graph(mode == Mode::ReverseForward));
            argv.push("-map".to_string());
            argv.push("[out]".to_string());
        }
    }
    argv.extend(format.codec_args());
    argv.push(out_name.to_string());
    argv
}

/// Validate params, parse `mode` + `format`, and return `(argv, out_name)`.
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
pub fn plan(in_name: &str, mode: &str, format: &str) -> Result<(Vec<String>, String), String> {
    let m = parse_mode(mode)?;
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, m, fmt), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_reverse_is_a_single_areverse_filter() {
        let (argv, out) = plan("in.mp3", "reverse", "mp3").unwrap();
        assert_eq!(out, "out.mp3");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-vn",
                "-af",
                "areverse",
                "-c:a",
                "libmp3lame",
                "-b:a",
                "192k",
                "out.mp3",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn empty_mode_and_format_fall_back_to_reverse_mp3() {
        let (argv, out) = plan("in.wav", "", "").unwrap();
        assert_eq!(out, "out.mp3");
        assert!(argv.windows(2).any(|w| w[0] == "-af" && w[1] == "areverse"));
    }

    #[test]
    fn forward_reverse_concats_original_then_reversal() {
        let (argv, out) = plan("in.wav", "forward-reverse", "wav").unwrap();
        assert_eq!(out, "out.wav");
        let fc = argv.iter().position(|a| a == "-filter_complex").unwrap();
        assert_eq!(
            argv[fc + 1],
            "[0:a]asplit=2[a][b];[b]areverse[r];[a][r]concat=n=2:v=0:a=1[out]"
        );
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "[out]"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "pcm_s16le"));
        // The combined modes must NOT also pass -af, or ffmpeg rejects the graph.
        assert!(!argv.iter().any(|a| a == "-af"), "{argv:?}");
    }

    #[test]
    fn reverse_forward_puts_the_reversal_first() {
        let (argv, _) = plan("in.flac", "reverse-forward", "flac").unwrap();
        let fc = argv.iter().position(|a| a == "-filter_complex").unwrap();
        assert_eq!(
            argv[fc + 1],
            "[0:a]asplit=2[a][b];[b]areverse[r];[r][a]concat=n=2:v=0:a=1[out]"
        );
    }

    #[test]
    fn underscore_mode_spelling_is_accepted() {
        assert_eq!(parse_mode("Forward_Reverse").unwrap(), Mode::ForwardReverse);
        assert_eq!(parse_mode("  REVERSE  ").unwrap(), Mode::Reverse);
    }

    #[test]
    fn album_art_is_always_dropped() {
        for mode in ["reverse", "forward-reverse", "reverse-forward"] {
            let (argv, _) = plan("in.mp3", mode, "mp3").unwrap();
            assert!(argv.iter().any(|a| a == "-vn"), "{mode}: {argv:?}");
        }
    }

    #[test]
    fn rejects_unknown_mode() {
        let err = plan("in.mp3", "backwards", "mp3").unwrap_err();
        assert!(err.contains("not supported"), "{err}");
        assert!(err.contains("reverse-forward"), "{err}");
    }

    #[test]
    fn rejects_unknown_format() {
        let err = plan("in.mp3", "reverse", "aiff").unwrap_err();
        assert!(err.contains("not supported"), "{err}");
    }

    #[test]
    fn each_format_maps_to_ext_and_mime() {
        for (s, ext, mime) in [
            ("mp3", "mp3", "audio/mpeg"),
            ("wav", "wav", "audio/wav"),
            ("ogg", "ogg", "audio/ogg"),
            ("flac", "flac", "audio/flac"),
            ("m4a", "m4a", "audio/mp4"),
        ] {
            let f = parse_format(s).unwrap();
            assert_eq!(f.ext(), ext);
            assert_eq!(f.mime(), mime);
            let (argv, out) = plan("in.mp3", "reverse", s).unwrap();
            assert_eq!(out, format!("out.{ext}"));
            assert_eq!(argv.last().unwrap(), &format!("out.{ext}"));
        }
    }

    #[test]
    fn suffix_and_doubling_track_the_mode() {
        assert_eq!(Mode::Reverse.suffix(), "-reversed");
        assert!(!Mode::Reverse.is_doubled());
        assert_eq!(Mode::ForwardReverse.suffix(), "-forward-reverse");
        assert!(Mode::ForwardReverse.is_doubled());
        assert_eq!(Mode::ReverseForward.suffix(), "-reverse-forward");
        assert!(Mode::ReverseForward.is_doubled());
    }
}
