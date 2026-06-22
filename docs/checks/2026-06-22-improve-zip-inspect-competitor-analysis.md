# zip-inspect — competitor analysis & improvement check (2026-06-22)

## Tool
`zip-inspect` — lists a `.zip` archive's contents (entry names, uncompressed/compressed
sizes, compression method, CRC-32, compression ratio, directory + encrypted flags,
modification time) and archive-wide totals, **without extracting or decompressing
anything**. Reads only the ZIP central directory. File-input → JSON, no standalone page
(same no-page pattern as `unzip` / `extract-tar` / `detect-file-type`). Surfaces: chat + CLI.

## Distinct from existing blocks
- `blocks/unzip` **extracts** files and returns each entry's inline content (text/base64).
  `zip-inspect` deliberately does NOT extract — it is a metadata/listing tool, and it
  reports fields `unzip` does not (compression method, compressed size, CRC-32, ratio,
  encrypted flag). Because it never decompresses, it also works on archives using methods
  the deflate-only build can't decode.
- `blocks/identify-archive-format` only detects the container type, not the entry listing.

## Competitors surveyed (top online "ZIP viewer / list contents" tools)
1. ConvertICO ZIP File Viewer — browse contents, navigate folders, preview, extract
   individual files, open password-protected ZIPs.
2. Encode64 ZIP Contents Extractor — list contents without full extraction; **tree / JSON /
   CSV output**; archive review / package check workflow.
3. FreeFileViewers Online ZIP Viewer — drag-drop preview of the file listing.
4. Tarpanda — browse/preview files inside a ZIP: **names, sizes, dates, folder structure**.
5. PhotoAppWorld Online ZIP Viewer — view contents, browse folders, download individual files.
6. ezyZip — content previewer for images/audio/video/docs inside the ZIP; multipart ZIP.

## Capability diff (competitor feature → our coverage)
| Capability | Competitors | zip-inspect | Notes |
|---|---|---|---|
| List entry names | all | ✅ | `name` (full path) |
| Uncompressed size | all | ✅ | `size` |
| Compressed size | few | ✅ | `compressed_size` (most viewers omit) |
| Compression method | rare | ✅ | `method` (Stored/Deflated/Bzip2/…); read from central dir even for un-decodable methods |
| CRC-32 checksum | rare | ✅ | `crc32` (8-char hex) |
| Compression ratio | rare | ✅ | per-entry `ratio_pct` + `total_ratio_pct` |
| Directory vs file | Tarpanda et al. | ✅ | `is_dir`, plus `file_count`/`dir_count` |
| Modification date | Tarpanda et al. | ✅ | `modified` (when recorded) |
| Encrypted/password flag | ConvertICO | ✅ | `encrypted` flag per entry (we flag, do not crack) |
| Archive totals (count/size) | some | ✅ | `count`, `total_size`, `total_compressed_size` |
| Folder-tree rendering | several | n/a | we return flat JSON with full paths; the consumer (LLM/CLI/page) renders a tree — JSON is the structured superset of tree/CSV |
| Preview individual file content | several | out of scope | that is extraction → already covered by `blocks/unzip` |
| Download individual file | several | out of scope | extraction → `blocks/unzip` |
| Decrypt / open password ZIP | ConvertICO | out of model | needs the AES feature (not in the deflate-only wasm-safe build); we only flag `encrypted` |
| Multipart ZIP (.z01/.z02) | ezyZip | out of model | single-file input; multi-part spanning needs >1 upload |

## Gaps closed in this build
The inspection scope already exceeds the typical viewer: most competitors only list names +
sizes (+ dates). We additionally report **compressed size, compression method, CRC-32,
per-entry + overall compression ratio, encrypted flag, and file/dir counts** — the columns a
package-audit / archive-review user actually wants. No additional in-model capability gap
remained to close after the first implementation.

## Out-of-model (intentionally NOT built)
- Inline preview / download of individual files = extraction → `blocks/unzip` already does this.
- Password-protected ZIP decryption — the AES decode path is excluded from the wasm-safe
  deflate-only build; `zip-inspect` only reports the `encrypted` flag.
- Multipart/spanned ZIP merging — single-file input model.
- Folder-tree / CSV rendering — left to the consuming surface; the JSON listing is the
  structured superset.

## Verification
- Unit tests (core): list with metadata (method/size/CRC/ratio/dir), error on non-zip,
  ratio edge cases — pass.
- Drift-guard schema test (block): authored chat schema == derived `schema_json()` — pass.
- `wafer build`: block.wasm validates & instantiates (588.1 KiB).
- CLI: `gizza tool zip-inspect url=https://codeload.github.com/octocat/Hello-World/zip/refs/heads/master`
  → correct JSON listing (1 file + 1 dir, sizes, method=Stored, CRC, modification time, totals).
- No page surface (file-input → JSON pattern), stated explicitly.

## Sources
- https://convertico.com/zip-file-viewer/
- https://encode64.com/en/utilities/zip-contents-extractor
- https://freefileviewers.com/zip/
- https://tarpanda.com/learn/view-zip-contents-without-extracting/
- https://photoappworld.com/en/online-tools/zip-viewer
- https://www.ezyzip.com/extract-multipart-zip-online.html
