# zip-integrity-checker — competitor analysis (2026-08-17)

Scan run **before** implementing (per `/create-next-tool` step 4). All notes are **paraphrased
observations of function**; no competitor copy, branding, or trademarks were reused.

## Model-fit decision (checked first)

The row is `type_hint = pure`, and it is: **ZIP bytes in → JSON verdict out**, no ffmpeg, no
model, no network beyond fetching the archive. Built as a pure-Rust block with `Input::File`
(`url` ⊕ `ref`).

**No standalone page.** This is the file-input → JSON shape (`unzip`, `zip-inspect`,
`extract-tar`, `detect-file-type`): the page generator's form drives a single upload → one
rendered artifact, and a per-entry pass/fail report is not one of its output modes. Surfaces are
therefore **chat + CLI**, matching the sibling archive blocks. Stated explicitly rather than
faked.

**Not a duplicate.** The closest sibling, `blocks/zip-inspect`, is a *listing* tool whose own
core doc-comment states it "NEVER decompresses entry data" — it reports the CRC-32 value **as
recorded in the central directory** and never checks whether the stored data actually hashes to
it. This tool does the opposite work: it decompresses every entry and recomputes the CRC-32 to
compare against the recorded one, which is the only way corruption is detected. `blocks/unzip` /
`blocks/archive-extractor` extract content (and would fail opaquely on a bad entry rather than
report which entry and why); `blocks/verify-checksum` compares one whole blob against one
user-supplied digest, with no ZIP structure awareness; `blocks/identify-archive-format` only
sniffs magic bytes. Nothing in `blocks/` verifies per-entry ZIP CRC-32s.

## Competitors reviewed

| # | Tool | Shape | Reachable |
|---|------|-------|-----------|
| 1 | Info-ZIP `unzip -t` | CLI, tests every member in memory | yes (man page) |
| 2 | 7-Zip `t` command | CLI/GUI archive test | yes (command reference) |
| 3 | unziper.com "test archive" | Web upload → integrity verdict | yes |
| 4 | Python `zipfile.ZipFile.testzip()` | Library call | reference-level (stdlib semantics) |
| 5 | WinZip / WinRAR "Test archive" | Desktop GUI action | listed only (behaviour from search summaries) |

### 1. Info-ZIP `unzip -t`
Extracts each member **into memory** (never to disk), recomputes the CRC of the expanded data and
compares it with the CRC stored for that member, printing a message when they differ. It names
each file as it tests it, and prints a single closing summary line of the
"no errors detected in …" form. Verbosity is tunable (`-q`, `-qq`) so scripts can ask for the
verdict without the per-file chatter. Distinct exit statuses separate "clean", "warnings",
"failed because the compression method or decryption is unsupported", and "bad password", so a
caller can branch on the *reason*, not just pass/fail. `-P` supplies a password for encrypted
members.

### 2. 7-Zip `t`
Same job across many container formats, with an output log level switch (quiet / errors /
errors+warnings / verbose), include/exclude filters, recursion into nested archives, and a
password switch. Its report prints archive-level facts (path, type, physical size, headers size,
method, block/solid info) alongside the verdict, and on failure names the offending file with a
data-error label plus an error count. On success it prints an everything-is-OK line followed by
folder/file counts and total sizes.

### 3. unziper.com "test archive"
Browser upload, no options at all. Reports the archive type, the original and compressed sizes,
and the number of files and folders, with an overall verdict. Its own notes state the size
ceiling is bounded by device RAM (they quote a ~4 GB theoretical maximum), that testing large
archives can be slow, and — importantly — that it detects problems but **cannot repair** them.

### 4. Python `zipfile.testzip()`
Minimal API: reads every member and returns the name of the **first** member whose CRC does not
match (or nothing if all are fine). Useful as a semantic floor — a machine-readable "which entry
is bad", not a formatted report.

