# Tool-page SVG rendering + network disclosure (infrastructure) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `format = "svg"` a real rendering path on tool pages and add a declarative, enforced "this makes a real network request" disclosure — the shared plumbing that batches A/B/C depend on.

**Architecture:** SVG results render as `<img src="data:image/svg+xml;base64,…">`, which cannot execute script and needs no sanitizer dependency; the raw markup stays available to "Copy result". A new `network: bool` in `ToolMeta` renders a badge, and `scripts/check-tool-hygiene.py` gains check 9 keeping that flag consistent with each block's Rust and each page's privacy copy. The `/tools/` index privacy claim is corrected in all four places it is emitted.

**Tech Stack:** Rust (maud templates, `tools/generator`), vanilla ES modules (`tools/generator/assets/runtime`), Python (hygiene gate), Playwright + `node --test`.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-28-tool-page-expansion-and-network-disclosure-design.md`.
- No per-tool slug branches in `tool.js` — behaviour is driven by `meta.toml` values only (existing rule, stated at the top of `tool.js`).
- SVG must never be injected via `innerHTML`. Data-URI `<img>` only.
- Local-only phrases, forbidden on any page with `network = true`: `nothing leaves your device`, `runs locally`, `no upload`, `works offline`, `runs entirely in your browser`.
- Branch from `main`, land as a PR. Never commit direct to main.
- `blocks/*/web/pkg/` is gitignored and must not be force-added.
- Run before pushing: `cargo test --manifest-path tools/generator/Cargo.toml`, `npm test`, `python3 scripts/check-tool-hygiene.py`, `bash scripts/check-tool-hygiene.test.sh`.

## File Structure

| File | Responsibility |
|---|---|
| `tools/generator/assets/runtime/tool-svg.js` (create) | Pure `svgDataUrl()` helper. Separate module so `node --test` can import it — `tool.js` runs on import and touches the DOM. Mirrors the `tool-ffmpeg.js` split. |
| `js/tool-svg.test.js` (create) | Node unit tests for `svgDataUrl`. |
| `package.json` (modify) | Register the new test file in the `test` script (it enumerates files explicitly). |
| `tools/generator/assets/runtime/tool.js` (modify) | `showSvgResult` branch, `lastResultText` for copy, error/reset clearing. |
| `tools/generator/src/meta.rs` (modify) | Document `format = "svg"`; add `network: bool`. |
| `tools/generator/src/template.rs` (modify) | Treat svg as media; keep "Copy result" for svg; render the network badge; correct the index claim. |
| `tools/generator/src/og.rs`, `tools/generator/src/index.rs` (modify) | The other two copies of the index privacy claim. |
| `tools/generator/assets/runtime/tool.css` (modify) | `.tool-network-note` styling. |
| `scripts/check-tool-hygiene.py` (modify) | Check 9: network flag ↔ Rust ↔ page copy. |
| `scripts/check-tool-hygiene.test.sh` (modify) | Self-test case for check 9. |
| `tests/tool-page-qr-paper-backup.spec.ts` (modify) | Existing spec asserts raw markup in `#tool-output`; must assert the rendered `<img>` instead. |

---

### Task 1: `svgDataUrl` helper

**Files:**
- Create: `tools/generator/assets/runtime/tool-svg.js`
- Create: `js/tool-svg.test.js`
- Modify: `package.json` (the `scripts.test` line)

**Interfaces:**
- Consumes: nothing.
- Produces: `svgDataUrl(svg: string) -> string` — a `data:image/svg+xml;base64,…` URI, or `""` for empty/blank/non-string input. Task 3 imports it.

- [ ] **Step 1: Write the failing test**

Create `js/tool-svg.test.js`:

```js
import { test } from "node:test";
import assert from "node:assert/strict";
import { svgDataUrl } from "../tools/generator/assets/runtime/tool-svg.js";

const PREFIX = "data:image/svg+xml;base64,";

test("svgDataUrl builds a base64 data URI that round-trips", () => {
  const svg = '<svg xmlns="http://www.w3.org/2000/svg"><rect width="4" height="4"/></svg>';
  const url = svgDataUrl(svg);
  assert.ok(url.startsWith(PREFIX));
  assert.equal(Buffer.from(url.slice(PREFIX.length), "base64").toString("utf8"), svg);
});

test("svgDataUrl handles non-ASCII label text", () => {
  // Chart titles/labels carry arbitrary user text; btoa() alone throws on any
  // code point > U+00FF, which is why the helper encodes via TextEncoder.
  const svg = '<svg xmlns="http://www.w3.org/2000/svg"><text>café 東京</text></svg>';
  const url = svgDataUrl(svg);
  assert.equal(Buffer.from(url.slice(PREFIX.length), "base64").toString("utf8"), svg);
});

test("svgDataUrl returns empty string for empty, blank or non-string input", () => {
  assert.equal(svgDataUrl(""), "");
  assert.equal(svgDataUrl("   \n"), "");
  assert.equal(svgDataUrl(null), "");
  assert.equal(svgDataUrl(undefined), "");
});
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `node --test js/tool-svg.test.js`
Expected: FAIL — `Cannot find module .../tool-svg.js`.

- [ ] **Step 3: Write the minimal implementation**

Create `tools/generator/assets/runtime/tool-svg.js`:

```js
// Pure helpers for the format="svg" output path. Kept out of tool.js — that
// module runs on import (reads window.GIZZA_TOOL, touches the DOM), so it can't
// be imported by node --test. Same split as tool-ffmpeg.js.

const PREFIX = "data:image/svg+xml;base64,";

/**
 * Build a data: URI for an SVG string, for use as an <img> src.
 *
 * Rendering SVG through <img> (rather than innerHTML) means the markup cannot
 * execute script, so no sanitizer is needed and no block has to be trusted to
 * escape user text correctly.
 *
 * Encodes via TextEncoder because btoa() throws on any code point > U+00FF and
 * chart labels carry arbitrary user text. Returns "" for empty/blank/non-string
 * input so the caller can fall back to the text pane instead of setting a
 * broken src.
 */
