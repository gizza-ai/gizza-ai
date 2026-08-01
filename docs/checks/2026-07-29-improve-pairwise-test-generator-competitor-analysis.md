# pairwise-test-generator — competitor analysis (2026-07-29)

Scan done **before** implementation. All descriptions paraphrased from public docs; no
competitor copy, branding, or trademarks reproduced.

## Top 3 real competitors

### 1. Microsoft PICT (Pairwise Independent Combinatorial Testing)
Open-source command-line generator. Input is a plain-text **model file**: one parameter per
line as `Name: value1, value2, value3`. Generates a pairwise (2-way) test set by default and
can go to higher interaction strength via an order flag. Rich model language: constraints
(`IF [a] = "x" THEN [b] <> "y"`), value weights, aliases (`|`), negative-testing markers
(`~`), and sub-models. Output is a tab-separated grid (header row of parameter names, one row
per generated case) written to stdout. Deterministic for a given seed.

- **Input format:** one param per line, `Name: v1, v2, …` — matches our chosen format.
- **Defaults:** order = 2 (pairwise); tab-separated output.
- **UX:** CLI flags (`/o:N` order, `/d:` value separator, `/e:` seed file).

### 2. allpairspy (thombashi, Python library)
Programmatic pairwise combination generator. You pass a list of value-lists and iterate
`AllPairs(...)`, which yields the reduced set of tuples covering every value pair. Supports a
user **filter function** to drop invalid combinations (constraints) and a `previously_tested`
seed. No fixed UI — output is Python tuples the caller formats. Emphasises deterministic,
minimal-ish suites.

- **Input format:** list of value lists (programmatic).
- **Defaults:** pairwise; greedy reduction.
- **UX:** library API; constraints via a callback.

### 3. BesTest — free online pairwise / all-pairs generator
Browser tool. Input is text, **one parameter per line, values comma-separated** (`Name: v1,
v2`). Supports up to ~10 parameters with up to ~30 values each. UI has an **Add parameter**
button and a **Load sample** button; validates "add at least two parameters with values".
Output exportable as **CSV** (spreadsheets/test tools), **Markdown** (wikis), or a printed
**table**. Uses a **greedy all-pairs** algorithm — seeds each test from an uncovered pair, then
fills remaining parameters with the values covering the most still-missing pairs; deterministic.
Notes it is not guaranteed-minimal but lands within a test or two of best-known results.

- **Input format:** one param per line, `Name: v1, v2, …`.
- **Defaults:** pairwise; table/CSV/Markdown output.
- **UX:** add-parameter + load-sample buttons; format toggle.

Other references skimmed (methodology, not tools): TestRail and Testomat blog explainers,
pairwise.org (points at PICT/ACTS/Hexawise), Hexawise (commercial, constraints + n-way).

## Table-stakes → our descriptor

| Capability | Competitors | Our decision |
|---|---|---|
| One-param-per-line `Name: v1, v2` text input | PICT, BesTest | **in-model** — `parameters` (required, multiline) |
| Deterministic greedy all-pairs (2-way) coverage | all three | **in-model** — core algorithm |
| CSV output (spreadsheet / test-mgmt import) | BesTest | **in-model** — `output_format=csv` |
| Markdown table output (wikis / PRs) | BesTest | **in-model** — `output_format=markdown` (default) |
| Printed / plain table output | BesTest | **in-model** — `output_format=ascii` |
| JSON output (programmatic) | allpairspy-style callers | **in-model** — `output_format=json` |
| Numbered case column | BesTest table | **in-model** — `include_index` (default true) |
| Load-sample / presets | BesTest | **in-model** — `[[example]]` preset chips |
| Validate ≥2 params, non-empty values, dup names | PICT, BesTest | **in-model** — validation errors |
| Reduction stat ("N tests instead of M") | BesTest | **in-model** — reported in error/summary via FAQ + count; kept out of the parseable table body |
| Higher interaction strength (3-way / n-way) | PICT `/o:N`, Hexawise | **out-of-model** — v1 is pairwise-only (the tool's namesake); documented as a limit |
| Constraints (IF/THEN, exclude invalid pairs) | PICT, allpairspy, Hexawise | **out-of-model** — needs a constraint mini-language; listed as a limit |
| Value weights / negative markers / aliases | PICT | **out-of-model** — advanced PICT-only syntax |
| Seed / previously-tested cases | PICT, allpairspy | **out-of-model** — no persistent state on a stateless page |

Every table-stake lands in the descriptor or is explicitly listed out-of-model above — none
dropped silently. Out-of-model items are documented on the page (limits + FAQ), not built.
