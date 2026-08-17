//! gizza-ai/mp4-to-flv core — pure ffmpeg argv construction shared by the chat
//! block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! mp4-to-flv **re-encodes** a video into an FLV (Flash Video) stream: the
//! container legacy RTMP ingest endpoints, Flash Media Server / Wowza / Red5
//! deployments, and old Flash-based players still expect. FLV is a thin
//! packet container — a 9-byte header followed by tag packets — which is
//! exactly why it survives as the on-the-wire framing for RTMP.
//!
//! Why this is a re-encode and not a remux: FLV accepts only a narrow codec
//! set. Video must be H.264 or one of the legacy Flash codecs (Sorenson
//! Spark/H.263, VP6, Screen Video); audio must be AAC, MP3, Nellymoser,
//! ADPCM, or PCM. An MP4 whose video is HEVC/AV1/VP9, or whose audio is Opus
//! or FLAC, cannot be stream-copied into FLV at all, so this tool always
//! encodes rather than pretending a copy will work. (The lossless
//! FLV→MP4 direction *is* a plain `-c copy` remux and is covered by
//! `mkv-to-mp4`'s copy mode.)
//!
//! Container rules this module encodes so ffmpeg never fails mid-run:
//! - **libx264 refuses odd dimensions**, so a scale filter that rounds both
//!   axes down to an even number is ALWAYS applied (see [`scale_filter`]).
//! - **FLV only allows MP3 at 44100/22050/11025 Hz**, so `audio_codec = "mp3"`
//!   pins `-ar 44100`. AAC carries its own sample rate and is left alone.
//! - FLV holds **one video and one audio stream**, so exactly one of each is
//!   mapped (`-map 0:v:0`, `-map 0:a:0?` — the `?` keeps silent sources working).
//!
//! Deliberately distinct from its neighbours: `video-transcode` targets the
//! modern delivery pair (mp4/webm) with a CRF knob, `video-to-h264` normalizes
//! codecs inside MP4, and `mkv-to-mp4` remuxes without re-encoding. None of
//! them can write FLV.

/// The video codec written into the FLV stream.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum VideoCodec {
    /// `h264` — libx264. The default and what every modern RTMP ingest
    /// endpoint expects; Flash Player has supported H.264-in-FLV since 9.0.115.
    H264,
    /// `flv1` — Sorenson Spark (H.263 variant), the original Flash video codec.
    /// Much weaker compression, but it is what pre-9 Flash players, Red5 demos,
    /// and some fixed-function set-top decoders can actually decode.
    Flv1,
}

impl VideoCodec {
    /// The exact ffmpeg encoder name passed to `-c:v`.
    pub fn encoder(self) -> &'static str {
        match self {
            VideoCodec::H264 => "libx264",
            VideoCodec::Flv1 => "flv",
        }
    }
}

/// Parse the user-facing `video_codec` string (the values the chat schema + page accept).
pub fn parse_video_codec(s: &str) -> Result<VideoCodec, String> {
    match s {
        "h264" => Ok(VideoCodec::H264),
        "flv1" => Ok(VideoCodec::Flv1),
        other => Err(format!(
            "video_codec {other:?} not supported (h264|flv1)"
        )),
    }
}

/// Default video codec — H.264, the RTMP-ingest default.
pub const DEFAULT_VIDEO_CODEC: VideoCodec = VideoCodec::H264;

/// Parse the user-facing `resolution` string into a target height cap in pixels.
/// `source` (the default) means "keep the input dimensions" and maps to `None`.
///
/// The values carry a `p` suffix on purpose: the ffmpeg page driver
/// numeric-coerces numeric-LOOKING field strings before calling `build_argv`,
/// so a bare `"720"` would arrive at the wasm export as a JS number and throw.
pub fn parse_resolution(s: &str) -> Result<Option<u32>, String> {
    match s {
        "source" => Ok(None),
        "1080p" => Ok(Some(1080)),
        "720p" => Ok(Some(720)),
        "576p" => Ok(Some(576)),
        "480p" => Ok(Some(480)),
        "360p" => Ok(Some(360)),
        "240p" => Ok(Some(240)),
        other => Err(format!(
            "resolution {other:?} not supported (source|1080p|720p|576p|480p|360p|240p)"
        )),
    }
}