export function svgDataUrl(svg) {
  if (typeof svg !== "string" || !svg.trim()) return "";
  const bytes = new TextEncoder().encode(svg);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return PREFIX + btoa(bin);
}
```

- [ ] **Step 4: Run the test and make sure it passes**

Run: `node --test js/tool-svg.test.js`
Expected: PASS — 3 tests.

- [ ] **Step 5: Register the test file in the npm script**

`package.json`'s `test` script enumerates files explicitly, so a new file is not picked up automatically. Change:

```json
    "test": "node --test js/query-prefill.test.js js/tool-ffmpeg.test.js js/tools-index.test.js"
```

to:

```json
    "test": "node --test js/query-prefill.test.js js/tool-ffmpeg.test.js js/tools-index.test.js js/tool-svg.test.js"
```

- [ ] **Step 6: Run the full JS suite**

Run: `npm test`
Expected: PASS, and the output includes the 3 new `svgDataUrl` tests.

- [ ] **Step 7: Commit**

```bash
git add tools/generator/assets/runtime/tool-svg.js js/tool-svg.test.js package.json
git commit -m "feat(tool-pages): add svgDataUrl helper for format=svg rendering"
```

---

### Task 2: Render `format = "svg"` as media in the page template

**Files:**
- Modify: `tools/generator/src/template.rs:278-325`
- Modify: `tools/generator/src/meta.rs:92` (doc comment on `format`)

**Interfaces:**
- Consumes: nothing from Task 1 (template-side only).
- Produces: pages whose `meta.format == "svg"` render `<img id="tool-output-media">`, a `#tool-output-download` anchor, and a `#tool-copy-output` ("Copy result") button. Task 3's JS targets exactly those ids.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `tools/generator/src/template.rs`. The existing tests build a `ToolMeta` via helpers named `branded()` and `sample()` — follow the pattern already used by `faq_json_ld_mirrors_content_faq_section`, setting `format` on a mutated `sample()`:

```rust
    #[test]
    fn svg_format_renders_an_img_and_download() {
        let mut meta = sample();
        meta.format = "svg".to_string();
        let html = render_page(&branded(), &meta, "", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(
            html.contains(r#"<img id="tool-output-media""#),
            "svg output renders the <img> media element"
        );
        assert!(
            html.contains(r#"id="tool-output-download""#),
            "svg output offers a download link"
        );
    }

    #[test]
    fn svg_format_keeps_copy_result_and_omits_copy_image() {
        // The SVG source is what an SVG user wants on the clipboard. "Copy image"
        // is deliberately NOT offered: ClipboardItem is reliably PNG-only and
        // canvas-drawing an SVG varies by browser.
        let mut meta = sample();
        meta.format = "svg".to_string();
        let html = render_page(&branded(), &meta, "", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(html.contains(r#"id="tool-copy-output""#), "Copy result is offered for svg");
        assert!(!html.contains(r#"id="tool-copy-image""#), "Copy image is not offered for svg");
    }

    #[test]
    fn image_format_still_offers_copy_image() {
        let mut meta = sample();
        meta.format = "image".to_string();
        let html = render_page(&branded(), &meta, "", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(html.contains(r#"id="tool-copy-image""#), "raster image output keeps Copy image");
        assert!(!html.contains(r#"id="tool-copy-output""#), "raster image output has no Copy result");
    }
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --manifest-path tools/generator/Cargo.toml svg_format`
Expected: FAIL — `svg_format_renders_an_img_and_download` fails because `format = "svg"` currently falls into the plain `output` branch, so no `<img>` is emitted.

