//! gizza-ai/video-fragmented-mp4 core — pure ffmpeg argv construction shared by
//! the chat block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! A normal ("progressive") MP4 stores one big `moov` index describing every
//! sample in the file, followed by a single `mdat` holding all the media. A
//! *fragmented* MP4 (fMP4) instead writes a tiny initialization-only `moov`
//! header and then a chain of self-describing `moof` + `mdat` fragments. That is
//! the shape Media Source Extensions players, DASH/CMAF packagers, and
//! byte-range streaming want: a player can append the init header plus any
//! individual fragment and start decoding without an index for the whole file.
//!
//! The conversion is a pure container rewrite — the encoded video/audio packets
//! are stream-copied (`-c copy`) by default, so it is lossless and near-instant.
//! `mode = "h264"` re-encodes to H.264/AAC for sources whose codecs a browser
//! can't decode, and additionally forces a regular keyframe cadence so fragments
//! come out evenly spaced.
//!
//! Distinct from mov-to-mp4 / mkv-to-mp4 / video-duration-fix-remux, which all
//! produce PROGRESSIVE MP4s (`-movflags +faststart` just relocates the one big
//! `moov`); none of them fragments the file.

/// How the fragmented MP4 is produced: lossless container rewrite or re-encode.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    /// `-c copy`: stream-copy every track into the fragmented container.
    /// Lossless, no quality change, near-instant.
    Copy,
    /// `-c:v libx264 -c:a aac`: re-encode to H.264/AAC, and force a regular
    /// keyframe cadence so fragments are evenly spaced. Lossy + slower.
    H264,
}

/// Parse the user-facing mode string (the values the chat schema + page accept).
pub fn parse_mode(s: &str) -> Result<Mode, String> {
    match s {
        "copy" => Ok(Mode::Copy),
        "h264" => Ok(Mode::H264),
        other => Err(format!("mode {other:?} not supported (copy|h264)")),
    }
}

/// Which streaming ecosystem the fragmented output is tuned for. All three are
/// fragmented and MSE-appendable; `dash`/`cmaf` add the extra muxer flags those
/// specs ask for.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Profile {
    /// Plain fragmented MP4 for Media Source Extensions / generic streaming.
    Mse,
    /// DASH-compatible fragmented output (`+dash+negative_cts_offsets`).
    Dash,
    /// CMAF-compatible fragmented output (`+cmaf+negative_cts_offsets`).
    Cmaf,
}

/// Parse the user-facing profile string.
pub fn parse_profile(s: &str) -> Result<Profile, String> {
    match s {
        "mse" => Ok(Profile::Mse),
        "dash" => Ok(Profile::Dash),
        "cmaf" => Ok(Profile::Cmaf),
        other => Err(format!(
            "profile {other:?} not supported (mse|dash|cmaf)"
        )),
    }
}

/// Largest accepted `fragment_duration` / `keyframe_interval`, in seconds.
/// Beyond this a "fragment" stops being a streaming unit; the cap also keeps the
/// page slider usable.
pub const MAX_SECONDS: f64 = 30.0;

/// Default forced-keyframe cadence for `mode = "h264"`, in seconds. 2 s is the
/// cadence streaming packagers conventionally target.
pub const DEFAULT_KEYFRAME_INTERVAL: f64 = 2.0;

/// The three `-movflags` every fragmented MP4 here carries:
/// * `frag_keyframe` — start a new fragment at each video keyframe, so every
///   fragment begins at a seekable random-access point;
/// * `empty_moov` — write an initialization-only `moov` instead of a full index;
/// * `default_base_moof` — use the default-base-is-moof flag rather than
///   absolute `base_data_offset`s, which browser parsers handle more robustly.
pub const BASE_MOVFLAGS: [&str; 3] = ["frag_keyframe", "empty_moov", "default_base_moof"];

/// Assemble the `-movflags` value, e.g. `+frag_keyframe+empty_moov+default_base_moof`.
pub fn movflags(profile: Profile, segment_index: bool) -> String {
    let mut flags: Vec<&str> = BASE_MOVFLAGS.to_vec();
    match profile {
        Profile::Mse => {}
        // DASH-IF / CMAF both want version-1 ctts so negative composition
        // offsets survive the round-trip.
        Profile::Dash => flags.extend(["dash", "negative_cts_offsets"]),
        Profile::Cmaf => flags.extend(["cmaf", "negative_cts_offsets"]),
    }
    if segment_index {
        // A global `sidx` at the head of the file lets a player byte-range
        // request individual fragments without parsing the whole chain.
        flags.push("global_sidx");
    }
    format!("+{}", flags.join("+"))
}

