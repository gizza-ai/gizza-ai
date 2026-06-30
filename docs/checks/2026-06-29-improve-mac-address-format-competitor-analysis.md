# mac-address-format — competitor analysis & surface checks (2026-06-29)

**Tool:** `mac-address-format` — reformat one or many EUI-48/EUI-64 MAC addresses between colon, hyphen, Cisco dotted, and bare hex notations with chosen upper/lower case. Pure Rust, runs on chat block, CLI, and browser page.

## Surface verification (all green)

| Surface | Check | Result |
| --- | --- | --- |
| Core + descriptor tests | `cd blocks/mac-address-format && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 10 core tests + 1 drift-guard schema test pass |
| Chat block (wasm32-wasip1) | `cd blocks/mac-address-format && CARGO_BUILD_JOBS=1 wafer build` | ✅ OK, `target/block.wasm` validates/instantiates (298.5 KiB) |
| Page wasm (wasm32-unknown-unknown) | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/mac-address-format/web --target web --release --out-dir pkg` | ✅ pkg built |
| CLI | `gizza tool mac-address-format input=... format=colon/cisco case=lower/upper` | ✅ outputs colon/lower and Cisco/upper forms |
| Page generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `tools/mac-address-format/` |
| Page (Playwright) | `tool-page-mac-address-format.spec.ts` | ✅ 4 passed |

The chat schema is single-sourced from `descriptor()` and locked by the `schema_json_matches_authored_chat_schema` drift test.

## Competitor landscape

Top MAC format tools users reach for:

1. **SubnettingCalculator MAC Address Converter** — Cisco dots, Windows hyphens, Linux colons.
2. **Ops Box MAC Address Formatter** — colon, hyphen, Cisco dot notation, raw/bare formats with bulk conversion.
3. **OpsCanopy MAC Address Formatter** — browser formatter with detection plus extra bit/OUI/EUI features.
4. **VSPIC MAC Address Converter** — textarea conversion between colon, hyphen, dot, and bare.
5. **calculate.co.nz MAC Address Format Converter** — common notations and upper/lower case.

## Capability diff

| Capability | Competitors | gizza mac-address-format |
| --- | --- | --- |
| Colon notation | all | ✅ |
| Hyphen notation | all | ✅ |
| Cisco dotted notation | all | ✅ |
| Bare hex notation | most | ✅ |
| Upper/lower case | some | ✅ |
| Multiple addresses / bulk | some | ✅ whitespace/comma/semicolon separated |
| Preserve order and duplicates | varies | ✅ 1:1 reformatter |
| EUI-64 support | some advanced tools | ✅ 16-hex-digit addresses |
| Vendor/OUI lookup | some advanced tools | ❌ separate existing `mac-vendor-lookup` tool |
| Scan free text for MACs | extractor tools | ❌ separate existing `extract-mac-addresses` tool |

## In-model gaps closed / confirmed

- Added validation for exactly 12 (EUI-48) or 16 (EUI-64) hex digits after accepted separators.
- Added all four common output styles: colon, hyphen, Cisco dotted, and bare.
- Added case control and bulk input handling while preserving input order and duplicates.
- Added page tests for default formatting, Cisco uppercase, multiple addresses, and invalid input.
- Clarified scope versus nearby existing tools: this is a deterministic formatter, not vendor lookup or free-text extraction.

## Out-of-model (intentionally not built)

- **Vendor/OUI enrichment** — already handled by `mac-vendor-lookup` and requires vendor data.
- **Free-text extraction/deduplication** — already handled by `extract-mac-addresses`; this formatter expects every token to be a MAC address.
- **IPv6 link-local/EUI-64 address generation** — useful but a separate network-calculation task.

No competitor copy, branding, or assets were used.
