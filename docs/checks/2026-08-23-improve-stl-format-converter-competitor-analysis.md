# stl-format-converter competitor analysis (2026-08-23)

Backlog row: `stl-format-converter` — converts an STL mesh between binary and ASCII encodings in either direction.

Research query: `online STL binary ASCII converter STL format convert binary to ascii STL tool`.

## Competitor scan

| Competitor | Observed table stakes | Fit decision |
|---|---|---|
| JAD Apps binary-to-ASCII STL converter | Browser/local conversion focused on binary STL bytes into readable ASCII text; pitches diff/debug workflows and no-upload handling. | In model. The descriptor/page support binary STL input as base64/hex/data URL and ASCII STL output, local WASM execution, diffable text, and explicit binary-header handling. |
| MeshConvert online mesh converter | General mesh format conversion, including STL among broader OBJ/PLY/COLLADA-style workflows; usually file-upload oriented and format-changing rather than only encoding-changing. | Partly out of model. Cross-format mesh conversion belongs to `mesh-convert`; this tool intentionally preserves geometry and only flips STL's binary/ASCII encoding. In-model gap addressed by documenting no scaling/repair/reordering. |
| A23D STL viewer / converter surface | Accepts both binary and ASCII STL, reports counts/dimensions, can export to other 3D formats, and discusses viewer/repair/material concerns. | Partly out of model. Viewing, unit conversion, repair, materials and export-to-OBJ/GLB are not this encoding converter. In-model pieces are auto-detection of both STL encodings, triangle-count reporting via summary mode, and clear malformed-file errors. |
| ConvertICO STL viewer/converter snippet | Browser-local STL viewer/converter messaging: binary and ASCII detection, friendly broken-file errors, privacy/local execution, and OBJ export. | Partly out of model. OBJ export/viewing is outside scope, but local execution, binary/ASCII auto-detect and actionable errors are in model and implemented. |
| vancha Binary-stl-to-ascii-stl utility | One-way binary STL to ASCII conversion for editing/readability. | In model and exceeded: supports binary-to-ASCII plus ASCII-to-binary, forced rewrite, normals and precision controls. |

## Controls and defaults to carry into the tool

| Capability / UX pattern | Default / control | In model? | Implementation decision |
|---|---:|---|---|
| Binary STL to ASCII STL | `to=auto`, `input_format=auto` | Yes | Auto detects binary bytes encoded as base64/hex/data URL and flips to ASCII. |
| ASCII STL to binary STL | `to=auto` | Yes | ASCII text flips to binary output. |
| Force direction | `to=ascii|binary|auto` enum | Yes | Included as `Param::enumv` and page select. |
| Binary pasted as text-safe bytes | `input_format=auto|ascii|base64|hex` enum | Yes | Included; hex accepts separators, base64 accepts data URL. |
| Binary output cannot be raw terminal text | `output_encoding=data-url|base64|hex` enum | Yes | Included; default data URL is page/download-friendly. |
| Preserve or repair normals | `normals=keep|recompute|zero` enum | Yes | Included; default `keep` preserves the source exactly. |
| Readable vs round-trip ASCII numbers | `number_format=scientific|decimal`, `precision` slider | Yes | Included; docs call out precision 9 for f32 round-trip safety. |
| Rename solid/header | `solid_name` text field | Yes | Included; binary header avoids starting with `solid`. |
| Summary-only report | `output=stl|summary` enum | Yes | Included for counts, sizes, color attribute warnings, and encoding diagnostics. |
| Mesh viewing / thumbnails | viewer canvas | No | Not built; belongs to STL viewer tools. |
| Cross-format exports (OBJ/PLY/GLB) | output format picker | No | Not built; belongs to `mesh-convert`. |
| Mesh repair / watertight checks | repair controls | No | Not built; belongs to `stl-repair`/mesh QA tools. |
| Upload/file picker UX | file input | Partly | Current gizza model is text/field based; page copy explains paste base64/hex or ASCII. |

## Worked examples selected

- Binary STL base64 to ASCII text, exercising the primary competitor surface.
- ASCII triangle to binary `data:model/stl;base64` output.
- Decimal-number output for readable diffs.
- Precision 9 round-trip example for lossless binary -> ASCII -> binary workflows.
- Summary mode so users can inspect triangle count, sizes and color attributes without dumping the full mesh.

## Notes

This tool is intentionally not a semantic duplicate of `mesh-convert` or `stl-repair`: it does not change mesh file type or topology. It preserves STL triangles and only rewrites the encoding and presentation details that are specific to STL's binary/ASCII split.
