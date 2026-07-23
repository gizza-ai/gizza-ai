//! gizza-ai/password-pwned-check — check a password against the Have I Been
//! Pwned (HIBP) Pwned Passwords corpus using the k-anonymity range API.
//!
//! Privacy model (k-anonymity): the password is SHA-1 hashed LOCALLY and only
//! the first 5 hex characters of the (uppercase) hash are ever transmitted, to
//! `GET https://api.pwnedpasswords.com/range/{prefix}`. The API returns every
//! breached suffix (the remaining 35 hex chars) sharing that prefix as
//! `SUFFIX:COUNT` lines; the match against our own suffix happens here, locally.
//! The raw password and the full 40-char hash never leave the device and are
//! never logged or returned.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). The handler stays thin (parse `Args`, run the check, emit
//! the flat `ToolResp` JSON) like `web-fetch`, rather than going through
//! `run_skill`, because the success shape is the flat `ToolResp`, not the
//! `{ "result": … }` wrapper `run_skill` produces.

// The #[wafer_block] macro emits wasm-only registration; supporting imports and
// the Args type are only used inside that impl.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use std::collections::HashMap;

use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use wafer_sdk::*;

/// HIBP Pwned Passwords range endpoint (k-anonymity). Only the 5-char prefix is
/// ever appended to this base — never the password or the full hash.
const RANGE_BASE: &str = "https://api.pwnedpasswords.com/range/";

#[derive(Deserialize)]
struct Args {
    password: String,
    #[serde(default)]
    padding: bool,
}

#[derive(Serialize)]
struct ToolResp {
    /// Whether the password was found in the breach corpus (count > 0).
    found: bool,
    /// How many times the password has been seen in breaches (0 if not found).
    count: u64,
    /// The 5 hex chars of the SHA-1 hash that were actually sent to the API.
    /// Echoed for transparency — this is the ONLY part of the hash transmitted.
    prefix: String,
    /// Whether response padding (Add-Padding header) was requested.
    padding: bool,
    /// Human-readable summary of the result.
    message: String,
}

/// Single-source param descriptor → chat schema (and CLI). `password` is a plain
/// required string (no `ref`), so there is no `url`⊕`ref` `oneOf`.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("password").required().describe(
                "The password to check. It is SHA-1 hashed locally; only the first 5 hex chars of \
                 that hash are sent to the API — never the password or the full hash.",
            ),
        )
        .param(
            Param::boolean("padding").describe(
                "If true, request padded responses (adds the 'Add-Padding: true' header) so an \
                 observer can't infer the queried hash prefix from the response size. Default: false.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// SHA-1 hash the password and split into the (prefix, suffix) the range API
/// uses: the first 5 hex chars (sent) and the remaining 35 hex chars (matched
/// locally). Both are UPPERCASE hex, matching what HIBP returns.
fn prefix_suffix(password: &str) -> (String, String) {
    let mut hasher = Sha1::new();
    hasher.update(password.as_bytes());
    let digest = hasher.finalize();
    let hex_upper = hex::encode_upper(digest); // 40 chars
    let (prefix, suffix) = hex_upper.split_at(5);
    (prefix.to_string(), suffix.to_string())
}

/// Scan a range-API response body for our suffix and return its breach count.
///
/// Each line is `SUFFIX:COUNT` (HIBP uses uppercase suffixes and CRLF line
/// endings). Matching is case-insensitive on the suffix. Padding rows (added by
/// the Add-Padding header) always carry `COUNT == 0`, so a count of 0 is treated
/// as "not found" — the real breach entries always have a positive count.
fn find_count(body: &str, suffix: &str) -> u64 {
    for line in body.lines() {
        let line = line.trim();
        let Some((cand, count)) = line.split_once(':') else {
            continue;
        };
        if cand.eq_ignore_ascii_case(suffix) {
            return count.trim().parse::<u64>().unwrap_or(0);
        }
    }
    0
}

/// Build the human-readable summary for a result.
fn summarize(found: bool, count: u64) -> String {
    if found {
        format!(
            "This password has appeared {count} time{} in known data breaches — do not use it.",
            if count == 1 { "" } else { "s" }
        )
    } else {
        "Good news — this password was not found in the Have I Been Pwned breach corpus.".to_string()
    }
}

#[cfg(target_arch = "wasm32")]
struct PasswordPwnedCheck;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/password-pwned-check",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Check whether a password appears in known breaches via the Have I Been Pwned k-anonymity range API.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Check whether a password appears in known data breaches via the Have I Been Pwned Pwned Passwords k-anonymity range API. The password is SHA-1 hashed locally and only the first 5 hex chars of the hash are sent; the suffix match happens locally, so the password and full hash never leave the device.",
        parameters = schema_json()
    ),
)]
impl PasswordPwnedCheck {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // Returns the flat ToolResp JSON directly (no `{ "result": … }` wrapper),
        // so it keeps a thin handle rather than using run_skill.
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("password-pwned-check")?;
    if args.password.is_empty() {
        return Err(SkillError::InvalidArgs(
            "password-pwned-check: 'password' must not be empty".to_string(),
        ));
    }