- [ ] **Step 3: Extend the media branch to include svg**

In `tools/generator/src/template.rs`, replace lines 279-302 (the block that starts `@if meta.format == "image" || meta.format == "video" || meta.format == "audio" {`) with:

```rust
                            // Media display covers svg too: the SVG is rendered as
                            // <img src="data:image/svg+xml;base64,…">, which cannot
                            // execute script the way innerHTML would, so no sanitizer
                            // dependency and no reliance on each block escaping user
                            // text correctly.
                            @let is_media = meta.format == "image" || meta.format == "video"
                                || meta.format == "audio" || meta.format == "svg";
                            // Binary media = the formats whose result is bytes, not text.
                            // svg is media for DISPLAY but still text for copy/paste.
                            @let is_binary_media = meta.format == "image" || meta.format == "video"
                                || meta.format == "audio";
                            @if is_media {
                                @if meta.format == "image" || meta.format == "svg" {
                                    img id="tool-output-media" class="tool-output-media" alt="" hidden;
                                } @else if meta.format == "video" {
                                    video id="tool-output-media" class="tool-output-media" controls hidden {}
                                } @else {
                                    audio id="tool-output-media" class="tool-output-media" controls hidden {}
                                }
                                // Media-output actions row. Download is offered for
                                // every media format; "Copy image" only for raster
                                // images — video/audio have no reliable ClipboardItem
                                // path, and for svg the clipboard is reliably PNG-only
                                // while canvas-drawing an SVG varies by browser, so svg
                                // uses the text "Copy result" button instead. All start
                                // hidden; tool.js reveals them once a result renders.
                                div class="tool-media-actions" {
                                    a id="tool-output-download" class="tool-output-download" download hidden { "Download" }
                                    @if meta.format == "image" {
                                        button id="tool-copy-image" class="tool-widget-btn" type="button" hidden
                                               title="Copy the image to the clipboard" { "Copy image" }
                                    }
                                }
                                output id="tool-output" class="tool-output" { "" }
                            } @else {
                                output id="tool-output" class="tool-output" { "" }
                            }
```

- [ ] **Step 4: Use the hoisted bindings below and delete the duplicate**

Immediately after that block, lines 303-325 currently re-declare `is_media`. Replace:

```rust
                            @let has_fields = meta.inputs.iter().any(|i| i.source == "field");
                            @let is_media = meta.format == "image" || meta.format == "video" || meta.format == "audio";
                            @if has_fields || !is_media {
```

with (the `@let is_media` line is deleted — it is now declared above and includes svg):

```rust
                            @let has_fields = meta.inputs.iter().any(|i| i.source == "field");
                            @if has_fields || !is_media {
```

and change the "Copy result" gate from `@if !is_media {` to:

```rust
                                    @if !is_binary_media {
```

Leave the `@if meta.format == "text"` text-download gate exactly as-is: svg uses the media download anchor, not the `data-text-download` one, so only one `#tool-output-download` is ever rendered.

- [ ] **Step 5: Update the `format` doc comment**

In `tools/generator/src/meta.rs`, change line 92's doc comment from:

```rust
    /// "number" or "text" — how to render the result.
```

to:

```rust
    /// How to render the result: "number", "text", "svg" (rendered as an
    /// <img> data URI, source still copyable), or the media formats
    /// "image"/"video"/"audio". Any other value renders as plain text.
```

- [ ] **Step 6: Run the tests and make sure they pass**

Run: `cargo test --manifest-path tools/generator/Cargo.toml`
Expected: PASS, including the three new tests and every pre-existing template test.

- [ ] **Step 7: Commit**

