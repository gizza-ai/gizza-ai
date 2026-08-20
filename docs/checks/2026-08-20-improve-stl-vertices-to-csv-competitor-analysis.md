# stl-vertices-to-csv — competitor analysis (2026-08-20)

Scan run **before** implementing, per the create-next-tool recipe. Everything below is a
paraphrased summary of what each tool advertises; no competitor copy, branding or trademark text
is reused anywhere in the block, its page, or its docs.

## Duplicate check (done first)

`ls blocks/ | grep -iE 'stl|mesh|vert'` → `stl-inspector`, `stl-repair`, `mesh-convert`,
`obj-vertices-to-csv`. Greping all four for `csv` returns **nothing** outside this new block:

- `blocks/stl-inspector` — read-only mesh *metrics* (counts, bbox, area, volume, watertight/manifold,
  edge topology). Outputs `report` or `json`; no per-triangle table.
- `blocks/stl-repair` — repairs a mesh and re-emits it. Outputs `report`, `stl` or `json`.
- `blocks/mesh-convert` — converts between Wavefront OBJ and STL (`to = stl|obj`). Mesh in, mesh out.
- `blocks/obj-vertices-to-csv` — the OBJ-side sibling. Reads `v` lines out of OBJ **text** only; it
  explicitly rejects STL with a "this looks like ASCII STL" error, has no binary/base64/hex path, and
  has no triangle/corner or facet-normal concept (OBJ vertices are a shared pool referenced by faces;
  STL stores three explicit corners per facet).

None of them emits a triangle-vertex CSV, so this is a new tool, not a semantic duplicate.

## Competitors reviewed

| # | Tool | Shape |
|---|------|-------|
| 1 | `bji219/STL_to_CSV` (GitHub Python script) | Scrapes `vertex` lines out of ASCII STL, writes one `x,y,z` row each. No options at all. |
| 2 | imagetostl.com — STL → XYZ point cloud | File upload (up to 500 MB, batch up to 200 files), fixed output: positions only, faces/materials/colours discarded. |
| 3 | Aspose 3D — STL point-cloud generator | Upload + interactive 3D preview; Y-up/Z-up choice, translate/scale/rotate; exports PLY, OBJ, GLB, glTF, XYZ, PCD. |
| 4 | Cimatron — STL to CSV converter (CAD-embedded) | Writes a coordinate CSV out of STL geometry; the one documented knob is a "dilute" factor that thins how many points reach the file. |
| — | Generic doc-converter sites (FileProInfo, Conholdate, GroupDocs, jedok) | Upload-and-download STL→CSV with no exposed geometry options; not informative for parameter design. |

## Table stakes → where each one landed

**In-model — shipped in the descriptor:**

| Capability | Seen in | Param |
|---|---|---|
| ASCII STL parsing, one row per corner | 1, 2, 4 | `stl`, `rows = vertex` (default) |
| Binary STL parsing | 2, 3, 4 | `input_format = auto\|ascii\|base64\|hex` (binary pasted as base64/hex, same convention as `stl-inspector`) |
| Plain positions-only output | 1, 2 | `columns = xyz` (default) |
| Y-up / Z-up handling | 3 | `up_axis = keep\|z-to-y\|y-to-z` |
| Uniform scaling / unit change | 3 | `scale` |
| Thin out the emitted points ("dilute") | 4 | `every_nth` |
| Drop duplicated positions (STL repeats every shared corner 2–6×) | implied by all point-cloud outputs | `dedupe = none\|adjacent\|all` |
| Fixed decimal places | implied (CSV/CAD import) | `precision` |
| Spreadsheet-friendly delimiter + header toggle | generic converters | `delimiter`, `header` |

**Gaps we close that none of the four expose** (the reason this tool is worth shipping):
`rows = triangle` (one row per facet, `v1x…v3z` — what CAD/Excel importers actually want),
`columns = indexed` (`triangle`,`corner` numbering so a flattened CSV can be regrouped),
facet-normal columns with `normal_source = stored|computed` (competitor #2 discards normals outright;
STL's stored normals are famously `0 0 0` in many exporters, so a right-hand-rule fallback matters),
and exact-boundary/error messages that name the offending line.

**Out of model — listed, not built:**

- Interactive 3D preview / point-cloud rendering (2, 3) — needs a WebGL viewer; the generator's page
  renders text output, and this repo ships no 3D canvas.
- Batch conversion of many files at once (2) — the block takes one input; batching is a CLI shell loop.
- Export to PLY / PCD / GLB / glTF / OBJ (3) — mesh-to-mesh conversion; `blocks/mesh-convert` owns the
  OBJ↔STL direction and the rest are separate formats, not a CSV concern.
- Surface *sampling* (Poisson-disk / uniform resampling, as Open3D does) (3) — generates new points that
  are not in the file. This tool is deliberately an extractor: every row is a stored coordinate.
- 500 MB uploads (2) — capped at 32 MiB of pasted input / 100 000 triangles so it stays inside a
  wasm sandbox. Stated in the descriptor and on the page.
- Translate/rotate by arbitrary angles (3) — only the two 90° up-axis conventions plus uniform scale are
  offered; a general transform matrix is a modelling operation, not an export option.

## UX patterns adopted

- Preset chips (`[[example]]`) for the four real jobs: default triangle-corner CSV, one-row-per-triangle
  CAD export, deduped XYZ point cloud (space-delimited, no header), and a binary-STL (base64) run.
- Friendly `[input.labels]` on every enum so the selects read as tasks, not as enum values.
- `multiline = true` on the STL field so pasted ASCII STL and wrapped base64 both survive.
- Placeholders on every text/number field, per the hygiene gate.

## Verification notes

Advertised-values matrix covered by the Playwright spec + CLI runs: each `rows`, `columns`,
`normal_source`, `up_axis`, `dedupe`, `delimiter` choice; both accepted binary encodings (base64 AND
hex) plus `input_format = auto` detection; the non-default `header` checkbox state; and the exact
100 000-triangle cap boundary (a header-only binary STL declaring 100 001 facets).
