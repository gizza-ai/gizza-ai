//! gizza-ai/video-to-prores core — pure ffmpeg argv construction shared by the
//! chat block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! video-to-prores re-encodes ANY input video into an **Apple ProRes 422**
//! intermediate inside a QuickTime **`.mov`** container: `prores_ks` at one of
//! the four 4:2:2 tiers (Proxy / LT / 422 / 422 HQ), forced `yuv422p10le`
//! (10-bit 4:2:2 — the property that DEFINES the 422 tier), the Apple vendor
//! atom (`-vendor apl0`) so editors recognise the file as real ProRes, and
//! uncompressed PCM audio, which is what an editorial workflow expects in a
//! `.mov`.
//!
//! ProRes is an *intermediate* (mezzanine) codec, not a delivery codec: it is
//! intra-frame only, so every frame is a keyframe and scrubbing/trimming is
//! cheap, and it is designed to survive many generations of re-encode. The
//! trade is size — plain ProRes 422 runs ~147 Mbps at 1080p29.97 (≈18 MB per
//! second of video), so the `resolution` downscale is the practical size lever.
//!
//! Deliberately distinct from its neighbours:
//! - `mp4-to-mov` REMUXES (`-c copy`) into a `.mov` and keeps whatever codec was
//!   already there — it never produces ProRes.
//! - `video-transcode` targets delivery containers/codecs (mp4 ⇆ webm) with a
//!   CRF knob; `video-to-h264` / `video-to-hevc` are delivery normalizers.
//! - This tool ALWAYS re-encodes and ALWAYS pins ProRes 422 + `yuv422p10le` +
//!   the Apple vendor atom. That forced intermediate is the whole point.
//!
//! Not covered here: ProRes 4444 / 4444 XQ (4:4:4 plus an alpha plane — a
//! different codec tier and a different, VFX-shaped user), and ProRes RAW
//! (ffmpeg has no encoder for it at all).

/// The ProRes 422 tier the output is encoded at. Every variant is 10-bit 4:2:2;
/// they differ only in target data rate (and therefore file size / headroom).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Profile {
    /// `proxy` — ~45 Mbps at 1080p29.97. Offline editing / rough cuts, then
    /// relink to the originals for the final grade.
    Proxy,
    /// `lt` — ~102 Mbps at 1080p29.97. Lighter than 422 with most of the
    /// headroom; good when storage matters more than the last stop of latitude.
    Lt,
    /// `standard` — plain "ProRes 422", ~147 Mbps at 1080p29.97. The default and
    /// the usual pick for general editing.
    Standard,
    /// `hq` — ~220 Mbps at 1080p29.97. Extra headroom for heavy grading or
    /// repeated re-encodes; visually indistinguishable from the source on most
    /// material.
    Hq,
}

impl Profile {
    /// The exact string passed to ffmpeg's `-profile:v` for `prores_ks`.
    /// (`prores_ks` also accepts the numeric forms 0/1/2/3; the names are used
    /// here because they are self-documenting in the generated CLI example.)
    pub fn as_arg(self) -> &'static str {
        match self {
            Profile::Proxy => "proxy",
            Profile::Lt => "lt",
            Profile::Standard => "standard",
            Profile::Hq => "hq",
        }
    }

    /// The QuickTime codec fourcc ffmpeg stamps into the output for this tier —
    /// what an editor (or `ffprobe -show_entries stream=codec_tag_string`) reads
    /// back to identify the variant.
    pub fn fourcc(self) -> &'static str {
        match self {
            Profile::Proxy => "apco",
            Profile::Lt => "apcs",
            Profile::Standard => "apcn",
            Profile::Hq => "apch",
        }
    }
}

/// Parse the user-facing profile string (the values the chat schema + page accept).
pub fn parse_profile(s: &str) -> Result<Profile, String> {
    match s {
        "proxy" => Ok(Profile::Proxy),
        "lt" => Ok(Profile::Lt),
        "standard" => Ok(Profile::Standard),
        "hq" => Ok(Profile::Hq),
        other => Err(format!(
            "profile {other:?} not supported (proxy|lt|standard|hq)"
        )),
    }
}

/// Default tier when none is supplied — plain ProRes 422, the general-editing pick.
pub const DEFAULT_PROFILE: Profile = Profile::Standard;

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
        "1440p" => Ok(Some(1440)),
        "1080p" => Ok(Some(1080)),
        "720p" => Ok(Some(720)),
        "540p" => Ok(Some(540)),
        "480p" => Ok(Some(480)),
        other => Err(format!(
            "resolution {other:?} not supported (source|2160p|1440p|1080p|720p|540p|480p)"
        )),
    }
}

