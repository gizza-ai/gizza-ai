//! gizza-ai/gif-frame-deduplicator core — pure ffmpeg argv construction shared
//! by the chat block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Animated GIFs exported from screen recorders, slideshows and animation tools
//! hold the same picture for many frames in a row. Every repeat is a full extra
//! frame in the file even though nothing changed on screen. This tool drops the
//! near-duplicate *consecutive* frames with ffmpeg's `mpdecimate` filter and
//! re-encodes the GIF with a clip-tuned palette.
//!
//! `mpdecimate` compares each frame against the previous kept one in 8×8 blocks
//! and calls it a duplicate when no block differs by more than `hi` **and**
//! fewer than `frac` of the blocks differ by more than `lo`. Both thresholds are
//! sums of absolute differences over a block, so a block's maximum is
//! `64 px × 255` = 16320.
//!
//! The single user-facing knob is a **similarity threshold** in percent: at
//! `threshold = 98`, frames that differ by less than 2% of that maximum are
//! duplicates (`hi = 16320 × 0.02 ≈ 326`). `lo` keeps ffmpeg's own 12:5 ratio to
//! `hi`, and `frac` stays at ffmpeg's default 0.33. So 100 removes only frames
//! that are essentially identical, 98 (the default) also absorbs dithering
//! noise and compression shimmer, and lower values remove frames that merely
//! *look* alike.
//!
//! **Marking a frame is not dropping it.** ffmpeg re-inserts decimated frames to
//! hold a constant frame rate unless the frame-rate mode says otherwise — the
//! classic "mpdecimate did nothing" trap. `-fps_mode vfr` is what makes the
//! duplicates really disappear, and it also leaves every kept frame at its
//! original timestamp, so the animation still runs for the same wall-clock time
//! (the kept frame simply holds longer where the duplicates were).
//!
//! Re-encoding a GIF means re-quantizing it, so the graph splits the decimated
//! stream, builds a palette from one branch (`palettegen=stats_mode=diff`, which
//! weights the pixels that actually change between frames) and applies it to the
//! other (`paletteuse`) — far better than the encoder's default fixed palette.

/// Output filename — always a GIF.
pub const OUT_NAME: &str = "optimized.gif";

/// Smallest accepted similarity threshold (percent). 0 treats every frame as a
/// duplicate of its predecessor, collapsing the GIF to a single frame.
pub const MIN_THRESHOLD: f64 = 0.0;
/// Largest accepted similarity threshold (percent). 100 keeps every frame that
/// is not essentially identical to the previous one.
pub const MAX_THRESHOLD: f64 = 100.0;
/// Threshold used when the caller leaves it unset.
pub const DEFAULT_THRESHOLD: f64 = 98.0;

/// The largest sum-of-absolute-differences an 8×8 `mpdecimate` block can reach:
/// 64 pixels × the 0-255 channel range.
pub const MAX_BLOCK_DIFF: f64 = 64.0 * 255.0;
/// ffmpeg's `hi`:`lo` ratio (defaults `64*12` : `64*5`), preserved across the
/// whole threshold range so `lo` stays meaningful.
pub const LO_OVER_HI: f64 = 5.0 / 12.0;
/// ffmpeg's default `frac` — the fraction of 8×8 blocks that must exceed `lo`
/// for a frame to count as *changed*. Kept fixed so the tool has one knob.
pub const FRAC: &str = "0.33";

/// Resolve a possibly-unset / non-finite threshold request: non-finite falls
/// back to [`DEFAULT_THRESHOLD`], anything else is clamped to 0-100.
pub fn resolve_threshold(threshold_percent: f64) -> f64 {
    if !threshold_percent.is_finite() {
        return DEFAULT_THRESHOLD;
    }
    threshold_percent.clamp(MIN_THRESHOLD, MAX_THRESHOLD)
}

/// Map a 0-100 similarity threshold onto `mpdecimate`'s `(hi, lo)`.
///
/// `hi` is the share of a block's maximum difference that still counts as "the
/// same picture": `hi = 16320 × (100 - threshold)/100`. `lo` keeps ffmpeg's 5/12
/// ratio. Both are floored at 1 — a 0 threshold would make the filter treat
/// every frame as changed, which is the opposite of what `threshold = 100`
/// means.
pub fn threshold_to_mpdecimate(threshold_percent: f64) -> (i64, i64) {
    let diff = (100.0 - resolve_threshold(threshold_percent)) / 100.0;
    let hi = (MAX_BLOCK_DIFF * diff).round().max(1.0) as i64;
    let lo = ((hi as f64) * LO_OVER_HI).round().max(1.0) as i64;
    (hi, lo)
}

/// Build the `-filter_complex` graph: decimate first, then split into a
/// palette-generating branch and a palette-applying branch (one decode pass).
pub fn build_filter(threshold_percent: f64) -> String {
    let (hi, lo) = threshold_to_mpdecimate(threshold_percent);
    format!(
        "[0:v]mpdecimate=hi={hi}:lo={lo}:frac={FRAC},split[s0][s1];\
         [s0]palettegen=stats_mode=diff[p];\
         [s1][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle"
    )
}

/// Build the ffmpeg argv (no leading `ffmpeg`).
///
/// `-fps_mode vfr` is load-bearing: without it ffmpeg re-inserts every frame
/// `mpdecimate` marked, and the output is the same size as the input. With it
/// the duplicates are gone and the kept frames stay at their original
/// timestamps, so the GIF still plays for the same length of time. `-loop 0` is
/// the GIF convention for "repeat forever".
pub fn build_argv(in_name: &str, threshold_percent: f64) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-filter_complex".into(),
        build_filter(threshold_percent),
        "-fps_mode".into(),
        "vfr".into(),
        "-loop".into(),
        "0".into(),
        OUT_NAME.into(),
    ]
}

