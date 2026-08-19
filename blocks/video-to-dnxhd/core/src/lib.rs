//! gizza-ai/video-to-dnxhd core — pure ffmpeg argv construction shared by the
//! chat block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! video-to-dnxhd re-encodes ANY input video into an **Avid DNxHD/DNxHR**
//! intermediate inside either a QuickTime **`.mov`** or an **`.mxf`** wrapper,
//! which is what Media Composer, DaVinci Resolve and Premiere Pro expect from a
//! mezzanine hand-off.
//!
//! Only the **DNxHR** profile family is exposed. DNxHR is resolution- and
//! frame-rate-independent, so it accepts arbitrary source geometry; classic
//! DNxHD is locked to a fixed table of (raster, fps, bitrate) triples — feed it
//! anything off that table and ffmpeg refuses with "video parameters
//! incompatible with DNxHD". Exposing DNxHD would therefore mean shipping a
//! knob that fails for most real uploads, so this tool always writes DNxHR
//! (the encoder is the same `dnxhd` codec, and the files are what modern Avid
//! and Resolve workflows actually want).
//!
//! The invariant that makes DNx different from every other encoder here: the
//! pixel format is DICTATED by the profile, not chosen freely. ffmpeg's
//! `dnxhd` encoder hard-rejects any other pairing:
//! - `dnxhr_lb` / `dnxhr_sq` / `dnxhr_hq` → `yuv422p` (8-bit 4:2:2)
//! - `dnxhr_hqx`                          → `yuv422p10le` (10-bit 4:2:2)
//! - `dnxhr_444`                          → `yuv444p10le` (10-bit 4:4:4)
//!
//! So `pixel_format` is a VALIDATED override, not a free choice: `auto` (the
//! default) resolves to the profile's native format, and an explicit value is
//! accepted only when it is the one the profile allows — otherwise the caller
//! gets a message naming the correct format instead of an opaque ffmpeg error.
//!
//! Deliberately distinct from its neighbours:
//! - `video-to-prores` writes Apple ProRes 422 (`prores_ks`) and only ever a
//!   `.mov`. ProRes and DNxHD/DNxHR are competing intermediates, not aliases:
//!   different encoder, different fourcc, different NLE ecosystem, and DNx is
//!   the one with an MXF wrapper.
//! - `video-transcode` / `video-to-h264` / `video-to-hevc` target DELIVERY
//!   codecs with a CRF knob. This tool always re-encodes to an intra-frame
//!   mezzanine, which is far larger and not meant for the web.
//!
//! Not covered here: fixed-raster DNxHD (see above), DNxUncompressed, and
//! OP-Atom MXF (Avid's per-track flavour — `mxf_opatom` accepts exactly one
//! stream, so it cannot carry video+audio in a single file like this tool does).

/// The DNxHR profile the output is encoded at. Each tier is a different
/// quality/bitrate point, and each pins its own pixel format.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Profile {
    /// `dnxhr_lb` — Low Bandwidth, roughly 22:1 compression. Offline/proxy
    /// editing; relink to the originals before the finish.
    Lb,
    /// `dnxhr_sq` — Standard Quality, roughly 7:1. The default: broadcast
    /// delivery and everyday online editing.
    Sq,
    /// `dnxhr_hq` — High Quality, roughly 4.5:1, still 8-bit 4:2:2.
    Hq,
    /// `dnxhr_hqx` — High Quality 10-bit 4:2:2. The first tier with real
    /// grading headroom; the usual pick for HDR/wide-gamut finishing.
    Hqx,
    /// `dnxhr_444` — 10-bit 4:4:4, no chroma subsampling. Compositing, keying
    /// and colour-critical VFX round-trips.
    P444,
}

impl Profile {
    /// The exact string passed to ffmpeg's `-profile:v` for the `dnxhd` encoder.
    pub fn as_arg(self) -> &'static str {
        match self {
            Profile::Lb => "dnxhr_lb",
            Profile::Sq => "dnxhr_sq",
            Profile::Hq => "dnxhr_hq",
            Profile::Hqx => "dnxhr_hqx",
            Profile::P444 => "dnxhr_444",
        }
    }

    /// The ONLY pixel format ffmpeg's `dnxhd` encoder accepts for this profile.
    /// Anything else fails at encoder-open with "pixel format is incompatible
    /// with DNxHR …", so this is validated up front instead.
    pub fn pix_fmt(self) -> &'static str {
        match self {
            Profile::Lb | Profile::Sq | Profile::Hq => "yuv422p",
            Profile::Hqx => "yuv422p10le",
            Profile::P444 => "yuv444p10le",
        }
    }
}

