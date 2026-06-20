# Toolchain bootstrap for building gizza tools

The `/new-tool`, `/improve-tool`, and `/create-next-tool` skills assume a working
toolchain (`cargo`, `wafer`, `wasm-pack`, `solobase`, `gizza`, `node`/Playwright,
`ffmpeg`). CI installs these fresh on every run (see `.github/workflows/deploy.yml`
— this doc mirrors it). On a fresh dev box or sandbox, bootstrap once with the
steps below; `scripts/bootstrap-toolchain.sh` automates them.

## Expected sibling layout

The root `gizza-ai` crate has **path deps** on `../solobase`, and `solobase build`
needs both siblings checked out next to this repo:

```
workspace/
  gizza-ai/     <- this repo
  solobase/     <- suppers-ai/solobase @ the SHA in solobase-pin.txt
  wafer-run/    <- wafer-run/wafer-run @ main
```

Note: the per-block crates, the `gizza` CLI, and `tools/generator` pull `wafer-*`
straight from git (`branch = "main"`), so **they build without the siblings**.
Only the whole-app `solobase build` needs `../solobase` (and `solobase-web` wasm).

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
   git clone https://github.com/suppers-ai/solobase ../solobase
   git -C ../solobase checkout "$(tr -d '[:space:]' < solobase-pin.txt)"
   ```
5. **solobase-web wasm** (required by solobase's `build.rs`, which `include_bytes!`s it):
   `wasm-pack build ../solobase/crates/solobase-web --target web --release --out-dir pkg`
6. **CLIs:**
   ```bash
   cargo install --path ../wafer-run/crates/wafer-cli --locked   # → wafer
   cargo install --path ../solobase/crates/solobase --locked      # → solobase
   cargo install --path cli                                       # → gizza
   ```
7. **Playwright + chromium** (page tests run **headed** under xvfb — the config
   sets `headless: false` for WebGPU):
   ```bash
   (cd tests && npm install && npx playwright install --with-deps chromium)
   ```
8. **Baseline build** — two parts (the bootstrap script does both):
   - `solobase build` — builds every block's `target/block.wasm` + the app wasm.
     Without it `gizza list` shows only blocks you've built yourself.
   - **Tool pages** — `solobase build` does NOT build the page `web/pkg/`; the
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
  prefix cargo/wafer/solobase commands with `. "$HOME/.cargo/env"` (or add it to
  the profile the runner sources).
- Each `blocks/<slug>/` and `tools/generator` is a **separate cargo workspace** —
  `cd` into it; never `-p <crate>` from the repo root.
- 2 CPU / ~4 GB RAM is enough but `solobase build` + a parallel block build can
  contend; build the baseline first, then iterate per-tool.
- A first full build is slow (each block pulls + compiles the wafer git deps and
  any block-specific crates); the cargo cache makes later per-tool builds fast.
