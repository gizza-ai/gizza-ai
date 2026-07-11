//! video-cut-segments core — pure, native-testable logic for a multi-window
//! keep/remove cut. No wafer/wasm-bindgen deps; shared by the chat block + page.
//!
//! # What it does
//!
//! Given ONE video and a typed list of `start-end` time windows, either **keep**
//! only those windows (extract + join, in source order) or **remove** them and
//! keep the rest. Both are a single ffmpeg pass built with the correct
//! `filter_complex` `trim`/`atrim` + `setpts`/`asetpts` + `concat` chain — not
//! the bare `select` filter, which accumulates audio/video desync across
//! multiple sections (see the competitor analysis). Because the `remove` tail
//! segment uses an open-ended `trim=start=X` (runs to EOF), we never need the
//! clip duration, so the single-pass page driver works.
//!
//! This module owns the pure pieces: timestamp parsing, window merge/complement,
//! and argv construction. Output is always re-encoded H.264/AAC mp4 (joining
//! several trimmed segments requires a re-encode).

/// A keep segment. `end == None` means "run to EOF" (the open-ended tail used by
/// the `remove` complement).
pub type Keep = (f64, Option<f64>);

/// Format an `f64` for an ffmpeg arg without a trailing `.0` (`8` not `8.0`,
/// `2.5` stays `2.5`) — compact and locale-independent.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Parse a single timestamp into seconds. Accepts `SS(.mmm)`, `MM:SS(.mmm)`, and
/// `HH:MM:SS(.mmm)`. Each field must be a finite, non-negative number.
pub fn parse_timestamp(s: &str) -> Result<f64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty timestamp".into());
    }
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() > 3 {
        return Err(format!("bad timestamp '{s}' (use SS, MM:SS, or HH:MM:SS)"));
    }
    let mut total = 0.0_f64;
    for (i, p) in parts.iter().enumerate() {
        let val: f64 = p
            .trim()
            .parse()
            .map_err(|_| format!("bad number '{}' in timestamp '{s}'", p.trim()))?;
        if !val.is_finite() || val < 0.0 {
            return Err(format!("timestamp '{s}' must use finite, non-negative numbers"));
        }
        let place = (parts.len() - 1 - i) as i32;
        total += val * 60f64.powi(place);
    }
    Ok(total)
}

/// Parse the segment list. Windows are separated by commas, semicolons, or
/// newlines; each is `start-end` (e.g. `0:05-0:10`). Returns the parsed
/// `(start, end)` windows in input order (validated: `end > start`, both
/// finite). Errors if no window is given.
pub fn parse_segments(input: &str) -> Result<Vec<(f64, f64)>, String> {
    let mut out = Vec::new();
    for tok in input.split(|c| c == ',' || c == ';' || c == '\n' || c == '\r') {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let (a, b) = tok
            .split_once('-')
            .ok_or_else(|| format!("segment '{tok}' must be start-end, e.g. 0:05-0:10"))?;
        let start = parse_timestamp(a)?;
        let end = parse_timestamp(b)?;
        if !(end > start) {
            return Err(format!(
                "segment '{tok}': end ({}) must be after start ({})",
                fmt_num(end),
                fmt_num(start)
            ));
        }
        out.push((start, end));
    }
    if out.is_empty() {
        return Err(
            "no segments given — provide one or more start-end windows, e.g. 0:05-0:10".into(),
        );
    }
    Ok(out)
}

/// Sort by start time and merge overlapping/adjacent windows so a frame is never
/// selected twice (keep) or double-counted (remove).
pub fn merge(mut v: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (s, e) in v {
        if let Some(last) = out.last_mut() {
            if s <= last.1 {
                if e > last.1 {
                    last.1 = e;
                }
                continue;
            }
        }
        out.push((s, e));
    }
    out
}

