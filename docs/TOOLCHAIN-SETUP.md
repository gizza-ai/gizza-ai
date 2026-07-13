# Toolchain bootstrap for building gizza tools

The `/new-tool`, `/improve-tool`, and `/create-next-tool` skills assume a working
toolchain (`cargo`, `wafer`, `wasm-pack`, `impresspress`, `gizza`, `node`/Playwright,
`ffmpeg`). CI installs these fresh on every run (see `.github/workflows/deploy.yml`
— this doc mirrors it). On a fresh dev box or sandbox, bootstrap once with the
steps below; `scripts/bootstrap-toolchain.sh` automates them.

## Expected sibling layout

The root `gizza-ai` crate has **path deps** on `../impresspress`, and `impresspress build`
needs both siblings checked out next to this repo:

```
workspace/
  gizza-ai/     <- this repo
  impresspress/     <- impresspress/impresspress @ the SHA in impresspress-pin.txt
  wafer-run/    <- wafer-run/wafer-run @ the SHA in wafer-run-pin.txt
```

Note: the per-block crates, the `gizza` CLI, and `tools/generator` pull `wafer-*`
straight from git (`branch = "main"`), so **they build without the siblings**.
Only the whole-app `impresspress build` needs `../impresspress` (and `impresspress-web` wasm).

The root, `cli/`, and `block-utils/` crates commit their `Cargo.lock`, which pins
the wafer-run git deps to the SHA in `wafer-run-pin.txt` (CI's "Verify wafer-run
pin consistency" step enforces the two agree, and that impresspress's own Cargo.lock
pins the same SHA). Bumping wafer-run = edit `wafer-run-pin.txt` + `cargo update`
in those three roots (+ bump `impresspress-pin.txt` if impresspress moved with it). The
per-block crates keep floating on `branch = "main"` deliberately: deploys never
compile them (committed `blocks/*/target/block.wasm` + `IMPRESSPRESS_SKIP_BLOCK_BUILD`),
and pinning them would mean a ~250-manifest sed on every bump.

## Steps (verified 2026-06-20)

1. **System packages:** `git`, a C toolchain, node, ffmpeg, xvfb.
   ```bash
   apt-get install -y build-essential pkg-config libssl-dev git \
                      nodejs npm ffmpeg xvfb
   ```
2. **Rust + wasm targets:**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
   . "$HOME/.cargo/env"
   rustup target add wasm32-unknown-unknown wasm32-wasip1
   ```
   `wafer build` compiles blocks to **wasm32-wasip1**; the web crates + app use
   **wasm32-unknown-unknown**.
3. **wasm-pack** — pin to match CI (≥0.15; old 0.9 can't parse
   `license.workspace = true`): `cargo install wasm-pack`.
4. **Sibling checkouts:**
   ```bash
   git clone https://github.com/wafer-run/wafer-run ../wafer-run
   git clone https://github.com/impresspress/impresspress ../impresspress
   git -C ../impresspress checkout "$(tr -d '[:space:]' < impresspress-pin.txt)"
   git -C ../wafer-run checkout "$(tr -d '[:space:]' < wafer-run-pin.txt)"
   ```
5. **impresspress-web wasm** (required by impresspress's `build.rs`, which `include_bytes!`s it):
   `wasm-pack build ../impresspress/crates/impresspress-web --target web --release --out-dir pkg`
6. **CLIs:**
   ```bash
   cargo install --path ../wafer-run/crates/wafer-cli --locked   # → wafer
   cargo install --path ../impresspress/crates/impresspress --locked      # → impresspress
   cargo install --path cli --locked                              # → gizza
   ```
7. **Playwright + chromium** (page tests run **headed** under xvfb — the config
   sets `headless: false` for WebGPU):
   ```bash
   (cd tests && npm install && npx playwright install --with-deps chromium)
   ```
8. **Baseline build** — two parts (the bootstrap script does both):
   - `impresspress build` — builds every block's `target/block.wasm` + the app wasm.
     Without it `gizza list` shows only blocks you've built yourself.
   - **Tool pages** — `impresspress build` does NOT build the page `web/pkg/`; the
     deploy workflow does it separately. `tools/generator` hard-aborts on the
     first block whose `web/pkg/` is missing, so build them all once:
     ```bash
     for dir in blocks/*/page; do
       tool="$(basename "$(dirname "$dir")")"
       wasm-pack build "blocks/$tool/web" --target web --release --out-dir pkg
     done
     cargo run --manifest-path tools/generator/Cargo.toml -- .
     ```

## Gotchas learned

- The non-login shell used by tooling does **not** auto-source `~/.cargo/env`;
  prefix cargo/wafer/impresspress commands with `. "$HOME/.cargo/env"` (or add it to
  the profile the runner sources).
- Each `blocks/<slug>/` and `tools/generator` is a **separate cargo workspace** —
  `cd` into it; never `-p <crate>` from the repo root.
- 2 CPU / ~4 GB RAM is enough but `impresspress build` + a parallel block build can
  contend; build the baseline first, then iterate per-tool.
- A first full build is slow (each block pulls + compiles the wafer git deps and
  any block-specific crates); the cargo cache makes later per-tool builds fast.
