//! gizza-ai/mxf-to-mp4 core — pure ffmpeg argv construction shared by the chat
//! block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! mxf-to-mp4 turns a broadcast SMPTE **MXF** file into a playable **MP4**. It is
//! deliberately *essence-aware* rather than a generic container relabel, because
//! a naive remux of an MXF produces a file that looks converted and is not:
//!
//! 1. **MXF audio is PCM, and `-c copy` into MP4 writes it as `ipcm`.** ffmpeg
//!    accepts that mux without complaint, but `ipcm` (ISO 14496-12 raw PCM) is
//!    decoded by essentially no browser or consumer player, so the result plays
//!    silent. Every audio mode here therefore *re-encodes audio to AAC*; the
//!    only question is how the tracks are combined.
//! 2. **Broadcast MXF ships audio as discrete MONO tracks** — channel-per-track,
//!    commonly 2, 4, 8 or 16 of them — not as one stereo track. Keeping "the"
//!    audio track gives you one arbitrary mono channel. `audio = "merge"`
//!    combines the first N with `amerge` and downmixes to stereo.
//!
//! The picture, by contrast, is often already MP4-legal: AVC-Intra, XAVC and
//! plain H.264 MXF all carry H.264 essence. `video = "rewrap"` stream-copies it
//! (`-c:v copy`) so a broadcast master is not needlessly re-encoded, while still
//! fixing the audio. MPEG-2-based XDCAM HD / IMX and DNxHD essence cannot be
//! rewrapped into MP4 — ffmpeg refuses with "Could not find tag for codec … not
//! currently supported in container" — so `video = "h264"` (the default)
//! re-encodes with libx264 and always produces a playable file.
//!
//! Deliberately distinct from its neighbours:
//! - `mkv-to-mp4` / `mov-to-mp4` `mode=copy` do a FULL `-c copy` (audio included),
//!   which is exactly the `ipcm` trap above; neither offers a video-copy +
//!   audio-transcode hybrid, and neither merges mono tracks.
//! - `video-to-h264` / `video-transcode` ALWAYS re-encode the picture.
//! - `video-audio-track-selector` keeps one track losslessly; it never combines them.
//! - `video-to-mxf` is the inverse direction.
//!
//! Everything that is a separate tool stays a separate tool: scaling
//! (`video-resize`), frame rate (`video-fps`), trimming (`video-trim`),
//! deinterlacing (`video-deinterlace`) and target-size encoding
//! (`video-target-filesize-encoder`) are not duplicated here.

/// What happens to the picture essence on the way into the MP4.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Video {
    /// `h264` — re-encode with libx264 at the chosen quality, pinned to
    /// `yuv420p` 8-bit 4:2:0 so the result plays everywhere. Always works,
    /// whatever the MXF held (MPEG-2 XDCAM, IMX/D-10, DNxHD, AVC-Intra, XAVC).
    H264,
    /// `rewrap` — `-c:v copy`: the picture stream is moved into the MP4
    /// untouched, with no generation loss and near-instantly. Requires the MXF's
    /// essence to be MP4-legal (H.264 family: AVC-Intra, XAVC, plain H.264; or
    /// HEVC). MPEG-2 / DNxHD essence makes ffmpeg refuse the mux — use `h264`.
    Rewrap,
}

/// Parse the user-facing `picture` string (the values the chat schema + page
/// accept). Named `picture` on every surface because the page's file input
/// already owns the name `video`.
pub fn parse_video(s: &str) -> Result<Video, String> {
    match s {
        "h264" => Ok(Video::H264),
        "rewrap" => Ok(Video::Rewrap),
        other => Err(format!("picture {other:?} not supported (h264|rewrap)")),
    }
}

/// How the MXF's audio tracks become MP4 audio. Every variant that keeps audio
/// re-encodes it to AAC — see the module docs for why `-c copy` is not an option.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Audio {
    /// `stereo` — take the FIRST audio track only and encode it as 2-channel
    /// AAC. The safe default: one track in, one normal stereo track out.
    Stereo,
    /// `merge` — combine the first `merge_tracks` tracks with `amerge` and
    /// downmix the result to stereo AAC. This is the broadcast case: MXF
    /// channel-per-track audio (L and R as two separate mono tracks, or four /
    /// eight discrete channels) becomes one ordinary stereo track.
    Merge,
    /// `all` — keep every audio track as its own AAC stream, preserving each
    /// track's native channel count. Use when the separate tracks matter (an
    /// alternate language, a clean effects bed) and the player can pick.
    All,
    /// `none` — `-an`: drop audio entirely, picture-only MP4.
    None,
}

