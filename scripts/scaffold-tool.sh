#!/usr/bin/env bash
# Scaffold the name-substituted BOILERPLATE for a new gizza tool. The skill
# (.claude/skills/new-tool/SKILL.md) fills the tool-specific logic afterward.
# Usage: scripts/scaffold-tool.sh <slug> <pure|ffmpeg>
set -euo pipefail

slug="${1:?usage: scaffold-tool.sh <slug> <pure|ffmpeg>}"
type="${2:?usage: scaffold-tool.sh <slug> <pure|ffmpeg>}"
[[ "$slug" =~ ^[a-z][a-z0-9-]*$ ]] || { echo "slug must be kebab-case [a-z0-9-]" >&2; exit 2; }
[[ "$type" == pure || "$type" == ffmpeg ]] || { echo "type must be pure|ffmpeg" >&2; exit 2; }
root="$(cd "$(dirname "$0")/.." && pwd)"
dir="$root/blocks/$slug"
[[ -e "$dir" ]] && { echo "blocks/$slug already exists — refusing to overwrite" >&2; exit 2; }

under="${slug//-/_}"
crate="gizza-ai-$slug"
ucrate="gizza_ai_${under}"

mkdir -p "$dir/core/src" "$dir/web/src" "$dir/src" "$dir/page" "$dir/tests"

cat > "$dir/Cargo.toml" <<EOF
[workspace]
resolver = "2"
members = [".", "core", "web"]

[package]
name = "${crate}-block"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
wafer-sdk = { git = "https://github.com/wafer-run/wafer-run.git", branch = "main" }
wafer-block = { git = "https://github.com/wafer-run/wafer-run.git", branch = "main" }
${crate}-core = { path = "core" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
EOF

cat > "$dir/core/Cargo.toml" <<EOF
[package]
name = "${crate}-core"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"

[dependencies]
EOF
cat > "$dir/core/src/lib.rs" <<'EOF'
//! __SLUG__ core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps. The new-tool skill replaces `run` with real logic.

/// Replace with the tool's real signature + logic (see SKILL.md).
pub fn run(input: &str) -> Result<String, String> {
    let _ = input;
    Err("not implemented".into())
}
EOF

cat > "$dir/web/Cargo.toml" <<EOF
[package]
name = "${crate}-web"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"

[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-bindgen = "0.2"
${crate}-core = { path = "../core" }
EOF
cat > "$dir/web/src/lib.rs" <<EOF
//! Browser-facing wasm-bindgen wrapper for /tools/$slug/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str) -> Result<String, JsValue> {
    ${ucrate}_core::run(input).map_err(|e| JsValue::from_str(&e))
}
EOF

cat > "$dir/src/lib.rs" <<EOF
//! gizza-ai/$slug — chat skill block (thin wrapper around core). The new-tool
//! skill replaces the skill(description, parameters) schema + Args + delegation.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args { input: String }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/$slug",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "$slug skill",
    skill(
        description = "TODO: describe what this tool does and its inputs.",
        parameters = r#"{ "type": "object", "properties": { "input": { "type": "string" } }, "required": ["input"], "additionalProperties": false }"#
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => return respond_error(format!("invalid args: {e}")),
        };
        match ${ucrate}_core::run(&args.input) {
            Ok(v) => GuestResult::respond(serde_json::to_vec(&serde_json::json!({ "result": v })).unwrap_or_default()),
            Err(e) => respond_error(e),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn respond_error(msg: String) -> GuestResult {
    GuestResult::respond(serde_json::to_vec(&serde_json::json!({ "error": msg })).unwrap_or_default())
}
EOF

cat > "$dir/wafer.toml" <<EOF
[package]
org = "gizza-ai"
name = "$slug"
version = "0.1.0"
abi = 1
summary = "TODO: one-line summary."
EOF

cat > "$dir/page/meta.toml" <<EOF
slug          = "$slug"
title         = "TODO — gizza.ai"
description   = "TODO."
tags          = []
h1            = "TODO"
hero_subtitle = "TODO."
wasm          = "${ucrate}_web"
export        = "run"
live          = false
output_label  = "Result"
format        = "text"

[[input]]
name        = "input"
label       = "Input"
placeholder = ""
source      = "field"
EOF
cat > "$dir/page/content.md" <<EOF
## About this tool

TODO: SEO copy.
EOF

if [[ "$type" == ffmpeg ]]; then
  cat > "$dir/web/src/lib.rs" <<EOF
//! Browser-facing wasm-bindgen wrapper for /tools/$slug/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core).
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct Plan { argv: Vec<String>, out_name: String }

#[wasm_bindgen]
pub fn build_argv(in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) = ${ucrate}_core::plan(in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&Plan { argv, out_name }).map_err(|e| JsValue::from_str(&e.to_string()))
}
EOF
  cat > "$dir/web/Cargo.toml" <<EOF
[package]
name = "${crate}-web"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"

[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-bindgen = "0.2"
serde = { version = "1", features = ["derive"] }
serde-wasm-bindgen = "0.6"
${crate}-core = { path = "../core" }
EOF
  cat > "$dir/core/src/lib.rs" <<'EOF'
//! __SLUG__ core — pure ffmpeg argv construction shared by the chat block + page.
/// Replace with the tool's real params + ffmpeg argv (see SKILL.md, image-resize core).
pub fn plan(in_name: &str) -> Result<(Vec<String>, String), String> {
    let _ = in_name;
    Err("not implemented".into())
}
EOF
  cat > "$dir/page/meta.toml" <<EOF
slug          = "$slug"
title         = "TODO — gizza.ai"
description   = "TODO."
tags          = []
h1            = "TODO"
hero_subtitle = "TODO."
wasm          = "${ucrate}_web"
export        = "build_argv"
runtime       = "ffmpeg"
output_label  = "Result"
format        = "image"

[[input]]
name   = "file"
source = "file"
accept = "image/*"
label  = "File"
EOF
fi

echo "scaffolded blocks/$slug ($type). Next: implement core/src/lib.rs, src/lib.rs (skill schema), web/src/lib.rs, page/meta.toml, page/content.md."
