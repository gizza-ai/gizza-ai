//! gizza-ai/table-to-image — render CSV/JSON tables as standalone SVG.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill and the pure core crate.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_table_to_image_core::{render, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_input_format")]
    input_format: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_true")]
    zebra: bool,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_accent")]
    accent: String,
    #[serde(default = "default_font_size")]
    font_size: u32,
    #[serde(default = "default_cell_padding")]
    cell_padding: u32,
    #[serde(default)]
    title: String,
    #[serde(default = "default_align")]
    align: String,
}

fn default_input_format() -> String {
    "auto".into()
}
fn default_delimiter() -> String {
    ",".into()
}
fn default_true() -> bool {
    true
}
fn default_theme() -> String {
    "light".into()
}
fn default_accent() -> String {
    "#2563eb".into()
}
fn default_font_size() -> u32 {
    14
}
fn default_cell_padding() -> u32 {
    10
}
fn default_align() -> String {
    "left".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("CSV text or JSON table data to render. CSV may include quoted fields; JSON may be an array of flat objects, an array of arrays, or a single object."),
        )
        .param(
            Param::enumv("input_format", ["auto", "csv", "json"])
                .default("auto")
                .describe("How to parse the table: auto sniffs JSON when the input starts with '[' or '{', otherwise CSV; csv and json force that parser. Default auto."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("CSV delimiter to use when parsing CSV input. Use ',', ';', '|', or '\\t' / 'tab'. Ignored for JSON. Default comma."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Treat the first CSV row or first JSON array row as a styled header. JSON object keys always become the header. Default true."),
        )
        .param(
            Param::boolean("zebra")
                .default(true)
                .describe("Shade alternating body rows for readability. Default true."),
        )
        .param(
            Param::enumv("theme", ["light", "dark", "slate", "blue", "green", "minimal"])
                .default("light")
                .describe("Visual theme for the SVG: light, dark, slate, blue, green, or minimal (transparent/border-light). Default light."),
        )
        .param(
            Param::string("accent")
                .default("#2563eb")
                .describe("CSS colour for the header band or minimal-theme underline, e.g. #2563eb, #16a34a, or tomato. Default #2563eb."),
        )
        .param(
            Param::integer("font_size")
                .min(8.0)
                .max(48.0)
                .default(14)
                .describe("Body font size in pixels (8–48). The title is drawn slightly larger. Default 14."),
        )
        .param(
            Param::integer("cell_padding")
                .min(0.0)
                .max(60.0)
                .default(10)
                .describe("Horizontal and vertical padding inside each table cell in pixels (0–60). Default 10."),
        )
        .param(
            Param::string("title")
                .default("")
                .describe("Optional title/caption drawn centered above the table."),
        )
        .param(
            Param::enumv("align", ["left", "center", "right"])
                .default("left")
                .describe("Text alignment for table cells: left, center, or right. Default left."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options(a: &Args) -> Options {
    Options {
        input_format: a.input_format.clone(),
        delimiter: a.delimiter.clone(),
        header: a.header,
        zebra: a.zebra,
        theme: a.theme.clone(),
        accent: a.accent.clone(),
        font_size: a.font_size,
        cell_padding: a.cell_padding,
        title: a.title.clone(),
        align: a.align.clone(),
    }
}

#[cfg(target_arch = "wasm32")]
struct TableToImage;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/table-to-image",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render CSV or JSON tables as a styled standalone SVG",
    skill(
        description = "Render CSV or JSON table data as a standalone SVG suitable for sharing in docs, chats, and issue comments. Supports CSV (custom delimiter), JSON arrays of objects or arrays, optional header styling, zebra rows, themes, accent colour, title, font size, cell padding, and left/center/right alignment. Output is deterministic SVG text — no uploads and no raster encoder required.",
        parameters = schema_json()
    ),
)]
impl TableToImage {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "table-to-image", |a: Args| {
            render(&a.input, &options(&a)).map_err(SkillError::InvalidArgs)
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
            r##"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "CSV text or JSON table data to render. CSV may include quoted fields; JSON may be an array of flat objects, an array of arrays, or a single object." },
                    "input_format": { "type": "string", "enum": ["auto", "csv", "json"], "default": "auto", "description": "How to parse the table: auto sniffs JSON when the input starts with '[' or '{', otherwise CSV; csv and json force that parser. Default auto." },
                    "delimiter": { "type": "string", "default": ",", "description": "CSV delimiter to use when parsing CSV input. Use ',', ';', '|', or '\\t' / 'tab'. Ignored for JSON. Default comma." },
                    "header": { "type": "boolean", "default": true, "description": "Treat the first CSV row or first JSON array row as a styled header. JSON object keys always become the header. Default true." },
                    "zebra": { "type": "boolean", "default": true, "description": "Shade alternating body rows for readability. Default true." },
                    "theme": { "type": "string", "enum": ["light", "dark", "slate", "blue", "green", "minimal"], "default": "light", "description": "Visual theme for the SVG: light, dark, slate, blue, green, or minimal (transparent/border-light). Default light." },
                    "accent": { "type": "string", "default": "#2563eb", "description": "CSS colour for the header band or minimal-theme underline, e.g. #2563eb, #16a34a, or tomato. Default #2563eb." },
                    "font_size": { "type": "integer", "minimum": 8, "maximum": 48, "default": 14, "description": "Body font size in pixels (8–48). The title is drawn slightly larger. Default 14." },
                    "cell_padding": { "type": "integer", "minimum": 0, "maximum": 60, "default": 10, "description": "Horizontal and vertical padding inside each table cell in pixels (0–60). Default 10." },
                    "title": { "type": "string", "default": "", "description": "Optional title/caption drawn centered above the table." },
                    "align": { "type": "string", "enum": ["left", "center", "right"], "default": "left", "description": "Text alignment for table cells: left, center, or right. Default left." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
