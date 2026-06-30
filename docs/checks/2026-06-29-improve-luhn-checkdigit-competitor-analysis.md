# luhn-checkdigit — competitor analysis & surface checks (2026-06-29)

**Tool:** `luhn-checkdigit` — compute the Luhn/mod-10 check digit for a partial number (payload without its check digit) and return the completed valid number. Pure Rust, no dependencies, runs on chat block, CLI, and browser page.

## Surface verification (all green)

| Surface | Check | Result |
| --- | --- | --- |
| Core + descriptor tests | `cd blocks/luhn-checkdigit && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 7 core tests + 1 drift-guard schema test pass |
| Chat block (wasm32-wasip1) | `cd blocks/luhn-checkdigit && CARGO_BUILD_JOBS=1 wafer build` | ✅ OK, `target/block.wasm` validates/instantiates (289.4 KiB) |
| Page wasm (wasm32-unknown-unknown) | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/luhn-checkdigit/web --target web --release --out-dir pkg` | ✅ pkg built |
| CLI | `gizza tool luhn-checkdigit number=424242424242424` and `number=7992739871` | ✅ returns check digits 2 and 3, with completed numbers |
| Page generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `tools/luhn-checkdigit/` |
| Page (Playwright) | `tool-page-luhn-checkdigit.spec.ts` | ✅ 2 passed |

The chat schema is single-sourced from `descriptor()` and locked by the `schema_json_matches_authored_chat_schema` drift test.

## Competitor landscape

Top Luhn/check-digit tools users reach for:

1. **SimplyCalc Luhn calculate check digit** — focused web calculator for computing a Luhn check digit from an input number.
2. **Good Calculators Luhn Algorithm Calculator** — combined validation/check-digit calculator.
3. **PlanetCalc Luhn algorithm** — online calculator with explanatory text and checksum behaviour.
4. **MyMathTables Luhn Algorithm / Modulus 10 Calculator** — validation plus calculation surface for card-like numbers.
5. **Wikipedia/reference implementations** — algorithm explanation and canonical example `7992739871 → 3`.

## Capability diff

| Capability | Competitors | gizza luhn-checkdigit |
| --- | --- | --- |
| Compute missing Luhn check digit | all focused calculators | ✅ |
| Return completed full number | most | ✅ |
| Ignore formatting separators | some | ✅ spaces, tabs, underscores, dashes |
| Explain generator vs validator distinction | varies | ✅ page copy links to `luhn-validate` |
| Canonical known examples | reference docs | ✅ unit tests include `7992739871 → 3` and test-card prefix |
| Local/private browser computation | varies | ✅ wasm page, no upload |
| CLI + chat/LLM API | rare | ✅ `gizza tool` and chat block |
| Generate valid real cards/IMEIs from issuer rules | some card-specific generators | ❌ out of model; Luhn only |
| Bulk CSV generation | some data generators | ❌ out of model for this focused calculator |

## In-model gaps closed / confirmed

- Shipped the distinct generator counterpart to existing `luhn-validate`: every input digit is payload and the tool appends the missing check digit.
- Added robust input cleaning for common separators while rejecting unexpected characters.
- Returned structured chat/CLI fields: `check_digit`, cleaned `payload`, `full_number`, and `length`.
- Added browser page copy clarifying that Luhn validity only catches typos and does not imply a real/active card.
- Added Playwright coverage for direct entry and query-param deep links.

## Out-of-model (intentionally not built)

- **Issuer/network-specific card generation** — requires BIN/IIN rules and can be misused; this tool only computes a checksum digit for user-provided payload.
- **Bulk fake-data generation** — handled by other generator-style tools; this tool stays focused and deterministic.
- **Validation of complete numbers** — already covered by `/tools/luhn-validate/`; this tool computes the missing digit.

No competitor copy, branding, or assets were used.
