# xliff-to-json — competitor analysis (2026-08-23)

Scan run **before** implementing, per the create-next-tool recipe. Everything below is a
paraphrased observation of publicly documented behaviour; no competitor copy, branding or
trademarks are reproduced or reused.

## Scan

Two web searches (`XLIFF to JSON converter online tool translation units source target`,
`convert XLIFF xlf file to i18n JSON key value localization CLI npm`) plus fetches of the
reachable results. Five sources were read; one (`npmjs.com/package/@ni/xliff-to-json-converter`)
403s the fetcher, so its behaviour is taken from the search-result summary and its GitHub-side
docs only, and it is not counted as a fully-read source.

| # | Source | What it is | Notes |
|---|--------|-----------|-------|
| 1 | `locize/xliff` (npm `xliff`) | The de-facto JS XLIFF library every other JS tool wraps; `xliff2js` / `xliff12ToJs` / `xliff2ToJs` | The reference data model: `resources[namespace][key] = { source, target, note?, additionalAttributes? }` plus `sourceLanguage`/`targetLanguage`/`xliffVersion`; explicit 1.2 (`<trans-unit>` under `<body>`) **and** 2.x (`<unit>`/`<segment>`) support, normalized to one shape. Ships `targetOfjs()` / `sourceOfjs()` helpers that flatten to `{ key: value }`. `source`/`target` may be an **array** mixing text and inline-element objects. |
| 2 | `gpttranslator.co/free-tools/xliff-to-json` | Browser-local single-page converter | Flat `{ id: value }` output; maps each unit id to its target, **falling back to source when the target is missing**. Handles both `trans-unit` and `unit`. Controls: upload, paste textarea, load-sample button, convert, clear, copy, download. No version/nesting/attribute options documented. |
| 3 | `localizely.com/localization-file-converter/xliff-to-json/` | Vendor's free converter page | Single target flavour, described as key-value JSON. Drag-and-drop or browse. No options, no documented version support, no documented empty-target/placeholder/plural behaviour. Upsells the paid platform. |
| 4 | `better-i18n.com/en/tools/translation-file-converter/xliff-to-json/` | Browser-local converter, part of an i18n toolkit | Upload or paste, one convert button. Its own FAQ covers **nested key handling**, file-size limits, and download — i.e. nesting is a documented user question in this category. No exposed option matrix. |
| 5 | `github.com/Tzahi12345/xliff-to-json` | Node CLI for Angular XLIFF | Directory or single file in, sibling `.json` out. Documented hard limitation: **fails if the input contains `<group>` tags** — the user is told to delete them first. No documented key/nesting/state options. |

Also seen in search results but not read in depth (same shape, no additional documented options):
`@ni/xliff-to-json-converter` (a `--source`/`--destination` CLI), `xliff_to_json_converter`
(positional in/out script), `t2ym/xliff-conv` (bidirectional, tied to one framework's i18n
behaviour), `@leading-works/json2xliff` (reverse direction; notable only because it documents
auto-detecting XLIFF 2.0 and otherwise assuming 1.2 — the same detection we do).

## Table-stakes inventory

Every item ends up either in our descriptor or in an explicit "not built" list — nothing is
dropped silently.

### In-model — shipped in the descriptor

