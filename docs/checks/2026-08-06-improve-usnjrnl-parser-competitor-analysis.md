# usnjrnl-parser — competitor analysis (2026-08-06)

Scan run **before** implementation, per the create-next-tool / improve-tool procedure.
Everything below is **paraphrased**. No competitor copy, branding, trademark, layout or
asset was reproduced.

## Scan

One search for the tool's function ("USN journal parser $UsnJrnl $J parse file create
rename delete forensics"). The result set is dominated by DFIR tooling; three real,
reachable tools were skimmed in depth.

| # | Tool | Shape | Reachable |
|---|------|-------|-----------|
| 1 | usnparser.com — browser USN Journal viewer/parser | Web app, WebAssembly, client-side | yes |
| 2 | USN Analytics (Kazamiya / Forensicist) | Native CLI, Win/Linux/macOS, Apache-2.0 | yes |
| 3 | USN-Journal-Parser (PoorBillionaire) | Python CLI | yes |
| — | MFTECmd, TZWorks JP, NTFS Journal Viewer | Native Windows GUI/CLI, referenced for feature vocabulary only | partially |

### 1. Browser USN Journal viewer (usnparser.com)

- Drag-and-drop file upload of an extracted `$Extend\$UsnJrnl:$J` stream.
- Optional second upload of `$MFT` purely to resolve full paths for each record.
- Parses entirely in the browser with WebAssembly; states nothing is uploaded.
- Advertises create / delete / rename / metadata-modification events as the useful classes.
- UX surface is thin: two drop zones, a parse button, a language picker. No documented
  filtering, sorting, per-record column list, or size limit.
- Positioning: incident response, intrusion timelines, ransomware, insider threat,
  recovering evidence of deleted files.

### 2. USN Analytics

- Correlates records by file reference number and walks parent references to
  **reconstruct full paths**.
- **Pairs rename/move records into a single row** instead of emitting the
  old-name and new-name records separately — the single strongest analysis idea in the scan.
- Timestamps interpreted as local time by default, `-u` switches to UTC.
- `-r` selects a plain "just parse" mode versus its richer analysis output.
- Also derives program-execution and file-open history by cross-referencing prefetch /
  LNK / ObjectID artifacts, and flags suspicious extensions and file names.
- Accepts carved USN data (bulk_extractor `ntfsusn` output) as well as a whole `$J`.

### 3. USN-Journal-Parser (PoorBillionaire)

- Output formats: default human-readable, `-c` CSV, `-b` mactime bodyfile, `-t` TLN
  (paired with `-s <system>` for the host column), `-v` verbose JSON with every property.
- Emitted fields: timestamp, file name, file attributes, reason codes, version, file
  reference number, parent reference number, USN, security id, and the MFT entry /
  sequence numbers decomposed out of the file reference.
- `-q` "quick" mode for large journals — i.e. skipping the sparse/zero regions rather
  than byte-walking them.
- No documented handling of V3 / V4 record layouts.

## Table stakes extracted

