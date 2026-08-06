# age-encrypt competitor analysis — 2026-08-06

## Sources scanned

- age manual pages for the command-line tool (`age(1)`) as the reference implementation surface.
- The `age-encryption` JavaScript package examples for browser-style API shape.
- The Rust `age` crate documentation for feasible wasm-safe implementation details.

## Table-stakes capabilities

| Capability / UX pattern | Seen in competitors | Model fit | Decision |
| --- | --- | --- | --- |
| ASCII-armored output with `-----BEGIN AGE ENCRYPTED FILE-----` headers | CLI manuals and JS examples emphasize copy-pasteable armored output | In model | Always emit armored text, not binary age bytes. |
| Passphrase encryption | CLI supports passphrase mode and treats it as mutually exclusive with recipient mode | In model | Add `mode=passphrase`, a passphrase field, and a bounded scrypt work factor. |
| X25519 native recipient encryption | CLI and library examples support public recipients starting with `age1` | In model | Add `mode=recipients` and accept one or more native age public keys. |
| Recipient files / multiple recipients | CLI accepts recipient files and repeated recipient flags | In model | Accept pasted recipients separated by new lines, commas, semicolons, or spaces; ignore `#` comments. |
| SSH recipients | CLI can use SSH public keys | Out of model for this block | Document as unsupported; reject SSH-looking keys by name because this pure text tool implements native age recipients only. |
| Identity generation and decryption | Library examples and CLI cover identity management and decrypt flows | Out of model for this block | Keep this block encryption-only; direct users to compatible age clients for decryption and key generation. |
| File encryption | CLI primarily encrypts files and streams | Out of model for page shape | Limit to small text snippets; larger files belong in the age CLI or a future file-oriented block. |
| Work-factor / memory control | Passphrase mode uses scrypt; defaults can be expensive | In model | Expose a 10-15 slider and cap the value to avoid wasm sandbox memory traps. |
| Preset/example buttons | Browser tools commonly offer examples or preset runs | In model | Add passphrase example chips for default and low-memory work factors. |

## Defaults chosen

- `mode`: `passphrase`, because it is usable without generating an age identity first.
- `work_factor`: `14`, which uses about 16 MiB scrypt scratch and leaves room in the wasm sandbox.
- Output format: text, so the generated page includes a copyable/downloadable armored result.

## Verification expectations

The required checks should prove both encryption families:

- passphrase round-trip through the `age` crate in unit tests;
- recipient round-trip using generated in-test X25519 identities;
- clean errors for empty plaintext, missing passphrase, missing recipients, SSH keys, and pasted secret identities;
- CLI/page output starts with the armored age header while remaining nondeterministic across runs.