/// What to do with the input's audio tracks.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Audio {
    /// `pcm16` — uncompressed 16-bit little-endian PCM (`pcm_s16le`). The
    /// default: editors expect uncompressed audio in a ProRes `.mov`.
    Pcm16,
    /// `pcm24` — uncompressed 24-bit PCM (`pcm_s24le`), for masters mixed at 24-bit.
    Pcm24,
    /// `none` — drop audio entirely (`-an`): a picture-only intermediate.
    None,
}

impl Audio {
    /// The ffmpeg encoder name, or `None` when audio is dropped (`-an`).
    pub fn codec(self) -> Option<&'static str> {
        match self {
            Audio::Pcm16 => Some("pcm_s16le"),
            Audio::Pcm24 => Some("pcm_s24le"),
            Audio::None => None,
        }
    }
}

/// Parse the user-facing audio string (the values the chat schema + page accept).
pub fn parse_audio(s: &str) -> Result<Audio, String> {
    match s {
        "pcm16" => Ok(Audio::Pcm16),
        "pcm24" => Ok(Audio::Pcm24),
        "none" => Ok(Audio::None),
        other => Err(format!("audio {other:?} not supported (pcm16|pcm24|none)")),
    }
}

/// Default audio handling — uncompressed 16-bit PCM.
pub const DEFAULT_AUDIO: Audio = Audio::Pcm16;

/// The forced pixel format. 10-bit 4:2:2 is what makes a file ProRes **422**;
/// `prores_ks` only accepts `yuv422p10le`, `yuv444p10le` or `yuva444p10le`, and
/// the latter two are the 4444 tier this tool deliberately does not cover.
pub const PIX_FMT: &str = "yuv422p10le";

/// The QuickTime vendor atom written into the output. ffmpeg defaults to
/// `Lavc`; `apl0` is the Apple identifier, and some editors check it before
/// accepting a file as genuine ProRes.
pub const VENDOR: &str = "apl0";

/// Build the downscale-only filter for a target height cap.
///
/// `scale=-2:'min(ih,720)'` caps the height at 720 while leaving anything
/// SHORTER untouched (`min` never grows the picture — asking for 1080p from a
/// 720p source is a no-op, not a blurry upscale), and `-2` derives the width
/// from the source aspect ratio rounded to an even number, which ProRes's
/// 4:2:2 chroma subsampling requires. The single quotes are parsed by ffmpeg's
/// own filtergraph parser (they protect the comma inside `min(...)`), so this
/// string is passed as ONE argv element with no shell involved.
pub fn scale_filter(max_height: u32) -> String {
    format!("scale=-2:'min(ih,{max_height})'")
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that re-encodes `in_name` →
/// `out_name` as Apple ProRes 422: optional downscale, `prores_ks` at `profile`,
/// forced `yuv422p10le`, the Apple vendor atom, and the chosen audio handling.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    profile: Profile,
    max_height: Option<u32>,
    audio: Audio,
) -> Vec<String> {
    let mut argv: Vec<String> = vec!["-i".into(), in_name.into()];
    if let Some(h) = max_height {
        argv.push("-vf".into());
        argv.push(scale_filter(h));
    }
    argv.extend([
        "-c:v".into(),
        "prores_ks".into(),
        "-profile:v".into(),
        profile.as_arg().into(),
        "-pix_fmt".into(),
        PIX_FMT.into(),
        "-vendor".into(),
        VENDOR.into(),
    ]);
    match audio.codec() {
        // `-c:a pcm_*` is a harmless no-op when the input has no audio track.
        Some(codec) => argv.extend(["-c:a".into(), codec.to_string()]),
        None => argv.push("-an".into()),
    }
    argv.push(out_name.into());
    argv
}