/// Parse the user-facing audio string (the values the chat schema + page accept).
pub fn parse_audio(s: &str) -> Result<Audio, String> {
    match s {
        "stereo" => Ok(Audio::Stereo),
        "merge" => Ok(Audio::Merge),
        "all" => Ok(Audio::All),
        "none" => Ok(Audio::None),
        other => Err(format!(
            "audio {other:?} not supported (stereo|merge|all|none)"
        )),
    }
}

/// Default picture handling — re-encode, because most broadcast MXF essence
/// (MPEG-2 XDCAM, IMX, DNxHD) simply cannot be rewrapped into MP4.
pub const DEFAULT_VIDEO: Video = Video::H264;
/// Default audio handling — first track as plain stereo AAC.
pub const DEFAULT_AUDIO: Audio = Audio::Stereo;
/// Default number of mono tracks combined when `audio = "merge"`. Two is the
/// channel-per-track L/R convention.
pub const DEFAULT_MERGE_TRACKS: u8 = 2;
/// Default AAC bitrate in kbps — transparent enough for a stereo delivery copy.
pub const DEFAULT_AUDIO_BITRATE: u16 = 192;
/// Default `quality` when none is supplied (≈ CRF 24 — a good size/quality balance).
pub const DEFAULT_QUALITY: u8 = 75;

/// Fewest tracks `amerge` can combine — merging one track is not a merge.
pub const MIN_MERGE_TRACKS: u8 = 2;
/// Most tracks `amerge` is offered here. 16 is the widest channel-per-track
/// layout broadcast MXF realistically carries.
pub const MAX_MERGE_TRACKS: u8 = 16;
/// Lowest AAC bitrate offered, in kbps.
pub const MIN_AUDIO_BITRATE: u16 = 32;
/// Highest AAC bitrate offered, in kbps.
pub const MAX_AUDIO_BITRATE: u16 = 320;

/// Lowest CRF the quality slider maps to (`quality = 100`). 18 is "visually
/// lossless" for libx264 — deliberately NOT 0 (true-lossless), which produces
/// enormous files that blow past the tool's output-size cap for even short clips.
pub const MIN_CRF: f32 = 18.0;
/// Highest CRF the quality slider maps to (`quality = 1`) — low quality, small
/// file. Above ~40 libx264 output degrades sharply, so the range stops here.
pub const MAX_CRF: f32 = 40.0;

/// Map web-conventional quality 1-100 to a practical ffmpeg libx264 CRF, high
/// quality → low CRF: `quality = 100` → CRF 18 (visually lossless), `quality =
/// 1` → CRF 40 (small, low quality), default 75 ≈ CRF 24. Only meaningful when
/// `video = "h264"`.
pub fn quality_to_crf(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let crf = MAX_CRF - (q - 1.0) * (MAX_CRF - MIN_CRF) / 99.0;
    crf.round().clamp(MIN_CRF, MAX_CRF) as u8
}

/// The `amerge` filtergraph combining the first `n` audio tracks into one
/// labelled output: `[0:a:0][0:a:1]amerge=inputs=2[a]`.
///
/// Note the inputs are addressed positionally (`0:a:0`, `0:a:1`, …) with NO `?`
/// — `-filter_complex` inputs cannot be optional, so an MXF with fewer tracks
/// than requested fails loudly with ffmpeg's own "matches no streams" message
/// rather than silently producing a partial mix.
pub fn amerge_filter(n: u8) -> String {
    let mut f = String::new();
    for i in 0..n {
        f.push_str(&format!("[0:a:{i}]"));
    }
    f.push_str(&format!("amerge=inputs={n}[a]"));
    f
}