### 5. WinZip / WinRAR "Test"
Desktop menu action over a selected archive; the surrounding vendor material frames a CRC
mismatch as the archive being damaged in transfer/storage, and pushes their repair features.

## Table stakes → decision

| # | Capability | Fit | Where it lands |
|---|-----------|-----|----------------|
| 1 | Decompress every entry and compare recomputed CRC-32 vs the stored one | in-model | core `check()`; per-entry `expected_crc32` + `actual_crc32` |
| 2 | Per-entry pass/fail with the *reason*, not just a boolean | in-model | `status` = `ok` / `crc_mismatch` / `size_mismatch` / `data_error` / `unsupported_method` / `encrypted`, plus a human `detail` |
| 3 | Single overall verdict + closing summary line | in-model | `ok` bool + `summary` string |
| 4 | Archive-level facts (entry/file/dir counts, total + compressed sizes) | in-model | `count`, `file_count`, `dir_count`, `total_size`, `total_compressed_size`, `bytes_verified` |
| 5 | Quiet / errors-only output level (`unzip -qq`, 7-Zip `-bb`) | in-model | `report` = `all` \| `problems` (enum) |
| 6 | Distinguish "failed" from "could not be tested" (unsupported method / encryption) | in-model | separate `failed_count` vs `skipped_count`; `ok` is false only for real failures |
| 7 | Name the first bad entry (testzip semantics) | in-model | `first_bad_entry` field |
| 8 | Structural validation of the archive itself (central directory, local headers, truncation) | in-model | `ZipArchive::new` + per-entry local-header read; truncated data surfaces as `data_error` with the byte counts |
| 9 | Detect a prepended stub / junk before the ZIP (self-extracting archives) | in-model | `prepended_bytes` (archive start offset) |
| 10 | Surface the archive comment | in-model | `comment` (omitted when empty) |
| 11 | Bounded work on hostile input (zip-bomb / decompression-ratio guard) | in-model | `max_uncompressed_mb` (1–4096, default 512) with an actionable over-budget error |
| 12 | Password for encrypted members (`unzip -P`, 7-Zip `-p`) | **out of model** | the `zip` dep is built `default-features = false, features = ["deflate"]` to stay wasm32-safe — no AES / ZipCrypto decrypt is wired (same constraint that skiplisted `zip-extract`). Encrypted entries are reported as `encrypted` + skipped, never silently passed |
| 13 | Other containers (RAR, 7z, tar, gz) | **out of model** here | ZIP only, by name and scope; `blocks/identify-archive-format` sniffs formats and `blocks/archive-extractor` handles the tar/gzip/bzip2/xz/zstd/lz4 family |
| 14 | Repairing a damaged archive | **out of model** | detection only — stated on the tool's own description, as unziper does |
| 15 | Recursing into nested archives / include-exclude filters (7-Zip) | **out of model** | one archive per call; a nested archive can be extracted with `unzip` and re-checked |
| 16 | 4 GB-class archives | **out of model** | input is capped at 64 MiB (the sandbox budget shared with `unzip`); the cap is stated in the parameter description and the error |

Every table stake above is either implemented or listed as out-of-model — none dropped silently.

## Deltas we ship that the competitors do not

- **Both CRCs in the report.** `unzip`/7-Zip tell you an entry failed; this returns the recorded
  CRC *and* the recomputed CRC side by side, so a mismatch is auditable and can be pasted into a
  bug report.
- **Declared-size verification.** The uncompressed byte count is compared with the size recorded
  in the central directory (`size_mismatch`), which catches a header-vs-data disagreement that a
  pass/fail CRC test alone would not attribute.
- **Machine-readable, one call.** JSON with an `ok` boolean, per-entry statuses and a
  `first_bad_entry` — the exit-code branching `unzip` needs, without parsing console text.

## UX / control patterns

Nothing to mirror on a page (there is none). The competitors' only real control surface is a
verbosity level and a password field — the former is `report`, the latter is out of model. The
CLI form is `gizza tool zip-integrity-checker url=… report=problems`.
