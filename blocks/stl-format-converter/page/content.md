## About this tool

An STL file stores the same thing two ways. **Binary STL** is an 80-byte header, a 4-byte
triangle count, then exactly 50 bytes per triangle — compact and fast, but not readable in a text
editor. **ASCII STL** spells every facet out as `facet normal` / `outer loop` / `vertex` lines —
readable, diffable and hand-editable, but roughly four to five times larger.

This converter goes **both ways**. Binary STL is not text, so paste its bytes as base64 or hex (a
`data:model/stl;base64,…` URL works too) and you get ASCII STL text back. Paste ASCII STL text and
you get a binary STL back as a downloadable `data:model/stl;base64,…` URL, or as raw base64/hex if
you are piping it somewhere else. Leave **Convert to** on *the other encoding* and the direction is
picked for you.

Only the encoding changes. Coordinates are carried as the same 32-bit floats a binary STL stores,
and nothing is welded, repaired, re-ordered, scaled or re-oriented — so the triangle list that goes
in is the triangle list that comes out.

### Worked example — binary in, ASCII out

Paste this base64 (a one-triangle binary STL named `demo`):

```
ZGVtbwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAIA/AAAAAAAAAAAAAAAAAAAgQQAAAAAAAAAAAAAAAAAAIEEAAAAAAAA=
```

and the converter returns the readable form:

```
solid demo
  facet normal 0.000000e+000 0.000000e+000 1.000000e+000
    outer loop
      vertex 0.000000e+000 0.000000e+000 0.000000e+000
      vertex 1.000000e+001 0.000000e+000 0.000000e+000
      vertex 0.000000e+000 1.000000e+001 0.000000e+000
    endloop
  endfacet
endsolid demo
```

Switch **ASCII number format** to *decimal* and the same triangle reads `vertex 0 0 0` /
`vertex 10 0 0` / `vertex 0 10 0` instead. Paste the ASCII form back in and you get the original
134-byte binary file — identical byte for byte, as long as the decimal places are set to 9.

### Limits and things worth knowing

- **Up to 100,000 triangles.** That is a ~5 MB binary STL, or ~6.7 MB of base64 to paste.
- **Binary STL cannot be pasted as text.** It contains raw float bytes and zero bytes, which no
  text field survives — encode it as base64 or hex first.
- **Six decimals is not lossless.** A 32-bit float needs 9 decimals in scientific notation to be
  reproduced exactly. The default 6 matches what most CAD exporters write and is fine for viewing
  or printing; use 9 when you intend to convert back to binary.
- **Per-triangle colour is lost going to ASCII.** Binary STL has 2 spare "attribute byte count"
  bytes per triangle that VisCAM and SolidView use for 15-bit colour; ASCII STL has nowhere to put
  them. Choose *A conversion summary* to see whether your file uses them.
- **Auto-detection does not trust the word `solid`.** Plenty of exporters write "solid" into a
  binary header, so the input encoding is decided by the 84 + 50 × triangle-count byte length
  instead. Force it with **Input encoding** if a file still guesses wrong.
- **Nothing is repaired.** Non-manifold edges, flipped windings, duplicate vertices and holes pass
  straight through untouched.

## FAQ

<details>
<summary>Why does my binary STL start with the word "solid"?</summary>

Because many exporters write a descriptive label into the binary format's 80-byte header, and that
label often begins with "solid". Software that decides the encoding by reading the first five bytes
then misreads the whole file. This tool ignores the keyword and checks the byte length instead — a
binary STL is always exactly `84 + 50 × triangle count` bytes. When writing binary output it also
avoids the trap: a solid name starting with "solid" is written into the header as `STL <name>`.

</details>

<details>
<summary>Will converting to ASCII and back change my model?</summary>

Not if you set the decimal places to 9. STL coordinates are 32-bit floats, and 9 decimals in
scientific notation is the smallest setting that reproduces every 32-bit float exactly, so the
round trip returns a byte-identical file. At the default 6 decimals the values are rounded to about
7 significant digits — invisible for a printed part, but not bit-exact. Facet normals are copied
through untouched unless you ask for them to be recomputed.

</details>

<details>
<summary>Why is my ASCII file so much bigger than the binary one?</summary>

Binary STL spends exactly 50 bytes on a triangle. ASCII STL spends seven lines of text on the same
triangle — around 230–330 bytes depending on the number format and decimal places. A 4× to 6×
increase is normal and does not mean anything was added. Choosing *decimal* number format and
fewer decimal places shrinks the text considerably; converting back to binary always returns it to
50 bytes per triangle.

</details>

<details>
<summary>Can I paste a .stl file straight in?</summary>

An ASCII `.stl` is plain text, so yes — open it in a text editor and paste the whole thing. A
binary `.stl` is not text and cannot be pasted; convert its bytes to base64 or hex first (for
example `base64 -w0 model.stl` or `xxd -p model.stl` on Linux and macOS, or
`certutil -encode model.stl out.txt` on Windows) and paste that. Hex may include spaces, colons or
dashes, and a leading `0x` is ignored.

</details>

<details>
<summary>What do the "attribute bytes" in the summary mean?</summary>

Every triangle in a binary STL ends with a 2-byte "attribute byte count" field that the original
specification says should be zero. VisCAM and SolidView repurposed it to store a 15-bit RGB colour
(5 bits per channel plus a validity bit), and Materialise Magics puts a part-wide `COLOR=` tag in
the 80-byte header instead. The summary reports what your file actually contains. These bytes are
preserved when you re-write binary as binary, but ASCII STL has no equivalent field, so converting
to ASCII drops them.

</details>

<details>
<summary>Does this repair, scale or re-orient the mesh?</summary>

No, and that is deliberate — a format conversion that quietly changes geometry is impossible to
trust. Triangles come out in the same order with the same coordinates. The only optional edits are
cosmetic: renaming the solid, and choosing whether facet normals are kept, recomputed from each
triangle's winding, or zeroed. For welding, hole filling, winding fixes or unit scaling, use a
dedicated repair or mesh-conversion tool.

</details>
