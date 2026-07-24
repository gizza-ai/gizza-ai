//! gizza-ai/seamless-loop-video core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Makes a clip loop seamlessly by crossfading (alpha-overlaying) its tail back
//! into its head. The output is ONE clip `crossfade` seconds SHORTER whose first
//! frame equals the source frame at `D - crossfade`, so repeating it reads as
//! continuous motion — the loop join becomes invisible.
//!
//! The filtergraph is a straight alpha crossfade via `overlay` (not `xfade`,
//! which needs a constant frame rate the trim/setpts branches don't satisfy):
//!
//! ```text
//! [0:v]split[s1][s2];
//! [s1]reverse,trim=start=X,setpts=PTS-STARTPTS,reverse[base];   # source[0, D-X]
//! [s2]reverse,trim=end=X,setpts=PTS-STARTPTS,reverse,           # tail source[D-X, D]
//!     format=yuva420p,fade=t=out:st=0:d=X:alpha=1[tail];        # tail fades out over first X
//! [base][tail]overlay=eof_action=pass[v]                        # first X = crossfade tail->head
//! ```
//!
//! Probe-free by design: the graph never learns the clip duration `D`. The
//! `reverse` + front/back `trim` locate the clip END (drop the last / first
//! `crossfade` seconds) without needing `D`, so it behaves identically on the
//! page (@ffmpeg/core) and the CLI. Cost: the clip is buffered to reverse it, so
//! this suits SHORT clips — very long / high-resolution inputs can exhaust
//! browser memory. Output is silent (there is no audio crossfade — see the
//! competitor analysis).

/// Default crossfade (overlap) length in seconds — a subtle, standard blend.
pub const DEFAULT_CROSSFADE: f64 = 0.5;
/// Minimum crossfade the page slider offers; below it the join is a hard cut.
pub const MIN_CROSSFADE: f64 = 0.1;
/// Maximum crossfade — long overlaps eat most of a short clip and stress browser
/// memory, so the tool caps here.
pub const MAX_CROSSFADE: f64 = 5.0;

/// Default encode quality (≈ CRF 23 — a good size/quality balance).
pub const DEFAULT_QUALITY: u8 = 75;
/// Lowest CRF the quality knob maps to (`quality = 100`) — "visually lossless"
/// for libx264; deliberately not 0 (true-lossless blows past output caps).
pub const MIN_CRF: f32 = 18.0;
/// Highest CRF the quality knob maps to (`quality = 1`) — small, low quality.
pub const MAX_CRF: f32 = 40.0;

/// Map web-conventional quality 1-100 (high → better) to a practical libx264 CRF
/// (low → better): `100` → CRF 18, `1` → CRF 40, default 75 ≈ CRF 23.
pub fn quality_to_crf(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let crf = MAX_CRF - (q - 1.0) * (MAX_CRF - MIN_CRF) / 99.0;
    crf.round().clamp(MIN_CRF, MAX_CRF) as u8
}

/// The seamless-loop crossfade filtergraph (see the module docs). `x` is the
/// crossfade length in seconds: `base` = source[0, D-x], `tail` = source[D-x, D]
/// alpha-faded out over its own length and overlaid onto base's first `x`
/// seconds. Output length = D - x; output frame 0 == source frame at D-x.
pub fn filter_complex(x: f64) -> String {
    format!(
        "[0:v]split[s1][s2];\
         [s1]reverse,trim=start={x},setpts=PTS-STARTPTS,reverse[base];\
         [s2]reverse,trim=end={x},setpts=PTS-STARTPTS,reverse,format=yuva420p,fade=t=out:st=0:d={x}:alpha=1[tail];\
         [base][tail]overlay=eof_action=pass[v]"
    )
}

/// Build the ffmpeg argv (no leading `ffmpeg`): apply the crossfade filtergraph,
/// map only the composed video (`-an`, the output is silent), and re-encode to
/// universally-playable H.264 / yuv420p MP4 with `+faststart`.
pub fn build_argv(in_name: &str, out_name: &str, crossfade: f64, crf: u8) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-filter_complex".into(),
        filter_complex(crossfade),
        "-map".into(),
        "[v]".into(),
        "-an".into(),
        "-c:v".into(),
        "libx264".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-crf".into(),
        crf.to_string(),
        "-preset".into(),
        "medium".into(),
        "-movflags".into(),
        "+faststart".into(),
        out_name.into(),
    ]
}

