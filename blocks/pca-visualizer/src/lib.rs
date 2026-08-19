//! gizza-ai/pca-visualizer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_scale() -> bool {
    true
}
fn default_perplexity() -> f64 {
    30.0
}
fn default_iterations() -> f64 {
    500.0
}
fn default_learning_rate() -> f64 {
    200.0
}
fn default_point_size() -> f64 {
    4.0
}
fn default_width() -> f64 {
    720.0
}
fn default_height() -> f64 {
    520.0
}

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    method: String,
    #[serde(default)]
    label_column: String,
    #[serde(default = "default_scale")]
    scale: bool,
    #[serde(default = "default_perplexity")]
    perplexity: f64,
    #[serde(default = "default_iterations")]
    iterations: f64,
    #[serde(default = "default_learning_rate")]
    learning_rate: f64,
    #[serde(default)]
    show_labels: bool,
    #[serde(default = "default_point_size")]
    point_size: f64,
    #[serde(default)]
    title: String,
    #[serde(default = "default_width")]
    width: f64,
    #[serde(default = "default_height")]
    height: f64,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The table to project: one observation per row, one variable per column, separated by commas, tabs, semicolons, pipes or spaces. A header row is detected automatically. One non-numeric column may hold the class label. Limits: 5000 rows and 100 numeric columns (1000 rows for t-SNE), minimum 3 rows and 2 numeric columns."),
        )
        .param(
            Param::enumv("method", ["pca", "tsne"])
                .default("pca")
                .describe("Which 2-D projection to compute: pca (default) is linear and keeps global structure, with each axis labelled by the share of variance it explains; tsne is non-linear and pulls local neighbourhoods apart into visible clusters. Both are deterministic — t-SNE is seeded from the PCA scores, so the same table always gives the same picture."),
        )
        .param(
            Param::string("label_column")
                .describe("Header name or 1-based column index of the column holding each row's class/group label, e.g. \"species\" or \"5\". It is dropped from the maths and used to colour the points and build the legend. Empty (the default) auto-detects the single non-numeric column, if the table has exactly one."),
        )
        .param(
            Param::boolean("scale")
                .default(true)
                .describe("Standardize every numeric column to unit variance before projecting (correlation-matrix PCA). Default true, which is what you want when the variables are in different units. Set false to keep the raw units so high-variance columns dominate (covariance-matrix PCA)."),
        )
        .param(
            Param::number("perplexity")
                .default(30)
                .min(1.0)
                .max(100.0)
                .describe("t-SNE only: roughly how many neighbours each point tries to keep close, 1 to 100. Default 30. Lower values expose small tight clusters, higher values favour the broad shape. Automatically clamped to (rows - 1) / 3 on small tables. Ignored when method=pca."),
        )
        .param(
            Param::integer("iterations")
                .default(500)
                .min(50.0)
                .max(2000.0)
                .describe("t-SNE only: gradient-descent iterations, 50 to 2000. Default 500. More iterations settle the layout further at a linear cost in time; the first quarter runs with early exaggeration to separate clusters. Ignored when method=pca."),
        )
        .param(
            Param::number("learning_rate")
                .default(200)
                .min(1.0)
                .max(1000.0)
                .describe("t-SNE only: gradient-descent step size, 1 to 1000. Default 200. Too low leaves the points in a dense ball, too high scatters them into a uniform cloud. Ignored when method=pca."),
        )
        .param(
            Param::boolean("show_labels")
                .default(false)
                .describe("Draw each point's label text next to its marker, on top of the colour coding. Default false. Only usable up to 200 points, above which the text is unreadable and the tool refuses it."),
        )
        .param(
            Param::number("point_size")
                .default(4)
                .min(1.0)
                .max(20.0)
                .describe("Marker radius in pixels, 1 to 20. Default 4. Drop to 2 for a few hundred crowded points, raise to 6-8 for a small table or a chart that will be viewed from a distance."),
        )
        .param(
            Param::string("title")
                .describe("Optional chart title drawn across the top of the SVG, e.g. \"Iris measurements — PCA\". Empty (the default) leaves the plot area untitled and reclaims the space."),
        )
        .param(
            Param::integer("width")
                .default(720)
                .min(300.0)
                .max(2000.0)
                .describe("SVG width in pixels, 300 to 2000. Default 720. The legend takes a fixed strip on the right, so widen the chart when the label names are long. Ignored when format=csv or json."),
        )
        .param(
            Param::integer("height")
                .default(520)
                .min(200.0)
                .max(2000.0)
                .describe("SVG height in pixels, 200 to 2000. Default 520. Ignored when format=csv or json."),
        )
        .param(
            Param::enumv("format", ["svg", "csv", "json"])
                .default("svg")
                .describe("Output format: svg (default) is a standalone scatter plot with axes, legend and grid; csv returns index,label,pc1,pc2 (tsne1,tsne2 for t-SNE) coordinate rows for another chart tool; json returns the full projection — coordinates, categories, variable names and the explained variance per axis."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pca-visualizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Project a high-dimensional table to 2-D with PCA or t-SNE and plot it as a labelled SVG scatter.",
    skill(
        description = "Reduce a wide numeric table to two dimensions and draw it. Paste the table as `data` — one observation per row, one variable per column, commas/tabs/semicolons/pipes/spaces all work, and a header row is detected automatically. method='pca' (default) runs a deterministic Jacobi eigen-decomposition and labels each axis with the share of variance it explains; method='tsne' runs a PCA-seeded t-distributed stochastic neighbor embedding, tuned with perplexity, iterations and learning_rate. One non-numeric column is used as the class label: it colours the points and builds the legend, auto-detected or named through label_column (header name or 1-based index). scale standardizes the variables first (default true, correlation PCA); set false for covariance PCA. Tune the picture with show_labels, point_size, title, width and height. format='svg' (default) returns a standalone scatter plot, 'csv' returns index,label,pc1,pc2 coordinate rows, and 'json' returns the whole projection including explained variance. Fully deterministic — no RNG — so the same table always yields byte-identical output. Limits: 5000 rows and 100 numeric variables for PCA, 1000 rows for t-SNE.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pca-visualizer", |a: Args| {
            gizza_ai_pca_visualizer_core::run(
                &a.data,
                &a.method,
                &a.label_column,
                a.scale,
                a.perplexity,
                a.iterations,
                a.learning_rate,
                a.show_labels,
                a.point_size,
                &a.title,
                a.width,
                a.height,
                &a.format,
            )
            .map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The table to project: one observation per row, one variable per column, separated by commas, tabs, semicolons, pipes or spaces. A header row is detected automatically. One non-numeric column may hold the class label. Limits: 5000 rows and 100 numeric columns (1000 rows for t-SNE), minimum 3 rows and 2 numeric columns." },
                    "method": { "type": "string", "enum": ["pca", "tsne"], "default": "pca", "description": "Which 2-D projection to compute: pca (default) is linear and keeps global structure, with each axis labelled by the share of variance it explains; tsne is non-linear and pulls local neighbourhoods apart into visible clusters. Both are deterministic — t-SNE is seeded from the PCA scores, so the same table always gives the same picture." },
                    "label_column": { "type": "string", "description": "Header name or 1-based column index of the column holding each row's class/group label, e.g. \"species\" or \"5\". It is dropped from the maths and used to colour the points and build the legend. Empty (the default) auto-detects the single non-numeric column, if the table has exactly one." },
                    "scale": { "type": "boolean", "default": true, "description": "Standardize every numeric column to unit variance before projecting (correlation-matrix PCA). Default true, which is what you want when the variables are in different units. Set false to keep the raw units so high-variance columns dominate (covariance-matrix PCA)." },
                    "perplexity": { "type": "number", "default": 30, "minimum": 1, "maximum": 100, "description": "t-SNE only: roughly how many neighbours each point tries to keep close, 1 to 100. Default 30. Lower values expose small tight clusters, higher values favour the broad shape. Automatically clamped to (rows - 1) / 3 on small tables. Ignored when method=pca." },
                    "iterations": { "type": "integer", "default": 500, "minimum": 50, "maximum": 2000, "description": "t-SNE only: gradient-descent iterations, 50 to 2000. Default 500. More iterations settle the layout further at a linear cost in time; the first quarter runs with early exaggeration to separate clusters. Ignored when method=pca." },
                    "learning_rate": { "type": "number", "default": 200, "minimum": 1, "maximum": 1000, "description": "t-SNE only: gradient-descent step size, 1 to 1000. Default 200. Too low leaves the points in a dense ball, too high scatters them into a uniform cloud. Ignored when method=pca." },
                    "show_labels": { "type": "boolean", "default": false, "description": "Draw each point's label text next to its marker, on top of the colour coding. Default false. Only usable up to 200 points, above which the text is unreadable and the tool refuses it." },
                    "point_size": { "type": "number", "default": 4, "minimum": 1, "maximum": 20, "description": "Marker radius in pixels, 1 to 20. Default 4. Drop to 2 for a few hundred crowded points, raise to 6-8 for a small table or a chart that will be viewed from a distance." },
                    "title": { "type": "string", "description": "Optional chart title drawn across the top of the SVG, e.g. \"Iris measurements — PCA\". Empty (the default) leaves the plot area untitled and reclaims the space." },
                    "width": { "type": "integer", "default": 720, "minimum": 300, "maximum": 2000, "description": "SVG width in pixels, 300 to 2000. Default 720. The legend takes a fixed strip on the right, so widen the chart when the label names are long. Ignored when format=csv or json." },
                    "height": { "type": "integer", "default": 520, "minimum": 200, "maximum": 2000, "description": "SVG height in pixels, 200 to 2000. Default 520. Ignored when format=csv or json." },
                    "format": { "type": "string", "enum": ["svg", "csv", "json"], "default": "svg", "description": "Output format: svg (default) is a standalone scatter plot with axes, legend and grid; csv returns index,label,pc1,pc2 (tsne1,tsne2 for t-SNE) coordinate rows for another chart tool; json returns the full projection — coordinates, categories, variable names and the explained variance per axis." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
