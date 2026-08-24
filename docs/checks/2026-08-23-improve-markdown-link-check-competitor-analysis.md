# markdown-link-check — competitor analysis (2026-08-23)

Scan run **before** implementing. All findings are paraphrased from public product
pages; no competitor copy, branding or trademarks are reproduced or reused.

## Competitors reviewed

1. **DevToolbox — Markdown Link Checker** (`dev-toolbox.tech`)
   Extracts every link and validates it over HTTP. Recognises inline links,
   reference-style links, images, angle-bracket autolinks, and bare URLs in prose.
   Sends HEAD with a GET fallback, ~10 s per-URL timeout, batches of ~50. Colour-codes
   2xx / 3xx / 4xx-5xx. Output is a table of line number, link text, URL and link type,
   with All / Broken / Redirects filters and a "copy report" button that emits a
   Markdown-formatted issue summary. Explicitly does **not** validate relative paths,
   fragment-only anchors, or `mailto:`.

2. **Markdown Viewer — Markdown Link Checker** (`trymarkdownviewer.com`)
   Purely structural, in-browser. Classifies each link/image as anchor, external,
   relative, mailto, image or empty. Verifies `#anchor` targets against the document's
   own heading slugs; flags empty targets `[]()`; warns on images with no alt text;
   flags malformed mail addresses. Table output with line, kind, status (ok / warn /
   error), text, target and a note. Controls: paste or open a `.md` file, a kind
   filter, and a "hide passing links" toggle. FAQ covers what it checks, why external
   reachability is limited in a browser, how anchors are matched, alt-text detection,
   privacy, and how it compares to the npm `markdown-link-check` CLI.

3. **md0 — Link Checker** (`md0.io`)
   Extraction and categorisation only. Sorts targets into external, relative, anchor
   and image buckets and lists them; states plainly that it does not validate anchors
   or reachability and points users at CLI checkers for live status. No size limit
   advertised.

(A fourth result, `puredevtools.tools`, returned HTTP 403 and was replaced by md0.)

## Table stakes → decision

| Capability | Seen in | Decision |
| --- | --- | --- |
| Parse inline `[t](u)`, images `![a](u)`, reference `[t][r]` + collapsed `[t][]`, autolinks `<u>`, and `[r]: u` definitions | all 3 | **In model** — implemented in the core scanner |
| Classify each link (anchor / external / relative / mailto / image / reference) | 2, 3 | **In model** — `kind` on every link, plus a `link_kind` filter param |
| Validate in-document `#anchor` targets against heading slugs | 2 | **In model** — GitHub-style slugger with duplicate `-1`/`-2` suffixes, `{#custom-id}` and `<a id=…>` anchors honoured; `check_anchors` toggle |
| Flag empty targets `[]( )` and empty link text | 2 | **In model** — ML001 / ML002 |
| Warn on images with no alt text | 2 | **In model** — ML003 |
| Undefined reference `[t][r]` with no definition | 2 (implied) | **In model** — ML004 |
| Duplicate reference definitions | named in the tool brief | **In model** — ML005 (first definition wins, later ones reported) |
| Unused reference definitions | none (gap we close) | **In model** — ML006 |
| Malformed `mailto:` address | 2 | **In model** — ML010 |
| Reversed link syntax `(text)[url]` | none (markdownlint MD011) | **In model** — ML008 |
| Unencoded space in a URL | none (gap we close) | **In model** — ML009 |
| Unclosed link syntax `[text](url` | none (gap we close) | **In model** — ML012 |
| Insecure `http://` links | 1 (as a status colour) | **In model, opt-in** — ML011 behind `flag_insecure` (default off; docs legitimately cite `http://` examples) |
| Hide / show passing links | 2 | **In model** — `show_ok` boolean |
| Copyable Markdown issue report | 1 | **In model** — `report_format = "markdown"` renders a table; `"json"` for CI |
| Filter to a single link kind | 2 | **In model** — `link_kind` enum |
| Load a sample document / open a `.md` file | 1, 2 | **Partly in model** — the page ships preset example chips; file upload is not a control kind for pure text tools here |
| **Live HTTP status of external URLs** (HEAD/GET, redirects, response time, batching) | 1, 2 (partial) | **Out of model** — the page runs entirely in WebAssembly with no network egress, and browser CORS blocks HEAD against arbitrary hosts (competitor 2 concedes this itself; competitor 3 defers to CLI tools). Shipping it would mean an unverifiable remote-fetch path in the wasm/page surface. **Not built**; stated as a limit on the page and in the FAQ. |
| **Relative path existence on disk** | 1 (claimed), 2 | **Out of model** — there is no filesystem in the page/wasm surface; a single pasted document cannot resolve `../other.md`. Relative targets are still classified and syntax-checked. **Not built**; stated as a limit. |

## Design decisions taken from the scan

- **Structural-only, stated up front.** Two of three competitors already concede that
  browser link checking cannot do live HTTP. We make that the tool's honest promise:
  every check is local, deterministic and offline, so results are reproducible in CI.
- **Rule IDs.** Findings carry stable `MLxxx` IDs so a report can be diffed between
  runs and grepped in CI, mirroring the markdownlint convention users already know.
- **Severity split.** `error` for things that render wrong or dead-link (empty target,
  undefined reference, broken anchor, reversed syntax, unclosed syntax); `warn` for
  hygiene (missing alt text, unused definition, unencoded space, insecure scheme).
  Exit-worthy vs. advisory is then obvious to a CI consumer.
- **Three report formats.** `text` (default, `line:col severity rule message`),
  `markdown` (a table, matching the "copy report" affordance competitor 1 ships), and
  `json` (machine-readable, which no competitor offers — our differentiator).
- **Preset chips** on the page cover the common shapes (a clean doc, a doc with broken
  anchors, a doc with reference-definition problems), replacing the "Sample" buttons
  competitors ship.
- **Code is never scanned.** Fenced blocks and inline code spans are masked before
  scanning, so a `[foo](bar` inside a shell snippet is not a finding — none of the
  three competitors document doing this.
- **1 MB input cap**, stated on the page and enforced with a clear error.