/// Parse the user-facing profile string (the values the chat schema + page accept).
pub fn parse_profile(s: &str) -> Result<Profile, String> {
    match s {
        "dnxhr_lb" => Ok(Profile::Lb),
        "dnxhr_sq" => Ok(Profile::Sq),
        "dnxhr_hq" => Ok(Profile::Hq),
        "dnxhr_hqx" => Ok(Profile::Hqx),
        "dnxhr_444" => Ok(Profile::P444),
        other => Err(format!(
            "profile {other:?} not supported \
             (dnxhr_lb|dnxhr_sq|dnxhr_hq|dnxhr_hqx|dnxhr_444)"
        )),
    }
}

/// Default profile when none is supplied — DNxHR SQ, the everyday editing tier.
pub const DEFAULT_PROFILE: Profile = Profile::Sq;

/// The wrapper the DNx essence is written into.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Container {
    /// `mov` — QuickTime. The default: opens everywhere, and the video stream
    /// carries the `AVdh` codec tag that identifies DNxHD/DNxHR.
    Mov,
    /// `mxf` — SMPTE MXF OP1a, the broadcast/Avid interchange wrapper.
    Mxf,
}

impl Container {
    /// The output file extension, which is also how ffmpeg picks the muxer.
    pub fn ext(self) -> &'static str {
        match self {
            Container::Mov => "mov",
            Container::Mxf => "mxf",
        }
    }

    /// MIME type for the produced file. MXF has no `video/*` registration, so
    /// `application/mxf` (RFC 4539) is the correct type.
    pub fn mime(self) -> &'static str {
        match self {
            Container::Mov => "video/quicktime",
            Container::Mxf => "application/mxf",
        }
    }

    /// Whether the muxer needs `-strict unofficial`.
    ///
    /// The MXF muxer only admits the broadcast frame rates in the SMPTE tables
    /// (24, 25, 30, 50, 60, and the 1000/1001 variants) and hard-fails with
    /// "Unsupported frame rate N/1" on anything else — which includes plenty of
    /// real phone/screen-capture footage. Loosening the check keeps those
    /// sources working; it affects nothing for standard-rate input, and the
    /// alternative (silently resampling the timeline) would be worse.
    pub fn needs_strict_unofficial(self) -> bool {
        matches!(self, Container::Mxf)
    }
}

/// Parse the user-facing container string (the values the chat schema + page accept).
pub fn parse_container(s: &str) -> Result<Container, String> {
    match s {
        "mov" => Ok(Container::Mov),
        "mxf" => Ok(Container::Mxf),
        other => Err(format!("container {other:?} not supported (mov|mxf)")),
    }
}

/// Default wrapper — QuickTime `.mov`.
pub const DEFAULT_CONTAINER: Container = Container::Mov;

/// Parse the user-facing `resolution` string into a target height cap in pixels.
/// `source` (the default) means "don't scale at all" and maps to `None`.
///
/// The values carry a `p` suffix on purpose: the ffmpeg page driver
/// numeric-coerces numeric-LOOKING field strings before calling `build_argv`,
/// so a bare `"720"` would arrive at the wasm export as a JS number and throw.
pub fn parse_resolution(s: &str) -> Result<Option<u32>, String> {
    match s {
        "source" => Ok(None),
        "2160p" => Ok(Some(2160)),
        "1080p" => Ok(Some(1080)),
        "720p" => Ok(Some(720)),
        other => Err(format!(
            "resolution {other:?} not supported (source|2160p|1080p|720p)"
        )),
    }
}

