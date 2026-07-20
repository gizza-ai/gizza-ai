# regex-to-json — competitor analysis (2026-07-20)

New tool built from the backlog: "Parses each line/record of text with a
named-capture regex and emits structured JSON objects keyed by group name."

Dup check against existing blocks: `regex-extract` returns a flat list of ONE
chosen group's matches; `regex-tester` is a debugging breakdown (spans + all
groups per match); `log-parser` auto-detects known formats (JSON/logfmt/syslog/
access logs) with no user-supplied pattern. None emits user-schema'd data
records — regex-to-json is the text→data converter in the family. Cross-links
added in the core doc comment and the page FAQ.

## Competitors scanned (paraphrased — no copy/branding reused)

1. **Cribl Stream "Regex Extract" function** (docs.cribl.io) — event-pipeline
   function: named capture groups become event fields (JSON keys); options for
   iterating multiple matches with a max-exec cap, chaining additional regexes,
   transforming field names via an expression, and overwrite-vs-array on field
   collisions. Non-matching events pass through unmodified.
2. **Sumo Logic `parse regex` operator** — named capture groups
   `(?<field>…)`; `multi` parses every occurrence within one message (one
   result copy per match); `nodrop` keeps messages that match nothing;
   `(?i)` inline flag for case-insensitivity; if the pattern fails on a line no
   fields are assigned.
3. **AWS CloudWatch Logs Insights `parse` (regex mode)** — named capture
   groups produce fields; non-matching events are kept without the extracted
   fields; `multi` keyword emits one row per match; sibling modes exist for
   glob/logfmt/CSV input.

Context also skimmed: regex101/New Relic log-parsing testers (interactive
group breakdown + type-tagged Grok extraction).

## Table stakes → disposition

| Capability | Competitors | Disposition |
| --- | --- | --- |
| Named groups become JSON keys, `(?<n>…)` syntax | all three | **in-model** (`pattern`, also accepts Python `(?P<n>…)`; pattern with no named group errors with a hint) |
| One record per line | Cribl/Sumo/CloudWatch operate per event/line | **in-model** (line splitting, CRLF-safe, blank lines ignored) |
| Multiple matches per line (`multi`) | Sumo, CloudWatch, Cribl (max-exec loop) | **in-model** (`all_matches` boolean) |
| Keep non-matching lines (`nodrop` / pass-through) | all three | **in-model** (`unmatched` enum: skip / keep as `{"_raw": line}` / fail — fail is our validator addition) |
| Case-insensitive matching | Sumo `(?i)`, testers' flags | **in-model** (`ignore_case` boolean; inline `(?i)` also works) |
| Typed values (Grok-style `:int`) | New Relic Grok; Cribl keeps strings | **in-model** (`coerce_types` boolean, conservative rules: plain int/decimal/true/false/null only; leading-zero + sci-notation + i64-overflow stay strings) |
| Machine-consumable output shape | pipelines emit events; converters offer array vs JSON-lines | **in-model** (`output` enum: pretty `json` / `compact` array / `ndjson`) |
| Preset examples / one-click demos | testers ship pattern presets | **in-model** (3 `[[example]]` chips: server log → JSON, app log → NDJSON, key=value all-matches) |
| Stable record schema across rows | implied by field mapping | **in-model** (non-participating groups emit `null`; key order = pattern group order via preserve_order) |

## Out-of-model (listed, not built)

- **Chained/additional regexes + field-name transform expressions** (Cribl) —
  JS-expression pipeline features; out of scope for a single-pass converter.
- **Grok pattern library / per-field type annotations** (New Relic, Logstash) —
  a curated pattern library is a separate product surface.
- **PCRE features** (look-around, backreferences) — the Rust `regex` engine is
  linear-time by design; documented as a page limit + FAQ instead.
- **Custom record separators** (multi-line records, blank-line-separated
  stanzas) — records are lines here; documented on the page.
- **Field-collision overwrite-vs-array policy** (Cribl) — moot: the Rust regex
  engine rejects duplicate group names at compile time.

## Verification snapshot

- 20 unit tests (core 18 + drift-guard + args-defaults), incl. cap boundary at
  1,000,000 and 1,000,001 bytes.
- CLI matrix: every enum choice (`unmatched` skip/keep/fail, `output`
  json/compact/ndjson), both named-group spellings, coercion on/off,
  `all_matches`, `ignore_case`, no-named-group + invalid-pattern errors; the
  page's generated CLI example runs verbatim and succeeds.
- Playwright (10 tests): exact pretty/compact/ndjson outputs, keep/fail
  unmatched modes, non-default checkboxes (all_matches, coerce_types,
  ignore_case), `(?P<name>)` form, no-named-group error, 1 MB cap at/over
  boundary (direct-set + input event for the big fixture), example chip run,
  `?param=` deep-link.
