# jmespath-query — competitor analysis (2026-08-22)

Scan run BEFORE implementation, so every table stake below either landed in the shipped
descriptor/page or is listed as out-of-model. Findings are **paraphrased** — no competitor
copy, branding, or trademarks were reproduced.

## Competitors reviewed

| # | Tool | What it is |
|---|------|------------|
| 1 | jmespath.org (official spec site) | The reference playground: expression box + JSON box, live result pane, linked tutorial/spec |
| 2 | Mockoon — JMESPath evaluator | Standalone dev-tool page; pre-filled sample document, live result as you type, syntax + built-in-function primer below the widget |
| 3 | jsonpath.online — JMESPath tester | Live evaluator (~300 ms debounce), preset query buttons, copy-result, code generation for 6 languages, function reference + FAQ |
| 4 | mixedanalytics.com — JMESPath expression tester | Minimal two-box tester pre-filled with a filter example (`locations[?state == 'WA']`), explicit "runs in your browser, nothing stored" claim |
| 5 | hidekazu-konishi.com — JSONPath/JMESPath query tester | Dual-language tester framed around the AWS CLI `--query` use case, all client-side |

## Table stakes → where each landed

| Table stake | Seen at | Decision |
|---|---|---|
| Expression input + JSON document input | all 5 | **In-model, shipped** — `expression` + `json`, both required |
| Result shown as JSON | all 5 | **In-model, shipped** — serialized JSON result |
| Pretty/indented output | 2, 3 | **In-model, shipped** — `pretty`, defaulted **true** (every competitor shows indented output by default; the JMESPath result is one value, so indenting costs nothing) |
| Pre-filled sample data + example expressions | 1, 2, 3, 4 | **In-model, shipped** — three `[[example]]` chips (projection, filter, multiselect-hash) + real placeholders |
| Preset/quick-query buttons | 3 | **In-model, shipped** — same `[[example]]` chips |
| Copy result button | 3 | **In-model, shipped** — platform gives every text page Copy + Reset |
| Clear parse/eval errors | 1, 2, 3 | **In-model, shipped** — distinct `invalid JSON input:` / `invalid JMESPath expression:` / `JMESPath evaluation error:` prefixes carrying the engine's position info |
| Syntax primer + built-in function list | 1, 2, 3 | **In-model, shipped** — syntax section + grouped built-in function list in `page/content.md` |
| FAQ (what it is, vs JSONPath, vs jq, privacy, AWS CLI) | 3 | **In-model, shipped** — 5 `<details>` accordions |
| "Runs in your browser, nothing uploaded" | 3, 4, 5 | **In-model, shipped** — stated in hero + copy (wasm, no network) |
| AWS CLI `--query` framing | 3, 5 | **In-model, shipped** — page copy + FAQ position the tool as a place to draft/debug an `--query` expression offline; tags/description carry the angle |
| Unquoted string output (`--output text` / `jq -r` behaviour) | AWS CLI itself | **In-model, shipped** — `raw` boolean: when the result is a JSON string, emit it without quotes; string members of a top-level array are emitted one per line |

## Considered, not built

- **Code generation for Python/JS/Go/Java/PHP/C#** (competitor 3) — that is a *code emitter*, a
  different tool shape from "evaluate this expression"; it would double the descriptor surface for
  output nobody can verify in this widget. Deliberately declined, not blocked.
- **CSV export of the result** (competitor 3) — already this repo's `json-to-csv` /
  `json-to-html-table` territory; chaining beats duplicating.
- **Syntax highlighting / editor gutter in the input box** — the shared page runtime uses plain
  fields/textareas; adding a code editor is a platform change well beyond this tool, and would have
  to be declarative for every tool rather than a per-slug hack.
- **Live-as-you-type debounce** (competitor 3's 300 ms) — the shared runtime already re-runs on
  `input`; no per-tool work needed or wanted.

## Out-of-model (cannot run browser-local, no-account, no-server)

- Saved/shared query permalinks backed by a server, accounts, or history sync. (The page's
  `?expression=…&json=…` deep link is the local-only equivalent and *is* shipped.)
- Fetching the JSON document from a remote API URL inside the page (that is `web-fetch`'s job and
  needs a network capability the page target does not have).
- Running the expression against a live AWS account's API response.

## Family fit

`jq-query`, `jsonpath-query`, `jsonata-query` and `xpath-query` are already separate blocks — one
per expression *language*, which the skiplist's `jsonpath-validate` entry records as the
intentional shape. JMESPath is a fifth, distinct language (an IETF-draft spec, and the language
behind the AWS CLI `--query` flag); it is not a semantic duplicate of any of them. Parameter naming
follows the family: two required inputs plus two output-shaping booleans.
