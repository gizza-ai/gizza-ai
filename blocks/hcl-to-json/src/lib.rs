//! gizza-ai/hcl-to-json — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. No host calls — parsing
//! and rendering both happen inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    hcl: String,
    /// Blank falls back to the core's documented default ("nested"); same for
    /// every other string option below.
    #[serde(default)]
    blocks: String,
    #[serde(default)]
    expressions: String,
    #[serde(default)]
    sort_keys: bool,
    #[serde(default = "default_pretty")]
    pretty: bool,
    #[serde(default)]
    indent: String,
}

fn default_pretty() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("hcl")
                .required()
                .multiline()
                .describe("The HCL / HCL2 configuration text to convert — a Terraform .tf or .tfvars file, or a Packer, Nomad, Vault or Consul config. Example: resource \"aws_instance\" \"web\" { ami = \"ami-0123\" instance_type = \"t3.micro\" }. Capped at 1,048,576 bytes."),
        )
        .param(
            Param::enumv("blocks", ["nested", "arrays"])
                .default("nested")
                .describe("How a block body is shaped. 'nested' (default) writes a plain object for a block header that occurs once and an array when the same header repeats — the shape Terraform's own JSON configuration syntax uses. 'arrays' always wraps every block body in an array, so a script sees one stable shape no matter how many times a block occurs."),
        )
        .param(
            Param::enumv("expressions", ["template", "simplify"])
                .default("template")
                .describe("What to do with expressions JSON cannot hold (var.x, upper(s), a ? b : c). 'template' (default) writes them as Terraform-style \"${…}\" interpolation strings, preserving the source verbatim. 'simplify' first evaluates whatever is constant, so 1 + 2 becomes 3 and [1, 2 * 3, var.n] becomes [1, 6, \"${var.n}\"]."),
        )
        .param(
            Param::boolean("sort_keys")
                .default(false)
                .describe("Sort every object's keys alphabetically, at every level, instead of keeping the order they appear in the source. Useful when diffing two configurations. Default false."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Print the JSON across multiple indented lines. Turn it off for a single-line minified document. Default true."),
        )
        .param(
            Param::enumv("indent", ["2", "4", "tab"])
                .default("2")
                .describe("Indent unit used when pretty printing: '2' spaces (default), '4' spaces, or 'tab' for a real tab character. Ignored when pretty is false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/hcl-to-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert HashiCorp HCL2 configuration (Terraform, Packer, Nomad) to JSON.",
    skill(
        description = "Convert HashiCorp Configuration Language (HCL2) text — Terraform .tf/.tfvars, Packer, Nomad, Vault, Consul — into the equivalent JSON document. An attribute becomes a JSON property; a block becomes nested objects keyed by the block type and then by each label, and a repeated block header becomes an array (set blocks='arrays' to wrap every block body in an array instead, giving one stable machine-readable shape). Expressions JSON cannot represent — variable references, function calls, conditionals — are written as Terraform-style \"${…}\" interpolation strings, the same convention Terraform's own JSON syntax uses; expressions='simplify' folds the constant parts first (1 + 2 becomes 3). sort_keys sorts every level alphabetically; pretty and indent control the layout. Comments are dropped because JSON has none, and a syntax error reports the line and what was expected. Input is capped at 1,048,576 bytes.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "hcl-to-json", |a: Args| {
            gizza_ai_hcl_to_json_core::convert(
                &a.hcl,
                &a.blocks,
                &a.expressions,
                a.sort_keys,
                a.pretty,
                &a.indent,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "hcl": { "type": "string", "description": "The HCL / HCL2 configuration text to convert — a Terraform .tf or .tfvars file, or a Packer, Nomad, Vault or Consul config. Example: resource \"aws_instance\" \"web\" { ami = \"ami-0123\" instance_type = \"t3.micro\" }. Capped at 1,048,576 bytes." },
                    "blocks": { "type": "string", "enum": ["nested", "arrays"], "default": "nested", "description": "How a block body is shaped. 'nested' (default) writes a plain object for a block header that occurs once and an array when the same header repeats — the shape Terraform's own JSON configuration syntax uses. 'arrays' always wraps every block body in an array, so a script sees one stable shape no matter how many times a block occurs." },
                    "expressions": { "type": "string", "enum": ["template", "simplify"], "default": "template", "description": "What to do with expressions JSON cannot hold (var.x, upper(s), a ? b : c). 'template' (default) writes them as Terraform-style \"${…}\" interpolation strings, preserving the source verbatim. 'simplify' first evaluates whatever is constant, so 1 + 2 becomes 3 and [1, 2 * 3, var.n] becomes [1, 6, \"${var.n}\"]." },
                    "sort_keys": { "type": "boolean", "default": false, "description": "Sort every object's keys alphabetically, at every level, instead of keeping the order they appear in the source. Useful when diffing two configurations. Default false." },
                    "pretty": { "type": "boolean", "default": true, "description": "Print the JSON across multiple indented lines. Turn it off for a single-line minified document. Default true." },
                    "indent": { "type": "string", "enum": ["2", "4", "tab"], "default": "2", "description": "Indent unit used when pretty printing: '2' spaces (default), '4' spaces, or 'tab' for a real tab character. Ignored when pretty is false." }
                },
                "required": ["hcl"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The page/CLI send blank strings for untouched optional selects; every
    /// option must therefore have a documented blank fallback in the core.
    #[test]
    fn every_option_accepts_a_blank_value() {
        let out =
            gizza_ai_hcl_to_json_core::convert("a = 1\n", "", "", false, false, "").expect("blank");
        assert_eq!(out, r#"{"a":1}"#);
    }
}
