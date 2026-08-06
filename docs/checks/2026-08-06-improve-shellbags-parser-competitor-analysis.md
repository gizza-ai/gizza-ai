# shellbags-parser — competitor scan + design decisions (2026-08-06)

Scan run **before** implementation, per `create-next-tool` step 4. Everything below is
paraphrased from public documentation and format specifications; no competitor copy,
branding, or trademarks are reproduced or reused in the tool's page/CLI text.

## Competitors reviewed

| # | Tool | Surface | Reachable |
|---|------|---------|-----------|
| 1 | ShellBags Explorer / SBECmd (Eric Zimmerman) | Windows GUI + CLI | yes (via secondary documentation — the vendor page itself was not fetched) |
| 2 | `shellbags.py` (Willi Ballenthin, on top of `python-registry`) | Python CLI | yes |
| 3 | libyal `libfwsi` — Windows Shell Item format specification (+ the `winreg-kb` shellbag notes it backs) | format spec / library | yes |
| — | Cyber Triage shellbags write-up | vendor blog, used to cross-check field lists and stated caveats | yes |

Two originally-selected sources were unreachable (`blog.cyber5w.com` — DNS failure; a
truncated SBECmd walk-through) and were **replaced**, not dropped, so the scan still covers
three real competitor tools.

## What each competitor does

### 1. ShellBags Explorer / SBECmd
- Reads shellbags out of `NTUSER.DAT` and `UsrClass.dat`, either from files/directories of
  hives or from the live registry of the running user.
- Presents a reconstructed folder **tree** plus a flat table, and exports **CSV**.
- Per-entry fields reported: reconstructed absolute path, the bag/`NodeSlot` number, MRU
  position, the shell item's embedded created/modified/accessed timestamps, the owning
  registry key's last-write time (used to infer "first/last interacted"), an MFT entry
  number when the shell item carries an NTFS file reference, and child-bag counts.
- Ships a de-duplication option and per-hive classification of where an entry came from.

### 2. `shellbags.py`
- CLI: one or more raw hive file paths, `-v` (debug), `-p` (colorized debug),
  `-o {csv,bodyfile}` with **bodyfile as the default**.
- Emits TSK **bodyfile** lines (inode, path, uid/gid, size, atime/mtime/ctime/crtime) and a
  CSV alternative; reconstructed paths are printed with a "(Shellbag)" suffix on the name.
- Walks the `BagMRU` tree and rebuilds paths from the nested `Shell Item` blobs.

### 3. libfwsi (format spec)
Not a UI, but the authoritative structure reference the other two implement:
- Shell item: `size` (u16, includes itself; `0` terminates the list), `class type indicator`
  (u8), then type-specific data.
- Class ranges: `0x1F` root/GUID folder, `0x20–0x2F` volume, `0x30–0x3F` file entry,
  `0x40–0x4F` network location, `0x61` URI, `0x71` control-panel item.
- File entry layout: `+4` file size (u32), `+8` last-modified DOS/FAT date-time (u32),
  `+12` attribute flags (u16), `+14` primary name (ASCII or UTF-16LE, NUL-terminated,
  16-bit aligned).
- Extension block `0xBEEF0004`: `+0` size, `+2` version, `+4` signature, `+8` creation
  DOS/FAT date-time, `+12` last-access DOS/FAT date-time, then (version ≥ 7) an NTFS file
  reference (6-byte MFT entry + 2-byte sequence), then the long UTF-16LE name.
- Root-folder GUIDs (My Computer/This PC, Network, Control Panel, Recycle Bin, Users
  Files, …) map to display names.

## Table stakes → in-model / out-of-model

