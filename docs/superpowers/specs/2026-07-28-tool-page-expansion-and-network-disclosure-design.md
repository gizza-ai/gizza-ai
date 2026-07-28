# Tool-page expansion + network disclosure — design

Date: 2026-07-28
Status: approved, not yet implemented

## Problem

Of 812 blocks, 595 have a standalone page under `/tools/<slug>/` and 217 do not.
The page-less blocks are reachable from chat and the `gizza` CLI but have no
indexable surface, so roughly a quarter of the catalogue is invisible to search.

Two separate causes were confirmed by audit:

1. **Input model, not capability.** 144 blocks declare `wafer-run/network` and
   are treated as chat-only, but they never call `network::do_request` — they use
   the network solely to fetch a `url`/`ref` **input**. On a page that input
   arrives as an upload, so those tools need no network at all. They are blocked
   on a page upload path, not on networking.
2. **Output shape.** Tools returning SVG, PNG, ZIP or PDF have no page because
   the pure page path renders results as text (`out.textContent = String(value)`).
   `format = "svg"` already exists in `meta.toml` but is only a label: `tool.js`
   branches on `"number"` alone, so `qr-paper-backup` prints raw markup today.

Only **5 blocks in the repo** make a genuine outbound request: `web-fetch`,
`http-request`, `password-pwned-check`, `css-select-extract`, `ffmpeg`. None
have pages. Meanwhile the `/tools/` index claims "nothing leaves your device,
works offline" of everything it lists.

## Scope

32 tools get pages, in three batches, plus a shared infrastructure change.

**Batch A — 15 SVG tools** (chart/graphic generators, `Input::None`, no network):
`calendar-heatmap`, `candlestick-chart`, `correlation-heatmap`, `function-grapher`,
`gradient-image-generator`, `heatmap-chart`, `hexbin-density-chart`,
`latex-math-to-svg`, `line-series-chart`, `otpauth-qr-generator`,
`qr-code-generator`, `risk-matrix`, `scatter-chart`, `svg-placeholder-generator`,
`wifi-qr-code-generator`.

**Batch B — 12 text tools** (`Input::None`, no network, string output):
`crypto-keypair-generator`, `ed25519-key-pair-generator`, `generate-ecdsa-key-pair`,
`generate-pgp-key-pair`, `generate-rsa-key-pair`, `keypair-generator`,
`pdf-object-analyzer`, `pgp-encrypt`, `pgp-sign`, `photo-gps-mapper`,
`sm2-keypair-generate`, `ssh-keygen`.

**Batch C — 5 network tools**: `web-fetch`, `http-request`, `password-pwned-check`,
`css-select-extract`, `ffmpeg`.

Four of Batch A (`qr-code-generator`, `otpauth-qr-generator`,
`gradient-image-generator`, `hexbin-density-chart`) take a `format = svg|png`
parameter. Their pages expose the **SVG path only**; PNG output remains a
chat/CLI capability until a binary-output page path exists.

### Explicitly out of scope

- The remaining 48 binary/PDF/ZIP or file-input page-less tools — these need a
  page upload path and a binary result renderer.
- The 120 `url`/`ref` file-input tools — same dependency.
- The `requires` manifest drift: the 5 genuine-network blocks do **not** declare
  `wafer-run/network`, while 144 blocks that never call it do. Real, but it
  touches ~149 manifests and belongs in its own change.

## Approach

Shared plumbing lands first as one PR, then three tool batches, then a
`gizza-ai-pin.txt` bump in gizza-site to publish. This mirrors how the existing
595 pages were built and keeps each review to a single shape.

### PR 1 — infrastructure (gizza-ai)

**SVG rendering.** The SVG is rendered as
`<img src="data:image/svg+xml;base64,…">` rather than injected with
`innerHTML`. An SVG loaded as an image cannot execute script, so the XSS surface
disappears without adding DOMPurify to the tool-page runtime and without relying
on each block escaping user text correctly (`scatter-chart` has an `esc()`
helper, but that is per-block convention, not an enforced guarantee). It also
inherits the existing Download and "Copy image" affordances.

