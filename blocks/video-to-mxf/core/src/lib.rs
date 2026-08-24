//! gizza-ai/video-to-mxf core — pure ffmpeg argv construction shared by the
//! chat block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! video-to-mxf is **container-first**: it wraps a video into a SMPTE **MXF**
//! file using one of the MPEG-2-family codecs that broadcast delivery specs
//! actually name, or rewraps an already-compliant picture stream untouched.
//!
//! The profiles map to the specs stations ask for by name:
//! - `xdcam_hd422` — MPEG-2 422P@HL, 8-bit `yuv422p`, 50 Mbps CBR, long-GOP,
//!   wrapped as MXF **OP1a**. The house standard for HD tape-replacement
//!   delivery, and the default here.
//! - `xdcam_hd` — MPEG-2 MP@HL, 8-bit `yuv420p`, 35 Mbps CBR, MXF OP1a. The
//!   lighter HD tier, still long-GOP.
//! - `imx50` — SMPTE **D-10** (a.k.a. MPEG IMX 50): intra-frame-only MPEG-2
//!   4:2:2 at 50 Mbps CBR in the SD 625/50 raster, written through ffmpeg's
//!   dedicated `mxf_d10` muxer rather than plain OP1a.
//! - `copy` — no picture re-encode at all: the source video stream is rewrapped
//!   into MXF as-is (audio is still converted to PCM, because MXF cannot carry
//!   AAC). Use it when the essence is already spec-compliant and only the
//!   wrapper is wrong.
//!
//! Two invariants drive the validation below, and both come from ffmpeg
//! refusing the combination rather than from taste:
//!
//! 1. **The MXF muxer only accepts broadcast frame rates.** Feed it 10 fps and
//!    it aborts with "Unsupported frame rate 10/1. Set -strict option to
//!    'unofficial'". So `frame_rate = source` (the default) adds
//!    `-strict unofficial`, which wraps odd rates at the cost of a file that is
//!    not spec-conformant; choosing an explicit broadcast rate conforms and
//!    drops the flag.
//! 2. **CBR rate control needs a real raster.** `-minrate = -maxrate` on a tiny
//!    frame makes the MPEG-2 encoder fail to open ("impossible bitrate
//!    constraints"), so CBR is applied only when the picture is conformed to a
//!    broadcast raster. `resolution = source` therefore encodes average-VBR at
//!    the profile's bitrate and is documented as best-effort, not conformant.
//!
//! Deliberately distinct from its neighbours:
//! - `video-to-dnxhd` is codec-first (five DNxHR tiers, MOV default, MXF as an
//!   option). No DNxHR profile is exposed here, and no MPEG-2 broadcast profile
//!   is exposed there.
//! - `video-to-prores` always writes Apple ProRes in a QuickTime `.mov`.
//! - `video-transcode` / `video-to-h264` target web delivery with a CRF knob.
//!
//! Not covered here: the 1440×1080 anamorphic XDCAM HD variant (it needs a
//! non-square SAR that would distort 4:3 sources), OP-Atom MXF (`mxf_opatom`
//! carries exactly one stream per file), AVC-Intra and XAVC (no ffmpeg encoder
//! that produces conformant class-tagged essence), and 525/60 IMX — see
//! `Profile::Imx50` for why ffmpeg cannot mux that one.

/// The broadcast codec profile written into the MXF wrapper.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Profile {
    /// `xdcam_hd422` — MPEG-2 422P@HL, `yuv422p`, 50 Mbps CBR, long-GOP, OP1a.
    XdcamHd422,
    /// `xdcam_hd` — MPEG-2 MP@HL, `yuv420p`, 35 Mbps CBR, long-GOP, OP1a.
    XdcamHd,
    /// `imx50` — SMPTE D-10 / MPEG IMX 50: intra-only MPEG-2 4:2:2, 50 Mbps
    /// CBR, SD 720×576, muxed with `mxf_d10`.
    ///
    /// **25 fps only.** ffmpeg derives a fixed D-10 frame size from
    /// `bit_rate × time_base`; at 30000/1001 that is 208541.67 bytes, which is
    /// not an integer, so every packet is rejected with "Error submitting a
    /// packet to the muxer: Operation not permitted" — for every 525/60 raster
    /// (720×486, 720×512, 720×608 all fail). 25 fps divides exactly (250000
    /// bytes/frame) and works, so only 625/50 IMX is offered.
    Imx50,
    /// `copy` — rewrap the source video stream without re-encoding it.
    Copy,
}

