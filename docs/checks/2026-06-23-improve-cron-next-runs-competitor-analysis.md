# cron-next-runs — competitor analysis (2026-06-23)

Tool: `blocks/cron-next-runs` — compute the next N scheduled run times for a cron
expression (5-field crontab, optional seconds field, ranges/steps/lists,
month/weekday names, `@`-shortcuts), strictly after an explicit base time, in UTC.
Surfaces: chat/LLM API + CLI (`gizza tool cron-next-runs`) + page
(`/tools/cron-next-runs/`).

## Top competitors surveyed

1. **crontab.guru** — the canonical cron editor. As you type it shows (a) a
   plain-English description ("At 12:00 AM, every day") and (b) the **next run
   times**. 100% free, no signup, parses in the browser. The reference UX.
2. **Crontab Guru clones / wrappers** (toolspark, getnextool, hafiz.dev,
   fastdevkit, devtoolbox, cronguru.com.in) — same core: human-readable
   translation + next N execution times (some show next 10), preset quick-loads
   (every 5 min / hourly / daily / weekly / monthly), and a **visual dropdown
   builder**.
3. **cronexpressiongenerator / freeformatter cron** — validate + next-run preview,
   often with a builder UI and seconds support (Quartz 6/7-field).
4. **AWS EventBridge / Kubernetes CronJob doc helpers** — same next-run preview
   framed for a specific scheduler; note that some use 1-7=Mon-Sun (Quartz) vs
   the Vixie 0-7=Sun convention.
5. **cron-parser / croniter (libraries)** — programmatic "next N fire times" with
   ranges/steps/lists/names, leap-day handling, and the Vixie DOM∨DOW union rule.

(No competitor copy, branding, or trademarks were reproduced — feature shapes only.)

## Gap diff and ranking (fit-to-model)

| Competitor capability | gizza status | Action |
|---|---|---|
| Next N run times | Had it (count 1-100, default 5) | kept |
| Browser-local / private | Had it (wasm, no server) | kept |
| 5-field crontab, ranges, steps, lists | Had it | kept |
| Month/weekday three-letter names | Had it (JAN-DEC, SUN-SAT) | kept |
| `@`-shortcuts (@daily/@hourly/…) | Had it | kept |
| Seconds field (6-field) | Had it (`*/30 * * * * *`) | kept |
| Vixie DOM∨DOW union rule | Had it | kept |
| Leap-day / never-fires handling | Had it (8-yr bound, clear error) | kept |
| **Plain-English schedule description** | **Missing** | **ADDED** (see below) |
| Preset quick-loads | partial | covered by page examples + copy |
| Visual dropdown builder | Missing | **out of model** — page is single-field; not built |
| Pick start time / timezone | partial | `after` base time added; TZ below |

### Closed in this pass

- **Plain-English description** (the single highest-value gap — every crontab.guru
  variant leads with it). Added `Schedule::describe()` in core, surfaced on both
  outputs: text gets a `Schedule:    At 09:00, Monday through Friday (UTC)` line,
  JSON gets a `"description"` field. Covers `*/N` step phrasing ("Every 15
  minutes"), exact times ("At 09:00"), contiguous weekday/month runs ("Monday
  through Friday", "June through August"), `@hourly`, and DOM∨DOW unions. Unit
  tests (`descriptions`, `json_includes_description`) lock the phrasing; the page
  and JSON assertions cover it end-to-end.
- **Explicit base time** ("compute from when") — the `after` param accepts an
  ISO-8601/RFC-3339 UTC timestamp, a bare date, or a Unix epoch; blank = now
  (real clock in chat/CLI via `SystemTime`, `Date.now()` on the page). This makes
  the page deterministic and lets users preview from any point in time.
- Examples/presets surfaced in the page copy (`*/15 * * * *`, `0 9 * * MON-FRI`,
  `0 0 1 * *`, `0 0 13 * FRI`, `0 0 29 2 *`).

### Deliberately NOT built (out of gizza's model or low value)

- **Visual dropdown builder** — the page driver renders one declarative field set;
  a click-to-build cron UI is a bespoke front-end out of the shared model. The
  free-text field + live recompute + examples cover the same need.
- **Local-timezone display** — the tool is fixed to **UTC** (matching how cron
  daemons, CI and most schedulers express schedules, and keeping the core
  deterministic). A full IANA tz database/DST engine is a large dependency with
  no wasm-clean fit here; UTC is the correct, unambiguous default and is stated
  explicitly in the copy and every output. Documented, not built.
- **Quartz 1-7=Mon-Sun weekday convention** — we follow the standard Vixie
  0/7=Sunday convention (documented). Supporting a second, conflicting numbering
  would be ambiguous; left as a documented choice.
- **"Previous run" / backwards iteration** — not a crontab.guru feature and not
  requested by the backlog row; the next-runs forward walk is the scope.

## Verification (all surfaces, this pass)

- `cargo test --workspace` in `blocks/cron-next-runs`: **drift guard (1) + core
  (22) pass** — covers parsing, ranges/steps/lists, names, seconds, leap day,
  never-fires error, the Vixie union, timestamp parsing roundtrip, and the new
  description phrasing.
- `wafer build` (in `blocks/cron-next-runs/`): block.wasm **validates /
  instantiates** (clock import OK), 340 KiB.
- `wasm-pack build .../web --target web --release` → page module built.
- `gizza tool cron-next-runs …`: text + JSON + `@`-shortcut + default-now clock +
  out-of-range error all correct.
- Playwright `tool-page-cron-next-runs.spec.ts`: **3/3 pass** — text list +
  Schedule description, JSON output with `description`, and an invalid-expression
  error.

No LLM-facing input-schema drift (only output gained a field + copy updated in
descriptor, manifest, and authored drift JSON in lockstep).
