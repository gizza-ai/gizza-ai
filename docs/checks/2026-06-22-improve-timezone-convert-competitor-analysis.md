# timezone-convert — competitor analysis (2026-06-22)

Tool: convert a wall-clock date/time from one IANA timezone to another, with
correct daylight-saving (DST) handling. Pure-Rust (`chrono` + `chrono-tz`), runs
in-browser / chat / CLI. Output is a JSON object with the converted time, both
offsets, the hour/minute difference, target weekday, a DST flag, and the Unix
timestamp.

## Top competitors surveyed

1. **timeanddate.com — Time Zone Converter** — the canonical reference. Pick a
   date/time + two cities, get the converted time and the difference. Strong city
   autocomplete, "best meeting time" planner across many zones, calendar UI.
2. **WorldTimeBuddy** — visual multi-zone slider; great for picking a meeting
   slot across 3-10 zones at once. Browser-based, freemium.
3. **dateful.com / Time Zone Converter** — simple two-zone convert with a slider
   and shareable links; supports an arbitrary date.
4. **savvytime.com** — two/multi-zone convert, embeds, city search.
5. **Google search "3pm EST to PST"** — inline one-shot answer; no date control,
   assumes "today", limited to common abbreviations.

## Capability diff (what they have vs. this tool)

| Capability | Competitors | This tool | Verdict |
| --- | --- | --- | --- |
| Two-zone convert at an explicit date/time | yes | yes | parity |
| Full IANA tz database | yes | yes (`chrono-tz`) | parity |
| Correct DST (incl. rule changes by date) | yes | yes | parity |
| Half/45-min zones (IST +5:30, NPT +5:45) | yes | yes (verified IST) | parity |
| Show both UTC offsets | yes | yes (`from_offset`/`to_offset`) | parity |
| Show the difference between zones | yes | yes (hours + minutes) | parity |
| Target weekday / "next day" cue | yes | yes (`to_weekday`, date rolls over) | parity |
| Flag whether result is in DST | partial | yes (`to_is_dst`) | **ahead** |
| Reject non-existent spring-forward times | rare | yes (explicit error) | **ahead** |
| Unix timestamp of the instant | rare | yes (`unix`) | **ahead** |
| Shareable deep-link (query params) | yes | yes (page `?datetime&from&to`) | parity |
| 100% local / private / offline | no (server) | yes | **ahead** |

## Gaps considered — and decisions

- **City-name input + autocomplete** (e.g. "New York" → `America/New_York`):
  a UX nicety. Out of scope for the core compute model here — the tool takes
  canonical IANA names, which the chat LLM and the page placeholder/content.md
  guide the user toward. A full city→zone alias table is a large data set best
  added later as a separate lookup; not building it now. Documented in
  content.md with a region→IANA table covering the common cities.
- **Multi-zone / meeting planner** (one time shown across N zones at once): genuinely
  useful but a different IO shape (one input → a list of zones). The descriptor
  model is one `from` → one `to`. Left as a possible future `timezone-table` tool
  rather than overloading this one. NOT a gap to close here.
- **Visual time slider** (WorldTimeBuddy/dateful): interactive-UI feature, not a
  compute capability; the gizza page is a single-shot input→output renderer. Out
  of model.
- **"Now" / current-time default**: the page/chat take an explicit datetime so
  the conversion is deterministic and reproducible (the core has no clock, per the
  page recompute-on-input model). A user types the time they care about; this is
  the right call for a converter (vs. a clock tool, which already exists).

## Copy / UX work done

- SEO title/description/tags target "timezone converter", "convert time between
  time zones", "UTC converter", "DST", "IANA timezone", "meeting planner".
- content.md documents the input format (no embedded Z/offset — zone comes from
  the field), a region→IANA name table for the common cities, the DST-gap
  behavior, and a worked NY→Tokyo example.
- Error messages name valid IANA examples and explain the spring-forward gap.

## Conclusion

The tool is at capability parity with the mainstream two-zone converters on the
core job (explicit date/time, full IANA DST-aware conversion, offsets +
difference + weekday), and ahead on the DST flag, non-existent-time rejection,
Unix timestamp, and full local/offline privacy. The two real competitor features
not built — city autocomplete and a multi-zone meeting planner — are out of the
single-input→single-output compute model and are noted as future, separate tools
rather than partial additions here. No in-model gaps remained to close.

## Verification (all surfaces)

- `cargo test --workspace` — 10 core tests + 1 drift-guard schema test pass.
- `wafer build` — chat `block.wasm` validates + instantiates (chrono-tz is
  wasm32-wasip1 safe).
- `wasm-pack build .../web` — page wasm built.
- CLI: `gizza tool timezone-convert datetime="2024-01-10 14:30"
  from="America/New_York" to="Asia/Tokyo"` → `2024-01-11T04:30:00+09:00`; unknown
  zone returns a helpful error.
- Playwright (`tool-page-timezone-convert.spec.ts`): 4/4 pass — NY→Tokyo,
  India half-hour zone, unknown-zone error, query-param deep-link.