/// Parse the user-facing `pixel_format` string.
///
/// `auto` (the default) defers to the profile. Any other value is a request to
/// pin a specific format, which is honoured ONLY when it is the format the
/// profile mandates — see the module docs for why there is no freedom here.
/// Returns the resolved format string.
pub fn resolve_pixel_format(pixel_format: &str, profile: Profile) -> Result<&'static str, String> {
    let native = profile.pix_fmt();
    match pixel_format {
        "auto" => Ok(native),
        "yuv422p" | "yuv422p10le" | "yuv444p10le" => {
            if pixel_format == native {
                Ok(native)
            } else {
                Err(format!(
                    "pixel_format {pixel_format:?} is incompatible with profile {:?} \
                     (DNxHR pins the pixel format per profile: use {native:?} or \"auto\")",
                    profile.as_arg()
                ))
            }
        }
        other => Err(format!(
            "pixel_format {other:?} not supported (auto|yuv422p|yuv422p10le|yuv444p10le)"
        )),
    }
}

/// What to do with the input's audio tracks.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Audio {
    /// `pcm16` — uncompressed 16-bit little-endian PCM (`pcm_s16le`). The
    /// default: NLEs expect uncompressed audio alongside a DNx picture track.
    Pcm16,
    /// `pcm24` — uncompressed 24-bit PCM (`pcm_s24le`), for 24-bit masters.
    Pcm24,
    /// `copy` — stream-copy the source audio untouched (`-c:a copy`). Fastest,
    /// but the wrapper has to accept the incoming codec.
    Copy,
    /// `none` — drop audio entirely (`-an`): a picture-only intermediate.
    None,
}

impl Audio {
    /// The value for `-c:a`, or `None` when audio is dropped with `-an`.
    pub fn codec(self) -> Option<&'static str> {
        match self {
            Audio::Pcm16 => Some("pcm_s16le"),
            Audio::Pcm24 => Some("pcm_s24le"),
            Audio::Copy => Some("copy"),
            Audio::None => None,
        }
    }
}

/// Parse the user-facing audio string (the values the chat schema + page accept).
pub fn parse_audio(s: &str) -> Result<Audio, String> {
    match s {
        "pcm16" => Ok(Audio::Pcm16),
        "pcm24" => Ok(Audio::Pcm24),
        "copy" => Ok(Audio::Copy),
        "none" => Ok(Audio::None),
        other => Err(format!(
            "audio {other:?} not supported (pcm16|pcm24|copy|none)"
        )),
    }
}

/// Default audio handling — uncompressed 16-bit PCM.
pub const DEFAULT_AUDIO: Audio = Audio::Pcm16;

/// The smallest raster ffmpeg's `dnxhd` encoder will accept. Below this it
/// aborts with "Input dimensions too small, input must be at least 256x120".
/// Documented on the page so the failure is predictable rather than mysterious;
/// the tool does NOT silently upscale to reach it.
pub const MIN_WIDTH: u32 = 256;
/// See [`MIN_WIDTH`].
pub const MIN_HEIGHT: u32 = 120;

/// Build the downscale-only filter for a target height cap.
///
/// `scale=-2:'min(ih,720)'` caps the height at 720 while leaving anything
/// SHORTER untouched (`min` never grows the picture — asking for 2160p from a
/// 1080p source is a no-op, not a blurry upscale), and `-2` derives the width
/// from the source aspect ratio rounded to an even number, which DNxHR's 4:2:2
/// chroma subsampling requires. The single quotes are parsed by ffmpeg's own
/// filtergraph parser (they protect the comma inside `min(...)`), so this
/// string is passed as ONE argv element with no shell involved.
pub fn scale_filter(max_height: u32) -> String {
    format!("scale=-2:'min(ih,{max_height})'")
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that re-encodes `in_name` →
/// `out_name` as DNxHR: optional downscale, `dnxhd` at `profile`, the resolved
/// pixel format, the MXF strictness relaxation when wrapping in MXF, and the
/// chosen audio handling.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    profile: Profile,
    container: Container,
    max_height: Option<u32>,
    pix_fmt: &str,
    audio: Audio,
) -> Vec<String> {
    let mut argv: Vec<String> = vec!["-i".into(), in_name.into()];
    if let Some(h) = max_height {
        argv.push("-vf".into());
        argv.push(scale_filter(h));
    }
    argv.extend([
        "-c:v".into(),
        "dnxhd".into(),
        "-profile:v".into(),
        profile.as_arg().into(),
        "-pix_fmt".into(),
        pix_fmt.to_string(),
    ]);
    if container.needs_strict_unofficial() {
        argv.extend(["-strict".into(), "unofficial".into()]);
    }
    match audio.codec() {
        // `-c:a …` is a harmless no-op when the input has no audio track.
        Some(codec) => argv.extend(["-c:a".into(), codec.to_string()]),
        None => argv.push("-an".into()),
    }
    argv.push(out_name.into());
    argv
}

