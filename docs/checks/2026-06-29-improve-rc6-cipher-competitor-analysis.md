# Competitor analysis: rc6-cipher

Date: 2026-06-29
Tool: `gizza-ai/rc6-cipher`

## Goal

Provide an RC6-32/20 encrypt/decrypt tool for chat, CLI, and browser page surfaces. Support ECB/CBC modes, PKCS#7 padding, and hex/base64 key, IV, and ciphertext encoding. Position RC6 as an interoperability, learning, and CTF tool rather than a recommended modern default.

## Competitors reviewed

1. CodeTools RC Cipher Suite
   - URL: https://www.codertools.net/tools/rc-cipher.php
   - Notes: Search result describes a browser RC family tool covering RC2, RC4, RC5, and RC6.
   - Gap analysis: gizza focuses on a schema-backed single RC6 tool with chat and CLI surfaces, deterministic tests, and generated docs. A whole RC-family suite is out of scope for this block.

2. hiofd Online RC6 Encryption Tool
   - URL: https://tool.hiofd.com/en/rc6-encrypt/
   - Notes: Search result describes RC6 encryption with multiple modes, flexible key length, and UTF-8/base64/hex formats.
   - Gap analysis: gizza matches the core in-model needs: CBC/ECB, flexible keys, and hex/base64 I/O. It also adds chat/CLI invocation and page tests.

3. 8gwifi CipherFunctions
   - URL: https://8gwifi.org/CipherFunctions.jsp
   - Notes: Broad online symmetric-cipher tool catalog.
   - Gap analysis: The catalog breadth is out of model here; gizza's advantage is focused RC6 implementation and portable WASM/CLI/chat surfaces.

4. Online Mini Tools Encrypt Decrypt
   - URL: https://onlineminitools.com/index.php/encrypt-decrypt
   - Notes: General encryption/decryption page supporting algorithms such as AES, DES, Triple DES, Rabbit, and RC4.
   - Gap analysis: Generic encryption UX is useful, but RC6-specific support is not guaranteed. gizza provides a dedicated RC6 page with explicit mode/key/IV/format controls.

5. NIST AES finalist materials / RC6 references
   - URL: https://csrc.nist.rip/encryption/aes/round2/conf3/presentations/rc6-presentation.pdf
   - Notes: RC6 was an AES finalist and is parameterized by word size, rounds, and key length.
   - Gap analysis: The gizza tool intentionally implements the standard RC6-32/20 variant rather than exposing every parameter, keeping the UI small and interoperable.

## Fit-to-model decisions

Built in model:
- Pure Rust RC6-32/20 core with no external crypto crate.
- ECB and CBC modes with PKCS#7 padding for UTF-8 text.
- Hex/base64 encoding for key, IV, and ciphertext.
- RFC/spec-style 128-bit-key raw block vectors plus 16/24/32-byte key round-trip tests.
- Chat schema drift guard, browser wrapper, page content, CLI usage, and Playwright page tests.
- Security copy warning that RC6 is not the modern default and AES/passphrase tools should be preferred for new encryption.

Intentionally not built / out of model:
- Arbitrary RC6 parameterization (word size/round count/block length) in the UI.
- Authenticated encryption; RC6 CBC/ECB provides no AEAD guarantees.
- Password-based key derivation/container compatibility.
- Recipe-graph UI like CyberChef.

## Verification snapshot

- `cargo test --workspace` in `blocks/rc6-cipher/`: passed.
- `wafer build` in `blocks/rc6-cipher/`: passed and produced `target/block.wasm`.
- `wasm-pack build blocks/rc6-cipher/web --target web --release --out-dir pkg`: passed.
- `cargo install --path cli`: passed.
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`: passed and rendered `tools/rc6-cipher/`.
- CLI surface: `gizza tool rc6-cipher ...` encrypted and decrypted `attack at dawn` with RC6-CBC hex parameters.
- Playwright page test: `xvfb-run npx playwright test tool-page-rc6-cipher.spec.ts` passed (3 tests).
