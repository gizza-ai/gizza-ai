# obj-to-gltf — competitor analysis (2026-09-04)

Scan run **before** implementing, per `/create-next-tool` step 4. Everything below is a
paraphrased summary of publicly documented behavior; no competitor copy, branding, or
trademarked wording is reproduced or reused. Out-of-model items are listed, not built.

Backlog row: `obj-to-gltf,dev,Converts a Wavefront OBJ (+MTL) model into a standard glTF/GLB
file for the web.,Convert this OBJ model to GLB,pure`.

## Duplicate check (done first)

- `blocks/mesh-convert` — OBJ ↔ STL over pasted text only. Grepped `core/src/lib.rs`: its
  `Target` enum is `Obj | Stl`; it reduces every input to a flat triangle list and discards
  materials/UVs/normals by design. **No glTF/GLB path at all** → not a duplicate.
- `blocks/ply-to-obj`, `blocks/stl-format-converter`, `blocks/stl-inspector`,
  `blocks/stl-repair`, `blocks/obj-vertices-to-csv` — other source/target formats or
  report-style tools; none emit glTF.
- `docs/tool-skiplist.txt:447` skiplists `glb-gltf-packer`, and that entry explicitly records
  "no existing glTF/GLB block". That row was blocked because *packing separate .gltf + .bin +
  N texture files* is multi-FILE input (and unpacking is multi-file output). This row is
  different and unblocked: one pasted OBJ (+ optionally one pasted MTL) → one self-contained
  glTF/GLB artifact, which is exactly the single-input/single-output shape `mesh-convert`
  already ships.

## Competitors reviewed

| # | Tool | Shape | Notable options |
|---|------|-------|-----------------|
| 1 | obj2gltf (CesiumGS, npm CLI/API) | Local CLI, the de-facto reference implementation | `--binary` (GLB), `--separate`/`--separateTextures`, `--unlit` (KHR_materials_unlit), `--metallicRoughness` / `--specularGlossiness`, `--doubleSidedMaterial`, `--checkTransparency`, `--packOcclusion`, `--input-up-axis` / `--output-up-axis`, per-map texture overrides, `--secure` |
| 2 | ImageToStl (obj→glb / obj→gltf) | Server upload | No exposed conversion settings; asks users to upload the MTL + texture images alongside the OBJ, embeds textures into the GLB, converts TGA→PNG; 500 MB/file, 200-file batch, outputs deleted after 4 h |
| 3 | Convert3D | Browser-local upload widget | No exposed settings; compatibility table claims materials + textures supported both ways, vertex colors not carried into glTF |
| 4 | GLTFTools converter | Browser-local (wasm) | Many format pairs (glTF/GLB/OBJ/FBX/DAE/STL/PLY/USDZ), texture embedding, separate compression tool, AR/print presets; no documented axis/scale/binary toggles on the converter page |
| 5 | FurniMesh obj→glb | Browser-local (wasm) | No exposed settings; claims materials/UVs/hierarchy preserved where GLB supports them; no size cap beyond browser memory |

Consistent picture: the hosted converters are drag-and-drop with **no knobs**; the real option
surface in this space is obj2gltf's. So obj2gltf sets the table stakes, and the hosted tools set
the copy/expectation bar (privacy, "what is preserved", speed, MTL handling).

## Table stakes → decision

| Capability | Seen in | Verdict | Where it landed |
|---|---|---|---|
| GLB (binary) output | 1, 2, 4, 5 | **in-model, built** | `to = glb` → `data:model/gltf-binary;base64,…` (saveable `.glb`), same data-URL pattern `mesh-convert` uses for binary STL |
| glTF JSON output, self-contained | 1, 3, 4 | **in-model, built** | `to = gltf` (default) → pretty-printed glTF 2.0 with the buffer embedded as a `data:application/octet-stream;base64,…` URI (single-file `.gltf`, no sidecar `.bin`) |
| MTL materials → PBR | 1, 2, 3, 5 | **in-model, built** | optional `mtl` textarea; `newmtl`/`Kd`/`d`/`Tr`/`Ks`/`Ns`/`Ke`/`illum` → `pbrMetallicRoughness.baseColorFactor`, `roughnessFactor` (from `Ns`), `emissiveFactor`, `alphaMode` BLEND when α<1 |
| Per-material split into primitives | 1 | **in-model, built** | `usemtl` groups become separate mesh primitives, each with its own material index |
| UVs (`vt`) → `TEXCOORD_0` | 1, 5 | **in-model, built** | emitted when the OBJ has `vt`, with the OBJ→glTF V flip (`v → 1−v`) |
| Normals (`vn`), or generated | 1 | **in-model, built** | `normals = auto` (use `vn`, fall back to computed face normals) / `flat` (always recompute) / `none` (omit and let the viewer shade flat) |
| Up-axis conversion | 1 (`--input-up-axis`) | **in-model, built** | `up_axis = y` (default, glTF-native) / `z` (rotate Z-up CAD/print models into glTF's Y-up) |
| Unlit materials (KHR_materials_unlit) | 1 | **in-model, built** | `unlit` checkbox → the extension + `extensionsUsed` |
| Double-sided materials | 1 | **in-model, built** | `double_sided` checkbox → `doubleSided: true` |
| Uniform scale / unit conversion | — (Blender-class exporters) | **in-model, built** | `scale` (e.g. `0.001` for mm→m); not a competitor table stake but a one-line win the OBJ↔STL sibling already ships |
| Quads/n-gons handled | 1, all | **in-model, built** | fan triangulation, negative (relative) OBJ indices supported |
| Stated privacy / local execution | 2, 3, 4, 5 | **in-model, built** | page copy states the paste never leaves the browser (it is genuinely wasm-local here) |
| Texture image embedding (`map_Kd` → glTF image) | 1, 2, 3, 5 | **out-of-model** | needs the PNG/JPG *files* alongside the OBJ. This surface takes pasted text; multi-file input has no page and no CLI attachment path (see the `glb-gltf-packer` skiplist reasoning). `map_*` lines are parsed only to warn, in the page copy, that the texture reference is dropped. |
| Draco / meshopt compression | 4 | **out-of-model** | separate codec dependency + a different artifact contract; not attempted |
| FBX / DAE / USDZ / PLY sources | 4 | **out-of-model here** | separate parsers and separate backlog rows |
| Batch / multi-file upload | 2 | **out-of-model** | single-input surface by design |
| Specular-glossiness input mode | 1 | **considered, rejected** | `KHR_materials_pbrSpecularGlossiness` is archived in glTF 2.0; emitting metallic-roughness only keeps the output on the live spec path. Noted on the page instead. |
| Occlusion packing / per-map texture overrides | 1 | **out-of-model** | texture-file dependent (same blocker as embedding) |
| Scene hierarchy from `o`/`g` groups | 5 (claims hierarchy) | **considered, rejected** | glTF gets one node + one mesh; `usemtl` already splits primitives, and a node-per-group adds schema weight without changing what any viewer renders. Stated as a limit on the page. |

## UX patterns adopted

- Preset chips (`[[example]]`): a ready-to-run OBJ cube → glTF, the same cube → GLB, and a
  Z-up + millimetre source, so the page shows real output on first click (competitors give
  drag-and-drop instant results; chips are the paste-surface equivalent).
- Friendly `<select>` labels via `[input.labels]` for `to` / `up_axis` / `normals`.
- Both text areas (`obj`, `mtl`) are `multiline` so pasted newlines survive.
- Page copy states the real limits up front (paste-size cap, triangle cap, no texture images,
  no Draco) rather than letting users find them via an error.
