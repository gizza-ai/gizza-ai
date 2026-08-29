# round-robin-scheduler — competitor analysis (2026-08-29)

Scan run before finishing `blocks/round-robin-scheduler`, to decide which table-stakes controls belong in the descriptor and which scheduling features are outside the current gizza model. Observations are paraphrased from public tool pages; no competitor copy, branding, or trademarks is reused in the page.

## Competitors reviewed

| # | Tool | What it is |
|---|------|------------|
| 1 | bracketmaker.app round-robin generator | Online tournament scheduler with printable/exportable round-robin schedules for arbitrary team counts |
| 2 | noveblo.com round-robin maker | Lightweight round-robin fixture generator with participant names and automatic byes |
| 3 | scheduler.leaguelobster.com round-robin generator | League scheduler with free limited team counts and paid advanced scheduling controls |
| 4 | toool.cc sports league scheduler | Simple online scheduler for 4–16 teams, explicitly advertising balanced home/away |
| 5 | Playpass tournament scheduler | Broader tournament scheduler with sharing/editing/account features beyond round robin |

## Table stakes observed

| Capability | 1 | 2 | 3 | 4 | 5 | Our decision |
|---|---|---|---|---|---|---|
| Enter participant names | ✅ | ✅ | ✅ | ✅ | ✅ | **In-model** — `participants`, one per line, comma-separated, pasted bullet/numbered lists, comments, or a bare count |
| Generate every pair exactly once | ✅ | ✅ | ✅ | ✅ | ✅ | **In-model** — `schedule_type=single`, circle method |
| Odd-team bye rotation | ✅ | ✅ | ✅ | ✅ | ✅ | **In-model** — phantom entrant produces one rotating `BYE`; `include_byes` toggles display |
| Double round robin / home-and-away leg | ✅ | — | ✅ | — | ✅ | **In-model** — `schedule_type=double`, second leg mirrors home/away |
| Balanced home/away orientation | ✅ | — | ✅ | ✅ | ✅ | **In-model** — deterministic orientation keeps the host spread within one; double round robin is exact |
| Courts / fields / venues | ✅ | — | ✅ | — | ✅ | **In-model** — `courts`, as a count or comma-separated names, assigned across each round |
| Start round / custom numbering | — | — | ✅ | — | ✅ | **In-model** — `start_round` starts numbering at any positive integer |
| Output/export formats | ✅ | basic copy | ✅ | basic copy | ✅ | **In-model** — `output_format=text/markdown/csv/json`; CSV/JSON are scriptable, Markdown covers docs/wiki use |
| Deterministic shuffled draw | — | — | ✅ | — | — | **In-model** — `seed` reproducibly shuffles the entered order without depending on runtime randomness |
| Preset/example schedules | ✅ | ✅ | ✅ | ✅ | ✅ | **In-model** — page includes preset chips for 6 teams, odd byes, court assignment and CSV export |
| Accounts, saved schedules, public sharing links | ✅ | — | ✅ | — | ✅ | **Out-of-model** — no backend/accounts in this repository; URL query prefill is the local shareable shape |
| Manual fixture editing / drag-and-drop | ✅ | — | ✅ | — | ✅ | **Out-of-model** — the tool is deterministic compute, not a stateful schedule editor |
| Team availability constraints, blackout dates, time slots | paid/advanced | — | ✅ | — | ✅ | **Out-of-model for this tool** — constrained optimisation is a different scheduler; this block intentionally generates the unconstrained canonical round robin |
| Standings, scores, referees, notifications | ✅ | — | ✅ | — | ✅ | **Out-of-model** — league-management features, not fixture generation |
| PDF/print export | ✅ | browser print | ✅ | browser print | ✅ | **Not built separately** — Markdown/CSV/text plus browser print cover local export without a PDF engine |

## In-model UX/control choices adopted

- Participant input is multiline, with a placeholder showing the normal one-name-per-line roster.
- `schedule_type` and `output_format` are enums so the page renders selects rather than free text.
- Boolean checkboxes expose odd-roster bye display and summary display.
- `courts` accepts either a count (`2`) or names (`North Field, South Field`), covering both simple and real-venue pages without adding another control.
- `seed` keeps the default deterministic but gives users a reproducible way to change the draw.
- Example chips exercise the main competitor patterns: even teams, odd byes, court assignment, and spreadsheet export.

## Gaps deliberately closed

- The generated text names totals up front: participant count, rounds, matches, matches per participant, and byes per participant.
- CSV and JSON include a stable `round`/`match`/`home`/`away` shape suitable for automation.
- The parser accepts pasted numbered and bulleted lists, plus `#` comment lines, so a copied roster does not need manual cleanup.
- A bare count such as `8` expands to placeholder teams, matching count-first schedulers while still supporting named rosters.

## Out-of-model (listed, not built)

| Feature | Why it does not fit |
|---|---|
| Persistent schedules/accounts/public edit links | Requires a backend and identity layer; this repo ships local tools only |
| Drag-and-drop fixture editing | Stateful UI/editor workflow rather than deterministic pure function output |
| Time/date calendar generation | Requires availability/time-zone semantics and a richer event model; CSV export is the handoff point |
| Blackout constraints / venue capacity optimisation | Constraint solving is a separate scheduler category from canonical round-robin pairing |
| Score entry, standings and tie-breakers | League management after fixtures are played, not fixture generation |
| PDF-specific export | Browser print and text/Markdown/CSV/JSON outputs cover local export without a PDF renderer |
