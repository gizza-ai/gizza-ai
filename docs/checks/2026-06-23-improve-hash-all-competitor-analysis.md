# hash-all — competitor analysis (2026-06-23)

Tool: `blocks/hash-all` — pure-compute tool. Takes input text and computes EVERY
common digest at once (CRC-32, MD5, SHA-1, SHA-224/256/384/512, SHA3-256/512,
RIPEMD-160, BLAKE2b-512, BLAKE2s-256, BLAKE3, Whirlpool), returning them as one
aligned `label  value` table. The input can be read as UTF-8 text or decoded
first from hex / base64, and every digest can be rendered as lowercase/uppercase
hex or base64. Three surfaces (chat skill, CLI, standalone page) single-sourced
from `descriptor()`. All hashers are pure-Rust (RustCrypto + blake3) so it runs
on every backend including the chat Service Worker.

All notes below are **paraphrased** — no competitor copy, branding, or assets
were reproduced.

## Competitors surveyed

1. **CyberChef "To Hash" / hashing recipes (GCHQ, gchq.github.io/CyberChef)** —
   the reference "compute many hashes" tool. Has a single "Analyse hash" /
   per-algorithm operations and can chain several hash ops; supports a very wide
   algorithm list (MD2/4/5, SHA0/1/2/3, RIPEMD, Whirlpool, BLAKE2, Keccak, CRC).
   Strength: enormous breadth + chaining. Weakness: you assemble a recipe
   yourself; there is no single "show me ALL digests of this input" button.
2. **MD5Hashing.net "All hashes" / "Hash generator"** — paste text, get many
   algorithms at once (MD5, SHA-1/256/384/512, RIPEMD, Whirlpool, CRC-32, etc.)
   in a list. Closest feature match to ours. Ad-supported; computes server-side
   (input leaves the browser).
3. **Online-Convert / "hash generator" tools (various: emn178, hash.online-convert.com)** —
   emn178's online hashers are client-side and per-algorithm (one page each for
   MD5/SHA/SHA-3/RIPEMD); online-convert offers a multi-algorithm form but runs
   server-side.
4. **xorbin / browserling "all hashes" toys** — paste text, get a fixed handful
   of digests (MD5/SHA-1/SHA-256). Narrow algorithm set; usually hex-only.
5. **`hashlib` / openssl `dgst` / `rhash` CLI** — `rhash` is the closest CLI
   analogue (computes many checksums of a file/string at once: CRC32, MD5, SHA,
   RIPEMD, Whirlpool, BLAKE2/3). Developer-only, not a browser surface, and
   string vs file modes differ per tool.

## Capability diff (theirs → ours)

| Capability | Competitors | Ours | Status |
| --- | --- | --- | --- |
| Compute ALL digests in one action | MD5Hashing, rhash, online-convert | yes (one labeled table) | covered — the core value prop |
| CRC-32 | CyberChef, MD5Hashing, rhash | yes (IEEE/zip variant) | covered |
| MD5 / SHA-1 | all | yes | covered |
| SHA-2 (224/256/384/512) | all | yes | covered |
| SHA-3 (256/512) | CyberChef, emn178, rhash | yes | covered |
| RIPEMD-160 | CyberChef, MD5Hashing, rhash | yes | covered |
| BLAKE2b / BLAKE2s | CyberChef, rhash | yes | covered |
| BLAKE3 | rhash, CyberChef (recent) | yes | covered — many "all hash" pages still lack it |
| Whirlpool | CyberChef, MD5Hashing, rhash | yes | covered |
| Decode input from hex / base64 before hashing | CyberChef (via recipe) | yes (input_encoding) | covered — most one-click "all hash" pages hash the literal text only |
| Base64 digest output | CyberChef | yes (output_format) | covered — competitors are usually hex-only |
| Uppercase hex toggle | some | yes | covered |
| Runs fully client-side / private | CyberChef, emn178 (yes); MD5Hashing, online-convert (no — server-side) | yes (WASM, nothing uploaded) | covered — a differentiator vs the server-side multi-hash sites |
| API / CLI surface | rhash (CLI only) | yes (chat skill + CLI + page) | covered |

## Gaps considered and decisions

- **More exotic algorithms (MD2/MD4, SHA-0, Keccak-256 (pre-NIST), Tiger,
  GOST, CRC-16/CRC-64, SM3, Streebog, xxHash)** — CyberChef/rhash cover a longer
  tail. Deliberately **not** built: the backlog description names a specific
  modern set, and a 14-row table already covers the digests people actually
  request. MD5/SHA-1 are kept (and labeled legacy in the copy) because checksum
  comparisons still need them. Keccak-256 (the Ethereum pre-standard variant) is
  already its own `keccak-hash` tool, so it is intentionally not duplicated here.
- **HMAC / keyed hashing** — out of scope; that is the existing `hmac-generate`
  tool. hash-all is unkeyed digests only.
- **File input** — hashing a whole uploaded file is the existing `file-hash`
  tool (MD5/SHA-1/SHA-256/SHA-512/CRC-32). hash-all is the TEXT-input, wide-algo
  counterpart; the copy cross-links both directions (single algo → `hash-text`,
  file → `file-hash`) so the family stays discoverable without overlap.
- **Comparison / "does this match?" mode** — entering an expected digest and
  highlighting which algorithm matches is a nice UX idea but adds a second IO
  shape; the page's recompute-on-input model already lets a user eyeball the
  table against a target. Left out to keep one clean output.

## Result

No in-model capability, copy, or UX gap remained open after the survey. The tool
matches the best "all hashes at once" competitors on algorithm breadth, adds the
hex/base64 input-decoding and base64/uppercase output options most one-click
pages lack, and is fully client-side (private) unlike the server-side
multi-hash sites. Verified on all three surfaces: chat block (`wafer build` OK),
CLI (`gizza tool hash-all` — known `abc` vectors confirmed for every algorithm,
incl. the ISO/IEC 10118-3 Whirlpool vector), and the standalone page (Playwright,
2 passing specs covering the full table + the uppercase toggle).
