//! gizza-ai/video-scene-cut-diff core — pure scene-cut detection helpers shared
//! by the chat/CLI block. No wafer/wasm/ffmpeg-host deps: this crate only builds
//! the ffmpeg argv, parses the resulting log, and diffs the two timestamp lists.
//!
//! Pipeline (the block drives ffmpeg, this crate does the pure parts):
//! 1. `detect_argv` → an ffmpeg command that runs the `scene` detector over one
//!    video (`select='gt(scene,threshold)',showinfo`) and prints one `showinfo`
//!    line per flagged frame to the log — no output file is written.
//! 2. `parse_cuts` reads the `pts_time:` values out of that log = the scene-cut
//!    timestamps (seconds).
//! 3. `apply_min_scene` drops cuts closer together than `min_scene` seconds (a
//!    de-bounce so one hard cut isn't reported as several).
//! 4. `diff_cuts` matches the two edits' cut lists within `tolerance` seconds and
//!    classifies every cut as added, removed, moved, or unchanged.
//!
//! Pure Rust → runs on every backend, including the chat Service Worker (the
//! ffmpeg exec itself is dispatched to gizza-ai/ffmpeg-runtime by the block).

/// A matched cut whose timestamp shifted more than `SAME_EPSILON` between edits.
#[derive(Debug, Clone, PartialEq)]
pub struct MovedCut {
    /// Timestamp in edit 1 (seconds).
    pub from: f64,
    /// Timestamp in edit 2 (seconds).
    pub to: f64,
    /// `to - from` (seconds): positive = later in edit 2, negative = earlier.
    pub shift: f64,
}

/// The full comparison of two edits' scene-cut timestamps.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CutDiff {
    /// Scene-cut timestamps detected in edit 1 (seconds, sorted).
    pub cuts_a: Vec<f64>,
    /// Scene-cut timestamps detected in edit 2 (seconds, sorted).
    pub cuts_b: Vec<f64>,
    /// Cuts present in edit 2 but not edit 1 (edit-2 timestamps).
    pub added: Vec<f64>,
    /// Cuts present in edit 1 but not edit 2 (edit-1 timestamps).
    pub removed: Vec<f64>,
    /// Cuts present in both but shifted by more than `SAME_EPSILON`.
    pub moved: Vec<MovedCut>,
    /// Cuts present in both at the same time (within `SAME_EPSILON`) — edit-1 timestamps.
    pub unchanged: Vec<f64>,
}

/// A matched pair whose absolute shift is at or below this many seconds is treated
/// as the SAME cut in the same place (roughly one frame at 24–30 fps); above it,
/// the cut is reported as "moved". Kept below the smallest sensible `tolerance`.
pub const SAME_EPSILON: f64 = 0.04;

/// Hard cap on detected cuts per video — a pathologically low `threshold` can flag
/// nearly every frame; past this we refuse rather than emit a huge payload.
pub const MAX_CUTS: usize = 20_000;

/// Round to two decimals (10 ms) for the JSON payload / summaries.
pub fn round2(v: f64) -> f64 {
    if v.is_finite() {
        (v * 100.0).round() / 100.0
    } else {
        v
    }
}

/// Build the ffmpeg argv that detects scene cuts in `in_name`. Runs the `scene`
/// filter through `showinfo` and discards the video (`-f null -`); the flagged
/// frames' `pts_time` values land in the log, which `parse_cuts` reads.
///
/// `threshold` is ffmpeg's scene score cutoff (0.0–1.0): a frame is a scene cut
/// when its difference from the previous frame exceeds it. Lower = more cuts.
pub fn detect_argv(in_name: &str, threshold: f64) -> Vec<String> {
    let vf = format!("select='gt(scene\\,{threshold})',showinfo");
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

/// Extract scene-cut timestamps (seconds) from an ffmpeg `showinfo` log. Reads the
/// `pts_time:` field of every `showinfo`-filter line, returning a sorted, de-duplicated
/// list. Only lines emitted by the filter instance itself (tagged `[Parsed_showinfo_N
/// @ 0x…]`) are read; other `pts_time` mentions (e.g. input stream banners) are ignored.
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
                if t.is_finite() && t >= 0.0 {
                    cuts.push(round2(t));
                }
            }
        }
    }
    cuts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    cuts.dedup();
    cuts
}

/// Drop cuts that fall within `min_scene` seconds of the previously kept cut, so a
/// single transition detected across a couple of frames counts once. `min_scene`
/// <= 0 keeps every cut. Input is assumed sorted ascending.
pub fn apply_min_scene(cuts: &[f64], min_scene: f64) -> Vec<f64> {
    if min_scene <= 0.0 {
        return cuts.to_vec();
    }
    let mut kept: Vec<f64> = Vec::new();
    for &t in cuts {
        match kept.last() {
            Some(&last) if t - last < min_scene => {}
            _ => kept.push(t),
        }
    }
    kept
}

