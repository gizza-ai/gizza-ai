# lsb-extract — competitor analysis & improvement snapshot (2026-06-21)

`lsb-extract` recovers a payload hidden in an image's least-significant bits
(LSB steganography) — the inverse of the existing `lsb-embed` tool. Pure Rust
(the `image` crate + a bit-collector in `core`), so it runs on **all** backends
including the chat Service Worker. File-input → text/JSON output → **chat + CLI
only, no page** (the no-page file-input pattern, like `strings` /
`detect-file-type`).

## Surfaces verified

- **Chat / LLM API** — `descriptor()` single-sources the schema; a drift-guard
  unit test (`schema_json_matches_authored_chat_schema`) pins it. Block builds +
  **instantiates** under `wafer build` (`OK gizza-ai/lsb-extract 1488.8 KiB`).
- **CLI** — verified end-to-end against live public images:
  - True cross-tool round trip: `gizza tool lsb-embed url=<live QR PNG>
    message="top secret: the eagle lands at dawn"` → uploaded the stego PNG to a
    public host → `gizza tool lsb-extract url=<stego>` recovered the **exact**
    message in AUTO mode (`header_found: true`, depth auto-detected).
  - Bit-depth auto-detect: an lsb-embed at `bits_per_channel=3` was recovered
    with `bits_per_channel: 3` reported.
  - RAW mode across `channels` (rgb / single channel), `bits_per_channel` 1-8,
    `bit_order` msb/lsb, and `invert` — all return correct byte counts + hex.
  - Validation errors fire (`bits_per_channel must be between 1 and 8`,
    `bit_order must be "msb" or "lsb"`, clean image → "no lsb-embed payload").
- **Page** — none (binary file-input → text report has no page render mode).
- `gizza list` shows the tool; the site generator renders 93 tools (no page
  entry for lsb-extract, as expected).

## Competitors surveyed

| Tool | Channel select | Bit-depth | Bit order | Auto-detect | Output | Local? |
|------|----------------|-----------|-----------|-------------|--------|--------|
| StegOnline (georgeom.net/StegOnline) | R/G/B/A per-bit | all 32 planes | bit-pattern | partial | text/hex/file + **bit-plane viz** | yes (client) |
| zsteg (zed-0xff/zsteg, CLI) | `-c r,g,b,a,combos` | `-b 1-8` | `--lsb/--msb` | **`-a` try-all** + filetype | text/hex/filetype | local CLI |
| Aperi'Solve (aperisolve.com) | via zsteg | via zsteg | via zsteg | multi-tool deep scan | reports + files | **server upload** |
| Futureboy Stegano (futureboy.us/stegano) | n/a (steghide) | n/a | n/a | payload guess | file / MIME | server + password |
| stylesuxx (stylesuxx.github.io/steganography) | none | none | none | none | text only | yes (client) |
| incoherency (incoherency.co.uk/image-steganography) | none | n-LSB slider | none | none | reveals hidden **image** | yes (client) |

**Takeaways:** zsteg + StegOnline are the feature ceiling. zsteg owns the
brute-force matrix (channels × bits × MSB/LSB × scan-order) and filetype
reporting; StegOnline owns bit-plane *visualization*. The simple browser-local
tools offer almost no parameter control. Aperi'Solve is powerful but **uploads
to a server** — a privacy cost gizza avoids by running locally.

## Gaps closed in this pass (in-model, pure Rust)

1. **LSB-first bit order** — added `bit_order: msb|lsb` to RAW mode. MSB-first
   matches `lsb-embed`; LSB-first is the more common convention in other
   encoders (incoherency, many tutorial scripts, half of zsteg's matrix). Nearly
   doubles the messages decodable for trivial cost.
2. **Wider bit depth (1-8)** — RAW mode now reads up to 8 bits/channel (was 1-4),
   matching zsteg's `-b 1-8`. AUTO stays 1-4 (the lsb-embed format).
3. **File-type sniff of binary payloads** — `detected_filetype` surfaces "PNG
   image / ZIP archive / PDF document / gzip data / …" via a magic-byte table
   when the recovered bytes aren't valid UTF-8, matching zsteg's filetype output.
4. **Invert / XOR-0xFF toggle** — `invert: true` in RAW mode undoes a common
   one-byte obfuscation (zsteg's `--invert`).
5. **Hex preview + byte length + truncation cap** already present; kept.

## Gaps deliberately NOT built

- **Bit-plane image visualization** (StegOnline / Aperi'Solve marquee feature) —
  needs an image-output surface; a text/JSON tool can't deliver it. Out of model
  for this tool shape; flag for a future image-returning surface.
- **Reveal a hidden *image* payload** (incoherency) — image output, out of model.
- **Full automatic brute-force "try-all" sweep** ranking candidates — feasible in
  pure Rust but is a heavier, distinct tool; the manual channel/depth/order/
  invert controls already cover the common axes a user would sweep. Left for a
  dedicated `stego-scan`-style tool rather than overloading this one.
- **Pixel scan-order variants** (column-major / transposed, zsteg's `-o`),
  **non-contiguous bit-mask selection**, **multi-tool aggregation** (steghide /
  binwalk), **password / encrypted extraction**, **multi-image batch** — out of
  scope for a single-image → text/JSON call (and the last two need crypto / the
  steghide format).

No competitor copy, branding, or UI text was reused — this is a capability-only
comparison.

## Test summary

`cargo test --workspace` in `blocks/lsb-extract`: **18 tests pass** — 16 core
(auto round-trip at depths 1-4, non-UTF-8 payloads, empty payload, clean-image
error, raw channel/depth scaling, single channel, LSB-first ≠ MSB-first, invert
complements, depth-8, channel/bit-order parsing, filetype signatures, bad-bpc /
empty-input errors) + 2 block (chat-schema drift guard, hex preview).
