# gzip-size-estimator — competitor analysis (2026-06-22)

Tool: report the raw and gzipped (and brotli) byte size of pasted code/text to
estimate over-the-wire transfer weight. Surfaces: chat skill, CLI, standalone page.

## Competitors surveyed

| # | Tool | Input | Outputs | Level control | Brotli |
| - | ---- | ----- | ------- | ------------- | ------ |
| 1 | GZip Size Online (dafrok.github.io/gzip-size-online) | paste **or file upload** | gzipped size | level 1–9 selector | no |
| 2 | gzip-size (sindresorhus, npm lib) | string/buffer (programmatic) | gzipped size (number) | level option | no |
| 3 | gzip-size-cli (sindresorhus) | file / stdin | gzipped size, optional raw size | `--level` | no |
| 4 | jsize (antonmedv) | npm package name | minified + gzipped size of a published package | fixed | no |
| 5 | Gzip & Brotli Compression Level Estimator (paulcalvano.com) | **a URL** (fetches the live asset) | per-level table: gzip 1–9 + **brotli 1–11** sizes, ratios, % improvement; detects active encoding | full per-level | **yes** |

## Feature diff vs. our tool

What we already match or beat:
- Paste input (textarea, multiline) — same as #1's paste mode.
- Raw size + gzipped size — all competitors.
- Selectable gzip level 0–9 (`level` param / field) — matches #1/#3.
- **Bytes saved, percent reduction, and compression ratio** in one report —
  richer than the size-only output of #1/#2/#3.
- Binary units (KB = 1024) matching browser dev tools.
- Honest negative-savings reporting for tiny inputs (gzip header/trailer
  overhead) — competitors silently show a larger "compressed" number.

Gaps identified and closed this pass:
- **Brotli comparison** — only #5 offered it, and only via a URL fetch. Added a
  `Brotli size:` line (pure-Rust `brotli` crate, quality 11, lgwin 22 — the
  `Content-Encoding: br` baseline) with its own % reduction. `brotli` instantiates
  cleanly in wafer (wasm32-wasip1); block grew from ~342 KiB to ~1.3 MiB, still
  valid. This is the single most useful differentiator over the gzip-only tools,
  since modern CDNs serve brotli and it usually beats gzip on text.

Out-of-model / intentionally not built (stay honest about scope):
- **File upload** (#1/#3): the page driver's `source="field"` model takes pasted
  text, not a binary upload, for a pure text tool. Paste covers the JS/CSS/JSON
  bundle use-case; a file-upload variant would need the `AssetKind` file-input
  path and isn't worth a second tool here.
- **URL fetch of a live asset** (#5): would require a network block; gizza's page
  runs locally with no fetch. The CLI's SSRF-guarded fetch is for media tools, not
  this pure text estimator.
- **npm-package lookup** (#4 jsize): needs a registry network call + a bundler to
  minify; out of a local pure-Rust text tool's scope.
- **Full per-level comparison table** (#5): a table of gzip 0–9 × sizes is
  feasible purely but adds clutter to a single-line-output page; the adjustable
  `level` field already lets a user check any specific level, and we show the one
  brotli baseline. Deferred as low-value vs. the brotli add.

No competitor copy, branding, or trademarks were used.

## Verification (all surfaces, 2026-06-22)

- `cargo test --workspace` in `blocks/gzip-size-estimator/` — core (5) + drift
  guard (1) pass.
- `wafer build` — chat `block.wasm` validates/instantiates with flate2 + brotli.
- `wasm-pack build … web` — page wasm built.
- CLI: `gizza tool gzip-size-estimator input=… [level=…]` — reports raw, gzipped,
  saved, reduction, ratio, brotli; verified default, level 9, and tiny-overhead paths.
- Playwright `tool-page-gzip-size-estimator.spec.ts` — 4/4 pass (report fields incl.
  brotli, level field, tiny-input overhead message, query-param deep-link prefill).
