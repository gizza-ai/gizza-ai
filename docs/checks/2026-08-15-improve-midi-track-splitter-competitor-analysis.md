# midi-track-splitter — competitor analysis (2026-08-15)

Scan run BEFORE implementation, per `/create-next-tool` step 3. All notes are **paraphrased**;
no competitor copy, branding or assets are reproduced or reused. Search query:
"split multi-track MIDI file into separate single track MIDI files online tool".

## Competitors reviewed

| # | Tool | Kind | Reachable |
|---|------|------|-----------|
| 1 | MIDI-Splitter-Lite (VirtuosicAI, GitHub) | Desktop GUI utility | yes |
| 2 | midisplit (maxim-zhao, GitHub; built on DryWetMIDI) | CLI | yes |
| 3 | "MIDI Track Splitter" (v0-midi-to-separate-tracks.vercel.app) | Web app | yes (thin landing copy) |
| — | Bear File Converter / ofoct "Split MIDI Tracks" | Web app | **no** — HTTP 500, replaced by #3 |
| — | fileproinfo MIDI splitter | Web app | **no** — 404, dropped |
| — | musiccreator.ai "MIDI Splitter" | Web app | reachable but **not the same tool** — it is AI audio stem separation (mp3/wav/flac in), account + credits required; noted only as a naming collision |

### 1. MIDI-Splitter-Lite — desktop, track-oriented
- Splits the tracks of one MIDI file into separate files, one per track.
- Explicit option to **copy the first track into every exported file**, documented as the place
  tempo/setup information usually lives.
- Option to read **track names from the file** (drives what the user sees per track).
- Multi-select of tracks (ctrl-click) → export a **subset**, not always all.
- Stated limits: **Format 1 only** (Format 0 unsupported), MIDI 1.0 only.
- Output naming: unspecified in the docs.

### 2. midisplit — CLI, channel/voice-oriented
- `split` produces several files named by a **channel-based pattern** (`name.ch<N>.mid`);
  simultaneous notes on one channel are pushed to extra numbered files so each output is
  monophonic (its stated purpose is feeding a visualiser).
- Also ships `print` (dump the file for inspection) and `singletrack` (the inverse merge).
- Tempo/meta propagation is not documented; no size limits documented.

### 3. v0 MIDI Track Splitter — web
- Drag-and-drop or file-browse upload of a multi-track MIDI, "export each instrument
  separately". No options, limits, naming rules or packaging documented on the page.

## Table stakes extracted (tagged for gizza's pure-Rust / no-model / browser-local fit)

| Capability | Seen in | Fit | Decision |
|---|---|---|---|
| One output file per **track** | 1, 3 | in-model | `split_by = "track"` (default) |
| One output file per **MIDI channel** | 2 | in-model | `split_by = "channel"` |
| Copy the conductor/first track (tempo, time & key signature) into every output | 1 | in-model | `include_conductor` (default **on**) — without it the parts play at the MIDI default 120 BPM |
| Choose which tracks to export | 1 | in-model | `select` — indices and ranges (`1,3-5`); empty = all |
| Output file naming from the track-name meta event | 1, 2 | in-model | names derived from the track name (or channel/program), sanitised, index-prefixed; `filename_prefix` configurable |
| Skip empty / note-less tracks | implied by 1 (the conductor track is never a "part") | in-model | `skip_empty` (default **on**) |
| Format 0 vs Format 1 output | 1 (Format-1 input only) | in-model | `output_format` — `format-0` (true single-track, default) or `format-1` (conductor kept as its own track) |
| Accept **Format 0** input too | gap in 1 | in-model | supported: a Format-0 file is split by channel automatically (there is only one track to split) |
| Per-file stats before download (notes, channel, instrument) | partially 3 | in-model | JSON summary + per-file rows rendered on the page |
| Batch download of every part | 2, 3 | in-model (partly) | per-file Download buttons on the page + all bytes in the JSON/CLI output; a single **ZIP** bundle is *considered, rejected* — a zip dependency for one download click, and the CLI/chat surface already returns every file's base64 in one response |
| Monophonic voice-splitting (one file per simultaneous note) | 2 | in-model but **rejected** | out of scope for "split tracks"; it is a different tool (voice/poly split) and would double the schema |
| Drag-and-drop file upload | 1, 3 | **out-of-model for the pure page runtime** | the pure page runtime has no binary file input (only ffmpeg pages take `source = "file"`); input is base64/hex text, consistent with the existing `midi-to-json` / `midi-note-extract` / `midi-tempo-change` family |
| AI stem separation from audio | musiccreator.ai | **out-of-model** | needs an ML model; already skiplisted repo-wide (`audio-to-midi`, `monophonic-audio-to-midi`) |
| Accounts / credits / cloud queue | musiccreator.ai | **out-of-model** | gizza is browser-local, no account, no server |

## Design decisions taken from the scan

1. **`include_conductor` defaults to on.** The single most common complaint about naive splitters
   is that the exported parts lose the tempo map; competitor 1 exposes this as an option and
   explains why. We copy every global meta event (tempo, time signature, key signature, SMPTE
   offset) from the conductor track into each output, and additionally carry any track-level
   tempo events found anywhere in the file so a Format-0 input splits correctly too.
2. **`format-0` is the default output.** The backlog description asks for "separate single-track
   MIDI files", and a Format-0 file is exactly that; `format-1` stays available for users who want
   the conductor data visible as its own track (which is what competitor 1 produces).
3. **Both split axes ship.** Competitor 1 splits by track, competitor 2 by channel; real files
   need both (a Format-0 file has one track but up to 16 channels).
4. **Named outputs.** Filenames come from the track-name meta event when present, else the
   channel + General MIDI program name, else the index — so a DAW import shows `02-Bass.mid`
   rather than `track2.mid`. Every name is sanitised and de-duplicated.
5. **Stated limits on the page** (none of the three competitors state theirs): 4 MiB decoded
   input, at most 64 output files, SMPTE-timecode files are accepted and their division is
   preserved, and Format 2 files are rejected with an explanatory message.
6. **No copied copy.** Page title, hero, FAQ and examples are original.
