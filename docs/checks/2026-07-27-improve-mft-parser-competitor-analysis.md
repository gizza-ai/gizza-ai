# mft-parser — competitor analysis (2026-07-27)

Scan done BEFORE implementation. All findings paraphrased — no competitor copy, branding,
or trademarks reproduced. `mft-parser` is a binary-file-input forensic parser, so it ships
on the **chat + CLI** surfaces only (no standalone page — same shape as `evtx-parser`,
`pcap-network-forensics`; a raw `$MFT` binary with structured-JSON output fits neither the
text-field page nor the ffmpeg media page).

## What the tool does

Parses the NTFS Master File Table (`$MFT`) — the on-disk database NTFS keeps one 1024-byte
record per file/directory in — into a structured, filterable list of entries for filesystem
timeline / DFIR work. Each record carries two independent timestamp sets: `$STANDARD_INFORMATION`
(SI) and `$FILE_NAME` (FN), each with created / modified / MFT-modified / accessed (the "MACB"
times). Comparing SI vs FN is a classic anti-forensics ("timestomping") tell.

## Competitors scanned (top real tools)

1. **MFTECmd** (Eric Zimmerman / EZ Tools) — the de-facto IR standard. Windows-only .NET CLI.
   Parses `$MFT`, and also `$Boot`, `$J` (`$UsnJrnl`), `$SDS`, `$LogFile`. Emits analyst-ready
   CSV for Timeline Explorer, plus JSON and bodyfile. Flags SI-vs-FN timestamp anomalies.
2. **analyzeMFT.py** — long-standing open-source Python parser. CSV + bodyfile (mactime) output,
   full path reconstruction, deleted-record surfacing, anomaly/timestomp columns.
3. **omerbenamram/mft** — the pure-Rust crate + `mft_dump` CLI we build on. Emits JSON / JSONL /
   CSV. Full path resolution with a parent-reference cache; robust per-record error handling.
4. **dfir_ntfs** (Maxim Suhanov) — Python library exposing raw NTFS internals ($MFT, $UsnJrnl,
   $LogFile, volume images, VSS). Programmatic, library-first.
5. **mftparser.com** — browser-based `$MFT` analyzer with an interactive sortable table, path
   column, deleted/active filter, and CSV export; positions on "no upload, runs locally".

## Table-stakes params / features (tagged in-model / out-of-model)

| Capability | Status | Decision |
| --- | --- | --- |
| Per-record entry # + sequence # | in-model | `entry`, `sequence` fields |
| SI timestamps (C/M/MFT/A) | in-model | `standard_info` object |
| FN timestamps (C/M/MFT/A) | in-model | `file_name` object |
| Full path reconstruction (parent chain) | in-model | `path` via `get_full_path_for_entry` |
| File vs directory | in-model | `is_directory` |
| In-use vs deleted (unallocated record) | in-model | `in_use` + `status` filter |
| Logical file size | in-model | `size` (from best `$FILE_NAME`) |
| Timestomp / SI-vs-FN anomaly flag | in-model | `timestomp_suspect` + `only_timestomp` filter |
| Filter by path / name substring | in-model | `path_contains` |
| Filter files-only / dirs-only | in-model | `include` enum |
| Result cap for large tables | in-model | `max_records` (report says `truncated`) |
| Triage counts before drilling in | in-model | `summary` mode (counts + time span) |
| CSV / bodyfile (mactime, Timeline Explorer) output | **out-of-model** | Considered; our native surface is structured JSON the LLM reads directly (and the CLI prints). A downstream CSV/bodyfile shaping step is out of scope for a JSON tool — noted, not built. |
| `$UsnJrnl` / `$LogFile` / `$Boot` / `$SDS` parsing | **out-of-model** | Those are *other* NTFS streams, not `$MFT`. Out of this tool's scope. |
| Resident file-content / ADS extraction | **out-of-model** | Niche; would bloat output. Considered, not built. |
| Interactive sortable HTML table | **out-of-model** | No page for a binary-input tool; chat/CLI render JSON. |

## Worked example (SI-vs-FN)

A file whose `$STANDARD_INFORMATION.created` is years before its `$FILE_NAME.created` is a
timestomp candidate: an attacker backdated the SI times (visible in Explorer) but the FN times
(set by NTFS on link/rename) kept the true recent date. `timestomp_suspect=true` surfaces exactly
these; `only_timestomp=true` returns just them.

## UX / control patterns adopted

- Fixed-choice params are real enums (`include`, `status`) so the chat schema + CLI advertise the
  allowed values.
- Every filter defaults to "no filter" so a bare call returns a bounded, useful listing.
- `summary=true` mirrors the "triage first" workflow all the CLIs support.
- `max_records` default keeps output bounded; `truncated`/`matched_entries` tell the caller more matched.

## Limits stated to the user

- 64 MiB input cap (re-export a slice or a smaller `$MFT` if larger).
- Records with out-of-range/zeroed FILETIMEs are skipped and counted in `parse_errors`.
- Path reconstruction is best-effort: an orphaned record whose parent chain is missing falls back
  to its own filename.
