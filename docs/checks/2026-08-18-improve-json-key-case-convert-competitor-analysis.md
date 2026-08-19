# json-key-case-convert — competitor analysis (2026-08-18)

Scan run BEFORE implementing the tool, per `/create-next-tool` step 4. Everything below is a
paraphrase of observed behavior/features — no competitor copy, branding, or trademarks are
reproduced, and none of their wording was reused in our page.

Search: "JSON key case converter online camelCase snake_case convert all keys" (+ two refinement
searches for kebab/Pascal/SCREAMING_SNAKE variants and for recursive browser-side converters).

## Competitors reviewed

| # | Tool | Shape |
|---|------|-------|
| 1 | JSON Key Converter (mio0000.github.io/CaseConverter/json.html) | Client-side single-page web tool |
| 2 | Better Converter — JSON Camel/Snake/Pascal Case property-name converters | Server-backed web tool family, one page per target case |
| 3 | ToolGram — JSON Key CamelCasifier | Client-side single-page web tool (camelCase only) |
| 4 (library reference) | js-convert-case (npm) | JS library with `camelKeys`/`snakeKeys`/`pascalKeys`/`kebabKeys` + options |

## What each offers

**1. JSON Key Converter.** Five target cases: camelCase, PascalCase, snake_case, kebab-case,
SCREAMING_SNAKE_CASE. One button per case (no dropdown), plus Clear. Recurses through nested
objects *and* objects inside arrays; values of every type are left untouched. Paste-in textarea →
output pane with copy-to-clipboard. No indent/minify control, no key-exclusion, no upload, no
stated size limit or documented edge-case behavior (acronyms, `_id`, key collisions).

**2. Better Converter (JSON camel / snake / Pascal case).** One page per target case, so the target
is chosen by navigating rather than by a control. Input textarea, "load from URL", file upload
capped at 10 MB, and a Convert button. Pages state the source cases they accept but document
neither nested/array handling, nor indent/minify, nor worked examples or limits. Server round-trip
(the upload/Load-URL model implies data leaves the browser).

**3. ToolGram JSON Key CamelCasifier.** camelCase only. Textarea + Convert + Clear + copy-JSON.
States explicitly that it recurses through nested objects and arrays. Documents the basic rule
(underscore removed, following letter capitalized, first letter lowercase). Shows one small
worked example and a 2-question FAQ. Nothing about leading underscores, acronyms, output
formatting, or limits.

**4. js-convert-case (library, for option vocabulary).** Key-conversion functions take
`recursive` (default off), `recursiveInArray` (default off), and a type-preservation list.
Confirms that "recurse or not" is a real, expected knob even though the web tools hard-code it on.

## Table stakes → our design

| Capability | Competitors | In/out of model | Our decision |
|---|---|---|---|
| camelCase / PascalCase / snake_case / kebab-case / SCREAMING_SNAKE | 1 (all five); 2, 3 (subset) | in-model | `target_case` as a single `Param::enumv` with all five values — one control instead of one page/button per case |
| Recursive through nested objects **and** arrays | 1, 3 (always on); 4 (opt-in) | in-model | Recursion on by default via `recurse`, with an explicit off switch for shallow, top-level-only renames |
| Values never modified | 1, 3 | in-model | Same invariant; only keys are rewritten (stated on the page) |
| Copy result / clear | all | in-model | Provided by the shared page runtime (Copy result + Reset) |
| Indent / minify output | none | in-model | `indent` 0–8 (0 = minify) — a gap all three leave open |
| Key exclusion list | none | in-model | `preserve_keys`: exact key names to leave alone (`Content-Type`, `_id`, data-keyed maps) |
| Leading `_` / `$` / `@` sigils (`_id`, `$schema`, `__typename`) | undocumented everywhere | in-model | `preserve_prefix` (default on) keeps the sigil and converts the rest |
| Acronym-aware splitting (`userID` → `user_id`, `HTTPResponse` → `httpResponse`) | undocumented everywhere | in-model | Implemented in the splitter, documented on the page with worked examples |
| Key-collision behavior (`user_name` + `userName` → same key) | silently lossy | in-model | Hard error naming both keys and the JSON path, instead of dropping data |
| Stated limits (size, depth) | only #2 (10 MB upload) | in-model | 5 MB input cap and 100-level depth cap, both stated on the page and in error text |
| Worked examples / FAQ | #3 only (thin) | in-model | ≥1 input→output example plus 5 `<details>` FAQ entries, incl. acronyms, `_id`, collisions |
| Preset chips | none | in-model | Three `[[example]]` chips (API response → camelCase, → snake_case, kebab minified) |
| File upload / load-from-URL | #2 | out-of-model | Not built: gizza pages are paste-in, browser-local, no server round-trip |
| One page per target case (SEO surface) | #2 | out-of-model | Not built: a single page with a dropdown is the gizza tool shape |
| Converting *values* as well as keys | none | rejected | Out of scope for this tool; would break data |

## UX control patterns adopted

- Single `<select>` for the target case (derived from `Param::enumv`) instead of one button per case.
- Checkboxes for the two boolean knobs; the recursion default (on) matches every web competitor.
- Multiline textarea for the JSON input with a real JSON placeholder.
- Preset "Try:" chips for the three most common conversions — no competitor ships presets.
- Result panel with the shared Copy/Reset affordances, so parity with the copy button on all three.

## Out-of-model / not built

- Server-side file upload and load-from-URL (competitor #2) — no backend, no upload by design.
- Per-target-case landing pages — one tool, one page, target chosen by a control.
- Language-specific codegen (Swift/Java model mapping, seen in adjacent blog content) — a
  different tool family, not a key renamer.
