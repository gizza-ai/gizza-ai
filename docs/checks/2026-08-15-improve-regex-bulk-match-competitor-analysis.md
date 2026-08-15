# regex-bulk-match — competitor analysis (2026-08-15)

Scan run BEFORE implementation, per `create-next-tool` step 4. All findings are paraphrased
observations of publicly documented behaviour. No competitor copy, branding, or trademarks are
reproduced or reused anywhere in this tool.

## Tools reviewed

| # | Tool | What it does that is relevant |
|---|------|-------------------------------|
| 1 | Online String Tools — "Test a String with Regex" (`onlinestringtools.com/test-string-with-regex`) | The closest match: a "test each line with the regex separately" toggle that emits a parallel true/false status line per input line. Accepts `/pattern/flags` form with `g i m s u y`. Copy + download buttons. |
| 2 | 10xTools Regex Tester (`10xtools.io/regex-tester`) | Real-time matching, all six JS flags, capture-group display with labels, exact character position of each match, and a "common patterns library" preset list (email, URL, phone, IPv4, `YYYY-MM-DD` date, hex colour). |
| 3 | CSV Tools Online Regex Tester (`csvtoolsonline.com/tools/regex-tester`) | Bulk pattern testing across thousands of records, column selection, per-match position + groups + extracted data, and CSV export / "download matches". States a 55 MB input ceiling. |
| 4 | ExtendsClass / CyrilEx Regex Tester (`extendsclass.com/regex-tester.html`) | Multi-flavour engine picker (JS, Python, Ruby, Java, PHP/PCRE, MySQL), flag sets per flavour, substitution field, regex visualiser, share links. Explicitly single-string — no bulk mode. |

Two further candidates (`freetoolscorner.com` line-by-line mode, `codeshack.io/regex-tester`) both
returned HTTP 403 to the fetcher, so they were replaced by #3 and #4 rather than running the scan
short.

## Table stakes → decisions

Every table stake below lands in the descriptor or in the out-of-model list. Nothing was dropped
silently.

### In model — built into this tool

| Capability | Seen on | How it ships here |
|---|---|---|
| Per-line match / no-match status | 1, 3 | Core behaviour: one report row per input line. |
| Case-insensitive matching | 1, 2, 3, 4 | `ignore_case` boolean. |
| Dot-matches-newline / other modes | 1, 2, 4 | `dotall` boolean; every other Rust-regex mode is reachable via inline flags `(?i)`, `(?m)`, `(?s)`, `(?x)` (documented on the page). |
| Whole-string vs. anywhere matching | 3 (validation framing) | `full_match` boolean (off by default = "test", on = "validate"). |
| Capture groups per line, incl. **named** groups | 2, 3 | `captures` boolean; named groups are reported by name, unnamed by index, in text/JSON/CSV. |
| Match character position | 2, 3 | `show_position` boolean for the text/CSV report; JSON always carries `start`/`end`. |
| Filter to only matching / only non-matching rows | 1 (implicitly), 3 | `show` enum: `all` / `matching` / `non-matching`. |
| CSV export of matches + groups | 3 | `output = csv` — one row per reported line, one column per capture group. |
| Structured output | 3 | `output = json`. |
| Summary counts | 1, 3 | Every format reports lines tested / matched / not matched. |
| Preset patterns (email, URL, IPv4, date, hex colour) | 2 | `[[example]]` preset chips on the page, using exactly those pattern families. |
| Stated input ceiling | 3 (55 MB) | `max_lines` (default 1000, max 20000) with an explicit truncation notice in the output and on the page. |
| Blank-line / whitespace handling | practical need for pasted lists | `skip_blank` and `trim` booleans. |
| Copy / download the result | 1, 3 | Provided platform-wide by the generator for `format = "text"` pages (Copy result + Download). |

### Out of model — listed, not built

| Capability | Seen on | Why not |
|---|---|---|
| Multiple regex flavours (PCRE, Python, Java, Ruby, MySQL) | 4 | This block uses the Rust `regex` crate — one flavour. Shipping a flavour picker would be a lie about the engine. |
| Backreferences and look-around | 4 (PCRE/Python flavours) | Not supported by the Rust `regex` engine (linear-time guarantee). Documented as a limit on the page instead. |
| Substitution / replace preview | 4 | Different tool shape (a replace tool, not a match reporter); out of scope for a per-line match reporter. |
| Regex visualiser / railroad diagram | 4 | Needs a bespoke rendering surface the generic tool page does not have. |
| Live match highlighting inside the input box | 1, 2 | Requires per-tool JS overlaying the textarea; the generic page renderer has no such control kind. |
| Spreadsheet upload (`.csv`, `.xlsx`) + column selection | 3 | Column-scoped regex validation already exists in this repo as `regex-column-validate`; this tool is deliberately the line-oriented sibling. |
| Sticky (`y`) and unicode (`u`) JS flags | 1, 2 | Not Rust-regex concepts — Rust regex is Unicode-aware by default, and there is no sticky mode. |
| Save / share pattern links with passwords | 4 | Account/storage feature; this repo ships stateless tools (deep links via `?param=` already cover sharing). |

## Duplicate check

`ls blocks/ | grep -i regex` → `regex-capture-to-csv`, `regex-column-validate`, `regex-extract`,
`regex-literal-escape`, `regex-search`, `regex-tester`, `regex-to-json`, `text-splitter-regex`.
Each was inspected; none reports a per-line pass/fail table:

- `regex-tester` — one text blob, lists every match + groups. No per-line verdicts.
- `regex-search` — grep-style: returns the lines that match (and `invert` returns those that
  don't). It never reports the non-selected lines, so it cannot answer "which of these 200 IDs are
  valid, and what did each capture".
- `regex-extract` — returns matches only, no line accounting.
- `regex-capture-to-csv` — one row per MATCH in one text blob; non-matching input is invisible.
- `regex-column-validate` — CSV/column scoped, and reports only offending rows.

`regex-bulk-match` is the line-oriented match/no-match reporter with per-line captures. Not a
duplicate.
