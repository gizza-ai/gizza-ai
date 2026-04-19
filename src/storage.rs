use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use wafer_core::interfaces::storage::service::{
    FolderInfo, ListOptions, ObjectInfo, ObjectList, StorageError, StorageService,
};

use crate::bridge;

pub struct BrowserStorageService;

// Safety: wasm32-unknown-unknown is single-threaded.
unsafe impl Send for BrowserStorageService {}
unsafe impl Sync for BrowserStorageService {}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Convert a JsValue returned by a bridge call to a String.
/// If the value is a JS exception/error, returns Err with the message.
fn jsvalue_to_string(val: wasm_bindgen::JsValue) -> Result<String, StorageError> {
    if val.is_null() || val.is_undefined() {
        return Ok(String::new());
    }
    match val.as_string() {
        Some(s) => Ok(s),
        None => {
            // Check if it's an Error object.
            let msg = js_sys::Reflect::get(&val, &wasm_bindgen::JsValue::from_str("message"))
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_else(|| format!("{:?}", val));
            Err(StorageError::Internal(msg))
        }
    }
}

/// Await a bridge future and convert its JsValue result to a String.
async fn await_bridge(
    future: impl std::future::Future<Output = wasm_bindgen::JsValue>,
) -> Result<String, StorageError> {
    jsvalue_to_string(future.await)
}

// ─── JSON shapes returned by the bridge ──────────────────────────────────────

#[derive(Deserialize)]
struct GetResponse {
    data: Vec<u8>,
    meta: GetMeta,
}

#[derive(Deserialize)]
struct GetMeta {
    content_type: String,
    size: i64,
}

// ─── StorageService impl ──────────────────────────────────────────────────────

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl StorageService for BrowserStorageService {
    async fn put(
        &self,
        folder: &str,
        key: &str,
        data: &[u8],
        content_type: &str,
    ) -> Result<(), StorageError> {
        await_bridge(bridge::storage_put(folder, key, data, content_type))
            .await
            .map(|_| ())
    }

    async fn get(&self, folder: &str, key: &str) -> Result<(Vec<u8>, ObjectInfo), StorageError> {
        let json = await_bridge(bridge::storage_get(folder, key)).await?;

        if json.is_empty() {
            return Err(StorageError::NotFound);
        }

        let resp: GetResponse = serde_json::from_str(&json)
            .map_err(|e| StorageError::Internal(format!("parse error: {e}")))?;

        let info = ObjectInfo {
            key: key.to_string(),
            size: resp.meta.size,
            content_type: resp.meta.content_type,
            last_modified: Utc::now(),
        };

        Ok((resp.data, info))
    }

    async fn delete(&self, folder: &str, key: &str) -> Result<(), StorageError> {
        await_bridge(bridge::storage_delete(folder, key))
            .await
            .map(|_| ())
    }

    async fn list(&self, folder: &str, opts: &ListOptions) -> Result<ObjectList, StorageError> {
        let limit = if opts.limit > 0 { opts.limit as u32 } else { 0 };
        let offset = opts.offset as u32;

        let json = await_bridge(bridge::storage_list(folder, &opts.prefix, limit, offset)).await?;

        // Bridge returns a JSON array of key strings.
        // ObjectInfo fields beyond `key` are not available from the list call;
        // we use placeholder values (size=0, empty content_type, current time).
        let keys: Vec<String> = serde_json::from_str(&json)
            .map_err(|e| StorageError::Internal(format!("parse error: {e}")))?;

        let now = Utc::now();
        let total_count = keys.len() as i64;
        let objects = keys
            .into_iter()
            .map(|k| ObjectInfo {
                key: k,
                size: 0,
                content_type: String::new(),
                last_modified: now,
            })
            .collect();

        Ok(ObjectList {
            objects,
            total_count,
        })
    }

    async fn create_folder(&self, name: &str, _public: bool) -> Result<(), StorageError> {
        // OPFS has no concept of "public" folders; the flag is ignored.
        await_bridge(bridge::storage_create_folder(name))
            .await
            .map(|_| ())
    }

    async fn delete_folder(&self, name: &str) -> Result<(), StorageError> {
        await_bridge(bridge::storage_delete_folder(name))
            .await
            .map(|_| ())
    }

    async fn list_folders(&self) -> Result<Vec<FolderInfo>, StorageError> {
        let json = await_bridge(bridge::storage_list_folders()).await?;

        // Bridge returns a JSON array of folder name strings.
        // FolderInfo fields beyond `name` are not available; use defaults.
        let names: Vec<String> = serde_json::from_str(&json)
            .map_err(|e| StorageError::Internal(format!("parse error: {e}")))?;

        let now = Utc::now();
        let folders = names
            .into_iter()
            .map(|n| FolderInfo {
                name: n,
                public: false,
                created_at: now,
            })
            .collect();

        Ok(folders)
    }
}