/// Seconds → whole microseconds, the unit ffmpeg's `-frag_duration` /
/// `-min_frag_duration` take.
pub fn seconds_to_micros(seconds: f64) -> u64 {
    (seconds * 1_000_000.0).round() as u64
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that rewrites `in_name` as a
/// fragmented MP4 at `out_name`.
///
/// `fragment_duration` is in seconds; `0` means "one fragment per keyframe"
/// (ffmpeg's own behaviour with `frag_keyframe`). A positive value adds
/// `-min_frag_duration`, which keeps fragments keyframe-aligned but stops them
/// being cut shorter than the target — the same "fragments start at a random
/// access point, duration is approximate" contract dedicated fragmenters offer,
/// since the encoded video is never re-cut.
///
/// `keyframe_interval` (seconds) only applies to `Mode::H264`, where it forces a
/// regular keyframe cadence; `0` leaves the encoder's own GOP decision alone.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    mode: Mode,
    profile: Profile,
    fragment_duration: f64,
    keyframe_interval: f64,
    segment_index: bool,
) -> Vec<String> {
    let mut argv: Vec<String> = vec!["-i".into(), in_name.into(), "-map".into(), "0".into()];

    match mode {
        Mode::Copy => argv.extend(["-c".to_string(), "copy".to_string()]),
        Mode::H264 => {
            argv.extend([
                "-c:v".to_string(),
                "libx264".to_string(),
                // 8-bit 4:2:0 is the only chroma format browsers universally
                // decode; 10-bit/4:2:2 sources would otherwise stay unplayable.
                "-pix_fmt".to_string(),
                "yuv420p".to_string(),
                "-crf".to_string(),
                "23".to_string(),
            ]);
            if keyframe_interval > 0.0 {
                argv.extend([
                    "-force_key_frames".to_string(),
                    format!("expr:gte(t,n_forced*{})", trim_seconds(keyframe_interval)),
                ]);
            }
            argv.extend([
                "-c:a".to_string(),
                "aac".to_string(),
                "-b:a".to_string(),
                "128k".to_string(),
            ]);
        }
    }

    if fragment_duration > 0.0 {
        argv.extend([
            "-min_frag_duration".to_string(),
            seconds_to_micros(fragment_duration).to_string(),
        ]);
    }

    argv.extend([
        "-movflags".to_string(),
        movflags(profile, segment_index),
        out_name.to_string(),
    ]);
    argv
}

/// Render a seconds value for an ffmpeg expression without a trailing `.0`
/// (`2.0` → `2`, `1.5` → `1.5`) so the generated command reads like a
/// hand-written one.
fn trim_seconds(seconds: f64) -> String {
    if (seconds - seconds.round()).abs() < f64::EPSILON {
        format!("{}", seconds.round() as i64)
    } else {
        format!("{seconds}")
    }
}

