# matlab-mat-to-csv — competitor analysis (2026-09-23)

> **Outcome: the tool was NOT built.** This scan is kept as research only. The decision below to
> treat v7.3 (HDF5) as out-of-model was reversed on review: a `.mat` → CSV converter that reads only
> the Level-5 container would silently miss a major current MAT format, and this repo has no
> wasm-safe pure-Rust HDF5 reader to cover the other half. The slug is on
> `docs/tool-skiplist.txt`; see that line for the full reasoning. Note that the two closest
> analogues below (`scipy.io.loadmat`, `mat4js`) stop at exactly the same wall.

Scan run BEFORE implementing, per `create-next-tool` step 4. Paraphrased observations only — no
competitor copy, branding, or trademarks are reproduced here or in the shipped page.

## Search

One web search: *"convert MATLAB .mat file to CSV online tool"*, plus one follow-up sweep for
in-browser `.mat` readers. Four reachable, genuinely comparable tools were skimmed (no
substitutions needed — all four responded):

1. **convert.guru "MAT Converter"** — browser drag-and-drop `.mat` → CSV / JSON / H5.
2. **`mat2csv`** (MATLAB Central File Exchange / GitHub `TheHotChilli/mat2csv`) — the most-linked
   script answer to this exact question.
3. **`scipy.io.loadmat`** (SciPy reference manual) — the de-facto non-MATLAB reader that every
   Python "mat to csv" recipe is built on; the richest option surface of the four.
4. **`mat4js`** (`KovacsGG/mat4js`, npm `mat-for-js`) — the closest technical analogue: a pure-JS
   in-browser Level-5 MAT reader, i.e. exactly the shape of our page.

## What each exposes

| Tool | Settings exposed | Defaults | Limits / notes |
| --- | --- | --- | --- |
| convert.guru | file picker + a single "convert file to…" target (CSV / JSON / H5) | n/a | No variable picker, no delimiter, no precision, no transpose. No stated size cap. Claims conversion happens in the browser. MAT version support is not documented; in practice it behaves more like a preview/analyse step than a full converter |
| mat2csv | one argument: the `.mat` path | overwrites `<name>.csv` beside the input | Requires every field to be **vector-shaped**; converts through cell arrays, which the author notes is slow on large files. Multiple variables, structs, cells, tables, N-D arrays and complex numbers are all undocumented/unsupported |
| scipy.io.loadmat | `variable_names`, `byte_order`, `mat_dtype`, `squeeze_me`, `chars_as_strings`, `matlab_compatible`, `struct_as_record`, `verify_compressed_data_integrity`, `simplify_cells`, `uint16_codec`, `spmatrix`, `appendmat`, `mdict` | `variable_names=None` (all), `squeeze_me=False`, `struct_as_record=True`, `chars_as_strings=False`, `simplify_cells=False`, `verify_compressed_data_integrity=True` | Supports **v4 (Level 1.0), v6, and v7–7.2**; explicitly does **not** read **v7.3** (HDF5) without a separate HDF5 library. Returns a dict keyed by variable name plus the metadata keys `__header__`, `__version__`, `__globals__`. Sparse matrices come back as SciPy sparse. CSV writing is left to the caller |
| mat4js | `mat4js.read(ArrayBuffer)` — no options | n/a | **Level 5 only**; raises a feature error on v7.3/HDF5. Handles numeric arrays (int64/uint64 as BigInt), char arrays, cell arrays, scalar and non-scalar struct arrays, sparse (as `{x, y, nz}`), and complex as `{r, i}` objects. No object arrays, no write support. Auto-flattens 2-D vectors to 1-D |

## Table stakes → decision

Every table-stake below ends in the descriptor or in the out-of-model list. Nothing dropped silently.

