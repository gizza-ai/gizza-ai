# flatten-json — competitor scan + design decisions (2026-08-14)

Scan run **before** implementation, per `/create-next-tool` step 4. One web search
("flatten JSON online tool nested to dot notation key paths unflatten"), then the top 3 real
competitor tools were skimmed. Everything below is **paraphrased**; no competitor copy, brand
names in page text, or trademarks were reused. Findings are recorded as capability/UX facts only.

## Competitors skimmed

| # | Tool (URL) | What it is |
|---|---|---|
| 1 | coderstool.com/flatten-json | Browser JSON flattener/unflattener with presets and profile saving |
| 2 | dataformatterpro.com/json-flattener | Client-side flattener, Monaco editor, live "flatten as you type" |
| 3 | jsonviewertool.com/json-flatten | Flatten/unflatten pair with delimiter + pretty controls |

## Table-stakes observed (params / defaults / examples / UX)

**Parameters + defaults**

- **Direction**: flatten ⇄ unflatten in one tool (all 3). Two of the three also offer an
  auto-detect mode that guesses the direction from the input shape.
- **Separator / delimiter**: dot is the default everywhere; underscore and slash are the
  other offered choices, and one tool allows an arbitrary custom character.
- **Array index notation**: bracket style (`items[0]`) vs delimiter style (`items.0`) —
  offered as an explicit toggle by #2, implied by #1/#3's round-trip notes.
- **Depth**: all three advertise unlimited recursion; none offers a depth cap.
- **Empty objects/arrays**: #2 states empties collapse to an empty value while `null` is
  preserved — i.e. it is a documented behaviour, not a control.
- **Pretty output + indent**: #3 exposes both, pretty on by default; #1 exposes
  minified / escaped / pretty output modes.
- **Key decoration**: #1 offers a key prefix/suffix plus named presets ("SQL-like column
  keys", "ENV-style keys", "log-friendly keys", "compare-ready") — i.e. case/joiner recipes.
- **Size limits**: #2 states ~10 MB, browser-memory bound.

**Examples / copy**

- All three lead with the same one-line worked example shape: a two-level object becoming a
  single `parent.child` key. #3 shows a 3-key nested user object → 3 dotted paths.
- FAQ topics that recur: what flattening is, how arrays are handled, choosing a delimiter,
  round-tripping back to nested, key collisions, invalid-JSON troubleshooting, privacy
  (browser-local), and size limits.

**UX controls**

- Load-sample / example button (all 3), copy result, download result, clear/reset panels.
- Live re-run as you type (#2), fullscreen (#3), shareable URL state (#1).
- Warnings on delimiter conflicts and ambiguous numeric keys that break round-tripping (#1).

## In-model vs out-of-model

**In-model (built here)**

| Table-stake | How it lands in this tool |
|---|---|
| Flatten + unflatten in one tool | `direction = flatten \| unflatten \| auto` (default `flatten`) |
| Auto-detect direction | `direction = auto` — unflattens only when the document is a one-level object whose keys carry a separator/bracket index; otherwise flattens |
| Delimiter choice incl. custom | `separator` free-text, default `.` (dot/underscore/slash/any 1–8 chars) |
| Bracket vs delimiter array indices | `array_notation = bracket \| separator` (default `bracket`) |
| Empty object/array behaviour | `preserve_empty` (default true) — keeps `{}` / `[]` as leaf entries so round-trips survive; documented rather than silent |
| Pretty + indent | `pretty` (default true) + `indent` (default 2, 0–8) |
| Key-case recipes (SQL/ENV/log presets) | `key_case = preserve \| upper \| lower` plus example chips that combine it with `separator` and `output` |
| Size limit stated on the page | 5 MB input cap, 100-level depth cap, 200 000 key cap — all in the page copy and in the error text |
| Load-sample, copy, reset, URL state | Provided by the shared page generator (`[[example]]` chips, Copy result, Reset, `?param=` deep links) |

**Beyond table stakes (differentiators added)**

- `output = json | pairs | csv | paths` — the flat result as JSON, `key=value` lines, a
  two-column `key,value` CSV (the spreadsheet path the backlog row asks for), or just the
  path list. No scanned competitor emitted CSV or a bare path list.
- `max_depth` — flatten only the first N levels and leave deeper values as nested JSON
  (competitors are all-or-nothing).
- `flatten_arrays = false` — keep arrays intact as JSON values while still flattening
  objects (the "safe" mode of the `flat` npm library; none of the three exposed it).
- Round-trip conflict detection on unflatten with a message naming both colliding paths,
  instead of silently overwriting.

**Out-of-model (considered, not built)**

- Server-side batch processing of many documents, account/profile saving of option sets,
  and shareable saved profiles (#1) — need a backend and an account; gizza is local-only.
- Monaco/VS-Code-grade editor panes and fullscreen editing (#2, #3) — the shared page
  generator owns the input chrome; a per-tool editor would be a tool-specific UI fork.
- Live "flatten as you type" on every keystroke (#2) — the shared driver runs on change/blur;
  changing that is a platform-wide decision, not a per-tool one.
- File upload of `.json`/`.log` files (#2) — this block's input is a pasted/param string;
  file-input is reserved for the media/document block shapes.

## Decisions

1. **Default `direction = flatten`, not `auto`.** Auto-detection is genuinely ambiguous for a
   one-level object (`{"a.b": 1}` is both a valid flat map and a valid nested-free document), so
   the predictable direction is the default and `auto` is opt-in and documented.
2. **Default `array_notation = bracket`.** Bracket indices are unambiguous on the way back:
   `a[0]` is always an array element, while a bare numeric segment `a.0` could be an object key
   named `"0"`. With `separator` notation the documented rule is that an all-digit segment
   rebuilds an array; with `bracket` it rebuilds an object key. Both directions round-trip.
3. **Document order is preserved** (`serde_json` `preserve_order`), not sorted — sorting is
   already a separate block, and preserving order keeps the flatten/unflatten round-trip
   byte-identical.
4. **`output` other than `json` is flatten-only** and errors with an explicit message when
   combined with an unflatten run, rather than silently ignoring the setting.
5. **`key_case` applies to the generated path only** (flatten direction). It is a lossy option
   by nature, so the page copy says so and the FAQ covers the round-trip caveat.