/// True when `name` has a `.gif` extension (case-insensitive).
pub fn has_gif_ext(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("gif"))
}

/// Validate the request and return `(argv, out_name)`.
///
/// The input must be a GIF (the tool rewrites GIF frame timing, so any other
/// container is a user error worth naming), and `threshold_percent` must be a
/// finite 0-100 percentage. Shared by the chat block and the page.
pub fn plan(in_name: &str, threshold_percent: f64) -> Result<(Vec<String>, String), String> {
    if !has_gif_ext(in_name) {
        return Err(format!(
            "input must be an animated GIF (.gif), got {in_name:?}"
        ));
    }
    if !threshold_percent.is_finite() {
        return Err("threshold must be a finite number".to_string());
    }
    if threshold_percent < MIN_THRESHOLD || threshold_percent > MAX_THRESHOLD {
        return Err(format!(
            "threshold must be between {MIN_THRESHOLD} and {MAX_THRESHOLD} percent (got {threshold_percent})"
        ));
    }
    Ok((build_argv(in_name, threshold_percent), OUT_NAME.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg_after<'a>(argv: &'a [String], flag: &str) -> Option<&'a str> {
        argv.iter()
            .position(|a| a == flag)
            .and_then(|i| argv.get(i + 1))
            .map(String::as_str)
    }

    #[test]
    fn default_threshold_maps_to_two_percent_of_a_block() {
        // 16320 × 0.02 = 326.4 → 326; lo = 326 × 5/12 = 135.8 → 136.
        assert_eq!(threshold_to_mpdecimate(DEFAULT_THRESHOLD), (326, 136));
    }

    #[test]
    fn higher_threshold_is_stricter() {
        let (hi_98, _) = threshold_to_mpdecimate(98.0);
        let (hi_90, _) = threshold_to_mpdecimate(90.0);
        assert!(hi_90 > hi_98, "lower threshold must decimate more");
        // 100 = only essentially-identical frames; never a 0 threshold.
        assert_eq!(threshold_to_mpdecimate(100.0), (1, 1));
        // 0 = everything is a duplicate.
        assert_eq!(threshold_to_mpdecimate(0.0), (16320, 6800));
    }

    #[test]
    fn threshold_resolves_and_clamps() {
        assert_eq!(resolve_threshold(f64::NAN), DEFAULT_THRESHOLD);
        assert_eq!(resolve_threshold(f64::INFINITY), DEFAULT_THRESHOLD);
        assert_eq!(resolve_threshold(-5.0), MIN_THRESHOLD);
        assert_eq!(resolve_threshold(500.0), MAX_THRESHOLD);
        assert_eq!(resolve_threshold(95.5), 95.5);
    }

    #[test]
    fn filter_decimates_then_builds_and_applies_a_palette() {
        let f = build_filter(DEFAULT_THRESHOLD);
        assert_eq!(
            f,
            "[0:v]mpdecimate=hi=326:lo=136:frac=0.33,split[s0][s1];\
             [s0]palettegen=stats_mode=diff[p];\
             [s1][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle"
        );
        // Order matters: decimate BEFORE palettegen, so the palette is built
        // from the frames that actually survive.
        let dec = f.find("mpdecimate").unwrap();
        assert!(dec < f.find("palettegen").unwrap());
        assert!(f.find("palettegen").unwrap() < f.find("paletteuse").unwrap());
    }

    #[test]
    fn argv_sets_vfr_so_the_duplicates_are_really_dropped() {
        let argv = build_argv("in.gif", DEFAULT_THRESHOLD);
        assert_eq!(arg_after(&argv, "-fps_mode"), Some("vfr"));
        // No setpts: keeping the original timestamps keeps the play length.
        assert!(!arg_after(&argv, "-filter_complex")
            .unwrap()
            .contains("setpts"));
    }

    #[test]
    fn full_default_argv() {
        let argv = build_argv("in.gif", DEFAULT_THRESHOLD);
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.gif",
                "-filter_complex",
                "[0:v]mpdecimate=hi=326:lo=136:frac=0.33,split[s0][s1];\
                 [s0]palettegen=stats_mode=diff[p];\
                 [s1][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
                "-fps_mode",
                "vfr",
                "-loop",
                "0",
                "optimized.gif",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn plan_returns_gif_out_name() {
        let (argv, out) = plan("in.gif", 95.0).unwrap();
        assert_eq!(out, "optimized.gif");
        assert_eq!(argv.last().map(String::as_str), Some("optimized.gif"));
        assert_eq!(
            arg_after(&argv, "-filter_complex"),
            Some(build_filter(95.0).as_str())
        );
    }

    #[test]
    fn plan_requires_a_gif_input() {
        assert!(plan("in.gif", 98.0).is_ok());
        assert!(plan("CLIP.GIF", 98.0).is_ok());
        for bad in ["in.png", "in.mp4", "in.webp", "gif", "in"] {
            assert!(plan(bad, 98.0).is_err(), "{bad} must be rejected");
        }
    }

    #[test]
    fn plan_rejects_out_of_range_and_non_finite_thresholds() {
        assert!(plan("in.gif", -1.0).is_err());
        assert!(plan("in.gif", 100.5).is_err());
        assert!(plan("in.gif", f64::NAN).is_err());
        assert!(plan("in.gif", f64::INFINITY).is_err());
        // The inclusive ends are valid.
        assert!(plan("in.gif", 0.0).is_ok());
        assert!(plan("in.gif", 100.0).is_ok());
    }

    #[test]
    fn has_gif_ext_only_matches_the_last_segment() {
        assert!(has_gif_ext("a.b.gif"));
        assert!(!has_gif_ext("a.gif.png"));
        assert!(!has_gif_ext(""));
    }
}
