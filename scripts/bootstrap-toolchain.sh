#!/usr/bin/env bash
# Bootstrap the toolchain needed to build gizza tools (cargo, wafer, wasm-pack,
# impresspress, gizza, Playwright, ffmpeg) on a fresh box. Mirrors
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

log "sibling checkouts (optional — a tools-only clone has neither; only used if present)"
if [ -d "$parent/wafer-run" ]; then
  # Best-effort pin: a local checkout already on a work branch is left alone
  # by the `|| true`, same as the impresspress pin below.
  wafer_sha="$(tr -d '[:space:]' < wafer-run-pin.txt)"
  git -C "$parent/wafer-run" fetch --depth 1 origin "$wafer_sha" 2>/dev/null || true
  git -C "$parent/wafer-run" checkout "$wafer_sha" 2>/dev/null || true
else
  echo "no ../wafer-run checkout — skipping pin sync. Blocks, cli/, and tools/generator" \
       "all pull wafer-sdk/wafer-block/wafer-run straight from git, pinned via" \
       "wafer-run-pin.txt in cli/Cargo.lock and block-utils/Cargo.lock — a tools-only" \
       "clone builds fine without this sibling; it's only needed to build the" \
       "optional 'wafer' CLI from source."
fi

if [ -d "$parent/impresspress" ]; then
  git -C "$parent/impresspress" checkout "$(tr -d '[:space:]' < impresspress-pin.txt)" 2>/dev/null || true

  log "impresspress-web wasm (needed by impresspress build.rs)"
  wasm-pack build "$parent/impresspress/crates/impresspress-web" --target web --release --out-dir pkg
else
  echo "no ../impresspress checkout — skipping. A tools-only clone never calls the" \
       "impresspress CLI: blocks build via plain cargo + the gizza CLI, and tool pages" \
       "render via tools/generator directly (see the baseline steps below)."
fi

log "CLIs: gizza (wafer/impresspress are optional, only installed if their sibling checkout exists)"
if [ -d "$parent/wafer-run" ]; then
  command -v wafer >/dev/null 2>&1 || cargo install --path "$parent/wafer-run/crates/wafer-cli" --locked
fi
if [ -d "$parent/impresspress" ]; then
  command -v impresspress >/dev/null 2>&1 || cargo install --path "$parent/impresspress/crates/impresspress" --locked
fi
cargo install --path cli --locked

log "Playwright + chromium"
( cd tests && npm install && npx playwright install --with-deps chromium )

if [ -d "$parent/impresspress" ]; then
  log "baseline impresspress build (all blocks + app)"
  impresspress build
else
  echo "no ../impresspress checkout — skipping 'impresspress build'. The per-block" \
       "wasm-pack loop + tools/generator render below (already unconditional) is the" \
       "full baseline a tools-only clone needs."
fi

log "baseline tool pages (per-block web wasm + rendered pages)"
# Mirrors deploy.yml "Build tool pages": impresspress build does NOT build the page
# web/pkg/, and tools/generator hard-aborts on the first block whose web/pkg/ is
# missing — so build them all once up front.
# If site config is present, emit branded partials (site checkout); otherwise render generic pages.
for dir in blocks/*/page; do
  tool="$(basename "$(dirname "$dir")")"
  wasm-pack build "blocks/$tool/web" --target web --release --out-dir pkg
done
if [ -f site/site-config.toml ]; then
  # Branded render — mirrors deploy.yml "Build tool pages" (site checkout).
  cargo run --manifest-path chrome/Cargo.toml --bin emit_partials -- site/partials
  cargo run --manifest-path tools/generator/Cargo.toml -- . --site-config site/site-config.toml
else
  # Generic render — public toolkit checkout has no site config.
  cargo run --manifest-path tools/generator/Cargo.toml -- .
fi

log "done — toolchain ready"
wafer --version 2>/dev/null || true; impresspress --version 2>/dev/null || true; gizza list | head -1
