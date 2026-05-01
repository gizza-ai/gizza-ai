//! gizza-ai/web-fetch — fetches a URL via wafer-run/network.

use wafer_sdk::*;

struct WebFetch;

#[wafer_block(
    name = "gizza-ai/web-fetch",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fetch a URL and return its body",
    capabilities(callable_blocks = ["wafer-run/network"]),
)]
impl WebFetch {
    fn handle(_msg: Message, _body: Vec<u8>) -> GuestResult {
        GuestResult::error(WaferError::new(
            ErrorCode::UNIMPLEMENTED,
            "web-fetch not yet implemented",
        ))
    }
}
