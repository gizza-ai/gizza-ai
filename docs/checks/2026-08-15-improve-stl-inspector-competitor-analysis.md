# stl-inspector — competitor analysis (2026-08-15)

Scan run BEFORE implementation, per `/create-next-tool` step 4. One WebSearch
("online STL file inspector analyzer triangle count volume surface area watertight
check"), then the top 3 reachable real competitors were skimmed. Everything below is
**paraphrased** — no competitor copy, branding, or trademark is reproduced, and
out-of-model items are listed, not built.

## Competitors skimmed

| # | Tool | URL | Shape |
|---|------|-----|-------|
| 1 | iamRapid STL Analysis | https://iamrapid.com/tools/stl-analysis/ | Drag-drop upload, browser-local analysis, interactive 3D preview, print-ready verdict |
| 2 | SANIX3D Online STL File Analyzer | https://sanix3d.com/online-stl-file-analyzer/ | Upload + 3D preview, geometry readout plus resin/filament weight and print-cost estimation |
| 3 | GrandpaCAD Measure STL | https://grandpacad.com/en/tools/measure-stl | File attach, bounding box / volume / area / counts / watertight, links out to weight + scale tools |

(A fourth, 3D-AI STL Checker, was seen in results and matches #1's field set — triangle
count, bounding box, watertight edge check — so it added nothing new and was not
counted toward the three.)

## Table-stakes matrix

Every row lands in the descriptor OR in the out-of-model list below — nothing dropped
silently.

| Capability | Seen at | Decision | Where it lands |
|---|---|---|---|
| ASCII **and binary** STL input | 1, 2, 3 | **in-model** | `input_format` = `auto`/`ascii`/`base64`/`hex`; binary bytes pasted as base64 or hex (repo precedent: `blocks/bson-inspector`). Auto-detection prefers the binary 84 + 50·n length check over the `solid` prefix, which is the classic mis-detection trap |
| Triangle count | 1, 2, 3 | in-model | `triangles` in report + JSON |
| Vertex count (welded/distinct) | 1, 3 | in-model | `distinct_vertices`, driven by `weld_tolerance` |
| Bounding box X × Y × Z | 1, 2, 3 | in-model | `bounding box`, plus explicit `min`/`max` bounds and `center` |
| Surface area | 1, 2, 3 | in-model | `surface_area` (sum of triangle areas) |
| Volume | 1, 2, 3 | in-model | `volume` (absolute) **and** `signed_volume` — the sign is what reveals an inside-out mesh, which none of the three surface directly |
| Watertight status | 1, 2, 3 | in-model | `watertight` = zero boundary edges and zero non-manifold edges |
| Manifold status | 1 | in-model | `manifold` + `non_manifold_edges` count |
| Separate shell count | 1 | in-model | `shells` (union-find over shared welded vertices) |
| Print-ready / needs-repair verdict | 1, 2 | in-model | `Verdict` section with the specific reasons, not a bare yes/no |
| Units mm / cm / in | 2, 3 | in-model | `units` enum; labels every length/area/volume and drives the cm³ cross-check |
| Scale / resize percentage | 2, 3 | in-model | `scale` multiplier applied before measuring |
| Weight estimate from material density | 2, 3 | in-model | `density` (g/cm³, 0 = skip) → estimated grams. The *density table* is not: users pass the number |
| Open-edge / hole reporting | 2 | in-model | `boundary_edges` |
| Degenerate + duplicate triangle counts | (repair-side of 1, 3) | in-model | `degenerate_triangles`, `duplicate_triangles` |
| Stored-vs-computed normal check | — (none report it) | in-model, **ours goes further** | `normals_mismatched` (>1° from the geometric normal) and `normals_unset` (0,0,0 facets) |
| Interactive 3D viewer / rotate / zoom / PNG snapshot | 1, 2, 3 | **out-of-model** | The generic tool-page generator renders text/media output; there is no WebGL viewer control kind and no GPU surface here |
| Drag-and-drop file upload of a `.stl` | 1, 2, 3 | **out-of-model** on this page surface | `source = "file"` page inputs are wired to the ffmpeg/model runtimes only; pure-wasm pages take pasted fields. Binary STL is therefore pasted as base64/hex, exactly as `bson-inspector` does |
| Print cost (filament/resin price, electricity, currency) | 2 | **out-of-model** | Needs live pricing tables + currency data; it is commerce estimation, not mesh math |
| Support density / wall thickness / infill allowances | 2 | **out-of-model** | Slicer-level estimation; the numbers depend on a slicing engine this repo does not have |
| Mesh **repair** (weld, re-wind, hole fill, drop fragments) | 1, 3 | **out-of-scope by design** | Already shipped as `blocks/stl-repair`; this tool is read-only and the page/FAQ points there |
| STEP / IGES / 3MF / PLY / GLB input | 1, 3 | **out-of-model here** | Separate parsers; separate backlog rows (`ply-to-obj`, `obj-to-gltf`, …) |
| OBJ input | 1, 3 | out-of-scope by design | `blocks/stl-repair` and `blocks/mesh-convert` already read OBJ; widening this slug would duplicate them |

## Duplicate check (done first, per skill step 2)

`ls blocks/ | grep -iE 'stl|mesh|3d'` → `stl-repair`, `mesh-convert`. Both were read
before building:

- `blocks/stl-repair` — its `output=report` path does report triangles, distinct
  vertices, bounds, area, volume, watertight, boundary/non-manifold edges and shells,
  so the **ASCII half overlaps**. But its own descriptor states binary STL cannot be
  pasted as text, and it is a *mutating* repair tool.
- `blocks/mesh-convert` — OBJ↔STL conversion; **emits** binary STL as a data URL but
  also refuses binary STL input.

So **no block in this repo can read a binary STL at all**, and binary is the encoding
almost every slicer and CAD package exports by default. That unserved headline
capability — plus a read-only inspection surface (signed volume, normal-vs-geometry
mismatch, print-ready verdict) — is what justifies a separate block rather than a
skiplist. The page and FAQ cross-link to the repair tool instead of re-implementing it.

## UX / control patterns adopted

- Placeholder on every text and number field (a real base64 binary STL snippet on the
  main input, `1` for scale, `1.24` for density).
- `[[example]]` preset chips, since two of the three competitors ship a sample-file
  button: an ASCII tetrahedron, a base64 binary cube, and a PLA-density weight run.
- `[input.labels]` for friendly enum labels (`Auto-detect`, `Millimetres (mm)`, …).
- Enums as real `<select>`s via `Param::enumv` (`input_format`, `output`, `units`).
- Worked example on the page showing input **and** exact output, plus a stated
  triangle cap and the known limits.
