# rsa-encrypt — competitor analysis & surface checks (2026-06-29)

**Tool:** `rsa-encrypt` — encrypt a short plaintext message to an RSA public key and return base64 ciphertext.

## Surface checks

| Surface | Check | Result |
| --- | --- | --- |
| Core/unit | `CARGO_BUILD_JOBS=1 cargo test --workspace` in `blocks/rsa-encrypt` | ✅ 7 tests passed (schema drift + OAEP/PKCS#1 round trips/errors) |
| Chat block | `CARGO_BUILD_JOBS=1 wafer build` in `blocks/rsa-encrypt` | ✅ `target/block.wasm` validated |
| Web wasm | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/rsa-encrypt/web --target web --release --out-dir pkg` | ✅ `web/pkg` generated |
| Page generator | `CARGO_BUILD_JOBS=1 cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `tools/rsa-encrypt/` |
| CLI | `gizza tool rsa-encrypt message='hello rsa' public_key=… padding=oaep hash=sha256` | ✅ returned JSON with base64 RSA-2048 ciphertext |
| Page | `xvfb-run npx playwright test tool-page-rsa-encrypt.spec.ts` | ✅ 2 passed (success + bad-key error) |

## Competitors reviewed

1. **Devglan RSA Encryption and Decryption Online Tool** — combined encrypt/decrypt flow with key-size and cipher options, including OAEP SHA variants and legacy modes.
2. **emn178 Online Tools RSA Encryption** — focused RSA encrypt page supporting PKCS#1/OAEP and multiple hash algorithms.
3. **CodeToolTip RSA Encryption/Decryption** — client-side RSA page emphasizing PEM/base64 compatibility and privacy.
4. **8gwifi RSA functions** — broader RSA functions page with key generation, encrypt/decrypt, signing, and selectable ciphers.
5. **JavaInUse RSA generator/tool** — educational RSA key generation/encrypt/decrypt utility.

## Gap analysis

| Capability | Competitors | gizza `rsa-encrypt` | Decision |
| --- | --- | --- | --- |
| Client-side encryption | Some competitors advertise client-side/private operation | ✅ Browser wasm and CLI run locally | Kept explicit privacy copy in page metadata |
| OAEP support | Common in Devglan/emn178/8gwifi | ✅ OAEP default | Default is `oaep` |
| OAEP hash choice | Common tools expose SHA choices | ✅ `sha256`, `sha384`, `sha512` | Implemented enum param |
| Legacy PKCS#1 v1.5 | Common for compatibility | ✅ `pkcs1v15` option | Implemented, labelled legacy in descriptor |
| PEM key input | All major competitors accept PEM | ✅ SPKI and PKCS#1 public PEM accepted | Implemented parser fallback |
| Base64 ciphertext | Common output format | ✅ JSON/page text base64 | Implemented |
| Decryption | Many competitors combine encrypt/decrypt | ❌ Not in this tool | Out of scope; repo already separates tools by operation and private-key decryption is a different capability |
| Key generation | 8gwifi/JavaInUse include generation | ❌ Not in this tool | Out of scope; key generation belongs in separate generator tools |
| Hybrid encryption for large messages | Rarely exposed clearly | ❌ Direct RSA block only | Descriptor warns about RSA block-size limits |

## Improvements made from analysis

- Defaulted to modern OAEP while retaining `pkcs1v15` for compatibility.
- Added SHA-256/384/512 OAEP hash selection.
- Accepted both common public key PEM wrappers: `BEGIN PUBLIC KEY` and `BEGIN RSA PUBLIC KEY`.
- Added clear message-size/key-format errors and page coverage for bad-key feedback.
- Added privacy and block-size guidance in descriptor/page copy without copying competitor wording or branding.
