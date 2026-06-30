# Competitor analysis: rc2-cipher

Date: 2026-06-29
Tool: `gizza-ai/rc2-cipher`

## Goal

Provide a legacy-interoperability RC2 encrypt/decrypt tool for chat, CLI, and browser page surfaces. Support RFC 2268 RC2 with configurable effective key length (T1), ECB/CBC modes, PKCS#7 padding, and hex/base64 key, IV, and ciphertext encoding. Warn clearly that RC2 is obsolete and not appropriate for new designs.

## Competitors reviewed

1. KeyDecryptor RC2 Encryption and Decryption Online
   - URL: https://keydecryptor.com/encryption-tools/rc2
   - Notes: Search result describes browser-side RC2 encrypt/decrypt in CBC mode with hex ciphertext and hex key.
   - Gap analysis: gizza adds both CBC and ECB, base64 as well as hex, explicit effective-key-bits control, chat/CLI surfaces, and test vectors. KeyDecryptor appears narrower but has a dedicated web form.

2. 8gwifi CipherFunctions
   - URL: https://8gwifi.org/CipherFunctions.jsp
   - Notes: General online cipher page advertising many symmetric algorithms and client-side processing.
   - Gap analysis: Broad cipher catalog is out of scope for a single tool; gizza's advantage is focused RC2 documentation, deterministic schema, CLI/chat integration, and in-browser WASM.

3. CyberChef-style recipes
   - URL: https://gchq.github.io/CyberChef/
   - Notes: CyberChef is the common multi-operation workbench model used by analysts for encryption/decryption, encoding, and malware/config recipes. Search results often reference CyberChef recipes for payload decryption.
   - Gap analysis: CyberChef's recipe graph and many transforms are out of model here. gizza focuses on a single safe RC2 primitive with explicit parameters, easier CLI invocation, and schema-backed chat usage.

4. Adobe ColdFusion `Encrypt` documentation
   - URL: https://helpx.adobe.com/coldfusion/cfml-reference/coldfusion-functions/functions-e-g/encrypt.html
   - Notes: ColdFusion documents RC2 as an RFC 2268 symmetric encryption algorithm option for legacy application interoperability.
   - Gap analysis: This is an API reference rather than an interactive tool. gizza should help users interop with old application data while making the legacy/security warning prominent.

5. OpenSSL / legacy CLI interoperability
   - URL: https://www.openssl.org/docs/manmaster/man1/openssl-enc.html
   - Notes: CLI crypto tooling is often used for legacy ciphertext handling, but RC2 availability can depend on provider/build configuration and command-line flags.
   - Gap analysis: gizza offers a portable Rust/WASM implementation with browser, CLI, and chat surfaces. It does not attempt password-based key derivation compatibility with every legacy container format.

## Fit-to-model decisions

Built in model:
- Pure Rust core using the RustCrypto `rc2` crate.
- RFC 2268 single-block test vectors for ECB/raw block validation.
- CBC and ECB modes with PKCS#7 padding for text input.
- Configurable effective key bits (`0` defaults to the supplied key length).
- Hex/base64 encoding for key, IV, and ciphertext.
- Chat schema drift guard, browser wrapper, page content, CLI usage, and Playwright page tests.
- Security copy warning that RC2 is legacy-only and AES/passphrase tools should be used for new encryption.

Intentionally not built / out of model:
- Authenticated encryption: RC2 cannot provide modern AEAD security.
- Password-based key derivation formats for PKCS#12/S/MIME containers.
- Arbitrary binary plaintext page editing; this tool treats plaintext as UTF-8 text.
- Recipe-graph UI like CyberChef.

## Verification snapshot

- `cargo test --workspace` in `blocks/rc2-cipher/`: passed.
- `wafer build` in `blocks/rc2-cipher/`: passed and produced `target/block.wasm`.
- `wasm-pack build blocks/rc2-cipher/web --target web --release --out-dir pkg`: passed.
- `cargo install --path cli`: passed.
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`: passed and rendered `tools/rc2-cipher/`.
- CLI surface: `gizza tool rc2-cipher ...` encrypted and decrypted `attack at dawn` with RC2-CBC hex parameters.
- Playwright page test: `xvfb-run npx playwright test tool-page-rc2-cipher.spec.ts` passed (3 tests).
