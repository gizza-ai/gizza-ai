//! gizza-ai/geo-cluster — cluster latitude/longitude points by real ground
//! distance (haversine) with DBSCAN or a latitude-compensated grid, and report
//! the per-point cluster assignment plus each cluster's centroid.
//! Chat schema single-sourced from descriptor() (which also drives the CLI);
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_geo_cluster_core::cluster;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    points: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_radius")]
    radius: f64,
    #[serde(default = "default_units")]
    units: String,
    #[serde(default = "default_min_points")]
    min_points: f64,
    #[serde(default = "default_coord_order")]
    coord_order: String,
    #[serde(default = "default_output")]
    output: String,
}

fn default_method() -> String {
    "dbscan".into()
}
fn default_radius() -> f64 {
    500.0
}
fn default_units() -> String {
    "m".into()
}
fn default_min_points() -> f64 {
    2.0
}
fn default_coord_order() -> String {
    "lat_lon".into()
}
fn default_output() -> String {
    "text".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("points")
                .required()
                .describe("The points to cluster, in any one of three forms: CSV/TSV rows of `lat,lon` with an optional third label field (e.g. '51.5,-0.1,Shop'), a JSON array of pairs or objects (e.g. [[51.5,-0.1],{\"lat\":51.51,\"lng\":-0.1,\"label\":\"Depot\"}]), or GeoJSON (a Point, MultiPoint, Feature, FeatureCollection or GeometryCollection — a feature's name/label/title property becomes the point label). The form is auto-detected. Blank lines, '#' comments and one header row are skipped. Maximum 10000 points."),
        )
        .param(
            Param::enumv("method", ["dbscan", "grid"])
                .default("dbscan")
                .describe("Clustering algorithm. 'dbscan' (default) is density clustering: a point with at least min_points neighbours inside radius is a core point, cores that reach each other merge into one cluster of any shape, points merely reached by a core join as border points, and everything else is unclustered noise. 'grid' bins points into fixed cells radius across (cell width divided by cos(latitude) so cells stay roughly square on the ground) and calls every cell holding at least min_points points a cluster — cheaper and reproducible, but clusters can never span a cell boundary."),
        )
        .param(
            Param::number("radius")
                .default(500.0)
                .describe("Neighbourhood radius for dbscan, or the cell size for grid, expressed in units — a real ground distance measured with the haversine formula, not degrees. Default 500 (metres). Pick roughly the distance at which two points count as 'the same place': ~50-200 m for store visits, ~1-5 km for city districts. Must be positive and no larger than half the Earth's circumference."),
        )
        .param(
            Param::enumv("units", ["m", "km", "mi", "ft", "nmi"])
                .default("m")
                .describe("Distance unit for radius and for every distance in the output: 'm' metres (default), 'km' kilometres, 'mi' statute miles, 'ft' feet, 'nmi' nautical miles."),
        )
        .param(
            Param::integer("min_points")
                .min(1.0)
                .default(2)
                .describe("Minimum number of points a cluster must contain, counting the point itself (the DBSCAN core-point threshold, as in PostGIS and scikit-learn). Default 2. Raise it to demand denser groups and push sparse points into the unclustered class; set it to 1 to keep every point, so nothing is ever reported as noise."),
        )
        .param(
            Param::enumv("coord_order", ["lat_lon", "lon_lat"])
                .default("lat_lon")
                .describe("Which value comes first in the CSV rows and JSON pairs: 'lat_lon' (default) reads '51.5,-0.1' as latitude then longitude; 'lon_lat' reads it as longitude then latitude. GeoJSON input always uses [longitude, latitude] per RFC 7946 and ignores this setting, as do JSON objects with named lat/lon fields."),
        )
        .param(
            Param::enumv("output", ["text", "csv", "json", "geojson"])
                .default("text")
                .describe("Result format. 'text' (default) is a readable summary: counts, then one line per cluster with its size, centroid and radius. 'csv' is one row per input point with its cluster id (or 'noise'), cluster size and centroid — paste-ready for a spreadsheet. 'json' carries the full result: every point with its cluster (null for noise) plus per-cluster centroid, radius, bounding box and member list. 'geojson' is a FeatureCollection of the input points (each tagged with its cluster or 'noise') followed by one centroid feature per cluster, ready to drop on a map."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_args(a: &Args) -> Result<String, String> {
    cluster(
        &a.points,
        &a.method,
        a.radius,
        &a.units,
        a.min_points,
        &a.coord_order,
        &a.output,
    )
}

#[cfg(target_arch = "wasm32")]
struct GeoCluster;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/geo-cluster",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Cluster lat/lon points by ground distance and return assignments plus centroids",
    skill(
        description = "Group latitude/longitude points that sit near each other on the ground and report which cluster each point belongs to, plus each cluster's centroid. Distance is haversine great-circle distance, so radius is a real ground distance in the unit you choose (m, km, mi, ft, nmi) rather than degrees — one degree of longitude is ~111 km at the equator but ~28 km at 75° latitude, which is why clustering raw lat/lon columns in Euclidean space gets the wrong answer. method='dbscan' (default) is density clustering: a point with min_points neighbours inside radius is a core point, cores that reach each other merge into arbitrarily shaped clusters, and points that meet no core are reported as unclustered noise. method='grid' instead bins points into fixed cells radius across, latitude-compensated so cells stay roughly square. points accepts CSV `lat,lon[,label]` rows, a JSON array of pairs or {lat,lon,label} objects, or GeoJSON (Point/MultiPoint/Feature/FeatureCollection); coord_order switches CSV and JSON pairs between lat,lon and lon,lat. output is 'text' (default summary), 'csv' (one row per point), 'json' (points plus per-cluster centroid, radius, bbox and members) or 'geojson' (point features plus centroid features). Centroids are spherical means, so a cluster straddling the antimeridian is handled correctly, and results are deterministic: clusters are numbered by their first member in input order and a border point joins the first cluster that reaches it. Up to 10000 points. Runs locally.",
        parameters = schema_json()
    ),
)]
impl GeoCluster {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "geo-cluster", |a: Args| {
            run_args(&a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "points":      { "type": "string", "description": "The points to cluster, in any one of three forms: CSV/TSV rows of `lat,lon` with an optional third label field (e.g. '51.5,-0.1,Shop'), a JSON array of pairs or objects (e.g. [[51.5,-0.1],{\"lat\":51.51,\"lng\":-0.1,\"label\":\"Depot\"}]), or GeoJSON (a Point, MultiPoint, Feature, FeatureCollection or GeometryCollection — a feature's name/label/title property becomes the point label). The form is auto-detected. Blank lines, '#' comments and one header row are skipped. Maximum 10000 points." },
                    "method":      { "type": "string", "enum": ["dbscan", "grid"], "default": "dbscan", "description": "Clustering algorithm. 'dbscan' (default) is density clustering: a point with at least min_points neighbours inside radius is a core point, cores that reach each other merge into one cluster of any shape, points merely reached by a core join as border points, and everything else is unclustered noise. 'grid' bins points into fixed cells radius across (cell width divided by cos(latitude) so cells stay roughly square on the ground) and calls every cell holding at least min_points points a cluster — cheaper and reproducible, but clusters can never span a cell boundary." },
                    "radius":      { "type": "number", "default": 500.0, "description": "Neighbourhood radius for dbscan, or the cell size for grid, expressed in units — a real ground distance measured with the haversine formula, not degrees. Default 500 (metres). Pick roughly the distance at which two points count as 'the same place': ~50-200 m for store visits, ~1-5 km for city districts. Must be positive and no larger than half the Earth's circumference." },
                    "units":       { "type": "string", "enum": ["m", "km", "mi", "ft", "nmi"], "default": "m", "description": "Distance unit for radius and for every distance in the output: 'm' metres (default), 'km' kilometres, 'mi' statute miles, 'ft' feet, 'nmi' nautical miles." },
                    "min_points":  { "type": "integer", "minimum": 1, "default": 2, "description": "Minimum number of points a cluster must contain, counting the point itself (the DBSCAN core-point threshold, as in PostGIS and scikit-learn). Default 2. Raise it to demand denser groups and push sparse points into the unclustered class; set it to 1 to keep every point, so nothing is ever reported as noise." },
                    "coord_order": { "type": "string", "enum": ["lat_lon", "lon_lat"], "default": "lat_lon", "description": "Which value comes first in the CSV rows and JSON pairs: 'lat_lon' (default) reads '51.5,-0.1' as latitude then longitude; 'lon_lat' reads it as longitude then latitude. GeoJSON input always uses [longitude, latitude] per RFC 7946 and ignores this setting, as do JSON objects with named lat/lon fields." },
                    "output":      { "type": "string", "enum": ["text", "csv", "json", "geojson"], "default": "text", "description": "Result format. 'text' (default) is a readable summary: counts, then one line per cluster with its size, centroid and radius. 'csv' is one row per input point with its cluster id (or 'noise'), cluster size and centroid — paste-ready for a spreadsheet. 'json' carries the full result: every point with its cluster (null for noise) plus per-cluster centroid, radius, bounding box and member list. 'geojson' is a FeatureCollection of the input points (each tagged with its cluster or 'noise') followed by one centroid feature per cluster, ready to drop on a map." }
                },
                "required": ["points"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn defaults_run_dbscan_in_metres() {
        let a: Args = serde_json::from_str(r#"{"points":"51.5,-0.1\n51.5008,-0.1"}"#).unwrap();
        assert_eq!(a.method, "dbscan");
        assert_eq!(a.radius, 500.0);
        assert_eq!(a.units, "m");
        assert_eq!(a.min_points, 2.0);
        assert_eq!(a.coord_order, "lat_lon");
        assert_eq!(a.output, "text");
        let out = run_args(&a).unwrap();
        assert!(out.starts_with("2 points · 1 cluster · 0 unclustered"), "{out}");
    }

    #[test]
    fn args_map_through_to_the_core() {
        let a: Args = serde_json::from_str(
            r#"{"points":"[[51.5,-0.1],[51.55,-0.1]]","method":"grid","radius":10,
                "units":"km","min_points":1,"coord_order":"lat_lon","output":"json"}"#,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&run_args(&a).unwrap()).unwrap();
        assert_eq!(v["method"], "grid");
        assert_eq!(v["units"], "km");
        assert_eq!(v["radius"], 10.0);
        assert_eq!(v["min_points"], 1);
        assert_eq!(v["point_count"], 2);
    }

    #[test]
    fn invalid_args_surface_the_core_error() {
        let a: Args = serde_json::from_str(r#"{"points":"51.5,-0.1","method":"kmeans"}"#).unwrap();
        assert!(run_args(&a).unwrap_err().contains("invalid method 'kmeans'"));
    }
}
