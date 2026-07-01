# bech32-codec competitor analysis (2026-07-01)

## Tool

`bech32-codec` encodes a human-readable prefix (HRP) plus byte data into Bech32 (BIP 173) or Bech32m (BIP 350), and decodes/validates Bech32 strings back to HRP, checksum variant, and data.

## Competitor snapshot

1. IO Tools — Bech32 Address Encoder/Decoder
   - Positioning: web tool for Bitcoin SegWit Bech32 / Bech32m validation.
   - Strengths: explicit BIP 173 / BIP 350 language and address-validation framing.
   - Gaps vs gizza: narrower address-oriented copy; gizza exposes the generic HRP + payload primitive and supports hex/text payload rendering.

2. `bech32` npm package
   - Positioning: developer library for BIP 173 / BIP 350 encoding and decoding.
   - Strengths: mature package, handles Bech32 and Bech32m, suitable for application integration.
   - Gaps vs gizza: requires code/package installation; no browser-local UI, CLI wrapper, or copyable result surface for non-developers.

3. Ox `Bech32m` API docs
   - Positioning: TypeScript API documentation for Bech32m encode/decode.
   - Strengths: concise developer examples and typed byte-array API.
   - Gaps vs gizza: Bech32m-focused library documentation, not an interactive converter; does not cover Bech32 and Bech32m side-by-side in a UI.

4. BIP 350 reference documentation
   - Positioning: normative specification for Bech32m and witness-address rules.
   - Strengths: authoritative checksum constant, design rationale, and compatibility guidance.
   - Gaps vs gizza: specification text rather than an interactive encoder/decoder.

5. HexDocs `bip0173_0350` Bech32 module
   - Positioning: Elixir package docs for Bech32/Bech32m encode/decode.
   - Strengths: developer-facing API for both variants.
   - Gaps vs gizza: library documentation, not a tool UI; no end-user explanations for HRP, data format, checksum failures, or privacy.

## Gap decisions

Built / retained in-model:

- Both checksum variants: Bech32 and Bech32m.
- Encode and decode modes in one tool.
- Decode auto-detects the checksum variant and reports it.
- Hex and UTF-8 text payload modes.
- HRP validation, printable-ASCII enforcement, mixed-case rejection, Bech32 alphabet validation, checksum mismatch errors, and invalid padding detection.
- Browser-local page with query-param deep links, CLI surface, and chat/block surface.
- Page copy explains HRP, checksum variants, examples, Nostr/Lightning/SegWit use cases, and privacy.

Out-of-model / intentionally not built:

- Full Bitcoin address construction/validation by witness version and network. That is a higher-level address tool; this tool deliberately exposes the generic Bech32 primitive.
- Nostr-specific `npub` / `nsec` semantic parsing beyond decoding raw payload bytes.
- Wallet/network lookups or blockchain validation. The tool is deterministic and local only.

## Verification snapshot

- `cargo test --workspace` from `blocks/bech32-codec/`: passed.
- `wafer build` from `blocks/bech32-codec/`: passed and produced `target/block.wasm`.
- `wasm-pack build blocks/bech32-codec/web --target web --release --out-dir pkg`: passed.
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`: passed and rendered `/tools/bech32-codec/`.
- `cargo install --path cli`: passed.
- CLI encode/decode smoke tests using `gizza tool bech32-codec`: passed.
- `cd tests && xvfb-run npx playwright test tool-page-bech32-codec.spec.ts`: 5 passed.