/// Validate + parse the user-facing strings and build `(argv, out_name)` for an
/// input file. `out_name` is always `out.mov` — ProRes only ships in QuickTime.
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`).
pub fn plan(
    profile: &str,
    resolution: &str,
    audio: &str,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    let p = parse_profile(profile)?;
    let max_height = parse_resolution(resolution)?;
    let a = parse_audio(audio)?;
    let out_name = "out.mov".to_string();
    Ok((build_argv(in_name, &out_name, p, max_height, a), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_pair(argv: &[String], k: &str, v: &str) -> bool {
        argv.windows(2).any(|w| w[0] == k && w[1] == v)
    }

    #[test]
    fn parse_profile_known() {
        assert_eq!(parse_profile("proxy").unwrap(), Profile::Proxy);
        assert_eq!(parse_profile("lt").unwrap(), Profile::Lt);
        assert_eq!(parse_profile("standard").unwrap(), Profile::Standard);
        assert_eq!(parse_profile("hq").unwrap(), Profile::Hq);
    }

    #[test]
    fn parse_profile_rejects_unknown() {
        // 4444/4444xq are real prores_ks profiles but a DIFFERENT tier (4:4:4 +
        // alpha) that this tool deliberately does not cover — they must be
        // rejected, not silently encoded as 4:2:2.
        assert!(parse_profile("4444").is_err());
        assert!(parse_profile("4444xq").is_err());
        assert!(parse_profile("422").is_err());
        assert!(parse_profile("").is_err());
        assert!(parse_profile("2").is_err());
    }

    #[test]
    fn parse_resolution_known_and_unknown() {
        assert_eq!(parse_resolution("source").unwrap(), None);
        assert_eq!(parse_resolution("2160p").unwrap(), Some(2160));
        assert_eq!(parse_resolution("1080p").unwrap(), Some(1080));
        assert_eq!(parse_resolution("480p").unwrap(), Some(480));
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
        assert_eq!(parse_audio("none").unwrap(), Audio::None);
        assert!(parse_audio("aac").is_err());
        assert!(parse_audio("copy").is_err());
    }

    #[test]
    fn argv_always_forces_prores422_10bit_and_apple_vendor() {
        let (argv, out) = plan("standard", "source", "pcm16", "in.mp4").unwrap();
        assert_eq!(out, "out.mov");
        // The forced-intermediate invariants — every one must always be present.
        assert!(has_pair(&argv, "-c:v", "prores_ks"));
        assert!(has_pair(&argv, "-profile:v", "standard"));
        assert!(has_pair(&argv, "-pix_fmt", "yuv422p10le"));
        assert!(has_pair(&argv, "-vendor", "apl0"));
        assert!(has_pair(&argv, "-c:a", "pcm_s16le"));
        assert_eq!(argv.first().map(String::as_str), Some("-i"));
        assert_eq!(argv[1], "in.mp4");
        assert_eq!(argv.last().map(String::as_str), Some("out.mov"));
        // Delivery-codec flags have no place in an intermediate.
        assert!(!argv.iter().any(|a| a == "-crf" || a == "libx264"));
        // No scale filter at all when resolution = source.
        assert!(!argv.iter().any(|a| a == "-vf"));
    }

    #[test]
    fn every_profile_reaches_argv_and_has_a_distinct_fourcc() {
        for name in ["proxy", "lt", "standard", "hq"] {
            let (argv, _) = plan(name, "source", "pcm16", "in.mov").unwrap();
            assert!(has_pair(&argv, "-profile:v", name), "{name} not in argv");
            // Every tier stays 4:2:2 10-bit — that is what "422" means.
            assert!(has_pair(&argv, "-pix_fmt", "yuv422p10le"));
        }
        let tags: Vec<&str> = [Profile::Proxy, Profile::Lt, Profile::Standard, Profile::Hq]
            .iter()
            .map(|p| p.fourcc())
            .collect();
        assert_eq!(tags, ["apco", "apcs", "apcn", "apch"]);
    }

    #[test]
    fn resolution_adds_a_downscale_only_filter() {
        let (argv, _) = plan("hq", "720p", "pcm16", "in.mkv").unwrap();
        assert!(has_pair(&argv, "-vf", "scale=-2:'min(ih,720)'"));
        // -2 keeps the width even (ProRes 4:2:2 needs it); min() never upscales.
        assert_eq!(scale_filter(1080), "scale=-2:'min(ih,1080)'");
    }

    #[test]
    fn audio_none_drops_the_track_instead_of_encoding_it() {
        let (argv, _) = plan("standard", "source", "none", "in.mp4").unwrap();
        assert!(argv.iter().any(|a| a == "-an"));
        assert!(!argv.iter().any(|a| a == "-c:a"));
    }

    #[test]
    fn audio_pcm24_selects_the_24bit_encoder() {
        let (argv, _) = plan("standard", "source", "pcm24", "in.mp4").unwrap();
        assert!(has_pair(&argv, "-c:a", "pcm_s24le"));
        assert!(!argv.iter().any(|a| a == "-an"));
    }

    #[test]
    fn plan_rejects_bad_params_with_a_message_naming_the_choices() {
        let e = plan("ultra", "source", "pcm16", "in.mp4").unwrap_err();
        assert!(e.contains("proxy|lt|standard|hq"), "unhelpful error: {e}");
        let e = plan("standard", "8k", "pcm16", "in.mp4").unwrap_err();
        assert!(e.contains("1080p"), "unhelpful error: {e}");
        let e = plan("standard", "source", "mp3", "in.mp4").unwrap_err();
        assert!(e.contains("pcm16|pcm24|none"), "unhelpful error: {e}");
    }

    #[test]
    fn input_name_is_preserved_verbatim() {
        let (argv, _) = plan("proxy", "540p", "none", "in.webm").unwrap();
        assert_eq!(argv[1], "in.webm");
    }
}
