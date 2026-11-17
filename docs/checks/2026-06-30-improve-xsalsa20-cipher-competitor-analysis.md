# xsalsa20-cipher — competitor analysis & surface checks (2026-06-30)

**Tool:** `xsalsa20-cipher` — encrypt/decrypt UTF-8 data with XSalsa20 using a 32-byte key and 24-byte nonce; ciphertext/key/nonce encodings are hex or Base64.

## Surface verification

Re-run on 2026-11-17 for PR #240, after the review fixes. Rows marked "not re-run" keep their 2026-06-30 result.

| Surface | Check | Result |
| --- | --- | --- |
| Core tests | `cd blocks/xsalsa20-cipher && cargo test --workspace` | ✅ 10 core tests (including libsodium `crypto_stream_xsalsa20_xor_ic` known-answer vectors at counters 0, 1 and 2^60) + 3 block tests (chat-schema drift guard, counter parsing) |
| Lint | `cd blocks/xsalsa20-cipher && cargo clippy --workspace --all-targets` | ✅ clean |
| Chat block | `scripts/build-block-wasm.sh xsalsa20-cipher --check` | ✅ canonical `target/block.wasm` reproduces from the committed `Cargo.lock` |
| Page wasm | `wasm-pack build blocks/xsalsa20-cipher/web --target web --release --out-dir pkg` (wasm-pack 0.15.0) | ✅ pkg built |
| Hygiene | `python3 scripts/check-tool-hygiene.py xsalsa20-cipher` | ✅ OK |
| Generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | not re-run (2026-06-30: ✅ rendered `tools/xsalsa20-cipher/`) |
| CLI | `gizza tool xsalsa20-cipher ...` encrypt/decrypt | not re-run (2026-06-30: ✅ round trip) |
| Page | `tests/tool-page-xsalsa20-cipher.spec.ts` | run by CI (4 tests, including the libsodium known answer and malformed-counter rejection) |

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
