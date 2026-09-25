//! gizza-ai/gltf-glb-converter — chat skill block on the shared tool abstraction.
//! Converts a glTF 2.0 asset between the text `.gltf` container (JSON plus
//! external/`data:` buffers) and the packed binary `.glb` container, in either
//! direction, including pulling an external `.bin` in and moving image bytes
//! between the buffer and `data:` URIs. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    model: String,
    #[serde(default)]
    bin: String,
    #[serde(default = "default_auto")]
    input_format: String,
    #[serde(default = "default_auto")]
    to: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_auto")]
    images: String,
    #[serde(default)]
    buffer_uri: String,
    #[serde(default = "default_pretty")]
    pretty: bool,
    #[serde(default = "default_output_encoding")]
    output_encoding: String,
}

fn default_auto() -> String {
    "auto".to_string()
}
fn default_output() -> String {
    "file".to_string()
}
fn default_pretty() -> bool {
    true
}
fn default_output_encoding() -> String {
    "data-url".to_string()
}

const MODEL_DESC: &str = "The glTF 2.0 asset to convert. Paste the .gltf JSON as text (it starts \
    with '{' and contains \"asset\": { \"version\": \"2.0\" }), or a .glb file's bytes encoded as \
    base64 (they start 'Z2xURg') or hex (they start '676c5446'); a \
    data:model/gltf-binary;base64,... URL works too. GLB is binary, so it cannot be pasted \
    directly. Up to 16 MB of model bytes.";
const BIN_DESC: &str = "Optional bytes of the external buffer file a .gltf references, e.g. the \
    scene.bin next to it, as base64 or hex or a data: URL. Supply this when packing a .gltf whose \
    buffer uri is a relative filename rather than a data: URI — the converter has no file access, \
    so it cannot read that file itself. Leave blank for GLB input or for a .gltf whose buffers are \
    already data: URIs.";
const INPUT_FORMAT_DESC: &str = "How the pasted model is encoded. 'auto' (default) detects glTF \
    JSON text, a data: URL, hex bytes or base64 bytes. Set 'gltf', 'base64' or 'hex' to force one \
    — useful when a base64 blob would otherwise read as hex.";
const TO_DESC: &str = "Which container to write. 'auto' (default) flips the input: GLB in gives \
    glTF JSON out, glTF JSON in gives GLB out. 'glb' always packs to binary GLB, 'gltf' always \
    writes glTF JSON — pick one of these to re-pack an asset into the container it is already in \
    (for example to embed an external .bin into a .gltf).";
const OUTPUT_DESC: &str = "What to return. 'file' (default) returns the converted model. \
    'summary' returns a readable report of the conversion: direction, chunk sizes, scene/mesh/\
    material counts, vertex and triangle totals, and any warnings. 'buffer' returns only the \
    binary buffer — the scene.bin you save beside an unpacked .gltf.";
const IMAGES_DESC: &str = "Where texture image bytes should live. 'auto' (default) packs data: \
    URI images into the binary buffer when writing GLB and otherwise leaves images untouched, \
    which keeps the conversion byte-exact. 'buffer' always moves data: URI images into the buffer \
    as buffer views. 'uri' always pulls buffer-view images back out into data: URIs, which is what \
    makes an unpacked .gltf viewable on its own.";
const BUFFER_URI_DESC: &str = "When writing glTF JSON, the uri to record for the binary buffer \
    instead of embedding it. Blank (default) embeds the buffer as a \
    data:application/octet-stream;base64 URI so the .gltf is a single self-contained file. Set it \
    to a filename such as scene.bin to get the classic two-file layout, then re-run with \
    output=buffer to download those bytes.";
const PRETTY_DESC: &str = "Pretty-print the glTF JSON with two-space indentation so it is \
    readable and diffable. Turn it off for the smallest .gltf. Ignored for GLB output, whose JSON \
    chunk is always written compactly. Default true.";