```bash
git add tools/generator/src/template.rs tools/generator/src/meta.rs
git commit -m "feat(tool-pages): render format=svg as an <img> with download + copy source"
```

---

### Task 3: Wire the SVG render path in `tool.js`

**Files:**
- Modify: `tools/generator/assets/runtime/tool.js:1-40` (import, `showResult`, `showError`), `:178-212` (reset + copy handlers)
- Modify: `tests/tool-page-qr-paper-backup.spec.ts`

**Interfaces:**
- Consumes: `svgDataUrl` from Task 1; the `#tool-output-media` / `#tool-output-download` / `#tool-copy-output` elements from Task 2; `cfg.slug` (already present in `client_config`, `meta.rs:158`).
- Produces: the runtime behaviour batches A/B/C rely on. No exports.

- [ ] **Step 1: Write the failing test**

`qr-paper-backup` is the repo's only existing `format = "svg"` page, and its spec currently asserts the raw markup sits in `#tool-output` — which this change makes false. Rewrite the first test in `tests/tool-page-qr-paper-backup.spec.ts` to assert the rendered image instead:

```ts
test('qr-paper-backup renders a printable SVG sheet for text input', async ({ page }) => {
  await page.goto('/tools/qr-paper-backup/');
  await page.fill('#in-input', 'paper backup demo');
  await page.selectOption('#in-input_encoding', 'text');
  await page.fill('#in-chunk_bytes', '300');
  await page.fill('#in-columns', '2');
  await page.selectOption('#in-error_correction', 'M');

  // format="svg" now renders through <img src="data:image/svg+xml;base64,…">
  // instead of dumping markup into #tool-output.
  const img = page.locator('#tool-output-media');
  await expect(img).toBeVisible({ timeout: 15000 });
  const src = await img.getAttribute('src');
  expect(src?.startsWith('data:image/svg+xml;base64,')).toBe(true);

  const svg = Buffer.from(src!.slice('data:image/svg+xml;base64,'.length), 'base64').toString('utf8');
  expect(svg).toContain('<svg xmlns=');
  expect(svg).toContain('QR paper backup');
  expect(svg).toContain('Part 1 / 1');
  expect(svg).toContain('QRB1|1|1|');

  await expect(page.locator('#tool-output-download')).toBeVisible();
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'qr-paper-backup.svg');
});
```

Apply the same change to the second test (`deep-link decodes base64 and hides payload text`): read `#tool-output-media`'s `src`, decode it, and assert `toContain('11 bytes split into 1 QR codes.')` and `not.toContain('QRB1|1|1|')` against the decoded string rather than `#tool-output`.

- [ ] **Step 2: Run it to make sure it fails**

```bash
cargo run --manifest-path tools/generator/Cargo.toml -- .    # regenerate pkg/tools
cd tests && npx playwright test --config playwright.tool-pages.config.ts tool-page-qr-paper-backup.spec.ts
```
Expected: FAIL — `#tool-output-media` is present (Task 2) but never made visible, because `tool.js` has no svg branch yet.

- [ ] **Step 3: Implement the svg branch**

In `tools/generator/assets/runtime/tool.js`, add the import next to the existing one at the top:

```js
import { queryPrefill } from "./query-prefill.js";
import { svgDataUrl } from "./tool-svg.js";
```

Replace `showResult` and `showError` (lines 21-40) with:

```js
// The last result as text, for "Copy result". Kept separately because the svg
// path leaves #tool-output empty (the <img> is the visible result) while the
// SVG source is still what belongs on the clipboard.
let lastResultText = "";

function showResult(value) {
  out.classList.remove("error");
  if (custom.renderResult && custom.renderResult(value, customCtx)) {
    return;
  }
  lastResultText = String(value);
  if (cfg.format === "svg") {
    showSvgResult(lastResultText);
    return;
  }
  out.textContent = cfg.format === "number" ? formatNumber(value) : String(value);
  syncTextDownload();
}

// format="svg": render the markup as an <img> data URI and offer it as a .svg
// download. Never innerHTML — an SVG loaded as an image cannot execute script.
function showSvgResult(svg) {
  const media = document.getElementById("tool-output-media");
  const dl = document.getElementById("tool-output-download");
  const url = svgDataUrl(svg);
  if (!media || !url) {
    // Empty result, or a page rendered without the media element: fall back to
    // the text pane rather than leaving a blank widget.
    out.textContent = svg;
    return;
  }
  out.textContent = "";
  // A malformed SVG never paints and fires no error text of its own — show the
  // source so the user can see what came back instead of an empty box.
  media.onerror = () => {
    media.hidden = true;
    out.textContent = svg;
  };
  media.src = url;
  media.hidden = false;
  if (dl) {
    dl.href = url;
    dl.download = `${cfg.slug}.svg`;
    dl.hidden = false;
  }
}

function showError(message) {
  // Layout stability: never resize the widget on errors/keystrokes — nothing
  // may jump under the user's cursor. Wide layouts are the tool-widget--wide
  // class (meta.toml `wide = true`), not a JS width override.
  if (custom.renderError && custom.renderError(message, customCtx)) {
    return;
  }
  lastResultText = "";
  out.classList.add("error");
  out.textContent = message;
  syncTextDownload();
}
```

