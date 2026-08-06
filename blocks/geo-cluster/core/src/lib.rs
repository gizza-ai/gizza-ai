//! gizza-ai/geo-cluster core — cluster latitude/longitude points by ground
//! distance and report the cluster assignments plus each cluster's centroid.
//! Pure-Rust (`serde_json` only), shared by the chat block, the CLI, and the page.
//!
//! Two methods:
//!
//! * **dbscan** — density clustering. A point is a *core* point when at least
//!   `min_points` points (itself included) lie within `radius`; cores that reach
//!   each other merge, points reached by a core but not themselves core join as
//!   *border* points, and everything else is *noise*.
//! * **grid** — bin points into fixed cells `radius` across; a cell holding at
//!   least `min_points` points is a cluster, the rest are noise.
//!
//! Distance is **haversine** on a sphere (mean Earth radius), so `radius` is a
//! real ground distance in the caller's units — unlike a Euclidean clusterer run
//! over raw degrees, where one degree of longitude shrinks from ~111 km at the
//! equator to ~28 km at 75° latitude. Neighbour lookups go through a cell index
//! (cells are at least `radius` wide everywhere in the data, so the 3×3 halo
//! around a point is guaranteed to contain every point within `radius`), and the
//! in-loop test compares squared chord length rather than calling `haversine`,
//! which is the same ordering without the trigonometry.
//!
//! Determinism: points are scanned in input order, clusters are numbered by their
//! first member, and a border point reachable from two clusters joins the one that
//! reaches it first. The same input always produces the same output.

use serde_json::{json, Map, Value};

/// Mean Earth radius in metres (IUGG mean radius R₁).
const EARTH_R_M: f64 = 6_371_008.8;

/// Metres per degree of latitude on that sphere (~111.19 km).
const M_PER_DEG: f64 = EARTH_R_M * std::f64::consts::PI / 180.0;

/// Maximum number of input points. Keeps the worst case (every point inside one
/// radius) comfortably inside the wasm sandbox.
pub const MAX_POINTS: usize = 10_000;

/// Largest accepted radius in metres — beyond half the Earth's circumference a
/// "neighbourhood" stops meaning anything.
const MAX_RADIUS_M: f64 = 20_000_000.0;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Coordinate order for the *non-GeoJSON* input forms (CSV lines and JSON arrays
/// of numeric pairs). GeoJSON is always `[longitude, latitude]` per RFC 7946 and
/// ignores this setting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoordOrder {
    /// `lat,lon` — the first value is latitude (default).
    LatLon,
    /// `lon,lat` — the first value is longitude.
    LonLat,
}

impl CoordOrder {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "lat_lon" | "latlon" | "lat,lon" | "lat_lng" => Ok(CoordOrder::LatLon),
            "lon_lat" | "lonlat" | "lon,lat" | "lng_lat" => Ok(CoordOrder::LonLat),
            other => Err(format!(
                "invalid coord_order '{other}': expected 'lat_lon' or 'lon_lat'"
            )),
        }
    }
}

/// Which clustering method to run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Method {
    /// Density clustering with a haversine neighbourhood radius.
    Dbscan,
    /// Fixed-cell binning, one cluster per populated cell.
    Grid,
}

impl Method {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "dbscan" | "density" => Ok(Method::Dbscan),
            "grid" | "cell" | "geohash" => Ok(Method::Grid),
            other => Err(format!(
                "invalid method '{other}': expected 'dbscan' or 'grid'"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Method::Dbscan => "dbscan",
            Method::Grid => "grid",
        }
    }
}

/// Distance unit for `radius` and for every distance in the output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Unit {
    name: &'static str,
    /// How many metres one of these is.
    metres: f64,
}

impl Unit {
    pub fn parse(s: &str) -> Result<Self, String> {
        let u = match s.trim().to_ascii_lowercase().as_str() {
            "" | "m" | "meter" | "meters" | "metre" | "metres" => Unit {
                name: "m",
                metres: 1.0,
            },
            "km" | "kilometer" | "kilometers" | "kilometre" | "kilometres" => Unit {
                name: "km",
                metres: 1000.0,
            },
            "mi" | "mile" | "miles" => Unit {
                name: "mi",
                metres: 1609.344,
            },
            "ft" | "foot" | "feet" => Unit {
                name: "ft",
                metres: 0.3048,
            },
            "nmi" | "nm" | "nautical_mile" | "nautical_miles" => Unit {
                name: "nmi",
                metres: 1852.0,
            },
            other => {
                return Err(format!(
                    "invalid units '{other}': expected 'm', 'km', 'mi', 'ft' or 'nmi'"
                ))
            }
        };
        Ok(u)
    }

    /// Convert a value in these units to metres.
    fn to_metres(self, v: f64) -> f64 {
        v * self.metres
    }

    /// Convert metres to these units.
    fn from_metres(self, m: f64) -> f64 {
        m / self.metres
    }
}

