//! gizza-ai/normalize-country — resolves messy country names and codes to
//! canonical ISO 3166-1 forms.
//!
//! Thin chat-skill wrapper around `gizza-ai-normalize-country-core`. The chat
//! schema is derived from `descriptor()` (single source — shared shape across
//! chat + CLI); the handler delegates to `block_utils::run_skill`. No host calls
//! — the ISO 3166-1 table is compiled in, so it runs entirely inside the WASM
//! sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    name_style: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    on_unmatched: String,
    /// Collapse repeats of the same country (default false).
    #[serde(default)]
    dedupe: bool,
    #[serde(default)]
    sort: String,
    /// Allow the edit-distance layer to fix typos (default true).
    #[serde(default = "default_true")]
    fuzzy: bool,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The countries to normalize — one or many, in any mix of forms: ISO alpha-2 ('US'), alpha-3 ('USA'), numeric ('840' or '0840'), ISO short name, everyday name ('United States'), former name ('Burma'), endonym ('Deutschland'), or a comma-inverted register form ('Korea, Republic of'). Up to 1000 items per run, split by the delimiter param."),
        )
        .param(
            Param::enumv(
                "output",
                ["table", "name", "alpha2", "alpha3", "numeric", "flag", "csv", "json"],
            )
            .default("table")
            .describe("Result shape. 'table' (default) is an aligned column view with every canonical form plus how each row matched; 'name'/'alpha2'/'alpha3'/'numeric'/'flag' emit just that one value per line (numeric is zero-padded to three digits); 'csv' and 'json' carry every field plus the match status for a spreadsheet or a script."),
        )
        .param(
            Param::enumv("name_style", ["iso", "common"])
                .default("iso")
                .describe("Which name to emit. 'iso' (default) is the ISO 3166-1 English short name, e.g. 'Korea (Republic of)'; 'common' is the everyday English name, e.g. 'South Korea'. Only affects name output — codes are identical either way."),
        )
        .param(
            Param::enumv("delimiter", ["auto", "newline", "comma", "semicolon", "pipe", "tab"])
                .default("auto")
                .describe("How to split a batch. 'auto' (default) splits on newlines when the input has any — so a pasted column keeps commas inside names like 'Korea, Republic of' — and otherwise splits a single line on commas, semicolons, pipes and tabs. Set one explicitly to override."),
        )
        .param(
            Param::enumv("on_unmatched", ["keep", "blank", "omit", "only"])
                .default("keep")
                .describe("What to do with rows that resolve to no single country. 'keep' (default) echoes the original text so a converted column stays row-aligned with the source; 'blank' leaves an empty cell; 'omit' drops the row; 'only' returns just the problem rows, for auditing a list before you import it."),
        )
        .param(
            Param::boolean("dedupe")
                .default(false)
                .describe("When true, keep only the first row per country, so 'USA', 'U.S.' and 'united states' collapse to one. Unmatched rows dedupe by their own text. Default false."),
        )
        .param(
            Param::enumv("sort", ["input", "asc", "desc"])
                .default("input")
                .describe("Row order. 'input' (default) preserves the order you supplied; 'asc'/'desc' sort case-insensitively by the value being emitted (the code for code outputs, the name otherwise). Default 'input'."),
        )
        .param(
            Param::boolean("fuzzy")
                .default(true)
                .describe("When true (default), a near miss within a small edit-distance budget is accepted and reported as a 'fuzzy' match, so 'Swizerland' resolves to Switzerland. Two equally-close candidates are reported 'ambiguous' rather than guessed. Set false to accept only exact codes, ISO names and curated aliases."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct NormalizeCountry;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/normalize-country",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Normalize messy country names and codes to canonical ISO 3166-1 forms.",
    skill(
        description = "Normalize messy country names and codes to canonical ISO 3166-1 forms — English short name, alpha-2, alpha-3, numeric and flag emoji. Accepts any mix of codes ('US', 'USA', '840'), ISO names, everyday names ('United States'), former names ('Burma', 'Zaire', 'Swaziland'), endonyms ('Deutschland', 'Nihon'), UK constituent countries ('Scotland' -> GB), demonyms ('Swiss'), and comma-inverted forms ('Korea, Republic of'). Batch up to 1000 items in one call. Every row is tagged exact/alias/fuzzy/ambiguous/unmatched so unparseable entries are flagged instead of guessed. Use output=alpha2|alpha3|numeric|name|flag for a single-column list, csv/json for every field, or the default table view. name_style=common gives 'South Korea' instead of ISO's 'Korea (Republic of)'. on_unmatched=only audits a list for entries that will not import.",
        parameters = schema_json()
    )
)]
impl NormalizeCountry {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "normalize-country", |a: Args| {
            gizza_ai_normalize_country_core::normalize(
                &a.input,
                &a.output,
                &a.name_style,
                &a.delimiter,
                &a.on_unmatched,
                a.dedupe,
                &a.sort,
                a.fuzzy,
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
    /// reviewed. Authored 2026-08-29 with the tool.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The countries to normalize — one or many, in any mix of forms: ISO alpha-2 ('US'), alpha-3 ('USA'), numeric ('840' or '0840'), ISO short name, everyday name ('United States'), former name ('Burma'), endonym ('Deutschland'), or a comma-inverted register form ('Korea, Republic of'). Up to 1000 items per run, split by the delimiter param." },
                    "output": { "type": "string", "enum": ["table", "name", "alpha2", "alpha3", "numeric", "flag", "csv", "json"], "default": "table", "description": "Result shape. 'table' (default) is an aligned column view with every canonical form plus how each row matched; 'name'/'alpha2'/'alpha3'/'numeric'/'flag' emit just that one value per line (numeric is zero-padded to three digits); 'csv' and 'json' carry every field plus the match status for a spreadsheet or a script." },
                    "name_style": { "type": "string", "enum": ["iso", "common"], "default": "iso", "description": "Which name to emit. 'iso' (default) is the ISO 3166-1 English short name, e.g. 'Korea (Republic of)'; 'common' is the everyday English name, e.g. 'South Korea'. Only affects name output — codes are identical either way." },
                    "delimiter": { "type": "string", "enum": ["auto", "newline", "comma", "semicolon", "pipe", "tab"], "default": "auto", "description": "How to split a batch. 'auto' (default) splits on newlines when the input has any — so a pasted column keeps commas inside names like 'Korea, Republic of' — and otherwise splits a single line on commas, semicolons, pipes and tabs. Set one explicitly to override." },
                    "on_unmatched": { "type": "string", "enum": ["keep", "blank", "omit", "only"], "default": "keep", "description": "What to do with rows that resolve to no single country. 'keep' (default) echoes the original text so a converted column stays row-aligned with the source; 'blank' leaves an empty cell; 'omit' drops the row; 'only' returns just the problem rows, for auditing a list before you import it." },
                    "dedupe": { "type": "boolean", "default": false, "description": "When true, keep only the first row per country, so 'USA', 'U.S.' and 'united states' collapse to one. Unmatched rows dedupe by their own text. Default false." },
                    "sort": { "type": "string", "enum": ["input", "asc", "desc"], "default": "input", "description": "Row order. 'input' (default) preserves the order you supplied; 'asc'/'desc' sort case-insensitively by the value being emitted (the code for code outputs, the name otherwise). Default 'input'." },
                    "fuzzy": { "type": "boolean", "default": true, "description": "When true (default), a near miss within a small edit-distance budget is accepted and reported as a 'fuzzy' match, so 'Swizerland' resolves to Switzerland. Two equally-close candidates are reported 'ambiguous' rather than guessed. Set false to accept only exact codes, ISO names and curated aliases." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
