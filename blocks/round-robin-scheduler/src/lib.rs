//! gizza-ai/round-robin-scheduler — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_round_robin_scheduler_core::{generate, Options, OutputFormat, ScheduleType};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    participants: String,
    #[serde(default = "default_schedule_type")]
    schedule_type: String,
    #[serde(default = "default_format")]
    output_format: String,
    #[serde(default)]
    courts: String,
    #[serde(default = "default_start_round")]
    start_round: i64,
    #[serde(default = "default_true")]
    include_byes: bool,
    #[serde(default = "default_true")]
    include_summary: bool,
    #[serde(default)]
    seed: i64,
}
fn default_schedule_type() -> String {
    "single".to_string()
}
fn default_format() -> String {
    "text".to_string()
}
fn default_start_round() -> i64 {
    1
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("participants").required().describe(
                "The teams or players, one per line (a single comma-separated line also works). Example: 'Alice' then 'Bob' then 'Carol'. Leading '-', '*' or '1.' list markers are stripped, blank lines and '#' comments are ignored, and names must be unique (case-insensitive). A plain count such as '8' expands to Team 1…Team 8. Needs 2 to 64 participants.",
            ),
        )
        .param(
            Param::enumv("schedule_type", ["single", "double"])
                .default("single")
                .describe(
                    "single (default) plays every pair once over n-1 rounds; double plays every pair twice, appending a mirrored return leg with home and away swapped.",
                ),
        )
        .param(
            Param::enumv("output_format", ["text", "markdown", "csv", "json"])
                .default("text")
                .describe(
                    "Output format: text (default, a round-by-round listing), markdown (pipe table with a Round column), csv (round,match,home,away[,court] for spreadsheets), or json (flat array of fixture objects).",
                ),
        )
        .param(
            Param::string("courts")
                .default("")
                .describe(
                    "Courts, fields or venues for the parallel matches of each round. Either a count such as '4' (labelled Court 1…Court 4) or a comma-separated list such as 'North Field, South Field'. Empty (default) omits court assignment; at most 32.",
                ),
        )
        .param(
            Param::integer("start_round")
                .default(1)
                .min(1.0)
                .describe("Number given to the first round; later rounds count up from it. Default 1 — set it to continue an existing fixture list."),
        )
        .param(
            Param::boolean("include_byes")
                .default(true)
                .describe("With an odd number of participants, show who sits out each round (default true). Turn it off for a matches-only list."),
        )
        .param(
            Param::boolean("include_summary")
                .default(true)
                .describe("Prepend a summary of participants, rounds, matches and byes (default true). Applies to the text and markdown formats; csv and json are always just the fixtures."),
        )
        .param(
            Param::integer("seed")
                .default(0)
                .min(0.0)
                .describe("0 (default) keeps the entered order. Any other number deterministically shuffles the draw — the same seed always produces the same schedule."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/round-robin-scheduler",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a balanced round-robin match schedule from a list of participants",
    skill(
        description = "Generate a balanced round-robin match schedule from a list of participants. `participants` takes one name per line (a comma-separated line, '-'/'*'/'1.' list markers, '#' comments and a bare count like '8' all work); 2 to 64 names, unique case-insensitively. The circle method pairs everyone with everyone over n-1 rounds, an odd roster gets a rotating BYE so each participant rests exactly once, and home/away is balanced across the fixture list. schedule_type=single (default) | double (mirrored return leg, home and away swapped). courts assigns each round's parallel matches to a count ('4') or named venues ('North Field, South Field'). start_round renumbers the first round; include_byes and include_summary toggle the bye lines and the summary block. output_format=text (default) | markdown | csv | json. Deterministic — the same input always yields the same schedule; set seed to a non-zero number for a reproducible shuffled draw. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "round-robin-scheduler", |a: Args| {
            let opts = Options {
                schedule_type: ScheduleType::parse(&a.schedule_type)
                    .map_err(SkillError::InvalidArgs)?,
                format: OutputFormat::parse(&a.output_format).map_err(SkillError::InvalidArgs)?,
                courts: a.courts,
                start_round: a.start_round,
                include_byes: a.include_byes,
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
                    "participants": { "type": "string", "description": "The teams or players, one per line (a single comma-separated line also works). Example: 'Alice' then 'Bob' then 'Carol'. Leading '-', '*' or '1.' list markers are stripped, blank lines and '#' comments are ignored, and names must be unique (case-insensitive). A plain count such as '8' expands to Team 1…Team 8. Needs 2 to 64 participants." },
                    "schedule_type": { "type": "string", "enum": ["single", "double"], "default": "single", "description": "single (default) plays every pair once over n-1 rounds; double plays every pair twice, appending a mirrored return leg with home and away swapped." },
                    "output_format": { "type": "string", "enum": ["text", "markdown", "csv", "json"], "default": "text", "description": "Output format: text (default, a round-by-round listing), markdown (pipe table with a Round column), csv (round,match,home,away[,court] for spreadsheets), or json (flat array of fixture objects)." },
                    "courts": { "type": "string", "default": "", "description": "Courts, fields or venues for the parallel matches of each round. Either a count such as '4' (labelled Court 1…Court 4) or a comma-separated list such as 'North Field, South Field'. Empty (default) omits court assignment; at most 32." },
                    "start_round": { "type": "integer", "minimum": 1, "default": 1, "description": "Number given to the first round; later rounds count up from it. Default 1 — set it to continue an existing fixture list." },
                    "include_byes": { "type": "boolean", "default": true, "description": "With an odd number of participants, show who sits out each round (default true). Turn it off for a matches-only list." },
                    "include_summary": { "type": "boolean", "default": true, "description": "Prepend a summary of participants, rounds, matches and byes (default true). Applies to the text and markdown formats; csv and json are always just the fixtures." },
                    "seed": { "type": "integer", "minimum": 0, "default": 0, "description": "0 (default) keeps the entered order. Any other number deterministically shuffles the draw — the same seed always produces the same schedule." }
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
