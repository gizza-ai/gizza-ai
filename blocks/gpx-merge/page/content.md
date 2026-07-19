## About this tool

Use GPX Merge when you have several exported rides, runs, hikes, routes or waypoint files and need one clean GPX 1.1 document. Paste the GPX text from each file one after another; the tool collects tracks (`<trk>`), routes (`<rte>`) and waypoints (`<wpt>`) from every pasted document.

By default the result is one continuous track ordered by timestamp, which is useful for joining split recordings or rebuilding a route from separate stages. Switch **Merge mode** to keep each original segment break, or keep each source track as its own `<trk>` when you want the output to preserve daily stages.

### Worked example

Paste two GPX documents, leave **Merge mode** set to `single-track`, and keep **Order chronologically by timestamp** checked. The output contains one `<trk>` named `Merged track`; points from both inputs are sorted by their `<time>` values so a point recorded at `07:00:00Z` appears before points recorded at `08:00:00Z` and `09:00:00Z` even if the files were pasted in a different order.

### Limits and edge cases

- Input is GPX text only. Convert TCX, FIT or GeoJSON with a dedicated converter first.
- Multiple files are represented by pasting several complete GPX documents into the same input field.
- Coordinates, elevations and timestamps are preserved as written; this tool does not simplify or resample tracks.
- Timestamp sorting expects ISO-8601 / RFC-3339 values such as `2024-01-01T08:00:00Z`. Untimed points keep their input order and sort after timed points.
- `dedupe` removes only consecutive duplicate points with the same latitude, longitude and timestamp; it does not perform fuzzy GPS matching.

## FAQ

<details>
<summary>Can I merge more than two GPX files?</summary>

Yes. Paste each complete GPX file one after another in the input box. The parser scans the combined text and collects every track, route and waypoint it finds.

</details>

<details>
<summary>What is the difference between the merge modes?</summary>

`single-track` creates one track with one continuous segment. `single-track-multi-segment` creates one track but keeps each original segment break. `separate-tracks` writes each source track as a separate `<trk>` in the output.

</details>

<details>
<summary>Will waypoints be kept?</summary>

Waypoints are kept by default, including their name, description, symbol, elevation and timestamp when present. Turn off **Keep waypoints** if you only want tracks and routes in the merged file.

</details>

<details>
<summary>Does the tool upload my GPS data?</summary>

No. The merge runs locally in the browser WebAssembly module and returns text directly on the page.

</details>