/// Validate `crossfade` (finite, > 0, ≤ [`MAX_CROSSFADE`]) and `quality` (1-100),
/// then build `(argv, out_name)` for an input file. The output is ALWAYS
/// re-encoded H.264 MP4, so `out_name` is always `out.mp4`. Single source shared
/// by the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan(crossfade: f64, quality: u8, in_name: &str) -> Result<(Vec<String>, String), String> {
    if !crossfade.is_finite() || crossfade <= 0.0 {
        return Err(format!(
            "crossfade must be > 0 and finite, got {crossfade}"
        ));
    }
    if crossfade > MAX_CROSSFADE {
        return Err(format!(
            "crossfade must be <= {MAX_CROSSFADE}s (long overlaps consume most of a short clip), got {crossfade}"
        ));
    }
    if !(1..=100).contains(&quality) {
        return Err(format!("quality must be 1-100, got {quality}"));
    }
    let crf = quality_to_crf(quality);
    let out_name = "out.mp4".to_string();
    Ok((build_argv(in_name, &out_name, crossfade, crf), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_builds_crossfade_graph_and_reencodes_to_mp4() {
        let (argv, out) = plan(DEFAULT_CROSSFADE, DEFAULT_QUALITY, "in.webm").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(argv.first().map(String::as_str), Some("-i"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
        // Silent, H.264/yuv420p/faststart re-encode invariants.
        assert!(argv.iter().any(|a| a == "-an"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(argv.windows(2).any(|w| w[0] == "-pix_fmt" && w[1] == "yuv420p"));
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "[v]"));
        assert!(argv.windows(2).any(|w| w[0] == "-movflags" && w[1] == "+faststart"));
    }

    #[test]
    fn filtergraph_has_the_expected_stages() {
        let g = filter_complex(0.5);
        assert!(g.contains("[0:v]split[s1][s2]"));
        assert!(g.contains("reverse,trim=start=0.5"));
        assert!(g.contains("reverse,trim=end=0.5"));
        assert!(g.contains("format=yuva420p"));
        assert!(g.contains("fade=t=out:st=0:d=0.5:alpha=1[tail]"));
        assert!(g.contains("[base][tail]overlay=eof_action=pass[v]"));
    }

    #[test]
    fn crossfade_value_flows_into_every_branch() {
        // A single crossfade value drives both trims and the fade — a mismatch
        // would make output frame 0 not equal the loop point.
        let g = filter_complex(1.25);
        assert_eq!(g.matches("1.25").count(), 3);
    }

    #[test]
    fn quality_to_crf_endpoints_and_default() {
        assert_eq!(quality_to_crf(100), 18);
        assert_eq!(quality_to_crf(1), 40);
        let crf = quality_to_crf(DEFAULT_QUALITY);
        assert!((22..=24).contains(&crf), "expected CRF 22-24, got {crf}");
    }

    #[test]
    fn plan_maps_quality_into_crf_arg() {
        let (argv, _) = plan(0.5, 100, "in.mp4").unwrap();
        let i = argv.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(argv[i + 1], "18");
    }

    #[test]
    fn plan_rejects_nonpositive_or_nonfinite_crossfade() {
        assert!(plan(0.0, 75, "in.mp4").is_err());
        assert!(plan(-1.0, 75, "in.mp4").is_err());
        assert!(plan(f64::NAN, 75, "in.mp4").is_err());
        assert!(plan(f64::INFINITY, 75, "in.mp4").is_err());
    }

    #[test]
    fn plan_rejects_crossfade_over_cap() {
        assert!(plan(MAX_CROSSFADE + 0.01, 75, "in.mp4").is_err());
        assert!(plan(MAX_CROSSFADE, 75, "in.mp4").is_ok());
    }

    #[test]
    fn plan_rejects_out_of_range_quality() {
        assert!(plan(0.5, 0, "in.mp4").is_err());
        assert!(plan(0.5, 101, "in.mp4").is_err());
    }
}
