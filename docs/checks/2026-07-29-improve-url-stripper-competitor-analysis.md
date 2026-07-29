# url-stripper — competitor analysis (2026-07-29)

Function: remove web links from a block of text — `http/https/ftp` URLs and, by default,
scheme-less `www.` links — optionally bare email addresses too, replacing each with nothing
or a chosen placeholder, then tidying the whitespace left behind so the result reads as
clean prose. Pure-compute, runs entirely in the browser / CLI / chat sandbox; no link is
ever fetched.

## Competitors surveyed (top real tools)

| # | Tool | Removes www. (schemeless) | Also removes emails | Replace-with token | Whitespace cleanup | Client-side |
|---|------|---------------------------|---------------------|--------------------|--------------------|-------------|
| 1 | Browserling — Remove URLs from Text | partial | no | no | no | yes |
| 2 | TextFixer / TextMechanic — Remove Lines/URLs | no | no | no | line-level only | yes |
| 3 | Prepostseo — Remove URL from Text | yes | no | no | minimal | server |
| 4 | Editpad — Remove URLs | partial | no | no | no | yes |
| 5 | Code Beautify — Strip HTML/URLs | no | no | no | no | yes |

Paraphrased from public tool pages; no copy/branding/trademarks reproduced.

## Table-stakes → decision

| Capability | In competitors | Decision |
|------------|----------------|----------|
| Remove http/https URLs | all | **IN** — `SCHEME_RE` matches `https?`/`ftp` schemes. |
| Remove ftp URLs | some | **IN** — same scheme regex covers `ftp://`. |
| Remove scheme-less `www.` links | 1, 3 (partial) | **IN** — `remove_www` boolean (default true); most fielded tools miss bare `www.`. |
| Remove bare email addresses | none surveyed | **IN (edge)** — `remove_emails` boolean (default false), same pass. |
| Replace each link with a placeholder | none surveyed | **IN (edge)** — `replacement` string (empty = delete, or `[link]` etc.). Competitors only delete. |
| Clean up leftover whitespace | 2 (line-level only) | **IN (edge)** — `collapse_whitespace` (default true): collapses double spaces, drops space-before-punctuation, removes now-empty brackets, trims lines, keeps newlines/blank lines. |
| Keep sentence punctuation glued to a URL | none surveyed | **IN (edge)** — `trim_trailing` strips `.,;:!?` off the match so `See https://x.com/y.` → `See.`. |
| Report count of links removed | none surveyed | **IN (chat/CLI JSON)** — returns `urls_removed` / `emails_removed`; page shows cleaned text only. |
| Instant, client-side, no upload | most | **IN** — pure Rust/WASM in-browser, plus CLI + chat sandbox. |

## Out-of-model / not built

- **Following/validating links (dead-link check, unshortening):** requires network fetches;
  the tool is deliberately offline/no-fetch. Listed, not built.
- **Markdown/HTML link-syntax rewriting** (e.g. turning `[docs](url)` into just `docs`):
  out of scope — it strips the raw URL text and leaves the label/brackets. Documented as a
  limit, not built.
- **Whitelist/blacklist by domain:** a link-filter feature no surveyed competitor offers and
  outside the "strip everything" remit; not built.

## UX / page controls shipped

- `input` → multiline textarea with a realistic mixed URL/email/www. placeholder.
- `remove_emails` / `remove_www` / `collapse_whitespace` → checkboxes (defaults off/on/on).
- `replacement` → text field (blank = delete; placeholder hints `[link]`).
- `[[example]]` preset chips: strip-a-paragraph, links+emails-with-placeholder, and
  keep-www examples — one-click prefilled.
- Worked example + 4 FAQ accordions + stated limits (pattern-based detection, delimiter
  behaviour, non-deduped counts, no markdown rewriting) in `content.md`.

## Verification

- Descriptor drift test (`schema_json_matches_authored_chat_schema`) added in
  `src/lib.rs`; core `strip`/`render` unit tests already cover scheme/www/ftp/email removal,
  placeholder token, punctuation retention, bracket cleanup, and newline preservation.
- Chat block returns the `Stripped { text, urls_removed, emails_removed }` struct; web wasm
  export and page pass all five fields through to core `render`.
