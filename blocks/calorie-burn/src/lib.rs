//! gizza-ai/calorie-burn — MET-based activity energy cost as a chat skill block on
//! the shared tool abstraction. The chat schema is single-sourced from descriptor()
//! (which also drives the CLI and, via manifest.json, the page form); handle()
//! delegates to block_utils::run_skill, which hands the parsed Args to the shared
//! core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    weight: f64,
    #[serde(default = "default_weight_unit")]
    weight_unit: String,
    duration: f64,
    #[serde(default = "default_duration_unit")]
    duration_unit: String,
    #[serde(default = "default_activity")]
    activity: String,
    #[serde(default)]
    met: f64,
    #[serde(default = "default_basis")]
    basis: String,
    #[serde(default = "default_output")]
    output: String,
}

fn default_weight_unit() -> String {
    "kg".into()
}
fn default_duration_unit() -> String {
    "minutes".into()
}
fn default_activity() -> String {
    "walking-moderate".into()
}
fn default_basis() -> String {
    "gross".into()
}
fn default_output() -> String {
    "summary".into()
}

/// Every accepted `activity` value, authored here so the dropdown order is
/// stable and the drift guard has something to compare the core's table with.
/// `custom` is last and means "use my own `met`".
const ACTIVITY_VALUES: [&str; 60] = [
    "sleeping",
    "sitting-desk-work",
    "standing-light-work",
    "cooking",
    "house-cleaning",
    "gardening",
    "mowing-lawn",
    "moving-furniture",
    "shoveling-snow",
    "stairs-climbing",
    "walking-slow",
    "walking-moderate",
    "walking-brisk",
    "walking-uphill",
    "hiking",
    "backpacking",
    "jogging",
    "running-6mph",
    "running-8mph",
    "running-10mph",
    "running-trail",
    "cycling-leisure",
    "cycling-moderate",
    "cycling-vigorous",
    "mountain-biking",
    "indoor-cycling-class",
    "swimming-leisure",
    "swimming-laps-moderate",
    "swimming-laps-vigorous",
    "water-aerobics",
    "kayaking",
    "rowing-water",
    "weight-training-light",
    "weight-training-vigorous",
    "circuit-training",
    "calisthenics-vigorous",
    "rowing-machine",
    "elliptical",
    "stair-stepper",
    "jump-rope",
    "yoga-hatha",
    "pilates",
    "stretching",
    "tai-chi",
    "basketball-game",
    "soccer-casual",
    "soccer-competitive",
    "tennis-singles",
    "tennis-doubles",
    "volleyball",
    "badminton",
    "table-tennis",
    "golf-walking",
    "bowling",
    "dancing-social",
    "dancing-aerobic",
    "martial-arts",
    "boxing-bag",
    "rock-climbing",
    "custom",
];