/// Parse the user-facing `fps` string into an output frame rate.
/// `source` (the default) keeps the input's frame timing and maps to `None`.
///
/// Like `resolution`, the values carry an `fps` suffix so the page's numeric
/// coercion never turns them into JS numbers.
pub fn parse_fps(s: &str) -> Result<Option<&'static str>, String> {
    match s {
        "source" => Ok(None),
        "60fps" => Ok(Some("60")),
        "30fps" => Ok(Some("30")),
        "25fps" => Ok(Some("25")),
        "24fps" => Ok(Some("24")),
        "15fps" => Ok(Some("15")),
        other => Err(format!(
            "fps {other:?} not supported (source|60fps|30fps|25fps|24fps|15fps)"
        )),
    }
}

/// The audio codec written into the FLV stream.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AudioCodec {
    /// `aac` — the default. What modern RTMP ingest (and Flash 9.0.115+) expects.
    Aac,
    /// `mp3` — the classic Flash audio codec, for players that predate AAC-in-FLV.
    /// FLV restricts MP3 to 44100/22050/11025 Hz, so this pins `-ar 44100`.
    Mp3,
    /// `none` — drop audio entirely (`-an`): a video-only FLV.
    None,
}

impl AudioCodec {
    /// The ffmpeg encoder name, or `None` when audio is dropped (`-an`).
    pub fn encoder(self) -> Option<&'static str> {
        match self {
            AudioCodec::Aac => Some("aac"),
            AudioCodec::Mp3 => Some("libmp3lame"),
            AudioCodec::None => None,
        }
    }
}

/// Parse the user-facing `audio_codec` string (the values the chat schema + page accept).
pub fn parse_audio_codec(s: &str) -> Result<AudioCodec, String> {
    match s {
        "aac" => Ok(AudioCodec::Aac),
        "mp3" => Ok(AudioCodec::Mp3),
        "none" => Ok(AudioCodec::None),
        other => Err(format!(
            "audio_codec {other:?} not supported (aac|mp3|none)"
        )),
    }
}

/// Default audio codec — AAC.
pub const DEFAULT_AUDIO_CODEC: AudioCodec = AudioCodec::Aac;

/// Default video bitrate in kbps — a 720p-ish RTMP ingest rate.
pub const DEFAULT_VIDEO_BITRATE_KBPS: u32 = 2500;
/// Accepted video bitrate range in kbps.
pub const MIN_VIDEO_BITRATE_KBPS: u32 = 100;
pub const MAX_VIDEO_BITRATE_KBPS: u32 = 20000;

/// Default audio bitrate in kbps.
pub const DEFAULT_AUDIO_BITRATE_KBPS: u32 = 128;
/// Accepted audio bitrate range in kbps.
pub const MIN_AUDIO_BITRATE_KBPS: u32 = 32;
pub const MAX_AUDIO_BITRATE_KBPS: u32 = 320;

/// Default keyframe interval in seconds. Two seconds is the interval every
/// major RTMP ingest endpoint asks for (it bounds player join latency and
/// segment boundaries downstream).
pub const DEFAULT_KEYFRAME_SECONDS: u32 = 2;
/// Accepted keyframe interval range in seconds.
pub const MIN_KEYFRAME_SECONDS: u32 = 1;
pub const MAX_KEYFRAME_SECONDS: u32 = 10;

/// The forced pixel format. Both FLV video codecs are 8-bit 4:2:0 only, and a
/// 10-bit or 4:2:2 source would otherwise make libx264 pick an FLV-illegal
/// pixel format and the mux would fail.
pub const PIX_FMT: &str = "yuv420p";

