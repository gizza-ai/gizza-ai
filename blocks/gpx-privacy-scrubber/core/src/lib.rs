//! gpx-privacy-scrubber core — strip or fuzz the privacy-sensitive parts of a
//! GPX file (the start/end of a route, timestamps, and sensor extensions) before
//! sharing it. No wafer/wasm-bindgen deps; shared by the chat skill block and the
//! web page.
//!
//! The parser is a deliberately small, allocation-light string scanner rather
//! than a full XML tree: GPX in the wild is regular enough (`<trkpt>` / `<rtept>`
//! / `<wpt>` point elements, `<time>` stamps, `<extensions>` blocks) that byte
//! scanning is safe and keeps everything else in the document byte-for-byte
//! intact. Four deterministic passes, in order:
//!   1. `remove_extensions` — delete every `<extensions>…</extensions>` block
//!      (heart rate, cadence, power, temperature, hdop/vdop, …).
//!   2. `scrub_timestamps`   — replace the contents of every `<time>…</time>`
//!      with the Unix epoch (`1970-01-01T00:00:00Z`).
//!   3+4. the first and last track/route/way point — the ones that leak home /
//!      start / end locations — are either dropped whole (`remove`) or nudged a
//!      deterministic offset within `radius_m` metres (`fuzz`).

/// The three GPX point element names whose first/last occurrence leaks a
/// start/end location. Order is irrelevant; document order is what matters.
const POINT_TAGS: [&str; 3] = ["trkpt", "rtept", "wpt"];

/// The Unix epoch, written back over every `<time>` when timestamps are scrubbed.
const EPOCH: &str = "1970-01-01T00:00:00Z";

/// Metres per degree of latitude (WGS-84 mean); good enough for a privacy nudge.
const M_PER_DEG_LAT: f64 = 111_320.0;

/// A parsed point element: byte span of the whole element, plus the byte offset
/// just past the opening tag's `>` (so `[start, open_end)` is the opening tag).
#[derive(Clone, Copy)]
struct Point {
    start: usize,
    open_end: usize,
    end: usize,
}

/// Scrub a GPX document. `mode` is `"remove"` or `"fuzz"`; `radius_m` (clamped to
/// 1..=10000) is the fuzz offset in metres. Returns the scrubbed GPX text, or an
/// error string for empty input or a document with no GPX point tags.
pub fn run(
    gpx: &str,
    mode: &str,
    radius_m: u32,
    scrub_timestamps: bool,
    remove_extensions: bool,
) -> Result<String, String> {
    if gpx.trim().is_empty() {
        return Err("Input is empty. Paste a GPX file (XML) to scrub.".into());
    }
    let fuzz = match mode {
        "remove" => false,
        "fuzz" => true,
        other => {
            return Err(format!(
                "Unknown mode {other:?}. Use \"remove\" (drop the start/end points) or \"fuzz\" (offset them)."
            ))
        }
    };
    let radius_m = radius_m.clamp(1, 10_000) as f64;

    // Passes 1 and 2 rewrite the text wholesale; run them first so the point
    // spans in pass 3/4 are computed against the final byte layout.
    let mut text = gpx.to_string();
    if remove_extensions {
        text = remove_element_blocks(&text, "extensions");
    }
    if scrub_timestamps {
        text = scrub_time(&text);
    }

    let points = find_points(&text);
    if points.is_empty() {
        return Err(
            "No GPX point tags found. Expected at least one <trkpt>, <rtept>, or <wpt> element."
                .into(),
        );
    }

    // The first and last points are the privacy-sensitive ends. A single-point
    // document collapses the two into one target.
    let first = points[0];
    let last = points[points.len() - 1];

    let out = if fuzz {
        // Build the two opening-tag rewrites, then splice them in.
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        edits.push((first.start, first.open_end, fuzz_open_tag(&text, first, radius_m, 1.0)));
        if last.start != first.start {
            edits.push((last.start, last.open_end, fuzz_open_tag(&text, last, radius_m, -1.0)));
        }
        apply_edits(&text, edits)
    } else {
        // Drop the whole first/last point elements (line-expanded for tidy output).
        let mut spans: Vec<(usize, usize)> = Vec::new();
        spans.push(expand_span(&text, first.start, first.end));
        if last.start != first.start {
            spans.push(expand_span(&text, last.start, last.end));
        }
        apply_edits(&text, spans.into_iter().map(|(s, e)| (s, e, String::new())).collect())
    };

    Ok(out)
}

