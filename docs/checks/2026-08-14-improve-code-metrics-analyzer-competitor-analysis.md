# code-metrics-analyzer — competitor analysis (2026-08-14)

Scan run BEFORE implementation, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All findings are paraphrased from public documentation/UI descriptions. No competitor copy,
branding, or trademarks are reused anywhere in the tool.

## Competitors reviewed

| # | Tool | Shape | Reachable |
|---|------|-------|-----------|
| 1 | DevBolt "Code Complexity Analyzer" (devbolt.dev) | paste-a-snippet web tool, JS/TS only, client-side | yes |
| 2 | lizard (terryyin/lizard) | CLI, 29+ languages, per-function metrics | yes |
| 3 | scc (boyter/scc) | CLI, 130+ languages, file-level LOC + approximate complexity + COCOMO | yes |
| 4 | Browser line counters (webutils.io code-line-counter, linecounter.org, treasure.tools) | paste-a-file web tools, code/comment/blank split + language detection | yes |
| — | ClineTools "Code Complexity Analyzer" | paste web tool advertising cyclomatic/cognitive/Halstead/maintainability | **403 to the fetcher** — replaced by #4 above; its advertised metric list was still captured from search results and is covered by the table-stakes list |

## Table stakes observed

| Capability | Seen in | Fit | Where it lands |
|---|---|---|---|
| Paste source, analyze fully client-side / offline | 1, 4 | in-model | core is pure Rust, runs in the page wasm + CLI + chat block |
| Physical lines / code / comment / blank split | 3, 4 | in-model | `total_lines`, `code`, `comment`, `blank` (+ mixed code-and-comment lines counted as code, cloc convention) |
| Language auto-detection | 4 | in-model | `language = "auto"` scores keyword/syntax evidence across the 17 supported languages |
| Explicit language override | 1 (implicit), 2 (`-l`), 4 | in-model | `language` enum |
| Multi-language support (not just JS/TS) | 2, 3 | in-model | 17 languages: c, cpp, csharp, go, java, javascript, typescript, kotlin, lua, php, python, ruby, rust, shell, sql, swift, scala |
| Per-function table: name, line, length, complexity | 1, 2 | in-model | function table with line span, NLOC, CCN, cognitive, params, nesting |
| Cyclomatic complexity (CCN) per function + file total/avg/max | 1, 2, 3 | in-model | decision-point counting on comment/string-stripped code |
| Cognitive complexity | 1 | in-model (approximation) | nesting-weighted increments; documented as an approximation of the SonarSource definition |
| Nesting depth | 1 | in-model | max brace/indent depth inside each function |
| Parameter count per function | 2 (`-a`) | in-model | depth-0 comma split of the signature |
| Maintainability index + letter grade | 1 | in-model | classic `171 - 5.2 ln(V) - 0.23 G - 16.2 ln(LOC)`, rescaled 0–100, A–F band |
| Halstead volume | ClineTools (advertised) | in-model | token-based n1/n2/N1/N2 → volume; feeds the maintainability index and is reported in JSON |
| Complexity warning threshold | 2 (`-C`, default 15) | in-model | `complexity_threshold`, default 10 (the widely used "hard to test above 10" line); over-threshold functions are listed and counted |
| Sort results by a field | 2 (`-s`), 3 (`--sort`) | in-model | `sort` = line, complexity, cognitive, length, name |
| Machine-readable output (json / csv) | 2 (`--csv`, `-X`), 3 (json/csv/…) | in-model | `output` = summary, functions, json, csv |
| Risk banding / grades in the readable report | 1 | in-model | per-function risk (low/moderate/high/very-high) and an A–F maintainability grade |
| Truncation control for huge inputs | 2 (implicit) | in-model | `max_functions` (0 = all); the totals always reflect every function even when the list is truncated |
| Sample/preset snippets to try | 1 ("Clean Code", "Complex Handler", …) | in-model | three `[[example]]` preset chips on the page (our own snippets, written from scratch) |

## Out of model (listed, not built)

- **Multi-file / repository scanning, per-file tables, directory rollups** (scc, lizard, webutils.io):
  the block takes one pasted source text; there is no filesystem or multi-file upload surface.
- **COCOMO / LOCOMO cost + schedule estimation** (scc): defined over a whole codebase in KLOC;
  meaningless for a single pasted snippet, so it is deliberately not reported.
- **DRYness / unique-lines-of-code (ULOC)** (scc): a cross-file metric.
- **HTML / XML / checkstyle / SQL / openmetrics report exports** (lizard, scc): CI-integration formats
  that assume a file tree and a build server; JSON and CSV cover the machine-readable need here.
- **Full-AST accuracy** (lizard builds real per-language tokenizers; DevBolt parses JS/TS to an AST):
  our analyzer is a lexer + heuristic matcher, so exotic declaration forms (macro-generated
  functions, deeply nested closures assigned to expressions, C++ templates split across lines) can be
  missed or merged. Stated as a limit on the page rather than silently approximated.
- **Git history / churn / hotspot metrics** (scc plugins, commercial dashboards): no VCS access.
- **Sortable interactive HTML report with search** (lizard `-H`): the page renders a static text/JSON
  result; the generator has no per-tool table widget.

## Design decisions taken from the scan

1. Ship **per-function** metrics (lizard/DevBolt) *and* the **file-level LOC split** (scc/line
   counters) in one summary — no reviewed tool does both in a single paste-and-go surface.
2. Default `complexity_threshold` to **10**, not lizard's 15: the paste-a-snippet audience is looking
   at one function at a time, where 10 is the conventional "needs a second look" line. Configurable.
3. Keep **JSON and CSV** as first-class outputs so the CLI is scriptable, matching lizard/scc.
4. Report **language + how it was determined** (specified vs auto-detected) so an auto-detect miss is
   visible rather than silent.
5. Every advertised metric is computed from the same stripped-code pass, so the numbers in `summary`,
   `functions`, `json` and `csv` are identical by construction.
