# downsample-timeseries — competitor analysis (2026-07-20)

Scan done BEFORE implementation (build-time design input). Paraphrased only — no
competitor copy or branding.

## Competitors reviewed

1. **TimescaleDB Toolkit `lttb()`** (tigerdata.com docs) — SQL aggregate:
   `lttb(ts TIMESTAMPTZ, value DOUBLE PRECISION, resolution INT)` → timevector.
   Single required knob: the target point count ("resolution"). Timestamp x values
   are first-class.
2. **tsdownsample** (predict-idlab, Rust-backed Python) — algorithms: `LTTB`,
   `MinMax` (min+max per bin), `M4` (first/min/max/last per bin), `MinMaxLTTB`
   (MinMax prefilter with `minmax_ratio`, default 4, then LTTB), each with a
   NaN-aware variant. Signature `downsample([x], y, n_out)` → **indices** into the
   original arrays; x optional (index used when absent); `parallel` flag.
3. **`lttb` Python package / flot-downsample lineage** (sr.ht javiljoen lttb, the
   original Steinarsson JS `largestTriangleThreeBuckets(data, threshold)`) —
   Nx2 array sorted by x; validators reject unsorted x / bad shapes; requires
   `2 < n_out <= n`; always keeps the first and last point; `threshold = 0` or
   `>= n` returns the data unchanged.

(The PyPI page itself was an unreachable JS shell, so the sr.ht upstream + the
original flot-downsample reference implementation replaced it as the third
source.)

## Table stakes → in-model / out-of-model

| Capability | Source | Tag | Where it landed |
|---|---|---|---|
| Target point count as the single main knob | all three | in-model | `points` (integer, default 100, min 2) |
| LTTB algorithm, exact flot/Steinarsson bucket math | all three | in-model | `algorithm=lttb` (default) |
| MinMax per-bin algorithm | tsdownsample | in-model | `algorithm=minmax` |
| M4 (first/min/max/last per bin) | tsdownsample | in-model | `algorithm=m4` |
| Plain every-nth / uniform stride decimation | tsdownsample (EveryNth), common baseline | in-model | `algorithm=nth` |
| First + last point always kept (LTTB) | lttb pkg / flot | in-model | implemented + unit-tested |
| `points >= n` (or tiny data) → data returned unchanged | flot (`threshold 0/≥n`) | in-model | implemented + documented |
| x optional — use row index when absent | tsdownsample | in-model | 1-column input → x = row index; `x_column=index` forces it |
| Timestamp x values | TimescaleDB | in-model | ISO-8601 / RFC 3339 / `YYYY-MM-DD[ HH:MM[:SS]]` x parsed to epoch |
| Unsorted-x validation with a clear error | lttb pkg validators | in-model | error names the 1-based row |
| Indices output (select-from-original workflow) | tsdownsample | in-model | `output=indices` |
| Column selection for wide CSVs | (table stakes for a paste-in tool; TimescaleDB picks ts/value columns) | in-model | `x_column` / `y_column` (header name or 1-based index) |
| MinMaxLTTB hybrid + `minmax_ratio` | tsdownsample | **out-of-model** | pure perf optimization for >10M points; irrelevant under the 2 MB input cap — plain LTTB is O(n) and instant here |
| `parallel` / multithreading | tsdownsample | **out-of-model** | single-threaded wasm |
| NaN-aware algorithm variants | tsdownsample | **out-of-model** | we error with the offending row number instead (deterministic paste-in tool, not an array pipeline); documented on page |
| Streaming unbounded inputs | TimescaleDB (SQL scale) | **out-of-model** | 2 MB input cap, stated on page |
| Chart rendering of the result | flot plugin context | **out-of-model here** | separate existing tool (`blocks/line-series-chart`) |
| Time-bucket aggregation (mean/median resample) | TimescaleDB `time_bucket` family | **out-of-model** | different tool family (aggregation, not point selection); documented in FAQ |

## UX control patterns observed → adopted

- One required data field + one dominant numeric knob → `data` textarea +
  `points` number field with placeholder.
- Fixed algorithm choice → `Param::enumv` with friendly `[input.labels]`
  (select, not text).
- Library-style preset examples (n_out=…) → `[[example]]` chips: LTTB 200→12,
  indices output, M4, timestamped CSV.
- Output-shape choice (values vs indices) → `output` enum.

## Design decisions

- All four algorithms SELECT existing points (never interpolate), so the tool
  emits the ORIGINAL rows/elements verbatim — CSV rows keep every column and
  exact formatting, header preserved; JSON keeps element values (object key
  order preserved via serde_json `preserve_order`).
- Input auto-detects CSV (comma/tab/semicolon, quote-aware) vs JSON (`[…]` of
  numbers, `[x,y]` pairs, or objects with x/y-ish keys).
- `header` boolean (default true) treats a NON-NUMERIC first CSV row as a
  header; a numeric first row is always data, so pasted number lists just work.
- Caps: input ≤ 2,000,000 bytes; `points` 2–100,000. Both stated on the page and
  boundary-tested.
