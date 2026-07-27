//! gizza-ai/data-clusterer — cluster the numeric columns of a tabular (CSV)
//! dataset with KMeans, DBSCAN, or hierarchical clustering and visualize the
//! result. The chat schema is single-sourced from `descriptor()` (which also
//! drives the CLI); `handle()` delegates to the pure `core::run`. Pure compute →
//! runs on every backend including the chat Service Worker.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_data_clusterer_core::{run, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_clusters")]
    clusters: u32,
    #[serde(default = "default_eps")]
    eps: f64,
    #[serde(default = "default_min_samples")]
    min_samples: u32,
    #[serde(default = "default_linkage")]
    linkage: String,
    #[serde(default)]
    columns: String,
    #[serde(default = "default_true")]
    normalize: bool,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default)]
    title: String,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
}

fn default_method() -> String {
    "kmeans".into()
}
fn default_clusters() -> u32 {
    3
}
fn default_eps() -> f64 {
    1.0
}
fn default_min_samples() -> u32 {
    4
}
fn default_linkage() -> String {
    "average".into()
}
fn default_true() -> bool {
    true
}
fn default_output() -> String {
    "chart".into()
}
fn default_width() -> u32 {
    700
}
fn default_height() -> u32 {
    500
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The tabular data to cluster, as CSV text. Include a header row when you can (e.g. `height,weight`); a first row that is not entirely numeric is treated as headers. Quoted fields with embedded commas are handled."),
        )
        .param(
            Param::enumv("method", ["kmeans", "dbscan", "hierarchical"])
                .default("kmeans")
                .describe("Clustering algorithm: 'kmeans' (partition into `clusters` groups by nearest centroid), 'dbscan' (density-based; finds clusters via `eps`/`min_samples` and marks outliers as noise), or 'hierarchical' (agglomerative merging down to `clusters` groups). Default kmeans."),
        )
        .param(
            Param::integer("clusters")
                .min(1.0)
                .max(50.0)
                .default(3)
                .describe("Number of clusters: k for KMeans, or the target cluster count for hierarchical. Ignored by DBSCAN (which discovers the count). Default 3."),
        )
        .param(
            Param::number("eps")
                .default(1.0)
                .describe("DBSCAN only: neighbourhood radius. Two points are neighbours when their distance is ≤ eps (in standardized units when normalize is on). Larger eps merges more points. Default 1.0."),
        )
        .param(
            Param::integer("min_samples")
                .min(1.0)
                .default(4)
                .describe("DBSCAN only: minimum neighbourhood size (including the point itself) for a core point. Higher values require denser regions and label more points as noise. Default 4."),
        )
        .param(
            Param::enumv("linkage", ["average", "complete", "single", "ward"])
                .default("average")
                .describe("Hierarchical only: how cluster distances are combined when merging — 'average' (UPGMA), 'complete' (max), 'single' (min), or 'ward' (minimum variance). Default average."),
        )
        .param(
            Param::string("columns")
                .default("")
                .describe("Comma-separated feature columns to cluster on: header names (case-insensitive) or 1-based indices (e.g. `height,weight` or `2,3`). Blank uses every fully-numeric column. Distance is Euclidean."),
        )
        .param(
            Param::boolean("normalize")
                .default(true)
                .describe("Standardize each feature to zero mean and unit variance before clustering so columns on different scales contribute comparably. Default true."),
        )
        .param(
            Param::enumv("output", ["chart", "csv", "json"])
                .default("chart")
                .describe("Result format: 'chart' (an SVG scatter plot coloured by cluster, with centroids and a legend; 2 features plot directly, more than 2 are projected to 2D via PCA), 'csv' (each row plus its cluster label), or 'json' (cluster sizes, centroids, and the silhouette score). Default chart."),
        )
        .param(
            Param::string("title")
                .default("")
                .describe("Optional chart title drawn above the plot (chart output only)."),
        )
        .param(
            Param::integer("width")
                .min(200.0)
                .max(4000.0)
                .default(700)
                .describe("Chart width in pixels (200–4000). Default 700."),
        )
        .param(
            Param::integer("height")
                .min(150.0)
                .max(4000.0)
                .default(500)
                .describe("Chart height in pixels (150–4000). Default 500."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options(a: &Args) -> Options {
    Options {
        method: a.method.clone(),
        clusters: a.clusters,
        eps: a.eps,
        min_samples: a.min_samples,
        linkage: a.linkage.clone(),
        columns: a.columns.clone(),
        normalize: a.normalize,
        output: a.output.clone(),
        title: a.title.clone(),
        width: a.width,
        height: a.height,
    }
}

#[cfg(target_arch = "wasm32")]
struct DataClusterer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/data-clusterer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Cluster CSV data with KMeans, DBSCAN, or hierarchical methods and visualize it",
    skill(
        description = "Cluster the numeric columns of a CSV/tabular dataset and visualize the result — entirely in the browser, nothing uploaded. Choose KMeans (k groups by nearest centroid), DBSCAN (density-based, with automatic outlier/noise detection via eps and min_samples), or hierarchical/agglomerative clustering (merged down to a target count, with average/complete/single/Ward linkage). Feature columns are chosen by header name or 1-based index, or auto-detected (every fully-numeric column); non-numeric rows are skipped. Features can be z-score standardized so mixed-scale columns compare fairly. Output is a self-contained SVG scatter plot coloured by cluster (with centroid markers and a legend; datasets with more than two features are projected to 2D with PCA), a CSV of each row plus its cluster label, or a JSON report with cluster sizes, centroids, and the silhouette quality score. Distance is Euclidean; results are deterministic.",
        parameters = schema_json()
    ),
)]
impl DataClusterer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "data-clusterer", |a: Args| {
            run(&a.data, &options(&a)).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// copy, so an accidental descriptor edit can't silently change the
    /// LLM-facing schema (and the page controls the manifest renders from it).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The tabular data to cluster, as CSV text. Include a header row when you can (e.g. `height,weight`); a first row that is not entirely numeric is treated as headers. Quoted fields with embedded commas are handled." },
                    "method": { "type": "string", "enum": ["kmeans", "dbscan", "hierarchical"], "default": "kmeans", "description": "Clustering algorithm: 'kmeans' (partition into `clusters` groups by nearest centroid), 'dbscan' (density-based; finds clusters via `eps`/`min_samples` and marks outliers as noise), or 'hierarchical' (agglomerative merging down to `clusters` groups). Default kmeans." },
                    "clusters": { "type": "integer", "minimum": 1, "maximum": 50, "default": 3, "description": "Number of clusters: k for KMeans, or the target cluster count for hierarchical. Ignored by DBSCAN (which discovers the count). Default 3." },
                    "eps": { "type": "number", "default": 1.0, "description": "DBSCAN only: neighbourhood radius. Two points are neighbours when their distance is ≤ eps (in standardized units when normalize is on). Larger eps merges more points. Default 1.0." },
                    "min_samples": { "type": "integer", "minimum": 1, "default": 4, "description": "DBSCAN only: minimum neighbourhood size (including the point itself) for a core point. Higher values require denser regions and label more points as noise. Default 4." },
                    "linkage": { "type": "string", "enum": ["average", "complete", "single", "ward"], "default": "average", "description": "Hierarchical only: how cluster distances are combined when merging — 'average' (UPGMA), 'complete' (max), 'single' (min), or 'ward' (minimum variance). Default average." },
                    "columns": { "type": "string", "default": "", "description": "Comma-separated feature columns to cluster on: header names (case-insensitive) or 1-based indices (e.g. `height,weight` or `2,3`). Blank uses every fully-numeric column. Distance is Euclidean." },
                    "normalize": { "type": "boolean", "default": true, "description": "Standardize each feature to zero mean and unit variance before clustering so columns on different scales contribute comparably. Default true." },
                    "output": { "type": "string", "enum": ["chart", "csv", "json"], "default": "chart", "description": "Result format: 'chart' (an SVG scatter plot coloured by cluster, with centroids and a legend; 2 features plot directly, more than 2 are projected to 2D via PCA), 'csv' (each row plus its cluster label), or 'json' (cluster sizes, centroids, and the silhouette score). Default chart." },
                    "title": { "type": "string", "default": "", "description": "Optional chart title drawn above the plot (chart output only)." },
                    "width": { "type": "integer", "minimum": 200, "maximum": 4000, "default": 700, "description": "Chart width in pixels (200–4000). Default 700." },
                    "height": { "type": "integer", "minimum": 150, "maximum": 4000, "default": 500, "description": "Chart height in pixels (150–4000). Default 500." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