impl Profile {
    /// The ffmpeg muxer name. D-10 has its own mapping; everything else is OP1a.
    pub fn muxer(self) -> &'static str {
        match self {
            Profile::Imx50 => "mxf_d10",
            _ => "mxf",
        }
    }

    /// Target video bitrate in bits per second, or `None` when nothing is encoded.
    pub fn bitrate(self) -> Option<&'static str> {
        match self {
            Profile::XdcamHd422 | Profile::Imx50 => Some("50M"),
            Profile::XdcamHd => Some("35M"),
            Profile::Copy => None,
        }
    }

    /// The pixel format the profile pins, or `None` for the rewrap path.
    pub fn pix_fmt(self) -> Option<&'static str> {
        match self {
            Profile::XdcamHd422 | Profile::Imx50 => Some("yuv422p"),
            Profile::XdcamHd => Some("yuv420p"),
            Profile::Copy => None,
        }
    }
}

/// Parse the user-facing profile string (the values the chat schema + page accept).
pub fn parse_profile(s: &str) -> Result<Profile, String> {
    match s {
        "xdcam_hd422" => Ok(Profile::XdcamHd422),
        "xdcam_hd" => Ok(Profile::XdcamHd),
        "imx50" => Ok(Profile::Imx50),
        "copy" => Ok(Profile::Copy),
        other => Err(format!(
            "profile {other:?} not supported (xdcam_hd422|xdcam_hd|imx50|copy)"
        )),
    }
}

/// Default profile — the 50 Mbps 4:2:2 HD house standard.
pub const DEFAULT_PROFILE: Profile = Profile::XdcamHd422;

/// How the picture is conformed to a delivery raster.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Resolution {
    /// `auto` — use the profile's own delivery raster (1920×1080 for the XDCAM
    /// HD profiles, 720×576 for IMX 50, source size for `copy`).
    Auto,
    /// `source` — leave the picture size alone. Best-effort, not conformant,
    /// and encoded average-VBR instead of CBR.
    Source,
    /// `1920x1080` — square-pixel Full HD.
    Hd1080,
    /// `1280x720` — square-pixel 720p.
    Hd720,
}

/// Parse the user-facing resolution string.
pub fn parse_resolution(s: &str) -> Result<Resolution, String> {
    match s {
        "auto" => Ok(Resolution::Auto),
        "source" => Ok(Resolution::Source),
        "1920x1080" => Ok(Resolution::Hd1080),
        "1280x720" => Ok(Resolution::Hd720),
        other => Err(format!(
            "resolution {other:?} not supported (auto|source|1920x1080|1280x720)"
        )),
    }
}

/// Default resolution handling — conform to the profile's delivery raster.
pub const DEFAULT_RESOLUTION: Resolution = Resolution::Auto;

/// The output frame rate. `Source` keeps whatever the input has.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FrameRate {
    /// `source` — keep the input rate. Adds `-strict unofficial` so the MXF
    /// muxer accepts rates outside the broadcast set.
    Source,
    /// `23.976` — 24000/1001, film in an NTSC-derived world.
    F23976,
    /// `24` — true 24, film.
    F24,
    /// `25` — 625/50 (PAL) territories.
    F25,
    /// `29.97` — 30000/1001, 525/60 (NTSC) territories.
    F2997,
    /// `30` — true 30.
    F30,
    /// `50` — 625/50 high frame rate.
    F50,
    /// `59.94` — 60000/1001, 525/60 high frame rate.
    F5994,
    /// `60` — true 60.
    F60,
}

impl FrameRate {
    /// The exact value passed to `-r`. Fractional rates use their true
    /// `N/1001` form rather than a rounded decimal, so the muxer's edit-rate
    /// check recognises them.
    pub fn as_arg(self) -> Option<&'static str> {
        match self {
            FrameRate::Source => None,
            FrameRate::F23976 => Some("24000/1001"),
            FrameRate::F24 => Some("24"),
            FrameRate::F25 => Some("25"),
            FrameRate::F2997 => Some("30000/1001"),
            FrameRate::F30 => Some("30"),
            FrameRate::F50 => Some("50"),
            FrameRate::F5994 => Some("60000/1001"),
            FrameRate::F60 => Some("60"),
        }
    }

    /// Long-GOP length for this rate: 12 frames in the 625/50 family, 15 in the
    /// 525/60 family — the GOP sizes the XDCAM specs name. An unknown (source)
    /// rate is not conformant anyway, so it takes the 15-frame default.
    pub fn gop(self) -> u32 {
        match self {
            FrameRate::F25 | FrameRate::F50 => 12,
            _ => 15,
        }
    }
}

