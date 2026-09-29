//! gizza-ai/heart-rate-zones — heart-rate training zones as a chat skill block on
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
    age: f64,
    #[serde(default = "default_resting_hr")]
    resting_hr: f64,
    #[serde(default)]
    max_hr: f64,
    #[serde(default = "default_formula")]
    max_hr_formula: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_model")]
    model: String,
    #[serde(default)]
    intensity: f64,
    #[serde(default = "default_output")]
    output: String,
}

fn default_resting_hr() -> f64 {
    60.0
}
fn default_formula() -> String {
    "tanaka".into()
}
fn default_method() -> String {
    "karvonen".into()
}
fn default_model() -> String {
    "five-zone".into()
}
fn default_output() -> String {
    "summary".into()
}

const AGE_DESC: &str = "Your age in whole years, between 5 and 120. Used only to estimate maximum heart rate with the chosen max_hr_formula, so it is ignored when you supply a measured max_hr. For example 30 gives a Tanaka maximum of 187 bpm.";
const RESTING_DESC: &str = "Resting heart rate in beats per minute, between 25 and 120. Default 60. Measure it lying down right after waking, before getting up, counting for a full minute or reading it off a monitor, and average three to five mornings. Typical values are 40-50 for elite endurance athletes, 50-60 for well-trained, 60-80 for most adults. It shifts every zone under the karvonen method (it is the floor the heart-rate reserve is added to) and is ignored by percent-max.";
const MAX_HR_DESC: &str = "Measured maximum heart rate in beats per minute, between 100 and 250. Default 0, which means estimate it from age with max_hr_formula. Supply a real number from a maximal test, a race finish, or the highest value your monitor has ever recorded — any age formula carries roughly plus or minus 7-12 bpm of individual spread, so a measured maximum makes every zone meaningfully more accurate. Must be above resting_hr.";
const FORMULA_DESC: &str = "Which age-based equation estimates maximum heart rate when max_hr is 0. tanaka (default) is 208 - 0.7 x age, the modern meta-analysis fit and the better choice for most adults. fox is the classic 220 - age, which tends to overestimate for older people and underestimate for the young but is what most charts and gym posters use. gulati is 206 - 0.88 x age, derived from a large treadmill study of women. nes is 211 - 0.64 x age, from a large Norwegian cohort. inbar is 205.8 - 0.685 x age. oakland is the nonlinear 192 - 0.007 x age^2, whose gap to the linear fits widens with age. Ignored when max_hr is supplied.";
const METHOD_DESC: &str = "How each zone's heart-rate band is derived. karvonen (default) takes percentages of heart-rate reserve and adds resting heart rate back, so target = (max - resting) x percent + resting; it personalises the zones to your fitness and is the standard recommendation. percent-max takes plain percentages of maximum heart rate and ignores resting entirely, matching most published zone charts and watch defaults; its zones sit lower than Karvonen's. zoladz ignores percentages altogether and places five bands at fixed distances below maximum (max-50, max-40, max-30, max-20, max-10, each plus or minus 5 bpm), which is why it always returns five zones whatever model says.";
const MODEL_DESC: &str = "Which set of zones to report. five-zone (default) is the usual 50-60 / 60-70 / 70-80 / 80-90 / 90-100 percent split labelled recovery, aerobic base, tempo, threshold and VO2 max. three-zone is the polarized model used by many endurance coaches: easy below the first lactate turn point (50-81), moderate between the turn points (81-87), hard above the second (87-100). aha reports the two American Heart Association activity bands, moderate 50-70 and vigorous 70-85, which is what public-health guidance is written against. Ignored when method is zoladz.";
const INTENSITY_DESC: &str = "Optional single intensity to convert into one target heart rate, in percent, between 30 and 100. Default 0, which omits the extra line. Use it for a prescription written as a single number, for example intensity 70 under karvonen at age 30 with resting 60 gives 149 bpm. It is computed with the same method as the zones, except under zoladz where it falls back to the Karvonen reserve calculation because Zoladz defines no percentages.";
const OUTPUT_DESC: &str = "Output format. summary (default) is a readable report: the age and resting rate echoed back, the maximum heart rate with the formula or measurement it came from, the heart-rate reserve, then one aligned line per zone with its number, name, intensity band, bpm range and training focus. table returns the same content as two GitHub-flavoured markdown tables you can paste into a training plan or issue. json returns a machine-readable object with max_hr, max_hr_source, heart_rate_reserve, method, model, a zones array of zone/name/intensity/low_bpm/high_bpm/focus objects, and a target object when intensity is set.";

