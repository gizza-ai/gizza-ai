//! gizza-ai/video-scene-split core — pure scene-detection + clip-argv logic
//! shared by the chat/CLI block and the standalone page. No wafer/wasm/ffmpeg
//! deps: this crate only builds ffmpeg argv, parses the resulting log, turns the
//! cut list into scene windows, and renders the scene table as CSV.
//!
//! Pipeline (the caller drives ffmpeg; this crate does the pure parts):
//!  1. [`detect_argv`] → a pass that runs ffmpeg's `scene` detector over the
//!     video (`select='gt(scene,threshold)',showinfo`) and writes NO output
//!     file — every flagged frame lands in the log as a `showinfo` line.
//!  2. [`parse_cuts`] reads the `pts_time:` values out of that log (= the
//!     shot-boundary timestamps) and [`parse_duration`] reads the input
//!     `Duration:` banner so the last scene can be closed.
//!  3. [`apply_min_scene`] drops boundaries closer together than `min_scene`
//!     seconds, so one hard cut spread over two frames counts once.
//!  4. [`build_scenes`] turns the boundaries into `[start, end)` windows and
//!     [`clip_argv`] builds the per-scene extract command (re-encode or stream
//!     copy).
//!  5. [`scenes_csv`] renders the same table PySceneDetect's `list-scenes`
//!     emits, shipped alongside the clips.
//!
//! Pure Rust → compiles for the chat block (wasm32-wasip1), the page
//! (wasm32-unknown-unknown) and native tests alike.

/// ffmpeg scene-score cutoff, 0.0–1.0. Matches the sibling `video-scene-cut-diff`
/// block so the same footage reports the same cuts in both tools.
pub const DEFAULT_THRESHOLD: f64 = 0.3;

/// Shortest scene we will emit, in seconds (PySceneDetect's `min-scene-len`
/// default is the same 0.6 s).
pub const DEFAULT_MIN_SCENE: f64 = 0.6;

/// x264 constant-rate-factor for the re-encode mode (PySceneDetect's
/// `split-video` default is also 22).
pub const DEFAULT_CRF: i64 = 22;

/// x264 speed preset for the re-encode mode (PySceneDetect's default too).
pub const DEFAULT_PRESET: &str = "veryfast";

/// How each clip is extracted.
pub const DEFAULT_MODE: &str = "reencode";

/// The accepted `mode` values.
pub const MODES: [&str; 2] = ["reencode", "copy"];

/// The accepted `preset` values (the x264 presets worth exposing).
pub const PRESETS: [&str; 8] = [
    "ultrafast",
    "superfast",
    "veryfast",
    "faster",
    "fast",
    "medium",
    "slow",
    "veryslow",
];

/// Hard cap on clips per run — a pathologically low `threshold` on noisy footage
/// can flag hundreds of frames, and every clip is a separate ffmpeg pass. Past
/// this we refuse with an actionable message instead of grinding.
pub const MAX_SCENES: usize = 200;

/// Round to two decimals (10 ms) — the resolution every timestamp is reported at.
pub fn round2(v: f64) -> f64 {
    if v.is_finite() {
        (v * 100.0).round() / 100.0
    } else {
        v
    }
}

/// Format an `f64` for an ffmpeg arg / CSV cell without a trailing `.0`
/// (`2` not `2.0`, `2.5` stays `2.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Every knob the split takes, already validated by [`validate`].
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    /// ffmpeg scene score cutoff, 0.0–1.0.
    pub threshold: f64,
    /// Shortest scene in seconds; closer boundaries are merged.
    pub min_scene: f64,
    /// `"reencode"` or `"copy"`.
    pub mode: String,
    /// x264 CRF 0–51 (re-encode mode only).
    pub crf: i64,
    /// x264 speed preset (re-encode mode only).
    pub preset: String,
    /// Keep the audio track in each clip (false adds `-an`).
    pub keep_audio: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_THRESHOLD,
            min_scene: DEFAULT_MIN_SCENE,
            mode: DEFAULT_MODE.to_string(),
            crf: DEFAULT_CRF,
            preset: DEFAULT_PRESET.to_string(),
            keep_audio: true,
        }
    }
}

impl Params {
    /// True when clips are stream-copied rather than re-encoded.
    pub fn is_copy(&self) -> bool {
        self.mode == "copy"
    }
}

