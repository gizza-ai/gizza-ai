# pem-to-jwk — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/pem-to-jwk` — convert a PEM-encoded RSA/EC key into the
equivalent JSON Web Key (JWK, RFC 7517/7518). Pure-text input → JSON-data output,
so it ships chat + CLI + a page (text field input), like `luhn-validate` /
`json-yaml-converter`.

## What competitors do

- **Online "PEM to JWK" / "JWK converter" sites** (e.g. various jwt/jwk web
  tools) — paste a key, get a JWK. Strengths: zero install, often add a `kid`
  thumbprint. **Weakness: the key is sent to (or processed by) a web page you
  don't control** — for a *private* key that's a real exposure, and many such
  tools are ad-supported black boxes.
- **`pem-jwk` (npm), `jwcrypto`, `jose` CLIs, python `jwcrypto`/`authlib`** —
  local + scriptable, the correct tools for developers, but require a Node/Python
  toolchain and per-format knowledge (PKCS#1 vs PKCS#8 vs SPKI vs SEC1).
- **`step crypto key format`, OpenSSL + manual base64url** — powerful but fiddly;
  OpenSSL has no direct "to JWK" and needs scripting of the raw integers.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (RustCrypto: `rsa`, `p256`,
   `p384`, `p521`, `pkcs8`, `spki`) compiled to wasm: runs in the chat Service
   Worker, headless in the CLI, and in-browser on the page. Private keys never
   leave the device — the one thing the convenient web converters can't promise.
2. **Format-agnostic input.** Accepts RSA as PKCS#1 (`RSA PRIVATE/PUBLIC KEY`),
   PKCS#8 (`PRIVATE KEY`), or SPKI (`PUBLIC KEY`); EC as SEC1 (`EC PRIVATE KEY`),
   PKCS#8, or SPKI — auto-detected from the PEM label and the algorithm/curve OID.
   The user doesn't have to know or convert the encoding first.
3. **Correct, spec-compliant output.** All members are **base64url without
   padding** (RFC 7518). Private keys emit the full set — RSA `d, p, q, dp, dq,
   qi` (CRT params computed when the DER omits them) and EC `d` — so the result is
   a usable private JWK, not just the public half.
4. **Three curves.** EC over P-256, P-384 and P-521 (the JOSE `ES256/384/512`
   curves).
5. **Agent- + automation-friendly.** One string in, a JSON object out — ideal for
   an LLM building a JWKS or a CI step, addressable identically via chat, CLI
   (`gizza tool pem-to-jwk pem=...`) and a `?input=` deep-link.

## Honest scope

- **Does not emit `kid`/`use`/`alg`/`x5c`.** Output is the bare key JWK; thumbprint
  (`kid`) and usage metadata are not added (a deliberate, minimal contract — these
  are easy to add downstream and depend on the caller's policy).
- **No OKP (Ed25519/X25519) or RSA-PSS-specific params**, and no symmetric (`oct`)
  keys — RSA + the three NIST EC curves only.
- **Does not generate keys** (see `generate-rsa-key-pair`) or convert JWK→PEM
  (the reverse direction) — this is one-way PEM→JWK.

## Tests

5 core unit tests over **real keys generated with OpenSSL** (`rsa_priv/pub`,
`ec256_priv/pub`, `ec384_priv`), covering: RSA public JWK shape (`e == AQAB`, no
`d`), RSA private JWK has all CRT members and its `n` matches the public key, EC
P-256 public+private (32-byte coords, private `x/y` match the public PEM), EC P-384
private (48-byte `d`), and clear errors on empty / non-PEM / unknown-label input.
Plus the block drift-guard schema test. **CLI verified** end-to-end on all five
key files; **page** verified with Playwright (EC P-256 deep-link → `kty: EC` /
`crv: P-256`, and a non-PEM input → a `PEM` error message). `wafer build`
instantiates the chat block (828 KiB).
