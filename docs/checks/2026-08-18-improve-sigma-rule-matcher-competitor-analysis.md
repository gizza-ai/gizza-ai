# sigma-rule-matcher — competitor scan (2026-08-18)

Scan run BEFORE implementing, per `/create-next-tool` step 4. Everything below is a
**paraphrased** summary of publicly documented behaviour — no competitor copy, branding,
logos or trademarks are reproduced anywhere in this repo.

## Who was looked at

| # | Tool | What it is | Why it is the bar |
|---|------|-----------|-------------------|
| 1 | Chainsaw (WithSecure Labs) | Rust CLI that loads EVTX, converts to JSON and runs Sigma detection logic over it via a matching engine | The closest analogue: Sigma logic evaluated locally over Windows event records |
| 2 | Hayabusa (Yamato Security) | Rust Sigma-based hunting / timeline generator for Windows event logs, ships a curated rule set | Sets expectations for severity ranking, per-rule counts and a triage-first output |
| 3 | sigma-cli / pySigma (SigmaHQ) | The reference Sigma implementation: rule parsing, validation, backend conversion | Defines *correct* rule semantics — modifiers, condition grammar, logsource |
| 4 | Zircolite | Standalone Sigma-on-EVTX/JSON/auditd detector (SQL-backed) that takes already-converted JSON | Proves the "already-parsed JSON in, detections out" shape this tool targets |
| 5 | Sigma specification (SigmaHQ docs) | The rule-format spec itself: detection maps/lists, modifiers, condition expressions | The authority every one of the above is measured against |

## Table stakes observed, and where each landed

### Rule semantics (from the Sigma specification — all in-model, all implemented)

| Capability | In/out of model | Where it landed |
|---|---|---|
| `detection:` selections as maps, lists of maps, and keyword lists | in-model | implemented (map = AND of fields, list of maps = OR, keyword list = substring search over all values) |
| List of values under one field = OR | in-model | implemented |
| `null` value = field absent or null | in-model | implemented |
| Wildcards `*` and `?` in values, with `\*` / `\?` / `\\` escapes | in-model | implemented |
| Case-insensitive string comparison by default | in-model | implemented (`cased` opts back into case-sensitive) |
| Modifiers `contains`, `startswith`, `endswith` | in-model | implemented |
| Modifier `all` (list becomes AND) | in-model | implemented |
| Modifier `re` (+ `i`/`m`/`s` sub-modifiers) | in-model | implemented via the `regex` crate |
| Modifiers `gt`, `gte`, `lt`, `lte` | in-model | implemented (numeric compare) |
| Modifier `cidr` (IPv4/IPv6 subnet) | in-model | implemented (hand-rolled prefix compare on `Ipv4Addr`/`Ipv6Addr`) |
| Modifiers `base64`, `base64offset`, `utf16`/`utf16le`/`utf16be`/`wide` | in-model | implemented (encode the rule value, then match) |
| Modifier `windash` (dash variants `-`, `/`, `–`, `—`, `―`) | in-model | implemented (value expansion) |
| Modifier `exists` (true/false) | in-model | implemented |
| Modifier `fieldref` (compare a field against another field) | in-model | implemented |
| Modifier `cased` | in-model | implemented |
| Modifier `expand` (pipeline placeholders) | **out of model** | listed, not built: placeholders are resolved by a backend pipeline/config that does not exist in a browser-local tool |
| Condition grammar: `and` / `or` / `not`, parentheses, `1 of x`, `all of x`, `N of x`, `* ` wildcards over selection names, `them` | in-model | implemented (Pratt-style parser + evaluator) |
| Multi-document rule files (`---` separated) | in-model | implemented |
| Rule metadata: `title`, `id`, `status`, `level`, `description`, `author`, `tags`, `falsepositives`, `fields`, `logsource` | in-model | parsed and surfaced in the output |

### Product / CLI surface (from Chainsaw, Hayabusa, Zircolite)

| Capability | In/out of model | Where it landed |
|---|---|---|
| Filter loaded rules by severity (`level`) | in-model | `min_level` param (informational → critical, "any" default) |
| Filter loaded rules by `status` (stable/test/experimental/…) | in-model | `status` param |
| Time-range filtering of events (`--from` / `--to`) | in-model | `from` / `to` params (ISO-8601, inclusive) |
| Multiple output shapes — table, JSON, human report | in-model | `output` = `report` / `table` / `json` |
| Cap on the number of reported hits | in-model | `max_matches` (default 500) |
| Show the full matching event vs. a summary row | in-model | `show_event` boolean (JSON output) |
| Field-mapping files that tell the engine which event field a rule field means | in-model, **redesigned** | a mapping *file* is out of scope for a paste-in tool, so field resolution is automatic: exact key → dot-path → inside `System` / `EventData` / `Event.System` / `Event.EventData` → case-insensitive lookup, plus the common aliases (`EventID`, `Provider_Name`, `Channel`, `Computer`) |
| `logsource` gating against the event channel | in-model | `logsource` param: `ignore` (default, permissive) or `match` (channel/provider must line up via a built-in service→channel table) |
| Per-rule / per-level counts for triage | in-model | every output shape carries a summary (rules loaded, events scanned, hits by level, hits by rule) |
| Detection timeline sorted by timestamp | in-model | detections are reported in event order with the event timestamp |
| Reading EVTX **binary** files directly | **out of model here** — but already covered | listed, not built: `evtx-parser` is the existing block that decodes `.evtx` → JSON; this tool consumes that JSON, which keeps each tool one job |
| Curated/bundled rule sets shipped with the tool | **out of model** | listed, not built: bundling and updating thousands of third-party rules is a distribution problem, not a browser-local compute one; rules are pasted in |
| Loading rules from a directory tree / git repo | **out of model** | listed, not built: no filesystem or network in a browser-local tool; multi-document paste is the equivalent |
| Sigma → SIEM backend conversion (Splunk/ES/…) | **out of model** | listed, not built: that is a query-generation product, a different tool entirely |
| Aggregation conditions (`count() by X > N`, `near`) | **out of model** | listed, not built: correlation/aggregation is explicitly outside the core Sigma detection grammar most tools implement, and needs stateful windowing |
| Sigma correlation rules (v2 `correlation:` documents) | **out of model** | listed, not built: same reason; a correlation document is skipped with a clear message rather than silently mis-evaluated |

## UX patterns worth copying (structurally, not verbatim)

- **Severity is the primary axis.** Competitors rank output by level and colour it. Ours ranks
  the summary by level and puts the level first on every row.
- **Rule-count feedback.** They all report how many rules loaded vs. how many were skipped;
  ours reports loaded/skipped counts and *why* a rule was skipped.
- **Preset invocations.** Their docs lead with a copy-pasteable command; the page ships
  `[[example]]` chips (a process-creation hunt, an encoded-PowerShell hunt, a failed-logon
  hunt, a JSON-output run) so a first-time visitor gets a hit in one click.
- **Truncated event view by default, full view on request.** Mirrored with `show_event`.

## Deliberately rejected (in-model but declined)

- **A user-supplied field-mapping table.** In-model (it would just be another string param),
  but it duplicates what automatic resolution already does for real EVTX-shaped JSON and adds
  a second failure mode to explain. Revisit if real rules are seen missing hits.
- **A "why did this not match" rule debugger.** Interesting, but it belongs in the sibling
  backlog row `sigma-rule-validator` (explain a rule), not in the matcher.
