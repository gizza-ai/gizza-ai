//! gizza-ai/tournament-bracket-generator — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_tournament_bracket_generator_core::{
    generate, BracketType, Options, OutputFormat, Seeding,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    participants: String,
    #[serde(default = "default_bracket_type")]
    bracket_type: String,
    #[serde(default = "default_seeding")]
    seeding: String,
    #[serde(default = "default_format")]
    output_format: String,
    #[serde(default)]
    third_place_match: bool,
    #[serde(default = "default_true")]
    grand_final_reset: bool,
    #[serde(default)]
    tournament_name: String,
    #[serde(default = "default_true")]
    include_summary: bool,
    #[serde(default)]
    seed: i64,
}
fn default_bracket_type() -> String {
    "single".to_string()
}
fn default_seeding() -> String {
    "standard".to_string()
}
fn default_format() -> String {
    "text".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("participants").required().describe(
                "The teams or players, one per line (a single comma-separated line also works). Example: 'Lions' then 'Tigers' then 'Bears'. Leading '-', '*' or '1.' list markers are stripped, blank lines and '#' comments are ignored, and names must be unique (case-insensitive). A plain count such as '16' expands to Team 1…Team 16. Needs 2 to 64 participants; the field is rounded up to the next power of two and the spare slots become byes.",
            ),
        )
        .param(
            Param::enumv("bracket_type", ["single", "double"])
                .default("single")
                .describe(
                    "single (default) is a straight knockout — one loss and you are out, n-1 matches. double adds a losers bracket so everyone gets a second chance, plus a grand final: 2n-2 matches.",
                ),
        )
        .param(
            Param::enumv("seeding", ["standard", "ordered", "random"])
                .default("standard")
                .describe(
                    "How entries are placed in the bracket. standard (default) treats the entered order as the seeding and uses the classic 1-vs-N layout (1v8, 4v5, 2v7, 3v6), so the top seeds get the byes and seeds 1 and 2 can only meet in the final. ordered fills the slots top to bottom, pairing entry 1 with entry 2 — use it when you already know the pairings you want. random shuffles the entries with the reproducible draw set by `seed`, then places them the standard way.",
                ),
        )
        .param(
            Param::enumv("output_format", ["text", "markdown", "csv", "json"])
                .default("text")
                .describe(
                    "Output format: text (default, a printable round-by-round bracket sheet), markdown (pipe table with Match/Bracket/Round columns), csv (match,bracket,round,round_name,side_a,side_a_seed,side_b,side_b_seed,status for spreadsheets), or json (array of match objects).",
                ),
        )
        .param(
            Param::boolean("third_place_match")
                .default(false)
                .describe(
                    "Add a third-place playoff between the two semifinal losers (default false). Single elimination only, and only when the bracket has semifinals — a double-elimination losers bracket already decides third place.",
                ),
        )
        .param(
            Param::boolean("grand_final_reset")
                .default(true)
                .describe(
                    "Double elimination only: include the conditional reset match played when the losers-bracket finalist wins the grand final and both sides end on one loss (default true). Turn it off if your grand final is a single decider.",
                ),
        )
        .param(
            Param::string("tournament_name")
                .default("")
                .describe(
                    "Optional title printed above the bracket, e.g. 'Spring Cup'. Empty (default) prints no title. Shown in the text and markdown formats; csv and json are always just the matches.",
                ),
        )
        .param(
            Param::boolean("include_summary")
                .default(true)
                .describe(
                    "Prepend the summary line (format, participants, bracket size, byes, rounds, match count) and the seed list (default true). Applies to the text and markdown formats; csv and json are always just the matches.",
                ),
        )
        .param(
            Param::integer("seed")
                .default(0)
                .min(0.0)
                .describe(
                    "Draw seed used when seeding=random; ignored otherwise. Every value including the default 0 is a valid reproducible draw — change it to reshuffle, keep it to get the identical bracket again.",
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
    name = "gizza-ai/tournament-bracket-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a single- or double-elimination tournament bracket with seeding and byes",
    skill(
        description = "Build a single- or double-elimination tournament bracket from a list of participants. `participants` takes one name per line (a comma-separated line, '-'/'*'/'1.' list markers, '#' comments and a bare count like '16' all work); 2 to 64 names, unique case-insensitively. The field is rounded up to the next power of two and the spare slots become byes, which standard seeding hands to the top seeds. bracket_type=single (default, n-1 matches) | double (losers bracket + grand final, 2n-2 matches, optional grand_final_reset). seeding=standard (default, classic 1-vs-N layout) | ordered (entry 1 plays entry 2) | random (reproducible shuffle set by seed). third_place_match adds a semifinal-losers playoff to a single-elimination bracket. tournament_name titles the sheet and include_summary toggles the summary and seed list. output_format=text (default) | markdown | csv | json. Unplayed slots are shown as 'Winner of M3' / 'Loser of M3' so the sheet is printable before anyone plays. Deterministic and runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "tournament-bracket-generator", |a: Args| {
            let opts = Options {
                bracket_type: BracketType::parse(&a.bracket_type)
                    .map_err(SkillError::InvalidArgs)?,
                seeding: Seeding::parse(&a.seeding).map_err(SkillError::InvalidArgs)?,
                format: OutputFormat::parse(&a.output_format).map_err(SkillError::InvalidArgs)?,
                third_place_match: a.third_place_match,
                grand_final_reset: a.grand_final_reset,
                tournament_name: a.tournament_name,
                include_summary: a.include_summary,
                seed: a.seed.unsigned_abs(),
            };
            generate(&a.participants, &opts).map_err(SkillError::InvalidArgs)
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
                    "participants": { "type": "string", "description": "The teams or players, one per line (a single comma-separated line also works). Example: 'Lions' then 'Tigers' then 'Bears'. Leading '-', '*' or '1.' list markers are stripped, blank lines and '#' comments are ignored, and names must be unique (case-insensitive). A plain count such as '16' expands to Team 1…Team 16. Needs 2 to 64 participants; the field is rounded up to the next power of two and the spare slots become byes." },
                    "bracket_type": { "type": "string", "enum": ["single", "double"], "default": "single", "description": "single (default) is a straight knockout — one loss and you are out, n-1 matches. double adds a losers bracket so everyone gets a second chance, plus a grand final: 2n-2 matches." },
                    "seeding": { "type": "string", "enum": ["standard", "ordered", "random"], "default": "standard", "description": "How entries are placed in the bracket. standard (default) treats the entered order as the seeding and uses the classic 1-vs-N layout (1v8, 4v5, 2v7, 3v6), so the top seeds get the byes and seeds 1 and 2 can only meet in the final. ordered fills the slots top to bottom, pairing entry 1 with entry 2 — use it when you already know the pairings you want. random shuffles the entries with the reproducible draw set by `seed`, then places them the standard way." },
                    "output_format": { "type": "string", "enum": ["text", "markdown", "csv", "json"], "default": "text", "description": "Output format: text (default, a printable round-by-round bracket sheet), markdown (pipe table with Match/Bracket/Round columns), csv (match,bracket,round,round_name,side_a,side_a_seed,side_b,side_b_seed,status for spreadsheets), or json (array of match objects)." },
                    "third_place_match": { "type": "boolean", "default": false, "description": "Add a third-place playoff between the two semifinal losers (default false). Single elimination only, and only when the bracket has semifinals — a double-elimination losers bracket already decides third place." },
                    "grand_final_reset": { "type": "boolean", "default": true, "description": "Double elimination only: include the conditional reset match played when the losers-bracket finalist wins the grand final and both sides end on one loss (default true). Turn it off if your grand final is a single decider." },
                    "tournament_name": { "type": "string", "default": "", "description": "Optional title printed above the bracket, e.g. 'Spring Cup'. Empty (default) prints no title. Shown in the text and markdown formats; csv and json are always just the matches." },
                    "include_summary": { "type": "boolean", "default": true, "description": "Prepend the summary line (format, participants, bracket size, byes, rounds, match count) and the seed list (default true). Applies to the text and markdown formats; csv and json are always just the matches." },
                    "seed": { "type": "integer", "minimum": 0, "default": 0, "description": "Draw seed used when seeding=random; ignored otherwise. Every value including the default 0 is a valid reproducible draw — change it to reshuffle, keep it to get the identical bracket again." }
                },
                "required": ["participants"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
