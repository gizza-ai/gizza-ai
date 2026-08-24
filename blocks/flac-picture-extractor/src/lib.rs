//! gizza-ai/flac-picture-extractor — extract embedded artwork from a native FLAC
//! file and return the selected picture's bytes with a full metadata dump.
//!
//! Pipeline: resolve the file source (URL/ref) → parse the FLAC metadata chain →
//! select by picture type + 1-based index → wrap the image bytes in the shared
//! media envelope. Pure Rust; chat + CLI; no page for file-in → image-bytes-out.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source, AssetKind};
use gizza_ai_block_utils::{
    Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 128 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

const PICTURE_TYPE_VALUES: [&str; 22] = [
    "any",
    "other",
    "file-icon",
    "other-file-icon",
    "front-cover",
    "back-cover",
    "leaflet-page",
    "media",
    "lead-artist",
    "artist",
    "conductor",
    "band",
    "composer",
    "lyricist",
    "recording-location",
    "during-recording",
    "during-performance",
    "video-screen-capture",
    "bright-colored-fish",
    "illustration",
    "band-logo",
    "publisher-logo",
];

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    picture_type: Option<String>,
    #[serde(default)]
    picture_index: Option<u32>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::enumv("picture_type", PICTURE_TYPE_VALUES)
                .default("any")
                .label("Picture type")
                .describe("Which embedded FLAC picture role to extract. Use any for the first picture of any role, or choose one of the FLAC/ID3 APIC roles such as front-cover, back-cover, artist, band-logo or publisher-logo. Default: any."),
        )
        .param(
            Param::integer("picture_index")
                .min(1.0)
                .default(1)
                .label("Picture index")
                .describe("1-based index within the selected picture_type. With picture_type=any this is the file-order picture number; with picture_type=front-cover it is the Nth front-cover picture. Default: 1."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FlacPictureExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/flac-picture-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract embedded artwork from a native FLAC file",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Extract embedded artwork from a native FLAC file and return the selected image bytes plus a full metadata report. Reads native FLAC PICTURE metadata blocks, base64 METADATA_BLOCK_PICTURE entries in VORBIS_COMMENT, and the deprecated COVERART/COVERARTMIME pair. Reports picture type number/name/slug, MIME, description, declared dimensions/depth/colors, actual image-header dimensions for PNG/JPEG/GIF/WebP/BMP, byte size, source block, and an inventory of every embedded picture. Params: picture_type (any or a FLAC/ID3 APIC role such as front-cover, back-cover, artist, band-logo; default any) and picture_index (1-based, default 1). Native FLAC only: MP3 APIC, MP4 covr, WMA and Ogg-FLAC are named in clear errors. Provide the file as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl FlacPictureExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_flac_picture_extractor_core as core;

    let args: Args = serde_json::from_slice(&body).invalid_args("flac-picture-extractor")?;
    let picture_type = core::parse_type_filter(args.picture_type.as_deref().unwrap_or("any"))
        .map_err(SkillError::InvalidArgs)?;
    let picture_index = args.picture_index.unwrap_or(1) as usize;
    if picture_index == 0 {
        return Err(SkillError::InvalidArgs(
            "picture_index must be >= 1 (pictures are 1-based)".to_string(),
        ));
    }

    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;
    let report = core::parse(&bytes).map_err(SkillError::InvalidArgs)?;
    let selected =
        core::select(&report, &picture_type, picture_index).map_err(SkillError::InvalidArgs)?;
    if selected.is_url {
        let url = String::from_utf8_lossy(&selected.data);
        return Err(SkillError::InvalidArgs(format!(
            "the selected FLAC picture uses MIME '-->', so it stores a link instead of image bytes: {url}"
        )));
    }
    let llm = core::report_text(&report, selected);
    build_media_envelope(
        &selected.data,
        &selected.output_mime(),
        selected.filename(),
        llm,
        MAX_OUTPUT_BYTES,
    )
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
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "picture_type": {
                        "type": "string",
                        "enum": ["any", "other", "file-icon", "other-file-icon", "front-cover", "back-cover", "leaflet-page", "media", "lead-artist", "artist", "conductor", "band", "composer", "lyricist", "recording-location", "during-recording", "during-performance", "video-screen-capture", "bright-colored-fish", "illustration", "band-logo", "publisher-logo"],
                        "default": "any",
                        "description": "Which embedded FLAC picture role to extract. Use any for the first picture of any role, or choose one of the FLAC/ID3 APIC roles such as front-cover, back-cover, artist, band-logo or publisher-logo. Default: any."
                    },
                    "picture_index": {
                        "type": "integer",
                        "minimum": 1,
                        "default": 1,
                        "description": "1-based index within the selected picture_type. With picture_type=any this is the file-order picture number; with picture_type=front-cover it is the Nth front-cover picture. Default: 1."
                    }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn every_param_is_documented_and_fixed_choices_are_enums() {
        use gizza_ai_block_utils::ParamKind;
        let d = descriptor();
        assert_eq!(d.input, Input::File);
        for p in &d.params {
            assert!(!p.description.is_empty(), "{} needs a describe()", p.name);
        }
        let picture_type = d
            .params
            .iter()
            .find(|p| p.name == "picture_type")
            .expect("picture_type param");
        assert!(
            matches!(picture_type.kind, ParamKind::Enum(_)),
            "fixed-choice param must be Param::enumv"
        );
    }
}