const OUTPUT_ENCODING_DESC: &str = "How binary output (a GLB, or an extracted buffer) is \
    returned. 'data-url' (default) returns a data:model/gltf-binary;base64,... URL a browser can \
    save straight to a .glb file. 'base64' and 'hex' return the raw bytes for piping elsewhere. \
    Ignored when the output is glTF JSON.";

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("model").required().describe(MODEL_DESC))
        .param(Param::string("bin").default("").describe(BIN_DESC))
        .param(
            Param::enumv("input_format", ["auto", "gltf", "base64", "hex"])
                .default("auto")
                .describe(INPUT_FORMAT_DESC),
        )
        .param(
            Param::enumv("to", ["auto", "glb", "gltf"])
                .default("auto")
                .describe(TO_DESC),
        )
        .param(
            Param::enumv("output", ["file", "summary", "buffer"])
                .default("file")
                .describe(OUTPUT_DESC),
        )
        .param(
            Param::enumv("images", ["auto", "buffer", "uri"])
                .default("auto")
                .describe(IMAGES_DESC),
        )
        .param(
            Param::string("buffer_uri")
                .default("")
                .describe(BUFFER_URI_DESC),
        )
        .param(Param::boolean("pretty").default(true).describe(PRETTY_DESC))
        .param(
            Param::enumv("output_encoding", ["data-url", "base64", "hex"])
                .default("data-url")
                .describe(OUTPUT_ENCODING_DESC),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn convert_args(a: Args) -> Result<String, SkillError> {
    gizza_ai_gltf_glb_converter_core::run(
        &a.model,
        &a.bin,
        &a.input_format,
        &a.to,
        &a.output,
        &a.images,
        &a.buffer_uri,
        a.pretty,
        &a.output_encoding,
    )
    .map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gltf-glb-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a glTF 2.0 asset between text .gltf and packed binary .glb, either direction.",
    skill(
        description = "Convert a glTF 2.0 asset between the text .gltf container and the packed binary .glb container, in either direction. Paste .gltf JSON to get a GLB back as a data:model/gltf-binary;base64 URL, or paste a GLB's bytes as base64/hex to get glTF JSON back with its buffer embedded as a data: URI. Also packs an external scene.bin supplied as base64 into the output, merges multi-buffer assets into one, moves texture image bytes between the binary buffer and data: URIs, can write an external buffer uri and hand back the .bin separately, and can return a conversion summary with chunk sizes plus scene/mesh/material/vertex/triangle counts. Accessor bytes are copied unchanged, so a single-buffer GLB to glTF to GLB round trip is byte-exact. Cannot read files from disk (external buffers and textures must be pasted), does not decompress Draco or meshopt, and does not optimize, weld or quantize geometry. Runs offline with no network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "gltf-glb-converter", convert_args) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRIANGLE: &str = r#"{"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36}],"buffers":[{"uri":"data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAA","byteLength":36}]}"#;

    #[test]
    fn descriptor_documents_every_param() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        assert_eq!(props.len(), 9);
        for (name, spec) in props {
            assert!(
                spec["description"].as_str().unwrap_or_default().len() > 40,
                "{name} needs a useful description"
            );
        }
        assert_eq!(schema["required"], serde_json::json!(["model"]));
    }

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// copy exactly. Regenerate it deliberately whenever the params change.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let authored = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["model"],
            "properties": {
                "model": { "type": "string", "description": MODEL_DESC },
                "bin": { "type": "string", "default": "", "description": BIN_DESC },
                "input_format": { "type": "string", "enum": ["auto", "gltf", "base64", "hex"], "default": "auto", "description": INPUT_FORMAT_DESC },
                "to": { "type": "string", "enum": ["auto", "glb", "gltf"], "default": "auto", "description": TO_DESC },
                "output": { "type": "string", "enum": ["file", "summary", "buffer"], "default": "file", "description": OUTPUT_DESC },
                "images": { "type": "string", "enum": ["auto", "buffer", "uri"], "default": "auto", "description": IMAGES_DESC },
                "buffer_uri": { "type": "string", "default": "", "description": BUFFER_URI_DESC },
                "pretty": { "type": "boolean", "default": true, "description": PRETTY_DESC },
                "output_encoding": { "type": "string", "enum": ["data-url", "base64", "hex"], "default": "data-url", "description": OUTPUT_ENCODING_DESC }
            }
        });
        assert_eq!(derived, authored);
    }

    #[test]
    fn serde_defaults_match_schema_defaults_and_pack_a_gltf() {
        let a: Args = serde_json::from_str(&format!(
            "{{\"model\":{}}}",
            serde_json::to_string(TRIANGLE).unwrap()
        ))
        .unwrap();
        assert_eq!(a.bin, "");
        assert_eq!(a.input_format, "auto");
        assert_eq!(a.to, "auto");
        assert_eq!(a.output, "file");
        assert_eq!(a.images, "auto");
        assert_eq!(a.buffer_uri, "");
        assert!(a.pretty);
        assert_eq!(a.output_encoding, "data-url");
        let out = convert_args(a).unwrap();
        assert!(
            out.starts_with("data:model/gltf-binary;base64,Z2xURgIA"),
            "{out}"
        );
    }

    #[test]
    fn bad_target_is_an_invalid_argument() {
        let a: Args = serde_json::from_str(&format!(
            "{{\"model\":{},\"to\":\"fbx\"}}",
            serde_json::to_string(TRIANGLE).unwrap()
        ))
        .unwrap();
        match convert_args(a).unwrap_err() {
            SkillError::InvalidArgs(msg) => {
                assert!(msg.contains("expected 'auto', 'glb' or 'gltf'"), "{msg}")
            }
            other => panic!("wrong error: {other:?}"),
        }
    }

    #[test]
    fn unreadable_external_buffer_is_an_invalid_argument() {
        let a: Args = serde_json::from_str(
            r#"{"model":"{\"asset\":{\"version\":\"2.0\"},\"buffers\":[{\"uri\":\"scene.bin\",\"byteLength\":4}]}"}"#,
        )
        .unwrap();
        match convert_args(a).unwrap_err() {
            SkillError::InvalidArgs(msg) => {
                assert!(msg.contains("scene.bin"), "{msg}");
                assert!(msg.contains("external buffer field"), "{msg}");
            }
            other => panic!("wrong error: {other:?}"),
        }
    }
}
