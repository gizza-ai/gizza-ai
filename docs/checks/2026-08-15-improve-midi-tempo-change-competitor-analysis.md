# midi-tempo-change competitor analysis — 2026-08-15

Backlog row: `midi-tempo-change` — changes the tempo/BPM of a MIDI file while preserving note relationships.

## Competitors scanned

| Tool | What it offers | Table-stakes signals | Fit decision |
| --- | --- | --- | --- |
| MIDI Toolbox player | Browser-local MIDI playback with speed/BPM controls, looping and track mute/solo for practice. | Exact BPM/speed control, local file handling, practice-oriented slow-down/speed-up language, no-upload privacy copy. | In-model: BPM and speed multiplier, local browser processing, practice presets/copy. Out-of-model: live playback, piano roll, loop/mute/solo UI. |
| Bear File Converter / ofoct Change MIDI BPM | Upload a MIDI file, change BPM, download the converted file. | Simple target-BPM conversion, output download, explains that changing BPM changes duration. | In-model: target BPM, download .mid, duration-change summary. Out-of-model: server upload/storage workflow (we avoid it). |
| MIDI Editor Online / PureMIDI editor family | Full browser MIDI editor with piano roll, tempo/time-signature editing, playback, export. | Import .mid, edit tempo/time signature, preview, export MIDI/audio; multi-track UI. | In-model: tempo-map rewrite, preserve notes/controllers, export MIDI. Out-of-model: piano-roll note editing, audio render/export, SoundFont playback, visual score editing. |

## Required in-model behavior

- Accept Standard MIDI File bytes locally and return a downloadable `.mid` file.
- Set an exact target BPM for the first tempo event.
- Scale an existing tempo by a multiplier for practice slow-down/speed-up.
- Preserve note tick relationships, velocities, controllers, program changes, track names and time signatures.
- Handle files with existing tempo maps in two user-visible ways: scale every tempo event or flatten to one constant tempo.
- Report original BPM, new BPM, speed ratio, tempo-event counts, track/PPQ/note counts and playing-time before/after so users can verify the edit.
- Include an explicit re-notation option (`keep_duration`) for matching a project BPM while preserving wall-clock duration.
- Provide browser examples/preset chips for exact BPM, 75% practice speed, double speed, flattened tempo, same-duration retiming and base64 input.

## Out-of-model / not built

- Piano-roll note editing, note selection, quantization, velocity editing and track mute/solo are full DAW/editor surfaces, not this conversion tool.
- Audio rendering/playback with SoundFonts and WAV/MP3 export is outside this pure MIDI-byte rewrite.
- Looping practice playback is a player feature; this tool only rewrites and returns the MIDI file.
- Server-side upload/storage workflows are intentionally not copied; the page runs locally in the browser.

## Descriptor/page decisions

- `mode = set-bpm | scale`: mirrors competitor split between exact BPM and speed multiplier.
- `bpm`: 20–400 BPM, default 120.
- `factor`: 0.1–10×, default 1.0.
- `tempo_map = scale | flatten`: keeps or collapses existing tempo changes.
- `keep_duration`: non-default checkbox that rescales event ticks for the re-notation workflow.
- Page output is a custom JSON envelope rendered as a human summary plus `Download .mid`, with Playwright decoding the actual data URL to assert the tempo meta bytes and note delta.
