# itunes-library-parser — competitor scan + design decisions (2026-08-22)

Scan run **before** finalising the page copy, descriptor wording and parameter set, per
`/create-next-tool` step 3 / `/improve-tool` Phase 2. Everything below is paraphrased from public
documentation; no competitor copy, branding, product name or trademarked wording is reproduced in
the tool's page, descriptor or help text.

## Tools skimmed

| # | Tool | What it is | Reachable |
|---|------|-----------|-----------|
| 1 | A long-running Java/WinForms iTunes playlist exporter (ericdaugherty.com dev page) | Desktop + CLI exporter: reads the library XML, writes playlist files and optionally copies the audio | yes |
| 2 | `SebastianMuskalla/iExport` (GitHub README) | .NET CLI that parses the library XML and exports M3U/M3U8 playlists or copied file trees | yes |
| 3 | `samuel-walker/itunes-library-to-csv` (GitHub README) | Python script: one CSV per playlist, three columns | yes |
| 4 | A .NET desktop "library XML → M3U" utility aimed at network-speaker playback (simav8.com) | GUI that rewrites paths for a NAS/speaker and writes one M3U per playlist | yes |
| — | Generic "XML → M3U" web converters (101convert, itstillworks write-ups) | Marketing/how-to pages for generic file-conversion sites | skimmed; no real feature surface documented, used only as evidence that the *online, no-install* niche is thin |

## Table-stakes findings

| # | Capability | Seen in | In/out of model | Decision |
|---|-----------|---------|-----------------|----------|
| 1 | Parse the XML property list Music/iTunes writes (`Library.xml`) | all four | **in** | `plist` crate; `library` param takes the pasted XML. Binary `.itl` is not readable by anyone here either — stated as a limit, not silently failed |
| 2 | Export playlists as M3U | 1, 2, 4 | **in** | `output = m3u` (paths only) |
| 3 | Extended M3U with `#EXTM3U` / `#EXTINF` duration+title lines | 1 (EXT mode), 2 | **in** | `output = m3u8` emits the extended form. Note the naming split: tool 2 uses `.m3u8` purely to signal UTF-8 paths. We treat the two values as *plain vs extended*, and say so in the copy, because a browser tool is UTF-8 either way |
| 4 | Rewrite the music-folder prefix so paths resolve on another machine/NAS/drive letter | 1 (`-musicPath`), 4 (source\|target pairs) | **in** | `path_prefix` replaces the library's own `Music Folder` prefix; tracks outside it keep their path |
| 5 | Slash-direction conversion for the target player | 2, 4 | **in** | `path_style = original / unix / windows` |
| 6 | Flatten to bare file names (car radios, flat copied folders) | 2 (folder-copy modes), 1 (FLAT copy) | **in** | `path_style = filename` |
| 7 | Hide the app's own playlists (Library / Music / Downloaded / distinguished kinds) | 1 (`-includeAllWithBuiltin`), 2 (`ignoreDistinguishedPlaylists`), 3 (hard-coded filter list) | **in** | `include_builtin` (default off). Detection uses `Master` + `Distinguished Kind` **plus** a well-known-name fallback, because older/third-party exports omit both |
| 8 | Export one named playlist rather than everything | 1, 4 | **in** | `playlist` (case-insensitive); empty = whole library |
| 9 | List what playlists exist, with folder hierarchy | 1 (`-includeFolders`), 4 (checkbox list), 2 (hierarchical names) | **in** | `output = playlists` — a CSV index of name, kind (playlist/smart/folder/built-in), track count, parent folder, persistent ID |
| 10 | Playlist folders are containers, not track lists | 2 | **in** | selecting a folder returns a clear error instead of an empty export |
| 11 | CSV of track metadata | 3 | **in**, widened | tool 3 emits three fixed columns; we ship 39 selectable columns via `fields`, plus TSV and JSON |
| 12 | Sorting: tool 2 documents that playlist order is unreliable and hard-codes an artist/year/album sort | 2 | **in**, made configurable | `sort_by` = original / name / artist / album / year / duration / play_count / date_added, using `Sort Name` / `Sort Artist` / `Sort Album` when present. Tool 2's fixed order is not user-configurable; ours is |
| 13 | Library-level statistics | none of the four | **in** — differentiator | `output = summary`: track count, tracks with no file, total size, total playing time, playlist breakdown, music folder, app version, top artists/genres |
| 14 | Star ratings, play counts and date fields as usable values | none (3 stops at name/artist/album) | **in** — differentiator | `rating` renders iTunes' 0–100 as 0–5 stars, `rating_raw` keeps the original; `date_added`/`play_date` render as ISO-8601 |
| 15 | Preview a big library before committing | none | **in** | `limit` (0 = all, ceiling 100000) applied after sorting |
| 16 | Copying the actual audio files into a flat / Artist-Album / per-playlist tree | 1, 2 | **out** | no filesystem access from a browser tool or a one-shot CLI call. Mitigated: `path_style = filename` + `path_prefix` produce exactly the playlist text those copied trees need |
| 17 | Writing one file per playlist to an output folder | 1, 2, 3, 4 | **out** | one run returns one text artifact. Mitigated: `output = playlists` gives you the list, then one run per playlist; the page URL is deep-linkable so each is a bookmark |
| 18 | Checking that each track file actually exists on disk | 2 (noted as slow) | **out** | no filesystem access. Mitigated: `summary` reports how many tracks have no `Location` at all (cloud-only / missing-file entries) |
| 19 | WPL / ZPL / MPL playlist formats | 1 | **out** | player-specific XML wrappers with a much smaller audience than M3U; not built. Revisit only if asked for |
| 20 | Auto-discovering the library file on the local machine | 1, 2, 4 (desktop apps) | **out** | the user pastes the XML. This is also the privacy win: nothing is uploaded, fetched or written |

