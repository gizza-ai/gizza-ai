# ssh-public-key-from-private — competitor analysis & differentiation

**Tool:** `gizza-ai/ssh-public-key-from-private` — derive the OpenSSH public-key
line (the `id_*.pub` / `authorized_keys` single-line format) from a private key.
**Date:** 2026-06-22

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `ssh-keygen -y -f key` | CLI | The canonical answer. Requires a local OpenSSH install and the key on disk. What this tool mirrors offline. |
| `openssl pkey -in key -pubout` / `ssh-keygen` recipes (Baeldung, Lindevs, simplified.guide blog posts) | docs/CLI | Plenty of how-tos, all command-line; nothing for someone who just wants to paste a key and copy the `.pub` line. |
| Generic "online SSH key" / public-key generator sites | Web | Most **generate** new key pairs server-side; few derive a `.pub` from an existing private key, and the standard security advice is explicitly **"never paste a private key into a browser form"** because typical sites upload it. |
| gizza's own `pem-public-key-extract` | tool | Emits a **PEM SubjectPublicKeyInfo** block (`-----BEGIN PUBLIC KEY-----`), NOT the OpenSSH `.pub` line. Different output format — complementary, not a duplicate. |

## How gizza's tool is better / different

1. **Right output format for SSH.** Emits the OpenSSH wire format
   (`ssh-rsa` / `ecdsa-sha2-nistp256` / `ecdsa-sha2-nistp384` / `ssh-ed25519`
   + base64 blob + optional comment) — exactly what goes in `authorized_keys`
   or GitHub/GitLab, which `pem-public-key-extract`'s PEM SPKI output does not give.
2. **Local — the private key is never uploaded.** Runs in WASM (chat Service
   Worker + CLI + standalone page). This directly answers the security objection
   to browser tools: nothing leaves the machine. The offline equivalent of
   `ssh-keygen -y` with no install.
3. **Byte-for-byte identical to `ssh-keygen -y`.** Output was cross-checked
   against real `ssh-keygen -y -f` for RSA, P-256, P-384 and Ed25519 keys.
4. **Broad input acceptance.** PEM in PKCS#8 (`PRIVATE KEY`), PKCS#1
   (`RSA PRIVATE KEY`) or SEC1 (`EC PRIVATE KEY`) form, **or** raw DER bytes as
   hex/base64. Auto-detects the algorithm by default.
5. **Optional comment** appended as the trailing field (e.g. `user@host`).
6. **Honest about OpenSSH-container keys.** A modern
   `-----BEGIN OPENSSH PRIVATE KEY-----` file is rejected with the exact fix
   (`ssh-keygen -p -m PEM -f key`) rather than a confusing parse error.

## Verification

Eleven core unit tests cover RSA/P-256/P-384/Ed25519 PEM, base64-DER input,
comment appending, and every error path (empty, public-key, OpenSSH-container,
garbage, bad enums). The wire-format output was cross-checked **byte-for-byte
against `ssh-keygen -y -f`** for all four key types. **End-to-end CLI**: an
Ed25519 PEM produced `ssh-ed25519 AAAAC3Nza... test@x` matching ssh-keygen, RSA
produced `ssh-rsa AAAAB3Nza...`, and the public-key error path returned exit 1
with a clear message. Page Playwright (2 tests) covers the auto path and a
deep-link with a comment, asserting the exact P-256 `ecdsa-sha2-nistp256` line.

## Relationship to pem-public-key-extract

`pem-public-key-extract` derives the public key and emits a **PEM SPKI**
(`-----BEGIN PUBLIC KEY-----`) block — the form libraries/TLS want. This tool
derives the same public key but emits the **OpenSSH `.pub` line** — the form SSH
servers and Git hosts want. Same underlying derivation, two different wire
formats; both are kept.

## Scope / honest limitations

- Supports RSA, EC NIST **P-256 / P-384**, and Ed25519. Other curves
  (P-521, secp256k1) and `sk-*`/FIDO and certificate (`*-cert-v01`) keys are not
  derived.
- OpenSSH-container private keys (`-----BEGIN OPENSSH PRIVATE KEY-----`) are not
  read — convert to PEM first (clear error message tells the user how).
- Encrypted/passphrase-protected private keys are not decrypted; supply an
  unencrypted PEM/DER.

## Possible future enhancements

- Accept the OpenSSH private-key container directly (parse the bcrypt-KDF format).
- Add the SHA-256 fingerprint (`SHA256:...`) alongside the key line.
- Support P-521 once a deterministic public-point derivation path is wired.
