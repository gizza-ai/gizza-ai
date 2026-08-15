## About this tool

STL is the format almost every slicer and CAD package exports, and it is deliberately
minimal: a bare list of triangles, with no units, no vertex sharing, no colours and no
declared topology. That makes it easy to write and easy to get subtly wrong — a mesh can
look perfect on screen and still be inside-out, cracked along a seam, or made of two
objects that were never meant to touch.

This inspector reads one STL and reports **everything it can measure, without changing a
single byte**:

- **Input** — which encoding was actually found, and the solid name or the 80-byte binary
  header text.
- **Geometry** — triangle count, distinct welded vertices, bounding box, explicit min/max
  bounds, centre, surface area, absolute volume, and **signed** volume. The sign is the
  interesting one: on a closed mesh a negative signed volume means the normals point
  inward, i.e. the model is inside-out.
- **Mesh integrity** — watertight and manifold status, open (boundary) edges,
  non-manifold edges, edges whose two faces disagree on winding, disconnected shells,
  degenerate (zero-area) triangles, duplicate triangles, facet normals that disagree with
  the geometry by more than 1°, unset `(0,0,0)` normals, and non-zero attribute bytes
  (where some exporters hide per-facet colour).
- **Verdict** — print-ready, or a list of the specific defects that stop it, plus notes
  for things worth knowing that do not block a print.

Both STL flavours are accepted. **ASCII STL** is pasted as text. **Binary STL** is bytes,
so paste them base64- or hex-encoded (`base64 model.stl` or `xxd -p model.stl`); hex may
carry spaces, colons or dashes, and base64 may be standard or URL-safe, padded or not.
Auto-detection identifies binary by the `84 + 50 × triangle_count` length rule rather than
a leading `solid` keyword — plenty of exporters write the word "solid" into a *binary*
header, which is the classic reason a file is mis-read as text.

Everything runs locally in your browser. The mesh you paste never leaves the page.

### Worked example

A tetrahedron with corners at the origin and 10 mm along each axis, inspected with
`density = 1.24` (PLA):

```text
solid tetra
  facet normal 0 0 -1
    outer loop
      vertex 0 0 0
      vertex 0 10 0
      vertex 10 0 0
    endloop
  endfacet
  ... three more facets ...
endsolid tetra
```

Report:

```text
STL inspection report
=====================

Input
  Encoding              ASCII STL (text, auto-detected)
  Solid name / header   tetra
  Units                 mm
  Scale factor          1

Geometry
  Triangles             4
  Distinct vertices     4
  Bounding box          10 x 10 x 10 mm
  Bounds                min 0 0 0 / max 10 10 10 mm
  Center                5 5 5 mm
  Surface area          236.60254 mm²
  Volume                166.666667 mm³ (0.166667 cm³)
  Signed volume         166.666667 mm³ (positive — normals face outward)
  Estimated weight      0.206667 g at 1.24 g/cm³

Mesh integrity
  Watertight            yes
  Manifold              yes
  Open (boundary) edges 0
  Non-manifold edges    0
  Inconsistent winding  0
  Disconnected shells   1
  Degenerate triangles  0
  Duplicate triangles   0
  Normals mismatched    0
  Normals unset (0,0,0) 0
  Attribute bytes set   0

Verdict
  Print-ready           yes — closed, manifold, consistently wound, normals outward
```

The volume checks out by hand: a corner tetrahedron is `10 × 10 × 10 / 6 =
166.67 mm³`, which is `0.1667 cm³`, and at 1.24 g/cm³ that is `0.2067 g` of plastic.
Deleting one facet from the same mesh flips the verdict to
`no — 3 open (boundary) edges`, with `Watertight no` and the volume line marked
*approximate, the mesh is not closed*.

### Limits and edge cases

- **100,000 triangles** maximum. A binary STL at that cap is about 5 MB of bytes, or 6.7 MB
  once base64-encoded — already an awkward paste. Larger meshes are rejected with a clear
  message rather than freezing the tab, and a binary file whose header *declares* more than
  the cap is rejected before any bytes are read.
- **STL carries no units.** The `units` setting only labels the output and converts the
  volume to cm³ for the weight estimate; it never rescales the mesh. Use `scale` for that —
  `25.4` turns a model authored in inches into millimetres, `2` doubles every dimension and
  multiplies the volume by 8.
- **The weight estimate assumes a 100 % solid part.** Infill, wall count, supports and
  material shrinkage are slicer decisions and are not modelled. Treat it as the upper bound.
- **Volume needs a closed mesh.** The divergence-theorem sum still produces a number for an
  open mesh, but it is meaningless; the report says so on the volume line instead of quietly
  printing it.
