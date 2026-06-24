//! gizza-ai/svg-sprite-builder — combine several `<svg>` icons into one SVG
//! `<symbol>` sprite sheet. The chat schema is single-sourced from descriptor()
//! (which also drives the CLI); handle() delegates to block_utils::run_skill.
//! Pure compute → runs on all backends (chat, CLI, page).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_svg_sprite_builder_core::run as build_sprite;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    svgs: String,
    #[serde(default = "default_prefix")]
    prefix: String,
    #[serde(default = "default_id_source")]
    id_source: String,
    #[serde(default = "default_hidden")]
    hidden: bool,
}

fn default_prefix() -> String {
    "icon".to_string()
}
fn default_id_source() -> String {
    "auto".to_string()
}
fn default_hidden() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("svgs")
                .required()
                .describe("One or more complete <svg>…</svg> documents, pasted/concatenated one after another. Each top-level <svg> becomes one <symbol> in the sprite."),
        )
        .param(
            Param::string("prefix")
                .default("icon")
                .describe("Prefix for auto-generated symbol ids (icon-1, icon-2, …); also disambiguates duplicate ids. Default 'icon'."),
        )
        .param(
            Param::enumv("id_source", ["auto", "id", "title"])
                .default("auto")
                .describe("How each symbol id is chosen: 'auto' (prefix + index), 'id' (the source <svg>'s id attribute), or 'title' (its first <title> text). 'id'/'title' fall back to auto when missing."),
        )
        .param(
            Param::boolean("hidden")
                .default(true)
                .describe("Wrap the sprite in a visually-hidden, aria-hidden <svg> (recommended: drop it once at the top of the page without rendering anything). false emits a bare <svg> wrapper."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SvgSpriteBuilder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/svg-sprite-builder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Combine multiple SVGs into one <symbol> sprite sheet",
    skill(
        description = "Combine several standalone <svg> icons into a single SVG <symbol> sprite sheet. Paste one or more complete <svg>…</svg> documents in `svgs` (concatenated); each top-level <svg> becomes a <symbol id=… viewBox=…> you reference with <use href=\"#id\">. `prefix` seeds auto ids (icon-1, icon-2, …); `id_source` is 'auto', 'id' (the svg's id attribute), or 'title' (its <title> text). `hidden` (default true) wraps the sprite in a visually-hidden, aria-hidden <svg>. A viewBox is preserved, or derived from width/height. Output is the sprite SVG plus a comment listing the <use> snippets. Runs locally.",
        parameters = schema_json()
    ),
)]
impl SvgSpriteBuilder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "svg-sprite-builder", |a: Args| {
            build_sprite(&a.svgs, &a.prefix, &a.id_source, a.hidden).map_err(SkillError::InvalidArgs)
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
                    "svgs":      { "type": "string", "description": "One or more complete <svg>…</svg> documents, pasted/concatenated one after another. Each top-level <svg> becomes one <symbol> in the sprite." },
                    "prefix":    { "type": "string", "default": "icon", "description": "Prefix for auto-generated symbol ids (icon-1, icon-2, …); also disambiguates duplicate ids. Default 'icon'." },
                    "id_source": { "type": "string", "enum": ["auto", "id", "title"], "default": "auto", "description": "How each symbol id is chosen: 'auto' (prefix + index), 'id' (the source <svg>'s id attribute), or 'title' (its first <title> text). 'id'/'title' fall back to auto when missing." },
                    "hidden":    { "type": "boolean", "default": true, "description": "Wrap the sprite in a visually-hidden, aria-hidden <svg> (recommended: drop it once at the top of the page without rendering anything). false emits a bare <svg> wrapper." }
                },
                "required": ["svgs"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