| Table stake | Source | Decision |
|---|---|---|
| Decode USN_RECORD_V2 fully | all three | **built** |
| Decode USN_RECORD_V3 (128-bit file references) | gap in #3, needed for modern volumes | **built** |
| Recognise USN_RECORD_V4 range-tracking records | not covered by any competitor | **built** (counted + reported, never mis-shown as a file event) |
| Skip sparse / zeroed regions of `$J` and resynchronise after garbage | #3 `-q` | **built** (always on, with the skipped/resynced byte counts reported) |
| Decompose file reference → MFT entry + sequence | #3 | **built** |
| Decode reason bitmask to named flags | all three | **built** (24 documented `USN_REASON_*` flags; undocumented bits surface as `UNKNOWN_0x…` rather than being dropped) |
| Decode file-attribute bitmask | #1, #3 | **built** |
| Change-class filter (create / delete / rename / write / metadata / close) | #1 | **built** (`event`) |
| Files-only / directories-only filter | implied by attribute decoding | **built** (`include`) |
| Name substring filter | #2 (suspicious name/extension hunting) | **built** (`filter`) |
| Pair `RENAME_OLD_NAME` + `RENAME_NEW_NAME` into one row | #2 | **built** (`pair_renames`, default on) |
| CSV output | #3 | **built** (`mode=csv`) |
| mactime bodyfile output | #3 | **built** (`mode=bodyfile`) |
| TLN output with a host column | #3 (`-t` + `-s`) | **built** (`mode=tln` + `host`) |
| Verbose JSON with every field | #3 (`-v`) | **built** (`mode=json`) |
| Human-readable default | all three | **built** (`mode=report`, plus a dense `mode=list`) |
| Triage counts before drilling in | #2's analysis framing | **built** (`mode=summary`) |
| UTC timestamps | #2 (`-u`) | **built** — output is always UTC ISO-8601, stated on the page |
| Sort control | #2/#3 implicit | **built** (`sort` = journal order / newest first / name) |
| Result cap for a huge journal | #3 `-q` framing | **built** (`max_entries`, default 200, cap 5000) |

## Out-of-model (considered, not built)

- **Full path reconstruction from `$MFT`** (#1's optional second upload, #2's parent walk).
  A `$J` stream carries only the parent *reference number*, never the parent's name, so
  paths require a second artifact. Our page and chat surfaces take a single artifact per
  call, so this cannot be honest here. The parent MFT entry + sequence are emitted on every
  row so an analyst can join them against the sibling `mft-parser` tool, and the page says so.
- **Reading a live volume / disk image / mounted `\\.\C:`** (#1, #2, TZWorks JP). Browser-local
  wasm has no raw-device access.
- **Cross-artifact correlation with Prefetch / LNK / ObjectID / `$LogFile`** (#2, TriForce-style).
  Multi-artifact input, out of the one-artifact-per-call model.
- **Local-time output** (#2's default). A no-account browser tool would have to guess the
  acquisition host's timezone; a wrong guess silently mis-times a timeline, so output is
  UTC-only and labelled as such.
- **Carving records out of raw unallocated space** (#2's bulk_extractor path). The resync
  scanner already recovers records from a partially-zeroed or truncated `$J`, but a
  whole-disk carve is a different input shape.

## Considered, rejected (in-model but declined)

- **Suspicious-extension / suspicious-name scoring** (#2). Ships as a static blocklist in
  practice, ages badly, and produces confident-looking false positives on a forensic
  timeline. The `filter` substring plus the change-class filter cover the same hunting
  workflow without pretending to a verdict.
- **A `tag-list` control for `filter`.** File names legitimately contain commas, and the
  field is a single substring, not a set.

## Feasibility spike (before tagging anything out-of-model)

USN record decoding is fixed-layout little-endian structure walking — no crate needed and
no dependency risk: `serde`-free pure `u32`/`u64`/UTF-16LE reads over a `&[u8]`. FILETIME →
ISO-8601 UTC reuses the same integer civil-calendar conversion already proven in
`registry-hive-parser` / `amcache-parser` (no `chrono`/`jiff`, so the browser
`wasm32-unknown-unknown` build stays clean). Base64 input decoding uses `base64 0.22`,
already proven under wasmi by `amcache-parser`. Nothing in the table-stakes list needed a
crate that could fail to instantiate, so nothing was deferred for feasibility reasons.

## Surfaces verified

- `cargo test --workspace` (core unit + block drift-guard).
- `scripts/build-block-wasm.sh usnjrnl-parser` → committed `Cargo.lock` + `target/block.wasm`.
- `wasm-pack build blocks/usnjrnl-parser/web --target web --release --out-dir pkg`.
- `gizza tool usnjrnl-parser …` — one exact-output case plus one run per advertised enum
  value, both `pair_renames` states, and the `max_entries` cap boundary.
- Playwright `tests/tool-page-usnjrnl-parser.spec.ts` — real parsed output plus a `?param=`
  deep-link case.
- `python3 scripts/check-tool-hygiene.py usnjrnl-parser` → exit 0.
