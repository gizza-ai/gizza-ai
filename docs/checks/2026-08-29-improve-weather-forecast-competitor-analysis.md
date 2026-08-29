# weather-forecast — competitor analysis (2026-08-29)

Scan run **before** implementing, per the create-next-tool recipe. Everything below is a
paraphrased description of publicly documented behaviour. **No competitor copy, branding, or
trademarks are reproduced or reused** — only capability/parameter facts, used to decide our own
parameter surface.

## Scan

One WebSearch (`weather forecast API tool current conditions multi-day forecast CLI parameters
units`), then three reachable competitor tools skimmed in full:

| # | Competitor | Shape | What it is |
|---|---|---|---|
| 1 | Open-Meteo forecast + geocoding API (`open-meteo.com/en/docs`) | key-less JSON API | The upstream we build on. Also the reference for *which* variables a forecast tool is expected to expose. |
| 2 | wttr.in (`github.com/chubin/wttr.in`) | console/URL weather tool | The canonical "one command, get the weather" tool — the closest UX analogue to a gizza CLI/chat block. |
| 3 | Visual Crossing Timeline Weather API (`visualcrossing.com/resources/documentation/weather-api/timeline-weather-api/`) | commercial JSON API | Reference for location-string flexibility, unit *groups*, and default day counts. |

(The search also surfaced OpenWeatherMap One Call 3.0, Weatherbit and The Weather Company Daily
Forecast; their documented parameter sets are consistent with the three above — `units`
standard/metric/imperial, 3/5/7/10/15-day durations, current+hourly+daily in one payload — and
are folded into the table-stakes list rather than skimmed separately.)

## Table-stakes matrix

