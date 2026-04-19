use js_sys::{ArrayBuffer, Uint8Array};
use wafer_block::{
    meta::{
        META_REQ_ACTION, META_REQ_CLIENT_IP, META_REQ_CONTENT_TYPE, META_REQ_QUERY_PREFIX,
        META_REQ_RESOURCE, META_RESP_CONTENT_TYPE, META_RESP_COOKIE_PREFIX,
        META_RESP_HEADER_PREFIX, META_RESP_STATUS,
    },
    streams::{
        input::InputStream,
        output::{OutputStream, TerminalNotResponse},
    },
    ErrorCode, Message, MetaAccess,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, ResponseInit};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn http_method_to_action(method: &str) -> &'static str {
    match method.to_uppercase().as_str() {
        "GET" | "HEAD" => "retrieve",
        "POST" => "create",
        "PUT" | "PATCH" => "update",
        "DELETE" => "delete",
        _ => "execute",
    }
}

fn urlencoding_decode(s: &str) -> String {
    let mut bytes = Vec::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'+' {
            bytes.push(b' ');
        } else if b == b'%' {
            let h1 = chars.next().and_then(|c| (c as char).to_digit(16));
            let h2 = chars.next().and_then(|c| (c as char).to_digit(16));
            if let (Some(h1), Some(h2)) = (h1, h2) {
                bytes.push((h1 * 16 + h2) as u8);
            }
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|_| s.to_string())
}

fn error_code_to_http_status(code: &ErrorCode) -> u16 {
    match code {
        ErrorCode::Ok => 200,
        ErrorCode::Cancelled => 499,
        ErrorCode::InvalidArgument => 400,
        ErrorCode::DeadlineExceeded => 504,
        ErrorCode::NotFound => 404,
        ErrorCode::AlreadyExists => 409,
        ErrorCode::PermissionDenied => 403,
        ErrorCode::ResourceExhausted => 429,
        ErrorCode::FailedPrecondition => 412,
        ErrorCode::Aborted => 409,
        ErrorCode::OutOfRange => 400,
        ErrorCode::Unimplemented => 501,
        ErrorCode::Internal => 500,
        ErrorCode::Unavailable => 503,
        ErrorCode::DataLoss => 500,
        ErrorCode::Unauthenticated => 401,
        _ => 500,
    }
}

// ---------------------------------------------------------------------------
// Request conversion
// ---------------------------------------------------------------------------

/// Convert a browser `web_sys::Request` into a WAFER `(Message, InputStream)` pair.
///
/// Mirrors `http_to_message()` from `wafer-block-http-listener`.  The remote
/// address is always `"127.0.0.1"` — in a Service Worker the request comes
/// from the same device.
pub async fn request_to_message(
    request: &web_sys::Request,
) -> Result<(Message, InputStream), JsValue> {
    let method = request.method();
    let url_str = request.url();

    // Parse the URL so we can separate path and query string.
    let url = web_sys::Url::new(&url_str)?;
    let path = url.pathname();
    // search() includes the leading '?' — strip it.
    let search = url.search();
    let raw_query = if search.starts_with('?') {
        search[1..].to_string()
    } else {
        search
    };

    // Read body bytes via ArrayBuffer.
    const MAX_BODY_SIZE: usize = 10 * 1024 * 1024; // 10MB
    let body: Vec<u8> = {
        let promise = request.array_buffer()?;
        let ab_val = JsFuture::from(promise).await?;
        let ab: ArrayBuffer = ab_val.dyn_into()?;
        let arr = Uint8Array::new(&ab);
        if arr.length() as usize > MAX_BODY_SIZE {
            return Err(JsValue::from_str("Request body too large"));
        }
        arr.to_vec()
    };

    let mut msg = Message::new(format!("{}:{}", method, path));

    // HTTP-specific meta (mirrors the native listener).
    msg.set_meta("http.method", method.clone());
    msg.set_meta("http.path", path.clone());
    msg.set_meta("http.raw_query", raw_query.clone());
    msg.set_meta("http.remote_addr", "127.0.0.1");

    // Collect headers into meta.
    let headers: Headers = request.headers();
    // Iterate the headers JS iterator.
    let iter =
        js_sys::try_iter(&headers)?.ok_or_else(|| JsValue::from_str("headers not iterable"))?;
    let mut content_type = String::new();
    let mut host = String::new();
    for item in iter {
        let item = item?;
        // Each entry is a JS Array [name, value].
        let arr: js_sys::Array = item.dyn_into()?;
        let key = arr.get(0).as_string().unwrap_or_default().to_lowercase();
        let val = arr.get(1).as_string().unwrap_or_default();
        if key == "content-type" {
            content_type = val.clone();
        }
        if key == "host" {
            host = val.clone();
        }
        msg.set_meta(format!("http.header.{}", key), val);
    }

    msg.set_meta("http.content_type", content_type.clone());
    msg.set_meta("http.host", host);

    // Normalised request meta.
    msg.set_meta(META_REQ_ACTION, http_method_to_action(&method));
    msg.set_meta(META_REQ_RESOURCE, path.clone());
    msg.set_meta(META_REQ_CLIENT_IP, "127.0.0.1");
    msg.set_meta(META_REQ_CONTENT_TYPE, content_type);

    // Decode query parameters into both `http.query.*` and `req.query.*`.
    if !raw_query.is_empty() {
        for pair in raw_query.split('&') {
            let mut parts = pair.splitn(2, '=');
            if let (Some(key), Some(val)) = (parts.next(), parts.next()) {
                let decoded_val = urlencoding_decode(val);
                msg.set_meta(format!("http.query.{}", key), decoded_val.clone());
                msg.set_meta(format!("{}{}", META_REQ_QUERY_PREFIX, key), decoded_val);
            }
        }
    }

    Ok((msg, InputStream::from_bytes(body)))
}

