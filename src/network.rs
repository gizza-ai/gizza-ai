use std::collections::HashMap;

use serde::Deserialize;
use wafer_core::interfaces::network::service::{NetworkError, NetworkService, Request, Response};

use crate::bridge;

pub struct BrowserNetworkService;

unsafe impl Send for BrowserNetworkService {}
unsafe impl Sync for BrowserNetworkService {}

/// JSON shape returned by bridge.httpFetch:
/// `{ status: number, headers: { [key: string]: string }, body: number[] }`
#[derive(Deserialize)]
struct FetchResponse {
    status: u16,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    body: Vec<u8>,
}

#[async_trait::async_trait(?Send)]
impl NetworkService for BrowserNetworkService {
    async fn do_request(&self, req: &Request) -> Result<Response, NetworkError> {
        let headers_json = serde_json::to_string(&req.headers)
            .map_err(|e| NetworkError::Other(format!("failed to serialize headers: {e}")))?;

        let body_bytes: &[u8] = req.body.as_deref().unwrap_or(&[]);

        let js_val = bridge::http_fetch(&req.method, &req.url, &headers_json, body_bytes).await;

        // The bridge returns a JS object; stringify it so we can deserialize.
        let json_str = js_sys::JSON::stringify(&js_val)
            .map(|s| s.as_string().unwrap_or_default())
            .unwrap_or_default();

        let fetch_resp: FetchResponse = serde_json::from_str(&json_str).map_err(|e| {
            NetworkError::RequestError(format!("failed to parse fetch response: {e}"))
        })?;

        // Collapse single-value headers into Vec<String> as NetworkService::Response expects.
        let headers: HashMap<String, Vec<String>> = fetch_resp
            .headers
            .into_iter()
            .map(|(k, v)| (k, vec![v]))
            .collect();

        Ok(Response {
            status_code: fetch_resp.status,
            headers,
            body: fetch_resp.body,
        })
    }
}
