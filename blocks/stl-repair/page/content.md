## About this tool

Slicers are unforgiving: a model that looks perfect in a 3D viewer can still refuse to
slice because two triangles overlap, one face points inward, or a hairline crack runs
along a seam. This tool reads a mesh as **ASCII STL** or **Wavefront OBJ** text, tells you
exactly which of those problems it has, fixes the ones it can, and reports whether the
result is finally watertight.

The repair pass does five things, each of which you can turn off:

- **Weld coincident vertices.** STL stores every triangle's three corners separately, so
  nothing is connected until matching corners are merged. Corners closer together than the
  weld tolerance become one vertex — this is what closes hairline cracks from a sloppy
  export. Set the tolerance to `0` to merge only bit-identical positions.
- **Remove degenerate triangles** — zero-area faces whose corners collapse onto the same
  vertex or sit exactly on a straight line. They carry no surface and confuse slicers.
- **Remove duplicate triangles** — faces built from the same three vertices in any rotation
  or winding. Duplicates show up downstream as non-manifold edges and doubled walls.
- **Fix winding and normals.** Every face in a shell is made to agree on winding, then each
  closed shell is turned so its normals point outward. Facet normals are always recomputed
  from the geometry, so whatever normals the file claimed are ignored.
- **Fill holes** (off by default) and **keep only the largest shell** (off by default), for
  meshes with open boundaries or stray scanner fragments.

Then it measures the result: triangle and vertex counts, non-manifold edges, open boundary
edges, shell count, watertight yes/no, surface area, volume, and the bounding box.
Everything runs locally in your browser — the mesh is never uploaded anywhere.

### Worked example

A tetrahedron whose base face was exported wound the wrong way:

```
solid tetra
facet normal 0 0 0
outer loop
vertex 0 0 0
vertex 1 0 0
vertex 0 1 0
endloop
endfacet
facet normal 0 0 0
outer loop
vertex 0 0 0
vertex 1 0 0
vertex 0 0 1
endloop
endfacet
facet normal 0 0 0
outer loop
vertex 0 0 0
vertex 0 0 1
vertex 0 1 0
endloop
endfacet
facet normal 0 0 0
outer loop
vertex 1 0 0
vertex 0 1 0
vertex 0 0 1
endloop
endfacet
endsolid tetra
```

With the default settings the report comes back as:

```
STL repair report
=================

Input
  Format                  ASCII STL
  Solid name              tetra
  Triangles               4
  Distinct vertices       4

Problems found
  Degenerate triangles    0
  Duplicate triangles     0
  Coincident vertices     0
  Non-manifold edges      0
  Open (boundary) edges   0
  Flipped triangles       3
  Disconnected shells     1
  Watertight              no

Repairs applied
  Vertices welded         0
  Degenerate removed      0
  Duplicates removed      0
  Triangles re-wound      1
  Shells turned outward   0
  Holes filled            0 (0 triangles added)
  Fragments removed       0 (0 triangles dropped)

Result
  Triangles               4
  Distinct vertices       4
  Non-manifold edges      0
  Open (boundary) edges   0
  Disconnected shells     1
  Watertight              yes
  Surface area            2.366025
  Volume                  0.166667
  Bounding box            1 x 1 x 1
  Bounds                  min 0 0 0 / max 1 1 1
```

Three edges disagreed about which way round the surface went, one face was to blame, and
after re-winding it the solid is watertight with a volume of 1/6. Switch **Return** to
*Repaired mesh (STL)* to get the corrected file instead of the report, or to *Report as
JSON* to feed the same numbers into a script.

### What it does not do

- **Self-intersections** are neither detected nor repaired — that needs exact geometric
  predicates and a CSG-grade remesher.
- **Hole filling is a flat fan**, not a curvature-aware patch: each closed boundary loop is
  capped with triangles fanned from the loop's own centroid. Simple, roughly flat holes come
  out fine; a large or twisted hole gets a visibly flat cap. Boundary chains that never close
  are left alone rather than patched wrongly.
- **No remeshing, decimation or smoothing**, and no 3D preview.
- **Binary STL, PLY and OFF cannot be pasted** — this is a text-in, text-out tool. Re-export
  binary STL as ASCII STL first. (Binary STL *output* is available, as a data URL.)
- Input is capped at **100,000 triangles**.

## FAQ

<details>
<summary>What does "watertight" actually mean here?</summary>

A mesh is reported as watertight when every edge is shared by exactly two triangles that
traverse it in opposite directions. That means no open boundary edges, no edges shared by
three or more faces, and no neighbouring faces that disagree about which side is out. It is
the property a slicer needs in order to decide what is inside the solid and what is outside.

</details>

<details>
<summary>My mesh looks closed but reports thousands of open edges. Why?</summary>

Almost always the weld tolerance is too small for the file's precision. STL writes each
triangle's corners independently, often rounded to six or seven digits, so two faces that
should share a corner end up a fraction of a unit apart and the edge between them never
pairs up. Raise the weld tolerance — try `0.001` for a model measured in millimetres — and
run it again. The "Vertices welded" line tells you how many corners the tolerance merged.

</details>

<details>
<summary>Why is the volume shown as "n/a"?</summary>

Volume is only reported when the repaired mesh is closed. An open surface does not enclose a
region, so any number would be meaningless — the same triangles could be read as enclosing
almost anything. Close the mesh (enable hole filling, or raise the weld tolerance) and the
volume appears. Surface area and the bounding box are always reported, closed or not.

</details>

<details>
<summary>What is the difference between a flipped triangle and an inside-out model?</summary>

A flipped triangle disagrees with its immediate neighbours, so it shows up as a count under
"Flipped triangles" and is re-wound individually. An inside-out model is internally
consistent — every face agrees with every neighbour — but the whole shell faces the wrong
way. That is detected from the enclosed volume's sign and fixed by turning the entire shell,
which is what the "Shells turned outward" line counts.

</details>

<details>
<summary>Can I feed it an OBJ file?</summary>

Yes. The format is auto-detected from the text: `solid`/`facet`/`vertex` lines are read as
ASCII STL, and `v`/`f` lines as Wavefront OBJ. OBJ polygons are fan-triangulated, and OBJ
materials, UVs and vertex normals are dropped — STL stores raw triangles only. Output is
always STL.

</details>

<details>
<summary>Does my model get uploaded anywhere?</summary>

No. The whole repair runs as WebAssembly inside this page, so the mesh text never leaves
your device and no network request is made with it. The same code is available offline
through the command-line tool.

</details>
