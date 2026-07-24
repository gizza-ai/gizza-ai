## What this tool does

Paste a GPX track and split it into smaller segments. Choose **distance** to cut
after a target length, **time** to cut after an elapsed duration, or **stops** to
start a new segment when there is a large timestamp gap between points. The tool
can output a fresh GPX document with one track per segment, or a text summary of
distance, duration, and point counts.

Everything runs locally in WebAssembly. No map service or upload is required.

## Worked example

With `mode=stops`, `stop_gap_s=120`, and `output=summary`, a track with a
20-minute pause produces two segments:

```text
Split into 2 segments (stops, on gaps over 120 s).

Segment 1: 0.11 km (0.07 mi), 2 points, 0:01:00  [2024-01-01T00:00:00Z → 2024-01-01T00:01:00Z]
Segment 2: 0.11 km (0.07 mi), 2 points, 0:01:00  [2024-01-01T00:21:00Z → 2024-01-01T00:22:00Z]
```

Switch `output` to `gpx` when you want a copyable GPX file with tracks named
`Original name - Part 1`, `Original name - Part 2`, and so on.

## Limits and edge cases

- Time and stop modes require parseable GPX `<time>` values on track points.
- Distances use great-circle haversine distance between consecutive points.
- Distance/time splits duplicate the boundary point so segments remain
  geometrically contiguous; stop splits leave the real recording gap.
- The output preserves latitude, longitude, elevation, and timestamp values, but
  GPX extensions such as heart rate, cadence, or power are not retained.
- This is deterministic text processing; there is no interactive map preview or
  click-to-split UI.

## FAQ

<details>
<summary>Does this upload my route?</summary>

No. The GPX parser and splitter run locally in your browser. The tool does not
call a map API or send your track to a server.

</details>

<details>
<summary>Which split mode should I choose?</summary>

Use `distance` for equal-length chunks, `time` for workout intervals or elapsed
duration chunks, and `stops` when your recorder paused for breaks and you want
each continuous movement section separated.

</details>

<details>
<summary>Why did time or stops mode fail?</summary>

Those modes need timestamps on the GPX points. If your file has coordinates but
no `<time>` elements, use distance mode instead.

</details>

<details>
<summary>Are sensor extensions preserved?</summary>

No. The generated GPX keeps coordinates, elevation, and timestamps. Extra
extensions such as heart rate, cadence, power, or app-specific metadata are
dropped in this version.

</details>
