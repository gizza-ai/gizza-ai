# Competitor analysis: nacl-box-encrypt

Date: 2026-08-15

## Scope

Tool: `nacl-box-encrypt` — public-key authenticated encryption compatible with NaCl/libsodium `crypto_box` (`Curve25519/X25519 + XSalsa20-Poly1305`, 24-byte nonce, 16-byte tag).

## Competitor scan

Search query: online NaCl crypto_box public key encryption tool recipient public key nonce base64 hex.

1. 8gwifi NaCl Box Public Key Encryption
   - Table stakes: plaintext input, public/secret key fields, nonce, encrypt/decrypt, output encoding, explicit primitive names.
   - UX pattern: simple text boxes and operation buttons.
   - Fit: in-model. Implemented operation, key fields, nonce, text/hex/base64 data, base64/hex output.

2. PKI Tools libsodium functions page
   - Table stakes: receiver key pair, secret/message field, multiple encodings such as string/hex/base64/UInt8Array, crypto_box grouping.
   - UX pattern: grouped controls with encoding choices next to key/message values.
   - Fit: key/message encodings are in-model; UInt8Array-specific UI is out-of-model for this generic page. Implemented hex/base64 keys, text/hex/base64 data, and labels.

3. Libsodium/PHP sodium crypto_box references and examples
   - Table stakes: differentiates crypto_box from sealed boxes and secretbox, explains sender/recipient key roles, nonce length, authentication failure.
   - UX pattern: example-driven docs rather than a browser form.
   - Fit: in-model as copy/validation guidance. Implemented explicit key-role docs, nonce/tag validation, combined output layout, and authentication failure errors.

## Decisions

- In-model capabilities shipped:
  - Encrypt and decrypt operations.
  - 32-byte Curve25519 keys in hex/base64.
  - Required 24-byte nonce for encryption; optional separate nonce for decryption.
  - Combined `nonce || ciphertext || tag` output for easy copy/paste decrypt.
  - Plaintext/data encodings: text, hex, base64.
  - Output encodings: base64 default and hex.
  - Worked CLI example and preset chips.
  - FAQ coverage for key roles, nonce uniqueness, output layout, and secretbox distinction.

- Out-of-model or intentionally excluded:
  - Random nonce/key generation: useful, but this tool is deterministic and the current page model has no secure "generate once and persist" control semantics.
  - PEM/passphrase parsing: neighboring X25519 tools cover richer key inspection/generation; crypto_box itself consumes raw 32-byte keys.
  - Sealed boxes: separate libsodium primitive (`crypto_box_seal`) with anonymous sender semantics and different output layout.
  - UInt8Array-specific form mode: JavaScript-oriented representation; hex/base64/text cover the same bytes in the generic CLI/page model.

## Descriptor/page checklist

- Every parameter has `.describe()`.
- Fixed choices use `Param::enumv`.
- Page inputs include labels and placeholders for key/nonce/data fields.
- Examples cover base64 output, hex output, and decrypt mode.
- Limits and edge cases are documented without copying competitor wording or branding.