/// Parse the user-facing frame-rate string.
pub fn parse_frame_rate(s: &str) -> Result<FrameRate, String> {
    match s {
        "source" => Ok(FrameRate::Source),
        "23.976" => Ok(FrameRate::F23976),
        "24" => Ok(FrameRate::F24),
        "25" => Ok(FrameRate::F25),
        "29.97" => Ok(FrameRate::F2997),
        "30" => Ok(FrameRate::F30),
        "50" => Ok(FrameRate::F50),
        "59.94" => Ok(FrameRate::F5994),
        "60" => Ok(FrameRate::F60),
        other => Err(format!(
            "frame_rate {other:?} not supported (source|23.976|24|25|29.97|30|50|59.94|60)"
        )),
    }
}

/// Default frame-rate handling — keep the source rate.
pub const DEFAULT_FRAME_RATE: FrameRate = FrameRate::Source;

/// What to do with the input's audio. MXF cannot carry AAC, so there is no
/// stream-copy option: audio is always PCM or dropped.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Audio {
    /// `pcm16` — 16-bit little-endian PCM at 48 kHz. The broadcast default.
    Pcm16,
    /// `pcm24` — 24-bit PCM at 48 kHz, for 24-bit masters.
    Pcm24,
    /// `none` — drop audio entirely (`-an`).
    None,
}

impl Audio {
    /// The ffmpeg encoder name, or `None` when audio is dropped.
    pub fn codec(self) -> Option<&'static str> {
        match self {
            Audio::Pcm16 => Some("pcm_s16le"),
            Audio::Pcm24 => Some("pcm_s24le"),
            Audio::None => None,
        }
    }
}

/// Parse the user-facing audio string.
pub fn parse_audio(s: &str) -> Result<Audio, String> {
    match s {
        "pcm16" => Ok(Audio::Pcm16),
        "pcm24" => Ok(Audio::Pcm24),
        "none" => Ok(Audio::None),
        other => Err(format!("audio {other:?} not supported (pcm16|pcm24|none)")),
    }
}

/// Default audio handling — 48 kHz 16-bit PCM.
pub const DEFAULT_AUDIO: Audio = Audio::Pcm16;

/// The sample rate every MXF delivery spec here mandates.
pub const AUDIO_SAMPLE_RATE: &str = "48000";

/// VBV buffer for the long-GOP XDCAM profiles, in bits (the value the XDCAM
/// specs use: 2229248 bytes).
pub const XDCAM_BUFSIZE: &str = "17825792";

/// VBV buffer for D-10, in bits. D-10 is intra-only with a one-frame buffer.
pub const D10_BUFSIZE: &str = "2000000";

/// MIME type of the output. RFC 4539 registers `application/mxf`.
pub const OUTPUT_MIME: &str = "application/mxf";

/// The single output filename — MXF is the only container this tool writes.
pub const OUTPUT_NAME: &str = "out.mxf";

/// Conform the picture to `width`×`height` without distorting it: scale to fit
/// inside the raster, then pad the remainder with black and reset the sample
/// aspect ratio to square.
///
/// `force_original_aspect_ratio=decrease` never crops and never stretches, so a
/// 4:3 source delivered at 1920×1080 is pillarboxed and a 2.39:1 source is
/// letterboxed — which is what a delivery spec expects when it names one raster.
/// The commas are inside a single argv element parsed by ffmpeg's own
/// filtergraph parser; no shell is involved.
pub fn fit_filter(width: u32, height: u32) -> String {
    format!(
        "scale={width}:{height}:force_original_aspect_ratio=decrease,\
         pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1"
    )
}

/// Resolve `(profile, resolution)` to the raster the picture is conformed to,
/// or `None` when the picture is passed through at its source size.
pub fn target_raster(profile: Profile, resolution: Resolution) -> Option<(u32, u32)> {
    match resolution {
        Resolution::Source => None,
        Resolution::Hd1080 => Some((1920, 1080)),
        Resolution::Hd720 => Some((1280, 720)),
        Resolution::Auto => match profile {
            Profile::XdcamHd422 | Profile::XdcamHd => Some((1920, 1080)),
            // SMPTE D-10 625/50 active raster.
            Profile::Imx50 => Some((720, 576)),
            // Nothing can be rescaled without re-encoding.
            Profile::Copy => None,
        },
    }
}