/// Output serialization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Text,
    Csv,
    Json,
    GeoJson,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" | "txt" => Ok(OutputFormat::Text),
            "csv" => Ok(OutputFormat::Csv),
            "json" => Ok(OutputFormat::Json),
            "geojson" => Ok(OutputFormat::GeoJson),
            other => Err(format!(
                "invalid output '{other}': expected 'text', 'csv', 'json' or 'geojson'"
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Points
// ---------------------------------------------------------------------------

/// One input point, kept in input order.
struct Point {
    lat: f64,
    lon: f64,
    label: Option<String>,
    /// Unit vector on the sphere — the chord test and the centroid both use it.
    xyz: [f64; 3],
}

impl Point {
    fn new(lat: f64, lon: f64, label: Option<String>) -> Self {
        Point {
            lat,
            lon,
            label,
            xyz: unit_vec(lat, lon),
        }
    }
}

/// Latitude/longitude in degrees → a unit vector on the sphere.
fn unit_vec(lat: f64, lon: f64) -> [f64; 3] {
    let (phi, lam) = (lat.to_radians(), lon.to_radians());
    let c = phi.cos();
    [c * lam.cos(), c * lam.sin(), phi.sin()]
}

/// Great-circle distance in metres between two points on the sphere.
fn haversine_m(a_lat: f64, a_lon: f64, b_lat: f64, b_lon: f64) -> f64 {
    let (p1, p2) = (a_lat.to_radians(), b_lat.to_radians());
    let dp = p2 - p1;
    let dl = (b_lon - a_lon).to_radians();
    let h = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * EARTH_R_M * h.sqrt().clamp(-1.0, 1.0).asin()
}

/// Squared chord length between two unit vectors. Monotone in great-circle
/// distance, so `chord2 <= chord2_for(eps)` is exactly `distance <= eps` — but
/// without any trigonometry in the inner loop.
fn chord2(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    dx * dx + dy * dy + dz * dz
}

/// The squared chord length matching a great-circle distance of `m` metres.
fn chord2_for(m: f64) -> f64 {
    let half = (m / (2.0 * EARTH_R_M)).sin();
    4.0 * half * half
}

/// Validate a decimal-degree coordinate, with a hint when lat/lon look swapped.
fn validate_degrees(lat: f64, lon: f64) -> Result<(), String> {
    if !lat.is_finite() || !lon.is_finite() {
        return Err("coordinate values must be finite numbers".into());
    }
    if !(-90.0..=90.0).contains(&lat) {
        return Err(format!(
            "latitude {lat} is out of range (-90..90); if this looks like a longitude, \
             set coord_order to 'lon_lat'"
        ));
    }
    if !(-180.0..=180.0).contains(&lon) {
        return Err(format!(
            "longitude {lon} is out of range (-180..180); if this looks like a latitude, \
             set coord_order to 'lat_lon'"
        ));
    }
    Ok(())
}

/// Order a numeric pair `(a, b)` into `(lat, lon)`.
fn pair_to_latlon(a: f64, b: f64, order: CoordOrder) -> (f64, f64) {
    match order {
        CoordOrder::LatLon => (a, b),
        CoordOrder::LonLat => (b, a),
    }
}

/// Read a GeoJSON coordinate array `[lon, lat, ...]` → `(lat, lon)`.
fn geojson_coord(v: &Value) -> Result<(f64, f64), String> {
    let arr = v
        .as_array()
        .ok_or("invalid GeoJSON: a coordinate must be an array [lon, lat]")?;
    if arr.len() < 2 {
        return Err("invalid GeoJSON: a coordinate needs at least [lon, lat]".into());
    }
    let lon = arr[0]
        .as_f64()
        .ok_or("invalid GeoJSON: coordinate longitude is not a number")?;
    let lat = arr[1]
        .as_f64()
        .ok_or("invalid GeoJSON: coordinate latitude is not a number")?;
    Ok((lat, lon))
}

/// Parse one `a,b[,label]` line → `(lat, lon, label)` honoring the coord order.
/// Accepts comma, semicolon, tab, or whitespace as the field separator.
fn parse_coord_line(line: &str, order: CoordOrder) -> Result<(f64, f64, Option<String>), String> {
    let fields: Vec<&str> = if line.contains(',') {
        line.split(',').collect()
    } else if line.contains(';') {
        line.split(';').collect()
    } else if line.contains('\t') {
        line.split('\t').collect()
    } else {
        line.split_whitespace().collect()
    };
    if fields.len() < 2 {
        return Err("expected two numbers per line".into());
    }
    let a: f64 = fields[0]
        .trim()
        .parse()
        .map_err(|_| format!("'{}' is not a number", fields[0].trim()))?;
    let b: f64 = fields[1]
        .trim()
        .parse()
        .map_err(|_| format!("'{}' is not a number", fields[1].trim()))?;
    let (lat, lon) = pair_to_latlon(a, b, order);
    let label = fields
        .get(2..)
        .map(|rest| rest.join(",").trim().to_string())
        .filter(|s| !s.is_empty());
    Ok((lat, lon, label))
}

/// Walk a GeoJSON value collecting every Point / MultiPoint position (a Feature's
/// `name`/`label`/`title` property becomes the point label).
fn collect_geojson_points(v: &Value, label: Option<&str>, out: &mut Vec<Point>) -> Result<(), String> {
    let t = v
        .get("type")
        .and_then(Value::as_str)
        .ok_or("not GeoJSON: missing a top-level \"type\"")?;
    match t {
        "Point" => {
            let (lat, lon) = geojson_coord(
                v.get("coordinates")
                    .ok_or("invalid GeoJSON Point: missing \"coordinates\"")?,
            )?;
            out.push(Point::new(lat, lon, label.map(str::to_string)));
        }
        "MultiPoint" => {
            let coords = v
                .get("coordinates")
                .and_then(Value::as_array)
                .ok_or("invalid GeoJSON MultiPoint: missing \"coordinates\"")?;
            for c in coords {
                let (lat, lon) = geojson_coord(c)?;
                out.push(Point::new(lat, lon, label.map(str::to_string)));
            }
        }
        "Feature" => {
            let name = v
                .get("properties")
                .and_then(Value::as_object)
                .and_then(|p| {
                    ["name", "label", "title"]
                        .iter()
                        .find_map(|k| p.get(*k).and_then(Value::as_str))
                })
                .map(str::to_string);
            if let Some(g) = v.get("geometry") {
                if !g.is_null() {
                    collect_geojson_points(g, name.as_deref().or(label), out)?;
                }
            }
        }
        "FeatureCollection" => {
            let feats = v
                .get("features")
                .and_then(Value::as_array)
                .ok_or("invalid GeoJSON FeatureCollection: missing a \"features\" array")?;
            for f in feats {
                collect_geojson_points(f, label, out)?;
            }
        }
        "GeometryCollection" => {
            let geoms = v
                .get("geometries")
                .and_then(Value::as_array)
                .ok_or("invalid GeoJSON GeometryCollection: missing \"geometries\"")?;
            for g in geoms {
                collect_geojson_points(g, label, out)?;
            }
        }
        // Non-point geometries carry nothing to cluster; skip them.
        "LineString" | "MultiLineString" | "Polygon" | "MultiPolygon" => {}
        other => return Err(format!("unsupported GeoJSON type '{other}' for points")),
    }
    Ok(())
}

fn obj_num(obj: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|k| obj.get(*k).and_then(Value::as_f64))
}

/// Parse one JSON array element into a point: a `[a,b]` pair, an object with
/// `lat`/`lon` (aliases `latitude`/`longitude`/`lng`) and an optional `label`, or
/// an embedded GeoJSON Point/Feature.
fn json_point(item: &Value, order: CoordOrder, idx: usize) -> Result<Point, String> {
    if let Some(pair) = item.as_array() {
        if pair.len() < 2 {
            return Err(format!("point #{}: expected a [lat,lon] pair", idx + 1));
        }
        let a = pair[0]
            .as_f64()
            .ok_or_else(|| format!("point #{}: not a number", idx + 1))?;
        let b = pair[1]
            .as_f64()
            .ok_or_else(|| format!("point #{}: not a number", idx + 1))?;
        let (lat, lon) = pair_to_latlon(a, b, order);
        return Ok(Point::new(lat, lon, None));
    }
    if let Some(obj) = item.as_object() {
        if obj.contains_key("type") {
            let mut out = Vec::new();
            collect_geojson_points(item, None, &mut out)?;
            if out.len() != 1 {
                return Err(format!(
                    "point #{}: a GeoJSON element must be a single Point",
                    idx + 1
                ));
            }
            return Ok(out.pop().unwrap());
        }
        let lat = obj_num(obj, &["lat", "latitude"])
            .ok_or_else(|| format!("point #{}: missing a lat/latitude field", idx + 1))?;
        let lon = obj_num(obj, &["lon", "lng", "long", "longitude"])
            .ok_or_else(|| format!("point #{}: missing a lon/longitude field", idx + 1))?;
        let label = obj
            .get("label")
            .or_else(|| obj.get("name"))
            .and_then(Value::as_str)
            .map(str::to_string);
        return Ok(Point::new(lat, lon, label));
    }
    Err(format!("point #{}: expected a pair or an object", idx + 1))
}

/// Parse the `points` input (CSV lines, a JSON array, or GeoJSON).
fn parse_points(input: &str, order: CoordOrder) -> Result<Vec<Point>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("points is empty — paste one `lat,lon` per line, a JSON array, or GeoJSON".into());
    }
    let mut out = Vec::new();
    if trimmed.starts_with('{') {
        let v: Value = serde_json::from_str(trimmed).map_err(|e| format!("invalid JSON: {e}"))?;
        collect_geojson_points(&v, None, &mut out)?;
        if out.is_empty() {
            return Err("the GeoJSON contains no Point or MultiPoint geometry".into());
        }
    } else if trimmed.starts_with('[') {
        let v: Value = serde_json::from_str(trimmed).map_err(|e| format!("invalid JSON: {e}"))?;
        let arr = v.as_array().ok_or("invalid points: expected a JSON array")?;
        for (i, item) in arr.iter().enumerate() {
            out.push(json_point(item, order, i)?);
        }
    } else {
        for (i, line) in trimmed.lines().enumerate() {
            let l = line.trim();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            match parse_coord_line(l, order) {
                Ok((lat, lon, label)) => out.push(Point::new(lat, lon, label)),
                // Allow a header row as the very first non-empty line.
                Err(e) if out.is_empty() && i == 0 => {
                    let _ = e;
                    continue;
                }
                Err(e) => return Err(format!("points line {}: {e}", i + 1)),
            }
        }
    }
    if out.is_empty() {
        return Err("no points found in the input".into());
    }
    if out.len() > MAX_POINTS {
        return Err(format!(
            "{} points exceeds the limit of {MAX_POINTS}; split the input into smaller batches",
            out.len()
        ));
    }
    for p in &out {
        validate_degrees(p.lat, p.lon)?;
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Cell index
// ---------------------------------------------------------------------------

/// Divide a `span` of degrees into a whole number of bands at least `want` wide.
/// Using an exact divisor matters: with a leftover partial column the wrap at the
/// antimeridian is misaligned and two points either side of ±180° never meet.
fn tile(span: f64, want: f64) -> (f64, i64) {
    let n = (span / want).floor().max(1.0).min(1e7);
    (span / n, n as i64)
}

/// A latitude/longitude cell grid whose cells are at least `cell_m` metres wide
/// everywhere in the data, so a 3×3 halo always contains every point within
/// `cell_m`. Longitude columns wrap at the antimeridian.
struct CellGrid {
    dlat: f64,
    dlon: f64,
    nrows: i64,
    ncols: i64,
    /// Cell key → the indices of the points in it, in input order.
    cells: std::collections::HashMap<(i64, i64), Vec<usize>>,
}

impl CellGrid {
    fn build(points: &[Point], cell_m: f64) -> Self {
        let (dlat, nrows) = tile(180.0, (cell_m / M_PER_DEG).min(180.0));
        // The widest longitude spacing the data needs: at the highest |latitude|
        // present, `dlon` degrees of longitude must still span >= cell_m metres.
        let max_abs_lat = points.iter().fold(0.0f64, |m, p| m.max(p.lat.abs()));
        let coslat = max_abs_lat.to_radians().cos().abs().max(1e-9);
        let (dlon, ncols) = tile(360.0, (dlat / coslat).min(360.0));
        let mut cells: std::collections::HashMap<(i64, i64), Vec<usize>> =
            std::collections::HashMap::new();
        for (i, p) in points.iter().enumerate() {
            let key = Self::key_for(p, dlat, dlon, nrows, ncols);
            cells.entry(key).or_default().push(i);
        }
        CellGrid {
            dlat,
            dlon,
            nrows,
            ncols,
            cells,
        }
    }

    fn key_for(p: &Point, dlat: f64, dlon: f64, nrows: i64, ncols: i64) -> (i64, i64) {
        let row = (((p.lat + 90.0) / dlat).floor() as i64).clamp(0, nrows - 1);
        let col = (((p.lon + 180.0) / dlon).floor() as i64).rem_euclid(ncols);
        (row, col)
    }

    /// Every point index in the 3×3 cell halo around `p` (longitude wraps).
    fn halo(&self, p: &Point, out: &mut Vec<usize>) {
        out.clear();
        let (row, col) = Self::key_for(p, self.dlat, self.dlon, self.nrows, self.ncols);
        for dr in -1..=1 {
            for dc in -1..=1 {
                if self.ncols <= 1 && dc != 0 {
                    continue;
                }
                let c = (col + dc).rem_euclid(self.ncols);
                if let Some(ids) = self.cells.get(&(row + dr, c)) {
                    out.extend_from_slice(ids);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
    }
}

// ---------------------------------------------------------------------------
// Clustering
// ---------------------------------------------------------------------------

/// `labels[i]` is the 1-based cluster id of point `i`, or `None` for noise.
type Labels = Vec<Option<usize>>;

/// Density clustering with a haversine `eps_m` neighbourhood.
fn dbscan(points: &[Point], eps_m: f64, min_points: usize) -> Labels {
    let grid = CellGrid::build(points, eps_m);
    let eps2 = chord2_for(eps_m);
    let n = points.len();
    let mut labels: Labels = vec![None; n];
    let mut visited = vec![false; n];
    let mut next_id = 1usize;
    let mut halo: Vec<usize> = Vec::new();

    // Neighbours of `i` within eps (including `i` itself).
    let neighbours = |i: usize, halo: &mut Vec<usize>| -> Vec<usize> {
        grid.halo(&points[i], halo);
        halo.iter()
            .copied()
            .filter(|&j| chord2(&points[i].xyz, &points[j].xyz) <= eps2)
            .collect()
    };

    for i in 0..n {
        if visited[i] {
            continue;
        }
        visited[i] = true;
        let seed = neighbours(i, &mut halo);
        if seed.len() < min_points {
            // Not a core point (yet) — it may still be picked up as a border
            // point by a later cluster, so leave it unlabelled for now.
            continue;
        }
        let id = next_id;
        next_id += 1;
        labels[i] = Some(id);
        // Breadth-first expansion over the core points reachable from `i`.
        let mut queue = seed;
        let mut qi = 0usize;
        while qi < queue.len() {
            let j = queue[qi];
            qi += 1;
            if labels[j].is_none() {
                labels[j] = Some(id); // border points join the first cluster to reach them
            }
            if visited[j] {
                continue;
            }
            visited[j] = true;
            let nb = neighbours(j, &mut halo);
            if nb.len() >= min_points {
                queue.extend(nb);
            }
        }
    }
    renumber(labels)
}

/// Bin points into fixed cells `cell_m` across; a cell with at least
/// `min_points` points is a cluster.
fn grid_cluster(points: &[Point], cell_m: f64, min_points: usize) -> Labels {
    let (dlat, nrows) = tile(180.0, (cell_m / M_PER_DEG).min(180.0));
    let mut cells: std::collections::HashMap<(i64, i64), Vec<usize>> =
        std::collections::HashMap::new();
    for (i, p) in points.iter().enumerate() {
        let row = (((p.lat + 90.0) / dlat).floor() as i64).clamp(0, nrows - 1);
        // Cell width is widened by 1/cos(latitude) of the row's centre band so
        // cells stay roughly square on the ground instead of collapsing at the
        // poles, then rounded to a whole number of columns so each band tiles
        // exactly (no seam at the antimeridian).
        let band_lat = ((row as f64 + 0.5) * dlat - 90.0).clamp(-89.999_999, 89.999_999);
        let want = (dlat / band_lat.to_radians().cos().abs().max(1e-9)).min(360.0);
        let (dlon, ncols) = tile(360.0, want);
        let col = (((p.lon + 180.0) / dlon).floor() as i64).rem_euclid(ncols);
        cells.entry((row, col)).or_default().push(i);
    }
    let mut labels: Labels = vec![None; points.len()];
    let mut id = 1usize;
    // Assign ids in input order of each cell's first member, so numbering is
    // deterministic and matches the reading order of the input.
    let mut keys: Vec<((i64, i64), usize)> = cells
        .iter()
        .filter(|(_, ids)| ids.len() >= min_points)
        .map(|(k, ids)| (*k, ids[0]))
        .collect();
    keys.sort_by_key(|&(_, first)| first);
    for (key, _) in keys {
        for &i in &cells[&key] {
            labels[i] = Some(id);
        }
        id += 1;
    }
    labels
}

/// Renumber cluster ids 1..k by the input index of each cluster's first member.
fn renumber(labels: Labels) -> Labels {
    let mut order: Vec<usize> = Vec::new();
    let mut seen = std::collections::HashMap::new();
    for l in labels.iter().flatten() {
        if !seen.contains_key(l) {
            seen.insert(*l, order.len() + 1);
            order.push(*l);
        }
    }
    labels
        .into_iter()
        .map(|l| l.map(|id| seen[&id]))
        .collect()
}

/// Everything computed about one cluster.
struct Cluster {
    id: usize,
    members: Vec<usize>,
    lat: f64,
    lon: f64,
    /// Distance in metres from the centroid to the farthest member.
    radius_m: f64,
    min_lat: f64,
    min_lon: f64,
    max_lat: f64,
    max_lon: f64,
}

/// Build the per-cluster summaries. The centroid is the **spherical** mean (the
/// normalized mean of the members' unit vectors), so a cluster straddling the
/// antimeridian gets a sensible answer instead of an average near 0° longitude.
fn summarize(points: &[Point], labels: &Labels) -> Vec<Cluster> {
    let k = labels.iter().flatten().copied().max().unwrap_or(0);
    let mut clusters: Vec<Cluster> = (1..=k)
        .map(|id| Cluster {
            id,
            members: Vec::new(),
            lat: 0.0,
            lon: 0.0,
            radius_m: 0.0,
            min_lat: f64::INFINITY,
            min_lon: f64::INFINITY,
            max_lat: f64::NEG_INFINITY,
            max_lon: f64::NEG_INFINITY,
        })
        .collect();
    for (i, l) in labels.iter().enumerate() {
        if let Some(id) = l {
            clusters[*id - 1].members.push(i);
        }
    }
    for c in &mut clusters {
        let mut sum = [0.0f64; 3];
        for &i in &c.members {
            let p = &points[i];
            for d in 0..3 {
                sum[d] += p.xyz[d];
            }
            c.min_lat = c.min_lat.min(p.lat);
            c.max_lat = c.max_lat.max(p.lat);
            c.min_lon = c.min_lon.min(p.lon);
            c.max_lon = c.max_lon.max(p.lon);
        }
        let norm = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
        if norm > 1e-12 {
            let (x, y, z) = (sum[0] / norm, sum[1] / norm, sum[2] / norm);
            c.lat = z.clamp(-1.0, 1.0).asin().to_degrees();
            c.lon = y.atan2(x).to_degrees();
        } else {
            // Antipodal members cancel out; fall back to the first member.
            let p = &points[c.members[0]];
            c.lat = p.lat;
            c.lon = p.lon;
        }
        c.radius_m = c
            .members
            .iter()
            .map(|&i| haversine_m(c.lat, c.lon, points[i].lat, points[i].lon))
            .fold(0.0f64, f64::max);
    }
    clusters
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// Round to `dp` decimal places (and normalize `-0`).
fn round(v: f64, dp: i32) -> f64 {
    let f = 10f64.powi(dp);
    let r = (v * f).round() / f;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// A distance for display: one decimal place, with a bare integer when exact.
fn fmt_dist(v: f64) -> String {
    let r = round(v, 1);
    if (r - r.round()).abs() < f64::EPSILON {
        format!("{}", r.round() as i64)
    } else {
        format!("{r}")
    }
}

/// A coordinate for display: 6 decimal places (~0.1 m).
fn fmt_deg(v: f64) -> String {
    format!("{:.6}", round(v, 6))
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("{n} {word}")
    } else {
        format!("{n} {word}s")
    }
}

/// One CSV field, quoted only when it needs to be.
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_text(
    points: &[Point],
    labels: &Labels,
    clusters: &[Cluster],
    method: Method,
    radius: f64,
    unit: Unit,
    min_points: usize,
) -> String {
    let noise = labels.iter().filter(|l| l.is_none()).count();
    let mut out = String::new();
    out.push_str(&format!(
        "{} · {} · {} unclustered\n",
        plural(points.len(), "point"),
        plural(clusters.len(), "cluster"),
        noise
    ));
    out.push_str(&format!(
        "method {} · radius {} {} · min_points {}\n",
        method.as_str(),
        fmt_dist(radius),
        unit.name,
        min_points
    ));
    for c in clusters {
        out.push_str(&format!(
            "\nCluster {}  {}  centroid {}, {}  radius {} {}",
            c.id,
            plural(c.members.len(), "point"),
            fmt_deg(c.lat),
            fmt_deg(c.lon),
            fmt_dist(unit.from_metres(c.radius_m)),
            unit.name
        ));
    }
    if clusters.is_empty() {
        out.push_str("\nNo cluster met the min_points threshold.");
    }
    out
}

fn render_csv(points: &[Point], labels: &Labels, clusters: &[Cluster], unit: Unit) -> String {
    let has_labels = points.iter().any(|p| p.label.is_some());
    let mut out = String::from("point,latitude,longitude,");
    if has_labels {
        out.push_str("label,");
    }
    out.push_str("cluster,cluster_size,centroid_latitude,centroid_longitude");
    for (i, p) in points.iter().enumerate() {
        out.push('\n');
        out.push_str(&format!("{},{},{},", i + 1, fmt_deg(p.lat), fmt_deg(p.lon)));
        if has_labels {
            out.push_str(&csv_field(p.label.as_deref().unwrap_or("")));
            out.push(',');
        }
        match labels[i] {
            Some(id) => {
                let c = &clusters[id - 1];
                out.push_str(&format!(
                    "{},{},{},{}",
                    id,
                    c.members.len(),
                    fmt_deg(c.lat),
                    fmt_deg(c.lon)
                ));
            }
            None => out.push_str("noise,,,"),
        }
    }
    let _ = unit; // distances are not part of the per-point CSV row
    out
}

fn render_json(
    points: &[Point],
    labels: &Labels,
    clusters: &[Cluster],
    method: Method,
    radius: f64,
    unit: Unit,
    min_points: usize,
) -> String {
    let noise = labels.iter().filter(|l| l.is_none()).count();
    let cluster_json: Vec<Value> = clusters
        .iter()
        .map(|c| {
            json!({
                "cluster": c.id,
                "points": c.members.len(),
                "centroid": { "latitude": round(c.lat, 6), "longitude": round(c.lon, 6) },
                "radius": round(unit.from_metres(c.radius_m), 3),
                "bbox": {
                    "min_latitude": round(c.min_lat, 6),
                    "min_longitude": round(c.min_lon, 6),
                    "max_latitude": round(c.max_lat, 6),
                    "max_longitude": round(c.max_lon, 6),
                },
                "members": c.members.iter().map(|i| i + 1).collect::<Vec<_>>(),
            })
        })
        .collect();
    let point_json: Vec<Value> = points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut o = Map::new();
            o.insert("point".into(), json!(i + 1));
            o.insert("latitude".into(), json!(round(p.lat, 6)));
            o.insert("longitude".into(), json!(round(p.lon, 6)));
            if let Some(l) = &p.label {
                o.insert("label".into(), json!(l));
            }
            o.insert(
                "cluster".into(),
                match labels[i] {
                    Some(id) => json!(id),
                    None => Value::Null,
                },
            );
            Value::Object(o)
        })
        .collect();
    let root = json!({
        "method": method.as_str(),
        "radius": radius,
        "units": unit.name,
        "min_points": min_points,
        "point_count": points.len(),
        "cluster_count": clusters.len(),
        "unclustered": noise,
        "clusters": cluster_json,
        "points": point_json,
    });
    serde_json::to_string_pretty(&root).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn render_geojson(
    points: &[Point],
    labels: &Labels,
    clusters: &[Cluster],
    unit: Unit,
) -> String {
    let mut features: Vec<Value> = Vec::with_capacity(points.len() + clusters.len());
    for (i, p) in points.iter().enumerate() {
        let mut props = Map::new();
        props.insert("role".into(), json!("point"));
        props.insert("point".into(), json!(i + 1));
        props.insert(
            "cluster".into(),
            match labels[i] {
                Some(id) => json!(id),
                None => json!("noise"),
            },
        );
        if let Some(l) = &p.label {
            props.insert("label".into(), json!(l));
        }
        features.push(json!({
            "type": "Feature",
            "geometry": { "type": "Point", "coordinates": [round(p.lon, 6), round(p.lat, 6)] },
            "properties": props,
        }));
    }
    for c in clusters {
        features.push(json!({
            "type": "Feature",
            "geometry": { "type": "Point", "coordinates": [round(c.lon, 6), round(c.lat, 6)] },
            "properties": {
                "role": "centroid",
                "cluster": c.id,
                "points": c.members.len(),
                "radius": round(unit.from_metres(c.radius_m), 3),
                "units": unit.name,
            },
        }));
    }
    serde_json::to_string_pretty(&json!({ "type": "FeatureCollection", "features": features }))
        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Cluster `points` and render the result.
///
/// * `points` — CSV `lat,lon[,label]` lines, a JSON array of pairs/objects, or GeoJSON.
/// * `method` — `"dbscan"` (default) or `"grid"`.
/// * `radius` — DBSCAN neighbourhood radius / grid cell size, in `units`.
/// * `units` — `"m"` (default) | `"km"` | `"mi"` | `"ft"` | `"nmi"`.
/// * `min_points` — minimum points for a cluster, counting the point itself.
/// * `coord_order` — `"lat_lon"` (default) | `"lon_lat"` for the non-GeoJSON forms.
/// * `output` — `"text"` (default) | `"csv"` | `"json"` | `"geojson"`.
#[allow(clippy::too_many_arguments)]
pub fn cluster(
    points: &str,
    method: &str,
    radius: f64,
    units: &str,
    min_points: f64,
    coord_order: &str,
    output: &str,
) -> Result<String, String> {
    let method = Method::parse(method)?;
    let unit = Unit::parse(units)?;
    let order = CoordOrder::parse(coord_order)?;
    let fmt = OutputFormat::parse(output)?;

    if !radius.is_finite() || radius <= 0.0 {
        return Err(format!(
            "radius must be a positive number of {}; got {radius}",
            unit.name
        ));
    }
    let radius_m = unit.to_metres(radius);
    if radius_m > MAX_RADIUS_M {
        return Err(format!(
            "radius {radius} {} is larger than half the Earth's circumference",
            unit.name
        ));
    }
    if !min_points.is_finite() || min_points < 1.0 || min_points.fract() != 0.0 {
        return Err(format!(
            "min_points must be a whole number of 1 or more; got {min_points}"
        ));
    }
    let min_points = min_points as usize;

    let pts = parse_points(points, order)?;
    let labels = match method {
        Method::Dbscan => dbscan(&pts, radius_m, min_points),
        Method::Grid => grid_cluster(&pts, radius_m, min_points),
    };
    let clusters = summarize(&pts, &labels);

    Ok(match fmt {
        OutputFormat::Text => render_text(
            &pts, &labels, &clusters, method, radius, unit, min_points,
        ),
        OutputFormat::Csv => render_csv(&pts, &labels, &clusters, unit),
        OutputFormat::Json => render_json(
            &pts, &labels, &clusters, method, radius, unit, min_points,
        ),
        OutputFormat::GeoJson => render_geojson(&pts, &labels, &clusters, unit),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Two tight groups ~5.5 km apart, plus one far-away outlier.
    const THREE_GROUPS: &str = "\
51.5000,-0.1000,A1
51.5010,-0.1000,A2
51.5020,-0.1000,A3
51.5500,-0.1000,B1
51.5510,-0.1000,B2
51.5520,-0.1000,B3
40.7128,-74.0060,Far";

    #[test]
    fn dbscan_finds_two_clusters_and_one_noise_point() {
        let out = cluster(THREE_GROUPS, "dbscan", 500.0, "m", 2.0, "lat_lon", "text").unwrap();
        assert!(out.starts_with("7 points · 2 clusters · 1 unclustered\n"), "{out}");
        assert!(out.contains("Cluster 1  3 points  centroid 51.501000, -0.100000"), "{out}");
        assert!(out.contains("Cluster 2  3 points  centroid 51.551000, -0.100000"), "{out}");
    }

    #[test]
    fn dbscan_radius_units_are_ground_distance() {
        // 500 m keeps the two London groups apart; 10 km merges them but still
        // leaves New York out.
        let tight = cluster(THREE_GROUPS, "dbscan", 500.0, "m", 2.0, "lat_lon", "text").unwrap();
        assert!(tight.contains("2 clusters"), "{tight}");
        let loose = cluster(THREE_GROUPS, "dbscan", 10.0, "km", 2.0, "lat_lon", "text").unwrap();
        assert!(loose.contains("1 cluster · 1 unclustered"), "{loose}");
        // The same radius expressed in miles/feet/nautical miles agrees.
        let miles = cluster(THREE_GROUPS, "dbscan", 6.2137, "mi", 2.0, "lat_lon", "text").unwrap();
        assert!(miles.contains("1 cluster · 1 unclustered"), "{miles}");
        let feet = cluster(THREE_GROUPS, "dbscan", 32808.4, "ft", 2.0, "lat_lon", "text").unwrap();
        assert!(feet.contains("1 cluster · 1 unclustered"), "{feet}");
        let nmi = cluster(THREE_GROUPS, "dbscan", 5.3996, "nmi", 2.0, "lat_lon", "text").unwrap();
        assert!(nmi.contains("1 cluster · 1 unclustered"), "{nmi}");
    }

    #[test]
    fn min_points_promotes_a_group_to_noise() {
        // Group A has 3 points, so min_points 4 leaves everything unclustered.
        let out = cluster(THREE_GROUPS, "dbscan", 500.0, "m", 4.0, "lat_lon", "text").unwrap();
        assert!(out.starts_with("7 points · 0 clusters · 7 unclustered"), "{out}");
        assert!(out.contains("No cluster met the min_points threshold."), "{out}");
    }

    #[test]
    fn grid_bins_points_into_cells() {
        let out = cluster(THREE_GROUPS, "grid", 10.0, "km", 2.0, "lat_lon", "text").unwrap();
        // The two London groups fall in one ~10 km cell; New York is alone and
        // below min_points.
        assert!(out.starts_with("7 points · 1 cluster · 1 unclustered"), "{out}");
    }

    #[test]
    fn grid_with_min_points_one_keeps_singletons() {
        let out = cluster(THREE_GROUPS, "grid", 10.0, "km", 1.0, "lat_lon", "text").unwrap();
        assert!(out.starts_with("7 points · 2 clusters · 0 unclustered"), "{out}");
    }

    #[test]
    fn csv_output_carries_assignments_and_centroids() {
        let out = cluster(THREE_GROUPS, "dbscan", 500.0, "m", 2.0, "lat_lon", "csv").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "point,latitude,longitude,label,cluster,cluster_size,centroid_latitude,centroid_longitude"
        );
        assert_eq!(
            lines[1],
            "1,51.500000,-0.100000,A1,1,3,51.501000,-0.100000"
        );
        assert_eq!(lines[7], "7,40.712800,-74.006000,Far,noise,,,");
    }

    #[test]
    fn csv_omits_the_label_column_when_no_point_has_one() {
        let out = cluster("51.5,-0.1\n51.5001,-0.1", "dbscan", 500.0, "m", 2.0, "lat_lon", "csv")
            .unwrap();
        assert_eq!(
            out.lines().next().unwrap(),
            "point,latitude,longitude,cluster,cluster_size,centroid_latitude,centroid_longitude"
        );
    }

    #[test]
    fn json_output_has_clusters_and_null_for_noise() {
        let out = cluster(THREE_GROUPS, "dbscan", 500.0, "m", 2.0, "lat_lon", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["cluster_count"], 2);
        assert_eq!(v["unclustered"], 1);
        assert_eq!(v["units"], "m");
        assert_eq!(v["clusters"][0]["points"], 3);
        assert_eq!(v["clusters"][0]["centroid"]["latitude"], 51.501);
        assert_eq!(v["clusters"][0]["members"], json!([1, 2, 3]));
        assert_eq!(v["points"][6]["cluster"], Value::Null);
        assert_eq!(v["points"][0]["label"], "A1");
    }

    #[test]
    fn geojson_output_has_point_and_centroid_features() {
        let out = cluster(THREE_GROUPS, "dbscan", 500.0, "m", 2.0, "lat_lon", "geojson").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["type"], "FeatureCollection");
        let feats = v["features"].as_array().unwrap();
        assert_eq!(feats.len(), 9); // 7 points + 2 centroids
        assert_eq!(feats[0]["geometry"]["coordinates"], json!([-0.1, 51.5]));
        assert_eq!(feats[0]["properties"]["cluster"], 1);
        assert_eq!(feats[6]["properties"]["cluster"], "noise");
        assert_eq!(feats[7]["properties"]["role"], "centroid");
        assert_eq!(feats[7]["properties"]["points"], 3);
    }

    #[test]
    fn accepts_geojson_input_with_feature_names() {
        let gj = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"name":"Shop"},"geometry":{"type":"Point","coordinates":[-0.1,51.5]}},
          {"type":"Feature","properties":{"name":"Depot"},"geometry":{"type":"Point","coordinates":[-0.1,51.5008]}}
        ]}"#;
        let out = cluster(gj, "dbscan", 200.0, "m", 2.0, "lat_lon", "csv").unwrap();
        assert!(out.contains("1,51.500000,-0.100000,Shop,1,2,"), "{out}");
        assert!(out.contains("2,51.500800,-0.100000,Depot,1,2,"), "{out}");
    }

    #[test]
    fn accepts_json_pairs_and_objects() {
        let pairs = cluster("[[51.5,-0.1],[51.5008,-0.1]]", "dbscan", 200.0, "m", 2.0, "lat_lon", "text")
            .unwrap();
        assert!(pairs.starts_with("2 points · 1 cluster · 0 unclustered"), "{pairs}");
        let objs = cluster(
            r#"[{"lat":51.5,"lng":-0.1,"label":"a"},{"latitude":51.5008,"longitude":-0.1}]"#,
            "dbscan",
            200.0,
            "m",
            2.0,
            "lat_lon",
            "text",
        )
        .unwrap();
        assert!(objs.starts_with("2 points · 1 cluster · 0 unclustered"), "{objs}");
    }

    #[test]
    fn coord_order_lon_lat_swaps_the_pair() {
        // As lat,lon these would be out of range; as lon,lat they are valid.
        let out = cluster("-0.1,51.5\n-0.1,51.5008", "dbscan", 200.0, "m", 2.0, "lon_lat", "text")
            .unwrap();
        assert!(out.contains("centroid 51.500400, -0.100000"), "{out}");
    }

    #[test]
    fn skips_a_header_row_and_comment_lines() {
        let out = cluster(
            "lat,lon,name\n# a comment\n51.5,-0.1,A\n51.5008,-0.1,B",
            "dbscan",
            200.0,
            "m",
            2.0,
            "lat_lon",
            "text",
        )
        .unwrap();
        assert!(out.starts_with("2 points · 1 cluster"), "{out}");
    }

    #[test]
    fn centroid_is_spherical_across_the_antimeridian() {
        // ±179.99 should average to 180, not to 0.
        let out = cluster(
            "0,179.99\n0,-179.99",
            "dbscan",
            10.0,
            "km",
            2.0,
            "lat_lon",
            "text",
        )
        .unwrap();
        assert!(out.contains("centroid 0.000000, 180.000000"), "{out}");
    }

    #[test]
    fn errors_on_bad_input() {
        assert!(cluster("", "dbscan", 500.0, "m", 2.0, "lat_lon", "text")
            .unwrap_err()
            .contains("points is empty"));
        assert!(cluster("51.5,-0.1", "kmeans", 500.0, "m", 2.0, "lat_lon", "text")
            .unwrap_err()
            .contains("invalid method 'kmeans'"));
        assert!(cluster("51.5,-0.1", "dbscan", 0.0, "m", 2.0, "lat_lon", "text")
            .unwrap_err()
            .contains("radius must be a positive number"));
        assert!(cluster("51.5,-0.1", "dbscan", 500.0, "parsec", 2.0, "lat_lon", "text")
            .unwrap_err()
            .contains("invalid units 'parsec'"));
        assert!(cluster("51.5,-0.1", "dbscan", 500.0, "m", 0.0, "lat_lon", "text")
            .unwrap_err()
            .contains("min_points must be a whole number"));
        assert!(cluster("51.5,-0.1", "dbscan", 500.0, "m", 2.0, "lat_lon", "kml")
            .unwrap_err()
            .contains("invalid output 'kml'"));
        assert!(cluster("951.5,-0.1", "dbscan", 500.0, "m", 2.0, "lat_lon", "text")
            .unwrap_err()
            .contains("latitude 951.5 is out of range"));
        assert!(cluster("51.5,-0.1\nnope,here", "dbscan", 500.0, "m", 2.0, "lat_lon", "text")
            .unwrap_err()
            .contains("points line 2"));
    }

    #[test]
    fn rejects_more_than_the_cap_and_accepts_exactly_the_cap() {
        let at_cap: String = (0..MAX_POINTS)
            .map(|i| format!("{:.4},0.0\n", 10.0 + i as f64 * 0.0001))
            .collect();
        assert!(cluster(&at_cap, "grid", 1.0, "km", 1.0, "lat_lon", "text").is_ok());
        let over = format!("{at_cap}51.5,-0.1\n");
        let err = cluster(&over, "grid", 1.0, "km", 1.0, "lat_lon", "text").unwrap_err();
        assert!(err.contains("10001 points exceeds the limit of 10000"), "{err}");
    }

    #[test]
    fn dbscan_border_point_joins_the_first_cluster_deterministically() {
        // A chain: two dense cores 900 m apart with a single point in the middle.
        // Whichever way it is scanned, the answer must not change between runs.
        let data = "0,0\n0,0.001\n0,0.002\n0,0.009\n0,0.0125\n0,0.0135\n0,0.0145";
        let a = cluster(data, "dbscan", 250.0, "m", 3.0, "lat_lon", "csv").unwrap();
        let b = cluster(data, "dbscan", 250.0, "m", 3.0, "lat_lon", "csv").unwrap();
        assert_eq!(a, b);
        assert!(a.contains("\n4,0.000000,0.009000,noise,,,"), "{a}");
    }
}