Changes:

- `tools/generator/src/meta.rs` — document and accept `format = "svg"`.
- `tools/generator/src/template.rs` — add `svg` to the `is_media` set so the
  media element, Download and Copy image render.
- `tools/generator/assets/runtime/tool.js` — one branch in `showResult`: build
  the data URI, set `dl.download = "<slug>.svg"`, enable Copy image, and attach
  an `onerror` fallback that shows the raw value in the text pane when the SVG
  is empty or malformed.

`qr-paper-backup`, today's only `format = "svg"` tool, picks this up
automatically and stops printing raw markup.

**Network disclosure.** Add `network: bool` (default `false`) to `ToolMeta`.
When set, `template.rs` renders a badge above the widget stating that the tool
makes a real request, that it leaves the browser, and that the target site must
permit cross-origin access.

Per-tool privacy claims live in per-tool copy (`hero_subtitle`, `content.md`),
not in the shared template, so there is nothing for the template to suppress —
keeping those two consistent is the hygiene check's job, not the renderer's.
The check enforces both directions:

- a block whose Rust calls `network::do_request` and has a page MUST set
  `network = true`;
- a page with `network = true` MUST NOT contain local-only phrasing
  ("nothing leaves your device", "runs locally", "no upload", "works offline")
  in `hero_subtitle` or `content.md`.

**Index claim.** With the 5 network tools listed on `/tools/`, that page's hero
line and meta description ("nothing leaves your device, works offline") are no
longer true of everything listed, so the same PR qualifies both.

### PRs 2–4 — tool batches

Each tool gets `page/meta.toml`, `page/content.md` (~500 words including an FAQ
section, which feeds the FAQPage JSON-LD), and a `web/` crate wrapping its
existing `core`. No block logic changes: the pure logic exists and is unit
tested. `Cargo.lock` and `target/block.wasm` are committed; `web/pkg/` is not
(CI rebuilds it).

`password-pwned-check` copy must state precisely what leaves the browser — the
first 5 characters of the password's SHA-1 hash, never the password itself.
"Makes a network request" alone overstates the exposure.

## Error handling

- **Malformed/empty SVG** — the `<img>` fails silently, so the `onerror`
  fallback renders the raw value in the text pane instead of a blank result.
- **CORS refusal** on the network pages — the driver already emits
  "Couldn't fetch … the host may block cross-origin access"; the badge sets that
  expectation before the user runs the tool. `web-fetch`, `http-request` and
  `css-select-extract` will fail on most arbitrary URLs. This is not a
  regression: chat uses the same `bridge.httpFetch` → browser `fetch` path with
  no proxy, so the pages are exactly as capable as chat is today.

A same-origin proxy would make those three work for any URL, and was rejected:
it routes user traffic through gizza's server (contradicting the privacy
position) and publishes an open proxy.

## Testing

- Per-tool Playwright specs at `tests/tool-page-<slug>.spec.ts`, matching the
  existing convention.
- `#[test]`s in `template.rs` for the SVG media branch and the network badge.
- A `node --test` case for the `showResult` SVG path, alongside the existing
  `js/tool-ffmpeg.test.js`.
- The new hygiene check, run in CI.
- PR CI runs per-block cargo tests for changed blocks; the nightly sweep covers
  the full suite.

## Rollout and verification

1. Merge PR 1 (inert except that it fixes `qr-paper-backup`).
2. Merge batches A, B, C.
3. Bump `gizza-ai-pin.txt` in gizza-site; the push to `main` deploys.

Verify after deploy: sitemap tool count 595 → 627; sampled new pages return 200;
an SVG page renders an image rather than markup; the badge is visible on a
network page.

**Deploy cost:** 32 new pages add 32 per-tool `wasm-pack` builds to the deploy
loop (127 → 159), roughly 20–25 minutes on top of the current 67-minute deploy.
