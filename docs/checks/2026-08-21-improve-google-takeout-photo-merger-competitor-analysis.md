# google-takeout-photo-merger — competitor analysis (2026-08-21)

Scan run **before** implementation, per `.claude/skills/create-next-tool/SKILL.md` step 3.
All findings are paraphrased from public documentation; **no competitor copy, branding, or
trademark is reproduced or reused** anywhere in this block.

## Backlog row

```
google-takeout-photo-merger,image,
"Merges Google Takeout JSON sidecar metadata (dates, GPS, descriptions) back into each photo's EXIF.",
type_hint=pure
```

## Viability check (done first, per loop rules)

**Not a duplicate.** The closest existing blocks were each checked in source:

| block | what it does | why this tool is distinct |
|---|---|---|
| `blocks/exif-edit` | writes/removes individual EXIF fields on **one** JPEG/PNG from **explicit scalar args** (`date_taken`, `latitude`, `longitude`, `altitude`, `description`, …) | it cannot read a Google sidecar, cannot pair sidecars to photos, and handles one photo per call. This tool's whole value is the sidecar **parse + pairing + batch** layer. We reuse `exif-edit`'s engine as a crate dependency rather than re-implementing it. |
| `blocks/image-metadata-viewer`, `blocks/metadata-privacy-linter` | **read** EXIF | read-only; no write path |
| `blocks/strip-exif`, `blocks/gps-location-remover` | **remove** metadata | opposite direction |
| `blocks/photo-gps-mapper` | reads GPS out of a batch of photos into GeoJSON/CSV/GPX | read-only, no sidecars |
| `blocks/gmail-takeout-parser`, `blocks/youtube-takeout-stats` | other Takeout products (mbox, watch history) | different archive contents |
| `blocks/archive-extractor`, `blocks/create-zip` | generic zip in / zip out | no metadata logic |

**In-model.** The tool is inherently batch ("back into **each** photo's EXIF"), which rules out a
single-image descriptor. The repo already supports the shape needed:

- **Batch input** as one `Input::File` ZIP — the `zip` crate (deflate only, no default features) is
  proven wasm32-safe in `blocks/archive-extractor` and `blocks/create-zip`.
- **Batch output** as one ZIP envelope (`application/zip`) — an established pattern across 20 blocks
  (`archive-extractor`, `app-icon-set`, `favicon-generator`, `gif-extract-frames`, …).
- **EXIF write** via `gizza-ai-exif-edit-core` (`img-parts` + `kamadak-exif`), pure Rust, already
  shipping. Cross-block core dependencies are an established pattern (`document-text-extract`,
  `email-list-cleaner`, `browser-history-parser`, …).

Surfaces: **chat + CLI, no standalone page** — a ZIP result fits neither the text page nor the
ffmpeg-media page shape (the same no-page file-input pattern as `archive-extractor`). Stated
explicitly rather than claimed as a pass.

## Competitors reviewed

1. **google-photos-exif** (mattwilson1024, GitHub) — Node CLI, `--inputDir/--outputDir/--errorDir`.
2. **gophix** (alexdachin, GitHub) — Go CLI wrapping ExifTool; `fix` and `clean-json` commands.
3. **Metadata Fixer** (metadatafixer.com) — hosted commercial service; its public explainer of the
   Takeout sidecar format and filename rules is the most complete reference of the three.

A fourth data point (a Chrome-extension "merger") was skimmed but adds nothing beyond the above.

## Table stakes → decision