/// The libx264 speed preset. Fixed at `veryfast`: this encoder runs inside a
/// wasm build in the browser, where a slower preset costs minutes of wall clock
/// for a quality difference that a bitrate-targeted legacy stream cannot show.
pub const X264_PRESET: &str = "veryfast";

/// The sample rate MP3 audio is resampled to. FLV only permits MP3 at 44100,
/// 22050, or 11025 Hz, and a 48 kHz source is the common case.
pub const MP3_SAMPLE_RATE: &str = "44100";

/// Build the scale filter.
///
/// A filter is ALWAYS emitted, even for `resolution = source`, because libx264
/// rejects odd width/height outright ("width not divisible by 2") — an odd-sized
/// source would fail at encoder-open time with no output at all.
///
/// - `Some(n)`: `scale=-2:'2*trunc(min(ih,n)/2)'` — cap the height at `n`,
///   rounded DOWN to an even number, and derive an even width from the source
///   aspect ratio. `min` never upscales a smaller source.
/// - `None`: `scale='2*trunc(iw/2)':'2*trunc(ih/2)'` — keep the source size,
///   rounded down to even on both axes.
///
/// The single quotes are parsed by ffmpeg's own filtergraph parser (they protect
/// the comma inside `min(...)`), so this string is ONE argv element, no shell.
pub fn scale_filter(max_height: Option<u32>) -> String {
    match max_height {
        Some(h) => format!("scale=-2:'2*trunc(min(ih,{h})/2)'"),
        None => "scale='2*trunc(iw/2)':'2*trunc(ih/2)'".to_string(),
    }
}

/// Build the `-force_key_frames` expression that places a keyframe every
/// `seconds` seconds. Expressed in TIME, not frames, so it holds whether the
/// output frame rate came from the source or from `fps`.
pub fn keyframe_expr(seconds: u32) -> String {
    format!("expr:gte(t,n_forced*{seconds})")
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that re-encodes `in_name` →
/// `out_name` as an FLV stream.
#[allow(clippy::too_many_arguments)]
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    video_codec: VideoCodec,
    max_height: Option<u32>,
    fps: Option<&str>,
    video_bitrate_kbps: u32,
    keyframe_seconds: u32,
    audio_codec: AudioCodec,
    audio_bitrate_kbps: u32,
) -> Vec<String> {
    let mut argv: Vec<String> = vec!["-i".into(), in_name.into()];
    argv.push("-vf".into());
    argv.push(scale_filter(max_height));
    if let Some(rate) = fps {
        argv.push("-r".into());
        argv.push(rate.into());
    }
    // FLV carries at most one video + one audio stream. `?` on the audio map
    // keeps a silent source from failing the whole run.
    argv.push("-map".into());
    argv.push("0:v:0".into());
    if audio_codec != AudioCodec::None {
        argv.push("-map".into());
        argv.push("0:a:0?".into());
    }
    argv.push("-c:v".into());
    argv.push(video_codec.encoder().into());
    if video_codec == VideoCodec::H264 {
        argv.push("-preset".into());
        argv.push(X264_PRESET.into());
    }
    argv.extend(["-pix_fmt".into(), PIX_FMT.to_string()]);
    // Bitrate-targeted, with a matching cap + 2-second buffer: RTMP ingest is
    // bandwidth-bound, so a constrained rate matters more than a CRF target.
    argv.extend([
        "-b:v".into(),
        format!("{video_bitrate_kbps}k"),
        "-maxrate".into(),
        format!("{video_bitrate_kbps}k"),
        "-bufsize".into(),
        format!("{}k", video_bitrate_kbps * 2),
        "-force_key_frames".into(),
        keyframe_expr(keyframe_seconds),
    ]);
    match audio_codec.encoder() {
        Some(enc) => {
            argv.extend(["-c:a".into(), enc.to_string()]);
            argv.extend(["-b:a".into(), format!("{audio_bitrate_kbps}k")]);
            if audio_codec == AudioCodec::Mp3 {
                argv.extend(["-ar".into(), MP3_SAMPLE_RATE.to_string()]);
            }
        }
        None => argv.push("-an".into()),
    }
    // Name the muxer explicitly rather than relying on the output extension.
    argv.extend(["-f".into(), "flv".to_string()]);
    argv.push(out_name.into());
    argv
}

