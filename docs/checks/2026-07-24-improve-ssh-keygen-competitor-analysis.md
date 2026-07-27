# ssh-keygen competitor analysis (2026-07-24)

Tool: `gizza-ai/ssh-keygen` — generate OpenSSH-format Ed25519 or RSA key pairs locally, returning the private key, authorized_keys public key line, SHA256 fingerprint, algorithm, and RSA bit size.

## Competitor scan

| Source | Positioning | Table-stakes observed |
| --- | --- | --- |
| OpenSSH `ssh-keygen` documentation / CLI references | Canonical local baseline for SSH keys. | Algorithm selection (`-t`), RSA bit size (`-b`), optional public-key comment, OpenSSH private-key container, authorized_keys public line, SHA256 fingerprint via companion commands. |
| Browser SSH key generators with Ed25519/RSA support | Convenience UI around Web Crypto / client-side generation. | Ed25519 as recommended default, RSA legacy option, 2048/4096-bit choices, public/private copy areas, safety copy explaining local generation. |
| Multi-format key generator pages | Broader crypto format conversion/generation. | Export choices such as PEM/PKCS#8/PPK/ECDSA, passphrase encryption, randomart/fingerprint displays, sometimes browser-only assurances. |

## Fit-to-model decisions

| Capability / UX pattern | Decision | Rationale |
| --- | --- | --- |
| Ed25519 key generation | Built | Modern SSH default, fast, supported by the RustCrypto `ssh-key` crate under wasm32-wasip1. |
| RSA key generation | Built | Still table-stakes for legacy hosts; supports 2048, 3072, and 4096-bit sizes. |
| OpenSSH private-key output | Built | This is the differentiator versus existing PKCS#8/SPKI key-pair tools. |
| authorized_keys public-key line | Built | Directly pasteable to `authorized_keys`; cross-checked with `ssh-keygen -y -f`. |
| Optional comment | Built | Common `user@host` workflow and part of the public-key line. |
| SHA256 fingerprint | Built | Mirrors the fingerprint users check after generation/import. |
| ECDSA / P-256 / P-384 / P-521 | Not built for this pass | Existing focused ECDSA key-pair tooling covers non-SSH PEM/DER workflows; SSH ECDSA can be added later without blocking the Ed25519/RSA table stakes. |
| Passphrase-encrypted private keys | Out-of-model for this focused pass | Requires KDF/encryption UX, confirmation, and recovery copy beyond the simple generator; better as a dedicated enhancement. |
| PuTTY `.ppk` export | Out-of-model | Useful for PuTTY users but a separate container format and not necessary for OpenSSH/authorized_keys parity. |
| Randomart display | Out-of-model | Nice-to-have visual verification, not required for CLI/chat JSON output. |
| Standalone live page | Not built | Key generation is intentionally non-deterministic: the current page runtime recomputes on input changes and would regenerate secrets unexpectedly. This follows the existing no-page pattern for key generators. |

## Verification notes

- Unit tests cover Ed25519 happy path, RSA happy path, unknown algorithm, too-small RSA size, and case-insensitive key type.
- Drift guard checks the descriptor schema for `key_type`, `bits`, and `comment`.
- CLI verification generated Ed25519 and RSA keys without printing private key material, wrote each private key to a temporary 0600 file, and verified `ssh-keygen -y -f` exactly matched the returned public-key line.
- No Playwright page spec is applicable because the tool deliberately has no page.
