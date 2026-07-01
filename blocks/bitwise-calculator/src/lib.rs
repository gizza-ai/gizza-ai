//! gizza-ai/bitwise-calculator — bitwise AND/OR/XOR/NOT, shifts, rotates and
//! popcount on integers at a chosen bit width (8/16/32/64).
//!
//! Thin chat-skill wrapper around `gizza-ai-bitwise-calculator-core`. The chat
//! schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. No host calls — runs entirely
//! inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    a: String,
    /// "" defaults to "and" (matches the schema default).
    #[serde(default)]
    op: String,
    /// Second operand or shift/rotate count; unused for not/popcount.
    #[serde(default)]
    b: String,
    /// "" defaults to "32" (matches the schema default).
    #[serde(default)]
    bits: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("a")
                .required()
                .describe("First operand. Plain digits are decimal; prefix 0x for hex, 0b for binary, 0o for octal (e.g. 87, 0x57, 0b0101_0111). Underscores/spaces between digits are allowed; a leading '-' (e.g. -8) is read as two's complement at the chosen bit width."),
        )
        .param(
            Param::enumv(
                "op",
                ["and", "or", "xor", "not", "shl", "shr", "rotl", "rotr", "popcount"],
            )
            .default("and")
            .describe("Operation. 'and'/'or'/'xor' combine a with b; 'not' inverts every bit of a; 'shl'/'shr' shift a left/right by b bits (logical, zero-fill); 'rotl'/'rotr' rotate a left/right by b bits (count wraps modulo the width); 'popcount' counts a's set bits. Default 'and'."),
        )
        .param(
            Param::string("b")
                .describe("Second operand for and/or/xor (same formats as 'a'), or the shift/rotate count for shl/shr/rotl/rotr (a non-negative integer, e.g. 3). Ignored for not/popcount."),
        )
        .param(
            Param::enumv("bits", ["8", "16", "32", "64"])
                .default("32")
                .describe("Bit width of the operation: 8, 16, 32 or 64. Operands must fit the width; the result is masked to it and also shown as signed two's complement. Default 32."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bitwise-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Bitwise AND/OR/XOR/NOT, shifts, rotates and popcount at 8/16/32/64-bit widths.",
    skill(
        description = "Bitwise calculator for integers: AND, OR, XOR, NOT, logical left/right shifts (shl/shr), left/right rotates (rotl/rotr) and set-bit count (popcount) at a chosen bit width (8/16/32/64, default 32). Operands accept decimal, hex (0x…), binary (0b…) or octal (0o…) with optional _ separators; negative decimals are two's complement at the width. Returns the result rendered in binary (nibble-grouped), octal, decimal, hex and signed two's complement — e.g. a=87 op=and b=101 bits=8 → binary 0100 0101, decimal 69, hex 0x45.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "bitwise-calculator", |a: Args| {
            gizza_ai_bitwise_calculator_core::compute(&a.a, &a.op, &a.b, &a.bits)
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
    /// reviewed. Authored 2026-07-02 with the initial tool.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "a": { "type": "string", "description": "First operand. Plain digits are decimal; prefix 0x for hex, 0b for binary, 0o for octal (e.g. 87, 0x57, 0b0101_0111). Underscores/spaces between digits are allowed; a leading '-' (e.g. -8) is read as two's complement at the chosen bit width." },
                    "op": { "type": "string", "enum": ["and", "or", "xor", "not", "shl", "shr", "rotl", "rotr", "popcount"], "default": "and", "description": "Operation. 'and'/'or'/'xor' combine a with b; 'not' inverts every bit of a; 'shl'/'shr' shift a left/right by b bits (logical, zero-fill); 'rotl'/'rotr' rotate a left/right by b bits (count wraps modulo the width); 'popcount' counts a's set bits. Default 'and'." },
                    "b": { "type": "string", "description": "Second operand for and/or/xor (same formats as 'a'), or the shift/rotate count for shl/shr/rotl/rotr (a non-negative integer, e.g. 3). Ignored for not/popcount." },
                    "bits": { "type": "string", "enum": ["8", "16", "32", "64"], "default": "32", "description": "Bit width of the operation: 8, 16, 32 or 64. Operands must fit the width; the result is masked to it and also shown as signed two's complement. Default 32." }
                },
                "required": ["a"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
