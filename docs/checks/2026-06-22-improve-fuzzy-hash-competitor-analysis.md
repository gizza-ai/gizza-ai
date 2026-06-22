# fuzzy-hash — competitor analysis (2026-06-22)

**Tool:** `fuzzy-hash` — compute an SSDEEP context-triggered piecewise hash
(CTPH) of a file, returning `blocksize:sig1:sig2`, with an optional 0–100
similarity score against another ssdeep hash.

**Surfaces:** chat (LLM tool / `wafer` block) + CLI. No standalone page — a
file→fingerprint report fits neither the pure-text nor the ffmpeg-media page
shape, matching the existing no-page file-input tools (`file-hash`,
`detect-file-type`). Pure Rust, so it runs on every backend including the chat
Service Worker.

## What it does

- Reads a file via `url` (HTTP/HTTPS fetch) or `ref` (attachment id from a prior
  tool call), up to 64 MiB.
- Computes the ssdeep CTPH fuzzy hash: a 7-byte rolling hash fires reset
  triggers at a content-derived block size; two FNV-style piecewise sum hashes
  (at `blocksize` and `2*blocksize`) emit base64 signature chars. The block size
  auto-scales with file size (`3 * 2^k`, smallest that keeps the signature under
  64 chars).
- Optional `compare`: when given an existing ssdeep hash, returns a 0–100
  similarity score (edit-distance ratio over the signatures, with the standard
  block-size cap and 4+-run collapse) so callers can do near-duplicate /
  malware-family matching without a second round-trip.

Output JSON: `{ ssdeep, bytes, filename?, similarity?, compared_to? }`.

## Competitors reviewed

1. **ssdeep (Kornblum, the reference CLI / libfuzzy)** — the canonical
   implementation. Computes the CTPH hash and compares with `-m` against a list.
   Our `blocksize:sig1:sig2` format and the rolling-hash / dual-sum structure
   follow it. *Gap closed:* in-tool `compare` mirrors `ssdeep -k/-m` pairwise
   scoring. *Out of scope:* a persistent match-set / `.ssdeep` file database
   (multi-file corpora) — gizza tools are single-call, stateless.
2. **VirusTotal / threat-intel hash lookups** — surface ssdeep alongside
   MD5/SHA for malware triage. We emit the same ssdeep string those services
   index on, and `file-hash` already covers the crypto digests, so the pair
   covers a triage workflow. *Out of scope:* the actual reputation lookup
   (that's a network/API tool, not a hash function).
3. **TLSH / sdhash (other similarity-hash schemes)** — alternative fuzzy hashes.
   Deliberately *not* built: ssdeep is the most widely interoperable format
   (VirusTotal, YARA `ssdeep` module, most IR tooling), and shipping one
   well-known format beats several incompatible ones. Noted as a possible future
   sibling tool, not a gap in this one.
4. **Online "ssdeep hash generator" web tools** — paste/upload a file, get the
   hash, optionally compare two hashes. We match: hash via url/ref, and the
   `compare` param does the two-hash similarity in one call. They typically lack
   an LLM/agent surface and a scriptable CLI — we have both.
5. **Python `ssdeep` / `python-tlsh` libraries** — give `hash()` + `compare()`.
   Our core exposes the same two primitives (`fuzzy_hash`, `compare`) and the
   block surfaces both (hash always, compare via the optional param).

## Capability gaps + decisions

| Capability | Status |
| --- | --- |
| Compute ssdeep CTPH of a file | Built (core, chat, CLI) |
| `blocksize:sig1:sig2` reference format | Built |
| Pairwise similarity score 0–100 | Built via optional `compare` param |
| Block-size auto-scaling with file size | Built |
| 4+-run collapse + block-size score cap | Built (reference scoring behavior) |
| Persistent match-set / corpus DB | Out of model (stateless single-call tool) |
| TLSH / sdhash alternative schemes | Deferred (separate tool if needed) |
| Reputation / VirusTotal lookup | Out of model (network/API tool) |
| Standalone page | N/A (no-page file-input pattern) |

## Copy / UX

- Skill + manifest description explains the *similarity* property (the reason to
  use a fuzzy hash over MD5/SHA) and names concrete use cases: malware triage,
  near-duplicate detection, spam clustering — without copying any competitor
  copy or branding.
- `compare` is documented as optional with its exact format, and the response
  echoes `compared_to` so an agent can show what was matched.

## Verification

- `cargo test --workspace` — 8 tests (7 core: determinism, identical→100,
  similar→high, unrelated→low, garbage→0, block-size growth, empty parses; 1
  block: chat-schema drift guard). All pass.
- `wafer build` — chat `block.wasm` validates (instantiates clean in
  wasm32-wasip1).
- CLI — `gizza tool fuzzy-hash url=<zip>` returns a hash; with `compare=<same>`
  returns `similarity: 100`.
- `gizza list` shows the tool with its description.
- No page surface (stated, not claimed).