/// Reject the combinations ffmpeg cannot honour, with a message that names the
/// working alternative instead of leaving the user to read an encoder abort.
fn validate(profile: Profile, resolution: Resolution, frame_rate: FrameRate) -> Result<(), String> {
    match profile {
        Profile::Copy => {
            if !matches!(resolution, Resolution::Auto | Resolution::Source) {
                return Err(
                    "profile \"copy\" rewraps the video stream untouched and cannot rescale it; \
                     use resolution=auto (or source), or pick an encoding profile such as xdcam_hd422"
                        .into(),
                );
            }
            if frame_rate != FrameRate::Source {
                return Err(
                    "profile \"copy\" rewraps the video stream untouched and cannot change its \
                     frame rate; use frame_rate=source, or pick an encoding profile such as xdcam_hd422"
                        .into(),
                );
            }
        }
        Profile::Imx50 => {
            if resolution != Resolution::Auto {
                return Err(
                    "profile \"imx50\" is locked to the SMPTE D-10 625/50 raster (720x576); \
                     use resolution=auto, or pick xdcam_hd422/xdcam_hd for HD rasters"
                        .into(),
                );
            }
            if frame_rate != FrameRate::F25 {
                return Err(
                    "profile \"imx50\" requires frame_rate=25: ffmpeg's D-10 muxer needs a whole \
                     number of bytes per frame, which 50 Mbps only yields at 25 fps (525/60 IMX \
                     is rejected at every raster). Use frame_rate=25, or pick xdcam_hd422/xdcam_hd"
                        .into(),
                );
            }
        }
        Profile::XdcamHd422 | Profile::XdcamHd => {}
    }
    Ok(())
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that writes `in_name` → `out.mxf`.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    profile: Profile,
    resolution: Resolution,
    frame_rate: FrameRate,
    audio: Audio,
) -> Vec<String> {
    let mut argv: Vec<String> = vec!["-i".into(), in_name.into()];

    if let Some((w, h)) = target_raster(profile, resolution) {
        argv.push("-vf".into());
        argv.push(fit_filter(w, h));
    }
    if let Some(r) = frame_rate.as_arg() {
        argv.push("-r".into());
        argv.push(r.into());
    }

    match profile {
        Profile::Copy => argv.extend(["-c:v".into(), "copy".into()]),
        _ => {
            let bitrate = profile.bitrate().expect("encoding profile has a bitrate");
            argv.extend([
                "-c:v".into(),
                "mpeg2video".into(),
                "-pix_fmt".into(),
                profile.pix_fmt().expect("encoding profile pins a pix_fmt").into(),
                "-b:v".into(),
                bitrate.into(),
            ]);
            // CBR needs a real raster to stuff bits into: with -minrate =
            // -maxrate on a tiny source frame the encoder refuses to open, so
            // the source-size path stays average-VBR (and non-conformant).
            let cbr = target_raster(profile, resolution).is_some();
            if cbr {
                argv.extend([
                    "-minrate".into(),
                    bitrate.into(),
                    "-maxrate".into(),
                    bitrate.into(),
                ]);
            }
            if profile == Profile::Imx50 {
                // The SMPTE D-10 encoder settings: interlaced DCT, low delay,
                // 10-bit intra DC precision, non-linear quantiser, intra VLC,
                // a narrow QP window and top-field-first, all intra (-g 1).
                argv.extend([
                    "-flags".into(),
                    "+ildct+low_delay".into(),
                    "-dc".into(),
                    "10".into(),
                    "-non_linear_quant".into(),
                    "1".into(),
                    "-intra_vlc".into(),
                    "1".into(),
                    "-qmin".into(),
                    "1".into(),
                    "-qmax".into(),
                    "3".into(),
                    "-g".into(),
                    "1".into(),
                    "-top".into(),
                    "1".into(),
                    "-bufsize".into(),
                    D10_BUFSIZE.into(),
                    "-rc_init_occupancy".into(),
                    D10_BUFSIZE.into(),
                ]);
            } else {
                if cbr {
                    argv.extend(["-bufsize".into(), XDCAM_BUFSIZE.into()]);
                }
                argv.extend([
                    "-g".into(),
                    frame_rate.gop().to_string(),
                    "-bf".into(),
                    "2".into(),
                ]);
            }
        }
    }

    match audio.codec() {
        // `-c:a pcm_*` is a harmless no-op when the input has no audio track.
        Some(codec) => argv.extend([
            "-c:a".into(),
            codec.to_string(),
            "-ar".into(),
            AUDIO_SAMPLE_RATE.into(),
        ]),
        None => argv.push("-an".into()),
    }

    // A source rate may be outside the MXF edit-rate set; without this the
    // muxer aborts with "Unsupported frame rate ...".
    if frame_rate == FrameRate::Source {
        argv.extend(["-strict".into(), "unofficial".into()]);
    }

    argv.extend(["-f".into(), profile.muxer().into(), out_name.into()]);
    argv
}