/// Match two sorted cut lists within `tolerance` seconds (two-pointer nearest-in-
/// order) and classify each cut. A matched pair shifted by more than `SAME_EPSILON`
/// is "moved"; within it, "unchanged". Unmatched edit-1 cuts are "removed",
/// unmatched edit-2 cuts are "added".
pub fn diff_cuts(a: &[f64], b: &[f64], tolerance: f64) -> CutDiff {
    let tol = tolerance.max(0.0);
    let mut diff = CutDiff {
        cuts_a: a.to_vec(),
        cuts_b: b.to_vec(),
        ..Default::default()
    };
    let (mut i, mut j) = (0usize, 0usize);
    while i < a.len() && j < b.len() {
        let (ta, tb) = (a[i], b[j]);
        if (ta - tb).abs() <= tol {
            let shift = round2(tb - ta);
            if shift.abs() <= SAME_EPSILON {
                diff.unchanged.push(ta);
            } else {
                diff.moved.push(MovedCut {
                    from: ta,
                    to: tb,
                    shift,
                });
            }
            i += 1;
            j += 1;
        } else if ta < tb {
            diff.removed.push(ta);
            i += 1;
        } else {
            diff.added.push(tb);
            j += 1;
        }
    }
    while i < a.len() {
        diff.removed.push(a[i]);
        i += 1;
    }
    while j < b.len() {
        diff.added.push(b[j]);
        j += 1;
    }
    diff
}

impl CutDiff {
    /// One-line human summary of the comparison.
    pub fn summary(&self) -> String {
        format!(
            "Edit 2 vs edit 1: {} cut(s) in edit 1, {} in edit 2 — {} added, {} removed, {} moved, {} unchanged.",
            self.cuts_a.len(),
            self.cuts_b.len(),
            self.added.len(),
            self.removed.len(),
            self.moved.len(),
            self.unchanged.len(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_argv_has_scene_select_and_showinfo() {
        let argv = detect_argv("in.mp4", 0.3);
        let joined = argv.join(" ");
        assert!(joined.contains("gt(scene"), "{joined}");
        assert!(joined.contains("0.3"), "{joined}");
        assert!(joined.contains("showinfo"), "{joined}");
        assert!(joined.contains("-f null"), "{joined}");
        // Never writes an output file.
        assert!(argv.last().map(|s| s.as_str()) == Some("-"));
    }

    #[test]
    fn parse_cuts_reads_showinfo_pts_time() {
        let log = "\
[Parsed_showinfo_1 @ 0x55] n:0 pts:30720 pts_time:1.28 pos:1 fmt:yuv420p
Input #0, mov, from 'in.mp4': pts_time:0.00 (should be ignored — no showinfo)
[Parsed_showinfo_1 @ 0x55] n:1 pts:122880 pts_time:5.12 pos:2 fmt:yuv420p
[Parsed_showinfo_1 @ 0x55] n:2 pts:245760 pts_time:10.24 pos:3 fmt:yuv420p";
        let cuts = parse_cuts(log);
        assert_eq!(cuts, vec![1.28, 5.12, 10.24]);
    }

    #[test]
    fn parse_cuts_sorts_and_dedups() {
        let log = "\
[Parsed_showinfo_1 @ 0x] pts_time:5.00 x
[Parsed_showinfo_1 @ 0x] pts_time:1.00 x
[Parsed_showinfo_1 @ 0x] pts_time:5.00 x";
        assert_eq!(parse_cuts(log), vec![1.0, 5.0]);
    }

    #[test]
    fn min_scene_debounces_close_cuts() {
        let cuts = vec![1.0, 1.2, 1.3, 5.0, 5.4];
        // With a 0.5 s minimum, 1.2/1.3 collapse into 1.0 and 5.4 into 5.0.
        assert_eq!(apply_min_scene(&cuts, 0.5), vec![1.0, 5.0]);
        // Zero keeps everything.
        assert_eq!(apply_min_scene(&cuts, 0.0), cuts);
    }

    #[test]
    fn diff_identical_edits_are_all_unchanged() {
        let a = vec![1.0, 5.0, 10.0];
        let d = diff_cuts(&a, &a, 0.5);
        assert_eq!(d.unchanged, a);
        assert!(d.added.is_empty());
        assert!(d.removed.is_empty());
        assert!(d.moved.is_empty());
    }

    #[test]
    fn diff_classifies_added_removed_moved() {
        // edit 1: cuts at 1, 5, 10
        // edit 2: cuts at 1 (same), 5.3 (moved +0.3), 12 (10 removed, 12 added)
        let a = vec![1.0, 5.0, 10.0];
        let b = vec![1.0, 5.3, 12.0];
        let d = diff_cuts(&a, &b, 0.5);
        assert_eq!(d.unchanged, vec![1.0]);
        assert_eq!(d.moved.len(), 1);
        assert_eq!(d.moved[0].from, 5.0);
        assert_eq!(d.moved[0].to, 5.3);
        assert!((d.moved[0].shift - 0.3).abs() < 1e-9);
        assert_eq!(d.removed, vec![10.0]);
        assert_eq!(d.added, vec![12.0]);
    }

    #[test]
    fn diff_tiny_shift_within_epsilon_is_unchanged_not_moved() {
        let a = vec![2.0];
        let b = vec![2.02]; // 20 ms < SAME_EPSILON
        let d = diff_cuts(&a, &b, 0.5);
        assert_eq!(d.unchanged, vec![2.0]);
        assert!(d.moved.is_empty());
    }

    #[test]
    fn diff_empty_second_edit_removes_all() {
        let a = vec![1.0, 2.0];
        let d = diff_cuts(&a, &[], 0.5);
        assert_eq!(d.removed, a);
        assert!(d.added.is_empty());
    }

    #[test]
    fn summary_counts_every_bucket() {
        let a = vec![1.0, 5.0, 10.0];
        let b = vec![1.0, 5.3, 12.0];
        let s = diff_cuts(&a, &b, 0.5).summary();
        assert!(s.contains("3 cut(s) in edit 1"), "{s}");
        assert!(s.contains("1 added"), "{s}");
        assert!(s.contains("1 removed"), "{s}");
        assert!(s.contains("1 moved"), "{s}");
        assert!(s.contains("1 unchanged"), "{s}");
    }
}
