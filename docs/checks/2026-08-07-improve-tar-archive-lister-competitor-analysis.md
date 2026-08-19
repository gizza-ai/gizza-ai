# tar-archive-lister competitor analysis (2026-08-07)

## Scope

Tool: `tar-archive-lister` — enumerate tar/tar.gz archive members with paths, sizes, modes, owners, types, mtimes and link targets without unpacking file contents.

## Sources checked

- GNU/BSD `tar` long-listing patterns (`tar -tf`, `tar -tvf`, `tar -tzvf`) as documented in command-line help/articles.
- nixCraft guide for listing tar/tar.gz contents without extracting.
- Baeldung guide for viewing `.tar.gz` contents without extracting.
- Ask Ubuntu / Unix Stack Exchange examples for `tar -tvf archive.tgz`, `gunzip -c ... | tar tvf -`, and grepping/filtering archive listings.

## Table-stakes capabilities

| Capability | Seen in competitors | In model? | Decision |
| --- | --- | --- | --- |
| List archive paths without extraction | `tar -tf` / `tar -tzf` examples | Yes | `output=paths` emits one path per line and the default table also includes paths. |
| Verbose table with modes, owner/group, size, mtime and links | `tar -tvf` style listings | Yes | `output=table` mirrors long-listing fields, including symlink/hardlink targets. |
| Support gzip-compressed tarballs | `tar -tzf`, `.tgz` examples | Yes | gzip magic bytes are auto-detected after base64/hex decode. |
| Support plain tar | All `tar -tf archive.tar` examples | Yes | Plain tar is the fallback container. |
| Filter or search paths | CLI examples pipe through `grep` | Yes | `filter` supports substring and simple `*`/`?` globs inside the engine. |
| Sort output | GUI/archive viewers often sort by name/size/date | Yes | `sort` enum supports archive order, path, size, mtime and type. |
| Machine-readable formats | Automation commonly needs parseable output | Yes | `csv` and `json` outputs expose all header fields. |
| Show directories or files only | Archive viewers often let users hide folders | Yes | `include_dirs=false` drops directory entries. |
| PAX/GNU long names | Modern tar archives need long-path handling | Yes | Core parser handles PAX `path`/`linkpath` and GNU long-name/long-link records. |
| Extract, preview or download member contents | GUI archive viewers can open files | Out of model | This tool is intentionally metadata-only and never unpacks payloads. |
| Support `.tar.bz2`, `.tar.xz`, `.tar.zst` in-page | Native tar can use compressors when installed | Out of model for first version | The pure wasm deps here only include gzip. Other compressors are detected and rejected clearly; users can decompress first. |
| Direct file upload | Online archive viewers take file uploads | Out of model for current pure text schema | The shared CLI/chat/page schema uses base64/hex text input for deterministic cross-surface tests. |

## Defaults and UX choices

- Default encoding: `base64`, because it is the safest way to paste binary archives into text fields.
- Default output: `table`, matching the familiar `tar -tvf` long-listing view.
- Default sort: `archive`, preserving the physical member order from the tar stream.
- Default `include_dirs`: true, because `tar -tvf` includes directory headers when present.
- Default `limit`: 500, high enough for small packages while keeping browser output bounded.
- Preset chips cover the common competitor workflows: long listing, paths only, files sorted by size, CSV, JSON, and `*.txt` filtering.

## Worked examples to support

1. A small `.tar.gz` sample should produce a long listing with directory/file modes, sizes and mtimes.
2. `output=paths` should return exactly the archive member names, preserving archive order by default.
3. `filter=*.txt` should narrow paths without shelling out to grep.
4. `output=json` should include archive totals and structured entries.

## Limits and honesty notes

- Input is capped at 64 MiB after gzip decompression and 200,000 entries.
- The tool does not extract or inspect payload bytes, so it cannot preview file content or validate that a file's body matches a claimed type.
- bzip2/xz/zstd containers are not silently misreported; they return explicit unsupported-compressor errors.
- The tool is distinct from general compression/conversion blocks because it inspects tar header metadata and member structure rather than compressing or transforming raw bytes.
