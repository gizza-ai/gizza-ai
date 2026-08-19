//! gizza-ai/audio-format-conformance-checker — decide whether an audio file
//! really IS the format it claims to be, and whether the codec inside the
//! container is the expected one.
//!
//! Pipeline: resolve the file source (URL/ref) → `core::check` (magic-byte sniff
//! + symphonia stream parse + claim comparison) → flat JSON the LLM reads
//! directly.
//!
//! `AssetKind::Any` (not `Audio`) is deliberate: the whole point is to catch a
//! `.mp3` that is really a PNG or an HTML error page, so the transport MIME must
//! NOT be allowed to reject the input before the bytes are inspected.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a media-file → text verdict fits neither the
//! pure-text page nor the ffmpeg file→media page shape — the F3 "no-page
//! file-input" pattern, like media-info / detect-file-type / image-format-validator).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

/// 20 MiB. The wasm sandbox is 64 MiB and the stream parse holds a second copy
/// of the buffer, so this is the honest ceiling — stated in the parameter copy
/// and the skill description rather than discovered via an error.
const MAX_BYTES: usize = 20 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Format the file claims to be, or "auto" to fall back to the filename.
    #[serde(default)]
    claimed_format: Option<String>,
    /// Codec expected inside the container, or "any" to skip the codec check.
    #[serde(default)]
    expected_codec: Option<String>,
    /// Reject a claim that is only a container relative of the real format.
    #[serde(default)]
    strict: Option<bool>,
}

