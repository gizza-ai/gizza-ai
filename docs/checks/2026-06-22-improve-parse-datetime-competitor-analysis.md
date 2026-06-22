# parse-datetime — competitor analysis (2026-06-22)

Tool: `gizza-ai/parse-datetime`. Parses a single date/time string (many common
formats) into structured components (year, month + name, day, weekday,
day_of_year, ISO week, hour/minute/second, UTC offset) plus a canonical ISO 8601
rendering. Pure Rust (`chrono`), runs in chat / CLI / browser page; nothing is
uploaded.

## Surfaces verified (Phase 1)

- **Chat block** — `wafer build` instantiates the wasm32-wasip1 block; schema
  single-sourced from `descriptor()` and locked by a drift-guard unit test.
- **CLI** — `gizza tool parse-datetime input=...` verified across ISO+offset,
  US slash+time, month-name, bare time, and an invalid input (clean error,
  non-zero exit).
- **Page** — Playwright (`tool-page-parse-datetime.spec.ts`, 3 specs) drives
  `/tools/parse-datetime/` for ISO-with-offset, US slash date, and bare time.
- 18 core unit tests + 1 drift-guard test pass.

## Competitors surveyed (top 5)

1. **jc `datetime_iso`** (kellyjonbrazil.github.io/jc) — CLI/lib that converts an
   ISO-8601 string to JSON with `year, month, month_num, day, weekday,
   weekday_num, hour, minute, second, ...`. Closest analog to our output shape,
   but **ISO-8601 input only**.
2. **DenCode ISO8601 Converter** (dencode.com) — web converter focused on
   ISO-8601 formatting / round-tripping with `T` separator and `Z`/`+09:00`
   offsets. Conversion-oriented, not a component breakdown.
3. **IT-Tools Date Converter** (it-tools.tech/date-converter) — converts a date
   between ISO 8601, RFC 3339, RFC 7231, Unix timestamp, etc. Format conversion,
   not field extraction.
4. **Python `dateutil.parser`** (dateutil.readthedocs.io) — very tolerant
   multi-format parser → `datetime` object; the gold standard for "parse almost
   anything", but a library, not a hosted tool.
5. **Moment.js** (momentjs.com) — JS library; flexible parsing with explicit
   format tokens. Library, requires code.

## Gap diff + ranking (fit-to-model)

Capabilities already at or above the field:

- **Multi-format input** — beats jc/DenCode/IT-Tools (ISO-only or
  conversion-only): we accept ISO 8601 / RFC 3339, RFC 2822 email dates, US
  slash (month-first, day-first when first field >12), European dotted
  (day-first), year-first, month-name (with optional time), and bare clock
  times.
- **Structured component output** — matches jc's field set and adds
  `month_name`, `day_of_year`, `iso_week`, and `utc_offset_seconds`.
- **Canonical ISO 8601** — like DenCode/IT-Tools, every result includes a
  normalized `iso8601` value.
- **Validity checking** — invalid calendar dates (e.g. Feb 30) and
  unrecognizable input are rejected with an error rather than silently coerced.
- **Privacy** — runs entirely client-side / in-sandbox; no upload, unlike hosted
  web converters.

### Gaps closed this pass

- Added `month_name`, `weekday`, `day_of_year`, and `iso_week` to the output
  (jc exposes weekday/weekday_num but not month name, day-of-year, or ISO week).
- Documented the month-first vs day-first disambiguation rule and the two-digit
  year window in both the schema description and the page copy, since ambiguous
  numeric dates are the #1 source of user confusion across all the competitors.

### Out-of-model (intentionally not built)

- **Unix-timestamp ↔ date conversion** (IT-Tools, DenCode) — that is a separate
  conversion tool; gizza already has dedicated date tools (`date-diff`,
  `extract-dates`) and a timestamp converter would be its own block, not a
  parser feature.
- **Timezone-name resolution** (e.g. `America/New_York` → offset) needs a tz
  database; chrono-tz is large and the parser's job is to read the offset the
  input carries, not to look up named zones. Deferred.
- **Relative/natural-language dates** ("next Tuesday", "3 days ago") need a
  reference "now"; this parser is deterministic and clock-free by design.
- No copying of any competitor's copy, branding, or trademarks.

## Conclusion

`parse-datetime` is distinct from the existing `extract-dates` (which scans free
text for date *mentions*) and `date-diff` (interval math): it is a single-string
**parser → structured components**. It meets or exceeds the in-scope feature set
of the surveyed tools. No further in-model gaps remain.
