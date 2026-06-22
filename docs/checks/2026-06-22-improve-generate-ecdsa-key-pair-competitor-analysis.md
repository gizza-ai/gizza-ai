# generate-ecdsa-key-pair — competitor analysis & verification (2026-06-22)

## Tool
Generate a fresh ECDSA key pair on a NIST prime curve (P-256 / P-384 / P-521).
Outputs the private key as PKCS#8 PEM and the public key as SPKI PEM; optionally
also as JSON Web Keys (RFC 7517/7518) when `jwk=true`. Keys are generated locally
with a cryptographic RNG (`getrandom`/`OsRng`). Pure-Rust (`p256`/`p384`/`p521`),
so it runs on every backend (chat block + CLI).

Surfaces: **chat** (LLM/API) and **CLI**. **No standalone page** — like every other
key-generation tool in the repo (generate-rsa-key-pair, ed25519-key-pair-generator,
generate-pgp-key-pair), a zero-input non-deterministic generator does not fit the
page's recompute-on-input model.

## Surface verification (all pass)
- **Unit tests** (`cargo test --workspace`): 5 core + 1 drift-guard schema test — green.
  Covers curve-alias parsing, valid PEM round-trip (re-parse + public-key match),
  P-384/P-521 generation, JWK shape (private carries `d`, public does NOT leak `d`),
  and freshness (two calls differ).
- **Chat block** (`wafer build`): instantiates clean, 565 KiB, P-521 included (no
  getrandom/WASI-import failure — only random key *generation* is used, not the
  RFC-6979 signer that gated P-521 in `ecdsa-sign`).
- **CLI** (`gizza tool generate-ecdsa-key-pair …`): verified p256 (default),
  `curve=p384 jwk=true`, and `curve=p521` — all emit valid PEM (+ JWK when asked).
- **Cross-tool**: `openssl pkey -in <priv.pem> -text -noout` accepts the generated
  PKCS#8 PEM as a valid 256-bit EC key and derives the matching public key.

## Competitors surveyed (top 5 ECDSA / EC key-pair generators)
1. **CryptoTools / "ECDSA key generator" web tools** — curve picker (secp256k1,
   P-256/384/521), PEM + hex output, generate-in-browser. Many include secp256k1
   (Bitcoin/Ethereum) alongside the NIST curves.
2. **devglan / 8gwifi.org EC key generators** — P-256/384/521 + secp256k1, PEM
   output, sometimes DER and OpenSSH formats.
3. **JWK generators (mkjwk.org, jwkset.com)** — focus on JWK output for `EC` keys:
   `crv` (P-256/384/521), optional `kid`/`use`/`alg`, both private (with `d`) and
   public JWK. PEM often secondary.
4. **OpenSSL CLI** (`openssl ecparam -genkey -name prime256v1 | openssl pkcs8
   -topk8`) — the de-facto reference: any named curve, PEM/DER, SEC1 or PKCS#8.
5. **`ssh-keygen -t ecdsa -b 256|384|521`** — generates an OpenSSH ECDSA key pair
   (OpenSSH private + `.pub` format), P-256/384/521.

## Gap analysis (fit-to-model)
Closed / already covered:
- **Curve selection P-256/384/521** — covered (all three NIST prime curves).
- **PEM output, PKCS#8 private + SPKI public** — covered, the OpenSSL-standard
  encodings; cross-validated with openssl.
- **JWK output** — covered via `jwk=true` (RFC 7517 EC JWK; public JWK correctly
  omits `d`). Matches the dedicated JWK-generator competitors' core capability.

Out-of-model / intentionally not built (documented, not copied):
- **secp256k1 (Bitcoin/Ethereum)** — not a NIST prime curve; the `k256` crate would
  add it, but the tool's stated scope and name ("ECDSA on a NIST curve", mirroring
  `ecdsa-sign`) is the NIST P-curves. A separate `generate-secp256k1-key-pair` tool
  is the cleaner home; noting as a future tool rather than overloading this one.
- **OpenSSH / SEC1 / DER / passphrase-encrypted private keys** — additional output
  encodings; PKCS#8 PEM is the portable default and converts to the others with one
  `openssl`/`ssh-keygen` command. Out of scope for a focused generator.
- **`kid`/`use`/`alg` JWK metadata** — optional JOSE annotations; the base EC JWK
  (kty/crv/x/y[/d]) is the interoperable core. Could be a later enhancement.

No competitor copy, branding, or trademarks were used. No out-of-model features were
implemented.
