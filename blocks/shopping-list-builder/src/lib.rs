//! gizza-ai/shopping-list-builder — combine recipe ingredient lines into a
//! deduplicated, category-grouped shopping list.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    ingredients: String,
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default = "default_group_by")]
    group_by: String,
    #[serde(default = "default_unit_system")]
    unit_system: String,
    #[serde(default)]
    exclude: String,
    #[serde(default)]
    checkboxes: bool,
    #[serde(default)]
    show_sources: bool,
    #[serde(default = "default_format")]
    format: String,
}

fn default_scale() -> f64 {
    1.0
}
fn default_group_by() -> String {
    "category".to_string()
}
fn default_unit_system() -> String {
    "keep".to_string()
}
fn default_format() -> String {
    "markdown".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("ingredients").required().multiline().describe("Recipe ingredient lines to combine. Use one ingredient per line; start recipes with headers like `# Chili x2` and separate recipes with `---`."))
        .param(Param::number("scale").default(1.0).min(0.1).max(20.0).describe("Global multiplier applied to every quantity after any per-recipe `xN` multiplier. Default 1; allowed range 0.1 to 20."))
        .param(Param::enumv("group_by", ["category", "recipe", "none"]).default("category").describe("How to group the result: grocery category (default), source recipe, or no headings."))
        .param(Param::enumv("unit_system", ["keep", "metric", "us"]).default("keep").describe("How to render summed quantities: keep the input unit where possible, convert weight/volume families to metric, or convert to US kitchen units. Volume and weight are never cross-converted."))
        .param(Param::string("exclude").default("").describe("Comma- or newline-separated pantry staples to omit, such as salt, pepper, water, olive oil."))
        .param(Param::boolean("checkboxes").default(false).describe("When format=markdown, render each item as a GitHub-style checklist line (`- [ ]`). Default false."))
        .param(Param::boolean("show_sources").default(false).describe("Append the recipe names that contributed each merged item. Useful when auditing why something is on the list. Default false."))
        .param(Param::enumv("format", ["markdown", "text", "csv", "json"]).default("markdown").describe("Output format: markdown (default), plain text, spreadsheet-friendly CSV, or JSON."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/shopping-list-builder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a merged shopping list from recipe ingredients",
    skill(
        description = "Aggregate one or more pasted recipe ingredient lists into a deduplicated shopping list. It understands recipe headers like `# Tacos x2`, scales quantities globally, sums matching volume, weight and count units, groups by grocery category or recipe, can omit pantry staples, and exports markdown, text, CSV or JSON. It runs locally and does not fetch recipe URLs.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "shopping-list-builder", |a: Args| {
            gizza_ai_shopping_list_builder_core::run(
                &a.ingredients,
                a.scale,
                &a.group_by,
                &a.unit_system,
                &a.exclude,
                a.checkboxes,
                a.show_sources,
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
          "type":"object",
          "properties":{
            "ingredients":{"type":"string","description":"Recipe ingredient lines to combine. Use one ingredient per line; start recipes with headers like `# Chili x2` and separate recipes with `---`."},
            "scale":{"type":"number","default":1.0,"minimum":0.1,"maximum":20,"description":"Global multiplier applied to every quantity after any per-recipe `xN` multiplier. Default 1; allowed range 0.1 to 20."},
            "group_by":{"type":"string","enum":["category","recipe","none"],"default":"category","description":"How to group the result: grocery category (default), source recipe, or no headings."},
            "unit_system":{"type":"string","enum":["keep","metric","us"],"default":"keep","description":"How to render summed quantities: keep the input unit where possible, convert weight/volume families to metric, or convert to US kitchen units. Volume and weight are never cross-converted."},
            "exclude":{"type":"string","default":"","description":"Comma- or newline-separated pantry staples to omit, such as salt, pepper, water, olive oil."},
            "checkboxes":{"type":"boolean","default":false,"description":"When format=markdown, render each item as a GitHub-style checklist line (`- [ ]`). Default false."},
            "show_sources":{"type":"boolean","default":false,"description":"Append the recipe names that contributed each merged item. Useful when auditing why something is on the list. Default false."},
            "format":{"type":"string","enum":["markdown","text","csv","json"],"default":"markdown","description":"Output format: markdown (default), plain text, spreadsheet-friendly CSV, or JSON."}
          },
          "required":["ingredients"],
          "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