/// Build the ffmpeg argv (no leading `ffmpeg`) converting `in_name` → `out_name`.
///
/// Shape, in order: input, optional `amerge` filtergraph, stream mapping, video
/// codec, audio codec, `-write_tmcd 0`, `-movflags +faststart` (moov atom at the
/// front for progressive web playback), output. Only the video and audio streams
/// are mapped, so the MXF's timecode/data tracks are left behind — and
/// `-write_tmcd 0` stops the MP4 muxer from silently *re-creating* a `tmcd`
/// track out of the MXF's start-timecode metadata, which some players list as a
/// third, undecodable stream.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    video: Video,
    crf: u8,
    audio: Audio,
    merge_tracks: u8,
    audio_bitrate: u16,
) -> Vec<String> {
    let mut argv: Vec<String> = vec!["-i".into(), in_name.into()];

    // `merge` needs a filtergraph before the mapping that consumes its output.
    if audio == Audio::Merge {
        argv.push("-filter_complex".into());
        argv.push(amerge_filter(merge_tracks));
    }

    // Picture: always the first video stream. `?` keeps an audio-only MXF from
    // hard-failing on the map itself.
    argv.push("-map".into());
    argv.push("0:v:0?".into());

    match audio {
        // The merged filtergraph output replaces the source tracks.
        Audio::Merge => {
            argv.push("-map".into());
            argv.push("[a]".into());
        }
        // First track only.
        Audio::Stereo => {
            argv.push("-map".into());
            argv.push("0:a:0?".into());
        }
        // Every track, each kept as its own stream.
        Audio::All => {
            argv.push("-map".into());
            argv.push("0:a?".into());
        }
        Audio::None => {}
    }

    match video {
        Video::H264 => {
            argv.push("-c:v".into());
            argv.push("libx264".into());
            argv.push("-pix_fmt".into());
            argv.push("yuv420p".into());
            argv.push("-crf".into());
            argv.push(crf.to_string());
            argv.push("-preset".into());
            argv.push("medium".into());
        }
        Video::Rewrap => {
            argv.push("-c:v".into());
            argv.push("copy".into());
        }
    }

    match audio {
        Audio::None => argv.push("-an".into()),
        other => {
            argv.push("-c:a".into());
            argv.push("aac".into());
            argv.push("-b:a".into());
            argv.push(format!("{audio_bitrate}k"));
            // `stereo` and `merge` both guarantee an ordinary 2-channel track —
            // for `merge` this is what downmixes a 4- or 8-channel amerge result.
            // `all` deliberately preserves each track's native channel count.
            if other != Audio::All {
                argv.push("-ac".into());
                argv.push("2".into());
            }
        }
    }

    // Mapping alone is not enough: the MP4 muxer synthesises a `tmcd` track from
    // the MXF's start-timecode metadata unless this is off.
    argv.push("-write_tmcd".into());
    argv.push("0".into());

    argv.push("-movflags".into());
    argv.push("+faststart".into());
    argv.push(out_name.into());
    argv
}

