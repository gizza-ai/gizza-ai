# unix-timestamp-converter — competitor analysis & surface checks (2026-06-30)

**Tool:** `unix-timestamp-converter` — convert Unix epoch timestamps to UTC date output, or parse human-readable dates back to epoch values. Pure Rust, text-in/text-out JSON, runs in chat block, CLI and browser page.

## Surface verification

| Surface | Check | Result |
| --- | --- | --- |
| Core + schema tests | `cd blocks/unix-timestamp-converter && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 12 core tests + 1 drift-guard schema test pass |
| Chat block | `cd blocks/unix-timestamp-converter && CARGO_BUILD_JOBS=1 wafer build` | ✅ OK, `target/block.wasm` validates (464.5 KiB) |
| Page wasm | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/unix-timestamp-converter/web --target web --release --out-dir pkg` | ✅ pkg built |
| Generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `tools/unix-timestamp-converter/` |
| CLI | `gizza tool unix-timestamp-converter value=1700000000` and date-with-offset to timestamp | ✅ correct UTC/timestamp JSON |
| Page | `cd tests && xvfb-run npx playwright test tool-page-unix-timestamp-converter.spec.ts` | ✅ 4 passed |

## Competitor landscape

Top comparable timestamp converters users reach for:

1. **unixtimestamp.com** — current timestamp, timestamp-to-date and date-to-timestamp conversion, local/UTC views.
2. **EpochConverter.com** — broad epoch tooling, human date fields, milliseconds, timezone explanations.
3. **timestamp.online** — quick timestamp converter with seconds/milliseconds and formatted date output.
4. **CyberChef DateTime / From UNIX Timestamp recipes** — flexible but operation-oriented; users must choose recipe steps.
5. **it-tools Unix timestamp converter** — developer-focused local web utility with timestamp/date conversion.

## Capability diff

| Capability | Competitors | gizza unix-timestamp-converter |
| --- | --- | --- |
| Seconds timestamp to date | all | ✅ |
| Milliseconds timestamp | many | ✅ auto-detect + explicit unit |
| Microseconds / nanoseconds | fewer | ✅ auto-detect + explicit unit |
| Date string to timestamp | all | ✅ via shared multi-format parser |
| ISO 8601 / RFC 3339 offsets | many | ✅ shifts to UTC |
| RFC 2822 email dates | some | ✅ |
| Month-name and slash/dotted dates | many | ✅ |
| Output seconds/millis/micros/nanos together | some | ✅ |
| Calendar breakdown (weekday, day-of-year, ISO week) | some | ✅ |
| Local/private execution | varies | ✅ chat, CLI and browser page |

## In-model gaps closed / confirmed

The useful stateless converter capabilities are included: bidirectional conversion, auto direction selection, unit auto-detection, explicit mode/unit overrides, flexible date parsing, UTC-normalized output, multiple timestamp units, and calendar breakdown fields. Returning pretty JSON keeps CLI/chat/page output precise and copyable without a custom stateful UI.

## Out-of-model / intentionally not built

- Live “current timestamp” ticking output is intentionally omitted: gizza pages are deterministic functions of inputs, not timers.
- Arbitrary timezone database conversion is left to the existing timezone-focused tools; this converter normalizes to UTC and honors numeric offsets in the input.
- Calendar UI widgets are out of scope for the generated stateless page surface.

No competitor copy, branding, or trademarks were used.
