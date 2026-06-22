# keccak-hash — competitor analysis & improvement snapshot (2026-06-22)

## Tool

`keccak-hash` computes the **original Keccak** digest (Keccak-256 / Keccak-512)
of input text. This is the legacy-padding Keccak (`0x01` multi-rate padding) —
the hash Ethereum and the EVM use — which is **distinct from FIPS-202 SHA-3**
(`0x06` padding). The existing `hash-text` tool only offers SHA3-256/SHA3-512,
so Keccak was a genuine, un-covered gap (NOT a duplicate).

Surfaces: chat skill block, CLI (`gizza tool keccak-hash`), standalone page
(`/tools/keccak-hash/`).

## Correctness anchors (verified vectors)

- `keccak256("")` = `c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470`
  (the well-known Ethereum "empty" hash).
- `keccak256("abc")` = `4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45`
  — note this differs from `sha3-256("abc")`
  (`3a985da7…431532`), proving the padding distinction.
- `keccak512("")` = `0eab42de…3670680e`.
- Cross-checked in CLI: `keccak256("abc")`, `keccak512("abc")`, empty, and a
  bad-algorithm error path all behave correctly.

## Top competitors surveyed

1. **emn178 online-tools (keccak-256/512)** — the most-linked standalone Keccak
   pages. Offer Keccak-224/256/384/512, hex/base64 in/out, file input, live
   recompute. Client-side JS.
2. **CyberChef "Keccak" operation** — variant select (224/256/288/384/512),
   chainable recipe, hex/raw I/O. Powerful but heavy/general-purpose.
3. **simplycyber / dencode-style hash pages** — multi-algorithm hashers that
   include Keccak alongside SHA-3; UTF-8/hex input, hex output.
4. **ethers.js / web3 `keccak256` (developer libraries)** — the canonical
   reference for the Ethereum use-case; operate on `0x`-prefixed hex bytes and
   return `0x`-prefixed hex. Establishes the expectation that hex input should
   tolerate a `0x` prefix.
5. **MD5/SHA "all hashes" sites (e.g. those bundling Keccak)** — broad menus,
   copy-to-clipboard, sometimes ads/sign-up.

## Capability diff & gaps

| Capability | Competitors | keccak-hash (this tool) |
| --- | --- | --- |
| Keccak-256 | all | ✅ (default) |
| Keccak-512 | most | ✅ |
| Keccak-224 / 288 / 384 | some (emn178, CyberChef) | ❌ out of common need — see below |
| Hex input | most | ✅ (also tolerates `0x` prefix, matching ethers/web3) |
| Base64 input | some | ✅ |
| Hex output | all | ✅ (lowercase, + uppercase toggle) |
| Base64 output | some | ✅ |
| UTF-8 text input | all | ✅ (default) |
| Runs client-side / no upload | emn178, CyberChef | ✅ (WASM, in-browser; also offline-capable chat + CLI) |
| Distinguishes Keccak vs SHA-3 clearly | rarely | ✅ explicit copy + tool cross-link |
| Deep-link query params | rare | ✅ (`?text=…&algorithm=…`) |

### Gaps closed this build

- **`0x`-prefix tolerance on hex input** — matches the dominant developer
  workflow (ethers/web3 pass `0x…`). Added + unit-tested + page-tested.
- **Keccak-vs-SHA-3 clarity** — the single most common point of confusion in the
  space; the page, schema descriptions, and tool summary all spell out the
  padding difference and cross-link to `hash-text` for SHA-3. This is a real
  UX/copy edge over competitors that silently label Keccak as "SHA3".
- **Base64 in/out + uppercase** — parity with the broadest competitors.

### Gaps intentionally NOT closed (scoped out)

- **Keccak-224 / 288 / 384** — the RustCrypto `sha3` crate exposes Keccak-224
  and Keccak-384 (and Keccak-256-full/512-full), but real-world demand is almost
  entirely Keccak-256 (Ethereum) with Keccak-512 as the round-number companion.
  Keeping the menu to those two keeps the tool focused and the LLM schema small;
  the two extra sizes can be added later if requested. (No model/technical
  blocker — purely a scope choice.)
- **File input** — `keccak-hash` is a text tool. Hashing a whole binary file is
  the job of the existing `file-hash` tool; a Keccak-over-file variant would be
  a separate file-input block, out of scope here.
- **No competitor copy/branding/trademarks were reused.**

## Verification matrix (all green)

- `cargo test --workspace` — 15 core vector/edge tests + 1 schema drift-guard
  test pass.
- `wafer build` — block.wasm validates & instantiates (pure RustCrypto, runs in
  the chat Service Worker).
- `wasm-pack build` — page wasm built.
- CLI — `gizza tool keccak-hash` returns correct digests; bad-algorithm errors.
- Playwright (`tool-page-keccak-hash.spec.ts`) — 5/5 pass (default hash,
  keccak-512, uppercase, hex+0x input, query-param deep-link).
