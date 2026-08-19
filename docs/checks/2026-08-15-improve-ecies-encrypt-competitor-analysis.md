# Competitor analysis: ecies-encrypt

Date: 2026-08-15

## Scope

Tool: `ecies-encrypt` — ECIES-style hybrid encryption/decryption for elliptic-curve public keys, with ECDH, HKDF-SHA256, authenticated symmetric encryption, and copy/paste-safe encodings.

## Competitor scan

Search query: online ECIES encrypt decrypt tool public key AES GCM secp256k1 P-256 base64 hex.

1. Hi My Toolkit ECC / P-256 encrypt-decrypt page
   - Table stakes: public/private key fields, key generation nearby, AES-256-GCM explanation, nonce and authentication tag awareness, PEM-oriented examples.
   - UX pattern: operation flow with key fields, plaintext/ciphertext boxes, and explanatory step copy.
   - Fit: encryption/decryption, PEM keys, AES-256-GCM, nonce/tag handling, and explanatory copy are in-model. Key generation is intentionally out-of-model for this block because the gizza tool consumes provided keys and should not imply key custody.

2. ecies.py / ecies.js-compatible library examples
   - Table stakes: secp256k1 default, encrypt with public key, decrypt with private key, base64/hex payloads, ephemeral public key prefix, AES-256-GCM, and deterministic vectors for tests.
   - UX pattern: compact parameter set with library-compatible defaults.
   - Fit: in-model. The descriptor defaults to secp256k1, AES-256-GCM, base64 output, `ephemeral-and-point` KDF input, and 16-byte nonce compatibility.

3. Practical Cryptography for Developers ECIES example
   - Table stakes: describes ECIES as ECDH plus KDF plus symmetric authenticated encryption, distinguishes the ephemeral key from recipient keys, and shows worked examples.
   - UX pattern: docs/example-first rather than a form-heavy tool.
   - Fit: in-model as page copy and validation guidance. The page documents payload layout, key roles, authentication failure behavior, and edge cases.

4. Generic online AES encryption tools
   - Table stakes: encoding toggles, nonce/IV controls, mode/cipher naming, and exact copyable outputs.
   - UX pattern: select controls for algorithms and encodings, textareas for data.
   - Fit: encoding controls, cipher choice, nonce entry, and exact output verification are in-model. Password-derived keys and unauthenticated AES modes are out-of-model for an ECIES public-key tool.

## Decisions

- In-model capabilities shipped:
  - Encrypt and decrypt operations.
  - Curves: secp256k1, P-256, and P-384.
  - Ciphers: AES-256-GCM and XChaCha20-Poly1305.
  - AES-GCM nonce lengths: 16 bytes for common ECIES library compatibility and 12 bytes for standard GCM usage; XChaCha20 uses 24 bytes.
  - Key encodings: auto-detected PEM, hex, and base64.
  - Data encodings: text, hex, base64, with auto defaults for encrypt/decrypt.
  - Output encodings: base64 default and hex.
  - KDF input convention selector: `ephemeral-and-point` and `shared-x`.
  - Optional deterministic nonce and ephemeral private key fields for reproducible vectors and page tests.
  - Preset examples for base64 encrypt, hex encrypt with compressed ephemeral public key, and decrypt.

- Out-of-model or intentionally excluded:
  - Long-term key generation/storage: useful, but separate from the current gizza block model and easy to misuse if presented as a browser key manager.
  - Password-based encryption: a different threat model requiring KDF/salt controls and not ECIES.
  - HPKE, age, PGP, or NaCl sealed boxes: related tools with different wire formats and interoperability expectations.
  - Multi-recipient envelopes and file packaging: outside the single-message descriptor and generic page model.

## Descriptor/page checklist

- Every parameter has `.describe()`.
- Fixed choices use `Param::enumv`.
- Page inputs include labels and placeholders for key, data, nonce, and ephemeral key fields.
- Examples cover base64 output, hex output, compressed ephemeral keys, and decrypt mode.
- Limits and edge cases are documented without copying competitor wording or branding.