- [ ] **Step 4: Make "Copy result" and Reset aware of it**

In the reset handler (around line 182-188), add the clear alongside the existing ones:

```js
      if (media) media.hidden = true;
      if (dl) dl.hidden = true;
      if (copyImg) copyImg.hidden = true;
      lastResultText = "";
```

In the copy handler (around line 197), change:

```js
      const text = (out.textContent || "").trim();
```

to:

```js
      // lastResultText first: for format="svg" the visible result is the <img>
      // and #tool-output is empty, but the SVG source is what should be copied.
      const text = (lastResultText || out.textContent || "").trim();
```

- [ ] **Step 5: Run the test and make sure it passes**

```bash
cargo run --manifest-path tools/generator/Cargo.toml -- .
cd tests && npx playwright test --config playwright.tool-pages.config.ts tool-page-qr-paper-backup.spec.ts
```
Expected: PASS — both tests.

- [ ] **Step 6: Commit**

```bash
git add tools/generator/assets/runtime/tool.js tests/tool-page-qr-paper-backup.spec.ts
git commit -m "feat(tool-pages): render svg results as an image, keep source copyable"
```

---

### Task 4: `network` flag and disclosure badge

**Files:**
- Modify: `tools/generator/src/meta.rs` (the `ToolMeta` struct, near the `wide` field)
- Modify: `tools/generator/src/template.rs:152-154` (the hero section)
- Modify: `tools/generator/assets/runtime/tool.css`

**Interfaces:**
- Consumes: nothing.
- Produces: `ToolMeta.network: bool` (TOML key `network`, default `false`) and a `.tool-network-note` element in the hero. Task 5's hygiene check reads the same TOML key; batch C's pages set it.

- [ ] **Step 1: Write the failing tests**

Add to `tools/generator/src/template.rs`'s test module:

```rust
    #[test]
    fn network_flag_renders_the_disclosure_badge() {
        let mut meta = sample();
        meta.network = true;
        let html = render_page(&branded(), &meta, "", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(html.contains("tool-network-note"), "badge element is rendered");
        assert!(
            html.contains("makes a real network request"),
            "badge states that the request is real"
        );
        assert!(
            html.contains("must allow cross-origin access"),
            "badge sets the CORS expectation before the user runs the tool"
        );
    }

    #[test]
    fn pages_without_the_network_flag_have_no_badge() {
        let html = render_page(&branded(), &sample(), "", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(!html.contains("tool-network-note"), "no badge on a local-only tool");
    }
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --manifest-path tools/generator/Cargo.toml network_flag`
Expected: FAIL to compile — `ToolMeta` has no field `network`.

- [ ] **Step 3: Add the field**

In `tools/generator/src/meta.rs`, add to `ToolMeta` immediately after the `wide` field:

```rust
    /// Set for tools that make a real outbound request from the browser (the
    /// block calls `network::do_request`). Renders the disclosure badge and is
    /// enforced against the page's privacy copy by
    /// `scripts/check-tool-hygiene.py` check 9.
    #[serde(default)]
    pub network: bool,
```

- [ ] **Step 4: Render the badge**

In `tools/generator/src/template.rs`, replace the hero section at lines 152-154:

```rust
                    section class="tool-hero" {
                        h1 { (meta.h1) }
                        p class="tool-hero-sub" { (meta.hero_subtitle) }
                    }
```

with:

```rust
                    section class="tool-hero" {
                        h1 { (meta.h1) }
                        p class="tool-hero-sub" { (meta.hero_subtitle) }
                        // Every other tool on the site is local-only, so a tool that
                        // actually reaches the network has to say so before it runs.
                        @if meta.network {
                            p class="tool-network-note" {
                                "Heads up: this tool makes a real network request. "
                                "The address you enter is fetched from your browser, so \
                                 the request leaves your device and the target site must \
                                 allow cross-origin access — many sites do not."
                            }
                        }
                    }
```

