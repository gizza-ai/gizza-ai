# Tool-page batches A / B / C — next steps

Status: **not started.** The infrastructure they depend on is merged
(`6897f3cd`, PR #224). Each batch below needs its own spec-to-plan pass before
implementation — they are listed here so the work is not lost, not as a
ready-to-execute plan.

Design: `docs/superpowers/specs/2026-07-28-tool-page-expansion-and-network-disclosure-design.md`
Infra plan (completed): `docs/superpowers/plans/2026-07-28-tool-page-svg-and-network-disclosure-infra.md`

## Why these tools have no page today

Audit of all 812 blocks (2026-07-28): 595 have a `page/`, 217 do not. Of those 217:

- **136** were treated as chat-only because they declare `wafer-run/network` —
  but 120 of those use the network *only* to fetch a `url`/`ref` input. On a
  page that input is an upload, so they need no network at all. They are blocked
  on a page upload path, not on networking.
- Only **5 blocks in the whole repo** call `network::do_request`:
  `web-fetch`, `http-request`, `password-pwned-check`, `css-select-extract`,
  `ffmpeg`. These are batch C.
- The rest are blocked on output shape (binary/PDF/ZIP) or file input.

`requires` in `manifest.json` is NOT a usable signal for any of this: the 5
blocks that call `do_request` do not declare it, while 144 that never call it
do. Key off the Rust source, as hygiene check 9 does.

## Batch A — 15 SVG tools

`format = "svg"`, `Input::None`, no network. Now renderable thanks to the infra PR.

`calendar-heatmap`, `candlestick-chart`, `correlation-heatmap`, `function-grapher`,
`gradient-image-generator`, `heatmap-chart`, `hexbin-density-chart`,
`latex-math-to-svg`, `line-series-chart`, `otpauth-qr-generator`,
`qr-code-generator`, `risk-matrix`, `scatter-chart`, `svg-placeholder-generator`,
`wifi-qr-code-generator`.

Four of these (`qr-code-generator`, `otpauth-qr-generator`,
`gradient-image-generator`, `hexbin-density-chart`) take a `format = svg|png`
parameter. **Their pages expose the SVG path only** — PNG output stays a
chat/CLI capability until a binary-output page path exists.

## Batch B — 12 text tools

`format = "text"`, `Input::None`, no network, string output. Mostly keygen/crypto.

`crypto-keypair-generator`, `ed25519-key-pair-generator`, `generate-ecdsa-key-pair`,
`generate-pgp-key-pair`, `generate-rsa-key-pair`, `keypair-generator`,
`pdf-object-analyzer`, `pgp-encrypt`, `pgp-sign`, `photo-gps-mapper`,
`sm2-keypair-generate`, `ssh-keygen`.

## Batch C — 5 network tools

Each sets `network = true` in `page/meta.toml` (renders the disclosure badge).

`web-fetch`, `http-request`, `password-pwned-check`, `css-select-extract`, `ffmpeg`.

Two behave differently and the copy must reflect it:

- `password-pwned-check` and `ffmpeg` work reliably in-browser (the HIBP range
  API is CORS-enabled by design; `ffmpeg` on a page takes an upload and needs no
  network at all).
- `web-fetch`, `http-request` and `css-select-extract` will fail on most
  arbitrary URLs — most sites send no CORS headers. This is **not** a regression
  versus chat: chat uses the same `bridge.httpFetch` → browser `fetch` with no
  proxy, so the pages are exactly as capable as chat is today. The badge states
  the CORS caveat up front.

`password-pwned-check` copy must state precisely what leaves the browser: the
first 5 characters of the password's SHA-1 hash, never the password. "Makes a
network request" alone overstates the exposure.

A same-origin proxy would make the three CORS-limited tools work for any URL and
was **rejected**: it routes user traffic through gizza's server (contradicting
the privacy position) and publishes an open proxy.