| # | Table stake | Seen in | Fit | Where it lands |
|---|---|---|---|---|
| 1 | Look a place up by **name** (city/town), not just coordinates | wttr.in, Visual Crossing | in-model | `location` param — geocoded via Open-Meteo's key-less geocoding API |
| 2 | Accept **raw coordinates** in the same field | wttr.in, Visual Crossing (`lat,lon`) | in-model | `location` also accepts `"52.52,13.41"`; geocoding is skipped |
| 3 | **Disambiguate** ambiguous names (Berlin DE vs Berlin US; Springfield ×20) | Visual Crossing (address parsing) | in-model | `"Berlin, DE"` / `"Springfield, IL"` qualifier after a comma, plus an explicit `country` filter; 10 candidates are fetched and filtered locally |
| 4 | **Unit groups**, not per-field unit flags | Visual Crossing (`unitGroup=us/uk/metric`), wttr.in (`?u`/`?m`), OWM (`units=`) | in-model | `units` = `metric` \| `us` \| `uk` (`Param::enumv`) → maps to Open-Meteo `temperature_unit`/`wind_speed_unit`/`precipitation_unit` |
| 5 | **Current conditions** block (temp, feels-like, humidity, wind, pressure, cloud) | all three | in-model | always returned in `current` |
| 6 | **Multi-day daily forecast** with a caller-chosen day count | Weather Company (3/5/7/10/15), Open-Meteo (`forecast_days` 0–16) | in-model | `days` integer 1–16, default 7 |
| 7 | **Hourly** detail alongside daily | OWM One Call, Weatherbit, Visual Crossing (`include=hours`) | in-model | `hours` integer 0–48, default 0 (opt-in); sliced forward from the current hour, not from local midnight |
| 8 | **Plain-language condition text**, not a bare numeric code | wttr.in, Visual Crossing (`conditions`) | in-model | WMO code → text decoded locally (full 0–99 table), on `current`, each day, and each hour |
| 9 | **Local time** at the location (sunrise/sunset only make sense in local time) | all three | in-model | `timezone` param, default `auto` (Open-Meteo resolves the location's zone); accepts an explicit IANA name |
| 10 | Sunrise/sunset, UV index, precipitation probability, wind gusts, dominant wind direction | Visual Crossing, OWM | in-model | daily fields |
| 11 | **Compass wind direction**, not only degrees | wttr.in | in-model | 16-point cardinal derived locally (`wind_direction_cardinal`) |
| 12 | **One-line summary** suitable for a terminal / chat reply | wttr.in (`format=1..4`, `%`-notation) | in-model (partially) | a `summary` string is always returned; a user-defined `%`-format template is **not** built (see below) |
| 13 | Echo **which** place was actually resolved | Visual Crossing (`resolvedAddress`) | in-model | `location` object: name, admin1, country, country_code, lat/lon, elevation, timezone |

## Out-of-model / deliberately not built

Listed, not built — each is recorded so it is never silently dropped.

- **Weather alerts / severe-weather warnings** (OWM One Call, Visual Crossing). Open-Meteo has no
  alerts feed; every provider that has one requires an API key. gizza tools must stay key-less.
- **Moon phase** (wttr.in `/Moon`, Visual Crossing `moonphase`). Not in the Open-Meteo forecast
  response. Computable locally, but it is a *different* tool (astronomy, not forecast) and would
  bloat this schema — a candidate backlog row, not a param here.
- **Localised condition text in 74 languages** (wttr.in). Would need a translation table per
  language; the WMO decode table here is English-only.
- **ASCII-art / PNG / Prometheus output modes** (wttr.in). gizza blocks return one structured JSON
  envelope by contract; presentation belongs to the caller. The `summary` string covers the
  "one line I can print" need.
- **User-supplied one-line format templates** (`format=%l:+%c+%t`). Deliberately skipped: a
  mini-template language is a large, under-specified surface for a first version; `summary` plus
  the structured fields cover the same ground for an LLM or a `jq` pipeline.
- **Air quality / pollen / marine** (Open-Meteo ships these on *separate* endpoints). Separate
  tools, not extra params here.
- **Historical / past days** (`past_days`, Visual Crossing date ranges). This row is scoped
  "current conditions and a multi-day forecast"; historical archive is a distinct backlog tool.
- **Airport (IATA) codes and `@domain` / IP geolocation as location inputs** (wttr.in). Both need
  extra lookup datasets/services (an IATA table; an IP-geolocation service). Out of scope for a
  key-less block; city names and coordinates cover the same intent.

## Surface decisions (why this is a no-page block)

`weather-forecast` is a **network** block: chat + CLI, **no page**, following the documented
network shape (`new-tool/SKILL.md` step 3) and the existing precedent
`blocks/password-pwned-check`, `blocks/web-fetch`, `blocks/http-request`, `blocks/graphql-introspect`
— none of which ship a `page/`.

This was checked, not assumed: the shared page driver calls the browser wasm export
**synchronously** (`tools/generator/assets/runtime/tool.js`: `const result = fn(...gatherArgs());
showResult(result)`), so an `async`/`Promise`-returning export would render as `[object Promise]`.
Supporting a fetching page would mean changing the shared runtime for every tool in the repo —
a platform change, out of scope for one tool build. The hygiene gate's check 9 (a block that calls
`network::do_request` *and* ships a page must set `network = true` in `page/meta.toml`) is
forward-looking; no block in the repo ships a network page today.

Consequences, stated rather than glossed:

- **No Playwright spec** for this tool — there is no page to drive. The verifiable surfaces are the
  descriptor/schema (drift-guard unit test — the same schema chat consumes), the host-side unit
  tests, and the CLI.
- The `?param=` deep-link case and the page-QA rubric in the recipe are page-only and therefore N/A
  here; the CLI exact-output case stands in as the end-to-end assertion.

## Provider terms

Open-Meteo's forecast and geocoding APIs are free and key-less for non-commercial use; commercial
use is via a customer endpoint plus `apikey`. The block never sends a key and never sends anything
beyond the resolved place name/coordinates and the caller's unit/day choices. The page copy rule is
moot here (no page), but the same brand-free rule was applied to all block copy.

## Result

All 13 table stakes are in the descriptor. Eight capabilities are explicitly listed as
out-of-model above. Nothing from the scan was dropped silently.
