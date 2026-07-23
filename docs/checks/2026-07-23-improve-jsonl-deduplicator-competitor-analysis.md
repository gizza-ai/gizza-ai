# jsonl-deduplicator — competitor analysis (2026-07-23)

Built new via the create-next-tool loop; competitor scan run **before** implementing so the
descriptor shipped the table-stakes from the start. All copy below is **paraphrased** — no
competitor copy, branding, or trademarks reproduced.

## Landscape

There is **no** tool branded specifically as a "JSONL/NDJSON deduplicator". The online space is
dominated by JSON-*array* dedupers that happen to tolerate line-delimited input; true NDJSON dedup
is owned by CLI recipes (`jq`, Miller/`mlr`) that demand a terminal. A browser-local, no-account,
**JSONL-first** deduper fills the gap between both camps — that is this tool's positioning.

## Top 5 competitors (paraphrased profiles)

| tool | dedup modes | keep | nested keys | invalid-JSON handling | notes |
| ---- | ----------- | ---- | ----------- | --------------------- | ----- |
| Toolsvana JSON Duplicate Remover | whole-object, by-key | first | dot paths | inline validity check | drag-drop, demo data, dupe-count stats |
| JSONUtils JSON Duplicates Finder | whole-object (shallow/deep), by-key | first/last, remove-all, highlight | keys | — | analytics panel, dupes-only export |
| JSONBeautify Duplicate Array Finder | strict deep-equal (canonical hash), by-key, custom-JS | first/last/merge | single key | — | explicit JSONL support, grouped report, ~100k/s claim |
| Vinish.dev Remove Duplicate JSON | whole-record deep-equal, by-key | first | dot paths (`user.id`) | — | file upload or paste, single-purpose |
| CLI: `jq` / Miller (`mlr`) | whole-record (`uniq -a`, `jq unique`), by-key (`uniq -g`, `unique_by`) | first-seen (mlr order-preserving; `jq unique_by` **sorts**) | jq expressions | — | streaming, huge files; needs a terminal |

Sources: toolsvana.com/tool/json-duplicate-remover · jsonutils.org/json-duplicates-finder.html ·
jsonbeautify.org/en/json-duplicate-array-finder · vinish.dev/duplicate-json-remover ·
miller.readthedocs.io · jqlang.org.

## Table-stakes vs our tool (all in-model — pure wasm, browser-local, no server/account)

| capability | competitor precedent | shipped here |
| ---------- | -------------------- | ------------ |
| Whole-line / whole-record equality | all | ✅ `keys` empty = whole-line equality |
| By-key dedup (one or many fields) | all | ✅ `keys` = comma list |
| Nested key support (dot paths) | toolsvana, vinish, jsonutils | ✅ `user.id`; numeric segment indexes arrays (`tags.0`) |
| Keep first vs last | jsonutils, jsonbeautify | ✅ `keep` = first/last, order-preserving |
| Order preservation | Miller (we beat `jq unique_by`, which sorts) | ✅ first/last kept in stream order |
| Native JSONL/NDJSON I/O | jsonbeautify only | ✅ line-in, line-out (our niche) |
| Stats / counts | all serious ones | ✅ chat + CLI return total/kept/removed record counts |
| Field-order-insensitive match | jsonbeautify (canonical hash) | ✅ by-key mode canonicalizes values (serde sorts object keys) |
| Invalid-JSON handling | toolsvana (inline validity) | ✅ `on_invalid` = error (names the line) / skip / keep |
| Case-insensitive match | — | ✅ `ignore_case` (handy for emails/ids) |

## Out-of-model / considered, not built

- **Canonical whole-record deep-equality as a distinct mode** — the description pins the two modes
  to *whole-line equality* and *chosen key fields*; field-order-insensitive full-record matching is
  already reachable by naming the key fields, so a third "canonical whole-object" mode was declined
  to avoid schema bloat. (rejected on judgment, in-model.)
- **Grouped-duplicate report / dupes-only export / merge-matched** (jsonutils, jsonbeautify) —
  in-model but out of scope for a paste-ready deduper; the page stays a clean text-in/text-out tool.
- **Streaming multi-GB files beyond browser memory** (Miller's strength) — out-of-model without a
  backend; we process in a single in-memory pass (fast for typical exports, no server upload).
- **Saved presets / history** — needs an account/backend; out-of-model.
- **File drag-and-drop upload** — the shared page runtime is paste-oriented for pure text tools;
  paste covers the workflow.

## UX shipped

`keep` and `on_invalid` render as `<select>`s; `ignore_case` a checkbox; `keys`/`data` text/textarea.
Three one-click `[[example]]` preset chips (whole-line, by-key keep-last, skip-invalid) double as
worked examples. Privacy angle ("runs in your browser, nothing uploaded") stated on the page, backed
by pure wasm.
