# rncryptor-encrypt — competitor analysis (2026-08-13)

Scan run before finalizing implementation. Notes are paraphrased observations only; no competitor copy, branding, or assets are reused.

## Competitor scan

1. **The reference RNCryptor implementations (Swift/Objective-C, Python, C, Erlang, Kotlin ports).** These are libraries, not tools: you call an `encrypt(data, password)` function and get the whole container back. They expose almost no knobs — the format pins AES-256-CBC, PBKDF2-HMAC-SHA1 at 10,000 iterations, 8-byte salts, a 16-byte IV, and a trailing HMAC-SHA256 — and they always use fresh random salts, so there is no way to reproduce a published test vector by hand. There is no first-party web UI and no first-party command-line front end; a developer who just wants one container has to write a script.
2. **CyberChef-style recipe workbenches.** Extremely capable but assembly-required: producing this container means chaining a PBKDF2 key-derivation step, an AES-CBC encrypt step, and an HMAC step, then concatenating header fields in the right order by hand. Every intermediate value has to be moved between steps manually, and one wrong field order silently produces a container no library will open. Strong points worth matching: every input has an explicit encoding selector, output can be shown as hex or base64, and the whole thing runs client-side.
3. **General "AES encrypt online" form tools (the devglan/anycript/base64.sh family).** These offer mode selection (ECB/CBC/CTR/GCM), key sizes 128/192/256, an optional PBKDF2-from-password path, and base64/hex output, with a strong privacy pitch about running in the browser. What they do not do is emit a self-describing container — you get bare ciphertext plus separately displayed salt/IV, so the result is not interoperable with any shipping mobile app. Several of them also hide their iteration count and salt handling, which makes results hard to reproduce.
4. **The emn178-style online crypto toolkit.** The most parameterized of the form tools: modes, padding choices, and a real KDF picker (PBKDF2, EvpKDF, HKDF, scrypt, Argon2), plus text/file/URL input and multiple output encodings. Its lesson for us is breadth of *input* handling — text, hex, and base64 in, hex or base64 out — rather than container semantics, which it does not model at all.
5. **Password-blob encryptors that ship their own private format** (including our own `encrypt-file`/`text-encrypt` blocks and the many "encrypt this text with a password" pages). Good UX, one password field, self-describing output — but the blob only round-trips through the same tool. They are the right comparison for ergonomics and the wrong one for interop, which is the entire reason this tool exists.

Net finding: there is no convenient web tool that emits a byte-exact RNCryptor v3 password container. The gap is a container-aware tool with the encoding ergonomics of the form tools and the reproducibility of the spec's own test vectors.

## Table stakes and decisions

| Capability | Fit | Decision |
| --- | --- | --- |
| Byte-exact v3 password container (`0x03 0x01 | enc salt 8 | hmac salt 8 | IV 16 | ciphertext | HMAC-SHA256 32`) | in-model | The whole point of the tool; verified against the spec's published password vectors. |
| Spec-pinned crypto parameters (AES-256-CBC + PKCS#7, PBKDF2-HMAC-SHA1 ×10,000, 32-byte keys) | in-model | Fixed, not user-settable — a "tunable" iteration count would produce containers no RNCryptor library can open. Stated on the page so nobody goes looking for the knob. |
| Fresh random salts and IV per run | in-model | Default behavior; matches every reference implementation. |
| Reproducing a published test vector | in-model | `encryption_salt`, `hmac_salt`, and `iv` accept explicit hex so a run is deterministic. This is the reproducibility gap the reference libraries leave open. |
| Round-trip / decrypt | in-model | `operation=decrypt` verifies the HMAC in constant time before unpadding, so the tool can check its own output and open containers produced by a mobile app. |
| Input encoding selector (text / hex / base64) | in-model | `data_encoding`, matching the encoding-selector ergonomics of the workbench tools. |
| Output encoding selector (base64 / hex) | in-model | `output_encoding`; base64 default because that is what gets pasted between systems. |
| Client-side only, no upload, no account | in-model | Inherent — pure wasm compute. |
| Clear failure on a bad password | in-model | HMAC mismatch reports authentication failure explicitly rather than returning garbage plaintext. |
| Key-based v3 containers (options byte `0x00`, caller supplies both 32-byte keys, no salts) | in-model but **out of scope** | A different container variant with a different input shape; folding it in would add two more key fields to a password-first form. Recorded here so it is not dropped silently — it belongs in its own block if demand appears. |
| File upload / bulk encryption of many files | out-of-model | Text-and-bytes-in, text-out tool; the file-oriented blocks cover that shape. |
| v2/v1 legacy containers | out-of-model | v3 fixed a real password-truncation bug in v2; emitting the older formats would ship a known weakness. |
| Streaming / chunked encryption for very large inputs | out-of-model | The wasm sandbox is memory-bounded; the page states a practical size limit instead. |
| Key escrow, sharing links, server-side storage | out-of-model | Requires a backend. |

## Implementation stance

The tool produces and consumes exactly the v3 password container, with the spec's parameters hard-pinned. Everything user-facing is about getting bytes in and out cleanly (encoding selectors, an explicit salt/IV path for reproducibility) rather than about tuning cryptography, because tuning is precisely what breaks interoperability here.