// ---------------------------------------------------------------------------
// Response conversion
// ---------------------------------------------------------------------------

/// Apply response meta entries to a `web_sys::Headers` object.
///
/// Mirrors `apply_response_meta()` from the native listener.
fn apply_response_meta(headers: &Headers, meta: &[wafer_block::MetaEntry]) -> Result<(), JsValue> {
    for entry in meta {
        let k = entry.key.as_str();
        let v = &entry.value;
        match k {
            k if k == META_RESP_STATUS || k == "http.status" => {
                // Status is handled separately; skip.
            }
            k if k.starts_with(META_RESP_COOKIE_PREFIX)
                || k.starts_with("http.resp.set-cookie.") =>
            {
                headers.append("Set-Cookie", v)?;
            }
            k if k.starts_with(META_RESP_HEADER_PREFIX) => {
                let header_name = &k[META_RESP_HEADER_PREFIX.len()..];
                headers.set(header_name, v)?;
            }
            k if k.starts_with("http.resp.header.") => {
                let header_name = &k[17..];
                headers.set(header_name, v)?;
            }
            k if k == META_RESP_CONTENT_TYPE || k == "Content-Type" => {
                headers.set("Content-Type", v)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn get_status_code(meta: &[wafer_block::MetaEntry], default_code: u16) -> u16 {
    if let Some(code) = MetaAccess::get(meta, META_RESP_STATUS) {
        if let Ok(n) = code.parse::<u16>() {
            return n;
        }
    }
    if let Some(code) = MetaAccess::get(meta, "http.status") {
        if let Ok(n) = code.parse::<u16>() {
            return n;
        }
    }
    default_code
}

fn get_error_status_code(
    error: Option<&wafer_block::WaferError>,
    meta: &[wafer_block::MetaEntry],
) -> u16 {
    let from_meta = get_status_code(meta, 0);
    if from_meta > 0 {
        return from_meta;
    }
    if let Some(err) = error {
        return error_code_to_http_status(&err.code);
    }
    500
}

/// Build a `web_sys::Response` from raw bytes, a status code, and a
/// `web_sys::Headers` object.
fn make_response(
    body: Vec<u8>,
    status: u16,
    headers: Headers,
) -> Result<web_sys::Response, JsValue> {
    let init = ResponseInit::new();
    init.set_status(status);
    init.set_headers(&headers);

    if body.is_empty() {
        web_sys::Response::new_with_opt_str_and_init(None, &init)
    } else {
        // Copy into a Uint8Array then pass as BufferSource.
        let arr = Uint8Array::new_with_length(body.len() as u32);
        arr.copy_from(&body);
        let ab: ArrayBuffer = arr.buffer();
        web_sys::Response::new_with_opt_buffer_source_and_init(Some(&ab.into()), &init)
    }
}

/// Convert a WAFER `OutputStream` into a browser `web_sys::Response` by
/// buffering the entire stream.
///
/// Mirrors `wafer_output_to_response()` from `wafer-block-http-listener`.
pub async fn output_to_response(output: OutputStream) -> Result<web_sys::Response, JsValue> {
    match output.collect_buffered().await {
        Ok(buf) => {
            let status = get_status_code(&buf.meta, 200);
            let headers = Headers::new()?;
            apply_response_meta(&headers, &buf.meta)?;

            let has_ct = MetaAccess::contains_key(&buf.meta, META_RESP_CONTENT_TYPE)
                || MetaAccess::contains_key(&buf.meta, "Content-Type");
            if !has_ct {
                headers.set("Content-Type", "application/json")?;
            }

            make_response(buf.body, status, headers)
        }

        Err(TerminalNotResponse::Error(err)) => {
            let status = get_error_status_code(Some(&err), &err.meta);
            let headers = Headers::new()?;
            apply_response_meta(&headers, &err.meta)?;
            headers.set("Content-Type", "application/json")?;

            let body = serde_json::json!({
                "error": err.code,
                "message": err.message,
            })
            .to_string()
            .into_bytes();

            make_response(body, status, headers)
        }

        Err(TerminalNotResponse::Drop) => {
            let headers = Headers::new()?;
            make_response(Vec::new(), 204, headers)
        }

        Err(TerminalNotResponse::Continue(_msg)) => {
            let headers = Headers::new()?;
            headers.set("Content-Type", "application/json")?;
            make_response(
                b"{\"error\":\"continue not supported by listener\"}".to_vec(),
                500,
                headers,
            )
        }

        Err(TerminalNotResponse::Malformed) => {
            let headers = Headers::new()?;
            headers.set("Content-Type", "application/json")?;
            make_response(
                b"{\"error\":\"stream ended without terminal event\"}".to_vec(),
                500,
                headers,
            )
        }
    }
}
