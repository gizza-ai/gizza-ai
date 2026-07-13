//! gizza-ai/amazon-order-analyzer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    csv: String,
    #[serde(default)]
    output: String,
    /// 0 → the core default (10); the core clamps to 1..=MAX_TOP.
    #[serde(default)]
    top: u32,
}

/// Single source for the chat schema (and CLI + page). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("csv")
                .required()
                .describe("Your Amazon order-history CSV export, pasted whole (including the header row). Works with the classic Order History Report (Order Date, Title, Category, Quantity, Item Total, Order ID) and the newer Request My Data / Retail.OrderHistory export (Order Date, Product Name, Quantity, Total Owed). Column names are matched case-insensitively; currency symbols and thousands separators in amounts are tolerated."),
        )
        .param(
            Param::enumv("output", ["summary", "json"])
                .default("summary")
                .describe("Output shape. 'summary' (default) is a Markdown report: a total-spend caption, a spend-by-month table, top items by spend, and a category breakdown (when a Category column is present); 'json' is a structured object with the same data."),
        )
        .param(
            // Bounds reference the core clamp (MAX_TOP) so the schema can't drift
            // from what `analyze` enforces.
            Param::integer("top")
                .default(10)
                .min(1.0)
                .max(gizza_ai_amazon_order_analyzer_core::MAX_TOP as f64)
                .describe("How many top items (by total spend) to list (1-50). Line items are grouped by their exact title/product name and quantities summed. Default 10."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/amazon-order-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Summarize an Amazon order-history CSV: total spend by month, top items, and category breakdown.",
    skill(
        description = "Parse an Amazon order-history CSV export and summarize spending. Handles both common export shapes — the classic Order History Report (Order Date, Title, Category, Quantity, Item Total, Order ID) and the newer Request My Data / Retail.OrderHistory export (Order Date, Product Name, Quantity, Total Owed) — matching columns case-insensitively and tolerating currency symbols/thousands separators in amounts. Reports: total spend with item and order counts and date range, a spend-by-month table, the top items ranked by spend (grouped by title), and a per-category breakdown when the export includes a Category column. output='summary' (default) is Markdown; 'json' is a structured object. top caps the item list (default 10, max 50). Runs entirely in the sandbox — no upload, no network.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "amazon-order-analyzer", |a: Args| {
            gizza_ai_amazon_order_analyzer_core::analyze(&a.csv, &a.output, a.top)
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
                    "csv": { "type": "string", "description": "Your Amazon order-history CSV export, pasted whole (including the header row). Works with the classic Order History Report (Order Date, Title, Category, Quantity, Item Total, Order ID) and the newer Request My Data / Retail.OrderHistory export (Order Date, Product Name, Quantity, Total Owed). Column names are matched case-insensitively; currency symbols and thousands separators in amounts are tolerated." },
                    "output": { "type": "string", "enum": ["summary", "json"], "default": "summary", "description": "Output shape. 'summary' (default) is a Markdown report: a total-spend caption, a spend-by-month table, top items by spend, and a category breakdown (when a Category column is present); 'json' is a structured object with the same data." },
                    "top": { "type": "integer", "minimum": 1, "maximum": 50, "default": 10, "description": "How many top items (by total spend) to list (1-50). Line items are grouped by their exact title/product name and quantities summed. Default 10." }
                },
                "required": ["csv"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
