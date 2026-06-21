# change-case — competitor analysis (2026-06-22)

Tool: `blocks/change-case` — converts text between letter cases. Pure Rust→WASM
(no network/filesystem). Surfaces: chat skill, CLI (`gizza tool change-case`),
standalone page (`/tools/change-case/`).

## Competitors surveyed

| # | Tool | Notable scope |
| - | ---- | ------------- |
| 1 | convertcase.net | Upper, lower, Title, Sentence, Capitalize, aLtErNaTiNg, inverse/swap; download/copy. |
| 2 | caseconverter.cc | Upper, lower, Title, Sentence, plus word/char counts. |
| 3 | appdevtools.com/case-converter | Dev-focused: camel, snake, kebab, Pascal, CONSTANT, dot, path, header/param. |
| 4 | coderstool.com/convert-case | lower, UPPER, Sentence, Capitalize, alternating, inverse. |
| 5 | titlecase.com | Style-guide Title Case (AP/Chicago grammar rules). |

## Capability matrix vs gizza change-case

| Capability | Competitors | gizza change-case |
| ---------- | ----------- | ----------------- |
| UPPERCASE / lowercase | all | yes |
| Title Case (each word) | most | yes |
| Sentence case | most | yes (splits on `. ! ?`) |
| Capitalize first letter only | convertcase, coderstool | yes (`capitalize`) |
| Swap / inverse case | convertcase, coderstool | yes (`swap`) |
| aLtErNaTiNg case | convertcase, coderstool | **added** (`alternate`) |
| camelCase / PascalCase | appdevtools | yes |
| snake_case / CONSTANT_CASE | appdevtools | yes |
| kebab-case / Train-Case | appdevtools | yes |
| dot.case / path/case | appdevtools | yes |
| Re-tokenize existing camelCase / acronyms | partial | yes (`HTTPServer`→`http_server`) |
| Full Unicode case mapping | varies | yes (`straße`→`STRASSE`, `café`→`CAFÉ`) |
| Runs locally / no sign-up | most | yes (in-browser WASM) |

## Gaps closed this run

- **aLtErNaTiNg case** — offered by convertcase.net and coderstool but absent
  from the first cut. Added as `mode=alternate` (aliases: `alternating`,
  `spongebob`); alternates lower/upper across letters only, leaving punctuation
  and spacing untouched (`hi there` → `hI tHeRe`). Wired through core (+unit
  test), descriptor enum, drift-guard schema, manifest, page select, and SEO copy.

## Out-of-model / deliberately not built

- **Grammar-aware Title Case** (AP/Chicago: lowercase minor words like "a", "of",
  "the") — titlecase.com's differentiator. This needs an editorial style-rule
  engine / word list rather than a pure case mapping; out of scope for a
  deterministic case converter. Our `title` is the standard "capitalize every
  word" behavior, which matches the majority of competitors.
- **Word / character counts** — already covered by separate gizza tools
  (`word-count`, `text-statistics`); not duplicated here.
- **Copy/download buttons** — page-chrome concern handled by the shared tool
  page template, not the tool itself.

## Verification (this run)

- `cargo test --workspace` in `blocks/change-case` — 11 core + 1 drift-guard pass.
- `wafer build` — chat block validates OK (320 KiB).
- CLI: `gizza tool change-case` exercised across upper/lower/title/sentence/snake/
  camel/constant/alternate + default-mode + invalid-mode (exit 1) paths.
- Page: Playwright `tool-page-change-case.spec.ts` — 2 specs pass (letter cases +
  identifier cases via the `mode` select).

NEVER copied competitor copy, branding, or trademarks; behavior was derived
independently from the case definitions.
