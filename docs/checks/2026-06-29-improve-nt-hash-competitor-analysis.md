# nt-hash — competitor analysis & surface checks (2026-06-29)

**Tool:** `nt-hash` — compute the Windows NT / NTLM hash: `MD4(UTF-16LE(password))`.

## Surface checks

| Surface | Check | Result |
| --- | --- | --- |
| Core/workspace tests | `cd blocks/nt-hash && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 9 tests passed (descriptor drift guard + NTLM vectors/options) |
| Chat block | `cd blocks/nt-hash && CARGO_BUILD_JOBS=1 wafer build` | ✅ produced and validated `target/block.wasm` |
| Web wasm | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/nt-hash/web --target web --release --out-dir pkg` | ✅ built `web/pkg` |
| Generator | `CARGO_BUILD_JOBS=1 cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `/tools/nt-hash/` |
| CLI | `gizza tool nt-hash password=password`, uppercase, and base64 variants | ✅ returned `8846f7eaee8fb117ad06bdd830b7586c`, uppercase, and `iEb36u6PsRetBr3YMLdYbA==` |
| Page | `cd tests && xvfb-run npx playwright test tool-page-nt-hash.spec.ts` | ✅ 4 passed (hex, uppercase, base64, query-param deep-link) |

## Competitor scan

Searches reviewed:
- `online NTLM hash generator NT hash md4 utf-16le competitors`
- `NTLM hash generator online password nt hash`

Representative competitors and references:

1. **Browserling NTLM Hash** — simple free online NTLM password hash generator.
2. **IPVoid NTLM Generator** — calculates Microsoft's NT LAN Manager hash from any string in a browser.
3. **CodeBeautify NTLM Hash Generator** — web NTLM hash generator with explanatory copy.
4. **TestMu / LambdaTest NTLM Hash Generator** — free online NTLM hash generator page.
5. **General hash-generator sites** — many support MD5/SHA families; fewer explicitly support NTLM/UTF-16LE+MD4.

## Gap / fit analysis

| Capability | Competitors | gizza `nt-hash` | Decision |
| --- | --- | --- | --- |
| NTLM vector correctness | Dedicated generators output the canonical 32-char NTLM hash | ✅ `password` → `8846f7eaee8fb117ad06bdd830b7586c`; empty and `123456` vectors covered | Built |
| Correct UTF-16LE input encoding | NT hash requires UTF-16LE before MD4 | ✅ uses Rust `encode_utf16()` and little-endian bytes, including surrogate pairs | Built |
| Hex output | Universal baseline | ✅ lowercase hex default; uppercase option | Built |
| Base64 output | Less common but useful for binary digest transport | ✅ optional `output_format=base64` | Built |
| Warnings / safe guidance | Security tools often explain NTLM weaknesses | ✅ page and chat description warn NT hash is unsalted/MD4-broken and not for password storage | Built |
| Crack / reverse lookup | Some hash sites offer cracking/reversing | ❌ out-of-model and unsafe for this local generator; no lookup/cracking | Not built |
| NTLMv1/v2 response generation | Some penetration-testing workflows need challenge/response | ❌ distinct protocol tool; this one only computes NTOWF | Not built |
| Privacy/offline | Some competitors are web-hosted; privacy varies | ✅ pure wasm/page + CLI/chat; no network required | Built |

## Improvements made from analysis

- Implemented the exact NT hash definition, not plain MD4 of UTF-8 text.
- Added published/common test vectors and page coverage for default, uppercase, base64, and deep-link behavior.
- Included strong copy warning users away from using NT hashes for new password storage, with pointers to Argon2/bcrypt tools.
