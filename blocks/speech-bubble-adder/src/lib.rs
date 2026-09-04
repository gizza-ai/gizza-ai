//! gizza-ai/speech-bubble-adder — draw comic speech/thought bubbles on an image.
//!
//! Pure-Rust image transform, so it runs in the chat/CLI wafer runtime. It has
//! no standalone generated page because this repository's page renderer does not
//! have a pure-Rust image-bytes input/output mode; this mirrors add-text-to-image
//! and image-annotate.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{Envelope, ForUi, Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    text: String,
    #[serde(default)]
    bubbles: String,
    #[serde(default = "default_x")]
    x: f64,
    #[serde(default = "default_y")]
    y: f64,
    #[serde(default)]
    width: f64,
    #[serde(default)]
    height: f64,
    #[serde(default = "default_style")]
    style: String,
    #[serde(default = "default_tail")]
    tail: String,
    #[serde(default)]
    tail_x: Option<f64>,
    #[serde(default)]
    tail_y: Option<f64>,
    #[serde(default = "default_fill_color")]
    fill_color: String,
    #[serde(default = "default_text_color")]
    text_color: String,
    #[serde(default = "default_outline_color")]
    outline_color: String,
    #[serde(default = "default_outline_width")]
    outline_width: f64,
    #[serde(default)]
    font_size: f64,
    #[serde(default)]
    uppercase: bool,
    #[serde(default)]
    shadow: bool,
}

fn default_x() -> f64 { 20.0 }
fn default_y() -> f64 { 20.0 }
fn default_style() -> String { "speech".into() }
fn default_tail() -> String { "bottom-left".into() }
fn default_fill_color() -> String { "#ffffff".into() }
fn default_text_color() -> String { "#000000".into() }
fn default_outline_color() -> String { "#000000".into() }
fn default_outline_width() -> f64 { 3.0 }

