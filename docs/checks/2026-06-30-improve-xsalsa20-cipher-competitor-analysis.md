# xsalsa20-cipher — competitor analysis & surface checks (2026-06-30)

**Tool:** `xsalsa20-cipher` — encrypt/decrypt UTF-8 data with XSalsa20 using a 32-byte key and 24-byte nonce; ciphertext/key/nonce encodings are hex or Base64.

## Surface verification

| Surface | Check | Result |
| --- | --- | --- |
| Core + schema tests | `cd blocks/xsalsa20-cipher && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 4 core tests + 1 drift-guard schema test pass |
| Chat block | `cd blocks/xsalsa20-cipher && CARGO_BUILD_JOBS=1 wafer build` | ✅ `target/block.wasm` validates |
| Page wasm | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/xsalsa20-cipher/web --target web --release --out-dir pkg` | ✅ pkg built |
| Generator | `CARGO_BUILD_JOBS=1 cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `tools/xsalsa20-cipher/` |
| CLI | `gizza tool xsalsa20-cipher ...` encrypt/decrypt | ✅ encrypted `hello` then decrypted it |
| Page | `cd tests && xvfb-run npx playwright test tool-page-xsalsa20-cipher.spec.ts` | ✅ 3 passed |

## Competitor landscape

Comparable tools include CyberChef Salsa20/XSalsa20 recipes, libsodium examples, online stream-cipher calculators, command-line crypto snippets, and developer library docs. Most expose raw bytes and require users to manage key/nonce encodings themselves.

## Capability diff

| Capability | gizza xsalsa20-cipher |
| --- | --- |
| XSalsa20 32-byte key + 24-byte nonce | ✅ |
| Encrypt/decrypt workflow | ✅ |
| Hex and Base64 ciphertext | ✅ |
| Encoded or UTF-8 key/nonce input | ✅ |
| Counter offset | ✅ |
| Browser + CLI + chat surfaces | ✅ |
| Authentication / tamper detection | ❌ out of scope |

## Notes

XSalsa20 is distinct from the existing `salsa20-cipher` block because it uses a 24-byte nonce and derives a subkey internally. The implementation intentionally does not add authentication; users needing tamper detection should use an AEAD construction.

No competitor copy, branding, or trademarks were used.
