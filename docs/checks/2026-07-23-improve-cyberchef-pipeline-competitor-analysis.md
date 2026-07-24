# cyberchef-pipeline — competitor analysis (2026-07-23)

Tool function: chain multiple byte-level decode/transform steps (e.g. `from-base64` →
`gunzip` → `xor`) into a single client-side "recipe", applied top-to-bottom to a text/byte
buffer. A focused, page-friendly take on the CyberChef recipe model.

All findings paraphrased from public documentation/behavior. No competitor copy, branding, or
trademarks are reproduced. "CyberChef" is referenced only as the well-known category reference;
our page copy stays generic/brand-free.

## Competitors scanned

1. **CyberChef (GCHQ)** — the reference "cyber swiss army knife". Drag operations into a recipe
   list; runs 100% client-side (no upload); 300+ operations spanning Base64/hex/URL encoding,
   gzip/zlib/raw compression, XOR/bit-ops/classical & modern crypto, hashes, regex extraction.
   Recipes are shareable via URL. Also ships "Magic" auto-detection of nested encodings.
2. **KeyDecryptor / "single-purpose" decode sites** — the opposite model: one fixed tool per
   task (a Base64 page, a gunzip page, an XOR page). Paste → result, no recipe building. Simpler
   but can't express a multi-layer chain in one shot.
3. **Community CyberChef recipe collections** (mattnotmax, binary-cipher on GitHub) — document the
   *table-stakes decode chains* analysts actually reach for: From Base64 → Gunzip → XOR is the
   canonical malware-string unwrap; hexdump → decompress; base64 → raw-inflate.

## Table-stakes params / operations (each mapped to a decision)

| Capability | Decision | Notes |
|---|---|---|
| Ordered recipe, one op per step | **In-model** | `recipe` param, one operation per line, top→bottom |
| From/To Base64 | **In-model** | `from-base64` / `to-base64` (tolerant: whitespace + URL-safe + slack padding) |
| From/To Hex | **In-model** | `from-hex` (ignores whitespace/colons/commas/`0x`) / `to-hex` |
| URL percent decode/encode | **In-model** | `url-decode` / `url-encode` |
| ROT13 | **In-model** | `rot13` |
| Gunzip / Gzip | **In-model** | `gunzip` / `gzip` (flate2) |
| Zlib inflate/deflate | **In-model** | `zlib-inflate` / `zlib-deflate` |
| Raw DEFLATE inflate/deflate | **In-model** | `raw-inflate` / `raw-deflate` |
| XOR with key (Hex/UTF8/Base64/Decimal key formats) | **In-model** | `xor KEY [hex\|utf8\|base64\|decimal]`, repeating key |
| Byte arithmetic ADD / SUB | **In-model** | `add N` / `sub N` (mod 256) |
| Bitwise NOT | **In-model** | `not` |
| Reverse bytes | **In-model** | `reverse` |
| Upper / lower case | **In-model** | `upper` / `lower` (ASCII) |
| Choose how binary output is rendered | **In-model** | `output_format` = auto / utf8 / hex / base64 |
| Client-side / no upload | **In-model (native)** | runs as WASM in the browser — data never leaves the page |
| Preset recipe chips | **In-model** | `[[example]]` chips prefill common chains |
| Comments / blank lines in recipe | **In-model** | `#…` and blank lines ignored |

## Out-of-model (listed, not built)

- **Drag-and-drop visual recipe builder** — our recipe is a text DSL (one op per line); the same
  chaining power, no drag UI. This is a UX shape, not a capability gap.
- **"Magic" auto-detection of nested encodings** — already covered by the sibling
  `encoded-payload-decoder` block (auto-finds/unwraps embedded base64/hex/gzip/zlib). Not
  duplicated here; this tool is the *explicit* user-driven recipe.
- **Full modern-crypto operations** (AES/DES/Blowfish/RC4/…) — gizza already ships dedicated
  cipher blocks (`aes-cipher`, `des-cipher`, `blowfish-cipher`, `rc4-cipher`, `chacha20-cipher`,
  `xor-cipher`, …). Keeping those out of the recipe avoids re-implementing keyed crypto; XOR/bit
  math (the classic obfuscation layer) is included because it's the common "last layer" in decode
  chains and needs no key management.
- **Hashes / checksums as recipe steps** — covered by `hash-all`, `hash-text`, `sha256-hash`, etc.
- **Recipe-as-URL sharing in CyberChef's own JSON format** — our page already deep-links every
  param via `?input=…&recipe=…&output_format=…`, which is the equivalent shareable link.
- **300+ operation catalog** — we ship the ~20 operations that make up the common decode/transform
  chains; the long tail stays in the dedicated single-purpose blocks.

## Sibling-dup check (why this is NOT a duplicate)

- `multi-encoder` — applies exactly ONE encoding in ONE direction (no chaining).
- `encoded-payload-decoder` — AUTO-detects/unwraps embedded blobs (the "Magic" shape), not a
  user-authored ordered recipe.
- `text-pipeline-playground` — chains LINE-oriented text ops (grep/sort/replace/head), not
  byte-level decode/compress/XOR.

This tool is the missing "explicit, ordered, byte-level decode/transform recipe" surface.
