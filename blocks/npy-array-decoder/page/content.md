## About this tool

`.npy` is NumPy's own on-disk format for a single array: a short magic number, a
version, a Python-dict header holding the `descr` (dtype), `fortran_order` and
`shape`, then the raw element bytes with no compression and no padding. It is
trivial for NumPy to read and completely opaque to everything else — open one in
a text editor and you get a line of readable header followed by binary noise.

This decoder reads the format directly. Paste the file's bytes as base64 or hex
and it reports the dtype, the shape, whether the data is stored row-major (C) or
column-major (Fortran), where the data starts, and how many bytes it occupies —
then renders the values as a readable report, as JSON, or as CSV you can drop
into a spreadsheet. Nothing is uploaded: the parser is compiled to WebAssembly
and runs in this page.

To get the bytes out of a file, base64-encode it — `base64 -w0 array.npy` on
Linux, `base64 -i array.npy` on macOS, or
`certutil -encode array.npy out.txt` on Windows — then paste the result. Hex
works too (`xxd -p array.npy`), and **Input encoding: auto** tells the two apart
on its own.

### Worked example

A `2x3` array of doubles saved with `numpy.save` is 176 bytes: a 10-byte
prologue, a 118-byte header padded out to a 64-byte boundary, then `6 x 8 = 48`
bytes of data. Pasting its base64 with **Output: summary** gives:

```text
NumPy .npy file, format version 1.0
dtype:    float64 (descr <f8, 8 bytes per element, little-endian)
shape:    (2, 3) - 2 dimensions, 6 elements
order:    C (row-major)
layout:   header 118 bytes, data starts at offset 128, data 48 bytes
values:   all 6 elements
[[1, 2, 3.5], [4, 5, 6]]
```

Switch **Output** to `csv` and the same file becomes `1,2,3.5` / `4,5,6`; switch
it to `json` and you get the metadata plus a nested `data` array; switch it to
`header` and you get the metadata alone, with no values — useful when you only
want to know what is in a large file.

### What it reads

- **Format versions** 1.0, 2.0 and 3.0 (the versions differ in the width of the
  header-length field and the header's text encoding).
- **dtypes** — `bool`, `int8/16/32/64`, `uint8/16/32/64`, `float16/32/64`,
  `complex64/128`, fixed-width bytes (`S<n>`) and fixed-width text (`U<n>`),
  little-endian or big-endian. `float16` is widened by hand, so half-precision
  arrays decode exactly, subnormals included.
- **Both memory orders** — Fortran-ordered (column-major) data is re-indexed to
  row-major before it is printed, so the values always read in the same order
  NumPy would print them.
- **Any rank** — 0-d scalars, vectors, matrices and higher-rank arrays; empty
  arrays (a zero in the shape) are reported rather than treated as an error.

### Limits and edge cases

- **8 MiB** of decoded file bytes maximum — every element is materialised in
  memory before rendering.
- **1000 values** are rendered by default; raise **Max values rendered** up to
  100000. Past that cap the array is truncated in row-major order, `json` and
  `summary` switch to a flat list and set `truncated: true`, and `csv` emits
  whole rows only.
- **Not supported, by design:** object (`O`) arrays, which hold pickled Python
  objects and cannot be read without executing code; structured/record dtypes,
  whose `descr` is a list of named fields; `datetime64`/`timedelta64`; and
  extended-precision (`float96`/`float128`) values, which are platform specific.
  Each is rejected with a message that says which one it hit.
- **`.npz` files are ZIP archives** of `.npy` members, not `.npy` files — unzip
  first and decode a member.
- Complex numbers and non-finite floats (`NaN`, `Infinity`) have no JSON
  literal, so in `json` output they are emitted as strings; the JSON always
  parses.

## FAQ

<details>
<summary>How do I turn my .npy file into something I can paste here?</summary>

Base64-encode it. On Linux `base64 -w0 array.npy`, on macOS `base64 -i array.npy`,
on Windows `certutil -encode array.npy out.txt` (then strip the BEGIN/END lines),
or in Python `import base64, pathlib; print(base64.b64encode(pathlib.Path("array.npy").read_bytes()).decode())`.
Hex from `xxd -p array.npy` works just as well. Leave **Input encoding** on
`auto` and the tool works out which one you pasted from the file's own magic
bytes — a `.npy` file always begins with byte `0x93`, so hex starts `93` and
base64 starts `k05VTVBZ`. A `data:application/octet-stream;base64,` prefix is
accepted and ignored.

</details>

<details>
<summary>Why does it refuse my file with "object arrays hold pickled Python objects"?</summary>

The array was saved with `dtype=object` — the elements are not numbers but
pickled Python objects, and the only way to turn them back into values is to run
the pickle, which can execute arbitrary code. No decoder should do that to a file
you pasted from somewhere else. Re-save the array with a concrete numeric dtype
(`arr.astype("float64")`, say) and it will decode. The same reasoning applies to
structured/record dtypes: their `descr` is a list of named fields rather than one
dtype, so save the fields as separate arrays first.

</details>

<details>
<summary>What does "truncated .npy data" mean?</summary>

The header and the data disagree. The header declares a dtype and a shape, which
together fix exactly how many bytes must follow — `2 x 3` float64 values need
48 — and fewer bytes than that are present. The error prints both numbers so you
can see the size of the shortfall. In practice it means the paste was cut off,
the file was copied while it was still being written, or only part of a larger
file was captured. Extra bytes *after* the array are harmless: they are reported
as unused trailing bytes rather than treated as an error.

</details>

<details>
<summary>Why is my Fortran-ordered array printed in a different order than the file?</summary>

Because the file stores it column-major and this tool prints row-major, which is
how NumPy itself displays an array regardless of how it is laid out in memory. A
`2x3` array stored as `1 4 2 5 3 6` on disk is shown as `[[1, 2, 3], [4, 5, 6]]`.
The `summary` and `header` outputs both state `fortran_order`, so you can always
see how the bytes were actually written.

</details>

<details>
<summary>How does CSV output map an array with more than two dimensions?</summary>

The last axis becomes the columns and every earlier axis is folded into the rows,
so a `2x3x4` array comes out as 6 rows of 4 values, in row-major order. A 1-d
array is written one value per line, matching `numpy.savetxt`, and a 0-d scalar
is a single value. There is no header row — the array has no column names — and
fields containing the delimiter, a quote or a newline are quoted the RFC 4180
way. Set **CSV delimiter** to `;`, `|`, `tab` or any single character.

</details>
