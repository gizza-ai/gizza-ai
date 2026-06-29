# sm4-cipher — competitor analysis & surface checks (2026-06-29)

**Tool:** `sm4-cipher` — encrypt or decrypt UTF-8 text with the SM4 block cipher (GB/T 32907-2016 / GM/T 0002) in CBC or ECB mode, with hex or base64 key/IV/ciphertext encoding.

## Surface verification

| Surface | Check | Result |
| --- | --- | --- |
| Core + drift tests | `cd blocks/sm4-cipher && cargo test --workspace` | Covers CBC/ECB round-trips, GB/T single-block known-answer vector, error handling, and descriptor schema drift. |
| Chat block | `cd blocks/sm4-cipher && wafer build` | Validates the wasm32-wasip1 block instantiates. |
| Page wasm | `wasm-pack build blocks/sm4-cipher/web --target web --release --out-dir pkg` | Builds the browser wrapper. |
| CLI | `gizza tool sm4-cipher ...` | Verifies CBC encryption/decryption with the public test key/IV. |
| Page | `xvfb-run npx playwright test tool-page-sm4-cipher.spec.ts` | Verifies in-browser CBC encrypt→decrypt. |

## Competitor landscape

1. **OpenSSL / gmssl / Tongsuo CLIs** — reliable native implementations, but require local installation and correct command-line flags; SM4 support depends on OpenSSL version/provider configuration. They are byte-oriented and not friendly for quick paste-in text workflows.
2. **CyberChef** — broad crypto workbench with SM4 operations in some deployments/forks, but its UI mixes many encodings and operations; users still need to manage key/IV representation carefully.
3. **Online SM4 encrypt/decrypt pages** — common in Chinese-language developer tooling, but many send plaintext/key material to a server or provide unclear privacy guarantees; output encodings and padding defaults vary.
4. **Language libraries (`gmssl`, `sm-crypto`, RustCrypto `sm4`)** — suitable for application code but require writing snippets and manually handling PKCS#7 padding, mode selection, and hex/base64 conversion.

## Capability diff

| Capability | Competitors | gizza sm4-cipher |
| --- | --- | --- |
| SM4 block cipher | all | ✅ RustCrypto `sm4` |
| CBC mode | all | ✅ with required 16-byte IV |
| ECB mode | most | ✅ for compatibility/testing |
| PKCS#7 padding | common | ✅ encrypt/decrypt text payloads that are not block-aligned |
| Hex inputs/outputs | all | ✅ |
| Base64 inputs/outputs | many | ✅ default |
| Browser-local privacy | mixed | ✅ page wasm; no upload |
| Chat + CLI + page surfaces | rare | ✅ consistent descriptor/schema |
| Known-answer vector | CLIs/libs | ✅ GB/T single-block vector pinned in unit tests |

## In-model improvements included

- Clear mode and encoding selectors (`operation`, `cipher`, `format`) instead of ambiguous free-form flags.
- Explicit key and IV length validation with actionable errors.
- CBC and ECB support with PKCS#7 padding for practical text messages.
- A public GB/T known-answer vector test for implementation correctness.
- Browser page round-trip coverage for the most common CBC workflow.

## Out-of-model / intentionally not built

- Authenticated encryption: SM4 is a block cipher; authenticated modes such as GCM/CCM are a separate design and not requested by this backlog row.
- Binary file encryption/decryption: current surfaces are text-in/text-out. File encryption belongs in a separate file/media tool.
- Server-side key storage, KMS integration, or certificate workflows: outside the local pure-WASM model.

No competitor copy, branding, or trademarks were used.
