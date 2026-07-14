# gizza-ai tests

Playwright end-to-end specs for the generated, static tool pages
(`pkg/tools/<slug>/`, the output of `tools/generator/`), plus the fixtures
they exercise.

## What's here

- `tool_pages.spec.ts` — core smoke test: renders/asserts the `calculator`
  and `clock` pages (title, meta description, JSON-LD, brand link, footer
  copy, and a live compute round-trip).
- `tool-page-<slug>.spec.ts` (one per block, ~390 files) — goes to
  `/tools/<slug>/`, fills the page's form, and asserts the computed output
  for that specific tool.
- `fixtures/` — binary sample inputs (images, audio, video, PDF, PGP
  signatures, etc.) that the per-tool specs upload into file-based tool
  pages.
- `fixtures.ts` — shared `test`/`expect` re-export used by every spec; wraps
  Playwright's `test` with a worker-scoped persistent Chromium context
  (backed by `tests/.cache/chromium-profile/`, gitignored) so browser-side
  caches survive across tests and runs. Import `test`/`expect` from here,
  not `@playwright/test` directly.
- `serve_pkg.py` — static file server for `../pkg` that sends
  `Cache-Control: no-store`. Plain `python3 -m http.server` doesn't set that
  header, so the persistent Chromium profile from `fixtures.ts` would
  otherwise disk-cache stale, unhashed `tool.js`/`tool-audio.js` across page
  regenerations. `playwright.config.ts`'s `webServer` runs this.
- `playwright.config.ts` — `testDir: '.'`; headless Chromium by default
  (`HEADED=1` env var to run headed); `webServer` auto-starts
  `python3 serve_pkg.py ../pkg 8001`; tests hit `baseURL: http://localhost:8001`.

## Running

Pages must be rendered before Playwright can hit them:

```bash
# from the repo root — renders every block's page/ into pkg/tools/<slug>/
cargo run --manifest-path tools/generator/Cargo.toml -- .
```

Then run the specs:

```bash
cd tests
npm install
npx playwright install chromium              # first time only

npx playwright test                          # everything
npx playwright test tool_pages.spec.ts       # just the core smoke test
npx playwright test tool-page-calculator.spec.ts   # a single tool's spec
```

The `webServer` in `playwright.config.ts` starts `serve_pkg.py` automatically
(reusing an already-running server if one is up on :8001), so there's no
separate serve step. The repo-root `justfile` wraps the common case:

```bash
just test    # == cd tests && npx playwright test
```

## CI: changed-tool testing

`.github/workflows/test.yml` scopes pull-request runs to whatever changed,
and only runs the full suite on pushes to `main`:

- It diffs the PR head against its base branch to find touched
  `blocks/<slug>/` directories, ignoring changes under `target/block.wasm`,
  `web/pkg/`, `page/`, and `manifest.json` (those don't affect the compiled
  wasm or its tests).
- For each changed block that has a `web/` crate, it builds that block's
  browser wasm (`wasm-pack build blocks/<slug>/web --target web --release
  --out-dir pkg`), then regenerates all pages (`cargo run --manifest-path
  tools/generator/Cargo.toml -- .`).
- It then runs only that block's `tool-page-<slug>.spec.ts` (if one exists —
  missing specs are logged, not treated as failures), not the whole suite.
- On pushes to `main`, it instead renders every page and runs the full
  `tool_pages.spec.ts` core smoke test.
