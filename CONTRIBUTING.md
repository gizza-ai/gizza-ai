# Contributing

Thanks for considering a contribution to gizza's tool library. This repo is
the public, MIT-licensed half of gizza.ai: tool implementations (`blocks/`),
the `gizza` CLI (`cli/`), and the static tool-page generator (`tools/generator`).
All contributions are accepted under the [MIT license](LICENSE).

## Adding a new tool

1. **Scaffold it.** `scripts/scaffold-tool.sh <slug> <pure|ffmpeg>` generates the
   boilerplate under `blocks/<slug>/` (a `core/` crate for pure logic, `web/`
   for browser bindings, `page/` for the SEO copy + form metadata, plus
   `manifest.json` and `wafer.toml`). Use `pure` for compute-only tools
   (calculators, converters, text/data utilities) and `ffmpeg` for
   image/video tools that shell out to ffmpeg.

2. **Implement it.** Fill in the tool's logic in `blocks/<slug>/core/src`,
   wire up its `wafer_block!` macro in `blocks/<slug>/src/lib.rs`, and write
   the SEO copy + input metadata in `blocks/<slug>/page/`.

3. **Keep pages generic.** Tool page sources (`page/meta.toml`,
   `page/content.md`, `custom.js`, `custom.css`) must never hardcode
   site-specific branding (e.g. a literal `gizza.ai` string) — the page
   renderer (`tools/generator`) injects branding at render time via an
   optional site config that does not exist in this repo. This is enforced
   mechanically (`scripts/check-tool-hygiene.py`, check 8) — a leaked brand
   string fails the gate.

4. **Run the hygiene gate.** `scripts/check-tool-hygiene.py <slug>` runs the
   full strict check set for your tool (enum/manifest drift, FAQ formatting,
   leftover scaffold placeholders, summary drift, input placeholders, FAQ
   depth, meta-description length, and the branding check above). Fix
   everything it flags before opening a PR.

5. **Test it.**
   ```bash
   cargo test --manifest-path blocks/<slug>/core/Cargo.toml   # if present
   cargo test --manifest-path blocks/<slug>/Cargo.toml
   cargo build --target wasm32-wasip1 --release --manifest-path blocks/<slug>/Cargo.toml
   ```
   Add a CLI smoke check (`gizza tool <slug> …`) and, if the tool renders a
   page, a Playwright spec under `tests/tool-page-<slug>.spec.ts`.

6. **Open a PR.** CI (`.github/workflows/test.yml`) runs the hygiene gate,
   the changed block's cargo tests, the CLI/generator/block-utils test
   suites, and — for pages — a Playwright check of the changed tool page.

## Improving an existing tool

Same gate applies: run `scripts/check-tool-hygiene.py <slug>` and the block's
tests after any change to `blocks/<slug>/`.

## Code style

- Rust: `cargo fmt` (see `rustfmt.toml`); no raw SQL, no hardcoded
  domain-specific values — use `ConfigVar`/ manifest-declared config where a
  tool needs configuration.
- Fix at the root cause — no compatibility shims or quick fixes; if the right
  fix touches several files, touch them all in the same PR.
