//! gizza-ai/shadcn-theme-generator — chat skill block on the shared tool abstraction.
//!
//! Turns one or two seed colors into a complete shadcn/ui CSS-variable theme
//! (light `:root` + dark `.dark`, plus the Tailwind v4 `@theme inline` map),
//! with every foreground picked by measured WCAG contrast. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    primary: String,
    #[serde(default)]
    accent: String,
    #[serde(default = "default_neutral")]
    neutral: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_tailwind")]
    tailwind: String,
    #[serde(default = "default_radius")]
    radius: f64,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_true")]
    charts: bool,
    #[serde(default = "default_true")]
    sidebar: bool,
}

fn default_neutral() -> String {
    "zinc".to_string()
}
fn default_format() -> String {
    "oklch".to_string()
}
fn default_tailwind() -> String {
    "v4".to_string()
}
fn default_mode() -> String {
    "both".to_string()
}
fn default_radius() -> f64 {
    0.625
}
fn default_true() -> bool {
    true
}

const NEUTRALS: [&str; 5] = ["slate", "gray", "zinc", "neutral", "stone"];
const FORMATS: [&str; 3] = ["oklch", "hsl", "hex"];
const TAILWINDS: [&str; 2] = ["v4", "v3"];
const MODES: [&str; 3] = ["both", "light", "dark"];

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("primary")
                .required()
                .describe("Brand seed color for --primary, as #rrggbb, #rgb, bare hex, rgb(99, 102, 241) or hsl(239, 84%, 67%). Light mode keeps this color exactly; the dark block lifts its lightness so it still reads. Example: #6366f1."),
        )
        .param(
            Param::string("accent")
                .default("")
                .describe("Optional second seed for the --accent tokens, in the same notations as primary. Leave empty for the neutral accent upstream shadcn ships. Example: #10b981."),
        )
        .param(
            Param::enumv("neutral", NEUTRALS)
                .default("zinc")
                .describe("Grey family that tints backgrounds, borders and muted surfaces: slate (blue-leaning), gray, zinc (default), neutral (pure grey) or stone (warm)."),
        )
        .param(
            Param::enumv("format", FORMATS)
                .default("oklch")
                .describe("Notation for every emitted value: oklch (current shadcn/Tailwind v4 default), hsl, or hex. All three describe the same 8-bit sRGB colors."),
        )
        .param(
            Param::enumv("tailwind", TAILWINDS)
                .default("v4")
                .describe("Output shape: v4 emits plain :root/.dark blocks plus an @theme inline map (default); v3 emits @layer base and writes hsl values as bare H S% L% triplets for hsl(var(--token))."),
        )
        .param(
            Param::number("radius")
                .default(0.625)
                .min(0.0)
                .max(2.0)
                .describe("The --radius token in rem, from 0 (square) to 2 (fully rounded). shadcn's own default is 0.625; 0.5 is the older default."),
        )
        .param(
            Param::enumv("mode", MODES)
                .default("both")
                .describe("Which blocks to emit: both light and dark (default), light only (:root), or dark only (.dark)."),
        )
        .param(
            Param::boolean("charts")
                .default(true)
                .describe("Include the --chart-1 … --chart-5 series colors, fanned out in hue from the primary seed. Default true."),
        )
        .param(
            Param::boolean("sidebar")
                .default(true)
                .describe("Include the --sidebar-* token group used by the shadcn sidebar block. Default true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/shadcn-theme-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn a seed color into a full shadcn/ui light and dark CSS variable theme.",
    skill(
        description = "Generate a complete shadcn/ui CSS-variable theme from a seed color, locally. Pass primary as a hex, rgb() or hsl() color; light mode keeps it exactly and dark mode lifts it. accent optionally tints the --accent tokens. neutral picks the grey family (slate, gray, zinc, neutral, stone). format writes oklch, hsl or hex values. tailwind=v4 adds the @theme inline map while tailwind=v3 emits @layer base with bare HSL triplets. radius sets --radius in rem (0-2). mode limits output to light or dark. charts and sidebar toggle the --chart-* and --sidebar-* groups. Returns paste-ready CSS plus a measured WCAG contrast table for every foreground/background pair.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "shadcn-theme-generator", |a: Args| {
            gizza_ai_shadcn_theme_generator_core::run(
                &a.primary,
                &a.accent,
                &a.neutral,
                &a.format,
                &a.tailwind,
                a.radius,
                &a.mode,
                a.charts,
                a.sidebar,
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
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(r#"{
            "type":"object",
            "properties":{
                "primary":{"type":"string","description":"Brand seed color for --primary, as #rrggbb, #rgb, bare hex, rgb(99, 102, 241) or hsl(239, 84%, 67%). Light mode keeps this color exactly; the dark block lifts its lightness so it still reads. Example: #6366f1."},
                "accent":{"type":"string","default":"","description":"Optional second seed for the --accent tokens, in the same notations as primary. Leave empty for the neutral accent upstream shadcn ships. Example: #10b981."},
                "neutral":{"type":"string","enum":["slate","gray","zinc","neutral","stone"],"default":"zinc","description":"Grey family that tints backgrounds, borders and muted surfaces: slate (blue-leaning), gray, zinc (default), neutral (pure grey) or stone (warm)."},
                "format":{"type":"string","enum":["oklch","hsl","hex"],"default":"oklch","description":"Notation for every emitted value: oklch (current shadcn/Tailwind v4 default), hsl, or hex. All three describe the same 8-bit sRGB colors."},
                "tailwind":{"type":"string","enum":["v4","v3"],"default":"v4","description":"Output shape: v4 emits plain :root/.dark blocks plus an @theme inline map (default); v3 emits @layer base and writes hsl values as bare H S% L% triplets for hsl(var(--token))."},
                "radius":{"type":"number","minimum":0,"maximum":2,"default":0.625,"description":"The --radius token in rem, from 0 (square) to 2 (fully rounded). shadcn's own default is 0.625; 0.5 is the older default."},
                "mode":{"type":"string","enum":["both","light","dark"],"default":"both","description":"Which blocks to emit: both light and dark (default), light only (:root), or dark only (.dark)."},
                "charts":{"type":"boolean","default":true,"description":"Include the --chart-1 … --chart-5 series colors, fanned out in hue from the primary seed. Default true."},
                "sidebar":{"type":"boolean","default":true,"description":"Include the --sidebar-* token group used by the shadcn sidebar block. Default true."}
            },
            "required":["primary"],
            "additionalProperties":false
        }"#).unwrap();
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(actual, authored);
    }
}