## Per-tool checklist

Each tool needs:

- `page/meta.toml` — title/description/tags/h1/hero_subtitle, `wasm`, `export`,
  `output_label`, `format`, `[[input]]` blocks. Batch C also sets `network = true`.
- `page/content.md` — ~500 words including an FAQ section of ≥3 `<details>`
  accordions (feeds the FAQPage JSON-LD; hygiene checks 2 and 6 gate this).
- `web/` crate — a wasm-bindgen wrapper over the block's existing `core`. No
  block logic changes: the pure logic already exists and is unit tested.
- A Playwright spec at `tests/tool-page-<slug>.spec.ts`.

Commit `Cargo.lock` and `target/block.wasm`; **never** `web/pkg/` (gitignored,
CI rebuilds it).

Run `python3 scripts/check-tool-hygiene.py <slug>` per tool — per-slug mode
gates checks 1-9 strictly, including the placeholder, FAQ-depth and
50-170-char description rules that are only advisory repo-wide.

## Gotchas learned during the infra PR

- `blocks/<slug>/web/pkg/` is gitignored and mostly absent locally. A page
  cannot be Playwright-tested until you run
  `wasm-pack build blocks/<slug>/web --target web --release --out-dir pkg`.
- `.cargo/config.toml` carries a `[patch]` redirecting `wafer-*` to
  `../wafer-run`. Any cargo/wasm-pack run under `blocks/` can write a
  `Cargo.lock` with stripped git `source` lines. Check `git status --short`
  before committing and never stage a lockfile you did not intend to change.
- `tool.js` statically imports `tool-svg.js`; `tools/generator/src/main.rs`
  copies it next to each page. If a new page-emitting path is ever added, it
  must copy that file too or `tool.js` fails to load entirely on those pages.
- Adding a test file to `js/` requires editing the root `package.json` `test`
  script — it enumerates files explicitly.

## Deferred minors from the infra PR

Triaged as safe to leave by the final whole-branch review; revisit opportunistically.

1. `tool.js` — `compute()`'s "all fields empty" branch does not hide
   `#tool-output-media` / `#tool-output-download` (it only clears the text and
   `lastResultText`). Currently unreachable: `qr-paper-backup`'s checkbox always
   yields `"true"`/`"false"`. **Re-check when batch A lands an svg tool whose
   inputs are all optional text fields** — that would make it reachable.
2. `scripts/check-tool-hygiene.py` — `NETWORK_CALL_RE` rglobs the whole block
   tree including `vendor/`, and scans raw text rather than comment-stripped
   source (the file already has a `strip_line_comments` helper). Both fail loud
   (false positive on the gate), never silent.
3. Same file — the docstring reads 1,2,3,4,9,8. Cosmetic.

## After the batches

Bump `gizza-ai-pin.txt` in the gizza-site repo and push to `main` to deploy.

**Deploy cost:** each new page adds a per-tool `wasm-pack` build to the deploy
loop. All 32 pages take it from 127 to 159 builds — roughly 90 minutes versus
the current ~67.

Verify after deploy: sitemap tool count 595 → 627; sampled new pages return 200;
an SVG page renders an image rather than markup; the badge is visible on a
batch-C page.

## Also out of scope, still open

- The **48** binary/PDF/ZIP or file-input page-less tools, and the **120**
  `url`/`ref` file-input tools. Both need a page upload path and a binary result
  renderer — a larger piece of work than these three batches.
- **`requires` manifest drift**: the 5 genuine-network blocks do not declare
  `wafer-run/network`, while 144 blocks that never call it do. Real, but it
  touches ~149 manifests and belongs in its own change.
- `blocks/xsalsa20-cipher/` has no `manifest.json` — its 6 tracked files are
  stale `web/pkg/` build output committed before that path was gitignored, and
  the block's source is gone. Dead weight; not a live tool (no manifest means
  `gizza list` never surfaces it).
