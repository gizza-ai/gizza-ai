# geojson-validate — competitor analysis (2026-08-15)

Snapshot taken while building `blocks/geojson-validate`. Competitors were studied for
**ideas, checks and UX patterns only** — no copy, branding or assets were reproduced. All
descriptions below are paraphrased.

## Duplicate check (done first)

The backlog row overlaps three existing blocks, so each was read before scaffolding:

| Block | What it does | Overlap verdict |
| --- | --- | --- |
| `geojson-format` | Pretty-print/minify/round/re-key/rewind/bbox. Has a `validate` flag that gates formatting. | **Not a duplicate.** Its validation is a fail-fast precondition — it returns the *first* problem as an `Err` and its product is formatted GeoJSON. It also *fixes* winding rather than reporting it. A report-all validator with a verdict, severities, rule ids and counts is a different deliverable, matching the repo's existing formatter/validator pairs (`json-beautify` ↔ `json-schema-batch-validate`, `sql-linter`, `csv-structure-validator`). |
| `geojson-merge`, `geojson-to-csv`, `geojson-to-svg` | Combine / flatten / render. | No validation reporting surface. |
| `json-schema-batch-validate` | Validates JSON against a supplied JSON Schema. | Generic; carries no GeoJSON semantics (ring closure, winding, WGS 84 ranges). |

## Competitors reviewed

1. **A widely-used JS GeoJSON linter library** — the de-facto reference implementation. Emits a
   flat list of `{message, line, level}`, keeps going after the first problem, and exposes three
   switches: allow duplicate object members, warn on coordinate precision beyond six decimals, and
   ignore the right-hand rule. Line numbers come from a bespoke JSON parser.
2. **A Python validate-and-repair library** — splits findings into *invalid by spec* (unclosed
   rings, fewer than three unique nodes, exterior not counterclockwise, interior not clockwise) and
   *valid but problematic* (holes, self-intersection, ring intersection, duplicate nodes, excess
   precision, excess vertices, 3D coordinates, out-of-range coordinates, antimeridian crossing).
   Reports feature indices plus JSON paths and geometry-type counts, and can auto-fix a subset.
3. **A catalogue repository of GeoJSON geometry problems** — a 22-item taxonomy with example
   files, effectively the union of what the ecosystem checks. Adds: legacy `crs` member, zero-length
   LineString, wrong bbox coordinate order, single-element multi-geometries, Feature with null
   geometry, bare geometry with no wrapper, nested GeometryCollections, GeometryCollection holding
   a single type.
4. **A browser validator + repair site** — drag-and-drop upload, map preview, validation *and*
   automatic repair (winding order, duplicate coordinates).
5. **A free browser GeoJSON developer-tools site** — RFC 7946 validation with a map preview and
   raw JSON output, copy buttons, no signup, everything client-side.

## Table stakes → what shipped

| Capability | Competitors | This tool |
| --- | --- | --- |
| Report all issues, not just the first | 1, 2, 3 | ✅ full walk, nothing short-circuits except an uninterpretable subtree |
| Machine-readable location per issue | 1 (line), 2 (JSON path) | ✅ JSON path (`features[3].geometry.coordinates[0][2]`) + a stable kebab-case rule id |
| Structural/type checks | all | ✅ `type`, `features`, `properties`, `geometry`, `coordinates`, `geometries`, Feature `id` type |
| Coordinate range + non-numeric | 1, 2, 3 | ✅ lon −180…180, lat −90…90, finite numbers, position arity 2–3, with a swapped-pair/projected-CRS hint |
| Ring closure, minimum ring size | 2, 3 | ✅ ≥4 positions, closed, ≥3 distinct nodes |
| Right-hand-rule winding | 1, 2, 3, 4, 5 | ✅ exterior CCW / holes CW, **and** a `strict_winding` toggle (error ↔ warning) mirroring competitor 1's ignore switch |
| Error vs warning severity split | 2 | ✅ errors set the verdict, warnings never do |
| Excess coordinate precision | 1, 2, 3 | ✅ `max_precision`, default 6, −1 to disable |
| Legacy `crs`, bbox order, single-element multis, nested GC, null/bare geometry, zero-length line, duplicate nodes, antimeridian | 2, 3 | ✅ the `warn_problematic` family, switchable off for a spec-only check |
| bbox shape validation | 3 | ✅ 4-or-6 length, finite, in range, south ≤ north; `allow_bbox=false` flags the member itself |
| Counts / summary | 2 | ✅ features, geometry types, positions, rings (exterior/interior), bbox members, coordinate bounds |
| Machine-readable output | 1, 2 | ✅ `output=json` → `{valid, error_count, warning_count, errors[], warnings[], summary}` |
| Runs locally, no upload, no signup | 4, 5 | ✅ wasm in the browser; also chat + CLI |

## Considered, not built

- **Self-intersection / ring-intersection** (competitors 2, 3). Needs a real geometry engine
  (segment-intersection sweep, robust predicates) rather than a structural walk. Out of scope for
  this block's model; stated as a known limit on the page rather than silently missing.
- **Duplicate object members** (competitor 1). The JSON parser collapses repeated keys before the
  validator sees the document; catching it needs a bespoke tokenizer. Same reason line/column
  numbers are only available for the top-level parse error. Documented as a limit.
- **Line/column numbers per issue** (competitor 1). Same bespoke-parser dependency; JSON paths were
  chosen instead because they survive reformatting and address the *data*, not the byte offsets.
- **Excess-vertices threshold** (competitor 2) — an arbitrary per-consumer limit, not a property of
  the data; rejected as schema bloat when the summary already reports position counts.
- **3D-coordinate warning** (competitors 2, 3) — altitude is fully valid RFC 7946 and extremely
  common; warning on it would be noise. Positions are still arity-checked (2 or 3).
- **Automatic repair** (competitors 2, 4). Deliberately rejected: this block reports only, so the
  report can be trusted as a description of what you actually have. Rewinding, precision rounding
  and bbox recomputation already exist in `blocks/geojson-format`; the page points there.
- **Map preview of the offending geometry** (competitors 4, 5) — needs a tile provider (a network
  dependency and a third-party asset), which the browser-local, no-account model excludes.
- **File upload / drag-and-drop and multi-file batches** (competitor 4) — the page takes one pasted
  document; line-delimited GeoJSON must be checked a line at a time. Noted on the page.

## Notes

- `strict_winding` defaults to **on** even though RFC 7946 words the right-hand rule as SHOULD,
  because reversed winding is the single most consequential silent bug in the format (renderers
  fill the complement of the shape). The toggle exists for older data, and the FAQ explains the
  SHOULD/MUST distinction.
- `max_issues` (default 50, cap 1000) truncates the *listing* only; the verdict counts always
  reflect the full walk, and the JSON report carries a `truncated` flag so scripts can tell.
