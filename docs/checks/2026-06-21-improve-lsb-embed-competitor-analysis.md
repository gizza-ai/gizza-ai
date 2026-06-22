# lsb-embed — competitor analysis (2026-06-21)

**Tool:** `gizza-ai/lsb-embed` — hide a secret text message inside an image's
least-significant bits (LSB steganography) and return a stego PNG that looks
identical to the original.

**Surfaces:** chat (LLM API) + CLI. **No page** — a pure-Rust image-bytes output
has no page render mode in the gizza page driver (same as `add-text-to-image` /
`code-screenshot`). Output is always a lossless PNG so the hidden bits survive
(a JPEG/WebP re-encode would destroy them).

## How it works (this implementation)

- Decodes any common raster input (PNG/JPEG/WebP/GIF/BMP) via the `image` crate,
  works on RGBA8.
- Embeds bit-by-bit (MSB-first) across the R, G, B channels (alpha skipped so a
  transparent pixel can't silently drop data), at `bits_per_channel` 1–4.
- Wire format in the LSBs: `MAGIC "LSB1"` (4 B) + `bits_per_channel` (1 B) +
  payload length (4 B, big-endian) + payload bytes. The matching `extract`
  (kept in core, used by the round-trip unit tests) recovers the exact bytes.
- Capacity is validated up front with a clear "use a bigger image or raise
  bits_per_channel" error.
- Verified end-to-end: CLI embed into a live fetched PNG → `core::extract`
  recovered the message exactly; 1-bit changes keep max per-channel delta ≤ 1.

## Top competitors surveyed

| Tool | Notable features | Notes |
|------|------------------|-------|
| Steganography Online (imageonline.io) | LSB encode/decode, PNG/JPG/WebP, in-browser | Encode + decode pair; no encryption |
| stylesuxx.github.io/steganography | Classic client-side LSB encode/decode | Reference open implementation |
| DevGlan Image Steganography | LSB **+ AES** encryption, live preview, client-side | Adds a password layer on top of LSB |
| 8gwifi.org Steganography Tool | Variable bit depth (up to 8×), **AES-256-GCM**, deflate compression, Reed-Solomon ECC, also hides files & WAV audio | Most feature-rich; many features out of gizza's single-input model |
| SteganoCrypt | AES-256 then LSB embed, "imperceptible" changes | Encryption-first framing |
| ToolPix Image Steganography | Browser-side LSB, hide + recover text in PNG/JPG | Encode + decode pair |

## Gap analysis (fit-to-model)

**Closed / matched in-model:**
- LSB embed into the common raster formats, lossless PNG output — matched.
- **Variable bit depth** (`bits_per_channel` 1–4) for higher capacity — matched
  the 8gwifi "variable bit depth up to 8×" capability (capped at 4 to keep the
  image visually clean; 4 bits/channel already gives 4× capacity).
- Clear up-front capacity check with an actionable error — matched.
- Alpha-channel-safe embedding — an edge several naive LSB tools get wrong.

**Deliberately out of scope (separate tools / not fit to the single-input model):**
- **Decode/extract direction.** Competitors ship encode+decode together, but in
  gizza a single descriptor has ONE input + one result shape; embed outputs an
  image, extract would output text — two IO shapes. Extract logic exists and is
  tested in core (`extract`), ready for a future `lsb-extract` tool. Noted, not
  built here.
- **AES / password encryption before embed** (DevGlan, SteganoCrypt, 8gwifi).
  gizza already has dedicated `text-encrypt` / `encrypt-file` tools; a user can
  encrypt first then embed the ciphertext. Layering crypto into this tool would
  duplicate that scope — left composable instead.
- **Hiding an arbitrary file as the payload, or hiding image-in-image.** Would
  need a second media input; the page driver + descriptor model is single media
  input. Out of model. (Text payload covers the common case and any data can be
  base64-encoded into the text field if needed.)
- **WAV / audio carrier** (8gwifi) — no `AssetKind::Audio` input in the gizza
  model. Out of model.
- **Reed-Solomon error correction** — only meaningful when the carrier may be
  re-compressed; gizza outputs lossless PNG, so the bits are already exact. Not
  needed.

## Verification

- `cargo test --workspace` in `blocks/lsb-embed`: 8 core round-trip/edge tests +
  1 schema drift-guard test — all pass.
- `wafer build`: block.wasm validates and instantiates (1370.7 KiB).
- `gizza list` shows the tool; `gizza tool lsb-embed url=… message=… bits_per_channel=2`
  produced a valid PNG whose hidden message round-tripped exactly via `core::extract`.

**No competitor copy, branding, or trademarks were used.**

Sources:
- [Steganography Online — imageonline.io](https://imageonline.io/steganography-online/)
- [Steganography Online — stylesuxx.github.io](https://stylesuxx.github.io/steganography/)
- [DevGlan Image Steganography](https://www.devglan.com/online-tools/image-steganography-online)
- [8gwifi.org Steganography Tool](https://8gwifi.org/steganography-tool.jsp)
- [SteganoCrypt](https://steganocrypt.netlify.app/)
- [ToolPix Image Steganography](https://toolpix.pythonanywhere.com/image-editor/steganography)
