# keypair-generator competitor analysis (2026-07-25)

Tool goal: generate fresh X25519 or Ed25519 key pairs locally and return standard encodings for secure channels and signing.

## Sources checked

Web search: `online keypair generator ed25519 x25519 private public key hex base64 PEM`.

Skimmed representative public tools from the top results:
- Browser SSH key generators focused on Ed25519/OpenSSH output.
- General keypair generators offering Ed25519 plus other asymmetric algorithms with PEM export.
- Ed25519-specific generators that expose PEM and base64/hex conversion-adjacent output.

## Table-stakes capabilities and model fit

| Capability / UX pattern | Competitor expectation | In gizza model? | Decision |
| --- | --- | --- | --- |
| Generate locally/offline | Browser-side tools emphasize that keys stay on-device. | Yes | Implemented: pure Rust CSPRNG; CLI/chat run locally. |
| Algorithm choice | Common controls include Ed25519, RSA/ECDSA, and sometimes chain-specific variants. | Partly | Implemented Ed25519 and X25519 because the backlog requested those exact modern Curve25519 families. RSA/ECDSA already exist in sibling tools. |
| Standard PEM output | PEM private/public exports are expected for signing keys. | Yes | Implemented PKCS#8 private PEM and SPKI public PEM for both algorithms. |
| Raw encodings | Hex/base64 are useful for protocols and copy/paste workflows. | Yes | Implemented raw 32-byte private/public values in lower-hex and base64. |
| OpenSSH private/public files | SSH-key tools commonly export OpenSSH private blobs and authorized_keys lines. | Out of scope / duplicate | Existing `ssh-keygen` covers OpenSSH Ed25519/RSA with comments/fingerprints. This tool stays protocol-neutral PEM/raw. |
| Wallet addresses | Crypto wallet keypair tools return chain addresses. | Out of scope / duplicate | Existing `crypto-keypair-generator` covers bitcoin/ethereum/solana wallet-address flows. |
| Key strength / size controls | RSA/ECDSA tools expose bit size/curve controls. | Not applicable | Ed25519 and X25519 are fixed 32-byte key formats. No size parameter. |
| Download buttons / file bundle | Browser tools often offer copy/download for key files. | Not page-fit | This is a no-input nondeterministic generator. Existing no-page generator pattern uses chat+CLI only to avoid live page recomputation producing a new secret on every input event. |
| Usage guidance | Tools distinguish signing keys from key-agreement keys. | Yes | Descriptor and JSON output include usage text for Ed25519 vs X25519. |

## Defaults and examples

Default algorithm: `ed25519`, matching common recommendations for modern signing keys.

Worked CLI examples verified locally:

```bash
gizza tool keypair-generator algorithm=ed25519
gizza tool keypair-generator algorithm=x25519
```

Both return JSON with:
- `algorithm`
- `usage`
- `private_pem`
- `public_pem`
- `private_base64`
- `private_hex`
- `public_base64`
- `public_hex`

## Gaps intentionally not closed

- OpenSSH format, comments, and fingerprints: covered by `ssh-keygen`.
- Blockchain wallet addresses: covered by `crypto-keypair-generator`.
- RSA/ECDSA curve/bit-size selection: covered by `generate-rsa-key-pair` and `generate-ecdsa-key-pair`.
- Browser page/download UX: omitted because no-input nondeterministic key generation does not fit the generated page's recompute-on-input model.
