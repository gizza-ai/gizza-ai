//! gizza-ai/bpm-key-tag-reader — report the tempo (BPM) and musical key that
//! are already STORED in a track's tags, normalised into traditional, Camelot
//! and Open Key notation.
//!
//! Pipeline: resolve the audio source (URL/ref) → `core::read` (demux tags via
//! the pure-Rust symphonia metadata readers) → flat JSON the LLM reads directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a media-file → text report fits neither the
//! pure-text page nor the ffmpeg file→media page shape — the same "no-page
//! file-input" pattern as media-info / file-metadata-inspect / detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 256 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    key_notation: Option<String>,
    #[serde(default)]
    include_all_tags: Option<bool>,
}

#[derive(Serialize)]
struct TagResp {
    field: String,
    value: String,
}

#[derive(Serialize)]
struct BpmResp {
    bpm: f64,
    bpm_rounded: u32,
    source_field: String,
    raw: String,
}

#[derive(Serialize)]
struct KeyResp {
    key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    standard: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    camelot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    open_key: Option<String>,
    tonic: String,
    mode: String,
    source_field: String,
    raw: String,
}

#[derive(Serialize)]
struct EnergyResp {
    energy: String,
    source_field: String,
}

#[derive(Serialize)]
struct Resp {
    container: String,
    found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    bpm: Option<BpmResp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<KeyResp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    energy: Option<EnergyResp>,
    matched_tags: Vec<TagResp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    all_tags: Option<Vec<TagResp>>,
    tag_count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    notes: Vec<String>,
    bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

/// `Input::Audio` emits the `url`⊕`ref` `oneOf` with the "Audio URL" wording.
/// Two knobs: which key notation to report in, and whether to dump every other
/// tag alongside the tempo/key ones.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::enumv("key_notation", ["standard", "camelot", "open-key", "all"])
                .default("all")
                .label("Key notation")
                .describe(
                    "Which notation to report the musical key in: standard (traditional \
                     names like 'A minor'), camelot (harmonic-mixing wheel codes like '8A'), \
                     open-key (codes like '1m'), or all three at once. Default: all.",
                ),
        )
        .param(
            Param::boolean("include_all_tags")
                .default(false)
                .label("List every tag")
                .describe(
                    "Also list every other tag stored in the file, not just the tempo, key \
                     and energy ones. Useful for finding a non-standard field a DJ app used. \
                     Default: false.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct BpmKeyTagReader;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bpm-key-tag-reader",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Read the stored tempo and musical-key tags from a track and convert the key to Camelot or Open Key",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Read the tempo (BPM) and musical key that are already STORED in a track's tags, and convert the key into traditional, Camelot (harmonic-mixing wheel) and Open Key notation. Reads ID3v2 TBPM/TKEY and the TXXX: frames DJ software writes (INITIALKEY, EnergyLevel), Vorbis comments (BPM, INITIALKEY, KEY), the MP4/iTunes tmpo atom and com.apple.iTunes freeform atoms, plus RIFF INFO and ID3v1 — across MP3, FLAC, OGG, M4A/MP4, WAV, AIFF, CAF and MKV/WebM. Reports which literal field each value came from, and understands keys stored as names (Am, F#m, C major), Camelot codes (8A) or Open Key codes (1m). This reads existing tags only — it does NOT analyse the audio, so an untagged file reports that no tempo/key tags are stored. WMA/ASF and APEv2 tags are not supported. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl BpmKeyTagReader {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_bpm_key_tag_reader_core as core;

    let args: Args = serde_json::from_slice(&body).invalid_args("bpm-key-tag-reader")?;

    let key_notation = core::KeyNotation::parse(args.key_notation.as_deref().unwrap_or("all"))
        .map_err(SkillError::InvalidArgs)?;
    let opts = core::Options {
        key_notation,
        include_all_tags: args.include_all_tags.unwrap_or(false),
    };

    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_BYTES)?;

    let r = core::read(&bytes, opts).map_err(SkillError::InvalidArgs)?;

    let resp = Resp {
        container: r.container,
        found: r.found,
        bpm: r.bpm.map(|b| BpmResp {
            bpm: b.bpm,
            bpm_rounded: b.bpm_rounded,
            source_field: b.source_field,
            raw: b.raw,
        }),
        key: r.key.map(|k| KeyResp {
            key: k.display,
            standard: k.standard,
            camelot: k.camelot,
            open_key: k.open_key,
            tonic: k.tonic,
            mode: k.mode,
            source_field: k.source_field,
            raw: k.raw,
        }),
        energy: r.energy.map(|e| EnergyResp {
            energy: e.energy,
            source_field: e.source_field,
        }),
        matched_tags: r
            .matched_tags
            .into_iter()
            .map(|t| TagResp {
                field: t.field,
                value: t.value,
            })
            .collect(),
        all_tags: r.all_tags.map(|v| {
            v.into_iter()
                .map(|t| TagResp {
                    field: t.field,
                    value: t.value,
                })
                .collect()
        }),
        tag_count: r.tag_count,
        notes: r.notes,
        bytes: bytes.len(),
        filename: (!filename.is_empty()).then_some(filename),
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize bpm-key-tag-reader response: {e}")))
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
                    "url": { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "key_notation": {
                        "type": "string",
                        "enum": ["standard", "camelot", "open-key", "all"],
                        "default": "all",
                        "description": "Which notation to report the musical key in: standard (traditional names like 'A minor'), camelot (harmonic-mixing wheel codes like '8A'), open-key (codes like '1m'), or all three at once. Default: all."
                    },
                    "include_all_tags": {
                        "type": "boolean",
                        "default": false,
                        "description": "Also list every other tag stored in the file, not just the tempo, key and energy ones. Useful for finding a non-standard field a DJ app used. Default: false."
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
        assert_eq!(d.input, Input::Audio);
        for p in &d.params {
            assert!(!p.description.is_empty(), "{} needs a describe()", p.name);
        }
        let notation = d
            .params
            .iter()
            .find(|p| p.name == "key_notation")
            .expect("key_notation param");
        assert!(
            matches!(notation.kind, ParamKind::Enum(_)),
            "fixed-choice param must be Param::enumv"
        );
    }
}
