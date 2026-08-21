# jpg-stego-embed — competitor analysis (2026-08-21)

Scan run **before** implementation, per `/improve-tool` Phase 2. Everything below is a
paraphrased summary of publicly documented behaviour; no competitor copy, branding or
trademark text is reproduced or reused anywhere in this repo.

## Tools reviewed

| # | Tool | Shape | Why it is a competitor |
|---|------|-------|------------------------|
| 1 | imgconceal (open-source CLI, GitHub `tbpaolini/imgconceal`) | CLI | The closest match: hides *arbitrary files* in JPEG/PNG/WebP carriers |
| 2 | steghide (classic open-source CLI, Debian manpage) | CLI | The canonical "hide a file in a JPEG" reference tool |
| 3 | Mobilefish steganography service | Web form | Browser tool: hide a message *or* a file inside an image |
| 4 | Tembrica steganography (browser) | Web app | Modern browser tool with encryption + capacity meter |

`steghide.sourceforge.net` returned HTTP 500 during the scan; the Debian `steghide(1)`
manpage was used as the substitute source for the same tool.

## Table-stakes matrix

| Capability | Seen in | Verdict | Where it landed |
|---|---|---|---|
| Arbitrary file payload (any type) | 1, 2, 3, 4 | **in-model** | `payload_url` / `payload_ref` |
| Plain text-message payload | 3, 4 | **in-model** | `payload_text` |
| Password protection / encryption | 1, 2, 3, 4 | **in-model** | `password` → AES-256-GCM + PBKDF2-HMAC-SHA256 (reuses `blocks/encrypt-file/core`) |
| Compress payload before hiding | 1, 2 | **in-model** | `compress` (deflate, default on) |
| Record the original filename with the payload | 1, 2 (`-N` opts out) | **in-model** | `filename`, stored in the container header |
| Integrity checksum on the payload | 2 (CRC32) | **in-model** | CRC-32 of the plaintext payload in the header, verified on extract |
| Carrier stays a working, viewable image | 1, 2, 3, 4 | **in-model** | Pixels are never touched — the JPEG's entropy data is copied byte-for-byte |
| Report capacity / size impact to the user | 4 (live meter), 1 (`--check`) | **in-model** | Summary line reports payload bytes, stored bytes, output bytes, growth % and segment count |
| Carrier size ceiling stated up front | 3 (300 KB carrier / 100 KB payload) | **in-model** | 16 MB carrier, 8 MB payload, 40 MB output — stated in the descriptor and error text |
| Choice of where the data goes | (implicit in 1, 2) | **in-model** | `method` = `app` (APP9 segments, default) / `comment` (COM segments) / `append` (after the EOI marker) |
| Output stays JPEG | 1, 2 | **in-model** — and a differentiator | 3 and 4 both force a PNG re-encode; we return the original JPEG bytes plus data |
| Extraction / "does this image hold anything?" check | 1, 2, 3, 4 | **out-of-model for this tool** | A separate extract tool is the right unit; `blocks/scan-embedded-files` already flags appended payloads today. `core::extract` exists and is round-trip unit-tested so a future `jpg-stego-extract` can reuse it verbatim. |
| DCT-coefficient embedding (survives metadata stripping) | 2 | **out-of-model** | Spiked: needs read/write access to quantized DCT coefficients. `jpeg-decoder`, `zune-jpeg` and `jpeg-encoder` all expose pixels only; `mozjpeg-sys` is a C binding and will not instantiate under wasmi. No wasm-safe pure-Rust path exists. |
| LSB pixel embedding | 3, 4 | **out-of-model here — already built** | `blocks/lsb-embed` covers pixel-LSB hiding (PNG output). Deliberately a different tool: LSB cannot survive JPEG's lossy re-encode, which is why those tools all emit PNG. |
| Hiding several files in one pass | 1 | **out-of-model** | Single payload by design; zip first with `blocks/create-zip`, then hide the zip. |
| Appending to data already hidden in the image | 1 (`--append`) | **out-of-model** | Stateful multi-pass workflow; a second run overwrites rather than accumulates. |
| Interactive passphrase prompt, in-place overwrite, verbose/silent flags | 1, 2 | **out-of-model** | CLI ergonomics of a native binary; a block has no TTY and never mutates a source file. |
| CAPTCHA / access code | 3 | **out-of-model** | Anti-abuse for a hosted service; not applicable. |
| Image-inside-image payload | 4 | **covered** | An image *is* a file — pass it via `payload_url` / `payload_ref`. |

## Defaults chosen (and why)

- `method = "app"` — the payload lives in standard APP9 marker segments inside the JPEG
  header, so the file stays a structurally valid JPEG end to end. `append` (bytes after
  the EOI marker) is the classic CTF-style trick and is offered explicitly because some
  workflows expect it; `comment` uses COM segments for the same reason.
- `compress = true` — both CLI competitors deflate before encrypting; it shrinks text and
  makes the ciphertext less structured. Falls back to the raw bytes automatically when
  deflate does not help (already-compressed payloads such as zip/png/jpeg).
- No password by default — matching the browser tools, where encryption is an opt-in
  toggle; the summary states plainly when the payload is stored unencrypted.

## Worked example carried onto every surface

`gizza tool jpg-stego-embed url=<carrier.jpg> payload_text="meet at 7" password=hunter2`
→ a JPEG that renders identically, with the deflated + AES-256-GCM payload in APP9
segments. Reported in the summary as payload/stored/output bytes and growth percentage.

## Honest limits stated on every surface

- Nothing here survives a **re-encode**: resizing, re-saving, or a social-network upload
  pipeline strips marker segments and trailing bytes. This is inherent to any
  metadata-container approach and is stated in the descriptor text.
- The payload is **hidden, not undetectable**: `strings`, `binwalk`, `exiftool` and our own
  `scan-embedded-files` will spot it. Use `password` when confidentiality actually matters.
- Carrier must be a real JPEG (`FF D8 FF`); PNG/WebP carriers are rejected with an
  actionable message pointing at `lsb-embed`.
