## About this tool

**OBJ ↔ STL Mesh Converter** converts simple triangle meshes between Wavefront
OBJ and STL directly in the browser. Paste OBJ text (`v` and `f` lines) or ASCII
STL text (`solid` / `facet` / `vertex`), choose the output format, and get a
copyable mesh result.

- **OBJ to STL.** OBJ vertices and faces are triangulated and emitted as ASCII
  STL, or as a binary STL `data:model/stl;base64,...` URL you can save.
- **STL to OBJ.** ASCII STL facets are read as triangles and written as a clean
  OBJ object with deduplicated vertices and `f` triangle faces.
- **Geometry options.** Apply a uniform scale factor and optionally rotate between
  graphics-style Y-up coordinates and CAD/3D-print Z-up coordinates.
- **Local conversion.** The parser and writer run as WebAssembly in your browser;
  the pasted mesh is not uploaded.

### Worked example

Paste this OBJ triangle, leave **Convert to** as **STL**, **STL encoding** as
**ASCII STL**, **Scale factor** as `1`, **Axis conversion** as **Keep axes**, and
set **Mesh name** to `triangle`:

```obj
v 0 0 0
v 1 0 0
v 0 1 0
f 1 2 3
```

The output is:

```stl
solid triangle
  facet normal 0 0 1
    outer loop
      vertex 0 0 0
      vertex 1 0 0
      vertex 0 1 0
    endloop
  endfacet
endsolid triangle
```

The normal is recomputed from the face winding. If you choose **Binary STL data
URL**, the output starts with `data:model/stl;base64,` and can be saved as an
`.stl` file.

## FAQ

<details>
<summary>Can I upload a binary STL file?</summary>

No. This page takes pasted text, so it can read Wavefront OBJ text and ASCII STL
text. Binary STL input is raw bytes and cannot be safely pasted into a textarea;
re-export it as ASCII STL first, then convert it here. Binary STL is supported as
an output via a saveable data URL.

</details>

<details>
<summary>What happens to OBJ materials, UVs, normals, and textures?</summary>

They are not preserved. STL stores only triangles, and this converter deliberately
reduces the mesh to triangle geometry. OBJ `vt`, `vn`, `usemtl`, `mtllib`, groups,
and texture references are ignored on input. STL facet normals are recomputed
from triangle winding when STL is written.

</details>

<details>
<summary>How are OBJ quads or polygons handled?</summary>

Faces with more than three vertices are fan-triangulated: `f 1 2 3 4` becomes
triangles `(1,2,3)` and `(1,3,4)`. Negative OBJ indices are supported. Curved
surfaces, NURBS, smoothing groups, and material assignments are not evaluated.

</details>

<details>
<summary>When should I use the Y-up/Z-up axis options?</summary>

Many graphics tools treat Y as the vertical axis, while CAD and 3D-printing
workflows commonly treat Z as vertical. Choose **Y-up → Z-up** when moving a
model from a graphics coordinate frame into a printing/CAD coordinate frame, and
**Z-up → Y-up** for the reverse. Leave it on **Keep axes** if your coordinates are
already correct.

</details>

## Limits & notes

- Input must be text: Wavefront OBJ or ASCII STL. Binary STL input is not
  accepted.
- The converter preserves geometry only: no materials, textures, colors,
  smoothing groups, UVs, or custom metadata.
- OBJ polygon faces are triangulated with a simple fan, which is best for convex
  polygons. Non-planar or self-intersecting polygons may not triangulate the way
  a modeling package would.
- The binary STL option returns a base64 data URL rather than a raw byte preview;
  save it with a `.stl` extension if you need a file.
