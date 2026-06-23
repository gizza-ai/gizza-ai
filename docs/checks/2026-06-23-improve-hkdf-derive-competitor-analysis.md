# hkdf-derive — competitor analysis & improvement check (2026-06-23)

Tool: `gizza-ai/hkdf-derive` — HKDF (HMAC-based extract-and-expand key derivation,
RFC 5869). Surfaces: chat skill block, CLI (`gizza tool hkdf-derive`), page
(`/tools/hkdf-derive/`). Pure-Rust (`hkdf` + `hmac` + `sha1`/`sha2`) → runs on all
backends including the chat Service Worker and fully in-browser on the page.

## What this tool does

- **derive** mode (default): full HKDF extract-then-expand → `length` bytes of output
  key material (OKM).
- **extract** mode: returns only the intermediate pseudorandom key (PRK =
  `HMAC-Hash(salt, IKM)`), useful for inspecting/splitting the two HKDF stages.
- Inputs: IKM, optional salt, optional info/context label — each independently decodable
  as **utf8 / hex / base64**. Hash ∈ {sha1, sha256 (default), sha384, sha512}. Output
  length up to 255×HashLen. Output encoding hex (default) or base64.
- Deterministic; secret never leaves the device.

## Correctness — RFC 5869 test vectors

The core is unit-tested against the canonical **RFC 5869 Appendix A** vectors (verified by
fetching the RFC, not from memory):

- A.1 (SHA-256, salt+info, L=42): PRK and OKM exact match.
- A.3 (SHA-256, empty salt+info, L=42): PRK and OKM exact match.
- A.4 (SHA-1, salt+info, L=42): PRK and OKM exact match.
- A.6 (SHA-1, empty salt+info, L=42): OKM exact match.

These same vectors are re-asserted on the **CLI** and **page** surfaces (Playwright derives
A.1 OKM, extracts the A.1 PRK, and deep-links A.3 via query params). Output is therefore
interoperable with OpenSSL, Python `cryptography`/`hashlib`, Node `crypto.hkdf`, Go
`x/crypto/hkdf`, and WebCrypto `deriveBits`.

## Competitors surveyed

- **codertools.net — HKDF tool** (https://www.codertools.net/tools/hkdf.php): browser-based,
  RFC 5869 extract-expand, SHA-256/384/512, IKM/salt/info/length inputs.
- **patrickfav/hkdf** (Java library, https://github.com/patrickfav/hkdf): full RFC 5869,
  extract-then-expand, NIST 800-56C Rev.1 compatible — reference for the API surface, not a
  web tool.
- Various language stdlib HKDFs (Python `cryptography.hazmat HKDF`, Node `crypto.hkdf`,
  Go `golang.org/x/crypto/hkdf`) — the interop targets.

## Gap diff (fit-to-model)

| Capability | Competitor (codertools) | gizza hkdf-derive |
|---|---|---|
| Browser-only / no upload | yes | yes (wasm, plus chat + CLI) |
| RFC 5869 extract+expand | yes | yes |
| SHA-256 / 384 / 512 | yes | yes |
| SHA-1 (legacy interop) | not surfaced | **yes** |
| Separate **extract-only** (PRK) mode | no | **yes** |
| Per-field utf8 / hex / base64 decoding of IKM/salt/info | partial | **yes (all three, independently)** |
| hex **or base64** output | hex-centric | **yes (both)** |
| Length limit guard (255×HashLen) | unclear | **yes, explicit error** |
| RFC 5869 test-vector backing | unstated | **yes (4 vectors, asserted on 3 surfaces)** |
| LLM/chat + scriptable CLI surface | no | **yes** |

gizza is a strict superset of the surveyed web competitor's in-model feature set, and adds
extract-only mode, base64 I/O, SHA-1 interop, and explicit input/length validation.

## Out-of-model features (intentionally not built)

- **Argon2 / scrypt / PBKDF2 cross-links** — these are separate gizza tools
  (`argon2-hash`, `scrypt-derive`, `pbkdf2-derive`); HKDF is correctly scoped to
  high-entropy IKM and documents (in copy) that passwords belong to those KDFs.
- **Binary file output / key-file download** — HKDF output is short key material; text
  hex/base64 is the right surface, consistent with the sibling KDF tools.
- No competitor copy, branding, or trademarks were copied.

## Verification run (2026-06-23)

- `cargo test --workspace` (block): 11 core vectors + schema drift-guard — **pass**.
- `wafer build` (chat block.wasm): validates + instantiates — **OK (367.7 KiB)**.
- `wasm-pack build .../web`: **built**.
- `cargo install --path cli` + generator: page rendered (241 tools) — **OK**.
- CLI: A.1 derive OKM + A.1 extract PRK + utf8 default — **match**.
- Playwright `tool-page-hkdf-derive.spec.ts`: 3/3 **pass** (derive, extract, query-param).

No in-model gaps remain.