- [ ] **Step 5: Style it**

Append to `tools/generator/assets/runtime/tool.css`:

```css
/* Network-disclosure badge (meta.toml `network = true`). Deliberately plain and
   informational rather than alarming: the request is expected behaviour for
   these tools, it just has to be stated because every other tool is local-only. */
.tool-network-note {
  margin: 10px 0 0;
  padding: 10px 12px;
  border: 1px solid var(--tool-note-border, #fbbf24);
  border-radius: 8px;
  background: var(--tool-note-bg, #fffbeb);
  color: var(--tool-note-ink, #78350f);
  font-size: 0.92rem;
  line-height: 1.45;
}
```

- [ ] **Step 6: Run the tests and make sure they pass**

Run: `cargo test --manifest-path tools/generator/Cargo.toml`
Expected: PASS, both new tests plus everything pre-existing.

- [ ] **Step 7: Commit**

```bash
git add tools/generator/src/meta.rs tools/generator/src/template.rs tools/generator/assets/runtime/tool.css
git commit -m "feat(tool-pages): declarative network disclosure badge via meta.toml network=true"
```

---

### Task 5: Hygiene check 9 — network flag ↔ Rust ↔ page copy

**Files:**
- Modify: `scripts/check-tool-hygiene.py` (module docstring, regex constants near line 78, `check_block` before its `return problems`)
- Modify: `scripts/check-tool-hygiene.test.sh`

**Interfaces:**
- Consumes: the `network` TOML key from Task 4.
- Produces: a gating check. Batch C cannot land a network page that claims to be local-only, and cannot forget the flag.

- [ ] **Step 1: Write the failing self-test**

`scripts/check-tool-hygiene.test.sh` builds a scratch block under `blocks/zzhygiene-scratch/` and asserts the gate fails on it. Read the existing file first to match its scaffolding, then add a case that (a) writes a `src/lib.rs` calling `network::do_request`, (b) writes a `page/meta.toml` **without** `network = true`, and asserts the gate fails mentioning `network = true`; then (c) sets `network = true` but puts `works offline` in `hero_subtitle` and asserts it fails mentioning the local-only claim.

Follow the file's existing structure; the assertion shape it already uses is:

```bash
out="$(python3 "$root/scripts/check-tool-hygiene.py" "$slug" 2>&1 || true)"
case "$out" in
  *"network = true"*) ;;
  *) echo "FAIL: expected a check-9 violation, got: $out"; exit 1 ;;
esac
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `bash scripts/check-tool-hygiene.test.sh`
Expected: FAIL — the gate reports no check-9 violation, because the check does not exist.

- [ ] **Step 3: Add the constants**

In `scripts/check-tool-hygiene.py`, after `BRAND_RE` (line 78):

```python
# A block that actually reaches the network calls the host service directly.
# `requires` in manifest.json is NOT a reliable signal — the five blocks that
# call do_request do not declare it, while 144 that never call it do.
NETWORK_CALL_RE = re.compile(r"network::do_request")
# Claims that are false (or misleading) on a page that fetches over the network.
LOCAL_ONLY_RE = re.compile(
    r"nothing leaves your device|runs locally|no upload|works offline"
    r"|runs entirely in your browser",
    re.IGNORECASE,
)
```

- [ ] **Step 4: Add the check**

In `check_block`, immediately before `return problems`:

```python
    # 9. Network disclosure. A page whose block really reaches the network must
    #    say so (meta.toml `network = true`, which renders the badge), and a page
    #    that says so must not also claim to be local-only. Both directions are
    #    gated: a missing flag silently ships a page that contradicts the site's
    #    local-only promise, and a stale local-only sentence does the same.
    meta_path = slug_dir / "page" / "meta.toml"
    if meta_path.is_file():
        calls_network = any(
            NETWORK_CALL_RE.search(p.read_text(encoding="utf-8", errors="replace"))
            for p in slug_dir.rglob("*.rs")
        )
        try:
            meta = tomllib.loads(meta_path.read_text(encoding="utf-8", errors="replace"))
        except tomllib.TOMLDecodeError:
            meta = {}  # check 5-7 reports the parse error; don't double-report here
        declared = bool(meta.get("network", False))
        if calls_network and not declared:
            problems.append(
                f"{slug}: block calls network::do_request but page/meta.toml does not set "
                "network = true — the page must disclose the request (renders the badge)."
            )
        if declared:
            for name, text in (
                ("meta.toml hero_subtitle", str(meta.get("hero_subtitle", ""))),
                ("content.md", (slug_dir / "page" / "content.md").read_text(
                    encoding="utf-8", errors="replace"
                ) if (slug_dir / "page" / "content.md").is_file() else ""),
            ):
                m = LOCAL_ONLY_RE.search(text)
                if m:
                    problems.append(
                        f"{slug}: network = true but {name} claims {m.group(0)!r} "
                        "— a tool that fetches over the network is not local-only."
                    )