const TEXT_DESC: &str = "Caption text for the first bubble. Use plain text or explicit \\n line breaks. The bubble can auto-size around this text when width, height, or font_size are 0.";
const BUBBLES_DESC: &str = "Optional JSON array of additional bubbles. Each object must have text and may override x, y, width, height, style, tail, tail_x, tail_y, fill_color, text_color, outline_color, outline_width, font_size, uppercase, and shadow; omitted fields inherit the top-level defaults.";
const X_DESC: &str = "Left pixel coordinate for the first bubble's bounding box, measured from the image's top-left corner. Default 20.";
const Y_DESC: &str = "Top pixel coordinate for the first bubble's bounding box, measured from the image's top-left corner. Default 20.";
const WIDTH_DESC: &str = "Bubble width in pixels. Set 0 (default) to auto-size from the text. Positive values force the bubble box and allow text auto-fit.";
const HEIGHT_DESC: &str = "Bubble height in pixels. Set 0 (default) to auto-size from the wrapped text. Positive values force the bubble box and allow text auto-fit.";
const STYLE_DESC: &str = "Bubble shape: speech (rounded balloon, default), oval, thought (cloud with puffs), shout (jagged starburst), whisper (dashed outline), or caption (narrator box without a tail).";
const TAIL_DESC: &str = "Tail direction preset: bottom-left, bottom-center, bottom-right, top-left, top-center, top-right, left, right, or none. Ignored by caption style; thought style draws trailing puffs instead of a triangle.";
const TAIL_X_DESC: &str = "Optional exact x coordinate for the tail tip / thought-puff aim point. Provide tail_y too; when both are set they override the tail direction preset.";
const TAIL_Y_DESC: &str = "Optional exact y coordinate for the tail tip / thought-puff aim point. Provide tail_x too; when both are set they override the tail direction preset.";
const FILL_DESC: &str = "Bubble fill color as #rgb, #rrggbb, or #rrggbbaa. Default #ffffff.";
const TEXT_COLOR_DESC: &str = "Caption text color as #rgb, #rrggbb, or #rrggbbaa. Default #000000.";
const OUTLINE_COLOR_DESC: &str = "Bubble outline color as #rgb, #rrggbb, or #rrggbbaa. Default #000000.";
const OUTLINE_WIDTH_DESC: &str = "Outline width in pixels, 0 to 64. Set 0 for no outline. Default 3.";
const FONT_SIZE_DESC: &str = "Caption font size in pixels, 0 to 512. Set 0 (default) to auto-fit the text inside the bubble.";
const UPPERCASE_DESC: &str = "Uppercase caption text before layout for a comic lettering feel. Default false.";
const SHADOW_DESC: &str = "Draw a soft dark drop shadow behind each bubble. Default false.";

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(Param::string("text").required().describe(TEXT_DESC))
        .param(Param::string("bubbles").default("").describe(BUBBLES_DESC))
        .param(Param::number("x").default(20.0).describe(X_DESC))
        .param(Param::number("y").default(20.0).describe(Y_DESC))
        .param(Param::number("width").min(0.0).default(0.0).describe(WIDTH_DESC))
        .param(Param::number("height").min(0.0).default(0.0).describe(HEIGHT_DESC))
        .param(Param::enumv("style", ["speech", "oval", "thought", "shout", "whisper", "caption"]).default("speech").describe(STYLE_DESC))
        .param(Param::enumv("tail", ["bottom-left", "bottom-center", "bottom-right", "top-left", "top-center", "top-right", "left", "right", "none"]).default("bottom-left").describe(TAIL_DESC))
        .param(Param::number("tail_x").describe(TAIL_X_DESC))
        .param(Param::number("tail_y").describe(TAIL_Y_DESC))
        .param(Param::string("fill_color").default("#ffffff").describe(FILL_DESC))
        .param(Param::string("text_color").default("#000000").describe(TEXT_COLOR_DESC))
        .param(Param::string("outline_color").default("#000000").describe(OUTLINE_COLOR_DESC))
        .param(Param::number("outline_width").min(0.0).max(64.0).default(3.0).describe(OUTLINE_WIDTH_DESC))
        .param(Param::number("font_size").min(0.0).max(512.0).default(0.0).describe(FONT_SIZE_DESC))
        .param(Param::boolean("uppercase").default(false).describe(UPPERCASE_DESC))
        .param(Param::boolean("shadow").default(false).describe(SHADOW_DESC))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct SpeechBubbleAdder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/speech-bubble-adder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add comic speech, thought, shout, whisper or caption bubbles to an image",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Add comic-style speech, thought, shout, whisper, oval, or caption bubbles to an input image and return a PNG. Provide the image by url or ref, set the caption text, position the bubble with x/y, optionally force width/height or leave them at 0 for auto-sizing, choose a style and tail direction or exact tail_x/tail_y aim point, customize fill/text/outline colors and outline width, uppercase the text, add a drop shadow, and pass a bubbles JSON array for multiple balloons. Runs locally after the image is resolved and emits an image/png media envelope.",
        parameters = schema_json()
    ),
)]
impl SpeechBubbleAdder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;

    let args: Args = serde_json::from_slice(&body).invalid_args("speech-bubble-adder")?;
    let (bytes, _mime, in_filename) = resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let opts = gizza_ai_speech_bubble_adder_core::Options {
        text: args.text,
        bubbles_json: args.bubbles,
        x: args.x as f32,
        y: args.y as f32,
        width: args.width as f32,
        height: args.height as f32,
        style: args.style,
        tail: args.tail,
        tail_x: args.tail_x.map(|v| v as f32),
        tail_y: args.tail_y.map(|v| v as f32),
        fill_color: args.fill_color,
        text_color: args.text_color,
        outline_color: args.outline_color,
        outline_width: args.outline_width as f32,
        font_size: args.font_size as f32,
        uppercase: args.uppercase,
        shadow: args.shadow,
    };
    let png = gizza_ai_speech_bubble_adder_core::render(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    let out_len = png.len();
    let encoded = B64.encode(&png);
    let data_url = format!("data:image/png;base64,{encoded}");
    let stem = in_filename.rsplit_once('.').map(|(s, _)| s).unwrap_or(&in_filename);
    let filename = format!("{stem}-speech-bubble.png");
    let env = Envelope {
        for_llm: format!("added speech bubble(s) to {in_filename} ({out_len}-byte PNG: {filename})"),
        for_ui: ForUi { data_url, mime: "image/png".to_string(), filename },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_documents_every_param() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("url"));
        assert!(props.contains_key("ref"));
        assert!(props.contains_key("style"));
        assert!(props.contains_key("tail"));
        for (name, spec) in props {
            assert!(spec["description"].as_str().unwrap_or_default().len() > 25, "{name} needs a useful description");
        }
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                "text": { "type": "string", "description": TEXT_DESC },
                "bubbles": { "type": "string", "default": "", "description": BUBBLES_DESC },
                "x": { "type": "number", "default": 20.0, "description": X_DESC },
                "y": { "type": "number", "default": 20.0, "description": Y_DESC },
                // `minimum`/`maximum` serialize as integers (0, not 0.0) while `default`
                // keeps its float form — mirror that exactly or the guard trips.
                "width": { "type": "number", "minimum": 0, "default": 0.0, "description": WIDTH_DESC },
                "height": { "type": "number", "minimum": 0, "default": 0.0, "description": HEIGHT_DESC },
                "style": { "type": "string", "enum": ["speech", "oval", "thought", "shout", "whisper", "caption"], "default": "speech", "description": STYLE_DESC },
                "tail": { "type": "string", "enum": ["bottom-left", "bottom-center", "bottom-right", "top-left", "top-center", "top-right", "left", "right", "none"], "default": "bottom-left", "description": TAIL_DESC },
                "tail_x": { "type": "number", "description": TAIL_X_DESC },
                "tail_y": { "type": "number", "description": TAIL_Y_DESC },
                "fill_color": { "type": "string", "default": "#ffffff", "description": FILL_DESC },
                "text_color": { "type": "string", "default": "#000000", "description": TEXT_COLOR_DESC },
                "outline_color": { "type": "string", "default": "#000000", "description": OUTLINE_COLOR_DESC },
                "outline_width": { "type": "number", "minimum": 0, "maximum": 64, "default": 3.0, "description": OUTLINE_WIDTH_DESC },
                "font_size": { "type": "number", "minimum": 0, "maximum": 512, "default": 0.0, "description": FONT_SIZE_DESC },
                "uppercase": { "type": "boolean", "default": false, "description": UPPERCASE_DESC },
                "shadow": { "type": "boolean", "default": false, "description": SHADOW_DESC }
            },
            "required": ["text"],
            "additionalProperties": false,
            "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
        });
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