| # | Capability | Seen in | Verdict | Where it lands |
|---|-----------|---------|---------|----------------|
| 1 | Walk `BagMRU` recursively and reconstruct full folder paths | 1, 2 | **in-model** | core walker, all modes |
| 2 | Auto-detect the shellbag root for both `UsrClass.dat` and `NTUSER.DAT` (incl. the XP `ShellNoRoam` location) | 1, 2 | **in-model** | `bag_root = auto` (default) |
| 3 | Custom/explicit `BagMRU` key path | 2 | **in-model** | `custom_path` |
| 4 | Tree view of the reconstructed hierarchy | 1 | **in-model** | `mode = tree` (default) |
| 5 | Flat list of absolute paths | 1, 2 | **in-model** | `mode = list` |
| 6 | CSV export | 1, 2 | **in-model** | `mode = csv` |
| 7 | TSK bodyfile output | 2 | **in-model** | `mode = bodyfile` |
| 8 | Per-item raw/decoded diagnostics for damaged or unknown shell items | 3 (implied by the spec's fallbacks) | **in-model** | `mode = raw` |
| 9 | MRU position per entry | 1 | **in-model** | emitted in every detail-bearing mode |
| 10 | `NodeSlot` / bag number per entry | 1 | **in-model** | emitted in every detail-bearing mode |
| 11 | Shell-item created / modified / accessed timestamps | 1, 2, 3 | **in-model** | DOS/FAT decode, ISO-8601 output |
| 12 | Registry key last-write time (first/last-interacted proxy) | 1 | **in-model** | emitted per entry |
| 13 | NTFS MFT entry + sequence from the `0xBEEF0004` block | 1, 3 | **in-model** | emitted when version ≥ 7 |
| 14 | GUID → friendly folder-name resolution | 1, 3 | **in-model** | `resolve_guids` (default on) |
| 15 | Volume / drive-letter items (`C:\`) | 1, 3 | **in-model** | class `0x2F` decode |
| 16 | Network location items (`\\server\share`) | 1, 3 | **in-model** | class `0x40–0x4F` decode |
| 17 | Long (UTF-16) name preferred over the 8.3 primary name | 1, 3 | **in-model** | extension-block long name wins |
| 18 | Depth + entry caps so a huge hive stays readable | — (gizza chat/CLI constraint) | **in-model** | `max_depth`, `max_entries` |
| 19 | Hex **and** Base64 hive input | — (gizza page/chat constraint; competitors take file paths) | **in-model** | `input_encoding` |
| 20 | Read hives directly from a mounted disk image / live registry | 1 | **out-of-model** | no filesystem or disk-image access in a wasm sandbox — listed, not built |
| 21 | Replay `.LOG1`/`.LOG2` transaction logs before parsing | 1 | **out-of-model** | needs multiple correlated input files; the page takes one blob |
| 22 | Batch a directory of hives across many user profiles | 1, 2 | **out-of-model** | one input per invocation by design |
| 23 | Decode the `Bags\<slot>\Shell` view-preference values (icon size, sort column, window rect) | 1 | **out-of-model for v1** | a separate artifact tree with its own binary blobs; `NodeSlot` is emitted so the slot can be looked up with the sibling `registry-hive-parser` tool |
| 24 | Cross-hive de-duplication of repeated paths | 1 | **out-of-model for v1** | only meaningful across multiple hives, which is #22 |
| 25 | GUI timeline / column sorting | 1 | **out-of-model** | not a UI toolkit |

## UX control patterns adopted

- **Preset chips** (`[[example]]`) for the three journeys a user actually has: reconstruct the
  tree, export CSV, and dump raw shell-item diagnostics. Competitor CLIs express these as
  flag combinations; chips are the declarative equivalent here.
- **`<select>` for every fixed-choice field** (`input_encoding`, `mode`, `bag_root`) with
  friendly `[input.labels]`, so the output format is discoverable instead of being a
  remembered flag.
- **Checkbox** for `resolve_guids` (default on) — the same on/off shape as the competitors'
  friendly-name toggles.
- **Multiline textarea** for the hive bytes, since pasted hex wraps across lines.
- **Numeric caps** (`max_entries`, `max_depth`) surfaced as fields with placeholders rather
  than hidden constants, because the page has to stay responsive on a real 10 MB hive.

## Deliberate differences

- Input is **encoded bytes (hex/Base64), not a file path** — this repo's tools run in a
  browser/chat sandbox with no filesystem. The page states this limit explicitly.
- Everything runs locally; the hive is never uploaded. That is stated on the page as a
  property, not as a marketing claim.
- Where a shell item cannot be decoded, the tool reports the class byte and a hex preview
  instead of inventing a path — the `raw` mode exists precisely so an analyst can see what
  the parser saw.

## Stated limits carried onto the page

- Shellbags record folders browsed in Explorer/Open-Save dialogs, **not individual files**,
  and cannot prove a folder's contents were viewed or that access succeeded.
- Timestamps inside a shell item are the folder's own MACB values *as recorded when the bag
  was written*, which is not the same as when the user browsed it; the registry key's
  last-write time is the better proxy for that.
- Transaction-log replay is not performed, so a dirty hive may be missing the most recent
  entries.
- Decoding is best-effort for undocumented/vendor-specific shell item classes.