```

- [ ] **Step 5: Document it**

In the module docstring, after the check-4 paragraph (line 27) and before the check-8 paragraph, add:

```
  9. Network disclosure. A block that calls `network::do_request` and ships a page
     MUST set `network = true` in `page/meta.toml` (which renders the disclosure
     badge), and a page with that flag MUST NOT claim to be local-only ("nothing
     leaves your device", "runs locally", "no upload", "works offline", "runs
     entirely in your browser"). Every other tool on the site is local-only, so an
     undisclosed fetch silently contradicts the site's promise. `requires` in
     manifest.json is not usable for this: the blocks that call do_request do not
     declare it, and 144 that never call it do.
```

Also update the `Usage:` lines from `checks 1-4+8 gate` to `checks 1-4+8-9 gate`, and `checks 1-8 all gate` to `checks 1-9 all gate`.

- [ ] **Step 6: Run the self-test and the repo-wide gate**

```bash
bash scripts/check-tool-hygiene.test.sh
python3 scripts/check-tool-hygiene.py
```
Expected: self-test PASSES. The repo-wide run stays green — none of the five network blocks has a `page/` yet, so check 9 does not fire on any of them.

- [ ] **Step 7: Commit**

```bash
git add scripts/check-tool-hygiene.py scripts/check-tool-hygiene.test.sh
git commit -m "feat(hygiene): check 9 — network pages must disclose, and must not claim local-only"
```

---

### Task 6: Correct the site-wide privacy claim

**Files:**
- Modify: `tools/generator/src/template.rs:656-660` (index meta description) and `:699-702` (index hero paragraph)
- Modify: `tools/generator/src/og.rs:84-87`
- Modify: `tools/generator/src/index.rs:74-77`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing consumed by later tasks. This is the copy correction the spec requires once network tools appear in the index.

- [ ] **Step 1: Write the failing test**

Add to `tools/generator/src/template.rs`'s test module, matching the setup used by the existing `tools_index_lists_all_tools_with_chrome` test (line ~950):

```rust
    #[test]
    fn index_privacy_claim_is_qualified_for_network_tools() {
        let metas = [sample()];
        let hubs = crate::categories::build_hubs(&metas);
        let html = render_tools_index(&branded(), &metas, &hubs);
        assert!(
            !html.contains("nothing leaves your device, no sign-up, works offline"),
            "the unqualified blanket claim is gone"
        );
        assert!(
            html.contains("say so on their page"),
            "the index points at the per-page disclosure instead of promising local-only for all"
        );
    }
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test --manifest-path tools/generator/Cargo.toml index_privacy_claim`
Expected: FAIL — the blanket claim is still emitted verbatim.

- [ ] **Step 3: Update all four emission sites**

`template.rs` lines 658-660 — replace:

```rust
        "Browse every {}tool — free, private, browser-local utilities. \
         No sign-up, nothing leaves your device, works offline.",
```

with:

```rust
        "Browse every {}tool — free, private, browser-local utilities. \
         No sign-up, nothing uploaded. The few tools that fetch a URL say so on their page.",
```

`template.rs` lines 700-701 — replace:

```rust
                            "Free, private, browser-local tools. Everything runs in your browser — \
                             nothing leaves your device, no sign-up, works offline."
```

with:

```rust
                            "Free, private, browser-local tools. Everything runs in your browser — \
                             no sign-up, nothing uploaded. The few tools that fetch a URL say so \
                             on their page."
```

`index.rs` lines 75-77 — replace:

```rust
        "# {catalog_name}\n\n> Every {brand_prefix}tool — free, private, browser-local utilities. \
         Nothing leaves your device, no sign-up, works offline.\n\n",
```

with:

```rust
        "# {catalog_name}\n\n> Every {brand_prefix}tool — free, private, browser-local utilities. \
         No sign-up, nothing uploaded. The few tools that fetch a URL say so on their page.\n\n",
```

`og.rs` line 86 — the OG card is a fixed-width image, so keep it short. Replace:

```rust
                "{tool_count} free, private, browser-local tools — no sign-up, works offline."
```

with:

```rust
                "{tool_count} free, private, browser-local tools — no sign-up, nothing uploaded."
```

- [ ] **Step 4: Run the tests and make sure they pass**

Run: `cargo test --manifest-path tools/generator/Cargo.toml`
Expected: PASS. If a pre-existing test asserts the old wording, update that assertion to the new copy in the same commit.

- [ ] **Step 5: Verify no stale copy remains**

Run: `grep -rn "nothing leaves your device\|works offline" tools/ --include='*.rs'`
Expected: no matches.

- [ ] **Step 6: Commit**

```bash
git add tools/generator/src/template.rs tools/generator/src/og.rs tools/generator/src/index.rs
git commit -m "fix(tool-pages): qualify the index privacy claim now that network tools can ship pages"
```

---

### Task 7: Full verification and PR

**Files:** none (verification only).

- [ ] **Step 1: Run every gate**

```bash
cargo test --manifest-path tools/generator/Cargo.toml
npm test
python3 scripts/check-tool-hygiene.py
bash scripts/check-tool-hygiene.test.sh
```
Expected: all four green.

- [ ] **Step 2: Regenerate pages and run the affected Playwright specs**

```bash
cargo run --manifest-path tools/generator/Cargo.toml -- .
cd tests && npx playwright test --config playwright.tool-pages.config.ts tool-page-qr-paper-backup.spec.ts
```
Expected: PASS.

- [ ] **Step 3: Eyeball the one page this changes**

```bash
just serve       # serves pkg/ on :8001
```
Open `http://localhost:8001/tools/qr-paper-backup/`, run it, and confirm: the sheet renders as an image (not markup), "Download" saves `qr-paper-backup.svg`, and "Copy result" puts the SVG source on the clipboard.

- [ ] **Step 4: Confirm nothing else regressed**

```bash
git status --short          # no stray blocks/*/web/pkg/ staged
git diff --stat main
```
Expected: only the files listed in this plan's File Structure table.

- [ ] **Step 5: Open the PR**

```bash
git push -u origin HEAD
gh pr create --title "feat(tool-pages): svg rendering + network disclosure infrastructure" \
  --body "Implements PR 1 of docs/superpowers/specs/2026-07-28-tool-page-expansion-and-network-disclosure-design.md.

- format=\"svg\" renders as an <img> data URI (no innerHTML, no sanitizer dependency); source stays copyable via Copy result
- meta.toml \`network = true\` renders a disclosure badge
- hygiene check 9 gates both directions: undisclosed do_request, and local-only claims on a network page
- /tools/ index privacy claim qualified in all four places it is emitted

Only qr-paper-backup changes visibly today (it was printing raw markup). Unblocks batches A/B/C."
```

---

## Self-Review

**Spec coverage.** Every PR-1 item in the spec maps to a task: SVG data-URI rendering → Tasks 1-3; `meta.rs`/`template.rs`/`tool.js` changes → Tasks 2-3; `onerror` fallback → Task 3 Step 3; `network: bool` + badge → Task 4; hygiene check both directions → Task 5; index claim → Task 6. Batches A/B/C and the gizza-site pin bump are deliberately out of this plan — they need their own plans once this lands, since their pages cannot be tested before the SVG path exists.

**Deviation from the spec, recorded here:** the spec says SVG "inherits the existing Download and 'Copy image' affordances". Planning found that `#tool-copy-image` draws the `<img>` onto a canvas and writes a PNG `ClipboardItem`, which is unreliable for SVG sources (canvas tainting and intrinsic-size behaviour vary by browser). Task 2 therefore gives svg the text "Copy result" button — copying SVG source, which is more useful anyway — and omits "Copy image". The spec should be amended to match when this lands.

**Type consistency.** `svgDataUrl` is defined in Task 1 and consumed in Task 3 with the same signature. `ToolMeta.network` is added in Task 4 and read by Task 5's Python via the same TOML key `network`. `cfg.slug` used in Task 3 already exists (`meta.rs:158`). The `#tool-output-media` / `#tool-output-download` / `#tool-copy-output` ids used in Task 3 are exactly those rendered in Task 2.
