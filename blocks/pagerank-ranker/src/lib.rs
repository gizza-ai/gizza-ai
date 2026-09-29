//! gizza-ai/pagerank-ranker — chat skill block on the shared tool abstraction.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_pagerank_ranker_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

fn d_input_format() -> String {
    "auto".into()
}
fn d_true() -> bool {
    true
}
fn d_false() -> bool {
    false
}
fn d_damping() -> f64 {
    0.85
}
fn d_max_iter() -> u32 {
    100
}
fn d_tolerance() -> f64 {
    0.000001
}
fn d_dangling() -> String {
    "redistribute".into()
}
fn d_zero() -> u32 {
    0
}
fn d_decimals() -> u32 {
    6
}
fn d_format() -> String {
    "text".into()
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "d_input_format")]
    input_format: String,
    #[serde(default = "d_true")]
    directed: bool,
    #[serde(default = "d_false")]
    weighted: bool,
    #[serde(default = "d_damping")]
    damping: f64,
    #[serde(default = "d_max_iter")]
    max_iter: u32,
    #[serde(default = "d_tolerance")]
    tolerance: f64,
    #[serde(default = "d_dangling")]
    dangling: String,
    #[serde(default)]
    personalization: String,
    #[serde(default = "d_zero")]
    top: u32,
    #[serde(default = "d_decimals")]
    decimals: u32,
    #[serde(default = "d_format")]
    format: String,
}