/// Validate a numeric knob that arrives as an `f64` (the page coerces numeric
/// fields to JS numbers) and return it as a whole number in range.
fn whole_in_range(name: &str, value: f64, min: u32, max: u32, unit: &str) -> Result<u32, String> {
    if !value.is_finite() || value.fract() != 0.0 {
        return Err(format!(
            "{name} must be a whole number of {unit} between {min} and {max}, got {value}"
        ));
    }
    let v = value as i64;
    if v < min as i64 || v > max as i64 {
        return Err(format!(
            "{name} must be between {min} and {max} {unit}, got {v}"
        ));
    }
    Ok(v as u32)
}

/// Validate + parse the user-facing values and build `(argv, out_name)` for an
/// input file. `out_name` is always `out.flv`. Single source shared by the chat
/// block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
#[allow(clippy::too_many_arguments)]
pub fn plan(
    video_codec: &str,
    resolution: &str,
    fps: &str,
    video_bitrate_kbps: f64,
    keyframe_seconds: f64,
    audio_codec: &str,
    audio_bitrate_kbps: f64,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    let vc = parse_video_codec(video_codec)?;
    let max_height = parse_resolution(resolution)?;
    let rate = parse_fps(fps)?;
    let vb = whole_in_range(
        "video_bitrate",
        video_bitrate_kbps,
        MIN_VIDEO_BITRATE_KBPS,
        MAX_VIDEO_BITRATE_KBPS,
        "kbps",
    )?;
    let gop = whole_in_range(
        "keyframe_seconds",
        keyframe_seconds,
        MIN_KEYFRAME_SECONDS,
        MAX_KEYFRAME_SECONDS,
        "seconds",
    )?;
    let ac = parse_audio_codec(audio_codec)?;
    let ab = whole_in_range(
        "audio_bitrate",
        audio_bitrate_kbps,
        MIN_AUDIO_BITRATE_KBPS,
        MAX_AUDIO_BITRATE_KBPS,
        "kbps",
    )?;
    let out_name = "out.flv".to_string();
    Ok((
        build_argv(in_name, &out_name, vc, max_height, rate, vb, gop, ac, ab),
        out_name,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_pair(argv: &[String], k: &str, v: &str) -> bool {
        argv.windows(2).any(|w| w[0] == k && w[1] == v)
    }

    /// The defaults every surface uses, so tests read like a real invocation.
    fn default_plan(in_name: &str) -> (Vec<String>, String) {
        plan("h264", "source", "source", 2500.0, 2.0, "aac", 128.0, in_name).unwrap()
    }

    #[test]
    fn default_argv_writes_h264_aac_flv() {
        let (argv, out) = default_plan("in.mp4");
        assert_eq!(out, "out.flv");
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "in.mp4");
        assert!(has_pair(&argv, "-c:v", "libx264"));
        assert!(has_pair(&argv, "-preset", "veryfast"));
        assert!(has_pair(&argv, "-pix_fmt", "yuv420p"));
        assert!(has_pair(&argv, "-b:v", "2500k"));
        assert!(has_pair(&argv, "-maxrate", "2500k"));
        assert!(has_pair(&argv, "-bufsize", "5000k"));
        assert!(has_pair(&argv, "-c:a", "aac"));
        assert!(has_pair(&argv, "-b:a", "128k"));
        // FLV is the muxer, named explicitly, and it is the LAST thing before out.
        assert!(has_pair(&argv, "-f", "flv"));
        assert_eq!(argv.last().map(String::as_str), Some("out.flv"));
        // One video + one audio stream — FLV cannot hold more.
        assert!(has_pair(&argv, "-map", "0:v:0"));
        assert!(has_pair(&argv, "-map", "0:a:0?"));
        // AAC keeps the source sample rate: only MP3 is rate-restricted in FLV.
        assert!(!argv.iter().any(|a| a == "-ar"));
    }

    #[test]
    fn plan_rejects_unknown_enum_values_naming_the_choices() {
        let e = plan("vp6", "source", "source", 2500.0, 2.0, "aac", 128.0, "in.mp4").unwrap_err();
        assert!(e.contains("h264|flv1"), "unhelpful error: {e}");
        let e = plan("h264", "4k", "source", 2500.0, 2.0, "aac", 128.0, "in.mp4").unwrap_err();
        assert!(e.contains("1080p"), "unhelpful error: {e}");
        let e = plan("h264", "source", "29.97", 2500.0, 2.0, "aac", 128.0, "in.mp4").unwrap_err();
        assert!(e.contains("30fps"), "unhelpful error: {e}");
        let e = plan("h264", "source", "source", 2500.0, 2.0, "opus", 128.0, "in.mp4").unwrap_err();
        assert!(e.contains("aac|mp3|none"), "unhelpful error: {e}");
    }

    #[test]
    fn plan_rejects_out_of_range_numbers_with_the_bounds_in_the_message() {
        let e = plan("h264", "source", "source", 50.0, 2.0, "aac", 128.0, "in.mp4").unwrap_err();
        assert!(e.contains("video_bitrate") && e.contains("100"), "unhelpful: {e}");
        let e = plan("h264", "source", "source", 20001.0, 2.0, "aac", 128.0, "in.mp4").unwrap_err();
        assert!(e.contains("20000"), "unhelpful: {e}");
        let e = plan("h264", "source", "source", 2500.0, 0.0, "aac", 128.0, "in.mp4").unwrap_err();
        assert!(e.contains("keyframe_seconds"), "unhelpful: {e}");
        let e = plan("h264", "source", "source", 2500.0, 11.0, "aac", 128.0, "in.mp4").unwrap_err();
        assert!(e.contains("10"), "unhelpful: {e}");
        let e = plan("h264", "source", "source", 2500.0, 2.0, "aac", 16.0, "in.mp4").unwrap_err();
        assert!(e.contains("audio_bitrate") && e.contains("32"), "unhelpful: {e}");
        let e = plan("h264", "source", "source", 2500.0, 2.0, "aac", 321.0, "in.mp4").unwrap_err();
        assert!(e.contains("320"), "unhelpful: {e}");
        // Fractional values are rejected rather than silently truncated.
        let e = plan("h264", "source", "source", 2500.5, 2.0, "aac", 128.0, "in.mp4").unwrap_err();
        assert!(e.contains("whole number"), "unhelpful: {e}");
    }

    #[test]
    fn range_boundaries_are_inclusive() {
        for (vb, gop, ab) in [(100.0, 1.0, 32.0), (20000.0, 10.0, 320.0)] {
            let (argv, _) = plan("h264", "source", "source", vb, gop, "aac", ab, "in.mp4").unwrap();
            assert!(has_pair(&argv, "-b:v", &format!("{}k", vb as u32)));
            assert!(has_pair(&argv, "-b:a", &format!("{}k", ab as u32)));
            assert!(has_pair(&argv, "-force_key_frames", &keyframe_expr(gop as u32)));
        }
    }

    #[test]
    fn flv1_selects_sorenson_spark_and_drops_the_x264_preset() {
        let (argv, _) = plan("flv1", "source", "source", 800.0, 2.0, "mp3", 96.0, "in.mp4").unwrap();
        assert!(has_pair(&argv, "-c:v", "flv"));
        // `-preset` is a libx264 private option; the Sorenson encoder has none.
        assert!(!argv.iter().any(|a| a == "-preset"));
        assert!(has_pair(&argv, "-pix_fmt", "yuv420p"));
    }

    #[test]
    fn mp3_audio_pins_an_flv_legal_sample_rate() {
        let (argv, _) = plan("h264", "source", "source", 2500.0, 2.0, "mp3", 128.0, "in.mp4").unwrap();
        assert!(has_pair(&argv, "-c:a", "libmp3lame"));
        // FLV rejects MP3 at 48 kHz, the most common MP4 source rate.
        assert!(has_pair(&argv, "-ar", "44100"));
    }

    #[test]
    fn audio_none_drops_the_track_and_stops_mapping_it() {
        let (argv, _) = plan("h264", "source", "source", 2500.0, 2.0, "none", 128.0, "in.mp4").unwrap();
        assert!(argv.iter().any(|a| a == "-an"));
        assert!(!argv.iter().any(|a| a == "-c:a"));
        assert!(!argv.iter().any(|a| a == "-b:a"));
        assert!(!has_pair(&argv, "-map", "0:a:0?"));
        assert!(has_pair(&argv, "-map", "0:v:0"));
    }

    #[test]
    fn a_scale_filter_is_always_present_so_odd_sources_never_break_libx264() {
        // resolution = source still emits an even-rounding filter.
        let (argv, _) = default_plan("in.mp4");
        assert!(has_pair(
            &argv,
            "-vf",
            "scale='2*trunc(iw/2)':'2*trunc(ih/2)'"
        ));
        // A cap rounds the height down to even and derives an even width.
        let (argv, _) = plan("h264", "480p", "source", 900.0, 2.0, "aac", 96.0, "in.mov").unwrap();
        assert!(has_pair(&argv, "-vf", "scale=-2:'2*trunc(min(ih,480)/2)'"));
        assert_eq!(scale_filter(Some(720)), "scale=-2:'2*trunc(min(ih,720)/2)'");
    }

    #[test]
    fn every_resolution_and_fps_choice_reaches_the_argv() {
        for (name, h) in [
            ("1080p", 1080),
            ("720p", 720),
            ("576p", 576),
            ("480p", 480),
            ("360p", 360),
            ("240p", 240),
        ] {
            let (argv, _) = plan("h264", name, "source", 1200.0, 2.0, "aac", 128.0, "in.mp4").unwrap();
            assert!(has_pair(&argv, "-vf", &scale_filter(Some(h))), "{name}");
        }
        for (name, rate) in [
            ("60fps", "60"),
            ("30fps", "30"),
            ("25fps", "25"),
            ("24fps", "24"),
            ("15fps", "15"),
        ] {
            let (argv, _) = plan("h264", "source", name, 1200.0, 2.0, "aac", 128.0, "in.mp4").unwrap();
            assert!(has_pair(&argv, "-r", rate), "{name}");
        }
        // fps = source never pins an output rate.
        let (argv, _) = default_plan("in.mp4");
        assert!(!argv.iter().any(|a| a == "-r"));
    }

    #[test]
    fn keyframe_interval_is_expressed_in_time_not_frames() {
        let (argv, _) = plan("h264", "source", "60fps", 4000.0, 4.0, "aac", 128.0, "in.mp4").unwrap();
        // Time-based so it holds regardless of the output frame rate.
        assert!(has_pair(&argv, "-force_key_frames", "expr:gte(t,n_forced*4)"));
        assert_eq!(keyframe_expr(2), "expr:gte(t,n_forced*2)");
    }

    #[test]
    fn input_name_is_preserved_verbatim() {
        let (argv, out) = plan("flv1", "360p", "15fps", 500.0, 1.0, "none", 64.0, "in.webm").unwrap();
        assert_eq!(argv[1], "in.webm");
        // The output container never depends on the input extension.
        assert_eq!(out, "out.flv");
    }
}
