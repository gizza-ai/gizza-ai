# music-file-renamer — competitor scan + design decisions (2026-08-22)

Scan run **before** implementing, per `/create-next-tool` step 4. All findings are paraphrased
from public documentation; no competitor copy, branding or trademarked wording is reproduced or
used in the tool's page/descriptor.

## Tools skimmed

| # | Tool | What it is | Reachable |
|---|------|-----------|-----------|
| 1 | beets — path-format templates (docs) | Library manager; renames/moves files from tags via a template language | yes |
| 2 | Renamer (macOS) — "music" use case page | Batch renamer with an ID3 rename action | yes |
| 3 | Rename Expert — "rename MP3 files" example page | Windows batch renamer with ID3 placeholders | yes |
| 4 | Mp3tag — Tag→Filename converter | Tag editor whose converter also creates folders from the pattern | doc URL 404'd; only the search-result summary was available, so it is used as corroboration, not as one of the three primary reads |
| — | MusicBrainz Picard file-naming options | Wanted for its ASCII / Windows-compatibility switches | **unreachable** (404 on every doc path tried) — replaced by Rename Expert as the third primary read |

## Table-stakes findings

| # | Capability | Seen in | In/out of model | Decision |
|---|-----------|---------|-----------------|----------|
| 1 | Token/placeholder pattern over tag fields | all four | **in** | `pattern` param, `{token}` syntax, default `{artist}/{album}/{track} {title}` |
| 2 | Pattern creates **folders**, not just names (`Artist\Year - Album\Track - Title`) | Mp3tag, beets | **in** | `/` and `\` in the pattern both split into path components; the result is a move plan |
| 3 | Field set: artist, album artist, album, title, track, disc, year, genre, composer, comment, bitrate | Rename Expert, beets, Renamer | **in** | canonical tokens for all of them + any extra column/tag passes through as its own token |
| 4 | Prefer album artist over artist so split-artist albums don't fragment | beets (called out explicitly) | **in** | fallback chains: `{albumartist|artist}` takes the first non-empty |
| 5 | Zero-padded track numbers (`01`) | Renamer, Mp3tag, beets | **in** | `track_padding` (default 2); `03/12` and `3` both normalise to `03` |
| 6 | Case functions (`%upper`, `%lower`, `%title`) | beets | **in** | `case_style` = keep / lower / upper / title |
| 7 | Accent folding / ASCII-only paths (`%asciify`, "replace non-ASCII") | beets, Picard | **in** | `charset = ascii` transliterates Latin-1/Latin-Ext-A (`café → cafe`, `ß → ss`) |
| 8 | Replace characters illegal on the target filesystem | beets (`replace`), Picard (Windows compat) | **in** | `charset` = windows / unix / ascii + `replace_char`; Windows also fixes trailing dots/spaces and `CON`/`LPT1`-style reserved names |
| 9 | Max filename length | beets (`max_filename_length`) | **in** | `max_component` (default 100 chars per path component, cap 255) |
| 10 | Spaces → underscores | Mp3tag/Rename Expert presets | **in** | `space_style` = keep / underscore / hyphen |
| 11 | Skip files that lack the needed tag | Rename Expert (skips files with no ID3 data) | **in** | `on_missing` = unknown / skip / keep_original |
| 12 | Live preview before committing; nothing renamed until confirmed | Renamer, Rename Expert | **in** | the tool is **preview-only by design** — it never touches a filesystem, it emits a plan |
| 13 | Destination root / "move to library folder" | beets, Picard | **in** | `base_dir` prefix |
| 14 | Collision detection (two files → one target) | implied by every previewer | **in** | case-insensitive collision detection, flagged per entry + counted |
| 15 | Saved/reusable rename recipes | Renamer ("Renamerlets") | **in** (as presets) | six `[[example]]` chips on the page; the deep-linkable URL is the shareable recipe |
| 16 | Conditionals (`%if{}`), substrings (`%left{}`), disambiguators (`%aunique{}`) | beets | **out** | not built — the fallback-chain syntax covers the common `%if{albumartist,...}` case; full scripting is out of scope for a one-shot tool |
| 17 | Reading tags off the files themselves / walking a folder | all four (they are desktop apps) | **out** | a browser/CLI tool cannot walk a music folder. Mitigated: the tool eats the tag dumps those workflows already produce — Mp3tag-style CSV export, `ffprobe -show_format`, `exiftool`, and JSON from `ffprobe -print_format json`/music-metadata. `blocks/bpm-key-tag-reader` covers reading tags out of a single uploaded file. |
| 18 | Actually performing the move + deleting emptied folders | Picard, Renamer | **out** | no filesystem access. Mitigated: `format = sh` emits a reviewable `mkdir -p` + `mv -n` script you run yourself. |
| 19 | Online lookup / AcoustID fingerprint to fill missing tags | Picard | **out** | needs a network service + fingerprinting model |
| 20 | Multiple output shapes for the plan | none of them (they all only preview in-app) | **in** — differentiator | `format` = table / list / csv / json / sh |

## Feasibility spikes before tagging out-of-model

- **Accent folding without a crate** (#7): confirmed doable with a `char → &str` fold table over
  Latin-1 Supplement + Latin Extended-A plus combining-mark stripping, so `ascii` shipped rather
  than being deferred to a `deunicode` dependency. Core stays on `serde_json` only.
- **Fallback chains** (#4/#16): `{albumartist|artist|Unknown}` is a two-line change in the token
  walker, so beets' single most-recommended `%if` use case is covered without a script engine.
- **`sh` output** (#18): quoting with `'…'` and `'\''` escaping is enough to emit a safe script for
  arbitrary tag text, so the "actually move the files" gap is closed as far as it can be.

## Not a duplicate of `blocks/bulk-file-renamer`

`bulk-file-renamer` transforms **filename strings** (find/replace, regex, sequential numbering, case)
and never looks at metadata. This tool is **tag-driven**: it parses tag records (CSV/TSV, JSON,
`key=value` blocks) and builds a folder-and-file path out of the tag values. Different input, different
engine, no overlapping parameters. `bpm-key-tag-reader` reads tempo/key tags out of one audio file and
does not produce paths. Confirmed by reading each block's `core/src/lib.rs`.

## Verification note

Everything in the "in" column above is exercised by the unit tests, the CLI checks and
`tests/tool-page-music-file-renamer.spec.ts` (one real run per enum choice, per accepted input
format, a non-default checkbox state, and the exact `MAX_TRACKS` boundary at 5000/5001).
