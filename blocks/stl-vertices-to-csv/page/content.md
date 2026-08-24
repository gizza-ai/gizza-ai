## About this tool

STL Vertices to CSV flattens a triangle mesh into a coordinate table. Paste ASCII STL text directly, or paste a binary STL as base64 or hex bytes. The default output writes three CSV rows per facet, one for each explicit corner stored in the STL file, in file order.

This is different from an STL inspector or repair tool: it does not measure volume, fix topology, weld the mesh, or generate new points. It simply exposes the stored triangle coordinates so you can audit them in a spreadsheet, feed a CAD import, create a point-cloud text file, or compare the raw facets from two exports.

### Worked example

Input STL:

```stl
solid tri
facet normal 0 0 1
outer loop
vertex 0 0 0
vertex 1 0 0
vertex 0 1 0
endloop
endfacet
endsolid tri
```

Default output:

```csv
x,y,z
0,0,0
1,0,0
0,1,0
```

Switch Rows to `triangle` when you want one row per facet with nine coordinate columns (`v1x` through `v3z`). Switch Delimiter to `space` and turn the header off for a plain XYZ/ASC point-cloud file.

## FAQ

<details>
<summary>Why are there three rows for each STL triangle?</summary>

STL stores every facet as three explicit corner coordinates. The default `rows=vertex` preserves that storage model: one output row per corner, so each triangle contributes three rows. Use `columns=indexed` or `full` to add the triangle and corner numbers, or use `rows=triangle` to keep each facet on one row.

</details>

<details>
<summary>Does this weld duplicate vertices?</summary>

Not by default. STL repeats shared corners in every triangle that touches them, and the default output keeps those repeats. Set Repeated rows to `all` if you want each distinct coordinate only once, which is useful for point-cloud exports.

</details>

<details>
<summary>Can it parse binary STL files?</summary>

Yes. Paste the binary STL bytes as base64 or hex and leave Input format on `auto`, or force `base64`/`hex`. Auto-detection treats a binary STL as binary by its `84 + 50 × triangle_count` byte layout, not by the leading `solid` word, because many binary STL headers also start with `solid`.

</details>

<details>
<summary>What are stored and computed normals?</summary>

Stored normals are the `facet normal` vectors in the STL file. Many exporters write `0 0 0` or stale values there. Computed normals are derived from the triangle corner order with the right-hand rule, so they reflect the geometry being exported.

</details>

<details>
<summary>What are the limits and unsupported cases?</summary>

The tool accepts up to 100,000 triangles and 32 MiB of pasted input. It does not render a 3D preview, repair bad topology, fill holes, sample triangle surfaces, batch files, or convert PLY/OBJ/glTF/3MF containers. For mesh metrics use STL Inspector; for repairs use STL Repair.

</details>