| Table stake | Seen on | Fit | Decision |
| --- | --- | --- | --- |
| Read the classic Level-5 MAT container (v6 / v7, incl. the zlib-compressed elements v7 writes) | all 4 | in-model | Hand-rolled Level-5 reader; `miCOMPRESSED` elements inflate through the already-proven `flate2` (miniz_oxide) backend |
| Read MAT v4 (Level 1.0) too | scipy | in-model | Supported: the 20-byte MOPT header path, both byte orders, numeric + text + sparse `T` codes |
| Reject v7.3 (HDF5) with a real explanation rather than a parse crash | scipy, mat4js (both state it) | in-model | HDF5 magic (`\x89HDF\r\n\x1a\n`) is sniffed first and produces a named error that tells the user to re-save with `save(..., '-v7')` |
| Byte-order handling (`IM` / `MI`, and v4's MOPT `M` digit) | scipy (`byte_order`) | in-model | Detected from the file itself — no user-facing knob is needed, which is strictly better than scipy's manual override |
| Select which variable(s) to export | scipy (`variable_names`) | in-model | `variable` param: blank = every exportable variable, otherwise a comma-separated name list, order preserved |
| List what is inside the file before converting (a `whos`-style view) | convert.guru (preview), scipy (dict keys) | in-model | `output = "list"` (JSON metadata only) and `output = "summary"` (readable report) |
| Emit CSV | all 4 (the whole point) | in-model | `output = "csv"` is the default |
| Emit JSON as an alternative target | convert.guru | in-model | `output = "json"` — file metadata plus nested row arrays |
| Configurable delimiter | none of the four expose one; universal in CSV tooling and in our sibling blocks | in-model | `delimiter` param: any single ASCII character or the word `tab` |
| Column header row | MATLAB's own `writetable` writes one; `writematrix`/`csvwrite` do not | in-model | `header` checkbox, default **off** (matches the numeric-matrix default the competitors produce); on → `col1…colN` |
| Numeric precision control | `csvwrite` historically wrote 5 significant digits; `writematrix` writes full precision | in-model | `precision` integer, `0` = shortest round-trip representation (default), `1–17` = significant digits |
| Complex numbers | mat4js (`{r, i}`), scipy (native complex) | in-model | Rendered in one cell as `a+bi` / `a-bi`; flagged in `list`/`summary` and in the page copy |
| Char arrays | mat4js, scipy (`chars_as_strings`) | in-model | Exported as one CSV row per MATLAB char-matrix row, RFC-4180 quoted |
| Logical arrays | scipy, mat4js | in-model | Exported as `0`/`1`, class reported as `logical` |
| Sparse matrices | scipy, mat4js | in-model | Densified to a full grid (subject to the row cap), so the CSV matches what MATLAB shows |
| N-D (3-D+) arrays | mat4js flattens vectors; nobody documents N-D | in-model | Reshaped column-major to `dim1 × (product of the rest)`, i.e. exactly MATLAB's `reshape(A, size(A,1), [])`. Documented on the page |
| MATLAB's column-major storage vs row-major CSV | implicit everywhere | in-model | Handled: elements are de-interleaved to row-major on the way out. `transpose` checkbox additionally swaps rows/columns |
| Row cap / preview for huge variables | mat2csv warns about slowness | in-model | `limit` = max data rows per variable (default 1000, max 100000), with an explicit truncation note |
| Structs / cell arrays / object arrays | mat4js reads cells + structs; mat2csv converts through cells | **out-of-model** | These are trees, not tables — there is no single honest CSV shape for them. They are *listed* (name, class, size) in `list`/`summary` and named in a clear error if requested explicitly. Documented as a limit on the page |
| MATLAB `table` / `timetable` objects | mat2csv, MATLAB's own `writetable` | **out-of-model** | These serialise as `mxOBJECT`/subsystem-referenced data whose layout is undocumented; reading them properly needs the MAT subsystem block. Listed, never silently mangled |
| v7.3 / HDF5 files | scipy (via h5py), MATLAB | **out-of-model** | A full HDF5 reader is a separate tool-sized problem. Detected and named, with the `-v7` re-save instruction |
| Writing `.mat` files | mat4js states it has no write support either | **out-of-model** | This tool is one-directional by name |
| Batch / many-file conversion | mat2csv (script loop), convert.guru (one at a time) | **out-of-model** | One file per run; the CLI is loopable in a shell |
| 100 MB+ inputs | server-side converters | **out-of-model** | The block runs in a 64 MiB wasm sandbox: 8 MiB of decoded input, 32 MiB after inflation, stated plainly on the page |

## UX control patterns adopted

- A multiline textarea for the file bytes (base64 or hex) with an `auto` encoding detector, matching
  the sibling `npy-array-decoder` page so the two feel like one toolkit. None of the competitors
  offer a paste path at all — they all require a file on disk.
- `<select>` dropdowns for the two fixed-choice params (`input_format`, `output`), rendered from the
  descriptor enums.
- Two checkboxes (`header`, `transpose`), both default **off** so the zero-interaction result equals
  what MATLAB's own `writematrix` would produce.
- `[[example]]` preset chips — one per common task (export everything, pick one variable as
  tab-separated, inspect the file without dumping data). No competitor ships presets; the generator
  supports them, so this is a gap in our favour.

## Gaps we intentionally do not close

Structs, cell arrays, object/`table` classes, v7.3 (HDF5), `.mat` writing, batch mode, and
100 MB-scale inputs. All listed above with reasons; none are silently dropped, and none are hinted
at in the page copy.