/// Validate and normalize the knobs. Every message names the parameter, the
/// accepted range/values, and what was actually passed.
pub fn validate(p: Params) -> Result<Params, String> {
    if !p.threshold.is_finite() || !(0.0..=1.0).contains(&p.threshold) {
        return Err(format!(
            "threshold must be between 0.0 and 1.0 (got {})",
            p.threshold
        ));
    }
    if !p.min_scene.is_finite() || p.min_scene < 0.0 {
        return Err(format!(
            "min_scene must be 0 or more seconds (got {})",
            p.min_scene
        ));
    }
    if !MODES.contains(&p.mode.as_str()) {
        return Err(format!(
            "mode must be one of {} (got '{}')",
            MODES.join(", "),
            p.mode
        ));
    }
    if !(0..=51).contains(&p.crf) {
        return Err(format!("crf must be between 0 and 51 (got {})", p.crf));
    }
    if !PRESETS.contains(&p.preset.as_str()) {
        return Err(format!(
            "preset must be one of {} (got '{}')",
            PRESETS.join(", "),
            p.preset
        ));
    }
    Ok(p)
}

/// One detected scene: the half-open window `[start, end)` in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// 1-based scene number.
    pub index: usize,
    pub start: f64,
    pub end: f64,
}

impl Scene {
    /// Length of the scene in seconds.
    pub fn duration(&self) -> f64 {
        round2(self.end - self.start)
    }

    /// The name ffmpeg writes inside its (virtual) working dir — always ASCII
    /// and free of anything a filesystem or the browser FS could choke on.
    pub fn out_name(&self, ext: &str) -> String {
        format!("scene-{:03}.{ext}", self.index)
    }

    /// The user-facing name: `<stem>-Scene-001.mp4`, mirroring the convention
    /// desktop scene splitters use so downloaded clips sort correctly.
    pub fn entry_name(&self, stem: &str, ext: &str) -> String {
        format!("{stem}-Scene-{:03}.{ext}", self.index)
    }
}

/// Reduce a source filename to a safe, readable stem for clip names: the part
/// before the extension, with anything outside `[A-Za-z0-9._-]` replaced by `_`,
/// trimmed to 40 chars. Empty/unusable input falls back to `video`.
pub fn safe_stem(filename: &str) -> String {
    let base = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    let stem = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
    let cleaned: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    let cleaned = cleaned.trim_matches(['.', '_']).to_string();
    if cleaned.is_empty() {
        "video".to_string()
    } else {
        cleaned
    }
}

/// The extension each clip gets. Re-encoding always produces H.264/AAC in MP4;
/// stream copy has to keep the source container, since the copied packets may
/// not be MP4-legal (VP9/Opus in WebM, for instance).
pub fn clip_ext<'a>(in_ext: &'a str, mode: &str) -> &'a str {
    if mode == "copy" {
        in_ext
    } else {
        "mp4"
    }
}

