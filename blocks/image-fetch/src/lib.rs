//! gizza-ai/image-fetch — placeholder. Full impl in Task 6.

use wafer_sdk::*;

struct ImageFetch;

#[wafer_block(
    name = "gizza-ai/image-fetch",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fetch an image URL and return it for inline display",
    capabilities(callable_blocks = ["wafer-run/network"]),
)]
impl ImageFetch {
    fn handle(_msg: Message, _body: Vec<u8>) -> GuestResult {
        GuestResult::error(WaferError::new(
            ErrorCode::INTERNAL,
            "image-fetch not implemented yet",
        ))
    }
}