#[derive(Serialize)]
struct Resp {
    #[serde(flatten)]
    report: gizza_ai_audio_format_conformance_checker_core::Report,
    /// Source filename, when one was available.
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

/// Every format word `claimed_format` accepts. `auto` = take the claim from the
/// filename extension instead. Kept in sync with the core's `normalise_claim`.
const CLAIMED_FORMATS: [&str; 25] = [
    "auto", "mp3", "wav", "aiff", "aifc", "flac", "ogg", "oga", "opus", "spx", "m4a", "m4b", "mp4",
    "aac", "caf", "mka", "wma", "amr", "mid", "ape", "wv", "tta", "mpc", "au", "dsf",
];

/// Codec names the stream parse / signature can report. `any` skips the check.
const EXPECTED_CODECS: [&str; 12] = [
    "any", "pcm", "adpcm", "mp1", "mp2", "mp3", "aac", "flac", "alac", "vorbis", "opus", "speex",
];

/// `Input::File` emits the `url`⊕`ref` `oneOf` — the file arrives via URL fetch
/// or an attachment ref. Three fixed-choice/boolean controls shape the check.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::enumv("claimed_format", CLAIMED_FORMATS)
                .default("auto")
                .describe(
                    "Format the file claims to be, to check its real bytes against. Use 'auto' \
                     (the default) to take the claim from the filename extension instead. \
                     Example: claimed_format=mp3 on a file that is really a WAV returns \
                     verdict=container_mismatch with suggested_extension=wav.",
                ),
        )
        .param(
            Param::enumv("expected_codec", EXPECTED_CODECS)
                .default("any")
                .describe(
                    "Codec you expect INSIDE the container, checked separately from the \
                     container itself — e.g. expected_codec=aac on an .m4a that actually holds \
                     ALAC returns verdict=codec_mismatch. 'any' (the default) reports the codec \
                     without judging it.",
                ),
        )
        .param(
            Param::boolean("strict")
                .default(false)
                .describe(
                    "Reject a claimed extension that is only a RELATIVE of the real format \
                     rather than an exact match — .ogg for an Opus-in-Ogg file, or .mp4 for an \
                     M4A-branded file. Off by default (relatives pass); turn it on to require \
                     the exact canonical extension.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AudioFormatConformanceChecker;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-format-conformance-checker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Check whether an audio file really is the format and codec it claims to be",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Check an audio file for format conformance: identify its TRUE container from the magic bytes (MP3, WAV/RF64, AIFF/AIFF-C, FLAC, Ogg Vorbis/Opus/Speex/FLAC, M4A/M4B/MP4, AAC-ADTS, CAF, Matroska/WebM, WMA/ASF, AMR, MIDI, APE, WavPack, TTA, Musepack, AU, DSF), read the codec actually inside the container with the pure-Rust symphonia demuxers (PCM, ADPCM, MP1/MP2/MP3, AAC, FLAC, ALAC, Vorbis, Opus), and compare both against what the file claims. The claim comes from claimed_format, or from the filename extension when claimed_format is 'auto'. Returns a verdict (conformant, container_mismatch, codec_mismatch, codec_unverified, no_audio_track, not_audio, unrecognised, unknown_claim, no_claim), a one-line summary, an issues list, the signature bytes as hex, MIME type and format category, sample rate, channels, bit depth, duration and bitrate, plus suggested_extension/suggested_filename when the file is mislabelled. Also names common NON-audio content (PNG, JPEG, PDF, ZIP, HTML error pages) so a renamed or failed download is obvious. Never throws on odd input — a file whose container is known but whose codec cannot be read still gets a container verdict plus parse_error. Set strict=true to reject same-family relatives (.ogg for Opus, .mp4 for M4A). Files up to 20 MiB. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl AudioFormatConformanceChecker {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args =
        serde_json::from_slice(&body).invalid_args("audio-format-conformance-checker")?;
    // AssetKind::Any: a mislabelled file's transport MIME is exactly what we are
    // here to contradict, so it must not gate the fetch.
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let filename_opt = (!filename.is_empty()).then(|| filename.clone());
    let report = gizza_ai_audio_format_conformance_checker_core::check(
        &bytes,
        args.claimed_format.as_deref(),
        args.expected_codec.as_deref(),
        args.strict.unwrap_or(false),
        filename_opt.as_deref(),
    )
    .map_err(SkillError::InvalidArgs)?;

    let resp = Resp {
        report,
        filename: filename_opt,
    };
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!(
            "serialize audio-format-conformance-checker response: {e}"
        ))
    })
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
                    "claimed_format": {
                        "type": "string",
                        "enum": ["auto", "mp3", "wav", "aiff", "aifc", "flac", "ogg", "oga", "opus", "spx", "m4a", "m4b", "mp4", "aac", "caf", "mka", "wma", "amr", "mid", "ape", "wv", "tta", "mpc", "au", "dsf"],
                        "default": "auto",
                        "description": "Format the file claims to be, to check its real bytes against. Use 'auto' (the default) to take the claim from the filename extension instead. Example: claimed_format=mp3 on a file that is really a WAV returns verdict=container_mismatch with suggested_extension=wav."
                    },
                    "expected_codec": {
                        "type": "string",
                        "enum": ["any", "pcm", "adpcm", "mp1", "mp2", "mp3", "aac", "flac", "alac", "vorbis", "opus", "speex"],
                        "default": "any",
                        "description": "Codec you expect INSIDE the container, checked separately from the container itself — e.g. expected_codec=aac on an .m4a that actually holds ALAC returns verdict=codec_mismatch. 'any' (the default) reports the codec without judging it."
                    },
                    "strict": {
                        "type": "boolean",
                        "default": false,
                        "description": "Reject a claimed extension that is only a RELATIVE of the real format rather than an exact match — .ogg for an Opus-in-Ogg file, or .mp4 for an M4A-branded file. Off by default (relatives pass); turn it on to require the exact canonical extension."
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

    /// Every format word the descriptor advertises must be one the core can
    /// normalise — otherwise a documented choice silently yields `unknown_claim`.
    #[test]
    fn every_advertised_claimed_format_is_understood_by_the_core() {
        let wav = b"RIFF\x24\x08\x00\x00WAVEfmt ";
        for f in CLAIMED_FORMATS.iter().filter(|f| **f != "auto") {
            let r =
                gizza_ai_audio_format_conformance_checker_core::check(wav, Some(f), None, false, None)
                    .unwrap();
            assert_ne!(r.verdict, "unknown_claim", "claimed_format={f} not understood");
        }
    }

    /// Every advertised codec word must be comparable against a real codec name
    /// (the core lowercases both sides), i.e. it must round-trip as a match for
    /// the codec it names.
    #[test]
    fn every_advertised_expected_codec_can_match() {
        // A real PCM WAV: the only advertised codec we can produce without a
        // fixture, so assert the match path and the mismatch path around it.
        let mut wav = b"RIFF".to_vec();
        wav.extend_from_slice(&(36u32 + 400).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&44100u32.to_le_bytes());
        wav.extend_from_slice(&(44100u32 * 4).to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&400u32.to_le_bytes());
        wav.resize(44 + 400, 0);

        for c in EXPECTED_CODECS.iter() {
            let r = gizza_ai_audio_format_conformance_checker_core::check(
                &wav,
                Some("wav"),
                Some(c),
                false,
                None,
            )
            .unwrap();
            let expect_ok = *c == "any" || *c == "pcm";
            assert_eq!(
                r.verdict == "conformant",
                expect_ok,
                "expected_codec={c} gave verdict {}",
                r.verdict
            );
        }
    }
}