| Capability | Seen at | Our param | Our default |
|---|---|---|---|
| Parse XLIFF 1.2 `<trans-unit id>` with `<source>`/`<target>` | 1, 2, 5 | always on (version auto-detected) | — |
| Parse XLIFF 2.x `<unit id>` → `<segment>` → `<source>`/`<target>` | 1, 2 | always on (version auto-detected) | — |
| Flat `{ id: value }` output keyed by unit id | 1 (`targetOfjs`), 2, 3, 4 | `output` = `target` | — |
| Source **and** target retained per id | 1 | `output` = `pairs` | `pairs` (the backlog's "source/target pairs by id") |
| Source-only flat map | 1 (`sourceOfjs`) | `output` = `source` | — |
| Fall back to source text when the target is empty | 2 (its whole default behaviour) | `fallback_to_source` | `false` (opt-in — see decision 2) |
| Drop untranslated units | implied by 2's fallback framing; standard CAT-tool need | `include_empty_targets` | `true` |
| Nested key output for i18n bundles | 4 (FAQ topic), general i18next convention | `nested` + `separator` | `false` / `.` |
| Key by something other than id | 1 (`resname` lives in `additionalAttributes`) | `key` = `id` \| `resname` \| `source` | `id` |
| Translator notes | 1 (`note` field) | folded into `include_metadata` | `false` |
| Translation state / approved flag | 1 (`additionalAttributes`) | folded into `include_metadata` | `false` |
| Array-of-records output (order + duplicates preserved) | 1 (array-valued `source`/`target`) | `output` = `array` | — |
| Load-a-sample / preset | 2 (load-sample button), 4 | five `[[example]]` chips | — |
| Paste **or** upload | 2, 3, 4 | paste textarea (the page's field control); upload is a site-shell concern | — |

**Differentiator 1 — `<group>` does not break us.** Source 5 documents outright failure on
`<group>` and tells users to strip the tags by hand. Groups are legal and common in both
XLIFF 1.2 and 2.x (CAT tools emit them constantly), so we walk them transparently and, with
`include_metadata`, report each unit's enclosing group path.

**Differentiator 2 — inline placeholders survive.** Source 1 exposes them only as a raw mixed
array the caller must reassemble; sources 2–5 document nothing, and a naive text-only extraction
silently drops `<x/>`, `<ph>`, `<g>`, `<bpt>`/`<ept>`, `<pc>`, `<sc>`/`<ec>` — which is how an
Angular `{{name}}` interpolation quietly disappears from a bundle. Our `inline_tags` param
defaults to `placeholder`: each inline element becomes its `equiv-text` when the file supplies
one, else `{id}`. `strip` and `keep` are the other two honest choices.

**Differentiator 3 — multi-segment 2.x units are joined, not truncated.** An XLIFF 2.x `<unit>`
may hold several `<segment>` children that partition one string; concatenating them in document
order is the only correct reading. `<ignorable>` content is skipped.

### In-model but intentionally not built (listed, not dropped)

| Capability | Seen at | Why not |
|---|---|---|
| File upload control on the page | 2, 3, 4 | The page renders a paste textarea from the descriptor's string param; a file picker for a *text* param is a shared-generator concern, not a per-tool hack (workspace rule: no `cfg.slug ===` branches). Paste + the CLI's shell redirection cover the same ground today. |
| Download button for the JSON | 2, 4 | Already free: `format = "text"` pages get a Download link and a Copy button from the shared page shell. |
| Reverse direction (JSON → XLIFF) | 1, `@leading-works/json2xliff` | A different tool, not this backlog row. Worth its own block later. |
| Batch / whole-directory conversion | 5, `@ni/...` | One document in, one document out. The CLI form on the page shells out fine in a loop. |
| Filter by translation state (`translated`, `final`, `needs-review-translation`) | 1 (state is in `additionalAttributes`) | `include_empty_targets` already covers the "only give me finished strings" ask for the overwhelming majority; a second filter enum would be the 10th form field for a case that varies per CAT tool. State is still *exposed* via `include_metadata`, so a downstream `jq`/JS filter is one line. |
| Emit `sourceLanguage` / `targetLanguage` / `xliffVersion` as a header object | 1 | Would stop the output from being a drop-in i18n bundle — the single most common use of this conversion. The languages are visible in the input's own `<file>` element. |
| Plural / ICU message reconstruction | none document it | XLIFF stores plural variants as separate units with their own ids; reassembling them into ICU syntax is a per-framework convention, not an XLIFF operation, and would silently invent keys. |

### Out-of-model (cannot be done by this block at all)

| Capability | Seen at | Why |
|---|---|---|
| Actually translating the source text | 3 (vendor platform), `centus.com` guide | Machine translation needs a neural model; gizza is pure-Rust + ffmpeg on a wasmi runtime with no ML loader. Same class as the skiplisted `subtitle-translator` / `markdown-translate`. This tool only *extracts* what the file already contains. |
| Server-side translation memory, glossaries, team accounts, Git sync, CDN delivery | 3, 4 | Backend/account features; every gizza tool is browser-local, no account, no server. |
| Reading `.sdlxliff` / `.mqxliff` vendor dialects' proprietary extensions | implied by 3, 4's CAT-tool framing | The XLIFF core of those files parses fine here (they *are* XLIFF); vendor-private namespaced extensions are not interpreted. Stated on the page as a limit rather than silently half-supported. |

## UX control patterns adopted

- `<select>` (`Param::enumv`) for all three fixed-choice params (`output`, `key`, `inline_tags`)
  with `[input.labels]` giving plain-English labels — competitors expose zero options, so the
  labels have to teach the option, not just name it.
- Checkboxes for the three booleans, with defaults chosen so an untouched run is the lossless
  reading: every unit, both source and target, placeholders preserved, no metadata noise.
- `[[example]]` preset chips instead of the "load a sample" button sources 2 and 4 ship — five
  of them: a 1.2 file, a 2.x file, an Angular file with interpolation placeholders, an
  i18next-style nested bundle, and a translated-only target map.
- `multiline = true` on the XLIFF input so a pasted file keeps its newlines.
- Errors name the expected element (`<trans-unit>` / `<unit>`) and, for malformed XML, the byte
  position quick-xml reports — competitors surface a bare failure or, in source 5's case, a
  README instruction to edit the input by hand.

## Decisions recorded

1. **Default `output = pairs`**, not the flat target map most competitors emit. The backlog row
   asks for "source/target pairs by id" and that is the lossless default; `target` is one click
   away and is what an i18n bundle wants.
2. **`fallback_to_source` defaults to `false`.** Source 2 falls back silently, which is friendly
   for a demo and dangerous for a bundle: untranslated English then ships as if it were the
   translation, invisibly. Opt-in, and the page says why.
3. **`key = resname` falls back to `id`** when a unit has no `resname`, rather than dropping the
   unit — CAT tools populate `resname` inconsistently within one file.
4. **Duplicate keys: last wins in object shapes, and `output = array` preserves everything.**
   A file with several `<file>` elements can legally repeat an id. Object shapes cannot represent
   that, so the array shape is the documented escape hatch and `include_metadata` carries the
   originating file so duplicates are traceable.
5. **Empty-vs-missing `<target>` are treated the same** (both "no translation"), because CAT
   tools disagree on whether to emit `<target></target>` or omit the element, and a user filtering
   untranslated strings means the same thing in both cases.
6. **Output is bare JSON, no summary wrapper**, so the result is copy-paste-usable as a bundle.
   Unit counts belong in the page copy, not in the payload.
