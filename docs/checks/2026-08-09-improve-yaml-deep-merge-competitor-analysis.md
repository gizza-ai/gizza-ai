# yaml-deep-merge — competitor analysis (2026-08-09)

Scan run **before** implementing, per `/create-next-tool` step 4. One web search
("online YAML deep merge tool combine multiple YAML files Helm values override"),
then the top three real browser tools were skimmed. Everything below is a
**paraphrase** of observed behaviour — no competitor copy, naming, or branding is
reused anywhere in this block.

## Competitors skimmed

| # | Tool | Shape |
|---|------|-------|
| 1 | ToolsBox "YAML Merger" (toolsbox.io/code/yaml-merger) | Two textareas (base + incoming), single merged output, presets |
| 2 | Paji Dev Workshop "YAML Merger" (dev-workshop.marco79423.net/en/yaml-merger) | Up to 5 documents, merged left-to-right |
| 3 | Elysia Tools "YAML File Merger" (elysiatools.com/en/tools/yaml-merger) | Up to 5 uploaded files, 10 MB each, select-driven strategy matrix |

A fourth candidate, the `helm-merge-values` Helm plugin, was opened for merge
semantics but its README documents no strategy flags at all (it just defers to
Helm's own `-f` layering), so it contributed only the baseline Helm rule:
**values files are merged left-to-right, the rightmost file wins**, and a key set
to `null` in a later file **removes** that key from the result (Helm's documented
"deleting a default key" behaviour).

## Table-stakes matrix

| Capability | 1 | 2 | 3 | Decision |
|---|---|---|---|---|
| Deep (recursive) object merge | ✅ | ✅ | ✅ | **in-model** — the default and the point of the tool |
| Shallow / top-level-only merge | ✅ | — | ✅ | **in-model** → `object_merge = deep\|shallow` |
| Array replace | ✅ | ✅ | ✅ | **in-model** → `array_merge = replace` (default, matches Helm) |
| Array concatenate | ✅ | ✅ | ✅ | **in-model** → `array_merge = append` |
| Array concat + de-duplicate | — | — | ✅ | **in-model** → `array_merge = unique` |
| Array merge by key (object lists) | — | — | ✅ | **in-model** → `array_merge = by_key` + `array_key` (default `name`, the Kubernetes list-key convention) |
| Precedence: later document wins | ✅ | ✅ | ✅ | **in-model** → `precedence = last` (default) |
| Precedence: keep the first value | ✅ | ✅ | ✅ | **in-model** → `precedence = first` |
| Conflict → hard error | — | — | ✅ | **in-model** → `precedence = error` (reports the conflicting path) |
| More than two documents | — | ✅ (5) | ✅ (5) | **in-model** — unlimited-ish: `---`-separated, capped at 20 docs / 1 MiB |
| Output indent choice | ✅ (2/4) | — | — | **in-model** → `indent` 1–8 (hand-rolled emitter; free choice, not just 2/4) |
| Sort keys vs keep source order | ✅ | — | — | **in-model** → `sort_keys` (default off = source order preserved) |
| Preset / example buttons | ✅ | — | — | **in-model** → three `[[example]]` chips on the page |
| Copy result / download | ✅ | ✅ | ✅ | already platform-provided (Copy + Download on every `format = "text"` page) |
| Runs locally, nothing uploaded | ✅ | ✅ | ? | already true — wasm in the browser |
| `null` deletes a key (Helm) | — | — | — | **in-model, our differentiator** → `null_deletes` (default **on**, matching Helm) |
| Comment preservation | — | — | ✅ ("if possible") | **out-of-model, not built** — see below |
| File upload (multi-file, 10 MB each) | — | — | ✅ | **out-of-model** for this page shape — the page takes one pasted textarea; the CLI reads from argv/stdin redirection |
| Diff view of what each layer changed | — | — | — | **considered, rejected** — a second output pane is a different tool (`yaml-diff`-shaped), not a merge option |

### Out-of-model / not built (recorded, not silently dropped)

- **Comment preservation.** The merge runs through a YAML value model
  (`serde_yml::Value`); comments and anchors are not part of that model, so they
  are dropped and the output is re-emitted canonically. Stated as a limit on the
  page and in the FAQ rather than half-implemented. (`blocks/yaml-path-query` uses
  `yaml-rust2` markers for comment-preserving *in-place edits* of a single
  document; that technique does not extend to merging N documents, where the
  merged tree has no single source span.)
- **Multi-file upload UI.** Elysia's five file pickers need a multi-file page
  control that does not exist in the generator, and the merge semantics are
  identical to pasting `---`-separated documents. Rejected as UI surface without
  capability gain.
- **10 MB per file.** Our cap is 1 MiB of total input / 20 documents — sized for
  a browser wasm sandbox and stated on the page.

## UX patterns adopted

- **Preset chips** — competitor 1's presets map onto the generator's declarative
  `[[example]]` blocks: a base+override values layering, an array-strategy
  comparison, and a Helm-style `null` deletion.
- **A single paste area with `---` separators** rather than N fixed textareas —
  the YAML-native way to express "several documents", and it scales past the
  two/five-document ceilings all three competitors impose.
- **Every fixed choice is a `<select>`** (`Param::enumv` → generator select), with
  `[input.labels]` giving each strategy a plain-English label.
- **Errors name the path** — `precedence = error` and every parse failure report
  which document and which key path failed, instead of a bare "invalid YAML".
