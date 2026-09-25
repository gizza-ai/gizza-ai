# meeting-time-finder — competitor analysis (2026-09-25)

Scan run **before** implementation, per `/create-next-tool` step 4. Competitors were read for
*ideas, table-stakes parameters and UX patterns only* — every observation below is paraphrased;
no competitor copy, branding or trademark is reproduced or reused anywhere in the tool.

Backlog row: `meeting-time-finder` — "Finds meeting times that fall within working hours for
everyone given each participant's timezone." (type hint: pure)

## Duplicate / viability check

`ls blocks/ | grep -i -E 'meet|time|zone'` surfaces `timezone-convert` as the only neighbour in the
same family. Reading `blocks/timezone-convert/core/src/lib.rs` confirms it is a **different tool**:
it converts ONE known wall-clock instant from a source zone into target zones and renders a static
24-hour "Business/Leisure/Rest" hour grid. It has no concept of per-participant working hours, no
meeting duration, no slot enumeration, no scoring and no ranking — i.e. it answers "what time is
14:30 New York in Tokyo?", not "which slots on this date are inside everyone's working day?".
`docs/tool-skiplist.txt` contains no entry pointing at this slug (only `timezone-converter`, which
points at `timezone-convert`). Conclusion: **not a duplicate, viable as a pure block.**

## Competitors reviewed

1. **Clock7 — Meeting Time Finder** (`clock7.com/meeting-finder/`)
2. **YayRemote — Time Zone Overlap Finder** (`yayremote.com/free-tools/time-zone-overlap`)
3. **Best Meeting Planner** (`bestmeetingplanner.com`)
4. **Timezone Overlap Planner** (`timeoverlap.fyi`)

(Surfaced alongside them: InventiveHQ's world-clock planner, timezonetable.com, Koalendar's
planner — same feature set, nothing additional to learn, so the four above were read in depth.)

### Observed feature matrix (paraphrased)

| Capability | Clock7 | YayRemote | BestMeetingPlanner | timeoverlap.fyi |
|---|---|---|---|---|
| Participant cap | 5 cities | 2–6 people | unstated | unstated |
| Per-participant working hours | global start/end from a fixed list (7–10 / 17–20) | per person, default 9:00–17:00 | per city | per zone |
| Participant names/labels | no | yes | yes ("name · city") | yes |
| Meeting date | yes | yes | yes | yes |
| Duration | 30 / 60 / 90 / 120 min presets | 15–120 min | 30 / 60 / 120 min | minutes |
| Slot granularity | unstated | unstated | 30/60/90 min interval | hourly |
| Ranking | "least antisocial first" | proximity of each local time to noon | fairness-first: maximise the WORST individual score | 0–100 "pain"/comfort score, top 5 |
| Per-participant score shown | no | implied | yes, percent per person | yes, per slot |
| 24-hour visual overlap | colour-coded timeline | timeline | green/amber/red bands | heat map |
| No-overlap handling | explicit "no common business hours" message | message + suggests async | — | — |
| DST warning | none | relies on Intl | warns when a clock change lands on the date | mentions DST warnings |
| Calendar export | none | .ics download | .ics + Google/Outlook links | export/share |
| Shareable link | copy-link with selections | shareable URL | link restores all settings | share |
| Presets | region chips (US & Europe, US & Asia, …) | — | saved named city sets | quick-add city buttons |
| Weekend handling | not stated | not stated | not stated | not stated |

## Decisions — every table stake lands somewhere

**Built in (in-model, in the descriptor from the start):**

| Table stake | How it ships here |
|---|---|
| Multiple participants with IANA zones | `participants`, comma-separated, cap 12 (higher than every competitor's 5–6) |
| Names per participant | `Name@Zone` syntax, e.g. `Alice@Europe/London` |
| Per-participant working hours | `Zone:8-16` suffix, e.g. `Bob@Asia/Tokyo:10-18`; overnight windows supported |
| Global default working hours | `work_start` / `work_end` (default 09:00–17:00, the shared competitor default) |
| Meeting date | `date` (required; the page pre-fills today) |
| Meeting duration | `duration_minutes` (5–480; covers all four competitors' preset ranges) |
| Slot granularity | `granularity_minutes` enum 15/30/60 |
| Ranking | fairness-first (maximise the worst participant's score), then average, then earliest — the BestMeetingPlanner idea, combined with YayRemote's midday-proximity comfort term |
| Per-participant score + status | every slot lists each person's local time, `in hours` / `partial` / `outside` / `weekend`, and a 0–100 score |
| 24-hour visual overlap | `output_format = timeline` — per-participant hour rows plus an "everyone" row |
| No-overlap handling | `allow_partial` (default on) ranks best-effort slots and names who is outside hours; off returns an explicit "no slot works for everyone" answer |
| DST warning | offsets are compared across the search window; any transition on the date is reported in a `DST` note |
| Calendar export | `output_format = ics` emits a valid VEVENT for the top-ranked slot (downloadable from the page) |
| Shareable link | already platform-level: every field is a `?param=` deep-link |
| Region presets | `[[example]]` chips (US & Europe, US & Asia, Global team, follow-the-sun) |
| Searchable zone picker | `kind = "tag-list"` + `options = "timezones"` vocabulary |
| 12/24-hour clock | `clock` enum (`24h` default, `12h`) |

**Beyond the competitors (kept because it is cheap and honest here):** weekend awareness
(`skip_weekends`, evaluated per participant's LOCAL day — none of the four state what they do on a
Saturday), `table`/`json`/`csv` machine-readable outputs, and a display-zone override
(`display_zone`) so the ranked column can be read in any zone.

**Considered, not built (out-of-model — no accounts, no server, no persistence):**

- Google/Outlook "add to calendar" web links — they are outbound URLs to third-party services;
  the `.ics` download is the account-free equivalent and is built.
- Saved/named participant presets with drag-to-reorder — needs per-user persistence; the deep-link
  URL is the stateless substitute.
- Recurring-meeting rotation/fairness tracking across weeks (timeoverlap.fyi) — needs stored
  history of past meetings; a single-date tool cannot do it honestly.
- City-name geocoding ("London" → `Europe/London`): the zone vocabulary autocomplete covers zone
  selection; a city→zone gazetteer is a separate dataset and a separate tool.
- Live "current time" auto-refresh clocks — this block is deterministic and clock-free by design
  (the page supplies today's date as a smart default instead).

## Feasibility spike (before tagging anything out-of-model)

`chrono` + `chrono-tz` (already proven wasm-safe by `blocks/timezone-convert`) provide the full
IANA database including historical/future DST transitions, so per-zone offsets, DST-gap handling
and per-participant local weekdays are all pure-Rust and need no clock — that covers the entire
in-model list above. `.ics` output is plain text assembly with a deterministic `UID`/`DTSTAMP`
(derived from the meeting date, not a clock), so it stays reproducible in tests.
