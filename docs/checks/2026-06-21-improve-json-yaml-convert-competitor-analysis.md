# json-yaml-convert — competitor analysis & differentiation

**Tool:** `gizza-ai/json-yaml-convert` — convert config data between JSON, YAML,
and TOML.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| Online converters (json2yaml, transform.tools) | Web | Common, but most **upload your data**; many only do JSON↔YAML (no TOML), and TOML support is rare. |
| `yq` (mikefarah) | CLI | Excellent and supports JSON/YAML/TOML, but a native install and flag syntax. |
| `python -c` with `json`/`yaml`/`toml` libs | DIY | Need a runtime + the right libs installed. |
| gizza's own `json-yaml-converter` | tool | JSON↔YAML only — no TOML. |

## How gizza's tool is better / different

1. **Adds TOML** — the differentiator vs the existing `json-yaml-converter` and
   most web converters. All **six directions** (JSON↔YAML, JSON↔TOML, YAML↔TOML).
2. **Local — config never uploaded.** Runs in WASM (chat SW + CLI + page).
3. **Explicit from/to.** No guessing/auto-detect ambiguity — you choose source
   and target, so a YAML file that looks like JSON is handled correctly.
4. **Pretty or compact** JSON/TOML output.
5. **Honest about TOML's limits.** TOML needs a table root and has no `null`;
   those cases return a clear error rather than silent corruption.

## Verification

Seven core unit tests cover all six conversion directions plus the
top-level-array→TOML rejection and error cases. **End-to-end CLI**: JSON→TOML
produced `title = "x"` + `[server]` `port = 8080`; YAML→JSON produced
`{"count":3,"name":"gizza"}`. Page Playwright covers JSON→TOML and YAML→JSON.

## Relationship to json-yaml-converter

`json-yaml-converter` is the focused JSON↔YAML tool with auto-direction
detection. `json-yaml-convert` is the **three-format** converter (adds TOML) with
explicit from/to. Both are kept: the former for the quick auto JSON/YAML case,
this one when TOML is involved.

## Scope / honest limitations

- TOML output requires an object root and no `null` (reported as an error).
- Comments are not preserved (data round-trip, not document round-trip).

## Possible future enhancements

- Add CSV/INI to the format set.
- Auto-detect the source format.
- Preserve comments where the format allows.
