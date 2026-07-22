//! gizza-ai/before-after-slider — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_before_after_slider_core::{parse_orientation, parse_output, render, Options};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    before: String,
    after: String,
    #[serde(default = "default_before_label")]
    before_label: String,
    #[serde(default = "default_after_label")]
    after_label: String,
    #[serde(default = "default_orientation")]
    orientation: String,
    #[serde(default = "default_start")]
    start_position: f64,
    #[serde(default)]
    width: u64,
    #[serde(default)]
    move_on_hover: bool,
    #[serde(default = "default_handle_color")]
    handle_color: String,
    #[serde(default = "default_output")]
    output: String,
}
fn default_before_label() -> String {
    "Before".to_string()
}
fn default_after_label() -> String {
    "After".to_string()
}
fn default_orientation() -> String {
    "horizontal".to_string()
}
fn default_start() -> f64 {
    50.0
}
fn default_handle_color() -> String {
    "#ffffff".to_string()
}
fn default_output() -> String {
    "document".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("before")
                .required()
                .describe("The 'before' image source shown on the left (horizontal) or top (vertical). Pass an http(s):// URL, a relative path, or a data:image/...;base64,... URI. Embedded verbatim into the generated HTML. Example: https://picsum.photos/id/1015/900/600"),
        )
        .param(
            Param::string("after")
                .required()
                .describe("The 'after' image source shown on the right (horizontal) or bottom (vertical). Same accepted forms as 'before'. Works best when both images share the same dimensions. Example: https://picsum.photos/id/1016/900/600"),
        )
        .param(
            Param::string("before_label")
                .default("Before")
                .describe("Caption badge over the 'before' side. Set to an empty string to hide it. Default 'Before'."),
        )
        .param(
            Param::string("after_label")
                .default("After")
                .describe("Caption badge over the 'after' side. Set to an empty string to hide it. Default 'After'."),
        )
        .param(
            Param::enumv("orientation", ["horizontal", "vertical"])
                .default("horizontal")
                .describe("Wipe direction. 'horizontal' (default) = a vertical divider that drags left↔right. 'vertical' = a horizontal divider that drags up↕down."),
        )
        .param(
            Param::number("start_position")
                .default(50.0)
                .min(0.0)
                .max(100.0)
                .describe("Initial divider position as a percent (0–100). 50 puts it in the middle. Clamped into range. Default 50."),
        )
        .param(
            Param::integer("width")
                .default(0)
                .min(0.0)
                .describe("Maximum widget width in CSS pixels. 0 (default) = fluid: the slider fills its container and stays responsive. Set e.g. 720 to cap the width."),
        )
        .param(
            Param::boolean("move_on_hover")
                .default(false)
                .describe("If true, the divider follows the mouse on hover (no click needed). If false (default), the user drags or presses to move it. Touch always drags."),
        )
        .param(
            Param::string("handle_color")
                .default("#ffffff")
                .describe("Color of the divider line and round handle. Any CSS color: #rgb, #rrggbb, or a named color like 'white'. Default #ffffff."),
        )
        .param(
            Param::enumv("output", ["document", "embed"])
                .default("document")
                .describe("Output shape. 'document' (default) = a complete, save-as-.html standalone page. 'embed' = just the <style>+<div>+<script> snippet to paste into an existing web page."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run(a: Args) -> Result<String, String> {
    let opts = Options {
        before: a.before,
        after: a.after,
        before_label: a.before_label,
        after_label: a.after_label,
        orientation: parse_orientation(&a.orientation)?,
        start: a.start_position,
        width: a.width.min(u32::MAX as u64) as u32,
        move_on_hover: a.move_on_hover,
        handle_color: a.handle_color,
        output: parse_output(&a.output)?,
    };
    render(&opts)
}

#[cfg(target_arch = "wasm32")]
struct BeforeAfterSlider;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/before-after-slider",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a self-contained interactive before/after image comparison slider (HTML).",
    skill(
        description = "Build a self-contained interactive before/after image comparison slider from two image sources. Output is a single HTML blob (inline CSS + JS, no external libraries) that overlays the two images and lets the viewer drag a divider to wipe between them — supports pointer + touch drag, keyboard arrows, a start position, optional per-side labels, horizontal or vertical wipe, custom handle color, and multiple sliders on one page. Images are embedded by URL or data: URI. Choose 'document' output for a save-as-.html page or 'embed' for a paste-anywhere snippet.",
        parameters = schema_json()
    ),
)]
impl BeforeAfterSlider {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "before-after-slider", |a: Args| {
            run(a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}