/// Build the detection pass: run the `scene` filter through `showinfo` and throw
/// the video away (`-f null -`), so the only product is the log. `threshold` is
/// ffmpeg's scene score (0.0–1.0); a frame is flagged when its visual difference
/// from the previous frame exceeds it. Lower = more cuts.
pub fn detect_argv(in_name: &str, threshold: f64) -> Vec<String> {
    let vf = format!("select='gt(scene\\,{})',showinfo", fmt_num(threshold));
    [
        "-hide_banner",
        "-nostats",
        "-i",
        in_name,
        "-an",
        "-sn",
        "-map",
        "0:v:0",
        "-filter:v",
        &vf,
        "-f",
        "null",
        "-",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Extract shot-boundary timestamps (seconds) from a `showinfo` log: the
/// `pts_time:` field of every line the filter itself emitted (tagged
/// `[Parsed_showinfo_N @ 0x…]`), sorted and de-duplicated. Other `pts_time`
/// mentions (input banners) are ignored.
pub fn parse_cuts(log: &str) -> Vec<f64> {
    let mut cuts: Vec<f64> = Vec::new();
    for line in log.lines() {
        if !line.contains("Parsed_showinfo") {
            continue;
        }
        if let Some(idx) = line.find("pts_time:") {
            let rest = &line[idx + "pts_time:".len()..];
            let num: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
            if let Ok(t) = num.parse::<f64>() {
                if t.is_finite() && t > 0.0 {
                    cuts.push(round2(t));
                }
            }
        }
    }
    cuts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    cuts.dedup();
    cuts
}

/// Read the clip length from ffmpeg's `Duration: HH:MM:SS.ss` input banner (the
/// detection pass prints it even with `-hide_banner`). `N/A` → `None`.
pub fn parse_duration(log: &str) -> Option<f64> {
    let idx = log.find("Duration:")?;
    let rest = &log[idx + "Duration:".len()..];
    let field: String = rest
        .trim_start()
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != ',')
        .collect();
    let parts: Vec<&str> = field.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: f64 = parts[0].parse().ok()?;
    let m: f64 = parts[1].parse().ok()?;
    let s: f64 = parts[2].parse().ok()?;
    let total = h * 3600.0 + m * 60.0 + s;
    if total.is_finite() && total > 0.0 {
        Some(round2(total))
    } else {
        None
    }
}

/// Drop boundaries that land within `min_scene` seconds of the previous kept one
/// — counting the start of the video as the first boundary, so scene 1 is never
/// shorter than `min_scene` either. `min_scene <= 0` keeps every cut. Input is
/// assumed sorted ascending.
pub fn apply_min_scene(cuts: &[f64], min_scene: f64) -> Vec<f64> {
    if min_scene <= 0.0 {
        return cuts.to_vec();
    }
    let mut kept: Vec<f64> = Vec::new();
    let mut last = 0.0_f64;
    for &t in cuts {
        if t - last < min_scene {
            continue;
        }
        kept.push(t);
        last = t;
    }
    kept
}

/// Turn de-bounced boundaries + the clip duration into `[start, end)` windows.
/// A trailing scene shorter than `min_scene` is folded back into the one before
/// it (the tail equivalent of [`apply_min_scene`]). Errors when the duration is
/// unusable or the cap would be exceeded.
pub fn build_scenes(cuts: &[f64], duration: f64, min_scene: f64) -> Result<Vec<Scene>, String> {
    if !duration.is_finite() || duration <= 0.0 {
        return Err(
            "could not read the video duration — the file may be truncated or not a video".into(),
        );
    }
    let mut bounds: Vec<f64> = cuts
        .iter()
        .copied()
        .filter(|&t| t > 0.0 && t < duration)
        .collect();
    // Fold a too-short tail back into the previous scene.
    if min_scene > 0.0 {
        while let Some(&last) = bounds.last() {
            if duration - last < min_scene {
                bounds.pop();
            } else {
                break;
            }
        }
    }
    if bounds.len() + 1 > MAX_SCENES {
        return Err(format!(
            "{} scenes detected, over the {MAX_SCENES}-clip limit — raise threshold or min_scene, \
             or split a shorter section first",
            bounds.len() + 1
        ));
    }
    let mut scenes = Vec::with_capacity(bounds.len() + 1);
    let mut start = 0.0_f64;
    for (i, &b) in bounds.iter().enumerate() {
        scenes.push(Scene {
            index: i + 1,
            start: round2(start),
            end: round2(b),
        });
        start = b;
    }
    scenes.push(Scene {
        index: bounds.len() + 1,
        start: round2(start),
        end: round2(duration),
    });
    Ok(scenes)
}

/// Build the extract command for one scene, returning `(argv, out_name)`.
///
/// Seeking is `-ss <start>` BEFORE `-i` plus `-t <length>` after it — fast input
/// seek with an explicit output length, which behaves identically in native
/// ffmpeg and the browser build. `is_last` omits `-t` so the final clip simply
/// runs to EOF and no rounding can clip its tail.
///
/// Re-encode mode produces H.264 + AAC in MP4 (frame-accurate boundaries);
/// copy mode remuxes the original packets, which is near-instant but snaps each
/// start back to the previous keyframe.
pub fn clip_argv(in_name: &str, scene: &Scene, is_last: bool, p: &Params, ext: &str) -> (Vec<String>, String) {
    let out_name = scene.out_name(ext);
    let mut argv: Vec<String> = vec![
        "-hide_banner".into(),
        "-nostats".into(),
        "-ss".into(),
        fmt_num(scene.start),
        "-i".into(),
        in_name.into(),
    ];
    if !is_last {
        argv.push("-t".into());
        argv.push(fmt_num(scene.duration()));
    }
    if p.is_copy() {
        argv.push("-c".into());
        argv.push("copy".into());
        argv.push("-avoid_negative_ts".into());
        argv.push("make_zero".into());
    } else {
        argv.push("-c:v".into());
        argv.push("libx264".into());
        argv.push("-preset".into());
        argv.push(p.preset.clone());
        argv.push("-crf".into());
        argv.push(p.crf.to_string());
        argv.push("-pix_fmt".into());
        argv.push("yuv420p".into());
        if p.keep_audio {
            argv.push("-c:a".into());
            argv.push("aac".into());
        }
    }
    if !p.keep_audio {
        argv.push("-an".into());
    }
    argv.push("-movflags".into());
    argv.push("+faststart".into());
    argv.push(out_name.clone());
    (argv, out_name)
}

/// Render the scene table as CSV — the same shape a `list-scenes` export has, so
/// the timings can go straight into a spreadsheet or an edit list.
pub fn scenes_csv(scenes: &[Scene], stem: &str, ext: &str) -> String {
    let mut out = String::from("scene,start_seconds,end_seconds,duration_seconds,filename\n");
    for s in scenes {
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            s.index,
            fmt_num(s.start),
            fmt_num(s.end),
            fmt_num(s.duration()),
            s.entry_name(stem, ext)
        ));
    }
    out
}

