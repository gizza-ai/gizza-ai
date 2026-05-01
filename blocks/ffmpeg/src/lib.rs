//! gizza-ai/ffmpeg — fetches a URL and returns ffmpeg's `-i` log.

use wafer_sdk::*;

struct FfmpegSkill;

#[wafer_block(
    name = "gizza-ai/ffmpeg",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Inspect a media file via ffmpeg",
    capabilities(callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
)]
impl FfmpegSkill {
    fn handle(_msg: Message, _body: Vec<u8>) -> GuestResult {
        GuestResult::error(WaferError::new(
            ErrorCode::UNIMPLEMENTED,
            "ffmpeg skill not yet implemented",
        ))
    }
}
