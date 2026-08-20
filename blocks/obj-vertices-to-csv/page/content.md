## About this tool

OBJ Vertices to CSV extracts the stored vertex positions from Wavefront OBJ text. Every `v x y z` line that passes the filters becomes one output row, in file order. Faces, texture coordinates, normals, curves, material libraries, and mesh topology are not resampled or converted; this is a vertex-table extractor, not a mesh renderer.

The defaults produce a simple `x,y,z` CSV. Use the options to add the original 1-based OBJ vertex index, object/group/material context, per-vertex colour columns, Y-up/Z-up conversion, unit scaling, fixed decimal precision, repeated-position dedupe, object or group filters, and spreadsheet-friendly delimiter choices. Choose `delimiter=space` with `header=false` when you need a plain XYZ/ASC point-cloud text file.

### Worked example

Input OBJ:

```obj
# triangle
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 0.0 1.0 0.0
vt 0.0 0.0
vn 0.0 0.0 1.0
f 1/1/1 2/1/1 3/1/1
```

With the default options, the output is:

```csv
x,y,z
0.0,0.0,0.0
1.0,0.0,0.0
0.0,1.0,0.0
```

For a point-cloud export, set Delimiter to `space` and turn Include header row off.

## FAQ

<details>
<summary>Does this convert OBJ faces or sample points across mesh surfaces?</summary>

No. The output rows come only from stored OBJ `v` vertex lines. Face records (`f`) are ignored, so triangles are not expanded, resampled, triangulated, or densified. Use a mesh or point-cloud conversion tool if you need sampled surface points.

</details>

<details>
<summary>Why does the index column skip numbers after filtering?</summary>

`columns=indexed` writes the original 1-based OBJ vertex index, which is the number face records reference. If you filter by object/group or dedupe repeated positions, skipped vertices keep their original numbers out of the output, so remaining rows may jump from index 2 to index 9.

</details>

<details>
<summary>Can the tool keep per-vertex colours?</summary>

Yes, for OBJ files that use the common extended `v x y z r g b` form. The default `color=auto` adds `red,green,blue` columns when at least one vertex has colour values. Colour tokens are copied as written and are not rounded with the coordinate precision option.

</details>

<details>
<summary>How should I export a plain XYZ or ASC point-cloud file?</summary>

Set Delimiter to `space` and uncheck Include header row. That produces rows like `0.0 1.0 2.0`, one vertex per line. If your target expects a fixed number of decimal places, set Decimal places to a value from 0 to 15.

</details>

<details>
<summary>What are the limits and unsupported inputs?</summary>

The tool accepts Wavefront OBJ text up to 16 MiB and emits up to 500,000 vertex rows. It does not read binary formats, zipped models, STL, PLY, glTF/GLB, FBX, or referenced `.mtl` files. It parses and ignores a fourth OBJ `w` value on vertex lines, and it reports malformed vertex lines with their source line number.

</details>
