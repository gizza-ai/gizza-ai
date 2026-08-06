# stl-repair — competitor analysis (2026-08-06)

Scan run BEFORE implementation, per `.claude/skills/create-next-tool/SKILL.md` step 4.
Everything below is paraphrased from public product pages; no competitor copy, branding,
or trademark text is reused anywhere in the tool.

## Sources skimmed

1. iamRapid — "Repair 3D file" (https://iamrapid.com/tools/repair-3d-file/)
2. JustFixSTL (https://www.justfixstl.com/)
3. TrazaLab — STL file repair (https://trazalab.com/stl-file-repair-online.html)

Also surveyed from search result summaries (not fetched individually): SphereLinks, 3D-Editor,
Remeshy, AnyOnlineTool, stlfixer.net, Polyvia3D. Their advertised feature sets are a subset of
the three above.

## What competitors do

| Capability | iamRapid | JustFixSTL | TrazaLab | Our decision |
| --- | --- | --- | --- | --- |
| Detect degenerate / zero-area faces | yes | yes | yes | **in model** — `remove_degenerate` (default on) |
| Detect + remove duplicate faces | yes | (implied) | yes | **in model** — `remove_duplicates` (default on) |
| Weld duplicate / coincident vertices | yes | yes | yes | **in model** — `weld_tolerance` (default 1e-6, exact welding at 0) |
| Report watertight yes/no | yes | yes | yes (as a grade) | **in model** — reported before AND after repair |
| Count non-manifold edges | yes | yes | yes | **in model** — reported before and after |
| Count open boundary edges / holes | yes | yes | yes | **in model** — boundary edge + hole-loop counts |
| Fix flipped / inconsistent normals | yes | yes | yes | **in model** — `fix_winding` (default on): per-shell BFS winding harmonisation + signed-volume outward flip |
| Recompute facet normals | implied by binary STL export | yes | yes | **in model** — always recomputed from winding on export |
| Fill holes | yes | yes | yes ("curvature-aware") | **in model, simplified** — `fill_holes` (default off) fan-triangulates each closed boundary loop from its centroid. Curvature-aware/advanced patching is **out of model**; the page says so. |
| Count disconnected shells | yes | yes (isolated vertices) | yes (fragments) | **in model** — shell count reported |
| Remove isolated fragments | implied | yes | yes | **in model** — `keep_largest_shell` (default off) |
| Self-intersection detection/repair | yes (flags leftovers) | yes | yes | **OUT OF MODEL** — needs exact/robust predicates and a CSG-grade remesher; listed on the page as a known limit, not silently dropped. |
| Triangle / vertex counts | yes | yes | — | **in model** — reported before and after |
| Surface area, volume, bounding box, dimensions | yes | — | — | **in model** — area always, volume only when the result is closed (otherwise it is meaningless), bbox + dimensions always |
| Binary STL export | yes (binary only) | yes (binary + ASCII) | yes | **in model** — `stl_encoding` = `ascii` \| `binary` (binary returns a `data:model/stl;base64,…` URL) |
| OBJ input | yes | yes (+ OFF) | yes (+ PLY) | **in model for OBJ** — input format auto-detected (ASCII STL or Wavefront OBJ). OFF/PLY and **binary STL input** are out of model here: this is a paste-text tool with no binary upload surface. |
| A–F quality grade | — | — | yes | **skipped deliberately** — an invented letter grade is not a fact about the mesh; we report the underlying counts instead. |
| 3D viewport / before-after preview, measurement tools, thickness analysis, auto-rotate | yes | yes (viewport) | yes (overlays) | **OUT OF MODEL** — the gizza page runtime renders text/media, not an interactive WebGL mesh viewer. |
| Upload up to 100–200 MB files | yes | yes | yes | **out of model** — paste-text input; we cap at 100 000 triangles and say so on the page. |

## Defaults + UX patterns worth matching

- Every competitor is **one-click auto-repair**: no visible knobs. Our defaults reproduce that —
  paste, press run, get a report with degenerate/duplicate removal, welding and winding fixes
  already applied. The knobs exist for people who need them but nobody has to touch them.
- All three are **local/private** ("files never leave your device"). Ours is wasm in the page /
  a local CLI — same property, stated on the page.
- All three lead with a **diagnostic report**, not the file. Hence `output = report` is the
  default; `output = stl` returns the repaired mesh and `output = json` the machine-readable
  version of the same report.
- Competitors ship no presets, so instead of arbitrary chips we ship `[[example]]` chips for the
  three real use cases: diagnose a broken cube, export the repaired STL, close a hole.

## Table-stakes that are NOT in the descriptor (out-of-model list)

- Self-intersection detection and repair.
- Curvature-aware / advanced hole patching (we do centroid fan-fill only).
- Interactive 3D preview, measurement and wall-thickness analysis.
- Binary STL, PLY and OFF **input** (no binary paste/upload surface on this page).
- Remeshing / decimation / smoothing.

## Not copied

No competitor wording, screenshots, naming, or grading scheme was reused. The report layout,
parameter names and page copy were written from scratch for this tool.
