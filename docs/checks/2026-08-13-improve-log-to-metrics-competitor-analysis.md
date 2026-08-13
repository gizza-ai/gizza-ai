# log-to-metrics — competitor analysis (2026-08-13)

Scan run BEFORE implementing. Sources are the public documentation of the tools named
below; everything here is a paraphrase of documented behaviour — no competitor copy,
branding or trademarked wording is reused in the tool, its page, or this note.

## What the tool has to do

Take already-structured log lines (JSON/NDJSON, logfmt, CSV) and turn them into the
classic RED-style metric set: **rate** (throughput), **errors**, **duration**
(latency percentiles), grouped by one or more arbitrary fields.

## Competitors skimmed

| # | Tool | Shape | Relevant capability |
|---|------|-------|---------------------|
| 1 | angle-grinder (`agrind`, CLI, MIT) | pipeline query language over a log file | `json` / `logfmt` / `parse "…"` / `split` input stages, then `count`, `count_distinct(f)`, `sum(f)`, `average(f)`, `pXX(f)`, `total(f)` aggregators with `by k1, k2`, `sort by … [asc|desc]`, and `--output json|logfmt|format=…` |
| 2 | Grafana Loki / LogQL | server-side query language over log streams | `rate(…[5m])` = entries per second, `count_over_time`, `bytes_rate`; `| unwrap <label>` promotes a numeric field to a sample so `quantile_over_time(φ, …)`, `avg/min/max/sum/stddev_over_time` work; `by (…)` / `without (…)` grouping; `topk(k, …)`; string labels are float-parsed on unwrap |
| 3 | Elasticsearch `percentiles` aggregation | search-time metric aggregation | default percentile set `[1, 5, 25, 50, 75, 95, 99]`; options `percents`, `keyed`, `missing` (value substituted for docs lacking the field), `compression`; approximate (TDigest) and explicitly **non-deterministic** for large inputs; `hdr` alternative for latency |
| 4 (context) | Splunk `stats` (docs 403 to the fetcher; behaviour from the widely-documented command surface) | search command | `count`, `dc`, `avg`, `min`, `max`, `sum`, `median`, `perc<X>`, `stdev`, `per_second`, all with a `BY` clause and an "other" bucket for truncated group lists |
| 5 (context) | Datadog distributions | hosted metric type | user-defined percentiles (p50/p75/p90/p95/p99, up to two decimals, e.g. 99.99) over raw values |

## Table stakes extracted → where they land

| Capability | Competitor precedent | In model? | Decision |
|---|---|---|---|
| Group by N arbitrary fields | agrind `by a, b`, LogQL `by (…)`, Splunk `BY` | yes | `group_by` (comma list, up to 5 fields); blank = one `(all)` row |
| Count + share of total | all | yes | `count` + `percent` columns, always present |
| Rate (per second/minute/hour) | LogQL `rate`, Splunk `per_second` | yes | `rate` column derived from the log's own time span (`time_field`, auto-detected) + `rate_unit` (`auto`/`second`/`minute`/`hour`) |
| Latency percentiles, configurable set | agrind `pXX`, Elastic `percents`, Datadog | yes | `percentiles` (comma list, decimals allowed: `99.9`), default `50,95,99`, max 10 |
| Percentile method | Elastic TDigest (approx.), Splunk nearest-rank | yes | exact, in-memory: `percentile_method` = `linear` (numpy/R-7, matches the existing descriptive-stats tool) or `nearest` (nearest-rank, what most exporters report). Deterministic — unlike the approximate sketch-based implementations |
| min/avg/max/sum of the numeric field | agrind, LogQL `*_over_time`, Splunk | yes | emitted whenever `value_field` is set |
| Numeric field parsing from strings | LogQL unwrap float-parses label strings | yes | numeric strings parsed; a trailing duration unit (`ns/us/µs/ms/s/m/h`) is normalised to milliseconds; unparseable values are counted and reported, not silently dropped |
| Error counting / error rate | Splunk eval+count idiom, RED dashboards | yes | `error_field` + `error_values` (case-insensitive; `5*` prefix wildcards and `>=500`/`>400` comparisons); blank list uses a built-in default set |
| Sort by any metric | agrind `sort by … desc` | yes | `sort` = `count`/`group`/`sum`/`avg`/`max`/`errors`/`p_top` |
| Top-N + "other" rollup | Splunk `useother`, LogQL `topk` | yes | `limit` (default 20) + `other` checkbox that rolls the remainder into an `(other)` row (its percentiles are computed from the merged values, not averaged) |
| Multiple input formats | agrind `json`/`logfmt`/`split` | yes | `format` = `auto`/`json`/`logfmt`/`csv`, with nested JSON flattened to dotted paths |
| Machine-readable output | agrind `--output json`, all | yes | `output` = `table`/`json`/`csv`/`prometheus` (Prometheus text exposition, summary type with `quantile` labels — the natural "logs → metrics" hand-off) |
| Missing-field handling | Elastic `missing` | yes | grouped as `(missing)` rather than dropping the row, and counted |
| Distinct count (`dc`) | agrind, Splunk | yes but low value here | shipped as a `distinct` column only when `distinct_field` would be needed — **not built**; listed below instead to keep the form focused |

## Out of model (documented, not built)

- **Streaming / live tailing and time-bucketed series** (LogQL range vectors, Datadog time
  series). This tool is one-shot over pasted text; it reports one row per group over the whole
  input, not a per-interval series. The existing log-analyzer tool covers volume timelines.
- **Approximate sketches (TDigest/HDR) and cross-shard percentile merging** — irrelevant for a
  single in-memory batch; exact percentiles are strictly better here.
- **A full query language** (agrind's pipeline, LogQL's filters, Splunk's `eval`). Filtering
  before aggregation belongs to the existing log-parser tool, which can feed this one.
- **Distinct-count / cardinality aggregations** (`dc`, `count_distinct`) — omitted to keep the
  parameter surface manageable; csv-stats already reports distinct counts per column.
- **Server-side storage, alerting, dashboards** — out of scope for a stateless local tool.

## Positioning vs. the neighbouring gizza blocks

- `log-parser` — row-by-row structured view + filtering (feeds this tool).
- `log-analyzer` — fixed dimensions (severity counts, top errors, volume timeline).
- `log-pattern-miner` — clusters raw lines into message templates.
- `csv-stats` / `descriptive-stats` — per-column or single-list statistics, no grouping.
- **log-to-metrics** — the missing piece: group-by-any-field counts, rates, error rates and
  exact latency percentiles, plus a Prometheus exposition output.
