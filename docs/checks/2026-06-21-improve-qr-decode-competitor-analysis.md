# qr-decode — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/qr-decode` — read and decode the data in a QR code image.
Pure-Rust (`image` + `quircs`). Image input → text output, so **chat + CLI, no
page** (the page file-input path is ffmpeg-only — like `image-info` /
`detect-file-type`).

## What competitors do

- **Online QR readers** (many "scan QR from image" sites) — upload an image, get
  the decoded text. Strength: easy. **Weakness: you upload the image — which often
  *is* the secret (a Wi-Fi password, a 2FA `otpauth://` seed, a payment URL)** —
  to a third-party server, frequently ad-supported.
- **`zbarimg` (ZBar), `zxing` CLIs / libraries** — local and accurate, but require
  installing native tooling (often C libraries) and aren't browser-runnable.
- **Phone camera apps** — convenient for live scanning but can't decode an image
  file you already have on a computer, and aren't scriptable.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`image` decodes PNG/JPEG/GIF/
   BMP/WebP; `quircs` decodes the QR) compiled to wasm: runs in the chat Service
   Worker and headless in the CLI. The image — and any secret it encodes — never
   leaves the device.
2. **Decodes every code in the image.** Returns the text of *all* QR codes found,
   in detection order, not just the first — handy for sheets/screenshots with
   several codes.
3. **Robust detection.** `quircs` (a pure-Rust port of the `quirc` library)
   locates and perspective-corrects QR grids, so
   it handles rotated / slightly skewed / photographed codes, not only pixel-perfect
   renders.
4. **Chainable + agent-friendly.** Takes the image by `url` or `ref` and returns
   flat JSON (`decoded`, `count`) the model reads directly — callable identically
   from chat and CLI, and composable with the other media tools.

## Honest scope

- **Decode only** — this reads QR codes; generating them is a separate concern.
- **QR codes specifically** — not other 2D/1D barcode symbologies (Data Matrix,
  Aztec, Code-128, EAN, …).
- **No page** — image input + text output don't fit the page's text/field model
  (consistent with the other image-input tools).

## Tests

4 core unit tests: a known URL and a longer mixed-content string are each rendered
to a PNG QR (via the `qrcode` crate) and **round-trip decoded** back to the exact
original text; decoding a non-image errors; and a blank image (no QR) errors with
a clear message. Plus the block drift-guard schema test. **CLI verified** end-to-
end against a live QR-image endpoint (`api.qrserver.com` → a PNG encoding
`https://gizza.ai`), which decodes back to that URL. `wafer build` instantiates
the chat block in the wafer runtime (1.18 MiB) — confirming `image` + `quircs`
run under wasm32-wasip1. (Note: the `rqrr` QR crate compiles to wasm but pulls
filesystem WASI imports the runtime doesn't provide, so it fails to instantiate;
`quircs`, which decodes from a raw grayscale buffer, does not.)