/// Complement of the (merged, sorted) removal windows within `[0, EOF]`. The
/// final segment is open-ended (`None`) so we never need the clip duration.
pub fn complement(removals: &[(f64, f64)]) -> Vec<Keep> {
    let mut keeps: Vec<Keep> = Vec::new();
    let mut cursor = 0.0_f64;
    for (s, e) in removals {
        if *s > cursor {
            keeps.push((cursor, Some(*s)));
        }
        cursor = cursor.max(*e);
    }
    keeps.push((cursor, None)); // tail to EOF
    keeps
}

/// Build the ffmpeg argv (no leading `ffmpeg`) for the trim+concat keep list.
/// One `trim`/`atrim` pair per kept window, each re-based with
/// `setpts`/`asetpts`, then `concat`-ed into one H.264/AAC mp4.
pub fn build_argv(in_name: &str, out_name: &str, keeps: &[Keep]) -> Vec<String> {
    let mut graph = String::new();
    for (i, (s, e)) in keeps.iter().enumerate() {
        let end = e
            .map(|e| format!(":end={}", fmt_num(e)))
            .unwrap_or_default();
        graph.push_str(&format!(
            "[0:v]trim=start={}{end},setpts=PTS-STARTPTS[v{i}];",
            fmt_num(*s)
        ));
        graph.push_str(&format!(
            "[0:a]atrim=start={}{end},asetpts=PTS-STARTPTS[a{i}];",
            fmt_num(*s)
        ));
    }
    for i in 0..keeps.len() {
        graph.push_str(&format!("[v{i}][a{i}]"));
    }
    graph.push_str(&format!("concat=n={}:v=1:a=1[outv][outa]", keeps.len()));

    vec![
        "-i".into(),
        in_name.into(),
        "-filter_complex".into(),
        graph,
        "-map".into(),
        "[outv]".into(),
        "-map".into(),
        "[outa]".into(),
        "-c:v".into(),
        "libx264".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "aac".into(),
        out_name.into(),
    ]
}

