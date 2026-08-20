# obj-vertices-to-csv — competitor analysis (2026-08-20)

Scan run BEFORE implementing, per the create-next-tool recipe. All findings are **paraphrased
observations of publicly documented behaviour** — no competitor copy, branding, or trademarks are
reproduced or reused anywhere in the tool.

Function searched: extracting the vertex (`v`) coordinates of a Wavefront OBJ mesh into a flat
`x,y,z` text/CSV table.

## Competitors reviewed (top 3 real tools)

### 1. ImageToSTL — OBJ → XYZ point cloud (browser upload/convert service)

- **Surface:** file upload (drag-and-drop), server-side conversion, download link.
- **Options exposed:** none. No unit/scale, axis, precision, delimiter, header or filtering
  controls are documented on the page.
- **Output:** an `.xyz` point cloud — plain ASCII, one point per line, whitespace-separated.
  Documented explicitly: only vertex information is kept, and everything else (faces, materials,
  colours) is discarded.
- **Limits (stated):** up to 200 files per batch, 500 MB per file, files retained ~4 hours,
  slower/limited conversions when an ad blocker is active.
- **Takeaway:** confirms the core contract users expect — *vertices only, faces/materials dropped,
  one point per line* — and shows the whole category ships with **zero** formatting controls.

### 2. Autoconverter / Autoshaper (automapki) — OBJ → XYZ / ASC point cloud (desktop app)

- **Surface:** desktop app; File → Open, 3D viewport preview, File → Save As with the XYZ target.
- **Options exposed:** the conversion page documents no coordinate-formatting options (no
  separator, precision, axis-order or header settings).
- **Output:** ASCII, "each row contains exactly one point with X, Y and Z values", explicitly no
  headers or metadata.
- **Notes:** emphasises that the XYZ result stores points independently — connectivity (faces,
  edges) is lost by design.
- **Takeaway:** same output contract; the differentiator it offers over the web tools is a
  **preview of the model before export**, not richer output options.

### 3. MeshLab — import OBJ, export ASC/XYZ (desktop, open source)

- **Surface:** desktop; File → Import Mesh (OBJ among many formats), then export to `.asc`/`.xyz`.
- **Options exposed:** export-time selection of which per-vertex attributes to write. The ASC/XYZ
  family is described as a plain 3-column X Y Z text file, and MeshLab-family workflows commonly
  extend those rows with per-vertex **colour** columns when the mesh carries them.
- **Output:** whitespace-separated columns, no header row.
- **Takeaway:** the only competitor that acknowledges **per-vertex colour** riding along with the
  positions — which matches the extended OBJ `v x y z r g b` form written by scanners and by
  MeshLab itself.

Adjacent, reviewed and rejected as a competitor: online 3D viewers (e.g. the popular open-source
web viewer that loads OBJ plus a dozen CAD formats). They render and convert between mesh formats
but do not expose the vertex table as text, so they are not in this tool's category.

## Table stakes → decisions

| Table stake (observed) | Verdict | Where it lands |
| --- | --- | --- |
| Extract `v` lines only; discard `f`, `vt`, `vn`, materials | **in-model, built** | core parser ignores everything but `v` |
| One point per output line, in file order | **in-model, built** | default output |
| Plain X/Y/Z columns | **in-model, built** | `x,y,z` header, `columns = "xyz"` default |
| Header-less plain text (competitors emit no header) | **in-model, built** | `header = false` |
| Whitespace/other separators (XYZ/ASC are space-separated) | **in-model, built** | `delimiter` = comma / semicolon / tab / pipe / space |
| Per-vertex colour columns when present (MeshLab family) | **in-model, built** | `color` = auto / always / never → `red,green,blue` |
| Batch conversion of many files at once (200 files) | **out-of-model** | single-input tool; listed, not built |
| 500 MB uploads / server-side conversion + retained downloads | **out-of-model** | runs locally in the browser; input capped at 16 MiB |
| 3D preview of the mesh before export (Autoconverter) | **out-of-model here** | no viewport in the generic tool-page runtime; listed, not built |
| Surface *sampling* (points generated across triangles, e.g. CloudCompare/obj2pcd) | **out-of-model** | that is mesh resampling, a different tool; we extract stored vertices verbatim |

## Gaps we close that no competitor offers

Every competitor ships a zero-option converter, so the whole option set below is upside rather than
catch-up. Each is cheap, pure and deterministic:

- **`precision`** — keep each coordinate's source text byte-for-byte (default) or round to 0–15
  decimals. Competitors reformat silently; keeping the source token means a round-trip that changes
  nothing.
- **`up_axis`** — OBJ is Y-up, while CAD/GIS/printing pipelines are usually Z-up. `y-to-z` and
  `z-to-y` apply the standard ±90° rotation about X so the CSV lands in the target convention.
- **`scale`** — unit conversion (m → mm ×1000, mm → m ×0.001) in the same pass.
- **`objects`** — keep only vertices belonging to named `o`/`g` sections, which is how multi-part
  OBJ exports are segmented.
- **`columns`** — optional `index`, `object`, `group` and `material` context columns, so a vertex
  row can be traced back to its section (OBJ face indices are 1-based and global; `indexed` makes
  that index explicit).
- **`dedupe`** — drop adjacent or all repeated positions (welded-vertex exports repeat positions
  across seams).
- **RFC-4180 quoting** — object/group/material names can contain the delimiter; competitors' plain
  space-separated output cannot express that.

## Explicit non-goals (stated on the page, not silently dropped)

- No mesh resampling/densification — output rows are exactly the stored `v` lines that survive the
  filters.
- Faces (`f`), texture coordinates (`vt`), normals (`vn`), parameter-space vertices (`vp`),
  curves/surfaces and `.mtl` materials are not exported (`usemtl` is available only as a context
  column).
- A 4th `w` value on a `v` line (rational-curve weight, defaults to 1) is parsed and ignored.
- Binary or zipped model containers (STL, PLY, glTF/GLB, FBX) are out of scope — `mesh-convert`
  handles OBJ↔STL geometry conversion, `stl-inspector` inspects STL.

## Duplicate check

Not a duplicate. `blocks/mesh-convert` parses OBJ but *requires* `f` faces (it errors with "OBJ has
no faces (f lines) to build triangles from") and emits OBJ/STL geometry, never a coordinate table.
`blocks/stl-inspector` and `blocks/stl-repair` are STL-only. `blocks/geojson-coords-to-csv` is the
same *shape* of tool for GeoJSON positions, not for 3D mesh vertices. Nothing in `blocks/` produces
a vertex table from a Wavefront OBJ.
