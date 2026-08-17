# NPY array decoder competitor analysis (2026-08-17)

Tool: `npy-array-decoder` — decode a NumPy `.npy` binary array into metadata plus JSON/CSV values.

## Sources skimmed

- Perchance Online NPY Viewer — browser/local `.npy` inspector backed by Pyodide; emphasizes file loading, dtype/shape display, and local processing.
- Convert.guru `.NPY Converter` — online converter for NumPy arrays to CSV, JSON, or text; table-stakes are conversion formats and preserving numeric values.
- VS Code NPZ Viewer extension — editor-side viewer for `.npy`/`.npz` variables; table-stakes are shape/dtype metadata, array previews, and archive/member browsing.
- Desktop/GitHub NPY viewer projects — lightweight local viewers for 1D/2D arrays, often adding heatmap/table visualization for matrices.
- NumPy format documentation and examples — authoritative format behavior for magic/version/header fields, dtype descriptors, shape tuples, and Fortran order.

## Table-stakes capabilities and decisions

| Capability / UX pattern | Competitor expectation | Fit for this repo/model | Decision in this tool |
| --- | --- | --- | --- |
| Accept a real `.npy` file | File upload/dropzone is common for browser viewers | Partial fit: this generator's stable model is text/URL inputs, not binary upload controls | Accept base64, hex, and `data:` URI text; page docs give OS commands to encode files |
| Show dtype and shape before values | Every serious viewer/converter exposes dtype + shape | In-model | `summary`, `json`, and `header` include raw `descr`, friendly dtype name, item size, byte order, shape, rank, element count |
| Convert to JSON | Conversion sites advertise JSON export | In-model | `output=json` emits metadata plus nested `data`, with truncation flags |
| Convert to CSV | Conversion sites advertise CSV/text export | In-model | `output=csv` writes one row per last-axis slice; delimiter parameter supports comma, tab, and single-character separators |
| Header-only mode for large arrays | Header inspection without materializing huge values is common in Python snippets/tools | In-model | `output=header` returns metadata without data values |
| Version support | NumPy `.npy` v1/v2/v3 are documented | In-model | Parser accepts 1.0, 2.0, 3.0 and rejects other versions clearly |
| Dtype breadth | Users expect common numeric arrays to decode; object/record arrays are hard/sensitive | Mostly in-model | Supports bool, signed/unsigned ints, float16/32/64, complex64/128, fixed bytes, fixed Unicode; rejects object, record, datetime/timedelta and platform-specific long doubles with reasons |
| Endianness and Fortran order | Format stores byte order and memory order | In-model | Multi-byte values honor endian markers; Fortran-ordered arrays are re-indexed to row-major for display |
| Large array safeguards | Browser tools need memory caps and preview limits | In-model | 8 MiB decoded input cap; default 1000 rendered values, max 100000 |
| Matrix/heatmap visualization | Desktop and editor viewers often show 2D heatmaps/tables | Out-of-model for this repo's generic text-output page | Not built; output is textual JSON/CSV/summary |
| `.npz` archive member browsing | Some editor/converter tools support `.npz` | Out-of-model for this tool as scoped to one `.npy` stream; repo already has ZIP-related utilities | Rejects `.npz` magic as not `.npy` and documents unzipping first |
| Python/Pyodide execution | Some browser viewers use Pyodide/NumPy directly | Out-of-model by current gizza pure-Rust/wasm preference | Implemented std-only Rust parser; no Python, no NumPy, no pickle execution |

## Worked examples retained from scan

- A small `2x3` float64 array is the primary preset because it exercises dtype, shape, nested JSON and CSV row layout.
- A tiny uint8 vector in hex is included to make the input-encoding switch visible and to demonstrate header-only output.
- For docs, base64 shell snippets are called out because users normally start with a local binary `.npy` file rather than a paste-ready string.

## Gaps intentionally not closed

- No drag-and-drop binary file control until the generic page generator has a first-class binary/file input pattern.
- No heatmap/table visualization; this repo's tool pages are generic, deterministic text transforms.
- No unsafe pickle/object decoding; that would require executing untrusted Python pickle data.
- No `.npz` archive browsing in this tool; users should extract a member first or use a dedicated archive tool.
