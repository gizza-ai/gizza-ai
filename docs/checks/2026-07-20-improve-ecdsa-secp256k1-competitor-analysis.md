# ecdsa-secp256k1 — competitor analysis (2026-07-20)

**Tool:** generate secp256k1 keypairs and sign/verify messages with ECDSA — the
signature scheme behind Bitcoin and Ethereum keys. Pure-Rust `k256` (RustCrypto),
deterministic RFC 6979 signing, low-S normalized. Surfaces: chat + CLI + page
(sign/verify are deterministic; generate is one enum choice on the same page,
like competitors' "Generate Key Pair" button).

**Not a dup:** `crypto-keypair-generator` covers secp256k1 *wallet* generation
(addresses/WIF, no signing); `ecdsa-sign` is NIST P-256/P-384 only and rejects
secp256k1; `ed25519-sign-verify` is a different scheme. secp256k1 ECDSA
sign/verify existed nowhere in the corpus.

## Top competitors scanned
1. **8gwifi.org ECDSA Sign & Verify** — curve dropdown (secp256k1, P-256/384/521,
   Brainpool), PEM keys, sign/verify radio toggle, base64 signature out,
   pre-populated example keys, "Generate Keys" button.
2. **CoderTools ECDSA Signature Generator & Verifier** — curves secp256k1 +
   P-256/384/521; key formats hex/base64/space-hex/C-array/PEM; hash SHA-256/384/512,
   Keccak-256 (Ethereum), None (pre-hashed); message encodings text/hex/base64;
   outputs hex/base64/DER + individual r and s + compressed & uncompressed public
   key; Generate/Derive/Sign/Verify buttons; local processing.
3. **Devglan ECC Key Pair Generator & Verifier** — secp256k1 + P-256; hex keys;
   generate/sign/verify (+ ECDH shared secret); compact hex signatures; copy
   buttons; 100% client-side.

(paulmillr.com/ecc was unreachable at scan time and was replaced by Devglan.)

## Table stakes → descriptor decisions
| Capability | Tag | Decision |
|---|---|---|
| Generate secp256k1 keypair (all 3) | in-model | `operation=generate` (default): private hex + PKCS#8 PEM, public compressed/uncompressed hex + SPKI PEM |
| Sign message with private key (all 3) | in-model | `operation=sign`, RFC 6979 deterministic, low-S |
| Verify signature with public key (all 3) | in-model | `operation=verify`, `valid=false` result (not error) on mismatch |
| Hash choice SHA-256/384/512, Keccak-256, none/pre-hashed (CoderTools) | in-model | `hash` enum `sha256` (default) / `keccak256` / `sha384` / `sha512` / `none` (32-byte hex digest) |
| Message encodings text/hex/base64 (CoderTools) | in-model | `message_encoding` enum utf8/hex/base64, whitespace-tolerant hex |
| Key formats hex + base64 + PEM (8gwifi PEM, CoderTools all) | in-model | auto-detect: raw hex (optional `0x`), base64, SEC1/PKCS#8 private PEM, SPKI public PEM; compressed AND uncompressed public points |
| Signature output DER + compact + r/s components (CoderTools), base64 (8gwifi) | in-model | sign emits compact hex + base64, DER hex, r, s, recovery id + Ethereum-style v |
| Verify accepts DER or compact | in-model | auto-detect by DER `0x30` header / 64-byte length, hex or base64; high-S normalized before verify (openssl interop) |
| Pre-populated worked examples (8gwifi) | in-model | `[[example]]` chips: generate / sign / verify with a published RFC 6979 test vector |
| Curve dropdown with NIST/Brainpool curves | out-of-model here | P-256/P-384 sign + keygen are the sibling tools `ecdsa-sign` / `generate-ecdsa-key-pair`; Brainpool has no proven wasm crate in the corpus. Stated on the page. |
| ECDH shared secret (Devglan, Javainuse) | out-of-model | key-agreement, not signatures — a separate tool if ever backlogged |
| C/C++ array key/message encoding (CoderTools) | out-of-model | niche embedded convenience; hex covers it |
| WIF private-key import; EIP-191 `personal_sign` prefix; Bitcoin signed-message format | out-of-model | wallet-format plumbing beyond all 3 scanned competitors; `crypto-keypair-generator` owns WIF export. Keccak-256 raw hashing (what CoderTools ships) is in. |

## Correctness anchors
- RFC 6979/secp256k1 known-answer vector (key=1, "Satoshi Nakamoto", SHA-256)
  asserted in unit tests + Playwright + CLI exact-output.
- Sign→verify round-trips across every hash and both public-key point forms.
- k256 signing is low-S normalized (Bitcoin/Ethereum canonical); verify
  normalizes high-S DER inputs (e.g. from openssl) before checking.

No competitor copy, branding, or trademarks were used.
