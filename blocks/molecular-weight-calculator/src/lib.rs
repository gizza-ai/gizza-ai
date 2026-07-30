//! gizza-ai/molecular-weight-calculator — chat skill block on the shared tool
//! abstraction.
//!
//! Computes the molar mass, monoisotopic (exact) mass, atom/element counts and
//! per-element mass composition of a chemical formula. The chat schema is
//! single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! delegates to `block_utils::run_skill` and returns the pretty result JSON from
//! `core::analyze_json`. Pure compute — no host calls, no network, no ML.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_molecular_weight_calculator_core::{DEFAULT_DECIMALS, MAX_DECIMALS};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    formula: String,
    #[serde(default)]
    decimals: Option<u32>,
}

/// Single source for the chat schema (and CLI). `formula` is required;
/// `decimals` is optional and defaults to `DEFAULT_DECIMALS`.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("formula")
                .required()
                .describe(
                    "The chemical (molecular) formula to analyze. Element symbols are \
                     case-sensitive (Co = cobalt, CO = carbon monoxide). Supports subscript \
                     counts (H2O), nested groups in ()/[]/{} with a multiplier (Ca(OH)2, \
                     K4[Fe(CN)6]), and hydrate/dot notation with a leading coefficient \
                     (CuSO4·5H2O, CuSO4.5H2O, Na2CO3*10H2O). SMILES is not supported.",
                ),
        )
        .param(
            Param::integer("decimals")
                .default(DEFAULT_DECIMALS as i64)
                .min(0.0)
                .max(MAX_DECIMALS as f64)
                .describe(
                    "How many decimal places to round the molar and monoisotopic masses to, \
                     0–10. Mass percentages are always shown to 2 decimals. Default 4.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/molecular-weight-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute molar mass, monoisotopic mass and elemental composition from a chemical formula.",
    skill(
        description = "Compute the molecular weight of a chemical formula. Pass formula (e.g. H2O, NaCl, C6H12O6, C8H10N4O2 for caffeine) and optionally decimals (0–10, default 4). Element symbols are case-sensitive. The parser supports subscript counts, nested groups in ()/[]/{} with a multiplier (Ca(OH)2, K4[Fe(CN)6]), and hydrate/dot notation with a leading coefficient (CuSO4·5H2O, CuSO4.5H2O). Returns the average molar_mass in g/mol, the monoisotopic (exact) mass in Da, the total atom_count and element_count, the Hill-notation formula, and a per-element composition table with each element's atomic weight, mass contribution and mass percent. SMILES input is not supported.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "molecular-weight-calculator", |a: Args| {
            gizza_ai_molecular_weight_calculator_core::analyze_json(
                &a.formula,
                a.decimals.unwrap_or(DEFAULT_DECIMALS),
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
                    "formula": { "type": "string", "description": "The chemical (molecular) formula to analyze. Element symbols are case-sensitive (Co = cobalt, CO = carbon monoxide). Supports subscript counts (H2O), nested groups in ()/[]/{} with a multiplier (Ca(OH)2, K4[Fe(CN)6]), and hydrate/dot notation with a leading coefficient (CuSO4·5H2O, CuSO4.5H2O, Na2CO3*10H2O). SMILES is not supported." },
                    "decimals": { "type": "integer", "minimum": 0, "maximum": 10, "default": 4, "description": "How many decimal places to round the molar and monoisotopic masses to, 0–10. Mass percentages are always shown to 2 decimals. Default 4." }
                },
                "required": ["formula"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
