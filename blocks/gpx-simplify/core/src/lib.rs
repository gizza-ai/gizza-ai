//! gpx-simplify core — simplify GPX track points with Douglas-Peucker.

#[derive(Clone, Debug)]
struct Point {
    lat: f64,
    lon: f64,
    ele: Option<String>,
    time: Option<String>,
}

/// Simplify a GPX track, returning a new GPX document.
pub fn simplify(
    input: &str,
    tolerance_meters: f64,
    keep_every: usize,
    decimals: u32,
    output: &str,
) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input GPX is empty".into());
    }
    if !tolerance_meters.is_finite() || tolerance_meters < 0.0 {
        return Err("tolerance_meters must be a non-negative finite number".into());
    }
    if decimals > 8 {
        return Err("decimals must be between 0 and 8".into());
    }
    let output = if output.trim().is_empty() {
        "gpx"
    } else {
        output.trim()
    };
    if output != "gpx" && output != "summary" {
        return Err("output must be 'gpx' or 'summary'".into());
    }

    let points = parse_points(input)?;
    if points.len() < 2 {
        return Err("GPX must contain at least two <trkpt> points".into());
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    mark_rdp(&points, 0, points.len() - 1, tolerance_meters, &mut keep);
    if keep_every > 1 {
        for i in (0..points.len()).step_by(keep_every) {
            keep[i] = true;
        }
    }
    let kept: Vec<Point> = points
        .iter()
        .zip(keep.iter())
        .filter_map(|(p, k)| if *k { Some(p.clone()) } else { None })
        .collect();

    let distance_before = path_len(&points);
    let distance_after = path_len(&kept);
    if output == "summary" {
        return Ok(format!(
            "GPX simplify: {} → {} points ({:.1}% removed)\nTolerance: {} m\nDistance before: {:.1} m\nDistance after: {:.1} m",
            points.len(),
            kept.len(),
            100.0 * (points.len() - kept.len()) as f64 / points.len() as f64,
            trim(tolerance_meters),
            distance_before,
            distance_after
        ));
    }
    Ok(render_gpx(&kept, decimals))
}

/// Backwards scaffold-compatible entry point.
pub fn run(input: &str) -> Result<String, String> {
    simplify(input, 10.0, 0, 6, "gpx")
}

fn parse_points(input: &str) -> Result<Vec<Point>, String> {
    let mut pts = Vec::new();
    let mut rest = input;
    while let Some(start) = rest.find("<trkpt") {
        rest = &rest[start..];
        let Some(tag_end) = rest.find('>') else {
            return Err("unterminated <trkpt> tag".into());
        };
        let tag = &rest[..=tag_end];
        let lat = attr(tag, "lat").ok_or_else(|| "<trkpt> is missing lat".to_string())?;
        let lon = attr(tag, "lon").ok_or_else(|| "<trkpt> is missing lon".to_string())?;
        let lat: f64 = lat
            .parse()
            .map_err(|_| format!("invalid trkpt lat '{lat}'"))?;
        let lon: f64 = lon
            .parse()
            .map_err(|_| format!("invalid trkpt lon '{lon}'"))?;
        let after = &rest[tag_end + 1..];
        let Some(end) = after.find("</trkpt>") else {
            return Err("unterminated <trkpt> element".into());
        };
        let body = &after[..end];
        pts.push(Point {
            lat,
            lon,
            ele: child(body, "ele"),
            time: child(body, "time"),
        });
        rest = &after[end + "</trkpt>".len()..];
    }
    if pts.is_empty() {
        return Err("no <trkpt> elements found".into());
    }
    Ok(pts)
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let pat = format!("{name}=\"");
    let i = tag.find(&pat)? + pat.len();
    let end = tag[i..].find('"')?;
    Some(&tag[i..i + end])
}

fn child(body: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let i = body.find(&open)? + open.len();
    let end = body[i..].find(&close)?;
    Some(body[i..i + end].trim().to_string())
}

fn xy(p: &Point, lat0: f64) -> (f64, f64) {
    let r = 6_371_000.0;
    let x = p.lon.to_radians() * r * lat0.to_radians().cos();
    let y = p.lat.to_radians() * r;
    (x, y)
}