/// Validate every parameter, then build `(argv, out_name)`. `out_name` is always
/// `out.mp4`. Single source shared by the chat block (`src/lib.rs`) and the web
/// page (`web/src/lib.rs`).
pub fn plan(
    video: &str,
    quality: u8,
    audio: &str,
    merge_tracks: u8,
    audio_bitrate: u16,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    if !(1..=100).contains(&quality) {
        return Err(format!("quality must be 1-100, got {quality}"));
    }
    if !(MIN_MERGE_TRACKS..=MAX_MERGE_TRACKS).contains(&merge_tracks) {
        return Err(format!(
            "merge_tracks must be {MIN_MERGE_TRACKS}-{MAX_MERGE_TRACKS}, got {merge_tracks}"
        ));
    }
    if !(MIN_AUDIO_BITRATE..=MAX_AUDIO_BITRATE).contains(&audio_bitrate) {
        return Err(format!(
            "audio_bitrate must be {MIN_AUDIO_BITRATE}-{MAX_AUDIO_BITRATE} kbps, got {audio_bitrate}"
        ));
    }
    let v = parse_video(video)?;
    let a = parse_audio(audio)?;
    let crf = quality_to_crf(quality);
    let out_name = "out.mp4".to_string();
    Ok((
        build_argv(in_name, &out_name, v, crf, a, merge_tracks, audio_bitrate),
        out_name,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan_ok(video: &str, audio: &str) -> Vec<String> {
        plan(
            video,
            DEFAULT_QUALITY,
            audio,
            DEFAULT_MERGE_TRACKS,
            DEFAULT_AUDIO_BITRATE,
            "in.mxf",
        )
        .unwrap()
        .0
    }

    fn has_pair(argv: &[String], a: &str, b: &str) -> bool {
        argv.windows(2).any(|w| w[0] == a && w[1] == b)
    }

    #[test]
    fn parse_video_known_and_unknown() {
        assert_eq!(parse_video("h264").unwrap(), Video::H264);
        assert_eq!(parse_video("rewrap").unwrap(), Video::Rewrap);
        assert!(parse_video("copy").is_err());
        assert!(parse_video("").is_err());
    }

    #[test]
    fn parse_audio_known_and_unknown() {
        assert_eq!(parse_audio("stereo").unwrap(), Audio::Stereo);
        assert_eq!(parse_audio("merge").unwrap(), Audio::Merge);
        assert_eq!(parse_audio("all").unwrap(), Audio::All);
        assert_eq!(parse_audio("none").unwrap(), Audio::None);
        assert!(parse_audio("mono").is_err());
        assert!(parse_audio("pcm16").is_err());
    }

    /// Happy path: the default h264 + stereo combination.
    #[test]
    fn default_plan_transcodes_h264_with_stereo_aac() {
        let (argv, out) = plan(
            "h264",
            DEFAULT_QUALITY,
            "stereo",
            DEFAULT_MERGE_TRACKS,
            DEFAULT_AUDIO_BITRATE,
            "in.mxf",
        )
        .unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(argv.first().map(String::as_str), Some("-i"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
        assert!(has_pair(&argv, "-c:v", "libx264"));
        assert!(has_pair(&argv, "-pix_fmt", "yuv420p"));
        assert!(has_pair(&argv, "-preset", "medium"));
        // quality 75 → CRF ~24.
        let i = argv.iter().position(|a| a == "-crf").unwrap();
        let crf: u8 = argv[i + 1].parse().unwrap();
        assert!((23..=25).contains(&crf), "expected CRF 23-25, got {crf}");
        // Audio is ALWAYS re-encoded — never `-c copy` (which would write ipcm).
        assert!(has_pair(&argv, "-c:a", "aac"));
        assert!(has_pair(&argv, "-b:a", "192k"));
        assert!(has_pair(&argv, "-ac", "2"));
        assert!(has_pair(&argv, "-map", "0:a:0?"), "first track only: {argv:?}");
        assert!(has_pair(&argv, "-movflags", "+faststart"));
        assert!(!argv.iter().any(|a| a == "-filter_complex"));
    }

    /// Error path: every validated range rejects out-of-range input, and both
    /// enums reject unknown values.
    #[test]
    fn plan_rejects_invalid_parameters() {
        let d = (DEFAULT_MERGE_TRACKS, DEFAULT_AUDIO_BITRATE);
        assert!(plan("h264", 0, "stereo", d.0, d.1, "in.mxf").is_err());
        assert!(plan("h264", 101, "stereo", d.0, d.1, "in.mxf").is_err());
        assert!(plan("h264", 75, "stereo", 1, d.1, "in.mxf").is_err());
        assert!(plan("h264", 75, "stereo", 17, d.1, "in.mxf").is_err());
        assert!(plan("h264", 75, "stereo", d.0, 31, "in.mxf").is_err());
        assert!(plan("h264", 75, "stereo", d.0, 321, "in.mxf").is_err());
        assert!(plan("mpeg2", 75, "stereo", d.0, d.1, "in.mxf").is_err());
        assert!(plan("h264", 75, "surround", d.0, d.1, "in.mxf").is_err());
    }

    /// The error text names the accepted values, not just "invalid".
    #[test]
    fn errors_state_what_was_expected() {
        let e = plan("copy", 75, "stereo", 2, 192, "in.mxf").unwrap_err();
        assert!(e.contains("h264|rewrap"), "{e}");
        let e = plan("h264", 75, "stereo", 99, 192, "in.mxf").unwrap_err();
        assert!(e.contains("2-16") && e.contains("99"), "{e}");
    }

    /// The headline differentiator: keep the picture essence, fix only the audio.
    #[test]
    fn rewrap_copies_picture_but_still_reencodes_audio() {
        let argv = plan_ok("rewrap", "stereo");
        assert!(has_pair(&argv, "-c:v", "copy"));
        assert!(!argv.iter().any(|a| a == "libx264"), "rewrap must not re-encode picture");
        assert!(!argv.iter().any(|a| a == "-crf"));
        // Audio is STILL transcoded — a full `-c copy` would write unplayable ipcm.
        assert!(has_pair(&argv, "-c:a", "aac"));
        assert!(
            !has_pair(&argv, "-c", "copy"),
            "never a blanket -c copy: {argv:?}"
        );
    }

    /// The second differentiator: channel-per-track mono audio → one stereo track.
    #[test]
    fn merge_builds_amerge_graph_and_downmixes_to_stereo() {
        let (argv, _) = plan("h264", 75, "merge", 4, 192, "in.mxf").unwrap();
        assert!(has_pair(
            &argv,
            "-filter_complex",
            "[0:a:0][0:a:1][0:a:2][0:a:3]amerge=inputs=4[a]"
        ));
        assert!(has_pair(&argv, "-map", "[a]"));
        // 4 merged channels are downmixed to an ordinary stereo track.
        assert!(has_pair(&argv, "-ac", "2"));
        // The raw source tracks are not ALSO mapped.
        assert!(!has_pair(&argv, "-map", "0:a?"));
        assert!(!has_pair(&argv, "-map", "0:a:0?"));
    }

    #[test]
    fn amerge_filter_shape() {
        assert_eq!(amerge_filter(2), "[0:a:0][0:a:1]amerge=inputs=2[a]");
        assert_eq!(
            amerge_filter(3),
            "[0:a:0][0:a:1][0:a:2]amerge=inputs=3[a]"
        );
    }

    /// `all` keeps every track and, unlike stereo/merge, does NOT force 2 channels.
    #[test]
    fn all_keeps_every_track_at_native_channel_count() {
        let argv = plan_ok("h264", "all");
        assert!(has_pair(&argv, "-map", "0:a?"));
        assert!(has_pair(&argv, "-c:a", "aac"));
        assert!(
            !has_pair(&argv, "-ac", "2"),
            "all must preserve native channel counts: {argv:?}"
        );
    }

    #[test]
    fn none_drops_audio_entirely() {
        let argv = plan_ok("h264", "none");
        assert!(argv.iter().any(|a| a == "-an"));
        assert!(!argv.iter().any(|a| a == "-c:a"));
        assert!(!argv.iter().any(|a| a == "-b:a"));
        assert!(!has_pair(&argv, "-map", "0:a?"));
    }

    /// The MXF timecode/data track is never mapped into the MP4, and the MP4
    /// muxer is stopped from re-creating one from the start-timecode metadata.
    #[test]
    fn data_streams_are_never_mapped() {
        for audio in ["stereo", "merge", "all", "none"] {
            let argv = plan_ok("h264", audio);
            assert!(
                !argv.iter().any(|a| a.starts_with("0:d")),
                "{audio}: data track must not be mapped: {argv:?}"
            );
            assert!(
                has_pair(&argv, "-write_tmcd", "0"),
                "{audio}: muxer must not synthesise a tmcd track: {argv:?}"
            );
        }
    }

    #[test]
    fn quality_to_crf_endpoints() {
        assert_eq!(quality_to_crf(100), 18);
        assert_eq!(quality_to_crf(1), 40);
    }

    #[test]
    fn audio_bitrate_boundaries_accepted() {
        for kbps in [MIN_AUDIO_BITRATE, 128, MAX_AUDIO_BITRATE] {
            let (argv, _) = plan("h264", 75, "stereo", 2, kbps, "in.mxf").unwrap();
            assert!(has_pair(&argv, "-b:a", &format!("{kbps}k")));
        }
    }

    #[test]
    fn merge_track_boundaries_accepted() {
        for n in [MIN_MERGE_TRACKS, MAX_MERGE_TRACKS] {
            let (argv, _) = plan("h264", 75, "merge", n, 192, "in.mxf").unwrap();
            assert!(argv.iter().any(|a| a.contains(&format!("amerge=inputs={n}"))));
        }
    }
}
