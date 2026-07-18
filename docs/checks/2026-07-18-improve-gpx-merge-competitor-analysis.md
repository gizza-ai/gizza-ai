# gpx-merge — competitor analysis (2026-07-18)

New pure tool. Scan done BEFORE implementing. Sources skimmed (paraphrased only —
no competitor copy, branding, or trademarks reproduced):

- Dawarich GPX Merger — https://dawarich.app/tools/gpx-merger/
- GOTOES Combine GPS Files — https://gotoes.org/strava/Combine_GPX_TCX_FIT_Files.php
- iloveGPX Merge — https://ilovegpx.org/en/merge
- (also surveyed: The Ride Atlas, gpx.studio, GPXto, Komoot merge tool)

## Table-stakes (each tagged and routed)

| Capability | Competitors | Tag | Where it lands |
| --- | --- | --- | --- |
| Keep tracks separate vs. combine into one track | Dawarich, iloveGPX | in-model | `merge_mode` enum |
| Chronological ordering by timestamp | GOTOES (headline), our description | in-model | `sort_by_time` bool (default on) |
| Preserve elevation (`<ele>`) | iloveGPX, Dawarich | in-model | inherent — always carried |
| Preserve per-point timestamps (`<time>`) | Dawarich | in-model | inherent — always carried |
| Merge waypoints / markers | Dawarich | in-model | `include_waypoints` bool |
| Accept routes (`<rte>`) too, normalize to tracks | iloveGPX | in-model | inherent — `<rtept>` read as track points |
| Many files at once (2–30) | iloveGPX | in-model | multi-document paste, split on `</gpx>` |
| Custom track name / metadata | Dawarich (file description) | in-model | `track_name` param |
| Deduplicate overlapping/consecutive points | (implied by "continuity") | in-model | `dedupe` bool |
| Preserve-segment-breaks for multi-day/with-pauses | iloveGPX (gap detection) | in-model | `merge_mode = single-track-multi-segment` |

## UX control patterns matched
- Dropdown for the keep-separate/combine choice → `Param::enumv` renders a `<select>` with friendly labels.
- Checkboxes for chronological sort / dedupe / waypoints → `Param::boolean` checkboxes.
- Preset chips (`[[example]]`) stand in for competitors' preset merge strategies.

## Out-of-model / considered, not built
- **Interactive map preview + drag-to-reorder** (Ride Atlas, gpx.studio, GPXto, Komoot) — needs a
  Leaflet/MapLibre map surface; doesn't fit the browser-local wasm text-in/text-out page. Ordering
  is instead done deterministically by timestamp.
- **FIT / TCX / CSV input & output** (GOTOES) — FIT is a binary format; TCX↔GPX is already a
  separate gizza tool (`tcx-to-gpx`). Kept this tool GPX-in/GPX-out.
- **Timestamp shifting / offset per file** (GOTOES) — considered, rejected: niche, adds schema bloat,
  and conflicts with the "order by the real recorded time" premise.
- **Drag-and-drop multi-file upload** — the pure page takes pasted text; multiple documents are
  pasted concatenated and split on the `</gpx>` boundary. Stated as a limit on the page.
- **Cloud save / account sync** (Dawarich) — out of model (no backend/accounts).

## Design decision
Descriptor params: `input` (multiline GPX text, ≥1 document), `merge_mode`
(single-track | single-track-multi-segment | separate-tracks), `sort_by_time` (bool, default true),
`dedupe` (bool, default false), `include_waypoints` (bool, default true), `track_name`
(default "Merged track"). Output: one GPX 1.1 document.
