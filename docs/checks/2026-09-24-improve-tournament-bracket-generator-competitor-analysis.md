# tournament-bracket-generator competitor analysis — 2026-09-24

## Sources checked

| Competitor | Observed table-stakes capabilities | Fit for this block |
| --- | --- | --- |
| SnapToolSuite tournament bracket generator | Paste teams/players; single elimination, double elimination and round-robin; automatic byes; standard seeding; court schedule; printable PDF and CSV export. | In model: pasted roster, single/double elimination, byes, standard seeding, CSV-style export. Out of model for this pure local block: round-robin schedule matrix, court/time scheduling, PDF rendering. |
| BracketMaker.app single/double elimination pages | Any number of teams/players; seeded, blind/random and manual pairing options; automatic byes; optional third-place playoff; online result advancement; PDF/CSV download/share. | In model: seeded/ordered/random placement, automatic byes, third-place match, CSV/text/markdown/JSON export. Out of model: live score advancement, hosted sharing, PDF generation. |
| Challonge bracket generator / platform pages | Single and double elimination tournament formats, broader tournament operations, hosted brackets and signup/share workflows. | In model: core bracket structure for single and double elimination. Out of model: hosted tournament management, accounts, predictions/voting, live advancement. |
| WUTools tournament bracket generator | Single elimination, double elimination and round robin; works locally; phone/tablet/desktop; print and SVG export. | In model: local single/double bracket generation, deterministic copyable output. Out of model: round robin and SVG diagram export. |

## Decisions for this implementation

- Inputs: a multiline participant roster is required; a bare count expands to numbered placeholder teams for quick bracket sizing.
- Bracket type: implemented fixed-choice `single` and `double` modes with enum controls.
- Seeding: implemented `standard`, `ordered`, and reproducible `random` modes to cover seeded, manual/as-entered, and blind-draw competitor patterns.
- Byes: implemented automatic next-power-of-two bracket sizing with explicit `BYE` slots.
- Consolation/reset: implemented a single-elimination third-place match and an optional double-elimination grand-final reset.
- Outputs: implemented text, markdown, CSV, and JSON. Text is printable/copyable; CSV/JSON cover spreadsheet and automation use cases.
- UX controls: roster textarea, enum selects for bracket type/seeding/output format, boolean checkboxes, draw seed field, and example preset chips for common scenarios.

## Explicit out-of-model items

- Hosted tournament pages, account workflows, share links, live scoring, and automatic winner advancement.
- Round-robin league schedules and court/time scheduling.
- PDF, PNG, SVG, or drag-and-drop visual bracket rendering. This repository’s generic tool page provides copyable text output; richer drawing/export belongs in a media/diagram-specific block.

## Verification focus from the scan

The tests and CLI checks should cover standard seeding, byes, the non-default double-elimination mode, the non-default third-place checkbox, random draw seed reproducibility, the 64-participant cap boundary, and at least one secondary roster input form (bare count or comma-separated names).
