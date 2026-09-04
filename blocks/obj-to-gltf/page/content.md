## About this tool

OBJ is easy to paste into a bug report, but glTF and GLB are what most web viewers, engines and product configurators want. This converter turns a single pasted Wavefront OBJ model into glTF 2.0 without uploading it anywhere. Paste the matching MTL text when you have one, pick JSON glTF or binary GLB, and copy or save the result.

The converter handles the practical OBJ features used by small web assets: vertices, UVs, normals, faces, materials, negative indices and quads/n-gons. Faces are triangulated, primitives are grouped by material, the buffer is embedded in the `.gltf` JSON, and `.glb` is returned as a `data:model/gltf-binary;base64` URL.

### Worked example

Paste this OBJ:

```
v 0 0 0
v 1 0 0
v 0 1 0
f 1 2 3
```

With the defaults, the output starts like this:

```
{
  "asset": { "version": "2.0", "generator": "gizza-ai/obj-to-gltf" },
  "scenes": [{ "nodes": [0] }],
```

It also includes an embedded `data:application/octet-stream;base64,...` buffer, one mesh primitive, POSITION/NORMAL attributes and triangle indices.

If your OBJ came from a Z-up CAD or 3D-printing workflow, set **Source up axis** to Z-up. If it was authored in millimetres but the web viewer expects metres, set **Scale factor** to `0.001`.

### Limits and edge cases

- The combined OBJ + MTL paste is capped at **8 MB** and **200,000 triangles** after triangulation so the WebAssembly sandbox stays responsive.
- Texture image files referenced by `map_Kd` or other `map_*` lines are not embedded. Paste-only tools have no safe way to attach the PNG/JPG files that live beside an OBJ. Material colors from the MTL are still used.
- The MTL parser supports common color and material fields (`newmtl`, `Kd`, `d`, `Tr`, `Ke`, `Ks`, `Ns`, `illum`). It emits metallic-roughness glTF materials, not the archived specular-glossiness extension.
- The output uses a single scene, node and mesh. `usemtl` creates separate primitives, but OBJ `o`/`g` hierarchy is not preserved.
- Draco, meshopt, USDZ, FBX, DAE and batch conversion are separate workflows and are not built into this single pasted-text converter.
- The converter is for small to medium assets that fit in a paste box. For large textured production models, use a desktop exporter that can read every sidecar file.

## FAQ

<details>
<summary>Can I convert an OBJ that references texture images?</summary>

You can convert its geometry and material colors, but the texture image files are not embedded. OBJ stores image paths such as `map_Kd wood.png`; this page only receives pasted text, not the neighboring PNG or JPG files. The converter ignores those file references and keeps the rest of the material data.

</details>

<details>
<summary>Should I choose glTF or GLB?</summary>

glTF is JSON, so it is easier to inspect, diff and debug. This page embeds the binary buffer as a data URI, so it is still a single file. GLB is the compact binary container most viewers accept directly; choose it when you want a single saveable asset URL.

</details>

<details>
<summary>What does the Z-up option do?</summary>

Some CAD and 3D-printing tools treat +Z as up, while glTF uses +Y. Choosing Z-up rotates source positions and normals so +Z becomes +Y in the exported glTF. Triangle winding is preserved, so the faces keep the same orientation.

</details>

<details>
<summary>Will it keep quads and n-gons?</summary>

No. glTF mesh primitives are triangle-based in this converter, so quads and larger polygons are fan-triangulated. A four-corner face becomes two triangles; a five-corner face becomes three.

</details>

<details>
<summary>Does it upload the model?</summary>

No. The parser and exporter run in WebAssembly in the browser, and the CLI runs locally. The pasted OBJ and MTL text are not sent to a server.

</details>