## Gaps closed by this pass

- **Naming ambiguity on `m3u8` (#3).** Two of the four tools mean "UTF-8 paths" by `.m3u8` while
  the rest of the world means "extended M3U". The page copy and the `output` describe() now say
  explicitly that this tool's `m3u8` is the **extended** form and that both outputs are UTF-8.
- **Path re-rooting is the single most-repeated feature (#4/#5)** across three of the four tools —
  it is the reason these utilities exist at all (moved drive, NAS, different OS). Promoted to a
  worked example on the page and to a preset chip, rather than being a buried option.
- **Built-in-playlist filtering (#7)** is implemented three different ways across the competitors;
  none of them handle libraries that carry neither `Master` nor `Distinguished Kind`. Kept the
  name-list fallback and documented it in the FAQ.
- **`playlists`-first discovery (#9).** Tool 4's whole UI is a checkbox list of playlist names.
  The error message for an unknown playlist now names real playlists from the file and points at
  `output = playlists`.

## Not a duplicate of an existing block

- `blocks/plist-viewer` renders **any** property list as JSON or a `plutil -p` tree. It has no
  track/playlist model: no field selection, no `file://` decoding, no M3U, no path re-rooting, no
  summary. Confirmed by reading its `core/src/lib.rs` (params: `format`, `indent`, `sort_keys`,
  `data_encoding`).
- `blocks/music-file-renamer` consumes tag dumps and emits a rename/move **plan**; it never parses
  a library file and never emits a playlist.
- `blocks/xml-to-csv` is a generic element→row flattener; an iTunes library is a plist `<dict>`
  keyed by track ID, which flattens to nothing useful.

## Decisions carried into the page

1. Placeholders on every text/number field (library XML snippet, playlist name, field list,
   `D:\Music` prefix, `20`).
2. Friendly `<select>` labels for `output`, `path_style` and `sort_by`; canonical values unchanged.
3. Five `[[example]]` preset chips: default CSV export, playlist → extended M3U, re-root onto a
   Windows drive, playlist index, library summary.
4. A worked example on the page showing a real library snippet and its exact CSV output.
5. Limits and edge cases stated up front: 20 MB per run, 100000-row ceiling, XML-only (no `.itl`),
   nothing uploaded.