/// One-line human summary of a completed split.
pub fn summary(scenes: &[Scene], p: &Params, in_filename: &str) -> String {
    let how = if p.is_copy() {
        "stream copy".to_string()
    } else {
        format!("H.264 CRF {} {}", p.crf, p.preset)
    };
    let audio = if p.keep_audio { "audio kept" } else { "audio dropped" };
    format!(
        "Split {in_filename} into {} scene(s) at threshold {} (min scene {}s, {how}, {audio}).",
        scenes.len(),
        fmt_num(p.threshold),
        fmt_num(p.min_scene),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "\
  Duration: 00:00:03.00, start: 0.000000, bitrate: 80 kb/s
  Stream #0:0[0x1]: Video: h264, yuv420p, 128x128, 10 fps
[Parsed_showinfo_1 @ 0x55a] n:   0 pts:  10240 pts_time:1       duration_time:0.1 fmt:yuv420p s:128x128
[Parsed_showinfo_1 @ 0x55a] n:   1 pts:  20480 pts_time:2       duration_time:0.1 fmt:yuv420p s:128x128
";

    #[test]
    fn detect_argv_runs_scene_filter_with_no_output_file() {
        let argv = detect_argv("in.mp4", 0.3);
        assert_eq!(
            argv,
            vec![
                "-hide_banner", "-nostats", "-i", "in.mp4", "-an", "-sn", "-map", "0:v:0",
                "-filter:v", "select='gt(scene\\,0.3)',showinfo", "-f", "null", "-"
            ]
        );
    }

    #[test]
    fn parses_cuts_and_duration_from_a_real_log() {
        assert_eq!(parse_cuts(LOG), vec![1.0, 2.0]);
        assert_eq!(parse_duration(LOG), Some(3.0));
    }

    #[test]
    fn ignores_pts_time_outside_showinfo_lines() {
        let log = "frame= 10 pts_time:9.5 speed=1x\n";
        assert!(parse_cuts(log).is_empty());
    }

    #[test]
    fn duration_is_none_when_the_banner_is_missing_or_na() {
        assert_eq!(parse_duration("no banner here"), None);
        assert_eq!(parse_duration("  Duration: N/A, start: 0.0"), None);
    }

    /// Happy path: two cuts in a 3 s clip → three 1 s scenes with the expected
    /// windows, filenames, and CSV.
    #[test]
    fn builds_three_scenes_from_two_cuts() {
        let cuts = apply_min_scene(&parse_cuts(LOG), 0.6);
        let scenes = build_scenes(&cuts, parse_duration(LOG).unwrap(), 0.6).unwrap();
        assert_eq!(scenes.len(), 3);
        assert_eq!((scenes[0].start, scenes[0].end), (0.0, 1.0));
        assert_eq!((scenes[1].start, scenes[1].end), (1.0, 2.0));
        assert_eq!((scenes[2].start, scenes[2].end), (2.0, 3.0));
        assert_eq!(scenes[2].entry_name("clip", "mp4"), "clip-Scene-003.mp4");
        assert_eq!(scenes[0].out_name("mp4"), "scene-001.mp4");
        assert_eq!(
            scenes_csv(&scenes, "clip", "mp4"),
            "scene,start_seconds,end_seconds,duration_seconds,filename\n\
             1,0,1,1,clip-Scene-001.mp4\n\
             2,1,2,1,clip-Scene-002.mp4\n\
             3,2,3,1,clip-Scene-003.mp4\n"
        );
    }

    #[test]
    fn min_scene_merges_close_cuts_and_a_short_first_scene() {
        // 0.2 is inside the first 0.6 s, 1.05 is inside 1.0's window.
        let cuts = apply_min_scene(&[0.2, 1.0, 1.05, 2.0], 0.6);
        assert_eq!(cuts, vec![1.0, 2.0]);
        // min_scene 0 keeps everything.
        assert_eq!(apply_min_scene(&[0.2, 1.0], 0.0), vec![0.2, 1.0]);
    }

    #[test]
    fn a_too_short_tail_scene_is_folded_into_the_previous_one() {
        let scenes = build_scenes(&[1.0, 2.9], 3.0, 0.6).unwrap();
        assert_eq!(scenes.len(), 2);
        assert_eq!((scenes[1].start, scenes[1].end), (1.0, 3.0));
    }

    #[test]
    fn no_cuts_yields_one_whole_file_scene() {
        let scenes = build_scenes(&[], 3.0, 0.6).unwrap();
        assert_eq!(scenes.len(), 1);
        assert_eq!((scenes[0].start, scenes[0].end), (0.0, 3.0));
    }

    /// Error path: an unreadable duration and the clip cap both fail loudly.
    #[test]
    fn build_scenes_rejects_bad_duration_and_the_scene_cap() {
        let err = build_scenes(&[1.0], 0.0, 0.6).unwrap_err();
        assert!(err.contains("duration"), "{err}");
        let many: Vec<f64> = (1..=MAX_SCENES).map(|i| i as f64 * 0.01).collect();
        let err = build_scenes(&many, 100.0, 0.0).unwrap_err();
        assert!(err.contains("over the 200-clip limit"), "{err}");
    }

    #[test]
    fn validate_rejects_every_out_of_range_knob() {
        let bad_threshold = Params { threshold: 1.5, ..Params::default() };
        assert!(validate(bad_threshold).unwrap_err().contains("threshold"));
        let bad_mode = Params { mode: "fast".into(), ..Params::default() };
        assert!(validate(bad_mode).unwrap_err().contains("mode must be one of"));
        let bad_crf = Params { crf: 99, ..Params::default() };
        assert!(validate(bad_crf).unwrap_err().contains("crf"));
        let bad_preset = Params { preset: "turbo".into(), ..Params::default() };
        assert!(validate(bad_preset).unwrap_err().contains("preset"));
        let bad_min = Params { min_scene: -1.0, ..Params::default() };
        assert!(validate(bad_min).unwrap_err().contains("min_scene"));
        assert!(validate(Params::default()).is_ok());
    }

    #[test]
    fn reencode_argv_seeks_and_limits_length() {
        let p = Params::default();
        let scene = Scene { index: 2, start: 1.0, end: 2.0 };
        let (argv, out) = clip_argv("in.mp4", &scene, false, &p, "mp4");
        assert_eq!(out, "scene-002.mp4");
        assert_eq!(
            argv,
            vec![
                "-hide_banner", "-nostats", "-ss", "1", "-i", "in.mp4", "-t", "1", "-c:v",
                "libx264", "-preset", "veryfast", "-crf", "22", "-pix_fmt", "yuv420p", "-c:a",
                "aac", "-movflags", "+faststart", "scene-002.mp4"
            ]
        );
    }

    #[test]
    fn the_last_scene_runs_to_eof_without_t() {
        let p = Params::default();
        let scene = Scene { index: 3, start: 2.0, end: 3.0 };
        let (argv, _) = clip_argv("in.mp4", &scene, true, &p, "mp4");
        assert!(!argv.iter().any(|a| a == "-t"), "{argv:?}");
    }

    #[test]
    fn copy_mode_remuxes_keeps_the_container_and_can_drop_audio() {
        let p = Params { mode: "copy".into(), keep_audio: false, ..Params::default() };
        let ext = clip_ext("webm", &p.mode);
        assert_eq!(ext, "webm");
        let scene = Scene { index: 1, start: 0.0, end: 1.5 };
        let (argv, out) = clip_argv("in.webm", &scene, false, &p, ext);
        assert_eq!(out, "scene-001.webm");
        assert!(argv.windows(2).any(|w| w == ["-c", "copy"]), "{argv:?}");
        assert!(argv.iter().any(|a| a == "-an"), "{argv:?}");
        assert!(!argv.iter().any(|a| a == "libx264"), "{argv:?}");
        assert_eq!(clip_ext("webm", "reencode"), "mp4");
    }

    #[test]
    fn stems_are_sanitized_for_clip_names() {
        assert_eq!(safe_stem("My Holiday (2024).mp4"), "My_Holiday__2024");
        assert_eq!(safe_stem("/tmp/a/b.mov"), "b");
        assert_eq!(safe_stem("...."), "video");
        assert_eq!(safe_stem(""), "video");
    }

    #[test]
    fn summary_names_the_settings_that_produced_the_split() {
        let scenes = build_scenes(&[1.0, 2.0], 3.0, 0.6).unwrap();
        assert_eq!(
            summary(&scenes, &Params::default(), "clip.mp4"),
            "Split clip.mp4 into 3 scene(s) at threshold 0.3 (min scene 0.6s, H.264 CRF 22 veryfast, audio kept)."
        );
    }
}