/// Apply a set of non-overlapping `(start, end, replacement)` edits, in ascending
/// start order, producing a new string.
fn apply_edits(s: &str, mut edits: Vec<(usize, usize, String)>) -> String {
    edits.sort_by_key(|e| e.0);
    let mut out = String::with_capacity(s.len());
    let mut cursor = 0;
    for (start, end, repl) in edits {
        if start < cursor {
            continue; // defensive: skip an overlapping edit
        }
        out.push_str(&s[cursor..start]);
        out.push_str(&repl);
        cursor = end;
    }
    out.push_str(&s[cursor..]);
    out
}

/// Find every `<trkpt>` / `<rtept>` / `<wpt>` element in document order.
fn find_points(s: &str) -> Vec<Point> {
    let b = s.as_bytes();
    let mut res = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'<' {
            if let Some((tag, name_end)) = match_point_tag(s, i) {
                if let Some(gt) = find_byte(b, name_end, b'>') {
                    let open_end = gt + 1;
                    let self_close = gt > 0 && b[gt - 1] == b'/';
                    let end = if self_close {
                        open_end
                    } else {
                        close_tag_end(s, open_end, tag).unwrap_or(open_end)
                    };
                    res.push(Point { start: i, open_end, end });
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
    res
}

/// If a point tag opens at byte `i` (`s[i] == '<'`), return its name and the byte
/// offset just past the name. Requires a tag-name boundary after the name so
/// `<wpt`/`<wpt>`/`<wpt/>` match but a hypothetical `<wptx` does not.
fn match_point_tag(s: &str, i: usize) -> Option<(&'static str, usize)> {
    let rest = &s[i + 1..];
    for &tag in &POINT_TAGS {
        if let Some(after) = rest.strip_prefix(tag) {
            let boundary = after
                .as_bytes()
                .first()
                .map(|c| matches!(c, b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'/'))
                .unwrap_or(false);
            if boundary {
                return Some((tag, i + 1 + tag.len()));
            }
        }
    }
    None
}

/// Byte offset just past the `>` of `</tag ...>` at/after `from`.
fn close_tag_end(s: &str, from: usize, tag: &str) -> Option<usize> {
    let needle = format!("</{tag}");
    let rel = s[from..].find(&needle)?;
    let cs = from + rel;
    find_byte(s.as_bytes(), cs, b'>').map(|gt| gt + 1)
}

/// Index of the first `target` byte at/after `from`.
fn find_byte(b: &[u8], from: usize, target: u8) -> Option<usize> {
    (from..b.len()).find(|&j| b[j] == target)
}

/// Delete every `<tag …>…</tag>` block (line-expanded), returning the new text.
fn remove_element_blocks(s: &str, tag: &str) -> String {
    let open = format!("<{tag}");
    let mut out = String::with_capacity(s.len());
    let mut rest_start = 0;
    let b = s.as_bytes();
    loop {
        let Some(rel) = s[rest_start..].find(&open) else { break };
        let start = rest_start + rel;
        // Require a tag-name boundary so `<extensions>` / `<extensions attr=…>`
        // match but a longer name would not.
        let after = start + open.len();
        let boundary = b
            .get(after)
            .map(|c| matches!(c, b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'/'))
            .unwrap_or(false);
        if !boundary {
            out.push_str(&s[rest_start..after]);
            rest_start = after;
            continue;
        }
        let Some(open_gt) = find_byte(b, after, b'>') else {
            break;
        };
        let self_close = b[open_gt - 1] == b'/';
        let end = if self_close {
            open_gt + 1
        } else {
            match close_tag_end(s, open_gt + 1, tag) {
                Some(e) => e,
                None => break,
            }
        };
        let (es, ee) = expand_span(s, start, end);
        out.push_str(&s[rest_start..es]);
        rest_start = ee;
    }
    out.push_str(&s[rest_start..]);
    out
}

/// Replace the contents of every `<time>…</time>` with the Unix epoch.
fn scrub_time(s: &str) -> String {
    const OPEN: &str = "<time>";
    const CLOSE: &str = "</time>";
    let mut out = String::with_capacity(s.len());
    let mut rest_start = 0;
    loop {
        let Some(rel) = s[rest_start..].find(OPEN) else { break };
        let open_at = rest_start + rel;
        let content_start = open_at + OPEN.len();
        let Some(crel) = s[content_start..].find(CLOSE) else { break };
        let content_end = content_start + crel;
        out.push_str(&s[rest_start..content_start]);
        out.push_str(EPOCH);
        rest_start = content_end; // leaves the "</time>" for the next copy
    }
    out.push_str(&s[rest_start..]);
    out
}

/// Expand a byte span to swallow the indentation before it and the single
/// trailing newline after it, so a removed element that was alone on its line
/// leaves no blank line behind.
fn expand_span(s: &str, start: usize, end: usize) -> (usize, usize) {
    let b = s.as_bytes();
    let mut es = start;
    while es > 0 && matches!(b[es - 1], b' ' | b'\t') {
        es -= 1;
    }
    let mut ee = end;
    while ee < b.len() && matches!(b[ee], b' ' | b'\t') {
        ee += 1;
    }
    if ee < b.len() && b[ee] == b'\r' {
        ee += 1;
    }
    if ee < b.len() && b[ee] == b'\n' {
        ee += 1;
    }
    (es, ee)
}

/// Rewrite a point's opening tag with lat/lon nudged a deterministic offset
/// within `radius_m` metres. `sign` (+1 for the first point, -1 for the last)
/// pushes the two ends in opposite directions so they don't collapse together.
/// Points whose lat/lon can't be parsed are returned unchanged.
fn fuzz_open_tag(s: &str, p: Point, radius_m: f64, sign: f64) -> String {
    let tag = &s[p.start..p.open_end];
    let (Some(lat), Some(lon)) = (attr_f64(tag, "lat"), attr_f64(tag, "lon")) else {
        return tag.to_string();
    };
    // Split the radius across lat and lon so the straight-line displacement is
    // ~radius_m: each component is radius/√2, longitude scaled by cos(lat).
    let comp = (radius_m / M_PER_DEG_LAT) * std::f64::consts::FRAC_1_SQRT_2;
    let coslat = lat.to_radians().cos().abs().max(1e-6);
    let new_lat = (lat + sign * comp).clamp(-90.0, 90.0);
    let new_lon = (lon + sign * comp / coslat).clamp(-180.0, 180.0);
    let tag = set_attr(tag, "lat", &format!("{new_lat:.6}")).unwrap_or_else(|| tag.to_string());
    set_attr(&tag, "lon", &format!("{new_lon:.6}")).unwrap_or(tag)
}

/// Read a numeric attribute value out of an opening tag (`name="…"` / `name='…'`).
fn attr_f64(tag: &str, name: &str) -> Option<f64> {
    let (vstart, vend, _q) = attr_value_span(tag, name)?;
    tag[vstart..vend].trim().parse().ok()
}

/// Replace an attribute's value in an opening tag, preserving the quote style.
fn set_attr(tag: &str, name: &str, val: &str) -> Option<String> {
    let (vstart, vend, _q) = attr_value_span(tag, name)?;
    let mut out = String::with_capacity(tag.len() + val.len());
    out.push_str(&tag[..vstart]);
    out.push_str(val);
    out.push_str(&tag[vend..]);
    Some(out)
}

/// Byte span `[start, end)` of an attribute's quoted value plus its quote char.
fn attr_value_span(tag: &str, name: &str) -> Option<(usize, usize, u8)> {
    let key = format!("{name}=");
    let ki = tag.find(&key)?;
    let b = tag.as_bytes();
    let mut j = ki + key.len();
    while j < b.len() && matches!(b[j], b' ' | b'\t') {
        j += 1;
    }
    let q = *b.get(j)?;
    if q != b'"' && q != b'\'' {
        return None;
    }
    let vstart = j + 1;
    let rel = tag[vstart..].find(q as char)?;
    Some((vstart, vstart + rel, q))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<gpx version="1.1">
  <trk>
    <trkseg>
      <trkpt lat="52.100000" lon="4.100000"><ele>1</ele><time>2024-01-01T08:00:00Z</time></trkpt>
      <trkpt lat="52.200000" lon="4.200000"><ele>2</ele><time>2024-01-01T08:05:00Z</time><extensions><hr>150</hr></extensions></trkpt>
      <trkpt lat="52.300000" lon="4.300000"><ele>3</ele><time>2024-01-01T08:10:00Z</time></trkpt>
    </trkseg>
  </trk>
</gpx>
"#;

    #[test]
    fn remove_drops_first_and_last_points() {
        let out = run(SAMPLE, "remove", 200, false, false).unwrap();
        // The two end points are gone…
        assert!(!out.contains("52.100000"), "first point should be removed");
        assert!(!out.contains("52.300000"), "last point should be removed");
        // …the middle point survives untouched…
        assert!(out.contains(r#"lat="52.200000" lon="4.200000""#));
        // …exactly one trkpt remains…
        assert_eq!(out.matches("<trkpt").count(), 1);
        // …and no blank line is left where the points were.
        assert!(!out.contains("\n\n"), "removal should not leave a blank line");
    }

    #[test]
    fn remove_single_point_drops_it() {
        let gpx = r#"<gpx><wpt lat="1.0" lon="2.0"><name>home</name></wpt></gpx>"#;
        let out = run(gpx, "remove", 200, false, false).unwrap();
        assert!(!out.contains("<wpt"));
        assert_eq!(out, "<gpx></gpx>");
    }

    #[test]
    fn fuzz_offsets_ends_but_keeps_middle_and_count() {
        let out = run(SAMPLE, "fuzz", 200, false, false).unwrap();
        // All three points remain.
        assert_eq!(out.matches("<trkpt").count(), 3);
        // Middle point is byte-for-byte unchanged.
        assert!(out.contains(r#"lat="52.200000" lon="4.200000""#));
        // The two ends moved off their exact coordinates…
        assert!(!out.contains(r#"lat="52.100000" lon="4.100000""#));
        assert!(!out.contains(r#"lat="52.300000" lon="4.300000""#));
        // …deterministically: first point +comp, last point -comp.
        let comp = (200.0 / M_PER_DEG_LAT) * std::f64::consts::FRAC_1_SQRT_2;
        assert!(out.contains(&format!("lat=\"{:.6}\"", 52.1 + comp)));
        assert!(out.contains(&format!("lat=\"{:.6}\"", 52.3 - comp)));
    }

    #[test]
    fn fuzz_is_deterministic() {
        let a = run(SAMPLE, "fuzz", 500, false, false).unwrap();
        let b = run(SAMPLE, "fuzz", 500, false, false).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn scrub_timestamps_rewrites_every_time() {
        let out = run(SAMPLE, "fuzz", 200, true, false).unwrap();
        assert!(!out.contains("2024-01-01"), "no original timestamp should survive");
        assert_eq!(out.matches(EPOCH).count(), 3);
    }

    #[test]
    fn scrub_timestamps_off_keeps_them() {
        let out = run(SAMPLE, "fuzz", 200, false, false).unwrap();
        assert!(out.contains("2024-01-01T08:05:00Z"));
        assert!(!out.contains(EPOCH));
    }

    #[test]
    fn remove_extensions_strips_the_block() {
        let out = run(SAMPLE, "fuzz", 200, false, true).unwrap();
        assert!(!out.contains("<extensions"));
        assert!(!out.contains("<hr>"));
        // The surviving point keeps its other children.
        assert!(out.contains(r#"lat="52.200000" lon="4.200000""#));
    }

    #[test]
    fn remove_extensions_off_keeps_the_block() {
        let out = run(SAMPLE, "fuzz", 200, false, false).unwrap();
        assert!(out.contains("<extensions><hr>150</hr></extensions>"));
    }

    #[test]
    fn empty_input_errors() {
        assert!(run("   \n  ", "remove", 200, true, true).is_err());
    }

    #[test]
    fn no_point_tags_errors() {
        let gpx = r#"<gpx version="1.1"><metadata><time>2024-01-01T00:00:00Z</time></metadata></gpx>"#;
        let err = run(gpx, "remove", 200, true, true).unwrap_err();
        assert!(err.contains("No GPX point tags"));
    }

    #[test]
    fn unknown_mode_errors() {
        assert!(run(SAMPLE, "shred", 200, true, true).is_err());
    }

    #[test]
    fn rtept_and_wpt_are_recognised() {
        let gpx = r#"<gpx><rte><rtept lat="1.0" lon="1.0"/><rtept lat="2.0" lon="2.0"/><rtept lat="3.0" lon="3.0"/></rte></gpx>"#;
        let out = run(gpx, "remove", 200, false, false).unwrap();
        // First and last rtept dropped, middle kept.
        assert_eq!(out.matches("<rtept").count(), 1);
        assert!(out.contains(r#"lat="2.0" lon="2.0""#));
    }

    #[test]
    fn radius_is_clamped() {
        // radius 0 clamps to 1 (still a valid, tiny offset) rather than erroring.
        let out = run(SAMPLE, "fuzz", 0, false, false).unwrap();
        assert_eq!(out.matches("<trkpt").count(), 3);
    }
}