fn perp_distance(p: &Point, a: &Point, b: &Point) -> f64 {
    let lat0 = (a.lat + b.lat) / 2.0;
    let (px, py) = xy(p, lat0);
    let (ax, ay) = xy(a, lat0);
    let (bx, by) = xy(b, lat0);
    let dx = bx - ax;
    let dy = by - ay;
    if dx == 0.0 && dy == 0.0 {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    ((dy * px - dx * py + bx * ay - by * ax).abs()) / (dx * dx + dy * dy).sqrt()
}

fn mark_rdp(points: &[Point], first: usize, last: usize, tol: f64, keep: &mut [bool]) {
    if last <= first + 1 {
        return;
    }
    let mut best = 0.0;
    let mut idx = first;
    for i in first + 1..last {
        let d = perp_distance(&points[i], &points[first], &points[last]);
        if d > best {
            best = d;
            idx = i;
        }
    }
    if best > tol {
        keep[idx] = true;
        mark_rdp(points, first, idx, tol, keep);
        mark_rdp(points, idx, last, tol, keep);
    }
}

fn hav(a: &Point, b: &Point) -> f64 {
    let r = 6_371_000.0;
    let dlat = (b.lat - a.lat).to_radians();
    let dlon = (b.lon - a.lon).to_radians();
    let la1 = a.lat.to_radians();
    let la2 = b.lat.to_radians();
    let h = (dlat / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * r * h.sqrt().asin()
}

fn path_len(points: &[Point]) -> f64 {
    points.windows(2).map(|w| hav(&w[0], &w[1])).sum()
}

fn fmt_coord(v: f64, decimals: u32) -> String {
    let s = format!("{:.*}", decimals as usize, v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn render_gpx(points: &[Point], decimals: u32) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gpx version=\"1.1\" creator=\"gpx-simplify\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n  <trk>\n    <trkseg>\n");
    for p in points {
        out.push_str(&format!(
            "      <trkpt lat=\"{}\" lon=\"{}\">",
            fmt_coord(p.lat, decimals),
            fmt_coord(p.lon, decimals)
        ));
        if p.ele.is_none() && p.time.is_none() {
            out.push_str("</trkpt>\n");
            continue;
        }
        out.push('\n');
        if let Some(ele) = &p.ele {
            out.push_str(&format!("        <ele>{}</ele>\n", escape(ele)));
        }
        if let Some(time) = &p.time {
            out.push_str(&format!("        <time>{}</time>\n", escape(time)));
        }
        out.push_str("      </trkpt>\n");
    }
    out.push_str("    </trkseg>\n  </trk>\n</gpx>");
    out
}

fn trim(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GPX: &str = r#"<gpx><trk><trkseg>
<trkpt lat="0" lon="0"><ele>1</ele><time>2026-08-06T00:00:00Z</time></trkpt>
<trkpt lat="0" lon="0.0001"></trkpt>
<trkpt lat="0.001" lon="0.001"></trkpt>
<trkpt lat="0" lon="0.002"></trkpt>
</trkseg></trk></gpx>"#;

    #[test]
    fn simplifies_and_preserves_endpoints() {
        let out = simplify(GPX, 200.0, 0, 6, "gpx").unwrap();
        assert!(out.contains("lat=\"0\" lon=\"0\""));
        assert!(out.contains("lat=\"0\" lon=\"0.002\""));
        assert!(!out.contains("0.0001"));
    }

    #[test]
    fn low_tolerance_keeps_bend() {
        let out = simplify(GPX, 10.0, 0, 6, "summary").unwrap();
        assert!(out.contains("4 → 3 points"), "{out}");
    }

    #[test]
    fn keep_every_retains_periodic_samples() {
        let out = simplify(GPX, 10_000.0, 2, 6, "summary").unwrap();
        assert!(out.contains("4 → 3 points"), "{out}");
    }

    #[test]
    fn errors_are_actionable() {
        assert!(simplify("", 1.0, 0, 6, "gpx")
            .unwrap_err()
            .contains("empty"));
        assert!(simplify("<gpx/>", 1.0, 0, 6, "gpx")
            .unwrap_err()
            .contains("no <trkpt>"));
        assert!(simplify(GPX, -1.0, 0, 6, "gpx")
            .unwrap_err()
            .contains("non-negative"));
        assert!(simplify(GPX, 1.0, 0, 9, "gpx")
            .unwrap_err()
            .contains("decimals"));
        assert!(simplify(GPX, 1.0, 0, 6, "json")
            .unwrap_err()
            .contains("output"));
    }
}
