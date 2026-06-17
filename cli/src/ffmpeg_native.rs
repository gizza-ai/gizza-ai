//! ffmpeg via the system binary, implementing the shared FfmpegService trait.
use std::sync::Arc;

use gizza_ai_block_utils::ffmpeg::{ExecArgs, ExecResult, FfmpegError, FfmpegService};

pub struct NativeFfmpegService;

impl NativeFfmpegService {
    pub fn arc() -> Arc<dyn FfmpegService> {
        Arc::new(NativeFfmpegService)
    }
}

#[async_trait::async_trait]
impl FfmpegService for NativeFfmpegService {
    async fn exec(&self, args: ExecArgs) -> Result<ExecResult, FfmpegError> {
        let dir = tempfile::tempdir()
            .map_err(|e| FfmpegError::Bridge(format!("tempdir: {e}")))?;
        for (name, bytes) in &args.inputs {
            std::fs::write(dir.path().join(name), bytes)
                .map_err(|e| FfmpegError::Bridge(format!("write {name}: {e}")))?;
        }
        // ExecArgs.args is the ffmpeg argv WITHOUT the leading "ffmpeg" binary name
        // (the skill block builds e.g. ["-i","in.png","-vf","scale=...","out.png"]).
        let output = std::process::Command::new("ffmpeg")
            .args(&args.args)
            .current_dir(dir.path())
            .output();
        let output = match output {
            Ok(o) => o,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(FfmpegError::Bridge(
                    "`ffmpeg` not found on PATH — install ffmpeg to use this tool".into(),
                ))
            }
            Err(e) => return Err(FfmpegError::Bridge(format!("spawn ffmpeg: {e}"))),
        };
        let out_bytes = std::fs::read(dir.path().join(&args.output)).unwrap_or_default();
        Ok(ExecResult {
            exit_code: output.status.code().unwrap_or(-1),
            output: out_bytes,
            log: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}