/// Validate every param, then build `(argv, out_name)`. `out_name` is always
/// `out.mp4` — a fragmented MP4 is still an MP4. Single source shared by the
/// chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan(
    mode: &str,
    profile: &str,
    fragment_duration: f64,
    keyframe_interval: f64,
    segment_index: bool,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    let m = parse_mode(mode)?;
    let p = parse_profile(profile)?;
    if !fragment_duration.is_finite() || !(0.0..=MAX_SECONDS).contains(&fragment_duration) {
        return Err(format!(
            "fragment_duration must be 0-{MAX_SECONDS} seconds (0 = one fragment per keyframe), \
             got {fragment_duration}"
        ));
    }
    if !keyframe_interval.is_finite() || !(0.0..=MAX_SECONDS).contains(&keyframe_interval) {
        return Err(format!(
            "keyframe_interval must be 0-{MAX_SECONDS} seconds (0 = leave the encoder's own \
             keyframes), got {keyframe_interval}"
        ));
    }
    let out_name = "out.mp4".to_string();
    let argv = build_argv(
        in_name,
        &out_name,
        m,
        p,
        fragment_duration,
        keyframe_interval,
        segment_index,
    );
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag_value<'a>(argv: &'a [String], flag: &str) -> Option<&'a str> {
        argv.windows(2)
            .find(|w| w[0] == flag)
            .map(|w| w[1].as_str())
    }

    #[test]
    fn parse_mode_known() {
        assert_eq!(parse_mode("copy").unwrap(), Mode::Copy);
        assert_eq!(parse_mode("h264").unwrap(), Mode::H264);
    }

    #[test]
    fn parse_mode_rejects_unknown() {
        assert!(parse_mode("transcode").is_err());
        assert!(parse_mode("").is_err());
    }

    #[test]
    fn parse_profile_known_and_unknown() {
        assert_eq!(parse_profile("mse").unwrap(), Profile::Mse);
        assert_eq!(parse_profile("dash").unwrap(), Profile::Dash);
        assert_eq!(parse_profile("cmaf").unwrap(), Profile::Cmaf);
        assert!(parse_profile("hls").is_err());
    }

    #[test]
    fn default_plan_is_a_lossless_fragmenting_remux() {
        let (argv, out) = plan("copy", "mse", 0.0, 2.0, false, "in.mp4").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(argv.first().map(String::as_str), Some("-i"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
        // Lossless: stream copy, no encoder anywhere in the argv.
        assert_eq!(flag_value(&argv, "-c"), Some("copy"));
        assert!(!argv.iter().any(|a| a == "libx264"));
        // All tracks carried over.
        assert_eq!(flag_value(&argv, "-map"), Some("0"));
        // The three flags that make the file fragmented.
        assert_eq!(
            flag_value(&argv, "-movflags"),
            Some("+frag_keyframe+empty_moov+default_base_moof")
        );
        // No duration override when fragment_duration = 0.
        assert!(!argv.iter().any(|a| a == "-min_frag_duration"));
        // keyframe_interval is an h264-only knob — copy mode never forces keyframes.
        assert!(!argv.iter().any(|a| a == "-force_key_frames"));
    }

    #[test]
    fn fragment_duration_is_converted_to_microseconds() {
        let (argv, _) = plan("copy", "mse", 2.0, 0.0, false, "in.mp4").unwrap();
        assert_eq!(flag_value(&argv, "-min_frag_duration"), Some("2000000"));
        // Still keyframe-aligned: min_frag_duration only sets a floor.
        assert!(flag_value(&argv, "-movflags")
            .unwrap()
            .contains("frag_keyframe"));

        let (argv, _) = plan("copy", "mse", 0.5, 0.0, false, "in.mp4").unwrap();
        assert_eq!(flag_value(&argv, "-min_frag_duration"), Some("500000"));
    }

    #[test]
    fn dash_and_cmaf_profiles_add_their_flags() {
        let (argv, _) = plan("copy", "dash", 0.0, 0.0, false, "in.mp4").unwrap();
        assert_eq!(
            flag_value(&argv, "-movflags"),
            Some("+frag_keyframe+empty_moov+default_base_moof+dash+negative_cts_offsets")
        );
        let (argv, _) = plan("copy", "cmaf", 0.0, 0.0, false, "in.mp4").unwrap();
        assert_eq!(
            flag_value(&argv, "-movflags"),
            Some("+frag_keyframe+empty_moov+default_base_moof+cmaf+negative_cts_offsets")
        );
    }

    #[test]
    fn segment_index_appends_global_sidx() {
        let (argv, _) = plan("copy", "mse", 0.0, 0.0, true, "in.mp4").unwrap();
        assert_eq!(
            flag_value(&argv, "-movflags"),
            Some("+frag_keyframe+empty_moov+default_base_moof+global_sidx")
        );
    }

    #[test]
    fn h264_mode_reencodes_and_forces_keyframes() {
        let (argv, _) = plan("h264", "mse", 0.0, 2.0, false, "in.mkv").unwrap();
        assert_eq!(flag_value(&argv, "-c:v"), Some("libx264"));
        assert_eq!(flag_value(&argv, "-c:a"), Some("aac"));
        assert_eq!(flag_value(&argv, "-pix_fmt"), Some("yuv420p"));
        assert_eq!(
            flag_value(&argv, "-force_key_frames"),
            Some("expr:gte(t,n_forced*2)")
        );
        assert!(!argv.iter().any(|a| a == "copy"));
    }

    #[test]
    fn h264_keyframe_interval_zero_leaves_encoder_gop_alone() {
        let (argv, _) = plan("h264", "mse", 0.0, 0.0, false, "in.mp4").unwrap();
        assert_eq!(flag_value(&argv, "-c:v"), Some("libx264"));
        assert!(!argv.iter().any(|a| a == "-force_key_frames"));
    }

    #[test]
    fn fractional_keyframe_interval_keeps_its_decimals() {
        let (argv, _) = plan("h264", "mse", 0.0, 1.5, false, "in.mp4").unwrap();
        assert_eq!(
            flag_value(&argv, "-force_key_frames"),
            Some("expr:gte(t,n_forced*1.5)")
        );
    }

    #[test]
    fn plan_rejects_out_of_range_and_unknown_values() {
        assert!(plan("copy", "mse", -1.0, 2.0, false, "in.mp4").is_err());
        assert!(plan("copy", "mse", 31.0, 2.0, false, "in.mp4").is_err());
        assert!(plan("copy", "mse", 0.0, -0.5, false, "in.mp4").is_err());
        assert!(plan("copy", "mse", 0.0, 31.0, false, "in.mp4").is_err());
        assert!(plan("bogus", "mse", 0.0, 2.0, false, "in.mp4").is_err());
        assert!(plan("copy", "bogus", 0.0, 2.0, false, "in.mp4").is_err());
        assert!(plan("copy", "mse", f64::NAN, 2.0, false, "in.mp4").is_err());
    }

    #[test]
    fn boundary_values_are_accepted() {
        assert!(plan("copy", "mse", 0.0, 0.0, false, "in.mp4").is_ok());
        assert!(plan("copy", "mse", MAX_SECONDS, MAX_SECONDS, true, "in.mp4").is_ok());
        let (argv, _) = plan("copy", "mse", MAX_SECONDS, 0.0, false, "in.mp4").unwrap();
        assert_eq!(flag_value(&argv, "-min_frag_duration"), Some("30000000"));
    }
}
