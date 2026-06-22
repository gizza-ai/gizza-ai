# pbkdf2-derive — competitor analysis (2026-06-21)

New tool built end-to-end this run, then improved per a parallel competitor study.
All competitor notes are **paraphrased** from public tool behavior — no copy, branding,
or assets were reproduced.

## Competitor landscape (top 6 public PBKDF2 tools)

| Tool | PRF/hash | Salt input | Iterations | Length / output | Extras | Compute |
|------|----------|-----------|------------|------------------|--------|---------|
| 8gwifi.org | SHA-1/256/384/512 | base64; random gen | default 100k, presets 10k/100k/600k | 32 B; hex only | code snippets, KDF comparison, WPA PSK | **server-side** |
| asecuritysite.com | MD5/SHA-1/224/256/384/512 | text | user-set (low teaching defaults) | 4–96 B; hex | URL-param prefill (shareable) | **server-side** |
| devglan.com | SHA-256/512 | hex; auto-gen | configurable (rec ≥310k) | configurable; hex + copy | **verify mode**; weak-param warnings | **server-side** |
| encryptdecrypt.org | SHA-1/256/512 | hex; empty=auto | presets 1k–1M | 16/32/48/64 B + custom; hex, uppercase toggle | shows salt/time | **client-side** |
| tiny-online.tools | SHA-1/256/384/512 | hex; random gen | default 10k, presets 100k/310k/600k | 128/256/512-bit; **hex + base64** | — | **client-side (Web Crypto)** |
| lddgo.net | SHA-1/224/256/384/512 + SHA-3/SM3 | **text/hex/base64** | 1–65,536 (low cap) | 8-bit multiples; hex + base64 | **verify**; download | location unstated |

(Honorable mentions: codertools.net, hashing.tools, dcode.fr, toolsana.com — thinner offerings.)

## Gap diff vs gizza pbkdf2-derive

**Already at/above parity (built this run):**
- **All three salt encodings** (text/hex/base64) — the single biggest differentiator; only lddgo matches it, and lddgo isn't confirmed client-side.
- **hex + base64 output** — most competitors are hex-only.
- **Iteration count** default 100k, capped at 10,000,000 — beats lddgo's 65,536 cap; OWASP 600k guidance surfaced in the copy.
- **Modern hash set** SHA-1 (flagged legacy)/256/512; MD5 deliberately omitted.
- **Genuine browser-local privacy** (pure Rust→wasm, no network) — beats the 4 server-side competitors (8gwifi/asecuritysite/devglan + lddgo's likely-server compute); the copy says so.
- **Deterministic + interoperable**, verified against RFC 6070 (SHA-1) and RFC 7914 (SHA-256) vectors in unit tests.
- **Three surfaces**: chat/LLM schema, CLI, query-param deep-linkable page.

**Closed this run (gap #6 from the study — verify mode):**
- Added **`mode=verify`**: paste an `expected` key (hex or base64, auto-detected) and the tool checks whether the password+params reproduce it (constant-time byte compare). The expected key's length implies the derived length. Only devglan and lddgo had verify, neither in a trustworthy client-side way — gizza now does it locally. Wired through chat, CLI, and the page (`#in-mode`/`#in-expected`).

**Considered, deliberately NOT built (out-of-model or low-value):**
- **Random salt generator** — PBKDF2 is deterministic and the page recomputes on input; a random salt would change on reload, breaking reproducibility. Sibling tools that need random salts (argon2-hash, encrypt-file, text-encrypt) already provide one. (Out of the page's recompute-on-input model.)
- **SHA-384 / SHA-224 / SHA-3 / SM3 PRFs** — the common interop set is SHA-1/256/512; trivially extensible later if requested. Listed, not built to keep the surface focused.
- **Password-storage / PHC-style encoded string** (`$pbkdf2-sha256$…`) — a storage/verification shape distinct from a KDF calculator; argon2-hash already covers PHC hash+verify.
- **Code-snippet generator** (Python/Node/OpenSSL) — nice SEO magnet (8gwifi has it) but a presentation feature, not a compute capability; out of scope for this pass.
- **Server-side derivation / WPA PSK / accounts / hosted history** — outside gizza's browser-local model (server-side compute is a weakness to attack, not copy). A *stateless* URL-param share link is already in-model and supported (query-param prefill).

## Verification (this run)

- **Unit tests:** 14 core tests — RFC 6070 (SHA-1 c=1/2/4096), RFC 7914 (SHA-256 c=1), hex/base64 salt decode, base64 output round-trip, **verify hex/base64 round-trip + verify errors**, error cases. All green.
- **Drift-guard:** `schema_json_matches_authored_chat_schema` green after adding `mode`/`expected`.
- **Chat block:** `wafer build` validates `target/block.wasm` instantiates (355 KiB).
- **CLI:** `gizza tool pbkdf2-derive …` — reproduces the RFC 6070 vector, defaults apply, base64 output, `mode=verify` returns `{"match":true}` / `{"match":false}`, and errors on bad hash / missing expected (exit 1).
- **Page:** Playwright — derive form (RFC 6070 vector), verify form (match), and query-param deep link. All 3 green.