const WEIGHT_DESC: &str = "Body weight as a number, in the unit given by weight_unit. Required, because the MET formula scales energy cost directly with body mass: the same activity for the same time burns about 14% more in an 80 kg person than a 70 kg one. Must land between 20 and 300 kg (44.1 and 661.4 lb) once converted, so 70 with weight_unit kg and 154 with weight_unit lb are both fine. Use the person's current weight, not a goal weight.";
const WEIGHT_UNIT_DESC: &str = "Unit that weight is given in. kg (default) is kilograms; lb is international pounds, converted at 0.45359237 kg per pound before any arithmetic. The output echoes both, so 70 kg reports 154.3 lb as well.";
const DURATION_DESC: &str = "How long the activity lasted, as a number in the unit given by duration_unit. Required. Must land between 0.5 and 1440 minutes (up to 24 hours) once converted, so 45 with duration_unit minutes and 1.5 with duration_unit hours are both fine. Count moving time, not elapsed time including long rests, since MET values assume the effort is sustained throughout.";
const DURATION_UNIT_DESC: &str = "Unit that duration is given in. minutes (default) or hours; hours are multiplied by 60 before the calculation and the output echoes the minute figure, so 1.5 hours reports as 1.5 h (90 min).";
const ACTIVITY_DESC: &str = "Which activity to price, chosen from the 59-entry table of standard metabolic-equivalent values, or custom to supply your own met. Default walking-moderate (MET 3.5). Values are grouped by prefix so they are easy to guess: everyday and household (sleeping 0.95, sitting-desk-work 1.5, standing-light-work 2, cooking 3.3, house-cleaning 3.5, gardening 3.8, mowing-lawn 5, moving-furniture 5.8, shoveling-snow 6, stairs-climbing 8.8); walking and hiking (walking-slow 2.8, walking-moderate 3.5, walking-brisk 5, walking-uphill 6.3, hiking 6, backpacking 7); running (jogging 7, running-trail 9, running-6mph 9.8, running-8mph 11.8, running-10mph 14.5); cycling (cycling-leisure 4, cycling-moderate 8, mountain-biking 8.5, indoor-cycling-class 8.5, cycling-vigorous 12); water (kayaking 5, water-aerobics 5.5, swimming-leisure 6, rowing-water 7, swimming-laps-moderate 8.3, swimming-laps-vigorous 9.8); gym and studio (stretching 2.3, yoga-hatha 2.5, pilates 3, tai-chi 3, weight-training-light 3.5, elliptical 5, weight-training-vigorous 6, rowing-machine 7, circuit-training 7.2, calisthenics-vigorous 8, stair-stepper 9, jump-rope 11); sports (volleyball 4, table-tennis 4, dancing-social 4.5, golf-walking 4.8, badminton 5.5, boxing-bag 5.5, tennis-doubles 6, soccer-casual 7, dancing-aerobic 7.3, basketball-game 8, tennis-singles 8, rock-climbing 8, soccer-competitive 10, martial-arts 10.3). If nothing matches, pick the nearest entry and adjust it with met.";
const MET_DESC: &str = "Metabolic-equivalent override, between 0.5 and 30, or 0 (the default) to use the chosen activity's own value. One MET is resting metabolism, about 1 kcal per kg of body weight per hour. Required when activity is custom. Use it to dial intensity within an activity — walking-moderate is MET 3.5, but pushing a loaded pram uphill is closer to 5 — or to enter a value from a published compendium table that is not in the list. It also changes the reported MET-minutes and oxygen uptake, which are defined on this gross value.";
const BASIS_DESC: &str = "Whether to report gross or net energy. gross (default) is all energy the body used during the session, resting metabolism included, and is what almost every calculator and fitness tracker reports. net charges MET minus 1 instead, crediting back the roughly 1 MET you would have spent existing anyway, and is the honest figure for calories the session ADDED — about 30% lower for a moderate walk. Net is floored at zero, so a sub-resting activity such as sleeping reports 0 rather than a negative burn. MET-minutes and oxygen uptake always use the gross value.";
const OUTPUT_DESC: &str = "Output format. summary (default) is a readable report: the activity and its MET with the source of that value, the weight and duration echoed in both units, then the session total, the per-minute and per-hour rate, the MET-minutes of activity volume with how many such sessions reach the 500 MET-min/week public-health minimum, the implied oxygen uptake, a body-fat equivalent, and the same session priced across eight reference activities. table returns the same content as two GitHub-flavoured markdown tables you can paste into a training log. json returns a machine-readable object with activity, activity_label, met, met_source, met_charged, basis, weight_kg, weight_lb, duration_minutes, calories, kcal_per_minute, kcal_per_hour, met_minutes, sessions_to_500_met_minutes, vo2_ml_per_kg_per_min, oxygen_litres, fat_grams and a comparison array.";

