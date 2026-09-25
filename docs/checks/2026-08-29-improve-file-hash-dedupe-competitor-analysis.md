# file-hash-dedupe — competitor analysis (2026-08-29)

Scan run **before** implementation, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All findings are paraphrased from public product/feature pages — **no competitor copy,
branding or trademarks are reproduced in the block, its descriptor, or its docs**.

## Tool under build

`file-hash-dedupe` — content-addresses a set of files by hash and reports identical
duplicates regardless of filename.

## Top 3 competitors reviewed

| # | Tool | Kind | What it does |
|---|------|------|--------------|
| 1 | FolderManifest "Find Duplicates" (<https://www.foldermanifest.com/free-tools/find-duplicates>) | Browser tool | Upload files, SHA-256 each, group identical content regardless of filename; reports filename, size, hash grouping and wasted space. Hard caps: **10 files, 10 MB each**. In-memory only, no deletion, no folder scan. |
| 2 | PeaZip duplicate finder (<https://peazip.github.io/duplicates-hash-checksum.html>) | Desktop | Hash/checksum column that groups identical files; wide algorithm menu (Adler32, CRC16/24/32/64, MD4, MD5, RIPEMD-160, SHA-1, SHA-256/512, SHA3-256/512, BLAKE2S/2B, Whirlpool); optional byte-to-byte comparison reporting the differing bytes; reports size + hash for duplicates only, plus a count of non-unique files. |
| 3 | dupeGuru (<https://dupeguru.org/>) | Desktop, open source | Content-based scan modes (filename/string, MD5 content hash, picture block-RGB). Staged detection: group by exact size → hash a chunk → full-content hash (sometimes byte-by-byte) to confirm. Music mode reads audio tags; picture mode does perceptual matching. Presents duplicate groups with a reference ("keep") file and marks the rest. |

Also sighted for context (not one of the three deep reads): Hash File Online
(<https://hash-file.online/>, client-side MD5/SHA-1/SHA-256/SHA-512 with duplicate
detection) and Duplicate File Detective (Windows; CRC32/ADLER32/MD5/SHA1/SHA256/SHA512
plus optional byte-for-byte verification).

## Table-stakes → in-model / out-of-model

Every table-stake below lands in the descriptor or is listed as out-of-model. Nothing is
dropped silently.

### In-model (built into this tool)

| # | Table-stake | Seen in | Where it lands |
|---|-------------|---------|----------------|
| 1 | Match by **content, not filename** | 1, 2, 3 | Core: files are grouped by digest, `source` labels are report-only. |
| 2 | **Choice of hash algorithm** | 2, 3, DFD | `algorithm` enum: `sha256` (default), `sha1`, `md5`, `sha512`, `blake3`, `crc32`. |
| 3 | Per-file report row: name, size, hash | 1, 2 | `files[]` = `{index, source, bytes, hash, group}`. |
| 4 | **Duplicate groups** with a keep/delete suggestion | 1, 3 | `groups[]` = `{hash, bytes, members, keep, delete, count}`. |
| 5 | **Which copy to keep** is user-controllable (dupeGuru's reference-file rules) | 3 | `keep` enum: `first` (default), `last`, `shortest-name`. |
| 6 | "Show duplicates only" vs list everything | 2 ("hash only for duplicate files") | `include_unique` boolean, default `false`. |
| 7 | **Wasted-space / reclaimable bytes** roll-up | 1 | `summary.bytes_reclaimable` + `total_bytes`, `unique_files`, `duplicate_files`, `duplicate_groups`. |
| 8 | Confirm a hash match is a **real** content match, not a collision | 2, 3 (byte-to-byte / staged confirm) | Every file also gets an internal SHA-256 confirmation digest and its size; two files are only grouped when size **and** SHA-256 agree. A `algorithm`-digest match that fails confirmation is counted in `summary.hash_collisions` and left ungrouped. This is what makes the weak `crc32`/`md5` choices safe to offer. |
| 9 | Size-first staging (cheap pre-filter) | 3 | Grouping key is `(bytes, confirm-digest)` — same effect, and exact rather than heuristic. |
| 10 | Works on **any file type** | 1, 2 | `AssetKind::Any` — no MIME gate; documents, archives, media, binaries all accepted. |

### Out-of-model (listed, deliberately not built)

| # | Feature | Seen in | Why it is out of model |
|---|---------|---------|------------------------|
| A | Recursive **folder/disk scan** | 2, 3 | A wasm block has no filesystem; inputs arrive as URL/attachment sources one at a time. Users pass an explicit list. |
| B | **Deleting / moving / hardlinking** duplicates | 2, 3 | This toolkit's blocks are read-only and side-effect free. The tool reports a suggested delete list; acting on it is the caller's job. |
| C | **Byte-to-byte diff** reporting the exact differing offsets | 2 | Would require holding every input's full bytes simultaneously; the runtime budget is ~64 MiB. Collision *detection* (row 8) gives the safety property without the memory cost; the differing-offsets report does not fit. |
| D | **Perceptual / near-duplicate** image matching | 2, 3 (picture mode) | Already shipped as a separate block: `blocks/duplicate-image-finder` (dHash + similarity threshold). This tool is deliberately exact-match only. |
| E | **Music mode** (match by audio tags across bitrates) | 3 | Tag-based fuzzy matching, not content-addressing — a different tool shape. |
| F | Thumbnails / visual inspection UI | 2 | No page surface (see below); chat + CLI return JSON. |
| G | Adler32 / CRC16 / CRC24 / CRC64 / MD4 / RIPEMD / SHA3 / BLAKE2 / Whirlpool | 2 | The six offered algorithms cover every realistic dedupe use; the long tail adds enum surface with no dedupe benefit. `blocks/hash-all` already exposes the wide algorithm menu for a single input. |
| H | Chunk-hash prefilter for very large sets | 3 | An optimization for millions of local files. Here the input set is bounded (50 files, 32 MiB each) and each file is hashed once while streamed, so there is nothing to optimize away. |

## Surface decision — chat + CLI, **no standalone page**

The input is an **array of sources** (`Param::source_list("files", 2)` with `Input::None`),
which the page generator cannot represent: `page/meta.toml` `[[input]]` fields are scalar
controls plus a single-file upload, and the output is a JSON report rather than text or
media. This is the same shape and the same conclusion as the already-shipped
`blocks/duplicate-image-finder` (`Param::source_list("images", 2)`, `Input::None`, chat +
CLI, no page).

So this tool ships **chat + CLI only, with no `page/` and no `web/` crate**, and there is
**no Playwright spec** — there is no page to drive. That is an honest statement of the
surface, not a skipped check. The verification that does apply (workspace tests, the
pinned `wasm32-wasip1` block build, `gizza tool` CLI runs including an exact-output case
and the full advertised-values matrix, manifest sync, hygiene gate) is all run.

## Non-duplicate confirmation

Checked against existing blocks before building:

- `blocks/file-hash` — digests of **one** file (MD5/SHA-1/SHA-256/SHA-512/CRC-32). No set, no grouping.
- `blocks/hash-all` — every digest of **one** input. No set, no grouping.
- `blocks/duplicate-image-finder` — **images only**, perceptual dHash + similarity threshold. Different algorithm, different input class.
- `blocks/csv-dedupe`, `fuzzy-dedupe`, `find-duplicate-lines`, `remove-duplicate-lines`, `json-dedupe-array`, `jsonl-deduplicator` — dedupe **rows/lines/records inside one text document**, not files against each other.

`file-hash-dedupe` is the only block that content-addresses a **set of arbitrary files**
and reports byte-identical duplicates across them. Not a duplicate.

## Sources

- [FolderManifest — Find Duplicate Files Online](https://www.foldermanifest.com/free-tools/find-duplicates)
- [PeaZip — duplicate files finder by hash/checksum](https://peazip.github.io/duplicates-hash-checksum.html)
- [dupeGuru](https://dupeguru.org/)
- [Hash File Online](https://hash-file.online/)
- [Duplicate File Detective — features](https://www.duplicatedetective.com/features)
