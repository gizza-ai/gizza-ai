# log-pattern-miner — competitor analysis (2026-08-13)

Scan run **before** implementing, per `/create-next-tool` step 3. One web search
("Drain log parser online tool log pattern mining templates cluster log lines"), then the top 3
reachable real tools were skimmed. Everything below is **paraphrased**; no competitor copy,
branding or trademarked text is reproduced or shipped.

## Tools skimmed

| # | Tool | What it is | Surface |
|---|------|------------|---------|
| 1 | Drain3 (logpai) | Python streaming log-template miner; the de-facto reference implementation of the fixed-depth-parse-tree Drain algorithm | library + `drain3.ini` config, no hosted UI |
| 2 | drain-java (bric3) | JVM port of the same algorithm with a `tail`-style CLI front end | CLI + Java API |
| 3 | drain (Go, faceair) | Go port of the same algorithm, embedded in log pipelines | library |

Reference for the algorithm itself: the 2017 ICWS Drain paper (fixed-depth tree, similarity
threshold, per-leaf token-position merge into wildcards).

## Table stakes observed → our decision

| Capability (competitor default) | In model? | Our decision |
|---|---|---|
| Similarity threshold, default 0.4 | yes | `similarity`, number 0–1, **default 0.4** (page slider, step 0.05) |
| Parse-tree depth, default 4 (min 3 in Drain3, i.e. 2 token layers) | yes | `depth`, integer 2–8, **default 4** (we count token layers as `depth - 2`, same as the reference) |
| Max children per internal node, default 100 | yes | `max_children`, integer 2–1000, **default 100** |
| Extra word-splitting delimiters (none by default) | yes | `extra_delimiters`, string, default empty — each character also splits tokens (e.g. `=,:`) |
| Variable masking → typed placeholders (`<IP>`, `<HEX>`, `<*>`), configurable prefix/suffix | partly | Built-in mask set (`<NUM> <HEX> <IP> <MAC> <UUID> <DATE> <TIME> <PATH> <URL> <EMAIL> <STR>`) via `mask = typed` (default); `wildcard` renders every masked slot as `<*>`; `none` disables pre-masking. **Custom user regex masks are out of model** (see below) |
| Output per cluster: id, size, template | yes | `count` + `percent` + `template` in every format; stable rank order |
| Ranked/sorted view of the biggest clusters | yes | Ranked by count desc, first-seen order for ties; `max_patterns` (default 20) and `min_count` (default 1) |
| Prefix stripping before mining (drain-java: cut the line prefix by column / separator) | yes | `skip_tokens`, integer 0–16 — drop the first N whitespace tokens (timestamp/host/pid prefix) before mining |
| Parameter extraction (which variable values filled each slot) | yes | JSON output carries `variables[]` per template: placeholder position + up to 3 sample raw values |
| Sample/original line for a cluster | yes | JSON carries `first_index`/`first_line`, `last_index`/`last_line` and up to 3 distinct `examples`; the table format carries the first/last line numbers |
| Machine-readable output for downstream tooling | yes | `format = json` (full detail), `table` (TSV: count, percent, first, last, template), `lines` (one template per line, pipe-friendly) |

## Out of model (listed, deliberately not built)

- **Streaming / online state.** Drain3 keeps a persistent tree across runs (Kafka/file/Redis
  snapshots, `snapshot_interval_minutes`, `compress_state`) so a long-lived process keeps
  learning. gizza blocks are single-shot pure functions with no storage; this tool mines one
  pasted batch and returns. Same input → same output, always.
- **`max_clusters` with LRU eviction.** That bound exists to cap memory in an unbounded stream.
  In batch mode the equivalent user-facing control is "show me the top N", which is
  `max_patterns`; the miner itself keeps every cluster so counts stay exact.
- **User-supplied regex masking rules** (`masking = [{regex_pattern, mask_with}]` in the
  reference config). Would need a regex engine plus a JSON rule editor on the page for a
  minority use case; the built-in typed mask set plus `extra_delimiters` covers the common
  cases. Revisit if requested.
- **Configurable mask prefix/suffix** (`<`/`>`). Cosmetic; fixed here so output is predictable
  to parse.
- **Persistent cluster IDs across runs.** Meaningless without persisted state (see above); we
  rank by frequency instead, which is what a one-shot triage user actually wants.
- **File tailing / follow mode** (drain-java `-f`). Requires a live filesystem; not available in
  the browser/wasm sandbox.
- **Anomaly detection on top of templates.** A separate tool; templates + counts are the input
  to it, not the same job.

## UX decisions taken from the scan

- Competitors ship *config files*, not UI. The page therefore surfaces the two knobs that
  actually change results — similarity and depth — as **sliders** with the reference defaults,
  and hides nothing behind a config format.
- The reference implementations print `id (size N): template`. We keep the same information but
  add **percent of lines** (the number people actually quote in a triage) and the first/last
  line numbers so a template can be found again in the source file.
- `[[example]]` preset chips ship three real log shapes (SSH auth failures, an app log with
  UUIDs and durations, an nginx-style access log) so the page demonstrates the tool in one
  click, matching the "paste and see" expectation.
- Placeholders are typed by default (`<IP>` reads better in a triage than `<*>`), with a
  one-click `wildcard` mode for people who want the reference tools' plain wildcard rendering.

## Duplicate check

Nearest existing blocks were inspected before building:

- `blocks/log-analyzer` — counts by level, top errors, time span, volume timeline. Its
  "top errors" grouping masks hex/digit runs and groups **only warn/error lines** by exact
  masked string. No similarity clustering, no parse tree, no templates for the other 90 % of
  the log, no per-slot variables. Different deliverable.
- `blocks/cluster-similar-values` — Levenshtein clustering of values in a **CSV column**
  (canonical value + members), not log-line templating.
- `blocks/log-parser`, `blocks/log-to-table`, `blocks/syslog-triage` — field extraction from
  known log formats into rows; they never abstract a message into a template.

Conclusion: not a duplicate; built.
