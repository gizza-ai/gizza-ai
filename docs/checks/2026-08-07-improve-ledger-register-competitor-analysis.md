# ledger-register — competitor analysis (2026-08-07)

Scan run **before** implementing, per `.claude/skills/create-next-tool`. All findings are
paraphrased from public reference documentation; no competitor copy, branding or trademarks
are reproduced here or in the shipped page.

## Who was scanned

| # | Competitor | What was read |
|---|---|---|
| 1 | hledger (`register` command) | The `register` section of the official command manual — column layout, running-total modes, filtering, output formats |
| 2 | ledger-cli (`register` report) | The `ledger(1)` manual page — report options affecting the register (`--sort`, `--related`, `--invert`, `--head`/`--tail`, `--average`, `--real`, `--depth`, `--collapse`, `--wide`, `--period`, status flags) |
| 3 | ledger-cli 3 user manual, "The register command" | Prose description of the report shape: one line per posting, running balance in the last column, the final running balance equalling the account's balance |

(The Fava web UI was checked as a fourth data point but its landing page carries no register
documentation, so it is not counted as one of the three and nothing from it was used.)

## Table-stakes checklist

### In-model — built into this tool

| Capability | How competitors express it | Our param |
|---|---|---|
| One line per matching posting, with date, description, account, amount | the default register report shape | (always) |
| Running total column | the last column of the report | `running_total = "period"` (default) |
| Historical running total — start from the balance *before* the report period | `-H` / `--historical` | `running_total = "historical"` |
| Running **average** instead of a running total | `-A` / `--average` | `running_total = "average"` |
| No total column at all | not offered as such (implied by `--format`) | `running_total = "none"` |
| Account filter | positional query patterns | `account_filter` (comma-separated substrings, `not:`/`-` excludes) |
| Payee / description search | `-m` / `--match` | `payee_filter` |
| Date range, end exclusive | `-b` / `-e`, `-p` | `begin`, `end` |
| Status filter | `-C` cleared, `-P` pending, `-U` unmarked | `status` enum |
| Show the *other* side of the transaction | `-r` / `--related` | `related` |
| Flip the sign of every amount | `--invert` | `invert` |
| Ignore virtual `(…)` / `[…]` postings | `-R` / `--real` | `real_only` |
| Fold accounts to N levels | `--depth` / `--collapse` | `depth` (0–10) |
| Report `@` / `@@` priced postings at cost | `-B` / `--basis` | `cost_basis` |
| Sort order | `--sort EXPR` | `sort` enum (date, date-desc, amount, amount-asc, account) |
| First N / last N rows | `--head N` / `--tail N` | `limit` + `limit_from` |
| Output width control | `-w N` / `--width`, `--wide` | `width` (40–400 columns) |
| Machine-readable output | `-O csv` / `json` / `tsv` | `output_format` (text, csv, json, markdown) |

Every table-stake above ends up in the descriptor, so the chat, CLI and page surfaces all
expose it.

### UX controls competitors ship, mapped to page controls

- Competitors are CLIs with **preset one-letter flags** for the common reports, so the page
  gets `[[example]]` **preset chips** for the equivalents: a plain checkbook register, a
  historical (opening-balance) register, a running average, the related-account view, and a
  CSV export.
- `--depth` and `-w N` are numeric knobs → rendered as **sliders** (`kind = "slider"`).
- `-b` / `-e` are dates → rendered as **date pickers** (`kind = "date"`).
- Fixed-choice flags (status, running-total mode, sort, output format) → `Param::enumv`
  `<select>`s with friendly `[input.labels]`.
- The journal itself is a multi-line paste → `multiline = true` textarea.

### Out-of-model — listed, deliberately not built

| Capability | Why it is out of model here |
|---|---|
| Period-subtotal registers (`-D` / `-W` / `-M` / `-Q` / `-Y`, `--period EXPR`) | Turns the register into a periodic summary report, a different report shape; the sibling balance tool covers periodic aggregation better |
| `--by-payee`, `--subtotal` grouping | Same reason — grouping collapses the per-posting register this tool is about |
| Market-value / currency conversion (`-V`, `-X`, `--exchange`, `--market`) | Needs live or historical price lookup; this tool runs offline with no network |
| `--format` / `--date-format` custom format strings | A whole embedded format-expression language; the four output formats cover the practical need |
| `--display EXPR`, `--limit EXPR` value expressions | Same — an expression evaluator is its own tool |
| `--deviation` (each posting's distance from the average) | Niche; the average mode covers the useful part |
| `include FILE` resolution | There is no filesystem in the browser or the sandbox; skipped and reported in the notes |
| Checking that balance assertions actually hold | Assertions are parsed so they can never be mistaken for an amount, but verifying them is a separate lint-style tool |
| TSV output | Trivially derivable from the CSV output; not worth a fifth enum value |

## Copy / positioning notes

- Both reference tools describe the register as the "checkbook" view — one line per posting
  with a running balance whose last value equals the account balance. The page copy explains
  that relationship in our own words and cross-references the sibling balance tool.
- The end date being **exclusive** is the single most common surprise in both CLIs, so it is
  called out in the field label, the param description and an FAQ.
- Neither competitor runs in a browser; local-only, no-upload execution is our differentiator
  and is stated on the page.
