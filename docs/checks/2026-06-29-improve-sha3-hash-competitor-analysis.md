# SHA-3 Hash competitor analysis (2026-06-29)

Tool: `sha3-hash`

## Competitors reviewed

1. CyberChef "SHA3" operation
   - Hash input as bytes with selectable SHA3-224/256/384/512 size and chained recipe steps.
   - Powerful recipe model, but heavy UI and conflates SHA-3 with Keccak in some community recipes.
2. emn178 online hash tools (sha3 / keccak pages)
   - Per-page SHA3-256/384/512 and Keccak digests of typed text with live output.
   - Clear and fast, but separate pages per size and no encoded-input or base64-output controls in one place.
3. OpenSSL / `sha3sum` CLI (`openssl dgst -sha3-256`)
   - Authoritative FIPS-202 digests over files and stdin.
   - Command-line only; no decode-input-as-hex/base64 convenience and no shareable link.
4. Browser / Node `crypto` (`crypto.createHash('sha3-256')`)
   - Library-grade SHA-3 for developers embedding it in code.
   - Requires writing code; not a paste-and-read tool, and WebCrypto `subtle.digest` does not expose SHA-3 at all.
5. movable-type / xorbin style SHA-3 calculators
   - Single-box SHA3-256/512 of pasted text with hex output.
   - Convenient, but UTF-8 text only (no hex/base64 input), hex output only, and no Keccak-vs-SHA3 disambiguation.

## In-model gaps and actions taken

- FIPS-202 correctness: implemented standardized SHA-3 with 0x06 multi-rate padding, verified against the NIST `"abc"` vectors for SHA3-256 and SHA3-512.
- Keccak disambiguation: documented in the tool description that this is NIST SHA-3 (0x06), distinct from the original Keccak (0x01) used by Ethereum, and pointed users to the `keccak-hash` tool for Keccak-256/512.
- Variant selection: implemented `algorithm` with SHA3-256 (default), SHA3-384, and SHA3-512 in a single tool instead of separate pages.
- Input encoding: implemented `input_encoding` text / hex (leading `0x` accepted) / base64 so binary inputs can be hashed without a pre-decode step.
- Output format: implemented `output_format` hex (default) / base64, plus `uppercase` for uppercase hex, covering the common digest representations.
- Three surfaces: exposed the same parameters across the chat/LLM tool, the `gizza` CLI, and the page with `?text=&algorithm=` deep links so a result is shareable.

## Out-of-model or intentionally not implemented

- SHA3-224: omitted to keep the variant list to the three most-requested sizes; the underlying construction supports it but it is rarely used.
- SHAKE128/256 XOF: variable-length extendable output is a different primitive and is out of scope for a fixed-digest hasher.
- File/stream hashing: the tool hashes a single in-memory text/encoded input; large-file and streaming digests stay with CLI tools like `sha3sum`.
- HMAC-SHA3 / keyed hashing: keyed MACs are a separate concern and not part of a plain digest tool.
- Keccak (0x01) variants: intentionally excluded here and handled by the dedicated `keccak-hash` tool to avoid silently producing the wrong digest.

## Verification snapshot

- `cargo test --workspace` from `blocks/sha3-hash`: passed.
- `wafer build` from `blocks/sha3-hash`: passed and produced `target/block.wasm`.
- `wasm-pack build blocks/sha3-hash/web --target web --release --out-dir pkg`: passed.
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`: passed; rendered `tools/sha3-hash/`.
- `cargo install --path cli`: passed.
- `gizza tool sha3-hash text='abc'`: passed, returning `3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532`.
- `cd tests && xvfb-run npx playwright test tool-page-sha3-hash.spec.ts`: passed.
- `npm run test`: passed (41 JS tests).
