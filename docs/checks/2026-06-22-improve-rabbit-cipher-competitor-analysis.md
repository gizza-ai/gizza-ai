# rabbit-cipher — competitor analysis (2026-06-22)

Tool: **rabbit-cipher** — encrypt/decrypt with the Rabbit stream cipher (RFC 4503 /
eSTREAM), 128-bit key + optional 64-bit IV. Pure-Rust, runs on all three surfaces
(chat block, CLI, in-browser page).

## Surfaces verified (Phase 1)

- **Chat block:** `wafer build` → `OK gizza-ai/rabbit-cipher v0.1.0 (316.0 KiB)` —
  the block.wasm instantiates and validates in the wafer runtime.
- **CLI:** `gizza tool rabbit-cipher …` — encrypt, decrypt round-trip, IV path
  round-trip, and the not-16-bytes key error all behave correctly (exit non-zero on
  error).
- **Page:** `/tools/rabbit-cipher/` — 3 Playwright tests pass (encrypt→decrypt
  round-trip; a known IV ciphertext vector; bad-key-length error surfacing).
- **Correctness:** 9 core unit tests pass, including **all RFC 4503 Appendix A.1
  (no-IV) and A.2 (IV-setup) keystream test vectors** for the zero key, the
  non-zero key, and three IV cases. This is the strongest possible interop guarantee.

## Top competitors surveyed

1. **Browserling — Rabbit Decrypt/Encrypt** (browserling.com/tools/rabbit-decrypt) —
   single "password" field, paste text, press button. No IV input, no encoding
   choice, no standards reference. Built on CryptoJS (passphrase-derived key).
2. **FreeCodeFormat — Rabbit Encrypt/Decrypt** (freecodeformat.com/rabbit-encrypt.php)
   — "Rabbit Secret Key" field, text or file input, encrypt/decrypt buttons.
   Mentions "128-bit key + 64-bit IV" in copy but exposes only a passphrase, no IV
   field, no encoding selector. CryptoJS-style.
3. **GC Wizard — Rabbit cipher** (blog.gcwizard.net) — educational app; key + IV,
   focused on the algorithm walkthrough; part of a mobile/desktop multitool, not a
   web utility with hex/base64 I/O.
4. **CryptoJS `CryptoJS.Rabbit`** (the library behind most "online Rabbit" tools) —
   takes a *passphrase*, derives the key+IV via the OpenSSL `EVP_BytesToKey` KDF, and
   emits an OpenSSL-salted base64 blob. **Not byte-for-byte interoperable** with the
   raw RFC 4503 algorithm unless you pass a 128-bit WordArray key + 64-bit IV
   explicitly.
5. **Language libraries** (Go `rabbitio`, Lua `luarabbit`, Python PyECC `Rabbit.py`) —
   raw RFC-4503 key/IV APIs, the correct reference behavior, but no UI.

## Gap diff + ranking (fit-to-model)

| Capability | Competitors | rabbit-cipher | Verdict |
|---|---|---|---|
| Raw RFC-4503 key + IV (spec-interoperable) | Mostly NO (CryptoJS passphrase blobs) | **YES** | Closed — our differentiator |
| RFC test-vector–validated implementation | Not advertised | **YES (A.1 + A.2)** | Closed |
| Explicit IV field | Mostly missing | **YES (optional)** | Closed |
| Encoding choice (hex / base64) | Rare | **YES** | Closed |
| Text vs encoded key/IV | Rare | **YES (key_format)** | Closed |
| Documented byte order (MSB-first) | None | **YES** (page + descriptor) | Closed |
| Runs offline / nothing uploaded | Some | **YES (wasm, local)** | Closed |
| CLI + chat + page | None | **YES** | Closed |
| File (binary) input | FreeCodeFormat yes | text-only | **Out of model** — the pure-text page input takes UTF-8; binary-file encrypt would need an `AssetKind` file input (see SKILL.md). Not built. |
| CryptoJS-passphrase compatibility mode | Yes (default there) | NO | **Deliberately not built** — that is a non-standard OpenSSL KDF blob, not the Rabbit spec; would mislead on interop. Documented the distinction instead. |

## Copy / UX / visual

- Field labels state the exact byte sizes (16-byte key, 8-byte IV) and the accepted
  encodings per format, so users don't guess.
- `content.md` explains symmetry, IV reuse safety, MSB-first byte order, and the
  RFC-4503 provenance — more technical guidance than any surveyed competitor.
- No competitor copy, branding, or trademarks were copied.

## Conclusion

All in-model capability/copy/UX gaps are closed; rabbit-cipher exceeds the surveyed
web tools on standards-correctness (RFC-validated), IV/encoding control, and
multi-surface availability. The two unbuilt items (binary-file input, CryptoJS
passphrase mode) are out of model / deliberately omitted and recorded above.
