# mesh-convert — competitor analysis (2026-07-18)

Tool function: convert 3D meshes between Wavefront OBJ and STL, both directions,
ASCII or binary STL. Pure-Rust/wasm, browser-local, no upload. Scan done BEFORE
implementing. All notes are paraphrased — no competitor copy/branding/trademarks
reproduced.

## Competitors scanned (WebSearch "convert OBJ to STL online free browser tool")

1. **ImageToStl** (imagetostl.com) — binary STL is the default output; optional
   non-standard colored-STL variants (VisCAM / SolidView) for colored faces.
   Accepts OBJ plus optional MTL + texture sidecars. Vertex normals are collapsed
   into one per-face normal on output. Drag-and-drop upload, batch up to 200 files,
   500 MB/file cap, individual or bulk ZIP download. Discards textures/materials.
2. **Convert3D** (convert3d.org) — minimal landing page; drag-and-drop upload, no
   sign-up, no server upload. Keeps geometry, drops textures/materials/vertex colors.
   No documented ASCII/binary toggle, scale, or axis options on the page.
3. **MeshInspector** (meshinspector.com) — "Accuracy Control", automatic mesh repair /
   "Mesh Healer" (holes, flipped normals, non-manifold edges), scaling via a Transform
   panel, bounding-box inspection, folder/batch import, unlimited size, and many extra
   formats (3MF, STEP, PLY, glTF, CTM). 100% local desktop / auto-delete web.
4. **Meshy** (meshy.ai) — client-side, files never leave the browser, 50 MB cap.
5. **FurniMesh** (furnimesh.com) — WebAssembly glTF/3D-mesh pipeline, fully in-browser,
   no sign-up, no watermark, no UI size cap.

(≥3 reachable competitors profiled in depth; the remaining two corroborated the
"client-side, no upload" positioning and size caps.)

## Table-stakes params & UX (tagged for model fit)

| capability | competitor(s) | model fit | decision |
| --- | --- | --- | --- |
| OBJ → STL and STL → OBJ (both directions) | all | in-model | build — auto-detect input, `to` select |
| ASCII vs binary STL output | ImageToStl (binary default) | in-model | build — `stl_encoding` enum (default ascii; binary via download) |
| Uniform scale factor | MeshInspector | in-model | build — `scale` number param |
| Axis reorientation (Y-up ↔ Z-up) | implied by 3D-print workflows | in-model | build — `axis` enum (graphics Y-up ↔ print Z-up) |
| Solid / object name | STL format field | in-model | build — `name` param (STL solid + OBJ `o`) |
| Recalculate normals to per-face | ImageToStl | in-model | build — STL facet normals computed from geometry |
| Drop textures/materials on OBJ→STL | all | in-model (inherent) | STL has no materials; documented |
| Drag-and-drop **file upload** | all | out-of-model | pure-Rust pages take pasted text, not file uploads (only ffmpeg-runtime pages upload files); paste instead |
| Batch (many files at once) | ImageToStl (200), MeshInspector (folders) | out-of-model | single paste per run; listed, not built |
| Mesh repair / healing (holes, non-manifold) | MeshInspector | out-of-model | needs heavy geometry pipeline; out of scope |
| 3D preview / viewer | several | out-of-model | no WebGL viewer in the generic page |
| Other formats (3MF, STEP, PLY, glTF, CTM) | MeshInspector, FurniMesh | out-of-model | scope is OBJ ↔ STL only |
| Colored-STL variants (VisCAM/SolidView) | ImageToStl | out-of-model | non-standard; standard STL only |

## Design (in-model descriptor)

- `mesh` (string, required) — OBJ or **ASCII** STL source text, format auto-detected.
- `to` (enum `obj` | `stl`, default `stl`) — output format.
- `stl_encoding` (enum `ascii` | `binary`, default `ascii`) — STL byte encoding (ignored for OBJ output). Binary is delivered as a downloadable file.
- `scale` (number, default 1.0) — uniform multiplier on every vertex.
- `axis` (enum `keep` | `y-up-to-z-up` | `z-up-to-y-up`, default `keep`) — rotate the coordinate frame (graphics Y-up ↔ 3D-print Z-up).
- `name` (string, default `mesh`) — STL solid name / OBJ object name.

## Stated limits (page)

- Input is pasted **text**: OBJ or **ASCII** STL. **Binary** STL cannot be pasted —
  re-export it as ASCII STL (every CAD/slicer offers this) or use OBJ. Binary STL
  **output** is fully supported (download button).
- Geometry only: OBJ materials/MTL, textures, UVs, and vertex colors are dropped —
  STL stores raw triangles. Polygonal OBJ faces are fan-triangulated.
