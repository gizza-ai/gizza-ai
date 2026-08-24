# json-assertion-runner — competitor analysis (2026-08-22)

Scan run BEFORE implementation, per `.claude/skills/create-tool-loop/SKILL.md` step 4. All
findings are paraphrased from public documentation; no competitor copy, branding, or trademark
text is reproduced or reused in the tool's page/CLI copy.

## Scope

"Run declarative assertions against a JSON payload and report pass/fail." The closest real-world
prior art is not a single web toy but the **API-test assertion layer** shipped by test frameworks
and API clients — that is where the vocabulary of assertion operators is actually standardised.

## Competitors reviewed

| # | Competitor | What it is | Reachable |
|---|---|---|---|
| 1 | `steinfletcher/apitest-jsonpath` (Go) | JSONPath assertion helpers for the `apitest` HTTP test library | yes |
| 2 | `json-path/JsonPath` → `json-path-assert` (Java) | Hamcrest matchers built on the Jayway JSONPath engine | yes |
| 3 | `martin-helmich/phpunit-json-assert` (PHP) | PHPUnit assertions that address JSON documents by JSONPath | yes |
| 4 | SmartBear SoapUI / ReadyAPI JsonPath assertions (via ToolsQA walkthrough) | The four GUI assertion types an API client exposes | doc page reachable via ToolsQA; the vendor's own `json-match` page returned HTTP 403, so the ToolsQA walkthrough was used as the substitute source |

(A fifth angle — dataformatterpro's JSONPath tester — was skimmed and rejected as a comparable:
it *extracts* values and has no assertion/pass-fail concept, which is the same reason our own
existing `jsonpath-query` block is not a duplicate of this tool.)

## Table-stakes matrix

Every operator below is either **in the descriptor** or explicitly listed as out-of-model. None
were dropped silently.

| Capability | Seen in | Decision |
|---|---|---|
| Existence check on a path | 1 (`Present`), 2 (`hasJsonPath`), 4 (Existence Match) | **in** — `exists` |
| Absence check on a path | 1 (`NotPresent`), 2 (`hasNoJsonPath`) | **in** — `not_exists` |
| Value equality | 1 (`Equal`), 2 (`equalTo`), 3 (`assertJsonValueEquals`), 4 (JsonPath Match) | **in** — `equals` |
| Value inequality | 1 (`NotEqual`) | **in** — `not_equals` |
| JSON type check (string/number/object/array/…) | 3 (schema-backed type checks), general practice | **in** — `type` (adds `integer` as a distinct check) |
| Regex match on a value | 1 (`Matches`), 4 (JsonPath RegEx Match) | **in** — `matches` |
| Membership / substring containment | 1 (`Contains`), 2 (`hasItem`) | **in** — `contains` / `not_contains` (array membership, string substring, object key) |
| Length of the matched value | 1 (`Len`), 2 (`hasSize`) | **in** — `length` |
| Count of nodes the path matched | 4 (JsonPath Count) | **in** — `count` (distinct from `length`: nodes selected vs. size of one node) |
| Numeric bounds / min-max | 1 (`GreaterThan`/`LessThan`), general "numeric tolerance" guidance | **in** — `gt`, `gte`, `lt`, `lte`, plus an inclusive `range min..max` |
| Emptiness | 2 (`empty()` composition) | **in** — `empty` / `not_empty` |
| Case-insensitive string comparison | 4 (client-side ignore-case option) | **in** — `case_sensitive` boolean (applies to `equals`/`not_equals`/`contains`/`not_contains` on strings and to `matches`) |
| Several assertions in one run, reported together | 1 (`Chain`), 3 (`assertJsonDocumentMatches`) | **in** — the whole tool is a multi-rule runner; both a line DSL and a JSON rule array are accepted |
| Named assertions in the report | 3 (constraint labels), 4 (assertion names in the GUI) | **in** — optional `name` on a JSON rule |
| Stop at the first failure | common runner behaviour | **in** — `stop_on_first_failure` boolean |
| Machine-readable report for CI | guidance around JUnit-style output + run-log artifacts | **partly in** — `output = json` gives a full machine-readable report object. JUnit **XML** specifically is **out of model** (see below) |
| Whole-document JSON Schema validation | 3 (`assertJsonDocumentMatchesSchema`) | **out of model here** — already shipped as `blocks/json-schema-batch-validate`; pointing at it is better than duplicating it |
| JWT header/payload assertions | 1 (`JWTHeaderEqual`/`JWTPayloadEqual`) | **out of model** — JWT decoding is a different tool's job |
| Shared root path to de-duplicate prefixes | 1 (`Root`) | **out of model** for v1 — one line per assertion stays readable; revisit if users ask |
| HTTP status/header assertions | 1, 4 (they assert on a live response) | **out of model** — gizza tools are offline pure-compute; there is no request to make |

## UX / control patterns adopted

- Competitors are all code or GUI *forms*; the closest transferable pattern is the GUI's
  "one row per assertion" list (4). Our page keeps the equivalent as a multiline rules textarea
  with a one-assertion-per-line DSL, so it stays copy-pasteable into a repo.
- Preset buttons: the API clients ship "select from current response" helpers. The offline
  equivalent is `[[example]]` preset chips on the page — three are shipped (a passing run, a
  mixed pass/fail run, and a JSON-report run).
- Report shape follows the runner convention seen across all four: an overall verdict line, a
  pass/fail count, then one line per assertion with the actual value shown on failures.

## Explicit non-goals / stated limits (also documented on the page)

- No network: the payload is pasted, not fetched.
- JUnit XML output is not emitted; `output = json` is the machine-readable form.
- In the line DSL the path is the first whitespace-delimited token, so paths containing spaces
  (e.g. `$['my key']`) must use the JSON rules format.
- At most 200 assertions per run.
