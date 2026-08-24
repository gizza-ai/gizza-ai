# bpm-key-tag-reader — competitor analysis (2026-08-20)

Scope: read the **stored** tempo (BPM) and musical-key metadata out of a track's tags —
ID3v2 `TBPM`/`TKEY`, MP4 `tmpo`, Vorbis-comment `BPM`/`INITIALKEY`, and the DJ-software
`TXXX:` variants — and normalise the key into the notations DJs actually use.
This is a **tag reader**, not an audio analyser: nothing here estimates tempo or key from
the waveform (see "Out of model" below).

Scan done BEFORE implementation. All findings paraphrased; no competitor copy, branding
or trademark text is reused anywhere in this tool.

## Competitors reviewed (3 real tools)

| # | Tool | What it is | Relevant behaviour |
|---|------|-----------|--------------------|
| 1 | **Mp3tag** (mapping reference) | Desktop tag editor; publishes the canonical cross-container field-mapping table | Documents BPM as ID3v2.3/2.4 `TBPM`, MP4/iTunes `tmpo`, WMA `WM/BeatsPerMinute`; key as ID3v2.3/2.4 `TKEY`, WMA `WM/InitialKey`. Vorbis-comment and APEv2 fields are deliberately *not* remapped — they are surfaced under their literal field name. Takeaway: a reader must show the **raw field name it read from**, not just a value. |
| 2 | **tuneXplorer** (Abyss Media) | BPM/key analyser that writes its results back into tags | Writes/reads `TKEY`, `TBPM` and the comment field, targeting ID3v2.3 **and** 2.4 for compatibility with iTunes, Serato, Traktor, rekordbox and Virtual DJ. Handles MP3, WAV, FLAC, OGG, WMA, AIFF and M4A (AAC + ALAC) without converting first. Exports results as CSV. Takeaway: format breadth is table stakes, and the "written by DJ software" tags are the ones users actually have. |
| 3 | **Mixxx** (open-source DJ software) | Library/DJ app that reads key tags and re-displays them | Reads the stored key and renders it in a user-selected notation: Traditional, Lancelot (= Camelot) or Open Key. Its `keyutils` tables are the authoritative mapping used below. Takeaway: a raw `TKEY` string is close to useless to a DJ unless it is converted to Camelot/Open Key. |

Notation constants were taken from Mixxx's `src/track/keyutils.cpp` rather than memory:
C major = Camelot `8B` = Open Key `1d`; A minor = Camelot `8A` = Open Key `1m`
(Open Key number = Camelot number + 5, mod 12).

## Table stakes → where each one landed

| Capability | Verdict | Where it lands |
|---|---|---|
| Read ID3v2 `TBPM` (MP3/AIFF/WAV) | in-model | `bpm.source_field = "TBPM"` |
| Read MP4/iTunes `tmpo` atom | in-model | symphonia `isomp4` maps it to the standard BPM key |
| Read Vorbis-comment `BPM` (FLAC/Ogg) | in-model | matched case-insensitively |
| Read ID3v2 `TKEY` | in-model | `key.source_field = "TKEY"` |
| Read `TXXX:INITIALKEY` / Vorbis `INITIALKEY` (Mixed In Key, Serato, Traktor, rekordbox exports) | in-model | `TXXX:` prefix stripped before matching |
| Read MP4 freeform `com.apple.iTunes:initialkey` | in-model | symphonia surfaces freeform atoms with that key shape |
| Show the **raw field name** a value came from (Mp3tag) | in-model | every reading carries `source_field` + `source_tag` |
| Convert key → Traditional / Camelot / Open Key (Mixxx) | in-model | `key_notation` param: `standard` \| `camelot` \| `open-key` \| `all` (default `all`) |
| Accept a key already stored **as** Camelot or Open Key and convert back | in-model | parser accepts `8A`, `08A`, `1m`, `12d` as input forms |
| Accept the ID3v2 `TKEY` spec forms, incl. `o` = off-key | in-model | reported as `off-key` with a note |
| Format breadth: MP3, FLAC, OGG, M4A/MP4, WAV, AIFF, CAF, MKV/WebM (tuneXplorer) | in-model | symphonia demuxer feature set |
| Mixed In Key energy level (`TXXX:EnergyLevel`) | in-model (bonus) | reported when present |
| Dump every other tag found, not just BPM/key | in-model | `include_all_tags` boolean (default off, capped) |
| Clean "no tempo/key tags stored" answer instead of an error | in-model | `found = false` + an actionable note |

## Out of model (listed, deliberately not built)

- **Detecting BPM/key from the audio itself** (Tunebat, tuneXplorer's analyser, vocalremover,
  TagMyBeat). That is DSP/ML analysis, already ruled out for this repo — see the
  `bpm-detect`, `video-beat-detector` and `chord-recognizer` entries in `docs/tool-skiplist.txt`.
  This tool only reports what is already *stored* in the file.
- **Writing tags back** (tuneXplorer). This is a read-only reporter; `audio-metadata-stripper`
  covers the removal side.
- **Batch folder scan + CSV export** (tuneXplorer). The block takes a single `url`⊕`ref`
  source; multi-file batch has no page/chat surface in this model.
- **WMA/ASF `WM/BeatsPerMinute` + `WM/InitialKey`, and APEv2 tags** (Mp3tag's table).
  symphonia ships no ASF demuxer and no APEv2 tag reader, so these containers cannot be
  opened at all here. Named explicitly in the tool's own docs so the gap is honest.

## Surface decision

File-in → text-out with a pure-Rust engine, so this is the established **no-page
file-input** shape (same as `media-info`, `file-metadata-inspect`, `detect-file-type`):
chat + CLI only. The page generator has no render mode for a media-file → text report.
