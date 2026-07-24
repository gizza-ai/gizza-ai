# Toolchain bootstrap for building gizza tools

The `/new-tool`, `/improve-tool`, and `/create-next-tool` skills assume a working
toolchain (`cargo`, `wasm-pack`, `gizza`, `node`/Playwright, `ffmpeg`, optionally
`wafer`). CI installs these fresh on every run (see `.github/workflows/test.yml`
— this doc mirrors it). On a fresh dev box or sandbox, bootstrap once with the
steps below; `scripts/bootstrap-toolchain.sh` automates most of them.

## wafer-run pin

Each block crate, the `gizza` CLI, and `tools/generator` pull `wafer-sdk`/
`wafer-block`/`wafer-run` straight from git (`branch = "main"`). `cli/` and
`block-utils/` commit their lockfiles, and every new or changed block now
commits its own `Cargo.lock` alongside `target/block.wasm`. CI verifies that
all tracked locks resolve Wafer at `wafer-run-pin.txt` and that changed block
WASM byte-matches its locked canonical build. Bumping Wafer therefore requires
updating the pin, the root lockfiles, and any block artifacts changed in the
same PR.

## Steps

1. **System packages:** `git`, a C toolchain, node, ffmpeg.
   ```bash
   apt-get install -y build-essential pkg-config libssl-dev git nodejs npm ffmpeg
   ```
2. **Rust + wasm targets:**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
   . "$HOME/.cargo/env"
   rustup toolchain install 1.94.0 --profile minimal \
     --component clippy --component rustfmt \
     --target wasm32-unknown-unknown --target wasm32-wasip1
   ```
   The repository's `rust-toolchain.toml` selects this exact version.
   Blocks compile to **wasm32-wasip1** (the native/CLI artifact,
   `target/block.wasm`); each block's `web/` crate (browser bindings) uses
   **wasm32-unknown-unknown** via `wasm-pack`.
3. **wasm-pack** — pin to match CI (≥0.15; old 0.9 can't parse
   `license.workspace = true`): `cargo install wasm-pack`.
4. **`gizza` CLI:**
   ```bash
   cargo install --path cli --locked   # → gizza
   ```
5. **Optional: `wafer` CLI** — useful for Wafer-specific inspection and
   fixture commands. The committed CLI/MCP artifact must still be produced by
   `scripts/build-block-wasm.sh`, which is the same canonical path CI checks.
   ```bash
   git clone https://github.com/wafer-run/wafer-run ../wafer-run
   cargo install --path ../wafer-run/crates/wafer-cli --locked   # → wafer
   ```
6. **Playwright + chromium** (for the generated tool-page specs):
   ```bash
   (cd tests && npm install && npx playwright install --with-deps chromium)
   ```
7. **Baseline build:**
   ```bash
   # per block: native/CLI wasm
   for dir in blocks/*/; do
     block="$(basename "$dir")"
     scripts/build-block-wasm.sh "$block"
   done
   # per block: browser wasm (needed before rendering tool pages)
   for dir in blocks/*/page; do
     tool="$(basename "$(dirname "$dir")")"
     wasm-pack build "blocks/$tool/web" --target web --release --out-dir pkg
   done
   # generic tool pages (no site config — this repo has none)
   cargo run --manifest-path tools/generator/Cargo.toml -- .
   ```

## Gotchas learned

- The non-login shell used by tooling does **not** auto-source `~/.cargo/env`;
  prefix cargo/wafer/gizza commands with `. "$HOME/.cargo/env"` (or add it to
  the profile the runner sources).
- Each `blocks/<slug>/` and `tools/generator` is a **separate cargo workspace**
  — `cd` into it; never `-p <crate>` from the repo root.
- A first full build is slow (each block pulls + compiles the wafer-run git
  deps); the cargo cache makes later per-tool builds fast.
