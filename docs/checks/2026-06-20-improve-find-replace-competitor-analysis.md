# find-replace — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/find-replace` — find & replace text with literal or
regular-expression matching, case and global options. Chat + CLI + page
(pure-text, field inputs, `regex` crate).

## What competitors do

- **Online find-and-replace sites** (texthandler, online find replace,
  replace.surge, browserling) — paste text, find/replace. Strengths: simple.
  Weaknesses: many are literal-only or regex-only (not both), some lack a
  case toggle or "first match only", and a few send the text to a server.
- **Editor find/replace** (VS Code, Sublime, sed/`s///`) — powerful with regex +
  capture groups, but tied to an editor or the shell, not a shareable web tool.
- **Regex testers** (regex101) — great for crafting patterns, but oriented to
  testing, not bulk replacing and downloading the result.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`regex` crate) compiled to
   wasm, so the page does it in-browser and the CLI runs headless. Text never
   leaves the device.
2. **Literal *and* regex in one tool.** Default literal mode escapes regex
   metacharacters (so `.` `*` `(` are matched literally — a common footgun on
   regex-only tools); flip `regex=true` for full regular expressions with
   `$1`/`${name}` capture references in the replacement.
3. **Safe literal replacement.** In literal mode the replacement is inserted
   verbatim (NoExpand), so a replacement containing `$` isn't mis-interpreted as
   a group reference — something naive `s///` wrappers get wrong.
4. **Case + scope toggles.** `case_sensitive` (default true) and `global`
   (default true; false = first match only) cover the common variations.
5. **Reports the replacement count**, so you know how many matches were affected
   (the page shows the result text; chat/CLI return `{text, count}`).
6. **Three surfaces.** Same engine in chat (LLM tool), the CLI, and a shareable
   page with query-param deep-links.

## Honest scope

- Regex flavour is the Rust `regex` crate (RE2-style: linear-time, **no
  backreferences or lookaround**). This is a deliberate safety/performance
  trade-off — patterns can't catastrophically backtrack.
- Replacement capture syntax is `$1` / `${name}` (not `\1`).

## Tests

9 core unit tests: literal-global with escaped metachars (`a.b.c`→`a-b-c`, n=2),
regex digit class, case-insensitive (n=3) vs case-sensitive default (n=1),
replace-first-only, regex capture-group swap (`John Smith`→`Smith John`), literal
`$` not expanded, no-match → unchanged/0, and error cases (empty find, invalid
regex). Plus the block drift-guard schema test. CLI verified for all three modes
(literal, regex capture swap, case-insensitive first-only). Playwright: a literal
replace on the page and a regex capture-group replace via query-param deep-link
both pass.
