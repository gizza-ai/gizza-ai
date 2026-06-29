# seconds-to-hms — competitor analysis & surface checks (2026-06-29)

**Tool:** `seconds-to-hms` — convert raw seconds into clock-style and duration formats.

## Surface checks

| Surface | Check | Result |
| --- | --- | --- |
| Core/unit | `CARGO_BUILD_JOBS=1 cargo test --workspace` in `blocks/seconds-to-hms` | ✅ 12 tests passed (schema drift + all formats/errors) |
| Chat block | `CARGO_BUILD_JOBS=1 wafer build` in `blocks/seconds-to-hms` | ✅ `target/block.wasm` validated |
| Web wasm | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/seconds-to-hms/web --target web --release --out-dir pkg` | ✅ `web/pkg` generated |
| CLI | `gizza tool seconds-to-hms seconds=5025 format=hms decimals=0` and `seconds=90061 format=iso` | ✅ returned `"01:23:45"` and `"P1DT1H1M1S"` |
| Page generator | `CARGO_BUILD_JOBS=1 cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `tools/seconds-to-hms/` |
| Page | `xvfb-run npx playwright test tool-page-seconds-to-hms.spec.ts` | ✅ 3 passed (default, alternate formats/fractional, query params/errors) |

## Competitors reviewed

1. **Inch Calculator Seconds to Time Calculator** — seconds to hours/minutes/seconds with explanatory formula.
2. **CalculatorLib Seconds to HH:MM:SS Calculator** — direct HH:MM:SS conversion.
3. **Online Mini Tools Convert Seconds to Time** — time-format conversion for programming/science duration display.
4. **CalculatorAt Seconds to Time Calculator** — simple HH:MM:SS duration output.
5. **Online Tools Convert Seconds to Time** — H:M:S / HH:MM:SS digital-clock converter.

## Gap analysis

| Capability | Competitors | gizza `seconds-to-hms` | Decision |
| --- | --- | --- | --- |
| HH:MM:SS output | Universal | ✅ default `hms` | Implemented |
| Hours beyond 24 | Common for duration calculators | ✅ hours accumulate in `hms` | Implemented and documented |
| Split days | Some calculators show day/hour/min/sec | ✅ `dhms` format | Implemented |
| Short/auto display | Some tools use H:M:S style | ✅ `auto` drops leading zero days/hours | Implemented |
| Fractional seconds | Not always supported | ✅ `decimals` 0..9 with rounding | Implemented |
| Negative durations | Rarely explicit | ✅ `-` prefix | Implemented |
| ISO-8601 duration | Rare in simple competitors | ✅ `iso` format | Added for API/interoperability use |
| Human words | Common in general duration tools | ✅ `words` format | Added for copy/paste readability |
| Date/timestamp conversion | Epoch tools handle dates | ❌ duration-only | Out of scope; this tool intentionally converts seconds as a duration, not an epoch timestamp |

## Improvements made from analysis

- Added multiple output layouts beyond the core HH:MM:SS ask: day-clock, auto-short, ISO-8601, and words.
- Added fractional-second rounding and validation for non-finite values / invalid formats.
- Added page metadata/content and Playwright coverage for generated page behavior and query-param prefill.
