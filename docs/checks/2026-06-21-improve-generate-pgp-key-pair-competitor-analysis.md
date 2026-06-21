# generate-pgp-key-pair — competitor analysis & differentiation

**Tool:** `gizza-ai/generate-pgp-key-pair` — generate a new OpenPGP key pair (RSA
or ECC) with user ID and optional passphrase, emitting armored public/private
keys.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `gpg --full-generate-key` | CLI | The reference, but requires GnuPG installed and an interactive prompt flow; scripting it (`--batch`) is fiddly. |
| Online PGP key generators (e.g. some "PGP key generator" sites) | Web | Convenient but **generate your private key on a server** — unacceptable for key material; trust is the whole problem. |
| Mailvelope / Keybase / browser-extension PGP | App | Tied to an app/identity; not a one-off "give me a keypair" primitive. |
| `openpgp.js` playgrounds | Web/lib | Need to write JS; quality and key-format choices vary. |

## How gizza's tool is better / different

1. **Generated locally — private key never leaves the device.** Runs in WASM
   (chat service worker or CLI) with `getrandom` for the CSPRNG. This is the one
   thing the online generators get catastrophically wrong.
2. **Modern default.** `curve25519` produces a GnuPG-compatible v4 key with an
   **EdDSA** signing primary and a **Curve25519 ECDH** encryption subkey — fast,
   small, and current best practice; RSA 2048/3072/4096 available for legacy
   compatibility.
3. **Optional passphrase protection** of the private key (S2K), built in.
4. **Ready-to-use output.** ASCII-armored public *and* private keys plus the
   fingerprint, in one call.
5. **No install, no prompts.** `gizza tool generate-pgp-key-pair` or just ask in
   chat — no GnuPG, no interactive ceremony.

## Verification

Unit tests confirm the generated keys self-verify (`SignedSecretKey::verify` /
`SignedPublicKey::verify`) and that a passphrase-protected key unlocks with the
correct passphrase. **Cross-tool:** the CLI-generated Curve25519 public key was
fed to gizza's own `pgp-encrypt`, which successfully produced a
`-----BEGIN PGP MESSAGE-----` — proving the key is valid and interoperable with
real OpenPGP encryption.

## Surfaces & honest scope

- **Chat + CLI only — no web page.** A non-deterministic key generator doesn't
  fit the page's recompute-on-input model (same rationale as
  `generate-rsa-key-pair` / `ed25519-key-pair-generator`).
- RSA generation (especially 4096) is CPU-heavy and takes longer; Curve25519 is
  near-instant and is the default.

## Possible future enhancements

- Key expiration date option.
- Multiple user IDs.
- Choice of additional ECC curves (NIST P-256/384) for specific compliance needs.