impl From<Args> for Options {
    fn from(a: Args) -> Self {
        Options {
            input_format: a.input_format,
            directed: a.directed,
            weighted: a.weighted,
            damping: a.damping,
            max_iter: a.max_iter,
            tolerance: a.tolerance,
            dangling: a.dangling,
            personalization: a.personalization,
            top: a.top,
            decimals: a.decimals,
            format: a.format,
        }
    }
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().multiline().describe("Graph as an edge list (one edge per line, e.g. `a -> b` or `a,b,2`) or a square adjacency matrix. Blank lines and # comments are ignored."))
        .param(Param::enumv("input_format", ["auto", "edge-list", "matrix"]).default("auto").describe("How to read the input. auto treats a square all-numeric paste as a matrix and everything else as an edge list."))
        .param(Param::boolean("directed").default(true).describe("Treat edges as one-way links. Turn off to symmetrise each edge into both directions."))
        .param(Param::boolean("weighted").default(false).describe("Use edge-list weight columns or matrix cell values as weights. When false, every nonzero edge counts as weight 1."))
        .param(Param::number("damping").default(0.85).min(0.0).max(0.999).describe("PageRank damping factor: probability of following a link instead of teleporting. Default 0.85."))
        .param(Param::integer("max_iter").default(100).min(1.0).max(10000.0).describe("Maximum power-iteration steps before reporting non-convergence. Default 100."))
        .param(Param::number("tolerance").default(0.000001).min(0.000000000001).describe("Convergence threshold on total L1 score change between iterations. Default 1e-6."))
        .param(Param::enumv("dangling", ["redistribute", "self-loop", "drop"]).default("redistribute").describe("How to handle dangling nodes with no outgoing links: redistribute their mass through teleportation, keep it as a self-loop, or drop it."))
        .param(Param::string("personalization").describe("Optional teleport weights as node:weight pairs, e.g. `home:0.7, docs:0.3`. Blank means uniform teleportation."))
        .param(Param::integer("top").default(0).min(0.0).max(5000.0).describe("Show only the top N ranked nodes. 0 shows all nodes."))
        .param(Param::integer("decimals").default(6).min(0.0).max(12.0).describe("Decimal places for PageRank scores."))
        .param(Param::enumv("format", ["text", "json", "csv"]).default("text").describe("Output format: readable table, JSON, or CSV."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pagerank-ranker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute PageRank scores from an edge list or adjacency matrix",
    skill(
        description = "Compute PageRank scores for a directed or undirected graph supplied as an edge list or adjacency matrix. Supports weighted edges, damping, convergence tolerance, dangling-node policy, personalization, top-N truncation, and text/JSON/CSV output. Reports ranks, scores, share of total mass, in/out degree, convergence status, and dangling nodes.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pagerank-ranker", |a: Args| {
            let input = a.input.clone();
            let opts: Options = a.into();
            gizza_ai_pagerank_ranker_core::run(&input, &opts).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the chat schema, the CLI surface and `manifest.json` all
    /// derive from this descriptor, so every param must stay named, described,
    /// and — where the choices are fixed — enumerated.
    #[test]
    fn schema_json_has_expected_parameters() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(v["type"], "object");
        assert_eq!(v["required"], serde_json::json!(["input"]));
        let props = v["properties"].as_object().unwrap();
        for name in [
            "input",
            "input_format",
            "directed",
            "weighted",
            "damping",
            "max_iter",
            "tolerance",
            "dangling",
            "personalization",
            "top",
            "decimals",
            "format",
        ] {
            assert!(props.contains_key(name), "missing {name} in schema");
            assert!(
                props[name]
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(|d| !d.trim().is_empty())
                    .unwrap_or(false),
                "missing description for {name}"
            );
        }
        assert_eq!(props.len(), 12, "unexpected parameter count: {props:?}");

        assert_eq!(
            props["input_format"]["enum"],
            serde_json::json!(["auto", "edge-list", "matrix"])
        );
        assert_eq!(
            props["dangling"]["enum"],
            serde_json::json!(["redistribute", "self-loop", "drop"])
        );
        assert_eq!(
            props["format"]["enum"],
            serde_json::json!(["text", "json", "csv"])
        );

        assert_eq!(props["directed"]["type"], "boolean");
        assert_eq!(props["directed"]["default"], serde_json::json!(true));
        assert_eq!(props["weighted"]["default"], serde_json::json!(false));
        assert_eq!(props["damping"]["default"], serde_json::json!(0.85));
        assert_eq!(props["max_iter"]["default"], serde_json::json!(100));
        assert_eq!(props["decimals"]["default"], serde_json::json!(6));
        assert_eq!(props["top"]["default"], serde_json::json!(0));
        assert_eq!(props["input_format"]["default"], "auto");
        assert_eq!(props["dangling"]["default"], "redistribute");
        assert_eq!(props["format"]["default"], "text");
    }

    /// The descriptor defaults and the core defaults must not drift apart —
    /// otherwise the page (which omits untouched fields) and the CLI disagree.
    #[test]
    fn descriptor_defaults_match_core_defaults() {
        let d = Options::default();
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = &v["properties"];
        assert_eq!(props["input_format"]["default"], d.input_format);
        assert_eq!(props["directed"]["default"], d.directed);
        assert_eq!(props["weighted"]["default"], d.weighted);
        assert_eq!(props["damping"]["default"], d.damping);
        assert_eq!(props["max_iter"]["default"], d.max_iter);
        assert_eq!(props["tolerance"]["default"], d.tolerance);
        assert_eq!(props["dangling"]["default"], d.dangling);
        assert_eq!(props["top"]["default"], d.top);
        assert_eq!(props["decimals"]["default"], d.decimals);
        assert_eq!(props["format"]["default"], d.format);
    }

    /// Chat sends only `input`; every other field must fall back to the same
    /// defaults the core uses.
    #[test]
    fn args_default_to_the_core_defaults() {
        let a: Args = serde_json::from_str(r#"{"input":"a -> b"}"#).unwrap();
        let got: Options = a.into();
        let want = Options::default();
        assert_eq!(got.input_format, want.input_format);
        assert_eq!(got.directed, want.directed);
        assert_eq!(got.weighted, want.weighted);
        assert_eq!(got.damping, want.damping);
        assert_eq!(got.max_iter, want.max_iter);
        assert_eq!(got.tolerance, want.tolerance);
        assert_eq!(got.dangling, want.dangling);
        assert_eq!(got.personalization, want.personalization);
        assert_eq!(got.top, want.top);
        assert_eq!(got.decimals, want.decimals);
        assert_eq!(got.format, want.format);
    }
}
