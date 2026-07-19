//! gizza-ai/image-film-grain — overlay analog film grain on an image via
//! ffmpeg's `noise` filter, on the shared tool abstraction.
//!
//! Input::Image emits a url⊕ref oneOf; run() uses resolve_source → core::plan →
//! dispatch_ffmpeg → build_media_envelope. Users pass a friendly `amount` 0–100
//! (mapped onto the noise filter's strength) and a `monochrome` toggle (luma-only
//! neutral grain, the classic film look, vs colored grain on all channels);
//! `format` can convert the output. The chat schema is derived from
//! `descriptor()` (single source — shared across chat + CLI + page) and the
//! drift-guard test below proves it matches the authored schema.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, AssetKind, Input, Param, SkillError, SourceFields,
    ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_image_film_grain_core::{
    parse_format, OutFormat, DEFAULT_AMOUNT, DEFAULT_FORMAT, DEFAULT_MONOCHROME, FORMATS,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    amount: Option<f64>,
    #[serde(default)]
    monochrome: Option<bool>,
    #[serde(default)]
    format: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    // Input::Image → url⊕ref oneOf. `amount` is the friendly 0–100 grain knob
    // mapped onto the ffmpeg noise strength; `monochrome` picks neutral luma-only
    // grain (default, the classic film look) vs colored grain on all channels;
    // `format` optionally converts the output.
    ToolDescriptor::new(Input::Image)
        .param(
            Param::number("amount")
                .min(0.0)
                .max(100.0)
                .default(DEFAULT_AMOUNT)
                .describe(
                    "Grain amount, 0-100. 0 leaves the image unchanged, 20 (the default) is a \
                     subtle film-like speckle, 50+ is heavy, pushed-film grain, 100 is the \
                     maximum. Example: 40 for a noticeable analog look.",
                ),
        )
        .param(
            Param::boolean("monochrome")
                .default(DEFAULT_MONOCHROME)
                .describe(
                    "true (default) applies neutral gray grain to brightness only — the classic \
                     black-and-white film look that stays natural over color photos; false \
                     applies colored grain to every channel (RGB speckle), the look of pushed \
                     high-ISO color film.",
                ),
        )
        .param(Param::enumv("format", FORMATS).default(DEFAULT_FORMAT).describe(
            "Output format: keep (default) preserves the input format (and any animation); \
             png, jpg or webp convert — an animated GIF keeps only its first frame when \
             converting.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-film-grain",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Overlay adjustable analog film grain on an image — neutral monochrome or colored, with an optional format convert.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Add analog film grain to an image. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call); optional amount 0-100 (default 20), monochrome true|false (default true — neutral luma-only grain vs colored grain), and format keep|png|jpg|webp (default keep).",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid image-film-grain args: {e}")))?;
    let amount = args.amount.unwrap_or(DEFAULT_AMOUNT);
    let monochrome = args.monochrome.unwrap_or(DEFAULT_MONOCHROME);
    let format = parse_format(args.format.as_deref()).map_err(SkillError::InvalidArgs)?;
    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let in_path = format!("in.{ext}");
    let (argv, out_name) =
        gizza_ai_image_film_grain_core::plan(&in_path, amount, monochrome, format)
            .map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, in_path, bytes, out_name)?;
    // The envelope must describe the OUTPUT: converting changes mime + name.
    let (out_mime, out_display) = match format {
        OutFormat::Keep => (mime.clone(), in_name.clone()),
        OutFormat::Png => ("image/png".to_string(), rename_ext(&in_name, "png")),
        OutFormat::Jpg => ("image/jpeg".to_string(), rename_ext(&in_name, "jpg")),
        OutFormat::Webp => ("image/webp".to_string(), rename_ext(&in_name, "webp")),
    };
    let kind = if monochrome { "monochrome" } else { "colored" };
    let mut for_llm = format!("added {kind} film grain (amount {amount}) to {in_name}");
    if format != OutFormat::Keep {
        for_llm.push_str(&format!(" and converted it to {out_display}"));
    }
    build_media_envelope(&output, &out_mime, out_display, for_llm, MAX_BYTES)
}

/// `photo.jpeg` + `png` → `photo.png` (append when there is no extension).
#[cfg(target_arch = "wasm32")]
fn rename_ext(name: &str, ext: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => format!("{stem}.{ext}"),
        _ => format!("{name}.{ext}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gizza_ai_image_film_grain_core::parse_bool;

    /// Drift-guard: the descriptor-derived chat schema must match the authored
    /// schema below, so the LLM-facing tool definition never silently changes.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "amount": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 100,
                        "default": 20.0,
                        "description": "Grain amount, 0-100. 0 leaves the image unchanged, 20 (the default) is a subtle film-like speckle, 50+ is heavy, pushed-film grain, 100 is the maximum. Example: 40 for a noticeable analog look."
                    },
                    "monochrome": {
                        "type": "boolean",
                        "default": true,
                        "description": "true (default) applies neutral gray grain to brightness only — the classic black-and-white film look that stays natural over color photos; false applies colored grain to every channel (RGB speckle), the look of pushed high-ISO color film."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["keep", "png", "jpg", "webp"],
                        "default": "keep",
                        "description": "Output format: keep (default) preserves the input format (and any animation); png, jpg or webp convert — an animated GIF keeps only its first frame when converting."
                    }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn descriptor_exposes_every_format() {
        let json = schema_json();
        for name in FORMATS {
            assert!(json.contains(name), "schema format enum must list {name}");
        }
    }

    #[test]
    fn defaults_are_wired_through() {
        assert_eq!(DEFAULT_AMOUNT, 20.0);
        assert!(DEFAULT_MONOCHROME);
        assert_eq!(DEFAULT_FORMAT, "keep");
        // parse_bool/parse_format are the core helpers the run path relies on.
        assert!(parse_bool(None, DEFAULT_MONOCHROME).unwrap());
        assert_eq!(parse_format(None).unwrap(), OutFormat::Keep);
    }
}
