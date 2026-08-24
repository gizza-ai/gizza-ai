# ply-to-obj competitor analysis (2026-08-23)

Backlog row: `ply-to-obj` — converts a PLY mesh or point cloud into a Wavefront OBJ file.

Research query used by the builder: online PLY to OBJ converter / PLY mesh point cloud Wavefront OBJ conversion.

## Competitor scan

| Competitor class | Observed table stakes | Fit decision |
|---|---|---|
| General online mesh converters (MeshConvert / AnyConv-style tools) | Upload a PLY file and download OBJ; support ascii and binary inputs; hide low-level vertex-property decisions. | In model for the core PLY→OBJ conversion. Out of model for upload queues, server-side conversion and broad cross-format conversion. |
| Desktop mesh tools (MeshLab / CloudCompare / Blender import-export) | Preserve point clouds, vertex colors, normals and texture coordinates where OBJ can represent them; offer axis/scale transforms during export. | In model. The tool keeps colors (`v x y z r g b`), `vn`, `vt`, point-cloud vertices, scale and Y-up/Z-up transforms. |
| CAD/3D-print converters | Need triangulated faces and simple geometry-only OBJ for slicers/engines that reject n-gons or extra attributes. | In model. `triangulate=true` fan-splits n-gons, and booleans can drop colors/normals/UVs. |
| Viewer/analyser surfaces | Summarise vertex/face counts, properties, comments, and ignored data before export. | In model. `output=summary` reports encoding, counts, properties written, elements and ignored vertex fields. |
| Material/texture exporters | Emit `.mtl` files, texture images, materials and alpha. | Out of model for this repo surface: PLY does not carry OBJ material libraries; per-vertex alpha has no standard OBJ target. Listed in docs instead of built. |

## Controls and defaults

| Capability | Control/default | In model? | Decision |
|---|---:|---|---|
| ASCII PLY text and binary PLY bytes | `input_format=auto|ascii|base64|hex` | Yes | Auto reads ASCII, hex or base64 bytes. |
| Meshes and point clouds | same input | Yes | No-face PLY emits `v` lines only. |
| Vertex colors | `colors=true` | Yes | Writes OBJ color extension with RGB normalised to 0..1. |
| Vertex normals | `normals=true` | Yes | Writes `vn` lines and face references. |
| Texture coordinates | `uvs=true` | Yes | Writes `vt` from common PLY UV property names. |
| N-gon vs triangle output | `triangulate=false` | Yes | Keeps n-gons by default; fan triangulates on request. |
| Axis and scale conversion | `axis=keep`, `scale=1` | Yes | Positions scale; positions and normals rotate together. |
| Object name | `name=mesh` | Yes | Writes an OBJ `o` line. |
| Summary instead of OBJ | `output=obj|summary` | Yes | Included for diagnostics and ignored properties. |
| `.mtl`/textures/materials/alpha | n/a | No | Documented as a limit; not emitted. |

## Worked examples selected

- ASCII triangle to OBJ for the smallest verifiable mesh.
- Colored point cloud to show faces are optional and RGB survives.
- Quad triangulation with scaling and axis conversion for 3D-print/game-engine workflows.
- Summary report for scanner exports with comments and extra properties.
