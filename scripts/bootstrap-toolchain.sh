#!/usr/bin/env bash
# Bootstrap the toolchain needed to build gizza tools (cargo, wafer, wasm-pack,
# solobase, gizza, Playwright, ffmpeg) on a fresh box. Mirrors
# .github/workflows/deploy.yml. Idempotent: each step is skipped if already done.
# See docs/TOOLCHAIN-SETUP.md for the rationale. Run from the repo root.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
parent="$(dirname "$root")"
cd "$root"

log() { printf '\n=== %s ===\n' "$*"; }

log "system packages"
if command -v apt-get >/dev/null 2>&1; then
  apt-get update -qq || true
  apt-get install -y build-essential pkg-config libssl-dev git nodejs npm ffmpeg xvfb >/dev/null
else
  echo "non-apt system; install build tools, node, ffmpeg, xvfb manually" >&2
fi

log "rust + wasm targets"
if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
# shellcheck disable=SC1090
. "$HOME/.cargo/env"
rustup target add wasm32-unknown-unknown wasm32-wasip1

log "wasm-pack"
command -v wasm-pack >/dev/null 2>&1 || cargo install wasm-pack

log "sibling checkouts"
[ -d "$parent/wafer-run" ] || git clone --depth 1 https://github.com/wafer-run/wafer-run "$parent/wafer-run"
if [ ! -d "$parent/solobase" ]; then
  git clone https://github.com/suppers-ai/solobase "$parent/solobase"
fi
git -C "$parent/solobase" checkout "$(tr -d '[:space:]' < solobase-pin.txt)" 2>/dev/null || true
# Best-effort pin (mirrors the solobase pin above): a local checkout on a
# work branch is left alone by the `|| true`, same as solobase.
wafer_sha="$(tr -d '[:space:]' < wafer-run-pin.txt)"
git -C "$parent/wafer-run" fetch --depth 1 origin "$wafer_sha" 2>/dev/null || true
git -C "$parent/wafer-run" checkout "$wafer_sha" 2>/dev/null || true

log "solobase-web wasm (needed by solobase build.rs)"
wasm-pack build "$parent/solobase/crates/solobase-web" --target web --release --out-dir pkg

log "CLIs: wafer, solobase, gizza"
command -v wafer    >/dev/null 2>&1 || cargo install --path "$parent/wafer-run/crates/wafer-cli" --locked
command -v solobase >/dev/null 2>&1 || cargo install --path "$parent/solobase/crates/solobase" --locked
cargo install --path cli --locked

log "Playwright + chromium"
( cd tests && npm install && npx playwright install --with-deps chromium )

log "baseline solobase build (all blocks + app)"
solobase build

log "baseline tool pages (per-block web wasm + rendered pages)"
# Mirrors deploy.yml "Build tool pages": solobase build does NOT build the page
# web/pkg/, and tools/generator hard-aborts on the first block whose web/pkg/ is
# missing — so build them all once up front.
for dir in blocks/*/page; do
  tool="$(basename "$(dirname "$dir")")"
  wasm-pack build "blocks/$tool/web" --target web --release --out-dir pkg
done
cargo run --manifest-path tools/generator/Cargo.toml -- .

log "done — toolchain ready"
wafer --version; solobase --version 2>/dev/null || true; gizza list | head -1