/// Validate + plan: parse the segment list, resolve the keep windows for `mode`
/// (`keep` = the listed windows; `remove` = their complement), and return
/// `(argv, out_name)`. Output is always `out.mp4` (the join re-encodes).
pub fn plan(in_name: &str, segments: &str, mode: &str) -> Result<(Vec<String>, String), String> {
    let windows = merge(parse_segments(segments)?);
    let keeps: Vec<Keep> = match mode {
        "keep" => windows.into_iter().map(|(s, e)| (s, Some(e))).collect(),
        "remove" => complement(&windows),
        other => {
            return Err(format!(
                "mode must be 'keep' or 'remove', got '{other}'"
            ))
        }
    };
    let out_name = "out.mp4".to_string();
    Ok((build_argv(in_name, &out_name, &keeps), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- fmt_num ----------------------------------------------------------
    #[test]
    fn fmt_num_drops_trailing_zero() {
        assert_eq!(fmt_num(8.0), "8");
        assert_eq!(fmt_num(2.5), "2.5");
        assert_eq!(fmt_num(9.75), "9.75");
        assert_eq!(fmt_num(0.0), "0");
    }

    // --- parse_timestamp --------------------------------------------------
    #[test]
    fn parse_timestamp_seconds_minutes_hours() {
        assert_eq!(parse_timestamp("5"), Ok(5.0));
        assert_eq!(parse_timestamp("5.5"), Ok(5.5));
        assert_eq!(parse_timestamp("1:23"), Ok(83.0));
        assert_eq!(parse_timestamp("0:05"), Ok(5.0));
        assert_eq!(parse_timestamp("1:02:03"), Ok(3723.0));
        assert_eq!(parse_timestamp(" 0:01:00.5 "), Ok(60.5));
    }
    #[test]
    fn parse_timestamp_rejects_garbage() {
        assert!(parse_timestamp("abc").is_err());
        assert!(parse_timestamp("").is_err());
        assert!(parse_timestamp("1:2:3:4").is_err());
        assert!(parse_timestamp("-5").is_err());
    }

    // --- parse_segments ---------------------------------------------------
    #[test]
    fn parse_segments_comma_and_newline() {
        assert_eq!(
            parse_segments("0:00-0:02, 0:08-0:10"),
            Ok(vec![(0.0, 2.0), (8.0, 10.0)])
        );
        assert_eq!(
            parse_segments("0-2\n8-10\n"),
            Ok(vec![(0.0, 2.0), (8.0, 10.0)])
        );
    }
    #[test]
    fn parse_segments_errors() {
        assert!(parse_segments("").is_err()); // no windows
        assert!(parse_segments("0:05").is_err()); // missing -end
        assert!(parse_segments("5-3").is_err()); // end <= start
        assert!(parse_segments("5-5").is_err()); // zero length
    }

    // --- merge ------------------------------------------------------------
    #[test]
    fn merge_sorts_and_joins_overlaps() {
        assert_eq!(merge(vec![(8.0, 10.0), (0.0, 3.0)]), vec![(0.0, 3.0), (8.0, 10.0)]);
        assert_eq!(merge(vec![(0.0, 5.0), (4.0, 8.0)]), vec![(0.0, 8.0)]);
        assert_eq!(merge(vec![(0.0, 2.0), (2.0, 4.0)]), vec![(0.0, 4.0)]); // adjacent
    }

    // --- complement -------------------------------------------------------
    #[test]
    fn complement_middle_removal_keeps_ends() {
        // remove [3,9] → keep [0,3] and [9,EOF]
        assert_eq!(complement(&[(3.0, 9.0)]), vec![(0.0, Some(3.0)), (9.0, None)]);
    }
    #[test]
    fn complement_removal_at_start_has_no_leading_keep() {
        assert_eq!(complement(&[(0.0, 3.0)]), vec![(3.0, None)]);
    }
    #[test]
    fn complement_two_removals() {
        assert_eq!(
            complement(&[(2.0, 4.0), (8.0, 10.0)]),
            vec![(0.0, Some(2.0)), (4.0, Some(8.0)), (10.0, None)]
        );
    }

    // --- build_argv -------------------------------------------------------
    #[test]
    fn build_argv_keep_two_windows() {
        let keeps = vec![(0.0, Some(2.0)), (8.0, Some(10.0))];
        let argv = build_argv("in.mp4", "out.mp4", &keeps);
        let graph = &argv[3];
        assert!(graph.contains("[0:v]trim=start=0:end=2,setpts=PTS-STARTPTS[v0];"));
        assert!(graph.contains("[0:a]atrim=start=8:end=10,asetpts=PTS-STARTPTS[a1];"));
        assert!(graph.ends_with("[v0][a0][v1][a1]concat=n=2:v=1:a=1[outv][outa]"));
        assert_eq!(argv[0], "-i");
        assert_eq!(argv.last().unwrap(), "out.mp4");
        // maps + re-encode present
        assert!(argv.iter().any(|a| a == "libx264"));
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "[outv]"));
    }
    #[test]
    fn build_argv_open_ended_tail_has_no_end() {
        let argv = build_argv("in.mp4", "out.mp4", &[(9.0, None)]);
        let graph = &argv[3];
        assert!(graph.contains("[0:v]trim=start=9,setpts=PTS-STARTPTS[v0];"));
        assert!(!graph.contains(":end="));
    }

    // --- plan -------------------------------------------------------------
    #[test]
    fn plan_keep_builds_two_trims() {
        let (argv, out) = plan("in.mp4", "0-2, 8-10", "keep").unwrap();
        assert_eq!(out, "out.mp4");
        assert!(argv[3].contains("concat=n=2"));
    }
    #[test]
    fn plan_remove_builds_complement() {
        let (argv, _) = plan("in.mp4", "3-9", "remove").unwrap();
        // remove [3,9] → keep [0,3] + [9,EOF] = 2 segments, tail open-ended
        assert!(argv[3].contains("concat=n=2"));
        assert!(argv[3].contains("[0:v]trim=start=9,setpts"));
    }
    #[test]
    fn plan_rejects_bad_mode() {
        assert!(plan("in.mp4", "0-2", "flip").is_err());
    }
    #[test]
    fn plan_rejects_empty_segments() {
        assert!(plan("in.mp4", "", "keep").is_err());
    }
}
