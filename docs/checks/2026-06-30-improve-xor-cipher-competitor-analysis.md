# xor-cipher — competitor analysis & surface checks (2026-06-30)

**Tool:** `xor-cipher` — repeating-key bytewise XOR for text, hex, or Base64 data, with hex/Base64/UTF-8 output. Pure Rust and deterministic across chat block, CLI, and browser page.

## Surface verification

| Surface | Check | Result |
| --- | --- | --- |
| Core + schema tests | `cd blocks/xor-cipher && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ core tests + drift-guard schema test pass |
| Chat block | `cd blocks/xor-cipher && CARGO_BUILD_JOBS=1 wafer build` | ✅ `target/block.wasm` validates |
| Page wasm | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/xor-cipher/web --target web --release --out-dir pkg` | ✅ pkg built |
| Generator | `CARGO_BUILD_JOBS=1 cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `tools/xor-cipher/` |
| CLI | `gizza tool xor-cipher data=Hello key=K output=hex` and hex-to-UTF-8 reverse | ✅ JSON output contains `032e272724` and round-trips to `Hello` |
| Page | `cd tests && xvfb-run npx playwright test tool-page-xor-cipher.spec.ts` | ✅ page vector, round-trip, error, and deep-link coverage |

## Competitor landscape

Top comparable utilities:

1. **CyberChef XOR recipe** — strong general-purpose byte operations and encoding chains, but requires assembling the recipe manually.
2. **dCode XOR Cipher** — educational XOR encoder/decoder with text and hex-oriented modes.
3. **Cryptii / online XOR calculators** — quick browser forms for text/hex XOR with a key.
4. **CTF helper scripts and CryptoPals examples** — reproducible repeating-key XOR vectors, but not a polished web/CLI surface.
5. **General hex calculators** — can XOR bytes, but often lack repeating-key behavior and Base64/text convenience.

## Capability diff

| Capability | Competitors | gizza xor-cipher |
| --- | --- | --- |
| Repeating-key XOR | common | ✅ |
| Symmetric encrypt/decrypt workflow | common | ✅ same operation both directions |
| Text input | common | ✅ UTF-8 bytes |
| Hex input | common | ✅ whitespace-tolerant, optional `0x` prefix |
| Base64 input | some | ✅ standard Base64 |
| Text key | common | ✅ |
| Hex/Base64 key | some | ✅ |
| Hex output | common | ✅ default |
| Base64 output | some | ✅ |
| UTF-8 output | some | ✅ validates plaintext recovery |
| Known test vectors | CTF tools | ✅ CryptoPals challenge 5 + page vectors |
| Local/private execution | varies | ✅ browser + CLI + chat block |

## In-model gaps closed / confirmed

The useful stateless transform scope is covered: selectable data/key encodings, repeating-key behavior, symmetric round-trip, empty-key validation, text/hex/Base64 output formats, query-param page deep links, and CTF test-vector coverage. The implementation intentionally stays bytewise XOR, not alphabetic Vigenère/Caesar and not a secure stream cipher.

## Out-of-model / intentionally not built

- Breaking or recovering XOR keys from ciphertext is cryptanalysis and out of scope for this direct transform tool.
- File/binary upload UX is out of scope here; users can pass bytes as hex/Base64 through chat or CLI surfaces.
- Authenticated encryption is intentionally not added; users needing real cryptography should use modern authenticated cipher tools.

No competitor copy, branding, or trademarks were used.
