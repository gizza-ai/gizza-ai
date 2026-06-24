# chacha20-cipher — competitor analysis (2026-06-22)

Tool: `blocks/chacha20-cipher` — encrypt/decrypt with the ChaCha20 stream cipher
and the ChaCha20-Poly1305 AEAD construction (RFC 8439). Pure Rust, runs on all
three surfaces (chat block, CLI, in-browser page). Verified: 11 core unit tests
(incl. all three RFC 8439 §2.4.2 / §2.5.2 / §2.8.2 test vectors + a drift-guard),
`wafer build` (block.wasm instantiates), CLI stream+aead round-trips and an AEAD
auth-failure case, and 5 Playwright page tests.

## Top competitors surveyed

1. **CyberChef** (gchq.github.io/CyberChef) — "ChaCha" and "ChaCha20-Poly1305
   Encrypt/Decrypt" operations. Configurable key/nonce input radix (hex/UTF8/
   Base64/Latin1), 8/12-byte nonce + rounds (8/12/20) for raw ChaCha; AEAD op
   takes key, nonce, AAD and produces ciphertext + tag.
2. **devglan / various "online ChaCha20 encrypt" pages** — key + nonce + plaintext
   in, hex/base64 out; mostly raw ChaCha20, often without AEAD/tag handling.
3. **cryptii / cyberchef-style chains** — composable but raw-stream focused; nonce
   handling and tag concatenation left to the user.
4. **Language/library playgrounds** (e.g. libsodium `crypto_aead_chacha20poly1305_ietf`,
   Python `cryptography` `ChaCha20Poly1305`, Go `golang.org/x/crypto/chacha20poly1305`)
   — the reference behaviour we match: 32-byte key, 12-byte IETF nonce, 16-byte
   tag appended to the ciphertext, AAD authenticated-not-encrypted.
5. **emn178 / online crypto utilities** — single-mode raw stream ciphers, hex I/O.

## Capability diff (vs. our tool)

| Capability | Competitors | chacha20-cipher (ours) |
|---|---|---|
| Raw ChaCha20 stream (encrypt = decrypt) | yes (CyberChef, devglan) | yes (`mode=stream`) |
| ChaCha20-Poly1305 AEAD | yes (CyberChef, libs) | yes (`mode=aead`, 16-byte tag appended) |
| Associated data (AAD) | CyberChef AEAD only | yes (`aad`, authenticated-not-encrypted) |
| Tamper / auth-failure detection | yes (libs) | yes (decrypt verifies tag, errors on mismatch) |
| IETF 96-bit nonce + 32-bit counter (RFC 8439) | yes | yes |
| Text vs. encoded (hex/base64) key & nonce | radix selector (CyberChef) | `key_format=text\|encoded` + `format=hex\|base64` |
| Adjustable initial block counter | CyberChef partial | yes (`counter`, stream mode) |
| Runs fully client-side / no upload | CyberChef yes; some sites server-side | yes (WASM in-browser; chat + CLI too) |
| RFC 8439 test-vector verified | implicit | yes — explicit §2.4.2/§2.5.2/§2.8.2 vectors in tests |

## Gaps considered and decisions

- **Configurable rounds (ChaCha8/ChaCha12):** CyberChef exposes 8/12/20 rounds.
  RFC 8439 standardises 20 rounds and reduced-round variants are non-standard and
  rarely needed; **not built** (kept to the standard ChaCha20/20). In-scope to add
  later as an enum if demand appears, but it would dilute the "RFC 8439" framing.
- **Original DJB 64-bit-nonce ChaCha20 (8-byte nonce / 64-bit counter):** we
  implement the IETF/RFC 8439 variant (12-byte nonce, 32-bit counter), matching
  TLS/libsodium and the AEAD spec. Supporting the legacy 8-byte-nonce variant too
  would be a separate mode; the existing `salsa20-cipher` already covers the
  DJB-era 8-byte-nonce design family, so **not built** to avoid overlap.
- **XChaCha20 / XChaCha20-Poly1305 (24-byte nonce):** a useful extension (HChaCha20
  subkey derivation). Out of the immediate RFC 8439 scope; **noted, not built** —
  a candidate for a follow-up tool rather than another mode here.
- **File input:** ours is text-in/text-out like the sibling cipher tools
  (`salsa20-cipher`, `rc4-cipher`). Binary file AEAD would need an `AssetKind`
  file-input page surface that doesn't exist yet; **out of model**, consistent
  with the existing cipher tools.
- **Password-based key derivation:** intentionally absent (raw cipher) — we point
  users at `aes-cipher` / `text-encrypt` for password-based file encryption, same
  as `salsa20-cipher`.

## Copy / UX / framing improvements applied

- Two-mode UI (`stream` | `aead`) with an explicit **AAD** field that is clearly
  labelled "AEAD only, optional" — competitors often bury AEAD in a separate op.
- Output format note: in AEAD mode the encoded output is **ciphertext ‖ 16-byte
  tag**, documented in the param descriptions, the page copy, and the skill text
  so an LLM/CLI caller knows how to round-trip it.
- Security note steering users to authenticated password-based tools for real file
  encryption, and a prominent nonce-reuse warning.
- Tags/SEO target both "chacha20" and "chacha20-poly1305" / "aead" / "rfc 8439".

## Conclusion

No in-model capability gap versus the surveyed competitors: we match CyberChef's
raw-stream + AEAD + AAD coverage, add explicit RFC-8439 test-vector verification,
tamper detection on decrypt, configurable text/encoded key & nonce, and a fully
client-side three-surface deployment. The only deltas (reduced-round ChaCha,
legacy 8-byte-nonce DJB variant, XChaCha20) are deliberate scope decisions, not
missing functionality.