| # | Capability (paraphrased) | Seen in | Decision |
|---|---|---|---|
| 1 | Pair each media file with its sidecar JSON | all 3 | **in-model — built** (`pair_sidecars`) |
| 2 | Legacy `NAME.jpg.json` naming | all 3 | **built** |
| 3 | Current `NAME.jpg.supplemental-metadata.json` naming | Metadata Fixer | **built** |
| 4 | Truncated sidecar names (`.supplemental-metad.json`, `.suppleme.json`, …) — Google clips the whole sidecar name at ~46/51 chars | Metadata Fixer | **built** (prefix match on `NAME.jpg.suppl…`) |
| 5 | Bare-stem `NAME.json` fallback | google-photos-exif | **built** |
| 6 | `-edited` / `-bewerkt`-style suffix stripped before matching | google-photos-exif | **built** (`-edited` and the counter forms) |
| 7 | Duplicate counters: `NAME(1).jpg` pairs with `NAME.jpg(1).json` | google-photos-exif | **built** (both the swapped and the naive form) |
| 8 | Write date-taken from `photoTakenTime.timestamp` (Unix epoch) | all 3 | **built** — `DateTimeOriginal` + `DateTimeDigitized` + `DateTime` |
| 9 | Fall back to `creationTime` when `photoTakenTime` is absent | Metadata Fixer | **built** (`date_source` param) |
| 10 | Write GPS from `geoData` / `geoDataExif` | gophix, Metadata Fixer | **built** — `gps_source` param, `auto` prefers a non-zero `geoData` and falls back to `geoDataExif` |
| 11 | Write altitude | gophix | **built** (part of the `gps` field group) |
| 12 | Write the user's caption/description | gophix, Metadata Fixer | **built** — `ImageDescription` |
| 13 | Only fill fields that are **missing** by default; do not clobber real camera EXIF | google-photos-exif | **built** — `overwrite` (default `false`) |
| 14 | Fix wrong file extensions (a PNG named `.jpg`) | gophix | **built** — `fix_extension` (default `true`), magic-byte based |
| 15 | Set the file's timestamp so the OS/importer sorts correctly | google-photos-exif, Metadata Fixer | **built** — `set_file_times` (default `true`), writes the ZIP entry's DOS timestamp |
| 16 | Drop the now-redundant `.json` sidecars from the result | gophix (`clean-json`) | **built** — `keep_sidecars` (default `false`) |
| 17 | Report unmatched / unwritable files instead of silently dropping them | all 3 | **built** — every file is carried through to the output ZIP and classified in the report |
| 18 | Preview before writing | Metadata Fixer | **built** — `dry_run` returns the plan as a report, no ZIP |
| 19 | Choose which metadata groups to apply | gophix (implicit) | **built** — `fields` (`date,gps,description`) |

## Out of model (listed, deliberately not built)

| Capability | Why it is out of model here |
|---|---|
| HEIC / MP4 / MOV / AVI metadata writing | `exif-edit`'s engine (and the whole repo's pure-Rust metadata stack) writes EXIF into JPEG APP1 and PNG `eXIf` only. HEIC needs an ISOBMFF box writer; MP4/MOV need atom-level `mvhd`/`©day`/`udta` editing. Non-JPEG/PNG files are copied through untouched and reported as `skipped_unsupported`. google-photos-exif has the same JPEG/HEIC-only limit for the same reason. |
| XMP sidecar fallback for unwritable formats (gophix does this) | would emit a *second* file per photo; adds a metadata dialect the repo has no writer for. Reported instead. |
| Timezone derived from GPS position (Metadata Fixer) | needs a shipped tz-boundary database (tens of MB) plus historical DST rules. Sidecar timestamps are UTC epochs and are written as UTC; the tool documents this rather than guessing a local time. |
| People/face-tag names → keywords | Google's `people` array has no lossless standard EXIF home (`XPKeywords` is Windows-only UTF-16); punting is safer than inventing a mapping. |
| Multi-archive Takeout exports (photo in zip 1, sidecar in zip 7) | needs cross-call state. Documented: extract the parts together and re-zip, or run per album. Every competitor CLI has the same constraint (Metadata Fixer solves it only by holding the whole upload server-side). |
| Whole-Takeout (multi-GB) uploads | the block's input cap is 64 MiB, in line with `archive-extractor`. Documented as an album-at-a-time workflow. |
| Automatic download/unzip of the Takeout link | no long-lived storage in this model. |

## Notes

- No competitor wording, naming, or trademark is used in the descriptor, summary, or CLI copy.
- Out-of-model rows above are *listed*, per loop rules — none of them were silently dropped.
