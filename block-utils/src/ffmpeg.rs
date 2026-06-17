//! Shared ffmpeg bridge types and block implementation.
//!
//! `ExecArgs`, `ExecResult`, `FfmpegError`, `FfmpegService` (trait), and
//! `FfmpegBlock` all live here so that both the browser app (wasm32) and the
//! upcoming native CLI can register the identical block with their own
//! `FfmpegService` implementation.
//!
//! The browser-specific `BrowserFfmpegService` (which uses `#[wasm_bindgen]`
//! against `/js/ffmpeg.js`) stays in the `gizza-ai` app crate because the
//! module path is resolved relative to *that* crate's root.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wafer_block::{
    block::Block,
    context::Context,
    core_types::{ErrorCode, Message, WaferError},
    streams::{input::InputStream, output::OutputStream},
    types::BlockInfo,
};

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecArgs {
    pub args: Vec<String>,
    /// `(filename, bytes)` pairs — written to ffmpeg's virtual FS before exec.
    pub inputs: Vec<(String, Vec<u8>)>,
    /// Filename to read back after exec.
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResult {
    pub exit_code: i32,
    pub output: Vec<u8>,
    pub log: String,
}

#[derive(Debug, thiserror::Error)]
pub enum FfmpegError {
    #[error("bridge serialization: {0}")]
    Serialize(String),
    #[error("bridge call returned malformed response: {0}")]
    Bridge(String),
}

// ---------------------------------------------------------------------------
// FfmpegService trait
// ---------------------------------------------------------------------------

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait FfmpegService: wafer_block::MaybeSend + wafer_block::MaybeSync + 'static {
    async fn exec(&self, args: ExecArgs) -> Result<ExecResult, FfmpegError>;
}

// ---------------------------------------------------------------------------
// FfmpegBlock
// ---------------------------------------------------------------------------

/// Block implementation for `gizza-ai/ffmpeg-runtime`.
///
/// Dispatches `msg.kind = "ffmpeg.exec"` to a configurable `FfmpegService`
/// (e.g. `BrowserFfmpegService` in the browser app, or a native implementation
/// in the CLI).
pub struct FfmpegBlock {
    service: Arc<dyn FfmpegService>,
}

impl FfmpegBlock {
    pub fn new(service: Arc<dyn FfmpegService>) -> Self {
        Self { service }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl Block for FfmpegBlock {
    fn info(&self) -> BlockInfo {
        BlockInfo::new(
            "gizza-ai/ffmpeg-runtime",
            "0.1.0",
            "ffmpeg@v1",
            "Browser-side ffmpeg bridge: exec accepts CLI args + virtual-FS inputs",
        )
    }

    async fn handle(&self, _ctx: &dyn Context, msg: Message, input: InputStream) -> OutputStream {
        if msg.kind != "ffmpeg.exec" {
            return OutputStream::error(WaferError::new(
                ErrorCode::InvalidArgument,
                format!("FfmpegBlock: unknown msg.kind {}", msg.kind),
            ));
        }

        let body = input.collect_to_bytes().await;
        let args: ExecArgs = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => {
                return OutputStream::error(WaferError::new(
                    ErrorCode::InvalidArgument,
                    format!("FfmpegBlock: invalid request body: {e}"),
                ));
            }
        };

        match self.service.exec(args).await {
            Ok(result) => match serde_json::to_vec(&result) {
                Ok(bytes) => OutputStream::respond(bytes),
                Err(e) => OutputStream::error(WaferError::new(
                    ErrorCode::Internal,
                    format!("FfmpegBlock: serialize result: {e}"),
                )),
            },
            Err(e) => OutputStream::error(WaferError::new(
                ErrorCode::Unavailable,
                format!("ffmpeg exec failed: {e}"),
            )),
        }
    }
}