- **Welding drives almost everything.** STL stores each triangle's three corners
  independently, so vertex count, edge topology, shells and the watertight verdict all
  depend on `weld_tolerance` (default `0.000001`). Raise it to `0.001` to find out whether
  hairline export cracks are the only thing keeping a mesh open; set `0` to merge only
  bit-identical coordinates.
- **Binary coordinates are 32-bit floats**, so a value like `0.1` round-trips as
  `0.100000001`. Measurements of a binary mesh differ from the same mesh in ASCII in the
  seventh significant digit — expected, not a bug.
- **It only reports.** Nothing is welded, re-wound, hole-filled or simplified; use a mesh
  repair tool for that and re-run this check afterwards.
- **No 3D viewer.** This page returns text and JSON, not a rendered preview — you cannot
  rotate or zoom the model here. Load it in a slicer or mesh editor for that.
- Only STL is read. OBJ, PLY, 3MF, STEP and glTF are different formats and are rejected.

## FAQ

<details>
<summary>How do I paste a binary STL?</summary>

Encode the file's bytes first, then paste the encoded text. On macOS or Linux,
`base64 model.stl | pbcopy` (or `| xclip`) gives you base64; `xxd -p model.stl` gives you
hex. Either is accepted, and the encoding is auto-detected — hex is tried before base64,
because every hex string is also valid base64 and would otherwise be mis-read. If
auto-detection guesses wrong, set the input encoding explicitly to "Binary STL bytes,
base64" or "Binary STL bytes, hex".

</details>

<details>
<summary>My mesh says it is not watertight, but it looks closed. What is wrong?</summary>

Almost always the corners do not match *exactly*. STL writes each triangle's vertices
separately, so two faces only share an edge if their corner coordinates land on the same
welded vertex. Rounding in the exporter can leave neighbours a fraction of a micron apart,
which shows up as thousands of open boundary edges along seams that look perfectly closed.
Raise the weld tolerance to something like `0.001` (in the mesh's own units) and re-run: if
the open-edge count drops to zero, the geometry is fine and only the file's precision was
the problem. If it stays high, there are real holes.

</details>

<details>
<summary>What does a negative signed volume mean?</summary>

The volume is computed by summing signed tetrahedra from the origin to each triangle, so
its sign follows the winding order. On a closed mesh, a positive result means the facet
normals point outward — the convention every slicer expects. A negative result means the
mesh is inside-out: it will often still slice, but supports, wall ordering and boolean
operations can come out backwards. The absolute volume is reported either way, and the
verdict calls out an inverted mesh explicitly. On an *open* mesh the sign is not
conclusive and the report says so instead.

</details>

<details>
<summary>Why do the stored facet normals not have to match the geometry?</summary>

Every STL facet stores a normal vector alongside its three corners, but the corner winding
already implies one. When the two disagree by more than 1°, the file is internally
inconsistent — usually because a mesh was mirrored or scaled by a negative factor without
recomputing the normals. Most slicers ignore the stored value and recompute from the
winding, so a mismatch is reported as a note rather than a defect. A `(0,0,0)` normal is
counted separately: it is explicitly allowed by the format and simply means "derive it".

</details>

<details>
<summary>Are several disconnected shells a problem?</summary>

Not by itself. A shell is a group of triangles connected through shared welded vertices, so
a plate with three separate parts on it legitimately reports three shells and still prints.
The count is worth checking anyway: an unexpected extra shell is usually a stray fragment
left behind by a boolean operation, or a duplicate object sitting exactly on top of the
first one. If each shell is individually closed, the whole file is still watertight and the
verdict stays print-ready, with the shell count listed as a note.

</details>

<details>
<summary>Can I use this in a script or in CI?</summary>

Yes — switch the report format to JSON. You get one flat object with `triangles`,
`distinct_vertices`, `bbox_min`, `bbox_max`, `size`, `center`, `surface_area`, `volume`,
`signed_volume`, `volume_cm3`, `weight_grams`, `watertight`, `manifold`,
`outward_normals`, `boundary_edges`, `non_manifold_edges`,
`inconsistent_winding_edges`, `shells`, `degenerate_triangles`, `duplicate_triangles`,
`normals_mismatched`, `normals_unset`, `attribute_bytes_set`, `print_ready`, plus `issues`
and `notes` arrays. Gate a build on `print_ready`, or assert a bounding box before sending
a part to a printer. The same inspection is available from the command line, which is
usually the easier CI path.

</details>

<details>
<summary>Will it repair the problems it finds?</summary>

No. This tool is read-only on purpose, so the report can be trusted as a description of the
file you actually have — nothing is welded, re-wound, hole-filled, decimated or
re-exported, and no output mesh is produced. Fixing holes, flipping inverted normals and
dropping degenerate facets are separate, destructive operations; run a mesh repair tool for
those, then paste the result back here to confirm the numbers moved the way you expected.

</details>
