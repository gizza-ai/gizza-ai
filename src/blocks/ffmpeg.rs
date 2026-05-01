//! gizza-ai/ffmpeg-runtime native block.
//!
//! Dispatches `msg.kind = "ffmpeg.exec"` to a configurable
//! `FfmpegService` (e.g. `BrowserFfmpegService` in production,
//! `FakeFfmpegService` in tests).

use std::sync::Arc;

use wafer_block::{
    block::Block,
    context::Context,
    core_types::{ErrorCode, Message, WaferError},
    streams::{input::InputStream, output::OutputStream},
    types::BlockInfo,
};

use crate::ffmpeg::{ExecArgs, FfmpegService};

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

    async fn handle(
        &self,
        _ctx: &dyn Context,
        msg: Message,
        input: InputStream,
    ) -> OutputStream {
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
