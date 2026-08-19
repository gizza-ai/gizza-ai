# ledger-balance — competitor analysis (2026-08-07)

Scope: a tool that takes a **ledger-cli / hledger plain-text journal** as text and reports the
**balance of every account and sub-account**. Research done BEFORE implementing so the descriptor
ships the table-stakes options from day one.

All notes below are paraphrased from public documentation. No competitor copy, branding, or
trademarked wording is reproduced or reused in this repo.

## Competitors skimmed

1. **hledger `balance` / `bal`** — <https://hledger.org/hledger.html>, cheatsheet at
   <https://devhints.io/hledger>. The reference implementation for this report shape.
2. **ledger-cli `balance` / `bal`** — <https://ledger-cli.org/doc/ledger.1.html> (403 to the
   fetcher; read via the Debian mirror <https://manpages.debian.org/testing/ledger/ledger.1.en.html>).
   The original plain-text-accounting balance report.
3. **beancount `bean-report balances` (aka `bal` / `trial`) + fava** —
   <https://manpages.ubuntu.com/manpages/focal/man1/bean-report.1.html>,
   <https://beancount.github.io/docs/running_beancount_and_generating_reports/>. Same job in a
   different journal dialect; fava renders the same tree in a browser.

hledger and ledger converge on nearly the same option set, so the table below merges them and
notes where beancount differs.

## Table-stakes checklist

| Capability | Competitor flag(s) | In model? | Where it landed |
|---|---|---|---|
| Balance for every account **and its parents** (roll-up) | default behaviour of all three | in | core rolls every posting up the `:` hierarchy |
| Indented **tree** layout, right-aligned amounts, dashed total row | default of hledger/ledger | in | `layout = tree`, default `text` output |
| **Flat** layout (full account names, leaf subtotals only) | `--flat` | in | `layout = flat` |
| **Depth** limit — fold deeper accounts into the parent | `--depth N` | in | `depth` (0 = unlimited) |
| Show **zero-balance** accounts | `-E` / `--empty` | in | `include_empty` |
| **Account query / filter** | positional query args, `not:` prefix | in | `account_filter`, comma-separated, `not:` excludes |
| **Date range** | `-b/--begin`, `-e/--end` (end exclusive) | in | `begin`, `end` (`kind = "date"` pickers) |
| **Status** filter | `-C/--cleared`, `-P/--pending`, `-U/--unmarked` | in | `status` enum: all/cleared/pending/unmarked |
| Ignore **virtual** postings | `-R/--real` | in | `real_only` |
| Suppress the **total** row | `--no-total` | in | `show_total` (inverted, default on) |
| **Sort by amount** instead of account name | `-S/--sort-amount`, `--sort EXPR` | in | `sort` enum: account / amount / amount-asc |
| **Percent** of the report total | `--percent` | in | `percent` |
| **Machine-readable output** | `-O csv,tsv,json,html`; beancount `-f csv/html/xls` | in | `output_format`: text / csv / json / markdown |
| **Cost basis** (`@`/`@@` prices already in the journal) | `-B/--cost` | in | `cost_basis` |
| **Multi-commodity** accounts reported per commodity | all three | in | one line per commodity, name on the last line (ledger style) |
| Inferred blank posting amount | all three | in | core infers the single blank posting |
| Balance **assertions** `= AMT` parsed, not counted as amounts | all three | in (parse) | parsed and ignored — see out-of-model for *checking* them |
| Directives: `account`, `alias`, `commodity`, `D`, `Y`, `P`, `comment`/`end comment` | all three | in | parsed (`commodity`/`D` set display precision, `alias` rewrites, `Y` fills yearless dates) |

## UX controls competitors expose (and what we ship)

- hledger's web UI and fava both drive the report from **toggles**, not free text: tree/flat, a
  depth stepper, an "include zero accounts" switch, and a date range. → we render `layout`,
  `sort`, `status`, `output_format` as `<select>`s (`Param::enumv`), the four flags as
  checkboxes, `depth` as a **slider** (0–10), and `begin`/`end` as native **date pickers**
  (`kind = "date"`).
- Both CLIs ship canned invocations in their docs (assets-only, monthly, depth-2 summary). Those
  are the preset idiom here → four `[[example]]` **chips** on the page: a starter journal, a
  depth-2 summary, an expenses-only filter, and a multi-commodity/cost-basis journal.
- fava shows percentages next to the tree → `percent` checkbox.
- Nobody offers a copy-paste box that works with no install; that (plus JSON/CSV/Markdown export
  of the same numbers) is our differentiator, so `output_format` is a first-class param.

## Out of model — listed, deliberately NOT built

- **Market value / price conversion at a date** (`-V`, `-X COMM`, `--value=WHEN`). Needs a price
  database keyed by date plus a conversion graph; `P` directives are parsed but not applied.
  Cost basis (`-B`) *is* implemented because the price travels with the posting.
- **Multi-period columnar reports** (`-D/-W/-M/-Q/-Y`, `--period`). A different report shape
  (one column per period), not a balance list.
- **Budget reports** (`--budget`) and **periodic/automated transactions** (`~`, `=` rules).
  Requires a rule engine on top of the parser.
- **`include FILE`** — there is no filesystem in the browser or in the wasm sandbox. `include`
  lines are ignored and reported in the tool's notes rather than failing the parse.
- **Balance-assertion *checking*** (erroring when `= AMT` disagrees with the running balance).
  Assertions are parsed so they never corrupt an amount, but this tool reports balances rather
  than validating them; a dedicated checker is a better home for it.
- **Beancount-dialect journals** (`open`/`close`/`balance` directives, `YYYY-MM-DD Account`
  capitalization rules). Beancount postings are close enough that simple files parse, but the
  directive vocabulary is not supported — the page says so.
- **HTML / XLS / SQL export** (`-O html`, beancount `-f xls`). Text, CSV, JSON and Markdown
  cover the copy-paste and scripting cases; the rest is formatting noise for a paste tool.

## Limits we state on the page

- 5,000 transactions per run (a paste tool, not a repository-scale reporter).
- Account depth limit 0–10 for the depth control.
- Amounts are parsed as `1,234.56` or `1.234,56`; the ambiguous `1,23` is read as a decimal comma.
