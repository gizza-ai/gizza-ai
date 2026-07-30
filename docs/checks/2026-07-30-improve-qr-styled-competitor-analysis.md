# qr-styled — competitor analysis (2026-07-30)

Tool function: generate a **styled** QR code — custom colors, gradient body, non-square
module shapes, customizable finder "eyes", and an embedded center logo — as a scalable SVG.
Distinct from the existing `blocks/qr-code-generator` (which does payload building +
plain solid-color SVG/PNG only, no gradients / module shapes / eye styling / logo).

## Competitor scan (one WebSearch; top real tools skimmed)

Paraphrased observations only — no competitor copy/branding/trademarks reproduced.

1. **QRCode Monkey** — customize the shape of the corner (eye) elements and the body
   modules; set colors for all elements; add a gradient (linear/radial) to the body; upload
   a center logo. High EC recommended with a logo.
2. **ME-QR (Art QR)** — brand colors or gradients; center logo; module shapes: dots, rounded
   squares, or vector shapes.
3. **qrGrid** — module shape helpers (circular / smooth-edge / rounded-corner), gradients,
   separate finder-pattern colors, logos.
4. **QRKIT / Free-AI-Domain generator** — foreground / background / eye colors (solid or
   gradient); module styles rounded / dots / diamond; drop a center logo; optional "SCAN ME"
   frame with a call-to-action caption.

## Table-stakes → in-model descriptor OR out-of-model

| Feature | Seen at | Decision |
|---|---|---|
| Body (module) color | all | **in-model** `fg_color` |
| Background color incl. transparent | all | **in-model** `bg_color` (accepts `transparent`) |
| Gradient body (linear + radial, angle) | Monkey, ME-QR, QRKIT | **in-model** `gradient` (none/linear/radial) + `gradient_color` + `gradient_angle` |
| Module shape (square / rounded / dots) | qrGrid, ME-QR, Monkey, QRKIT | **in-model** `module_shape` (square/rounded/dots) |
| Eye (finder) shape | Monkey, qrGrid | **in-model** `eye_shape` (square/rounded/circle) |
| Eye color separate from body | qrGrid, QRKIT | **in-model** `eye_color` (empty = match body) |
| Center logo embed | all | **in-model** `logo` (data:image URI) + `logo_size`; EC auto-raised to H, knockout drawn behind |
| Error correction level | all | **in-model** `error_correction` (L/M/Q/H) |
| Quiet zone / margin | all | **in-model** `margin` (modules) |
| Output size | all | **in-model** `size` (SVG width/height px) |
| SVG (vector) output | qrGrid, Monkey (SVG export) | **in-model** — SVG is the native output |
| "SCAN ME" decorative frame + caption | QRKIT, ME-QR | **out-of-model** — decorative framing has many template variants (rounded banners, ribbons, badges); a single caption param can't cover it. Users can add a caption in their design tool. |
| Diamond / star / vector custom module shapes | ME-QR, Free-AI | **out-of-model** — square/rounded/dots cover the mainstream; exotic vector shapes are a long tail. |
| 3D / photo-realistic styles (Vextrude) | Vextrude | **out-of-model** — needs raster/3D rendering, not a pure SVG generator. |
| Dynamic / trackable QR (analytics redirect) | Monkey, ME-QR paid | **out-of-model** — needs a hosted redirect + tracking backend; this tool is offline/pure. |

## Feasibility spikes (before tagging)

- **Module shapes / eyes / gradients / logo knockout**: all expressible as hand-built SVG
  (`<rect>`/`<circle>`, `rx`, `<linearGradient>`/`<radialGradient>`, `<image href="data:…">`).
  Confirmed feasible pure-Rust from the `qrcode` crate's `to_colors()` matrix — no new deps
  beyond `qrcode` (already proven wasm-safe, `features=["svg"]` not even needed since we build
  the SVG by hand).
- **Logo**: accept a `data:image/...` URI only (no network fetch → stays pure + safe); embed
  verbatim; draw a background-colored knockout circle behind it and force EC to High so the
  code still scans with the center occluded.

## Design outcome

Pure Rust, SVG image-bytes output → **chat + CLI, NO page** (image-bytes have no page render
mode — same as `qr-code-generator`, `wifi-qr-code-generator`, `otpauth-qr-generator`). Every
in-model table-stake above lands in the descriptor with a `.describe()`; every fixed-choice
param is a `Param::enumv`; a schema drift-guard test locks the LLM-facing schema.
