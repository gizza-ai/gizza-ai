//! gizza-ai/dna-reverse-complement — reverse-complements a DNA/RNA sequence.
//!
//! Thin chat-skill wrapper around `gizza-ai-dna-reverse-complement-core`. The chat
//! schema is derived from `descriptor()` (single source — shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_dna_reverse_complement_core as core;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    sequence: String,
    /// "reverse_complement" (default) | "complement" | "reverse".
    #[serde(default)]
    operation: String,
    /// "auto" (default) | "dna" | "rna".
    #[serde(default)]
    output_alphabet: String,
    /// Keep the input's upper/lower case (default true).
    #[serde(default = "default_true")]
    preserve_case: bool,
    /// Wrap output lines at N characters; 0 (default) = one line per sequence.
    #[serde(default)]
    line_width: u32,
    /// "error" (default) | "drop" | "keep".
    #[serde(default)]
    on_invalid: String,
    /// Append a composition summary (default false).
    #[serde(default)]
    show_stats: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("sequence")
                .required()
                .describe("The DNA or RNA sequence to transform. Raw bases or FASTA (one or more '>' records) are both accepted; spaces, tabs and line breaks between bases are ignored. Accepts A/C/G/T/U, the IUPAC ambiguity codes R Y S W K M B D H V N, and the gap symbols '-' and '.'. Example: 'ATGCTTA'. Maximum 1000000 characters."),
        )
        .param(
            Param::enumv("operation", ["reverse_complement", "complement", "reverse"])
                .default("reverse_complement")
                .describe("Which transform to apply. 'reverse_complement' (default) complements every base and reverses the order — the opposite strand read 5'->3'. 'complement' complements in place without reversing. 'reverse' only reverses the order of the bases."),
        )
        .param(
            Param::enumv("output_alphabet", ["auto", "dna", "rna"])
                .default("auto")
                .describe("Alphabet of the output. 'auto' (default) keeps the input's: RNA (U) if the input contains U and no T, otherwise DNA (T). 'dna' forces U to be written as T; 'rna' forces T to be written as U."),
        )
        .param(
            Param::boolean("preserve_case")
                .default(true)
                .describe("When true (default) the input's upper/lower case is kept, so lower-case regions stay marked. Set false to uppercase the whole output."),
        )
        .param(
            // Bounds reference the core cap so the LLM-facing schema can't drift
            // from what `convert` actually enforces.
            Param::integer("line_width")
                .default(0)
                .min(0.0)
                .max(core::MAX_LINE_WIDTH as f64)
                .describe("Wrap each output sequence at this many characters per line, 0-200. 0 (default) puts each sequence on one line; 60 is the usual FASTA convention."),
        )
        .param(
            Param::enumv("on_invalid", ["error", "drop", "keep"])
                .default("error")
                .describe("What to do with a character that is not a base, IUPAC code, or gap (digits from numbered listings, '*', punctuation). 'error' (default) rejects the input and names the character and its position; 'drop' deletes it; 'keep' passes it through untouched (it is still repositioned by a reverse)."),
        )
        .param(
            Param::boolean("show_stats")
                .default(false)
                .describe("When true, append a '#'-prefixed summary after the sequence: record count, length, GC content, ambiguous-code count and gap count. Default false so the output stays copy-paste-ready."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Map the string/number args onto the core's typed options.
fn to_options(a: &Args) -> Result<core::Options, String> {
    Ok(core::Options {
        operation: core::parse_operation(&a.operation)?,
        output_alphabet: core::parse_alphabet(&a.output_alphabet)?,
        preserve_case: a.preserve_case,
        line_width: a.line_width as usize,
        on_invalid: core::parse_on_invalid(&a.on_invalid)?,
        show_stats: a.show_stats,
    })
}

#[cfg(target_arch = "wasm32")]
struct DnaReverseComplement;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/dna-reverse-complement",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reverse-complement a DNA or RNA sequence, IUPAC codes and case included.",
    skill(
        description = "Return the reverse complement of a DNA or RNA sequence. Give it raw bases or FASTA (multiple '>' records are transformed individually, headers kept); whitespace between bases is ignored. The full IUPAC alphabet is supported with the standard degenerate pairings (A<->T, C<->G, U->A, R<->Y, K<->M, B<->V, D<->H, while S, W and N are self-complementary) and gaps ('-', '.') map to themselves. Use operation='complement' to complement without reversing or operation='reverse' to reverse without complementing. output_alphabet='auto' (default) keeps RNA as RNA and DNA as DNA; force it with 'dna' or 'rna'. Case is preserved unless preserve_case=false. Set line_width=60 for FASTA-style wrapping, on_invalid='drop' to strip digits/punctuation from a numbered paste, and show_stats=true for length and GC content. Example: sequence='ATGC' returns 'GCAT'.",
        parameters = schema_json()
    )
)]
impl DnaReverseComplement {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "dna-reverse-complement", |a: Args| {
            let opts = to_options(&a).map_err(SkillError::InvalidArgs)?;
            core::convert(&a.sequence, &opts).map_err(SkillError::InvalidArgs)
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
                    "sequence": { "type": "string", "description": "The DNA or RNA sequence to transform. Raw bases or FASTA (one or more '>' records) are both accepted; spaces, tabs and line breaks between bases are ignored. Accepts A/C/G/T/U, the IUPAC ambiguity codes R Y S W K M B D H V N, and the gap symbols '-' and '.'. Example: 'ATGCTTA'. Maximum 1000000 characters." },
                    "operation": { "type": "string", "enum": ["reverse_complement", "complement", "reverse"], "default": "reverse_complement", "description": "Which transform to apply. 'reverse_complement' (default) complements every base and reverses the order — the opposite strand read 5'->3'. 'complement' complements in place without reversing. 'reverse' only reverses the order of the bases." },
                    "output_alphabet": { "type": "string", "enum": ["auto", "dna", "rna"], "default": "auto", "description": "Alphabet of the output. 'auto' (default) keeps the input's: RNA (U) if the input contains U and no T, otherwise DNA (T). 'dna' forces U to be written as T; 'rna' forces T to be written as U." },
                    "preserve_case": { "type": "boolean", "default": true, "description": "When true (default) the input's upper/lower case is kept, so lower-case regions stay marked. Set false to uppercase the whole output." },
                    "line_width": { "type": "integer", "minimum": 0, "maximum": 200, "default": 0, "description": "Wrap each output sequence at this many characters per line, 0-200. 0 (default) puts each sequence on one line; 60 is the usual FASTA convention." },
                    "on_invalid": { "type": "string", "enum": ["error", "drop", "keep"], "default": "error", "description": "What to do with a character that is not a base, IUPAC code, or gap (digits from numbered listings, '*', punctuation). 'error' (default) rejects the input and names the character and its position; 'drop' deletes it; 'keep' passes it through untouched (it is still repositioned by a reverse)." },
                    "show_stats": { "type": "boolean", "default": false, "description": "When true, append a '#'-prefixed summary after the sequence: record count, length, GC content, ambiguous-code count and gap count. Default false so the output stays copy-paste-ready." }
                },
                "required": ["sequence"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_defaults_produce_the_documented_options() {
        let a: Args = serde_json::from_str(r#"{"sequence":"ACGT"}"#).unwrap();
        let o = to_options(&a).unwrap();
        assert_eq!(o, core::Options::default());
        assert_eq!(core::convert(&a.sequence, &o).unwrap(), "ACGT");
    }

    #[test]
    fn an_unknown_enum_value_is_rejected_by_name() {
        let a: Args =
            serde_json::from_str(r#"{"sequence":"ACGT","operation":"revcomp"}"#).unwrap();
        let err = to_options(&a).unwrap_err();
        assert!(err.contains("invalid operation"), "{err}");
    }
}