/// Single source for the chat schema, the CLI, and (via
/// `scripts/sync-tool-manifest.py`) the page form's controls.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("age")
                .required()
                .min(5.0)
                .max(120.0)
                .describe(AGE_DESC),
        )
        .param(
            Param::number("resting_hr")
                .min(25.0)
                .max(120.0)
                .default(60.0)
                .describe(RESTING_DESC),
        )
        .param(
            Param::number("max_hr")
                .min(0.0)
                .max(250.0)
                .default(0.0)
                .describe(MAX_HR_DESC),
        )
        .param(
            Param::enumv("max_hr_formula", ["tanaka", "fox", "gulati", "nes", "inbar", "oakland"])
                .default("tanaka")
                .describe(FORMULA_DESC),
        )
        .param(
            Param::enumv("method", ["karvonen", "percent-max", "zoladz"])
                .default("karvonen")
                .describe(METHOD_DESC),
        )
        .param(
            Param::enumv("model", ["five-zone", "three-zone", "aha"])
                .default("five-zone")
                .describe(MODEL_DESC),
        )
        .param(
            Param::number("intensity")
                .min(0.0)
                .max(100.0)
                .default(0.0)
                .describe(INTENSITY_DESC),
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
    name = "gizza-ai/heart-rate-zones",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute heart-rate training zones from age and resting HR with the Karvonen, percent-of-max or Zoladz method",
    skill(
        description = "Compute target heart-rate training zones from an age and a resting heart rate. Estimates maximum heart rate with Tanaka (208 - 0.7 x age), Fox (220 - age), Gulati (206 - 0.88 x age), Nes (211 - 0.64 x age), Inbar (205.8 - 0.685 x age) or the nonlinear Oakland fit (192 - 0.007 x age^2), or uses a measured maximum when you have one, then derives the zone bands by the Karvonen heart-rate-reserve method ((max - resting) x percent + resting), by plain percentage of maximum heart rate, or by Zoladz fixed bpm offsets below maximum. Reports the maximum heart rate and where it came from, the heart-rate reserve, and one row per zone with its intensity band, beats-per-minute range and training focus, in the five-zone model, the three-zone polarized model, or the two American Heart Association activity bands. An optional single intensity percentage is converted into one target heart rate. Returns a readable summary, markdown tables, or JSON. Pure arithmetic computed locally; no data leaves the machine, and the result is a training guide, not medical advice.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "heart-rate-zones", |a: Args| {
            gizza_ai_heart_rate_zones_core::run(
                a.age,
                a.resting_hr,
                a.max_hr,
                &a.max_hr_formula,
                &a.method,
                &a.model,
                a.intensity,
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
            &vec![serde_json::json!("age")]
        );
    }

    /// The descriptor's declared defaults are what chat/the CLI leave out, so
    /// serde's `#[serde(default = ...)]` fallbacks must agree with them.
    #[test]
    fn args_defaults_match_the_descriptor() {
        let a: Args = serde_json::from_str(r#"{"age":30}"#).unwrap();
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = &schema["properties"];
        assert_eq!(a.resting_hr, props["resting_hr"]["default"].as_f64().unwrap());
        assert_eq!(a.max_hr, props["max_hr"]["default"].as_f64().unwrap());
        assert_eq!(a.max_hr_formula, props["max_hr_formula"]["default"]);
        assert_eq!(a.method, props["method"]["default"]);
        assert_eq!(a.model, props["model"]["default"]);
        assert_eq!(a.intensity, props["intensity"]["default"].as_f64().unwrap());
        assert_eq!(a.output, props["output"]["default"]);
    }

    /// The defaulted Args reach the core and produce the documented headline, so
    /// a default drifting out of the core's accepted set is caught.
    #[test]
    fn defaulted_args_run_through_the_core() {
        let a: Args = serde_json::from_str(r#"{"age":30}"#).unwrap();
        let out = gizza_ai_heart_rate_zones_core::run(
            a.age,
            a.resting_hr,
            a.max_hr,
            &a.max_hr_formula,
            &a.method,
            &a.model,
            a.intensity,
            &a.output,
        )
        .unwrap();
        assert!(out.contains("Karvonen heart-rate reserve"), "{out}");
        assert!(out.contains("Maximum HR 187 bpm"), "{out}");
        assert!(out.contains("Heart-rate reserve 127 bpm"), "{out}");
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
        for v in variants("max_hr_formula") {
            gizza_ai_heart_rate_zones_core::run(
                35.0, 55.0, 0.0, &v, "karvonen", "five-zone", 70.0, "summary",
            )
            .unwrap_or_else(|e| panic!("max_hr_formula={v}: {e}"));
        }
        for v in variants("method") {
            gizza_ai_heart_rate_zones_core::run(
                35.0, 55.0, 0.0, "tanaka", &v, "five-zone", 70.0, "summary",
            )
            .unwrap_or_else(|e| panic!("method={v}: {e}"));
        }
        for v in variants("model") {
            gizza_ai_heart_rate_zones_core::run(
                35.0, 55.0, 0.0, "tanaka", "karvonen", &v, 70.0, "summary",
            )
            .unwrap_or_else(|e| panic!("model={v}: {e}"));
        }
        for v in variants("output") {
            gizza_ai_heart_rate_zones_core::run(
                35.0, 55.0, 0.0, "tanaka", "karvonen", "five-zone", 70.0, &v,
            )
            .unwrap_or_else(|e| panic!("output={v}: {e}"));
        }
    }

    /// The numeric bounds the schema advertises must be the bounds the core
    /// actually accepts, at the exact boundary.
    #[test]
    fn advertised_numeric_bounds_are_accepted_at_the_boundary() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let p = &schema["properties"];
        let bound = |name: &str, key: &str| p[name][key].as_f64().unwrap();
        gizza_ai_heart_rate_zones_core::run(
            bound("age", "minimum"),
            bound("resting_hr", "minimum"),
            bound("max_hr", "maximum"),
            "tanaka",
            "karvonen",
            "five-zone",
            bound("intensity", "maximum"),
            "summary",
        )
        .unwrap();
        gizza_ai_heart_rate_zones_core::run(
            bound("age", "maximum"),
            bound("resting_hr", "maximum"),
            bound("max_hr", "maximum"),
            "fox",
            "percent-max",
            "aha",
            bound("intensity", "minimum"),
            "summary",
        )
        .unwrap();
        // The auto-estimate path at the oldest advertised age: Fox gives a
        // maximum of 100 bpm, which only clears a low resting rate.
        gizza_ai_heart_rate_zones_core::run(
            bound("age", "maximum"),
            bound("resting_hr", "minimum"),
            0.0,
            "fox",
            "karvonen",
            "five-zone",
            0.0,
            "summary",
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
            "required": ["age"],
            "properties": {
                "age": {
                    "type": "number",
                    "minimum": 5,
                    "maximum": 120,
                    "description": AGE_DESC
                },
                "resting_hr": {
                    "type": "number",
                    "minimum": 25,
                    "maximum": 120,
                    "default": 60.0,
                    "description": RESTING_DESC
                },
                "max_hr": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 250,
                    "default": 0.0,
                    "description": MAX_HR_DESC
                },
                "max_hr_formula": {
                    "type": "string",
                    "enum": ["tanaka", "fox", "gulati", "nes", "inbar", "oakland"],
                    "default": "tanaka",
                    "description": FORMULA_DESC
                },
                "method": {
                    "type": "string",
                    "enum": ["karvonen", "percent-max", "zoladz"],
                    "default": "karvonen",
                    "description": METHOD_DESC
                },
                "model": {
                    "type": "string",
                    "enum": ["five-zone", "three-zone", "aha"],
                    "default": "five-zone",
                    "description": MODEL_DESC
                },
                "intensity": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 100,
                    "default": 0.0,
                    "description": INTENSITY_DESC
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
