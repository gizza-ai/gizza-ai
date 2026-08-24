//! gizza-ai/wav-to-numpy-npy — export decoded WAV PCM samples as a NumPy `.npy`
//! array file.
//!
//! Thin chat-skill wrapper around `gizza-ai-wav-to-numpy-npy-core`. The
//! descriptor is the single source for the chat schema, CLI, and generated page
//! controls; `handle()` delegates to `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    input_format: String,
    #[serde(default)]
    dtype: String,
    #[serde(default)]
    shape: String,
    #[serde(default)]
    mono: bool,
    #[serde(default)]
    fortran_order: bool,
    #[serde(default)]
    start_frame: u64,
    #[serde(default)]
    max_frames: u64,
    #[serde(default)]
    output: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("Uncompressed WAV audio bytes encoded as base64 (default) or hex. Only RIFF/WAVE PCM 8/16/24/32-bit integer or 32/64-bit IEEE float is decoded; compressed MP3/AAC/FLAC/Ogg and companded A-law/mu-law input is rejected with a clear message."),
        )
        .param(
            Param::enumv("input_format", ["base64", "hex"])
                .default("base64")
                .describe("Encoding of the pasted WAV bytes: 'base64' (default) or 'hex'. Hex may include whitespace, ':' or '-' separators."),
        )
        .param(
            Param::enumv(
                "dtype",
                ["float32", "float64", "int16", "int32", "uint8", "auto"],
            )
                .default("float32")
                .describe("NumPy dtype of the exported array. 'float32' (default) and 'float64' write the normalized amplitude in [-1,1]; 'int16', 'int32' and 'uint8' write integers scaled to that type's full range; 'auto' keeps the source's own dtype and its raw stored values, matching scipy.io.wavfile.read (8-bit->uint8, 16-bit->int16, 24/32-bit->int32 left-justified, float->float32/float64)."),
        )
        .param(
            Param::enumv(
                "shape",
                ["auto", "frames_channels", "channels_frames", "flat"],
            )
                .default("auto")
                .describe("Array shape. 'auto' (default) is 1-D (frames,) for mono and 2-D (frames, channels) otherwise, like scipy/soundfile; 'frames_channels' is always 2-D even for mono (soundfile's always_2d); 'channels_frames' transposes to (channels, frames), the torchaudio layout; 'flat' is a 1-D interleaved array of every sample."),
        )
        .param(
            Param::boolean("mono")
                .default(false)
                .describe("Average all channels down to one before exporting (default false, which keeps every channel). A downmix is lossy and cannot be undone, so it is opt-in."),
        )
        .param(
            Param::boolean("fortran_order")
                .default(false)
                .describe("Write column-major (Fortran) data and set the .npy header's fortran_order flag. Default false = C order, what numpy.save writes for a freshly read array. Ignored for 1-D shapes, which NumPy always records as fortran_order: False."),
        )
        .param(
            Param::integer("start_frame")
                .min(0.0)
                .default(0)
                .describe("Zero-based index of the first sample frame to export (default 0). One frame is one sample per channel, so at 44100 Hz frame 44100 is one second in. Errors if it is at or past the end of the clip."),
        )
        .param(
            Param::integer("max_frames")
                .min(0.0)
                .max(1_000_000.0)
                .default(0)
                .describe("How many sample frames to export, starting at start_frame (0-1000000). '0' (the default) exports to the end of the clip. The export is additionally bounded by a 4000000-value array cap and a per-output byte cap."),
        )
        .param(
            Param::enumv("output", ["base64", "hex", "info"])
                .default("base64")
                .describe("How the .npy file is returned: 'base64' (default, decode it with `base64 -d > audio.npy`), 'hex' (one unbroken run, reversed by `xxd -r -p`), or 'info' (a report of the source format, the resulting dtype/shape/order/byte sizes and a ready-to-run np.load snippet, with no sample bytes)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct WavToNumpyNpy;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/wav-to-numpy-npy",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Export decoded WAV PCM samples as a NumPy .npy array file.",
    skill(
        description = "Decode an uncompressed WAV clip (pasted as base64 or hex bytes) and export its PCM samples as a NumPy .npy v1.0 array file that np.load() reads back directly. Choose the dtype (float32/float64 normalized to [-1,1], int16/int32/uint8 scaled to full range, or 'auto' for the source's own dtype and raw stored values the way scipy.io.wavfile.read returns them), the shape (1-D for mono and (frames, channels) otherwise, always-2-D, channels-first, or flat interleaved), an optional mono downmix, C or Fortran memory order, and a start_frame/max_frames window. Returns the .npy as base64 or hex, or an 'info' report of the source format, the resulting array header and byte sizes plus a np.load snippet. Decodes RIFF/WAVE PCM 8/16/24/32-bit integer and 32/64-bit IEEE float; compressed MP3/AAC/FLAC/Ogg and companded A-law/mu-law input is rejected clearly rather than guessed.",
        parameters = schema_json()
    ),
)]
impl WavToNumpyNpy {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "wav-to-numpy-npy", |a: Args| {
            gizza_ai_wav_to_numpy_npy_core::run(
                &a.input,
                &a.input_format,
                &a.dtype,
                &a.shape,
                a.mono,
                a.fortran_order,
                a.start_frame,
                a.max_frames,
                &a.output,
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
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema.get("properties").unwrap();

        assert_eq!(schema.get("required").unwrap(), &serde_json::json!(["input"]));
        assert_eq!(schema.get("additionalProperties").unwrap(), false);

        assert_eq!(
            props["input_format"]["enum"],
            serde_json::json!(["base64", "hex"])
        );
        assert_eq!(props["input_format"]["default"], "base64");

        assert_eq!(
            props["dtype"]["enum"],
            serde_json::json!(["float32", "float64", "int16", "int32", "uint8", "auto"])
        );
        assert_eq!(props["dtype"]["default"], "float32");

        assert_eq!(
            props["shape"]["enum"],
            serde_json::json!(["auto", "frames_channels", "channels_frames", "flat"])
        );
        assert_eq!(props["shape"]["default"], "auto");

        assert_eq!(props["mono"]["type"], "boolean");
        assert_eq!(props["mono"]["default"], false);

        assert_eq!(props["fortran_order"]["type"], "boolean");
        assert_eq!(props["fortran_order"]["default"], false);

        assert_eq!(props["start_frame"]["type"], "integer");
        assert_eq!(props["start_frame"]["minimum"], 0);
        assert_eq!(props["start_frame"]["default"], 0);

        assert_eq!(props["max_frames"]["type"], "integer");
        assert_eq!(props["max_frames"]["minimum"], 0);
        assert_eq!(props["max_frames"]["maximum"], 1000000);
        assert_eq!(props["max_frames"]["default"], 0);

        assert_eq!(
            props["output"]["enum"],
            serde_json::json!(["base64", "hex", "info"])
        );
        assert_eq!(props["output"]["default"], "base64");

        for key in [
            "input",
            "input_format",
            "dtype",
            "shape",
            "mono",
            "fortran_order",
            "start_frame",
            "max_frames",
            "output",
        ] {
            assert!(
                props[key]["description"].as_str().unwrap().len() > 20,
                "missing description for {key}"
            );
        }
    }
}