/// Validate + parse the user-facing strings and build `(argv, out_name)` for an
/// input file. `out_name` follows the chosen container (`out.mov` / `out.mxf`).
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
pub fn plan(
    profile: &str,
    container: &str,
    resolution: &str,
    pixel_format: &str,
    audio: &str,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    let p = parse_profile(profile)?;
    let c = parse_container(container)?;
    let max_height = parse_resolution(resolution)?;
    let pf = resolve_pixel_format(pixel_format, p)?;
    let a = parse_audio(audio)?;
    let out_name = format!("out.{}", c.ext());
    Ok((
        build_argv(in_name, &out_name, p, c, max_height, pf, a),
        out_name,
    ))
}

/// MIME type for the output of a plan built with `container`. Kept next to
/// [`Container::mime`] so the chat block does not re-derive it from the name.
pub fn output_mime(container: &str) -> Result<&'static str, String> {
    Ok(parse_container(container)?.mime())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_pair(argv: &[String], k: &str, v: &str) -> bool {
        argv.windows(2).any(|w| w[0] == k && w[1] == v)
    }

    #[test]
    fn parse_profile_known() {
        assert_eq!(parse_profile("dnxhr_lb").unwrap(), Profile::Lb);
        assert_eq!(parse_profile("dnxhr_sq").unwrap(), Profile::Sq);
        assert_eq!(parse_profile("dnxhr_hq").unwrap(), Profile::Hq);
        assert_eq!(parse_profile("dnxhr_hqx").unwrap(), Profile::Hqx);
        assert_eq!(parse_profile("dnxhr_444").unwrap(), Profile::P444);
        assert_eq!(DEFAULT_PROFILE, Profile::Sq);
    }

    #[test]
    fn parse_profile_rejects_fixed_raster_dnxhd_and_junk() {
        // Fixed-raster DNxHD profiles are real ffmpeg values but deliberately
        // unsupported (they only encode an exact raster/fps/bitrate triple), so
        // they must be rejected rather than silently mapped onto a DNxHR tier.
        assert!(parse_profile("dnxhd").is_err());
        assert!(parse_profile("dnxhd120").is_err());
        assert!(parse_profile("dnxhr_hqz").is_err());
        assert!(parse_profile("lb").is_err());
        assert!(parse_profile("").is_err());
    }

    #[test]
    fn parse_container_known_and_unknown() {
        assert_eq!(parse_container("mov").unwrap(), Container::Mov);
        assert_eq!(parse_container("mxf").unwrap(), Container::Mxf);
        assert_eq!(DEFAULT_CONTAINER, Container::Mov);
        assert!(parse_container("mp4").is_err());
        assert!(parse_container("MOV").is_err());
        assert!(parse_container("").is_err());
    }

    #[test]
    fn parse_resolution_known_and_unknown() {
        assert_eq!(parse_resolution("source").unwrap(), None);
        assert_eq!(parse_resolution("2160p").unwrap(), Some(2160));
        assert_eq!(parse_resolution("1080p").unwrap(), Some(1080));
        assert_eq!(parse_resolution("720p").unwrap(), Some(720));
        // Bare numbers are NOT accepted: the page driver would coerce them to a
        // JS number before build_argv ever sees them.
        assert!(parse_resolution("720").is_err());
        assert!(parse_resolution("4k").is_err());
        assert!(parse_resolution("").is_err());
    }

    #[test]
    fn parse_audio_known_and_unknown() {
        assert_eq!(parse_audio("pcm16").unwrap(), Audio::Pcm16);
        assert_eq!(parse_audio("pcm24").unwrap(), Audio::Pcm24);
        assert_eq!(parse_audio("copy").unwrap(), Audio::Copy);
        assert_eq!(parse_audio("none").unwrap(), Audio::None);
        assert_eq!(DEFAULT_AUDIO, Audio::Pcm16);
        assert!(parse_audio("aac").is_err());
        assert!(parse_audio("mp3").is_err());
    }

    #[test]
    fn every_profile_pins_the_pixel_format_ffmpeg_demands() {
        // These pairings are not stylistic — ffmpeg's dnxhd encoder rejects
        // every other combination at encoder-open time.
        assert_eq!(Profile::Lb.pix_fmt(), "yuv422p");
        assert_eq!(Profile::Sq.pix_fmt(), "yuv422p");
        assert_eq!(Profile::Hq.pix_fmt(), "yuv422p");
        assert_eq!(Profile::Hqx.pix_fmt(), "yuv422p10le");
        assert_eq!(Profile::P444.pix_fmt(), "yuv444p10le");
    }

    #[test]
    fn pixel_format_auto_follows_the_profile() {
        assert_eq!(resolve_pixel_format("auto", Profile::Sq).unwrap(), "yuv422p");
        assert_eq!(
            resolve_pixel_format("auto", Profile::Hqx).unwrap(),
            "yuv422p10le"
        );
        assert_eq!(
            resolve_pixel_format("auto", Profile::P444).unwrap(),
            "yuv444p10le"
        );
    }

    #[test]
    fn pixel_format_explicit_is_accepted_only_when_it_matches_the_profile() {
        // Redundant-but-correct is fine: the user pinned what the profile wants.
        assert_eq!(
            resolve_pixel_format("yuv422p10le", Profile::Hqx).unwrap(),
            "yuv422p10le"
        );
        // A valid format for a DIFFERENT tier must not silently win over the
        // profile — that would produce an ffmpeg failure with no explanation.
        let e = resolve_pixel_format("yuv422p10le", Profile::Sq).unwrap_err();
        assert!(e.contains("yuv422p"), "error should name the fix: {e}");
        assert!(e.contains("dnxhr_sq"), "error should name the profile: {e}");
        let e = resolve_pixel_format("yuv444p10le", Profile::Hqx).unwrap_err();
        assert!(e.contains("yuv422p10le"), "error should name the fix: {e}");
        // Formats outside the enum are a different (parse) failure.
        let e = resolve_pixel_format("rgb24", Profile::Sq).unwrap_err();
        assert!(e.contains("auto|yuv422p|yuv422p10le|yuv444p10le"), "{e}");
    }

    #[test]
    fn argv_always_forces_the_dnxhd_encoder_and_a_dnxhr_profile() {
        let (argv, out) = plan("dnxhr_sq", "mov", "source", "auto", "pcm16", "in.mp4").unwrap();
        assert_eq!(out, "out.mov");
        assert!(has_pair(&argv, "-c:v", "dnxhd"));
        assert!(has_pair(&argv, "-profile:v", "dnxhr_sq"));
        assert!(has_pair(&argv, "-pix_fmt", "yuv422p"));
        assert!(has_pair(&argv, "-c:a", "pcm_s16le"));
        assert_eq!(argv.first().map(String::as_str), Some("-i"));
        assert_eq!(argv[1], "in.mp4");
        assert_eq!(argv.last().map(String::as_str), Some("out.mov"));
        // Delivery-codec flags have no place in an intermediate.
        assert!(!argv.iter().any(|a| a == "-crf" || a == "libx264"));
        // No scale filter at all when resolution = source.
        assert!(!argv.iter().any(|a| a == "-vf"));
        // MOV needs no strictness relaxation — that is an MXF-only concession.
        assert!(!argv.iter().any(|a| a == "-strict"));
    }

    #[test]
    fn every_profile_reaches_argv_with_its_own_pixel_format() {
        for (name, pix) in [
            ("dnxhr_lb", "yuv422p"),
            ("dnxhr_sq", "yuv422p"),
            ("dnxhr_hq", "yuv422p"),
            ("dnxhr_hqx", "yuv422p10le"),
            ("dnxhr_444", "yuv444p10le"),
        ] {
            let (argv, _) = plan(name, "mov", "source", "auto", "pcm16", "in.mov").unwrap();
            assert!(has_pair(&argv, "-profile:v", name), "{name} not in argv");
            assert!(has_pair(&argv, "-pix_fmt", pix), "{name} wrong pix_fmt");
        }
    }

    #[test]
    fn container_drives_the_extension_the_mime_and_mxf_strictness() {
        let (argv, out) = plan("dnxhr_hqx", "mxf", "source", "auto", "pcm24", "in.mp4").unwrap();
        assert_eq!(out, "out.mxf");
        assert_eq!(argv.last().map(String::as_str), Some("out.mxf"));
        // Without this the MXF muxer rejects any non-broadcast frame rate.
        assert!(has_pair(&argv, "-strict", "unofficial"));
        assert!(has_pair(&argv, "-pix_fmt", "yuv422p10le"));
        assert_eq!(output_mime("mxf").unwrap(), "application/mxf");
        assert_eq!(output_mime("mov").unwrap(), "video/quicktime");
        assert!(output_mime("mp4").is_err());
    }

    #[test]
    fn resolution_adds_a_downscale_only_filter() {
        let (argv, _) = plan("dnxhr_hq", "mov", "720p", "auto", "pcm16", "in.mkv").unwrap();
        assert!(has_pair(&argv, "-vf", "scale=-2:'min(ih,720)'"));
        // -2 keeps the width even (DNxHR 4:2:2 needs it); min() never upscales.
        assert_eq!(scale_filter(1080), "scale=-2:'min(ih,1080)'");
        assert_eq!(scale_filter(2160), "scale=-2:'min(ih,2160)'");
    }

    #[test]
    fn audio_none_drops_the_track_instead_of_encoding_it() {
        let (argv, _) = plan("dnxhr_sq", "mov", "source", "auto", "none", "in.mp4").unwrap();
        assert!(argv.iter().any(|a| a == "-an"));
        assert!(!argv.iter().any(|a| a == "-c:a"));
    }

    #[test]
    fn audio_copy_and_pcm24_select_the_right_encoder() {
        let (argv, _) = plan("dnxhr_sq", "mov", "source", "auto", "copy", "in.mp4").unwrap();
        assert!(has_pair(&argv, "-c:a", "copy"));
        assert!(!argv.iter().any(|a| a == "-an"));
        let (argv, _) = plan("dnxhr_sq", "mov", "source", "auto", "pcm24", "in.mp4").unwrap();
        assert!(has_pair(&argv, "-c:a", "pcm_s24le"));
    }

    #[test]
    fn plan_rejects_bad_params_with_a_message_naming_the_choices() {
        let e = plan("dnxhd220x", "mov", "source", "auto", "pcm16", "in.mp4").unwrap_err();
        assert!(e.contains("dnxhr_lb|dnxhr_sq"), "unhelpful error: {e}");
        let e = plan("dnxhr_sq", "mp4", "source", "auto", "pcm16", "in.mp4").unwrap_err();
        assert!(e.contains("mov|mxf"), "unhelpful error: {e}");
        let e = plan("dnxhr_sq", "mov", "8k", "auto", "pcm16", "in.mp4").unwrap_err();
        assert!(e.contains("1080p"), "unhelpful error: {e}");
        let e = plan("dnxhr_sq", "mov", "source", "rgb24", "pcm16", "in.mp4").unwrap_err();
        assert!(e.contains("yuv444p10le"), "unhelpful error: {e}");
        let e = plan("dnxhr_sq", "mov", "source", "auto", "aac", "in.mp4").unwrap_err();
        assert!(e.contains("pcm16|pcm24|copy|none"), "unhelpful error: {e}");
    }

    #[test]
    fn minimum_raster_is_documented_not_silently_patched() {
        // The encoder's own floor. The argv must NOT contain an upscale pad or
        // scale that quietly changes the user's framing to satisfy it.
        assert_eq!((MIN_WIDTH, MIN_HEIGHT), (256, 120));
        let (argv, _) = plan("dnxhr_lb", "mov", "source", "auto", "pcm16", "in.mp4").unwrap();
        assert!(!argv.iter().any(|a| a.contains("pad=")));
        assert!(!argv.iter().any(|a| a.contains("256")));
    }

    #[test]
    fn input_name_is_preserved_verbatim() {
        let (argv, _) = plan("dnxhr_444", "mxf", "1080p", "auto", "none", "in.webm").unwrap();
        assert_eq!(argv[1], "in.webm");
    }
}