    let (prefix, suffix) = prefix_suffix(&args.password);
    let url = format!("{RANGE_BASE}{prefix}");

    // Only the 5-char prefix is in the URL. The password/full hash are never sent.
    let mut headers: HashMap<String, String> = HashMap::new();
    if args.padding {
        headers.insert("Add-Padding".to_string(), "true".to_string());
    }

    let resp = wafer_sdk::clients::network::do_request("GET", &url, &headers, None)?;
    if resp.status_code != 200 {
        // Only the 5-char prefix appears in `url`, so surfacing it is safe.
        return Err(SkillError::HttpStatus {
            status: resp.status_code,
            url,
        });
    }

    let text = String::from_utf8_lossy(&resp.body);
    let count = find_count(&text, &suffix);
    let found = count > 0;

    let tool = ToolResp {
        found,
        count,
        prefix,
        padding: args.padding,
        message: summarize(found, count),
    };
    serde_json::to_vec(&tool)
        .map_err(|e| SkillError::Serialize(format!("serialize tool response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// authored chat schema, so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "password": { "type": "string", "description": "The password to check. It is SHA-1 hashed locally; only the first 5 hex chars of that hash are sent to the API — never the password or the full hash." },
                    "padding":  { "type": "boolean", "description": "If true, request padded responses (adds the 'Add-Padding: true' header) so an observer can't infer the queried hash prefix from the response size. Default: false." }
                },
                "required": ["password"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// Known SHA-1 vector: `sha1("password")` =
    /// 5BAA61E4C9B93F3F0682250B6CF8331B7EE68FD8 (uppercase). Prefix = first 5,
    /// suffix = remaining 35.
    #[test]
    fn prefix_suffix_of_password() {
        let (prefix, suffix) = prefix_suffix("password");
        assert_eq!(prefix, "5BAA6");
        assert_eq!(suffix, "1E4C9B93F3F0682250B6CF8331B7EE68FD8");
        assert_eq!(prefix.len(), 5);
        assert_eq!(suffix.len(), 35);
        assert_eq!(format!("{prefix}{suffix}").len(), 40);
    }

    #[test]
    fn prefix_suffix_is_uppercase_hex() {
        let (prefix, suffix) = prefix_suffix("hunter2");
        assert!(prefix.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_lowercase()));
        assert!(suffix.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_lowercase()));
    }

    #[test]
    fn find_count_matches_suffix_and_reads_count() {
        // Suffix of sha1("password"), as HIBP would return it in a range response.
        let body = "003D68EB55068C33ACE09247EE4C639306B:3\r\n\
                    1E4C9B93F3F0682250B6CF8331B7EE68FD8:12345\r\n\
                    011053FD0102E94D6AE2F8B83D76FAF94F6:1";
        assert_eq!(find_count(body, "1E4C9B93F3F0682250B6CF8331B7EE68FD8"), 12345);
    }

    #[test]
    fn find_count_is_case_insensitive_on_suffix() {
        let body = "1e4c9b93f3f0682250b6cf8331b7ee68fd8:7";
        assert_eq!(find_count(body, "1E4C9B93F3F0682250B6CF8331B7EE68FD8"), 7);
    }

    #[test]
    fn find_count_returns_zero_when_absent() {
        let body = "003D68EB55068C33ACE09247EE4C639306B:3\r\n\
                    011053FD0102E94D6AE2F8B83D76FAF94F6:1";
        assert_eq!(find_count(body, "1E4C9B93F3F0682250B6CF8331B7EE68FD8"), 0);
    }

    #[test]
    fn find_count_ignores_padding_rows_with_zero_count() {
        // A padding row carries COUNT == 0 and must be treated as not-found.
        let body = "1E4C9B93F3F0682250B6CF8331B7EE68FD8:0\r\n\
                    011053FD0102E94D6AE2F8B83D76FAF94F6:1";
        assert_eq!(find_count(body, "1E4C9B93F3F0682250B6CF8331B7EE68FD8"), 0);
    }

    #[test]
    fn find_count_tolerates_blank_and_malformed_lines() {
        let body = "\r\nGARBAGE_NO_COLON\r\n1E4C9B93F3F0682250B6CF8331B7EE68FD8:42\r\n";
        assert_eq!(find_count(body, "1E4C9B93F3F0682250B6CF8331B7EE68FD8"), 42);
    }

    #[test]
    fn summarize_reflects_found_and_pluralizes() {
        assert!(summarize(true, 1).contains("1 time "));
        assert!(summarize(true, 5).contains("5 times"));
        assert!(summarize(false, 0).contains("not found"));
    }
}