/// Validate + parse the user-facing strings and build `(argv, out_name)`.
pub fn plan(
    profile: &str,
    resolution: &str,
    frame_rate: &str,
    audio: &str,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    let p = parse_profile(profile)?;
    let r = parse_resolution(resolution)?;
    let f = parse_frame_rate(frame_rate)?;
    let a = parse_audio(audio)?;
    validate(p, r, f)?;
    let out_name = OUTPUT_NAME.to_string();
    Ok((build_argv(in_name, &out_name, p, r, f, a), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(argv: &[String]) -> String {
        argv.join(" ")
    }

    #[test]
    fn default_plan_is_xdcam_hd422_cbr_at_1920x1080() {
        let (argv, out) = plan("xdcam_hd422", "auto", "25", "pcm16", "in.mp4").unwrap();
        assert_eq!(out, "out.mxf");
        let s = joined(&argv);
        assert!(s.contains("-c:v mpeg2video"), "{s}");
        assert!(s.contains("-pix_fmt yuv422p"), "{s}");
        assert!(s.contains("-b:v 50M -minrate 50M -maxrate 50M"), "{s}");
        assert!(s.contains("-bufsize 17825792"), "{s}");
        assert!(s.contains("scale=1920:1080"), "{s}");
        assert!(s.contains("-r 25"), "{s}");
        // 25 fps is the 625/50 family: 12-frame GOP.
        assert!(s.contains("-g 12 -bf 2"), "{s}");
        assert!(s.contains("-c:a pcm_s16le -ar 48000"), "{s}");
        assert!(!s.contains("-strict"), "explicit rate must not relax the muxer: {s}");
        assert_eq!(argv.last().map(String::as_str), Some("out.mxf"));
        assert_eq!(argv[argv.len() - 3..argv.len() - 1], ["-f", "mxf"]);
    }

    #[test]
    fn exact_argv_for_the_documented_default_run() {
        let (argv, _) = plan("xdcam_hd422", "auto", "source", "pcm16", "in.mp4").unwrap();
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp4",
                "-vf",
                "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1",
                "-c:v",
                "mpeg2video",
                "-pix_fmt",
                "yuv422p",
                "-b:v",
                "50M",
                "-minrate",
                "50M",
                "-maxrate",
                "50M",
                "-bufsize",
                "17825792",
                "-g",
                "15",
                "-bf",
                "2",
                "-c:a",
                "pcm_s16le",
                "-ar",
                "48000",
                "-strict",
                "unofficial",
                "-f",
                "mxf",
                "out.mxf",
            ]
        );
    }

    #[test]
    fn source_resolution_drops_cbr_and_the_scale_filter() {
        let (argv, _) = plan("xdcam_hd422", "source", "25", "pcm24", "in.mov").unwrap();
        let s = joined(&argv);
        assert!(!s.contains("-vf"), "source size must not be rescaled: {s}");
        assert!(s.contains("-b:v 50M"), "{s}");
        assert!(!s.contains("-minrate"), "CBR needs a real raster: {s}");
        assert!(!s.contains("-bufsize"), "{s}");
        assert!(s.contains("-c:a pcm_s24le -ar 48000"), "{s}");
    }

    #[test]
    fn xdcam_hd_is_35_mbps_4_2_0_and_ntsc_rates_take_a_15_frame_gop() {
        let (argv, _) = plan("xdcam_hd", "1280x720", "29.97", "pcm16", "in.mp4").unwrap();
        let s = joined(&argv);
        assert!(s.contains("-pix_fmt yuv420p"), "{s}");
        assert!(s.contains("-b:v 35M -minrate 35M -maxrate 35M"), "{s}");
        assert!(s.contains("scale=1280:720"), "{s}");
        assert!(s.contains("-r 30000/1001"), "{s}");
        assert!(s.contains("-g 15"), "{s}");
    }

    #[test]
    fn imx50_uses_the_d10_muxer_and_the_sd_raster() {
        let (argv, out) = plan("imx50", "auto", "25", "pcm16", "in.mp4").unwrap();
        assert_eq!(out, "out.mxf");
        let s = joined(&argv);
        assert!(s.contains("scale=720:576"), "{s}");
        assert!(s.contains("-f mxf_d10"), "{s}");
        assert!(s.contains("-flags +ildct+low_delay"), "{s}");
        assert!(s.contains("-g 1"), "intra-only: {s}");
        assert!(s.contains("-bufsize 2000000 -rc_init_occupancy 2000000"), "{s}");
        assert!(!s.contains("-bf 2"), "D-10 has no B-frames: {s}");
    }

    #[test]
    fn copy_rewraps_without_touching_the_picture_but_still_writes_pcm() {
        let (argv, _) = plan("copy", "auto", "source", "pcm16", "in.mov").unwrap();
        let s = joined(&argv);
        assert!(s.contains("-c:v copy"), "{s}");
        assert!(!s.contains("-vf"), "{s}");
        assert!(!s.contains("mpeg2video"), "{s}");
        assert!(s.contains("-c:a pcm_s16le -ar 48000"), "MXF cannot carry AAC: {s}");
        assert!(s.contains("-strict unofficial"), "{s}");
    }

    #[test]
    fn audio_none_drops_the_track() {
        let (argv, _) = plan("xdcam_hd422", "auto", "25", "none", "in.mp4").unwrap();
        let s = joined(&argv);
        assert!(s.contains("-an"), "{s}");
        assert!(!s.contains("-c:a"), "{s}");
    }

    #[test]
    fn fifty_fps_keeps_the_twelve_frame_pal_gop() {
        assert_eq!(FrameRate::F50.gop(), 12);
        assert_eq!(FrameRate::F5994.gop(), 15);
        assert_eq!(FrameRate::Source.gop(), 15);
    }

    #[test]
    fn fit_filter_never_crops_or_stretches() {
        let f = fit_filter(1920, 1080);
        assert!(f.contains("force_original_aspect_ratio=decrease"));
        assert!(f.contains("pad=1920:1080"));
        assert!(f.ends_with("setsar=1"));
    }

    #[test]
    fn unknown_values_are_rejected_with_the_accepted_list() {
        let e = plan("dnxhr_sq", "auto", "25", "pcm16", "in.mp4").unwrap_err();
        assert!(e.contains("xdcam_hd422|xdcam_hd|imx50|copy"), "unhelpful: {e}");
        let e = plan("xdcam_hd422", "4k", "25", "pcm16", "in.mp4").unwrap_err();
        assert!(e.contains("auto|source|1920x1080|1280x720"), "unhelpful: {e}");
        let e = plan("xdcam_hd422", "auto", "23.98", "pcm16", "in.mp4").unwrap_err();
        assert!(e.contains("23.976"), "unhelpful: {e}");
        let e = plan("xdcam_hd422", "auto", "25", "aac", "in.mp4").unwrap_err();
        assert!(e.contains("pcm16|pcm24|none"), "unhelpful: {e}");
    }

    #[test]
    fn imx50_rejects_hd_rasters_and_non_pal_rates() {
        let e = plan("imx50", "1920x1080", "25", "pcm16", "in.mp4").unwrap_err();
        assert!(e.contains("720x576"), "unhelpful: {e}");
        let e = plan("imx50", "auto", "29.97", "pcm16", "in.mp4").unwrap_err();
        assert!(e.contains("frame_rate=25"), "unhelpful: {e}");
        let e = plan("imx50", "auto", "source", "pcm16", "in.mp4").unwrap_err();
        assert!(e.contains("frame_rate=25"), "unhelpful: {e}");
    }

    #[test]
    fn copy_rejects_rescaling_and_retiming() {
        let e = plan("copy", "1920x1080", "source", "pcm16", "in.mov").unwrap_err();
        assert!(e.contains("cannot rescale"), "unhelpful: {e}");
        let e = plan("copy", "auto", "25", "pcm16", "in.mov").unwrap_err();
        assert!(e.contains("cannot change its frame rate"), "unhelpful: {e}");
    }
}
