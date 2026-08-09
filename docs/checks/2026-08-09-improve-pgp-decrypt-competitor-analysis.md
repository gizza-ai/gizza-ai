# PGP decrypt competitor analysis (2026-08-09)

Backlog tool: `pgp-decrypt` — decrypt an ASCII-armored OpenPGP message using either a private key (plus optional key passphrase) or a symmetric message password.

## Competitor scan

| Competitor | Observed surface | Table-stakes controls and UX | In-model decisions for this tool | Out-of-model / not built |
| --- | --- | --- | --- | --- |
| 8gwifi PGP encrypt/decrypt (`8gwifi.org/pgpencdec.jsp`) | Search result and snippets show a combined encrypt/decrypt page, fields for PGP private key and passphrase, built-in examples/code, and no-install online use. | Multiline encrypted message field, multiline private key field, passphrase field, same-page decrypt output, explicit private-key/passphrase distinction. | `message`, `private_key`, and `passphrase` are all explicit inputs; message and key fields are multiline textareas. Errors distinguish missing key, public key pasted into private-key field, wrong key, and wrong passphrase. | Combined encrypt/decrypt/key-generation UI is already covered by separate gizza tools; this block stays focused on decrypt. |
| Devglan PGP encryption/decryption (`devglan.com/online-tools/pgp-encryption-decryption`) | Online combined encryption/decryption using browser OpenPGP; snippets emphasize private-key decryption, all work in-browser, and separate file handling for binary/large payloads. | Browser-local privacy statement, private-key and passphrase inputs, message-oriented text UX, warning that files/large binary payloads are a separate flow. | Page copy states local WebAssembly execution; armored input is capped at 4 MiB; `output_format` offers auto/text/base64/hex so binary payloads are not silently corrupted. | Dedicated PGP file decrypt upload/download flow is not built; current gizza page model is text output for this pure tool, so binary bytes are encoded rather than downloaded. |
| pgptool.dev decrypt (`pgptool.dev/decrypt/`) | Search snippets show a focused decrypt page: paste ciphertext beginning with `BEGIN PGP MESSAGE`, supply private key and passphrase, get plaintext; troubleshooting calls out wrong private key vs wrong passphrase/session key failure. | Focused decrypt-only UX, full armor paste, private key/passphrase, helpful failure causes, recipient matching. | The core inspects session-key packet types to choose public-key vs password mode; result includes recipient key IDs when available and error messages point to wrong key/passphrase/missing password. | No passphrase brute forcing or key recovery; if a private-key passphrase is forgotten, that is outside model and intentionally not attempted. |
| Kleopatra PGP decrypt (`kleopatra.app/tools/pgp-decrypt`) | Browser decrypt page for received PGP messages using a private key; snippets emphasize no installation and client-side processing. | Message field, private key field, passphrase support, privacy assurance, simple decrypted-message result. | Same essential controls plus optional signature public key; page labels use friendly explanations and placeholders for complete armor blocks. | Native Kleopatra-style keyring integration and local secret-key discovery are not available in a browser-only gizza block. |

## Table-stakes matrix

| Capability | Decision | Notes |
| --- | --- | --- |
| ASCII-armored message textarea | In model | `message` is required and multiline; validation requires a complete `BEGIN/END PGP MESSAGE` block. |
| Private key textarea | In model | `private_key` is optional because symmetric messages do not need it; public-key messages require it. |
| Key passphrase / symmetric password | In model | One `passphrase` field covers protected private keys and symmetric message passwords, matching common online-tool UX. |
| Auto-detect public-key vs symmetric encryption | In model | Core inspects PKESK/SKESK packets instead of forcing users to pick a mode. |
| Output plaintext | In model | JSON result includes plaintext plus metadata so CLI/page/chat surfaces share one shape. |
| Binary payload handling | In model with encoded output | `auto` falls back to base64; explicit `base64` and `hex` are offered. Raw file download is out of scope for this text page. |
| Embedded signature reporting and verification | In model | Optional `public_key` verifies encrypted-and-signed messages; without it the signature is reported as unverified. |
| Large file decrypt | Out of model | Browser memory and text-page output make large binary files unsuitable; docs state a 4 MiB armor cap. |
| Passphrase recovery/brute force | Out of model | Not safe or practical in this block; errors point users to the correct key/passphrase instead. |
| Keyring integration | Out of model | This toolkit accepts pasted armor; it does not access OS keyrings or browser storage. |

## Defaults and UX choices

- `output_format` default is `auto`, mirroring competitor behavior that shows readable text immediately while preserving binary payloads by encoding them.
- Multiline textareas are used for message, private key, and optional public key so armor line breaks survive paste.
- The output is pretty JSON rather than a bare plaintext pane because the gizza tool abstraction exposes metadata consistently across CLI, chat, and page surfaces.
- The page does not include preset chips: real PGP messages and private keys are user-specific, and shipping fake keys as examples would be noisy and risky.