/// Single source for the chat schema, the CLI, and (via
/// `scripts/sync-tool-manifest.py`) the page form's controls.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("weight")
                .required()
                .min(20.0)
                .max(660.0)
                .describe(WEIGHT_DESC),
        )
        .param(
            Param::enumv("weight_unit", ["kg", "lb"])
                .default("kg")
                .describe(WEIGHT_UNIT_DESC),
        )
        .param(
            Param::number("duration")
                .required()
                .min(0.5)
                .max(1440.0)
                .describe(DURATION_DESC),
        )
        .param(
            Param::enumv("duration_unit", ["minutes", "hours"])
                .default("minutes")
                .describe(DURATION_UNIT_DESC),
        )
        .param(
            Param::enumv("activity", ACTIVITY_VALUES)
                .default("walking-moderate")
                .describe(ACTIVITY_DESC),
        )
        .param(
            Param::number("met")
                .min(0.0)
                .max(30.0)
                .default(0.0)
                .describe(MET_DESC),
        )
        .param(
            Param::enumv("basis", ["gross", "net"])
                .default("gross")
                .describe(BASIS_DESC),
        )
        .param(
            Param::enumv("output", ["summary", "table", "json"])
                .default("summary")
                .describe(OUTPUT_DESC),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/calorie-burn",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Estimate calories burned for an activity, duration and body weight from standard MET values",
    skill(
        description = "Estimate the calories burned by one bout of activity from its metabolic equivalent (MET), the body weight and the elapsed time, using the standard kcal/min = MET x 3.5 x kg / 200. Pick from a 59-entry table of standard MET values spanning everyday and household tasks, walking and hiking, running, cycling, swimming and water sports, gym and studio work, and field sports, or pass activity=custom with your own met value; any named activity's MET can also be overridden with met to dial intensity. Weight is accepted in kilograms or pounds and duration in minutes or hours, both converted before the arithmetic and echoed back in both forms. Reports the session total, the per-minute and per-hour rate, the MET-minutes of activity volume together with how many such sessions reach the 500 MET-min/week public-health minimum, the implied oxygen uptake in ml/kg/min and total litres, a body-fat equivalent at 7700 kcal per kg, and the same session priced across eight reference activities for context. basis=net charges MET minus 1 to report only the energy the session added above resting metabolism, instead of the gross figure trackers show. Returns a readable summary, markdown tables, or JSON. Pure arithmetic computed locally; no data leaves the machine, and MET values are population averages, so the result is an estimate rather than a measurement.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "calorie-burn", |a: Args| {
            gizza_ai_calorie_burn_core::run(
                a.weight,
                &a.weight_unit,
                a.duration,
                &a.duration_unit,
                &a.activity,
                a.met,
                &a.basis,
                &a.output,
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
    fn dump_schema() {
        println!("SCHEMA_BEGIN{}SCHEMA_END", schema_json());
    }

    #[test]
    fn every_param_is_described() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().expect("object schema");
        assert_eq!(props.len(), 8, "parameter count changed");
        for (name, spec) in props {
            let desc = spec["description"].as_str().unwrap_or("");
            assert!(desc.len() > 20, "param {name} needs a real description");
        }
        assert_eq!(
            schema["required"].as_array().unwrap(),
            &vec![serde_json::json!("weight"), serde_json::json!("duration")]
        );
    }

    /// The descriptor's declared defaults are what chat/the CLI leave out, so
    /// serde's `#[serde(default = ...)]` fallbacks must agree with them.
    #[test]
    fn args_defaults_match_the_descriptor() {
        let a: Args = serde_json::from_str(r#"{"weight":70,"duration":30}"#).unwrap();
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = &schema["properties"];
        assert_eq!(a.weight_unit, props["weight_unit"]["default"]);
        assert_eq!(a.duration_unit, props["duration_unit"]["default"]);
        assert_eq!(a.activity, props["activity"]["default"]);
        assert_eq!(a.met, props["met"]["default"].as_f64().unwrap());
        assert_eq!(a.basis, props["basis"]["default"]);
        assert_eq!(a.output, props["output"]["default"]);
    }

    /// The defaulted Args reach the core and produce the documented headline, so
    /// a default drifting out of the core's accepted set is caught.
    #[test]
    fn defaulted_args_run_through_the_core() {
        let a: Args = serde_json::from_str(r#"{"weight":70,"duration":30}"#).unwrap();
        let out = gizza_ai_calorie_burn_core::run(
            a.weight,
            &a.weight_unit,
            a.duration,
            &a.duration_unit,
            &a.activity,
            a.met,
            &a.basis,
            &a.output,
        )
        .unwrap();
        assert!(out.contains("Walking, moderate (3 mph / 4.8 km/h)"), "{out}");
        assert!(out.contains("Energy burned        129 kcal"), "{out}");
    }

    /// The authored dropdown list must be exactly the set the core knows about —
    /// an extra value would be a dead option and a missing one an unreachable
    /// MET entry.
    #[test]
    fn authored_activity_values_match_the_core_table() {
        assert_eq!(
            ACTIVITY_VALUES.to_vec(),
            gizza_ai_calorie_burn_core::activity_keys()
        );
        assert_eq!(*ACTIVITY_VALUES.last().unwrap(), "custom");
    }

    /// Every enum variant the descriptor advertises must be one the core
    /// accepts — an advertised-but-rejected option is a broken dropdown.
    #[test]
    fn every_advertised_enum_variant_is_accepted_by_the_core() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let variants = |p: &str| -> Vec<String> {
            schema["properties"][p]["enum"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect()
        };
        for v in variants("weight_unit") {
            let weight = if v == "lb" { 154.0 } else { 70.0 };
            gizza_ai_calorie_burn_core::run(weight, &v, 30.0, "minutes", "jogging", 0.0, "gross", "summary")
                .unwrap_or_else(|e| panic!("weight_unit={v}: {e}"));
        }
        for v in variants("duration_unit") {
            gizza_ai_calorie_burn_core::run(70.0, "kg", 1.0, &v, "jogging", 0.0, "gross", "summary")
                .unwrap_or_else(|e| panic!("duration_unit={v}: {e}"));
        }
        for v in variants("activity") {
            let met = if v == "custom" { 6.0 } else { 0.0 };
            gizza_ai_calorie_burn_core::run(70.0, "kg", 30.0, "minutes", &v, met, "gross", "summary")
                .unwrap_or_else(|e| panic!("activity={v}: {e}"));
        }
        for v in variants("basis") {
            gizza_ai_calorie_burn_core::run(70.0, "kg", 30.0, "minutes", "jogging", 0.0, &v, "summary")
                .unwrap_or_else(|e| panic!("basis={v}: {e}"));
        }
        for v in variants("output") {
            gizza_ai_calorie_burn_core::run(70.0, "kg", 30.0, "minutes", "jogging", 0.0, "gross", &v)
                .unwrap_or_else(|e| panic!("output={v}: {e}"));
        }
    }

    /// The numeric bounds the schema advertises must be the bounds the core
    /// actually accepts, at the exact boundary. The weight maximum is only
    /// reachable in pounds (660 lb = 299.4 kg, inside the 300 kg cap) and the
    /// duration maximum only in minutes, so each boundary is tested in the unit
    /// that can express it.
    #[test]
    fn advertised_numeric_bounds_are_accepted_at_the_boundary() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let p = &schema["properties"];
        let bound = |name: &str, key: &str| p[name][key].as_f64().unwrap();
        gizza_ai_calorie_burn_core::run(
            bound("weight", "minimum"),
            "kg",
            bound("duration", "minimum"),
            "minutes",
            "walking-moderate",
            bound("met", "minimum"),
            "gross",
            "summary",
        )
        .unwrap();
        gizza_ai_calorie_burn_core::run(
            bound("weight", "maximum"),
            "lb",
            bound("duration", "maximum"),
            "minutes",
            "custom",
            bound("met", "maximum"),
            "net",
            "json",
        )
        .unwrap();
    }

    /// Drift guard: the chat/CLI/page schema is generated from `descriptor()`, so any
    /// change to a param name, type, enum, bound, or default must be mirrored here.
    #[test]
    fn schema_matches_the_authored_contract() {
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let authored: serde_json::Value = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["weight", "duration"],
            "properties": {
                "weight": {
                    "type": "number",
                    "minimum": 20,
                    "maximum": 660,
                    "description": WEIGHT_DESC
                },
                "weight_unit": {
                    "type": "string",
                    "enum": ["kg", "lb"],
                    "default": "kg",
                    "description": WEIGHT_UNIT_DESC
                },
                "duration": {
                    "type": "number",
                    "minimum": 0.5,
                    "maximum": 1440,
                    "description": DURATION_DESC
                },
                "duration_unit": {
                    "type": "string",
                    "enum": ["minutes", "hours"],
                    "default": "minutes",
                    "description": DURATION_UNIT_DESC
                },
                "activity": {
                    "type": "string",
                    "enum": ACTIVITY_VALUES.to_vec(),
                    "default": "walking-moderate",
                    "description": ACTIVITY_DESC
                },
                "met": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 30,
                    "default": 0.0,
                    "description": MET_DESC
                },
                "basis": {
                    "type": "string",
                    "enum": ["gross", "net"],
                    "default": "gross",
                    "description": BASIS_DESC
                },
                "output": {
                    "type": "string",
                    "enum": ["summary", "table", "json"],
                    "default": "summary",
                    "description": OUTPUT_DESC
                }
            }
        });
        assert_eq!(actual, authored);
    }
}
